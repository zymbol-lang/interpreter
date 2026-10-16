# Mapa de ruta — v0.0.11: la VM es el motor, y lo que cuesta cada cosa

> **Estado: abierta.** La rama `v0.0.11` existe desde el 2026-10-12 y lleva un solo commit propio,
> el de versión (`62baa56`). **La etapa 1 está implementada** (2026-10-13) salvo el sondeo de las
> cinco zonas: ver «Avance» dentro de ella. **La etapa 2 tiene lista de trabajos y arnés de medición**
> (2026-10-14), y su primera corrección hecha: PERF-06 (2026-10-16, `ZYVM-023`). Ver «Avance» dentro de
> ella. De las etapas 3 y 4 no hay nada hecho.
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
| 4 | La VM del navegador (`zyvm.js`) | Posición por confirmar: ver decisión D5 |

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

- **El sondeo de las cinco zonas.** Sigue pendiente. El cambio se subió a la rama con la puerta en
  verde y sin él; hay que hacerlo antes de la release.
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

### Lo que ya se sabía

| Id | Qué es | Estado |
|---|---|---|
| ZYVM-022 | El valor que devuelve una llamada sobrevive en su registro temporal: la VM tiene dos arrays donde el tree-walker tiene uno (1,79× de memoria, medido en la v0.0.10) | Abierto, con celda `KNOWN` en `zyquality/cost/` |
| GLB-127 | `>> +a` lo rechazan `zytw` y `zyvm` y `zyjs` lo acepta | Abierto; no es de rendimiento, se hereda |

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
| D1 | ¿Qué motor graba los goldens? | `zyvm`, desde el cierre de la etapa 1: es el motor que ejecuta todo el mundo, y `zytw` tiene que coincidir con él |
| D2 | ¿Qué hace el REPL? | Se queda en el tree-walker en esta versión, y se dice. Portarlo es diseño, no un flag |
| D3 | ¿Qué pasa con `--vm`? | Se conserva y se acepta sin efecto: hay scripts que lo escriben |
| D4 | ¿`std/db` sale de las features por defecto? | Sí. Los releases de Linux y macOS ya se construyen sin ella |
| D5 | ¿Dónde va la etapa 4? | Después de 3.3, dentro de esta versión si la etapa 2 no la desplaza |
| D6 | ¿Qué falló en el contenedor de 4 GB? | No es una decisión sino un dato que falta, y cambia el primer arreglo de 3.2. ZyBigBench, hecho en ese contenedor, apunta a una pista: su `README` dice que el perfil de release con LTO no terminaba allí |
| D7 | PERF-07: el error que sustituye al aborto, ¿se captura con `!?`, y de qué familia es? | Que se capture, como cualquier error de ejecución (el `RecursionError` de Python lo es); la familia la nombra el autor |
| D8 | PERF-05 b: ¿la cadena guarda si es ASCII, o cuántos caracteres tiene? | Aplazar hasta tener el perfil de cargas reales; sin él, el arreglo cambia una representación por un caso que quizá ningún programa paga |

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
