use crate::grep::Match;
use anyhow::Result;
use std::fs;
use std::iter::Peekable;
use std::path::PathBuf;

#[derive(Debug)]
pub struct File {
    pub path: PathBuf,
    pub line_numbers: Box<[u64]>,
    pub chunks: Box<[(u64, u64)]>,
    pub contents: Box<[u8]>,
}

impl File {
    fn new(path: PathBuf, lnums: Vec<u64>, chunks: Vec<(u64, u64)>, contents: Vec<u8>) -> Self {
        Self {
            path,
            line_numbers: lnums.into_boxed_slice(),
            chunks: chunks.into_boxed_slice(),
            contents: contents.into_boxed_slice(),
        }
    }
}

pub struct Files<I: Iterator> {
    iter: Peekable<I>,
    min_context: u64,
    max_context: u64,
    saw_error: bool,
}

impl<I: Iterator> Files<I> {
    pub fn new(iter: I, min_context: u64, max_context: u64) -> Self {
        Self {
            iter: iter.peekable(),
            min_context,
            max_context,
            saw_error: false,
        }
    }
}

struct Line<'a>(&'a [u8], u64);

impl<I: Iterator<Item = Result<Match>>> Files<I> {
    fn calculate_chunk_range<'contents>(
        &self,
        match_start: u64,
        match_end: u64,
        lines: &mut impl Iterator<Item = Line<'contents>>,
    ) -> (u64, u64) {
        let before_start = match_start.saturating_sub(self.max_context);
        let before_end = match_start.saturating_sub(self.min_context);
        let after_start = match_end + self.min_context;
        let after_end = match_end + self.max_context;

        let mut range_start = before_start;
        let mut range_end = after_end;

        for Line(line, lnum) in lines {
            assert!(
                lnum <= after_end,
                "line {} is over its end of chunk {}",
                lnum,
                after_end
            );

            let is_start = before_start <= lnum && lnum < before_end;
            let is_end = after_start < lnum && lnum <= after_end;
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

            if lnum == after_end {
                break; // Do not consume next line from `lines` for next chunk
            }
        }

        (range_start, range_end)
    }
}

impl<I: Iterator<Item = Result<Match>>> Iterator for Files<I> {
    type Item = Result<File>;

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
        // Assumes that matched lines are sorted by source location
        let mut lines = contents
            .split(|b| *b == b'\n')
            .enumerate()
            .map(|(n, l)| Line(l, n as u64 + 1));
        let mut lnums = vec![line_number];
        let mut chunks = Vec::new();

        'chunks: loop {
            let first_match_line = line_number;

            loop {
                match self.iter.peek() {
                    None => break 'chunks,
                    Some(Err(_)) => {
                        self.saw_error = true;
                        break 'chunks;
                    }
                    Some(Ok(m)) if m.path != path => break 'chunks,
                    Some(Ok(m)) if m.line_number - line_number >= self.max_context * 2 => {
                        chunks.push(self.calculate_chunk_range(
                            first_match_line,
                            line_number,
                            &mut lines,
                        ));
                        break; // End of chunk
                    }
                    Some(Ok(m)) => {
                        // Next match
                        line_number = m.line_number;
                        lnums.push(line_number);
                        self.iter.next().unwrap().unwrap();
                    }
                }
            }

            // Go to next chunk
            line_number = self.iter.next().unwrap().unwrap().line_number;
            lnums.push(line_number); // first match line of next chunk
        }

        Some(Ok(File::new(path, lnums, chunks, contents)))
    }
}
