use criterion::{criterion_group, criterion_main, Criterion};
use hgrep::chunk::{File, LineMatch};
use hgrep::printer::{Printer, PrinterOptions, TermColorSupport, TextWrapMode};
use hgrep::ripgrep;
use hgrep::syntect::{LockableWrite, SyntectPrinter};
use hgrep_bench::node_modules_path;
use hgrep_bench::read_package_lock_json;
use rayon::prelude::*;
use std::cmp;
use std::io;
use std::io::Write;
use std::iter;
use std::mem;
use std::path::Path;
use std::sync::{Mutex, MutexGuard};

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
impl<'a> LockableWrite<'a> for Sink {
    type Locked = SinkLock<'a>;
    fn lock(&'a self) -> Self::Locked {
        SinkLock(self.0.lock().unwrap())
    }
}

fn get_opts() -> PrinterOptions<'static> {
    let mut opts = PrinterOptions::default();
    opts.color_support = TermColorSupport::True;
    opts.term_width = 80;
    opts
}

fn large_file(c: &mut Criterion) {
    let (path, contents) = read_package_lock_json();
    let lines = contents.lines().count() as u64;
    let mut files = vec![];
    for l in (1..=lines).step_by(500) {
        let s = l.saturating_sub(6);
        let e = cmp::min(l + 6, lines);
        files.push(File::new(
            path.into(),
            vec![LineMatch::lnum(l)],
            vec![(s, e)],
            contents.clone().into_bytes(),
        ))
    }

    c.bench_function("syntect::package-lock.json", |b| {
        b.iter(|| {
            let sink = Sink(Mutex::new(vec![]));
            let opts = get_opts();
            let mut printer = SyntectPrinter::new(sink, opts).unwrap();
            files
                .clone()
                .into_par_iter()
                .try_for_each(|f| printer.print(f))
                .unwrap();
            let buf = mem::take(printer.writer_mut()).0.into_inner().unwrap();
            assert!(!buf.is_empty());
        })
    });
}

fn with_ripgrep(c: &mut Criterion) {
    #[inline]
    fn run_ripgrep(pat: &str, dir: &Path, opts: PrinterOptions<'_>) -> bool {
        let sink = Sink(Mutex::new(vec![]));
        let printer = SyntectPrinter::new(sink, opts).unwrap();
        let mut config = ripgrep::Config::new(3, 6);
        config.no_ignore(true);
        ripgrep::grep(printer, pat, Some(iter::once(dir.as_os_str())), config).unwrap()
    }

    let node_modules = node_modules_path();
    c.bench_function("syntect::ripgrep-large", |b| {
        b.iter(|| assert!(run_ripgrep(r"\bparcel\b", node_modules, get_opts())))
    });

    let project_src = Path::new("..").join("src");
    c.bench_function("syntect::ripgrep-small", |b| {
        b.iter(|| assert!(run_ripgrep("Printer", &project_src, get_opts())))
    });

    let testdata = Path::new("..").join("testdata").join("chunk");
    c.bench_function("syntect::ripgrep-tiny", |b| {
        b.iter(|| assert!(run_ripgrep(r"\*$", &testdata, get_opts())))
    });

    c.bench_function("syntect::ripgrep-no-wrap", |b| {
        b.iter(|| {
            let mut opts = get_opts();
            opts.text_wrap = TextWrapMode::Never;
            assert!(run_ripgrep("Printer", &project_src, opts))
        })
    });

    c.bench_function("syntect::ripgrep-background", |b| {
        b.iter(|| {
            let mut opts = get_opts();
            opts.background_color = true;
            assert!(run_ripgrep("Printer", &project_src, opts))
        })
    });
}

criterion_group!(syntect, large_file, with_ripgrep);
criterion_main!(syntect);
