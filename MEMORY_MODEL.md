# Zymbol Memory Model — Design vs. Implementation Audit

> **Version audited: v0.0.7 — 2026-07-12. Fully resolved in v0.0.8** (branch
> `v0.0.8`): every bug finding (MM-1, MM-2, MM-3, MM-4, MM-9, and the VM findings
> MM-10, MM-11 discovered during verification) is **fixed**, and every doc gap
> (MM-5, MM-6, MM-7, MM-8) is documented in the GUIDE. The audit narrative below
> is kept as written against v0.0.7 — each affected section carries a **v0.0.8**
> note describing the new behavior. Promoted to `REFERENCE.md` as L18–L24 (all
> fixed). Regression tests: `tests/bugs/bug_mm*.zy` (TW == VM parity).
>
> Design sources: `GUIDE.md` §4 (Variables and Constants), §9 (Functions), §10/10b
> (Lambdas, Capture Semantics), §17 (Modules); `REFERENCE.md` §20 (Known Limitations).
> Implementation source: `crates/zymbol-interpreter` (tree-walker). Every claim below was
> verified by executing `.zy` test programs against the v0.0.7 binary (tree-walker and
> `--vm` where relevant).

This document maps each documented memory/scoping rule to its actual implementation,
records the mechanics, and lists the divergences found. Findings are numbered `MM-1` …
`MM-11` (tracked in `REFERENCE.md` as L18–L24).

---

## Table of Contents

