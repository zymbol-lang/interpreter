//! Bytecode definitions for Zymbol-Lang Register VM

use serde::{Deserialize, Serialize};

pub type Reg = u16;
pub type Label = u32;
pub type FuncIdx = u32;
pub type StrIdx = u32;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Instruction {
    // ── Literals ──────────────────────────────────────────────────────────
    LoadInt(Reg, i64),
    LoadFloat(Reg, f64),
    LoadBool(Reg, bool),
    LoadStr(Reg, StrIdx),
    LoadChar(Reg, char),
    LoadUnit(Reg),
    /// Load a named-function reference (type ##())
    MakeFunc(Reg, FuncIdx),
    /// Load a no-capture lambda reference (type ##->)
    MakeLambda(Reg, FuncIdx),
    /// Create a closure: dst = Closure(func_idx, upvalues captured from current frame)
    MakeClosure(Reg, FuncIdx, Vec<Reg>),

    // ── Register moves ────────────────────────────────────────────────────
    CopyReg(Reg, Reg),
    MoveReg(Reg, Reg),

    // ── Integer arithmetic ────────────────────────────────────────────────
    AddInt(Reg, Reg, Reg),
    SubInt(Reg, Reg, Reg),
    MulInt(Reg, Reg, Reg),
    DivInt(Reg, Reg, Reg),
    ModInt(Reg, Reg, Reg),
    PowInt(Reg, Reg, Reg),
    NegInt(Reg, Reg),
    AddIntImm(Reg, Reg, i32),
    SubIntImm(Reg, Reg, i32),
    MulIntImm(Reg, Reg, i32),
    CmpEqImm(Reg, Reg, i32),
    CmpNeImm(Reg, Reg, i32),
    CmpLtImm(Reg, Reg, i32),
    CmpLeImm(Reg, Reg, i32),
    CmpGtImm(Reg, Reg, i32),
    CmpGeImm(Reg, Reg, i32),

    // ── Float arithmetic ──────────────────────────────────────────────────
    AddFloat(Reg, Reg, Reg),
    SubFloat(Reg, Reg, Reg),
    MulFloat(Reg, Reg, Reg),
    DivFloat(Reg, Reg, Reg),
    PowFloat(Reg, Reg, Reg),
    NegFloat(Reg, Reg),
    IntToFloat(Reg, Reg),
    /// dst = (src as Float).round() as Int  — ##|expr| / ###expr
    FloatToIntRound(Reg, Reg),
    /// dst = (src as Float).trunc() as Int  — ##!expr
    FloatToIntTrunc(Reg, Reg),

    // ── String ops ────────────────────────────────────────────────────────
    ConcatStr(Reg, Reg, Reg),
    /// dst = base $++ item0 item1 …  — concat string parts, or push to array
    ConcatBuild(Reg, Reg, Vec<Reg>),
    StrLen(Reg, Reg),
    /// dst = str_reg.repeat(n_reg) → String
    StrRepeat(Reg, Reg, Reg),
    /// dst = str.split(char_reg) → Array of Strings
    StrSplit(Reg, Reg, Reg),
    /// dst = str.contains(elem_reg) → Bool  (elem is Char or String)
    StrContains(Reg, Reg, Reg),
    /// dst = str[lo_reg..=hi_reg] → String (exclusive hi like Python)
    StrSlice(Reg, Reg, Reg),
    /// dst = explode String → Array<Char>; Array/Tuple → Rc clone (no-op).
    /// Emitted before every for-each loop to convert strings to char arrays once
    /// (O(N)), avoiding the O(N²) pattern of ArrayGet on String per iteration.
    StrChars(Reg, Reg),
    /// dst = str[idx] → Char  (0-based, no allocation).
    /// ASCII fast path: O(1) byte lookup. Unicode fallback: O(N) chars().nth().
    /// Used by the string-specific for-each loop to avoid StrChars allocations.
    StrCharAt(Reg, Reg, Reg),
    /// dst = str$??pat → Array<Int>  — all char-indices where pat is found
    StrFindPos(Reg, Reg, Reg),
    /// dst = str$++[pos:text] → String  — insert text at char position
    StrInsert(Reg, Reg, Reg, Reg),
    /// dst = str$--[pos:count] → String  — remove count chars at pos
    StrRemove(Reg, Reg, Reg, Reg),
    /// dst = str$~~[pat:rep] → String  — replace all occurrences
    StrReplace(Reg, Reg, Reg, Reg),
    /// dst = str$~~[pat:rep:n] → String  — replace first n occurrences
    StrReplaceN(Reg, Reg, Reg, Reg, Reg),
    /// Interpolated string: dst = build string from parts stored in field_regs
    /// field_regs: alternating literal StrIdx / variable Reg
    /// Encoded as: BuildStr(dst, Vec<StringPart>)
    BuildStr(Reg, Vec<BuildPart>),

    // ── Comparison ────────────────────────────────────────────────────────
    CmpEq(Reg, Reg, Reg),
    CmpNe(Reg, Reg, Reg),
    CmpLt(Reg, Reg, Reg),
    CmpLe(Reg, Reg, Reg),
    CmpGt(Reg, Reg, Reg),
    CmpGe(Reg, Reg, Reg),

    // ── Logical ──────────────────────────────────────────────────────────
    And(Reg, Reg, Reg),
    Or(Reg, Reg, Reg),
    Not(Reg, Reg),

    // ── Control flow ─────────────────────────────────────────────────────
    Jump(Label),
    JumpIf(Reg, Label),
    JumpIfNot(Reg, Label),

    // ── Functions ─────────────────────────────────────────────────────────
    Call(Reg, FuncIdx, Vec<Reg>),
    TailCall(FuncIdx, Vec<Reg>),
    Return(Reg),
    /// Call a function/lambda stored in a register (for lambdas as values)
    CallDynamic(Reg, Reg, Vec<Reg>),
    /// Call a stdlib native function; builtin_id is a constant from the `builtins` module
    CallBuiltin(Reg, u16, Vec<Reg>),

    // ── I/O ──────────────────────────────────────────────────────────────
    Print(Reg),
    PrintNewline,
    /// Set active numeral mode; `base` is the block base codepoint of the script.
    SetNumeralMode(u32),

    // ── Arrays ───────────────────────────────────────────────────────────
    NewArray(Reg),
    ArrayPush(Reg, Reg),
    ArrayGet(Reg, Reg, Reg),
    ArraySet(Reg, Reg, Reg),
    ArrayLen(Reg, Reg),
    ArrayRemove(Reg, Reg),
    /// Remove first occurrence of val from arr in-place ($-)
    ArrayRemoveValue(Reg, Reg),
    /// Remove all occurrences of val from arr in-place ($--)
    ArrayRemoveAll(Reg, Reg),
    /// Insert val at position idx in arr in-place ($+[i])
    ArrayInsert(Reg, Reg, Reg),
    /// Remove elements [lo..hi) from arr in-place ($-[lo..hi]); hi_reg = lo_reg + 1
    ArrayRemoveRange(Reg, Reg),
    /// dst = arr.contains(elem)
    ArrayContains(Reg, Reg, Reg),
    /// dst = arr[lo..hi] (exclusive hi, like Python slicing)
    ArraySlice(Reg, Reg, Reg),
    /// HOF: dst = arr.map(lambda_reg)
    ArrayMap(Reg, Reg, Reg),
    /// HOF: dst = arr.filter(lambda_reg)
    ArrayFilter(Reg, Reg, Reg),
    /// HOF: dst = arr.reduce(init_reg, lambda_reg)
    ArrayReduce(Reg, Reg, Reg, Reg),
    /// Fused: dst = (str $/ sep)$#  — count parts, zero Vec<Value> (via intrinsics::split::count)
    StrSplitCount(Reg, Reg, Reg),
    /// Fused: dst = (str $/ sep) $> fn  — no intermediate Vec<Value>
    StrSplitMap(Reg, Reg, Reg, Reg),
    /// Fused: dst = (str $/ sep) $| fn  — filter parts without materializing split
    StrSplitFilter(Reg, Reg, Reg, Reg),
    /// Fused: dst = (str $/ sep) $< (init, fn)  — reduce over parts directly
    StrSplitReduce(Reg, Reg, Reg, Reg, Reg),
    /// HOF: dst = arr.sort(ascending, opt_func_reg)
    /// ascending: true=$^+, false=$^-; func_reg=u8::MAX means natural order
    ArraySort(Reg, Reg, bool, Reg),

    // ── Tuples ───────────────────────────────────────────────────────────
    /// Build a positional tuple: dst = (regs[0], regs[1], ...)
    MakeTuple(Reg, Vec<Reg>),

    // ── Named tuples ─────────────────────────────────────────────────────
    /// Build a named tuple from field values; field names in string pool
    MakeNamedTuple(Reg, Vec<StrIdx>, Vec<Reg>),
    /// dst = named_tuple.field_name (or positional index)
    NamedTupleGet(Reg, Reg, StrIdx),

    // ── Pattern match ─────────────────────────────────────────────────────
    MatchInt(Reg, i64, Label),
    MatchRange(Reg, i64, i64, Label),
    MatchStr(Reg, StrIdx, Label),
    MatchBool(Reg, bool, Label),

    // ── Data ops ─────────────────────────────────────────────────────────
    /// dst = parse_number(str) → Int or Float
    NumericEval(Reg, Reg),
    /// dst = (type_symbol_str, len, value) tuple
    TypeOf(Reg, Reg),
    /// dst = Bool(true) if src is an Array, false otherwise
    IsArray(Reg, Reg),
    /// Base conversion: 0x|expr|, 0b|expr|, 0o|expr|, 0d|expr|
    /// prefix: 2=binary, 8=octal, 10=decimal, 16=hex
    BaseConvert(Reg, Reg, u8),

    // ── Try/catch ────────────────────────────────────────────────────────
    /// Begin a try block; catch_label is where to jump on error
    TryBegin(Label),
    /// End try block normally (jump over catch)
    TryEnd(Label),
    /// Catch handler: load error value into reg
    TryCatch(Reg),
    /// Finally always runs; no special opcode needed — compiler emits the block twice

    // ── Shell execution ───────────────────────────────────────────────────
    /// Execute a shell command; parts work like BuildStr (literals and registers)
    BashExec(Reg, Vec<BuildPart>),
    /// Execute a Zymbol script (</ path />); raises VmError if exit code != 0
    Execute(Reg, Vec<BuildPart>),

    // ── Format ops ───────────────────────────────────────────────────────
    /// Format number with thousands separators: #,|expr| / #,.N|expr| / #,!N|expr|
    /// precision_kind: 0 = none, 1 = round, 2 = truncate; precision_n: N (0 if kind=none)
    FmtThousands(Reg, Reg, u8, u32),
    /// Format number in scientific notation: #^|expr| / #^.N|expr| / #^!N|expr|
    /// precision_kind: 0 = none, 1 = round, 2 = truncate; precision_n: N (0 if kind=none)
    FmtScientific(Reg, Reg, u8, u32),

    // ── Precision ops ────────────────────────────────────────────────────
    /// Round dst = #.precision|src|  (standard rounding)
    RoundFloat(Reg, Reg, u32),
    /// Truncate dst = #!precision|src|  (floor toward zero)
    TruncFloat(Reg, Reg, u32),

    // ── Error check ──────────────────────────────────────────────────────
    /// dst = src$!  → Bool: #1 if src is an error value, #0 otherwise
    IsError(Reg, Reg),
    /// Load the error kind string ("IO", "Index", "Type", "Div", "_") into dst
    LoadErrorKind(Reg),

    /// Raise a runtime error with message from string pool (for deferred errors)
    RaiseError(StrIdx),

    // ── Output param writeback ────────────────────────────────────────────
    /// Before a Call: pairs of (callee_param_idx, caller_dst_reg) for output params.
    /// On Return, callee's param registers are written back to caller's dst registers.
    SetupOutputWriteback(Vec<(u16, Reg)>),

    // ── Module global vars ────────────────────────────────────────────────
    /// Load a module-level global variable: dst = global_vars[idx]
    LoadGlobal(Reg, u16),
    /// Store to a module-level global variable: global_vars[idx] = src
    StoreGlobal(u16, Reg),

    // ── TUI primitives ───────────────────────────────────────────────────
    /// @~ N — sleep N milliseconds (ms value in reg)
    Sleep(Reg),
    /// >>! — clear terminal screen (no registers)
    ClearScreen,
    /// >>? — query terminal size → Value::Tuple([rows, cols]) stored in dst
    QueryTerminalSize(Reg),
    /// <<| var / <<|? var — read one key → Value::Char in dst; blocking flag
    ReadKey(Reg, bool),
    /// << var / << "prompt" var / << #|var| / << <typespec> "prompt" var
    /// read a line from stdin → store in dst.
    /// prompt_reg: Some(r) = print register r as prompt before reading; None = no prompt
    /// kind: how to validate/coerce the line (and whether to re-prompt). See `InputKind`.
    ReadLine(Reg, Option<Reg>, InputKind),
    /// >>~ pos > items — positioned output (pos tuple in r_pos, items in Vec<Reg>)
    PrintAt(Reg, Vec<Reg>),
    /// >>| { } — enter alternate screen + raw mode
    EnterTui,
    /// >>| { } — leave alternate screen + disable raw mode
    ExitTui,

    // ── Hot variable initialization ───────────────────────────────────────
    /// If dst currently holds Unit (uninitialized), set it to the neutral element.
    /// Used for hot-def (x°) variables: runs on every potential first-use site
    /// but is a no-op after the first real initialization.
    HotInit(Reg, HotNeutral),

    // ── CLI args ─────────────────────────────────────────────────────────
    /// dst = Array of CLI arguments passed after the script path (argv[1..])
    LoadCliArgs(Reg),

    // ── Halt ─────────────────────────────────────────────────────────────
    Halt,
}

