# Zymbol for agentic programming — safeguards and gaps

> **Rank: derived.** Every claim below is a program that was run against
> `zymbol 0.0.9` (2026-09-11 / 2026-09-12) in all three engines — `zytw`,
> `zyvm`, `zyjs` — unless a line says otherwise. It describes what the engines
> do; it does not decide anything.
>
> **§ 12 is re-measured against `zymbol 0.0.10` (2026-10-11); §§ 1–11 are not.**
> Every open row was probed again, over kinds of value and not one representative:
> G1, G2, G4, G5 and G8 stand, G10 was closed that day, and **G3 was wrong as
> written** — the browser engine has a budget, and the Rust engines do not run an
> infinite recursion "until a timeout" but abort the process. The measurement, row
> by row, is in § 12. The sections above it carry their 2026-09 measurement.
>
> **The rules it reports live in `zymbol-design/PREMISES.md`**, by id, and each
> section below names the ones it is measuring. Where this document and a
> premise disagree, this document is what is wrong. It was written the other way
> round on 2026-09-11 — measuring first and stating rules in prose — and six of
> its eleven sections turned out to be re-enunciating premises that already
> existed while five were enunciating rules nobody had declared. Those five are
> now `AGT-1`…`AGT-5`.
>
> Companion documents: `LLM.md` (the language on one page),
> `zymbol-design/PREMISES.md` (the rules), `zymbol-design/MEMORY_MODEL.md`
> (design-vs-implementation audit), `REFERENCE.md` (limits and error taxonomy).

Code written by a model fails differently from code written by a person. A model
does not forget a rule — it never had it; it fills a gap with the most probable
shape from another language. So the properties that matter are not the ones that
make correct code easier to write, they are the ones that make **incorrect code
impossible to write silently**: the failure must be visible at the site where it
is introduced, and the amount of context needed to review one line must be that
one line.

This document states which of those properties Zymbol has today, with the probe
that establishes each one, and — in §3 — which it does not.

---

## 1. Importing cannot execute anything

> Measures: **AGT-1.**

A module body admits only imports, the export block, literal-initialised
bindings and function definitions. Anything that computes is **E013**, raised
before a single statement runs:

```zymbol
// m/malo.zy
# malo {
    #> { f }
    >> "SIDE EFFECT ON IMPORT" ¶      // E013
    f() { <~ 1 }
}
```

```
$ zymbol check s14_import_effect.zy
error: E013: executable statement not allowed in module body
  --> m/malo.zy:3:5
  = help: modules may only contain imports, exports, constants, variables, and function definitions
  = note: reached from s14_import_effect.zy (1:1)
```

This removes the entire class of supply-chain attack that `import` and
`npm install` carry in other ecosystems: in Zymbol there is no install-time or
import-time code. It is the single most valuable property for generated code,
because a model selects dependencies by plausibility of the name.

`check` follows imports and says where it came from (`reached from`), so the
whole graph is audited from the entry file.

## 2. A module declares its surface

> Measures: **AGT-2.**

Omitting `#>` is **E014**, not an implicit "export everything":

```
error: E014: module 'sinexp' does not declare what it exports
  = help: add '#> { … }' inside the module block, naming what it exports —
          an empty '#> { }' says it exports nothing
```

Nothing leaves a module by accident, and the export list is readable without
reading the module.

## 3. The reachable program is enumerable before running it

> Measures: **AGT-3.**

Four independent facts combine into one guarantee:

| fact | probe | result |
|---|---|---|
| no dynamic import | `ruta = "./m/est"` then `<# ruta => Z` | `error: imports must come before any statement` |
| imports are first | any `<#` after a statement | same error |
| sub-script paths are literal | `p = "./x.zy"` then `</ p />` | `Runtime error: file not found: p` — `p` is the path, not a variable |
| no `eval` | — | there is no form that executes a string as code |

Therefore **every line of code a `.zy` can reach is known statically**. A
reviewer — human or model — that reads the entry file and follows `<#` has read
everything that can run. No other mainstream scripting language offers this.

## 4. The effect surface is lexical and finite

> Measures: **AGT-4.**

Every way a program can touch the world outside itself has its own mark or its
own import:

| gate | form |
|---|---|
| filesystem | `<# std/io` |
| network | `<# std/net` (blocking `ureq`, not the shell) |
| database | `<# std/db` |
| clock / entropy | `<# std/time`, `<# std/random` |
| process | `<\ "cmd" \>` |
| sub-script | `</ ./sub.zy />` |
| stdin / argv | `<<`, `><` |

