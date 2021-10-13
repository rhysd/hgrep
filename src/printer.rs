use crate::chunk::Chunk;
use anyhow::{Error, Result};
use bat::line_range::{LineRange, LineRanges};
use bat::{Input, PrettyPrinter};
use std::fmt;
use std::path::PathBuf;

#[derive(Debug)]
pub struct PrintError {
    path: PathBuf,
    start: u64,
    end: u64,
    cause: Option<String>,
}

impl std::error::Error for PrintError {}

impl fmt::Display for PrintError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Could not print range L{}..L{} of {:?}",
            self.start, self.end, &self.path,
        )?;
        if let Some(cause) = &self.cause {
            write!(f, ". Caused by: {}", cause)?;
        }
        Ok(())
    }
}

// Trait to replace printer implementation for unit tests
pub trait Printer {
    fn print(&self, chunk: Chunk) -> Result<()>;
}

pub struct BatPrinter<'a> {
    context_lines: u64,
    theme: Option<&'a str>,
    tab_width: Option<usize>,
    grid: bool,
}

impl<'a> BatPrinter<'a> {
    pub fn new(context_lines: u64) -> Self {
        Self {
            context_lines,
            theme: None,
            tab_width: None,
            grid: true,
        }
    }

    pub fn tab_width(&mut self, width: usize) {
        self.tab_width = Some(width);
    }

    pub fn theme(&mut self, theme: &'a str) {
        self.theme = Some(theme);
    }

    pub fn grid(&mut self, enabled: bool) {
        self.grid = enabled;
    }
}

impl<'a> Printer for BatPrinter<'a> {
    fn print(&self, chunk: Chunk) -> Result<()> {
        // XXX: PrettyPrinter instance must be created for each print() call because there is no way
        // to clear line_ranges in the instance.
        let mut pp = PrettyPrinter::new();

        let input = Input::from_file(&chunk.path).name(&chunk.path).kind("File");
        pp.input(input);

        pp.line_numbers(true);
        pp.grid(self.grid);
        pp.header(true);
        if let Some(theme) = self.theme {
            pp.theme(theme);
        }

        let start = chunk.line_numbers[0].saturating_sub(self.context_lines);
        let end = chunk.line_numbers[chunk.line_numbers.len() - 1] + self.context_lines;
        pp.line_ranges(LineRanges::from(vec![LineRange::new(
            start as usize,
            end as usize,
        )]));

        for lnum in chunk.line_numbers.iter().copied() {
            pp.highlight(lnum as usize);
        }

        if !self.grid {
            print!("\n\n");
        }

        // Note: print() returns true when no error
        // Note: bat's Error type cannot be converted to anyhow::Error due to lack of some type bounds
        match pp.print() {
            Ok(true) => Ok(()),
            Ok(false) => Err(Error::new(PrintError {
                path: chunk.path,
                start,
                end,
                cause: None,
            })),
            Err(err) => Err(Error::new(PrintError {
                path: chunk.path,
                start,
                end,
                cause: Some(format!("{}", err)),
            })),
        }
    }
}
