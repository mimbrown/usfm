//! CLI for running the USFM parser's conformance suites: the tcdocs test
//! suite and the vendored usfm-grammar fixtures (`ROOTS` in `lib.rs`).

use usfm_tests::*;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut baseline: Option<String> = None;
    let mut write_baseline: Option<String> = None;
    let mut category: Option<String> = None;
    let mut show: Option<String> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--help" | "-h" => {
                print_help();
                return;
            }
            "--list" => {
                list_tests();
                return;
            }
            "--categories" => {
                list_categories();
                return;
            }
            "--show" => {
                let Some(name) = args.get(i + 1) else {
                    eprintln!("--show needs a test name");
                    std::process::exit(2);
                };
                show = Some(name.clone());
                i += 1;
            }
            "--baseline" | "--write-baseline" => {
                let Some(path) = args.get(i + 1) else {
                    eprintln!("{} needs a file path", args[i]);
                    std::process::exit(2);
                };
                if args[i] == "--baseline" {
                    baseline = Some(path.clone());
                } else {
                    write_baseline = Some(path.clone());
                }
                i += 1;
            }
            arg if arg.starts_with('-') => {
                eprintln!("Unknown argument: {}", arg);
                print_help();
                std::process::exit(2);
            }
            arg => category = Some(arg.to_string()),
        }
        i += 1;
    }

    if let Some(name) = show {
        show_test(&name);
        return;
    }
    match category {
        Some(category) => run_category(&category),
        None => run_all(baseline.as_deref(), write_baseline.as_deref()),
    }
}

/// Print one test's diagnostics, its output, and the expected USX it is
/// compared against, all after the harness's normalisation. This is the
/// view to work from when writing a patch in `tests/tcdocs-patches`.
fn show_test(name: &str) {
    let Some(path) = path_for_name(name) else {
        eprintln!("Cannot find test {}", name);
        eprintln!("Use --list to list test names");
        std::process::exit(1);
    };
    let Ok(test) = TestCase::load(&path) else {
        eprintln!("Cannot load test {}", name);
        std::process::exit(1);
    };
    println!("{} ({:?}): {}", test.name, test.metadata.validated, test.metadata.description);
    if test.has_patch() {
        println!("patched by {}", test.patch_path().display());
    }
    let parsed = test.parse_to_usx_with_options(
        test.expected_has_end_milestones(),
        test.expected_has_vid(),
    );
    let (mut actual, diagnostics) = match parsed {
        Ok(parsed) => parsed,
        Err(err) => {
            eprintln!("{}", err);
            std::process::exit(1);
        }
    };
    println!("\nDIAGNOSTICS ({}):", diagnostics.len());
    for diagnostic in &diagnostics {
        println!("  {}", diagnostic);
    }
    test.normalize_for_comparison(&mut actual);
    println!("\nACTUAL:\n{}", actual);
    match test.read_expected_usx() {
        Ok(mut expected) => {
            test.normalize_for_comparison(&mut expected);
            println!("\nEXPECTED:\n{}", expected);
            match compare_xml(&actual, &expected) {
                Ok(()) => println!("\nMATCH"),
                Err(mismatch) => println!("\nMISMATCH: {}", mismatch),
            }
        }
        Err(err) => println!("\nEXPECTED: {}", err),
    }
    match test.run() {
        TestResult::Passed => println!("\nRESULT: pass"),
        other => println!("\nRESULT: {:?}", other),
    }
}

fn print_help() {
    println!(
        r#"USFM Parser Test Runner

Usage:
    cargo run --package usfm_tests [OPTIONS] [CATEGORY]

Options:
    --help, -h              Show this help message
    --list                  List all test cases
    --categories            List all test categories
    --baseline FILE         Compare failures against FILE (one test name per
                            line) and exit non-zero on any difference. Fixed
                            tests must be removed from the baseline, so CI
                            fails on a regression and on stale expectations.
    --write-baseline FILE   Write the current failures to FILE
    --show NAME             Print one test's diagnostics, output and expected
                            USX after normalisation (e.g. --show basic/minimal)

Arguments:
    CATEGORY        Run tests for a specific category (e.g., basic, mandatory,
                    usfm-grammar/bugfixes), or any prefix of a test name

Examples:
    cargo run --package usfm_tests                       # Run all tests
    cargo run --package usfm_tests basic                 # Run basic tests only
    cargo run --package usfm_tests usfm-grammar/bugfixes # Run the vendored cases
    cargo run --package usfm_tests --categories          # List categories
    cargo run --package usfm_tests -- --baseline tests/tcdocs-baseline.txt
"#
    );
}

fn list_tests() {
    let tests = discover_tests();
    println!("Found {} test cases:\n", tests.len());
    for test in &tests {
        let status = match test.metadata.validated {
            ValidationStatus::Pass => "✓",
            ValidationStatus::Fail => "✗",
            ValidationStatus::Unknown => "?",
        };
        println!("  {} {} - {}", status, test.name, test.metadata.description);
    }
}

