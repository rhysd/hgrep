use crate::grep::Match;
use anyhow::Result;
use std::cmp;
use std::fs;
use std::iter::Peekable;
use std::path::PathBuf;

#[cfg_attr(test, derive(Debug, PartialEq))]
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
        let before_start = cmp::max(match_start.saturating_sub(self.max_context), 1);
        let before_end = cmp::max(match_start.saturating_sub(self.min_context), 1);
        let after_start = match_end + self.min_context;
        let after_end = match_end + self.max_context;

        let mut range_start = before_start;
        let mut range_end = after_end;

        for Line(line, lnum) in lines {
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

        Some(Ok(File::new(path, lnums, chunks, contents)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn test_success_case(inputs: &[&str]) {
        let dir = Path::new("testdata").join("grep_lines_to_chunks_per_file");

        let matches = inputs
            .iter()
            .map(|input| {
                let infile = dir.join(format!("{}.in", input));
                fs::read_to_string(&infile)
                    .unwrap()
                    .split('\n')
                    .enumerate()
                    .filter_map(|(idx, line)| {
                        line.ends_with('*').then(|| {
                            Ok(Match {
                                path: infile.clone(),
                                line_number: idx as u64 + 1,
                            })
                        })
                    })
                    .collect::<Vec<Result<Match>>>()
                    .into_iter()
            })
            .flatten();

        let got: Vec<_> = Files::new(matches, 3, 6).collect::<Result<_>>().unwrap();

        let expected: Vec<_> = inputs
            .iter()
            .map(|input| {
                let outfile = dir.join(format!("{}.out", input));
                let (chunks, lnums) = fs::read_to_string(&outfile)
                    .unwrap()
                    .split('\n')
                    .filter(|s| !s.is_empty())
                    .map(|line| {
                        let mut s = line.split(',');
                        let range = s.next().unwrap();
                        let mut rs = range.split(' ');
                        let chunk_start: u64 = rs.next().unwrap().parse().unwrap();
                        let chunk_end: u64 = rs.next().unwrap().parse().unwrap();
                        let lines = s.next().unwrap();
                        let lnums: Vec<u64> =
                            lines.split(' ').map(|s| s.parse().unwrap()).collect();
                        ((chunk_start, chunk_end), lnums)
                    })
                    .fold(
                        (Vec::new(), Vec::new()),
                        |(mut chunks, mut lnums), (chunk, mut match_lnums)| {
                            chunks.push(chunk);
                            lnums.append(&mut match_lnums);
                            (chunks, lnums)
                        },
                    );
                let infile = dir.join(format!("{}.in", input));
                let contents = fs::read(&infile).unwrap();
                File::new(infile, lnums, chunks, contents)
            })
            .collect();

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
    }
}
