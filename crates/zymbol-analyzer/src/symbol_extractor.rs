//! Symbol extraction from Zymbol AST
//!
//! Walks the AST to extract symbol definitions for indexing,
//! supporting go-to-definition and find-references functionality.

use zymbol_ast::{Block, Expr, Program, Statement, DestructureItem, DestructurePattern};
use zymbol_span::Span;

/// Kind of symbol extracted from the AST
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolKind {
    /// Variable declaration (via =)
    Variable,
    /// Constant declaration (via :=)
    Constant,
    /// Function declaration
    Function,
    /// Function parameter
    Parameter,
    /// Module declaration (via #)
    Module,
    /// Import statement (via <#)
    Import,
    /// Loop iterator variable
    Iterator,
}

impl SymbolKind {
    /// Convert to LSP SymbolKind
    pub fn to_lsp(&self) -> lsp_types::SymbolKind {
        match self {
            SymbolKind::Variable => lsp_types::SymbolKind::VARIABLE,
            SymbolKind::Constant => lsp_types::SymbolKind::CONSTANT,
            SymbolKind::Function => lsp_types::SymbolKind::FUNCTION,
            SymbolKind::Parameter => lsp_types::SymbolKind::VARIABLE,
            SymbolKind::Module => lsp_types::SymbolKind::MODULE,
            SymbolKind::Import => lsp_types::SymbolKind::MODULE,
            SymbolKind::Iterator => lsp_types::SymbolKind::VARIABLE,
        }
    }
}

/// A symbol extracted from the source code
#[derive(Debug, Clone)]
pub struct Symbol {
    /// Symbol name
    pub name: String,
    /// Kind of symbol
    pub kind: SymbolKind,
    /// Location where the symbol is defined
    pub span: Span,
    /// Detail string (e.g., function signature)
    pub detail: Option<String>,
    /// Child symbols (e.g., function parameters, block-local variables)
    pub children: Vec<Symbol>,
}

impl Symbol {
    /// Create a new symbol
    pub fn new(name: String, kind: SymbolKind, span: Span) -> Self {
        Self {
            name,
            kind,
            span,
            detail: None,
            children: Vec::new(),
        }
    }

    /// Add a detail string to the symbol
    pub fn with_detail(mut self, detail: String) -> Self {
        self.detail = Some(detail);
        self
    }

    /// Add a child symbol
    pub fn with_child(mut self, child: Symbol) -> Self {
        self.children.push(child);
        self
    }

    /// Convert to LSP DocumentSymbol
    pub fn to_document_symbol(&self) -> lsp_types::DocumentSymbol {
        let range = crate::diagnostics::span_to_range(&self.span);

        #[allow(deprecated)]
        lsp_types::DocumentSymbol {
            name: self.name.clone(),
            detail: self.detail.clone(),
            kind: self.kind.to_lsp(),
            tags: None,
            deprecated: None,
            range,
            selection_range: range,
            children: if self.children.is_empty() {
                None
            } else {
                Some(
                    self.children
                        .iter()
                        .map(|c| c.to_document_symbol())
                        .collect(),
                )
            },
        }
    }
}

/// Symbol extractor - walks AST and extracts symbol definitions
pub struct SymbolExtractor {
    /// Collected symbols
    symbols: Vec<Symbol>,
}

impl SymbolExtractor {
    /// Create a new symbol extractor
    pub fn new() -> Self {
        Self {
            symbols: Vec::new(),
        }
    }

    /// Extract symbols from a program
    pub fn extract(&mut self, program: &Program) -> Vec<Symbol> {
        self.symbols.clear();

        // Extract module declaration if present
        if let Some(module_decl) = &program.module_decl {
            self.symbols.push(Symbol::new(
                module_decl.name.clone(),
                SymbolKind::Module,
                module_decl.span,
            ));
        }

        // Extract imports
        for import in &program.imports {
            // Use the alias directly (it's required in Zymbol)
            self.symbols.push(Symbol::new(
                import.alias.clone(),
                SymbolKind::Import,
                import.span,
            ));
        }

        // Extract symbols from statements
        for statement in &program.statements {
            self.extract_from_statement(statement);
        }

        std::mem::take(&mut self.symbols)
    }

