# Dot Convention for Module Names

## 🎯 Purpose

The **dot prefix (`.`)** in module names indicates that the file comes from a **subdirectory**, solving the ambiguity between file names and folder structures.

---

## 📝 Convention

```zymbol
# .folder_file   → File is located at: folder/file.zy
# file            → File is located at: file.zy (same level)
```

### **Why?**

Without the dot, it's impossible to distinguish:
- `# matematicas_module` → Could be `matematicas_module.zy` (file) OR `matematicas/module.zy` (folder/file)

With the dot:
- `# .matematicas_module` → **Clearly** `matematicas/module.zy` (folder/file)
- `# matematicas_module` → **Clearly** `matematicas_module.zy` (single file)

---

## ✅ Examples

### Directory Structure
```
my_project/
├── matematicas/
│   ├── module.zy       → # .matematicas_module
│   ├── ko.zy           → # .matematicas_ko
│   ├── el.zy           → # .matematicas_el
│   └── he.zy           → # .matematicas_he
├── utils.zy            → # utils
└── app.zy              → # app
```

### File: `matematicas/module.zy`
```zymbol
# .matematicas_module

#> {
    sumar
    restar
    PI
}

PI := 3.14159

sumar(a, b) {
    <~ a + b
}
```

**Interpretation:**
- `.` = File is in a folder
- `matematicas_module` = `matematicas` (folder) + `module` (file)
- Physical path: `matematicas/module.zy`

---

### File: `matematicas/ko.zy`
```zymbol
# .matematicas_ko

#> {
    mat::sumar <= 더하다
    mat.PI <= 파이
}

<# ./module <= mat
```

**Interpretation:**
- `.` = File is in a folder
- `matematicas_ko` = `matematicas` (folder) + `ko` (file)
- Physical path: `matematicas/ko.zy`

---

### File: `utils.zy` (root level)
```zymbol
# utils

#> {
    helper_function
}

helper_function() {
    <~ "helper"
}
```

**Interpretation:**
- No dot = File is at root level
- Physical path: `utils.zy`

---

## 🔍 Ambiguity Resolution

### Scenario 1: Underscore in File Name

**Without dot convention:**
```
matematicas_advanced.zy  → # matematicas_advanced
```
Is this `matematicas_advanced.zy` or `matematicas/advanced.zy`? **Ambiguous!**

**With dot convention:**
```
matematicas/advanced.zy  → # .matematicas_advanced  (folder/file)
matematicas_advanced.zy  → # matematicas_advanced   (single file)
```
**Clear!**

---

### Scenario 2: Nested Folders

```
libs/
└── math/
    └── core/
        └── module.zy  → # .math_core_module
```

**Path breakdown:**
- `.` → File is in folders
- `math_core_module` → `libs/math/core/module.zy`

**Alternative (deeper nesting):**
```
libs/math/core/advanced.zy  → # .math_core_advanced
```

---

## 📊 Comparison Table

| File Path | Module Name | Explanation |
|-----------|-------------|-------------|
| `app.zy` | `# app` | Root level file |
| `utils.zy` | `# utils` | Root level file |
| `math/module.zy` | `# .math_module` | Folder: `math`, File: `module` |
| `math/ko.zy` | `# .math_ko` | Folder: `math`, File: `ko` |
| `ui/components.zy` | `# .ui_components` | Folder: `ui`, File: `components` |
| `core/advanced.zy` | `# .core_advanced` | Folder: `core`, File: `advanced` |

---

## 🛠️ Implementation

### Parser Support

Modified `crates/zymbol-parser/src/lib.rs` to accept optional leading dot:

```rust
// Parse module name (supports optional leading dot for folder indication)
// Syntax: # .folder_file or # file
let mut name = String::new();

// Check for optional leading dot (indicates file is in a folder)
if matches!(self.peek().kind, TokenKind::Dot) {
    name.push('.');
    self.advance(); // consume dot
}

// Parse identifier after optional dot
match &self.peek().kind {
    TokenKind::Ident(ident) => {
        name.push_str(ident);
        self.advance();
    }
    _ => {
        return Err(Diagnostic::error("expected module name after '#'")
            .with_span(self.peek().span.clone()))
    }
}
```

---

## ✅ Benefits

1. **Clarity**: Immediately know if file is in a folder
2. **No Ambiguity**: Clear distinction between file names and paths
3. **Consistency**: Principle of least surprise
4. **Scalability**: Works with deeply nested structures
5. **Simplicity**: Single character (`.`) conveys the information

---

## 📚 Usage Guidelines

### When to Use Dot Prefix

**Use `.` when:**
- File is inside a subdirectory
- Module name combines folder + file name
- Example: `matematicas/ko.zy` → `# .matematicas_ko`

**Don't use `.` when:**
- File is at root level
- Module name is just the file name
- Example: `app.zy` → `# app`

---

## 🔄 Migration from Old Convention

### Before (Ambiguous)
```zymbol
// File: matematicas/module.zy
# matematicas

// File: matematicas/ko.zy
# matematicas_ko
```
**Problem:** Can't tell if `matematicas` is folder or file name

### After (Clear)
```zymbol
// File: matematicas/module.zy
# .matematicas_module

// File: matematicas/ko.zy
# .matematicas_ko
```
**Clear:** Dot indicates these are in folders

---

## 🌍 Real-World Example

```
game_engine/
├── physics/
│   ├── module.zy       # .physics_module
│   ├── collision.zy    # .physics_collision
│   └── gravity.zy      # .physics_gravity
├── graphics/
│   ├── module.zy       # .graphics_module
│   ├── renderer.zy     # .graphics_renderer
│   └── shaders.zy      # .graphics_shaders
└── main.zy             # main
```

**Usage:**
```zymbol
<# ./physics/module <= physics
<# ./graphics/renderer <= gfx

physics::init()
gfx::render_frame()
```

---

## 📖 Summary

- **`.` prefix** = File in a subdirectory
- **No `.` prefix** = File at root level
- **Format**: `# .folder_file` or `# file`
- **Benefit**: No ambiguity, clear structure

---

**Version:** 1.0.0
**Status:** Implemented and tested
**Last Updated:** 2025-12-29
