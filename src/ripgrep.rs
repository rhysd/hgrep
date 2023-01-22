use crate::chunk::Files;
use crate::grep::GrepMatch;
use crate::printer::Printer;
use anyhow::{Context, Result};
use grep_matcher::{LineTerminator, Matcher};
use grep_pcre2::{RegexMatcher as Pcre2Matcher, RegexMatcherBuilder as Pcre2MatcherBuilder};
use grep_regex::{RegexMatcher, RegexMatcherBuilder};
use grep_searcher::{BinaryDetection, MmapChoice, Searcher, SearcherBuilder, Sink, SinkMatch};
use ignore::overrides::OverrideBuilder;
use ignore::types::{Types, TypesBuilder};
use ignore::{Walk, WalkBuilder};
use rayon::iter::ParallelBridge;
use rayon::prelude::*;
use std::env;
use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

// Note: 'main is a lifetime of scope of main() function

fn parse_size(input: &str) -> Result<u64> {
    if input.is_empty() {
        anyhow::bail!("Size string must not be empty");
    }

    let i = input.len() - 1;
    let (input, mag) = match input.as_bytes()[i] {
        b'k' | b'K' => (&input[..i], 1 << 10),
        b'm' | b'M' => (&input[..i], 1 << 20),
        b'g' | b'G' => (&input[..i], 1 << 30),
        _ => (input, 1),
    };

    let u: u64 = input
        .parse()
        .with_context(|| format!("could not parse {:?} as unsigned integer", input))?;

    Ok(u * mag)
}

#[derive(Default, Debug)]
pub struct Config<'main> {
    min_context: u64,
    max_context: u64,
    no_ignore: bool,
    hidden: bool,
    case_insensitive: bool,
    smart_case: bool,
    globs: Box<[&'main str]>,
    glob_case_insensitive: bool,
    fixed_strings: bool,
    word_regexp: bool,
    follow_symlink: bool,
    multiline: bool,
    crlf: bool,
    multiline_dotall: bool,
    mmap: bool,
    max_count: Option<u64>,
    max_depth: Option<usize>,
    max_filesize: Option<u64>,
    line_regexp: bool,
    pcre2: bool,
    types: Vec<&'main str>,
    types_not: Vec<&'main str>,
    invert_match: bool,
    one_file_system: bool,
    no_unicode: bool,
    regex_size_limit: Option<usize>,
    dfa_size_limit: Option<usize>,
}

impl<'main> Config<'main> {
    pub fn new(min: u64, max: u64) -> Self {
        let mut config = Self::default();
        config.min_context(min).max_context(max);
        config
    }

    pub fn min_context(&mut self, num: u64) -> &mut Self {
        self.min_context = num;
        self
    }

    pub fn max_context(&mut self, num: u64) -> &mut Self {
        self.max_context = num;
        self
    }

    pub fn no_ignore(&mut self, yes: bool) -> &mut Self {
        self.no_ignore = yes;
        self
    }

    pub fn hidden(&mut self, yes: bool) -> &mut Self {
        self.hidden = yes;
        self
    }

    pub fn case_insensitive(&mut self, yes: bool) -> &mut Self {
        self.case_insensitive = yes;
        if yes {
            self.smart_case = false;
        }
        self
    }

    pub fn smart_case(&mut self, yes: bool) -> &mut Self {
        self.smart_case = yes;
        if yes {
            self.case_insensitive = false;
        }
        self
    }

    pub fn globs(&mut self, globs: impl Iterator<Item = &'main str>) -> &mut Self {
        self.globs = globs.collect();
        self
    }

    pub fn glob_case_insensitive(&mut self, yes: bool) -> &mut Self {
        self.glob_case_insensitive = yes;
        self
    }

    pub fn fixed_strings(&mut self, yes: bool) -> &mut Self {
        self.fixed_strings = yes;
        if yes {
            self.pcre2 = false; // for regex::escape
        }
        self
    }

    pub fn word_regexp(&mut self, yes: bool) -> &mut Self {
        self.word_regexp = yes;
        if yes {
            self.line_regexp = false;
        }
        self
    }

    pub fn line_regexp(&mut self, yes: bool) -> &mut Self {
        self.line_regexp = yes;
        if yes {
            self.word_regexp = false;
        }
        self
    }