    /// Extract symbols from a statement
    fn extract_from_statement(&mut self, stmt: &Statement) {
        match stmt {
            Statement::Assignment(assignment) => {
                self.symbols.push(Symbol::new(
                    assignment.name.clone(),
                    SymbolKind::Variable,
                    assignment.span,
                ));
                // Also check for lambda assignments
                self.extract_from_expr(&assignment.value);
            }

            Statement::ConstDecl(const_decl) => {
                self.symbols.push(Symbol::new(
                    const_decl.name.clone(),
                    SymbolKind::Constant,
                    const_decl.span,
                ));
                self.extract_from_expr(&const_decl.value);
            }

            Statement::FunctionDecl(func) => {
                // Build parameter list for detail
                let params: Vec<String> = func
                    .parameters
                    .iter()
                    .map(|p| {
                        match p.kind {
                            zymbol_ast::ParameterKind::Mutable => format!("~{}", p.name),
                            zymbol_ast::ParameterKind::Output => format!("<~{}", p.name),
                            zymbol_ast::ParameterKind::Normal => p.name.clone(),
                        }
                    })
                    .collect();
                let detail = format!("({})", params.join(", "));

                // Create function symbol with parameters as children
                let mut func_symbol = Symbol::new(
                    func.name.clone(),
                    SymbolKind::Function,
                    func.span,
                )
                .with_detail(detail);

                // Add parameters as children
                for param in &func.parameters {
                    func_symbol.children.push(Symbol::new(
                        param.name.clone(),
                        SymbolKind::Parameter,
                        param.span,
                    ));
                }

                // Extract symbols from function body
                let body_symbols = self.extract_from_block(&func.body);
                func_symbol.children.extend(body_symbols);

                self.symbols.push(func_symbol);
            }

            Statement::Loop(loop_stmt) => {
                // Extract iterator variable if present
                if let Some(iterator) = &loop_stmt.iterator_var {
                    self.symbols.push(Symbol::new(
                        iterator.clone(),
                        SymbolKind::Iterator,
                        loop_stmt.span,
                    ));
                }

                // Extract from loop body
                for stmt in &loop_stmt.body.statements {
                    self.extract_from_statement(stmt);
                }
            }

            Statement::If(if_stmt) => {
                // Extract from then block
                for stmt in &if_stmt.then_block.statements {
                    self.extract_from_statement(stmt);
                }

                // Extract from else-if branches
                for else_if in &if_stmt.else_if_branches {
                    for stmt in &else_if.block.statements {
                        self.extract_from_statement(stmt);
                    }
                }

                // Extract from else block
                if let Some(else_block) = &if_stmt.else_block {
                    for stmt in &else_block.statements {
                        self.extract_from_statement(stmt);
                    }
                }
            }

            Statement::Match(match_expr) => {
                for case in &match_expr.cases {
                    if let Some(block) = &case.block {
                        for stmt in &block.statements {
                            self.extract_from_statement(stmt);
                        }
                    }
                }
            }

            Statement::Try(try_stmt) => {
                // Extract from try block
                for stmt in &try_stmt.try_block.statements {
                    self.extract_from_statement(stmt);
                }

                // Extract from catch blocks
                for catch in &try_stmt.catch_clauses {
                    for stmt in &catch.block.statements {
                        self.extract_from_statement(stmt);
                    }
                }

                // Extract from finally block
                if let Some(finally) = &try_stmt.finally_clause {
                    for stmt in &finally.block.statements {
                        self.extract_from_statement(stmt);
                    }
                }
            }

            Statement::Expr(expr_stmt) => {
                self.extract_from_expr(&expr_stmt.expr);
            }

            Statement::Input(input) => {
                // Input creates a variable
                self.symbols.push(Symbol::new(
                    input.variable.clone(),
                    SymbolKind::Variable,
                    input.span,
                ));
            }

            Statement::CliArgsCapture(capture) => {
                self.symbols.push(Symbol::new(
                    capture.variable_name.clone(),
                    SymbolKind::Variable,
                    capture.span,
                ));
            }

            Statement::DestructureAssign(d) => {
                // Register bound variable names as symbols
                match &d.pattern {
                    DestructurePattern::Array(items) | DestructurePattern::Positional(items) => {
                        for item in items {
                            if let DestructureItem::Bind(name) | DestructureItem::Rest(name) = item {
                                self.symbols.push(Symbol::new(
                                    name.clone(),
                                    SymbolKind::Variable,
                                    d.span,
                                ));
                            }
                        }
                    }
                    DestructurePattern::NamedTuple(fields) => {
                        for (_field, var) in fields {
                            self.symbols.push(Symbol::new(
                                var.clone(),
                                SymbolKind::Variable,
                                d.span,
                            ));
                        }
                    }
                }
            }

            // Statements without symbol definitions
            Statement::Output(_)
            | Statement::Return(_)
            | Statement::Break(_)
            | Statement::Continue(_)
            | Statement::Newline(_)
            | Statement::LifetimeEnd(_) => {}
        }
    }

