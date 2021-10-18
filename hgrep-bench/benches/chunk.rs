use criterion::{criterion_group, criterion_main, Criterion};
use hgrep::grep::BufReadExt;
use std::fs;
use std::path::Path;

fn prepare() -> Vec<u8> {
    let data_dir = Path::new("..").join("testdata").join("chunk");
    let mut buf = String::new();
    for entry in fs::read_dir(&data_dir).unwrap() {
        let path = entry.unwrap().path();
        for (idx, line) in fs::read_to_string(&path).unwrap().lines().enumerate() {
            if line.ends_with('*') {
                let l = idx + 1;
                let s = path.as_os_str().to_str().unwrap();
                buf += &format!("{}:{}: {}\n", s, l, line);
            }
        }
    }
    buf.into_bytes()
}

fn read_chunk_testdata(c: &mut Criterion) {
    let data = prepare();
    c.bench_function("min_1_max_1", |b| {
        b.iter(|| {
            for f in data.grep_lines().chunks_per_file(1, 1) {
                let f = f.unwrap();
                assert!(!f.line_numbers.is_empty());
                assert!(!f.chunks.is_empty());
            }
        })
    });
    c.bench_function("min_3_max_6", |b| {
        b.iter(|| {
            for f in data.grep_lines().chunks_per_file(3, 6) {
                let f = f.unwrap();
                assert!(!f.line_numbers.is_empty());
                assert!(!f.chunks.is_empty());
            }
        })
    });
    c.bench_function("min_6_max_12", |b| {
        b.iter(|| {
            for f in data.grep_lines().chunks_per_file(6, 12) {
                let f = f.unwrap();
                assert!(!f.line_numbers.is_empty());
                assert!(!f.chunks.is_empty());
            }
        })
    });
    c.bench_function("min_5_max_5", |b| {
        b.iter(|| {
            for f in data.grep_lines().chunks_per_file(5, 5) {
                let f = f.unwrap();
                assert!(!f.line_numbers.is_empty());
                assert!(!f.chunks.is_empty());
            }
        })
    });
}

criterion_group!(chunk, read_chunk_testdata);
criterion_main!(chunk);
