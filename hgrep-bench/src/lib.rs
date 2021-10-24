use std::fs;
use std::path::Path;

pub fn read_package_lock_json() -> (&'static str, String) {
    let path = "package-lock.json";
    let contents = fs::read_to_string(path).expect(
        "put large file as \"package-lock.json\" at root of hgrep-bench directory by `npm install`",
    );
    (path, contents)
}

pub fn node_modules_path() -> &'static Path {
    let path = Path::new("node_modules");
    assert!(
        path.is_dir(),
        "put \"node_modules\" directory in hgrep-bench directory by `npm install`"
    );
    path
}
