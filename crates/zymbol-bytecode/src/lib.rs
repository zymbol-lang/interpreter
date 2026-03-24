//! Bytecode definitions for Zymbol-Lang Register VM

pub type Reg = u16;
pub type Label = u32;
pub type FuncIdx = u32;
pub type StrIdx = u32;

#[derive(Debug, Clone)]
pub enum Instruction {
    // ── Literals ──────────────────────────────────────────────────────────
    LoadInt(Reg, i64),
    LoadFloat(Reg, f64),
    LoadBool(Reg, bool),
    LoadStr(Reg, StrIdx),
    LoadChar(Reg, char),
    LoadUnit(Reg),
    /// Load a function/lambda reference (stored as FuncIdx)
    MakeFunc(Reg, FuncIdx),
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

    // ── String ops ────────────────────────────────────────────────────────
    ConcatStr(Reg, Reg, Reg),
    StrLen(Reg, Reg),
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

    // ── I/O ──────────────────────────────────────────────────────────────
    Print(Reg),
    PrintNewline,

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
    /// Format number with comma separators: c|expr|
    FormatComma(Reg, Reg),
    /// Format number in scientific notation: e|expr|
    FormatScientific(Reg, Reg),

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

    // ── Halt ─────────────────────────────────────────────────────────────
    Halt,
}

/// Part of a BuildStr instruction
#[derive(Debug, Clone)]
pub enum BuildPart {
    /// A literal string from the string pool
    Lit(StrIdx),
    /// A register whose value gets to_string()'d
    Reg(Reg),
}

#[derive(Debug, Clone)]
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

#[derive(Debug)]
pub struct CompiledProgram {
    pub main: Chunk,
    pub functions: Vec<Chunk>,
    pub string_pool: Vec<String>,
}

impl CompiledProgram {
    pub fn new(main: Chunk) -> Self {
        Self {
            main,
            functions: Vec::new(),
            string_pool: Vec::new(),
        }
    }
}
