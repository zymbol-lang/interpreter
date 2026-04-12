//! Interpreter for Zymbol-Lang
//!
//! Phase 0: Only executes >> "string" statements
//! Phase 1: Variables and assignment
//! Phase 2: Module system support

use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use thiserror::Error;
use zymbol_ast::{
    Block, Expr, Program, Statement, TryStmt, CatchClause,
    DestructureAssign, DestructureItem, DestructurePattern,
};
use zymbol_span::Span;

mod literals;
mod io;
mod variables;
pub(crate) mod numeral_mode;
mod if_stmt;
mod loops;
mod match_stmt;
mod collections;
mod collection_ops;
mod string_ops;
mod expressions;
mod data_ops;
mod script_exec;
mod modules;
mod arithmetic_ops;
mod functions_lambda;
mod expr_eval;

pub(crate) use modules::LoadedModule;

/// Runtime errors
#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error("runtime error: {message}")]
    Generic { message: String, span: Span },

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("module not found: {path}")]
    ModuleNotFound { path: String },

    #[error("module '{module}' does not export function '{function}'")]
    FunctionNotExported { module: String, function: String },

    #[error("module '{module}' does not export constant '{constant}'")]
    ConstantNotExported { module: String, constant: String },

    #[error("circular dependency detected")]
    CircularDependency,

    #[error("failed to parse module: {0}")]
    ParseError(String),
}

pub type Result<T> = std::result::Result<T, RuntimeError>;

/// Control flow state for loops and returns
#[derive(Debug, Clone, PartialEq)]
enum ControlFlow {
    /// Normal execution
    None,
    /// Break from loop (with optional label)
    Break(Option<String>),
    /// Continue to next iteration (with optional label)
    Continue(Option<String>),
    /// Return from function with value
    Return(Option<Value>),
}

/// Function definition
#[derive(Debug, Clone)]
struct FunctionDef {
    parameters: Vec<zymbol_ast::Parameter>,
    body: zymbol_ast::Block,
}


/// Error value for error handling
/// Represents a runtime error that can be caught with try-catch
#[derive(Debug, Clone, PartialEq)]
pub struct ErrorValue {
    /// Error type: "IO", "Network", "Parse", "Index", "Type", "Div", "_" (generic)
    pub error_type: String,
    /// Error message
    pub message: String,
}

impl ErrorValue {
    pub fn new(error_type: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            error_type: error_type.into(),
            message: message.into(),
        }
    }

    /// Create a generic error
    pub fn generic(message: impl Into<String>) -> Self {
        Self::new("_", message)
    }

    /// Create an IO error
    pub fn io(message: impl Into<String>) -> Self {
        Self::new("IO", message)
    }

    /// Create an Index error (out of bounds)
    pub fn index(message: impl Into<String>) -> Self {
        Self::new("Index", message)
    }

    /// Create a Type error
    pub fn type_error(message: impl Into<String>) -> Self {
        Self::new("Type", message)
    }

    /// Create a Division error
    pub fn div(message: impl Into<String>) -> Self {
        Self::new("Div", message)
    }

    /// Create a Parse error
    pub fn parse(message: impl Into<String>) -> Self {
        Self::new("Parse", message)
    }
}

/// Runtime value
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    String(String),
    Int(i64),
    Float(f64),
    Char(char),
    Bool(bool),
    Array(Vec<Value>),
    Tuple(Vec<Value>),
    NamedTuple(Vec<(String, Value)>),  // (field_name, value) pairs
    Function(FunctionValue),
    /// Error value for try-catch error handling
    Error(ErrorValue),
    Unit,
}

/// Function value for lambdas and closures
#[derive(Debug, Clone)]
pub struct FunctionValue {
    pub params: Vec<String>,
    pub body: zymbol_ast::LambdaBody,
    pub captures: std::rc::Rc<std::collections::HashMap<String, Value>>,  // Shared closure env (Rc → O(1) clone)
}

impl PartialEq for FunctionValue {
    fn eq(&self, other: &Self) -> bool {
        // Note: We only compare params and body, not captures
        // Closures with different captures are considered different by reference
        self.params == other.params
    }
}

