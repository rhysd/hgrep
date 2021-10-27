use criterion::{black_box, criterion_group, criterion_main, Criterion};
use hgrep::chunk::File;
use hgrep::printer::Printer;
use hgrep::ripgrep;
use hgrep::Result;
use hgrep_bench::*;
use std::iter;
use std::path::Path;

#[derive(Default)]
struct DummyPrinter;
impl Printer for DummyPrinter {
    fn print(&self, _: File) -> Result<()> {
        black_box(Ok(()))
    }
}

#[inline]
fn run_ripgrep(pat: &str, path: &Path) -> bool {
    let mut config = ripgrep::Config::new(3, 6);
    config.no_ignore(true);
    ripgrep::grep(
        DummyPrinter,
        pat,
        Some(iter::once(path.as_os_str())),
        config,
    )
    .unwrap()
}

fn bench(c: &mut Criterion) {
    let dir = Path::new("..").join("testdata").join("chunk");
    c.bench_function("ripgrep::testdata", |b| {
        b.iter(|| {
            assert!(run_ripgrep(r"\*$", &dir));
        })
    });

    let dir = node_modules_path();
    c.bench_function("ripgrep::node_modules", |b| {
        b.iter(|| {
            assert!(run_ripgrep(r"\bparcel\b", &dir));
        })
    });

    let file = package_lock_json_path();
    c.bench_function("ripgrep::package-lock.json", |b| {
        b.iter(|| {
            assert!(run_ripgrep(r"\bparcel\b", &file));
        })
    });
}

criterion_group!(ripgrep, bench);
criterion_main!(ripgrep);
