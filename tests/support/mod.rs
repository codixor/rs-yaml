use std::fs;
use std::path::PathBuf;

pub fn fixture(path: &str) -> String {
    fs::read_to_string(golden_root().join(path))
        .unwrap_or_else(|error| panic!("failed to read fixture {path}: {error}"))
}

fn golden_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("golden")
}
