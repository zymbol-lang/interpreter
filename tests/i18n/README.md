# Zymbol i18n (Internationalization) Examples

This directory demonstrates Zymbol's internationalization capabilities using module re-exports with aliases.

## Overview

Zymbol allows developers to create **translation layers** for modules, enabling code written in one language (e.g., Spanish) to be used by developers who speak other languages (Korean, Greek, Hebrew, etc.) using native identifiers.

## Directory Structure (Rust-style Best Practice)

```
tests/i18n/
├── matematicas/          ← Module directory
│   ├── module.zy         ← Main module (like Rust's mod.rs)
│   ├── ko.zy             ← Korean translation
│   ├── el.zy             ← Greek translation
│   └── he.zy             ← Hebrew translation
├── app_coreano.zy        ← Example app (Korean)
├── app_griego.zy         ← Example app (Greek)
└── app_hebreo.zy         ← Example app (Hebrew)
```

### Why This Structure?

Following Rust's module system best practices:
- **Scalability**: Group related files in directories
- **Clean namespace**: Avoids polluting root directory
- **Discoverability**: All translations for a module in one place
- **Standard pattern**: `module.zy` ≈ Rust's `mod.rs`

## Files

### Main Module
- `matematicas/module.zy` - Math utilities module in Spanish
  - Module name: `# .matematicas_module` (dot indicates folder)
  - Functions: `sumar`, `restar`, `multiplicar`, `dividir`
  - Constants: `PI`, `E`

### Translation Layers
- `matematicas/ko.zy` - Korean translation (한국어) → `# .matematicas_ko`
- `matematicas/el.zy` - Greek translation (Ελληνικά) → `# .matematicas_el`
- `matematicas/he.zy` - Hebrew translation (עברית) → `# .matematicas_he`

**Dot Convention:** The `.` prefix indicates the file is in a subdirectory. See `DOT_CONVENTION.md` for details.

### Example Applications
- `app_coreano.zy` - App using Korean identifiers
- `app_griego.zy` - App using Greek identifiers
- `app_hebreo.zy` - App using Hebrew identifiers

## How It Works

### 1. Original Module (Spanish)

```zymbol
# matematicas
#> {
    sumar
    restar
    multiplicar
    dividir
    PI
    E
}

PI := 3.14159
E := 2.71828

sumar(a, b) {
    <~ a + b
}
```

### 2. Translation Layer (Korean)

File: `matematicas/ko.zy`

```zymbol
# .matematicas_ko
#> {
    mat::sumar <= 더하다
    mat::restar <= 빼다
    mat::multiplicar <= 곱하다
    mat::dividir <= 나누다
    mat.PI <= 파이
    mat.E <= 이
}

<# ./module <= mat
```

**Note:** The `.` prefix in `# .matematicas_ko` indicates this file is in the `matematicas/` folder. Without the dot, `matematicas_ko` could be ambiguous (is it a file `matematicas_ko.zy` or `matematicas/ko.zy`?).

### 3. Using Translated Names

File: `app_coreano.zy`

```zymbol
<# ./matematicas/ko <= 수학

결과 = 수학::더하다(10, 5)    // Calls matematicas::sumar(10, 5)
원주율 = 수학.파이             // Accesses matematicas.PI
```

## Running the Examples

```bash
# Korean version
zymbol run app_coreano.zy

# Greek version
zymbol run app_griego.zy

# Hebrew version
zymbol run app_hebreo.zy
```

## Output

All examples produce equivalent output with localized labels:

**Korean (한국어):**
```
합계: 15
차이: 5
곱셈: 50
나눗셈: 2
파이: 3.14159
자연상수: 2.71828
```

**Greek (Ελληνικά):**
```
Άθροισμα: 15
Διαφορά: 5
Γινόμενο: 50
Διαίρεση: 2
Πι: 3.14159
Ε: 2.71828
```

**Hebrew (עברית):**
```
סכום: 15
הפרש: 5
מכפלה: 50
חלוקה: 2
פאי: 3.14159
אי: 2.71828
```

## Naming Convention (Rust-style)

Translation modules follow the **ISO 639-1** language code convention within a module directory:

**Directory structure:**
```
module_name/
├── module.zy     ← Main module (original language)
├── ko.zy         ← Korean (한국어)
├── el.zy         ← Greek (Ελληνικά)
├── he.zy         ← Hebrew (עברית)
├── es.zy         ← Spanish (Español)
├── en.zy         ← English
├── ja.zy         ← Japanese (日本語)
├── zh.zy         ← Chinese (中文)
├── ar.zy         ← Arabic (العربية)
├── ru.zy         ← Russian (Русский)
└── hi.zy         ← Hindi (हिन्दी)
```

**Import pattern:**
```zymbol
<# ./module_name/ko <= alias    // Korean translation
<# ./module_name/el <= alias    // Greek translation
<# ./module_name/module <= alias // Original module
```

## Benefits

1. **No Code Changes** - Original module remains untouched
2. **Native Identifiers** - Developers use their own language
3. **Type Safety** - Full type checking across translations
4. **Single Source** - One implementation, many language interfaces
5. **Composable** - Translation layers can be composed
6. **Unicode Support** - Full Unicode identifier support

## Creating Your Own Translation

To create a translation for a module:

1. Create a module directory: `my_module/`
2. Place main module as `my_module/module.zy`
3. Add translation files: `my_module/fr.zy`, `my_module/ja.zy`, etc.
4. In translation files, import from `./module`

**Example:**

Directory structure:
```
my_module/
├── module.zy    ← Original module
└── fr.zy        ← French translation
```

French translation template (`my_module/fr.zy`):
```zymbol
# fr
#> {
    orig::function1 <= fonction1
    orig::function2 <= fonction2
    orig.CONSTANT <= CONSTANTE
}

<# ./module <= orig
```

Usage:
```zymbol
<# ./my_module/fr <= mon_module
mon_module::fonction1()
```

## Technical Details

- **Syntax**: Uses `::` for functions, `.` for constants
- **Re-export**: `alias::item <= new_name`
- **Extension**: All files use `.zy` extension
- **Module System**: v1.1.0 with re-export support
- **Implementation**: 100% functional (Lexer, Parser, Interpreter)

## Supported Languages

Zymbol supports **all Unicode characters** in identifiers, enabling translations to:
- Latin scripts (English, Spanish, French, German, etc.)
- Cyrillic (Russian, Ukrainian, Bulgarian, etc.)
- Asian languages (Chinese, Japanese, Korean, Thai, etc.)
- Middle Eastern (Arabic, Hebrew, Persian, etc.)
- Indian languages (Hindi, Tamil, Bengali, etc.)
- And many more!

## License

See main project LICENSE file.