/// Neutral element kind for hot-definition variables (x°)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HotNeutral {
    Int,    // 0
    IntOne, // 1  (multiplicative identity: *=, /=)
    Array,  // []
    String, // ""
}

/// Part of a BuildStr instruction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BuildPart {
    /// A literal string from the string pool
    Lit(StrIdx),
    /// A register whose value gets to_string()'d
    Reg(Reg),
}

/// How a `ReadLine` should validate and coerce the line it reads. Mirrors the
/// tree-walker's `InputCast`; the typed variants re-prompt until the input is valid.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InputKind {
    /// Store the trimmed line verbatim as a String.
    Raw,
    /// Legacy `#|var|`: parse to Int/Float, falling back to String.
    Numeric,
    /// `##.` — any valid floating-point number → Float.
    Float,
    /// `##.(total, decimals)` — fixed-format decimal → Float.
    Decimal { total: u32, decimals: u32 },
    /// `###` / `###(n)` — integer with at most `max_digits` digits → Int.
    Int { max_digits: Option<u32> },
    /// `##"` / `##"(n)` — string of at most `max` characters → String.
    Text { max: Option<u32> },
    /// `##'` — exactly one character → Char.
    Char,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chunk {
    pub name: String,
    pub instructions: Vec<Instruction>,
    pub num_registers: u16,
    pub num_params: u16,
}

