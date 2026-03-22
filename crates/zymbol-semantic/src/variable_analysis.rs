//! Variable Liveness Analysis for Zymbol-Lang
//!
//! Analyzes variable usage to detect:
//! - Variables declared but never used
//! - Variables that could be optimized away
//! - Dead code related to variable assignments
//! - Invalid access to underscore variables (_variable)

use std::collections::{HashMap, HashSet};
use zymbol_ast::{Block, Expr, Program, Statement};
use zymbol_span::Span;
use zymbol_error::Diagnostic;

/// Scope identifier
type ScopeId = usize;

/// Scope node in the scope tree
#[derive(Debug, Clone)]
struct ScopeNode {
    id: ScopeId,
    parent: Option<ScopeId>,
    /// Underscore variables declared in THIS scope (name -> declaration span)
    underscore_vars: HashMap<String, Span>,
    children: Vec<ScopeId>,
}

impl ScopeNode {
    fn new(id: ScopeId, parent: Option<ScopeId>) -> Self {
        Self {
            id,
            parent,
            underscore_vars: HashMap::new(),
            children: Vec::new(),
        }
    }
}

/// Scope tree for tracking block-local _variables
#[derive(Debug)]
struct ScopeTree {
    scopes: Vec<ScopeNode>,
    current: ScopeId,
}

impl ScopeTree {
    fn new() -> Self {
        let root = ScopeNode::new(0, None);
        Self {
            scopes: vec![root],
            current: 0,
        }
    }

    /// Enter a new scope (create child of current scope)
    fn enter_scope(&mut self) -> ScopeId {
        let new_id = self.scopes.len();
        let parent_id = self.current;

        let new_scope = ScopeNode::new(new_id, Some(parent_id));
        self.scopes.push(new_scope);

        // Add as child to parent
        self.scopes[parent_id].children.push(new_id);

        // Make this the current scope
        self.current = new_id;
        new_id
    }

    /// Exit current scope (return to parent)
    fn exit_scope(&mut self) {
        if let Some(parent_id) = self.scopes[self.current].parent {
            self.current = parent_id;
        }
    }

    /// Declare an underscore variable in current scope
    fn declare_underscore_var(&mut self, name: String, span: Span) {
        self.scopes[self.current].underscore_vars.insert(name, span);
    }

    /// Check if an underscore variable can be accessed from current scope
    /// Returns Ok(()) if valid, Err(Diagnostic) if invalid
    fn validate_underscore_access(&self, name: &str, usage_span: &Span) -> Result<(), Diagnostic> {
        // Check if this _variable is declared in the CURRENT scope
        if self.scopes[self.current].underscore_vars.contains_key(name) {
            return Ok(()); // Valid - accessing in declaration scope
        }

        // Search for where this _variable is declared
        let declared_scope = self.find_underscore_var_scope(name);

        if let Some(decl_scope_id) = declared_scope {
            let decl_scope = &self.scopes[decl_scope_id];
            let decl_span = &decl_scope.underscore_vars[name];

            // Check if it's in a parent scope (accessing from inner scope)
            if self.is_ancestor(decl_scope_id, self.current) {
                return Err(Diagnostic::error(format!(
                    "cannot access underscore variable '{}' from inner scope",
                    name
                ))
                .with_span(*usage_span)
                .with_note(format!(
                    "'{}' was declared at {}:{}",
                    name, decl_span.start.line, decl_span.start.column
                ))
                .with_help(format!(
                    "underscore variables are strictly local to their declaration block\n\
                     '{}' was declared in an outer scope and cannot be accessed from nested blocks",
                    name
                )));
            }

            // Check if it's in a child scope (accessing from outer scope)
            if self.is_ancestor(self.current, decl_scope_id) {
                return Err(Diagnostic::error(format!(
                    "cannot access underscore variable '{}' from outer scope",
                    name
                ))
                .with_span(*usage_span)
                .with_note(format!(
                    "'{}' was declared at {}:{}",
                    name, decl_span.start.line, decl_span.start.column
                ))
                .with_help(format!(
                    "underscore variables are strictly local to their declaration block\n\
                     '{}' was declared in an inner scope and is not accessible here",
                    name
                )));
            }

            // If neither ancestor relationship exists, it's in a sibling/unrelated scope
            // This is OK - sibling scopes can have independent _variables with same names
            // Only return error if there's a parent-child relationship
        }

        // Variable not found anywhere OR it's in unrelated scope - OK
        // (undefined variable will be caught by normal checks if needed)
        Ok(())
    }

