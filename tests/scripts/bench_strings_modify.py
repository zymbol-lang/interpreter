#!/usr/bin/env python3
"""Python — String Modification Stress Test
Mirrors bench_strings_modify.zy exactly.
Zymbol @ i:0..N → range(N+1) in Python (0..N inclusive)

Run: python3 tests/scripts/bench_strings_modify.py
"""
import time

print("=== Python String Modify Stress ===")
t0 = time.perf_counter()

# ── M1: Replace all — char pattern ──────────────────────────────────────────
# 0..2999 inclusive = 3000 iters
t = time.perf_counter()
base_str = "the quick brown fox jumps over the lazy dog near the river bank"
total_len = 0
for i in range(3000):
    result = base_str.replace(" ", "_")
    total_len += len(result)
elapsed = time.perf_counter() - t
print(f"M1_replace_char:    len={total_len}  ({elapsed:.3f}s)")

# ── M2: Replace all — string pattern ────────────────────────────────────────
# 0..1999 inclusive = 2000 iters
t = time.perf_counter()
sentence = "the cat sat on the mat near the hat by the flat"
total_len = 0
for i in range(2000):
    result = sentence.replace("the", "a")
    total_len += len(result)
elapsed = time.perf_counter() - t
print(f"M2_replace_str:     len={total_len}  ({elapsed:.3f}s)")

# ── M3: Replace N — limit replacements ──────────────────────────────────────
# 0..1999 inclusive = 2000 iters; Python replace(old, new, count)
t = time.perf_counter()
text = "the quick brown fox jumps over the dog on the floor of the room"
total_len = 0
for i in range(2000):
    result = text.replace("o", "0", 2)
    total_len += len(result)
elapsed = time.perf_counter() - t
print(f"M3_replace_n:       len={total_len}  ({elapsed:.3f}s)")

# ── M4: Find positions — char pattern ────────────────────────────────────────
# 0..2999 inclusive = 3000 iters
t = time.perf_counter()
haystack = "a man a plan a canal panama"
total_hits = 0
for i in range(3000):
    positions = [idx for idx, ch in enumerate(haystack) if ch == 'a']
    total_hits += len(positions)
elapsed = time.perf_counter() - t
print(f"M4_findpos_char:    hits={total_hits}  ({elapsed:.3f}s)")

# ── M5: Find positions — string pattern ─────────────────────────────────────
# 0..1999 inclusive = 2000 iters
t = time.perf_counter()
corpus = "a man a plan a canal panama banana"
total_hits = 0
for i in range(2000):
    pos = 0
    count = 0
    while True:
        idx = corpus.find("an", pos)
        if idx == -1:
            break
        count += 1
        pos = idx + 1
    total_hits += count
elapsed = time.perf_counter() - t
print(f"M5_findpos_str:     hits={total_hits}  ({elapsed:.3f}s)")

# ── M6: Insert — build structured strings ────────────────────────────────────
# 0..1999 inclusive = 2000 iters
t = time.perf_counter()
base = "content goes here for this item"
total_len = 0
for i in range(2000):
    tagged = "[ITEM] " + base
    total_len += len(tagged)
elapsed = time.perf_counter() - t
print(f"M6_insert_front:    len={total_len}  ({elapsed:.3f}s)")

# ── M7: Insert — middle insertion ────────────────────────────────────────────
# 0..1999 inclusive = 2000 iters
t = time.perf_counter()
word = "helloworld"
total_len = 0
for i in range(2000):
    spaced = word[:5] + " " + word[5:]
    total_len += len(spaced)
elapsed = time.perf_counter() - t
print(f"M7_insert_mid:      len={total_len}  ({elapsed:.3f}s)")

# ── M8: Remove — strip prefix ────────────────────────────────────────────────
# 0..2999 inclusive = 3000 iters
t = time.perf_counter()
log_line = ">>> ERROR: connection timeout at host 192.168.1.1"
total_len = 0
for i in range(3000):
    stripped = log_line[4:]
    total_len += len(stripped)
elapsed = time.perf_counter() - t
print(f"M8_remove_prefix:   len={total_len}  ({elapsed:.3f}s)")

# ── M9: Remove — strip from middle ───────────────────────────────────────────
# 0..1999 inclusive = 2000 iters
t = time.perf_counter()
data = "field_name:HIDDEN:field_value"
total_len = 0
for i in range(2000):
    cleaned = data[:11] + data[17:]
    total_len += len(cleaned)
elapsed = time.perf_counter() - t
print(f"M9_remove_mid:      len={total_len}  ({elapsed:.3f}s)")

# ── M10: Pipeline — replace + find + measure ─────────────────────────────────
# 0..999 inclusive = 1000 iters
t = time.perf_counter()
raw = "  Hello,  World!  How   are   you  today?  "
total_positions = 0
for i in range(1000):
    step1 = raw.replace("  ", " ")
    step2 = step1.replace(",", " ")
    step3 = step2.replace("!", " ")
    positions = [idx for idx, ch in enumerate(step3) if ch == ' ']
    total_positions += len(positions)
elapsed = time.perf_counter() - t
print(f"M10_pipeline:       pos={total_positions}  ({elapsed:.3f}s)")

total_time = time.perf_counter() - t0
print(f"=== Done ({total_time:.3f}s total) ===")
