#!/usr/bin/env python3
"""Python Benchmark: Recursion vs Loop — equivalent to bench_recursion_loop.zy
Mirrors every variant with call counters to show exact call counts.

Run: python3 tests/scripts/bench_recursion_loop.py
"""

import time
import sys
sys.setrecursionlimit(10_000_000)

# ─── call counters ────────────────────────────────────────────────────────────
calls = {}

def reset(name):
    calls[name] = 0

def count(name):
    calls[name] += 1

def report(name):
    return f"({calls[name]:,} calls)"

# ─── Fibonacci — recursivo ────────────────────────────────────────────────────
def fib_rec(n):
    count("fib_rec")
    if n <= 1:
        return n
    return fib_rec(n - 1) + fib_rec(n - 2)

# ─── Fibonacci — iterativo (función wrapper, 1 call + N loops) ────────────────
def fib_loop(n):
    count("fib_loop")
    if n <= 1:
        return n
    a, b = 0, 1
    for _ in range(2, n + 1):        # N-1 iterations
        a, b = b, a + b
    return b

# ─── Fibonacci — puro loop (sin función, 0 calls) ─────────────────────────────
# (inline — no function call at all)

# ─── Ackermann — recursivo ────────────────────────────────────────────────────
def ackermann_rec(m, n):
    count("ackermann_rec")
    if m == 0:
        return n + 1
    if n == 0:
        return ackermann_rec(m - 1, 1)
    return ackermann_rec(m - 1, ackermann_rec(m, n - 1))

# ─── Ackermann — iterativo con stack explícito ────────────────────────────────
def ackermann_loop(m_init, n_init):
    count("ackermann_loop")
    stack = [m_init]
    n = n_init
    iters = 0
    while stack:
        iters += 1
        m = stack.pop()
        if m == 0:
            n += 1
        elif n == 0:
            stack.append(m - 1)
            n = 1
        else:
            stack.append(m - 1)
            stack.append(m)
            n -= 1
    calls["ackermann_loop_iters"] = iters
    return n

# ─── run ──────────────────────────────────────────────────────────────────────
print("=== Python Benchmark: Recursion vs Loop ===")

t_total = time.perf_counter()

# fib_rec(30)
reset("fib_rec")
t = time.perf_counter()
r1 = fib_rec(30)
elapsed = time.perf_counter() - t
print(f"fib_rec(30)        = {r1}  ({elapsed:.3f}s)  {report('fib_rec')}")

# fib_loop(30)
reset("fib_loop")
t = time.perf_counter()
r2 = fib_loop(30)
elapsed = time.perf_counter() - t
print(f"fib_loop(30)       = {r2}  ({elapsed:.3f}s)  {report('fib_loop')} + 29 loop iters")

# fib_pure(30) — inline, 0 function calls
t = time.perf_counter()
a, b = 0, 1
for _ in range(2, 31):
    a, b = b, a + b
elapsed = time.perf_counter() - t
print(f"fib_pure(30)       = {b}  ({elapsed:.6f}s)  (0 calls, 29 loop iters)")

print()

# ackermann_rec(3,6)
reset("ackermann_rec")
reset("ackermann_loop")
t = time.perf_counter()
r4 = ackermann_rec(3, 6)
elapsed = time.perf_counter() - t
print(f"ackermann_rec(3,6)  = {r4}  ({elapsed:.3f}s)  {report('ackermann_rec')}")

# ackermann_loop(3,6)
t = time.perf_counter()
r5 = ackermann_loop(3, 6)
elapsed = time.perf_counter() - t
iters = calls.get("ackermann_loop_iters", 0)
print(f"ackermann_loop(3,6) = {r5}  ({elapsed:.3f}s)  (1 call, {iters:,} stack iters)")

print()
total = time.perf_counter() - t_total
print(f"=== Done ({total:.3f}s total) ===")