    /// Find which scope (if any) declares this _variable
    fn find_underscore_var_scope(&self, name: &str) -> Option<ScopeId> {
        for scope in &self.scopes {
            if scope.underscore_vars.contains_key(name) {
                return Some(scope.id);
            }
        }
        None
    }

    /// Check if ancestor_id is an ancestor of descendant_id
    fn is_ancestor(&self, ancestor_id: ScopeId, descendant_id: ScopeId) -> bool {
        if ancestor_id == descendant_id {
            return false; // Same scope, not ancestor
        }

        let mut current = descendant_id;
        while let Some(parent_id) = self.scopes[current].parent {
            if parent_id == ancestor_id {
                return true;
            }
            current = parent_id;
        }
        false
    }
}

/// Variable usage information
#[derive(Debug, Clone)]
pub struct VariableInfo {
    /// Name of the variable
    pub name: String,
    /// Span where the variable was declared
    pub declaration_span: Span,
    /// Spans where the variable is read/used
    pub usage_spans: Vec<Span>,
    /// Spans where the variable is assigned (after initial declaration)
    pub assignment_spans: Vec<Span>,
    /// Is this a constant (declared with :=)?
    pub is_const: bool,
    /// Scope depth where this variable was declared
    pub scope_depth: usize,
}

impl VariableInfo {
    fn new(name: String, declaration_span: Span, is_const: bool, scope_depth: usize) -> Self {
        Self {
            name,
            declaration_span,
            usage_spans: Vec::new(),
            assignment_spans: Vec::new(),
            is_const,
            scope_depth,
        }
    }

    /// Check if the variable is ever used (read, not just assigned)
    pub fn is_used(&self) -> bool {
        !self.usage_spans.is_empty()
    }

    /// Check if variable is only declared but never used
    pub fn is_unused(&self) -> bool {
        self.usage_spans.is_empty() && self.assignment_spans.is_empty()
    }

    /// Check if variable is assigned but never read
    pub fn is_write_only(&self) -> bool {
        self.usage_spans.is_empty() && !self.assignment_spans.is_empty()
    }
}

/// Severity level for diagnostics
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Warning,
    Info,
}

/// Variable analysis diagnostic
#[derive(Debug, Clone)]
pub struct VariableDiagnostic {
    pub severity: Severity,
    pub message: String,
    pub span: Span,
    pub help: Option<String>,
}

/// Variable liveness analyzer
pub struct VariableAnalyzer {
    /// Current scope depth (0 = global)
    scope_depth: usize,
    /// Variables in current analysis scope
    /// Key: variable name, Value: variable info
    variables: HashMap<String, VariableInfo>,
    /// Set of variable names currently in scope (for shadowing detection)
    current_scope_vars: Vec<HashSet<String>>,
    /// Collected diagnostics
    diagnostics: Vec<VariableDiagnostic>,
    /// Scope tree for tracking block-local _variables
    scope_tree: ScopeTree,
    /// Semantic errors (for _variable violations)
    semantic_errors: Vec<Diagnostic>,
}

impl VariableAnalyzer {
    pub fn new() -> Self {
        Self {
            scope_depth: 0,
            variables: HashMap::new(),
            current_scope_vars: vec![HashSet::new()],
            diagnostics: Vec::new(),
            scope_tree: ScopeTree::new(),
            semantic_errors: Vec::new(),
        }
    }

