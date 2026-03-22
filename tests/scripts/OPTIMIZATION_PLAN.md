# Zymbol-Lang — Optimization Plan

Based on benchmark results (10-run averages) and deep analysis of the interpreter source.

## Benchmark Baseline — v0.0.1

| Benchmark | Zymbol avg | Python avg | Ratio | Root cause |
|-----------|-----------|-----------|-------|------------|
| stress (core) | 1006ms | 74ms | ~14x | Scope-stack clone + Value clone per read |
| bench_match | 437ms | 57ms | ~8x | Literal bound re-evaluation + function call overhead |
| bench_recursion | 6624ms | 210ms | ~32x | FunctionDef clone + full scope-stack clone per call |
| bench_collections | 6879ms | 43ms | ~160x | O(n) array clone on every `$+` append |
| bench_strings | 24ms | 24ms | ~1x | Already at parity |

---

## Section A — Language-Level Changes

Changes to Zymbol's semantics that close the gap with Python's built-in primitives.

### A1. In-place mutation for assign-back pattern `arr = arr$+ x`

**Why it is slow today.**
`arr = arr$+ i` evaluates the RHS by cloning the entire `Vec<Value>` from the scope map, pushing one element, and storing the new Vec back. For a loop building N elements, this is O(N²) total allocation. Python's `list.append` is O(1) amortized.

**Proposed change.**
Detect `assign.name == lhs-of-$+` at assignment dispatch and route to an in-place path that mutates the Vec by mutable reference — no clone, no new allocation. Same observable semantics since the old binding is immediately overwritten anyway. Apply the same pattern to `$-` and `$~`.

**Impact.** `bench_collections` 6879ms → ~100ms (~68x gain). Biggest single improvement.
**Compatibility.** Fully transparent — same output, faster path.

---

### A2. Call-frame model to replace whole-scope-stack clone

**Why it is slow today.**
Every function call does `self.get_scope_stack()` which deep-clones the entire `Vec<HashMap<String, Value>>`. For fib(30) that is 2.7M full scope-stack clones.

**Proposed change.**
Because Zymbol functions have **no closures** (confirmed in source: `capture_environment` always returns empty), the callee never needs to see the caller's scope. Use `std::mem::swap` to atomically hide the caller's scope and give the callee a clean one. Zero allocation, zero copy.

**Impact.** `bench_recursion` 6624ms → ~400ms (~16x gain on recursion).
**Compatibility.** Fully transparent.

---

### A3. Native `Set`/`Map` collection type for O(1) `$?` lookups

**Why it is slow today.**
`$?` on arrays is a linear scan: up to N comparisons per lookup. Python's `in` on a set is O(1).

**Proposed change.**
Add `Value::Set(IndexSet<Value>)` with `$?` dispatching to hash lookup. As a lightweight step, detect homogeneous scalar arrays at `$?` call time and use a hash check inline.

**Impact.** Programs using arrays as sets get O(1) lookups. Moderate benchmark impact (~20ms in bench_collections after A1 is applied). Major impact on real programs.
**Compatibility.** Additive — new type, no syntax change required.

---

### A4. Tail-call optimization for accumulator recursion

**Why it is slow today.**
`sum_down(n, acc)` calls itself 1,000 times, each with a full scope save/restore. The call is in tail position.

**Proposed change.**
Detect `<~ self_func(args)` in tail position and replace with a parameter rebind + jump-to-top instead of a real call. Safe in Zymbol because there are no closures to update between tail iterations.

**Impact.** Eliminates all scope overhead for tail-recursive functions. Complements A2.
**Compatibility.** Fully transparent — tail calls already work, just become free.

---

## Section B — Interpreter-Level Optimizations

Code changes inside the interpreter, no language changes required. **Ordered by estimated impact.**

---

### B1 ★★★ `Rc<FunctionDef>` — eliminate AST body clone on every call

**File:** `functions_lambda.rs` lines ~119, ~150
**Current:**
```rust
let func_def = self.functions.get(&ident.name).cloned()  // deep AST clone!
```
Every function call deep-clones the `FunctionDef` including its `body: Block` (full AST subtree). For fib(30): 2.7M deep AST clones.

**Fix:** Wrap in `Rc<FunctionDef>`. Clone becomes a reference-count increment.
```rust
// lib.rs: HashMap<String, Rc<FunctionDef>>
let func_def = self.functions.get(&ident.name).map(Rc::clone)
```
**Difficulty:** Low | **Impact:** 3–5x on `bench_recursion`

---

### B2 ★★★ `mem::swap` scope stack — zero-copy function call/return

**File:** `functions_lambda.rs` lines 56–97, 218–298
**Current:**
```rust
let saved_scope_stack = self.get_scope_stack();       // lib.rs:364 — full clone
let saved_mutable_stack = self.mutable_vars_stack.clone();
let saved_const_stack = self.const_vars_stack.clone();
```

