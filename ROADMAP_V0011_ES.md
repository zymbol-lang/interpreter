# Mapa de ruta — v0.0.11: la VM es el motor, y lo que cuesta cada cosa

> **Estado: abierta.** La rama `v0.0.11` existe desde el 2026-10-12; su primer commit fue el de
> versión (`62baa56`). **La etapa 1 está implementada** (2026-10-13) salvo el sondeo de las
> cinco zonas: ver «Avance» dentro de ella. **La etapa 2 tiene lista de trabajos y arnés de medición**
> (2026-10-14), y las cinco correcciones de la lista hechas: PERF-06, PERF-04, PERF-05 a, PERF-03 y PERF-07
> (2026-10-16, `ZYVM-023`, `ZYVM-025`, `GLB-130`, `GLB-132` y `GLB-133`). Quedan PERF-05 b (aplazada, D8) y PERF-08
> (reproducir primero). Ver «Avance» dentro de ella. De las etapas 3 y 4 no hay nada hecho.
>
> **Plan de cierre (2026-10-19):** lo que queda va en seis grupos, aprobados por el autor con la opción recomendada de
> cada uno; ver «Plan de cierre — lo que queda, por grupos». Los grupos 1 a 5 están hechos.
>
> **Avance del 2026-10-20:** 3.2 (forma A), 3.1 (forma B) y 3.4 (forma A) hechas; 3.3 aplazada por el autor, que la
> retoma quizá el 2026-10-23; la etapa 4, en pausa entera. De paso: `ZYVM-031`, `GLB-142` y `GLB-143`, corregidos.
>
> **Alcance y fecha (decisión del autor, 2026-10-20):** las etapas 3 (3.1 a 3.4) y 4 (`zyvm.js`) **entran en la v0.0.11**,
> empezando por 3.2. Esto deroga lo que el grupo 6 y D5 decidieron el 2026-10-19, que las pasaban a la v0.0.12. La v0.0.11
> sale el **2026-11-01**; si falta mucho, como tarde el **2026-11-11**. Recortar el alcance para llegar lo decide el autor.
>
> **Escrito el 2026-10-13.** Toda cifra sin otra fecha se midió ese día, sobre esa rama, con un
> binario de release. Las que vienen de antes llevan su fecha y hay que volver a medirlas antes
> de citarlas fuera de aquí.
>
> **Es un plan, no un registro.** Lo que se haga se cuenta después en `IMPL_V0011.md`, con el
> mismo esqueleto que `IMPL_V0010.md`. Si este documento y la rama discrepan, manda la rama.

---

## Qué clase de release es

La v0.0.8 fue de **deuda**, la v0.0.9 de **decisión**, la v0.0.10 de **medición**. La v0.0.11 es
de **optimización**: el autor la definió como «la optimización de tiempos y recursos del
lenguaje». No es un release de funciones.

«Recursos» son tres cosas distintas, y conviene no mezclarlas porque se miden distinto:

| Recurso | Qué es | Con qué se mide |
|---|---|---|
| Tiempo | Lo que tarda un programa en ejecutarse | Instrucciones de CPU (`perf`), `--count`, `zyquality/cost/` |
| Memoria | Lo que ocupa un programa mientras corre | Pico de RSS, `allocs`/`bytes`/`copied` de `--count` |
| Peso | Lo que cuesta construir, descargar e instalar el lenguaje | Pico de memoria al compilar, tamaño del binario, número de crates, bibliotecas exigidas |

## El orden

| Etapa | Qué es | Termina en |
|---|---|---|
| 1 | La VM pasa a ser el motor por defecto de Rust | Un cambio hecho y sostenido por la puerta |
| 2 | Optimización de tiempos y recursos | Mejoras medidas, una por hallazgo |
| 3 | Cuatro evaluaciones: componentes, motor compacto, `.zyb`, WebAssembly | Una **decisión** del autor por cada una |
| 4 | La VM del navegador (`zyvm.js`) | En la v0.0.11, después de la etapa 3: ver decisión D5 |

La etapa 1 va primero porque todo lo demás se mide sobre el motor que la gente ejecuta: optimizar
antes de cambiar el defecto es optimizar el motor equivocado. La etapa 3 evalúa y no construye;
lo que salga de ella con un «implementar» entra en esta versión o en la siguiente según lo decida
el autor.

---

## Punto de partida

Medido el 2026-10-13. Tiempos de pared de **una sola corrida**, con arranque incluido: sirven para
ver órdenes de magnitud, no para comparar mejoras de un 5 %.

### Tiempo

| Programa | Tree-walker | VM | `zymbol.js` (node) |
|---|---|---|---|
| `fib(30)` | 1 186 ms | 175 ms | 10 052 ms |
| `bench/bench_recursion.zy` | 1 500 ms | 185 ms | 13 700 ms |
| `bench/bench_collections.zy` | 37 ms | 13 ms | 295 ms |
| `bench/bench_strings.zy` | 67 ms | 29 ms | 578 ms |

Node tarda unos 55 ms en arrancar y comprobar un programa de una línea; en las dos últimas filas
eso es una parte visible del total.

### Memoria

| Programa | Tree-walker | VM | `zymbol.js` (node) |
|---|---|---|---|
| Programa de una línea | 11,1 MB | 11,0 MB | 57,2 MB |

### Peso

Una compilación limpia de `zymbol-cli` en release, muestreando cada proceso del compilador:

| Medida | Valor |
|---|---|
| Crates en el árbol de dependencias | 208 |
| De ellos, los que entran por `zymbol-lsp` | 116 |
| Suma máxima de memoria de los procesos del compilador (16 trabajos) | 3 850 MB |
| Enlazado final con LTO, un solo proceso | 1 342 MB |
| Compilar el crate `zymbol_vm`, un solo proceso | 798–890 MB |
| Tamaño del binario `zymbol` | 12,76 MB |
| Tiempo de la compilación limpia | 3 min 23 s |

El perfil de release es `lto = true`, `codegen-units = 1`, `strip = true`.

Lo que había en memoria en el momento del pico no era el lenguaje:

- **El LSP dentro del binario `zymbol`:** `tokio`, `lsp_types` (589 MB en su proceso),
  `tower_lsp`, `futures_util`.
- **El portapapeles del REPL (`arboard`):** `image`, `moxcms`, `pxfm`, `x11rb_protocol` — un códec
  de imágenes y el protocolo X11 para copiar texto.
- **HTTP (`ureq`):** `rustls` (404 MB) y `ring`.

`std/db` pesa poco y molesta por otra razón. Sin la feature `db` el árbol baja de 208 a 204
crates, el binario de 12,76 a 12,54 MB y el enlazado final de 1 342 a 1 329 MB. Lo que cambia de
verdad es que el binario por defecto exige `libodbc.so.2` y `libltdl.so.7` para **arrancar**. Los
releases de Linux y macOS ya se construyen con `--no-default-features`; quien se lo encuentra es
quien sigue el `cargo build --release` del README.

---

## Etapa 1 — La VM es el motor por defecto

**Decidido por el autor** el 2026-10-11 para esta versión, como segundo commit de la rama.
`zymbol run fichero.zy` ejecuta en la VM; el tree-walker sigue existiendo y se pide con `--tw`.

### Antes del cambio: sondear cinco zonas

Los goldens y el veneno de temporales prueban la superficie que alguien declaró, no lo que nadie
ha preguntado. El día que se sondeó una sola zona (ediciones, estado de módulo, `<~`) salieron
siete fallos de la VM y siete del tree-walker que la puerta no veía. Quedan cinco sin sondear:

1. Bucles y variables calientes (`°x`, `x°`)
2. `??` en todas sus formas de patrón
3. Flujo de errores (`!?`, `:!`, `:>`, `$!!`)
4. Cadenas y formato
5. Módulos

El método es el de la sonda de operando con efecto (distingue una evaluación de dos y un orden de
otro, que es lo que un golden no puede), en los tres motores, y con `ZYMBOL_POISON_TEMPS=1` para
la VM. Cada diferencia se archiva como hallazgo con su celda `open_finding` y se decide una a una.

### Lo que exige el cambio

| Pieza | Qué hay hoy | Qué hay que hacer |
|---|---|---|
| `zymbol-cli`, subcomando `run` | El tree-walker es el defecto; `--vm` lo cambia; `--tw` ya existe y hoy sólo importa para un `.zyp` | Invertir el defecto. La ayuda de `--vm` dice «(experimental)» (`main.rs:43`) y deja de ser cierto |
| Los arneses | Al menos 15 ficheros de `zyquality/`, `ZyDDT/` y `web/tests/` invocan `run` sin nombrar motor | Nombrar el motor en cada uno, **en el mismo cambio** |
| Los goldens | Se graban con `zytw` (`zyquality/GOVERNANCE.md`) | Decisión D1 |
| El REPL | `zymbol-repl` depende sólo de `zymbol-interpreter`: no tiene camino a la VM | Decisión D2 |
| Los documentos | Al menos 12 ficheros afirman que el tree-walker es el defecto | Corregirlos; `write-zymbol/SKILL.md` obliga a regenerar su SHA-256 y `LLM.md` a tocar sus sondas |

**La trampa de esta etapa está en la segunda fila.** `zyquality/engines.toml` define `zytw` como
`zymbol run {file}` y `zyvm` como `zymbol run --vm {file}`. El día que el defecto cambie, sin
tocar ese fichero, `zytw` ejecutará la VM: la puerta comparará la VM consigo misma, dará cero
divergencias y seguirá verde. Lo mismo vale para `ZyDDT/engines.toml`. Es el fallo que
`GOVERNANCE.md` describe —«nada corrió» leído como «nada falló»— con otra forma.

Los 12 ficheros de la última fila, de una búsqueda textual que puede dejarse alguno:
`ARCHITECTURE.md`, `IMPLEMENTATION.md`, `GUIDE.md`, `LLM.md`, `CHANGELOG.md`, `MANUAL.md`,
`IMPL_V007.md`, `IMPL_V0010.md` (los tres últimos son históricos y no se tocan), más
`zymbol-design/FORMATTER_RULES.md`, `web/install.md`, `web/changelog.md` y
`web/.well-known/agent-skills/write-zymbol/SKILL.md`. Falta sumar el `CLAUDE.md` de la raíz del
espacio de trabajo.

### Avance — 2026-10-13

Hecho, y subido a la rama `v0.0.11` de cada repositorio:

