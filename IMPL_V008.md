# Implementation Plan — v0.0.8 Memory Model, Auto-Free, and Validation-Driven Fixes

> **Status: code and documentation complete on branch `v0.0.8`; the tag is not cut.**
> `Cargo.toml` reads `version = "0.0.8"` and the CHANGELOG entry is dated `2026-08-01`.
> `main` has not been merged and there is no `v0.0.8` tag — that is the remaining step,
> and it is the trigger for the four release workflows. Part II below is the closure
> checklist.
>
> **Documentation pass — 2026-07-29.** Every document listed in Part II § A was reconciled
> against the code, and feature #11 (`.zyp` packages) was documented across CHANGELOG /
> ARCHITECTURE / GUIDE / REFERENCE / SYMBOLS / ROADMAP / README. Three pieces of real debt
> were found while verifying and recorded in § E rather than silently fixed.
>
> **Release pass — 2026-08-01.** Three changes landed after that pass (features #12–#14
> below: numeral-mode reach, the ordering rule, and the static-tooling audit), § E.2 is
> **fixed**, and every figure in this document was re-measured. Current state:
> **936 unit tests** · **544/544 TW/VM parity** · **523/525 golden** (§ E.1) ·
> **formatter property 600 PASS / 0 FAIL, no regressions** · **benchmark gate 14/14**.
> One item of debt remains open (§ E.3) and one decision is unmade (§ E.1).
>
> **Correction, 2026-08-09.** The parity figure above is wrong and is left in place only
> because this document is the v0.0.8 record. It was measured in a working tree holding
> test files that `.gitignore` kept out of the repository; a clean clone of v0.0.8 has
> **536**, which is what the release notes said and what the `.deb` gate built from a
> clone actually ran. The other figures on this line were not affected. See the note under
> "Current status" in [README.md](README.md) for the full account and the rule that came
> out of it: re-derive the count in a fresh clone before quoting it.

Unlike v0.0.7 (a stdlib expansion designed up front), v0.0.8 is a **debt release**: its
scope was set by two sources of evidence, not by a feature wish list —

1. the design-vs-implementation audit in [MEMORY_MODEL.md](MEMORY_MODEL.md) (findings
   MM-1 … MM-9, plus MM-10/MM-11 found while verifying the fixes), and
2. three **validation projects** written in Zymbol, each of which surfaced divergences
   that no unit test had.

---

## The validation-project cycle

This is the working method that produced most of this release, and it is the part worth
keeping for v0.0.9:

```
write a real application in Zymbol
        │
        ├─► it does not compile / behaves differently under --vm / warns wrongly
        │        └─► file a finding (HLZ-xxx) in the project's HALLAZGOS file
        │
        ├─► classify: interpreter bug · language gap · doc error · not a gap
        │
        └─► fix in the interpreter + regression test in tests/ + CHANGELOG entry
```

