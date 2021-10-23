use crate::chunk::File;
use anyhow::Result;
use std::env;
use term::terminfo::TermInfo;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TermColorSupport {
    True,
    Ansi256,
    Ansi16,
}

impl TermColorSupport {
    fn detect() -> Self {
        if env::var("COLORTERM")
            .ok()
            .map(|v| v.eq_ignore_ascii_case("truecolor"))
            .unwrap_or(false)
        {
            return TermColorSupport::True;
        }

        if let Ok(info) = TermInfo::from_env() {
            if let Some(colors) = info.numbers.get("colors") {
                if *colors < 256 {
                    return TermColorSupport::Ansi16;
                }
            }
        }

        // Assume 256 colors by default (I'm not sure this is correct)
        TermColorSupport::Ansi256
    }
}

pub struct PrinterOptions<'main> {
    pub tab_width: usize,
    pub theme: Option<&'main str>,
    pub grid: bool,
    pub background_color: bool,
    pub color_support: TermColorSupport,
}

impl<'main> Default for PrinterOptions<'main> {
    fn default() -> Self {
        Self {
            tab_width: 4,
            theme: None,
            grid: true,
            background_color: false,
            color_support: TermColorSupport::detect(),
        }
    }
}

// Trait to replace printer implementation for unit tests
pub trait Printer {
    fn print(&self, file: File) -> Result<()>;
}