    /// Get semantic errors collected during analysis
    pub fn semantic_errors(&self) -> &[Diagnostic] {
        &self.semantic_errors
    }

    /// Analyze a program and return diagnostics
    pub fn analyze(&mut self, program: &Program) -> Vec<VariableDiagnostic> {
        self.diagnostics.clear();
        self.variables.clear();
        self.scope_depth = 0;
        self.current_scope_vars = vec![HashSet::new()];
        self.scope_tree = ScopeTree::new();
        self.semantic_errors.clear();

        // Analyze all statements in the program
        for statement in &program.statements {
            self.analyze_statement(statement);
        }

        // Generate diagnostics for unused variables
        self.generate_diagnostics();

        // Return collected diagnostics
        self.diagnostics.clone()
    }

    /// Enter a new scope
    fn enter_scope(&mut self) {
        self.scope_depth += 1;
        self.current_scope_vars.push(HashSet::new());
        self.scope_tree.enter_scope();
    }

    /// Exit current scope
    fn exit_scope(&mut self) {
        if self.scope_depth > 0 {
            self.current_scope_vars.pop();
            self.scope_depth -= 1;
            self.scope_tree.exit_scope();
        }
    }

    /// Record a variable declaration
    fn declare_variable(&mut self, name: String, span: Span, is_const: bool) {
        // Underscore variables: register in scope tree for strict scoping
        if name.starts_with('_') {
            self.scope_tree.declare_underscore_var(name.clone(), span);
            // Still track for unused variable warnings (but won't generate them)
            let info = VariableInfo::new(name.clone(), span, is_const, self.scope_depth);
            self.variables.insert(name.clone(), info);
            return;
        }

        // Record the variable
        let info = VariableInfo::new(name.clone(), span, is_const, self.scope_depth);
        self.variables.insert(name.clone(), info);

        // Add to current scope set
        if let Some(scope_set) = self.current_scope_vars.last_mut() {
            scope_set.insert(name);
        }
    }

    /// Record a variable usage (read)
    fn use_variable(&mut self, name: &str, span: Span) {
        // Skip underscore placeholders (but not _variables)
        if name == "_" {
            return;
        }

        // Validate underscore variable access
        if name.starts_with('_') {
            if let Err(diagnostic) = self.scope_tree.validate_underscore_access(name, &span) {
                self.semantic_errors.push(diagnostic);
                return; // Don't track usage if invalid
            }
        }

        if let Some(info) = self.variables.get_mut(name) {
            info.usage_spans.push(span);
        }
    }

    /// Record a variable assignment (after initial declaration)
    fn assign_variable(&mut self, name: &str, span: Span) {
        // Validate underscore variable access
        if name.starts_with('_') {
            if let Err(diagnostic) = self.scope_tree.validate_underscore_access(name, &span) {
                self.semantic_errors.push(diagnostic);
                return; // Don't track assignment if invalid
            }
        }

        if let Some(info) = self.variables.get_mut(name) {
            info.assignment_spans.push(span);
        }
    }

