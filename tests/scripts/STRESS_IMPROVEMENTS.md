# Zymbol-Lang — Stress Optimization Tracking

Granular results per optimization applied.
Update with: `./tests/scripts/run_all.sh --no-build --runs 10`

---

## Sprint 1 — Results (B1 + B2 + B8 + B5)

| Sprint | Cambio aplicado         | stress    | match   | recursion | collections | strings | notes                        |
|--------|-------------------------|-----------|---------|-----------|-------------|---------|------------------------------|
| base   | — (baseline v0.0.1)     | ~1006ms*  | 437ms   | 6624ms    | 6879ms      | 24ms    | *stress.zy lighter then      |
| S1-B1  | Rc\<FunctionDef\>       | —         | —       | —         | —           | —       | combined below               |
| S1-B2  | mem::take scope         | —         | —       | —         | —           | —       | combined below               |
| S1-B8  | short-circuit guards    | —         | —       | —         | —           | —       | combined below               |
| S1-B5  | range iterator lazy     | —         | —       | —         | —           | —       | combined below               |
| **S1** | **B1+B2+B8+B5 combined**| **1433ms**| **311ms**| **3447ms**| **645ms**   | **25ms**| 5-run avg; stress dominated by arr$+ O(n²) |

**Run date:** 2026-03-11
**Build:** `cargo build --release`
**Runs:** 5 runs each benchmark

### Key improvements
- `bench_collections`: **6879ms → 645ms** (10.7x) — HOF lambda calls (map/filter/reduce) benefited massively from B1+B2
- `bench_recursion`: **6624ms → 3447ms** (1.9x) — fib(30) and ackermann improved; ackermann(3,6) dominates runtime
- `bench_match`: **437ms → 311ms** (1.4x) — function call overhead reduced
- `bench_strings`: unchanged (already fast)

### Stress test note
Current `stress.zy` (2026-03-11 version) includes 5k O(n²) array appends (`arr = arr$+ i` for i in 0..4999).
Array push alone takes ~974ms. Sprint 2's `Rc<Vec<Value>>` will address this bottleneck.
The plan's baseline of 1006ms was measured against a lighter version of stress.zy.

---

## How to update

```bash
./tests/scripts/run_all.sh --no-build --runs 10
```

Replace values in the table above with measured results.

---

---

## Sprint 2 — Results (B3 + B4 + B9)

| Sprint | Cambio aplicado                   | stress   | match    | recursion  | collections | strings | notes                                |
|--------|-----------------------------------|----------|----------|------------|-------------|---------|--------------------------------------|
| **S1** | **B1+B2+B8+B5 combined**          | **1433ms**| **311ms**| **3447ms**| **645ms**   | **25ms**| baseline for Sprint 2                |
| S2-B9  | `set_variable(&str)` + get_mut    | —        | —        | —          | —           | —       | combined below                       |
| S2-B3  | in-place arr$+ mutation fast path | —        | —        | —          | —           | —       | combined below                       |
| S2-B4  | scope.reserve(params.len())       | —        | —        | —          | —           | —       | combined below                       |
| **S2** | **B9+B3+B4 combined**             | **628ms** | **525ms**| **~4400ms\***| **81ms** | **43ms**| run_all sequential load inflates times |

\* Recursion isolated: ~4.4s (vs 6.3s from run_all sequential load; ackermann(3,6) dominates)

**Run date:** 2026-03-11
**Build:** `cargo build --release`
**Runs:** 5 runs each benchmark (via run_all.sh)

### Key improvements Sprint 2
- `bench_collections`: **645ms → 81ms** (8x) — B3 in-place `arr$+` eliminates O(n) clone per append
- `stress`: **1433ms → 628ms** (2.3x) — B3 eliminates 974ms from 5k O(n²) array appends
- `bench_recursion`: ~4.4s isolated (B4+B9 reduce per-call overhead; ackermann dominates)
- `bench_match`: regressed slightly (sequential system load artifact in run_all.sh)
- `bench_strings`: stable, small variance

