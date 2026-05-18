#!/usr/bin/env python3
"""
Verifica que los ejemplos de código en GUIDE.md son ejecutables
y producen el output esperado (marcado con // →).

Estrategia:
  - Extrae todos los bloques ```zymbol del GUIDE.
  - Descarta bloques sin anotaciones // → (no verificables).
  - Descarta bloques que contienen // ✗ (ejemplos de error esperado).
  - Para bloques multi-archivo (// file: ...) crea los archivos
    en un directorio temporal y ejecuta el archivo principal.
  - Compara output real vs. esperado línea a línea.
"""

import re
import subprocess
import sys
import tempfile
import os
from pathlib import Path
from dataclasses import dataclass, field
from typing import Optional

ZYMBOL = Path(__file__).parents[2] / "target" / "release" / "zymbol"
GUIDE  = Path(__file__).parents[2] / "GUIDE.md"

# ── resultado de cada caso ─────────────────────────────────────────────────────

@dataclass
class Result:
    index   : int
    section : str
    status  : str        # PASS | FAIL | ERROR | SKIP
    reason  : str = ""
    expected: list[str] = field(default_factory=list)
    actual  : list[str] = field(default_factory=list)


# ── extracción de bloques ──────────────────────────────────────────────────────

BLOCK_RE  = re.compile(r"```zymbol\n(.*?)```", re.DOTALL)
SECTION_RE = re.compile(r"^#{1,3} (.+)$", re.MULTILINE)
ARROW_RE  = re.compile(r"//\s*→\s*(.+)$")


def extract_blocks(text: str) -> list[dict]:
    """Devuelve lista de {index, section, code} para cada bloque zymbol."""
    # Índice de secciones por posición en el texto
    sections = [(m.start(), m.group(1)) for m in SECTION_RE.finditer(text)]

    blocks = []
    for i, m in enumerate(BLOCK_RE.finditer(text)):
        pos  = m.start()
        code = m.group(1)
        sec  = next(
            (s for p, s in reversed(sections) if p < pos),
            "top"
        )
        blocks.append({"index": i + 1, "section": sec, "code": code})
    return blocks


# ── clasificación de bloques ───────────────────────────────────────────────────

def collect_expected(code: str) -> list[str]:
    """
    Extrae valores de // → en orden.
    Limpia sufijos explicativos del GUIDE:
      valor  (nota)         → valor
      valor  — nota         → valor
      valor  ← nota         → valor
    Si el valor contiene '...' es un comodín: se marca como WILD.
    """
    lines = []
    for line in code.splitlines():
        m = ARROW_RE.search(line)
        if m:
            raw = m.group(1).strip()
            # Quitar sufijos explicativos tras separadores visuales
            cleaned = re.sub(r'\s+(←|—|--)\s.*$', '', raw)
            cleaned = re.sub(r'\s{2,}\(.*$', '', cleaned)
            cleaned = re.sub(r'\s\((?!.*\).*\().*\)$', '', cleaned)
            lines.append(cleaned.strip())
    return lines


def is_wildcard(expected: str) -> bool:
    """El valor esperado contiene '...' — comparación parcial."""
    return "..." in expected


def is_error_example(code: str) -> bool:
    """El bloque muestra intencionalmente un error (// ✗ o // Error o error expected)."""
    return bool(re.search(r"//\s*(✗|Runtime error|E\d{3})", code))


def is_cli_args(code: str) -> bool:
    """Bloque que depende de argumentos de línea de comandos (><)."""
    return ">< " in code or "><\n" in code or code.strip().startswith("><")


def is_repl_session(code: str) -> bool:
    """Bloque en formato REPL (líneas que comienzan con '> ')."""
    lines = [l for l in code.splitlines() if l.strip()]
    return bool(lines) and sum(1 for l in lines if l.startswith("> ")) > len(lines) // 2


def is_description_only(expected_list: list[str]) -> bool:
    """Todas las anotaciones son descripciones en prosa, no valores literales."""
    PROSE_RE = re.compile(r'^\(.*\)$|^prints\b|^\d+ times$', re.IGNORECASE)
    return all(PROSE_RE.match(e) for e in expected_list)


def has_external_import(code: str) -> bool:
    """Bloque importa un módulo externo (<#) no satisfecho dentro del mismo bloque."""
    if "<#" not in code:
        return False
    # Colectar módulos definidos en el bloque via '// file:'
    defined = set()
    for m in re.finditer(r'//\s*file:\s*(\S+)', code):
        path = m.group(1).lstrip("./")
        # Nombre sin extensión
        defined.add(path.replace(".zy", "").split("/")[-1])
        defined.add(path)

    # Colectar módulos importados: <# ./path => alias
    for m in re.finditer(r'<#\s+\./([^\s=]+)', code):
        mod = m.group(1).lstrip("./").replace(".zy", "")
        if mod not in defined and mod + ".zy" not in defined:
            return True  # importa algo no definido en el bloque
    return False


