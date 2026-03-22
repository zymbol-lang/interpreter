#!/usr/bin/env python3
"""Python Benchmark: Recursion — equivalent to bench_recursion.zy
Workloads mirror Zymbol recursive function calls.

Run: python3 tests/scripts/bench_recursion.py
"""

import time
import sys

sys.setrecursionlimit(100000)

print("=== Python Benchmark: Recursion ===")

t0 = time.perf_counter()

# --- 1. Fibonacci(30) — ~2.7M recursive calls ---
def fib(n):
    if n <= 1:
        return n
    return fib(n - 1) + fib(n - 2)

t = time.perf_counter()
r1 = fib(30)
elapsed = time.perf_counter() - t
print(f"fib(30) = {r1}  ({elapsed:.3f}s)")

# --- 2. Factorial(20) ---
def fact(n):
    if n <= 1:
        return 1
    return n * fact(n - 1)

t = time.perf_counter()
r2 = fact(20)
elapsed = time.perf_counter() - t
print(f"fact(20) = {r2}  ({elapsed:.3f}s)")

# --- 3. Power(2, 20) via recursion ---
def pow_rec(base, exp):
    if exp == 0:
        return 1
    return base * pow_rec(base, exp - 1)

t = time.perf_counter()
r3 = pow_rec(2, 20)
elapsed = time.perf_counter() - t
print(f"pow(2,20) = {r3}  ({elapsed:.3f}s)")

# --- 4. Ackermann(3, 6) ---
def ackermann(m, n):
    if m == 0:
        return n + 1
    if n == 0:
        return ackermann(m - 1, 1)
    return ackermann(m - 1, ackermann(m, n - 1))

t = time.perf_counter()
r4 = ackermann(3, 6)
elapsed = time.perf_counter() - t
print(f"ackermann(3,6) = {r4}  ({elapsed:.3f}s)")

# --- 5. Accumulative sum via recursion (depth 1000) ---
def sum_down(n, acc):
    if n == 0:
        return acc
    return sum_down(n - 1, acc + n)

t = time.perf_counter()
r5 = sum_down(1000, 0)
elapsed = time.perf_counter() - t
print(f"sum_down(1000) = {r5}  ({elapsed:.3f}s)")

total = time.perf_counter() - t0
print(f"=== Done ({total:.3f}s total) ===")
