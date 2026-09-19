//! The binary, run as a process.
//!
//! What these check is the part that only exists once the pieces are wired
//! together: that the bytes on standard output are the library's own output,
//! that the exit code says what `--strict` and `--deny-warnings` promise, and
//! that `--diagnostics json` is JSON. The transformations themselves are
//! tested in `usfm_pipeline`, and the flags in `src/args.rs`.
//!
//! No test dependency: the binary is run with `std::process::Command` on the
//! path cargo hands us, and the JSON is checked by hand.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// The binary cargo built for this test run.
const BIN: &str = env!("CARGO_BIN_EXE_usfm");

/// One of the tcdocs conformance inputs, by test name.
///
/// The suite is a git submodule; a missing file here means it was not checked
/// out, which the message says, exactly as the `usfm_tests` build does.
fn tcdocs(name: &str) -> PathBuf {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tcdocs/tests/basic")
        .join(name)
        .join("origin.usfm");
    assert!(
        path.exists(),
        "{} is missing; run `git submodule update --init tcdocs`",
        path.display()
    );
    path
}

/// Write `contents` to a file in this test run's temporary directory and hand
/// back its path.
fn input(name: &str, contents: &str) -> PathBuf {
    let path = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    std::fs::write(&path, contents).expect("writing a test input");
    path
}

/// Run `usfm` with `args`.
fn usfm<I, S>(args: I) -> Output
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    Command::new(BIN)
        .args(args)
        .output()
        .expect("running the usfm binary")
}

fn stdout(output: &Output) -> &str {
    std::str::from_utf8(&output.stdout).expect("stdout is UTF-8")
}

fn stderr(output: &Output) -> &str {
    std::str::from_utf8(&output.stderr).expect("stderr is UTF-8")
}

/// What `usfm_usx` writes for the same file, parsed the same way the CLI
/// parses it.
fn usx_in_process(path: &Path) -> String {
    let source = std::fs::read_to_string(path).expect("reading the input");
    let document = usfm_parser::parser::Parser::new(&source)
        .parse(&usfm_parser::DEFAULT_STYLESHEET)
        .document;
    usfm_usx::to_usx_string(&document)
}

/// `parse --format usx` writes exactly what the library writes — byte for
/// byte, with nothing added around it — and `--output` writes the same bytes
/// to a file.
#[test]
fn usx_output_is_the_library_output() {
    for name in ["minimal", "footnote"] {
        let path = tcdocs(name);
        let output = usfm([
            "parse".as_ref(),
            "--format".as_ref(),
            "usx".as_ref(),
            path.as_os_str(),
        ]);
        assert!(output.status.success(), "{}: {}", name, stderr(&output));
        assert_eq!(stdout(&output), usx_in_process(&path), "{name}");

        let written = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("{name}.usx"));
        let output = usfm([
            "parse".as_ref(),
            "-f".as_ref(),
            "usx".as_ref(),
            "-o".as_ref(),
            written.as_os_str(),
            path.as_os_str(),
        ]);
        assert!(output.status.success(), "{}: {}", name, stderr(&output));
        assert!(stdout(&output).is_empty(), "{name}");
        assert_eq!(
            std::fs::read_to_string(&written).expect("reading the output file"),
            usx_in_process(&path),
            "{name}"
        );
    }
}

