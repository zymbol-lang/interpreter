#!/usr/bin/env python3
"""Python Benchmark: Strings — equivalent to bench_strings.zy
Workloads mirror Zymbol string operators: +, /, $#, $[..], $?, char iteration

Run: python3 tests/scripts/bench_strings.py
"""

import time

print("=== Python Benchmark: Strings ===")

t0 = time.perf_counter()

# --- 1. String concat build (2k single-char appends) ---
# Mirrors: s = s + "x"  (0..2000 inclusive = 2001 iters in Zymbol)
t = time.perf_counter()
s = ""
for i in range(2001):   # match Zymbol 0..2000 inclusive
    s = s + "x"
elapsed = time.perf_counter() - t
print(f"string_concat: len = {len(s)}  ({elapsed:.3f}s)")

# --- 2. Split operations (1k splits on 10-element CSV) ---
# Mirrors: csv / ','  (0..1000 inclusive = 1001 iters in Zymbol)
t = time.perf_counter()
csv = "one,two,three,four,five,six,seven,eight,nine,ten"
total_parts = 0
for i in range(1001):   # match Zymbol 0..1000 inclusive
    parts = csv.split(",")
    total_parts += len(parts)
elapsed = time.perf_counter() - t
print(f"split_ops: total_parts = {total_parts}  ({elapsed:.3f}s)")

# --- 3. Slice ops (2k substring extractions) ---
# Mirrors: long_str$[0..13]  (exclusive end = 13 chars, 0..2000 = 2001 iters)
t = time.perf_counter()
long_str = "abcdefghijklmnopqrstuvwxyz"
slice_chars = 0
for i in range(2001):   # match Zymbol 0..2000 inclusive
    sl = long_str[0:13]
    slice_chars += len(sl)
elapsed = time.perf_counter() - t
print(f"slice_ops: slice_chars = {slice_chars}  ({elapsed:.3f}s)")

# --- 4. Length queries (5k $# on variable-length strings) ---
# Mirrors: w$#  (0..5000 inclusive = 5001 iters in Zymbol)
t = time.perf_counter()
words = ["hello", "world", "zymbol", "lang", "symbolic", "unicode"]
total_len = 0
for i in range(5001):   # match Zymbol 0..5000 inclusive
    w = words[i % 6]
    total_len += len(w)
elapsed = time.perf_counter() - t
print(f"length_ops: total_len = {total_len}  ({elapsed:.3f}s)")

# --- 5. Char iteration (count 'a' occurrences, ~500 iterations) ---
# Mirrors: @ iter:0..500 { @ ch:sentence { ... } }  (0..500 = 501 iters)
t = time.perf_counter()
sentence = "a man a plan a canal panama"
total_a = 0
for _ in range(501):    # match Zymbol 0..500 inclusive
    for ch in sentence:
        if ch == 'a':
            total_a += 1
elapsed = time.perf_counter() - t
print(f"char_iter: total_a = {total_a}  ({elapsed:.3f}s)")

# --- 6. String contains scan (2k lookups) ---
# Mirrors: haystack$? needle  (0..2000 inclusive = 2001 iters in Zymbol)
t = time.perf_counter()
haystack = "the quick brown fox jumps over the lazy dog"
needles = ['a', 'e', 'i', 'o', 'u', 'z', 'x', 'q']
contain_hits = 0
for i in range(2001):   # match Zymbol 0..2000 inclusive
    needle = needles[i % 8]
    if needle in haystack:
        contain_hits += 1
elapsed = time.perf_counter() - t
print(f"string_contains: {contain_hits} hits  ({elapsed:.3f}s)")

# --- 7. Multi-token string build (2k iterations) ---
# Mirrors: token = prefix + "-" + suffix + "-" + i  (0..1999 inclusive = 2000 iters)
t = time.perf_counter()
prefix = "Zymbol"
suffix = "lang"
build_total = 0
for i in range(2000):   # match Zymbol 0..1999 inclusive
    token = prefix + "-" + suffix + "-" + str(i)
    build_total += len(token)
elapsed = time.perf_counter() - t
print(f"multi_token_build: total_chars = {build_total}  ({elapsed:.3f}s)")

total_time = time.perf_counter() - t0
print(f"=== Done ({total_time:.3f}s total) ===")
