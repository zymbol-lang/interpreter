//! Definition-Use Chain Analysis for Zymbol-Lang
//!
//! Tracks where variables are defined and used to determine lifetimes

use std::collections::{HashMap, HashSet};
use zymbol_ast::{DestructureItem, DestructurePattern, Expr, Statement};
use zymbol_span::Span;
use crate::cfg::{ControlFlowGraph, NodeId};

/// Type of variable use
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UseType {
    /// Variable is read
    Read,
    /// Variable is written/assigned
    Write,
    /// Variable is both read and written (e.g., x = x + 1)
    ReadWrite,
}

/// A definition of a variable
#[derive(Debug, Clone)]
pub struct Definition {
    /// Variable name
    pub var_name: String,
    /// CFG node where defined
    pub node: NodeId,
    /// Source location
    pub span: Span,
    /// Is this an underscore variable (_variable)?
    pub is_underscore: bool,
    /// Scope depth where defined
    pub scope_depth: usize,
}

/// A use of a variable
#[derive(Debug, Clone)]
pub struct Use {
    /// Variable name
    pub var_name: String,
    /// CFG node where used
    pub node: NodeId,
    /// Source location
    pub span: Span,
    /// Type of use (read/write/both)
    pub use_type: UseType,
}

/// Reason why a lifetime is ambiguous
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AmbiguityReason {
    /// Variable is modified inside a loop
    LoopVariant,
    /// Variable is used in some branches but not others
    ConditionalUse,
    /// Multiple possible last uses that can reach each other
    MultipleExitPaths,
}

/// An ambiguous lifetime case
#[derive(Debug, Clone)]
pub struct AmbiguousLifetime {
    /// Variable name
    pub variable: String,
    /// Reason for ambiguity
    pub reason: AmbiguityReason,
    /// Nodes involved in the ambiguity
    pub nodes: HashSet<NodeId>,
    /// Suggested location for explicit lifetime annotation
    pub suggested_span: Span,
}

/// Definition-Use chain for a single variable
#[derive(Debug, Clone)]
pub struct DefUseChain {
    /// Variable name
    pub variable: String,
    /// All definitions of this variable
    pub definitions: Vec<Definition>,
    /// All uses of this variable
    pub uses: Vec<Use>,
    /// Nodes where this is the last use (computed)
    pub last_uses: HashSet<NodeId>,
    /// Is this variable ambiguous (needs explicit lifetime)?
    pub is_ambiguous: bool,
    /// Ambiguity details if ambiguous
    pub ambiguity: Option<AmbiguousLifetime>,
}

impl DefUseChain {
    /// Create a new def-use chain
    pub fn new(variable: String) -> Self {
        Self {
            variable,
            definitions: Vec::new(),
            uses: Vec::new(),
            last_uses: HashSet::new(),
            is_ambiguous: false,
            ambiguity: None,
        }
    }

    /// Add a definition
    pub fn add_definition(&mut self, def: Definition) {
        self.definitions.push(def);
    }

    /// Add a use
    pub fn add_use(&mut self, use_: Use) {
        self.uses.push(use_);
    }

    /// Find last uses of this variable
    /// A use is "last" if no other uses are reachable from it
    /// Only Read and ReadWrite uses are considered (not pure Write)
    pub fn find_last_uses(&mut self, cfg: &ControlFlowGraph) {
        // Filter to only Read and ReadWrite uses (ignore pure Write)
        let read_uses: Vec<&Use> = self.uses.iter()
            .filter(|u| matches!(u.use_type, UseType::Read | UseType::ReadWrite))
            .collect();

        if read_uses.is_empty() {
            // No actual reads - variable is never used (only assigned)
            return;
        }

        // Get all read use nodes
        let use_nodes: HashSet<NodeId> = read_uses.iter().map(|u| u.node).collect();

        // For each read use, check if any other uses are reachable from it
        for use_ in &read_uses {
            let reachable_uses = find_reachable_uses(use_.node, &use_nodes, cfg);

            if reachable_uses.is_empty() {
                // No uses reachable from this use - this is a last use
                self.last_uses.insert(use_.node);
            }
        }

        // Check for ambiguity
        if self.last_uses.len() > 1 {
            // Multiple last uses - check if they're mutually exclusive
            if !are_mutually_exclusive(&self.last_uses, cfg) {
                self.is_ambiguous = true;
                self.ambiguity = Some(AmbiguousLifetime {
                    variable: self.variable.clone(),
                    reason: AmbiguityReason::MultipleExitPaths,
                    nodes: self.last_uses.clone(),
                    suggested_span: self.uses.last().unwrap().span,
                });
            }
        }
    }

