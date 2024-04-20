use crate::grep::GrepMatch;
use anyhow::Result;
use encoding_rs::{Encoding, UTF_8};
use memchr::{memchr2, memchr_iter, Memchr};
use pathdiff::diff_paths;
use std::cmp;
use std::env;
use std::fs;
use std::iter::Peekable;
use std::path::PathBuf;

fn decode_text(mut bytes: Vec<u8>, encoding: Option<&'static Encoding>) -> String {
    if let Some(encoding) = encoding {
        return encoding.decode_with_bom_removal(&bytes).0.into_owned();
    }

    if let Some((encoding, bom_len)) = Encoding::for_bom(&bytes) {
        if encoding == UTF_8 {
            bytes.drain(..bom_len); // Strip UTF-8 BOM from file (#20)
        } else {
            return encoding
                .decode_without_bom_handling(&bytes[bom_len..])
                .0
                .into_owned();
        }
    }

    String::from_utf8(bytes)
        .unwrap_or_else(|err| String::from_utf8_lossy(err.as_bytes()).into_owned())
}

#[cfg_attr(test, derive(Debug, PartialEq))]
#[derive(Clone)] // Implement Clone for benchmark
pub struct LineMatch {
    pub line_number: u64,
    // Byte offsets of start/end positions within the line. Inherit from GrepMatch
    pub ranges: Vec<(usize, usize)>,
}

impl LineMatch {
    pub fn new(line_number: u64, ranges: Vec<(usize, usize)>) -> Self {
        Self {
            line_number,
            ranges,
        }
    }

    pub fn lnum(line_number: u64) -> Self {
        Self {
            line_number,
            ranges: vec![],
        }
    }
}

#[cfg_attr(test, derive(Debug, PartialEq))]
#[derive(Clone)] // Implement Clone for benchmark
pub struct File {
    pub path: PathBuf,
    pub line_matches: Box<[LineMatch]>,
    pub chunks: Box<[(u64, u64)]>, // Start/End line number of the chunk
    pub contents: Box<str>,
}

impl File {
    pub fn new(
        path: PathBuf,
        lm: Vec<LineMatch>,
        chunks: Vec<(u64, u64)>,
        contents: String,
    ) -> Self {
        Self {
            path,
            line_matches: lm.into_boxed_slice(),
            chunks: chunks.into_boxed_slice(),
            contents: contents.into_boxed_str(),
        }
    }

    pub fn sample_file() -> Self {
        let lmats = vec![
            LineMatch::new(3, vec![(4, 7)]),
            LineMatch::new(4, vec![(7, 10)]),
        ];
        let chunks = vec![(1, 7)];
        let contents = "\
// Parse input as float number and print sqrt of it
fn print_sqrt<S: AsRef<str>>(input: S) {
    let result = input.as_ref().parse::<f64>();
    if let Ok(f) = result {
        println!(\"sqrt of {:.2} is {:.2}\", f, f.sqrt());
    }
}\
        ";
        Self::new(
            PathBuf::from("sample.rs"),
            lmats,
            chunks,
            contents.to_string(),
        )
    }

    pub fn first_line(&self) -> &str {
        let mut line = self.contents.as_ref();
        if let Some(idx) = memchr2(b'\n', b'\r', line.as_bytes()) {
            line = &line[..idx];
        }
        line
    }
}

pub struct LinesInclusive<'a> {
    lnum: u64,
    prev: usize,
    buf: &'a str,
    iter: Memchr<'a>,
}

impl<'a> LinesInclusive<'a> {
    pub fn new(buf: &'a str) -> Self {
        Self {
            lnum: 1,
            prev: 0,
            buf,
            iter: memchr_iter(b'\n', buf.as_bytes()),
        }
    }
}

impl<'a> Iterator for LinesInclusive<'a> {
    type Item = (&'a str, u64);
    fn next(&mut self) -> Option<Self::Item> {
        if let Some(idx) = self.iter.next() {
            let lnum = self.lnum;
            let end = idx + 1;
            let line = &self.buf[self.prev..end];
            self.prev = end;
            self.lnum += 1;
            Some((line, lnum))
        } else if self.prev == self.buf.len() {
            None
        } else {
            let line = &self.buf[self.prev..];
            self.prev = self.buf.len();
            Some((line, self.lnum))
        }
    }
}

// Optimized version of str::Lines with line numbers
struct Lines<'a>(LinesInclusive<'a>);

