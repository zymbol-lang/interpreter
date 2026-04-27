# Zymbol-Lang — Roadmap

> Current status: **v0.0.2** — interpreter feature-complete, dual execution modes,
> 243/246 VM parity tests passing (3 failures: HTTP client + CLI args not yet in VM).
> Collection API v0.0.2 + destructuring. **1-based indexing** across all collections.

---

## What's Done

### Core Language (complete)

| Feature | Status |
|---------|--------|
| Variables (`=`) and constants (`:=`) | ✅ |
| All primitive types: Int, Float, String, Char, Bool, Array, Tuple | ✅ |
| Arithmetic, comparison, logical operators | ✅ |
| Compound assignment (`+=`, `-=`, `*=`, `/=`, `%=`, `^=`, `++`, `--`) | ✅ |
| String interpolation in any context | ✅ |
| Output `>>` (multi-item juxtaposition) | ✅ |
| Input `<<` with prompt | ✅ |
| CLI args capture `><` | ✅ |
| Control flow: `?` / `_?` / `_` | ✅ |
| Match `??` (literal, range, guard `_?`, wildcard) | ✅ |
| All loop forms: infinite, while, for-each, range | ✅ |
| Range step and reverse range | ✅ |
| Labeled loops with `@!` / `@>` | ✅ |
| Functions with isolated scope | ✅ |
| Output parameters `<~` (pass by reference) | ✅ |
| Lambdas with implicit and explicit return | ✅ |
| Closures (outer scope capture) | ✅ |
| Higher-order functions: `$>` map, `$|` filter, `$<` reduce | ✅ |
| Pipe operator `\|>` with placeholder `_` | ✅ |
| Arrays: full CRUD + direct index update | ✅ |
| Array positional insert `$+[i]` | ✅ |
| Array positional remove `$-[i]`, range `$-[i..j]` | ✅ |
| Array remove-all `$--`, find-all positions `$??` | ✅ |
| Negative indices `arr[-1]` (tree-walker + VM parity) | ✅ |
| **1-based indexing** — `arr[1]` is first element; index 0 = runtime error | ✅ |
| Sort `$^+` (ascending) / `$^-` (descending), natural + custom comparator | ✅ |
| Destructuring assignment: `[a, b, *rest] = arr`, `(name: n) = t` | ✅ |
| Named tuples with `.field` access | ✅ |
| String operators: split, slice, find, insert, remove, replace | ✅ |
| Error handling: `!?` / `:!` / `:>` with typed catch | ✅ |
| Module system: `#` / `#>` / `<#` with aliases | ✅ |
| Data operators: `#|x|`, `x#?`, `#.N|x|`, `#!N|x|`, `c|x|`, `e|x|` | ✅ |
| Base literals and conversions: `0x`, `0b`, `0o`, `0d` | ✅ |
| Shell execution: `<\ cmd \>` (BashExec) and `</ file.zy />` | ✅ |
| Explicit variable lifetime: `\ var` | ✅ |

### Execution Modes (complete)

| Component | Status | Notes |
|-----------|--------|-------|
| Tree-walker interpreter | ✅ | Default mode, best error messages |
| Scope pool recycling | ✅ | Zero allocation per scope push/pop |
| Tail-call optimization (TCO) | ✅ | Detects `<~ f(same_args)` restart |
| Register VM | ✅ | `--vm` flag, 4.4× faster than tree-walker on fib(35) |
| Flat register stack | ✅ | All frames share one `Vec<Value>`, zero alloc per call |
| `sizeof(Value)` = 16 bytes | ✅ | Via `Rc<T>` heap payloads (was 40 bytes) |
| String pool pre-interning | ✅ | `LoadStr` = O(1) `Rc::clone` |
| Immediate operands | ✅ | `AddIntImm`, `CmpLeImm`, etc. |
| Closures in VM | ✅ | `MakeClosure` + `collect_free_vars()` |

### Tooling (complete)

| Tool | Status |
|------|--------|
| CLI: `run`, `build`, `check`, `fmt`, `repl` | ✅ |
| Interactive REPL with history | ✅ |
| Code formatter | ✅ |
| LSP server (diagnostics, symbols, hover, go-to-def) | ✅ |
| VS Code extension | ✅ |
| Standalone executable builder | ✅ |
| Install script | ✅ |

### Test Coverage (complete)

| Suite | Status |
|-------|--------|
| 94 E2E tests (47 tree-walker + 47 VM) | ✅ PASS |
| VM parity check (vm_compare.sh) | ✅ 159/159 PASS |
| RosettaStone i18n suite (105 languages) | ✅ PASS |

---

## Known Gaps (open issues)

These are language features defined in the EBNF spec that are not yet implemented.
They are documented in the manual as known limitations.

### Language

| Gap | Description | Workaround |
|-----|-------------|------------|
| **Match multi-value arms** | `1, 2 : "low"` syntax not parsed | Use guard: `_? n == 1 \|\| n == 2 : "low"` |
| **Match identifier binding** | `n : n * 2` pattern not supported | Use guard or extract value before match |
| **Module constant access** | `alias.CONST` fails at runtime | Use getter function: `alias::get_CONST()` |
| **HOF with lambda variable** | `arr$> fn` where `fn` is a variable | Wrap: `arr$> (x -> fn(x))` |
| **Named functions as values** | `f = myFunc` fails | Wrap: `f = x -> myFunc(x)` |
| **CLI args in VM mode** | `><` capture not implemented in VM | Use tree-walker for CLI arg programs |
| **`$!!` from lambdas** | Error propagation only works in named functions | Wrap lambda body in a named function |
| **`do-while ~>`** | Post-condition loop syntax defined in EBNF, not parsed | Infinite loop with `@!` break at end |