impl Chunk {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            instructions: Vec::new(),
            num_registers: 0,
            num_params: 0,
        }
    }
}

/// Initial value for a module-level global variable
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GlobalInit {
    Int(i64),
    Float(f64),
    Bool(bool),
    Char(char),
    Str(String),
    Unit,
}

/// Builtin function IDs — shared between compiler (emit site) and VM (dispatch).
pub mod builtins {
    // std/math functions
    pub const SQRT:    u16 = 0;
    pub const EXP:     u16 = 1;
    pub const LN:      u16 = 2;
    pub const LOG:     u16 = 3;
    pub const POW:     u16 = 4;
    pub const SIN:     u16 = 5;
    pub const COS:     u16 = 6;
    pub const TAN:     u16 = 7;
    pub const ASIN:    u16 = 8;
    pub const ACOS:    u16 = 9;
    pub const ATAN:    u16 = 10;
    pub const ATAN2:   u16 = 11;
    pub const TANH:    u16 = 12;
    pub const SINH:    u16 = 13;
    pub const COSH:    u16 = 14;
    pub const SIGMOID: u16 = 15;
    pub const ABS:     u16 = 16;
    pub const MAX:     u16 = 17;
    pub const MIN:     u16 = 18;
    pub const FLOOR:   u16 = 19;
    pub const CEIL:    u16 = 20;
    pub const ROUND:   u16 = 21;
    // std/random functions
    pub const RAND_ENTERO:   u16 = 100;
    pub const RAND_RANGO:    u16 = 101;
    pub const RAND_PESO_F64: u16 = 102;
    // std/json functions
    pub const JSON_DECODE:   u16 = 200;
    pub const JSON_ENCODE:   u16 = 201;
    // std/io functions
    pub const IO_READ:       u16 = 300;
    pub const IO_WRITE:      u16 = 301;
    pub const IO_APPEND:     u16 = 302;
    pub const IO_EXISTS:     u16 = 303;
    pub const IO_DELETE:     u16 = 304;
    pub const IO_LIST:       u16 = 305;
    pub const IO_MKDIR:      u16 = 306;
    // std/net functions
    pub const NET_GET:       u16 = 400;
    pub const NET_POST:      u16 = 401;
    pub const NET_POST_JSON: u16 = 402;
    pub const NET_HEAD:      u16 = 403;
    // std/db functions (vendor-neutral via ODBC)
    pub const DB_CONNECT:      u16 = 500;
    pub const DB_DISCONNECT:   u16 = 501;
    pub const DB_EXEC:         u16 = 502;
    pub const DB_QUERY:        u16 = 503;
    pub const DB_QUERY_ONE:    u16 = 504;
    pub const DB_QUERY_VALUE:  u16 = 505;
    pub const DB_TX:           u16 = 506;
    pub const DB_BEGIN:        u16 = 507;
    pub const DB_COMMIT:       u16 = 508;
    pub const DB_ROLLBACK:     u16 = 509;
    pub const DB_SAVEPOINT:    u16 = 510;
    pub const DB_RELEASE:      u16 = 511;
    pub const DB_ROLLBACK_TO:  u16 = 512;
    pub const DB_EXEC_SCRIPT:  u16 = 513;
    pub const DB_TABLE_EXISTS: u16 = 514;
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CompiledProgram {
    pub main: Chunk,
    pub functions: Vec<Chunk>,
    pub string_pool: Vec<String>,
    /// Initial values for module global variables (indexed by LoadGlobal/StoreGlobal idx)
    pub global_var_inits: Vec<GlobalInit>,
}

impl CompiledProgram {
    pub fn new(main: Chunk) -> Self {
        Self {
            main,
            functions: Vec::new(),
            string_pool: Vec::new(),
            global_var_inits: Vec::new(),
        }
    }
}
