//! The server, run as a process and spoken to over stdio.
//!
//! What this checks is the part that only exists once the pieces are wired
//! together: that the binary speaks the protocol's framing, that opening a
//! document publishes the toolchain's diagnostics with the right code, range
//! and severity, that changing it to a clean document publishes an empty list
//! (which is how an editor is told to clear the squiggles), that
//! `textDocument/formatting` answers with the one edit that replaces the
//! document — and with `null` plus a `window/showMessage` where the file has
//! an error — that `textDocument/hover` answers in markdown, and that
//! `shutdown`/`exit` end the process.
//!
//! No LSP client dependency: the messages are `serde_json::Value`s with a
//! `Content-Length` header written by hand, the way
//! `apps/usfm_cli/tests/cli.rs` runs the CLI with `std::process::Command`. A
//! reader thread hands every message it parses to the test over a channel, so
//! a server that never answers fails on a timeout instead of hanging CI.

use std::io::{BufRead, BufReader, Read, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{Receiver, RecvTimeoutError, channel};
use std::time::Duration;

use serde_json::{Value, json};

/// The binary cargo built for this test run.
const BIN: &str = env!("CARGO_BIN_EXE_usfm-language-server");

/// How long any one message may take. Generous for a loaded CI machine, and
/// still short enough that a hang is a failure rather than a timeout of the
/// whole job.
const TIMEOUT: Duration = Duration::from_secs(30);

/// A document with one unknown marker: `\qqq` is in no stylesheet, so the
/// parser drops it and reports `unknown-marker` (an Error).
///
/// The marker is deliberately not `\zzz`: a `z`-prefixed marker is USFM's
/// user-extension space, and an unknown one of those is the gentler
/// `unknown-custom-marker` (a Warning). Both come through this server the same
/// way; this test asserts the stricter one.
const WITH_UNKNOWN_MARKER: &str = "\\id GEN\n\\c 1\n\\p\n\\v 1 text \\qqq more";

/// The same document with the marker taken out: nothing to report.
const CLEAN: &str = "\\id GEN\n\\c 1\n\\p\n\\v 1 text more\n";

/// The server process, its stdin, and every message it has sent.
struct Lsp {
    child: Child,
    stdin: ChildStdin,
    messages: Receiver<Value>,
    /// Everything read while waiting for something else. A notification the
    /// server sends *before* the response it belongs to — the formatting
    /// refusal's `window/showMessage` — would otherwise be thrown away by the
    /// wait for that response.
    skipped: Vec<Value>,
    next_id: i64,
}

impl Lsp {
    /// Start the binary with a reader thread behind it.
    fn start() -> Self {
        let mut child = Command::new(BIN)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            // Inherited, so a panic in the server shows up in the test output.
            .stderr(Stdio::inherit())
            .spawn()
            .expect("spawning the language server");
        let stdin = child.stdin.take().expect("the server's stdin");
        let stdout = child.stdout.take().expect("the server's stdout");

        let (sender, messages) = channel();
        std::thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            while let Some(message) = read_message(&mut reader) {
                if sender.send(message).is_err() {
                    break; // the test is over
                }
            }
        });

        Self {
            child,
            stdin,
            messages,
            skipped: Vec::new(),
            next_id: 0,
        }
    }

    /// Send a request and hand back its `result`.
    fn request(&mut self, method: &str, params: Value) -> Value {
        self.next_id += 1;
        let id = self.next_id;
        self.send(json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}));
        let response = self.wait_for(|message| message["id"] == json!(id));
        assert_eq!(
            response.get("error"),
            None,
            "{method} failed: {response:#?}"
        );
        response["result"].clone()
    }

    /// Send a notification, which has no answer of its own.
    fn notify(&mut self, method: &str, params: Value) {
        self.send(json!({"jsonrpc": "2.0", "method": method, "params": params}));
    }

    fn send(&mut self, message: Value) {
        let body = serde_json::to_string(&message).expect("serializing a message");
        write!(self.stdin, "Content-Length: {}\r\n\r\n{body}", body.len())
            .and_then(|()| self.stdin.flush())
            .expect("writing to the server");
    }

    /// The next message matching `wanted`, keeping the ones before it (a
    /// `window/logMessage` arrives whenever the server feels like it) in
    /// [`Lsp::skipped`].
    fn wait_for(&mut self, wanted: impl Fn(&Value) -> bool) -> Value {
        loop {
            match self.messages.recv_timeout(TIMEOUT) {
                Ok(message) if wanted(&message) => return message,
                Ok(message) => self.skipped.push(message),
                Err(RecvTimeoutError::Timeout) => panic!("the server said nothing in {TIMEOUT:?}"),
                Err(RecvTimeoutError::Disconnected) => panic!("the server closed its output"),
            }
        }
    }

    /// The `window/showMessage` notifications seen so far, newest last.
    fn show_messages(&self) -> Vec<&Value> {
        self.skipped
            .iter()
            .filter(|message| is_show_message(message))
            .collect()
    }

    /// One `window/showMessage`, taken from what has already been read or
    /// waited for. The server writes a notification and the response to the
    /// request it belongs to on the same stream, and nothing in the protocol
    /// fixes their order, so a test that wants both takes the response first
    /// and asks for the notification here.
    fn show_message(&mut self) -> Value {
        match self.skipped.iter().position(is_show_message) {
            Some(index) => self.skipped.remove(index),
            None => self.wait_for(is_show_message),
        }
    }

    /// The next `textDocument/publishDiagnostics` for `uri`, as the list of
    /// diagnostics it carries.
    fn diagnostics(&mut self, uri: &str) -> Vec<Value> {
        let message = self.wait_for(|message| {
            message["method"] == json!("textDocument/publishDiagnostics")
                && message["params"]["uri"] == json!(uri)
        });
        message["params"]["diagnostics"]
            .as_array()
            .expect("a diagnostics array")
            .clone()
    }
}

