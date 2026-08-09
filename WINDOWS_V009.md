# Windows x86_64 — the eleven findings that ship in v0.0.9

**Status:** fixed and verified on Windows 11; the Linux side re-verified on Debian.
**Branch:** `v0.0.9`

This work started as a hotfix branch off v0.0.8, and this document was called
`HOTFIX_V008_01.md`. **There is no hotfix release.** Eleven findings is not a patch on
top of a release, it is the substance of the next one, so the branch became `v0.0.9`
and these corrections ship as part of it rather than as a separate 0.0.8.1.

This document was written on Linux, by reading the code and reproducing a Windows
user's report. It has since been rewritten on the Windows 11 machine where the work
actually happened. What follows is the record, not the plan: what each finding turned
out to be once it could be run, what was changed, and what is still open.

The count went from five findings to eleven. That is the headline. Every one of the
six new ones was a POSIX assumption that Linux could never have surfaced, and three of
them were in the test suite itself — which is why the suite reported a healthy build
right up to the moment a user tried to run it.

**Environment of record:** Windows 11 Pro 26200, rustc 1.97.1, cargo 1.97.1, Git for
Windows (its `sh.exe` at `C:\Program Files\Git\usr\bin\sh.exe`), SQLite ODBC driver
from ch-werner.de.

## The report

A user on Windows 11 (drive `D:`, project at
`D:\...\zy-Serpiente`) hit two distinct failures with the v0.0.8 release, and both
looked like the same "module not found" problem. They were unrelated.

```powershell
PS D:\...\zy-Serpiente> zymbol run "d:\...\zy-Serpiente\serpiente.zy"
Runtime error: failed to execute bash command: program not found
```

and, in VS Code, four red diagnostics on the two `<#` import lines:

```
E002: Module '/d:/OneDrive - Abastible S.A/Documentos/GitHub/zy-Serpiente\logica.zy' not found
```

Both reproduced exactly on Windows, byte for byte.

---

## W-1 — `<\ \>` hardcoded `sh`, which does not exist on Windows

**Severity:** blocking. **Status:** fixed, verified in both engines.

The runtime error did not come from the imports. It came from `juego.zy:41`, where the
game seeds its LCG from three shell commands, and from the fact that `<\ \>` was
`Command::new("sh")` at three sites: the tree-walker, the VM's `BashExec`, and the
VM's `Execute`.

**What the fix is.** One lookup, in `zymbol_common::shell`, shared by both engines so
they cannot drift apart on which shell a script runs in:

1. `ZYMBOL_SH`, for a shell somewhere we would not think to look.
2. `sh.exe` on `PATH`.
3. Git for Windows, located from `git.exe` on `PATH` when possible and from the
   standard install prefixes otherwise.
4. `cmd /C`, with a one-time warning on stderr, so `<\ \>` still does something on a
   machine with no POSIX shell at all.

Step 4 is a real change of meaning, which is why it warns rather than failing
silently. The order matters more than the fallback: wherever a POSIX shell exists it
wins, so a script written on Linux keeps working on a Windows box that has Git.

**WSL is deliberately absent**, and not only because not every machine has it.
`wsl.exe` would find a shell in a *different filesystem namespace*, where the script's
own `D:\project` is `/mnt/d/project` and a path it built by hand no longer names the
file it means. A shell that silently redefines what a path is would be worse than an
honest error. For the same reason the lookup never probes a bare `bash.exe` on `PATH`:
on Windows that name is usually the WSL launcher stub in `System32`.

**The half of this that the Linux diagnosis could not see.** Finding a shell turned
out to be half the problem. With Git for Windows found, `<\ "echo hola" \>` worked and
`<\ "date +%N" \>` did not:

```
ns:  /usr/bin/sh: line 1: date: command not found
```

Git for Windows keeps its coreutils beside `sh.exe` in a directory that is not on the
Windows `PATH`, so the shell started and then could not find `date`, `od` or `tr`. The
child now gets the shell's own directory prepended to its `PATH`. With that, the three
POSIX seed commands from `juego.zy` run unmodified under both engines:

```
ns:      030767400
pid:     1377
rnd:     14518
semilla: 1025384615
```

So the second layer the original diagnosis worried about — "even with a shell present,
`date +%N`, `$$` and `/dev/urandom` are POSIX" — resolved itself: with the right
`PATH`, a POSIX shell brings its POSIX utilities. No script needed rewriting, and the
question of a native entropy source stays where it belongs, outside a hotfix.

The error messages were wrong twice over — naming bash while running `sh`, and never
naming what failed to spawn. Both now name the program.

