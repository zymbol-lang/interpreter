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
| D-09 | 2026-11-22 | La forma del «enséñalo ya» del fotograma (GAP-ZYE-015, opción 2) | **`@~ 0`: ninguna forma nueva.** Una espera de 0 ms es una espera, y vacía el fotograma por la misma regla que `<<\|`, `<<\|?`, `@~` y `<<`; GUIDE § 3b documenta el modismo. GO y Chaturanga lo escriben antes de pensar | `t::show()` en `std/term`, que hoy sólo tiene funciones puras; un símbolo nuevo (`SYMBOLS.md` § 17); un vaciado por tiempo, que no es determinista |
| D-10 | 2026-11-22 | `engine = ">=0.0.9"` en GO, serpiente, klingon y Chaturanga | **Se queda: es verdadero.** Medido con el binario v0.0.9 de la release (suma verificada): `check` pasa en sus 65 ficheros y sus suites dan la misma salida en 0.0.9 que en 0.0.12 (21 de 21). ZyTetris pide `>=0.0.11` y falla en la 0.0.9: el suyo también es correcto | Subirlo a `>=0.0.12`, que dejaría fuera versiones en que los juegos corren |
| D-11 | 2026-11-22 | El workflow de Windows, en rojo en cada release porque se detiene ante la firma | **La firma, «skipped».** El build y el artefacto sin firmar en verde; el job de firma sólo corre con los secretos de SignPath y, sin ellos, sale omitido; un resumen del run dice que la firma manual está pendiente | Dejarlo en rojo; verde con un `::warning::`, que dejaría salir en verde una release sin `.msi` |
| D-12 | 2026-11-22 | `zyp.toml` descartaba en silencio una clave desconocida: Chaturanga y ZyBank escribían `description` y su descripción no llegaba al `zyp.json` | **Una grafía, `desc`, y un aviso W012** para toda clave desconocida; el paquete se escribe igual. Se corrigen los dos manifiestos y se regenera `chaturanga.zyp`. **Hecho** | Aceptar `description` como alias; rechazar el paquete, que rompe la regla de que empaquetar es permisivo |

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
| GAP-ZYE-015 | El fotograma: la opción 2, ya decidida por el autor para esta versión; su «enséñalo ya» es `@~ 0` (D-09) | `ZyEdit/HALLAZGOS.md` |
| DIS-ZYE-001 | Inicio, Fin, RePág, AvPág, Supr, F1–F12 y Shift+Tab llegan como `0d0` | `ZyEdit/HALLAZGOS.md` |
| DIS-ZYE-002 | Shift y Ctrl sobre una flecha se pierden | `ZyEdit/HALLAZGOS.md` |
| DIS-ZYE-003 | Ctrl+/, Ctrl+Espacio y Alt+letra llegan como un carácter imprimible | `ZyEdit/HALLAZGOS.md` |
| DIS-ZYE-004 | El cursor del terminal, oculto dentro de `>>\|` | `ZyEdit/HALLAZGOS.md` |
| DIS-ZYE-005 | Arrancar otro programa en un terminal, escribirle y leer lo que dibuja | `ZyEdit/HALLAZGOS.md` |
| GAP-ZYE-012 | Enterarse del cambio de tamaño de la ventana sin sondear | `ZyEdit/HALLAZGOS.md` |
| GAP-VMJS-001 | `<<` dentro del comparador de `$^`, en el navegador | `web/zymbol_vm/HALLAZGOS.md` |
| IDEA-001 | Un módulo `std/ansi` con nombres para los colores | `serpiente/HALLAZGOS_ES.md` |
| IDEA-Z004 | Lectura con tipo y longitud: `<< :###2 "…"` | `Zofia/HALLAZGOS.md` |

### C. Del tema: texto

| Id | Qué | Origen |
|---|---|---|
| GAP-ZYE-010 · GAP-TET-005 | Mayúsculas y minúsculas | `ZyEdit/HALLAZGOS.md`, `ZyTetris/HALLAZGOS.md` |
| GAP-ZYE-007 | Segmentar en clústeres de grafemas | `ZyEdit/HALLAZGOS.md` |
| GAP-TET-003 | `й` frente a `и` + U+0306: ¿se normalizan los identificadores? | `ZyTetris/HALLAZGOS.md` |
| GAP-TET-004 | La `a` latina y la `а` cirílica, sin aviso | `ZyTetris/HALLAZGOS.md` |
| GAP-ZYE-006 | Leer un fichero que no es UTF-8 válido | `ZyEdit/HALLAZGOS.md` |

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
