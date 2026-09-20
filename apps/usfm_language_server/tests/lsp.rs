//! The server, run as a process and spoken to over stdio.
//!
//! What this checks is the part that only exists once the pieces are wired
//! together: that the binary speaks the protocol's framing, that opening a
//! document publishes the toolchain's diagnostics with the right code, range
//! and severity, that changing it to a clean document publishes an empty list
//! (which is how an editor is told to clear the squiggles), and that
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

    /// The next message matching `wanted`, ignoring the ones before it (a
    /// `window/logMessage` arrives whenever the server feels like it).
    fn wait_for(&self, wanted: impl Fn(&Value) -> bool) -> Value {
        loop {
            match self.messages.recv_timeout(TIMEOUT) {
                Ok(message) if wanted(&message) => return message,
                Ok(_) => continue,
                Err(RecvTimeoutError::Timeout) => panic!("the server said nothing in {TIMEOUT:?}"),
                Err(RecvTimeoutError::Disconnected) => panic!("the server closed its output"),
            }
        }
    }

    /// The next `textDocument/publishDiagnostics` for `uri`, as the list of
    /// diagnostics it carries.
    fn diagnostics(&self, uri: &str) -> Vec<Value> {
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
    // Only what this ticket implements: full text sync, UTF-16 positions.
    assert_eq!(result["capabilities"]["textDocumentSync"], json!(1));
    assert_eq!(result["capabilities"]["positionEncoding"], json!("utf-16"));
    assert_eq!(result["capabilities"].get("hoverProvider"), None);
    assert_eq!(
        result["capabilities"].get("documentFormattingProvider"),
        None
    );
    assert_eq!(result["capabilities"].get("completionProvider"), None);
    assert_eq!(result["serverInfo"]["name"], json!("usfm-language-server"));

    lsp.notify("initialized", json!({}));
    lsp.notify(
        "textDocument/didOpen",
        json!({
            "textDocument": {
                "uri": uri,
                "languageId": "usfm",
                "version": 1,
                "text": WITH_UNKNOWN_MARKER,
            }
        }),
    );

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

    assert_eq!(lsp.request("shutdown", json!(null)), json!(null));
    lsp.notify("exit", json!(null));

    // The process ends on its own; `Drop` would kill it, which would hide a
    // server that ignored `exit`.
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