- **El CLI:** `zymbol run fichero.zy` ejecuta en la VM; `--tw` pide el tree-walker; `--vm` se
  acepta y no cambia nada. La ayuda ya no dice «experimental». Comprobado con `--count` sobre
  `fib(30)`: sin flag y con `--vm` dan 366 asignaciones, con `--tw` 10 770 646.
- **Los arneses nombran el motor:** `zyquality/engines.toml`, `ZyDDT/engines.toml`, y las suites
  con tabla propia (`tui`, `llm`, `cost`, `coverage`, `bench`, `project`, y el corredor de
  PowerShell). También los corredores de ZyBank, serpiente, klingon_galaxy y atrapa, y las sondas
  de GO y Divergente_ES, donde `""` significaba «tree-walker».
- **Los documentos del intérprete** que afirmaban el defecto antiguo: `ARCHITECTURE.md`,
  `IMPLEMENTATION.md`, `GUIDE.md`, `LLM.md`, `README.md`, `AGENTIC.md`, y el `CLAUDE.md` de la raíz.
- **El inventario de mensajes:** podada una entrada, el texto de ayuda que ya no existe.

Medido con el defecto nuevo: `zyq suite` da *all gates pass* (683 ficheros, 677 coinciden, 0
divergen; 647 + 30 goldens; las nueve aplicaciones; ZyDDT en verde; `cost` en verde con su `KNOWN`
de ZYVM-022), `cargo test` 1 065 pasan y 0 fallan, `zyddt premises` 29 premisas y ninguna sin celda.

Lo que **no** está hecho:

- ~~**El sondeo de las cinco zonas.**~~ **Hecho el 2026-10-16**, después de la etapa 2. Cada zona es un programa
  con operandos que imprimen, en los tres motores y con `ZYMBOL_POISON_TEMPS=1`, y queda en el corpus
  (`zyquality/corpus/sondeo_v0011/`). Encontró y se corrigieron: `GLB-135` (la condición de `@ cond` se evaluaba
  una vez de más en los dos motores de Rust), `ZYJS-063` (`()` era un valor en zyjs), `ZYJS-064` y `ZYJS-065`
  (listas de patrones anidadas y una comparación evaluada dos veces) y `ZYJS-066` (un `_err` de más en un `:!`
  anidado). Dejó dos preguntas para el autor: el formato en base de un entero negativo (`0x|-1|`) y qué hace una
  línea que empieza por una edición después de un `>>` sin `¶`.
- **D1, D2 y D3** se dejaron en su opción conservadora, que no cambia nada: los goldens los sigue
  grabando `zytw`, el REPL sigue en el tree-walker, `--vm` se conserva.
- **Los documentos de `web/`** (`install.md`, `write-zymbol/SKILL.md`) llevan sello `0.0.10` y
  describen el binario publicado: cambian en la release, con su sello y su SHA-256.
- **`LLM.md` sigue titulado v0.0.10** y la suite `llm` lo marca (no es puerta). Viene del commit de
  versión, no de este cambio: las 88 sondas pasan.
- **`GUIDE.md` § 3b** dice que algunas primitivas de TUI son sólo del tree-walker. Con la VM por
  defecto esa frase hay que comprobarla o quitarla.
- **Las suites que no nombran motor** (`guide`, `fmt`) siguen al defecto: desde hoy prueban la VM.

### Qué queda del tree-walker

No se retira. Sigue siendo el segundo testigo del consenso (dos motores de Rust que no comparten
diseño), el motor del REPL mientras D2 no diga otra cosa, y el banco donde se prueba primero una
función nueva. Lo que pierde es la prioridad: un problema que sólo afecta a su rendimiento deja de
ser urgente.

### Cómo se sabe que está hecha

- `zyq suite` da *all gates pass* con el defecto nuevo, y `zyq consensus` sigue contando dos
  motores de Rust distintos (comprobar que `zytw` y `zyvm` no ejecutan lo mismo, no sólo que
  coinciden).
- `zyddt premises` y `zyddt axis`, sin celda roja que nadie haya decidido.
- Las aplicaciones de `zyquality/project/`, también con `ZYMBOL_POISON_TEMPS=1`.
- `zymbol run --help` no dice «experimental».

---

## Etapa 2 — Optimización de tiempos y recursos

### De dónde sale la lista

Este documento decía que la etapa no tenía lista de trabajos y que escribirla sería inventarla. La
lista salió de fuera: **ZyBigBench**, una medición hecha entre el 2026-10-10 y el 2026-10-13 en otra
sesión (una cuenta aparte de claude.ai, en un contenedor de 4 GB), que buscó defectos por **curva de
crecimiento** — la misma operación a n, 2n, 4n y 8n — en vez de por tiempo a un solo tamaño. Encontró
ocho (PERF-01 a PERF-08); el 2026-10-14 se trajo a `zyquality/bench/big/` y se volvió a medir sobre
esta rama con los instrumentos del proyecto.

PERF-01 y PERF-02 ya estaban corregidos (`ee48143`, ZYVM-013, 2026-10-10). Los otros seis siguen
vivos. Las recomendaciones de abajo se incorporaron al plan por decisión del autor el 2026-10-14; cada
una se archiva como hallazgo antes de tocar código.

Los defectos de complejidad no necesitan un perfil para ordenarse: el perfil de cargas reales —GO a
19×19, la búsqueda de Chaturanga, `ZyBF/pesado2.zy`— sigue haciendo falta, pero para las
**constantes**, y va después.

### La lista

Cifras en instrucciones (`perf stat -e instructions:u`, mediana de 7, VM de esta rama), salvo donde se
dice otra cosa. El control es el mismo programa sin el defecto.

| Orden | Id | Qué | Recomendación | Motores | Cómo se sujeta en `cost/` |
|---|---|---|---|---|---|
| 1 | PERF-06 | `s = s "x"` es cuadrático | Implementar | VM | `--count` bytes, exacto |
| 2 | PERF-04 | `@ ch:texto` es cuadrático | Implementar | VM | Reloj |
| 3 | PERF-05 a | `s$[a..b]` copia el texto entero | Implementar | VM y TW | `--count` bytes, exacto |
| — | PERF-05 b | `s$#` y `s[i]` recorren el texto | Aplazar (D8) | VM y TW | — |
| 4 | PERF-03 | El diccionario busca recorriendo | Implementar, como pieza propia | VM, TW y zyjs | Reloj |
| 5 | PERF-07 | La recursión profunda aborta sin error de Zymbol | Implementar el tope (D7) | VM y TW | Celda de ZyDDT |
| — | PERF-08 | El binario estático con musl es más lento | Reproducir primero | Empaquetado | — |

**1. PERF-06 — `s = s "x"`.** 128 000 añadidos cuestan 968 M instrucciones contra 290 M con la mitad
(×3,3), y `bytes` va de 2,05 a 8,20 GB (×4,00). Control: el tree-walker es lineal (b = 1,00), y
`v = v $+ i` en la VM dobla al doblar n. Causa: el compilador manda `ConcatStr` a un temporal nuevo
(`zymbol-compiler`, `BinaryOp::Concat`), y la rama de la VM que reutiliza el búfer exige que destino y
operando izquierdo sean el mismo registro: la vía rápida existe y no se alcanza. Arreglo: cuando el
destino de la asignación es la variable del operando izquierdo, emitir `ConcatStr(s, s, x)`. Va
primero porque es la forma de construir texto de cualquier programa y el arreglo es local. Riesgo: un
registro reutilizado mientras sigue vivo — `ZYMBOL_POISON_TEMPS=1` y la puerta entera.

**2. PERF-04 — `@ ch:texto`.** 4,5 G → 17,8 G instrucciones de 147 456 a 294 912 caracteres (×4 al
doblar); `--count` no lo ve (los pasos doblan). Control: el tree-walker es lineal, Python 33 → 66 M.
Causa: cada vuelta es un `StrCharAt(i)` que llama a `is_ascii()` sobre todo el texto, y con texto no
ASCII además a `chars().nth(i)`. Arreglo: una instrucción de iteración con un cursor de bytes en un
registro — O(1) por carácter, sin reservar, sobre la cadena que había al entrar al bucle. En
`strings_scan_long` la VM ejecuta ~1 300 instrucciones por paso donde `fib_rec` ejecuta ~110.

**3. PERF-05 a — `s$[a..b]`.** 2 000 cortes de 10 caracteres: 1,6 G instrucciones sobre 73 728
caracteres y 3,2 G sobre 147 456; Python 1,5 M. Control: `v$[1..10]` sobre un arreglo, 3,7 M a
cualquier tamaño. Causa: el corte copia el texto a un `Vec<char>` antes de cortar — 516 KB por corte
en la VM y 590 KB en el tree-walker, y `--count` lo ve exacto en bytes. Arreglo: localizar los dos
bordes con `char_indices` (o por bytes si el texto es ASCII) y copiar sólo el trozo. El mismo
`collect()` está en 12 sitios de la VM y 8 del tree-walker: se revisan todos.

**PERF-05 b — `s$#` y `s[i]`: aplazado.** 15 600 instrucciones por `s$#` sobre 147 456 caracteres
contra 490 de Python: un recorrido a ~40 GB/s, cuadrático sólo dentro de un bucle sobre un texto largo.
El arreglo de verdad cambia la representación de la cadena (una marca «es ASCII» o la cuenta de
caracteres dentro de `ZyStr`), y se decide con el perfil de cargas reales delante (D8). En el
tree-walker, además, **leer una variable de texto la copia** (`Value::String(String)`, sin `Rc`): es
HLZ-012 para cadenas, y se archiva como deuda declarada con su celda `KNOWN`.

**4. PERF-03 — el diccionario.** Leer la última clave cuesta ~40 000 instrucciones con 2 000 claves y
~105 000 con 4 000; Python ~425 a cualquier tamaño. Control: leer `d["k1"]`, plano. Por eso insertar
n claves es cuadrático en los tres motores (`dict_insert`, b = 1,98). Los tres buscan recorriendo:
`Vec<(String, Value)>` en los dos de Rust, `keys.indexOf` en `zymbol.js`. Arreglo: `IndexMap` (ya es
dependencia de `zymbol-common`), que conserva el orden de inserción que imprime `#(…)`; borrar con
`shift_remove`, nunca `swap_remove`. Va aparte porque cambia la representación del valor —igualdad,
impresión, recorrido, borrado, desestructuración— y porque la tupla con nombre comparte hoy esa
representación sin necesitar el índice.

