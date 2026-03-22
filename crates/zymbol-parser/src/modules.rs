//! Module system parsing for Zymbol-Lang
//!
//! Handles parsing of module declarations, imports, and exports:
//! - Module declaration: # module_name (with optional dot prefix for folders)
//! - Export blocks: #> { items } (public API definition)
//! - Import statements: <# path <= alias (import with required alias)
//! - Module paths: ./relative, ../parent, absolute paths

use zymbol_ast::{ExportBlock, ExportItem, ImportStmt, ItemType, ModuleDecl, ModulePath};
use zymbol_error::Diagnostic;
use zymbol_lexer::TokenKind;

use crate::Parser;

impl Parser {
    /// Parse module declaration: # [.]module_name [#> { exports }]
    /// Supports dot prefix for folder indication: # .folder_file
    pub(crate) fn parse_module_declaration(&mut self) -> Result<ModuleDecl, Diagnostic> {
        let start_token = self.peek().clone();

        // Consume #
        if !matches!(start_token.kind, TokenKind::Hash) {
            return Err(Diagnostic::error("expected '#' for module declaration")
                .with_span(start_token.span));
        }
        self.advance(); // consume #

        // Parse module name (supports optional leading dot for folder indication)
        // Syntax: # .folder_file or # file
        let mut name = String::new();

        // Check for optional leading dot (indicates file is in a folder)
        if matches!(self.peek().kind, TokenKind::Dot) {
            name.push('.');
            self.advance(); // consume dot
        }

        // Parse identifier after optional dot
        match &self.peek().kind {
            TokenKind::Ident(ident) => {
                name.push_str(ident);
                self.advance();
            }
            _ => {
                return Err(Diagnostic::error("expected module name after '#'")
                    .with_span(self.peek().span))
            }
        }

        // Consume optional semicolon
        if matches!(self.peek().kind, TokenKind::Semicolon) {
            self.advance();
        }

        // Track end span for final span calculation
        let mut end_span = self.peek().span;

        // Parse optional export block
        let export_block = if matches!(self.peek().kind, TokenKind::ExportBlock) {
            let block = self.parse_export_block()?;
            end_span = block.span;
            Some(block)
        } else {
            None
        };

        let span = start_token.span.to(&end_span);
        Ok(ModuleDecl::new(name, export_block, span))
    }

    /// Parse export block: #> { items }
    pub(crate) fn parse_export_block(&mut self) -> Result<ExportBlock, Diagnostic> {
        let start_token = self.peek().clone();

        // Consume #>
        if !matches!(start_token.kind, TokenKind::ExportBlock) {
            return Err(Diagnostic::error("expected '#>' for export block")
                .with_span(start_token.span));
        }
        self.advance(); // consume #>

        // Consume {
        let lbrace_token = self.peek().clone();
        if !matches!(lbrace_token.kind, TokenKind::LBrace) {
            return Err(Diagnostic::error("expected '{' after '#>'")
                .with_span(lbrace_token.span));
        }
        self.advance(); // consume {

        // Parse export items
        let mut items = Vec::new();
        while !matches!(self.peek().kind, TokenKind::RBrace) && !self.is_at_end() {
            items.push(self.parse_export_item()?);

            // Consume optional comma or semicolon
            if matches!(self.peek().kind, TokenKind::Comma | TokenKind::Semicolon) {
                self.advance();
            }
        }

        // Consume }
        let end_token = self.peek().clone();
        if !matches!(end_token.kind, TokenKind::RBrace) {
            return Err(Diagnostic::error("expected '}' to close export block")
                .with_span(end_token.span));
        }
        self.advance(); // consume }

        let span = start_token.span.to(&end_token.span);
        Ok(ExportBlock::new(items, span))
    }

