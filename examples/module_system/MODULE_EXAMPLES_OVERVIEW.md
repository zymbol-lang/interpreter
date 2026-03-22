# Module System Examples - Complete Overview

This document provides a complete overview of all module system examples created for Zymbol v1.1.0.

## Files Created

### Executable Examples (.zl)

| File | Purpose | Features Demonstrated |
|------|---------|----------------------|
| **simple_example.zl** | Basic module usage | Import with alias, function calls (::), constant access (.) |
| **app.zl** | Complete application | Re-exports, facades, renamed items, path resolution |
| **lib/math_utils.zl** | Math utilities module | Module declaration, export block, public/private items |
| **lib/text_utils.zl** | Text utilities module | String functions, constants, visibility control |
| **lib/core_library.zl** | Facade module | Re-export original names, re-export renamed, own items |
| **utils/config.zl** | Configuration module | Constants module, public/private distinction |

### Documentation Files (.md)

| File | Purpose | Content |
|------|---------|---------|
| **README.md** | Main guide | Directory structure, how to run examples, concepts explained |
| **ERROR_EXAMPLES.md** | Error reference | 12 common errors with expected error messages |
| **TESTING_CHECKLIST.md** | Testing guide | Complete checklist for implementation validation |
| **MODULE_EXAMPLES_OVERVIEW.md** | This file | Complete overview of all examples |

## Directory Structure

```
examples/module_system/
│
├── README.md                       # Main documentation
├── ERROR_EXAMPLES.md               # Error cases and messages
├── TESTING_CHECKLIST.md            # Implementation testing guide
├── MODULE_EXAMPLES_OVERVIEW.md     # This overview
│
├── simple_example.zl               # Basic usage example
├── app.zl                          # Complete application example
│
├── lib/                            # Library modules subdirectory
│   ├── math_utils.zl               # Math module
│   ├── text_utils.zl               # Text module
│   └── core_library.zl             # Facade module (re-exports)
│
└── utils/                          # Utilities subdirectory
    └── config.zl                   # Configuration module
```

## Quick Start

### Run Simple Example
```bash
cd examples/module_system
zymbol run simple_example.zl
```

**What it demonstrates:**
- Importing modules with `<# path <= alias`
- Calling functions with `alias::function()`
- Accessing constants with `alias.CONSTANT`

### Run Complete Application
```bash
cd examples/module_system
zymbol run app.zl
```

**What it demonstrates:**
- All features of simple example, plus:
- Re-exported functions (original and renamed)
- Re-exported constants (original and renamed)
- Facade pattern
- Path resolution with `./` and `../`

## Feature Matrix

| Feature | simple_example.zl | app.zl | math_utils.zl | text_utils.zl | core_library.zl | config.zl |
|---------|-------------------|--------|---------------|---------------|-----------------|-----------|
| Module declaration (`#`) | - | - | ✅ | ✅ | ✅ | ✅ |
| Export block (`#>`) | - | - | ✅ | ✅ | ✅ | ✅ |
| Import (`<#`) | ✅ | ✅ | - | - | ✅ | - |
| Function call (`::`) | ✅ | ✅ | - | - | - | - |
| Constant access (`.`) | ✅ | ✅ | - | - | - | - |
| Re-export function | - | ✅ | - | - | ✅ | - |
| Re-export constant | - | ✅ | - | - | ✅ | - |
| Re-export renamed | - | ✅ | - | - | ✅ | - |
| Public items | - | - | ✅ | ✅ | ✅ | ✅ |
| Private items | - | - | ✅ | ✅ | ✅ | ✅ |
| Own + re-export mix | - | - | - | - | ✅ | - |
| Path resolution (`./`) | ✅ | ✅ | - | - | ✅ | - |
| Path resolution (`../`) | - | ✅ | - | - | ✅ | - |

## Learning Path

### Step 1: Understand Module Declaration
**Read**: `lib/math_utils.zl`

Key points:
- Module name must match file name
- Export block declares public API
- Items not in export block are private

### Step 2: Import and Use Modules
**Read**: `simple_example.zl`

Key points:
- Import requires alias: `<# path <= alias`
- Call functions: `alias::function()`
- Access constants: `alias.CONSTANT`

### Step 3: Path Resolution
**Read**: `simple_example.zl` and `lib/core_library.zl`

Key points:
- Current directory: `./lib/module`
- Parent directory: `../utils/module`
- Subdirectories: `./dir/subdir/module`

### Step 4: Re-Export Pattern
**Read**: `lib/core_library.zl`

