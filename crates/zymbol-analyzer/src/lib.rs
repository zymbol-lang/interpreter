//! Zymbol-Lang LSP Analysis Engine
//!
//! This crate provides the core analysis functionality for Zymbol-Lang's
//! Language Server Protocol (LSP) implementation.
//!
//! # Architecture
//!
//! ```text
//! zymbol-lsp (future) --> zymbol-analyzer --> compiler crates
//!                               |
//!         ┌─────────────────────┼─────────────────────┐
//!         ▼                     ▼                     ▼
//!   DocumentCache         SymbolIndex          DiagnosticPipeline
//!    (DashMap)         (3-level index)        (lexer→parser→semantic)
//! ```
//!
//! # Features
//!
//! - **Document Management**: Thread-safe document cache with lazy parsing
//! - **Diagnostics**: Error/warning pipeline with span-to-range conversion
//! - **Semantic Tokens**: Token classification for syntax highlighting
//! - **Symbol Extraction**: AST walking to find definitions
//! - **Symbol Index**: Three-level index for go-to-def and find-refs

pub mod cache;
pub mod diagnostics;
pub mod document;
pub mod module_index;
pub mod semantic_tokens;
pub mod symbol_extractor;
pub mod symbols;
pub mod workspace;

use lsp_types::{DocumentSymbol, Hover, HoverContents, Location, MarkedString, Position};
use parking_lot::RwLock;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use cache::DocumentCache;
use module_index::{ExportedKind, ExportedSymbol, ImportInfo, ModuleExports, ModuleIndex};
use symbol_extractor::SymbolExtractor;
use symbols::SymbolIndex;
use workspace::{uri_to_path, Workspace};

/// Main analyzer for Zymbol-Lang LSP
///
/// Provides the public API for LSP operations including diagnostics,
/// semantic tokens, symbol navigation, and hover information.
pub struct Analyzer {
    /// Document cache
    cache: DocumentCache,
    /// Symbol index (protected by RwLock for read-heavy workload)
    symbols: Arc<RwLock<SymbolIndex>>,
    /// Workspace for managing all .zy files
    workspace: RwLock<Workspace>,
    /// Module index for cross-file navigation
    module_index: ModuleIndex,
}

impl Analyzer {
    /// Create a new analyzer
    pub fn new() -> Self {
        Self {
            cache: DocumentCache::new(),
            symbols: Arc::new(RwLock::new(SymbolIndex::new())),
            workspace: RwLock::new(Workspace::new()),
            module_index: ModuleIndex::new(),
        }
    }

    /// Initialize the workspace with root directories
    ///
    /// This scans all directories for .zy files and indexes their exports.
    pub fn initialize_workspace(&self, roots: Vec<PathBuf>) {
        let mut workspace = self.workspace.write();
        for root in roots {
            workspace.add_root(root);
        }
    }

    /// Scan the workspace for .zy files
    ///
    /// This should be called after adding roots to discover all modules.
    pub fn scan_workspace(&self) {
        let workspace = self.workspace.read();

        for module_info in workspace.all_modules() {
            // Don't scan files that are already open in the editor
            let uri = module_info.uri.as_ref();
            if self.cache.contains(uri) {
                continue;
            }

            // Try to read and parse the file for exports
            if let Ok(content) = std::fs::read_to_string(&module_info.path) {
                self.index_background_module(&module_info.path, &content);
            }
        }
    }

    /// Index a module that is not open in the editor (background file)
    fn index_background_module(&self, path: &Path, content: &str) {
        // Quick parse to extract module declaration and exports
        let (tokens, _lexer_diags) = zymbol_lexer::Lexer::new(content, zymbol_span::FileId(0)).tokenize();
        let parser = zymbol_parser::Parser::new(tokens);
        let parse_result = parser.parse();

        if let Ok(program) = parse_result {
            // Extract module name
            let module_name = program
                .module_decl
                .as_ref()
                .map(|m| m.name.clone())
                .unwrap_or_else(|| {
                    path.file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("unknown")
                        .to_string()
                });

            // Update workspace with module name
            self.workspace.read().update_module_name(path, Some(module_name.clone()));

            // Build exports
            let uri = workspace::path_to_uri(path);
            let mut exports = ModuleExports::new(module_name, path.to_path_buf(), uri);

            // Extract exports from module declaration
            if let Some(module_decl) = &program.module_decl {
                if let Some(export_block) = &module_decl.export_block {
                    for item in &export_block.items {
                        match item {
                            zymbol_ast::ExportItem::Own { name, rename, span } => {
                                let public_name = rename.as_ref().unwrap_or(name).clone();
                                // Check if it's a function or constant by internal name
                                let kind = self.infer_export_kind(&program, name);
                                let params = self.get_function_params(&program, name);

                                let export = match kind {
                                    ExportedKind::Function => {
                                        ExportedSymbol::function(public_name, *span, params)
                                    }
                                    _ => ExportedSymbol::constant(public_name, *span),
                                };
                                exports.add_export(export);
                            }
                            zymbol_ast::ExportItem::ReExport {
                                module_alias,
                                item_name,
                                item_type,
                                rename,
                                span,
                            } => {
                                let exported_name =
                                    rename.as_ref().unwrap_or(item_name).clone();
                                let kind = match item_type {
                                    zymbol_ast::ItemType::Function => ExportedKind::Function,
                                    zymbol_ast::ItemType::Constant => ExportedKind::Constant,
                                };

                                // Try to resolve the source module
                                if let Some(resolved) =
                                    self.resolve_import_alias_internal(path, module_alias)
                                {
                                    let export = ExportedSymbol::re_export(
                                        exported_name,
                                        kind,
                                        *span,
                                        item_name.clone(),
                                        resolved,
                                    );
                                    exports.add_export(export);
                                }
                            }
                        }
                    }
                }
            }

            // Register exports
            self.module_index.index_module(path.to_path_buf(), exports);

            // Register imports
            self.module_index.clear_imports(path);
            for import in &program.imports {
                if let Some(resolved) = self.resolve_import_path(&import.path, path) {
                    self.module_index.register_import(
                        path,
                        ImportInfo {
                            alias: import.alias.clone(),
                            resolved_path: resolved,
                            span: import.span,
                        },
                    );
                }
            }
        }
    }

    /// Infer whether an exported name is a function or constant
    fn infer_export_kind(&self, program: &zymbol_ast::Program, name: &str) -> ExportedKind {
        for stmt in &program.statements {
            if let zymbol_ast::Statement::FunctionDecl(func) = stmt {
                if func.name == name {
                    return ExportedKind::Function;
                }
            }
            if let zymbol_ast::Statement::ConstDecl(decl) = stmt {
                if decl.name == name {
                    return ExportedKind::Constant;
                }
            }
            if let zymbol_ast::Statement::Assignment(assign) = stmt {
                if assign.name == name {
                    return ExportedKind::Variable;
                }
            }
        }
        ExportedKind::Constant
    }

    /// Get function parameters for an exported function
    fn get_function_params(&self, program: &zymbol_ast::Program, name: &str) -> Vec<String> {
        for stmt in &program.statements {
            if let zymbol_ast::Statement::FunctionDecl(func) = stmt {
                if func.name == name {
                    return func.parameters.iter().map(|p| p.name.clone()).collect();
                }
            }
        }
        Vec::new()
    }

