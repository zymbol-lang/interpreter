# tests/ — what is still here, and where the rest went

The `.zy` corpus and its `.expected` golden files **are not in this directory
any more**. They live in [ZyQuality](https://github.com/zymbol-lang/zyquality),
which is this project's point of record for testing.

```text
../zyquality/
    zyq            the differential runner
    corpus/        585 .zy files, 583 with a .expected golden
    corpus.toml    which engine may be judged on which file, and why not
    reject/        forms every engine must refuse
    suites.toml    the script suites, so `zyq suite` runs them too
    fmt/           formatter properties P1-P4 + baseline
    tui/           key input through a real pty
    bench/         the benchmark programs, runners and baseline
    docs/          GUIDE.md example verification
    project/       the real programs written in Zymbol
    platform/      the native Windows runner
    notes/         historical measurement records
```

## Why it moved

The corpus existed twice — here and in `zyquality/corpus/` — and the two copies
had already drifted **28 files apart**. `arity/` and `loops/labels/`, the work
that v0.0.9 added precisely because the four engines disagreed about labels,
were tested by `vm_compare.sh` and invisible to every other engine's suite.

Underneath that, five runners each carried their own idea of the same corpus:

| runner | how it excluded a file | how it normalised output |
|---|---|---|
| `vm_compare.sh` | `@vm-skip` marker, `VM_COMPARE_EXCLUDE`, three fixed paths | five `sed` expressions |
| `web/tests/test_runner.mjs` | a 40-entry `SKIP_SET` literal in JavaScript | `trimEnd` + tabs |
| `zyml/tests/parity.sh` | `input/`, `manual_check.zy`, `grep -L lib_time` | nothing |
| `engine_compare.sh` | nothing | nothing |
| `zyq consensus` | stderr prefixes | nothing |

A file excluded from the browser comparison because it shells out was still
counted as a divergence by zyml. Those lists are now one file, `corpus.toml`,
next to the corpus they describe.

## What is still here

| | |
|---|---|
| `parser_integration.rs` | a Rust integration test; `cargo test` runs it |
| `scripts/` | wrappers only. No `.zy` files, no suite logic, no baselines. |

Every script in `scripts/` keeps its name, its flags and its exit codes, and
delegates. Two entry points have no wrapper because a shell one would be a lie:
`guide_verify.py` is now `../zyquality/docs/guide_verify.py`, and `run-tests.ps1`
is `../zyquality/platform/run-tests.ps1` — that one stays a native PowerShell
reimplementation rather than a wrapper, because the whole reason it exists is to
test Windows with no bash, no coreutils and no WSL.

`../examples/` is untouched: those are this repository's own example programs,
not the shared corpus, and `fmt_property.sh` still sweeps them. Worth knowing:
none of the 98 has a golden, so nothing checks what they print — only that
formatting them does not change it.

### The eleven `.zy` files that used to be in `scripts/`

None of them was a test, which took some digging to establish:

| | what it was | who ran it |
|---|---|---|
| `bench_*.zy` ×7, `stress.zy` | benchmarks for `run_all.sh` / `bench_gate.sh` | only the browser parity suite, where all eight failed |
| `lib_time.zy` | a **module** — the CLI refuses to run it directly | the same, so it failed by definition |
| `_test_fib_approaches.zy` | an orphan; no script referenced it | the same |
| `manual_check.zy` | an interactive tool that shells out and waits for a person | nobody |

All four runners in this repository excluded `*/scripts/*`, and zyml's parity
excluded them via `grep -L lib_time`. The only suite that executed them was
`web/tests/test_runner.mjs` — and its ten failures were exactly these files,
reported in that repository's README as divergences of the JavaScript engine.

They now live in `../zyquality/bench/` (the benchmarks, with one copy of
`lib_time.zy` instead of two) and `../zyquality/corpus/manual/`
(`manual_check.zy`, beside the manual corpus it drives).

## Running the tests

Every script here keeps its name, its flags and its exit codes. They delegate.

```bash
bash tests/scripts/vm_compare.sh          # → zyq consensus --engines zytw,zyvm
bash tests/scripts/engine_compare.sh      # → zyq consensus  (all four)
bash tests/scripts/expected_compare.sh    # → zyq expect
bash tests/scripts/semantic_compare.sh    # → zyq expect --via check
bash tests/scripts/fmt_property.sh        # → zyquality/fmt/fmt_property.sh
bash tests/scripts/run_all.sh             # → zyquality/bench/run_all.sh
bash tests/scripts/bench_gate.sh          # → zyquality/bench/bench_gate.sh
bash tests/scripts/run-project-tests.sh   # → zyquality/project/run-project-tests.sh
cargo test                                # 969 Rust unit tests, unaffected
```

Or the whole thing at once, which is the point of all this:

```bash
cd ../zyquality && ./zyq suite
```

They need a ZyQuality checkout beside this one:

```bash
git clone https://github.com/zymbol-lang/zyquality.git ../zyquality
make -C ../zyquality
```

or `ZYQ_ROOT=/path/to/zyquality`. Without one they exit **2**, not 0 — a gate
must not read "nothing ran" as "nothing failed".

## Adding a test

Add the `.zy` to `../zyquality/corpus/`, then record its golden:

```bash
cd ../zyquality
./zyq expect --regen --new --engines zytw --filter your/new/file
git diff corpus/                          # read it before committing
```

If the file cannot be compared on some engine, say so in `corpus.toml` with a
reason — an exclusion nobody wrote a reason for is indistinguishable from a bug
somebody hid. If it prints elapsed time it is a benchmark, and belongs in
`../zyquality/bench/` rather than in the corpus with an exclusion to silence it.

## One change of interface

`VM_COMPARE_EXCLUDE` took an extended regex and is no longer read. Exclusions
are declared once and grouped by tag:

```bash
VM_COMPARE_EXCLUDE='stdlib/stdlib_db'  bash tests/scripts/vm_compare.sh   # was
bash tests/scripts/vm_compare.sh --without STD_DB                        # now
```

`ZYMBOL_BIN` is unchanged and still points the suite at an installed package
instead of the build tree.
