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
mod index_nav;
mod stdlib;

/// What each `std/` module registers at run time — see
/// [`stdlib::registered_names`]. Used to keep `zymbol_common::stdlib` honest.
pub use stdlib::registered_names as stdlib_registered_names;

pub(crate) use modules::LoadedModule;

/// Runtime errors
#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error("{message}")]
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

    #[error("E004: Circular import detected: module '{module}' is already being loaded")]
    CircularImport { module: String },

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

/// Function definition — Zymbol source function or native Rust function.
#[derive(Clone)]
enum FunctionDef {
    Zymbol {
        parameters: Vec<zymbol_ast::Parameter>,
        body: zymbol_ast::Block,
        /// Path of the module where this function was defined.
        /// Used to restore the correct scope when a function is called through a re-export adapter.
        origin_module_path: Option<PathBuf>,
        /// Auto-free (v0.0.8): body statement index → variables destroyed
        /// after that statement finishes normally (last-use analysis).
        auto_free: HashMap<usize, Vec<String>>,
    },
    Native {
        name:  &'static str,
        arity: i8,  // expected argument count; -1 = variadic
        func:  fn(Vec<Value>, zymbol_span::Span) -> Result<Value>,
    },
}

impl std::fmt::Debug for FunctionDef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FunctionDef::Zymbol { parameters, .. } =>
                write!(f, "FunctionDef::Zymbol({})", parameters.len()),
            FunctionDef::Native { name, arity, .. } =>
                write!(f, "FunctionDef::Native({}, arity={})", name, arity),
        }
    }
}


/// Error value for error handling
/// Represents a runtime error that can be caught with try-catch
#[derive(Debug, Clone, PartialEq)]
pub struct ErrorValue {
    /// Error type: "IO", "Network", "DB", "Parse", "Index", "Type", "Div", "_" (generic)
    pub error_type: String,
    /// Error message
    pub message: String,
}

use zymbol_common::typesym;

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

    /// Create a DB (database / ODBC) error
    #[cfg(feature = "db")]
    pub fn db(message: impl Into<String>) -> Self {
        Self::new("DB", message)
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

    /// Create a Range error — an integer that left `zymbol_common::num`'s range.
    /// Raised by arithmetic, by the `###` cast, and by any reader that turns
    /// outside data into an integer.
    pub fn range(message: impl Into<String>) -> Self {
        Self::new("Range", message)
    }

    /// Create a Parse error
    pub fn parse(message: impl Into<String>) -> Self {
        Self::new("Parse", message)
    }

    /// Create a Key error — a dictionary key that is not there.
    ///
    /// Decision 10 of `Divergente_ES/forma/README.md`: reading an absent key is
    /// an error, not `##_`. It is Python's `KeyError`, not JavaScript's
    /// `undefined`, and it is coherent with `a[0]`, which is also an error
    /// rather than a silently wrong answer.
    pub fn key(message: impl Into<String>) -> Self {
        Self::new("Key", message)
    }
}

/// The type symbol a value carries, before any refinement.
///
/// "Base" because an array is always [`typesym::ARRAY`] here, whatever its
/// elements hold: this is what error messages name, and a failed destructuring
/// is about the shape rather than about the mix. `#?` refines it —
/// [`type_symbol_of`] — and is the only thing that does.
pub(crate) fn base_type_symbol(value: &Value) -> String {
    match value {
        Value::Int(_) => typesym::INT.to_string(),
        Value::Float(_) => typesym::FLOAT.to_string(),
        Value::String(_) => typesym::STRING.to_string(),
        Value::Char(_) => typesym::CHAR.to_string(),
        Value::Bool(_) => typesym::BOOL.to_string(),
        Value::Array(_) => typesym::ARRAY.to_string(),
        Value::Tuple(_) => typesym::TUPLE.to_string(),
        Value::NamedTuple(_) => typesym::DICT.to_string(),
        Value::Function(f) => if f.is_named_fn { typesym::FUNCTION.to_string() } else { typesym::LAMBDA.to_string() },
        Value::Error(err) => format!("##{}", err.error_type),
        Value::Unit => typesym::UNIT.to_string(),
    }
}

/// What `#?` answers: [`base_type_symbol`], except that an array whose elements
/// are not all one type is a list, [`typesym::LIST`].
///
/// The mix is read from the value **now**, not from how the literal was written:
/// `#[…]` declares a mix to the analyzer and leaves no trace on the value, so
/// `json::decode`'s heterogeneous array answers `##[` without any mark, and
/// `#[1, "dos"]$-[2]` answers `##]` because a single Int is not a mix.
pub(crate) fn type_symbol_of(value: &Value) -> String {
    match value {
        Value::Array(items) => {
            typesym::array_symbol(items.iter().map(|v| base_type_symbol(v))
                .collect::<Vec<_>>().iter().map(String::as_str)).to_string()
        }
        other => base_type_symbol(other),
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

/// The module aliases visible at one point in the program (alias -> file path).
///
/// Behind an `Rc` because every function value carries the set that was visible
/// where it was written, and every call frame swaps one in: without sharing,
/// creating a lambda inside a loop would deep-copy the map on each iteration.
pub type ModuleAliases = std::rc::Rc<std::collections::HashMap<String, std::path::PathBuf>>;

/// What makes two function values **the same function** (BUG-ZYB-012).
///
/// `a = uno` and `b = uno` name one function and must compare equal; two
/// functions with identical bodies must not. Neither answer falls out of the
/// data by itself: a named function is turned into a value afresh on every
/// lookup — new captures, cloned body — so pointer equality on the value fails,
/// and structural equality would call two identical definitions the same
/// function, which is exactly what identity is for.
///
/// So identity is carried explicitly, and it is the thing that was *written*:
/// the definition for a named function, the evaluation for a lambda.
#[derive(Debug, Clone)]
pub(crate) enum FnIdentity {
    /// A named function: the `FunctionDef` it came from. Looking the name up
    /// twice yields the same `Rc`, so two values built from it are one function.
    Named(std::rc::Rc<FunctionDef>),
    /// A lambda: the evaluation that created it. Cloning the value keeps the
    /// number, so two names for one lambda agree; evaluating the expression a
    /// second time makes a different closure, which it is.
    Lambda(u64),
    /// A native `std/` function reached as a value — not constructible today.
    Native,
}

impl PartialEq for FnIdentity {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (FnIdentity::Named(a), FnIdentity::Named(b)) => std::rc::Rc::ptr_eq(a, b),
            (FnIdentity::Lambda(a), FnIdentity::Lambda(b)) => a == b,
            // A named function and a lambda are never the same function, and
            // two natives have nothing to compare.
            _ => false,
        }
    }
}

/// The next lambda identity. One counter for the process: a lambda evaluated in
/// a loop is a new closure each time round, and each one is itself.
pub(crate) fn next_lambda_identity() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(1);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

