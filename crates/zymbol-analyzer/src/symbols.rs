//! Symbol indexing for Zymbol-Lang LSP
//!
//! Provides a three-level index for efficient symbol lookup:
//! 1. by_document: For document cleanup when files change
//! 2. by_name: For go-to-definition lookup
//! 3. references: For find-all-references

use std::collections::HashMap;
use std::sync::Arc;

use crate::symbol_extractor::{Symbol, SymbolKind};
use zymbol_span::Span;

/// A reference to a symbol in the index
#[derive(Debug, Clone)]
pub struct SymbolRef {
    /// Document URI where the symbol is defined
    pub uri: Arc<str>,
    /// Symbol name
    pub name: String,
    /// Symbol kind
    pub kind: SymbolKind,
    /// Location span
    pub span: Span,
    /// Optional detail (e.g., function signature)
    pub detail: Option<String>,
}

impl SymbolRef {
    /// Create from a Symbol and document URI
    pub fn from_symbol(symbol: &Symbol, uri: Arc<str>) -> Self {
        Self {
            uri,
            name: symbol.name.clone(),
            kind: symbol.kind,
            span: symbol.span,
            detail: symbol.detail.clone(),
        }
    }

    /// Convert to LSP Location
    pub fn to_location(&self) -> lsp_types::Location {
        // Create URI - if it doesn't start with file://, add it
        let uri_str = if self.uri.starts_with("file://") {
            self.uri.to_string()
        } else {
            format!("file://{}", &*self.uri)
        };
        let uri = lsp_types::Url::parse(&uri_str)
            .unwrap_or_else(|_| lsp_types::Url::parse("file:///unknown").unwrap());
        lsp_types::Location {
            uri,
            range: crate::diagnostics::span_to_range(&self.span),
        }
    }
}

/// Three-level symbol index for efficient lookup
#[derive(Debug, Default)]
pub struct SymbolIndex {
    /// Index by document URI - for cleanup when documents change
    by_document: HashMap<Arc<str>, Vec<SymbolRef>>,
    /// Index by symbol name - for go-to-definition
    by_name: HashMap<String, Vec<SymbolRef>>,
    /// Index of symbol references (usage sites)
    references: HashMap<String, Vec<SymbolRef>>,
}

impl SymbolIndex {
    /// Create a new empty symbol index
    pub fn new() -> Self {
        Self::default()
    }

    /// Index symbols from a document
    ///
    /// This removes any existing symbols for the document and indexes new ones.
    pub fn index_document(&mut self, uri: Arc<str>, symbols: Vec<Symbol>) {
        // First, remove existing symbols for this document
        self.remove_document(&uri);

        // Convert symbols to refs and index them
        let refs: Vec<SymbolRef> = symbols
            .iter()
            .flat_map(|s| Self::flatten_symbol(s, uri.clone()))
            .collect();

        // Index by document
        self.by_document.insert(uri.clone(), refs.clone());

        // Index by name
        for sym_ref in &refs {
            self.by_name
                .entry(sym_ref.name.clone())
                .or_default()
                .push(sym_ref.clone());
        }
    }

    /// Flatten a symbol and its children into a list of refs
    fn flatten_symbol(symbol: &Symbol, uri: Arc<str>) -> Vec<SymbolRef> {
        let mut refs = vec![SymbolRef::from_symbol(symbol, uri.clone())];

        for child in &symbol.children {
            refs.extend(Self::flatten_symbol(child, uri.clone()));
        }

        refs
    }

    /// Remove all symbols for a document
    pub fn remove_document(&mut self, uri: &str) {
        let uri_arc: Arc<str> = Arc::from(uri);

        if let Some(doc_refs) = self.by_document.remove(&uri_arc) {
            // Remove from by_name index
            for sym_ref in doc_refs {
                if let Some(name_refs) = self.by_name.get_mut(&sym_ref.name) {
                    name_refs.retain(|r| r.uri != uri_arc);
                    if name_refs.is_empty() {
                        self.by_name.remove(&sym_ref.name);
                    }
                }
            }
        }

        // Also clean up references
        self.references.retain(|_, refs| {
            refs.retain(|r| r.uri != uri_arc);
            !refs.is_empty()
        });
    }

    /// Add a reference (usage) for a symbol
    pub fn add_reference(&mut self, name: &str, reference: SymbolRef) {
        self.references
            .entry(name.to_string())
            .or_default()
            .push(reference);
    }

    /// Find symbol definitions by name
    pub fn find_definitions(&self, name: &str) -> Vec<&SymbolRef> {
        self.by_name
            .get(name)
            .map(|refs| refs.iter().collect())
            .unwrap_or_default()
    }

    /// Find all references to a symbol
    pub fn find_references(&self, name: &str) -> Vec<&SymbolRef> {
        self.references
            .get(name)
            .map(|refs| refs.iter().collect())
            .unwrap_or_default()
    }

    /// Get all symbols in a document
    pub fn get_document_symbols(&self, uri: &str) -> Vec<&SymbolRef> {
        let uri_arc: Arc<str> = Arc::from(uri);
        self.by_document
            .get(&uri_arc)
            .map(|refs| refs.iter().collect())
            .unwrap_or_default()
    }