Key points:
- Re-export with original name: `alias::function`
- Re-export with rename: `alias::function <= new_name`
- Mix own items and re-exports in same block
- Symbol consistency: `::` for functions, `.` for constants

### Step 5: Complete Application
**Read**: `app.zl`

Key points:
- Use re-exported items like native items
- Renamed items accessed by new name
- Facade module simplifies imports

### Step 6: Error Cases
**Read**: `ERROR_EXAMPLES.md`

Key points:
- File name must match module name
- Cannot re-export private items
- Cannot re-export non-existent items
- Symbol must match item type

## Code Walkthrough

### Example 1: Basic Module (`lib/math_utils.zl`)

```zymbol
# math_utils                    # Module declaration (matches file name)

#> {                            # Export block starts
    add                         # Export function 'add'
    subtract                    # Export function 'subtract'
    multiply                    # Export function 'multiply'
    PI                          # Export constant 'PI'
    E                           # Export constant 'E'
}                               # Export block ends

PI := 3.14159                   # Public constant (in export block)
E := 2.71828                    # Public constant (in export block)

INTERNAL_PRECISION := 0.0001    # Private constant (NOT in export block)

add(a, b) {                     # Public function (in export block)
    <~ a + b
}

subtract(a, b) {                # Public function (in export block)
    <~ a - b
}

multiply(a, b) {                # Public function (in export block)
    <~ a * b
}

internal_round(value) {         # Private function (NOT in export block)
    <~ value
}
```

**Visibility:**
- ✅ Public: `add`, `subtract`, `multiply`, `PI`, `E`
- ❌ Private: `INTERNAL_PRECISION`, `internal_round`

### Example 2: Using Modules (`simple_example.zl`)

```zymbol
<# ./lib/math_utils <= math     # Import module with alias 'math'
<# ./lib/text_utils <= text     # Import module with alias 'text'

result1 = math::add(10, 20)     # Call function with ::
result2 = math::subtract(50, 15) # Call function with ::

pi = math.PI                    # Access constant with .
max = text.MAX_LENGTH           # Access constant with .
```

**Key points:**
- Alias is required: `<= math` (cannot omit)
- Functions use `::` (scope resolution)
- Constants use `.` (member access)

### Example 3: Re-Export Facade (`lib/core_library.zl`)

```zymbol
# core_library

<# ./math_utils <= math         # Import source module
<# ./text_utils <= text         # Import source module
<# ../utils/config <= cfg       # Import from parent directory

#> {
    // Re-export functions (original names) - use ::
    math::add                   # Re-export 'add' as 'add'
    math::multiply              # Re-export 'multiply' as 'multiply'

    // Re-export constants (original names) - use .
    math.PI                     # Re-export 'PI' as 'PI'
    math.E                      # Re-export 'E' as 'E'

    // Re-export functions (renamed) - use ::
    math::subtract <= minus     # Re-export 'subtract' as 'minus'
    text::trim <= strip         # Re-export 'trim' as 'strip'

    // Re-export constants (renamed) - use .
    cfg.APP_NAME <= APPLICATION_NAME  # Re-export with new name

    // Own items
    process_data                # Own function
    LIBRARY_VERSION             # Own constant
}

LIBRARY_VERSION := "2.0.0"      # Own constant

process_data(value) {           # Own function
    result = math::add(value, 10)    # Use imported function internally
    doubled = math::multiply(result, 2)
    <~ doubled
}
```

**Key points:**
- Re-export syntax matches access syntax
- Can rename during re-export with `<=`
- Mix own items with re-exports
- Can use imports internally

### Example 4: Consumer (`app.zl`)

```zymbol
<# ./lib/core_library <= core   # Import facade module

// Use re-exported items (original names)
sum = core::add(5, 3)           # Call re-exported function
pi = core.PI                    # Access re-exported constant

// Use re-exported items (renamed)
diff = core::minus(10, 3)       # Call renamed function (was 'subtract')
app = core.APPLICATION_NAME     # Access renamed constant (was 'APP_NAME')

// Use facade's own items
processed = core::process_data(100)  # Call facade's own function
version = core.LIBRARY_VERSION       # Access facade's own constant
```

**Benefits:**
- Single import gives access to many modules
- Renamed items more intuitive
- Facade provides cohesive API

## Symbol Reference

### Module System Symbols

| Symbol | Name | Usage | Example |
|--------|------|-------|---------|
| `#` | Module declaration | Declare module | `# module_name` |
| `#>` | Export block | Define public API | `#> { item1, item2 }` |
| `<#` | Import statement | Import module | `<# ./path <= alias` |
| `<=` | Alias operator | Assign alias / rename | `<# path <= alias` or `item <= new` |
| `::` | Scope resolution | Function calls | `alias::function()` |
| `.` | Member access | Constants | `alias.CONSTANT` |

