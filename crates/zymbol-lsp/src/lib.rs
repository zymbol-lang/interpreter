//! Zymbol-Lang Language Server library
//!
//! Exposes `run()` so that both the standalone `zymbol-lsp` binary and the
//! `zymbol lsp` subcommand can share the exact same server implementation.

use std::sync::Arc;

use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};
use tracing::{info, debug, warn};

use zymbol_analyzer::Analyzer;
use zymbol_formatter::{format_with_config, FormatterConfig};

/// The Zymbol Language Server
struct ZymbolLanguageServer {
    client: Client,
    analyzer: Analyzer,
}

impl ZymbolLanguageServer {
    fn new(client: Client) -> Self {
        Self {
            client,
            analyzer: Analyzer::new(),
        }
    }

    async fn publish_diagnostics(&self, uri: Url) {
        let uri_str = uri.as_str();
        let diagnostics = self.analyzer.get_diagnostics(uri_str);
        self.client.publish_diagnostics(uri, diagnostics, None).await;
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for ZymbolLanguageServer {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        info!("Zymbol Language Server initializing...");

        let mut roots = Vec::new();
        if let Some(folders) = params.workspace_folders {
            for folder in folders {
                if let Ok(path) = folder.uri.to_file_path() {
                    roots.push(path);
                }
            }
        } else if let Some(root_uri) = params.root_uri {
            if let Ok(path) = root_uri.to_file_path() {
                roots.push(path);
            }
        }

        if !roots.is_empty() {
            info!("Initializing workspace with {} root(s)", roots.len());
            self.analyzer.initialize_workspace(roots);
        }

        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Options(
                    TextDocumentSyncOptions {
                        open_close: Some(true),
                        change: Some(TextDocumentSyncKind::FULL),
                        save: Some(TextDocumentSyncSaveOptions::SaveOptions(SaveOptions {
                            include_text: Some(true),
                        })),
                        ..Default::default()
                    },
                )),

                semantic_tokens_provider: Some(
                    SemanticTokensServerCapabilities::SemanticTokensOptions(
                        SemanticTokensOptions {
                            legend: self.analyzer.semantic_tokens_legend(),
                            full: Some(SemanticTokensFullOptions::Bool(true)),
                            range: Some(false),
                            ..Default::default()
                        },
                    ),
                ),

                document_symbol_provider: Some(OneOf::Left(true)),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                definition_provider: Some(OneOf::Left(true)),
                references_provider: Some(OneOf::Left(true)),
                workspace_symbol_provider: Some(OneOf::Left(true)),
                document_formatting_provider: Some(OneOf::Left(true)),

                completion_provider: Some(CompletionOptions {
                    trigger_characters: Some(vec![
                        ".".to_string(),
                        ":".to_string(),
                        "$".to_string(),
                    ]),
                    ..Default::default()
                }),

                signature_help_provider: Some(SignatureHelpOptions {
                    trigger_characters: Some(vec!["(".to_string(), ",".to_string()]),
                    retrigger_characters: Some(vec![",".to_string()]),
                    ..Default::default()
                }),

                rename_provider: Some(OneOf::Right(RenameOptions {
                    prepare_provider: Some(true),
                    work_done_progress_options: Default::default(),
                })),

                code_action_provider: Some(CodeActionProviderCapability::Options(
                    CodeActionOptions {
                        code_action_kinds: Some(vec![
                            CodeActionKind::QUICKFIX,
                            CodeActionKind::REFACTOR,
                            CodeActionKind::REFACTOR_EXTRACT,
                        ]),
                        ..Default::default()
                    },
                )),

                workspace: Some(WorkspaceServerCapabilities {
                    workspace_folders: Some(WorkspaceFoldersServerCapabilities {
                        supported: Some(true),
                        change_notifications: Some(OneOf::Left(true)),
                    }),
                    file_operations: None,
                }),

                ..Default::default()
            },
            server_info: Some(ServerInfo {
                name: "zymbol-lsp".to_string(),
                version: Some(env!("CARGO_PKG_VERSION").to_string()),
            }),
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        info!("Zymbol Language Server initialized successfully");

