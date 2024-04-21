use criterion::{black_box, criterion_group, criterion_main, Criterion};
use hgrep::chunk::{File, LineMatch};
use hgrep::printer::{Printer, PrinterOptions, TextWrapMode};
use hgrep::ripgrep;
use hgrep::syntect::{SyntectAssets, SyntectPrinter, WriteOnLocked};
use hgrep_bench::{printer_opts, read_package_lock_json, rust_releases_path};
use rayon::prelude::*;
use std::io;
use std::io::Write;
use std::iter;
use std::mem;
use std::path::Path;
use std::sync::{Mutex, MutexGuard};
use std::{cmp, fs};

struct SinkLock<'a>(MutexGuard<'a, Vec<u8>>);
impl<'a> Write for SinkLock<'a> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.write(buf)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
}

#[derive(Default)]
struct Sink(Mutex<Vec<u8>>);
impl WriteOnLocked for Sink {
    type Locked<'a> = SinkLock<'a>;
    fn lock(&self) -> Self::Locked<'_> {
        SinkLock(self.0.lock().unwrap())
    }
}

fn create_files_for_contents(contents: String, path: &Path, per_lines: usize) -> Vec<File> {
    let mut files = vec![];
    let lines = contents.lines().count() as u64;
    for l in (1..=lines).step_by(per_lines) {
        let s = l.saturating_sub(6);
        let e = cmp::min(l + 6, lines);
        files.push(File::new(
            path.into(),
            vec![LineMatch::lnum(l)],
            vec![(s, e)],
            contents.clone(),
        ))
    }
    files
}

fn load_assets(c: &mut Criterion) {
    c.bench_function("syntect::load-assets", |b| {
        b.iter(|| {
            let assets = SyntectAssets::load(None).unwrap();
            black_box(assets)
        })
    });
}

fn print_files(c: &mut Criterion) {
    #[inline]
    fn run(files: Vec<File>, assets: SyntectAssets) {
        let sink = Sink(Mutex::new(vec![]));
        let opts = printer_opts();
        let mut printer = SyntectPrinter::with_assets(assets, sink, opts);
        files
            .into_par_iter()
            .try_for_each(|f| printer.print(f))
            .unwrap();
        let buf = mem::take(printer.writer_mut()).0.into_inner().unwrap();
        assert!(!buf.is_empty());
    }

    let assets = SyntectAssets::load(None).unwrap();

    let (path, contents) = read_package_lock_json();
    let files = create_files_for_contents(contents, path, 500);
    c.bench_function("syntect::print-large", |b| {
        b.iter(|| run(files.clone(), assets.clone()))
    });

    let readme = Path::new("..").join("README.md");
    let contents = fs::read_to_string(readme).unwrap();
    let files = create_files_for_contents(contents, path, 10);
    c.bench_function("syntect::print-small", |b| {
        b.iter(|| run(files.clone(), assets.clone()))
    });

    let readme = Path::new("..").join("LICENSE.txt");
    let contents = fs::read_to_string(readme).unwrap();
    let files = create_files_for_contents(contents, path, 1);
    c.bench_function("syntect::print-tiny", |b| {
        b.iter(|| run(files.clone(), assets.clone()))
    });
}

fn with_ripgrep(c: &mut Criterion) {
    #[inline]
    fn run_ripgrep(pat: &str, dir: &Path, opts: PrinterOptions<'_>) -> bool {
        let sink = Sink(Mutex::new(vec![]));
        let printer = SyntectPrinter::new(sink, opts).unwrap();
        let mut config = ripgrep::Config::new(3, 6);
        config.no_ignore(true);
        ripgrep::grep(printer, pat, Some(iter::once(dir)), config).unwrap()
    }

    let rust_releases = rust_releases_path();
    c.bench_function("syntect::ripgrep-large-file", |b| {
        b.iter(|| assert!(run_ripgrep(r"\brustc\b", rust_releases, printer_opts())))
    });

    let project_src = Path::new("..").join("src");
    c.bench_function("syntect::ripgrep-small", |b| {
        b.iter(|| assert!(run_ripgrep("Printer", &project_src, printer_opts())))
    });

    let testdata = Path::new("..").join("testdata").join("chunk");
    c.bench_function("syntect::ripgrep-tiny", |b| {
        b.iter(|| assert!(run_ripgrep(r"\*$", &testdata, printer_opts())))
    });

    c.bench_function("syntect::ripgrep-no-wrap", |b| {
        b.iter(|| {
            let mut opts = printer_opts();
            opts.text_wrap = TextWrapMode::Never;
            assert!(run_ripgrep("Printer", &project_src, opts))
        })
    });

    c.bench_function("syntect::ripgrep-background", |b| {
        b.iter(|| {
            let mut opts = printer_opts();
            opts.background_color = true;
            assert!(run_ripgrep("Printer", &project_src, opts))
        })
    });
}

criterion_group!(syntect, print_files, with_ripgrep, load_assets);
criterion_main!(syntect);
