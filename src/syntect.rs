use crate::chunk::File;
use crate::chunk::Line;
use crate::printer::{Printer, PrinterOptions, TermColorSupport, TextWrapMode};
use anyhow::Result;
use flate2::read::ZlibDecoder;
use memchr::{memchr_iter, Memchr};
use rgb2ansi256::rgb_to_ansi256;
use std::cmp;
use std::collections::HashSet;
use std::ffi::OsStr;
use std::fmt;
use std::io::Write;
use std::io::{self, Stdout, StdoutLock};
use std::ops::{Deref, DerefMut};
use std::path::Path;
use std::str::Chars;
use syntect::highlighting::{
    Color, FontStyle, HighlightIterator, HighlightState, Highlighter, Style, Theme, ThemeSet,
};
use syntect::parsing::{ParseState, ScopeStack, SyntaxReference, SyntaxSet};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

// Note for lifetimes:
// - 'file is a lifetime for File instance which is passed to print() method
// - 'main is a lifetime for the scope of main function (the caller of printer)

const SYNTAX_SET_BIN: &[u8] = include_bytes!("../assets/syntaxes.bin");
const THEME_SET_BIN: &[u8] = include_bytes!("../assets/themes.bin");

pub trait LockableWrite<'a> {
    type Locked: Write;
    fn lock(&'a self) -> Self::Locked;
}

impl<'a> LockableWrite<'a> for Stdout {
    type Locked = StdoutLock<'a>;
    fn lock(&'a self) -> Self::Locked {
        self.lock()
    }
}

pub fn list_themes<W: Write>(mut out: W) -> Result<()> {
    let mut seen = HashSet::new();
    let bat_defaults = bincode::deserialize_from(ZlibDecoder::new(THEME_SET_BIN))?;
    let defaults = ThemeSet::load_defaults();
    for themes in &[bat_defaults, defaults] {
        for name in themes.themes.keys() {
            if !seen.contains(name) {
                writeln!(out, "{}", name)?;
                seen.insert(name);
            }
        }
    }
    Ok(())
}

// Use u64::log10 once it is stabilized: https://github.com/rust-lang/rust/issues/70887
#[inline]
fn num_digits(n: u64) -> u16 {
    (n as f64).log10() as u16 + 1
}

#[derive(Debug)]
pub struct PrintError {
    message: String,
}

impl PrintError {
    fn new<S: Into<String>>(msg: S) -> Self {
        Self {
            message: msg.into(),
        }
    }
}

impl std::error::Error for PrintError {}

impl fmt::Display for PrintError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Error while printing output with syntect: {}",
            &self.message
        )
    }
}

struct Token<'line> {
    style: Style,
    text: &'line str,
}

impl<'line> Token<'line> {
    fn chomp(&mut self) {
        if self.text.ends_with('\n') {
            self.text = &self.text[..self.text.len() - 1];
            if self.text.ends_with('\r') {
                self.text = &self.text[..self.text.len() - 1];
            }
        }
    }
}

#[derive(Clone, Copy)]
enum RegionBoundary {
    Start,
    End,
}

enum DrawEvent {
    RegionStart,
    RegionEnd,
    Char(char),
    TokenBoundary(Style), // Previous style
    Done,
}

struct DrawEvents<'a, 'line: 'a> {
    tokens: &'a [Token<'line>],
    chars_in_token: Chars<'line>,
    regions: &'a [(usize, usize)],
    current_style: Style,
    in_region: bool,
    byte_offset: usize,
}

impl<'a, 'line: 'a> DrawEvents<'a, 'line> {
    fn new(tokens: &'a [Token<'line>], regions: &'a [(usize, usize)]) -> Self {
        let (chars_in_token, current_style, tokens) =
            if let Some((head, tail)) = tokens.split_first() {
                (head.text.chars(), head.style, tail)
            } else {
                ("".chars(), Style::default(), tokens)
            };

        Self {
            tokens,
            chars_in_token,
            regions,
            current_style,
            in_region: false,
            byte_offset: 0,
        }
    }

    fn region_boundary(&mut self) -> Option<RegionBoundary> {
        let o = self.byte_offset;

        // Eat done regions
        let num_done_regions = self.regions.iter().take_while(|(_, e)| *e < o).count();
        if num_done_regions > 0 {
            self.regions = &self.regions[num_done_regions..];
        }

        let (s, e) = *self.regions.first()?;
        if o == s {
            if o == e {
                None
            } else {
                Some(RegionBoundary::Start)
            }
        } else if o == e {
            // When the next region is adjcent, skip changing highlight
            match self.regions.get(1) {
                Some((s, _)) if o == *s => None,
                _ => Some(RegionBoundary::End),
            }
        } else {
            None
        }
    }

    fn next_event(&mut self) -> DrawEvent {
        match self.region_boundary() {
            Some(RegionBoundary::Start) if !self.in_region => {
                self.in_region = true;
                return DrawEvent::RegionStart;
            }
            Some(RegionBoundary::End) if self.in_region => {
                self.in_region = false;
                return DrawEvent::RegionEnd;
            }
            _ => { /* fall through */ }
        }

        if let Some(c) = self.chars_in_token.next() {
            self.byte_offset += c.len_utf8();
            return DrawEvent::Char(c);
        }

        if let Some((head, tail)) = self.tokens.split_first() {
            let prev_style = self.current_style;
            self.current_style = head.style;
            self.chars_in_token = head.text.chars();
            self.tokens = tail;
            DrawEvent::TokenBoundary(prev_style)
        } else {
            DrawEvent::Done
        }
    }
}

