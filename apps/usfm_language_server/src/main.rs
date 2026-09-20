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
//! What it answers: diagnostics on every change (ticket 30); since ticket 31
//! `textDocument/formatting` — the document written back out by
//! `usfm_codegen`, the same text `usfm format` writes — and
//! `textDocument/hover`, which is the stylesheet's own words about the marker
//! under the cursor, or the reference (`GEN 1:1`) where the cursor is in a
//! verse; and since ticket 32 `textDocument/documentSymbol` (the outline of
//! chapters and verses), `textDocument/completion` (the markers that may be
//! written where the cursor is) and `textDocument/codeAction` (the quick
//! fixes for the repairs the parser reports).
//!
//! A module per part, so that each is a pure function with tests of its own
//! and this file stays the wiring: [`documents`] keeps the open text,
//! [`convert`] turns a byte [`Span`](usfm::Span) into a protocol range and a
//! protocol position back into an offset, [`stylesheet`] decides which `.sty`
//! a document is parsed with, [`locate`] finds the node under a position (and
//! the reference it is in), [`hover`] writes the markdown for it, [`format`]
//! holds the formatter and the rule under which it refuses, [`symbols`]
//! builds the outline, [`completion`] the marker list and [`actions`] the
//! quick fixes.
//!
//! Nothing is debounced. A parse of a whole book is a few milliseconds
//! (`docs/benchmarks.md`: the corpus parses at tens of MiB/s, and the largest
//! book in it is under a megabyte), so a keystroke's parse finishes long
//! before the next keystroke arrives; a debounce would only add latency to
//! measure later.

mod actions;
mod completion;
mod convert;
mod documents;
mod format;
mod hover;
mod locate;
mod stylesheet;
mod symbols;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::Mutex;
use tower_lsp_server::jsonrpc::Result;
use tower_lsp_server::ls_types::{
    CodeAction, CodeActionKind, CodeActionOptions, CodeActionOrCommand, CodeActionParams,
    CodeActionProviderCapability, CodeActionResponse, CompletionOptions, CompletionParams,
    CompletionResponse, DidChangeTextDocumentParams, DidCloseTextDocumentParams,
    DidOpenTextDocumentParams, DocumentFormattingParams, DocumentSymbolParams,
    DocumentSymbolResponse, Hover, HoverContents, HoverParams, HoverProviderCapability,
    InitializeParams, InitializeResult, InitializedParams, MarkupContent, MarkupKind, MessageType,
    NumberOrString, OneOf, PositionEncodingKind, ServerCapabilities, ServerInfo,
    TextDocumentSyncCapability, TextDocumentSyncKind, TextEdit, Uri, WorkspaceEdit,
};
use tower_lsp_server::{Client, LanguageServer, LspService, Server};

