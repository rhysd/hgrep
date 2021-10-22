use crate::grep::Match;
use anyhow::Result;
use memchr::{memchr_iter, Memchr};
use pathdiff::diff_paths;
use std::cmp;
use std::env;
use std::fs;
use std::iter::Peekable;
use std::path::PathBuf;

#[cfg_attr(test, derive(Debug, PartialEq))]
#[derive(Clone)]
pub struct File {
    pub path: PathBuf,
    pub line_numbers: Box<[u64]>,
    pub chunks: Box<[(u64, u64)]>,
    pub contents: Box<[u8]>,
}

impl File {
    pub fn new(path: PathBuf, lnums: Vec<u64>, chunks: Vec<(u64, u64)>, contents: Vec<u8>) -> Self {
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
    cwd: Option<PathBuf>,
}

impl<I: Iterator> Files<I> {
    pub fn new(iter: I, min_context: u64, max_context: u64) -> Self {
        Self {
            iter: iter.peekable(),
            min_context,
            max_context,
            saw_error: false,
            cwd: env::current_dir().ok(),
        }
    }
}

pub struct Line<'a>(pub &'a [u8], pub u64);
struct Lines<'a> {
    lnum: usize,
    prev: usize,
    buf: &'a [u8],
    iter: Memchr<'a>,
}
impl<'a> Lines<'a> {
    pub fn new(buf: &'a [u8]) -> Self {
        Self {
            lnum: 1,
            prev: 0,
            buf,
            iter: memchr_iter(b'\n', buf),
        }
    }
}
impl<'a> Iterator for Lines<'a> {
    type Item = Line<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        if let Some(idx) = self.iter.next() {
            let lnum = self.lnum;
            let end = idx + 1;
            let mut line = &self.buf[self.prev..end - 1];
            if line.ends_with(b"\r") {
                line = &line[..line.len() - 1];
            }
            self.prev = end;
            self.lnum += 1;
            Some(Line(line, lnum as u64))
        } else if self.prev == self.buf.len() {
            None
        } else {
            let mut line = &self.buf[self.prev..];
            if line.ends_with(b"\n") {
                line = &line[..line.len() - 1];
            }
            if line.ends_with(b"\r") {
                line = &line[..line.len() - 1];
            }
            self.prev = self.buf.len();
            Some(Line(line, self.lnum as u64))
        }
    }
}

