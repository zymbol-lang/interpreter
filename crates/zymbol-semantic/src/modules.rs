//! Module system semantic analysis for Zymbol-Lang
//!
//! Provides semantic validation for the Zymbol module system, including:
//! - File name validation (module name must match filename)
//! - Path resolution (./, ../, subdirectories)
//! - Import validation (modules exist, no circular dependencies)
//! - Export validation (items exist and are visible)
//! - Re-export validation (correct types, items exist)

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use thiserror::Error;
use zymbol_ast::{ExportItem, ImportStmt, ItemType, ModuleDecl, ModulePath, Program};
use zymbol_error::Diagnostic;
use zymbol_span::{Position, Span};

/// Semantic validation errors
#[derive(Debug, Error)]
pub enum SemanticError {
    #[error("E001: Module name '{module_name}' does not match file name '{file_name}'")]
    ModuleNameMismatch {
        module_name: String,
        file_name: String,
        span: Span,
    },

    #[error("E002: Module '{path}' not found")]
    ModuleNotFound { path: String, span: Span },

    #[error("E003: Failed to resolve path '{path}'")]
    PathResolutionFailed { path: String, span: Span },

    #[error("E004: Circular dependency detected: {cycle}")]
    CircularDependency { cycle: String, span: Span },

    #[error("E005: Item '{item}' not found in module")]
    ItemNotFound { item: String, span: Span },

    #[error("E006: Cannot re-export '{item}' - item not found in module '{module}'")]
    ReExportItemNotFound {
        item: String,
        module: String,
        span: Span,
    },

    #[error("E007: Type mismatch in re-export: '{item}' is a {actual} but used '::' (function) syntax")]
    ReExportTypeMismatch {
        item: String,
        actual: String,
        span: Span,
    },

    #[error("E008: Item '{item}' is private and cannot be imported")]
    PrivateItem { item: String, span: Span },

    #[error("E009: Duplicate export of item '{item}'")]
    DuplicateExport { item: String, span: Span },

    #[error("E010: Cannot re-export private item '{item}' from module '{module}'")]
    ReExportPrivateItem {
        item: String,
        module: String,
        span: Span,
    },

    #[error("E011: Import alias '{alias}' conflicts with existing import")]
    ImportAliasConflict { alias: String, span: Span },

    #[error("E012: Module '{module}' does not export item '{item}'")]
    ModuleDoesNotExport {
        module: String,
        item: String,
        span: Span,
    },
}

impl SemanticError {
    /// Convert to Diagnostic for unified error reporting
    pub fn to_diagnostic(&self) -> Diagnostic {
        match self {
            SemanticError::ModuleNameMismatch { span, .. } => Diagnostic::error(self.to_string())
                .with_span(*span)
                .with_help("The module name must match the filename (without .zy extension)"),

            SemanticError::ModuleNotFound { span, .. } => Diagnostic::error(self.to_string())
                .with_span(*span)
                .with_help("Check that the module file exists at the specified path"),

            SemanticError::PathResolutionFailed { span, .. } => {
                Diagnostic::error(self.to_string())
                    .with_span(*span)
                    .with_help("Ensure the path is valid and does not go above the root")
            }

            SemanticError::CircularDependency { span, .. } => Diagnostic::error(self.to_string())
                .with_span(*span)
                .with_help("Refactor to remove the circular dependency"),

            SemanticError::ItemNotFound { span, .. } => Diagnostic::error(self.to_string())
                .with_span(*span)
                .with_help("Check that the item is defined in the module"),

            SemanticError::ReExportItemNotFound { span, .. } => {
                Diagnostic::error(self.to_string())
                    .with_span(*span)
                    .with_help("The item must be exported by the imported module")
            }

            SemanticError::ReExportTypeMismatch { span, actual, .. } => {
                let help = if actual == "constant" {
                    "Use '.' syntax for constants: alias.CONSTANT"
                } else {
                    "Use '::' syntax for functions: alias::function"
                };
                Diagnostic::error(self.to_string())
                    .with_span(*span)
                    .with_help(help)
            }

            SemanticError::PrivateItem { span, .. } => Diagnostic::error(self.to_string())
                .with_span(*span)
                .with_help("Only exported items can be imported"),

            SemanticError::DuplicateExport { span, .. } => Diagnostic::error(self.to_string())
                .with_span(*span)
                .with_help("Each item can only be exported once"),

            SemanticError::ReExportPrivateItem { span, .. } => {
                Diagnostic::error(self.to_string())
                    .with_span(*span)
                    .with_help("Cannot re-export items that are not exported by the source module")
            }

            SemanticError::ImportAliasConflict { span, .. } => {
                Diagnostic::error(self.to_string())
                    .with_span(*span)
                    .with_help("Use a different alias for this import")
            }

            SemanticError::ModuleDoesNotExport { span, .. } => {
                Diagnostic::error(self.to_string())
                    .with_span(*span)
                    .with_help("Check the module's export block")
            }
        }
    }
}

