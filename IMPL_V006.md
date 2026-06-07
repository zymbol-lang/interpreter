# Implementation Plan — v0.0.6 Bytecode Standalone + VM Input + TUI Fixes

Features span four categories: one breaking syntax change (`=>`), one structural improvement
(bytecode-embedded standalones), VM input parity, and four targeted bug/TUI fixes.

**Dependency chain:**
- Feature 1 (FatArrow) must be applied first — all `.zy` source files depend on it.
- Feature 2 (bytecode standalone) requires `zymbol-bytecode` to be serializable before
  touching `zymbol-standalone` or the compiler entry point.
- Feature 5 (VM `<<`) builds on existing `Statement::Input` AST — no parser changes needed.
- Features 6–8 are independent point fixes and can be applied in any order.

Recommended implementation order: 1 → 2 → 5 → 6 → 7 → 8 → 3 → 4.

---

## Feature map

| # | Feature | Type | Complexity |
|---|---------|------|------------|
| 1 | FatArrow `=>` — universal "maps to" operator | Breaking | Low |
| 2 | Standalone binaries embed bytecode (not source) | Improved | Medium |
| 3 | Typed wildcards in test golden files | Added | Low |
| 4 | GAP-Z009 — named fn retains module alias as HOF value | Fixed | Low |
| 5 | VM `<<` input support (`ReadLine` instruction) | Added | Medium |
| 6 | BUG-007 — semantic checker rejects recursive int functions | Fixed | Low |
| 7 | TUI-FIX-01 — `<<` inside `>>|` freezes terminal | Fixed | Low |
| 8 | TUI-FIX-02 — `>>|` cursor not at (1,1) on entry | Fixed | Low |

---

## Feature 1 — FatArrow `=>` as universal "maps-to" operator (Breaking)

This is the third and final step of the alias separator evolution documented in
`IMPL_V005.md §Feature-7`. The separator settles on `=>` (`FatArrow`) to resolve the
ambiguity that `:` introduced in v0.0.5: `:` already served as match-arm separator,
named-tuple field separator, and range separator (`@ i:1..10`), making
`<# path : alias` ambiguous to read in mixed contexts.

**Contract:** `=>` = `=` (mapping) + `>` (outward direction). Reads as "becomes" /
"maps to". Has no other meaning in the language.

**Three contexts unified under one token:**
- Match arms: `pattern => result` (was `pattern : result`)
- Import alias: `<# path => alias` (was `<# path : alias`)
- Export rename: `#> { fn => pub }` (was `fn : pub_name`)

### 1a. Lexer — new token `FatArrow`

**File:** `crates/zymbol-lexer/src/lib.rs`

Add to `TokenKind` enum (before the `Eq` / `=` branch so the two-char lookahead fires first):

```rust
/// => — fat arrow; "maps to" in match arms, import aliases, export renames
FatArrow,
```

In `next_token()`, add lookahead before plain `=`:

```rust
// When current char is '=' and next is '>':
if self.peek() == Some('>') {
    self.advance(); // consume >
    return Token::new(TokenKind::FatArrow, self.span(start));
}
```

### 1b. Parser — `parse_import_statement()`

**File:** `crates/zymbol-parser/src/modules.rs`

```rust
// Before (v0.0.5)
if !matches!(alias_sep.kind, TokenKind::Colon) {
    return Err(Diagnostic::error("expected ':' for module alias")
        .with_span(alias_sep.span));
}
self.advance(); // consume :

// After (v0.0.6)
if !matches!(alias_sep.kind, TokenKind::FatArrow) {
    return Err(Diagnostic::error("expected '=>' for module alias")
        .with_span(alias_sep.span)
        .with_help("import syntax: <# path => alias"));
}
self.advance(); // consume =>
```

Update doc comment: `/// Parse import statement: <# path => alias`.

### 1c. Parser — `parse_export_item()` rename branches (×3)

**File:** `crates/zymbol-parser/src/modules.rs`

Three places where an optional rename is gated on a separator token:

```rust
// Before (×3)
let rename = if matches!(self.peek().kind, TokenKind::Colon) {
    self.advance(); // consume :
    // … error: "expected new name after ':'"

// After (×3)
let rename = if matches!(self.peek().kind, TokenKind::FatArrow) {
    self.advance(); // consume =>
    // … error: "expected new name after '=>'"
```

### 1d. Parser — match arm separator

**File:** `crates/zymbol-parser/src/match_expr.rs` (or `expressions.rs`)

```rust
// Before
if !matches!(sep.kind, TokenKind::Colon) {
    return Err(Diagnostic::error("expected ':' after match pattern")…);
}

// After
if !matches!(sep.kind, TokenKind::FatArrow) {
    return Err(Diagnostic::error("expected '=>' after match pattern")
        .with_span(sep.span)
        .with_help("match arm syntax: pattern => result"));
}
```

### 1e. Formatter fix

**File:** `crates/zymbol-formatter/src/lib.rs`

The formatter had a latent bug emitting `<=` for import/export constructs instead of the
current separator. Fix: emit `" => "` (with surrounding spaces) for all three contexts —
import alias, export rename, match arm.

### 1f. Source file migration

All `.zy` files using `:` as the import/export/match separator must be updated.
Patterns to replace globally:

| Before | After |
|--------|-------|
| `<# path : alias` | `<# path => alias` |
| `fn : pub_name` (in `#> { }`) | `fn => pub_name` |
| `pattern : result` (in `?? { }`) | `pattern => result` |

Affected directories: `tests/`, `_staging/`, all example files.

---

## Feature 2 — Standalone binaries embed bytecode (not source)

### Problem

`zymbol build` embedded the raw `.zy` source and re-ran the full pipeline
(lex → parse → compile) on every execution. The standalone binary linked 7 crates
unnecessarily and had startup latency proportional to source size.

### Solution

Compile to bytecode **at build time** inside `zymbol build`, serialize via `bincode`,
and embed the bytes in a `const`. The generated binary links only `zymbol-bytecode` +
`zymbol-vm` (2 crates instead of 7). Zero lex/parse/compile overhead at startup.

**Result:** serpiente standalone 2.2 MB → 901 KB (~2.4× smaller).

### 2a. Serialize `CompiledProgram` and related types

**File:** `crates/zymbol-bytecode/Cargo.toml`

```toml
[dependencies]
serde   = { workspace = true, features = ["derive"] }
bincode = { workspace = true }
```

**File:** `crates/zymbol-bytecode/src/lib.rs`

Add `#[derive(Serialize, Deserialize)]` to all public types:
`CompiledProgram`, `Instruction`, `Chunk`, `GlobalInit`, `BuildPart`, `HotNeutral`.

### 2b. `write_bytecode()` in `zymbol-standalone`

**File:** `crates/zymbol-standalone/src/lib.rs`

Replace `write_source()` with `write_bytecode()`:

```rust
pub fn write_bytecode(program: &CompiledProgram, out_dir: &Path) -> Result<()> {
    let bytes = bincode::serialize(program)?;
    let dest = out_dir.join("src/bytecode.bin");
    std::fs::write(&dest, &bytes)?;
    Ok(())
}
```

New constructor that accepts a base directory for resolving module imports during
build-time compilation:

```rust
pub fn new_from_source(src: &str, base_dir: &Path) -> Result<Self> {
    let program = Compiler::compile_with_dir(src, base_dir)?;
    Ok(Self { program })
}
```

### 2c. Compiler — `compile_with_dir`

**File:** `crates/zymbol-compiler/src/lib.rs`

New public entry point:

```rust
pub fn compile_with_dir(src: &str, base_dir: &Path) -> Result<CompiledProgram> {
    // identical to compile() but sets the module resolver's search root to base_dir
}
```

### 2d. Standalone template `main.rs`

The generated `main.rs` shrinks from ~60 lines to 16 lines:

```rust
fn main() {
    const BYTECODE: &[u8] = include_bytes!("bytecode.bin");
    let program: zymbol_bytecode::CompiledProgram =
        bincode::deserialize(BYTECODE).expect("corrupt bytecode");
    let mut vm = zymbol_vm::Vm::new(program);
    if let Err(e) = vm.run() {
        eprintln!("{}", e);
        std::process::exit(1);
    }
}
```