    /// Search for symbols matching a pattern (case-insensitive)
    pub fn search(&self, pattern: &str) -> Vec<&SymbolRef> {
        let pattern_lower = pattern.to_lowercase();
        self.by_name
            .iter()
            .filter(|(name, _)| name.to_lowercase().contains(&pattern_lower))
            .flat_map(|(_, refs)| refs.iter())
            .collect()
    }

    /// Get all indexed symbols
    pub fn all_symbols(&self) -> Vec<&SymbolRef> {
        self.by_name.values().flat_map(|refs| refs.iter()).collect()
    }

    /// Clear the entire index
    pub fn clear(&mut self) {
        self.by_document.clear();
        self.by_name.clear();
        self.references.clear();
    }

    /// Get statistics about the index
    pub fn stats(&self) -> IndexStats {
        IndexStats {
            documents: self.by_document.len(),
            symbols: self.by_name.values().map(|v| v.len()).sum(),
            references: self.references.values().map(|v| v.len()).sum(),
        }
    }
}

/// Statistics about the symbol index
#[derive(Debug, Clone)]
pub struct IndexStats {
    /// Number of indexed documents
    pub documents: usize,
    /// Number of unique symbols
    pub symbols: usize,
    /// Number of references
    pub references: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use zymbol_span::{FileId, Position};

    fn create_symbol(name: &str, kind: SymbolKind) -> Symbol {
        Symbol::new(
            name.to_string(),
            kind,
            Span::new(Position::new(1, 1, 0), Position::new(1, 5, 4), FileId(0)),
        )
    }

    #[test]
    fn test_index_document() {
        let mut index = SymbolIndex::new();
        let uri: Arc<str> = Arc::from("file:///test.zy");

        let symbols = vec![
            create_symbol("x", SymbolKind::Variable),
            create_symbol("y", SymbolKind::Constant),
        ];

        index.index_document(uri.clone(), symbols);

        let stats = index.stats();
        assert_eq!(stats.documents, 1);
        assert_eq!(stats.symbols, 2);
    }

    #[test]
    fn test_find_definitions() {
        let mut index = SymbolIndex::new();
        let uri: Arc<str> = Arc::from("file:///test.zy");

        let symbols = vec![create_symbol("myVar", SymbolKind::Variable)];

        index.index_document(uri, symbols);

        let defs = index.find_definitions("myVar");
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0].name, "myVar");
    }

    #[test]
    fn test_remove_document() {
        let mut index = SymbolIndex::new();
        let uri: Arc<str> = Arc::from("file:///test.zy");

        let symbols = vec![create_symbol("x", SymbolKind::Variable)];
        index.index_document(uri.clone(), symbols);

        assert_eq!(index.stats().symbols, 1);

        index.remove_document("file:///test.zy");

        assert_eq!(index.stats().symbols, 0);
        assert_eq!(index.stats().documents, 0);
    }

    #[test]
    fn test_search() {
        let mut index = SymbolIndex::new();
        let uri: Arc<str> = Arc::from("file:///test.zy");

        let symbols = vec![
            create_symbol("myVariable", SymbolKind::Variable),
            create_symbol("myFunction", SymbolKind::Function),
            create_symbol("other", SymbolKind::Variable),
        ];

        index.index_document(uri, symbols);

        let results = index.search("my");
        assert_eq!(results.len(), 2);

        let results = index.search("func");
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_nested_symbols() {
        let mut index = SymbolIndex::new();
        let uri: Arc<str> = Arc::from("file:///test.zy");

        let mut func = create_symbol("myFunc", SymbolKind::Function);
        func.children.push(create_symbol("param1", SymbolKind::Parameter));
        func.children.push(create_symbol("param2", SymbolKind::Parameter));

        let symbols = vec![func];
        index.index_document(uri, symbols);

        // Should index the function and its parameters
        assert_eq!(index.stats().symbols, 3);

        // Should be able to find parameters by name
        let params = index.find_definitions("param1");
        assert_eq!(params.len(), 1);
    }

    #[test]
    fn test_document_reindex() {
        let mut index = SymbolIndex::new();
        let uri: Arc<str> = Arc::from("file:///test.zy");

        // Initial index
        let symbols1 = vec![create_symbol("x", SymbolKind::Variable)];
        index.index_document(uri.clone(), symbols1);
        assert_eq!(index.stats().symbols, 1);

        // Re-index with different symbols
        let symbols2 = vec![
            create_symbol("a", SymbolKind::Variable),
            create_symbol("b", SymbolKind::Variable),
        ];
        index.index_document(uri, symbols2);

        // Should only have new symbols
        assert_eq!(index.stats().symbols, 2);
        assert!(index.find_definitions("x").is_empty());
        assert_eq!(index.find_definitions("a").len(), 1);
    }

    #[test]
    fn test_add_reference() {
        let mut index = SymbolIndex::new();
        let uri: Arc<str> = Arc::from("file:///test.zy");

        // Add a symbol definition
        let symbols = vec![create_symbol("x", SymbolKind::Variable)];
        index.index_document(uri.clone(), symbols);

        // Add a reference
        let ref_span = Span::new(
            Position::new(5, 10, 50),
            Position::new(5, 11, 51),
            FileId(0),
        );
        let reference = SymbolRef {
            uri: uri.clone(),
            name: "x".to_string(),
            kind: SymbolKind::Variable,
            span: ref_span,
            detail: None,
        };
        index.add_reference("x", reference);

        let refs = index.find_references("x");
        assert_eq!(refs.len(), 1);
    }
}