struct Canvas<'file, W: Write> {
    out: W,
    theme: &'file Theme,
    true_color: bool,
    has_background: bool,
    match_bg: Option<Color>,
    region_fg: Option<Color>,
    region_bg: Option<Color>,
    current_fg: Option<Color>,
    current_bg: Option<Color>,
}

impl<'file, W: Write> Deref for Canvas<'file, W> {
    type Target = W;
    fn deref(&self) -> &Self::Target {
        &self.out
    }
}
impl<'file, W: Write> DerefMut for Canvas<'file, W> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.out
    }
}

impl<'file, W: Write> Canvas<'file, W> {
    fn draw_spaces(&mut self, num: usize) -> Result<()> {
        for _ in 0..num {
            self.out.write_all(b" ")?;
        }
        Ok(())
    }

    fn draw_newline(&mut self) -> Result<()> {
        writeln!(self.out, "\x1b[0m")?; // Reset on newline to ensure to reset color
        self.current_fg = None;
        self.current_bg = None;
        Ok(())
    }

    fn set_color(&mut self, code: u8, c: Color) -> Result<()> {
        // In case of c.a == 0 and c.a == 1 are handling for special colorscheme by bat for non true
        // color terminals. Color value is encoded in R. See `to_ansi_color()` in bat/src/terminal.rs
        match c.a {
            0 if c.r <= 7 => write!(self.out, "\x1b[{}m", c.r + code)?, // 16 colors; e.g. 3 => 33 (Yellow), 6 => 36 (Cyan) (code=30)
            0 => write!(self.out, "\x1b[{};5;{}m", code + 8, c.r)?, // 256 colors; code=38 for fg, code=48 for bg
            1 => { /* Pass through. Do nothing */ }
            _ if self.true_color => {
                write!(self.out, "\x1b[{};2;{};{};{}m", code + 8, c.r, c.g, c.b)?
            }
            _ => write!(
                self.out,
                "\x1b[{};5;{}m",
                code + 8,
                rgb_to_ansi256(c.r, c.g, c.b),
            )?,
        }
        Ok(())
    }

    fn set_bg(&mut self, c: Color) -> Result<()> {
        if self.current_bg != Some(c) {
            self.set_color(40, c)?;
            self.current_bg = Some(c);
        }
        Ok(())
    }

    fn set_fg(&mut self, c: Color) -> Result<()> {
        if self.current_fg != Some(c) {
            self.set_color(30, c)?;
            self.current_fg = Some(c);
        }
        Ok(())
    }

    fn set_default_bg(&mut self) -> Result<()> {
        if self.has_background {
            if let Some(bg) = self.theme.settings.background {
                self.set_bg(bg)?;
            }
        }
        Ok(())
    }

    fn set_background(&mut self, c: Color) -> Result<()> {
        if self.has_background {
            self.set_bg(c)?;
        }
        Ok(())
    }

    fn set_bold(&mut self) -> Result<()> {
        self.out.write_all(b"\x1b[1m")?;
        Ok(())
    }

    fn set_underline(&mut self) -> Result<()> {
        self.out.write_all(b"\x1b[4m")?;
        Ok(())
    }

    fn set_font_style(&mut self, style: FontStyle) -> Result<()> {
        if style.contains(FontStyle::BOLD) {
            self.set_bold()?;
        }
        if style.contains(FontStyle::UNDERLINE) {
            self.set_underline()?;
        }
        Ok(())
    }

    fn unset_font_style(&mut self, style: FontStyle) -> Result<()> {
        if style.contains(FontStyle::BOLD) {
            self.out.write_all(b"\x1b[22m")?;
        }
        if style.contains(FontStyle::UNDERLINE) {
            self.out.write_all(b"\x1b[24m")?;
        }
        Ok(())
    }

    fn set_style(&mut self, style: Style) -> Result<()> {
        self.set_background(style.background)?;
        self.set_fg(style.foreground)?;
        self.set_font_style(style.font_style)?;
        Ok(())
    }

    fn set_match_bg_color(&mut self) -> Result<()> {
        if let Some(bg) = self.match_bg {
            self.set_bg(bg)?;
        }
        Ok(())
    }

    fn set_match_style(&mut self, style: Style) -> Result<()> {
        self.set_fg(style.foreground)?;
        self.set_match_bg_color()?;
        self.set_font_style(style.font_style)
    }

    fn set_region_color(&mut self) -> Result<()> {
        if let Some(c) = self.region_fg {
            self.set_fg(c)?;
        }
        if let Some(c) = self.region_bg {
            self.set_bg(c)?;
        }
        Ok(())
    }

    fn fill_spaces(&mut self, written_width: usize, max_width: usize) -> Result<()> {
        if written_width < max_width {
            self.draw_spaces(max_width - written_width)?;
        }
        Ok(())
    }
}

struct LineChars<'a> {
    horizontal: &'a str,
    vertical: &'a str,
    vertical_and_right: &'a str,
    down_and_horizontal: &'a str,
    up_and_horizontal: &'a str,
    dashed_horizontal: &'a str,
}

