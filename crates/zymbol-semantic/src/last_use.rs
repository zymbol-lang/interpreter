//! Last-use analysis for automatic destruction (auto-free) — v0.0.8.
//!
//! Computes, for one straight-line *region* (the top-level program or a named
//! function body), the point after which each variable can be destroyed: the
//! last region-level statement that mentions it anywhere in its subtree.
//!
//! ## Soundness model
//!
//! Auto-free must be **invisible**: a correct program behaves identically with
//! and without it — it only releases memory earlier. The analysis is purely
//! lexical and conservative:
//!
//! - A region is a flat statement sequence executed once, top to bottom. All
//!   repetition (`@`) and branching (`?`, `??`, `!?`) live *inside* single
//!   region-level statements, so destroying after the last-mentioning
//!   statement is temporally after every possible use inside it.
//! - Mentions are collected from the entire statement subtree — including
//!   nested blocks, lambda bodies (capture-by-value happens at the statement
//!   that contains the lambda literal), `{var}` string interpolations (the
//!   full brace content, verbatim, matching the runtime resolver), and input
//!   prompts. A mention in a branch that never runs still counts: it only
//!   delays destruction (safe direction).
//! - Only region-level *creations* (plus function parameters, for function
//!   bodies) are candidates. Variables created inside nested blocks already
//!   die at block end via scoping.
//! - Anything dubious poisons the name and it is never auto-freed: hot
//!   definitions (`x°`/`°x`), constants, `_`-prefixed names, free variables
//!   of named functions that are used as first-class values (their bodies are
//!   captured by snapshot at the point of use), and — for module programs —
//!   every module-level binding (they participate in the module state
//!   write-back protocol).
//!
//! The `Expr` walker is exhaustive on purpose (no `_` arm): adding a new
//! expression variant must fail compilation here so the mention rules are
//! reviewed.

use std::collections::{HashMap, HashSet};
use zymbol_ast::{
    Block, DestructureItem, DestructurePattern, Expr, InputPrompt, Pattern, Program, Statement,
};
use zymbol_common::Literal;
use zymbol_lexer::StringPart;

// ─────────────────────────────────────────────────────────────────────────────
// Mention collection
// ─────────────────────────────────────────────────────────────────────────────

/// Accumulates mention data while walking one region.
#[derive(Default)]
struct Mentions {
    /// name → last region-level statement index that mentions it
    last: HashMap<String, usize>,
    /// names that must never be auto-freed (hot mentions found in this walk)
    poisoned: HashSet<String>,
    /// current region-level statement index
    current: usize,
}

impl Mentions {
    fn mention(&mut self, name: &str) {
        self.last.insert(name.to_string(), self.current);
    }
    fn poison(&mut self, name: &str) {
        self.poisoned.insert(name.to_string());
        self.mention(name);
    }
}

/// Record every `{content}` segment of an interpolated string, verbatim —
/// exactly what the runtime resolver looks up (escaped braces were turned
/// into sentinels by the lexer, so every raw `{` here is real interpolation).
fn scan_interpolated(s: &str, m: &mut Mentions) {
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '{' {
            let start = i + 1;
            let mut end = start;
            while end < chars.len() && chars[end] != '}' {
                end += 1;
            }
            if end < chars.len() {
                let name: String = chars[start..end].iter().collect();
                m.mention(&name);
                i = end + 1;
                continue;
            }
        }
        i += 1;
    }
}

fn scan_literal(lit: &Literal, m: &mut Mentions) {
    if let Literal::InterpolatedString(s) = lit {
        scan_interpolated(s, m);
    }
}

fn scan_pattern(p: &Pattern, m: &mut Mentions) {
    match p {
        Pattern::Literal(lit, _) => scan_literal(lit, m),
        Pattern::Range(start, end, _) => {
            scan_expr(start, m);
            scan_expr(end, m);
        }
        Pattern::List(items, _) => {
            for item in items {
                scan_pattern(item, m);
            }
        }
        Pattern::Wildcard(_) => {}
        Pattern::Comparison(_, expr, _) => scan_expr(expr, m),
        // Ident patterns compare against the named variable at runtime — a read.
        Pattern::Ident(name, _) => m.mention(name),
        Pattern::Or(alternatives, _) => {
            for alt in alternatives {
                scan_pattern(alt, m);
            }
        }
    }
}

fn scan_block(block: &Block, m: &mut Mentions) {
    for stmt in &block.statements {
        scan_stmt(stmt, m);
    }
}