impl Drop for Lsp {
    fn drop(&mut self) {
        // A test that failed before `exit` must not leave the process behind.
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn is_show_message(message: &Value) -> bool {
    message["method"] == json!("window/showMessage")
}

/// One `Content-Length`-framed message, or `None` at end of input.
fn read_message(reader: &mut BufReader<impl Read>) -> Option<Value> {
    let mut length = None;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).ok()? == 0 {
            return None;
        }
        let line = line.trim_end_matches(['\r', '\n']);
        if line.is_empty() {
            break; // the blank line that ends the header
        }
        if let Some(value) = line.strip_prefix("Content-Length: ") {
            length = Some(value.parse::<usize>().expect("a Content-Length"));
        }
    }
    let mut body = vec![0; length.expect("a message with no Content-Length")];
    reader.read_exact(&mut body).ok()?;
    Some(serde_json::from_slice(&body).expect("a JSON-RPC message"))
}

/// A path in this test run's temporary directory, and the `file:` URI for it.
///
/// A real directory, because the server looks for a `custom.sty` beside the
/// document: this one has none, so the parse uses the default stylesheet.
fn document(name: &str) -> (PathBuf, String) {
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let uri = format!("file://{}", path.display());
    (path, uri)
}

/// Open `text` as `uri`, the way a client does after `initialized`.
fn open(lsp: &mut Lsp, uri: &str, text: &str) {
    lsp.notify(
        "textDocument/didOpen",
        json!({
            "textDocument": {
                "uri": uri,
                "languageId": "usfm",
                "version": 1,
                "text": text,
            }
        }),
    );
}

/// `shutdown`, `exit`, and the wait for the process to end on its own —
/// `Drop` would kill it, which would hide a server that ignored `exit`.
fn shutdown(lsp: &mut Lsp) {
    assert_eq!(lsp.request("shutdown", json!(null)), json!(null));
    lsp.notify("exit", json!(null));

    let deadline = std::time::Instant::now() + TIMEOUT;
    loop {
        match lsp.child.try_wait().expect("waiting for the server") {
            Some(status) => {
                assert!(status.success(), "the server exited with {status}");
                break;
            }
            None if std::time::Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(20));
            }
            None => panic!("the server did not exit within {TIMEOUT:?}"),
        }
    }
}

#[test]
fn diagnostics_are_published_on_open_and_cleared_on_a_clean_change() {
    let (_path, uri) = document("41MAT.SFM");
    let mut lsp = Lsp::start();

    let result = lsp.request(
        "initialize",
        json!({
            "processId": null,
            "rootUri": null,
            "capabilities": {},
        }),
    );
    // What is implemented: full text sync, UTF-16 positions, and since
    // ticket 31 formatting and hover. Completion is ticket 32's.
    assert_eq!(result["capabilities"]["textDocumentSync"], json!(1));
    assert_eq!(result["capabilities"]["positionEncoding"], json!("utf-16"));
    assert_eq!(result["capabilities"]["hoverProvider"], json!(true));
    assert_eq!(
        result["capabilities"]["documentFormattingProvider"],
        json!(true)
    );
    assert_eq!(result["capabilities"].get("completionProvider"), None);
    assert_eq!(result["serverInfo"]["name"], json!("usfm-language-server"));

    lsp.notify("initialized", json!({}));
    open(&mut lsp, &uri, WITH_UNKNOWN_MARKER);

    let diagnostics = lsp.diagnostics(&uri);
    assert_eq!(diagnostics.len(), 1, "{diagnostics:#?}");
    let diagnostic = &diagnostics[0];
    assert_eq!(diagnostic["code"], json!("unknown-marker"));
    assert_eq!(diagnostic["source"], json!("usfm"));
    // `DiagnosticSeverity.Error`.
    assert_eq!(diagnostic["severity"], json!(1));
    // `\qqq` is on the fourth line (line 3, counting from 0) after `\v 1 text `.
    assert_eq!(
        diagnostic["range"],
        json!({
            "start": {"line": 3, "character": 10},
            "end": {"line": 3, "character": 14},
        }),
    );
    assert!(
        diagnostic["message"].as_str().unwrap().contains("\\qqq"),
        "{diagnostic:#?}",
    );

    // Editing the marker away clears the diagnostic: an empty list is
    // published, not no message at all.
    lsp.notify(
        "textDocument/didChange",
        json!({
            "textDocument": {"uri": uri, "version": 2},
            "contentChanges": [{"text": CLEAN}],
        }),
    );
    assert_eq!(lsp.diagnostics(&uri), Vec::<Value>::new());

    shutdown(&mut lsp);
}