const UNICODE_LINE_CHARS: LineChars<'static> = LineChars {
    horizontal: "─",
    vertical: "│",
    vertical_and_right: "├",
    down_and_horizontal: "┬",
    up_and_horizontal: "┴",
    dashed_horizontal: "╶",
};

const ASCII_LINE_CHARS: LineChars<'static> = LineChars {
    horizontal: "-",
    vertical: "|",
    vertical_and_right: "|",
    down_and_horizontal: "-",
    up_and_horizontal: "-",
    dashed_horizontal: "-",
};

// Note: More flexible version of syntect::easy::HighlightLines for our use case
struct LineHighlighter<'a> {
    hl: Highlighter<'a>,
    parse_state: ParseState,
    hl_state: HighlightState,
    syntaxes: &'a SyntaxSet,
}

impl<'a> LineHighlighter<'a> {
    fn new(syntax: &SyntaxReference, theme: &'a Theme, syntaxes: &'a SyntaxSet) -> Self {
        let hl = Highlighter::new(theme);
        let parse_state = ParseState::new(syntax);
        let hl_state = HighlightState::new(&hl, ScopeStack::new());
        Self {
            hl,
            parse_state,
            hl_state,
            syntaxes,
        }
    }

    fn skip_line(&mut self, line: &str) {
        let ops = self.parse_state.parse_line(line, self.syntaxes);
        for _ in HighlightIterator::new(&mut self.hl_state, &ops, line, &self.hl) {}
    }

    fn highlight<'line>(&mut self, line: &'line str) -> Vec<Token<'line>> {
        let ops = self.parse_state.parse_line(line, self.syntaxes);
        HighlightIterator::new(&mut self.hl_state, &ops, line, &self.hl)
            .map(|(style, text)| Token { style, text })
            .collect()
    }
}

// Like chunk::Lines, but includes newlines
struct LinesInclusive<'a> {
    lnum: usize,
    prev: usize,
    buf: &'a [u8],
    iter: Memchr<'a>,
}
impl<'a> LinesInclusive<'a> {
    pub fn new(buf: &'a [u8]) -> Self {
        Self {
            lnum: 1,
            prev: 0,
            buf,
            iter: memchr_iter(b'\n', buf),
        }
    }
}
impl<'a> Iterator for LinesInclusive<'a> {
    type Item = Line<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        if let Some(idx) = self.iter.next() {
            let lnum = self.lnum;
            let end = idx + 1;
            let line = &self.buf[self.prev..end];
            self.prev = end;
            self.lnum += 1;
            Some(Line(line, lnum as u64))
        } else if self.prev == self.buf.len() {
            None
        } else {
            let line = &self.buf[self.prev..];
            self.prev = self.buf.len();
            Some(Line(line, self.lnum as u64))
        }
    }
}

// Drawer is responsible for one-time screen drawing
struct Drawer<'file, W: Write> {
    theme: &'file Theme,
    grid: bool,
    term_width: u16,
    lnum_width: u16,
    background: bool,
    first_only: bool,
    gutter_color: Color,
    wrap: bool,
    tab_width: u16,
    chars: LineChars<'file>,
    canvas: Canvas<'file, W>,
}

impl<'file, W: Write> Drawer<'file, W> {
    fn new(out: W, opts: &PrinterOptions, theme: &'file Theme, chunks: &[(u64, u64)]) -> Self {
        let last_lnum = chunks.last().map(|(_, e)| *e).unwrap_or(0);
        let mut lnum_width = num_digits(last_lnum);
        if chunks.len() > 1 {
            lnum_width = cmp::max(lnum_width, 3); // Consider '...' in gutter
        }

        let gutter_color = theme.settings.gutter_foreground.unwrap_or(Color {
            r: 128,
            g: 128,
            b: 128,
            a: 255,
        });

        let (region_fg, region_bg) = if let Some(bg) = theme.settings.find_highlight {
            (theme.settings.find_highlight_foreground, Some(bg))
        } else {
            (None, theme.settings.selection)
        };

        let canvas = Canvas {
            theme,
            true_color: opts.color_support == TermColorSupport::True,
            has_background: opts.background_color,
            region_fg,
            region_bg,
            current_fg: None,
            current_bg: None,
            match_bg: theme.settings.line_highlight.or(theme.settings.background),
            out,
        };

        let chars = if opts.ascii_lines {
            ASCII_LINE_CHARS
        } else {
            UNICODE_LINE_CHARS
        };

        Drawer {
            theme,
            grid: opts.grid,
            term_width: opts.term_width,
            lnum_width,
            background: opts.background_color,
            gutter_color,
            wrap: opts.text_wrap == TextWrapMode::Char,
            tab_width: opts.tab_width as u16,
            first_only: opts.first_only,
            chars,
            canvas,
        }
    }

    #[inline]
    fn gutter_width(&self) -> u16 {
        if self.grid {
            self.lnum_width + 4
        } else {
            self.lnum_width + 2
        }
    }

    fn draw_horizontal_line(&mut self, sep: &str) -> Result<()> {
        self.canvas.set_fg(self.gutter_color)?;
        self.canvas.set_default_bg()?;
        let gutter_width = self.gutter_width();
        for _ in 0..gutter_width - 2 {
            self.canvas.write_all(self.chars.horizontal.as_bytes())?;
        }
        self.canvas.write_all(sep.as_bytes())?;
        for _ in 0..self.term_width - gutter_width + 1 {
            self.canvas.write_all(self.chars.horizontal.as_bytes())?;
        }
        self.canvas.draw_newline()
    }

