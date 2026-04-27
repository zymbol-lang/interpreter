# Zymbol-Lang v0.0.4 — Revisión de Segunda Pasada

**Alcance**: verificación del MANUAL.md actualizado (3602 líneas, +825 vs primera revisión) contra los issues declarados como completados en `ES_issues_v0_0_4.md`.

**Conclusión general**: el trabajo es sustancial y de calidad. De las 12 issues P0+P1+P2 marcadas como completas, **11 están implementadas correctamente** en código según el propio registro. Lo que falla no es la implementación — es la **sincronización documental**. Los cambios dispersos dejaron varias secciones del manual contradictorias entre sí. También hay 2 problemas nuevos introducidos por las propias mejoras que conviene cazar antes de que se solidifiquen.

---

## 1. Inconsistencias internas del manual (crítico)

Estas son el tipo de error que degrada la credibilidad de un *authoritative reference* más rápido que cualquier feature faltante. Todos son arreglos textuales pequeños.

### 1.1 🔴 L5 y L6 siguen vivas en §20, pero están resueltas en §9

§20 líneas 2716–2731 mantienen:

```
### L5 — Named functions are not first-class values
Symptom: fn = myFunc → "undefined variable 'myFunc'".
fn = x -> myFunc(x)          // ✅ wrap in lambda

### L6 — HOF $>, $|, $< require inline lambdas
nums$> fn              // ❌ not accepted
nums$> (x -> fn(x))    // ✅ always works
```

Pero §9 línea 984 muestra:

```
r = nums$> double        // ✅ direct reference
r = nums$| is_big        // ✅ direct reference
```

Y el registro declara P0-A y P2-A completos. **Acción**: tachar L5 y L6 como `~~L5~~ Fixed in v0.0.4` / `~~L6~~ Fixed in v0.0.4`, siguiendo el patrón ya usado para L3, L4, L7, L8, L10.

### 1.2 🔴 Header dice 150/150, cuerpo dice 393/393

Línea 8: *"Test coverage: 150/150 interpreter PASS (wt + vm); 13/13 index_nav PASS"*

Línea 117 (§1): *"Both modes produce identical output on 393/393 parity tests."*

Apéndice A encabezado: *"version 2.3.0, sprint v0.0.4_1"*

El registro de issues confirma 393/393 como cifra correcta. Actualizar la línea 8 para evitar que un lector dude cuál cifra es la real.

### 1.3 🔴 Tabla §23 dice que L3 es "Known gap" pero §20 dice "Fixed"

§20 línea 2681: *"~~L3 — Module alias.CONST does not work~~ Fixed"*
§23 línea 3115: *"Modules (constants via `.`) | ❌ | ❌ | Known gap"*

Contradicción directa. Probablemente la fila de §23 está desactualizada. Revisar empíricamente: si `m.PI` funciona, cambiar a ✅|✅; si no, reabrir L3.

### 1.4 🟠 Tabla §23 contradice la afirmación "393/393 paridad"

§23 lista seis filas con disparidad explícita:

| Feature | WT | VM | Nota |
|---------|----|----|------|
| `$/` String split | ✅ | ⚠ | VM: Unsupported (tree-walker only) |
| `$++` String build | ✅ | ⚠ | VM: Unsupported (tree-walker only) |
| `##.` / `###` / `##!` Casts | ✅ | ⚠ | VM: Unsupported (tree-walker only) |
| `#,\|x\|` / `#^\|x\|` Format | ✅ | ⚠ | Full parity pending in VM |
| `><` CLI args | ✅ | — | VM not supported |
| `$!!` from lambdas (L13) | ❌ | ❌ | — |

Si el registro dice 393/393 parity, **una de dos**:
- (a) La "paridad" se mide sobre tests que evitan estos features → entonces el manual debería decir *"393/393 pass on the shared feature subset; features marked ⚠ in §23 are tree-walker only"*.
- (b) Estos features ahora hacen algo consistente en VM (por ejemplo emiten el mismo error) → entonces §23 debería reflejarlo.

Tal como está, un lector atento piensa: *"¿en qué quedamos, hay paridad total o no?"*.

---

## 2. Problemas nuevos introducidos por los cambios

### 2.1 🔴 §0 menciona símbolos que no existen

§0 Design Philosophy, tabla "#":

> *"Type reflection | `##int`, `##->`, `##type` | introspect the type of a value"*

Verificación:
- `##->` sí existe (añadido por P0-B, aparece en §2 como símbolo de Function).
- `##int` **no existe** — en §2 el símbolo de Int es `###`, no `##int`.
- `##type` **no existe** — la introspección de tipo es `x#?` (postfix), no `##type` (prefix).

Este es el primer ejemplo de uso real de la filosofía articulada, y contiene dos inventos. Debilita toda §0 porque es la sección que vende la coherencia simbólica. **Acción**: reemplazar por símbolos que realmente existen. Sugerencia:

```
| Type reflection      | `x#?`, `##->`       | introspect the type of a value |
| Type cast             | `##.x`, `###x`, `##!x` | numeric transformations |
```

### 2.2 🟠 §9 vs §10b — la asimetría función/lambda se contradice ahora

§9 línea 918 dice:

> *"Functions used as first-class values capture the scope at the point of assignment (like lambdas)"*

§10b línea 1142 dice:

> *"Named functions (`name(params) { }`) execute in a fully isolated scope — they do not capture outer variables and cannot read or write the caller's locals."*

Ambas afirmaciones son ciertas en distintas circunstancias — **pero están escritas como si fueran generales**. La verdad matizada es:

- Llamada directa por nombre: aislado (§10b tiene razón en ese caso)
- Llamada después de asignación a variable: captura (§9 tiene razón en ese caso)

Esto es lo que yo llamaría **"defunctionalization at reference time"**: cuando el identificador `fn` se *lee como valor*, en ese momento se materializa como cierre. Cuando `fn` se *invoca directamente*, no pasa por esa ruta.

La semántica es defendible, pero pone al programador en una posición incómoda:

```zymbol
base = 10
adder(n) { <~ n + base }

adder(5)        // runtime error: base not in scope
f = adder       // captura scope: {base: 10}
f(5)            // → 15
```

Tres efectos secundarios no triviales:

1. **No hay transparencia referencial entre llamada y reificación**: `adder(5)` y `(f = adder)(5)` no son equivalentes. Esto sorprende a cualquier programador funcional.
2. **Refactor inocente rompe código**: si alguien toma una llamada directa que funciona y la factoriza a variable para reusar, *puede* romper la llamada original si dependía de variables externas.
3. **Difícil de enseñar**: la regla no es *"las funciones no capturan"* ni *"las funciones sí capturan"*, sino *"las funciones capturan iff son leídas como valor"*. Es la primera vez que veo esta semántica en un lenguaje.

**Recomendación**: o bien unificar (todas capturan siempre, en ambos modos de uso), o bien mantener pero **documentar explícitamente la asimetría con un aviso destacado**. La redacción actual tiene frases que se contradicen en secciones separadas. Una caja tipo:

> ⚠ **Asymmetric capture**: A named function called directly (`fn(args)`) does **not** see outer variables. Reading the function's name as a value (`f = fn`) creates a closure that captures outer scope. The two forms are **not** interchangeable when the function body references outer names.

Eso al menos elimina la contradicción y convierte el comportamiento en una decisión explícita del usuario.

---

## 3. Lo que quedó bien (confirmación)

Para contrapeso de lo crítico: estas secciones del nuevo manual están bien ejecutadas.

### 3.1 ✅ §0 Design Philosophy (salvo el bug de §2.1)
La articulación *"minimalismo simbólico propio, no APL, convergencia independiente desde ABAP"* es honesta y defensible. La tabla de coherencia de `_` (non-binding) y `#` (meta-level) son **ejemplos reales** de cómo un diseño simbólico puede tener estructura interna — no solo glifos aleatorios. Esto responde directamente a mi objeción §A.3.1 y §A.5.4 originales.

### 3.2 ✅ §1b Lexical Structure — completo y correcto
Cubre las 5 cosas que faltaban: identifiers, comentarios (con nesting), whitespace (con la excepción documentada), escapes, literales. La aclaración *"any other `\X` sequence passes X through unchanged"* es buena política — evita tener que enumerar todas las no-escapes. La nota sobre ausencia de `\uXXXX` es correcta para mantener minimalismo.

### 3.3 ✅ §10b Evaluation Order and Capture Semantics
Lo que pedí en §A.4.3. La sección sobre **loop closures con snapshot por iteración** es particularmente importante y está bien explicada: el contraste con Python (*"Python's late-binding default loops"*) orienta al lector que viene de otro lenguaje. La sección sobre **writes to captured variables stay local** también es exactamente el tipo de semántica que la gente asume mal si no se documenta.

### 3.4 ✅ `@:label` / `@:label!` / `@:label>`
Mejor que la alternativa original `@label` vs `@ label`. El `:` como separador tiene precedente en el propio lenguaje (rangos `1..5:2`, pattern match `pat : value`, for-each `i:arr`). Consistencia interna.

### 3.5 ✅ Pipe sin `_` obligatorio
Implementación elegante — la ausencia de `()` implica `f(_)`. Código como `5 |> double |> inc |> double` es ahora legible. El diseño mantiene la explicitez cuando el valor va en segunda posición (`add(_, 5)`). Buen compromiso.

### 3.6 ✅ L14 descubierta y documentada
El bug *"destructuring ignora `:=`"* encontrado durante la validación de P1-E es exactamente el tipo de hallazgo que sólo aparece cuando se documenta formalmente. El hecho de que se haya identificado, documentado y dejado como known limitation (no escondido) es profesional.

