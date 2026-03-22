#!/bin/bash
# Test all i18n translation examples

set -e

ZYMBOL="../../target/release/zymbol"

echo "==================================="
echo "Zymbol i18n Translation Tests"
echo "==================================="
echo ""

# Test Korean
echo "🇰🇷 Testing Korean (한국어)..."
$ZYMBOL run app_coreano.zy > /tmp/zymbol_ko.out
if grep -q "합계: 15" /tmp/zymbol_ko.out; then
    echo "   ✅ Korean translation: PASSED"
else
    echo "   ❌ Korean translation: FAILED"
    exit 1
fi
echo ""

# Test Greek
echo "🇬🇷 Testing Greek (Ελληνικά)..."
$ZYMBOL run app_griego.zy > /tmp/zymbol_el.out
if grep -q "Άθροισμα: 15" /tmp/zymbol_el.out; then
    echo "   ✅ Greek translation: PASSED"
else
    echo "   ❌ Greek translation: FAILED"
    exit 1
fi
echo ""

# Test Hebrew
echo "🇮🇱 Testing Hebrew (עברית)..."
$ZYMBOL run app_hebreo.zy > /tmp/zymbol_he.out
if grep -q "סכום: 15" /tmp/zymbol_he.out; then
    echo "   ✅ Hebrew translation: PASSED"
else
    echo "   ❌ Hebrew translation: FAILED"
    exit 1
fi
echo ""

echo "==================================="
echo "All i18n tests PASSED! 🎉"
echo "==================================="
echo ""
echo "Summary:"
echo "  ✅ Korean (ko) - 더하다, 빼다, 파이"
echo "  ✅ Greek (el) - προσθέτω, αφαιρώ, ΠΙ"
echo "  ✅ Hebrew (he) - חיבור, חיסור, פאי"
echo ""