Auditing what a program may do is a `grep` over a closed list, not a
flow-sensitive analysis. Combined with §3, the audit is complete: no gate can be
reached through a name computed at runtime.

Since 2026-10-21 (G1) the audit is a command and the list can be enforced. `zymbol check
--effects file.zy` prints every use of a gate in every file the program can reach, with
file and line. `zymbol run --deny net,shell file.zy` reads that surface **before running
anything** and refuses the program if it uses a denied gate — in the entry, in a module it
imports or in a sub-script it runs — and also if part of it cannot be read, since nothing
can say what an unread file would do. The names are `io`, `net`, `db`, `time`, `random`,
`shell` (`<\ \>`), `sub` (`</ />`), `stdin` (`<<`, `<<|`) and `argv` (`><`). Without
`--deny` nothing is restricted. It is static because §3 makes it possible: one check in
the CLI serves both Rust engines; the browser engine has no command line to ask it from.

## 5. Memory isolation

> Measures: **MEM-2, MEM-3, MEM-6, COL-6.**

Values have **value semantics**. There are no references and no aliasing at the
language level; the only shared-identity structure is module state (§3 of
`MEMORY_MODEL.md`, and gap G7 below). `Rc` + copy-on-write is a performance
mechanism, verified semantically neutral — every write detaches first.

```zymbol
contador = 10
toca(c) {
    >> "inside reads=" c ¶            // 10 — it came in as a parameter
    c = 999                           // stays inside the call
    >> "inside now=" c ¶              // 999
}
toca(contador)
>> "outside=" contador ¶              // 10
```

A named function sees **only its parameters** (MEM-2). A file-level name read
inside one is refused statically — `lee() { <~ contador }` is `error: 'contador'
is read from outside this function`, with `= help: … pass 'contador' as one` —
and so is a name from a block, from another frame or from the importing script.
Constants (`:=`) are the one exception, global by definition (MEM-1). A write
inside a call never escapes it. A lambda is light (MEM-6): it reads the scope it
is written in — the file at file level, its function inside one — and writes
only the names it declares. There is no door on either side; G9 below records
the one that used to be open.

Module state is the deliberate exception to value semantics, and it is fenced: a
module's mutable bindings **cannot be exported**. `#> { n }` where `n` is a
variable is `E005: Item 'n' not found in module` — only constants and functions
leave. A module's state is therefore reachable only by calling one of that
module's own functions, and identity is the file path, so every alias and every
importer shares that one environment. It is a closed environment with a single
door, not a global.

## 6. Mutation is declared at both ends

> Measures: **MEM-5.**

The strongest safeguard in the language, and the rarest: an output parameter is
written at the **signature and at every call site**, and each half is an error
without the other.

```zymbol
bump(b<~) { b = b + 100 }
bump(y<~)                      // required — 'bump(y)' does not compile
```

```
$ zymbol check s4b.zy          // signature marked, call site not
error: argument 1 of 'bump' is an output parameter and must be marked '<~' at the call site
  = help: write the argument as 'name<~' — the mark says the value comes back
          changed, so a reader does not have to open the function to find out

$ zymbol check s4c.zy          // call site marked, signature not
error: argument 1 of 'bump' is marked '<~' but the function does not declare it as an output parameter
```

That help text is the whole principle: **the cost of reviewing a call is the
call**. A model proposing a diff cannot hide a write behind a function name, and
a reviewer skimming a hundred generated lines sees every outbound mutation
without opening anything. `~` is the other half — an explicit working copy the
caller never sees. Unmarked is by value, always.

## 7. Scope, lifetime and destruction

> Measures: **MEM-6**, and auto-free, which is not a premise of anyone yet.

```zymbol
_tmp = 1          // exact block scope: invisible to inner AND outer blocks
\ secreto         // explicit destruction; use afterwards is an error
```

```
$ zymbol check s7_underscore.zy
error: cannot access underscore variable '_t' from outer scope
  = help: underscore variables are strictly local to their declaration block

$ zymbol run s3_destroy.zy
Runtime error: use after destruction: variable 'secreto' was destroyed after its last use
  --> s3_destroy.zy:4
```

Auto-free releases a value after its last use and is **required to be invisible**
— the analysis in `crates/zymbol-semantic/src/last_use.rs` is lexical and
conservative, and anything dubious (hot definitions, constants, `_` names,
captured free variables, every module-level binding) is poisoned and never
freed. Destruction is therefore either automatic and unobservable, or manual and
explicit; there is no third state.

Constants are enforced statically:

```
error: cannot reassign constant 'PI'
  = help: constants declared with ':=' cannot be modified
```

