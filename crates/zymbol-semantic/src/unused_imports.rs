//! Warn about an import whose alias the file never names again.
//!
//! An unused import is the cheapest trace of a loose wire. In 囲碁 the frozen
//! second engine was imported as `思2`, offered on the menu, and never called:
//! choosing v2 played v3, and no tool said a word (GO/HALLAZGOS_ES.md HLZ-015,
//! GO/Depurando_GO.md DG-02).
//!
//! The scan is over tokens, like `check_stdlib_access`, and for the same
//! reason: a re-export (`#> { es::saludo => saludo }`) uses an alias without
//! being an expression, and it is a use. An alias counts as used wherever it
//! is followed by `::` (a function) or `.` (a constant) and is not itself the
//! end of a `.`/`::` chain.
//!
//! The import statement's own tokens are skipped: its path and its alias are
//! tokens too, and neither is a use — `<# ./m => m` names `m` twice without
//! using it once.
//!
//! There is no way to silence it, unlike a variable's `_`: a variable is
//! sometimes written to be discarded, an import never is.

use std::collections::HashSet;
use zymbol_ast::ImportStmt;
use zymbol_error::Diagnostic;
use zymbol_lexer::{Token, TokenKind};

/// A warning for every import whose alias is never used as `alias::` or `alias.`.
pub fn check_unused_imports(tokens: &[Token], imports: &[ImportStmt]) -> Vec<Diagnostic> {
    if imports.is_empty() {
        return Vec::new();
    }

    let mut used: HashSet<&str> = HashSet::new();
    let mut in_import = false;
    let mut after_arrow = false;
    for (i, tok) in tokens.iter().enumerate() {
        // `<# path => alias`: skip from `<#` through the alias.
        match &tok.kind {
            TokenKind::ModuleImport => {
                in_import = true;
                after_arrow = false;
                continue;
            }
            TokenKind::FatArrow if in_import => {
                after_arrow = true;
                continue;
            }
            _ if in_import => {
                if after_arrow {
                    in_import = false;
                }
                continue;
            }
            _ => {}
        }

        let TokenKind::Ident(name) = &tok.kind else {
            continue;
        };
        if i > 0
            && matches!(
                tokens[i - 1].kind,
                TokenKind::Dot | TokenKind::ScopeResolution
            )
        {
            continue;
        }
        if matches!(
            tokens.get(i + 1).map(|t| &t.kind),
            Some(TokenKind::Dot | TokenKind::ScopeResolution)
        ) {
            used.insert(name.as_str());
        }
    }

    imports
        .iter()
        .filter(|import| !used.contains(import.alias.as_str()))
        .map(|import| {
            Diagnostic::warning(format!("import '{}' is never used", import.alias))
                .with_span(import.span)
                .with_help(format!(
                    "remove the import, or call into it as {}::name or {}.NAME",
                    import.alias, import.alias
                ))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use zymbol_lexer::Lexer;
    use zymbol_parser::Parser;
    use zymbol_span::FileId;

    fn unused(src: &str) -> Vec<String> {
        let (tokens, diags) = Lexer::new(src, FileId(0)).tokenize();
        assert!(diags.is_empty(), "lex errors: {diags:?}");
        let program = Parser::new(tokens.clone()).parse().expect("test source must parse");
        check_unused_imports(&tokens, &program.imports)
            .into_iter()
            .map(|d| d.message)
            .collect()
    }

    #[test]
    fn an_alias_never_named_again_warns() {
        let w = unused("<# ./m => a\n<# ./m => b\n>> a::f() ¶\n");
        assert_eq!(w, vec!["import 'b' is never used"]);
    }

    #[test]
    fn a_function_a_constant_and_a_re_export_are_uses() {
        let src = "# capa {\n    <# ./m => a\n    <# ./m => b\n    <# std/math => c\n    #> { a::f => g, h }\n    h() { <~ b::f() + c.PI }\n}\n";
        assert!(unused(src).is_empty(), "{:?}", unused(src));
    }

    /// The path and the alias are tokens too, and neither is a use.
    #[test]
    fn the_import_statement_is_not_a_use() {
        let w = unused("<# ./m => m\n>> 1 ¶\n");
        assert_eq!(w, vec!["import 'm' is never used"]);
    }

    /// `x.a.b` is a chain of fields, not the alias `a`.
    #[test]
    fn a_field_named_like_the_alias_is_not_a_use() {
        let w = unused("<# ./m => a\nx = #(a: #(b: 1))\n>> x.a.b ¶\n");
        assert_eq!(w, vec!["import 'a' is never used"]);
    }
}
