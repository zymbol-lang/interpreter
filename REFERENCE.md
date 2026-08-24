# Zymbol-Lang — Language Reference

Complete lookup reference: known limitations, error taxonomy, and symbol table.

**Interpreter version**: v0.0.9

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
Regression test: `zyquality/corpus/errors/semantic/no_false_positive_unused.zy` (`zymbol check`
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

Regression test: `zyquality/corpus/lambdas/error_propagate_lambda.zy` (TW == VM parity).

### L11 — Arrays must be homogeneous *(by design)*

All elements of an array must share the same type. This is enforced by the semantic checker:

```zymbol
record = ["English", "en.zy", #0]    // ❌ String + String + Bool
```

**Why**: arrays are Zymbol's ordered mutable collection for uniform data — sequences of the same kind of value. This constraint enables type-safe collection operations (`$>`, `$|`, `$<`, `$^`) without runtime type dispatch.

A deliberate mix in an array is **declared** with `#[…]`, which is the same type
and is not checked. `#?` tells them apart from v0.0.9 — `##]` when the elements
are all one type and `##[` when they are not — and that is a reading of the
value, not a second type: an array out of `json::decode` answers `##[` with no
mark written anywhere, and `#[1, "dos"]$-[2]` answers `##]`:

```zymbol
mixto = #[#0, 1, '2', "tres", 4.0]     // ✅ the mix is declared
```

**Heterogeneous records belong in dictionaries**, which are key-addressed:

```zymbol
// ✅ Dictionary — heterogeneous, mutable, key-addressed:
record = #(lang: "English", file: "en.zy", active: #1)
>> record.lang ¶
>> record.active ¶

// ✅ Array of dictionaries — uniform container of heterogeneous records:
langs = [
    #(lang: "English", file: "en.zy",  active: #1),
    #(lang: "Spanish", file: "es.zy",  active: #1),
    #(lang: "Chinese", file: "zh.zy",  active: #0)
]
@ entry:langs {
    ? entry.active { >> entry.lang " → " entry.file ¶ }
}
```

The design distinction maps cleanly: **`[…]` = typed sequences**, **`#[…]` = a
declared mix**, **`(a: 1)` = key-addressed records**, **`(1, 2)` = a fixed,
immutable group of values that travel together.

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
is unchanged. Regression test: `zyquality/corpus/errors/semantic/const_destructure_overwrite.zy`.

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
`zyquality/corpus/bugs/bug_l16_try_scope_restore.zy` (TW == VM parity).

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
`zyquality/corpus/bugs/bug_mm1_hot_def_fn_scope.zy` (TW == VM).

### ~~L19 — Module-state mutations by intra-module calls were lost~~ Fixed in v0.0.8

In the tree-walker, only the frame called directly via `alias::` wrote module state
back; a private helper's mutation was discarded, and the outer frame then clobbered
the store with its stale copy. Write-back now runs for every module frame and is
diff-based (only changed keys persist), same-module nested calls see the caller's live
values, and the caller's copies are refreshed on return (MEMORY_MODEL.md MM-2).
Regression test: `zyquality/corpus/bugs/bug_mm2_module_state_helper.zy` (TW == VM).

### ~~L20 — `\ x` inside a function poisoned the caller's same-named variable~~ Fixed in v0.0.8

The destroyed-names set was global, so `\ x` inside a callee made the caller's own `x`
raise a false `use after destruction`. The set is now saved/restored per call frame
(MEMORY_MODEL.md MM-3). Regression test: `zyquality/corpus/bugs/bug_mm3_destroy_frame_local.zy`.

### ~~L21 — Modules loaded at runtime skipped semantic analysis~~ Fixed in v0.0.8

`zymbol run` only lexed + parsed imported modules, so semantic-only violations inside
module functions (e.g. reassigning a `:=` module constant) executed silently, leaving
split-brain state (`alias.CONST` stale vs. mutated function view). Both engines now run
the full semantic gate (VariableAnalyzer + TypeChecker) at import time, and module
constants are re-marked `const` inside module frames as a runtime backstop
(MEMORY_MODEL.md MM-4). Regression test: `zyquality/corpus/bugs/bug_mm4_module_const_guard.zy`.

### ~~L22 — Root-scope constants vanished at call depth ≥ 2~~ Fixed in v0.0.8

The tree-walker forwarded constants only one frame deep, so any function-calling-
function chain (including recursion and lambda frames) lost them at depth ≥ 2 even
though semantic analysis accepted the program. Top-level `:=` constants now live in a
global table not swapped by call frames: visible and immutable at any depth; module
frames still never see script constants (MEMORY_MODEL.md MM-9). Regression test:
`zyquality/corpus/bugs/bug_mm9_const_call_depth.zy` (TW == VM).

### ~~L23 — VM: each import alias gets its own module state copy~~ Fixed in v0.0.8

The VM compiler recompiled a module on every import, allocating fresh global-variable
slots per alias — so `a::increment()` was invisible through `b::get_value()`, and
diamond dependencies (two modules importing the same third module) held divergent
copies. The compiler now caches compiled modules by canonical file path: any later
import — another alias or another importer — binds to the same chunks and global
slots, matching the tree-walker's per-path state identity (GUIDE §17,
MEMORY_MODEL.md MM-10). Regression test: `zyquality/corpus/bugs/bug_mm10_alias_shared_state.zy`
(two aliases + diamond, TW == VM).

### ~~L24 — Leftover loop-iterator value differs between engines~~ Fixed in v0.0.8

When `@ i:1..3 { }` reuses a pre-declared outer `i` (GUIDE §8), the VM left the first
out-of-range value (`4`) while the tree-walker leaves the last executed value (`3`).
The VM's range loops now advance a hidden counter and publish it to the named
iterator at the top of each iteration — leftover value and body-write semantics
(writes to the iterator inside the body cannot alter the iteration) match the
tree-walker exactly (MEMORY_MODEL.md MM-11). Regression test:
`zyquality/corpus/bugs/bug_mm11_iterator_leftover.zy` (7 variants, TW == VM).

### ~~L25 — Juxtaposition did not work inside call arguments~~ Fixed in v0.0.8

Implicit concatenation existed only at statement level, so `f(a " " b)`,
`[a " " b]` and `(a " " b)` were parse errors and every composed string handed
to a function needed an intermediate variable first. It now works in call
arguments, array elements, tuple elements and grouped expressions. A comma
still separates, and a following `(` never continues the chain in those
positions (it is ambiguous with a lambda, a tuple and a grouped expression) —
GUIDE §13. Found while building zy-GO, whose side panel spent six variables on
nothing else. Regression test:
`zyquality/corpus/strings/30_juxtaposition_delimited.zy` (TW == VM).

### ~~L26 — Variable used only as a range bound warned "unused"~~ Fixed in v0.0.8

`total = xs$#` followed by `@ i:1..total { }` reported `total` as an unused
variable even though the loop header reads it. The unused-variable analyzer
skipped the `start`/`end`/`step` expressions of a range, so a name used only as
a bound never counted as a use. The warning was noisy rather than wrong (the
program ran correctly), but it fired non-deterministically depending on how many
other variables shared the scope. Fixed by analyzing the range bounds; a
genuinely unused variable still warns. Found in zy-GO's `設定描画`. Regression:
`crates/zymbol-semantic/tests/underscore_semantics.rs` (three cases).

