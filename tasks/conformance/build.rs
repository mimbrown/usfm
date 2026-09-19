//! Build script to generate individual test functions from the conformance
//! roots: the tcdocs submodule and the vendored usfm-grammar fixtures.

use std::env;
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("generated_tests.rs");

    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let tcdocs_root = Path::new(&manifest_dir).join("../../tcdocs/tests");
    // Kept in step with `ROOTS` in src/lib.rs: the prefix here only names the
    // generated functions, while lib.rs derives the test names from it.
    let usfm_grammar_root = Path::new(&manifest_dir).join("fixtures/usfm-grammar");

    let mut file = File::create(&dest_path).unwrap();

    // Write module header
    writeln!(
        file,
        "// Auto-generated test functions from the conformance roots"
    )
    .unwrap();
    writeln!(
        file,
        "// Do not edit manually - regenerate with `cargo build`"
    )
    .unwrap();
    writeln!(file).unwrap();

    // A missing submodule is a hard error, not a warning: generating zero
    // tests would let the runner report a 100% pass rate having run nothing.
    // The vendored fixtures are in-tree and cannot stand in for it, so this
    // stays a check on tcdocs alone.
    assert!(
        tcdocs_root.exists(),
        "tcdocs submodule not found at {tcdocs_root:?}; run `git submodule update --init tcdocs`"
    );

    let mut tcdocs_count = 0;
    generate_tests_recursive(&tcdocs_root, &tcdocs_root, "", &mut file, &mut tcdocs_count);

    assert!(
        tcdocs_count > 0,
        "no test cases found under {tcdocs_root:?}; is the tcdocs submodule checked out?"
    );

    let mut vendored_count = 0;
    generate_tests_recursive(
        &usfm_grammar_root,
        &usfm_grammar_root,
        "usfm_grammar_",
        &mut file,
        &mut vendored_count,
    );

    println!(
        "cargo:warning=Generated {} test functions ({tcdocs_count} tcdocs, {vendored_count} usfm-grammar)",
        tcdocs_count + vendored_count
    );

    // Tell Cargo to re-run if either root changes
    println!("cargo:rerun-if-changed=../../tcdocs/tests");
    println!("cargo:rerun-if-changed=fixtures/usfm-grammar");
}

fn generate_tests_recursive(
    root: &Path,
    dir: &Path,
    prefix: &str,
    file: &mut File,
    count: &mut usize,
) {
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

        let test_name = format!("test_{}{}", prefix, test_name);
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
            generate_tests_recursive(root, &path, prefix, file, count);
        }
    }
}
