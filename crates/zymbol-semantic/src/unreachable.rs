//! Warn about code that cannot run (HLZ-016, decided 2026-09-27).
//!
//! Three shapes, each decided without running anything:
//!
//! - **after an exit**: a statement that follows `<~`, `@!` or `@>` in the
//!   same block;
//! - **every branch leaves**: a statement after a `? … _ { }` whose branches
//!   all leave the block;
//! - **constant condition**: a branch whose condition folds to `#0`, the
//!   branches after one that folds to `#1`, and a `@ cond { }` loop whose
//!   condition folds to `#0`. A condition folds when it is made of Bool and
//!   number literals only, with `!`, unary `-`/`+`, `&&`, `||` and the six
//!   comparisons.
//!
//! What it does not do, on purpose: `? #1 { … }` with no other branch is the
//! idiom for a block with a scope of its own (GUIDE § scope, 27 files in the
//! corpus), and nothing in it is dead. A value that depends on anything —
//! a variable, a call, another function's result — is never folded: that is
//! value-flow analysis, not a warning. It would not have caught the colour
//! bug in GO (GO/Depurando_GO.md DG-01), and it does not claim to.
//!
//! A dead block that is empty says nothing: the warning points at the first
//! statement that will not run, and there is none.
//!
//! zyjs mirrors this pass in `unreachableCode` (web/src/zymbol/zymbol.js),
//! with the same shapes, the same folding and the same positions.

use std::collections::HashSet;
use zymbol_ast::{Block, Expr, LambdaBody, Program, Statement};
use zymbol_common::{BinaryOp, Literal, UnaryOp};
use zymbol_error::Diagnostic;

use crate::last_use::walk_sub_exprs;

/// Warnings for every statement the program can never reach.
pub fn check_unreachable(program: &Program) -> Vec<Diagnostic> {
    let mut pass = Pass { out: Vec::new(), seen: HashSet::new() };
    pass.block(&program.statements);
    pass.out
}

struct Pass {
    out: Vec<Diagnostic>,
    /// Blocks already visited, by address (compared, never dereferenced). A
    /// lambda is reachable both through the block that holds its statement and
    /// through that statement's expressions, which `walk_stmt_exprs` collects
    /// from nested blocks as well.
    seen: HashSet<*const Vec<Statement>>,
}

const HELP_AFTER_EXIT: &str = "delete it, or move it above the exit";
const HELP_CONSTANT: &str = "remove the dead code, or make the condition depend on something that can change";

#[derive(Clone, Copy, PartialEq)]
enum Const {
    Bool(bool),
    Num(f64),
}

fn fold(e: &Expr) -> Option<Const> {
    match e {
        Expr::Literal(lit) => match lit.value {
            Literal::Bool(b) => Some(Const::Bool(b)),
            Literal::Int(n) => Some(Const::Num(n as f64)),
            Literal::Float(x) => Some(Const::Num(x)),
            _ => None,
        },
        Expr::Group(g) => fold(&g.expr),
        Expr::Unary(u) => match (u.op, fold(&u.operand)?) {
            (UnaryOp::Not, Const::Bool(b)) => Some(Const::Bool(!b)),
            (UnaryOp::Neg, Const::Num(n)) => Some(Const::Num(-n)),
            (UnaryOp::Pos, Const::Num(n)) => Some(Const::Num(n)),
            _ => None,
        },
        Expr::Binary(b) => {
            let (l, r) = (fold(&b.left)?, fold(&b.right)?);
            match (b.op, l, r) {
                (BinaryOp::And, Const::Bool(x), Const::Bool(y)) => Some(Const::Bool(x && y)),
                (BinaryOp::Or, Const::Bool(x), Const::Bool(y)) => Some(Const::Bool(x || y)),
                (BinaryOp::Eq, Const::Bool(x), Const::Bool(y)) => Some(Const::Bool(x == y)),
                (BinaryOp::Neq, Const::Bool(x), Const::Bool(y)) => Some(Const::Bool(x != y)),
                (BinaryOp::Eq, Const::Num(x), Const::Num(y)) => Some(Const::Bool(x == y)),
                (BinaryOp::Neq, Const::Num(x), Const::Num(y)) => Some(Const::Bool(x != y)),
                (BinaryOp::Lt, Const::Num(x), Const::Num(y)) => Some(Const::Bool(x < y)),
                (BinaryOp::Le, Const::Num(x), Const::Num(y)) => Some(Const::Bool(x <= y)),
                (BinaryOp::Gt, Const::Num(x), Const::Num(y)) => Some(Const::Bool(x > y)),
                (BinaryOp::Ge, Const::Num(x), Const::Num(y)) => Some(Const::Bool(x >= y)),
                _ => None,
            }
        }
        _ => None,
    }
}