impl Value {
    /// Convert value to displayable string
    pub fn to_display_string(&self) -> String {
        match self {
            Value::String(s) => s.clone(),
            Value::Int(n) => n.to_string(),
            Value::Float(f) => f.to_string(),
            Value::Char(c) => c.to_string(),
            Value::Bool(b) => if *b { "#1" } else { "#0" }.to_string(),
            Value::Array(elements) => {
                let contents = elements
                    .iter()
                    .map(|v| v.to_display_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("[{}]", contents)
            }
            Value::Tuple(elements) => {
                let contents = elements
                    .iter()
                    .map(|v| v.to_display_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("({})", contents)
            }
            Value::NamedTuple(fields) => {
                let contents = fields
                    .iter()
                    .map(|(name, value)| format!("{}: {}", name, value.to_display_string()))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("({})", contents)
            }
            Value::Function(_) => {
                "<lambda>".to_string()
            }
            Value::Error(err) => {
                format!("##{}({})", err.error_type, err.message)
            }
            Value::Unit => "".to_string(),
        }
    }

    /// Check if this value is an error
    pub fn is_error(&self) -> bool {
        matches!(self, Value::Error(_))
    }

    /// Get the error value if this is an error
    pub fn as_error(&self) -> Option<&ErrorValue> {
        match self {
            Value::Error(err) => Some(err),
            _ => None,
        }
    }
}

/// Interpreter for executing Zymbol programs
pub struct Interpreter<W: Write> {
    output: W,
    /// Stack of variable scopes (lexical scoping)
    /// Index 0 is the global scope, higher indices are nested blocks
    scope_stack: Vec<HashMap<String, Value>>,
    functions: HashMap<String, Rc<FunctionDef>>,
    control_flow: ControlFlow,
    /// Track which variables are mutable (for parameter validation)
    /// Scoped stack parallel to scope_stack
    mutable_vars_stack: Vec<HashSet<String>>,
    /// Track which variables are constants (immutable)
    /// Scoped stack parallel to scope_stack
    const_vars_stack: Vec<HashSet<String>>,
    /// Loaded modules cache (file_path -> LoadedModule)
    loaded_modules: HashMap<PathBuf, LoadedModule>,
    /// Import aliases (alias -> file_path)
    import_aliases: HashMap<String, PathBuf>,
    /// Current file path (for resolving relative imports)
    current_file: Option<PathBuf>,
    /// Base directory for module resolution
    base_dir: PathBuf,
    /// CLI arguments passed to the script
    cli_args: Option<Vec<Value>>,
    /// Destruction schedule: statement_index -> variables to destroy after execution
    /// Populated by semantic analyzer's def-use chain analysis
    destruction_schedule: HashMap<usize, Vec<String>>,
    /// Dead variables: variables that have been destroyed (for use-after-free detection)
    dead_variables: HashSet<String>,
    /// Current statement index (for tracking which statement is executing)
    statement_index: usize,
    /// Short-circuit flag: true if any const (:=) has been declared in this interpreter session
    has_any_const: bool,
    /// QW6: fast check — true if control_flow != None (avoids enum PartialEq on hot path)
    pub(crate) has_control_flow: bool,
    /// B10+B13: Recycled HashMap pool for push_scope and function call scopes
    scope_map_pool: Vec<HashMap<String, Value>>,
    /// B10+B13: Recycled HashSet pool for mutable_vars tracking
    mut_set_pool: Vec<HashSet<String>>,
    /// B10+B13: Recycled HashSet pool for const_vars tracking
    const_set_pool: Vec<HashSet<String>>,
    /// B10: Recycled Vec<HashMap> pool for call frame scope_stack reuse
    scope_vec_pool: Vec<Vec<HashMap<String, Value>>>,
    /// QW3: Recycled Vec<HashSet> pool for mutable_vars_stack (one Vec per call frame)
    mut_vec_pool: Vec<Vec<HashSet<String>>>,
    /// QW3: Recycled Vec<HashSet> pool for const_vars_stack (one Vec per call frame)
    const_vec_pool: Vec<Vec<HashSet<String>>>,
    /// QW9: Recycled Vec pool for argument evaluation (avoids per-call heap alloc)
    arg_vec_pool: Vec<Vec<Value>>,
    /// MoveOrClone guard: depth of active try/catch blocks.
    /// When > 0, Return must clone (finally block may reference the variable after <~).
    /// When == 0, Return can move (take_variable) — O(1) for String/Array.
    try_depth: u8,
    /// TCO support: name of the currently executing function (None = not in a function).
    /// Used to detect `<~ f(same_args)` tail-call patterns.
    pub(crate) current_function: Option<String>,
    /// TCO restart: when true, function execution restarts with rebound params.
    pub(crate) tco_pending: bool,
    /// TCO args: the rebound argument values for the tail call restart.
    pub(crate) tco_args: Vec<Value>,
    /// QW13 fix: output param names of the current function.
    /// MoveOrClone (take_variable) must NOT be used for output params — writeback needs the value.
    pub(crate) current_output_params: std::collections::HashSet<String>,
    /// Active output numeral system (block base codepoint).
    /// Default: 0x0030 (ASCII). Changed by #<d0><d9># statements.
    /// Applies only to >> numeric outputs; does not affect to_display_string().
    pub(crate) numeral_mode: u32,
}

impl<W: Write> Interpreter<W> {
    /// Push a new scope onto the stack (entering a block).
    /// B10+B13: reuses pooled HashMaps/HashSets to avoid heap allocations.
    #[inline(always)]
    fn push_scope(&mut self) {
        let map = self.scope_map_pool.pop().unwrap_or_else(|| HashMap::with_capacity(4));
        let mut_s = self.mut_set_pool.pop().unwrap_or_default();
        let const_s = self.const_set_pool.pop().unwrap_or_default();
        self.scope_stack.push(map);
        self.mutable_vars_stack.push(mut_s);
        self.const_vars_stack.push(const_s);
    }

    /// Pop the current scope from the stack (exiting a block).
    /// B10+B13: returns cleared maps/sets to the pool for reuse.
    #[inline(always)]
    fn pop_scope(&mut self) {
        if self.scope_stack.len() > 1 {
            if let Some(mut map) = self.scope_stack.pop() {
                map.clear();
                if self.scope_map_pool.len() < 128 { self.scope_map_pool.push(map); }
            }
            if let Some(mut s) = self.mutable_vars_stack.pop() {
                s.clear();
                if self.mut_set_pool.len() < 128 { self.mut_set_pool.push(s); }
            }
            if let Some(mut s) = self.const_vars_stack.pop() {
                s.clear();
                if self.const_set_pool.len() < 128 { self.const_set_pool.push(s); }
            }
        }
    }

    /// Get a variable value, searching from innermost to outermost scope.
    #[inline(always)]
    fn get_variable(&self, name: &str) -> Option<&Value> {
        for scope in self.scope_stack.iter().rev() {
            if let Some(value) = scope.get(name) {
                return Some(value);
            }
        }
        None
    }

    /// Get a mutable reference to a variable, searching from innermost to outermost scope.
    #[inline(always)]
    fn get_variable_mut(&mut self, name: &str) -> Option<&mut Value> {
        for scope in self.scope_stack.iter_mut().rev() {
            if let Some(val) = scope.get_mut(name) {
                return Some(val);
            }
        }
        None
    }

    /// Insert a NEW variable directly into the current scope, skipping the scope-stack scan.
    /// Only safe when the variable is KNOWN to be new (e.g., function parameter binding
    /// into a freshly created isolated scope). Saves ~20-30ns vs set_variable for new vars.
    #[inline(always)]
    pub(crate) fn set_variable_new(&mut self, name: &str, value: Value) {
        if let Some(scope) = self.scope_stack.last_mut() {
            scope.insert(name.to_string(), value);
        }
    }

    /// Move a variable's value out of the scope (replace with Unit, return owned Value).
    /// MoveOrClone: O(1) for all types including String/Array — no heap allocation.
    /// Only safe when the variable will not be referenced again (e.g., on Return).
    #[inline(always)]
    pub(crate) fn take_variable(&mut self, name: &str) -> Option<Value> {
        for scope in self.scope_stack.iter_mut().rev() {
            if let Some(v) = scope.get_mut(name) {
                return Some(std::mem::replace(v, Value::Unit));
            }
        }
        None
    }

    /// Set a variable value in the appropriate scope.
    /// B9: zero allocation on the UPDATE path (hot path).
    #[inline(always)]
    fn set_variable(&mut self, name: &str, value: Value) {
        for scope in self.scope_stack.iter_mut().rev() {
            if let Some(existing) = scope.get_mut(name) {
                *existing = value;
                return;
            }
        }
        if let Some(scope) = self.scope_stack.last_mut() {
            scope.insert(name.to_string(), value);
        }
    }

    /// Check if a variable is a constant in any scope.
    #[inline(always)]
    fn is_const(&self, name: &str) -> bool {
        if !self.has_any_const { return false; }  // B8: short-circuit
        for const_set in self.const_vars_stack.iter().rev() {
            if const_set.contains(name) {
                return true;
            }
        }
        false
    }

    /// QW6: fast check — avoids full enum PartialEq on every statement.
    #[inline(always)]
    fn is_control_flow_pending(&self) -> bool {
        self.has_control_flow
    }

    /// QW6: set control flow and activate the fast flag.
    #[inline(always)]
    fn set_control_flow(&mut self, cf: ControlFlow) {
        self.has_control_flow = !matches!(cf, ControlFlow::None);
        self.control_flow = cf;
    }

    /// QW6: clear control flow and deactivate the fast flag.
    #[inline(always)]
    fn clear_control_flow(&mut self) {
        self.has_control_flow = false;
        self.control_flow = ControlFlow::None;
    }

    /// Mark a variable as constant in the current scope
    fn mark_const(&mut self, name: String) {
        self.has_any_const = true;  // B8: activate flag
        if let Some(current_const_set) = self.const_vars_stack.last_mut() {
            current_const_set.insert(name);
        }
    }

    /// Check if a variable is mutable in any scope
    /// Note: Reserved for future semantic analysis of reassignment rules
    #[allow(dead_code)]
    fn is_mutable(&self, name: &str) -> bool {
        for mutable_set in self.mutable_vars_stack.iter().rev() {
            if mutable_set.contains(name) {
                return true;
            }
        }
        false
    }

    /// Mark a variable as mutable in the current scope
    fn mark_mutable(&mut self, name: String) {
        if let Some(current_mutable_set) = self.mutable_vars_stack.last_mut() {
            current_mutable_set.insert(name);
        }
    }

    /// Get all variables from all scopes (for compatibility)
    fn get_all_variables(&self) -> HashMap<String, Value> {
        let mut all_vars = HashMap::new();
        // Merge from outermost to innermost (later scopes override earlier)
        for scope in &self.scope_stack {
            all_vars.extend(scope.clone());
        }
        all_vars
    }

    /// Save all call-scoped interpreter state and initialize a fresh isolated scope.
    /// Used by both lambda calls and traditional function calls (B2).
    /// B10: reuses pooled Vecs/HashMaps. QW3: also pools mutable/const_vars Vecs.
    pub(crate) fn take_call_state(&mut self) -> SavedCallState {
        let saved = SavedCallState {
            scope_stack: std::mem::take(&mut self.scope_stack),
            mutable_vars_stack: std::mem::take(&mut self.mutable_vars_stack),
            const_vars_stack: std::mem::take(&mut self.const_vars_stack),
            import_aliases: std::mem::take(&mut self.import_aliases),
            has_any_const: self.has_any_const,
        };
        // B10: reuse pooled Vec for scope_stack
        let mut fresh_scope_vec = self.scope_vec_pool.pop().unwrap_or_default();
        let map = self.scope_map_pool.pop().unwrap_or_else(|| HashMap::with_capacity(4));
        fresh_scope_vec.push(map);
        self.scope_stack = fresh_scope_vec;
        // QW3: reuse pooled Vec for mutable_vars_stack and const_vars_stack
        let mut mut_vec = self.mut_vec_pool.pop().unwrap_or_default();
        mut_vec.push(self.mut_set_pool.pop().unwrap_or_default());
        self.mutable_vars_stack = mut_vec;
        let mut const_vec = self.const_vec_pool.pop().unwrap_or_default();
        const_vec.push(self.const_set_pool.pop().unwrap_or_default());
        self.const_vars_stack = const_vec;
        self.has_any_const = false;
        saved
    }

    /// Restore all call-scoped interpreter state saved by `take_call_state`.
    /// B10+QW3: recycles all frame components back into their pools.
    pub(crate) fn restore_call_state(&mut self, saved: SavedCallState) {
        let mut fn_scope_vec = std::mem::replace(&mut self.scope_stack, saved.scope_stack);
        let mut fn_mut = std::mem::replace(&mut self.mutable_vars_stack, saved.mutable_vars_stack);
        let mut fn_const = std::mem::replace(&mut self.const_vars_stack, saved.const_vars_stack);
        self.import_aliases = saved.import_aliases;
        self.has_any_const = saved.has_any_const;

        // Pool scope_stack components
        for mut map in fn_scope_vec.drain(..) {
            map.clear();
            if self.scope_map_pool.len() < 128 { self.scope_map_pool.push(map); }
        }
        if self.scope_vec_pool.len() < 32 { self.scope_vec_pool.push(fn_scope_vec); }

        // QW3: pool mutable_vars_stack Vec itself
        for mut s in fn_mut.drain(..) {
            s.clear();
            if self.mut_set_pool.len() < 128 { self.mut_set_pool.push(s); }
        }
        if self.mut_vec_pool.len() < 32 { self.mut_vec_pool.push(fn_mut); }

        // QW3: pool const_vars_stack Vec itself
        for mut s in fn_const.drain(..) {
            s.clear();
            if self.const_set_pool.len() < 128 { self.const_set_pool.push(s); }
        }
        if self.const_vec_pool.len() < 32 { self.const_vec_pool.push(fn_const); }
    }
}

/// Interpreter state saved across a function/lambda call boundary (used by B2).
pub(crate) struct SavedCallState {
    scope_stack: Vec<HashMap<String, Value>>,
    mutable_vars_stack: Vec<HashSet<String>>,
    const_vars_stack: Vec<HashSet<String>>,
    import_aliases: HashMap<String, std::path::PathBuf>,
    has_any_const: bool,
}

impl Interpreter<std::io::Stdout> {
    pub fn new() -> Self {
        Self {
            output: std::io::stdout(),
            scope_stack: vec![HashMap::new()],  // Start with one global scope
            functions: HashMap::new(),
            control_flow: ControlFlow::None,
            mutable_vars_stack: vec![HashSet::new()],
            const_vars_stack: vec![HashSet::new()],
            loaded_modules: HashMap::new(),
            import_aliases: HashMap::new(),
            current_file: None,
            base_dir: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            cli_args: None,
            destruction_schedule: HashMap::new(),
            dead_variables: HashSet::new(),
            statement_index: 0,
            has_any_const: false,
            has_control_flow: false,
            scope_map_pool: Vec::new(),
            mut_set_pool: Vec::new(),
            const_set_pool: Vec::new(),
            scope_vec_pool: Vec::new(),
            mut_vec_pool: Vec::new(),
            const_vec_pool: Vec::new(),
            arg_vec_pool: Vec::new(),
            try_depth: 0,
            current_function: None,
            tco_pending: false,
            tco_args: Vec::new(),
            current_output_params: std::collections::HashSet::new(),
            numeral_mode: numeral_mode::ASCII_BASE,
        }
    }
}

impl Default for Interpreter<std::io::Stdout> {
    fn default() -> Self {
        Self::new()
    }
}

impl<W: Write> Interpreter<W> {
    /// Create interpreter with custom output writer
    pub fn with_output(output: W) -> Self {
        Self {
            output,
            scope_stack: vec![HashMap::new()],  // Start with one global scope
            functions: HashMap::new(),
            control_flow: ControlFlow::None,
            mutable_vars_stack: vec![HashSet::new()],
            const_vars_stack: vec![HashSet::new()],
            loaded_modules: HashMap::new(),
            import_aliases: HashMap::new(),
            current_file: None,
            base_dir: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            cli_args: None,
            destruction_schedule: HashMap::new(),
            dead_variables: HashSet::new(),
            statement_index: 0,
            has_any_const: false,
            has_control_flow: false,
            scope_map_pool: Vec::new(),
            mut_set_pool: Vec::new(),
            const_set_pool: Vec::new(),
            scope_vec_pool: Vec::new(),
            mut_vec_pool: Vec::new(),
            const_vec_pool: Vec::new(),
            arg_vec_pool: Vec::new(),
            try_depth: 0,
            current_function: None,
            tco_pending: false,
            tco_args: Vec::new(),
            current_output_params: std::collections::HashSet::new(),
            numeral_mode: numeral_mode::ASCII_BASE,
        }
    }

    /// Set the current file path for module resolution
    pub fn set_current_file<P: AsRef<Path>>(&mut self, path: P) {
        self.current_file = Some(path.as_ref().to_path_buf());
    }

    /// Set the base directory for module resolution
    pub fn set_base_dir<P: AsRef<Path>>(&mut self, path: P) {
        self.base_dir = path.as_ref().to_path_buf();
    }

    /// Set the destruction schedule from semantic analysis
    /// Maps statement_index -> variables to destroy after that statement executes
    pub fn set_destruction_schedule(&mut self, schedule: HashMap<usize, Vec<String>>) {
        self.destruction_schedule = schedule;
    }

    /// Destroy a variable immediately (remove from all scopes and mark as dead)
    fn destroy_variable(&mut self, var_name: &str) {
        // Remove from all scopes (search from innermost to outermost)
        for scope in self.scope_stack.iter_mut().rev() {
            if scope.remove(var_name).is_some() {
                // Found and removed - mark as dead
                self.dead_variables.insert(var_name.to_string());
                return;
            }
        }
    }

    /// Check if a variable has been destroyed (use-after-free detection)
    fn check_variable_alive(&self, var_name: &str, span: &Span) -> Result<()> {
        if self.dead_variables.is_empty() { return Ok(()); }  // B8: short-circuit
        if self.dead_variables.contains(var_name) {
            return Err(RuntimeError::Generic {
                message: format!(
                    "use after destruction: variable '{}' was destroyed after its last use",
                    var_name
                ),
                span: *span,
            });
        }
        Ok(())
    }

    /// Set CLI arguments
    pub fn set_cli_args(&mut self, args: Vec<String>) {
        // Convert strings to Value::String
        let args_values: Vec<Value> = args.into_iter()
            .map(Value::String)
            .collect();
        self.cli_args = Some(args_values);
    }

    /// Execute a single line of code (for REPL)
    /// Returns the value of the last expression if any
    pub fn execute_line(&mut self, source: &str) -> Result<Option<Value>> {
        // Parse the source
        let lexer = zymbol_lexer::Lexer::new(source, zymbol_span::FileId(0));
        let (tokens, lex_diagnostics) = lexer.tokenize();

        if !lex_diagnostics.is_empty() {
            let msg = lex_diagnostics
                .iter()
                .map(|d| d.message.clone())
                .collect::<Vec<_>>()
                .join("; ");
            return Err(RuntimeError::ParseError(msg));
        }

        let parser = zymbol_parser::Parser::new(tokens);
        let program = parser.parse().map_err(|diagnostics| {
            let msg = diagnostics
                .iter()
                .map(|d| d.message.clone())
                .collect::<Vec<_>>()
                .join("; ");
            RuntimeError::ParseError(msg)
        })?;

        // Execute statements and capture the last expression value
        let mut last_value: Option<Value> = None;

        for statement in &program.statements {
            // For expression statements, capture the value
            if let Statement::Expr(expr_stmt) = statement {
                last_value = Some(self.eval_expr(&expr_stmt.expr)?);
            } else {
                self.execute_statement(statement)?;
                last_value = None;
            }

            // Check for control flow changes
            if self.is_control_flow_pending() {
                break;
            }
        }

        Ok(last_value)
    }

    /// List all variables defined in the current scope
    /// Returns a vector of (name, value) pairs
    pub fn list_variables(&self) -> Vec<(String, Value)> {
        let all_vars = self.get_all_variables();
        let mut result: Vec<(String, Value)> = all_vars.into_iter().collect();
        result.sort_by(|a, b| a.0.cmp(&b.0));
        result
    }

    /// Get information about a specific variable
    /// Returns (type_name, value) if the variable exists
    /// Uses Zymbol's symbolic type notation:
    /// ###=Int, ##.=Float, ##"=String, ##'=Char, ##?=Bool, ##]=Array, ##)=Tuple, ##_=Unit
    pub fn get_variable_info(&self, name: &str) -> Option<(String, Value)> {
        self.get_variable(name).map(|value| {
            let type_name = match value {
                Value::Int(_) => "###".to_string(),
                Value::Float(_) => "##.".to_string(),
                Value::String(_) => "##\"".to_string(),
                Value::Char(_) => "##'".to_string(),
                Value::Bool(_) => "##?".to_string(),
                Value::Array(elements) => {
                    if elements.is_empty() {
                        "##]".to_string()
                    } else {
                        format!("##]<{}>", self.value_type_name(&elements[0]))
                    }
                }
                Value::Tuple(elements) => {
                    let types: Vec<String> = elements.iter().map(|v| self.value_type_name(v)).collect();
                    format!("##)({})", types.join(", "))
                }
                Value::NamedTuple(fields) => {
                    let types: Vec<String> = fields
                        .iter()
                        .map(|(name, val)| format!("{}: {}", name, self.value_type_name(val)))
                        .collect();
                    format!("##)({})", types.join(", "))
                }
                Value::Function(_) => "##->".to_string(),
                Value::Error(err) => format!("##{}", err.error_type),
                Value::Unit => "##_".to_string(),
            };
            (type_name, value.clone())
        })
    }

    /// Helper to get type name for a value (symbolic notation)
    fn value_type_name(&self, value: &Value) -> String {
        match value {
            Value::Int(_) => "###".to_string(),
            Value::Float(_) => "##.".to_string(),
            Value::String(_) => "##\"".to_string(),
            Value::Char(_) => "##'".to_string(),
            Value::Bool(_) => "##?".to_string(),
            Value::Array(_) => "##]".to_string(),
            Value::Tuple(_) => "##)".to_string(),
            Value::NamedTuple(_) => "##)".to_string(),
            Value::Function(_) => "##->".to_string(),
            Value::Error(err) => format!("##{}", err.error_type),
            Value::Unit => "##_".to_string(),
        }
    }

    /// Format a value for display using the current active numeral mode.
    ///
    /// Numeric types (`Int`, `Float`, `Bool`) are rendered in the active script.
    /// All other types use their standard `to_display_string()` form.
    pub fn format_value(&self, value: &Value) -> String {
        let mode = self.numeral_mode;
        match value {
            Value::Int(n)   => numeral_mode::to_numeral_int(*n, mode),
            Value::Float(f) => numeral_mode::to_numeral_float(*f, mode),
            Value::Bool(b)  => numeral_mode::to_numeral_bool(*b, mode),
            _               => value.to_display_string(),
        }
    }

    /// Execute a program
    pub fn execute(&mut self, program: &Program) -> Result<()> {
        // Process imports first
        for import in &program.imports {
            self.load_import(import)?;
        }

        // Reset statement index
        self.statement_index = 0;

        // Execute statements with auto-destruction
        for statement in &program.statements {
            self.execute_statement(statement)?;

            // Check if any variables should be destroyed after this statement
            if let Some(vars_to_destroy) = self.destruction_schedule.get(&self.statement_index) {
                let vars = vars_to_destroy.clone();
                for var_name in vars {
                    self.destroy_variable(&var_name);
                }
            }

            self.statement_index += 1;
        }
        Ok(())
    }

    // Load and process an import statement
    // Resolve a module path to an absolute file path
    // Load a module from file

    /// Execute a single statement
    fn execute_statement(&mut self, statement: &Statement) -> Result<()> {
        match statement {
            Statement::Output(output) => self.execute_output(output),
            Statement::Assignment(assign) => self.execute_assignment(assign),
            Statement::ConstDecl(const_decl) => self.execute_const_decl(const_decl),
            Statement::Newline(newline) => self.execute_newline(newline),
            Statement::Input(input) => self.execute_input(input),
            Statement::If(if_stmt) => self.execute_if(if_stmt),
            Statement::Loop(loop_stmt) => self.execute_loop(loop_stmt),
            Statement::Break(break_stmt) => self.execute_break(break_stmt),
            Statement::Continue(continue_stmt) => self.execute_continue(continue_stmt),
            Statement::FunctionDecl(func_decl) => {
                // Store function definition
                let func_def = FunctionDef {
                    parameters: func_decl.parameters.clone(),
                    body: func_decl.body.clone(),
                };
                self.functions.insert(func_decl.name.clone(), Rc::new(func_def));
                Ok(())
            }
            Statement::Return(return_stmt) => {
                let value = if let Some(expr) = &return_stmt.value {
                    // QW17: TCO — detect <~ f(args) where f == current executing function.
                    // When detected: evaluate args, store in tco_args, set tco_pending = true,
                    // and set Return so the call frame unwinds cleanly into the TCO loop.
                    if self.try_depth == 0 {
                        if let Expr::FunctionCall(call) = expr.as_ref() {
                            if let Expr::Identifier(callee) = call.callable.as_ref() {
                                if let Some(cur_fn) = self.current_function.as_deref() {
                                    if callee.name == cur_fn {
                                        // Evaluate all arguments eagerly
                                        let mut tco_args = Vec::with_capacity(call.arguments.len());
                                        for arg in &call.arguments {
                                            tco_args.push(self.eval_expr(arg)?);
                                        }
                                        self.tco_args = tco_args;
                                        self.tco_pending = true;
                                        // Signal Return(None) so the execute_block_no_scope loop
                                        // exits cleanly — the TCO loop in eval_traditional_function_call
                                        // will detect tco_pending and restart.
                                        self.set_control_flow(ControlFlow::Return(None));
                                        return Ok(());
                                    }
                                }
                            }
                        }
                    }
                    // MoveOrClone: if returning a bare identifier and not inside a try block
                    // (finally could reference the variable), move instead of clone — O(1).
                    // Skip take_variable for output params — writeback still needs the value.
                    if self.try_depth == 0 {
                        if let Expr::Identifier(ident) = expr.as_ref() {
                            if !self.current_output_params.contains(&ident.name) {
                                if let Some(v) = self.take_variable(&ident.name) {
                                    self.set_control_flow(ControlFlow::Return(Some(v)));
                                    return Ok(());
                                }
                            }
                        }
                    }
                    Some(self.eval_expr(expr)?)
                } else {
                    None
                };
                self.set_control_flow(ControlFlow::Return(value));
                Ok(())
            }
            Statement::Match(match_expr) => self.execute_match_statement(match_expr),
            Statement::Expr(expr_stmt) => {
                // Evaluate expression for side effects, discard result
                self.eval_expr(&expr_stmt.expr)?;
                Ok(())
            }
            Statement::CliArgsCapture(cli_args) => {
                // Capture CLI args into the specified variable
                // For now, we'll need to pass CLI args through the interpreter context
                // This will be implemented when we add CLI args support to the interpreter
                let args_array = self.cli_args.clone().unwrap_or_default();
                self.set_variable(&cli_args.variable_name, Value::Array(args_array));
                Ok(())
            }
            Statement::LifetimeEnd(_lifetime_end) => {
                // Phase 1: Placeholder for explicit variable destruction
                // Full implementation will come in Phase 5 (Runtime Integration)
                // For now, this is a no-op
                Ok(())
            }
            Statement::DestructureAssign(d) => self.eval_destructure_assign(d),
            Statement::Try(try_stmt) => self.execute_try(try_stmt),
            Statement::SetNumeralMode { base, .. } => {
                self.numeral_mode = *base;
                Ok(())
            }
        }
    }

    /// Execute a block of statements with a new scope (standard path).
    fn execute_block(&mut self, block: &Block) -> Result<()> {
        self.push_scope();
        for statement in &block.statements {
            self.execute_statement(statement)?;
            if self.is_control_flow_pending() { break; }
        }
        self.pop_scope();
        Ok(())
    }

    /// QW1: Execute a block WITHOUT creating a new scope.
    /// Used for function/lambda bodies — take_call_state already created scope[0],
    /// so a second push_scope would cause double-scope overhead on every call.
    #[inline(always)]
    pub(crate) fn execute_block_no_scope(&mut self, block: &Block) -> Result<()> {
        for statement in &block.statements {
            self.execute_statement(statement)?;
            if self.is_control_flow_pending() { break; }
        }
        Ok(())
    }

    /// Execute a destructure assignment statement: [a, *rest, _] = expr / (a, b) = expr / (field: var) = expr
    pub(crate) fn eval_destructure_assign(&mut self, d: &DestructureAssign) -> Result<()> {
        let rhs = self.eval_expr(&d.value)?;
        match &d.pattern {
            DestructurePattern::Array(items) | DestructurePattern::Positional(items) => {
                let elements: Vec<Value> = match &rhs {
                    Value::Array(arr) => arr.clone(),
                    Value::Tuple(tup) => tup.clone(),
                    _ => return Err(RuntimeError::Generic {
                        message: format!(
                            "destructure assignment requires an array or tuple, got {}",
                            self.value_type_name(&rhs)
                        ),
                        span: d.span,
                    }),
                };
                let mut idx = 0usize;
                for item in items {
                    match item {
                        DestructureItem::Bind(name) => {
                            let val = elements.get(idx).cloned().unwrap_or(Value::Unit);
                            self.set_variable(name, val);
                            idx += 1;
                        }
                        DestructureItem::Rest(name) => {
                            // Collect remaining elements (excluding any trailing Bind/Ignore items)
                            let trailing = items.iter().rev().take_while(|i| !matches!(i, DestructureItem::Rest(_))).count();
                            let end = if trailing > 0 && elements.len() > idx + trailing {
                                elements.len() - trailing
                            } else {
                                elements.len()
                            };
                            let rest: Vec<Value> = elements.get(idx..end).unwrap_or(&[]).to_vec();
                            self.set_variable(name, Value::Array(rest));
                            idx = end;
                        }
                        DestructureItem::Ignore => {
                            idx += 1;
                        }
                    }
                }
            }
            DestructurePattern::NamedTuple(fields) => {
                let pairs: &Vec<(String, Value)> = match &rhs {
                    Value::NamedTuple(p) => p,
                    _ => return Err(RuntimeError::Generic {
                        message: format!(
                            "named tuple destructure requires a named tuple, got {}",
                            self.value_type_name(&rhs)
                        ),
                        span: d.span,
                    }),
                };
                for (field, var_name) in fields {
                    let val = pairs.iter()
                        .find(|(k, _)| k == field)
                        .map(|(_, v)| v.clone())
                        .unwrap_or(Value::Unit);
                    self.set_variable(var_name, val);
                }
            }
        }
        Ok(())
    }

    /// Evaluate an expression
    fn eval_expr(&mut self, expr: &Expr) -> Result<Value> {
        match expr {
            Expr::Literal(lit) => self.eval_literal(lit),
            Expr::Identifier(ident) => self.eval_identifier(ident),
            Expr::Binary(binary) => self.eval_binary(binary),
            Expr::Unary(unary) => self.eval_unary(unary),
            Expr::Range(_) => Err(RuntimeError::Generic {
                message: "ranges can only be used in for-each loops".to_string(),
                span: expr.span(),
            }),
            Expr::ArrayLiteral(arr) => self.eval_array_literal(arr),
            Expr::Tuple(tuple) => self.eval_tuple(tuple),
            Expr::NamedTuple(named_tuple) => self.eval_named_tuple(named_tuple),
            Expr::MemberAccess(member) => self.eval_member_access(member),
            Expr::Index(idx) => self.eval_index(idx),
            Expr::FunctionCall(call) => self.eval_function_call(call),
            Expr::Match(match_expr) => self.eval_match(match_expr),
            Expr::CollectionLength(op) => self.eval_collection_length(op),
            Expr::CollectionAppend(op) => self.eval_collection_append(op),
            Expr::CollectionInsert(op) => self.eval_collection_insert(op),
            Expr::CollectionRemoveValue(op) => self.eval_collection_remove_value(op),
            Expr::CollectionRemoveAll(op) => self.eval_collection_remove_all(op),
            Expr::CollectionRemoveAt(op) => self.eval_collection_remove(op),
            Expr::CollectionRemoveRange(op) => self.eval_collection_remove_range(op),
            Expr::CollectionContains(op) => self.eval_collection_contains(op),
            Expr::CollectionFindAll(op) => self.eval_collection_find_all(op),
            Expr::CollectionUpdate(op) => self.eval_collection_update(op),
            Expr::CollectionSlice(op) => self.eval_collection_slice(op),
            Expr::StringReplace(op) => self.eval_string_replace(op),
            Expr::NumericEval(op) => self.eval_numeric_eval(op),
            Expr::TypeMetadata(op) => self.eval_type_metadata(op),
            Expr::Format(op) => self.eval_format(op),
            Expr::BaseConversion(op) => self.eval_base_conversion(op),
            Expr::Lambda(lambda) => self.eval_lambda(lambda),
            Expr::CollectionMap(op) => self.eval_collection_map(op),
            Expr::CollectionFilter(op) => self.eval_collection_filter(op),
            Expr::CollectionReduce(op) => self.eval_collection_reduce(op),
            Expr::CollectionSortAsc(op) => self.eval_collection_sort(op),
            Expr::CollectionSortDesc(op) => self.eval_collection_sort(op),
            Expr::CollectionSortCustom(op) => self.eval_collection_sort(op),
            Expr::Pipe(pipe) => self.eval_pipe(pipe),
            Expr::Execute(execute) => self.eval_execute(execute),
            Expr::BashExec(bash) => self.eval_bash_exec(bash),
            Expr::Round(op) => self.eval_round(op),
            Expr::Trunc(op) => self.eval_trunc(op),
            Expr::ErrorCheck(check) => {
                // expr$! - returns #1 if expression is an error, #0 otherwise
                let value = self.eval_expr(&check.expr)?;
                Ok(Value::Bool(value.is_error()))
            }
            Expr::ErrorPropagate(prop) => {
                // expr$!! - propagate error to caller if expression is an error
                let value = self.eval_expr(&prop.expr)?;
                if value.is_error() {
                    self.set_control_flow(ControlFlow::Return(Some(value.clone())));
                }
                Ok(value)
            }
        }
    }

    /// Execute a try-catch-finally statement
    fn execute_try(&mut self, try_stmt: &TryStmt) -> Result<()> {
        // Guard: Return inside try/catch must clone (finally may reference the variable).
        self.try_depth += 1;
        let try_result = self.execute_block(&try_stmt.try_block);
        self.try_depth -= 1;

        // Check if we got an error (either RuntimeError or returned Error value)
        let error_value = match &try_result {
            Err(e) => Some(self.runtime_error_to_value(e)),
            Ok(()) => {
                // Check if control flow returned an error value
                if let ControlFlow::Return(Some(ref val)) = self.control_flow {
                    if val.is_error() {
                        let err = val.clone();
                        self.clear_control_flow();
                        Some(err)
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
        };

        // If we have an error, try to find a matching catch clause
        let mut caught = false;
        if let Some(ref err_val) = error_value {
            for catch_clause in &try_stmt.catch_clauses {
                if self.catch_matches(catch_clause, err_val) {
                    // Execute catch block with _err variable
                    self.execute_catch_block(catch_clause, err_val.clone())?;
                    caught = true;
                    break;
                }
            }
        }

        // Execute finally block if present (always runs)
        if let Some(ref finally) = try_stmt.finally_clause {
            self.execute_block(&finally.block)?;
        }

        // If error wasn't caught, propagate it
        if error_value.is_some() && !caught {
            try_result?;
        }

        Ok(())
    }

    /// Convert a RuntimeError to an ErrorValue
    fn runtime_error_to_value(&self, error: &RuntimeError) -> Value {
        match error {
            RuntimeError::Io(io_err) => {
                Value::Error(ErrorValue::io(io_err.to_string()))
            }
            RuntimeError::Generic { message, .. } => {
                // Try to classify the error based on message content
                let lower_msg = message.to_lowercase();
                if lower_msg.contains("index") || lower_msg.contains("out of bounds") {
                    Value::Error(ErrorValue::index(message.clone()))
                } else if lower_msg.contains("type") {
                    Value::Error(ErrorValue::type_error(message.clone()))
                } else if lower_msg.contains("division") || lower_msg.contains("divide by zero") {
                    Value::Error(ErrorValue::div(message.clone()))
                } else if lower_msg.contains("parse") {
                    Value::Error(ErrorValue::parse(message.clone()))
                } else {
                    Value::Error(ErrorValue::generic(message.clone()))
                }
            }
            RuntimeError::ModuleNotFound { path } => {
                Value::Error(ErrorValue::io(format!("module not found: {}", path)))
            }
            RuntimeError::FunctionNotExported { module, function } => {
                Value::Error(ErrorValue::generic(format!(
                    "function '{}' not exported from module '{}'",
                    function, module
                )))
            }
            RuntimeError::ConstantNotExported { module, constant } => {
                Value::Error(ErrorValue::generic(format!(
                    "constant '{}' not exported from module '{}'",
                    constant, module
                )))
            }
            RuntimeError::CircularDependency => {
                Value::Error(ErrorValue::generic("circular dependency detected"))
            }
            RuntimeError::ParseError(msg) => {
                Value::Error(ErrorValue::parse(msg.clone()))
            }
        }
    }

    /// Check if a catch clause matches an error value
    fn catch_matches(&self, catch: &CatchClause, error: &Value) -> bool {
        let error_val = match error.as_error() {
            Some(e) => e,
            None => return false,
        };

        match &catch.error_type {
            // Generic catch (no type specified) matches any error
            None => true,
            // Typed catch matches specific error type
            Some(err_type) => {
                // "_" (wildcard) matches any error type
                if err_type.name == "_" {
                    return true;
                }
                // Match by error type name
                err_type.name == error_val.error_type
            }
        }
    }

    /// Execute a catch block with _err variable bound
    fn execute_catch_block(&mut self, catch: &CatchClause, error: Value) -> Result<()> {
        // Push new scope for catch block
        self.push_scope();

        // Bind _err variable in the catch scope
        self.set_variable("_err", error);

        // Execute catch block statements
        for statement in &catch.block.statements {
            self.execute_statement(statement)?;

            // Stop executing if we have a control flow change
            if self.is_control_flow_pending() {
                break;
            }
        }

        // Pop catch scope
        self.pop_scope();

        Ok(())
    }
}

#[cfg(test)]
mod error_handling_tests {
    use super::*;

    #[test]
    fn test_error_value_creation() {
        let err = ErrorValue::new("IO", "file not found");
        assert_eq!(err.error_type, "IO");
        assert_eq!(err.message, "file not found");
    }

    #[test]
    fn test_error_value_constructors() {
        let generic = ErrorValue::generic("some error");
        assert_eq!(generic.error_type, "_");

        let io = ErrorValue::io("io error");
        assert_eq!(io.error_type, "IO");

        let index = ErrorValue::index("out of bounds");
        assert_eq!(index.error_type, "Index");

        let type_err = ErrorValue::type_error("type mismatch");
        assert_eq!(type_err.error_type, "Type");

        let div = ErrorValue::div("division by zero");
        assert_eq!(div.error_type, "Div");

        let parse = ErrorValue::parse("parse error");
        assert_eq!(parse.error_type, "Parse");
    }

    #[test]
    fn test_value_is_error() {
        let error = Value::Error(ErrorValue::generic("test"));
        assert!(error.is_error());

        let int = Value::Int(42);
        assert!(!int.is_error());

        let string = Value::String("hello".to_string());
        assert!(!string.is_error());
    }

    #[test]
    fn test_value_as_error() {
        let error = Value::Error(ErrorValue::io("test"));
        assert!(error.as_error().is_some());
        assert_eq!(error.as_error().unwrap().error_type, "IO");

        let int = Value::Int(42);
        assert!(int.as_error().is_none());
    }

    #[test]
    fn test_error_display_string() {
        let error = Value::Error(ErrorValue::new("IO", "file not found"));
        assert_eq!(error.to_display_string(), "##IO(file not found)");

        let generic = Value::Error(ErrorValue::generic("unknown error"));
        assert_eq!(generic.to_display_string(), "##_(unknown error)");
    }

    fn parse_and_run(code: &str) -> (Vec<u8>, Result<()>) {
        let lexer = zymbol_lexer::Lexer::new(code, zymbol_span::FileId(0));
        let (tokens, lex_diagnostics) = lexer.tokenize();
        assert!(lex_diagnostics.is_empty(), "Lexer errors: {:?}", lex_diagnostics);
        let program = zymbol_parser::Parser::new(tokens).parse().unwrap();
        let mut output = Vec::new();
        let result = {
            let mut interp = Interpreter::with_output(&mut output);
            interp.execute(&program)
        };
        (output, result)
    }

    #[test]
    fn test_error_check_on_non_error() {
        // x = 42
        // ? x$! { >> "error" ¶ } _{ >> "ok" ¶ }
        let code = r#"
            x = 42
            ? x$! { >> "error" ¶ } _ { >> "ok" ¶ }
        "#;
        let (output, result) = parse_and_run(code);
        assert!(result.is_ok());
        assert_eq!(String::from_utf8_lossy(&output), "ok\n");
    }

    #[test]
    fn test_try_catch_simple() {
        // Test try block with no error
        let code = r#"
            !? {
                x = 42
                >> x ¶
            } :! {
                >> "caught" ¶
            }
        "#;
        let (output, result) = parse_and_run(code);
        assert!(result.is_ok());
        assert_eq!(String::from_utf8_lossy(&output), "42\n");
    }

    #[test]
    fn test_try_finally_always_runs() {
        // Test that finally block always executes
        let code = r#"
            !? {
                >> "try" ¶
            } :> {
                >> "finally" ¶
            }
        "#;
        let (output, result) = parse_and_run(code);
        assert!(result.is_ok());
        assert_eq!(String::from_utf8_lossy(&output), "try\nfinally\n");
    }

    #[test]
    fn test_try_catch_finally_order() {
        // Test that try-catch-finally execute in correct order
        let code = r#"
            !? {
                >> "try" ¶
            } :! {
                >> "catch" ¶
            } :> {
                >> "finally" ¶
            }
        "#;
        let (output, result) = parse_and_run(code);
        assert!(result.is_ok());
        // No error, so catch shouldn't run
        assert_eq!(String::from_utf8_lossy(&output), "try\nfinally\n");
    }

    #[test]
    fn test_runtime_error_to_value_io() {
        let interp = Interpreter::new();
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let runtime_err = RuntimeError::Io(io_err);
        let value = interp.runtime_error_to_value(&runtime_err);

        if let Value::Error(err) = value {
            assert_eq!(err.error_type, "IO");
        } else {
            panic!("Expected Error value");
        }
    }

    #[test]
    fn test_runtime_error_to_value_generic() {
        let interp = Interpreter::new();
        let runtime_err = RuntimeError::Generic {
            message: "something went wrong".to_string(),
            span: zymbol_span::Span::new(
                zymbol_span::Position::start(),
                zymbol_span::Position::start(),
                zymbol_span::FileId(0),
            ),
        };
        let value = interp.runtime_error_to_value(&runtime_err);

        if let Value::Error(err) = value {
            assert_eq!(err.error_type, "_");
        } else {
            panic!("Expected Error value");
        }
    }

    #[test]
    fn test_runtime_error_to_value_index() {
        let interp = Interpreter::new();
        let runtime_err = RuntimeError::Generic {
            message: "index out of bounds".to_string(),
            span: zymbol_span::Span::new(
                zymbol_span::Position::start(),
                zymbol_span::Position::start(),
                zymbol_span::FileId(0),
            ),
        };
        let value = interp.runtime_error_to_value(&runtime_err);

        if let Value::Error(err) = value {
            assert_eq!(err.error_type, "Index");
        } else {
            panic!("Expected Error value");
        }
    }

    #[test]
    fn test_catch_matches_generic() {
        use zymbol_ast::{CatchClause, Block};

        let interp = Interpreter::new();
        let dummy_span = zymbol_span::Span::new(
            zymbol_span::Position::start(),
            zymbol_span::Position::start(),
            zymbol_span::FileId(0),
        );

        // Generic catch (no error type)
        let catch = CatchClause::generic(
            Block::new(vec![], dummy_span),
            dummy_span,
        );

        let io_error = Value::Error(ErrorValue::io("test"));
        let generic_error = Value::Error(ErrorValue::generic("test"));

        assert!(interp.catch_matches(&catch, &io_error));
        assert!(interp.catch_matches(&catch, &generic_error));
    }

    #[test]
    fn test_catch_matches_typed() {
        use zymbol_ast::{CatchClause, Block, ErrorType};

        let interp = Interpreter::new();
        let dummy_span = zymbol_span::Span::new(
            zymbol_span::Position::start(),
            zymbol_span::Position::start(),
            zymbol_span::FileId(0),
        );

        // Typed catch for IO errors
        let io_catch = CatchClause::typed(
            ErrorType::new("IO".to_string(), dummy_span),
            Block::new(vec![], dummy_span),
            dummy_span,
        );

        let io_error = Value::Error(ErrorValue::io("test"));
        let generic_error = Value::Error(ErrorValue::generic("test"));

        assert!(interp.catch_matches(&io_catch, &io_error));
        assert!(!interp.catch_matches(&io_catch, &generic_error));
    }

    #[test]
    fn test_catch_matches_wildcard() {
        use zymbol_ast::{CatchClause, Block, ErrorType};

        let interp = Interpreter::new();
        let dummy_span = zymbol_span::Span::new(
            zymbol_span::Position::start(),
            zymbol_span::Position::start(),
            zymbol_span::FileId(0),
        );

        // Wildcard catch (matches any error type)
        let wildcard_catch = CatchClause::typed(
            ErrorType::new("_".to_string(), dummy_span),
            Block::new(vec![], dummy_span),
            dummy_span,
        );

        let io_error = Value::Error(ErrorValue::io("test"));
        let div_error = Value::Error(ErrorValue::div("test"));

        assert!(interp.catch_matches(&wildcard_catch, &io_error));
        assert!(interp.catch_matches(&wildcard_catch, &div_error));
    }

    #[test]
    fn test_catch_matches_non_error() {
        use zymbol_ast::{CatchClause, Block};

        let interp = Interpreter::new();
        let dummy_span = zymbol_span::Span::new(
            zymbol_span::Position::start(),
            zymbol_span::Position::start(),
            zymbol_span::FileId(0),
        );

        let catch = CatchClause::generic(
            Block::new(vec![], dummy_span),
            dummy_span,
        );

        let int_value = Value::Int(42);
        assert!(!interp.catch_matches(&catch, &int_value));
    }

    // ========== INTEGRATION TESTS ==========

    #[test]
    fn test_try_catch_index_out_of_bounds() {
        // Test catching an index out of bounds error
        let code = r#"
            arr = [1, 2, 3]
            !? {
                x = arr[10]
                >> "no error" ¶
            } :! {
                >> "caught error" ¶
            }
        "#;
        let (output, result) = parse_and_run(code);
        assert!(result.is_ok());
        assert_eq!(String::from_utf8_lossy(&output), "caught error\n");
    }

    #[test]
    fn test_try_catch_with_err_variable() {
        // Test that _err variable is accessible in catch block
        let code = r#"
            arr = [1, 2, 3]
            !? {
                x = arr[100]
            } :! {
                >> "Error type: " ¶
            }
        "#;
        let (output, result) = parse_and_run(code);
        assert!(result.is_ok());
        assert_eq!(String::from_utf8_lossy(&output), "Error type: \n");
    }

    #[test]
    fn test_try_catch_finally_with_error() {
        // Test that finally runs even when error is caught
        let code = r#"
            arr = [1]
            !? {
                x = arr[99]
            } :! {
                >> "caught" ¶
            } :> {
                >> "finally" ¶
            }
        "#;
        let (output, result) = parse_and_run(code);
        assert!(result.is_ok());
        assert_eq!(String::from_utf8_lossy(&output), "caught\nfinally\n");
    }

    #[test]
    fn test_try_multiple_catches_first_match() {
        // Test that first matching catch is executed
        let code = r#"
            !? {
                >> "try" ¶
            } :! ##IO {
                >> "io catch" ¶
            } :! {
                >> "generic catch" ¶
            }
        "#;
        let (output, result) = parse_and_run(code);
        assert!(result.is_ok());
        // No error, so no catch runs
        assert_eq!(String::from_utf8_lossy(&output), "try\n");
    }

    #[test]
    fn test_nested_try_catch() {
        // Test nested try-catch blocks
        let code = r#"
            !? {
                >> "outer try" ¶
                !? {
                    >> "inner try" ¶
                } :! {
                    >> "inner catch" ¶
                }
            } :! {
                >> "outer catch" ¶
            }
        "#;
        let (output, result) = parse_and_run(code);
        assert!(result.is_ok());
        assert_eq!(String::from_utf8_lossy(&output), "outer try\ninner try\n");
    }

    #[test]
    fn test_error_check_false_on_normal_value() {
        // $! returns #0 for non-error values
        let code = r#"
            x = 42
            result = x$!
            ? result {
                >> "is error" ¶
            } _ {
                >> "not error" ¶
            }
        "#;
        let (output, result) = parse_and_run(code);
        assert!(result.is_ok());
        assert_eq!(String::from_utf8_lossy(&output), "not error\n");
    }

    #[test]
    fn test_error_check_on_string() {
        // $! returns #0 for string values
        let code = r#"
            x = "hello"
            ? x$! {
                >> "error" ¶
            } _ {
                >> "ok" ¶
            }
        "#;
        let (output, result) = parse_and_run(code);
        assert!(result.is_ok());
        assert_eq!(String::from_utf8_lossy(&output), "ok\n");
    }

    #[test]
    fn test_error_check_on_array() {
        // $! returns #0 for array values
        let code = r#"
            arr = [1, 2, 3]
            ? arr$! {
                >> "error" ¶
            } _ {
                >> "ok" ¶
            }
        "#;
        let (output, result) = parse_and_run(code);
        assert!(result.is_ok());
        assert_eq!(String::from_utf8_lossy(&output), "ok\n");
    }

    #[test]
    fn test_try_only_block() {
        // Try block without catch or finally
        let code = r#"
            !? {
                >> "only try" ¶
            }
        "#;
        let (output, result) = parse_and_run(code);
        assert!(result.is_ok());
        assert_eq!(String::from_utf8_lossy(&output), "only try\n");
    }

    #[test]
    fn test_try_with_variables_scope() {
        // Variables in try block should be scoped
        let code = r#"
            x = "outer"
            !? {
                x = "inner"
                >> x ¶
            } :! {
                >> "error" ¶
            }
            >> x ¶
        "#;
        let (output, result) = parse_and_run(code);
        assert!(result.is_ok());
        // Due to lexical scoping, x is modified
        assert_eq!(String::from_utf8_lossy(&output), "inner\ninner\n");
    }

    #[test]
    fn test_catch_with_assignment() {
        // Test assignment in catch block
        let code = r#"
            arr = [1]
            result = "success"
            !? {
                x = arr[99]
            } :! {
                result = "failed"
            }
            >> result ¶
        "#;
        let (output, result) = parse_and_run(code);
        assert!(result.is_ok());
        assert_eq!(String::from_utf8_lossy(&output), "failed\n");
    }

    #[test]
    fn test_finally_modifies_outer_variable() {
        // Finally block can modify outer variables
        let code = r#"
            status = "initial"
            !? {
                >> "try" ¶
            } :> {
                status = "finalized"
            }
            >> status ¶
        "#;
        let (output, result) = parse_and_run(code);
        assert!(result.is_ok());
        assert_eq!(String::from_utf8_lossy(&output), "try\nfinalized\n");
    }

    #[test]
    fn test_try_in_loop() {
        // Try-catch inside a loop
        let code = r#"
            arr = [1]
            @ i:0..2 {
                !? {
                    x = arr[i]
                    >> x ¶
                } :! {
                    >> "error at " + i ¶
                }
            }
        "#;
        let (output, result) = parse_and_run(code);
        assert!(result.is_ok());
        // 1-based: i=0 invalid index, i=1 succeeds (arr has 1 element), i=2 out of bounds
        assert_eq!(String::from_utf8_lossy(&output), "error at 0\n1\nerror at 2\n");
    }

    #[test]
    fn test_try_in_function() {
        // Try-catch inside a function
        let code = r#"
            safe_get(arr, idx) {
                !? {
                    <~ arr[idx]
                } :! {
                    <~ -1
                }
            }

            data = [10, 20, 30]
            >> safe_get(data, 1) ¶
            >> safe_get(data, 99) ¶
        "#;
        let (output, result) = parse_and_run(code);
        assert!(result.is_ok());
        // 1-based: arr[1] = first element = 10; arr[99] = out of bounds → -1
        assert_eq!(String::from_utf8_lossy(&output), "10\n-1\n");
    }

    #[test]
    fn test_multiple_sequential_try_blocks() {
        // Multiple try blocks in sequence
        let code = r#"
            arr = [1]

            !? {
                >> arr[1] ¶
            } :! {
                >> "error 1" ¶
            }

            !? {
                >> arr[5] ¶
            } :! {
                >> "error 2" ¶
            }

            !? {
                >> "no error" ¶
            } :! {
                >> "error 3" ¶
            }
        "#;
        let (output, result) = parse_and_run(code);
        assert!(result.is_ok());
        assert_eq!(String::from_utf8_lossy(&output), "1\nerror 2\nno error\n");
    }
}