    pub fn follow_symlink(&mut self, yes: bool) -> &mut Self {
        self.follow_symlink = yes;
        self
    }

    pub fn multiline(&mut self, yes: bool) -> &mut Self {
        self.multiline = yes;
        self
    }

    pub fn crlf(&mut self, yes: bool) -> &mut Self {
        self.crlf = yes;
        self
    }

    pub fn multiline_dotall(&mut self, yes: bool) -> &mut Self {
        self.multiline_dotall = yes;
        self
    }

    pub fn mmap(&mut self, yes: bool) -> &mut Self {
        self.mmap = yes;
        self
    }

    pub fn max_count(&mut self, num: u64) -> &mut Self {
        self.max_count = Some(num);
        self
    }

    pub fn max_depth(&mut self, num: usize) -> &mut Self {
        self.max_depth = Some(num);
        self
    }

    pub fn pcre2(&mut self, yes: bool) -> &mut Self {
        self.pcre2 = yes;
        self
    }

    pub fn types(&mut self, types: impl Iterator<Item = &'main str>) -> &mut Self {
        self.types = types.collect();
        self
    }

    pub fn types_not(&mut self, types: impl Iterator<Item = &'main str>) -> &mut Self {
        self.types_not = types.collect();
        self
    }

    pub fn max_filesize(&mut self, input: &str) -> Result<&mut Self> {
        self.max_filesize = Some(parse_size(input)?);
        Ok(self)
    }

    pub fn invert_match(&mut self, yes: bool) -> &mut Self {
        self.invert_match = yes;
        self
    }

    pub fn one_file_system(&mut self, yes: bool) -> &mut Self {
        self.one_file_system = yes;
        self
    }

    pub fn no_unicode(&mut self, yes: bool) -> &mut Self {
        self.no_unicode = yes;
        self
    }

    pub fn regex_size_limit(&mut self, input: &str) -> Result<&mut Self> {
        self.regex_size_limit = Some(parse_size(input)? as usize);
        Ok(self)
    }

    pub fn dfa_size_limit(&mut self, input: &str) -> Result<&mut Self> {
        self.dfa_size_limit = Some(parse_size(input)? as usize);
        Ok(self)
    }

    fn build_walker(&self, mut paths: impl Iterator<Item = &'main Path>) -> Result<Walk> {
        let target = paths.next().unwrap();

        let mut builder = OverrideBuilder::new(target);
        if self.glob_case_insensitive {
            builder.case_insensitive(true)?;
        }
        for glob in self.globs.iter() {
            builder.add(glob)?;
        }
        let overrides = builder.build()?;

        let mut builder = WalkBuilder::new(target);
        for path in paths {
            builder.add(path);
        }
        builder
            .hidden(!self.hidden)
            .parents(!self.no_ignore)
            .ignore(!self.no_ignore)
            .git_global(!self.no_ignore)
            .git_ignore(!self.no_ignore)
            .git_exclude(!self.no_ignore)
            .require_git(false)
            .follow_links(self.follow_symlink)
            .max_depth(self.max_depth)
            .max_filesize(self.max_filesize)
            .overrides(overrides)
            .types(self.build_types()?)
            .same_file_system(self.one_file_system);

        if !self.no_ignore {
            builder.add_custom_ignore_filename(".rgignore");
        }

        Ok(builder.build())
    }

    fn build_regex_matcher(&self, pat: &str) -> Result<RegexMatcher> {
        let mut builder = RegexMatcherBuilder::new();
        builder
            .case_insensitive(self.case_insensitive)
            .case_smart(self.smart_case)
            .word(self.word_regexp)
            .multi_line(true)
            .unicode(!self.no_unicode);

        if self.multiline {
            builder.dot_matches_new_line(self.multiline_dotall);
            if self.crlf {
                builder.crlf(true).line_terminator(None);
            }
        } else {
            builder
                .line_terminator(Some(b'\n'))
                .dot_matches_new_line(false)
                .crlf(self.crlf);
        }

        if let Some(limit) = self.regex_size_limit {
            builder.size_limit(limit);
        }
        if let Some(limit) = self.dfa_size_limit {
            builder.dfa_size_limit(limit);
        }

        Ok(if self.fixed_strings {
            let mut s = regex::escape(pat);
            if self.line_regexp {
                s = format!("^(?:{})$", s);
            }
            builder.build(&s)?
        } else if self.line_regexp {
            builder.build(&format!("^(?:{})$", pat))?
        } else {
            builder.build(pat)?
        })
    }