    /// Parse export item: own_item, alias::func, alias.CONST, or renamed
    /// Supports three forms:
    /// - Own item: identifier (exports own function/constant)
    /// - Re-export function: alias::function [<= new_name]
    /// - Re-export constant: alias.CONSTANT [<= NEW_NAME]
    pub(crate) fn parse_export_item(&mut self) -> Result<ExportItem, Diagnostic> {
        let start = self.peek().span;

        // Parse first identifier
        let first_token = self.peek().clone();
        let first_name = match &first_token.kind {
            TokenKind::Ident(name) => {
                let name = name.clone();
                self.advance();
                name
            }
            _ => {
                return Err(Diagnostic::error("expected identifier in export item")
                    .with_span(first_token.span))
            }
        };

        // Track end span
        let mut end_span = first_token.span;

        // Check for :: (function re-export) or . (constant re-export)
        match &self.peek().kind {
            TokenKind::ScopeResolution => {
                // Re-export function: alias::function [<= new_name]
                self.advance(); // consume ::

                // Parse function name
                let func_token = self.peek().clone();
                let func_name = match &func_token.kind {
                    TokenKind::Ident(name) => {
                        let name = name.clone();
                        self.advance();
                        end_span = func_token.span;
                        name
                    }
                    _ => {
                        return Err(Diagnostic::error("expected function name after '::'")
                            .with_span(func_token.span))
                    }
                };

                // Check for rename (<= new_name)
                let rename = if matches!(self.peek().kind, TokenKind::Le) {
                    self.advance(); // consume <=
                    let rename_token = self.peek().clone();
                    match &rename_token.kind {
                        TokenKind::Ident(name) => {
                            let name = name.clone();
                            self.advance();
                            end_span = rename_token.span;
                            Some(name)
                        }
                        _ => {
                            return Err(Diagnostic::error("expected new name after '<='")
                                .with_span(rename_token.span))
                        }
                    }
                } else {
                    None
                };

                let span = start.to(&end_span);
                Ok(ExportItem::re_export(
                    first_name,
                    func_name,
                    ItemType::Function,
                    rename,
                    span,
                ))
            }
            TokenKind::Dot => {
                // Re-export constant: alias.CONSTANT [<= NEW_NAME]
                self.advance(); // consume .

                // Parse constant name
                let const_token = self.peek().clone();
                let const_name = match &const_token.kind {
                    TokenKind::Ident(name) => {
                        let name = name.clone();
                        self.advance();
                        end_span = const_token.span;
                        name
                    }
                    _ => {
                        return Err(Diagnostic::error("expected constant name after '.'")
                            .with_span(const_token.span))
                    }
                };

                // Check for rename (<= NEW_NAME)
                let rename = if matches!(self.peek().kind, TokenKind::Le) {
                    self.advance(); // consume <=
                    let rename_token = self.peek().clone();
                    match &rename_token.kind {
                        TokenKind::Ident(name) => {
                            let name = name.clone();
                            self.advance();
                            end_span = rename_token.span;
                            Some(name)
                        }
                        _ => {
                            return Err(Diagnostic::error("expected new name after '<='")
                                .with_span(rename_token.span))
                        }
                    }
                } else {
                    None
                };

                let span = start.to(&end_span);
                Ok(ExportItem::re_export(
                    first_name,
                    const_name,
                    ItemType::Constant,
                    rename,
                    span,
                ))
            }
            _ => {
                // Own item: just identifier
                let span = start.to(&end_span);
                Ok(ExportItem::own(first_name, span))
            }
        }
    }

    /// Parse import statement: <# path <= alias
    /// Path can be relative (./file, ../file) or absolute
    /// Alias is REQUIRED (not optional)
    pub(crate) fn parse_import_statement(&mut self) -> Result<ImportStmt, Diagnostic> {
        let start_token = self.peek().clone();

        // Consume <#
        if !matches!(start_token.kind, TokenKind::ModuleImport) {
            return Err(Diagnostic::error("expected '<#' for import statement")
                .with_span(start_token.span));
        }
        self.advance(); // consume <#

        // Parse module path
        let path = self.parse_module_path()?;

        // Consume <=
        let le_token = self.peek().clone();
        if !matches!(le_token.kind, TokenKind::Le) {
            return Err(Diagnostic::error("expected '<=' for module alias")
                .with_span(le_token.span));
        }
        self.advance(); // consume <=

        // Parse alias
        let alias_token = self.peek().clone();
        let alias = match &alias_token.kind {
            TokenKind::Ident(name) => {
                let name = name.clone();
                self.advance();
                name
            }
            _ => {
                return Err(Diagnostic::error("expected alias name after '<='")
                    .with_span(alias_token.span))
            }
        };

        let mut end_span = alias_token.span;

        // Consume optional semicolon
        if matches!(self.peek().kind, TokenKind::Semicolon) {
            end_span = self.peek().span;
            self.advance();
        }

        let span = start_token.span.to(&end_span);
        Ok(ImportStmt::new(path, alias, span))
    }

