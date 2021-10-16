use crate::chunk::Files;
use crate::grep::Match;
use crate::printer::Printer;
use anyhow::Result;
use grep_matcher::Matcher;
use grep_pcre2::{RegexMatcher as Pcre2Matcher, RegexMatcherBuilder as Pcre2MatcherBuilder};
use grep_regex::{RegexMatcher, RegexMatcherBuilder};
use grep_searcher::{BinaryDetection, MmapChoice, Searcher, SearcherBuilder, Sink, SinkMatch};
use ignore::overrides::OverrideBuilder;
use ignore::{WalkBuilder, WalkParallel, WalkState};
use rayon::prelude::*;
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
}

impl<'main> Config<'main> {
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
            .overrides(overrides);

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
        builder.build()
    }
}

pub fn grep<'main, P: Printer + Send>(
    printer: P,
    pat: &str,
    paths: impl Iterator<Item = &'main OsStr>,
    config: Config<'main>,
) -> Result<()> {
    let paths = walk(paths, &config)?;
    if paths.is_empty() {
        return Ok(());
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

struct Matches<'a> {
    multiline: bool,
    count: &'a Option<Mutex<u64>>,
    path: PathBuf,
    buf: Vec<Match>,
}

impl<'a> Sink for Matches<'a> {
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
        let path = self.path.clone();
        self.buf.push(Match { path, line_number });
        if self.multiline {
            for i in 1..mat.lines().count() {
                let line_number = line_number + i as u64;
                let path = self.path.clone();
                self.buf.push(Match { path, line_number });
            }
        }
        Ok(true)
    }
}

struct Ripgrep<'main, M: Matcher, P: Printer + Send> {
    config: Config<'main>,
    matcher: M,
    count: Option<Mutex<u64>>,
    printer: Mutex<P>,
}

impl<'main, P: Printer + Send> Ripgrep<'main, RegexMatcher, P> {
    fn with_regex(pat: &str, config: Config<'main>, printer: P) -> Result<Self> {
        Ok(Self::new(config.build_regex_matcher(pat)?, config, printer))
    }
}

impl<'main, P: Printer + Send> Ripgrep<'main, Pcre2Matcher, P> {
    fn with_pcre2(pat: &str, config: Config<'main>, printer: P) -> Result<Self> {
        Ok(Self::new(config.build_pcre2_matcher(pat)?, config, printer))
    }
}

impl<'main, M, P> Ripgrep<'main, M, P>
where
    M: Matcher + Sync,
    P: Printer + Send,
{
    fn new(matcher: M, config: Config<'main>, printer: P) -> Self {
        Self {
            count: config.max_count.map(Mutex::new),
            matcher,
            printer: Mutex::new(printer),
            config,
        }
    }

    fn search(&self, path: PathBuf) -> Result<Vec<Match>> {
        if let Some(count) = &self.count {
            if *count.lock().unwrap() == 0 {
                return Ok(vec![]);
            }
        }
        let file = File::open(&path)?;
        let mut searcher = self.config.build_searcher();
        let mut matches = Matches {
            multiline: self.config.multiline,
            count: &self.count,
            path,
            buf: vec![],
        };
        searcher.search_file(&self.matcher, &file, &mut matches)?;
        Ok(matches.buf)
    }

    fn grep_file(&self, path: PathBuf) -> Result<()> {
        let matches = self.search(path)?;
        let printer = self.printer.lock().unwrap();
        let (min, max) = (self.config.min_context, self.config.max_context);
        for file in Files::new(matches.into_iter().map(Ok), min, max) {
            printer.print(file?)?;
        }
        Ok(())
    }

    fn grep(&self, paths: Vec<PathBuf>) -> Result<()> {
        paths
            .into_par_iter()
            .try_for_each(|path| self.grep_file(path))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunk::File;
    use crate::test::{read_all_expected_chunks, read_expected_chunks};
    use std::ffi::OsStr;
    use std::fs;
    use std::iter;
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
            let printer = DummyPrinter::default();
            let pat = r"\*$";
            let file = dir.join(format!("{}.in", input));
            let paths = iter::once(OsStr::new(&file));
            let mut config = Config::default();
            config.min_context(3).max_context(6);

            grep(&printer, pat, paths, config).unwrap();

            let expected = read_expected_chunks(&dir, input)
                .map(|f| vec![f])
                .unwrap_or_else(Vec::new);

            assert_eq!(
                expected,
                printer.0.into_inner().unwrap(),
                "test file: {:?}",
                file
            );
        }
    }

    #[test]
    fn test_grep_all_files_at_once() {
        let dir = Path::new("testdata").join("chunk");
        let inputs = read_all_inputs(&dir);

        let printer = DummyPrinter::default();
        let pat = r"\*$";
        let mut config = Config::default();
        config.min_context(3).max_context(6);
        let paths = inputs
            .iter()
            .map(|s| dir.join(format!("{}.in", s)).into_os_string())
            .collect::<Vec<_>>();
        let paths = paths.iter().map(AsRef::as_ref);

        grep(&printer, pat, paths, config).unwrap();

        let mut got = printer.0.into_inner().unwrap();
        got.sort_by(|a, b| a.path.cmp(&b.path));

        let mut expected = read_all_expected_chunks(&dir, &inputs);
        expected.sort_by(|a, b| a.path.cmp(&b.path));

        assert_eq!(expected, got);
    }
}