| Project | What it is | Findings it produced |
|---------|------------|----------------------|
| [zy-GO](https://github.com/zymbol-lang/zy-GO) | Go/囲碁 engine, 13 modules across four subdirectories, TUI | HLZ-001 … HLZ-011 |
| zy-Serpiente | Snake, TUI, i18n rework against [USERAPPI18N.md](USERAPPI18N.md) | HLZ-SRP-001 |
| zyKlingonGalaxy | Space game written in pIqaD (Private Use Area identifiers) | HLZ-KL-001 |

Two properties of this cycle matter more than the individual fixes:

- **It finds silent divergences.** HLZ-008, HLZ-010 and HLZ-SRP-001 were all cases where
  one engine was correct and the other silently wrong. No error, no warning — the kind of
  defect a golden test only catches if someone already suspected it.
- **It sizes new capabilities honestly.** `std/term` exists because zy-GO carried a
  hand-maintained ~40-range East Asian width table inside the game (`表示/文字.zy`). The
  need was measured in a real program before it became a module — see the rubric in
  [IMPL_V007.md](IMPL_V007.md) § "Symbol vs module".

A finding is also allowed to end as **not a gap** (GAP-004 in zy-Serpiente) or as
**dismissed** (`do-while`, match identifier binding). Filing one is not a commitment to
implement it.

---

## Feature map

| # | Change | Type | Engines | Origin |
|---|--------|------|---------|--------|
| 1 | Auto-free — destruction at last use | Added | TW + VM | MEMORY_MODEL audit |
| 2 | `std/term` — terminal display metrics | Added | TW + VM | zy-GO |
| 3 | `##!` on `Char` → Unicode code point | Added | TW + VM | zy-GO |
| 4 | Delimited juxtaposition (HLZ-007) | Added | parser only | zy-GO |
| 5 | `ZymbolType::Number` (HLZ-002) | Fixed | semantic | zy-GO |
| 6 | Module state survives a returning function (HLZ-SRP-001) | Fixed | TW | zy-Serpiente |
| 7 | One identifier rule everywhere (HLZ-KL-001) | Fixed | lexer/semantic/LSP | zyKlingonGalaxy |
| 8 | VM module fixes (HLZ-008, HLZ-009, HLZ-010, MM-10, MM-11) | Fixed | VM | zy-GO + audit |
| 9 | Frame-local runtime state (MM-1, MM-3), module write-back (MM-2), import-time semantic gate (MM-4) | Fixed | TW + VM | audit |
| 10 | Match or-patterns `p1 \|\| p2` (alternatives in a `??` arm) | Added | TW + VM + JS mirror | direct request, zy-GO key handling |
| 11 | Zymbol Packages (`.zyp`) — `zymbol package`, `zymbol run pkg.zyp` | Added | CLI + web | distributing zy-GO as one file |
| 12 | Numeral mode reaches every string-building path, and collections | Fixed | TW + VM + JS mirror | zyKlingonGalaxy HUD + audit |
| 13 | One ordering rule for `<`/`<=`/`>`/`>=` in all three engines | Fixed | TW + VM + JS mirror | audit of #12 |
| 14 | Static-tooling audit: recursive `check`, stdlib visibility, re-export indexing order, pattern escaping | Changed + Fixed | check/LSP/formatter | LSP-vs-`check`-vs-runtime sweep |

Features #12–#14 arrived after the documentation pass, from two audits rather than from a
validation project — see § 12–§ 14. They are the reason this document has a second
verification date: the earlier figures in Part I (847 unit tests, 519/519 parity, 503/503
golden) are the counts at the commit each section describes, kept as written. The current
totals are in the status block at the top and in "Verification commands" below.

## 11. Zymbol Packages (`.zyp`)

A `.zyp` is a ZIP archive of **source**: `zyp.toml` (manifest), `zyp.json` (the same
manifest pre-serialized, so the browser never parses TOML), and the packaged `.zy` tree
under `src/`. It is unrelated to `zymbol build` / `zymbol-standalone`, which produces a
native executable — neither feature depends on the other in either direction.

Three decisions are worth keeping:

- **Strict closure, permissive packaging.** The closure walks module imports and
  `</ />` targets from the declared `[[script]]` entries; an unreachable `.zy` is never
  packaged. But anything that cannot be resolved statically — an absolute import, a
  `<\ shell \>`, a parse error — becomes a warning (`W001`–`W011`), never a hard failure,
  so `--dry-run` always yields something the author can inspect instead of an opaque
  failure partway through a large project. The single exception is a `[[script]]` that
  turns out to be a module file: that is a hard error, because a package whose entry point
  cannot run is not permissive, it is broken.
- **Ephemeral extraction, no `chdir`.** `zymbol run pkg.zyp` extracts to a temp dir and
  runs from there without changing the process's working directory. Code is disposable;
  data the script writes is not — a `std/io` write to a relative path still lands in the
  user's real cwd. Verified end-to-end.
- **`zymbol-package` depends only on `ast`/`lexer`/`parser`.** It never compiles or
  executes Zymbol code, so a future package manager or the LSP can use it without pulling
  in the interpreter, the VM, the compiler, `clap`, `tokio` or `odbc-api`.

Security: ZIP entry names and `[[script]].path` share one lexical containment rule
(`path_safety`), checked at manifest parse time, at extraction, and at write time. The
original vulnerability was a `[[script]].path` of `../../elsewhere.zy` escaping the
extraction directory and getting arbitrary source on the user's disk read and run — hence
the same check behind three doors rather than one. Decompressed size is capped at 100 MiB
per entry and in total.

The writer is deterministic (fixed 1980-01-01 timestamps, fixed entry order): the same
source tree yields a byte-identical archive, so a `.zyp` can be verified by hash. Confirmed
by building the same project twice and comparing SHA-256.

The pre-1.0 semver trap is worth stating twice: `engine = "0.0.8"` is a *caret*
requirement, which pre-1.0 matches only `0.0.8` exactly and would refuse to run on 0.0.9.
`zymbol package` always synthesizes `engine = ">=x.y.z"`, and a unit test pins that
behavior.

Web side: `web/src/zymbol/zyp.js` reads the ZIP by hand (central directory +
`DecompressionStream('deflate-raw')`), and `web/src/zymbol/module-resolver.js` provides the
path-normalizing resolver that replaced one which collapsed every import to its basename —
silently colliding same-named modules in different directories and defeating zymbol.js's
module cache and circular-import detection. Loading a `.zyp` **mounts** the whole tree and
**opens one tab** (the default `[[script]]`); mounted ≠ open is the playground's file model,
and the reason a 22-file package no longer opens 22 tabs.

---

# Part I — Shipped architecture

## 1. Auto-free: destruction at last use

Always on, both engines, invisible by design: it never changes a correct program's
behavior, it only lowers peak memory (measured: two sequential 30 MB strings peak at
~64 MB instead of ~94 MB in the tree-walker).

### 1a. The analysis — `crates/zymbol-semantic/src/last_use.rs`

Purely lexical and conservative. Two public entry points:

```rust
pub fn region_schedule(
    stmts: &[Statement],
    param_candidates: &[String],
    excluded: &HashSet<String>,
) -> HashMap<usize, Vec<String>>      // statement index → names to free after it

pub fn auto_free_exclusions(program: &Program) -> HashSet<String>
```

- A **region** is a flat statement sequence: the top-level program, or a named function
  body. Nested blocks are not regions — mentions inside them are attributed to the
  enclosing statement.
- **Mentions** are collected from the whole statement subtree: nested blocks, loop
  bodies, lambda bodies (capture is attributed to the statement holding the lambda),
  `{var}` string interpolations (scanned verbatim, mirroring the runtime resolver) and
  input prompts.
- The `Expr` walker is **exhaustive — no `_` arm**. New syntax fails compilation until
  someone decides its mention rule. Keep it that way.
- **Never freed** (`auto_free_exclusions`): constants, hot names (`x°`/`°x`),
  `_`-prefixed names, module-level bindings (they belong to the module state write-back
  protocol), output and mutable parameters, and the free variables of any named function
  that is used as a *value* somewhere — taking a function as a value snapshots its free
  variables at a point a regional analysis cannot see.
- Schedule slots are sorted, so output is stable across runs.

### 1b. Tree-walker

- Per-body schedules are stored in `FunctionDef::Zymbol { …, auto_free }`
  (`crates/zymbol-interpreter/src/lib.rs:94`) and applied by `execute_body_scheduled`.
- Destruction is **skipped while control flow is pending** — frame and loop teardown own
  those paths.
- Freed names go into a frame-local `auto_dead_variables` set. Touching one (impossible
  in a correct program) raises a distinctive `internal: use after auto-destruction`,
  including from string interpolation (`literals.rs:55`), which would otherwise silently
  print `{var}`.

### 1c. Register VM

- `crates/zymbol-compiler/src/lib.rs`: `emit_auto_free` emits `LoadUnit` on the
  variable's register after its last-use statement. Applied to the main program
  (`:427`), to each function body (`:810`) and per module context — `compile_import`
  swaps `auto_free_excluded` for the module's own exclusion set while compiling it
  (`:700`).
- **Known limitation** (carried into Part II): expression temporaries may hold a value
  until their register is overwritten, so the VM's peak-memory win is smaller than the
  tree-walker's.

The previously dead wiring (`set_destruction_schedule`, `statement_index`) was removed.
`zymbol check`'s ambiguous-lifetime warnings (the older def-use analyzer) are unchanged.

## 2. `std/term`

Follows the v0.0.7 stdlib checklist exactly — see [IMPL_V007.md](IMPL_V007.md) for the
seven steps. Concretely:

| Step | Location |
|------|----------|
| TW `register()` + native fns | `crates/zymbol-interpreter/src/stdlib/term.rs` |
| Registry arm | `crates/zymbol-interpreter/src/stdlib/mod.rs` |
| Builtin ids | `crates/zymbol-bytecode/src/lib.rs` — `TERM_WIDTH = 600` … `TERM_TRUNCATE = 604` |
| Compiler emit | `stdlib_builtin_entries("std/term")` |
| VM dispatch | `crates/zymbol-vm/src/stdlib_builtins.rs` |
| Deps | `unicode-width`, `unicode-segmentation` in workspace + interpreter + vm |
| Tests | `tests/stdlib/stdlib_term.zy` (TW == VM) + 4 unit tests over the pure helpers |

Functions: `width`, `pad_left`, `pad_right`, `center`, `truncate`. Width is measured in
**terminal columns over grapheme clusters**, not grapheme count: `"手番"$#` is `2`,
`term::width("手番")` is `4`. `truncate` never splits a wide glyph; `center` gives the
spare column to the right.

The module boundary is deliberate and worth restating before anything is added to it:
`std/term` answers a question about the **screen**. Everything that operates on a
string's **content** — split (`$/`), slice (`$[..]`), replace (`$~~`), repeat (`$*`),
join, trim — is a language symbol and never enters this module. The name `term` (not
`text`) exists to keep that line visible.

## 3. `##!` on a `Char`

`##!'A'` → `65`, `##!'あ'` → `12354`. The only direct Char→Int route (the workaround was
inverting a base literal, `0d|c|`, and stripping the prefix), and what makes characters
classifiable by range — a `Char` is otherwise neither comparable nor castable. `###` is
unchanged: a Char has no fractional part, so only the truncating cast was extended.

## 4. Delimited juxtaposition (HLZ-007)

Implicit concatenation now works in call arguments, array elements, tuple elements and
grouped expressions, with the same same-line rule as at statement level. **Parser-only
change** — `BinaryOp::Concat` already existed, so the tree-walker, compiler, VM and
formatter needed nothing.

Two points to remember when touching this code:

- In these positions a following `(` never continues the chain: it is ambiguous with a
  lambda, a tuple and a grouped expression.
- Cost of the trade: `f(a b)` with a forgotten comma now concatenates instead of raising
  a parse error.

The finding was originally filed against string interpolation (`"{t.field}"` is
rejected). Measuring zy-GO's side panel showed that limit cost nothing — the
intermediate variables there hold *calls*, not field accesses. Only juxtaposition was
load-bearing. **The measurement is the reason only one of the two walls was moved.**

## 5. `ZymbolType::Number` (HLZ-002)

"Int or Float, undetermined": accepted as an array index, compatible with Int and Float,
compatible with nothing else. It replaces the old `Numeric` → `Float` resolution, which
asserted more than was known and rejected `arr[(r - 1) * n + c]` inside a function. The
static error for passing a String to a function that adds to its parameter is preserved,
and now reads "expects Number".

## 6. Module state survives a returning function (HLZ-SRP-001)

`f() { v = "en"  <~ v }` returned `"en"` and left the module's `v` unchanged, in the
tree-walker only. Cause: the MoveOrClone optimisation in `Statement::Return` moves a
returned bare identifier out of scope (O(1) for strings and arrays); the module
write-back then found no key and read that as "this frame never touched it".

Fix: `current_output_params` became `move_guard_names`
(`crates/zymbol-interpreter/src/functions_lambda.rs:450`) and now holds **both** output
parameters and the module variables injected into the frame. Anything read again after
the return is cloned, not moved.

> Rule for future optimisations of this kind: any name that is read *after* the frame
> ends must be in `move_guard_names`. Output params were (QW13); module state was not.

## 7. One identifier rule everywhere (HLZ-KL-001)

`"{x}"` validated the name with `is_alphanumeric()`, narrower than the identifier rule
used everywhere else: kanji (category `Lo`) passed, Private Use Area glyphs (`Co`, the
pIqaD script) did not. The same narrower rule had been copied into the semantic
analyser's interpolation scan (false "unused variable") and three LSP helpers (no hover,
no completion).

