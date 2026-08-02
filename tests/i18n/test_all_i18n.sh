#!/usr/bin/env bash
# =============================================================================
# test_all_i18n.sh — the code-i18n pattern, end to end
#
#   bash tests/i18n/test_all_i18n.sh
#
# One Spanish base module, four translation layers, four consumer programs.
# See I18N.md for the pattern this exercises.
#
# expected_compare.sh already diffs these programs against their .expected
# files under the tree-walker. This script covers what that one does not:
#
#   · zymbol check on every module — a translation layer that runs fine can
#     still be wrong. E001 (module name vs file path) went unnoticed here for
#     months because `run` does not check a module reached through an import.
#   · the register VM — module code is the likeliest place for the two engines
#     to diverge, and a translation layer is module code by definition.
#
# Override the binary with ZYMBOL=/path/to/zymbol.
# =============================================================================
set -u
cd "$(dirname "$0")"

ZYMBOL="${ZYMBOL:-$(command -v zymbol || echo ../../target/release/zymbol)}"
if ! command -v "$ZYMBOL" >/dev/null 2>&1 && [ ! -x "$ZYMBOL" ]; then
    echo "zymbol not found: $ZYMBOL (build it, or set ZYMBOL=/path/to/zymbol)"
    exit 1
fi

fallo=0

# language · translation module · consumer program · a line only a correct run prints
casos=(
    "Greek|matematicas/ελληνικά.zy|Ελληνική_εφαρμογή.zy|Άθροισμα: 15"
    "Hebrew|matematicas/עִברִית.zy|אפליקציית_מתמטיקה.zy|סכום: 15"
    "Korean|matematicas/한국인.zy|한국_앱.zy|합계: 15"
    "Mandarin|matematicas/中文.zy|中文_应用.zy|和: 15"
)

echo "═══ Zymbol i18n — translation layers ═══"
echo "  base module : matematicas/module.zy (Spanish)"
echo "  binary      : $ZYMBOL"
echo ""

# The base has to check clean too — every layer re-exports from it.
if "$ZYMBOL" check matematicas/module.zy > /dev/null 2>&1; then
    echo "  ok    check   matematicas/module.zy"
else
    echo "  FAIL  check   matematicas/module.zy"
    "$ZYMBOL" check matematicas/module.zy 2>&1 | sed 's/^/          /'
    fallo=1
fi
echo ""

for caso in "${casos[@]}"; do
    IFS='|' read -r idioma modulo programa marca <<< "$caso"
    printf "─── %s\n" "$idioma"

    if "$ZYMBOL" check "$modulo" > /dev/null 2>&1; then
        printf "  ok    check   %s\n" "$modulo"
    else
        printf "  FAIL  check   %s\n" "$modulo"
        "$ZYMBOL" check "$modulo" 2>&1 | sed 's/^/          /'
        fallo=1
    fi

    salida_tw=$("$ZYMBOL" run "$programa" 2>&1)
    if echo "$salida_tw" | grep -qF "$marca"; then
        printf "  ok    run     %s\n" "$programa"
    else
        printf "  FAIL  run     %s — expected '%s'\n" "$programa" "$marca"
        echo "$salida_tw" | sed 's/^/          /'
        fallo=1
    fi

    salida_vm=$("$ZYMBOL" run --vm "$programa" 2>&1)
    if [ "$salida_vm" = "$salida_tw" ]; then
        printf "  ok    --vm    identical to the tree-walker\n"
    else
        printf "  FAIL  --vm    diverges from the tree-walker\n"
        diff <(echo "$salida_tw") <(echo "$salida_vm") | sed 's/^/          /'
        fallo=1
    fi
    echo ""
done

if [ "$fallo" -eq 0 ]; then
    echo "═══ every translation layer checks, runs and agrees across engines ═══"
else
    echo "═══ FAILED ═══"
    exit 1
fi
