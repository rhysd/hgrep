use crate::read::Match;
use anyhow::Result;
use std::iter::Peekable;
use std::path::PathBuf;

#[derive(Debug)]
pub struct Chunk {
    pub path: PathBuf,
    pub line_numbers: Box<[u64]>,
}

impl Chunk {
    fn new(path: PathBuf, lnums: Vec<u64>) -> Self {
        Self {
            path,
            line_numbers: lnums.into_boxed_slice(),
        }
    }
}

pub struct Chunks<I: Iterator> {
    iter: Peekable<I>,
    context_lines: u64,
    saw_error: bool,
}

impl<I: Iterator<Item = Result<Match>>> Chunks<I> {
    pub fn new(iter: I, context_lines: u64) -> Self {
        Self {
            iter: iter.peekable(),
            context_lines,
            saw_error: false,
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
        let mut lnums = vec![line_number];

        loop {
            let end = match self.iter.peek() {
                None | Some(Err(_)) => true,
                Some(Ok(m)) if m.path != path => true,
                Some(Ok(m)) => m.line_number - line_number >= self.context_lines * 2,
            };
            if end {
                return Some(Ok(Chunk::new(path, lnums)));
            }

            lnums.push(self.iter.next().unwrap().unwrap().line_number);
        }
    }
}