Fix: `Lexer::is_ident_start` / `Lexer::is_ident_continue` are now **public and the single
definition** (`crates/zymbol-lexer/src/lib.rs`), deferred to by
`crates/zymbol-lexer/src/literals.rs:94`, the semantic analyser, and
`crates/zymbol-analyzer/src/lib.rs:1031,1345,1400`.

> Rule: never re-derive "is this an identifier character?" locally. There is one answer
> and it lives in the lexer.

## 8. VM module correctness

| Finding | Fix |
|---------|-----|
| HLZ-008 | `compile_import` now registers module functions' output-param flags, so `alias::f(x<~)` emits `SetupOutputWriteback` |
| HLZ-009 | `ArraySlice` handles `String` (it only failed when the subject was a runtime value, i.e. inside module functions) |
| HLZ-010 | `compile_interpolated_string` consults `global_consts` and `global_var_map` before falling through to literal text |
| MM-10 | Compiled modules are cached by **canonical file path** (`compiled_modules: HashMap<PathBuf, CompiledModuleExports>`, `crates/zymbol-compiler/src/lib.rs:330`), so two aliases or a diamond import share one set of global slots — matching the tree-walker's per-path state identity |
| MM-11 | Range loops no longer leave the out-of-range value in the named iterator's register |

With HLZ-008 and HLZ-009 closed, all six zy-GO suites pass under `--vm`.