### ~~L27 — Misuse of a `std/` module reached run time unreported~~ Fixed in v0.0.8

`std/` modules are native, with no file on disk for the tooling to read, so an
alias bound to one was a blind spot: `math::inventada(2.0)`, `m::PI()` (calling a
constant), `m.sin` (reading a function) and a typo in a re-export
(`t::widht => ancho`, which silently breaks every caller of an i18n layer) all
passed `zymbol check` and showed nothing in the editor. `zymbol_common::stdlib`
now holds the export table — names plus arity, kept in step with both engines by
`crates/zymbol-cli/tests/stdlib_parity.rs` — and `zymbol check` and the LSP both
report through `zymbol_semantic::check_stdlib_access`, with a "did you mean" for
near misses. A named-tuple field may share an alias's name (`resp.json.user`), so
only a name that does not itself follow `.` or `::` is read as a module access.
Regression: `crates/zymbol-semantic/src/stdlib_access.rs` (8 cases),
`crates/zymbol-cli/tests/cli_check_stdlib.rs` (4 cases).

### ~~L28 — A qualified call's argument count was never checked, and the VM ran it anyway~~ Fixed in v0.0.8

The argument-count check only ever looked at a bare identifier as the callable, so
`f("a","b")` was reported while the same mistake written `m::f("a","b")` was not — and
neither was `math::sqrt(4.0, 9.0)`, even though L27 had already put every `std/`
function's arity in `zymbol_common::stdlib`; the number was recorded and never read.

Worse, the two engines then disagreed. The tree-walker raised. The VM did not check at
all: `Instruction::Call` discarded `arg_regs.len()` and copied every argument into the
callee's register window, so one argument too many **overwrote one of the callee's own
locals** and execution continued with corrupted state, while one too few left a
parameter reading as `Unit`. `CallBuiltin` likewise ignored the surplus, so
`math::sqrt(4.0, 9.0)` printed `2` under `--vm` and raised under the tree-walker.

`crates/zymbol-semantic/src/call_arity.rs` now builds the alias → function → arity table
(following re-exports, depth-capped, `-1` for variadic `std/` functions such as
`net::get`), injected via `TypeChecker::set_module_arities` by `zymbol check`, `zymbol run`,
`zymbol build` and the analyzer alike — so the CLI, the editor and both engines agree, and
a mismatch is fatal before execution in every one of them. Supplying the table only to
`check` would have left `run` rejecting `f(a, b)` while executing `m::f(a, b)`: the same
mistake, two behaviours.

The VM compiler also emits `RaiseError` at the call site, with the tree-walker's exact
wording. That is a backstop for callers that drive the compiler and VM without semantic
analysis, not the reported path — without it, `Instruction::Call` still copies a surplus
argument over one of the callee's registers.

Found by the zyml engine, which had the rule and rejected `ZethyCLI/main.zy` over a call
the Rust tooling accepted. Both programs it caught had the bad call on a rarely taken
branch — ZethyCLI's "Ollama not reachable" arm, and an extra argument in ZyAudit's
`测试/test_析答.zy` that was broken under the tree-walker and worked by accident under the
VM. Rejecting before execution also puts all three engines on the same behaviour, zyml
included; only the message wording still differs. Regression: `zyquality/corpus/arity/` (7 cases,
TW == VM), `crates/zymbol-semantic/src/call_arity.rs` (6 cases).

### ~~L29 — `@!`, `@>` and labelled jumps were never checked, and the four engines disagreed~~ Fixed in v0.0.9

Nothing verified that a break had a loop to break, or that `@:outer!` named a loop that
actually enclosed it. Every engine improvised, and no two improvised alike. For
`@:outer i:1..3 { >> i ¶  @:nope! }`:

| engine | v0.0.8 behaviour |
|--------|------------------|
| tree-walker | printed `i=1`, unwound **every** enclosing loop, and continued the program. Silent. |
| register VM | `VM compile error: unsupported construct: break label 'nope' not found` — before any output |
| `zymbol.js` | printed `i=1`, unwound every loop and ended the program. Silent. |
| zyml | printed `i=1`, then a runtime error naming the label |

`zymbol check` reported nothing in any of the seven cases now in `zyquality/corpus/loops/labels/`,
which is why none of the pairwise parity suites could see it: `vm_compare.sh` compares the
tree-walker against the VM, `web/tests/test_runner.mjs` the CLI against the browser
engine, `zyml/tests/parity.sh` the CLI against zyml. Every pair was covered; the four
together never were. `tests/scripts/engine_compare.sh` now runs all four at once.

A label is lexical where it is declared and lexical where it is used, so this is decidable
statically and is now decided statically:
`crates/zymbol-semantic/src/loop_context.rs`, fatal in `check`, `run` and `build` alike, in
every engine, and reported in the editor as you type. `cfg.rs` had resolved labels this way
since it was written — its `build_break` carries the comment *"should be caught by semantic
analysis"*. This is that analysis, eleven versions later.

A **function or lambda body is a boundary**: `f() { @! }` is an error however its call
sites are nested. That was already true of the VM and of zyml; the tree-walker was the
outlier.

`@~` (sleep) is deliberately excluded. `SYMBOLS.md`, and this file's own symbol table,
described it as loop-only by inheritance from the `@` prefix; no engine has ever enforced
that, and none should — a pause does not act on the loop's control flow. The documentation
was corrected rather than the code.

Regression: `zyquality/corpus/loops/labels/` (9 cases, all four engines agree),
`crates/zymbol-semantic/src/loop_context.rs` (12 cases). Zero false positives across the
1080 `.zy` files in the workspace.

### ~~L30 — `() -> { }` parsed in two engines and not in the other two~~ Fixed in v0.0.9

A zero-parameter lambda ran in `zymbol.js` and in zyml and failed at parse time under the
tree-walker and the VM with `expected expression, found RParen`. The grammar was on the
side of the two that refused it — `lambda_params` required at least one identifier — but
that was a limit nobody had chosen: `parse_lambda` already built an empty parameter list
for the shape, and only `is_lambda_start` refused to hand it the input.

The form is now legal everywhere. `()` is unambiguous: there is no empty tuple in Zymbol,
and a call's parentheses always follow a callable, so `(` `)` `->` can only begin a lambda.
The EBNF was widened to `"(" , [ identifier , { "," , identifier } ] , ")"`.

Found by `tests/scripts/engine_compare.sh` while checking something else — no pairwise
suite covers it, because the two engines that agreed with each other were on opposite sides
of each pair. Regression: `zyquality/corpus/lambdas/29_zero_param_thunk.zy`, run through all four.

### ~~L31 — the browser engine never checked argument counts~~ Fixed in v0.0.9

L28 made a wrong argument count fatal in the Rust engines and left `zymbol.js` as it was, so
`math::sqrt(4.0, 9.0)` printed `2` in the playground and was refused outright by the CLI —
the same program, two answers, on the tool a visitor reaches for first. Five of the ten
disagreements in `web/tests/test_runner.mjs` were this.

The playground now checks all three call forms. `std/` arities ship with the engine
(`STDLIB_ARITIES` in `zymbol.js`, a copy of `zymbol-common::stdlib` that
`web/tests/test_check.mjs` compares against the Rust source on every run, so the copy cannot
drift). User-module arities come from the caller's resolver — `moduleAritiesFor` reads each
imported module and `checkSource(src, {moduleArities})` receives the table, the same split
as `module_arities` / `set_module_arities` in Rust. A qualified call whose module cannot be
resolved is left unchecked rather than guessed at.

