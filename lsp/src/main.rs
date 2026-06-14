use std::collections::HashMap;

use nlf_core::analysis::{ScopeBuilder, ScopeId, SymbolIndex, SymbolKind};
use nlf_core::lexer::TokenKind;
use nlf_core::{explorer, lexer, parser, stdlib, translator};
use tokio::sync::RwLock;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

#[derive(Debug)]
struct Backend {
    client: Client,
    documents: RwLock<HashMap<Url, String>>
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL)),
                completion_provider: Some(CompletionOptions {
                    resolve_provider: Some(false),
                    trigger_characters: Some(vec![".".to_string(), "\"".to_string()]),
                    ..Default::default()
                }),
                ..Default::default()
            },
            server_info: None,
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "server initialized!")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }
    
    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let scope_id;

        let items = {
            let uri = params.text_document_position.text_document.uri;
            let docs = self.documents.read().await;

            let contents = docs.get(&uri).unwrap().to_string();
            let position = params.text_document_position.position;
            let source = line_to_source(&contents, position);
            
            let tokens = match lexer::lex(contents) {
                Ok(tokens) => tokens,
                Err(_) => return Ok(None)
            };

            let program = match parser::parse(tokens) {
                Ok(program) => program,
                Err(_) => return Ok(None)
            };

            let index = {
                let mut scope_builder = ScopeBuilder::new();

                explorer::visit_program(&program, &mut scope_builder);

                SymbolIndex::from(scope_builder)
            };
            
            let scope = index.scope_at_position(source);
            scope_id = scope;
            let symbols = index.visible_symbols(scope);
            let match_table = lexer::match_table();

            let mut items = vec![];

            for symbol in symbols {
                if !symbol.name.starts_with(|c: char| { c.is_ascii_alphabetic() }) {
                    continue;
                }

                let kind: CompletionItemKind = match symbol.kind {
                    SymbolKind::Variable => CompletionItemKind::VARIABLE,
                    SymbolKind::Function => CompletionItemKind::FUNCTION,
                    SymbolKind::Class => CompletionItemKind::CLASS,
                    _ => todo!()
                };

                items.push(CompletionItem {
                    label: symbol.name.clone(),
                    kind: Some(kind),
                    sort_text: Some(format!("0_{}", symbol.name)),
                    ..Default::default()
                });
            }

            for function in stdlib::FUNCTION_TABLE.lock().keys() {
                items.push(CompletionItem {
                    label: function.to_string(),
                    kind: Some(CompletionItemKind::FUNCTION),
                    detail: Some("Standard library function".to_string()),
                    sort_text: Some(format!("1_{}", function.to_string())),
                    ..Default::default()
                });
            }

            for (token, kind) in match_table {
                if matches!(kind, TokenKind::Keyword(_) | TokenKind::BooleanLiteral { .. }) {
                    items.push(CompletionItem {
                        label: token.to_string(),
                        kind: Some(CompletionItemKind::KEYWORD),
                        sort_text: Some(format!("2_{}", token)),
                        ..Default::default()
                    });
                }
            }

            items
        };

        self.client.show_message(MessageType::INFO, format!("Updated {} - Scope: {}", items.len(), scope_id.0)).await;

        Ok(Some(CompletionResponse::Array(items)))
    }

    async fn did_save(
        &self,
        params: DidSaveTextDocumentParams,
    ) {
    
        let uri = params.text_document.uri;
        self.client.publish_diagnostics(uri.clone(), vec![], None).await;

        {
            let docs = self.documents.read().await;

            let contents = docs.get(&uri).unwrap().to_string();

            let tokens = match lexer::lex(contents.clone()) {
                Ok(tokens) => tokens,
                Err(err) => {
                    let bindings = err.source_bindings.unwrap();
                    let diagnostics = vec![
                        Diagnostic {
                            range: Range {
                                start: source_to_line(&contents, bindings.0),
                                end: source_to_line(&contents, bindings.1),
                            },
                            severity: Some(DiagnosticSeverity::ERROR),
                            message: format!("{:?}", err.kind),
                            ..Default::default()
                        }
                    ];

                    self.client.publish_diagnostics(uri, diagnostics, None).await;
                    return;
                }
            };
            
            let mut outer_diagnostics: Option<Vec<_>> = None;
            let mut outer_error: Option<_> = None;

            match parser::parse(tokens) {
                Ok(ast) => {
                    match translator::translate(ast) {
                        Ok(_) => (),
                        Err(err) => {
                            outer_error = Some(format!("Translation error: {:?}", err.kind));
                        }
                    }
                },
                Err(err) => {
                    let bindings = err.source_bindings.unwrap();
                    let diagnostics = vec![
                        Diagnostic {
                            range: Range {
                                start: source_to_line(&contents, bindings.0),
                                end: source_to_line(&contents, bindings.1),
                            },
                            severity: Some(DiagnosticSeverity::ERROR),
                            message: format!("{:?}", err.kind),
                            ..Default::default()
                        }
                    ];

                    outer_diagnostics = Some(diagnostics);
                }
            };

            if let Some(diagnostics) = outer_diagnostics {
                self.client.publish_diagnostics(uri.clone(), diagnostics, None).await;
            };

            if let Some(message) = outer_error {
                self.client.show_message(MessageType::ERROR, message).await;
            }
        }
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let mut docs = self.documents.write().await;

        docs.insert(params.text_document.uri, params.text_document.text);
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let mut docs = self.documents.write().await;

        if let Some(doc) = docs.get_mut(&params.text_document.uri) {
            if let Some(change) = params.content_changes.first() {
                *doc = change.text.clone();
            }
        }
    }
    
    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let mut docs = self.documents.write().await;
        docs.remove(&params.text_document.uri);
    }
}

fn source_to_line(contents: &String, position: usize) -> Position {
    let mut cursor = 0;

    for (line_index, line) in contents.lines().enumerate() {
        let line_end = cursor + line.len();

        if position <= line_end {
            let character = position.checked_sub(cursor).unwrap_or(0) as u32;
            return Position::new(line_index as u32, character);
        }

        cursor = line_end + 2;
    }

    Position::new(contents.lines().count() as u32, 0)
}

fn line_to_source(contents: &String, position: Position) -> usize {
    let mut cursor = 0;

    for (line_index, line) in contents.lines().enumerate() {
        let line_end = cursor + line.len();

        if position.line == line_index as u32 {
            return cursor + position.character as usize
        }

        cursor = line_end + 2;
    }

    0
}

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(|client| Backend { client, documents: RwLock::new(HashMap::new()) });
    Server::new(stdin, stdout, socket).serve(service).await;
}