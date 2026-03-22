#!/usr/bin/env python3
"""Python Benchmark: Collections — equivalent to bench_collections.zy
Workloads mirror Zymbol array operators: $+, $-, $?, $[..], $>, $|, $<

Run: python3 tests/scripts/bench_collections.py
"""

import time
from functools import reduce

print("=== Python Benchmark: Collections ===")

t0 = time.perf_counter()

# --- 1. Build array via append (3k elements) ---
# Mirrors: arr = arr$+ i  (Zymbol $+ clones the array; Python list.append is O(1))
t = time.perf_counter()
arr = []
for i in range(3000):   # 0..2999 inclusive = 3000 iters
    arr = arr + [i]     # force O(n) clone to match Zymbol semantics
elapsed = time.perf_counter() - t
print(f"array_build: len = {len(arr)}  ({elapsed:.3f}s)")

# --- 2. Array contains — 2k lookups in a 500-element array ---
# Mirrors: base$? (i % 500)
t = time.perf_counter()
base = list(range(500))
found = 0
for i in range(2000):   # 0..1999 inclusive = 2000 iters
    if (i % 500) in base:
        found += 1
elapsed = time.perf_counter() - t
print(f"array_contains: {found} hits  ({elapsed:.3f}s)")

# --- 3. Slice operations (1k slices on fixed 100-element array) ---
# Mirrors: src$[0..49]  (exclusive end = 49 elements)
t = time.perf_counter()
src = list(range(100))
slice_sum = 0
for i in range(1000):   # 0..999 inclusive = 1000 iters
    sl = src[0:49]      # exclusive end, matches Zymbol $[0..49]
    slice_sum += len(sl)
elapsed = time.perf_counter() - t
print(f"slice_ops: slice_sum = {slice_sum}  ({elapsed:.3f}s)")

# --- 4. Map — $> over 5k elements (single pass) ---
# Mirrors: data5k$> (x -> x * 2)
t = time.perf_counter()
data5k = list(range(5000))
doubled = list(map(lambda x: x * 2, data5k))
elapsed = time.perf_counter() - t
print(f"map_hof: output len = {len(doubled)}  ({elapsed:.3f}s)")

# --- 5. Filter — $| over 5k elements (single pass) ---
# Mirrors: data5k$| (x -> x % 2 == 0)
t = time.perf_counter()
evens = list(filter(lambda x: x % 2 == 0, data5k))
elapsed = time.perf_counter() - t
print(f"filter_hof: evens = {len(evens)}  ({elapsed:.3f}s)")

# --- 6. Reduce — $< sum over 5k elements (single pass) ---
# Mirrors: data5k$< (0, (acc, x) -> acc + x)
t = time.perf_counter()
total = reduce(lambda acc, x: acc + x, data5k, 0)
elapsed = time.perf_counter() - t
print(f"reduce_hof: sum = {total}  ({elapsed:.3f}s)")

# --- 7. Remove ops — 200 removals from 500-element array ---
# Mirrors: rem_arr$- 0  (remove element at index 0)
t = time.perf_counter()
rem_arr = list(range(500))
for i in range(200):
    rem_arr = rem_arr[1:]   # remove first element, O(n) copy like Zymbol $-
elapsed = time.perf_counter() - t
print(f"remove_ops: remaining len = {len(rem_arr)}  ({elapsed:.3f}s)")

total_time = time.perf_counter() - t0
print(f"=== Done ({total_time:.3f}s total) ===")