## 9. Runtime state made frame-local (MM-1 … MM-4)

- **MM-1**: `loop_scope_depths` saved/restored in `SavedCallState` — `x°`/`°x` inside a
  function called from a `@` loop no longer panics.
- **MM-2**: module write-back runs for every module frame (including bare-name
  intra-module calls) and is **diff-based** — only keys whose value changed against the
  injected snapshot are persisted, so an outer frame cannot clobber a nested call's
  write-back. Parameters shadowing module variables are excluded.
- **MM-3**: `dead_variables` is frame-local — `\ x` in a callee no longer poisons the
  caller's same-named variable.
- **MM-4**: `zymbol run` applies the same semantic gate to imported modules as to the
  entry file, in **both** engines, plus defence in depth (module constants are re-marked
  `const` inside module frames).

## 10. Match or-patterns (`||`)

Unlike features 1–9, this one is not an audit finding or a validation-project HLZ —
it was requested directly against `GO/対局.zy`, whose key-handling arms only matched
the lowercase letter (`'p'`) and silently fell through to the default arm on the
uppercase one. `['p', 'P']` (list containment, already implemented) covers exactly
that one shape — a scalar against a set of literals — but has no equivalent once a
non-literal pattern is involved.

- New `Pattern::Or(Vec<Pattern>, Span)` in `crates/zymbol-ast/src/match_stmt.rs`.
  `p1 || p2 || p3` matches if any alternative matches; alternatives are tested left
  to right and the first match wins.
- `||` is recognised **only at the top level of an arm** — `parse_pattern` builds the
  chain, `parse_pattern_primary` (the old `parse_pattern` body) parses one link, and
  list elements call the primary form directly, so `[1, 2]` is never ambiguous with
  two alternatives.
- Alternatives mix any pattern kind in one arm: `1..10 || 20..30` (range),
  `< 0 || > 100` (comparison), `1 || expected || 9` (literal + ident),
  `["run", _] || ["build", _]` (structural list).
- Tree-walker: `pattern_matches` gained an `Or` arm that short-circuits on the first
  alternative returning `Some(true)` — a straight recursive reuse of the existing
  per-kind matchers.
- VM: `compile_match_expr`'s body was the one place per-pattern-kind code was welded
  to arm-body emission, which made an `Or` arm impossible to express without
  duplicating every kind. Refactored into `emit_pattern_test(pattern, r_sub, ctx) ->
  (skip_patches, to_body_patches)` — a pattern only emits its runtime test and hands
  back two placeholder lists, never the arm body. `compile_match_expr` now compiles
  the body once per case, at the label both patch lists converge on. The `Or` case
  chains `emit_pattern_test` across its alternatives: each non-last alternative's
  success (fall-through) is turned into an explicit jump to the body, and its failure
  patches to the next alternative; the last alternative's failure becomes the arm's
  own skip. No instruction set changes — this is a control-flow-graph refactor of
  the existing per-kind emitters.
- Semantic passes (`def_use`, `last_use`, `variable_analysis`, `type_check`) and the
  formatter (`p1 || p2` round-trips unchanged) each gained a one-line `Or` arm that
  recurses into the alternatives — no new logic, since an alternative is just another
  pattern.
- Mirrored in the browser interpreter (`web/src/zymbol/zymbol.js`): `parseMatchArm` now calls
  `parseMatchPattern`, which wraps the old arm-parsing body (renamed
  `parseMatchPatternPrimary`) with the same top-level-only `||` chaining;
  `matchPattern` gained an `'or'` case with the same left-to-right short-circuit.
  Header version comment updated.
- Documented in GUIDE.md §7 ("Or Patterns — Alternatives with `\|\|`", with the note
  that `['p', 'P']` and `'p' || 'P'` are equivalent for the literal-only case, and
  that `||` is the only way to mix pattern *kinds*), IMPLEMENTATION.md (EBNF split
  into `pattern` / `pattern_primary`, feature table), and REFERENCE.md (symbol
  table row).
- Verified: `cargo test` (all crates, no failures), `vm_compare.sh` 539/539 (536
  pre-existing files plus 3 new), `fmt_property.sh --baseline` no regressions (598/598
  non-skipped),
  byte-identical output against the `web/src/zymbol/zymbol.js` mirror on every example in the
  GUIDE section and in the three new test files. Regression tests:
  `tests/match/16_or_pattern_basic.zy`, `tests/match/17_or_pattern_mixed.zy`,
  `tests/match/18_or_pattern_block.zy` (each TW == VM).

## 11b. One module-path resolution rule (`ModulePath::resolve_from`)