**5. PERF-07 — el aborto.** El tree-walker aborta (134) entre 8 000 y 9 000 niveles; la VM, a través
de `$>`, entre 2 500 y 3 000 (~24 KB de pila de Rust por cada reentrada del bucle). Control: la
recursión directa en la VM llega a 2 048 000 niveles. Lo grave no es el límite: el proceso desaparece
sin error de Zymbol y sin la salida pendiente. Arreglo: un contador de profundidad que lance un error
de ejecución, con su línea, antes de agotar la pila (D7). Subir el límite de la VM achicando su marco
es otra cosa, y queda aplazada.

**PERF-08 — musl: reproducir antes de decidir.** Según ZyBigBench, el estático de la v0.0.10 es 11×
más lento en `str_concat` y 1,6–2,3× en interpolación, cortes y cierres, por el asignador de musl; y es
la descarga por defecto para «any Linux». No se reprodujo aquí. No hace falta compilar musl: se
descargan los dos binarios oficiales de la v0.0.10 (estático y `.deb`, mismo commit), se verifica su
SHA-256 y se les pasa `bench/big/`. Si se confirma: `mimalloc` sólo para el target musl, detrás del
asignador que cuenta para `--count`. Es empaquetado y se decide en la release. PERF-06 se lleva el
×11 de `str_concat` de todas formas.

### Avance — 2026-10-16

**PERF-06, corregido** como `ZYVM-023` (`ZyDDT/HALLAZGOS/zyvm.md`). Archivado primero, con tres casos
de `zyquality/cost/` como deuda declarada —uno por receptor: la variable del programa, la local de una función y
el estado de un módulo—, y cerrado después en el mismo día. `s = s x y` se escribe sobre el registro del receptor,
y `DetachGlobal` suelta también un texto, no sólo un agregado. `bytes` de `--count` de 2 000 a 8 000 añadidos:
×8,12 antes, ×1,12 ahora.

Dos cosas que salieron por el camino:

- **El corredor de `cost/` no leía `open_finding` en las razones contadas**: ninguna deuda declarada hasta hoy
  era de `--count`. Ahora lo hace, y una deuda se da por pagada sólo cuando pasan todas las lecturas de su
  motor — si no, PERF-04, ciego al conteo, se daría por pagado al declararlo.
- **Sin instrucciones de `perf`:** `perf_event_paranoid` está en 3. La cifra citable es la de `--count`, que es
  exacta; la de reloj, 1 200 000 añadidos en ~127 ms en la VM.

`zyquality/corpus/modules_scope/concat_receptor.zy` sujeta las formas que el arreglo podía romper.

**PERF-04, corregido** como `ZYVM-025`. El bucle de texto lleva un cursor de bytes y `StrCharNext` decodifica el
carácter que hay en él, en vez de buscar el carácter `i` desde el principio en cada vuelta. 1 280 000 caracteres
ASCII: 21,4 s antes, 0,21 s ahora. Se sujeta por reloj (`growth/foreach-over-text`), porque `--count` no lo ve.

Al leer ese bucle salió **`ZYVM-024`**, que la puerta no veía: cuando el iterable era un nombre, la VM recorría la
variable y no el texto que tenía al empezar, así que un cuerpo que la reescribía se recorría sobre el texto nuevo,
y pasado su final daba el carácter 0. Se corrigió antes que el cursor, que depende de ello;
`zyquality/corpus/loops/14_foreach_string_snapshot.zy` lo sujeta.

**PERF-05 a, corregido** como `GLB-130`. Un corte de texto localiza su trozo y lo copia solo, en vez de copiar el
texto entero a un `Vec<char>`: `zymbol_common::text`, usado por la VM y el tree-walker. 8 000 cortes de un carácter
sobre 8 000 «ñ» en la VM: 1 155 MB antes, 2,66 MB ahora. Se sujeta sólo por `bytes` (un `clock_off` nuevo en el
corredor de `cost/`): el reloj sigue cuadrático por PERF-05 b, que cuenta los caracteres en cada corte.

Por el camino salieron tres cosas más, todas archivadas:

- **`ZYVM-026`**, corregido: un texto cortado a través de un parámetro iba por `ArraySlice`, que recortaba el rango
  como antes de D3.
- **`GLB-129`**, corregido por decisión del autor (la VM es la referencia): `$[i:n]` no coincidía en ninguno de sus
  errores en los tres motores. Y `zyquality/messages/reach.py` corría `run` y `run --vm`, que desde la etapa 1 eran
  los dos la VM.
- **`ZYTW-016`**, abierto como deuda declarada: leer una variable de texto la copia en el tree-walker. Es la parte de
  PERF-05 b que este documento ya decía que se archivaría así.

**PERF-03, corregido** como `GLB-132`, en los tres motores. En Rust el diccionario es un `IndexMap`
(`zymbol_common::Dict`); en `zymbol.js`, un índice de claves y la escritura en el sitio que ZYJS-014 dio a los
arreglos. 16 000 claves: la VM 438 → 14 ms, el tree-walker 651 → 28 ms, `zymbol.js` 2 685 → 399 ms.

Antes hubo que decidir **`GLB-131`** —el autor, el 2026-10-16—: un literal que escribe una clave dos veces se
rechaza al analizar el programa. Los tres motores la guardaban dos veces, contra `COLLECTIONS.md` § 5, que ya decía
que las claves son únicas.

**PERF-07, corregido** como `GLB-133`, con D7 decidida por el autor: la llamada que haría más de **10 000 llamadas
activas** a la vez es un error `##Depth`, nuevo, que `!?` captura; el mismo número en los tres motores. La pila del
programa sube a 512 MiB y un seguro mira la que queda; `zymbol build` y el REPL corren ahora en ese hilo también.

Con un número, qué llamada cuenta pasó a verse, y la llamada en cola no se decidía igual en los tres. La regla que
quedó es la de la VM —`<~ f(…)` a la propia función, sin un `!?` suyo abierto—, y salieron dos hallazgos más,
corregidos: **`ZYTW-017`** (un `!?` de quien llama desactivaba la llamada en cola de toda la cadena de debajo) y
**`ZYVM-027`** (dentro de un módulo la VM nunca la tomaba). `LLM.md` § 12 cambió una frase, con su sonda.

**PERF-08, reproducido** como `GLB-134`, con los dos binarios oficiales de la v0.0.10 para x86-64 (SHA-256
verificado): el estático es ×17 más lento en `str_concat`, ×4,6 en `str_slice_small` y ×1,3 en interpolación y
cierres, e igual en los 16 programas largos. Lo peor era copiar un texto entero, que esta rama ya no hace (PERF-06,
PERF-05 a); cuánto queda en el `HEAD` no se ha medido, porque aquí no hay target musl. La decisión sigue siendo de la
release.

**Medición de la etapa 2, 2026-10-17** (`zyquality/bench/big/README.md` tiene las tablas). En la VM, frente a la rama
al empezar la etapa y frente a la v0.0.10 publicada, en la misma sesión: los 16 programas largos dan lo mismo que
Python y cuestan ×0,84 y ×0,79 en tiempo de CPU (bytes ×0,78); las operaciones de cada PERF bajan entre ×8 y ×50 y
crecen lineales. La comprobación de profundidad de GLB-133 encarecía un ~8 % una llamada vacía: se quitó el seguro de
pila de la llamada directa, que no recurre en Rust, y queda ~4 %, sin `perf` para atribuirlo.

**La etapa sigue abierta (2026-10-17).** El autor la amplió tras medir contra Python: «python es nuestro oráculo,
debemos acercarnos porque no es una competencia es una validación, pero debemos mejorar bajo nuestra oferta de valor
simbólica». Hecho desde entonces, en instrucciones y variantes del mismo `HEAD`: el bucle de la VM deja de buscar el
código de la función en cada instrucción y de soltar valores al escribir argumentos y resultados (`fib_rec` −6,9 %,
`tak` −8,5 %, `loop_mod` −7,8 %, `collatz` −8,2 %), y PERF-05 b como `ZYVM-028` (`s$#` de ×61 a ×1,2 de Python; un
corte corto de ×85 a ×2,1).

**El control del bucle contado, en una instrucción** (2026-10-17). Al volcar el bytecode de un `@ i:1..n` salió que
el control de cada vuelta eran nueve despachos —la dirección del rango se preguntaba dos veces en cada una— frente a
unos seis del trabajo. `RangeStep` hace el final de la vuelta entero: dos despachos. Frente al `HEAD` anterior, en
instrucciones: `loop_mod` −34 %, `matmul` −24 %, `match_loop` −23 %, `sort_builtin` −19 %, `array_update` −41 %.
Por el camino, **`ZYVM-029`**, corregido con ello: un bucle cuyo límite es 2^53−1 corría todas sus vueltas y abortaba
al salir, porque el contador avanzaba antes de compararse con el límite.

**`$>`, `$|` y `$<` sin reentrar en la VM** (2026-10-17, decidido por el autor). Una instrucción, `HofNext`, hace
cada vuelta: recoge lo que devolvió la lambda, toma el elemento siguiente y le monta el marco a la lambda, que vuelve
a ella con su `Return`. `ArrayMap`, `ArrayFilter` y `ArrayReduce`, que entraban en el bucle de la VM desde Rust una
vez por elemento, se retiran. Una primera versión con un bucle de ocho instrucciones por elemento **costaba más**
(`hof_chain` +52 %): la reentrada no era el coste; la llamada sí. Medido frente al commit anterior: `hof_chain`
−3,5 % (×0,97 de Python), `sort_cmp_300k` −1,9 %, `fib_rec` −1,5 % —esto último por un `Return` que ya no llama al
destructor de un error de marco vacío—. Por el camino salió **`ZYJS-067`**, corregido: `"abc" $> (c -> ##!c)` daba un
texto en zyjs y un array en Rust.

**Sin `LoadUnit` antes de `Return`** (2026-10-17). El auto-liberado del último uso vaciaba los parámetros justo antes
del `Return`, que suelta el marco entero en el mismo despacho: en `tak` eran dos de los cinco despachos del caso base.
El optimizador final los quita (nunca el registro devuelto; los parámetros de salida no se auto-liberan). `tak` −8,4 %
(de ×1,44 a ×1,32 de Python), `nqueens` −1,8 %. Al hacerlo salió **`ZYVM-030`**, corregido antes en su commit: un
parámetro que nada lee no tenía registro, y la llamada escribía fuera del marco sin comprobar límites — también en la
v0.0.10.