This is also the foundation for the upcoming `.zyb` bytecode file format
(see `ROADMAP.md — "Bytecode File Format"`).

---

## Feature 3 — Typed wildcards in test golden files

### New wildcard tokens for `.expected` files

| Wildcard | Matches |
|----------|---------|
| `****` | any text (existing) |
| `***int***` | any integer (`-?[0-9]+`) |
| `***float***` | any float, including scientific notation |
| `***num***` | any number (int or float) |
| `***time***` | execution timing values such as `0.167s` or `12ms` |
| `***date***` | ISO 8601 dates such as `2026-05-26` |
| `***path***` | any non-whitespace path |

### 3a. `expected_compare.sh` — Python-backed matching

**File:** `tests/scripts/expected_compare.sh`

When a `.expected` file contains a typed wildcard, delegate line matching to Python 3:

```bash
python3 - <<'EOF'
import re, sys
pattern = sys.argv[1]
actual  = sys.argv[2]
# replace typed wildcards with regex groups
pattern = pattern.replace('***time***', r'\d+(?:\.\d+)?(?:ms|s)')
pattern = pattern.replace('***date***', r'\d{4}-\d{2}-\d{2}')
# … other wildcards …
pattern = pattern.replace('****', '.*')
sys.exit(0 if re.fullmatch(pattern, actual) else 1)
EOF
```

Falls back to existing `****` glob when Python 3 is absent.

### 3b. `--regen --smart` flag

New flag in `expected_compare.sh`: scans actual output for timing and date patterns and
replaces them with the corresponding typed wildcard in the regenerated `.expected` file.
Fixes `tests/stress_v2/bench_*.zy` tests that were failing due to timing variance.

### 3c. `semantic_compare.sh`

**File:** `tests/scripts/semantic_compare.sh`

Same typed wildcard support added (runs `zymbol check`, not `zymbol run`).

### 3d. `vm_compare.sh` — restore `tests/manual/`

**File:** `tests/scripts/vm_compare.sh`

`tests/manual/` files restored to the VM parity suite: **466 total, 463 PASS + 3 `@vm-skip`**.
The three skipped files are interactive TUI tests (`05_key_input.zy`, `06_tui_block.zy`,
`07_output_pos_sparse.zy`) that require a real TTY.

---

## Feature 4 — GAP-Z009: named functions retain module aliases as HOF values

**File:** `crates/zymbol-interpreter/src/functions_lambda.rs`

### Problem

A named function that references a module alias (e.g. `mat::sqrt`) fails with
`"undefined module alias: 'mat'"` when passed as a first-class value to a higher-order
function and invoked from inside it. The function's `Value::Function` representation did
not carry the module alias table of its definition scope.

### Fix

When storing a named function as `Value::Function`, capture the current `import_aliases`
snapshot at definition time. When the function is subsequently called as a HOF argument,
restore the captured aliases before executing the body, then discard them on return.

```rust
// Capturing at definition time:
Value::Function(FunctionValue {
    def: func_def.clone(),
    captured_aliases: self.import_aliases.clone(),  // ← new field
})

// Restoring at call time:
let saved = std::mem::replace(
    &mut self.import_aliases,
    func_val.captured_aliases.clone(),
);
let result = self.eval_traditional_function_call(…);
self.import_aliases = saved;
result
```

### New regression test

**File:** `tests/bugs/bug_named_fn_module_alias_hof.zy`

Three-file fixture: a math module, a named function that uses it, and a main script that
passes the named function to `$>` map. Verifies that the HOF invocation succeeds.

---

## Feature 5 — VM `<<` input support (`ReadLine` instruction)

The tree-walker has always supported `<<` and `<<~`. The VM had no corresponding
bytecode instruction, so any program using `<<` failed silently or at runtime in `--vm`
mode. This feature adds full parity.

### 5a. New bytecode instruction `ReadLine`

**File:** `crates/zymbol-bytecode/src/lib.rs`

```rust
/// Read a line from stdin into dst register.
/// prompt: optional register containing the prompt string to print first.
/// cast:   when true, attempt Int/Float parse (InputCast::Numeric).
ReadLine(Reg, Option<Reg>, bool),
```

