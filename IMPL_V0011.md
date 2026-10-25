# Implementation Plan — v0.0.11: the VM is the engine, and what each thing costs

> **Status: draft, not released.** Written on 2026-10-25 from the `v0.0.11` branch at
> `b2a14b0`, the day the documents were audited against the implementation. The release is
> planned for **2026-11-01** (at the latest 2026-11-11). Part I is what has landed, Part II
> what remains before the tag. When the tag exists, this block says so and names it.
>
> **The plan is `ROADMAP_V0011_ES.md`** (in Spanish, the author's working language for it):
> its stages, the author's decisions D1–D8, the measurements behind each one. This document
> is the account of what came of it, with the same skeleton as `IMPL_V0010.md`. Where the
> two disagree about what was decided, the plan is right; about what the branch holds, the
> branch is.
>
> **Every gate figure below was measured on 2026-10-25**, on `b2a14b0`, against a release
> build with `--features db`. Figures from earlier days carry their date.

---

## What kind of release this is

An **optimization** release, defined by the author on 2026-10-13 as "the optimization of the
language's time and resources". The register VM became the default engine; what grew with
the size of the input and should not have was measured against Python as the reference and
brought down, without changing what a program means; and the interpreter was split so that
a binary can be built without what it does not use.

Late in the branch — 2026-10-21 to 2026-10-24 — the rows of `AGENTIC.md` § 12 that were still
open were each decided by the author and implemented the same day. That part changes what a
program may do, and it is listed apart in the CHANGELOG under *Changed — rules a program can
break on* and *Added*.

## Part I — What landed

### Stage 1 — the VM is the default engine (2026-10-13)

`zymbol run file.zy` runs the register VM; `--tw` asks for the tree-walker, `--vm` is accepted
and changes nothing (D3), the REPL stays on the tree-walker (D2). Both `engines.toml` name
`--tw` for `zytw`, in the same change — without it the gate would have compared the VM with
itself and stayed green. The five-zone probe the plan asked for before the change was done
on 2026-10-16.

### Stage 2 — what grew with n (2026-10-16 to 2026-10-19)

The seven rows of the plan's list, from `zyquality/bench/big/HALLAZGOS.md` (PERF-01…08,
re-measured against the v0.0.10 release binaries):

| Row | What it was | Fixed by |
|---|---|---|
| PERF-06 | building a text with `s = s x y` copied it whole | ZYVM-023 |
| PERF-04 | `@ ch:texto` was quadratic in the VM | ZYVM-024, ZYVM-025 |
| PERF-05 a | a slice of a text copied the text | GLB-130, ZYVM-026 |
| PERF-03 | a dictionary key was found by walking the keys | GLB-132 (`IndexMap`, foldhash) |
| PERF-07 | a deep recursion aborted the process | GLB-133, D7 (`##Depth` at 10 000) |
| PERF-05 b | `s$#`, `s[i]` walked the text | ZYVM-028, D8 |
| PERF-08 | the static musl binary was slower | GLB-134 (mimalloc, tuned) |