    /// Analyze a statement
    fn analyze_statement(&mut self, stmt: &Statement) {
        match stmt {
            Statement::Assignment(assignment) => {
                // First analyze the value expression (to track variable usage)
                self.analyze_expr(&assignment.value);

                // Then record the assignment
                if !self.variables.contains_key(&assignment.name) {
                    // First assignment - declaration
                    self.declare_variable(
                        assignment.name.clone(),
                        assignment.span,
                        false,
                    );
                } else {
                    // Reassignment
                    self.assign_variable(&assignment.name, assignment.span);
                }
            }

            Statement::ConstDecl(const_decl) => {
                // Analyze value expression first
                self.analyze_expr(&const_decl.value);

                // Then record constant declaration
                self.declare_variable(
                    const_decl.name.clone(),
                    const_decl.span,
                    true,
                );
            }

            Statement::Output(output) => {
                // Analyze all output expressions
                for expr in &output.exprs {
                    self.analyze_expr(expr);
                }
            }

            Statement::Input(input) => {
                // Input creates a variable if it doesn't exist
                if !self.variables.contains_key(&input.variable) {
                    self.declare_variable(
                        input.variable.clone(),
                        input.span,
                        false,
                    );
                }
                // InputPrompt is not an Expr, so we skip analyzing it
            }

            Statement::If(if_stmt) => {
                // Analyze condition
                self.analyze_expr(&if_stmt.condition);

                // Analyze then block
                self.analyze_block(&if_stmt.then_block);

                // Analyze else-if branches
                for else_if in &if_stmt.else_if_branches {
                    self.analyze_expr(&else_if.condition);
                    self.analyze_block(&else_if.block);
                }

                // Analyze else block
                if let Some(else_block) = &if_stmt.else_block {
                    self.analyze_block(else_block);
                }
            }

            Statement::Loop(loop_stmt) => {
                // Analyze loop condition for while loops
                if let Some(condition) = &loop_stmt.condition {
                    self.analyze_expr(condition);
                }

                // Analyze iterable for for-each loops
                if let Some(iterable) = &loop_stmt.iterable {
                    self.analyze_expr(iterable);
                }

                // Enter loop body scope first
                self.enter_scope();

                // If it's a for-each loop, the iterator variable is declared INSIDE the loop scope
                if let Some(iterator_var) = &loop_stmt.iterator_var {
                    self.declare_variable(
                        iterator_var.clone(),
                        loop_stmt.span,
                        false,
                    );
                }

                // Analyze loop body statements (don't use analyze_block to avoid double scope entry)
                for statement in &loop_stmt.body.statements {
                    self.analyze_statement(statement);
                }

                // Exit loop body scope
                self.exit_scope();
            }

            Statement::FunctionDecl(func) => {
                self.enter_scope();

                // Function parameters are considered "used" automatically
                for param in &func.parameters {
                    let param_name = param.name.clone();

                    // Declare parameter and mark as used
                    self.declare_variable(
                        param_name.clone(),
                        param.span,
                        false,
                    );
                    // Mark as used immediately (parameters are always "used")
                    self.use_variable(&param_name, param.span);
                }

                // Analyze function body
                self.analyze_block(&func.body);

                self.exit_scope();
            }

            Statement::Return(ret) => {
                if let Some(value) = &ret.value {
                    self.analyze_expr(value);
                }
            }

            Statement::Match(match_stmt) => {
                // Analyze scrutinee
                self.analyze_expr(&match_stmt.scrutinee);

                // Analyze each case
                for case in &match_stmt.cases {
                    // Pattern might have variable bindings (we skip pattern analysis for now)

                    // Analyze value expression if present
                    if let Some(value) = &case.value {
                        self.analyze_expr(value);
                    }

                    // Analyze side effect block if present
                    if let Some(block) = &case.block {
                        self.analyze_block(block);
                    }
                }
            }

            Statement::Break(_) | Statement::Continue(_) | Statement::Newline(_) => {
                // No variables involved
            }

            Statement::Expr(expr_stmt) => {
                self.analyze_expr(&expr_stmt.expr);
            }

            Statement::CliArgsCapture(capture) => {
                // CLI args capture creates a variable
                if !self.variables.contains_key(&capture.variable_name) {
                    self.declare_variable(
                        capture.variable_name.clone(),
                        capture.span,
                        false,
                    );
                }
            }

            Statement::LifetimeEnd(lifetime_end) => {
                // Lifetime end marks explicit destruction of a variable
                // For now, just mark it as used (Phase 6 will add full semantics)
                self.use_variable(&lifetime_end.variable_name, lifetime_end.span);
            }

            Statement::Try(try_stmt) => {
                // Analyze try block
                self.analyze_block(&try_stmt.try_block);

                // Analyze catch clauses
                for catch in &try_stmt.catch_clauses {
                    self.enter_scope();
                    // _err is implicitly defined in catch blocks
                    self.declare_variable("_err".to_string(), catch.span, false);
                    self.analyze_block(&catch.block);
                    self.exit_scope();
                }

                // Analyze finally clause
                if let Some(ref finally) = try_stmt.finally_clause {
                    self.analyze_block(&finally.block);
                }
            }
        }
    }