### 5b. Compiler — compile `Statement::Input`

**File:** `crates/zymbol-compiler/src/lib.rs`

```rust
Statement::Input(input) => {
    let prompt_reg = match &input.prompt {
        None => None,
        Some(Output::Literal(s)) => {
            let r = self.alloc_reg();
            self.emit(Instruction::LoadStr(r, s.clone()));
            Some(r)
        }
        Some(Output::Interpolated(parts)) => {
            let r = self.compile_build_str(parts)?;
            Some(r)
        }
    };
    let dst = self.alloc_reg();
    let cast = matches!(input.cast, InputCast::Numeric);
    self.emit(Instruction::ReadLine(dst, prompt_reg, cast));
    self.register_map.insert(input.variable.clone(), dst);
    Ok(())
}
```

Simple (non-interpolated) prompts emit `LoadStr` + `ReadLine`.
Interpolated prompts emit a `BuildStr` sequence before `ReadLine`.
`InputCast::Numeric` sets the `cast` flag to `true`.

### 5c. VM handler — `ReadLine`

**File:** `crates/zymbol-vm/src/lib.rs`

```rust
Instruction::ReadLine(dst, prompt_reg, cast) => {
    // Suspend raw mode if inside a TUI block
    let in_tui = !self.tui_stack.is_empty();
    if in_tui {
        let _ = crossterm::terminal::disable_raw_mode();
        let _ = crossterm::execute!(
            std::io::stdout(),
            crossterm::cursor::Show
        );
    }

    // Print optional prompt
    if let Some(pr) = prompt_reg {
        print!("{}", self.reg(*pr).to_display_string());
        std::io::stdout().flush().ok();
    }

    // Read line
    let mut line = String::new();
    std::io::stdin().read_line(&mut line).unwrap_or(0);
    let line = line.trim_end_matches('\n')
                   .trim_end_matches('\r')
                   .to_string();

    // Restore raw mode
    if in_tui {
        let _ = crossterm::terminal::enable_raw_mode();
        let _ = crossterm::execute!(
            std::io::stdout(),
            crossterm::cursor::Hide
        );
    }

    // Optional numeric cast (mirrors interpreter: normalize Unicode digits first)
    let value = if *cast {
        let ascii = normalize_unicode_digits(&line);
        ascii.parse::<i64>().map(Value::Int)
            .or_else(|_| ascii.parse::<f64>().map(Value::Float))
            .unwrap_or_else(|_| Value::String(ZyStr::new(line)))
    } else {
        Value::String(ZyStr::new(line))
    };
    self.set_reg(*dst, value);
}
```

### 5d. Def-use analysis

**File:** `crates/zymbol-compiler/src/def_use.rs`

Add arm: `Instruction::ReadLine(dst, ..) => defs.insert(*dst)`.

---

## Feature 6 — BUG-007: semantic checker rejects recursive integer functions

**File:** `crates/zymbol-semantic/src/type_check.rs`

### Root cause

After GAP-Z008 made `Numeric.to_type()` return `ZymbolType::Float`, recursive integer
functions like `gcd(a, b)` were rejected with:

```
argument 2 has type Float, but function expects Int
```

The parameter `a` only had a `Numeric` constraint (no direct Int evidence), so
`a % b` resolved to `Float`. Then the recursive call `gcd(b, a % b)` passed a `Float`
where `b`'s `Exact(Int)` type was expected. `types_compatible_static()` had no
`(Float, Int)` arm — fell through to `_ => false`.

### Fix

```rust
// In types_compatible_static():

// Bidirectional numeric compatibility — consistent with runtime dynamic dispatch (BUG-Z001)
(ZymbolType::Float, ZymbolType::Int) => true,
```

### New regression test

**File:** `tests/bugs/bug_semantic_numeric_recursive.zy`

Covers `gcd`-style and `fibonacci`-style recursive functions that use `%` or `-` on
parameters with only a `Numeric` constraint. `zymbol check` must pass without error.

---

## Feature 7 — TUI-FIX-01: `<<` inside `>>|` freezes terminal

