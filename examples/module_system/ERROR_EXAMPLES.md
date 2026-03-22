# Module System Error Examples

This document shows common errors and their expected error messages.

## Error 1: File Name Doesn't Match Module Name

**File**: `math_helpers.zl`

```zymbol
# math_utils    // ❌ ERROR

#> { add }

add(a, b) {
    <~ a + b
}
```

**Expected Error**:
```
Error E001: Module name mismatch
  File: math_helpers.zl
  Module declared as: math_utils
  Expected: # math_helpers

Help: Module name must match file name (without .zl extension)
```

---

## Error 2: Re-Exporting Non-Existent Item

**File**: `facade.zl`

```zymbol
# facade

<# ./math_utils <= math

#> {
    math::add           // ✅ OK (exists)
    math::divide        // ❌ ERROR (not exported by math_utils)
}
```

**Expected Error**:
```
Error: Cannot re-export 'divide' from module 'math_utils'
  Reason: 'divide' is not exported by 'math_utils'

Available exports from 'math_utils':
  - add
  - subtract
  - multiply
  - PI
  - E
```

---

## Error 3: Re-Exporting Private Item

**File**: `bad_reexport.zl`

```zymbol
# bad_reexport

<# ./math_utils <= math

#> {
    math::internal_round    // ❌ ERROR (private in math_utils)
}
```

**Expected Error**:
```
Error: Cannot re-export 'internal_round' from module 'math_utils'
  Reason: 'internal_round' is private (not exported)

Public items from 'math_utils':
  - add
  - subtract
  - multiply
  - PI
  - E
```

---

## Error 4: Name Conflict in Exports

**File**: `conflicting.zl`

```zymbol
# conflicting

<# ./math_utils <= math
<# ./text_utils <= text

#> {
    math::add
    process         // Own function
    text::concat <= process    // ❌ ERROR (conflicts with own function)
}

process(data) {
    <~ data
}
```

**Expected Error**:
```
Error: Export name conflict
  Name: 'process'

Conflicting declarations:
  1. Own function 'process' (line 10)
  2. Re-export of 'text::concat' renamed to 'process' (line 7)

Help: Use a different rename for the re-export
  Suggestion: text::concat <= text_concat
```

---

## Error 5: Missing Import Alias

**File**: `no_alias.zl`

```zymbol
# no_alias

<# ./math_utils        // ❌ ERROR (missing <= alias)

result = math_utils::add(5, 3)
```

**Expected Error**:
```
Syntax Error: Import statement requires alias
  Line 3: <# ./math_utils
                         ^
  Expected: <= identifier

Example: <# ./math_utils <= math
```

---

## Error 6: Wrong Symbol for Item Type

**File**: `wrong_symbol.zl`

```zymbol
# wrong_symbol

<# ./math_utils <= math

#> {
    math.add        // ❌ ERROR ('add' is a function, use ::)
    math::PI        // ❌ ERROR ('PI' is a constant, use .)
}
```

**Expected Error**:
```
Error: Incorrect re-export symbol for item type
  Line 6: math.add

  'add' is a FUNCTION - use :: for functions
  Correct: math::add

Error: Incorrect re-export symbol for item type
  Line 7: math::PI

  'PI' is a CONSTANT - use . for constants
  Correct: math.PI
```

---

## Error 7: Circular Import

**File**: `module_a.zl`

```zymbol
# module_a

<# ./module_b <= b    // ❌ ERROR (circular dependency)

#> { func_a }

func_a() {
    <~ b::func_b()
}
```

**File**: `module_b.zl`

```zymbol
# module_b

<# ./module_a <= a    // ❌ ERROR (circular dependency)

#> { func_b }

func_b() {
    <~ a::func_a()
}
```

**Expected Error**:
```
Error: Circular import detected
  Import chain:
    module_a → module_b → module_a

  Location:
    module_a.zl (line 3): <# ./module_b <= b
    module_b.zl (line 3): <# ./module_a <= a

Help: Refactor modules to break circular dependency
  - Extract shared functionality to a third module
  - Use dependency injection
  - Redesign module boundaries
```