def is_prose_expected(val: str) -> bool:
    """Valor esperado es descripción en prosa, no output literal."""
    return bool(re.match(r'^\(.*\)$', val.strip()))


def is_context_dependent(code: str) -> bool:
    """
    Heurística: el bloque usa variables/funciones que no define él mismo.
    Señales: primera línea no es asignación/función/import/loop/if/output.
    Se detecta cuando hay expresiones de uso sin definición de las partes.
    """
    # Si empieza directamente con >> o un identifier usado (no =)
    stripped = [l.strip() for l in code.splitlines() if l.strip() and not l.strip().startswith("//")]
    if not stripped:
        return False
    first = stripped[0]
    # Si la primera línea usa una variable sin definirla → depende de contexto
    if re.match(r'^[a-zA-Z_]\w*\s*[^=({\[<>:]', first) and "(" not in first.split()[0]:
        return True
    return False


def is_multi_file(code: str) -> bool:
    return "// file:" in code


# ── ejecución ─────────────────────────────────────────────────────────────────

WARNING_LINE = re.compile(
    r"^(warning:|  -->|  = help:|  = note:|  = warning:|note:)"
)


def strip_warnings(output: str) -> str:
    """Elimina líneas de advertencia del output para comparación."""
    return "\n".join(
        line for line in output.splitlines()
        if not WARNING_LINE.match(line)
    ).strip()


def run_single(code: str, tmpdir: str) -> tuple[int, str]:
    """Escribe code en un archivo .zy y lo ejecuta. Retorna (returncode, output_sin_warnings)."""
    path = os.path.join(tmpdir, "test.zy")
    with open(path, "w") as f:
        f.write(code)
    r = subprocess.run(
        [str(ZYMBOL), "run", path],
        capture_output=True, text=True, timeout=10
    )
    combined = r.stdout + r.stderr
    return r.returncode, strip_warnings(combined)


def run_multi_file(code: str, tmpdir: str) -> tuple[int, str]:
    """
    Divide el bloque en archivos según comentarios // file: path
    y ejecuta el último que sea main.zy o el último archivo listado.
    """
    file_re = re.compile(r"//\s*file:\s*(\S+)")
    chunks   = re.split(r"(?=//\s*file:\s*\S+)", code)

    files_created = []
    for chunk in chunks:
        chunk = chunk.strip()
        if not chunk:
            continue
        m = file_re.match(chunk)
        if not m:
            continue
        rel_path = m.group(1)
        # strip leading ./ or similar
        rel_path = rel_path.lstrip("./")
        abs_path = os.path.join(tmpdir, rel_path)
        os.makedirs(os.path.dirname(abs_path), exist_ok=True)
        # remove the // file: line from the code
        body = re.sub(r"//\s*file:\s*\S+\n?", "", chunk, count=1)
        with open(abs_path, "w") as f:
            f.write(body)
        files_created.append(abs_path)

    if not files_created:
        return 1, "no files extracted"

    # El archivo principal es el último (generalmente main.zy)
    main = files_created[-1]
    r = subprocess.run(
        [str(ZYMBOL), "run", main],
        capture_output=True, text=True, timeout=10
    )
    combined = r.stdout + r.stderr
    return r.returncode, strip_warnings(combined)


# ── comparación ───────────────────────────────────────────────────────────────

def normalize(s: str) -> str:
    return s.strip()


def compare_output(expected: list[str], actual_raw: str) -> tuple[bool, list[str]]:
    """
    Compara líneas de output real con líneas esperadas.
    Retorna (ok, mismatches).
    """
    actual_lines = [l for l in actual_raw.splitlines() if l.strip()]
    mismatches   = []
    ok           = True

    if len(actual_lines) != len(expected):
        ok = False
        mismatches.append(
            f"  líneas esperadas={len(expected)} obtenidas={len(actual_lines)}"
        )
        # Muestra diferencias punto a punto hasta el mínimo
        for i in range(min(len(expected), len(actual_lines))):
            if normalize(actual_lines[i]) != normalize(expected[i]):
                mismatches.append(f"  [{i+1}] esperado: {expected[i]!r}  obtenido: {actual_lines[i]!r}")
        return ok, mismatches

    for i, (exp, got) in enumerate(zip(expected, actual_lines)):
        if is_prose_expected(exp):
            pass  # descripción en prosa — saltar comparación
        elif is_wildcard(exp):
            # Comparación parcial: el output debe comenzar con la parte antes de '...'
            prefix = exp.split("...")[0].strip()
            if prefix and not normalize(got).startswith(prefix):
                ok = False
                mismatches.append(f"  [{i+1}] esperado (wild): {exp!r}  obtenido: {got!r}")
        elif normalize(got) != normalize(exp):
            ok = False
            mismatches.append(f"  [{i+1}] esperado: {exp!r}  obtenido: {got!r}")

    return ok, mismatches


