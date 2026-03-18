use std::fs;

use nlf_core::lexer::TokenKind;
use nlf_core::{lexer, parser, stdlib, translator};
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

#[derive(Debug)]
struct Backend {
    client: Client,
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL)),
                completion_provider: Some(CompletionOptions {
                    resolve_provider: Some(false),
                    trigger_characters: Some(vec![".".to_string()]),
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
        let file_path = params.text_document_position.text_document.uri.to_file_path().unwrap();
        let path = file_path.to_str().unwrap();

        let contents = fs::read_to_string(path).unwrap();

        let position = params.text_document_position.position;

        let source = line_to_source(&contents, position);

        let program = parser::parse(lexer::lex(contents).unwrap()).unwrap();
        
        let symbols = translator::find_symbols_at(source as u32, program);

        let match_table = lexer::match_table();

        let mut items = vec![];

        for symbol in symbols {
            if !symbol.starts_with(|c: char| { c.is_ascii_alphabetic() }) {
                continue;
            };

            items.push(CompletionItem {
                label: symbol.clone(),
                kind: Some(CompletionItemKind::VARIABLE),
                sort_text: Some(format!("0_{}", symbol)),
                ..Default::default()
            });
        }

        self.client.show_message(MessageType::INFO, format!("Updated {}", items.len())).await;

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

        Ok(Some(CompletionResponse::Array(items)))
    }

    async fn did_save(
        &self,
        params: DidSaveTextDocumentParams,
    ) {
    
        let uri = params.text_document.uri;
        self.client.publish_diagnostics(uri.clone(), vec![], None).await;

        {
            let file_path = uri.to_file_path().unwrap();
            let path = file_path.to_str().unwrap();

            // let module = loader::resolve_module(path, pg_context);

            let contents = fs::read_to_string(path).unwrap();

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

    let (service, socket) = LspService::new(|client| Backend { client });
    Server::new(stdin, stdout, socket).serve(service).await;
}