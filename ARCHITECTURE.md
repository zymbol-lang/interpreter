# Zymbol-Lang — Interpreter Architecture

## Overview

Zymbol-Lang is a minimalist symbolic programming language with no words in its grammar. This document
describes the architecture of its Rust implementation: a workspace of 20 crates organized
by compilation phase, execution mode, and tooling.

The interpreter supports two independent execution strategies:

- **Tree-walker** (the default of a loose `.zy`): walks the AST directly, scope-stack based
- **Register VM** (`--vm`; the default of a `.zyp`): compiles AST to bytecode and runs it on a
  register-based virtual machine. At parity with the tree-walker since v0.0.8, and the default
  engine from v0.0.11 (decided 2026-10-11)

---

## Design Principles

1. **Modularity** — each compilation phase is an isolated crate with clear boundaries
2. **Dual execution** — tree-walker and VM coexist without shared runtime state
3. **Unicode-first** — identifiers, strings, and operators support full Unicode including emojis
4. **No words** — all language constructs use pure symbolic operators (not "no keywords":
   see `SYMBOLS.md` §1.2 for why the precise claim is about words)
5. **Explicit over implicit** — no hidden coercions, no automatic newlines, no magic

---

## Workspace Structure

```
interpreter/
├── Cargo.toml                  # Workspace manifest (19 members)
├── Cargo.lock
├── zymbol-lang.ebnf            # Formal grammar (EBNF) — v3.4.0
├── install-zymbol.sh           # Install script
├── crates/
│   ├── zymbol-span/            # Source position tracking
│   ├── zymbol-error/           # Diagnostic system
│   ├── zymbol-common/          # Shared types and utilities
│   ├── zymbol-lexer/           # Tokenization
│   ├── zymbol-ast/             # AST node definitions
│   ├── zymbol-parser/          # Recursive descent parser
│   ├── zymbol-semantic/        # Semantic analysis and type checking
│   ├── zymbol-interpreter/     # Tree-walker executor
│   ├── zymbol-bytecode/        # Bytecode instruction set
│   ├── zymbol-compiler/        # AST → Bytecode compiler
│   ├── zymbol-vm/              # Register-based virtual machine
│   ├── zymbol-host/            # The VM's native host: terminal, network, database
│   ├── zymbol-intrinsics/      # Unboxed string/collection intrinsics (VM fast paths)
│   ├── zymbol-formatter/       # Code formatter
│   ├── zymbol-analyzer/        # LSP analysis engine
│   ├── zymbol-lsp/             # Language Server Protocol (tower-lsp)
│   ├── zymbol-repl/            # Interactive REPL
│   ├── zymbol-package/         # .zyp source archives (manifest, closure, ZIP I/O)
│   ├── zymbol-cli/             # Main CLI entry point
│   └── zymbol-standalone/      # Standalone executable builder
├── tests/                      # Wrappers over ../zyquality/, where the corpus and the gate live
└── docs/                       # Extended documentation
```

---

## Crate Catalog

### Foundation

#### `zymbol-span`
Unicode-aware source position tracking. Provides `FileId`, `Position`, `Span`, and
`SourceMap` with line-start caching. Handles grapheme clusters for emoji identifiers.

**Dependencies**: none

#### `zymbol-error`
Diagnostic system with severity levels (`Error`, `Warning`, `Note`). Colorized terminal
output using `owo-colors`, source context rendering with caret annotations.

**Dependencies**: `zymbol-span`

#### `zymbol-common`
Shared types across all crates: symbol interning (`u32` IDs), `Literal` enum
(`Int`, `Float`, `String`, `Char`, `Bool`), binary and unary operator types.

Also holds `stdlib`: the export table for the `std/` modules (function names with
their arity, constant names). `std/` has no file on disk — the tree-walker
registers native functions and the compiler maps names to VM builtin ids, neither
of which the tooling can read — so without this table `math::inventada(2.0)` passed
every static check and only failed at run time. `zymbol_semantic::check_stdlib_access`
reads it, and both `zymbol check` and the LSP call that one function, so they agree.
`crates/zymbol-cli/tests/stdlib_parity.rs` fails if the table and the two engines
disagree on a name or an arity: adding a stdlib function means touching the engine,
the compiler's builtin table, and this list.

It also holds what every engine has to say the same way, which is why it is a crate of
its own and not a copy in each engine:

- `num` — the numeric model: the safe integer ±(2⁵³ − 1), checked in every engine;
- `errkind` — which `##` family a runtime error belongs to, read from its message, so
  that `:! ##Div` catches the same failures in the tree-walker, the VM and the browser
  engine;
