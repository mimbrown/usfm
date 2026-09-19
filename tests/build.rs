//! Build script to generate individual test functions from tcdocs test suite

use std::env;
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("generated_tests.rs");

    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let tcdocs_root = Path::new(&manifest_dir).join("../tcdocs/tests");

    let mut file = File::create(&dest_path).unwrap();

    // Write module header
    writeln!(file, "// Auto-generated test functions from tcdocs suite").unwrap();
    writeln!(file, "// Do not edit manually - regenerate with `cargo build`").unwrap();
    writeln!(file).unwrap();

    // A missing submodule is a hard error, not a warning: generating zero
    // tests would let the runner report a 100% pass rate having run nothing.
    assert!(
        tcdocs_root.exists(),
        "tcdocs submodule not found at {tcdocs_root:?}; run `git submodule update --init tcdocs`"
    );

    let mut test_count = 0;
    generate_tests_recursive(&tcdocs_root, &tcdocs_root, &mut file, &mut test_count);

    assert!(
        test_count > 0,
        "no test cases found under {tcdocs_root:?}; is the tcdocs submodule checked out?"
    );
    println!("cargo:warning=Generated {} test functions", test_count);

    // Tell Cargo to re-run if tcdocs changes
    println!("cargo:rerun-if-changed=../tcdocs/tests");
}

fn generate_tests_recursive(root: &Path, dir: &Path, file: &mut File, count: &mut usize) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };

    // Check if this is a test directory
    let is_test_dir = dir.join("metadata.xml").exists() || dir.join("origin.usfm").exists();

    if is_test_dir {
        // Generate test function name from path
        let rel_path = dir.strip_prefix(root).unwrap();
        let test_name = rel_path
            .to_string_lossy()
            .replace(['/', '\\', '-', '.'], "_")
            .to_lowercase();

        let test_name = format!("test_{}", test_name);
        let path_str = dir.to_string_lossy();

        writeln!(file, "#[test]").unwrap();
        writeln!(file, "fn {}() {{", test_name).unwrap();
        writeln!(file, "    let path = std::path::Path::new({:?});", path_str).unwrap();
        writeln!(file, "    crate::run_single_test(path);").unwrap();
        writeln!(file, "}}").unwrap();
        writeln!(file).unwrap();

        *count += 1;
    }

    // Recurse into subdirectories
    let mut entries: Vec<_> = entries.flatten().collect();
    entries.sort_by_key(|e| e.path());

    for entry in entries {
        let path = entry.path();
        if path.is_dir() {
            generate_tests_recursive(root, &path, file, count);
        }
    }
}
