# VM Register Parity — Implementation Plan

> Goal: bring the VM from 285/320 to 320/320 on `vm_compare.sh`.
> 35 failing tests grouped in 7 categories.

---

## Sprint 1 — Standalone operators (10 tests, all S)

### Phase 1 — Numeric Casts `##.` `###` `##!` (6 tests)

**Failing**: `casts/01..06`  
**Root cause**: `Expr::NumericCast` in `compile_expr` returns `Err(Unsupported)`.

**Changes**:
1. `zymbol-bytecode/src/lib.rs` — add two instructions in `// ── Type conversion`:
   ```rust
   FloatToIntRound(Reg, Reg),   // f.round() as i64
   FloatToIntTrunc(Reg, Reg),   // f.trunc() as i64
   ```
2. `zymbol-vm/src/lib.rs` — add execution arms after `IntToFloat`:
   ```rust
   Instruction::FloatToIntRound(dst, src) => { wreg!(dst, Value::Int(rf!(src).round() as i64)); }
   Instruction::FloatToIntTrunc(dst, src) => { wreg!(dst, Value::Int(rf!(src).trunc() as i64)); }
   ```
3. `zymbol-compiler/src/lib.rs` — replace `Expr::NumericCast(_) => Err(Unsupported)`:
   - `CastKind::ToFloat`     → emit `IntToFloat(dst, r_src)`
   - `CastKind::ToIntRound`  → emit `FloatToIntRound(dst, r_src)`
   - `CastKind::ToIntTrunc`  → emit `FloatToIntTrunc(dst, r_src)`
4. `zymbol-compiler/src/lib.rs` — add arms in `eliminate_dead_code` for both new instructions.

---

### Phase 2 — ConcatBuild `$++` (2 tests)

**Failing**: `strings/15_concat_build`, `gaps/g17_*`  
**Root cause**: `Expr::ConcatBuild` returns `Err(Unsupported)`.

**Changes**:
1. `zymbol-bytecode/src/lib.rs` — add in `// ── String ops`:
   ```rust
   ConcatBuild(Reg, Reg, Vec<Reg>),  // dst, base, [items]
   ```
2. `zymbol-vm/src/lib.rs` — add execution arm:
   ```rust
   Instruction::ConcatBuild(dst, base, items) => {
       match rreg!(*base).clone() {
           Value::String(s) => {
               let mut r = (*s).clone();
               for &reg in items { r.push_str(&rreg!(reg).to_string_repr()); }
               wreg!(*dst, Value::String(Rc::new(r)));
           }
           Value::Array(arr) => {
               let mut a = (*arr).clone();
               for &reg in items { a.push(rreg!(reg).clone()); }
               wreg!(*dst, Value::Array(Rc::new(a)));
           }
           _ => raise!(VmError::TypeError { ... })
       }
   }
   ```
3. `zymbol-compiler/src/lib.rs` — replace `Expr::ConcatBuild(_) => Err(Unsupported)`:
   ```rust
   Expr::ConcatBuild(op) => {
       let r_base = self.compile_expr(&op.base, ctx)?;
       let item_regs: Vec<Reg> = op.items.iter()
           .map(|e| self.compile_expr(e, ctx))
           .collect::<Result<_, _>>()?;
       let dst = ctx.alloc_temp()?;
       ctx.emit(Instruction::ConcatBuild(dst, r_base, item_regs));
       Ok(dst)
   }
   ```
4. Add arm in `eliminate_dead_code`.

---

### Phase 3 — `@N` Times with variable (1 test)

**Failing**: `loops/04_times.zy`  
**Root cause**: `compile_loop` only detects TIMES when condition is `Expr::Literal(Int)`. When `reps` is a variable, it falls through to `compile_while_loop` which re-evaluates the condition as boolean — no counter is managed.

**Changes** (`zymbol-compiler/src/lib.rs` only):

