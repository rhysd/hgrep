use crate::grep::Match;
use anyhow::Result;
use std::fs;
use std::iter::Peekable;
use std::path::PathBuf;

#[derive(Debug)]
pub struct Chunk {
    pub path: PathBuf,
    pub line_numbers: Box<[u64]>,
    pub chunks: Box<[(u64, u64)]>,
    pub contents: Box<[u8]>,
}

impl Chunk {
    fn new(path: PathBuf, lnums: Vec<u64>, chunks: Vec<(u64, u64)>, contents: Vec<u8>) -> Self {
        Self {
            path,
            line_numbers: lnums.into_boxed_slice(),
            chunks: chunks.into_boxed_slice(),
            contents: contents.into_boxed_slice(),
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
    fn calculate_range(&self, match_start: u64, match_end: u64, contents: &[u8]) -> (u64, u64) {
        let start_start = match_start.saturating_sub(self.max_context);
        let start_end = match_start.saturating_sub(self.min_context);
        let end_start = match_end + self.min_context;
        let end_end = match_end + self.max_context;

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
}

impl<I: Iterator<Item = Result<Match>>> Iterator for Chunks<I> {
    type Item = Result<Chunk>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.saw_error {
            return None;
        }

        let Match {
            path,
            mut line_number,
        } = match self.iter.next()? {
            Ok(m) => m,
            Err(e) => {
                self.saw_error = true;
                return Some(Err(e));
            }
        };
        let contents = match fs::read(&path) {
            Ok(vec) => vec,
            Err(err) => {
                self.saw_error = true;
                return Some(Err(err.into()));
            }
        };
        let mut lnums = vec![line_number];
        let mut chunks = Vec::new();

        'chunk: loop {
            let first_match = line_number;
            loop {
                let end = match self.iter.peek() {
                    None => break 'chunk,
                    Some(Err(_)) => {
                        self.saw_error = true;
                        break 'chunk;
                    }
                    Some(Ok(m)) if m.path != path => true,
                    Some(Ok(m)) => m.line_number - line_number >= self.max_context * 2,
                };
                if end {
                    chunks.push(self.calculate_range(first_match, line_number, &contents));
                    break;
                }

                // Go to next match
                line_number = self.iter.next().unwrap().unwrap().line_number;
                lnums.push(line_number);
            }

            // Go to next chunk
            line_number = self.iter.next().unwrap().unwrap().line_number;
            lnums.push(line_number); // first match line of next chunk
        }

        Some(Ok(Chunk::new(path, lnums, chunks, contents)))
    }
}