CLI ↔ browser parity went from 527/537 to 533/538. The five that remain are unrelated
(module constants, a leftover loop iterator, a parent-path alias).

---

### ~~L32 — A destructuring pattern does not check the shape it receives~~ Fixed in v0.0.9

The pattern is purely syntactic: either bracket shape accepts either collection type, in all
four engines.

```zymbol
[a, b, c] = (1, 2, 3)    // accepted — array pattern, tuple value
(a, b, c) = [1, 2, 3]    // accepted — tuple pattern, array value
```

The two types are otherwise distinct — `(1,2,3) == [1,2,3]` is `#0`, and they print
differently — so the laxity is confined to the pattern and contradicts the rest of the
model. Its practical cost is that a function returning `<~ (a, b)` is routinely received
with `[a, b]`, and nothing marks the mismatch: `serpiente/` does this 35 times.

**Fixed.** The pattern is typed: `[ … ]` accepts only an array, `( … )` only a tuple, and a
mismatch is a runtime error — `array pattern '[ … ]' requires an array, got ##(`, spelled
identically in every engine (`zyq consensus` compares the text, and the VM's
`zymbol_type_name` had to be bypassed because it writes `##[]` where the tree-walker writes
`##]`).

It broke four places on the first run, every one of them a real mistake the laxity had been
hiding: `tui/03_terminal_size_positive.zy`, `tui/07_terminal_size_ops.zy` and two Rust tests
received `>>?` — documented in their own comments as *"returns (rows, cols) as a positional
tuple"* — with `[H, W]`. All four now say `(H, W)`. Regression tests:
`corpus/errors/runtime/destructure_pattern_type.zy` (both directions, caught with `!?` so the
message can be compared across engines).

### ~~L33 — The last name of a pattern does not absorb the remainder~~ Fixed in v0.0.9

Today a pattern truncates in one direction and under-fills in the other, and neither is
diagnosed:

```zymbol
t = (1,2,3,4,5,6,7,8,9)
(a, b, c)    = t          // c = 3      — values 4..9 dropped in silence
(p, q, r, s) = (1,2,3)    // s = ##_    — Unit, no diagnostic
```

**Decision.** The **last name of the pattern absorbs whatever remains**, and is `##_` (Unit)
when nothing remains. Destructuring therefore never fails on a length mismatch: with two
names there is always somewhere to put the rest.

```zymbol
(a, b, c) = (1,2,3,4,5,6,7,8,9)   // c = (3,4,5,6,7,8,9)
(a, b, c) = ('a','b')             // c = ##_
(a, b, c) = (1,2,3)               // c = 3
[a, b, c] = [1,2,3,4,5]           // c = [3,4,5]
```

The absorbed remainder takes the container's own shape — a tuple yields a tuple, an array
yields an array. **This also settles an existing inconsistency**: `(h, *tail) = (1,2,3)`
currently yields the *array* `[2,3]` out of a tuple.

**`*rest` keeps a distinct meaning and is not deprecated.** Without the mark, the last name
is a scalar when exactly one value remains; with it, the binding is always a collection:

```zymbol
(a, b, c)  = (1,2,3)   // c = 3     — scalar
(a, b, *c) = (1,2,3)   // c = (3)   — collection of one
(a, b, *c) = (1,2)     // c = ()    — empty collection, not Unit
```

`*` is how a caller asks for a **stable type**; bare absorption is how it asks for
convenience.

**Accepted consequence.** The type of the last binding is decided at run time by the length
of the value: `c` above is a Tuple, a Unit and an Int in three otherwise identical
statements. In particular, a function that grows one more return value keeps every existing
call site compiling, and the last variable at each site silently turns from a scalar into a
collection — `serpiente/` has 35 such sites. This was weighed against the convenience and
accepted on 2026-08-15. The planned Clippy-style lint is the intended place to flag it, not
the type checker.

**The two sub-questions, closed on 2026-08-15:**

- **A single-name pattern absorbs too.** The rule has no exceptions: `[solo] = [1,2,3]`
  binds `[1,2,3]`, where it used to bind `1`. `[x] = arr` therefore says what `x = arr`
  already said — accepted as the cost of a rule with no special cases.
- **Nothing is discarded implicitly, so no new discard syntax was added.** Absorption is
  what makes the discard question mostly go away: every value ends up under some name.
  Dropping one is the programmer's own act — name it `_something` (the declared-but-unused
  prefix the analyzer already honours) or end its life with `\name`. `_` was left exactly as
  it was: it still discards one position in an array pattern, still does not parse inside a
  positional pattern, and there is still no anonymous `*_`. By the uniform rule, a `_` in
  the last position absorbs the remainder without binding it.

**Implemented in all four engines**, each mirroring `Interpreter::bind_positional`: the
tree-walker (`bind_positional`), the VM (two new instructions, `DestructureCheck` and
`DestructureAbsorb`, since the choice between Unit, scalar and collection is only knowable at
run time), `zymbol.js` (`bindPositional`), and `zyml` (`DSeq` grew an `is_tuple` flag, as its
AST had been folding both patterns into one constructor). Regression test:
`corpus/collections/33_destructure_absorb.zy`, agreed by all four.

**Two divergences surfaced while implementing this**, neither of which the gate could see
because no corpus file exercised a `*rest` on a tuple or a `*rest` with items after it:

- the tree-walker returned `Value::Array` for *any* rest, so `(h, *tail) = (1,2,3)` gave the
  array `[2,3]` under TW and the tuple `(2,3)` under the VM (whose `ArraySlice` preserves the
  container). Both now keep the container's shape.
- the VM and `zymbol.js` ignored the items that follow a `*rest`: `[w, *x, y] = [10..50]`
  gave `x=[20,30,40] y=50` under TW and `x=[20,30,40,50] y=30` under the VM. Both now count
  those slots from the end, as the tree-walker's `trailing` always did.

### ~~L34 — An output-parameter slot accepts an expression~~ Fixed in v0.0.9

```zymbol
g(b<~) { b = b + 100 }
g(2 + 3)    // accepted; the write lands nowhere and nothing is reported
```

`<~` promises the change travels back to the caller (SYMBOLS.md §9.1). When the argument is
not assignable there is no caller variable to travel back to, and the call is accepted
anyway — the one guarantee the mark exists to make, silently withdrawn.

**Fixed.** Passing a non-assignable expression to a `<~` slot is now a semantic error,
caught before the program runs:

```
error: argument 1 of 'g' is an output parameter '<~' and needs a variable, not an expression
  help: '<~' writes the change back into the caller's variable; there is nowhere to write an
        expression back to — assign it to a variable first
```

`TypeChecker` records which slots of each function are outputs (`record_output_slots`) and
checks the arguments at each call (`check_output_arguments`). The browser engine mirrors it
in its checker — `funcOutSlots` filled beside `funcArity`, checked by `checkOutputArgs`
next to the existing `checkArity` — and words the diagnostic identically, which is what
`web/tests/test_check.mjs` compares. Functions without an output parameter — nearly all of
them — record nothing.

Regression test: `corpus/errors/semantic/output_param_expression.zy`, graded via `check`.