---

## W-2 — the LSP built and parsed `file://` URIs by hand

**Severity:** blocking in the IDE. **Status:** fixed; still to be re-checked in VS Code.

`uri_to_path` stripped `file://` and nothing else. VS Code sends
`file:///D:/OneDrive%20-%20.../serpiente.zy`, so what came back was
`/D:/OneDrive - .../serpiente.zy` — which on Windows is a **relative** path, since an
absolute one must begin at the drive. Every import was then looked for under a
directory that does not exist. Reproduced exactly, including the mangled string from
the user's screenshot:

```
uri_to_path()       : Some("/D:/OneDrive - .../juego.zy")
Url::to_file_path() : Some("D:\\OneDrive - .../juego.zy")   <- correct
```

`path_to_uri` had the mirror bug, formatting `file://{path}` into
`file://D:\dir\x.zy`: backslashes unescaped, drive unencoded, and no URI any editor
will match against the document it has open. The same `format!` appeared at eight call
sites, each with its own copy of the `if starts_with("file://")` dance.

Everything now goes through `Url::from_file_path` / `Url::to_file_path` and a single
`uri_str_to_url`. The hand-written `percent_decode` added for BUG-003 went with them;
`Url` decodes as a matter of course, so the Unicode-directory fix is preserved for
free.

**One trap worth naming**, because it cost a debugging round and the new tests caught
it: on Windows `Url::parse("D:\\proyecto\\main.zy")` *succeeds*. A drive letter is a
syntactically valid scheme, so a bare path came back as a `d:` URL naming no local
file, and every caller that passes a path rather than a URI got `None`. A single-letter
scheme is now read as a drive letter — no scheme anyone uses is one character long.

**Why this survived to a release:** the URI tests asserted Unix-shaped literals. They
passed on Linux while asserting nothing whatever about Windows. They now build their
expectations per platform and cover drive letters, spaces and non-ASCII on both.

Note for reviewers: `path_to_uri` now percent-encodes on Linux too, where it used to
emit raw UTF-8. Documents are keyed by URI string, so the change is self-consistent,
but it is the one behaviour here that is visible off Windows.

---

## W-3 — `ModulePath::resolve_from` assumed a POSIX filesystem root

**Severity:** latent when reported. **Status:** fixed, verified in both engines.

`HOME` is not set on Windows — confirmed empty on the machine of record, with
`USERPROFILE=C:\Users\o_espinoza` — so `~/mod` fell back to `/root`, a path that
cannot exist there. And `/mod` resolved against `PathBuf::from("/")`, which on Windows
is root-relative to whichever drive happens to be current.

Home lookup now goes `HOME` → `USERPROFILE` → `HOMEDRIVE`+`HOMEPATH`. `HOME` stays
first on every platform: a user who sets it means it, and Git Bash sets it.