- `typesym`, `typeword` — a type's symbol (`##.`), which a program asks with, and its
  name in words (`Float`), which a diagnostic uses;
- `shell` — locating the shell that `<\ \>` and `</ />` run through (Windows has no `sh`);
- `vscreen` — a 24×80 screen with no terminal behind it, and the key script that drives it,
  for `zymbol run --keys`;
- `count` — the five counters behind `zymbol run --count` (`steps`, `calls`, `allocs`,
  `bytes`, `copied`): thread-local cells with constant initializers, always compiled, and
  `make_mut`, the one function every write to a shared collection goes through — the one
  that counts `copied`.

**Dependencies**: none

---

### Frontend

#### `zymbol-lexer`
Tokenizes source text into 100+ token types. Handles string interpolation
(`StringInterpolated(Vec<StringPart>)`), all symbolic operators (`>>`, `<<`, `?`, `@`,
`??`, `$#`, `$+`, `!?`, etc.), and Unicode identifiers.

**Dependencies**: `zymbol-span`, `zymbol-common`, `zymbol-error`

#### `zymbol-ast`
Complete AST node definitions. Key types:

```
Program → module_decl?, imports[], statements[]

Statement:
  Output, Assignment, ConstDecl, Input, If, Loop, Break, Continue,
  Try, FunctionDecl, Return, Match, Expr, CliArgsCapture

Expr:
  Literal, Identifier, Binary, Unary, Range, ArrayLiteral, Tuple,
  NamedTuple, MemberAccess, Index, FunctionCall, Lambda,
  CollectionLength, CollectionAppend, CollectionSlice, CollectionContains,
  StringSplit, StringSlice, ErrorCheck, ErrorPropagate, Pipe,
  NumericEval, TypeOf, BaseConversion, Format
```

**Dependencies**: `zymbol-lexer`, `zymbol-span`, `zymbol-common`

#### `zymbol-parser`
Recursive descent parser with error recovery. Produces `Program` AST from token stream.
Module-aware: handles `#` (declare), `<#` (import), `#>` (export).

**Dependencies**: `zymbol-lexer`, `zymbol-ast`, `zymbol-common`, `zymbol-error`, `zymbol-span`

---

### Semantic Analysis

#### `zymbol-semantic`
Static analysis passes over the AST:
- `VariableAnalyzer`: unused variable detection, def-use chains
- `TypeChecker`: type inference and validation
- `ControlFlowGraph`: CFG construction for reachability analysis
- `ModuleAnalyzer`: import/export validation, circular dependency detection
- `last_use` (v0.0.8): purely lexical, conservative last-use analysis that drives auto-free
  in both engines. A region is a flat statement sequence (the top-level program, or a named
  function body); mentions are collected from the whole statement subtree, including nested
  blocks, loop and lambda bodies, `{var}` interpolations and input prompts. Constants, hot
  names (`x°`/`°x`), `_`-prefixed names, module-level bindings and output parameters are
  never candidates. Its `Expr` walker is exhaustive (no `_` arm) on purpose: new syntax fails
  to compile until its mention rules are reviewed

**Dependencies**: `zymbol-ast`, `zymbol-error`, `zymbol-span`, `zymbol-common`

---

### Tree-Walker Execution

#### `zymbol-interpreter`
Walks the AST directly and evaluates it. It was the default execution mode until v0.0.11; it is now asked for with `--tw`, and is the only engine the REPL has.

**Runtime `Value` enum**:

> `Int` is stored in an `i64`, but the *language's* integer is narrower: the
> safe-integer range ±(2⁵³ − 1) defined in `zymbol-common/src/num.rs`. The ten
> spare bits are headroom for the checks, not range a program can use — every
> operation that could produce a value outside it raises `##Range`. See
> REFERENCE.md § Numeric limits.

```rust
enum Value {
    Int(i64), Float(f64), Char(char), Bool(bool),
    String(String),
    Array(Rc<Vec<Value>>),                  // shared; copied when written
    Tuple(Rc<Vec<Value>>),
    NamedTuple(Rc<Vec<(String, Value)>>),
    Function(FunctionValue),  // params, body, captures: Rc<HashMap>
    Error(ErrorValue),
    Unit,
}
```