impl<I: Iterator<Item = Result<Match>>> Files<I> {
    fn calculate_chunk_range<'contents>(
        &self,
        match_start: u64,
        match_end: u64,
        lines: &mut impl Iterator<Item = Line<'contents>>,
    ) -> (u64, u64) {
        let before_start = cmp::max(match_start.saturating_sub(self.max_context), 1);
        let before_end = cmp::max(match_start.saturating_sub(self.min_context), 1);
        let after_start = match_end + self.min_context;
        let after_end = match_end + self.max_context;

        let mut range_start = before_start;
        let mut range_end = after_end;
        let mut last_lnum = None;

        for Line(line, lnum) in lines {
            last_lnum = Some(lnum);
            assert!(lnum <= after_end, "line {} > chunk {}", lnum, after_end);

            let in_before = before_start <= lnum && lnum < before_end;
            let in_after = after_start < lnum && lnum <= after_end;
            if line.is_empty() {
                if in_before {
                    range_start = lnum + 1;
                }
                if in_after {
                    range_end = lnum.saturating_sub(1);
                    break;
                }
            }

            if lnum == after_end {
                break; // Do not consume next line from `lines` for next chunk
            }
        }
        if let Some(n) = last_lnum {
            range_end = cmp::min(range_end, n); // Make end of chunk fit to end of file
        }

        (range_start, range_end)
    }

    fn relative_path(&self, path: PathBuf) -> PathBuf {
        if !path.is_relative() {
            if let Some(cwd) = &self.cwd {
                if let Some(diff) = diff_paths(&path, cwd) {
                    return diff;
                }
            }
        }
        path
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
        let mut lines = Lines::new(&contents);
        let mut lnums = vec![line_number];
        let mut chunks = Vec::new();

        'chunks: loop {
            let first_match_line = line_number;

            enum State {
                NextMatch,
                EndOfFile,
                EndOfChunk,
                Error,
            }

            loop {
                let peeked = match self.iter.peek() {
                    None => State::EndOfFile,
                    Some(Err(_)) => State::Error,
                    Some(Ok(m)) if m.path != path => State::EndOfFile,
                    Some(Ok(m)) if m.line_number - line_number >= self.max_context * 2 => {
                        State::EndOfChunk
                    }
                    Some(Ok(_)) => State::NextMatch,
                };

                // Actions for each states
                match peeked {
                    State::EndOfFile | State::EndOfChunk => chunks.push(
                        self.calculate_chunk_range(first_match_line, line_number, &mut lines),
                    ),
                    State::Error => self.saw_error = true,
                    State::NextMatch => {
                        // Next match
                        line_number = self.iter.next().unwrap().unwrap().line_number;
                        lnums.push(line_number);
                    }
                }

                // Transition of each states
                match peeked {
                    State::EndOfFile | State::Error => break 'chunks,
                    State::EndOfChunk => break,
                    State::NextMatch => continue,
                }
            }

            // Go to next chunk
            line_number = self.iter.next().unwrap().unwrap().line_number;
            lnums.push(line_number); // first match line of next chunk
        }

        if chunks.is_empty() {
            assert!(lnums.is_empty());
            return None;
        }

        let path = self.relative_path(path);
        Some(Ok(File::new(path, lnums, chunks, contents)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test;
    use anyhow::Error;
    use std::fmt;
    use std::path::Path;

    fn test_success_case(inputs: &[&str]) {
        let dir = Path::new("testdata").join("chunk");

        let matches = test::read_all_matches(&dir, inputs);
        let got: Vec<_> = Files::new(matches.into_iter(), 3, 6)
            .collect::<Result<_>>()
            .unwrap();
        let expected = test::read_all_expected_chunks(&dir, inputs);

        assert_eq!(got, expected);
    }

    macro_rules! success_case_tests {
        {$($name:ident($tests:expr);)+} => {
            $(
                #[test]
                fn $name() {
                    test_success_case(&$tests);
                }
            )+
        }
    }

    success_case_tests! {
        // One chunk
        test_single_max(["single_max"]);
        test_before_and_after(["before_and_after"]);
        test_before(["before"]);
        test_after(["after"]);
        test_edges(["edges"]);
        test_edges_out(["edges_out"]);
        test_blank_min(["blank_min"]);
        test_blank_min_max(["blank_min_max"]);
        test_blank_min_edge(["blank_min_edge"]);
        test_blank_max_bottom(["blank_max_bottom"]);
        test_blank_max_top(["blank_max_top"]);
        test_all_blank(["all_blank"]);
        test_top_file_edge(["top_file_edge"]);
        test_top_inner_file_edge(["top_inner_file_edge"]);
        test_min_file_edge(["min_file_edge"]);
        test_one_line(["one_line"]);
        test_no_context(["no_context"]);
        // Zero chunk
        test_no_chunk_long(["no_chunk_long"]);
        test_no_chunk_middle(["no_chunk_middle"]);
        test_no_chunk_short(["no_chunk_short"]);
        test_no_chunk_empty(["no_chunk_empty"]);
        // Two chunks or more
        test_two_chunks(["two_chunks"]);
        test_two_chunks_contact(["two_chunks_contact"]);
        test_two_chunks_joint(["two_chunks_joint"]);
        test_two_chunks_blank_between(["two_chunks_blank_between"]);
        test_two_chunks_all_blank_between(["two_chunks_all_blank_between"]);
        test_two_chunks_max_blank_between(["two_chunks_max_blank_between"]);
        test_two_chunks_neighbors(["two_chunks_neighbors"]);
        test_three_chunks(["three_chunks"]);
        test_three_chunks_joint_all(["three_chunks_joint_all"]);
        test_three_chunks_joint_first(["three_chunks_joint_first"]);
        test_three_chunks_joint_second(["three_chunks_joint_second"]);
        // Edge cases
        test_so_many_neighbors(["so_many_neighbors"]);
        // Multiple files
        test_two_files(["single_max", "before"]);
        test_no_chunk_file_between(["single_max", "no_chunk_long", "before"]);
        test_no_chunk_file_begin(["no_chunk_long", "single_max"]);
        test_no_chunk_file_end(["single_max", "no_chunk_long"]);
        test_no_chunk_files(["no_chunk_long", "no_chunk_short"]);
    }

    #[test]
    fn test_same_min_ctx_and_max_ctx() {
        let dir = Path::new("testdata").join("chunk");
        let matches = test::read_matches(&dir, "single_max");
        let got: Vec<_> = Files::new(matches.into_iter(), 3, 3)
            .collect::<Result<_>>()
            .unwrap();

        let path = dir.join("single_max.in");
        let expected = File {
            line_numbers: vec![8].into_boxed_slice(),
            chunks: vec![(5, 11)].into_boxed_slice(),
            contents: fs::read(&path).unwrap().into_boxed_slice(),
            path,
        };

        assert_eq!(got.len(), 1);
        assert_eq!(got[0], expected);
    }

    #[test]
    fn test_zero_context() {
        let dir = Path::new("testdata").join("chunk");
        let matches = test::read_matches(&dir, "single_max");
        let got: Vec<_> = Files::new(matches.into_iter(), 0, 0)
            .collect::<Result<_>>()
            .unwrap();

        let path = dir.join("single_max.in");
        let expected = File {
            line_numbers: vec![8].into_boxed_slice(),
            chunks: vec![(8, 8)].into_boxed_slice(),
            contents: fs::read(&path).unwrap().into_boxed_slice(),
            path,
        };

        assert_eq!(got.len(), 1);
        assert_eq!(got[0], expected);
    }

    #[test]
    fn test_error_while_matching() {
        #[derive(Debug)]
        struct DummyError;
        impl fmt::Display for DummyError {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "dummy error!")
            }
        }
        impl std::error::Error for DummyError {}

        let matches: Vec<Result<Match>> = vec![Err(Error::new(DummyError))];
        let err = Files::new(matches.into_iter(), 3, 6)
            .collect::<Result<Vec<_>>>()
            .unwrap_err();
        assert_eq!(format!("{}", err), "dummy error!");
    }
}
