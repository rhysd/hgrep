use crate::chunk::{File, LineMatch};
use crate::grep::GrepMatch;
use anyhow::Result;
use std::fs;
use std::path::Path;

pub(crate) fn read_matches<S: AsRef<str>>(dir: &Path, input: S) -> Vec<Result<GrepMatch>> {
    let path = dir.join(format!("{}.in", input.as_ref()));
    let path = path.as_path();
    fs::read_to_string(path)
        .unwrap()
        .lines()
        .enumerate()
        .filter_map(|(idx, line)| {
            line.ends_with('*').then(|| {
                Ok(GrepMatch {
                    path: path.into(),
                    line_number: idx as u64 + 1,
                    ranges: vec![],
                })
            })
        })
        .collect::<Vec<Result<GrepMatch>>>()
}

pub(crate) fn read_all_matches<S: AsRef<str>>(dir: &Path, inputs: &[S]) -> Vec<Result<GrepMatch>> {
    inputs
        .iter()
        .flat_map(|input| read_matches(dir, input).into_iter())
        .collect()
}

pub(crate) fn read_expected_chunks<S: AsRef<str>>(dir: &Path, input: S) -> Option<File> {
    let input = input.as_ref();
    let outfile = dir.join(format!("{}.out", input));
    let (chunks, lmats) = fs::read_to_string(outfile)
        .unwrap()
        .lines()
        .filter(|s| !s.is_empty())
        .map(|line| {
            // Format: One chunk per line.
            //   {start line} {last line},{lnum1} {lnum2}...
            let mut s = line.split(',');
            let range = s.next().unwrap();
            let mut rs = range.split(' ');
            let chunk_start: u64 = rs.next().unwrap().parse().unwrap();
            let chunk_end: u64 = rs.next().unwrap().parse().unwrap();
            let lines = s.next().unwrap();
            let lmats: Vec<_> = lines
                .split(' ')
                .map(|s| s.parse().unwrap())
                .map(LineMatch::lnum)
                .collect();
            ((chunk_start, chunk_end), lmats)
        })
        .fold(
            (Vec::new(), Vec::new()),
            |(mut chunks, mut lmats), (chunk, mut match_lmats)| {
                chunks.push(chunk);
                lmats.append(&mut match_lmats);
                (chunks, lmats)
            },
        );
    if chunks.is_empty() || lmats.is_empty() {
        return None;
    }
    let infile = dir.join(format!("{}.in", input));
    let contents = fs::read(&infile).unwrap();
    Some(File::new(infile, lmats, chunks, contents))
}

pub(crate) fn read_all_expected_chunks<S: AsRef<str>>(dir: &Path, inputs: &[S]) -> Vec<File> {
    inputs
        .iter()
        .filter_map(|input| read_expected_chunks(dir, input))
        .collect()
}
