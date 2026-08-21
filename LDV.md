# Language-Driven Validation (LDV)

> **What this document is.** The method by which Zymbol is validated: what an LDV project is,
> what counts as a failure, what has to happen before the cycle closes, and where the record
> of every finding lives.
>
> **What it is not.** Not the test suite (`README.md` § Testing, `zyquality/GOVERNANCE.md`),
> which is the *verification* layer this method feeds. Not a list of the projects — the
> `README.md` § Language-Driven Validation table is that, and § 5 here indexes their logs.
>
> **Method.** Every claim below was checked against the eight gap logs in the application
> repositories, not against a previous edition of this document. Where the practice has
> drifted from what it says about itself, § 5.2 says so instead of tidying it away.

---

## Table of contents

1. [Validation is not verification](#1-validation-is-not-verification)
2. [LDV against TDD](#2-ldv-against-tdd)
3. [The decalogue](#3-the-decalogue)
4. [The cycle](#4-the-cycle)
5. [The gap logs](#5-the-gap-logs)
6. [What LDV costs, and when not to use it](#6-what-ldv-costs-and-when-not-to-use-it)

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
| `fmt_property.sh` | that formatting is reparseable, idempotent and meaning-preserving |

Every one of them takes a program that already exists and asks whether the implementation
handles it correctly. **None of them can tell you that the language cannot express a Go
board.** No suite reports a construct nobody has written yet; a missing capability produces no
failing test, because there is no test — that is what "missing" means.

That is the gap LDV fills, and the reason the method is named for validation rather than for
testing. The unit under test is the language: its syntax, its semantics, its standard library
and its tooling. The instrument is a complete application built in it.

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
| **Cycle time** | seconds | a release |
| **Cost of the test** | cheap; rerun on every save | expensive; months of work, and it cannot be rerun |
| **What it finds** | known-unknowns — the case you thought of when you wrote the assertion | unknown-unknowns — the case nobody could think of until a real domain forced it |
| **Primary artifact** | the passing suite | the gap log |
| **Regression protection** | the test itself is the protection | a separate, cheap layer, distilled from the finding (§ 4, Refactor) |

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

**7 — Regressions belong to a different layer.** Each finding is distilled into a minimal,
fast, automated case in the corpus and the unit suites. The application is never the
regression test; it is where the regression test came from.

**8 — Validation is expensive, verification is cheap.** An LDV project costs weeks of work and
cannot be rerun. Its value is entirely in discovering unknown-unknowns. Once one is found, the
knowledge has to be moved into something that costs milliseconds, or it will be lost the next
time somebody refactors the thing that caused it.

**9 — Culture is a test case.** Writing each project in a different natural language —
English, Mandarin, Spanish, Klingon pIqaD, Japanese — is a deliberate validation of the
keyword-free claim, of Unicode handling end to end, and of application-level i18n. It is what
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
```

The refactor step is not optional bookkeeping — it is the only part that survives. The
application will be superseded; the corpus entry will be run by every engine on every commit
for the rest of the project.

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

Eight published projects, eight logs, ~3,800 lines of recorded findings. This table is the only
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
- **It has poor resolution.** A failure points at a region, not a line. Reducing an application
  failure to a minimal `.zy` is real work, and it is the work that produces the value.
- **It scales with domain distance, not with size.** A third terminal game would validate
  almost nothing. The projects that paid were the ones that moved: a CLI over an HTTP service,
  then a TUI, then scientific computing, then a persistent 361-point data structure threaded
  through modules with recursive traversal. Choosing a domain the language has already served
  is the one reliable way to run the method and learn nothing.
- **It is not a gate.** Nothing in CI runs an LDV project, and nothing should. If a finding
  matters, it is in the corpus; if it is not in the corpus, it is not protected. The
  application is the microscope, not the alarm.

---

## Related documents

| Document | Answers |
|----------|---------|
| `README.md` § Language-Driven Validation | The projects, what each put under test, and the code that came out |
| `SYMBOLS.md` § 17 | The rules a Green-state operator must satisfy before it can exist |
| `zyquality/GOVERNANCE.md` (separate repository) | The verification layer: one corpus, four engines, one verdict |
| `USERAPPI18N.md` | Decalogue point 10 in its clearest form — 囲碁's i18n architecture as doctrine |
| `ROADMAP.md` | Where the still-open findings went |
| `CHANGELOG.md` | Which release closed which log |