### Static Analyzer False Positives

| Warning | Cause |
|---------|-------|
| `unused variable` for interpolation `"{x}"` | Analyzer does not track string interpolation usage |
| `unused variable` for BashExec `<\ {x} \>` | Analyzer does not track BashExec variable usage |
| `arithmetic on non-numeric` for string `/` split | Analyzer cannot distinguish `/` operators by context |
| `type mismatch` for `arr[i] = val` | Analyzer does not model indexed assignment |

---

## Next Steps

### Near Term

#### Fix known language gaps

- **Match multi-value arms**: extend parser to accept `val1, val2 : expr` arm syntax
- **Match identifier binding**: extend AST to support `ident : body` pattern
- **Module constant access**: fix `alias.CONST` lookup in module scope resolver

#### Fix static analyzer false positives

- Track variable usage inside string interpolation expressions
- Track variable usage inside BashExec template strings
- Distinguish string split `/` from arithmetic `/` in type checker
- Model `arr[i] = val` as a mutation rather than a type mismatch

#### VM completeness

- **CLI args capture `><`** in VM mode (parity with tree-walker)
- **Module system in VM**: full parity with tree-walker for `<#` imports
- **Format expressions in VM**: `e|x|`, `c|x|` full parity (`#.N|x|` already working)

### Medium Term

#### Performance

- **Bytecode disk cache (`.zyc` files)**
  Serialize `CompiledProgram` to disk with `bincode`. On re-run, check hash and skip
  compilation if source unchanged. Target: startup 15–40ms → ~2ms.

- **Recursion performance in VM**
  Root cause: frame allocation cost on deep call stacks. Strategy: pre-allocate frame
  pool, reduce `Box` allocations in `FrameInfo`.

- **DCE (Dead Code Elimination) improvements**
  Sprint 5I added a basic DCE pass. Extend to eliminate unused variables across
  function boundaries and in HOF chains.

#### Language extensions

- **Array type inference relaxation**: allow mixed-type arrays with dynamic dispatch
  (currently requires homogeneous element types)
- **Module constants via `.`**: complete the `alias.CONST` access path
- **`$!!` error propagation from lambdas**: currently limited to named functions; extend
  to propagate through the lambda's call frame to its immediate caller
- **`do-while ~>` post-condition loop**: implement EBNF rule `block ~> expr`; parser
  and both interpreters (tree-walker + VM) need to handle the new AST node

### Long Term

#### JIT Compilation (Cranelift backend)

Planned as Sprint 5E in the VM perf roadmap. Use `cranelift-jit` to compile hot
functions to native code at runtime. Target: maximum throughput on all benchmarks,
including recursion.

Architecture:
```
CompiledProgram
    │
    ├── Cold path  →  VM interpreter (current)
    └── Hot path   →  Cranelift JIT → native code
```

#### LLVM Backend

Ahead-of-time compilation to native executables via LLVM. Target: use cases requiring
maximum performance or deployment without the Zymbol runtime.

#### Standard Library

Built-in modules accessible via `<#`:

| Module | Description |
|--------|-------------|
| `std/io` | File read/write, path utilities |
| `std/math` | sqrt, floor, ceil, sin, cos, log, etc. |
| `std/string` | Advanced string utilities |
| `std/time` | Timestamps, duration, formatting |
| `std/net` | HTTP client (basic) |
| `std/json` | JSON parse and serialize |
| `std/env` | Environment variables, OS info |

#### Package Manager

A minimal package manager for sharing Zymbol modules:

- `zymbol add user/package` — install from GitHub
- `zymbol.toml` — project manifest
- Local and remote module resolution
- Semantic versioning

#### Language Server Improvements

- Completion (autocomplete for variables, functions, module exports)
- Rename symbol across files
- Find all references
- Inlay hints (type annotations on hover)

---

## Performance Targets

Current benchmarks (release build, post-Sprint 5D+):

| Benchmark | Tree-walker | VM (now) | VM (target) |
|-----------|:-----------:|:--------:|:-----------:|
| Stress | ~200ms | **67ms** | <60ms |
| Match | ~165ms | **50ms** | <50ms |
| Collections | ~14s | **33ms** | <30ms |
| Strings | ~43ms | 36ms | <25ms |
| Recursion | ~1480ms | 308ms | <200ms |

Recursion and strings are the remaining performance targets.
Both are addressed by the Cranelift JIT milestone.

---

## Version History

| Version | Milestone | Description |
|---------|-----------|-------------|
| Sprint 1–3 | Foundation | Lexer, parser, AST, basic interpreter |
| Sprint 4A–4B | Register VM | `zymbol-bytecode`, `zymbol-compiler`, `zymbol-vm` |
| Sprint 4C | E2E coverage | 88/88 tests passing |
| Sprint 4D–4H | VM Parity | 99/99 vm_compare PASS |
| Sprint 5B–5C | VM performance | Flat register stack, scope pool recycling |
| Sprint 5D–5D+ | VM memory | `sizeof(Value)` 40→16 bytes, string pool, slim frames |
| Sprint 5I | Language complete | Indexed assign, comma concat, guard patterns, range step, BaseConvert, labeled loops |
| v0.0.2 | Collection API + destructuring | `$+[i]` `$-` `$--` `$-[i]` `$-[i..j]` `$??` `$^+` `$^-`, negative indices normalized, destructuring assignment |
