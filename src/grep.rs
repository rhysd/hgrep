use crate::chunk::Chunks;
use anyhow::{Error, Result};
use std::borrow::Cow;
use std::ffi::OsString;
use std::fmt;
use std::io::BufRead;
use std::path::PathBuf;
use std::str;

#[cfg(target_os = "windows")]
fn bytes_to_os_string(bytes: &[u8]) -> OsString {
    use std::os::windows::prelude::*;
    OsString::from_wide(bytes)
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

pub struct Match {
    pub path: PathBuf,
    pub line_number: u64,
}

pub struct GrepLines<R: BufRead> {
    reader: R,
}

impl<R: BufRead> GrepLines<R> {
    pub fn chunks(self, context_lines: u64) -> Chunks<Self> {
        Chunks::new(self, context_lines)
    }
}

fn parse_line(line: Vec<u8>) -> Result<Match> {
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
        Some(lnum) => Ok(Match {
            path: PathBuf::from(bytes_to_os_string(path)),
            line_number: lnum,
        }),
        None => ParseError::err(line, "Could not parse line number as unsigned integer"),
    }
}

impl<R: BufRead> Iterator for GrepLines<R> {
    type Item = Result<Match>;

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