Add `compile_dynamic_times_loop` method. When `lp.condition` is not a literal int:
```rust
fn compile_dynamic_times_loop(&mut self, lp: &Loop, ctx: &mut FunctionCtx) -> Result<(), CompileError> {
    // r_n = eval(condition)   — evaluated ONCE
    let r_n = self.compile_expr(cond, ctx)?;
    // r_i = 0
    let r_i = ctx.alloc_reg_named("__times_i")?;
    ctx.emit(Instruction::LoadInt(r_i, 0));
    // loop_start:
    let loop_start = ctx.current_label();
    // r_cmp = r_i >= r_n
    let r_cmp = ctx.alloc_temp()?;
    ctx.emit(Instruction::CmpGe(r_cmp, r_i, r_n));
    let end_patch = ctx.emit_jump_if_placeholder(r_cmp);
    // [body]
    self.compile_block(&lp.body, ctx)?;
    // r_i += 1
    ctx.emit(Instruction::AddIntImm(r_i, r_i, 1));
    ctx.emit(Instruction::Jump(loop_start as Label));
    // loop_end:
    let loop_end = ctx.current_label();
    ctx.patch_jump(end_patch, loop_end);
    Ok(())
}
```

---

### Phase 5 — Unicode eval `#|x|` (1 test)

**Failing**: `strings/16_unicode_eval.zy`  
**Root cause**: VM's `NumericEval` arm only tries `parse::<i64>()` and `parse::<f64>()` without Unicode normalization.

**Changes** (`zymbol-vm/src/lib.rs`):

Add `normalize_unicode_digits` (copy from `zymbol-interpreter/src/data_ops.rs:19`) and call it before ASCII parse in `Instruction::NumericEval`:
```rust
Instruction::NumericEval(dst, src) => {
    let s = ri!(src).to_string_repr();
    let normalized = normalize_unicode_digits(&s);
    let val = if let Ok(n) = normalized.parse::<i64>() {
        Value::Int(n)
    } else if let Ok(f) = normalized.parse::<f64>() {
        Value::Float(f)
    } else {
        raise!(VmError::NumericEval { got: s.to_string() })
    };
    wreg!(*dst, val);
}
```

---

## Sprint 2 — Patterns and Lambdas (10 tests)

### Phase 4 — Match `Pattern::List` (3 tests)

**Failing**: `match/08_list_pattern_exact`, `match/09_list_pattern_wildcard`, `match/10_list_pattern_length`  
**Root cause**: `Pattern::List(_, _)` arm in `compile_match_expr` is a no-op `{}`.

**No new bytecode instructions needed** — uses `ArrayLen`, `ArrayGet`, `CmpEq`, `CmpEqImm`, `JumpIfNot`.

**Changes** (`zymbol-compiler/src/lib.rs`):

Replace the empty `Pattern::List` arm with:
1. `ArrayLen(r_len, r_sub)` — get scrutinee length
2. `LoadInt(r_expected, patterns.len())` + `CmpEq(r_ok, r_len, r_expected)` + `JumpIfNot(r_ok, next_case)`
3. For each sub-pattern at index `i`:
   - `LoadInt(r_idx, i+1)` + `ArrayGet(r_elem, r_sub, r_idx)`
   - `Pattern::Wildcard` → skip (always matches)
   - `Pattern::Literal(Int(n))` → `CmpEqImm(r_cmp, r_elem, n)` + `JumpIfNot(r_cmp, next_case)`
   - `Pattern::Literal(String(s))` → `MatchStr(r_elem, idx, next_case)`
4. Emit body + `Jump(match_end_label)`
5. Patch all skip jumps to `next_case`

Nested `Pattern::List` in sub-patterns can be left unimplemented for now (not tested).

---

### Phase 6 — Complex Lambdas (7 tests)

**Failing**: `lambdas/15,17,18,19,21,23,26`

**Step 6.0 — Diagnose before implementing**

Run each test individually to identify sub-categories:
```bash
for f in 15 17 18 19 21 23 26; do
    echo "=== lambdas/${f}_* ==="
    ./target/release/zymbol run --vm tests/lambdas/${f}_*.zy 2>&1 | head -5
done
```

**Known sub-problems based on static analysis**:

**6A — `BinaryOp::Pipe` / `|>` operator** (likely tests 21, 23):
In `compile_binary`, `BinaryOp::Pipe => Err(Unsupported)`. Implement as:
```rust
BinaryOp::Pipe => {
    // val |> f → f(val)
    // right must be callable (Identifier or lambda)
    let r_left = self.compile_expr(&bin.left, ctx)?;
    let r_fn = self.compile_expr(&bin.right, ctx)?;
    let dst = ctx.alloc_temp()?;
    ctx.emit(Instruction::CallDynamic(dst, r_fn, vec![r_left]));
    Ok(dst)
}
```

**6B — Block lambdas with `<~`** (tests 17, 18):
Verify `compile_lambda` for `LambdaBody::Block` correctly handles `Statement::Return(r)`.
Currently adds `LoadUnit + Return` after the block regardless — this is OK because the block's own `Return(r)` from `<~` executes first and the VM exits the frame. Check if `compile_block` translates `Statement::Return` to `Instruction::Return`.

**6C — Closures that modify captured state** (tests 18, 19, 26):
The VM captures by value (snapshot of register at closure creation time). If tests expect mutations to propagate back to the enclosing scope, this is a fundamental semantic difference.
Mitigation: check if the WT also captures by value or by reference (look at `functions_lambda.rs`). If by reference, implement upvalue cells (Lua-style) in the VM.

---

## Sprint 3 — Module System Part 1 (7 tests)

### Phase 7D — Circular Import Detection (4 tests)

**Failing**: `modules_scope/circ_mod_a,b,c`, `test_circular_modules`  
**Type**: `[Tree error]` — WT detects circular import and errors; VM presumably loops or silently fails.

**Changes** (`zymbol-compiler/src/lib.rs`):

Add `loading_stack: HashSet<PathBuf>` to `Compiler` struct. In `compile_import`:
```rust
if self.loading_stack.contains(&resolved_path) {
    return Err(CompileError::CircularImport(resolved_path.display().to_string()));
}
self.loading_stack.insert(resolved_path.clone());
// ... compile module ...
self.loading_stack.remove(&resolved_path);
```

Add `CircularImport(String)` variant to `CompileError`. The error message must match the WT's `RuntimeError::CircularImport` format (check `test_circular_modules.expected`).

---

### Phase 7A — Intra-Module Function Calls (1 test + cascading)

**Failing**: `bugs/bug01_module_intra_calls.zy`  
**Root cause**: Only exported functions are registered in `function_index` under `alias::funcname`. Private functions (not in `#>` block) are invisible to the compiler when it compiles the exported functions' bodies.

**Changes** (`zymbol-compiler/src/lib.rs`):

In `compile_import`, before the main compilation pass, pre-register ALL functions of the module (exported + private) using an internal naming scheme. Then compile each function with access to all intra-module names:

```rust
// Phase 1: reserve slots for ALL module functions (exported + private)
let all_module_funcs: Vec<&FunctionDecl> = module_prog.statements.iter()
    .filter_map(|s| if let Statement::FunctionDecl(d) = s { Some(d) } else { None })
    .collect();

// Register private functions with internal names: "__module_priv__{alias}__{name}"
for func in &all_module_funcs {
    if !exported_names.contains(&func.name.as_str()) {
        let internal_name = format!("__priv__{}__{}", alias, func.name);
        let idx = self.functions.len() as FuncIdx;
        self.functions.push(Chunk::new(&internal_name));
        self.function_index.insert(internal_name.clone(), idx);
        // Also register by short name for intra-module lookup
        self.function_index.insert(func.name.clone(), idx); // TEMP: short name
    }
}

// Phase 2: compile all functions with short names available
// ... (ensure compile_function can find both exported and private names)
```

Note: the short-name registration must be scoped to the module compilation to avoid polluting the global function namespace. A `module_local_functions: HashMap<String, FuncIdx>` parameter to `compile_function` is cleaner.

---

## Sprint 4 — Module System Part 2 (4 tests)

### Phase 7C — Nested Imports and Import Aliases in Top-Level Functions (G17 + i18n)

**Failing**: `gaps/g17_*`, `i18n/한국_앱`, `i18n/Ελλ`, `i18n/אפל`, `i18n/test_http_api`, `i18n/test_modulos_sistema`

