#!/usr/bin/env python3
"""Python Benchmark: Pattern Matching — equivalent to bench_match.zy
Workloads mirror the Zymbol ?? match expression.

Run: python3 tests/scripts/bench_match.py
"""

import time

print("=== Python Benchmark: Pattern Matching ===")

t0 = time.perf_counter()

# --- 1. Value match (50k iterations) ---
def classify(x):
    if x == 1: return "one"
    if x == 2: return "two"
    if x == 3: return "three"
    if x == 4: return "four"
    if x == 5: return "five"
    return "other"

t = time.perf_counter()
hits = 0
for i in range(50001):   # 0..50000 inclusive = 50001 iters
    val = (i % 6) + 1
    label = classify(val)
    if label != "other":
        hits += 1
elapsed = time.perf_counter() - t
print(f"value_match: {hits} non-other hits  ({elapsed:.3f}s)")

# --- 2. Range match (50k iterations) ---
def grade(score):
    if 90 <= score <= 100: return "A"
    if 80 <= score <= 89:  return "B"
    if 70 <= score <= 79:  return "C"
    if 60 <= score <= 69:  return "D"
    if  0 <= score <= 59:  return "F"
    return "?"

t = time.perf_counter()
a_count = 0
for i in range(50001):   # 0..50000 inclusive
    score = i % 101
    if grade(score) == "A":
        a_count += 1
elapsed = time.perf_counter() - t
print(f"range_match: {a_count} A-grades  ({elapsed:.3f}s)")

# --- 3. Nested match (20k iterations) ---
def quadrant(x, y):
    if 0 <= x <= 100:
        return "Q1" if 0 <= y <= 100 else "Q4"
    else:
        return "Q2" if 0 <= y <= 100 else "Q3"

t = time.perf_counter()
q1_count = 0
for i in range(20001):   # 0..20000 inclusive
    x = i % 200
    y = i % 150
    if quadrant(x, y) == "Q1":
        q1_count += 1
elapsed = time.perf_counter() - t
print(f"nested_match: {q1_count} Q1 hits  ({elapsed:.3f}s)")

total = time.perf_counter() - t0
print(f"=== Done ({total:.3f}s total) ===")
