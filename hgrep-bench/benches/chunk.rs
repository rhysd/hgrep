use criterion::{black_box, criterion_group, criterion_main, Criterion};
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

#[inline]
fn count_chunks(data: &[u8], min: u64, max: u64) -> usize {
    let mut total = 0;
    for f in data.grep_lines().chunks_per_file(min, max) {
        let f = f.unwrap();
        assert!(!f.line_numbers.is_empty());
        assert!(!f.chunks.is_empty());
        total += f.chunks.len();
    }
    total
}

fn testdata_dir(c: &mut Criterion) {
    let data = prepare();
    c.bench_function("min_1_max_1", |b| {
        b.iter(|| black_box(count_chunks(&data, 1, 1)))
    });
    c.bench_function("min_3_max_6", |b| {
        b.iter(|| black_box(count_chunks(&data, 3, 6)))
    });
    c.bench_function("min_6_max_12", |b| {
        b.iter(|| black_box(count_chunks(&data, 6, 12)))
    });
    c.bench_function("min_5_max_5", |b| {
        b.iter(|| black_box(count_chunks(&data, 5, 5)))
    });
}

fn large_file(c: &mut Criterion) {
    let mut buf_per_10_lines = String::new();
    let mut buf_per_100_lines = String::new();
    let mut buf_per_1000_lines = String::new();
    let mut buf_per_3000_lines = String::new();
    let contents = match fs::read_to_string("package-lock.json") {
        Ok(s) => s,
        Err(err) => panic!(
            "put large file as \"package-lock.json\" at root of hgrep-bench directory by `npm install`: {}",
            err,
        ),
    };
    for (idx, line) in contents.lines().enumerate() {
        let n = idx + 1;
        if n % 10 != 0 {
            continue;
        }
        let input = format!("package-lock.json:{}:{}\n", n, line);
        buf_per_10_lines += &input;

        if n % 100 == 0 {
            buf_per_100_lines += &input;
        }
        if n % 1000 == 0 {
            buf_per_1000_lines += &input;
        }
        if n % 3000 == 0 {
            buf_per_3000_lines += &input;
        }
    }
    let data_per_10_lines = buf_per_10_lines.into_bytes();
    let data_per_100_lines = buf_per_100_lines.into_bytes();
    let data_per_1000_lines = buf_per_1000_lines.into_bytes();
    let data_per_3000_lines = buf_per_3000_lines.into_bytes();

    c.bench_function("large_file_per_10_lines", |b| {
        b.iter(|| black_box(count_chunks(&data_per_10_lines, 2, 4)))
    });
    c.bench_function("large_file_per_100_lines", |b| {
        b.iter(|| black_box(count_chunks(&data_per_100_lines, 2, 4)))
    });
    c.bench_function("large_file_per_1000_lines", |b| {
        b.iter(|| black_box(count_chunks(&data_per_1000_lines, 2, 4)))
    });
    c.bench_function("large_file_per_3000_lines", |b| {
        b.iter(|| black_box(count_chunks(&data_per_3000_lines, 2, 4)))
    });
}

criterion_group!(chunk, testdata_dir, large_file);
criterion_main!(chunk);
