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

    // > The TERM environment variable is its primary source of information. Its value is expected to follow the terminfo(5) and
    // > termcap(5) convention (laid out in term(7)) of a root name followed by zero or more hyphen-separated feature suffixes
    // > (such as, for example, “teken-256color”). The TerminalCapabilities class only cares about the root name, which it takes
    // > to denote a family of terminal types (e.g. “putty”), and whether there are “-24bit”, “-truecolor”, “-256color”, or
    // > “-square” suffixes in the value. Other feature suffixes are ignored.
    //
    // https://jdebp.uk/Softwares/nosh/guide/TerminalCapabilities.html
    fn detect_from_term() -> Option<Self> {
        env::var("TERM").ok().and_then(|v| {
            if v.ends_with("-truecolor") || v.ends_with("-24bit") {
                Some(Self::True)
            } else if v.ends_with("-256color") || v.ends_with("-square") {
                Some(Self::Ansi256)
            } else {
                None
            }
        })
    }

    #[cfg(not(windows))]
    fn detect() -> Self {
        use terminfo::capability::MaxColors;
        use terminfo::Database;

        if let Some(support) = Self::detect_from_colorterm().or_else(Self::detect_from_term) {
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
        use windows_version::OsVersion;

        if let Some(support) = Self::detect_from_colorterm().or_else(Self::detect_from_term) {
            return support;
        }

        // Windows 10.0.15063 or later supports 24-bit colors (true colors).
        // https://github.com/Textualize/rich/issues/140
        //
        // Note that Windows 10.0.15063 is Windows 10 1703, which was released on April 5, 2017 so it is pretty old.
        if OsVersion::current() >= OsVersion::new(10, 0, 0, 15063) {
            Self::True
        } else {
            Self::Ansi16
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
        struct Envs {
            colorterm: Option<&'static str>,
            term: Option<&'static str>,
            want: TermColorSupport,
        }

        for test in [
            Envs {
                colorterm: Some("truecolor"),
                term: None,
                want: TermColorSupport::True,
            },
            Envs {
                colorterm: Some("24bit"),
                term: None,
                want: TermColorSupport::True,
            },
            Envs {
                colorterm: Some(""), // Setting some other values indicates 16 colors support
                term: None,
                want: TermColorSupport::Ansi16,
            },
            Envs {
                colorterm: None,
                term: Some("xterm-truecolor"),
                want: TermColorSupport::True,
            },
            Envs {
                colorterm: None,
                term: Some("xterm-24bit"),
                want: TermColorSupport::True,
            },
            Envs {
                colorterm: None,
                term: Some("xterm-256color"),
                want: TermColorSupport::Ansi256,
            },
            Envs {
                colorterm: None,
                term: Some("xterm-square"),
                want: TermColorSupport::Ansi256,
            },
            Envs {
                colorterm: None,
                term: Some("xterm-unknown"),
                #[cfg(not(windows))]
                want: TermColorSupport::Ansi256,
                #[cfg(windows)]
                want: TermColorSupport::True,
            },
            Envs {
                colorterm: Some("truecolor"), // Checking COLORTERM is preceded
                term: Some("xterm-256color"),
                want: TermColorSupport::True,
            },
            Envs {
                colorterm: Some(""),
                term: Some("xterm-256color"),
                want: TermColorSupport::Ansi16,
            },
            Envs {
                colorterm: None,
                term: None,
                #[cfg(not(windows))]
                want: TermColorSupport::Ansi256,
                #[cfg(windows)]
                want: TermColorSupport::True,
            },
        ] {
            let mut guard = EnvGuard::default();
            let Envs {
                colorterm,
                term,
                want,
            } = test;
            guard.set_env("COLORTERM", colorterm);
            guard.set_env("TERM", term);
            let detected = TermColorSupport::detect();
            assert_eq!(detected, want, "COLORTERM={colorterm:?} and TERM={term:?}",);
        }
    }
}
