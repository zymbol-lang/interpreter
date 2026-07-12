# Zymbol-Lang — Language Reference

Complete lookup reference: known limitations, error taxonomy, and symbol table.

**Interpreter version**: v0.0.7

See also: [GUIDE.md](GUIDE.md) — full language guide with examples  
See also: [IMPLEMENTATION.md](IMPLEMENTATION.md) — EBNF grammar and internals

---

## Table of Contents

20. [Known Limitations and Workarounds](#20-known-limitations-and-workarounds)
20b. [Error Taxonomy](#20b-error-taxonomy)
21. [Complete Symbol Reference](#21-complete-symbol-reference)

---

## 20. Known Limitations and Workarounds

Limitations are classified in two categories:

- **By design** — intentional constraints that reflect a deliberate language decision. They will not change without a redesign.
- **Implementation gap** — behaviors that diverge from intent due to incomplete implementation. Subject to change in future versions.

---

### By Design

---

### ~~L1 — Postfix operators directly in `>>`~~ Fixed

Postfix operators (`$#`, `$?`, `#?`, …) are now accepted as items in `>>`
juxtaposition (verified on both engines in v0.0.7):

```zymbol
arr = [1, 2, 3]
>> "len=" arr$# ¶      // ✅ → len=3
>> "has=" arr$? 2 ¶    // ✅ → has=#1
```

Parentheses remain valid and are still useful for grouping: `>> (arr$? 3) ¶`.

### ~~L3 — Module alias.CONST does not work~~ Fixed

`alias.CONST` access now works correctly:

```zymbol
<# ./math => m
pi = m.PI    // ✅ works
e  = m.E     // ✅ works
```

**Root cause fixed**: the TypeChecker was emitting a fatal "undefined variable" error
for the module alias identifier before the interpreter could evaluate the member access.
Fix: `TypeChecker` now registers import aliases from `program.imports` before analysis passes.

### ~~L4 — `#>` export block must come before definitions~~ Fixed

`#>` can now appear in any of these positions — all are valid:

```zymbol
// ✅ Position 1 — right after # declaration (always worked):
# module_name
#> { add, PI }
PI := 3.14
add(a, b) { <~ a + b }

// ✅ Position 2 — after imports (G14 fix):
# module_name
<# ./dep => d
#> { add, PI }
PI := 3.14
add(a, b) { <~ a + b }
```

The only remaining restriction: `#>` must come before executable statements and function definitions (not at the end of the file).

### ~~L5 — Named functions are not first-class values~~ Fixed in v0.0.4

`fn = myFunc` and `arr$> myFunc` now work directly.

```zymbol
double = (x -> x * 2)
fn = double          // ✅ assign lambda to variable
>> fn(5) ¶           // → 10
```

### ~~L6 — HOF `$>`, `$|`, `$<` require inline lambdas~~ Fixed in v0.0.4

Named functions and lambda variables are now accepted directly:

```zymbol
double = (x -> x * 2)
nums = [1, 2, 3]
>> (nums$> double) ¶    // ✅ → [2, 4, 6]
```

### ~~L7 — Match multi-value arms not implemented~~ Fixed in v0.0.4

Multi-value arms are supported via list containment patterns:

```zymbol
?? y {
    [1, 2] => "low"
    _      => "other"
}  // ✅ (arm separator is `=>` since v0.0.6)
```

### L8 — ~~Negative array indices: WT vs VM behavior differs~~ Fixed in v0.0.2

Negative indices are now normalized in both tree-walker and VM:

```zymbol
arr = [10, 20, 30, 40, 50]
>> arr[-1] ¶    // → 50 (last element)
>> arr[-2] ¶    // → 40
```

### ~~L9 — False positive warnings~~ Fixed

The analyzer now tracks variable usage inside string interpolation (`"{x}"`) and
inside BashExec commands (both `<\ "ls {x}" \>` interpolation and juxtaposed
items `<\ "cat " x \>`) — no false "unused variable" warnings (verified 2026-06-12).
Regression test: `tests/errors/semantic/no_false_positive_unused.zy` (`zymbol check`
must stay clean).

### ~~L10 — Collection operators cannot be chained~~ Fixed in v0.0.4

`$+` now chains left-to-right:

```zymbol
arr = [1, 2, 3]$+ 4$+ 5$+ 6    // ✅ → [1, 2, 3, 4, 5, 6]
```

The argument to `$+` is parsed at structural postfix level (index, call, member access) but stops before the next `$` operator, enabling the chain. If the argument itself needs a collection op, wrap it in parentheses:

```zymbol
arr = base$+ (other$#)$+ 0    // appends length of other, then 0
```

---

### Implementation Gaps

---

### L12 — `do-while` (`~>`) *(dismissed 2026-06-12)*

A post-condition loop will **not** be implemented. The infinite loop with a trailing
break is the idiomatic form, and coining `~>` for it would add a symbol without
demonstrated need (SYMBOLS.md: "a new symbol enters the grammar reluctantly"):

```zymbol
// ✅ The idiom — body runs at least once:
@ {
    body_here()
    ? !condition { @! }
}
```

### ~~L13 — `$!!` from lambdas not supported~~ Fixed

`$!!` inside a lambda propagates the error as an early return to the lambda's
caller — identical semantics to named functions, in both engines (verified
2026-06-12; the old "silent no-op" description was stale):

```zymbol
handler = (x -> { x$!! <~ "ok" })
r = handler(err_value)    // r is the error — "ok" never evaluates
? r$! { >> "handle it" ¶ }
```

Regression test: `tests/lambdas/error_propagate_lambda.zy` (TW == VM parity).

### L11 — Arrays must be homogeneous *(by design)*

All elements of an array must share the same type. This is enforced by the semantic checker:

```zymbol
record = ["English", "en.zy", #0]    // ❌ String + String + Bool
```

**Why**: arrays are Zymbol's ordered mutable collection for uniform data — sequences of the same kind of value. This constraint enables type-safe collection operations (`$>`, `$|`, `$<`, `$^`) without runtime type dispatch.

**Heterogeneous records belong in named tuples**, which are immutable and field-named:

```zymbol
// ✅ Named tuple — heterogeneous, immutable, field-addressed:
record = (lang: "English", file: "en.zy", active: #1)
>> record.lang ¶
>> record.active ¶

// ✅ Array of named tuples — uniform container of heterogeneous records:
langs = [
    (lang: "English", file: "en.zy",  active: #1),
    (lang: "Spanish", file: "es.zy",  active: #1),
    (lang: "Chinese", file: "zh.zy",  active: #0)
]
@ entry:langs {
    ? entry.active { >> entry.lang " → " entry.file ¶ }
}
```

The design distinction maps cleanly: **arrays = typed sequences**, **named tuples = structured records**.

---

### ~~L14 — Destructuring does not enforce constant immutability~~ Fixed in v0.0.7

Destructuring into a `:=` constant is now a **semantic error**, consistent with direct
reassignment (`limit = 200`):

```zymbol
limit := 100
[limit, extra] = [200, 300]
// error: cannot reassign constant 'limit'
// help: constants declared with ':=' cannot be modified — use a different
//       name in the destructuring pattern
```

Destructuring into regular variables (including re-binding existing `=` variables)
is unchanged. Regression test: `tests/errors/semantic/const_destructure_overwrite.zy`.

### ~~L16 — `!?` corrupts outer scope when a function fails~~ Fixed in v0.0.7

When a named function called inside `!?` failed at runtime, the caller's scope was
corrupted — all outer variables became undefined after the `!?` block (tree-walker),
and in the VM the `:!` clause did not even fire for errors raised inside called
functions. Both are fixed:

- **Tree-walker**: every error exit path of a function/lambda call now restores the
  caller's scope state before propagating (previously the early `?` return skipped
  `restore_call_state`, leaving the function's isolated scope in place).
- **VM**: `raise!` now unwinds the frame stack to the nearest ancestor frame with an
  active catch, popping callee frames and their registers, instead of only checking
  the top frame.

```zymbol
base = 10
adder(n) { <~ n + base }   // 'base' is not visible in the isolated fn scope

!? {
    _x = adder(5)          // fails: 'base' undefined inside adder
} :! {
    >> "caught" ¶          // fires in BOTH engines
}

>> base ¶                  // → 10  — outer scope intact
```

Works for errors raised at any call depth (typed catches included). Regression test:
`tests/bugs/bug_l16_try_scope_restore.zy` (TW == VM parity).

### L17 — `std/db` not available in prebuilt Linux/macOS binaries *(by design)*

`std/db` links against the system's ODBC driver manager, and the driver manager loads
engine drivers with `dlopen` — impossible in the fully static Linux/aarch64 binaries,
and a hard startup dependency (`libodbc.dylib`) that the macOS binaries refuse to
impose. Those builds are compiled without the `db` cargo feature, so `<# std/db`
reports **`module not found: std/db`**.

Where `std/db` IS available:

- **Windows prebuilt binaries** — ODBC is part of the OS (`odbc32.dll`).
- **Any source build** — `cargo build --release` (the `db` feature is on by default).
  Build-time prereq: `unixodbc-dev`; runtime prereq: `unixodbc` + the engine's ODBC
  driver. To reproduce the prebuilt behavior use `--no-default-features`.

### ~~L18 — `x°`/`°x` inside a function called from a `@` loop panics~~ Fixed in v0.0.8

The caller's loop anchors leaked into the callee frame (`loop_scope_depths` was not
saved across the call boundary), so a hot definition inside the function indexed a
scope that no longer existed — index-out-of-bounds panic in the tree-walker. Now the
anchors are frame-local: inside a function with no `@` of its own, `x°`/`°x` anchor to
the function scope (MEMORY_MODEL.md MM-1). Regression test:
`tests/bugs/bug_mm1_hot_def_fn_scope.zy` (TW == VM).

### ~~L19 — Module-state mutations by intra-module calls were lost~~ Fixed in v0.0.8

In the tree-walker, only the frame called directly via `alias::` wrote module state
back; a private helper's mutation was discarded, and the outer frame then clobbered
the store with its stale copy. Write-back now runs for every module frame and is
diff-based (only changed keys persist), same-module nested calls see the caller's live
values, and the caller's copies are refreshed on return (MEMORY_MODEL.md MM-2).
Regression test: `tests/bugs/bug_mm2_module_state_helper.zy` (TW == VM).

### ~~L20 — `\ x` inside a function poisoned the caller's same-named variable~~ Fixed in v0.0.8

The destroyed-names set was global, so `\ x` inside a callee made the caller's own `x`
raise a false `use after destruction`. The set is now saved/restored per call frame
(MEMORY_MODEL.md MM-3). Regression test: `tests/bugs/bug_mm3_destroy_frame_local.zy`.

### ~~L21 — Modules loaded at runtime skipped semantic analysis~~ Fixed in v0.0.8

`zymbol run` only lexed + parsed imported modules, so semantic-only violations inside
module functions (e.g. reassigning a `:=` module constant) executed silently, leaving
split-brain state (`alias.CONST` stale vs. mutated function view). Both engines now run
the full semantic gate (VariableAnalyzer + TypeChecker) at import time, and module
constants are re-marked `const` inside module frames as a runtime backstop
(MEMORY_MODEL.md MM-4). Regression test: `tests/bugs/bug_mm4_module_const_guard.zy`.

### ~~L22 — Root-scope constants vanished at call depth ≥ 2~~ Fixed in v0.0.8

The tree-walker forwarded constants only one frame deep, so any function-calling-
function chain (including recursion and lambda frames) lost them at depth ≥ 2 even
though semantic analysis accepted the program. Top-level `:=` constants now live in a
global table not swapped by call frames: visible and immutable at any depth; module
frames still never see script constants (MEMORY_MODEL.md MM-9). Regression test:
`tests/bugs/bug_mm9_const_call_depth.zy` (TW == VM).

### L23 — VM: each import alias gets its own module state copy *(open)*

In the tree-walker, module state identity is per file path — two aliases to the same
module share one state (by design, GUIDE §17). The VM compiles per-alias state, so
`a::increment()` is invisible through `b::get_value()`. Workaround: import each module
under a single alias per program. (MEMORY_MODEL.md MM-10.)

### L24 — Leftover loop-iterator value differs between engines *(open)*

When `@ i:1..3 { }` reuses a pre-declared outer `i` (GUIDE §8), the value left after
the loop differs: the tree-walker leaves the last executed value (`3`), the VM leaves
the first out-of-range value (`4`). Do not rely on the leftover value.
(MEMORY_MODEL.md MM-11.)

---

## 20b. Error Taxonomy

Zymbol errors are classified into three categories based on when and how they are detected.

---

### Parser Errors

Detected during the parsing phase — the source text does not conform to the grammar. Execution never begins.

```
Error [line N, col M]: unexpected token '...' — expected '...'
```

**Common triggers:**
- Unmatched braces, brackets, or parentheses
- Operator with missing operand (e.g., `+` with no right-hand side)
- Invalid label syntax
- Malformed string literal or interpolation

Parser errors are always fatal. They cannot be caught with error-handling syntax (`!?`, `$!`).

---

### Semantic Errors

Detected after parsing, during the semantic analysis phase. The grammar is valid but the code violates a language rule.

```
Error [line N]: undefined variable 'x'
Error [line N]: module 'mod' is private
```

**Common triggers:**
- Reference to an undefined variable or function
- Calling an undefined function (bare-identifier calls are statically checked since v0.0.7)
- Accessing a private module from outside
- Circular imports

Semantic errors are always fatal and cannot be caught at runtime. They are reported before execution starts.

---

### Runtime Errors

Detected during execution. The code is grammatically and semantically valid, but a condition fails at runtime.

```
RuntimeError: ##kind(message)
```

Runtime errors in Zymbol are **values** — they propagate through the call stack until caught or they terminate execution. They can be caught with:

- `!? { } :! { }` — try/catch block; `_err` holds the error as `##Kind(message)`
- `:! ##Kind { }` — typed catch clause, matches a specific error kind
- `:> { }` — finally block (always executes, regardless of error)

Related operators:
- `$!` — returns `#1` if the value is an error, `#0` otherwise
- `$!!` — re-propagates an error value from within a named function to its caller (see §16)

**Common sources:**
- Index out of bounds: `arr[99]` when array has fewer elements
- Division by zero: `x / 0`
- Named tuple field not found: `t.nonexistent`

Runtime errors carry a **kind** (e.g., `##Index`, `##Div`, `##Type`) and a **message** string. The value in `_err` has the format `##Kind(message)`. The `#?` type symbol of an error value is the kind itself — `(##Index, N, ...)` — there is no generic error type symbol.

```zymbol
!? {
    v = arr[99]
} :! ##Index {
    >> _err ¶   // ##Index(array index out of bounds: index 99 for array of length 3)
}
```

---

### Soft Errors from the Standard Library (v0.0.7)

`std/json`, `std/io`, `std/net`, and `std/db` distinguish two failure channels:

- **Hard `RuntimeError`** — programmer mistakes (wrong argument type or count). The
  program is malformed; execution aborts as usual.
- **Soft `Error` value** — recoverable environmental failures (file not found, network
  timeout, malformed JSON, SQL error). The function **returns** an error value instead of
  aborting; test it with `$!`, propagate it with `$!!`, or catch it with `!? … :! ##Kind`.

| Kind | Returned by |
|------|-------------|
| `##Parse(...)` | `json::decode` / `json::decode_map` / `json::encode` on malformed data |
| `##IO(...)` | `std/io` functions on filesystem failure |
| `##Network(...)` | `std/net` functions on HTTP/connection failure |
| `##DB(...)` | `std/db` functions on SQL/ODBC failure |

```zymbol
<# std/io => io
txt = io::read("missing.txt")
? txt$! { >> "could not read" ¶ }
```

---

### Fail-safe Operations

Some operations are intentionally **fail-safe**: they never raise a runtime error; instead, they return a neutral value on failure.

| Operation | Failure result |
|-----------|---------------|
| `#\|"abc"\|` — numeral conversion | original string unchanged |
| `arr$? val` — contains | `false` |
| `#?val` — safe type check | `false` (never errors) |

Fail-safe operations are distinguished from error-handling by the absence of any error path — they are guaranteed to return a valid value of a predictable type.

---

## 21. Complete Symbol Reference

| Symbol | Operation | Example |
|--------|-----------|---------|
| `=` | Assignment | `x = 5` |
| `[..] =` | Array destructure | `[a, b, *rest] = arr` |
| `(..) =` | Tuple destructure | `(name: n, age: a) = t` |
| `:=` | Constant | `PI := 3.14` |
| `>>` | Output | `>> "hello" ¶` |
| `<<` | Input | `<< "prompt: " var` |
| `<< <typespec>` | Typed/validated input (v0.0.7) | `<< ##.(5,2) "p: " v`, `<< ###(4) "n: " n`, `<< ##"(20) "s: " s`, `<< ##' "c: " c` |
| `@~` | Sleep (ms) | `@~ 500` |
| `>>!` | Clear screen | `>>!` |
| `>>?` | Query terminal size | `[H, W] = >>?` |
| `>>~` | Positioned/styled output | `>>~ (5, 10) > "text"` |
| `<<\|` | Blocking key input | `<<\| k` |
| `<<\|?` | Non-blocking key input | `<<\|? k` |
| `>>\|` | TUI block (alternate screen + raw mode) | `>>\| { ... }` |
| `¶` / `\\` | Newline in output | `>> msg ¶` |
| `?` | If | `? x > 0 { }` |
| `_?` | Else-if | `_? x < 0 { }` |
| `_` | Else / wildcard | `_{ }` |
| `??` | Match | `?? x { pat => val }` |
| `[p, q]` | Match list pattern | `?? arr { [_, _] => ... }` |
| `@` | Loop (while) | `@ cond { }` |
| `@` | Loop (times) | `@ N { }` — repeats exactly N times when N is a positive Int |
| `@` | Loop (infinite) | `@ { }` |
| `@!` | Break | `@!` or `@! label` |
| `@>` | Continue | `@>` or `@> label` |
| `->` | Lambda | `x -> x * 2` |
| `<~` | Return / output param | `<~ value` |
| `\|>` | Pipe | `val \|> fn` or `val \|> fn(_)` |
| `$#` | Length | `arr$#` |
| `$+` | Append by value | `arr$+ elem` |
| `$+[i]` | Insert at position | `arr$+[2] elem` |
| `$-` | Remove first by value | `arr$- val` |
| `$--` | Remove all by value | `arr$-- val` |
| `$-[i]` | Remove at index | `arr$-[1]` |
| `$-[i..j]` | Remove range (1-based inclusive) | `arr$-[2..3]` |
| `$-[i:n]` | Remove range (count-based) | `arr$-[2:2]` |
| `$?` | Contains | `arr$? val` |
| `$??` | Find all indices of value | `arr$?? val` |
| `arr[i] = val` | Direct element update (arrays only) | `arr[2] = 99` |
| `arr[i] += val` | Compound element update (arrays only) | `arr[1] += 5` |
| `arr[i]$~` | Functional update — returns new collection | `arr[2]$~ 99` |
| `arr[i>j]$~` | Deep functional update (nested) | `m[1>2]$~ 99` |
| `nt[i]$~` / `nt["f"]$~` | Named-tuple update by index or field name | `p["y"]$~ 42` |
| `arr[i>j]` | Scalar deep access (row i, col j) | `m[2>3]` → `6` |
| `arr[i>j>k]` | Scalar deep access depth 3+ | `cubo[1>2>1]` |
| `arr[(e)>j]` | Computed first step | `m[(n)>(n)]` |
| `arr[a>b]` | Variable indices as nav atoms | `m[row>col]` |
| `arr[-1>-1]` | Negative indices in nav path | last row, last col |
| `arr[[i>j]]` | Flat extraction — single path wrapped | `m[[2>3]]` → `[6]` |
| `arr[p ; q]` | Flat extraction — multiple paths | `m[1>1 ; 2>3]` → `[1, 6]` |
| `arr[[g] ; [g]]` | Structured extraction | `m[[1>1] ; [2>3]]` → `[[1], [6]]` |
| `arr[[p,q] ; [r,s]]` | Structured, multi-value groups | `m[[1>1,1>3] ; [3>1,3>3]]` |
| `arr[i>r1..r2]` | Range on last step (expand axis) | `m[[1>2..3]]` → `[2, 3]` |
| `arr[r1..r2>j]` | Range on intermediate step (fan-out) | `m[[1..2>3]]` |
| `$[i..j]` | Slice (1-based inclusive) | `arr$[1..3]` |
| `$[i:n]` | Slice (count-based) | `arr$[1:2]` |
| `$^+` | Sort ascending (primitives) | `arr$^+` |
| `$^-` | Sort descending (primitives) | `arr$^-` |
| `$^` | Sort with comparator (tuples) | `arr$^ (a,b -> a.f < b.f)` |
| `$>` | Map | `arr$> (x -> f(x))` |
| `$\|` | Filter | `arr$\| (x -> cond)` |
| `$<` | Reduce | `arr$< (0, (a,x) -> a+x)` |
| `$~~[p:r]` | String replace | `s$~~["o":"0"]` |
| `$/` | String split by char or substring | `"a,b" $/ ','` |
| `$++` | ConcatBuild — append to string or array | `"x=" $++ n flag` |
| `$*` | String repeat | `"=" $* 20` |
| `!?` | Try | `!? { } :! { }` |
| `:!` | Catch | `:! ##Div { }` |
| `:>` | Finally | `:> { }` |
| `$!` | Is error | `val$!` |
| `$!!` | Propagate error | `val$!!` |
| `#\|x\|` | Numeric eval (ASCII + 69 Unicode scripts) | `#\|"42"\|`, `#\|"๔๒"\|` |
| `x#?` | Type metadata | `42#?` |
| `#.N\|x\|` | Round N decimals | `#.2\|3.14159\|` |
| `#!N\|x\|` | Truncate N decimals | `#!2\|3.14159\|` |
| `##.expr` | Cast to Float | `##.42` → `42` (Float) |
| `###expr` | Cast to Int (rounding) | `###3.7` → `4` |
| `##!expr` | Cast to Int (truncating) | `##!3.7` → `3` |
| `#,\|x\|` | Comma format | `#,\|1234567\|` |
| `#^\|x\|` | Scientific notation | `#^\|12345.0\|` |
| `0x`, `0b`, `0o`, `0d` | Base literals | `0x41` → `'A'` |
| `#` | Module declaration | `# name` |
| `#>` | Module export | `#> { fn, CONST }` |
| `<#` | Module import | `<# ./mod => alias` |
| `=>` | Alias / re-export rename | (used in `<#` import, `#>` export, and match arms) |
| `::` | Module function call | `m::func(args)` |
| `.` | Member access | `tuple.field` |
| `<\ cmd \>` | BashExec | `<\ ls -la \>` |
| `</ f.zy />` | Execute script | `</ ./sub.zy />` |
| `>< args` | CLI args capture | `>< args` |
| `\ var` | Explicit lifetime end | `\ x` |
| `#1` / `#0` | Bool true / false | `? #1 { }` |
| `#d0d9#` | Numeral mode switch | `#०९#` (Devanagari), `#09#` (reset) |
| `++` / `--` | Increment / decrement | `x++` |
| `+=` `-=` `*=` `/=` `%=` `^=` | Compound assignment | `x += 5` |
| `x°` / `°x` | Hot definition — auto-init to neutral value on first use | `°sum += item` |

---