    /// Check if this variable is modified in a loop
    pub fn check_loop_variant(&mut self, cfg: &ControlFlowGraph) {
        // Find back edges (loops)
        let back_edges = cfg.find_back_edges();

        for (tail, head) in &back_edges {
            // Get nodes in this loop
            let loop_nodes = nodes_in_loop(*tail, *head, cfg);

            // Check if any writes happen in the loop
            let has_write_in_loop = self.uses.iter().any(|u| {
                matches!(u.use_type, UseType::Write | UseType::ReadWrite)
                    && loop_nodes.contains(&u.node)
            });

            if has_write_in_loop {
                self.is_ambiguous = true;
                self.ambiguity = Some(AmbiguousLifetime {
                    variable: self.variable.clone(),
                    reason: AmbiguityReason::LoopVariant,
                    nodes: loop_nodes.clone(),
                    suggested_span: self.uses.iter()
                        .find(|u| loop_nodes.contains(&u.node))
                        .map(|u| u.span)
                        .unwrap_or_else(|| self.uses[0].span),
                });
                break;
            }
        }
    }
}

/// Find all uses reachable from a given node
fn find_reachable_uses(
    from: NodeId,
    all_uses: &HashSet<NodeId>,
    cfg: &ControlFlowGraph,
) -> Vec<NodeId> {
    let mut reachable = Vec::new();
    let mut visited = HashSet::new();
    let mut worklist = vec![from];

    while let Some(node) = worklist.pop() {
        if visited.contains(&node) {
            continue;
        }
        visited.insert(node);

        // Don't include the starting node itself
        if node != from && all_uses.contains(&node) {
            reachable.push(node);
        }

        // Add successors to worklist
        for &successor in cfg.successors(node) {
            if !visited.contains(&successor) {
                worklist.push(successor);
            }
        }
    }

    reachable
}

/// Check if a set of nodes are mutually exclusive (no path between any pair)
fn are_mutually_exclusive(nodes: &HashSet<NodeId>, cfg: &ControlFlowGraph) -> bool {
    let nodes_vec: Vec<NodeId> = nodes.iter().copied().collect();

    // Check all pairs
    for i in 0..nodes_vec.len() {
        for j in (i + 1)..nodes_vec.len() {
            let n1 = nodes_vec[i];
            let n2 = nodes_vec[j];

            // Check if there's a path from n1 to n2 or n2 to n1
            if has_path(n1, n2, cfg) || has_path(n2, n1, cfg) {
                return false; // Not mutually exclusive
            }
        }
    }

    true // All pairs are mutually exclusive
}

/// Check if there's a path from start to end in the CFG
fn has_path(start: NodeId, end: NodeId, cfg: &ControlFlowGraph) -> bool {
    let mut visited = HashSet::new();
    let mut worklist = vec![start];

    while let Some(node) = worklist.pop() {
        if node == end {
            return true;
        }

        if visited.contains(&node) {
            continue;
        }
        visited.insert(node);

        for &successor in cfg.successors(node) {
            if !visited.contains(&successor) {
                worklist.push(successor);
            }
        }
    }

    false
}