This was the last item left open on `zyml`, the OCaml engine: it accepted the call as
before, having no equivalent analysis pass, so the rejection would have had to happen at
compile time — the one engine of the four where this was not a check but a new piece of
machinery. `zyml` was **retired on 2026-08-17** and the item closed with it. Enforced in
every engine that exists.

### ~~L35 — `zyml` does not lex `~` in a parameter list~~ Fixed in v0.0.9

The OCaml engine rejects the working-copy mark at lexing time:

```
f(a~) { … }     → zyml: Lex error: unexpected character '~' (line 1)
g(b<~) { … }    → accepted by all four engines
```

So `~` is a three-engine feature and `<~` is a four-engine one. The parity gate cannot see
this: no corpus file uses either mark in a signature, and no `.zy` in the whole workspace
declares a `<~` parameter — the feature is documented, implemented and unexercised.

**Fixed.** `zyml` lexes `~` as `TMod` and accepts it in a signature. Two places needed it:
the lexer, which fell through to "unexpected character", and `is_func_decl`, which decides
whether `name(…) {` is a declaration by scanning the tokens between the parentheses and
admitted only `TIdent | TComma | TRet` — so a `~` made it stop reading the whole thing as a
function.

A function's scope is already isolated, so `~` needs no run-time support: it declares in the
signature the working copy the body would otherwise write by hand, and only `<~` sends
anything back.

The gate now holds both marks: `corpus/functions/param_marks.zy` exercises `~`, `<~`, the
two together, `<~` alongside a return value, and a two-output swap — agreed by all four
engines.

### ~~L36 — `<~` was invisible at the call site~~ Fixed in v0.0.9

`f(x)` did not reveal that `x` might come back changed: the mark lived only in the
signature, so a reader had to open `f` to find out which arguments a call modifies. In a
language whose whole claim is that the mark tells you what happens, that was the one place
the mark was missing.

**Fixed.** The mark is now written at the call site too, and is **required** wherever the
callee declares an output parameter:

```zymbol
bump(b<~) { b = b + 100 }

y = 2
bump(y)      // ✗ error: argument 1 of 'bump' is an output parameter and must be
             //   marked '<~' at the call site
bump(y<~)    // ✓
```

The reverse is an error too — marking an argument the callee does not declare as an output
(`calc(z<~)` where `calc(a)`) — which is what stops the annotation drifting away from the
signature. Being required is the whole point: an optional mark would be absent from exactly
the call sites nobody revisited.

This coins **no new symbol**: it is `<~` in a new position, which is the bar SYMBOLS.md sets
for extending the grammar. It has no run-time meaning — the callee's signature is still what
makes a parameter an output — so the tree-walker and the VM needed no change at all; the AST
carries `out_args` and the checkers compare it against the signature.

**The same rule holds for qualified calls**, `m::f(x<~)`. That needed a table the arity
check did not have: `module_out_slots` in `zymbol-semantic` (mirrored by `moduleOutSlotsFor`
in `zymbol.js`), built beside `module_arities` rather than by widening it — `ModuleArities`
is read in six places across three crates and none of them has anything to say about output
parameters. A module that cannot be resolved leaves its calls unchecked, exactly as the
arity check already does.

Nine corpus files had to gain the mark, which is the measure of what it buys: every one of
them was a call that modified its caller's variable without saying so.

Implemented in `zytw`, `zyvm` and `zyjs` — every engine that exists. `zyml`, the OCaml
engine, never enforced it: like L34, it had no analysis pass to host the check. It was
retired on 2026-08-17 and the gap closed with it.

### ~~L37 — the browser engine mixed diagnostics into the program's output~~ Fixed in v0.0.9

The Rust engines keep two streams apart: what the program prints goes to stdout, what the
engine has to say *about* the program — a runtime error, a refusal to run — goes to stderr,
and the process exits non-zero. `zymbol.js` had one stream, because a browser has one panel,
and wrote everything through `onOutput`.

That single difference was **69 of the 91 corpus divergences**, and it hid the rest: the gate
read as "the browser engine disagrees about 91 programs" when it disagreed about 22 and
formatted the other 69 differently. Two of the 69 were real, though, and worth separating:

- a module file run directly was refused by the CLI (stderr, exit 1) and **run** by the
  browser engine (stdout, exit 0) — `runZymbol` returned success for a program it had
  declined to execute;
- every runtime error landed in the program's own output, so a test comparing stdout could
  not tell a program that printed "Runtime error: …" from one that failed.

**Fixed** by giving `runZymbol` an optional `opts.onError`. A caller that is a process
(`web/tests/run_one.mjs`) passes one and gets the CLI's split — diagnostics on stderr, exit 1.
The playground passes none and keeps today's behaviour, everything in the one panel, which
is all a browser has; the fallback is `onOutput`, so nothing there changed. The refusal is
recorded as `Interpreter.moduleRefused` rather than emitted, so `runZymbol` can route it and
report the failure.

Corpus agreement across `zytw`, `zyvm` and `zyjs` went from 506/599 to 575/599, and the
example pool from 194/216 to 206/216.

### ~~L38 — the browser engine's remaining divergences~~ Fixed in v0.0.9

With L37's stream split in place, what the gate had been calling "91 disagreements" turned
out to be 22, and they fell into a handful of causes. All of them are now closed, and
`zytw`, `zyvm` and `zyjs` agree on **597 of 597** corpus files that all three can run.

Nothing here changed the Rust engines: every fix brought `zymbol.js` (or its Node runner) to
what the two of them already did.

| Fault | What it was |
|---|---|
| `2 ^ 3 ^ 2` | `parseExponent` used `if` instead of recursing, so it parsed one `^` and failed on the second. `^` is right-associative: 2^(3^2) = 512 |
| `1e10` | the exponent was only read after a decimal point, so `e10` was lexed as an identifier. Now read for integers too, and only when a digit follows — `e\|x\|` keeps its meaning |
| `grid$+ [0, 0]` | `$+` followed by `[` was always the positional insert. The Rust lexer distinguishes them by the space (`DollarPlusLBracket`); `zymbol.js` now emits the same two tokens |
| leftover iterator | the iterator lived in a per-iteration scope that was discarded. It now publishes to a pre-existing outer name — and takes whatever the body left there, so `@ w:1..3 { w = 100 }` leaves 100 (MM-11) |
| root constants | crossing a function boundary admitted only functions and modules, so `K := 5` was invisible at call depth ≥ 2 (MM-9) |
| module analysis | a module loaded at run time was parsed but never analysed, so one reassigning its own `:=` constant simply ran (MM-4). The checker also gained the module-scope notion `Env` already had, without which every `_private` helper looked like a scope violation |
| nested imports | the resolver was called without the importing file's path, so `<# ./module` inside `i18n/matematicas/中文.zy` was looked up beside the *entry* file. The runner now returns a child resolver rooted at the module, the shape the playground already used |
| execution ceiling | the 50 000-step cap that protects a browser tab was being applied to a Node process, failing long-but-finite programs the CLI runs happily |
| positioned output | `>>~` and `>>!` need a terminal context; without one the runner dropped the escape sequences the CLI has always written |
| `mI'` | the apostrophe did not continue an identifier, so Klingon `mI'` opened a character literal that swallowed the file. `Lexer::is_ident_continue` admits any non-whitespace, non-operator character |