/// An error diagnostic is reported either way; only `--strict` makes it fatal.
/// A document without `\id` is `missing-id`, which is an error.
#[test]
fn an_error_is_fatal_only_under_strict() {
    let path = input("no-id.usfm", "\\p hello\n");

    let output = usfm(["parse".as_ref(), path.as_os_str()]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(
        stderr(&output).contains("error[missing-id]"),
        "{}",
        stderr(&output)
    );
    assert!(!stdout(&output).is_empty());

    let output = usfm(["parse".as_ref(), "--strict".as_ref(), path.as_os_str()]);
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    assert!(stdout(&output).is_empty(), "no output under --strict");
    assert!(stderr(&output).contains("--strict"), "{}", stderr(&output));
}

/// `--deny-warnings` is the stricter threshold: an input whose worst
/// diagnostic is a warning passes `--strict` and fails this.
#[test]
fn deny_warnings_is_fatal_for_a_warning() {
    // `ZZZ` is not a book code USFM lists, which is `unlisted-book-code`, a
    // warning; nothing else in this input is reported at all.
    let path = input("unlisted-book.usfm", "\\id ZZZ\n\\c 1\n\\p \\v 1 text\n");

    let output = usfm(["parse".as_ref(), "--strict".as_ref(), path.as_os_str()]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(
        stderr(&output).contains("warning[unlisted-book-code]"),
        "{}",
        stderr(&output)
    );

    let output = usfm([
        "parse".as_ref(),
        "--deny-warnings".as_ref(),
        path.as_os_str(),
    ]);
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    assert!(
        stdout(&output).is_empty(),
        "no output under --deny-warnings"
    );
    assert!(
        stderr(&output).contains("--deny-warnings"),
        "{}",
        stderr(&output)
    );
}

/// `--diagnostics json` writes one JSON object per line, and nothing else, on
/// standard error. Checked by hand rather than with a JSON parser: the shape
/// is fixed by `Diagnostic::to_json_line`, which has its own tests.
#[test]
fn json_diagnostics_are_one_object_per_line() {
    let path = input(
        "two-diagnostics.usfm",
        "\\p hello\n\\p \\zz unknown marker\n",
    );
    let output = usfm([
        "parse".as_ref(),
        "--diagnostics".as_ref(),
        "json".as_ref(),
        path.as_os_str(),
    ]);
    assert!(output.status.success(), "{}", stderr(&output));

    let lines: Vec<&str> = stderr(&output).lines().collect();
    assert_eq!(lines.len(), 2, "{:?}", lines);
    for line in &lines {
        assert!(line.starts_with('{'), "{line}");
        assert!(line.ends_with('}'), "{line}");
        assert!(line.contains(r#""code":"#), "{line}");
        assert!(line.contains(r#""severity":"#), "{line}");
        assert!(
            line.contains(r#""line":1"#) || line.contains(r#""line":2"#),
            "{line}"
        );
    }
    assert!(lines[0].contains(r#""code":"missing-id""#), "{}", lines[0]);
    assert!(
        lines[1].contains(r#""code":"unknown-custom-marker""#),
        "{}",
        lines[1]
    );

    // The text rendering is the default, and is not JSON.
    let output = usfm(["parse".as_ref(), path.as_os_str()]);
    assert!(
        stderr(&output).starts_with("input:1:1: error[missing-id]"),
        "{}",
        stderr(&output)
    );
}

/// Text from the document reaches the HTML as character data, not as markup:
/// `<` and `&` in the source are escaped.
#[test]
fn html_output_escapes_text() {
    let path = input(
        "reserved.usfm",
        "\\id GEN\n\\c 1\n\\p \\v 1 Tom & <Jerry> won.\n",
    );
    let output = usfm([
        "parse".as_ref(),
        "--format".as_ref(),
        "html".as_ref(),
        path.as_os_str(),
    ]);
    assert!(output.status.success(), "{}", stderr(&output));

    let html = stdout(&output);
    assert!(html.contains("Tom &amp; &lt;Jerry&gt; won."), "{html}");
    assert!(!html.contains("<Jerry>"), "{html}");
}

/// The two combinations of flags that used to reach a `todo!()` or an
/// `unimplemented!()` in the old binary are ordinary errors: a message that
/// names the flag to change, and exit 1.
#[test]
fn an_unwritable_combination_is_an_error() {
    let one = input("weave-a.usfm", "\\id GEN\n\\c 1\n\\p \\v 1 One. Two.\n");
    let two = input("weave-b.usfm", "\\id GEN\n\\c 1\n\\p \\v 1 Eins. Zwei.\n");

    let output = usfm([
        "parse".as_ref(),
        "--format".as_ref(),
        "usx".as_ref(),
        "--diglot".as_ref(),
        two.as_os_str(),
        one.as_os_str(),
    ]);
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    assert!(stderr(&output).contains("--diglot"), "{}", stderr(&output));

    let output = usfm([
        "parse".as_ref(),
        "--format".as_ref(),
        "prompt".as_ref(),
        one.as_os_str(),
    ]);
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    assert!(stderr(&output).contains("--diglot"), "{}", stderr(&output));

    // With both sides it writes the weave.
    let output = usfm([
        "parse".as_ref(),
        "--format".as_ref(),
        "prompt".as_ref(),
        "--diglot".as_ref(),
        two.as_os_str(),
        one.as_os_str(),
    ]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(
        stdout(&output).contains("<lang dialect=\"a\">One.</lang>"),
        "{}",
        stdout(&output)
    );
}
