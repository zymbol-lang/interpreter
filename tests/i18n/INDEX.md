# Zymbol i18n Examples - File Index

Complete index of all files in the i18n demonstration directory.

## Directory Structure

```
tests/i18n/
├── matematicas/              ← Module directory (Rust-style)
│   ├── module.zy             ← Main module (Spanish)
│   ├── ko.zy                 ← Korean translation
│   ├── el.zy                 ← Greek translation
│   └── he.zy                 ← Hebrew translation
├── app_coreano.zy            ← Korean example app
├── app_griego.zy             ← Greek example app
├── app_hebreo.zy             ← Hebrew example app
├── test_all_i18n.sh          ← Automated test suite
├── README.md                 ← Main documentation
├── BEST_PRACTICES.md         ← Rust-style organization guide
└── INDEX.md                  ← This file
```

---

## Module Files

### `matematicas/module.zy`
**Purpose:** Main mathematics module (original, Spanish)  
**Module name:** `# module`  
**Exports:**
- Functions: `sumar`, `restar`, `multiplicar`, `dividir`
- Constants: `PI`, `E`

**Usage:**
```zymbol
<# ./matematicas/module <= mat
result = mat::sumar(10, 5)
```

---

### `matematicas/ko.zy`
**Purpose:** Korean translation of matematicas module  
**Module name:** `# ko`  
**Language:** 한국어 (Korean)  
**Exports:**
- `더하다` (sumar → add)
- `빼다` (restar → subtract)
- `곱하다` (multiplicar → multiply)
- `나누다` (dividir → divide)
- `파이` (PI)
- `이` (E → Euler's number)

**Usage:**
```zymbol
<# ./matematicas/ko <= 수학
결과 = 수학::더하다(10, 5)
```

---

### `matematicas/el.zy`
**Purpose:** Greek translation of matematicas module  
**Module name:** `# el`  
**Language:** Ελληνικά (Greek)  
**Exports:**
- `προσθέτω` (sumar → add)
- `αφαιρώ` (restar → subtract)
- `πολλαπλασιάζω` (multiplicar → multiply)
- `διαιρώ` (dividir → divide)
- `ΠΙ` (PI)
- `Ε` (E)

**Usage:**
```zymbol
<# ./matematicas/el <= μαθ
αποτέλεσμα = μαθ::προσθέτω(10, 5)
```

---

### `matematicas/he.zy`
**Purpose:** Hebrew translation of matematicas module  
**Module name:** `# he`  
**Language:** עברית (Hebrew)  
**Exports:**
- `חיבור` (sumar → add)
- `חיסור` (restar → subtract)
- `כפל` (multiplicar → multiply)
- `חילוק` (dividir → divide)
- `פאי` (PI)
- `אי` (E)

**Usage:**
```zymbol
<# ./matematicas/he <= מתמטיקה
תוצאה = מתמטיקה::חיבור(10, 5)
```

---

## Application Files

### `app_coreano.zy`
**Purpose:** Example application using Korean translation  
**Imports:** `matematicas/ko.zy`  
**Demonstrates:**
- Importing translated module
- Using Korean function names
- Using Korean constant names
- Console output in Korean

**Output:**
```
합계: 15
차이: 5
곱셈: 50
나눗셈: 2
파이: 3.14159
자연상수: 2.71828
```

---

### `app_griego.zy`
**Purpose:** Example application using Greek translation  
**Imports:** `matematicas/el.zy`  
**Demonstrates:**
- Importing translated module
- Using Greek function names
- Using Greek constant names
- Console output in Greek

**Output:**
```
Άθροισμα: 15
Διαφορά: 5
Γινόμενο: 50
Διαίρεση: 2
Πι: 3.14159
Ε: 2.71828
```

---

### `app_hebreo.zy`
**Purpose:** Example application using Hebrew translation  
**Imports:** `matematicas/he.zy`  
**Demonstrates:**
- Importing translated module
- Using Hebrew function names
- Using Hebrew constant names
- Console output in Hebrew (RTL)

**Output:**
```
סכום: 15
הפרש: 5
מכפלה: 50
חלוקה: 2
פאי: 3.14159
אי: 2.71828
```

---

## Testing & Documentation

### `test_all_i18n.sh`
**Purpose:** Automated test suite for all translations  
**Type:** Shell script  
**Tests:**
- Korean translation correctness
- Greek translation correctness
- Hebrew translation correctness

**Usage:**
```bash
./test_all_i18n.sh
```

---

### `README.md`
**Purpose:** Main documentation and user guide  
**Contents:**
- Overview of i18n system
- Directory structure explanation
- How to use translations
- Example code snippets
- Naming conventions
- Creating new translations

---

### `BEST_PRACTICES.md`
**Purpose:** Guide for Rust-style module organization  
**Contents:**
- Recommended vs not recommended structures
- Migration guide (flat → Rust-style)
- File naming conventions
- Import patterns
- Real-world examples
- Language code reference (ISO 639-1)

---

### `INDEX.md`
**Purpose:** Complete file listing and reference  
**Contents:**
- This document
- Quick reference for all files
- Usage examples for each file

---

## Quick Reference

### Running Examples

```bash
# Run Korean example
zymbol run app_coreano.zy

# Run Greek example
zymbol run app_griego.zy

# Run Hebrew example
zymbol run app_hebreo.zy

# Run all tests
./test_all_i18n.sh
```

### Adding New Translation

```bash
# 1. Create translation file
touch matematicas/fr.zy

# 2. Write translation (see BEST_PRACTICES.md)
# 3. Create example app
touch app_frances.zy

# 4. Update test suite (optional)
# Edit test_all_i18n.sh
```

---

## File Count

- **Module files:** 4 (1 original + 3 translations)
- **Application files:** 3 (one per language)
- **Test files:** 1
- **Documentation files:** 3
- **Total:** 11 files

---

## Language Support

| Language | Code | Module File | App File | Status |
|----------|------|-------------|----------|--------|
| Spanish (original) | - | `module.zy` | - | ✅ |
| Korean | `ko` | `ko.zy` | `app_coreano.zy` | ✅ |
| Greek | `el` | `el.zy` | `app_griego.zy` | ✅ |
| Hebrew | `he` | `he.zy` | `app_hebreo.zy` | ✅ |

---

## Last Updated

2025-12-29

## Version

i18n Examples v1.0.0 (Rust-style organization)
