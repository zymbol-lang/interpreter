//! Validate uses of a `std/` module against its export table.
//!
//! `std/` modules are native: nothing on disk describes them, so an alias
//! bound to one used to be a blind spot for every static check. A call to a
//! function that does not exist — `math::inventada(2.0)` — parsed, passed
//! `zymbol check`, and only failed once the program ran.
//!
//! The names come from `zymbol_common::stdlib`, the table the two engines are
//! kept in step with, and both `zymbol check` and the LSP call this function,
//! so the editor and the CLI agree on what counts as an error.
//!
//! The scan is over tokens rather than the AST on purpose: a re-export inside
//! an export block (`t::width => ancho`) is not an expression, and it is worth
//! checking too — a typo there breaks every caller of the layer module.

use std::collections::HashMap;
use zymbol_ast::ImportStmt;
use zymbol_common::stdlib;
use zymbol_error::Diagnostic;
use zymbol_lexer::{Token, TokenKind};

/// Diagnostics for `alias::name` / `alias.name` where `alias` names a `std/`
/// module and `name` is not something that module exports.
pub fn check_stdlib_access(tokens: &[Token], imports: &[ImportStmt]) -> Vec<Diagnostic> {
    let mut aliases: HashMap<&str, &'static stdlib::StdModule> = HashMap::new();
    for import in imports {
        if !import.path.is_stdlib() {
            continue;
        }
        let path = format!("std/{}", import.path.components[1..].join("/"));
        if let Some(module) = stdlib::module(&path) {
            aliases.insert(import.alias.as_str(), module);
        }
    }
    if aliases.is_empty() {
        return Vec::new();
    }

    let mut diagnostics = Vec::new();
    for (i, window) in tokens.windows(3).enumerate() {
        let (alias_tok, sep, name_tok) = (&window[0], &window[1], &window[2]);

        let TokenKind::Ident(alias) = &alias_tok.kind else {
            continue;
        };

        // `resp.json.user` is a chain of named-tuple fields that happens to
        // contain a field named like an import alias — not a module access.
        // Only a name that does not itself come after `.` or `::` can be one.
        if i > 0
            && matches!(
                tokens[i - 1].kind,
                TokenKind::Dot | TokenKind::ScopeResolution
            )
        {
            continue;
        }
        let Some(module) = aliases.get(alias.as_str()) else {
            continue;
        };
        let TokenKind::Ident(name) = &name_tok.kind else {
            continue;
        };

        match sep.kind {
            TokenKind::ScopeResolution => {
                if module.function(name).is_some() {
                    continue;
                }
                let diag = if module.has_constant(name) {
                    Diagnostic::error(format!(
                        "'{}' is a constant of {}, not a function",
                        name, module.path
                    ))
                    .with_span(name_tok.span)
                    .with_help(format!("read it with '.': {}.{}", alias, name))
                } else {
                    let mut diag = Diagnostic::error(format!(
                        "{} does not export a function '{}'",
                        module.path, name
                    ))
                    .with_span(name_tok.span);
                    diag = match closest(name, module.functions.iter().map(|f| f.name)) {
                        Some(suggestion) => diag.with_help(format!(
                            "did you mean '{}::{}'?",
                            alias, suggestion
                        )),
                        None => diag.with_help(format!(
                            "{} exports: {}",
                            module.path,
                            module
                                .functions
                                .iter()
                                .map(|f| f.name)
                                .collect::<Vec<_>>()
                                .join(", ")
                        )),
                    };
                    diag
                };
                diagnostics.push(diag);
            }
            TokenKind::Dot => {
                if module.has_constant(name) {
                    continue;
                }
                // A `.` on something that is not a known constant is only an
                // error when the module has no such name at all; `math.sin`
                // is a wrong-operator mistake worth naming precisely.
                let diag = if module.function(name).is_some() {
                    Diagnostic::error(format!(
                        "'{}' is a function of {}, not a constant",
                        name, module.path
                    ))
                    .with_span(name_tok.span)
                    .with_help(format!("call it with '::': {}::{}(…)", alias, name))
                } else {
                    Diagnostic::error(format!(
                        "{} does not export a constant '{}'",
                        module.path, name
                    ))
                    .with_span(name_tok.span)
                };
                diagnostics.push(diag);
            }
            _ => {}
        }
    }

    diagnostics
}

