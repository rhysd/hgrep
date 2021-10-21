use crate::chunk::File;
use crate::chunk::{Line, Lines};
use crate::printer::{Printer, PrinterOptions};
use anyhow::Result;
use console::Term;
use rgb2ansi256::rgb_to_ansi256;
use std::cmp;
use std::collections::HashSet;
use std::env;
use std::ffi::OsStr;
use std::fmt;
use std::io;
use std::io::{BufWriter, Write};
use std::path::Path;
use syntect::easy::HighlightLines;
use syntect::highlighting::{Color, Style, Theme, ThemeSet};
use syntect::parsing::{SyntaxReference, SyntaxSet};
use term::terminfo::TermInfo;
use unicode_width::UnicodeWidthStr;

// Note for lifetimes:
// - 'file is a lifetime for File instance which is passed to print() method
// - 'main is a lifetime for the scope of main function (the caller of printer)

const SYNTAX_SET_BIN: &[u8] = include_bytes!("../assets/bat/assets/syntaxes.bin");
const THEME_SET_BIN: &[u8] = include_bytes!("../assets/bat/assets/themes.bin");

#[derive(Clone, Copy, PartialEq, Eq)]
enum TermColorSupport {
    True,
    Ansi256,
    Ansi16,
}

impl TermColorSupport {
    fn detect() -> Self {
        if env::var("COLORTERM")
            .ok()
            .map(|v| v == "truecolor")
            .unwrap_or(false)
        {
            return TermColorSupport::True;
        }

        if let Ok(info) = TermInfo::from_env() {
            if let Some(colors) = info.numbers.get("colors") {
                if *colors == 256 {
                    return TermColorSupport::Ansi256;
                } else {
                    return TermColorSupport::Ansi16;
                }
            }
        }

        // Assume 256 colors by default (I'm not sure this is correct)
        TermColorSupport::Ansi256
    }
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

enum HighlightedLine<'file> {
    Lossless(u64, bool, Vec<(Style, &'file str)>),
    Loss(u64, bool, Vec<(Style, String)>),
    Separator,
}

impl<'file> HighlightedLine<'file> {
    fn line_number(&self) -> Option<u64> {
        match self {
            HighlightedLine::Lossless(n, _, _) => Some(*n),
            HighlightedLine::Loss(n, _, _) => Some(*n),
            HighlightedLine::Separator => None,
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
            self.lnum_width + 5
        } else {
            self.lnum_width + 3
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
            0 if c.r <= 7 => write!(self.out, "\x1b[{}m", c.r + 30)?, // 16 colors; e.g. 0 => 33 (Yellow), 6 => 36 (Cyan)
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

    fn set_style(&mut self, s: Style) -> Result<()> {
        self.set_bg(s.background)?;
        self.set_fg(s.foreground)?;
        Ok(())
    }

    fn draw_horizontal_line(&mut self, sep: &str) -> Result<()> {
        self.set_fg(self.gutter_color)?;
        let gutter_width = self.gutter_width();
        for _ in 0..gutter_width - 3 {
            self.out.write_all("─".as_bytes())?;
        }
        self.out.write_all(sep.as_bytes())?;
        for _ in 0..self.term_width - gutter_width + 2 {
            self.out.write_all("─".as_bytes())?;
        }
        self.reset_color()
    }

    fn reset_color(&mut self) -> Result<()> {
        self.out.write_all(b"\x1b[0m")?;
        Ok(())
    }

    fn draw_line_number(&mut self, lnum: u64, matched: bool) -> Result<()> {
        let color = if matched {
            self.theme.settings.foreground.unwrap()
        } else {
            self.gutter_color
        };
        self.set_fg(color)?;
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
        // + 1 for left margin and - 3 for length of "..."
        let left_margin = self.lnum_width + 1 - 3;
        for _ in 0..left_margin {
            self.out.write_all(b" ")?;
        }
        let gutter_width = if self.grid {
            write!(self.out, "... ┝")?;
            5
        } else {
            write!(self.out, "...")?;
            3
        };
        self.set_default_bg()?;
        let body_width = self.term_width - left_margin - gutter_width; // This crashes when terminal width is smaller than gutter
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
            for _ in 0..term_width - written_width + 1 {
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
            let num_tabs = self.draw_text(text)?;
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
            self.draw_text(text)?;
        }
        self.reset_color()
    }

    fn draw_code_line<'a>(&mut self, parts: impl Iterator<Item = (Style, &'a str)>) -> Result<()> {
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
            let num_tabs = self.draw_text(text)?;
            width += self.text_width(text, num_tabs);
        }
        self.fill_rest_with_spaces(width)
    }

    fn draw_line(&mut self, line: HighlightedLine<'file>) -> Result<()> {
        match line {
            HighlightedLine::Lossless(lnum, matched, parts) => {
                self.draw_line_number(lnum, matched)?;
                if matched {
                    if let Some(bg) = self.match_color {
                        return self.draw_matched_code_line(bg, parts.into_iter());
                    }
                }
                self.draw_code_line(parts.into_iter())
            }
            HighlightedLine::Loss(lnum, matched, parts) => {
                self.draw_line_number(lnum, matched)?;
                let parts = parts.iter().map(|(s, t)| (*s, t.as_str()));
                if matched {
                    if let Some(bg) = self.match_color {
                        return self.draw_matched_code_line(bg, parts);
                    }
                }
                self.draw_code_line(parts)
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
        writeln!(self.out, "\x1b[1m {}", path.as_os_str().to_string_lossy())?;
        self.reset_color()?;
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
}

impl<'file, W: Write> Drop for Drawer<'file, W> {
    fn drop(&mut self) {
        self.out
            .flush()
            .expect("could not flush stdout for syntect printer");
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

pub struct SyntectPrinter<'main> {
    stdout: io::Stdout, // Protected with mutex because it should print file by file
    syntaxes: SyntaxSet,
    themes: ThemeSet,
    opts: PrinterOptions<'main>,
    term_width: u16,
    term_support: TermColorSupport,
}

impl<'main> SyntectPrinter<'main> {
    pub fn new(opts: PrinterOptions<'main>) -> Result<Self> {
        Ok(Self {
            stdout: io::stdout(),
            syntaxes: bincode::deserialize_from(flate2::read::ZlibDecoder::new(SYNTAX_SET_BIN))?,
            themes: load_themes(opts.theme)?,
            opts,
            term_width: Term::stdout().size().1,
            term_support: TermColorSupport::detect(),
        })
    }

    fn parse_highlights<'file>(
        &self,
        file: &'file File,
        syntax: &SyntaxReference,
        theme: &Theme,
    ) -> Vec<HighlightedLine<'file>> {
        assert!(!file.chunks.is_empty());
        let mut hl = HighlightLines::new(syntax, theme);

        // TODO: Consider capacity. It would be able to be calculated by {num of chunks} * {min context lines}
        let mut lines = vec![];

        let mut matched = file.line_numbers.as_ref();
        let mut chunks = file.chunks.iter();
        let mut chunk = chunks.next().unwrap(); // OK since chunks is not empty

        for Line(bytes, lnum) in Lines::new(&file.contents) {
            let (start, end) = *chunk;
            if lnum < start {
                let line = String::from_utf8_lossy(bytes);
                hl.highlight(line.as_ref(), &self.syntaxes); // XXX: Returned Vec is discarded.
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
                        lines.push(HighlightedLine::Lossless(lnum, matched, ranges));
                    }
                    Err(_) => {
                        let line = String::from_utf8_lossy(bytes);
                        let ranges = hl.highlight(&line, &self.syntaxes);
                        // `line` is Cow<'file>, but Cow::<'file>::as_ref() returns &'_ str which does not live long enough
                        let ranges = ranges
                            .into_iter()
                            .map(|(n, text)| (n, text.to_string()))
                            .collect();
                        lines.push(HighlightedLine::Loss(lnum, matched, ranges));
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
            term_width: self.term_width,
            lnum_width,
            tab_width: self.opts.tab_width as u16,
            background: self.opts.background_color,
            gutter_color,
            match_color: theme.settings.line_highlight.or(theme.settings.background),
            true_color: self.term_support == TermColorSupport::True,
            out: BufWriter::new(self.stdout.lock()), // Take lock here to print files in serial from multiple threads
        }
    }

    fn theme(&self) -> &Theme {
        let name = self.opts.theme.unwrap_or_else(|| {
            if self.term_support == TermColorSupport::Ansi16 {
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

impl<'main> Printer for SyntectPrinter<'main> {
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
        drawer.draw_footer()
    }
}