/// Information about an exported item
#[derive(Debug, Clone, PartialEq)]
pub enum ExportedItem {
    /// Own function defined in this module
    Function { name: String },
    /// Own constant defined in this module
    Constant { name: String },
    /// Re-exported function from another module
    ReExportedFunction {
        original_module: String,
        original_name: String,
        exported_name: String,
    },
    /// Re-exported constant from another module
    ReExportedConstant {
        original_module: String,
        original_name: String,
        exported_name: String,
    },
}

impl ExportedItem {
    pub fn name(&self) -> &str {
        match self {
            ExportedItem::Function { name } => name,
            ExportedItem::Constant { name } => name,
            ExportedItem::ReExportedFunction { exported_name, .. } => exported_name,
            ExportedItem::ReExportedConstant { exported_name, .. } => exported_name,
        }
    }

    pub fn is_function(&self) -> bool {
        matches!(
            self,
            ExportedItem::Function { .. } | ExportedItem::ReExportedFunction { .. }
        )
    }

    pub fn is_constant(&self) -> bool {
        matches!(
            self,
            ExportedItem::Constant { .. } | ExportedItem::ReExportedConstant { .. }
        )
    }
}

/// Export table for a module
#[derive(Debug, Clone)]
pub struct ExportTable {
    /// Module name
    pub module_name: String,
    /// File path
    pub file_path: PathBuf,
    /// Exported items
    pub items: HashMap<String, ExportedItem>,
}

impl ExportTable {
    pub fn new(module_name: String, file_path: PathBuf) -> Self {
        Self {
            module_name,
            file_path,
            items: HashMap::new(),
        }
    }

    pub fn add_item(&mut self, item: ExportedItem) {
        self.items.insert(item.name().to_string(), item);
    }

    pub fn get_item(&self, name: &str) -> Option<&ExportedItem> {
        self.items.get(name)
    }

    pub fn has_item(&self, name: &str) -> bool {
        self.items.contains_key(name)
    }
}

/// Module analyzer for semantic validation
pub struct ModuleAnalyzer {
    /// Base directory for resolving relative paths
    pub(crate) base_dir: PathBuf,
    /// Export tables for all loaded modules (module_path -> ExportTable)
    pub(crate) export_tables: HashMap<PathBuf, ExportTable>,
    /// Import graph for circular dependency detection (module -> [dependencies])
    pub(crate) import_graph: HashMap<PathBuf, Vec<PathBuf>>,
    /// Diagnostics collected during analysis
    pub(crate) diagnostics: Vec<Diagnostic>,
}

impl ModuleAnalyzer {
    /// Create a new module analyzer with a base directory
    pub fn new<P: AsRef<Path>>(base_dir: P) -> Self {
        Self {
            base_dir: base_dir.as_ref().to_path_buf(),
            export_tables: HashMap::new(),
            import_graph: HashMap::new(),
            diagnostics: Vec::new(),
        }
    }

    /// Analyze a program and validate module semantics
    pub fn analyze(&mut self, program: &Program, file_path: &Path) -> Result<(), Vec<Diagnostic>> {
        // Clear previous diagnostics
        self.diagnostics.clear();

        // Validate module declaration if present
        if let Some(ref module_decl) = program.module_decl {
            if let Err(err) = self.validate_module_name(module_decl, file_path) {
                self.diagnostics.push(err.to_diagnostic());
            }
        }

        // Validate and register imports
        for import in &program.imports {
            if let Err(err) = self.validate_import(import, file_path) {
                self.diagnostics.push(err.to_diagnostic());
            }
        }

        // Check for circular dependencies
        if let Err(err) = self.check_circular_dependencies(file_path) {
            self.diagnostics.push(err.to_diagnostic());
        }

        // Build export table
        if let Some(ref module_decl) = program.module_decl {
            if let Err(errors) = self.build_export_table(module_decl, file_path, &program.imports) {
                self.diagnostics.extend(errors.into_iter().map(|e| e.to_diagnostic()));
            }
        }

        if self.diagnostics.is_empty() {
            Ok(())
        } else {
            Err(self.diagnostics.clone())
        }
    }