Two corpus files were changed rather than an engine: `stdlib_json_decode*.zy` compared the
*text* of a malformed-JSON error, which comes from whichever JSON parser the engine embeds
and has been marked engine-specific in `zymbol.js` since v0.0.7. They now assert the contract
— a catchable Error value rather than an abort — with `$!`.

Three more turned up in `web/examples/`, which the corpus does not cover:

| Fault | What it was |
|---|---|
| `3.14159265` printed as `3.1415926499999998` | the lexer assembled a float from its parts (`3 + 14159265/100000000`) instead of parsing the literal. The digits are collected as ASCII first, so a literal written in any numeral script still parses exactly |
| `<< name` with no input | the runner's `inputFn` answered `''` for ever. `null` is the engine's documented EOF signal, and with it `<<` past the end raises "end of input while waiting for …", as the CLI does on a closed stdin |
| an empty stdin was one empty line | `''.split('\n')` is `['']`, so the first `<<` succeeded with an empty answer before EOF was ever reached. A trailing newline no longer adds a line either |

Corpus **597/597** and the example pool **210/210**, all three engines.

---

### ~~L41 — a module could not hold a collection~~ Fixed in v0.0.9

A module body accepts only literal-initialised bindings (E013, GUIDE §17), and
"literal" was implemented as *scalar*: `is_literal_expr` matched
`Expr::Literal` and a signed literal, nothing else. So all three collections
were refused as module state, at any nesting depth:

```zymbol
# catalogo {
    tabla = #(es: "hola", en: "hi")   // E013: variable initializer in module
    LADOS := [10, 20, 30]            //       must be a literal
}
```

The documentation never said this. GUIDE.md wrote "literal RHS only", and
`[1, 2, 3]` is a literal by every ordinary reading of the word — it names a
value, it does not compute one. What the rule is *for* is keeping execution out
of a module body, and a collection literal executes nothing.

**The cost was structural rather than cosmetic.** A module is where a lookup
table belongs — it is the language's only unit of shared state — so with tables
locked out, the four game applications wrote their translation catalogues as
`??` chains inside a function instead: 455 branches in zy-GO, 394 in
Chaturanga, 96 in Hov veS, 68 in Serpiente. Each one also carries a
hand-maintained list of its own keys, because a `??` chain cannot be asked what
it contains. The dictionary arriving in v0.0.9 with a computed key, `$?` and
`@ k:d` made the table expressible for the first time — and E013 was what still
kept it out of the only place it could live.

**The browser engine never had the restriction**: `zymbol.js` checks the
*shape* of a module statement (`VarAssign`, `ConstAssign`, `FuncDecl`, …) and
never looks at the initializer, so it had been running these modules all along.
This was therefore a live three-engine divergence that no suite could see —
no corpus file put a collection in a module, since two engines out of three
refused to parse one.

**Fixed.** A collection literal is a literal, recursively: an array, a
positional tuple and a dictionary qualify when every element does, so a
dictionary of dictionaries — the shape of a decoded JSON object — is one
initializer. Anything that computes is E013 as before, at any depth.

The VM needed the machinery, not just the permission: `ModuleConst` and
`GlobalInit` were both scalar-only enums, and the four sites that turn a module
constant into bytecode each emitted exactly one `Load*` instruction. They now
share one emitter (`emit_module_const`), since a collection needs a sequence —
build each element, then one instruction to gather them — and four hand-written
copies of that could not stay in agreement.

Regression test: `corpus/modules_scope/module_collection_state.zy` — exported
collection constants, private dictionary state, a key inserted into module state
and read back, and a nested dictionary. Agreed by all three engines.

**The same blind spot hid the opposite error.** `zymbol.js` did not check the
initializer at all — it checked the statement's *type*, and `VarAssign` is
allowed in a module body — so it was not only accepting collection literals but
also `x = 1 + 2` and `t = json::decode(raw)`, running them while both Rust
engines refused to parse the file. Neither direction was visible to the gate,
for the same reason: a file two engines reject has no golden for the third to
disagree with. The browser engine now applies the same rule, worded the same
way, and the form is in `reject/modules/02_computed_module_initializer.zy`.

### ~~L42 — a parameter used as a dictionary key was declared an Int~~ Fixed in v0.0.9

```zymbol
busca(d, k) { <~ d[k] }
>> busca(#(es: "hola"), "es") ¶
```

```
error: argument 2 has type String, but function 'busca' expects Int
```

Both engines run this correctly; only the checker refused it. The constraint
collector had one rule for the bracket — *"if indexing with a param, it should
be Int"* — written when the bracket only ever reached a position. Decision 7
gave the dictionary a computed key, and the bracket became two operations under
one sign: a POSITION in an array, a string or a positional tuple, always an
Int; a KEY in a dictionary, always a String.

The index cannot decide which; only the receiver can. So the constraint now
follows the receiver, and where the receiver is unknown — a parameter, a
returned value — nothing is constrained, which is the safe direction:
`infer_expr` still checks the pair at the use site.

Reading the receiver out of the type environment does not work here, because
during signature inference every local is `Any`. A pre-scan
(`collect_index_receiver_kinds`) notes which body-local names are bound to a
collection literal, and a name bound to two different kinds is dropped rather
than guessed at.

This **gained** a diagnostic as well as fixing one: an Int passed where a
dictionary key belongs is now refused before the program runs, where it used to
be accepted and fail at run time — with two different messages, one per engine.

Regression test: `corpus/collections/41_dict_key_parameter.zy`.

### ~~L43 — `zymbol fmt` refused every file that marks an output argument~~ Fixed in v0.0.9

L36 put the output mark at the call site, `f(x<~)`, and the formatter was never
taught to print it. The mark belongs to the call and not to the argument
expression — it is `out_args` on `FunctionCallExpr`, not part of the `Expr` —
so `format_function_call` walked the arguments and dropped it.

The safety gate caught the loss and refused the file, which is the fail-closed
design working exactly as intended: nothing was corrupted. What it means in
practice is that `zymbol fmt` stopped working on any file that uses the mark —
**nine corpus files**, and every application file that passes an argument by
output. The formatter suite reported it as nine P1 failures; P1 is the reparse
property, and the file never got as far as being reparsed.

Two things kept this quiet. The mark became *required* in this same release, so
the files that use it are all recent; and a refusal to format is silent unless
somebody was formatting.

Fixed by writing `<~` after the marked argument in both the inline and the
broken-over-lines paths, and counting it in the width estimate that chooses
between them.

**The same run surfaced a second one**: a float literal that overflows —
`1.0e400` — is already `inf` when the lexer is done, and `{:e}` prints `inf`,
which reads back as an identifier. Any overflowing literal produces exactly that
value, so the formatter now writes `1.0e400` for it. NaN is deliberately left
alone: no literal produces one, so a NaN could only come from somewhere the
formatter should not be guessing about, and refusing is the right answer.

### ~~L44 — a module function call copied the whole module's state~~ Fixed in v0.0.9

A module function frame is given the module's state on entry and the changes are
diffed back on exit. The tree-walker's collections are plain `Vec`s and not
reference counted, so both halves are **deep copies** — and it copied
everything, twice:

- `self.loaded_modules.get(path).cloned()` cloned the entire `LoadedModule`,
  every value in it, including `constants`, which this path never reads;
- then every module variable was cloned again into the frame, whether or not
  the body ever names it.

