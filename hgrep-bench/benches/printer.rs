use criterion::{criterion_group, criterion_main, Criterion};
use hgrep::chunk::File;
use hgrep::printer::{BatPrinter, Printer};
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

    c.bench_function("node_modules", |b| {
        b.iter(|| {
            let printer = BatPrinter::new();
            for file in files.clone().into_iter() {
                printer.print(file).unwrap();
            }
        })
    });
}

criterion_group!(printer, large_file);
criterion_main!(printer);
