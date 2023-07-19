use crate::chunk::File;
use anyhow::Result;
use std::env;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TextWrapMode {
    Char,
    Never,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TermColorSupport {
    True,
    Ansi256,
    Ansi16,
}

impl TermColorSupport {
    fn detect() -> Self {
        use terminfo::capability::MaxColors;
        use terminfo::Database;

        if env::var("COLORTERM")
            .map(|v| v.eq_ignore_ascii_case("truecolor") || v.eq_ignore_ascii_case("24bit"))
            .unwrap_or(false)
        {
            return TermColorSupport::True;
        }

        // Detect Windows Terminal on Windows. WT supports 24bit colors.
        // XXX: `WT_SESSION` should not be used for detecting WT: https://github.com/Textualize/rich/issues/140
        #[cfg(windows)]
        if env::var("WT_SESSION")
            .map(|v| !v.is_empty())
            .unwrap_or(false)
        {
            return TermColorSupport::True;
        }

        if let Ok(info) = Database::from_env() {
            if let Some(MaxColors(colors)) = info.get() {
                if colors < 256 {
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
    pub term_width: u16,
    pub custom_assets: bool,
    pub text_wrap: TextWrapMode,
    pub first_only: bool,
    pub ascii_lines: bool,
}

impl<'main> Default for PrinterOptions<'main> {
    fn default() -> Self {
        use terminal_size::{terminal_size, Width};
        Self {
            tab_width: 4,
            theme: None,
            grid: true,
            background_color: false,
            color_support: TermColorSupport::detect(),
            custom_assets: false,
            term_width: terminal_size().map(|(Width(w), _)| w).unwrap_or(80), // Note: `tput` returns 80 when tty is not found
            text_wrap: TextWrapMode::Char,
            first_only: false,
            ascii_lines: false,
        }
    }
}

// Trait to replace printer implementation for unit tests
pub trait Printer {
    fn print(&self, file: File) -> Result<()>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test::EnvGuard;

    #[test]
    fn test_detect_true_color_from_env() {
        {
            let _guard = EnvGuard::set_env("COLORTERM", Some("truecolor"));
            let detected = TermColorSupport::detect();
            assert_eq!(detected, TermColorSupport::True);
        }

        {
            let _guard = EnvGuard::set_env("COLORTERM", Some("24bit"));
            let detected = TermColorSupport::detect();
            assert_eq!(detected, TermColorSupport::True);
        }

        {
            let _guard = EnvGuard::set_env("COLORTERM", Some("falsecolor"));
            let detected = TermColorSupport::detect();
            assert_ne!(detected, TermColorSupport::True);
        }

        {
            let _guard = EnvGuard::set_env("COLORTERM", None);
            let detected = TermColorSupport::detect();
            assert_ne!(detected, TermColorSupport::True);
        }

        // Check detecting Windows Terminal. This cannot be separate test case because it tweaks environment variables.
        // Note that cargo runs tests in parallel.
        #[cfg(windows)]
        {
            let _guard_colorterm = EnvGuard::set_env("COLORTERM", None);
            let _guard_wt_session =
                EnvGuard::set_env("WT_SESSION", Some("13045d8c-6d2d-4d92-b0e9-dfd7bc8bc8f2"));
            let detected = TermColorSupport::detect();
            assert_eq!(detected, TermColorSupport::True);
        }
    }
}
