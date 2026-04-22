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
- [x] **P3-B** — MANUAL.md dividido en tres archivos: `GUIDE.md` (§0–§19 + §22, 2903 líneas), `REFERENCE.md` (§20/§20b/§21, 418 líneas), `IMPLEMENTATION.md` (§23 + Apéndice A + modelo TW/VM, 561 líneas). `MANUALdeprecate.md` conservado como fuente histórica. Cada archivo tiene header propio, ToC y cross-links. (§A.4.6)
- [x] **P3-C** — Arrays homogéneos: decisión "by design" documentada en §20 L11. Arrays = typed sequences (ordered, mutable, uniform type). Heterogeneous records = named tuples. Distinción explícita con ejemplos verificados. (§A.3.4)
- [x] **P3-D** — §20 reestructurado con categorías "By Design" / "Implementation Gap". Todas las limitaciones etiquetadas: L1/L9/L12/L13/L14 → *(implementation gap)*; L11 → *(by design)*. Header explicativo añadido. (§A.4.8)
- [x] **P3-E** — Modelo de tipos completo documentado en §2. Tabla de value types ampliada: Function (`<function/N>`) y Lambda (`<lambda/N>`) comparten `##->`, se distinguen por display. Error: tipo IS el kind (`##Index`, `##Div`, etc.) — no hay `##error` genérico. Tabla de non-value types: Range (loop-only) y Module (namespace-only). Tabla completa de `#?` con los 3 campos: `(type_symbol, count, display)`. Bug en §20b corregido: eliminado `##error` incorrecto. 393/393 parity. (§A.4.4)
- [x] **P3-F** — Inconsistencia resuelta: era un error de documentación en §22. El parser **exige** `# name { ... }` (línea 57-62 de `zymbol-parser/src/modules.rs`). §22 mostraba `# calc` sin llaves con `#>` suelto — corregido a la forma canónica `# calc { #> { ... } ... }`. §17 ya era correcto. (§A.4.5)
- [x] **P3-G** — §20b "Error Taxonomy" añadida al MANUAL: parser errors (fatal, pre-execution), semantic errors (fatal, pre-execution), runtime errors (catchable via `!?/:!/:>`). Tabla de operaciones fail-safe (`#|..|`, `$?`, `#?`). (§A.4.7)
- [x] **P3-H** — §16 "Exception Flow vs Value Flow" añadida: excepciones (`!?/:!/:>`) para captura en fronteras/cleanup; valores (`$!/$!!`) para propagar errores como valores de retorno. Distinción clave: `$!!` es early-return, no throw — no capturable con `!?`. Tabla de decisión incluida. Todos los ejemplos verificados. (§A.5.5)

---

## Prioridad 4 — Investigación / largo plazo

- [x] **P4-A** — **Decisión: mantener `arr[a>b]`**. El contexto resuelve la ambigüedad: dentro de `[]` sin espacio (`a>b`) es navegación; con espacios (`a > b`) es comparación. Alternativas evaluadas (`:`, `>>`, `,`) rompen otras partes de la gramática o introducen más ruido visual. Anotado en §11c de GUIDE.md como comportamiento intencional. (§A.3.3)
- [x] **P4-B** — §11 "Why 1-based Indexing" añadida en GUIDE.md: alineación matemática, legibilidad humana, simetría exacta positivo/negativo, patrones de loop naturales, índice 0 siempre error. Ejemplos verificados. (§A.5.3)
- [x] **P4-C** — **Decisión: no documentar**. APL/J/K operan sobre arrays sin variables, sin flujo de control general. Zymbol tiene variables, flujo lógico completo, y módulos. La única conexión es convergencia de gestos simbólicos — independiente, no derivada. Mencionarlo en la documentación introduciría una comparación que no aporta y podría confundir el posicionamiento del lenguaje. (§A.5.2)

---

## Prioridad 5 — Segunda pasada (revisión_segunda_pasada_v0_0_4.md)

Fuente: `revision_segunda_pasada_v0_0_4.md` §1, §2, §3.

### P5 — Sincronización documental (§1 del informe)