    /// Validate that module name matches filename
    pub(crate) fn validate_module_name(
        &self,
        module_decl: &ModuleDecl,
        file_path: &Path,
    ) -> Result<(), SemanticError> {
        let file_stem = file_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("");

        if module_decl.name != file_stem {
            return Err(SemanticError::ModuleNameMismatch {
                module_name: module_decl.name.clone(),
                file_name: file_stem.to_string(),
                span: module_decl.span,
            });
        }

        Ok(())
    }

    /// Resolve module path to absolute file path
    pub(crate) fn resolve_module_path(
        &self,
        module_path: &ModulePath,
        current_file: &Path,
    ) -> Result<PathBuf, SemanticError> {
        let current_dir = current_file.parent().unwrap_or(&self.base_dir);

        let mut resolved = current_dir.to_path_buf();

        // Handle parent directory navigation
        if module_path.is_relative {
            for _ in 0..module_path.parent_levels {
                if !resolved.pop() {
                    return Err(SemanticError::PathResolutionFailed {
                        path: format!("{:?}", module_path.components),
                        span: module_path.span,
                    });
                }
            }
        }

        // Add path components
        for component in &module_path.components {
            resolved.push(component);
        }

        // Add .zy extension
        resolved.set_extension("zy");

        Ok(resolved)
    }

    /// Validate an import statement
    fn validate_import(
        &mut self,
        import: &ImportStmt,
        current_file: &Path,
    ) -> Result<(), SemanticError> {
        // Resolve the module path
        let resolved_path = self.resolve_module_path(&import.path, current_file)?;

        // Check if module file exists
        if !resolved_path.exists() {
            return Err(SemanticError::ModuleNotFound {
                path: resolved_path.to_string_lossy().to_string(),
                span: import.span,
            });
        }

        // Register in import graph
        self.import_graph
            .entry(current_file.to_path_buf())
            .or_default()
            .push(resolved_path);

        Ok(())
    }

    /// Check for circular dependencies using DFS
    pub(crate) fn check_circular_dependencies(&self, start: &Path) -> Result<(), SemanticError> {
        let mut visited = HashSet::new();
        let mut rec_stack = Vec::new();

        self.dfs_cycle_check(start, &mut visited, &mut rec_stack)
    }

    fn dfs_cycle_check(
        &self,
        current: &Path,
        visited: &mut HashSet<PathBuf>,
        rec_stack: &mut Vec<PathBuf>,
    ) -> Result<(), SemanticError> {
        if rec_stack.contains(&current.to_path_buf()) {
            // Found a cycle
            let cycle_start = rec_stack
                .iter()
                .position(|p| p == current)
                .unwrap_or(0);
            let cycle: Vec<String> = rec_stack[cycle_start..]
                .iter()
                .chain(std::iter::once(&current.to_path_buf()))
                .map(|p| p.to_string_lossy().to_string())
                .collect();

            // Create a dummy span since we don't have a specific location for circular deps
            let dummy_span = Span::new(
                Position { line: 1, column: 1, byte_offset: 0 },
                Position { line: 1, column: 1, byte_offset: 0 },
                zymbol_span::FileId(0),
            );
            return Err(SemanticError::CircularDependency {
                cycle: cycle.join(" → "),
                span: dummy_span,
            });
        }

        if visited.contains(&current.to_path_buf()) {
            return Ok(());
        }

        visited.insert(current.to_path_buf());
        rec_stack.push(current.to_path_buf());

        if let Some(dependencies) = self.import_graph.get(current) {
            for dep in dependencies {
                self.dfs_cycle_check(dep, visited, rec_stack)?;
            }
        }

        rec_stack.pop();
        Ok(())
    }

