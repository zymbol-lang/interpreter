//! Register VM executor for Zymbol-Lang
//!
//! Sprint 4: replaces tree-walker for hot paths.
//! Sprint 5C: flat register stack — all frames share one Vec<Value>.
//! Sprint 5D: Value uses Rc<T> for heap payloads → sizeof(Value) = 16 bytes.
//! Sprint 5G: ZyStr tagged-pointer SSO — strings ≤ 7 bytes stored inline.
//! Sprint 6A: fused split+HOF instructions (StrSplitMap/Filter/Reduce/Count).
//! Design goals:
//! - registers[idx] O(1) vs HashMap lookup
//! - Flat value_stack: no per-call heap alloc, better cache locality
//! - mem::replace for Return → O(1) move of String/Array
//! - Rc<T> payloads: 2.5x cheaper memset/memcpy on flat stack

mod zy_str;
pub use zy_str::ZyStr;
mod stdlib_builtins;

use zymbol_intrinsics as intrinsics;

use std::fmt;
use std::io::Write;
use std::mem;
use std::rc::Rc;

use thiserror::Error;
use zymbol_bytecode::{BuildPart, Chunk, CompiledProgram, FuncIdx, InputKind, Instruction, Reg};
use zymbol_common::num;

// ──────────────────────────────────────────────────────────────────────────────
// Numeral-mode helpers (mirrors zymbol-interpreter::numeral_mode)
// ──────────────────────────────────────────────────────────────────────────────

const ASCII_BASE: u32 = 0x0030;

/// Rewrites one **formatted number** into the script identified by `block_base`
/// (mirrors zymbol-interpreter::numeral_mode::map_numeral_number): digits, the
/// decimal separator and the thousands separator all follow the script.
///
/// The argument must be a single number and nothing else — the separators are
/// ordinary punctuation, so running this over composite text would rewrite
/// marks that were never separators. Composite text never reaches here: a list,
/// an interpolation and a concatenation each map their numbers one at a time.
///
/// Takes `s` by value so the ASCII fast-path hands the buffer straight back
/// instead of re-allocating it.
fn map_numeral_number(s: String, block_base: u32) -> String {
    if block_base == ASCII_BASE {
        return s;
    }
    let decimal = zymbol_lexer::digit_blocks::decimal_separator(block_base);
    let thousands = zymbol_lexer::digit_blocks::thousands_separator(block_base);
    s.chars()
        .map(|ch| match ch {
            '0'..='9' => char::from_u32(block_base + (ch as u32 - ASCII_BASE)).unwrap_or(ch),
            '.' => decimal,
            ',' => thousands,
            _ => ch,
        })
        .collect()
}

fn numeral_int(value: i64, base: u32) -> String { map_numeral_number(value.to_string(), base) }
fn numeral_float(value: f64, base: u32) -> String { map_numeral_number(value.to_string(), base) }
fn numeral_bool(value: bool, base: u32) -> String { format!("#{}", numeral_int(if value { 1 } else { 0 }, base)) }

/// Append `n` to `s` in the active script. In ASCII mode — the default, and the
/// path a `"label" i` concatenation inside a loop takes on every iteration — it
/// formats straight into the existing buffer, with no intermediate String.
#[inline]
fn push_numeral_int(s: &mut String, n: i64, base: u32) {
    use std::fmt::Write as _;
    if base == ASCII_BASE {
        let _ = write!(s, "{}", n);
    } else {
        s.push_str(&numeral_int(n, base));
    }
}

/// `push_numeral_int` for floats.
#[inline]
fn push_numeral_float(s: &mut String, f: f64, base: u32) {
    use std::fmt::Write as _;
    if base == ASCII_BASE {
        let _ = write!(s, "{}", f);
    } else {
        s.push_str(&numeral_float(f, base));
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Runtime Value
// ──────────────────────────────────────────────────────────────────────────────

/// Runtime value — lives in registers.
/// Sprint 5D: heap payloads wrapped in Rc<T> so sizeof(Value) == 16 bytes.
/// Layout: 8-byte discriminant (padded from 1-byte tag with 11 variants) + 8-byte payload.
#[derive(Debug, Clone)]
pub enum Value {
    Int(i64),
    Float(f64),
    Bool(bool),
    Char(char),
    Unit,
    /// Named function reference (type ##(), display <funct/N>)
    Function(FuncIdx, u8),
    /// Sprint 5G: ZyStr (8 bytes) — inline for ≤ 7 bytes, heap Rc<String> otherwise.
    String(ZyStr),
    Array(Rc<Vec<Value>>),
    /// Positional tuple: (v1, v2, v3)
    Tuple(Rc<Vec<Value>>),
    /// Named tuple: Vec of (field_name, value) pairs
    NamedTuple(Rc<Vec<(String, Value)>>),
    /// Lambda (with or without captures): function + arity + upvalues (type ##->, display <lambd/N>)
    Closure(FuncIdx, u8, Rc<Vec<Value>>),
    /// Error value (error-as-value flow: <~ _err inside :! block)
    /// Inner string is the formatted error: "##Kind(message)"
    Error(ZyStr),
}

/// Materialize a module-level initializer into a runtime value.
///
/// Split out of `Vm::run` because the collection variants are recursive: a
/// dictionary of dictionaries is one initializer and has to be built depth
/// first. It runs once per global at startup, never inside the dispatch loop.
fn global_init_value(init: &zymbol_bytecode::GlobalInit) -> Value {
    use zymbol_bytecode::GlobalInit as G;
    match init {
        G::Int(n) => Value::Int(*n),
        G::Float(f) => Value::Float(*f),
        G::Bool(b) => Value::Bool(*b),
        G::Char(c) => Value::Char(*c),
        G::Str(s) => Value::String(ZyStr::new(s.clone())),
        G::Unit => Value::Unit,
        G::Array(items) => Value::Array(Rc::new(items.iter().map(global_init_value).collect())),
        G::Tuple(items) => Value::Tuple(Rc::new(items.iter().map(global_init_value).collect())),
        G::Dict(fields) => Value::NamedTuple(Rc::new(
            fields.iter().map(|(k, v)| (k.clone(), global_init_value(v))).collect(),
        )),
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Int(n) => write!(f, "{}", n),
            Value::Float(n) => write!(f, "{}", n),
            Value::String(s) => write!(f, "{}", s.as_ref()),
            Value::Char(c) => write!(f, "{}", c),
            Value::Bool(b) => write!(f, "{}", if *b { "#1" } else { "#0" }),
            Value::Array(arr) => {
                write!(f, "[")?;
                for (i, v) in arr.as_ref().iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", v)?;
                }
                write!(f, "]")
            }
            Value::Tuple(items) => {
                write!(f, "(")?;
                for (i, v) in items.as_ref().iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{}", v)?;
                }
                write!(f, ")")
            }
            Value::NamedTuple(fields) => {
                // `#(…)` — see the note on the tree-walker's `to_display_string_in`.
                write!(f, "#(")?;
                for (i, (name, val)) in fields.as_ref().iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{}: {}", name, val)?;
                }
                write!(f, ")")
            }
            Value::Function(_, arity) => write!(f, "<funct/{}>", arity),
            Value::Closure(_, arity, _) => write!(f, "<lambd/{}>", arity),
            Value::Unit => write!(f, "()"),
            Value::Error(s) => write!(f, "{}", s.as_ref()),
        }
    }
}

impl Value {
    /// Readable type name for diagnostics. Mirrors `zymbol-interpreter`'s
    /// `type_word` and (until it was retired) zyml's `type_name`, so a message naming a type reads the
    /// same whichever engine produced it.
    fn type_word(&self) -> &'static str {
        match self {
            Value::Int(_)        => "integer",
            Value::Float(_)      => "float",
            Value::Bool(_)       => "bool",
            Value::String(_)     => "string",
            Value::Char(_)       => "char",
            Value::Array(_)      => "array",
            Value::Tuple(_) | Value::NamedTuple(_) => "tuple",
            Value::Function(..)  => "function",
            Value::Closure(..)   => "lambda",
            Value::Error(_)      => "error",
            Value::Unit          => "unit",
        }
    }

    #[inline(always)]
    fn is_truthy(&self) -> bool {
        match self {
            Value::Bool(b) => *b,
            Value::Int(n) => *n != 0,
            Value::Unit => false,
            _ => true,
        }
    }

    fn to_string_repr(&self) -> String {
        match self {
            Value::String(s) => s.to_string(),
            // A standalone Unit is nothing, and only INSIDE a collection is it
            // `()` — `[1, , 3]` reads like a typo. `Display` renders the nested
            // form, so this arm is what tells the two apart.
            //
            // Without it, `"" u` built `"()"` in this engine and `""` in the
            // other two: a program composing a message with a NULL column
            // printed something different depending on which one ran it.
            Value::Unit => String::new(),
            other => other.to_string(),
        }
    }

    /// `to_string_repr` with every digit rendered in the numeral system
    /// identified by `block_base` — collection elements included, since a
    /// number does not stop being a number by sitting in a list.
    ///
    /// Mirrors `Value::to_display_string_in` in the tree-walker; the two must
    /// agree character for character.
    fn to_display_in(&self, block_base: u32) -> String {
        // Standalone Unit is nothing; nested Unit is `()`. Mirrors
        // `to_display_string_in` in the tree-walker, which spells the rule the
        // same way and for the same reason.
        fn nested(v: &Value, block_base: u32) -> String {
            match v {
                Value::Unit => "()".to_string(),
                other => other.to_display_in(block_base),
            }
        }
        match self {
            Value::Unit     => String::new(),
            Value::Int(n)   => numeral_int(*n, block_base),
            Value::Float(f) => numeral_float(*f, block_base),
            Value::Bool(b)  => numeral_bool(*b, block_base),
            Value::String(s) => s.to_string(),
            Value::Array(arr) => {
                let contents: Vec<String> =
                    arr.iter().map(|v| nested(v, block_base)).collect();
                format!("[{}]", contents.join(", "))
            }
            Value::Tuple(items) => {
                let contents: Vec<String> =
                    items.iter().map(|v| nested(v, block_base)).collect();
                format!("({})", contents.join(", "))
            }
            Value::NamedTuple(fields) => {
                // `#(…)` — see the note on the tree-walker's `to_display_string_in`.
                let contents: Vec<String> = fields
                    .iter()
                    .map(|(name, v)| format!("{}: {}", name, nested(v, block_base)))
                    .collect();
                format!("#({})", contents.join(", "))
            }
            other => other.to_string(),
        }
    }

    fn type_name(&self) -> &'static str {
        match self {
            Value::Int(_)           => "Int",
            Value::Float(_)         => "Float",
            Value::String(_)        => "String",
            Value::Char(_)          => "Char",
            Value::Bool(_)          => "Bool",
            Value::Array(_)         => "Array",
            Value::Tuple(_)         => "Tuple",
            Value::NamedTuple(_)    => "Dict",
            Value::Function(_, _)   => "Function",
            Value::Closure(_, _, _) => "Function",
            Value::Unit             => "Unit",
            Value::Error(_)         => "Error",
        }
    }

    /// The type as a diagnostic names it, the way the analyzer does: `Int`,
    /// `[Int]`, `(Int, String)`, `#(k: Int)`, `Function`
    /// (`zymbol_common::typeword`, decided 2026-09-16). The tree-walker's
    /// `Value::type_label`, over this engine's values: a message says what kind
    /// of value arrived, and both engines name it with one table.
    pub fn type_label(&self) -> String {
        use zymbol_common::typeword as tw;
        match self {
            Value::Int(_) => tw::INT.to_string(),
            Value::Float(_) => tw::FLOAT.to_string(),
            Value::String(_) => tw::STRING.to_string(),
            Value::Char(_) => tw::CHAR.to_string(),
            Value::Bool(_) => tw::BOOL.to_string(),
            Value::Unit => tw::UNIT.to_string(),
            Value::Error(_) => tw::ERROR.to_string(),
            Value::Function(_, _) | Value::Closure(_, _, _) => tw::FUNCTION.to_string(),
            Value::Array(items) => tw::array(items.iter().map(Value::type_label)),
            Value::Tuple(items) => tw::tuple(items.iter().map(Value::type_label)),
            Value::NamedTuple(fields) => {
                tw::dict(fields.iter().map(|(k, v)| (k.as_str(), v.type_label())))
            }
        }
    }

    /// Returns the Zymbol symbolic type name used in stdlib error messages.
    pub fn zymbol_type_name(&self) -> &'static str {
        match self {
            Value::Int(_)           => "###",
            Value::Float(_)         => "##.",
            Value::String(_)        => "##\"",
            Value::Char(_)          => "##'",
            Value::Bool(_)          => "##?",
            Value::Array(_)         => "##[]",
            Value::Tuple(_)         => "##()",
            Value::NamedTuple(_)    => "##(name:)",
            Value::Function(_, _)   => "##fn",
            Value::Closure(_, _, _) => "##fn",
            Value::Unit             => "##_",
            Value::Error(_)         => "##!",
        }
    }

    /// The type name spelled as the tree-walker's `value_type_name` spells it.
    ///
    /// Destructuring errors are compared verbatim across engines by `zyq consensus`, so the
    /// two must agree to the character — which is why the spellings come from
    /// `zymbol_common::typesym` rather than from a table written out here. This is the
    /// BASE symbol: an array is `##]` whatever it holds, because a failed destructuring is
    /// about the shape and not about the mix. `#?` refines it; see `refined_type_symbol`.
    ///
    /// `zymbol_type_name` above uses a different spelling for the same types (`##[]`
    /// against `##]`, `##()` against `##)`) and cannot be reused here.
    fn tw_type_name(&self) -> &'static str {
        use zymbol_common::typesym as ts;
        match self {
            Value::Int(_)           => ts::INT,
            Value::Float(_)         => ts::FLOAT,
            Value::String(_)        => ts::STRING,
            Value::Char(_)          => ts::CHAR,
            Value::Bool(_)          => ts::BOOL,
            Value::Array(_)         => ts::ARRAY,
            Value::Tuple(_)         => ts::TUPLE,
            Value::NamedTuple(_)    => ts::DICT,
            Value::Function(_, _)   => ts::FUNCTION,
            Value::Closure(_, _, _) => ts::LAMBDA,
            Value::Unit             => ts::UNIT,
            Value::Error(_)         => "##!",
        }
    }

    /// `tw_type_name`, except that an ERROR names its own kind — `##Index`, not the
    /// generic `##!` — exactly as the tree-walker's `base_type_symbol` does.
    ///
    /// The kind is the prefix before `(` of the error's own text, which is where
    /// `Instruction::TypeOf` already read it from to answer `#?`. Written once because
    /// it had been written once and MISSED once: `#?` said `##Index` while a diagnostic
    /// naming the same value said `##!`.
    fn tw_type_name_owned(&self) -> String {
        match self {
            Value::Error(s) => {
                let t = s.as_ref();
                t.find('(').map(|i| &t[..i]).unwrap_or(t).to_string()
            }
            other => other.tw_type_name().to_string(),
        }
    }

    /// The message inside an error's `##Kind(…)` text, which is what `#?` counts.
    ///
    /// The tree-walker keeps kind and message in separate fields and answers
    /// `err.message.len()` (`data_ops.rs`); this engine keeps one string, so the message
    /// is what sits between the first `(` and the final `)`. Answering 0 made the same
    /// value report a length of 57 under one engine and 0 under the other.
    fn error_message_len(&self) -> i64 {
        match self {
            Value::Error(s) => {
                let t = s.as_ref();
                match (t.find('('), t.strip_suffix(')')) {
                    (Some(i), Some(no_paren)) => no_paren[i + 1..].len() as i64,
                    _ => t.len() as i64,
                }
            }
            _ => 0,
        }
    }

    /// What `#?` answers: `tw_type_name`, except that an array whose elements are not all
    /// one type is a list, `##[`.
    ///
    /// The mix is read from the value NOW, not from how the literal was written: `#[…]`
    /// declares a mix to the analyzer and leaves no trace on the value, so a heterogeneous
    /// array out of `json::decode` answers `##[` with no mark anywhere, and
    /// `#[1, "dos"]$-[2]` answers `##]` because a single Int is not a mix.
    fn refined_type_symbol(&self) -> &'static str {
        match self {
            Value::Array(items) => {
                let bases: Vec<&'static str> = items.iter().map(Value::tw_type_name).collect();
                zymbol_common::typesym::array_symbol(bases.into_iter())
            }
            other => other.tw_type_name(),
        }
    }

    /// The `(symbol, count)` pair `#?` builds its tuple from. Written once because it had
    /// been written twice — the two dispatch paths below each carried their own copy, and
    /// a table kept in two places is a table that eventually disagrees with itself.
    fn type_metadata(&self) -> (&'static str, i64) {
        let count = match self {
            Value::Int(n) => n.to_string().len() as i64,
            Value::Float(fl) => fl.to_string().len() as i64,
            Value::String(s) => s.as_ref().chars().count() as i64,
            Value::Char(_) | Value::Bool(_) => 1,
            Value::Array(a) => a.as_ref().len() as i64,
            Value::Tuple(t) => t.as_ref().len() as i64,
            Value::NamedTuple(f) => f.as_ref().len() as i64,
            Value::Function(_, arity) => *arity as i64,
            Value::Closure(_, arity, _) => *arity as i64,
            _ => 0,
        };
        let symbol = match self {
            Value::Unit | Value::Error(_) => zymbol_common::typesym::UNIT,
            other => other.refined_type_symbol(),
        };
        (symbol, count)
    }

    /// Equality for pattern matching and $? operator
    fn equals(&self, other: &Value) -> bool {
        match (self, other) {
            (Value::Int(a), Value::Int(b)) => a == b,
            (Value::Float(a), Value::Float(b)) => a == b,
            // Int/Float promotion, to match the ordering comparisons and the
            // tree-walker. See zymbol-interpreter::values_equal_static.
            (Value::Int(a), Value::Float(b)) => (*a as f64) == *b,
            (Value::Float(a), Value::Int(b)) => *a == (*b as f64),
            (Value::String(a), Value::String(b)) => a.as_ref() == b.as_ref(),
            (Value::Char(a), Value::Char(b)) => a == b,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Unit, Value::Unit) => true,
            // Two functions are equal when they are THE SAME function
            // (BUG-ZYB-012). A named one is its index in the function table,
            // which is stable however many names point at it; a closure is its
            // index AND the upvalues it captured, because the same lambda
            // evaluated twice — next time round a loop — is two closures.
            //
            // Same shape as the missing `Array` arm above: no arm meant
            // `_ => false`, so a function never equalled itself, while the
            // browser engine said `#1` to any two functions at all. Neither had
            // been decided; identity is what was.
            (Value::Function(ia, aa), Value::Function(ib, ab)) => ia == ib && aa == ab,
            (Value::Closure(ia, aa, ua), Value::Closure(ib, ab, ub)) => {
                ia == ib && aa == ab && Rc::ptr_eq(ua, ub)
            }
            (Value::Tuple(a), Value::Tuple(b)) => {
                a.len() == b.len() && a.iter().zip(b.iter()).all(|(x, y)| x.equals(y))
            }
            // Arrays compare element by element, exactly as tuples do. The
            // missing arm fell through to `_ => false`, so `[1,2,3] == [1,2,3]`
            // answered #0 in the VM and #1 in the other two engines (DM-02): a
            // silent wrong answer, and a `?` on it took the opposite branch.
            //
            // Recursing through `equals` is what gives nested arrays and the
            // Int/Float promotion for free — element equality has to be the same
            // relation as scalar equality, or `[1] == [1.0]` disagrees with
            // `1 == 1.0`, which is #1 and documented.
            (Value::Array(a), Value::Array(b)) => {
                a.len() == b.len() && a.iter().zip(b.iter()).all(|(x, y)| x.equals(y))
            }
            // Two dictionaries are equal when they hold the same keys with the
            // same values — decided 2026-08-19 (DM-22). Both Rust engines said
            // `#0`, which was indefensible: every other collection compares by
            // value, and a dictionary that never equals another cannot be
            // tested, deduplicated or asserted on.
            //
            // Key ORDER is not part of it. Insertion order is preserved for
            // walking, as in Python's dict, but two dictionaries built in a
            // different order still hold the same thing.
            (Value::NamedTuple(a), Value::NamedTuple(b)) => {
                a.len() == b.len()
                    && a.iter().all(|(ka, va)| {
                        b.iter().any(|(kb, vb)| ka == kb && va.equals(vb))
                    })
            }
            _ => false,
        }
    }

}

// ──────────────────────────────────────────────────────────────────────────────
// FrameInfo — Sprint 5C: metadata only, no registers
// Sprint 5E: slim FrameInfo — heap-allocate rare fields to reduce sizeof
// ──────────────────────────────────────────────────────────────────────────────

/// An error that reached a handler and has not been dealt with yet (GLB-010).
///
/// It stays pending through the clauses of its `!?` until one handles it, and
/// through the `:>` after them; whatever is still pending when the `!?` ends is
/// raised again. The original error is kept, not its printed form, so it leaves
/// with its own kind and — through `origin_*` — from the line that raised it.
struct Pending {
    /// The register of the `!?` that caught it (`TryLand`), which is also how
    /// `TryHandled` and `TryRethrow` recognise it as theirs.
    tag: Reg,
    err: VmError,
    kind: &'static str,
    origin_ip: u32,
    origin_chunk: u32,
}

/// Error state of a frame — allocated the first time an error lands in it.
#[derive(Default)]
struct FrameError {
    pending: Vec<Pending>,
}

/// Frame metadata — registers live in value_stack[base..next_base]
/// sizeof target: ~40 bytes (down from 88)
struct FrameInfo {
    /// Offset in value_stack where this frame's registers start
    base: u32,
    /// Saved IP of the caller (valid when this frame is not the top)
    ip: u32,
    /// chunk_idx: u32::MAX = main chunk, else index into program.functions
    chunk_idx: u32,
    /// Register in the caller frame where the return value should be written
    return_reg: u16,
    /// The armed handler: where an error raised in this frame goes
    /// (u32::MAX = none). The one it replaced lives in a register of the `!?`
    /// that armed it (`TryBegin`), so nesting costs no allocation.
    catch_ip: u32,
    /// Pending errors — allocated only when an error lands in this frame
    error: Option<Box<FrameError>>,
    /// Output param writeback — None for most functions (saves 24 bytes)
    // Box is intentional: keeps `Option` at 8 bytes (vs 24 for a bare Vec) in the
    // common None case. The extra allocation only happens on the rare write path.
    #[allow(clippy::box_collection)]
    writeback: Option<Box<Vec<(usize, Reg)>>>,
}