    fn build_pcre2_matcher(&self, pat: &str) -> Result<Pcre2Matcher> {
        let mut builder = Pcre2MatcherBuilder::new();
        builder
            .caseless(self.case_insensitive)
            .case_smart(self.smart_case)
            .word(self.word_regexp)
            .multi_line(true)
            .crlf(self.crlf);

        #[cfg(target_pointer_width = "64")]
        {
            builder
                .jit_if_available(true)
                .max_jit_stack_size(Some(10 * (1 << 20)));
        }

        if !self.no_unicode {
            builder.utf(true).ucp(true);
        }

        if self.multiline {
            builder.dotall(self.multiline_dotall);
        }

        if self.line_regexp {
            Ok(builder.build(&format!("^(?:{})$", pat))?)
        } else {
            Ok(builder.build(pat)?)
        }
    }

    fn build_searcher(&self) -> Searcher {
        let mut builder = SearcherBuilder::new();
        let mmap = if self.mmap {
            unsafe { MmapChoice::auto() }
        } else {
            MmapChoice::never()
        };
        builder
            .binary_detection(BinaryDetection::quit(0))
            .line_number(true)
            .multi_line(self.multiline)
            .memory_map(mmap)
            .invert_match(self.invert_match);
        if self.crlf {
            builder.line_terminator(LineTerminator::crlf());
        }
        builder.build()
    }

    fn build_types(&self) -> Result<Types> {
        let mut builder = TypesBuilder::new();
        builder.add_defaults();
        for ty in &self.types {
            builder.select(ty);
        }
        for ty in &self.types_not {
            builder.negate(ty);
        }
        Ok(builder.build()?)
    }

    pub fn print_types<W: io::Write>(&self, out: W) -> Result<()> {
        fn print<W: io::Write>(mut out: W, types: &Types) -> io::Result<()> {
            for def in types.definitions() {
                write!(out, "\x1b[1m{}\x1b[0m: ", def.name())?;
                let mut globs = def.globs().iter();
                out.write_all(globs.next().unwrap().as_bytes())?;
                for glob in globs {
                    out.write_all(b", ")?;
                    out.write_all(glob.as_bytes())?;
                }
                out.write_all(b"\n")?;
            }
            Ok(())
        }

        use crate::broken_pipe::IgnoreBrokenPipe;
        let types = self.build_types()?;
        print(out, &types).ignore_broken_pipe()?;
        Ok(())
    }
}

pub fn grep<'main, P: Printer + Sync>(
    printer: P,
    pat: &str,
    paths: Option<impl Iterator<Item = &'main Path>>,
    config: Config<'main>,
) -> Result<bool> {
    let entries = if let Some(paths) = paths {
        config.build_walker(paths)?
    } else {
        let cwd = env::current_dir()?;
        let paths = std::iter::once(cwd.as_path());
        config.build_walker(paths)?
    };

    let paths = entries.filter_map(|entry| match entry {
        Ok(entry) => {
            if entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
                Some(Ok(entry.into_path()))
            } else {
                None
            }
        }
        Err(err) => Some(Err(anyhow::Error::new(err))),
    });

    if config.pcre2 {
        Ripgrep::with_pcre2(pat, config, printer)?.grep(paths)
    } else {
        Ripgrep::with_regex(pat, config, printer)?.grep(paths)
    }
}

#[derive(Default)]
struct LineRegions<'a> {
    ranges: &'a [(usize, usize)],
    offset: usize,
}

impl<'a> LineRegions<'a> {
    fn new(ranges: &'a [(usize, usize)]) -> Self {
        Self { ranges, offset: 0 }
    }