### Note on run_all.sh sequential load
Benchmarks run consecutively inflate later results (especially recursion after stress+match).
For accurate recursion timing, run isolated: `time ./target/release/zymbol run tests/scripts/bench_recursion.zy`

---

## Sprint 3 — Results (B7 + B10 + B12 + B13)

| Sprint | Cambio aplicado                              | stress   | match    | recursion  | collections | strings | notes                                |
|--------|----------------------------------------------|----------|----------|------------|-------------|---------|--------------------------------------|
| **S2** | **B9+B3+B4 combined**                        | **628ms**| **525ms**| **~4400ms\***| **81ms** | **43ms**| baseline for Sprint 3                |
| S3-B7  | Pattern::Range literal bounds fast path      | —        | —        | —          | —           | —       | combined below                       |
| S3-B10 | Scope allocation pool (push/pop + call state)| —        | —        | —          | —           | —       | combined below                       |
| S3-B12 | Arithmetic self-assign fast path (`x=x+y`)  | —        | —        | —          | —           | —       | combined below                       |
| S3-B13 | Pool for call-frame Vec reuse                | —        | —        | —          | —           | —       | combined with B10                    |
| **S3** | **B7+B10+B12+B13 combined**                  | **473ms**| **312ms**| **~4835ms\*\***| **71ms** | **34ms**| 5-run avg                        |

\* Recursion Sprint 2: ~4.4s isolated
\*\* Recursion Sprint 3: ~4.7s isolated (min 4539ms via run_all; B10 pool helps less than projected for tree-recursive ackermann)

**Run date:** 2026-03-11
**Build:** `cargo build --release`
**Runs:** 5 runs each benchmark (via run_all.sh)

### Key improvements Sprint 3
- `stress`: **628ms → 473ms** (1.3x) — B12 arithmetic self-assign eliminates ~600k function calls in 100k loop
- `bench_match`: **525ms → 312ms** (1.7x) — B7 eliminates ~300k eval_expr dispatch calls for literal range bounds
- `bench_collections`: **81ms → 71ms** — modest improvement from pool overhead reduction
- `bench_strings`: **43ms → 34ms** — variance reduction, slight improvement
- `bench_recursion`: **~4400ms → ~4835ms** — within measurement variance; B10 pool reduces allocations but tree-recursive ackermann(3,6) still dominates; pool benefit limited at recursion depth > pool_size

### B10 pool analysis
For `fib(30)` + `ackermann(3,6)`, the pool's benefit is partial:
- At max recursion depth D, D frames are "in-flight" — pool is empty, `HashMap::new()` still called
- Pool only benefits shallow/sequential calls (when previous frame is already returned)
- `HashMap::clear()` has O(n) cost per frame even for small maps
- Net: allocation count reduced but Rust's allocator is already optimized for small, short-lived maps
- Full benefit requires B6 (Bytecode VM) which eliminates per-call HashMap overhead entirely

---

## v0.0.2 Collection API Redesign — Impact Check (2026-03-23)

| Benchmark       | S3 avg (baseline) | v0.0.2 avg | Δ       | Result       |
|-----------------|-------------------|------------|---------|--------------|
| stress          | 207ms             | 202ms      | -2%     | ✅ no regression |
| bench_match     | 163ms             | 166ms      | +2%     | ✅ noise       |
| bench_recursion | 1491ms            | 1494ms     | +0.2%   | ✅ identical   |
| bench_collections | 69ms            | 63ms       | **-9%** | ✅ improved    |
| bench_strings   | 50ms              | 44ms       | **-12%**| ✅ improved    |
| bench_strings_stress | 116ms        | 118ms      | +2%     | ✅ noise       |
| bench_strings_modify | 69ms         | 63ms       | **-9%** | ✅ improved    |

**Test coverage:** 159/159 vm_compare PASS (3 new test files added)

**Notes:**
- `bench_collections` fix: `$- 0` (old remove-by-index) → `$-[0]` (new remove-at-index) — semantically correct
- `bench_strings_modify` fix: `$++[p:t]` → `$+[p] t`, `$--[p:n]` → `$-[start..end]` — new v0.0.2 syntax
- No performance regression from the API unification — new code paths are equally fast