**Interpreter state** (key fields):
```rust
struct Interpreter {
    scope_stack: Vec<HashMap<String, Value>>,     // lexical scoping
    functions: HashMap<String, Rc<FunctionDef>>,
    control_flow: ControlFlow,                    // Break | Continue | Return | None
    mutable_vars_stack: Vec<HashSet<String>>,
    const_vars_stack: Vec<HashSet<String>>,
    loaded_modules: HashMap<PathBuf, LoadedModule>,
    // Object pools (Sprint 5 optimizations)
    scope_map_pool: Vec<HashMap<String, Value>>,
    mut_set_pool: Vec<HashSet<String>>,
    arg_vec_pool: Vec<Vec<Value>>,
    // Tail-call optimization
    tco_pending: bool,
    tco_args: Vec<Value>,
}
```

**Key features**:
- Lexical scoping via scope stack push/pop
- Closures: `capture_environment()` captures outer scope vars into `Rc<HashMap>`
- Tail-call optimization (TCO): detects `<~ f(same_args)` and restarts without stack growth
- Module system: file-based imports with alias resolution and circular dep detection
- Native stdlib (`src/stdlib/`): `std/math`, `std/random` (v0.0.6); `std/json`, `std/io`,
  `std/net`, `std/db` (v0.0.7); `std/term` (v0.0.8); `std/time` (v0.0.9). Each module registers
  `FunctionDef::Native` entries; `std/*` import paths resolve in-process (no filesystem
  lookup)
- Auto-free (v0.0.8): a variable is destroyed right after the statement holding its last
  use, not at scope end. Schedules come from `zymbol_semantic::last_use` and are stored per
  body in `FunctionDef::Zymbol::auto_free`, applied by `execute_body_scheduled`. Unobservable
  by design — it only lowers peak memory
- Copy on write (v0.0.9, HLZ-012/014): `Array`, `Tuple` and `NamedTuple` sit behind an `Rc`
  and are copied when *written*, through `zymbol_common::count::make_mut`, which also counts
  what it copies. Reading one element, or handing a collection to a function, copies nothing
- Output parameters (`<~`) are the caller's variable, *lent* for the call and given back when
  it ends, not copied in and out (v0.0.10, GLB-117)
- A module's state lives in one place: in a module frame `scope_stack[0]` *is* the module's
  table, moved in and out and never copied (v0.0.10, ZYTW-010)
- Bash execution: `<\ command \>` captures stdout + stderr via `Command::output()`
- Error handling: try/catch/finally with typed catch (`:! ##IO`, `:! ##Parse`, etc.)

**Dependencies**: `zymbol-ast`, `zymbol-parser`, `zymbol-lexer`, `zymbol-span`,
`zymbol-error`, `zymbol-semantic`

---

### Register VM Execution

#### `zymbol-bytecode`
Instruction set definition: 171 instruction variants (counted 2026-10-17); the table names the
ones a reader meets first.

