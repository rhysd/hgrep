use crate::chunk::File;
use crate::chunk::{Line, Lines};
use crate::printer::{Printer, PrinterOptions};
use anyhow::Result;
use console::Term;
use std::ffi::OsStr;
use std::io;
use std::io::Write;
use std::path::Path;
use syntect::easy::HighlightLines;
use syntect::highlighting::{Style, ThemeSet};
use syntect::parsing::{SyntaxReference, SyntaxSet};
use unicode_width::UnicodeWidthStr;

enum HighlightedLine<'file> {
    Lossless(u64, Vec<(Style, &'file str)>),
    Loss(u64, Vec<(Style, String)>),
    // TODO: Add snip separator
}

impl<'file> HighlightedLine<'file> {
    fn line_number(&self) -> u64 {
        match self {
            HighlightedLine::Lossless(n, _) => *n,
            HighlightedLine::Loss(n, _) => *n,
        }
    }
}

struct Writer<'file, W: Write> {
    lines: Vec<HighlightedLine<'file>>,
    out: W,
    grid: bool,
    tab_width: u16,
    term_width: u16,
    lnum_width: u16,
}

impl<'file, W: Write> Writer<'file, W> {
    #[inline]
    fn gutter_width(&self) -> u16 {
        self.lnum_width + 3
    }

    fn write_reset(&mut self) -> Result<()> {
        self.out.write_all(b"\x1b[0m")?;
        Ok(())
    }

    fn write_line_number(&mut self, lnum: u64) -> Result<()> {
        self.write_reset()?;
        let width = (lnum as f64).log10() as u16;
        for _ in 0..(self.lnum_width - width) {
            self.out.write_all(b" ")?;
        }
        write!(self.out, " {}: ", lnum)?;
        Ok(())
    }

    fn write_text(&mut self, text: &str) -> Result<u16> {
        if self.tab_width == 0 {
            write!(self.out, "{}", text)?;
            return Ok(text.width_cjk() as u16);
        }
        unimplemented!()
    }

    fn write_line_body<'b>(&mut self, parts: impl Iterator<Item = (Style, &'b str)>) -> Result<()> {
        // TODO: 256 colors terminal support
        for (style, text) in parts {
            write!(
                self.out,
                "\x1b[48;2;{};{};{}m\x1b[38;2;{};{};{}m{}",
                style.background.r,
                style.background.g,
                style.background.b,
                style.foreground.r,
                style.foreground.g,
                style.foreground.b,
                text
            )?;
        }
        Ok(())
    }

    fn write_line(&mut self, line: HighlightedLine<'file>) -> Result<()> {
        match line {
            HighlightedLine::Lossless(lnum, parts) => {
                self.write_line_number(lnum)?;
                self.write_line_body(parts.into_iter())?;
            }
            HighlightedLine::Loss(lnum, parts) => {
                self.write_line_number(lnum)?;
                self.write_line_body(parts.iter().map(|(s, t)| (*s, t.as_str())))?;
            }
        }
        Ok(())
    }

    fn write_lines(&mut self) -> Result<()> {
        // Move out self.lines otherwise borrowck complains mutable borrow of &mut self.out and immutable borrow of &self.lines
        let lines = std::mem::take(&mut self.lines);
        for line in lines.into_iter() {
            self.write_line(line)?;
            writeln!(self.out)?;
        }
        self.write_reset()
    }

    fn write_header(&mut self, path: &Path) -> Result<()> {
        writeln!(self.out)?;
        writeln!(self.out, "{:?}", path)?;
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
    ) -> Vec<HighlightedLine<'file>> {
        assert!(!file.chunks.is_empty());
        let mut hl = HighlightLines::new(syntax, &self.themes.themes["base16-ocean.dark"]); // TODO: theme

        // TODO: Consider capacity. It would be able to be calculated by {num of chunks} * {min context lines}
        let mut lines = vec![];

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
                match std::str::from_utf8(bytes) {
                    Ok(line) => {
                        let ranges = hl.highlight(line, &self.syntaxes);
                        lines.push(HighlightedLine::Lossless(lnum, ranges));
                    }
                    Err(_) => {
                        let line = String::from_utf8_lossy(bytes);
                        let ranges = hl.highlight(&line, &self.syntaxes);
                        // `line` is Cow<'file>, but Cow::<'file>::as_ref() returns &'_ str which does not live long enough
                        let ranges = ranges
                            .into_iter()
                            .map(|(n, text)| (n, text.to_string()))
                            .collect();
                        lines.push(HighlightedLine::Loss(lnum, ranges));
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
    ) -> Writer<'file, io::StdoutLock<'_>> {
        let last_lnum = lines[lines.len() - 1].line_number();
        let lnum_width = (last_lnum as f64).log10() as u16;
        Writer {
            lines,
            out: self.stdout.lock(), // Take lock here to print files in serial from multiple threads
            grid: self.opts.grid,
            term_width: self.term_width,
            lnum_width,
            tab_width: self.opts.tab_width as u16,
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
            let highlighted = self.parse_highlights(&file, syntax);
            let mut writer = self.build_writer(highlighted);
            writer.write_header(&file.path)?;
            writer.write_lines()
        } else {
            unimplemented!()
        }
    }
}
