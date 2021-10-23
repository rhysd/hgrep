use crate::chunk::File;
use crate::chunk::Line;
use crate::printer::{Printer, PrinterOptions, TermColorSupport};
use anyhow::Result;
use memchr::{memchr_iter, Memchr};
use rgb2ansi256::rgb_to_ansi256;
use std::cmp;
use std::collections::HashSet;
use std::ffi::OsStr;
use std::fmt;
use std::io::{self, Stdout, StdoutLock};
use std::io::{BufWriter, Write};
use std::path::Path;
use syntect::highlighting::{
    Color, FontStyle, HighlightIterator, HighlightState, Highlighter, Style, Theme, ThemeSet,
};
use syntect::parsing::{ParseState, ScopeStack, SyntaxReference, SyntaxSet};
use unicode_width::UnicodeWidthStr;

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
    let bat_defaults = bincode::deserialize_from(flate2::read::ZlibDecoder::new(THEME_SET_BIN))?;
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

// Note: More flexible version of syntect::easy::HighlightLines for our use case
struct LineHighlighter<'a> {
    hl: Highlighter<'a>,
    parse_state: ParseState,
    hl_state: HighlightState,
}

impl<'a> LineHighlighter<'a> {
    fn new(syntax: &SyntaxReference, theme: &'a Theme) -> Self {
        let hl = Highlighter::new(theme);
        let parse_state = ParseState::new(syntax);
        let hl_state = HighlightState::new(&hl, ScopeStack::new());
        Self {
            hl,
            parse_state,
            hl_state,
        }
    }

    fn skip_line(&mut self, line: &str, syntax_set: &SyntaxSet) {
        let ops = self.parse_state.parse_line(line, syntax_set);
        for _ in HighlightIterator::new(&mut self.hl_state, &ops, line, &self.hl) {}
    }

    fn highlight<'line>(
        &mut self,
        line: &'line str,
        syntax_set: &SyntaxSet,
    ) -> Vec<(Style, &'line str)> {
        let ops = self.parse_state.parse_line(line, syntax_set);
        HighlightIterator::new(&mut self.hl_state, &ops, line, &self.hl).collect()
    }

    fn highlight_owned(&mut self, line: &str, syntax_set: &SyntaxSet) -> Vec<(Style, String)> {
        let ops = self.parse_state.parse_line(line, syntax_set);
        HighlightIterator::new(&mut self.hl_state, &ops, line, &self.hl)
            .map(|(n, s)| (n, s.to_string()))
            .collect()
    }
}

enum HighlightedLine<'file> {
    Lossless {
        lnum: u64,
        matched: bool,
        parts: Vec<(Style, &'file str)>,
    },
    Loss {
        lnum: u64,
        matched: bool,
        parts: Vec<(Style, String)>,
    },
    Separator,
}