**Fix:**
```rust
let mut saved = std::mem::take(&mut self.scope_stack);
self.scope_stack = vec![HashMap::new()];
// ... run function body ...
self.scope_stack = saved;
```
Same for `mutable_vars_stack`, `const_vars_stack`, `import_aliases`.

**Difficulty:** Low | **Impact:** 5–10x on `bench_recursion` (compounds with B1)

---

### B3 ★★★ In-place `$+` mutation path in assignment

**Files:** `variables.rs:14–28`, `collection_ops.rs:42–65`
**Current:** `eval_collection_append` clones the Vec, pushes, returns new Vec.

**Fix:** In `execute_assignment`, pattern-match on RHS:
```rust
if let Expr::CollectionAppend(op) = &assign.value {
    if let Expr::Identifier(id) = &op.collection {
        if id.name == assign.name {
            // in-place path: get_array_mut + push, no clone
            return self.append_variable_in_place(&assign.name, &op.element, span);
        }
    }
}
```
Requires new `get_array_mut` method that returns `&mut Vec<Value>` from the scope stack.

**Difficulty:** Medium | **Impact:** 50–80x on `bench_collections`

---

### B4 ★★ Zero-clone `&Value` fast path in binary expressions

**File:** `expr_eval.rs:113` + `expressions.rs:53–115`
**Current:** Every variable read in `a + b` or `i < 100` does `.cloned()` unconditionally.

**Fix:** Add `get_variable_ref(&str) -> Option<&Value>`. In `eval_binary` when both sides are identifiers and the result type is known to be a primitive, borrow by reference and match without cloning:
```rust
if let (Expr::Identifier(l), Expr::Identifier(r)) = (&binary.left, &binary.right) {
    if let (Some(Value::Int(a)), Some(Value::Int(b))) =
        (self.get_variable_ref(&l.name), self.get_variable_ref(&r.name)) {
        return Ok(Value::Int(a + b)); // zero clone
    }
}
```
**Difficulty:** Low-Medium | **Impact:** 2–3x on `stress`, compounds on `bench_recursion`

---

### B5 ★★ Iterator-based range loop — no Vec materialization

**File:** `expr_eval.rs:68–84` (`eval_iterable`)
**Current:** `@ i:0..N` builds a full `Vec<Value>` of N elements before the first iteration.
For `@ i:0..499 { @ _j:0..499 {} }`: 500 Vec allocations × 500 elements × 24 bytes = 6MB transient.

**Fix:** In `loops.rs`, detect `loop_stmt.iterable` is an `Expr::Range` and replace `eval_iterable` with a plain integer counter:
```rust
if let Expr::Range(range) = &loop_stmt.iterable {
    let mut current = eval_range_start(...);
    loop {
        self.set_variable(&loop_stmt.var, Value::Int(current));
        self.execute_block(&loop_stmt.body)?;
        current += step;
        if current > end { break; }
    }
}
```
**Difficulty:** Low | **Impact:** 20–30% on `stress` nested loops

---

### B6 ★★ `Vec<(String, Value)>` scopes for function call frames

**File:** `lib.rs:227` (`scope_stack: Vec<HashMap<String, Value>>`)
**Current:** Each function call creates a `HashMap` for its scope. For functions with 1–3 params, `HashMap` overhead (header + bucket array + hash computation) dominates the actual work.

**Fix:** Use a two-tier scope: global scope stays `HashMap`, function call frames use `Vec<(String, Value)>`. Linear scan on 1–3 items is faster than hashing due to cache locality.
```rust
enum ScopeFrame {
    Hash(HashMap<String, Value>),
    Linear(Vec<(String, Value)>),
}
```
**Difficulty:** Medium-High | **Impact:** 1.5–2x compounding with B2

---

### B7 ★ Cache literal range bounds in match patterns

**File:** `match_stmt.rs:90–111`
**Current:** `Pattern::Range(start_expr, end_expr)` calls `eval_expr` on both bounds on every match invocation. For `bench_match`'s 50k iterations with 5 arms: 500,000 literal evaluations.

**Fix:** Pre-compute constant bounds once at match-entry time:
```rust
// Before the loop over match cases, for each Range pattern:
let bounds: Vec<Option<(i64, i64)>> = cases.iter().map(|c| {
    if let Pattern::Range(s, e, _) = &c.pattern {
        if let (Ok(Value::Int(sv)), Ok(Value::Int(ev))) =
            (eval_literal_only(s), eval_literal_only(e)) {
            return Some((sv, ev));
        }
    }
    None
}).collect();
```
**Difficulty:** Medium | **Impact:** 30–50% on `bench_match`