**Root cause (G17)**: The compiler processes `program.imports` before compiling top-level functions, so `alias::fn` should be in `function_index`. If failing, check:
1. Is `program.imports` populated when the compiler receives it?
2. Does `compile_import` run before `compile_function` for top-level functions?

**Root cause (i18n)**: Modules with Unicode filenames or BashExec — check if path resolution handles non-ASCII paths, and if BashExec expressions in imported modules are compiled correctly.

**Changes**: Likely debug-only — trace through what `program.imports` contains when `compile_program` starts.

### Phase 7E — Module Scope Isolation (test_module_scope)

**Failing**: `modules_scope/test_module_scope.zy`  
**Root cause**: Module functions and top-level code may share register space if the compiler does not create isolated `FunctionCtx` instances for each module compilation.

Verify that `compile_import` creates a fresh `FunctionCtx` for each module function — it should, since `compile_function` creates its own `FunctionCtx`. If the shared `global_consts` or `function_index` is leaking state between modules, scope it properly.

---

## Sprint 5 — Module Private State (3 tests)

### Phase 7B — Module-Level Mutable Variable Persistence (G13, G14, and related)

**Failing**: `gaps/g13_module_private_state`, `gaps/g14_export_block_position`  
**Root cause**: The VM has no persistent mutable state per module. Functions are pure chunks with no shared state.

**Architecture change** — this is the most invasive phase:

1. `zymbol-bytecode/src/lib.rs` — add to `CompiledProgram`:
   ```rust
   pub module_var_init: Vec<(String, Vec<(String, LiteralValue)>)>, // module_alias → [(var_name, init_val)]
   ```
   Add new instructions:
   ```rust
   LoadModuleVar(Reg, u16, u16),   // dst, module_idx, var_idx
   StoreModuleVar(u16, u16, Reg),  // module_idx, var_idx, src
   ```

2. `zymbol-vm/src/lib.rs` — add to `Vm` struct:
   ```rust
   module_state: Vec<Vec<Value>>,  // module_idx → [var values]
   ```
   Initialize from `program.module_var_init`. Implement `LoadModuleVar` / `StoreModuleVar` arms in the execution loop.

3. `zymbol-compiler/src/lib.rs` — during `compile_import`, for each `Statement::Assignment` at module top-level (mutable `=`, not const `:=`):
   - Record `(var_name, initial_value)` in `module_var_init`
   - When compiling module function bodies, replace accesses to those variables with `LoadModuleVar` / `StoreModuleVar` instead of `alloc_reg` / `CopyReg`

---

## Summary Table

| Sprint | Phase | Tests | Complexity | Files Changed |
|--------|-------|-------|------------|---------------|
| S1 | Casts `##.` `###` `##!` | 6 | S | bytecode, vm, compiler |
| S1 | ConcatBuild `$++` | 2 | S | bytecode, vm, compiler |
| S1 | `@N` times variable | 1 | S | compiler |
| S1 | Unicode eval `#\|x\|` | 1 | S | vm |
| S2 | Match `Pattern::List` | 3 | M | compiler |
| S2 | Lambdas complex | 7 | L | compiler (+ vm if upvalue cells needed) |
| S3 | Circular import detection | 4 | M | compiler |
| S3 | Intra-module calls | 1+ | M | compiler |
| S4 | Nested imports / G17 / i18n | ~6 | M | compiler |
| S4 | Scope isolation | ~1 | M | compiler + vm |
| S5 | Module private state | ~3 | L | bytecode + vm + compiler |

**Total**: 35 tests across 5 sprints.

---

## Implementation Notes

- **Dead-code eliminator**: every new `Instruction` variant must have an arm in `eliminate_dead_code` at the bottom of `zymbol-compiler/src/lib.rs` (~line 3050). Missing arms cause valid instructions to be removed.
- **`rreg!` / `wreg!` macros**: use these in the VM hot-loop for performance. Use `reg_get()` / `reg_set()` only outside the loop.
- **`CompiledProgram` is public API**: changes to it affect `zymbol-cli` and `zymbol-lsp` callers. Add fields with `Default` / `Vec::new()` to avoid breaking changes.
- **Test after each sprint**: run `bash tests/scripts/vm_compare.sh` after each sprint to confirm progress and catch regressions.
