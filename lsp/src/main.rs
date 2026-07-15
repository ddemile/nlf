use std::collections::HashMap;
use std::fs;
use std::pin::Pin;
use std::str::FromStr;
use std::sync::Arc;

use nlf_core::analysis::{self, CompletionResolver, Symbol, SymbolIndex, SymbolKind, TypedScopeBuilder};
use nlf_core::interpreter::prototypes::{self, BuiltInPrototype};
use nlf_core::lexer::TokenKind;
use nlf_core::loader::{self, ModuleKind};
use nlf_core::parser::{FunctionType, Type, TypeArena, TypeId};
use nlf_core::stdlib::{CoreModules, FUNCTION_TABLE};
use nlf_core::{explorer, lexer, parser, stdlib, translator, type_checker};
use parking_lot::Mutex;
use serde::Deserialize;
use serde_json::Value;
use tokio::sync::RwLock;
use tower_lsp::jsonrpc::{self, Result};
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

#[derive(Debug)]
struct Backend {
    client: Client,
    documents: RwLock<HashMap<Url, String>>
}

const LINE_ENDING_LENGTH: usize = if cfg!(target_os = "windows") { 2 } else { 1 };

#[derive(Debug, Deserialize)]
struct TextDocumentContentParams {
    uri: Url,
}

impl Backend {
    async fn get_file_contents(&self, uri: &Url) -> String {
        let docs = self.documents.read().await;

        if docs.contains_key(uri) {
            return docs.get(uri).unwrap().to_string()
        }
        
        fs::read_to_string(uri.to_file_path().unwrap()).unwrap()
    }

    async fn resolve_import(&self, name: &str, source: &str, uri: &Url) -> Option<(Symbol, Url, String)> {
        let module_source = loader::resolve_module(source.into(), Some(uri.to_file_path().unwrap().parent().unwrap().to_path_buf())).unwrap();

        let uri = match module_source.kind {
            ModuleKind::Core => Url::from_str(&format!("nlf://core/{}.nlf", module_source.path.strip_prefix("core:")?)).ok()?,
            ModuleKind::Standard => Url::from_file_path(module_source.path.clone()).ok()?.clone(),
            ModuleKind::Library => todo!()
        };
        
        let contents = if self.documents.read().await.contains_key(&uri) {
            self.get_file_contents(&uri).await
        } else {
            loader::read_module(&module_source).ok()?
        };

        let imported_index = analysis::get_symbol_index(module_source.path.into(), contents.clone())?;

        let source_symbol = imported_index.find_export(name)?;

        Some((source_symbol, uri, contents))
    }