---

### B8 ★ Short-circuit semantic feature guards in hot paths

**File:** `variables.rs:16`, `expr_eval.rs:110`
**Current:** Every identifier read calls `check_variable_alive` (HashSet lookup) and every assignment calls `is_const` (scope scan). These are for auto-destruction and immutability tracking — features unused in most programs.

**Fix:** Add early-exit guards:
```rust
// check_variable_alive — O(1) guard
if self.dead_variables.is_empty() { return Ok(()); }

// is_const — skip loop when no constants exist
if !self.has_any_const { return false; }
```
**Difficulty:** Low | **Impact:** 10–20% on `stress`

---

### B9 ★ Reuse HOF argument Vec across lambda invocations

**File:** `collection_ops.rs:358–376` (map), `collection_ops.rs:396–419` (filter)
**Current:** `vec![element]` allocates a new 1-element Vec per element per HOF call.
For map over 5k elements: 5,000 single-element Vec allocations.

**Fix:**
```rust
let mut args = Vec::with_capacity(1);
for element in arr {
    args.clear();
    args.push(element);
    let result = self.eval_lambda_call_with_args(&func, &args, span)?;
    ...
}
```
**Difficulty:** Low | **Impact:** 5–10% on `bench_collections` after B3

---

### B10 ★ ASCII fast-path for `$#` string length

**File:** `collection_ops.rs:30`, `string_ops.rs:116,186`
**Current:** `s.chars().count()` is O(n) UTF-8 traversal even for pure-ASCII strings.

**Fix:**
```rust
let len = if s.is_ascii() { s.len() } else { s.chars().count() };
```
**Difficulty:** Low | **Impact:** Marginal on benchmarks; meaningful for large strings

---

## Implementation Roadmap

### Sprint 1 — High impact, low effort (target: recursion + stress)
| # | Change | Expected result |
|---|--------|----------------|
| B1 | `Rc<FunctionDef>` | bench_recursion: 6624ms → ~1500ms |
| B2 | `mem::swap` scope stack | bench_recursion: ~1500ms → ~400ms |
| B8 | Short-circuit guards | stress: 1006ms → ~800ms |
| B5 | Iterator range loop | stress: ~800ms → ~600ms |

### Sprint 2 — Medium effort, huge impact (target: collections)
| # | Change | Expected result |
|---|--------|----------------|
| B3/A1 | In-place `$+` mutation | bench_collections: 6879ms → ~100ms |
| B4 | Zero-clone binary fast path | stress: ~600ms → ~250ms |
| B9 | Reuse HOF arg Vec | bench_collections: ~100ms → ~80ms |

### Sprint 3 — Architectural improvements
| # | Change | Expected result |
|---|--------|----------------|
| B6 | Vec-scope for function frames | bench_recursion: ~400ms → ~200ms |
| B7 | Cache match literal bounds | bench_match: 437ms → ~200ms |
| A3 | Native Set type | real programs: O(1) membership |
| A4 | Tail-call optimization | tail-recursive: ~0 overhead |

### Projected post-optimization ratios

| Benchmark | Current | Post-Sprint1 | Post-Sprint2 | Post-Sprint3 |
|-----------|---------|-------------|-------------|-------------|
| stress | ~14x | ~8x | ~3x | ~2x |
| bench_match | ~8x | ~8x | ~8x | ~3x |
| bench_recursion | ~32x | ~3x | ~2x | ~1.5x |
| bench_collections | ~160x | ~150x | ~2x | ~1.5x |
| bench_strings | ~1x | ~1x | ~1x | ~1x |

---

## Key Lessons from MiniLux (315x speedup history)

MiniLux achieved 315x improvement (440x → 1.4x vs Python) through the same class of changes:

1. **sys time 30s→0.03s**: Confirmed that heap allocation from cloning Values on every
   variable read was the dominant cost. Fix: in-place mutation methods.
2. **`Rc<FunctionDef>`**: Eliminated deep AST clone on every function call.
3. **`get_var_ref` zero-clone reads**: Reduced `Expr::Index` from full-container-clone to
   single-element borrow. Equivalent to B4 here.
4. **`into_iter()` for function params**: Move args instead of double-cloning.
5. **Vec scope + pool recycling**: Eliminated ~240k HashMap alloc/dealloc for fib(25).
6. **Binary expression fast paths**: Eliminated 2M+ unnecessary clones for `$var op literal`.

All six apply directly to Zymbol. The main differences:
- Zymbol's array immutability (`$+` always clones) is more aggressive than MiniLux's `$array =
  $array + [x]`, making B3/A1 even more critical here.
- Zymbol functions have **guaranteed no closures**, making B2 (mem::swap) simpler to implement
  than in MiniLux (which had closures that could capture variables).