/// How a statement leaves its block, when it always does.
fn leaves(stmt: &Statement) -> Option<&'static str> {
    match stmt {
        Statement::Return(_) => Some("'<~'"),
        Statement::Break(_) => Some("'@!'"),
        Statement::Continue(_) => Some("'@>'"),
        Statement::If(i) => {
            let else_block = i.else_block.as_ref()?;
            let all = block_leaves(&i.then_block)
                && i.else_if_branches.iter().all(|b| block_leaves(&b.block))
                && block_leaves(else_block);
            all.then_some("")
        }
        _ => None,
    }
}

fn block_leaves(b: &Block) -> bool {
    b.statements.iter().any(|s| leaves(s).is_some())
}

impl Pass {
    fn warn_at(&mut self, stmt: &Statement, message: String, help: &str) {
        self.out.push(Diagnostic::warning(message).with_span(stmt.span()).with_help(help));
    }

    /// Warn at the first statement of a dead block, if it has one.
    fn dead(&mut self, stmts: &[Statement], message: &str) {
        if let Some(first) = stmts.first() {
            self.warn_at(first, message.to_string(), HELP_CONSTANT);
        }
    }

    fn block(&mut self, stmts: &Vec<Statement>) {
        if !self.seen.insert(stmts as *const _) {
            return;
        }
        if let Some(i) = stmts.iter().position(|s| leaves(s).is_some()) {
            if let Some(next) = stmts.get(i + 1) {
                let message = match leaves(&stmts[i]) {
                    Some("") => "unreachable code: every branch of the '?' above leaves the block".to_string(),
                    Some(exit) => format!(
                        "unreachable code: this statement comes after {exit}, which always leaves the block"
                    ),
                    None => unreachable!(),
                };
                self.warn_at(next, message, HELP_AFTER_EXIT);
            }
        }
        for s in stmts {
            self.stmt(s);
        }
    }

    fn stmt(&mut self, stmt: &Statement) {
        match stmt {
            Statement::If(i) => {
                let mut conds: Vec<&Expr> = vec![&i.condition];
                conds.extend(i.else_if_branches.iter().map(|b| &*b.condition));
                let mut blocks: Vec<&Vec<Statement>> = vec![&i.then_block.statements];
                blocks.extend(i.else_if_branches.iter().map(|b| &b.block.statements));
                if let Some(eb) = &i.else_block {
                    blocks.push(&eb.statements);
                }
                for (k, c) in conds.iter().enumerate() {
                    match fold(c) {
                        Some(Const::Bool(false)) => {
                            self.dead(blocks[k], "this branch never runs: its condition is always #0");
                        }
                        Some(Const::Bool(true)) => {
                            if let Some(later) = blocks[k + 1..].iter().find(|b| !b.is_empty()) {
                                self.dead(later, "this branch never runs: an earlier condition in the chain is always #1");
                            }
                            break;
                        }
                        _ => {}
                    }
                }
                for b in blocks {
                    self.block(b);
                }
            }
            Statement::Loop(l) => {
                if l.iterator_var.is_none() && l.iterable.is_none() {
                    if let Some(c) = &l.condition {
                        if fold(c) == Some(Const::Bool(false)) {
                            self.dead(&l.body.statements, "this loop never runs: its condition is always #0");
                        }
                    }
                }
                self.block(&l.body.statements);
            }
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
            Statement::Match(mx) => {
                for case in &mx.cases {
                    if let Some(b) = &case.block {
                        self.block(&b.statements);
                    }
                }
            }
            Statement::TuiBlock(tb) => self.block(&tb.body.statements),
            _ => {}
        }
        // Blocks that live inside expressions: a lambda's body, a `??` arm.
        crate::last_use::walk_stmt_exprs(stmt, &mut |e| self.expr(e));
    }

