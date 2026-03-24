# Baseline v0.0.2 — Post-Collection API Redesign

Fecha: 2026-03-23
Binario: target/release/zymbol (v0.0.2 — new collection operators)
Corrección: 159/159 vm_compare PASS

## Resultados (3 runs cada benchmark)

| Benchmark           | Zymbol min | Zymbol avg | Zymbol max |
|---------------------|-----------|-----------|-----------|
| stress              | 198ms      | 202ms      | 210ms      |
| bench_match         | 158ms      | 166ms      | 172ms      |
| bench_recursion     | 1465ms     | 1494ms     | 1524ms     |
| bench_collections   | 59ms       | 63ms       | 67ms       |
| bench_strings       | 43ms       | 44ms       | 45ms       |
| bench_strings_stress| 113ms      | 118ms      | 122ms      |
| bench_strings_modify| 59ms       | 63ms       | 69ms       |

## Baseline anterior (Sprint 3, 2026-03-22, 5 runs)

| Benchmark           | Zymbol min | Zymbol avg | Zymbol max | Python min | Python avg | Ratio avg |
|---------------------|-----------|-----------|-----------|-----------|-----------|-----------|
| stress              | 197ms      | 207ms      | 223ms      | 71ms       | 79ms       | **2.6x**  |
| bench_match         | 157ms      | 163ms      | 174ms      | 54ms       | 60ms       | **2.7x**  |
| bench_recursion     | 1476ms     | 1491ms     | 1510ms     | 214ms      | 218ms      | **6.8x**  |
| bench_collections   | 62ms       | 69ms       | 78ms       | 41ms       | 47ms       | **1.5x**  |
| bench_strings       | 42ms       | 50ms       | 59ms       | 25ms       | 27ms       | **1.9x**  |
| bench_strings_stress| 114ms      | 116ms      | 117ms      | 45ms       | 54ms       | **2.1x**  |
| bench_strings_modify| 63ms       | 69ms       | 80ms       | 34ms       | 41ms       | **1.7x**  |

## Objetivos Sprint 1 (B8 + B5 + B1 + B2)

| Benchmark       | Actual avg | Objetivo   | Mejora esperada |
|-----------------|-----------|-----------|----------------|
| stress          | 207ms      | ~150ms     | ~25–30%         |
| bench_recursion | 1491ms     | ~300ms     | ~5x             |
| bench_match     | 163ms      | ~130ms     | ~20%            |
