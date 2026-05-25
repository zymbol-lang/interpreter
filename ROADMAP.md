# Zymbol-Lang — Roadmap

> Current status: **v0.0.6 (in progress)** — stdlib infrastructure complete.
> `std/math` (13 functions + PI, E) and `std/random` (xoshiro256++) shipped.
> Semantic analyzer updated. 9 stdlib tests passing.
> Remaining v0.0.6 items: float formatting, typed input, depuration polish.

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

#### Bytecode File Format (`.zyb`)

Introduce a first-class bytecode format so compiled programs can be distributed and
executed independently of the source code and compilation pipeline.

**Architecture:**
```
file.zy  ──►  zymbol compile file.zy -o file.zyb   ──►  file.zyb
                                                           │
                                         ┌─────────────────┴──────────────────┐
                                         ▼                                     ▼
                               zymbol run --vm file.zyb              standalone binary
                               (skip lex/parse/compile,              (embed .zyb bytes
                                detect .zyb by extension)             + minimal VM stub)
```

**Implementation steps:**

1. **Serde on `zymbol-bytecode`** — add `Serialize`/`Deserialize` to `CompiledProgram`
   and all its types. Use `bincode` or `postcard` for a compact binary format.

2. **`zymbol compile` subcommand** — new CLI command that runs lex → parse → compile
   and writes the result as `file.zyb`.

3. **`zymbol run --vm file.zyb`** — detect `.zyb` extension, deserialize directly into
   `CompiledProgram`, skip lex/parse/compile entirely.

4. **Standalone refactor** — `zymbol build` compiles to bytecode at build time and
   embeds the `.zyb` bytes in the binary. The generated executable only links
   `zymbol-bytecode` + `zymbol-vm`, dropping lexer/parser/AST/compiler from the
   standalone. Reduces dead weight and improves startup time.

This is also the foundation for the future LLVM backend — `.zyb` becomes the
intermediate representation handed off to the native code compiler.

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

| Module | Description | Status |
|--------|-------------|--------|
| `std/math` | `sqrt exp ln log pow sin cos abs max min floor ceil round` + `PI E` | ✅ v0.0.6 |
| `std/random` | `entero rango peso_f64` (xoshiro256++) | ✅ v0.0.6 |
| `std/io` | File read/write, path utilities | planned |
| `std/env` | Environment variables, OS info | planned |
| `std/json` | JSON parse and serialize | planned |
| `std/net` | HTTP client (basic) | planned |
| `std/string` | Advanced string utilities | planned |
| `std/time` | Timestamps, duration, formatting | planned |

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

## v0.0.6 Roadmap — Refinement & Scientific Stdlib

> **Focus:** polish of existing features + stdlib foundation for scientific computing.
> Primary drivers: general depuration + **Zofía** (first scientific computing project
> in Zymbol — tensors, neural networks, transformer encoder from scratch).
> Zofía's `HALLAZGOS.md` is the living gap tracker that feeds this milestone.
> No new syntax in core language — improvements are additive.

### Depuration (polish)

| Item | Area | Description |
|------|------|-------------|
| Error suggestions | Diagnostics | `"undefined 'funciom'"` → `help: did you mean 'funcion'?` |
| `zymbol check` exit code | CLI | Non-zero exit on warnings — enables CI pipelines |
| REPL persistent history | REPL | Persist `~/.zymbol_history` across sessions |
| `--quiet` flag | CLI | Suppress startup banner — clean output in scripts |
| `--time` flag | CLI | Print execution time — `zymbol run --time file.zy` |
| `--no-color` flag | CLI | Disable ANSI output — for piped output / log files |
| LSP module completion | LSP | Autocomplete module names in `<#` and `alias::` calls |
| VM: remaining parity | VM | Cover the 5% gaps exposed by Zofía's tensor operations |

### New Features (driven by Zofía HALLAZGOS)

#### ✅ `std/math` — mathematical functions  ← GAP-Z001, GAP-Z002  **[DONE]**

Shipped in v0.0.6. Names follow the international standard (C / Python / Rust).
For localized names use the i18n three-layer pattern — re-export under the
target language's names (see `Zofia/modulos/matematica_std.zy` for the Spanish adapter).

