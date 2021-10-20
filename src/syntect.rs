use crate::chunk::File;
use crate::chunk::{Line, Lines};
use crate::printer::{Printer, PrinterOptions};
use anyhow::Result;
use console::Term;
use std::cmp;
use std::ffi::OsStr;
use std::io;
use std::io::BufWriter;
use std::io::Write;
use std::path::Path;
use syntect::easy::HighlightLines;
use syntect::highlighting::{Color, Style, Theme, ThemeSet};
use syntect::parsing::{SyntaxReference, SyntaxSet};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

enum HighlightedLine<'file> {
    Lossless(u64, bool, Vec<(Style, &'file str)>),
    Loss(u64, bool, Vec<(Style, String)>),
    // TODO: Add snip separator
}

impl<'file> HighlightedLine<'file> {
    fn line_number(&self) -> u64 {
        match self {
            HighlightedLine::Lossless(n, _, _) => *n,
            HighlightedLine::Loss(n, _, _) => *n,
        }
    }
}

struct Writer<'file, W: Write> {
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
}

impl<'file, W: Write> Writer<'file, W> {
    #[inline]
    fn gutter_width(&self) -> u16 {
        if self.grid {
            self.lnum_width + 5
        } else {
            self.lnum_width + 3
        }
    }

    // TODO: 256 colors terminal support
    fn write_bg(&mut self, c: Color) -> Result<()> {
        write!(self.out, "\x1b[48;2;{};{};{}m", c.r, c.g, c.b)?;
        Ok(())
    }

    fn write_fg(&mut self, c: Color) -> Result<()> {
        write!(self.out, "\x1b[38;2;{};{};{}m", c.r, c.g, c.b)?;
        Ok(())
    }

    fn write_bg_color(&mut self) -> Result<()> {
        if self.background {
            if let Some(bg) = self.theme.settings.background {
                self.write_bg(bg)?;
            }
        }
        Ok(())
    }

    fn write_style(&mut self, s: Style) -> Result<()> {
        self.write_bg(s.background)?;
        self.write_fg(s.foreground)?;
        Ok(())
    }

    fn write_horizontal_line(&mut self, sep: &str) -> Result<()> {
        self.write_fg(self.gutter_color)?;
        let gutter_width = self.gutter_width();
        for _ in 0..gutter_width - 3 {
            self.out.write_all("─".as_bytes())?;
        }
        self.out.write_all(sep.as_bytes())?;
        for _ in 0..self.term_width - gutter_width + 2 {
            self.out.write_all("─".as_bytes())?;
        }
        self.write_reset()
    }

    fn write_reset(&mut self) -> Result<()> {
        self.out.write_all(b"\x1b[0m")?;
        Ok(())
    }

    fn write_line_number(&mut self, lnum: u64) -> Result<()> {
        self.write_fg(self.gutter_color)?;
        let width = (lnum as f64).log10() as u16 + 1;
        for _ in 0..(self.lnum_width - width) {
            self.out.write_all(b" ")?;
        }
        if self.grid {
            write!(self.out, " {} │", lnum)?;
        } else {
            write!(self.out, " {}", lnum)?;
        }
        self.write_bg_color()?;
        write!(self.out, " ")?;
        Ok(()) // Do not reset color because another color text will follow
    }

    fn write_text(&mut self, text: &str) -> Result<usize> {
        if self.tab_width == 0 {
            write!(self.out, "{}", text)?;
            return Ok(text.width_cjk()); // XXX: This does not consider width of \t in terminal
        }
        let mut width = 0;
        let mut start_idx = 0;
        for (i, c) in text.char_indices() {
            if c == '\t' {
                write!(self.out, "{}", &text[start_idx..i])?;
                for _ in 0..self.tab_width {
                    self.out.write_all(b" ")?;
                }
                start_idx = i + 1;
                width += self.tab_width as usize;
            } else {
                width += c.width_cjk().unwrap_or(0);
            }
        }
        let rest = &text[start_idx..];
        write!(self.out, "{}", rest)?;
        width = rest.width_cjk();
        Ok(width)
    }

    fn write_line_body_bg<'a>(
        &mut self,
        parts: impl Iterator<Item = (Style, &'a str)>,
    ) -> Result<()> {
        let gutter_width = self.gutter_width() as usize;
        let mut width = gutter_width;
        for (style, text) in parts {
            self.write_style(style)?;
            width += self.write_text(text)?;
        }

        if width == gutter_width {
            self.write_bg_color()?; // For empty line
        }

        let term_width = self.term_width as usize;
        if width < term_width {
            for _ in 0..term_width - width {
                self.out.write_all(b" ")?;
            }
        }
        self.write_reset()
    }

    fn write_line_body_no_bg<'a>(
        &mut self,
        parts: impl Iterator<Item = (Style, &'a str)>,
    ) -> Result<()> {
        for (style, text) in parts {
            self.write_fg(style.foreground)?;
            self.out.write_all(text.as_bytes())?;
        }
        self.write_reset()
    }