// ──────────────────────────────────────────────────────────────────────────────
// VM Error
// ──────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum VmError {
    // Phrased as the LANGUAGE, not as the check that noticed. "type error:
    // expected Int, got String" names an internal predicate; a reader is told
    // what the program did wrong.
    #[error("this needs {expected} and got {got}")]
    TypeError { expected: &'static str, got: String },
    #[error("{op} requires a numeric value, got {got}")]
    CastError { op: &'static str, got: String },
    #[error("division by zero")]
    DivisionByZero,
    #[error("modulo by zero")]
    ModuloByZero,
    /// An integer result outside `zymbol_common::num`'s range. Spelled exactly
    /// as the tree-walker spells it — `zyq consensus` compares the text.
    #[error("integer overflow: {a} {op} {b}")]
    IntOverflow { a: i64, op: &'static str, b: i64 },
    /// `###`/`##!` on a float with no integer form in range.
    #[error("integer overflow: {op} cannot represent this float")]
    CastOverflow { op: &'static str },
    /// `container` is spelled as the tree-walker spells it — the read path there
    /// names the thing that was too short, and one message that always said
    /// "array" told a program indexing past the end of a STRING that its array
    /// was short. `zyq consensus` compares the text.
    #[error("{container} index out of bounds: index {index} for {container} of length {length}")]
    IndexOutOfBounds { index: i64, length: usize, container: &'static str },
    #[error("index 0 is invalid — Zymbol uses 1-based indexing (use 1 for the first element, -1 for the last)")]
    IndexZero,
    #[error("undefined function index {0}")]
    UndefinedFunction(FuncIdx),
    #[error("register {0} out of range")]
    RegisterOutOfRange(Reg),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Generic(String),
    /// A type mistake worded as the tree-walker words that operation —
    /// `map requires array, got Int` — and still of the `##Type` family, which
    /// `TypeError`'s generic `this needs X and got Y` was (step 3.5b, decided
    /// 2026-09-16). The type is named with `Value::type_label`.
    #[error("{0}")]
    TypeMsg(String),

    /// A message whose family is `##Index`: the type is right and the VALUE is
    /// not — a count that came out negative, an index below 1. D1 puts those in
    /// `##Index`, and `TypeMsg` was the only message-carrying variant there
    /// was, so they were answering `##Type`.
    #[error("{0}")]
    IndexMsg(String),

    /// A runtime error that knows where it happened. Built once, at the edge of
    /// `run`, from the instruction pointer the VM was on — so the hot loop pays
    /// two stores per instruction and nothing more. `Display` is the message
    /// alone, so `!?` catches the same text it always did.
    #[error("{message}")]
    Located { message: String, file: String, line: u32, column: u32 },
}

impl VmError {
    /// Where the error happened, if it knows. Line only — see
    /// `RuntimeError::location` for why there is no column.
    pub fn location(&self) -> Option<(&str, u32)> {
        match self {
            VmError::Located { file, line, .. } => Some((file, *line)),
            _ => None,
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Free helpers (not methods — avoids borrow conflicts in the dispatch loop)
// ──────────────────────────────────────────────────────────────────────────────

/// Run `cmd` through the system shell, for `<\ \>` and `</ />`.
///
/// Shared by both instructions and resolved by the same code the tree-walker uses
/// (`zymbol_common::shell`), so the two engines cannot disagree about which shell
/// a script runs in. A spawn failure names the program, which the previous
/// `failed to execute bash command: program not found` did not — it named a shell
/// the code was not even running.
/// Whether a value is, or holds anywhere inside it, a function or a lambda.
/// A function has no text a shell could take; the tree-walker refuses it at any
/// depth of the collection it is joined from (GLB-017 A).
fn holds_function(v: &Value) -> bool {
    match v {
        Value::Function(..) | Value::Closure(..) => true,
        Value::Array(items) | Value::Tuple(items) => items.iter().any(holds_function),
        Value::NamedTuple(fields) => fields.iter().any(|(_, v)| holds_function(v)),
        _ => false,
    }
}

/// The text a value becomes inside `<\ … \>`: a collection is its elements
/// joined with spaces, at any depth — shell words, the tree-walker's
/// `value_to_bash_str`. Its display form (`[1, 2]`, `(3, x)`, `#(…)`) is shell
/// syntax or a comment to `sh` (GLB-017 G).
fn shell_text(v: &Value) -> String {
    match v {
        Value::Array(items) | Value::Tuple(items) => {
            items.iter().map(shell_text).collect::<Vec<_>>().join(" ")
        }
        Value::NamedTuple(fields) => {
            fields.iter().map(|(_, v)| shell_text(v)).collect::<Vec<_>>().join(" ")
        }
        other => other.to_string_repr(),
    }
}

fn run_in_shell(cmd: &str) -> Result<std::process::Output, VmError> {
    let mut shell =
        zymbol_common::shell::shell_command(cmd).map_err(|e| VmError::Generic(e.to_string()))?;
    shell.output().map_err(|e| {
        VmError::Generic(format!(
            "failed to run `{}`: {}",
            shell.get_program().to_string_lossy(),
            e
        ))
    })
}

fn fmt_comma_int(n: i64) -> String {
    let neg = n < 0;
    let digits = format!("{}", n.unsigned_abs());
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, c) in digits.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 { out.push(','); }
        out.push(c);
    }
    let mut s: String = out.chars().rev().collect();
    if neg { s.insert(0, '-'); }
    s
}

/// Format with thousands separators: prec_kind 0=none, 1=round, 2=truncate
/// The decimal count held in a register, for the `*Dyn` format opcodes.
fn vm_precision_from(v: &Value) -> Result<u32, VmError> {
    match v {
        Value::Int(n) if *n >= 0 => Ok(*n as u32),
        // Worded as the tree-walker words them (step 3.5b).
        Value::Int(n) => Err(VmError::IndexMsg(format!("decimal count must not be negative, got {}", n))),
        other => Err(VmError::TypeMsg(format!(
            "decimal count must be a whole number, got {}", other.type_label()))),
    }
}

/// The number a format opcode operates on. Mirrors the immediate path, which
/// also accepts a string that parses — the tree-walker rejects a non-number and
/// returning 0.0 silently made the two engines disagree.
fn vm_number_from(v: &Value) -> Result<f64, VmError> {
    match v {
        Value::Int(n) => Ok(*n as f64),
        Value::Float(f) => Ok(*f),
        other => ascii_digits(other.to_string().trim())
            .parse::<f64>()
            .map_err(|_| VmError::TypeError {
                expected: "number",
                got: other.type_name().to_string(),
            }),
    }
}

/// A Char read as the one-character string it is, for `#|c|` (GAP-ZYB-012).
///
/// Mirrors the String arm above, including the 69 digit scripts, and returns
/// the character unchanged when it is not a number — which is what "safe
/// conversion" means for a string too.
fn vm_char_as_number(c: char) -> Value {
    let s = c.to_string();
    match num::parse(&s) {
        num::Num::Int(i) => Value::Int(i),
        num::Num::Float(f) => Value::Float(f),
        num::Num::None => match normalize_unicode_digits(&s).map(|n| num::parse(&n)) {
            Some(num::Num::Int(i)) => Value::Int(i),
            Some(num::Num::Float(f)) => Value::Float(f),
            _ => Value::Char(c),
        },
    }
}

/// Map the ASCII digits of a formatted number into the active numeral script.
fn vm_fmt_thousands(num: f64, prec_kind: u8, prec_n: u32) -> String {
    let num = match prec_kind {
        1 => { let m = 10f64.powi(prec_n as i32); (num * m).round() / m }
        2 => { let m = 10f64.powi(prec_n as i32); (num * m).trunc() / m }
        _ => num,
    };
    let neg = num < 0.0;
    let abs_f = num.abs();
    let int_part = abs_f.floor() as i64;
    let mut s = fmt_comma_int(int_part);
    match prec_kind {
        0 => {
            let full_s = format!("{}", abs_f);
            if let Some(dot_pos) = full_s.find('.') {
                s.push_str(&full_s[dot_pos..]);
            }
        }
        _ => {
            if prec_n > 0 {
                let frac = abs_f - int_part as f64;
                let frac_s = format!("{:.prec$}", frac, prec = prec_n as usize);
                if let Some(dot_pos) = frac_s.find('.') {
                    s.push_str(&frac_s[dot_pos..]);
                }
            }
        }
    }
    if neg { s.insert(0, '-'); }
    s
}

/// Format in scientific notation: prec_kind 0=none, 1=round, 2=truncate
fn vm_fmt_scientific(num: f64, prec_kind: u8, prec_n: u32) -> String {
    match prec_kind {
        0 => format!("{:e}", num),
        1 => format!("{:.prec$e}", num, prec = prec_n as usize),
        _ => vm_fmt_scientific_truncate(num, prec_n),
    }
}

fn vm_fmt_scientific_truncate(num: f64, n: u32) -> String {
    if num == 0.0 {
        if n == 0 { return "0e0".to_string(); }
        return format!("{:.prec$e}", 0.0f64, prec = n as usize);
    }
    let exp = num.abs().log10().floor() as i32;
    let mantissa = num / 10f64.powi(exp);
    let m = 10f64.powi(n as i32);
    let truncated = (mantissa * m).trunc() / m;
    if n == 0 {
        format!("{}e{}", truncated as i64, exp)
    } else {
        format!("{:.prec$}e{}", truncated, exp, prec = n as usize)
    }
}

#[inline(always)]
/// Numeric equality against an integer immediate, promoting Float like every
/// other comparison does. `?? 3.0 { 3 => … }` compiles to CmpEqImm, so without
/// this a Float subject raised a type error in the fast path and silently left
/// the destination register unwritten in the slow one.
fn num_eq_imm(v: &Value, imm: i64) -> Option<bool> {
    match v {
        Value::Int(n) => Some(*n == imm),
        Value::Float(f) => Some(*f == imm as f64),
        _ => None,
    }
}

/// Ordering comparison (`<`, `<=`, `>`, `>=`) — the single rule both engines use.
///
/// Numeric when *both* sides are numbers, where a string counts as a number if
/// `#|…|` would convert it: digits from any of the 69 supported scripts, so
/// `"४२" > "९"` compares 42 against 9 exactly as `"42" > "9"` does. Two
/// non-numeric strings compare lexicographically. A number against a string
/// that is not a number is `None` — the caller raises, matching the
/// tree-walker's `compare_values`.
///
/// Equality (`==`, `!=`) deliberately does NOT go through here: `"5" == 5` is
/// false in both engines and stays false.
fn cmp_order(va: &Value, vb: &Value) -> Option<i32> {
    use std::cmp::Ordering;
    fn ord(o: Ordering) -> i32 { match o { Ordering::Less => -1, Ordering::Equal => 0, Ordering::Greater => 1 } }
    fn as_int(s: &str) -> Option<i64> {
        match num::parse(&ascii_digits(s.trim())) { num::Num::Int(n) => Some(n), _ => None }
    }
    fn as_f64(s: &str) -> Option<f64> { ascii_digits(s.trim()).parse::<f64>().ok() }
    // NaN has no ordering against anything, itself included. Folding that into
    // `Equal` made `nan <= 1.0` and `nan >= 1.0` both true.
    fn f_ord(x: f64, y: f64) -> i32 { x.partial_cmp(&y).map_or(INCOMPARABLE, ord) }

    match (va, vb) {
        (Value::Int(x), Value::Int(y))     => Some(ord(x.cmp(y))),
        (Value::Float(x), Value::Float(y)) => Some(f_ord(*x, *y)),
        (Value::Int(x), Value::Float(y))   => Some(f_ord(*x as f64, *y)),
        (Value::Float(x), Value::Int(y))   => Some(f_ord(*x, *y as f64)),
        (Value::Char(x), Value::Char(y))   => Some(ord(x.cmp(y))),
        (Value::Bool(x), Value::Bool(y))   => Some(ord(x.cmp(y))),
        (Value::String(x), Value::String(y)) => {
            match (as_int(x.as_str()), as_int(y.as_str())) {
                (Some(a), Some(b)) => Some(ord(a.cmp(&b))),
                _ => match (as_f64(x.as_str()), as_f64(y.as_str())) {
                    (Some(a), Some(b)) => Some(f_ord(a, b)),
                    _ => Some(ord(x.as_str().cmp(y.as_str()))),
                },
            }
        }
        (Value::String(s), Value::Int(i)) => match as_int(s.as_str()) {
            Some(n) => Some(ord(n.cmp(i))),
            None => as_f64(s.as_str()).map(|f| f_ord(f, *i as f64)),
        },
        (Value::Int(i), Value::String(s)) => match as_int(s.as_str()) {
            Some(n) => Some(ord(i.cmp(&n))),
            None => as_f64(s.as_str()).map(|f| f_ord(*i as f64, f)),
        },
        (Value::String(s), Value::Float(f)) => as_f64(s.as_str()).map(|n| f_ord(n, *f)),
        (Value::Float(f), Value::String(s)) => as_f64(s.as_str()).map(|n| f_ord(*f, n)),
        // ZYVM-001: a tuple used to compare element by element here, so
        // `(1, 2) < (3, 4)` answered `#1` under the VM and was refused by the
        // tree-walker. The tuple is positional and heterogeneous — `(1, "a") <
        // (2, #0)` has no defensible answer — so the language has no ordering
        // for it, and the two engines now say so with one message. Equality is
        // untouched: `==` does not go through here, and two tuples still
        // compare equal element by element.
        _ => None,
    }
}

/// The tree-walker's message for an ordering comparison it refuses to make, so
/// both engines fail with the same text.
fn cmp_order_error(va: &Value, vb: &Value, op: &str) -> String {
    match (va, vb) {
        (Value::String(s), Value::Int(i)) =>
            format!("cannot compare string '{}' with integer {} using operator '{}'", s.as_ref(), i, op),
        (Value::Int(i), Value::String(s)) =>
            format!("cannot compare integer {} with string '{}' using operator '{}'", i, s.as_ref(), op),
        (Value::String(s), Value::Float(f)) =>
            format!("cannot compare string '{}' with float {} using operator '{}'", s.as_ref(), f, op),
        (Value::Float(f), Value::String(s)) =>
            format!("cannot compare float {} with string '{}' using operator '{}'", f, s.as_ref(), op),
        (a, b) =>
            format!("cannot compare values with operator '{}': {} and {}", op, a.type_name(), b.type_name()),
    }
}

/// The tree-walker's refusal for an arithmetic operation whose operands are not
/// numbers — **spelled per operator** (ZYVM-002).
///
/// It used to be one string, `+ is arithmetic only …`, reached from every
/// arithmetic instruction through one shared macro. So `7 - "a"` was refused
/// with guidance about an operator the program had not written, and `'a' + 'b'`
/// — where that guidance is exactly right — got the generic "this needs a
/// number" instead, because it took a different path. The message was tied to
/// the code route, not to the operator; now it is tied to the operator.
fn arith_type_error(op: &str, a: &Value, b: &Value) -> String {
    match op {
        "+" => "+ is arithmetic only — use juxtaposition to concatenate strings: \"a\" b \"c\"".to_string(),
        "/" => "/ requires numeric operands — use $/ to split strings".to_string(),
        "^" => format!("power operator requires numeric operands: {}, {}", a.type_name(), b.type_name()),
        _   => format!("arithmetic requires numeric operands: {}, {}", a.type_name(), b.type_name()),
    }
}

/// The tree-walker's refusal for `&&` / `||` on an operand that is not a Bool.
///
/// ZYVM-001: the VM read both operands through `is_truthy()`, so `7 && 3`
/// answered `#1` where the tree-walker refused — and the semantic analyser,
/// which both engines share, had already emitted the warning in both. The
/// language settled this for the loop specifier in v0.0.9 — there is no
/// truthiness; a thing is what it is or it is refused — and the logical
/// operators follow the same rule.
fn logical_type_error(op: &str, v: &Value) -> String {
    format!("logical {} requires boolean operands, got {}", op, v.type_name())
}

/// Returned by `cmp_direct` and `cmp_order` for values with no ordering at all
/// — two different types, or anything involving NaN.
///
/// It is deliberately not a value the sign tests would accept: an ordering
/// comparison against it must be *false in all four directions*, which is what
/// IEEE-754 says about NaN. That is why the operators below ask `ord_lt(r)`
/// rather than `r < 0` — `INCOMPARABLE > 0` is true, and reading the sign is
/// exactly the bug this constant exists to prevent.
const INCOMPARABLE: i32 = 2;

#[inline] fn ord_lt(r: i32) -> bool { r == -1 }
#[inline] fn ord_le(r: i32) -> bool { r == -1 || r == 0 }
#[inline] fn ord_gt(r: i32) -> bool { r == 1 }
#[inline] fn ord_ge(r: i32) -> bool { r == 1 || r == 0 }

fn cmp_direct(va: &Value, vb: &Value) -> i32 {
    use std::cmp::Ordering;
    fn ord(o: Ordering) -> i32 { match o { Ordering::Less => -1, Ordering::Equal => 0, Ordering::Greater => 1 } }
    match (va, vb) {
        (Value::Int(x), Value::Int(y))     => ord(x.cmp(y)),
        // `unwrap_or(Equal)` here made NaN equal to every float, including
        // itself: `partial_cmp` returns None precisely when one side is NaN, and
        // callers read 0 as "equal". INCOMPARABLE is a non-zero code, so `==` is
        // false and `!=` is true — which is what IEEE-754 says about NaN, and
        // what the other three engines already did.
        (Value::Float(x), Value::Float(y)) => x.partial_cmp(y).map_or(INCOMPARABLE, ord),
        (Value::Int(x), Value::Float(y))   => (*x as f64).partial_cmp(y).map_or(INCOMPARABLE, ord),
        (Value::Float(x), Value::Int(y))   => x.partial_cmp(&(*y as f64)).map_or(INCOMPARABLE, ord),
        (Value::String(x), Value::String(y)) => ord(x.as_str().cmp(y.as_str())),
        (Value::Char(x), Value::Char(y))   => ord(x.cmp(y)),
        (Value::Bool(x), Value::Bool(y))   => ord(x.cmp(y)),
        // Unit is equal to Unit. There is one Unit value, so this is both the
        // type test and the value test, and `##_ == ##_` had better be `#1` now
        // that `##_` can be written (GAP-ZYB-009).
        //
        // Missing here, this VM said a Unit was not equal to ITSELF while the
        // other two engines said it was — the fourth arm to go missing from a
        // comparison in this file, after `Array` (DM-02), `NamedTuple` (DM-22)
        // and `Function` (BUG-ZYB-012). `Value::equals` had it; this does not
        // share code with it, and that is the whole defect.
        (Value::Unit, Value::Unit)         => 0,
        (Value::Tuple(x), Value::Tuple(y)) => {
            if x.len() != y.len() { return 1; }
            for (a, b) in x.iter().zip(y.iter()) {
                let r = cmp_direct(a, b);
                if r != 0 { return r; }
            }
            0
        }
        // Same arm for arrays, and for the same reason as in `Value::equals`
        // above: without it `[1,2,3] == [1,2,3]` was #0 in the VM alone (DM-02).
        //
        // This function returns an ordering code and `==` reads 0 as equal, so a
        // non-zero result also makes `<>` true — which is all `==`/`<>` need. It
        // is not an order on arrays: no engine defines one, and the first
        // differing element's code is returned only so equality is decided.
        (Value::Array(x), Value::Array(y)) => {
            if x.len() != y.len() { return 1; }
            for (a, b) in x.iter().zip(y.iter()) {
                let r = cmp_direct(a, b);
                if r != 0 { return r; }
            }
            0
        }
        // Two dictionaries are equal when they hold the same keys with the same
        // values (DM-22, decided 2026-08-19). Key ORDER is not part of it: two
        // dictionaries built in a different order still hold the same thing, so
        // this looks each key up rather than zipping.
        (Value::NamedTuple(x), Value::NamedTuple(y)) => {
            if x.len() != y.len() { return 1; }
            for (ka, va) in x.iter() {
                match y.iter().find(|(kb, _)| kb == ka) {
                    Some((_, vb)) if cmp_direct(va, vb) == 0 => {}
                    _ => return 1,
                }
            }
            0
        }
        // Two functions are equal when they are THE SAME function (BUG-ZYB-012)
        // — see `Value::equals`, which this has to agree with, because the two
        // dispatch loops of this VM reach equality through different doors:
        // one calls `equals` and the other calls this.
        //
        // There is no ORDER on functions and none is implied: `1` only means
        // "not equal", which is what `==` and `<>` read.
        (Value::Function(ia, aa), Value::Function(ib, ab)) => {
            if ia == ib && aa == ab { 0 } else { 1 }
        }
        (Value::Closure(ia, aa, ua), Value::Closure(ib, ab, ub)) => {
            if ia == ib && aa == ab && Rc::ptr_eq(ua, ub) { 0 } else { 1 }
        }
        _ => 1,
    }
}

/// Human-readable description of what an `InputKind` expects (for re-prompt hints and
/// the EOF error). Kept in sync with the tree-walker's `describe_input_cast`.
fn vm_describe_input_kind(kind: &InputKind) -> String {
    match kind {
        InputKind::Raw | InputKind::Text { max: None } => "text".to_string(),
        InputKind::Numeric | InputKind::Float => "a number".to_string(),
        InputKind::Decimal { total, decimals } => format!(
            "a number with up to {} digits and {} decimals", total, decimals
        ),
        InputKind::Int { max_digits: Some(n) } => format!("an integer of up to {} digits", n),
        InputKind::Int { max_digits: None } => "an integer".to_string(),
        InputKind::Text { max: Some(n) } => format!("text of up to {} characters", n),
        InputKind::Char => "a single character".to_string(),
    }
}

/// Validate a trimmed input line against an `InputKind`, producing the typed VM value
/// or `Err(hint)`. Mirrors the tree-walker's `validate_input` so both engines agree.
fn vm_validate_input(s: &str, kind: &InputKind) -> Result<Value, String> {
    match kind {
        InputKind::Raw => Ok(Value::String(ZyStr::new(s.to_string()))),
        InputKind::Numeric => {
            match num::parse(s) {
                num::Num::Int(i) => Ok(Value::Int(i)),
                num::Num::Float(f) => Ok(Value::Float(f)),
                num::Num::None => match normalize_unicode_digits(s).map(|n| num::parse(&n)) {
                    Some(num::Num::Int(i)) => Ok(Value::Int(i)),
                    Some(num::Num::Float(f)) => Ok(Value::Float(f)),
                    _ => Ok(Value::String(ZyStr::new(s.to_string()))),
                },
            }
        }
        InputKind::Float => ascii_digits(s).parse::<f64>()
            .map(Value::Float)
            .map_err(|_| vm_describe_input_kind(kind)),
        InputKind::Decimal { total, decimals } => vm_validate_decimal(&ascii_digits(s), *total, *decimals)
            .map(Value::Float)
            .ok_or_else(|| vm_describe_input_kind(kind)),
        InputKind::Int { max_digits } => vm_validate_int(&ascii_digits(s), *max_digits)
            .map(Value::Int)
            .ok_or_else(|| vm_describe_input_kind(kind)),
        InputKind::Text { max } => {
            let too_long = matches!(max, Some(n) if s.chars().count() > *n as usize);
            if too_long { Err(vm_describe_input_kind(kind)) }
            else { Ok(Value::String(ZyStr::new(s.to_string()))) }
        }
        InputKind::Char => {
            let mut it = s.chars();
            match (it.next(), it.next()) {
                (Some(c), None) => Ok(Value::Char(c)),
                _ => Err(vm_describe_input_kind(kind)),
            }
        }
    }
}

/// Parse `s` as an integer with at most `max_digits` digits (ignoring a leading sign).
fn vm_validate_int(s: &str, max_digits: Option<u32>) -> Option<i64> {
    let n: i64 = s.parse().ok()?;
    if let Some(maxd) = max_digits {
        if s.chars().filter(|c| c.is_ascii_digit()).count() > maxd as usize {
            return None;
        }
    }
    Some(n)
}

/// Parse `s` as a fixed-format decimal (optional sign, digits, at most one `.`,
/// at most `decimals` fractional and `total` overall digits; no scientific notation).
fn vm_validate_decimal(s: &str, total: u32, decimals: u32) -> Option<f64> {
    let value: f64 = s.parse().ok()?;
    let body = s.strip_prefix(['+', '-']).unwrap_or(s);
    let mut int_digits = 0u32;
    let mut frac_digits = 0u32;
    let mut seen_dot = false;
    for c in body.chars() {
        if c == '.' {
            if seen_dot { return None; }
            seen_dot = true;
        } else if c.is_ascii_digit() {
            if seen_dot { frac_digits += 1; } else { int_digits += 1; }
        } else {
            return None;
        }
    }
    if frac_digits > decimals || int_digits + frac_digits > total {
        return None;
    }
    Some(value)
}

/// The ASCII form of a numeric string written in any of the 69 supported digit
/// scripts (mirrors `zymbol_interpreter::data_ops::ascii_digits`): every numeric
/// cast normalizes through this, so a number the program rendered under an
/// active numeral mode parses back exactly like its ASCII twin.
fn ascii_digits(s: &str) -> std::borrow::Cow<'_, str> {
    if s.is_ascii() {
        return std::borrow::Cow::Borrowed(s);
    }
    match normalize_unicode_digits(s) {
        Some(normalized) => std::borrow::Cow::Owned(normalized),
        None => std::borrow::Cow::Borrowed(s),
    }
}

// The one normalizer, shared with the tree-walker and with the lexer's own
// literal scanner — see `zymbol_lexer::digit_blocks::ascii_number`.
use zymbol_lexer::digit_blocks::ascii_number as normalize_unicode_digits;

#[inline(always)]
/// The `##` family of a VM error.
///
/// A variant that says what went wrong decides it; everything else is read from
/// the message by the rule the tree-walker uses (`zymbol_common::errkind`), so a
/// native's `incompatible argument type(s)` is a ##Type in both engines — it was
/// a ##_ here, and `:! ##Type` could not meet it (GLB-010).
fn vm_error_kind(e: &VmError) -> &'static str {
    match e {
        VmError::TypeError { .. } | VmError::CastError { .. } | VmError::TypeMsg(_) => "Type",
        VmError::DivisionByZero | VmError::ModuloByZero => "Div",
        VmError::IntOverflow { .. } | VmError::CastOverflow { .. } => "Range",
        VmError::IndexOutOfBounds { .. } | VmError::IndexZero | VmError::IndexMsg(_) => "Index",
        VmError::Io(_) => "IO",
        VmError::Generic(m) | VmError::Located { message: m, .. } => {
            zymbol_common::errkind::error_kind_of_message(m)
        }
        _ => "_",
    }
}

fn get_chunk(program: &CompiledProgram, chunk_idx: usize) -> &Chunk {
    // chunk_idx == usize::MAX OR u32::MAX as usize both indicate the main chunk
    if chunk_idx >= program.functions.len() {
        &program.main
    } else {
        &program.functions[chunk_idx]
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// TuiGuard — restores terminal on drop, regardless of control-flow path
// ──────────────────────────────────────────────────────────────────────────────

struct TuiGuard;
impl Drop for TuiGuard {
    fn drop(&mut self) {
        let _ = crossterm::execute!(std::io::stdout(),
            crossterm::terminal::LeaveAlternateScreen,
            crossterm::cursor::Show);
        let _ = crossterm::terminal::disable_raw_mode();
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// VM — Sprint 5C: flat register stack
// ──────────────────────────────────────────────────────────────────────────────

pub struct VM<W: Write> {
    /// Flat register stack: all registers of all frames concatenated.
    /// value_stack[base..] are the registers of the current (top) frame.
    value_stack: Vec<Value>,
    /// Frame metadata stack (no registers — those live in value_stack)
    frame_stack: Vec<FrameInfo>,
    /// Scratch buffer for TailCall arg staging — reused, no alloc per call
    tco_buf: Vec<Value>,
    /// Pending output param writeback set by SetupOutputWriteback, consumed by Call
    pending_output_writeback: Vec<(usize, Reg)>,
    /// Interned string pool — ZyStr for each entry, shared across LoadStr calls
    string_rcs: Vec<ZyStr>,
    /// Active numeral mode: block base codepoint (0x0030 = ASCII default).
    numeral_mode: u32,
    /// Module-level global variables (mutable, shared across all calls)
    global_vars: Vec<Value>,
    /// Globals ended by `\`, and the name each one had, so a later read can say
    /// what happened instead of «'x' is undefined» (GLB-008). A map and not a
    /// set because the message needs the name and `LoadGlobal` only carries an
    /// index. Assigning the name again revives it, which `StoreGlobal` does.
    destroyed_globals: std::collections::HashMap<u16, String>,
    /// Absolute value-stack slots of function locals ended with `\` that have
    /// not been assigned since (ZYVM-005). Empty unless a program destroys one.
    destroyed_slots: std::collections::HashSet<usize>,
    /// CLI arguments passed after the script path (argv[1..], skipping --vm flags)
    cli_args: Vec<String>,
    /// The code a top-level `<~ n` asked the program to end with (GAP-ZYB-006).
    exit_code: Option<i64>,
    /// The instruction the dispatch loop is on, and which chunk it belongs to
    /// (`u32::MAX` = the main chunk). Written once per instruction so that an
    /// error leaving the loop can be told where it came from — the loop's own
    /// `ip` is a local, and by the time `?` has carried the error out of `run`
    /// there is nothing left to ask.
    cur_ip: u32,
    cur_chunk: u32,
    /// The error `raise!` is carrying to a handler, between the jump and the
    /// `TryLand` that receives it.
    in_flight: Option<Pending>,
    /// Terminal guards: dropping one restores the terminal, so they live as long
    /// as the TUI they opened — across every nested run of the loop.
    tui_stack: Vec<TuiGuard>,
    output: W,
}

impl<W: Write> VM<W> {
    pub fn new(output: W) -> Self {
        Self {
            value_stack: Vec::with_capacity(4096),  // ~160KB, covers deep recursion
            frame_stack: Vec::with_capacity(256),
            tco_buf: Vec::with_capacity(16),
            pending_output_writeback: Vec::new(),
            string_rcs: Vec::new(),
            numeral_mode: 0x0030, // ASCII_BASE default
            global_vars: Vec::new(),
            destroyed_globals: std::collections::HashMap::new(),
            destroyed_slots: std::collections::HashSet::new(),
            cli_args: Vec::new(),
            exit_code: None,
            cur_ip: 0,
            cur_chunk: u32::MAX,
            in_flight: None,
            tui_stack: Vec::new(),
            output,
        }
    }

    /// Set CLI arguments before running (argv after the script path, minus VM flags).
    pub fn set_cli_args(&mut self, args: Vec<String>) {
        self.cli_args = args;
    }

    /// The exit status a top-level `<~ n` asked for, if the program asked
    /// (GAP-ZYB-006).
    pub fn exit_code(&self) -> Option<i64> {
        self.exit_code
    }

    /// Stringify a value under the active numeral mode.
    ///
    /// Mirrors `Value::to_string_repr()` except every digit — including the ones
    /// nested in arrays and tuples — maps through `self.numeral_mode`. Used by
    /// every string-building instruction (ConcatStr, ConcatBuild, BuildStr,
    /// PrintAt) so `#d0d9#` reaches strings, not just bare `>>`.
    fn numeral_repr(&self, v: &Value) -> String {
        if self.numeral_mode == ASCII_BASE {
            return v.to_string_repr();
        }
        v.to_display_in(self.numeral_mode)
    }

    pub fn run(&mut self, program: &CompiledProgram) -> Result<(), VmError> {
        match self.run_inner(program) {
            Ok(()) => Ok(()),
            // The one place a VM error learns where it came from: `cur_ip` and
            // `cur_chunk` still hold the instruction that raised, and the chunk
            // knows which line of which file it was compiled from.
            Err(e) => Err(self.locate(e, program)),
        }
    }

    /// Attach the failing instruction's source position, when the program was
    /// compiled with one. A chunk with no `src` (an older bundle, or one built
    /// before positions existed) reports nothing rather than line 0.
    fn locate(&self, err: VmError, program: &CompiledProgram) -> VmError {
        // Already located — an inner frame that knew better wins.
        if matches!(err, VmError::Located { .. }) {
            return err;
        }
        let chunk = if self.cur_chunk == u32::MAX {
            &program.main
        } else {
            match program.functions.get(self.cur_chunk as usize) {
                Some(c) => c,
                None => return err,
            }
        };
        let Some(pos) = chunk.src.get(self.cur_ip as usize) else { return err };
        if pos.line == 0 {
            return err;
        }
        let Some(file) = program.files.get(pos.file as usize) else { return err };
        if file.is_empty() {
            return err;
        }
        VmError::Located {
            message: err.to_string(),
            file: file.clone(),
            line: pos.line,
            column: pos.column,
        }
    }

    fn run_inner(&mut self, program: &CompiledProgram) -> Result<(), VmError> {
        // Reset flat stack for this execution
        self.value_stack.clear();
        self.frame_stack.clear();
        // Pre-allocate to avoid Vec reallocation during deep recursion.
        // 128K Value slots (~2MB) covers fib(30)'s 2.7M-call stack depth of ≤30.
        if self.value_stack.capacity() < (1 << 17) {
            self.value_stack.reserve((1 << 17) - self.value_stack.capacity());
        }
        if self.frame_stack.capacity() < 1024 {
            self.frame_stack.reserve(1024 - self.frame_stack.capacity());
        }

        // Pre-intern string pool: O(n) once, then LoadStr is O(1) Rc clone
        self.string_rcs = program.string_pool.iter().map(|s| ZyStr::from_str_ref(s)).collect();

        // Initialize global variables from program inits
        self.global_vars = program.global_var_inits.iter().map(global_init_value).collect();

        // Push initial frame for main chunk
        let num_regs = program.main.num_registers as usize;
        self.value_stack.resize(num_regs, Value::Unit);
        let main_frame = FrameInfo {
            base: 0,
            ip: 0,
            chunk_idx: u32::MAX,
            return_reg: 0,
            catch_ip: u32::MAX,
            error: None,
            writeback: None,
        };
        self.frame_stack.push(main_frame);

        let result = self.exec(program, 0).map(|_| ());
        // The terminal is given back on every way out, error included.
        self.tui_stack.clear();
        self.in_flight = None;
        result
    }

    /// The dispatch loop: run from the frame on top of the stack until the frame
    /// at index `floor` returns, and hand back what it returned.
    ///
    /// `run` enters it once, at the main frame. A higher-order operator enters it
    /// again for each call it makes, after pushing the callee's frame — so the
    /// function `$>` calls runs on the same loop as everything else. It used to
    /// run on a second, smaller loop that knew 96 of the instructions and skipped
    /// the rest in silence: `["a,b"]$> (s -> s$/ ',')` answered `[()]` (ZYVM-004).
    ///
    /// An error with no handler at or above `floor` unwinds to `floor` and
    /// returns, and the operator raises it in the loop outside — where the
    /// `!?` around the `$>` can meet it.
    fn exec(&mut self, program: &CompiledProgram, floor: usize) -> Result<Value, VmError> {
        // Local variables — avoid frame_stack.last() overhead on every instruction
        let (mut ip, mut chunk_idx, mut base): (usize, usize, usize) = {
            let top = self.frame_stack.last().unwrap();
            (top.ip as usize, top.chunk_idx as usize, top.base as usize)
        };

        // ── Hot-path register access macros ──────────────────────────────────
        // SAFETY: base + reg < value_stack.len() by compiler construction.
        macro_rules! rreg {
            ($r:expr) => { unsafe { self.value_stack.get_unchecked(base + $r as usize) } }
        }
        macro_rules! wreg {
            ($r:expr, $v:expr) => { unsafe { *self.value_stack.get_unchecked_mut(base + $r as usize) = $v } }
        }
        // ri!: read register as Int for a POSITION — an index, a count, a
        // repetition. Not for an arithmetic operand: those go through `ri2!`,
        // which knows which operator asked and can therefore say so.
        //
        // This macro used to carry the `+ is arithmetic only …` guidance for a
        // String, because it was the only Int reader and `+` was the commonest
        // way to reach it. That made the message a property of the code route
        // rather than of the operator (ZYVM-002): `7 - "a"` quoted the guidance
        // for `+`, and `arr["x"]` would have quoted it too.
        macro_rules! ri {
            ($r:expr) => {
                match unsafe { self.value_stack.get_unchecked(base + $r as usize) } {
                    Value::Int(n) => *n,
                    other => raise!(VmError::TypeError { expected: "a number", got: other.type_name().to_string() }),
                }
            }
        }
        // rf!: read register as Float (Int coerced), raise TypeError on mismatch.
        // As with `ri!`, arithmetic operands use `rf2!` instead.
        macro_rules! rf {
            ($r:expr) => {
                match unsafe { self.value_stack.get_unchecked(base + $r as usize) } {
                    Value::Float(n) => *n,
                    Value::Int(n) => *n as f64,
                    other => raise!(VmError::TypeError { expected: "Float", got: other.type_name().to_string() }),
                }
            }
        }
        // ri2!/rf2!: read BOTH arithmetic operands, and on failure raise the
        // tree-walker's message for the operator that asked (ZYVM-002).
        //
        // The message is built in a first step that ends the borrow of
        // `value_stack`, because `raise!` needs `self` mutably — the reason
        // these are not one expression.
        macro_rules! ri2 {
            ($a:expr, $b:expr, $op:expr) => {{
                let read = match (rreg!($a), rreg!($b)) {
                    (Value::Int(x), Value::Int(y)) => Ok((*x, *y)),
                    (va, vb) => Err(arith_type_error($op, va, vb)),
                };
                match read { Ok(v) => v, Err(msg) => raise!(VmError::Generic(msg)) }
            }}
        }
        macro_rules! rf2 {
            ($a:expr, $b:expr, $op:expr) => {{
                let read = match (rreg!($a), rreg!($b)) {
                    (Value::Float(x), Value::Float(y)) => Ok((*x, *y)),
                    (Value::Float(x), Value::Int(y))   => Ok((*x, *y as f64)),
                    (Value::Int(x), Value::Float(y))   => Ok((*x as f64, *y)),
                    (Value::Int(x), Value::Int(y))     => Ok((*x as f64, *y as f64)),
                    (va, vb) => Err(arith_type_error($op, va, vb)),
                };
                match read { Ok(v) => v, Err(msg) => raise!(VmError::Generic(msg)) }
            }}
        }
        // The immediate forms: the other operand is the literal the compiler
        // folded in, so the message names it exactly as the general form would.
        macro_rules! ri_imm {
            ($r:expr, $imm:expr, $op:expr) => {{
                let read = match rreg!($r) {
                    Value::Int(x) => Ok(*x),
                    va => Err(arith_type_error($op, va, &Value::Int($imm as i64))),
                };
                match read { Ok(v) => v, Err(msg) => raise!(VmError::Generic(msg)) }
            }}
        }

        // raise_kind!: carry an error to the nearest armed handler at or above
        // `floor`, with its kind already decided. A rethrow uses this directly,
        // so the error leaves with the kind it arrived with.
        //
        // L16 fix: an error raised inside a called function must reach a catch
        // armed in ANY ancestor frame, not just the top one. Walk the frame stack
        // for the nearest active catch, pop the frames above it (releasing their
        // registers), and resume at the catch — whose `TryLand` receives the
        // error from `in_flight`.
        //
        // Never write one inside a `for`/`while`/`loop` of an instruction: the
        // `continue` below would continue THAT loop, and a label cannot reach
        // into a macro. Break out with the error and raise after the loop.
        macro_rules! raise_kind {
            ($e:expr, $kind:expr) => {{
                let _err: VmError = $e;
                let _kind: &'static str = $kind;
                let target = self.frame_stack.iter()
                    .rposition(|f| f.catch_ip != u32::MAX)
                    .filter(|&t| t >= floor);
                if let Some(target) = target {
                    while self.frame_stack.len() - 1 > target {
                        let callee_base = self.frame_stack.last().unwrap().base as usize;
                        self.frame_stack.pop();
                        self.value_stack.truncate(callee_base);
                    }
                    let catch = {
                        let frame = self.frame_stack.last_mut().unwrap();
                        base = frame.base as usize;
                        chunk_idx = frame.chunk_idx as usize;
                        // Disarmed until `TryLand` re-arms the one outside it: an
                        // error raised before then must not come back here.
                        std::mem::replace(&mut frame.catch_ip, u32::MAX)
                    };
                    self.in_flight = Some(Pending {
                        tag: 0,
                        err: _err,
                        kind: _kind,
                        origin_ip: self.cur_ip,
                        origin_chunk: self.cur_chunk,
                    });
                    ip = catch as usize;
                    continue;
                }
                // Nothing here meets it. A nested run gives back the frames it
                // pushed, so the operator that started it sees the stack it had.
                if floor > 0 && self.frame_stack.len() > floor {
                    let floor_base = self.frame_stack[floor].base as usize;
                    self.frame_stack.truncate(floor);
                    self.value_stack.truncate(floor_base);
                }
                return Err(_err);
            }};
        }
        macro_rules! raise {
            ($e:expr) => {{
                let _err: VmError = $e;
                let _kind = vm_error_kind(&_err);
                raise_kind!(_err, _kind)
            }};
        }

        // An integer result, or the overflow the tree-walker would have raised.
        // Every integer instruction goes through this: the VM used to use the
        // `wrapping_*` family throughout, so `10 ^ 20` answered with the low 64
        // bits of the true product and no program could tell.
        macro_rules! iop {
            ($v:expr, $a:expr, $op:expr, $b:expr) => {
                match $v {
                    Some(n) => n,
                    None => raise!(VmError::IntOverflow { a: $a, op: $op, b: $b }),
                }
            };
        }

        // Ordering comparison, raising the tree-walker's error when the two
        // values are not comparable (a number against non-numeric text).
        // rb2!: both logical operands as Bool, or the tree-walker's refusal.
        // The left operand is reported first, as there — `7 && "a"` names the 7.
        macro_rules! rb2 {
            ($a:expr, $b:expr, $op:expr) => {{
                let read = match (rreg!($a), rreg!($b)) {
                    (Value::Bool(x), Value::Bool(y)) => Ok((*x, *y)),
                    (va, vb) => Err(logical_type_error($op, if matches!(va, Value::Bool(_)) { vb } else { va })),
                };
                match read { Ok(v) => v, Err(msg) => raise!(VmError::Generic(msg)) }
            }}
        }
        // rn!: the operand of unary `-`. Its refusal used to come out of `ri!`
        // as the guidance for `+` (ZYVM-002): `-"a"` was refused with advice
        // about concatenation.
        macro_rules! rn {
            ($r:expr) => {{
                let read = match rreg!($r) {
                    Value::Int(n)   => Ok(*n),
                    va => Err(format!("negation requires numeric operand, got {}", va.type_name())),
                };
                match read { Ok(v) => v, Err(msg) => raise!(VmError::Generic(msg)) }
            }}
        }
        macro_rules! ord_or_raise {
            ($a:expr, $b:expr, $op:expr) => {
                match cmp_order(rreg!($a), rreg!($b)) {
                    Some(r) => r,
                    None => raise!(VmError::Generic(
                        cmp_order_error(rreg!($a), rreg!($b), $op)
                    )),
                }
            };
        }
        // The same rule against a folded literal. The Int case is answered
        // without building a Value, so the loop counter pays nothing for it.
        macro_rules! ord_imm_or_raise {
            ($r:expr, $imm:expr, $op:expr) => {{
                match rreg!($r) {
                    Value::Int(n) => { let n = *n; let i = $imm as i64; if n < i { -1 } else if n > i { 1 } else { 0 } }
                    _ => {
                        let rhs = Value::Int($imm as i64);
                        let read = match cmp_order(rreg!($r), &rhs) {
                            Some(r) => Ok(r),
                            None => Err(cmp_order_error(rreg!($r), &rhs, $op)),
                        };
                        match read { Ok(r) => r, Err(msg) => raise!(VmError::Generic(msg)) }
                    }
                }
            }};
        }


        loop {
            // chunk borrows only from `program` — no conflict with `self.value_stack`
            let chunk = get_chunk(program, chunk_idx);
            if ip >= chunk.instructions.len() {
                // Fell off end of chunk — implicit return Unit
                if self.frame_stack.len() == 1 {
                    return Ok(Value::Unit);
                }
                // The frame a higher-order operator pushed ran off its end.
                if self.frame_stack.len() - 1 == floor {
                    self.frame_stack.pop();
                    self.value_stack.truncate(base);
                    return Ok(Value::Unit);
                }
                let (return_reg, wb) = {
                    let frame = self.frame_stack.last().unwrap();
                    let wb: Vec<(u16, Value)> = if let Some(writeback) = &frame.writeback {
                        writeback.iter()
                            .map(|&(param_idx, caller_reg)| {
                                (caller_reg, self.value_stack[base + param_idx].clone())
                            })
                            .collect()
                    } else {
                        Vec::new()
                    };
                    (frame.return_reg, wb)
                };
                self.frame_stack.pop();
                // Truncate: remove callee's registers from flat stack
                self.value_stack.truncate(base);
                // Restore caller's base
                base = self.frame_stack.last().unwrap().base as usize;
                // Write Unit as return value
                self.value_stack[base + return_reg as usize] = Value::Unit;
                // Apply output param writeback
                for (caller_reg, val) in wb {
                    self.value_stack[base + caller_reg as usize] = val;
                }
                // Restore caller's ip and chunk_idx
                let caller = self.frame_stack.last().unwrap();
                ip = caller.ip as usize;
                chunk_idx = caller.chunk_idx as usize;
                continue;
            }

            // Reference (no clone!) — safe because chunk borrows from program, not self
            let instr = &chunk.instructions[ip];
            // Where we are, for whoever has to report an error. Two stores of a
            // register-resident u32; the alternative was threading a position
            // through every `?` in the match below.
            self.cur_ip = ip as u32;
            self.cur_chunk = chunk_idx as u32;
            // Advance IP before executing (branches will override this)
            ip += 1;

            // Use & patterns to copy Copy fields without cloning the instruction
            match instr {
                // ── Literals ────────────────────────────────────────────────
                &Instruction::LoadInt(dst, n)    => wreg!(dst, Value::Int(n)),
                &Instruction::LoadFloat(dst, n)  => wreg!(dst, Value::Float(n)),
                &Instruction::LoadBool(dst, b)   => wreg!(dst, Value::Bool(b)),
                &Instruction::LoadStr(dst, idx)  => {
                    wreg!(dst, Value::String(self.string_rcs[idx as usize].clone()));
                }
                &Instruction::LoadChar(dst, c)   => wreg!(dst, Value::Char(c)),
                &Instruction::LoadUnit(dst)       => wreg!(dst, Value::Unit),

                // ── Register moves ──────────────────────────────────────────
                &Instruction::CopyReg(dst, src) => {
                    let v = rreg!(src).clone();
                    wreg!(dst, v);
                }
                &Instruction::MoveReg(dst, src) => {
                    let v = mem::replace(
                        unsafe { self.value_stack.get_unchecked_mut(base + src as usize) },
                        Value::Unit,
                    );
                    unsafe { *self.value_stack.get_unchecked_mut(base + dst as usize) = v; }
                }

                // ── Integer arithmetic — dynamic dispatch for Float registers ──
                // The compiler selects AddInt/etc. for registers of Unknown static type
                // (e.g., from ArrayGet). At runtime we fall back to Float arithmetic
                // when either operand is Float, matching tree-walker behaviour.
                &Instruction::AddInt(dst, a, b) => {
                    if matches!(unsafe { self.value_stack.get_unchecked(base + a as usize) }, Value::Float(_))
                    || matches!(unsafe { self.value_stack.get_unchecked(base + b as usize) }, Value::Float(_)) {
                        let (fa, fb) = rf2!(a, b, "+"); wreg!(dst, Value::Float(fa + fb));
                    } else { let (va, vb) = ri2!(a, b, "+"); wreg!(dst, Value::Int(iop!(num::add(va, vb), va, "+", vb))); }
                }
                &Instruction::SubInt(dst, a, b) => {
                    if matches!(unsafe { self.value_stack.get_unchecked(base + a as usize) }, Value::Float(_))
                    || matches!(unsafe { self.value_stack.get_unchecked(base + b as usize) }, Value::Float(_)) {
                        let (fa, fb) = rf2!(a, b, "-"); wreg!(dst, Value::Float(fa - fb));
                    } else { let (va, vb) = ri2!(a, b, "-"); wreg!(dst, Value::Int(iop!(num::sub(va, vb), va, "-", vb))); }
                }
                &Instruction::MulInt(dst, a, b) => {
                    if matches!(unsafe { self.value_stack.get_unchecked(base + a as usize) }, Value::Float(_))
                    || matches!(unsafe { self.value_stack.get_unchecked(base + b as usize) }, Value::Float(_)) {
                        let (fa, fb) = rf2!(a, b, "*"); wreg!(dst, Value::Float(fa * fb));
                    } else { let (va, vb) = ri2!(a, b, "*"); wreg!(dst, Value::Int(iop!(num::mul(va, vb), va, "*", vb))); }
                }
                &Instruction::DivInt(dst, a, b) => {
                    if matches!(unsafe { self.value_stack.get_unchecked(base + a as usize) }, Value::Float(_))
                    || matches!(unsafe { self.value_stack.get_unchecked(base + b as usize) }, Value::Float(_)) {
                        let (fa, fb) = rf2!(a, b, "/"); wreg!(dst, Value::Float(fa / fb));
                    } else {
                        let (va, vb) = ri2!(a, b, "/");
                        if vb == 0 { raise!(VmError::DivisionByZero); }
                        wreg!(dst, Value::Int(va / vb));
                    }
                }
                &Instruction::ModInt(dst, a, b) => {
                    if matches!(unsafe { self.value_stack.get_unchecked(base + a as usize) }, Value::Float(_))
                    || matches!(unsafe { self.value_stack.get_unchecked(base + b as usize) }, Value::Float(_)) {
                        let (fa, fb) = rf2!(a, b, "%");
                        // As in the integer branch below and in `DivFloat`: a
                        // zero divisor is an error whichever type it was written
                        // as, not a NaN.
                        if fb == 0.0 { raise!(VmError::ModuloByZero); }
                        wreg!(dst, Value::Float(fa % fb));
                    } else {
                        let (va, vb) = ri2!(a, b, "%");
                        if vb == 0 { raise!(VmError::ModuloByZero); }
                        wreg!(dst, Value::Int(va % vb));
                    }
                }
                &Instruction::PowInt(dst, a, b) => {
                    if matches!(unsafe { self.value_stack.get_unchecked(base + a as usize) }, Value::Float(_))
                    || matches!(unsafe { self.value_stack.get_unchecked(base + b as usize) }, Value::Float(_)) {
                        let (fa, fb) = rf2!(a, b, "^"); wreg!(dst, Value::Float(fa.powf(fb)));
                    } else {
                        let (va, vb) = ri2!(a, b, "^");
                        // A negative exponent is a float operation, as in the
                        // tree-walker. This used to answer Int(0), so `2 ^ -2`
                        // was 0 here and 0.25 there.
                        if vb < 0 {
                            wreg!(dst, Value::Float((va as f64).powf(vb as f64)));
                        } else {
                            let e = u32::try_from(vb).unwrap_or(u32::MAX);
                            wreg!(dst, Value::Int(iop!(num::pow(va, e), va, "^", vb)));
                        }
                    }
                }
                &Instruction::NegInt(dst, src) => {
                    if matches!(unsafe { self.value_stack.get_unchecked(base + src as usize) }, Value::Float(_)) {
                        let v = rf!(src); wreg!(dst, Value::Float(-v));
                    } else { let v = rn!(src); wreg!(dst, Value::Int(-v)); }
                }

                // ── Integer immediate variants ──────────────────────────────
                &Instruction::AddIntImm(dst, src, imm) => {
                    if matches!(unsafe { self.value_stack.get_unchecked(base + src as usize) }, Value::Float(_)) {
                        let v = rf!(src); wreg!(dst, Value::Float(v + imm as f64));
                    } else { let v = ri_imm!(src, imm, "+"); wreg!(dst, Value::Int(iop!(num::add(v, imm as i64), v, "+", imm as i64))); }
                }
                &Instruction::SubIntImm(dst, src, imm) => {
                    if matches!(unsafe { self.value_stack.get_unchecked(base + src as usize) }, Value::Float(_)) {
                        let v = rf!(src); wreg!(dst, Value::Float(v - imm as f64));
                    } else { let v = ri_imm!(src, imm, "-"); wreg!(dst, Value::Int(iop!(num::sub(v, imm as i64), v, "-", imm as i64))); }
                }
                &Instruction::MulIntImm(dst, src, imm) => {
                    if matches!(unsafe { self.value_stack.get_unchecked(base + src as usize) }, Value::Float(_)) {
                        let v = rf!(src); wreg!(dst, Value::Float(v * imm as f64));
                    } else { let v = ri_imm!(src, imm, "*"); wreg!(dst, Value::Int(iop!(num::mul(v, imm as i64), v, "*", imm as i64))); }
                }
                &Instruction::CmpEqImm(dst, src, imm) => {
                    match num_eq_imm(rreg!(src), imm as i64) {
                        Some(r) => wreg!(dst, Value::Bool(r)),
                        None => wreg!(dst, Value::Bool(false)),
                    }
                }
                &Instruction::CmpNeImm(dst, src, imm) => {
                    match num_eq_imm(rreg!(src), imm as i64) {
                        Some(r) => wreg!(dst, Value::Bool(!r)),
                        None => wreg!(dst, Value::Bool(true)),
                    }
                }
                // The immediate comparisons keep the Int fast path — this is
                // the loop counter's instruction — and hand everything else to
                // the same ordering rule the general form uses. They used to
                // read the register through `ri!`, so `'a' < 7` was refused
                // here as "this needs a number and got Char" and there as
                // "cannot compare values with operator 'Lt': Char and Int":
                // one comparison, two refusals, decided by whether the
                // right-hand side happened to be a literal small enough to fold.
                &Instruction::CmpLtImm(dst, src, imm) => { let r = ord_imm_or_raise!(src, imm, "Lt"); wreg!(dst, Value::Bool(ord_lt(r))); }
                &Instruction::CmpLeImm(dst, src, imm) => { let r = ord_imm_or_raise!(src, imm, "Le"); wreg!(dst, Value::Bool(ord_le(r))); }
                &Instruction::CmpGtImm(dst, src, imm) => { let r = ord_imm_or_raise!(src, imm, "Gt"); wreg!(dst, Value::Bool(ord_gt(r))); }
                &Instruction::CmpGeImm(dst, src, imm) => { let r = ord_imm_or_raise!(src, imm, "Ge"); wreg!(dst, Value::Bool(ord_ge(r))); }

                // ── Float arithmetic ────────────────────────────────────────
                &Instruction::AddFloat(dst, a, b) => { let (va, vb) = rf2!(a, b, "+"); wreg!(dst, Value::Float(va + vb)); }
                &Instruction::SubFloat(dst, a, b) => { let (va, vb) = rf2!(a, b, "-"); wreg!(dst, Value::Float(va - vb)); }
                &Instruction::MulFloat(dst, a, b) => { let (va, vb) = rf2!(a, b, "*"); wreg!(dst, Value::Float(va * vb)); }
                &Instruction::DivFloat(dst, a, b) => {
                    let (va, vb) = (rf!(a), rf!(b));
                    if vb == 0.0 { raise!(VmError::DivisionByZero); }
                    wreg!(dst, Value::Float(va / vb));
                }
                &Instruction::PowFloat(dst, a, b) => { let (va, vb) = rf2!(a, b, "^"); wreg!(dst, Value::Float(va.powf(vb))); }
                &Instruction::NegFloat(dst, src)  => {
                    let read = match rreg!(src) {
                        Value::Float(n) => Ok(*n),
                        Value::Int(n)   => Ok(*n as f64),
                        va => Err(format!("negation requires numeric operand, got {}", va.type_name())),
                    };
                    let v = match read { Ok(v) => v, Err(msg) => raise!(VmError::Generic(msg)) };
                    wreg!(dst, Value::Float(-v));
                }

                // ── Type conversion ─────────────────────────────────────────
                &Instruction::IntToFloat(dst, src) => {
                    let v = match rreg!(src) {
                        Value::Int(n)   => *n as f64,
                        Value::Float(f) => *f,
                        other => raise!(VmError::CastError { op: "##.", got: other.type_name().to_string() }),
                    };
                    wreg!(dst, Value::Float(v));
                }
                &Instruction::FloatToIntRound(dst, src) => {
                    let v = match rreg!(src) {
                        Value::Float(f) => match num::from_f64(f.round()) {
                            Some(n) => n,
                            None => raise!(VmError::CastOverflow { op: "###" }),
                        },
                        Value::Int(n)   => *n,
                        other => raise!(VmError::CastError { op: "###", got: other.type_name().to_string() }),
                    };
                    wreg!(dst, Value::Int(v));
                }
                &Instruction::FloatToIntTrunc(dst, src) => {
                    let v = match rreg!(src) {
                        Value::Float(f) => match num::from_f64(f.trunc()) {
                            Some(n) => n,
                            None => raise!(VmError::CastOverflow { op: "##!" }),
                        },
                        Value::Int(n)   => *n,
                        // Char → its Unicode code point (matches the tree-walker).
                        Value::Char(c)  => *c as u32 as i64,
                        other => raise!(VmError::CastError { op: "##!", got: other.type_name().to_string() }),
                    };
                    wreg!(dst, Value::Int(v));
                }

                // ── String ops ──────────────────────────────────────────────
                &Instruction::ConcatStr(dst, a, b) => {
                    // In-place buffer reuse: only valid when dst == a, so the left
                    // register is overwritten anyway. try_into_string reuses the heap
                    // buffer when Rc::strong_count == 1, avoiding O(N) allocs in loops.
                    let can_reuse = dst == a && a != b;
                    let result = if can_reuse {
                        let left = std::mem::replace(
                            unsafe { self.value_stack.get_unchecked_mut(base + a as usize) },
                            Value::Unit,
                        );
                        match (left, unsafe { self.value_stack.get_unchecked(base + b as usize) }) {
                            (Value::String(l), Value::String(r)) => {
                                let r_str = r.as_str().to_string();
                                let mut s = l.try_into_string();
                                s.push_str(&r_str);
                                s
                            }
                            (Value::String(l), Value::Int(n)) => {
                                let n = *n;
                                let mut s = l.try_into_string();
                                push_numeral_int(&mut s, n, self.numeral_mode);
                                s
                            }
                            (Value::String(l), Value::Float(f)) => {
                                let f = *f;
                                let mut s = l.try_into_string();
                                push_numeral_float(&mut s, f, self.numeral_mode);
                                s
                            }
                            (l, r) => {
                                let ls = self.numeral_repr(&l);
                                let rs = self.numeral_repr(r);
                                let mut s = String::with_capacity(ls.len() + rs.len());
                                s.push_str(&ls);
                                s.push_str(&rs);
                                s
                            }
                        }
                    } else {
                        match (rreg!(a), rreg!(b)) {
                            (Value::String(l), Value::String(r)) => {
                                let mut s = String::with_capacity(l.len() + r.len());
                                s.push_str(l.as_ref());
                                s.push_str(r.as_ref());
                                s
                            }
                            (Value::String(l), Value::Int(n)) => {
                                let mut s = String::with_capacity(l.len() + 20);
                                s.push_str(l.as_ref());
                                push_numeral_int(&mut s, *n, self.numeral_mode);
                                s
                            }
                            (Value::Int(n), Value::String(r)) => {
                                let mut s = String::with_capacity(20 + r.len());
                                push_numeral_int(&mut s, *n, self.numeral_mode);
                                s.push_str(r.as_ref());
                                s
                            }
                            (Value::String(l), Value::Float(f)) => {
                                let mut s = String::with_capacity(l.len() + 24);
                                s.push_str(l.as_ref());
                                push_numeral_float(&mut s, *f, self.numeral_mode);
                                s
                            }
                            (l, r) => {
                                let ls = self.numeral_repr(l);
                                let rs = self.numeral_repr(r);
                                let mut s = String::with_capacity(ls.len() + rs.len());
                                s.push_str(&ls);
                                s.push_str(&rs);
                                s
                            }
                        }
                    };
                    wreg!(dst, Value::String(ZyStr::new(result)));
                }
                Instruction::ConcatBuild(dst, base_reg, item_regs) => {
                    let (dst, base_reg) = (*dst, *base_reg);
                    let base_val = rreg!(base_reg).clone();
                    let result = match base_val {
                        Value::Array(arr) => {
                            let mut new_arr = arr.as_ref().clone();
                            for &ir in item_regs {
                                new_arr.push(rreg!(ir).clone());
                            }
                            Value::Array(Rc::new(new_arr))
                        }
                        other => {
                            let mut s = self.numeral_repr(&other);
                            for &ir in item_regs {
                                let part = self.numeral_repr(rreg!(ir));
                                s.push_str(&part);
                            }
                            Value::String(ZyStr::new(s))
                        }
                    };
                    wreg!(dst, result);
                }
                &Instruction::StrLen(dst, src) => {
                    let n = match rreg!(src) {
                        Value::String(s)      => if s.is_ascii() { s.len() as i64 } else { s.chars().count() as i64 },
                        Value::Array(arr)     => arr.len() as i64,
                        other => raise!(VmError::TypeMsg(format!("cannot get length of {} - only arrays, tuples, and strings have length", other.type_label()))),
                    };
                    wreg!(dst, Value::Int(n));
                }

                &Instruction::StrRepeat(dst, str_reg, n_reg) => {
                    let result = {
                        let s = match &self.value_stack[base + str_reg as usize] {
                            Value::String(s) => s.as_str().to_owned(),
                            Value::Char(c)   => c.to_string(),
                            other => raise!(VmError::TypeMsg(format!("$* requires a string, got {}", other.type_label()))),
                        };
                        let n = match &self.value_stack[base + n_reg as usize] {
                            Value::Int(n) if *n >= 0 => *n as usize,
                            // A count that IS an Int and is negative is the
                            // other message, as in the tree-walker.
                            Value::Int(n) => raise!(VmError::IndexMsg(format!(
                                "$* repetition count must be non-negative, got {}", n))),
                            other => raise!(VmError::TypeMsg(format!(
                                "$* repetition count must be an integer, got {}", other.type_label()))),
                        };
                        s.repeat(n)
                    };
                    wreg!(dst, Value::String(ZyStr::new(result)));
                }

                // ── Comparison ──────────────────────────────────────────────
                &Instruction::CmpEq(dst, a, b) => { let r = cmp_direct(rreg!(a), rreg!(b)); wreg!(dst, Value::Bool(r == 0)); }
                &Instruction::CmpNe(dst, a, b) => { let r = cmp_direct(rreg!(a), rreg!(b)); wreg!(dst, Value::Bool(r != 0)); }
                &Instruction::CmpLt(dst, a, b) => { let r = ord_or_raise!(a, b, "Lt"); wreg!(dst, Value::Bool(ord_lt(r))); }
                &Instruction::CmpLe(dst, a, b) => { let r = ord_or_raise!(a, b, "Le"); wreg!(dst, Value::Bool(ord_le(r))); }
                &Instruction::CmpGt(dst, a, b) => { let r = ord_or_raise!(a, b, "Gt"); wreg!(dst, Value::Bool(ord_gt(r))); }
                &Instruction::CmpGe(dst, a, b) => { let r = ord_or_raise!(a, b, "Ge"); wreg!(dst, Value::Bool(ord_ge(r))); }

                // ── Logical ─────────────────────────────────────────────────
                // ZYVM-001: these read both operands through `is_truthy()`, so
                // `7 && 3` answered `#1` here and was refused by the tree-walker
                // — while the semantic analyser the two engines SHARE had
                // already warned in both. There is no truthiness in Zymbol
                // (the loop specifier settled it in v0.0.9), so a non-Bool
                // operand is refused, with the tree-walker's words.
                &Instruction::And(dst, a, b) => { let (va, vb) = rb2!(a, b, "AND"); wreg!(dst, Value::Bool(va && vb)); }
                &Instruction::Or (dst, a, b) => { let (va, vb) = rb2!(a, b, "OR");  wreg!(dst, Value::Bool(va || vb)); }
                // `!` takes a Bool and nothing else: `!7` is refused, not
                // coerced — there is no truthiness (GUIDE § Logical, GLB-019 A).
                &Instruction::Not(dst, src)  => {
                    let v = match rreg!(src) {
                        Value::Bool(b) => *b,
                        other => raise!(VmError::Generic(format!(
                            "logical NOT requires boolean operand, got {}", other.type_name()))),
                    };
                    wreg!(dst, Value::Bool(!v));
                }
                &Instruction::IsInt(dst, src) => { let v = matches!(rreg!(src), Value::Int(_)); wreg!(dst, Value::Bool(v)); }
                &Instruction::AsLoopCond(dst, src) => {
                    match rreg!(src) {
                        &Value::Bool(b) => wreg!(dst, Value::Bool(b)),
                        other => {
                            let got = other.type_word();
                            raise!(VmError::Generic(format!(
                                "loop expects a count or a condition, got {got}"
                            )))
                        }
                    }
                }

                // ── Control flow ────────────────────────────────────────────
                &Instruction::Jump(label)          => { ip = label as usize; }
                &Instruction::JumpIf(cond, label)    => { if  rreg!(cond).is_truthy() { ip = label as usize; } }
                &Instruction::JumpIfNot(cond, label) => { if !rreg!(cond).is_truthy() { ip = label as usize; } }

                // ── Functions ───────────────────────────────────────────────
                Instruction::Call(dst, func_idx, arg_regs) => {
                    let (dst, func_idx) = (*dst, *func_idx);
                    if func_idx as usize >= program.functions.len() {
                        raise!(VmError::UndefinedFunction(func_idx));
                    }
                    let num_regs = program.functions[func_idx as usize].num_registers as usize;
                    let _num_args = arg_regs.len();

                    // Save current IP to caller frame before pushing callee
                    self.frame_stack.last_mut().unwrap().ip = ip as u32;

                    let new_base = self.value_stack.len();
                    // Extend flat stack with Unit (single vectorizable loop), then
                    // overwrite the arg slots with the actual arg values via unsafe
                    // indexed write (no bounds check, no double capacity check).
                    self.value_stack.resize(new_base + num_regs, Value::Unit);

                    // Copy args from caller into callee arg registers
                    for (i, &reg) in arg_regs.iter().enumerate() {
                        let val = unsafe { self.value_stack.get_unchecked(base + reg as usize).clone() };
                        unsafe { *self.value_stack.get_unchecked_mut(new_base + i) = val; }
                    }

                    let wb = mem::take(&mut self.pending_output_writeback);
                    self.frame_stack.push(FrameInfo {
                        base: new_base as u32,
                        ip: 0,
                        chunk_idx: func_idx,
                        return_reg: dst,
                        catch_ip: u32::MAX,
                        error: None,
                        writeback: if wb.is_empty() { None } else { Some(Box::new(wb)) },
                    });

                    base = new_base;
                    ip = 0;
                    chunk_idx = func_idx as usize;
                }

                // TailCall: reuse current frame — no push/pop, no heap alloc
                Instruction::TailCall(func_idx, arg_regs) => {
                    let func_idx = *func_idx;
                    if func_idx as usize >= program.functions.len() {
                        raise!(VmError::UndefinedFunction(func_idx));
                    }
                    let num_regs = program.functions[func_idx as usize].num_registers as usize;

                    // Stage args into tco_buf (move to avoid aliasing)
                    let tco_buf = &mut self.tco_buf;
                    tco_buf.clear();
                    for &reg in arg_regs.iter() {
                        tco_buf.push(mem::replace(
                            unsafe { self.value_stack.get_unchecked_mut(base + reg as usize) },
                            Value::Unit,
                        ));
                    }

                    // Extend value_stack if callee needs more registers than we have
                    let current_size = self.value_stack.len() - base;
                    if num_regs > current_size {
                        self.value_stack.resize(base + num_regs, Value::Unit);
                    }

                    // Zero all registers [base..base+num_regs]
                    for v in &mut self.value_stack[base..base + num_regs] {
                        *v = Value::Unit;
                    }

                    // Write staged args into callee registers
                    for (i, v) in self.tco_buf.drain(..).enumerate() {
                        self.value_stack[base + i] = v;
                    }

                    // Update frame metadata (same base, new chunk). The compiler
                    // never emits a tail call inside a `!?`, so there is no
                    // handler here to keep.
                    let frame = self.frame_stack.last_mut().unwrap();
                    frame.chunk_idx = func_idx;
                    frame.catch_ip = u32::MAX;

                    ip = 0;
                    chunk_idx = func_idx as usize;
                }

                &Instruction::Return(src) => {
                    // Pop callee frame first to access writeback info
                    let frame = self.frame_stack.pop().unwrap();
                    let return_reg = frame.return_reg;

                    // Collect output writeback values BEFORE mem::replace so that if
                    // the return register overlaps with an output param register (e.g.
                    // `c<~` returned via `<~ c`), we capture the live value, not Unit.
                    let wb_pending = frame.writeback.map(|wb| {
                        wb.iter().map(|&(param_idx, caller_reg)| {
                            (caller_reg, unsafe { self.value_stack.get_unchecked(base + param_idx).clone() })
                        }).collect::<Vec<_>>()
                    });

                    // Take the return value (zeroes out callee register — OK since
                    // writeback was already collected above)
                    let result = mem::replace(
                        unsafe { self.value_stack.get_unchecked_mut(base + src as usize) },
                        Value::Unit,
                    );

                    if self.frame_stack.is_empty() {
                        // A `$!!` that carries an error out of the top level ends
                        // the program saying which error it was, as a runtime
                        // error — `run` locates it at this Return, compiled from
                        // the `$!!` statement's line (ZYJS-027, decided
                        // 2026-09-16). It used to end with status 1 and nothing.
                        if let Value::Error(e) = &result {
                            return Err(VmError::Generic(e.as_ref().to_string()));
                        }
                        // GAP-ZYB-006: a `<~` that reaches the top level ends
                        // the program, and its value is the exit status. The
                        // stop was already here — only the value was being
                        // dropped on the floor.
                        self.exit_code = Some(match &result {
                            Value::Int(n) => *n,
                            Value::Unit => 0,
                            // The analyzer rejects a non-integer before this
                            // runs; if one arrives anyway, "something went
                            // wrong" beats inventing a number.
                            _ => 1,
                        });
                        return Ok(Value::Unit);
                    }

                    // The frame a higher-order operator pushed has returned.
                    if self.frame_stack.len() == floor {
                        self.value_stack.truncate(base);
                        return Ok(result);
                    }

                    // Truncate value_stack: remove callee's registers
                    self.value_stack.truncate(base);

                    // Restore caller context — single last() access
                    let caller = self.frame_stack.last().unwrap();
                    base = caller.base as usize;
                    ip = caller.ip as usize;
                    chunk_idx = caller.chunk_idx as usize;

                    // Write return value into caller
                    unsafe { *self.value_stack.get_unchecked_mut(base + return_reg as usize) = result; }

                    // Apply output param writeback (rare path)
                    if let Some(wbs) = wb_pending {
                        for (caller_reg, val) in wbs {
                            unsafe { *self.value_stack.get_unchecked_mut(base + caller_reg as usize) = val; }
                        }
                    }
                }

                // ── I/O ─────────────────────────────────────────────────────
                &Instruction::Print(reg) => {
                    let mode = self.numeral_mode;
                    match &self.value_stack[base + reg as usize] {
                        Value::String(s)  => write!(self.output, "{}", s)?,
                        Value::Int(n)     => write!(self.output, "{}", numeral_int(*n, mode))?,
                        Value::Float(f)   => write!(self.output, "{}", numeral_float(*f, mode))?,
                        Value::Bool(b)    => write!(self.output, "{}", numeral_bool(*b, mode))?,
                        Value::Char(c)    => write!(self.output, "{}", c)?,
                        Value::Unit       => {},
                        // Arrays and tuples render their elements in the active
                        // script too (Display would hand back ASCII digits).
                        other             => write!(self.output, "{}", other.to_display_in(mode))?,
                    }
                }
                Instruction::PrintNewline => {
                    writeln!(self.output)?;
                }
                &Instruction::SetNumeralMode(base_cp) => {
                    self.numeral_mode = base_cp;
                }

                // ── Arrays ───────────────────────────────────────────────────
                &Instruction::NewArray(dst) => {
                    self.reg_set(dst, Value::Array(Rc::new(Vec::new())));
                }
                &Instruction::ArrayPush(arr_reg, val_reg) => {
                    let val = unsafe { self.value_stack.get_unchecked(base + val_reg as usize).clone() };
                    match unsafe { self.value_stack.get_unchecked_mut(base + arr_reg as usize) } {
                        Value::Array(rc_arr) => Rc::make_mut(rc_arr).push(val),
                        Value::Tuple(rc_tup) => Rc::make_mut(rc_tup).push(val),
                        Value::String(s) => {
                            // $+ on string: d $+ s → string concatenation
                            use std::fmt::Write as _;
                            let mut buf = s.clone().try_into_string();
                            match val {
                                Value::String(r) => buf.push_str(r.as_str()),
                                Value::Char(c) => buf.push(c),
                                other => { let _ = write!(buf, "{}", other); }
                            }
                            *s = ZyStr::new(buf);
                        }
                        other => raise!(VmError::TypeMsg(format!("cannot append to {} - only arrays, tuples, and strings support $+", other.type_label()))),
                    }
                }
                &Instruction::ArrayGet(dst, arr_reg, idx_reg) => {
                    // A dictionary is addressed by KEY, and the key may be
                    // computed (decision 7, DM-09). Checked before the index is
                    // read as an Int, which is what used to make `d[clave]` a
                    // type error here.
                    if let (Value::NamedTuple(fields), Value::String(key)) =
                        (&self.value_stack[base + arr_reg as usize], self.reg_get(idx_reg))
                    {
                        let key = key.as_str().to_string();
                        match fields.iter().find(|(k, _)| *k == key) {
                            Some((_, v)) => { let v = v.clone(); self.reg_set(dst, v); }
                            None => {
                                let available: Vec<String> =
                                    fields.iter().map(|(k, _)| k.clone()).collect();
                                raise!(VmError::Generic(missing_key_msg(&key, &available)));
                            }
                        }
                        continue;
                    }
                    // Decision 11: a dictionary is addressed by KEY, never by
                    // position. In a mutable dictionary a positional index is
                    // fragile — adding a key changes what sits at each position.
                    if let (Value::NamedTuple(fields), Value::Int(_)) =
                        (&self.value_stack[base + arr_reg as usize], self.reg_get(idx_reg))
                    {
                        let first = fields.first().map(|(k, _)| k.clone())
                            .unwrap_or_else(|| "clave".to_string());
                        raise!(VmError::TypeMsg(format!(
                            "a dictionary is addressed by key, not by position\nhelp: use d[\"{}\"] — adding a key changes what sits at each position",
                            first
                        )));
                    }
                    // The words of the tree-walker for each of the two ways
                    // this can fail (step 3.5b): a String reaches a dictionary
                    // key, and anything else that is not an Int is not an index.
                    let idx = match self.reg_get(idx_reg) {
                        Value::Int(n) => *n,
                        Value::String(_) => {
                            let label = self.value_stack[base + arr_reg as usize].type_label();
                            raise!(VmError::TypeMsg(format!(
                                "a String addresses a dictionary key, and this is {}", label)))
                        }
                        other => raise!(VmError::TypeMsg(format!(
                            "index must be an integer, got {}", other.type_label()))),
                    };
                    let val = match &self.value_stack[base + arr_reg as usize] {
                        Value::Array(arr) => {
                            let i = if idx == 0 { raise!(VmError::IndexZero);
                            } else if idx < 0 { arr.len() as i64 + idx } else { idx - 1 };
                            if i < 0 || i as usize >= arr.len() {
                                raise!(VmError::IndexOutOfBounds { index: idx, length: arr.len() , container: "array" });
                            }
                            arr[i as usize].clone()
                        }
                        Value::Tuple(items) => {
                            let i = if idx == 0 { raise!(VmError::IndexZero);
                            } else if idx < 0 { items.len() as i64 + idx } else { idx - 1 };
                            if i < 0 || i as usize >= items.len() {
                                raise!(VmError::IndexOutOfBounds { index: idx, length: items.len() , container: "tuple" });
                            }
                            items[i as usize].clone()
                        }
                        Value::NamedTuple(fields) => {
                            let i = if idx == 0 { raise!(VmError::IndexZero);
                            } else if idx < 0 { fields.len() as i64 + idx } else { idx - 1 };
                            if i < 0 || i as usize >= fields.len() {
                                raise!(VmError::IndexOutOfBounds { index: idx, length: fields.len() , container: "named tuple" });
                            }
                            fields[i as usize].1.clone()
                        }
                        Value::String(s) => {
                            // Single-pass: find the i-th char without collecting Vec<char>
                            let char_count = s.chars().count();
                            let i = if idx == 0 { raise!(VmError::IndexZero);
                            } else if idx < 0 { char_count as i64 + idx } else { idx - 1 };
                            if i < 0 || i as usize >= char_count {
                                raise!(VmError::IndexOutOfBounds { index: idx, length: char_count , container: "string" });
                            }
                            let ch = s.chars().nth(i as usize).unwrap();
                            Value::Char(ch)
                        }
                        other => raise!(VmError::TypeMsg(format!(
                            "cannot index into {} — expected array, tuple, or string", other.type_label()))),
                    };
                    self.reg_set(dst, val);
                }
                &Instruction::DeepSet(dst, path_reg, val_reg) => {
                    let val = self.reg_get(val_reg).clone();
                    let path = match self.reg_get(path_reg) {
                        Value::Array(p) => p.clone(),
                        other => raise!(VmError::TypeError { expected: "Array", got: other.type_name().to_string() }),
                    };
                    let root = mem::replace(&mut self.value_stack[base + dst as usize], Value::Unit);
                    match vm_deep_set(root, &path, val) {
                        Ok(updated) => self.value_stack[base + dst as usize] = updated,
                        Err(e) => raise!(e),
                    }
                }
                &Instruction::IterPairs(dst, src) => {
                    let val = match &self.value_stack[base + src as usize] {
                        Value::NamedTuple(nt) => Value::Array(Rc::new(
                            nt.iter()
                                .map(|(k, v)| Value::Tuple(Rc::new(vec![
                                    Value::String(ZyStr::new(k.clone())), v.clone(),
                                ])))
                                .collect::<Vec<_>>(),
                        )),
                        Value::String(s) => Value::Array(Rc::new(
                            s.chars().map(Value::Char).collect::<Vec<_>>(),
                        )),
                        Value::Array(arr) => Value::Array(arr.clone()),
                        Value::Tuple(t) => Value::Tuple(t.clone()),
                        other => raise!(VmError::TypeMsg(format!("can only iterate over ranges, arrays, strings, tuples and dictionaries, got {}", other.type_label()))),
                    };
                    self.reg_set(dst, val);
                }
                &Instruction::AssertMutable(reg, name_idx) => {
                    if let Value::Tuple(_) = self.reg_get(reg) {
                        let name = self.string_rcs[name_idx as usize].as_str();
                        raise!(VmError::Generic(tuple_immutable_msg(name)));
                    }
                }
                &Instruction::DeepSetInPlace(dst, path_reg, val_reg, name_idx) => {
                    let val = self.reg_get(val_reg).clone();
                    let path = match self.reg_get(path_reg) {
                        Value::Array(p) => p.clone(),
                        other => raise!(VmError::TypeError { expected: "Array", got: other.type_name().to_string() }),
                    };
                    let root = mem::replace(&mut self.value_stack[base + dst as usize], Value::Unit);
                    if let Value::Tuple(_) = &root {
                        self.value_stack[base + dst as usize] = root;
                        let name = self.string_rcs[name_idx as usize].as_str();
                        raise!(VmError::Generic(tuple_immutable_msg(name)));
                    }
                    match vm_deep_set(root, &path, val) {
                        Ok(updated) => self.value_stack[base + dst as usize] = updated,
                        Err(e) => raise!(e),
                    }
                }
                &Instruction::ArraySet(arr_reg, idx_reg, val_reg) => {
                    let val = self.reg_get(val_reg).clone();
                    let idx_val = self.reg_get(idx_reg).clone();
                    match &mut self.value_stack[base + arr_reg as usize] {
                        Value::Array(rc_arr) => {
                            let idx = match idx_val {
                                Value::Int(n) => n,
                                other => raise!(VmError::TypeMsg(format!("array update index must be an integer, got {}", other.type_label()))),
                            };
                            let arr = Rc::make_mut(rc_arr);
                            let i = if idx == 0 { raise!(VmError::IndexZero);
                            } else if idx < 0 { arr.len() as i64 + idx } else { idx - 1 };
                            if i < 0 || i as usize >= arr.len() {
                                raise!(VmError::IndexOutOfBounds { index: idx, length: arr.len() , container: "array" });
                            }
                            arr[i as usize] = val;
                        }
                        Value::NamedTuple(rc_fields) => {
                            let fields = Rc::make_mut(rc_fields);
                            match idx_val {
                                // A positional WRITE corrupts data rather than
                                // returning the wrong value: strictly worse than
                                // the positional read decision 11 withdrew.
                                Value::Int(_) => {
                                    let first = fields.first().map(|(k, _)| k.clone());
                                    raise!(VmError::TypeMsg(dict_not_positional(
                                        "d[n]$~ value", first.as_deref())));
                                }
                                #[allow(unreachable_patterns)]
                                Value::Int(idx) => {
                                    let i = if idx == 0 { raise!(VmError::IndexZero);
                                    } else if idx < 0 { fields.len() as i64 + idx } else { idx - 1 };
                                    if i < 0 || i as usize >= fields.len() {
                                        raise!(VmError::IndexOutOfBounds { index: idx, length: fields.len() , container: "named tuple" });
                                    }
                                    fields[i as usize].1 = val;
                                }
                                Value::String(name) => {
                                    if let Some(f) = fields.iter_mut().find(|(k, _)| k == name.as_str()) {
                                        f.1 = val;
                                    } else {
                                        // A key that is not there gets added.
                                        fields.push((name.as_str().to_string(), val));
                                    }
                                }
                                other => raise!(VmError::TypeMsg(format!("named tuple update index must be an integer or field name (string), got {}", other.type_label()))),
                            }
                        }
                        other => raise!(VmError::TypeError { expected: "Array", got: other.type_name().to_string() }),
                    }
                }
                &Instruction::ArrayLen(dst, src) => {
                    let n = match self.reg_get(src) {
                        Value::Array(arr) => arr.len() as i64,
                        Value::String(s) => if s.is_ascii() { s.len() as i64 } else { s.chars().count() as i64 },
                        Value::Tuple(items) => items.len() as i64,
                        Value::NamedTuple(fields) => fields.len() as i64,
                        other => raise!(VmError::TypeMsg(format!("cannot get length of {} - only arrays, tuples, and strings have length", other.type_label()))),
                    };
                    self.reg_set(dst, Value::Int(n));
                }
                &Instruction::ArrayRemove(arr_reg, idx_reg) => {
                    // In a dictionary the ADDRESS is the key, so `$-[…]` — which
                    // already means "remove by address" for the array — is the
                    // same operator with the same sense (decision 9). Checked
                    // before the index is read as an Int.
                    if let (Value::NamedTuple(fields), Value::String(key)) =
                        (&self.value_stack[base + arr_reg as usize], self.reg_get(idx_reg))
                    {
                        let key = key.as_str().to_string();
                        let mut out = fields.as_ref().clone();
                        match out.iter().position(|(k, _)| *k == key) {
                            Some(i) => { out.remove(i); }
                            None => {
                                let available: Vec<String> =
                                    fields.iter().map(|(k, _)| k.clone()).collect();
                                raise!(VmError::Generic(missing_key_msg(&key, &available)));
                            }
                        }
                        self.value_stack[base + arr_reg as usize] =
                            Value::NamedTuple(Rc::new(out));
                        continue;
                    }
                    let idx = match self.as_int(idx_reg) { Ok(v) => v, Err(e) => raise!(e) };
                    let result = match std::mem::replace(&mut self.value_stack[base + arr_reg as usize], Value::Unit) {
                        Value::Array(mut rc_arr) => {
                            let arr = Rc::make_mut(&mut rc_arr);
                            let i = if idx == 0 { raise!(VmError::IndexZero);
                            } else if idx < 0 { arr.len() as i64 + idx } else { idx - 1 };
                            if i < 0 || i as usize >= arr.len() {
                                raise!(VmError::IndexOutOfBounds { index: idx, length: arr.len() , container: "array" });
                            }
                            arr.remove(i as usize);
                            Value::Array(rc_arr)
                        }
                        Value::Tuple(rc_tup) => {
                            let mut tup = rc_tup.as_ref().clone();
                            let i = if idx == 0 { raise!(VmError::IndexZero);
                            } else if idx < 0 { tup.len() as i64 + idx } else { idx - 1 };
                            if i < 0 || i as usize >= tup.len() {
                                raise!(VmError::IndexOutOfBounds { index: idx, length: tup.len() , container: "tuple" });
                            }
                            tup.remove(i as usize);
                            Value::Tuple(Rc::new(tup))
                        }
                        Value::NamedTuple(rc_fields) => {
                            let first = rc_fields.first().map(|(k, _)| k.clone());
                            raise!(VmError::TypeMsg(dict_not_positional("d$-[n]", first.as_deref())));
                        }
                        Value::String(rc_s) => {
                            let mut chars: Vec<char> = rc_s.chars().collect();
                            let i = if idx == 0 { raise!(VmError::IndexZero);
                            } else if idx < 0 { chars.len() as i64 + idx } else { idx - 1 };
                            if i < 0 || i as usize >= chars.len() {
                                raise!(VmError::IndexOutOfBounds { index: idx, length: chars.len() , container: "string" });
                            }
                            chars.remove(i as usize);
                            Value::String(ZyStr::new(chars.iter().collect()))
                        }
                        other => raise!(VmError::TypeMsg(format!("cannot remove from {} - only arrays, tuples, and strings support $-[i]", other.type_label()))),
                    };
                    self.value_stack[base + arr_reg as usize] = result;
                }

                &Instruction::ArrayRemoveValue(arr_reg, val_reg) => {
                    let val = self.reg_get(val_reg).clone();
                    match self.value_stack[base + arr_reg as usize].clone() {
                        Value::Array(rc_arr) => {
                            let mut arr = rc_arr.as_ref().clone();
                            if let Some(pos) = arr.iter().position(|v| v.equals(&val)) {
                                arr.remove(pos);
                            }
                            self.value_stack[base + arr_reg as usize] = Value::Array(Rc::new(arr));
                        }
                        Value::String(rc_s) => {
                            let result = match &val {
                                Value::Char(c) => {
                                    let chars: Vec<char> = rc_s.chars().collect();
                                    if let Some(pos) = chars.iter().position(|ch| ch == c) {
                                        let mut out = chars; out.remove(pos); out.iter().collect()
                                    } else { rc_s.to_string() }
                                }
                                Value::String(p) => {
                                    let s = rc_s.as_str();
                                    let pc: Vec<char> = p.chars().collect();
                                    let sc: Vec<char> = s.chars().collect();
                                    if pc.is_empty() { s.to_string() } else {
                                        let mut found = false;
                                        let mut out = sc.clone();
                                        for i in 0..=sc.len().saturating_sub(pc.len()) {
                                            if sc[i..i+pc.len()] == pc[..] {
                                                out.drain(i..i+pc.len()); found = true; break;
                                            }
                                        }
                                        let _ = found; out.iter().collect()
                                    }
                                }
                                _ => raise!(VmError::TypeMsg(format!("$- on string requires char or string value, got {}", val.type_label()))),
                            };
                            self.value_stack[base + arr_reg as usize] = Value::String(ZyStr::new(result));
                        }
                        other => raise!(VmError::TypeMsg(format!("$- requires an array, tuple, or string, got {}", other.type_label()))),
                    }
                }

                &Instruction::ArrayRemoveAll(arr_reg, val_reg) => {
                    let val = self.reg_get(val_reg).clone();
                    match self.value_stack[base + arr_reg as usize].clone() {
                        Value::Array(rc_arr) => {
                            let arr: Vec<Value> = rc_arr.as_ref().iter().filter(|v| !v.equals(&val)).cloned().collect();
                            self.value_stack[base + arr_reg as usize] = Value::Array(Rc::new(arr));
                        }
                        Value::Tuple(rc_tup) => {
                            let tup: Vec<Value> = rc_tup.as_ref().iter().filter(|v| !v.equals(&val)).cloned().collect();
                            self.value_stack[base + arr_reg as usize] = Value::Tuple(Rc::new(tup));
                        }
                        Value::String(rc_s) => {
                            let result = match &val {
                                Value::Char(c) => rc_s.chars().filter(|ch| ch != c).collect(),
                                Value::String(p) => {
                                    if p.is_empty() { rc_s.to_string() }
                                    else { rc_s.replace(p.as_str(), "") }
                                }
                                _ => raise!(VmError::TypeMsg(format!("$-- on string requires char or string value, got {}", val.type_label()))),
                            };
                            self.value_stack[base + arr_reg as usize] = Value::String(ZyStr::new(result));
                        }
                        other => raise!(VmError::TypeMsg(format!("$-- requires an array, tuple, or string, got {}", other.type_label()))),
                    }
                }

                &Instruction::ArrayInsert(arr_reg, idx_reg, val_reg) => {
                    let idx = match self.as_int(idx_reg) { Ok(v) => v, Err(e) => raise!(e) };
                    let val = self.reg_get(val_reg).clone();
                    match self.value_stack[base + arr_reg as usize].clone() {
                        Value::Array(rc_arr) => {
                            let mut arr = rc_arr.as_ref().clone();
                            if idx <= 0 || (idx - 1) as usize > arr.len() {
                                raise!(VmError::IndexOutOfBounds { index: idx, length: arr.len() , container: "array" });
                            }
                            arr.insert((idx - 1) as usize, val);
                            self.value_stack[base + arr_reg as usize] = Value::Array(Rc::new(arr));
                        }
                        Value::Tuple(rc_tup) => {
                            let mut tup = rc_tup.as_ref().clone();
                            if idx <= 0 || (idx - 1) as usize > tup.len() {
                                raise!(VmError::IndexOutOfBounds { index: idx, length: tup.len() , container: "tuple" });
                            }
                            tup.insert((idx - 1) as usize, val);
                            self.value_stack[base + arr_reg as usize] = Value::Tuple(Rc::new(tup));
                        }
                        Value::String(rc_s) => {
                            let mut chars: Vec<char> = rc_s.chars().collect();
                            if idx <= 0 || (idx - 1) as usize > chars.len() {
                                raise!(VmError::IndexOutOfBounds { index: idx, length: chars.len() , container: "string" });
                            }
                            let i = (idx - 1) as usize;
                            match val {
                                Value::Char(c) => { chars.insert(i, c); }
                                Value::String(ref ins) => {
                                    for (j, c) in ins.chars().enumerate() { chars.insert(i + j, c); }
                                }
                                _ => raise!(VmError::TypeMsg(format!("$+[i] on string requires char or string element, got {}", val.type_label()))),
                            }
                            self.value_stack[base + arr_reg as usize] = Value::String(ZyStr::new(chars.iter().collect()));
                        }
                        other => raise!(VmError::TypeMsg(format!("$+[i] requires an array, tuple, or string, got {}", other.type_label()))),
                    }
                }

                &Instruction::ArrayRemoveRange(arr_reg, lo_reg) => {
                    // hi_reg = lo_reg + 1 by compiler convention
                    let lo_raw = match self.as_int(lo_reg) { Ok(v) => v, Err(e) => raise!(e) };
                    let hi_raw = match self.as_int(lo_reg + 1) { Ok(v) => v, Err(e) => raise!(e) };
                    // lo: 0=default start (1-based 1 = internal 0), positive=1-based (subtract 1), negative=not supported
                    let lo = (if lo_raw == 0 { 0i64 } else { lo_raw - 1 }).max(0) as usize;
                    // hi: positive=1-based inclusive (stays same as 0-based exclusive)
                    let hi = hi_raw.max(0) as usize;
                    match self.value_stack[base + arr_reg as usize].clone() {
                        Value::Array(rc_arr) => {
                            let mut arr = rc_arr.as_ref().clone();
                            if lo <= hi && hi <= arr.len() {
                                arr.drain(lo..hi);
                            }
                            self.value_stack[base + arr_reg as usize] = Value::Array(Rc::new(arr));
                        }
                        Value::Tuple(rc_tup) => {
                            let mut tup = rc_tup.as_ref().clone();
                            if lo <= hi && hi <= tup.len() {
                                tup.drain(lo..hi);
                            }
                            self.value_stack[base + arr_reg as usize] = Value::Tuple(Rc::new(tup));
                        }
                        Value::NamedTuple(rc_nt) => {
                            let fields = rc_nt.as_ref().clone();
                            let _ = (lo, hi);
                            let first = fields.first().map(|(k, _)| k.clone());
                            raise!(VmError::TypeMsg(dict_not_positional("d$-[a..b]", first.as_deref())));
                        }
                        Value::String(rc_s) => {
                            let mut chars: Vec<char> = rc_s.chars().collect();
                            if lo <= hi && hi <= chars.len() {
                                chars.drain(lo..hi);
                            }
                            self.value_stack[base + arr_reg as usize] = Value::String(ZyStr::new(chars.iter().collect()));
                        }
                        other => raise!(VmError::TypeMsg(format!("$-[..] requires an array, tuple, or string, got {}", other.type_label()))),
                    }
                }

                // ── Pattern match ────────────────────────────────────────────
                &Instruction::MatchInt(reg, val, label) => {
                    let v = match self.as_int(reg) { Ok(v) => v, Err(e) => raise!(e) };
                    if v == val { ip = label as usize; }
                }
                &Instruction::MatchRange(reg, lo, hi, label) => {
                    // A range pattern matches numbers; anything else is the
                    // tree-walker's `range pattern type mismatch` (step 3.5b).
                    let v = match self.reg_get(reg) {
                        Value::Int(n) => *n,
                        _ => raise!(VmError::TypeMsg("range pattern type mismatch".to_string())),
                    };
                    if v >= lo && v <= hi { ip = label as usize; }
                }
                &Instruction::MatchStr(reg, idx, label) => {
                    let s = &program.string_pool[idx as usize];
                    if let Value::String(v) = self.reg_get(reg) {
                        if v.as_ref() == s { ip = label as usize; }
                    }
                }
                &Instruction::MatchBool(reg, val, label) => {
                    if let Value::Bool(b) = self.reg_get(reg) {
                        if *b == val { ip = label as usize; }
                    }
                }

                // ── Function refs / closures ─────────────────────────────────
                &Instruction::MakeFunc(dst, func_idx) => {
                    let arity = program.functions.get(func_idx as usize)
                        .map(|c| c.num_params as u8).unwrap_or(0);
                    self.reg_set(dst, Value::Function(func_idx, arity));
                }
                &Instruction::MakeLambda(dst, func_idx) => {
                    let arity = program.functions.get(func_idx as usize)
                        .map(|c| c.num_params as u8).unwrap_or(0);
                    self.reg_set(dst, Value::Closure(func_idx, arity, Rc::new(vec![])));
                }
                Instruction::MakeClosure(dst, func_idx, captured_regs) => {
                    let (dst, func_idx) = (*dst, *func_idx);
                    let arity = program.functions.get(func_idx as usize)
                        .map(|c| c.num_params as u8).unwrap_or(0);
                    let upvalues: Vec<Value> = captured_regs.iter()
                        .map(|&r| self.reg_get(r).clone())
                        .collect();
                    self.reg_set(dst, Value::Closure(func_idx, arity, Rc::new(upvalues)));
                }

                // ── String ops ──────────────────────────────────────────────
                &Instruction::StrSplit(dst, str_reg, sep_reg) => {
                    // Borrow both at once — no clone of the full string
                    let parts: Vec<Value> = {
                        let s_val = &self.value_stack[base + str_reg as usize];
                        let sep_val = &self.value_stack[base + sep_reg as usize];
                        match (s_val, sep_val) {
                            (Value::String(s), Value::Char(c)) => {
                                let c = *c;
                                s.split(c).map(|p| Value::String(ZyStr::from_str_ref(p))).collect()
                            }
                            (Value::String(s), Value::String(sep)) => {
                                let sep = sep.clone();
                                s.split(sep.as_str()).map(|p| Value::String(ZyStr::from_str_ref(p))).collect()
                            }
                            (Value::String(_), other) => raise!(VmError::TypeMsg(format!("$/ delimiter must be a char or string, got {}", other.type_label()))),
                            (other, _) => raise!(VmError::TypeMsg(format!("$/ requires a string on the left, got {}", other.type_label()))),
                        }
                    };
                    self.reg_set(dst, Value::Array(Rc::new(parts)));
                }
                // ── Fused split instructions (via zymbol-intrinsics) ────────
                &Instruction::StrSplitCount(dst, str_reg, sep_reg) => {
                    let count = {
                        let s_v   = &self.value_stack[base + str_reg  as usize];
                        let sep_v = &self.value_stack[base + sep_reg as usize];
                        match (s_v, sep_v) {
                            (Value::String(s), Value::Char(c))   => intrinsics::split::count(s.as_str(), *c),
                            (Value::String(s), Value::String(sep)) => intrinsics::split::count_str(s.as_str(), sep.as_str()),
                            (Value::String(_), o) => raise!(VmError::TypeMsg(format!("$/ delimiter must be a char or string, got {}", o.type_label()))),
                            (o, _) => raise!(VmError::TypeMsg(format!("$/ requires a string on the left, got {}", o.type_label()))),
                        }
                    };
                    self.reg_set(dst, Value::Int(count));
                }
                &Instruction::StrSplitMap(dst, str_reg, sep_reg, func_reg) => {
                    let callable = self.reg_get(func_reg).clone();
                    let (s_owned, sep_owned) = {
                        let s_v   = &self.value_stack[base + str_reg as usize];
                        let sep_v = &self.value_stack[base + sep_reg as usize];
                        match (s_v, sep_v) {
                            (Value::String(s), Value::Char(_))   => (s.clone(), sep_v.clone()),
                            (Value::String(s), Value::String(_)) => (s.clone(), sep_v.clone()),
                            (Value::String(_), o) => raise!(VmError::TypeMsg(format!("$/ delimiter must be a char or string, got {}", o.type_label()))),
                            (o, _) => raise!(VmError::TypeMsg(format!("$/ requires a string on the left, got {}", o.type_label()))),
                        }
                    };
                    let parts: Vec<String> = match &sep_owned {
                        Value::Char(c) => s_owned.split(*c).map(str::to_string).collect(),
                        Value::String(sep_s) => s_owned.split(sep_s.as_str()).map(str::to_string).collect(),
                        _ => unreachable!(),
                    };
                    // The split first, then the function — the tree-walker's order.
                    if !matches!(callable, Value::Function(..) | Value::Closure(..)) {
                        raise!(VmError::TypeMsg("map requires lambda function".to_string()));
                    }
                    let mut results = Vec::with_capacity(parts.len());
                    let outcome: Result<(), VmError> = 'calls: {
                        for part in parts {
                            let v = Value::String(ZyStr::new(part));
                            match self.call_callable(callable.clone(), vec![v], program) {
                                Ok(r) => results.push(r),
                                Err(e) => break 'calls Err(e),
                            }
                        }
                        Ok(())
                    };
                    if let Err(e) = outcome { raise!(e); }
                    self.reg_set(dst, Value::Array(Rc::new(results)));
                }
                &Instruction::StrSplitFilter(dst, str_reg, sep_reg, func_reg) => {
                    let callable = self.reg_get(func_reg).clone();
                    let (s_owned, sep_owned) = {
                        let s_v   = &self.value_stack[base + str_reg as usize];
                        let sep_v = &self.value_stack[base + sep_reg as usize];
                        match (s_v, sep_v) {
                            (Value::String(s), Value::Char(_))   => (s.clone(), sep_v.clone()),
                            (Value::String(s), Value::String(_)) => (s.clone(), sep_v.clone()),
                            (Value::String(_), o) => raise!(VmError::TypeMsg(format!("$/ delimiter must be a char or string, got {}", o.type_label()))),
                            (o, _) => raise!(VmError::TypeMsg(format!("$/ requires a string on the left, got {}", o.type_label()))),
                        }
                    };
                    let parts: Vec<String> = match &sep_owned {
                        Value::Char(c) => s_owned.split(*c).map(str::to_string).collect(),
                        Value::String(sep_s) => s_owned.split(sep_s.as_str()).map(str::to_string).collect(),
                        _ => unreachable!(),
                    };
                    // The split first, then the function — the tree-walker's order.
                    if !matches!(callable, Value::Function(..) | Value::Closure(..)) {
                        raise!(VmError::TypeMsg("filter requires lambda function".to_string()));
                    }
                    let mut results = Vec::new();
                    let outcome: Result<(), VmError> = 'calls: {
                        for part in parts {
                            let v = Value::String(ZyStr::new(part));
                            match self.call_callable(callable.clone(), vec![v.clone()], program) {
                                Ok(keep) => if keep.is_truthy() { results.push(v); },
                                Err(e) => break 'calls Err(e),
                            }
                        }
                        Ok(())
                    };
                    if let Err(e) = outcome { raise!(e); }
                    self.reg_set(dst, Value::Array(Rc::new(results)));
                }
                &Instruction::StrSplitReduce(dst, str_reg, sep_reg, init_reg, func_reg) => {
                    let callable = self.reg_get(func_reg).clone();
                    let mut acc = self.reg_get(init_reg).clone();
                    let (s_owned, sep_owned) = {
                        let s_v   = &self.value_stack[base + str_reg as usize];
                        let sep_v = &self.value_stack[base + sep_reg as usize];
                        match (s_v, sep_v) {
                            (Value::String(s), Value::Char(_))   => (s.clone(), sep_v.clone()),
                            (Value::String(s), Value::String(_)) => (s.clone(), sep_v.clone()),
                            (Value::String(_), o) => raise!(VmError::TypeMsg(format!("$/ delimiter must be a char or string, got {}", o.type_label()))),
                            (o, _) => raise!(VmError::TypeMsg(format!("$/ requires a string on the left, got {}", o.type_label()))),
                        }
                    };
                    let parts: Vec<String> = match &sep_owned {
                        Value::Char(c) => s_owned.split(*c).map(str::to_string).collect(),
                        Value::String(sep_s) => s_owned.split(sep_s.as_str()).map(str::to_string).collect(),
                        _ => unreachable!(),
                    };
                    // The split first, then the function — the tree-walker's order.
                    if !matches!(callable, Value::Function(..) | Value::Closure(..)) {
                        raise!(VmError::TypeMsg("reduce requires lambda function".to_string()));
                    }
                    let outcome: Result<(), VmError> = 'calls: {
                        for part in parts {
                            let elem = Value::String(ZyStr::new(part));
                            match self.call_callable(callable.clone(), vec![mem::replace(&mut acc, Value::Unit), elem], program) {
                                Ok(r) => acc = r,
                                Err(e) => break 'calls Err(e),
                            }
                        }
                        Ok(())
                    };
                    if let Err(e) = outcome { raise!(e); }
                    self.reg_set(dst, acc);
                }
                &Instruction::StrContains(dst, str_reg, elem_reg) => {
                    let result = {
                        let s_val = &self.value_stack[base + str_reg as usize];
                        let e_val = &self.value_stack[base + elem_reg as usize];
                        match (s_val, e_val) {
                            (Value::String(s), Value::Char(c))    => s.contains(*c),
                            (Value::String(s), Value::String(sub)) => s.contains(sub.as_str()),
                            (Value::String(_), _) => false,
                            (other, _) => raise!(VmError::TypeMsg(format!("cannot search {} - only arrays, tuples, and strings support contains", other.type_label()))),
                        }
                    };
                    wreg!(dst, Value::Bool(result));
                }
                &Instruction::StrSlice(dst, str_reg, lo_reg) => {
                    let lo_val = match self.as_int(lo_reg) { Ok(v) => v, Err(e) => raise!(e) };
                    let hi_val = match self.as_int(lo_reg + 1) { Ok(v) => v, Err(e) => raise!(e) };
                    let result = match &self.value_stack[base + str_reg as usize] {
                        Value::String(s) => {
                            if s.is_ascii() {
                                // Fast path: byte indices == char indices
                                let len = s.len() as i64;
                                let lo = (if lo_val == 0 { 0i64 } else if lo_val < 0 { len + lo_val } else { lo_val - 1 }).max(0).min(len) as usize;
                                let hi = (if hi_val < 0 { len + hi_val + 1 } else { hi_val }).max(0).min(len) as usize;
                                let hi = hi.max(lo);
                                s[lo..hi].to_string()
                            } else {
                                // Unicode: single-pass via char_indices to find byte offsets
                                let char_len = s.chars().count() as i64;
                                let lo = (if lo_val == 0 { 0i64 } else if lo_val < 0 { char_len + lo_val } else { lo_val - 1 }).max(0).min(char_len) as usize;
                                let hi = (if hi_val < 0 { char_len + hi_val + 1 } else { hi_val }).max(0).min(char_len) as usize;
                                let hi = hi.max(lo);
                                let mut byte_lo = s.len();
                                let mut byte_hi = s.len();
                                for (ci, (bi, _)) in s.char_indices().enumerate() {
                                    if ci == lo { byte_lo = bi; }
                                    if ci == hi { byte_hi = bi; break; }
                                }
                                s[byte_lo..byte_hi].to_string()
                            }
                        }
                        other => raise!(VmError::TypeMsg(format!("cannot slice {} - only arrays, tuples, named tuples, and strings support slice", other.type_label()))),
                    };
                    self.reg_set(dst, Value::String(ZyStr::new(result)));
                }
                &Instruction::StrChars(dst, src) => {
                    // String → Array<Char> (O(N) once per loop start).
                    // Array/Tuple/NamedTuple → Rc clone, O(1) pass-through.
                    let val = match &self.value_stack[base + src as usize] {
                        Value::String(s) => {
                            let chars: Vec<Value> = s.chars().map(Value::Char).collect();
                            Value::Array(Rc::new(chars))
                        }
                        Value::Array(arr)       => Value::Array(arr.clone()),
                        Value::Tuple(t)         => Value::Tuple(t.clone()),
                        // A dictionary yields its KEYS, in insertion order —
                        // `for k in d` as Python spells it (decision 8). It used
                        // to pass the dictionary through unchanged, so the
                        // indexed read below handed back the VALUES, while the
                        // tree-walker refused to walk one at all: three engines,
                        // two answers, and neither was the decided one.
                        //
                        // With `d[k]` available the key is enough to reach the
                        // value, so no destructuring pattern has to enter `@`.
                        Value::NamedTuple(nt) => Value::Array(Rc::new(
                            nt.iter().map(|(k, _)| Value::String(ZyStr::new(k.clone()))).collect::<Vec<_>>(),
                        )),
                        other => raise!(VmError::TypeMsg(format!("can only iterate over ranges, arrays, strings, tuples and dictionaries, got {}", other.type_label()))),
                    };
                    wreg!(dst, val);
                }
                &Instruction::StrCharAt(dst, str_reg, idx_reg) => {
                    let ch = match (&self.value_stack[base + str_reg as usize],
                                    &self.value_stack[base + idx_reg as usize]) {
                        (Value::String(s), Value::Int(i)) => {
                            let i = *i as usize;
                            if s.is_ascii() {
                                // O(1) ASCII fast path
                                s.as_bytes().get(i).map(|&b| b as char)
                                    .unwrap_or('\0')
                            } else {
                                s.chars().nth(i).unwrap_or('\0')
                            }
                        }
                        (o, _) => raise!(VmError::TypeError {
                            expected: "String",
                            got: o.type_name().to_string(),
                        }),
                    };
                    wreg!(dst, Value::Char(ch));
                }
                // ── String modification operators ─────────────────────────────
                &Instruction::StrFindPos(dst, str_reg, pat_reg) => {
                    let positions: Vec<Value> = {
                        let s_val = &self.value_stack[base + str_reg as usize];
                        let p_val = &self.value_stack[base + pat_reg as usize];
                        match (s_val, p_val) {
                            (Value::String(s), Value::Char(c)) => {
                                let c = *c;
                                if s.is_ascii() && c.is_ascii() {
                                    // ASCII fast path: byte_offset == char_offset
                                    s.bytes().enumerate()
                                        .filter(|(_, b)| *b == c as u8)
                                        .map(|(i, _)| Value::Int((i + 1) as i64))
                                        .collect()
                                } else {
                                    s.char_indices()
                                        .filter(|(_, ch)| *ch == c)
                                        .enumerate()
                                        .map(|(ci, _)| Value::Int((ci + 1) as i64))
                                        .collect()
                                }
                            }
                            (Value::String(s), Value::String(pat)) => {
                                let pat = pat.clone();
                                if s.is_ascii() {
                                    // ASCII fast path: byte_offset == char_offset
                                    s.match_indices(pat.as_str())
                                        .map(|(bi, _)| Value::Int((bi + 1) as i64))
                                        .collect()
                                } else {
                                    // Unicode: build char-index map: byte_offset → char_index
                                    let char_idx: std::collections::HashMap<usize, usize> =
                                        s.char_indices().enumerate().map(|(ci, (bi, _))| (bi, ci)).collect();
                                    s.match_indices(pat.as_str())
                                        .filter_map(|(bi, _)| char_idx.get(&bi).map(|&ci| Value::Int((ci + 1) as i64)))
                                        .collect()
                                }
                            }
                            (Value::Array(arr), needle) => {
                                let needle = needle.clone();
                                arr.iter().enumerate()
                                    .filter(|(_, v)| v.equals(&needle))
                                    .map(|(i, _)| Value::Int((i + 1) as i64))
                                    .collect()
                            }
                            (Value::Tuple(tup), needle) => {
                                let needle = needle.clone();
                                tup.iter().enumerate()
                                    .filter(|(_, v)| v.equals(&needle))
                                    .map(|(i, _)| Value::Int((i + 1) as i64))
                                    .collect()
                            }
                            (Value::String(_), other) => raise!(VmError::TypeMsg(format!("$?? on string requires char or string value, got {}", other.type_label()))),
                            (other, _) => raise!(VmError::TypeMsg(format!("$?? requires an array, tuple, or string, got {}", other.type_label()))),
                        }
                    };
                    wreg!(dst, Value::Array(Rc::new(positions)));
                }
                &Instruction::StrInsert(dst, str_reg, pos_reg, text_reg) => {
                    let result = {
                        let s = match &self.value_stack[base + str_reg as usize] {
                            Value::String(s) => s.clone(),
                            other => raise!(VmError::TypeError { expected: "String", got: other.type_name().to_string() }),
                        };
                        let pos = ri!(pos_reg);
                        let text = match &self.value_stack[base + text_reg as usize] {
                            Value::String(t) => t.clone(),
                            other => raise!(VmError::TypeError { expected: "String", got: other.type_name().to_string() }),
                        };
                        // 1-based: pos=1 inserts at beginning; pos=N+1 appends at end
                        let pos_0based = if pos <= 0 { 0i64 } else { pos - 1 };
                        if s.is_ascii() {
                            let p = (pos_0based.max(0) as usize).min(s.len());
                            let mut r = String::with_capacity(s.len() + text.len());
                            r.push_str(&s[..p]);
                            r.push_str(&text);
                            r.push_str(&s[p..]);
                            r
                        } else {
                            let char_len = s.chars().count() as i64;
                            let p = (pos_0based.max(0).min(char_len)) as usize;
                            let byte_pos = s.char_indices().nth(p).map(|(bi, _)| bi).unwrap_or(s.len());
                            let mut r = String::with_capacity(s.len() + text.len());
                            r.push_str(&s[..byte_pos]);
                            r.push_str(&text);
                            r.push_str(&s[byte_pos..]);
                            r
                        }
                    };
                    wreg!(dst, Value::String(ZyStr::new(result)));
                }
                &Instruction::StrRemove(dst, str_reg, pos_reg, count_reg) => {
                    let result = {
                        let s = match &self.value_stack[base + str_reg as usize] {
                            Value::String(s) => s.clone(),
                            other => raise!(VmError::TypeError { expected: "String", got: other.type_name().to_string() }),
                        };
                        let pos_raw = ri!(pos_reg);
                        let pos = if pos_raw <= 0 { 0usize } else { (pos_raw - 1) as usize };
                        let count = ri!(count_reg).max(0) as usize;
                        if s.is_ascii() {
                            let lo = pos.min(s.len());
                            let hi = (lo + count).min(s.len());
                            let mut r = String::with_capacity(s.len() - (hi - lo));
                            r.push_str(&s[..lo]);
                            r.push_str(&s[hi..]);
                            r
                        } else {
                            let mut indices = s.char_indices();
                            let byte_lo = indices.nth(pos).map(|(bi, _)| bi).unwrap_or(s.len());
                            let byte_hi = if count == 0 {
                                byte_lo
                            } else {
                                // re-scan from byte_lo for `count` more chars
                                let mut h = byte_lo;
                                let mut remaining = count;
                                for (bi, _) in s[byte_lo..].char_indices() {
                                    if remaining == 0 { h = byte_lo + bi; break; }
                                    remaining -= 1;
                                }
                                if remaining > 0 { s.len() } else { h }
                            };
                            let mut r = String::with_capacity(s.len() - (byte_hi - byte_lo));
                            r.push_str(&s[..byte_lo]);
                            r.push_str(&s[byte_hi..]);
                            r
                        }
                    };
                    wreg!(dst, Value::String(ZyStr::new(result)));
                }
                &Instruction::StrReplace(dst, str_reg, pat_reg, rep_reg) => {
                    let result = {
                        let s = match &self.value_stack[base + str_reg as usize] {
                            Value::String(s) => s.clone(),
                            other => raise!(VmError::TypeMsg(format!("$~~ requires a string, got {}", other.type_label()))),
                        };
                        let rep = match &self.value_stack[base + rep_reg as usize] {
                            Value::String(r) => r.clone(),
                            other => raise!(VmError::TypeMsg(format!("$~~ replacement must be a string, got {}", other.type_label()))),
                        };
                        match &self.value_stack[base + pat_reg as usize] {
                            Value::String(pat) => s.replace(pat.as_str(), rep.as_str()),
                            Value::Char(c) => s.replace(*c, rep.as_str()),
                            other => raise!(VmError::TypeMsg(format!("$~~ pattern must be a string or char, got {}", other.type_label()))),
                        }
                    };
                    wreg!(dst, Value::String(ZyStr::new(result)));
                }
                &Instruction::StrReplaceN(dst, str_reg, pat_reg, rep_reg, n_reg) => {
                    let result = {
                        let s = match &self.value_stack[base + str_reg as usize] {
                            Value::String(s) => s.clone(),
                            other => raise!(VmError::TypeMsg(format!("$~~ requires a string, got {}", other.type_label()))),
                        };
                        let rep = match &self.value_stack[base + rep_reg as usize] {
                            Value::String(r) => r.clone(),
                            other => raise!(VmError::TypeMsg(format!("$~~ replacement must be a string, got {}", other.type_label()))),
                        };
                        let max = ri!(n_reg).max(0) as usize;
                        // Avoid heap-allocating a String for char patterns: use char directly.
                        #[derive(Copy, Clone)]
                        enum Pat<'a> { Ch(char), Str(&'a str) }
                        let pat = match &self.value_stack[base + pat_reg as usize] {
                            Value::String(p) => Pat::Str(p.as_str()),
                            Value::Char(c) => Pat::Ch(*c),
                            other => raise!(VmError::TypeMsg(format!("$~~ pattern must be a string or char, got {}", other.type_label()))),
                        };
                        if max == 0 {
                            match pat {
                                Pat::Str(p) => s.replace(p, rep.as_str()),
                                Pat::Ch(c)  => s.replace(c, rep.as_str()),
                            }
                        } else {
                            let mut out = String::with_capacity(s.len());
                            let mut remaining = s.as_str();
                            let mut count = 0;
                            while count < max {
                                let found = match pat {
                                    Pat::Str(p) => remaining.find(p).map(|pos| (pos, p.len())),
                                    Pat::Ch(c)  => remaining.find(c).map(|pos| (pos, c.len_utf8())),
                                };
                                if let Some((pos, pat_len)) = found {
                                    out.push_str(&remaining[..pos]);
                                    out.push_str(&rep);
                                    remaining = &remaining[pos + pat_len..];
                                    count += 1;
                                } else {
                                    break;
                                }
                            }
                            out.push_str(remaining);
                            out
                        }
                    };
                    wreg!(dst, Value::String(ZyStr::new(result)));
                }

                Instruction::BuildStr(dst, parts) => {
                    let dst = *dst;
                    let cap: usize = parts.iter().map(|p| match p {
                        BuildPart::Lit(idx) => program.string_pool[*idx as usize].len(),
                        BuildPart::Reg(_) => 4,
                    }).sum();
                    let mut result = String::with_capacity(cap);
                    for part in parts {
                        match part {
                            BuildPart::Lit(idx) => result.push_str(&program.string_pool[*idx as usize]),
                            BuildPart::Reg(r) => {
                                let part = self.numeral_repr(self.reg_get(*r));
                                result.push_str(&part);
                            }
                        }
                    }
                    self.reg_set(dst, Value::String(ZyStr::new(result)));
                }

                // ── Dynamic call (lambdas / closures stored in variables) ────
                Instruction::CallDynamic(dst, callee_reg, arg_regs) => {
                    let (dst, callee_reg) = (*dst, *callee_reg);
                    let callable = unsafe { self.value_stack.get_unchecked(base + callee_reg as usize).clone() };

                    let (func_idx, upvalues): (FuncIdx, Vec<Value>) = match callable {
                        Value::Function(idx, _) => (idx, Vec::new()),
                        Value::Closure(idx, _, uvs) => (idx, uvs.as_ref().clone()),
                        other => raise!(VmError::TypeError { expected: "Function", got: other.type_name().to_string() }),
                    };

                    if func_idx as usize >= program.functions.len() {
                        raise!(VmError::UndefinedFunction(func_idx));
                    }
                    let callee_chunk = &program.functions[func_idx as usize];
                    let num_params = callee_chunk.num_params as usize;
                    let num_regs = callee_chunk.num_registers as usize;

                    // Save caller IP
                    self.frame_stack.last_mut().unwrap().ip = ip as u32;

                    let new_base = self.value_stack.len();
                    self.value_stack.resize(new_base + num_regs, Value::Unit);

                    // Copy explicit args into [0..num_args)
                    for (i, &reg) in arg_regs.iter().enumerate() {
                        let val = unsafe { self.value_stack.get_unchecked(base + reg as usize).clone() };
                        unsafe { *self.value_stack.get_unchecked_mut(new_base + i) = val; }
                    }

                    // Load upvalues into [num_params..num_params+k)
                    for (i, uv) in upvalues.into_iter().enumerate() {
                        let slot = new_base + num_params + i;
                        if slot < new_base + num_regs {
                            self.value_stack[slot] = uv;
                        }
                    }

                    let wb = mem::take(&mut self.pending_output_writeback);
                    self.frame_stack.push(FrameInfo {
                        base: new_base as u32,
                        ip: 0,
                        chunk_idx: func_idx,
                        return_reg: dst,
                        catch_ip: u32::MAX,
                        error: None,
                        writeback: if wb.is_empty() { None } else { Some(Box::new(wb)) },
                    });

                    base = new_base;
                    ip = 0;
                    chunk_idx = func_idx as usize;
                }

                // ── Stdlib builtin call ───────────────────────────────────────
                Instruction::CallBuiltin(dst, builtin_id, arg_regs) => {
                    let dst = *dst;
                    let builtin_id = *builtin_id;
                    let args: Vec<Value> = arg_regs.iter()
                        .map(|&r| unsafe { self.value_stack.get_unchecked(base + r as usize).clone() })
                        .collect();
                    // `raise!`, not `?`. The `?` propagated straight out of the
                    // interpreter loop without looking for an armed `:!`, so no
                    // hard error from any `std/` function was catchable in this
                    // engine — `!? { m::ln(0.0) } :! ##_ { }` caught it in the
                    // tree-walker and aborted the program here.
                    let result = match crate::stdlib_builtins::call(builtin_id, args) {
                        Ok(v) => v,
                        Err(e) => raise!(VmError::Generic(e)),
                    };
                    wreg!(dst, result);
                }

                // ── Array higher-order ops ────────────────────────────────────
                &Instruction::ArrayContains(dst, arr_reg, elem_reg) => {
                    let elem = self.reg_get(elem_reg).clone();
                    let result = match self.reg_get(arr_reg) {
                        Value::Array(arr) => arr.as_ref().iter().any(|v| v.equals(&elem)),
                        Value::String(s) => match &elem {
                            Value::Char(c) => s.as_ref().contains(*c),
                            Value::String(sub) => s.as_ref().contains(sub.as_str()),
                            _ => false,
                        },
                        // On a DICTIONARY the question is about the KEY, which
                        // is what `in` asks in Python and in JS. Decision 10
                        // makes reading an absent key an error, so this is what
                        // lets a dictionary built piece by piece be consulted at
                        // all. A POSITIONAL tuple keeps the value question:
                        // there are no keys to ask about.
                        Value::NamedTuple(fields) => match &elem {
                            Value::String(key) => {
                                fields.iter().any(|(k, _)| k.as_str() == key.as_str())
                            }
                            other => raise!(VmError::TypeMsg(format!("a dictionary is asked about a key, so `$?` needs a String, got {}", other.type_label()))),
                        },
                        Value::Tuple(t) => t.as_ref().iter().any(|v| v.equals(&elem)),
                        other => raise!(VmError::TypeMsg(format!("cannot search {} - only arrays, tuples, and strings support contains", other.type_label()))),
                    };
                    self.reg_set(dst, Value::Bool(result));
                }
                &Instruction::ArraySlice(dst, arr_reg, lo_reg) => {
                    // hi_reg = lo_reg + 1 by compiler convention
                    let lo = match self.as_int(lo_reg) { Ok(v) => v, Err(e) => raise!(e) };
                    let hi = match self.as_int(lo_reg + 1) { Ok(v) => v, Err(e) => raise!(e) };
                    let result = match self.reg_get(arr_reg) {
                        Value::Array(arr) => {
                            let arr = arr.as_ref();
                            let len = arr.len() as i64;
                            // lo: 0=default start (internal 0), positive=1-based (subtract 1), negative=from end
                            let lo_norm = (if lo == 0 { 0i64 } else if lo < 0 { len + lo } else { lo - 1 }).max(0).min(len) as usize;
                            // hi: positive=1-based inclusive = 0-based exclusive (no change); negative=len+hi+1
                            let hi_norm = (if hi < 0 { len + hi + 1 } else { hi }).max(0).min(len) as usize;
                            let lo_norm = lo_norm.min(arr.len());
                            let hi_norm = hi_norm.min(arr.len()).max(lo_norm);
                            Value::Array(Rc::new(arr[lo_norm..hi_norm].to_vec()))
                        }
                        Value::Tuple(tup) => {
                            let tup = tup.as_ref();
                            let len = tup.len() as i64;
                            let lo_norm = (if lo == 0 { 0i64 } else if lo < 0 { len + lo } else { lo - 1 }).max(0).min(len) as usize;
                            let hi_norm = (if hi < 0 { len + hi + 1 } else { hi }).max(0).min(len) as usize;
                            let lo_norm = lo_norm.min(tup.len());
                            let hi_norm = hi_norm.min(tup.len()).max(lo_norm);
                            Value::Tuple(Rc::new(tup[lo_norm..hi_norm].to_vec()))
                        }
                        Value::NamedTuple(fields) => {
                            let fields = fields.as_ref();
                            let len = fields.len() as i64;
                            let lo_norm = (if lo == 0 { 0i64 } else if lo < 0 { len + lo } else { lo - 1 }).max(0).min(len) as usize;
                            let hi_norm = (if hi < 0 { len + hi + 1 } else { hi }).max(0).min(len) as usize;
                            let _ = (lo_norm, hi_norm);
                            // No key-based replacement, and it does not get one:
                            // "the first two keys" is not a question a dictionary
                            // should answer — Python's `dict` has no slicing.
                            let first = fields.first().map(|(k, _)| k.clone());
                            raise!(VmError::TypeMsg(dict_not_positional("d$[a..b]", first.as_deref())));
                        }
                        // Strings slice too. The tree-walker has always allowed
                        // `s$[3..]`; the VM only reached this instruction when
                        // the subject was a runtime value rather than a literal
                        // the compiler could fold, which is why the gap showed
                        // up inside module functions and nowhere else.
                        Value::String(rc_s) => {
                            let chars: Vec<char> = rc_s.chars().collect();
                            let len = chars.len() as i64;
                            let lo_norm = (if lo == 0 { 0i64 } else if lo < 0 { len + lo } else { lo - 1 }).max(0).min(len) as usize;
                            let hi_norm = (if hi < 0 { len + hi + 1 } else { hi }).max(0).min(len) as usize;
                            let hi_norm = hi_norm.max(lo_norm);
                            Value::String(ZyStr::new(chars[lo_norm..hi_norm].iter().collect()))
                        }
                        other => raise!(VmError::TypeMsg(format!("cannot slice {} - only arrays, tuples, named tuples, and strings support slice", other.type_label()))),
                    };
                    self.reg_set(dst, result);
                }
                &Instruction::ArrayMap(dst, arr_reg, func_reg) => {
                    let callable = self.reg_get(func_reg).clone();
                    // The function before the array, as the tree-walker checks
                    // them — so an empty array with no function is refused too;
                    // it used to answer `[]` (step 3.5b).
                    if !matches!(callable, Value::Function(..) | Value::Closure(..)) {
                        raise!(VmError::TypeMsg("map requires lambda function".to_string()));
                    }
                    let arr = match self.reg_get(arr_reg).clone() {
                        Value::Array(a) => a.as_ref().clone(),
                        other => raise!(VmError::TypeMsg(format!("map requires array, got {}", other.type_label()))),
                    };
                    let mut results = Vec::with_capacity(arr.len());
                    let outcome: Result<(), VmError> = 'calls: {
                        for elem in arr {
                            match self.call_callable(callable.clone(), vec![elem], program) {
                                Ok(r) => results.push(r),
                                Err(e) => break 'calls Err(e),
                            }
                        }
                        Ok(())
                    };
                    if let Err(e) = outcome { raise!(e); }
                    self.reg_set(dst, Value::Array(Rc::new(results)));
                }
                &Instruction::ArrayFilter(dst, arr_reg, func_reg) => {
                    let callable = self.reg_get(func_reg).clone();
                    // The function before the array, as the tree-walker checks
                    // them — so an empty array with no function is refused too;
                    // it used to answer `[]` (step 3.5b).
                    if !matches!(callable, Value::Function(..) | Value::Closure(..)) {
                        raise!(VmError::TypeMsg("filter requires lambda function".to_string()));
                    }
                    let arr = match self.reg_get(arr_reg).clone() {
                        Value::Array(a) => a.as_ref().clone(),
                        other => raise!(VmError::TypeMsg(format!("filter requires array, got {}", other.type_label()))),
                    };
                    let mut results = Vec::new();
                    let outcome: Result<(), VmError> = 'calls: {
                        for elem in arr {
                            match self.call_callable(callable.clone(), vec![elem.clone()], program) {
                                Ok(keep) => if keep.is_truthy() { results.push(elem); },
                                Err(e) => break 'calls Err(e),
                            }
                        }
                        Ok(())
                    };
                    if let Err(e) = outcome { raise!(e); }
                    self.reg_set(dst, Value::Array(Rc::new(results)));
                }
                &Instruction::ArrayReduce(dst, arr_reg, init_reg, func_reg) => {
                    let callable = self.reg_get(func_reg).clone();
                    if !matches!(callable, Value::Function(..) | Value::Closure(..)) {
                        raise!(VmError::TypeMsg("reduce requires lambda function".to_string()));
                    }
                    // Before the array, as the tree-walker checks it: a one-
                    // parameter lambda is a mistake, not a fold, and calling it
                    // anyway answered the initial value (GLB-015 A).
                    if let Value::Function(_, n) | Value::Closure(_, n, _) = &callable {
                        if *n != 2 {
                            raise!(VmError::Generic(format!(
                                "reduce lambda requires 2 parameters (accumulator, element), got {n}")));
                        }
                    }
                    let arr = match self.reg_get(arr_reg).clone() {
                        Value::Array(a) => a.as_ref().clone(),
                        other => raise!(VmError::TypeMsg(format!("reduce requires array, got {}", other.type_label()))),
                    };
                    let mut acc = self.reg_get(init_reg).clone();
                    let outcome: Result<(), VmError> = 'calls: {
                        for elem in arr {
                            match self.call_callable(callable.clone(), vec![mem::replace(&mut acc, Value::Unit), elem], program) {
                                Ok(r) => acc = r,
                                Err(e) => break 'calls Err(e),
                            }
                        }
                        Ok(())
                    };
                    if let Err(e) = outcome { raise!(e); }
                    self.reg_set(dst, acc);
                }
                &Instruction::ArraySort(dst, arr_reg, ascending, func_reg) => {
                    let arr = match self.reg_get(arr_reg).clone() {
                        Value::Array(a) => a.as_ref().clone(),
                        other => raise!(VmError::TypeMsg(format!("sort requires an array, got {}", other.type_label()))),
                    };
                    let mut items = arr;
                    if func_reg == u16::MAX {
                        // Natural order
                        items.sort_by(vm_natural_cmp);
                        if !ascending {
                            items.reverse();
                        }
                    } else {
                        // Custom comparator: bubble sort to avoid unsafe borrow
                        let callable = self.reg_get(func_reg).clone();
                        let n = items.len();
                        let outcome: Result<(), VmError> = 'calls: {
                            for i in 0..n {
                                for j in 0..n.saturating_sub(i + 1) {
                                    let keep = match self.call_callable(
                                        callable.clone(),
                                        vec![items[j].clone(), items[j + 1].clone()],
                                        program,
                                    ) {
                                        // A comparator answers a Bool, and no
                                        // truthiness stands in for one (GLB-024).
                                        Ok(Value::Bool(keep)) => keep,
                                        Ok(other) => break 'calls Err(VmError::Generic(format!(
                                            "sort comparator must return a Bool, got {}", other.type_name()))),
                                        Err(e) => break 'calls Err(e),
                                    };
                                    if !keep {
                                        items.swap(j, j + 1);
                                    }
                                }
                            }
                            Ok(())
                        };
                        if let Err(e) = outcome { raise!(e); }
                    }
                    self.reg_set(dst, Value::Array(Rc::new(items)));
                }

                // ── Destructuring ────────────────────────────────────────────
                &Instruction::DestructureCheck(src, wants_tuple) => {
                    let v = self.reg_get(src);
                    let ok = if wants_tuple {
                        matches!(v, Value::Tuple(_))
                    } else {
                        matches!(v, Value::Array(_))
                    };
                    if !ok {
                        let got = v.type_label();
                        raise!(VmError::Generic(if wants_tuple {
                            format!("tuple pattern '( … )' requires a tuple, got {got}")
                        } else {
                            format!("array pattern '[ … ]' requires an array, got {got}")
                        }));
                    }
                }
                &Instruction::CallableCheck(reg, pipe) => {
                    if !matches!(self.reg_get(reg), Value::Function(..) | Value::Closure(..)) {
                        raise!(VmError::TypeMsg(if pipe {
                            "pipe operator requires a callable function or lambda"
                        } else {
                            "expression is not callable"
                        }.to_string()));
                    }
                }
                &Instruction::LoopStepCheck(step) => {
                    let n = match self.reg_get(step) {
                        Value::Int(n) => *n,
                        other => raise!(VmError::TypeMsg(format!("step must be an integer, got {}", other.type_label()))),
                    };
                    if n <= 0 {
                        raise!(VmError::Generic(format!("step must be positive, got {n}")));
                    }
                }
                &Instruction::LoopBoundsCheck(start, end) => {
                    // No `for` here: raise! continues the dispatch loop, and
                    // inside an inner loop it would continue that one instead.
                    let bad = match (self.reg_get(start), self.reg_get(end)) {
                        (Value::Int(_), Value::Int(_)) => false,
                        _ => true,
                    };
                    if bad {
                        // Both bounds named, as the tree-walker names them.
                        raise!(VmError::TypeMsg(format!(
                            "range bounds must be integers, got {} and {}",
                            self.reg_get(start).type_label(), self.reg_get(end).type_label()
                        )));
                    }
                }
                &Instruction::DestroyLocal(reg, name_idx) => {
                    let _ = name_idx;
                    let abs = self.frame_stack.last().unwrap().base as usize + reg as usize;
                    self.destroyed_slots.insert(abs);
                    self.reg_set(reg, Value::Unit);
                }
                &Instruction::CheckAlive(reg, name_idx) => {
                    let dead = !self.destroyed_slots.is_empty()
                        && self.destroyed_slots.contains(&(self.frame_stack.last().unwrap().base as usize + reg as usize));
                    if dead {
                        let name = program.string_pool[name_idx as usize].clone();
                        raise!(VmError::Generic(format!(
                            "use after destruction: variable '{}' was destroyed after its last use", name)));
                    }
                }
                &Instruction::Revive(reg) => {
                    if !self.destroyed_slots.is_empty() {
                        let abs = self.frame_stack.last().unwrap().base as usize + reg as usize;
                        self.destroyed_slots.remove(&abs);
                    }
                }
                &Instruction::Pos(dst, src) => {
                    let v = match rreg!(src) {
                        v @ (Value::Int(_) | Value::Float(_)) => v.clone(),
                        other => raise!(VmError::Generic(format!(
                            "unary plus requires numeric operand, got {}", other.type_name()))),
                    };
                    wreg!(dst, v);
                }
                &Instruction::OutputSlotCheck(slot, admits_tuple) => {
                    let bad = match self.reg_get(slot) {
                        Value::Int(_) => None,
                        Value::Tuple(_) if admits_tuple => None,
                        other => Some(other.to_string_repr()),
                    };
                    if let Some(got) = bad {
                        raise!(VmError::Generic(format!(">>~ slot expects Int, got {got}")));
                    }
                }
                &Instruction::DestructureRest(dst, src, from, trailing) => {
                    let (len, is_tuple) = match self.reg_get(src) {
                        Value::Array(a) => (a.len(), false),
                        Value::Tuple(t) => (t.len(), true),
                        _ => (0, false),
                    };
                    let lo = (from as usize - 1).min(len);
                    // The trailing names get their share only if the elements
                    // reach that far — the tree-walker's rule, exactly.
                    let end = if trailing > 0 && len > lo + trailing as usize {
                        len - trailing as usize
                    } else {
                        len
                    };
                    let slice: Vec<Value> = match self.reg_get(src) {
                        Value::Array(a) => a.as_ref().get(lo..end).unwrap_or(&[]).to_vec(),
                        Value::Tuple(t) => t.as_ref().get(lo..end).unwrap_or(&[]).to_vec(),
                        _ => Vec::new(),
                    };
                    let v = if is_tuple { Value::Tuple(Rc::new(slice)) } else { Value::Array(Rc::new(slice)) };
                    self.reg_set(dst, v);
                }
                &Instruction::DestructureTail(dst, src, k, from, trailing) => {
                    let len = match self.reg_get(src) {
                        Value::Array(a) => a.len(),
                        Value::Tuple(t) => t.len(),
                        _ => 0,
                    };
                    let lo = (from as usize - 1).min(len);
                    let v = if trailing > 0 && len > lo + trailing as usize {
                        let i = len - k as usize;
                        match self.reg_get(src) {
                            Value::Array(a) => a.as_ref().get(i).cloned().unwrap_or(Value::Unit),
                            Value::Tuple(t) => t.as_ref().get(i).cloned().unwrap_or(Value::Unit),
                            _ => Value::Unit,
                        }
                    } else {
                        Value::Unit
                    };
                    self.reg_set(dst, v);
                }
                &Instruction::DestructureAbsorb(dst, src, from) => {
                    let value = match self.reg_get(src) {
                        Value::Array(arr) => {
                            let rest = &arr.as_ref()[(from as usize - 1).min(arr.len())..];
                            match rest.len() {
                                0 => Value::Unit,
                                1 => rest[0].clone(),
                                _ => Value::Array(Rc::new(rest.to_vec())),
                            }
                        }
                        Value::Tuple(tup) => {
                            let rest = &tup.as_ref()[(from as usize - 1).min(tup.len())..];
                            match rest.len() {
                                0 => Value::Unit,
                                1 => rest[0].clone(),
                                _ => Value::Tuple(Rc::new(rest.to_vec())),
                            }
                        }
                        _ => Value::Unit,
                    };
                    self.reg_set(dst, value);
                }

                // ── Tuples ───────────────────────────────────────────────────
                Instruction::MakeTuple(dst, regs) => {
                    let dst = *dst;
                    let items: Vec<Value> = regs.iter().map(|&r| self.reg_get(r).clone()).collect();
                    self.reg_set(dst, Value::Tuple(Rc::new(items)));
                }

                // ── Named tuples ─────────────────────────────────────────────
                Instruction::MakeNamedTuple(dst, names, regs) => {
                    let dst = *dst;
                    let mut fields = Vec::with_capacity(names.len());
                    for (name_idx, &reg) in names.iter().zip(regs.iter()) {
                        let name = program.string_pool[*name_idx as usize].clone();
                        let val = self.reg_get(reg).clone();
                        fields.push((name, val));
                    }
                    self.reg_set(dst, Value::NamedTuple(Rc::new(fields)));
                }
                &Instruction::RequireBool(src, is_and) => {
                    let v = self.reg_get(src);
                    if !matches!(v, Value::Bool(_)) {
                        let msg = logical_type_error(if is_and { "AND" } else { "OR" }, v);
                        raise!(VmError::Generic(msg));
                    }
                }
                &Instruction::RequireDict(src) => {
                    let v = self.reg_get(src);
                    if !matches!(v, Value::NamedTuple(_)) {
                        let got = v.type_label();
                        raise!(VmError::Generic(format!(
                            "the pattern #(…) requires a dictionary, got {}\nhelp: #(key: name) = d unpacks a dictionary; use (a, b) for a tuple, [a, b] for an array",
                            got
                        )));
                    }
                }
                &Instruction::NamedTupleGet(dst, tuple_reg, field_idx) => {
                    let field_name = &program.string_pool[field_idx as usize];
                    let result = match self.reg_get(tuple_reg) {
                        Value::NamedTuple(fields) => {
                            let field_name = field_name.clone();
                            match fields.iter().find(|(n, _)| *n == field_name).map(|(_, v)| v.clone()) {
                                Some(v) => v,
                                None => {
                                    let available: Vec<String> = fields.iter().map(|(n, _)| n.clone()).collect();
                                    raise!(VmError::Generic(missing_key_msg(&field_name, &available)));
                                }
                            }
                        }
                        // A numeric field name is unreachable from source — the
                        // parser refuses `a.1` in every engine — so an array
                        // reaching here is the dot on the wrong collection, and
                        // it gets the same message the tree-walker gives.
                        Value::Array(arr) => {
                            if let Ok(i) = field_name.parse::<usize>() {
                                arr.get(i).cloned().unwrap_or(Value::Unit)
                            } else {
                                let field_name = field_name.clone();
                                let got = Value::Array(arr.clone()).type_label();
                                raise!(VmError::Generic(format!(
                                    "the dot reaches a dictionary key, and this is {}\nhelp: use d.{} on a #(…) — for a position, use x[1]",
                                    got, field_name
                                )));
                            }
                        }
                        Value::Tuple(_) => {
                            let field_name = field_name.clone();
                            raise!(VmError::Generic(format!(
                                "a positional tuple is addressed by position, not by name: '{}'\nhelp: use t[1] — names live in a dictionary, #(key: value)",
                                field_name
                            )));
                        }
                        other => {
                            let got = other.type_label();
                            let field_name = field_name.clone();
                            raise!(VmError::Generic(format!(
                                "the dot reaches a dictionary key, and this is {}\nhelp: use d.{} on a #(…) — for a position, use x[1]",
                                got, field_name
                            )));
                        }
                    };
                    self.reg_set(dst, result);
                }

                // ── Data ops ─────────────────────────────────────────────────
                &Instruction::NumericEval(dst, src) => {
                    let result = match self.reg_get(src) {
                        Value::String(s) => {
                            let s_rc = s.clone();
                            let trimmed = s_rc.as_ref().trim();
                            if let num::Num::Int(i) = num::parse(trimmed) {
                                Value::Int(i)
                            } else if let num::Num::Float(f) = num::parse(trimmed) {
                                Value::Float(f)
                            } else if let Some(normalized) = normalize_unicode_digits(trimmed) {
                                if let num::Num::Int(i) = num::parse(&normalized) {
                                    Value::Int(i)
                                } else if let num::Num::Float(f) = num::parse(&normalized) {
                                    Value::Float(f)
                                } else {
                                    Value::String(s_rc)
                                }
                            } else {
                                Value::String(s_rc)
                            }
                        }
                        Value::Int(n) => Value::Int(*n),
                        Value::Float(f) => Value::Float(*f),
                        // GAP-ZYB-012: a Char reads like the one-character
                        // string it is — `#|'७'|` is 7, as `#|"७"|` already
                        // was. A Char that is not a digit comes back untouched.
                        Value::Char(c) => vm_char_as_number(*c),
                        other => other.clone(),
                    };
                    self.reg_set(dst, result);
                }
                &Instruction::IsArray(dst, src) => {
                    let is_arr = matches!(self.reg_get(src), Value::Array(_));
                    self.reg_set(dst, Value::Bool(is_arr));
                }
                &Instruction::TypeOf(dst, src) => {
                    let val = self.reg_get(src).clone();
                    let tuple_val = if matches!(&val, Value::Error(_)) {
                        Value::Tuple(Rc::new(vec![
                            Value::String(ZyStr::new(val.tw_type_name_owned())),
                            Value::Int(val.error_message_len()),
                            val.clone(),
                        ]))
                    } else {
                        let (type_sym, len) = val.type_metadata();
                        Value::Tuple(Rc::new(vec![
                            Value::String(ZyStr::new(type_sym.to_string())),
                            Value::Int(len),
                            val,
                        ]))
                    };
                    self.reg_set(dst, tuple_val);
                }

                &Instruction::BaseConvert(dst, src, radix) => {
                    let val = self.reg_get(src).clone();
                    let result = match val {
                        // Char → string representation (display code in given base)
                        Value::Char(ch) => {
                            let code = ch as u32;
                            let s = match radix {
                                2  => format!("0b{:b}", code),
                                8  => format!("0o{:o}", code),
                                10 => format!("0d{:04}", code),
                                _  => format!("0x{:04X}", code),
                            };
                            Value::String(ZyStr::new(s))
                        }
                        // Int → String (format integer in specified base)
                        Value::Int(n) => {
                            let s = match radix {
                                2  => format!("0b{:b}", n),
                                8  => format!("0o{:o}", n),
                                10 => format!("0d{:04}", n),
                                _  => format!("0x{:04X}", n),
                            };
                            Value::String(ZyStr::new(s))
                        }
                        // String → Char (parse in given base, then create char)
                        Value::String(s) => {
                            let stripped = s.as_ref()
                                .trim_start_matches("0b").trim_start_matches("0B")
                                .trim_start_matches("0o").trim_start_matches("0O")
                                .trim_start_matches("0d").trim_start_matches("0D")
                                .trim_start_matches("0x").trim_start_matches("0X");
                            let code_res = match radix {
                                2  => u32::from_str_radix(stripped, 2),
                                8  => u32::from_str_radix(stripped, 8),
                                10 => stripped.parse::<u32>(),
                                _  => u32::from_str_radix(stripped, 16),
                            };
                            match code_res {
                                Ok(code) if code <= 0x10FFFF => {
                                    match char::from_u32(code) {
                                        Some(ch) => Value::Char(ch),
                                        None => raise!(VmError::Generic(format!(
                                            "invalid Unicode character code: {}", code
                                        ))),
                                    }
                                }
                                Ok(code) => raise!(VmError::Generic(format!(
                                    "character code must be in range 0..0x10FFFF, got {}", code
                                ))),
                                Err(_) => raise!(VmError::Generic(format!(
                                    "failed to parse '{}' as base-{} number", stripped, radix
                                ))),
                            }
                        }
                        other => raise!(VmError::TypeMsg(format!("base conversion expressions work with char, int, or string, got {}", other.type_label()))),
                    };
                    self.reg_set(dst, result);
                }

                // ── Try/catch (GLB-010) ───────────────────────────────────────
                // The handler this `!?` replaces is saved in its own register,
                // packed as `pending_len << 32 | catch_ip`: re-armed on the way
                // out, and the pending errors truncated back to what they were,
                // which discards any that belonged to a `!?` inside this one.
                &Instruction::TryBegin(catch_label, save) => {
                    let packed = {
                        let frame = self.frame_stack.last_mut().unwrap();
                        let pending = frame.error.as_ref().map_or(0, |e| e.pending.len());
                        let outer = std::mem::replace(&mut frame.catch_ip, catch_label);
                        ((pending as i64) << 32) | outer as i64
                    };
                    wreg!(save, Value::Int(packed));
                }
                &Instruction::TryEnd(save) => {
                    let outer = match rreg!(save) { Value::Int(n) => (*n & 0xFFFF_FFFF) as u32, _ => u32::MAX };
                    self.frame_stack.last_mut().unwrap().catch_ip = outer;
                }
                &Instruction::TryLand(save, err_reg) => {
                    let (outer, keep) = match rreg!(save) {
                        Value::Int(n) => ((*n & 0xFFFF_FFFF) as u32, (*n >> 32) as usize),
                        _ => (u32::MAX, 0),
                    };
                    let mut pending = self.in_flight.take().unwrap_or_else(|| Pending {
                        tag: 0,
                        err: VmError::Generic("unknown error".to_string()),
                        kind: "_",
                        origin_ip: self.cur_ip,
                        origin_chunk: self.cur_chunk,
                    });
                    pending.tag = save;
                    let value = Value::Error(ZyStr::new(format!("##{}({})", pending.kind, pending.err)));
                    {
                        let frame = self.frame_stack.last_mut().unwrap();
                        frame.catch_ip = outer;
                        let errors = frame.error.get_or_insert_with(Default::default);
                        errors.pending.truncate(keep);
                        errors.pending.push(pending);
                    }
                    wreg!(err_reg, value);
                }
                &Instruction::TryHandled(tag) => {
                    if let Some(errors) = self.frame_stack.last_mut().unwrap().error.as_mut() {
                        if errors.pending.last().is_some_and(|p| p.tag == tag) {
                            errors.pending.pop();
                        }
                    }
                }
                &Instruction::TryRethrow(tag) => {
                    let pending = match self.frame_stack.last_mut().unwrap().error.as_mut() {
                        Some(errors) if errors.pending.last().is_some_and(|p| p.tag == tag) => errors.pending.pop(),
                        _ => None,
                    };
                    if let Some(p) = pending {
                        // From where it was first raised: an uncaught rethrow is
                        // located at that instruction, not at the end of the `!?`.
                        self.cur_ip = p.origin_ip;
                        self.cur_chunk = p.origin_chunk;
                        raise_kind!(p.err, p.kind);
                    }
                }

                // ── Shell execution ───────────────────────────────────────────
                Instruction::BashExec(dst, parts) => {
                    let dst = *dst;
                    let passes_function = parts.iter().any(|part| {
                        matches!(part, BuildPart::Reg(r) if holds_function(self.reg_get(*r)))
                    });
                    if passes_function {
                        raise!(VmError::Generic(
                            "cannot use function in bash command interpolation".to_string()
                        ));
                    }
                    let mut cmd = String::new();
                    for part in parts {
                        match part {
                            BuildPart::Lit(idx) => cmd.push_str(&program.string_pool[*idx as usize]),
                            BuildPart::Reg(r) => cmd.push_str(&shell_text(self.reg_get(*r))),
                        }
                    }
                    let out = match run_in_shell(&cmd) { Ok(o) => o, Err(e) => raise!(e.into()) };
                    // Capture both stdout and stderr (mirrors tree-walker behavior)
                    let mut result = String::from_utf8_lossy(&out.stdout).into_owned();
                    if !out.stderr.is_empty() {
                        let stderr_str = String::from_utf8_lossy(&out.stderr);
                        if result.is_empty() {
                            result = stderr_str.into_owned();
                        } else {
                            result.push_str(&stderr_str);
                        }
                    }
                    // Strip trailing newline (consistent with shell $(...) behavior)
                    let result = result.trim_end_matches('\n').to_string();
                    self.reg_set(dst, Value::String(ZyStr::new(result)));
                }

                // ── Execute expression </ path /> ─────────────────────────────
                Instruction::Execute(dst, parts) => {
                    let dst = *dst;
                    let mut cmd = String::new();
                    for part in parts {
                        match part {
                            BuildPart::Lit(idx) => cmd.push_str(&program.string_pool[*idx as usize]),
                            BuildPart::Reg(r) => cmd.push_str(&self.reg_get(*r).to_string_repr()),
                        }
                    }
                    let out = match run_in_shell(&cmd) { Ok(o) => o, Err(e) => raise!(e.into()) };
                    if !out.status.success() {
                        let mut msg = String::from_utf8_lossy(&out.stderr).into_owned();
                        if msg.is_empty() {
                            msg = String::from_utf8_lossy(&out.stdout).into_owned();
                        }
                        let msg = msg.trim_end().to_string();
                        raise!(VmError::Generic(msg));
                    }
                    let result = String::from_utf8_lossy(&out.stdout).into_owned();
                    self.reg_set(dst, Value::String(ZyStr::new(result)));
                }

                // ── Format ops ────────────────────────────────────────────────
                // GAP-ZYB-001: the same four operations with the decimal count
                // in a register. The count is read, checked, and the immediate
                // path below does the rest.
                &Instruction::FmtThousandsDyn(dst, src, prec_kind, prec_reg) => {
                    let n = match vm_precision_from(self.reg_get(prec_reg)) {
                        Ok(n) => n,
                        Err(e) => raise!(e),
                    };
                    let f = match vm_number_from(self.reg_get(src)) {
                        Ok(f) => f,
                        Err(e) => raise!(e),
                    };
                    let s = map_numeral_number(vm_fmt_thousands(f, prec_kind, n), self.numeral_mode);
                    self.reg_set(dst, Value::String(ZyStr::new(s)));
                }
                &Instruction::FmtScientificDyn(dst, src, prec_kind, prec_reg) => {
                    let n = match vm_precision_from(self.reg_get(prec_reg)) {
                        Ok(n) => n,
                        Err(e) => raise!(e),
                    };
                    let f = match vm_number_from(self.reg_get(src)) {
                        Ok(f) => f,
                        Err(e) => raise!(e),
                    };
                    let s = map_numeral_number(vm_fmt_scientific(f, prec_kind, n), self.numeral_mode);
                    self.reg_set(dst, Value::String(ZyStr::new(s)));
                }
                &Instruction::RoundFloatDyn(dst, src, prec_reg) => {
                    let n = match vm_precision_from(self.reg_get(prec_reg)) {
                        Ok(n) => n,
                        Err(e) => raise!(e),
                    };
                    let f = match vm_number_from(self.reg_get(src)) {
                        Ok(f) => f,
                        Err(e) => raise!(e),
                    };
                    let m = 10f64.powi(n as i32);
                    self.reg_set(dst, Value::Float((f * m).round() / m));
                }
                &Instruction::TruncFloatDyn(dst, src, prec_reg) => {
                    let n = match vm_precision_from(self.reg_get(prec_reg)) {
                        Ok(n) => n,
                        Err(e) => raise!(e),
                    };
                    let f = match vm_number_from(self.reg_get(src)) {
                        Ok(f) => f,
                        Err(e) => raise!(e),
                    };
                    let m = 10f64.powi(n as i32);
                    self.reg_set(dst, Value::Float((f * m).trunc() / m));
                }
                &Instruction::FmtThousands(dst, src, prec_kind, prec_n) => {
                    let f = match self.reg_get(src) {
                        Value::Int(n) => *n as f64,
                        Value::Float(f) => *f,
                        other => match ascii_digits(other.to_string().trim()).parse::<f64>() {
                            Ok(f) => f,
                            // The tree-walker rejects a non-number here; returning
                            // 0.0 silently made the two engines disagree.
                            Err(_) => raise!(VmError::TypeMsg(format!("format expressions only work with numbers, got {}", other.type_label()))),
                        },
                    };
                    let s = map_numeral_number(vm_fmt_thousands(f, prec_kind, prec_n), self.numeral_mode);
                    self.reg_set(dst, Value::String(ZyStr::new(s)));
                }
                &Instruction::FmtScientific(dst, src, prec_kind, prec_n) => {
                    let f = match self.reg_get(src) {
                        Value::Int(n) => *n as f64,
                        Value::Float(f) => *f,
                        other => match ascii_digits(other.to_string().trim()).parse::<f64>() {
                            Ok(f) => f,
                            Err(_) => raise!(VmError::TypeMsg(format!("format expressions only work with numbers, got {}", other.type_label()))),
                        },
                    };
                    let s = map_numeral_number(vm_fmt_scientific(f, prec_kind, prec_n), self.numeral_mode);
                    self.reg_set(dst, Value::String(ZyStr::new(s)));
                }

                // ── Precision ops ─────────────────────────────────────────────
                &Instruction::RoundFloat(dst, src, precision) => {
                    let f = match self.reg_get(src) {
                        Value::Int(n) => *n as f64,
                        Value::Float(f) => *f,
                        // Digits in any script (a number rendered under a numeral
                        // mode rounds like its ASCII twin); a string that is not a
                        // number at all is an error, as it is in the tree-walker —
                        // it used to become 0.0 without a word.
                        Value::String(s) => match ascii_digits(s.as_ref().trim()).parse::<f64>() {
                            Ok(f) => f,
                            Err(_) => raise!(VmError::Generic(format!(
                                "cannot convert string '{}' to number for rounding", s.as_ref()
                            ))),
                        },
                        other => raise!(VmError::TypeMsg(format!("round expressions only work with numbers or numeric strings, got {}", other.type_label()))),
                    };
                    let m = 10_f64.powi(precision as i32);
                    self.reg_set(dst, Value::Float((f * m).round() / m));
                }
                &Instruction::TruncFloat(dst, src, precision) => {
                    let f = match self.reg_get(src) {
                        Value::Int(n) => *n as f64,
                        Value::Float(f) => *f,
                        Value::String(s) => match ascii_digits(s.as_ref().trim()).parse::<f64>() {
                            Ok(f) => f,
                            Err(_) => raise!(VmError::Generic(format!(
                                "cannot convert string '{}' to number for truncation", s.as_ref()
                            ))),
                        },
                        other => raise!(VmError::TypeMsg(format!("truncate expressions only work with numbers or numeric strings, got {}", other.type_label()))),
                    };
                    let m = 10_f64.powi(precision as i32);
                    self.reg_set(dst, Value::Float((f * m).trunc() / m));
                }

                // ── Error check ───────────────────────────────────────────────
                &Instruction::IsError(dst, src) => {
                    let is_err = matches!(self.reg_get(src), Value::Error(_));
                    self.reg_set(dst, Value::Bool(is_err));
                }
                &Instruction::LoadErrorKind(dst) => {
                    let kind = self.frame_stack.last()
                        .and_then(|f| f.error.as_ref())
                        .and_then(|e| e.pending.last())
                        .map(|p| p.kind.to_string())
                        .unwrap_or_default();
                    unsafe { *self.value_stack.get_unchecked_mut(base + dst as usize) = Value::String(ZyStr::new(kind)); }
                }

                // ── Output param writeback setup ──────────────────────────────
                Instruction::SetupOutputWriteback(pairs) => {
                    self.pending_output_writeback = pairs.iter()
                        .map(|&(param_idx, caller_reg)| (param_idx as usize, caller_reg))
                        .collect();
                }

                &Instruction::RaiseError(msg_idx) => {
                    let msg = program.string_pool[msg_idx as usize].clone();
                    raise!(VmError::Generic(msg));
                }

                &Instruction::LoadGlobal(dst, gvar_idx) => {
                    if let Some(name) = self.destroyed_globals.get(&gvar_idx) {
                        raise!(VmError::Generic(format!(
                            "use after destruction: variable '{}' was destroyed \
                             after its last use", name)));
                    }
                    let val = self.global_vars
                        .get(gvar_idx as usize)
                        .cloned()
                        .unwrap_or(Value::Unit);
                    wreg!(dst, val);
                }

                &Instruction::StoreGlobal(gvar_idx, src) => {
                    // Assigning revives a destroyed name: `\` ends a life, it
                    // does not burn the name.
                    self.destroyed_globals.remove(&gvar_idx);
                    let val = rreg!(src).clone();
                    if let Some(slot) = self.global_vars.get_mut(gvar_idx as usize) {
                        *slot = val;
                    }
                }

                &Instruction::DestroyGlobal(gvar_idx, name_idx) => {
                    let name = self.string_rcs[name_idx as usize].to_string();
                    self.destroyed_globals.insert(gvar_idx, name);
                    if let Some(slot) = self.global_vars.get_mut(gvar_idx as usize) {
                        *slot = Value::Unit;
                    }
                }

                // ── TUI primitives ──────────────────────────────────────────────
                &Instruction::Sleep(reg) => {
                    let ms = match rreg!(reg) {
                        Value::Int(n) if *n >= 0 => *n as u64,
                        Value::Int(n) => raise!(VmError::Generic(format!(
                            "@~ requires non-negative ms, got {}", n))),
                        other => raise!(VmError::TypeMsg(format!("@~ requires integer milliseconds, got {}", other.type_label()))),
                    };
                    std::thread::sleep(std::time::Duration::from_millis(ms));
                }

                Instruction::ClearScreen => {
                    crossterm::execute!(std::io::stdout(),
                        crossterm::terminal::Clear(crossterm::terminal::ClearType::All),
                        crossterm::cursor::MoveTo(0, 0)).ok();
                }

                &Instruction::QueryTerminalSize(dst) => {
                    let (cols, rows) = crossterm::terminal::size().unwrap_or((80, 24));
                    wreg!(dst, Value::Tuple(std::rc::Rc::new(vec![Value::Int(rows as i64), Value::Int(cols as i64)])));
                }

                &Instruction::ReadKey(dst, blocking) => {
                    use crossterm::event::{self, Event};
                    let ch = if blocking {
                        loop {
                            match event::read() {
                                Ok(Event::Key(key)) if vm_is_key_press(&key) => {
                                    break vm_map_key_code(&key)
                                }
                                Ok(_) => continue,
                                Err(e) => return Err(VmError::Generic(e.to_string())),
                            }
                        }
                    } else {
                        // Drain to the first keypress rather than giving up on the
                        // first event that is not one — see the tree-walker's
                        // execute_key_input for why a single read per call makes a
                        // game loop fall a fixed number of ticks behind on Windows.
                        let mut found = '\0';
                        while event::poll(std::time::Duration::ZERO).unwrap_or(false) {
                            match event::read() {
                                Ok(Event::Key(key)) if vm_is_key_press(&key) => {
                                    found = vm_map_key_code(&key);
                                    break;
                                }
                                Ok(_) => continue,
                                Err(_) => break,
                            }
                        }
                        found
                    };
                    wreg!(dst, Value::Char(ch));
                }

                Instruction::ReadLine(dst, prompt_reg, kind) => {
                    use std::io::{BufRead, Write};
                    let in_tui = !self.tui_stack.is_empty();
                    // Read / validate / re-prompt loop (mirrors the tree-walker). An empty
                    // raw line means EOF (a typed blank line is "\n"); EOF aborts so a failed
                    // constraint cannot spin forever on a closed pipe.
                    // The loop breaks with a Result and the error is raised after
                        // it: a `raise!` in here would `continue` THIS loop, which
                        // read end-of-input again with the handler already spent,
                        // so `!? { << n } :! { }` never caught it.
                    let value: Result<Value, VmError> = loop {
                        if let Some(pr) = prompt_reg {
                            print!("{}", self.numeral_repr(rreg!(*pr)));
                            std::io::stdout().flush().ok();
                        }
                        if in_tui {
                            crossterm::terminal::disable_raw_mode().ok();
                            crossterm::execute!(std::io::stdout(), crossterm::cursor::Show).ok();
                        }
                        let mut line = String::new();
                        std::io::stdin().lock().read_line(&mut line).ok();
                        if in_tui {
                            crossterm::execute!(std::io::stdout(), crossterm::cursor::Hide).ok();
                            if let Err(e) = crossterm::terminal::enable_raw_mode() {
                                break Err(VmError::Generic(format!("input: failed to restore raw mode: {}", e)));
                            }
                        }
                        if line.is_empty() {
                            break Err(VmError::Generic(format!(
                                "end of input while waiting for {}", vm_describe_input_kind(kind)
                            )));
                        }
                        match vm_validate_input(line.trim(), kind) {
                            Ok(v) => break Ok(v),
                            Err(hint) => {
                                println!("  ({})", hint);
                                std::io::stdout().flush().ok();
                            }
                        }
                    };
                    let value = match value { Ok(v) => v, Err(e) => raise!(e) };
                    wreg!(*dst, value);
                }

                Instruction::PrintAt(r_pos, item_regs) => {
                    let pos_val = rreg!(*r_pos).clone();
                    let (fila, col, bks, fg, bg) = vm_extract_pos(pos_val);
                    if let (Some(r), Some(c)) = (fila, col) {
                        crossterm::execute!(std::io::stdout(),
                            crossterm::cursor::MoveTo(c - 1, r - 1)).ok();
                    }
                    let mut styled = false;
                    if bks & 1 != 0 { crossterm::execute!(std::io::stdout(), crossterm::style::SetAttribute(crossterm::style::Attribute::Bold)).ok();       styled = true; }
                    if bks & 2 != 0 { crossterm::execute!(std::io::stdout(), crossterm::style::SetAttribute(crossterm::style::Attribute::Italic)).ok();     styled = true; }
                    if bks & 4 != 0 { crossterm::execute!(std::io::stdout(), crossterm::style::SetAttribute(crossterm::style::Attribute::Underlined)).ok(); styled = true; }
                    let mut colored = false;
                    if let Some(fg) = fg {
                        crossterm::execute!(std::io::stdout(),
                            crossterm::style::SetForegroundColor(
                                crossterm::style::Color::AnsiValue(fg as u8))).ok();
                        colored = true;
                    }
                    if let Some(bg) = bg {
                        crossterm::execute!(std::io::stdout(),
                            crossterm::style::SetBackgroundColor(
                                crossterm::style::Color::AnsiValue(bg as u8))).ok();
                        colored = true;
                    }
                    for &r in item_regs {
                        print!("{}", self.numeral_repr(rreg!(r)));
                    }
                    if styled || colored {
                        crossterm::execute!(std::io::stdout(),
                            crossterm::style::SetAttribute(crossterm::style::Attribute::Reset)).ok();
                    }
                    std::io::stdout().flush().ok();
                }

                Instruction::EnterTui => {
                    if let Err(e) = crossterm::terminal::enable_raw_mode() {
                        raise!(VmError::Generic(format!("failed to enable raw mode: {}", e)));
                    }
                    if let Err(e) = crossterm::execute!(std::io::stdout(),
                        crossterm::terminal::EnterAlternateScreen,
                        crossterm::cursor::MoveTo(0, 0),
                        crossterm::cursor::Hide)
                    {
                        let _ = crossterm::terminal::disable_raw_mode();
                        raise!(VmError::Generic(format!("failed to enter alternate screen: {}", e)));
                    }
                    self.tui_stack.push(TuiGuard);
                }

                Instruction::ExitTui => {
                    // Pop the guard — its Drop performs cleanup.
                    // If ExitTui is skipped (@:label! / error), the guard is dropped
                    // when `run` clears the stack on its way out.
                    self.tui_stack.pop();
                }

                &Instruction::HotInit(dst, neutral) => {
                    if matches!(self.reg_get(dst), Value::Unit) {
                        let val = match neutral {
                            zymbol_bytecode::HotNeutral::Int => Value::Int(0),
                            zymbol_bytecode::HotNeutral::IntOne => Value::Int(1),
                            zymbol_bytecode::HotNeutral::Array => Value::Array(Rc::new(Vec::new())),
                            zymbol_bytecode::HotNeutral::String => Value::String(ZyStr::from_str_ref("")),
                        };
                        wreg!(dst, val);
                    }
                }

                &Instruction::LoadCliArgs(dst) => {
                    let arr: Vec<Value> = self.cli_args.iter()
                        .map(|s| Value::String(ZyStr::from_str_ref(s)))
                        .collect();
                    wreg!(dst, Value::Array(Rc::new(arr)));
                }

                Instruction::Halt => return Ok(Value::Unit),
            }
        }
    }

    // ── Helpers ──────────────────────────────────────────────────────────────

    #[inline(always)]
    fn reg_get(&self, reg: Reg) -> &Value {
        let base = self.frame_stack.last().unwrap().base as usize;
        &self.value_stack[base + reg as usize]
    }

    #[inline(always)]
    fn reg_set(&mut self, reg: Reg, val: Value) {
        let base = self.frame_stack.last().unwrap().base as usize;
        self.value_stack[base + reg as usize] = val;
    }

    #[inline(always)]
    fn as_int(&self, reg: Reg) -> Result<i64, VmError> {
        match self.reg_get(reg) {
            Value::Int(n) => Ok(*n),
            other => Err(VmError::TypeError {
                expected: "Int",
                got: other.type_name().to_string(),
            }),
        }
    }

    /// Call a Function or Closure value from inside an instruction — the calls
    /// `$>`, `$|`, `$<`, `$^` and their string-splitting forms make.
    ///
    /// Pushes the callee's frame exactly as `CallDynamic` does and runs the same
    /// dispatch loop down to it (`exec`). It used to hand the call to a second
    /// interpreter loop that knew 96 of the instructions and skipped the other
    /// 49, so a split, a slice, a format, a `??` over a range, a `$!` or a `!?`
    /// inside the callee all answered `##_` without an error (ZYVM-004). There
    /// is one loop now; an instruction added to it is added everywhere.
    fn call_callable(
        &mut self,
        callable: Value,
        args: Vec<Value>,
        program: &CompiledProgram,
    ) -> Result<Value, VmError> {
        let (func_idx, upvalues) = match callable {
            Value::Function(idx, _) => (idx, None),
            Value::Closure(idx, _, upvalues) => (idx, Some(upvalues)),
            other => return Err(VmError::TypeError { expected: "Function", got: other.type_name().to_string() }),
        };
        let Some(chunk) = program.functions.get(func_idx as usize) else {
            return Err(VmError::UndefinedFunction(func_idx));
        };
        let num_params = chunk.num_params as usize;
        let num_regs = chunk.num_registers as usize;

        let floor = self.frame_stack.len();
        let new_base = self.value_stack.len();
        self.value_stack.resize(new_base + num_regs, Value::Unit);
        for (i, v) in args.into_iter().enumerate() {
            if i < num_regs { self.value_stack[new_base + i] = v; }
        }
        if let Some(upvalues) = upvalues {
            for (i, uv) in upvalues.iter().enumerate() {
                let slot = num_params + i;
                if slot < num_regs { self.value_stack[new_base + slot] = uv.clone(); }
            }
        }
        self.frame_stack.push(FrameInfo {
            base: new_base as u32,
            ip: 0,
            chunk_idx: func_idx,
            return_reg: 0,
            catch_ip: u32::MAX,
            error: None,
            writeback: None,
        });

        // The instruction that called: an error the operator raises after this
        // call has to be reported where the operator is, not inside the callee.
        let (caller_ip, caller_chunk) = (self.cur_ip, self.cur_chunk);
        let result = self.exec(program, floor);
        match &result {
            Ok(_) => {
                self.cur_ip = caller_ip;
                self.cur_chunk = caller_chunk;
            }
            // `exec` gives back its frames when it raises; one that left
            // through `?` may not have, and the operator must see its own stack.
            Err(_) => {
                if self.frame_stack.len() > floor {
                    let floor_base = self.frame_stack[floor].base as usize;
                    self.frame_stack.truncate(floor);
                    self.value_stack.truncate(floor_base);
                }
            }
        }
        result
    }
}