use usfm::StyleSheet;
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
    /// The text of `uri` and the stylesheet it is parsed with, or `None` when
    /// the client has not opened it.
    ///
    /// Every feature starts here, so that a hover, a formatting request and
    /// the published diagnostics all describe the same text parsed the same
    /// way. A stylesheet that could not be read is reported as it is read,
    /// which the `Stylesheets` cache makes happen once per path.
    async fn source(&self, uri: &Uri) -> Option<(String, Arc<StyleSheet>)> {
        let text = self.documents.text(uri).await?;
        let (sheet, warning) = {
            let path = uri.to_file_path();
            self.stylesheets.lock().await.for_document(path.as_deref())
        };
        if let Some(warning) = warning {
            // Never fatal: the document is parsed with the default sheet, and
            // the editor's user is told why their project's markers are
            // unknown.
            self.client
                .show_message(MessageType::WARNING, warning)
                .await;
        }
        Some((text, sheet))
    }

    async fn publish_diagnostics(&self, uri: &Uri, version: Option<i32>) {
        let Some((text, sheet)) = self.source(uri).await else {
            return;
        };

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
    /// The capabilities are what is implemented and no more: full text
    /// synchronisation, so the server is handed the whole document on every
    /// change; UTF-16 positions (the protocol's default, said out loud because
    /// [`convert`] depends on it); whole-document formatting; hover; and,
    /// since ticket 32, document symbols, completion after a `\` and
    /// `quickfix` code actions.
    ///
    /// Each one is advertised only once it answers: a capability the server
    /// claims and then answers with `null` is a question the editor asks for
    /// nothing.
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
                // Whole-document formatting only: `rangeFormatting` would have
                // to write a fragment of USFM, and the writer's unit is a
                // document. This is what `editor.formatOnSave` asks for.
                document_formatting_provider: Some(OneOf::Left(true)),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                document_symbol_provider: Some(OneOf::Left(true)),
                // `\` opens every marker, so it is the one character that
                // should pop the list up on its own. A client that asks
                // explicitly (`Ctrl+Space`) is answered the same way, from
                // whatever has been typed after the `\`.
                completion_provider: Some(CompletionOptions {
                    trigger_characters: Some(vec!["\\".to_owned()]),
                    // Every item is complete when it is sent: the sheet's name
                    // and description are already in hand, so there is nothing
                    // for a resolve round trip to fetch.
                    resolve_provider: Some(false),
                    ..Default::default()
                }),
                // Only `quickfix`: every action here repairs a diagnostic.
                // Saying so lets the editor skip the request when it is
                // collecting refactorings.
                code_action_provider: Some(CodeActionProviderCapability::Options(
                    CodeActionOptions {
                        code_action_kinds: Some(vec![CodeActionKind::QUICKFIX]),
                        resolve_provider: Some(false),
                        ..Default::default()
                    },
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

    /// The whole document, rewritten by `usfm_codegen`, as one edit.
    ///
    /// One edit and not a minimal diff: the writer produces a text, not a
    /// patch, and a diff of the two would be a guess at which lines
    /// correspond. If a client flickers on it, a diff is a follow-up
    /// (ticket 31).
    ///
    /// **A refusal is `null`, not an empty list.** `[]` means "already
    /// formatted, nothing to change", which is not true here and would leave
    /// the editor silently doing nothing on save; `null` is the protocol's
    /// "no result", and the reason goes to the user as a `window/showMessage`
    /// warning, which is what the CLI prints on stderr in the same case.
    async fn formatting(&self, params: DocumentFormattingParams) -> Result<Option<Vec<TextEdit>>> {
        let uri = params.text_document.uri;
        let Some((text, sheet)) = self.source(&uri).await else {
            return Ok(None);
        };
        let result = usfm::parse_with(&text, &sheet);
        if let Some(refusal) = format::refusal(&result.diagnostics) {
            self.client
                .show_message(MessageType::WARNING, refusal)
                .await;
            return Ok(None);
        }

        let formatted = format::formatted(&result.document);
        if formatted == text {
            // Nothing to do, and now `[]` is the honest answer.
            return Ok(Some(Vec::new()));
        }
        let index = LineIndex::new(&text);
        Ok(Some(vec![TextEdit {
            range: convert::whole_document(&index, &text),
            new_text: formatted,
        }]))
    }

    /// What the stylesheet and the tree say about the position hovered.
    ///
    /// See [`hover`] for the rule; the range is the located node's span, so
    /// the editor underlines the marker's whole construct rather than the word
    /// under the pointer.
    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let position = params.text_document_position_params;
        let Some((text, sheet)) = self.source(&position.text_document.uri).await else {
            return Ok(None);
        };
        let offset = convert::offset(&text, position.position);
        let result = usfm::parse_with(&text, &sheet);
        let Some(hover) = hover::hover(&result.document, offset) else {
            return Ok(None);
        };
        let index = LineIndex::new(&text);
        Ok(Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: hover.markdown,
            }),
            range: Some(convert::range(&index, hover.span)),
        }))
    }

    /// The outline: the book, its chapters and their verses, with sidebars
    /// and `\periph` divisions where they stand. See [`symbols`].
    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> Result<Option<DocumentSymbolResponse>> {
        let Some((text, sheet)) = self.source(&params.text_document.uri).await else {
            return Ok(None);
        };
        let result = usfm::parse_with(&text, &sheet);
        let index = LineIndex::new(&text);
        // Nested, not flat: the protocol's two shapes, and the nested one is
        // the tree the Outline view draws.
        Ok(Some(DocumentSymbolResponse::Nested(symbols::symbols(
            &result.document,
            &index,
        ))))
    }

    /// The markers that may be written where the cursor is. See
    /// [`completion`].
    ///
    /// `null` rather than an empty list where the cursor is not after a `\`:
    /// an empty list is a claim that nothing may be written there, which
    /// would stop the editor falling back to its own word completion.
    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let position = params.text_document_position;
        let Some((text, sheet)) = self.source(&position.text_document.uri).await else {
            return Ok(None);
        };
        let offset = convert::offset(&text, position.position);
        let result = usfm::parse_with(&text, &sheet);
        let index = LineIndex::new(&text);
        Ok(
            completion::completions(&result.document, &text, &index, offset)
                .map(CompletionResponse::Array),
        )
    }

    /// The quick fixes for the diagnostics the request carries. See
    /// [`actions`].
    ///
    /// The request's diagnostics are the editor's copy of what this server
    /// published, so each is matched back to the parse's own by code *and*
    /// range and the edit is computed from the toolchain's span — never from
    /// the protocol range, which would have to be converted back to an offset
    /// and could name a place the tree knows nothing about.
    async fn code_action(&self, params: CodeActionParams) -> Result<Option<CodeActionResponse>> {
        let uri = params.text_document.uri;
        let Some((text, sheet)) = self.source(&uri).await else {
            return Ok(None);
        };
        let result = usfm::parse_with(&text, &sheet);
        let index = LineIndex::new(&text);

        let wanted = |diagnostic: &usfm::Diagnostic| {
            params.context.diagnostics.iter().any(|reported| {
                reported.code == Some(NumberOrString::String(diagnostic.code.to_string()))
                    && reported.range == convert::range(&index, diagnostic.span)
            })
        };

        let actions = result
            .diagnostics
            .iter()
            .filter(|diagnostic| wanted(diagnostic))
            .filter_map(|diagnostic| {
                let fix = actions::fix(&result.document, &text, diagnostic)?;
                let edits = fix
                    .edits
                    .into_iter()
                    .map(|edit| TextEdit {
                        range: convert::range(&index, edit.span),
                        new_text: edit.text,
                    })
                    .collect();
                Some(CodeActionOrCommand::CodeAction(CodeAction {
                    title: fix.title,
                    kind: Some(CodeActionKind::QUICKFIX),
                    diagnostics: Some(vec![convert::diagnostic(&index, diagnostic)]),
                    edit: Some(WorkspaceEdit {
                        changes: Some(HashMap::from([(uri.clone(), edits)])),
                        ..Default::default()
                    }),
                    // Each fix is the only sensible edit for its diagnostic,
                    // which is what `isPreferred` means: `Ctrl+.` applies it
                    // without a menu.
                    is_preferred: Some(true),
                    ..Default::default()
                }))
            })
            .collect();
        Ok(Some(actions))
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
