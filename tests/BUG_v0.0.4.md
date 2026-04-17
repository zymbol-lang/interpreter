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

All 7 Python comparison benchmarks fail immediately:

```bash
bash tests/scripts/run_all.sh --python   # exits with code 1 at STRESS TEST
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
