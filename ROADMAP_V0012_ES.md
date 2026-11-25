# Mapa de ruta — v0.0.12: Alpha pública — terminal y texto

> **Estado: abierta** desde el 2026-11-22. Rama `v0.0.12` en los 23 repositorios (los 22 de la v0.0.11 y ZyTetris),
> empujada al abrirla; primer commit, el de versión (`b910446`). Es un plan, no un registro: lo hecho, con su
> verificación, irá a `IMPL_V0012.md`; los hallazgos, a su origen (`ZyDDT/HALLAZGOS/`, el `HALLAZGOS.md` de cada
> aplicación), y lo que quede abierto, a `ZyDDT/PENDIENTES.md`.
>
> **Cómo se decide.** El autor decide cada punto entre opciones, con un ejemplo cuando es de código; cada decisión lleva
> un id `D-NN`, correlativo en esta versión, y el código y los registros la citan por él. Nada se construye sin su
> decisión (`zymbol-design/HOW_TO_CHANGE_ZYMBOL.md` § 1).

---

## El tema (D-04)

La v0.0.11 fue la de la optimización. La v0.0.12 es la primera que se anuncia como **alpha pública**, y su tema es
**terminal y texto**: de las 19 decisiones de diseño que la v0.0.11 dejó abiertas, 15 son de una de las dos cosas, y
DIS-ZYE-005 bloquea las etapas 4 y 5 de ZyEdit. Lo que no es del tema entra sólo si es barato o si bloquea algo.

---

## Decisiones tomadas

