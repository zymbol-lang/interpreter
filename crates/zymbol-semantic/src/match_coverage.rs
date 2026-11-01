//! Warn about a `??` that may find no arm (G5, decided by the author on
//! 2026-10-24).
//!
//! A value no arm matches is a run-time error — `no pattern matched in match
//! expression`, of kind `##_`, which `!?` catches — and that stays the error:
//! GUIDE § 7 documents a `??` without `_` as a statement that only those values
//! can arrive. What this adds is that `check` says so before anything runs,
//! because the mistake it points at is the usual one in generated code: cases
//! enumerated, and one forgotten.
//!
//! A `??` is complete, and nothing is said, when:
//!
//! - an arm is `_`, alone or as one alternative of `||`;
//! - its Bool literals include both `#1` and `#0`;
//! - two comparisons against constants leave no number out: `< a` (or `<= a`)
//!   with `> b` (or `>= b`) where `b < a`, or `b == a` and one of the two
//!   includes it; and `== k` with `<> k`.
//!
//! Anything else without `_` is warned, a name used as a pattern included: what
//! it holds is known only when it runs.
//!
//! zyjs mirrors this pass in `matchCoverage` (web/src/zymbol/zymbol.js), with
//! the same rules, message and positions.

use std::collections::HashSet;
use zymbol_ast::{Expr, LambdaBody, MatchExpr, Pattern, Program, Statement};
use zymbol_common::{BinaryOp, Literal};
use zymbol_error::Diagnostic;

use crate::last_use::walk_sub_exprs;
use crate::unreachable::{fold, Const};

const MESSAGE: &str = "this '??' has no '_' arm: a value no arm matches stops the program";
const HELP: &str = "add `_ => …` for every other value — or, if no other value can arrive, catch the error where it matters with `!?`";

/// Warnings for every `??` that may find no arm.
pub fn check_match_coverage(program: &Program) -> Vec<Diagnostic> {
    let mut pass = Pass { out: Vec::new(), seen: HashSet::new() };
    pass.block(&program.statements);
    pass.out.sort_by_key(|d| d.span.map(|s| (s.start.line, s.start.column)));
    pass.out
}

struct Pass {
    out: Vec<Diagnostic>,
    /// `??`s already judged, by address (compared, never dereferenced): one in a
    /// nested block is reached both through that block and through the
    /// expressions `walk_stmt_exprs` collects from it.
    seen: HashSet<*const MatchExpr>,
}

impl Pass {
    fn block(&mut self, stmts: &[Statement]) {
        for s in stmts {
            self.stmt(s);
        }
    }

    fn stmt(&mut self, stmt: &Statement) {
        match stmt {
            Statement::Match(mx) => {
                self.judge(mx);
                for case in &mx.cases {
                    if let Some(b) = &case.block {
                        self.block(&b.statements);
                    }
                }
            }
            Statement::If(i) => {
                self.block(&i.then_block.statements);
                for b in &i.else_if_branches {
                    self.block(&b.block.statements);
                }
                if let Some(eb) = &i.else_block {
                    self.block(&eb.statements);
                }
            }
            Statement::Loop(l) => self.block(&l.body.statements),
            Statement::Try(t) => {
                self.block(&t.try_block.statements);
                for c in &t.catch_clauses {
                    self.block(&c.block.statements);
                }
                if let Some(f) = &t.finally_clause {
                    self.block(&f.block.statements);
                }
            }
            Statement::FunctionDecl(f) => self.block(&f.body.statements),
            Statement::TuiBlock(tb) => self.block(&tb.body.statements),
            _ => {}
        }
        // `??` written as a value, and the blocks that live inside expressions.
        crate::last_use::walk_stmt_exprs(stmt, &mut |e| self.expr(e));
    }

    fn expr(&mut self, e: &Expr) {
        match e {
            Expr::Match(mx) => {
                self.judge(mx);
                for case in &mx.cases {
                    if let Some(b) = &case.block {
                        self.block(&b.statements);
                    }
                }
            }
            Expr::Lambda(l) => {
                if let LambdaBody::Block(b) = &l.body {
                    self.block(&b.statements);
                }
            }
            _ => {}
        }
        walk_sub_exprs(e, &mut |c| self.expr(c));
    }

    fn judge(&mut self, mx: &MatchExpr) {
        if !self.seen.insert(mx as *const _) {
            return;
        }
        if !complete(mx) {
            self.out.push(Diagnostic::warning(MESSAGE).with_span(mx.span).with_help(HELP));
        }
    }
}

