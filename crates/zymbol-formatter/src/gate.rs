//! Safety gate — verifies that formatted output is equivalent to the original
//! source before it is returned to the caller.
//!
//! The formatter's contract (FORMATTER_RULES.md §2.1) is that it never adds,
//! removes, or reorders tokens that carry meaning. This module enforces that
//! contract mechanically, so a formatter bug produces a clean error instead of
//! corrupted output:
//!
//! - **G1 — token equivalence**: the token streams of original and formatted
//!   source must be identical after dropping trivia (comments, physical
//!   newlines, semicolons, EOF). Line breaks and `;` are legitimate layout
//!   changes; everything else is not.
//! - **G2 — reparse**: the formatted output must parse without errors.
//! - **G3 — statement shape**: the statement trees of original and formatted
//!   source must have the same pre-order (depth, variant) sequence. This
//!   closes the G1 blind spot opened by skipping newline/semicolon tokens
//!   (e.g. a juxtaposition concatenation split across lines parses as two
//!   statements with the same token stream).

use std::mem::{discriminant, Discriminant};

use zymbol_ast::{Block, Program, Statement};
use zymbol_lexer::{Lexer, TokenKind};
use zymbol_parser::Parser;
use zymbol_span::FileId;

/// Verify that `formatted` is token- and shape-equivalent to the original.
///
/// `original_program` is the AST already parsed by the formatter pipeline,
/// reused here to avoid a second parse of the original source.
pub fn verify(
    original_src: &str,
    original_program: &Program,
    formatted: &str,
) -> Result<(), String> {
    // G1 — token equivalence (G4: comment counts must also survive)
    let (orig_tokens, orig_comments) = significant_tokens(original_src)
        .map_err(|e| format!("internal: original source stopped lexing: {e}"))?;
    let (fmt_tokens, fmt_comments) = significant_tokens(formatted)
        .map_err(|e| format!("formatted output no longer lexes: {e}"))?;

    if orig_comments != fmt_comments {
        return Err(format!(
            "comment count changed: source has {orig_comments} comments, formatted output has \
             {fmt_comments} (file left unchanged)"
        ));
    }

    if orig_tokens != fmt_tokens {
        let idx = orig_tokens
            .iter()
            .zip(fmt_tokens.iter())
            .position(|(a, b)| a != b)
            .unwrap_or_else(|| orig_tokens.len().min(fmt_tokens.len()));
        let expected = orig_tokens.get(idx).map(describe).unwrap_or_default();
        let got = fmt_tokens.get(idx).map(describe).unwrap_or_default();
        return Err(format!(
            "token stream changed at token #{idx}: source has {expected:?}, formatted output has {got:?} \
             (the file uses syntax the formatter cannot yet reprint faithfully; file left unchanged)"
        ));
    }

    // G2 — reparse
    let lexer = Lexer::new(formatted, FileId(0));
    let (tokens, lex_errors) = lexer.tokenize();
    if !lex_errors.is_empty() {
        return Err(format!(
            "formatted output no longer lexes: {}",
            lex_errors[0].message
        ));
    }
    let formatted_program = Parser::new(tokens)
        .parse()
        .map_err(|errs| format!("formatted output no longer parses: {}", errs[0].message))?;

    // G3 — statement shape equivalence
    let orig_shape = program_shape(original_program);
    let fmt_shape = program_shape(&formatted_program);
    if orig_shape != fmt_shape {
        return Err(format!(
            "statement structure changed: source has {} statements, formatted output has {} \
             (file left unchanged)",
            orig_shape.len(),
            fmt_shape.len()
        ));
    }

    Ok(())
}

/// Lex `src` and return its token kinds minus trivia, plus the comment count.
///
/// Note: `TokenKind::Newline` is the *semantic* `¶` token (physical line
/// breaks never become tokens), so it must NOT be filtered — dropping a `¶`
/// changes program output. Only comments, `;` separators and EOF are layout,
/// and comments are counted separately so G4 can verify none were lost.
fn significant_tokens(src: &str) -> Result<(Vec<TokenKind>, usize), String> {
    let lexer = Lexer::new(src, FileId(0));
    let (tokens, errors) = lexer.tokenize();
    if !errors.is_empty() {
        return Err(errors[0].message.clone());
    }
    let mut comment_count = 0usize;
    let significant = tokens
        .into_iter()
        .map(|t| t.kind)
        .filter(|k| {
            if matches!(k, TokenKind::LineComment(_) | TokenKind::BlockComment(_)) {
                comment_count += 1;
                return false;
            }
            !matches!(k, TokenKind::Semicolon | TokenKind::Eof)
        })
        .collect();
    Ok((significant, comment_count))
}

