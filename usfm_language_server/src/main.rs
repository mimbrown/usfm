mod language_config;
mod options;
mod worker;

use options::{Options, WorkspaceOption};
use worker::WorkspaceWorker;

use std::collections::HashMap;
use std::fmt::Display;
use std::fs;
use std::sync::Arc;

use data_layer::DataLayer;
use ropey::Rope;
use rusqlite::OpenFlags;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::sync::{Mutex, OnceCell, RwLock, SetError};
use tower_lsp_server::jsonrpc::{Error, ErrorCode, Result};
use tower_lsp_server::{Client, LanguageServer, LspService, Server};
use tower_lsp_server::{UriExt, lsp_types::*};
use unic::segment::WordBoundIndices;

fn offset_to_position(offset: usize, source_text: &str) -> Option<Position> {
    let rope = Rope::from_str(source_text);
    let line = rope.try_byte_to_line(offset).ok()?;
    let first_char_of_line = rope.try_line_to_char(line).ok()?;
    // Original offset is byte, but Rope uses char offset
    let offset = rope.try_byte_to_char(offset).ok()?;
    let column = offset - first_char_of_line;
    Some(Position::new(line as u32, column as u32))
}

fn position_to_offset(position: Position, source_text: &str) -> Option<usize> {
    let rope = Rope::from_str(source_text);
    let line_char = rope.try_line_to_byte(position.line.try_into().ok()?).ok()?;
    let post_text = &source_text[line_char..];
    let char: usize = position.character.try_into().ok()?;
    let line_bytes: usize = post_text
        .chars()
        .take(char)
        .map(|char| char.len_utf8())
        .sum();
    Some(line_char + line_bytes)
}

fn extract_range<'a>(range: Range, source_text: &'a str) -> Option<&'a str> {
    let start = position_to_offset(range.start, source_text)?;
    let end = position_to_offset(range.end, source_text)?;
    Some(&source_text[start..end])
}

fn get_word<'a>(offset: usize, text: &'a str) -> Option<(usize, &'a str)> {
    let pre_text = &text[0..offset];
    let mut pre_offset_bytes: usize = 0;
    for char in pre_text.chars().rev() {
        if char.is_whitespace() {
            break;
        }
        pre_offset_bytes += char.len_utf8();
    }
    let post_text = &text[offset..];
    let mut post_offset_bytes: usize = 0;
    for char in post_text.chars() {
        if char.is_whitespace() {
            break;
        }
        post_offset_bytes += char.len_utf8();
    }
    let start = offset - pre_offset_bytes;
    let end = offset + post_offset_bytes;
    let sub_str = &text[start..end];
    for (sub_offset, word) in WordBoundIndices::new(sub_str) {
        let sub_start = start + sub_offset;
        let sub_end = sub_start + word.len();
        if sub_start <= offset && sub_end > offset {
            if word.chars().any(|ch| ch.is_alphabetic()) {
                return Some((sub_start, word));
            } else {
                break;
            }
        }
    }
    None
}

// #[derive(Debug, Serialize, Deserialize, Clone)]
// #[serde(rename_all = "camelCase")]
// struct Options {
//     // run: Run,
//     enable: bool,
//     config_path: String,
// }