/// Whether some arm matches whatever arrives — as far as that can be decided
/// without running anything.
fn complete(mx: &MatchExpr) -> bool {
    let mut alts: Vec<&Pattern> = Vec::new();
    for case in &mx.cases {
        match &case.pattern {
            Pattern::Or(ps, _) => alts.extend(ps.iter()),
            p => alts.push(p),
        }
    }
    if alts.iter().any(|p| matches!(p, Pattern::Wildcard(_))) {
        return true;
    }
    let bools: HashSet<bool> = alts
        .iter()
        .filter_map(|p| match p {
            Pattern::Literal(Literal::Bool(b), _) => Some(*b),
            _ => None,
        })
        .collect();
    if bools.len() == 2 {
        return true;
    }
    let cmps: Vec<(BinaryOp, f64)> = alts
        .iter()
        .filter_map(|p| match p {
            Pattern::Comparison(op, x, _) => match fold(x) {
                Some(Const::Num(k)) => Some((op.clone(), k)),
                _ => None,
            },
            _ => None,
        })
        .collect();
    cmps.iter().any(|(lo, a)| {
        cmps.iter().any(|(hi, b)| match (lo, hi) {
            (BinaryOp::Lt | BinaryOp::Le, BinaryOp::Gt | BinaryOp::Ge) => {
                b < a || (b == a && (matches!(lo, BinaryOp::Le) || matches!(hi, BinaryOp::Ge)))
            }
            (BinaryOp::Eq, BinaryOp::Neq) => a == b,
            _ => false,
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use zymbol_lexer::Lexer;
    use zymbol_parser::Parser;
    use zymbol_span::FileId;

    /// The lines warned about, in order.
    fn warned(src: &str) -> Vec<u32> {
        let (tokens, diags) = Lexer::new(src, FileId(0)).tokenize();
        assert!(diags.is_empty(), "lex errors: {diags:?}");
        let program = Parser::new(tokens).parse().expect("test source must parse");
        check_match_coverage(&program).into_iter().map(|d| d.span.map(|s| s.start.line).unwrap_or(0)).collect()
    }

    #[test]
    fn a_match_that_may_find_no_arm_is_warned() {
        assert_eq!(warned("n = 7\nr = ?? n {\n    1 => \"a\"\n    2 => \"b\"\n}\n>> r ¶\n"), vec![2]);
        assert_eq!(warned("n = 7\n?? n {\n    1 => { >> 1 ¶ }\n}\n"), vec![2]);
        assert_eq!(warned("b = #1\n>> ?? b { #1 => 1 } ¶\n"), vec![2]);
        assert_eq!(warned("k = 1\n>> ?? k { < 0 => 1  > 0 => 2 } ¶\n"), vec![2]);
        assert_eq!(warned("d = [1]\n>> ?? 1 { d => 1 } ¶\n"), vec![2]);
        // Inside a function, a lambda, a `??` arm and a loop.
        assert_eq!(warned("f(x) {\n    <~ ?? x { 1 => 1 }\n}\n>> f(1) ¶\n"), vec![2]);
        assert_eq!(warned("g = x -> {\n    <~ ?? x { 1 => 1 }\n}\n>> g(1) ¶\n"), vec![2]);
        assert_eq!(warned("n = 1\n?? n {\n    _ => {\n        >> ?? n { 1 => 1 } ¶\n    }\n}\n"), vec![4]);
        assert_eq!(warned("@ i:1..2 {\n    >> ?? i { 1 => 1 } ¶\n}\n"), vec![2]);
    }

    #[test]
    fn a_complete_match_is_not() {
        assert!(warned("n = 7\n>> ?? n { 1 => 1  _ => 2 } ¶\n").is_empty());
        assert!(warned("n = 7\n>> ?? n { 1 || _ => 1 } ¶\n").is_empty());
        assert!(warned("b = #1\n>> ?? b { #1 => 1  #0 => 2 } ¶\n").is_empty());
        assert!(warned("b = #1\n>> ?? b { #1 || #0 => 1 } ¶\n").is_empty());
        assert!(warned("k = 1\n>> ?? k { < 0 => 1  >= 0 => 2 } ¶\n").is_empty());
        assert!(warned("k = 1\n>> ?? k { <= 0 => 1  > 0 => 2 } ¶\n").is_empty());
        assert!(warned("k = 1\n>> ?? k { < 5 => 1  > -5 => 2 } ¶\n").is_empty());
        assert!(warned("k = 1\n>> ?? k { == 3 => 1  <> 3 => 2 } ¶\n").is_empty());
    }
}
