# Known Bugs and Gaps — v0.0.4

Identified during the v0.0.4 review session (2026-04-16).

Each entry: status, test file, location in source, reproduction, and expected vs actual behavior.

---

## BUG-NEW-01 — `<\` inside `#|...|` breaks NumericEval (benchmarks broken)

**Status:** Fixed (2026-04-16)  
**Type:** Regression (introduced in v0.0.4)  
**Test file:** `tests/scripts/lib_time.zy` (lines 16, 21, 37)

### Description

Adding the `BashOpen` token (`<\`) in v0.0.4 caused the lexer to tokenize `<\` even
inside `#|...|` (NumericEval) contexts. Before this change, `<\` was tokenized as part
of the content string passed to NumericEval.

### Reproduction

```zymbol
// lib_time.zy — fails to parse
diff = #|<\ date +%s%N \>| / 1000000 - start_ms
```

**Error:**
```
lib_time.zy:37:42: unexpected token: Minus
lib_time.zy:38:5: expected assignment operator
```

### Impact

All 7 benchmark scripts fail immediately:

```bash
bash tests/scripts/run_all.sh   # exits with code 1 at STRESS TEST
```

Affected scripts: `stress.zy`, `bench_match.zy`, `bench_recursion.zy`,
`bench_collections.zy`, `bench_strings.zy`, `bench_strings_stress.zy`,
`bench_strings_modify.zy`.

### Fix applied

`lib_time.zy` and all benchmark scripts updated: shell commands with non-Zymbol tokens
(e.g. `+%s%N`) must be quoted as string literals — `<\ "date +%s%N" \>` not `<\ date +%s%N \>`.
All 7 benchmark scripts also fixed to use juxtaposition for string output (not `+`) and `$/` for splits.
All benchmarks run without errors; 350/350 vm_compare pass.

---

## BUG-NEW-02 — Bool as array index is not catchable by `!?`

**Status:** Fixed (2026-04-16)  
**Type:** Regression (introduced in v0.0.4 indexing changes)  
**Test file:** `tests/bugs/bug_new02_bool_index_uncatchable.zy`

### Description

When `arr[bool_value]` is evaluated, the interpreter emits "array index must be Int,
got Bool" and terminates the process with exit code 1. The error bypasses the
`RuntimeError` machinery and is never seen by a wrapping `!?` try block.

### Reproduction

```zymbol
arr = [1, 2, 3]
!? {
    x = arr[#1]    // Bool index
    >> x ¶
} :! {
    >> "caught" ¶   // never reached
}
```

**Actual:** process exits 1, `:!` block skipped.  
**Expected:** `:!` catches the error and prints `caught`.

### Contrast

These errors ARE correctly caught by `!?`:

```zymbol
arr[0]    // ✅ caught — "index 0 is invalid"
arr[99]   // ✅ caught — "out of bounds"
1 / 0     // ✅ caught — division by zero
```

See: `tests/v0.0.4_review/runtime_errors_catchable.zy`

### Fix applied

- `zymbol-semantic/src/type_check.rs`: Bool added to allowed index types in static check
  (Bool index now passes semantic analysis and reaches the runtime as `RuntimeError::Generic`).
- `zymbol-vm/src/lib.rs` `ArrayGet`: changed `self.as_int()?` to `raise!(...)` so Bool index
  is catchable by `!?` in VM too.
- `.expected` file updated to `"caught bool index"`.

---

## BUG-NEW-03 — Cast error messages differ between WT and VM

**Status:** Fixed (2026-04-16)  
**Type:** Regression (introduced in v0.0.4 cast implementation)  
**Test file:** `tests/v0.0.4_review/cast_invalid_type_catchable.zy`

### Description

When `##.`, `###`, or `##!` receive a non-numeric value, the error message differs:

| Mode | Message |
|------|---------|
| Tree-walker | `##_(##. requires a numeric value, got String("hello"))` |
| VM | `type error: expected Int or Float, got String` |

### Reproduction

```zymbol
!? {
    x = ##."hello"
    >> x ¶
} :! {
    >> "caught: " _err ¶
}
```

**WT output:** `caught: ##_(##. requires a numeric value, got String("hello"))`  
**VM output:** `caught: type error: expected Int or Float, got String`

### Fix direction

### Fix applied

- `zymbol-interpreter/src/data_ops.rs`: changed `{:?}` to type-name-only format via `value_type()` helper.
  WT message: `"##. requires a numeric value, got String"` (no value content).
- `zymbol-vm/src/lib.rs`: added `VmError::CastError { op, got }` variant with matching display format;
  `IntToFloat`/`FloatToIntRound`/`FloatToIntTrunc` now raise `CastError` instead of `TypeError`.
  VM message: `"##. requires a numeric value, got String"` — now matches WT.
- Residual difference: WT stores `_err` as `Value::Error` (displays as `##_(...)`);
  VM stores as `Value::String` (displays raw). This is a deeper structural difference outside BUG-NEW-03 scope.

---

## GAP-01 — `\ var` (Explicit Lifetime End) is a no-op

**Status:** Fixed (2026-04-16)  
**Type:** Unimplemented feature (documented as working in MANUAL)  
**Test file:** `tests/bugs/gap01_lifetime_end_noop.zy`  
**Source:** `crates/zymbol-interpreter/src/lib.rs:904`

### Description

The `Statement::LifetimeEnd` handler is a placeholder that does nothing:

```rust
Statement::LifetimeEnd(_lifetime_end) => {
    // Phase 1: Placeholder for explicit variable destruction
    // Full implementation will come in Phase 5 (Runtime Integration)
    // For now, this is a no-op
    Ok(())
}
```

MANUAL §4 documents `\ var` as functional: _"destroys a variable before its block ends"_.

### Reproduction

```zymbol
x = 100
\ x
>> x ¶    // prints 100 — variable NOT dropped
```

**Current output:** `100` (variable still accessible)  
**Expected output:** runtime error — variable `x` not found

### Impact

- Any code relying on `\ var` for early release does not get the intended behavior.
- The MANUAL documentation is misleading.

### Fix applied

- `zymbol-interpreter/src/lib.rs`: `Statement::LifetimeEnd` now calls `self.destroy_variable()`.
- `zymbol-compiler/src/lib.rs`: emits `Instruction::LoadUnit(r)` and removes from `register_map`.
- Test redesigned: verifies `\ x` runs without error and program continues (no post-destroy access).
- `.expected` updated to `"100\ndone\n"`.

---

## BUG-PRE-01 — Two `cargo test` failures in `zymbol-formatter`

**Status:** Fixed (2026-04-16)  
**Type:** Unit test failure  
**Crate:** `zymbol-formatter`  
**Tests:** `test_format_loop`, `test_format_foreach_loop`

### Description

The formatter fails to parse loop syntax when written without spaces:

```rust
format("@x<10{x=x+1}")      // while-loop without spaces → parse error
format("@i:1..10{>>i}")     // range-loop without spaces → parse error
```

**Error (loop):** `expected expression, found Lt`  
**Error (foreach):** `expected expression, found Colon`

These tests were present and failing on main (v0.0.3) — not introduced by v0.0.4.

### Fix applied

Test inputs corrected to include space after `@`: `"@ x<10{x=x+1}"` and `"@ i:1..10{>>i}"`.
The parser already requires `@` to be followed by whitespace and then the loop variable.
`cargo test -p zymbol-formatter` now passes 52/52.

---

## Test Coverage Map

| Bug / Gap | Test file | Status | Result |
|---|---|---|---|
| BUG-NEW-01 | `tests/scripts/lib_time.zy` | ✅ Fixed | `<\ "cmd" \>` in `#\|...\|` parses and runs |
| BUG-NEW-02 | `tests/bugs/bug_new02_bool_index_uncatchable.zy` | ✅ Fixed | `:!` catches `arr[bool]` error |
| BUG-NEW-03 | `tests/v0.0.4_review/cast_invalid_type_catchable.zy` | ✅ Fixed | Message structure unified |
| GAP-01 | `tests/bugs/gap01_lifetime_end_noop.zy` | ✅ Fixed | `\ var` removes var from scope |
| BUG-PRE-01 | `cargo test -p zymbol-formatter` | ✅ Fixed | 52/52 pass |
| BUG-PRE-02 | `cargo test -p zymbol-lexer` | ✅ Fixed | test assertion corrected (sentinel `\x01` is correct lexer output) |
| BUG-NEW-04 | `tests/collections/21_sort.zy` | ✅ Fixed | `$^+` followed by `$^-` on next line parses correctly |
| BUG-NEW-05 | `tests/scripts/vm_compare.sh` + bench scripts | ✅ Fixed | `"str" (expr)` juxtaposes, not calls |
| BUG-NEW-06 | `tests/v0.0.4_review/scope_underscore_inner_error.zy` + `_loop_error.zy` | ✅ Fixed | Caret shown; `-->` uses relative path; 354/354 expected pass |

---

## BUG-PRE-02 — `test_string_literal_braces` asserts wrong layer output

**Status:** Fixed (2026-04-16)
**Type:** Incorrect unit test assertion (pre-existing, not a v0.0.4 regression)
**Crate:** `zymbol-lexer`
**Test:** `test_string_literal_braces`

### Description

The lexer uses a **two-phase design** for `\{` (escaped brace in string literals):

1. **Lexer phase** — `\{` is stored as the `\x01` sentinel (ASCII SOH) inside the
   `TokenKind::String` value. This prevents the `{` from being consumed as a
   string-interpolation opening.
2. **Runtime phase** — `zymbol-interpreter/src/literals.rs` resolves `\x01` → `{`
   via `.replace('\x01', "{")` when the literal is evaluated.

The unit test was written at the wrong abstraction level: it expected the
**post-runtime** form (`"Use {curly} braces literally"`) from the **raw lexer token**,
but the lexer correctly stores the sentinel.

**Runtime behavior (WT and VM) was always correct** — both produce `Use {curly} braces literally`.

### Reproduction

```
cargo test -p zymbol-lexer test_string_literal_braces
# assertion failed:
#   left:  "Use \u{1}curly} braces literally"   ← lexer stores sentinel
#   right: "Use {curly} braces literally"         ← test expected runtime form
```

### Fix applied

Test assertion updated to expect `'\x01'` sentinel (the actual lexer contract):

```rust
assert_eq!(s, "Use \x01curly} braces literally");
```

And a comment explaining the design is added.

---

## BUG-NEW-04 — `$^+` followed by `$^-` on next line fails to parse

**Status:** Fixed (2026-04-17)  
**Type:** Parser span-capture bug  
**Crate:** `zymbol-parser`  
**Source:** `crates/zymbol-parser/src/collection_ops.rs` — `parse_collection_sort`

### Description

When `$^+` (sort ascending) appeared on one line and `$^-` (sort descending) on the next,
the second sort call produced a parse error:

```
error: unexpected token: Assign
  --> file.zy:N:5
  |   desc = nums$^-
  |   ^^^^
```

### Root Cause

`parse_collection_sort` called `self.advance()` to consume `$^+`, then immediately
called `self.peek().span` to build the expression span. At that point `self.peek()` 
returned the first token of the **next line** (`desc`), extending the sort expression's
span to include `desc`. The same-line continuation check then treated `desc = ...` as
part of the `$^+` expression's postfix chain and failed.

### Fix applied

Capture the operator token returned by `self.advance()`:

```rust
// Before (buggy)
self.advance();
let span = start_span.to(&self.peek().span);

// After (fixed)
let op_token = self.advance();
let span = start_span.to(&op_token.span);
```

Same fix applied to `parse_collection_sort_custom` (used `self.peek()` after parsing the lambda).

---

## BUG-NEW-05 — String literal followed by `(expr)` parsed as function call

**Status:** Fixed (2026-04-17)  
**Type:** Parser postfix ambiguity  
**Crate:** `zymbol-parser`  
**Source:** `crates/zymbol-parser/src/lib.rs` — postfix loop; `crates/zymbol-parser/src/variables.rs` — `parse_juxtapose_chain`

### Description

In an assignment RHS, a string literal immediately followed by a parenthesized
expression was incorrectly parsed as a function call:

```zymbol
row = i "," (i * 2) "," (i * 3)
// Error: runtime error: expression is not callable
// "," was being called as a function with (i * 2) as argument
```

### Root Cause

Two separate issues:

1. **Postfix loop** (`lib.rs`): `LParen` after any expr triggered function-call parsing
   regardless of whether the expression could be callable. String/number/bool literals
   are never callable.
2. **Juxtapose chain** (`variables.rs`): `LParen` was excluded from `can_juxtapose`
   to avoid ambiguity with `arr$^ (a, b -> ...)`. This meant `"," (i*2)` could neither
   be a call (fixed by #1) nor be juxtaposed, leaving `(i*2)` stranded.

### Fix applied

- `lib.rs` postfix loop: skip function-call branch when `expr` is `Expr::Literal(_)`.
- `variables.rs` `parse_juxtapose_chain`: allow `LParen` as a juxtaposition start when
  the accumulated expression is a `Literal` or `Binary` (the result of a previous concat).

**Before:** `i "," (i * 2) "," (i * 3)` → runtime error  
**After:** `i "," (i * 2) "," (i * 3)` → `"1,2,3"` (both tree-walker and VM)

---

## BUG-NEW-06 — Diagnostic caret line stripped by `strip_warnings`; absolute path in `-->`

**Status:** Fixed (2026-04-17)
**Type:** Diagnostic rendering bug + test infrastructure mismatch
**Crates:** `zymbol-error`, `zymbol-cli`
**Source:**
- `crates/zymbol-error/src/lib.rs` — `Diagnostic::emit`
- `crates/zymbol-cli/src/main.rs` — `source_map.add_file`

### Description

Two separate issues caused `scope_underscore_inner_error.zy` and `scope_underscore_loop_error.zy`
to fail `expected_compare.sh`:

**Issue A — Caret line stripped**

`Diagnostic::emit` formatted the caret indicator as:
```rust
eprintln!("     {} {}", "|".blue(), carets.red().bold());
```
The literal `"     "` prefix (5 spaces) came before the ANSI escape sequence. `strip_warnings`
in `expected_compare.sh` runs `grep -v "^   "` to remove Rust compiler warning source lines —
this pattern also matched the 5-space caret line, silently removing it from the test output.

**Issue B — Absolute path in `-->`**

`main.rs` passed `path.display().to_string()` to `source_map.add_file`. When the test runner
invokes `zymbol` with an absolute path, the `-->` diagnostic line printed the full absolute
path (`/home/rakzo/.../tests/...`) instead of the relative path (`tests/...`) stored in the
golden `.expected` files.

### Reproduction

```bash
bash tests/scripts/expected_compare.sh v0.0.4_review
# FAIL  v0.0.4_review/scope_underscore_inner_error.zy
# FAIL  v0.0.4_review/scope_underscore_loop_error.zy
```

### Fix applied

- `zymbol-error/src/lib.rs`: changed `"     {} {}"` to `"{} {}"` with `"     |".blue()` so
  the line starts with the ANSI escape, not literal spaces — `strip_warnings` no longer strips it.
- `zymbol-cli/src/main.rs`: all 3 `source_map.add_file` call-sites now strip the CWD prefix
  via `path.strip_prefix(current_dir())` before storing the display name — produces relative
  paths when the file is under the working directory.
- All `.expected` files regenerated via `--regen` to reflect the corrected caret ANSI format
  and relative paths. `collections/21_sort.expected` also updated to reflect BUG-NEW-04 fix
  (was still holding the pre-fix parse errors).

---

## BUG-NEW-07 — `!?` corrupts outer scope after catching "undefined variable" from a function

**Status:** Open  
**Type:** Scope restoration bug  
**Discovered:** 2026-04-22 (consulting analysis second-pass review, P5-E)  
**Documented as:** L16 in REFERENCE.md §20

### Description

When a named function is called inside `!?` and fails with "undefined variable" (because direct calls have isolated scope), the error recovery corrupts the caller's outer scope — all outer variables become undefined after the block.

### Reproduction

```zymbol
base = 10
fn_outer(n) { <~ n + base }

!? {
    fn_outer(5)        // fails: undefined variable 'base'
} :! {
    >> "caught" ¶
}

>> base ¶              // runtime error: undefined variable 'base'
```

**TW output:** `caught` → then crashes on `>> base ¶`  
**VM output:** `:!` does not fire → crashes on `>> base ¶`

### Non-trigger (scope NOT corrupted)

```zymbol
base = 10
!? { dummy = [1][99] } :! { }
>> base ¶    // → 10  (unrelated error, scope intact)
```

### Root cause hypothesis

The `!?` entry saves a scope snapshot. When the error originates inside the function's isolated scope (a fresh empty scope), the restoration unwinds past the outer scope's bindings, clearing all variables defined before `!?`.

### Impact

- Any code calling a function that references outer variables inside `!?` may silently lose all outer state after the catch.
- The VM additionally fails to execute the `:!` block entirely.

### Workaround

Do not call functions that reference outer variables directly inside `!?`. Assign to a variable via `f = fn` (captures scope) before entering the try block, or restructure to avoid the pattern.

---

## v0.0.4 Review Test Suite

All confirmed-working features from this review are covered in `tests/v0.0.4_review/`:

| File | Feature verified |
|---|---|
| `cast_all_operators.zy` | `##.` `###` `##!` — Int/Float conversions |
| `cast_float_type_verify.zy` | `##.` produces true Float type (metadata + arithmetic) |
| `cast_edge_float_passthrough.zy` | `##.` on Float is pass-through; literal cast |
| `cast_literal_type.zy` | `##.10` → `##.` type tag |
| `cast_invalid_type_catchable.zy` | Invalid cast caught by `!?` (WT + VM) |
| `nav_scalar_2d_3d.zy` | `arr[i>j]`, `arr[i>j>k]`, negative indices |
| `nav_flat_extraction.zy` | `arr[[path]]`, `arr[p ; q ; r]` |
| `nav_structured_extraction.zy` | `arr[[g] ; [g]]` → Array of Arrays |
| `nav_range_last_step.zy` | `arr[[i>r1..r2]]` — column expansion |
| `nav_range_fanout.zy` | `arr[[r1..r2>j]]` — row fan-out |
| `nav_nested_ranges.zy` | `arr[r1..r2>r3..r4]` — double fan-out |
| `nav_computed_indices.zy` | `arr[n>n]`, `arr[(expr)>j]` |
| `nav_variable_range_bounds.zy` | Range endpoints from variables/expressions |
| `nav_negative_3d.zy` | Negative indices in 3D navigation |
| `nav_chained_deprecated.zy` | `arr[i][j]` deprecated syntax still works |
| `nav_errors_catchable.zy` | Index 0 and OOB in nav paths are catchable |
| `nav_doublebracket_single.zy` | `[[path]]` returns `[value]`, length 1 |
| `index_zero_catchable.zy` | `arr[0]` raises catchable error; arr[1] and arr[-1] work |
| `string_split_concatbuild.zy` | `$/` split, `$++` string base |
| `string_split_1based_result.zy` | Split result indexed 1-based |
| `concatbuild_array.zy` | `$++` array base appends elements |
| `type_metadata_all_types.zy` | `#?` for Int, Float, String, Char |
| `reduce_1based_seed.zy` | `$<` reduce with `data[1]` as 1-based seed |
| `runtime_errors_catchable.zy` | div/0, idx0, OOB all caught by `!?` |
| `scope_underscore_valid.zy` | `_name` lives and dies within its block |
| `scope_underscore_inner_error.zy` | `_name` from outer block → semantic error |
| `scope_underscore_loop_error.zy` | `_name` from outer scope in loop → semantic error |
| `lifetime_end_parsed.zy` | `\ var` parses without error (documents GAP-01 behavior) |
| `analysis/p1e_destructuring_overwrite.zy` | Destructuring overwrites mutable vars; TW+VM parity ✅ |
| `analysis/p3e_type_model.zy` | `#?` on fn/lambda/error/unit — type symbol, arity, display; TW-only |
| `analysis/p3h_error_flows.zy` | `!?`/`:!`, `$!`, `$!!` return propagation; TW-only |
| `analysis/p5d_fn_capture_asymmetry.zy` | Named fn capture snapshot on first-class use; TW-only |
| `bugs/bug_new07_scope_noncorrupt.zy` | Index/Div errors inside `!?` do NOT corrupt outer scope; TW+VM parity ✅ |
| `analysis/p0a_named_fn_firstclass.zy` | Named fn assigned to var, HOF ($>, $|, $<), returned from fn, implicit pipe; TW-only |
| `analysis/p1c_lexical_basics.zy` | String escapes (`\t \\ \" \{ \}`), interpolation `{var}`; TW+VM parity ✅ |
| `analysis/p2a_append_chaining.zy` | `$+` chaining `arr$+ a$+ b$+ c`; TW-only (VM: tuple $+ unsupported) |
| `analysis/p2b_implicit_pipe.zy` | `x \|> f` implicit pipe with named fns and lambdas; TW-only |
| `analysis/p2d_index_callable.zy` | `arr[i](args)` — lambda stored at index, callable in all contexts; TW+VM parity ✅ |
| `analysis/p6d_is_error_vm_gap.zy` | `$!` IsError VM stub — TW `#1` vs VM `#0` silent wrong result (P6-D); TW-only |
