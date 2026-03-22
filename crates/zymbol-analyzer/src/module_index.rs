//! Module index for Zymbol-Lang LSP
//!
//! Provides indexing of module exports for:
//! - Cross-file go-to-definition
//! - Module completions
//! - Import validation
//! - Circular dependency detection

use dashmap::DashMap;
use parking_lot::RwLock;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use zymbol_span::Span;

/// Kind of exported symbol
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportedKind {
    /// Function (called with `::`)
    Function,
    /// Constant (accessed with `.`)
    Constant,
    /// Variable (accessed with `.`)
    Variable,
}

impl ExportedKind {
    /// Check if this kind uses function call syntax (`::`)
    pub fn is_callable(&self) -> bool {
        matches!(self, ExportedKind::Function)
    }

    /// Check if this kind uses property access syntax (`.`)
    pub fn is_property(&self) -> bool {
        matches!(self, ExportedKind::Constant | ExportedKind::Variable)
    }
}

/// An exported symbol from a module
#[derive(Debug, Clone)]
pub struct ExportedSymbol {
    /// Symbol name
    pub name: String,
    /// Kind of symbol
    pub kind: ExportedKind,
    /// Span in source file
    pub span: Span,
    /// Original name if this is a re-export with rename
    pub original_name: Option<String>,
    /// Source module if this is a re-export
    pub source_module: Option<PathBuf>,
    /// Function parameters (if function)
    pub parameters: Option<Vec<String>>,
}

impl ExportedSymbol {
    /// Create a new function export
    pub fn function(name: String, span: Span, parameters: Vec<String>) -> Self {
        Self {
            name,
            kind: ExportedKind::Function,
            span,
            original_name: None,
            source_module: None,
            parameters: Some(parameters),
        }
    }

    /// Create a new constant export
    pub fn constant(name: String, span: Span) -> Self {
        Self {
            name,
            kind: ExportedKind::Constant,
            span,
            original_name: None,
            source_module: None,
            parameters: None,
        }
    }

    /// Create a re-exported symbol
    pub fn re_export(
        name: String,
        kind: ExportedKind,
        span: Span,
        original_name: String,
        source_module: PathBuf,
    ) -> Self {
        Self {
            name,
            kind,
            span,
            original_name: Some(original_name),
            source_module: Some(source_module),
            parameters: None,
        }
    }
}

/// Exports from a single module
#[derive(Debug, Clone)]
pub struct ModuleExports {
    /// Module name (from `# module_name` declaration)
    pub name: String,
    /// File path
    pub file_path: PathBuf,
    /// URI
    pub uri: Arc<str>,
    /// Exported symbols
    pub exports: Vec<ExportedSymbol>,
}

impl ModuleExports {
    /// Create new module exports
    pub fn new(name: String, file_path: PathBuf, uri: Arc<str>) -> Self {
        Self {
            name,
            file_path,
            uri,
            exports: Vec::new(),
        }
    }

    /// Add an export
    pub fn add_export(&mut self, symbol: ExportedSymbol) {
        self.exports.push(symbol);
    }

    /// Get export by name
    pub fn get_export(&self, name: &str) -> Option<&ExportedSymbol> {
        self.exports.iter().find(|e| e.name == name)
    }

    /// Get all functions
    pub fn functions(&self) -> impl Iterator<Item = &ExportedSymbol> {
        self.exports.iter().filter(|e| e.kind == ExportedKind::Function)
    }

    /// Get all constants
    pub fn constants(&self) -> impl Iterator<Item = &ExportedSymbol> {
        self.exports
            .iter()
            .filter(|e| matches!(e.kind, ExportedKind::Constant | ExportedKind::Variable))
    }
}

/// Module import information
#[derive(Debug, Clone)]
pub struct ImportInfo {
    /// Alias used in the importing file
    pub alias: String,
    /// Resolved absolute path
    pub resolved_path: PathBuf,
    /// Span of the import statement
    pub span: Span,
}

/// Index of all module exports in the workspace
#[derive(Debug, Default)]
pub struct ModuleIndex {
    /// Exports by file path
    exports: DashMap<PathBuf, ModuleExports>,
    /// Import graph: file -> [imported files]
    import_graph: RwLock<HashMap<PathBuf, Vec<ImportInfo>>>,
    /// Import alias mapping: (file, alias) -> resolved path
    alias_map: DashMap<(PathBuf, String), PathBuf>,
}