/// Get all nodes that are part of a loop (between tail and head)
fn nodes_in_loop(tail: NodeId, head: NodeId, cfg: &ControlFlowGraph) -> HashSet<NodeId> {
    let mut loop_nodes = HashSet::new();
    loop_nodes.insert(head);

    // Find all nodes that can reach tail and are reachable from head
    let mut worklist = vec![tail];
    let mut visited = HashSet::new();

    while let Some(node) = worklist.pop() {
        if visited.contains(&node) || node == head {
            continue;
        }
        visited.insert(node);
        loop_nodes.insert(node);

        // Walk backwards to find all nodes that can reach tail
        for &pred in cfg.predecessors(node) {
            if !visited.contains(&pred) {
                worklist.push(pred);
            }
        }
    }

    loop_nodes
}

/// Def-Use analyzer for a program
#[derive(Debug)]
pub struct DefUseAnalyzer {
    /// Def-use chains for all variables
    chains: HashMap<String, DefUseChain>,
    /// Current scope depth
    scope_depth: usize,
}

impl DefUseAnalyzer {
    /// Create a new analyzer
    pub fn new() -> Self {
        Self {
            chains: HashMap::new(),
            scope_depth: 0,
        }
    }

    /// Analyze statements and build def-use chains
    pub fn analyze(&mut self, statements: &[Statement], cfg: &ControlFlowGraph) -> HashMap<String, DefUseChain> {
        // Build initial chains from statements
        // We need to use CFG NodeIds, not enumerate indices
        // In sequential CFG: NodeId 0 = Entry, NodeId 1 = Exit, NodeId 2 = Stmt[0], NodeId 3 = Stmt[1], etc.
        for (stmt_index, stmt) in statements.iter().enumerate() {
            // Map statement index to CFG NodeId
            // For sequential CFG: NodeId = stmt_index + 2 (because Entry=0, Exit=1 are created first)
            let node_id = stmt_index + 2;
            self.analyze_statement(stmt, node_id);
        }

        // Compute last uses for each chain
        for chain in self.chains.values_mut() {
            chain.find_last_uses(cfg);
            chain.check_loop_variant(cfg);
        }

        self.chains.clone()
    }

    /// Generate destruction schedule from def-use chains
    /// Returns a map from statement_index -> list of variables to destroy
    /// Requires the CFG to map NodeIds to statement indices
    pub fn generate_destruction_schedule(&self, cfg: &ControlFlowGraph) -> HashMap<usize, Vec<String>> {
        let mut schedule: HashMap<usize, Vec<String>> = HashMap::new();

        for (var_name, chain) in &self.chains {
            // Skip ambiguous variables - they need explicit lifetime annotations
            if chain.is_ambiguous {
                continue;
            }

            // Skip underscore variables - they're destroyed at block end via scoping
            if var_name.starts_with('_') {
                continue;
            }

            // For each last use, schedule destruction after that statement
            for &last_use_node in &chain.last_uses {
                // Map CFG NodeId to actual statement index
                if let Some(crate::cfg::CfgNode::Statement { stmt_index, .. }) = cfg.get_node(last_use_node) {
                    schedule
                        .entry(*stmt_index)
                        .or_default()
                        .push(var_name.clone());
                    // Ignore Entry, Exit, and Condition nodes - they don't correspond to statements
                }
            }
        }

        schedule
    }

