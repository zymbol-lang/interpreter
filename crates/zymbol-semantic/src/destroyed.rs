//! Warn about a read of a name that `\ name` has certainly destroyed (G4,
//! decided by the author on 2026-10-23).
//!
//! `zymbol-design/PREMISES.md` MEM-8: using a name after `\` is an error, and it
//! must be a **run-time** error — a `\` inside a branch that never runs destroys
//! nothing, and a static refusal could not tell the two apart. So this is a
//! warning, not a refusal: the program runs exactly as before, the run-time error
//! is still the error, and `!?` still catches it. What it adds is that `check`
//! sees the read that is certain to fail, where it used to say "No errors or
//! warnings".
//!
//! "Certainly" is a must-analysis over the blocks, in order:
//!
//! - `\ x` adds `x`; any assignment to `x` (`=`, `<<`, a destructuring, a loop
//!   variable, a `##Kind(x) =>` binding) takes it out again — a new value lives.
//! - After a `?`, a `??` or a `!? :!`, a name is destroyed only if it is in every
//!   branch that can reach the next statement; a missing `_` branch is the path
//!   that skipped them all.
//! - A loop may not run, so after it a name is destroyed only if it already was.
//!   Inside its body, a name the body assigns anywhere is not certain either: a
//!   later turn reads what the turn before gave back (GLB-147).
//! - After `<~`, `@!` or `@>` the rest of the block cannot run: no warning there.
//! - Each function starts empty: its body runs when it is called, not where it is
//!   written.
//!
//! Reads are identifiers and the `{name}` of an interpolated text. A lambda's
//! body is not followed (it runs later, and its parameters may reuse the name),
//! nor are a `??` expression's arms (an arm can bind the name): no false warning
//! in either, at the price of saying nothing there.
//!
//! zyjs mirrors this pass in `destroyedReads` (web/src/zymbol/zymbol.js), with the
//! same rules, message and positions.

use std::collections::HashSet;
use zymbol_ast::{Block, DestructureItem, DestructurePattern, Expr, Program, Statement};
use zymbol_common::Literal;
use zymbol_error::Diagnostic;
use zymbol_span::Span;

use crate::last_use::walk_sub_exprs;
use crate::unreachable::{fold, Const};

/// Warnings for every read of a name `\` has certainly destroyed.
pub fn check_destroyed_reads(program: &Program) -> Vec<Diagnostic> {
    let mut functions = HashSet::new();
    function_names(&program.statements, &mut functions);
    // A module alias is refused by `\` the same way.
    functions.extend(program.imports.iter().map(|i| i.alias.clone()));
    let mut pass = Pass { out: Vec::new(), functions };
    pass.block(&program.statements, Some(State::default()));
    pass.out
}

/// The names certainly destroyed, each with the line of its `\`.
#[derive(Clone, Default, PartialEq)]
struct State(Vec<(String, u32)>);

impl State {
    fn line_of(&self, name: &str) -> Option<u32> {
        self.0.iter().find(|(n, _)| n == name).map(|(_, l)| *l)
    }
    fn destroy(&mut self, name: &str, line: u32) {
        self.revive(name);
        self.0.push((name.to_string(), line));
    }
    fn revive(&mut self, name: &str) {
        self.0.retain(|(n, _)| n != name);
    }
    /// Destroyed on both paths. `None` is a path that cannot get here.
    fn meet(a: Option<State>, b: Option<State>) -> Option<State> {
        match (a, b) {
            (None, x) | (x, None) => x,
            (Some(a), Some(b)) => Some(State(a.0.into_iter().filter(|(n, _)| b.line_of(n).is_some()).collect())),
        }
    }
}

struct Pass {
    out: Vec<Diagnostic>,
    /// Declared functions and module aliases: `\` on one is refused before
    /// anything runs, and destroys nothing.
    functions: HashSet<String>,
}

fn function_names(stmts: &[Statement], out: &mut HashSet<String>) {
    for s in stmts {
        if let Statement::FunctionDecl(f) = s {
            out.insert(f.name.clone());
            function_names(&f.body.statements, out);
        }
    }
}

