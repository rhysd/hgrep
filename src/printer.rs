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
    fn detect_from_colorterm() -> Option<Self> {
        // > The existence of this variable signifies extra colour capabilities of some sort. If it has the value “truecolor”
        // > or the value “24bit” then the terminal is taken to understand ISO 8613-6/ITU T.416 Direct colour SGR 38 and SGR
        // > 48 control sequences. Otherwise it indicates that the terminal understands the additional 8 (de facto) standard
        // > colours (from AIXTerm) set by SGR 90–97 and SGR 100–107.
        //
        // http://jdebp.uk/Softwares/nosh/guide/TerminalCapabilities.html
        env::var("COLORTERM").ok().map(|v| {
            if v.eq_ignore_ascii_case("truecolor") || v.eq_ignore_ascii_case("24bit") {
                Self::True
            } else {
                Self::Ansi16
            }
        })
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
                    return Self::Ansi16;
                }
            }
        }

        // Assume 256 colors by default (I'm not sure this is correct)
        Self::Ansi256
    }

    #[cfg(windows)]
    fn detect() -> Self {
        // Windows 10.0.15063 or later supports 24-bit colors (true colors).
        // https://github.com/Textualize/rich/issues/140
        //
        // hgrep doesn't detect it because it is very messy to get Windows OS version. WIN32's official sysinfoapi.h APIs such
        // as `GetVersion`, `GetVersionEx`, `VerifyVersionInfo`, ... don't return OS version of the current system. Please read
        // the 'Remarks' section in the following document. `RtlGetNtVersionNumbers` works but ntdll.dll does not always exist
        // and can cause a link error.
        //
        // https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-verifyversioninfoa
        //
        // Windows 10.0.15063 is Windows 10 1703, which was released on April 5, 2017 so it is pretty old.
        // Setting 'ansi' theme manually should still work for those who are using older Windows due to some reason.
        Self::detect_from_colorterm().unwrap_or(Self::True)
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
            let _guard = EnvGuard::set_env("COLORTERM", Some("someothervalue"));
            let detected = TermColorSupport::detect();
            assert_eq!(detected, TermColorSupport::Ansi16);
        }

        {
            let _guard = EnvGuard::set_env("COLORTERM", None);
            let detected = TermColorSupport::detect();
            if cfg!(windows) {
                assert_eq!(detected, TermColorSupport::True);
            } else {
                assert_ne!(detected, TermColorSupport::True);
            }
        }
    }
}