**La llamada, sin tres llamadas fuera de línea** (2026-10-17). Montar un marco hacía `Vec::push` del marco y
`resize_with` de sus registros fuera de línea, y un `mem::take` que creaba y soltaba un `Vec` vacío de parámetros de
salida en cada llamada. Las tres van en línea o no se hacen. `fib_rec` −6,7 %, `tak` −3,2 %, `hof_chain` −1,2 %.
Una prueba que no se quedó: copiar los escalares sin la llamada al `clone` derivado empeoraba (+0,5 a +1,2 %); con LTO
ya estaba en línea donde importaba.

**`$^` con comparador: probado y descartado** (2026-10-17). La otra mitad de lo decidido —ordenar sin reentrar— se
construyó como `HofNext`: una instrucción `SortNext` con el estado del merge sort de GLB-108 en registros, pregunta por
pregunta igual (la sonda con un comparador que imprime daba lo mismo en los tres motores). Costaba **más**:
`sort_cmp_300k` +6,8 %. Leer y escribir siete registros de estado en cada comparación pesa más que volver a entrar en
`exec`, donde el bucle de Rust tiene ese estado en registros de la CPU. Lo que acerca `$^` a Python es abaratar la
llamada misma, no cambiar quién la hace.

**`call_callable` más barato (opción A, 2026-10-17).** Escribía los argumentos con una asignación —comprobación de
límites y destructor del `Unit` que había— y los operadores le pasaban un clon del invocable en cada llamada. Ahora
`ptr::write`, como `Call`, y el invocable prestado. `sort_cmp_300k` −9,8 % (de ×1,25 a ×1,13 de Python). Medido
exacto, restando una variante sin `$^`: **648 instrucciones por llamada al comparador**, y con `HofNext` una lambda
identidad cuesta ~525 por elemento — una llamada con su retorno ronda las 400 en cualquier camino. Eso es lo que
queda, y la opción B (un comparador puro evaluado sin llamada) es la que lo evita en `$^`.

**Opción B: el comparador puro de `$^`, sin llamada** (decidida por el autor el 2026-10-17: sólo en la VM). Si el
trozo del comparador es una secuencia recta de `Load*`, `CopyReg`, `x.campo`, `x[i]`, `x$#` y las seis comparaciones,
sin escribir en sus parámetros, la VM lo evalúa sin marco (`call_pure`): el mismo merge sort de GLB-108, las mismas
preguntas, la misma comprobación de profundidad, los mismos `steps` y `calls` de `--count`, y el error donde lo daría
la llamada. Para que no puedan divergir, `ArrayGet`, `NamedTupleGet` y `ArrayLen` salieron a funciones que usan los
dos caminos —eso solo mueve `matmul` un +1 %, reacomodo del código de `exec`—. Un comparador con bloque, con efectos o
que reasigna un parámetro se sigue llamando. `sort_cmp_300k` −39 % frente al commit anterior: de ×1,13 a **×0,69** de
Python. La primera versión sólo bajaba un 15 %: lo que costaba no era el marco sino escribir registros con el
destructor del valor anterior y mover un `Result` del tamaño de `VmError` en cada comparación.
`zyquality/corpus/collections/sort_comparador_puro.zy` sujeta el «igual».

**`Instruction` con etiqueta propia, `#[repr(u8)]`** (2026-10-17). Sin ella, el compilador de Rust escondía la
variante en un nicho de la capacidad de un `Vec`, y el bucle de la VM la reconstruía en cada instrucción. Una línea:
entre −3 % y −12 % en los 16 programas (`fib_rec` ×1,10 → ×1,02, `nqueens` ×1,06 → **×0,95**, `tak` ×1,27 → ×1,21,
`collatz` −12 %, `strings_scan_*` −11,5 %). Probado y descartado: meter en una caja los dos `Vec` de
`MakeNamedTuple`, la variante que hace medir 56 bytes a todas, bajaba la instrucción a 32 bytes pero costaba un 1 %
más de instrucciones, con ciclos casi iguales.

**`TailCall` con las piezas de `Call`** (2026-10-17): un `push` fuera de línea por argumento, una asignación con
destructor por registro del marco y otra por argumento eran ~13 % de `tak`. `tak` −8,0 %: ×1,21 → **×1,11** de Python.

**Una edición de un paso sin ruta en el montón** (2026-10-17). `v[i]$~ x`, `d["k"]$~ x` y `d.k$~ x` construían un
array de un elemento para llevar el paso, en cada edición. `SetOne`/`SetOneInPlace` llaman al mismo `vm_deep_set_in`
con el paso prestado. `gap.py`: `arr_update_each` de ×1,9 a **×0,9** de Python; `arr_update_one` de ×3,0 a ~×1,4;
`dict_update_existing` de ×2,5 a ~×1,4. `zyquality/corpus/collections/edicion_un_paso.zy` lo sujeta.

**El recorrido de una colección, en una instrucción** (2026-10-17), como el rango: `IterStep` al final de cada vuelta
en vez de la prueba arriba y el salto de vuelta. `gap.py`: `arr_foreach` ×1,3 → **×0,9**, `str_iter` ×2,1 → ×1,4;
`strings_scan_*` −19 %, `array_update` −16 %.

**`x = expr` sin la copia final** (2026-10-17). La expresión se calculaba en un temporal y un `CopyReg` lo llevaba a
`x`. Ahora, si la última instrucción es la que produce el valor y nada salta detrás de ella —un literal, aritmética, una
comparación, una lectura, una longitud, un corte—, escribe directamente en `x`; sólo instrucciones que leen todos sus
operandos antes de escribir el destino. `collatz` −19 %, `primes_trial` −16,5 %, `mandelbrot` −14,7 %, `nqueens`
−13,6 % (×0,82 de Python), `loop_mod` −10,9 %.

**El hash del diccionario: foldhash** (2026-10-18, por decisión del autor: «la mejor opción»). Hashear la clave era un
cuarto de `d["k"]`. La opción recomendada primero —guardar el hash en el texto, como Python— no servía para el caso
común: una clave de hasta 7 bytes vive dentro de los 8 del `ZyStr`, sin sitio para el hash. foldhash, con semilla
aleatoria por proceso, es el hash por defecto de hashbrown y ya estaba en el `Cargo.lock`: rápido y resistente a claves
elegidas para chocar, que SipHash daba y un FNV no. El orden de un diccionario es el de inserción, así que ninguna
salida depende de la semilla. `gap.py`: `dict_lookup` ×1,3 → **×1,0**, `dict_lookup_last` ×1,2 → ×0,8, `dict_insert`
×1,4 → ×1,2.

### Lo que ya se sabía

| Id | Qué es | Estado |
|---|---|---|
| ZYVM-022 | El valor que devuelve una llamada sobrevive en su registro temporal: la VM tiene dos arrays donde el tree-walker tiene uno (1,79× de memoria, medido en la v0.0.10) | Abierto, con celda `KNOWN` en `zyquality/cost/` |
| GLB-127 | `>> +a` lo rechazan `zytw` y `zyvm` y `zyjs` lo acepta | Corregido el 2026-10-21 en el parser; no era de rendimiento. Destapó GLB-144 y GLB-145 (corregidos el mismo día; GLB-145 por decisión del autor, opción A) |

Lo que se abrió después, el 2026-10-19 —`GLB-139`, `GLB-140`, `ZYJS-068`— y el destino de cada tema abierto están en
«Plan de cierre — lo que queda, por grupos», más abajo.

### El método

Es el de la v0.0.10 y no se relaja:

- Se cuenta con los dos instrumentos del proyecto, nunca con un reloj propio. **`zymbol run --count`**
  —pasos, asignaciones, bytes, copias— es exacto: una corrida basta y un antes/después en esas unidades
  es el que se cita. **`perf stat -e instructions:u`**, mediana de 7, es la unidad del tiempo y la única
  que compara con Python.
- **`--count` no ve un recorrido dentro de una instrucción:** `s$#` sobre un texto largo es un paso.
  PERF-03, PERF-04 y el `s$#` de la VM son ciegos a él; un caso de `cost/` para ellos va por reloj.
- `perf` necesita `kernel.perf_event_paranoid` en 2 o menos, y eso lo cambia el autor; al terminar
  vuelve a 3. **`perf stat` devuelve 0 cuando el programa muere por una señal**: el estado se toma de
  una corrida sin `perf`.
- Se comparan variantes del **mismo** `HEAD`, nunca un binario de hoy contra uno de ayer.
- Una mejora se afirma como **razón** (`zyquality/cost/`), no como valor.
- Un hallazgo, un commit, una ficha. El hallazgo se archiva con su caso en rojo **antes** del arreglo.

Las herramientas están en `zyquality/bench/big/` (ver su `README.md`): `run.py` (16 programas
largos contra un gemelo en Python, suite `bigbench`), `gap.py` (cada operación contra Python a n y
2n) y `scaling.py` (el exponente de crecimiento en tiempo, en pasos y en bytes, y si `--count` puede
sujetar lo que encuentra). Primera corrida en instrucciones, 2026-10-14: 16 de 16 programas y 108 de
108 valores iguales a Python en los dos motores, dispersión 0,00 % en la VM.

### Lo que no es prioridad

El rendimiento del tree-walker, salvo donde comparte el defecto con la VM (PERF-03, PERF-05 a). Y el
de `zymbol.js`: se sustituye en la etapa 4, y optimizar lo que se va a reemplazar es trabajo que no
queda — PERF-03 es la excepción, porque cambia la representación de un valor que los tres motores
tienen que imprimir igual.

---

## Plan de cierre — lo que queda, por grupos (2026-10-19)

El 2026-10-19 se revisó `zyquality/bench/big/HALLAZGOS.md` contra la rama: PERF-01 a 07 están corregidos y PERF-08 es
`GLB-134`. La revisión abrió tres hallazgos (`GLB-139`, `GLB-140`, `ZYJS-068`) y midió un cuarto hecho, el tamaño del
valor del tree-walker. Con lo que ya estaba abierto salieron doce temas, que aquí van agrupados por lo que comparten:
la misma causa, el mismo código o el mismo motor. **El autor aprobó el plan ese día, con la opción recomendada de cada
grupo, y pidió empezar por el grupo 1.** Cada grupo sigue el método de la etapa 2: ficha con su caso en rojo, arreglo,
puerta entera.