---

## Error 8: Using Module Without Import

**File**: `no_import.zl`

```zymbol
# no_import

result = math::add(5, 3)    // ❌ ERROR ('math' not imported)
```

**Expected Error**:
```
Error: Undefined module 'math'
  Line 3: result = math::add(5, 3)
                   ^^^^

  'math' has not been imported

Help: Add import statement:
  <# ./math_utils <= math
```

---

## Error 9: Re-Export Conflict (Same Name from Different Modules)

**File**: `reexport_conflict.zl`

```zymbol
# reexport_conflict

<# ./math_utils <= math
<# ./text_utils <= text

#> {
    math::concat        // Hypothetical function
    text::concat        // ❌ ERROR (name conflict)
}
```

**Expected Error**:
```
Error: Re-export name conflict
  Name: 'concat'

Conflicting re-exports:
  1. math::concat (line 6)
  2. text::concat (line 7)

Help: Rename one or both re-exports to avoid conflict
  Examples:
    math::concat <= math_concat
    text::concat <= text_concat
```

---

## Error 10: Invalid Rename Identifier

**File**: `bad_rename.zl`

```zymbol
# bad_rename

<# ./math_utils <= math

#> {
    math::add <= 123invalid    // ❌ ERROR (invalid identifier)
}
```

**Expected Error**:
```
Syntax Error: Invalid identifier for rename
  Line 6: math::add <= 123invalid
                       ^^^^^^^^^^

  Identifiers cannot start with digits

Help: Use a valid identifier
  Example: math::add <= add_renamed
```

---

## Error 11: Missing Export Block for Re-Export

**File**: `reexport_without_export.zl`

```zymbol
# reexport_without_export

<# ./math_utils <= math

// ❌ ERROR: Re-export requires #> block
// Cannot use math::add in other files without re-exporting
```

**Expected Error** (when consumer tries to use it):
```
Error: 'add' is not exported by module 'reexport_without_export'

  Module 'reexport_without_export' imports 'math_utils' but does not
  re-export 'add' in its #> block.

Help: Add to export block in reexport_without_export.zl:
  #> {
      math::add
  }
```

---

## Error 12: Path Not Found

**File**: `missing_path.zl`

```zymbol
# missing_path

<# ./nonexistent/module <= mod    // ❌ ERROR (path doesn't exist)
```

**Expected Error**:
```
Error E003: Module file not found
  Import: <# ./nonexistent/module
  Expected file: ./nonexistent/module.zl

  The specified file does not exist.

Help: Check the path and file name
  - Ensure the directory exists: ./nonexistent/
  - Ensure the file exists: module.zl
  - Check for typos in the path
```

---

## Summary of Error Codes

| Code | Error | Description |
|------|-------|-------------|
| E001 | Module name mismatch | File name doesn't match module declaration |
| E002 | Invalid module name | Module name is not a valid identifier |
| E003 | Module file not found | Import path doesn't exist |
| E004 | Circular import | Module import cycle detected |
| E005 | Invalid path | Path syntax is malformed |
| E006 | Re-export non-existent | Trying to re-export item that doesn't exist |
| E007 | Re-export private | Trying to re-export private item |
| E008 | Export name conflict | Duplicate names in export block |
| E009 | Missing import alias | Import statement missing `<= alias` |
| E010 | Wrong re-export symbol | Using `.` for function or `::` for constant |
| E011 | Undefined module | Using module that wasn't imported |
| E012 | Invalid rename identifier | Rename target is not valid identifier |

---

## Testing Error Handling

To properly test the module system, the implementation should:

1. **Detect** all these errors during semantic analysis
2. **Report** clear error messages with line numbers
3. **Suggest** fixes where possible
4. **Prevent** runtime errors by catching issues early

These error examples should be used to:
- Validate semantic analyzer implementation
- Write test cases for error detection
- Generate helpful error messages
- Create user documentation