Landed alongside feature #10 and easy to miss, because it looks like a refactor and is
actually a fixed divergence. The tree-walker, the semantic analyzer and the VM compiler
each answered "given this import and this importing file, which file is it?" separately.
They agreed on relative paths and diverged on the rest: `compile_import` ignored
`is_absolute` and `home_relative`, so `<# /abs/path => x` resolved to a *different file*
under `--vm` than under the tree-walker — silently, because both paths exist in a normal
checkout.

`ModulePath::resolve_from(&self, importer: &Path) -> PathBuf` is now the single rule and
all three call it. `zymbol-package`'s closure computation was the fourth would-be consumer
and is what forced the consolidation: it would otherwise have inherited whichever copy it
was written against, and a package's contents would have depended on which engine's rule
its author had in mind.

> Rule, same shape as § 7: never re-derive "which file does this import mean?" locally.
> There is one answer and it lives in `ModulePath`.

## 12. Numeral mode reaches every string-building path

`#d0d9#` sets the active output digit script. Only `>>`'s own per-item formatting was ever
numeral-aware, so the *same value* through a different route to the screen silently
reverted to ASCII: `#०९#` then `y = "{n}"` then `>> y` printed `0`–`9`.

The generic conversions — `Value::to_display_string()` (TW), `to_string_repr()`/`Display`
(VM) — have no interpreter context and therefore no mode to read. **They are unchanged.**
The fix is at every call site that *did* have `&self`/`&mut self` access to the mode and
was calling the context-free conversion anyway: `value_to_concat_str` (juxtaposition and
`$++`), `interpolate_string`, both `execute_output_pos` branches, and in the VM both copies
each of `ConcatStr`, `ConcatBuild`, `BuildStr`, plus `PrintAt` and the `ReadLine` prompt.

An audit of that fix then found three things it had left undone, and one thing that only
looked like a defect:

1. **Collections reverted at depth.** `>> [1, 2, 3]` printed ASCII while each element
   printed alone followed the mode — the conversion applied the mode at the top level only.
   `to_display_string_in`/`to_repr_string_in` (TW) and `to_display_in` (VM) recurse over
   arrays, tuples and named tuples. Brackets, commas and separators stay ASCII: they are
   syntax, not numbers.
2. **The digits did not come back.** `#|…|` normalized Unicode digits but `#.N|…|`,
   `#!N|…|`, `<<###`, `<<#.` and `<<#(n,d)` did not — a program could render `१२०` and
   then refuse to read it. All numeric casts normalize through a shared `ascii_digits`
   helper before parsing. Non-numeric strings are still rejected.
3. **The VM answered `0` where the tree-walker raised.** `#.1|"४२"|` was an error in one
   engine and `0` in the other; `c|…|`/`e|…|` on a non-number likewise. Both now fail with
   the tree-walker's message. (Pre-existing, exposed by the audit.)
4. **Not a defect: the mode also reaches text used as *data*.** A file name or shell
   command built by interpolation gets the active script too. That is intended — `#d0d9#`
   states how *this program* writes numbers, and validating that is the developer's
   responsibility. Documented in GUIDE.md § "Intent and Responsibility", with the one
   exception: `json::encode` keeps emitting ASCII, because a serialization format has a
   grammar of its own.

> **Performance note, worth keeping.** The first version routed *every* concatenation
> through `numeral_int`, allocating an intermediate `String` even in ASCII mode: ~8% on
> `"label" i` in a 3M-iteration loop (VM 0.34 s → 0.37 s, TW 0.750 s → 0.793 s).
> `map_ascii_digits` now takes its buffer by value and the VM's hot paths write straight
> into the destination when the mode is ASCII. Back to 0.32–0.34 s / 0.75 s. A correctness
> fix on a hot path needs a before/after measurement, not just a passing test.

## 13. One ordering rule, and no second-class digit script

`? "5" > 5` coerced and answered `#0`; `? "४२" > 5` raised *cannot compare*. Same operator,
same shape of operands, and the only difference was which script wrote the digits.

The three engines had three implementations and disagreed **even in ASCII**: `"5" > 5` was
`#0` in the tree-walker and `#1` in the VM (whose `cmp_direct` returned "greater" for every
pair outside its table); `"10" > "9"` was `#1` in the tree-walker and `#0` in the VM; and
the VM's call-frame loop held a *third* variant that answered `false` for anything but
`Int`/`Int`.

The rule, now identical in all three:

| Operands | Result |
|----------|--------|
| both are numbers (a string counts if `#\|…\|` would convert it — any of the 69 scripts) | numeric comparison |
| both are non-numeric text | lexicographic |
| a number meets text that is not a number | error, same message in every engine |

Equality is deliberately **excluded**: `==` never coerces, so `"5" == 5` and `"५" == 5` are
both `#0`. Also aligned: `'a' < 'b'` and `#0 < #1` were a VM feature and a tree-walker
error; both compare them now.

Implementation: `cmp_order`/`cmp_order_error` (VM, used by both interpreter loops), the
rewritten string arms of `compare_values` (TW), and `orderValues` in
`web/src/zymbol/zymbol.js`.

## 14. The static-tooling audit

Two engines had been audited against each other all release (`vm_compare.sh`). The tools
had not been audited against either. Running the analyzer over the workspace's ~918 `.zy`
files and diffing its diagnostics against `zymbol check` and against run-time behavior
found four divergences — repeatable via
`crates/zymbol-analyzer/examples/lsp_scan.rs`, which prints the analyzer's diagnostics for
a file list.

