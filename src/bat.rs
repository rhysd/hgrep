use crate::chunk::File;
use crate::printer::{Printer, PrinterOptions, TermColorSupport, TextWrapMode};
use anyhow::{Error, Result};
use bat::assets::HighlightingAssets;
use bat::config::{Config, VisibleLines};
use bat::controller::Controller;
use bat::input::Input;
use bat::line_range::{HighlightedLineRanges, LineRange, LineRanges};
use bat::style::{StyleComponent, StyleComponents};
use bat::WrappingMode;
use std::env;
use std::fmt;
use std::path::PathBuf;
use std::sync::Mutex;

#[derive(Debug)]
pub struct BatPrintError {
    path: PathBuf,
    cause: Option<String>,
}

impl std::error::Error for BatPrintError {}

impl fmt::Display for BatPrintError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Could not print file {:?}", &self.path)?;
        if let Some(cause) = &self.cause {
            write!(f, ". Caused by: {}", cause)?;
        }
        Ok(())
    }
}

// Brought from bat/src/bin/bat/directories.rs dde770aa210ab9eeb5469e152cec6fcaab374d84
fn get_cache_dir() -> Option<PathBuf> {
    // on all OS prefer BAT_CACHE_PATH if set
    if let Some(path) = env::var_os("BAT_CACHE_PATH") {
        return Some(PathBuf::from(path));
    }

    #[cfg(target_os = "macos")]
    let dir = env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| dirs::home_dir().map(|d| d.join(".cache")));

    #[cfg(not(target_os = "macos"))]
    let dir = dirs::cache_dir();

    dir.map(|d| d.join("bat"))
}

pub struct BatPrinter<'main> {
    opts: PrinterOptions<'main>,
    config: Config<'main>,
    assets: HighlightingAssets,
}

impl<'main> BatPrinter<'main> {
    pub fn new(opts: PrinterOptions<'main>) -> Self {
        let styles = if opts.grid {
            &[
                StyleComponent::LineNumbers,
                StyleComponent::Snip,
                StyleComponent::HeaderFilename,
                StyleComponent::Grid,
            ][..]
        } else {
            &[
                StyleComponent::LineNumbers,
                StyleComponent::Snip,
                StyleComponent::HeaderFilename,
            ][..]
        };

        let wrapping_mode = match opts.text_wrap {
            TextWrapMode::Char => WrappingMode::Character,
            TextWrapMode::Never => WrappingMode::NoWrapping(true),
        };

        let mut config = Config {
            colored_output: true,
            term_width: opts.term_width as usize,
            style_components: StyleComponents::new(styles),
            tab_width: opts.tab_width,
            true_color: opts.color_support == TermColorSupport::True,
            wrapping_mode,
            ..Default::default()
        };

        if let Some(theme) = &opts.theme {
            config.theme = theme.to_string();
        } else if opts.color_support == TermColorSupport::Ansi16 {
            config.theme = "ansi".to_string();
        }

        let assets = if opts.custom_assets {
            get_cache_dir()
                .and_then(|path| HighlightingAssets::from_cache(&path).ok())
                .unwrap_or_else(HighlightingAssets::from_binary)
        } else {
            HighlightingAssets::from_binary()
        };

        Self {
            opts,
            assets,
            config,
        }
    }

    pub fn themes(&self) -> impl Iterator<Item = &str> {
        self.assets.themes()
    }

    pub fn list_themes(&mut self) -> Result<()> {
        let sample = File::sample_file();
        let mut themes: Vec<_> = self.assets.themes().collect();
        themes.sort_unstable();
        for theme in themes.into_iter() {
            println!("\x1b[1m{:?}\x1b[0m", theme);
            self.config.theme = theme.to_string();
            self.print(sample.clone())?;
            println!();
        }
        Ok(())
    }

    pub fn print(&self, file: File) -> Result<()> {
        if file.chunks.is_empty() || file.line_matches.is_empty() {
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
            .map(|(s, e)| LineRange::new(*s as usize, *e as usize));
        let ranges = if self.opts.first_only {
            ranges.take(1).collect()
        } else {
            ranges.collect()
        };
        config.visible_lines = VisibleLines::Ranges(LineRanges::from(ranges));

        let input =
            Input::from_reader(Box::new(file.contents.as_ref())).with_name(Some(&file.path));

        let ranges = file
            .line_matches
            .iter()
            .map(|m| {
                let n = m.line_number as usize;
                LineRange::new(n, n)
            })
            .collect();

        config.highlighted_lines = HighlightedLineRanges(LineRanges::from(ranges));

        if !self.opts.grid {
            print!("\n\n"); // Empty lines as files separator
        }

        let controller = Controller::new(&config, &self.assets);

        // Note: controller.run() returns true when no error
        // XXX: bat's Error type cannot be converted to anyhow::Error since it does not implement Sync
        match controller.run(vec![input], None) {
            Ok(true) => Ok(()),
            Ok(false) => Err(Error::new(BatPrintError {
                path: file.path,
                cause: None,
            })),
            Err(err) => Err(Error::new(BatPrintError {
                path: file.path,
                cause: Some(format!("{}", err)),
            })),
        }
    }
}

impl<'main> Printer for Mutex<BatPrinter<'main>> {
    fn print(&self, file: File) -> Result<()> {
        self.lock().unwrap().print(file)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunk::LineMatch;

    fn sample_file() -> File {
        let path = PathBuf::from("test.rs");
        let lmats = vec![LineMatch::lnum(1)];
        let chunks = vec![(1, 2)];
        let contents = "fn main() {\n    println!(\"hello\");\n}\n"
            .as_bytes()
            .to_vec();
        File::new(path, lmats, chunks, contents)
    }

    #[test]
    fn test_print_default() {
        let p = BatPrinter::new(PrinterOptions::default());
        let f = sample_file();
        p.print(f).unwrap();
    }

    #[test]
    fn test_print_with_flags() {
        let opts = PrinterOptions {
            tab_width: 2,
            theme: Some("Nord"),
            grid: false,
            text_wrap: TextWrapMode::Never,
            ..Default::default()
        };
        let p = BatPrinter::new(opts);
        let f = sample_file();
        p.print(f).unwrap();
    }

    #[test]
    fn test_print_nothing() {
        let p = BatPrinter::new(PrinterOptions::default());
        let f = File::new(PathBuf::from("x.txt"), vec![], vec![], vec![]);
        p.print(f).unwrap();
    }
}
