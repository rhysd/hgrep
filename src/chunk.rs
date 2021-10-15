use crate::grep::Match;
use anyhow::Result;
use std::fs;
use std::iter::Peekable;
use std::path::PathBuf;

#[derive(Debug)]
pub struct Chunk {
    pub path: PathBuf,
    pub line_numbers: Box<[u64]>,
    pub range_start: u64,
    pub range_end: u64,
}

impl Chunk {
    fn new(path: PathBuf, lnums: Vec<u64>, start: u64, end: u64) -> Self {
        Self {
            path,
            line_numbers: lnums.into_boxed_slice(),
            range_start: start,
            range_end: end,
        }
    }
}

pub struct Chunks<I: Iterator> {
    iter: Peekable<I>,
    min_context: u64,
    max_context: u64,
    saw_error: bool,
}

impl<I: Iterator<Item = Result<Match>>> Chunks<I> {
    pub fn new(iter: I, min_context: u64, max_context: u64) -> Self {
        Self {
            iter: iter.peekable(),
            min_context,
            max_context,
            saw_error: false,
        }
    }
}

impl<I: Iterator<Item = Result<Match>>> Chunks<I> {
    fn calculate_range(&self, lnums: &[u64], contents: &[u8]) -> (u64, u64) {
        let start = lnums[0];
        let start_start = start.saturating_sub(self.max_context);
        let start_end = start.saturating_sub(self.min_context);
        let end = lnums[lnums.len() - 1];
        let end_start = end + self.min_context;
        let end_end = end + self.max_context;

        let mut range_start = start_start;
        let mut range_end = end_end;

        for (idx, line) in contents.split(|&b| b == b'\n').enumerate() {
            let lnum = idx as u64 + 1;
            if end_end < lnum {
                break;
            }

            let is_start = start_start <= lnum && lnum < start_end;
            let is_end = end_start < lnum && lnum <= end_end;
            if !is_start && !is_end {
                continue;
            }

            if line.is_empty() {
                if is_start {
                    range_start = lnum + 1;
                }
                if is_end {
                    range_end = lnum.saturating_sub(1);
                    break;
                }
            }
        }

        (range_start, range_end)
    }

    fn next_chunk(&mut self, path: PathBuf, mut line_number: u64) -> Option<Result<Chunk>> {
        let mut lnums = vec![line_number];
        let contents = fs::read(&path).unwrap(); // TODO

        loop {
            let end = match self.iter.peek() {
                None | Some(Err(_)) => true,
                Some(Ok(m)) if m.path != path => true,
                Some(Ok(m)) => m.line_number - line_number >= self.max_context * 2,
            };
            if end {
                let (start, end) = self.calculate_range(&lnums, &contents);
                return Some(Ok(Chunk::new(path, lnums, start, end)));
            }

            line_number = self.iter.next().unwrap().unwrap().line_number;
            lnums.push(line_number);
        }
    }
}

impl<I: Iterator<Item = Result<Match>>> Iterator for Chunks<I> {
    type Item = Result<Chunk>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.saw_error {
            return None;
        }

        let Match { path, line_number } = match self.iter.next()? {
            Ok(m) => m,
            Err(e) => {
                self.saw_error = true;
                return Some(Err(e));
            }
        };

        self.next_chunk(path, line_number)
    }
}