| Divergence | What it cost |
|---|---|
| `check` followed no imports | A module that failed to parse was invisible until run time; `check` returning clean meant nothing for a modular project. Now transitive (stdlib excluded, cycles cut), with `note: reached from <importer>`. LSP gets it via `ModuleIndex::set_module_errors` + a `module-has-errors` diagnostic on the import line |
| `index_background_module` registered imports *after* reading the export block | A re-export resolves through the file's own alias map, which did not exist yet → every i18n layer looked like it exported nothing: **33 false `export-not-found`** across four projects. Now 0 |
| `std/` has no file on disk | `math::inventada()`, `m::PI()`, `m.sin`, and a typo in a stdlib re-export all passed `check` in silence. `zymbol_common::stdlib` is the shared export table; `check_stdlib_access` is the single reader for both `check` and the LSP |
| `format_pattern` printed literals via `Display` | `'\n'` in a `??` arm came back as a raw newline and no longer lexed; the fail-closed gate refused to write, so `zymbol fmt` was unusable on TUI key-handling code. Now routed through the expression escaper — this closes § E.2 |

Two properties of this audit are worth carrying into v0.0.9:

- **The style-vs-correctness split is deliberate.** Recursive `check` reports *errors* from
  imported modules but leaves *warnings* (unused variables, ambiguous lifetimes) with the
  file named on the command line. A warning is about the code you are editing; an error in
  a dependency is about whether your program runs at all.
- **`stdlib_parity.rs` is the part that does not rot.** A hand-maintained export table would
  drift from the implementation within one release. That test fails if the table diverges
  from what the tree-walker or the VM compiler actually registers, which is what makes the
  fix durable rather than a snapshot.

> Rule: two implementations checked against each other is not coverage. `vm_compare.sh`
> compared TW to VM for the whole release and never once ran the analyzer. Every tool that
> claims to answer "is this program correct?" needs its answer diffed against the others.

---

# Part II — What remains to close v0.0.8

## A. Release closure

The code is done; the surrounding material is not. Documentation is checked per file
because the branch touched features that five documents describe.

| Item | State | Action |
|------|-------|--------|
| `REFERENCE.md` | ✅ **done 2026-07-29** — version line now reads v0.0.8; auto-free documented under memory semantics with the VM-temporaries limitation; `.zyp` error taxonomy added with the real message strings from `PackageError`; `##!` on Char was already in the symbol table | — |
| `ARCHITECTURE.md` | ✅ **done 2026-07-29** — `last_use` added to `zymbol-semantic`; auto-free and `std/term` added to the tree-walker's feature list; new `zymbol-package` crate section; crate count 18 → 19; dependency graph, CLI table and TW/VM parity table updated | — |
| `README.md` | ✅ **done 2026-07-29** — badge v0.0.7 → v0.0.8; stdlib/packages/auto-free bullets; `zymbol package` in Quick Start with the build-vs-package distinction; test figures and project layout corrected | — |
| `SYMBOLS.md` | ✅ **done 2026-07-29** — new "Symbol Changes in v0.0.8" section: `\|\|` or-patterns, `##!` extended to Char, `std/term` as a module (with the screen-vs-content boundary), and `.zyp` as having no grammar surface at all | — |
| `ROADMAP.md` | ✅ **done 2026-07-29**, figures refreshed 2026-08-01 — "VM completeness" rewritten as a statement of measured parity (544/544), both stale bullets struck through with evidence; status header now v0.0.8; `std/term` and the v0.0.8 features added to "What's Done"; Package Manager section reframed around `.zyp` as its first step; the suite table's formatter and JS-mirror rows updated | — |
| `IMPLEMENTATION.md` | ✅ **done 2026-07-29**, figures refreshed 2026-08-01 — parity figure now 544/544; auto-free and `.zyp` rows added to the coverage table | — |
| `GUIDE.md` | ✅ **done 2026-07-29** — the `.zyp` section no longer delegates to `CLAUDE.md` (which is agent instructions, not user documentation): manifest format, the `>=` semver warning, the full `W001`–`W011` table, the extraction/cwd split, engine precedence, and browser loading are all inline | — |
| `MEMORY_MODEL.md` | already covers the v0.0.8 features | No action |
| `CHANGELOG.md` | ✅ **done 2026-08-01** — dated `[0.0.8] — 2026-08-01`; new `### Changed` section (`ModulePath::resolve_from`, recursive `check`); the static-tooling audit added under Fixed; header figures re-measured; the stale `web/zymbol.js` / `web/zyp.js` / `web/test_zyp.mjs` paths and the "one tab per source file" claim corrected | — |
| `IMPL_V008.md` | ✅ **done 2026-08-01** — features #12–#14 documented in Part I; § E.2 closed; all figures re-measured | — |
| Git | `main` not merged; no tag | Merge to `main`, tag `v0.0.8` (branch naming: version only, no prefix). **The tag is the trigger** for all four release workflows (`release: published`) |
| `web/` distribution | `install.html`, `index.html` and `changelog.html` are already bumped to v0.0.8 with `pending` SHA256 — **those download links 404 until the tag exists**. `web/README.md`'s banner still claims they point at v0.0.7 | Merge `web/v0.0.8` to `main` (GitHub Pages) **only after** the release assets exist; fill the hashes in the same change; fix the README banner |
| VS Code extension | v0.1.5, README reviewed for v0.0.5. No `.zyp` file association, no `std/term` snippets, no `##!`-on-Char snippet. `\|\|` colours by accident (it is in the logical-operator rule) | Not a release blocker; do it with `bash build-extension.sh` (that script only) |
| JS mirror parity | 7 open gaps — see § E.3 | Decide port-or-declare before the next distribution refresh |
| `/usr/bin/zymbol`, `/usr/bin/zymbol-lsp` | ✅ both are symlinks into `interpreter/target/release/`, so they track the local build. The "stale system install" note from 2026-07-29 no longer applies | Run `cargo build --release` before relying on the IDE — `zymbol-lsp`'s binary predates the last two commits |

## B. Auto-free debt