    /// Build export table for a module
    pub(crate) fn build_export_table(
        &mut self,
        module_decl: &ModuleDecl,
        file_path: &Path,
        imports: &[ImportStmt],
    ) -> Result<(), Vec<SemanticError>> {
        let mut errors = Vec::new();
        let mut export_table = ExportTable::new(module_decl.name.clone(), file_path.to_path_buf());

        // Build import alias map
        let mut import_map: HashMap<String, PathBuf> = HashMap::new();
        for import in imports {
            if let Ok(resolved) = self.resolve_module_path(&import.path, file_path) {
                import_map.insert(import.alias.clone(), resolved);
            }
        }

        if let Some(ref export_block) = module_decl.export_block {
            let mut exported_names = HashSet::new();

            for export_item in &export_block.items {
                match export_item {
                    ExportItem::Own { name, rename, span } => {
                        let public_name = rename.as_ref().unwrap_or(name);

                        // Check for duplicates by public name
                        if exported_names.contains(public_name) {
                            errors.push(SemanticError::DuplicateExport {
                                item: public_name.clone(),
                                span: *span,
                            });
                            continue;
                        }

                        exported_names.insert(public_name.clone());

                        // Note: The actual type (function vs constant) is determined later
                        // when we have access to the program's AST. For now we store it
                        // as a function, which will be validated during type checking.
                        export_table.add_item(ExportedItem::Function {
                            name: public_name.clone(),
                        });
                    }

                    ExportItem::ReExport {
                        module_alias,
                        item_name,
                        item_type,
                        rename,
                        span,
                    } => {
                        let exported_name = rename.as_ref().unwrap_or(item_name);

                        // Check for duplicates
                        if exported_names.contains(exported_name) {
                            errors.push(SemanticError::DuplicateExport {
                                item: exported_name.clone(),
                                span: *span,
                            });
                            continue;
                        }

                        exported_names.insert(exported_name.clone());

                        // Verify the imported module exists
                        if import_map.contains_key(module_alias) {
                            // Verify the item exists in the imported module
                            // (this would require loading and analyzing the imported module first)
                            // For now, we'll add it to the export table

                            let item = match item_type {
                                ItemType::Function => ExportedItem::ReExportedFunction {
                                    original_module: module_alias.clone(),
                                    original_name: item_name.clone(),
                                    exported_name: exported_name.clone(),
                                },
                                ItemType::Constant => ExportedItem::ReExportedConstant {
                                    original_module: module_alias.clone(),
                                    original_name: item_name.clone(),
                                    exported_name: exported_name.clone(),
                                },
                            };

                            export_table.add_item(item);
                        } else {
                            errors.push(SemanticError::ReExportItemNotFound {
                                item: item_name.clone(),
                                module: module_alias.clone(),
                                span: *span,
                            });
                        }
                    }
                }
            }
        }

        // Store export table
        self.export_tables
            .insert(file_path.to_path_buf(), export_table);

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Get export table for a module
    pub fn get_export_table(&self, file_path: &Path) -> Option<&ExportTable> {
        self.export_tables.get(file_path)
    }

    /// Get all diagnostics
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    /// Validate that exported items actually exist in the program
    pub fn validate_exports(&mut self, program: &Program, file_path: &Path) {
        use zymbol_ast::Statement;

        // Collect all defined functions and constants
        let mut defined_functions: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut defined_constants: std::collections::HashSet<String> = std::collections::HashSet::new();

        for stmt in &program.statements {
            match stmt {
                Statement::FunctionDecl(func) => {
                    defined_functions.insert(func.name.clone());
                }
                Statement::ConstDecl(const_decl) => {
                    defined_constants.insert(const_decl.name.clone());
                }
                _ => {}
            }
        }

        // Validate each exported item exists
        if let Some(ref module_decl) = program.module_decl {
            if let Some(ref export_block) = module_decl.export_block {
                for item in &export_block.items {
                    if let ExportItem::Own { name, span, .. } = item {
                        // Validate by internal name (the symbol that must exist in this file)
                        if !defined_functions.contains(name) && !defined_constants.contains(name) {
                            self.diagnostics.push(
                                SemanticError::ItemNotFound {
                                    item: name.clone(),
                                    span: *span,
                                }.to_diagnostic()
                            );
                        }
                    }
                }
            }
        }

        // Update export table with correct item types
        if let Some(export_table) = self.export_tables.get_mut(file_path) {
            let mut updated_items = HashMap::new();
            for (name, item) in &export_table.items {
                let updated = if defined_constants.contains(name) {
                    match item {
                        ExportedItem::Function { name } => ExportedItem::Constant { name: name.clone() },
                        other => other.clone(),
                    }
                } else {
                    item.clone()
                };
                updated_items.insert(name.clone(), updated);
            }
            export_table.items = updated_items;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};
    use zymbol_ast::{ExportBlock, ExportItem, ModuleDecl, ModulePath};
    use zymbol_span::{FileId, Position, Span};

    fn create_test_span() -> Span {
        Span {
            start: Position {
                line: 1,
                column: 1,
                byte_offset: 0,
            },
            end: Position {
                line: 1,
                column: 10,
                byte_offset: 9,
            },
            file_id: FileId(0),
        }
    }

    #[test]
    fn test_validate_module_name_matches() {
        let analyzer = ModuleAnalyzer::new("/tmp");
        let module_decl = ModuleDecl::new(
            "math_utils".to_string(),
            None,
            create_test_span(),
        );

        let result = analyzer.validate_module_name(&module_decl, Path::new("/tmp/math_utils.zy"));
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_module_name_mismatch() {
        let analyzer = ModuleAnalyzer::new("/tmp");
        let module_decl = ModuleDecl::new(
            "wrong_name".to_string(),
            None,
            create_test_span(),
        );

        let result = analyzer.validate_module_name(&module_decl, Path::new("/tmp/math_utils.zy"));
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            SemanticError::ModuleNameMismatch { .. }
        ));
    }

    #[test]
    fn test_resolve_relative_path() {
        let analyzer = ModuleAnalyzer::new("/project");
        let module_path = ModulePath::new(
            vec!["lib".to_string(), "math_utils".to_string()],
            true,
            0,
            create_test_span(),
        );

        let current_file = Path::new("/project/app.zy");
        let resolved = analyzer
            .resolve_module_path(&module_path, current_file)
            .unwrap();

        assert_eq!(resolved, PathBuf::from("/project/lib/math_utils.zy"));
    }

    #[test]
    fn test_resolve_parent_directory() {
        let analyzer = ModuleAnalyzer::new("/project");
        let module_path = ModulePath::new(
            vec!["utils".to_string(), "config".to_string()],
            true,
            1,
            create_test_span(),
        );

        let current_file = Path::new("/project/lib/core.zy");
        let resolved = analyzer
            .resolve_module_path(&module_path, current_file)
            .unwrap();

        assert_eq!(resolved, PathBuf::from("/project/utils/config.zy"));
    }

    #[test]
    fn test_circular_dependency_detection() {
        let mut analyzer = ModuleAnalyzer::new("/project");

        // Create a cycle: A -> B -> C -> A
        let path_a = PathBuf::from("/project/a.zy");
        let path_b = PathBuf::from("/project/b.zy");
        let path_c = PathBuf::from("/project/c.zy");

        analyzer
            .import_graph
            .insert(path_a.clone(), vec![path_b.clone()]);
        analyzer
            .import_graph
            .insert(path_b.clone(), vec![path_c.clone()]);
        analyzer
            .import_graph
            .insert(path_c.clone(), vec![path_a.clone()]);

        let result = analyzer.check_circular_dependencies(&path_a);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            SemanticError::CircularDependency { .. }
        ));
    }

    #[test]
    fn test_export_table_duplicate_detection() {
        let mut analyzer = ModuleAnalyzer::new("/project");

        let export_block = ExportBlock::new(
            vec![
                ExportItem::own("add".to_string(), None, create_test_span()),
                ExportItem::own("add".to_string(), None, create_test_span()), // Duplicate
            ],
            create_test_span(),
        );

        let module_decl = ModuleDecl::new(
            "math".to_string(),
            Some(export_block),
            create_test_span(),
        );

        let result = analyzer.build_export_table(
            &module_decl,
            Path::new("/project/math.zy"),
            &[],
        );

        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert_eq!(errors.len(), 1);
        assert!(matches!(
            errors[0],
            SemanticError::DuplicateExport { .. }
        ));
    }
}
