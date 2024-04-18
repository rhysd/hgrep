use criterion::{criterion_group, criterion_main, Criterion};
use gag::Gag;
use hgrep::bat::BatPrinter;
use hgrep::chunk::{File, LineMatch};
use hgrep::printer::Printer;
use hgrep::syntect::SyntectPrinter;
use hgrep_bench::{printer_opts, read_package_lock_json};
use rayon::prelude::*;

fn large_file(c: &mut Criterion) {
    let (path, contents) = read_package_lock_json();
    let mut count = 0;
    let files = std::iter::from_fn(move || {
        count += 100;
        if count > 1000 {
            None
        } else {
            Some(File::new(
                path.into(),
                vec![LineMatch::lnum(count)],
                vec![(count - 6, count + 6)],
                contents.clone().into_bytes(),
                None,
            ))
        }
    })
    .collect::<Vec<_>>();

    c.bench_function("printer::bat", |b| {
        b.iter(|| {
            let _gag = Gag::stdout().unwrap();
            let printer = BatPrinter::new(printer_opts());
            for file in files.clone().into_iter() {
                printer.print(file).unwrap();
            }
        })
    });

    c.bench_function("printer::syntect", |b| {
        b.iter(|| {
            let _gag = Gag::stdout().unwrap();
            let printer = SyntectPrinter::with_stdout(printer_opts()).unwrap();
            files
                .clone()
                .into_par_iter()
                .try_for_each(|f| printer.print(f))
                .unwrap();
        })
    });
}

criterion_group!(printer, large_file);
criterion_main!(printer);