    /// Analyze a single statement
    fn analyze_statement(&mut self, stmt: &Statement, node_index: usize) {
        match stmt {
            Statement::Assignment(assign) => {
                // This is a definition (write)
                let chain = self.chains.entry(assign.name.clone()).or_insert_with(|| {
                    DefUseChain::new(assign.name.clone())
                });

                // Check if this is the first definition or a reassignment
                if chain.definitions.is_empty() {
                    // First definition
                    chain.add_definition(Definition {
                        var_name: assign.name.clone(),
                        node: node_index,
                        span: assign.span,
                        is_underscore: assign.name.starts_with('_'),
                        scope_depth: self.scope_depth,
                    });
                } else {
                    // Reassignment - also a use (read-write)
                    chain.add_use(Use {
                        var_name: assign.name.clone(),
                        node: node_index,
                        span: assign.span,
                        use_type: UseType::Write,
                    });
                }

                // Analyze RHS expression for reads
                self.analyze_expr(&assign.value, node_index);
            }

            Statement::ConstDecl(const_decl) => {
                // Constants are definitions
                let chain = self.chains.entry(const_decl.name.clone()).or_insert_with(|| {
                    DefUseChain::new(const_decl.name.clone())
                });

                chain.add_definition(Definition {
                    var_name: const_decl.name.clone(),
                    node: node_index,
                    span: const_decl.span,
                    is_underscore: const_decl.name.starts_with('_'),
                    scope_depth: self.scope_depth,
                });

                // Analyze RHS
                self.analyze_expr(&const_decl.value, node_index);
            }

            Statement::Output(output) => {
                // Analyze output expressions
                for expr in &output.exprs {
                    self.analyze_expr(expr, node_index);
                }
            }

            Statement::Input(input) => {
                // Input is a definition
                let chain = self.chains.entry(input.variable.clone()).or_insert_with(|| {
                    DefUseChain::new(input.variable.clone())
                });

                chain.add_definition(Definition {
                    var_name: input.variable.clone(),
                    node: node_index,
                    span: input.span,
                    is_underscore: input.variable.starts_with('_'),
                    scope_depth: self.scope_depth,
                });
            }

            Statement::Return(ret) => {
                if let Some(expr) = &ret.value {
                    self.analyze_expr(expr, node_index);
                }
            }

            Statement::Expr(expr_stmt) => {
                self.analyze_expr(&expr_stmt.expr, node_index);
            }

            Statement::DestructureAssign(d) => {
                // RHS is a use
                self.analyze_expr(&d.value, node_index);
                // Each bound variable on LHS is a definition
                let bound_names: Vec<String> = match &d.pattern {
                    DestructurePattern::Array(items) | DestructurePattern::Positional(items) => {
                        items.iter().filter_map(|item| match item {
                            DestructureItem::Bind(name) | DestructureItem::Rest(name) => Some(name.clone()),
                            DestructureItem::Ignore => None,
                        }).collect()
                    }
                    DestructurePattern::NamedTuple(pairs) => {
                        pairs.iter().map(|(_, var)| var.clone()).collect()
                    }
                };
                for name in bound_names {
                    let chain = self.chains.entry(name.clone()).or_insert_with(|| {
                        DefUseChain::new(name.clone())
                    });
                    chain.add_definition(Definition {
                        var_name: name.clone(),
                        node: node_index,
                        span: d.span,
                        is_underscore: name.starts_with('_'),
                        scope_depth: self.scope_depth,
                    });
                }
            }

            Statement::LifetimeEnd(lifetime_end) => {
                // Explicit destruction is a use
                let chain = self.chains.entry(lifetime_end.variable_name.clone()).or_insert_with(|| {
                    DefUseChain::new(lifetime_end.variable_name.clone())
                });

                chain.add_use(Use {
                    var_name: lifetime_end.variable_name.clone(),
                    node: node_index,
                    span: lifetime_end.span,
                    use_type: UseType::Read,
                });
            }

            Statement::If(if_stmt) => {
                // Analyze the condition expression
                self.analyze_expr(&if_stmt.condition, node_index);

                // Analyze then block
                self.scope_depth += 1;
                for stmt in &if_stmt.then_block.statements {
                    self.analyze_statement(stmt, node_index);
                }
                self.scope_depth -= 1;

                // Analyze else-if branches
                for branch in &if_stmt.else_if_branches {
                    self.analyze_expr(&branch.condition, node_index);
                    self.scope_depth += 1;
                    for stmt in &branch.block.statements {
                        self.analyze_statement(stmt, node_index);
                    }
                    self.scope_depth -= 1;
                }

                // Analyze else block
                if let Some(ref else_block) = if_stmt.else_block {
                    self.scope_depth += 1;
                    for stmt in &else_block.statements {
                        self.analyze_statement(stmt, node_index);
                    }
                    self.scope_depth -= 1;
                }
            }

            Statement::Loop(loop_stmt) => {
                // Analyze the loop condition (while loops)
                if let Some(ref condition) = loop_stmt.condition {
                    self.analyze_expr(condition, node_index);
                }
                // Analyze the iterable expression (for-each loops)
                if let Some(ref iterable) = loop_stmt.iterable {
                    self.analyze_expr(iterable, node_index);
                }

                self.scope_depth += 1;

                // Note: Iterator variables are NOT tracked for automatic destruction.
                // They have special loop-bound lifetime managed by the interpreter.
                // We mark them as ambiguous by setting is_underscore=true so they're
                // excluded from destruction schedules.
                if let Some(ref iter_var) = loop_stmt.iterator_var {
                    let chain = self.chains.entry(iter_var.clone()).or_insert_with(|| {
                        DefUseChain::new(iter_var.clone())
                    });

                    // Mark as ambiguous (loop variant)
                    chain.is_ambiguous = true;
                    chain.ambiguity = Some(AmbiguousLifetime {
                        variable: iter_var.clone(),
                        reason: AmbiguityReason::LoopVariant,
                        nodes: HashSet::new(),
                        suggested_span: loop_stmt.body.span,
                    });
                }

                // Analyze loop body
                for stmt in &loop_stmt.body.statements {
                    self.analyze_statement(stmt, node_index);
                }

                self.scope_depth -= 1;
            }

            Statement::Match(match_stmt) => {
                // Match expression has scrutinee
                self.analyze_expr(&match_stmt.scrutinee, node_index);

                // Analyze match cases
                for case in &match_stmt.cases {
                    // Analyze pattern guards
                    self.analyze_pattern(&case.pattern, node_index);

                    // Analyze case value
                    if let Some(ref value) = case.value {
                        self.analyze_expr(value, node_index);
                    }

                    // Analyze case block
                    if let Some(ref block) = case.block {
                        self.scope_depth += 1;
                        for stmt in &block.statements {
                            self.analyze_statement(stmt, node_index);
                        }
                        self.scope_depth -= 1;
                    }
                }
            }

            Statement::Try(try_stmt) => {
                // Analyze try block statements
                for stmt in &try_stmt.try_block.statements {
                    self.analyze_statement(stmt, node_index);
                }
                // Analyze catch clauses
                for catch in &try_stmt.catch_clauses {
                    for stmt in &catch.block.statements {
                        self.analyze_statement(stmt, node_index);
                    }
                }
                // Analyze finally clause
                if let Some(ref finally) = try_stmt.finally_clause {
                    for stmt in &finally.block.statements {
                        self.analyze_statement(stmt, node_index);
                    }
                }
            }

            Statement::Break(_) | Statement::Continue(_) | Statement::Newline(_) |
            Statement::FunctionDecl(_) | Statement::CliArgsCapture(_) |
            Statement::SetNumeralMode { .. } => {
                // No variable uses in these statements
            }
        }
    }

