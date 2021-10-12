use crate::chunk::Chunks;
use crate::grep::Match;
use crate::printer::Printer;
use anyhow::{Error, Result};
use grep_regex::RegexMatcher;
use grep_searcher::{BinaryDetection, Searcher, Sink, SinkMatch};
use ignore::{WalkBuilder, WalkParallel, WalkState};
use rayon::prelude::*;
use std::ffi::OsStr;
use std::fs::File;
use std::io;
use std::path::PathBuf;
use std::sync::mpsc::channel;
use std::sync::Mutex;

#[derive(Default)]
pub struct Config {
    context_lines: u64,
    no_ignore: bool,
}

impl Config {
    pub fn new(context_lines: u64) -> Self {
        Self {
            context_lines,
            ..Default::default()
        }
    }

    pub fn no_ignore(&mut self, yes: bool) -> &Self {
        self.no_ignore = yes;
        self
    }

    pub fn build_walker<'a>(&self, mut paths: impl Iterator<Item = &'a OsStr>) -> WalkParallel {
        let mut builder = WalkBuilder::new(paths.next().unwrap());
        for path in paths {
            builder.add(path);
        }
        builder
            .hidden(false)
            .parents(!self.no_ignore)
            .ignore(!self.no_ignore)
            .git_global(!self.no_ignore)
            .git_ignore(!self.no_ignore)
            .git_exclude(!self.no_ignore)
            .require_git(false);

        if !self.no_ignore {
            builder.add_custom_ignore_filename(".rgignore");
        }

        builder.build_parallel()
    }
}

pub fn grep<'a, P: Printer + Send>(
    printer: P,
    pat: &str,
    paths: impl Iterator<Item = &'a OsStr>,
    config: Config,
) -> Result<()> {
    let paths = walk(paths, &config)?;
    if paths.is_empty() {
        return Ok(());
    }

    let printer = Mutex::new(printer);
    paths.into_par_iter().try_for_each(|path| {
        let matches = search(pat, path)?;
        let printer = printer.lock().unwrap();
        for chunk in Chunks::new(matches.into_iter().map(Ok), config.context_lines) {
            printer.print(chunk?)?;
        }
        Ok(())
    })
}

fn walk<'a>(paths: impl Iterator<Item = &'a OsStr>, config: &Config) -> Result<Vec<PathBuf>> {
    let walker = config.build_walker(paths);
    let (tx, rx) = channel();
    walker.run(|| {
        // This function is called per threads for initialization.
        let tx = tx.clone();
        Box::new(move |entry| {
            let quit = entry.is_err();
            let path = entry.map_err(Error::new);
            tx.send(path).unwrap();
            if quit {
                WalkState::Quit
            } else {
                WalkState::Continue
            }
        })
    });
    drop(tx); // Notify sender finishes

    let mut files = vec![];
    for entry in rx.into_iter() {
        let entry = entry?;
        if entry.file_type().map(|f| f.is_file()).unwrap_or(false) {
            files.push(entry.into_path());
        }
    }
    Ok(files)
}

struct Matches {
    path: PathBuf,
    buf: Vec<Match>,
}

impl Sink for Matches {
    type Error = io::Error;

    fn matched(&mut self, _searcher: &Searcher, mat: &SinkMatch<'_>) -> Result<bool, Self::Error> {
        let line_number = mat.line_number().unwrap();
        let path = self.path.clone();
        self.buf.push(Match { path, line_number });
        Ok(true)
    }
}

fn search(pat: &str, path: PathBuf) -> Result<Vec<Match>> {
    let file = File::open(&path)?;
    let matcher = RegexMatcher::new(pat)?;
    let mut searcher = Searcher::new();
    searcher.set_binary_detection(BinaryDetection::quit(0));
    let mut matches = Matches { path, buf: vec![] };
    searcher.search_file(&matcher, &file, &mut matches)?;
    Ok(matches.buf)
}