| Grupo | Temas | Se implementa | Se cierra como deuda de la v0.0.11 | Estado |
|---|---|---|---|---|
| 1. Leer un texto por posición | `GLB-139`, `ZYTW-016` | Migas en la VM | `GLB-139` en el tree-walker y en zyjs; `ZYTW-016` | **Hecho** (2026-10-19) |
| 2. Editar una colección sin copiarla | `GLB-140`, `ZYJS-068` | `GLB-140` en el tree-walker y en zyjs; `ZYJS-068` | — | **Hecho** (2026-10-19), y dos más que salieron: `ZYTW-018`, `GLB-141` |
| 3. La memoria de los motores | `Value` del tree-walker (120 bytes), `ZYVM-022` | `FunctionValue` y `ErrorValue` en caja; `ZYVM-022` opción A, medida antes | `ZYVM-022` si cuesta más de un 2 % de pasos | **Hecho** (2026-10-19): `ZYTW-019`, y `ZYVM-022` corregida (+0,05 % de pasos) |
| 4. El binario y su distribución | `GLB-134`, D4 | D4; medir musl en el `HEAD` y, si hace falta, `mimalloc` | — | **Hecho** (2026-10-19); quedan los documentos de `web/`, que cambian en la release |
| 5. La puerta y los documentos | D1, `GUIDE.md` § 3b, `LLM.md` | Quitar la frase de § 3b; `LLM.md` a v0.0.11 | D1 se queda como está, documentado | **Hecho** (2026-10-19) |
| 6. El alcance de la versión | Etapas 3 y 4 | Las etapas 3 (3.1 a 3.4) y 4, por decisión del autor del 2026-10-20 | — (el 2026-10-19 eran 3.1, 3.3, 3.4 y la etapa 4, a la v0.0.12) | En curso, desde 3.2 |

### Grupo 1 — Leer un texto por posición

Una posición de Zymbol cuenta caracteres y el texto se guarda en UTF-8: llegar a `s[i]` exigía caminar desde el
principio, y la VM sólo se lo ahorraba con un texto ASCII (`ZYVM-028`). Dentro de un bucle, cuadrático.

```zymbol
s = ""
@ _i:1..128000 { s = s "ñ" }
k = 0
@ i:1..128000 { ? s[i] == 'ñ' { k = k + 1 } }
>> k ¶
```

El control es el mismo programa con `"a"`: plano en los dos motores de Rust. Con «ñ», antes, la VM ×13 de 32 000 a
128 000, el tree-walker ×13 y zyjs ×15 (que recorre también un texto ASCII). Python indexa en O(1).

- **VM, opción A, migas — elegida y hecha.** Un texto del montón que no es ASCII guarda, la primera vez que se le pide
  una posición, el byte donde empieza cada carácter 64; `s[i]` y un corte saltan a la miga anterior. 128 000 «ñ»:
  941 → 39 ms. Descartadas: recordar la última posición (deja cuadrático el acceso hacia atrás o al azar) y un texto de
  ancho fijo como el de Python (rehace `ZyStr`).
- **Tree-walker — deuda.** Arreglarlo pide que su texto deje de ser un `String` que se copia en cada lectura
  (`ZYTW-016`); llevar `ZyStr` a `zymbol-common` cerraría los dos, y es el cambio más grande de esta lista.
- **zyjs — deuda.** Lo pequeño sería guardar `[...v]` en el valor la primera vez (zyjs nunca cambia un texto en el
  sitio); el motor se sustituye en la etapa 4.

Lo sujetan `growth/index-a-text` y `growth/slice-a-text` (este, por reloj otra vez en la VM) de `zyquality/cost/`, y
`zyquality/corpus/strings/34_position_crumbs.zy`.

### Grupo 2 — Editar una colección sin copiarla

La regla es «escribo en el sitio si el valor es sólo mío», y tiene dos huecos. `GLB-140`: una edición de dos niveles
copia el array exterior entero en el tree-walker y en zyjs.

```zymbol
fila = [0, 0, 0]
m = []
@ _i:1..400 { m$+ fila }
@ k:1..4000 {
    r = (k % 400) + 1
    m[r>2]$~ k
}
>> m[1>2] ¶
```

El control, `m[r]$~ k` sobre un array de números, copia 0 en los tres. Con dos niveles el tree-walker copia 1 612 000
valores (las 400 filas en cada edición) y zyjs pasa de 61 a 651 ms de 2 000 a 32 000 filas; la VM copia 3. Es la
forma de editar una matriz, y GoL y GO corren en el playground sobre zyjs.

`ZYJS-068`: con un literal como operando, zyjs copia el receptor.

```zymbol
m = []
@ i:1..40000 { m$+ [i, 0, 0] }
>> m$# ¶
```

Con la fila en una variable (`m$+ fila`), 211 ms a 20 000; con el literal, 2 123 ms a 20 000 y 9 818 a 40 000.

- **Tree-walker:** la edición profunda por `edit_receiver_done`, como la de un nivel: la raíz sale de su variable,
  se baja con `&mut` y `make_mut` nivel a nivel, y vuelve; el mismo `receiver_read_first` conserva el orden de
  `GLB-115`.
- **zyjs:** el `own` de `ZYJS-014`, nivel a nivel en la ruta; y en `plainOperand`, un literal de colección cuyos
  elementos sean todos `plainOperand` (un literal no ejecuta código del programa).

**Hecho el 2026-10-19.** El tree-walker escribe la ruta en el sitio: 4 000 ediciones sobre 400 filas copiaban 1 612 000
valores y ahora 0, también a través de `<~`. zyjs escribe en el sitio el nivel de fuera y copia sólo la fila que escribe
—marcar también las filas dejaría que una leída antes con `r = m[2]` viera la escritura—: `growth/update-nested`
384 → 5 916 ms antes, 275 → 474 ms ahora. `ZYJS-068`: 312 → 6 634 ms antes, 171 → 317 ms ahora. Comparar el tree-walker
de antes con el de después sacó dos divergencias que la puerta no veía, corregidas el mismo día: `ZYTW-018` (el
tree-walker abortaba al añadir una clave en el último paso de una ruta, que la VM y zyjs añaden) y `GLB-141` (la VM y
zyjs leían una clave ausente a medio camino como `Unit`, donde `COLLECTIONS.md` dice `##Key`). Lo sujetan
`growth/update-nested` y `growth/append-rows`, sin deuda en ningún motor, y
`zyquality/corpus/collections/edicion_profunda_en_el_sitio.zy`.

### Grupo 3 — La memoria de los motores

El `Value` del tree-walker mide **120 bytes** y el de la VM, 16: `Function(FunctionValue)` va en línea y mide 120
(`ErrorValue`, 48; `String`, 24).

```zymbol
v = []
@ i:1..40000 { v$+ i }
@ _i:1..40000 { v$-[1] }
>> v$# ¶
```

Quitar el primero desplaza los que quedan en cualquier lenguaje: Python `pop(0)` 87 ms, la VM 189, el tree-walker
3 141 (mueve 7,5 veces más memoria). El control, `v$-[v$#]`, ~12 y ~37 ms. Un array de 2 000 000 de enteros: 235,9 MiB
en el tree-walker, 37,7 en la VM.

- **`Value` del tree-walker:** `FunctionValue` y `ErrorValue` en caja; unos 32 bytes, por medir, y lo que cueste una
  reserva más al crear una función como valor lo dirá `closure_create_call`.
- **`ZYVM-022`** (el resultado de una llamada sobrevive en su temporal; ×1,82 de memoria en `autofree/call-result`):
  liberar sólo los temporales que pueden ser una colección, con el tipo por registro que el compilador ya lleva. Se
  mide antes con `--count` en `bench/big`; si cuesta más de un 2 % de pasos, se cierra como deuda escribiendo la razón
  en `alloc_temp`.

**Hecho el 2026-10-19.** El `Value` del tree-walker mide **24 bytes**, no los 32 estimados: con las dos variantes en caja
la más grande es un `String`, y la etiqueta cabe en un hueco de su capacidad (`ZYTW-019`). Un array de 2 000 000 de
enteros, 236,1 → 53,3 MiB; quitar 40 000 por el principio, ~2 640 → ~313 ms; crear un cierre o un error cuesta una
reserva más (+1,6 % y ~+5 % con ruido). `ZYVM-022`, en tres variantes medidas contra el mismo `HEAD`: limpiar todo
temporal de tipo desconocido costaba +18,7 % de pasos; sólo los que recibieron una llamada, +0,45 % con un pico de
+6,4 %; y sin los que una instrucción aritmética mostró numéricos, **+0,05 %** con un pico de +0,8 %, que es lo que
quedó. `autofree/call-result` pasa en la VM: 68 → 39 MiB. Por el camino, `zyddt findings` no veía una cabecera que
nombraba abierto un hallazgo cerrado cuando la frase iba en negrita o tras una coma, y la de `zyvm.md` llevaba así desde
el arreglo: se amplió el patrón, con su caso en la autoprueba.

### Grupo 4 — El binario y su distribución

- **`GLB-134`:** medir el estático en el `HEAD` (el target musl se instala con `rustup`), porque esta rama ya no copia
  textos enteros; si algo queda por encima de ×1,5, `mimalloc` sólo para ese target, debajo del `CountingAllocator`
  para que `--count` siga contando.
- **D4:** `default = []` en `zymbol-cli`, `std/db` con `--features db`, dicho en el README y en `install.md`; los
  trabajos de la puerta que prueban `STD_DB` compilan con la feature.

**Hecho el 2026-10-19.** D4: `default = []` en los **cuatro** crates que tenían `db` por defecto —`zymbol-cli`,
`zymbol-vm`, `zymbol-interpreter`, `zymbol-compiler`—, porque `cargo build` en la raíz compila cada miembro con sus
defectos y Cargo une las features: con sólo el del CLI, `db` seguía encendido. Sin la feature: 281 paquetes en vez de 286,
11,76 MB en vez de 11,96, y ninguna biblioteca de ODBC para arrancar. La nombran ahora la puerta (`gate.yml`, en la
compilación y en `cargo test`), la release de Windows —donde ODBC es del sistema y el binario la llevaba—, el `.exe` de
`build-packages.sh`, `bench/run_all.sh` y `coverage/run.sh`; y lo dicen el README, `REFERENCE.md` L17,
`packaging/verify/README.md`, el `README-odbc.md` del corpus y el `CLAUDE.md` de la raíz. Sin la feature, `<# std/db`
responde `module not found: std/db`, así que un binario sin ella hace **fallar** los ficheros `STD_DB` de la puerta en vez
de saltarlos. La trampa que queda: `cargo test --release` sin `--features db` reescribe `target/release/zymbol` sin ella
(dicho en `CLAUDE.md`). `web/install.md` y su gemelo describen el binario publicado y cambian en la release.

