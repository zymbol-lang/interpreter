# Análisis de Zymbol-Lang v0.0.4 — Informe de Consultoría

**Alcance**: MANUAL.md (2777 líneas), con foco en sintaxis, semántica y coherencia del diseño.
**Perspectiva**: consultoría en diseño de lenguajes de programación.
**Tono**: crítico-constructivo. Donde algo funciona bien, lo digo. Donde veo riesgos, los marco.

---

## 1. Resumen ejecutivo

Zymbol v0.0.4 es un lenguaje con **ambición real y ejecución sólida** en varias dimensiones: cobertura de tests honesta (150/150 + 13/13), paridad tree-walker/VM documentada (243/246), soporte Unicode de digit blocks sin precedente conocido (69 scripts), y un *manual de referencia* que reconoce sus propias limitaciones abiertamente (L1–L13, sección EBNF Coverage).

Dicho esto, el lenguaje está en una **tensión de diseño** que conviene resolver explícitamente antes de comprometerse con v0.1:

> La promesa es *"minimalista simbólico, sin keywords"*, pero el inventario operacional del manual lista ~100 símbolos/operadores con semánticas contextuales. Eso ya no es minimalismo — es una **notación densa**, más cercana a APL/J/K que a Lua o Scheme. El manual todavía no asume esa identidad.

El resto del informe desarrolla esto con ejemplos y recomendaciones priorizadas.

---

## 2. Fortalezas destacables (mantener y amplificar)

### 2.1 Honestidad documental
Las secciones 20 (Known Limitations) y 23 (EBNF Coverage Status) son **ejemplares**. Muchos proyectos ocultan la brecha entre spec y realidad; Zymbol la tabula con ✅/⚠/❌. Recomendación: no perder esto nunca. Es una ventaja competitiva real frente a lenguajes que se venden por encima de lo que implementan.

### 2.2 Numeral modes (§18b)
La innovación genuina del lenguaje. 69 digit blocks Unicode, `#|"๔๒"| == 42`, y la invariante elegante de que `#` siempre es ASCII (U+0023) para que `#0` booleano nunca se confunda con `0` entero, incluso en scripts exóticos. **Esto no existe en ningún lenguaje mainstream.** Si Zymbol tiene una "historia de diferenciación" natural, es ésta.