While module state could only be a scalar this was invisible. L41 made a table
module state, and the cost became proportional to the largest thing in the
module rather than to what the function touches — **309 ms → 26 ms** for 20 000
calls to a function that never names the module's sixty-key table, and
**502 ms → 395 ms** for the accessor that does.

**Fixed** by taking out of the module only what the frame needs: the bindings
the body actually names, and the small parts (aliases, constant names, the
function table). The mention set comes from the same exhaustive walk that
auto-free uses (`zymbol_semantic::mentioned_names`) and is computed once per
body; it is an over-approximation on purpose, since injecting too much is what
the code did before and is always safe, while injecting too little would be an
undefined-variable error.

Regression test: `corpus/modules_scope/module_state_mentions.zy` — the seven
ways a body can name a module binding, including inside a nested block, inside
a `{…}` interpolation, inside a lambda, and a write from a function that never
reads it.

**What remains**: the accessor that does name the table still copies it once per
call, because the tree-walker's values are not reference counted. The register
VM does not have this shape at all — its globals are read in place — and it is
between 10× and 20× faster on the same programs. Making the tree-walker's
collections reference counted is the real fix and is not this release's.

---

### L45 — `zymbol fmt` refuses `expr#?[i]`, which runs

```zymbol
x = [1, 2]
>> x#?[1] ¶            // runs, in all three engines
```

```text
safety gate: token stream changed at token #8: source has "Ident(\"x\")",
formatted output has "LParen"
```

`format_index` parenthesises unconditionally when the indexed expression is a
`#?`, on the grounds that *"without parens the parser sees `expr#?` as a complete
statement and then `[i]` at statement level"*. That is true where a statement
starts and false inside an expression, which is where this form actually appears
— and the visitor does not know which of the two it is in.

The added parenthesis changes the token stream, so the fail-closed gate refuses
the file rather than rewriting it. Nothing is corrupted; the file simply cannot
be formatted, silently unless somebody was formatting. Same shape as L43.

**Workaround**: write `(expr#?)[i]`, which is what the formatter wanted to
produce anyway. Two corpus files do.

**Fix**: thread the statement-versus-expression position into `format_index` and
parenthesise only at statement level. Found while adding `##(` and `##[`, not
caused by it — the form has never formatted.

### L39 — `>>` takes arithmetic, not comparison — **by design**

The output operator has a narrower grammar than the rest of the language. Everything up to and
including arithmetic works; comparison and the logical operators do not:

| Written | Result |
|---|---|
| `>> 2 + 3 * 4 ¶` | `14` |
| `>> -7 % 3 ¶` | `-1` |
| `>> -2 ^ 2 ¶` | `4` |
| `>> 1 == 1 ¶` | **parse error** |
| `>> #1 && #0 ¶` | **parse error** |
| `>> (1 == 1) ¶` | `#1` — parentheses always work |

**The cause.** Arguments are juxtaposed — `>> "x=" n " end" ¶` is three of them — so the parser
must decide where one argument ends and the next begins. `<` and `>` are the same characters
that open `<#` and `<~` and close `>>|`, and a comparison in argument position is ambiguous
against the next argument. The cut is drawn below comparison, and parentheses lift anything
over it.

**Two traps inside the rule**, both worth stating because both were wrong somewhere:

- `+` and `-` **join** two arguments rather than separating them. `>> "Score: " -95 ¶` is
  `"Score: " - 95` — an arithmetic error on a string, not two items. Write
  `>> "Score: " (-95) ¶`. Every engine agrees, which is why no suite ever flagged it.
- A leading unary is a *term*, so the rest of the arithmetic still applies to it. This was
  broken until v0.0.9: the output parser returned as soon as it saw `-`, so `>> 7 % 3 ¶`
  printed `1` while `>> -7 % 3 ¶` was `error: expected expression, found Percent`, and
  `-5 / 2`, `-3 * 4`, `-2 * -3` and `-2 ^ 2` failed with it. The resulting rule — `%` works,
  but not after a minus — was not one anybody could learn. Fixed in v0.0.9; the browser
  engine, which had been accepting the *whole* expression language here, was narrowed to the
  same cut in the same change, so a program written in the playground parses outside it.

Gate: `zyquality/corpus/output/10_unary_in_output.zy` prints every form twice, once through
`>>` and once through a variable, so a value that depends on which parser saw it is a
failure; `zyquality/reject/output/01_comparison_unparenthesized.zy` and `02` fix the limit
itself.

---

### L40 — imports come before every statement — **by design**

In an executable file, every `<#` import precedes the first statement:

```zymbol
<# std/json => js         // ✅ imports first
>> "ready" ¶
```

```zymbol
>> "ready" ¶
<# std/json => js         // ✗ error: imports must come before any statement
```

The rule is as old as the parser — the loop that reads the leading run of `<#` stops at the
first statement, and anything after it is a parse error — but it was **written in no
document** until v0.0.9, and the browser engine did not enforce it: it parsed `Import` as an
ordinary statement and ran the second program above, printing `ready` and then the encoded
JSON. A program that broke the rule worked in exactly one engine of three, and no text said
which of them was right (DM-12).

Fixed in v0.0.9 in three places at once, because the rule needed all three to be worth
anything: the browser engine enforces it, the Rust diagnostic names the rule instead of the
token (it used to be `unexpected token: ModuleImport` with `help: expected statement`), and
this entry exists. Gate: `zyquality/reject/modules/01_import_after_code.zy`.

Blank lines and comments before an import are fine — only statements close the import
section.

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
- Calling any function with the wrong number of arguments (see *Argument counts* below)
- Accessing a private module from outside
- Circular imports
- `@!` or `@>` with no enclosing loop, or a labelled jump whose label names no enclosing
  loop (see *Loop context* below)

Semantic errors are always fatal and cannot be caught at runtime. They are reported before execution starts.

---

### Loop Context

Since v0.0.9 both loop-jump rules are checked statically:

| Written | Requires |
|---------|----------|
| `@!` / `@>` | an enclosing `@` loop |
| `@:name!` / `@:name>` | an enclosing `@` loop labelled `name` |

```
error: '@!' outside a loop
  help: '@!' breaks the enclosing '@' loop; there is none here. A function or
        lambda body does not see the caller's loops.

error: no enclosing loop is labelled 'nope'
  help: labels in scope here: 'outer'
```

A **function or lambda body is a boundary** — the caller's loops are not in scope inside a
callee, so `f() { @! }` is an error however its call sites are nested.

`@~` (sleep) carries no such requirement: it pauses execution without acting on the loop's
control flow, and is legal at top level.

Fatal before execution, in `zymbol check`, `zymbol run` and `zymbol build`, and identically
in the tree-walker, the register VM and `zymbol.js` — and in zyml, the OCaml engine, until it
was retired on 2026-08-17. See L29 for what these four did before, which was four different
things; being the fourth answer is what made the disagreement impossible to defend.

---

### Argument Counts

Since v0.0.8 every call form is checked against the callee's parameter list, whichever way
it is written:

| Call form | Example |
|-----------|---------|
| Bare identifier | `f("a", "b")` |
| Module alias | `ui::show_error("a", "b")` |
| Standard library | `math::sqrt(4.0, 9.0)` |

A mismatch is a **semantic error**: fatal, reported before execution begins, by `zymbol
check`, `zymbol run` and `zymbol build` alike, and identically under both engines. It is
fatal even where the call could never run —

```zymbol
? (#0) {
    s::saluda("ana", "sobra")   // rejected; the program does not start
}
```