## 8. Failures are loud where models are wrong

> Measures: **COL-5** for the absent key; the rest is measurement with no premise behind it.

The places where a generated program usually goes wrong *quietly* all abort or
warn:

| hazard | Zymbol |
|---|---|
| `"5" == 5` | `#0` — `==` never coerces |
| integer overflow | `Runtime error: integer overflow` — catchable `##Range`, never a wrap, never a silent promotion to Float |
| wrong argument count | static: `function 'f' expects 2 argument(s), but 1 were provided`, with the expected signature |
| `??` with no matching arm | `Runtime error: no pattern matched in match expression` — never an empty value |
| a loop specifier that is neither Int nor Bool | runtime error in all three engines |
| a variable that is never read | `warning: unused variable` |
| `m[1][2]` | `error: chained index does not exist: 'm[…][…]' is not a form of Zymbol` |

## 9. One spelling per operation

> Measures: **COL-7** and **SYM-1**.

`a[i]$~ v` is the only update form, in every collection — `a[i] = v` does not
exist, and chained reading was retired in v0.0.9 (the diagnostic above names the
replacement). Every alternative spelling removed is generation space closed: a
model cannot reach for a remembered form from another language, because the
second form does not parse.

Related: **Zymbol has no keywords in any human language**. There is no `if`,
`for` or `return` to write out of habit, and no reserved identifier in any
script — so lexical transfer from the model's training distribution fails
immediately and visibly rather than subtly.

## 10. Three engines are an executable specification

> Measures: no premise — this is method, and `ZyDDT/CHARTER.md` owns it.

`zytw`, `zyvm` and `zyjs` are graded on the same corpus by `zyquality/`: **666
corpus files and 41 forms every engine must refuse**. Behaviour that is merely
*unspecified* — the gap where a model infers a rule that was never true — shows
up as an engine divergence rather than as a surprise in production. The rules
are in `zyquality/GOVERNANCE.md`; an exclusion requires a written reason.

Note the limit: the gate compares what the corpus covers. Gap G10 below was
found by probing outside it.

## 11. Packages are source, never a binary

> Measures: **AGT-5.**

A `.zyp` is a ZIP of `.zy` files plus its manifest — readable, diffable,
auditable. `zymbol run pkg.zyp` extracts to an ephemeral directory and **never
`chdir`s**, so code is disposable while data a script writes lands in the user's
real working directory.

---

## 12. What the language does not guarantee

Four gaps **open** (`G3`–`G5` and `G8`), three closed (`G10` on 2026-10-11, `G1` on
2026-10-21, `G2` on 2026-10-22), one withdrawn and two superseded by premises. `G3` is not an implementation gap at all — it is
a design decision nobody has taken, and should not be
read as accepted. `G1` and `G2` were the other two: `G1` was the reason `AGT-4` claimed
auditability and not enforceability, and it was decided and implemented on 2026-10-21
(`--deny`, §4); `G2`, a value pasted into a shell command, the next day. Each is a decision — implement, deprecate, or dismiss with a
written reason — and a dismissal with its reason is worth as much here as an
implementation, because a rule whose reason has been deleted is
indistinguishable from one nobody can justify.

