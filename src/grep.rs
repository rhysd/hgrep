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
}

impl<R: BufRead> GrepLines<R> {
    pub fn chunks_per_file(self, min: u64, max: u64) -> Files<Self> {
        Files::new(self, min, max)
    }
}

fn parse_line(line: Vec<u8>) -> Result<GrepMatch> {
    // {path}:{lnum}:{line}...
    let mut split = line.splitn(3, |&b| b == b':');
    let (path, lnum) = match (split.next(), split.next(), split.next()) {
        (Some(p), Some(l), Some(_)) if p.is_empty() || l.is_empty() => {
            return ParseError::err(line, "Path or line number is empty")
        }
        (Some(p), Some(l), Some(_)) => (p, l),
        _ => return ParseError::err(line, "Path or line number is missing"),
    };
    match str::from_utf8(lnum).ok().and_then(|s| s.parse().ok()) {
        Some(lnum) => Ok(GrepMatch {
            path: PathBuf::from(bytes_to_os_string(path)),
            line_number: lnum,
            ranges: vec![], // Regions are not supported
        }),
        None => ParseError::err(line, "Could not parse line number as unsigned integer"),
    }
}

impl<R: BufRead> Iterator for GrepLines<R> {
    type Item = Result<GrepMatch>;

    fn next(&mut self) -> Option<Self::Item> {
        let mut buf = Vec::new();
        self.reader.read_until(b'\n', &mut buf).unwrap();
        if buf.is_empty() {
            return None;
        }
        Some(parse_line(buf))
    }
}

pub trait BufReadExt: BufRead + Sized {
    fn grep_lines(self) -> GrepLines<Self>;
}

impl<R: BufRead> BufReadExt for R {
    fn grep_lines(self) -> GrepLines<Self> {
        GrepLines { reader: self }
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

    let expected = &[
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

    assert_eq!(&output, expected);
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