— because the check is static and an argument-count mismatch is never intentional. This is
what the language had always done for the bare-identifier form; v0.0.8 aligned the other
two with it rather than inventing a rule for them.

A `std/` function declared variadic — `net::get`, which takes a URL with or without a
header map — accepts any count and is never reported.

Before v0.0.8 only the bare-identifier form was checked. The other two passed `zymbol check`
silently, and the two engines then disagreed: the tree-walker raised at run time, while the
VM ignored the mismatch — a surplus argument was written over one of the callee's own
registers and execution continued with corrupted state. See `zyquality/corpus/arity/` for the
regression corpus, which covers all three call forms plus the variadic and dead-branch
cases.

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
- Division by zero: `x / 0`; modulo by zero: `x % 0`
- Integer overflow: any result outside ±(2⁵³ − 1) — see [Numeric limits](#numeric-limits)
- Named tuple field not found: `t.nonexistent`

Runtime errors carry a **kind** (e.g., `##Index`, `##Div`, `##Type`, `##Range`, `##Key`) and a **message** string. The value in `_err` has the format `##Kind(message)`. The `#?` type symbol of an error value is the kind itself — `(##Index, N, ...)` — there is no generic error type symbol.

```zymbol
!? {
    v = arr[99]
} :! ##Index {
    >> _err ¶   // ##Index(array index out of bounds: index 99 for array of length 3)
}
```

---

### Numeric limits

| Type | Range | Leaving it |
| --- | --- | --- |
| `Int` | −9007199254740991 … 9007199254740991, i.e. ±(2⁵³ − 1) | `##Range` error |
| `Float` | IEEE-754 binary64 | `inf` / `-inf` — a value, not an error |

**Float semantics** are IEEE-754 throughout, which is narrower than "whatever the
host language does" and had to be made so in all four engines:

| | Rule |
| --- | --- |
| `==` | **Exact.** No tolerance. `0.1 + 0.2 == 0.3` is `#0`. Int/Float promotion applies first, so `1.0 == 1` is `#1`. |
| `NaN` | False in **every** direction, itself included: `n == n`, `n < 1.0`, `n <= 1.0` and `n >= 1.0` are all `#0`; only `n <> n` is `#1`. |
| `-0.0` | Keeps its sign in output: prints `-0`. |
| Output | Always plain digits, never an exponent — `1.0e21` prints `1000000000000000000000`. Ask for scientific notation with `#^`. |

> The tree-walker used to compare floats with an **absolute** tolerance
> (`|a−b| < f64::EPSILON`), which called `1e-20` and `-5e-20` equal, broke
> transitivity, and did nothing at all near 1e300. The tolerance had been added
> to make `1.0 == 1` work; promotion is what actually does that.

> A three-state comparator cannot express "no ordering", and all four engines had
> collapsed NaN into *equal* somewhere. `<`, `<=`, `>`, `>=` now test an
> `INCOMPARABLE` code rather than the sign of a comparison result.

The integer bound is the mantissa of a double, chosen so that **every engine
represents every Zymbol integer exactly and natively**: `i64` in the tree-walker
and the register VM, a `Number` in the browser, and — when the bound was
chosen — OCaml's 63-bit `int` in `zyml`. Nothing is boxed, nothing is a
`BigInt`, and no engine is approximating, which is why an integer means one
thing across all of them.

**The browser is the constraint that binds**, and it still does: `Number` is an
f64, so 2⁵³−1 is where exactness ends. OCaml's 63-bit `int` was the looser of
the two and would have allowed more; `zyml` was retired on 2026-08-17 and the
bound does not move, because the engine that set it is the one still shipping.

Before v0.0.9 there was no rule at all and each engine used its host's. The same
program gave four answers — this is the observation the rule exists to answer,
kept verbatim although `zyml` has since been retired:

```zymbol
>>(10 ^ 20) ¶
// zytw  Runtime error: power operation overflow      (the only engine that noticed)
// zyvm  7766279631452241920                          (the low 64 bits)
// zyml  -1457092405402533888                         (the low 63 bits)
// zyjs  100000000000000000000                        (right by luck: 10²⁰ = 2²⁰·5²⁰)
```

`##Range` is raised by:

| Source | Example |
| --- | --- |
| `+`, `-`, `*`, `^` | `9007199254740991 + 1` → `integer overflow: 9007199254740991 + 1` |
| An integer literal | `9223372036854775807` → `integer literal out of range` (lexical, not runtime) |
| `###` / `##!` on a float | `###1.0e300` → `integer overflow: ### cannot represent this float` |

Operations that **cannot** raise it: unary `-` (the range is symmetric), `/` and
`%` on integers (a quotient or remainder of in-range operands is in range), and
anything on floats.

Readers of outside data do not raise it either — they degrade, because the data
is not the program's mistake. A JSON number past the range becomes a `Float`; a
`BIGINT` column past it stays the `String` the driver sent, like `DECIMAL`; `#|x|`
on digits past it returns the string unchanged rather than a rounded number.

> A quantity that may exceed the range — a position hash, an accumulator over a
> long loop — must be reduced as it goes (`h = h % 1000000`, as `Chaturanga`'s
> position hash does) or held as a `Float`.

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
| `##Time(...)` | `std/time` on a date that does not exist, an unknown zone or unit, an unreadable pattern, or a local zone the machine cannot report |

`##Key` is raised by reading a dictionary key that is not there — through the dot
or through the bracket, since it is the same lookup:

```zymbol
u = #(nombre: "Ana", edad: 30)
!? {
    >> u["sueldo"] ¶
} :! ##Key {
    >> _err ¶   // ##Key(no key 'sueldo' in dictionary — available: nombre, edad)
}
```

It is Python's `KeyError`, not JavaScript's `undefined`, and it is coherent with
`a[0]`, which is also an error rather than a silently wrong answer. It is what
makes `d$? "clave"` necessary: a dictionary built piece by piece has to be
askable before it is read.

`std/term` (v0.0.8) has no soft-error channel: its five functions are pure measurements
over a string and cannot fail environmentally.

`##Time` (v0.0.9) is the one soft channel that is mostly not environmental: a date arrives
from a form, a file or a database column, and the 30th of February is *data* that a program
has to be able to handle rather than a bug that should stop it. Only the `"local"` zone can
fail for the usual environmental reason — the machine's zone could not be read — and it
fails rather than guessing, because a wrong date is worse than a caught error. A wrong
argument **type** stays hard, as everywhere else in `std/`.

```zymbol
<# std/io => io
txt = io::read("missing.txt")
? txt$! { >> "could not read" ¶ }
```

---

### Package Errors (`.zyp`, v0.0.8)

These are **CLI-level** errors from `zymbol package` / `zymbol run pkg.zyp`, not values a
Zymbol program can catch — they occur before (or instead of) any code running.

| Condition | Message |
|-----------|---------|
| Archive missing or unreadable | `cannot open package '<path>': <cause>` |
| No `zyp.toml` at the archive root | `zyp.toml not found in archive (expected at the archive root)` |
| No `zyp.toml` in the packaged directory | `zyp.toml not found at <path> (pass a directory containing one, or use --script to synthesize one)` |
| Malformed manifest | `invalid zyp.toml: <detail>` |
| `--script NAME` doesn't exist | `no script named 'NAME' in this package` |
| Several scripts, none `default = true`, none named | `no default script declared in zyp.toml, and none selected with --script (use --script <name>)` |
| Two `[[script]]` entries share a `name` | `duplicate [[script]] name '<name>' in zyp.toml` |
| More than one `default = true` | `more than one [[script]] is marked default = true: <names>` |
| `engine` requirement not satisfied | `package '<name>' requires engine <req>, this interpreter is <current>` |
| Unsafe path in a ZIP entry or `[[script]].path` | `unsafe path in package: '<p>' — paths must be relative and stay inside the package (no '..', no leading '/', no drive letter, no backslash)` |
| Entry or total decompressed size over 100 MiB | `archive entry '<name>' is too large (exceeds the <n>-byte decompression limit)` |
| A `[[script]]` that is a module file | `script '<name>' (<path>) is a module file (has a # module declaration) — modules are imported with <#, not run directly` |
| Declared script absent from the archive | `script '<name>' is declared in zyp.toml as '<path>', but that file is not in the archive` |

**Path containment**: a ZIP entry name and a `[[script]].path` obey one lexical rule — no
`..` component, no absolute prefix, no backslash, no NUL, no Windows drive letter — enforced
at manifest parse time, at extraction, and at write time.

Everything the closure cannot resolve statically is a **warning**, not an error
(`W001`–`W011`); see [GUIDE.md § Distributing a Multi-File Program](GUIDE.md#distributing-a-multi-file-program-zyp).

---

### Memory: Automatic Destruction at Last Use (v0.0.8)

A variable's memory is released right after the statement containing its **last use**,
rather than at scope end. This is on in both engines and **unobservable by design**: it
never changes the behavior of a correct program, it only lowers peak memory.

Never auto-freed (conservative exclusions): constants, hot names (`x°`/`°x`), `_`-prefixed
names, module-level bindings, output/mutable parameters, and free variables of named
functions used as first-class values.

If an auto-destroyed name is ever read — impossible in a correct program — the tree-walker
raises a distinctive `internal: use after auto-destruction` error rather than silently
producing a wrong value. String interpolation reports it too, instead of printing `{var}`
verbatim.

**Known limitation (VM)**: `emit_auto_free` clears the *named* variable's register, but a
temporary holding the same large value survives until its register is reused — so the VM's
peak-memory win is smaller than the tree-walker's.

---

### Fail-safe Operations

Some operations are intentionally **fail-safe**: they never raise a runtime error; instead, they return a neutral value on failure.

| Operation | Failure result |
|-----------|---------------|
| `#\|"abc"\|` — numeral conversion | original string unchanged |
| `arr$? val` — contains | `#0`, including when the value is of another type (`[1,2,3]$? "x"` → `#0`) |
| `val#?` — type metadata | always the 3-tuple `(type_symbol, count, display)`; there is no failure path. **Postfix only** — `#?val` does not parse |

Fail-safe operations are distinguished from error-handling by the absence of any error path — they are guaranteed to return a valid value of a predictable type.

### Warnings that a correct program still gets

These are warnings, not errors: the program runs, and `zymbol check` exits 0. They are
listed because each one appears in ordinary correct code, so a reader who does not expect
them will hunt for a bug that is not there (verified 2026-08-17, both engines).

| Warning | When | How to silence it |
|---------|------|-------------------|
| `ambiguous lifetime for 'i'` · *variable is modified inside a loop* | Every **named iterator** of a `@` loop with a binder — `@ i:1..3`, `@ f:arr`, `@ c:"hola"`. A `while` header and an outer variable written inside the body do **not** trigger it. | Prefix the iterator: `@ _i:1..3`. Use a bare name only when the value is genuinely needed after the loop. |
| `range direction is decided at runtime` | A range bound that is not a literal — `@ i:1..MAX`. If the end turns out lower than the start, the loop counts **down** rather than not running. | Nothing to silence: guard the empty case, or accept the descending walk. |
| `unused variable 'x'` | A variable that is never read. Reads inside string interpolation (`"{x}"`) and inside `<\ … \>` **do** count as uses (L9). | Prefix `_` when the variable is deliberate: `_unused = 42`. |
| `loop expects a count or a condition, got [Int]` | The analyzer can tell statically that a `@` specifier is neither. It warns rather than errors because the inference is approximate — the engines refuse the form at run time regardless. | Use `@ x:items` to walk a collection, `@ items$#` to count it. |

---

## 21. Complete Symbol Reference

| Symbol | Operation | Example |
|--------|-----------|---------|
| `=` | Assignment | `x = 5` |
| `[..] =` | Array destructure | `[a, b, *rest] = arr` |
| `(..) =` | Positional tuple destructure | `(a, b) = t` — the form that receives a `<~ (a, b)` return |
| `(n: ..) =` | Named tuple destructure | `(name: n, age: a) = t` |
| `:=` | Constant | `PI := 3.14` |
| `>>` | Output | `>> "hello" ¶` |
| `<<` | Input | `<< "prompt: " var` |
| `<< <typespec>` | Typed/validated input (v0.0.7) | `<< ##.(5,2) "p: " v`, `<< ###(4) "n: " n`, `<< ##"(20) "s: " s`, `<< ##' "c: " c` |
| `@~` | Sleep (ms) | `@~ 500` |
| `>>!` | Clear screen | `>>!` |
| `>>?` | Query terminal size (positional tuple) | `(H, W) = >>?` |
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
| `p \|\| q` | Match or-pattern (alternatives) | `?? k { 'p' \|\| 'P' => ... }` |
| `@` | Loop (while) | `@ cond { }` |
| `@` | Loop (times) | `@ N { }` — repeats exactly N times when N is a positive Int |
| `@` | Loop (infinite) | `@ { }` |
| `@!` | Break — needs an enclosing loop | `@!` or `@:label!` |
| `@>` | Continue — needs an enclosing loop | `@>` or `@:label>` |
| `->` | Lambda | `x -> x * 2`, `(a, b) -> a + b`, `() -> 42` (thunk, v0.0.9) |
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
| ~~`arr[i] = val`~~ | **Withdrawn.** Indexed assignment does not exist — `=` gives a value to a NAME | use `arr[i]$~ val` |
| ~~`arr[i] += val`~~ | **Withdrawn** with the above | use `arr[i]$~ (arr[i] + val)` |
| `arr[i]$~ val` | Update. Result **used** → builds; result **discarded** → modifies in place | `arr[2]$~ 99` |
| `arr[i>j]$~ val` | Deep update, same rule | `m[1>2]$~ 99` |
| `d["k"]$~ val` | Dictionary update by key; **adds** the key if absent | `p["y"]$~ 42` |
| `d[k1>k2]$~ val` | Deep update by key — a step's value decides: Int is a position, String is a key | `config[k1>k2]$~ 9090` |
| `d$-["k"]` | Remove a key — `$-[…]` is "by address", and a dictionary's address is its key | `u$-["ciudad"]` |
| `d$? "k"` | Does the dictionary have this key? (`in`, as in Python/JS) | `u$? "edad"` |
| `#[…]` | Array with a **declared** mix of element types — same type as `[…]` | `#[#0, 1, "x"]` |
| `@ (k, v):x` | For-each with a destructuring pattern where a name would go | `@ (k,v):pares { }` |
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
| `##!expr` | Cast to Int (truncating); `Char` → code point | `##!3.7` → `3`, `##!'A'` → `65` |
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