`GLB-134`: el estático del `HEAD` iba a ×2,1 de glibc en `s = s "x"` (×17 en la v0.0.10) y a ×1,05 en los 16 programas
largos. Con `mimalloc` sólo para musl, debajo del `CountingAllocator` —`--count` da lo mismo—, y tres opciones fijadas
al arrancar para que no retenga memoria, va a ×0,983 en los 16 programas y como glibc en cada operación; el pico de
memoria queda de 1 a 6 MiB por encima en los programas que hacen crecer colecciones. La ficha tiene la tabla y por qué
20 ms de purga y no 0. El target musl se instaló con `rustup`; la CI compila el C con `musl-gcc`.

### Grupo 5 — La puerta y los documentos

- **D1:** `zyq expect` compara hoy los goldens **sólo con `zytw`**; la VM, el motor que ejecuta la gente, queda
  verificada sólo de forma indirecta, a través del consenso. Los goldens los sigue grabando `zytw` —lo nuevo se
  construye primero en el tree-walker—, y `zyq expect` los compara con `zytw` y `zyvm`.
  **Corregido el 2026-10-19:** `zyquality/corpus.toml` ya decía por qué no —comparar un segundo motor con un golden
  «answers the consensus question in a noisier way»— y la vía indirecta es completa: el consenso obliga a `zyvm` a
  coincidir con `zytw` en cada fichero, y `expect` obliga a `zytw` a coincidir con el golden. El autor decidió dejarlo
  como está y escribirlo; `corpus.toml` lo dice ahora junto a la razón que ya tenía.
- **`GUIDE.md` § 3b** dice que algunas primitivas de TUI son sólo del tree-walker y remite a unas notas por primitiva
  que no existen; la suite `tui` ya obliga a que los dos motores coincidan. Se quita la frase.
- **`LLM.md`** sigue titulado v0.0.10, y la suite `llm` lo marca.

**Hecho el 2026-10-19.** § 3b dice ahora lo comprobado: las siete primitivas (`@~`, `>>!`, `>>?`, `>>~`, `<<|`, `<<|?`,
`>>|`) dan la misma salida, byte a byte, en los dos motores con `--keys`, y la suite `tui` los sujeta. `LLM.md` se titula
v0.0.11 y dice que pasa contra `zymbol 0.0.11`: la suite `llm` da 89 de 89 sin `STALE TITLE`.

### Grupo 6 — El alcance de la versión

La v0.0.11 se cierra con las etapas 1 y 2, los grupos 1 a 5 y la **medición** de 3.2 (compilar sin LSP, REPL, HTTP ni
`db`, y medir peso, memoria al compilar y arranque), que contesta D6. Las evaluaciones 3.1, 3.3 y 3.4 y la etapa 4
pasan a la v0.0.12 tal como están escritas aquí.

**Cambiado por el autor el 2026-10-20:** las etapas 3 y 4 enteras entran en la v0.0.11, que sale el 2026-11-01 (como tarde
el 2026-11-11). El orden de trabajo es 3.2, 3.1, 3.3, 3.4 y la etapa 4. Cada evaluación de la etapa 3 sigue terminando en
una decisión del autor —implementar, aplazar o desestimar— y nada se construye sin ella. Si el calendario no alcanza, lo que
se deja fuera se le propone al autor; no se recorta por cuenta propia.

---

## Etapa 3 — Cuatro evaluaciones

Cada una responde a una pregunta con una medición y termina en una decisión del autor:
**implementar**, **aplazar** o **desestimar**. Ninguna se construye sin esa decisión.

### 3.1 Separar Zymbol en componentes

**La pregunta:** ¿se puede partir el intérprete en un núcleo sin dependencias del sistema, un
anfitrión y unas herramientas, y cuánto cuesta?

| Capa | Qué contiene | Dependencias que arrastra hoy |
|---|---|---|
| Núcleo | Lexer, parser, semántico, compilador, bytecode, VM | Ninguna que deba ser del sistema |
| Anfitrión | Terminal, red, base de datos, shell, ficheros, reloj | `crossterm`, `ureq`, `odbc-api` |
| Herramientas | LSP, REPL, formateador, `.zyp`, `build` | `tower-lsp`, `tokio`, `arboard` |

La frontera del anfitrión ya está medio dibujada: los builtins nativos llevan un id numérico por
familia en `zymbol-bytecode` — matemáticas (0), azar (100), JSON (200), E/S (300), red (400),
base de datos (500), terminal (600), tiempo (700). Hoy `zymbol-vm` llama a `crossterm`, `ureq` y
`odbc-api` directamente; separar es poner esas llamadas detrás de una interfaz.

Es el requisito de 3.2 y de 3.4, y define el contrato que tendrá que cumplir la VM del navegador.

#### Evaluación — 2026-10-20

Medido sobre `17bfdb8` con `cargo check --target wasm32-unknown-unknown`: span, common, lexer, ast, parser, semantic,
bytecode, intrinsics y compiler **ya compilan**; `zymbol-vm` y `zymbol-interpreter` fallan dentro de `crossterm`, y sólo
ahí —`ureq` y `odbc-api` ya no entran sin features desde 3.2—. rustc se detiene en ese crate, así que el código propio de
la VM no quedó comprobado. Dos cosas más compilan para wasm pero fallan al ejecutarse: `SystemTime::now` aborta y `std::fs`
responde «unsupported».

La VM ya tiene dos terminales —la real, con `crossterm`, y la virtual de `--keys` (`vscreen::Headless`)—, elegidas con 19
guardas `if self.headless` en siete instrucciones (`ClearScreen`, `QueryTerminalSize`, `ReadKey`, `ReadLine`, `PrintAt`,
`EnterTui`, `ExitTui`). La salida de `>>` ya llega inyectada (`VM<W: Write>`).

**Decidido por el autor el 2026-10-20: forma B.** Una interfaz `Host` sólo para la VM —terminal, reloj, red y base de
datos— con dos implementaciones: la nativa (`crossterm`, `ureq`, `odbc-api`) y la virtual de `--keys`, que sustituye las
guardas. El tree-walker no cambia: no es parte del núcleo de la tabla de arriba, ni va a WebAssembly. `>>` sigue fuera de
la interfaz, para no tocar el camino caliente. Se verifica contra `17bfdb8` con la puerta entera, la suite `tui`, los pasos
de `--count`, `ab.py` alternado y `ZYMBOL_POISON_TEMPS=1`, y la CI pasa a comprobar `cargo check -p zymbol-vm --target
wasm32-unknown-unknown` (el primer paso de 3.4). Descartadas: la misma interfaz en los dos motores con la implementación
compartida (A: el doble de trabajo, y el consenso dejaría de ver un fallo del anfitrión que los dos compartieran) y una
feature `term` sin interfaz (C: el motor de 3.4 se quedaría sin TUI ni entrada).

**Hecho el 2026-10-20** (`interpreter` `11f5c5d` y `b0ba2d0`, `zyquality` `0a8c0ad`). `zymbol_vm::host` tiene dos
interfaces y no una, porque `--keys` cambia la terminal y nada más: `Terminal` (las siete primitivas, `@~`, `<<`) y `Host`
(el reloj, la semilla de `std/random`, `std/net`, `std/db`). `zymbol-vm` ya no depende de `crossterm`, `ureq` ni ODBC: sus
implementaciones nativas se movieron sin cambios al crate nuevo `zymbol-host` (`NativeTerminal`, `NativeHost`, con `http` y
`db` como features). La de `--keys` es `host::Headless` y sustituye las 19 guardas; una VM a la que nadie da otra tiene
`BasicTerminal` y `BasicHost`. El reloj de `std/time` pasa por `zymbol_intrinsics::time::Clock`, con las funciones de
siempre para el tree-walker y variantes `_with` para la VM. Contra `11f5c5d`, los dos binarios con `-p zymbol-cli --features
db`: el corpus da el mismo código de salida, la misma salida y los mismos pasos, llamadas y copias en los dos motores —salvo
los dos ficheros ENVIRONMENT, que cambian de una corrida a otra—, con una asignación de 24 bytes más en la VM (la terminal
nativa en su caja); `ab.py`, mismos pasos y CPU de la VM entre ×0,89 y ×1,05. Salió de paso `ZYVM-031`: el ejecutable de
`zymbol build` había perdido `std/net` y `std/db` con D4 y con 3.2, y se corrigió antes (`7dd33bb`).

**Lo que encontró para 3.4:** con la terminal fuera, `cargo check -p zymbol-vm --target wasm32-unknown-unknown` sólo falla
en una aserción de `ZyStr`, «ZyStr requires a 64-bit platform». Con ella relajada a 32 bits —en una prueba que no se
guardó— la VM entera compila para wasm32. La comprobación en CI que pedía B espera a esa decisión, que es de 3.4.

### 3.2 Un motor ultracompacto

**La pregunta:** ¿cuánto pesa un `zymbol` sin LSP, sin REPL, sin `std/db` y sin HTTP, y qué pierde?

Para contenedores pequeños —el caso que lo motivó son los de 4 GB— donde el LSP y el portapapeles
son peso muerto. Lo que hay que medir sobre un binario así: pico de memoria al compilar, tamaño,
número de crates y tiempo de arranque, contra la tabla de «Peso» de arriba.

Dos formas posibles, y la evaluación tiene que elegir: *features* apagables en el mismo crate
(`lsp`, `repl`, `http`, `db`) o un segundo binario. La primera es menos trabajo; la segunda da un
nombre a lo que se instala.

El paso más barato no necesita esperar a la evaluación: sacar `db` de las features por defecto
(decisión D4).

**Falta un dato:** qué falló exactamente en el contenedor de 4 GB. Si fue memoria al compilar, el
arreglo es el perfil de LTO y sacar LSP y portapapeles; si fue el enlazado por `-lodbc` o el
arranque del binario, es D4. Ver D6.