    /// Analyze an expression for variable uses
    fn analyze_expr(&mut self, expr: &Expr, node_index: usize) {
        match expr {
            Expr::Identifier(ident) => {
                // This is a read
                let chain = self.chains.entry(ident.name.clone()).or_insert_with(|| {
                    DefUseChain::new(ident.name.clone())
                });

                chain.add_use(Use {
                    var_name: ident.name.clone(),
                    node: node_index,
                    span: ident.span,
                    use_type: UseType::Read,
                });
            }

            Expr::Binary(binary) => {
                self.analyze_expr(&binary.left, node_index);
                self.analyze_expr(&binary.right, node_index);
            }

            Expr::Unary(unary) => {
                self.analyze_expr(&unary.operand, node_index);
            }

            Expr::FunctionCall(call) => {
                for arg in &call.arguments {
                    self.analyze_expr(arg, node_index);
                }
            }

            Expr::ArrayLiteral(array) => {
                for elem in &array.elements {
                    self.analyze_expr(elem, node_index);
                }
            }

            Expr::Tuple(tuple) => {
                for elem in &tuple.elements {
                    self.analyze_expr(elem, node_index);
                }
            }

            Expr::NamedTuple(named_tuple) => {
                for (_name, expr) in &named_tuple.fields {
                    self.analyze_expr(expr, node_index);
                }
            }

            Expr::Index(index) => {
                // Both array and index can have variable uses
                self.analyze_expr(&index.array, node_index);
                self.analyze_expr(&index.index, node_index);
            }

            Expr::Range(range) => {
                // Start, end, and optional step can have variable uses
                self.analyze_expr(&range.start, node_index);
                self.analyze_expr(&range.end, node_index);
                if let Some(ref step) = range.step {
                    self.analyze_expr(step, node_index);
                }
            }

            Expr::MemberAccess(member) => {
                // The object can be a variable
                self.analyze_expr(&member.object, node_index);
            }

            Expr::Match(match_expr) => {
                // Scrutinee can have variables
                self.analyze_expr(&match_expr.scrutinee, node_index);

                // Match cases values and blocks need analysis
                for case in &match_expr.cases {
                    // Analyze pattern guards
                    self.analyze_pattern(&case.pattern, node_index);

                    if let Some(ref value_expr) = case.value {
                        self.analyze_expr(value_expr, node_index);
                    }

                    if let Some(ref block) = case.block {
                        self.scope_depth += 1;
                        for stmt in &block.statements {
                            self.analyze_statement(stmt, node_index);
                        }
                        self.scope_depth -= 1;
                    }
                }
            }

            Expr::Lambda(lambda) => {
                // Lambda body can reference outer variables
                // Note: Lambda parameters are in isolated scope, not tracked here
                match &lambda.body {
                    zymbol_ast::LambdaBody::Expr(expr) => {
                        self.analyze_expr(expr, node_index);
                    }
                    zymbol_ast::LambdaBody::Block(block) => {
                        self.scope_depth += 1;
                        for stmt in &block.statements {
                            self.analyze_statement(stmt, node_index);
                        }
                        self.scope_depth -= 1;
                    }
                }
            }

            // Collection operations
            Expr::CollectionLength(col_len) => {
                self.analyze_expr(&col_len.collection, node_index);
            }

            Expr::CollectionAppend(col_append) => {
                self.analyze_expr(&col_append.collection, node_index);
                self.analyze_expr(&col_append.element, node_index);
            }

            Expr::CollectionInsert(op) => {
                self.analyze_expr(&op.collection, node_index);
                self.analyze_expr(&op.index, node_index);
                self.analyze_expr(&op.element, node_index);
            }

            Expr::CollectionRemoveValue(op) => {
                self.analyze_expr(&op.collection, node_index);
                self.analyze_expr(&op.value, node_index);
            }

            Expr::CollectionRemoveAll(op) => {
                self.analyze_expr(&op.collection, node_index);
                self.analyze_expr(&op.value, node_index);
            }

            Expr::CollectionRemoveAt(op) => {
                self.analyze_expr(&op.collection, node_index);
                self.analyze_expr(&op.index, node_index);
            }

            Expr::CollectionRemoveRange(op) => {
                self.analyze_expr(&op.collection, node_index);
                if let Some(start) = &op.start {
                    self.analyze_expr(start, node_index);
                }
                if let Some(end) = &op.end {
                    self.analyze_expr(end, node_index);
                }
            }

            Expr::CollectionContains(col_contains) => {
                self.analyze_expr(&col_contains.collection, node_index);
                self.analyze_expr(&col_contains.element, node_index);
            }

            Expr::CollectionUpdate(col_update) => {
                // target is an IndexExpr containing collection and index
                self.analyze_expr(&col_update.target, node_index);
                self.analyze_expr(&col_update.value, node_index);
            }

            Expr::CollectionSlice(col_slice) => {
                self.analyze_expr(&col_slice.collection, node_index);
                if let Some(start) = &col_slice.start {
                    self.analyze_expr(start, node_index);
                }
                if let Some(end) = &col_slice.end {
                    self.analyze_expr(end, node_index);
                }
            }

            Expr::CollectionMap(col_map) => {
                self.analyze_expr(&col_map.collection, node_index);
                self.analyze_expr(&col_map.lambda, node_index);
            }

            Expr::CollectionFilter(col_filter) => {
                self.analyze_expr(&col_filter.collection, node_index);
                self.analyze_expr(&col_filter.lambda, node_index);
            }

            Expr::CollectionReduce(col_reduce) => {
                self.analyze_expr(&col_reduce.collection, node_index);
                self.analyze_expr(&col_reduce.initial, node_index);
                self.analyze_expr(&col_reduce.lambda, node_index);
            }

            Expr::CollectionSortAsc(op) | Expr::CollectionSortDesc(op) | Expr::CollectionSortCustom(op) => {
                self.analyze_expr(&op.collection, node_index);
                if let Some(ref cmp) = op.comparator {
                    self.analyze_expr(cmp, node_index);
                }
            }

            Expr::CollectionFindAll(op) => {
                self.analyze_expr(&op.collection, node_index);
                self.analyze_expr(&op.value, node_index);
            }

            // String operations
            Expr::StringReplace(str_replace) => {
                self.analyze_expr(&str_replace.string, node_index);
                self.analyze_expr(&str_replace.pattern, node_index);
                self.analyze_expr(&str_replace.replacement, node_index);
                if let Some(count) = &str_replace.count {
                    self.analyze_expr(count, node_index);
                }
            }

            Expr::StringSplit(op) => {
                self.analyze_expr(&op.string, node_index);
                self.analyze_expr(&op.delimiter, node_index);
            }

            Expr::ConcatBuild(op) => {
                self.analyze_expr(&op.base, node_index);
                for item in &op.items { self.analyze_expr(item, node_index); }
            }

            Expr::NumericCast(op) => self.analyze_expr(&op.expr, node_index),

            // Data operations
            Expr::NumericEval(num_eval) => {
                self.analyze_expr(&num_eval.expr, node_index);
            }

            Expr::TypeMetadata(type_meta) => {
                self.analyze_expr(&type_meta.expr, node_index);
            }

            Expr::Format(format) => {
                self.analyze_expr(&format.expr, node_index);
            }

            Expr::BaseConversion(base_conv) => {
                self.analyze_expr(&base_conv.expr, node_index);
            }

            Expr::Round(round) => {
                self.analyze_expr(&round.expr, node_index);
            }

            Expr::Trunc(trunc) => {
                self.analyze_expr(&trunc.expr, node_index);
            }

            Expr::Pipe(pipe) => {
                // Left value and callable can have variables
                self.analyze_expr(&pipe.left, node_index);
                self.analyze_expr(&pipe.callable, node_index);
                // Arguments can have variables (but not placeholders)
                for arg in &pipe.arguments {
                    if let zymbol_ast::PipeArg::Expr(expr) = arg {
                        self.analyze_expr(expr, node_index);
                    }
                }
            }

            Expr::Execute(_execute) => {
                // Execute just has a path string - no variable uses
            }

            Expr::BashExec(bash_exec) => {
                for arg in &bash_exec.args {
                    self.analyze_expr(arg, node_index);
                }
            }

            Expr::ErrorCheck(check) => {
                // Analyze the inner expression
                self.analyze_expr(&check.expr, node_index);
            }

            Expr::ErrorPropagate(prop) => {
                // Analyze the inner expression
                self.analyze_expr(&prop.expr, node_index);
            }

            Expr::DeepIndex(di) => {
                self.analyze_expr(&di.array, node_index);
                for step in &di.path.steps {
                    self.analyze_expr(&step.index, node_index);
                    if let Some(end) = &step.range_end { self.analyze_expr(end, node_index); }
                }
            }
            Expr::FlatExtract(fe) => {
                self.analyze_expr(&fe.array, node_index);
                for path in &fe.paths {
                    for step in &path.steps {
                        self.analyze_expr(&step.index, node_index);
                        if let Some(end) = &step.range_end { self.analyze_expr(end, node_index); }
                    }
                }
            }
            Expr::StructuredExtract(se) => {
                self.analyze_expr(&se.array, node_index);
                for group in &se.groups {
                    for path in &group.paths {
                        for step in &path.steps {
                            self.analyze_expr(&step.index, node_index);
                            if let Some(end) = &step.range_end { self.analyze_expr(end, node_index); }
                        }
                    }
                }
            }

            // Literals and other leaf nodes - no variable uses
            Expr::Literal(_) => {}
        }
    }

