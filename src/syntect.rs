use crate::chunk::{File, Line};
use crate::printer::{Printer, PrinterOptions, TermColorSupport, TextWrapMode};
use ansi_colours::ansi256_from_rgb;
use anyhow::Result;
use flate2::read::ZlibDecoder;
use memchr::{memchr_iter, Memchr};
use std::cmp;
use std::ffi::OsStr;
use std::fmt;
use std::io::{self, Stdout, StdoutLock, Write};
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

fn load_bat_themes() -> Result<ThemeSet> {
    Ok(bincode::deserialize_from(ZlibDecoder::new(THEME_SET_BIN))?)
}

fn load_syntax_set() -> Result<SyntaxSet> {
    Ok(bincode::deserialize_from(ZlibDecoder::new(SYNTAX_SET_BIN))?)
}

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

pub fn list_themes<W: Write>(out: W, opts: &PrinterOptions<'_>) -> Result<()> {
    let syntaxes = load_syntax_set()?;
    list_themes_with_syntaxes(out, opts, &syntaxes)
}

fn list_themes_with_syntaxes<W: Write>(
    mut out: W,
    opts: &PrinterOptions<'_>,
    syntaxes: &SyntaxSet,
) -> Result<()> {
    use crate::io::IgnoreBrokenPipe;

    let themes = {
        let mut m = load_bat_themes()?.themes;
        m.extend(ThemeSet::load_defaults().themes.into_iter());
        let mut v: Vec<_> = m.into_iter().collect();
        v.sort_by(|l, r| l.0.cmp(&r.0));
        v
    };

    let syntax = syntaxes.find_syntax_by_name("Rust").unwrap();
    let sample_file = File::sample_file();

    themes
        .iter()
        .try_for_each(|(name, theme)| {
            let mut drawer = Drawer::new(&mut out, opts, theme, &sample_file.chunks);
            drawer.canvas.set_bold()?;
            write!(drawer.canvas, "{:?}", name)?;
            drawer.canvas.draw_newline()?;
            drawer.canvas.draw_sample()?;
            writeln!(drawer.canvas)?;

            let hl = LineHighlighter::new(syntax, theme, syntaxes);
            drawer.draw_file(&sample_file, hl)?;
            writeln!(drawer.canvas)
        })
        .ignore_broken_pipe()?;

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

#[derive(Debug)]
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
    NotFound,
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

    fn region_boundary(&mut self) -> RegionBoundary {
        let o = self.byte_offset;

        // Eat done regions
        let num_done_regions = self.regions.iter().take_while(|(_, e)| *e < o).count();
        if num_done_regions > 0 {
            self.regions = &self.regions[num_done_regions..];
        }

        match self.regions.first().copied() {
            Some((s, e)) if o == s && o < e => RegionBoundary::Start,
            Some((_, e)) if o == e => {
                // When the next region is adjcent, skip changing highlight
                match self.regions.get(1) {
                    Some((s, _)) if o == *s => RegionBoundary::NotFound,
                    _ => RegionBoundary::End,
                }
            }
            _ => RegionBoundary::NotFound,
        }
    }

    fn next_event(&mut self) -> DrawEvent {
        match self.region_boundary() {
            RegionBoundary::Start if !self.in_region => {
                self.in_region = true;
                return DrawEvent::RegionStart;
            }
            RegionBoundary::End if self.in_region => {
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

#[inline]
#[allow(clippy::many_single_char_names)]
fn blend_fg_color(fg: Color, bg: Color) -> Color {
    if fg.a == 0xff || fg.a == 0 || fg.a == 1 {
        return fg; // 0 and 1 are special cases for 16 colors and 256 colors themes
    }
    let x = fg.a as u32;
    let y = (255 - fg.a) as u32;
    let r = (fg.r as u32 * x + bg.r as u32 * y) / 255;
    let g = (fg.g as u32 * x + bg.g as u32 * y) / 255;
    let b = (fg.b as u32 * x + bg.b as u32 * y) / 255;
    Color {
        r: r as u8,
        g: g as u8,
        b: b as u8,
        a: 255,
    }
}

#[inline]
fn color_average(c: Color) -> u8 {
    ((c.r as u32 + c.g as u32 + c.b as u32) / 3) as u8
}

#[inline]
fn weak_blend_fg_color(mut fg: Color, bg: Color) -> Color {
    let fg_avg = color_average(fg);
    let bg_avg = color_average(bg);
    let heuristic_ratio = if fg_avg > bg_avg {
        if fg_avg - bg_avg >= 200 {
            4 // Vivid dark theme uses further weaker gutter foreground
        } else {
            3 // Dark theme uses weaker gutter foreground than light theme
        }
    } else {
        2
    };
    fg.a /= heuristic_ratio;
    blend_fg_color(fg, bg)
}

#[inline]
fn diff_u8(x: u8, y: u8) -> u8 {
    if x > y {
        x - y
    } else {
        y - x
    }
}

#[derive(Debug)]
struct Palette {
    foreground: Color,
    background: Color,
    match_bg: Color,
    match_lnum_fg: Color,
    region_fg: Color,
    region_bg: Color,
    gutter_fg: Color,
}

impl Palette {
    const NO_COLOR: Color = Color {
        r: 0,
        g: 0,
        b: 0,
        a: 1, // Special color which means pass though
    };
    const YELLOW_COLOR_16: Color = Color {
        r: 3, // Yellow
        g: 0,
        b: 0,
        a: 0,
    };
    const BLACK_COLOR_16: Color = Color {
        r: 0, // Black
        g: 0,
        b: 0,
        a: 0,
    };
    const ANSI16: Palette = Palette {
        foreground: Self::NO_COLOR,
        background: Self::NO_COLOR,
        match_bg: Self::NO_COLOR,
        match_lnum_fg: Self::YELLOW_COLOR_16,
        region_fg: Self::BLACK_COLOR_16,
        region_bg: Self::YELLOW_COLOR_16,
        gutter_fg: Self::NO_COLOR,
    };

    fn new(theme: &Theme) -> Self {
        let background = theme.settings.background.unwrap_or(Self::NO_COLOR);
        let foreground = theme.settings.foreground.unwrap_or(Self::NO_COLOR);
        let foreground = blend_fg_color(foreground, background);

        if foreground.a == 1 && background.a == 1 {
            return Self::ANSI16;
        }

        // gutter and gutter_foreground are not fit to show line numbers and borders in some color themes
        let gutter_fg = weak_blend_fg_color(foreground, background);
        let match_lnum_fg = blend_fg_color(foreground, background);

        let match_bg = theme
            .settings
            .line_highlight
            .unwrap_or_else(|| weak_blend_fg_color(foreground, background));

        let (region_fg, region_bg) = if let Some(bg) = theme.settings.find_highlight {
            let fg = theme.settings.find_highlight_foreground.unwrap_or_else(|| {
                let avg = color_average(bg);
                let avg_fg = color_average(foreground);
                let avg_bg = color_average(background);
                // Choose foreground or background looking at distance
                if diff_u8(avg_fg, avg) > diff_u8(avg_bg, avg) {
                    foreground
                } else {
                    background
                }
            });
            let fg = blend_fg_color(fg, bg);
            (fg, bg)
        } else {
            (background, foreground)
        };

        Self {
            foreground,
            background,
            match_bg,
            match_lnum_fg,
            region_fg,
            region_bg,
            gutter_fg,
        }
    }

    fn is_ansi16(&self) -> bool {
        self.foreground.a == 1 && self.foreground.r <= 7
    }
}

struct Canvas<W: Write> {
    out: W,
    true_color: bool,
    has_background: bool,
    palette: Palette,
    current_fg: Option<Color>,
    current_bg: Option<Color>,
}

impl<W: Write> Deref for Canvas<W> {
    type Target = W;
    fn deref(&self) -> &Self::Target {
        &self.out
    }
}
impl<W: Write> DerefMut for Canvas<W> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.out
    }
}

impl<W: Write> Canvas<W> {
    fn new(out: W, opts: &PrinterOptions<'_>, theme: &Theme) -> Self {
        let palette = if opts.color_support == TermColorSupport::Ansi16 {
            Palette::ANSI16
        } else {
            Palette::new(theme)
        };

        Self {
            out,
            true_color: opts.color_support == TermColorSupport::True,
            has_background: !palette.is_ansi16() && opts.background_color,
            palette,
            current_fg: None,
            current_bg: None,
        }
    }

    fn draw_spaces(&mut self, num: usize) -> io::Result<()> {
        for _ in 0..num {
            self.out.write_all(b" ")?;
        }
        Ok(())
    }

    fn draw_newline(&mut self) -> io::Result<()> {
        writeln!(self.out, "\x1b[0m")?; // Reset on newline to ensure to reset color
        self.current_fg = None;
        self.current_bg = None;
        Ok(())
    }

    fn set_color(&mut self, code: u8, c: Color) -> io::Result<()> {
        // In case of c.a == 0 and c.a == 1 are handling for special colorscheme by bat for non true
        // color terminals. Color value is encoded in R. See `to_ansi_color()` in bat/src/terminal.rs
        match c.a {
            0 if c.r <= 7 => write!(self.out, "\x1b[{}m", c.r + code)?, // 16 colors; e.g. 3 => 33 (Yellow), 6 => 36 (Cyan) (code=30)
            0 => write!(self.out, "\x1b[{};5;{}m", code + 8, c.r)?, // 256 colors; code=38 for fg, code=48 for bg
            1 => write!(self.out, "\x1b[0m")?, // Pass though. Reset color to set default terminal font color
            _ if self.true_color => {
                write!(self.out, "\x1b[{};2;{};{};{}m", code + 8, c.r, c.g, c.b)?;
            }
            _ => {
                let c = ansi256_from_rgb((c.r, c.g, c.b));
                write!(self.out, "\x1b[{};5;{}m", code + 8, c)?;
            }
        }
        Ok(())
    }

    fn set_bg(&mut self, c: Color) -> io::Result<()> {
        if self.current_bg != Some(c) {
            self.set_color(40, c)?;
            self.current_bg = Some(c);
        }
        Ok(())
    }

    fn set_fg(&mut self, c: Color) -> io::Result<()> {
        if self.current_fg != Some(c) {
            self.set_color(30, c)?;
            self.current_fg = Some(c);
        }
        Ok(())
    }

    fn set_default_bg(&mut self) -> io::Result<()> {
        if self.has_background {
            self.set_bg(self.palette.background)?;
        }
        Ok(())
    }

    fn set_default_fg(&mut self) -> io::Result<()> {
        self.set_fg(self.palette.foreground)
    }

    fn set_background(&mut self, c: Color) -> io::Result<()> {
        if self.has_background {
            self.set_bg(c)?;
        }
        Ok(())
    }

    fn set_bold(&mut self) -> io::Result<()> {
        self.out.write_all(b"\x1b[1m")?;
        Ok(())
    }

    fn set_underline(&mut self) -> io::Result<()> {
        self.out.write_all(b"\x1b[4m")?;
        Ok(())
    }

    fn unset_bold(&mut self) -> io::Result<()> {
        self.out.write_all(b"\x1b[22m")?;
        Ok(())
    }

    fn unset_underline(&mut self) -> io::Result<()> {
        self.out.write_all(b"\x1b[24m")?;
        Ok(())
    }

    fn set_font_style(&mut self, style: FontStyle) -> io::Result<()> {
        if style.contains(FontStyle::BOLD) {
            self.set_bold()?;
        }
        if style.contains(FontStyle::UNDERLINE) {
            self.set_underline()?;
        }
        Ok(())
    }

    fn unset_font_style(&mut self, style: FontStyle) -> io::Result<()> {
        if style.contains(FontStyle::BOLD) {
            self.unset_bold()?;
        }
        if style.contains(FontStyle::UNDERLINE) {
            self.unset_underline()?;
        }
        Ok(())
    }

    fn set_style(&mut self, style: Style) -> io::Result<()> {
        self.set_background(style.background)?;
        self.set_fg(style.foreground)?;
        self.set_font_style(style.font_style)?;
        Ok(())
    }

    fn set_match_bg_color(&mut self) -> io::Result<()> {
        self.set_bg(self.palette.match_bg)
    }

    fn set_match_style(&mut self, style: Style) -> io::Result<()> {
        self.set_match_bg_color()?;
        self.set_fg(style.foreground)?;
        self.set_font_style(style.font_style)
    }

    fn set_region_color(&mut self) -> io::Result<()> {
        self.set_fg(self.palette.region_fg)?;
        self.set_bg(self.palette.region_bg)
    }

    fn set_gutter_color(&mut self) -> io::Result<()> {
        self.set_fg(self.palette.gutter_fg)?;
        self.set_default_bg()
    }

    fn set_match_lnum_color(&mut self) -> io::Result<()> {
        self.set_fg(self.palette.match_lnum_fg)?;
        self.set_default_bg()
    }

    fn fill_spaces(&mut self, written_width: usize, max_width: usize) -> io::Result<()> {
        if written_width < max_width {
            self.draw_spaces(max_width - written_width)?;
        }
        Ok(())
    }

    fn draw_sample_row(&mut self, colors: &[(&str, Color)]) -> io::Result<()> {
        for (name, color) in colors {
            write!(self.out, "    {} ", name)?;
            self.set_bg(*color)?;
            self.out.write_all(b"    \x1b[0m")?;
        }
        writeln!(self.out)?;
        self.current_fg = None;
        self.current_bg = None;
        Ok(())
    }

    #[rustfmt::skip]
    fn draw_sample(&mut self) -> io::Result<()> {
        self.draw_sample_row(&[("Foreground:   ", self.palette.foreground), ("Background:   ", self.palette.background)])?;
        self.draw_sample_row(&[("MatchLineBG:  ", self.palette.match_bg),   ("MatchLineNum: ", self.palette.match_lnum_fg)])?;
        self.draw_sample_row(&[("MatchRegionFG:", self.palette.region_fg),  ("MatchRegionBG:", self.palette.region_bg)])?;
        self.draw_sample_row(&[("GutterFG:     ", self.palette.gutter_fg)])
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
            .map(|(mut style, text)| {
                style.foreground = blend_fg_color(style.foreground, style.background);
                Token { style, text }
            })
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
    grid: bool,
    term_width: u16,
    lnum_width: u16,
    first_only: bool,
    wrap: bool,
    tab_width: u16,
    chars: LineChars<'file>,
    canvas: Canvas<W>,
}

impl<'file, W: Write> Drawer<'file, W> {
    fn new(out: W, opts: &PrinterOptions<'_>, theme: &'file Theme, chunks: &[(u64, u64)]) -> Self {
        let last_lnum = chunks.last().map(|(_, e)| *e).unwrap_or(0);
        let mut lnum_width = num_digits(last_lnum);
        if chunks.len() > 1 {
            lnum_width = cmp::max(lnum_width, 3); // Consider '...' in gutter
        }

        let chars = if opts.ascii_lines {
            ASCII_LINE_CHARS
        } else {
            UNICODE_LINE_CHARS
        };

        Drawer {
            grid: opts.grid,
            term_width: opts.term_width,
            lnum_width,
            wrap: opts.text_wrap == TextWrapMode::Char,
            tab_width: opts.tab_width as u16,
            first_only: opts.first_only,
            chars,
            canvas: Canvas::new(out, opts, theme),
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

    fn draw_horizontal_line(&mut self, sep: &str) -> io::Result<()> {
        self.canvas.set_gutter_color()?;
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

    fn draw_line_number(&mut self, lnum: u64, matched: bool) -> io::Result<()> {
        if matched {
            self.canvas.set_match_lnum_color()?;
        } else {
            self.canvas.set_gutter_color()?;
        }
        let width = num_digits(lnum);
        self.canvas
            .draw_spaces((self.lnum_width - width) as usize)?;
        write!(self.canvas, " {}", lnum)?;
        if self.grid {
            if matched {
                self.canvas.set_gutter_color()?;
            }
            write!(self.canvas, " {}", self.chars.vertical)?;
        }
        self.canvas.set_default_bg()?;
        self.canvas.write_all(b" ")?;
        Ok(()) // Do not reset color because another color text will follow
    }

    fn draw_wrapping_gutter(&mut self) -> io::Result<()> {
        self.canvas.set_gutter_color()?;
        self.canvas.draw_spaces(self.lnum_width as usize + 2)?;
        if self.grid {
            write!(self.canvas, "{} ", self.chars.vertical)?;
        }
        Ok(())
    }

    fn draw_separator_line(&mut self) -> io::Result<()> {
        self.canvas.set_gutter_color()?;
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

    fn draw_text_wrappping(
        &mut self,
        matched: bool,
        style: Style,
        in_region: bool,
    ) -> io::Result<()> {
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
    ) -> io::Result<()> {
        // The highlighter requires newline at the end. But we don't want it since
        // - we sometimes need to fill the rest of line with spaces
        // - we clear colors before writing newline
        if let Some(tok) = tokens.last_mut() {
            tok.chomp();
            if tok.text.is_empty() {
                tokens.pop(); // As the result of `chomp()`, text may be empty. Empty token can be removed
            }
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

    fn draw_body(&mut self, file: &File, mut hl: LineHighlighter<'_>) -> io::Result<()> {
        assert!(!file.chunks.is_empty());

        let mut matched = file.line_matches.as_ref();
        let mut chunks = file.chunks.iter();
        let mut chunk = chunks.next().unwrap(); // OK since chunks is not empty

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

    fn draw_header(&mut self, path: &Path) -> io::Result<()> {
        self.draw_horizontal_line(self.chars.horizontal)?;
        self.canvas.set_default_bg()?;
        let path = path.as_os_str().to_string_lossy();
        self.canvas.set_default_fg()?;
        self.canvas.set_bold()?;
        write!(self.canvas, " {}", path)?;
        if self.canvas.has_background {
            self.canvas
                .fill_spaces(path.width_cjk() + 1, self.term_width as usize)?;
        }
        self.canvas.draw_newline()?;
        if self.grid {
            self.draw_horizontal_line(self.chars.down_and_horizontal)?;
        }
        Ok(())
    }

    fn draw_footer(&mut self) -> io::Result<()> {
        if self.grid {
            self.draw_horizontal_line(self.chars.up_and_horizontal)?;
        }
        Ok(())
    }

    fn draw_file(&mut self, file: &File, hl: LineHighlighter) -> io::Result<()> {
        self.draw_header(&file.path)?;
        self.draw_body(file, hl)?;
        self.draw_footer()
    }
}

fn load_themes(name: Option<&str>) -> Result<ThemeSet> {
    let bat_defaults: ThemeSet = load_bat_themes()?;
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

pub struct SyntectAssets {
    pub syntax_set: SyntaxSet,
    pub theme_set: ThemeSet,
}

impl SyntectAssets {
    pub fn load(theme: Option<&str>) -> Result<Self> {
        Ok(Self {
            syntax_set: load_syntax_set()?,
            theme_set: load_themes(theme)?,
        })
    }
}

impl Clone for SyntectAssets {
    fn clone(&self) -> Self {
        let syntax_set = self.syntax_set.clone();
        let mut theme_set = ThemeSet::new(); // ThemeSet does not implement Clone
        theme_set.themes = self.theme_set.themes.clone();
        Self {
            syntax_set,
            theme_set,
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
    pub fn new(writer: W, opts: PrinterOptions<'main>) -> Result<Self> {
        Ok(Self {
            writer,
            syntaxes: load_syntax_set()?,
            themes: load_themes(opts.theme)?,
            opts,
        })
    }

    pub fn with_assets(assets: SyntectAssets, writer: W, opts: PrinterOptions<'main>) -> Self {
        Self {
            writer,
            syntaxes: assets.syntax_set,
            themes: assets.theme_set,
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
        let name = name.or_else(|| match path.file_name().and_then(OsStr::to_str) {
            Some(".clang-format") => Some("YAML"),
            _ => None,
        });
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
        use crate::io::IgnoreBrokenPipe;

        if file.chunks.is_empty() || file.line_matches.is_empty() {
            return Ok(());
        }

        let mut buf = vec![];
        let theme = self.theme();
        let syntax = self.find_syntax(&file.path)?;

        let hl = LineHighlighter::new(syntax, theme, &self.syntaxes);
        Drawer::new(&mut buf, &self.opts, theme, &file.chunks).draw_file(&file, hl)?;

        // Take lock here to print files in serial from multiple threads
        let mut output = self.writer.lock();
        output.write_all(&buf).ignore_broken_pipe()?;
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
    use std::str;

    lazy_static! {
        static ref ASSETS: SyntectAssets = SyntectAssets::load(None).unwrap();
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
                    if !ranges.is_empty() {
                        lmats.push(LineMatch::new(lnum, ranges));
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
            let mut opts = PrinterOptions {
                term_width: 80,
                color_support: TermColorSupport::True,
                ..Default::default()
            };
            f(&mut opts);
            let mut printer = SyntectPrinter::with_assets(ASSETS.clone(), stdout, opts);
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

        macro_rules! uitests {
            ($($input:ident($f:expr),)+) => {
                $(
                    #[test]
                    fn $input() {
                        run_parametrized_uitest_single_chunk(stringify!($input), $f);
                    }
                )+
            }
        }

        uitests!(
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
            test_wrap_accross_regions(|_| {}),
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

    // Separate module from `ui` since pretty_assertions is too slow for showing diff between byte sequences.
    mod list_themes {
        use super::*;

        fn run_parametrized_uitest_list_themes(
            mut input: &str,
            f: fn(&mut PrinterOptions<'_>) -> (),
        ) {
            if input.starts_with("test_") {
                input = &input["test_".len()..];
            }
            let file = format!("list_themes_{}.out", input);
            let expected = Path::new("testdata").join("syntect").join(file);
            let expected = fs::read(&expected).unwrap();

            let mut opts = PrinterOptions {
                term_width: 80,
                color_support: TermColorSupport::True,
                ..Default::default()
            };
            f(&mut opts);

            let mut got = vec![];
            list_themes_with_syntaxes(&mut got, &opts, &ASSETS.syntax_set).unwrap();

            assert_eq!(
                expected,
                got,
                "expected:\n{}\ngot:\n{}",
                str::from_utf8(&expected).unwrap(),
                str::from_utf8(&got).unwrap()
            );
        }

        macro_rules! list_theme_uitests {
            ($($input:ident($f:expr),)+) => {
                $(
                    #[test]
                    fn $input() {
                        run_parametrized_uitest_list_themes(stringify!($input), $f);
                    }
                )+
            }
        }

        list_theme_uitests! {
            test_default(|_| {}),
            test_no_grid(|o| {
                o.grid = false;
            }),
            test_background(|o| {
                o.background_color = true;
            }),
        }
    }

    #[derive(Debug)]
    struct DummyError;
    impl std::error::Error for DummyError {}
    impl fmt::Display for DummyError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "dummy error!")
        }
    }

    struct ErrorStdoutLock(io::ErrorKind);
    impl Write for ErrorStdoutLock {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::Error::new(self.0, DummyError))
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    struct ErrorStdout(io::ErrorKind);
    impl<'a> LockableWrite<'a> for ErrorStdout {
        type Locked = ErrorStdoutLock;
        fn lock(&'a self) -> Self::Locked {
            ErrorStdoutLock(self.0)
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
    fn test_write_error() {
        let file = sample_chunk("README.md");
        let opts = PrinterOptions::default();
        let printer =
            SyntectPrinter::with_assets(ASSETS.clone(), ErrorStdout(io::ErrorKind::Other), opts);
        let err = printer.print(file).unwrap_err();
        assert_eq!(&format!("{}", err), "dummy error!", "message={}", err);
    }

    #[test]
    fn test_no_error_at_broken_pipe() {
        let file = sample_chunk("README.md");
        let opts = PrinterOptions::default();
        let printer = SyntectPrinter::with_assets(
            ASSETS.clone(),
            ErrorStdout(io::ErrorKind::BrokenPipe),
            opts,
        );
        printer.print(file).unwrap();
    }

    #[test]
    fn test_unknown_theme() {
        let opts = PrinterOptions {
            theme: Some("this theme does not exist"),
            ..Default::default()
        };
        let err = match SyntectPrinter::with_stdout(opts) {
            Err(e) => e,
            Ok(_) => panic!("error did not occur"),
        };
        let msg = format!("{}", err);
        assert!(msg.contains("Unknown theme"), "message={:?}", msg);
    }

    #[test]
    fn test_print_nothing() {
        let file = File::new(PathBuf::from("x.txt"), vec![], vec![], vec![]);
        let opts = PrinterOptions::default();
        let stdout = DummyStdout(RefCell::new(vec![]));
        let mut printer = SyntectPrinter::with_assets(ASSETS.clone(), stdout, opts);
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
        let mut printer = SyntectPrinter::with_assets(ASSETS.clone(), stdout, opts);
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

        let opts = PrinterOptions {
            color_support: TermColorSupport::True,
            ..Default::default()
        };
        let stdout = DummyStdout(RefCell::new(vec![]));
        let mut printer = SyntectPrinter::with_assets(ASSETS.clone(), stdout, opts);
        printer.print(file).unwrap();

        let printed = mem::take(printer.writer_mut()).0.into_inner();
        let mut lines = printed.split_inclusive(|b| *b == b'\n');

        // One region per one character, but color codes between adjacent regions are not inserted
        let expected = b"\x1b[38;2;0;0;0m\x1b[48;2;255;231;146mthis is test";
        let this_is_test_line = lines.nth(3).unwrap();
        let found = this_is_test_line
            .windows(expected.len())
            .any(|s| s == expected);
        assert!(
            found,
            "line={:?}",
            str::from_utf8(this_is_test_line).unwrap()
        );
    }

    #[test]
    fn test_wrote_error_on_list_themes() {
        let opts = PrinterOptions::default();
        let err = list_themes_with_syntaxes(
            ErrorStdoutLock(io::ErrorKind::Other),
            &opts,
            &ASSETS.syntax_set,
        )
        .unwrap_err();
        assert_eq!(&format!("{}", err), "dummy error!", "message={}", err);
    }

    #[test]
    fn test_no_error_at_broken_pip_on_list_themes() {
        let opts = PrinterOptions::default();
        list_themes_with_syntaxes(
            ErrorStdoutLock(io::ErrorKind::BrokenPipe),
            &opts,
            &ASSETS.syntax_set,
        )
        .unwrap();
    }
}
