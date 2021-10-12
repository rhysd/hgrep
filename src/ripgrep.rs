use crate::chunk::Chunks;
use crate::grep::Match;
use crate::printer::Printer;
use anyhow::Result;
use grep_regex::RegexMatcher;
use grep_searcher::{Searcher, Sink, SinkMatch};
use rayon::prelude::*;
use std::collections::HashSet;
use std::ffi::OsStr;
use std::fs::File;
use std::io;
use std::path::PathBuf;
use std::sync::Mutex;
use walkdir::WalkDir;

pub fn grep<'a, P: Printer + Send>(
    printer: P,
    pat: &str,
    paths: impl Iterator<Item = &'a OsStr>,
    context: u64,
) -> Result<()> {
    // Use HashSet for unique paths
    let paths: HashSet<_> = paths
        .flat_map(WalkDir::new)
        .filter_map(|entry| match entry {
            Ok(e) if e.file_type().is_file() => Some(Ok(e.into_path())),
            Err(e) => Some(Err(e)),
            _ => None,
        })
        .collect::<std::result::Result<_, _>>()?;

    if paths.is_empty() {
        return Ok(());
    }

    let printer = Mutex::new(printer);
    paths.into_par_iter().try_for_each(|path| {
        let matches = grep_file(pat, path)?;
        let printer = printer.lock().unwrap();
        for chunk in Chunks::new(matches.into_iter().map(Ok), context) {
            printer.print(chunk?)?;
        }
        Ok(())
    })
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

fn grep_file(pat: &str, path: PathBuf) -> Result<Vec<Match>> {
    let file = File::open(&path)?;
    let matcher = RegexMatcher::new(pat)?;
    let mut searcher = Searcher::new();
    let mut matches = Matches { path, buf: vec![] };
    searcher.search_file(&matcher, &file, &mut matches)?;
    Ok(matches.buf)
}