- [ ] **P5-A** — Corregir §0 GUIDE.md: símbolos `##int` (no existe) y `##type` (no existe) en la tabla de coherencia `#`. Reemplazar por `x#?`, `###`, `##.`, `##!` que sí existen. (§2.1 del informe)
- [ ] **P5-B** — §23 IMPLEMENTATION.md: fila "Modules (constants via `.`)" dice `❌|❌|Known gap` pero L3 está marcada Fixed y verificada empíricamente en TW y VM → actualizar a `✅|✅|Fixed in v0.0.4`. (§1.3 del informe)
- [ ] **P5-C** — §23 IMPLEMENTATION.md: reconciliar las filas `⚠` (features TW-only) con la afirmación "393/393 parity". Añadir nota explícita: la paridad se mide sobre el subconjunto de features soportadas por ambos modos; los features marcados `⚠` quedan excluidos de ese conteo. (§1.4 del informe)

### P5 — Problemas nuevos introducidos por los cambios (§2 del informe)

- [ ] **P5-D** — GUIDE.md §9 vs §10b asimetría captura: añadir caja de aviso explícita sobre el comportamiento `adder(5)` (aislado) vs `f = adder; f(5)` (captura). §10b generaliza incorrectamente al decir que funciones nombradas "do not capture outer variables" sin distinguir el modo de uso. (§2.2 del informe)
- [ ] **P5-E** — **Bug nuevo descubierto**: `!?/:!` corrompe el scope exterior cuando una función que referencia una variable outer falla dentro del bloque try. Reproducible: `base = 10`, `adder(n) { <~ n + base }`, luego `!? { adder(5) } :! { }`, después `>> base ¶` → `undefined variable 'base'`. Documentar en §20 REFERENCE.md como L16 *(implementation gap)* y añadir al BUG tracker. (descubierto durante verificación de P5-D)

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
| P3-B   | ✅ completo | 2026-04-22 | GUIDE.md + REFERENCE.md + IMPLEMENTATION.md; MANUALdeprecate.md conservado |
| P3-C   | ✅ completo | 2026-04-21 | Arrays homogéneos by design; L11 reescrita; named tuples para registros heterogéneos |
| P3-D   | ✅ completo | 2026-04-21 | §20 categorizado: "By Design" / "Implementation Gap"; todas las L etiquetadas |
| P3-E   | ✅ completo | 2026-04-22 | §2 expandida: value types + non-value types + tabla `#?`; `##->` unifica fn/lambda; error type = kind |
| P3-F   | ✅ completo | 2026-04-21 | Error de doc en §22: `# calc` sin llaves → corregido a `# calc { ... }`; parser siempre exige `{` |
| P3-G   | ✅ completo | 2026-04-21 | §20b "Error Taxonomy" añadida: parser/semantic/runtime + tabla fail-safe |
| P3-H   | ✅ completo | 2026-04-21 | §16 "Exception Flow vs Value Flow": `!?/:!` = excepciones, `$!/$!!` = valores; `$!!` es early-return no throw |
| P4-A   | ✅ decisión | 2026-04-22 | Mantener `arr[a>b]`; contexto resuelve ambigüedad; §11c anotado |
| P4-B   | ✅ completo | 2026-04-22 | §11 GUIDE.md: 4 razones + simetría +/-  + loop natural + índice 0 = error |
| P4-C   | ✅ decisión | 2026-04-22 | No documentar; conexión solo convergencia de gestos, no relación estructural |
| P5-A   | pendiente  |            | §0 GUIDE.md: `##int`/`##type` no existen → reemplazar por símbolos reales |
| P5-B   | pendiente  |            | §23 IMPL: fila L3 dice `❌` pero está Fixed → actualizar a `✅` |
| P5-C   | pendiente  |            | §23 IMPL: reconciliar filas `⚠` con afirmación "393/393 parity" |
| P5-D   | pendiente  |            | §9 vs §10b: añadir aviso explícito sobre asimetría captura directa/as-value |
| P5-E   | pendiente  |            | Bug nuevo: `!?/:!` corrompe scope exterior; documentar como L16 + BUG tracker |