fn scan_stmt(stmt: &Statement, m: &mut Mentions) {
    match stmt {
        Statement::Output(o) => {
            for e in &o.exprs {
                scan_expr(e, m);
            }
        }
        Statement::Assignment(a) => {
            if a.hot || a.pre_hot {
                m.poison(&a.name);
            } else {
                m.mention(&a.name);
            }
            scan_expr(&a.value, m);
        }
        Statement::ConstDecl(c) => {
            // Constants are never auto-freed.
            m.poison(&c.name);
            scan_expr(&c.value, m);
        }
        Statement::DestructureAssign(d) => {
            match &d.pattern {
                DestructurePattern::Array(items) | DestructurePattern::Positional(items) => {
                    for item in items {
                        match item {
                            DestructureItem::Bind(n) | DestructureItem::Rest(n) => m.mention(n),
                            DestructureItem::Ignore => {}
                        }
                    }
                }
                DestructurePattern::NamedTuple(pairs) => {
                    for (_field, var) in pairs {
                        m.mention(var);
                    }
                }
            }
            scan_expr(&d.value, m);
        }
        Statement::LifetimeEnd(l) => m.mention(&l.variable_name),
        Statement::Input(inp) => {
            m.mention(&inp.variable);
            if let Some(InputPrompt::Interpolated(parts)) = &inp.prompt {
                for part in parts {
                    if let StringPart::Variable(name) = part {
                        m.mention(name);
                    }
                }
            }
        }
        Statement::If(if_stmt) => {
            scan_expr(&if_stmt.condition, m);
            scan_block(&if_stmt.then_block, m);
            for branch in &if_stmt.else_if_branches {
                scan_expr(&branch.condition, m);
                scan_block(&branch.block, m);
            }
            if let Some(else_block) = &if_stmt.else_block {
                scan_block(else_block, m);
            }
        }
        Statement::Loop(lp) => {
            // The iterator write may reuse an outer variable of the same name.
            if let Some(iter_var) = &lp.iterator_var {
                m.mention(iter_var);
            }
            if let Some(cond) = &lp.condition {
                scan_expr(cond, m);
            }
            if let Some(iterable) = &lp.iterable {
                scan_expr(iterable, m);
            }
            scan_block(&lp.body, m);
        }
        Statement::Break(_) | Statement::Continue(_) | Statement::Newline(_) => {}
        Statement::Try(t) => {
            scan_block(&t.try_block, m);
            for clause in &t.catch_clauses {
                scan_block(&clause.block, m);
            }
            if let Some(fin) = &t.finally_clause {
                scan_block(&fin.block, m);
            }
        }
        // A named function body is its own region: mentions inside it are NOT
        // uses in the enclosing region. Direct calls run in isolated frames;
        // first-class value uses capture at the point of use, which
        // `auto_free_exclusions` handles program-wide.
        Statement::FunctionDecl(_) => {}
        Statement::Return(r) => {
            if let Some(v) = &r.value {
                scan_expr(v, m);
            }
        }
        Statement::Match(mx) => scan_match(mx, m),
        Statement::Expr(es) => scan_expr(&es.expr, m),
        Statement::CliArgsCapture(c) => m.mention(&c.variable_name),
        Statement::SetNumeralMode { .. } => {}
        Statement::Sleep(s) => scan_expr(&s.duration, m),
        Statement::KeyInput(k) => m.mention(&k.variable),
        Statement::ClearScreen(_) => {}
        Statement::OutputPos(op) => {
            for slot in op.slots.iter().flatten() {
                scan_expr(slot, m);
            }
            for item in &op.items {
                scan_expr(item, m);
            }
        }
        Statement::TuiBlock(tb) => scan_block(&tb.body, m),
    }
}

fn scan_match(mx: &zymbol_ast::MatchExpr, m: &mut Mentions) {
    scan_expr(&mx.scrutinee, m);
    for case in &mx.cases {
        scan_pattern(&case.pattern, m);
        if let Some(v) = &case.value {
            scan_expr(v, m);
        }
        if let Some(b) = &case.block {
            scan_block(b, m);
        }
    }
}

/// Walk the expression a precision operator carries, if it has one.
///
/// GAP-ZYB-001: the decimal count can be computed, and a name used there is a
/// use like any other. Missing it lets the last-use analyzer free the variable
/// before the operator reads it, which surfaced as
/// "use of 'n' after auto-destruction".
fn scan_precision(p: &zymbol_ast::Precision, f: &mut dyn FnMut(&Expr)) {
    if let zymbol_ast::Precision::Dynamic(e) = p {
        f(e);
    }
}

fn scan_precision_op(op: &Option<zymbol_ast::PrecisionOp>, f: &mut dyn FnMut(&Expr)) {
    if let Some(op) = op {
        scan_precision(op.precision(), f);
    }
}

