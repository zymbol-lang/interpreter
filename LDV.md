# Language-Driven Validation (LDV)

> **What this document is.** The method by which Zymbol is validated: what an LDV project is,
> what counts as a failure, what has to happen before the cycle closes, and where the record
> of every finding lives.
>
> **What it is not.** Not the test suite (`README.md` § Testing, `zyquality/GOVERNANCE.md`,
> `ZyDDT/CHARTER.md`), which is the *verification* layer this method feeds — and which the
> applications then join, rather than leaving (§ 1). Not a list of the projects — the
> `README.md` § Language-Driven Validation table is that, and § 5 here indexes their logs.
>
> **Method.** Every claim below was checked against the eight gap logs in the application
> repositories and against the harness that runs them — `zyquality/suites.toml`,
> `zyquality/project/apps.toml`, `ZyFmtCheck/`, `ZyDDT/` — not against a previous edition of
> this document. Where the practice has drifted from what it says about itself, § 5.2 says so
> instead of tidying it away; where a previous edition was simply wrong about the
> applications' life after the release, § 1 says that instead of quietly restating it.
>
> **Authorship.** The LDV applications, like the interpreter itself, are written with
> **[Claude Code](https://claude.ai/code)** (Anthropic) as the engineering team, under the
> author's direction. § 7 says what that changes about the method and what it does not.

---

## Table of contents

1. [Validation is not verification](#1-validation-is-not-verification)
2. [LDV against TDD](#2-ldv-against-tdd)
3. [The decalogue](#3-the-decalogue)
4. [The cycle](#4-the-cycle)
5. [The gap logs](#5-the-gap-logs)
6. [What LDV costs, and when not to use it](#6-what-ldv-costs-and-when-not-to-use-it)
7. [Authorship & AI collaboration](#7-authorship--ai-collaboration)

---

## 1. Validation is not verification

The two words are used interchangeably in casual speech and mean different things in
engineering. Verification asks *was the product built right* — does the implementation match
its specification. Validation asks *was the right product built* — does the thing, correct or
not, actually serve the need it exists for.

Zymbol's automated suites are verification, all of them:

| Suite | Verifies |
|-------|----------|
| `cargo test` | each crate against its own contracts |
| `zyq consensus` (`vm_compare.sh`, `engine_compare.sh`) | that the engines agree with each other |
| `zyq expect` (`expected_compare.sh`) | that output matches a recorded golden |
| `zyq reject` | that malformed programs are refused |
| `zyq suite --only fmt` (`fmt_property.sh`) | that formatting is reparseable, idempotent and meaning-preserving |
| `zyq suite --only messages` | that no engine defines a diagnostic another does not |
| `zyfmtcheck` (`ZyFmtCheck/`) | that the formatter moved things and changed nothing |
| `zyddt suite` (`ZyDDT/`) | a declared axis, covered exhaustively rather than sampled |

Every one of them takes a program that already exists and asks whether the implementation
handles it correctly. **None of them can tell you that the language cannot express a Go
board.** No suite reports a construct nobody has written yet; a missing capability produces no
failing test, because there is no test — that is what "missing" means.

That is the gap LDV fills, and the reason the method is named for validation rather than for
testing. The unit under test is the language: its syntax, its semantics, its standard library
and its tooling. The instrument is a complete application built in it.

### The instrument does not stop when the release ships

Everything above is about what a *new* application discovers. It says nothing about that
application's life afterwards, and earlier editions of this document conflated the two — they
called an LDV project un-rerunnable, said it would be superseded, and said nothing in CI ran
one *and nothing should*. None of that survived contact with the workspace. Once built, an LDV
application becomes part of the verification layer it was defined against:

- `zyquality/project/apps.toml` registers seven of the eight, and `zyquality/suites.toml`
  declares the `project` suite with **`gate = true`**. It runs ~40 goldens through both
  engines, and a red there fails `zyq suite`. Only ZethyCLI is absent, because it never grew a
  suite of its own.
- **`ZyFmtCheck`'s default body is the applications**, not the corpus: `./bin/zyfmtcheck` with
  no arguments formats them in a copy, compares code and comments, and re-runs their suites
  from that copy. Its README says why the corpus could not do this job — *"they are the richest
  Zymbol there is: real nesting, hand-aligned tables, comments in awkward places, five writing
  systems, modules importing each other. A formatter's damage lives in exactly what a short
  file does not have."* It found five formatter defects on its first run, every one of them
  invisible to the token gate and to P1–P4.
- `ZyDDT` graded the three engines over the applications *after* its own 394 declared cells,
  the 661-file corpus and 222 example programs were all green, and opened **nine further
  findings** (§ 3, point 12). They were not reachable from a corpus, and the reason is
  structural rather than accidental.

So an application is two things, and the second is permanent: the microscope that found the
defect, and afterwards a body of real Zymbol that every later change is measured against.
`apps.toml` states the difference from the corpus in one line — *"a test case failing says a
feature broke, an application failing says something people rely on broke."*

---

## 2. LDV against TDD

LDV borrows TDD's shape — a red state, a green state, and a discipline that forbids skipping
from one to the other. It differs in every quantity that matters, and the differences are the
reason it is not called Test-Driven Language Development.

| | TDD | LDV |
|---|---|---|
| **Unit under test** | a function, a module | the language: syntax, semantics, `std/`, tooling |
| **The test** | an assertion written before the code | an application written *as if the language already supported it* |
| **Red** | an assertion fails | the language cannot express the concept — or expresses it and returns a silently wrong answer |
| **Green** | the code changes | the *language* changes: an operator derived under `SYMBOLS.md` § 17, a semantic fix, a TW/VM divergence closed, or a `std/` module |
| **Cycle time** | seconds | a release to discover; a commit to re-run |
| **Cost of the test** | cheap; rerun on every save | expensive to build — months of work — then re-run on every commit like any other suite. What cannot be rerun is the *discovery* |
| **What it finds** | known-unknowns — the case you thought of when you wrote the assertion | unknown-unknowns — the case nobody could think of until a real domain forced it |
| **Primary artifact** | the passing suite | the gap log |
| **Regression protection** | the test itself is the protection | first a separate cheap layer distilled from the finding (§ 4, Refactor); then the application itself, kept in the gate as a second and coarser body (§ 1) |
| **After it passes** | delete it or keep it; either costs nothing | maintained across every breaking change — and the migration finds more (§ 3, point 12) |

The two are not rivals. TDD is how the interpreter's crates are written; LDV is how it is
discovered *what to write*. The output of an LDV cycle is, among other things, more TDD tests.

---

## 3. The decalogue

**1 — The language is the product.** The unit under test is not an application module. It is
the language itself: syntax, semantics, standard library, and the tooling a user actually
holds (`check`, `fmt`, the LSP, the two engines).

**2 — Validation first; verification as a by-product.** The goal is not to prove the code
correct — that is what the suites do. It is to prove the language *viable and expressive* for
a domain. But an application composes features that a per-feature suite exercises one at a
time, so LDV also verifies the implementation exactly where the suites cannot reach: in the
intersections. That is where it finds the worst class of defect — a silent wrong answer rather
than a crash — and it is why the logs carry two categories, `GAP` for the incapacity and `BUG`
for the wrong answer, instead of pretending every finding is one kind.

**3 — The test is an application.** One non-trivial program, written entirely in the language,
in a domain that was not chosen to be easy. Not a demo, not a benchmark, not a feature tour: a
thing somebody would want to use.

**4 — The red state is an incapacity.** The test fails not with an assertion error but when the
language cannot say what the program means, or says it and answers wrongly. A workaround
counts as red: if the program can only express the concept by leaving the language (a shell
call, a hand-written table, a duplicated block), the language did not support it.

**5 — The green state is a language change.** The cycle does not close because the application
works. It closes when the interpreter, the compiler, the standard library or the tooling has
changed to remove the incapacity — or the finding is explicitly rejected, with the reason
recorded.

**6 — Findings over features.** The project's primary artifact is not the program. It is the
gap log: every friction, bug, missing capability and idea, each with an ID, a type, a
reproduction and a status (§ 5). A finished application with no log has validated nothing,
because nothing is left that anyone else can act on.

**7 — Regressions belong to a different layer first.** Each finding is distilled into a
minimal, fast, automated case in the corpus and the unit suites, and a finding is not closed
until it is there. That layer is the first line and it is the one that *names* what broke: the
application answers "something changed in 囲碁", the minimal case answers "the
interpolated-string arm is gone", and only the second is a diagnosis. The rule this point
exists to enforce is that no finding may live **only** in the application — not that the
application stops being run. It does not stop being run (§ 1, point 12).

**8 — Validation is expensive, verification is cheap.** An LDV project costs weeks of work,
and the *discovery* cannot be rerun — nobody finds the same unknown-unknown twice. Once one is
found, the knowledge has to be moved into something that costs milliseconds, or it will be
lost the next time somebody refactors the thing that caused it. The application, once written,
is cheap to re-run and is re-run; what stays expensive is building the next one.

**9 — Culture is a test case.** Writing each project in a different natural language —
English, Mandarin, Spanish, Klingon pIqaD, Japanese — is a deliberate validation of the
wordless-grammar claim, of Unicode handling end to end, and of application-level i18n. It is what
turned "language-neutral" from a design intention into a result, and it is how the
double-width glyph, pIqaD interpolation and numeral-mode defects were found at all.

**10 — The artifact is a design document.** The project's code and prose are a proof of
concept: they show future users what the language can do, and they tell the roadmap what to do
next. `USERAPPI18N.md` is the clearest case — it is 囲碁's i18n architecture, written up after
the fact as doctrine.

**11 — The log closes against a release.** Every finding carries a status, and a version is not
called done while a finding is open and unaccounted for — fixed, deferred with a reason, or
rejected with a reason. 囲碁's eleven findings were all closed in v0.0.8, each with its own
regression test. This is the point where the method stops being a story about having written a
big program: an open-ended list of complaints validates nothing, and a rule that nothing
enforces is not a rule.

**12 — The application is maintained, not archived, and it keeps finding things.** A language
change is not validated until the applications have been carried across it, and the carrying is
a validation act in its own right: it exercises the change under real load, it walks the
migration path a user will walk, and it finds what the original cycle could not. Nineteen
migration commits across the eight projects say so, in four campaigns — v0.0.4's closed-block
modules, v0.0.5's `<=` → `:` and then `=>`, the v0.0.7 standard library, and four separate
v0.0.9 changes, of which the dictionary's `#(…)` notation touched four applications in a single
day.

The measured case is ZyDDT's. With its 394 declared cells green, the 661-file corpus green and
222 example programs green, running the same engines over the **LDV applications** opened
**nine further findings** — `ZYJS-007`, `ZYJS-009`–`011` and `GLB-001`–`005`, all since fixed.
Its own index states why, and it is this document's § 1 in two lines: *"a complete application
finds what no corpus can, because a corpus is written one file at a time and these defects need
two parts of a program to get in each other's way."* `ZYJS-007` is the clean example — `zyjs`
continued an identifier with ASCII digits only, so चतुरङ्गम्'s `कार्यस्थितिः२` parsed wrongly in the
browser and correctly under both Rust engines. **No file in the corpus is named that way**, and
no file was going to be.

So an application has a third use beyond discovery and regression: it is the reference body
against which anything *new* is graded — a fourth engine, a highlighter, a formatter, a
grammar. An instrument left to rot is a museum piece. None of the eight has been left to rot,
ZethyCLI included: a v0.0.3 project still being carried forward six releases later.

---

## 4. The cycle

```text
   choose a domain the language has never been asked to serve
                          │
                          ▼
   ┌──── RED ─────────────────────────────────────────────┐
   │  write the application as if the language supported  │
   │  it. Every incapacity, workaround or silently wrong  │
   │  answer is a finding: ID, repro, type, status.       │
   └──────────────────────┬───────────────────────────────┘
                          ▼
   ┌──── GREEN ───────────────────────────────────────────┐
   │  change the language. Derive the operator (SYMBOLS   │
   │  §17), fix the semantics, close the TW/VM gap, or    │
   │  add the std/ module — or reject the finding, with   │
   │  the reason written down.                            │
   └──────────────────────┬───────────────────────────────┘
                          ▼
   ┌──── REFACTOR ────────────────────────────────────────┐
   │  distil into the cheap layer: a minimal .zy in the   │
   │  corpus with its golden, a cargo test, a line in     │
   │  GUIDE/REFERENCE. Then close the finding.            │
   └──────────────────────┬───────────────────────────────┘
                          ▼
              the release ships with the log closed
                          │
                          ▼
   ┌──── MIGRATE ─────────────────────────────────────────┐
   │  the application enters the gate and stays there.    │
   │  When the language moves again, carry it across the  │
   │  change — and log what the migration itself finds.   │
   └──────────────────────┬───────────────────────────────┘
                          │
                          └──► back to GREEN, carrying a finding
                               nobody set out to look for
```

The first three steps are the cycle that discovers. The fourth is the one that keeps: it runs
for the rest of the project's life, on every commit, and it is why the eight applications are
not eight finished artefacts but eight live ones.

The refactor step is not optional bookkeeping — it is the part that turns a symptom into a
diagnosis. The corpus entry will be run by every engine on every commit for the rest of the
project, and it is what makes "囲碁 went red" mean something specific. It does not *replace*
the application: no LDV project has been superseded, and the migrate step is the reason.

**Why the intersections matter.** The three worst v0.0.8 findings are the argument for
building an application rather than extending the suite: under `--vm`, output parameters of
*module* functions were dropped, `String` was truncated *inside a module*, and `"{CONST}"`
interpolation compiled to literal text *inside a function*. Each needs two features composed
before it appears, each returns a wrong answer in silence, and none was reachable by testing
modules, output parameters or interpolation on their own. A Go board is state threaded through
cooperating modules; there was nowhere else for it to live, so it hit all three.

---

## 5. The gap logs

### 5.1 The index

Eight published projects, eight logs, ~5,800 lines of recorded findings. This table is the only
index of them that exists.

| Project | Version | Log | ID scheme |
|---------|---------|-----|-----------|
| [ZethyCLI](https://github.com/zymbol-lang/zy-ZethyCLI) | v0.0.3 | `GAPS.md` | `G1`–`G21`, classed CRITICAL / SIGNIFICANT / MINOR |
| [ZyAudit](https://github.com/zymbol-lang/zy-ZyAudit) | v0.0.4 | `HALLAZGOS_ES.md` | `BUG-NNN` / `GAP-NNN` / `ERROR-NNN` / `IDEA-NNN` |
| [Serpiente](https://github.com/zymbol-lang/zy-Serpiente) | v0.0.5 | `HALLAZGOS_ES.md` | same, plus `HLZ-SRP-001` |
| [Hov veS](https://github.com/zymbol-lang/zyKlingonGalaxy) | v0.0.5 | `hallazgos_es.md` | `HLZ-NNN` and `HLZ-KL-NNN` |
| [Zofía](https://github.com/zymbol-lang/zy-Zofia) | v0.0.6 | `HALLAZGOS.md` | `BUG-ZNNN` / `GAP-ZNNN` / `IDEA-ZNNN` |
| [囲碁 (Igo)](https://github.com/zymbol-lang/zy-GO) | v0.0.8 | `HALLAZGOS_ES.md` | `HLZ-NNN` with a type column, plus `IDEA-NNN` |
| [चतुरङ्गम् (Chaturanga)](https://github.com/zymbol-lang/zyChaturanga) | v0.0.9 | `HALLAZGOS_ES.md` | `HLZ-CHA-NNN` / `IDEA-CHA-NNN` — scoped from the first entry |
| [ZyBank](https://github.com/zymbol-lang/ZyBank) | v0.0.9 | `HALLAZGOS.md` | `BUG-ZYB-NNN` / `GAP-ZYB-NNN` / `ERROR-ZYB-NNN` / `IDEA-ZYB-NNN` — the canonical form entire |

The substance is consistent across all eight: a reading guide, findings with a reproduction and
a status, and a resolution history. Several logs cross-reference each other — Zofía opens with
the lessons carried over from Serpiente, Serpiente's `HLZ-SRP-001` is discussed next to 囲碁's
`HLZ-008` — which is the method working: a finding in one domain is worth stating in the
vocabulary of the next.

### 5.2 Where the form has drifted

Recorded rather than quietly corrected, because the drift is real and its cost is real.

- **Four file names for one artifact:** `GAPS.md`, `HALLAZGOS.md`, `HALLAZGOS_ES.md`,
  `hallazgos_es.md`. Decalogue point 6 names the log `HALLAZGOS.md`; that is literally true of
  **two logs in eight** — Zofía's, which arrived at the name on its own, and ZyBank's, which
  is the first to adopt it deliberately.
- **Three ID schemes**, and `TYPE-NNN` is now the plurality: `Gn` (1 project), `TYPE-NNN`
  (4 — Zofía, ZyAudit, Serpiente and ZyBank), `HLZ-NNN` (3 — of which चतुरङ्गम् is the only one
  scoped by project throughout).
- **A live collision.** 囲碁's `HLZ-001`–`HLZ-011` and Hov veS's `HLZ-001`–`HLZ-003` are
  different findings sharing the same identifiers. A bare `HLZ-002` is ambiguous across the
  two logs, and both logs cite IDs in prose. Hov veS already invented the fix mid-log, when it
  started writing `HLZ-KL-001`.

**Canonical form for a new project**, which is what the majority convention becomes once the
collision is taken seriously: a file named `HALLAZGOS.md`, sections `BUG` / `GAP` / `ERROR` /
`IDEA`, a summary table at the top with ID, module, context and status, and identifiers
**scoped by project** — `BUG-GO-001`, not `BUG-001`. Renaming the six older logs is not
free: they link to each other by path and cite each other by ID, so it is a coordinated change
across six repositories rather than six independent commits, and it is deferred rather than
pretended away.

चतुरङ्गम् is the first log written against this form rather than retrofitted to it: every
identifier is scoped (`HLZ-CHA-001`) from the first entry, which is the half of the convention
that costs nothing to adopt at the start and a coordinated rename afterwards. It kept the
`HALLAZGOS_ES.md` name of the three logs before it rather than the `HALLAZGOS.md` the decalogue
asks for.

**ZyBank is the first to adopt the whole form**: the file name the decalogue asks for, the four
`BUG` / `GAP` / `ERROR` / `IDEA` sections, a summary table with ID, module, context and status,
and identifiers scoped from the first entry (`BUG-ZYB-001`). So the convention now exists in
full somewhere, which is what makes the pending rename of the six older logs a mechanical job
rather than a design question. The file-name drift is six against two.

---

## 6. What LDV costs, and when not to use it

The honest limits, so the method is not applied where it does not pay:

- **It cannot be scheduled.** You cannot plan to find three silent VM bugs. You can only plan
  to build something big enough that they surface.
- **The costs below are paid with AI assistance** (§ 7), which is what makes eight projects
  possible rather than one. It changes the price of the instrument, not the nature of what the
  instrument finds — and it introduces one failure mode of its own, § 7.3.
- **It has poor resolution.** A failure points at a region, not a line. Reducing an application
  failure to a minimal `.zy` is real work, and it is the work that produces the value.
- **It scales with domain distance, not with size.** A third terminal game would validate
  almost nothing. The projects that paid were the ones that moved: a CLI over an HTTP service,
  then a TUI, then scientific computing, then a persistent 361-point data structure threaded
  through modules with recursive traversal. Choosing a domain the language has already served
  is the one reliable way to run the method and learn nothing.
- **It is not the *first* alarm, and it has poor resolution as one.** The application is the
  microscope; the cheap layer is the alarm. A finding that lives only in the application is
  not protected, and that is decalogue point 7 — it stands. What does not stand is the
  absolute this document used to state. Seven of the eight applications *are* in a gate
  (`zyquality/project`, `gate = true`), and `ZyFmtCheck`'s default body is the applications
  rather than the corpus. They are a second alarm, coarser and slower, and coarse is not the
  same as absent.
- **The instrument has a maintenance bill, and it recurs.** An application in the gate must be
  carried across every breaking change to the language, in every project, before the release
  can close — nineteen migration commits so far, and the figure grows with each project added.
  It is worth paying, and point 12 says why, but it is the reason a ninth project is a larger
  decision than the first was.

---

## 7. Authorship & AI collaboration

Zymbol is designed by
**[OscarE.EspinozaB](https://github.com/zymbol-lang/interpreter/commits?author=OscarEEspinozaB)**.
Every decision about the language originates from and is controlled by its author. The LDV
applications — ZethyCLI, ZyAudit, Serpiente, Hov veS, Zofía, 囲碁, चतुरङ्गम्, ZyBank — are
written with **[Claude Code](https://claude.ai/code)** (Anthropic) as the engineering team,
under the author's direction, exactly as the interpreter is (`README.md` § Authorship & AI
Collaboration). The use of AI is transparent and intentional — it is not concealed or
minimized.

Stating it here rather than only in the README matters, because LDV makes quantitative claims
about cost, and those claims are only readable if the reader knows who is paying.

### 7.1 What it changes

**The cost of the instrument, and therefore how many exist.** § 2 records the cost of an LDV
test as *"expensive to build — months of work"*, and § 6 opens with *"it cannot be
scheduled"*. Both remain true of the **discovery** — a finding still closes against a release,
and no amount of assistance schedules an unknown-unknown.
What changed is the price of the *application*: eight of them exist across v0.0.3–v0.0.9, in
five natural languages and five unrelated domains. Without AI assistance that number would be
one or two, and § 6's warning that *"it scales with domain distance, not with size"* would be
an untested principle instead of a measured result — a third terminal game validates almost
nothing, and knowing that required building enough of them to see it.

### 7.2 What it does not change

**An unknown-unknown is unknown to the AI too.** LDV works because a real domain *resists* —
a 361-point board threaded through cooperating modules made three silent VM defects surface
that no amount of cleverness would have predicted. The method's value was never that someone
thought hard enough about where bugs might be; it is that a genuine application goes where
nobody was looking. Assistance lowers the cost of building the instrument. It does not lower
the cost of knowing where to point it, and it does not substitute for a domain that pushes
back.

**Nor does it change what stays with the author**: the design rationale, the specification
that guides each feature, the suite that defines correctness, the judgment on what to build
and what to reject, and the final say on every merged change.

### 7.3 The risk it introduces, named

A fluent, fast implementer routes around an incapacity **smoothly**. Asked to write something
the language cannot say, the path of least resistance is a workaround — a shell call, a
hand-written table, a duplicated block — delivered without complaint, and the finding is never
recorded. The application still works, so nothing looks wrong. That is the failure mode of
LDV under assistance, and it is silent.

Decalogue point 4 already anticipates it:

> **A workaround counts as red:** if the program can only express the concept by leaving the
> language, the language did not support it.

That rule was written as a definition. Under AI assistance it has to be read as an
**obligation on the implementer**: stop at the workaround and log it, rather than ship it and
move on. It is the one part of the method that assistance makes harder rather than cheaper,
and it is why § 5's logs — not the applications — are the primary artifact.

---

## Related documents

| Document | Answers |
|----------|---------|
| `README.md` § Authorship & AI Collaboration | The same disclosure for the interpreter, and what AI does not replace |
| `README.md` § Language-Driven Validation | The projects, what each put under test, and the code that came out |
| `SYMBOLS.md` § 17 | The rules a Green-state operator must satisfy before it can exist |
| `zyquality/GOVERNANCE.md` (separate repository) | The verification layer today: one corpus, three engines, one verdict — and `project/apps.toml`, where the applications are gated |
| `ZyDDT/CHARTER.md` (separate repository) | The layer points 7 and 8 ask for, named and given an admission rule. It supersedes ZyQuality, which stays authoritative until it can answer everything ZyQuality answers |
| `ZyFmtCheck/README.md` (separate repository) | Why the applications, and not the corpus, are the body that shows what a formatter damaged |
| `USERAPPI18N.md` | Decalogue point 10 in its clearest form — 囲碁's i18n architecture as doctrine |
| `ROADMAP.md` | Where the still-open findings went |
| `CHANGELOG.md` | Which release closed which log |