fn list_categories() {
    let tests = discover_tests();
    let categories = get_categories(&tests);
    println!("Test categories:\n");
    for category in &categories {
        let count = filter_by_category(&tests, category).len();
        println!("  {:20} ({} tests)", category, count);
    }
}

/// A run with no tests must not report success: with the `tcdocs` submodule
/// missing, discovery finds nothing there and the summary would read 100% off
/// the vendored fixtures alone. So the tcdocs root is required to be non-empty,
/// not just the run as a whole.
fn require_tests(tests: &[TestCase]) {
    if tests.is_empty() || !tests.iter().any(|t| t.path.starts_with(TCDOCS_ROOT)) {
        eprintln!("No test cases found under {}", TCDOCS_ROOT);
        eprintln!("Run `git submodule update --init tcdocs` and try again");
        std::process::exit(1);
    }
}

fn run_category(category: &str) {
    let all_tests = discover_tests();
    require_tests(&all_tests);
    let tests: Vec<TestCase> = filter_by_category(&all_tests, category)
        .into_iter()
        .cloned()
        .collect();
    if tests.is_empty() {
        eprintln!("Category not found: {}", category);
        eprintln!("Use --categories to list available categories");
        std::process::exit(1);
    }

    println!("Running tests for category: {}\n", category);
    let summary = run_tests(&tests);
    println!("{}", summary);

    if !summary.is_success() {
        std::process::exit(1);
    }
}

fn run_all(baseline: Option<&str>, write_baseline: Option<&str>) {
    let all_tests = discover_tests();
    require_tests(&all_tests);
    let categories = get_categories(&all_tests);

    println!("=== USFM Parser Test Suite ===\n");
    println!("Running {} tests across {} categories\n", all_tests.len(), categories.len());

    println!("{:20} {:>6} {:>6} {:>6} {:>6} {:>8}",
        "Category", "Pass", "Fail", "Panic", "Skip", "Rate");
    println!("{}", "-".repeat(60));

    for category in &categories {
        let category_tests: Vec<_> = filter_by_category(&all_tests, category)
            .into_iter()
            .cloned()
            .collect();
        let summary = run_tests(&category_tests);
        println!(
            "{:20} {:>6} {:>6} {:>6} {:>6} {:>7.1}%",
            category,
            summary.passed,
            summary.failed,
            summary.panicked,
            summary.skipped,
            summary.pass_rate()
        );
    }

    println!("{}", "-".repeat(60));

    let summary = run_tests(&all_tests);
    println!(
        "{:20} {:>6} {:>6} {:>6} {:>6} {:>7.1}%",
        "TOTAL",
        summary.passed,
        summary.failed,
        summary.panicked,
        summary.skipped,
        summary.pass_rate()
    );

    println!("\n{}", summary);

    if let Some(path) = write_baseline {
        save_baseline(path, &summary);
    }

    let ok = match baseline {
        Some(path) => check_baseline(path, &summary),
        None => summary.is_success(),
    };
    if !ok {
        std::process::exit(1);
    }
}

/// Names of the tests that did not succeed, sorted for stable output.
fn failing_names(summary: &TestSummary) -> Vec<&str> {
    let mut names: Vec<&str> = summary
        .failures
        .iter()
        .map(|(test, _)| test.name.as_str())
        .collect();
    names.sort_unstable();
    names
}

fn save_baseline(path: &str, summary: &TestSummary) {
    let mut out = String::from(
        "# tcdocs tests that currently fail. One name per line.\n\
         # Regenerate with: cargo run --package usfm_tests -- --write-baseline tests/tcdocs-baseline.txt\n",
    );
    for name in failing_names(summary) {
        out.push_str(name);
        out.push('\n');
    }
    if let Err(err) = std::fs::write(path, out) {
        eprintln!("Could not write baseline {}: {}", path, err);
        std::process::exit(1);
    }
    println!("Wrote baseline to {}", path);
}

/// Compare this run against a baseline of known failures. Both directions
/// are errors: a failure that is not in the baseline is a regression, and a
/// baseline entry that now succeeds is a stale expectation that must be
/// removed so the file keeps describing the real state.
fn check_baseline(path: &str, summary: &TestSummary) -> bool {
    let contents = match std::fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(err) => {
            eprintln!("Could not read baseline {}: {}", path, err);
            return false;
        }
    };
    let expected: std::collections::BTreeSet<&str> = contents
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect();
    let actual: std::collections::BTreeSet<&str> = failing_names(summary).into_iter().collect();

    let regressions: Vec<_> = actual.difference(&expected).collect();
    let fixed: Vec<_> = expected.difference(&actual).collect();

    if !regressions.is_empty() {
        println!("\nREGRESSIONS ({} tests fail that are not in {}):", regressions.len(), path);
        for name in &regressions {
            println!("  {}", name);
        }
    }
    if !fixed.is_empty() {
        println!(
            "\nSTALE BASELINE ({} tests in {} now succeed; remove them):",
            fixed.len(),
            path
        );
        for name in &fixed {
            println!("  {}", name);
        }
    }
    if regressions.is_empty() && fixed.is_empty() {
        println!("Failures match baseline {} ({} known failures)", path, expected.len());
        true
    } else {
        false
    }
}
