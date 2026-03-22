//! Module system AST nodes for Zymbol-Lang
//!
//! Contains AST structures for the module system:
//! - Module declaration: # module_name (with optional dot prefix for folders)
//! - Export blocks: #> { items } (public API definition)
//! - Import statements: <# path <= alias (import with required alias)
//! - Module paths: ./relative, ../parent, absolute paths

use zymbol_span::Span;

/// Module declaration: # module_name [#> { exports }]
#[derive(Debug, Clone)]
pub struct ModuleDecl {
    pub name: String,
    pub export_block: Option<ExportBlock>,
    pub span: Span,
}

/// Export block: #> { items }
#[derive(Debug, Clone)]
pub struct ExportBlock {
    pub items: Vec<ExportItem>,
    pub span: Span,
}

/// Items that can be exported
#[derive(Debug, Clone)]
pub enum ExportItem {
    /// Own item: identifier
    Own {
        name: String,
        span: Span,
    },
    /// Re-export: alias::function or alias.CONSTANT
    ReExport {
        module_alias: String,
        item_name: String,
        item_type: ItemType,
        rename: Option<String>,
        span: Span,
    },
}

/// Type of item being re-exported
#[derive(Debug, Clone, PartialEq)]
pub enum ItemType {
    /// Function (uses ::)
    Function,
    /// Constant (uses .)
    Constant,
}

/// Import statement: <# path <= alias
#[derive(Debug, Clone)]
pub struct ImportStmt {
    pub path: ModulePath,
    pub alias: String,
    pub span: Span,
}

/// Module path: ./dir/module, ../module, etc.
#[derive(Debug, Clone)]
pub struct ModulePath {
    pub components: Vec<String>,
    pub is_relative: bool,
    pub parent_levels: usize, // 0 for ./, 1 for ../, 2 for ../../
    pub span: Span,
}

impl ModuleDecl {
    pub fn new(name: String, export_block: Option<ExportBlock>, span: Span) -> Self {
        Self {
            name,
            export_block,
            span,
        }
    }
}

impl ExportBlock {
    pub fn new(items: Vec<ExportItem>, span: Span) -> Self {
        Self { items, span }
    }
}

impl ExportItem {
    pub fn own(name: String, span: Span) -> Self {
        ExportItem::Own { name, span }
    }

    pub fn re_export(
        module_alias: String,
        item_name: String,
        item_type: ItemType,
        rename: Option<String>,
        span: Span,
    ) -> Self {
        ExportItem::ReExport {
            module_alias,
            item_name,
            item_type,
            rename,
            span,
        }
    }
}

impl ImportStmt {
    pub fn new(path: ModulePath, alias: String, span: Span) -> Self {
        Self { path, alias, span }
    }
}

impl ModulePath {
    pub fn new(
        components: Vec<String>,
        is_relative: bool,
        parent_levels: usize,
        span: Span,
    ) -> Self {
        Self {
            components,
            is_relative,
            parent_levels,
            span,
        }
    }
}