A leading `/` now resolves against the root of **the drive the importing file is on**,
rather than a driveless root. That keeps `<# /lib/x` inside the project's own drive
instead of wherever the process happened to start. Both verified with scratch modules
on `C:\` and `D:\`.

---

## W-4 — `starts_with('/')` used as the absolute-path test

**Severity:** latent. **Status:** fixed at three sites — one more than the original
diagnosis found.

`D:\lib\x.zy` does not start with a slash, so it was filed as relative and joined onto
the caller's directory. The two documented sites were the tree-walker's `</ path />`
and the VM compiler's. The third was
`crates/zymbol-package/src/closure.rs:465`, the walker that computes what goes into a
`.zyp`: without it, packaging on Windows would silently drop a file the engines will
still go and execute.

`crates/zymbol-package/src/path_safety.rs:29` also tests `starts_with('/')` and was
deliberately left alone: it is a lexical security check that already rejects drive
letters and backslashes separately.

---

## W-5 — TUI support on Windows

**Severity:** unknown when reported. **Status:** renders correctly; the keyboard was
broken for a different reason, now W-6.

`serpiente.zy` reaches a playable screen on Windows. Raw mode enters (alternate
screen, cursor hidden), box drawing renders, and the accented Spanish survives:

```
╭───────────────────────────────╮
│          Z Y M B O L          │
│       S E R P I E N T E       │
├───────────────────────────────┤
│   ► [1]  Lento       160 ms   │
│     [3]  Rápido      100 ms   │
│     [L]  Idioma: Español      │
╰───────────────────────────────╯
```

So crossterm's Windows support is real and the earlier caution was unnecessary. What
was *not* fine was input, which turned out to be its own finding.

---

## W-6 — Windows reports key releases as well as presses, and `<<|` counted both

**Severity:** blocking in any TUI program. **Status:** fixed in three engines; needs a
human at a keyboard to confirm.

Reported by the user as three symptoms that looked unrelated: the game started by
itself as though Enter had been pressed twice, an arrow moved two cells in the menu,
and in play a turn only registered two cells later, no matter how slow the speed.

They are one cause. The Windows console delivers a `KEY_EVENT` when a key goes down
and another when it comes back up, and crossterm passes both through. All four read
sites matched `Event::Key(KeyEvent { code, .. })` — discarding `kind` — so every
keystroke counted twice. Unix sends no releases, so Linux could never see it.

The delay-that-does-not-depend-on-speed is the same cause seen from further away: each
keystroke enqueued two events while the game consumed one per tick, so a backlog built
up and the turn arrived late regardless of how long a tick lasted.

Now filtered to `Press | Repeat` in the tree-walker, the VM and — the same bug, found
by looking — the REPL, where it would have echoed every character twice. `Repeat`
counts as a press because a held key auto-repeats on Unix too.

**No automated suite covers this.** The three project suites all say so explicitly:
`>>|` refuses to start without a real terminal, so the game loop is tested by hand.
See "Testing terminals" below.

---

## W-7 — tests used `stdout().is_tty()` as a headless probe, and one hung forever

**Severity:** blocking the suite. **Status:** fixed, then refined on Linux.

`cargo test` never finished on Windows. `test_key_input_headless_graceful` skips itself
when a terminal is present and asked `stdout().is_tty()` — but under `cargo test`
stdout is captured, so it read "headless" while the process still had a console
attached. `<<|` then sat waiting for a keypress nobody was going to press, and the run
had to be killed by hand.

On Windows, "stdout is not a tty" and "there is no terminal" are different statements:
crossterm talks to `CONIN$`/`CONOUT$` rather than the standard handles. The same
mismatch failed `cli_repl_terminal_size_positive`, which asserted the 24×80 crossterm
falls back to with no terminal — Windows answered a real 30×120 and the test failed on
a correct answer.

The first fix asked `terminal::size().is_ok()`. The Linux side then sharpened it: on a
box with no controlling terminal, `size()` still answers while `enable_raw_mode()`
fails with `ENXIO`, so a test that asked the first question and asserted the second
failed there. The TUI-block test now asks by *doing* — enable raw mode, undo it —
while `<<|` keeps the broader probe, because there the risk being avoided is blocking
forever rather than a wrong assertion.

Confirmed on Windows: under `cargo test` with a console attached, raw mode is
available, the block executes for real, and the trace shows it
(`?1049h … ?1049l`).

---

## W-8 — the VM leaked `\\?\` extended-length paths into error messages

**Severity:** engine divergence. **Status:** fixed.

Caught by the parity suite, not by a user:

```
TW: 7 parse error(s) in 'D:/...\tests\i18n\matematicas\sistema.zy'
VM: 7 parse error(s) in '\\?\D:\...\tests\i18n\matematicas\sistema.zy'
```

The compiler canonicalises a module path for cycle detection, which is the right use —
it is about identity. It then used that same path in the message. On Linux
`canonicalize()` changes nothing visible; on Windows it returns the extended-length
form, so the VM named a path the user cannot type and disagreed with the tree-walker
while doing it.

The file already knew: three lines below, the semantic gate carries a comment saying
it uses the non-canonical path for exactly this reason. The parse-error branch now
does the same.

---

## W-9 — six tests hardcoded `/tmp`

**Severity:** blocking those tests. **Status:** fixed; `std/io` verified on Windows.

Three `std/io` tests and three `std/db` tests wrote to `/tmp/...`, which does not exist
on Windows. They use relative paths now, and the artifacts are gitignored. `std/io`
passes on Windows in both engines as a result.

`stdlib_io_rw.expected` also pinned the POSIX `strerror` text for a missing file. That
string differs on Windows *and* is translated into the system language — the machine of
record answers `El sistema no puede encontrar el archivo especificado` — so a literal
golden would have failed even between two Windows boxes with different locales. It now
matches with a wildcard, which is what the test means: a soft IO error rather than a
crash.

`std/io` has no temp-directory or environment access, so there is no portable way for a
`.zy` to ask for a scratch location. Adding one is a language-surface question for the
symbol-vs-module rubric, not a hotfix.

---

## W-10 — the `std/db` tests pinned a driver name that exists only on Linux

**Severity:** blocking those tests. **Status:** fixed; `std/db` verified on Windows.

The tests said `Driver={SQLite3}`, which is what unixODBC registers after
`apt install libsqliteodbc`. The Windows installer for the same driver calls it
`SQLite3 ODBC Driver`, so the connection string could only ever work on the platform
the tests were written on.

Worth being clear about what was *not* wrong: ODBC needs no Windows-specific work at
all. The driver manager is part of the OS, the build already links `odbc32.lib`, and
`odbc-api` works there as-is. Only a driver and a data source were missing.

The tests now name a DSN — which is what ODBC provides for exactly this — and
`tests/stdlib/README-odbc.md` carries the one-line setup for Linux, macOS and Windows.
On Windows a user DSN lives in `HKCU` and needs no administrator; only installing the
driver itself does.

**This needs a `[zymbol_sqlite]` entry in `~/.odbc.ini` on Linux** before those three
tests pass again.

A second Windows-only trap sits underneath: braces are string interpolation in Zymbol,
so a connection string containing them must be written `Driver=\{...\}`. That
`Driver={SQLite3}` parsed at all was luck — `SQLite3` happens to be a valid identifier,
and an unknown one is left alone. `SQLite3 ODBC Driver` has spaces and is a lex error.
A DSN sidesteps the question entirely.

Verified on Windows in both engines: connect, DDL, parameter binding with a value
containing a quote and an ampersand (`O'Brien & Co.`), rows as named tuples,
transactions, savepoints with `rollback_to`, and the Spanish i18n adapter layer.

