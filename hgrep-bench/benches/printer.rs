use criterion::{criterion_group, criterion_main, Criterion};
use gag::Gag;
use hgrep::bat::BatPrinter;
use hgrep::chunk::File;
use hgrep::printer::{Printer, PrinterOptions, TermColorSupport};
use hgrep::syntect::SyntectPrinter;
use rayon::prelude::*;
use std::fs;
use std::path::PathBuf;

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
    let mut count = 0;
    let files = std::iter::from_fn(move || {
        count += 100;
        if count > 1000 {
            None
        } else {
            Some(File::new(
                path.into(),
                vec![count],
                vec![(count - 6, count + 6)],
                contents.clone().into_bytes(),
            ))
        }
    })
    .collect::<Vec<_>>();

    c.bench_function("bat", |b| {
        b.iter(|| {
            let _gag = Gag::stdout().unwrap();
            let mut opts = PrinterOptions::default();
            opts.color_support = TermColorSupport::True;
            opts.term_width = 80;
            let printer = BatPrinter::new(opts);
            for file in files.clone().into_iter() {
                printer.print(file).unwrap();
            }
        })
    });

    c.bench_function("syntect", |b| {
        b.iter(|| {
            let _gag = Gag::stdout().unwrap();
            let mut opts = PrinterOptions::default();
            opts.color_support = TermColorSupport::True;
            opts.term_width = 80;
            let printer = SyntectPrinter::with_stdout(opts).unwrap();
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
