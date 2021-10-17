use crate::chunk::File;
use anyhow::{Error, Result};
use bat::assets::HighlightingAssets;
use bat::config::{Config, VisibleLines};
use bat::controller::Controller;
use bat::input::Input;
use bat::line_range::{HighlightedLineRanges, LineRange, LineRanges};
use bat::style::{StyleComponent, StyleComponents};
use console::Term;
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

pub struct BatPrinter<'main> {
    grid: bool,
    config: Config<'main>,
    assets: HighlightingAssets,
}

impl<'main> BatPrinter<'main> {
    pub fn new() -> Self {
        let styles = &[
            StyleComponent::LineNumbers,
            StyleComponent::Snip,
            StyleComponent::Header,
            StyleComponent::Grid,
        ];
        let config = Config {
            colored_output: true,
            true_color: true,
            term_width: Term::stdout().size().1 as usize,
            style_components: StyleComponents::new(styles),
            ..Default::default()
        };
        Self {
            grid: true,
            assets: HighlightingAssets::from_binary(),
            config,
        }
    }

    pub fn tab_width(&mut self, width: usize) {
        self.config.tab_width = width;
    }

    pub fn theme(&mut self, theme: &str) {
        self.config.theme = theme.to_string();
    }

    pub fn no_grid(&mut self) {
        self.grid = false;
        self.config.style_components.0.remove(&StyleComponent::Grid);
    }
}

impl<'a> Printer for BatPrinter<'a> {
    fn print(&self, file: File) -> Result<()> {
        if file.chunks.is_empty() || file.line_numbers.is_empty() {
            return Ok(()); // Ensure to print some match
        }

        // XXX: We don't use `bat::PrettyPrinter`.
        //
        // `bat::PrettyPrinter` is an API exposed by bat and intended to be used by other Rust programs.
        // However this does not fit to hgrep implementation.
        //
        // 1. `bat::PrettyPrinter` cannot be shared across threads even if it is guarded with `Mutex<T>`
        //    since it keeps `dyn io::Read` value. hgrep processes each files in `rayon::ParallelIterator`,
        //    a printer instance must be used by each threads.
        // 2. `bat::PrettyPrinter` does not provide a way to clear highlighted lines. So the printer
        //    instance cannot be reused for the next file. `bat::PrettyPrinter` can print multiple files
        //    at once. But it does not provide a way to specify the highlighted lines per file. The same
        //    lines are highlighted in all files and it is not fit to hgrep use case.
        // 3. To avoid 1. and 2., we created `bat::PrettyPrinter` instance per `BatPrinter::print()` call.
        //    It worked but was very slow since it loaded syntax highlighting assets each time. It was
        //    3.3x slower than current implementation. See commit 8655b801b40f8b3f7d4d343cae185604fa918d5b
        //    for more details.

        let mut config = self.config.clone();

        let ranges = file
            .chunks
            .iter()
            .map(|(s, e)| LineRange::new(*s as usize, *e as usize))
            .collect();
        config.visible_lines = VisibleLines::Ranges(LineRanges::from(ranges));

        let input =
            Input::from_reader(Box::new(file.contents.as_ref())).with_name(Some(&file.path));

        let ranges = file
            .line_numbers
            .iter()
            .map(|n| LineRange::new(*n as usize, *n as usize))
            .collect();
        config.highlighted_lines = HighlightedLineRanges(LineRanges::from(ranges));

        if !self.grid {
            print!("\n\n"); // Empty lines as files separator
        }

        let controller = Controller::new(&config, &self.assets);
        // Note: controller.run() returns true when no error
        // XXX: bat's Error type cannot be converted to anyhow::Error since it does not implement Sync
        match controller.run(vec![input]) {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_file() -> File {
        let path = PathBuf::from("test.rs");
        let lnums = vec![1];
        let chunks = vec![(1, 2)];
        let contents = "fn main() {\n    println!(\"hello\");\n}\n"
            .as_bytes()
            .to_vec();
        File::new(path, lnums, chunks, contents)
    }

    #[test]
    fn test_print_default() {
        let p = BatPrinter::new();
        let f = sample_file();
        p.print(f).unwrap();
    }

    #[test]
    fn test_print_with_flags() {
        let mut p = BatPrinter::new();
        p.tab_width(2);
        p.theme("Nord");
        p.no_grid();
        let f = sample_file();
        p.print(f).unwrap();
    }

    #[test]
    fn test_print_nothing() {
        let p = BatPrinter::new();
        let f = File::new(PathBuf::from("x.txt"), vec![], vec![], vec![]);
        p.print(f).unwrap();
    }
}
