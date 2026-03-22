# String Stress Baseline — pre-optimización

**Fecha**: 2026-03-15
**Commit**: post Sprint 5D+
**Build**: release (`cargo build --release`)
**Runs**: 3-run avg

## Totales

| Runner | avg | min | max |
|--------|-----|-----|-----|
| WT     | 112ms | 107ms | 119ms |
| VM     |  68ms |  62ms |  75ms |
| Python |  39ms |  37ms |  41ms |

VM = **1.74x más lento que Python**

## Desglose por escenario (tiempo interno del script, run representativo)

| Escenario       | Qué mide                          | WT    | VM    | Python |
|-----------------|-----------------------------------|-------|-------|--------|
| S1_heavy_concat | 8 000 appends char a char         | 3ms   | 4ms   | 3ms    |
| S2_tokenize     | 2 000 splits CSV (13 tokens)      | 6ms   | 3ms   | 2ms    |
| S3_sliding_slices | 4 000 ventanas de 10 chars      | 6ms   | 3ms   | 3ms    |
| S4_char_freq    | 400 iters × 43-char string        | 22ms  | 9ms   | 4ms    |
| S5_multi_search | 3 000 búsquedas substring (5 pats)| 4ms   | 2ms   | 1ms    |
| S6_template_build | 4 000 strings multi-parte       | 8ms   | 5ms   | 3ms    |
| S7_word_analysis | 1 000 splits + iterar palabras   | 10ms  | 3ms   | 3ms    |
| S8_chunk_extract | 5 000 slices de 5 chars          | 8ms   | 4ms   | 2ms    |
| S9_join_sim     | 400 joins manuales (10 palabras)  | 7ms   | 3ms   | 1ms    |
| S10_num_format  | 6 000 registros int→string        | 13ms  | 6ms   | 5ms    |

## Bottlenecks identificados (VM)

| # | Operación          | Problema                                      | Impacto |
|---|--------------------|-----------------------------------------------|---------|
| 1 | ArrayGet en String | Vec<char> completo por cada acceso → O(N²)    | S4: 9ms vs 4ms |
| 2 | StrSlice           | Vec<char> + 2 iteraciones completas por slice | S3/S8 |
| 3 | ConcatStr          | Sin with_capacity, hasta 3 allocs por op      | S1/S6/S9/S10 |
| 4 | StrSplit           | Clona string completo innecesariamente        | S2/S7 |
| 5 | Print              | Alloca String temp aunque sea Int/String      | general |
| 6 | StrContains        | .clone() innecesario del elemento             | S5 |

## Objetivo post-optimización

| Runner | Target | Mejora esperada |
|--------|--------|-----------------|
| VM     | ~35ms  | ~2x vs baseline |
| Python | 39ms   | referencia      |