impl<'file> HighlightedLine<'file> {
    fn loss_less(lnum: u64, matched: bool, mut parts: Vec<(Style, &'file str)>) -> Self {
        // Remove newline at the end
        if let Some((_, last)) = parts.last_mut() {
            if last.ends_with('\n') {
                *last = &last[..last.len() - 1];
            }
            if last.ends_with('\r') {
                *last = &last[..last.len() - 1];
            }
        }
        HighlightedLine::Lossless {
            lnum,
            matched,
            parts,
        }
    }

    fn loss(lnum: u64, matched: bool, mut parts: Vec<(Style, String)>) -> Self {
        // Remove newline at the end
        if let Some((_, last)) = parts.last_mut() {
            if last.ends_with('\n') {
                last.pop();
            }
            if last.ends_with('\r') {
                last.pop();
            }
        }
        HighlightedLine::Loss {
            lnum,
            matched,
            parts,
        }
    }

    fn line_number(&self) -> Option<u64> {
        match self {
            HighlightedLine::Lossless { lnum, .. } => Some(*lnum),
            HighlightedLine::Loss { lnum, .. } => Some(*lnum),
            HighlightedLine::Separator => None,
        }
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

struct Drawer<'file, W: Write> {
    lines: Vec<HighlightedLine<'file>>,
    theme: &'file Theme,
    out: W,
    grid: bool,
    tab_width: u16,
    term_width: u16,
    lnum_width: u16,
    background: bool,
    gutter_color: Color,
    match_color: Option<Color>,
    true_color: bool,
}

impl<'file, W: Write> Drawer<'file, W> {
    #[inline]
    fn gutter_width(&self) -> u16 {
        if self.grid {
            self.lnum_width + 4
        } else {
            self.lnum_width + 2
        }
    }

    fn set_bg(&mut self, c: Color) -> Result<()> {
        // In case of c.a == 0 and c.a == 1 are handling for special colorscheme by bat for non true
        // color terminals. Color value is encoded in R. See `to_ansi_color()` in bat/src/terminal.rs
        match c.a {
            0 if c.r <= 7 => write!(self.out, "\x1b[{}m", c.r + 40)?, // 16 colors; e.g. 0 => 40 (Black), 7 => 47 (White)
            0 => write!(self.out, "\x1b[48;5;{}m", c.r)?,             // 256 colors
            1 => { /* Pass through. Do nothing */ }
            _ if self.true_color => write!(self.out, "\x1b[48;2;{};{};{}m", c.r, c.g, c.b)?,
            _ => write!(self.out, "\x1b[48;5;{}m", rgb_to_ansi256(c.r, c.g, c.b))?,
        }
        Ok(())
    }

    fn set_fg(&mut self, c: Color) -> Result<()> {
        // In case of c.a == 0 and c.a == 1 are handling for special colorscheme by bat for non true
        // color terminals. Color value is encoded in R. See `to_ansi_color()` in bat/src/terminal.rs
        match c.a {
            0 if c.r <= 7 => write!(self.out, "\x1b[{}m", c.r + 30)?, // 16 colors; e.g. 3 => 33 (Yellow), 6 => 36 (Cyan)
            0 => write!(self.out, "\x1b[38;5;{}m", c.r)?,             // 256 colors
            1 => { /* Pass through. Do nothing */ }
            _ if self.true_color => write!(self.out, "\x1b[38;2;{};{};{}m", c.r, c.g, c.b)?,
            _ => write!(self.out, "\x1b[38;5;{}m", rgb_to_ansi256(c.r, c.g, c.b))?,
        }
        Ok(())
    }

    fn set_default_bg(&mut self) -> Result<()> {
        if self.background {
            if let Some(bg) = self.theme.settings.background {
                self.set_bg(bg)?;
            }
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

    fn set_style(&mut self, s: Style) -> Result<()> {
        self.set_bg(s.background)?;
        self.set_fg(s.foreground)?;
        Ok(())
    }

    fn draw_horizontal_line(&mut self, sep: &str) -> Result<()> {
        self.set_fg(self.gutter_color)?;
        self.set_default_bg()?;
        let gutter_width = self.gutter_width();
        for _ in 0..gutter_width - 2 {
            self.out.write_all("─".as_bytes())?;
        }
        self.out.write_all(sep.as_bytes())?;
        for _ in 0..self.term_width - gutter_width + 1 {
            self.out.write_all("─".as_bytes())?;
        }
        self.reset_color()?;
        writeln!(self.out)?;
        Ok(())
    }

    fn reset_color(&mut self) -> Result<()> {
        self.out.write_all(b"\x1b[0m")?;
        Ok(())
    }

    fn draw_line_number(&mut self, lnum: u64, matched: bool) -> Result<()> {
        let fg = if matched {
            self.theme.settings.foreground.unwrap()
        } else {
            self.gutter_color
        };
        self.set_fg(fg)?;
        self.set_default_bg()?;
        let width = num_digits(lnum);
        for _ in 0..(self.lnum_width - width) {
            self.out.write_all(b" ")?;
        }
        write!(self.out, " {}", lnum)?;
        if self.grid {
            if matched {
                self.set_fg(self.gutter_color)?;
            }
            self.out.write_all(" │".as_bytes())?;
        }
        self.set_default_bg()?;
        write!(self.out, " ")?;
        Ok(()) // Do not reset color because another color text will follow
    }

    fn draw_separator_line(&mut self) -> Result<()> {
        self.set_fg(self.gutter_color)?;
        self.set_default_bg()?;
        // + 1 for left margin and - 3 for length of "..."
        let left_margin = self.lnum_width + 1 - 3;
        for _ in 0..left_margin {
            self.out.write_all(b" ")?;
        }
        let w = if self.grid {
            write!(self.out, "... ┝")?;
            5
        } else {
            write!(self.out, "...")?;
            3
        };
        self.set_default_bg()?;
        let body_width = self.term_width - left_margin - w; // This crashes when terminal width is smaller than gutter
        for _ in 0..body_width / 2 {
            self.out.write_all(" ━".as_bytes())?;
        }
        Ok(()) // We don't need to reset color for next line
    }

    // Returns number of tab characters in the text
    fn draw_text(&mut self, text: &str) -> Result<usize> {
        if self.tab_width == 0 {
            write!(self.out, "{}", text)?;
            return Ok(0); // XXX: This does not consider width of \t in terminal
        }
        let mut num_tabs = 0;
        let mut start_idx = 0;
        for (i, c) in text.char_indices() {
            if c == '\t' {
                let eaten = &text[start_idx..i];
                write!(self.out, "{}", eaten)?;
                for _ in 0..self.tab_width {
                    self.out.write_all(b" ")?;
                }
                start_idx = i + 1;
                num_tabs += 1;
            }
        }
        let rest = &text[start_idx..];
        write!(self.out, "{}", rest)?;
        Ok(num_tabs)
    }

    #[inline]
    fn text_width(&self, text: &str, num_tabs: usize) -> usize {
        num_tabs * (self.tab_width.saturating_sub(1) as usize) + text.width_cjk()
    }

    fn fill_rest_with_spaces(&mut self, written_width: usize) -> Result<()> {
        let term_width = self.term_width as usize;
        if written_width < term_width {
            for _ in 0..term_width - written_width {
                self.out.write_all(b" ")?;
            }
        }
        self.reset_color()
    }

    fn draw_code_line_bg<'a>(
        &mut self,
        parts: impl Iterator<Item = (Style, &'a str)>,
    ) -> Result<()> {
        let gutter_width = self.gutter_width() as usize;
        let mut width = gutter_width;
        for (style, text) in parts {
            self.set_style(style)?;
            self.set_font_style(style.font_style)?;
            let num_tabs = self.draw_text(text)?;
            self.unset_font_style(style.font_style)?;
            width += self.text_width(text, num_tabs);
        }

        if width == gutter_width {
            self.set_default_bg()?; // For empty line
        }

        self.fill_rest_with_spaces(width)
    }

    fn draw_code_line_no_bg<'a>(
        &mut self,
        parts: impl Iterator<Item = (Style, &'a str)>,
    ) -> Result<()> {
        for (style, text) in parts {
            self.set_fg(style.foreground)?;
            self.set_font_style(style.font_style)?;
            self.draw_text(text)?;
            self.unset_font_style(style.font_style)?;
        }
        self.reset_color()
    }

    fn draw_unmatched_code_line<'a>(
        &mut self,
        parts: impl Iterator<Item = (Style, &'a str)>,
    ) -> Result<()> {
        if self.background {
            self.draw_code_line_bg(parts)
        } else {
            self.draw_code_line_no_bg(parts)
        }
    }

    fn draw_matched_code_line<'a>(
        &mut self,
        bg: Color,
        parts: impl Iterator<Item = (Style, &'a str)>,
    ) -> Result<()> {
        self.set_bg(bg)?;
        let mut width = self.gutter_width() as usize;
        for (style, text) in parts {
            self.set_fg(style.foreground)?;
            self.set_font_style(style.font_style)?;
            let num_tabs = self.draw_text(text)?;
            self.unset_font_style(style.font_style)?;
            width += self.text_width(text, num_tabs);
        }
        self.fill_rest_with_spaces(width)
    }

    fn draw_line(&mut self, line: HighlightedLine<'file>) -> Result<()> {
        match line {
            HighlightedLine::Lossless {
                lnum,
                matched,
                parts,
            } => {
                self.draw_line_number(lnum, matched)?;
                if matched {
                    if let Some(bg) = self.match_color {
                        return self.draw_matched_code_line(bg, parts.into_iter());
                    }
                }
                self.draw_unmatched_code_line(parts.into_iter())
            }
            HighlightedLine::Loss {
                lnum,
                matched,
                parts,
            } => {
                self.draw_line_number(lnum, matched)?;
                let parts = parts.iter().map(|(s, t)| (*s, t.as_str()));
                if matched {
                    if let Some(bg) = self.match_color {
                        return self.draw_matched_code_line(bg, parts);
                    }
                }
                self.draw_unmatched_code_line(parts)
            }
            HighlightedLine::Separator => self.draw_separator_line(),
        }
    }

    fn draw_lines(&mut self) -> Result<()> {
        // Move out self.lines otherwise borrowck complains mutable borrow of &mut self.out and immutable borrow of &self.lines
        let lines = std::mem::take(&mut self.lines);
        for line in lines.into_iter() {
            self.draw_line(line)?;
            writeln!(self.out)?;
        }
        Ok(())
    }

    fn draw_header(&mut self, path: &Path) -> Result<()> {
        self.draw_horizontal_line("─")?;
        self.set_default_bg()?;
        let path = path.as_os_str().to_string_lossy();
        self.set_bold()?;
        write!(self.out, " {}", path)?;
        if self.background {
            self.fill_rest_with_spaces(path.width_cjk() + 1)?;
        } else {
            self.reset_color()?;
        }
        writeln!(self.out)?;
        if self.grid {
            self.draw_horizontal_line("┬")?;
        }
        Ok(())
    }

    fn draw_footer(&mut self) -> Result<()> {
        if self.grid {
            self.draw_horizontal_line("┴")?;
        }
        Ok(())
    }

    fn flush(&mut self) -> Result<()> {
        Ok(self.out.flush()?)
    }
}

fn load_themes(name: Option<&str>) -> Result<ThemeSet> {
    let bat_defaults: ThemeSet =
        bincode::deserialize_from(flate2::read::ZlibDecoder::new(THEME_SET_BIN))?;
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
        Ok(Self {
            writer: out,
            syntaxes: bincode::deserialize_from(flate2::read::ZlibDecoder::new(SYNTAX_SET_BIN))?,
            themes: load_themes(opts.theme)?,
            opts,
        })
    }

    pub fn writer_mut(&mut self) -> &mut W {
        &mut self.writer
    }

    fn parse_highlights<'file>(
        &self,
        file: &'file File,
        syntax: &SyntaxReference,
        theme: &Theme,
    ) -> Vec<HighlightedLine<'file>> {
        assert!(!file.chunks.is_empty());
        let mut hl = LineHighlighter::new(syntax, theme);

        let mut lines = vec![];

        let mut matched = file.line_numbers.as_ref();
        let mut chunks = file.chunks.iter();
        let mut chunk = chunks.next().unwrap(); // OK since chunks is not empty

        // Note: `bytes` contains newline at the end since SyntaxSet requires it. The newline will be trimmed when
        // `HighlightedLine` instance is created.
        for Line(bytes, lnum) in LinesInclusive::new(&file.contents) {
            let (start, end) = *chunk;
            if lnum < start {
                let line = String::from_utf8_lossy(bytes);
                hl.skip_line(line.as_ref(), &self.syntaxes); // Discard parsed result
                continue;
            }
            if start <= lnum && lnum <= end {
                let matched = match matched.first().copied() {
                    Some(n) if n == lnum => {
                        matched = &matched[1..];
                        true
                    }
                    _ => false,
                };
                match std::str::from_utf8(bytes) {
                    Ok(line) => {
                        let ranges = hl.highlight(line, &self.syntaxes);
                        lines.push(HighlightedLine::loss_less(lnum, matched, ranges));
                    }
                    Err(_) => {
                        let line = String::from_utf8_lossy(bytes);
                        // `line` is Cow<'file>, but Cow::<'file>::as_ref() returns &'_ str which does not live long enough
                        let ranges = hl.highlight_owned(&line, &self.syntaxes);
                        lines.push(HighlightedLine::loss(lnum, matched, ranges));
                    }
                }
                if lnum == end {
                    if let Some(c) = chunks.next() {
                        lines.push(HighlightedLine::Separator);
                        chunk = c;
                    } else {
                        break;
                    }
                }
            }
        }

        lines
    }

    fn build_drawer<'file>(
        &self,
        lines: Vec<HighlightedLine<'file>>,
        theme: &'file Theme,
        includes_separator: bool,
    ) -> Drawer<'file, impl Write + '_> {
        let last_lnum = lines[lines.len() - 1].line_number().unwrap(); // Separator is never at the end of line
        let mut lnum_width = num_digits(last_lnum);
        if includes_separator {
            lnum_width = cmp::max(lnum_width, 3); // Consider '...' in gutter
        }
        let gutter_color = theme.settings.gutter_foreground.unwrap_or(Color {
            r: 128,
            g: 128,
            b: 128,
            a: 255,
        });
        Drawer {
            lines,
            theme,
            grid: self.opts.grid,
            term_width: self.opts.term_width,
            lnum_width,
            tab_width: self.opts.tab_width as u16,
            background: self.opts.background_color,
            gutter_color,
            match_color: theme.settings.line_highlight.or(theme.settings.background),
            true_color: self.opts.color_support == TermColorSupport::True,
            out: BufWriter::new(self.writer.lock()), // Take lock here to print files in serial from multiple threads
        }
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
        if file.chunks.is_empty() || file.line_numbers.is_empty() {
            return Ok(());
        }

        let theme = self.theme();
        let syntax = self.find_syntax(&file.path)?;
        let highlighted = self.parse_highlights(&file, syntax, theme);
        let include_separator = file.chunks.len() > 1;
        let mut drawer = self.build_drawer(highlighted, theme, include_separator); // Lock is acquired here
        drawer.draw_header(&file.path)?;
        drawer.draw_lines()?;
        drawer.draw_footer()?;
        drawer.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunk::File;
    use std::cell::{RefCell, RefMut};
    use std::fmt;
    use std::fs;
    use std::mem;
    use std::path::PathBuf;

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

    #[cfg(not(windows))]
    mod uitests {
        use super::*;
        use std::path::Path;

        fn read_chunks(path: PathBuf) -> File {
            let contents = fs::read(&path).unwrap();
            let mut lnums = vec![];
            let mut chunks = vec![];
            for (idx, line) in contents.split_inclusive(|b| *b == b'\n').enumerate() {
                let lnum = (idx + 1) as u64;
                let pat = "*match to this line*".as_bytes();
                if line.windows(pat.len()).any(|s| s == pat) {
                    lnums.push(lnum);
                    chunks.push((lnum.saturating_sub(6), lnum + 6));
                }
            }
            File::new(path, lnums, chunks, contents)
        }

        fn run_uitest(infile: PathBuf, outfile: PathBuf, f: fn(&mut PrinterOptions<'_>) -> ()) {
            let stdout = DummyStdout(RefCell::new(vec![]));
            let mut opts = PrinterOptions::default();
            opts.term_width = 80;
            opts.color_support = TermColorSupport::True;
            f(&mut opts);
            let mut printer = SyntectPrinter::new(stdout, opts).unwrap();
            let file = read_chunks(infile);
            printer.print(file).unwrap();
            let printed = mem::take(printer.writer_mut()).0.into_inner();
            let expected = fs::read(outfile).unwrap();
            assert_eq!(
                printed,
                expected,
                "got:\n{}\nwant:\n{}",
                String::from_utf8_lossy(&printed),
                String::from_utf8_lossy(&expected),
            );
        }

        fn run_parametrized_uitest(mut input: &str, f: fn(&mut PrinterOptions<'_>) -> ()) {
            let dir = Path::new(".").join("testdata").join("syntect");
            if input.starts_with("test_") {
                input = &input["test_".len()..];
            }
            let infile = dir.join(format!("{}.rs", input));
            let outfile = dir.join(format!("{}.out", input));
            run_uitest(infile, outfile, f);
        }

        macro_rules! uitest {
            ($($input:ident($f:expr),)+) => {
                $(
                    #[cfg(not(windows))]
                    #[test]
                    fn $input() {
                        run_parametrized_uitest(stringify!($input), $f);
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

    fn readme_chunk() -> File {
        let readme = PathBuf::from("README.md");
        let lnums = vec![3];
        let chunks = vec![(1, 6)];
        let contents = fs::read(&readme).unwrap();
        File::new(readme, lnums, chunks, contents)
    }

    #[test]
    fn test_error_write() {
        let file = readme_chunk();
        let opts = PrinterOptions::default();
        let printer = SyntectPrinter::new(ErrorStdout, opts).unwrap();
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
        let mut printer = SyntectPrinter::new(stdout, opts).unwrap();
        printer.print(file).unwrap();
        let printed = mem::take(printer.writer_mut()).0.into_inner();
        assert!(
            printed.is_empty(),
            "pritned:\n{}",
            String::from_utf8_lossy(&printed)
        );
    }
}