| Id | Fecha | La pregunta | Decisión del autor | Descartado |
|---|---|---|---|---|
| D-01 | 2026-11-22 | ¿Dónde vive la palabra «alpha»? | **Un rótulo, no la versión.** `Cargo.toml` dice `0.0.12` y el tag será `v0.0.12`; «alpha» va en el título de la release, el README, la web y `zymbol --version` (`zymbol 0.0.12 (alpha)`, `2b22fc2`) | `0.0.12-alpha` como versión semver: con ella `engine = ">=0.0.9"` y `">=0.0.11"` dan `false` (medido con el crate `semver` del intérprete), y ningún `.zyp` existente arrancaría. Declarar alpha a toda la serie 0.0.x sin rótulo por versión |
| D-02 | 2026-11-22 | ¿En qué repositorios se abre la rama? | **Los 22 de la v0.0.11 y ZyTetris**, que entra en el ciclo de release | Dejar ZyTetris en `main` |
| D-03 | 2026-11-22 | ¿Se empuja la rama al abrirla? | **Sí**, como en la v0.0.11 | Sólo en local |
| D-04 | 2026-11-22 | ¿Qué tema tiene la versión? | **Terminal y texto** (arriba) | Vaciar `PENDIENTES.md` sin tema; dedicarla al pulido de lo que ve quien llega |
| D-05 | 2026-11-22 | ¿Cómo se ordena el texto? (BUG-TET-001 · GAP-TET-002) | **Por punto de código, en los cuatro motores**, en cualquier máquina; `GUIDE.md` deja de decir «lexicográfico». **Hecho** el mismo día | Por idioma (ICU en Rust, y el golden dependería del `LANG`); punto de código más un comparador de diccionario documentado, que necesita antes GAP-TET-005 |
| D-06 | 2026-11-22 | `.zyb` (3.3 de la v0.0.11, aplazada dos veces) | **Desestimado, con la razón escrita** en `ROADMAP.md` § Bytecode File Format: ahorra de 1 a 15 ms sobre `zymbol run`; `zymbol build --release` ya lleva el bytecode; como oráculo de `zymbol_vm.js` ya está `compile_diff.mjs` | `zymbol compile` explícito (~1,5 días); caché automática al estilo de `__pycache__` (~2–3 días) |
| D-07 | 2026-11-22 | `$^+` frente a la lectura numérica de `<` (`"10" > "9"`) | **`$^+` ordena por punto de código, y se documenta.** La lectura de `<` no es un orden total: `"2" < "10"`, `"10" < "1a"` y `"1a" < "2"` son un ciclo. Para ordenar números escritos como texto, convertirlos antes. **Hecho** | `$^+` con la regla de `<` (el resultado dependería del orden de entrada); numérico sólo si todos los textos son números |
| D-08 | 2026-11-22 | `$^+`/`$^-` sobre elementos sin orden natural | **Un `##Type`**, antes de mover nada, con una ayuda que pide un comparador, como ya hacía `<`. **Hecho** (GLB-152) | El orden lexicográfico de Python para arreglos; dejarlo y documentarlo |
| D-09 | 2026-11-22 | La forma del «enséñalo ya» del fotograma (GAP-ZYE-015, opción 2) | **`@~ 0`: ninguna forma nueva.** Una espera de 0 ms es una espera, y vacía el fotograma por la misma regla que `<<\|`, `<<\|?`, `@~` y `<<`; GUIDE § 3b documenta el modismo. GO y Chaturanga lo escriben antes de pensar. **Hecho** el mismo día en los cuatro motores; destapó ZYVM-033 (`¶` en escalera en la VM) | `t::show()` en `std/term`, que hoy sólo tiene funciones puras; un símbolo nuevo (`SYMBOLS.md` § 17); un vaciado por tiempo, que no es determinista |
| D-10 | 2026-11-22 | `engine = ">=0.0.9"` en GO, serpiente, klingon y Chaturanga | **Se queda: es verdadero.** Medido con el binario v0.0.9 de la release (suma verificada): `check` pasa en sus 65 ficheros y sus suites dan la misma salida en 0.0.9 que en 0.0.12 (21 de 21). ZyTetris pide `>=0.0.11` y falla en la 0.0.9: el suyo también es correcto | Subirlo a `>=0.0.12`, que dejaría fuera versiones en que los juegos corren |
| D-11 | 2026-11-22 | El workflow de Windows, en rojo en cada release porque se detiene ante la firma | **La firma, «skipped».** El build y el artefacto sin firmar en verde; el job de firma sólo corre con los secretos de SignPath y, sin ellos, sale omitido; un resumen del run dice que la firma manual está pendiente | Dejarlo en rojo; verde con un `::warning::`, que dejaría salir en verde una release sin `.msi` |
| D-12 | 2026-11-22 | `zyp.toml` descartaba en silencio una clave desconocida: Chaturanga y ZyBank escribían `description` y su descripción no llegaba al `zyp.json` | **Una grafía, `desc`, y un aviso W012** para toda clave desconocida; el paquete se escribe igual. Se corrigen los dos manifiestos y se regenera `chaturanga.zyp`. **Hecho** | Aceptar `description` como alias; rechazar el paquete, que rompe la regla de que empaquetar es permisivo |
| D-13 | 2026-11-22 | ¿Con qué valor llegan a `<<\|` Inicio, Fin, RePág, AvPág, Supr, F1–F12 y Shift+Tab? (DIS-ZYE-001) | **Glifos, como las flechas:** Inicio `⇱`, Fin `⇲`, RePág `⇞`, AvPág `⇟`, Supr `⌦`, Shift+Tab `⇤`, F1–F12 `①`–`⑫`. El guion de `--keys` gana `HOME`, `END`, `PGUP`, `PGDN`, `DEL`, `F1`…`F12` y `BACKTAB`. **Hecho** el 2026-11-22 en los cuatro motores y en `--keys` | Puntos de uso privado (los de macOS: se escriben como cifras); glifos más constantes con nombre en `std/term` (dos grafías, y `std/term` dejaría de ser sólo funciones) |
| D-14 | 2026-11-22 | ¿Cómo pregunta un programa por el modificador de una tecla? (DIS-ZYE-002, DIS-ZYE-003) | **Una segunda variable, opcional: `<<\| k, m`** (y `<<\|? k, m`). `m` es un Int: Shift = 1, Alt = 2, Ctrl = 4. `<<\| k` no cambia; Ctrl+letra sigue llegando como su carácter de control, con `m = 4`. El guion de `--keys` gana `SHIFT+`, `ALT+` sobre cualquier tecla. **Hecho** el 2026-11-22 (instrucción `ReadKeyMods`) | `t::mods()`, una función con estado en `std/term`; arreglar sólo DIS-ZYE-003 y seguir perdiendo Shift+→ |
| D-15 | 2026-11-22 | En `<<\| k, m`, ¿lleva `m` el bit de Shift cuando la tecla es texto imprimible (`'A'`, `'!'`)? | **No: el carácter ya lo dice.** Shift se informa sólo en teclas que no son texto (flechas, `⇱ ⇲ ⇞ ⇟ ⌦`, `①`–`⑫`, Enter, Tab, Retroceso, Esc; `⇤` llega con `m = 1`). Una `'A'` llega con `m = 0` en las cuatro plataformas, con Bloq Mayús o sin él, y en `--keys` basta `A`. Ctrl+Alt sobre texto imprimible es AltGr y da `m = 0`. **Hecho** el 2026-11-22 | Que `m` diga lo que se pulsó: con Bloq Mayús Linux daría 1 y Windows 0, y AltGr daría 6 en Windows |
| D-16 | 2026-11-23 | ¿Cómo pide un programa el cursor del terminal dentro de `>>\|` (DIS-ZYE-004), y cuánto dura? | **`>>~ (fila, col)` sin nada que imprimir** lo muestra ahí y **dura hasta que se mueva u oculte**: `>>~ (0, 0)` lo oculta. Cada fotograma termina con el cursor devuelto a ese sitio, aunque se haya dibujado en otro, y `>>!` no lo oculta. Sólo una posición: `>>~ (1, 6, 1)` y `>>~ (1,)` son error de sintaxis; fuera de numeración es `##Range`; fuera de `>>\|` es error. En `--keys`, una línea `[cursor f,c]` tras el fotograma. **Hecho** el 2026-11-23, con la instrucción `ShowCursor` | `t::cursor(f, c)` en `std/term` (rompe la familia `>>! >>? >>~` del canal de pantalla); desestimar (ZyEdit sigue pintando su caret); que dure un solo fotograma (un `<<\|?` sin dibujo lo ocultaría) |
| D-17 | 2026-11-23 | ¿Cómo se entera un programa bloqueado en `<<\|` de que la ventana cambió de tamaño (GAP-ZYE-012)? | **Un glifo `⤢` (U+2922) como «tecla»**, con `m = 0`, en `<<\|` y `<<\|?`; el programa vuelve a preguntar `>>?`. La pantalla ya tiene el tamaño nuevo: lo dibujado conserva su sitio y lo que no cabe se corta. El playground lo envía al cambiar el panel (un `⤢` aunque lleguen varios sin leer). `--keys` gana `RESIZE filas cols`. **Hecho** el 2026-11-23 | `⤢` sólo si el programa preguntó `>>?` (regla que depende de algo hecho antes); desestimar (sondeo con `<<\|?`). Contra aceptado: un «pulsa cualquier tecla» también avanza al redimensionar |
| D-18 | 2026-11-23 | ¿Qué forma tiene pasar texto a mayúsculas o minúsculas (GAP-ZYE-010, GAP-TET-005), y qué hace el plegado? | **Módulo nuevo `std/text`: `upper`, `lower`, `fold`**, con las reglas de Unicode sin idioma. `upper`/`lower` son las correspondencias completas (`ß` → `SS`, sigma final) y dan String; `fold` es el plegado simple, **punto a punto, conserva la longitud** y devuelve el tipo que recibe. Los motores del navegador llevan una tabla generada de la UCD de la versión de Rust (18.0.0), comprobada contra Rust en todos los puntos de código por `web/tests/test_casemap.mjs`. **Hecho** el 2026-11-23 | Una marca nueva (`$↑` `$↓`: ninguna marca existente la deriva, `^` ya tiene tres significados); desestimar. Para el plegado: el completo (`ß` = `ss`, pero cambia la longitud) o sólo `upper`/`lower` |
| D-19 | 2026-11-23 | ¿Cómo se ofrece la segmentación en clústeres de grafemas, UAX #29 (GAP-ZYE-007)? | **`tx::graphemes(s)` en `std/text`**: un Array de Strings, un clúster en cada uno; el `Char` sigue siendo un punto de código. En el navegador, tablas generadas de Unicode 17 (la versión de `unicode-segmentation`), que pasan los 766 casos de `GraphemeBreakTest.txt`; `web/tests/test_graphemes.mjs` las compara con el binario Rust en 4 000 textos. **Hecho** el 2026-11-23 | En `std/term` (sólo métricas de pantalla); desestimar (ZyEdit seguía aproximando por anchos) |
| D-20 | 2026-11-23 | ¿Se normalizan los identificadores (GAP-TET-003)? `й` como U+0439 y como `и` + U+0306 eran dos variables distintas que se ven igual | **NFC, la equivalencia canónica**: lo que Unicode declara la misma letra es el mismo nombre, en variables, funciones, parámetros, etiquetas, `{…}`, tipos de error y alias. `ﬁ` y `fi` siguen distintos. Las cadenas y las rutas de módulo conservan lo escrito; `zymbol fmt` también deja cada nombre como está escrito. Rust usa `unicode-normalization` (Unicode 18); el navegador, una tabla generada de esa versión que pasa todo `NormalizationTest.txt` y que `web/tests/test_normalize.mjs` compara con el lexer de Rust en 4 000 nombres. **Hecho** el 2026-11-23 | NFKC como Python (une `x²` con `x2`); rechazar lo que no esté en NFC; no normalizar y sólo avisar. Para `fmt`: reescribir en NFC |
| D-21 | 2026-11-24 | ¿Quién avisa de que dos nombres se ven iguales y no lo son (GAP-TET-004), y con qué regla? | **Sólo el editor** (LSP y panel del playground), como la regla del 2026-09-25 para los caracteres que se parecen a un símbolo: `check` y `run` callan. **Regla: el esqueleto de UTS #39**: dos nombres distintos de un fichero con el mismo esqueleto avisan sobre el posterior; un nombre tras `::` o `.` no cuenta (la capa griega `mat.E => Ε`). LSP con `unicode-security` (Unicode 18); playground con `skeleton.js`, generado del mismo `confusables.txt`; `web/tests/test_lookalike.mjs` compara los dos editores en 3 000 nombres. Medido sobre 3 784 `.zy`: fuera de esa capa, sólo las sondas del 2026-09-25. **Hecho** el 2026-11-24 | También en `zymbol check` (rompe la regla del 2026-09-25); desestimar. Para la regla: una tabla corta latín–cirílico–griego escrita a mano |
| D-22 | 2026-11-24 | ¿Cómo se leen y escriben los bytes de un fichero que no es UTF-8 válido (GAP-ZYE-006)? | **Al estilo de ZyBF: un byte es un `Char` de `0x00` a `0xFF`**, como ya lo escriben `0x01` y `##'n` (GAP-BF-001, GLB-109). `io::read_bytes(p)` da un **String** con un carácter por byte y `io::write_bytes(p, t)` lo escribe tal cual; un carácter mayor que `0xFF` es `##Range` y no toca el fichero. Sin tipo nuevo: `$#`, `t[i]`, `$?`, la yuxtaposición y comparar con `0xFF` sirven ya. `io::read` sigue fallando cerrado ante UTF-8 inválido. **Hecho** el 2026-11-24 | Un Array de Int o de Char (no busca secuencias con `$?`, un valor del motor por byte); texto con sustitución U+FFFD (pierde los bytes); desestimar |
| D-23 | 2026-11-24 | ¿Con qué ancho escriben `0x\|…\|`, `0o\|…\|`, `0b\|…\|` y `0d\|…\|` un Char o un Int? Rellenaban `0x` y `0d` a cuatro cifras (un punto de código) y `0b` y `0o` a ninguna: los bytes de `é` salían `0x00C3 0x00A9` | **Nunca menos de un byte**, como ya se escribe su literal en cada base (`0x00`–`0xFF`, `0o000`–`0o377`, `0b00000000`–`0b11111111`): `0x41 0o101 0b01000001`; un valor mayor ocupa sus cifras (`0x20AC`). **El decimal sin relleno**: `0d65`, `0d0`–`0d255` (la D-24 lo volvió número y la D-25 lo devolvió a texto). Una sola regla para Char e Int, signo delante (`-0x01`). Es la mirada de Zenith (`0b{:08b}`), llevada a las tres bases que tienen ancho de byte. Mueve 4 goldens y el GUIDE; ninguna aplicación lo usaba. **Hecho** el 2026-11-24 | Bytes enteros (rellenar a 2, 3… bytes: `0x0100`, `0d00256`); el decimal a tres cifras (`0d065`); dejarlo como estaba; un `0u\|…\|` para el punto de código, como Zenith (símbolo por meter: los diagnósticos ya escriben `U+0041`) |
| D-24 | 2026-11-24 | `##'n` y `##!c` no se entienden ni se parecen a la familia de bases: `##!letra_A` da `65` donde `0x|letra_A|` da `"0x41"`, y dejaron hallazgos en las apps (ERROR-BF-002, las sumas de cifras de ZyEdit, la tabla ASCII de ZyBank). ¿Cuál es la puerta entre un carácter y su código? | **Sólo los prefijos de base**: cada uno convierte entre el carácter y su elemento en esa base; ~~**el elemento decimal es el número** (`0d\|'A'\|` → `65`, `0d\|65\|` → `'A'`), los de `0x`, `0o` y `0b` son texto~~ (**sustituido por la D-25** el mismo día: el decimal rompía la homogeneidad). **`##'` se retira** de las expresiones (sigue como tipo y en `<< ##'`) y **`##!` deja de tomar un `Char`** (sigue truncando un Float): **error ya**, en los cuatro motores, con una línea de ayuda; se migraron corpus, ZyDDT, ZyBF, ZyEdit, ZyTetris, ZyBank, GoL, GO y Zofía. La ranura `IntToChar` del bytecode se queda, sin emitirse, para no renumerar. **Hecho** el 2026-11-24 | Como Zenith: todo texto (`0d\|'A'\|` → `"0d65"`, `0x\|65\|` → `'A'`; pierde `0x\|4660\|`); retirar `##!` entero; aviso ahora y error en la v0.0.13 |
| D-25 | 2026-11-24 | La D-24 deja el decimal como la base distinta: `0d\|'A'\|` da `65` y `0x\|'A'\|` da `"0x41"`. ¿Cómo se vuelve homogéneo? | **Tres puertas, una por resultado**: `0x\|v\|`, `0d\|v\|`, `0o\|v\|`, `0b\|v\|` dan **siempre el texto** en esa base (`0d\|'A'\|` → `"0d65"`, `0d\|"0b01000001"\|` → `"0d65"`); **las barras solas** `\|v\|` dan **siempre el carácter** (`\|65\|`, `\|"0d65"\|`, `\|"0x41"\|` → `'A'`); **`#\|v\|` da siempre el número**, y un `Char` es su código (`#\|'A'\|` → `65`, `#\|'7'\|` → `55`; el valor de la cifra es el del texto, `#\|"{c}"\|`, lo que cambia el arreglo de GAP-ZYB-012). Todas toman un Char, un Int o un texto en cualquier base, con signo y prefijo; sin prefijo, en la base de la puerta (en decimal para `\|…\|` y `#\|…\|`). Un texto con el prefijo de otra base se lee bien: antes `0x\|"0d65"\|` daba `'e'`. En los cuatro motores, con el lector compartido `num::parse_base_text` / `bases.js`; `\|…\|` es `BasePrefix::Char` y la VM usa el radix 0 de `BaseConvert`, sin instrucción nueva. Se migraron corpus, ZyDDT, ZyBF, ZyEdit, ZyTetris, ZyBank, GoL, GO, Zofía y los manuales v0.0.11. **Hecho** el 2026-11-24 | Como propuso el autor en su ejemplo, `0d\|Int\|` → `'A'` (el decimal conserva un brazo propio); `#\|…\|` sólo para textos con prefijo; para una cifra, su valor y no su código (el byte `0x37` de ZyBF daría 7); prefijo obligatorio en los textos |
| D-26 | 2026-11-25 | B-2 de `zymbol-design/BYTES.md`: escribir un byte en su sitio con `b[i]$~ v` copiaba el String entero en los cuatro motores, ASCII o no; con 16 000 bytes, 0,6 s en Rust y 5,7 s en el navegador, y 1 MB tardaría horas. El mismo bucle sobre un Array era lineal. ¿Qué se hace? | **En su sitio, en Rust**: `Text` (TW) y `ZyStr` (VM) cambian un carácter por otro del mismo ancho en UTF-8 sin mover ningún byte cuando nadie más tiene el texto, y conservan la cuenta y las migas de posición: 16 000 bytes en 25 y 46 ms. Un cambio de ancho sigue copiando, sin el `Vec<char>` de antes. El navegador sigue cuadrático porque un string de JS no se puede escribir: **ZYJS-076**, deuda declarada en `zyquality/cost/` (`growth/byte-write-in-place`). El caso de coste destapó otro: en la VM, añadir un `Char` a un texto (`b = b 0x80`) copiaba el texto entero (**ZYVM-034**, corregido). Ningún programa cambia lo que dice. **Hecho** el 2026-11-25 | Una forma editable del String en los cuatro motores (arregla también el navegador, pero toca todas sus operaciones de texto); el buffer como Array de códigos, sin tocar motores; sólo registrarlo para la v0.0.13 |

