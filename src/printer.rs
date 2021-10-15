use crate::chunk::File;
use anyhow::{Error, Result};
use bat::line_range::{LineRange, LineRanges};
use bat::{Input, PrettyPrinter};
use std::fmt;
use std::path::PathBuf;

#[derive(Debug)]
pub struct PrintError {
    path: PathBuf,
    cause: Option<String>,
}

impl std::error::Error for PrintError {}

impl fmt::Display for PrintError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Could not print file {:?}", &self.path)?;
        if let Some(cause) = &self.cause {
            write!(f, ". Caused by: {}", cause)?;
        }
        Ok(())
    }
}

// Trait to replace printer implementation for unit tests
pub trait Printer {
    fn print(&self, file: File) -> Result<()>;
}

pub struct BatPrinter<'a> {
    theme: Option<&'a str>,
    tab_width: Option<usize>,
    grid: bool,
}

impl<'a> BatPrinter<'a> {
    pub fn new() -> Self {
        Self {
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
    fn print(&self, file: File) -> Result<()> {
        // XXX: PrettyPrinter instance must be created for each print() call because there is no way
        // to clear line_ranges in the instance.
        let mut pp = PrettyPrinter::new();

        let input = Input::from_bytes(&file.contents)
            .name(&file.path)
            .kind("File");
        pp.input(input);

        pp.line_numbers(true);
        pp.grid(self.grid);
        pp.header(true);
        pp.snip(true);
        if let Some(theme) = self.theme {
            pp.theme(theme);
        }

        let ranges = file
            .chunks
            .iter()
            .map(|(s, e)| LineRange::new(*s as usize, *e as usize))
            .collect();

        pp.line_ranges(LineRanges::from(ranges));

        for lnum in file.line_numbers.iter().copied() {
            pp.highlight(lnum as usize);
        }

        if !self.grid {
            print!("\n\n"); // Empty lines as files separator
        }

        // Note: print() returns true when no error
        // Note: bat's Error type cannot be converted to anyhow::Error due to lack of some type bounds
        match pp.print() {
            Ok(true) => Ok(()),
            Ok(false) => Err(Error::new(PrintError {
                path: file.path,
                cause: None,
            })),
            Err(err) => Err(Error::new(PrintError {
                path: file.path,
                cause: Some(format!("{}", err)),
            })),
        }
    }
}