    fn draw_line_number(&mut self, lnum: u64, matched: bool) -> Result<()> {
        let fg = if matched {
            self.theme.settings.foreground.unwrap()
        } else {
            self.gutter_color
        };
        self.canvas.set_fg(fg)?;
        self.canvas.set_default_bg()?;
        let width = num_digits(lnum);
        self.canvas
            .draw_spaces((self.lnum_width - width) as usize)?;
        write!(self.canvas, " {}", lnum)?;
        if self.grid {
            if matched {
                self.canvas.set_fg(self.gutter_color)?;
            }
            write!(self.canvas, " {}", self.chars.vertical)?;
        }
        self.canvas.set_default_bg()?;
        write!(self.canvas, " ")?;
        Ok(()) // Do not reset color because another color text will follow
    }

    fn draw_wrapping_gutter(&mut self) -> Result<()> {
        self.canvas.set_fg(self.gutter_color)?;
        self.canvas.set_default_bg()?;
        self.canvas.draw_spaces(self.lnum_width as usize + 2)?;
        if self.grid {
            write!(self.canvas, "{} ", self.chars.vertical)?;
        }
        Ok(())
    }

    fn draw_separator_line(&mut self) -> Result<()> {
        self.canvas.set_fg(self.gutter_color)?;
        self.canvas.set_default_bg()?;
        // + 1 for left margin and - 3 for length of "..."
        let left_margin = self.lnum_width + 1 - 3;
        self.canvas.draw_spaces(left_margin as usize)?;
        let w = if self.grid {
            write!(self.canvas, "... {}", self.chars.vertical_and_right)?;
            5
        } else {
            write!(self.canvas, "...")?;
            3
        };
        self.canvas.set_default_bg()?;
        let body_width = self.term_width - left_margin - w; // This crashes when terminal width is smaller than gutter
        for _ in 0..body_width {
            self.canvas
                .write_all(self.chars.dashed_horizontal.as_bytes())?;
        }
        self.canvas.draw_newline()
    }

    fn draw_text_wrappping(&mut self, matched: bool, style: Style, in_region: bool) -> Result<()> {
        self.canvas.draw_newline()?;
        self.draw_wrapping_gutter()?;
        if in_region {
            self.canvas.set_region_color()
        } else if matched {
            self.canvas.set_match_style(style)
        } else {
            self.canvas.set_style(style)
        }
    }

    fn draw_line(
        &mut self,
        mut tokens: Vec<Token<'_>>,
        lnum: u64,
        regions: Option<Vec<(usize, usize)>>,
    ) -> Result<()> {
        // The highlighter requires newline at the end. But we don't want it since
        // - we sometimes need to fill the rest of line with spaces
        // - we clear colors before writing newline
        if let Some(tok) = tokens.last_mut() {
            tok.chomp();
        }

        let body_width = (self.term_width - self.gutter_width()) as usize;
        let matched = regions.is_some();

        let tokens = tokens.as_slice();
        let regions = regions.as_ref().map(AsRef::as_ref).unwrap_or(&[][..]);
        let mut events = DrawEvents::new(tokens, regions);

        self.draw_line_number(lnum, matched)?;
        if matched {
            self.canvas.set_match_style(events.current_style)?;
        } else if !tokens.is_empty() {
            self.canvas.set_style(events.current_style)?;
        }

        let mut width = 0; // Text width written to terminal
        let mut saw_zwj = false;
        loop {
            match events.next_event() {
                DrawEvent::Char('\t') if self.tab_width > 0 => {
                    let w = self.tab_width as usize;
                    if width + w > body_width && self.wrap {
                        self.canvas.draw_spaces(body_width - width)?;
                        self.draw_text_wrappping(matched, events.current_style, events.in_region)?;
                        width = 0;
                    } else {
                        self.canvas.draw_spaces(w)?;
                        width += w;
                    }
                }
                DrawEvent::Char(c) => {
                    // Handle zero width joiner
                    let w = if c == '\u{200d}' {
                        saw_zwj = true;
                        0
                    } else if saw_zwj {
                        saw_zwj = false;
                        0 // Do not count width while joining current character into previous one with ZWJ
                    } else {
                        c.width_cjk().unwrap_or(0)
                    };
                    if width + w > body_width && self.wrap {
                        self.canvas.draw_spaces(body_width - width)?;
                        self.draw_text_wrappping(matched, events.current_style, events.in_region)?;
                        width = 0;
                    }
                    write!(self.canvas, "{}", c)?;
                    width += w;
                }
                DrawEvent::TokenBoundary(prev_style) => {
                    if !events.in_region {
                        self.canvas.unset_font_style(prev_style.font_style)?;
                        if !matched {
                            self.canvas
                                .set_background(events.current_style.background)?;
                        }
                        self.canvas.set_fg(events.current_style.foreground)?;
                        self.canvas
                            .set_font_style(events.current_style.font_style)?;
                    }
                }
                DrawEvent::RegionStart => {
                    self.canvas.set_region_color()?;
                }
                DrawEvent::RegionEnd => {
                    self.canvas.set_match_style(events.current_style)?;
                }
                DrawEvent::Done => break,
            }
        }

        if matched {
            self.canvas.set_match_bg_color()?;
        } else if width == 0 {
            self.canvas.set_default_bg()?;
        }
        if self.canvas.has_background || matched {
            self.canvas.fill_spaces(width, body_width)?;
        }

        self.canvas.draw_newline()
    }