D-05, D-07 y D-08 se implementaron juntas el 2026-11-22: `ZyDDT/HALLAZGOS/GLOBAL.md` § GLB-152 (los cuatro motores no
ordenaban los `Char` y devolvían intacto lo que no tiene orden) y `ZyDDT/HALLAZGOS/zyjs.md` § ZYJS-073 (`<` entre textos en
`zyjs` iba por unidad UTF-16). Las sujeta `zyquality/corpus/collections/sort_orden_natural.zy`.

---

## Lo que queda por decidir o hacer

Lo comprobado el 2026-11-22 lleva esa fecha. Las filas de `ZyDDT/PENDIENTES.md` se volvieron a ejecutar por última vez el
2026-10-30: antes de decidir cada una, se vuelve a ejecutar.

### A. Restos de herramienta y de la release v0.0.11 (comprobados el 2026-11-22)

| Qué | Dónde | Decisión |
|---|---|---|
| El Worker de Cloudflare no está delante del sitio: `Accept: text/markdown` devuelve `text/html`, sin `Link` ni `Vary: Accept` | `web/worker/` | ninguna: lo despliega el autor (`npx wrangler deploy`) |
| El job de pruebas unitarias de `gate.yml` clona `zyquality` sin rama | `interpreter/.github/workflows/gate.yml`, línea 100 | **hecho** (`36aeeef`) |
| `verify-deb.sh` rotula su prueba «run (tree-walker)» cuando el motor por defecto es la VM — y por eso las dos ejecuciones de su sección 5 probaban la VM | `interpreter/packaging/verify/verify-deb.sh`, líneas 379 y 381 | **hecho** (`36aeeef`): `run --tw` |
| `zymbol package` pierde la `description` del manifiesto | `interpreter/crates/zymbol-package/` | **D-12**: hecho, W012 |
| GO, serpiente, klingon y Chaturanga piden `engine = ">=0.0.9"` en su `zyp.toml` y en su `.zyp` del playground | cada repositorio y `web/examples/games/` | **cerrado por D-10**: es verdadero |
| El workflow de Windows sale en rojo en cada release: se detiene a propósito ante la firma | `release-windows.yml` | **D-11**: hecho (`2a404ad`), comprobado en la ejecución 45059778904 |
| Las Actions avisan de que Node 20 está obsoleto (`checkout@v4`, `upload-artifact@v4`) | los workflows | mecánico |
| Los marcadores «Revisado para» de 53 documentos y el registro de versiones de la cabecera de `zymbol.js` | `aprende_zymbol/`, `web/` | aplazados por el autor el 2026-10-31: cada documento se lee antes de marcarlo |