### Problem

`execute_input()` (tree-walker) and the (then-missing) VM `ReadLine` handler called
`stdin().read_line()` while the terminal was in raw mode. Raw mode passes bytes directly
without line buffering, so `\n` is never produced — the read blocks indefinitely.

### Fix — Interpreter

**File:** `crates/zymbol-interpreter/src/io.rs`

Wrap the read with a raw-mode suspend/restore guard when inside a TUI block:

```rust
pub(crate) fn execute_input(&mut self, input: &Input) -> Result<()> {
    if self.tui_depth > 0 {
        let _ = crossterm::terminal::disable_raw_mode();
        let _ = crossterm::execute!(
            std::io::stdout(),
            crossterm::cursor::Show
        );
    }

    // … existing prompt + read_line logic (unchanged) …

    if self.tui_depth > 0 {
        let _ = crossterm::terminal::enable_raw_mode();
        let _ = crossterm::execute!(
            std::io::stdout(),
            crossterm::cursor::Hide
        );
    }
    Ok(())
}
```

`tui_depth: u8` is incremented by `execute_tui_block()` before the body and decremented
on exit (both normal and error paths). Added in v0.0.5 TUI-FIX-01; this fix uses it.

### Fix — VM

Covered by Feature 5c: the `ReadLine` handler checks `!self.tui_stack.is_empty()` and
applies the same raw-mode suspend/restore pattern.

---

## Feature 8 — TUI-FIX-02: `>>|` cursor not at (1,1) on entry

### Problem

Some terminals inherit the main-screen cursor position when `EnterAlternateScreen` is
issued. The first `<<` prompt or `>>~` positioned output appeared at an arbitrary row
instead of the top-left corner.

### Fix — Interpreter

**File:** `crates/zymbol-interpreter/src/io.rs`

In `execute_tui_block()`, add `cursor::MoveTo(0, 0)` to the enter sequence:

```rust
crossterm::execute!(
    std::io::stdout(),
    crossterm::terminal::EnterAlternateScreen,
    crossterm::cursor::Hide,
    crossterm::cursor::MoveTo(0, 0),  // ← always start at top-left
)
```

### Fix — VM

**File:** `crates/zymbol-vm/src/lib.rs`

Same `cursor::MoveTo(0, 0)` added immediately after `EnterAlternateScreen` in the
`EnterTui` instruction handler.

---

## VS Code extension — v0.1.2

### Syntax highlighting

**File:** `vscode/syntaxes/zymbol.tmGrammar.json`

- `=>` (`FatArrow`) added to the module-syntax and match-arm token classes.
- `$*` added to `collection-operators` character class (was missing in v0.0.5).

### New snippets (`zymbol.json`)

| Snippet | Expansion |
|---------|-----------|
| `outps` | `>>~ (row, col, BKS, fg, bg) > value` |
| `outpc` | `>>~ (,,,fg) > value` |
| `repeat` | `"str" $* n` |
| `hotacc` | `total° += value` |

Built: `zymbol-lang-0.1.2-2026-05-04.vsix`

---

## Summary of changed files

