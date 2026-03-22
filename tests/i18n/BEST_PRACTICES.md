# Zymbol Module Organization - Best Practices

## Rust-style Module Structure ✅

Following Rust's module system conventions provides better scalability and maintainability.

---

## Directory Structure

### ✅ **RECOMMENDED: Rust-style (Directory + module.zy)**

```
my_project/
├── matematicas/              ← Module directory
│   ├── module.zy             ← Main module (like Rust's mod.rs)
│   ├── ko.zy                 ← Korean translation
│   ├── el.zy                 ← Greek translation
│   ├── he.zy                 ← Hebrew translation
│   ├── ja.zy                 ← Japanese translation
│   └── zh.zy                 ← Chinese translation
├── ui_components/
│   ├── module.zy             ← UI framework (original)
│   ├── es.zy                 ← Spanish translation
│   └── fr.zy                 ← French translation
└── app.zy                    ← Application entry point
```

**Benefits:**
- ✅ Scalable: Easy to add new translations
- ✅ Organized: Related files grouped together
- ✅ Clean namespace: Root directory stays clean
- ✅ Discoverable: All translations in one place
- ✅ Standard: Follows Rust conventions

---

### ❌ **NOT RECOMMENDED: Flat structure**

```
my_project/
├── matematicas.zy           ← Original module
├── matematicas_ko.zy        ← Korean
├── matematicas_el.zy        ← Greek
├── matematicas_he.zy        ← Hebrew
├── matematicas_ja.zy        ← Japanese
├── matematicas_zh.zy        ← Chinese
├── ui_components.zy         ← UI framework
├── ui_components_es.zy      ← Spanish
├── ui_components_fr.zy      ← French
└── app.zy
```

**Problems:**
- ❌ Cluttered: Root directory becomes crowded
- ❌ Hard to navigate: Files scattered
- ❌ Name collisions: Prefixes pollute namespace
- ❌ Poor discoverability: Hard to find related files

---

## File Naming

### Main Module

**Pattern:** `module.zy` (fixed name, like Rust's `mod.rs`)

```zymbol
# module

#> {
    function1
    function2
    CONSTANT
}

function1() { <~ "result" }
CONSTANT := 42
```

### Translation Files

**Pattern:** `{lang_code}.zy` (ISO 639-1)

```zymbol
# ko

#> {
    mod::function1 <= 함수1
    mod::function2 <= 함수2
    mod.CONSTANT <= 상수
}

<# ./module <= mod
```

---

## Import Patterns

### Importing Original Module

```zymbol
<# ./matematicas/module <= math

result = math::sumar(5, 3)
```

### Importing Translation

```zymbol
<# ./matematicas/ko <= 수학

결과 = 수학::더하다(5, 3)
```

### Importing Multiple Translations

```zymbol
<# ./matematicas/ko <= 한국수학
<# ./matematicas/ja <= 日本数学

// Use Korean names
한국결과 = 한국수학::더하다(10, 5)

// Use Japanese names
日本結果 = 日本数学::足す(10, 5)
```

---

## Adding New Translations

### Step 1: Create Translation File

```bash
# Create new translation file
cd my_module/
touch pt.zy  # Portuguese translation
```

### Step 2: Write Translation

File: `my_module/pt.zy`

```zymbol
# pt

#> {
    mod::add <= adicionar
    mod::subtract <= subtrair
    mod::multiply <= multiplicar
    mod.PI <= PI
}

<# ./module <= mod
```

### Step 3: Use Translation

```zymbol
<# ./my_module/pt <= matematica

soma = matematica::adicionar(10, 5)
pi_valor = matematica.PI
```

---

## Real-World Example

### Directory Structure

```
calculator/
├── core/
│   ├── module.zy         ← Core math (English)
│   ├── es.zy             ← Spanish
│   ├── fr.zy             ← French
│   ├── de.zy             ← German
│   └── it.zy             ← Italian
├── ui/
│   ├── module.zy         ← UI components (English)
│   ├── ko.zy             ← Korean
│   ├── ja.zy             ← Japanese
│   └── zh.zy             ← Chinese
└── apps/
    ├── calculator_es.zy  ← Spanish app
    ├── calculator_ko.zy  ← Korean app
    └── calculator_ja.zy  ← Japanese app
```

### Spanish App Example

File: `apps/calculator_es.zy`

```zymbol
<# ../core/es <= matematicas
<# ../ui/es <= interfaz

// Use Spanish names throughout
resultado = matematicas::sumar(10, 5)
interfaz::mostrar_resultado(resultado)
```

### Korean App Example

File: `apps/calculator_ko.zy`

```zymbol
<# ../core/ko <= 수학
<# ../ui/ko <= 인터페이스

// Use Korean names throughout
결과 = 수학::더하다(10, 5)
인터페이스::결과표시(결과)
```

---

## Module Composition

### Nested Modules

```
game_engine/
├── physics/
│   ├── module.zy         ← Physics engine
│   └── ja.zy             ← Japanese
├── graphics/
│   ├── module.zy         ← Graphics engine
│   └── ko.zy             ← Korean
└── audio/
    ├── module.zy         ← Audio engine
    └── zh.zy             ← Chinese
```

### Importing Nested Modules

```zymbol
<# ./game_engine/physics/ko <= 물리엔진
<# ./game_engine/graphics/ko <= 그래픽엔진

물리엔진::중력계산(9.8)
그래픽엔진::화면렌더링()
```

---

## Language Code Reference (ISO 639-1)

| Code | Language | Example Usage |
|------|----------|---------------|
| `en` | English | `module/en.zy` |
| `es` | Spanish | `module/es.zy` |
| `ko` | Korean | `module/ko.zy` |
| `ja` | Japanese | `module/ja.zy` |
| `zh` | Chinese | `module/zh.zy` |
| `fr` | French | `module/fr.zy` |
| `de` | German | `module/de.zy` |
| `it` | Italian | `module/it.zy` |
| `pt` | Portuguese | `module/pt.zy` |
| `ru` | Russian | `module/ru.zy` |
| `ar` | Arabic | `module/ar.zy` |
| `hi` | Hindi | `module/hi.zy` |
| `el` | Greek | `module/el.zy` |
| `he` | Hebrew | `module/he.zy` |
| `th` | Thai | `module/th.zy` |

---

## Migration Guide

### From Flat to Rust-style

**Before:**
```
matematicas.zy
matematicas_ko.zy
matematicas_el.zy
```

**After:**
```bash
mkdir matematicas
mv matematicas.zy matematicas/module.zy
mv matematicas_ko.zy matematicas/ko.zy
mv matematicas_el.zy matematicas/el.zy

# Update imports in translation files
sed -i 's/<# \.\/matematicas/<# .\/module/' matematicas/*.zy

# Update imports in apps
sed -i 's/<# \.\/matematicas_ko/<# .\/matematicas\/ko/' app.zy
```

---

## Summary

✅ **DO:**
- Use directory structure for modules
- Name main module `module.zy`
- Use ISO 639-1 codes for translations
- Group related translations together
- Follow Rust conventions

❌ **DON'T:**
- Use flat file structure
- Mix naming conventions
- Use custom language codes
- Scatter translations across directories
- Pollute root namespace

---

**References:**
- Rust Module System: https://doc.rust-lang.org/book/ch07-00-managing-growing-projects-with-packages-crates-and-modules.html
- ISO 639-1 Language Codes: https://en.wikipedia.org/wiki/List_of_ISO_639-1_codes
