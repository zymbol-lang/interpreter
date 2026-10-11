# Implementation Plan — v0.0.10: the premises as source, a matrix with no red cell, and edits that happen in place

> **Status: in development on branch `v0.0.10`, feature-complete, release not started.**
> `Cargo.toml` reads `0.0.10` and the installed binary answers `zymbol 0.0.10`. Part I is
> what has landed, Part II is what remains before the tag.
>
> **This plan was written on 2026-10-11 and then acted on the same day.** Its first
> draft listed five debts in § E. Working through them closed all five, took five
> decisions to the author, filed eight findings, added a way to count what a program costs,
and found one — in § E.4 — that the draft
> itself had got wrong. § 13 and § E say what happened. It is committed on `v0.0.10`, one
> commit per finding and per repository (§ A, *Git*).
>
> **Every gate figure in this document was measured on 2026-10-11**, on the tree that
> holds those changes, against a release build of it. The commands and their output
> are in "Verification" below.
>
> **Figures that were not measured today say so.** Before/after numbers for the
> performance work are quoted from the commit or the finding that measured them, with its
> date. What today's run confirms is the *ratio* each one is now held to by
> `zyquality/cost/` — not the milliseconds.

---

## What kind of release this is

v0.0.7 was a stdlib expansion designed up front. v0.0.8 was a **debt release**. v0.0.9
was a **decision release**: rules that existed in the prose and in no engine, decided once
and enforced. v0.0.10 is a **measurement release**.

Two things changed about *how the language is known*, and almost everything in Part I
follows from them.

**The design got a rank.** On 2026-09-12 the nine documents that say what the language
*must* do left `interpreter/` for a repository of their own, `zymbol-design/`, and one new
file was written on top of them: `PREMISES.md`, 29 premises with an id each. Its rank is
**source**. Everything in this directory is now derived from it — `AGENTIC.md` cites the
ids it measures — and where a description and a premise disagree, the description is what
is wrong.

**The premises got a matrix.** ZyDDT, built at the end of v0.0.9, asks every engine the
same cell and reports who disagrees. At the tag it had filed 23 findings. It has filed
**200 more** since — `GLB-008…125`, `ZYTW-002…015`, `ZYVM-003…021`, `ZYJS-014…062` — and
its red list went from 258 ids to none (2026-09-27; it has stayed there, with every later
finding entering as a red cell or a declared debt and leaving green). None is open.

That is the release: not a feature that was planned, but **what a systematic question
finds when it is asked some 1 700 times of three engines**. Nobody had filed that `!?` could
not nest in the VM, that a `$>` callback ran on a second dispatch loop which skipped 49
instructions in silence, or that `"aa"$~~["a":"b":-1]` replaced the whole string. No
program in the corpus wrote those forms, so no golden could disagree about them.

Three sources of evidence set the scope:

1. **The matrix itself** — 46 declared axes, and the premises crossed against their cells
   in both directions by `zyddt premises`.
2. **Two LDV projects of a new shape** — GoL, the first application *escalated* to LDV
   rather than born one, and ZyBF, the first **mini LDV**.
3. **A performance sweep the author asked for** (2026-10-08 … 10-11), which started from an
   outside measurement of the VM against Python and ended in the model of how an edit
   reaches its collection.

---

## One thing v0.0.9 said that v0.0.10 takes back

It belongs at the top because a reader of both plans will otherwise find them in
contradiction.

IMPL_V009 § 10 is titled *"A named function captures the file's variables"* and records
that as shipped. **MEM-2 reverses it**: a variable is visible only inside its own scope,
and a named function sees its parameters and nothing else (`031148e`, 2026-09-13). The
v0.0.9 behaviour was the retirement, on 2026-08-24, of an isolation the language had had
before; v0.0.10 restores it, and for a stated reason — both ways of reaching a body,
calling it and taking it as a value, now behave alike.

It was measured before it was decided. Across 1 251 files, four sites read a file variable
from a named function: all four were corpus files testing that very rule, and **none was
in the nine applications**.

The lambda took one more day. It first got a *warning* where the function got an error,
and that asymmetry was withdrawn the next morning (`6c23097`, MEM-6): a lambda is a
**light** environment that opens no boundary of its own, so it reads whatever its
enclosing scope holds — the file at file level, the function's locals inside a function —
and inherits the isolation rather than being excepted from it. The strong reading would
have made `n$> (x -> x * factor)` inexpressible, and a rule that removes a capability
instead of relocating it is not a rule.

---

## The method changed too, and that is part of the release

**The design moved out.** `PREMISES.md`, `COLLECTIONS.md`, `SYMBOLS.md`/`SIMBOLOS_ES.md`,
`MEMORY_MODEL.md`, `FORMATTER_RULES.md`, `I18N.md`, `USERAPPI18N.md`, `LDV.md` and
`DESIGN_STD_DB.md` are in `zymbol-design/`, with `HOW_TO_CHANGE_ZYMBOL.md` as the door for
whoever touches an engine: the order (decide → implement → hold), the ways out of a hard
decision, and what every harness can and cannot see. A premise is changed by the author,
in a commit that changes nothing else.

**A finding is a file with a state, and the state is computed.** `ZyDDT/HALLAZGOS/` holds
one file per engine plus `GLOBAL.md`. The tables that used to be kept by hand rotted —
the one in `GLOBAL.md` stopped at `GLB-007` — so `zyddt findings` now reads each ficha's
own *Estado* line, and turns red on a closed finding still declared as debt and on a
header that contradicts its ficha (`STALE SUMMARY`; seven did on 2026-10-10).

**Open debt is declared, never silent.** A finding that is not fixed today is held by a
`KNOWN` cell naming it (`open_finding`), or by a case in the cost harness. Paying the debt
turns that cell red until the declaration is removed — so a fix cannot land without the
record noticing.