---

## W-11 — no `.gitattributes`, so every text file arrived as CRLF

**Severity:** root cause of a whole class. **Status:** file added; the working tree
still needs `git add --renormalize .`.

Git for Windows defaults to `core.autocrlf=true`. With no `.gitattributes`, every `.zy`,
`.expected` and `.input` was checked out as CRLF:

```
i/lf  w/crlf  attr/     tests/i18n/test_database.zy
```

Three consequences, none of them obviously about line endings:

- `.expected` goldens arrive as CRLF while program output is LF, so every golden
  comparison fails unless the harness normalises both sides. The shell suites do not,
  which is why they cannot run on Windows at all — and why the first attempt to use
  `vm_compare.sh` as a reference produced nothing usable.
- A parse error quotes the offending source, and a CRLF source file puts the carriage
  return *inside* the quoted text: `String(" >\r\n>> ")` on Windows against
  `String(" >\n>> ")` on Linux. Identical sources, different diagnostics.
- `.input` fixtures are fed through stdin, where a stray CR is a character the program
  under test was never meant to see.

`.gitattributes` now pins `eol=lf`. Adding the file changes nothing already on disk, so
an existing clone needs `git add --renormalize .` — deliberately left as a separate
step, since it touches every text file in the repository.

---

## Testing terminals across the three platforms

W-6 and W-7 were both invisible to the suite, and they were invisible for the same
reason: the tests asked Unix-shaped questions (`is_tty`, "no tty means 24×80") and
nothing could press a key. The shape of a fix, if it is wanted, is two tiers:

**Tier one — pure logic, no terminal.** Extract the decision from the match arms into
`key_event_to_char(Event) -> Option<char>` and test it with synthetic events. This runs
everywhere today, needs no new environment, and would have caught W-6 exactly: feed a
`Press`/`Release` pair, assert one character.

**Tier two — a real PTY.** `portable-pty` gives one on all three platforms (ConPTY on
Windows 10 1809+, `openpty` elsewhere). The decisive design choice is to **assert on
the rendered grid, not the byte stream**: the escape sequences crossterm emits differ
legitimately per platform, the resulting screen must not. Parse the output into a cell
buffer and compare that, or the test fails on Windows for reasons that are not bugs and
gets disabled.

Neither is done. Tier one costs little and is where the value is.

---

## What is not a Windows problem

Several defects surfaced only because Windows forced a careful look. They are recorded
here so they are not mistaken for regressions:

**Eight `.expected` goldens carry an absolute path from one machine** —
`/home/rakzo/github/zymbol-lang/interpreter/...`. They pass on the clone that sits at
that exact path and fail everywhere else, including any other Linux box. Confirmed
empirically: the Linux run reports two failures where Windows reports ten, and the
eight in the difference are precisely these.

**Two goldens are stale** — `memory02_function_isolation` and
`errors/parser/parent_path_alias` contain `warning:` blocks and blank lines that the
suite's own `strip_warnings` removes from the actual output, so they cannot match on
any platform. Verified failing on Linux too, and deliberately left outside this
hotfix.

