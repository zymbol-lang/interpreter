#!/usr/bin/env python3
"""STRESS V2 — Higher-Order Functions
Optimal Python: list comprehensions, generator expressions, sorted().
NOT using reduce(lambda) where sum/max builtins exist — that IS Python optimal.
Comparison: tests/stress_v2/bench_hof.zy

Run: python3 tests/stress_v2/bench_hof.py
"""
import time

print("=== Python HOF v2 ===")
t0 = time.perf_counter()

nums = list(range(1, 10001))  # 1..10000 inclusive, O(1) amortized build

# H1: map — square all elements (20 passes)
t = time.perf_counter()
total = 0
for _ in range(20):
    sq = [x * x for x in nums]
    total += len(sq)
elapsed = time.perf_counter() - t
print(f"H1_map_square:      reps=20 len={total}  ({elapsed:.3f}s)")

# H2: filter — keep multiples of 7 (20 passes)
t = time.perf_counter()
total = 0
for _ in range(20):
    sevens = [x for x in nums if x % 7 == 0]
    total += len(sevens)
elapsed = time.perf_counter() - t
print(f"H2_filter_mod7:     reps=20 len={total}  ({elapsed:.3f}s)")

# H3: reduce — sum all (50 passes)
# Python optimal: sum() (C-level). Note: Zymbol uses lambda HOF dispatch here.
t = time.perf_counter()
total = 0
for _ in range(50):
    s = sum(nums)
    total += s
elapsed = time.perf_counter() - t
print(f"H3_reduce_sum:      reps=50 result={total}  ({elapsed:.3f}s)")

# H4: filter → reduce (generator, no intermediate list — Python optimal)
t = time.perf_counter()
total = 0
for _ in range(10):
    even_sum = sum(x for x in nums if x % 2 == 0)
    total += even_sum
elapsed = time.perf_counter() - t
print(f"H4_filter_reduce:   reps=10 result={total}  ({elapsed:.3f}s)")

# H5: map → reduce (generator, no intermediate list)
t = time.perf_counter()
total = 0
for _ in range(10):
    sum_sq = sum(x * x for x in nums)
    total += sum_sq
elapsed = time.perf_counter() - t
print(f"H5_map_reduce:      reps=10 result={total}  ({elapsed:.3f}s)")

# H6: sort ascending natural (10 passes)
t = time.perf_counter()
for _ in range(10):
    asc = sorted(nums)
elapsed = time.perf_counter() - t
print(f"H6_sort_asc:        reps=10 done  ({elapsed:.3f}s)")

# H7: sort with custom key — descending (5 passes)
t = time.perf_counter()
for _ in range(5):
    desc = sorted(nums, reverse=True)
elapsed = time.perf_counter() - t
print(f"H7_sort_custom:     reps=5 done  ({elapsed:.3f}s)")

# H8: filter → map → sort (full pipeline, 5 passes)
t = time.perf_counter()
total = 0
for _ in range(5):
    pipeline = sorted([x * 2 for x in nums if x % 3 == 0], reverse=True)
    total += len(pipeline)
elapsed = time.perf_counter() - t
print(f"H8_filter_map_sort: reps=5 len={total}  ({elapsed:.3f}s)")

total_time = time.perf_counter() - t0
print(f"=== Done ({total_time:.3f}s total) ===")