### B. Del tema: terminal

| Id | Qué | Origen |
|---|---|---|
| GAP-ZYE-015 | El fotograma: la opción 2, ya decidida por el autor para esta versión; su «enséñalo ya» es `@~ 0` (D-09). **Hecho** el 2026-11-22 | `ZyEdit/HALLAZGOS.md` |
| DIS-ZYE-001 | Inicio, Fin, RePág, AvPág, Supr, F1–F12 y Shift+Tab llegan como `0d0`. **Hecho** el 2026-11-22 (D-13) | `ZyEdit/HALLAZGOS.md` |
| DIS-ZYE-002 | Shift y Ctrl sobre una flecha se pierden. **Hecho** el 2026-11-22 (D-14) | `ZyEdit/HALLAZGOS.md` |
| DIS-ZYE-003 | Ctrl+/, Ctrl+Espacio y Alt+letra llegan como un carácter imprimible. **Hecho con resto** el 2026-11-22 (D-14): en un terminal Ctrl+/ sigue siendo `7`, con `m = 4` | `ZyEdit/HALLAZGOS.md` |
| DIS-ZYE-004 | El cursor del terminal, oculto dentro de `>>\|`. **Hecho** el 2026-11-23 (D-16) | `ZyEdit/HALLAZGOS.md` |
| DIS-ZYE-005 | Arrancar otro programa en un terminal, escribirle y leer lo que dibuja | `ZyEdit/HALLAZGOS.md` |
| GAP-ZYE-012 | Enterarse del cambio de tamaño de la ventana sin sondear. **Hecho** el 2026-11-23 (D-17) | `ZyEdit/HALLAZGOS.md` |
| GAP-VMJS-001 | `<<` dentro del comparador de `$^`, en el navegador | `web/zymbol_vm/HALLAZGOS.md` |
| IDEA-001 | Un módulo `std/ansi` con nombres para los colores | `serpiente/HALLAZGOS_ES.md` |
| IDEA-Z004 | Lectura con tipo y longitud: `<< :###2 "…"` | `Zofia/HALLAZGOS.md` |