/// Functional update (`$~`) through an index path — mirrors the tree-walker's
/// `deep_update_value` over VM values. Steps are Int (1-based, negative counts
/// from the end) for arrays, tuples, and named tuples; a String step addresses
/// a named-tuple field by name. An empty remaining path replaces the value.
/// The tree-walker's refusal of `t[i] = val`, word for word.
///
/// Spelled once and quoted from here because `zyq consensus` compares text: two
/// engines that refuse the same program with different wording are still a
/// divergence. The tree-walker's copy is in
/// `zymbol-interpreter/src/variables.rs`.
fn tuple_immutable_msg(name: &str) -> String {
    format!(
        "cannot modify tuple '{}': tuples are immutable\nhelp: use 'new = {}[i]$~ value' for a functional update",
        name, name
    )
}

/// The refusal of an absent dictionary key, spelled as the tree-walker spells
/// it (`zymbol-interpreter::variables::missing_key_msg`) — `zyq consensus`
/// compares text, and this engine used to say the least of the three: no list of
/// available keys at all.
/// The refusal of a positional address on a dictionary, spelled as the
/// tree-walker spells it (`collection_ops::dict_not_positional`).
///
/// Decision 11 withdrew `d[2]`, and the reasoning covers the whole family: in a
/// mutable dictionary a position is not a stable address. A positional WRITE is
/// strictly worse than a positional read, since it corrupts data rather than
/// returning the wrong value.
fn dict_not_positional(op: &str, first_key: Option<&str>) -> String {
    let k = first_key.unwrap_or("clave");
    format!(
        "a dictionary is addressed by key, not by position: `{}` has no meaning here\nhelp: use the key — d[\"{}\"], d[\"{}\"]$~ value, d$-[\"{}\"] — because adding a key changes what sits at each position",
        op, k, k, k
    )
}