    fn draw_body(&mut self, file: &File, mut hl: LineHighlighter<'_>) -> Result<()> {
        assert!(!file.chunks.is_empty());

        let mut matched = file.line_matches.as_ref();
        let mut chunks = file.chunks.iter();
        let mut chunk = chunks.next().unwrap(); // OK since chunks is not empty

        // Note: `bytes` contains newline at the end since SyntaxSet requires it. The newline will be trimmed when
        // `HighlightedLine` instance is created.
        for Line(bytes, lnum) in LinesInclusive::new(&file.contents) {
            let (start, end) = *chunk;
            if lnum < start {
                hl.skip_line(String::from_utf8_lossy(bytes).as_ref()); // Discard parsed result
                continue;
            }
            if start <= lnum && lnum <= end {
                let regions = match matched.split_first() {
                    Some((m, ms)) if m.line_number == lnum => {
                        matched = ms;
                        Some(m.ranges.clone()) // XXX: Cannot move out ranges in line match
                    }
                    _ => None,
                };
                let line = String::from_utf8_lossy(bytes);
                // Collect to `Vec` rather than handing HighlightIterator as-is. HighlightIterator takes ownership of Highlighter
                // while the iteration. When the highlighter is stored in `self`, it means the iterator takes ownership of `self`.
                self.draw_line(hl.highlight(line.as_ref()), lnum, regions)?;

                if lnum == end {
                    if self.first_only {
                        break;
                    }
                    if let Some(c) = chunks.next() {
                        self.draw_separator_line()?;
                        chunk = c;
                    } else {
                        break;
                    }
                }
            }
        }

        Ok(())
    }

    fn draw_header(&mut self, path: &Path) -> Result<()> {
        self.draw_horizontal_line(self.chars.horizontal)?;
        self.canvas.set_default_bg()?;
        let path = path.as_os_str().to_string_lossy();
        self.canvas.set_bold()?;
        write!(self.canvas, " {}", path)?;
        if self.background {
            self.canvas
                .fill_spaces(path.width_cjk() + 1, self.term_width as usize)?;
        }
        self.canvas.draw_newline()?;
        if self.grid {
            self.draw_horizontal_line(self.chars.down_and_horizontal)?;
        }
        Ok(())
    }

    fn draw_footer(&mut self) -> Result<()> {
        if self.grid {
            self.draw_horizontal_line(self.chars.up_and_horizontal)?;
        }
        Ok(())
    }
}

fn load_themes(name: Option<&str>) -> Result<ThemeSet> {
    let bat_defaults: ThemeSet = bincode::deserialize_from(ZlibDecoder::new(THEME_SET_BIN))?;
    match name {
        None => Ok(bat_defaults),
        Some(name) if bat_defaults.themes.contains_key(name) => Ok(bat_defaults),
        Some(name) => {
            let defaults = ThemeSet::load_defaults();
            if defaults.themes.contains_key(name) {
                Ok(defaults)
            } else {
                let msg = format!("Unknown theme '{}'. See --list-themes output", name);
                Err(PrintError::new(msg).into())
            }
        }
    }
}

pub struct SyntectPrinter<'main, W>
where
    for<'a> W: LockableWrite<'a>,
{
    writer: W, // Protected with mutex because it should print file by file
    syntaxes: SyntaxSet,
    themes: ThemeSet,
    opts: PrinterOptions<'main>,
}

impl<'main> SyntectPrinter<'main, Stdout> {
    pub fn with_stdout(opts: PrinterOptions<'main>) -> Result<Self> {
        Self::new(io::stdout(), opts)
    }
}

impl<'main, W> SyntectPrinter<'main, W>
where
    for<'a> W: LockableWrite<'a>,
{
    pub fn new(out: W, opts: PrinterOptions<'main>) -> Result<Self> {
        Ok(Self::with_assets(
            out,
            bincode::deserialize_from(ZlibDecoder::new(SYNTAX_SET_BIN))?,
            load_themes(opts.theme)?,
            opts,
        ))
    }

    fn with_assets(
        writer: W,
        syntaxes: SyntaxSet,
        themes: ThemeSet,
        opts: PrinterOptions<'main>,
    ) -> Self {
        Self {
            writer,
            syntaxes,
            themes,
            opts,
        }
    }

    pub fn writer_mut(&mut self) -> &mut W {
        &mut self.writer
    }

    fn theme(&self) -> &Theme {
        let name = self.opts.theme.unwrap_or_else(|| {
            if self.opts.color_support == TermColorSupport::Ansi16 {
                "ansi"
            } else {
                "Monokai Extended" // Our 25bit -> 8bit color conversion works really well with this colorscheme
            }
        });
        &self.themes.themes[name]
    }

    fn find_syntax(&self, path: &Path) -> Result<&SyntaxReference> {
        let name = match path.extension().and_then(OsStr::to_str) {
            Some("fs") => Some("F#"),
            Some("h") => Some("C++"),
            Some("pac") => Some("JavaScript (Babel)"),
            _ => None,
        };
        if let Some(syntax) = name.and_then(|n| self.syntaxes.find_syntax_by_name(n)) {
            return Ok(syntax);
        }

        Ok(self
            .syntaxes
            .find_syntax_for_file(path)?
            .unwrap_or_else(|| self.syntaxes.find_syntax_plain_text()))
    }
}

