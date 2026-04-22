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

- [x] **P5-A** — §0 GUIDE.md: `##int` y `##type` eliminados — violan el dogma no-keywords (son palabras latinas, no símbolos). Reemplazados por `x#?`, `##->`, `###`, `##.` — todos símbolos puros verificados. (§2.1 del informe)
- [x] **P5-B** — §23 IMPLEMENTATION.md: fila L3 actualizada `❌|❌|Known gap` → `✅|✅|Fixed in v0.0.4 (~~L3~~)`. Verificado empíricamente en TW y VM. (§1.3 del informe)
- [x] **P5-C** — §23 IMPLEMENTATION.md: nota añadida antes de la tabla explicando que "393/393 parity" cubre solo features ✅|✅; los tests de features `⚠` (TW-only) y `—` corren solo contra tree-walker y no forman parte del conteo. Leyenda de símbolos añadida. (§1.4 del informe)

### P5 — Problemas nuevos introducidos por los cambios (§2 del informe)

- [x] **P5-D** — §10b GUIDE.md: "Named Functions vs Lambdas" reescrita con aviso ⚠ explícito. Tabla de asimetría: llamada directa = aislado, como valor = captura snapshot. Ejemplo verificado con ambos modos. §9 y §10b ahora son consistentes. (§2.2 del informe)
- [x] **P5-E** — L16 añadida a §20 REFERENCE.md: descripción completa, diferencia TW/VM, hipótesis de causa raíz, workaround. BUG-NEW-07 añadido a `tests/BUG_v0.0.4.md` con reproducción mínima y non-trigger. (descubierto durante verificación de P5-D)

---

## Prioridad 6 — Tercera pasada: auditoría VM (2026-04-22)

Descubiertos al ejecutar `vm_compare.sh` tras crear los tests P0-A→P5-E.
Fuente: inspección empírica + lectura de `zymbol-compiler/src/lib.rs` y `zymbol-vm/src/lib.rs`.

### P6-A — Tabla ⚠ incorrecta: cuatro features ya tienen paridad TW+VM

La tabla §23 de `IMPLEMENTATION.md` marcaba como `⚠ TW-only` cuatro features que ya tienen
bytecode e instrucciones VM correctamente implementadas. Verificado con tests reales antes
de documentar.

| Feature | Antes | Después | Tests que lo confirman |
|---------|-------|---------|------------------------|
| `$/` string split | ⚠ | ✅\|✅ | `strings/14_split_operator.zy`, `v0.0.4_review/string_split_concatbuild.zy` |
| `$++` concat-build | ⚠ | ✅\|✅ | `strings/15_concat_build.zy`, `v0.0.4_review/concatbuild_array.zy` |
| `##.` / `###` / `##!` casts | ⚠ | ✅\|✅ | `casts/01_to_float.zy` … `casts/06_cast_negative.zy`, `v0.0.4_review/cast_all_operators.zy` |
| `#,\|x\|` / `#^\|x\|` format | ⚠ | ✅\|✅ | `strings/12_format_operators.zy`, `strings/13_format_precision.zy` |

- [x] **P6-A** — Tabla §23 IMPLEMENTATION.md corregida: cuatro filas `⚠` → `✅|✅`. (2026-04-22)

### P6-B — Named functions como first-class values: gap real en VM

`f = namedFn`, `arr$> namedFn`, `namedFn |> implicitPipe` — todos fallan en VM con
`VM compile error: undefined variable 'name'`.

**Causa raíz**: el compilador resuelve identificadores con `ctx.get_reg()` → `global_consts` →
`global_var_map`. Las funciones definidas con `fn(args) { }` solo existen en `self.functions[]`
y `self.function_index`, nunca en los registros ni en las tablas de variables. No existe la
instrucción `LoadFunction(dst, func_idx)` en el bytecode.

**Alcance**: cualquier uso de función nombrada como valor: asignación, HOF (`$>`, `$|`, `$<`),
pipe implícito, destructuring de retorno. Las lambdas no tienen este problema porque se
compilan a `MakeClosure`.

**Tests `@vm-skip`**: `analysis/p0a_named_fn_firstclass.zy`, `analysis/p2b_implicit_pipe.zy`,
`analysis/p3e_type_model.zy`, `analysis/p5d_fn_capture_asymmetry.zy`.

**Fix requerido**:
1. Añadir instrucción `LoadFunction(dst: Reg, func_idx: u32)` al bytecode.
2. En resolución de identificadores del compilador: si el nombre está en `function_index`, emitir `LoadFunction`.
3. En el VM: `LoadFunction` crea un `Value::Closure` (o variante equivalente) a partir del índice.

- [ ] **P6-B** — VM: implementar `LoadFunction` para named functions como first-class values.

### P6-C — `$!!` (ErrorPropagate) no compilado en VM

