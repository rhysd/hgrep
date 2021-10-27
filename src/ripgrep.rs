use crate::chunk::Files;
use crate::grep::GrepMatch;
use crate::printer::Printer;
use anyhow::Result;
use grep_matcher::{LineTerminator, Matcher};
use grep_pcre2::{RegexMatcher as Pcre2Matcher, RegexMatcherBuilder as Pcre2MatcherBuilder};
use grep_regex::{RegexMatcher, RegexMatcherBuilder};
use grep_searcher::{BinaryDetection, MmapChoice, Searcher, SearcherBuilder, Sink, SinkMatch};
use ignore::overrides::OverrideBuilder;
use ignore::types::{Types, TypesBuilder};
use ignore::{WalkBuilder, WalkParallel, WalkState};
use rayon::prelude::*;
use std::env;
use std::ffi::OsStr;
use std::fs::File;
use std::io;
use std::path::PathBuf;
use std::sync::mpsc::channel;
use std::sync::Mutex;

// Note: 'main is a lifetime of scope of main() function

#[derive(Default)]
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

    pub fn max_filesize(&mut self, num: u64) -> &mut Self {
        self.max_filesize = Some(num);
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

    fn build_walker(&self, mut paths: impl Iterator<Item = &'main OsStr>) -> Result<WalkParallel> {
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
            .types(self.build_types()?);

        if !self.no_ignore {
            builder.add_custom_ignore_filename(".rgignore");
        }

        Ok(builder.build_parallel())
    }

    fn build_regex_matcher(&self, pat: &str) -> Result<RegexMatcher> {
        let mut builder = RegexMatcherBuilder::new();
        builder
            .case_insensitive(self.case_insensitive)
            .case_smart(self.smart_case)
            .word(self.word_regexp)
            .multi_line(true);

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
            .memory_map(mmap);
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

    pub fn print_types<W: io::Write>(&self, mut out: W) -> Result<()> {
        let types = self.build_types()?;
        for def in types.definitions() {
            out.write_all(def.name().as_bytes())?;
            out.write_all(b": ")?;
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
}

pub fn grep<'main, P: Printer + Sync>(
    printer: P,
    pat: &str,
    paths: Option<impl Iterator<Item = &'main OsStr>>,
    config: Config<'main>,
) -> Result<bool> {
    let paths = if let Some(paths) = paths {
        walk(paths, &config)?
    } else {
        let cwd = env::current_dir()?;
        let paths = std::iter::once(cwd.as_os_str());
        walk(paths, &config)?
    };

    if paths.is_empty() {
        return Ok(false);
    }

    if config.pcre2 {
        Ripgrep::with_pcre2(pat, config, printer)?.grep(paths)
    } else {
        Ripgrep::with_regex(pat, config, printer)?.grep(paths)
    }
}

fn walk<'main>(
    paths: impl Iterator<Item = &'main OsStr>,
    config: &Config<'main>,
) -> Result<Vec<PathBuf>> {
    let walker = config.build_walker(paths)?;

    let (tx, rx) = channel();
    walker.run(|| {
        // This function is called per threads for initialization.
        let tx = tx.clone();
        Box::new(move |entry| match entry {
            Ok(entry) => {
                if entry.file_type().map(|f| f.is_file()).unwrap_or(false) {
                    tx.send(Ok(entry.into_path())).unwrap();
                }
                WalkState::Continue
            }
            Err(err) => {
                tx.send(Err(anyhow::Error::new(err))).unwrap();
                WalkState::Quit
            }
        })
    });
    drop(tx); // Notify sender finishes

    rx.into_iter().collect()
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

        let range = self
            .matcher
            .find(mat.bytes())
            .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("{}", e)))?
            .map(|m| (m.start(), m.end()));

        #[inline]
        fn region_in_line(
            range: Option<(usize, usize)>,
            line_start: usize,
            line_end: usize,
        ) -> Option<(usize, usize)> {
            let (start, end) = range?;

            let region_start = if start < line_start {
                0
            } else if line_start <= start && start < line_end {
                start - line_start
            } else {
                // line_end <= start
                return None;
            };

            let region_end = if end < line_start {
                return None;
            } else if line_start <= end && end < line_end {
                end - line_start
            } else {
                // line_end <= end
                line_end - line_start
            };

            if region_start == region_end {
                return None;
            }
            Some((region_start, region_end))
        }

        let mut start = 0;
        let mut line_number = line_number;
        for line in mat.lines() {
            let end = start + line.len();
            self.buf.push(GrepMatch {
                path: path.to_owned(),
                line_number,
                range: region_in_line(range, start, end),
            });
            start = end;
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

    fn search(&self, path: PathBuf) -> Result<Vec<GrepMatch>> {
        if let Some(count) = &self.count {
            if *count.lock().unwrap() == 0 {
                return Ok(vec![]);
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
        Ok(matches.buf)
    }

    fn grep_file(&self, path: PathBuf) -> Result<bool> {
        let matches = self.search(path)?;
        let (min, max) = (self.config.min_context, self.config.max_context);
        let mut found = false;
        for file in Files::new(matches.into_iter().map(Ok), min, max) {
            self.printer.print(file?)?;
            found = true;
        }
        Ok(found)
    }

    fn grep(&self, paths: Vec<PathBuf>) -> Result<bool> {
        paths
            .into_par_iter()
            .map(|path| self.grep_file(path))
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
                    let (start, end) = mem::take(&mut lmat.range).unwrap();
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
            let paths = iter::once(OsStr::new(&file));
            let mut config = Config::new(3, 6);
            if cfg!(target_os = "windows") {
                config.crlf(true);
            }

            let found = grep(&printer, pat, Some(paths), config).unwrap();
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
        let mut config = Config::new(3, 6);
        if cfg!(target_os = "windows") {
            config.crlf(true);
        }
        let paths = inputs
            .iter()
            .map(|s| dir.join(format!("{}.in", s)).into_os_string())
            .collect::<Vec<_>>();
        let paths = paths.iter().map(AsRef::as_ref);

        let found = grep(&printer, pat, Some(paths), config).unwrap();

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
        let paths = iter::once(path.as_os_str());
        let printer = DummyPrinter::default();
        let pat = "^this does not match to any line!!!!!!$";
        let mut config = Config::new(3, 6);
        if cfg!(target_os = "windows") {
            config.crlf(true);
        }
        let found = grep(&printer, pat, Some(paths), config).unwrap();
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
            let paths = iter::once(path.as_os_str());
            let printer = DummyPrinter::default();
            let pat = ".*";
            let mut config = Config::new(3, 6);
            if cfg!(target_os = "windows") {
                config.crlf(true);
            }
            grep(&printer, pat, Some(paths), config).unwrap_err();
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
        let paths = iter::once(path.as_os_str());
        let pat = ".*";
        let mut config = Config::new(3, 6);
        if cfg!(target_os = "windows") {
            config.crlf(true);
        }
        let err = grep(ErrorPrinter, pat, Some(paths), config).unwrap_err();
        let msg = format!("{}", err);
        assert_eq!(msg, "dummy error");
    }

    #[test]
    fn test_print_types() {
        let config = Config::default();
        let mut buf = Vec::new();
        config.print_types(&mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();

        let re = Regex::new(r"^\w+: .+(, .+)*$").unwrap();
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
                    range: Some((start, end)),
                })
            }
        }

        File::new(path, line_matches, chunks, contents.into_bytes())
    }

    fn test_ripgrep_config(file: &str, pat: &str, f: fn(&mut Config) -> ()) {
        let path = Path::new("testdata").join("ripgrep").join(file);
        let paths = iter::once(path.as_os_str());
        let printer = DummyPrinter::default();

        let mut config = Config::new(1, 2);
        if cfg!(target_os = "windows") {
            config.crlf(true);
        }
        f(&mut config);

        assert!(grep(&printer, pat, Some(paths), config).unwrap());

        let mut files = printer.0.into_inner().unwrap();
        assert_eq!(files.len(), 1);

        let expected = read_ripgrep_expected(file);
        assert_eq!(files.pop().unwrap(), expected);
    }

    #[test]
    fn test_multiline() {
        test_ripgrep_config("multiline.txt", r"this\r?\nis the\r?\ntest string", |c| {
            c.multiline(true);
        });
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
}