# ── main ──────────────────────────────────────────────────────────────────────

def main():
    text   = GUIDE.read_text()
    blocks = extract_blocks(text)

    results = []
    counts  = {"PASS": 0, "FAIL": 0, "ERROR": 0, "SKIP": 0}

    with tempfile.TemporaryDirectory() as tmpdir:
        for b in blocks:
            idx     = b["index"]
            section = b["section"]
            code    = b["code"]

            expected = collect_expected(code)

            # Sin anotaciones → no verificable
            if not expected:
                r = Result(idx, section, "SKIP", "sin anotaciones // →")
                results.append(r)
                counts["SKIP"] += 1
                continue

            # Ejemplos de error intencional
            if is_error_example(code):
                r = Result(idx, section, "SKIP", "ejemplo de error intencional")
                results.append(r)
                counts["SKIP"] += 1
                continue

            # Bloques que dependen de CLI args
            if is_cli_args(code):
                r = Result(idx, section, "SKIP", "requiere argumentos CLI (><)")
                results.append(r)
                counts["SKIP"] += 1
                continue

            # Sesiones de REPL (líneas con '> ')
            if is_repl_session(code):
                r = Result(idx, section, "SKIP", "sesión REPL — no ejecutable como archivo")
                results.append(r)
                counts["SKIP"] += 1
                continue

            # Importa módulos externos no definidos en el bloque
            if has_external_import(code):
                r = Result(idx, section, "SKIP", "importa módulo externo — necesita setup")
                results.append(r)
                counts["SKIP"] += 1
                continue

            # Anotaciones sólo descriptivas (no valores literales)
            if is_description_only(expected):
                r = Result(idx, section, "SKIP", "anotación descriptiva — sin valor literal")
                results.append(r)
                counts["SKIP"] += 1
                continue

            # Ejecutar
            sub_tmpdir = os.path.join(tmpdir, f"block_{idx:03d}")
            os.makedirs(sub_tmpdir, exist_ok=True)

            try:
                if is_multi_file(code):
                    rc, output = run_multi_file(code, sub_tmpdir)
                else:
                    rc, output = run_single(code, sub_tmpdir)
            except subprocess.TimeoutExpired:
                r = Result(idx, section, "ERROR", "timeout (>10s)", expected)
                results.append(r)
                counts["ERROR"] += 1
                continue
            except Exception as e:
                r = Result(idx, section, "ERROR", str(e), expected)
                results.append(r)
                counts["ERROR"] += 1
                continue

            if rc != 0:
                # Puede ser error real o sólo warnings que ya se filtraron
                if not output:
                    r = Result(idx, section, "ERROR", f"exit {rc} sin output", expected)
                    results.append(r)
                    counts["ERROR"] += 1
                    continue
                # Si queda output tras filtrar warnings, es un error real
                if re.search(r"(error:|Error:|Runtime error)", output):
                    r = Result(idx, section, "ERROR",
                               f"exit {rc}: {output.splitlines()[0][:120]}",
                               expected, output.splitlines())
                    results.append(r)
                    counts["ERROR"] += 1
                    continue
                # De lo contrario comparar normalmente (rc=0 con warnings → rc≠0 pero output útil)

            ok, mismatches = compare_output(expected, output)
            status = "PASS" if ok else "FAIL"
            r = Result(idx, section, status,
                       "\n".join(mismatches) if not ok else "",
                       expected, output.splitlines())
            results.append(r)
            counts[status] += 1

    # ── Reporte ───────────────────────────────────────────────────────────────

    total = len(blocks)
    tested = counts["PASS"] + counts["FAIL"] + counts["ERROR"]

    print(f"\n{'='*72}")
    print(f"  GUIDE.md — verificación de ejemplos ejecutables")
    print(f"  Bloques totales: {total}  |  Probados: {tested}  |  Omitidos: {counts['SKIP']}")
    print(f"  PASS={counts['PASS']}  FAIL={counts['FAIL']}  ERROR={counts['ERROR']}")
    print(f"{'='*72}\n")

    for r in results:
        if r.status == "SKIP":
            continue
        symbol = {"PASS": "✓", "FAIL": "✗", "ERROR": "!"}.get(r.status, "?")
        print(f"  [{r.status}] #{r.index:03d}  §{r.section}")
        if r.status != "PASS":
            print(f"         {r.reason}")
            if r.actual:
                print(f"         output real: {r.actual[:3]}")
            print()

    # Salida con código de error si hay fallos
    if counts["FAIL"] > 0 or counts["ERROR"] > 0:
        sys.exit(1)


if __name__ == "__main__":
    main()