impl<'a> Lines<'a> {
    pub fn new(buf: &'a str) -> Self {
        Self(LinesInclusive::new(buf))
    }
}

impl<'a> Iterator for Lines<'a> {
    type Item = (&'a str, u64);
    fn next(&mut self) -> Option<Self::Item> {
        let (mut line, lnum) = self.0.next()?;
        if let Some(l) = line.strip_suffix('\n') {
            line = l.strip_suffix('\r').unwrap_or(l);
        }
        Some((line, lnum))
    }
}

pub struct Files<I: Iterator> {
    iter: Peekable<I>,
    min_context: u64,
    max_context: u64,
    saw_error: bool,
    cwd: Option<PathBuf>,
    encoding: Option<&'static Encoding>,
}

impl<I: Iterator> Files<I> {
    pub fn new(
        iter: I,
        min_context: u64,
        max_context: u64,
        encoding: Option<&str>,
    ) -> Result<Self> {
        let encoding = if let Some(label) = encoding {
            let encoding = Encoding::for_label(label.as_bytes())
                .ok_or_else(|| anyhow::anyhow!("Unknown encoding name: {label:?}"))?;
            Some(encoding)
        } else {
            None
        };

        Ok(Self {
            iter: iter.peekable(),
            min_context,
            max_context,
            saw_error: false,
            cwd: env::current_dir().ok(),
            encoding,
        })
    }
}

impl<I: Iterator<Item = Result<GrepMatch>>> Files<I> {
    fn calculate_chunk_range<'contents>(
        &self,
        match_start: u64,
        match_end: u64,
        lines: impl Iterator<Item = (&'contents str, u64)>,
    ) -> (u64, u64) {
        let before_start = cmp::max(match_start.saturating_sub(self.max_context), 1);
        let before_end = cmp::max(match_start.saturating_sub(self.min_context), 1);
        let after_start = match_end + self.min_context;
        let after_end = match_end + self.max_context;

        let mut range_start = before_start;
        let mut range_end = after_end;
        let mut last_lnum = None;

        for (line, lnum) in lines {
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

    fn error_item(&mut self, e: anyhow::Error) -> Option<Result<File>> {
        self.saw_error = true;
        Some(Err(e))
    }
}

impl<I: Iterator<Item = Result<GrepMatch>>> Iterator for Files<I> {
    type Item = Result<File>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.saw_error {
            return None;
        }

        let GrepMatch {
            path,
            mut line_number,
            ranges,
        } = match self.iter.next()? {
            Ok(m) => m,
            Err(e) => return self.error_item(e),
        };
        let contents = match fs::read(&path) {
            Ok(vec) => decode_text(vec, self.encoding),
            Err(err) => return self.error_item(err.into()), // TODO: Add file path to the context of the error
        };
        // Assumes that matched lines are sorted by source location
        let mut lines = Lines::new(&contents);
        let mut lmats = vec![LineMatch {
            line_number,
            ranges,
        }];
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
                    Some(Ok(m)) if m.line_number <= line_number => {
                        // When the same line number is reported multiple times, ignore the grep line.
                        // This happens when reading output from `rg --vimgrep` (#13)
                        self.iter.next();
                        continue;
                    }
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
                    State::Error => {
                        let err = self.iter.next().unwrap().unwrap_err();
                        return self.error_item(err);
                    }
                    State::NextMatch => {
                        // Next match
                        let m = self.iter.next().unwrap().unwrap();
                        line_number = m.line_number;
                        lmats.push(LineMatch::new(line_number, m.ranges));
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
            let m = self.iter.next().unwrap().unwrap();
            line_number = m.line_number;
            // First match line of next chunk
            lmats.push(LineMatch::new(line_number, m.ranges));
        }

        if chunks.is_empty() {
            return None;
        }

        let path = self.relative_path(path);
        Some(Ok(File::new(path, lmats, chunks, contents)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test;
    use anyhow::Error;
    use encoding_rs::{SHIFT_JIS, UTF_16BE, UTF_16LE, UTF_8};
    use std::fmt;
    use std::iter;
    use std::path::Path;

    fn test_success_case(inputs: &[&str]) {
        let dir = Path::new("testdata").join("chunk");

        let matches = test::read_all_matches(&dir, inputs);
        let got: Vec<_> = Files::new(matches.into_iter(), 3, 6, None)
            .unwrap()
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
        let got: Vec<_> = Files::new(matches.into_iter(), 3, 3, None)
            .unwrap()
            .collect::<Result<_>>()
            .unwrap();

        let path = dir.join("single_max.in");
        let expected = File {
            line_matches: vec![LineMatch::lnum(8)].into_boxed_slice(),
            chunks: vec![(5, 11)].into_boxed_slice(),
            contents: fs::read_to_string(&path).unwrap().into_boxed_str(),
            path,
        };

        assert_eq!(got.len(), 1);
        assert_eq!(got[0], expected);
    }

    #[test]
    fn test_zero_context() {
        let dir = Path::new("testdata").join("chunk");
        let matches = test::read_matches(&dir, "single_max");
        let got: Vec<_> = Files::new(matches.into_iter(), 0, 0, None)
            .unwrap()
            .collect::<Result<_>>()
            .unwrap();

        let path = dir.join("single_max.in");
        let expected = File {
            line_matches: vec![LineMatch::lnum(8)].into_boxed_slice(),
            chunks: vec![(8, 8)].into_boxed_slice(),
            contents: fs::read_to_string(&path).unwrap().into_boxed_str(),
            path,
        };

        assert_eq!(got.len(), 1);
        assert_eq!(got[0], expected);
    }

    #[test]
    fn test_same_line_occurs_repeatedly() {
        // Same line may be reported multiple times when reading output from `rg --vimgrep` (regression test for #17)

        let mat = |lnum| {
            Result::Ok(GrepMatch {
                path: "Cargo.toml".into(),
                line_number: lnum,
                ranges: vec![],
            })
        };
        let matches = [mat(1), mat(1), mat(1), mat(2), mat(2), mat(2)];

        let mut files = Files::new(matches.into_iter(), 0, 0, None).unwrap();
        let File {
            line_matches,
            chunks,
            ..
        } = files.next().unwrap().unwrap();
        assert!(files.next().is_none());

        let want = vec![LineMatch::lnum(1), LineMatch::lnum(2)].into_boxed_slice();
        assert_eq!(line_matches, want);
        let want = vec![(1, 1), (2, 2)].into_boxed_slice();
        assert_eq!(chunks, want);
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

        for matches in [
            vec![Err(Error::new(DummyError))], // Error at first match
            vec![
                Ok(GrepMatch {
                    path: "Cargo.toml".into(),
                    line_number: 1,
                    ranges: vec![],
                }),
                Err(Error::new(DummyError)), // Error at second match
            ],
        ] {
            let err = Files::new(matches.into_iter(), 3, 6, None)
                .unwrap()
                .collect::<Result<Vec<_>>>()
                .unwrap_err();
            assert_eq!(format!("{}", err), "dummy error!");
        }
    }

    #[test]
    fn test_files_invalid_encoding() {
        let msg = match Files::new(iter::empty::<()>(), 3, 6, Some("foooooooo")) {
            Ok(_) => panic!("error did not happen"),
            Err(err) => format!("{err}"),
        };
        assert!(
            msg.contains("Unknown encoding name: \"foooooooo\""),
            "message={msg:?}",
        );
    }

    #[test]
    fn test_files_with_encoding() {
        let files = Files::new(iter::empty::<()>(), 3, 6, Some("utf-16")).unwrap();
        assert_eq!(files.encoding, Some(UTF_16LE));
    }

    #[test]
    fn test_files_decode_file() {
        let tests = [
            (Some("utf-8"), "utf8_bom.txt"),
            (Some("utf-16le"), "utf16le_bom.txt"),
            (Some("utf-16be"), "utf16be_bom.txt"),
            (Some("sjis"), "sjis.txt"),
            (None, "utf8_bom.txt"),    // Detect from BOM
            (None, "utf16le_bom.txt"), // Detect from BOM
            (None, "utf16be_bom.txt"), // Detect from BOM
        ];

        let dir = {
            let mut p = PathBuf::from("testdata");
            p.push("chunk");
            p.push("encoding");
            p
        };
        let contents = fs::read_to_string(dir.join("utf8.txt")).unwrap();

        for (enc, file) in tests {
            let path = dir.join(file);
            let ranges = vec![(0, 3)]; // "う"
            let item = Ok(GrepMatch {
                path: path.clone(),
                line_number: 4,
                ranges: ranges.clone(),
            });
            let files = Files::new(iter::once(item), 1, 3, enc)
                .unwrap()
                .map(Result::unwrap)
                .collect::<Vec<_>>();

            let expected = [File {
                path,
                line_matches: vec![LineMatch {
                    line_number: 4,
                    ranges,
                }]
                .into_boxed_slice(),
                chunks: vec![(3, 5)].into_boxed_slice(), // Line 3 to 5 should be a chunk because line 2 and line 4 are empty
                contents: contents.clone().into_boxed_str(),
            }];

            assert_eq!(files, expected, "read file {file:?} with encoding {enc:?}");
        }
    }

    #[test]
    fn test_file_get_first_line() {
        let tests = [
            ("", ""),
            ("hello", "hello"),
            ("hello\nworld", "hello"),
            ("hello\r\nworld", "hello"),
            ("\nhello", ""),
            ("\r\nhello", ""),
        ];
        for (lines, first_line) in tests {
            let file = File::new(PathBuf::from("foo"), vec![], vec![], lines.to_string());
            assert_eq!(
                file.first_line(),
                first_line,
                "first line of {lines:?} is incorrect",
            );
        }
    }

    // "こんにちは\r\n" in several encodings
    const HELLO_UTF_16BE: &[u8] = b"\x30\x53\x30\x93\x30\x6B\x30\x61\x30\x6F\x00\x0D\x00\x0A";
    const HELLO_UTF_16BE_BOM: &[u8] =
        b"\xFE\xFF\x30\x53\x30\x93\x30\x6B\x30\x61\x30\x6F\x00\x0D\x00\x0A";
    const HELLO_UTF_16LE: &[u8] = b"\x53\x30\x93\x30\x6B\x30\x61\x30\x6F\x30\x0D\x00\x0A\x00";
    const HELLO_UTF_16LE_BOM: &[u8] =
        b"\xFF\xFE\x53\x30\x93\x30\x6B\x30\x61\x30\x6F\x30\x0D\x00\x0A\x00";
    const HELLO_UTF_8: &[u8] =
        b"\xE3\x81\x93\xE3\x82\x93\xE3\x81\xAB\xE3\x81\xA1\xE3\x81\xAF\x0D\x0A";
    const HELLO_UTF_8_BOM: &[u8] =
        b"\xEF\xBB\xBF\xE3\x81\x93\xE3\x82\x93\xE3\x81\xAB\xE3\x81\xA1\xE3\x81\xAF\x0D\x0A";
    const HELLO_SJIS: &[u8] = b"\x82\xB1\x82\xF1\x82\xC9\x82\xBF\x82\xCD\x0D\x0A";

    #[test]
    fn test_decode_content_with_specified_encoding() {
        let tests = [
            (UTF_16BE, HELLO_UTF_16BE),
            (UTF_16BE, HELLO_UTF_16BE_BOM),
            (UTF_16LE, HELLO_UTF_16LE),
            (UTF_16LE, HELLO_UTF_16LE_BOM),
            (UTF_8, HELLO_UTF_8),
            (UTF_8, HELLO_UTF_8_BOM),
            (SHIFT_JIS, HELLO_SJIS),
        ];

        for (encoding, contents) in tests {
            let text = decode_text(contents.to_vec(), Some(encoding));
            assert_eq!(text, "こんにちは\r\n", "encoding={encoding:?}");
        }
    }

    #[test]
    fn test_decode_content_with_encoding_detected_from_bom() {
        let tests = [HELLO_UTF_16BE_BOM, HELLO_UTF_16LE_BOM, HELLO_UTF_8_BOM];
        for contents in tests {
            let text = decode_text(contents.to_vec(), None);
            assert_eq!(text, "こんにちは\r\n", "input={contents:?}");
        }
    }

    #[test]
    fn test_decode_with_replacement_char_for_malformed_utf8_file() {
        let text = decode_text(vec![0xff], Some(UTF_8));
        assert_eq!(text, "\u{fffd}");
    }
}
