//! Loop-context analysis: `@!`, `@>` and their labelled forms.
//!
//! Two rules, both decidable statically because a label is lexical on the
//! declaring side (`@:outer { }`) and on the referring side (`@:outer!`) alike:
//!
//! 1. `@!` / `@>` require an enclosing `@` loop.
//! 2. `@:L!` / `@:L>` require an enclosing `@` loop labelled `L`. Declared
//!    somewhere in the file is not enough — a sibling loop's label is not in
//!    scope.
//!
//! A function or lambda body is a **boundary**: the loops of the caller are not
//! in scope inside a callee, so `f() { @! }` is an error even when every call
//! site is inside a loop. This matches how the register VM already compiles
//! (`break outside loop`) and how function scope works everywhere else in the
//! language — a function called by name sees only its own parameters.
//!
//! # Why this is a semantic error rather than a runtime one
//!
//! Before v0.0.9 nothing checked either rule and the four engines gave four
//! different answers to the same program. `@:nope!` inside `@:outer`:
//!
//! | engine | v0.0.8 behaviour |
//! |--------|------------------|
//! | tree-walker | unwound *every* enclosing loop, then carried on. Silent. |
//! | register VM | `VM compile error: unsupported construct: break label 'nope' not found` |
//! | `zymbol.js` | unwound every loop and terminated the program. Silent. |
//! | zyml | runtime error at the statement |
//!
//! and `zymbol check` reported nothing at all. Making it a semantic error puts
//! the answer in one place, before execution, so a branch that never runs is
//! still checked — which a runtime error can never do. `cfg.rs` has resolved
//! labels this way since it was written; its `build_break` even carries the
//! comment "should be caught by semantic analysis". This is that analysis.
//!
//! Note what is deliberately *not* here: `@~` (sleep). It pauses execution but
//! does not act on the loop's control flow, so it carries no loop requirement —
//! every engine has always accepted it at top level, and the documentation that
//! claimed otherwise was corrected rather than the code.

use std::collections::HashSet;

use zymbol_ast::{Block, Expr, LambdaBody, Program, Statement};
use zymbol_error::Diagnostic;
use zymbol_span::Span;

// `last_use.rs` owns the exhaustive one-level walkers; this module borrows them
// rather than growing a second copy of fifty match arms that would drift apart.
use crate::last_use::{walk_stmt_exprs, walk_sub_exprs};

/// Check every `@!` / `@>` in the program against its loop context.
///
/// Returns fatal diagnostics only; a program with an empty result is free of
/// loop-context errors.
pub fn check_loop_context(program: &Program) -> Vec<Diagnostic> {
    let mut checker = LoopContext::default();
    checker.stmts(&program.statements);
    checker.errors
}

#[derive(Default)]
struct LoopContext {
    /// One entry per enclosing loop, innermost last. `None` is an unlabelled
    /// loop — it still satisfies a bare `@!`, but no labelled one.
    labels: Vec<Option<String>>,
    errors: Vec<Diagnostic>,
}

impl LoopContext {
    fn stmts(&mut self, stmts: &[Statement]) {
        for stmt in stmts {
            self.stmt(stmt);
        }
    }

    fn block(&mut self, block: &Block) {
        self.stmts(&block.statements);
    }

    /// Analyze `body` with an empty loop stack, then restore. Used for every
    /// construct that a `@!` cannot escape from: function and lambda bodies.
    fn across_boundary(&mut self, body: &Block) {
        let saved = std::mem::take(&mut self.labels);
        self.block(body);
        self.labels = saved;
    }

    fn stmt(&mut self, stmt: &Statement) {
        match stmt {
            Statement::Loop(l) => {
                self.labels.push(l.label.clone());
                self.block(&l.body);
                self.labels.pop();
                // A loop's own header can hold a lambda: `@ x:arr$> (y -> ...)`
                if let Some(c) = &l.condition {
                    self.expr(c);
                }
                if let Some(it) = &l.iterable {
                    self.expr(it);
                }
            }

            Statement::Break(b) => self.control(b.label.as_deref(), b.span, "@!", "break"),
            Statement::Continue(c) => self.control(c.label.as_deref(), c.span, "@>", "continue"),

            Statement::FunctionDecl(f) => self.across_boundary(&f.body),

            // Blocks that a `@!` *does* escape from: the loop stack carries through.
            Statement::If(i) => {
                self.expr(&i.condition);
                self.block(&i.then_block);
                for branch in &i.else_if_branches {
                    self.expr(&branch.condition);
                    self.block(&branch.block);
                }
                if let Some(b) = &i.else_block {
                    self.block(b);
                }
            }
            Statement::Try(t) => {
                self.block(&t.try_block);
                for c in &t.catch_clauses {
                    self.block(&c.block);
                }
                if let Some(f) = &t.finally_clause {
                    self.block(&f.block);
                }
            }
            Statement::Match(m) => {
                self.expr(&m.scrutinee);
                for case in &m.cases {
                    if let Some(v) = &case.value {
                        self.expr(v);
                    }
                    if let Some(b) = &case.block {
                        self.block(b);
                    }
                }
            }
            Statement::TuiBlock(t) => self.block(&t.body),

            // Everything else can still hide a lambda in an expression position.
            // None of these statements carries a block of its own, so the
            // walker's own block recursion never fires here.
            other => walk_stmt_exprs(other, &mut |e| self.expr(e)),
        }
    }