| Category | Instructions |
|----------|-------------|
| Load | `LoadInt`, `LoadFloat`, `LoadBool`, `LoadStr`, `CopyReg` |
| Arithmetic | `AddInt`, `SubInt`, `MulInt`, `DivInt`, + immediate variants (`AddIntImm`, etc.) |
| Comparison | `CmpEq`, `CmpNe`, `CmpLt`, `CmpLe`, `CmpGt`, `CmpGe` + immediate variants |
| Strings | `ConcatStr`, `StrLen`, `StrSplit`, `StrContains`, `StrSlice`, `BuildStr` |
| Arrays | `NewArray`, `ArrayPush`, `ArrayGet`, `ArraySet`, `ArrayLen`, `ArraySlice` |
| Tuples | `MakeTuple`, `MakeNamedTuple`, `NamedTupleGet` |
| Control | `Jump`, `JumpIf`, `JumpIfNot`, `JumpIfNotCond` (the jump of a `?` condition, which refuses what is not a Bool), `RangeStep` (the end of a pass of a counted loop: step, test, jump back — one dispatch), `IterStep` (the same for a for-each), `Call`, `TailCall`, `CallDynamic`, `Return` |
| Closures | `MakeClosure`, `CallDynamic` |
| Higher-order | `HofItems`, `HofNext` (one turn of `$>`/`$\|`/`$<`: calls the lambda on the VM's own frames, and the lambda returns to it), `HofRebuild` |
| Data ops | `NumericEval`, `TypeOf`, `BaseConversion`, `RoundFloat`, `Format` |
| I/O | `Print`, `PrintNewline` |
| System | `BashExec`, `ExecuteBytecode` |

`CompiledProgram` contains: `functions: Vec<Chunk>`, `strings: Vec<Rc<String>>` (pre-interned pool).

**Dependencies**: none

#### `zymbol-compiler`
Compiles `Program` AST to `CompiledProgram` bytecode.

- Register allocation per `FunctionCtx`
- `StaticType` inference (`Int`, `Float`, `Bool`, `String`, `Unknown`) for immediate operands
- Loop break/continue patching resolved at loop end
- Closure compilation: `MakeClosure` + `collect_free_vars()` static analysis
- Module resolution via `compile_with_dir()`

**Dependencies**: `zymbol-bytecode`, `zymbol-ast`, `zymbol-parser`, `zymbol-lexer`, `zymbol-span`

#### `zymbol-vm`
Register-based virtual machine.

**VM `Value` enum** (16 bytes via `Rc<T>` heap payloads, Sprint 5D; `ZyStr` inline ≤7 bytes, Sprint 5G):
```rust
enum Value {
    Int(i64), Float(f64), Bool(bool), Char(char), Unit,
    Function(FuncIdx, u8),
    String(ZyStr),                       // inline for ≤ 7 bytes, heap Rc<String> otherwise
    Array(Rc<Vec<Value>>),
    Tuple(Rc<Vec<Value>>),
    NamedTuple(Rc<Vec<(String, Value)>>),
    Closure(FuncIdx, u8, Rc<Vec<Value>>),  // func + arity + captured upvalues
    Error(ZyStr),                        // "##Kind(message)" — error-as-value flow
}
```

**Frame model** (Sprint 5C — flat register stack):
```rust
struct FrameInfo {
    base: u32,            // start of this frame's registers in value_stack
    ip: u32,              // the caller's saved IP
    chunk_idx: u32,       // u32::MAX = the main chunk, else an index into program.functions
    return_reg: u16,      // the caller's register that receives the return value
    catch_ip: u32,        // the armed handler of this frame (u32::MAX = none)
    error: Option<Box<FrameError>>,             // allocated only when an error lands here
    writeback: Option<Box<Vec<(usize, Reg)>>>,  // output parameters to hand back
}

struct VM<W: Write> {
    value_stack: Vec<Value>,    // all frames share one Vec
    frame_stack: Vec<FrameInfo>,
    global_vars: Vec<Value>,    // a module's state, by slot
    in_flight: Option<Pending>, // an error that reached a handler and is not handled yet
    // … exit code, CLI args, the subscript runner, the virtual screen of `--keys`
}
```

**One dispatch loop** (GLB-010, ZYVM-004). `run` enters `exec(program, floor)` once, at the
main frame. A higher-order operator (`$>`, `$|`, `$<`, `$^`, and the splitting forms) pushes
the callee's frame and enters the *same* loop down to it (`call_callable`), so an instruction
added to the loop is added everywhere. It used to hand the call to a second, smaller loop
that knew 96 instructions and skipped 49 in silence: `["a,b"]$> (s -> s$/ ',')` answered
`[()]`. The CLI runs a program on a 64 MiB thread because of it — recursion depth through
`$>` went from 593 to 4 764.

**An error is state, not control flow.** `TryBegin` arms a handler in the frame (`catch_ip`)
and saves the one it replaced in a register of the `!?`, so nesting costs no allocation. A
landed error is `Pending` until a clause consumes it, goes on through the `:>` after them,
and is raised again if nothing did. An error with no handler at or above `floor` unwinds to
`floor`, and the operator raises it in the loop outside, where a `!?` around the `$>` can
meet it.

**Two instructions that exist for a rule.** `JumpIfNotCond` is the jump of a `?` condition:
it refuses what is not a Bool with a `##Type` where `JumpIfNot` read an Array as true
(GLB-121), and it costs no extra dispatch. An output argument (`<~`) is *lent*: the caller's
register lets go of the value once every argument has been read and gets it back when the
frame returns or is unwound, so the first edit inside the callee is the sole owner's and
writes in place (GLB-117).

**Key optimizations**:
- Flat value_stack: O(1) register access via `value_stack[base + reg]`
- String pool: `LoadStr` = O(1) `Rc::clone`
- Immediate operands: `AddIntImm`, `CmpLeImm`, etc.
- Lazy `Box<FrameError>`: no allocation unless a catch is hit
- Sentinel `catch_ip = u32::MAX` avoids a per-instruction branch
- A register write drops the old value only if it owns memory, and a frame holds what the
  function needs at once, not every temporary its body ever asked for (ZYVM-010)
- Edits are atomic and validate before they take the value, so a write to a module's state
  has one owner and costs one element, not the collection (ZYVM-003, ZYVM-013)

**A call's result is let go when its statement ends** (ZYVM-022, fixed 2026-10-19): the
compiler notes the register a `Call`, `CallDynamic` or `CallBuiltin` writes, takes it off the
list when an arithmetic instruction reads it (the register then holds a number), and
`release_temps` clears what is left at the end of the statement. Before, a temporary kept
whatever its last user left until something reused it, so the value a call returned and the
caller used once was alive while the next call built its own (see the auto-free note under
*Feature Parity*).

**Stdlib builtins**: `stdlib_builtins.rs` dispatches `std/*` module calls by numeric
builtin id resolved at compile time (math 0–, random 100–, json 200–, io 300–,
net 400–, db 500–514), mirroring the tree-walker's native modules for full parity.

**Dependencies**: `zymbol-bytecode`, `zymbol-intrinsics`

#### `zymbol-intrinsics`
Pure-Rust string/collection intrinsics operating on unboxed `&str`/primitive types —
no `Value` boxing, no VM types. The VM unwraps `ZyStr` → `&str`, calls the intrinsic,
and boxes the primitive result back. Keeps hot paths independently optimizable
(SIMD, Aho-Corasick, …) without touching the VM.

**Dependencies**: none

---

### Tooling

#### `zymbol-formatter`
Code formatter with `FormatterConfig` (indent size, line length). Uses AST visitor
pattern. Preserves comments. Enforces spacing rules: `x = 5` (spaces around `=`),
`arr$#` (no space before postfix), `1..10` (no spaces around range).

**Dependencies**: `zymbol-ast`, `zymbol-parser`, `zymbol-lexer`, `zymbol-span`, `zymbol-error`

#### `zymbol-analyzer`
LSP analysis engine decoupled from the language server.
- `DocumentCache`: thread-safe document storage (`DashMap`)
- `SymbolIndex`: 3-level definition lookup
- `ModuleIndex`: cross-file export tracking. Also records which modules do **not**
  compile (parse or semantic errors), so a file importing one is told on its import
  line (`module-has-errors`) instead of only finding out at run time — the editor's
  counterpart to `zymbol check` following imports
- `DiagnosticPipeline`: lexer → parser → semantic analysis per document

A module's export list includes its re-exports (`alias::item => name`). Imports are
registered before the export block is read, because a re-export resolves its source
through the file's own alias map; doing it the other way round dropped every
re-export on a file's first pass and made i18n layer modules (see `I18N.md`) look
like they exported nothing.

**Dependencies**: `zymbol-ast`, `zymbol-parser`, `zymbol-lexer`, `zymbol-semantic`,
`zymbol-span`, `zymbol-error`

#### `zymbol-lsp`
Language Server Protocol server using `tower-lsp` + `tokio`.
Features: diagnostics, semantic tokens, document symbols, go-to-definition,
find references, hover. Binary: `zymbol-lsp`.

**Dependencies**: `zymbol-analyzer`, `zymbol-formatter`

#### `zymbol-repl`
Interactive REPL with line editor. History (↑↓), selection, clipboard (Ctrl+C/V).
Built-in commands: `HELP`, `EXIT`, `VARS`, `CLEAR`, `HISTORY`.

**Dependencies**: `zymbol-lexer`, `zymbol-parser`, `zymbol-interpreter`, `zymbol-error`

#### `zymbol-host`
The register VM's native host (v0.0.11, `ROADMAP_V0011_ES.md` § 3.1). `zymbol-vm` depends
on nothing a system has to provide: what a program needs from outside comes through two
traits in `zymbol_vm::host` — `Terminal` (the seven TUI primitives, `@~`, `<<`) and `Host`
(the clock, `std/random`'s seed, `std/net`, `std/db`). This crate implements them for the
machine the binary runs on: `NativeTerminal` with `crossterm`, `NativeHost` with `ureq`
(feature `http`) and ODBC (feature `db`). The CLI and the executable `zymbol build` writes
install both; `zymbol run --keys` replaces the terminal with `zymbol_vm::host::Headless`
and nothing else. A VM nobody gave a host has `BasicTerminal` and `BasicHost`: standard
input and output, the system clock, and no network or database. That split is what lets
`zymbol-vm` build for `wasm32-unknown-unknown`. The tree-walker keeps its own calls into
`crossterm`, `ureq` and ODBC, as it did.

**Dependencies**: `zymbol-vm`, `zymbol-bytecode`, `zymbol-common`, `zymbol-intrinsics`,
`crossterm`, and optionally `ureq` and `odbc-api`

#### `zymbol-standalone`
Embeds a Zymbol source file into a Rust project template, builds it with `cargo`,
and produces a self-contained executable (debug or release).

#### `zymbol-package`
Zymbol Packages (`.zyp`): a portable ZIP archive of **source**, not of a compiled binary.
Unrelated to `zymbol-standalone` above, and with no dependency on it in either direction.

- `manifest`: `zyp.toml` parsing/validation (`[package]` + `[[script]]` entries), plus the
  `zyp.json` twin written into the archive so the web playground never parses TOML in the
  browser. `check_engine` validates `package.engine` as a semver *requirement*.
- `closure`: the transitive-dependency closure from the declared entry scripts, following
  module imports and `</ file.zy />` targets. Strict about what it packages (an unreachable
  `.zy` is never included) and permissive about what it can't resolve statically — absolute
  imports, `<\ shell \>`, parse errors all become warnings `W001`–`W011`, never hard
  failures.
- `reader` / `writer`: ZIP I/O. The writer is deterministic (fixed 1980-01-01 timestamps,
  fixed entry order), so the same source tree always yields a byte-identical archive.
  The reader enforces zip-slip and decompression-bomb guards before anything is written.
- `path_safety`: the single lexical rule (no `..`, no absolute prefix, no backslash, no NUL,
  no drive letter) applied to both ZIP entry names and `[[script]].path`.

Deliberately depends only on `zymbol-ast`/`zymbol-lexer`/`zymbol-parser` — it never compiles
or executes Zymbol code, so a future package manager or the LSP can use it without pulling in
`zymbol-interpreter`/`zymbol-vm`/`zymbol-compiler` and their dependency trees.

**Dependencies**: `zymbol-ast`, `zymbol-lexer`, `zymbol-parser`, `zip`, `toml`, `semver`

#### `zymbol-cli`
Main binary (`zymbol`). Orchestrates the full pipeline.

**Dependencies**: all crates

---

## Execution Pipelines

### Tree-Walker (`--tw` flag)

```
Source file
    │
    ▼
zymbol-lexer        tokenize()  →  Vec<Token>
    │
    ▼
zymbol-parser       parse()     →  Program (AST)
    │
    ▼
zymbol-semantic     analyze()   →  diagnostics (warnings/errors)
    │
    ▼
zymbol-interpreter  execute()   →  stdout / return value
```

### Register VM (default)

```
Source file
    │
    ▼
zymbol-lexer  →  zymbol-parser  →  Program (AST)
                                        │
                                        ▼
                              zymbol-semantic  →  diagnostics
                                        │
                                        ▼
                              zymbol-compiler  →  CompiledProgram
                                                   (Chunk[], strings[])
                                        │
                                        ▼
                                   zymbol-vm  →  stdout / return value
```

---

## Crate Dependency Graph

```
zymbol-span ──────────────────────────────────────────────────┐
zymbol-common ────────────────────────────────────────────────┤
                                                              │
zymbol-error (span)                                          │
zymbol-lexer (span, common, error)                           │
zymbol-ast   (lexer, span, common)                           │
zymbol-parser (lexer, ast, common, error, span)              │
zymbol-semantic (ast, error, span, common)                   │
                                                              │
zymbol-interpreter (ast, parser, lexer, span, error, semantic)│
                                                              │
zymbol-bytecode (none)                                        │
zymbol-compiler (bytecode, ast, parser, lexer, span)          │
zymbol-vm (bytecode, common, lexer, intrinsics)               │
zymbol-host (vm, bytecode, common, intrinsics)                │
                                                              │
zymbol-formatter (ast, parser, lexer, span, error)            │
zymbol-analyzer  (ast, parser, lexer, semantic, span, error)  │
zymbol-lsp       (analyzer, formatter)                        │
zymbol-repl      (lexer, parser, interpreter, error, span)    │
zymbol-standalone                                             │
zymbol-package   (ast, lexer, parser)                         │
zymbol-cli       (all of the above) ─────────────────────────┘
```

---

## CLI Reference

| Command | Description |
|---------|-------------|
| `zymbol run FILE` | Execute with the register VM (default since v0.0.11) |
| `zymbol run --tw FILE` | Execute with the tree-walker |
| `zymbol run --vm FILE` | The same as `zymbol run FILE`: accepted because scripts written before v0.0.11 spell it |
| `zymbol run --keys KEYS FILE` | Run a full-screen program with no terminal: `>>\|` draws on a virtual 24×80 screen, `<<\|` reads its keys from `KEYS`, and each block's last frame goes to the output as text. Tree-walker and VM |
| `zymbol run --count FILE` | After the program ends, one line on stderr — `steps`, `calls`, `allocs`, `bytes`, `copied` — exact, the same on every run. Tree-walker and VM (not the browser engine) |
| `zymbol run FILE [ARGS]` | Pass CLI arguments to program (`><` capture) |
| `zymbol run PKG.zyp [--script NAME] [--tw\|--vm] [--keep-temp]` | Run a `.zyp` package. Extracts to an ephemeral temp dir and runs from there — never `chdir`s. The VM unless told otherwise, as for a loose `.zy` file; precedence is `--tw` > `--vm` > manifest `mode` > VM |
| `zymbol package DIR [-o OUT.zyp] [--script NAME]... [--dry-run]` | Package source (+ its transitive dependency closure) into a portable `.zyp`. `--dry-run` prints the closure and warnings without writing the archive |
| `zymbol build FILE -o OUT [--release]` | Package into standalone executable (bundles source + interpreter; not native compilation). Requires: Rust/Cargo installed, full repo checkout, run from `interpreter/` |
| `zymbol check FILE` | Syntax and semantic check of the whole program: the file **and every module it imports**, transitively. Errors in a module are reported at the module's own line, followed by `note: reached from <importer>`. Style warnings (unused variables, ambiguous lifetimes) stay with the file named on the command line |
| `zymbol fmt FILE [--write] [--check] [--indent N]` | Format source code |
| `zymbol repl` | Start interactive REPL |
| `zymbol-lsp` | Start LSP server (stdio transport) |

`zymbol build` and `zymbol package` are unrelated: `build` produces a native executable that
embeds the interpreter, `package` produces a `.zyp` archive of source that still needs a
`zymbol` binary to run.

---

## Feature Parity: Tree-Walker vs VM

| Feature | Tree-walker | VM |
|---------|:-----------:|:--:|
| Variables (`=`, `:=`) | ✓ | ✓ |
| Arithmetic / Comparison | ✓ | ✓ |
| Control flow (`?`, `_?`, `_`) | ✓ | ✓ |
| Loops (`@`, `@!`, `@>`, range) | ✓ | ✓ |
| Functions (decl + lambda) | ✓ | ✓ |
| Closures (outer scope capture) | ✓ | ✓ |
| Arrays and collection ops | ✓ | ✓ |
| Tuples (positional + named) | ✓ | ✓ |
| Strings (interpolation + ops) | ✓ | ✓ |
| Match (`??`) | ✓ | ✓ |
| Match or-patterns (`p1 \|\| p2`) | ✓ | ✓ |
| Error handling (`!?`, `:!`, `:>`) | ✓ | ✓ |
| I/O (`>>`, `<<`) | ✓ | ✓ |
| Bash execution (`<\ \>`) | ✓ | ✓ |
| Script execution (`</ />`) | ✓ | ✓ |
| Module system (`#`, `<#`, `#>`) | ✓ | ✓ |
| CLI args capture (`><`) | ✓ | ✓ |
| Format expressions (`#,\|x\|`, `#^.N\|x\|`) | ✓ | ✓ |
| Base conversion (`0b`, `0x`, etc.) | ✓ | ✓ |
| Pipe operator (`\|>`) | ✓ | ✓ |
| Native stdlib (`std/*`, incl. `std/term`) | ✓ | ✓ |
| Auto-free (destruction at last use) | ✓ | ✓ (see note) |
| Error values (`##Kind("…")`, `##Kind(m) =>`) | ✓ | ✓ |
| Int → Char cast (`##'expr`) | ✓ | ✓ |
| Virtual screen (`--keys`) and counters (`--count`) | ✓ | ✓ |

Measured 2026-10-20 on the v0.0.11 branch: `zyq consensus --engines zytw,zyvm` (what
`tests/scripts/vm_compare.sh` now delegates to) reports **708 of 714 corpus files agreeing
and 0 diverging**, the other 6 excused for every engine by a reason declared in
`zyquality/corpus.toml`. Adding `zyjs` does not change the agreement count — all three
engines reach the same 708. Against the browser engine alone the pair agrees on 695, the
19 in the difference being the files `corpus.toml` excuses (`std/db` is ODBC, `<\ cmd \>`
has no browser equivalent, TUI needs a real TTY, and the 6 excused for every engine) —
excused, not divergent. Agreement on a corpus is not the whole question: ZyDDT's matrix
asks the engines 1 722 declared cells (2026-10-21), and all three agree on every one.

**No corpus file carries an in-file skip marker.** `@vm-skip` was one of five incompatible
exclusion mechanisms that `zyquality/corpus.toml` replaced; nothing reads it any more.
Every exclusion is declared there, naming the engine, a tag and a required reason — an
exclusion nobody justified cannot be told from a bug somebody hid.

**Auto-free note**: both engines implement it, and they agree — for a named variable (two
arrays used in sequence cost 1.00 of one, in both) and, since 2026-10-19, for the value a call
returns and the caller uses once. That second shape cost 1.79× one in the VM against 1.00 in
the tree-walker on 2026-10-11 (ZYVM-022: `emit_auto_free` cleared the *named* variable's
register, and the call's result stayed in a temporary until something reused it). The fix
clears, at the end of the statement, only the call results that may be a collection — +0.05 %
of the steps of `zyquality/bench/big` where clearing every temporary cost +18.7 %. The cost case
`autofree/call-result` in `zyquality/cost` holds it: 0.96 on the VM, 0.99 on the tree-walker
(2026-10-20). The VM still uses about six times less memory per array.

---

## Performance Notes

The microbenchmarks are `zyquality/bench/`. Their wall times move by tens of percent
between two runs on one day — different rows each time — so the figures below are **CPU
instructions in user mode** (`perf stat -e instructions:u`, release build, median of seven
runs, measured 2026-10-11 on v0.0.10), which is what a gate can hold; `zymbol run --count`
is the same idea built into the engines. Instructions say how much work was done, not how
long it took, and the two ratios need not match:

| Benchmark | Tree-walker | VM | Tree-walker ÷ VM |
|-----------|:-----------:|:--:|:----------------:|
| Collections | 122 M | 49 M | 2.5× |
| Index read | 977 M | 321 M | 3.0× |
| Strings | 276 M | 78 M | 3.5× |
| Stress loop | 1 983 M | 416 M | 4.8× |
| Match | 1 610 M | 276 M | 5.8× |
| Recursion (`fib`) | 16 747 M | 1 777 M | **9.4×** |

**There is no single speedup factor, and the widely-quoted "~4×" is not its middle.** The
microbenchmarks span 2.5×–9.4× in instructions (on v0.0.9 the same programs read 1.4×–6.1×
in milliseconds); real programs sit far above them in time, because the tree-walker's cost
is per call frame and per scope, and the microbenchmarks are shallow.

**Not re-measured for v0.0.10.** The table below was taken on 2026-09-07, on v0.0.9, in
seconds, on programs that have changed since; read it as the order of magnitude it was.

| Real workload (Chaturanga) | Tree-walker | VM | VM speedup |
|---------------|:-----------:|:--:|:----------:|
| `perft(3)`, 4448 nodes | 1.94 s | 0.171 s | **11x** |
| full alpha-beta suite | 12.78 s | 0.989 s | **13x** |

The go engine (囲碁) reports 4.1× at 19×19 on its seeded self-play game;
alpha-beta search sits higher still, because it is recursion with output
parameters and array indexing in the innermost loop — exactly where the
tree-walker pays most, once per frame and once per scope. Quote the workload,
not a bare multiplier.

**Those two rows read 6.11 s / 42× and 43.5 s / 46× until 2026-09-02**, and the
go row read 8–14×. The VM did not change. The tree-walker copied aggregates it
had no reason to copy, in two places, and both are now closed (zy-GO's HLZ-012
and HLZ-014): reading one element cloned the whole collection, and so did handing
it to a function. `Value` now holds `Array`/`Tuple`/`NamedTuple` behind an `Rc`
and copies **when written** (`Rc::make_mut`, 32 sites) — the model this VM has
had all along, ported to the other engine. The tree-walker came out 3.4× faster
on this workload with nothing on the VM side moving at all.

A ratio between two engines measures both of them, on the day it was taken. Take
it again before quoting it.

An earlier revision of this table reported Collections as ~14 s under the
tree-walker (a HashMap-clone-per-scope limitation). That was fixed; it is 71 ms
now, and the entry had outlived the defect it described.

Key VM optimizations implemented:
- **Sprint 5C**: flat register stack — zero allocation per call frame
- **Sprint 5D**: `sizeof(Value)` reduced from 40 to 16 bytes via `Rc<T>` payloads
- **Sprint 5D+**: string pool pre-interning, slim `FrameInfo` (~40 bytes), lazy error boxing
- **Immediate operands**: `AddIntImm`, `CmpLeImm` — eliminates register loads for constants
- **Sprint v0.0.10**: one dispatch loop (the callbacks of `$>` no longer run on a second one);
  `<~` lends the caller's collection instead of copying it in (2 000 calls over 16 000
  elements: 1 204 M instructions → 19 M); a module's state is edited where it lives
  (ZYVM-003); a register write drops the old value only if it owns memory, and a frame holds
  what the function needs at once (ZYVM-010)
