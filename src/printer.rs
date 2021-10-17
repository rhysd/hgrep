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