    /// Analyze a block of statements
    fn analyze_block(&mut self, block: &Block) {
        self.enter_scope();

        for statement in &block.statements {
            self.analyze_statement(statement);
        }

        self.exit_scope();
    }

    /// Analyze an expression to find variable usages
    fn analyze_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Identifier(ident) => {
                // This is a variable usage
                self.use_variable(&ident.name, ident.span);
            }

            Expr::Binary(binary) => {
                self.analyze_expr(&binary.left);
                self.analyze_expr(&binary.right);
            }

            Expr::Unary(unary) => {
                self.analyze_expr(&unary.operand);
            }

            Expr::FunctionCall(call) => {
                self.analyze_expr(&call.callable);
                for arg in &call.arguments {
                    self.analyze_expr(arg);
                }
            }

            Expr::Lambda(lambda) => {
                self.enter_scope();

                // Lambda parameters are considered used
                for param in &lambda.params {
                    self.declare_variable(param.clone(), lambda.span, false);
                    self.use_variable(param, lambda.span);
                }

                // Analyze lambda body
                match &lambda.body {
                    zymbol_ast::LambdaBody::Expr(expr) => {
                        self.analyze_expr(expr);
                    }
                    zymbol_ast::LambdaBody::Block(block) => {
                        self.analyze_block(block);
                    }
                }

                self.exit_scope();
            }

            Expr::Match(match_expr) => {
                // Analyze scrutinee
                self.analyze_expr(&match_expr.scrutinee);

                // Analyze each case
                for case in &match_expr.cases {
                    // Pattern might have variable bindings (skip for now)

                    // Analyze value expression if present
                    if let Some(value) = &case.value {
                        self.analyze_expr(value);
                    }

                    // Analyze side effect block if present
                    if let Some(block) = &case.block {
                        self.analyze_block(block);
                    }
                }
            }

            Expr::Tuple(tuple) => {
                for element in &tuple.elements {
                    self.analyze_expr(element);
                }
            }

            Expr::NamedTuple(named_tuple) => {
                for (_field_name, value) in &named_tuple.fields {
                    self.analyze_expr(value);
                }
            }

            Expr::ArrayLiteral(array) => {
                for element in &array.elements {
                    self.analyze_expr(element);
                }
            }

            Expr::Index(index) => {
                self.analyze_expr(&index.array);
                self.analyze_expr(&index.index);
            }

            Expr::CollectionSlice(slice) => {
                self.analyze_expr(&slice.collection);
                if let Some(start) = &slice.start {
                    self.analyze_expr(start);
                }
                if let Some(end) = &slice.end {
                    self.analyze_expr(end);
                }
            }

            Expr::MemberAccess(member) => {
                self.analyze_expr(&member.object);
            }

            Expr::Pipe(pipe) => {
                self.analyze_expr(&pipe.left);
                self.analyze_expr(&pipe.callable);
                for arg in &pipe.arguments {
                    match arg {
                        zymbol_ast::PipeArg::Placeholder => {
                            // Placeholder doesn't use variables
                        }
                        zymbol_ast::PipeArg::Expr(expr) => {
                            self.analyze_expr(expr);
                        }
                    }
                }
            }

            // Collection operations
            Expr::CollectionLength(op) => {
                self.analyze_expr(&op.collection);
            }

            Expr::CollectionAppend(op) => {
                self.analyze_expr(&op.collection);
            }

            Expr::CollectionRemove(op) => {
                self.analyze_expr(&op.collection);
            }

            Expr::CollectionContains(op) => {
                self.analyze_expr(&op.collection);
                self.analyze_expr(&op.element);
            }