    /// Parse module path: ./dir/module, ../module, etc.
    /// Supports:
    /// - Relative current: ./file or ./dir/file
    /// - Relative parent: ../file or ../../dir/file
    /// - Absolute: file (treated as current directory)
    pub(crate) fn parse_module_path(&mut self) -> Result<ModulePath, Diagnostic> {
        let start = self.peek().span;
        let mut components = Vec::new();
        let mut is_relative = false;
        let mut parent_levels = 0;
        let mut end_span = start;

        // Check for relative path markers (./ or ../)
        if matches!(self.peek().kind, TokenKind::Dot) {
            is_relative = true;
            self.advance(); // consume .

            // Check for /
            if matches!(self.peek().kind, TokenKind::Slash) {
                self.advance(); // consume /
                parent_levels = 0; // ./
            } else {
                return Err(Diagnostic::error("expected '/' after '.'")
                    .with_span(self.peek().span));
            }
        } else if matches!(self.peek().kind, TokenKind::DotDot) {
            // Handle ../ parent directory path
            is_relative = true;
            parent_levels = 1;
            self.advance(); // consume ..

            // Expect /
            let slash_token = self.peek().clone();
            if !matches!(slash_token.kind, TokenKind::Slash) {
                return Err(Diagnostic::error("expected '/' after '..'")
                    .with_span(slash_token.span));
            }
            self.advance(); // consume /

            // Count additional ../ levels
            while matches!(self.peek().kind, TokenKind::DotDot) {
                self.advance(); // consume ..
                if matches!(self.peek().kind, TokenKind::Slash) {
                    self.advance(); // consume /
                    parent_levels += 1;
                } else {
                    return Err(Diagnostic::error("expected '/' after '..'")
                        .with_span(self.peek().span));
                }
            }
        }

        // Parse path components (identifiers separated by /)
        loop {
            let token = self.peek().clone();
            match &token.kind {
                TokenKind::Ident(name) => {
                    components.push(name.clone());
                    end_span = token.span;
                    self.advance();

                    // Check for / to continue path
                    if matches!(self.peek().kind, TokenKind::Slash) {
                        self.advance(); // consume /
                        // Continue to next component
                    } else {
                        // End of path
                        break;
                    }
                }
                _ => {
                    if components.is_empty() {
                        return Err(Diagnostic::error("expected module path")
                            .with_span(token.span));
                    }
                    break;
                }
            }
        }

        let span = start.to(&end_span);
        Ok(ModulePath::new(components, is_relative, parent_levels, span))
    }
}

#[cfg(test)]
mod tests {
    use zymbol_ast::{Expr, ExportItem, ItemType, Program, Statement};
    use zymbol_error::Diagnostic;
    use zymbol_lexer::Lexer;
    use zymbol_span::FileId;

    fn parse(source: &str) -> Result<Program, Vec<Diagnostic>> {
        let lexer = Lexer::new(source, FileId(0));
        let (tokens, lex_diagnostics) = lexer.tokenize();

        if !lex_diagnostics.is_empty() {
            return Err(lex_diagnostics);
        }

        let parser = crate::Parser::new(tokens);
        parser.parse()
    }

    #[test]
    fn test_parse_module_declaration() {
        let program = parse("# math_utils").expect("should parse");
        assert!(program.module_decl.is_some());
        let module = program.module_decl.unwrap();
        assert_eq!(module.name, "math_utils");
        assert!(module.export_block.is_none());
    }

    #[test]
    fn test_parse_module_with_export_block() {
        let program = parse("# math_utils\n#> { add, subtract, PI }").expect("should parse");
        assert!(program.module_decl.is_some());
        let module = program.module_decl.unwrap();
        assert_eq!(module.name, "math_utils");
        assert!(module.export_block.is_some());

        let export_block = module.export_block.unwrap();
        assert_eq!(export_block.items.len(), 3);
    }

    #[test]
    fn test_parse_import_statement() {
        let program = parse("<# ./lib/math_utils <= math").expect("should parse");
        assert_eq!(program.imports.len(), 1);

        let import = &program.imports[0];
        assert_eq!(import.alias, "math");
        assert!(import.path.is_relative);
        assert_eq!(import.path.parent_levels, 0);
        assert_eq!(import.path.components.len(), 2);
        assert_eq!(import.path.components[0], "lib");
        assert_eq!(import.path.components[1], "math_utils");
    }

    #[test]
    fn test_parse_import_parent_directory() {
        let program = parse("<# ../utils/config <= cfg").expect("should parse");
        assert_eq!(program.imports.len(), 1);

        let import = &program.imports[0];
        assert_eq!(import.alias, "cfg");
        assert!(import.path.is_relative);
        assert_eq!(import.path.parent_levels, 1);
        assert_eq!(import.path.components.len(), 2);
        assert_eq!(import.path.components[0], "utils");
        assert_eq!(import.path.components[1], "config");
    }

