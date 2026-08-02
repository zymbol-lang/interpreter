# Package verification

Building a package and shipping a *working* package are different claims. The
release workflows used to only make the first one: they compiled, hashed and
uploaded. Nothing ever installed the result. This directory closes that gap.

## What runs where

| Stage | Where | What it proves |
|-------|-------|----------------|
| `build-linux` | `release-linux.yml` | The packages compile and are produced |
| `verify-linux` | `verify-linux-packages.yml` → `verify-deb.sh` in `debian:12` | The `.deb` installs on a clean system and the installed binary works |
| `publish-linux` | `release-linux.yml` | Only reached if verification passed |

Publication is gated: `gh release upload` now lives in a job that `needs`
verification. A `.deb` that does not install can no longer reach a user.

The verified file is the *published* file — packages travel from build to
publish as a workflow artifact, so nothing is rebuilt in between.

## Running it locally

```bash
sudo apt install podman          # rootless, no daemon; docker works too
bash packaging/verify/run-local.sh
```

`run-local.sh` builds the binary with the packaged feature set, builds the
`.deb`, and hands it to the same script and the same `debian:12` image CI uses —
a failure here is the failure CI would report. `--no-build` reuses what is
already built; `--scope smoke` skips the E2E suite.

**Where it compiles.** A binary runs on its build machine's glibc or newer,
never older. On a host newer than the verification image — Debian 13 is glibc
2.41, `debian:12` is 2.36 — a host build produces a package that correctly
refuses to install in the container, and the gate goes red for a reason
unrelated to your changes. So the compile happens in `rust:1-bookworm` by
default, and `--host-build` opts out when your host is not newer.

Neither path writes `target/release/`, so a `zymbol` on your PATH pointing at the
development build keeps its `std/db`.

Or drive the container yourself:

```bash
podman run --rm -v "$PWD:/workspace" -w /workspace debian:12 \
  bash packaging/verify/verify-deb.sh --deb packaging/dist/zymbol_lang_v0.0.8_x86_64.deb
```

## verify-deb.sh

It installs and removes `zymbol-lang` system-wide, so it refuses to run outside
a container unless given `--force-host`.

Every check runs even after one fails — a single run reports every problem the
package has. Exit 0 means all checks passed.

| Section | Checks |
|---------|--------|
| 1. Metadata | `Package`, `Version` vs `Cargo.toml`, `Architecture`, `Maintainer`, `Description`, `Depends: libc6`, filename matches the release convention |
| 2. Contents | `/usr/bin/zymbol` (mode 755), desktop entry, copyright, icon; nothing under `/usr/local` or `/opt` |
| 3. Installation | `dpkg -i` on a pristine Debian with no unmet dependencies, `dpkg --audit` clean, `zymbol` on `PATH`, `--version` agrees with `Cargo.toml` |
| 4. Linkage | Every `.so` the binary needs is covered by `Depends` — see below |
| 5. CLI | `run`, `run --vm`, `check` (accept *and* reject), `fmt`, `repl` over a pipe, non-ASCII source, `package` → `run` round-trip of a `.zyp` |
| 6. E2E | `tests/scripts/vm_compare.sh` against `/usr/bin/zymbol`, plus a floor on how many files the suite found. Not run at all if section 5 showed the binary cannot execute — see below |
| 7. Lifecycle | Reinstall over itself, `dpkg -r` leaves no files behind |

### The glibc floor, and the Depends that lied about it

The first run of this gate found that `control.in` hard-coded
`Depends: libc6 (>= 2.17)` while the binary needs whatever glibc built it. A
binary runs on its build machine's glibc or newer, never older, so the package
claimed a compatibility nothing had made true. `dpkg` would install it on a
system too old to run it — dependency satisfied, then this at exec:

```text
/usr/bin/zymbol: /lib/x86_64-linux-gnu/libc.so.6: version `GLIBC_2.39' not found
```

`build-packages.sh` now derives the floor from the binary's own versioned
symbols (`glibc_min_for`), for both `Depends` in the `.deb` and `Requires` in the
`.rpm`. The Arch `PKGBUILD` declares an unversioned `glibc`, which is right for a
rolling release. Section 4 compares the two numbers and fails when the declared
floor is lower than the real one — a **static** check, so it catches this even
when the container's glibc happens to be new enough to hide it. That matters:
CI compiles on `ubuntu-22.04` (2.35) and verifies on `debian:12` (2.36), so no
dynamic check would ever have noticed.

The supported floor is whatever the release builder ships — currently glibc 2.35
from `ubuntu-22.04`, covering Ubuntu 22.04+, Debian 12+ and current Fedora.
Older systems (Debian 11, Ubuntu 20.04, RHEL 8) are served by the static musl
binary, which depends on no glibc at all.

### Why the linkage check exists

`control.in` declares only `libc6`. A binary built with default features links
`libodbc.so.2` and `libltdl.so.7` for `std/db`, neither of which a pristine
Debian ships and neither of which the package declares — it would install fine
and then fail to start. That is why every release build passes
`--no-default-features`, and section 4 is what stops someone from quietly
dropping that flag: anything outside `libc / libm / libgcc / libdl / libpthread
/ librt / vdso / ld-linux` fails the release.

### The E2E suite cannot judge a binary that does not run

`vm_compare.sh` compares the two engines *against each other*. A binary that
fails identically under both — one that cannot start at all, say — produces
identical output everywhere and scores a flawless 544/544. The first run of this
gate did exactly that: eight failed checks in sections 3–5, and section 6
cheerfully green.

So section 5 records whether the binary can execute a trivial program, and
section 6 refuses to run when it cannot, reporting a failure instead of a score.
Two neighbouring checks had the same flavour of weakness and were tightened:
`fmt` asserts on formatted content rather than on any output at all (a
dynamic-loader error on stderr is also output), and the broken-program check
asserts on the diagnostic rather than on a non-zero exit (a binary that cannot
start also exits non-zero).

### The std/db tests, and the bug they found

The whole corpus runs — nothing is filtered. The packaged binary has no
`std/db`, so the five tests importing it only reach the "module not found" path,
but both engines must still say the *same thing* about it, and they did not:

```text
tree-walker:  Runtime error: module not found: std/db
VM:           Runtime error: module not found: std/db.zy
```

`compile_import` fell through to the file-resolution path for stdlib imports it
had no entries for, and that path formats `{}.zy`. A stdlib path has no file to
resolve to — `ModulePath::resolve_from` returns `None` for it by contract — so
the compiler now reports it right there, as `load_stdlib_module` does in the
tree-walker. Fixed in `zymbol-compiler`.

This is only visible in a build without the `db` feature, which is exactly what
ships in the packages, and the documented 544/544 never saw it because it is
measured with a full-featured binary. Verifying the artefact users install is
what surfaced it, so the suite stays unfiltered.

Current numbers for a release `.deb`: 544 files, 544 pass, 0 fail, 0 skip.

## Suite entry points

The three suites take `ZYMBOL_BIN` to choose the interpreter, defaulting to
`target/release/zymbol`:

```bash
ZYMBOL_BIN=/usr/bin/zymbol bash tests/scripts/vm_compare.sh
```

`vm_compare.sh` also gained an exit status — 0 when nothing mismatches, 1 on any
mismatch, 2 when the interpreter is missing. It previously always exited 0,
which made it useless as a gate. `VM_COMPARE_SUMMARY=path` writes
`total/pass/fail/skip/excluded` for machine consumption.

## Not verified yet

Deliberate: Linux `.deb` x86_64 first, pulled tight, then the rest.

- `.rpm`, `.pkg.tar.zst`, the static musl binary, and everything aarch64 are
  built and published without verification.
- macOS and Windows have no verification at all. Windows is the bigger gap: the
  MSI is generated by a WiX template and signed, and neither the installer nor
  the signature is ever exercised.

Extending means adding jobs to `verify-linux-packages.yml` — `fedora:40` for
rpm, `archlinux` for the Arch package, `alpine` to prove the musl binary runs
without glibc, and QEMU for aarch64. The same script structure applies; only the
install command and the metadata reader change per format.