    /// Resolve an import path from ModulePath
    fn resolve_import_path(
        &self,
        module_path: &zymbol_ast::ModulePath,
        from_file: &Path,
    ) -> Option<PathBuf> {
        let from_dir = from_file.parent()?;
        let mut resolved = from_dir.to_path_buf();

        // Handle parent levels
        for _ in 0..module_path.parent_levels {
            if !resolved.pop() {
                return None;
            }
        }

        // Add path components
        for component in &module_path.components {
            resolved.push(component);
        }

        // Add .zy extension
        resolved.set_extension("zy");

        Some(resolved)
    }

    /// Internal helper to resolve import alias from path
    fn resolve_import_alias_internal(&self, from_file: &Path, alias: &str) -> Option<PathBuf> {
        self.module_index.resolve_alias(from_file, alias)
    }

    /// Resolve an import alias to a module path (for external use)
    pub fn resolve_import_alias(&self, uri: &str, alias: &str) -> Option<PathBuf> {
        let path = uri_to_path(uri)?;
        self.module_index.resolve_alias(&path, alias)
    }

    /// Get module completions for an alias (after `::` or `.`)
    ///
    /// - After `::` returns functions
    /// - After `.` returns constants
    pub fn get_module_completions(
        &self,
        uri: &str,
        alias: &str,
        is_function_call: bool,
    ) -> Vec<lsp_types::CompletionItem> {
        let path = match uri_to_path(uri) {
            Some(p) => p,
            None => return Vec::new(),
        };

        let module_path = match self.module_index.resolve_alias(&path, alias) {
            Some(p) => p,
            None => return Vec::new(),
        };

        let exports = match self.module_index.get_exports(&module_path) {
            Some(e) => e,
            None => return Vec::new(),
        };

        let mut items = Vec::new();

        for export in &exports.exports {
            let matches = if is_function_call {
                export.kind.is_callable()
            } else {
                export.kind.is_property()
            };

            if matches {
                let kind = match export.kind {
                    ExportedKind::Function => lsp_types::CompletionItemKind::FUNCTION,
                    ExportedKind::Constant => lsp_types::CompletionItemKind::CONSTANT,
                    ExportedKind::Variable => lsp_types::CompletionItemKind::VARIABLE,
                };

                let (insert_text, insert_format) = if export.kind == ExportedKind::Function {
                    let params = export.parameters.as_ref().map_or(String::new(), |p| {
                        p.iter()
                            .enumerate()
                            .map(|(i, name)| format!("${{{}:{}}}", i + 1, name))
                            .collect::<Vec<_>>()
                            .join(", ")
                    });
                    (
                        Some(format!("{}({})$0", export.name, params)),
                        lsp_types::InsertTextFormat::SNIPPET,
                    )
                } else {
                    (None, lsp_types::InsertTextFormat::PLAIN_TEXT)
                };

                let detail = export.parameters.as_ref().map(|p| format!("({})", p.join(", ")));

                items.push(lsp_types::CompletionItem {
                    label: export.name.clone(),
                    kind: Some(kind),
                    detail,
                    insert_text,
                    insert_text_format: Some(insert_format),
                    ..Default::default()
                });
            }
        }

        items
    }

    /// Get all available import aliases for completions
    pub fn get_import_alias_completions(&self, uri: &str) -> Vec<lsp_types::CompletionItem> {
        let path = match uri_to_path(uri) {
            Some(p) => p,
            None => return Vec::new(),
        };

        let imports = self.module_index.get_imports(&path);

        imports
            .into_iter()
            .map(|import| lsp_types::CompletionItem {
                label: import.alias.clone(),
                kind: Some(lsp_types::CompletionItemKind::MODULE),
                detail: Some(format!("→ {}", import.resolved_path.display())),
                ..Default::default()
            })
            .collect()
    }

    /// Open a document
    ///
    /// This adds the document to the cache and indexes its symbols.
    pub fn open_document(&self, uri: Arc<str>, content: String, version: i32) {
        // Add to cache
        self.cache.open(uri.clone(), content.clone(), version);

        // Index symbols
        self.index_document(&uri);

        // Index module exports and imports
        if let Some(path) = uri_to_path(&uri) {
            self.index_background_module(&path, &content);
        }
    }

    /// Update a document's content
    ///
    /// This updates the document in the cache and re-indexes its symbols.
    pub fn update_document(&self, uri: &str, content: String, version: i32) {
        // Update in cache
        self.cache.update(uri, content.clone(), version);

        // Re-index symbols
        self.index_document(uri);

        // Re-index module exports and imports
        if let Some(path) = uri_to_path(uri) {
            self.index_background_module(&path, &content);
        }
    }

    /// Close a document
    ///
    /// This removes the document from the cache and its symbols from the index.
    /// Note: We don't remove from module_index as the file still exists on disk.
    pub fn close_document(&self, uri: &str) {
        self.cache.close(uri);
        self.symbols.write().remove_document(uri);
    }

    /// Add a workspace root and scan for modules
    pub fn add_workspace_root(&self, path: PathBuf) {
        self.workspace.write().add_root(path);
        self.scan_workspace();
    }

    /// Remove a workspace root
    pub fn remove_workspace_root(&self, path: &std::path::Path) {
        self.workspace.write().remove_root(path);
        // Clean up module index for removed files
        for module_path in self.module_index.all_modules() {
            if module_path.starts_with(path) {
                self.module_index.remove_module(&module_path);
            }
        }
    }

