#!/usr/bin/env python3
"""Python — Extended String Stress Test
Mirrors bench_strings_stress.zy exactly (same iteration counts, same logic).

Zymbol @ i:0..N → range(N+1) in Python (0..N inclusive)

Run: python3 tests/scripts/bench_strings_stress.py
"""

import time

print("=== Python String Stress (Extended) ===")

t0 = time.perf_counter()

# ── S1: Heavy concat accumulation ──────────────────────────────────────────────
# 8 000 single-char appends  (0..7999 inclusive = 8000 iters)
t = time.perf_counter()
s = ""
for i in range(8000):
    s = s + "x"
s1_len = len(s)
elapsed = time.perf_counter() - t
print(f"S1_heavy_concat:    len={s1_len}  ({elapsed:.3f}s)")

# ── S2: Repeated tokenization ───────────────────────────────────────────────────
# 2 000 splits of a 13-token CSV  (0..1999 inclusive = 2000 iters)
t = time.perf_counter()
csv = "alpha,beta,gamma,delta,epsilon,zeta,eta,theta,iota,kappa,lambda,mu,nu"
total_tokens = 0
for i in range(2000):
    parts = csv.split(",")
    total_tokens += len(parts)
elapsed = time.perf_counter() - t
print(f"S2_tokenize:        total={total_tokens}  ({elapsed:.3f}s)")

# ── S3: Sliding-window slices ────────────────────────────────────────────────────
# 4 000 overlapping 10-char windows  (0..3999 inclusive = 4000 iters)
t = time.perf_counter()
text = "the quick brown fox jumps over the lazy dog near the river bank"
slice_total = 0
for i in range(4000):
    start = i % 53
    sl = text[start:start + 10]
    slice_total += len(sl)
elapsed = time.perf_counter() - t
print(f"S3_sliding_slices:  chars={slice_total}  ({elapsed:.3f}s)")

# ── S4: Char frequency count ─────────────────────────────────────────────────────
# 400 full iterations over a 43-char sentence  (0..399 inclusive = 400 iters)
t = time.perf_counter()
corpus = "the quick brown fox jumps over the lazy dog"
vowels = 0
consonants = 0
for _ in range(400):
    for ch in corpus:
        if ch in "aeiou":
            vowels += 1
        elif ch != " ":
            consonants += 1
elapsed = time.perf_counter() - t
print(f"S4_char_freq:       vowels={vowels} consonants={consonants}  ({elapsed:.3f}s)")

# ── S5: Multi-pattern substring search ───────────────────────────────────────────
# 3 000 contains checks cycling through 5 patterns  (0..2999 inclusive = 3000 iters)
t = time.perf_counter()
haystack = "artificial intelligence natural language processing machine learning neural network deep"
patterns = ["intel", "lang", "learn", "neural", "deep"]
hits = 0
for i in range(3000):
    p = patterns[i % 5]
    if p in haystack:
        hits += 1
elapsed = time.perf_counter() - t
print(f"S5_multi_search:    hits={hits}  ({elapsed:.3f}s)")

# ── S6: Template / markup building ───────────────────────────────────────────────
# 4 000 multi-part string assemblies  (0..3999 inclusive = 4000 iters)
t = time.perf_counter()
template_total = 0
for i in range(4000):
    sq = i * i
    line = "row:" + str(i) + " sq:" + str(sq) + " tag:item-" + str(i) + "-end"
    template_total += len(line)
elapsed = time.perf_counter() - t
print(f"S6_template_build:  chars={template_total}  ({elapsed:.3f}s)")

# ── S7: Word-level analysis ───────────────────────────────────────────────────────
# 1 000 sentence splits → iterate words  (0..999 inclusive = 1000 iters)
t = time.perf_counter()
sentence = "the quick brown fox jumps over the lazy dog"
word_chars = 0
long_words = 0
for _ in range(1000):
    words = sentence.split(" ")
    for w in words:
        wlen = len(w)
        word_chars += wlen
        if wlen > 4:
            long_words += 1
elapsed = time.perf_counter() - t
print(f"S7_word_analysis:   total={word_chars} long={long_words}  ({elapsed:.3f}s)")

# ── S8: Fixed-size chunk extraction ─────────────────────────────────────────────
# 5 000 5-char windows from a 62-char alphabet  (0..4999 inclusive = 5000 iters)
t = time.perf_counter()
data = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789"
chunk_total = 0
for i in range(5000):
    pos = (i * 7) % 57
    chunk = data[pos:pos + 5]
    chunk_total += len(chunk)
elapsed = time.perf_counter() - t
print(f"S8_chunk_extract:   total={chunk_total}  ({elapsed:.3f}s)")

# ── S9: Simulated join ───────────────────────────────────────────────────────────
# 400 manual joins of a 10-element word list  (0..399 inclusive = 400 iters)
t = time.perf_counter()
wlist = ["one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten"]
join_total = 0
for _ in range(400):
    joined = ""
    for j in range(10):
        if j > 0:
            joined = joined + ","
        joined = joined + wlist[j]
    join_total += len(joined)
elapsed = time.perf_counter() - t
print(f"S9_join_sim:        total={join_total}  ({elapsed:.3f}s)")

# ── S10: Numeric-string formatting ──────────────────────────────────────────────
# 6 000 formatted records  (0..5999 inclusive = 6000 iters)
t = time.perf_counter()
fmt_total = 0
for i in range(6000):
    diff = i - 3000
    score = (i * 37) % 1000
    record = "id=" + str(i) + " diff=" + str(diff) + " score=" + str(score)
    fmt_total += len(record)
elapsed = time.perf_counter() - t
print(f"S10_num_format:     chars={fmt_total}  ({elapsed:.3f}s)")

total_time = time.perf_counter() - t0
print(f"=== Done ({total_time:.3f}s total) ===")
