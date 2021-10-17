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
        let config = Config {
            colored_output: true,
            true_color: true,
            term_width: Term::stdout().size().1 as usize,
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

    pub fn grid(&mut self, enabled: bool) {
        self.grid = enabled;
    }
}

impl<'a> Printer for BatPrinter<'a> {
    fn print(&self, file: File) -> Result<()> {
        if file.chunks.is_empty() || file.line_numbers.is_empty() {
            return Ok(()); // Ensure to print some match
        }

        // XXX: We don't use `bat::PrettyPrinter`.
        //
        // `bat::PrettyPrinter` is an API exposed by bat and intended to be used by other Rust prgoram.
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
        //    It worked but is very slow. It is 3.3x slower than current implementation. See commit
        //    8655b801b40f8b3f7d4d343cae185604fa918d5b for more details.

        let mut config = self.config.clone();

        let mut styles = Vec::with_capacity(4);
        styles.push(StyleComponent::LineNumbers);
        styles.push(StyleComponent::Snip);
        styles.push(StyleComponent::Header);
        if self.grid {
            styles.push(StyleComponent::Grid);
        }
        config.style_components = StyleComponents::new(&styles);

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
        // XXX: bat's Error type cannot be converted to anyhow::Error due to lack of some type bounds
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