**`tests/output/` was hidden by a homonym in `.gitignore`.** The rule `tests/output/`
was read as "test output directory"; it is the test suite for the `>>` *output
operator*. Eight hand-written cases with goldens lived for two months on one machine
and were never committed. Found from a file-count discrepancy between the two
platforms, and fixed by committing them — not by excluding them, which would have made
the numbers agree by deleting eight real tests.

**A tree-walker/VM divergence on the error path of `std/db`**: member access on an
error value reports `Cannot access member 'cod' on non-tuple value` under one engine and
`type error: expected Tuple, got Error` under the other. Not exercised now that the DSN
exists, but still there.

---

## The Windows test runner

`tests/scripts/run-tests.ps1` runs the four correctness suites natively:

```powershell
.\tests\scripts\run-tests.ps1                      # everything
.\tests\scripts\run-tests.ps1 -Suite vm -Detail
.\tests\scripts\run-tests.ps1 -Suite expected -Filter stdlib_db
.\tests\scripts\run-tests.ps1 -ZymbolBin "C:\Program Files\Zymbol-Lang\zymbol.exe" -Suite vm
```

It is a port, not a wrapper. Requiring Git Bash to test a Windows build is the
assumption this branch exists to remove, and the typed wildcards the shell version
delegates to python3 are implemented with .NET regex, so there is no Python dependency
either.

Four things it has to handle that the shell suites never did, each of which cost a
debugging round:

- **UTF-8 on the child's stdout.** Without pinning it, every test with an accent or a
  pIqaD codepoint fails on the encoding alone.
- **CRLF against LF goldens** — see W-11.
- **`cmd /c ... 2>&1` rather than two pipes.** Reading stdout and stderr separately and
  concatenating them cannot reproduce the *order* they interleave in: stdout is
  block-buffered when it is not a terminal, so a program that prints and then dies
  emits the error first. A golden recorded through `2>&1` will never match a
  concatenation.
- **The stdin BOM.** .NET builds a process's `StandardInput` writer from
  `Console.InputEncoding` and sets `AutoFlush`, and setting `AutoFlush` flushes — which
  writes that encoding's preamble. Merely *reading* the property put a UTF-8 BOM at the
  head of the pipe, and the interpreter read U+FEFF as the first character of the first
  line. Letting `cmd` do the redirection avoids it.

The file must stay ASCII-only. Windows PowerShell 5.1 reads a script with no BOM in the
system ANSI code page, so an em dash arrives as three CP1252 bytes ending in a curly
quote — which PowerShell accepts as a string delimiter, producing a parse error
pointing several statements away from the character that caused it.

---

## Verification

On Windows 11, at the merge of this branch:

| | Result |
|---|---|
| `cargo test --workspace --release` | 949 passed, 0 failed, 4 ignored |
| Tree-walker / VM parity | 543 pass, 0 fail, 1 skip — 544 total |
| Formatter idempotency | 547 pass, 0 fail |
| Expected-output goldens | 515 pass, 10 fail |
| Semantic diagnostics | 15 pass, 5 fail |
| zy-GO `試験/全試験.sh` | `全試験 PASS` |
| zy-Serpiente `pruebas/todas.sh` | `todas PASA` |
| zyKlingonGalaxy `mIw/Hoch.sh` | `Hoch PASS` |

On Debian, same commit: `cargo test` 945/945, `vm_compare` 544/544, `fmt_property`
600 pass / 0 fail. The parity totals agree exactly across the two platforms.

The 15 golden failures account for themselves completely:

| Count | Cause | Windows-specific? |
|-------|-------|-------------------|
| 8 | goldens carrying one machine's absolute path | no |
| 2 | stale goldens, failing on Linux too | no |
| 2 | `stress_v2` benchmarks exceeding a 10 s timeout — they pass at 120 s | no |
| 2 | `manual/tui/*`, where a reachable console changes what `<<|` does | **yes** |
| 1 | `i18n/test_database`, CRLF inside quoted source — W-11 | **yes** |

## Still open

- **Play `serpiente.zy`.** W-6 is the one fix no suite can confirm; all three project
  suites state that the game loop is tested by hand.
- **Re-check W-2 in VS Code** with the rebuilt LSP. The old server was observed
  reproducing the bug live during this work; the new one has not been watched in the
  editor.
- **`git add --renormalize .`**, which closes `i18n/test_database`.
- **The eight machine-specific goldens.** Regenerating them from the repository root
  with relative paths has to happen on Linux, or they would be rewritten with Windows
  separators.
- **Regenerating the signed `.msi`**: Windows signing is manual, so a release from this
  branch needs that step done by hand.