/// Formatting and hover over the wire (ticket 31): the edit the client
/// applies on save, the refusal when the file has an error, and the two
/// shapes of hover.
#[test]
fn formatting_rewrites_the_whole_document_and_hover_explains_what_is_there() {
    let (_path, uri) = document("42MRK.SFM");
    let mut lsp = Lsp::start();

    lsp.request(
        "initialize",
        json!({"processId": null, "rootUri": null, "capabilities": {}}),
    );
    lsp.notify("initialized", json!({}));
    open(&mut lsp, &uri, CLEAN);
    assert_eq!(lsp.diagnostics(&uri), Vec::<Value>::new());

    // The formatted text is `usfm_codegen`'s, with the trailing newline the
    // CLI's `usfm format` adds — the point being that the two write the same
    // bytes. `\p` and the `\v` after it are one paragraph, so the line break
    // between them becomes a space: the edit is a real change.
    let mut expected = usfm::codegen::to_usfm_string(&usfm::parse(CLEAN).document);
    if !expected.ends_with('\n') {
        expected.push('\n');
    }
    assert_ne!(expected, CLEAN);

    let edits = lsp.request(
        "textDocument/formatting",
        json!({
            "textDocument": {"uri": uri},
            "options": {"tabSize": 2, "insertSpaces": true},
        }),
    );
    let edits = edits.as_array().expect("an array of edits").clone();
    assert_eq!(edits.len(), 1, "{edits:#?}");
    assert_eq!(edits[0]["newText"], json!(expected));
    // The range is the whole document: from the start to the empty line after
    // the trailing break, which is `CLEAN`'s four lines.
    assert_eq!(
        edits[0]["range"],
        json!({
            "start": {"line": 0, "character": 0},
            "end": {"line": 4, "character": 0},
        }),
    );

    // Hover on `\p`, which is the third line: the stylesheet's own words.
    let hover = lsp.request(
        "textDocument/hover",
        json!({
            "textDocument": {"uri": uri},
            "position": {"line": 2, "character": 1},
        }),
    );
    assert_eq!(hover["contents"]["kind"], json!("markdown"));
    let markdown = hover["contents"]["value"].as_str().expect("markdown");
    assert!(
        markdown.contains("p - Paragraph - Normal - First Line Indent"),
        "{markdown}",
    );
    assert!(markdown.contains("Occurs under:"), "{markdown}");
    // The range is the paragraph's own span, which runs to the end of the
    // verse text on the line after it.
    assert_eq!(hover["range"]["start"], json!({"line": 2, "character": 0}));

    // Hover on the `\v`, which is the fourth: the reference.
    let hover = lsp.request(
        "textDocument/hover",
        json!({
            "textDocument": {"uri": uri},
            "position": {"line": 3, "character": 0},
        }),
    );
    let markdown = hover["contents"]["value"].as_str().expect("markdown");
    assert!(markdown.contains("GEN 1:1"), "{markdown}");

    // Nothing has been said in a notification yet, so the one after the
    // refusal below is the refusal's.
    assert!(lsp.show_messages().is_empty(), "{:#?}", lsp.show_messages());

    // A document with an error is not formatted: the answer is `null` — not
    // an empty edit list, which would mean "already formatted" — and the
    // reason is shown to the user.
    lsp.notify(
        "textDocument/didChange",
        json!({
            "textDocument": {"uri": uri, "version": 2},
            "contentChanges": [{"text": WITH_UNKNOWN_MARKER}],
        }),
    );
    assert_eq!(lsp.diagnostics(&uri).len(), 1);

    let refused = lsp.request(
        "textDocument/formatting",
        json!({
            "textDocument": {"uri": uri},
            "options": {"tabSize": 2, "insertSpaces": true},
        }),
    );
    assert_eq!(refused, json!(null));

    let message = lsp.show_message();
    // `MessageType.Warning`.
    assert_eq!(message["params"]["type"], json!(2));
    let text = message["params"]["message"].as_str().expect("a message");
    assert!(text.contains("not formatted"), "{text}");
    assert!(text.contains("\\qqq"), "{text}");

    shutdown(&mut lsp);
}
