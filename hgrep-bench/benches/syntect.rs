use criterion::{criterion_group, criterion_main, Criterion};
use hgrep::chunk::File;
use hgrep::printer::{Printer, PrinterOptions, TermColorSupport};
use hgrep::ripgrep;
use hgrep::syntect::{LockableWrite, SyntectPrinter};
use rayon::prelude::*;
use std::cmp;
use std::fs;
use std::io;
use std::io::Write;
use std::iter;
use std::mem;
use std::path::{Path, PathBuf};
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

fn large_file(c: &mut Criterion) {
    let path = PathBuf::from("package-lock.json");
    let path = path.as_path();
    let contents = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(err) => panic!(
            "put large file as \"package-lock.json\" at root of hgrep-bench directory by `npm install`: {}",
            err,
        ),
    };
    let lines = contents.lines().count() as u64;
    let mut files = vec![];
    for l in (1..=lines).step_by(500) {
        let s = l.saturating_sub(6);
        let e = cmp::min(l + 6, lines);
        files.push(File::new(
            path.into(),
            vec![l],
            vec![(s, e)],
            contents.clone().into_bytes(),
        ))
    }

    c.bench_function("printer-only", |b| {
        b.iter(|| {
            let mut opts = PrinterOptions::default();
            opts.color_support = TermColorSupport::True;
            opts.term_width = 80;
            let sink = Sink(Mutex::new(vec![]));
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

fn node_modules(c: &mut Criterion) {
    let dir = Path::new("node_modules");
    assert!(
        dir.is_dir(),
        "put \"node_modules\" directory in hgrep-bench directory by `npm install`"
    );

    c.bench_function("printer+ripgrep", |b| {
        b.iter(|| {
            let mut opts = PrinterOptions::default();
            opts.color_support = TermColorSupport::True;
            opts.term_width = 80;
            let sink = Sink(Mutex::new(vec![]));
            let printer = SyntectPrinter::new(sink, opts).unwrap();
            let mut config = ripgrep::Config::new(3, 6);
            config.no_ignore(true);
            let found = ripgrep::grep(
                printer,
                r"\bparcel\b",
                Some(iter::once(dir.as_os_str())),
                config,
            )
            .unwrap();
            assert!(found);
        })
    });
}

criterion_group!(syntect, large_file, node_modules);
criterion_main!(syntect);