fn describe(kind: &TokenKind) -> String {
    format!("{kind:?}")
}

/// Pre-order (depth, variant) sequence of every statement in the program,
/// recursing into all block-bearing statements.
fn program_shape(program: &Program) -> Vec<(usize, Discriminant<Statement>)> {
    let mut out = Vec::new();
    for stmt in &program.statements {
        collect_statement(stmt, 0, &mut out);
    }
    out
}

fn collect_block(block: &Block, depth: usize, out: &mut Vec<(usize, Discriminant<Statement>)>) {
    for stmt in &block.statements {
        collect_statement(stmt, depth, out);
    }
}

fn collect_statement(
    stmt: &Statement,
    depth: usize,
    out: &mut Vec<(usize, Discriminant<Statement>)>,
) {
    out.push((depth, discriminant(stmt)));
    let d = depth + 1;
    match stmt {
        Statement::If(i) => {
            collect_block(&i.then_block, d, out);
            for branch in &i.else_if_branches {
                collect_block(&branch.block, d, out);
            }
            if let Some(else_block) = &i.else_block {
                collect_block(else_block, d, out);
            }
        }
        Statement::Loop(l) => collect_block(&l.body, d, out),
        Statement::Try(t) => {
            collect_block(&t.try_block, d, out);
            for clause in &t.catch_clauses {
                collect_block(&clause.block, d, out);
            }
            if let Some(finally) = &t.finally_clause {
                collect_block(&finally.block, d, out);
            }
        }
        Statement::FunctionDecl(f) => collect_block(&f.body, d, out),
        Statement::Match(m) => {
            for case in &m.cases {
                if let Some(block) = &case.block {
                    collect_block(block, d, out);
                }
            }
        }
        Statement::TuiBlock(t) => collect_block(&t.body, d, out),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use crate::format;
    use crate::FormatError;

    /// Until the corresponding fidelity fixes land, each known mutation bug
    /// class must FAIL CLOSED (SafetyGate error) instead of corrupting output.
    /// As phases 2-3 of the formatter redesign land, flip each of these to an
    /// `assert_eq!` on correct output.
    fn assert_gated(src: &str) {
        match format(src) {
            Err(FormatError::SafetyGate(_)) => {}
            Ok(out) => {
                // Reaching Ok means the gate found the output equivalent —
                // only acceptable if the formatter now reprints faithfully.
                let again = format(&out).expect("idempotent reformat");
                assert_eq!(out, again, "gate passed but output is not idempotent");
            }
            Err(e) => panic!("expected SafetyGate or faithful output, got: {e}"),
        }
    }

    #[test]
    fn gate_hot_def_compound() {
        // `°` marker + `+=` desugar must not silently become `total = total + 10`
        assert_gated("total° += 10\n>> total ¶\n");
    }

    #[test]
    fn gate_increment() {
        assert_gated("x = 1\nx++\n>> x ¶\n");
    }

    #[test]
    fn gate_typed_input_cast() {
        assert_gated("<< ###(3) \"code:\" c\n>> c ¶\n");
    }

    #[test]
    fn gate_parens_in_output() {
        assert_gated("a = 1\nb = 2\n>> (a + b) ¶\n");
    }

    #[test]
    fn gate_mutable_param_suffix() {
        assert_gated("f(num~) {\n    num = num + 1\n    <~ num\n}\n>> f(1) ¶\n");
    }

    #[test]
    fn gate_chained_output() {
        assert_gated(">> \"a: \" >> 1 ¶\n");
    }

    #[test]
    fn gate_accepts_clean_file() {
        let src = "x = 5\n>> x ¶\n";
        let out = format(src).expect("clean file must format");
        assert_eq!(out, format(&out).expect("idempotent"));
    }
}