```zymbol
<# std/math => mat

mat::sqrt(x)          -- raíz cuadrada
mat::exp(x)           -- e^x
mat::ln(x)            -- logaritmo natural
mat::log(x)           -- logaritmo natural (alias)
mat::log(x, base)     -- logaritmo en base arbitraria
mat::pow(base, exp)   -- base^exp  (Float)
mat::sin(x)           -- seno (radianes)
mat::cos(x)           -- coseno (radianes)
mat::abs(x)           -- |x|  (Int→Int, Float→Float)
mat::max(a, b)        -- máximo escalar
mat::min(a, b)        -- mínimo escalar
mat::floor(x)         -- entero inferior
mat::ceil(x)          -- entero superior
mat::round(x)         -- redondeo al más cercano
mat.PI                -- 3.141592653589793
mat.E                 -- 2.718281828459045
```

- **Int → Float promotion:** `mat::sqrt(4)` → `2.0` — all functions accept `###`.
- **Implementation:** `crates/zymbol-interpreter/src/stdlib/math.rs` — thin Rust
  wrappers over `f64` stdlib methods. `enum FunctionDef::Native` dispatch. No new syntax.
- **Tests:** `interpreter/tests/stdlib/stdlib_math_*.zy` (6 test files, all passing).

#### ✅ `std/random` — pseudo-random number generation  ← GAP-Z003  **[DONE]**

Shipped in v0.0.6. Uses xoshiro256++ with thread-local state, auto-seeded from
`SystemTime` on first call. No seed object needed from Zymbol's perspective.

```zymbol
<# std/random => rnd

rnd::entero(min, max)  -- Int en [min, max]  (uniforme)
rnd::rango(n)          -- Int en [0, n-1]    (uniforme)
rnd::peso_f64()        -- Float en [-0.1, 0.1]  (inicialización de pesos NN)
```

- **Implementation:** `crates/zymbol-interpreter/src/stdlib/random.rs`.
- **Tests:** `interpreter/tests/stdlib/stdlib_random_*.zy` (3 test files, all passing).

#### Float formatting in `>>`  ← GAP-Z004, IDEA-Z002

Zofía's tensor printer outputs `0.3333333333333333` — unreadable for
educational output. A format modifier on `>>`:

```zymbol
>> x :#4    -- 4 decimal places: "0.3333"
>> x :#2e   -- scientific notation 2 dec: "3.33e-01"
>> x :#0    -- integer truncation: "0"
```

Applies only to numeric values. Ignored for strings/booleans/etc.

#### ~~`$@` functional map operator~~  ← IDEA-Z003  **[DISCARDED]**

Redundant with the existing `$>` map operator, which already covers every
use case identified in Zofía:

```zymbol
resultado = vec$> relu_escalar          -- referencia a función nombrada
resultado = vec$> (v -> mat::exp(v))    -- lambda inline
exps      = vec$> (v -> mat::exp(v))    -- softmax step 1
```

Adding `$@` would be duplicate syntax with no benefit. Discarded.

#### Typed input constraints  ← IDEA-Z004

Restrict `<<` at the operator level — invalid characters rejected on keypress,
no manual validation code needed. Uses existing Zymbol type symbols (no English
letters — language-agnostic):

```zymbol
<< :###4    "Capas (1–99): "      -- integer, up to 4 digits
<< :+###4   "Dimensión: "         -- positive integer, up to 4 digits
<< :+##.4   "Tasa aprendizaje: "  -- positive decimal, up to 4 decimal places
<< :##"30   "Etiqueta: "          -- text, max 30 chars
```

### Zofía integration checkpoints

| Zofía Phase | Unblocked by | Estado |
|-------------|-------------|--------|
| Fase 1 — tensor | Depuration items (float format) | pendiente |
| Fase 2 — grad | No new Zymbol features needed | pendiente |
| Fase 3 — activacion | `std/math` (`exp`, `log`) | ✅ desbloqueado |
| Fase 4 — atencion | `std/math` (`sqrt`) | ✅ desbloqueado |
| Fase 5 — transformador | `std/math` (`sin`, `cos`, `pow`) + `std/random` (`peso_f64`) | ✅ desbloqueado |

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
| v0.0.3 | i18n + safe access | Unicode grapheme clusters, safe access `?.`, null coalescing |
| v0.0.4 | Module system + REPL | Full module import/export, circular import detection, REPL improvements |
| v0.0.5 | TUI + VM parity | `>>|` `>>~` `>>!` `>>?` `<<\|` `@~`, `$*` string repeat, tuple equality VM, LSP `<<\|` fix, text styles `>>~(,,BKS,fg,bg)` |
| **v0.0.6** | **Refinement + Scientific Stdlib** | `std/math` (sqrt/exp/ln/log/pow/sin/cos/abs/max/min/floor/ceil/round + PI/E), `std/random` (xoshiro256++), stdlib infrastructure (`enum FunctionDef`, `load_stdlib_module`, semantic suppression), 9 stdlib tests — **driven by Zofía**; float formatting + typed input pending |
