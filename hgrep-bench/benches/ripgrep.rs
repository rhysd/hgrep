use criterion::{black_box, criterion_group, criterion_main, Criterion};
use hgrep::chunk::File;
use hgrep::printer::Printer;
use hgrep::ripgrep;
use hgrep::Result;
use std::iter;
use std::path::Path;

#[derive(Default)]
struct DummyPrinter;
impl Printer for DummyPrinter {
    fn print(&self, _: File) -> Result<()> {
        black_box(Ok(()))
    }
}

fn testdata_dir(c: &mut Criterion) {
    let dir = Path::new("..").join("testdata").join("chunk");
    c.bench_function("testdata", |b| {
        b.iter(|| {
            let found = ripgrep::grep(
                DummyPrinter,
                r"\*$",
                iter::once(dir.as_os_str()),
                ripgrep::Config::new(3, 6),
            )
            .unwrap();
            assert!(found);
        })
    });
}

fn node_modules(c: &mut Criterion) {
    let dir = Path::new("node_modules");
    assert!(
        dir.is_dir(),
        "put \"node_modules\" directory in hgrep-bench directory by `npm install`"
    );

    c.bench_function("node_modules", |b| {
        b.iter(|| {
            black_box(
                ripgrep::grep(
                    DummyPrinter,
                    r"\bparcel\b",
                    iter::once(dir.as_os_str()),
                    ripgrep::Config::new(3, 6),
                )
                .unwrap(),
            )
        })
    });
}

criterion_group!(ripgrep, testdata_dir, node_modules);
criterion_main!(ripgrep);
