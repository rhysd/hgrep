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
    fn detect_from_colorterm() -> Option<TermColorSupport> {
        env::var("COLORTERM")
            .map(|v| v.eq_ignore_ascii_case("truecolor") || v.eq_ignore_ascii_case("24bit"))
            .unwrap_or(false)
            .then_some(TermColorSupport::True)
    }

    #[cfg(not(windows))]
    fn detect() -> Self {
        use terminfo::capability::MaxColors;
        use terminfo::Database;

        if let Some(support) = Self::detect_from_colorterm() {
            return support;
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

    #[cfg(windows)]
    fn detect() -> Self {
        if let Some(support) = Self::detect_from_colorterm() {
            return support;
        }

        #[link(name = "ntdll")]
        extern "system" {
            pub fn RtlGetNtVersionNumbers(major: *mut u32, minor: *mut u32, build: *mut u32);
        }
        let (mut major, mut minor, mut build) = (0u32, 0u32, 0u32);
        unsafe {
            RtlGetNtVersionNumbers(&mut major as _, &mut minor as _, &mut build as _);
        }
        build = build & 0xffff;

        // Windows beyond 10.0.15063 supports 24bit colors.
        // https://github.com/Textualize/rich/issues/140
        if major > 10 || major == 10 && (minor > 0 || build >= 15063) {
            TermColorSupport::True
        } else {
            TermColorSupport::Ansi16
        }
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

        #[cfg(not(windows))]
        {
            let _guard = EnvGuard::set_env("COLORTERM", Some("falsecolor"));
            let detected = TermColorSupport::detect();
            assert_ne!(detected, TermColorSupport::True);
        }

        #[cfg(not(windows))]
        {
            let _guard = EnvGuard::set_env("COLORTERM", None);
            let detected = TermColorSupport::detect();
            assert_ne!(detected, TermColorSupport::True);
        }
    }
}
