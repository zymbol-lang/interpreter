# Changelog

All notable changes to Zymbol-Lang are documented here.

Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)  
Versioning: [Semantic Versioning](https://semver.org/) (pre-1.0 series)

---

## [Unreleased]

---

## [0.0.5] — 2026-04-29

### Added

**Hot Definition operator `°` (U+00B0)**
- New postfix operator on identifiers: `x° += 1` auto-initializes `x` to the neutral
  value of the inferred context on first use, then applies the operation.
- Neutral values: numeric context → `0` / `0.0`; string context → `""`;
  array context → `[]`.
- Valid in both LHS (`x° += n`) and RHS (`p = p° + c`).
- Warnings emitted on semantically vacuous hot-defs:
  `x° *= 5` → always 0; `x° /= 2` → division of 0; `x° ^= 2` → always 0.
- Undefined variable error now includes hint: `'x' is undefined — did you mean 'x°' (hot definition)?`
- Implemented across: lexer (`HotIdent` token), parser (`hot: bool` field on `Assignment`),
  interpreter (neutral inference in `loops.rs`), semantic type-checker (`type_check.rs`).

### Fixed

**BUG-001 — Re-exported functions lose origin module scope**
- Functions accessed through an i18n re-export adapter (`alias::fn <= newname`) raised
  `undefined variable` for any module-level variable the function read.
- Root cause: `eval_traditional_function_call` loaded context from the adapter module path,
  which carries no variables.
- Fix: `FunctionDef` now carries `origin_module_path: Option<PathBuf>`. The call site
  derives `effective_path` from that field, falling back to the caller's module only when
  the function has no recorded origin.
- New test: `tests/bugs/bug001_scope_reexport.zy` (3-file i18n fixture).

**BUG-002 — `><` CLI args capture not registered in semantic scope**
- `zymbol check` and the LSP reported `undefined variable` for any use of the captured
  identifier inside blocks (`? {}`, `@ {}`, etc.) after `>< args`.
- Root cause: `Statement::CliArgsCapture` had no handler in `type_check.rs`.
- Fix: added handler that calls `env.define_var(name, Array(String))`.
- New test: `tests/bugs/bug002_cli_args_scope.zy`.

**BUG-003 — LSP percent-decodes Unicode directory names in file URIs**
- VS Code sends `file:///home/user/%E6%BA%90%E7%A0%81/mod.zy` for paths inside directories
  with Unicode names (e.g. `源码/`). The LSP resolver built a path with the literal
  percent-encoded segment, which does not exist on the filesystem → `module-not-found` for
  every import inside those directories. CLI was unaffected.
- Fix: `uri_to_path` in `workspace.rs` now calls `percent_decode` before constructing the
  `PathBuf`. Multi-byte UTF-8 sequences (e.g. `源` = 3 bytes) are collected as raw bytes
  before UTF-8 reconstruction. No new dependencies.
- Four new unit tests in `workspace.rs`: encoded Unicode, plain Unicode, `%2F`, no-op.

**GAP-001 — Arithmetic expressions as slice bounds `$[start..end]`**
- `$[pos-1..end]` or `$[start..pos+1]` caused a parse error; only literals and plain
  identifiers were accepted as bounds.
- Root cause: `parse_collection_slice` called `parse_postfix` for bounds, which stops
  before `+`/`-` and cannot consume `..` without ambiguity.
- Fix: new `parse_slice_bound()` method in `collection_ops.rs` wraps `parse_postfix` with
  a `+`/`-` loop, stopping before `..`. Replaces all three bound call-sites in
  `parse_collection_slice`.
- New test: `tests/gaps/gap001_slice_arith_bounds.zy`.

**GAP-002 — Parenthesized expressions not accepted as `$++` items**
- `"prefix" $++ (expr)` failed with a parse error; `>>` accepted the same form correctly.
- Root cause: `parse_string_insert` gated item collection with `can_juxtapose()`, which
  intentionally excludes `LParen` to avoid lambda-comparator ambiguity in `$^+`.
- Fix: local `can_start` flag in `parse_string_insert` adds `TokenKind::LParen` without
  modifying `can_juxtapose` globally. `$^+` and juxtaposition chains are unaffected.
- New test: `tests/gaps/gap002_concat_paren_items.zy`.

**GAP-003 — `ambiguous lifetime` warning on every loop iterator variable**
- `@ elem:arr { }` always emitted `warning: ambiguous lifetime for 'elem'` regardless of
  whether the programmer had signalled intent.
- Fix in `def_use.rs` — two suppression rules, no new syntax:
  1. `_` prefix (`@ _elem:arr`): existing "intentionally ignored" convention now also
     suppresses the lifetime warning, consistent with unused-variable suppression.
  2. Pre-defined variable (`x = 0` then `@ x:arr`): if the variable already has a
     definition before the loop, the reuse is deliberate and no warning is emitted.
  Normal unnamed iterator variables still warn as before.
- New test: `tests/gaps/gap003_loop_iter_lifetime_warning.zy`.

### Test suite — v0.0.5

| Suite | Result |
|-------|--------|
| `cargo test` (all crates) | all pass |
| `expected_compare.sh gaps` | **15 / 15 pass** |
| `expected_compare.sh bugs` | **8 / 8 pass** |

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
- **Closed block syntax** (`# name { ... }`): module body is now explicitly delimited by
  braces. Flat/open syntax is no longer valid. Any token after the closing `}` is a parse
  error. `<#` imports, `#>` export block, literal constants, literal variables, and function
  definitions are the only elements permitted inside the block.
- **E013 — ExecutableStatementInModule**: new semantic error raised when an executable
  statement (`>>`, `<<`, function call, `?`, `@`, `!?`, `<~`, `<\ \>`, etc.) appears at
  module top-level. Variable and constant initializers must use a literal RHS; non-literal
  initializers also trigger E013.
- All existing module files migrated to block syntax (modules_scope, gaps, bugs, i18n).
- New tests `11_block_syntax_basic` and `12_private_state_block` covering block syntax
  end-to-end and private mutable state persistence inside blocks.
- MANUAL.md §17 rewritten: required block syntax, allowed/forbidden element table,
  E013 reference, all code examples updated.
- **E001 enforcement**: `# name { }` declaration must exactly match the filename stem.
  Dot-prefix convention (`# .name`) supported for subdirectory modules.
  E001 was previously defined but not triggered; it now fires on every `zymbol check`.
- **Module-file guard**: `zymbol run module.zy` detects a module declaration and exits
  with a clear error instead of silently doing nothing. Exit code 1.

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
- `tests/errors/runtime/` — 10 regression cases: one per catchable/runtime error type
  (div-zero, index-zero, index-oob, type-cast, undefined-var, module-not-found, E004,
  E008, E010, E012). Verified with `expected_compare.sh errors/runtime`.
- `tests/errors/catchable/` — 5 catch-block cases: `##Div`, `##Index`, `##Type`,
  generic `:!`, and a combined all-types sequence. Verified with `expected_compare.sh errors/catchable`.
- `tests/errors/semantic/` — 18 semantic regression cases (E001–E013 + support modules).
  Verified with the new `tests/scripts/semantic_compare.sh` (uses `zymbol check`).
- `tests/scripts/semantic_compare.sh` — new script: runs `zymbol check`, strips ANSI
  codes, supports `****` wildcards and `--regen`. Mirrors `expected_compare.sh`.
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
- False positive "unused variable" warnings for constants and variables listed in `#>`:
  `VariableAnalyzer` now marks exported items as used before emitting diagnostics.

### VM performance — Sprint 6A: Fused split intrinsics (2026-04-17)

New crate `zymbol-intrinsics` — pure Rust functions operating on `&str` / primitives,
zero VM types, zero boxing. Architecture mirrors CPython `Objects/unicodeobject.c`:
VM → adapter (unbox `ZyStr` → `&str`) → intrinsic fn → primitive → adapter (box → `Value`).
Circular dependencies avoided: `zymbol-intrinsics` has zero workspace dependencies.

**New crate `crates/zymbol-intrinsics/`:**
- `split.rs` — `count`, `count_str`, `first`, `last`, `join`, `join_str`, `count_where`, `parts`, `parts_str`.
- `search.rs` — `count_char`, `count_str`, `find_positions_char`, `find_positions_str`.
- `transform.rs` — `replace_char`, `replace_str`, `replace_n_char`, `replace_n_str`, `repeat`, `trim`.

**4 new fused bytecode instructions in `zymbol-bytecode`:**
- `StrSplitCount(dst, str, sep)` — fused `(s $/ sep)$#`; calls `intrinsics::split::count`, zero `Vec<Value>`.
- `StrSplitMap(dst, str, sep, fn)` — fused `(s $/ sep) $> fn`; iterates parts directly.
- `StrSplitFilter(dst, str, sep, fn)` — fused `(s $/ sep) $| fn`; no intermediate array.
- `StrSplitReduce(dst, str, sep, init, fn)` — fused `(s $/ sep) $< (init, fn)`; streaming fold.

**Compiler pattern detection (`zymbol-compiler`):**
- `compile_collection_length` detects `(s $/ sep)$#` → emits `StrSplitCount`.
- `compile_collection_map` detects `(s $/ sep) $> fn` → emits `StrSplitMap`.
- `compile_collection_filter` detects `(s $/ sep) $| fn` → emits `StrSplitFilter`.
- `compile_collection_reduce` detects `(s $/ sep) $< (init, fn)` → emits `StrSplitReduce`.
- `max_reg_used` updated with all 4 new instruction arms.

**VM dispatch (both sites) updated in `zymbol-vm`:**
- Both dispatch sites handle all 4 new instructions; `Char` and `String` separator variants dispatched.

**Benchmark (release, split-count inline vs 2-statement, 100 000 iterations):**

| Pattern | Time |
|---------|------|
| `(csv $/ ',')$#` (fused `StrSplitCount`) | 5 ms |
| `parts = csv $/ ','` ; `parts$#` (unfused) | 10 ms |

*50% reduction for the inline form. The 2-statement form cannot be fused without
dataflow analysis and still uses `StrSplit` + `ArrayLen`.*

---

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

**Benchmark results** (`run_all.sh --runs 3`, release binary):

| Benchmark | Zymbol tree-walker | Zymbol VM |
|-----------|-------------------|-----------|
| Stress core | 224 ms | — |
| Pattern match | 177 ms | — |
| Recursion (`fib(30)` + `ackermann(3,6)`) | 1 760 ms | — |
| Collections | 61 ms | 33 ms |
| Strings | 45 ms | 36 ms |
| Strings Stress | 123 ms | — |
| Strings Modify | 62 ms | — |

The recursion benchmark is dominated by `fib_rec(30)` (2.7 M recursive calls in the
tree-walker); iterative and VM paths are significantly faster.

---

## [0.0.3] — 2026-04-09

### Added

**Numeral Modes** (`#d0d9#` syntax)

Zymbol can display numbers in any of **69 Unicode digit scripts** at runtime.
The mode-switch token `#d0d9#` takes the zero-digit and nine-digit of the target
script enclosed in `#…#`. It persists until the next mode-switch in the same file.
Mode is file-local — modules never inherit or alter the caller's active script.

```zymbol
#०९#   // activate Devanagari
>> 42  ¶    // → ४२
>> 3.14 ¶   // → ३.१४

#٠٩#   // activate Arabic-Indic
>> 42 ¶     // → ٤٢

#09#   // restore ASCII
>> 42 ¶     // → 42
```

**What is affected:**
- `>>` output of `Int`, `Float`, and `Bool` values — all digits are rewritten to
  the active script.
- Boolean output: `#` prefix stays ASCII; the `0`/`1` digit adapts to the active
  script (`#१` = true in Devanagari, `#٠` = false in Arabic-Indic).
  This keeps `#0` (bool false) visually distinct from `0` (integer zero) in every
  script.

**What is NOT affected:** string content, char literals, array brackets `[]`,
tuple parentheses `()`, float decimal point (always ASCII `.`).

**Native digit literals in source code:**

Any of the 69 supported scripts can be used directly as integer literals — in
assignments, loop ranges, comparisons, and modulo operands. The lexer normalises
all scripts to the same internal integer value:

```zymbol
#०९#

n = ४२        // same as n = 42
@ i:१..१५ {  // range 1..15 in Devanagari
    ? i % १५ == ० { >> "FizzBuzz" ¶ }
    _? i % ३  == ० { >> "Fizz" ¶ }
    _? i % ५  == ० { >> "Buzz" ¶ }
    _ { >> i ¶ }
}
```

**Boolean literals in any script:**

`#` followed by the native `0` or `1` digit of any supported script lexes as a
boolean identical to ASCII `#0`/`#1`. The # prefix is always ASCII:

| Script | False | True | Mode token |
| ------ | ----- | ---- | ---------- |
| ASCII (default) | `#0` | `#1` | `#09#` |
| Devanagari | `#०` | `#१` | `#०९#` |
| Arabic-Indic | `#٠` | `#١` | `#٠٩#` |
| Ext. Arabic-Indic | `#۰` | `#۱` | `#۰۹#` |
| Bengali | `#০` | `#১` | `#০৯#` |
| Gurmukhi | `#੦` | `#੧` | `#੦੯#` |
| Gujarati | `#૦` | `#૧` | `#૦૯#` |
| Tamil | `#௦` | `#௧` | `#௦௯#` |
| Telugu | `#౦` | `#౧` | `#౦౯#` |
| Kannada | `#೦` | `#೧` | `#೦೯#` |
| Thai | `#๐` | `#๑` | `#๐๙#` |
| Myanmar | `#၀` | `#၁` | `#၀၉#` |
| Mathematical Bold | `#𝟎` | `#𝟏` | `#𝟎𝟗#` |
| Klingon pIqaD ¹ | `#` | `#` | `##` |

> ¹ Klingon pIqaD digits live in the ConScript Unicode Registry (CSUR) Private
> Use Area (U+F8F0–U+F8F9). They render only with a pIqaD-capable font such as
> _pIqaD-qolqoS_. Internally treated as a valid digit block; no special-casing
> in the interpreter.

**Selected supported scripts (25 of 69 shown):**

| Script | Range | Sample digits |
| ------ | ----- | ------------- |
| ASCII | U+0030–U+0039 | `0123456789` |
| Arabic-Indic | U+0660–U+0669 | `٠١٢٣٤٥٦٧٨٩` |
| Ext. Arabic-Indic | U+06F0–U+06F9 | `۰۱۲۳۴۵۶۷۸۹` |
| Devanagari | U+0966–U+096F | `०१२३४५६७८९` |
| Bengali | U+09E6–U+09EF | `০১২৩৪৫৬৭৮৯` |
| Gujarati | U+0AE6–U+0AEF | `૦૧૨૩૪૫૬૭૮૯` |
| Tamil | U+0BE6–U+0BEF | `௦௧௨௩௪௫௬௭௮௯` |
| Telugu | U+0C66–U+0C6F | `౦౧౨౩౪౫౬౭౮౯` |
| Thai | U+0E50–U+0E59 | `๐๑๒๓๔๕๖๗๘๙` |
| Tibetan | U+0F20–U+0F29 | `༠༡༢༣༤༥༦༧༨༩` |
| Myanmar | U+1040–U+1049 | `၀၁၂၃၄၅၆၇၈၉` |
| Khmer | U+17E0–U+17E9 | `០១២៣៤៥៦៧៨៩` |
| Mongolian | U+1810–U+1819 | `᠐᠑᠒᠓᠔᠕᠖᠗᠘᠙` |
| Mathematical Bold | U+1D7CE–U+1D7D7 | `𝟎𝟏𝟐𝟑𝟒𝟓𝟔𝟕𝟖𝟗` |
| Mathematical Double-struck | U+1D7D8–U+1D7E1 | `𝟘𝟙𝟚𝟛𝟜𝟝𝟞𝟟𝟠𝟡` |
| Mathematical Monospace | U+1D7F6–U+1D7FF | `𝟶𝟷𝟸𝟹𝟺𝟻𝟼𝟽𝟾𝟿` |
| Segmented/LCD | U+1FBF0–U+1FBF9 | `🯰🯱🯲🯳🯴🯵🯶🯷🯸🯹` |
| Klingon pIqaD ¹ | U+F8F0–U+F8F9 | `` _(CSUR PUA — requires pIqaD font)_ |
| _(+51 additional BMP/SMP scripts)_ | | _(see `crates/zymbol-lexer/src/digit_blocks.rs`)_ |

New crate `digit_blocks` (inside `zymbol-lexer`) maps the base codepoint for each
of the 69 registered blocks and provides `digit_value(char)` and
`digit_block_base(char)` used by both the lexer (literal normalisation) and the
interpreter (output formatting).

**Command execution operators**
- `</ path.zy />` — execute a `.zy` sub-script and capture its output.
- `<\ cmd \>` — execute a shell (bash) command and capture stdout + stderr.

**Tests**
- 71 i18n/numerals test cases covering every supported numeral system, including
  all boolean-literal and comparison-result forms across scripts.

**Tooling**
- LSP refactor: library logic extracted into `lib.rs`, `main.rs` simplified.
- MANUAL.md §18b and EBNF grammar updated to document all numeral-mode constructs.

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

[Unreleased]: https://github.com/zymbol-lang/zymbol/compare/v0.0.5...HEAD
[0.0.5]: https://github.com/zymbol-lang/zymbol/compare/v0.0.4...v0.0.5
[0.0.4]: https://github.com/zymbol-lang/zymbol/compare/v0.0.3...v0.0.4
[0.0.3]: https://github.com/zymbol-lang/zymbol/compare/v0.0.2...v0.0.3
[0.0.2]: https://github.com/zymbol-lang/zymbol/compare/v0.0.1...v0.0.2
[0.0.1]: https://github.com/zymbol-lang/zymbol/releases/tag/v0.0.1