**The gate measures cost.** `zyquality/cost/` holds 14 cases whose limit is a *ratio*
declared in `cost.toml` — growth when the input quadruples, a call against the same code
inline — and no baseline in milliseconds. It exists because the microbenchmarks in
`bench/` run fixed programs that never touch module state or copy an aggregate, which is
where every quadratic cost of this release was hiding.

**The web suites entered the gate.** The ones that need `node` and nothing else run under
`zyq suite --only web` since 2026-10-10. They were outside it until `test_shell.mjs` was
found red for days (ZYJS-059).

| Project | Position | Log | Findings |
|---------|----------|-----|----------|
| [GoL (Ζωή)](https://github.com/zymbol-lang/ZyGoL) | ninth — the first *escalated* | `HALLAZGOS.md` | 5 `BUG` + 8 `GAP` + 1 `ERROR` + 3 `IDEA` = **17** |
| [ZyBF](https://github.com/zymbol-lang/ZyBF) | the first **mini LDV** | `HALLAZGOS.md` | 1 `GAP` + 1 `ERROR` + 1 `IDEA` = **3**, all three changed the language |

GoL was the fifth terminal grid game, which `LDV.md` § 6 advised against. What paid for it
was not the grid but **an application that tests itself from inside the language** — it
needed to build an error, read one back, see what a command exited with, hand arguments to
a subscript, and drive a full-screen program without a terminal. Those are §§ 7–8 below.
ZyBF is the cycle at the smallest size that still runs it: one short program, its log, its
suite in the gate, agreeing in all three engines.

---

## Feature map

| # | Change | Type | Engines | Origin |
|---|--------|------|---------|--------|
| 1 | `PREMISES.md` — 29 premises, each crossed with its cells | Added | design | author |
| 2 | MEM-2: a named function sees only its parameters; a lambda inherits | Changed | TW + VM + JS | MEM-2, MEM-6 |
| 3 | MEM-7: one name, one thing; `_` means private, not reserved | Changed | TW + VM + JS | MEM-7 |
| 4 | `\ name` — refused on what is not alive; ends a module's variable for the module | Changed | TW + VM + JS | GLB-055, GLB-120 |
| 5 | The error family says *what* is wrong: `##Type` vs `##Index` | Decided + enforced | TW + VM + JS | D1 |
| 6 | Collections are strict: the VM and zyjs stop guessing | Changed | VM + JS | D2 revoked |
| 7 | A slice written high-to-low builds backwards | Added | TW + VM + JS | D3 |
| 8 | A constant twice, a loose range, `<< C`: static errors | Changed | analyser | D4, D5 |
| 9 | VM: one dispatch loop, a handler stack, errors that stay pending | Fixed | VM | GLB-010, ZYVM-004 |
| 10 | Diagnostics name types, quote tokens as written, and say where | Changed | TW + VM + JS | F3, G5 |
| 11 | Four new warnings; the type checker enters where it never entered | Added | analyser + JS | HLZ-015/016, ZYJS-035, GLB-061 |
| 12 | `##Kind("…")` builds an error; `##Kind(m) =>` reads it back | Added | TW + VM + JS | GoL |
| 13 | A command's exit status is a soft `##IO` that carries it | Added | TW + VM + JS | GoL |
| 14 | A subscript runs in-process and takes its own arguments | Changed | TW + VM + JS | GLB-017, GoL |
| 15 | `zymbol run --keys` — a full-screen program tested without a terminal | Added | TW + VM | GoL |
| 16 | `$^` is a stable merge sort; `$??` overlaps; an empty pattern is `##Index` | Fixed | TW + VM + JS | GLB-073…084, GLB-108 |
| 17 | `##'expr` — an Int code point to its Char | Added | TW + VM + JS | ZyBF (GLB-109) |
| 18 | An edit in place writes where the collection lives | Fixed | TW + VM + JS | ZYVM-003/013, GLB-115 |
| 19 | An output parameter is the caller's variable, lent | Changed | TW + VM + JS | GLB-117 (L49) |
| 20 | A module's state lives in one place | Fixed | TW + VM | ZYTW-010, GLB-119 |
| 21 | A call frame holds what the function needs at once | Fixed | VM | ZYVM-010 |
| 22 | A condition is a Bool and nothing else | Decided + enforced | TW + VM + JS | GLB-121 (L51) |
| 23 | A block lambda that ends without `<~` gives `##_` | Fixed | TW | ZYTW-015 (L52) |
| 24 | The dictionary pattern is written `#(…)`, and the mark needs its pairs | Changed | TW + VM + JS | GLB-122, GLB-123 (L53) |
| 25 | An identifier in a `??` pattern is checked as a read | Fixed | analyser + JS | GLB-124 (L54) |
| 26 | `LLM.md` behind probes that run | Added | `zyquality` | IMPL_V009 § E.3 |
| 27 | A named function is a `??` pattern; a module alias is never a value | Decided + enforced | TW + VM + JS | GLB-125 (L55) |
| 28 | `zymbol run --count` — steps, calls, allocations, bytes, elements copied | Added | TW + VM, `zyquality/cost` | the bench's noise |

---

# Part I — Shipped

## 1. The premises — `zymbol-design/PREMISES.md`

Twenty-nine, in five families: `MEM-1…8` (memory and scope), `COL-1…7` (collections),
`SYM-1…8` (the symbol system), `AGT-1…5` (what an agent can know before running a
program) and `TYP-2`. `zyddt premises` crosses each with its cells in both directions — a
premise with no cell and a cell naming a premise that does not exist are both reported.
Today: 0 with no cell, 7 checked by review rather than by execution (the `SYM` family is
mostly about how a mark is chosen, which no program can answer), 1 held by a suite outside
the layer.

The weight is uneven and that is information: `COL-7` (*nesting is navigated with `>`*)
holds 61 cells, `MEM-8` 27, `TYP-2` 25, `MEM-6` 22 — and `COL-6`, *assignment copies;
there is no aliasing*, holds **one**.

## 2. Scope: MEM-2, MEM-6, MEM-7

The reversal is described above. What shipped with it:

- **A module's top level is its state** (MEM-4), never a crossing. The first sweep for
  MEM-2 reported 240 hits and 168 were exactly that.
- **One name, one thing, inside one strong environment** (MEM-7). A file variable may not
  take the name of an import alias; a function's variable hides an alias of its name
  behind the dot (GLB-070). `x.f(…)` is a module call only when `x` is a module (GLB-066).
- **`_` means private, not reserved.** Outside its block a read is undefined and an
  assignment declares a new one. `_k°` is a legitimate private accumulator; only the
  prefix `°_k` is refused.
- **A loop iterator over a visible name assigns it** (ERROR-GOL-017), and one name in two
  scopes is two variables (GLB-105) — the latter uncovered *true* warnings in the
  applications' modules.

## 3. `\ name` says what it ends

MEM-8, in the two places no engine applied it whole.

**GLB-055.** `\` on a name that is not visible is the static `undefined variable` a read
would be; from a function on an outer name, `'x' is destroyed from outside this function`;
on a constant, a function or a module alias, refused statically; a second `\` is a
run-time use after destruction. `\ x` in the VM had done *nothing at all* for a file
variable (GLB-008).

**GLB-120 / L50**, decided 2026-10-11. A module's state is the module's, so its end is
too: every function of the module finds the variable destroyed, assigning it gives it back
to the module, and a parameter or a lambda's copy of that name is its own. A probe of 39
steps against the rule differed in 9 in the tree-walker and 11 in the VM; none now.

## 4. One error family, and collections that stop guessing

Five decisions of 2026-09-15, D1–D5. Four were implemented; **one was revoked five days
later, and the revocation is the one that mattered.**

- **D1.** The family is *what* is wrong, not where. A wrong **type** is `##Type`; a wrong
  **value** of the right type — 0, negative, out of bounds — is `##Index`. All three
  engines declare the kind at the raise site: the VM in its variant, zyjs in
  `ZyRuntimeError(msg, kind)`, the tree-walker in `RuntimeError::Kinded`, which *wraps*
  so the ~200 sites with nothing to declare keep being read by their words.
- **D2 — revoked 2026-09-20.** It had said collections were tolerant. The author:
  permissiveness is the defect; a stricter language asks for correct expressions and gets
  more exact requests. **Strict was not a new design — it was the tree-walker**, which
  already refused all 40 forms. The VM accepted 13 of them and now accepts 2. What it had
  been doing: `"ab"$+ 5` answered `ab5`; `[1,2]$[1..9]` clamped to `[1, 2]`; `$-[0..2]`
  emptied the array, a 0 having passed through `.max(0)` as a valid start; and a
  replacement count of −1 was clamped to 0, which means *all*.
- **D3.** A slice written high-to-low **builds, by reversing**: `a$[3..1]` → `[30, 20,
  10]`, and in navigation too. It is the only reversal the language has — `$^-` sorts,
  which is a different question — and no new spelling was invented for it.
- **D4, D5.** A constant declared twice, a loose `v..3`, `@ (a,b):range`, `<< C` and
  `@ C:` over a constant are static errors in the shared analyser. Derived from MEM-1 and
  MEM-7, so no premise changed.

Decided alongside, and binding: **a string is an array of characters**, so `$~` writes into
one; the higher-order operators `$>` `$|` `$<` walk whatever `@` walks, a dictionary
yielding its keys; a `$|` predicate and a `$^` comparator must answer a Bool — no
truthiness, the third time that rule has been closed.

*A trap worth keeping:* the tree-walker is the reference for `$` again, **but check its
text before copying it**. Four of its messages printed the internal 0-based offset, and
aligning on them would have spread the defect to two more engines (GLB-047).

## 5. The VM has one dispatch loop

`cfcf2f2`. The largest single change to an engine in the release, and two findings.

**GLB-010.** The VM kept one handler per frame and no notion of an error still pending.
So a `!?` could not nest, a `:!` filter that did not match *swallowed* the error, a `:>`
without a catch had nothing to rethrow, and `@!`/`@>`/`<~` crossed a `!?` without
disarming it or running its `:>`. Now the outer handler is saved in a register of its own,
a landed error is pending until a clause consumes it, and every way out of a `!?` emits
what leaving it owes.

**ZYVM-004.** A function called by `$>`/`$|`/`$<`/`$^` ran on a **second loop that knew 96
instructions and skipped 49 in silence** — `["a,b"]$> (s -> s$/ ',')` answered `[()]`. It
is deleted; the one loop is reentrant, and an error from a callee reaches the `!?` around
the operator.

The CLI runs programs on a 64 MiB thread as a consequence: recursion depth through `$>`
went from 593 to 4 764 in the VM, and from 899 to 7 212 in the tree-walker.

## 6. Diagnostics — phases F3 and G5

- **A message names the type, not the value's `Debug`**, and never names a type with a
  symbol. The analyser writes a dictionary as `#(k: Int)`.
- **A refusal quotes the token as written**: `found '='`, and the comparison operator is
  `'<'`, not `'Lt'`.
- **One refusal, one message.** The parser's cascade is cut; a diagnostic in a `<~` is
  given once (GLB-095).
- **An error says where**: after a nested block it is at its statement's line (GLB-106);
  in an expression lambda, at the statement that called it (ZYVM-012); a `??` no arm
  matches, at the line of its statement (ZYVM-011).
- **The lexer names the cause.** An unterminated string says so and stops blaming a brace;
  when a quote and a brace are both open it reports both, quote first.
- **The arity help shows the signature** from the caller's view — `expected signature:
  g(a, b<~)` — and `wording.baseline` has zero entries: a stale one is red, and
  `--regen-baseline` is retired.

## 7. What the analyser now sees

Four warnings that did not exist: **an import nobody uses** (HLZ-015), **code that cannot
run** (HLZ-016 — after `<~`, `@!`, `@>`, or under a condition that folds), **a bare
expression statement that does nothing** (ZYJS-035), and **an unused constant, called a
constant** (GLB-061). `zymbol run` now prints the type phase's warnings when it refuses,
where it used to drop them (DG-08).

And the type checker **enters the places it never entered** (step 3.8, ZYTW-001, GLB-075,
GLB-112): a lambda's block body, the arguments of `<\ … \>`, the operand of a cast. This is
the sixth appearance of one shape — *an arm of the inference that discards its operand
turns off every check below it* — and it is recorded as a pattern, not as six bugs.

The inference itself was corrected in a run of twelve (GLB-092…105): an assignment gives
its type to the name where it was born; an empty array reassigned with elements is not a
type change; `##_` never changes a variable's type; a constraint reaches a parameter only
when **every** path requires it; the right side of `&&` and `||` does not bind one.

## 8. An error is a value you can build — from GoL

`1e1a2d1`, each of the three decided before it was built.

- **`##Kind("message")`** builds a soft error value: the spelling an error prints as, and
  the kind a `:!` matches. The kind is any name in any script.
- **`##Kind(m) =>`** in `??` reads the message back — **the first pattern that creates a
  name**. The binding is a birth like `m = …`: a visible `m` is assigned, never shadowed
  (MEM-7).
- **A status other than 0 is a soft `##IO` that carries it**: `exit N`, then what the
  command said.

A soft error is a value: the documents stopped promising that a `:!` catches one.

## 9. Subscripts, and a screen without a terminal

**In-process** (GLB-017 I, `77aa0c9`). The VM had run `zymbol run "<path>"` through `sh`:
the first `zymbol` on the `PATH`, always the tree-walker, and a `"` in the path got out of
its quotes. Both engines now run a subscript in this process, down the road `zymbol run`
takes, in the caller's engine. A standalone executable, which has no runner, refuses one.

**Its own arguments** (`e572147`). `</ app.zy -L es --list />`: the words after the path
are the subscript's command line, literal like the path — so `zymbol package` and AGT-3
still read the reachable program without running it.

**`zymbol run app.zy --keys app.keys`.** `>>|` draws on a virtual 24×80 screen, `<<|` reads
a key script, and each block's last frame goes to the output as text. One implementation
for both engines, in `zymbol_common::vscreen`. This is what let GoL delete its Python
harness and be 100 % Zymbol. Without `--keys`, `>>|` with no terminal still fails, and
says so in the language's words (GLB-068).

## 10. The `$` family, measured against itself

- **`$^` is stable** (GLB-073): swap only when the second goes before the first. With a
  comparator it is a **stable merge sort** (GLB-108) — it had been quadratic in all three
  engines; at n = 4 000 the VM went from 1 102 ms to 21 ms (measured 2026-10-09). The
  first patch proposed from outside was a merge that undid the stability; no application
  uses `$^` with a comparator, which is why nobody had met either defect.
- **`$??` overlaps**, and an **empty pattern is a `##Index`** for `$??`, `$/` and `$~~`
  (GLB-078/079/081). Found on the way: a tree-walker panic on a long pattern, wrong
  non-ASCII positions in the VM, and a first-N replace that rescanned its own output.
- **`$-[i - 1]` and `$[…]` take arithmetic** in their bounds (GLB-116).
- **A dictionary is never addressed by position**, nested or not (GLB-065), and `d$- v`
  removes by value from one in the VM as it did in the tree-walker (ZYVM-017).
- **`-1 =>` is a pattern**; `(3) =>` is not (GLB-062).

## 11. `##'expr` — from ZyBF

A Brainf*** interpreter's `.` has to turn a cell into a character, and there was no
Int → Char conversion. `##'` was already the Char mark for typed input; as a prefix on an
expression it is now the cast, a sibling of `###` and `##.`, and like them it refuses
nothing statically — `##Type` and `##Range` at run time (GLB-109). ZyBF's `.` went to
`salida = salida ##'cinta[p]`, and the program from 96 lines to 75.

Its other two findings: a base-prefixed literal is a **Char in any range**, which three
documents said wrongly (GLB-110); and a prefix operator opens a juxtaposed operand, so `=`
and `+=` stop disagreeing about juxtaposition (GLB-111, GLB-114).

## 12. Edits happen in place

The performance work, and the one part of the release whose scope the author set in a
sentence: *a language that is not optimal stops being functional.*

It began outside. ZyBench measured the VM against Python on 2026-10-08, and what was
validated from it was documented before anything was changed. The sweep that followed
found one shape four times: **a collection with two owners is copied by its first edit.**

- **Module state in the VM** (ZYVM-003, ZYVM-013). `LoadGlobal` cloned the `Rc`, so
  `make_mut` saw two owners: updating one element cost the size of the collection. Edit
  instructions are now atomic and validate before they take the value.
- **The order of an edit's operand** (GLB-115). Left to right, in every engine, as
  everywhere else in the language. The tree-walker had kept a different order only on the
  fast paths of three of its eight operators.
- **`<~` lends** (GLB-117, L49). An output parameter was copied in and written back, so the
  caller's variable and the parameter were two owners while the call lasted. It is now the
  caller's variable, **lent**: 2 000 calls of `put(t<~, i, v) { t[i]$~ v }` over 16 000
  elements went from 2 808 M instructions to 55 M in the tree-walker and from 1 204 M to
  19 M in the VM (commit `3186680`). Three answers depended on the copy and each was
  decided: what the function wrote before failing **stays written**; one variable cannot
  be two output arguments of one call (static, `'e' is given to 'h' as an output argument
  twice`); module state passed to a function of its own module is still copied.
- **The mirror of file variables** (`af025e0`). Both engines kept a second copy of every
  file-level variable so a function could read it — which, since MEM-2, it cannot. Only
  what a function can still reach is mirrored.
- **Module state in the tree-walker** (ZYTW-010, `13b394b`). A module frame was handed a
  copy of each variable it names and compared them all on the way out. Now the module's
  table *is* the frame's outermost scope, moved and never copied: 12 000 calls writing one
  cell of a 12 000-cell table went from 1 930 ms to 27 ms, and the growth ratio from 13.76
  to 3.27. The author declared it debt on 2026-10-10 and asked for the fix the same day.
  **It also fixed what nothing was looking at**: a frame that failed dropped its copy, and
  with it what it had written before failing.
- **A name that is module state is that state, however it gets its value** (GLB-119) —
  nine forms changed in the VM and one in zyjs that the finding did not list.
- **The call frame** (ZYVM-010). `drop_glue<Value>` was 10–20 % of the VM's time; a register
  write now drops the old value only if it owns memory, and a frame holds what a function
  needs at once rather than every temporary its body ever asked for. No `unsafe`.

Today's cost gate holds all of it: every ratio within its declared limit, the worst growth
at ×4 being 3.87 against a limit of 6.00 (4.04 in the morning's run).

## 13. What writing this plan found — 2026-10-11

The first draft of this document recorded five debts and touched nothing, as IMPL_V008
and IMPL_V009 had. The author then asked for them to be corrected. Three of the five had
been carried, whole, from IMPL_V009 § E — a month in a released plan, with no finding
number and no cell, while `zyddt findings` reported nothing open.

**A condition is a Bool and nothing else (GLB-121, L51).** `? x { }` with `x` not a Bool
was a warning, and under it each engine chose a branch by rules of its own. Thirteen kinds
of value were swept: **nine diverged**, all three engines exiting 0. The tree-walker read
an Int that is not 0 and a String that is not empty as true; the VM read everything but 0
and Unit as true; zyjs used its host's rules. Three options went to the author with that
table — a `##Type` error, declared debt, or a truthiness table — and the first was taken.
Static where the analyser knows the type; a `##Type` at run time through a parameter,
before either block runs. The VM got a jump of its own, `JumpIfNotCond`, so the check
costs no extra dispatch. Measured before deciding: **0 of 1 292 programs** in the
workspace provoked the old warning.

**A block lambda that ends without `<~` gives `##_` (ZYTW-015, L52).** The tree-walker
alone refused it, on the natural shape of a callback. Four forms were measured — called
as a statement, used as a value, a `<~` on one path only, and as the lambda of `$>` — and
in all four the VM and zyjs went on with `##_`, as a named function does in all three.

**The dictionary pattern is written `#(…)` (GLB-122, GLB-123, L53).** The bare
`(name: n) = d` had run for a release after the bare literal was refused. Refusing it
turned out to need two more things. In a **loop head** the Rust parser accepted *only*
the bare pattern and zyjs neither, so the refusal alone would have removed the form
instead of relocating it: `@ #(k: a, v: b):rows` is accepted in all three now — which is
what `zymbol-lang.ebnf` had been saying all along, of a parser that did not exist. And the
converse, `#(a, b) = (1, 2)`, a positional pattern under the dictionary's mark, ran in the
two Rust engines and was refused by zyjs; decided separately, and refused.

**`LLM.md` has a harness (`zyquality/docs/llm_verify.py`, suite `llm`).** The document is
tables of forms and cannot be executed as GUIDE.md is, so it is not: **88 probes**, each
quoting the sentence it stands behind, run in all three engines. The suite goes red when a
probe stops holding, when a quoted sentence is no longer in the document, and when one of
the fifteen rules of § 1 or the twelve bullets of § 12 has no probe — so a rule cannot be
added without something that runs. **Its first run paid for it**:

- `LLM.md` § 6 said `ambiguous lifetime for 'i'` is emitted for *every* bare iterator.
  It is emitted when the body assigns to its own iterator, and has been since
  ERROR-ZYB-003. `REFERENCE.md` said the same and was corrected with it.
- **ZYJS-061** — rule 11, *"`$>` with a named function takes NO parentheses"*: true of
  the Rust parser, and zyjs ran `nums$> (double)`.
- **GLB-124 (L54)** — found through a probe that was itself written wrong. An identifier
  used as a `??` pattern is a read of that name, and the type checker's arm for it was
  empty: a file variable named from inside a function passed `check`, and then the
  tree-walker failed half way through the output, the VM refused to compile, and zyjs read
  the variable MEM-2 says it cannot see. The seventh appearance of the blind-spot shape
  of § 7.

**And the gate, run on all of it, found two more**: a hot name accepted as a `??` pattern
by zyjs alone (ZYJS-062, fixed), and an identifier pattern that names a *function* or a
*module* — not the same question as GLB-124, since there the name exists and is in view.

**A function is a value and a module is not (GLB-125, L55).** It was held for a few hours
as declared debt, with two `KNOWN` cells, and then decided — after a measurement that
changed the question. The same pattern with the name in a **variable** (`g = f`, then
`g =>`) already matched by identity in all three engines; only the declared name failed,
because the two Rust engines looked it up among the variables and not the functions. So
`f =>` is a comparison, as `==` on functions is. For a module the measurement went the
other way and widened the finding: `x = M`, `M == M` and `f(M)` passed `check` exactly as
`M =>` did, each engine then answering on its own. A bare alias is refused wherever it
stands as a value — `'M' is a module, not a value` — and `M::name(…)` and `M.NAME` are
untouched. Swept before it was kept: 0 of 1 292 programs in the workspace get the new
error.

**`zymbol run --count`.** The verification table of this plan's first draft carried a
warning that the benchmark suite moved different rows by ±40 % between two runs on one
day, and a recommendation to "count instructions, not milliseconds" with nothing to count
them with (`perf` is installed and `perf_event_paranoid=3` forbids it). The flag is that
instrument, built into the engines:

```text
$ zymbol run --vm --count put4000.zy
count  steps=124026  calls=4000  allocs=20624  bytes=3006621  copied=0
```

Five numbers, exact — three runs give the same line. `steps` and `calls` are the engine's
own work (a bytecode instruction in the VM; a statement or an expression in the
tree-walker) and are not comparable between engines. `allocs` and `bytes` come from a
counting allocator. `copied` is the number of elements duplicated because a collection
about to be written had another owner, counted at the one function every such write now
goes through (`zymbol_common::count::make_mut`).

The last one is why the flag is not just a step counter. **A step counter would have seen
none of the four quadratic costs of § 12**: writing into a shared collection is one step
at any size. Checked on a program that is quadratic on purpose — `b = a` then `b[1]$~ i`,
N times over N elements — `copied` goes from 250 000 at N = 500 to 4 000 000 at N = 2 000,
×16 to the unit, in both engines.

`zyquality/cost/` uses it: every `growth-ratio` case is now also asked in counted units,
60 assertions beside the timed ones. All ten templates measure **×4.00 in steps** at ×4
the input (×4.51 and ×4.59 for the sort, which is n·log n) and **0 elements copied**. The
limits sit at 4.2 where the clock's have to sit at 6.0.

What it costs: a clock could not say — old and new binaries interleaved, minimum of 12,
gave anything from −0.3 % to +4.6 %, the size of the noise the flag was built to get away
from. Counted in CPU instructions the same evening, once the author had opened `perf`:
**+0.4 % to +1.6 %** for the counters. **It is compiled in, not behind a feature.** § A has
the whole measurement, and the one number in it that is not small.

What held each of these down afterwards: `axes/condition.toml`, a new axis of 13 kinds of
value × 5 places a condition can be written, plus eight cells by hand (73); twenty-six cells
across `refusal`, `runtime-functions-hof` and `syntax-functions-lambdas`; four files in
`zyquality/reject/` and two in the corpus with their goldens.

---

# Part II — What remains to close v0.0.10

## A. Release closure

**What was checked on 2026-10-11 is the gate, the state of the repositories, and the
documents the day's changes touched** — not every document against the binary. A row
marked ✅ says what was done to it; a row without one says what a `grep` found. The v0.0.9
pass is the measure of what a full review finds (19 items in GUIDE, five false claims in
LLM, a normative grammar wrong in both directions).

| Item | State |
|------|-------|
| `CHANGELOG.md` | ✅ `[0.0.10] — unreleased` written 2026-10-11: 226 lines, every entry naming its finding. It is written at the grain of this plan, not of v0.0.9's 922 lines — § E.5 says why that is a limit and not a choice. Needs a date when the tag is cut |
| `LLM.md` | ✅ titled v0.0.10 and **verified by a suite** (`zyq suite --only llm`, 88 probes, § 13). One false claim corrected, four rules added |
| `GUIDE.md` | ✅ header and *New in v0.0.10* written; three sections added (a condition is a Bool, a block lambda without `<~`, the dictionary pattern in a loop head). `guide_verify` executes its annotated examples. The body outside those sections was not re-read |
| `REFERENCE.md` | ✅ header at v0.0.10; L51–L54 added; the `ambiguous lifetime` row corrected. The error taxonomy and the symbol table were **not** checked for `##Kind(…)`, `##'expr` and the D1 families |
| `IMPLEMENTATION.md` + `zymbol-lang.ebnf` | ✅ grammar at **3.4.0**: `##'expr`, `##Kind("…")`, the `##Kind(m)` pattern, subscript arguments and the negative literal pattern written in, each probed in all three engines; `loop_pattern` finally true. The copy in IMPLEMENTATION.md is byte-identical again. A targeted pass, as the header says — not a second rule-by-rule verification |
| `AGENTIC.md` | G10 closed with its history. **Not re-measured as a whole**: G4 was probed and stands; G1, G2, G3, G5 and G8 carry their 2026-09 measurement, and the header now says so |
| `README.md` | Badge at v0.0.10. *"Current status (v0.0.9 branch — 0.0.8 is the latest published release)"* and the figures under it are still v0.0.9's — to be rewritten from the Verification table below, in a fresh clone |
| `ROADMAP.md` | Status header says *v0.0.9 (in development)*. Not touched |
| `ARCHITECTURE.md` | Parity and benchmark paragraphs are dated 2026-09-07. The VM section predates the single dispatch loop (§ 5), the lending of `<~` (§ 12) and `JumpIfNotCond` (§ 13). Not touched |
| `zymbol-design/` | On `v0.0.10`, clean, pushed. `MEMORY_MODEL.md` and MEM-5 were updated on 2026-10-10/11 for ZYTW-010 and `<~` |
| `web/` | On `v0.0.10`, 166 commits since the tag, suites green in the gate. **The install pages and their Markdown twins still say v0.0.9, correctly** — the download links are not touched until the release exists. `web_release.md` and `web_manual_update.md` are the procedures |
| VS Code extension | `0.1.6`, six commits since the tag (the grammar knows `##'` as a cast and an error kind in any script). Needs a version and `bash build-extension.sh`. The `tmgrammar` surface in ZyDDT reports nothing unmarked |
| Git | **21 repositories**: 20 on `v0.0.10` and ZyBF on `main`. The work of 2026-10-11 is committed in `interpreter/`, `web/`, `zyquality/` and `ZyDDT/`, one commit per finding and per repository as the branch has gone so far; a file that held pieces of several findings was committed piece by piece, and every intermediate state of `interpreter/` was compiled before it was committed. Pushing, tagging and merging are the release itself and are not done. `.github/` and `packaging/` are identical between `main` and `v0.0.10`, so the workflow definitions the release event reads are already the right ones — the order in `agents/release_merge.md` (merge **before** publishing) is still the order |
| Installers | `agents/release_installers.md`. The `.deb` gate mounts ZyQuality in the container; it has not been run against this branch |
| Windows `.msi` | Signing is manual in the SignPath portal; the Windows job fails by design until it is done |
| The cost of `--count` | ✅ **Measured 2026-10-11** in CPU instructions (`perf stat -e instructions:u`, three binaries: `d736ae6`, today's tree with the counters turned into no-ops, and today's tree). **The counters themselves: +0.4 % to +1.6 % in the VM, +0.4 % to +1.1 % in the tree-walker** — inside the 1–2 % that was set as the limit, so they stay compiled in. **`JumpIfNotCond` costs nothing**: the VM is −1.9 % to +0.4 % against the baseline before counting. **The tree-walker, whole, is +2.0 % on `bench_recursion`, +1.6 % on `stress`, +1.4 % on `bench_match`** against `d736ae6`, and most of that is not the step counter: it survives in the no-op build, where the only things left are the allocator wrapper and the ownership check in `make_mut` — and the tree-walker asks the allocator about 15 times per call where the VM asks about 5. One real cost was found and removed on the way: `?` computed the source position of its condition on every run and now only on the way out (−0.7 points). Whether +2 % on the tree-walker's worst case is acceptable, or the allocator goes behind a cargo feature, is the author's call |
| zyjs and `--count` | Not built. The browser engine has a step counter of its own (`this.steps`, its runaway-loop limit) and cannot count allocations; whether `run_one.mjs` should print half a line is undecided |
| Toolchain | Still no `rust-toolchain.toml`: `cargo fmt --check` and `clippy -D warnings` are red by version, not by debt. Do not run `cargo fmt` to "fix" it before the tag |

## B. Inherited debt — auto-free, a third time

Both items of IMPL_V008 § B are still in `ROADMAP.md` § Known Gaps and **still undecided**.
The deadline IMPL_V008 set itself has now passed three times.

1. **VM expression temporaries.** This is no longer the item it was: ZYVM-010 made the
   compiler reuse temporaries per statement, so the window in which a temporary keeps a
   large value alive is not the one IMPL_V008 measured. **Re-measure before deciding** —
   by peak RSS with sequential against overlapping lifetimes, not by final memory.
2. **Flat regions only.** Unchanged. The standing recommendation is unchanged too: leave
   it, and write the reason into `last_use.rs`.

## C. Decisions that are open

**The VM as the default engine.** Confirmed by the author on 2026-10-10; **the version is
not set**. Measured that day with the VM standing in as the default: every golden via
`run` and via `check` matches under `zyvm`, the strict two-engine consensus diverges on
nothing including the words, `guide_verify` passes with `--vm` forced, and start-up is
2.5 ms against 2.4 ms. What the change itself needs and has not had: the default of
`zymbol run` in `zymbol-cli` (a `.zyp` already defaults to the VM), **the REPL, which runs
on the tree-walker**, the goldens, which are recorded and checked against `zytw` only, and
every document and install page that says which engine is the default.

The recommendation on record is to make it the *first* commit of the next version rather
than the last of this one. On the day it was confirmed, probing one zone — edits, module
state, `<~` — turned up seven faults in the VM and seven in the tree-walker that the gate
had not seen, and the VM changed inside. The zones not yet probed that way: loops and hot
variables, `??`, error flow, strings and formatting, modules.

**The applications' screens.** `--keys` and subscripts with arguments have reached GoL
only. A ZyBank pilot works and agrees in both engines but cannot have a golden — its
movements carry today's date — and the four games seed their randomness from the clock.
To decide: fix "today" and the seed per application, or give the CLI an option.

**Dismissed, and staying dismissed:** `do-while ~>` and match identifier binding
(2026-06-12). `##Kind(m) =>` is not the second one reopened — it binds a *message* out of
an error, and § 8 says why that is a birth and not a comparison.

## D. Live divergences the gate does not see

Same statement as IMPL_V009 § D, and the same qualification: the gate is not blind to
these shapes, **it has no file with them**. A suite reports on the corpus it has. What
changed is the size of the denominator — some 1 700 cells where there were 394 on 2026-08-30 — and the fact
that ZyDDT's own verdict line now says it: *GREEN — nothing regressed in what has been
declared. It does not mean correct: an axis nobody declared has no row above.*

---

## Verification

Measured 2026-10-11 on branch `v0.0.10`, on the tree described in § 13 — `d736ae6` plus
that day's changes in `interpreter/`, `web/`, `zyquality/` and `ZyDDT/` — against a
release build of it. The same run on `d736ae6` alone, that morning,
also reported *all gates pass*; the two differ by exactly what § 13 added.

```bash
cargo build --release && cargo test --workspace   # 1065 passed, 0 failed, 4 ignored
cd ../zyquality && make && ./zyq suite            # all gates pass
```

| Suite | Result |
|-------|--------|
| `zyq consensus` | 683 files × 3 engines: **677 agree, 0 diverge**, 6 with too few engines |
| `zyq expect` | 647 goldens via `run`: 647 match, **0 stale**, 0 unchecked · 30 via `check`: 30 match |
| `zyq reject` | 55 forms × 3 engines: **55 refused everywhere**, 0 accepted somewhere |
| `zyq audit` | 683 `.zy` · 677 with goldens · 6 consensus-only · 6 excused for every engine |
| `zyq selftest` | 56 checks, all passed |
| `fmt` | 781 files: 733 PASS, **0 FAIL**, 48 SKIP · P1–P4 all zero |
| `fmtcheck` | 9 LDV applications formatted, **suite unchanged** in every one · 5 pins hold |
| `tui` | 3 cases × 2 engines: 3 agree, 0 diverge |
| `guide` | 243 blocks, 130 tested: **130 PASS**, 0 FAIL |
| `llm` | **88 probes PASS**, 0 FAIL · 27 of 27 rules and bullets of `LLM.md` covered — new in this pass |
| `lsp` | 683 files · 14 disagreements, **0 outside the baseline** |
| `project` | 9 LDV applications: **every golden holds**, 0 diverge |
| `addressing` | 24 cells: **24 hold**, 0 do not |
| `messages` | Rust 925 · zyjs 555 · nothing new on the shared surface (baseline 212 of 438) |
| `reach` | 685 diagnostics defined in the gated layers: 517 provoked, 168 not — nothing new unprovoked |
| `bench` | 16 PASS, 0 FAIL, 0 NEW |
| `zyddt` | 46 axes, **1 704 cells: 1 704 agree**, 171 of them checked against an oracle that is not ours · 0 diverging, 0 known · 12 pins, all agree · findings: **0 open**, 226 closed, 2 re-routed |
| `zyddt premises` | 29 declared · 0 with no cell · 7 by review · 1 held elsewhere |
| `cost` | 14 cases: **every ratio within its declared limit** — the timed ones, and 60 counted with `--count` (steps ×4.00, 0 elements copied) |
| `course` | the course imports the playground's highlighter; no copy |
| `web` | 15 suites pass · example pool 332 files: 216 agree, **0 diverge** |

Three things about this table that a reader should not take further than they go:

- **`bench` is noise at this size, and two runs on one day show it.** The morning run had
  `tw/bench_strings_modify` at +37 % and `tw/bench_index_read` at +20 % against the
  baseline; the afternoon run has the first at −3 % and `vm/bench_strings` at +58 %
  (27 ms against 17). Different rows each time, on programs that last 15–40 ms. The one
  row a change of this pass could plausibly move — `vm/bench_recursion`, since every `?`
  now compiles to `JumpIfNotCond` — reads +0 % and +5 %, which is inside the same noise
  and is not a measurement. `zymbol run --count` (§ 13) is what came of this bullet; it
  answers "is this linear" exactly, and it does **not** answer this one, because the new
  jump replaces the old one instruction for instruction and the step count is identical
  by construction. That needed CPU instructions, and got them: it costs nothing (§ A,
  *The cost of `--count`*).
- **Two message baselines were edited by hand-run commands.** `messages/extract.py
  --prune` and `messages/reach.py --prune` removed four entries for the two condition
  warnings that no longer exist; and `reach.py --baseline` then **added one**: the
  tree-walker's `undefined variable '{}' in match pattern`, which nothing provokes any more
  because the analyser refuses every such program first (GLB-124, GLB-125). It is kept as a
  backstop rather than deleted, and listing it is a statement that it is dead from the
  command line. `git diff zyquality/messages/` is the review: one line in, four out.
- **The figures need re-deriving in a fresh clone** before they are quoted in release
  notes. The v0.0.8 rule stands: a count measured in a working tree that holds files
  `.gitignore` keeps out is not the count a clone will get.

A finding is not closed until it has a regression test that **fails on the previous
binary**.

---

## E. Debt found while writing this plan (2026-10-11) — and what became of it

The first draft of this section followed IMPL_V008 § E and IMPL_V009 § E: record, touch
nothing. It listed five items. All five were acted on the same day; § 13 is the account,
and this section keeps what each one was and how it ended, because the pattern is the
useful part.

**Three of the five were IMPL_V009's own § E.1, § E.2 and § E.3**, inherited whole. A debt
written into a released plan has an author and a date and no owner: it is not a finding
(no number), not a cell (nothing goes red), and the next plan copies it forward. That is
how `zyddt findings` could say *0 open* for a month over three live divergences. **The
rule that came out of it: a debt recorded in a plan is filed as a finding the same day,**
with an `open_finding` cell if it is not fixed — which is what was done with the one that
remains (GLB-125).

### E.1 — ✅ FIXED — a block lambda called for its effect

ZYTW-015. Decided: it gives `##_`. § 13.

### E.2 — ✅ FIXED — the unmarked dictionary pattern

GLB-122, and GLB-123 for its converse. Decided: refused, both. § 13.

### E.3 — ✅ FIXED — nothing verified `LLM.md`

`zyquality/docs/llm_verify.py`. § 13.

### E.4 — ✅ FIXED, and the draft of this section was wrong about it

The draft said `AGENTIC.md` § G10 *"describes a divergence that no longer exists"*, on
the strength of one probe:

```zymbol
x = 5
? x { >> "si" ¶ } _ { >> "no" ¶ }
```

All three engines warned and printed `si`, and the draft concluded the divergence was
gone and the row stale. **It was not gone.** With an Int the three engines had always
agreed on the branch; with an Array, a Float, an empty String, a Char, a Tuple or a
Dictionary they did not — nine of thirteen kinds of value. `AGENTIC.md` was right, and
its own text says how the mistake is made: *"the check was on the diagnostic, not on the
branch taken"*.

That is move 3 of `zymbol-design/HOW_TO_CHANGE_ZYMBOL.md` — *close a finding by checking
what it was not* — and § 5 of the same document names this exact case: *"Sweep the type
range, not a representative … four of the seven cases of the `?`-condition divergence
hide behind an `Int` that behaves."* The draft was written without reading it. Reading it
was the first thing done when the corrections began, and the sweep followed within the
hour.

It is kept here rather than quietly rewritten because it is the third time the same
representative has hidden the same divergence: `DM-05` on 2026-09-04, and this draft on
2026-10-11, both closing it on an Int. Fixed as GLB-121; held by an axis whose first
dimension is the type, so that the next reading of it cannot be a single value.

### E.5 — The CHANGELOG is written from the plan, and that is thinner than v0.0.9's

Still true, and now a property of a file that exists. `[0.0.10]` is 226 lines where
`[0.0.9]` is 922, for a release that is larger: 164 commits in `interpreter/`, 260 in
`ZyDDT/`, 166 in `web/`, 112 in `zyquality/`, 28 in `zymbol-design/`. Each entry names its
finding, and the finding — not the entry — holds the measurement, the alternatives and
the decision. That is a deliberate trade and it has a cost: `ZyDDT/HALLAZGOS/` is written
in Spanish and lives in another repository, so a reader of the CHANGELOG alone gets the
*what* and has to go elsewhere for the *why* that v0.0.9's entries carried inline.
Whether to expand the entries before the tag is the author's call.