1. [Physical Memory Layout](#1-physical-memory-layout)
2. [Global Memory](#2-global-memory)
3. [Block Scopes](#3-block-scopes)
4. [Function Memory](#4-function-memory)
5. [Constant Memory](#5-constant-memory)
6. [Module Memory](#6-module-memory)
7. [Memory of Functions Inside Modules](#7-memory-of-functions-inside-modules)
8. [Lambdas and Captures](#8-lambdas-and-captures)
9. [Findings Summary](#9-findings-summary)
10. [Verified Behavior Matrix](#10-verified-behavior-matrix)

---

## 1. Physical Memory Layout

The tree-walker holds all program state inside the `Interpreter` struct
(`crates/zymbol-interpreter/src/lib.rs:327`):

| Structure | Role |
|---|---|
| `scope_stack: Vec<HashMap<String, Value>>` | Lexical scopes. Index 0 = global (or function-frame bottom). One map pushed per block. |
| `mutable_vars_stack` / `const_vars_stack` | Parallel stacks tracking `~` params and `:=` constants per scope. |
| `loop_scope_depths: Vec<usize>` | Indices into `scope_stack` where `@` loop-anchor scopes start (used by `x°` / `°x`). |
| `functions: HashMap<String, Rc<FunctionDef>>` | Named function table. **Not scoped** — a definition executed anywhere registers globally. |
| `loaded_modules: HashMap<PathBuf, LoadedModule>` | One entry per module file path. Holds persistent module state. |
| `import_aliases: HashMap<String, PathBuf>` | Alias → module path. Call-scoped (saved/restored per call). |
| `dead_variables: HashSet<String>` | Names destroyed by `\` (use-after-destruction detection). **Global, not call-scoped** — see MM-3. |

Values have **value semantics**: assignment, argument passing, and capture copy the
`Value` (with fast paths that mutate in place when the interpreter can prove the target
is the same variable, e.g. `arr = arr$+ x`). There are no references or aliasing at the
language level; the only shared-identity structure is `LoadedModule`.

Performance machinery that is *semantically neutral* (verified): scope-map pooling
(B10/B13), scope elision for blocks that declare no variables (QW7 in `if_stmt.rs:16`,
QW16 in `loops.rs:36`), self-assign fast paths (B3/B12), and move-on-return (MoveOrClone).

---

## 2. Global Memory

**Design** (GUIDE §4): the top-level scope holds regular variables, constants, and
function definitions. Regular variables follow lexical scoping — visible and writable
from any inner block.

**Implementation**: `scope_stack[0]` of the root interpreter. Reads walk innermost →
outermost (`get_variable`, `lib.rs:477`); writes update the nearest scope that already
holds the name, otherwise create it in the **innermost** scope (`set_variable`,
`lib.rs:531`).

**Verified**: matches design. Two consequences worth stating explicitly (implicit in the
GUIDE, now explicit here):

- **There is no shadowing.** Assigning to an outer name from an inner block always
  mutates the outer variable; it never creates an inner copy.
- Global variables are **not** visible inside directly-called named functions
  (see §4) — global memory is only "global" for block-structured code, not across
  function-call frames. Constants are the exception (§5).

---

## 3. Block Scopes

**Design** (GUIDE §4 "Variable Scope"): a variable declared inside a block (`? {}`,
`_? {}`, `_ {}`, `@ {}`, `?? {}` arms, `!? {}`) dies when the block ends. Outer
variables are readable and writable from inner blocks. `_name` variables have exact
block scope (no access from inner, outer, or sibling blocks — semantic error). `\ var`
destroys a variable early; reassignment resurrects it.

**Implementation**:

- `execute_block` pushes/pops one scope map (`lib.rs:1138`). QW7/QW16 skip the
  push when the block's top-level statements declare no new variable — semantically
  neutral because in that case every write resolves to an outer scope anyway.
- `@` loops additionally push a persistent **loop-anchor scope** for the whole loop
  (`push_loop_scope`, `lib.rs:441`); the body gets a fresh scope per iteration only if
  it introduces new names (QW16).
- The loop iterator variable is bound with `set_variable`: it lands in the loop-anchor
  scope **unless a variable with the same name already exists outside**, in which case
  the outer variable is reused and keeps the final iteration value after the loop
  (MM-6).
- `_name` rules are enforced by `zymbol-semantic` at analysis time (verified: exact
  error `cannot access underscore variable '_x' from inner scope`).
- `\ var` removes the name from all scopes of the *current frame* and records it in
  `dead_variables` (`destroy_variable`, `lib.rs:808`). Reassignment removes the dead
  mark (resurrect — verified).

**Verified**: matches design, with two footnotes — MM-6 (iterator reuse) and MM-3
(`dead_variables` leaks across call frames, §4).

---

## 4. Function Memory

**Design** (GUIDE §9 "Function Scope", §10b "Named Functions vs Lambdas"):

- Direct call `fn(args)`: fully isolated scope; only parameters are visible; cannot
  read or write outer variables.
- Used as a value (`f = fn`): captures a by-value snapshot of the referenced outer
  variables at the point of **assignment** (asymmetric capture).

**Implementation**: a direct call runs `take_call_state` (`lib.rs:619`), which swaps out
the entire `scope_stack` / `mutable_vars_stack` / `const_vars_stack` / `import_aliases`
and installs a single fresh scope holding the parameters. `restore_call_state`
(`lib.rs:645`) puts everything back on every exit path, including errors (the v0.0.7
L16 fix). `f = fn` goes through `func_def_to_value` (`functions_lambda.rs:239`), which
collects free identifiers in the body and snapshots them from the current scope stack —
same mechanism as lambda capture.

**Verified**: isolation, snapshot-at-assignment, output-param write-back (`<~` params),
recursion, and TCO all behave as documented. **However, the isolation is not total** —
three states cross the call boundary when the design says nothing should:

- **Constants `:=` are injected into every direct-call frame** — intentional
  ("constants are globally scoped by design", `functions_lambda.rs:360-371`) but
  undocumented in the GUIDE (MM-5, doc gap) and broken beyond call depth 1 in the
  tree-walker (MM-9). See §5.
- **`loop_scope_depths` is *not* saved by `take_call_state`.** A function called from
  inside a `@` loop inherits the caller's loop-anchor indices, which point into a
  scope stack that no longer exists. Any `x°` / `°x` use inside that function
  **panics the interpreter** (`index out of bounds`, `lib.rs:456` / `lib.rs:468`).
  The VM executes the same program without crashing → engine divergence (MM-1, bug).

  ```zymbol
  f() { x° *= 7  <~ x }
  @ i:1..2 { >> f() ¶ }    // TW: panic — VM: prints 7, 7
  ```

- **`dead_variables` is not saved by `take_call_state`.** `\ x` inside a function
  marks the *name* `x` dead globally; after the call returns, the caller's own `x`
  raises `use after destruction` even though it was never destroyed (MM-3, bug).
  The VM is unaffected → engine divergence.

  ```zymbol
  kill() { x = 5  \ x  <~ 1 }
  x = 10
  r = kill()
  >> x ¶    // TW: runtime error "use after destruction" — VM: 10
  ```

> **v0.0.8**: both leaks are fixed — `loop_scope_depths` (MM-1) and
> `dead_variables` (MM-3) are saved and restored per call frame in
> `SavedCallState`. Both examples above now print the VM result in both engines.
> Regression tests: `tests/bugs/bug_mm1_hot_def_fn_scope.zy`,
> `tests/bugs/bug_mm3_destroy_frame_local.zy`.

---

## 5. Constant Memory

**Design** (GUIDE §4): `NAME := value` declares an immutable binding; reassignment is
an error. The GUIDE does not specify constant *scope*.

**Implementation**:

- Declaration stores the value like a normal variable and marks the name in the
  current scope's `const_vars_stack` entry (`variables.rs:247`). Redeclaration in any
  visible scope → runtime error `constant 'C' already declared`.
- Reassignment is blocked twice: statically by `zymbol-semantic` (also inside function
  bodies) and at runtime by `is_const` (`lib.rs:549`).
- **Scope**: a constant declared inside a block dies with the block (verified — same
  lexical rule as variables).
- **Cross-frame visibility (design intent)**: `:=` constants declared at the top level
  are meant to behave as **globals that pierce function isolation**. Three independent
  sources agree on this intent: the injection code's comment ("constants are globally
  scoped by design — nested call chains propagate constants correctly",
  `functions_lambda.rs:360-364`), the semantic analyzer (accepts top-level constants
  referenced in any function body), and the VM (resolves them at any call depth).
  This contradicts the GUIDE's "only their parameters are in scope" (MM-5 — GUIDE
  needs an explicit note). Function-local `:=` declarations are local only (the
  semantic analyzer rejects references from other functions — verified).
- **Tree-walker implementation falls one level short (MM-9, bug).** On a direct call,
  constants from the caller's saved stack are copied into the callee's fresh frame
  (`functions_lambda.rs:365-371`) — but the copies are **not re-marked** in the
  callee's `const_vars_stack`. When that callee calls another function, the injection
  loop finds no marked constants to forward, so the constant vanishes at call depth
  ≥ 2. The semantic layer accepted the program, so this surfaces as a *runtime* error
  in TW only; the VM returns the correct value:

  ```zymbol
  K := 5
  inner() { <~ K * 2 }
  outer() { <~ inner() + K }
  >> outer() ¶    // designed: 15 — VM: 15 ✅ — TW: runtime error "'K' is undefined"
  ```

  Corollary: any recursive function or helper chain that reads a global constant
  breaks in the tree-walker. Workaround: pass the constant as a parameter.
- The unmarked injected copies also mean runtime `is_const` does not protect them
  inside function bodies; immutability there rests on the semantic layer, **which does
  not run on modules loaded at runtime** — see MM-4 in §7.

> **v0.0.8**: MM-9 fixed. Root-scope `:=` constants are recorded in a
> `global_consts` table on the interpreter that is **not** swapped by
> `take_call_state`: `get_variable` falls back to it (script frames only —
> module frames keep their isolation), and `is_const` consults it, so top-level
> constants are visible and immutable at any call depth, through recursion and
> lambda frames. Block-local constants remain lexically scoped and are still
> forwarded one frame at a time — now re-marked (`mark_const`) so chains work.
> A parameter may shadow a forwarded constant (`unmark_const` at binding).
> Regression test: `tests/bugs/bug_mm9_const_call_depth.zy`.

```zymbol
PI := 3.14
area(r) { <~ r * r * PI }   // works: PI injected into the isolated frame
>> area(2) ¶                 // → 12.56  (TW and VM agree — call depth 1)
```

---

## 6. Module Memory

**Design** (GUIDE §17): a module has exported constants (`:=`, read via `alias.CONST`),
private mutable state (`=` variables, persisting across calls, reachable only through
exported functions), and functions. Initializers must be literals (E013).

**Implementation** (`modules.rs`, `LoadedModule` at `modules.rs:21`):

- At import, the module file is **lexed and parsed only** — the semantic analyzer is
  *not* run on runtime-loaded modules (`load_module`, `modules.rs:120`). E013 is
  enforced by the parser, so structural rules hold, but semantic-only checks (constant
  reassignment, `_name` rules, unused warnings) are silently skipped (root cause of
  MM-4).
- The module body executes once in a private interpreter; its final state is captured
  into a `LoadedModule`:
  - `constants` — exported `:=` values, **snapshot copied at load time**;
  - `all_variables` — every module-level binding (constants + private `=` state);
  - `functions` / `all_functions` — exported vs. complete function tables;
  - `import_aliases` — the module's own imports.
- One `LoadedModule` per resolved file path. **Two aliases to the same file share the
  same state** (verified: increments through alias `a` are visible through alias `b`).
  `std/*` modules are singletons under synthetic `__stdlib__/…` keys.
- **State persistence** works by *inject + write-back*: an `alias::fn()` call injects
  `all_variables` into the fresh frame before the body runs
  (`functions_lambda.rs:347`), and after the body finishes, every key that existed in
  `all_variables` at load time is copied back from the frame
  (`functions_lambda.rs:465-488`). Function locals/params are excluded automatically
  because they were never in `all_variables`.

**Verified**: the documented counter pattern (`c::increment()` / `c::get_value()`)
works in both engines; exported-constant reads (`alias.CONST`) work (L3 fixed);
private variables are inaccessible from outside. Divergences: MM-2 and MM-4 (§7).

---

## 7. Memory of Functions Inside Modules

**Design** (GUIDE §17): module functions see the module's constants and private state;
private helpers are callable from other functions in the same module (BUG-01 fix);
state mutations persist across calls.

**Implementation**: two distinct call paths with **different memory semantics**:

1. **External call `alias::fn(args)`** (`module_info = Some`): injects
   `all_variables`, swaps in the module's `all_functions` table and `import_aliases`,
   executes, then **writes back** module keys. State persists. ✅
2. **Intra-module call `helper(args)`** (bare name, `module_info = None`): the
   callee's frame *does* get the module's `all_variables` injected (via
   `origin_module_path`, `functions_lambda.rs:377-383`), so it can **read** module
   state — but the **write-back step is skipped** (it is gated on `module_info`,
   `functions_lambda.rs:471`). Any mutation of module state inside a helper is
   **silently lost**, and the outer exported function then writes its own stale copy
   back (MM-2, bug).

```zymbol
# counter {
    #> { get_value, bump_via_helper }
    count = 0
    do_bump()          { count = count + 10 }
    bump_via_helper()  { do_bump() }
    get_value()        { <~ count }
}
```
```zymbol
<# ./counter => c
c::bump_via_helper()
>> c::get_value() ¶    // designed: 10 — TW: 0 (mutation lost) — VM: 10 ✅
```

> **v0.0.8**: MM-2 fixed. Write-back now runs for **every** module frame
> (`module_ctx_path` covers both call paths) and is **diff-based** — only keys
> whose value changed relative to the injected snapshot are persisted, so an
> outer frame that never touched a key cannot clobber a nested call's
> write-back. Same-module nested calls inject the **caller's live values**
> instead of the stale store, and on return the caller's copies are refreshed
> with the keys just written back. Parameters named like module variables are
> excluded from write-back. The rule of thumb below is obsolete — helpers may
> mutate state freely. Regression test:
> `tests/bugs/bug_mm2_module_state_helper.zy`.

**Rule of thumb until fixed** *(obsolete since v0.0.8, kept for history)*: mutate
module state only in the function that is called directly through `alias::`;
helpers may read state and must return values instead of mutating.

Additionally, because runtime-loaded modules skip semantic analysis (§6), a module
function **can reassign a module constant** at runtime in the tree-walker. The mutation
persists in `all_variables` (visible to subsequent `alias::` calls) while the exported
`constants` snapshot keeps the original value — a split-brain state (MM-4):

```zymbol
# modconst {
    #> { MAX, get_max, set_evil }
    MAX := 100
    get_max()  { <~ MAX }
    set_evil() { MAX = 5 }
}
```
```zymbol
<# ./modconst => m
>> m.MAX ¶           // → 100
m::set_evil()        // designed: error — TW: silently succeeds
>> m::get_max() ¶    // TW: 5 — VM: 100
>> m.MAX ¶           // → 100 (stale snapshot; now inconsistent with get_max in TW)
```

**Mitigation**: `zymbol check <module>.zy` *does* run the semantic analyzer on the file
directly and catches the reassignment. Run `check` on every module file in CI.

> **v0.0.8**: MM-4 fixed. Importing a module now runs the full semantic gate
> (`VariableAnalyzer` + `TypeChecker`) in **both** engines — the tree-walker's
> `load_module` and the VM compiler's `compile_import` — with identical error
> text. The example above fails at import time. As a runtime backstop, module
> constant names (`LoadedModule.const_names`) are re-marked `const` when
> injected into module frames, so reassignment errors even if analysis is
> bypassed. Regression test: `tests/bugs/bug_mm4_module_const_guard.zy`.

---

## 8. Lambdas and Captures

**Design** (GUIDE §10/10b): capture by value at creation, only referenced variables;
writes to captured variables stay local; per-iteration snapshots in loops.

**Implementation**: `eval_lambda` (`functions_lambda.rs:19`) collects free identifiers
and snapshots them via `capture_only`. Calls run in an isolated frame (`take_call_state`)
seeded with the captures, then params. A no-capture expression lambda takes a fast path
(plain `push_scope`) — safe because expression bodies cannot contain assignments.

**Verified**: all documented capture semantics hold (snapshot, write-locality,
per-iteration capture, left-to-right argument evaluation). Constants are *not*
specially injected into lambda frames — a lambda sees a constant only by capturing it,
which works transparently since capture reads any visible scope.

---

## 9. Findings Summary

| ID | Severity | Area | Finding | Status (v0.0.8) |
|----|----------|------|---------|-----------------|
| **MM-1** | 🔴 Crash | Functions × `°` | `x°` / `°x` inside a function called from within a `@` loop panicked the tree-walker (`loop_scope_depths` not saved in `take_call_state`). | ✅ **Fixed** — anchors are frame-local (`SavedCallState`); REFERENCE L18 |
| **MM-2** | 🔴 Bug | Module functions | Module-state mutations made by intra-module (bare-name) calls were lost; only the directly-called `alias::` frame was written back. | ✅ **Fixed** — diff-based write-back on every module frame + same-module live injection + caller refresh; REFERENCE L19 |
| **MM-3** | 🔴 Bug | `\` × functions | `\ x` inside a function poisoned the caller's same-named variable (`dead_variables` was global). | ✅ **Fixed** — frame-local in `SavedCallState`; REFERENCE L20 |
| **MM-4** | 🟠 Gap | Module constants | Runtime-loaded modules skipped semantic analysis → module functions could reassign `:=` constants with split-brain state. | ✅ **Fixed** — semantic gate at import in both engines + runtime const re-marking; REFERENCE L21 |
| **MM-5** | 🟡 Doc gap | Constants × functions | Script-level `:=` constants are globally scoped by design — GUIDE §9 "only their parameters are in scope" was incomplete. | 📘 **Documented** — GUIDE §4 "Constant Scope" + §9 note |
| **MM-6** | 🟡 Doc gap | Loops | The `@ var:iterable` iterator reuses a pre-existing outer variable of the same name; otherwise it dies with the loop. | 📘 **Documented** — GUIDE §8; leftover value is engine-specific (MM-11 / L24) |
| **MM-7** | 🟡 Doc conflict | `°` × VM | GUIDE said `x°`/`°x` are "tree-walker only, add `@vm-skip`"; IMPLEMENTATION.md marks them ✅/✅; the VM executes them. | 📘 **Resolved** — stale GUIDE note replaced; both engines supported (514/514 parity, 0 skips) |
| **MM-8** | 🟢 Info | Modules | Module state identity is per file path: multiple aliases share one `LoadedModule`. Exported constants are copied at load time. | 📘 **Documented** — GUIDE §17; VM divergence tracked as MM-10 / L23 |
| **MM-9** | 🔴 Bug | Constants × nested calls | In the tree-walker, a global constant vanished at call depth ≥ 2 (injected copies were not re-marked const). | ✅ **Fixed** — root-scope constants live in a global table not swapped by frames; REFERENCE L22 |
| **MM-10** | 🟠 Bug (VM) | Modules × VM | The VM gave each import alias its own module state copy; the tree-walker shares one state per file path. | ✅ **Fixed** — compiler caches compiled modules by canonical path; aliases and diamond importers share chunks and global slots; REFERENCE L23 |
| **MM-11** | 🟡 Bug (VM) | Loops × VM | Leftover iterator value after a loop that reuses an outer variable differed: TW leaves the last executed value, VM left the first out-of-range value (body writes could also alter iteration). | ✅ **Fixed** — VM range loops advance a hidden counter published to the named iterator per iteration; REFERENCE L24 |

## 10. Verified Behavior Matrix

Programs executed against the **v0.0.7** binary (tree-walker unless noted).
Every ❌ row below is fixed in v0.0.8 — both engines now produce the designed
result (see the regression tests in `tests/bugs/bug_mm*.zy`):

| # | Behavior | Designed | Observed | Match |
|---|----------|----------|----------|-------|
| T1 | Outer var readable/writable in block; inner var dies at block end | GUIDE §4 | ✅ | ✅ |
| T2 | Using a block-local var after the block | error | semantic error | ✅ |
| T3 | `:=` constant visible inside direct-call function | *(undocumented)* | visible, TW=VM | MM-5 |
| T4 | Global `=` var invisible inside direct-call function | error | runtime error | ✅ |
| T5 | `:=` declared in block dies with block | *(undocumented)* | dies | ✅ (doc'd here) |
| T6 | Iterator var after loop (fresh name) | not accessible | semantic error | ✅ |
| T7 | Iterator var after loop (pre-declared) | *(undocumented)* | survives = 3 | MM-6 |
| T8 | Module private state across `alias::` calls | persists | 2 (TW=VM) | ✅ |
| T9 | Module state mutated by intra-module helper | persists | TW 0 / VM 10 | ❌ MM-2 |
| T10 | `x°` in function called from loop | anchors to fn scope | TW panic / VM 7,7 | ❌ MM-1 |
| T11 | `\ x` in function, caller has own `x` | caller unaffected | TW error / VM 10 | ❌ MM-3 |
| T12 | Lambda write to captured var stays local | local copy | 1 then 0 | ✅ |
| T13 | `f = fn` snapshot at assignment | 15, 15 | 15, 15 | ✅ |
| T14 | `°x` in function called from nested loops | anchors to fn scope | TW panic | ❌ MM-1 |
| T17 | `\ x` then reassign resurrects | works | 20 | ✅ |
| T18 | Reassign script constant inside function | error | semantic error (TW=VM) | ✅ |
| T19 | Module fn reassigns module constant | error | TW 100/5/100 split-brain; VM 100/100/100 | ❌ MM-4 |
| T20 | Redeclare constant | error | runtime error | ✅ |
| T21 | Loop closures capture per-iteration value | 11, 13 | 11, 13 | ✅ |
| T23 | `x°` / `°x` at top level outside loops | global anchor | 7, 3 | ✅ |
| T25 | `_name` access from inner block | semantic error | semantic error | ✅ |
| T26 | Inner assignment to outer name (no shadowing) | mutates outer | 2 | ✅ |
| T27 | Two aliases to same module share state | *(undocumented)* | shared (2) | MM-8 |
| T28 | Global `:=` read at call depth 2 (`outer` → `inner`) | 15 | TW runtime error / VM 15 | ❌ MM-9 |
| T29 | `:=` declared inside a function, read by its callee | error | semantic error (TW=VM) | ✅ |

---

*Related docs: [GUIDE.md](GUIDE.md) · [REFERENCE.md](REFERENCE.md) ·
[IMPLEMENTATION.md](IMPLEMENTATION.md) · [ARCHITECTURE.md](ARCHITECTURE.md)*
