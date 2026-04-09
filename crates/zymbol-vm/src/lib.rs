//! Register VM executor for Zymbol-Lang
//!
//! Sprint 4: replaces tree-walker for hot paths.
//! Sprint 5C: flat register stack — all frames share one Vec<Value>.
//! Sprint 5D: Value uses Rc<T> for heap payloads → sizeof(Value) = 16 bytes.
//! Design goals:
//! - registers[idx] O(1) vs HashMap lookup
//! - Flat value_stack: no per-call heap alloc, better cache locality
//! - mem::replace for Return → O(1) move of String/Array
//! - Rc<T> payloads: 2.5x cheaper memset/memcpy on flat stack

use std::fmt;
use std::io::Write;
use std::mem;
use std::rc::Rc;

use thiserror::Error;
use zymbol_bytecode::{BuildPart, Chunk, CompiledProgram, FuncIdx, Instruction, Reg};

// ──────────────────────────────────────────────────────────────────────────────
// Numeral-mode helpers (mirrors zymbol-interpreter::numeral_mode)
// ──────────────────────────────────────────────────────────────────────────────

const ASCII_BASE: u32 = 0x0030;

fn map_ascii_digits(s: &str, block_base: u32) -> String {
    if block_base == ASCII_BASE {
        return s.to_string();
    }
    s.chars()
        .map(|ch| {
            if ch.is_ascii_digit() {
                char::from_u32(block_base + (ch as u32 - ASCII_BASE)).unwrap_or(ch)
            } else {
                ch
            }
        })
        .collect()
}

fn numeral_int(value: i64, base: u32) -> String { map_ascii_digits(&value.to_string(), base) }
fn numeral_float(value: f64, base: u32) -> String { map_ascii_digits(&value.to_string(), base) }
fn numeral_bool(value: bool, base: u32) -> String { format!("#{}", numeral_int(if value { 1 } else { 0 }, base)) }

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
    /// Function/lambda reference stored as index into program.functions
    Function(FuncIdx),
    String(Rc<String>),
    Array(Rc<Vec<Value>>),
    /// Positional tuple: (v1, v2, v3)
    Tuple(Rc<Vec<Value>>),
    /// Named tuple: Vec of (field_name, value) pairs
    NamedTuple(Rc<Vec<(String, Value)>>),
    /// Closure: function + captured upvalues from the enclosing scope
    Closure(FuncIdx, Rc<Vec<Value>>),
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
                write!(f, "(")?;
                for (i, (name, val)) in fields.as_ref().iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{}: {}", name, val)?;
                }
                write!(f, ")")
            }
            Value::Function(_) => write!(f, "<lambda>"),
            Value::Closure(_, _) => write!(f, "<lambda>"),
            Value::Unit => write!(f, "()"),
        }
    }
}

impl Value {
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
            Value::String(s) => s.as_ref().clone(),
            other => other.to_string(),
        }
    }

    fn type_name(&self) -> &'static str {
        match self {
            Value::Int(_) => "Int",
            Value::Float(_) => "Float",
            Value::String(_) => "String",
            Value::Char(_) => "Char",
            Value::Bool(_) => "Bool",
            Value::Array(_) => "Array",
            Value::Tuple(_) => "Tuple",
            Value::NamedTuple(_) => "Tuple",
            Value::Function(_) => "Function",
            Value::Closure(_, _) => "Function",
            Value::Unit => "Unit",
        }
    }

    /// Equality for pattern matching and $? operator
    fn equals(&self, other: &Value) -> bool {
        match (self, other) {
            (Value::Int(a), Value::Int(b)) => a == b,
            (Value::Float(a), Value::Float(b)) => a == b,
            (Value::String(a), Value::String(b)) => a.as_ref() == b.as_ref(),
            (Value::Char(a), Value::Char(b)) => a == b,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Unit, Value::Unit) => true,
            _ => false,
        }
    }

}

// ──────────────────────────────────────────────────────────────────────────────
// FrameInfo — Sprint 5C: metadata only, no registers
// Sprint 5E: slim FrameInfo — heap-allocate rare fields to reduce sizeof
// ──────────────────────────────────────────────────────────────────────────────

/// Error state for a frame with an active try/catch — heap-allocated on demand.
struct FrameError {
    error_val: Option<Value>,
    /// Error kind string: "IO", "Index", "Type", "Div", "_"
    error_kind: String,
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
    /// Try/catch: instruction index to jump to on error (u32::MAX = no active catch)
    catch_ip: u32,
    /// Nesting depth of try blocks in this frame
    try_depth: u8,
    /// Error state — allocated only when a try block is active (None for normal calls)
    error: Option<Box<FrameError>>,
    /// Output param writeback — None for most functions (saves 24 bytes)
    writeback: Option<Box<Vec<(usize, Reg)>>>,
}

// ──────────────────────────────────────────────────────────────────────────────
// VM Error
// ──────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum VmError {
    #[error("type error: expected {expected}, got {got}")]
    TypeError { expected: &'static str, got: String },
    #[error("division by zero")]
    DivisionByZero,
    #[error("index out of bounds: index {index}, length {length}")]
    IndexOutOfBounds { index: i64, length: usize },
    #[error("undefined function index {0}")]
    UndefinedFunction(FuncIdx),
    #[error("register {0} out of range")]
    RegisterOutOfRange(Reg),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Generic(String),
}

// ──────────────────────────────────────────────────────────────────────────────
// Free helpers (not methods — avoids borrow conflicts in the dispatch loop)
// ──────────────────────────────────────────────────────────────────────────────

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
fn cmp_direct(va: &Value, vb: &Value) -> i32 {
    use std::cmp::Ordering;
    fn ord(o: Ordering) -> i32 { match o { Ordering::Less => -1, Ordering::Equal => 0, Ordering::Greater => 1 } }
    match (va, vb) {
        (Value::Int(x), Value::Int(y))     => ord(x.cmp(y)),
        (Value::Float(x), Value::Float(y)) => ord(x.partial_cmp(y).unwrap_or(Ordering::Equal)),
        (Value::Int(x), Value::Float(y))   => ord((*x as f64).partial_cmp(y).unwrap_or(Ordering::Equal)),
        (Value::Float(x), Value::Int(y))   => ord(x.partial_cmp(&(*y as f64)).unwrap_or(Ordering::Equal)),
        (Value::String(x), Value::String(y)) => ord(x.as_ref().as_str().cmp(y.as_ref().as_str())),
        (Value::Char(x), Value::Char(y))   => ord(x.cmp(y)),
        (Value::Bool(x), Value::Bool(y))   => ord(x.cmp(y)),
        _ => 1,
    }
}

