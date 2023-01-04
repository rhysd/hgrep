use crate::chunk::Files;
use anyhow::{Error, Result};
use std::borrow::Cow;
use std::ffi::OsString;
use std::fmt;
use std::io::BufRead;
use std::path::PathBuf;
use std::str;

#[cfg(target_os = "windows")]
fn bytes_to_os_string(bytes: &[u8]) -> OsString {
    // This does not allow invalid sequence as UTF-8. Invalid characters are replaced with U+FFFD
    String::from_utf8_lossy(bytes).to_string().into()
}

#[cfg(not(target_os = "windows"))]
fn bytes_to_os_string(bytes: &[u8]) -> OsString {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;
    OsStr::from_bytes(bytes).into()
}

#[derive(Debug)]
pub struct ParseError {
    line: String,
    message: Cow<'static, str>,
}

impl ParseError {
    fn err<T>(line: Vec<u8>, msg: impl Into<Cow<'static, str>>) -> Result<T> {
        let line = match String::from_utf8(line) {
            Ok(s) => s,
            Err(e) => String::from_utf8_lossy(e.as_bytes()).to_string(),
        };
        Err(Error::new(Self {
            line,
            message: msg.into(),
        }))
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}: Could not parse line {:?}. Did you forget adding -n and/or -H to the grep command?",
            self.message, self.line
        )
    }
}

impl std::error::Error for ParseError {}

#[derive(Debug, PartialEq, Eq)]
pub struct GrepMatch {
    pub path: PathBuf,
    pub line_number: u64,
    // Byte offsets of start/end positions within the line
    pub ranges: Vec<(usize, usize)>,
}

pub struct GrepLines<R: BufRead> {
    reader: R,
    prev_lnum: u64,
}

impl<R: BufRead> GrepLines<R> {
    pub fn chunks_per_file(self, min: u64, max: u64) -> Files<Self> {
        Files::new(self, min, max)
    }
}

impl<R: BufRead> Iterator for GrepLines<R> {
    type Item = Result<GrepMatch>;

    fn next(&mut self) -> Option<Self::Item> {
        let mut line = Vec::new();
        self.reader.read_until(b'\n', &mut line).unwrap();
        if line.is_empty() {
            return None;
        }

        // {path}:{lnum}:{line}...
        let mut split = line.splitn(3, |&b| b == b':');
        let (path, lnum) = match (split.next(), split.next(), split.next()) {
            (Some(p), Some(l), Some(_)) if p.is_empty() || l.is_empty() => {
                return Some(ParseError::err(line, "Path or line number is empty"));
            }
            (Some(p), Some(l), Some(_)) => (p, l),
            _ => return Some(ParseError::err(line, "Path or line number is missing")),
        };

        match str::from_utf8(lnum).ok().and_then(|s| s.parse().ok()) {
            Some(lnum) if lnum <= self.prev_lnum => self.next(), // Ignore same lines are reported. This happens with `rg --vimgrep` (#13)
            Some(lnum) => {
                let mat = GrepMatch {
                    path: PathBuf::from(bytes_to_os_string(path)),
                    line_number: lnum,
                    ranges: vec![], // Regions are not supported
                };
                self.prev_lnum = lnum;
                Some(Ok(mat))
            }
            None => Some(ParseError::err(
                line,
                "Could not parse line number as unsigned integer",
            )),
        }
    }
}

pub trait BufReadExt: BufRead + Sized {
    fn grep_lines(self) -> GrepLines<Self>;
}

impl<R: BufRead> BufReadExt for R {
    fn grep_lines(self) -> GrepLines<Self> {
        GrepLines {
            reader: self,
            prev_lnum: 0,
        }
    }
}

#[test]
fn test_read_ok() {
    let input = [
        "/path/to/foo.txt:1:    hello",
        "/path/to/bar.txt:100:    bye",
        "/path/to/bar.txt:110:    this : line : include : colon",
    ]
    .join("\n")
    .into_bytes();

    let output: Vec<_> = input.grep_lines().collect::<Result<_>>().unwrap();

    let expected = vec![
        GrepMatch {
            path: PathBuf::from("/path/to/foo.txt"),
            line_number: 1,
            ranges: vec![],
        },
        GrepMatch {
            path: PathBuf::from("/path/to/bar.txt"),
            line_number: 100,
            ranges: vec![],
        },
        GrepMatch {
            path: PathBuf::from("/path/to/bar.txt"),
            line_number: 110,
            ranges: vec![],
        },
    ];

    assert_eq!(output, expected);
}

#[test]
fn test_read_error() {
    let input = [
        "",
        "/path/to/foo.txt:   foo",
        "123:   foo",
        "/path/to/foo.txt:   hello : world",
        ":",
        "::",
    ]
    .join("\n")
    .into_bytes();

    let msgs: Vec<_> = input
        .grep_lines()
        .map(|r| format!("{}", r.unwrap_err()))
        .collect();

    let expected = &[
        "Path or line number is missing:",
        "Path or line number is missing:",
        "Path or line number is missing:",
        "Could not parse line number as unsigned integer:",
        "Path or line number is missing:",
        "Path or line number is empty:",
    ];

    assert_eq!(msgs.len(), expected.len());

    for (got, expected) in msgs.iter().zip(expected.iter()) {
        assert!(
            got.contains(expected),
            "expected {:?} is included in {:?}",
            expected,
            got
        );
    }
}

#[test]
fn test_same_line_is_repeated() {
    // Regression test for #13. This may happen with `rg --vimgrep`
    let input = [
        "/path/to/foo.txt:1:1:bye",
        "/path/to/foo.txt:1:2:bye",
        "/path/to/foo.txt:1:3:bye",
    ]
    .join("\n")
    .into_bytes();

    let output: Vec<_> = input.grep_lines().collect::<Result<_>>().unwrap();

    let expected = vec![GrepMatch {
        path: PathBuf::from("/path/to/foo.txt"),
        line_number: 1,
        ranges: vec![],
    }];

    assert_eq!(output, expected);
}