    #[test]
    fn test_parse_module_function_call() {
        let program = parse("result = math::add(5, 10)").expect("should parse");
        match &program.statements[0] {
            Statement::Assignment(assign) => match &assign.value {
                Expr::FunctionCall(call) => {
                    // Callable should be a MemberAccessExpr (math.add)
                    match call.callable.as_ref() {
                        Expr::MemberAccess(member) => {
                            match member.object.as_ref() {
                                Expr::Identifier(ident) => {
                                    assert_eq!(ident.name, "math");
                                }
                                _ => panic!("Expected identifier for module"),
                            }
                            assert_eq!(member.field, "add");
                        }
                        _ => panic!("Expected member access for module call"),
                    }
                    assert_eq!(call.arguments.len(), 2);
                }
                _ => panic!("Expected function call"),
            },
            _ => panic!("Expected assignment"),
        }
    }

    #[test]
    fn test_parse_export_own_item() {
        let program = parse("# test\n#> { my_function }").expect("should parse");
        let module = program.module_decl.unwrap();
        let export_block = module.export_block.unwrap();

        assert_eq!(export_block.items.len(), 1);
        match &export_block.items[0] {
            ExportItem::Own { name, .. } => {
                assert_eq!(name, "my_function");
            }
            _ => panic!("Expected own export item"),
        }
    }

    #[test]
    fn test_parse_export_reexport_function() {
        let program = parse("# facade\n#> { math::add }").expect("should parse");
        let module = program.module_decl.unwrap();
        let export_block = module.export_block.unwrap();

        assert_eq!(export_block.items.len(), 1);
        match &export_block.items[0] {
            ExportItem::ReExport { module_alias, item_name, item_type, rename, .. } => {
                assert_eq!(module_alias, "math");
                assert_eq!(item_name, "add");
                assert_eq!(item_type, &ItemType::Function);
                assert!(rename.is_none());
            }
            _ => panic!("Expected re-export item"),
        }
    }

    #[test]
    fn test_parse_export_reexport_constant() {
        let program = parse("# facade\n#> { math.PI }").expect("should parse");
        let module = program.module_decl.unwrap();
        let export_block = module.export_block.unwrap();

        assert_eq!(export_block.items.len(), 1);
        match &export_block.items[0] {
            ExportItem::ReExport { module_alias, item_name, item_type, rename, .. } => {
                assert_eq!(module_alias, "math");
                assert_eq!(item_name, "PI");
                assert_eq!(item_type, &ItemType::Constant);
                assert!(rename.is_none());
            }
            _ => panic!("Expected re-export item"),
        }
    }

    #[test]
    fn test_parse_export_reexport_renamed() {
        let program = parse("# facade\n#> { math::subtract <= minus }").expect("should parse");
        let module = program.module_decl.unwrap();
        let export_block = module.export_block.unwrap();

        assert_eq!(export_block.items.len(), 1);
        match &export_block.items[0] {
            ExportItem::ReExport { module_alias, item_name, item_type, rename, .. } => {
                assert_eq!(module_alias, "math");
                assert_eq!(item_name, "subtract");
                assert_eq!(item_type, &ItemType::Function);
                assert_eq!(rename.as_ref().unwrap(), "minus");
            }
            _ => panic!("Expected re-export item"),
        }
    }

    #[test]
    fn test_parse_export_mixed_items() {
        let program = parse("# core\n#> { math::add, own_func, math.PI, text::trim <= strip }").expect("should parse");
        let module = program.module_decl.unwrap();
        let export_block = module.export_block.unwrap();

        assert_eq!(export_block.items.len(), 4);

        // First: re-export function
        match &export_block.items[0] {
            ExportItem::ReExport { item_type, .. } => assert_eq!(item_type, &ItemType::Function),
            _ => panic!("Expected re-export"),
        }

        // Second: own item
        match &export_block.items[1] {
            ExportItem::Own { name, .. } => assert_eq!(name, "own_func"),
            _ => panic!("Expected own item"),
        }

        // Third: re-export constant
        match &export_block.items[2] {
            ExportItem::ReExport { item_type, .. } => assert_eq!(item_type, &ItemType::Constant),
            _ => panic!("Expected re-export"),
        }

        // Fourth: re-export renamed
        match &export_block.items[3] {
            ExportItem::ReExport { rename, .. } => {
                assert_eq!(rename.as_ref().unwrap(), "strip");
            }
            _ => panic!("Expected re-export"),
        }
    }

    #[test]
    fn test_parse_complete_module_example() {
        let source = r#"
# app
<# ./lib/math_utils <= math
<# ./lib/text_utils <= text

result = math::add(5, 10)
name = text::uppercase("alice")
"#;

        let program = parse(source).expect("should parse");

        // Check module declaration
        assert!(program.module_decl.is_some());
        assert_eq!(program.module_decl.unwrap().name, "app");

        // Check imports
        assert_eq!(program.imports.len(), 2);
        assert_eq!(program.imports[0].alias, "math");
        assert_eq!(program.imports[1].alias, "text");

        // Check statements
        assert_eq!(program.statements.len(), 2);
    }
}