fn scan_expr(expr: &Expr, m: &mut Mentions) {
    match expr {
        Expr::Literal(lit) => scan_literal(&lit.value, m),
        Expr::Identifier(ident) => {
            if ident.hot || ident.pre_hot {
                m.poison(&ident.name);
            } else {
                m.mention(&ident.name);
            }
        }
        Expr::Binary(b) => {
            scan_expr(&b.left, m);
            scan_expr(&b.right, m);
        }
        Expr::Unary(u) => scan_expr(&u.operand, m),
        Expr::Range(r) => {
            scan_expr(&r.start, m);
            scan_expr(&r.end, m);
            if let Some(step) = &r.step {
                scan_expr(step, m);
            }
        }
        Expr::ArrayLiteral(a) => {
            for e in &a.elements {
                scan_expr(e, m);
            }
        }
        Expr::Tuple(t) => {
            for e in &t.elements {
                scan_expr(e, m);
            }
        }
        Expr::Group(g) => scan_expr(&g.expr, m),
        Expr::NamedTuple(nt) => {
            for (_name, e) in &nt.fields {
                scan_expr(e, m);
            }
        }
        Expr::MemberAccess(ma) => scan_expr(&ma.object, m),
        Expr::Index(ix) => {
            scan_expr(&ix.array, m);
            scan_expr(&ix.index, m);
        }
        Expr::FunctionCall(call) => {
            // The callable itself may mention variables (lambda vars, ops[i](x)).
            scan_expr(&call.callable, m);
            for arg in &call.arguments {
                scan_expr(arg, m);
            }
        }
        Expr::Match(mx) => scan_match(mx, m),
        Expr::CollectionLength(op) => scan_expr(&op.collection, m),
        Expr::CollectionAppend(op) => {
            scan_expr(&op.collection, m);
            scan_expr(&op.element, m);
        }
        Expr::CollectionInsert(op) => {
            scan_expr(&op.collection, m);
            scan_expr(&op.index, m);
            scan_expr(&op.element, m);
        }
        Expr::CollectionRemoveValue(op) => {
            scan_expr(&op.collection, m);
            scan_expr(&op.value, m);
        }
        Expr::CollectionRemoveAll(op) => {
            scan_expr(&op.collection, m);
            scan_expr(&op.value, m);
        }
        Expr::CollectionRemoveAt(op) => {
            scan_expr(&op.collection, m);
            scan_expr(&op.index, m);
        }
        Expr::CollectionRemoveRange(op) => {
            scan_expr(&op.collection, m);
            if let Some(s) = &op.start {
                scan_expr(s, m);
            }
            if let Some(e) = &op.end {
                scan_expr(e, m);
            }
        }
        Expr::CollectionContains(op) => {
            scan_expr(&op.collection, m);
            scan_expr(&op.element, m);
        }
        Expr::CollectionFindAll(op) => {
            scan_expr(&op.collection, m);
            scan_expr(&op.value, m);
        }
        Expr::CollectionUpdate(op) => {
            scan_expr(&op.target, m);
            scan_expr(&op.value, m);
        }
        Expr::CollectionSlice(op) => {
            scan_expr(&op.collection, m);
            if let Some(s) = &op.start {
                scan_expr(s, m);
            }
            if let Some(e) = &op.end {
                scan_expr(e, m);
            }
        }
        Expr::StringRepeat(op) => {
            scan_expr(&op.string, m);
            scan_expr(&op.count, m);
        }
        Expr::StringReplace(op) => {
            scan_expr(&op.string, m);
            scan_expr(&op.pattern, m);
            scan_expr(&op.replacement, m);
            if let Some(c) = &op.count {
                scan_expr(c, m);
            }
        }
        Expr::StringSplit(op) => {
            scan_expr(&op.string, m);
            scan_expr(&op.delimiter, m);
        }
        Expr::ConcatBuild(op) => {
            scan_expr(&op.base, m);
            for item in &op.items {
                scan_expr(item, m);
            }
        }
        Expr::NumericEval(op) => scan_expr(&op.expr, m),
        Expr::TypeMetadata(op) => scan_expr(&op.expr, m),
        Expr::Format(op) => {
            scan_expr(&op.expr, m);
            scan_precision_op(&op.precision, &mut |e| scan_expr(e, m));
        }
        Expr::BaseConversion(op) => scan_expr(&op.expr, m),
        Expr::Lambda(lambda) => {
            // Capture-by-value happens where the lambda literal appears: every
            // outer name its body references is read HERE. Parameters shadow,
            // but treating them as mentions too is only conservative.
            match &lambda.body {
                zymbol_ast::LambdaBody::Expr(e) => scan_expr(e, m),
                zymbol_ast::LambdaBody::Block(b) => scan_block(b, m),
            }
        }
        Expr::CollectionMap(op) => {
            scan_expr(&op.collection, m);
            scan_expr(&op.lambda, m);
        }
        Expr::CollectionFilter(op) => {
            scan_expr(&op.collection, m);
            scan_expr(&op.lambda, m);
        }
        Expr::CollectionReduce(op) => {
            scan_expr(&op.collection, m);
            scan_expr(&op.initial, m);
            scan_expr(&op.lambda, m);
        }
        Expr::CollectionSortAsc(op) | Expr::CollectionSortDesc(op) | Expr::CollectionSortCustom(op) => {
            scan_expr(&op.collection, m);
            if let Some(cmp) = &op.comparator {
                scan_expr(cmp, m);
            }
        }
        Expr::Pipe(p) => {
            scan_expr(&p.left, m);
            scan_expr(&p.callable, m);
            for arg in &p.arguments {
                if let zymbol_ast::PipeArg::Expr(e) = arg {
                    scan_expr(e, m);
                }
            }
        }
        Expr::Execute(_) => {}
        Expr::BashExec(be) => {
            for arg in &be.args {
                scan_expr(arg, m);
            }
        }
        Expr::Round(op) => {
            scan_expr(&op.expr, m);
            scan_precision(&op.precision, &mut |e| scan_expr(e, m));
        }
        Expr::Trunc(op) => {
            scan_expr(&op.expr, m);
            scan_precision(&op.precision, &mut |e| scan_expr(e, m));
        }
        Expr::NumericCast(op) => scan_expr(&op.expr, m),
        Expr::ErrorCheck(op) => scan_expr(&op.expr, m),
        Expr::ErrorPropagate(op) => scan_expr(&op.expr, m),
        Expr::DeepIndex(di) => {
            scan_expr(&di.array, m);
            for step in &di.path.steps {
                scan_expr(&step.index, m);
                if let Some(end) = &step.range_end {
                    scan_expr(end, m);
                }
            }
        }
        Expr::FlatExtract(fe) => {
            scan_expr(&fe.array, m);
            for path in &fe.paths {
                for step in &path.steps {
                    scan_expr(&step.index, m);
                    if let Some(end) = &step.range_end {
                        scan_expr(end, m);
                    }
                }
            }
        }
        Expr::StructuredExtract(se) => {
            scan_expr(&se.array, m);
            for group in &se.groups {
                for path in &group.paths {
                    for step in &path.steps {
                        scan_expr(&step.index, m);
                        if let Some(end) = &step.range_end {
                            scan_expr(end, m);
                        }
                    }
                }
            }
        }
        Expr::TerminalSize(_) => {}
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Region-level creations
// ─────────────────────────────────────────────────────────────────────────────

/// Names a region-level statement creates (or may create) in the region scope.
fn creations(stmt: &Statement, out: &mut Vec<String>) {
    match stmt {
        Statement::Assignment(a) => {
            if !a.hot && !a.pre_hot {
                out.push(a.name.clone());
            }
        }
        Statement::DestructureAssign(d) => match &d.pattern {
            DestructurePattern::Array(items) | DestructurePattern::Positional(items) => {
                for item in items {
                    match item {
                        DestructureItem::Bind(n) | DestructureItem::Rest(n) => out.push(n.clone()),
                        DestructureItem::Ignore => {}
                    }
                }
            }
            DestructurePattern::NamedTuple(pairs) => {
                for (_field, var) in pairs {
                    out.push(var.clone());
                }
            }
        },
        Statement::Input(inp) => out.push(inp.variable.clone()),
        Statement::KeyInput(k) => out.push(k.variable.clone()),
        Statement::CliArgsCapture(c) => out.push(c.variable_name.clone()),
        _ => {}
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Public API
// ─────────────────────────────────────────────────────────────────────────────

/// Destruction schedule for one region: statement index → names to destroy
/// after that statement finishes normally (skipped when control flow is
/// pending — the frame or loop teardown owns cleanup on those paths).
pub fn region_schedule(
    stmts: &[Statement],
    param_candidates: &[String],
    excluded: &HashSet<String>,
) -> HashMap<usize, Vec<String>> {
    // Pass 1: collect mentions across the whole region.
    let mut m = Mentions::default();
    for (i, stmt) in stmts.iter().enumerate() {
        m.current = i;
        scan_stmt(stmt, &mut m);
    }

    // Pass 2: candidates = region-level creations + parameters.
    let mut candidates: HashSet<String> = param_candidates.iter().cloned().collect();
    let mut created = Vec::new();
    for stmt in stmts {
        creations(stmt, &mut created);
    }
    candidates.extend(created);

    let mut schedule: HashMap<usize, Vec<String>> = HashMap::new();
    for name in candidates {
        if name.starts_with('_') || m.poisoned.contains(&name) || excluded.contains(&name) {
            continue;
        }
        if let Some(&last) = m.last.get(&name) {
            schedule.entry(last).or_default().push(name);
        }
    }
    // Deterministic order inside each slot (stable across runs for tests).
    for names in schedule.values_mut() {
        names.sort();
    }
    schedule
}

/// Program-wide names that must never be auto-freed, in any region:
///
/// - names with hot (`°`) mentions anywhere,
/// - constants declared anywhere,
/// - free mentions of named functions that are *used as values* somewhere
///   (bare identifier outside a direct-call position): using a function as a
///   value snapshots its free variables at that moment, which a purely
///   regional analysis cannot see,
/// - for module programs (`# name { }`), every module-level binding: module
///   variables participate in the state write-back protocol and module
///   constants are re-marked at injection.
/// Every name mentioned anywhere in a block, nested blocks and lambda bodies
/// included, plus the names inside `{…}` string interpolations.
///
/// This is the same walk `auto_free_exclusions` uses, exposed because the
/// tree-walker needs the opposite question: not "which names may I free" but
/// "which of the module's bindings does this body actually touch". A module
/// function frame is given a copy of the module's state on entry, and the
/// tree-walker's values are not reference-counted, so a module holding a
/// sixty-key table paid a deep copy of it on every call — including calls to
/// functions that never name the table (REFERENCE.md L44).
///
/// The answer is deliberately an **over-approximation**: a name that appears
/// anywhere counts, whether or not the mention is reached. Injecting too much
/// is what the code did before and is always safe; injecting too little would
/// be an undefined-variable error, so the walk being exhaustive over `Expr`
/// (no `_` arm) is what makes this usable.
pub fn mentioned_names(block: &zymbol_ast::Block) -> HashSet<String> {
    let mut m = Mentions::default();
    for stmt in &block.statements {
        scan_stmt(stmt, &mut m);
    }
    let mut out: HashSet<String> = m.last.into_keys().collect();
    out.extend(m.poisoned);
    out
}

pub fn auto_free_exclusions(program: &Program) -> HashSet<String> {
    let mut excluded = HashSet::new();

    // Hot mentions + constants anywhere in the program (single full walk).
    let mut all = Mentions::default();
    for stmt in &program.statements {
        scan_stmt(stmt, &mut all);
        // scan_stmt skips FunctionDecl bodies — walk them here for hot/const
        // poisoning and to collect per-function data below.
        collect_decl_poisons(stmt, &mut excluded);
    }
    excluded.extend(all.poisoned.iter().cloned());

    // Named functions: name → free mention set (mentions − params − names
    // whose first mention is a region-level creation in the body, walked
    // sequentially like the runtime capture collector — over-approximated
    // toward "free", which only widens the exclusion set).
    let mut fn_free: HashMap<String, HashSet<String>> = HashMap::new();
    collect_fn_free_vars(&program.statements, &mut fn_free);

    // Which function names are used as values anywhere (top level or inside
    // any body)? A mention in a direct-call callable position is a call, not
    // a value use.
    let mut value_used: HashSet<String> = HashSet::new();
    let fn_names: HashSet<&String> = fn_free.keys().collect();
    scan_value_uses(&program.statements, &fn_names, &mut value_used);

    for name in &value_used {
        if let Some(free) = fn_free.get(name) {
            excluded.extend(free.iter().cloned());
        }
    }

    // Module programs: module-level bindings are owned by the module state
    // protocol, never by auto-free.
    if program.module_decl.is_some() {
        let mut created = Vec::new();
        for stmt in &program.statements {
            creations(stmt, &mut created);
        }
        excluded.extend(created);
    }

    excluded
}

/// Poison hot/const names found inside function declaration bodies (recursing
/// into nested declarations).
fn collect_decl_poisons(stmt: &Statement, excluded: &mut HashSet<String>) {
    if let Statement::FunctionDecl(decl) = stmt {
        let mut m = Mentions::default();
        for s in &decl.body.statements {
            scan_stmt(s, &mut m);
            collect_decl_poisons(s, excluded);
        }
        excluded.extend(m.poisoned);
    }
}

/// For every function declaration (recursively), compute its free mention set.
fn collect_fn_free_vars(stmts: &[Statement], out: &mut HashMap<String, HashSet<String>>) {
    for stmt in stmts {
        if let Statement::FunctionDecl(decl) = stmt {
            let params: HashSet<&str> = decl.parameters.iter().map(|p| p.name.as_str()).collect();
            // Sequential region walk mirroring the runtime capture collector:
            // a creation makes the name local only from that statement on.
            let mut local: HashSet<String> = HashSet::new();
            let mut free: HashSet<String> = HashSet::new();
            for (i, s) in decl.body.statements.iter().enumerate() {
                let mut m = Mentions::default();
                m.current = i;
                scan_stmt(s, &mut m);
                let mut created = Vec::new();
                creations(s, &mut created);
                for name in m.last.keys() {
                    if params.contains(name.as_str()) || local.contains(name) {
                        continue;
                    }
                    // A creation statement mentions its own target; the target
                    // becomes local, not free — unless the same statement also
                    // reads it (e.g. `x = x + 1`), which we cannot separate
                    // here, so treat self-creating mentions as local only when
                    // the statement is a pure creation of that name.
                    if created.contains(name) && !mention_reads_name(s, name) {
                        continue;
                    }
                    free.insert(name.clone());
                }
                for name in created {
                    local.insert(name);
                }
            }
            out.insert(decl.name.clone(), free);
            // Nested declarations inside this body
            collect_fn_free_vars(&decl.body.statements, out);
        }
    }
}

/// Does this statement mention `name` outside of its own creation target?
/// (Approximate: scans the value/subject expressions only.)
fn mention_reads_name(stmt: &Statement, name: &str) -> bool {
    let mut m = Mentions::default();
    match stmt {
        Statement::Assignment(a) => scan_expr(&a.value, &mut m),
        Statement::DestructureAssign(d) => scan_expr(&d.value, &mut m),
        Statement::Input(inp) => {
            if let Some(InputPrompt::Interpolated(parts)) = &inp.prompt {
                for part in parts {
                    if let StringPart::Variable(n) = part {
                        m.mention(n);
                    }
                }
            }
        }
        _ => return true, // unknown creation shape: stay conservative
    }
    m.last.contains_key(name)
}

/// Find value-uses of named functions: any identifier mention of a function
/// name that is not the callable of a direct call.
fn scan_value_uses(
    stmts: &[Statement],
    fn_names: &HashSet<&String>,
    value_used: &mut HashSet<String>,
) {
    // Walk everything (including declaration bodies): a value use inside any
    // function body still snapshots at that point.
    for stmt in stmts {
        if let Statement::FunctionDecl(decl) = stmt {
            scan_value_uses(&decl.body.statements, fn_names, value_used);
            continue;
        }
        let mut m = ValueUseScan { fn_names, value_used };
        m.stmt(stmt);
    }
}

/// Dedicated walker that distinguishes call-position identifier mentions from
/// value-position ones. It reuses the exhaustive `scan_expr` for everything
/// except the two constructs where the distinction matters.
struct ValueUseScan<'a> {
    fn_names: &'a HashSet<&'a String>,
    value_used: &'a mut HashSet<String>,
}

impl<'a> ValueUseScan<'a> {
    fn stmt(&mut self, stmt: &Statement) {
        // Collect all mentions of the statement subtree...
        let mut all = Mentions::default();
        scan_stmt(stmt, &mut all);
        // ...and all identifiers that appear strictly as direct-call callables.
        let mut call_only = Mentions::default();
        collect_call_callables(stmt, &mut call_only);
        for name in all.last.keys() {
            if self.fn_names.contains(name) && !call_only.last.contains_key(name) {
                self.value_used.insert(name.clone());
            }
        }
        // A name mentioned BOTH as call callable and elsewhere in the same
        // statement would be missed above — rescan expressions for extra
        // (non-callable) occurrences.
        let mut extra = ExtraOccurrences {
            fn_names: self.fn_names,
            value_used: self.value_used,
        };
        extra.stmt(stmt);
    }
}

/// Collect identifiers appearing as `FunctionCall.callable` anywhere.
fn collect_call_callables(stmt: &Statement, m: &mut Mentions) {
    let mut all = Mentions::default();
    scan_stmt(stmt, &mut all);
    // We only need names — reuse a focused expression walk instead:
    struct CallableWalk<'a> {
        m: &'a mut Mentions,
    }
    impl<'a> CallableWalk<'a> {
        fn expr(&mut self, e: &Expr) {
            if let Expr::FunctionCall(call) = e {
                if let Expr::Identifier(id) = call.callable.unwrap_group() {
                    self.m.mention(&id.name);
                }
            }
            walk_sub_exprs(e, &mut |sub| self.expr(sub));
        }
    }
    let mut w = CallableWalk { m };
    walk_stmt_exprs(stmt, &mut |e| w.expr(e));
}

/// Detect fn-name identifiers occurring outside call-callable position, even
/// when the same name also appears as a callable in the same statement.
struct ExtraOccurrences<'a> {
    fn_names: &'a HashSet<&'a String>,
    value_used: &'a mut HashSet<String>,
}

impl<'a> ExtraOccurrences<'a> {
    fn stmt(&mut self, stmt: &Statement) {
        walk_stmt_exprs(stmt, &mut |e| self.expr(e));
    }
    fn expr(&mut self, e: &Expr) {
        match e {
            Expr::FunctionCall(call) => {
                // Callable identifier = call position; anything else recurses.
                if !matches!(call.callable.unwrap_group(), Expr::Identifier(_)) {
                    self.expr(&call.callable);
                }
                for arg in &call.arguments {
                    self.expr(arg);
                }
            }
            Expr::Identifier(id) => {
                if self.fn_names.contains(&id.name) {
                    self.value_used.insert(id.name.clone());
                }
            }
            other => walk_sub_exprs(other, &mut |sub| self.expr(sub)),
        }
    }
}

/// Visit every direct sub-expression of `e` (one level).
pub(crate) fn walk_sub_exprs(e: &Expr, f: &mut dyn FnMut(&Expr)) {
    // Reuse the exhaustive mention walker structure by scanning into a probe
    // is not possible (it flattens identifiers) — instead enumerate one level
    // via a Mentions-independent traversal built on scan_expr's shape.
    // To avoid duplicating 50 arms, we lean on a generic trick: scan_expr is
    // the single source of truth for reachability; here we only need the
    // recursion topology for identifier-position analysis, and the two
    // walkers above (CallableWalk/ExtraOccurrences) recurse through this
    // function. Enumerate sub-expressions explicitly:
    match e {
        Expr::Literal(_) | Expr::Identifier(_) | Expr::Execute(_) | Expr::TerminalSize(_) => {}
        Expr::Binary(b) => {
            f(&b.left);
            f(&b.right);
        }
        Expr::Unary(u) => f(&u.operand),
        Expr::Range(r) => {
            f(&r.start);
            f(&r.end);
            if let Some(s) = &r.step {
                f(s);
            }
        }
        Expr::ArrayLiteral(a) => {
            for x in &a.elements {
                f(x);
            }
        }
        Expr::Tuple(t) => {
            for x in &t.elements {
                f(x);
            }
        }
        Expr::Group(g) => f(&g.expr),
        Expr::NamedTuple(nt) => {
            for (_n, x) in &nt.fields {
                f(x);
            }
        }
        Expr::MemberAccess(ma) => f(&ma.object),
        Expr::Index(ix) => {
            f(&ix.array);
            f(&ix.index);
        }
        Expr::FunctionCall(c) => {
            f(&c.callable);
            for a in &c.arguments {
                f(a);
            }
        }
        Expr::Match(mx) => {
            f(&mx.scrutinee);
            for case in &mx.cases {
                if let Pattern::Range(s, e2, _) = &case.pattern {
                    f(s);
                    f(e2);
                }
                if let Pattern::Comparison(_, x, _) = &case.pattern {
                    f(x);
                }
                if let Some(v) = &case.value {
                    f(v);
                }
                if let Some(b) = &case.block {
                    for st in &b.statements {
                        walk_stmt_exprs(st, f);
                    }
                }
            }
        }
        Expr::CollectionLength(op) => f(&op.collection),
        Expr::CollectionAppend(op) => {
            f(&op.collection);
            f(&op.element);
        }
        Expr::CollectionInsert(op) => {
            f(&op.collection);
            f(&op.index);
            f(&op.element);
        }
        Expr::CollectionRemoveValue(op) => {
            f(&op.collection);
            f(&op.value);
        }
        Expr::CollectionRemoveAll(op) => {
            f(&op.collection);
            f(&op.value);
        }
        Expr::CollectionRemoveAt(op) => {
            f(&op.collection);
            f(&op.index);
        }
        Expr::CollectionRemoveRange(op) => {
            f(&op.collection);
            if let Some(s) = &op.start {
                f(s);
            }
            if let Some(x) = &op.end {
                f(x);
            }
        }
        Expr::CollectionContains(op) => {
            f(&op.collection);
            f(&op.element);
        }
        Expr::CollectionFindAll(op) => {
            f(&op.collection);
            f(&op.value);
        }
        Expr::CollectionUpdate(op) => {
            f(&op.target);
            f(&op.value);
        }
        Expr::CollectionSlice(op) => {
            f(&op.collection);
            if let Some(s) = &op.start {
                f(s);
            }
            if let Some(x) = &op.end {
                f(x);
            }
        }
        Expr::StringRepeat(op) => {
            f(&op.string);
            f(&op.count);
        }
        Expr::StringReplace(op) => {
            f(&op.string);
            f(&op.pattern);
            f(&op.replacement);
            if let Some(c) = &op.count {
                f(c);
            }
        }
        Expr::StringSplit(op) => {
            f(&op.string);
            f(&op.delimiter);
        }
        Expr::ConcatBuild(op) => {
            f(&op.base);
            for x in &op.items {
                f(x);
            }
        }
        Expr::NumericEval(op) => f(&op.expr),
        Expr::TypeMetadata(op) => f(&op.expr),
        Expr::Format(op) => {
            f(&op.expr);
            scan_precision_op(&op.precision, f);
        }
        Expr::BaseConversion(op) => f(&op.expr),
        Expr::Lambda(lambda) => match &lambda.body {
            zymbol_ast::LambdaBody::Expr(x) => f(x),
            zymbol_ast::LambdaBody::Block(b) => {
                for st in &b.statements {
                    walk_stmt_exprs(st, f);
                }
            }
        },
        Expr::CollectionMap(op) => {
            f(&op.collection);
            f(&op.lambda);
        }
        Expr::CollectionFilter(op) => {
            f(&op.collection);
            f(&op.lambda);
        }
        Expr::CollectionReduce(op) => {
            f(&op.collection);
            f(&op.initial);
            f(&op.lambda);
        }
        Expr::CollectionSortAsc(op) | Expr::CollectionSortDesc(op) | Expr::CollectionSortCustom(op) => {
            f(&op.collection);
            if let Some(c) = &op.comparator {
                f(c);
            }
        }
        Expr::Pipe(p) => {
            f(&p.left);
            f(&p.callable);
            for arg in &p.arguments {
                if let zymbol_ast::PipeArg::Expr(x) = arg {
                    f(x);
                }
            }
        }
        Expr::BashExec(be) => {
            for a in &be.args {
                f(a);
            }
        }
        Expr::Round(op) => {
            f(&op.expr);
            scan_precision(&op.precision, f);
        }
        Expr::Trunc(op) => {
            f(&op.expr);
            scan_precision(&op.precision, f);
        }
        Expr::NumericCast(op) => f(&op.expr),
        Expr::ErrorCheck(op) => f(&op.expr),
        Expr::ErrorPropagate(op) => f(&op.expr),
        Expr::DeepIndex(di) => {
            f(&di.array);
            for step in &di.path.steps {
                f(&step.index);
                if let Some(x) = &step.range_end {
                    f(x);
                }
            }
        }
        Expr::FlatExtract(fe) => {
            f(&fe.array);
            for path in &fe.paths {
                for step in &path.steps {
                    f(&step.index);
                    if let Some(x) = &step.range_end {
                        f(x);
                    }
                }
            }
        }
        Expr::StructuredExtract(se) => {
            f(&se.array);
            for group in &se.groups {
                for path in &group.paths {
                    for step in &path.steps {
                        f(&step.index);
                        if let Some(x) = &step.range_end {
                            f(x);
                        }
                    }
                }
            }
        }
    }
}

/// Visit every top-level expression of a statement (recursing through nested
/// blocks but NOT into function declaration bodies).
pub(crate) fn walk_stmt_exprs(stmt: &Statement, f: &mut dyn FnMut(&Expr)) {
    match stmt {
        Statement::Output(o) => {
            for e in &o.exprs {
                f(e);
            }
        }
        Statement::Assignment(a) => f(&a.value),
        Statement::ConstDecl(c) => f(&c.value),
        Statement::DestructureAssign(d) => f(&d.value),
        Statement::LifetimeEnd(_) | Statement::Newline(_) => {}
        Statement::Input(_) => {}
        Statement::If(i) => {
            f(&i.condition);
            for st in &i.then_block.statements {
                walk_stmt_exprs(st, f);
            }
            for br in &i.else_if_branches {
                f(&br.condition);
                for st in &br.block.statements {
                    walk_stmt_exprs(st, f);
                }
            }
            if let Some(eb) = &i.else_block {
                for st in &eb.statements {
                    walk_stmt_exprs(st, f);
                }
            }
        }
        Statement::Loop(l) => {
            if let Some(c) = &l.condition {
                f(c);
            }
            if let Some(it) = &l.iterable {
                f(it);
            }
            for st in &l.body.statements {
                walk_stmt_exprs(st, f);
            }
        }
        Statement::Break(_) | Statement::Continue(_) => {}
        Statement::Try(t) => {
            for st in &t.try_block.statements {
                walk_stmt_exprs(st, f);
            }
            for cl in &t.catch_clauses {
                for st in &cl.block.statements {
                    walk_stmt_exprs(st, f);
                }
            }
            if let Some(fin) = &t.finally_clause {
                for st in &fin.block.statements {
                    walk_stmt_exprs(st, f);
                }
            }
        }
        Statement::FunctionDecl(_) => {}
        Statement::Return(r) => {
            if let Some(v) = &r.value {
                f(v);
            }
        }
        Statement::Match(mx) => {
            f(&mx.scrutinee);
            for case in &mx.cases {
                if let Some(v) = &case.value {
                    f(v);
                }
                if let Some(b) = &case.block {
                    for st in &b.statements {
                        walk_stmt_exprs(st, f);
                    }
                }
            }
        }
        Statement::Expr(es) => f(&es.expr),
        Statement::CliArgsCapture(_) => {}
        Statement::SetNumeralMode { .. } => {}
        Statement::Sleep(s) => f(&s.duration),
        Statement::KeyInput(_) | Statement::ClearScreen(_) => {}
        Statement::OutputPos(op) => {
            for slot in op.slots.iter().flatten() {
                f(slot);
            }
            for item in &op.items {
                f(item);
            }
        }
        Statement::TuiBlock(tb) => {
            for st in &tb.body.statements {
                walk_stmt_exprs(st, f);
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use zymbol_lexer::Lexer;
    use zymbol_parser::Parser;
    use zymbol_span::FileId;

    fn parse(src: &str) -> Program {
        let lexer = Lexer::new(src, FileId(0));
        let (tokens, diags) = lexer.tokenize();
        assert!(diags.is_empty(), "lex errors: {:?}", diags);
        Parser::new(tokens).parse().expect("parse failed")
    }

    fn schedule(src: &str) -> HashMap<usize, Vec<String>> {
        let program = parse(src);
        let excluded = auto_free_exclusions(&program);
        region_schedule(&program.statements, &[], &excluded)
    }

    #[test]
    fn simple_last_use() {
        let s = schedule("x = 1\n>> x ¶\n>> 2 ¶");
        assert_eq!(s.get(&1), Some(&vec!["x".to_string()]));
    }

    #[test]
    fn interpolation_counts_as_use() {
        let s = schedule("x = 1\n>> \"v={x}\" ¶\n>> 2 ¶");
        assert_eq!(s.get(&1), Some(&vec!["x".to_string()]));
        assert!(s.get(&0).is_none());
    }

    #[test]
    fn mention_inside_loop_counts_at_loop_statement() {
        let s = schedule("total = 0\n@ i:1..3 {\n    total = total + i\n}\n>> total ¶");
        // total: last mention at statement 2 (the output)
        assert_eq!(s.get(&2), Some(&vec!["total".to_string()]));
    }

    #[test]
    fn const_never_scheduled() {
        let s = schedule("K := 5\n>> K ¶");
        assert!(s.values().all(|v| !v.contains(&"K".to_string())));
    }

    #[test]
    fn hot_names_never_scheduled() {
        let s = schedule("@ i:1..3 { °acc += i }\n>> acc ¶");
        assert!(s.values().all(|v| !v.contains(&"acc".to_string())));
    }

    #[test]
    fn underscore_never_scheduled() {
        let s = schedule("_t = 1\n>> _t ¶");
        assert!(s.is_empty());
    }

    #[test]
    fn value_used_function_free_vars_excluded() {
        let program = parse("base = 10\nadder(n) { <~ n + base }\nf = adder\n>> f(5) ¶");
        let excluded = auto_free_exclusions(&program);
        assert!(excluded.contains("base"), "base must be excluded: {:?}", excluded);
        let s = region_schedule(&program.statements, &[], &excluded);
        assert!(s.values().all(|v| !v.contains(&"base".to_string())));
    }

    #[test]
    fn direct_call_only_fn_does_not_exclude() {
        // g is only ever called directly — its body locals stay schedulable
        // in the outer region.
        let program = parse("x = 1\ng(n) { <~ n }\n>> g(x) ¶\n>> 2 ¶");
        let excluded = auto_free_exclusions(&program);
        let s = region_schedule(&program.statements, &[], &excluded);
        assert_eq!(s.get(&2), Some(&vec!["x".to_string()]));
    }

    #[test]
    fn hof_operand_is_value_use() {
        let program = parse("k = 2\ndouble(x) { <~ x * k }\nr = [1,2]$> double\n>> r ¶");
        let excluded = auto_free_exclusions(&program);
        assert!(excluded.contains("k"), "k must be excluded: {:?}", excluded);
    }

    #[test]
    fn module_level_names_excluded() {
        let program = parse("# m {\n    #> { f }\n    count = 0\n    f() { count = count + 1 }\n}");
        let excluded = auto_free_exclusions(&program);
        assert!(excluded.contains("count"));
    }

    #[test]
    fn params_are_candidates() {
        let program = parse("f(a, b) {\n    c = a + 1\n    >> c ¶\n    <~ b\n}");
        let Statement::FunctionDecl(decl) = &program.statements[0] else {
            panic!("expected decl")
        };
        let excluded = HashSet::new();
        let s = region_schedule(
            &decl.body.statements,
            &["a".to_string(), "b".to_string()],
            &excluded,
        );
        // a last mentioned at stmt 0 (`c = a + 1`); c at stmt 1 (`>> c`);
        // b at the `<~ b` return statement (destruction there is skipped at
        // runtime by the control-flow guard). Statement indices account for
        // parser-inserted Newline statements, so locate the return by shape.
        let ret_idx = decl.body.statements.iter().position(|st| matches!(st, Statement::Return(_))).unwrap();
        assert_eq!(s.get(&0), Some(&vec!["a".to_string()]));
        assert_eq!(s.get(&1), Some(&vec!["c".to_string()]));
        assert_eq!(s.get(&ret_idx), Some(&vec!["b".to_string()]));
    }

    #[test]
    fn lambda_body_mentions_count_at_lambda_statement() {
        let s = schedule("m = 3\nf = (x -> x * m)\n>> f(2) ¶");
        // m captured at statement 1 → last mention index 1
        assert_eq!(s.get(&1).map(|v| v.contains(&"m".to_string())), Some(true));
    }
}
