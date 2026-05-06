# Implementation Plan — VM Parity Gaps

Tracks every remaining difference between the tree-walker and the VM.
Target: `vm_compare.sh` reports 0 FAIL, 0 unwarranted SKIP.

## ✅ COMPLETE (2026-05-05)

`vm_compare.sh` reports **423 PASS · 0 FAIL · 0 SKIP**.
All gaps implemented. Unit tests: 0 failures across all crates.

---

## Baseline (before implementation)

| Status | Count | Root cause |
|--------|-------|-----------|
| FAIL | 4 | 2 × CLI args unimplemented · 2 × error-message difference |
| SKIP | 6 | 3 × TTY-only (moved to `tests/manual/`) · 3 × unimplemented constructs |

---

## Step 0 — Move TTY-only tests to `tests/manual/`

These three tests require a real terminal and cannot be automated.
They should not appear in `vm_compare.sh` output at all.

**Files to move:**

```
tests/tui/05_key_input.zy        → tests/manual/tui/05_key_input.zy
tests/tui/06_tui_block.zy        → tests/manual/tui/06_tui_block.zy
tests/tui/07_output_pos_sparse.zy → tests/manual/tui/07_output_pos_sparse.zy
```

`vm_compare.sh` already skips them via `[vm-skip]` marker, but moving them out of
`tests/` removes the SKIP lines entirely — the report becomes cleaner and
no longer needs the marker.

Update `vm_compare.sh` if it has any hard-coded paths for these files.

---

## Gap 1 — Error message parity: undefined variable hot-definition hint

**Affects:** `errors/runtime/undefined_var.zy`, `memory05_function_error.zy` (2 FAIL → 2 PASS)

**Problem:**

| Executor | Message |
|----------|---------|
| Tree-walker | `'outer' is undefined — did you mean 'outer°' (hot definition)?` |
| VM | `undefined variable: 'outer'` |

The tree-walker appends a hot-definition hint to every undefined-variable error.
The VM does not.

**Fix — `crates/zymbol-vm/src/lib.rs`**

Find where `RaiseError` / undefined-variable messages are generated in the VM
(around the `Instruction::LoadVar` / scope lookup). Change the error string to
match the tree-walker format including the `°` hint:

```rust
// Before
format!("undefined variable: '{}'", name)

// After
format!("'{}' is undefined — did you mean '{}°' (hot definition)?", name, name)
```

Also ensure the error output prefix matches: the tree-walker prints
`Runtime error:` while the VM may use a different prefix — align them.

**Scope:** ~2 lines in `crates/zymbol-vm/src/lib.rs`.

---

## Gap 2 — TypeOf on Error values returns wrong type symbol

**Affects:** `analysis/p3e_type_model.zy` (1 SKIP → 1 PASS)

**Problem:**

The `TypeOf` instruction returns `("##_", 0)` for `Value::Error(_)`.
Expected: `("##<ErrorKind>", 0)` — e.g., `"##IndexError"`, `"##TypeError"`.

Tree-walker reference (`crates/zymbol-interpreter/src/data_ops.rs`):
```rust
Value::Error(err) => format!("##{}", err.error_type)
```

**Fix — `crates/zymbol-vm/src/lib.rs`**

Find the `Instruction::TypeOf` handler (there are two — one in main dispatch,
one in a helper). In both, add the `Value::Error` arm:

```rust
Value::Error(err) => {
    regs[dst] = Value::Tuple(Arc::new(vec![
        Value::Str(Arc::from(format!("##{}", err.error_type))),
        Value::Int(0),
        val.clone(),
    ]));
}
```

**Scope:** ~6 lines in `crates/zymbol-vm/src/lib.rs`, two locations (~line 1975, ~line 3095).

---

## Gap 3 — Named functions used as first-class values don't capture outer scope

**Affects:** `analysis/p5d_fn_capture_asymmetry.zy` (1 SKIP → 1 PASS)

**Problem:**

When a named function that references outer-scope variables is assigned to a
variable (`f = adder`), the VM emits `MakeFunc(dst, func_idx)` — no closure.
At call time the outer variables are gone. The tree-walker captures correctly.

**Fix — `crates/zymbol-compiler/src/lib.rs`**

When compiling an `Expr::Identifier` that resolves to a named function
**and** that function has free variables referencing the current scope,
emit `MakeClosure` instead of `MakeFunc`.

Steps:
1. After resolving the identifier to a function index, call `collect_free_vars`
   on the function body (same helper used for lambdas).
2. If `free_vars` is non-empty, load each free var into a register and emit
   `Instruction::MakeClosure(dst, func_idx, captures)`.
3. If `free_vars` is empty, keep the existing `MakeFunc`.

**Scope:** ~20 lines in `crates/zymbol-compiler/src/lib.rs`,
reusing the existing `collect_free_vars` infrastructure.

---

## Gap 4 — Hot definitions (`°`) not compiled in VM

**Affects:** `gaps/gap_hot_definition_basic.zy` (1 SKIP → 1 PASS)

**Problem:**

The `°` operator on an identifier marks it as a hot variable — it
auto-initializes to the neutral element (0 / "" / []) on first access
instead of raising an undefined-variable error.

The compiler (`crates/zymbol-compiler/src/lib.rs`) ignores `id.hot == true`
on `IdentifierExpr` and on assignment targets. The tree-walker handles this
in three places: `variables.rs:44`, `loops.rs:20`, `expressions.rs:104`.