### 3.7 ✅ L15 descubierta y corregida
`arr[i](args)` no invocaba la lambda en contexto `>>` — encontrado durante documentación de §10b, arreglado el mismo día. Esto es exactamente cómo debería funcionar el ciclo "documentar para descubrir bugs". Bien.

### 3.8 ✅ Apéndice A EBNF normativa
La gramática completa está. Las anotaciones `[NOT IMPLEMENTED]` y `[WT only]` son precisas. La sección de "retired operators" al final es una cortesía al lector que migra desde v0.0.2/v0.0.3 — buen detalle.

---

## 4. Resumen de lo que queda pendiente (del propio registro)

Para cerrar la v0.0.4 como release limpio, el registro marca:

- **P3-B** aplazado (separar manual usuario / reference / impl notes) — decisión razonable
- **P3-C** arrays homogéneos — decisión arquitectónica, no bloqueante de v0.1
- **P3-D** separar limitaciones "por diseño" vs "por implementación"
- **P3-E** modelo de tipos completo (parcial: §0 y §2 ya incorporan Function y Error; falta Module, Range)
- **P3-F** braces obligatorias vs opcionales en módulos (§17 vs §22)
- **P3-G** taxonomía de errores (runtime / semantic / parser)
- **P3-H** cuándo usar `!?/:!/:>` vs `$!/$!!`

De estos, **P3-F es el más urgente** porque es una contradicción activa dentro del manual (§17 exige braces, §22 usa sin braces). Los demás son adiciones, no correcciones.

**P3-D** es un ejercicio mayoritariamente editorial pero importante: ahora mismo un lector no puede distinguir *"L11 arrays homogéneos — esto no cambiará"* de *"L12 do-while — esto se va a implementar"*. Sugerencia de estructura:

```
### 20.1 Design Limitations (by choice)
L11 — Arrays homogéneos
L13 — $!! desde lambdas (por diseño: lambdas no tienen stack frame nombrable)

### 20.2 Pending Implementations (planned)
L12 — do-while
L14 — destructuring ignora :=

### 20.3 Workaround-only Warnings
L1 — postfix en >>
L9 — falsos positivos del analizador
```

---

## 5. Nueva lista de acciones priorizadas

### Urgente — antes de v0.1 (todas son ediciones de texto)

1. Tachar **L5** y **L6** en §20 como resueltas.
2. Actualizar línea 8 del header: **150/150 → 393/393** (o decir ambos: tests interpretador + tests de paridad, explicando qué mide cada uno).
3. Corregir §0: reemplazar `##int` / `##type` por símbolos reales (`x#?`, `###`, `##.`, `##!`).
4. Resolver contradicción **L3 fixed vs §23 known gap** (verificar empíricamente qué dice el código).
5. Reconciliar **§23 "⚠" rows con "393/393 parity"** — clarificar qué mide la paridad.
6. Caja destacada en §9 sobre la **asimetría captura directa/as-value** (o, alternativa más radical, eliminar la asimetría unificando el comportamiento).

### Ya están planificadas — ejecutar según cronograma
7. P3-F (braces módulos) — corregir §22 para casar con §17, o viceversa.
8. P3-D (limitaciones por diseño vs por implementación) — reestructurar §20.
9. P3-E — añadir Module y Range al modelo de tipos.
10. P3-G y P3-H — documentos arquitecturales nuevos.

### Largo plazo (no bloquea v0.1)
11. P3-C arrays homogéneos.
12. P4-A/B/C — investigación sobre `>` nav, 1-based manifesto, diferenciación APL.

---

## 6. Cierre

El trabajo entre mi primera revisión y esta segunda es serio. **Cerraron 12 de 21 issues declaradas, con código y documentación en todas ellas.** Las P0 y P1 están resueltas en fondo (las funciones-como-valores son un cambio arquitectural real, no cosmético). Las nuevas secciones (§0, §1b, §10b, Apéndice A) elevan el manual de "reference ambiguo" a "reference con pretensiones normativas reales".

Los problemas que identifico en esta segunda pasada son de **dos clases**:

- **Sincronización documental** (§1): cambios en una sección que no se propagaron a otras. Son arreglos de 5-30 minutos cada uno, no requieren decisiones.
- **Un problema de diseño nuevo** (§2.2): la asimetría función-directa vs función-como-valor es un hallazgo real. Merece una decisión explícita: o unificar o destacar como decisión consciente.

Recomendación final: **antes de cerrar v0.0.4 haz una pasada de "coherence sweep"** sobre el manual entero buscando referencias cruzadas rotas. Un script simple que grep las menciones a `L1` ... `L15` y verifique que cada una aparezca consistente en §20, §23 y el cuerpo haría la mayoría del trabajo.

La v0.0.5 está **genuinamente cerca**. El esqueleto del manual ya es normativo. Resuelve P3-F y los seis puntos de §5 de este informe, y tienes release candidate.