    async fn handle_content_request(
        &self,
        params: TextDocumentContentParams,
    ) -> jsonrpc::Result<Value> {
        let uri = params.uri;

        let Some(host) = uri.host_str() else {
            return Err(jsonrpc::Error {
                code: jsonrpc::ErrorCode::InvalidParams,
                message: format!("Failed to get url host").into(),
                data: None,
            })
        };

        if host != "core" {
            return Err(jsonrpc::Error {
                code: jsonrpc::ErrorCode::InvalidParams,
                message: format!("{} host is not allowed", host).into(),
                data: None,
            })
        }
        
        let module_name = uri.path().trim_start_matches("/");

        let Some(file) = CoreModules::get(module_name) else {
            return Err(jsonrpc::Error {
                code: jsonrpc::ErrorCode::InvalidParams,
                message: format!("Core file not found: {}", module_name).into(),
                data: None,
            })
        };

        let contents = String::from_utf8(file.data.to_vec()).or_else(|_| {
            Err(jsonrpc::Error {
                code: jsonrpc::ErrorCode::InvalidParams,
                message: format!("Failed to convert file data to text").into(),
                data: None,
            })
        })?;

        Ok(serde_json::json!({ "text": contents }))
    }
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
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                definition_provider: Some(OneOf::Left(true)),
                semantic_tokens_provider: Some(
                    SemanticTokensServerCapabilities::SemanticTokensOptions(
                        SemanticTokensOptions {
                            legend: SemanticTokensLegend {
                                token_types: vec![
                                    "variable".into(),
                                    "function".into(),
                                    "class".into(),
                                    "type".into(),
                                    "keyword".into(),
                                ],
                                token_modifiers: vec![
                                    "declaration".into(),
                                    "readonly".into(),
                                    "static".into(),
                                ],
                            },
                            full: Some(SemanticTokensFullOptions::Bool(true)),
                            ..Default::default()
                        },
                    ),
                ),
                ..Default::default()
            },
            server_info: Some(ServerInfo {
                name: "nlf".to_string(),
                version: None,
            })
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "server initialized!")
            .await;

        self.client
            .send_notification::<tower_lsp::lsp_types::notification::ShowMessage>(
                tower_lsp::lsp_types::ShowMessageParams {
                    typ: MessageType::INFO,
                    message: "NLF Language Server initialized".into(),
                },
            )
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }
    
    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let scope_id;

        let items = {
            let mut completion_resolver = CompletionResolver::default();

            let uri = params.text_document_position.text_document.uri;

            let contents = self.get_file_contents(&uri).await;

            let position = params.text_document_position.position;
            let source = line_to_source(&contents, position);

            let tokens = match lexer::lex(contents) {
                Ok(tokens) => tokens,
                Err(_) => return Ok(None)
            };

            let index = {
                let program = match parser::lax_parse(tokens) {
                    Ok(program) => program,
                    Err(_) => return Ok(None)
                };
                
                let typed_program = match type_checker::check_types(program, uri.to_file_path().unwrap()) {
                    Ok(typed_program) => typed_program,
                    Err(_) => return Ok(None)
                };

                explorer::visit_program(&typed_program, &mut completion_resolver);

                let type_arena = Arc::new(Mutex::new(TypeArena::new()));
                type_arena.lock().register_defaults();

                let mut scope_builder = TypedScopeBuilder::new(type_arena);

                explorer::visit_program(&typed_program, &mut scope_builder);

                let index = SymbolIndex::from(scope_builder);

                index
            };

            fn get_completion<'a>(backend: &'a Backend, definition: &'a Symbol, uri: &'a Url, index: &'a SymbolIndex) -> Pin<Box<dyn Future<Output = CompletionResponse> + Send + 'a>> {
                Box::pin(async move {
                    let mut items = vec![];
                    match &definition.kind {
                        SymbolKind::Definition(ty) => {
                            let arena = index.arena.lock();
                            let ty = arena.get(*ty);

                            fn get_prototype_completions(prototype: &BuiltInPrototype) -> CompletionResponse {
                                CompletionResponse::Array(prototype.methods.iter().map(|(name, _)| {
                                    CompletionItem {
                                        label: name.clone(),
                                        kind: Some(CompletionItemKind::METHOD),
                                        ..Default::default()
                                    }
                                }).collect())
                            }

                            match ty {
                                Type::Instance { class: class_type } => {
                                    for (method_name, _) in class_type.methods.iter() {
                                        if *method_name == class_type.name {
                                            continue;
                                        }
                                        
                                        items.push(CompletionItem {
                                            label: method_name.clone(),
                                            kind: Some(CompletionItemKind::METHOD),
                                            ..Default::default()
                                        });
                                    }

                                    for field in class_type.fields.iter() {
                                        items.push(CompletionItem {
                                            label: field.name.clone(),
                                            kind: Some(CompletionItemKind::FIELD),
                                            ..Default::default()
                                        });
                                    }
                                }
                                Type::String => return get_prototype_completions(&prototypes::STRING_PROTOTYPE),
                                Type::Number => return get_prototype_completions(&prototypes::NUMBER_PROTOTYPE),
                                Type::Array(_) => return get_prototype_completions(&prototypes::ARRAY_PROTOTYPE),
                                Type::Object(entries) => {
                                    for key in entries.keys() {
                                        items.push(CompletionItem {
                                            label: key.to_string(),
                                            kind: Some(CompletionItemKind::FIELD),
                                            ..Default::default()
                                        });
                                    }
                                },
                                _ => {}
                            }
                        }
                        SymbolKind::Import(source_path) => {
                            let Some((source_symbol, _, _)) = backend.resolve_import(&definition.name, source_path, &uri).await else {
                                panic!()
                            };

                            return get_completion(backend, &source_symbol, uri, index).await
                        }
                        _ => {}
                    }
                    CompletionResponse::Array(items)
                })
            }

            for candidate in completion_resolver.candidates {
                if source >= candidate.completion_span.start && source <= candidate.completion_span.end {
                    if let Some(Symbol { name, kind: SymbolKind::Variable, .. }) = index.symbol_at(candidate.object_span.start) {
                        if let Some(definition) = index.find_definition(name, candidate.object_span.start) {
                            return Ok(Some(get_completion(&self, definition, &uri, &index).await))
                        }
                    }
                }
            }

            let scope = index.scope_at_position(source);
            scope_id = scope;
            let symbols = index.visible_symbols(scope);
            let match_table = lexer::match_table();

            let mut items = vec![];

            for symbol in symbols {
                if !symbol.name.starts_with(|c: char| { c.is_ascii_alphabetic() }) {
                    continue;
                }

                if matches!(symbol.kind, SymbolKind::Variable) {
                    continue;
                }

                let kind: CompletionItemKind = match symbol.kind {
                    SymbolKind::Definition(_) | SymbolKind::Import(_) => CompletionItemKind::VARIABLE,
                    SymbolKind::Function { .. } => CompletionItemKind::FUNCTION,
                    SymbolKind::Class(_) => CompletionItemKind::CLASS,
                    _ => continue
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

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        let contents = self.get_file_contents(&uri).await;

        let index = match analysis::get_symbol_index(uri.to_file_path().unwrap(), contents.clone()) {
            Some(index) => index,
            None => return Ok(None)
        };

        let source = line_to_source(&contents, position);

        let symbol = index.symbol_at(source);

        if let Some(symbol) = symbol {
            fn get_type(type_id: TypeId, arena: Arc<Mutex<TypeArena>>) -> Type {
                arena.lock().get(type_id).clone()
            }

            fn get_hover_text<'a>(symbol: &'a Symbol, index: &'a SymbolIndex, source: usize, uri: &'a Url, backend: &'a Backend) -> std::pin::Pin<Box<dyn std::future::Future<Output = String> + Send + 'a>> {
                Box::pin(async move {
                    match &symbol.kind {
                        SymbolKind::Definition(ty) => {
                            let ty = get_type(*ty, index.arena.clone());

                            format!("let {}: {}", symbol.name, ty.to_string())
                        },
                        SymbolKind::Variable => {
                            let definition = index.find_definition(&symbol.name, source);
                            
                            if let Some(definition) = definition {
                                get_hover_text(definition, index, source, uri, backend).await
                            } else if let Some(_) = FUNCTION_TABLE.lock().get(symbol.name.as_str()) {
                                format!("// Native function\nfn {}(): unknown", symbol.name)
                            } else {
                                format!("{}", symbol.name)
                            }
                        },
                        SymbolKind::Function(FunctionType { arguments: args, return_ty }) => {
                            let return_ty = get_type(*return_ty, index.arena.clone());

                            format!(
                                "fn {}({}): {}",
                                symbol.name,
                                args.iter().map(|arg| format!("{}: {}", arg.variable.value.clone(), get_type(arg.ty, index.arena.clone()).to_string())).collect::<Vec<String>>().join(", "),
                                return_ty.to_string()
                            )
                        },
                        SymbolKind::Class(_) => format!("class {}", symbol.name),
                        SymbolKind::Import(source_path) => {
                            let Some((source_symbol, _, _)) = backend.resolve_import(&symbol.name, source_path, uri).await else {
                                return format!("// Failed to resolve {}", symbol.name)
                            };

                            get_hover_text(&source_symbol, index, source, uri, backend).await
                        },
                        SymbolKind::Property(object_pos) => {
                            let source_symbol = index.symbol_at(*object_pos).unwrap();

                            let definition = index.find_definition(&source_symbol.name, source_symbol.span.start).unwrap();

                            let SymbolKind::Definition(type_id) = definition.kind else {
                                return format!("{}", symbol.name);
                            };

                            let ty = get_type(type_id, index.arena.clone());

                            match ty {
                                Type::Instance { class: class_type } => {
                                    if let Some(FunctionType { arguments: args, return_ty }) = class_type.methods.get(&symbol.name) {
                                        return format!(
                                            "{}.{}({}): {}",
                                            class_type.name,
                                            symbol.name,
                                            args.iter().map(|arg| format!("{}: {}", arg.variable.value.clone(), get_type(arg.ty, index.arena.clone()).to_string())).collect::<Vec<String>>().join(", "),
                                            get_type(*return_ty, index.arena.clone()).to_string()
                                        )
                                    } else if let Some(field) = class_type.fields.iter().find(|field| field.name == symbol.name) {
                                        // TODO: use actual visibility
                                        return format!("public {}: {}", field.name, get_type(field.ty, index.arena.clone()).to_string())
                                    }
                                }
                                Type::Object(entries) => {
                                    if let Some(ty) = entries.get(&symbol.name) {
                                        return format!("{}: {}", symbol.name, ty.to_string())
                                    }
                                }
                                _ => return format!("{}", symbol.name)
                            }

                            get_hover_text(&definition, index, source, uri, backend).await
                        }
                        _ => todo!()
                    }
                })
            }

            let hover_text = get_hover_text(symbol, &index, source, &uri, &self).await;

            let contents = HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: format!("```nlf\n{}\n```", hover_text),
            });

            return Ok(Some(Hover { contents, range: None }));
        }

        Ok(None)
    }

    async fn goto_definition(&self, params: GotoDefinitionParams) -> Result<Option<GotoDefinitionResponse>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        let contents = self.get_file_contents(&uri).await;

        let index = match analysis::get_symbol_index(uri.to_file_path().unwrap(), contents.clone()) {
            Some(index) => index,
            None => return Ok(None)
        };

        let source = line_to_source(&contents, position);

        let symbol = index.symbol_at(source);

        let Some(symbol) = symbol else {
            return Ok(None) 
        };

        if !matches!(symbol.kind, SymbolKind::Variable) {
            return Ok(None)
        }

        let source_symbol = index.find_definition(&symbol.name, source).cloned();

        let Some(mut source_symbol) = source_symbol else {
            return Ok(None)
        };

        let mut uri = uri;
        let mut contents = contents;

        if let SymbolKind::Import(source_path) = &source_symbol.kind {
            let resolved_symbol = self.resolve_import(&symbol.name, source_path, &uri).await;

            let Some((resolved_symbol, resolved_uri, resolved_contents)) = resolved_symbol else {
                return Ok(None)
            };

            uri = resolved_uri;
            contents = resolved_contents;
            source_symbol = resolved_symbol.clone();
        }

        let location = Location {
            uri,
            range: Range { start: source_to_line(&contents, source_symbol.span.start), end: source_to_line(&contents, source_symbol.span.end) }
        };

        Ok(Some(GotoDefinitionResponse::Scalar(location)))
    }

    async fn semantic_tokens_full(&self, params: SemanticTokensParams) -> Result<Option<SemanticTokensResult>> {
        let uri = params.text_document.uri;

        let contents = self.get_file_contents(&uri).await;

        let index = match analysis::get_symbol_index(uri.to_file_path().unwrap(), contents.clone()) {
            Some(index) => index,
            None => return Ok(None)
        };

        fn get_definition_type(ty: &Type) -> u32 {
            match ty {
                Type::Function(_) => 1,
                Type::Class(_) => 2,
                _ => 0
            }
        }

        fn get_symbol_type<'a>(backend: &'a Backend, uri: &'a Url, symbol: &'a Symbol, index: &'a SymbolIndex) -> std::pin::Pin<Box<dyn std::future::Future<Output = Option<u32>> + Send + 'a>> {
            Box::pin(async move {
                match &symbol.kind {
                    SymbolKind::Definition(ty) => {
                        let ty = {
                            let arena = index.arena.lock();
                            arena.get(*ty).clone()
                        };
                        Some(get_definition_type(&ty))
                    },
                    SymbolKind::Function(_) => Some(1),
                    SymbolKind::Class(_) => Some(2),
                    SymbolKind::Import(source_path) => {
                        let (resolved_symbol, uri , _) = backend.resolve_import(&symbol.name, &source_path, uri).await?;

                        get_symbol_type(backend, &uri, &resolved_symbol, index).await
                    },
                    _ => None
                }
            })
        }

        let mut data = Vec::new();
        let mut prev = (0, 0);

        let mut symbols = index.symbols.clone();

        // IMPORTANT: LSP requires tokens sorted top-to-bottom, left-to-right
        symbols.sort_by(|a, b| {
            let pa = source_to_line(&contents, a.span.start);
            let pb = source_to_line(&contents, b.span.start);

            pa.line
                .cmp(&pb.line)
                .then(pa.character.cmp(&pb.character))
        });

        for symbol in symbols {
            let (SymbolKind::Variable | SymbolKind::Definition(_)) = symbol.kind else {
                continue
            };

            let token_type = if let SymbolKind::Definition(ty) = symbol.kind {
                let ty = {
                    let arena = index.arena.lock();
                    arena.get(ty).clone()
                };
                get_definition_type(&ty)
            } else {
                let Some(source_symbol) = index.find_definition(&symbol.name, symbol.span.start).cloned() else {
                    continue;
                };

                match get_symbol_type(self, &uri, &source_symbol, &index).await {
                    Some(ty) => ty,
                    None => continue
                }
            };

            let position = source_to_line(&contents, symbol.span.start);

            data.push(SemanticToken {
                delta_line: position.line - prev.0,
                delta_start: if position.line == prev.0 { position.character - prev.1 } else { position.character },
                length: (symbol.span.end - symbol.span.start) as u32,
                token_type,
                token_modifiers_bitset: 0
            });

            prev = (position.line, position.character);
        }

        Ok(Some(SemanticTokensResult::Tokens(SemanticTokens { result_id: None, data })))
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

        cursor = line_end + LINE_ENDING_LENGTH;
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

        cursor = line_end + LINE_ENDING_LENGTH;
    }

    0
}

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::build(|client| Backend { client, documents: RwLock::new(HashMap::new()) })
        .custom_method("textDocument/content", Backend::handle_content_request)
        .finish();
    Server::new(stdin, stdout, socket).serve(service).await;
}