| File | Type | Change |
|------|------|--------|
| `crates/zymbol-lexer/src/lib.rs` | edit | Add `FatArrow` token before `=` branch |
| `crates/zymbol-parser/src/modules.rs` | edit | `FatArrow` for import alias + export rename (×3) |
| `crates/zymbol-parser/src/match_expr.rs` | edit | `FatArrow` for match-arm separator |
| `crates/zymbol-formatter/src/lib.rs` | edit | Emit `=>` for import/export/match constructs |
| `crates/zymbol-bytecode/Cargo.toml` | edit | Add `serde`, `bincode` deps |
| `crates/zymbol-bytecode/src/lib.rs` | edit | Derive `Serialize`/`Deserialize` on all types; add `ReadLine` instruction |
| `crates/zymbol-compiler/src/lib.rs` | edit | `compile_with_dir`; compile `Statement::Input` → `ReadLine` |
| `crates/zymbol-compiler/src/def_use.rs` | edit | `ReadLine` def-use arm |
| `crates/zymbol-standalone/src/lib.rs` | edit | Replace `write_source()` with `write_bytecode()`; add `new_from_source(base_dir)` |
| `crates/zymbol-vm/src/lib.rs` | edit | `ReadLine` handler; `EnterTui` cursor fix |
| `crates/zymbol-interpreter/src/io.rs` | edit | `execute_input` raw-mode suspend; `execute_tui_block` cursor fix |
| `crates/zymbol-interpreter/src/functions_lambda.rs` | edit | Capture `import_aliases` in named-fn HOF values |
| `crates/zymbol-semantic/src/type_check.rs` | edit | `(Float, Int) => true` in `types_compatible_static` |
| `tests/scripts/expected_compare.sh` | edit | Typed wildcards + `--regen --smart` |
| `tests/scripts/semantic_compare.sh` | edit | Typed wildcards |
| `tests/scripts/vm_compare.sh` | edit | Restore `tests/manual/` (466 files total) |
| `tests/bugs/bug_named_fn_module_alias_hof.zy` | new | GAP-Z009 regression test |
| `tests/bugs/bug_semantic_numeric_recursive.zy` | new | BUG-007 regression test |
| All `.zy` files with `:` as separator | edit | Migrate to `=>` |
| `vscode/syntaxes/zymbol.tmGrammar.json` | edit | `FatArrow`; `$*` collection operator |
| `vscode/snippets/zymbol.json` | edit | `outps`, `outpc`, `repeat`, `hotacc` snippets |

CLI (`zymbol-cli`) · AST · REPL: **no changes required**.
Lexer, parser, and formatter changes are confined to `FatArrow` (Feature 1) — no new
constructs beyond the separator token.

---

## Test plan

| Feature | Test | What to verify |
|---------|------|----------------|
| 1 | All existing module + match tests | `<# path => alias` parses; `pattern => arm` evaluates correctly; old `:` syntax yields clear error |
| 2 | `zymbol build serpiente.zy -o out/` | Standalone executes; binary ≤ 1 MB |
| 3 | `expected_compare.sh --regen --smart tests/stress_v2/` | Timing lines replaced by `***time***`; subsequent compare passes |
| 4 | `tests/bugs/bug_named_fn_module_alias_hof.zy` | TW + VM both pass |
| 5 | `tests/i18n/test_http_api.zy --vm` | `<<` prompt + read executes in VM mode |
| 6 | `tests/bugs/bug_semantic_numeric_recursive.zy` | `zymbol check` exits 0 on `gcd`-style functions |
| 7 | `tests/manual/tui/05_key_input.zy` | `<<` no longer freezes inside `>>|` (manual only — requires TTY) |
| 8 | `tests/manual/tui/04_output_pos.zy` | Content appears at row 1 col 1 on TUI entry (manual only) |

---

## ✅ Conclusion — v0.0.6 RELEASED (2026-06-07)

All features in this plan are implemented and shipped. QA pass complete; version bumped to 0.0.6.

| # | Feature | Status |
|---|---------|--------|
| 0 | `FatArrow` `=>` — universal maps-to operator | ✅ complete — full source migration done |
| 1 | Standalone binaries embed bytecode via `bincode` | ✅ complete — 2.4× size reduction validated |
| 2 | Typed wildcards `***time***` / `***date***` / etc. | ✅ complete — `--regen --smart` operational |
| 3 | GAP-Z009 named-fn module-alias HOF retention | ✅ complete — regression test added |
| 4 | VM `<<` input support (`ReadLine` instruction) | ✅ complete — numeric cast + TUI mode handled |
| 5 | BUG-007 semantic recursive integer functions | ✅ complete — `(Float, Int) => true` fix |
| 6 | TUI-FIX-01 `<<` inside `>>|` freezes terminal | ✅ complete — raw-mode suspend/restore |
| 7 | TUI-FIX-02 `>>|` cursor not at (1,1) on entry | ✅ complete — `MoveTo(0,0)` after alternate screen |

Test coverage at release: **478 / 478 E2E files PASS (0 SKIP)**; `cargo test` **820 passed, 0 failed, 4 ignored**.
Validated by `bash tests/scripts/vm_compare.sh` (0 FAIL).