Both items are **known and accepted**, not regressions. Neither blocks the release; both
should be decided explicitly before v0.0.9 rather than inherited silently.

1. **VM expression temporaries.** `emit_auto_free` clears the *named* variable's
   register; a temporary holding the same large value lives until its register is
   reused. Consequence: the VM's peak-memory win is measurably smaller than the
   tree-walker's. Fixing it means teaching the register allocator to release temporaries
   at their last read — a change to allocation, not to the analysis, and worth a
   before/after measurement on the same 30 MB benchmark used for the tree-walker.
2. **Flat regions only.** A variable created inside a nested block, loop body or lambda
   body is attributed to the enclosing statement and freed there — correct, but later
   than necessary in long loop bodies. Extending regions into blocks would need the
   analysis to model block lifetimes, which is exactly the complexity the current design
   avoids. **Recommendation: leave it, and write the reason into `last_use.rs` so the
   next reader does not rediscover it as a bug.**

## C. Language gaps still open

Two, both from the ROADMAP gaps table. Neither has been designed; both are subject to
the symbol-vs-module rubric and to the "a new symbol enters the grammar reluctantly"
rule in `SYMBOLS.md`.

| Gap | Description | Current idiom |
|-----|-------------|---------------|
| **Match multi-value arms** `[NI02]` | `1, 2 => "low"` — one arm, several values — is not parsed | `[1, 2] => "low"` (list containment) |
| **Dict / map literal** `[NI05]` | No `key: value` collection literal | Named tuples, or arrays of `(k, v)` pairs |

Already **dismissed** (2026-06-12, with the language author) and not to be reopened
without new evidence: `do-while ~>` `[NI01]` and match identifier binding `[NI03]`.

Neither open gap has been requested by a validation project yet. Per the cycle above,
**the next move is not to implement them — it is to see whether the next project needs
them.** `std/term` earned its way in that order.

## D. VM parity — the ROADMAP is out of date

Measured on this branch, both ROADMAP "VM completeness" bullets are stale:

- **"Format expressions in VM: `e|x|`, `c|x|` full parity"** — already done, and the
  syntax in that line predates v0.0.6. Verified in both engines on the current release
  binary:

  ```
  #,|x|     → 12,345.678     TW == VM
  #^|x|     → 1.2345678e4    TW == VM
  #^.3|x|   → 1.235e4        TW == VM
  #,.2|x|   → 12,345.68      TW == VM
  ```

  `compile_format` (`crates/zymbol-compiler/src/lib.rs:3515`) emits `FmtThousands` /
  `FmtScientific` with both precision kinds. **Delete the bullet.**

- **"Module system in VM: full parity"** — HLZ-008, HLZ-009, HLZ-010, MM-10 and MM-11
  closed the known divergences, and 544/544 parity tests pass with **0 skipped**. (The
  `@vm-skip` on `tests/gaps/gap_key_input_type_check.zy` is gone from the count: it is a
  `zymbol check` test that never executes.) **Rewrite the bullet as a statement of
  current parity**, and if any doubt remains, name the specific construct still missing
  rather than leaving an open-ended claim. *(Done — ROADMAP.md updated 2026-07-29; the
  figure there needs the 541 → 544 bump made on 2026-08-01.)*

The general point: an unqualified "not at parity" line in the ROADMAP is worse than no
line, because it sends projects to the tree-walker by default — which is what zy-GO did
before HLZ-008 was found.

---

## Verification commands

Measured 2026-08-01 on `v0.0.8` @ `85eedf9`:

```bash
cargo test --workspace                            # 936 passed, 0 failed
bash tests/scripts/vm_compare.sh                  # TW == VM parity: 544/544, 0 skip
bash tests/scripts/expected_compare.sh            # golden: 523/525 — see § E.1
bash tests/scripts/fmt_property.sh --baseline tests/scripts/fmt_property_baseline.txt
                                                  # 643 files: 600 PASS / 43 SKIP / 0 FAIL
bash tests/scripts/run_all.sh --vm --runs 3       # benchmark gate 14/14, no regressions
```

From `web/` (plain Node, no `package.json`):

```bash
node tests/test_runner.mjs                        # CLI vs JS engine: 516/521, 39 skipped
node tests/test_runner.mjs --dir examples         # example pool: 208/210
node tests/test_catalog.mjs --check               # 208 entries / 219 files, no orphans
node tests/test_zyp.mjs                           # .zyp reader + resolver: all pass
node tests/test_filestore.mjs                     # playground file model: all pass
```

A finding is not closed until it has a regression test that **fails on the previous
binary**. Every HLZ and MM entry in the v0.0.8 CHANGELOG names its test.

---

## E. Debt found during the documentation pass (2026-07-29)

All three were found by running the full verification suite while reconciling the docs, and
none was fixed in that pass: writing documentation is not the moment to change behavior.
E.2 has since been fixed on its own commit with its own regression test (§ 14). E.1 is a
decision nobody has made yet, and E.3 has grown from five gaps to seven.

**Status at 2026-08-01:** E.1 open (decision) · E.2 **closed** · E.3 open (7 gaps).

### E.1 — Two `.expected` fixtures are stale, and the suite reports them as failures

`bash tests/scripts/expected_compare.sh` reports **523/525**, failing on:

- `tests/errors/parser/parent_path_alias.zy`
- `tests/memory02_function_isolation.zy`

**Not an interpreter regression.** Both `.expected` files are byte-identical to `HEAD` and
the actual output is byte-identical to what they contain. The mismatch is in the harness:
`run_file()` pipes output through `strip_warnings`, which drops blank lines and lines
starting with `warning:`, with an arrow-prefixed location, with an `=` help marker, or with
three spaces — but the `.expected` file is compared *unfiltered*. These two fixtures were hand-written with exactly those lines
(a `warning: unused variable` block, and blank lines between stacked parser errors), so
they can never match filtered output.