fn missing_key_msg(key: &str, available: &[String]) -> String {
    if available.is_empty() {
        format!("no key '{}' in dictionary — it is empty", key)
    } else {
        format!("no key '{}' in dictionary — available: {}", key, available.join(", "))
    }
}

fn vm_deep_set(col: Value, path: &[Value], new_val: Value) -> Result<Value, VmError> {
    let Some((step, rest)) = path.split_first() else {
        return Ok(new_val);
    };
    fn resolve(idx: i64, len: usize, container: &'static str) -> Result<usize, VmError> {
        if idx == 0 {
            return Err(VmError::IndexZero);
        }
        let i = if idx < 0 { len as i64 + idx } else { idx - 1 };
        if i < 0 || i as usize >= len {
            return Err(VmError::IndexOutOfBounds { index: idx, length: len, container });
        }
        Ok(i as usize)
    }
    fn int_step(step: &Value) -> Result<i64, VmError> {
        match step {
            Value::Int(n) => Ok(*n),
            // The tree-walker's words for a step that is neither (step 3.5b).
            other => Err(VmError::TypeMsg(format!(
                "a navigation step is a position (Int) or a dictionary key (String), got {}",
                other.type_label()))),
        }
    }
    match col {
        Value::Array(mut rc) => {
            let arr = Rc::make_mut(&mut rc);
            let i = resolve(int_step(step)?, arr.len(), "array")?;
            let sub = mem::replace(&mut arr[i], Value::Unit);
            arr[i] = vm_deep_set(sub, rest, new_val)?;
            Ok(Value::Array(rc))
        }
        Value::Tuple(mut rc) => {
            let tup = Rc::make_mut(&mut rc);
            let i = resolve(int_step(step)?, tup.len(), "tuple")?;
            let sub = mem::replace(&mut tup[i], Value::Unit);
            tup[i] = vm_deep_set(sub, rest, new_val)?;
            Ok(Value::Tuple(rc))
        }
        Value::NamedTuple(mut rc) => {
            let fields = Rc::make_mut(&mut rc);
            let i = match step {
                Value::String(name) => match fields.iter().position(|(k, _)| k == name.as_str()) {
                    Some(i) => i,
                    // A key that is not there gets ADDED, as it does in Python.
                    // The array refuses the same move (decision 13) and the two
                    // are not inconsistent: an array is addressed by POSITION,
                    // so writing past the end leaves a hole; a dictionary is
                    // addressed by KEY and has no holes to leave.
                    None => {
                        fields.push((name.as_str().to_string(), Value::Unit));
                        fields.len() - 1
                    }
                },
                // A positional WRITE corrupts data rather than returning the
                // wrong value: strictly worse than the positional read that
                // decision 11 withdrew.
                _ => {
                    let first = fields.first().map(|(k, _)| k.clone());
                    return Err(VmError::TypeMsg(dict_not_positional(
                        "d[n]$~ value", first.as_deref())));
                }
            };
            let sub = mem::replace(&mut fields[i].1, Value::Unit);
            fields[i].1 = vm_deep_set(sub, rest, new_val)?;
            Ok(Value::NamedTuple(rc))
        }
        other => Err(VmError::TypeMsg(format!(
            "$~ writes into a collection, and this is {}\nhelp: use a[1]$~ v on an array or tuple, d[\"key\"]$~ v on a #(…)",
            other.type_label()
        ))),
    }
}

