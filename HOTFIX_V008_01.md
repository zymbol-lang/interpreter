# Hotfix v0.0.8_HotFix01 — Windows x86_64

**Status:** findings documented, no fix applied yet.
**Branch:** `v0.0.8_HotFix01`
**Purpose:** this branch is meant to be cloned on a Windows 11 machine, where the fixes are
written, compiled and tested against a real Windows shell, a real terminal and a real
VS Code install. Everything below was diagnosed on Linux by reading the code and by
reproducing the failures from the evidence a Windows user reported; nothing here has been
executed on Windows yet.

## The report

A user on Windows 11 (drive `D:`, project at
`D:\OneDrive - Abastible S.A\Documentos\GitHub\zy-Serpiente`) hit two distinct failures with
the v0.0.8 release, and both looked like the same "module not found" problem. They are
unrelated.

```powershell
PS D:\...\zy-Serpiente> zymbol run "d:\...\zy-Serpiente\serpiente.zy"
Runtime error: failed to execute bash command: program not found
```

and, in VS Code, four red diagnostics on the two `<#` import lines:

```
E002: Module '/d:/OneDrive - Abastible S.A/Documentos/GitHub/zy-Serpiente\logica.zy' not found
E002: Module '/d:/OneDrive - Abastible S.A/Documentos/GitHub/zy-Serpiente\dibujo.zy' not found
```

The imports themselves are fine. `zymbol check serpiente.zy` on the same sources under Linux
reports no errors or warnings.

---

## W-1 — `<\ \>` hardcodes `sh`, which does not exist on Windows

**Severity:** blocking. This is what stops `zymbol run` from executing.

The runtime error does **not** come from the imports. It comes from `juego.zy:41`, where the
game seeds its LCG from three shell commands:

```zymbol
_ns  = <\ "date +%N" \>
_pid = <\ "echo $$" \>
_rnd = <\ "od -An -N2 -tu2 /dev/urandom | tr -d ' \n'" \>
```

The `<\ \>` operator is implemented as `Command::new("sh").arg("-c")` in both engines:

| Site | Construct |
|------|-----------|
| `crates/zymbol-interpreter/src/script_exec.rs:111` | tree-walker, `<\ \>` |
| `crates/zymbol-vm/src/lib.rs:2557` | VM, `Instruction::BashExec` |
| `crates/zymbol-vm/src/lib.rs:2587` | VM, `Instruction::Execute` (`</ path />`) |

There is no `sh` on a stock Windows PATH, so the spawn fails with `program not found`.

Two secondary observations at the same sites:

- **The message is wrong twice over.** It says "bash" while the code runs `sh`, and it never
  names the program it failed to spawn. `failed to execute bash command: program not found`
  gives the user nothing to act on. Fixing the wording is worth doing regardless of which
  option below is chosen.
- **`</ path />` diverges between engines.** The tree-walker interprets the target `.zy` file
  in-process (`eval_execute`, `script_exec.rs:20`), while the VM shells out through `sh -c`.
  On Windows that means `</ file.zy />` works under `--tw` and fails under `--vm`. This is a
  pre-existing TW/VM divergence that Windows merely exposes; decide separately whether it is
  in scope for the hotfix.

**Second layer — the commands are not portable either.** Even with a shell present,
`date +%N`, `$$` and `/dev/urandom` are POSIX. The source comment (*"sh puro, sin bash"*)
shows the intent was portability across POSIX shells, not across operating systems. So
fixing the spawn alone does not make `serpiente.zy` run on Windows unless the shell found is
a POSIX one.

**Decision deferred to Windows.** The options considered, to be settled with real tests
there:

1. Look for a POSIX `sh` on Windows (`sh` on PATH, Git for Windows at
   `C:\Program Files\Git\usr\bin\sh.exe`, WSL) and fail with a diagnostic that names what was
   tried. Keeps `<\ \>` semantics identical on all three platforms.
2. Use `cmd /C` on Windows. Smallest change, but it redefines `<\ \>` as "the native system
   shell", which breaks any script written against POSIX syntax — including this one.
3. Leave execution as-is and only document `<\ \>` as requiring a POSIX shell, with an error
   message that says so.

The related question — whether Zymbol should offer a native entropy source so examples stop
reaching for the shell just to seed a PRNG — is also deferred, and should be weighed against
the symbol-vs-module rubric rather than folded into a hotfix by default.

---

## W-2 — The LSP builds and parses `file://` URIs by hand, which breaks on Windows

**Severity:** blocking in the IDE. This is what produces the four false E002 diagnostics.

`crates/zymbol-analyzer/src/workspace.rs:254`:

```rust
pub fn uri_to_path(uri: &str) -> Option<PathBuf> {
    let raw = uri.strip_prefix("file://")?;   // only strips the scheme
    Some(PathBuf::from(percent_decode(raw)))
}
```

VS Code sends `file:///d%3A/OneDrive.../serpiente.zy`. Stripping only `file://` leaves
`/d%3A/...`, which percent-decodes to `/d:/...`. On Windows that is **not** an absolute path —
a drive letter must start the path, with no leading separator — so `PathBuf` treats it as
relative to the current directory of the active drive. `ModulePath::resolve_from` then pushes
the component with the native separator, producing exactly the string in the diagnostic:

```
/d:/OneDrive - Abastible S.A/Documentos/GitHub/zy-Serpiente\logica.zy
↑ leading slash before the drive                          ↑ native separator
```

The file is never looked for where it actually is, hence E002. This works on Linux only by
accident: there, stripping `file://` happens to leave a valid path.

The inverse function has the mirror bug — `workspace.rs:245`:

```rust
pub fn path_to_uri(path: &Path) -> Arc<str> {
    let uri = format!("file://{}", path.display());   // → file://D:\dir\x.zy
    Arc::from(uri)
}
```

That URI is malformed: backslashes are not escaped and the drive is not encoded. Diagnostics
and go-to-definition targets published under it will not match the document VS Code has open.
The same `format!("file://{}", …)` pattern appears at:

- `crates/zymbol-analyzer/src/symbols.rs:47`
- `crates/zymbol-analyzer/src/lib.rs:856`, `:1292`, `:1326`, `:1850`, `:1882`
- `crates/zymbol-analyzer/src/diagnostics.rs:354`
- `crates/zymbol-analyzer/examples/lsp_scan.rs:63`

**Direction of the fix:** use `lsp_types::Url::to_file_path()` / `Url::from_file_path()`,
which already handle drive letters, percent-encoding and separators — and which
`crates/zymbol-lsp/src/lib.rs:45` already uses correctly. The hand-rolled `percent_decode`
helper (added for BUG-003, Unicode directory names) becomes unnecessary at the same time.

**Watch the tests.** `workspace.rs:344-365` and `:381` assert the current Unix-shaped
behaviour and construct URIs with `format!("file://{}", …)`; they will need to be rewritten,
not just re-run. Add Windows cases (`file:///C:/…`, drive letters, spaces, non-ASCII).

---

## W-3 — `ModulePath::resolve_from` assumes a POSIX filesystem root

**Severity:** latent — not the reported failure, same family of bug.

`crates/zymbol-ast/src/modules.rs:186-192`:

```rust
let mut resolved = if self.is_absolute {
    if self.home_relative {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
        PathBuf::from(home)
    } else {
        PathBuf::from("/")
    }
}
```

- `~/mod` reads `HOME`, which Windows does not set (it uses `USERPROFILE`), so it silently
  falls back to `/root` — a path that cannot exist on Windows.
- `/mod` resolves against `PathBuf::from("/")`, which on Windows is a root-relative path with
  no drive.

Both should go through a platform-aware home lookup and a drive-aware root. Note this
function is the shared source of truth for the tree-walker, the semantic analyzer and the VM
compiler, so one fix covers all three.

---

## W-4 — `starts_with('/')` used as the absolute-path test

**Severity:** latent — same family.

- `crates/zymbol-interpreter/src/script_exec.rs:24` — `</ path />` in the tree-walker
- `crates/zymbol-compiler/src/lib.rs:1820` — the same construct in the VM compiler

On Windows `D:\lib\x.zy` does not start with `/`, so it is classified as relative and joined
onto the current file's directory, yielding a nonsense path. `Path::is_absolute()` is the
portable test.

---

## W-5 — TUI support on Windows: to be verified, not assumed

`>>|` (raw mode) and `>>?` (terminal size) go through `crossterm`
(`crates/zymbol-interpreter/Cargo.toml:19`, `crates/zymbol-vm/Cargo.toml:15`), which supports
Windows. So the TUI part of the game has a real chance of working once W-1 is resolved — but
this has never been exercised on Windows. Verify on Windows Terminal and on the legacy
console host: raw mode, ANSI sequence handling, and the code page (UTF-8 output, the box
drawing and the accented Spanish text).

---

## Verification plan for Windows 11

Build:

```powershell
cd interpreter
cargo build --release
```

Reproduce before touching anything, so the fixes have a baseline:

```powershell
# W-1
.\target\release\zymbol.exe run "D:\...\zy-Serpiente\serpiente.zy"
.\target\release\zymbol.exe run --vm "D:\...\zy-Serpiente\serpiente.zy"

# W-2: open the folder in VS Code with the v0.0.8 extension and confirm the
# false E002 on the <# lines; check go-to-definition on an import alias too.

# W-3 / W-4: a scratch file importing via ~/ and via an absolute D:\ path,
# plus a </ D:\...\x.zy /> execute, under both --tw and --vm.
```

After fixing, the non-negotiable checks are that the existing suites still pass **on Linux**
(nothing here should regress POSIX behaviour):

```bash
cargo test
bash tests/scripts/vm_compare.sh     # 536 cases — tests/output/ is gitignored
```

and, on Windows, that `serpiente.zy` runs to a playable screen under whichever `<\ \>`
option is chosen.

## Out of scope until decided

- Native entropy source / rewriting the `serpiente` seed (depends on the W-1 decision).
- The `</ path />` TW-vs-VM divergence noted under W-1.
- Regenerating the signed `.msi`: Windows signing is manual, so a release from this branch
  needs that step done by hand.
