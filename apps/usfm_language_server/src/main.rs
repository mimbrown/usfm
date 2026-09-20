//! `usfm-language-server`: the USFM toolchain over the Language Server
//! Protocol.
//!
//! The server the VS Code extension in `vscode/` spawns. It is the rebuild
//! M6 asks for (ticket 30): the parked `wip/usfm_language_server` checked
//! words against a SQLite lexicon with regexes and never saw a parse tree,
//! while this one is a thin shell around the facade — every diagnostic it
//! publishes is `usfm::parse_with`'s, which is the parser's repairs and
//! `usfm_semantic`'s checks together, the same list `usfm parse` prints.
//!
//! This ticket is diagnostics and nothing else. Three modules hold the parts
//! the rest of M6 reuses: [`documents`] keeps the open text, [`convert`] turns
//! a byte [`Span`](usfm::Span) into a protocol range, and [`stylesheet`]
//! decides which `.sty` a document is parsed with.
//!
//! Nothing is debounced. A parse of a whole book is a few milliseconds
//! (`docs/benchmarks.md`: the corpus parses at tens of MiB/s, and the largest
//! book in it is under a megabyte), so a keystroke's parse finishes long
//! before the next keystroke arrives; a debounce would only add latency to
//! measure later.

mod convert;
mod documents;
mod stylesheet;

use std::path::PathBuf;

use tokio::sync::Mutex;
use tower_lsp_server::jsonrpc::Result;
use tower_lsp_server::ls_types::{
    DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
    InitializeParams, InitializeResult, InitializedParams, MessageType, PositionEncodingKind,
    ServerCapabilities, ServerInfo, TextDocumentSyncCapability, TextDocumentSyncKind, Uri,
};
use tower_lsp_server::{Client, LanguageServer, LspService, Server};

use usfm::span::LineIndex;

use documents::Documents;
use stylesheet::Stylesheets;

/// The server's state: the open documents and the stylesheets read so far.
struct Backend {
    client: Client,
    documents: Documents,
    /// A `Mutex` and not an `RwLock`: reading a sheet also *records* it, so
    /// every use of this is a write.
    stylesheets: Mutex<Stylesheets>,
}

impl Backend {
    fn new(client: Client) -> Self {
        Self {
            client,
            documents: Documents::default(),
            stylesheets: Mutex::new(Stylesheets::default()),
        }
    }

    /// Parse the document at `uri` and publish what the toolchain had to say
    /// about it.
    ///
    /// The text comes from the store rather than from the notification, so
    /// that what is published is always what the server believes the file to
    /// be — the same text every later feature will format and hover over.
    ///
    /// The list is always published, empty included: an empty list is how the
    /// protocol says "this file is clean now", so the editor clears the
    /// squiggles a previous version left.
    async fn publish_diagnostics(&self, uri: &Uri, version: Option<i32>) {
        let Some(text) = self.documents.text(uri).await else {
            return;
        };
        let (sheet, warning) = {
            let path = uri.to_file_path();
            self.stylesheets.lock().await.for_document(path.as_deref())
        };
        if let Some(warning) = warning {
            // Never fatal: the document is parsed with the default sheet, and
            // the editor's user is told why their project's markers are
            // unknown. The `Stylesheets` cache makes sure this is said once.
            self.client
                .show_message(MessageType::WARNING, warning)
                .await;
        }

        let result = usfm::parse_with(&text, &sheet);
        let index = LineIndex::new(&text);
        let diagnostics = result
            .diagnostics
            .iter()
            .map(|diagnostic| convert::diagnostic(&index, diagnostic))
            .collect();
        self.client
            .publish_diagnostics(uri.clone(), diagnostics, version)
            .await;
    }
}

impl LanguageServer for Backend {
    /// The capabilities are what this ticket implements and no more: full text
    /// synchronisation, so the server is handed the whole document on every
    /// change, and the positions it publishes are UTF-16 (the protocol's
    /// default, said out loud because [`convert`] depends on it).
    ///
    /// Formatting and hover arrive with ticket 31, symbols, completion and
    /// code actions with 32. Advertising them before they work would only make
    /// the editor ask questions this server answers with `null`.
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        // `initializationOptions.stylesheet`: a path to the project's `.sty`.
        // Anything else the client sends is ignored rather than refused — an
        // older client sends the parked server's options, and a server that
        // refuses to start over an unknown key helps nobody.
        let stylesheet = params
            .initialization_options
            .as_ref()
            .and_then(|options| options.get("stylesheet"))
            .and_then(|value| value.as_str())
            .map(str::to_owned);
        let root = workspace_root(&params);
        self.stylesheets
            .lock()
            .await
            .configure(stylesheet.as_deref(), root);

        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                position_encoding: Some(PositionEncodingKind::UTF16),
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                ..Default::default()
            },
            server_info: Some(ServerInfo {
                name: "usfm-language-server".to_owned(),
                version: Some(env!("CARGO_PKG_VERSION").to_owned()),
            }),
            ..Default::default()
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "usfm-language-server ready")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let document = params.text_document;
        self.documents.set(&document.uri, document.text).await;
        self.publish_diagnostics(&document.uri, Some(document.version))
            .await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        // Full synchronisation: the last change is the whole new document.
        let Some(change) = params.content_changes.into_iter().next_back() else {
            return;
        };
        let uri = params.text_document.uri;
        self.documents.set(&uri, change.text).await;
        self.publish_diagnostics(&uri, Some(params.text_document.version))
            .await;
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri;
        self.documents.remove(&uri).await;
        // The server no longer tracks the file, so it can no longer say
        // anything true about it: clear what it published.
        self.client.publish_diagnostics(uri, Vec::new(), None).await;
    }
}

/// The workspace root a relative `stylesheet` option is resolved against.
///
/// The first workspace folder, or the deprecated `rootUri` that a client
/// without folders still sends. A client in single-file mode sends neither,
/// and then a relative path is resolved against the document instead
/// ([`Stylesheets::for_document`]).
fn workspace_root(params: &InitializeParams) -> Option<PathBuf> {
    let folder = params
        .workspace_folders
        .as_ref()
        .and_then(|folders| folders.first())
        .map(|folder| &folder.uri);
    #[allow(
        deprecated,
        reason = "`root_uri` is how a client without folders says it"
    )]
    let uri = folder.or(params.root_uri.as_ref())?;
    Some(uri.to_file_path()?.into_owned())
}

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let (service, socket) = LspService::new(Backend::new);
    Server::new(stdin, stdout, socket).serve(service).await;
}