/// Is this a key going *down*, as opposed to coming back up?
///
/// Mirrors the tree-walker's `is_key_press` — Windows reports key releases as well
/// as presses, so `<<|` counted every keystroke twice there. See the comment on the
/// tree-walker copy for the full story; the two must agree or the engines diverge on
/// exactly the platform where the behaviour is hard to notice.
fn vm_is_key_press(key: &crossterm::event::KeyEvent) -> bool {
    use crossterm::event::KeyEventKind;
    matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat)
}

/// Translate a key event into the single character `<<|` yields.
///
/// Mirrors the tree-walker's `map_key_code`; the two must agree or the engines
/// diverge on the keyboard, which only the pty harness can catch.
fn vm_map_key_code(key: &crossterm::event::KeyEvent) -> char {
    use crossterm::event::{KeyCode::*, KeyModifiers};

    // Ctrl+letter is a control character, and that is what the terminal puts on
    // the wire: Ctrl+A is 0x01, Ctrl+S is 0x13. crossterm hands it over
    // decoded — `Char('a')` with CONTROL set — and this function used to read
    // the code and drop the modifiers, so Ctrl+A arrived as the letter `a`
    // (BUG-ZYB-006). Not "the combination never arrived": it arrived wearing
    // another key's clothes, which is worse — a Ctrl+X shortcut fired when the
    // user typed an x into a text field, and no full-screen program could offer
    // Ctrl+S, Ctrl+Q or Ctrl+C at all.
    //
    // Handing back the control character adds nothing to the language: `0d1`
    // already writes it, and `##!t < 32` already asks the question.
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        if let Char(c) = key.code {
            let lower = c.to_ascii_lowercase();
            if lower.is_ascii_lowercase() {
                return (lower as u8 - b'a' + 1) as char;
            }
        }
    }

    match key.code {
        Char(c) => c,
        Up      => '\u{2191}',
        Down    => '\u{2193}',
        Left    => '\u{2190}',
        Right   => '\u{2192}',
        Enter   => '\n',
        Esc     => '\x1B',
        // Tab and Backspace used to fall through to `'\0'` together, so a
        // program could not tell them apart — harmless in a numeric field,
        // which is why ZyBank treated both as "delete", and impossible in a
        // form where Tab moves between fields: every jump would erase a
        // character. Both now carry what the terminal sends for them.
        Tab       => '\t',      // 0d9
        Backspace => '\x7F',    // 0d127 — DEL, which is what a terminal sends
        _         => '\0',
    }
}