impl ModuleIndex {
    /// Create a new module index
    pub fn new() -> Self {
        Self {
            exports: DashMap::new(),
            import_graph: RwLock::new(HashMap::new()),
            alias_map: DashMap::new(),
        }
    }

    /// Index exports for a module
    pub fn index_module(&self, path: PathBuf, exports: ModuleExports) {
        self.exports.insert(path, exports);
    }

    /// Remove a module from the index
    pub fn remove_module(&self, path: &Path) {
        self.exports.remove(path);
        self.import_graph.write().remove(path);
        // Remove aliases for this file
        self.alias_map.retain(|(file, _), _| file != path);
    }

    /// Get exports for a module
    pub fn get_exports(&self, path: &Path) -> Option<ModuleExports> {
        self.exports.get(path).map(|r| r.clone())
    }

    /// Register an import
    pub fn register_import(&self, from_file: &Path, import: ImportInfo) {
        // Add to import graph
        {
            let mut graph = self.import_graph.write();
            graph
                .entry(from_file.to_path_buf())
                .or_default()
                .push(import.clone());
        }

        // Add alias mapping
        self.alias_map.insert(
            (from_file.to_path_buf(), import.alias),
            import.resolved_path,
        );
    }

    /// Clear imports for a file (before re-indexing)
    pub fn clear_imports(&self, from_file: &Path) {
        self.import_graph.write().remove(from_file);
        self.alias_map.retain(|(file, _), _| file != from_file);
    }

    /// Resolve an alias to a module path
    pub fn resolve_alias(&self, from_file: &Path, alias: &str) -> Option<PathBuf> {
        self.alias_map
            .get(&(from_file.to_path_buf(), alias.to_string()))
            .map(|r| r.clone())
    }

    /// Resolve a symbol in a module (alias::symbol or alias.symbol)
    pub fn resolve_symbol(
        &self,
        from_file: &Path,
        alias: &str,
        symbol: &str,
    ) -> Option<(ModuleExports, ExportedSymbol)> {
        let module_path = self.resolve_alias(from_file, alias)?;
        let exports = self.get_exports(&module_path)?;
        let symbol = exports.get_export(symbol)?.clone();
        Some((exports, symbol))
    }

    /// Check for circular dependencies starting from a file
    pub fn check_circular_deps(&self, start: &Path) -> Option<Vec<PathBuf>> {
        let graph = self.import_graph.read();

        let mut visited = HashSet::new();
        let mut rec_stack = Vec::new();

        Self::dfs_cycle(&graph, start, &mut visited, &mut rec_stack)
    }

    fn dfs_cycle(
        graph: &HashMap<PathBuf, Vec<ImportInfo>>,
        current: &Path,
        visited: &mut HashSet<PathBuf>,
        rec_stack: &mut Vec<PathBuf>,
    ) -> Option<Vec<PathBuf>> {
        if rec_stack.contains(&current.to_path_buf()) {
            // Found a cycle - return the cycle path
            let cycle_start = rec_stack.iter().position(|p| p == current)?;
            let mut cycle: Vec<PathBuf> = rec_stack[cycle_start..].to_vec();
            cycle.push(current.to_path_buf());
            return Some(cycle);
        }

        if visited.contains(&current.to_path_buf()) {
            return None;
        }

        visited.insert(current.to_path_buf());
        rec_stack.push(current.to_path_buf());

        if let Some(imports) = graph.get(current) {
            for import in imports {
                if let Some(cycle) = Self::dfs_cycle(graph, &import.resolved_path, visited, rec_stack)
                {
                    return Some(cycle);
                }
            }
        }

        rec_stack.pop();
        None
    }

    /// Get all files that import a given module
    pub fn get_importers(&self, module_path: &Path) -> Vec<PathBuf> {
        let graph = self.import_graph.read();

        graph
            .iter()
            .filter(|(_, imports)| imports.iter().any(|i| i.resolved_path == module_path))
            .map(|(file, _)| file.clone())
            .collect()
    }

    /// Get imports for a file
    pub fn get_imports(&self, file: &Path) -> Vec<ImportInfo> {
        self.import_graph
            .read()
            .get(file)
            .cloned()
            .unwrap_or_default()
    }

    /// Get all indexed modules
    pub fn all_modules(&self) -> Vec<PathBuf> {
        self.exports.iter().map(|entry| entry.key().clone()).collect()
    }

