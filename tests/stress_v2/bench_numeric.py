#!/usr/bin/env python3
"""STRESS V2 — Numeric Algorithms
Optimal Python: builtins, list comprehensions, generator sums.
Comparison: tests/stress_v2/bench_numeric.zy

Run: python3 tests/stress_v2/bench_numeric.py
"""
import time
import math

print("=== Python Numeric v2 ===")
t0 = time.perf_counter()

# N1: loop fibonacci (iterative) — 10 iters of fib(40)
t = time.perf_counter()
result = 0
for _ in range(10):
    a, b = 0, 1
    for _ in range(39):
        a, b = b, a + b
    result = b
elapsed = time.perf_counter() - t
print(f"N1_fib40_loop:      result={result}  ({elapsed:.3f}s)")

# N2: sum of digits for all numbers 1..50 000
t = time.perf_counter()
grand_total = sum(sum(int(d) for d in str(n)) for n in range(1, 50001))
elapsed = time.perf_counter() - t
print(f"N2_digit_sum:       total={grand_total}  ({elapsed:.3f}s)")

# N3: count primes up to 10 000 (trial division — same algorithm as Zymbol)
t = time.perf_counter()
prime_count = 0
for n in range(2, 10001):
    limit = int(math.isqrt(n))
    is_prime = True
    for d in range(2, limit + 1):
        if n % d == 0:
            is_prime = False
            break
    if is_prime:
        prime_count += 1
elapsed = time.perf_counter() - t
print(f"N3_primes_10k:      count={prime_count}  ({elapsed:.3f}s)")

# N4: modular square accumulation — 100 000 iters
t = time.perf_counter()
total = sum((i * i) % 1000 for i in range(1, 100001))
elapsed = time.perf_counter() - t
print(f"N4_modular_sq:      total={total}  ({elapsed:.3f}s)")

# N5: FizzBuzz count — 1 000 000 iters
t = time.perf_counter()
fizz_count = buzz_count = fizzbuzz_count = 0
for i in range(1, 1000001):
    if i % 15 == 0:
        fizzbuzz_count += 1
    elif i % 3 == 0:
        fizz_count += 1
    elif i % 5 == 0:
        buzz_count += 1
elapsed = time.perf_counter() - t
print(f"N5_fizzbuzz_1M:     fizz={fizz_count} buzz={buzz_count} fb={fizzbuzz_count}  ({elapsed:.3f}s)")

# N6: dot product simulation — two 5 000-element arrays
t = time.perf_counter()
a_arr = list(range(1, 5001))
b_arr = list(range(5000, 0, -1))
dot = sum(a * b for a, b in zip(a_arr, b_arr))
elapsed = time.perf_counter() - t
print(f"N6_dot_product:     result={dot}  ({elapsed:.3f}s)")

total_time = time.perf_counter() - t0
print(f"=== Done ({total_time:.3f}s total) ===")
