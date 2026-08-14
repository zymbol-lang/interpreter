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

### ~~L23 — VM: each import alias gets its own module state copy~~ Fixed in v0.0.8

The VM compiler recompiled a module on every import, allocating fresh global-variable
slots per alias — so `a::increment()` was invisible through `b::get_value()`, and
diamond dependencies (two modules importing the same third module) held divergent
copies. The compiler now caches compiled modules by canonical file path: any later
import — another alias or another importer — binds to the same chunks and global
slots, matching the tree-walker's per-path state identity (GUIDE §17,
MEMORY_MODEL.md MM-10). Regression test: `tests/bugs/bug_mm10_alias_shared_state.zy`
(two aliases + diamond, TW == VM).

### ~~L24 — Leftover loop-iterator value differs between engines~~ Fixed in v0.0.8

When `@ i:1..3 { }` reuses a pre-declared outer `i` (GUIDE §8), the VM left the first
out-of-range value (`4`) while the tree-walker leaves the last executed value (`3`).
The VM's range loops now advance a hidden counter and publish it to the named
iterator at the top of each iteration — leftover value and body-write semantics
(writes to the iterator inside the body cannot alter the iteration) match the
tree-walker exactly (MEMORY_MODEL.md MM-11). Regression test:
`tests/bugs/bug_mm11_iterator_leftover.zy` (7 variants, TW == VM).

### ~~L25 — Juxtaposition did not work inside call arguments~~ Fixed in v0.0.8

Implicit concatenation existed only at statement level, so `f(a " " b)`,
`[a " " b]` and `(a " " b)` were parse errors and every composed string handed
to a function needed an intermediate variable first. It now works in call
arguments, array elements, tuple elements and grouped expressions. A comma
still separates, and a following `(` never continues the chain in those
positions (it is ambiguous with a lambda, a tuple and a grouped expression) —
GUIDE §13. Found while building zy-GO, whose side panel spent six variables on
nothing else. Regression test:
`tests/strings/30_juxtaposition_delimited.zy` (TW == VM).

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
included; only the message wording still differs. Regression: `tests/arity/` (7 cases,
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

`zymbol check` reported nothing in any of the seven cases now in `tests/loops/labels/`,
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

Regression: `tests/loops/labels/` (9 cases, all four engines agree),
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
of each pair. Regression: `tests/lambdas/29_zero_param_thunk.zy`, run through all four.

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
in the tree-walker, the register VM, `zymbol.js` and zyml. See L29 for what these four did
before, which was four different things.

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
registers and execution continued with corrupted state. See `tests/arity/` for the
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

Runtime errors carry a **kind** (e.g., `##Index`, `##Div`, `##Type`, `##Range`) and a **message** string. The value in `_err` has the format `##Kind(message)`. The `#?` type symbol of an error value is the kind itself — `(##Index, N, ...)` — there is no generic error type symbol.

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

The integer bound is the mantissa of a double, chosen so that **all four engines
represent every Zymbol integer exactly and natively**: `i64` in the tree-walker
and the register VM, OCaml's 63-bit `int` in `zyml`, a `Number` in the browser.
Nothing is boxed, nothing is a `BigInt`, and no engine is approximating — which
is why an integer means one thing across all of them.

Before v0.0.9 there was no rule at all and each engine used its host's. The same
program gave four answers:

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

`std/term` (v0.0.8) has no soft-error channel: its five functions are pure measurements
over a string and cannot fail environmentally.

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