    /// Search for exports matching a query
    pub fn search_exports(&self, query: &str) -> Vec<(PathBuf, ExportedSymbol)> {
        let query_lower = query.to_lowercase();
        let mut results = Vec::new();

        for entry in self.exports.iter() {
            for export in &entry.exports {
                if export.name.to_lowercase().contains(&query_lower) {
                    results.push((entry.key().clone(), export.clone()));
                }
            }
        }

        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zymbol_span::{FileId, Position};

    fn test_span() -> Span {
        Span::new(
            Position::new(1, 1, 0),
            Position::new(1, 10, 9),
            FileId(0),
        )
    }

    #[test]
    fn test_module_index_basic() {
        let index = ModuleIndex::new();
        let path = PathBuf::from("/test/math.zy");

        let mut exports = ModuleExports::new(
            "math".to_string(),
            path.clone(),
            Arc::from("file:///test/math.zy"),
        );
        exports.add_export(ExportedSymbol::function(
            "add".to_string(),
            test_span(),
            vec!["a".to_string(), "b".to_string()],
        ));
        exports.add_export(ExportedSymbol::constant("PI".to_string(), test_span()));

        index.index_module(path.clone(), exports);

        let retrieved = index.get_exports(&path);
        assert!(retrieved.is_some());

        let exports = retrieved.unwrap();
        assert_eq!(exports.exports.len(), 2);
        assert!(exports.get_export("add").is_some());
        assert!(exports.get_export("PI").is_some());
    }

    #[test]
    fn test_import_resolution() {
        let index = ModuleIndex::new();

        let main_path = PathBuf::from("/project/main.zy");
        let math_path = PathBuf::from("/project/lib/math.zy");

        // Register import
        index.register_import(
            &main_path,
            ImportInfo {
                alias: "m".to_string(),
                resolved_path: math_path.clone(),
                span: test_span(),
            },
        );

        // Resolve alias
        let resolved = index.resolve_alias(&main_path, "m");
        assert_eq!(resolved, Some(math_path));
    }

    #[test]
    fn test_circular_dependency_detection() {
        let index = ModuleIndex::new();

        let a_path = PathBuf::from("/project/a.zy");
        let b_path = PathBuf::from("/project/b.zy");
        let c_path = PathBuf::from("/project/c.zy");

        // Create cycle: a -> b -> c -> a
        index.register_import(
            &a_path,
            ImportInfo {
                alias: "b".to_string(),
                resolved_path: b_path.clone(),
                span: test_span(),
            },
        );
        index.register_import(
            &b_path,
            ImportInfo {
                alias: "c".to_string(),
                resolved_path: c_path.clone(),
                span: test_span(),
            },
        );
        index.register_import(
            &c_path,
            ImportInfo {
                alias: "a".to_string(),
                resolved_path: a_path.clone(),
                span: test_span(),
            },
        );

        let cycle = index.check_circular_deps(&a_path);
        assert!(cycle.is_some());
    }

    #[test]
    fn test_get_importers() {
        let index = ModuleIndex::new();

        let math_path = PathBuf::from("/project/lib/math.zy");
        let main_path = PathBuf::from("/project/main.zy");
        let test_path = PathBuf::from("/project/test.zy");

        // Both main and test import math
        index.register_import(
            &main_path,
            ImportInfo {
                alias: "m".to_string(),
                resolved_path: math_path.clone(),
                span: test_span(),
            },
        );
        index.register_import(
            &test_path,
            ImportInfo {
                alias: "math".to_string(),
                resolved_path: math_path.clone(),
                span: test_span(),
            },
        );

        let importers = index.get_importers(&math_path);
        assert_eq!(importers.len(), 2);
        assert!(importers.contains(&main_path));
        assert!(importers.contains(&test_path));
    }

    #[test]
    fn test_search_exports() {
        let index = ModuleIndex::new();
        let path = PathBuf::from("/test/utils.zy");

        let mut exports = ModuleExports::new(
            "utils".to_string(),
            path.clone(),
            Arc::from("file:///test/utils.zy"),
        );
        exports.add_export(ExportedSymbol::function(
            "formatString".to_string(),
            test_span(),
            vec![],
        ));
        exports.add_export(ExportedSymbol::function(
            "parseNumber".to_string(),
            test_span(),
            vec![],
        ));

        index.index_module(path, exports);

        let results = index.search_exports("format");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].1.name, "formatString");
    }
}
