# Implementation Plan — v0.0.9: the collections decided, three engines instead of four, and Windows

> **Status: in development on branch `v0.0.9`.** `Cargo.toml` reads `0.0.9`; the
> CHANGELOG section is dated `unreleased`. Part I is what has landed, Part II is what
> remains before the tag.
>
> **Every figure in this document was measured on 2026-09-07** against the working tree
> and the installed `zymbol 0.0.9`, not carried forward from v0.0.8. The gate reports
> **`all gates pass`**. The commands and their output are in "Verification" below.
>
> **Windows is not repeated here.** Eleven findings, fixed and verified on Windows 11,
> have their own record in [WINDOWS_V009.md](WINDOWS_V009.md), which is written as the
> account of what running on Windows found rather than as a plan. § 12 says only what
> this document needs to say about it: why it is inside v0.0.9 and not a 0.0.8.1.

---

## What kind of release this is

v0.0.7 was a stdlib expansion designed up front. v0.0.8 was a **debt release**, scoped by
an audit and three validation projects. v0.0.9 is a **decision release**.

Most of its content is not a new capability. It is a rule that existed in the prose and
in no engine, or existed in three engines in three different ways, now decided once and
enforced in all three. The chained index `m[i][j]` had been "deprecated" since v0.0.4 and
refused nowhere. The dictionary and the positional tuple shared one bracket and were told
apart by a colon. `@ []` ran zero times in the tree-walker, forever in the VM, and raised
in the fourth engine. A `$` whose result was discarded modified in half the family and did
nothing at all in the other half, silently.

None of those were bugs anybody had filed. They were **decisions nobody had made**, and
what makes them one release rather than a scattering of fixes is that they all come from
the same act: writing down what the collection model actually is
([COLLECTIONS.md](COLLECTIONS.md), new in this release) and then measuring every engine
against it.

Three sources of evidence set the scope, in the same way v0.0.8's two did:

1. **Two new LDV projects** — चतुरङ्गम् (Chaturanga, the seventh) and ZyBank (the eighth),
   which between them produced **38 recorded findings**.
2. **A user's report from Windows 11**, which began as a hotfix branch and grew from five
   findings to eleven.
3. **A systematic sweep of the edit model**, operator by operator against every receiver
   shape — the only one of the three that was not driven by someone hitting a wall.

---

## The method changed too, and that is part of the release

Three structural changes landed alongside the language work. Each is the kind of thing
that is invisible in a feature list and decides what the next release can measure.

**The corpus left the interpreter.** QA now lives in `zyquality/`, a repository that
depends on neither engine while both depend on it. Before, `interpreter/tests/` and the
browser runner's corpus had drifted **28 files apart**, and the missing ones were exactly
`arity/` and `loops/labels/` — the files added *because* the engines disagreed. Five
incompatible exclusion mechanisms (a `@vm-skip` marker, a `VM_COMPARE_EXCLUDE` regex, a
40-entry `SKIP_SET` literal inside the runner, a `grep -L lib_time`, and nothing at all)
became one file, `corpus.toml`, where **an exclusion requires a written reason** — one
without is indistinguishable from a bug somebody hid.

**The fourth engine was retired.** `zyml`, the OCaml closure-compiling engine, was dropped
on 2026-08-17: it could not run 131 of the 599 corpus files and diverged on 15 more, while
the two Rust engines diverged on zero, and the register VM beat it 3.4× at 19×19 go even
after copy-on-write. Dropping it also closed the two items it was the last holdout on.
Historical mentions of it stay in the documents deliberately — `corpus/arity/` and
`corpus/loops/labels/` exist because of that engine, and a rule whose reason has been
deleted is indistinguishable from one nobody can justify.

**The method got a name and a document.** [LDV.md](LDV.md) — Language-Driven Validation —
states what an LDV project is, what counts as a failure, and indexes the eight gap logs
(~5,800 lines of recorded findings). Two things in it are corrections of earlier practice
rather than restatements: the applications are **not** retired after their release (seven
of eight are registered in `zyquality/project/apps.toml` and run as a gate), and the
document says plainly that the applications, like the interpreter, are written with AI
assistance under the author's direction, and what that changes about the method.