#### Medición — 2026-10-20

Sobre `6666f8f`, rustc 1.99.1. Un prototipo desechable (fuera de la rama, en `target/w32/`) puso `lsp`, `repl` y `http`
como features del CLI y `http` en la VM y el tree-walker; `crossterm` se queda, porque la TUI es parte del lenguaje. Cada
cifra sale de una compilación limpia, `cargo build --release --offline -p zymbol-cli`, dentro de un cgroup
(`systemd-run --user --scope -p MemoryMax=… -p MemorySwapMax=0`): la memoria es el `memory.peak` del cgroup —lo que ve el
límite de un contenedor— y, aparte, la suma del RSS de cada proceso muestreada cada 100 ms, que es lo que medía la tabla
de «Peso» y cuenta dos veces las páginas compartidas. El programa de control es la misma orden en `HEAD` sin features.

| Medida | 2026-10-13 | `HEAD` | `HEAD --features db` | Compacto |
|---|---|---|---|---|
| Crates (normales y de build, `cargo tree`) | 208 | 210 | 214 | 100 |
| Pico de memoria del cgroup, 16 trabajos | — | 2 731 MB | 2 778 MB | 1 601 MB |
| Suma de RSS de los procesos, 16 trabajos | 3 850 MB | 3 970 MB | 4 220 MB | 2 716 MB |
| Enlazado final con LTO | 1 342 MB | 1 344 MB | 1 366 MB | 870 MB |
| Compilar `zymbol_vm` | 798–890 MB | 799–855 MB | 949 MB | 763–923 MB |
| Binario | 12,76 MB | 12,32 MB | 12,54 MB | 6,60 MB |
| Compilación limpia, 16 núcleos | 3 min 23 s | 3 min 02 s | 3 min 05 s | 1 min 48 s |
| Arranque de un programa de una línea, mediana de 300, alternados | — | 6,3 ms | 6,8 ms | 5,9 ms |
| Pico de RSS de ese arranque (`wait4`) | 11,0 MB | 7,8 MB | 7,8 MB | 7,8 MB |
| `target/` al terminar | — | 549 MB | — | 203 MB |

El compacto con las tres features encendidas da un binario idéntico al de `HEAD`, byte a byte en tamaño (12 321 240): las
features no cambian nada cuando están puestas. El pico de RSS del arranque de 2026-10-13 se midió con otro instrumento.

**D6, en un cgroup de 4 GB sin swap:**

| Configuración | Termina | Tiempo | CPU total | Pico del cgroup |
|---|---|---|---|---|
| `HEAD`, LTO completo, 16 núcleos | sí | 3 min 03 s | 562 s | 2 684 MB |
| `HEAD`, LTO completo, 4 trabajos | sí | 3 min 18 s | — | 2 225 MB |
| `HEAD`, LTO completo, 2 núcleos (`taskset`) | sí | 4 min 29 s | 391 s | 2 228 MB |
| `HEAD`, sin LTO, 2 núcleos | sí | 4 min 16 s | 411 s | 1 764 MB |
| `HEAD`, LTO `thin`, 16 núcleos | sí | 2 min 32 s | — | 2 671 MB |
| Compacto, 2 núcleos | sí | 2 min 18 s | 180 s | 1 296 MB |

**La memoria al compilar no reproduce D6:** el perfil de release con LTO completo cabe en 4 GB con 2 núcleos y deja 1,8 GB
libres. Quitar el LTO tampoco es el arreglo que apuntaba la pista: con 16 trabajos sube el pico de la fase en paralelo, y
con 2 núcleos ahorra un 5 % del tiempo. ZyBigBench sólo dejó escrito que el perfil «no termina en un contenedor de 4 GB»,
con otra toolchain, sobre la v0.0.10. Quedan dos explicaciones que desde aquí no se pueden comprobar: un límite de tiempo
por orden en un contenedor de pocos núcleos lentos (con uno solo, 6,5 minutos de CPU), o memoria que ya ocupaba el resto
del contenedor —un `target/` en RAM son 549 MB más—. El compacto ataca las dos: −54 % de CPU y −42 % de memoria con 2
núcleos.

Lo que pierde el compacto: `zymbol lsp`, `zymbol repl`, `std/net` y `std/db`. De la puerta, sólo
`stdlib/stdlib_net_type_err.zy` (`i18n/test_http_api.zy` ya está excluido como ENVIRONMENT).

**Decidido por el autor el 2026-10-20: implementar la forma A.** Features `lsp`, `repl` y `http` en `zymbol-cli`,
encendidas por defecto, de modo que `cargo build --release` da lo mismo que hoy; el compacto es
`cargo build --release -p zymbol-cli --no-default-features`. Sin `http`, `<# std/net` responde `module not found:
std/net` en los dos motores, como `std/db` desde D4. La CI comprueba que el compacto compila. Las formas descartadas: un
segundo binario (B: `main.rs` tendría que pasar a una biblioteca, `cargo build` en la raíz seguiría compilando todo el
workspace, y la extensión dejó el binario aparte por el «unified binary» de su v0.1.1) y el compacto por defecto (C: quien
compila desde el README perdería el LSP y la extensión).

### 3.3 `.zyb` — Zymbol Binary

**La pregunta:** ¿merece la pena un formato de bytecode en disco, al estilo del `.class` de la
JVM, y qué habría que garantizar?

Tiene precedente en el repo: `zymbol-standalone` ya serializa el `CompiledProgram` con `bincode`
para incrustarlo en el ejecutable que produce `zymbol build`. Un `.zyb` sería eso mismo en un
fichero, con una cabecera.

Lo que la evaluación tiene que contestar:

- **Estabilidad.** `bincode` sobre un `enum` de 167 variantes no es un formato estable: añadir
  una instrucción renumera las demás. Hace falta una versión en la cabecera y una regla para el
  desajuste. La más simple es rechazar un `.zyb` de otra versión.
- **Qué ahorra.** Cuánto del arranque es lexer, parser, semántico y compilador. Sin medirlo no se
  sabe si un `.zyb` arranca antes de forma apreciable.
- **Qué permite.** Un motor que sólo ejecuta `.zyb` no necesita lexer, parser, semántico ni
  compilador: sería el compacto de 3.2 llevado al extremo, y lo más parecido a una JVM.
- **Qué rompe.** `</ fichero.zy />` ejecuta otro programa en tiempo de ejecución y hoy lo compila
  al vuelo; con sólo `.zyb` eso tendría que ir precompilado dentro.
- **Su relación con `.zyp`.** Un `.zyp` es fuente a propósito. Un `.zyb` dentro de un `.zyp`, o en
  vez de él, cambia lo que el paquete promete.

Sirve además para otra cosa: un `.zyb` generado por Rust es el oráculo de la VM del navegador
(mismo bytecode, otro ejecutor; cualquier diferencia es un fallo del ejecutor).

#### Evaluación — 2026-10-20

El ejecutable de `zymbol build --release` ya es un `.zyb` sin fichero: lleva el `CompiledProgram` en `bincode` y arranca
sin lexer, parser, semántico ni compilador. Lo que un `.zyb` ahorraría es, como mucho, lo que separa `zymbol run x.zy` de
ese ejecutable. Medido sobre `c64c8cd`, alternados, CPU, mediana de 60 rondas; los siete dan la misma salida de las dos
maneras:

| Programa | Bytecode | `zymbol run` | Ejecutable | Ahorro |
|---|---|---|---|---|
| Programa de una línea | 148 B | 4,7 ms | 3,6 ms | 1,1 ms |
| `GO/試験/api試験` | 23 KB | 13,3 | 4,0 | 9,3 |
| `GO/試験/文字試験` | 9 KB | 8,5 | 4,1 | 4,5 |
| `GO/試験/盤試験` | 24 KB | 17,2 | 4,2 | 12,9 |
| `Chaturanga/परीक्षा/भाषापरीक्षा` | 66 KB | 19,9 | 4,9 | 15,0 |
| `Chaturanga/परीक्षा/नियमपरीक्षा` | 53 KB | 19,5 | 7,0 | 12,6 |
| `GO/試験/思考試験` (con cálculo) | 63 KB | 49,0 | 38,9 | 10,1 |

A esta escala la máquina varía mucho (`盤試験` dio 10 ms en una corrida suelta y 15,6–18,9 entre cuartiles en esta). Un
medidor por fases en un mismo proceso suma unos 5,6 ms de front-end en frío para `盤試験`; el resto de la diferencia —parte
es el tamaño del binario, 1,1 ms en el programa de una línea— no está localizado.

Al medir salieron dos hallazgos: **`GLB-142`** (una capa de reexportación hacía parsear su módulo de origen una vez por
nombre; `api試験` pasó de 51 a 13 ms en `zymbol run`, corregido en `c64c8cd`) y **`GLB-143`** (`zymbol check` parsea un
módulo una vez por importador). Las preguntas de arriba: el formato necesitaría versión en la cabecera y rechazar otra;
`</ fichero.zy />` seguiría compilando al vuelo, con el `.zy` al lado; `.zyp` no cambia; y como oráculo de la etapa 4 no
hace falta mientras esté en pausa, y un motor wasm puede llevar el compilador, que ya compila a wasm.

**Decidido por el autor el 2026-10-20: se aplaza**, ni ahora ni a la v0.0.12: el autor la retoma, quizá, el viernes
2026-10-23. Las opciones que quedan sobre la mesa: un `.zyb` explícito (`zymbol compile`, ~1,5 días), una caché automática
al estilo de `__pycache__` (~2–3 días) o desestimarlo.

### 3.4 WebAssembly

**La pregunta:** ¿qué cuesta y qué da un motor en WebAssembly, como opción, para sistemas web que
lo necesiten por escala?

**El principio no se evalúa:** el motor del navegador por defecto es JavaScript y lo seguirá
siendo. Zymbol tiene que ejecutarse fácil y rápido en cualquier navegador, sea cual sea su
capacidad, porque quien más probablemente lo adopte tiene menos recursos, menos infraestructura y
una barrera de idioma — y esa es la promesa del lenguaje. Un WebAssembly sería un motor fuerte
**opcional**, nunca un requisito.

Lo que hay que medir:

- **Tamaño** del `.wasm` del núcleo, contra `zymbol.js` (625 KB; 181 KB comprimido).
- **Velocidad** contra la VM nativa y contra la VM de JavaScript de la etapa 4.
- **Qué queda fuera**: entrada interactiva (`<<`, `<<|`), TUI, `std/db`, shell.

