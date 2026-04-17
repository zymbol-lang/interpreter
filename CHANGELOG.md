# Changelog

All notable changes to Zymbol-Lang are documented here.

Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)  
Versioning: [Semantic Versioning](https://semver.org/) (pre-1.0 series)

---

## [Unreleased]

---

## [0.0.4] — 2026-04-16

### Breaking Changes

- **1-based indexing across all collections** — `arr[1]` is the first element.
  Index `0` now raises a runtime error instead of silently returning a value.
  Affects: arrays, tuples, named tuples, and strings.
  Negative indices are preserved: `arr[-1]` still means last element.

### Added

**Multi-dimensional indexing** (`arr[i>j>k]`)
- Scalar deep access: `arr[i>j>k]` — navigate nested arrays to a single value.
- Flat extraction: `arr[p ; q]` or `arr[[i>j]]` — returns a flat `Array`.
- Structured extraction: `arr[[g] ; [g]]` — returns an `Array` of `Arrays`.
- Range steps: `arr[i..j > k]` — range over one navigation dimension.
- Nested ranges: `arr[[i..j] ; [k..l]]` — double fan-out.
- New MANUAL.md section §11c. New test directory `tests/index_nav/` (15 cases).
- Deprecated: chained `arr[i][j]` syntax (still works, no longer recommended).

**Type conversion casts**
- `##.expr` — convert to `Float`.
- `###expr` — convert to `Int` (round).
- `##!expr` — convert to `Int` (truncate).
- New tokens: `HashHashDot`, `HashHashHash`, `HashHashBang`.
- New test directory `tests/casts/` (6 cases).

**String operations**
- `string$/ delim` — split string by delimiter, returns `Array(String)`.
- `base$++ a b c` — ConcatBuild: concatenate/append multiple items in one expression.

**Interpolated string literal**
- New `Literal::InterpolatedString` variant — strings with `{var}` are distinguished
  at compile time. Literal braces are escaped with `\{` and `\}`.

**Module system**
- Circular import detection: raises a clear `RuntimeError::CircularImport` instead
  of a stack overflow. The detection set propagates transitively to sub-modules.
- Private functions in modules can now call each other (intra-module calls, BUG-01 fix).
- Re-export from another module via `ExportItem::ReExport` (used by i18n nested modules).

**VM — full parity (320/320 tests)**
- Module private mutable state: new instructions `LoadGlobal(Reg, u16)` and
  `StoreGlobal(u16, Reg)`, `global_vars: Vec<Value>` field in the VM, `GlobalInit`
  in `CompiledProgram`.
- Float type propagation: Sub/Mul/Div/Pow now set `StaticType::Float` so downstream
  operations select the correct Float instruction variant.
- Lambda support in HOF: ~40 missing instruction arms added to `call_function`
  hot-loop; `$>` map / `$|` filter / `$<` reduce with lambdas now work in `--vm` mode.
- List pattern compilation: `??` match with `[a, b, *rest]` patterns now compiles
  to bytecode.
- Unicode numeric eval: `normalize_unicode_digits` converts any of the 69 supported
  Unicode digit scripts to ASCII before `#|expr|` evaluation.

**Test suite**
- `tests/index_nav/` — 15 cases covering all navigation forms and error bounds.
- `tests/casts/` — 6 cases: to_float, to_int_round, to_int_trunc, expressions, errors.
- `tests/gaps/` — 8 cases: module const access, private state, export block position,
  BashExec edge cases.
- `tests/test_catch01–10` — 10 error-handling cases: basic, typed, finally, nested,
  loop, function, check, multiple, scope.
- `tests/scope01–05` — 5 scope cases: if block, nested blocks, loop block, match block,
  shadowing.
- 320 `.expected` files generated for the full VM parity suite.

**EBNF grammar** (`zymbol-lang.ebnf`, +226 lines)
- Formal rules: `nav_index`, `nav_path`, `nav_step`, `nav_atom`, `struct_group`.
- `numeric_cast_expr` rule for `##.`, `###`, `##!`.
- `index_suffix` updated: 1-based, negative indices supported.
- Comma-concat (`"a", b, "c"`) documented as removed; juxtaposition is canonical.

**Documentation** (MANUAL.md, +680 lines)
- New §11c Multi-dimensional Indexing.
- §4 Variables: subsections Variable Scope, Underscore Variables (`_name`),
  Explicit Lifetime End.
- §7 Match: List Patterns subsection.
- §11 Arrays: Negative Indices and Symmetric Slices subsection.
- §18 Data Operators: Type Conversion Casts subsection.
- §20 Known Limitations: L3 (module alias.CONST) and L4 (export block position)
  marked as Fixed.

### Changed

- All existing test cases in `tests/collections/`, `tests/lambdas/`, `tests/strings/`,
  and benchmarks migrated from 0-based to 1-based indexing.
- `packaging/publish-release.sh` and `packaging/templates/zymbol.wxs.in` updated.

### Fixed

- VM: arithmetic operations now propagate `StaticType::Float` correctly (was silently
  treating float results as Int in some compound expressions).
- Module constants: `take_variable` no longer corrupts module constants on write-back
  (was using a Unit sentinel; fix: `scope.remove(name)`).
- Limitation L3: `alias.CONST` now resolves correctly in all contexts.
- Limitation L4: `#>` export block can now appear after function definitions.

### VM performance — Sprint 5G: Small String Optimization (2026-04-17)

`Value::String` payload changed from `Rc<String>` (always heap) to `ZyStr` — an 8-byte
tagged-pointer type that stores strings ≤ 7 bytes inline (no heap allocation, no atomic ops)
and falls back to a raw `Rc<String>` pointer for longer strings.

**`ZyStr` encoding (little-endian, 8 bytes):**
```
Inline (byte[7] bit7 == 1): bytes[0..len] = UTF-8 data, byte[7] = 0x80 | len
Heap   (byte[7] bit7 == 0): bytes[0..8] as u64 (LE) = raw *const String from Rc::into_raw()
```
Valid on x86-64 / arm64 where user-space pointers have bit 63 == 0.

**Changes in `crates/zymbol-vm/src/zy_str.rs` (new file):**
- `ZyStr::new(String)`: wraps the `String` directly in `Rc` (1 allocation for heap strings).
- `ZyStr::from_str_ref(&str)`: inline if ≤ 7 bytes, otherwise `Rc::new(s.to_string())`.
- `ZyStr::clone` (heap): `Rc::increment_strong_count` — single atomic op, no intermediate Rc value.
- `ZyStr::drop` (heap): `drop(Rc::from_raw(ptr))` — decrements and frees when last owner.
- `Deref<Target = str>`: all `&str` methods available on `&ZyStr` without `.as_str()` calls.
- 11 unit tests: size_is_8_bytes, inline/heap boundary, clone safety, Deref, Unicode.

**Additional micro-optimizations applied in the same sprint:**
- `StrSplit`: changed `ZyStr::new(p.to_string())` → `ZyStr::from_str_ref(p)`. Short split
  parts (≤ 7 bytes) now go inline with zero allocation.
- `ArrayRemove` (Array arm): replaced `rc_arr.as_ref().clone()` + `Rc::new(arr)` with
  `std::mem::replace` + `Rc::make_mut` — mutates the Vec in-place when refcount == 1,
  clones only when shared.
- `BuildStr` (both dispatch sites): added `String::with_capacity(sum_of_lit_lens + 4×reg_parts)`
  pre-pass to avoid reallocation during string interpolation.

**Benchmark results (VM, 5-run min, release binary):**

| Benchmark | Sprint 5F | Sprint 5G | Delta |
|-----------|-----------|-----------|-------|
| Stress core | 80 ms | 69 ms | −11 ms |
| Pattern Match | 74 ms | 43 ms | −31 ms |
| Recursion | 261 ms | 279 ms | +18 ms |
| Collections | 38 ms | 36 ms | −2 ms |
| Strings | 25 ms | 33 ms | +8 ms |
| Strings Stress | 42 ms | 56 ms | +14 ms |
| Strings Modify | 49 ms | 57 ms | +8 ms |

*Sprint 5F numbers from single-run baseline; Sprint 5G numbers from 5-run min. Net: CPU-bound
benchmarks (Stress, Pattern Match, Collections) improve; string-heavy benchmarks are neutral
to slightly worse because the benchmark strings are mostly > 7 bytes (bypass inline SSO path).*

---

### VM performance — Sprint 5F (2026-04-16)

Targeted micro-optimizations to the register VM hot paths.

**`StrReplace` char pattern — heap alloc eliminated**
- `zymbol-vm/src/lib.rs` `StrReplace`: char pattern previously built a temporary
  `String::with_capacity(4)` before calling `str::replace`. Changed to pass `char`
  directly as a Rust `Pattern`, eliminating one heap allocation per call.
- `StrReplaceN`: same problem; refactored to use a local `enum Pat { Ch(char), Str(&str) }`
  avoiding `c.to_string()` for both the `max == 0` and the bounded-replace paths.

**`Call` instruction — resize strategy confirmed optimal**
- Investigated replacing `value_stack.resize(n, Value::Unit)` + unsafe indexed overwrite
  with individual `push` calls per argument. Benchmarks showed `push` × n is slower than
  `resize` + `get_unchecked_mut` because `resize` produces a single vectorizable fill loop
  and the unsafe writes have no per-element branch overhead. Reverted; comment updated to
  document the trade-off for future reference.

**Benchmark delta (VM, 3-run avg, release binary):**

| Benchmark | Before | After | Delta |
|-----------|--------|-------|-------|
| Strings Modify | 51 ms | 49 ms | −2 ms |
| Recursion | 271 ms | 261 ms | −10 ms |
| Pattern match | 49 ms | 44 ms | −5 ms |
| Others | — | — | ±noise |

**Remaining structural gap vs Python (strings):**
`StrSplit`, `StrReplace`, and `StrReplaceN` each wrap their result in `Rc::new(String)` —
one unavoidable heap allocation per call with the current `Value` representation. Python
delegates these to C extensions with SIMD internals and no boxing. Eliminating the gap
requires Small String Optimization (SSO) in the `Value` enum — tracked for Sprint 5G.

---

### Post-release fixes (2026-04-16 review)

Six bugs and gaps identified during the v0.0.4 review session, all resolved same day.
Full record: `tests/BUG_v0.0.4.md`.

**BUG-NEW-01 — `<\` inside `#|...|` breaks NumericEval** (regression, v0.0.4)
- Introducing `BashOpen` (`<\`) caused the lexer to tokenize `<\` even inside
  NumericEval context (`#|...|`), breaking `#|<\ date +%s%N \>| / 1000000`.
- Fix: shell commands containing non-Zymbol tokens must be quoted:
  `<\ "date +%s%N" \>`. All 7 benchmark scripts updated.
- `lib_time.zy` and all benchmark string output corrected to juxtaposition (not `+`).
- All 7 Python comparison benchmarks restored to full operation.

**BUG-NEW-02 — Bool as array index not catchable by `!?`** (regression, v0.0.4)
- `arr[bool]` terminated the process with exit code 1, bypassing the `!?`/`:!`
  try/catch machinery.
- Fix (`zymbol-semantic`): `Bool` added to allowed index types so static analysis
  passes and the error reaches the runtime.
- Fix (`zymbol-vm`): `ArrayGet` changed from `as_int()?` to `raise!(...)` so the
  error is catchable in VM mode.

**BUG-NEW-03 — Cast error messages differed between WT and VM** (regression, v0.0.4)
- `##.`, `###`, `##!` on non-numeric values produced different error text in each
  execution path.
- Fix (`zymbol-interpreter/data_ops.rs`): replaced `{:?}` with a `value_type()`
  helper that yields the type name only (no value content).
- Fix (`zymbol-vm`): added `VmError::CastError { op, got }` variant; cast
  instructions now raise it instead of the generic `TypeError`.
- Both paths now emit: `"##. requires a numeric value, got String"`.

**GAP-01 — `\ var` (Explicit Lifetime End) was a no-op** (unimplemented)
- `Statement::LifetimeEnd` handler was a placeholder that did nothing; MANUAL §4
  documented it as functional.
- Fix (`zymbol-interpreter`): handler now calls `destroy_variable()`.
- Fix (`zymbol-compiler`): emits `LoadUnit(r)` and removes variable from
  `register_map`, preventing post-destroy use at compile time.

**BUG-PRE-01 — Two `cargo test` failures in `zymbol-formatter`** (pre-existing)
- `test_format_loop` and `test_format_foreach_loop` used inputs without the required
  space after `@` (`@x<10{...}` instead of `@ x<10{...}`).
- Fix: test inputs corrected; `cargo test -p zymbol-formatter` now passes 52/52.

**BUG-PRE-02 — `test_string_literal_braces` asserted wrong layer output** (pre-existing)
- The lexer stores `\{` as the `\x01` sentinel (ASCII SOH) to prevent it from being
  consumed as a string-interpolation delimiter. The test expected the post-runtime
  resolved form (`{`) from the raw lexer token.
- Fix: assertion updated to `"Use \x01curly} braces literally"` with a comment
  explaining the two-phase design.

### Test suite — v0.0.4 final state

| Suite | Result |
|-------|--------|
| `cargo test` (all crates) | **717 / 717 pass** |
| `vm_compare.sh` (WT vs VM parity) | **350 / 350 pass** |
| `run_all.sh` (7 benchmark suites) | **7 / 7 pass** |

**Python comparison benchmarks** (`run_all.sh --python --runs 3`, release binary):

| Benchmark | Zymbol (avg) | Python (avg) | Ratio |
|-----------|-------------|-------------|-------|
| Stress core | 224 ms | 80 ms | ~2.8× |
| Pattern match | 177 ms | 74 ms | ~2.4× |
| Recursion (`fib(30)` + `ackermann(3,6)`) | 1 760 ms | 209 ms | ~8.4× |
| Collections | 61 ms | 38 ms | ~1.6× |
| Strings | 45 ms | 25 ms | ~1.8× |
| Strings Stress | 123 ms | 42 ms | ~2.9× |
| Strings Modify | 62 ms | 38 ms | ~1.6× |

The recursion benchmark is dominated by `fib_rec(30)` (2.7 M recursive calls in the
tree-walker); iterative and VM paths are significantly faster. All other benchmarks
run within 2–3× of CPython, with collections and string modification at ~1.6×.

---

## [0.0.3] — 2026-04-09

### Added

- **Numeral modes** (`#d0d9#` syntax): output digits in any of 70+ Unicode scripts
  at runtime (Devanagari, Arabic-Indic, Thai, Klingon pIqaD, and more).
  New crate `digit_blocks` maps base codepoints for all supported scripts.
- **Command execution operators**:
  - `</ path />` — execute a `.zy` script file.
  - `<\ cmd \>` — execute a shell (bash) command.
- 71 i18n/numerals test cases covering every supported numeral system.
- LSP refactor: library logic extracted into `lib.rs`, `main.rs` simplified.
- MANUAL.md and EBNF grammar updated to document all new constructs.

### Changed

- Workspace version bumped to `0.0.3`.

---

## [0.0.2] — 2026-03-24

### Added

**Collection API** (arrays, tuples, strings — unified operators)
- `$+[i]` — insert at position.
- `$-` — remove first occurrence by value.
- `$--` — remove all occurrences by value.
- `$-[i]` / `$-[i..j]` / `$-[i:n]` — remove at index or range.
- `$??` — find all indices of a value.
- `$[s:n]` — count-based slice alias.
- `$^+` / `$^-` — sort ascending/descending, natural or custom comparator.

**Destructuring assignment**
- Array destructuring: `[a, b, *rest] = arr`.
- Named-tuple destructuring: `(name: n) = t`.
- Negative indices `arr[-1]` normalized in both tree-walker and VM.

**Tests**
- 20 new E2E test cases (`tests/collections/13–32`).
- 159/159 VM parity tests passing.

**Documentation**
- EBNF v2.1.0: `destructure_assign` grammar, fixed equality (`== | <>`),
  removed unimplemented `^=`, interpolation and negative-index notes.
- MANUAL.md: §11b Destructuring, negative indices, `!=` → `<>`, sort and
  destructuring in symbol reference and coverage table.
- ROADMAP.md: v0.0.2 header, 159/159 test count, version history entry.

### Changed

- Number formatting operators renamed: `c|..|` → `#,|..|`, `e|..|` → `#^|..|`.
- Export alias syntax formalized.

---

## [0.0.1] — 2026-03-22

Initial release — Zymbol-Lang interpreter v5I.

### Added

**Core language**
- Variables (`=`) and constants (`:=`), all primitive types: `Int`, `Float`,
  `String`, `Char`, `Bool`, `Array`, `Tuple`.
- Arithmetic, comparison, logical operators; compound assignment
  (`+=`, `-=`, `*=`, `/=`, `%=`, `^=`, `++`, `--`).
- String interpolation; output `>>` (multi-item juxtaposition); input `<<`;
  CLI args capture `><`.
- Control flow: `?` / `_?` / `_` (if / else-if / else).
- Match `??` with literal, range, guard `_?`, and wildcard arms.
- All loop forms: infinite, while, for-each, range with step, reverse range.
- Labeled loops with `@!` (break) and `@>` (continue).
- Functions with isolated scope; output parameters `<~` (pass by reference).
- Lambdas with implicit and explicit return; closures (outer scope capture).
- Higher-order functions: `$>` map, `$|` filter, `$<` reduce.
- Pipe operator `|>` with placeholder `_`.
- Error handling: `!?` / `:!` / `:>` try/catch/finally with typed catch.
- Module system: `#` / `#>` / `<#` with aliases and re-exports.
- Data operators: `#|x|`, `x#?`, `#.N|x|`, `#!N|x|`.
- Base literals and conversions: `0x`, `0b`, `0o`, `0d`.
- Explicit variable lifetime: `\ var`.

**Execution**
- Tree-walker interpreter (default): scope pool recycling, zero allocation per
  scope push/pop, tail-call optimization (TCO).
- Register VM (`--vm`): flat register stack, 4.4× faster than tree-walker on
  `fib(35)`, 16-byte `Value` via `Rc<T>` heap payloads.

**Tooling** (17-crate Rust workspace)
- `zymbol-cli` — `run`, `build`, `check`, `fmt`, `repl` subcommands.
- `zymbol-lsp` — Language Server Protocol via tower-lsp + tokio.
- `zymbol-formatter` — configurable indentation.
- `zymbol-repl` — interactive REPL with history.
- `zymbol-standalone` — embeds `.zy` files into Rust project templates.
- `zymbol-analyzer` — LSP analysis engine, document cache, symbol index.

**Tests**
- 88/88 E2E tests passing.
- 99/99 VM parity tests passing.
- RosettaStone i18n suite: 105 languages.
- 19 verified examples in `examples/`.

---

[Unreleased]: https://github.com/zymbol-lang/zymbol/compare/v0.0.4...HEAD
[0.0.4]: https://github.com/zymbol-lang/zymbol/compare/v0.0.3...v0.0.4
[0.0.3]: https://github.com/zymbol-lang/zymbol/compare/v0.0.2...v0.0.3
[0.0.2]: https://github.com/zymbol-lang/zymbol/compare/v0.0.1...v0.0.2
[0.0.1]: https://github.com/zymbol-lang/zymbol/releases/tag/v0.0.1