| # | Gap | Status |
|---|---|---|
| G1 | **No capability control.** `std/net`, `std/io`, `std/db` and `<\ \>` are unrestricted. `zymbol run` offers no `--deny-net`/`--deny-io`, and `zyp.toml` does not declare required effects. §4 makes the surface *auditable*; nothing makes it *enforceable*. *Re-measured 2026-10-11 on 0.0.10, all three engines:* `io::write`, `io::read`, `io::delete`, `<\ "echo …" \>` and `net::head` all run unrestricted; `zymbol run --help` has no capability flag; a `zyp.toml` has `name`, `version`, `engine`, `mode`, `desc` and its `[[script]]` entries, and no effects. **Closed 2026-10-21** — decided by the author, option A of four (B: also declare effects in `zyp.toml`; C: deny by default; D: dismiss, keeping AGT-4 at "auditable"), and implemented the same day: `zymbol run --deny <effects>` refuses a program **before running any of it** when anything it can reach uses a denied gate, naming file and line, or when part of it cannot be read; `zymbol check --effects` lists the surface (§4). Measured on a program whose network use is in an imported module and whose shell use is in a sub-script, both engines and a `.zyp`: refused with exit 1, and the file its first line writes is never written. Held by `crates/zymbol-cli/tests/cli_deny.rs` (7 tests, run by the Gate's `cargo test`) and the `effects` tests of `zymbol-package`. Not covered: the browser engine, which has no command line; and declaring effects in a package, option B, left for later. | closed |
| G2 | **Shell injection.** `<\ "{cmd}" \>` interpolates without quoting or escaping: a string built at runtime becomes a command. *Re-measured 2026-10-11 over eight kinds of value* interpolated into `<\ "echo [{v}]" \>` (`zytw` and `zyvm` agree on all; `zyjs` refuses every shell command): a `String` holding `$(echo PWNED)` is executed, and a `Char` `;` splits the command (`##IO(exit 127: …)`); an `Int`, a `Float`, a Bool (`#1`) and a `String` with a space are inert; an Array, a Tuple and a Dictionary reach the shell as `[1, 2]`, `(1, 2)` and `#(a: 1)` — a glob or a syntax error. New since 0.0.9: a failing command is a soft `##IO` that carries its exit status, so the *breakage* is loud; the *injection* is not. **Decided 2026-10-21 by the author, option A of four** (B: an opt-in quote; C: a second, shell-less form; D: dismiss, `--deny shell` being the control): what is written in `<\ \>` is the command, and every value placed in it is data — exactly one shell word, quoted; a collection, one word per element. The 22 uses that quoted a value by hand (ZyAudit 17, ZethyCLI 3, the corpus 2) are migrated with it. **Closed 2026-10-22**, implemented in both Rust engines: the text written in the command — a string literal, the text of an interpolated one, a char literal — goes to the shell as it is; every value, side by side or as `{v}`, is quoted by `zymbol_common::shell::quote_word` (POSIX `'…'`; through `cmd.exe`, a value holding `"`, `%` or a line break is a soft `##IO` and nothing runs). Measured: `"x; echo INYECTADO"` is printed, not run; `it's a $(id).txt` reaches the command as itself; a collection is one word per element; `""` is one empty word. Held by five `environment/shell-*` cells with an oracle — four of them WRONG ANSWER against the binary before, both engines agreeing on the wrong output. Not covered: the browser engine, which runs no shell; and argument injection — a value starting with `-` is still read as an option by the program it reaches, which quoting cannot change. | closed |
| G3 | **No termination bound** — in the Rust engines. An infinite loop (`@ { }`) and a tail recursion (`f(n) { <~ f(n + 1) }`) run until an external timeout. A non-tail recursion (`<~ 1 + f(n + 1)`) does not: since v0.0.11 the call that would make more than 10 000 calls active at once is a `##Depth` error, in the three engines, that `!?` catches (D7, GLB-133). Until then `zytw` aborted the process when its stack was gone (`fatal runtime error: stack overflow`) and `zyvm` grew its value stack until an allocation failed. A generated program that does not terminate hangs, or kills, whatever runs it. **The browser engine does have a budget, and this row said it did not:** as the playground runs it, `@ { }` stops after 100 000 iterations and a recursion after 2 000 000 steps, each with a `Runtime error`; `web/tests/run_one.mjs` lifts both limits so that `zyq` grades it on the same programs as the Rust engines, which is why a probe taken through it saw none. *Re-measured 2026-10-11.* | open |
| G4 | **Use-after-destruction is runtime-only.** `zymbol check` reports "No errors or warnings" on a program that `\`-destroys a variable and then reads it, at file level and inside a function alike. | open |
| G5 | **Non-exhaustive `??` is runtime-only.** No static check that the arms cover the subject. *Re-measured 2026-10-11 over six kinds of subject* (Int, Float, String, Char, Bool, Array) with arms that do not cover it: `zymbol check` says *No errors or warnings* for every one, and all three engines stop at run time with the same `Runtime error: no pattern matched in match expression`, at the line of the statement. An arm of another kind than the subject (a Tuple or a Dictionary against an Int arm) is a type error `check` does refuse — `zyjs` only at run time. An exhaustive Bool (`#0` and `#1`) runs. | open |
| ~~G6~~ | **Superseded 2026-09-12 by `MEM-7`.** Two of the four forms I listed were the model working, not defects: a block assignment reaching its container's name is *one* name and no shadowing (`MEM-6`), and a parameter reusing a file-level name is two different strong environments. The two that remain — `f(a, a)`, which answers 2/1/2 depending on the engine, and a silent redefinition — are declared debt with four red cells. | superseded |
| ~~G7~~ | ~~Module state is shared mutable global memory.~~ **Withdrawn 2026-09-12** — measured, not a gap: it is the intended design (a module owns its environment), and it is fenced. A module's mutable bindings cannot be exported (`E005`), so the state is reachable only through that module's own functions. Identity by file path means one environment per module, which is what "self-contained" requires. The number is not reused. | withdrawn |
| G8 | **No reproducibility.** `std/random` seeds from the system clock into a thread-local cell, with no seed object exposed to Zymbol. Applications that need a reproducible run (zy-GO's benchmark) implement their own LCG threaded through a `<~` parameter. *Re-measured 2026-10-11:* three draws of `random::entero(1, 1000000)` differ between two runs of the same program in every engine and between engines; `std/random` exports `entero`, `rango` and `peso_f64`, and no seed; no engine reads an environment variable that fixes one. | open |
| ~~G9~~ | **Superseded 2026-09-12 by `MEM-2`**, decided and held by 13 cells, four of them red then; implemented 2026-09-13, and all of them green since. Original text: a named function reads the file's top-level names it does not declare — by value, at call time, and only names declared lexically before it. `check` says nothing. Harmless as a mutation channel (the copy is one-way), but it makes a function non-relocatable: the same body stops compiling once moved into a module, which is what a refactor does. | closed 2026-09-13 |
| ~~G10~~ | **Closed 2026-10-11 by `GLB-121`** — decided by the author and implemented the same day: a condition is a Bool and nothing else, refused statically where the type is known and a `##Type` at run time where it is not. Held by `axes/condition.toml` (73 cells). Original text: `?` with a non-Bool condition diverges across all three engines, with only a warning. The measurement stays below, widened to thirteen kinds of value. | closed |

### G10 in detail — closed 2026-10-11

> **This section is history.** The gap stood for a month with no finding and no
> cell — `zyddt findings` reported nothing open the whole time — and was decided
> on 2026-10-11 (`GLB-121`): there is no truthiness, in `?` as everywhere else.
> All three engines now refuse every row of the table below that is not a Bool.
> Re-measured that day over thirteen kinds of value before the fix: nine
> diverged, not four of seven.
>
> One more turn of the mechanism this section describes: the v0.0.10 release
> plan, drafted that same day, probed `x = 5`, saw the three engines agree, and
> recorded that the divergence no longer existed. The sweep of types undid it
> an hour later. That is `DM-05` again, exactly.

`? x { … } _ { … }` with `x` not a Bool. The loop specifier rule ("no
truthiness") is enforced; the `?` condition is not, and the three engines
disagree:

| `x` | `zytw` | `zyvm` | `zyjs` |
|---|---|---|---|
| `[1,2]` | NO | **SI** | **SI** (no warning) |
| `[]` | NO | **SI** | **SI** (no warning) |
| `""` | NO | **SI** | NO |
| `3.5` | NO | **SI** | **SI** |
| `"texto"` | SI | SI | — |
| `1` | SI | SI | SI |
| `0` | NO | NO | NO |

Four of seven cases diverge. `zyvm` appears to treat everything non-zero as
true; `zytw` applies per-type rules; `zyjs` warns for scalars but not for
arrays. All three exit 0. A generated `? arr { … }` therefore behaves
differently depending on which engine runs it, with no error anywhere.

**This is the third surface of a rule already decided twice.** "There is no
truthiness" was settled for the loop specifier in v0.0.9, and again for the
logical operators on 2026-08-30 (ZyDDT's `ZYVM-001`, 40 of 252 cells: *"error in
all three engines"*, the VM's `And`/`Or` changed to demand `Bool`). The `?`
condition is the surface that was never closed.

Note also how it survived: `DM-05` in `Divergente_ES/INDICE.md` covers *an `Int`
as a `?` condition* and is marked closed on 2026-09-04 — **"the three warn"**.
They do warn, identically, and with an `Int` they also agree on which branch
runs. The check was on the diagnostic, not on the branch taken, and with an
array, a Float or an empty string the branch diverges under the same warning.
Nothing in the corpus writes `? arr {`, so `zyq consensus` never asked.

---

## 13. Operating checklist

```bash
zymbol check file.zy      # parse + semantics, follows imports, reports where from
zymbol check --effects file.zy   # every gate it can reach, with file and line (§4)
zymbol run --deny net,shell file.zy   # refused before running if it reaches one
zymbol fmt file.zy --write
zymbol run file.zy        # register VM (the default)
zymbol run --tw file.zy   # tree-walker — run both; a difference is a bug in one of them
cd zyquality && ./zyq suite    # the whole gate, one verdict
```

Reviewing generated Zymbol, in order: the `<#` lines say what can run (§3);
`check --effects` says what it can touch, and `--deny` holds it to that (§4); the `<~` marks say what
it changes (§6); `check` says whether it is well-formed. Nothing in §12 is
covered by any of those four steps.