fn vm_extract_pos(val: Value) -> (Option<u16>, Option<u16>, i64, Option<i64>, Option<i64>) {
    let items = match val {
        Value::Tuple(v) => (*v).clone(),
        _ => return (None, None, 0, None, None),
    };
    // Variable-based mode: >>~ pos > items compiles as MakeTuple([r_pos]) wrapping the
    // dense tuple. Unwrap one level when the outer tuple holds a single inner tuple.
    if items.len() == 1 {
        if let Value::Tuple(_) = &items[0] {
            return vm_extract_pos(items.into_iter().next().unwrap());
        }
    }
    let get_int = |i: usize| -> Option<i64> {
        match items.get(i) {
            Some(Value::Int(n)) => Some(*n),
            _ => None, // Unit or absent = None
        }
    };
    let fila = get_int(0).map(|n| n as u16);
    let col  = get_int(1).map(|n| n as u16);
    let bks  = get_int(2).unwrap_or(0);
    let fg   = get_int(3);
    let bg   = get_int(4);
    (fila, col, bks, fg, bg)
}

fn vm_natural_cmp(a: &Value, b: &Value) -> std::cmp::Ordering {
    match (a, b) {
        (Value::Int(x), Value::Int(y))       => x.cmp(y),
        (Value::Float(x), Value::Float(y))   => x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal),
        (Value::Int(x), Value::Float(y))     => (*x as f64).partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal),
        (Value::Float(x), Value::Int(y))     => x.partial_cmp(&(*y as f64)).unwrap_or(std::cmp::Ordering::Equal),
        (Value::String(x), Value::String(y)) => x.cmp(y),
        (Value::Bool(x), Value::Bool(y))     => x.cmp(y),
        _                                    => std::cmp::Ordering::Equal,
    }
}