    /// A `@!` or `@>`, with or without a label.
    fn control(&mut self, label: Option<&str>, span: Span, sym: &str, word: &str) {
        match label {
            None => {
                if self.labels.is_empty() {
                    self.errors.push(
                        Diagnostic::error(format!("'{sym}' outside a loop"))
                            .with_span(span)
                            .with_help(format!(
                                "'{sym}' {word}s the enclosing '@' loop; there is none here. \
                                 A function or lambda body does not see the caller's loops."
                            )),
                    );
                }
            }
            Some(name) => {
                let found = self
                    .labels
                    .iter()
                    .any(|l| l.as_deref() == Some(name));
                if !found {
                    self.errors.push(
                        Diagnostic::error(format!(
                            "no enclosing loop is labelled '{name}'"
                        ))
                        .with_span(span)
                        .with_help(self.in_scope_help(name, sym)),
                    );
                }
            }
        }
    }

    /// The help line for an unresolved label: say what *is* reachable, because
    /// the usual cause is a typo or a label on a sibling loop rather than an
    /// enclosing one.
    fn in_scope_help(&self, wanted: &str, sym: &str) -> String {
        let mut seen = HashSet::new();
        let in_scope: Vec<&str> = self
            .labels
            .iter()
            .filter_map(|l| l.as_deref())
            .filter(|l| seen.insert(*l))
            .collect();

        if in_scope.is_empty() {
            if self.labels.is_empty() {
                format!(
                    "'{sym}' is inside no loop at all. A function or lambda body does not \
                     see the caller's loops."
                )
            } else {
                format!(
                    "the enclosing loops have no labels — write '@:{wanted} {{ }}' on the one \
                     you mean to target"
                )
            }
        } else {
            format!(
                "labels in scope here: {}",
                in_scope
                    .iter()
                    .map(|l| format!("'{l}'"))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }
    }

    /// Descend into an expression looking for lambda bodies. Nothing else in an
    /// expression can hold a statement.
    fn expr(&mut self, e: &Expr) {
        if let Expr::Lambda(lambda) = e {
            match &lambda.body {
                LambdaBody::Block(b) => self.across_boundary(b),
                LambdaBody::Expr(inner) => self.expr(inner),
            }
            return;
        }
        walk_sub_exprs(e, &mut |sub| self.expr(sub));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zymbol_lexer::Lexer;
    use zymbol_parser::Parser;
    use zymbol_span::FileId;

    fn errors_for(src: &str) -> Vec<String> {
        let lexer = Lexer::new(src, FileId(0));
        let (tokens, diags) = lexer.tokenize();
        assert!(diags.is_empty(), "lex errors: {diags:?}");
        let program = Parser::new(tokens).parse().expect("test source must parse");
        check_loop_context(&program)
            .iter()
            .map(|d| d.message.clone())
            .collect()
    }

    #[test]
    fn bare_break_inside_loop_is_fine() {
        assert!(errors_for("@ i:1..3 { @! }").is_empty());
    }

    #[test]
    fn bare_break_outside_loop_is_an_error() {
        let e = errors_for("@!");
        assert_eq!(e.len(), 1);
        assert!(e[0].contains("outside a loop"), "{e:?}");
    }

    #[test]
    fn bare_continue_outside_loop_is_an_error() {
        let e = errors_for("@>");
        assert_eq!(e.len(), 1);
        assert!(e[0].contains("outside a loop"), "{e:?}");
    }

    #[test]
    fn labelled_break_resolves_to_an_enclosing_loop() {
        assert!(errors_for("@:outer i:1..3 { @ j:1..3 { @:outer! } }").is_empty());
    }

    #[test]
    fn unknown_label_is_an_error() {
        let e = errors_for("@:outer i:1..3 { @:nope! }");
        assert_eq!(e.len(), 1);
        assert!(e[0].contains("labelled 'nope'"), "{e:?}");
    }

    #[test]
    fn sibling_label_is_not_in_scope() {
        let e = errors_for("@:first i:1..2 { }\n@:second j:1..2 { @:first! }");
        assert_eq!(e.len(), 1);
        assert!(e[0].contains("labelled 'first'"), "{e:?}");
    }

    #[test]
    fn a_function_body_does_not_see_the_callers_loops() {
        let e = errors_for("f() { @! }\n@ i:1..3 { f() }");
        assert_eq!(e.len(), 1);
        assert!(e[0].contains("outside a loop"), "{e:?}");
    }

    #[test]
    fn a_lambda_body_does_not_see_the_callers_loops() {
        let e = errors_for("@ i:1..3 { g = (x) -> { @! } }");
        assert_eq!(e.len(), 1);
        assert!(e[0].contains("outside a loop"), "{e:?}");
    }

    #[test]
    fn a_loop_inside_a_function_is_its_own_context() {
        assert!(errors_for("f() { @:in i:1..3 { @:in! } }").is_empty());
    }

    #[test]
    fn break_escapes_if_try_and_match_blocks() {
        assert!(errors_for("@ i:1..3 { ? i > 1 { @! } }").is_empty());
        assert!(errors_for("@ i:1..3 { !? { @! } :! { @! } }").is_empty());
        assert!(errors_for("@ i:1..3 { ?? i { 1 => { @! } _ => { @> } } }").is_empty());
    }

    #[test]
    fn sleep_carries_no_loop_requirement() {
        assert!(errors_for("@~ 10").is_empty());
    }

    #[test]
    fn unlabelled_enclosing_loop_does_not_satisfy_a_labelled_break() {
        let e = errors_for("@ i:1..3 { @:outer! }");
        assert_eq!(e.len(), 1);
        assert!(e[0].contains("labelled 'outer'"), "{e:?}");
    }
}