    fn write_line_body<'a>(&mut self, parts: impl Iterator<Item = (Style, &'a str)>) -> Result<()> {
        if self.background {
            self.write_line_body_bg(parts)
        } else {
            self.write_line_body_no_bg(parts)
        }
    }

    fn write_line_body_with_bg<'a>(
        &mut self,
        bg: Color,
        parts: impl Iterator<Item = (Style, &'a str)>,
    ) -> Result<()> {
        let gutter_width = self.gutter_width() as usize;
        self.write_bg(bg)?;
        let mut width = gutter_width;
        for (style, text) in parts {
            self.write_fg(style.foreground)?;
            width += self.write_text(text)?;
        }
        let term_width = self.term_width as usize;
        if width < term_width {
            for _ in 0..term_width - width {
                self.out.write_all(b" ")?;
            }
        }
        self.write_reset()
    }

    fn write_line(&mut self, line: HighlightedLine<'file>) -> Result<()> {
        match line {
            HighlightedLine::Lossless(lnum, matched, parts) => {
                self.write_line_number(lnum)?;
                if matched {
                    if let Some(bg) = self.match_color {
                        return self.write_line_body_with_bg(bg, parts.into_iter());
                    }
                }
                self.write_line_body(parts.into_iter())
            }
            HighlightedLine::Loss(lnum, matched, parts) => {
                self.write_line_number(lnum)?;
                let parts = parts.iter().map(|(s, t)| (*s, t.as_str()));
                if matched {
                    if let Some(bg) = self.match_color {
                        return self.write_line_body_with_bg(bg, parts);
                    }
                }
                self.write_line_body(parts)
            }
        }
    }

    fn write_lines(&mut self) -> Result<()> {
        // Move out self.lines otherwise borrowck complains mutable borrow of &mut self.out and immutable borrow of &self.lines
        let lines = std::mem::take(&mut self.lines);
        for line in lines.into_iter() {
            self.write_line(line)?;
            writeln!(self.out)?;
        }
        Ok(())
    }

    fn write_header(&mut self, path: &Path) -> Result<()> {
        self.write_horizontal_line("─")?;
        writeln!(self.out, "\x1b[1m {}", path.as_os_str().to_string_lossy())?;
        self.write_reset()?;
        if self.grid {
            self.write_horizontal_line("┬")?;
        }
        Ok(())
    }

    fn write_footer(&mut self) -> Result<()> {
        if self.grid {
            self.write_horizontal_line("┴")?;
        }
        Ok(())
    }
}

pub struct SyntectPrinter<'main> {
    stdout: io::Stdout, // Protected with mutex because it should print file by file
    syntaxes: SyntaxSet,
    themes: ThemeSet,
    opts: PrinterOptions<'main>,
    term_width: u16,
}

impl<'main> SyntectPrinter<'main> {
    pub fn new(opts: PrinterOptions<'main>) -> Self {
        let syntaxes = SyntaxSet::load_defaults_newlines();
        let themes = ThemeSet::load_defaults();
        Self {
            stdout: io::stdout(),
            syntaxes,
            themes,
            opts,
            term_width: Term::stdout().size().1,
        }
    }

    pub fn themes(&self) -> impl Iterator<Item = &str> {
        self.themes.themes.keys().map(AsRef::as_ref)
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
                        chunk = c;
                    } else {
                        break;
                    }
                }
            }
        }

        lines
    }

    fn build_writer<'file>(
        &self,
        lines: Vec<HighlightedLine<'file>>,
        theme: &'file Theme,
    ) -> Writer<'file, impl Write + '_> {
        let last_lnum = lines[lines.len() - 1].line_number();
        let lnum_width = cmp::max((last_lnum as f64).log10() as u16 + 1, 3); // Consider '...' in gutter
        let gutter_color = theme.settings.gutter_foreground.unwrap_or(Color {
            r: 128,
            g: 128,
            b: 128,
            a: 0,
        });
        Writer {
            lines,
            theme,
            grid: self.opts.grid,
            term_width: self.term_width,
            lnum_width,
            tab_width: self.opts.tab_width as u16,
            background: self.opts.background_color,
            gutter_color,
            match_color: theme.settings.line_highlight.or(theme.settings.background),
            // match_color: theme.settings.line_highlight.or(theme.settings.background),
            out: BufWriter::new(self.stdout.lock()), // Take lock here to print files in serial from multiple threads
        }
    }
}

impl<'main> Printer for SyntectPrinter<'main> {
    fn print(&self, file: File) -> Result<()> {
        if file.chunks.is_empty() || file.line_numbers.is_empty() {
            return Ok(());
        }

        if let Some(syntax) = file
            .path
            .extension()
            .and_then(OsStr::to_str)
            .and_then(|ext| self.syntaxes.find_syntax_by_extension(ext))
        {
            let theme = &self.themes.themes["base16-ocean.dark"]; // TODO: Theme
            let highlighted = self.parse_highlights(&file, syntax, theme);
            let mut writer = self.build_writer(highlighted, theme);
            writer.write_header(&file.path)?;
            writer.write_lines()?;
            writer.write_footer()
        } else {
            unimplemented!()
        }
    }
}