    fn expr(&mut self, e: &Expr) {
        match e {
            Expr::Lambda(l) => {
                if let LambdaBody::Block(b) = &l.body {
                    self.block(&b.statements);
                }
            }
            Expr::Match(mx) => {
                for case in &mx.cases {
                    if let Some(b) = &case.block {
                        self.block(&b.statements);
                    }
                }
            }
            _ => {}
        }
        walk_sub_exprs(e, &mut |c| self.expr(c));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zymbol_lexer::Lexer;
    use zymbol_parser::Parser;
    use zymbol_span::FileId;

    fn warnings(src: &str) -> Vec<(u32, String)> {
        let (tokens, diags) = Lexer::new(src, FileId(0)).tokenize();
        assert!(diags.is_empty(), "lex errors: {diags:?}");
        let program = Parser::new(tokens).parse().expect("test source must parse");
        check_unreachable(&program)
            .into_iter()
            .map(|d| (d.span.map(|s| s.start.line as u32).unwrap_or(0), d.message))
            .collect()
    }

    #[test]
    fn a_statement_after_return_is_unreachable() {
        let w = warnings("f(x) {\n    <~ x\n    >> 1 ¶\n}\n>> f(1) ¶\n");
        assert_eq!(w, vec![(3, "unreachable code: this statement comes after '<~', which always leaves the block".into())]);
    }

    #[test]
    fn every_branch_leaving_makes_the_rest_unreachable() {
        let w = warnings("g(x) {\n    ? x > 0 { <~ 1 } _ { <~ 2 }\n    >> 3 ¶\n}\n>> g(1) ¶\n");
        assert_eq!(w, vec![(3, "unreachable code: every branch of the '?' above leaves the block".into())]);
    }

    #[test]
    fn constant_conditions() {
        let w = warnings("? #0 {\n    >> 1 ¶\n}\n? 1 == 2 {\n    >> 2 ¶\n}\n? #1 {\n    >> 3 ¶\n} _ {\n    >> 4 ¶\n}\n@ #0 {\n    >> 5 ¶\n}\n");
        let lines: Vec<u32> = w.iter().map(|(l, _)| *l).collect();
        assert_eq!(lines, vec![2, 5, 10, 13], "{w:?}");
    }

    /// Inside a lambda's block body too — the blind spot this analyser has had
    /// three times (a block arm that does not descend switches every check off).
    #[test]
    fn inside_a_lambda_block() {
        let w = warnings("h = (x -> {\n    <~ x\n    >> 1 ¶\n})\n>> h(1) ¶\n");
        assert_eq!(w.len(), 1, "{w:?}");
    }

    #[test]
    fn the_control_program_says_nothing() {
        let src = "listo() { <~ #1 }\nf(x) {\n    ? x == 0 { <~ 0 }\n    >> \"sigue\" ¶\n    <~ 1\n}\n? #1 {\n    _tmp = 2\n    >> _tmp ¶\n}\n@ #1 {\n    ? listo() { @! }\n}\n>> f(1) ¶\n";
        assert!(warnings(src).is_empty(), "{:?}", warnings(src));
    }
}