/// The candidate within edit distance 2 of `name`, closest first.
fn closest<'a>(name: &str, candidates: impl Iterator<Item = &'a str>) -> Option<&'a str> {
    candidates
        .map(|c| (edit_distance(name, c), c))
        .filter(|(d, _)| *d <= 2)
        .min_by_key(|(d, _)| *d)
        .map(|(_, c)| c)
}

/// Levenshtein distance over characters.
fn edit_distance(a: &str, b: &str) -> usize {
    let b_chars: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b_chars.len()).collect();
    let mut current = vec![0usize; b_chars.len() + 1];

    for (i, a_char) in a.chars().enumerate() {
        current[0] = i + 1;
        for (j, b_char) in b_chars.iter().enumerate() {
            let cost = usize::from(a_char != *b_char);
            current[j + 1] = (prev[j] + cost)
                .min(prev[j + 1] + 1)
                .min(current[j] + 1);
        }
        std::mem::swap(&mut prev, &mut current);
    }

    prev[b_chars.len()]
}

#[cfg(test)]
mod tests {
    use super::*;
    use zymbol_lexer::Lexer;
    use zymbol_span::FileId;

    fn diagnose(source: &str) -> Vec<Diagnostic> {
        let (tokens, _) = Lexer::new(source, FileId(0)).tokenize();
        let program = zymbol_parser::Parser::new(tokens.clone())
            .parse()
            .expect("test source must parse");
        check_stdlib_access(&tokens, &program.imports)
    }

    #[test]
    fn accepts_real_stdlib_functions() {
        assert!(diagnose("<# std/math => math\n\n>> math::sin(2.0) ¶\n>> math.PI ¶\n").is_empty());
    }

    #[test]
    fn rejects_unknown_function() {
        let diags = diagnose("<# std/math => math\n\n>> math::inventada(2.0) ¶\n");
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("does not export a function 'inventada'"));
    }

    #[test]
    fn suggests_a_near_miss() {
        let diags = diagnose("<# std/math => m\n\n>> m::sqr(4.0) ¶\n");
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].help.as_deref(), Some("did you mean 'm::sqrt'?"));
    }

    #[test]
    fn names_the_wrong_operator() {
        let calls_a_constant = diagnose("<# std/math => m\n\n>> m::PI() ¶\n");
        assert_eq!(calls_a_constant.len(), 1);
        assert!(calls_a_constant[0].message.contains("is a constant"));

        let reads_a_function = diagnose("<# std/math => m\n\n>> m.sin ¶\n");
        assert_eq!(reads_a_function.len(), 1);
        assert!(reads_a_function[0].message.contains("is a function"));
    }

    #[test]
    fn checks_re_exports_in_an_export_block() {
        let typo = diagnose(
            "# capa {\n    #> {\n        t::widht => ancho\n    }\n\n    <# std/term => t\n}\n",
        );
        assert_eq!(typo.len(), 1);
        assert_eq!(typo[0].help.as_deref(), Some("did you mean 't::width'?"));

        assert!(diagnose(
            "# capa {\n    #> {\n        t::width => ancho\n    }\n\n    <# std/term => t\n}\n"
        )
        .is_empty());
    }

    #[test]
    fn ignores_aliases_that_are_not_stdlib() {
        assert!(diagnose("<# ./local => l\n\n>> l::whatever(1) ¶\n").is_empty());
    }

    /// A named-tuple field can share an import alias's name. `resp.json.user`
    /// is a field chain, and reading `json` there as the module made a working
    /// example (`examples/api_demo/04_post_json.zy`) light up red.
    #[test]
    fn a_field_named_like_an_alias_is_not_a_module_access() {
        assert!(diagnose(concat!(
            "<# std/json => json\n\n",
            "resp = #(json: #(user: \"ada\", score: 42))\n",
            ">> resp.json.user ¶\n",
            ">> resp.json.score ¶\n"
        ))
        .is_empty());
    }

    #[test]
    fn edit_distance_basics() {
        assert_eq!(edit_distance("sqrt", "sqrt"), 0);
        assert_eq!(edit_distance("sqr", "sqrt"), 1);
        assert_eq!(edit_distance("", "abc"), 3);
    }
}
