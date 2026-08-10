# Zymbol-Lang — Interpreter Architecture

## Overview

Zymbol-Lang is a minimalist symbolic programming language with no keywords. This document
describes the architecture of its Rust implementation: a workspace of 19 crates organized
by compilation phase, execution mode, and tooling.

The interpreter supports two independent execution strategies:

- **Tree-walker** (default): walks the AST directly, scope-stack based
- **Register VM** (experimental, `--vm` flag): compiles AST to bytecode and runs it on a
  register-based virtual machine

---

## Design Principles

1. **Modularity** — each compilation phase is an isolated crate with clear boundaries
2. **Dual execution** — tree-walker and VM coexist without shared runtime state
3. **Unicode-first** — identifiers, strings, and operators support full Unicode including emojis
4. **No keywords** — all language constructs use pure symbolic operators
5. **Explicit over implicit** — no hidden coercions, no automatic newlines, no magic

---

## Workspace Structure

```
interpreter/
├── Cargo.toml                  # Workspace manifest (19 members)
├── Cargo.lock
├── zymbol-lang.ebnf            # Formal grammar (EBNF) — v3.1.0, generated 2026-06-12
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
│   ├── zymbol-intrinsics/      # Unboxed string/collection intrinsics (VM fast paths)
│   ├── zymbol-formatter/       # Code formatter
│   ├── zymbol-analyzer/        # LSP analysis engine
│   ├── zymbol-lsp/             # Language Server Protocol (tower-lsp)
│   ├── zymbol-repl/            # Interactive REPL
│   ├── zymbol-package/         # .zyp source archives (manifest, closure, ZIP I/O)
│   ├── zymbol-cli/             # Main CLI entry point
│   └── zymbol-standalone/      # Standalone executable builder
├── tests/                      # End-to-end test suite
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
Walks the AST directly and evaluates it. This is the default execution mode.

**Runtime `Value` enum**:
```rust
enum Value {
    Int(i64), Float(f64), Char(char), Bool(bool),
    String(String),
    Array(Vec<Value>),
    Tuple(Vec<Value>),
    NamedTuple(Vec<(String, Value)>),
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
  `std/net`, `std/db` (v0.0.7); `std/term` (v0.0.8). Each module registers
  `FunctionDef::Native` entries; `std/*` import paths resolve in-process (no filesystem
  lookup)
- Auto-free (v0.0.8): a variable is destroyed right after the statement holding its last
  use, not at scope end. Schedules come from `zymbol_semantic::last_use` and are stored per
  body in `FunctionDef::Zymbol::auto_free`, applied by `execute_body_scheduled`. Unobservable
  by design — it only lowers peak memory
- Bash execution: `<\ command \>` captures stdout + stderr via `Command::output()`
- Error handling: try/catch/finally with typed catch (`:! ##IO`, `:! ##Parse`, etc.)

**Dependencies**: `zymbol-ast`, `zymbol-parser`, `zymbol-lexer`, `zymbol-span`,
`zymbol-error`, `zymbol-semantic`

---

### Register VM Execution

#### `zymbol-bytecode`
Instruction set definition. ~60 instruction types organized by category:

| Category | Instructions |
|----------|-------------|
| Load | `LoadInt`, `LoadFloat`, `LoadBool`, `LoadStr`, `CopyReg` |
| Arithmetic | `AddInt`, `SubInt`, `MulInt`, `DivInt`, + immediate variants (`AddIntImm`, etc.) |
| Comparison | `CmpEq`, `CmpNe`, `CmpLt`, `CmpLe`, `CmpGt`, `CmpGe` + immediate variants |
| Strings | `ConcatStr`, `StrLen`, `StrSplit`, `StrContains`, `StrSlice`, `BuildStr` |
| Arrays | `NewArray`, `ArrayPush`, `ArrayGet`, `ArraySet`, `ArrayLen`, `ArraySlice` |
| Tuples | `MakeTuple`, `MakeNamedTuple`, `NamedTupleGet` |
| Control | `Jump`, `JumpIf`, `JumpIfNot`, `Call`, `TailCall`, `CallDynamic`, `Return` |
| Closures | `MakeClosure`, `CallDynamic` |
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
    base: usize,          // start of this frame's registers in value_stack
    next_base: usize,     // start of next frame
    catch_ip: Label,      // Label::MAX = no active catch
    error_info: Option<Box<FrameError>>,  // lazy allocation
    // ...return bookkeeping
}

struct VM {
    value_stack: Vec<Value>,    // all frames share one Vec
    frame_stack: Vec<FrameInfo>,
    ip: usize,
    program: CompiledProgram,
}
```

**Key optimizations**:
- Flat value_stack: O(1) register access via `value_stack[base + reg]`
- String pool: `LoadStr` = O(1) `Rc::clone`
- Immediate operands: `AddIntImm`, `CmpLeImm`, etc.
- Lazy `Box<FrameError>`: no allocation unless a catch is hit
- Sentinel `catch_ip = Label::MAX` avoids per-instruction branch

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

### Tree-Walker (default)

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

### Register VM (`--vm` flag)

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
zymbol-vm (bytecode)                                          │
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
| `zymbol run FILE` | Execute with tree-walker (default) |
| `zymbol run --vm FILE` | Execute with register VM |
| `zymbol run FILE [ARGS]` | Pass CLI arguments to program (`><` capture) |
| `zymbol run PKG.zyp [--script NAME] [--tw\|--vm] [--keep-temp]` | Run a `.zyp` package. Extracts to an ephemeral temp dir and runs from there — never `chdir`s. Defaults to `--vm` (loose `.zy` files still default to the tree-walker); precedence is `--tw` > `--vm` > manifest `mode` > VM |
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

Measured on v0.0.9: `tests/scripts/vm_compare.sh` reports 551/551 files with byte-identical
output under both engines. The three rows that used to read "partial"/"—" (module system,
CLI args, format expressions) were verified and are at parity; the last known divergences
were closed by HLZ-008/009/010 and MM-10/MM-11.

Exactly one test carries `@vm-skip` — `tests/gaps/gap_key_input_type_check.zy` — and it is
skipped by design: it is a `zymbol check` test that never executes.

**Auto-free note**: both engines implement it, but the VM's peak-memory win is currently
smaller. `emit_auto_free` clears the *named* variable's register, while a temporary holding
the same large value lives until its register is reused. Closing that gap is a register
allocator change, not an analysis change.

---

## Performance Notes

Benchmarks vs CPython 3 (release build, post-Sprint 5D+):

| Benchmark | Tree-walker | VM | Python |
|-----------|:-----------:|:--:|:------:|
| Stress loop | ~200ms | 67ms | 77ms |
| Match | ~165ms | 50ms | 75ms |
| Collections | ~14s | 33ms | 44ms |
| Strings | ~43ms | 36ms | 25ms |
| Recursion (fib) | ~1480ms | 308ms | 218ms |

VM is 4.4× faster than tree-walker on `fib(35)`. Collections improvement is dramatic
(tree-walker limitation with HashMap cloning per scope).

Key VM optimizations implemented:
- **Sprint 5C**: flat register stack — zero allocation per call frame
- **Sprint 5D**: `sizeof(Value)` reduced from 40 to 16 bytes via `Rc<T>` payloads
- **Sprint 5D+**: string pool pre-interning, slim `FrameInfo` (~40 bytes), lazy error boxing
- **Immediate operands**: `AddIntImm`, `CmpLeImm` — eliminates register loads for constants