| Project | Position | Log | Findings |
|---------|----------|-----|----------|
| [चतुरङ्गम् (Chaturanga)](https://github.com/zymbol-lang/zyChaturanga) | seventh | `HALLAZGOS_ES.md` | 4 `HLZ-CHA` + 1 `IDEA-CHA` — the first log scoped by project from its first entry |
| [ZyBank](https://github.com/zymbol-lang/ZyBank) | eighth | `HALLAZGOS.md` | 12 `BUG` + 13 `GAP` + 6 `ERROR` + 2 `IDEA` = **33** — the first to adopt the canonical form entire |

ZyBank is where most of this release comes from, and the reason is worth stating: it is a
double-entry accounting program, so it is the first LDV project whose **domain has an
external authority**. A go engine is right when it plays go; a ledger is right when it
balances, and the vocabulary is not the programmer's to choose (`DINERO.md` records the
doctrine that came out of it). It is also the project that forced the dictionary, by
needing an empty one.

---

## Feature map

| # | Change | Type | Engines | Origin |
|---|--------|------|---------|--------|
| 1 | The rule of the result: a `$` whose result is used builds, discarded modifies | Decided + enforced | TW + VM + JS | edit-model sweep |
| 2 | Decision 19: discarding a *consulting* `$` is dead code and warns | Added | TW + VM + JS | edit-model sweep |
| 3 | `#(…)` — the dictionary gets a notation of its own; bare `(a: 1)` refused | Changed | TW + VM + JS | ZyBank (GAP-ZYB-003/004) |
| 4 | `##(` `##[` `##)` `##]` — `#?` tells the four collections apart | Added | TW + VM + JS | edit-model sweep |
| 5 | `#[…]` — an array whose mix of element types is declared | Added | TW + VM + JS | COLLECTIONS sweep |
| 6 | The chained index `m[i][j]` refused for reading as well as writing | Changed | TW + VM + JS | COLLECTIONS sweep |
| 7 | The dot writes what the dot reads; a deep write has one form | Fixed | TW + VM + JS | edit-model sweep |
| 8 | `##_` — the Unit literal; `==` stops constraining a parameter | Added | TW + VM + JS | ZyBank |
| 9 | `Int` is a safe integer ±(2⁵³−1), fail-closed | Changed | all engines | numeric-model decision |
| 10 | `std/time` — the clock and the civil calendar | Added | TW + VM + JS | ZyBank |
| 11 | `#\|c\|` reads a digit in any of the 69 scripts; `#,`/`#^` write in the active one | Fixed | TW + VM + JS | ZyBank (69 written, none read) |
| 12 | A named function captures the file's variables, like a lambda | Changed | TW + VM + JS | edit-model sweep |
| 13 | A top-level `<~` is the program's exit status | Fixed | TW + VM + JS | divergence nobody had reported |
| 14 | `@ <expr>` takes a count or a condition — no truthiness | Changed | all engines | four-engine disagreement |
| 15 | A module declares what it exports (`#>` required, E014) | Changed | TW + VM + JS | L48 |
| 16 | A module can hold a collection; a call no longer copies module state | Fixed | TW + VM | ZyBank |
| 17 | The formatter moves things and changes nothing | Fixed | formatter | ZyFmtCheck over the LDV apps |
| 18 | One diagnostic family across three engines | Changed | TW + VM + JS | messages inventory |
| 19 | The tree-walker copies an aggregate when it is written, not when it is passed | Fixed | TW | HLZ-012/HLZ-014 |
| 20 | Windows: eleven findings | Fixed | runtime, LSP, tests | user report |

---

# Part I — Shipped

## 1. The collections decided — [COLLECTIONS.md](COLLECTIONS.md)

The point of record for the three collections, and the document the rest of Part I
depends on. Three rules it states that had not been stated anywhere:

- **The rule of the result.** A `$` edit whose result is **used** builds and leaves the
  original alone; one whose result is **discarded** modifies in place. `b = a[2]$~ 99`
  and `a[2]$~ 99` are the same operator answering the same question differently, and the
  answer is what the surrounding code does with it.
- **`=` never writes into a collection.** `arr[i] = v` does not exist, in any collection,
  and neither does `m[i][j] = v`. There is one update form, `$~`.
- **`[…]` is homogeneous and checked; `#[…]` declares a mix and is not.** They are the
  same type — `[1, 2] == #[1, 2]` — and the mark is a statement about intent, not about
  representation.

## 2. The edit model, operator by operator

The sweep that produced most of this release. Every editing operator against every
receiver shape — bare name, dot, bracket, navigator, and mixed — is
`corpus/collections/edicion_modelo_completo.zy`, and all three engines agree on every
line. What it found:

**Decision 19 was documented and enforced nowhere.** COLLECTIONS § 1 splits the `$` family
in two: the editing half modifies when its result is discarded, and the consulting half
*always* builds — so discarding one is dead code. `s$~~["a":"X"]` as a statement ran,
changed nothing, and no engine said a word. Ten operators now warn, one warning each,
identical wording in all three engines: `$#` `$?` `$??` `$[..]` `$>` `$|` `$<` `$/` `$*`
`$~~`. A list rather than "not an edit", so a new operator has to be classified
deliberately instead of falling into a default. `$!`/`$!!` are excluded: propagating an
error is an effect.

**Two false type warnings, both on the documented in-place form.** `arr$^+` warned
`'arr' was [Int] but assigned [?]` — sorting reorders, it does not retype, and the
inference answered `Array(Unknown)`. `b$++ 7 8` answered `Array(Any)` one line after
checking that every item fits the base, contradicting its own check. Both in the two Rust
engines only, so they were live divergences as well as false.

**A silent data-destruction bug across the whole `$` family**, found while closing the
above: a write reached the wrong place when the receiver's path was spelled one way rather
than another. The fix is stated as a rule — *a write reaches its receiver's path, however
the path is spelled* — because that is the shape that generalises.

## 3. `#(…)` — the dictionary has a notation of its own

`(1, 2)` and `(a: 1)` shared the parentheses and not the semantics, and the colon was the
whole of what told them apart. COLLECTIONS.md had accepted that deliberately: the
alternative was a notation of its own, and `{}` is the block delimiter of the entire
language.

**The empty one forced it.** `()` would have to be both the empty tuple and the empty
dictionary, and they are not the same value — one takes `d["k"]$~ v`, the other answers
*tuples are immutable*. The empty dictionary was reachable (take the only key out of
`(a: 1)` and `$#` is 0) and unwritable, so every program that built one at run time
started it with an invented key and removed it afterwards.

`#` is the meta/type mark, the same one `#[…]` uses. **Both spellings would have been
worse than either**, so the bare form is refused: 276 literals were migrated across the
corpus, the applications and the examples. `#()` is the empty dictionary; keys may now be
strings, which the bracket always accepted and the literal never could.

The vocabulary follows the notation: a tuple is immutable by definition, so "named tuple"
stopped being a defensible name the moment the thing could change. It is a dictionary
everywhere now.

## 4. `#?` tells the four collections apart

Before, an array and a dictionary answered alike, so a program could not ask what it was
holding without taking the value apart. Measured in both engines:

```
[1,2,3]#?        → (##], 3, [1, 2, 3])
#[1,"x"]#?       → (##[, 2, [1, x])
(1,"x")#?        → (##), 2, (1, x))
#(k: 1, j: 2)#?  → (##(, 2, #(k: 1, j: 2))
```

`##(` is the dictionary and `##()` — with the closing paren — is a named function. The
collision is real and is the reason the symbol table names both on the same line.

## 5. The chained index is withdrawn, for reading too

`m[i][j]` was deprecated in v0.0.4 and refused nowhere. In between it parsed and ran, and
GUIDE.md said a warning "may be added in a future version" — so the rule existed in the
prose and in no engine, which left the language with two spellings of one access and
nothing to tell them apart. The chained *write* had already been withdrawn; the read was
the half that had been left.

**The rule governs how an element is addressed, not what is done with it**, so the chain
is refused as soon as it is read, whatever follows: read, read into a name, edit, build,
any other `$`. One rule, not five. And **a refusal stops cascading** — one bad line used
to report 22 errors, because parser recovery advanced by a single token.

What stays legal is a bracket after something that is not itself an access:
`[1,2,3][2]`, `f()[2]`, and indexing what an extraction built — an extraction does not
descend into `m`, it builds a new collection that was nowhere in it. The distinction is
covered as a product in the `addressing` suite: **24 cells, 24 hold** — every chained
receiver refused by every engine, every navigator receiver accepted and agreed.

## 6. `##_` — the Unit literal

Unit was the only type whose value could not be written: reachable everywhere (a function
without `<~`, `json::decode("null")`, a `NULL` column out of `std/db`) and unspellable, so
asking meant taking the type reflection apart. **The workaround was wrong**, and wrong in
a way the literal made visible: `#?`'s second field is 0 for *four* values — Unit, `""`,
`[]` and `#()` — so a predicate written on the count answers yes to all four, and an empty
text column is an everyday thing.

`##_` is not a new mark: it was already Unit's type symbol and already the "any kind" mark
in `:! ##_`, and both are the reading `_` has throughout the language. Alongside it,
**`==` no longer constrains a parameter's type** — a comparison is not a declaration —
and **`==` on a function is identity**: a function equals only itself, so two names for
one function agree and two functions with the same body do not.

## 7. `Int` is a safe integer

±(2⁵³−1), fail-closed in every engine: overflow is a catchable `##Range` error, never a
wrap and never a promotion to Float. The bound is the mantissa of a double, not the width
of any one implementation — which is what makes it *one* rule: every engine holds this
range exactly and natively, so an integer means the same thing in all of them. A wider
`Int` would have to be approximated somewhere, and an approximation that only shows up
past 2⁵³ is the kind of disagreement nobody finds until it matters.

Negation is total: the range is symmetric, so `-x` is the one integer operation that can
never overflow. The cost is stated rather than hidden — nanoseconds since the epoch no
longer fit, which is why `std/time` counts in milliseconds.

## 8. `std/time` — the clock and the civil calendar

Seven native functions in three engines, following the v0.0.7 stdlib checklist. An instant
is **milliseconds** since the epoch, always UTC; a date is a *reading* of one, so every
function takes an optional trailing zone (`"UTC"`, `"local"`, `"+1000"`).

The design decision worth keeping: **below a day it is duration, from a day up it is
calendar.** A minute is always 60 000 ms; a month lands on the same day of the month
(clamped — 31 Jan + 1 month = 28 Feb) and a day across a daylight-saving change is still a
day. `format` renders POSIX codes in **ASCII digits whatever the numeral mode**, because a
timestamp is a serialization format with a grammar of its own — a localized date is built
from `parts` instead. A date that does not exist is a soft `##Time`, not a crash.

## 9. The numeral mode reads as well as writes

ZyBank found the asymmetry: **69 scripts written, none read.** `#|c|` now reads a digit in
any of them, and `#,`/`#^` write their digits in the active script rather than reverting to
ASCII.

**The separators follow the script too, and the pair never inverts.** `,` groups and `.`
divides, in every script — the writing system chooses the *glyph*, never the roles. That is
a language decision and not a locale one, and stating it was what stopped a plausible
"European format" feature from being invented.

## 10. A named function captures the file's variables

A named function now reads a file-level name at **call** time, by value, and a write inside
stays inside the call. A lambda captures the same names at **creation**. Neither sees
another frame's locals.

**This was asymmetric before**, and the asymmetry cost more than correctness: knowing what
a body actually reads is also what makes the frame cheap to build, and the change recovered
**44% of `bench_recursion`**.

## 11. The formatter moves things and changes nothing

`ZyFmtCheck` runs over the LDV applications by default, because a formatter's damage shows
up in hand-aligned tables, five writing systems and modules importing each other, and not
in a corpus of short files. It found what the token gate could not see:

- **`zymbol fmt` printed a literal as its value renders, not as it was written.** Eleven of
  sixteen literal forms did not survive a format. A real byte offset had to come first.
- Four more things the formatter had no business changing, and two files it refused
  outright (`#[…]` and `@ (k, v):x` — the safety gate doing its job on syntax the formatter
  had not been taught).

**Why four properties and a safety gate all missed it** is the useful part: the gate
compares what it can re-lex, and a literal that re-lexes to a *different value* passes every
check that asks whether the output parses.

## 12. Windows — eleven findings

See [WINDOWS_V009.md](WINDOWS_V009.md). Only the framing belongs here: this began as a
hotfix branch off v0.0.8 and there is **no 0.0.8.1**. Eleven findings is not a patch on top
of a release, it is the substance of the next one.

The count went from five to eleven once the work could actually be run on Windows, and
**every one of the six new ones was a POSIX assumption Linux could never have surfaced** —
three of them inside the test suite itself, which is why the suite reported a healthy build
right up to the moment a user tried to run it.

## 13. One diagnostic family across three engines

The `messages` suite is an inventory: every message one engine defines and another does
not, against a recorded baseline. Unifying the collection messages found real defects
underneath — a bad string literal reported as three diagnostics instead of one (DM-10),
diagnostics coming out in `HashMap` order so they differed run to run, and `zymbol run`
warning less than `zymbol check` because the def-use pass ran only in the latter.

Two rules came out of it: **diagnostics stop naming the engine** (`VM compile error:` is now
`error:`), and a diagnostic names **types, not values**.

## 14. A module declares what it exports

L48. A module with no export block meant three different things across the engines. It is
now **E014**, and `#> { }` is how a module says it exports nothing — an empty block is a
statement, an absent one was an accident.

Alongside it: a module can hold a collection, and an intra-module call no longer copies the
whole module's state.

---

# Part II — What remains to close v0.0.9

## A. Release closure

Verified in this pass (2026-09-07) unless the row says otherwise.

| Item | State |
|------|-------|
| `COLLECTIONS.md` | ✅ new in this release, and the point of record the rest depends on |
| `LDV.md` | ✅ new — the method, the decalogue, the index of the eight gap logs |
| `LLM.md` | ✅ reviewed 2026-09-07 — five false claims corrected (§ E.3) |
| `GUIDE.md` | ✅ reviewed 2026-09-07 — 19 items; `guide_verify` back to 115/115 |
| `WINDOWS_V009.md` | ✅ rewritten on Windows as the record of what was found |
| `README.md` | ✅ reconciled 2026-09-07 — the broken example fixed and verified, every figure re-measured, ZyBank's log corrected from 25 to 33 |
| `ROADMAP.md` | ✅ reconciled 2026-09-07 — status header, the four coverage tables, the dictionary gap row, and the performance targets |
| `REFERENCE.md` | ✅ reconciled 2026-09-07 — `#(a: 1)` as the record literal, `##Key`, the symbol-table row |
| `IMPLEMENTATION.md` + `zymbol-lang.ebnf` | ✅ reconciled 2026-09-07 in two passes — figures and vocabulary, then a **rule-by-rule verification against the binary** (§ E.5). Grammar now at 3.3.0 |
| `ARCHITECTURE.md` | ✅ parity paragraph re-measured 2026-09-07. Its `NamedTuple`/`MakeNamedTuple` mentions are Rust identifiers that still exist in the code, and are left alone |
| `MEMORY_MODEL.md` | ✅ v0.0.9 measurement added beside the v0.0.8 one |
| `LDV.md` | ✅ the two undated corpus counts now say which day they are |
| `SYMBOLS.md` | ✅ no withdrawn syntax found |
| `I18N.md`, `USERAPPI18N.md` | ✅ no withdrawn syntax found; examples not re-executed in this pass |
| `CHANGELOG.md` | 922 lines under `[0.0.9] — unreleased`, 57 entries. Needs a date when the tag is cut |
| Git | Branch `v0.0.9`, not merged. Follow `agents/release_merge.md`: merge **before** publishing — the release event reads the workflows from the default branch |
| Windows `.msi` | Signing is manual; a release from this branch needs that step by hand |
| VS Code extension | Not audited in this pass. The v0.0.8 surface gaps stood then and are not known to have closed |

## B. Inherited debt — auto-free

Both items moved to [ROADMAP.md](ROADMAP.md) § Known Gaps on 2026-08-31 and **are still
undecided**. IMPL_V008 § B set itself the deadline "decided explicitly before v0.0.9"; that
deadline has now passed twice. Neither is a regression and neither blocks anything:

1. **VM expression temporaries** — `emit_auto_free` clears the named variable's register; a
   temporary holding the same large value lives until its register is reused.
2. **Flat regions only** — a variable created inside a nested block is freed at the
   enclosing statement. The standing recommendation is to leave it and write the reason
   into `last_use.rs`.

## C. Language gaps

`do-while ~>` and match identifier binding remain **dismissed** (2026-06-12, with the
language author) and are not to be reopened without new evidence. Match multi-value arms
are answered by or-patterns. The dict/map literal is answered by § 3 — though the ROADMAP
row that says so is now wrong about *how* (§ E.4).

## D. Live divergences the gate does not see

The gate is not blind to either of the shapes below — `verdict_of`
(`zyquality/src/engine.ml`) takes a non-zero exit as its primary signal and a diagnostic
prefix on stderr as its secondary one, precisely so that "one engine refuses what the
others run" is not mistaken for agreement. **It simply has no file with these shapes.**
That is the denominator problem, not a hole in the comparison: a suite reports on the
corpus it has, and a form nobody wrote produces no failing test. See § E.1 and § E.2.

---

## Verification

Measured 2026-09-07 on branch `v0.0.9`, against the installed `zymbol 0.0.9`.

```bash
cargo test --workspace              # 1026 passed, 0 failed, 4 ignored
cd ../zyquality && ./zyq suite      # all gates pass
```

| Suite | Result |
|-------|--------|
| `zyq consensus` | 666 files: **660 agree, 0 diverge**, 6 with too few engines |
| `zyq expect` | 639 goldens via `run`: 635 match, **0 stale**, 4 unchecked · 25 via `check`: 25 match |
| `zyq reject` | 41 forms: **41 refused everywhere**, 0 accepted somewhere |
| `zyq audit` | 666 `.zy` · 664 with goldens · 6 excused for every engine · **no hygiene problems** |
| `zyq selftest` | 51 checks, all passed |
| `fmt` | 764 files: 710 PASS, **0 FAIL**, 55 SKIP · P1–P4 all zero |
| `fmtcheck` | 6 LDV applications formatted, **suite unchanged** in every one |
| `addressing` | 24 cells: **24 hold**, 0 do not |
| `messages` | shared surface 706 · nothing new against the 759/1140 baseline |
| `bench` | 16 PASS, 0 FAIL, **no performance regressions** |
| `lsp` | 666 files · 16 disagreements, **0 outside the baseline** |
| `project` | 7 LDV applications, **0 diverge** in every one |
| `guide` | 115 annotated examples, **115 PASS** |
| web example pool | 332 files: 216 agree, **0 diverge** |

> Two figures need re-deriving in a fresh clone before they are quoted in release notes.
> The v0.0.8 rule stands (README § Current status): a count measured in a working tree that
> holds files `.gitignore` keeps out of the repository is not the count a clone will get.

A finding is not closed until it has a regression test that **fails on the previous
binary**.

---

## E. Debt found during the documentation pass (2026-09-07)

Same pattern as IMPL_V008 § E, and the same rule applies: writing documentation is not the
moment to change behaviour, so what follows is recorded rather than silently fixed. E.3 and
E.4 are documentation and were partly fixed in the pass; E.1 and E.2 are language questions
and were not touched.

### E.1 — A block lambda called for its effect: the tree-walker is alone

```zymbol
lam = () -> { >> "efecto" ¶ }
lam()
>> "sigo" ¶
```

| Engine | Behaviour |
|--------|-----------|
| `zytw` | prints `efecto`, then `Runtime error: block lambda must use <~ to return value`, and **aborts — with `rc=0`** |
| `zyvm` | prints `efecto` and `sigo` |
| `zyjs` | prints `efecto` and `sigo` |
| `zymbol check` | *No errors or warnings* |

Two against one. **The gate would catch this if it had the file** — the tree-walker's
stderr carries a `Runtime error` prefix, which `verdict_of` reads as a refusal even at
`rc=0`. It has no such file, which is the whole of why `zyq consensus` reports 0
divergences here and is right to.

Two things make it worth deciding rather than filing: a block lambda called for its effect
is the natural shape of a callback, and the tree-walker aborts **with `rc=0`**, so a script
that dies halfway looks successful to everything downstream of it.

Undecided, deliberately: whether requiring `<~` in a lambda invoked for its effect is
right at all is a language question, and there is no corpus file to add until it is
answered. It belongs in a `Divergente_ES` **DM** document.

### E.2 — The unmarked dictionary pattern survived the migration

`reject/collections/06_dict_sin_marca.zy` refuses the literal `d = (a: 1, b: 2)`, and its
own reasoning is: *"Both spellings would have been worse than either: two ways to write one
thing is what the mark was introduced to end."*

The **pattern** was not migrated. Both of these run, silently, in both engines:

```zymbol
#(name: n, age: y) = person    // the corpus spelling
 (name: n, age: y) = person    // accepted, no warning
```

A pattern is not a literal, so this may be deliberate. But it is exactly the second
spelling the mark was introduced to end, and nothing in the corpus or the reject set states
which it is. **Decide, then either add a reject case or write the exemption down.**

### E.3 — Nothing verifies `LLM.md`

`LLM.md` opens with *"Everything below was run through `zymbol 0.0.9` in both engines"* and
**no suite checks it**. `zyquality/suites.toml` registers `guide`, which executes every
annotated example in GUIDE.md; there is no equivalent for LLM.md, and that is why it had
accumulated five false claims while GUIDE.md had only the ones the last commit broke:

| Claim | Reality, measured |
|-------|-------------------|
| Rule 5: "functions see none of the caller's variables; only top-level `:=` constants pierce it" | A named function reads file-level names at call time, by value (§ 10 above) |
| "postfix on a literal does not parse: `[1,2]#?`" | Parses in all five shapes tried, both engines |
| "a runtime error in all four engines" | Three, 130 lines below the document's own note that `zyml` was retired |
| `(name: n, age: y) = person // named-tuple destructuring` | § E.2, plus the retired vocabulary |
| `std/time diff` with no sign convention | `diff(a, b)` is `a − b`, so earlier-first is **negative** — a model will write it backwards |

All five corrected. **The durable fix is a `zyquality/docs/llm_verify.py`** alongside
`guide_verify.py`; LLM.md's dense style means most blocks carry no `// →` annotation, so it
is not a copy of the existing script. Not done.

*Found in the same pass:* `guide_verify.py`'s gloss-stripping heuristic truncated any
annotated value that is itself parenthesised — `(##), 2, (1, x))` became `(##), 2,`. Fixed
in `zyquality/`, with the reason in a comment.

### E.4 — ✅ FIXED (2026-09-07) — the withdrawn dictionary syntax across six documents

> Kept for the record, because the *shape* is the useful part: a syntax change lands in the
> engines and in the document that argues for it, and then sits in every other document
> until someone greps for it.

Found by scanning every `.md` in this directory for `(name: value)` outside a `#(`, and for
"named tuple":

| File | What it said | Now |
|------|--------------|-----|
| `README.md` | An "array of dictionaries" example using `(name: "Alice", age: 25)` — **verified not to compile** | `#(…)`, and the corrected example runs in both engines |
| `ROADMAP.md` | The dict-literal gap resolved "— **No new type and no new notation**" | No new *type*, but a notation of its own, and why the empty one forced it |
| `REFERENCE.md` | `(a: 1)` as the key-addressed record; "Named tuple field not found"; "Named tuple destructure" | `#(a: 1)`; `##Key`, verified by catching it; "Dictionary destructure" |
| `IMPLEMENTATION.md` | "Named tuples" in the feature table; "Named tuple form" in the EBNF prose | Dictionaries, with the notation dated |
| **`zymbol-lang.ebnf`** | `tuple_or_grouped` still admitted `"(" identifier ":" expr … ")"` | `dictionary_literal` / `declared_mix_literal` / `dictionary_destructure`, and `[NI05]` — which said dict literals were *not implemented* — corrected |
| `ARCHITECTURE.md` | — | Untouched: its `NamedTuple`, `MakeNamedTuple` and `NamedTupleGet` are **Rust identifiers that still exist** (`crates/zymbol-bytecode/src/lib.rs:208,210`), not user-facing vocabulary |

A second broken example turned up in the same file while checking the first, and it was
not a dictionary at all: 囲碁's flood-fill snippet marks a point visited with
`訪問[点] = 1` — the **indexed assignment**, withdrawn in this same release. Corrected to
`訪問[点]$~ 1` and verified through an output parameter in both engines. It is the better
illustration of the two, because the edit form *is* the rule of the result: the statement
discards its result, so it modifies in place, which is exactly what marking a point
visited means.

The EBNF is the one that mattered most and was not in the original finding. IMPLEMENTATION.md
says of its copy: *"the canonical grammar is maintained in `zymbol-lang.ebnf` … If they ever
diverge, the file wins"* — so the normative grammar was **admitting a program every engine
refuses**, which is the exact failure its own v0.0.9 header describes itself as having fixed
once already. Both files are synced, and the header now records two correction passes.

Two figure classes were re-measured at the same time and are listed in "Verification":
the corpus counts (`597/599`, `655/661`, `544/544` and `530/532` were all in circulation
across four documents) and the ZyBank log, which the README gave as 25 findings with a
breakdown of 8/12/3/2 where the log holds **33** (12 BUG, 13 GAP, 6 ERROR, 2 IDEA), all
closed rather than open.

### E.5 — ✅ FIXED (2026-09-07) — the normative grammar, verified rule by rule

The collections pass corrected the grammar where the release had obviously moved it. A
second pass then asked the opposite question of **every** rule — *does the parser actually
do this?* — and the grammar was wrong in both directions. Each rule changed was probed
against the installed binary, and the probe is named in the comment beside it.

**It admitted what the parser refuses** — the dangerous direction, since a normative
grammar that blesses programs no engine runs is worse than one that is merely incomplete:

| Rule | The grammar said | The parser does |
|------|------------------|-----------------|
| `assignment_stmt` | `identifier "[" expr "]" compound_assign_op expr`, with `compound_assign_op` covering `=` `+=` `-=` `*=` `/=` `%=` `^=` | Refuses **all seven** at one site (`zymbol-parser/src/variables.rs:87`). `compound_assign_op` went with them — no production used it any more |
| `postfix_op` | `"[" expr "]"`, repeatable — so `m[1][2]` parsed | Refused since v0.0.9, for reading as well as writing |

**It described the implementation wrongly**, and one of these had been wrong for as long
as the file existed:

- **`#,|x|` was documented as an "element count" and `#^|x|` as a "maximum value."** They
  are the two number formats: `#,|12345.678|` → `12,345.678`, `#^|12345.678|` → `1.2345678e4`.
- The times loop said `Int expr > 0`. A count of 0 or less runs the body **zero times** and
  is not an error; and the condition path no longer reads its specifier through truthiness
  at all, which is the v0.0.9 rule.
- `id_start`/`id_continue` were given as "letter or symbol". The real rule is stated by
  **exclusion** — not whitespace, not a digit in any of the 69 blocks, not an operator
  character — which is why it takes every script without listing one, and why the
  apostrophe in `mI'` continues an identifier.

**It was missing four forms the parser accepts**: `##_` (the Unit literal), `0d` (the
decimal base literal, and the note that a base literal in the ASCII range is a *character*),
`@ (k, v) : pairs` (a pattern in the loop head), the edit statement `a[2]$~ 99`, and a
String as a nav step — `d["x">"y"]` walks two dictionaries.

**Four symbols were dangling**: `arg_list` (meant `call_arg_list`), `arm_pipe_expr` (used by
`match_arm_value` and never defined — now written out), and `letter`/`unicode_symbol_char`
(used by `id_start` and never declared as terminals).

The grammar is at **3.3.0**, its header records what this pass found in each direction, and
IMPLEMENTATION.md's copy is byte-identical to it again — that copy claims to be *verbatim*
and to lose to the file on any disagreement, so a stale copy is a second normative grammar.
The eight probes were re-run afterwards and all eight now agree with what the grammar says.

What this pass does **not** give is a re-derivation from the parser (`agents/ebnf_regen.md`,
still not run since v0.0.7). Every rule it touched is verified; an untouched corner stays
unverified rather than normative, and the header now says so.