            Expr::CollectionUpdate(op) => {
                self.analyze_expr(&op.target);
                self.analyze_expr(&op.value);
            }

            Expr::CollectionMap(op) => {
                self.analyze_expr(&op.collection);
                self.analyze_expr(&op.lambda);
            }

            Expr::CollectionFilter(op) => {
                self.analyze_expr(&op.collection);
                self.analyze_expr(&op.lambda);
            }

            Expr::CollectionReduce(op) => {
                self.analyze_expr(&op.collection);
                self.analyze_expr(&op.initial);
                self.analyze_expr(&op.lambda);
            }

            // String operations - analyze component expressions
            Expr::StringFindPositions(op) => {
                self.analyze_expr(&op.string);
                self.analyze_expr(&op.pattern);
            }

            Expr::StringInsert(op) => {
                self.analyze_expr(&op.string);
                self.analyze_expr(&op.position);
                self.analyze_expr(&op.text);
            }

            Expr::StringRemove(op) => {
                self.analyze_expr(&op.string);
                self.analyze_expr(&op.position);
                self.analyze_expr(&op.count);
            }

            Expr::StringReplace(op) => {
                self.analyze_expr(&op.string);
                self.analyze_expr(&op.pattern);
                self.analyze_expr(&op.replacement);
                if let Some(count) = &op.count {
                    self.analyze_expr(count);
                }
            }

            // Numeric/Type operations
            Expr::NumericEval(op) => {
                self.analyze_expr(&op.expr);
            }

            Expr::TypeMetadata(op) => {
                self.analyze_expr(&op.expr);
            }

            // Base conversions
            Expr::BaseConversion(op) => {
                self.analyze_expr(&op.expr);
            }

            // Format expressions
            Expr::Format(op) => {
                self.analyze_expr(&op.expr);
            }

            // Precision expressions
            Expr::Round(op) => {
                self.analyze_expr(&op.expr);
            }

            Expr::Trunc(op) => {
                self.analyze_expr(&op.expr);
            }

            // Literals don't use variables
            Expr::Literal(_) => {
                // Literals are constant values - no variable usage
                // In the future, we could analyze string interpolation {var} syntax
            }

            Expr::Range(_) => {
                // Range expressions: start..end
                // We could analyze start/end if they were Expr, but they're literals/identifiers only
            }

            // Execute expressions
            Expr::Execute(_) => {
                // Skip for now - these might have variable interpolation
            }

            Expr::BashExec(_) => {
                // Skip for now - these might have variable interpolation
            }

            // Error handling expressions
            Expr::ErrorCheck(check) => {
                self.analyze_expr(&check.expr);
            }

            Expr::ErrorPropagate(prop) => {
                self.analyze_expr(&prop.expr);
            }
        }
    }

    /// Generate diagnostics for all unused variables
    fn generate_diagnostics(&mut self) {
        let mut vars: Vec<&VariableInfo> = self.variables.values().collect();
        vars.sort_by_key(|v| (v.declaration_span.start.line, v.declaration_span.start.column));
        for var_info in vars {
            // Skip underscore-prefixed variables (intentionally unused)
            if var_info.name.starts_with('_') {
                continue;
            }

            if var_info.is_unused() {
                self.diagnostics.push(VariableDiagnostic {
                    severity: Severity::Warning,
                    message: format!("unused variable '{}'", var_info.name),
                    span: var_info.declaration_span,
                    help: Some(
                        "consider removing this variable or prefixing with '_' if intentionally unused".to_string()
                    ),
                });
            } else if var_info.is_write_only() {
                self.diagnostics.push(VariableDiagnostic {
                    severity: Severity::Warning,
                    message: format!(
                        "variable '{}' is assigned but never read",
                        var_info.name
                    ),
                    span: var_info.declaration_span,
                    help: Some(
                        "consider removing this variable or using its value".to_string()
                    ),
                });
            }
        }
    }
}

impl Default for VariableAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}