        self.analyzer.scan_workspace();
        info!("Workspace scan completed");

        let registration = Registration {
            id: "zymbol-file-watcher".to_string(),
            method: "workspace/didChangeWatchedFiles".to_string(),
            register_options: Some(
                serde_json::to_value(DidChangeWatchedFilesRegistrationOptions {
                    watchers: vec![FileSystemWatcher {
                        glob_pattern: GlobPattern::String("**/*.zy".to_string()),
                        kind: Some(WatchKind::all()),
                    }],
                })
                .unwrap(),
            ),
        };

        if let Err(e) = self.client.register_capability(vec![registration]).await {
            warn!("Failed to register file watcher: {}", e);
        }

        self.client
            .log_message(MessageType::INFO, "Zymbol-Lang LSP ready")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        info!("Zymbol Language Server shutting down");
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri;
        let content = params.text_document.text;
        let version = params.text_document.version;
        debug!("Document opened: {}", uri);
        self.analyzer
            .open_document(Arc::from(uri.as_str()), content, version);
        self.publish_diagnostics(uri).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        let version = params.text_document.version;
        if let Some(change) = params.content_changes.into_iter().last() {
            debug!("Document changed: {}", uri);
            self.analyzer
                .update_document(uri.as_str(), change.text, version);
            self.publish_diagnostics(uri).await;
        }
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        let uri = params.text_document.uri;
        debug!("Document saved: {}", uri);
        self.publish_diagnostics(uri).await;
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri;
        debug!("Document closed: {}", uri);
        self.analyzer.close_document(uri.as_str());
        self.client.publish_diagnostics(uri, vec![], None).await;
    }

    async fn semantic_tokens_full(
        &self,
        params: SemanticTokensParams,
    ) -> Result<Option<SemanticTokensResult>> {
        let uri = params.text_document.uri;
        debug!("Semantic tokens requested: {}", uri);
        let tokens = self.analyzer.get_semantic_tokens(uri.as_str());
        Ok(tokens.map(SemanticTokensResult::Tokens))
    }

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> Result<Option<DocumentSymbolResponse>> {
        let uri = params.text_document.uri;
        debug!("Document symbols requested: {}", uri);
        let symbols = self.analyzer.get_document_symbols(uri.as_str());
        if symbols.is_empty() {
            Ok(None)
        } else {
            Ok(Some(DocumentSymbolResponse::Nested(symbols)))
        }
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;
        debug!("Hover requested at {:?}", position);
        Ok(self.analyzer.get_hover(uri.as_str(), position))
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;
        debug!("Go to definition at {:?}", position);
        let location = self.analyzer.find_definition(uri.as_str(), position);
        Ok(location.map(GotoDefinitionResponse::Scalar))
    }

    async fn references(&self, params: ReferenceParams) -> Result<Option<Vec<Location>>> {
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;
        debug!("Find references at {:?}", position);
        let locations = self.analyzer.find_references(uri.as_str(), position);
        if locations.is_empty() {
            Ok(None)
        } else {
            Ok(Some(locations))
        }
    }

    async fn symbol(
        &self,
        params: WorkspaceSymbolParams,
    ) -> Result<Option<Vec<SymbolInformation>>> {
        let query = &params.query;
        debug!("Workspace symbol search: {}", query);
        let symbols = self.analyzer.workspace_symbol_search(query);
        if symbols.is_empty() {
            Ok(None)
        } else {
            Ok(Some(symbols))
        }
    }

    async fn code_action(
        &self,
        params: CodeActionParams,
    ) -> Result<Option<CodeActionResponse>> {
        let uri = params.text_document.uri;
        let range = params.range;
        let diagnostics = &params.context.diagnostics;
        debug!("Code action requested at {:?}", range);
        let actions = self.analyzer.get_code_actions(uri.as_str(), range, diagnostics);
        if actions.is_empty() {
            Ok(None)
        } else {
            Ok(Some(actions))
        }
    }

    async fn prepare_rename(
        &self,
        params: TextDocumentPositionParams,
    ) -> Result<Option<PrepareRenameResponse>> {
        let uri = params.text_document.uri;
        let position = params.position;
        debug!("Prepare rename at {:?}", position);
        Ok(self.analyzer.prepare_rename(uri.as_str(), position))
    }

    async fn rename(&self, params: RenameParams) -> Result<Option<WorkspaceEdit>> {
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;
        let new_name = &params.new_name;
        debug!("Rename to '{}' at {:?}", new_name, position);
        Ok(self.analyzer.rename(uri.as_str(), position, new_name))
    }

    async fn signature_help(
        &self,
        params: SignatureHelpParams,
    ) -> Result<Option<SignatureHelp>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;
        debug!("Signature help requested at {:?}", position);
        Ok(self.analyzer.get_signature_help(uri.as_str(), position))
    }

    async fn completion(
        &self,
        params: CompletionParams,
    ) -> Result<Option<CompletionResponse>> {
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;
        debug!("Completion requested at {:?}", position);
        let items = self.analyzer.get_completions(uri.as_str(), position);
        if items.is_empty() {
            Ok(None)
        } else {
            Ok(Some(CompletionResponse::Array(items)))
        }
    }

    async fn formatting(
        &self,
        params: DocumentFormattingParams,
    ) -> Result<Option<Vec<TextEdit>>> {
        let uri = params.text_document.uri;
        let tab_size = params.options.tab_size as usize;
        debug!("Formatting requested: {}", uri);

        let Some(content) = self.analyzer.get_document_content(uri.as_str()) else {
            return Ok(None);
        };

        let config = FormatterConfig::new().with_indent_size(tab_size);
        let formatted = match format_with_config(&content, config) {
            Ok(f) => f,
            Err(e) => {
                debug!("Format error: {}", e);
                return Ok(None);
            }
        };

        if formatted == content {
            return Ok(Some(vec![]));
        }

        let lines: Vec<&str> = content.lines().collect();
        let last_line = lines.len().saturating_sub(1) as u32;
        let last_char = lines.last().map(|l| l.len() as u32).unwrap_or(0);

        Ok(Some(vec![TextEdit {
            range: Range {
                start: Position::new(0, 0),
                end: Position::new(last_line, last_char),
            },
            new_text: formatted,
        }]))
    }

    async fn did_change_workspace_folders(&self, params: DidChangeWorkspaceFoldersParams) {
        debug!("Workspace folders changed");
        for folder in params.event.added {
            if let Ok(path) = folder.uri.to_file_path() {
                info!("Adding workspace root: {}", path.display());
                self.analyzer.add_workspace_root(path);
            }
        }
        for folder in params.event.removed {
            if let Ok(path) = folder.uri.to_file_path() {
                info!("Removing workspace root: {}", path.display());
                self.analyzer.remove_workspace_root(&path);
            }
        }
    }

    async fn did_change_watched_files(&self, params: DidChangeWatchedFilesParams) {
        debug!("Watched files changed: {} changes", params.changes.len());
        for change in params.changes {
            let path = match change.uri.to_file_path() {
                Ok(p) => p,
                Err(_) => continue,
            };
            if path.extension().is_some_and(|ext| ext == "zy") {
                match change.typ {
                    FileChangeType::CREATED | FileChangeType::CHANGED => {
                        debug!("File changed: {}", path.display());
                        self.analyzer.on_file_changed(path);
                    }
                    FileChangeType::DELETED => {
                        debug!("File deleted: {}", path.display());
                        self.analyzer.on_file_deleted(&path);
                    }
                    _ => {}
                }
            }
        }
    }
}

/// Start the LSP server, reading from stdin and writing to stdout.
pub async fn run() {
    info!("Starting Zymbol Language Server...");
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let (service, socket) = LspService::new(ZymbolLanguageServer::new);
    Server::new(stdin, stdout, socket).serve(service).await;
}
