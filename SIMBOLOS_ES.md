# Zymbol — Referencia Semiótica y Morfológica

> **Qué es este documento.** Una descripción del sistema de signos de Zymbol: el inventario de
> marcas, las reglas por las que las marcas se combinan en operadores, el significado que
> aporta cada marca y — declarado explícitamente en vez de disimulado — cada lugar donde esas
> reglas no se cumplen.
>
> **Qué no es.** No es un tutorial (`GUIDE.md`), no es una tabla de consulta de comportamiento
> (`REFERENCE.md` §21), y no es una gramática (`zymbol-lang.ebnf`). Esos tres responden *qué
> hace un operador*. Este responde *por qué el operador tiene la forma que tiene*, y *qué forma
> puede tener un futuro operador*.
>
> **Cómo leerlo.** Las Partes I–IV describen el sistema tal como es; la Parte V dice qué forma
> puede tener un futuro operador. Están escritas en presente y sin fechas, porque un sistema de
> signos no es un registro de cambios. Todo lo que pertenece al tiempo — qué versión acuñó qué,
> y qué encontró este análisis al contrastarlo con el lenguaje en ejecución — está reunido en la
> [Parte VI](#parte-vi--diacronía-y-hallazgos) y en ningún otro lugar.
>
> **Método.** La descripción es de la implementación, no de documentación anterior. El
> inventario de grafemas es el predicado `is_operator_char` en
> `crates/zymbol-lexer/src/lib.rs`; el inventario de operadores es `TokenKind` en el mismo
> archivo; las afirmaciones de comportamiento fueron ejecutadas, no recordadas.

> **Nota de traducción.** Esta es una versión en español de `SYMBOLS.md`, generada para
> revisión. El documento canónico del repositorio permanece en inglés (`CLAUDE.md` así lo
> exige); este archivo es una copia de trabajo, no un sustituto.

---

## Tabla de contenidos

**Parte I — El sistema de signos**
1. [Qué tipo de sistema de signos es Zymbol](#1-qué-tipo-de-sistema-de-signos-es-zymbol)
2. [El inventario de grafemas](#2-el-inventario-de-grafemas)
3. [Aglutinación: la afirmación y sus límites](#3-aglutinación-la-afirmación-y-sus-límites)

**Parte II — Morfología**
4. [Clases de morfemas según su posición](#4-clases-de-morfemas-según-su-posición)
5. [Procesos productivos](#5-procesos-productivos)
6. [Alomorfía y variación libre](#6-alomorfía-y-variación-libre)

**Parte III — El léxico de morfemas**
7. [Cómo leer una entrada](#7-cómo-leer-una-entrada)
8. [Núcleos de dominio](#8-núcleos-de-dominio)
9. [Marcas de operación y modalidad](#9-marcas-de-operación-y-modalidad)
10. [Marcas estructurales](#10-marcas-estructurales)
11. [Delimitadores y marcas literales](#11-delimitadores-y-marcas-literales)

**Parte IV — Dónde el sistema no es regular**
12. [Alosemia: una marca, lectura determinada por el anfitrión](#12-alosemia-una-marca-lectura-determinada-por-el-anfitrión)
13. [Homógrafos declarados](#13-homógrafos-declarados)
14. [Signos opacos](#14-signos-opacos)
15. [Residuo de lenguaje natural](#15-residuo-de-lenguaje-natural)
16. [Restricciones de contexto y herencia de restricciones](#16-restricciones-de-contexto-y-herencia-de-restricciones)

**Parte V — Normativa**
17. [Reglas de diseño para nuevos operadores](#17-reglas-de-diseño-para-nuevos-operadores)
18. [El registro de combinaciones ocupadas](#18-el-registro-de-combinaciones-ocupadas)

**Parte VI — Diacronía y hallazgos**
19. [Diacronía del sistema de signos](#19-diacronía-del-sistema-de-signos)
20. [Qué encontró la descripción del sistema](#20-qué-encontró-la-descripción-del-sistema)

**Apéndices**
- [A. Convenciones de glosado](#apéndice-a--convenciones-de-glosado)
- [B. Índice de grafemas](#apéndice-b--índice-de-grafemas)
- [Notas](#notas)

---

## Parte I — El sistema de signos

### 1. Qué tipo de sistema de signos es Zymbol

#### 1.1 La notación junto al lenguaje

La restricción fundacional de Zymbol — ninguna palabra clave, en ningún idioma natural — se
argumenta en `GUIDE.md` §0 y no se vuelve a argumentar aquí. La consecuencia que importa para
este documento es estructural: como ninguna construcción puede ser una palabra, toda
construcción tiene que ser una **marca o una secuencia de marcas**, y el lenguaje necesita
entonces una explicación explícita de qué significan sus marcas y cómo se combinan. Las
palabras clave del lenguaje natural traen su significado desde afuera; las marcas no. Un
lenguaje sin palabras clave tiene que aportar ese significado él mismo, en un documento como
este, o la coherencia queda solo declarada.

#### 1.2 La afirmación "sin palabras clave", enunciada con precisión

La afirmación que sobrevive el contacto con la implementación es:

> **Ninguna construcción de la gramática es una palabra.** El control de flujo, la E/S, el
> tipado, la estructura de módulos, las operaciones sobre colecciones y el manejo de errores se
> expresan enteramente con marcas de un inventario cerrado (§2.1), y ese inventario no contiene
> letras.

Tres cosas que la afirmación **no** cubre, todas reales y todas léxico y no gramática:

| Residuo | Ejemplo | Por qué no es una violación de la gramática |
|---|---|---|
| Nombres de tipos de error | `:! ##Index { }` | `##` es la gramática; `Index` llena una ranura de identificador abierta |
| Nombres de la biblioteca estándar | `json::decode_map(s)` | Las rutas de módulo y los nombres de función son identificadores, como los de cualquier usuario |
| Identificadores convencionales | `_err` | No reservado; solo convención |

El §15 da el residuo completo y argumenta por qué eliminarlo costaría más de lo que aporta. La
disciplina importante es que la frontera esté **declarada**, de modo que "Zymbol no tiene
palabras clave" signifique algo verificable en vez de algo aspiracional.

#### 1.3 Icono, índice, símbolo

Las marcas de Zymbol no son uniformes en cómo significan. Usando la distinción tripartita de
Peirce como herramienta de clasificación — no como decoración —, el inventario se divide así, y
la división tiene una consecuencia práctica.

| Modo | Cómo significa | Ejemplos en Zymbol |
|---|---|---|
| **Icónico** | se parece a lo que significa | `>>` `<<` `->` `<~` `\|>` `..` `>` `<` `<>` `><` `##"` `##'` `##]` `##)` `##()` `##->` `#०९#` |
| **Indicial** | señala un contexto en vez de representarlo | `°` `_` `@:etiqueta` … `@:etiqueta!` |
| **Convencional** | arbitrario; hay que aprenderlo | `$` `@` `#` `¶` `?` `!` `~` `#1` / `#0` |

La consecuencia: **los signos icónicos se enseñan a sí mismos y los convencionales no.** Un
lector que nunca ha visto Zymbol adivinará correctamente `>>` y `->`, y jamás adivinará `$^-`.
Este documento existe para la tercera fila. Es también la razón por la que esa tercera fila debe
mantenerse pequeña: cada marca convencional es un costo de memorización que la iconicidad no
impone.

Vale la pena aislar dos pares icónicos, porque son pares mínimos que hacen el principio
refutable:

```
>  <          <  >
convergen     divergen
= entrada en la frontera del proceso    = los dos lados difieren
= ><  (argumentos de la línea de comandos)    = <>  (distinto de)
```

Ninguno de los dos se deriva de una plantilla de ranuras. Ambos se leen de inmediato como
imágenes. El §3.4 los clasifica en consecuencia.

---

### 2. El inventario de grafemas

#### 2.1 La clase de operadores — cerrada, 29 caracteres

Este es el conjunto completo de caracteres que Zymbol reserva para operadores. Un carácter de
este conjunto nunca puede aparecer en un identificador; uno fuera de él sí puede (sujeto al
§2.3). El conjunto es el predicado `is_operator_char`, citado tal cual:

```
>  <  =  !  +  -  *  /  %  ^  &  |  ?  :  .  ,  ;  (  )  [  ]  {  }  @  ~  #  $  ¶  \
```

Veintinueve marcas. Dos observaciones sobre la composición del conjunto:

- **Veintiocho son puntuación ASCII.** La única excepción es `¶` (U+00B6). Un inventario
  alcanzable desde el teclado fue una restricción deliberada, y `¶` se escribe con AltGr+R en
  una distribución española — lo cual también explica por qué tiene una variante libre en ASCII,
  `\\` (§6.1).
- **Ninguna letra y ningún dígito.** Los dígitos aparecen dentro de operadores solo como
  *argumentos* (`#.2\|x\|`, `@~ 500`) o como carga literal (`#1`, `#०९#`), nunca como el operador
  mismo.

#### 2.2 Tres marcas fuera de la clase de operadores

| Marca | Estado | Comportamiento |
|---|---|---|
| `"` | reservada por precedencia del escáner | abre un literal de cadena cuando *inicia* un token |
| `'` | reservada por precedencia del escáner | abre un literal de carácter cuando *inicia* un token |
| `°` (U+00B0) | **diacrítico sobre un identificador** | marca de definición activa (hot-definition); ver §4.6 |

`°` es la marca más inusual del lenguaje, y la razón es posicional: no es un token de operador
en absoluto. No está en el conjunto del §2.1, no se lexa como token y no puede aparecer sola. Se
adhiere a un identificador — `x°` o `°x` — y modifica dónde queda anclada la vinculación de ese
identificador. Morfológicamente esto es un **diacrítico**, no un operador, y el lenguaje tiene
exactamente uno.

#### 2.3 La clase abierta, y una consecuencia que vale la pena declarar

Los identificadores se definen negativamente: un identificador es cualquier secuencia de
caracteres que no sea espacio en blanco, no sea un dígito de un sistema numérico soportado, y no
esté en el conjunto del §2.1. Esto es lo que permite que `変数`, `متغير`, `naïve°` y los
identificadores con emoji funcionen sin una lista blanca por escritura.

Una definición negativa traza su frontera en algún lugar, y aquí cae en un sitio inesperado:

```zymbol
ab"c" = 5
>> ab"c" ¶        // → 5
```

`"` y `'` están reservados solo en posición **inicial**. En medio de un token son caracteres de
identificador ordinarios, porque la definición negativa no los reclama.[^quotes] Esto es
consecuencia del diseño y no un defecto en él: la alternativa es una lista blanca positiva, que
es justamente lo que la clase abierta existe para evitar.

#### 2.4 Cómo puede crecer el inventario

La clase de operadores es cerrada en el sentido de que ampliarla es un acto deliberado con un
costo documentado, no en el sentido de que nunca pueda cambiar. El §17 regla 5 da el
procedimiento. En la práctica el inventario es casi estático: el crecimiento ocurre combinando
marcas que ya existen, y una marca genuinamente nueva es tan poco frecuente que cada una merece
nombrarse individualmente (§20).

---

### 3. Aglutinación: la afirmación y sus límites

#### 3.1 La afirmación

Los operadores de Zymbol son **aglutinantes**: un operador es una secuencia de marcas, cada
marca aporta un significado, los significados se componen, y las fronteras entre marcas son
visibles en la forma escrita. `<<|?` no es un trígrafo arbitrario que resulta significar
"consultar si hay una tecla"; son tres morfemas.

Esta es una afirmación más fuerte y más útil que "los símbolos son consistentes", porque es
refutable de una forma concreta: para cualquier operador, o bien se puede segmentar y glosar
cada segmento, o no se puede. Donde no se puede, la forma está **lexicalizada**, y el §3.4 lo
declara así.

#### 3.2 La plantilla de ranuras

Los operadores segmentables siguen una plantilla. Leyendo de izquierda a derecha:

```
[VINCULADOR]   DOMINIO   [OPERACIÓN]   [MODALIDAD]   [ARGUMENTO]
```

| Ranura | Llenada por | Aporta |
|---|---|---|
| VINCULADOR | `:` | "el dominio siguiente está vinculado a un nombre / una cláusula" |
| **DOMINIO** | `$` `@` `#` `>>` `<<` `?` `!` | *en qué mundo vive la operación* — obligatorio |
| OPERACIÓN | `+ - * / ^ ~ # < > \| . ,` | *qué se hace en ese mundo* |
| MODALIDAD | `?` `!` | *cuán cierto / cuán forzoso* |
| ARGUMENTO | `[i]`, `(n)`, `\|x\|`, una etiqueta, un número | el operando o parámetro |

**La ranura de modalidad es final.** En todo el inventario, cuando `?` o `!` lleva fuerza modal
es la marca más a la derecha del operador: `$??`, `$!!`, `<<|?`, `@!`, `##!`, `>>!`, `>>?`,
`@:exterior!`. No hay ningún operador en el que un `?` o `!` modal sea seguido de otra marca de
operación. Esta es la generalización estructural más confiable del lenguaje.

#### 3.3 El principio en competencia: la colocación icónica

La plantilla no es la única regla de orden, y donde las dos entran en conflicto, **gana la
iconicidad**. Compárese:

```
<#   =  IN + META      importar        la flecha a la izquierda — el flujo entra
#>   =  META + OUT     exportar        la flecha a la derecha — el flujo sale
```

Bajo la plantilla sola esto es una inconsistencia: uno pone la marca de dirección antes del
dominio, el otro después. Bajo la colocación icónica es una regla: **una marca de dirección se
sitúa en el borde del signo que mira hacia la dirección que señala.** La misma regla explica
`<~` (retorna hacia la izquierda, flecha a la izquierda), `->` (entra al cuerpo hacia la
derecha, flecha a la derecha), `|>`, `=>`, y las mitades reflejadas de `<\ … \>`.

Declarar ambos principios, y declarar cuál gana, es más honesto que presentar una sola plantilla
y tratar `<#` como una excepción a ella.

#### 3.4 Tres grados de transparencia

Todo operador del lenguaje cae en una de tres clases. Esta clasificación es lo que hace precisa
la afirmación aglutinante en vez de promocional.

| Clase | Definición | Cantidad | Ejemplos |
|---|---|---|---|
| **Transparente** | totalmente segmentable; significado = composición de las partes | mayoría | `<<\|?` `$^-` `@:exterior!` `##!` `$??` `#.2\|x\|` |
| **Semi-transparente** | segmentable, pero el todo significa más que las partes | 6 | `!?` `:!` `:>` `\|>` `::` `$++` |
| **Opaco** | no composicional — un solo signo léxico, cualquiera sea su forma interna | 10 | `¶` `><` `#1` `#0` `0x` `0b` `0o` `0d` `###` `°` |

Glosas resueltas de la clase transparente (convenciones en el Apéndice A):

```
<<      |       ?
IN      UNIDAD  IRR
"tomar una unidad del flujo de entrada, sin comprometerse"     → consultar si hay una tecla

$       ^       -
COLL    ORDEN   REV
"imponer un orden a la colección, invertido"                   → ordenar descendente

@       :exterior  !
TEMP    ETQ        FRZ
"actuar con fuerza sobre el contexto temporal llamado exterior" → romper el bucle etiquetado

#       #       !
META    TIPO    FRZ
"cruzar al nivel de tipo, con fuerza"                           → convertir a Int, truncando

<<      ##.     (5,2)   "p"     v
IN      TIPO.F  ARG     PROMPT  DESTINO
"leer hacia adentro, restringido a Float con 5 dígitos totales / 2 decimales"
```

Formas semi-transparentes, con el excedente declarado:

| Forma | Segmentos | Excedente no predecible a partir de las partes |
|---|---|---|
| `!?` | ERR + IRR | que abre un *bloque* cuyo fallo queda capturado |
| `:!` | VIN + ERR | que vincula específicamente en `_err` |
| `:>` | VIN + OUT | que se ejecuta incondicionalmente después del bloque |
| `\|>` | GATE + OUT | que el valor de la izquierda se inyecta como *argumento* |
| `::` | VIN + VIN | que el nombre de la izquierda es un *espacio de nombres* de módulo |
| `$++` | COLL + SUMA + PL | que acepta tipos mixtos y los convierte a cadena |

---

## Parte II — Morfología

### 4. Clases de morfemas según su posición

#### 4.1 Prefijo (proclítico al operando)

`>>` `>>!` `>>?` `>>~` `>>|` `<<` `<<|` `<<|?` `<#` `#>` `<~` `?` `_?` `??` `@` `!?` `##.`
`###` `##!` `#|` `#.N` `#!N` `#,` `#^` `><` `\` `!`

#### 4.2 Sufijo (enclítico al operando)

`$#` `$!` `$!!` `#?` `++` `--` `°` `$~` (sobre una expresión de índice) `~` y `<~` (sobre un
nombre de parámetro — §9.1)

#### 4.3 Infijo

`+ - * / % ^` `== <> < > <= >=` `&& ||` `=` `:=` `+= -= *= /= %= ^=` `=>` `->` `::` `.` `..`
`|>` `:` (paso de rango, iteración, campo de tupla) `,` `;`

La mayoría de los operadores `$` son **infijos en efecto aunque escritos como sufijo-seguido-de-
operando**: `arr$+ elem`, `s$/ ','`. La colección va a la izquierda, el argumento a la derecha,
y el operador se sitúa entre ambos sin requerir espacios en blanco.

#### 4.4 Circunfijo (pares de encierro)

| Par | Encierra | Nota |
|---|---|---|
| `( )` | tupla, argumentos de llamada, agrupación, ranuras de estilo `>>~` | |
| `[ ]` | literal de lista, índice, corte, ruta de navegación, argumento posicional de `$+`/`$-` | cinco roles — ver §13.3 |
| `{ }` | bloque, lista de exportación, interpolación de cadenas | |
| `\| … \|` | el operando de un operador de formato/evaluación: `#.2\|x\|` | una *cerca*, no una puerta |
| `<\ … \>` | comando de shell | las mitades son imágenes especulares |
| `</ … />` | ruta de script | las mitades son imágenes especulares |
| `#d₀d₉#` | cambio de modo numérico | `d₀` y `d₉` representan los dígitos *cero* y *nueve* del sistema de destino; la carga es una demostración, ver §5.4 |
| `>>\| { }` | región TUI | prefijo de dominio + bloque |

#### 4.5 Discontinuo con concordancia

La construcción de bucle etiquetado es el único lugar donde dos marcas separadas deben
**concordar**:

```zymbol
@:temporizador {      // declaración    @ : ETQ
    @~ 60000
    @:temporizador!    // referencia     @ : ETQ FRZ   — la etiqueta debe coincidir
}
```

`@:temporizador` y `@:temporizador!` no son dos operadores independientes: el segundo solo está
autorizado por el primero. Esto es concordancia morfológica, y es la razón por la que el
break/continue etiquetado se escribe `@:temporizador!` y no `@!temporizador` — esto último
pondría el argumento después de la ranura de modalidad, violando el §3.2.

La concordancia no es decoración: una referencia cuya etiqueta no coincide con ningún bucle
envolvente se rechaza antes de que el programa se ejecute (§16.1). Un requisito morfológico que
nada hace cumplir no es un requisito, es solo un hábito.

#### 4.6 Diacrítico — la marca `°`

`°` se adhiere a un identificador y selecciona el ámbito al que se ancla su vinculación. La
posición sobre el anfitrión es el único rasgo que las distingue:

| Forma | Se ancla a | Vida |
|---|---|---|
| `x°` (sufijo) | el ámbito `@` envolvente más cercano | muere con el bucle |
| `°x` (prefijo) | el ámbito **por encima** del `@` más cercano | sobrevive al bucle |

Ambas se autoinicializan al valor neutro de la operación en su primer uso (`0`, `1`, `[]`, `""`
según el contexto — `GUIDE.md` §4). Fuera de cualquier bucle, las dos formas coinciden.

Dos hechos hacen que `°` sea categóricamente distinta de cualquier otra marca:

1. **No pertenece a la clase de operadores** (§2.1). Sobrevive dentro de identificadores por la
   regla de la clase abierta, y el lexer la elimina después.
2. Su significado es **puramente posicional**. Ninguna otra marca de Zymbol cambia de
   significado al pasar de un lado de su anfitrión al otro. Escribir ambas a la vez (`°x°`) es
   un error diagnosticado.

---

### 5. Procesos productivos

#### 5.1 Reduplicación

Duplicar una marca es una derivación productiva en exactamente **dos** dominios, `$` y `?`,
donde significa *exhaustivo / plural / completivo*:

| Simple | Reduplicada | Resultado simple | Resultado reduplicado | Relación |
|---|---|---|---|---|
| `?` si | `??` coincidencia | una rama evaluada | n ramas evaluadas | plural |
| `$?` contiene | `$??` todos los índices | `#1`/`#0` | `[2, 3, 5]` | plural |
| `$-` quita el primero | `$--` quita todos | `[1,2,3,2]` | `[1, 3]` | completivo |
| `$~` actualiza un sitio | `$~~` reemplaza | un índice | `"banana"` → `"bAnAnA"` | completivo |
| `$+` agrega uno | `$++` acumula | un elemento | construcción iterada | iterativo |
| `$!` prueba error | `$!!` propaga | Bool | expulsa hacia arriba | intensivo |

`$!!` es el único miembro cuyo excedente es fuerza y no pluralidad; se incluye aquí porque se
deriva de `$!` por el mismo proceso visible, y se señala para que la glosa siga siendo honesta.

**Duplicación que no es derivación.** Estas formas contienen un carácter duplicado pero son
signos léxicos únicos. En particular `&` solo *no es un token* — `x = 1 & 2` es un error léxico
— así que `&&` no puede ser derivación de nada.

| Forma | Por qué es léxica |
|---|---|
| `&&` | `&` no existe como forma simple |
| `==` `++` `--` `+=` … | convenciones aritméticas heredadas, no derivaciones de Zymbol |
| `\\` | variante libre de `¶`; no relacionada con `\` (fin de vida) — ver §13.1 |
| `::` | no "vinculado dos veces"; un recorrido de espacio de nombres |
| `>>` `<<` | intensificación *con cambio de categoría*: relación → canal |
| `..` | extensión de `.`, pero produce un rango, no un acceso más profundo |

`##` es el caso límite y no pertenece a ninguna de las dos tablas: *sí* es composicional — `#`
meta, duplicado, da el nivel de tipo — pero encabeza un paradigma propio en vez de derivar
operadores individuales a partir de los de `#`, así que tratarlo como reduplicación sería
sobrestimar el caso.

#### 5.2 Sufijación modal

`?` y `!` en posición final convierten una operación definida en una incierta o en una forzada.
Es el afijo más regular del lenguaje:

| Base | `+ ?` | `+ !` |
|---|---|---|
| `<<\|` leer una tecla | `<<\|?` consultar si hay una tecla | — |
| `>>` escribir | `>>?` preguntar el tamaño de la terminal | `>>!` forzar la limpieza de pantalla |
| `$` sobre un valor | `$?` ¿lo contiene? | `$!` ¿es un error? |
| `@:etiqueta` un bucle | — | `@:etiqueta!` terminarlo |
| `##` un cruce de tipo | — | `##!` truncar en vez de redondear |

#### 5.3 Composición entre dominios

Un núcleo de dominio puede tomar el operador de otro dominio como su argumento, sin que ninguno
de los dos cambie de significado. `<< ##.(5,2) "p" v` es el dominio de entrada alojando una
restricción del dominio de tipo; `##.` significa exactamente lo mismo que en cualquier otro
lugar.

Este es el mecanismo por el cual el lenguaje crece sin acuñar nada: una composición que nunca se
ha escrito ya tiene un significado, resuelto de antemano por las marcas de las que está hecha.
Añadirla es cuestión de implementar lo que la notación ya decía.

#### 5.4 Elisión y demostración

Dos procesos marginales pero reales:

**Ranuras elididas.** `>>~` toma una tupla de cinco ranuras en la que cualquier ranura puede
quedar vacía, marcada solo por su coma: `>>~ (,,, 196) > "rojo"` fija el color de frente y deja
la posición y el estilo intactos. La coma es un **morfema cero** — mantiene abierta una posición
sin llenarla.

**Carga demostrativa.** `#d₀d₉#` nombra un sistema numérico no nombrándolo sino
**exhibiéndolo**: `#०९#` dice "devanagari" al contener los devanagari `०` y `९`; `#09#` reinicia
al contener ASCII. No hay nombre que traducir ni tabla que consultar, que es el mecanismo
funcionando exactamente como se pretendía. Es el signo icónico más claro del lenguaje y el que
mejor demuestra el principio fundacional.

---

### 6. Alomorfía y variación libre

Tres lugares donde un significado tiene más de una forma.

#### 6.1 `¶` ~ `\\` — el morfema de salto de línea

Ambos emiten un salto de línea en el flujo de salida; son intercambiables en todas partes. `¶`
es la forma canónica y la que preserva el formateador. `\\` existe porque `¶` no es alcanzable
en todas las distribuciones de teclado. Esto es variación libre en el sentido estricto: ningún
contexto las distingue.

#### 6.2 Los sistemas numéricos

`#1` y `#0` aceptan los dígitos *de cualquiera de los 69 sistemas soportados* — `#१`, `#١`,
`#𝟏` son el mismo token que `#1`. Los literales enteros aceptan igualmente cualquier sistema
único de forma consistente (`४२` es `42`). Un morfema, sesenta y nueve realizaciones gráficas,
seleccionadas por el sistema del autor y no por el contexto gramatical.

#### 6.3 `@etiqueta` ~ `@:etiqueta` — la forma fusionada

Una etiqueta de bucle puede escribirse fusionada (`@etiqueta`) o con el vinculador visible
(`@:etiqueta`). Se lexan como tokens distintos (`AtLabel`, `AtColonLabel`) y significan lo
mismo.

La forma con dos puntos es canónica, y la razón es morfológica y no estética: pone la ranura
VINCULADOR sobre la página, de modo que la construcción sigue siendo segmentable bajo el §3.2.
La forma fusionada esconde una frontera de morfema, que es justamente lo que una notación
aglutinante no puede permitirse a menudo.

Nótese el riesgo de espaciado documentado en `GUIDE.md` §1b — `@ etiqueta` con un espacio no es
una etiqueta en absoluto, sino un bucle cuya primera expresión es `etiqueta`.

---

## Parte III — El léxico de morfemas

### 7. Cómo leer una entrada

Cada núcleo de dominio a continuación se presenta como:

- **Glosa** — la abreviatura usada en las glosas interlineales (Apéndice A)
- **Contrato** — el invariante que respeta todo miembro del paradigma. Un operador propuesto que
  rompiera el contrato se rechaza bajo el §17 regla 2, sin importar cuán conveniente sea.
- **Paradigma** — el conjunto completo de formas
- **Restricciones** — dónde pueden y no pueden aparecer los miembros
- **Excepciones** — miembros que no respetan el contrato, nombrados en vez de omitidos

---

### 8. Núcleos de dominio

#### 8.1 `$` — COLL, el dominio de colecciones

**Contrato.** `$X` toma una colección a su izquierda, devuelve un valor **nuevo**, y nunca muta
al receptor. La marca después de `$` nombra la operación usando las mismas marcas base que el
resto del lenguaje.

| Forma | Operación | Composición |
|---|---|---|
| `$#` | longitud | COLL + META → el metaconteo |
| `$+` / `$+[i]` | agregar / insertar en índice | COLL + SUMA (+ posición) |
| `$-` / `$--` | quitar el primero / quitar todos | COLL + RESTA (+ PL) |
| `$-[i]` / `$-[i..j]` / `$-[i:n]` | quitar en índice / rango / cantidad | COLL + RESTA + posición |
| `$?` / `$??` | contiene / todos los índices | COLL + IRR (+ PL) |
| `$[i..j]` / `$[i:n]` | corte inclusivo / por cantidad | COLL + tramo |
| `$^+` / `$^-` / `$^` | ordenar asc / desc / por comparador | COLL + ORDEN (+ dirección) |
| `$>` | mapear | COLL + OUT — cada elemento transformado hacia afuera |
| `$\|` | filtrar | COLL + GATE — solo pasan los elementos que califican |
| `$<` | reducir | COLL + IN — colapsa hacia adentro a un valor |
| `$~~[p:r]` | reemplazar todo | COLL + MOD + PL |
| `$/` | dividir (por carácter o subcadena) | COLL + DIV |
| `$*` | repetir (cadenas) | COLL + MUL |
| `$++` | concatenar-construir | COLL + SUMA + PL |
| `$!` / `$!!` | es error / propagar | COLL·valor + ERR (+ FRZ) |
| `arr[i]$~` | actualización funcional | posición + COLL + MOD |

**Excepciones.** `$!` y `$!!` no toman una colección — toman cualquier valor, incluido un
escalar de error. Están en el paradigma `$` solo bajo una lectura más débil de `$` como "operar
sobre el valor que tienes", que ningún otro miembro necesita. Un par irregular en un paradigma
de dieciséis es un precio pequeño, pero es un precio, y la manera honesta de asumirlo es
nombrarlo en vez de ensanchar el contrato hasta que lo cubra todo y no restrinja nada.

#### 8.2 `@` — TEMP, el dominio temporal

**Contrato.** Toda forma `@` opera *dentro de* un contexto temporal. `@X` siempre significa
"actuar sobre el contexto temporal actual de la manera X".

| Forma | Operación |
|---|---|
| `@ { }` | bucle infinito |
| `@ N { }` | repetir N veces |
| `@ cond { }` | mientras |
| `@ x:arr { }` | para cada uno |
| `@:etiqueta { }` | bucle etiquetado |
| `@!` / `@:etiqueta!` | romper / romper etiquetado |
| `@>` / `@:etiqueta>` | continuar / continuar etiquetado |
| `@~ N` | dormir N milisegundos |

**Restricciones.** `@!` y `@>`, etiquetados o no, son **errores semánticos fuera de un bucle**,
y una forma etiquetada es un error a menos que un bucle *envolvente* lleve esa etiqueta. `@~`
no lo es: pausa sin tocar el control de flujo. La línea entre ambos, y por qué cae ahí, está en
el §16.1.

#### 8.3 `#` — META, el dominio meta, y `##` — TYPE

**Contrato.** `#` señala un cruce de frontera: del espacio de valores al espacio de tipos, del
valor en tiempo de ejecución a la representación en pantalla, o del archivo al módulo nombrado.
Duplicar a `##` mueve del nivel meta al nivel de tipo propiamente dicho.

| Forma | Operación | Nivel |
|---|---|---|
| `# nombre` | declaración de módulo | archivo → módulo nombrado |
| `#>` / `<#` | exportar / importar | superficie del módulo |
| `#1` / `#0` | literales Bool | verdad tipada, no enteros |
| `#\|x\|` | evaluación numérica de una cadena | valor → número |
| `x#?` | metadatos de tipo → `(símbolo, conteo, presentación)` | valor → meta |
| `#.N\|x\|` / `#!N\|x\|` | redondear / truncar N decimales | valor → presentación |
| `#,\|x\|` / `#^\|x\|` | formato de coma / científico | valor → presentación |
| `#d₀d₉#` | cambio de modo numérico | script de presentación |
| `##.` / `###` / `##!` | convertir a Float / redondear a Int / truncar a Int | cruce de tipo |
| `##"` / `##'` | marcadores String / Char | tipo, solo en typespec de entrada |

**El paradigma de símbolos de tipo es icónico.** Los valores que devuelve `#?` son miniaturas
de la notación propia de cada tipo, razón por la cual no necesitan tabla alguna para
aprenderse:

| Símbolo | Tipo | Representa |
|---|---|---|
| `##"` | String | la comilla que delimita una |
| `##'` | Char | la comilla que delimita uno |
| `##]` | Array | el corchete que cierra uno |
| `##)` | Tuple / NamedTuple | el paréntesis que cierra una |
| `##()` | Function | la sintaxis de llamada |
| `##->` | Lambda | la sintaxis de definición |
| `##.` | Float | el punto decimal |
| `##?` | Bool | la pregunta que un Bool responde |
| `##_` | Unit | la marca no vinculante |
| `###` | Int | — **no icónico**; Int no tiene notación que representar |
| `##<Ident>` | tipo de error | — **no icónico**; ver §15 |

Dos de once no son icónicos, y ambos están nombrados. `###` es arbitrario y debe memorizarse;
`##Index` es una palabra.

#### 8.4 `>>` — OUT, el flujo de salida

**Contrato.** `>>` y sus derivados actúan sobre la terminal como superficie de salida. La marca
después de `>>` selecciona *qué aspecto* de la superficie se ve afectado.

| Forma | Operación | Composición |
|---|---|---|
| `>>` | imprimir (yuxtaposición, sin salto de línea implícito) | OUT |
| `>>!` | limpiar la pantalla | OUT + FRZ — forzar la superficie a un estado conocido |
| `>>?` | consultar el tamaño de la terminal (`[H, W] = >>?`) | OUT + IRR — preguntarle algo a la superficie |
| `>>~ (…) > elementos` | salida posicionada / con estilo | OUT + MOD — modificar posición y estilo |
| `>>\| { }` | bloque TUI: pantalla alterna + modo raw | OUT + GATE — una región controlada |

El `>` interno de `>>~ (5,10) > "texto"` es de nuevo la marca de dirección, introduciendo la
carga después de la tupla de estilo.

#### 8.5 `<<` — IN, el flujo de entrada

**Contrato.** `<<` y sus derivados hacen entrar datos al programa. La marca después de `<<`
selecciona el *medio y la granularidad*.

| Forma | Lee | Bloqueante |
|---|---|---|
| `<<` | una línea | sí |
| `<< <typespec> "prompt" var` | un valor validado; vuelve a preguntar hasta que sea válido | sí |
| `<<\|` | una tecla | sí |
| `<<\|?` | una tecla si hay una pendiente, si no `'\0'` | **no** |

Los typespecs son la familia de conversión `##` colocada antes del prompt, con un tamaño
opcional:

| Forma | Lee → | Restricción |
|---|---|---|
| `<< ##.(T,D) "p" v` | `Float` | ≤T dígitos totales, ≤D después del punto, sin exponente |
| `<< ##. "p" v` | `Float` | cualquier número válido |
| `<< ###(N) "p" v` | `Int` | ≤N dígitos |
| `<< ##"(N) "p" v` | `String` | ≤N caracteres |
| `<< ##' "p" v` | `Char` | exactamente un carácter |

El tamaño entre paréntesis es la única concesión del lenguaje a un argumento nombrado en un
flujo de entrada. Ambos motores validan de forma idéntica; un signo inicial no cuenta para el
presupuesto de dígitos.

**`<<|` frente a `<<|?` — el par mínimo modal.** Esto es el §5.2 aplicado:

| Forma | Se lee como | Devuelve |
|---|---|---|
| `<<\|` | "dame una tecla" — realis | `Char`, tras bloquear |
| `<<\|?` | "¿hay una tecla?" — irrealis | `Char`, o `'\0'` de inmediato |

La forma irrealis igual devuelve un `Char`; lo que no puede es prometer uno *significativo*, así
que responde con el carácter nulo.[^sentinel] El irrealis estrecha la garantía, no el tipo — que
es lo que mantiene a `?` como sufijo y no como un operador distinto.

#### 8.6 `><` — la frontera del proceso

`><` captura la línea de comandos en un arreglo de cadenas. Es icónico (flechas que convergen =
entrada en la frontera) pero **lexicalizado**: nada en la forma convergente predice
específicamente *los argumentos de la línea de comandos*, en oposición a cualquier otra
entrada. Un paradigma de uno — el único núcleo de dominio que no encabeza nada.

#### 8.7 `?` — IRR, el dominio irrealis

**Contrato.** Dondequiera que `?` encabece una construcción, el resultado es condicional: el
resultado depende de una pregunta que puede resultar falsa, vacía o inexistente.

| Forma | Operación |
|---|---|
| `? cond { }` | si |
| `_? cond { }` | sino-si |
| `?? x { pat => val }` | coincidencia |
| `$?` / `$??` | contiene / todos los índices |
| `x#?` | consulta de metadatos de tipo |
| `!?` | intentar — el bloque puede o no lanzar |
| `<<\|?` | consultar si hay una tecla |
| `>>?` | consultar el tamaño de la terminal |

**Excepción.** `##?` es el *símbolo de tipo* Bool, no una consulta. Es el tipo de la respuesta,
no una pregunta — un homógrafo dentro del paradigma `#` y no un miembro de este. Ver §13.4.

#### 8.8 `!` — el núcleo de fuerza / error

`!` es la marca más polisémica del lenguaje. En vez de fingir que una sola glosa la cubre, el
§12.1 da las tres lecturas y la regla que elige entre ellas. Como núcleo de dominio aparece en
`!?` (intentar) y habilita `:!` (capturar) y `##<Ident>` (tipos de error).

---

### 9. Marcas de operación y modalidad

#### 9.1 `~` — MOD, modificación

**Contrato.** `~` marca que algo se *modifica* o *se envía de vuelta modificado* — nunca que
algo se crea.

| Forma | Qué se modifica |
|---|---|
| `param~` | el parámetro — una **copia de trabajo** que el cuerpo puede reasignar; el argumento del llamador queda intacto |
| `param<~` | el parámetro — **por referencia**; el cambio llega al llamador |
| `<~ valor` | el valor, enviado de vuelta al llamador |
| `$~~[p:r]` | la cadena, por reemplazo |
| `arr[i]$~` | la colección, en un índice (devuelve una copia nueva) |
| `@~` | el flujo temporal, pausado |
| `>>~` | la posición y el estilo de salida |

**El par de parámetros es donde la morfología se gana su lugar.** `a~` y `b<~` difieren en una
marca, y esa marca es la que significa "fluye de vuelta":

```zymbol
f(a~)  { a = a + 1  <~ a }     // copia mutable
g(b<~) { b = b + 1 }           // por referencia

x = 5
r = f(x)
>> "r=" r " x=" x ¶            // → r=6 x=5   — la x del llamador queda intacta

y = 5
g(y)
>> "y=" y ¶                    // → y=6       — el cambio viajó de vuelta
```

`~` dice que el parámetro puede modificarse. `<~` dice que la modificación viaja de vuelta. Nada
de esto hay que memorizarlo por separado: es el §9.6 y el §3.3 aplicados a una ranura de
parámetro.

**Para qué sirve `~`.** Evita que el cuerpo tenga que abrir con una copia que de otro modo
habría que escribir a mano. Sin ella, una función que necesita trabajar sobre su argumento tiene
que decirlo explícitamente:

```zymbol
// la copia, escrita a mano
f(a) {
    local = a
    local = local + 1
    <~ local
}

// lo mismo, declarado
g(a~) {
    a = a + 1
    <~ a
}
```

Las dos son equivalentes, y la segunda es el punto de la marca: una copia de trabajo se declara
en la firma en vez de ensamblarse en la primera línea del cuerpo. Lo que nunca debe hacer es
llegar al punto de llamada — ese es el trabajo de `<~`, y mantener los dos trabajos en dos
marcas distintas es la razón por la que ninguna de las dos necesita un calificador.

#### 9.2 `|` — GATE, y el homógrafo de cerca

**Contrato (puerta).** `|` controla el paso: `$|` filtra, `||` admite cualquiera de las dos
alternativas, `|>` pasa un valor a través de una función, `<<|` estrecha la entrada de líneas a
un solo carácter, `>>|` abre una región de pantalla controlada.

**Homógrafo (cerca).** En `#.2|x|`, `#,|x|`, `#^|x|`, `#|x|`, las dos marcas `|` son un
**delimitador circunfijo**, no una puerta. Encierran un operando. Nada pasa a través de ellas.
Es un homógrafo genuino, resuelto por posición: una cerca siempre viene en par inmediatamente
después de un operador de formato encabezado por `#`; una puerta nunca lo hace.

#### 9.3 `_` — NBND, no vinculante

**Contrato.** `_` marca una posición que existe sintácticamente pero no vincula ningún nombre.
Nunca introduce un nombre en el ámbito.

| Forma | Posición |
|---|---|
| `_ { }` | rama sino |
| `_?` | sino-si |
| `_` en `?? x { _ => … }` | comodín de coincidencia |
| `[a, _, c] = arr` | desestructuración, elemento omitido |
| `x \|> f(_, 2)` | marcador de posición de pipe |
| `_nombre` | prefijo de identificador declarado-pero-no-usado |
| `##_` | el símbolo del tipo Unit |
| `:! ##_ { }` | capturar cualquier tipo de error |

Es la marca más regular del lenguaje: ocho usos, un significado, sin excepciones.

#### 9.4 `:` — BND, vinculación y nombrado

**Contrato.** `:` introduce o referencia un *nombre*, o nombra un componente de un argumento
compuesto.

| Forma | Nombra |
|---|---|
| `:=` | una vinculación inmutable |
| `::` | alcanzar a través de una vinculación de módulo |
| `@:etiqueta` | un bucle |
| `@:etiqueta!` / `@:etiqueta>` | el bucle al que se apunta |
| `:!` | la cláusula de error |
| `:>` | la cláusula de limpieza |
| `nombre: valor` | un campo de tupla nombrada |
| `@ i:arr` | la variable de iteración |
| `1..10:2` | el paso |
| `$[i:n]` | la cantidad en un corte |
| `$~~[p:r]` | patrón frente a reemplazo |

Las últimas tres son más débiles: ahí `:` separa dos componentes de un argumento en vez de
introducir un nombre. El §13.5 lo registra como un homógrafo gradual en vez de pretender que el
contrato lo cubre.

#### 9.5 `=>` — MAP, "se convierte en"

**Contrato.** El lado izquierdo se conoce internamente bajo un nombre o forma; el lado derecho
es cómo se expresa, se compara o se exporta. `=` lleva la relación de correspondencia, `>` la
dirección hacia afuera, hacia quien consume.

| Forma | Operación |
|---|---|
| `?? x { pat => val }` | el patrón se convierte en el resultado |
| `<# ruta => alias` | el módulo se conoce como alias |
| `#> { fn => pub }` | el nombre interno se convierte en el nombre público |

Esto completa el paradigma de flechas: `->` (hacia adentro del cuerpo), `<~` (de vuelta al
llamador), `=>` (a través, hacia quien consume).

#### 9.6 `->` y `<~` — la frontera de la función

`->` apunta *hacia adentro* del cuerpo de una función; `<~` apunta *de vuelta hacia afuera*, al
llamador. Juntas son las marcas de entrada y salida de la frontera de la función, y sus formas
son icónicas de exactamente eso.

`<~` ocupa dos posiciones, y la lectura es la misma en ambas — solo cambia qué es lo que viaja de
vuelta:

| Posición | Forma | Qué viaja de vuelta |
|---|---|---|
| prefijo, en el cuerpo | `<~ valor` | el valor de retorno |
| sufijo, sobre un parámetro | `f(p<~)` | la modificación hecha a `p` |

**La lista de parámetros puede estar vacía.** `() -> cuerpo` es un thunk. No es una marca nueva
ni una nueva lectura de `->`: la flecha sigue apuntando hacia adentro de un cuerpo, y lo que la
precede sigue siendo una lista de parámetros — una que resulta no tener miembros. `()` no choca
con nada, porque Zymbol no tiene tupla vacía y los paréntesis de una llamada siempre siguen a
algo invocable.

#### 9.7 `.` — entrar en

**Contrato.** `.` significa "entrar en": en un miembro de una estructura, en la parte
fraccionaria de un número, o (duplicado) a través de un tramo.

| Forma | Operación |
|---|---|
| `tupla.campo` | entrar en un campo |
| `modulo.CONST` | entrar en una constante de módulo |
| `3.14` | entrar en la parte fraccionaria |
| `1..5` | recorrer un rango |

**Nota.** La navegación en profundidad dentro de colecciones anidadas *no* usa `.` — usa `>`:
`m[1>2]`, `cubo[1>2>1]`. Ahí la marca es de nuevo la marca de dirección, que se lee "hacia
adelante, al siguiente nivel". Es deliberado (`.` es acceso binario a un miembro; `>` encadena)
pero significa que "entrar en" tiene dos exponentes según si el paso es hacia un miembro
*nombrado* o hacia un nivel *indexado*.

---

### 10. Marcas estructurales

| Marca | Rol | Notas |
|---|---|---|
| `=` | asignación | |
| `:=` | declaración de constante | |
| `+= -= *= /= %= ^=` | asignación compuesta | convención heredada (§5.1) |
| `++` `--` | incremento / decremento | convención heredada |
| `== <> < > <= >=` | comparación | `<>` icónico: divergente = distinto |
| `&& \|\|` | Y / O lógicos | `&` solo no es un token |
| `!` | NO lógico | solo en posición prefija |
| `+ - * / % ^` | aritmética | `+` es solo numérico — nunca concatena cadenas |
| `,` | separador; morfema cero en las ranuras de `>>~` | |
| `;` | separador de sentencias; separador de ruta en `arr[p ; q]` | |
| `\ var` | fin de vida explícito | la única destrucción *observable* |
| `//`, `/* */` | comentarios; se admite anidar | resuelto de `/` por coincidencia máxima |
| `{nombre}` | interpolación de cadenas | solo identificador, cualquier sistema |

---

### 11. Delimitadores y marcas literales

| Marca | Rol |
|---|---|
| `" … "` | literal de cadena; escapes `\n \t \r \" \\ \{ \}`; sin `\uXXXX` |
| `' … '` | literal de carácter — un carácter Unicode |
| `0x` `0b` `0o` `0d` | prefijos de base para códigos de carácter: `0x41` → `'A'` |
| `#1` / `#0` | literales Bool, en cualquiera de 69 sistemas numéricos |
| `¶` / `\\` | salto de línea en el flujo de salida (§6.1) |
| `[ … ]` | literal de arreglo |
| `( … )` | tupla, tupla nombrada, agrupación, argumentos |

Los prefijos de base son opacos *y además* son abreviaturas del inglés (he**x**adecimal,
**b**inario, **o**ctal, **d**ecimal). El §15 los registra.

---

## Parte IV — Dónde el sistema no es regular

Esta parte existe porque un sistema simbólico cuyas excepciones no están documentadas no es un
sistema — es un conjunto de hábitos. Todo lo que sigue es un lugar donde "una marca, un
significado" no se cumple, declarado junto con la regla que resuelve la ambigüedad.

### 12. Alosemia: una marca, lectura determinada por el anfitrión

La *alosemia* — el mismo morfema leído de forma distinta según a qué se adhiere — es distinta de
la homografía, donde dos signos sin relación comparten una forma. Las marcas siguientes son
alosémicas: las lecturas están relacionadas, y el **dominio anfitrión elige** entre ellas.

#### 12.1 `!` — tres lecturas

| Lectura | Elegida por | Ejemplos |
|---|---|---|
| negación lógica | posición prefija en una expresión | `!bandera` |
| fuerza / terminar | al final de la palabra, tras un núcleo de dominio | `@!` `>>!` `##!` `#!N` |
| dominio de error | adyacencia a `?`, `:`, o `$` | `!?` `:!` `$!` `$!!` |

Las tres están relacionadas (todas son decisivas en vez de tentativas) pero no son
intercambiables, y quien selecciona es el anfitrión, no el juicio del lector. Nótese que fuerza
y error se distinguen puramente por dominio: en `##!` el `!` trunca, en `$!` prueba si hay un
error, y lo único que los distingue es `#` frente a `$`.

#### 12.2 `>` — cinco lecturas, una dirección

| Lectura | Ejemplos |
|---|---|
| comparación — icónico, el extremo ancho mira hacia el valor mayor | `a > b` |
| flujo hacia afuera | `>>` `#>` `$>` `\|>` `:>` |
| hacia adelante en el tiempo | `@>` `@:etiqueta>` |
| hacia quien consume | `->` `=>` |
| paso de profundidad en una ruta de navegación | `m[1>2>3]` |

Las cinco son "hacia / adelante". `>` es polisémico pero no homográfico: ninguna lectura de `>`
contradice a otra.

#### 12.3 `~` — modificación frente a canal

En `$~~`, `arr[i]$~`, `@~` y `>>~`, `~` modifica algo. En `<~` y `param~` está más cerca de
*canal* — la ruta por la que un valor viaja de vuelta. Están relacionadas, pero la segunda
lectura trata de transporte y no de cambio, y el contrato del §9.1 se estira para cubrirla.

#### 12.4 `#` frente a `##`

`#` es nivel meta, `##` es nivel de tipo. Donde una forma tiene tres, la segmentación es `##` +
`#`, no `#` + `##` — pero `###` es el caso donde segmentar no aporta nada: el tercer `#` no
significa *Int* bajo ninguna lectura, es simplemente la marca que quedó. Por eso `###` figura
entre los signos opacos (§14) aunque su forma sea descomponible. Segmentabilidad y
composicionalidad son propiedades distintas, y `###` es el lugar más claro del lenguaje donde se
separan. El trígrafo es inequívoco en la práctica solo porque no existe ningún otro trígrafo que
empiece con `#`.

---

### 13. Homógrafos declarados

A diferencia del §12, estos son significados genuinamente sin relación que comparten una forma.

#### 13.1 `\` y `\\`

| Forma | Significado |
|---|---|
| `\ var` | destruir la variable ahora |
| `\\` | emitir un salto de línea |

No tienen nada en común. `\\` es una variante libre de `¶` (§6.1) y `\` es un operador de fin de
vida. Se distinguen por lo que sigue: un identificador frente a una segunda barra invertida. Es
el homógrafo más agudo del lenguaje y no hay defensa de principio para él — es el costo de haber
elegido una alternativa alcanzable desde el teclado para `¶`.

#### 13.2 `.` — tres significados

Acceso a miembro (`t.f`), el punto decimal (`3.14`), y — duplicado — un rango (`1..5`).
Desambiguado por lo que lo rodea: dígitos a ambos lados lo hace decimal, un segundo punto lo
hace un rango, en cualquier otro caso es acceso.

#### 13.3 `[ ]` — cinco roles

Literal de arreglo, índice, corte (con `$`), ruta de navegación, y argumento posicional de `$+`
/ `$-`. Desambiguado por lo que precede al corchete: nada (literal), una expresión (índice), `$`
(corte), `$+` / `$-` (posición). Dentro de una ruta de navegación, el contenido sigue su propia
gramática (`>`, `;`, `..`, anidamiento).

#### 13.4 `?` en `##?`

`##?` es el símbolo del tipo Bool. Cualquier otro `?` del lenguaje es irrealis (§8.7). La
justificación — "un Bool es lo que responde una pregunta" — es post-hoc; la razón real es que el
paradigma de símbolos de tipo es icónico y `?` era la marca disponible. Se registra como
homógrafo.

#### 13.5 `:` en posiciones que dividen un argumento

En `1..10:2`, `$[i:n]` y `$~~[p:r]`, `:` separa dos componentes de un mismo argumento en vez de
introducir un nombre. Homógrafo gradual: la lectura "nombra un componente" es forzada, y este
documento prefiere decirlo así en vez de ensanchar el contrato hasta que se cumpla vacíamente.

#### 13.6 `^`, `*`, `/`

| Marca | Roles |
|---|---|
| `^` | exponenciación · orden de clasificación (`$^`) · notación científica (`#^`) |
| `*` | multiplicación · repetición de cadenas (`$*`) · patrón de resto (`[a, *resto]`) |
| `/` | división · dividir (`$/`) · comentario (`//`, `/* */`) · ruta de script (`</ … />`) |

Cada una se resuelve por el núcleo de dominio que la precede, salvo `//`, que se resuelve por
coincidencia máxima frente a `/`.

---

### 14. Signos opacos

Diez formas **no son composicionales** y deben aprenderse como un todo. Algunas pueden cortarse
en piezas; ninguna puede leerse a partir de esas piezas.

| Signo | Significado | Por qué no puede derivarse |
|---|---|---|
| `¶` | salto de línea | un logograma; el pilcrow *es* la marca de párrafo |
| `><` | argumentos de línea de comandos | icónico de entrada, pero nada predice "línea de comandos" |
| `#1` / `#0` | verdadero / falso | `#` + dígito es una convención, no una composición |
| `0x` `0b` `0o` `0d` | prefijos de base | abreviaturas del inglés (§15) |
| `###` | conversión a Int / símbolo de tipo Int | segmentable como `##` + `#`, pero la tercera marca no aporta significado (§12.4) |
| `°` | definición activa | diacrítico; el significado es posicional, no composicional (§4.6) |

Listarlos es el punto. Diez formas opacas frente a las 97 formas de operador catalogadas en
`REFERENCE.md` §21 es una proporción defendible — y una proporción solo es defendible una vez
que alguien la contó. Un sistema simbólico que nunca cuenta sus signos opacos siempre creerá
que tiene pocos.

---

### 15. Residuo de lenguaje natural

La regla de diseño 4 (§17) prohíbe palabras de lenguaje natural en la gramática. La regla se
cumple. Lo que sigue es todo lo que en el lenguaje *es* una palabra, para que el alcance de la
regla quede inequívoco.

| Residuo | Forma | Evaluación |
|---|---|---|
| **Tipos de error** | `##IO` `##Network` `##Parse` `##Index` `##Type` `##Div` `##_` | Seis palabras en inglés más `##_`, el único miembro simbólico. `##` es gramática; el nombre después de eso es una ranura de identificador abierta — el analizador acepta *cualquier* identificador, incluido `##Índice`, que simplemente nunca coincide en tiempo de ejecución. |
| **Biblioteca estándar** | `std/math` `std/random` `std/json` `std/io` `std/net` `std/term` `std/db`, y cada función en ellos | Rutas de módulo y nombres de función. Identificadores, tratados igual que los de los módulos de usuario. |
| **Prefijos de base** | `0x` `0b` `0o` `0d` | Abreviaturas de hexadecimal / binario / octal / decimal. El elemento más evitable de esta lista, y el más arraigado. |
| **Identificador convencional** | `_err` | No reservado; una convención que la cláusula catch rellena. |

**Por qué esta es la frontera correcta.** Simbolizar el residuo significaría acuñar una marca
por cada tipo de error y por cada función de biblioteca — un inventario que crece sin límite, en
un sistema cuyo valor viene de que su inventario sea pequeño y cerrado. La rúbrica símbolo-frente-
a-módulo ya traza esta línea para las capacidades: *una operación nombrada sobre un recurso
direccionado* (una ruta, una URL, una conexión) es una llamada de módulo; los símbolos se
reservan para flujos de proceso ambientales (`>>`, `<<`, `><`, `<\ \>`). Los tipos de error son
recursos nombrados por el mismo criterio.

**Qué sería una violación real.** Una construcción de control de flujo, un tipo, un operador o
una declaración escritos con letras. No hay ninguno, en ninguna versión.

---

### 16. Restricciones de contexto y herencia de restricciones

Algunas marcas son legales solo en contextos específicos. Donde la restricción se deriva del
dominio y no del operador individual, es **heredada** — lo cual es lo que la hace predecible
para operadores que todavía no existen.

#### 16.1 Heredada: la regla de contexto de bucle de `@`

La regla no es "estos operadores resultan necesitar un bucle". Es: **una sentencia con prefijo
`@` que actúa sobre el control de flujo del bucle es inválida fuera de uno.** La cláusula sobre
el control de flujo es la que hace el trabajo, y es la que decide qué miembros heredan la
restricción:

| Sentencia | Qué hace | ¿Actúa sobre el control de flujo? | ¿Necesita un bucle? |
|---|---|---|---|
| `@!` | rompe | sí — abandona el bucle | **sí** |
| `@>` | continúa | sí — salta a la siguiente iteración | **sí** |
| `@:E!` / `@:E>` | rompe/continúa un bucle nombrado | sí, y el nombre debe resolverse | **sí**, etiquetado `E` |
| `@~ N` | pausa N ms | **no** — la ejecución se reanuda donde estaba | no |

```zymbol
@:temporizador {
    @:temporizador!    // break etiquetado — necesita un bucle envolvente llamado 'temporizador'
}

@~ 500                 // legal en el nivel superior: una pausa no es un salto
```

`@~` es el miembro que muestra que la regla trata del control de flujo y no del prefijo `@`. Es
temporal, se escribe con `@`, y no hereda nada — porque una pausa se reanuda donde quedó, y una
construcción que no mueve el control de flujo del bucle no tiene motivo para necesitar uno.[^atsleep]

**El cuerpo de una función o lambda es una frontera.** Los bucles del llamador no están en el
ámbito dentro de un callee, así que esto es un error aunque cada punto de llamada esté dentro de
un bucle:

```zymbol
f() { @! }              // error: '@!' fuera de un bucle
@ i:1..3 { f() }
```

Esto se deriva de los marcos en vez de imponerse encima de ellos: un callee tiene su propio
ámbito, y un contexto de bucle es parte de un ámbito. La morfología y el tiempo de ejecución
coinciden aquí, que es el caso normal y vale la pena notarlo cuando se cumple.

#### 16.2 Restricciones por operador

| Restricción | Aplica a |
|---|---|
| solo en el cuerpo de una función | `<~` |
| requiere modo raw de un `>>\|` envolvente | `<<\|`, `<<\|?` |
| requiere una TTY; falla con salida redirigida | `>>\|` |
| solo en posición de typespec de entrada | `##"`, `##'` |
| solo en posición de sentencia | `<<`, `<<\|`, `<<\|?`, `><` |
| solo en el nivel superior de una rama de coincidencia | `\|\|` como patrón-o — los elementos de lista siguen siendo patrones primarios, así que `[1, 2]` nunca es ambiguo con dos alternativas |
| paréntesis obligatorios para operadores sufijos dentro de `>>` | `(arr$#)` |

---

## Parte V — Normativa

### 17. Reglas de diseño para nuevos operadores

Cada regla declara qué prohíbe, por qué, y cómo comprobar una propuesta contra ella.

**1 — Derivar, no inventar.**
Un nuevo operador debe poder explicarse como una composición de marcas ya presentes en el
inventario.
*Comprobación:* escribir la glosa interlineal (Apéndice A). Si cada segmento tiene una glosa
existente y la composición produce el significado buscado, el operador es derivable.
*Ejemplo:* `<<|?` = IN + UNIDAD + IRR no necesita ninguna marca nueva. Tampoco la necesitan la
entrada tipada, `||` en patrones, ni `##!` sobre `Char`.

**2 — Un significado abstracto por marca base.**
Un nuevo uso de una marca existente debe ajustarse al contrato de esa marca (Parte III).
*Comprobación:* encontrar el contrato, aplicarlo a la propuesta, y ver si la frase resulta
verdadera. `~` significa modificación; un nuevo `~X` debe involucrar transformar algo.

**3 — Las restricciones de contexto se heredan, no se repiten.**
Si un dominio lleva una restricción, todo nuevo miembro de ese dominio la lleva también.
*Comprobación:* §16.1. Una nueva sentencia `@` que actúa sobre el contexto temporal es inválida
fuera de un bucle, y esto no es una decisión que deba volver a tomarse.

**4 — Ninguna palabra de lenguaje natural en la gramática.**
Ni en inglés, ni en ningún otro idioma. El control de flujo, los tipos, los operadores y las
declaraciones son marcas.
*Alcance:* la gramática, no el léxico. Los identificadores son libres; los nombres de módulo y
de función son identificadores; los tipos de error llenan una ranura de identificador. El §15 es
el residuo exhaustivo, y cualquier adición a él es un cambio que exige el mismo escrutinio que
una marca nueva.

**5 — Ninguna marca base nueva sin un carácter abstracto documentado.**
Si ninguna marca existente sirve, el significado de la marca nueva se define en este documento
*antes* de implementarse.
*Comprobación:* la marca tiene una glosa, un contrato, un paradigma que encabeza o al que se
une, y una clase de posición.
*Por qué importa el orden:* una marca que se publica antes de describirse adquiere su
significado de lo que resulten ser sus primeros usos, y ese significado después es muy difícil
de corregir. La descripción es el diseño; la implementación la sigue.

**6 — Ninguna marca puede llevar dos significados sin relación.**
*Comprobación:* si las dos lecturas no pueden enunciarse como un solo contrato, son homógrafos,
y un homógrafo es un defecto que hay que saldar, no una característica que documentar y olvidar.
*Deuda pendiente:* el §13 enumera seis. `\` / `\\` (§13.1) es el que más vale la pena retirar.
*Ejemplo de pago:* `<=` significó alguna vez tanto "menor o igual" como "conocido como" en los
alias de módulo. Las dos lecturas no comparten contrato, así que la segunda se trasladó a `=>`,
donde la flecha hacia afuera dice lo que hace la correspondencia. `<=` es ahora exclusivamente
comparación.

**7 — Preferir lo icónico sobre lo convencional.**
Cuando hay dos formas derivables disponibles, elegir la que su forma representa su significado.
*Justificación:* §1.3 — los signos icónicos no cuestan nada aprender y los convencionales
cuestan una consulta.
*Ejemplos:* `<>` para "distinto de" en vez de `!=`; `><` en vez de una forma con letras; todo el
paradigma de tipo `##`.

**8 — La modalidad va al final.**
Un `?` o `!` modal es la marca más a la derecha del operador (§3.2). Ningún argumento o etiqueta
va después de él.
*Ejemplo:* por eso el break etiquetado es `@:exterior!` y no `@!exterior`.

---

### 18. El registro de combinaciones ocupadas

Consultar antes de diseñar un nuevo operador. Aquí está listada cada combinación que el lenguaje
gasta actualmente; lo que no aparece está sin gastar.

#### `>>` — flujo de salida
| Forma | Significado |
|---|---|
| `>>` | imprimir |
| `>>!` | limpiar pantalla |
| `>>?` | consultar tamaño de terminal |
| `>>~` | salida posicionada / con estilo |
| `>>\|` | bloque TUI |

#### `<<` — flujo de entrada
| Forma | Significado |
|---|---|
| `<<` | leer línea |
| `<< ##.` / `<< ###` / `<< ##"` / `<< ##'` | entrada tipada |
| `<<\|` | leer tecla, bloqueante |
| `<<\|?` | leer tecla, no bloqueante |

#### `@` — contexto temporal / de bucle
| Forma | Significado |
|---|---|
| `@` | bucle (infinito / N / mientras / para-cada) |
| `@!` / `@>` | romper / continuar |
| `@:etiqueta` | bucle etiquetado |
| `@:etiqueta!` / `@:etiqueta>` | romper / continuar etiquetado |
| `@etiqueta` | etiqueta fusionada (§6.3) |
| `@~` | dormir |

#### `#` — meta / tipo
| Forma | Significado |
|---|---|
| `#` | declaración de módulo |
| `#>` / `<#` | exportar / importar |
| `#1` / `#0` | literales Bool |
| `#\|` | evaluación numérica |
| `#?` | metadatos de tipo (sufijo) |
| `#.N` / `#!N` | redondear / truncar N decimales |
| `#,` / `#^` | formato de coma / científico |
| `#d₀d₉#` | modo numérico |
| `##.` / `###` / `##!` | conversiones |
| `##"` / `##'` | marcadores String / Char — solo en posición de typespec de entrada |
| `##]` `##)` `##()` `##->` `##?` `##_` | símbolos de tipo, solo resultados de `#?` |
| `##<Ident>` | tipo de error |

#### `$` — colección
| Forma | Significado |
|---|---|
| `$#` | longitud |
| `$+` / `$+[i]` | agregar / insertar |
| `$-` / `$--` / `$-[i]` / `$-[i..j]` / `$-[i:n]` | variantes de quitar |
| `$?` / `$??` | contiene / todos los índices |
| `$[i..j]` / `$[i:n]` | variantes de corte |
| `$^` / `$^+` / `$^-` | variantes de orden |
| `$>` / `$\|` / `$<` | mapear / filtrar / reducir |
| `$~~` | reemplazar todo |
| `$/` | dividir |
| `$*` | repetir |
| `$++` | concatenar-construir |
| `$!` / `$!!` | es error / propagar |
| `$~` | actualización funcional (sufijo sobre un índice) |

#### `~` — modificación
| Forma | Significado |
|---|---|
| `<~` | retorno / parámetro de salida |
| `param~` | parámetro mutable |
| `$~` / `$~~` | actualización funcional / reemplazar |
| `@~` | dormir |
| `>>~` | salida posicionada |

#### `!` — fuerza / error
| Forma | Significado |
|---|---|
| `!` | NO lógico |
| `@!` | romper |
| `!?` / `:!` | intentar / capturar |
| `$!` / `$!!` | es error / propagar |
| `##!` / `#!N` | conversión truncante / truncar decimales |
| `>>!` | limpiar pantalla |

#### `=>`, `->`, `<~`, `|>` — flechas
| Forma | Significado |
|---|---|
| `=>` | rama de coincidencia, alias de importación, renombrado de exportación |
| `->` | lambda |
| `<~` | retorno |
| `\|>` | pipe |

#### Sin asignar pero alcanzable
`&` (simple) es hoy un error léxico y por lo tanto está libre. `>>=`, `<<=`, `@?`, `$&`, `#&` y
`##&` no están ocupados. Cualquiera de ellos está disponible para una propuesta que sobreviva el
§17.

---

## Parte VI — Diacronía y hallazgos

Aquí vive todo lo que tiene una fecha. Las Partes I–V describen un sistema; esta parte registra
cómo llegó a ser así y qué reveló contrastarlo con el lenguaje en ejecución.

La separación es deliberada. Un sistema de signos leído como un todo debería leerse como un
todo — no como prosa interrumpida cada pocos párrafos por una nota sobre qué versión se
equivocó en algo. Esas notas vale la pena conservarlas; simplemente no vale la pena leerlas
primero.

### 19. Diacronía del sistema de signos

Aquí solo se registran cambios al *sistema de signos*; el historial de funcionalidades y
correcciones está en `CHANGELOG.md`. El patrón que vale la pena notar es la segunda columna: el
crecimiento es casi enteramente recombinación, y una marca genuinamente nueva ha ocurrido una
vez en cinco versiones.

| Versión | Cambio al sistema de signos |
|---|---|
| **v0.0.5** | Una marca base nueva — `°`, el diacrítico de definición activa, con dos lecturas posicionales (§4.6) — y siete operadores derivados: la familia TUI `>>!` `>>?` `>>~` `>>\|`, el par de entrada por tecla `<<\|` `<<\|?`, y `@~`. |
| **v0.0.6** | Ninguna marca nueva. `=>` se unificó como el único separador "se convierte en" en ramas de coincidencia, alias de importación y renombrados de exportación (con ruptura): `pat : resultado` → `pat => resultado`, `<# ruta <= alias` → `=>`, `#> { fn <= pub }` → `=>`. Se retiró el doble rol de `<=`, saldando una violación de la regla 6. |
| **v0.0.7** | Ninguna marca nueva. Entrada tipada por composición — `<< ##.(5,2)`, `<< ###(4)`, `<< ##"(20)`, `<< ##'` — que ocupó por primera vez `##"` y `##'`. Se estableció la biblioteca estándar como módulos y no como símbolos, según la rúbrica símbolo-frente-a-módulo. |
| **v0.0.8** | Ninguna marca nueva. `\|\|` se extendió a las ramas de coincidencia como patrón-o, reconocido solo en el nivel superior de una rama. `##!` se extendió a `Char` → punto de código (`##!'A'` → `65`), la única ruta directa Char→Int. Se añadió `std/term` como módulo, deliberadamente y no como símbolos. Se añadió el empaquetado `.zyp` sin ninguna superficie de lenguaje. |
| **v0.0.9** | Ninguna marca nueva. `->` acepta una lista de parámetros vacía: `() -> cuerpo` es un thunk (§9.6) — un cambio a qué puede llenar la ranura antes de la flecha, no a la flecha. Dos cambios de aplicación sin ninguna superficie: `@!`/`@>` y los saltos etiquetados pasaron a ser errores semánticos (§16.1), y el motor del navegador empezó a verificar el número de argumentos. |

---

### 20. Qué encontró la descripción del sistema

Las Partes I–V se escribieron contrastando cada afirmación con los cuatro motores en vez de con
la edición anterior de este documento. Eso resultó ser una manera de encontrar defectos, lo cual
no era la intención. Se registran aquí porque el *tipo* de defecto es instructivo: cada uno es
un lugar donde la notación decía algo que la implementación no cumplía.

#### 20.1 Defectos en el lenguaje, hallados al describirlo

| Hallazgo | Qué expuso la descripción | Estado |
|---|---|---|
| `@:etiqueta!` con una etiqueta que no se puede resolver | La concordancia (§4.5) es un requisito morfológico, y nada lo hacía cumplir. Cuatro motores, cuatro comportamientos — el tree-walker deshacía en silencio todos los bucles envolventes. | Error semántico en los cuatro (REFERENCE.md L29) |
| `@!` / `@>` fuera de cualquier bucle | El mismo vacío, sin etiqueta. | Misma corrección |
| `@~` fuera de un bucle | Lo inverso: documentado como restringido, nunca restringido por nada. Escribir por qué un miembro hereda una restricción (§16.1) mostró que este no tiene ninguna razón para hacerlo. | Se corrigió la documentación, no el código |
| `() -> cuerpo` | La ranura de parámetros antes de `->` se describía como necesitada de al menos un miembro. Nada lo exigía, y dos motores ya ejecutaban la forma vacía. | Legal en todas partes (REFERENCE.md L30) |
| Conteo de argumentos en `zymbol.js` | No es un hallazgo notacional — se encontró con las mismas corridas de los cuatro motores. | Verificado (REFERENCE.md L31) |
| `->  {` en el formateador | Dos espacios después de la flecha de un lambda de bloque. El §4.7 de `FORMATTER_RULES.md` decía uno. | Corregido |

La forma común: **una regla que está escrita pero no se hace cumplir no es una regla.** Tres de
los seis eran reglas que este mismo documento ya enunciaba (concordancia de etiqueta, la
restricción de contexto de bucle, y la supuesta participación de `@~` en ella); una cuarta la
enunciaba la gramática, y la implementación tampoco coincidía con ella. Enunciar una regla y
comprobarla son actos distintos, y solo el segundo se sostiene.

La herramienta que lo hizo detectable es `tests/scripts/engine_compare.sh`, que ejecuta un
programa a través del tree-walker, la VM de registros, `zymbol.js` y zyml al mismo tiempo. Toda
suite existente compara un *par* de motores, y un par puede a lo sumo contener dos de cuatro
respuestas en desacuerdo.

#### 20.2 Defectos en este documento, hallados de la misma manera

Registrados para que el modo de fallo quede legible en vez de disimulado.

- `°` estaba ausente, aunque es la marca base más reciente que acuñó el lenguaje y el único
  diacrítico que tiene.
- El paradigma `>>` listaba un solo miembro; tiene cinco. `>>!`, `>>?`, `>>~` y `>>|` no
  aparecían ni en la sección de familia ni en el registro de ocupadas — así que el §18, cuyo
  propósito entero es prevenir colisiones, estaba anunciando cuatro combinaciones como libres.
- `<<|?` se documentaba como que devolvía `''`, que no es un literal `Char` válido de Zymbol y
  nunca fue el valor real en tiempo de ejecución.
- `><`, `\`, `\\`, `$*`, `&&`, `<>`, `;` y las marcas aritméticas y de comparación no tenían
  entrada en ninguna parte.
- La regla de diseño 6 decía que el doble rol de `<=` estaba "programado para corregirse"
  mientras la sección de arriba decía que la corrección ya se había publicado. Ya se había
  publicado.

Todos estos son el mismo fallo: el documento se mantenía contra sí mismo. La nota `Método` al
principio existe para impedir que eso se repita.

---

## Apéndice A — Convenciones de glosado

Las glosas interlineales de este documento usan las siguientes abreviaturas. Una línea de glosa
alinea una abreviatura por morfema, en el orden de la fuente.

| Glosa | Morfema | Glosa | Morfema |
|---|---|---|---|
| COLL | dominio de colección `$` | IRR | irrealis / incierto `?` |
| TEMP | dominio temporal `@` | FRZ | fuerza / terminar `!` |
| META | nivel meta `#` | ERR | dominio de error `!` |
| TYPE | nivel de tipo `##` | PL | reduplicación: exhaustivo / plural |
| IN | flujo hacia adentro `<` `<<` | MOD | modificación `~` |
| OUT | flujo hacia afuera `>` `>>` | GATE | puerta `\|` |
| UNIDAD | granularidad de unidad única `\|` | NBND | no vinculante `_` |
| VIN | vinculación `:` | ETQ | una etiqueta de bucle |
| MAP | se convierte en `=>` | PROF | paso de profundidad en una ruta de navegación `>` |
| SUMA / RESTA | `+` / `-` | ORDEN / REV | `^` / `-` en formas de orden |

Ejemplo:

```
arr  $     ?      ?
     COLL  IRR    PL
     "preguntarle a la colección dónde, exhaustivamente"     → todos los índices de un valor
```

---

## Apéndice B — Índice de grafemas

Dónde se trata cada marca del §2.1.

| Marca | Tratamiento principal | También |
|---|---|---|
| `>` | §12.2 dirección | §8.4, §9.7 |
| `<` | §8.5 hacia adentro | §12.2 |
| `=` | §10 | §9.5 `=>` |
| `!` | §8.8, §12.1 | §5.2 |
| `+` `-` | §10 aritmética | §8.1 `$+` `$-` |
| `*` | §13.6 | §8.1 `$*` |
| `/` | §13.6 | §8.1 `$/` |
| `%` | §10 | |
| `^` | §13.6 | §8.1 `$^` |
| `&` | §5.1 — solo `&&`; `&` está libre | §18 |
| `\|` | §9.2 puerta frente a cerca | §8.5 |
| `?` | §8.7 irrealis | §5.2, §13.4 |
| `:` | §9.4 vinculación | §13.5 |
| `.` | §9.7 entrar en | §13.2 |
| `,` | §10, §5.4 morfema cero | |
| `;` | §10 | |
| `( )` | §4.4, §11 | |
| `[ ]` | §13.3 cinco roles | §4.4 |
| `{ }` | §4.4 | §10 interpolación |
| `@` | §8.2 temporal | §16.1 |
| `~` | §9.1, §12.3 | |
| `#` | §8.3 meta | §12.4 |
| `$` | §8.1 colección | |
| `¶` | §6.1, §14 | |
| `\` | §13.1 | §6.1 |
| `°` | §4.6 diacrítico | §2.2 |
| `"` `'` | §2.2, §2.3 | §11 |

---

## Documentos relacionados

| Documento | Responde |
|---|---|
| `GUIDE.md` | Cómo escribir Zymbol; la referencia autorizada del lenguaje |
| `REFERENCE.md` §21 | Qué hace cada operador — la tabla de consulta |
| `REFERENCE.md` §20 | Limitaciones conocidas y su estado |
| `IMPLEMENTATION.md` | Gramática EBNF, cobertura de funcionalidades, internos de los motores |
| `zymbol-lang.ebnf` | La gramática normativa |
| `MEMORY_MODEL.md` | Semántica de ámbito y vida detrás de `°`, `\`, `~` |
| `FORMATTER_RULES.md` | Cómo se disponen las marcas en la página — espaciado, bloques, líneas en blanco |
| `CHANGELOG.md` | Historial completo de versiones |

---

## Notas

[^quotes]: `ab"c" = 5` vincula un identificador cuyo nombre contiene dos comillas, y
`>> ab"c" ¶` imprime `5`. El escáner llega a sus ramas de cadena y carácter antes que a su rama
de identificador, así que `"` y `'` están reservados al inicio de un token; `is_ident_continue`
nunca los excluye, así que en medio del token no lo están.

[^sentinel]: `'\0'`, el carácter nulo, de `crates/zymbol-interpreter/src/io.rs`. Un programa
distingue "sin tecla" de una pulsación real comparando contra él:
`? k <> '\0' { … }`.

[^atsleep]: Ningún motor ha exigido nunca un bucle alrededor de `@~`, en ninguna versión. La
restricción se afirmaba por herencia del prefijo `@` en vez de derivarse de lo que hace la
sentencia — ver §20.1.