    fn line_ranges(&mut self, line_len: usize) -> Vec<(usize, usize)> {
        // Invariant: self.ranges is sorted and not over-wrapped
        let line_start = self.offset;
        let line_end = line_start + line_len;

        let mut ret = vec![];
        let mut next_start_idx = 0;
        for (idx, (range_start, range_end)) in self.ranges.iter().copied().enumerate() {
            // ls < le < rs < re
            if line_end < range_start {
                break;
            }

            let start = if range_start < line_start {
                0
            } else if line_start <= range_start && range_start < line_end {
                range_start - line_start
            } else {
                // line_end <= range_start
                break;
            };

            let end = if range_end < line_start {
                // This range is not useful for later lines
                next_start_idx = idx;
                continue;
            } else if line_start <= range_end && range_end < line_end {
                range_end - line_start
            } else {
                // line_end <= range_end
                line_end - line_start
            };

            if start < end {
                ret.push((start, end));
            }
        }

        if next_start_idx > 0 {
            self.ranges = &self.ranges[next_start_idx..];
        }
        self.offset = line_end; // Offset for next line

        ret
    }
}

struct Matches<'a, M: Matcher> {
    count: &'a Option<Mutex<u64>>,
    path: PathBuf,
    matcher: &'a M,
    buf: Vec<GrepMatch>,
}

impl<'a, M: Matcher> Sink for Matches<'a, M> {
    type Error = io::Error;

    fn matched(&mut self, _searcher: &Searcher, mat: &SinkMatch<'_>) -> Result<bool, Self::Error> {
        if let Some(count) = &self.count {
            // Note: AtomicU64 is not available since it does not provide fetch_saturating_sub
            let mut c = count.lock().unwrap();
            if *c == 0 {
                return Ok(false);
            }
            *c -= 1;
        }

        let line_number = mat.line_number().unwrap();
        let path = &self.path;

        let mut ranges = vec![];
        self.matcher
            .find_iter(mat.bytes(), |m| {
                ranges.push((m.start(), m.end()));
                true
            })
            .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("{}", e)))?;
        let mut regions = LineRegions::new(&ranges);

        let mut line_number = line_number;
        for line in mat.lines() {
            self.buf.push(GrepMatch {
                path: path.to_owned(),
                line_number,
                ranges: regions.line_ranges(line.len()),
            });
            line_number += 1;
        }

        Ok(true)
    }
}

struct Ripgrep<'main, M: Matcher, P: Printer> {
    config: Config<'main>,
    matcher: M,
    count: Option<Mutex<u64>>,
    printer: P,
}

impl<'main, P: Printer + Sync> Ripgrep<'main, RegexMatcher, P> {
    fn with_regex(pat: &str, config: Config<'main>, printer: P) -> Result<Self> {
        Ok(Self::new(config.build_regex_matcher(pat)?, config, printer))
    }
}

impl<'main, P: Printer + Sync> Ripgrep<'main, Pcre2Matcher, P> {
    fn with_pcre2(pat: &str, config: Config<'main>, printer: P) -> Result<Self> {
        Ok(Self::new(config.build_pcre2_matcher(pat)?, config, printer))
    }
}