impl<'main, W> Printer for SyntectPrinter<'main, W>
where
    for<'a> W: LockableWrite<'a>,
{
    fn print(&self, file: File) -> Result<()> {
        if file.chunks.is_empty() || file.line_matches.is_empty() {
            return Ok(());
        }

        let mut buf = vec![];
        let theme = self.theme();
        let syntax = self.find_syntax(&file.path)?;

        let mut drawer = Drawer::new(&mut buf, &self.opts, theme, &file.chunks);
        drawer.draw_header(&file.path)?;
        let hl = LineHighlighter::new(syntax, theme, &self.syntaxes);
        drawer.draw_body(&file, hl)?;
        drawer.draw_footer()?;

        // Take lock here to print files in serial from multiple threads
        let mut output = self.writer.lock();
        output.write_all(&buf)?;
        Ok(output.flush()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunk::{File, LineMatch};
    use lazy_static::lazy_static;
    use std::cell::{RefCell, RefMut};
    use std::fmt;
    use std::fs;
    use std::mem;
    use std::path::PathBuf;

    lazy_static! {
        static ref SYNTAX_SET: SyntaxSet =
            bincode::deserialize_from(ZlibDecoder::new(SYNTAX_SET_BIN)).unwrap();
        static ref THEME_SET: ThemeSet = load_themes(None).unwrap();
    }

    fn syntax_set() -> SyntaxSet {
        SYNTAX_SET.clone()
    }

    // ThemeSet does not implement Clone
    fn theme_set() -> ThemeSet {
        let mut ts = ThemeSet::new();
        ts.themes = THEME_SET.themes.clone();
        ts
    }

    struct DummyStdoutLock<'a>(RefMut<'a, Vec<u8>>);
    impl<'a> Write for DummyStdoutLock<'a> {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.0.write(buf)
        }
        fn flush(&mut self) -> io::Result<()> {
            self.0.flush()
        }
    }

    #[derive(Default)]
    struct DummyStdout(RefCell<Vec<u8>>);
    impl<'a> LockableWrite<'a> for DummyStdout {
        type Locked = DummyStdoutLock<'a>;
        fn lock(&'a self) -> Self::Locked {
            DummyStdoutLock(self.0.borrow_mut())
        }
    }

    mod ui {
        use super::*;
        use pretty_assertions::assert_eq;
        use std::cmp;
        use std::path::Path;

        fn read_chunk<'a>(
            iter: &mut impl Iterator<Item = (usize, &'a str)>,
        ) -> Option<(Vec<LineMatch>, (u64, u64))> {
            let mut lmats = vec![];
            let (mut s, mut e) = (u64::MAX, 0);
            for _ in 0..12 {
                if let Some((idx, line)) = iter.next() {
                    let lnum = (idx + 1) as u64;
                    s = cmp::min(s, lnum);
                    e = cmp::max(e, lnum);

                    let mut ranges = vec![];
                    let mut l = line;
                    let mut base = 0;
                    while let (Some(start), Some(i)) = (l.find("*match to "), l.find(" line*")) {
                        let end = i + " line*".len();
                        ranges.push((base + start, base + end));
                        l = &l[end..];
                        base += end;
                    }
                    if ranges.len() > 0 {
                        lmats.push(LineMatch {
                            line_number: lnum,
                            ranges,
                        });
                    }
                } else {
                    break;
                }
            }
            if s == u64::MAX || e == 0 || lmats.is_empty() {
                return None;
            }
            s = cmp::max(lmats[0].line_number.saturating_sub(6), s);
            e = cmp::min(lmats[lmats.len() - 1].line_number + 6, e);
            Some((lmats, (s, e)))
        }

        fn read_chunks(path: PathBuf) -> File {
            let contents = fs::read_to_string(&path).unwrap();
            let mut lmats = vec![];
            let mut chunks = vec![];
            let mut lines = contents.lines().enumerate();
            while let Some((ls, c)) = read_chunk(&mut lines) {
                lmats.extend(ls);
                chunks.push(c);
            }
            File::new(path, lmats, chunks, contents.into_bytes())
        }

        #[cfg(not(windows))]
        fn read_expected_file(expected_file: &Path) -> Vec<u8> {
            fs::read(expected_file).unwrap()
        }

        #[cfg(windows)]
        fn read_expected_file(expected_file: &Path) -> Vec<u8> {
            let mut contents = fs::read(expected_file).unwrap();

            // Replace '.\path\to\file' with './path/to/file'
            let mut slash: Vec<u8> = expected_file
                .to_str()
                .unwrap()
                .as_bytes()
                .iter()
                .copied()
                .map(|b| if b == b'\\' { b'/' } else { b })
                .collect();

            // replace foo.out with foo.rs
            slash.truncate(slash.len() - ".out".len());
            slash.extend_from_slice(b".rs");

            // Find index position of the slash path
            let base = match contents.windows(slash.len()).position(|s| s == slash) {
                Some(i) => i,
                None => panic!(
                    "File path {:?} (converted from {:?}) is not found in expected file contents:\n{}",
                    String::from_utf8_lossy(&slash).as_ref(),
                    &expected_file,
                    String::from_utf8_lossy(&contents).as_ref()
                ),
            };

            // Replace / with \
            for (i, byte) in slash.into_iter().enumerate() {
                if byte == b'/' {
                    contents[base + i] = b'\\';
                }
            }

            contents
        }

        fn run_uitest(file: File, expected_file: PathBuf, f: fn(&mut PrinterOptions<'_>) -> ()) {
            let stdout = DummyStdout(RefCell::new(vec![]));
            let mut opts = PrinterOptions::default();
            opts.term_width = 80;
            opts.color_support = TermColorSupport::True;
            f(&mut opts);
            let mut printer = SyntectPrinter::with_assets(stdout, syntax_set(), theme_set(), opts);
            printer.print(file).unwrap();
            let printed = mem::take(printer.writer_mut()).0.into_inner();
            let expected = read_expected_file(&expected_file);
            assert_eq!(
                printed,
                expected,
                "got:\n{}\nwant:\n{}",
                String::from_utf8_lossy(&printed),
                String::from_utf8_lossy(&expected),
            );
        }

        fn run_parametrized_uitest_single_chunk(
            mut input: &str,
            f: fn(&mut PrinterOptions<'_>) -> (),
        ) {
            let dir = Path::new(".").join("testdata").join("syntect");
            if input.starts_with("test_") {
                input = &input["test_".len()..];
            }
            let infile = dir.join(format!("{}.rs", input));
            let outfile = dir.join(format!("{}.out", input));
            let file = read_chunks(infile);
            run_uitest(file, outfile, f);
        }

        macro_rules! uitest {
            ($($input:ident($f:expr),)+) => {
                $(
                    #[test]
                    fn $input() {
                        run_parametrized_uitest_single_chunk(stringify!($input), $f);
                    }
                )+
            }
        }

        uitest!(
            test_default(|_| {}),
            test_background(|o| {
                o.background_color = true;
            }),
            test_no_grid(|o| {
                o.grid = false;
            }),
            test_theme(|o| {
                o.theme = Some("Nord");
            }),
            test_tab_width_2(|o| {
                o.tab_width = 2;
            }),
            test_hard_tab(|o| {
                o.tab_width = 0;
            }),
            test_ansi256_colors(|o| {
                o.color_support = TermColorSupport::Ansi256;
            }),
            test_ansi16_colors(|o| {
                o.color_support = TermColorSupport::Ansi16;
            }),
            test_long_line(|_| {}),
            test_long_line_bg(|o| {
                o.background_color = true;
            }),
            test_empty_lines(|_| {}),
            test_empty_lines_bg(|o| {
                o.background_color = true;
            }),
            test_wrap_between_text(|_| {}),
            test_wrap_middle_of_text(|_| {}),
            test_wrap_middle_of_spaces(|_| {}),
            test_wrap_middle_of_tab(|_| {}),
            test_wrap_twice(|_| {}),
            test_wrap_no_grid(|o| {
                o.grid = false;
            }),
            test_wrap_theme(|o| {
                o.theme = Some("Nord");
            }),
            test_wrap_ansi256(|o| {
                o.color_support = TermColorSupport::Ansi256;
            }),
            test_wrap_middle_text_bg(|o| {
                o.background_color = true;
            }),
            test_wrap_between_bg(|o| {
                o.background_color = true;
            }),
            test_no_wrap_default(|o| {
                o.text_wrap = TextWrapMode::Never;
            }),
            test_no_wrap_no_grid(|o| {
                o.text_wrap = TextWrapMode::Never;
                o.grid = false;
            }),
            test_no_wrap_background(|o| {
                o.text_wrap = TextWrapMode::Never;
                o.background_color = true;
            }),
            test_multi_line_numbers(|_| {}),
            test_multi_chunks_default(|_| {}),
            test_multi_chunks_no_grid(|o| {
                o.grid = false;
            }),
            test_multi_chunks_bg(|o| {
                o.background_color = true;
            }),
            test_japanese_default(|_| {}),
            test_japanese_background(|o| {
                o.background_color = true;
            }),
            test_wrap_japanese_after(|_| {}),
            test_wrap_japanese_before(|_| {}),
            test_wrap_break_wide_char(|_| {}),
            test_wrap_break_wide_char_bg(|o| {
                o.background_color = true;
            }),
            test_wrap_japanese_louise(|_| {}),
            test_wrap_jp_louise_bg(|o| {
                o.background_color = true;
            }),
            test_wrap_jp_louise_no_grid(|o| {
                o.grid = false;
            }),
            test_wrap_emoji(|_| {}),
            test_wrap_emoji_zwj(|_| {}),
            test_emoji(|_| {}),
            test_emoji_bg(|o| {
                o.background_color = true;
            }),
            test_no_grid_background(|o| {
                o.grid = false;
                o.background_color = true;
            }),
            test_wide_char_region(|_| {}),
            test_wide_char_region_bg(|o| {
                o.background_color = true;
            }),
            test_wrap_match_at_second_line(|_| {}),
            test_wrap_region_accross_line(|_| {}),
            test_wrap_region_jp_accross_line(|_| {}),
            test_wrap_match_at_second_line_bg(|o| {
                o.background_color = true;
            }),
            test_region_at_end_of_line(|_| {}),
            test_region_at_end_of_line_bg(|o| {
                o.background_color = true;
            }),
            test_region_at_line_start(|_| {}),
            test_wrap_region_line_start(|_| {}),
            test_wrap_region_line_end(|_| {}),
            test_wrap_3_lines_emoji(|_| {}),
            test_first_only(|o| {
                o.first_only = true;
            }),
            test_ascii_lines_grid(|o| {
                o.ascii_lines = true;
            }),
            test_ascii_lines_no_grid(|o| {
                o.ascii_lines = true;
                o.grid = false;
            }),
            test_multi_regions(|_| {}),
            test_multi_regions_bg(|o| {
                o.background_color = true;
            }),
            test_wrap_between_regions(|_| {}),
            test_wrap_regions_japanese(|_| {}),
        );
    }

    #[derive(Debug)]
    struct DummyError;
    impl std::error::Error for DummyError {}
    impl fmt::Display for DummyError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "dummy error!")
        }
    }

    struct ErrorStdoutLock;
    impl Write for ErrorStdoutLock {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::Error::new(io::ErrorKind::Other, DummyError))
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[derive(Default)]
    struct ErrorStdout;
    impl<'a> LockableWrite<'a> for ErrorStdout {
        type Locked = ErrorStdoutLock;
        fn lock(&'a self) -> Self::Locked {
            ErrorStdoutLock
        }
    }

    fn sample_chunk(file: &str) -> File {
        let readme = PathBuf::from(file);
        let lmats = vec![LineMatch::lnum(3)];
        let chunks = vec![(1, 6)];
        let contents = fs::read(&readme).unwrap();
        File::new(readme, lmats, chunks, contents)
    }

    #[test]
    fn test_error_write() {
        let file = sample_chunk("README.md");
        let opts = PrinterOptions::default();
        let printer = SyntectPrinter::with_assets(ErrorStdout, syntax_set(), theme_set(), opts);
        let err = printer.print(file).unwrap_err();
        assert_eq!(&format!("{}", err), "dummy error!", "message={}", err);
    }

    #[test]
    fn test_unknown_theme() {
        let mut opts = PrinterOptions::default();
        opts.theme = Some("this theme does not exist");
        let err = match SyntectPrinter::with_stdout(opts) {
            Err(e) => e,
            Ok(_) => panic!("error did not occur"),
        };
        let msg = format!("{}", err);
        assert!(msg.contains("Unknown theme"), "message={:?}", msg);
    }

    #[test]
    fn test_list_themes() {
        let mut buf = vec![];
        list_themes(&mut buf).unwrap();
        let out = String::from_utf8(buf).unwrap();

        // From bat's assets
        assert!(out.contains("Monokai Extended\n"), "output={:?}", out);

        // From default assets
        assert!(out.contains("base16-ocean.dark\n"), "output={:?}", out);
    }

    #[test]
    fn test_print_nothing() {
        let file = File::new(PathBuf::from("x.txt"), vec![], vec![], vec![]);
        let opts = PrinterOptions::default();
        let stdout = DummyStdout(RefCell::new(vec![]));
        let mut printer = SyntectPrinter::with_assets(stdout, syntax_set(), theme_set(), opts);
        printer.print(file).unwrap();
        let printed = mem::take(printer.writer_mut()).0.into_inner();
        assert!(
            printed.is_empty(),
            "pritned:\n{}",
            String::from_utf8_lossy(&printed)
        );
    }

    #[test]
    fn test_no_syntax_found() {
        let file = sample_chunk("LICENSE.txt");
        let opts = PrinterOptions::default();
        let stdout = DummyStdout(RefCell::new(vec![]));
        let mut printer = SyntectPrinter::with_assets(stdout, syntax_set(), theme_set(), opts);
        printer.print(file).unwrap();
        let printed = mem::take(printer.writer_mut()).0.into_inner();
        assert!(!printed.is_empty());
    }

    #[test]
    fn test_adjacent_regions() {
        let contents = b"this is test\n";
        let ranges = (0..contents.len()).map(|i| (i, i + 1)).collect();
        let lmats = vec![LineMatch {
            line_number: 1,
            ranges,
        }];
        let chunks = vec![(1, 1)];
        let file = File::new(PathBuf::from("test.txt"), lmats, chunks, contents.to_vec());

        let mut opts = PrinterOptions::default();
        opts.color_support = TermColorSupport::True;
        let stdout = DummyStdout(RefCell::new(vec![]));
        let mut printer = SyntectPrinter::with_assets(stdout, syntax_set(), theme_set(), opts);
        printer.print(file).unwrap();

        let printed = mem::take(printer.writer_mut()).0.into_inner();
        let mut lines = printed.split_inclusive(|b| *b == b'\n');

        // One region per one character, but color codes between adjacent regions are not inserted
        let expected = b"\x1b[38;2;0;0;0m\x1b[48;2;255;231;146mthis is test";
        let this_is_test_line = lines.nth(3).unwrap();
        let found = this_is_test_line
            .windows(expected.len())
            .find(|s| s == expected)
            .is_some();
        assert!(
            found,
            "line={:?}",
            std::str::from_utf8(this_is_test_line).unwrap()
        );
    }
}