/// Function value for lambdas and closures
#[derive(Debug, Clone)]
pub struct FunctionValue {
    pub params: Vec<String>,
    pub body: zymbol_ast::LambdaBody,
    pub captures: std::rc::Rc<std::collections::HashMap<String, Value>>,  // Shared closure env (Rc → O(1) clone)
    /// True when this value was created from a named FunctionDecl used as a first-class value.
    /// Named functions may complete their block without <~ and return Unit (unlike block lambdas).
    pub is_named_fn: bool,
    /// What makes this the same function as another — see [`FnIdentity`].
    ///
    /// Crate-private because it names `FunctionDef`, which is: the identity is
    /// something the engine decides and nothing outside constructs.
    pub(crate) identity: FnIdentity,
    /// The module aliases visible where this function was written.
    ///
    /// Restored for the duration of the call, so `alias::fn` inside the body
    /// means what it meant at the definition site. Anonymous lambdas used to
    /// leave this empty and inherit the *caller's* aliases instead, which is
    /// the same thing only while the lambda is called from where it was
    /// defined — crossing into another module lost them (BUG-ZYB-001).
    pub module_aliases: ModuleAliases,
}

impl PartialEq for FunctionValue {
    /// Two function values are equal when they are the same function.
    ///
    /// This compared `params` until v0.0.9, which made every one-argument
    /// function equal to every other — and no caller ever saw it, because
    /// `values_equal_static` had no arm for `Function` and answered `#0` to all
    /// of them, including a function against itself (BUG-ZYB-012).
    fn eq(&self, other: &Self) -> bool {
        self.identity == other.identity
    }
}

impl Value {
    /// Convert value to displayable string, in ASCII numerals.
    ///
    /// Callers with access to the active numeral mode should use
    /// `Interpreter::format_value` (or `to_display_string_in`) instead — this
    /// bare form is for contexts that have no interpreter to read the mode from.
    pub fn to_display_string(&self) -> String {
        self.to_display_string_in(numeral_mode::ASCII_BASE)
    }

    /// Convert value to displayable string with every digit rendered in the
    /// numeral system identified by `block_base`.
    ///
    /// The mode reaches *inside* collections: an array of Ints under `#०९#`
    /// renders each element in Devanagari, because a number does not stop being
    /// a number by sitting in a list. Brackets, commas, the `-` sign and the
    /// decimal `.` stay ASCII; strings and chars are never touched.
    pub fn to_display_string_in(&self, block_base: u32) -> String {
        // Standalone Unit prints as nothing, but INSIDE a collection it must be
        // visible — `[1, , 3]` reads like a typo. Both engines render nested
        // Unit as `()` (the VM always did; the tree-walker since 2026-06-12).
        fn nested(v: &Value, block_base: u32) -> String {
            match v {
                Value::Unit => "()".to_string(),
                other => other.to_display_string_in(block_base),
            }
        }
        match self {
            Value::String(s) => s.clone(),
            Value::Int(n) => numeral_mode::to_numeral_int(*n, block_base),
            Value::Float(f) => numeral_mode::to_numeral_float(*f, block_base),
            Value::Char(c) => c.to_string(),
            Value::Bool(b) => numeral_mode::to_numeral_bool(*b, block_base),
            Value::Array(elements) => {
                let contents = elements
                    .iter()
                    .map(|v| nested(v, block_base))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("[{}]", contents)
            }
            Value::Tuple(elements) => {
                let contents = elements
                    .iter()
                    .map(|v| nested(v, block_base))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("({})", contents)
            }
            Value::NamedTuple(fields) => {
                // `#(…)`, the way the literal is written. A dictionary printed as
                // `(a: 1)` could not be typed back in: that spelling is refused
                // since v0.0.9, and `()` would be the empty tuple as well.
                let contents = fields
                    .iter()
                    .map(|(name, value)| format!("{}: {}", name, nested(value, block_base)))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("#({})", contents)
            }
            Value::Function(f) => {
                if f.is_named_fn {
                    format!("<funct/{}>", f.params.len())
                } else {
                    format!("<lambd/{}>", f.params.len())
                }
            }
            Value::Error(err) => {
                format!("##{}({})", err.error_type, err.message)
            }
            Value::Unit => "".to_string(),
        }
    }

    /// Repr form: like `to_display_string` but with delimiters that make the
    /// type unambiguous — strings get `"..."`, chars get `'...'`, Unit shows
    /// as `()`.  Used by the REPL to display evaluated expression results.
    pub fn to_repr_string(&self) -> String {
        self.to_repr_string_in(numeral_mode::ASCII_BASE)
    }

    /// Repr form rendered in the numeral system identified by `block_base`.
    pub fn to_repr_string_in(&self, block_base: u32) -> String {
        match self {
            Value::String(s) => format!("\"{}\"", s),
            Value::Char(c)   => format!("'{}'", c),
            Value::Unit      => "()".to_string(),
            Value::Array(elements) => {
                let contents = elements
                    .iter()
                    .map(|v| v.to_repr_string_in(block_base))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("[{}]", contents)
            }
            Value::Tuple(elements) => {
                let contents = elements
                    .iter()
                    .map(|v| v.to_repr_string_in(block_base))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("({})", contents)
            }
            Value::NamedTuple(fields) => {
                let contents = fields
                    .iter()
                    .map(|(name, v)| format!("{}: {}", name, v.to_repr_string_in(block_base)))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("#({})", contents)
            }
            _ => self.to_display_string_in(block_base),
        }
    }

