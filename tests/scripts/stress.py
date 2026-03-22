#!/usr/bin/env python3
"""Python Stress Test — equivalent to tests/scripts/stress.zy
Provides a reference baseline for comparing Zymbol-Lang performance.

Run: python3 tests/scripts/stress.py
"""

import time

print("=== Python Stress Test ===")

t0 = time.perf_counter()

# --- 1. Arithmetic loop (100k iterations) ---
t = time.perf_counter()
s = 0
for i in range(100000):
    s = s + i
elapsed = time.perf_counter() - t
print(f"arithmetic: sum(0..99999) = {s}  ({elapsed:.3f}s)")

# --- 2. Nested loop (500 x 500 = 250k iterations) ---
t = time.perf_counter()
count = 0
for i in range(500):
    for j in range(500):
        count += 1
elapsed = time.perf_counter() - t
print(f"nested_loop: {count} iterations  ({elapsed:.3f}s)")

# --- 3. String concatenation (3k single-char appends) ---
t = time.perf_counter()
s = ""
for i in range(3000):
    s = s + "a"
elapsed = time.perf_counter() - t
print(f"string_concat: len = {len(s)}  ({elapsed:.3f}s)")

# --- 4. Array push (5k elements) ---
t = time.perf_counter()
arr = []
for i in range(5000):
    arr.append(i)
elapsed = time.perf_counter() - t
print(f"array_push: len = {len(arr)}  ({elapsed:.3f}s)")

# --- 5. Fibonacci recursive (fib 25) ---
t = time.perf_counter()
def fib(n):
    if n <= 1:
        return n
    return fib(n - 1) + fib(n - 2)

fib_result = fib(25)
elapsed = time.perf_counter() - t
print(f"fib(25) = {fib_result}  ({elapsed:.3f}s)")

# --- 6. Array contains (1k inserts + 1k lookups) ---
t = time.perf_counter()
lookup_arr = list(range(1000))
hits = 0
for i in range(1000):
    if i in lookup_arr:
        hits += 1
elapsed = time.perf_counter() - t
print(f"array_contains: {hits} hits  ({elapsed:.3f}s)")

total = time.perf_counter() - t0
print(f"=== Done ({total:.3f}s total) ===")