impl Pass {
    /// Walk a block from `state`; what is certainly destroyed after it, or `None`
    /// when it always leaves (`<~`, `@!`, `@>`) and nothing follows it.
    fn block(&mut self, stmts: &[Statement], mut state: Option<State>) -> Option<State> {
        for s in stmts {
            let Some(st) = state else {
                // Unreachable: a function declared here still has a body to check.
                self.functions_only(s);
                continue;
            };
            state = self.stmt(s, st);
        }
        state
    }

    fn functions_only(&mut self, s: &Statement) {
        if let Statement::FunctionDecl(f) = s {
            self.block(&f.body.statements, Some(State::default()));
        }
    }

    fn sub(&mut self, b: &Block, state: &State) -> Option<State> {
        self.block(&b.statements, Some(state.clone()))
    }

    fn stmt(&mut self, s: &Statement, mut st: State) -> Option<State> {
        match s {
            Statement::LifetimeEnd(l) => {
                if !self.functions.contains(&l.variable_name) {
                    st.destroy(&l.variable_name, l.span.start.line);
                }
            }
            Statement::Assignment(a) => {
                self.expr(&a.value, &st);
                st.revive(&a.name);
            }
            Statement::ConstDecl(c) => {
                self.expr(&c.value, &st);
                st.revive(&c.name);
            }
            Statement::DestructureAssign(d) => {
                self.expr(&d.value, &st);
                for n in bound(&d.pattern) {
                    st.revive(&n);
                }
            }
            Statement::Input(i) => st.revive(&i.variable),
            Statement::KeyInput(k) => st.revive(&k.variable),
            Statement::CliArgsCapture(c) => st.revive(&c.variable_name),
            Statement::Output(o) => o.exprs.iter().for_each(|e| self.expr(e, &st)),
            Statement::Expr(e) => self.expr(&e.expr, &st),
            Statement::Sleep(sl) => self.expr(&sl.duration, &st),
            Statement::Return(r) => {
                if let Some(v) = &r.value {
                    self.expr(v, &st);
                }
                return None;
            }
            Statement::Break(_) | Statement::Continue(_) => return None,
            Statement::FunctionDecl(f) => {
                self.block(&f.body.statements, Some(State::default()));
            }
            Statement::If(i) => {
                // A branch whose condition folds to #0, and every branch after
                // one that folds to #1, never runs (`unreachable.rs` says so):
                // nothing in it is read, and no path comes out of it.
                let mut conds: Vec<&Expr> = vec![&i.condition];
                conds.extend(i.else_if_branches.iter().map(|b| &*b.condition));
                let mut blocks: Vec<&Block> = vec![&i.then_block];
                blocks.extend(i.else_if_branches.iter().map(|b| &b.block));
                let mut out: Option<State> = None;
                let mut decided = false;
                for (c, b) in conds.iter().zip(blocks.iter()) {
                    self.expr(c, &st);
                    match fold(c) {
                        Some(Const::Bool(false)) => continue,
                        Some(Const::Bool(true)) => {
                            out = State::meet(out, self.sub(b, &st));
                            decided = true;
                            break;
                        }
                        _ => out = State::meet(out, self.sub(b, &st)),
                    }
                }
                if !decided {
                    out = match &i.else_block {
                        Some(eb) => State::meet(out, self.sub(eb, &st)),
                        None => State::meet(out, Some(st.clone())),
                    };
                }
                return out;
            }
            Statement::Loop(l) => {
                if let Some(c) = &l.condition {
                    self.expr(c, &st);
                    if l.iterable.is_none() && fold(c) == Some(Const::Bool(false)) {
                        return Some(st); // never runs
                    }
                }
                if let Some(it) = &l.iterable {
                    self.expr(it, &st);
                }
                // The loop's own variable is assigned too, and outlives the loop
                // (`\ n` then `@ n:1..3 { }` leaves `n` at 3).
                let mut assigned = assigned_in(&l.body.statements);
                if let Some(v) = &l.iterator_var {
                    assigned.insert(v.clone());
                }
                if let Some(p) = &l.iterator_pattern {
                    assigned.extend(bound(p));
                }
                // A turn after the first starts where the last one ended: what
                // the body assigns anywhere in it is alive there, in the scope
                // `\` took it from (GLB-147), and a read of it before the
                // assignment is not certain to fail.
                let mut inner = st.clone();
                inner.0.retain(|(n, _)| !assigned.contains(n));
                self.block(&l.body.statements, Some(inner));
                // It may not have run, and `@!` may leave it half way: only what
                // was destroyed before it, and that nothing in it assigns, is
                // destroyed for certain after it.
                st.0.retain(|(n, _)| !assigned.contains(n));
                return Some(st);
            }
            Statement::Match(m) => {
                self.expr(&m.scrutinee, &st);
                let mut out = Some(st.clone()); // no arm matched: an error, but a path
                for case in &m.cases {
                    let mut arm = st.clone();
                    if let Some(n) = case.pattern.bound_name() {
                        arm.revive(n);
                    }
                    if let Some(v) = &case.value {
                        self.expr(v, &arm);
                    }
                    let end = match &case.block {
                        Some(b) => self.block(&b.statements, Some(arm)),
                        None => Some(arm),
                    };
                    out = State::meet(out, end);
                }
                return out;
            }
            Statement::Try(t) => {
                let after_try = self.sub(&t.try_block, &st);
                // A failure can come at any point of the block: before its `\`,
                // or after an assignment that revived a name.
                let mut failed = st.clone();
                let assigned = assigned_in(&t.try_block.statements);
                failed.0.retain(|(n, _)| !assigned.contains(n));
                let mut out = after_try.clone();
                for c in &t.catch_clauses {
                    out = State::meet(out, self.sub(&c.block, &failed));
                }
                if let Some(f) = &t.finally_clause {
                    let base = State::meet(Some(failed.clone()), after_try).unwrap_or_default();
                    self.sub(&f.block, &base);
                }
                return out;
            }
            Statement::TuiBlock(tb) => return self.sub(&tb.body, &st),
            Statement::OutputPos(op) => {
                for e in op.slots.iter().flatten().chain(op.items.iter()) {
                    self.expr(e, &st);
                }
            }
            Statement::Newline(_) | Statement::SetNumeralMode { .. } | Statement::ClearScreen(_) => {}
        }
        Some(st)
    }