    /// Readable type name for diagnostics, as opposed to [`Self::type_name`],
    /// which yields the language's `##` type symbol. Mirrored zyml's `type_name`
    /// and the VM's `type_word`, so a message naming a type reads the same
    /// whichever engine produced it.
    pub fn type_word(&self) -> &'static str {
        match self {
            Value::Int(_)        => "integer",
            Value::Float(_)      => "float",
            Value::Bool(_)       => "bool",
            Value::String(_)     => "string",
            Value::Char(_)       => "char",
            Value::Array(_)      => "array",
            Value::Tuple(_) | Value::NamedTuple(_) => "tuple",
            Value::Function(f)   => if f.is_named_fn { "function" } else { "lambda" },
            Value::Error(_)      => "error",
            Value::Unit          => "unit",
        }
    }

    /// The type as a diagnostic spells it — `Int`, `String`, `Char` — as
    /// opposed to [`Self::type_word`] (prose: `integer`) and [`Self::type_name`]
    /// (the language's `##` symbol).
    ///
    /// It is the VM's `type_name`, and it exists here so that a message naming
    /// a type is one text across the engines rather than two that happen to
    /// agree (GLOBAL-001).
    pub fn type_ident(&self) -> &'static str {
        match self {
            Value::Int(_)        => "Int",
            Value::Float(_)      => "Float",
            Value::String(_)     => "String",
            Value::Char(_)       => "Char",
            Value::Bool(_)       => "Bool",
            Value::Array(_)      => "Array",
            Value::Tuple(_)      => "Tuple",
            Value::NamedTuple(_) => "Dict",
            Value::Function(_)   => "Function",
            Value::Error(_)      => "Error",
            Value::Unit          => "Unit",
        }
    }

    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Int(_)        => "###",
            Value::Float(_)      => "##.",
            Value::String(_)     => "##\"",
            Value::Char(_)       => "##'",
            Value::Bool(_)       => "##?",
            Value::Array(_)      => "##[]",
            Value::Tuple(_)      => "##()",
            Value::NamedTuple(_) => "##(name:)",
            Value::Function(_)   => "##fn",
            Value::Error(_)      => "##!",
            Value::Unit          => "##_",
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
    /// Indices into scope_stack where @ loop scopes start.
    /// Used by x° (set_at_nearest_loop) and °x (set_above_nearest_loop).
    loop_scope_depths: Vec<usize>,
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
    /// Modules currently being loaded (for circular import detection)
    loading_modules: HashSet<PathBuf>,
    /// Import aliases (alias -> file_path)
    import_aliases: ModuleAliases,
    /// The module variables injected into the CURRENT frame, as they were at
    /// injection time.
    ///
    /// Two things read it. The write-back when the frame returns diffs against
    /// it, so a frame that never touched a key cannot clobber what a nested
    /// call wrote (MM-2). And a call to another function of the same module
    /// flushes the difference to the store on the way in, so the callee sees
    /// what this frame has written rather than what the store last heard
    /// (MM-12 — see `flush_module_frame`).
    frame_module_vars: HashMap<String, Value>,
    /// Current file path (for resolving relative imports)
    current_file: Option<PathBuf>,
    /// Base directory for module resolution
    base_dir: PathBuf,
    /// The code a top-level `<~ n` asked the program to end with (GAP-ZYB-006).
    ///
    /// `<~` hands a value back to whoever called; a program is called by the
    /// operating system, so a value handed back at the top level is its exit
    /// status. That derivation is why this needed no new symbol — and the
    /// register VM and the browser engine already stopped the program here,
    /// while this engine walked past it and ran the rest of the file.
    exit_code: Option<i64>,
    /// CLI arguments passed to the script
    cli_args: Option<Vec<Value>>,
    /// Auto-free (v0.0.8): top-level statement index → variables to destroy
    /// after that statement (last-use analysis, computed in `execute()`)
    destruction_schedule: HashMap<usize, Vec<String>>,
    /// Dead variables: variables that have been destroyed (for use-after-free detection)
    dead_variables: HashSet<String>,
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
    /// Which of a module's bindings each module function body actually names,
    /// keyed by the address of its `Rc<FunctionDef>`.
    ///
    /// A module function frame is given a copy of the module's state on entry
    /// and diffs it on the way out. The tree-walker's collections are not
    /// reference-counted, so both halves are deep copies, and the cost was
    /// proportional to the whole of the module's state rather than to the part
    /// the function touches — a module holding a sixty-key table paid for it on
    /// every call, including calls to functions that never name the table.
    /// Computed once per function body and reused (REFERENCE.md L44).
    module_var_mentions: HashMap<usize, std::rc::Rc<std::collections::HashSet<String>>>,
    /// MoveOrClone guard: depth of active try/catch blocks.
    /// When > 0, Return must clone (finally block may reference the variable after <~).
    /// When == 0, Return can move (take_variable) — O(1) for String/Array.
    try_depth: u8,
    /// Depth of active >>| TUI blocks. When > 0, raw mode is active and ¶/\\ must emit \r\n.
    pub(crate) tui_depth: u8,
    /// TCO support: name of the currently executing function (None = not in a function).
    /// Used to detect `<~ f(same_args)` tail-call patterns.
    pub(crate) current_function: Option<String>,
    /// TCO restart: when true, function execution restarts with rebound params.
    pub(crate) tco_pending: bool,
    /// TCO args: the rebound argument values for the tail call restart.
    pub(crate) tco_args: Vec<Value>,
    /// Names the MoveOrClone optimisation in `Statement::Return` must NOT move
    /// out of scope, because something after the return still needs to read them:
    ///   - output parameters (QW13), whose writeback copies the value to the caller;
    ///   - module state variables (MM-2), whose write-back diffs the frame's final
    ///     value against the injected snapshot. `<~ v` used to move `v` out, the
    ///     write-back then found nothing, and the mutation was silently dropped —
    ///     a module function that wrote state *and* returned a value lost the write
    ///     in the tree-walker while the VM kept it.
    pub(crate) move_guard_names: std::collections::HashSet<String>,
    /// Active output numeral system (block base codepoint).
    /// Default: 0x0030 (ASCII). Changed by #<d0><d9># statements.
    /// Applies to every path that turns an Int/Float/Bool into text — `>>`,
    /// `>>~`, string interpolation, juxtaposition, `$++` and the elements nested
    /// inside arrays and tuples. That includes text the program then uses as
    /// data (a shell command, a file name): the mode is a statement of intent
    /// about how this program writes numbers, and validating that is the
    /// developer's responsibility. Only the bare `Value::to_display_string()`
    /// (no numeral_mode field to read) stays ASCII — every call site with
    /// `&self` access routes through the active mode instead.
    pub(crate) numeral_mode: u32,
    /// Called by `<<` (input) statements to read one line from the user.
    /// Receives no arguments; the prompt is printed by execute_input via self.output
    /// before invoking this function.  The default implementation reads from stdin.
    /// Override in the REPL to temporarily exit raw mode while the user types.
    pub(crate) input_fn: Box<dyn FnMut() -> std::io::Result<String>>,
    /// MM-9: constants declared with `:=` at the root scope of top-level code.
    /// NOT swapped by take_call_state — visible inside script functions at any
    /// call depth (including through lambda frames). Module function frames do
    /// not consult this table: modules only see their own state.
    global_consts: HashMap<String, Value>,
    /// MM-2: path of the module whose function frame is currently executing
    /// (None = script code). Saved/restored across call boundaries; used to
    /// sync module state between nested calls into the same module.
    pub(crate) current_module_path: Option<PathBuf>,
    /// MM-9: call-frame depth — 0 while executing top-level statements.
    /// Distinguishes the root scope from a function frame's bottom scope.
    call_depth: usize,
    /// The file body's variables, reachable at any call depth.
    ///
    /// A named function CAPTURES what its body reads from the scope it was
    /// written in, exactly as a lambda does (ERROR-ZYB-002). That scope is the
    /// file body — and `take_call_state` swaps the whole scope stack away on
    /// every call, so by the time a function two frames down is entered there is
    /// nothing left to read it from.
    ///
    /// Mirroring on write costs O(1) per top-level assignment, which is the
    /// cheap side of the trade: the alternative was cloning the scope stack on
    /// every call, and a program that calls in a loop would pay it every
    /// iteration.
    ///
    /// Only writes that land in the file body itself are mirrored — a block's
    /// locals are not the file's, and a named function is written at file level
    /// so it cannot see them anyway.
    file_vars: HashMap<String, Value>,
    /// The free names of each named function's body, computed once per
    /// definition instead of once per call.
    ///
    /// Capturing (above) needs to know what a body reads from outside itself,
    /// and that answer walks the whole body AST. It depends on the DEFINITION
    /// alone, so a recursive function was re-deriving its own answer on every
    /// invocation: `bench_recursion` lost 32% the day named functions started
    /// capturing. The JavaScript engine already cached it on the function
    /// object; this is the same cache, keyed by definition address.
    ///
    /// The `Rc` is kept in the map so that address cannot be reused by a later
    /// allocation while an entry for it is still live.
    free_names_cache: HashMap<usize, (Rc<FunctionDef>, Rc<Vec<String>>)>,
    /// Auto-free (v0.0.8): names destroyed by the last-use schedule in the
    /// CURRENT frame. Separate from `dead_variables` so an analyzer bug
    /// surfaces as a distinctive internal error, never as a user-facing `\`
    /// lifetime error. Frame-local (saved/restored in SavedCallState).
    auto_dead_variables: HashSet<String>,
    /// Auto-free (v0.0.8): program-wide exclusion set (hot names, constants,
    /// free variables of value-used named functions, module bindings).
    /// Computed once per `execute()`; consulted when registering functions.
    auto_free_excluded: Rc<HashSet<String>>,
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

    /// Push a loop-anchor scope for a `@` loop and record its depth.
    pub(crate) fn push_loop_scope(&mut self) {
        self.push_scope();
        self.loop_scope_depths.push(self.scope_stack.len() - 1);
    }

    /// Pop the loop-anchor scope when a `@` loop ends.
    pub(crate) fn pop_loop_scope(&mut self) {
        self.loop_scope_depths.pop();
        self.pop_scope();
    }

    /// `x°`: write variable to nearest enclosing `@` scope.
    /// Variable lives for the loop duration, dies when the loop ends.
    pub(crate) fn set_at_nearest_loop(&mut self, name: &str, value: Value) {
        if let Some(&idx) = self.loop_scope_depths.last() {
            self.scope_stack[idx].insert(name.to_string(), value);
        } else {
            self.scope_stack[0].insert(name.to_string(), value);
        }
    }

    /// `°x`: write variable to the scope ABOVE the nearest `@`.
    /// Variable survives the loop (anchors to next outer loop or global/function scope).
    pub(crate) fn set_above_nearest_loop(&mut self, name: &str, value: Value) {
        let len = self.loop_scope_depths.len();
        if len >= 2 {
            let idx = self.loop_scope_depths[len - 2];
            self.scope_stack[idx].insert(name.to_string(), value);
        } else {
            // Single loop or no loop: anchor to global/function bottom scope
            self.scope_stack[0].insert(name.to_string(), value);
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
        // MM-9: root-scope constants are globally visible in script code
        // regardless of call depth. Module frames skip the fallback —
        // modules only see their own state.
        if self.current_module_path.is_none() {
            return self.global_consts.get(name);
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
        // A new assignment after explicit destruction (`\var`) resurrects the variable.
        if !self.dead_variables.is_empty() {
            self.dead_variables.remove(name);
        }
        if !self.auto_dead_variables.is_empty() {
            self.auto_dead_variables.remove(name);
        }
        let at_file_level = self.call_depth == 0 && self.scope_stack.len() == 1;
        if let Some(scope) = self.scope_stack.last_mut() {
            scope.insert(name.to_string(), value.clone());
        }
        if at_file_level {
            self.file_vars.insert(name.to_string(), value);
        }
    }

    /// Move a variable's value out of the scope (replace with Unit, return owned Value).
    /// MoveOrClone: O(1) for all types including String/Array — no heap allocation.
    /// Only safe when the variable will not be referenced again (e.g., on Return).
    #[inline(always)]
    pub(crate) fn take_variable(&mut self, name: &str) -> Option<Value> {
        // Remove the entry entirely rather than replacing with Unit sentinel.
        // A Unit sentinel would be written back to module.all_variables on write-back,
        // corrupting module constants returned via bare-identifier <~ CONST expressions.
        // After a Return statement the variable is unreachable anyway, so removal is safe.
        for scope in self.scope_stack.iter_mut().rev() {
            if scope.contains_key(name) {
                return scope.remove(name);
            }
        }
        None
    }

    /// Set a variable value in the appropriate scope.
    /// B9: zero allocation on the UPDATE path (hot path).
    #[inline(always)]
    fn set_variable(&mut self, name: &str, value: Value) {
        // A new assignment after explicit destruction (`\var`) resurrects the variable.
        if !self.dead_variables.is_empty() {
            self.dead_variables.remove(name);
        }
        if !self.auto_dead_variables.is_empty() {
            self.auto_dead_variables.remove(name);
        }
        let at_depth_zero = self.call_depth == 0;
        for (i, scope) in self.scope_stack.iter_mut().enumerate().rev() {
            if let Some(existing) = scope.get_mut(name) {
                *existing = value.clone();
                if at_depth_zero && i == 0 {
                    self.file_vars.insert(name.to_string(), value);
                }
                return;
            }
        }
        let at_file_level = at_depth_zero && self.scope_stack.len() == 1;
        if let Some(scope) = self.scope_stack.last_mut() {
            scope.insert(name.to_string(), value.clone());
        }
        if at_file_level {
            self.file_vars.insert(name.to_string(), value);
        }
    }

    /// Check if a variable is a constant in any scope.
    #[inline(always)]
    pub(crate) fn is_const(&self, name: &str) -> bool {
        // B8: short-circuit
        if !self.has_any_const && self.global_consts.is_empty() { return false; }
        // MM-9: root-scope constants stay immutable at any call depth
        if self.current_module_path.is_none() && self.global_consts.contains_key(name) {
            return true;
        }
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
    pub(crate) fn mark_const(&mut self, name: String) {
        self.has_any_const = true;  // B8: activate flag
        if let Some(current_const_set) = self.const_vars_stack.last_mut() {
            current_const_set.insert(name);
        }
    }

    /// Remove a const mark from every scope of the current frame.
    /// Used when binding a parameter whose name shadows a forwarded constant —
    /// the parameter must stay assignable inside the function body.
    pub(crate) fn unmark_const(&mut self, name: &str) {
        if !self.has_any_const { return; }
        for const_set in self.const_vars_stack.iter_mut() {
            const_set.remove(name);
        }
    }

    /// True while executing a top-level statement in the root scope
    /// (not inside any function/lambda frame and not inside a block).
    pub(crate) fn is_root_scope(&self) -> bool {
        self.call_depth == 0 && self.scope_stack.len() == 1
    }

    /// MM-9: record a root-scope constant in the global table.
    pub(crate) fn record_global_const(&mut self, name: String, value: Value) {
        self.global_consts.insert(name, value);
    }

    /// Names declared as constants in the root scope. Captured by module
    /// loading so module constants stay immutable inside module functions.
    pub(crate) fn root_const_names(&self) -> HashSet<String> {
        let mut names: HashSet<String> =
            self.const_vars_stack.first().cloned().unwrap_or_default();
        names.extend(self.global_consts.keys().cloned());
        names
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
            frame_module_vars: std::mem::take(&mut self.frame_module_vars),
            has_any_const: self.has_any_const,
            // MM-1: loop anchors index into the caller's scope_stack — they must
            // not leak into the callee frame or x°/°x would write out of bounds.
            loop_scope_depths: std::mem::take(&mut self.loop_scope_depths),
            // MM-3: destroyed names are frame-local — a `\ x` inside the callee
            // must not poison the caller's own `x`.
            dead_variables: std::mem::take(&mut self.dead_variables),
            auto_dead_variables: std::mem::take(&mut self.auto_dead_variables),
            // MM-2: module context is frame-local.
            current_module_path: self.current_module_path.take(),
        };
        self.call_depth += 1;
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
        self.frame_module_vars = saved.frame_module_vars;
        self.has_any_const = saved.has_any_const;
        self.loop_scope_depths = saved.loop_scope_depths;      // MM-1
        self.dead_variables = saved.dead_variables;            // MM-3
        self.auto_dead_variables = saved.auto_dead_variables;  // auto-free
        self.current_module_path = saved.current_module_path;  // MM-2
        self.call_depth = self.call_depth.saturating_sub(1);

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
    pub(crate) scope_stack: Vec<HashMap<String, Value>>,
    mutable_vars_stack: Vec<HashSet<String>>,
    pub(crate) const_vars_stack: Vec<HashSet<String>>,
    pub(crate) import_aliases: ModuleAliases,
    pub(crate) frame_module_vars: HashMap<String, Value>,
    has_any_const: bool,
    loop_scope_depths: Vec<usize>,
    dead_variables: HashSet<String>,
    auto_dead_variables: HashSet<String>,
    pub(crate) current_module_path: Option<PathBuf>,
}

fn default_input_fn() -> Box<dyn FnMut() -> std::io::Result<String>> {
    Box::new(|| {
        let mut buf = String::new();
        std::io::stdin().read_line(&mut buf)?;
        Ok(buf)
    })
}

impl Interpreter<std::io::Stdout> {
    pub fn new() -> Self {
        Self {
            output: std::io::stdout(),
            scope_stack: vec![HashMap::new()],  // Start with one global scope
            file_vars: HashMap::new(),
            free_names_cache: HashMap::new(),
            loop_scope_depths: Vec::new(),
            functions: HashMap::new(),
            control_flow: ControlFlow::None,
            mutable_vars_stack: vec![HashSet::new()],
            const_vars_stack: vec![HashSet::new()],
            loaded_modules: HashMap::new(),
            loading_modules: HashSet::new(),
            import_aliases: ModuleAliases::default(),
            frame_module_vars: HashMap::new(),
            current_file: None,
            base_dir: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            exit_code: None,
            cli_args: None,
            destruction_schedule: HashMap::new(),
            dead_variables: HashSet::new(),
            has_any_const: false,
            has_control_flow: false,
            scope_map_pool: Vec::new(),
            mut_set_pool: Vec::new(),
            const_set_pool: Vec::new(),
            scope_vec_pool: Vec::new(),
            mut_vec_pool: Vec::new(),
            const_vec_pool: Vec::new(),
            arg_vec_pool: Vec::new(),
            module_var_mentions: HashMap::new(),
            try_depth: 0,
            tui_depth: 0,
            current_function: None,
            tco_pending: false,
            tco_args: Vec::new(),
            move_guard_names: std::collections::HashSet::new(),
            numeral_mode: numeral_mode::ASCII_BASE,
            input_fn: default_input_fn(),
            global_consts: HashMap::new(),
            current_module_path: None,
            call_depth: 0,
            auto_dead_variables: HashSet::new(),
            auto_free_excluded: Rc::new(HashSet::new()),
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
            file_vars: HashMap::new(),
            free_names_cache: HashMap::new(),
            loop_scope_depths: Vec::new(),
            functions: HashMap::new(),
            control_flow: ControlFlow::None,
            mutable_vars_stack: vec![HashSet::new()],
            const_vars_stack: vec![HashSet::new()],
            loaded_modules: HashMap::new(),
            loading_modules: HashSet::new(),
            import_aliases: ModuleAliases::default(),
            frame_module_vars: HashMap::new(),
            current_file: None,
            base_dir: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            exit_code: None,
            cli_args: None,
            destruction_schedule: HashMap::new(),
            dead_variables: HashSet::new(),
            has_any_const: false,
            has_control_flow: false,
            scope_map_pool: Vec::new(),
            mut_set_pool: Vec::new(),
            const_set_pool: Vec::new(),
            scope_vec_pool: Vec::new(),
            mut_vec_pool: Vec::new(),
            const_vec_pool: Vec::new(),
            arg_vec_pool: Vec::new(),
            module_var_mentions: HashMap::new(),
            try_depth: 0,
            tui_depth: 0,
            current_function: None,
            tco_pending: false,
            tco_args: Vec::new(),
            move_guard_names: std::collections::HashSet::new(),
            numeral_mode: numeral_mode::ASCII_BASE,
            input_fn: default_input_fn(),
            global_consts: HashMap::new(),
            current_module_path: None,
            call_depth: 0,
            auto_dead_variables: HashSet::new(),
            auto_free_excluded: Rc::new(HashSet::new()),
        }
    }

    /// Override the input callback used by `<<` statements.
    /// The provided function must read one line and return it (including the trailing newline).
    pub fn set_input_fn(&mut self, f: impl FnMut() -> std::io::Result<String> + 'static) {
        self.input_fn = Box::new(f);
    }

    pub fn writer(&self) -> &W { &self.output }
    pub fn writer_mut(&mut self) -> &mut W { &mut self.output }

    pub fn flush_output(&mut self) -> std::io::Result<()> {
        self.output.flush()
    }

    /// Set the current file path for module resolution
    pub fn set_current_file<P: AsRef<Path>>(&mut self, path: P) {
        self.current_file = Some(path.as_ref().to_path_buf());
    }

    /// Set the base directory for module resolution
    /// The exit status a top-level `<~ n` asked for, if the program asked
    /// (GAP-ZYB-006).
    pub fn exit_code(&self) -> Option<i64> {
        self.exit_code
    }

    pub fn set_base_dir<P: AsRef<Path>>(&mut self, path: P) {
        self.base_dir = path.as_ref().to_path_buf();
    }

    /// Destroy a variable immediately (remove from all scopes and mark as dead)
    fn destroy_variable(&mut self, var_name: &str) {
        // A destroyed root constant must not resurrect through the global table.
        if !self.global_consts.is_empty() {
            self.global_consts.remove(var_name);
        }
        // Remove from all scopes (search from innermost to outermost)
        for scope in self.scope_stack.iter_mut().rev() {
            if scope.remove(var_name).is_some() {
                // Found and removed - mark as dead
                self.dead_variables.insert(var_name.to_string());
                return;
            }
        }
    }

    /// Auto-free (v0.0.8): destroy a variable scheduled after its last use.
    /// Invisible by design — the analyzer only schedules provably dead names.
    /// Marked in `auto_dead_variables` (not `dead_variables`) so an analyzer
    /// bug surfaces as a distinctive internal error.
    fn auto_destroy_variable(&mut self, name: &str) {
        // Defense in depth: constants are excluded by the analyzer already.
        if self.is_const(name) {
            return;
        }
        for scope in self.scope_stack.iter_mut().rev() {
            if scope.remove(name).is_some() {
                self.auto_dead_variables.insert(name.to_string());
                return;
            }
        }
    }

    /// Check if a variable has been destroyed (use-after-free detection)
    fn check_variable_alive(&self, var_name: &str, span: &Span) -> Result<()> {
        // B8: short-circuit
        if self.dead_variables.is_empty() && self.auto_dead_variables.is_empty() {
            return Ok(());
        }
        if self.dead_variables.contains(var_name) {
            return Err(RuntimeError::Generic {
                message: format!(
                    "use after destruction: variable '{}' was destroyed after its last use",
                    var_name
                ),
                span: *span,
            });
        }
        // Auto-free is invisible by design — reaching this error means the
        // last-use analyzer scheduled a destruction too early.
        if self.auto_dead_variables.contains(var_name) {
            return Err(RuntimeError::Generic {
                message: format!(
                    "internal: use of '{}' after auto-destruction — this is a bug in the last-use analyzer, please report it (workaround: add a later `>> {}` mention or a `\\ {}` at the intended end of life)",
                    var_name, var_name, var_name
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

    /// Reset interpreter scope: clears all variables, functions, and aliases.
    /// Keeps the output writer and any already-loaded modules.
    pub fn reset_scope(&mut self) {
        self.scope_stack.clear();
        self.scope_stack.push(HashMap::new());
        self.mutable_vars_stack.clear();
        self.mutable_vars_stack.push(HashSet::new());
        self.const_vars_stack.clear();
        self.const_vars_stack.push(HashSet::new());
        self.functions.clear();
        self.dead_variables.clear();
        self.import_aliases = ModuleAliases::default();
        self.frame_module_vars.clear();
        self.loading_modules.clear();
        self.loop_scope_depths.clear();
        self.has_any_const = false;
        self.has_control_flow = false;
        self.control_flow = ControlFlow::None;
        self.tco_pending = false;
        self.tco_args.clear();
        self.current_function = None;
        self.numeral_mode = 0x0030;
        self.tui_depth = 0;
        self.try_depth = 0;
        self.global_consts.clear();
        self.current_module_path = None;
        self.call_depth = 0;
        self.auto_dead_variables.clear();
        self.destruction_schedule.clear();
        self.auto_free_excluded = Rc::new(HashSet::new());
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
                    format!("{}({})", typesym::TUPLE, types.join(", "))
                }
                Value::NamedTuple(fields) => {
                    let types: Vec<String> = fields
                        .iter()
                        .map(|(name, val)| format!("{}: {}", name, self.value_type_name(val)))
                        .collect();
                    format!("{}({})", typesym::DICT, types.join(", "))
                }
                Value::Function(f) => if f.is_named_fn { "##()".to_string() } else { "##->".to_string() },
                Value::Error(err) => format!("##{}", err.error_type),
                Value::Unit => "##_".to_string(),
            };
            (type_name, value.clone())
        })
    }

    /// Helper to get type name for a value (symbolic notation)
    fn value_type_name(&self, value: &Value) -> String {
        base_type_symbol(value).to_string()
    }

    /// Format a value for display using the current active numeral mode.
    ///
    /// Numeric types (`Int`, `Float`, `Bool`) are rendered in the active script,
    /// including the ones nested inside arrays, tuples and named tuples.
    pub fn format_value(&self, value: &Value) -> String {
        value.to_display_string_in(self.numeral_mode)
    }

    /// Repr form of `format_value`: numerals respect the active numeral mode,
    /// strings/chars are quoted, Unit shows as `()`.  Used by the REPL.
    pub fn format_value_repr(&self, value: &Value) -> String {
        value.to_repr_string_in(self.numeral_mode)
    }

    /// Bind a function declaration to its name.
    ///
    /// Called twice for a top-level declaration — once by the hoisting pass in
    /// `execute` and once when the statement itself runs — which is harmless: it
    /// builds the same `FunctionDef` and overwrites the same entry.
    pub(crate) fn register_function(&mut self, func_decl: &zymbol_ast::FunctionDecl) {
        // Auto-free (v0.0.8): schedule body locals and by-value params
        // for destruction after their last use. Output/Mutable params
        // participate in caller write-back — never freed early.
        let mut excluded: HashSet<String> = (*self.auto_free_excluded).clone();
        let mut param_candidates: Vec<String> = Vec::new();
        for p in &func_decl.parameters {
            match p.kind {
                zymbol_ast::ParameterKind::Normal => {
                    param_candidates.push(p.name.clone());
                }
                _ => {
                    excluded.insert(p.name.clone());
                }
            }
        }
        let auto_free = zymbol_semantic::region_schedule(
            &func_decl.body.statements,
            &param_candidates,
            &excluded,
        );
        let func_def = FunctionDef::Zymbol {
            parameters: func_decl.parameters.clone(),
            body: func_decl.body.clone(),
            origin_module_path: self.current_file.clone(),
            auto_free,
        };
        self.functions.insert(func_decl.name.clone(), Rc::new(func_def));
    }

    /// Execute a program
    pub fn execute(&mut self, program: &Program) -> Result<()> {
        // Process imports first
        for import in &program.imports {
            self.load_import(import)?;
        }

        // Auto-free (v0.0.8): compute the program-wide exclusion set and the
        // top-level destruction schedule. Function declarations executed below
        // compute their own body schedules against the same exclusions.
        // Invisible optimization — see zymbol_semantic::last_use.
        self.auto_free_excluded = Rc::new(zymbol_semantic::auto_free_exclusions(program));
        self.destruction_schedule =
            zymbol_semantic::region_schedule(&program.statements, &[], &self.auto_free_excluded);

        // Hoisting: a function declared anywhere at the top level is callable
        // from anywhere at the top level, including above its own declaration.
        //
        // This used to be decided by architecture rather than by anybody: the VM
        // compiles the file before running it and so registers every name first
        // (zymbol-compiler `compile`, "First pass: register function names"),
        // while this engine and the browser one bound each name as its statement
        // executed. `>> f(2) ¶` above `f(x) { <~ x * 10 }` printed 20 under
        // `--vm` and was `undefined function: 'f'` by default — the same program,
        // two answers (DM-03).
        //
        // The static analyzer had already picked a side: `zymbol check` passes
        // that program, because zymbol-semantic collects declarations before
        // checking calls. So the analyzer promised something only one of the
        // three engines delivered.
        //
        // Top level only, which is what the VM does. A function declared inside a
        // block still appears when the block runs; hoisting it out would move the
        // three engines apart again, in the other direction.
        //
        // Must run after `auto_free_excluded` is set: `register_function` reads
        // it to compute the body's destruction schedule.
        for statement in &program.statements {
            if let Statement::FunctionDecl(func_decl) = statement {
                self.register_function(func_decl);
            }
        }

        // Execute statements with auto-destruction after each one's last uses
        for (i, statement) in program.statements.iter().enumerate() {
            self.execute_statement(statement)?;

            // GAP-ZYB-006: a `<~` that reaches the top level ends the program,
            // and its value is the exit status. The other two engines already
            // stopped here; this one used to walk past and keep going, so the
            // same file printed different things under `--vm`.
            if let ControlFlow::Return(value) = &self.control_flow {
                self.exit_code = Some(match value {
                    Some(Value::Int(n)) => *n,
                    // No value: ended deliberately, with nothing to report.
                    None => 0,
                    // Rejected by the analyzer before running; if one arrives
                    // anyway, saying "something went wrong" beats inventing a
                    // number out of a value that is not one.
                    Some(_) => 1,
                });
                self.clear_control_flow();
                break;
            }

            // Pending control flow (shouldn't reach top level): teardown owns cleanup
            if self.is_control_flow_pending() {
                continue;
            }
            if let Some(vars_to_destroy) = self.destruction_schedule.get(&i) {
                let vars = vars_to_destroy.clone();
                for var_name in vars {
                    self.auto_destroy_variable(&var_name);
                }
            }
        }
        Ok(())
    }

    /// Execute a function body applying its auto-free schedule (v0.0.8).
    /// Destruction is skipped while control flow is pending — the frame or
    /// loop teardown owns cleanup on return/break paths.
    pub(crate) fn execute_body_scheduled(
        &mut self,
        block: &Block,
        schedule: &HashMap<usize, Vec<String>>,
    ) -> Result<()> {
        if schedule.is_empty() {
            return self.execute_block_no_scope(block);
        }
        for (i, statement) in block.statements.iter().enumerate() {
            self.execute_statement(statement)?;
            if self.is_control_flow_pending() {
                break;
            }
            if let Some(names) = schedule.get(&i) {
                for name in names {
                    self.auto_destroy_variable(name);
                }
            }
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
                self.register_function(func_decl);
                Ok(())
            }
            Statement::Return(return_stmt) => {
                let value = if let Some(expr) = &return_stmt.value {
                    // QW17: TCO — detect <~ f(args) where f == current executing function.
                    // When detected: evaluate args, store in tco_args, set tco_pending = true,
                    // and set Return so the call frame unwinds cleanly into the TCO loop.
                    if self.try_depth == 0 {
                        if let Expr::FunctionCall(call) = expr.unwrap_group() {
                            if let Expr::Identifier(callee) = call.callable.unwrap_group() {
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
                    // Names in move_guard_names are read again after the return —
                    // output-param writeback and module-state write-back — so they
                    // are cloned instead.
                    if self.try_depth == 0 {
                        if let Expr::Identifier(ident) = expr.unwrap_group() {
                            if !self.move_guard_names.contains(&ident.name) {
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
            Statement::LifetimeEnd(lifetime_end) => {
                self.destroy_variable(&lifetime_end.variable_name);
                Ok(())
            }
            Statement::DestructureAssign(d) => self.eval_destructure_assign(d),
            Statement::Try(try_stmt) => self.execute_try(try_stmt),
            Statement::SetNumeralMode { base, .. } => {
                self.numeral_mode = *base;
                Ok(())
            }
            Statement::Sleep(s) => self.execute_sleep(s),
            Statement::ClearScreen(cs) => self.execute_clear_screen(cs),
            Statement::KeyInput(ki) => self.execute_key_input(ki),
            Statement::OutputPos(op) => self.execute_output_pos(op),
            Statement::TuiBlock(tb) => self.execute_tui_block(tb),
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
        self.bind_destructure_pattern(&d.pattern, rhs, d.span)
    }

    /// Bind a destructuring pattern to a value that is already evaluated.
    ///
    /// Split out of `eval_destructure_assign` so a loop head can use the very
    /// same pattern: `@ (k, v):pares { … }` binds each element exactly as
    /// `(k, v) = par` would, which is the whole point — the pattern language is
    /// one language, and the loop stops needing a first line that only unpacks.
    pub(crate) fn bind_destructure_pattern(
        &mut self,
        pattern: &DestructurePattern,
        rhs: Value,
        span: zymbol_span::Span,
    ) -> Result<()> {
        match pattern {
            // The pattern is typed: `[ … ]` takes an array, `( … )` takes a tuple.
            // A mismatch is an error rather than a silent reinterpretation (REFERENCE.md L32).
            DestructurePattern::Array(items) => {
                let elements: Vec<Value> = match &rhs {
                    Value::Array(arr) => arr.clone(),
                    _ => return Err(RuntimeError::Generic {
                        message: format!(
                            "array pattern '[ … ]' requires an array, got {}",
                            self.value_type_name(&rhs)
                        ),
                        span,
                    }),
                };
                self.bind_positional(items, elements, false);
            }
            DestructurePattern::Positional(items) => {
                let elements: Vec<Value> = match &rhs {
                    Value::Tuple(tup) => tup.clone(),
                    _ => return Err(RuntimeError::Generic {
                        message: format!(
                            "tuple pattern '( … )' requires a tuple, got {}",
                            self.value_type_name(&rhs)
                        ),
                        span,
                    }),
                };
                self.bind_positional(items, elements, true);
            }
            DestructurePattern::NamedTuple(fields) => {
                let pairs: &Vec<(String, Value)> = match &rhs {
                    Value::NamedTuple(p) => p,
                    _ => return Err(RuntimeError::Generic {
                        message: format!(
                            "the pattern #(…) requires a dictionary, got {}\nhelp: #(key: name) = d unpacks a dictionary; use (a, b) for a tuple, [a, b] for an array",
                            crate::base_type_symbol(&rhs)
                        ),
                        span,
                    }),
                };
                for (field, var_name) in fields {
                    // A key the dictionary does not hold is `##Key`, never a silent
                    // Unit: binding nothing made `#(zzz: n) = d` succeed with `n`
                    // empty and exit 0, which is the very answer decision 10 exists
                    // to refuse. The register VM already raised here.
                    let Some(val) = pairs.iter().find(|(k, _)| k == field).map(|(_, v)| v.clone())
                    else {
                        let available: Vec<String> =
                            pairs.iter().map(|(k, _)| k.clone()).collect();
                        return Err(RuntimeError::Generic {
                            message: crate::variables::missing_key_msg(field, &available),
                            span,
                        });
                    };
                    self.set_variable(var_name, val);
                }
            }
        }
        Ok(())
    }

    /// Bind an array or positional-tuple pattern against the values it received.
    ///
    /// The **last item of the pattern absorbs whatever remains** (REFERENCE.md L33), so a
    /// length mismatch is never an error: it binds `Unit` when nothing is left, the bare
    /// value when exactly one is, and a collection when several are. `is_tuple` selects the
    /// shape that collection takes — the remainder keeps the shape of the container it came
    /// from.
    ///
    /// An explicit `*rest` opts out of absorption: it already governs how the values are
    /// shared out, and its binding is always a collection, even of one element or none.
    fn bind_positional(&mut self, items: &[DestructureItem], elements: Vec<Value>, is_tuple: bool) {
        let wrap = |vals: Vec<Value>| if is_tuple { Value::Tuple(vals) } else { Value::Array(vals) };
        let has_rest = items.iter().any(|i| matches!(i, DestructureItem::Rest(_)));
        let mut idx = 0usize;

        for (pos, item) in items.iter().enumerate() {
            let absorbs = !has_rest && pos + 1 == items.len();
            match item {
                DestructureItem::Bind(name) => {
                    let val = if absorbs {
                        match elements.len().saturating_sub(idx) {
                            0 => Value::Unit,
                            1 => elements[idx].clone(),
                            _ => wrap(elements[idx..].to_vec()),
                        }
                    } else {
                        elements.get(idx).cloned().unwrap_or(Value::Unit)
                    };
                    self.set_variable(name, val);
                    idx = if absorbs { elements.len() } else { idx + 1 };
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
                    self.set_variable(name, wrap(rest));
                    idx = end;
                }
                DestructureItem::Ignore => {
                    // In the last position `_` absorbs the remainder without binding it.
                    idx = if absorbs { elements.len() } else { idx + 1 };
                }
            }
        }
    }

    /// Evaluate an expression
    fn eval_expr(&mut self, expr: &Expr) -> Result<Value> {
        match expr {
            // Grouping parens are transparent
            Expr::Group(group) => self.eval_expr(&group.expr),
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
            Expr::StringRepeat(op) => self.eval_string_repeat(op),
            Expr::StringReplace(op) => self.eval_string_replace(op),
            Expr::StringSplit(op) => self.eval_string_split(op),
            Expr::ConcatBuild(op) => self.eval_concat_build(op),
            Expr::NumericCast(op) => self.eval_numeric_cast(op),
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
            Expr::DeepIndex(di) => self.eval_deep_index(di),
            Expr::FlatExtract(fe) => self.eval_flat_extract(fe),
            Expr::StructuredExtract(se) => self.eval_structured_extract(se),
            Expr::TerminalSize(t) => self.eval_terminal_size(t.span),
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
            // A pending `Return` is VALUE flow and never exception flow, even
            // when the value it carries is an error.
            //
            // This used to look inside the return, and treat an error found
            // there as something `:!` should catch. That made `$!!` — an early
            // return, by definition — behave as a throw whenever it happened to
            // sit inside a `!?`, so a function propagating a failure upwards
            // was intercepted by its own catch clause instead of returning.
            // The register VM and the browser engine both returned the value;
            // only the tree-walker caught it, and `GUIDE.md` § "Value flow"
            // states the rule the other two follow: "`$!!` … does not throw an
            // exception, so it cannot be caught with `!?`/`:!`".
            //
            // A `<~` of an ordinary value already left through here untouched,
            // so this is the same path, now taken by every return alike. The
            // finally clause below still runs: that is what a finally is.
            Ok(()) => None,
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
        //
        // "Always" includes the case where the try block returned. A pending
        // `ControlFlow::Return` has to be set aside first, or the finally runs
        // against a frame that is already unwinding: `execute_block` stops at
        // the first statement it sees while control flow is pending, so
        // `:> { >> "cleaning" ¶ }` printed `cleaning` and swallowed the newline
        // — half a statement, which is worse than none. The return is put back
        // afterwards unless the finally raised its own control flow, which
        // legitimately wins.
        if let Some(ref finally) = try_stmt.finally_clause {
            let pending = std::mem::replace(&mut self.control_flow, ControlFlow::None);
            let pending_flag = std::mem::replace(&mut self.has_control_flow, false);
            let finally_result = self.execute_block(&finally.block);
            // BUG-ZYB-011: a `:>` is cleanup, and cleanup does not decide what
            // the function returns. A `<~` written inside it is discarded, and
            // the return the try block was carrying continues — which is what
            // the browser engine has always done, and what makes this clause
            // safe to read: whatever it contains, the value coming back is the
            // one the reader saw at the `<~` above it.
            //
            // Java and Python do the opposite and let the finally win; both
            // warn against relying on it in their own style guides. Zymbol
            // takes the warning instead of the feature, and the analyzer says
            // so at the `<~` rather than letting it look like it did something.
            self.control_flow = pending;
            self.has_control_flow = pending_flag;
            finally_result?;
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
                // Checked before the rest: an integer that left its range is a
                // ##Range whatever else the message happens to mention.
                if lower_msg.contains("overflow") || lower_msg.contains("out of range") {
                    Value::Error(ErrorValue::range(message.clone()))
                // Before the index branch: a missing key is a ##Key even though
                // the reader reached it through the index syntax `d["k"]`.
                } else if lower_msg.contains("no key") {
                    Value::Error(ErrorValue::key(message.clone()))
                } else if lower_msg.contains("index") || lower_msg.contains("out of bounds") {
                    Value::Error(ErrorValue::index(message.clone()))
                } else if lower_msg.contains("type") {
                    Value::Error(ErrorValue::type_error(message.clone()))
                } else if lower_msg.contains("division") || lower_msg.contains("divide by zero")
                    || lower_msg.contains("modulo") {
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
            RuntimeError::CircularImport { module } => {
                Value::Error(ErrorValue::generic(format!(
                    "E004: Circular import detected: module '{}' is already being loaded",
                    module
                )))
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
                    >> "error at " i ¶
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