#[inline(always)]
fn get_chunk(program: &CompiledProgram, chunk_idx: usize) -> &Chunk {
    // chunk_idx == usize::MAX OR u32::MAX as usize both indicate the main chunk
    if chunk_idx >= program.functions.len() {
        &program.main
    } else {
        &program.functions[chunk_idx]
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
    /// Interned string pool — Rc<String> for each entry, shared across LoadStr calls
    string_rcs: Vec<Rc<String>>,
    /// Active numeral mode: block base codepoint (0x0030 = ASCII default).
    numeral_mode: u32,
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
            output,
        }
    }

    pub fn run(&mut self, program: &CompiledProgram) -> Result<(), VmError> {
        // Reset flat stack for this execution
        self.value_stack.clear();
        self.frame_stack.clear();

        // Pre-intern string pool: O(n) once, then LoadStr is O(1) Rc clone
        self.string_rcs = program.string_pool.iter().map(|s| Rc::new(s.clone())).collect();

        // Push initial frame for main chunk
        let num_regs = program.main.num_registers as usize;
        self.value_stack.resize(num_regs, Value::Unit);
        let main_frame = FrameInfo {
            base: 0,
            ip: 0,
            chunk_idx: u32::MAX,
            return_reg: 0,
            catch_ip: u32::MAX,
            try_depth: 0,
            error: None,
            writeback: None,
        };
        self.frame_stack.push(main_frame);

        // Local variables — avoid frame_stack.last() overhead on every instruction
        let mut ip: usize = 0;
        let mut chunk_idx: usize = usize::MAX;
        // base: offset in value_stack where current frame's registers start
        let mut base: usize = 0;

        // ── Hot-path register access macros ──────────────────────────────────
        // SAFETY: base + reg < value_stack.len() by compiler construction.
        macro_rules! rreg {
            ($r:expr) => { unsafe { self.value_stack.get_unchecked(base + $r as usize) } }
        }
        macro_rules! wreg {
            ($r:expr, $v:expr) => { unsafe { *self.value_stack.get_unchecked_mut(base + $r as usize) = $v } }
        }
        // ri!: read register as Int, raise TypeError on mismatch
        macro_rules! ri {
            ($r:expr) => {
                match unsafe { self.value_stack.get_unchecked(base + $r as usize) } {
                    Value::Int(n) => *n,
                    other => raise!(VmError::TypeError { expected: "Int", got: other.type_name().to_string() }),
                }
            }
        }
        // rf!: read register as Float (Int coerced), raise TypeError on mismatch
        macro_rules! rf {
            ($r:expr) => {
                match unsafe { self.value_stack.get_unchecked(base + $r as usize) } {
                    Value::Float(n) => *n,
                    Value::Int(n) => *n as f64,
                    other => raise!(VmError::TypeError { expected: "Float", got: other.type_name().to_string() }),
                }
            }
        }

        macro_rules! raise {
            ($e:expr) => {{
                let _err = $e;
                if let Some(frame) = self.frame_stack.last_mut() {
                    if frame.catch_ip != u32::MAX {
                        let catch = frame.catch_ip;
                        frame.catch_ip = u32::MAX;
                        let kind = match &_err {
                            VmError::TypeError { .. } => "Type",
                            VmError::DivisionByZero => "Div",
                            VmError::IndexOutOfBounds { .. } => "Index",
                            VmError::Io(_) => "IO",
                            _ => "_",
                        };
                        frame.try_depth = 0;
                        let err_data = frame.error.get_or_insert_with(|| Box::new(FrameError { error_val: None, error_kind: String::new() }));
                        err_data.error_kind = kind.to_string();
                        err_data.error_val = Some(Value::String(Rc::new(format!("{}", _err))));
                        ip = catch as usize;
                        continue;
                    }
                }
                return Err(_err);
            }};
        }

        loop {
            // chunk borrows only from `program` — no conflict with `self.value_stack`
            let chunk = get_chunk(program, chunk_idx);
            if ip >= chunk.instructions.len() {
                // Fell off end of chunk — implicit return Unit
                if self.frame_stack.len() == 1 {
                    return Ok(());
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

                // ── Integer arithmetic ──────────────────────────────────────
                &Instruction::AddInt(dst, a, b) => {
                    let (va, vb) = (ri!(a), ri!(b));
                    wreg!(dst, Value::Int(va.wrapping_add(vb)));
                }
                &Instruction::SubInt(dst, a, b) => {
                    let (va, vb) = (ri!(a), ri!(b));
                    wreg!(dst, Value::Int(va.wrapping_sub(vb)));
                }
                &Instruction::MulInt(dst, a, b) => {
                    let (va, vb) = (ri!(a), ri!(b));
                    wreg!(dst, Value::Int(va.wrapping_mul(vb)));
                }
                &Instruction::DivInt(dst, a, b) => {
                    let (va, vb) = (ri!(a), ri!(b));
                    if vb == 0 { raise!(VmError::DivisionByZero); }
                    wreg!(dst, Value::Int(va / vb));
                }
                &Instruction::ModInt(dst, a, b) => {
                    let (va, vb) = (ri!(a), ri!(b));
                    if vb == 0 { raise!(VmError::DivisionByZero); }
                    wreg!(dst, Value::Int(va % vb));
                }
                &Instruction::PowInt(dst, a, b) => {
                    let (va, vb) = (ri!(a), ri!(b));
                    wreg!(dst, Value::Int(if vb < 0 { 0 } else { va.wrapping_pow(vb as u32) }));
                }
                &Instruction::NegInt(dst, src) => { let v = ri!(src); wreg!(dst, Value::Int(-v)); }

                // ── Integer immediate variants ──────────────────────────────
                &Instruction::AddIntImm(dst, src, imm) => { let v = ri!(src); wreg!(dst, Value::Int(v.wrapping_add(imm as i64))); }
                &Instruction::SubIntImm(dst, src, imm) => { let v = ri!(src); wreg!(dst, Value::Int(v.wrapping_sub(imm as i64))); }
                &Instruction::MulIntImm(dst, src, imm) => { let v = ri!(src); wreg!(dst, Value::Int(v.wrapping_mul(imm as i64))); }
                &Instruction::CmpEqImm(dst, src, imm) => { let v = ri!(src); wreg!(dst, Value::Bool(v == imm as i64)); }
                &Instruction::CmpNeImm(dst, src, imm) => { let v = ri!(src); wreg!(dst, Value::Bool(v != imm as i64)); }
                &Instruction::CmpLtImm(dst, src, imm) => { let v = ri!(src); wreg!(dst, Value::Bool(v  < imm as i64)); }
                &Instruction::CmpLeImm(dst, src, imm) => { let v = ri!(src); wreg!(dst, Value::Bool(v <= imm as i64)); }
                &Instruction::CmpGtImm(dst, src, imm) => { let v = ri!(src); wreg!(dst, Value::Bool(v  > imm as i64)); }
                &Instruction::CmpGeImm(dst, src, imm) => { let v = ri!(src); wreg!(dst, Value::Bool(v >= imm as i64)); }

                // ── Float arithmetic ────────────────────────────────────────
                &Instruction::AddFloat(dst, a, b) => { let (va, vb) = (rf!(a), rf!(b)); wreg!(dst, Value::Float(va + vb)); }
                &Instruction::SubFloat(dst, a, b) => { let (va, vb) = (rf!(a), rf!(b)); wreg!(dst, Value::Float(va - vb)); }
                &Instruction::MulFloat(dst, a, b) => { let (va, vb) = (rf!(a), rf!(b)); wreg!(dst, Value::Float(va * vb)); }
                &Instruction::DivFloat(dst, a, b) => { let (va, vb) = (rf!(a), rf!(b)); wreg!(dst, Value::Float(va / vb)); }
                &Instruction::PowFloat(dst, a, b) => { let (va, vb) = (rf!(a), rf!(b)); wreg!(dst, Value::Float(va.powf(vb))); }
                &Instruction::NegFloat(dst, src)  => { let v = rf!(src); wreg!(dst, Value::Float(-v)); }

                // ── Type conversion ─────────────────────────────────────────
                &Instruction::IntToFloat(dst, src) => { let v = ri!(src); wreg!(dst, Value::Float(v as f64)); }

                // ── String ops ──────────────────────────────────────────────
                &Instruction::ConcatStr(dst, a, b) => {
                    let result = match (rreg!(a), rreg!(b)) {
                        // Fast path: String ++ String — one alloc with known capacity
                        (Value::String(l), Value::String(r)) => {
                            let mut s = String::with_capacity(l.len() + r.len());
                            s.push_str(l.as_ref());
                            s.push_str(r.as_ref());
                            s
                        }
                        // Fast path: String ++ Int — avoid intermediate to_string alloc
                        (Value::String(l), Value::Int(n)) => {
                            let mut s = String::with_capacity(l.len() + 20);
                            s.push_str(l.as_ref());
                            use std::fmt::Write as _;
                            let _ = write!(s, "{}", n);
                            s
                        }
                        // Fast path: Int ++ String — avoid intermediate to_string alloc
                        (Value::Int(n), Value::String(r)) => {
                            let mut s = String::with_capacity(20 + r.len());
                            use std::fmt::Write as _;
                            let _ = write!(s, "{}", n);
                            s.push_str(r.as_ref());
                            s
                        }
                        // Fast path: String ++ Float
                        (Value::String(l), Value::Float(f)) => {
                            let mut s = String::with_capacity(l.len() + 24);
                            s.push_str(l.as_ref());
                            use std::fmt::Write as _;
                            let _ = write!(s, "{}", f);
                            s
                        }
                        // Slow path: anything else — convert both sides
                        (l, r) => {
                            let ls = l.to_string_repr();
                            let rs = r.to_string_repr();
                            let mut s = String::with_capacity(ls.len() + rs.len());
                            s.push_str(&ls);
                            s.push_str(&rs);
                            s
                        }
                    };
                    wreg!(dst, Value::String(Rc::new(result)));
                }
                &Instruction::StrLen(dst, src) => {
                    let n = match rreg!(src) {
                        Value::String(s)      => if s.is_ascii() { s.len() as i64 } else { s.chars().count() as i64 },
                        Value::Array(arr)     => arr.len() as i64,
                        other => raise!(VmError::TypeError { expected: "String or Array", got: other.type_name().to_string() }),
                    };
                    wreg!(dst, Value::Int(n));
                }

                // ── Comparison ──────────────────────────────────────────────
                &Instruction::CmpEq(dst, a, b) => { let r = cmp_direct(rreg!(a), rreg!(b)); wreg!(dst, Value::Bool(r == 0)); }
                &Instruction::CmpNe(dst, a, b) => { let r = cmp_direct(rreg!(a), rreg!(b)); wreg!(dst, Value::Bool(r != 0)); }
                &Instruction::CmpLt(dst, a, b) => { let r = cmp_direct(rreg!(a), rreg!(b)); wreg!(dst, Value::Bool(r  < 0)); }
                &Instruction::CmpLe(dst, a, b) => { let r = cmp_direct(rreg!(a), rreg!(b)); wreg!(dst, Value::Bool(r <= 0)); }
                &Instruction::CmpGt(dst, a, b) => { let r = cmp_direct(rreg!(a), rreg!(b)); wreg!(dst, Value::Bool(r  > 0)); }
                &Instruction::CmpGe(dst, a, b) => { let r = cmp_direct(rreg!(a), rreg!(b)); wreg!(dst, Value::Bool(r >= 0)); }

                // ── Logical ─────────────────────────────────────────────────
                &Instruction::And(dst, a, b) => { let (va, vb) = (rreg!(a).is_truthy(), rreg!(b).is_truthy()); wreg!(dst, Value::Bool(va && vb)); }
                &Instruction::Or (dst, a, b) => { let (va, vb) = (rreg!(a).is_truthy(), rreg!(b).is_truthy()); wreg!(dst, Value::Bool(va || vb)); }
                &Instruction::Not(dst, src)  => { let v = rreg!(src).is_truthy(); wreg!(dst, Value::Bool(!v)); }

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
                    let num_args = arg_regs.len();

                    // Save current IP to caller frame before pushing callee
                    self.frame_stack.last_mut().unwrap().ip = ip as u32;

                    let new_base = self.value_stack.len();
                    // Extend flat stack: initialize only non-arg slots to Unit;
                    // arg slots are written immediately after, avoiding double-write.
                    self.value_stack.resize(new_base + num_regs, Value::Unit);

                    // Copy args from caller into callee arg registers
                    for (i, &reg) in arg_regs.iter().enumerate() {
                        let val = unsafe { self.value_stack.get_unchecked(base + reg as usize).clone() };
                        unsafe { *self.value_stack.get_unchecked_mut(new_base + i) = val; }
                    }
                    let _ = num_args; // used above

                    let wb = mem::take(&mut self.pending_output_writeback);
                    self.frame_stack.push(FrameInfo {
                        base: new_base as u32,
                        ip: 0,
                        chunk_idx: func_idx as u32,
                        return_reg: dst as u16,
                        catch_ip: u32::MAX,
                        try_depth: 0,
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

                    // Update frame metadata (same base, new chunk)
                    let frame = self.frame_stack.last_mut().unwrap();
                    frame.chunk_idx = func_idx as u32;

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
                        return Ok(());
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
                        other             => write!(self.output, "{}", other)?,
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
                        other => raise!(VmError::TypeError { expected: "Array", got: other.type_name().to_string() }),
                    }
                }
                &Instruction::ArrayGet(dst, arr_reg, idx_reg) => {
                    let idx = self.as_int(idx_reg)?;
                    let val = match &self.value_stack[base + arr_reg as usize] {
                        Value::Array(arr) => {
                            let i = if idx < 0 { arr.len() as i64 + idx } else { idx };
                            if i < 0 || i as usize >= arr.len() {
                                raise!(VmError::IndexOutOfBounds { index: idx, length: arr.len() });
                            }
                            arr[i as usize].clone()
                        }
                        Value::Tuple(items) => {
                            let i = if idx < 0 { items.len() as i64 + idx } else { idx };
                            if i < 0 || i as usize >= items.len() {
                                raise!(VmError::IndexOutOfBounds { index: idx, length: items.len() });
                            }
                            items[i as usize].clone()
                        }
                        Value::NamedTuple(fields) => {
                            let i = if idx < 0 { fields.len() as i64 + idx } else { idx };
                            if i < 0 || i as usize >= fields.len() {
                                raise!(VmError::IndexOutOfBounds { index: idx, length: fields.len() });
                            }
                            fields[i as usize].1.clone()
                        }
                        Value::String(s) => {
                            // Single-pass: find the i-th char without collecting Vec<char>
                            let char_count = s.chars().count();
                            let i = if idx < 0 { char_count as i64 + idx } else { idx };
                            if i < 0 || i as usize >= char_count {
                                raise!(VmError::IndexOutOfBounds { index: idx, length: char_count });
                            }
                            let ch = s.chars().nth(i as usize).unwrap();
                            Value::Char(ch)
                        }
                        other => raise!(VmError::TypeError { expected: "Array", got: other.type_name().to_string() }),
                    };
                    self.reg_set(dst, val);
                }
                &Instruction::ArraySet(arr_reg, idx_reg, val_reg) => {
                    let idx = self.as_int(idx_reg)?;
                    let val = self.reg_get(val_reg).clone();
                    match &mut self.value_stack[base + arr_reg as usize] {
                        Value::Array(rc_arr) => {
                            let arr = Rc::make_mut(rc_arr);
                            let i = if idx < 0 { arr.len() as i64 + idx } else { idx };
                            if i < 0 || i as usize >= arr.len() {
                                raise!(VmError::IndexOutOfBounds { index: idx, length: arr.len() });
                            }
                            arr[i as usize] = val;
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
                        other => raise!(VmError::TypeError { expected: "Array or String", got: other.type_name().to_string() }),
                    };
                    self.reg_set(dst, Value::Int(n));
                }
                &Instruction::ArrayRemove(arr_reg, idx_reg) => {
                    let idx = self.as_int(idx_reg)?;
                    let result = match self.value_stack[base + arr_reg as usize].clone() {
                        Value::Array(rc_arr) => {
                            let mut arr = rc_arr.as_ref().clone();
                            let i = if idx < 0 { arr.len() as i64 + idx } else { idx };
                            if i < 0 || i as usize >= arr.len() {
                                raise!(VmError::IndexOutOfBounds { index: idx, length: arr.len() });
                            }
                            arr.remove(i as usize);
                            Value::Array(Rc::new(arr))
                        }
                        Value::Tuple(rc_tup) => {
                            let mut tup = rc_tup.as_ref().clone();
                            let i = if idx < 0 { tup.len() as i64 + idx } else { idx };
                            if i < 0 || i as usize >= tup.len() {
                                raise!(VmError::IndexOutOfBounds { index: idx, length: tup.len() });
                            }
                            tup.remove(i as usize);
                            Value::Tuple(Rc::new(tup))
                        }
                        Value::NamedTuple(rc_fields) => {
                            let mut fields = rc_fields.as_ref().clone();
                            let i = if idx < 0 { fields.len() as i64 + idx } else { idx };
                            if i < 0 || i as usize >= fields.len() {
                                raise!(VmError::IndexOutOfBounds { index: idx, length: fields.len() });
                            }
                            fields.remove(i as usize);
                            Value::NamedTuple(Rc::new(fields))
                        }
                        Value::String(rc_s) => {
                            let mut chars: Vec<char> = rc_s.chars().collect();
                            let i = if idx < 0 { chars.len() as i64 + idx } else { idx };
                            if i < 0 || i as usize >= chars.len() {
                                raise!(VmError::IndexOutOfBounds { index: idx, length: chars.len() });
                            }
                            chars.remove(i as usize);
                            Value::String(Rc::new(chars.iter().collect()))
                        }
                        other => raise!(VmError::TypeError { expected: "Array, Tuple, or String", got: other.type_name().to_string() }),
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
                                    } else { rc_s.as_ref().clone() }
                                }
                                Value::String(p) => {
                                    let s = rc_s.as_ref();
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
                                _ => raise!(VmError::TypeError { expected: "Char or String", got: val.type_name().to_string() }),
                            };
                            self.value_stack[base + arr_reg as usize] = Value::String(Rc::new(result));
                        }
                        other => raise!(VmError::TypeError { expected: "Array or String", got: other.type_name().to_string() }),
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
                                    if p.is_empty() { rc_s.as_ref().clone() }
                                    else { rc_s.replace(p.as_str(), "") }
                                }
                                _ => raise!(VmError::TypeError { expected: "Char or String", got: val.type_name().to_string() }),
                            };
                            self.value_stack[base + arr_reg as usize] = Value::String(Rc::new(result));
                        }
                        other => raise!(VmError::TypeError { expected: "Array, Tuple, or String", got: other.type_name().to_string() }),
                    }
                }

                &Instruction::ArrayInsert(arr_reg, idx_reg, val_reg) => {
                    let idx = self.as_int(idx_reg)?;
                    let val = self.reg_get(val_reg).clone();
                    match self.value_stack[base + arr_reg as usize].clone() {
                        Value::Array(rc_arr) => {
                            let mut arr = rc_arr.as_ref().clone();
                            if idx < 0 || idx as usize > arr.len() {
                                raise!(VmError::IndexOutOfBounds { index: idx, length: arr.len() });
                            }
                            arr.insert(idx as usize, val);
                            self.value_stack[base + arr_reg as usize] = Value::Array(Rc::new(arr));
                        }
                        Value::Tuple(rc_tup) => {
                            let mut tup = rc_tup.as_ref().clone();
                            if idx < 0 || idx as usize > tup.len() {
                                raise!(VmError::IndexOutOfBounds { index: idx, length: tup.len() });
                            }
                            tup.insert(idx as usize, val);
                            self.value_stack[base + arr_reg as usize] = Value::Tuple(Rc::new(tup));
                        }
                        Value::String(rc_s) => {
                            let mut chars: Vec<char> = rc_s.chars().collect();
                            let i = idx as usize;
                            if idx < 0 || i > chars.len() {
                                raise!(VmError::IndexOutOfBounds { index: idx, length: chars.len() });
                            }
                            match val {
                                Value::Char(c) => { chars.insert(i, c); }
                                Value::String(ref ins) => {
                                    for (j, c) in ins.chars().enumerate() { chars.insert(i + j, c); }
                                }
                                _ => raise!(VmError::TypeError { expected: "Char or String", got: val.type_name().to_string() }),
                            }
                            self.value_stack[base + arr_reg as usize] = Value::String(Rc::new(chars.iter().collect()));
                        }
                        other => raise!(VmError::TypeError { expected: "Array, Tuple, or String", got: other.type_name().to_string() }),
                    }
                }

                &Instruction::ArrayRemoveRange(arr_reg, lo_reg) => {
                    // hi_reg = lo_reg + 1 by compiler convention
                    let lo = self.as_int(lo_reg)? as usize;
                    let hi = self.as_int(lo_reg + 1)? as usize;
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
                            let mut fields = rc_nt.as_ref().clone();
                            if lo <= hi && hi <= fields.len() {
                                fields.drain(lo..hi);
                            }
                            self.value_stack[base + arr_reg as usize] = Value::NamedTuple(Rc::new(fields));
                        }
                        Value::String(rc_s) => {
                            let mut chars: Vec<char> = rc_s.chars().collect();
                            if lo <= hi && hi <= chars.len() {
                                chars.drain(lo..hi);
                            }
                            self.value_stack[base + arr_reg as usize] = Value::String(Rc::new(chars.iter().collect()));
                        }
                        other => raise!(VmError::TypeError { expected: "Array, Tuple, NamedTuple, or String", got: other.type_name().to_string() }),
                    }
                }

                // ── Pattern match ────────────────────────────────────────────
                &Instruction::MatchInt(reg, val, label) => {
                    let v = self.as_int(reg)?;
                    if v == val { ip = label as usize; }
                }
                &Instruction::MatchRange(reg, lo, hi, label) => {
                    let v = self.as_int(reg)?;
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
                    self.reg_set(dst, Value::Function(func_idx));
                }
                Instruction::MakeClosure(dst, func_idx, captured_regs) => {
                    let (dst, func_idx) = (*dst, *func_idx);
                    let upvalues: Vec<Value> = captured_regs.iter()
                        .map(|&r| self.reg_get(r).clone())
                        .collect();
                    self.reg_set(dst, Value::Closure(func_idx, Rc::new(upvalues)));
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
                                s.split(c).map(|p| Value::String(Rc::new(p.to_string()))).collect()
                            }
                            (Value::String(s), Value::String(sep)) => {
                                let sep = sep.clone(); // Rc clone only
                                s.split(sep.as_str()).map(|p| Value::String(Rc::new(p.to_string()))).collect()
                            }
                            (Value::String(_), other) => raise!(VmError::TypeError { expected: "Char or String", got: other.type_name().to_string() }),
                            (other, _) => raise!(VmError::TypeError { expected: "String", got: other.type_name().to_string() }),
                        }
                    };
                    self.reg_set(dst, Value::Array(Rc::new(parts)));
                }
                &Instruction::StrContains(dst, str_reg, elem_reg) => {
                    let result = {
                        let s_val = &self.value_stack[base + str_reg as usize];
                        let e_val = &self.value_stack[base + elem_reg as usize];
                        match (s_val, e_val) {
                            (Value::String(s), Value::Char(c))    => s.contains(*c),
                            (Value::String(s), Value::String(sub)) => s.contains(sub.as_str()),
                            (Value::String(_), _) => false,
                            (other, _) => raise!(VmError::TypeError { expected: "String", got: other.type_name().to_string() }),
                        }
                    };
                    wreg!(dst, Value::Bool(result));
                }
                &Instruction::StrSlice(dst, str_reg, lo_reg) => {
                    let lo_val = self.as_int(lo_reg)?;
                    let hi_val = self.as_int(lo_reg + 1)?;
                    let result = match &self.value_stack[base + str_reg as usize] {
                        Value::String(s) => {
                            if s.is_ascii() {
                                // Fast path: byte indices == char indices
                                let len = s.len() as i64;
                                let lo = (if lo_val < 0 { len + lo_val } else { lo_val }).max(0).min(len) as usize;
                                let hi = (if hi_val < 0 { len + hi_val } else { hi_val }).max(0).min(len) as usize;
                                s[lo..hi].to_string()
                            } else {
                                // Unicode: single-pass via char_indices to find byte offsets
                                let char_len = s.chars().count() as i64;
                                let lo = (if lo_val < 0 { char_len + lo_val } else { lo_val }).max(0).min(char_len) as usize;
                                let hi = (if hi_val < 0 { char_len + hi_val } else { hi_val }).max(0).min(char_len) as usize;
                                let mut byte_lo = s.len();
                                let mut byte_hi = s.len();
                                for (ci, (bi, _)) in s.char_indices().enumerate() {
                                    if ci == lo { byte_lo = bi; }
                                    if ci == hi { byte_hi = bi; break; }
                                }
                                s[byte_lo..byte_hi].to_string()
                            }
                        }
                        other => raise!(VmError::TypeError { expected: "String", got: other.type_name().to_string() }),
                    };
                    self.reg_set(dst, Value::String(Rc::new(result)));
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
                        Value::NamedTuple(nt)   => Value::NamedTuple(nt.clone()),
                        other => raise!(VmError::TypeError {
                            expected: "String or Array",
                            got: other.type_name().to_string(),
                        }),
                    };
                    wreg!(dst, val);
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
                                        .map(|(i, _)| Value::Int(i as i64))
                                        .collect()
                                } else {
                                    s.char_indices()
                                        .filter(|(_, ch)| *ch == c)
                                        .enumerate()
                                        .map(|(ci, _)| Value::Int(ci as i64))
                                        .collect()
                                }
                            }
                            (Value::String(s), Value::String(pat)) => {
                                let pat = pat.clone();
                                if s.is_ascii() {
                                    // ASCII fast path: byte_offset == char_offset
                                    s.match_indices(pat.as_str())
                                        .map(|(bi, _)| Value::Int(bi as i64))
                                        .collect()
                                } else {
                                    // Unicode: build char-index map: byte_offset → char_index
                                    let char_idx: std::collections::HashMap<usize, usize> =
                                        s.char_indices().enumerate().map(|(ci, (bi, _))| (bi, ci)).collect();
                                    s.match_indices(pat.as_str())
                                        .filter_map(|(bi, _)| char_idx.get(&bi).map(|&ci| Value::Int(ci as i64)))
                                        .collect()
                                }
                            }
                            (Value::Array(arr), needle) => {
                                let needle = needle.clone();
                                arr.iter().enumerate()
                                    .filter(|(_, v)| v.equals(&needle))
                                    .map(|(i, _)| Value::Int(i as i64))
                                    .collect()
                            }
                            (Value::Tuple(tup), needle) => {
                                let needle = needle.clone();
                                tup.iter().enumerate()
                                    .filter(|(_, v)| v.equals(&needle))
                                    .map(|(i, _)| Value::Int(i as i64))
                                    .collect()
                            }
                            (Value::String(_), other) => raise!(VmError::TypeError { expected: "Char or String", got: other.type_name().to_string() }),
                            (other, _) => raise!(VmError::TypeError { expected: "String, Array, or Tuple", got: other.type_name().to_string() }),
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
                        if s.is_ascii() {
                            let p = (pos.max(0) as usize).min(s.len());
                            let mut r = String::with_capacity(s.len() + text.len());
                            r.push_str(&s[..p]);
                            r.push_str(&text);
                            r.push_str(&s[p..]);
                            r
                        } else {
                            let char_len = s.chars().count() as i64;
                            let p = (pos.max(0).min(char_len)) as usize;
                            let byte_pos = s.char_indices().nth(p).map(|(bi, _)| bi).unwrap_or(s.len());
                            let mut r = String::with_capacity(s.len() + text.len());
                            r.push_str(&s[..byte_pos]);
                            r.push_str(&text);
                            r.push_str(&s[byte_pos..]);
                            r
                        }
                    };
                    wreg!(dst, Value::String(Rc::new(result)));
                }
                &Instruction::StrRemove(dst, str_reg, pos_reg, count_reg) => {
                    let result = {
                        let s = match &self.value_stack[base + str_reg as usize] {
                            Value::String(s) => s.clone(),
                            other => raise!(VmError::TypeError { expected: "String", got: other.type_name().to_string() }),
                        };
                        let pos   = ri!(pos_reg).max(0) as usize;
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
                    wreg!(dst, Value::String(Rc::new(result)));
                }
                &Instruction::StrReplace(dst, str_reg, pat_reg, rep_reg) => {
                    let result = {
                        let s = match &self.value_stack[base + str_reg as usize] {
                            Value::String(s) => s.clone(),
                            other => raise!(VmError::TypeError { expected: "String", got: other.type_name().to_string() }),
                        };
                        let rep = match &self.value_stack[base + rep_reg as usize] {
                            Value::String(r) => r.clone(),
                            other => raise!(VmError::TypeError { expected: "String", got: other.type_name().to_string() }),
                        };
                        match &self.value_stack[base + pat_reg as usize] {
                            Value::String(pat) => s.replace(pat.as_str(), rep.as_str()),
                            Value::Char(c) => {
                                let mut ps = String::with_capacity(4);
                                ps.push(*c);
                                s.replace(ps.as_str(), rep.as_str())
                            }
                            other => raise!(VmError::TypeError { expected: "Char or String", got: other.type_name().to_string() }),
                        }
                    };
                    wreg!(dst, Value::String(Rc::new(result)));
                }
                &Instruction::StrReplaceN(dst, str_reg, pat_reg, rep_reg, n_reg) => {
                    let result = {
                        let s = match &self.value_stack[base + str_reg as usize] {
                            Value::String(s) => s.clone(),
                            other => raise!(VmError::TypeError { expected: "String", got: other.type_name().to_string() }),
                        };
                        let rep = match &self.value_stack[base + rep_reg as usize] {
                            Value::String(r) => r.clone(),
                            other => raise!(VmError::TypeError { expected: "String", got: other.type_name().to_string() }),
                        };
                        let max = ri!(n_reg).max(0) as usize;
                        let pat_str = match &self.value_stack[base + pat_reg as usize] {
                            Value::String(p) => p.to_string(),
                            Value::Char(c) => c.to_string(),
                            other => raise!(VmError::TypeError { expected: "Char or String", got: other.type_name().to_string() }),
                        };
                        if max == 0 {
                            s.replace(pat_str.as_str(), rep.as_str())
                        } else {
                            let mut out = String::with_capacity(s.len());
                            let mut remaining = s.as_str();
                            let mut count = 0;
                            while count < max {
                                if let Some(pos) = remaining.find(pat_str.as_str()) {
                                    out.push_str(&remaining[..pos]);
                                    out.push_str(&rep);
                                    remaining = &remaining[pos + pat_str.len()..];
                                    count += 1;
                                } else {
                                    break;
                                }
                            }
                            out.push_str(remaining);
                            out
                        }
                    };
                    wreg!(dst, Value::String(Rc::new(result)));
                }

                Instruction::BuildStr(dst, parts) => {
                    let dst = *dst;
                    let mut result = String::new();
                    for part in parts {
                        match part {
                            BuildPart::Lit(idx) => result.push_str(&program.string_pool[*idx as usize]),
                            BuildPart::Reg(r) => result.push_str(&self.reg_get(*r).to_string_repr()),
                        }
                    }
                    self.reg_set(dst, Value::String(Rc::new(result)));
                }

                // ── Dynamic call (lambdas / closures stored in variables) ────
                Instruction::CallDynamic(dst, callee_reg, arg_regs) => {
                    let (dst, callee_reg) = (*dst, *callee_reg);
                    let callable = unsafe { self.value_stack.get_unchecked(base + callee_reg as usize).clone() };

                    let (func_idx, upvalues): (FuncIdx, Vec<Value>) = match callable {
                        Value::Function(idx) => (idx, Vec::new()),
                        Value::Closure(idx, uvs) => (idx, uvs.as_ref().clone()),
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
                        chunk_idx: func_idx as u32,
                        return_reg: dst as u16,
                        catch_ip: u32::MAX,
                        try_depth: 0,
                        error: None,
                        writeback: if wb.is_empty() { None } else { Some(Box::new(wb)) },
                    });

                    base = new_base;
                    ip = 0;
                    chunk_idx = func_idx as usize;
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
                        other => raise!(VmError::TypeError { expected: "Array or String", got: other.type_name().to_string() }),
                    };
                    self.reg_set(dst, Value::Bool(result));
                }
                &Instruction::ArraySlice(dst, arr_reg, lo_reg) => {
                    // hi_reg = lo_reg + 1 by compiler convention
                    let lo = self.as_int(lo_reg)?;
                    let hi = self.as_int(lo_reg + 1)?;
                    let result = match self.reg_get(arr_reg) {
                        Value::Array(arr) => {
                            let arr = arr.as_ref();
                            let len = arr.len() as i64;
                            let lo = (if lo < 0 { len + lo } else { lo }).max(0) as usize;
                            let hi = (if hi < 0 { len + hi } else { hi }).min(len) as usize;
                            let lo = lo.min(arr.len());
                            let hi = hi.min(arr.len());
                            Value::Array(Rc::new(arr[lo..hi].to_vec()))
                        }
                        Value::Tuple(tup) => {
                            let tup = tup.as_ref();
                            let len = tup.len() as i64;
                            let lo = (if lo < 0 { len + lo } else { lo }).max(0) as usize;
                            let hi = (if hi < 0 { len + hi } else { hi }).min(len) as usize;
                            let lo = lo.min(tup.len());
                            let hi = hi.min(tup.len());
                            Value::Tuple(Rc::new(tup[lo..hi].to_vec()))
                        }
                        Value::NamedTuple(fields) => {
                            let fields = fields.as_ref();
                            let len = fields.len() as i64;
                            let lo = (if lo < 0 { len + lo } else { lo }).max(0) as usize;
                            let hi = (if hi < 0 { len + hi } else { hi }).min(len) as usize;
                            let lo = lo.min(fields.len());
                            let hi = hi.min(fields.len());
                            Value::NamedTuple(Rc::new(fields[lo..hi].to_vec()))
                        }
                        other => raise!(VmError::TypeError { expected: "Array, Tuple, or NamedTuple", got: other.type_name().to_string() }),
                    };
                    self.reg_set(dst, result);
                }
                &Instruction::ArrayMap(dst, arr_reg, func_reg) => {
                    let callable = self.reg_get(func_reg).clone();
                    let arr = match self.reg_get(arr_reg).clone() {
                        Value::Array(a) => a.as_ref().clone(),
                        other => raise!(VmError::TypeError { expected: "Array", got: other.type_name().to_string() }),
                    };
                    let mut results = Vec::with_capacity(arr.len());
                    for elem in arr {
                        let result = self.call_callable(callable.clone(), vec![elem], program, ip, chunk_idx)?;
                        results.push(result);
                    }
                    self.reg_set(dst, Value::Array(Rc::new(results)));
                }
                &Instruction::ArrayFilter(dst, arr_reg, func_reg) => {
                    let callable = self.reg_get(func_reg).clone();
                    let arr = match self.reg_get(arr_reg).clone() {
                        Value::Array(a) => a.as_ref().clone(),
                        other => raise!(VmError::TypeError { expected: "Array", got: other.type_name().to_string() }),
                    };
                    let mut results = Vec::new();
                    for elem in arr {
                        let keep = self.call_callable(callable.clone(), vec![elem.clone()], program, ip, chunk_idx)?;
                        if keep.is_truthy() {
                            results.push(elem);
                        }
                    }
                    self.reg_set(dst, Value::Array(Rc::new(results)));
                }
                &Instruction::ArrayReduce(dst, arr_reg, init_reg, func_reg) => {
                    let callable = self.reg_get(func_reg).clone();
                    let arr = match self.reg_get(arr_reg).clone() {
                        Value::Array(a) => a.as_ref().clone(),
                        other => raise!(VmError::TypeError { expected: "Array", got: other.type_name().to_string() }),
                    };
                    let mut acc = self.reg_get(init_reg).clone();
                    for elem in arr {
                        acc = self.call_callable(callable.clone(), vec![acc, elem], program, ip, chunk_idx)?;
                    }
                    self.reg_set(dst, acc);
                }
                &Instruction::ArraySort(dst, arr_reg, ascending, func_reg) => {
                    let arr = match self.reg_get(arr_reg).clone() {
                        Value::Array(a) => a.as_ref().clone(),
                        other => raise!(VmError::TypeError { expected: "Array", got: other.type_name().to_string() }),
                    };
                    let mut items = arr;
                    if func_reg == u16::MAX {
                        // Natural order
                        items.sort_by(|a, b| vm_natural_cmp(a, b));
                        if !ascending {
                            items.reverse();
                        }
                    } else {
                        // Custom comparator: bubble sort to avoid unsafe borrow
                        let callable = self.reg_get(func_reg).clone();
                        let n = items.len();
                        for i in 0..n {
                            for j in 0..n.saturating_sub(i + 1) {
                                let keep = self.call_callable(
                                    callable.clone(),
                                    vec![items[j].clone(), items[j + 1].clone()],
                                    program, ip, chunk_idx,
                                )?;
                                if !keep.is_truthy() {
                                    items.swap(j, j + 1);
                                }
                            }
                        }
                    }
                    self.reg_set(dst, Value::Array(Rc::new(items)));
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
                &Instruction::NamedTupleGet(dst, tuple_reg, field_idx) => {
                    let field_name = &program.string_pool[field_idx as usize];
                    let result = match self.reg_get(tuple_reg) {
                        Value::NamedTuple(fields) => {
                            let field_name = field_name.clone();
                            let available: Vec<String> = fields.iter().map(|(n, _)| n.clone()).collect();
                            fields.iter().find(|(n, _)| *n == field_name)
                                .map(|(_, v)| v.clone())
                                .ok_or_else(|| VmError::Generic(format!(
                                    "runtime error: Named tuple has no field '{}'. Available fields: {}",
                                    field_name, available.join(", ")
                                )))?
                        }
                        // positional index on array (tuple[0])
                        Value::Array(arr) => {
                            if let Ok(i) = field_name.parse::<usize>() {
                                arr.get(i).cloned().unwrap_or(Value::Unit)
                            } else {
                                raise!(VmError::TypeError { expected: "Int index", got: field_name.clone() });
                            }
                        }
                        Value::Tuple(_) => {
                            let field_name = field_name.clone();
                            raise!(VmError::Generic(format!(
                                "runtime error: Cannot access field '{}' on positional tuple. Use positional indexing like tuple[0]",
                                field_name
                            )));
                        }
                        other => raise!(VmError::TypeError { expected: "Tuple", got: other.type_name().to_string() }),
                    };
                    self.reg_set(dst, result);
                }

                // ── Data ops ─────────────────────────────────────────────────
                &Instruction::NumericEval(dst, src) => {
                    let result = match self.reg_get(src) {
                        Value::String(s) => {
                            let s_rc = s.clone();
                            let s_str = s_rc.as_ref().trim();
                            if let Ok(i) = s_str.parse::<i64>() {
                                Value::Int(i)
                            } else if let Ok(f) = s_str.parse::<f64>() {
                                Value::Float(f)
                            } else {
                                Value::String(s_rc)
                            }
                        }
                        Value::Int(n) => Value::Int(*n),
                        Value::Float(f) => Value::Float(*f),
                        other => other.clone(),
                    };
                    self.reg_set(dst, result);
                }
                &Instruction::TypeOf(dst, src) => {
                    let val = self.reg_get(src).clone();
                    let (type_sym, len) = match &val {
                        Value::Int(n) => ("###", n.to_string().len() as i64),
                        Value::Float(fl) => ("##.", fl.to_string().len() as i64),
                        Value::String(s) => ("##\"", s.as_ref().chars().count() as i64),
                        Value::Char(_) => ("##'", 1),
                        Value::Bool(_) => ("##?", 1),
                        Value::Array(a) => ("##]", a.as_ref().len() as i64),
                        Value::Tuple(t) => ("##)", t.as_ref().len() as i64),
                        Value::NamedTuple(f) => ("##)", f.as_ref().len() as i64),
                        Value::Function(_) | Value::Closure(_, _) => ("##>", 0),
                        Value::Unit => ("##_", 0),
                    };
                    let tuple_val = Value::Tuple(Rc::new(vec![
                        Value::String(Rc::new(type_sym.to_string())),
                        Value::Int(len),
                        val,
                    ]));
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
                            Value::String(Rc::new(s))
                        }
                        // Int → String (format integer in specified base)
                        Value::Int(n) => {
                            let s = match radix {
                                2  => format!("0b{:b}", n),
                                8  => format!("0o{:o}", n),
                                10 => format!("0d{:04}", n),
                                _  => format!("0x{:04X}", n),
                            };
                            Value::String(Rc::new(s))
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
                        other => raise!(VmError::TypeError {
                            expected: "Char, Int, or String",
                            got: other.type_name().to_string(),
                        }),
                    };
                    self.reg_set(dst, result);
                }

                // ── Try/catch ─────────────────────────────────────────────────
                &Instruction::TryBegin(catch_label) => {
                    let frame = self.frame_stack.last_mut().unwrap();
                    frame.catch_ip = catch_label as u32;
                    frame.try_depth += 1;
                }
                Instruction::TryEnd(_) => {
                    let frame = self.frame_stack.last_mut().unwrap();
                    if frame.try_depth > 0 { frame.try_depth -= 1; }
                    if frame.try_depth == 0 { frame.catch_ip = u32::MAX; }
                }
                &Instruction::TryCatch(err_reg) => {
                    let err = self.frame_stack.last_mut().unwrap()
                        .error.as_mut()
                        .and_then(|e| e.error_val.take())
                        .unwrap_or_else(|| Value::String(Rc::new("unknown error".to_string())));
                    unsafe { *self.value_stack.get_unchecked_mut(base + err_reg as usize) = err; }
                }

                // ── Shell execution ───────────────────────────────────────────
                Instruction::BashExec(dst, parts) => {
                    let dst = *dst;
                    let mut cmd = String::new();
                    for part in parts {
                        match part {
                            BuildPart::Lit(idx) => cmd.push_str(&program.string_pool[*idx as usize]),
                            BuildPart::Reg(r) => cmd.push_str(&self.reg_get(*r).to_string_repr()),
                        }
                    }
                    let out = std::process::Command::new("sh")
                        .arg("-c")
                        .arg(&cmd)
                        .output()
                        .map_err(VmError::Io)?;
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
                    self.reg_set(dst, Value::String(Rc::new(result)));
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
                    let out = std::process::Command::new("sh")
                        .arg("-c")
                        .arg(&cmd)
                        .output()
                        .map_err(VmError::Io)?;
                    if !out.status.success() {
                        let mut msg = String::from_utf8_lossy(&out.stderr).into_owned();
                        if msg.is_empty() {
                            msg = String::from_utf8_lossy(&out.stdout).into_owned();
                        }
                        let msg = msg.trim_end().to_string();
                        return Err(VmError::Generic(format!("runtime error: {}", msg)));
                    }
                    let result = String::from_utf8_lossy(&out.stdout).into_owned();
                    self.reg_set(dst, Value::String(Rc::new(result)));
                }

                // ── Format ops ────────────────────────────────────────────────
                &Instruction::FmtThousands(dst, src, prec_kind, prec_n) => {
                    let f = match self.reg_get(src) {
                        Value::Int(n) => *n as f64,
                        Value::Float(f) => *f,
                        other => other.to_string().parse::<f64>().unwrap_or(0.0),
                    };
                    let s = vm_fmt_thousands(f, prec_kind, prec_n);
                    self.reg_set(dst, Value::String(Rc::new(s)));
                }
                &Instruction::FmtScientific(dst, src, prec_kind, prec_n) => {
                    let f = match self.reg_get(src) {
                        Value::Int(n) => *n as f64,
                        Value::Float(f) => *f,
                        other => other.to_string().parse::<f64>().unwrap_or(0.0),
                    };
                    let s = vm_fmt_scientific(f, prec_kind, prec_n);
                    self.reg_set(dst, Value::String(Rc::new(s)));
                }

                // ── Precision ops ─────────────────────────────────────────────
                &Instruction::RoundFloat(dst, src, precision) => {
                    let f = match self.reg_get(src) {
                        Value::Int(n) => *n as f64,
                        Value::Float(f) => *f,
                        Value::String(s) => s.as_ref().trim().parse::<f64>().unwrap_or(0.0),
                        other => raise!(VmError::TypeError { expected: "number", got: other.type_name().to_string() }),
                    };
                    let m = 10_f64.powi(precision as i32);
                    self.reg_set(dst, Value::Float((f * m).round() / m));
                }
                &Instruction::TruncFloat(dst, src, precision) => {
                    let f = match self.reg_get(src) {
                        Value::Int(n) => *n as f64,
                        Value::Float(f) => *f,
                        Value::String(s) => s.as_ref().trim().parse::<f64>().unwrap_or(0.0),
                        other => raise!(VmError::TypeError { expected: "number", got: other.type_name().to_string() }),
                    };
                    let m = 10_f64.powi(precision as i32);
                    self.reg_set(dst, Value::Float((f * m).trunc() / m));
                }

                // ── Error check ───────────────────────────────────────────────
                &Instruction::IsError(dst, _src) => {
                    // In the VM, error values never exist in registers (errors are caught by
                    // try/catch and stored in _err as String). So $! always returns #0 for
                    // any value that can appear in a register.
                    self.reg_set(dst, Value::Bool(false));
                }
                &Instruction::LoadErrorKind(dst) => {
                    let kind = self.frame_stack.last()
                        .and_then(|f| f.error.as_ref())
                        .map(|e| e.error_kind.clone())
                        .unwrap_or_default();
                    unsafe { *self.value_stack.get_unchecked_mut(base + dst as usize) = Value::String(Rc::new(kind)); }
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

                Instruction::Halt => return Ok(()),
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

    /// Dispatch a call to either a Function or a Closure value.
    /// Used by HOF opcodes (ArrayMap, ArrayFilter, ArrayReduce).
    fn call_callable(
        &mut self,
        callable: Value,
        args: Vec<Value>,
        program: &CompiledProgram,
        caller_ip: usize,
        caller_chunk: usize,
    ) -> Result<Value, VmError> {
        match callable {
            Value::Function(idx) => self.call_function(idx, args, &[], program, caller_ip, caller_chunk),
            Value::Closure(idx, upvalues) => self.call_function(idx, args, upvalues.as_ref(), program, caller_ip, caller_chunk),
            other => Err(VmError::TypeError { expected: "Function", got: other.type_name().to_string() }),
        }
    }

    fn call_function(
        &mut self,
        func_idx: FuncIdx,
        args: Vec<Value>,
        upvalues: &[Value],
        program: &CompiledProgram,
        _caller_ip: usize,
        _caller_chunk: usize,
    ) -> Result<Value, VmError> {
        if func_idx as usize >= program.functions.len() {
            return Err(VmError::UndefinedFunction(func_idx));
        }
        let chunk = &program.functions[func_idx as usize];
        let num_params = chunk.num_params as usize;
        let num_regs = chunk.num_registers as usize;

        // Extend the flat stack for this function's registers
        let base = self.value_stack.len();
        self.value_stack.resize(base + num_regs, Value::Unit);

        // Write args
        for (i, v) in args.into_iter().enumerate() {
            if i < num_regs { self.value_stack[base + i] = v; }
        }
        // Load upvalues into [num_params..num_params+k)
        for (i, uv) in upvalues.iter().enumerate() {
            let slot = num_params + i;
            if slot < num_regs { self.value_stack[base + slot] = uv.clone(); }
        }

        let mut ip = 0usize;
        let chunk_idx = func_idx as usize;
        loop {
            let chunk = &program.functions[chunk_idx];
            if ip >= chunk.instructions.len() { break; }
            let instr = &chunk.instructions[ip];
            ip += 1;
            macro_rules! r { ($r:expr) => { &self.value_stack[base + $r as usize] } }
            macro_rules! w { ($r:expr, $v:expr) => { self.value_stack[base + $r as usize] = $v } }
            match instr {
                &Instruction::Return(src) => {
                    let result = mem::replace(&mut self.value_stack[base + src as usize], Value::Unit);
                    self.value_stack.truncate(base);
                    return Ok(result);
                }
                &Instruction::Halt => break,
                &Instruction::LoadInt(dst, n) => w!(dst, Value::Int(n)),
                &Instruction::LoadFloat(dst, n) => w!(dst, Value::Float(n)),
                &Instruction::LoadBool(dst, b) => w!(dst, Value::Bool(b)),
                &Instruction::LoadStr(dst, idx) => w!(dst, Value::String(self.string_rcs[idx as usize].clone())),
                &Instruction::LoadChar(dst, c) => w!(dst, Value::Char(c)),
                &Instruction::LoadUnit(dst) => w!(dst, Value::Unit),
                &Instruction::CopyReg(dst, src) => { let v = r!(src).clone(); w!(dst, v); }
                &Instruction::MoveReg(dst, src) => {
                    let v = mem::replace(&mut self.value_stack[base + src as usize], Value::Unit);
                    w!(dst, v);
                }
                &Instruction::AddInt(dst, a, b) => {
                    if let (Value::Int(va), Value::Int(vb)) = (r!(a), r!(b)) {
                        let res = va.wrapping_add(*vb); w!(dst, Value::Int(res));
                    }
                }
                &Instruction::SubInt(dst, a, b) => {
                    if let (Value::Int(va), Value::Int(vb)) = (r!(a), r!(b)) {
                        let res = va.wrapping_sub(*vb); w!(dst, Value::Int(res));
                    }
                }
                &Instruction::MulInt(dst, a, b) => {
                    if let (Value::Int(va), Value::Int(vb)) = (r!(a), r!(b)) {
                        let res = va.wrapping_mul(*vb); w!(dst, Value::Int(res));
                    }
                }
                &Instruction::ModInt(dst, a, b) => {
                    if let (Value::Int(va), Value::Int(vb)) = (r!(a), r!(b)) {
                        if *vb != 0 { w!(dst, Value::Int(va % vb)); }
                    }
                }
                &Instruction::AddIntImm(dst, src, imm) => {
                    if let Value::Int(v) = r!(src) { w!(dst, Value::Int(v.wrapping_add(imm as i64))); }
                }
                &Instruction::SubIntImm(dst, src, imm) => {
                    if let Value::Int(v) = r!(src) { w!(dst, Value::Int(v.wrapping_sub(imm as i64))); }
                }
                &Instruction::MulIntImm(dst, src, imm) => {
                    if let Value::Int(v) = r!(src) { w!(dst, Value::Int(v.wrapping_mul(imm as i64))); }
                }
                &Instruction::CmpEqImm(dst, src, imm) => {
                    if let Value::Int(v) = r!(src) { w!(dst, Value::Bool(*v == imm as i64)); }
                }
                &Instruction::CmpNeImm(dst, src, imm) => {
                    if let Value::Int(v) = r!(src) { w!(dst, Value::Bool(*v != imm as i64)); }
                }
                &Instruction::CmpLtImm(dst, src, imm) => {
                    if let Value::Int(v) = r!(src) { w!(dst, Value::Bool(*v < imm as i64)); }
                }
                &Instruction::CmpLeImm(dst, src, imm) => {
                    if let Value::Int(v) = r!(src) { w!(dst, Value::Bool(*v <= imm as i64)); }
                }
                &Instruction::CmpGtImm(dst, src, imm) => {
                    if let Value::Int(v) = r!(src) { w!(dst, Value::Bool(*v > imm as i64)); }
                }
                &Instruction::CmpGeImm(dst, src, imm) => {
                    if let Value::Int(v) = r!(src) { w!(dst, Value::Bool(*v >= imm as i64)); }
                }
                &Instruction::CmpEq(dst, a, b) => {
                    let res = r!(a).equals(r!(b)); w!(dst, Value::Bool(res));
                }
                &Instruction::CmpNe(dst, a, b) => {
                    let res = !r!(a).equals(r!(b)); w!(dst, Value::Bool(res));
                }
                &Instruction::CmpGt(dst, a, b) => {
                    let res = match (r!(a), r!(b)) {
                        (Value::Int(x), Value::Int(y)) => x > y, _ => false
                    };
                    w!(dst, Value::Bool(res));
                }
                &Instruction::Not(dst, src) => {
                    let v = r!(src).is_truthy(); w!(dst, Value::Bool(!v));
                }
                &Instruction::Jump(label) => { ip = label as usize; }
                &Instruction::JumpIf(cond, label) => { if r!(cond).is_truthy() { ip = label as usize; } }
                &Instruction::JumpIfNot(cond, label) => { if !r!(cond).is_truthy() { ip = label as usize; } }
                &Instruction::ConcatStr(dst, a, b) => {
                    let result = match (r!(a), r!(b)) {
                        (Value::String(l), Value::String(r)) => {
                            let mut s = String::with_capacity(l.len() + r.len());
                            s.push_str(l.as_ref());
                            s.push_str(r.as_ref());
                            s
                        }
                        (l, r) => {
                            let ls = l.to_string_repr();
                            let rs = r.to_string_repr();
                            let mut s = String::with_capacity(ls.len() + rs.len());
                            s.push_str(&ls);
                            s.push_str(&rs);
                            s
                        }
                    };
                    w!(dst, Value::String(Rc::new(result)));
                }
                &Instruction::MakeFunc(dst, func_idx) => {
                    w!(dst, Value::Function(func_idx));
                }
                Instruction::MakeClosure(dst, func_idx, captured_regs) => {
                    let (dst, func_idx) = (*dst, *func_idx);
                    let upvalues: Vec<Value> = captured_regs.iter()
                        .map(|&cr| self.value_stack[base + cr as usize].clone())
                        .collect();
                    self.value_stack[base + dst as usize] = Value::Closure(func_idx, Rc::new(upvalues));
                }
                Instruction::Call(dst, func_idx, arg_regs) => {
                    let (dst, func_idx) = (*dst, *func_idx);
                    let args: Vec<Value> = arg_regs.iter()
                        .map(|&r| self.value_stack[base + r as usize].clone())
                        .collect();
                    let result = self.call_function(func_idx, args, &[], program, 0, chunk_idx)?;
                    self.value_stack[base + dst as usize] = result;
                }
                Instruction::TailCall(func_idx, arg_regs) => {
                    let func_idx = *func_idx;
                    let args: Vec<Value> = arg_regs.iter()
                        .map(|&r| self.value_stack[base + r as usize].clone())
                        .collect();
                    let result = self.call_function(func_idx, args, &[], program, 0, chunk_idx)?;
                    self.value_stack.truncate(base);
                    return Ok(result);
                }
                Instruction::CallDynamic(dst, callee_reg, arg_regs) => {
                    let (dst, callee_reg) = (*dst, *callee_reg);
                    let callable = self.value_stack[base + callee_reg as usize].clone();
                    let args: Vec<Value> = arg_regs.iter()
                        .map(|&r| self.value_stack[base + r as usize].clone())
                        .collect();
                    let result = self.call_callable(callable, args, program, 0, chunk_idx)?;
                    self.value_stack[base + dst as usize] = result;
                }
                &Instruction::CmpLt(dst, a, b) => {
                    let res = match (r!(a), r!(b)) {
                        (Value::Int(x), Value::Int(y))       => x < y,
                        (Value::Float(x), Value::Float(y))   => x < y,
                        (Value::Int(x), Value::Float(y))     => (*x as f64) < *y,
                        (Value::Float(x), Value::Int(y))     => *x < (*y as f64),
                        (Value::String(x), Value::String(y)) => x.as_ref() < y.as_ref(),
                        _ => false,
                    };
                    w!(dst, Value::Bool(res));
                }
                &Instruction::CmpLe(dst, a, b) => {
                    let res = match (r!(a), r!(b)) {
                        (Value::Int(x), Value::Int(y))       => x <= y,
                        (Value::Float(x), Value::Float(y))   => x <= y,
                        (Value::Int(x), Value::Float(y))     => (*x as f64) <= *y,
                        (Value::Float(x), Value::Int(y))     => *x <= (*y as f64),
                        (Value::String(x), Value::String(y)) => x.as_ref() <= y.as_ref(),
                        _ => false,
                    };
                    w!(dst, Value::Bool(res));
                }
                &Instruction::CmpGe(dst, a, b) => {
                    let res = match (r!(a), r!(b)) {
                        (Value::Int(x), Value::Int(y))       => x >= y,
                        (Value::Float(x), Value::Float(y))   => x >= y,
                        (Value::Int(x), Value::Float(y))     => (*x as f64) >= *y,
                        (Value::Float(x), Value::Int(y))     => *x >= (*y as f64),
                        (Value::String(x), Value::String(y)) => x.as_ref() >= y.as_ref(),
                        _ => false,
                    };
                    w!(dst, Value::Bool(res));
                }
                &Instruction::NamedTupleGet(dst, tuple_reg, field_idx) => {
                    let field_name = &program.string_pool[field_idx as usize];
                    let result = match &self.value_stack[base + tuple_reg as usize] {
                        Value::NamedTuple(fields) => {
                            let field_name = field_name.clone();
                            fields.iter()
                                .find(|(n, _)| *n == field_name)
                                .map(|(_, v)| v.clone())
                                .unwrap_or(Value::Unit)
                        }
                        _ => Value::Unit,
                    };
                    self.value_stack[base + dst as usize] = result;
                }
                _ => {
                    // For unsupported instructions in HOF mini-VM, skip
                }
            }
        }
        self.value_stack.truncate(base);
        Ok(Value::Unit)
    }
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