### 2.3 Separación round/truncate en todas las dimensiones
`#.N` (round display) vs `#!N` (truncate display), `###` (round cast) vs `##!` (truncate cast). Muchos lenguajes colapsan estas operaciones y generan bugs sutiles (Python `int()` trunca, `round()` en Py3 usa banker's rounding; JS `Math.round()` redondea `-0.5→0` pero trunca `0.5→1`…). Zymbol lo hace sistemático. Bien pensado.

### 2.4 `_err` implícito en catch
Captura ergonómica. No requiere nombrar explícitamente la variable de error. Correcto.

### 2.5 Tuples inmutables vs arrays mutables
Dicotomía limpia, consistente con Erlang/Elixir/Rust. La nota que distingue *"constante de binding" (:=)* vs *"valor inmutable" (tuple)* es precisa.

### 2.6 `$^` con comparador explícito
Que la dirección (asc/desc) se codifique en el lambda (`a < b` vs `a > b`) en vez de un flag booleano es más expresivo y menos ambiguo. Bien.

---

## 3. Problemas críticos de diseño / semántica

Los ordeno por severidad decreciente.

### 3.1 🔴 Asimetría funciones / lambdas (inconsistencia fundamental)

El manual declara en §9:

- *"Functions have isolated scope — they cannot access outer variables"*
- *"Lambdas DO capture the outer scope"*

**Esto es una asimetría profunda**. En casi todos los lenguajes modernos (Python, JS, Rust, Swift, OCaml, Haskell, Scheme, Ruby, Lua), ambas formas capturan. ABAP no captura — pero ABAP tampoco tiene lambdas de primera clase. Zymbol mezcla ambos modelos y eso genera L5 + L6 + L13 como **consecuencias** de la asimetría, no como limitaciones independientes:

- **L5**: `fn = myFunc` falla porque las funciones no son valores
- **L6**: `nums$> fn` falla porque los HOF esperan lambdas literales
- **L13**: `$!!` sólo funciona en funciones (el mecanismo de error necesita stack frame con semántica de función tradicional, no closure)

**Pregunta que recomiendo hacerse**: ¿es la asimetría una decisión deliberada o un artefacto de implementación? Si es deliberada, el manual debe articular la razón (¿rendimiento? ¿simplicidad del VM? ¿herencia de ABAP?). Si es artefacto, es el ítem #1 del roadmap para v0.1.

Sospecha razonable: el register VM modelado en Lua 5 probablemente distingue internamente prototypes globales (funciones) de closures (lambdas). Lua 5 en cambio las unificó en su LClosure/CClosure. Si el diseño de Zymbol es consciente, explicarlo. Si no, considerar unificarlo.

### 3.2 🔴 Whitespace semánticamente significativo en `@label` vs `@ label`

§8, línea 664:

> *"`@label` (fused) is the loop declaration. `@ label` (with space) is a while loop where `label` is the condition variable. The space is significant."*

Esto es una **mina de bugs**. Python y Haskell tienen indentación significativa, pero es *consistente en toda la sintaxis*. Aquí un único espacio cambia semántica radicalmente en un solo constructo — el resto del lenguaje es whitespace-insensitive. Casos problemáticos previsibles:

- Auto-formatters que "normalizan espacios" romperán código.
- Copy-paste desde otra fuente con espacios invisibles (tabs, NBSP U+00A0) dará errores oscuros.
- El diff de Git no resaltará la diferencia claramente.

**Alternativa**: usar símbolo distinto. Por ejemplo `@:label` o `@#label` para etiquetas, dejando `@ expr` libre para while. Vale la pena iterar aquí antes de estabilizar.

---

#### ✅ Decisión de diseño (2026-04-21)

Se adopta **`@:label`** como separador explícito. Tabla de cambios:

| Uso | Sintaxis anterior | Sintaxis nueva |
|-----|-------------------|----------------|
| Declarar loop etiquetado | `@outer { }` | `@:outer { }` |
| Loop etiquetado + spec | `@outer i:1..10 { }` | `@:outer i:1..10 { }` |
| Break a etiqueta | `@! outer` | `@:outer!` |
| Continue a etiqueta | `@> outer` | `@:outer>` |
| Break sin etiqueta | `@!` | `@!` (sin cambio) |
| Continue sin etiqueta | `@>` | `@>` (sin cambio) |
| Loop sin etiqueta | `@ expr { }` | `@ expr { }` (sin cambio) |

**Semántica del nuevo esquema:**
- `@:` es el prefijo de toda operación de loop etiquetado.
- El nombre de la etiqueta va siempre inmediatamente después de `@:`, sin espacio.
- El símbolo de acción (`!` break, `>` continue) va al final: `@:name!`, `@:name>`.
- `@:name!` y `@:name>` son tokens únicos en el lexer (no `@:name` + operador suelto), evitando colisión con `!` (NOT) y `>` (comparación).

**Ventajas respecto al diseño anterior:**
- Elimina el whitespace semánticamente significativo: `@ count` siempre es condición, `@:outer` siempre es etiqueta.
- Simetría completa: prefijo `@:` para declarar y para referenciar (`@:name!`, `@:name>`).
- Legibilidad: en `@:outer!` el destino del break es visible antes del símbolo de acción.

**Impacto:** breaking change pre-1.0. 10 archivos de test afectados (~43 ocurrencias). El lexer emitirá error de migración si encuentra la sintaxis antigua `@ident` (sin `:`).

**Fix relacionado (2026-04-21):** el semantic checker emitía falso warning `"loop condition should be Bool, got Int"` cuando se usaba `@ count { }` con una variable entera. Corregido: cualquier condición de tipo `Int` se reconoce como TIMES loop, consistente con el comportamiento del runtime.

**Pendiente:** implementación en lexer + parser + compilador VM + migración de tests + actualización MANUAL §8 + EBNF. Ver P2-C en `ES_issues_v0_0_4.md`.

### 3.3 🔴 Colisión visual `>` en navegación vs comparación

§11c declara que `>` dentro de `[...]` es *siempre* un depth separator, nunca comparación. Y luego hace falta la regla:

> *"`arr[a>b]` donde a,b son identificadores es navegación. `arr[(a>b)]` es un índice 1D donde `(a>b)` evalúa a Bool."*

Esto es **parsing context-sensitive** a nivel humano (no solo de máquina). Costos:

- El lector humano tiene que rastrear "¿estoy dentro de unos corchetes?" para decidir qué significa `>`.
- Los IDEs con syntax highlighting basados en lexer regular no pueden colorear distinto.
- Colisión con el operador existente de redirección/output (`>>`).

**Alternativas a considerar**:
- `arr[i@j]` (usar `@` como nav separator, aunque colisiona con loops)
- `arr[i, j]` (coma, como NumPy/Julia — pero colisiona con tuples)
- `arr[i → j]` (flecha Unicode, usada ya para lambdas como `->`)
- `arr[i/j]` (slash, como paths — tampoco es perfecto)
- `arr[i.j]` (punto — lo más conciso, pero colisiona con member access)

No tengo una respuesta obvia, pero el `>` actual tiene costo cognitivo que crecerá conforme la gente use nav paths en expresiones reales.

### 3.4 🟠 Arrays homogéneos (L11) — limitación de fondo

```
record = ["English", "en.zy", #0]   // ❌ parser error
```

Los workarounds propuestos (codificar booleanos como strings, usar arrays paralelos) son regresiones a los años 70. Esto limita severamente la utilidad de arrays como contenedor general y empuja a usar tuples para casi todo — pero tuples son inmutables.

**Preguntas a responder**:
1. ¿La homogeneidad es necesaria para el VM basado en registros? (Lua 5 no la requiere; sus tables son heterogéneas.)
2. Si es por tipado, ¿no sería mejor un tipo-suma explícito `Any` o una tag sumtype que el parser deduzca?
3. Tuples ya permiten mixto — si arrays hicieran lo mismo, la distinción sería sólo mutabilidad (como Erlang: list vs tuple).

### 3.5 🟠 Encadenamiento de operadores prohibido (L10)

```
arr = [1,2,3]$+ 4$+ 5    // ❌
```

Esto es **anti-ergonómico**. Toda la tendencia moderna en APIs de colecciones (Rust iterators, JS array methods, Scala, Elixir, Haskell, LINQ) es que los ops de colección sean encadenables. El manual admite que es una limitación pero no explica el motivo.

**Causa probable**: la gramática trata `$+` como operador binario donde el lado izquierdo es "primary" y el derecho es "expression", pero después del primer `$+` el resultado no retorna al contexto primary. Solución: hacer los operadores de colección right-associative con resultado siendo de nuevo primary. Es ~1 línea de EBNF.

### 3.6 🟠 Pipe `|>` obligatoriamente con `_`

```
5 |> double(_)         // siempre
```

F#, Elixir, OCaml, Clojure todos permiten `5 |> double` cuando el valor va como primer argumento. Requerir siempre `_` quita la forma ligera del pipe y mantiene solo la "forma adversa" (valor en posición no-primera). Compromiso razonable: **permitir ambos**.

- `x |> f` ≡ `f(x)`
- `x |> f(a, _)` ≡ `f(a, x)`

Esto no daña el resto y aumenta la ergonomía en el 80% de casos.

### 3.7 🟡 Paridad VM / Tree-walker incompleta

La sección 23 muestra ⚠ en:
- `$/` split, `$++` build, casts `##.`/`###`/`##!`, format `#,|x|`/`#^|x|`, `><` CLI args

El manual promete en §1 *"both modes produce identical output on 243/246 parity tests"* pero varias filas contradicen eso. Hay dos caminos:

- **Camino A (estricto)**: paridad total antes de v0.1.
- **Camino B (honesto)**: declarar que el VM es un **subconjunto optimizado** del tree-walker, y que el tree-walker es el **intérprete canónico**. Esto es legítimo (Ruby YARV, Python cpython, etc. tenían "slow-path interpreters" y "fast-path JITs"). Hay que decidir y ser explícito.

### 3.8 🟡 Destructuring assignment — semántica sobre variables existentes indefinida

§11b dice: *"Destructuring always creates new variables — it does not update existing ones."*

¿Qué pasa si `a` ya existe en scope? ¿Error? ¿Shadow? ¿Sobrescribe? El manual no lo aclara. Y esto colisiona potencialmente con el scoping lexical de §4. Caso concreto:

```zymbol
a = 99
[a, b] = [1, 2]    // ¿cuál es el valor de a ahora?
```

Necesita decisión y documentación.

---

## 4. Problemas del MANUAL como documento

### 4.1 🔴 Falta gramática formal (EBNF) en el manual
El manual tiene una sección "EBNF Coverage Status" pero **no incluye la EBNF misma**. Para un *"authoritative reference"* (palabras del propio manual en línea 3), la gramática formal debería estar o bien incrustada o bien enlazada como apéndice normativo. Sin ella, hay construcciones ambiguas (como el caso `>` en corchetes) que dependen de la implementación.

### 4.2 🔴 Falta sección de estructura léxica
Preguntas que el manual no responde claramente:
- ¿Qué caracteres son válidos en identificadores? (El ejemplo `सक्रिय = #१` sugiere Unicode identifiers completos, pero no hay regla.)
- ¿Cuál es la sintaxis de comentarios? Solo se ven `//` en ejemplos, ningún `/* */` ni mención explícita. ¿Multilínea?
- Reglas de whitespace (especialmente relevante dado §3.2).
- Caracteres de escape en strings. Se usan `\{`, `\}`, `\"`, `\\`, pero no hay tabla completa. ¿`\n`? ¿`\t`? ¿`\uXXXX`?
- Keywords reservadas — ¿o realmente no hay ninguna? (Afirmación fuerte que merecería verificación explícita.)

### 4.3 🟠 Falta sección de semántica de evaluación
- Orden de evaluación de argumentos en llamadas: ¿izquierda a derecha? ¿no especificado?
- Orden de evaluación de operandos binarios
- Semántica de paso por referencia en `<~` params: ¿es alias compartido? ¿copy-on-write? ¿qué pasa si se lee y escribe la misma variable dos veces?
- Semántica de alias en asignación de arrays. §11 dice *"value semantics — assigning creates independent copy"*, pero no dice cuándo ocurre la copia (¿siempre? ¿on-write?).
- Semántica de captura de lambdas: ¿captura por valor o por referencia? (Ejemplo relevante: `@ i:1..5 { fns = fns$+ (x -> x + i) }` — ¿todos los lambdas capturan el mismo `i` final, o cada uno captura su iteración? En Python: uno final. En Scheme: cada iteración. Zymbol no lo define.)

### 4.4 🟠 Modelo de tipos incompleto
La tabla de §2 no incluye:
- Tipos de función / lambda (¿`##→` o algún símbolo?)
- Tipo de error (¿`##!` retorna qué si se aplica?)
- Tipo Module (lo que es `m` en `<# ./calc <= m`)
- Tipo de Range (¿`1..5` es un valor? ¿Es un tipo?)

### 4.5 🟠 Inconsistencia en sintaxis de módulos
§17 dice *"a module file contains exactly one closed block: `# name { ... }`"*. Pero §22 (Complete Module Example) usa:

```zymbol
// file: calc.zy
# calc

#> { ... }

add(a, b) { <~ a + b }
```

**Sin braces**. El lector queda confundido. O bien:
- Las braces son obligatorias (actualizar §22)
- Las braces son opcionales (actualizar §17)
- Hay dos formas válidas y deberían documentarse ambas

### 4.6 🟡 Mezcla de semántica y detalles de implementación
"Tree-walker vs VM" aparece en casi cada sección. En un **reference manual**, la semántica del lenguaje debería ser única. Los modos de ejecución son detalles del runtime. Sugerencia:
- Mover toda la discusión tree-walker/VM a un apéndice único
- El cuerpo principal describe **el lenguaje**, no las implementaciones

### 4.7 🟡 Terminología no definida
- *"runtime error"* vs *"semantic error"* vs *"parser error"* — aparecen sin una taxonomía declarada al inicio.
- *"fail-safe"* en `#|"abc"|` — se usa pero no se define (¿devuelve error tipado? ¿devuelve el string? El ejemplo sugiere lo segundo).
- *"homogeneous array"* — ¿homogéneo exacto o structural? (¿`[1, 2.0]` es Int+Float = error, o se unifica?)

### 4.8 🟡 La sección Known Limitations mezcla dos categorías
Hay dos tipos de limitaciones en §20:
- **Por diseño**: L11 (arrays homogéneos — puede ser deliberado)
- **Por implementación pendiente**: L12 (`do-while` no implementado pero en EBNF)

Conviene separarlas. Un lector quiere saber *"¿esto va a cambiar?"* — eso sólo lo contesta esta distinción.

---

## 5. Preguntas arquitectónicas abiertas (más allá del manual)

Estas no son defectos — son decisiones que conviene articular antes de v0.1. Cada una es una oportunidad para sharpening de la identidad del lenguaje.

### 5.1 ¿Quién es el usuario objetivo?
El manual combina features de:
- Lenguaje embebido (`<\ cmd \>` BashExec, `</ f.zy />` execute script)
- Lenguaje de propósito general (módulos, HOF, closures)
- Lenguaje exótico i18n (69 digit scripts)

Cada audiencia tiene prioridades distintas. **Pregunta**: ¿shell scripting mejor que bash? ¿DSL para administración pública (que conecta con ParasolOS)? ¿Herramienta educativa para i18n? Una respuesta clara guía qué features pulir primero.

### 5.2 Relación con APL/J/K
Los lenguajes array-oriented históricos son los únicos precedentes reales para la densidad simbólica de Zymbol. ¿Qué aporta Zymbol que esos no hagan? Si la respuesta es *"syntactic familiarity occidental + Unicode digits + 1-based indexing"*, dilo explícitamente. Es un nicho legítimo.

### 5.3 1-based indexing como declaración
La decisión es claramente herencia de ABAP (20 años de experiencia). Lua, R, Julia, Matlab, Fortran la comparten. Es legítima. Pero el manual no la defiende. Mi sugerencia: una sección breve *"Why 1-based"* o al menos una nota en §11. Un lenguaje nuevo que elige la opción minoritaria debe justificarla, o parecerá descuido.

### 5.4 La promesa "sin keywords"
Estrictamente sí — no hay `if`, `while`, `function`, `return`, etc. Pero el inventario de símbolos a memorizar supera al de lenguajes con keywords. ¿Es *"reemplazar keywords por símbolos"* un ahorro cognitivo real o una traducción? Esta pregunta merece contestarse empíricamente (tests de onboarding con usuarios nuevos) antes de vender el lenguaje bajo esa promesa.

### 5.5 Manejo de errores — ¿excepciones o valores?
El sistema actual mezcla ambos:
- `!?` / `:!` / `:>` = excepciones al estilo Java
- `$!` / `$!!` = errores como valores al estilo Rust/Go

La mezcla no es incoherente (Rust tiene `panic!` y `Result`), pero merece un documento arquitectural que explique *cuándo usar cada uno* y *qué garantías dan*. ¿`$!!` desde una función llamada desde `!?` se captura en el catch? El manual no lo dice.

---

## 6. Recomendaciones priorizadas

### Prioridad 1 — Fundamentos (antes de v0.1)
1. **Resolver asimetría función/lambda** (§3.1). Ítem #1 de riesgo de diseño. Documentar o cambiar.
2. **Paridad VM↔Tree-walker o declaración explícita de subset** (§3.7).
3. **Agregar estructura léxica al manual** (§4.2). Al menos: identifiers, comments, escapes, whitespace rules.
4. **Incluir EBNF normativa** como apéndice (§4.1).
5. **Resolver destructuring sobre variables existentes** (§3.8).

### Prioridad 2 — Ergonomía
6. **Encadenamiento de operadores `$+`, `$-`, etc.** (§3.5). Alto ROI, probablemente poco esfuerzo.
7. **Pipe sin `_` obligatorio** cuando el valor va en primera posición (§3.6).
8. **Reconsiderar `@label` vs `@ label`** con símbolo distintivo (§3.2).
9. **Documentar orden de evaluación y semántica de captura** (§4.3).

### Prioridad 3 — Claridad y posicionamiento
10. **Articular identidad del lenguaje**: minimalismo simbólico vs densidad funcional (§5.4).
11. **Separar manual de usuario / reference manual / implementation notes** (§4.6).
12. **Decidir estrategia con arrays homogéneos** (§3.4). Si se mantiene: explicar por qué. Si se relaja: roadmap.

### Prioridad 4 — Investigación futura
13. **Reconsiderar `>` en navegación** (§3.3). No urgente pero lo será cuando haya codebase grande.
14. **"Why 1-based" manifiesto breve** (§5.3).
15. **Documento arquitectural sobre manejo de errores** (§5.5).

---

## 7. Qué no está roto (explícitamente)

Para contrapeso de lo crítico: estas decisiones son **correctas** y no las tocaría:

- Separación `+` (aritmético) vs juxtaposition en `>>` (string). Valiente y coherente.
- `:=` para constantes vs `=` para variables. Distinción clara.
- `_`-prefixed variables con scope exacto de bloque. Diseño sofisticado, justificado.
- Distinción round vs truncate en casts y formato.
- Arrays mutables + tuples inmutables como dicotomía primaria.
- `$^+` / `$^-` para primitivos vs `$^` con comparador para compuestos.
- El hecho de documentar limitaciones explícitamente (§20) y coverage gaps (§23).
- 1-based indexing con índices negativos simétricos (`arr[1]` / `arr[-1]`). Consistente.
- Numeral modes completos (§18b) — joya del lenguaje.

---

## 8. Cierre

Zymbol v0.0.4 tiene **más madurez que la mayoría de lenguajes en su fase**, especialmente en rigor de testing y honestidad documental. Los problemas señalados son mayoritariamente *decisiones de diseño no articuladas* más que *errores de implementación*. Eso es una buena noticia: se resuelven con pensamiento, no con refactoring masivo.

El riesgo real es el otro: que el lenguaje se estabilice en v0.1 **sin haber decidido** sobre los puntos de §3 y §5. Cada uno de ellos será 10× más costoso de revisar después.

Recomendación final: tomar las 5 preguntas de §5 como *design review explícito* antes de v0.1, y tratar las P1 como bloqueadores de release. El resto puede seguir iterando.