Depende de 3.1. El primer paso, barato y útil aunque nunca se publique nada, es comprobar en CI
que el núcleo compila a `wasm32-unknown-unknown`: es la prueba de que no tiene dependencias del
sistema escondidas.

#### Evaluación — 2026-10-20

Un prototipo desechable (fuera de la rama, en `target/w32/src-wasm`): `ZyStr` con su aserción de 64 bits relajada —lo
único del núcleo que impedía compilar, ver 3.1—, un crate `cdylib` con dos exportaciones escritas a mano, sin
`wasm-bindgen` (lexer → parser → compilador → VM, el reloj importado de JavaScript), y Node 24 como ejecutor. Los 16
programas de `zyquality/bench/big/` dan en wasm exactamente la misma salida que la VM nativa.

| | Sin comprimir | `gzip -9` |
|---|---|---|
| `zymbol.js` | 636 KB | 185 KB |
| wasm, `opt-level = 3` | 1 809 KB | 474 KB |
| wasm, `opt-level = "z"` | 1 066 KB | 319 KB |

Tiempo transcurrido del proceso, arranque de Node incluido, mediana de 5, media geométrica de los 16: `zymbol.js` va a ×61
de la VM nativa; el wasm, sólo con Liftoff (el compilador rápido de V8), a ×3,2, unas 19 veces más rápido que `zymbol.js`.
En `fib_rec`: 0,98 s nativo, 1,9 s wasm, 70 s `zymbol.js`; con TurboFan, ya compilado, ~1,3 s. Pero compilar con TurboFan
el módulo de `opt-level = 3` cuesta ~3,7 s de CPU (~1,4 s el de `"z"`): Node lo espera al salir, y un navegador lo hace en
otro hilo, más despacio en un equipo modesto — una razón más para que sea opcional.

Fuera, o por construir: la entrada `<<`, la TUI (una `Terminal` en JavaScript), los imports (un resolvedor desde
JavaScript, como el del playground), `std/net` (`Host::http` es síncrono), `std/db`, el shell, `</ />`, la recursión
profunda (la pila de wasm es de 1 MiB y se fija al enlazar) y el análisis semántico, que compila pero el prototipo no corre.

**Decidido por el autor el 2026-10-20: forma A.** `ZyStr` pasa a ser portable a 32 bits —su texto ya guarda el puntero
como `u64`, y en 32 bits el byte 7 queda a cero y no choca con la marca de texto en línea—, y la CI comprueba que la VM
compila a `wasm32-unknown-unknown`. No se publica ningún motor. Por qué 32 bits: es el WebAssembly que ejecutan los
navegadores y Node, y el único que Rust estable compila (`wasm64-unknown-unknown` es de nivel 3, sólo con nightly y
recompilando `std`). Las pruebas sobre `wasm64` quedan para después, por decisión del autor. Descartadas por ahora: un
motor provisional `zywasm` en el consenso (B, ~2–3 días) y aplazar.

**Hecho el 2026-10-20** (`interpreter` `dd311b1`, `zyquality` `3f9bbd5`). La aserción de `ZyStr` admite punteros de 4
bytes, su comentario dice por qué el byte 7 no choca, y la prueba de tamaños habla en palabras (8 y 40 en 64 bits, 4 y 20
en wasm32). Ejecutadas en `wasm32-wasip1` con el WASI de Node, pasan las pruebas del núcleo —vm 14, intrinsics 16, lexer
191, parser 58, common 37, semantic 75—, salvo las que lanzan un proceso o escriben un fichero temporal, que allí no pueden
correr. Con el código de la rama, el prototipo de medición (que sigue fuera de ella, en `target/w32/proto-main`) da en
wasm32 la salida de la VM nativa en los 16 programas. `gate.yml` comprueba `cargo check -p zymbol-compiler -p zymbol-vm
--target wasm32-unknown-unknown`.

---

## Etapa 4 — La VM del navegador (`zyvm.js`)

`zymbol.js` es un tree-walker `async` de 12 714 líneas en un solo fichero, y en `fib(30)` va 57×
por detrás de la VM de Rust. Con la VM como motor por defecto, el navegador debería reflejar ese
motor y no el que deja de serlo.

Un prototipo de juguete del 2026-10-13 —el mismo `fib(30)`, bytecode escrito a mano— dio 113 ms
para un bucle síncrono de VM de registros en JavaScript, con 14 808 951 pasos contra los
14 808 956 que cuenta `zymbol run --vm --count`. Es un techo y no una promesa: le faltan
comprobaciones de tipo, posiciones de error y builtins, y no dice nada de agregados.

El plan, en cinco pasos:

1. **Sonda.** Una VM mínima en JavaScript que ejecute un `.zyb` real, con `fib` y una carga de
   agregados. Decide si se sigue.
2. **Partir el monolito** sin cambiar comportamiento: lexer, parser y comprobador a ficheros
   propios, con `zymbol.js` como fachada que reexporta.
3. **La VM completa** sobre bytecode compilado por Rust, como cuarto motor provisional en
   `engines.toml`, hasta cero divergencias contra `zyvm`.
4. **El compilador**, sobre el AST de JavaScript, con el bytecode de Rust como oráculo.
5. **El cambio:** el playground usa la VM y se borra el intérprete antiguo.

Módulos ES en JavaScript, no TypeScript: `web/` no tiene paso de build y GitHub Pages sirve las
fuentes tal cual.

**En pausa desde el 2026-10-20, entera** (decisión del autor, D5): primero los pasos 3 a 5 y, el mismo día, también la
sonda y la partición del monolito. Todo depende de cómo salga 3.4: si el motor de WebAssembly sale bien, no haría falta una
VM nueva en JavaScript.

Tres riesgos que hay que resolver en el paso 1:

- **Suspensión dentro de lambdas.** La VM de Rust reentra su bucle para las lambdas de `$>`, `$|`
  y `$<`; un bucle síncrono en JavaScript no puede pausar ahí para un `<<`.
- **Copia al escribir sin contador de referencias.** Rust decide con el contador del `Rc`;
  JavaScript no tiene uno.
- **Int contra Float.** Un `number` no distingue `1` de `1.0`.

---

## Cómo dependen las etapas

- La 2 necesita la 1: se optimiza el motor que se ejecuta.
- 3.2 y 3.4 necesitan 3.1.
- La 4 necesita 3.3, al menos el volcado del bytecode.
- 3.1 y 3.3 no dependen entre sí y pueden ir en cualquier orden.

---

## Decisiones abiertas

| Id | La pregunta | Recomendación |
|---|---|---|
| D1 | ¿Qué motor graba los goldens? | **Decidida por el autor el 2026-10-19 (grupo 5):** se queda como está. Los graba `zytw`, porque lo nuevo se construye primero en el tree-walker, y `zyq expect` los compara con el motor que los grabó, como dice `zyquality/corpus.toml`: la VM queda comprobada contra cada golden por el consenso, que la obliga a coincidir con `zytw`, más `expect`. `zyq expect --engines zyvm` da 677 de 677. Una primera versión de esta fila proponía añadir `zyvm` a `expect` sin contar con esa razón escrita |
| D2 | ¿Qué hace el REPL? | Se queda en el tree-walker en esta versión, y se dice. Portarlo es diseño, no un flag |
| D3 | ¿Qué pasa con `--vm`? | Se conserva y se acepta sin efecto: hay scripts que lo escriben |
| D4 | ¿`std/db` sale de las features por defecto? | **Decidida por el autor el 2026-10-19 (grupo 4):** sí. Los releases de Linux y macOS ya se construyen sin ella |
| D5 | ¿Dónde va la etapa 4? | **Decidida por el autor el 2026-10-20:** en la v0.0.11, después de la etapa 3. Deroga lo decidido el 2026-10-19 (grupo 6), que la pasaba a la v0.0.12 porque la etapa 2 la había desplazado. **El mismo día, al ver el calendario,** el autor dejó **en pausa la etapa 4 entera** —primero los pasos 3 a 5, después también la sonda y la partición del monolito—: si el motor de WebAssembly de 3.4 sale bien, no haría falta una VM nueva en JavaScript |
| D6 | ¿Qué falló en el contenedor de 4 GB? | No es una decisión sino un dato que falta, y cambia el primer arreglo de 3.2. ZyBigBench, hecho en ese contenedor, apunta a una pista: su `README` dice que el perfil de release con LTO no terminaba allí. **Contestada en parte por la medición de 3.2 (2026-10-20):** no fue la memoria al compilar —en `HEAD` el perfil con LTO cabe en 4 GB con 2 núcleos, con 2,2 GB de pico—; quedan un límite de tiempo por orden o memoria ocupada por otra cosa, que no se pueden comprobar desde aquí. El compacto reduce las dos |
| D7 | PERF-07: el error que sustituye al aborto, ¿se captura con `!?`, y de qué familia es? | **Decidida por el autor el 2026-10-16:** se captura con `!?`; su familia es una nueva, `##Depth`; el límite es de 10 000 llamadas activas, el mismo número en los tres motores, con la pila del programa en 512 MiB y un seguro que mira la pila que queda |
| D8 | PERF-05 b: ¿la cadena guarda si es ASCII, o cuántos caracteres tiene? | **Decidida por el autor el 2026-10-17:** entra en la etapa 2, con Python como oráculo. Hecho como `ZYVM-028`: el texto guarda su número de caracteres, calculado la primera vez; de ahí sale si es ASCII |

---

## Antes de dar algo por hecho

Lo que pide `zymbol-design/HOW_TO_CHANGE_ZYMBOL.md` § 5, que vale para cada etapa:

- `zyddt premises`, `zyddt axis` y `zyq suite`. Una suite que no pudo correr sale con **2**, nunca
  con 0.
- Un hallazgo que se deja abierto lleva una celda que lo declara. Rojo que nadie decidió es ruido.
- Se comprueba la consecuencia y no el mensaje: qué rama corrió, qué valor salió, qué código de
  salida.
- Si una parte no se pudo hacer, se dice cuál. Reducir el alcance lo decide el autor.

---

*Autoría y uso de IA: este documento sigue la convención de `README.md` § Authorship & AI
Collaboration. Las decisiones sobre el lenguaje son del autor; las mediciones y la redacción son
trabajo de ingeniería asistido por Claude Code bajo su dirección.*