    /// Extract symbols from a block, returning child symbols
    fn extract_from_block(&mut self, block: &Block) -> Vec<Symbol> {
        let mut children = Vec::new();

        for stmt in &block.statements {
            match stmt {
                Statement::Assignment(assignment) => {
                    children.push(Symbol::new(
                        assignment.name.clone(),
                        SymbolKind::Variable,
                        assignment.span,
                    ));
                }
                Statement::ConstDecl(const_decl) => {
                    children.push(Symbol::new(
                        const_decl.name.clone(),
                        SymbolKind::Constant,
                        const_decl.span,
                    ));
                }
                _ => {
                    // Recursively extract from nested statements
                    // but don't add to children (only top-level in block)
                }
            }
        }

        children
    }

    /// Extract symbols from an expression (for lambda assignments)
    fn extract_from_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Lambda(lambda) => {
                // Lambda parameters could be tracked separately if needed
                match &lambda.body {
                    zymbol_ast::LambdaBody::Block(block) => {
                        for stmt in &block.statements {
                            self.extract_from_statement(stmt);
                        }
                    }
                    zymbol_ast::LambdaBody::Expr(inner_expr) => {
                        self.extract_from_expr(inner_expr);
                    }
                }
            }

            Expr::Match(match_expr) => {
                for case in &match_expr.cases {
                    if let Some(block) = &case.block {
                        for stmt in &block.statements {
                            self.extract_from_statement(stmt);
                        }
                    }
                }
            }

            // Other expressions don't define symbols
            _ => {}
        }
    }
}

impl Default for SymbolExtractor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zymbol_lexer::Lexer;
    use zymbol_parser::Parser;
    use zymbol_span::FileId;

    fn parse_program(source: &str) -> Option<Program> {
        let lexer = Lexer::new(source, FileId(0));
        let (tokens, _) = lexer.tokenize();
        let parser = Parser::new(tokens);
        parser.parse().ok()
    }

    fn extract_symbols(source: &str) -> Vec<Symbol> {
        let program = parse_program(source).expect("failed to parse");
        let mut extractor = SymbolExtractor::new();
        extractor.extract(&program)
    }

    #[test]
    fn test_extract_variable() {
        let symbols = extract_symbols("x = 5");
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "x");
        assert_eq!(symbols[0].kind, SymbolKind::Variable);
    }

    #[test]
    fn test_extract_constant() {
        let symbols = extract_symbols("PI := 3.14159");
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "PI");
        assert_eq!(symbols[0].kind, SymbolKind::Constant);
    }

    #[test]
    fn test_extract_function() {
        let symbols = extract_symbols("add(a, b) { <~ a + b }");
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "add");
        assert_eq!(symbols[0].kind, SymbolKind::Function);
        assert!(symbols[0].detail.as_ref().unwrap().contains("a, b"));

        // Should have parameter children
        assert_eq!(symbols[0].children.len(), 2);
        assert_eq!(symbols[0].children[0].name, "a");
        assert_eq!(symbols[0].children[0].kind, SymbolKind::Parameter);
    }

    #[test]
    fn test_extract_multiple_symbols() {
        let symbols = extract_symbols("x = 1\ny = 2\nz = 3");
        assert_eq!(symbols.len(), 3);
    }

    #[test]
    fn test_to_document_symbol() {
        let symbol = Symbol::new(
            "test".to_string(),
            SymbolKind::Function,
            zymbol_span::Span::new(
                zymbol_span::Position::new(1, 1, 0),
                zymbol_span::Position::new(1, 5, 4),
                FileId(0),
            ),
        );

        let doc_symbol = symbol.to_document_symbol();
        assert_eq!(doc_symbol.name, "test");
        assert_eq!(doc_symbol.kind, lsp_types::SymbolKind::FUNCTION);
    }
}
