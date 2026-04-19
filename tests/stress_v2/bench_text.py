#!/usr/bin/env python3
"""STRESS V2 — Text Processing
Optimal Python: f-strings, list comprehensions, sorted(key=), generators.
Comparison: tests/stress_v2/bench_text.zy

Run: python3 tests/stress_v2/bench_text.py
"""
import time

print("=== Python Text v2 ===")
t0 = time.perf_counter()

# T1: template building — f-strings, 5 000 iters
t = time.perf_counter()
total_len = 0
names = ["Alice", "Bob", "Carol", "Dave", "Eve"]
for i in range(1, 5001):
    name = names[(i - 1) % 5]
    score = i % 100
    line = f"User {name} scored {score} points in round {i}"
    total_len += len(line)
elapsed = time.perf_counter() - t
print(f"T1_template_build:  total={total_len}  ({elapsed:.3f}s)")

# T2: join simulation — str.join() (optimal: single alloc), 1 000 iters
t = time.perf_counter()
words = ["apple", "banana", "cherry", "date", "elderberry", "fig", "grape"]
total_len = 0
for _ in range(1000):
    result = ",".join(words)
    total_len += len(result)
elapsed = time.perf_counter() - t
print(f"T2_join_sim:        total={total_len}  ({elapsed:.3f}s)")

# T3: word count pipeline — split + filter, 1 000 iters
t = time.perf_counter()
corpus = "the quick brown fox jumps over the lazy dog and the cat sat on the mat by the tree"
short_total = long_total = 0
for _ in range(1000):
    words_list = corpus.split(" ")
    short_total += sum(1 for w in words_list if len(w) <= 3)
    long_total  += sum(1 for w in words_list if len(w) > 3)
elapsed = time.perf_counter() - t
print(f"T3_word_buckets:    short={short_total} long={long_total}  ({elapsed:.3f}s)")

# T4: sort strings by length (key= function), 500 iters
t = time.perf_counter()
items = ["banana", "apple", "fig", "elderberry", "date", "cherry", "grape", "avocado", "kiwi", "mango"]
by_len = None
for _ in range(500):
    by_len = sorted(items, key=len)
elapsed = time.perf_counter() - t
print(f"T4_sort_by_len:     first={by_len[0]} last={by_len[-1]}  ({elapsed:.3f}s)")

# T5: tokenize + deduplicate count — split, sort, count unique (200 iters)
t = time.perf_counter()
text = "one two three one two four five three six one two three four"
sorted_tokens = None
for _ in range(200):
    sorted_tokens = sorted(text.split(" "))
uniq_count = len(set(text.split(" ")))
elapsed = time.perf_counter() - t
print(f"T5_sort_dedup:      unique={uniq_count}  ({elapsed:.3f}s)")

# T6: string transform pipeline — replace + split + count, 500 iters
t = time.perf_counter()
total = 0
log = "ERROR: timeout at 192.168.1.1 port 8080 after 30 seconds"
for _ in range(500):
    cleaned = log.replace("ERROR: ", "").replace(".", " ")
    word_count = len(cleaned.split(" "))
    total += word_count
elapsed = time.perf_counter() - t
print(f"T6_transform_pipe:  total={total}  ({elapsed:.3f}s)")

total_time = time.perf_counter() - t0
print(f"=== Done ({total_time:.3f}s total) ===")
