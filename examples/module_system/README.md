# Zymbol Module System Examples

This directory contains examples demonstrating the Zymbol v1.1.0 module system.

## Directory Structure

```
examples/module_system/
├── README.md                    # This file
├── simple_example.zl            # Basic module usage
├── app.zl                       # Complete example with re-exports
├── lib/
│   ├── math_utils.zl            # Math utilities module
│   ├── text_utils.zl            # Text utilities module
│   └── core_library.zl          # Facade module (re-exports)
└── utils/
    └── config.zl                # Configuration constants module
```

## Examples

### 1. Simple Example (`simple_example.zl`)

Demonstrates basic module usage:
- Importing modules with required alias
- Calling functions with `::` syntax
- Accessing constants with `.` syntax
- Using multiple modules in one script

**Run**: `zymbol run simple_example.zl`

### 2. Complete Application (`app.zl`)

Demonstrates advanced features:
- Re-exported functions (original names)
- Re-exported functions (renamed)
- Re-exported constants (original names)
- Re-exported constants (renamed)
- Facade module pattern
- Direct module imports
- Path resolution (`./lib/`, `./utils/`)

**Run**: `zymbol run app.zl`

## Module Files

### `lib/math_utils.zl`

Features demonstrated:
- Module declaration: `# math_utils`
- Export block: `#> { add, subtract, multiply, PI, E }`
- Public constants: `PI := 3.14159`
- Public functions: `add(a, b)`
- Private items (not exported)

### `lib/text_utils.zl`

Features demonstrated:
- String manipulation functions
- Public constant `MAX_LENGTH`
- Private helper function `is_whitespace`
- Default private visibility

### `lib/core_library.zl`

Features demonstrated:
- **Facade pattern**: Re-exports from multiple modules
- **Import syntax**: `<# ./math_utils <= math`
- **Re-export functions (original)**: `math::add`
- **Re-export constants (original)**: `math.PI`
- **Re-export functions (renamed)**: `math::subtract <= minus`
- **Re-export constants (renamed)**: `cfg.APP_NAME <= APPLICATION_NAME`
- **Own functions**: `process_data`, `calculate_area`
- **Own constants**: `LIBRARY_VERSION`
- **Path resolution**: `../utils/config` (parent directory)

### `utils/config.zl`

Features demonstrated:
- Configuration constants module
- Public constants: `APP_NAME`, `VERSION`, `DEBUG_MODE`
- Private constant: `INTERNAL_KEY`
- Function returning constant data

## Key Concepts Demonstrated

### 1. Module Declaration
```zymbol
# module_name    // Must match file name
```

### 2. Export Block
```zymbol
#> {
    public_func
    PUBLIC_CONST
}
```

### 3. Import with Alias (REQUIRED)
```zymbol
<# ./path/to/module <= alias
```

### 4. Function Calls (::)
```zymbol
result = math::add(5, 3)
```

### 5. Constant Access (.)
```zymbol
value = math.PI
```

### 6. Re-Export (Facade Pattern)

**Functions** - Use `::` (same as calling):
```zymbol
#> {
    math::add              // Original name
    math::subtract <= minus   // Renamed
}
```

**Constants** - Use `.` (same as accessing):
```zymbol
#> {
    math.PI                // Original name
    math.E <= EULER        // Renamed
}
```

### 7. Symbol Consistency

| Item Type | Access Syntax | Re-Export Syntax | Re-Export Renamed |
|-----------|---------------|------------------|-------------------|
| Function  | `math::add()` | `math::add`      | `math::add <= sum` |
| Constant  | `math.PI`     | `math.PI`        | `math.PI <= PI_VALUE` |

**Rule**: Use the same symbol for re-export as you use for access.

### 8. Path Resolution

- `./module` - Current directory
- `./lib/module` - Subdirectory
- `../module` - Parent directory
- `../utils/config` - Parent directory, then subdirectory
- Simple paths treated as `./`: `module` → `./module`

### 9. Visibility Rules

- **Default**: Everything is private
- **Public**: Only items listed in `#>` block
- **Re-export**: Can only re-export public items from imported modules

### 10. File Naming Rule

**CRITICAL**: File name must match module name

✅ Correct:
- File: `math_utils.zl` → Module: `# math_utils`

❌ Incorrect:
- File: `math_utils.zl` → Module: `# math` (ERROR)

## Testing the Grammar

These examples serve as test cases for the Zymbol v1.1.0 grammar defined in `zymbol.ebnf`.

### Parser Test Points

1. **Module declaration**: `# identifier`
2. **Export block**: `#> { items }`
3. **Export own item**: `identifier`
4. **Re-export function**: `alias::function`
5. **Re-export constant**: `alias.CONSTANT`
6. **Re-export renamed function**: `alias::function <= new_name`
7. **Re-export renamed constant**: `alias.CONSTANT <= NEW_NAME`
8. **Import with alias**: `<# path <= alias`
9. **Path resolution**: `./`, `../`, subdirectories
10. **Function call**: `alias::function(args)`
11. **Constant access**: `alias.CONSTANT`

### Semantic Test Points

1. File name matches module name
2. Cannot re-export private items
3. Cannot re-export non-existent items
4. No name conflicts in exports
5. Imported alias is required
6. Module must be imported before use
7. Re-exported items are accessible to consumers

## Benefits Demonstrated

1. **Encapsulation**: Private items hidden from consumers
2. **Clear API**: Explicit export blocks define public interface
3. **Code Organization**: Facade pattern aggregates related functionality
4. **Backward Compatibility**: Rename re-exports for compatibility layers
5. **Type Safety**: Visual distinction between functions (`::`) and constants (`.`)
6. **Predictability**: File name = module name
7. **Flexibility**: Any `.zl` file can be executed (no "main.zl" restriction)

## Next Steps

After validating these examples work correctly with the parser:

1. Test error cases (invalid exports, circular imports, etc.)
2. Create more complex examples (nested modules, larger hierarchies)
3. Document error messages
4. Create migration guide for v1.0.0 users