// impl Default for Options {
//     fn default() -> Self {
//         Self {
//             enable: true,
//             config_path: "usfm.config.json".into(),
//         }
//     }
// }

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct UsfmConfig {
    lexicon: Option<UsfmConfigLexicon>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct UsfmConfigLexicon {
    r#type: String,
    path: String,
}

struct Backend {
    client: Client,
    workspace_workers: Arc<RwLock<Vec<WorkspaceWorker>>>,
    db: Mutex<Option<DataLayer>>,
    file_cache: Mutex<HashMap<Uri, String>>,
}

impl Backend {
    async fn info<M>(&self, message: M)
    where
        M: Display,
    {
        self.client.log_message(MessageType::INFO, message).await;
    }

    async fn compute_diagnostics(&self, text: &String) -> Vec<Diagnostic> {
        let mut diagnostics = vec![];
        if let Some(db) = self.db.lock().await.as_ref() {
            for (start, word) in WordBoundIndices::new(text) {
                if word.chars().any(|ch| ch.is_alphabetic()) {
                    let has_word = db.has_lexeme(word).unwrap_or(false);
                    if !has_word {
                        diagnostics.push(Diagnostic::new(
                            Range::new(
                                offset_to_position(start, text).unwrap(),
                                offset_to_position(start + word.len(), text).unwrap(),
                            ),
                            Some(DiagnosticSeverity::WARNING),
                            Some(NumberOrString::String("missing-lexeme".into())),
                            Some("usfm".into()),
                            format!("'{}' does not exist in the lexicon", word),
                            None,
                            None,
                        ));
                    }
                }
            }
        }
        diagnostics
    }

    async fn get_text(&self, uri: Uri) -> Option<String> {
        self.file_cache
            .lock()
            .await
            .get(&uri)
            .map(|cached| cached.clone())
            .or_else(|| match fs::read_to_string(uri.to_file_path()?) {
                Ok(text) => Some(text),
                Err(_) => None,
            })
    }

    async fn has_lexicon(&self) -> bool {
        self.db.lock().await.is_some()
    }
}

impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        // let options = params
        //     .initialization_options
        //     .and_then(|mut value| serde_json::from_value::<Vec<WorkspaceOption>>(value).ok());

        let workers = if let Some(workspace_folders) = &params.workspace_folders {
            workspace_folders
                .iter()
                .map(|workspace_folder| WorkspaceWorker::new(workspace_folder.uri.clone()))
                .collect()
        // client sent deprecated root uri
        // } else if let Some(root_uri) = params.root_uri {
        //     vec![WorkspaceWorker::new(root_uri)]
        // // client is in single file mode, create no workers
        } else {
            vec![]
        };

        *self.workspace_workers.write().await = workers;

        // if let Some(options) = options {
        //     for worker in &workers {
        //         worker
        //             .init_linter(
        //                 &options
        //                     .iter()
        //                     .find(|workspace_option| {
        //                         worker.is_responsible_for_uri(&workspace_option.workspace_uri)
        //                     })
        //                     .map(|workspace_options| workspace_options.options.clone())
        //                     .unwrap_or_default(),
        //             )
        //             .await;
        //     }
        // }

        self.init_config().await;
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                code_action_provider: Some(CodeActionProviderCapability::Simple(true)),
                execute_command_provider: Some(ExecuteCommandOptions {
                    commands: vec!["add-lexeme".into()],
                    ..Default::default()
                }),
                // completion_provider: Some(CompletionOptions::default()),
                // document_range_formatting_provider: Some(OneOf::Left(true)),
                // semantic_tokens_provider: Some(SemanticTokensServerCapabilities::SemanticTokensOptions(SemanticTokensOptions {
                //     full: Some(SemanticTokensFullOptions::Bool(true)),
                //     ..Default::default()
                // })),
                // diagnostic_provider: Some(DiagnosticServerCapabilities::Options(DiagnosticOptions {
                //     identifier: Some("usfm".into()),
                //     inter_file_dependencies: false,
                //     workspace_diagnostics: false,
                //     ..Default::default()
                // })),
                ..Default::default()
            },
            ..Default::default()
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.info("server initialized!").await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        self.client
            .log_message(
                MessageType::INFO,
                format!(
                    "opened diagnostics for {}",
                    params.text_document.uri.as_str()
                ),
            )
            .await;
        self.file_cache.lock().await.insert(
            params.text_document.uri.clone(),
            params.text_document.text.clone(),
        );
        self.client
            .publish_diagnostics(
                params.text_document.uri,
                self.compute_diagnostics(&params.text_document.text).await,
                Some(params.text_document.version),
            )
            .await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        if let Some(changes) = params.content_changes.last() {
            let text = &changes.text;
            self.file_cache
                .lock()
                .await
                .insert(params.text_document.uri.clone(), text.clone());
            // cache.remove
            self.client
                .publish_diagnostics(
                    params.text_document.uri,
                    self.compute_diagnostics(text).await,
                    Some(params.text_document.version),
                )
                .await;
        }
        // let text = &params.content_changes.last().unwrap().text;
    }

    // async fn did_save(&self, params: DidSaveTextDocumentParams) {
    //     self.file_cache.lock().await.remove(&params.text_document.uri);
    // }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        self.file_cache
            .lock()
            .await
            .remove(&params.text_document.uri);
    }

    // async fn diagnostic(&self, params: DocumentDiagnosticParams) -> Result<DocumentDiagnosticReportResult> {
    //     // self.client
    //     //     .log_message(MessageType::INFO, format!("diagnostics for {}", params.text_document.uri))
    //     //     .await;
    //     let Some(content) = self.get_text(params.text_document.uri).await else {
    //         return Err(Error::invalid_params("Document URI is invalid"));
    //     };

    //     Ok(DocumentDiagnosticReportResult::Report(
    //         DocumentDiagnosticReport::Full(RelatedFullDocumentDiagnosticReport {
    //             related_documents: None,
    //             full_document_diagnostic_report: FullDocumentDiagnosticReport {
    //                 result_id: None,
    //                 items: self.compute_diagnostics(&content).await,
    //             }
    //         })
    //     ))
    // }

    // async fn semantic_tokens_full(&self, params: SemanticTokensParams) -> Result<Option<SemanticTokensResult>> {
    //     params.text_document;
    //     Ok(None)
    // }

    // async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
    //     Ok(Some(CompletionResponse::Array(vec![
    //         CompletionItem::new_simple("Hello".to_string(), "Some detail".to_string()),
    //         CompletionItem::new_simple("Bye".to_string(), "More detail".to_string())
    //     ])))
    // }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let Some(text) = self
            .get_text(params.text_document_position_params.text_document.uri)
            .await
        else {
            return Ok(None);
        };
        let position = params.text_document_position_params.position;
        let Some(offset) = position_to_offset(position, &text) else {
            return Ok(None);
        };
        let Some((word_start, word)) = get_word(offset, &text) else {
            return Ok(None);
        };
        // let pre_text = &text[0..offset];
        // let mut pre_offset_chars: u32 = 0;
        // let mut pre_offset_bytes: usize = 0;
        // for char in pre_text.chars().rev() {
        //     if !char.is_alphabetic() {
        //         break;
        //     }
        //     pre_offset_chars += 1;
        //     pre_offset_bytes += char.len_utf8();
        // }
        // let post_text = &text[offset..];
        // let mut post_offset_chars: u32 = 0;
        // let mut post_offset_bytes: usize = 0;
        // for char in post_text.chars() {
        //     if !char.is_alphabetic() {
        //         break;
        //     }
        //     post_offset_chars += 1;
        //     post_offset_bytes += char.len_utf8();
        // }
        // let word = &text[(offset - pre_offset_bytes)..(offset + post_offset_bytes)];
        if let Some(db) = self.db.lock().await.as_ref() {
            let Some(lexemes) = db.get_entries_by_lexeme(word).ok() else {
                return Ok(None);
            };
            // let Some(pre_offset_bytes): Option<u32> = pre_offset_bytes.try_into().ok() else {
            //     return Ok(None);
            // };
            Ok(Some(Hover {
                contents: HoverContents::Array(
                    lexemes
                        .iter()
                        .enumerate()
                        .map(|(index, entry)| {
                            MarkedString::from_markdown(format!(
                                "({}) **{}** *{}*\n\n{}",
                                index + 1,
                                word,
                                entry.pos,
                                entry.gloss
                            ))
                        })
                        .collect(),
                ),
                range: Some(Range::new(
                    offset_to_position(word_start, &text).unwrap(),
                    offset_to_position(word_start + word.len(), &text).unwrap(),
                    // Position::new(position.line, position.character - pre_offset_chars),
                    // Position::new(position.line, position.character + post_offset_chars),
                )),
            }))
        } else {
            Ok(None)
        }
    }

    async fn code_action(&self, params: CodeActionParams) -> Result<Option<CodeActionResponse>> {
        if self.has_lexicon().await {
            if let Some(diagnostic) = params.context.diagnostics.iter().find(|diagnostic| {
                diagnostic.code == Some(NumberOrString::String("missing-lexeme".into()))
            }) {
                if let Some(text) = self.get_text(params.text_document.uri).await {
                    if let Some(word) = extract_range(diagnostic.range, &text) {
                        return Ok(Some(vec![CodeActionOrCommand::CodeAction(CodeAction {
                            title: "Add to dictionary".into(),
                            kind: Some(CodeActionKind::QUICKFIX),
                            diagnostics: Some(vec![diagnostic.clone()]),
                            command: Some(Command {
                                title: "Add to dictionary".into(),
                                command: "add-lexeme".into(),
                                arguments: Some(vec![json!(word)]),
                            }),
                            is_preferred: Some(true),
                            ..Default::default()
                        })]));
                    }
                }
            }
        }

        Ok(None)
    }

    async fn execute_command(&self, params: ExecuteCommandParams) -> Result<Option<Value>> {
        match params.command.as_str() {
            "add-lexeme" => {
                let mut needs_refresh = false;
                if let Some(db) = self.db.lock().await.as_ref() {
                    let mut iter = params.arguments.iter();
                    let lexeme = iter
                        .next()
                        .and_then(|value| value.as_str())
                        .unwrap_or_default();
                    let gloss = iter
                        .next()
                        .and_then(|value| value.as_str())
                        .unwrap_or_default();
                    let pos = iter
                        .next()
                        .and_then(|value| value.as_str())
                        .unwrap_or_default();
                    db.add_entry(lexeme, gloss, pos)
                        .map_err(|_| Error::internal_error())?;
                    needs_refresh = true;
                }
                if needs_refresh {
                    for (uri, text) in self.file_cache.lock().await.iter() {
                        self.client
                            .publish_diagnostics(
                                uri.clone(),
                                self.compute_diagnostics(text).await,
                                None,
                            )
                            .await;
                    }
                }
                // self.client.publish_diagnostics(
                //     params..uri,
                //     self.compute_diagnostics(text).await,
                //     Some(params.text_document.version)
                // ).await;
                // self.client.show_message_request(typ, message, actions)
                // self.client
                //     .log_message(MessageType::INFO, format!("got add lexeme command: {:?}", params.arguments))
                //     .await;
            }
            _ => {}
        }
        Ok(None)
    }

    // async fn formatting(&self, params: DocumentFormattingParams) -> Result<Option<Vec<TextEdit>>> {
    //     // params.;
    //     Ok(None)
    // }
}

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(|client| Backend {
        client,
        db: Mutex::new(None),
        file_cache: Mutex::new(HashMap::new()),
        workspace_workers: Arc::new(RwLock::new(vec![])),
    });
    Server::new(stdin, stdout, socket).serve(service).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_word_from_text() {
        let text = "ã jao kʰã lo";
        assert_eq!(get_word(7, text), None);
        assert_eq!(get_word(8, text), Some((8, "kʰã")));
        assert_eq!(get_word(12, text), Some((8, "kʰã")));
        assert_eq!(get_word(14, text), None);
    }
}
