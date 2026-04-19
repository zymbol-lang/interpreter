#!/usr/bin/env python3
"""STRESS V2 — Data Pipeline (Split + HOF)
Optimal Python: generator expressions over split(), no intermediate lists.
Comparison: tests/stress_v2/bench_pipeline.zy

Run: python3 tests/stress_v2/bench_pipeline.py
"""
import time

print("=== Python Pipeline v2 ===")
t0 = time.perf_counter()

csv = "alpha,beta,gamma,delta,epsilon,zeta,eta,theta,iota,kappa,lambda,mu"

# P1: split + count (5 000 iters)
t = time.perf_counter()
total = 0
for _ in range(5000):
    total += len(csv.split(","))
elapsed = time.perf_counter() - t
print(f"P1_split_count:     total={total}  ({elapsed:.3f}s)")

# P2: split + map lengths (2 000 iters)
t = time.perf_counter()
total = 0
for _ in range(2000):
    lens = [len(w) for w in csv.split(",")]
    total += len(lens)
elapsed = time.perf_counter() - t
print(f"P2_split_map:       total={total}  ({elapsed:.3f}s)")

# P3: split + filter long words (2 000 iters)
t = time.perf_counter()
total = 0
for _ in range(2000):
    long_words = [w for w in csv.split(",") if len(w) > 4]
    total += len(long_words)
elapsed = time.perf_counter() - t
print(f"P3_split_filter:    total={total}  ({elapsed:.3f}s)")

# P4: split + reduce total length — generator sum, no intermediate list (2 000 iters)
t = time.perf_counter()
total = 0
for _ in range(2000):
    char_total = sum(len(w) for w in csv.split(","))
    total += char_total
elapsed = time.perf_counter() - t
print(f"P4_split_reduce:    total={total}  ({elapsed:.3f}s)")

# P5: multi-field CSV — split ':' then each record by ','
t = time.perf_counter()
records = "alice,30,95:bob,25,88:carol,35,92:dave,28,79:eve,32,85:frank,27,91:grace,33,87"
total_score = 0
for _ in range(1000):
    for row in records.split(":"):
        fields = row.split(",")
        total_score += int(fields[2])
elapsed = time.perf_counter() - t
print(f"P5_csv_rows:        total_score={total_score}  ({elapsed:.3f}s)")

# P6: word frequency — split sentence, count words > 3 chars (2 000 iters)
t = time.perf_counter()
sentence = "the quick brown fox jumps over the lazy dog and the cat sat on the mat"
total = 0
for _ in range(2000):
    long_words = [w for w in sentence.split(" ") if len(w) > 3]
    total += len(long_words)
elapsed = time.perf_counter() - t
print(f"P6_word_filter:     total={total}  ({elapsed:.3f}s)")

# P7: build + split pipeline — generate CSV then aggregate
t = time.perf_counter()
total = 0
for i in range(1, 501):
    row = f"{i},{i*2},{i*3}"
    total += sum(len(p) for p in row.split(","))
elapsed = time.perf_counter() - t
print(f"P7_build_split_agg: total={total}  ({elapsed:.3f}s)")

total_time = time.perf_counter() - t0
print(f"=== Done ({total_time:.3f}s total) ===")
