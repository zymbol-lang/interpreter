# Zymbol v0.0.4 — Issues del análisis de consultoría

Fuente: `ES_analisis_zymbol_v0_0_4.md`
Referencia de secciones: §N = sección del MANUAL.md, §A.N = sección del análisis.

---

## Prioridad 0 — Prerequisitos de arquitectura

Deben resolverse **antes** de los demás porque otros issues dependen de ellos.

- [x] **P0-A** — **Funciones como ciudadanos de primera clase**: `fn = myFunc`, `arr$> myFunc`, funciones retornadas desde funciones. Decisión: Opción A (captura scope en punto de uso). (§A.3.1, roadmap item #3)
  - *Prerequisito de*: P0-B, P2-B, y la utilidad completa de todos los HOF (`$>`, `$|`, `$<`, `$^`, `|>`)
  - *Resuelto*: L5 (`fn = myFunc` ✅), L6 (`arr$> fn` ✅ para `$>`, `$|`, `$<`)
  - *Archivos modificados*: `zymbol-interpreter/src/lib.rs`, `functions_lambda.rs`, `expr_eval.rs`, `zymbol-parser/src/functions.rs`, `collection_ops.rs`

- [x] **P0-B** — **Tipo función en el modelo de tipos**: símbolo `##->`, display `<function/N>` vs `<lambda/N>`, arity via `##type`. Function y Error añadidos a tabla §2 del MANUAL. (§A.4.4, §A.3.1)
  - *Prerequisito de*: P3-E (modelo de tipos completo)

---

## Prioridad 1 — Fundamentos (bloqueadores de v0.1)

- [x] **P1-A** — Asimetría función/lambda resuelta por P0-A. MANUAL §9 actualizado: scope aislado en llamada directa, captura en uso como valor, HOF sin wrapper, tabla "cuándo usar cada uno" revisada. (§A.3.1, §9)
- [x] **P1-B** — Paridad total VM↔Tree-walker alcanzada: 390/390 tests idénticos. Fixes: eliminado prefijo `"runtime error: "` doble en TW y VM, named tuple field errors catchables en VM via `raise!`, módulos privados emiten `RaiseError` en compilador, `_err` en catch formateado como `##kind(msg)`. (§A.3.7, §23)
- [x] **P1-C** — Sección §1b "Lexical Structure" añadida al MANUAL: identificadores (Unicode, reglas first/rest char), comentarios (line + nested block), whitespace (no significativo, excepción `@label`), escapes de strings (`\n \t \r \" \\ \{ \}`), interpolación `{var}`, literales numéricos/char/bool, tokens de newline `¶`/`\\`, y nota de ausencia de keywords reservadas. (§A.4.2)
- [x] **P1-D** — EBNF incluida como Apéndice A normativo en el MANUAL: referencia a `zymbol-lang.ebnf` en §23, grammar completa embebida en bloque de código. EBNF actualizada: test coverage 390/390, comentario pipe_call corregido (funciones first-class en v0.0.4). (§A.4.1)
- [x] **P1-E** — Semántica de destructuring sobre variables existentes documentada en §11b: **sobrescribe** (no error, no shadow). Casos cubiertos: variable mutable existente, posición `_` (intacta), scope de función (aislado), VM (idéntico). Bug L14 añadido a §20: destructuring ignora inmutabilidad de constantes (`:=`). (§A.3.8, §11b)

---

## Prioridad 2 — Ergonomía

- [x] **P2-A** — Encadenamiento de `$+` habilitado: `arr$+ 4$+ 5$+ 6` funciona. Fix: `parse_postfix_structural()` añadida al parser — argumento de `$+` parsea `[]`/`.`/`()` pero detiene antes del siguiente `$X`, permitiendo el encadenamiento izquierda-derecha. MANUAL L10 marcada como resuelta. 391/391 parity tests. (§A.3.5)
- [x] **P2-B** — Permitir pipe sin `_` cuando el valor va en primera posición: `x |> f` ≡ `f(x)`. Fix: rama implícita en `parse_pipe()` y `parse_pipe_no_comparison()` — si no hay `(`, crea `PipeExpr` con `arguments: [Placeholder]`. MANUAL §15 reescrito. EBNF `pipe_call` actualizada. 393/393 parity. (§A.3.6)
- [x] **P2-C** — ~~Reconsiderar `@label` vs `@ label`~~ → **Implementado**: `@:label` / `@:label!` / `@:label>`. Nuevos tokens `AtColonLabel`, `AtColonLabelBreak`, `AtColonLabelContinue` en lexer. Parser actualizado; AST/intérprete/VM sin cambios (usan `Option<String>`). Tests migrados: `10_labeled.zy`, `12_labeled_multi.zy`, `test_me.zy`. MANUAL §8 reescrito con tabla. EBNF actualizada. 393/393 parity. (§A.3.2, §8)
- [x] **P2-D** — Documentar orden de evaluación de argumentos y semántica de captura de lambdas en closures de bucle. Nueva sección §10b "Evaluation Order and Capture Semantics" en MANUAL: izquierda-a-derecha, captura por valor en creación, loop closures con snapshots por iteración, writes a capturas locales, named functions vs lambdas. (§A.4.3)
  - *Bug descubierto y corregido al validar ejemplos*: **L15** — `arr[i](args)` no invocaba la lambda en contexto `>>`. Fix: `parse_output_item_postfix` usaba whitelist `is_callable` que excluía `IndexExpr`; cambiado a blacklist que excluye solo `Literal`. `ops[1](5)` ahora funciona en todos los contextos. MANUAL §10 corregido, L15 eliminado de §20.

---

## Prioridad 3 — Claridad y posicionamiento

- [x] **P3-A** — Articular identidad del lenguaje. **Decisión**: minimalismo simbólico propio — origen: 20 años de SAP ABAP (reacción a la verbosidad), no influencia de APL/J/K (convergencia independiente). Principio clave: símbolos de teclado que *sugieren* significado vs glifos abstractos de APL. Símbolo compartido cuando el espíritu es similar (`_` = no-binding, `#` = meta-level). La complejidad entró por profundidad, no keywords. Nueva §0 "Design Philosophy" en MANUAL con tabla comparativa Zymbol vs APL. (§A.5.1, §A.5.4)
- [ ] **P3-B** — Separar manual de usuario / reference manual / notas de implementación; mover discusión tree-walker/VM a apéndice. **Aplazado**: las menciones TW/VM en el cuerpo son pocas y ya están contenidas; §23 y Apéndice A concentran los detalles de runtime. Refactorización estructural con bajo beneficio en esta etapa — retomar cuando el MANUAL supere su tamaño actual o cuando se genere documentación externa (web, wiki). (§A.4.6)
- [ ] **P3-C** — Decidir estrategia con arrays homogéneos (L11): si se mantiene, documentar el porqué; si se relaja, incluir en roadmap. (§A.3.4)
- [ ] **P3-D** — Separar limitaciones "por diseño" de limitaciones "por implementación pendiente" en §20 Known Limitations. (§A.4.8)
- [ ] **P3-E** — Definir modelo de tipos completo: tipos de función/lambda, tipo de error, tipo Module, tipo Range. (§A.4.4)
- [ ] **P3-F** — Resolver inconsistencia en sintaxis de módulos: braces obligatorias vs opcionales (§17 vs §22). (§A.4.5)
- [ ] **P3-G** — Definir taxonomía de errores: runtime error, semantic error, parser error. (§A.4.7)
- [ ] **P3-H** — Documentar arquitectura del manejo de errores: cuándo usar `!?/:!/:>` (excepciones) vs `$!/$!!` (valores). (§A.5.5)

---

## Prioridad 4 — Investigación / largo plazo

- [ ] **P4-A** — Reconsiderar `>` como separador de navegación en `arr[a>b]` vs operador de comparación: evaluar alternativas. (§A.3.3)
- [ ] **P4-B** — Agregar nota/sección "Why 1-based indexing" al manual. (§A.5.3)
- [ ] **P4-C** — Explorar relación/diferenciación con APL/J/K como ejercicio de posicionamiento. (§A.5.2)

---

## Registro de avance

| ID     | Estado     | Fecha      | Notas |
|--------|------------|------------|-------|
| P0-A   | ✅ completo | 2026-04-21 | Opción A implementada: 8/8 tests, 0 regresiones |
| P0-B   | ✅ completo | 2026-04-21 | `##->` formalizado, display `<function/N>` / `<lambda/N>`, MANUAL §2 + §18 + §14 actualizados |
| P1-A   | ✅ completo | 2026-04-21 | Cerrado por P0-A; MANUAL §9 reescrito |
| P1-B   | ✅ completo | 2026-04-21 | 390/390 TW↔VM idénticos; 5 fixes en compiler/VM/TW |
| P1-C   | ✅ completo | 2026-04-21 | §1b "Lexical Structure" añadida al MANUAL |
| P1-D   | ✅ completo | 2026-04-21 | Apéndice A añadido al MANUAL con EBNF completa |
| P1-E   | ✅ completo | 2026-04-21 | Semántica = sobrescribe; bug L14 documentado en §20 |
| P2-A   | ✅ completo | 2026-04-21 | `$+` encadenamiento; `parse_postfix_structural()` en parser |
| P2-B   | ✅ completo | 2026-04-21 | `x \|> f` ≡ `f(x)` implícito; `parse_pipe()` + `parse_pipe_no_comparison()` actualizados; 393/393 parity |
| P2-C   | ✅ completo | 2026-04-21 | `@:label`/`@:label!`/`@:label>` implementados; 3 tests migrados; MANUAL §8 + EBNF actualizados; 393/393 parity |
| P2-D   | ✅ completo | 2026-04-21 | §10b añadida; bug L15 (`arr[i](args)`) descubierto y corregido en `parse_output_item_postfix`; 393/393 parity |
| P3-A   | ✅ completo | 2026-04-21 | §0 "Design Philosophy" añadida: minimalismo simbólico propio, coherencia `_`/`#`, complejidad por profundidad no keywords |
| P3-B   | ⏸ aplazado | 2026-04-21 | Retomar cuando el MANUAL crezca o se genere documentación externa |
| P3-C   | pendiente  |            |       |
| P3-D   | pendiente  |            |       |
| P3-E   | pendiente  |            |       |
| P3-F   | pendiente  |            |       |
| P3-G   | pendiente  |            |       |
| P3-H   | pendiente  |            |       |
| P4-A   | pendiente  |            |       |
| P4-B   | pendiente  |            |       |
| P4-C   | pendiente  |            |       |