impl<'main, M, P> Ripgrep<'main, M, P>
where
    M: Matcher + Sync,
    P: Printer + Sync,
{
    fn new(matcher: M, config: Config<'main>, printer: P) -> Self {
        Self {
            count: config.max_count.map(Mutex::new),
            matcher,
            printer,
            config,
        }
    }

    // Return Result<Option<Vec<_>>> instead of Result<Vec<_>> to make the `filter_map` predicate easy
    // in `grep()` method
    fn search(&self, path: PathBuf) -> Result<Option<Vec<GrepMatch>>> {
        if let Some(count) = &self.count {
            if *count.lock().unwrap() == 0 {
                return Ok(None);
            }
        }

        let file = File::open(&path)?;
        let mut searcher = self.config.build_searcher();
        let mut matches = Matches {
            count: &self.count,
            path,
            matcher: &self.matcher,
            buf: vec![],
        };

        searcher.search_file(&self.matcher, &file, &mut matches)?;
        if matches.buf.is_empty() {
            return Ok(None);
        }

        Ok(Some(matches.buf))
    }

    fn print_matches(&self, matches: Vec<GrepMatch>) -> Result<bool> {
        let (min, max) = (self.config.min_context, self.config.max_context);
        let mut found = false;
        for file in Files::new(matches.into_iter().map(Ok), min, max) {
            self.printer.print(file?)?;
            found = true;
        }
        Ok(found)
    }

    fn grep<I>(&self, paths: I) -> Result<bool>
    where
        I: Iterator<Item = Result<PathBuf>> + Send,
    {
        paths
            .par_bridge()
            .filter_map(|path| match path {
                Ok(path) => self.search(path).transpose(),
                Err(err) => Some(Err(err)),
            })
            .map(|matches| self.print_matches(matches?))
            .try_reduce(|| false, |a, b| Ok(a || b))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunk::{File, LineMatch};
    use crate::test::{read_all_expected_chunks, read_expected_chunks};
    use pretty_assertions::assert_eq;
    use regex::Regex;
    use std::ffi::OsStr;
    use std::fs;
    use std::iter;
    use std::mem;
    use std::path::Path;
    use std::sync::Mutex;

    #[derive(Default)]
    struct DummyPrinter(Mutex<Vec<File>>);
    impl Printer for &DummyPrinter {
        fn print(&self, file: File) -> Result<()> {
            self.0.lock().unwrap().push(file);
            Ok(())
        }
    }

    impl DummyPrinter {
        fn validate_and_remove_region_ranges(&mut self) {
            for file in self.0.get_mut().unwrap().iter_mut() {
                let lines: Vec<_> = file.contents.split_inclusive(|b| *b == b'\n').collect();
                for lmat in file.line_matches.iter_mut() {
                    // Reset `lmat.range` to None since ranges in `expected` are `None`
                    let (start, end) = mem::take(&mut lmat.ranges)[0];
                    let line = lines[lmat.line_number as usize - 1];
                    let matched_part = &line[start..end];
                    assert_eq!(
                        matched_part,
                        b"*",
                        "{:?} did not match to pattern '\\*$'. Line was {:?} (lnum={}). Byte range was ({}, {})",
                        std::str::from_utf8(matched_part).unwrap(),
                        std::str::from_utf8(line).unwrap(),
                        lmat.line_number,
                        start,
                        end
                    )
                }
            }
        }
    }

    fn read_all_inputs(dir: &Path) -> Vec<String> {
        let mut inputs = Vec::new();
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.extension() == Some(OsStr::new("in")) {
                inputs.push(path.file_stem().unwrap().to_string_lossy().to_string());
            }
        }
        inputs
    }

    #[test]
    fn test_grep_each_file() {
        let dir = Path::new("testdata").join("chunk");
        let inputs = read_all_inputs(&dir);

        for input in inputs.iter() {
            let mut printer = DummyPrinter::default();
            let pat = r"\*$";
            let file = dir.join(format!("{}.in", input));
            let paths = iter::once(file.as_path());
            let found = grep(&printer, pat, Some(paths), Config::new(3, 6)).unwrap();
            let expected = read_expected_chunks(&dir, input)
                .map(|f| vec![f])
                .unwrap_or_else(Vec::new);

            printer.validate_and_remove_region_ranges();
            let got = printer.0.into_inner().unwrap();

            assert_eq!(found, !expected.is_empty(), "test file: {:?}", file);
            assert_eq!(expected, got, "test file: {:?}", file);
        }
    }

    #[test]
    fn test_grep_all_files_at_once() {
        let dir = Path::new("testdata").join("chunk");
        let inputs = read_all_inputs(&dir);

        let mut printer = DummyPrinter::default();
        let pat = r"\*$";
        let paths = inputs
            .iter()
            .map(|s| dir.join(format!("{}.in", s)).into_os_string())
            .collect::<Vec<_>>();
        let paths = paths.iter().map(AsRef::as_ref);

        let found = grep(&printer, pat, Some(paths), Config::new(3, 6)).unwrap();

        printer.validate_and_remove_region_ranges();

        let mut got = printer.0.into_inner().unwrap();
        got.sort_by(|a, b| a.path.cmp(&b.path));

        let mut expected = read_all_expected_chunks(&dir, &inputs);
        expected.sort_by(|a, b| a.path.cmp(&b.path));

        assert!(found);
        assert_eq!(expected, got);
    }

    #[test]
    fn test_grep_no_match_found() {
        let path = Path::new("testdata").join("chunk").join("single_max.in");
        let paths = iter::once(path.as_path());
        let printer = DummyPrinter::default();
        let pat = "^this does not match to any line!!!!!!$";
        let found = grep(&printer, pat, Some(paths), Config::new(3, 6)).unwrap();
        let files = printer.0.into_inner().unwrap();
        assert!(!found, "result: {:?}", files);
        assert!(files.is_empty(), "result: {:?}", files);
    }

    #[test]
    fn test_grep_path_does_not_exist() {
        for path in &[
            Path::new("testdata")
                .join("chunk")
                .join("this-file-does-not-exist.txt"),
            Path::new("testdata").join("this-directory-dies-not-exist"),
        ] {
            let paths = iter::once(path.as_path());
            let printer = DummyPrinter::default();
            let pat = ".*";
            grep(&printer, pat, Some(paths), Config::new(3, 6)).unwrap_err();
            assert!(printer.0.into_inner().unwrap().is_empty());
        }
    }

    struct ErrorPrinter;
    impl Printer for ErrorPrinter {
        fn print(&self, _: File) -> Result<()> {
            anyhow::bail!("dummy error")
        }
    }

    #[test]
    fn test_grep_print_error() {
        let path = Path::new("testdata").join("chunk").join("single_max.in");
        let paths = iter::once(path.as_path());
        let pat = ".*";
        let err = grep(ErrorPrinter, pat, Some(paths), Config::new(3, 6)).unwrap_err();
        let msg = format!("{}", err);
        assert_eq!(msg, "dummy error");
    }

    #[test]
    fn test_print_types() {
        let config = Config::default();
        let mut buf = Vec::new();
        config.print_types(&mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();

        let re = Regex::new(r"^\x1b\[1m\w+\x1b\[0m: .+(, .+)*$").unwrap();
        for line in output.lines() {
            assert!(re.is_match(line), "{:?} did not match to {:?}", line, re);
        }
    }

    fn read_ripgrep_expected(file_name: &str) -> File {
        let path = Path::new("testdata").join("ripgrep").join(file_name);
        let contents = fs::read_to_string(&path).unwrap();
        let mut lines = contents.lines();

        let mut chunks = vec![];
        {
            let chunks_line = lines.next().unwrap();
            assert!(
                chunks_line.starts_with("# chunks: "),
                "actual={:?}",
                chunks_line
            );
            let chunks_line = &chunks_line["# chunks: ".len()..];
            for chunk in chunks_line.split(", ") {
                let mut s = chunk.split(' ');
                let start = s.next().unwrap().parse().unwrap();
                let end = s.next().unwrap().parse().unwrap();
                chunks.push((start, end));
            }
        }

        let mut line_matches = vec![];
        {
            let matches_line = lines.next().unwrap();
            assert!(
                matches_line.starts_with("# lines: "),
                "actual={:?}",
                matches_line
            );
            let matches_line = &matches_line["# lines: ".len()..];
            for mat in matches_line.split(", ") {
                let mut s = mat.split(' ');
                let line_number = s.next().unwrap().parse().unwrap();
                let start = s.next().unwrap().parse().unwrap();
                let end = s.next().unwrap().parse().unwrap();
                line_matches.push(LineMatch {
                    line_number,
                    ranges: vec![(start, end)],
                })
            }
        }

        File::new(path, line_matches, chunks, contents.into_bytes())
    }

    fn test_ripgrep_config(file: &str, pat: &str, f: fn(&mut Config) -> ()) {
        let path = Path::new("testdata").join("ripgrep").join(file);
        let paths = iter::once(path.as_path());
        let printer = DummyPrinter::default();

        let mut config = Config::new(1, 2);
        f(&mut config);

        let found = grep(&printer, pat, Some(paths), config).unwrap();
        assert!(found, "file={}", file);

        let mut files = printer.0.into_inner().unwrap();
        assert_eq!(files.len(), 1, "file={}", file);

        let expected = read_ripgrep_expected(file);
        assert_eq!(files.pop().unwrap(), expected, "file={}", file);
    }

    #[test]
    fn test_multiline() {
        test_ripgrep_config("multiline.txt", r"this\r?\nis the\r?\ntest string", |c| {
            c.multiline(true);
        });
    }

    #[test]
    fn test_multiline_crlf() {
        test_ripgrep_config(
            "multiline_windows.txt",
            r"this\r?\nis the\r?\ntest string",
            |c| {
                c.crlf(true);
                c.multiline(true);
            },
        );
    }

    #[test]
    fn test_case_insensitive() {
        test_ripgrep_config("case_insensitive.txt", r"this is test", |c| {
            c.case_insensitive(true);
        });
    }

    #[test]
    fn test_fixed_strings() {
        test_ripgrep_config("fixed_string.txt", r"this\sis\stest", |c| {
            c.fixed_strings(true);
        });
    }

    #[test]
    fn test_pcre2() {
        test_ripgrep_config("pcre2.txt", r"this\sis\stest", |c| {
            c.pcre2(true);
        });
    }

    macro_rules! line_regions_tests {
        {$(
            $name:ident(
                $ranges:expr,
                $line_lens:expr,
                [$($expected:expr),*],
            );
        )+} => {
            $(
                #[test]
                fn $name() {
                    let ranges = &$ranges;
                    let line_lens = &$line_lens;
                    let expected = &[
                        $(
                            &$expected[..],
                        )*
                    ];

                    let mut r = LineRegions::new(ranges);
                    for (idx, len) in line_lens.iter().copied().enumerate() {
                        assert_eq!(&r.line_ranges(len), expected[idx], "index={}", idx);
                    }
                }
            )+
        }
    }

    line_regions_tests! {
        region_no_region(
            [],
            [2, 2, 2],
            [[], [], []],
        );
        region_entire_line(
            [(1, 5)],
            [2, 2, 2],
            [[(1, 2)], [(0, 2)], [(0, 1)]],
        );
        region_entire_lines(
            [(2, 6)],
            [2, 2, 2, 2],
            [[], [(0, 2)], [(0, 2)], []],
        );
        region_entire_region(
            [(1, 5)],
            [10],
            [[(1, 5)]],
        );
        region_left_most(
            [(0, 3)],
            [5],
            [[(0, 3)]],
        );
        region_right_most(
            [(3, 5)],
            [5],
            [[(3, 5)]],
        );
        region_accross_lines(
            [(3, 5)],
            [2, 2, 2, 2],
            [[], [(1, 2)], [(0, 1)], []],
        );
        regions_accross_lines(
            [(3, 5), (9, 11)],
            [2, 2, 2, 2, 2, 2, 2],
            [[], [(1, 2)], [(0, 1)], [], [(1, 2)], [(0, 1)], []],
        );
        regions_entire_lines(
            [(1, 2), (5, 6)],
            [4, 4],
            [[(1, 2)], [(1, 2)]],
        );
        regions_multi_regions_in_line(
            [(1, 2), (3, 4)],
            [10],
            [[(1, 2), (3, 4)]],
        );
        regions_multi_regions_in_lines(
            [(1, 2), (3, 4), (6, 7), (8, 9)],
            [5, 5],
            [[(1, 2), (3, 4)], [(1, 2), (3, 4)]],
        );
    }

    #[test]
    fn test_parse_size() {
        let tests = &[
            ("123", Ok(123)),
            ("123k", Ok(123 * 1024)),
            ("123m", Ok(123 * 1024 * 1024)),
            ("123g", Ok(123 * 1024 * 1024 * 1024)),
            ("123K", Ok(123 * 1024)),
            ("123M", Ok(123 * 1024 * 1024)),
            ("123G", Ok(123 * 1024 * 1024 * 1024)),
            ("", Err("Size string must not be empty")),
            ("abc", Err("could not parse \"abc\" as unsigned integer")),
            ("123kk", Err("could not parse \"123k\" as unsigned integer")),
            ("-123k", Err("could not parse \"-123\" as unsigned integer")),
        ];

        for (input, want) in tests.iter().copied() {
            let mut c = Config::default();
            let errs = [
                ("max-filesize", c.max_filesize(input).err()),
                ("regex-size-limit", c.regex_size_limit(input).err()),
                ("dfa-size-limit", c.dfa_size_limit(input).err()),
            ];
            for (opt, err) in errs {
                match want {
                    Ok(want) => assert_eq!(
                        c.max_filesize,
                        Some(want),
                        "wanted {} for {:?} ({})",
                        want,
                        input,
                        opt,
                    ),
                    Err(want) => {
                        let err = err.unwrap_or_else(|| {
                            panic!(
                                "wanted error {:?} but no error for {:?} ({})",
                                want, input, opt
                            )
                        });
                        let msg = format!("{}", err);
                        assert!(
                            msg.contains(want),
                            "wanted error {:?} but got {:?} for {:?} ({})",
                            want,
                            msg,
                            input,
                            opt,
                        );
                    }
                }
            }
        }
    }
}