    fn expr(&mut self, e: &Expr, st: &State) {
        if st.0.is_empty() {
            return;
        }
        let mut found: Vec<(String, Span)> = Vec::new();
        reads(e, &mut found);
        for (name, span) in found {
            if let Some(line) = st.line_of(&name) {
                self.out.push(
                    Diagnostic::warning(format!("'{name}' is read after '\\ {name}' destroyed it (line {line})"))
                        .with_span(span)
                        .with_help("this read fails when it runs — move the destruction after it, or remove it"),
                );
            }
        }
    }
}

/// Every name a block can assign, at any depth — functions declared in it aside,
/// whose bodies are not run here.
fn assigned_in(stmts: &[Statement]) -> HashSet<String> {
    let mut out = HashSet::new();
    for s in stmts {
        match s {
            Statement::Assignment(a) => {
                out.insert(a.name.clone());
            }
            Statement::ConstDecl(c) => {
                out.insert(c.name.clone());
            }
            Statement::DestructureAssign(d) => out.extend(bound(&d.pattern)),
            Statement::Input(i) => {
                out.insert(i.variable.clone());
            }
            Statement::KeyInput(k) => {
                out.insert(k.variable.clone());
            }
            Statement::CliArgsCapture(c) => {
                out.insert(c.variable_name.clone());
            }
            Statement::If(i) => {
                out.extend(assigned_in(&i.then_block.statements));
                for b in &i.else_if_branches {
                    out.extend(assigned_in(&b.block.statements));
                }
                if let Some(eb) = &i.else_block {
                    out.extend(assigned_in(&eb.statements));
                }
            }
            Statement::Loop(l) => {
                out.extend(assigned_in(&l.body.statements));
                if let Some(v) = &l.iterator_var {
                    out.insert(v.clone());
                }
                if let Some(p) = &l.iterator_pattern {
                    out.extend(bound(p));
                }
            }
            Statement::Match(m) => {
                for c in &m.cases {
                    if let Some(b) = &c.block {
                        out.extend(assigned_in(&b.statements));
                    }
                }
            }
            Statement::Try(t) => {
                out.extend(assigned_in(&t.try_block.statements));
                for c in &t.catch_clauses {
                    out.extend(assigned_in(&c.block.statements));
                }
                if let Some(f) = &t.finally_clause {
                    out.extend(assigned_in(&f.block.statements));
                }
            }
            Statement::TuiBlock(tb) => out.extend(assigned_in(&tb.body.statements)),
            _ => {}
        }
    }
    out
}