    /// Analyze pattern for variable uses
    fn analyze_pattern(&mut self, pattern: &zymbol_ast::Pattern, node_index: usize) {
        use zymbol_ast::Pattern;

        match pattern {
            Pattern::Literal(_, _) | Pattern::Wildcard(_) => {}
            Pattern::Range(start, end, _) => {
                self.analyze_expr(start, node_index);
                self.analyze_expr(end, node_index);
            }
            Pattern::List(patterns, _) => {
                for p in patterns {
                    self.analyze_pattern(p, node_index);
                }
            }
            Pattern::Comparison(_, expr, _) => {
                self.analyze_expr(expr, node_index);
            }
            Pattern::Ident(name, span) => {
                let chain = self.chains.entry(name.clone()).or_insert_with(|| {
                    DefUseChain::new(name.clone())
                });
                chain.add_use(Use {
                    var_name: name.clone(),
                    node: node_index,
                    span: *span,
                    use_type: UseType::Read,
                });
            }
        }
    }

    /// Get the def-use chain for a variable
    pub fn get_chain(&self, var_name: &str) -> Option<&DefUseChain> {
        self.chains.get(var_name)
    }

    /// Get all ambiguous variables
    pub fn get_ambiguous_variables(&self) -> Vec<&DefUseChain> {
        self.chains.values().filter(|c| c.is_ambiguous).collect()
    }
}