The closing plan of 2026-10-19 then took what the review of those rows found, in five
groups: GLB-139 and ZYTW-016 (a text read by position), GLB-140 and ZYJS-068 (an edit that
copied its receiver), ZYTW-019 (the tree-walker's `Value`, 120 → 24 bytes) and ZYVM-022 (a
call's result kept alive), D4 (`std/db` off by default) and GLB-134, and the documents. On
the way: GLB-141, ZYTW-018.

The VM's dispatch was reworked alongside, each change measured in instructions and kept
only if it paid: `RangeStep` (ZYVM-029) and `IterStep`, calls that push their frame inline,
no stack check on a direct call, a tail call inside a module (ZYVM-027), `$>`/`$|`/`$<` on
the VM's own frames, `$^` with a comparator that only compares its parameters evaluated
without a call. One was built, measured worse and not kept: `$^` without re-entry for every
comparator (`b227120`).

### Stage 3 — four evaluations (2026-10-20)

| | Evaluation | Decision |
|---|---|---|
| 3.1 | separate the components | **form B**, done: the VM's terminal and host are traits (`zymbol_vm::host`), `zymbol-host` the native one; `zymbol-vm` depends on no system library |
| 3.2 | an ultra-compact engine | **form A**, done: `lsp`, `repl` and `http` are features, on by default; the compact build is 100 crates and 6.6 MB against 210 and 12.3 |
| 3.3 | `.zyb`, Zymbol Binary | **out of v0.0.11** (author, 2026-10-25), to v0.0.12. Measuring it found GLB-142 and GLB-143, both fixed |
| 3.4 | WebAssembly | **form A**, done: `ZyStr` takes 32-bit pointers; the compiler and the VM build for `wasm32`, and CI checks it. WebAssembly stays an optional engine, never the web's |

### Stage 4 — the browser VM (`zyvm.js`)

**Paused whole** by the author on 2026-10-20: if 3.4's engine works out, a new VM in
JavaScript would not be needed.

### The safeguards of `AGENTIC.md` § 12 (2026-10-21 to 2026-10-24)

| Row | Decision of the author | What it is now |
|---|---|---|
| G1 | option A of four | `zymbol run --deny <effects>`, `zymbol check --effects` |
| G2 | option A of four | a value in `<\ \>` is one quoted word |
| G3 | option A of four | `zymbol run --timeout <s>`, exit 124 |
| G4 | option A of four | a warning for a read `\ x` has certainly destroyed |
| G5 | option A of four | a warning for a `??` that may find no arm |
| G8 | option A of four | `random::semilla(n)`, the same sequence in the three engines |
| ZethyCLI G1 | option A, widened | `std/net` with a time limit per request and per process (`net::timeout(s)`) |

Their probes found GLB-147, GLB-148, GLB-149, ZYVM-032, ZYJS-070 and ZYJS-071, all fixed.
On 2026-10-21, GLB-127 (`>> +a`) was fixed in the parser and uncovered GLB-144, GLB-145
(a unary takes its operand's postfix: `-v[1]` is `-(v[1])`) and GLB-146, fixed the same day.

On 2026-10-24 the author closed the scope: of what was left to decide, only ZethyCLI G1
entered; everything else, and any new finding, goes to v0.0.12 unless it blocks the release
(`ZyDDT/PENDIENTES.md` § 2).

## Part II — What remains before the tag

| | What | State on 2026-10-25 |
|---|---|---|
| CHANGELOG | `[0.0.11]` completed and dated | draft written 2026-10-25 |
| This document | Part I checked against the branch, Verification re-run on the tagged tree | draft |
| Installers | `packaging/verify/run-local.sh --scope full`, every installer against the installed binary | not run on this branch yet |
| Sister repositories | merged before publishing — the release reads its workflows from the default branch | — |
| User documents | review markers (`> **Revisado para …`) name v0.0.8 in 53 documents, v0.0.9 in one, v0.0.5 in `manual_en.md`; none v0.0.10. Each needs its content read before it is stamped | not started; the author's call whether before the tag |
| `zymbol.js` | its header's version log stops at v0.0.8 | not decided |

## Verification

```bash
cargo build --release --features db
cargo test --release --features db              # 1110 passed, 0 failed, 4 ignored
cd ../zyquality && make && ./zyq suite          # all gates pass, 11 min 56 s
```

| Suite | Result |
|-------|--------|
| `zyq consensus` | 714 files × 3 engines: **708 agree, 0 diverge**, 6 with too few engines |
| `zyq consensus --engines zytw,zyjs` | 714 files: 695 agree, **0 diverge**, 19 excused in `corpus.toml` |
| `zyq expect` | 677 goldens via `run`: 677 match, **0 stale**, 0 unchecked · 31 via `check`: 31 match |
| `zyq reject` | 57 forms × 3 engines: **57 refused everywhere**, 0 accepted somewhere |
| `zyq audit` | 714 `.zy` · 708 with goldens · 6 consensus-only · 6 excused for every engine |
| `zyq selftest` | 56 checks, all passed |
| `fmt` | 812 files: 764 PASS, **0 FAIL**, 48 SKIP · P1–P4 all zero |
| `fmtcheck` | 9 LDV applications formatted, **suite unchanged** in every one · 5 pins hold |
| `tui` | 3 cases × 2 engines: 3 agree, 0 diverge |
| `guide` | 248 blocks, 135 tested: **135 PASS**, 0 FAIL |
| `llm` | **89 probes PASS**, 0 FAIL · 27 of 27 rules and bullets of `LLM.md` covered |
| `lsp` | 714 files · 14 disagreements, **0 outside the baseline** |
| `project` | 9 LDV applications: **every golden holds**, `zytw` and `zyvm` agree on every one |
| `addressing` | 24 cells: **24 hold**, 0 do not |
| `messages` | Rust 955 · zyjs 570 · nothing new on the shared surface (baseline 211 of 436) |
| `reach` | 696 diagnostics defined in the gated layers: 527 provoked, 169 not — nothing new unprovoked |
| `bench` | 16 PASS, 0 FAIL, 0 NEW |
| `bigbench` | 16 of 16 programs agree with Python |
| `zyddt` | 46 axes, **1 765 cells, all agree, none known**, 194 checked against an oracle that is not ours · 12 pins, all agree · findings: **0 open**, 274 closed |
| `zyddt premises` | 29 declared · 0 with no cell · 7 by review · 1 held elsewhere |
| `cost` | 25 cases: **every ratio within its declared limit**, none KNOWN |
| `course` | the course imports the playground's highlighter; no copy |
| `web` | 17 suites pass · example pool 332 files: 216 agree, **0 diverge** |

The instruction set has 172 variants (`MatchCharRange`, ZYVM-032, is the last).

---

*Authorship and use of AI: this document follows the convention of `README.md` § Authorship &
AI Collaboration. Decisions about the language are the author's; the measurements and the
writing are engineering work assisted by Claude Code under the author's direction.*