`Expr::ErrorPropagate` no tiene case en el match del compilador — cae en el arm catch-all
`_ => Err(CompileError::Unsupported(format!("expression {:?}", std::mem::discriminant(expr))))`,
produciendo `VM compile error: unsupported construct: expression Discriminant(44)`.

`Expr::ErrorCheck` (`$!`) sí tiene case en el compilador (emite `IsError`) pero el VM
siempre devuelve `#0` — ver P6-D.

**Test `@vm-skip`**: `analysis/p3h_error_flows.zy` (falla por este error + P6-D).

**Fix requerido**:
1. Añadir case `Expr::ErrorPropagate` en el compilador: compilar el inner expr, emitir `IsError` y un `ReturnIfError` condicional (o instrucción dedicada `PropagateError`).
2. El VM necesita manejar `PropagateError`: si el valor en el registro es un error, hacer early return con ese valor.

- [ ] **P6-C** — VM: compilar `Expr::ErrorPropagate` (`$!!`).

### P6-D — `$!` (IsError) siempre devuelve `#0` en VM

`Instruction::IsError` está implementado en el VM pero con un stub permanente:

```rust
&Instruction::IsError(dst, _src) => {
    // In the VM, error values never exist in registers (errors are caught by
    // try/catch and stored in _err as String). So $! always returns #0 for
    // any value that can appear in a register.
    self.reg_set(dst, Value::Bool(false));
}
```

Este supuesto era válido antes de v0.0.4 cuando los errores no podían ser valores de retorno.
Con el sistema de error-como-valor (`<~ _err` desde `:!`), los errores SÍ pueden estar en
registros. El stub hace que `r$!` siempre sea `#0` — resultado silenciosamente incorrecto.

**Diferencia TW vs VM**:
- TW: `safe([1,2,3], 99)$!` → `#1` (correcto)
- VM: `safe([1,2,3], 99)$!` → `#0` (incorrecto, no crashea)

**Test**: `analysis/p6d_is_error_vm_gap.zy` (nuevo, `@vm-skip`).

**Fix requerido**: `IsError` debe verificar si el valor en `src` es `Value::Error(_)` y devolver
`Value::Bool(true/false)` según corresponda.

- [ ] **P6-D** — VM: corregir `IsError` stub — verificar tipo real del registro.

### P6-E — Tuple `$+` append falla en VM con TypeError

`ArrayPush` en el VM solo maneja `Value::Array`. Si el receptor es `Value::Tuple`, lanza
`Runtime error: type error: expected Array, got Tuple`.

En el TW, `$+` sobre una Tuple funciona y devuelve una nueva Tuple con el elemento agregado.

**Test `@vm-skip`**: `analysis/p2a_append_chaining.zy` (las filas de array pasan, la de tuple falla).

**Fix requerido**: extender el arm `Instruction::ArrayPush` en el VM para manejar
`Value::Tuple(rc_items)` → `Rc::make_mut(rc_items).push(val)`.

- [ ] **P6-E** — VM: extender `ArrayPush` para manejar `Value::Tuple`.

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
| P5-A   | ✅ completo | 2026-04-22 | §0 GUIDE.md: `##int`/`##type` eliminados (violan dogma no-keywords); reemplazados por `x#?`, `##->`, `###`, `##.` |
| P5-B   | ✅ completo | 2026-04-22 | §23 IMPL: L3 `❌|❌` → `✅|✅|Fixed in v0.0.4`; verificado TW+VM |
| P5-C   | ✅ completo | 2026-04-22 | §23 IMPL: nota de paridad + leyenda añadidas; ⚠/— excluidos del conteo de 393 |
| P5-D   | ✅ completo | 2026-04-22 | §10b: tabla asimetría + aviso ⚠; directa=aislado, as-value=snapshot; verificado |
| P5-E   | ✅ completo | 2026-04-22 | L16 en REFERENCE.md + BUG-NEW-07 en BUG_v0.0.4.md; TW/VM diferencia documentada |
| P6-A   | ✅ completo | 2026-04-22 | Tabla §23: `$/`, `$++`, casts, format corregidas de ⚠ a ✅\|✅; verificado con 9 tests reales |
| P6-B   | ✅ completo  | 2026-04-22 | VM: identifier lookup → `MakeFunc` para named fns; `TypeOf` `##->` + arity; 399/404 parity (+2); p0a+p2b ahora TW+VM ✅ |
| P6-C   | ✅ completo  | 2026-04-22 | VM: `Expr::ErrorPropagate` compilado (IsError + JumpIfNot + Return); combinado con P6-D |
| P6-D   | ✅ completo  | 2026-04-22 | VM: `Value::Error(ZyStr)` añadido; `TryCatch` → `Value::Error`; `IsError` verifica tipo real; 401/404 (+2) |
| P6-E   | ✅ completo  | 2026-04-22 | VM: `ArrayPush` extendido a `Value::Tuple` (ambos handlers); p2a ahora TW+VM ✅; 402/404 parity |