Regular error fixtures survive this because their diagnostic lines carry ANSI colour codes
before the first visible character, so the leading-whitespace patterns never match them.
The warning in `memory02` is emitted uncoloured, so it is stripped.

Two possible fixes, both cheap, and the choice is a judgment call rather than a bug:
regenerate the two files with `--regen` (accepts the filter as the contract), or stop
filtering the actual output and compare both sides raw (makes warnings part of the golden
contract, which is arguably what these two fixtures were trying to express). **Still not
decided.** It does not block the release — the suite's two failures are understood and
reproducible — but it should not be inherited by v0.0.9 as an unexplained `523/525`.

### E.2 — ✅ FIXED (2026-07-29, commit `c4f610d`) — formatter escaping in match patterns

> Kept for the record, because the diagnosis is the useful part. The fix is § 14's fourth
> row: `format_pattern` now routes literals through the same escaper as `format_literal`.
> `tests/bugs/bug_char_escape_lexing.zy` is in the property corpus and `fmt_property.sh`
> reports **0 failures**, where it reported this one before.

At the time of writing, `bash tests/scripts/fmt_property.sh` reported one **new** P1 failure
against the baseline: `tests/bugs/bug_char_escape_lexing.zy` (a file added by this branch).

```console
$ zymbol fmt tests/bugs/bug_char_escape_lexing.zy
Error: failed to format file: tests/bugs/bug_char_escape_lexing.zy
Caused by:
    safety gate: formatted output no longer lexes: expected closing ' for char literal
```

**Cause**: `format_pattern`'s `Pattern::Literal` arm writes `lit.to_string()` —
`Literal`'s `Display`, which does not escape — while `format_literal` (the *expression*
path) correctly routes through `escape_char`/`escape_string`. So `'\n'` in an **expression**
formats fine, and `'\n'` in a **match pattern** is emitted as a raw newline between two
quotes, which no longer lexes.

```rust
// crates/zymbol-formatter/src/visitor.rs — format_pattern
Pattern::Literal(lit, _) => {
    self.output.write(&lit.to_string());   // ← no escaping
}
```

The fail-closed safety gate is doing its job: the formatter refuses to write output it
cannot re-lex, so no file is corrupted. The consequence is that `zymbol fmt` is unusable on
any file containing `'\n'`, `'\t'`, `'\r'`, `'\0'` or `'\\'` in a match arm — which is
exactly what key-handling code in a TUI program looks like.

Affects string patterns too: `?? s { "a\nb" => ... }` takes the same unescaped path.

**Fix shape**: route `Pattern::Literal` through the same escaping used by
`format_literal` rather than through `Display`. Needs a regression test that formats a file
with an escaped char pattern and reparses it — the property suite already catches it, so
adding the fixture to the corpus is enough once the fix lands. *(Done exactly this way.)*

### E.3 — Seven divergences between the JS mirror and the Rust engines

Re-measured 2026-08-01 from `web/`. Two suites, and each finds gaps the other cannot:

- `node tests/test_runner.mjs` → **516/521**, 39 skipped (irreducible in a browser:
  BashExec, ANSI/TUI, `std/db`, step limits). 5 failures.
- `node tests/test_runner.mjs --dir examples` → **208/210**. 2 further failures, in the
  example pool only. This is the argument for the pool being real files on disk: neither
  of them is reachable by `interpreter/tests/`.

| Test | Missing in `web/src/zymbol/zymbol.js` | Direction |
|------|---------------------------------------|-----------|
| `bugs/bug_mm11_iterator_leftover.zy` | MM-11 — leftover loop-iterator value; JS prints `0` where both Rust engines print the iterated values | JS **permissive** |
| `bugs/bug_mm4_module_const_guard.zy` | MM-4 — import-time semantic gate; the mirror runs the module instead of reporting `cannot reassign constant 'MAX'` | JS **permissive** |
| `bugs/bug_mm9_const_call_depth.zy` | MM-9 — root-scope constants at call depth ≥ 2; JS raises `'K' is undefined` | JS stricter |
| `errors/parser/parent_path_alias.zy` | HLZ-005 — the `'./../' is not a module path` diagnostic; the mirror rejects the file, but with different text and one error instead of three | text only |
| `modules_scope/interp_global_const.zy` | Interpolation of a global constant — `"{DIR}"` printed verbatim | JS wrong |
| `examples/rosetta-stone/klingon.zy` | **HLZ-KL-001 is not ported.** The JS lexer does not accept `'` in identifiers, so `f(mI') { … }` — ordinary tlhIngan Hol — fails to parse: `Expected RPAREN, got 'EOF'`. Reduced case: `f(mI') { <~ mI' }` runs in the CLI and throws in the browser | JS wrong |
| `examples/projects/math-es/calculadora.zy` | **Float literal precision, and this one predates v0.0.8.** The lexer accumulates digit by digit (`const f = value + frac / div`, `zymbol.js:501`), so `3.14159265` becomes `3.1415926499999998`. Affects *every* float literal, not just this example: `>> 3.14159265 ¶` already diverges. Introduced when digit-script support was added to the lexer in v0.0.4 | JS wrong |

Two things to take from the direction column. MM-4 and MM-11 are the worse failure mode:
the mirror is *permissive* where Rust is correct, so a playground user gets output where
the CLI would have refused. And the last row is a reminder that "parity with the Rust
engines" was never measured on plain float literals until the example pool existed —
five releases of a language whose landing page runs in the browser.

`node tests/test_zyp.mjs`, `node tests/test_filestore.mjs` and
`node tests/test_catalog.mjs --check` all pass in full.