impl Default for DefUseAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zymbol_ast::{Assignment, LiteralExpr, IdentifierExpr};
    use zymbol_common::Literal;
    use zymbol_span::{FileId, Position};

    fn dummy_span() -> Span {
        Span {
            file_id: FileId(0),
            start: Position { line: 1, column: 1, byte_offset: 0 },
            end: Position { line: 1, column: 1, byte_offset: 0 },
        }
    }

    #[test]
    fn test_simple_def_use() {
        let stmts = vec![
            Statement::Assignment(Assignment::new(
                "x".to_string(),
                Expr::Literal(LiteralExpr::new(Literal::Int(42), dummy_span())),
                dummy_span(),
            )),
            Statement::Assignment(Assignment::new(
                "y".to_string(),
                Expr::Identifier(IdentifierExpr::new("x".to_string(), dummy_span())),
                dummy_span(),
            )),
        ];

        let cfg = ControlFlowGraph::build_sequential(&stmts);
        let mut analyzer = DefUseAnalyzer::new();
        let chains = analyzer.analyze(&stmts, &cfg);

        // x should have 1 definition and 1 use
        let x_chain = chains.get("x").unwrap();
        assert_eq!(x_chain.definitions.len(), 1);
        assert_eq!(x_chain.uses.len(), 1);
        assert_eq!(x_chain.last_uses.len(), 1); // Last use at node 1

        // y should have 1 definition and 0 uses
        let y_chain = chains.get("y").unwrap();
        assert_eq!(y_chain.definitions.len(), 1);
        assert_eq!(y_chain.uses.len(), 0); // Unused variable
    }

    #[test]
    fn test_reassignment() {
        let stmts = vec![
            Statement::Assignment(Assignment::new(
                "x".to_string(),
                Expr::Literal(LiteralExpr::new(Literal::Int(1), dummy_span())),
                dummy_span(),
            )),
            Statement::Assignment(Assignment::new(
                "x".to_string(),
                Expr::Literal(LiteralExpr::new(Literal::Int(2), dummy_span())),
                dummy_span(),
            )),
        ];

        let cfg = ControlFlowGraph::build_sequential(&stmts);
        let mut analyzer = DefUseAnalyzer::new();
        let chains = analyzer.analyze(&stmts, &cfg);

        let x_chain = chains.get("x").unwrap();
        assert_eq!(x_chain.definitions.len(), 1); // Initial definition
        assert_eq!(x_chain.uses.len(), 1); // Reassignment counts as use
    }
}
