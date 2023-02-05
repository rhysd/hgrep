use std::fs;
use std::path::Path;

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

pub fn package_lock_json_path() -> &'static Path {
    let path = Path::new("package-lock.json");
    assert!(
        path.is_file(),
        "run prepare.bash at hgrep/bench/ directory before running benches",
    );
    path
}

pub fn read_package_lock_json() -> (&'static Path, String) {
    let path = package_lock_json_path();
    (path, fs::read_to_string(path).unwrap())
}

pub fn node_modules_path() -> &'static Path {
    let path = Path::new("node_modules");
    assert!(
        path.is_dir(),
        "run prepare.bash at hgrep/bench/ directory before running benches",
    );
    path
}

pub fn rust_releases_path() -> &'static Path {
    let path = Path::new("rust_releases.md");
    assert!(
        path.is_dir(),
        "run prepare.bash at hgrep/bench/ directory before running benches",
    );
    path
}