### Symbol Consistency Rule

**For re-exports, use the same symbol as for access:**

| Item Type | Access Syntax | Re-Export Original | Re-Export Renamed |
|-----------|---------------|-------------------|-------------------|
| Function | `math::add()` | `math::add` | `math::add <= sum` |
| Constant | `math.PI` | `math.PI` | `math.PI <= PI_VALUE` |

## Common Patterns

### Pattern 1: Utility Library
```zymbol
# utils

#> {
    helper1
    helper2
    CONSTANT1
}

CONSTANT1 := 100

helper1() { <~ #1 }
helper2() { <~ #0 }
```

### Pattern 2: Aggregator Facade
```zymbol
# facade

<# ./module_a <= a
<# ./module_b <= b

#> {
    a::func1
    a::func2
    b::func3
    b::func4
}
```

### Pattern 3: Renaming Facade
```zymbol
# compat_layer

<# ./new_api <= api

#> {
    api::new_function <= old_function    # Backward compatibility
    api::updated_func <= legacy_func
}
```

### Pattern 4: Namespace Flattening
```zymbol
# common

<# ./deep/path/module_a <= a
<# ./deep/path/module_b <= b

#> {
    a::func <= common_func_a
    b::func <= common_func_b
}
```

## Testing These Examples

### Prerequisites
- Zymbol interpreter with module system support (v1.1.0+)
- All example files in correct directory structure

### Run Tests
```bash
# Test basic usage
zymbol run simple_example.zl

# Test complete application
zymbol run app.zl

# Test each module individually (if interpreter supports module execution)
zymbol check lib/math_utils.zl
zymbol check lib/text_utils.zl
zymbol check lib/core_library.zl
zymbol check utils/config.zl
```

### Expected Output

**simple_example.zl:**
```
10 + 20 = 30
50 - 15 = 35
7 * 8 = 56

Circle with radius 10:
  Circumference: 62.8318
  Area: 314.159

Original: 'alice'
Uppercase: 'ALICE'
Trimmed: 'hello world'
Concatenated: 'Hello, ALICE'

Maximum text length: 1000
```

**app.zl:**
```
Sum: 8
Product: 20
Uppercase: HELLO

PI: 3.14159
E: 2.71828
Max Length: 1000

Difference: 7
Trimmed: 'text'
Joined: 'Hello World'
Config path: /etc/zymbol/config.zl

Application: Zymbol App
Version: 1.1.0
Debug mode: #1

Processed data: 220
Circle area (r=5): 78.53975

Core library version: 2.0.0

Direct access - App: Zymbol App
Max connections: 100
```

## Next Steps

After validating these examples work:

1. **Review Error Cases**: Check `ERROR_EXAMPLES.md` for error handling
2. **Use Testing Checklist**: Follow `TESTING_CHECKLIST.md` for implementation
3. **Create More Examples**: Add domain-specific examples
4. **Write Tests**: Create automated tests based on these examples
5. **Document Edge Cases**: Document any discovered edge cases

## Questions and Troubleshooting

### Q: Can I execute a module file directly?
A: Yes! Any `.zl` file can be executed, even if it has a module declaration and export block. The export block is only used when the module is imported by another file.

### Q: What if I want to use a module without an alias?
A: Not supported. All imports must have an alias. This makes code more readable and prevents naming conflicts.

### Q: Can I import the same module with different aliases?
A: Yes, you can import the same module multiple times with different aliases if needed, though this is rarely useful.

### Q: Can I re-export an already re-exported item?
A: Yes! Re-export chains work. If A exports `func`, B re-exports A's `func`, and C re-exports B's `func`, consumers of C can use `func`.

### Q: What happens if I forget to export an item?
A: It becomes private and cannot be accessed from other modules. This is by design (default private).

### Q: Can I mix positional and named syntax in re-exports?
A: No. You must use `::` for functions and `.` for constants consistently.

## Summary

These examples provide:
- ✅ Complete working code demonstrating all module features
- ✅ Clear progression from simple to complex
- ✅ Real-world patterns (facade, aggregator, renaming)
- ✅ Error cases with expected messages
- ✅ Testing checklist for implementation
- ✅ Documentation and reference materials

Use these examples as:
- **Learning material** for Zymbol users
- **Test cases** for implementation validation
- **Reference implementation** for grammar specification
- **Documentation examples** for user guides
