use crate::chunk::Chunks;
use crate::grep::Match;
use crate::printer::Printer;
use anyhow::{Error, Result};
use grep_regex::RegexMatcher;
use grep_searcher::{BinaryDetection, Searcher, Sink, SinkMatch};
use ignore::{DirEntry, WalkBuilder, WalkState};
use rayon::prelude::*;
use std::ffi::OsStr;
use std::fs::File;
use std::io;
use std::path::PathBuf;
use std::sync::mpsc::channel;
use std::sync::Mutex;

pub fn grep<'a, P: Printer + Send>(
    printer: P,
    pat: &str,
    paths: impl Iterator<Item = &'a OsStr>,
    context: u64,
) -> Result<()> {
    let paths = walk(paths)?;
    if paths.is_empty() {
        return Ok(());
    }

    let printer = Mutex::new(printer);
    paths.into_par_iter().try_for_each(|path| {
        let matches = search(pat, path)?;
        let printer = printer.lock().unwrap();
        for chunk in Chunks::new(matches.into_iter().map(Ok), context) {
            printer.print(chunk?)?;
        }
        Ok(())
    })
}

fn walk<'a>(mut paths: impl Iterator<Item = &'a OsStr>) -> Result<Vec<PathBuf>> {
    let mut builder = WalkBuilder::new(paths.next().unwrap());
    for path in paths {
        builder.add(path);
    }
    builder
        .parents(true)
        .ignore(true)
        .git_global(true)
        .git_ignore(true)
        .git_exclude(true)
        .require_git(false)
        .add_custom_ignore_filename(".rgignore");

    let walker = builder.build_parallel();

    let (tx, rx) = channel();
    walker.run(|| {
        // This function is called per threads for initialization.
        let tx = tx.clone();
        Box::new(move |entry| {
            let quit = entry.is_err();
            let path = entry.map(DirEntry::into_path).map_err(Error::new);
            tx.send(path).unwrap();
            if quit {
                WalkState::Quit
            } else {
                WalkState::Continue
            }
        })
    });

    drop(tx); // Notify sender finishes
    rx.into_iter().collect()
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