/// The names a destructuring pattern binds.
fn bound(p: &DestructurePattern) -> Vec<String> {
    match p {
        DestructurePattern::Array(items) | DestructurePattern::Positional(items) => items
            .iter()
            .filter_map(|i| match i {
                DestructureItem::Bind(n) | DestructureItem::Rest(n) => Some(n.clone()),
                DestructureItem::Ignore => None,
            })
            .collect(),
        DestructurePattern::NamedTuple(pairs) => pairs.iter().map(|(_, v)| v.clone()).collect(),
    }
}

/// Every name an expression reads where it is written.
fn reads(e: &Expr, out: &mut Vec<(String, Span)>) {
    match e {
        Expr::Identifier(i) => out.push((i.name.clone(), i.span)),
        // Runs later, and its parameters may reuse the name.
        Expr::Lambda(_) => {}
        // Its arms can bind the name; only the value it matches is read here.
        Expr::Match(m) => reads(&m.scrutinee, out),
        Expr::Literal(lit) => {
            if let Literal::InterpolatedString(s) = &lit.value {
                let mut rest = s.as_str();
                while let Some(open) = rest.find('{') {
                    let after = &rest[open + 1..];
                    let Some(close) = after.find('}') else { break };
                    let name = &after[..close];
                    if !name.is_empty() && !name.contains(char::is_whitespace) {
                        out.push((name.to_string(), lit.span));
                    }
                    rest = &after[close + 1..];
                }
            }
        }
        _ => walk_sub_exprs(e, &mut |sub| reads(sub, out)),
    }
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
        check_destroyed_reads(&program).into_iter().map(|d| d.span.map(|s| s.start.line).unwrap_or(0)).collect()
    }

    #[test]
    fn a_certain_read_is_warned_where_it_is() {
        assert_eq!(warned("x = 5\n\\ x\n>> x ¶\n"), vec![3]);
        assert_eq!(warned("f() {\n    y = 1\n    \\ y\n    <~ y\n}\n>> f() ¶\n"), vec![4]);
        assert_eq!(warned("x = 5\n\\ x\nx += 1\n>> x ¶\n"), vec![3]);
        assert_eq!(warned("x = 5\n\\ x\n>> \"v={x}\" ¶\n"), vec![3]);
        assert_eq!(warned("x = 5\nc = #1\n? c { \\ x } _ { \\ x }\n>> x ¶\n"), vec![4]);
        assert_eq!(warned("x = 5\n? #1 { \\ x }\n>> x ¶\n"), vec![3]);
    }

    #[test]
    fn an_uncertain_or_revived_name_is_not() {
        // The `\` in a branch that may not run (MEM-8's own example).
        assert!(warned("x = 5\nc = #0\n? c { \\ x }\n>> x ¶\n").is_empty());
        // A read in a branch that never runs.
        assert!(warned("x = 5\n\\ x\n? #0 { >> x ¶ }\n").is_empty());
        // Assigned again, in the same block, in a loop, through a loop's variable.
        assert!(warned("x = 5\n\\ x\nx = 3\n>> x ¶\n").is_empty());
        assert!(warned("x = 5\n\\ x\nn = 0\n@ n < 2 {\n    n = n + 1\n    x = n\n}\n>> x ¶\n").is_empty());
        assert!(warned("x = 5\n\\ x\n@ x:1..3 { }\n>> x ¶\n").is_empty());
        // Read before the body assigns it: a later turn finds the earlier turn's
        // value, given back to the file (GLB-147).
        assert!(warned("x = 5\n\\ x\n@ i:1..3 {\n    ? i > 1 { >> x ¶ }\n    x = i\n}\n").is_empty());
        // A lambda's parameter, and a `\` on a function (refused, destroys nothing).
        assert!(warned("x = 5\n\\ x\nf = (x) -> x + 1\n>> f(2) ¶\n").is_empty());
        assert!(warned("f() { <~ 1 }\n\\ f\n>> f() ¶\n").is_empty());
        // After `<~` nothing runs.
        assert!(warned("g() {\n    x = 1\n    \\ x\n    <~ 0\n    >> x ¶\n}\n>> g() ¶\n").is_empty());
    }
}