**Fix — `crates/zymbol-compiler/src/lib.rs`**

**4a. Hot variable read (`x°` as RHS)**

In the `Expr::Identifier` branch of `compile_expr`, when `id.hot == true`:
- If the variable is already in scope: compile as normal load.
- If not in scope: emit `LoadInt(dst, 0)` (default neutral Int) and
  register it in scope at the same time, so subsequent accesses find it.

For full neutral-element inference, defer to a helper `hot_neutral(value_type)`
that returns the appropriate zero/empty instruction. Since we don't yet have
type inference, default to `LoadInt(0)` — the same as the tree-walker's
heuristic (`Value::Int(0)` for unknown hot vars, later unified by assignment).

**4b. Hot variable write (`x° = expr`)**

In the LHS assignment compilation, when the target identifier has `.hot == true`
and does NOT yet exist in scope: declare it (same as normal first assignment).
This is already the default behaviour for assignments — no change needed here.

**4c. Hot self-reference in loops (`arr° = arr°$+ item`)**

The tricky case: `arr°` appears on both sides. The RHS read must see the
currently-accumulated value (or `[]` on first iteration). This works if 4a
initialises the register on first read and subsequent reads see the updated value.

**Scope:** ~25 lines in `crates/zymbol-compiler/src/lib.rs`.

---

## Gap 5 — CLI args capture (`><`) not compiled in VM

**Affects:** `bugs/bug002_cli_args_scope.zy`, `i18n/test_cli_args.zy` (2 FAIL → 2 PASS)

**Problem:**

`Statement::CliArgsCapture` returns `CompileError::Unsupported` immediately
(`crates/zymbol-compiler/src/lib.rs:679`).

The tree-walker populates the captured variable with the process's `argv[1..]`
as a Zymbol array of strings.

**Fix — two files**

**5a. Pass CLI args through VM execution context**

In `crates/zymbol-vm/src/lib.rs`, the `Vm` struct (or its `run` entry point)
must accept `Vec<String>` for the CLI arguments and store them.

```rust
// Vm::run signature (or a new fn)
pub fn run_with_args(&mut self, program: &Program, args: Vec<String>) -> Result<()>
```

The CLI entry point in `crates/zymbol-cli/src/run.rs` already collects
`std::env::args().skip(1)` — thread this down to the VM run call.

**5b. Compile `CliArgsCapture` statement**

In `crates/zymbol-compiler/src/lib.rs`, replace the `Unsupported` error:

```rust
Statement::CliArgsCapture(cap) => {
    let dst = ctx.alloc_reg();
    ctx.emit(Instruction::LoadCliArgs(dst));
    ctx.define_var(&cap.variable, dst);
    Ok(())
}
```

**5c. Implement `Instruction::LoadCliArgs` in VM**

Add the instruction to `crates/zymbol-bytecode/src/lib.rs`:
```rust
LoadCliArgs(Reg),   // loads cli args array into register
```

In `crates/zymbol-vm/src/lib.rs`, handle it:
```rust
Instruction::LoadCliArgs(dst) => {
    let arr = self.cli_args.iter()
        .map(|s| Value::Str(Arc::from(s.as_str())))
        .collect::<Vec<_>>();
    regs[*dst] = Value::Array(Arc::new(arr));
}
```

**Scope:** ~30 lines across 3 crates
(`zymbol-bytecode/lib.rs`, `zymbol-compiler/lib.rs`, `zymbol-vm/lib.rs`, `zymbol-cli/run.rs`).

---

## Implementation order

| # | Gap | FAILs fixed | SKIPs fixed | Effort |
|---|-----|-------------|-------------|--------|
| 0 | Move TTY tests to `tests/manual/` | — | 3 | Trivial |
| 1 | Error message parity | 2 | — | Low |
| 2 | TypeOf Error | — | 1 | Low |
| 3 | Named fn capture | — | 1 | Medium |
| 4 | Hot definitions | — | 1 | Medium-High |
| 5 | CLI args capture | 2 | — | High |

Recommended order: 0 → 1 → 2 → 3 → 4 → 5.
After gap 0+1+2, `vm_compare.sh` will show 2 FAIL / 1 SKIP remaining.
After gap 3, 1 SKIP remaining.
After gap 4, 0 SKIP remaining.
After gap 5, 0 FAIL / 0 SKIP — full parity.

---

## Key files

| File | Role |
|------|------|
| `crates/zymbol-vm/src/lib.rs` | Gap 1 (error msg) + Gap 2 (TypeOf) + Gap 5c (LoadCliArgs) |
| `crates/zymbol-compiler/src/lib.rs` | Gap 3 (MakeClosure) + Gap 4 (hot defs) + Gap 5b (CliArgsCapture) |
| `crates/zymbol-bytecode/src/lib.rs` | Gap 5a (new instruction LoadCliArgs) |
| `crates/zymbol-cli/src/run.rs` | Gap 5a (thread args to VM) |
| `tests/tui/0{5,6,7}_*.zy` | Step 0 (move to `tests/manual/`) |
| `tests/scripts/vm_compare.sh` | Step 0 (update if path-specific logic exists) |

---

## Verification

After each gap, run:

```bash
cargo test
bash tests/scripts/vm_compare.sh
```

Final target: `vm_compare.sh` output shows **0 FAIL · 0 SKIP** (excluding `tests/manual/`).