### C. Del tema: texto

| Id | Qué | Origen |
|---|---|---|
| GAP-ZYE-010 · GAP-TET-005 | Mayúsculas y minúsculas. **Hecho** el 2026-11-23 (D-18) | `ZyEdit/HALLAZGOS.md`, `ZyTetris/HALLAZGOS.md` |
| GAP-ZYE-007 | Segmentar en clústeres de grafemas. **Hecho** el 2026-11-23 (D-19) | `ZyEdit/HALLAZGOS.md` |
| GAP-TET-003 | `й` frente a `и` + U+0306: ¿se normalizan los identificadores? **Hecho** el 2026-11-23 (D-20) | `ZyTetris/HALLAZGOS.md` |
| GAP-TET-004 | La `a` latina y la `а` cirílica, sin aviso. **Hecho** el 2026-11-24 (D-21) | `ZyTetris/HALLAZGOS.md` |
| GAP-ZYE-006 | Leer un fichero que no es UTF-8 válido. **Hecho** el 2026-11-24 (D-22) | `ZyEdit/HALLAZGOS.md` |

### D. Fuera del tema: entran si son baratos o si bloquean algo

| Id | Qué | Origen |
|---|---|---|
| BUG-TET-009 | `#\|texto\|` con un entero fuera de ±(2^53−1) en `zyjs` | `ZyTetris/HALLAZGOS.md` |
| BUG-VMJS-001 | `#^.N` en `zyjs`: los empates y el `.0` | `web/zymbol_vm/HALLAZGOS.md` |
| BUG-ZYE-011 | La VM copia el arreglo entero tras `(a, k) = v[1]` y `v[1]$~ otra` | `ZyEdit/HALLAZGOS.md` |
| BUG-ZYE-014 | `a$+[i] v` y `a$-[i..j]` cuestan el arreglo entero | `ZyEdit/HALLAZGOS.md` |
| GAP-ZYE-008 (resto) | Que el error de `###"42"` diga que la forma es `#\|texto\|` | `ZyEdit/HALLAZGOS.md` |
| BUG-ZYE-009 | `@ i:r1 + 1..r2 - 1`: los motores Rust lo rechazan y los de JavaScript lo ejecutan | `ZyEdit/HALLAZGOS.md` |
| IDEA-TET-007 | Escribir los símbolos de tipo de `#?` en una expresión | `ZyTetris/HALLAZGOS.md` |
| IDEA-TET-008 | `zymbol fmt` con un arreglo de números negativos | `ZyTetris/HALLAZGOS.md` |
| GoL | `αριθμός` en lugar de `#\|texto\|` | `ZyTetris/HALLAZGOS.md` § GAP-TET-006 |
| `web/plan_zywasm32.md` | El playground sobre la VM wasm32 (8–9 días, sin empezar), ahora que `zymbol_vm.js` es el motor por defecto | `web/plan_zywasm32.md` |
| wasm64 | Las pruebas sobre wasm64, que el autor dejó «para luego» | `ROADMAP_V0011_ES.md` § 3.4 |