    /// Handle file created/changed notification from workspace watcher
    pub fn on_file_changed(&self, path: PathBuf) {
        // Check if it's a .zy file
        if path.extension().is_some_and(|ext| ext == "zy") {
            // Update workspace
            self.workspace.read().add_module(path.clone());

            // Re-index if not open in editor
            let uri = workspace::path_to_uri(&path);
            if !self.cache.contains(uri.as_ref()) {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    self.index_background_module(&path, &content);
                }
            }
        }
    }

    /// Handle file deleted notification from workspace watcher
    pub fn on_file_deleted(&self, path: &std::path::Path) {
        self.workspace.read().remove_module(path);
        self.module_index.remove_module(path);
    }

    /// Index symbols for a document
    fn index_document(&self, uri: &str) {
        if let Some(doc) = self.cache.get(uri) {
            if let Some(program) = doc.ast() {
                let mut extractor = SymbolExtractor::new();
                let symbols = extractor.extract(program);
                self.symbols.write().index_document(Arc::from(uri), symbols);
            }
        }
    }

    /// Get diagnostics for a document
    pub fn get_diagnostics(&self, uri: &str) -> Vec<lsp_types::Diagnostic> {
        let mut all_diagnostics = Vec::new();

        if let Some(doc) = self.cache.get(uri) {
            // Collect standard diagnostics
            all_diagnostics = diagnostics::DiagnosticPipeline::collect(&doc);

            // Collect module-related diagnostics
            if let Some(path) = uri_to_path(uri) {
                all_diagnostics.extend(self.collect_module_diagnostics(&path, &doc));
            }
        }

        all_diagnostics
    }

    /// Collect module-related diagnostics for a document
    fn collect_module_diagnostics(
        &self,
        path: &Path,
        doc: &document::Document,
    ) -> Vec<lsp_types::Diagnostic> {
        let mut diagnostics = Vec::new();

        // Parse the document to get AST
        let parse_result = doc.parse();
        let program = match &parse_result.program {
            Some(p) => p,
            None => return diagnostics,
        };

        // Check for circular dependencies
        if let Some(cycle) = self.module_index.check_circular_deps(path) {
            let cycle_str: Vec<String> = cycle.iter().map(|p| p.display().to_string()).collect();
            diagnostics.push(lsp_types::Diagnostic {
                range: lsp_types::Range {
                    start: lsp_types::Position { line: 0, character: 0 },
                    end: lsp_types::Position { line: 0, character: 0 },
                },
                severity: Some(lsp_types::DiagnosticSeverity::ERROR),
                code: Some(lsp_types::NumberOrString::String("circular-dependency".to_string())),
                code_description: None,
                source: Some("zymbol".to_string()),
                message: format!("Circular dependency detected: {}", cycle_str.join(" → ")),
                related_information: None,
                tags: None,
                data: None,
            });
        }

        // Validate imports
        for import in &program.imports {
            let resolved = self.resolve_import_path(&import.path, path);

            match resolved {
                Some(resolved_path) => {
                    // Check if the module exists
                    if !resolved_path.exists() {
                        diagnostics.push(lsp_types::Diagnostic {
                            range: diagnostics::span_to_range(&import.span),
                            severity: Some(lsp_types::DiagnosticSeverity::ERROR),
                            code: Some(lsp_types::NumberOrString::String("module-not-found".to_string())),
                            code_description: None,
                            source: Some("zymbol".to_string()),
                            message: format!("Module not found: {}", resolved_path.display()),
                            related_information: None,
                            tags: None,
                            data: None,
                        });
                    }
                }
                None => {
                    diagnostics.push(lsp_types::Diagnostic {
                        range: diagnostics::span_to_range(&import.span),
                        severity: Some(lsp_types::DiagnosticSeverity::ERROR),
                        code: Some(lsp_types::NumberOrString::String("invalid-import-path".to_string())),
                        code_description: None,
                        source: Some("zymbol".to_string()),
                        message: "Cannot resolve import path".to_string(),
                        related_information: None,
                        tags: None,
                        data: None,
                    });
                }
            }
        }

        // Validate module access in the code
        let tokens = doc.token_list();
        self.validate_module_access(path, tokens, &mut diagnostics);

        diagnostics
    }

    /// Validate module access patterns in the code
    fn validate_module_access(
        &self,
        from_file: &Path,
        tokens: &[zymbol_lexer::Token],
        diagnostics: &mut Vec<lsp_types::Diagnostic>,
    ) {
        let mut i = 0;
        while i + 2 < tokens.len() {
            // Look for patterns: alias :: symbol or alias . symbol
            if let zymbol_lexer::TokenKind::Ident(alias) = &tokens[i].kind {
                let is_function_call =
                    matches!(&tokens[i + 1].kind, zymbol_lexer::TokenKind::ScopeResolution);
                let is_property_access = matches!(&tokens[i + 1].kind, zymbol_lexer::TokenKind::Dot);

                if is_function_call || is_property_access {
                    if let zymbol_lexer::TokenKind::Ident(symbol) = &tokens[i + 2].kind {
                        // Check if the alias resolves to a module
                        if let Some(module_path) = self.module_index.resolve_alias(from_file, alias)
                        {
                            // Check if the module has this export
                            if let Some(exports) = self.module_index.get_exports(&module_path) {
                                match exports.get_export(symbol) {
                                    Some(export) => {
                                        // Check if the access type is correct
                                        if is_function_call && !export.kind.is_callable() {
                                            diagnostics.push(lsp_types::Diagnostic {
                                                range: diagnostics::span_to_range(&tokens[i + 2].span),
                                                severity: Some(lsp_types::DiagnosticSeverity::ERROR),
                                                code: Some(lsp_types::NumberOrString::String(
                                                    "incorrect-access".to_string(),
                                                )),
                                                code_description: None,
                                                source: Some("zymbol".to_string()),
                                                message: format!(
                                                    "'{}' is not a function. Use '.' to access constants: {}.{}",
                                                    symbol, alias, symbol
                                                ),
                                                related_information: None,
                                                tags: None,
                                                data: None,
                                            });
                                        } else if is_property_access && export.kind.is_callable() {
                                            diagnostics.push(lsp_types::Diagnostic {
                                                range: diagnostics::span_to_range(&tokens[i + 2].span),
                                                severity: Some(lsp_types::DiagnosticSeverity::WARNING),
                                                code: Some(lsp_types::NumberOrString::String(
                                                    "incorrect-access".to_string(),
                                                )),
                                                code_description: None,
                                                source: Some("zymbol".to_string()),
                                                message: format!(
                                                    "'{}' is a function. Use '::' to call: {}::{}()",
                                                    symbol, alias, symbol
                                                ),
                                                related_information: None,
                                                tags: None,
                                                data: None,
                                            });
                                        }
                                    }
                                    None => {
                                        diagnostics.push(lsp_types::Diagnostic {
                                            range: diagnostics::span_to_range(&tokens[i + 2].span),
                                            severity: Some(lsp_types::DiagnosticSeverity::ERROR),
                                            code: Some(lsp_types::NumberOrString::String(
                                                "export-not-found".to_string(),
                                            )),
                                            code_description: None,
                                            source: Some("zymbol".to_string()),
                                            message: format!(
                                                "'{}' is not exported by module '{}'",
                                                symbol, exports.name
                                            ),
                                            related_information: None,
                                            tags: None,
                                            data: None,
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            }
            i += 1;
        }
    }

    /// Get semantic tokens for a document
    pub fn get_semantic_tokens(&self, uri: &str) -> Option<lsp_types::SemanticTokens> {
        let doc = self.cache.get(uri)?;
        let tokens = doc.token_list();
        Some(semantic_tokens::generate_semantic_tokens(tokens))
    }

    /// Get the semantic tokens legend
    pub fn semantic_tokens_legend(&self) -> lsp_types::SemanticTokensLegend {
        semantic_tokens::semantic_tokens_legend()
    }

    /// Get document symbols (for outline view)
    pub fn get_document_symbols(&self, uri: &str) -> Vec<DocumentSymbol> {
        if let Some(doc) = self.cache.get(uri) {
            if let Some(program) = doc.ast() {
                let mut extractor = SymbolExtractor::new();
                let symbols = extractor.extract(program);
                return symbols.iter().map(|s| s.to_document_symbol()).collect();
            }
        }
        Vec::new()
    }

    /// Find definition of symbol at position
    pub fn find_definition(&self, uri: &str, pos: Position) -> Option<Location> {
        // Get context at position (could be module access like `m::add` or `m.PI`)
        let doc = self.cache.get(uri)?;
        let tokens = doc.token_list();

        // Convert LSP position to 1-indexed
        let line = pos.line + 1;
        let column = pos.character + 1;

        // Find token at position and check context
        let mut prev_tokens: Vec<&zymbol_lexer::Token> = Vec::new();
        let mut found_token: Option<&zymbol_lexer::Token> = None;

        for token in tokens {
            if token.span.start.line == line
                && token.span.start.column <= column
                && token.span.end.column >= column
            {
                found_token = Some(token);
                break;
            }
            prev_tokens.push(token);
        }

        let token = found_token?;

        // Check if this is a module access pattern: `alias::symbol` or `alias.symbol`
        if let zymbol_lexer::TokenKind::Ident(symbol_name) = &token.kind {
            // Look back for `::` or `.` and then an identifier (the alias)
            if prev_tokens.len() >= 2 {
                let last = prev_tokens[prev_tokens.len() - 1];
                let second_last = prev_tokens[prev_tokens.len() - 2];

                let is_module_access = matches!(
                    &last.kind,
                    zymbol_lexer::TokenKind::ScopeResolution | zymbol_lexer::TokenKind::Dot
                );

                if is_module_access {
                    if let zymbol_lexer::TokenKind::Ident(alias) = &second_last.kind {
                        // This is a module access - try cross-file navigation
                        if let Some(path) = uri_to_path(uri) {
                            if let Some((exports, export)) =
                                self.module_index.resolve_symbol(&path, alias, symbol_name)
                            {
                                // Found in another module
                                let target_uri = if exports.uri.starts_with("file://") {
                                    exports.uri.to_string()
                                } else {
                                    format!("file://{}", exports.file_path.display())
                                };

                                return Some(Location {
                                    uri: lsp_types::Url::parse(&target_uri).ok()?,
                                    range: diagnostics::span_to_range(&export.span),
                                });
                            }
                        }
                    }
                }
            }

            // Not a module access - use local symbol index
            let index = self.symbols.read();
            let defs = index.find_definitions(symbol_name);
            return defs.first().map(|r| r.to_location());
        }

        None
    }

    /// Find all references to symbol at position
    pub fn find_references(&self, uri: &str, pos: Position) -> Vec<Location> {
        // Get the symbol name at position
        let name = match self.get_symbol_at_position(uri, pos) {
            Some(n) => n,
            None => return Vec::new(),
        };

        let index = self.symbols.read();

        // Get definitions
        let mut locations: Vec<Location> = index
            .find_definitions(&name)
            .iter()
            .map(|r| r.to_location())
            .collect();

        // Get references
        locations.extend(index.find_references(&name).iter().map(|r| r.to_location()));

        // If this is an exported symbol, find references in other files
        if let Some(current_path) = uri_to_path(uri) {
            // Check if this symbol is exported
            if let Some(exports) = self.module_index.get_exports(&current_path) {
                if exports.get_export(&name).is_some() {
                    // Find all files that import this module
                    let importers = self.module_index.get_importers(&current_path);

                    for importer_path in importers {
                        // Find references in the importing file
                        let importer_uri = workspace::path_to_uri(&importer_path);
                        if let Some(doc) = self.cache.get(importer_uri.as_ref()) {
                            let tokens = doc.token_list();

                            // Find the alias used to import this module
                            let imports = self.module_index.get_imports(&importer_path);
                            let aliases: Vec<&str> = imports
                                .iter()
                                .filter(|i| i.resolved_path == current_path)
                                .map(|i| i.alias.as_str())
                                .collect();

                            // Search for `alias::name` or `alias.name` patterns
                            let mut i = 0;
                            while i + 2 < tokens.len() {
                                if let zymbol_lexer::TokenKind::Ident(alias) = &tokens[i].kind {
                                    if aliases.contains(&alias.as_str()) {
                                        let is_access = matches!(
                                            &tokens[i + 1].kind,
                                            zymbol_lexer::TokenKind::ScopeResolution
                                                | zymbol_lexer::TokenKind::Dot
                                        );
                                        if is_access {
                                            if let zymbol_lexer::TokenKind::Ident(sym) =
                                                &tokens[i + 2].kind
                                            {
                                                if sym == &name {
                                                    let url = lsp_types::Url::parse(
                                                        importer_uri.as_ref(),
                                                    );
                                                    if let Ok(url) = url {
                                                        locations.push(Location {
                                                            uri: url,
                                                            range: diagnostics::span_to_range(
                                                                &tokens[i + 2].span,
                                                            ),
                                                        });
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                                i += 1;
                            }
                        }
                    }
                }
            }
        }

        locations
    }

    /// Get hover information for symbol at position
    pub fn get_hover(&self, uri: &str, pos: Position) -> Option<Hover> {
        let name = self.get_symbol_at_position(uri, pos)?;

        let index = self.symbols.read();
        let defs = index.find_definitions(&name);
        let def = defs.first()?;

        let content = match def.kind {
            symbol_extractor::SymbolKind::Function => {
                let detail = def.detail.as_deref().unwrap_or("()");
                format!("```zymbol\n{}{}\n```", name, detail)
            }
            symbol_extractor::SymbolKind::Constant => {
                format!("```zymbol\n{} := ...\n```\n(constant)", name)
            }
            symbol_extractor::SymbolKind::Variable => {
                format!("```zymbol\n{} = ...\n```\n(variable)", name)
            }
            symbol_extractor::SymbolKind::Parameter => {
                format!("```zymbol\n{}\n```\n(parameter)", name)
            }
            symbol_extractor::SymbolKind::Module => {
                format!("```zymbol\n# {}\n```\n(module)", name)
            }
            symbol_extractor::SymbolKind::Import => {
                format!("```zymbol\n<# ... <= {}\n```\n(import)", name)
            }
            symbol_extractor::SymbolKind::Iterator => {
                format!("```zymbol\n@ {}:...\n```\n(loop iterator)", name)
            }
        };

        Some(Hover {
            contents: HoverContents::Scalar(MarkedString::String(content)),
            range: Some(diagnostics::span_to_range(&def.span)),
        })
    }

    /// Get symbol name at a position in a document
    fn get_symbol_at_position(&self, uri: &str, pos: Position) -> Option<String> {
        let doc = self.cache.get(uri)?;
        let tokens = doc.token_list();

        // Convert LSP position to 1-indexed
        let line = pos.line + 1;
        let column = pos.character + 1;

        // Find token at position
        for token in tokens {
            if token.span.start.line == line
                && token.span.start.column <= column
                && token.span.end.column >= column
            {
                if let zymbol_lexer::TokenKind::Ident(name) = &token.kind {
                    return Some(name.clone());
                }
            }
        }

        None
    }

    /// Search for symbols matching a pattern
    pub fn workspace_symbol_search(&self, query: &str) -> Vec<lsp_types::SymbolInformation> {
        let index = self.symbols.read();
        let results = index.search(query);

        results
            .iter()
            .map(|sym_ref| {
                #[allow(deprecated)]
                lsp_types::SymbolInformation {
                    name: sym_ref.name.clone(),
                    kind: sym_ref.kind.to_lsp(),
                    tags: None,
                    deprecated: None,
                    location: sym_ref.to_location(),
                    container_name: None,
                }
            })
            .collect()
    }

    /// Check if a document is in the cache
    pub fn has_document(&self, uri: &str) -> bool {
        self.cache.contains(uri)
    }

    /// Get document count
    pub fn document_count(&self) -> usize {
        self.cache.len()
    }

    /// Get symbol index statistics
    pub fn symbol_stats(&self) -> symbols::IndexStats {
        self.symbols.read().stats()
    }

    /// Get document content for formatting
    pub fn get_document_content(&self, uri: &str) -> Option<String> {
        self.cache.get(uri).map(|doc| doc.content.to_string())
    }

    /// Get completion items at a position
    pub fn get_completions(&self, uri: &str, pos: Position) -> Vec<lsp_types::CompletionItem> {
        let mut items = Vec::new();

        // Check if we're in a module access context (after `::` or `.`)
        if let Some(doc) = self.cache.get(uri) {
            let content = &doc.content;
            if let Some(offset) = position_to_offset(content, pos) {
                // Look back to find context
                if let Some((context, is_function_call)) =
                    self.find_module_access_context(content, offset)
                {
                    // Get module completions
                    return self.get_module_completions(uri, &context, is_function_call);
                }
            }

            // Not in module access context - get regular completions
            if let Some(program) = doc.ast() {
                let mut extractor = SymbolExtractor::new();
                let symbols = extractor.extract(program);

                for symbol in &symbols {
                    items.push(symbol_to_completion(symbol));
                    // Also add children (parameters, local vars)
                    for child in &symbol.children {
                        items.push(symbol_to_completion(child));
                    }
                }
            }
        }

        // Get symbols from all open documents (workspace)
        let index = self.symbols.read();
        for sym_ref in index.all_symbols() {
            // Avoid duplicates from current file
            if sym_ref.uri.as_ref() != uri {
                items.push(lsp_types::CompletionItem {
                    label: sym_ref.name.clone(),
                    kind: Some(symbol_kind_to_completion(sym_ref.kind)),
                    detail: sym_ref.detail.clone(),
                    ..Default::default()
                });
            }
        }

        // Add import alias completions
        items.extend(self.get_import_alias_completions(uri));

        // Add Zymbol operators/keywords as snippets
        items.extend(builtin_completions());

        items
    }

    /// Find if cursor is in a module access context (after `alias::` or `alias.`)
    /// Returns (alias, is_function_call) if found
    fn find_module_access_context(&self, content: &str, offset: usize) -> Option<(String, bool)> {
        if offset < 2 {
            return None;
        }

        let before = &content[..offset];
        let chars: Vec<char> = before.chars().collect();

        // Check the last two characters
        let len = chars.len();
        if len < 2 {
            return None;
        }

        let last_two = format!("{}{}", chars[len - 2], chars[len - 1]);
        let is_function_call = last_two == "::";
        let is_property_access = chars[len - 1] == '.' && (len < 2 || chars[len - 2] != '.');

        if !is_function_call && !is_property_access {
            return None;
        }

        // Find the identifier before the accessor
        let skip = if is_function_call { 2 } else { 1 };
        let search_end = len - skip;

        // Scan backwards for the identifier
        let mut end = search_end;
        while end > 0 && chars[end - 1].is_whitespace() {
            end -= 1;
        }

        let mut start = end;
        while start > 0 && (chars[start - 1].is_alphanumeric() || chars[start - 1] == '_') {
            start -= 1;
        }

        if start == end {
            return None;
        }

        let alias: String = chars[start..end].iter().collect();
        Some((alias, is_function_call))
    }

    /// Get signature help at a position (for function calls)
    pub fn get_signature_help(&self, uri: &str, pos: Position) -> Option<lsp_types::SignatureHelp> {
        let doc = self.cache.get(uri)?;
        let content = &doc.content;

        // Convert LSP position to offset
        let offset = position_to_offset(content, pos)?;

        // Find the function call context by scanning backwards for '('
        let (func_name, active_param) = find_function_call_context(content, offset)?;

        // Look up the function in the symbol index
        let index = self.symbols.read();
        let defs = index.find_definitions(&func_name);

        // Find a function definition
        let func_def = defs.iter().find(|d| d.kind == symbol_extractor::SymbolKind::Function)?;

        // Parse the parameter list from detail
        let params = parse_parameters(func_def.detail.as_deref().unwrap_or("()"));

        // Build signature information
        let param_infos: Vec<lsp_types::ParameterInformation> = params
            .iter()
            .map(|p| lsp_types::ParameterInformation {
                label: lsp_types::ParameterLabel::Simple(p.clone()),
                documentation: None,
            })
            .collect();

        let signature = lsp_types::SignatureInformation {
            label: format!("{}({})", func_name, params.join(", ")),
            documentation: None,
            parameters: Some(param_infos),
            active_parameter: Some(active_param as u32),
        };

        Some(lsp_types::SignatureHelp {
            signatures: vec![signature],
            active_signature: Some(0),
            active_parameter: Some(active_param as u32),
        })
    }

    /// Prepare rename - check if symbol at position can be renamed
    pub fn prepare_rename(&self, uri: &str, pos: Position) -> Option<lsp_types::PrepareRenameResponse> {
        let name = self.get_symbol_at_position(uri, pos)?;

        // Check if this symbol exists in the index
        let index = self.symbols.read();
        let defs = index.find_definitions(&name);

        if defs.is_empty() {
            return None;
        }

        // Find the token at the position to get its range
        let doc = self.cache.get(uri)?;
        let tokens = doc.token_list();

        let line = pos.line + 1;
        let column = pos.character + 1;

        for token in tokens {
            if token.span.start.line == line
                && token.span.start.column <= column
                && token.span.end.column >= column
            {
                if let zymbol_lexer::TokenKind::Ident(_) = &token.kind {
                    let range = diagnostics::span_to_range(&token.span);
                    return Some(lsp_types::PrepareRenameResponse::Range(range));
                }
            }
        }

        None
    }

    /// Rename a symbol across all documents
    pub fn rename(&self, uri: &str, pos: Position, new_name: &str) -> Option<lsp_types::WorkspaceEdit> {
        let old_name = self.get_symbol_at_position(uri, pos)?;

        // Validate new name (must be valid identifier)
        if new_name.is_empty() || !is_valid_identifier(new_name) {
            return None;
        }

        let mut changes: std::collections::HashMap<lsp_types::Url, Vec<lsp_types::TextEdit>> =
            std::collections::HashMap::new();

        // Find all occurrences in all open documents
        for doc_uri in self.cache.uris() {
            if let Some(doc) = self.cache.get(&doc_uri) {
                let tokens = doc.token_list();
                let mut edits = Vec::new();

                for token in tokens {
                    if let zymbol_lexer::TokenKind::Ident(name) = &token.kind {
                        if name == &old_name {
                            let range = diagnostics::span_to_range(&token.span);
                            edits.push(lsp_types::TextEdit {
                                range,
                                new_text: new_name.to_string(),
                            });
                        }
                    }
                }

                if !edits.is_empty() {
                    let url = if doc_uri.starts_with("file://") {
                        lsp_types::Url::parse(&doc_uri).ok()?
                    } else {
                        lsp_types::Url::parse(&format!("file://{}", doc_uri)).ok()?
                    };
                    changes.insert(url, edits);
                }
            }
        }

        if changes.is_empty() {
            return None;
        }

        Some(lsp_types::WorkspaceEdit {
            changes: Some(changes),
            document_changes: None,
            change_annotations: None,
        })
    }

    /// Get code actions for a range (quick fixes)
    pub fn get_code_actions(
        &self,
        uri: &str,
        range: lsp_types::Range,
        diagnostics: &[lsp_types::Diagnostic],
    ) -> Vec<lsp_types::CodeActionOrCommand> {
        let mut actions = Vec::new();

        let doc = match self.cache.get(uri) {
            Some(d) => d,
            None => return actions,
        };

        let url = match lsp_types::Url::parse(uri) {
            Ok(u) => u,
            Err(_) => match lsp_types::Url::parse(&format!("file://{}", uri)) {
                Ok(u) => u,
                Err(_) => return actions,
            },
        };

        // Check each diagnostic for applicable code actions
        for diag in diagnostics {
            // Action for unused variables
            if diag.message.contains("unused variable") {
                if let Some(var_name) = extract_variable_name(&diag.message) {
                    // Action 1: Prefix with underscore
                    if !var_name.starts_with('_') {
                        let new_name = format!("_{}", var_name);
                        let edit = create_rename_edit(&url, &diag.range, &new_name);

                        actions.push(lsp_types::CodeActionOrCommand::CodeAction(
                            lsp_types::CodeAction {
                                title: format!("Prefix '{}' with underscore", var_name),
                                kind: Some(lsp_types::CodeActionKind::QUICKFIX),
                                diagnostics: Some(vec![diag.clone()]),
                                edit: Some(edit),
                                is_preferred: Some(true),
                                ..Default::default()
                            },
                        ));
                    }

                    // Action 2: Remove the line (if we can find the full statement)
                    if let Some(line_range) = get_full_line_range(&doc.content, diag.range.start.line) {
                        let edit = create_delete_edit(&url, &line_range);

                        actions.push(lsp_types::CodeActionOrCommand::CodeAction(
                            lsp_types::CodeAction {
                                title: format!("Remove unused variable '{}'", var_name),
                                kind: Some(lsp_types::CodeActionKind::QUICKFIX),
                                diagnostics: Some(vec![diag.clone()]),
                                edit: Some(edit),
                                is_preferred: Some(false),
                                ..Default::default()
                            },
                        ));
                    }
                }
            }

            // Action for write-only variables
            if diag.message.contains("assigned but never read") {
                if let Some(var_name) = extract_variable_name(&diag.message) {
                    // Suggest prefixing with underscore
                    if !var_name.starts_with('_') {
                        let new_name = format!("_{}", var_name);
                        let edit = create_rename_edit(&url, &diag.range, &new_name);

                        actions.push(lsp_types::CodeActionOrCommand::CodeAction(
                            lsp_types::CodeAction {
                                title: format!("Prefix '{}' with underscore (mark as intentional)", var_name),
                                kind: Some(lsp_types::CodeActionKind::QUICKFIX),
                                diagnostics: Some(vec![diag.clone()]),
                                edit: Some(edit),
                                is_preferred: Some(true),
                                ..Default::default()
                            },
                        ));
                    }
                }
            }
        }

        // Add refactoring actions based on cursor position
        if let Some(name) = self.get_symbol_at_position(uri, range.start) {
            // Extract to variable (for expressions)
            // This is more complex, skip for now

            // Extract to function
            actions.push(lsp_types::CodeActionOrCommand::CodeAction(
                lsp_types::CodeAction {
                    title: format!("Extract '{}' to function", name),
                    kind: Some(lsp_types::CodeActionKind::REFACTOR_EXTRACT),
                    disabled: Some(lsp_types::CodeActionDisabled {
                        reason: "Not yet implemented".to_string(),
                    }),
                    ..Default::default()
                },
            ));
        }

        actions
    }
}

/// Extract variable name from diagnostic message like "unused variable 'x'"
fn extract_variable_name(message: &str) -> Option<String> {
    let start = message.find('\'')?;
    let end = message[start + 1..].find('\'')?;
    Some(message[start + 1..start + 1 + end].to_string())
}

/// Create a workspace edit that renames text at a range
fn create_rename_edit(url: &lsp_types::Url, range: &lsp_types::Range, new_text: &str) -> lsp_types::WorkspaceEdit {
    let mut changes = std::collections::HashMap::new();
    changes.insert(
        url.clone(),
        vec![lsp_types::TextEdit {
            range: *range,
            new_text: new_text.to_string(),
        }],
    );
    lsp_types::WorkspaceEdit {
        changes: Some(changes),
        document_changes: None,
        change_annotations: None,
    }
}

/// Create a workspace edit that deletes a range
fn create_delete_edit(url: &lsp_types::Url, range: &lsp_types::Range) -> lsp_types::WorkspaceEdit {
    let mut changes = std::collections::HashMap::new();
    changes.insert(
        url.clone(),
        vec![lsp_types::TextEdit {
            range: *range,
            new_text: String::new(),
        }],
    );
    lsp_types::WorkspaceEdit {
        changes: Some(changes),
        document_changes: None,
        change_annotations: None,
    }
}

/// Get the range for a full line (including newline)
fn get_full_line_range(content: &str, line_num: u32) -> Option<lsp_types::Range> {
    let lines: Vec<&str> = content.lines().collect();
    let line_idx = line_num as usize;

    if line_idx >= lines.len() {
        return None;
    }

    Some(lsp_types::Range {
        start: lsp_types::Position { line: line_num, character: 0 },
        end: lsp_types::Position { line: line_num + 1, character: 0 },
    })
}

/// Check if a string is a valid Zymbol identifier
fn is_valid_identifier(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_alphanumeric() || c == '_')
}

/// Convert LSP Position to byte offset in content
fn position_to_offset(content: &str, pos: Position) -> Option<usize> {
    let mut offset = 0;
    for (line_num, line) in content.lines().enumerate() {
        if line_num == pos.line as usize {
            // Found the line, add column offset
            let col = pos.character as usize;
            // Handle UTF-8 properly
            let line_offset: usize = line.chars().take(col).map(|c| c.len_utf8()).sum();
            return Some(offset + line_offset);
        }
        offset += line.len() + 1; // +1 for newline
    }
    None
}

/// Find the function call context at a position
/// Returns (function_name, active_parameter_index)
fn find_function_call_context(content: &str, offset: usize) -> Option<(String, usize)> {
    let before = &content[..offset];

    // Count commas and find the opening paren
    let mut paren_depth = 0;
    let mut comma_count = 0;
    let mut paren_pos = None;

    for (i, ch) in before.chars().rev().enumerate() {
        let pos = before.len() - i - 1;
        match ch {
            ')' => paren_depth += 1,
            '(' => {
                if paren_depth == 0 {
                    paren_pos = Some(pos);
                    break;
                }
                paren_depth -= 1;
            }
            ',' if paren_depth == 0 => comma_count += 1,
            _ => {}
        }
    }

    let paren_pos = paren_pos?;

    // Find the function name before the paren
    let before_paren = &before[..paren_pos];
    let func_name: String = before_paren
        .chars()
        .rev()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect::<String>()
        .chars()
        .rev()
        .collect();

    if func_name.is_empty() {
        return None;
    }

    Some((func_name, comma_count))
}

/// Parse parameters from a detail string like "(a, b, ~c)"
fn parse_parameters(detail: &str) -> Vec<String> {
    let trimmed = detail.trim_matches(|c| c == '(' || c == ')');
    if trimmed.is_empty() {
        return Vec::new();
    }
    trimmed.split(',').map(|s| s.trim().to_string()).collect()
}

/// Convert a Symbol to a CompletionItem
fn symbol_to_completion(symbol: &symbol_extractor::Symbol) -> lsp_types::CompletionItem {
    use lsp_types::{CompletionItem, CompletionItemKind, InsertTextFormat};

    let (kind, insert_text, insert_format) = match symbol.kind {
        symbol_extractor::SymbolKind::Function => {
            // For functions, add snippet with parameters
            let snippet = if let Some(detail) = &symbol.detail {
                // detail is like "(a, b)" - convert to snippet
                let params = detail.trim_matches(|c| c == '(' || c == ')');
                if params.is_empty() {
                    format!("{}()$0", symbol.name)
                } else {
                    let param_snippets: Vec<String> = params
                        .split(", ")
                        .enumerate()
                        .map(|(i, p)| format!("${{{}:{}}}", i + 1, p.trim_start_matches(['~', '<'])))
                        .collect();
                    format!("{}({})$0", symbol.name, param_snippets.join(", "))
                }
            } else {
                format!("{}($0)", symbol.name)
            };
            (CompletionItemKind::FUNCTION, Some(snippet), InsertTextFormat::SNIPPET)
        }
        symbol_extractor::SymbolKind::Constant => {
            (CompletionItemKind::CONSTANT, None, InsertTextFormat::PLAIN_TEXT)
        }
        symbol_extractor::SymbolKind::Variable => {
            (CompletionItemKind::VARIABLE, None, InsertTextFormat::PLAIN_TEXT)
        }
        symbol_extractor::SymbolKind::Parameter => {
            (CompletionItemKind::VARIABLE, None, InsertTextFormat::PLAIN_TEXT)
        }
        symbol_extractor::SymbolKind::Module => {
            (CompletionItemKind::MODULE, None, InsertTextFormat::PLAIN_TEXT)
        }
        symbol_extractor::SymbolKind::Import => {
            (CompletionItemKind::MODULE, None, InsertTextFormat::PLAIN_TEXT)
        }
        symbol_extractor::SymbolKind::Iterator => {
            (CompletionItemKind::VARIABLE, None, InsertTextFormat::PLAIN_TEXT)
        }
    };

    CompletionItem {
        label: symbol.name.clone(),
        kind: Some(kind),
        detail: symbol.detail.clone(),
        insert_text,
        insert_text_format: Some(insert_format),
        ..Default::default()
    }
}

/// Convert SymbolKind to CompletionItemKind
fn symbol_kind_to_completion(kind: symbol_extractor::SymbolKind) -> lsp_types::CompletionItemKind {
    use lsp_types::CompletionItemKind;
    match kind {
        symbol_extractor::SymbolKind::Function => CompletionItemKind::FUNCTION,
        symbol_extractor::SymbolKind::Constant => CompletionItemKind::CONSTANT,
        symbol_extractor::SymbolKind::Variable => CompletionItemKind::VARIABLE,
        symbol_extractor::SymbolKind::Parameter => CompletionItemKind::VARIABLE,
        symbol_extractor::SymbolKind::Module => CompletionItemKind::MODULE,
        symbol_extractor::SymbolKind::Import => CompletionItemKind::MODULE,
        symbol_extractor::SymbolKind::Iterator => CompletionItemKind::VARIABLE,
    }
}

/// Built-in Zymbol operators and constructs
fn builtin_completions() -> Vec<lsp_types::CompletionItem> {
    use lsp_types::{CompletionItem, CompletionItemKind, InsertTextFormat};

    vec![
        // Control flow
        CompletionItem {
            label: "if".to_string(),
            kind: Some(CompletionItemKind::KEYWORD),
            detail: Some("? condition { }".to_string()),
            insert_text: Some("? ${1:condition} {\n\t$0\n}".to_string()),
            insert_text_format: Some(InsertTextFormat::SNIPPET),
            ..Default::default()
        },
        CompletionItem {
            label: "else".to_string(),
            kind: Some(CompletionItemKind::KEYWORD),
            detail: Some("_ { }".to_string()),
            insert_text: Some("_ {\n\t$0\n}".to_string()),
            insert_text_format: Some(InsertTextFormat::SNIPPET),
            ..Default::default()
        },
        CompletionItem {
            label: "else-if".to_string(),
            kind: Some(CompletionItemKind::KEYWORD),
            detail: Some("_? condition { }".to_string()),
            insert_text: Some("_? ${1:condition} {\n\t$0\n}".to_string()),
            insert_text_format: Some(InsertTextFormat::SNIPPET),
            ..Default::default()
        },
        CompletionItem {
            label: "match".to_string(),
            kind: Some(CompletionItemKind::KEYWORD),
            detail: Some("?? expr { pattern : value }".to_string()),
            insert_text: Some("?? ${1:expr} {\n\t${2:pattern} : ${3:value}\n\t_ : ${4:default}\n}".to_string()),
            insert_text_format: Some(InsertTextFormat::SNIPPET),
            ..Default::default()
        },
        // Loops
        CompletionItem {
            label: "loop".to_string(),
            kind: Some(CompletionItemKind::KEYWORD),
            detail: Some("@ { } (infinite loop)".to_string()),
            insert_text: Some("@ {\n\t$0\n}".to_string()),
            insert_text_format: Some(InsertTextFormat::SNIPPET),
            ..Default::default()
        },
        CompletionItem {
            label: "while".to_string(),
            kind: Some(CompletionItemKind::KEYWORD),
            detail: Some("@ condition { }".to_string()),
            insert_text: Some("@ ${1:condition} {\n\t$0\n}".to_string()),
            insert_text_format: Some(InsertTextFormat::SNIPPET),
            ..Default::default()
        },
        CompletionItem {
            label: "for".to_string(),
            kind: Some(CompletionItemKind::KEYWORD),
            detail: Some("@ i:collection { }".to_string()),
            insert_text: Some("@ ${1:i}:${2:collection} {\n\t$0\n}".to_string()),
            insert_text_format: Some(InsertTextFormat::SNIPPET),
            ..Default::default()
        },
        CompletionItem {
            label: "for-range".to_string(),
            kind: Some(CompletionItemKind::KEYWORD),
            detail: Some("@ i:1..10 { }".to_string()),
            insert_text: Some("@ ${1:i}:${2:1}..${3:10} {\n\t$0\n}".to_string()),
            insert_text_format: Some(InsertTextFormat::SNIPPET),
            ..Default::default()
        },
        // Functions
        CompletionItem {
            label: "function".to_string(),
            kind: Some(CompletionItemKind::KEYWORD),
            detail: Some("name(params) { }".to_string()),
            insert_text: Some("${1:name}(${2:params}) {\n\t$0\n}".to_string()),
            insert_text_format: Some(InsertTextFormat::SNIPPET),
            ..Default::default()
        },
        CompletionItem {
            label: "lambda".to_string(),
            kind: Some(CompletionItemKind::KEYWORD),
            detail: Some("x -> expr".to_string()),
            insert_text: Some("${1:x} -> ${0:expr}".to_string()),
            insert_text_format: Some(InsertTextFormat::SNIPPET),
            ..Default::default()
        },
        CompletionItem {
            label: "return".to_string(),
            kind: Some(CompletionItemKind::KEYWORD),
            detail: Some("<~ value".to_string()),
            insert_text: Some("<~ $0".to_string()),
            insert_text_format: Some(InsertTextFormat::SNIPPET),
            ..Default::default()
        },
        // Error handling
        CompletionItem {
            label: "try".to_string(),
            kind: Some(CompletionItemKind::KEYWORD),
            detail: Some("!? { } :! { }".to_string()),
            insert_text: Some("!? {\n\t$1\n} :! {\n\t$0\n}".to_string()),
            insert_text_format: Some(InsertTextFormat::SNIPPET),
            ..Default::default()
        },
        CompletionItem {
            label: "try-finally".to_string(),
            kind: Some(CompletionItemKind::KEYWORD),
            detail: Some("!? { } :! { } :> { }".to_string()),
            insert_text: Some("!? {\n\t$1\n} :! {\n\t$2\n} :> {\n\t$0\n}".to_string()),
            insert_text_format: Some(InsertTextFormat::SNIPPET),
            ..Default::default()
        },
        // I/O
        CompletionItem {
            label: "print".to_string(),
            kind: Some(CompletionItemKind::KEYWORD),
            detail: Some(">> expr".to_string()),
            insert_text: Some(">> $0".to_string()),
            insert_text_format: Some(InsertTextFormat::SNIPPET),
            ..Default::default()
        },
        CompletionItem {
            label: "println".to_string(),
            kind: Some(CompletionItemKind::KEYWORD),
            detail: Some(">> expr ¶".to_string()),
            insert_text: Some(">> $0 ¶".to_string()),
            insert_text_format: Some(InsertTextFormat::SNIPPET),
            ..Default::default()
        },
        CompletionItem {
            label: "input".to_string(),
            kind: Some(CompletionItemKind::KEYWORD),
            detail: Some("<< \"prompt\" variable".to_string()),
            insert_text: Some("<< \"${1:prompt}\" ${0:variable}".to_string()),
            insert_text_format: Some(InsertTextFormat::SNIPPET),
            ..Default::default()
        },
        // Module system
        CompletionItem {
            label: "import".to_string(),
            kind: Some(CompletionItemKind::KEYWORD),
            detail: Some("<# ./path <= alias".to_string()),
            insert_text: Some("<# ./${1:module} <= ${0:alias}".to_string()),
            insert_text_format: Some(InsertTextFormat::SNIPPET),
            ..Default::default()
        },
        CompletionItem {
            label: "export".to_string(),
            kind: Some(CompletionItemKind::KEYWORD),
            detail: Some("#> { items }".to_string()),
            insert_text: Some("#> {\n\t$0\n}".to_string()),
            insert_text_format: Some(InsertTextFormat::SNIPPET),
            ..Default::default()
        },
        CompletionItem {
            label: "module".to_string(),
            kind: Some(CompletionItemKind::KEYWORD),
            detail: Some("# module_name".to_string()),
            insert_text: Some("# ${0:module_name}".to_string()),
            insert_text_format: Some(InsertTextFormat::SNIPPET),
            ..Default::default()
        },
        // Literals
        CompletionItem {
            label: "true".to_string(),
            kind: Some(CompletionItemKind::CONSTANT),
            detail: Some("#1".to_string()),
            insert_text: Some("#1".to_string()),
            insert_text_format: Some(InsertTextFormat::PLAIN_TEXT),
            ..Default::default()
        },
        CompletionItem {
            label: "false".to_string(),
            kind: Some(CompletionItemKind::CONSTANT),
            detail: Some("#0".to_string()),
            insert_text: Some("#0".to_string()),
            insert_text_format: Some(InsertTextFormat::PLAIN_TEXT),
            ..Default::default()
        },
    ]
}

impl Default for Analyzer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_analyzer_creation() {
        let analyzer = Analyzer::new();
        assert_eq!(analyzer.document_count(), 0);
    }

    #[test]
    fn test_open_document() {
        let analyzer = Analyzer::new();
        analyzer.open_document(Arc::from("file:///test.zy"), "x = 5".to_string(), 1);

        assert!(analyzer.has_document("file:///test.zy"));
        assert_eq!(analyzer.document_count(), 1);
    }

    #[test]
    fn test_close_document() {
        let analyzer = Analyzer::new();
        analyzer.open_document(Arc::from("file:///test.zy"), "x = 5".to_string(), 1);
        analyzer.close_document("file:///test.zy");

        assert!(!analyzer.has_document("file:///test.zy"));
        assert_eq!(analyzer.document_count(), 0);
    }

    #[test]
    fn test_get_diagnostics_valid() {
        let analyzer = Analyzer::new();
        analyzer.open_document(Arc::from("file:///test.zy"), ">> \"Hello\"".to_string(), 1);

        let diagnostics = analyzer.get_diagnostics("file:///test.zy");
        // Valid code should have minimal diagnostics
        assert!(diagnostics.iter().all(|d| d.severity != Some(lsp_types::DiagnosticSeverity::ERROR)));
    }

    #[test]
    fn test_get_semantic_tokens() {
        let analyzer = Analyzer::new();
        analyzer.open_document(Arc::from("file:///test.zy"), "x = 5".to_string(), 1);

        let tokens = analyzer.get_semantic_tokens("file:///test.zy");
        assert!(tokens.is_some());
        assert!(!tokens.unwrap().data.is_empty());
    }

    #[test]
    fn test_get_document_symbols() {
        let analyzer = Analyzer::new();
        analyzer.open_document(
            Arc::from("file:///test.zy"),
            "x = 5\ny = 10".to_string(),
            1,
        );

        let symbols = analyzer.get_document_symbols("file:///test.zy");
        assert_eq!(symbols.len(), 2);
    }

    #[test]
    fn test_semantic_tokens_legend() {
        let analyzer = Analyzer::new();
        let legend = analyzer.semantic_tokens_legend();

        assert!(!legend.token_types.is_empty());
        assert!(!legend.token_modifiers.is_empty());
    }

    #[test]
    fn test_workspace_symbol_search() {
        let analyzer = Analyzer::new();
        analyzer.open_document(
            Arc::from("file:///test.zy"),
            "myVariable = 5\nmyFunction(x) { <~ x }".to_string(),
            1,
        );

        let results = analyzer.workspace_symbol_search("my");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_symbol_stats() {
        let analyzer = Analyzer::new();
        analyzer.open_document(Arc::from("file:///test.zy"), "x = 5\ny = 10".to_string(), 1);

        let stats = analyzer.symbol_stats();
        assert_eq!(stats.documents, 1);
        assert_eq!(stats.symbols, 2);
    }

    #[test]
    fn test_update_document() {
        let analyzer = Analyzer::new();
        analyzer.open_document(Arc::from("file:///test.zy"), "x = 5".to_string(), 1);

        let symbols_before = analyzer.get_document_symbols("file:///test.zy");
        assert_eq!(symbols_before.len(), 1);

        analyzer.update_document("file:///test.zy", "a = 1\nb = 2\nc = 3".to_string(), 2);

        let symbols_after = analyzer.get_document_symbols("file:///test.zy");
        assert_eq!(symbols_after.len(), 3);
    }

    #[test]
    fn test_find_definition() {
        let analyzer = Analyzer::new();
        analyzer.open_document(
            Arc::from("file:///test.zy"),
            "myVar = 5".to_string(),
            1,
        );

        // Position at the start of myVar (0-indexed)
        let loc = analyzer.find_definition("file:///test.zy", Position { line: 0, character: 0 });
        assert!(loc.is_some());
    }

    #[test]
    fn test_get_hover() {
        let analyzer = Analyzer::new();
        analyzer.open_document(
            Arc::from("file:///test.zy"),
            "myFunc(a, b) { <~ a + b }".to_string(),
            1,
        );

        let hover = analyzer.get_hover("file:///test.zy", Position { line: 0, character: 0 });
        assert!(hover.is_some());
    }
}
