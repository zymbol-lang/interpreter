# Module Dependency Graph

Visual representation of module dependencies in the example system.

## Simple Example Dependencies

```
simple_example.zl
    │
    ├──> lib/math_utils.zl
    │
    └──> lib/text_utils.zl
```

**Dependency Count**: 2 direct imports

---

## Complete Application Dependencies

```
app.zl
    │
    ├──> lib/core_library.zl (facade)
    │         │
    │         ├──> lib/math_utils.zl
    │         │
    │         ├──> lib/text_utils.zl
    │         │
    │         └──> utils/config.zl
    │
    └──> utils/config.zl (direct)
```

**Dependency Count**:
- app.zl: 2 direct imports (core_library, config)
- core_library.zl: 3 direct imports (math_utils, text_utils, config)
- Total modules involved: 5

---

## Detailed Dependency Analysis

### 1. Leaf Modules (No dependencies)

These modules don't import anything:

```
lib/math_utils.zl
    Dependencies: NONE
    Exports: add, subtract, multiply, PI, E

lib/text_utils.zl
    Dependencies: NONE
    Exports: uppercase, lowercase, trim, concat, MAX_LENGTH

utils/config.zl
    Dependencies: NONE
    Exports: APP_NAME, VERSION, DEBUG_MODE, MAX_CONNECTIONS, get_config_path
```

### 2. Intermediate Modules (Facade)

These modules aggregate other modules:

```
lib/core_library.zl
    Dependencies:
        └─> lib/math_utils.zl
        └─> lib/text_utils.zl
        └─> utils/config.zl

    Exports (Re-exported):
        From math_utils:
            - math::add (function, original name)
            - math::multiply (function, original name)
            - math::subtract <= minus (function, renamed)
            - math.PI (constant, original name)
            - math.E (constant, original name)

        From text_utils:
            - text::uppercase (function, original name)
            - text::lowercase (function, original name)
            - text::trim <= strip (function, renamed)
            - text::concat <= join (function, renamed)
            - text.MAX_LENGTH (constant, original name)

        From config:
            - cfg::get_config_path <= config_path (function, renamed)
            - cfg.APP_NAME <= APPLICATION_NAME (constant, renamed)
            - cfg.VERSION <= APP_VERSION (constant, renamed)
            - cfg.DEBUG_MODE <= DEBUG (constant, renamed)

    Exports (Own):
        - process_data (function)
        - calculate_area (function)
        - LIBRARY_VERSION (constant)
```

### 3. Consumer Modules (Applications)

These modules use the system:

```
simple_example.zl
    Dependencies:
        └─> lib/math_utils.zl (direct)
        └─> lib/text_utils.zl (direct)

    Uses:
        - math::add, math::subtract, math::multiply
        - math.PI
        - text::uppercase, text::trim, text::concat
        - text.MAX_LENGTH

app.zl
    Dependencies:
        └─> lib/core_library.zl (facade)
        └─> utils/config.zl (direct)

    Uses (via core_library):
        - core::add, core::multiply (re-exported from math_utils)
        - core::minus (re-exported from math_utils, renamed)
        - core.PI, core.E (re-exported from math_utils)
        - core::uppercase, core::lowercase (re-exported from text_utils)
        - core::strip, core::join (re-exported from text_utils, renamed)
        - core.MAX_LENGTH (re-exported from text_utils)
        - core::config_path (re-exported from config, renamed)
        - core.APPLICATION_NAME, core.APP_VERSION, core.DEBUG (re-exported from config, renamed)
        - core::process_data, core::calculate_area (core_library own)
        - core.LIBRARY_VERSION (core_library own)

    Uses (direct):
        - config.APP_NAME
        - config.MAX_CONNECTIONS
```

---

## Import Graph (ASCII Art)

```
                    ┌─────────────────┐
                    │  simple_example │
                    │      .zl        │
                    └────────┬────────┘
                             │
                   ┌─────────┴─────────┐
                   │                   │
                   ▼                   ▼
            ┌──────────┐        ┌──────────┐
            │math_utils│        │text_utils│
            │   .zl    │        │   .zl    │
            └──────────┘        └──────────┘


                    ┌─────────────────┐
                    │      app        │
                    │      .zl        │
                    └────────┬────────┘
                             │
                   ┌─────────┴─────────┐
                   │                   │
                   ▼                   ▼
            ┌──────────┐        ┌──────────┐
            │  core    │        │  config  │
            │ library  │        │   .zl    │
            │   .zl    │        └──────────┘
            └────┬─────┘
                 │
        ┌────────┼────────┐
        │        │        │
        ▼        ▼        ▼
   ┌────────┐┌────────┐┌────────┐
   │  math  ││  text  ││ config │
   │ utils  ││ utils  ││  .zl   │
   │  .zl   ││  .zl   ││        │
   └────────┘└────────┘└────────┘
```

---

## Re-Export Flow

### How re-exports work in core_library.zl

```
Source Module         Facade Module              Consumer
─────────────         ─────────────              ────────

math_utils.zl         core_library.zl            app.zl
  exports:              imports:                   imports:
  - add               <# math_utils <= math      <# core_library <= core
  - subtract
  - multiply          re-exports:                uses:
  - PI                #> {                       - core::add
  - E                   math::add                - core::minus
                        math::multiply           - core.PI
                        math::subtract <= minus  - core::process_data
                        math.PI
                        math.E
                      }

                      own items:
                      - process_data() {
                          math::add(...)
                        }
```

**Flow**:
1. `math_utils.zl` exports `add`, `subtract`, `multiply`, `PI`, `E`
2. `core_library.zl` imports `math_utils` as `math`
3. `core_library.zl` re-exports:
   - `math::add` (original name)
   - `math::multiply` (original name)
   - `math::subtract <= minus` (renamed)
   - `math.PI` (original name)
   - `math.E` (original name)
4. `app.zl` imports `core_library` as `core`
5. `app.zl` uses:
   - `core::add()` → calls `math_utils.add()`
   - `core::minus()` → calls `math_utils.subtract()`
   - `core.PI` → accesses `math_utils.PI`

---

## Dependency Levels

### Level 0: No Dependencies
- `lib/math_utils.zl`
- `lib/text_utils.zl`
- `utils/config.zl`

### Level 1: Depends on Level 0
- `lib/core_library.zl`
  - Depends on: math_utils, text_utils, config

### Level 2: Depends on Level 1 (or Level 0)
- `simple_example.zl`
  - Depends on: math_utils, text_utils (Level 0)
- `app.zl`
  - Depends on: core_library (Level 1), config (Level 0)

---

## Module Interaction Matrix

| Module | Imports | Exports | Re-exports | Consumers |
|--------|---------|---------|------------|-----------|
| **math_utils** | - | 5 items | - | core_library, simple_example |
| **text_utils** | - | 5 items | - | core_library, simple_example |
| **config** | - | 5 items | - | core_library, app |
| **core_library** | 3 modules | 17 items | 14 items | app |
| **simple_example** | 2 modules | - | - | - |
| **app** | 2 modules | - | - | - |

---

## Circular Dependency Detection

### Valid Dependencies (DAG - Directed Acyclic Graph)

```
✅ VALID: No cycles detected

math_utils ─┐
text_utils ─┼──> core_library ──> app
config ─────┘         └───────────┘
```

### Invalid Dependencies (Would cause errors)

```
❌ INVALID: Circular dependency

Example 1 (Direct cycle):
    module_a ──> module_b
         ▲           │
         └───────────┘

Example 2 (Indirect cycle):
    module_a ──> module_b ──> module_c
         ▲                        │
         └────────────────────────┘
```

---

## Path Resolution Examples

### From app.zl

```
app.zl location: examples/module_system/

Import: <# ./lib/core_library <= core
Resolves to: examples/module_system/lib/core_library.zl

Import: <# ./utils/config <= config
Resolves to: examples/module_system/utils/config.zl
```

### From core_library.zl

```
core_library.zl location: examples/module_system/lib/

Import: <# ./math_utils <= math
Resolves to: examples/module_system/lib/math_utils.zl

Import: <# ./text_utils <= text
Resolves to: examples/module_system/lib/text_utils.zl

Import: <# ../utils/config <= cfg
Resolves to: examples/module_system/utils/config.zl
```

**Path Resolution Rules**:
- `./` = current directory
- `../` = parent directory
- Multiple `../` levels supported: `../../path`
- Subdirectories: `./subdir/module`

---

## Module Loading Order

When `app.zl` is executed:

```
1. Load app.zl
   └─> Parse module declaration: # app (if present)
   └─> Parse imports:
       ├─> <# ./lib/core_library <= core
       │   └─> Load core_library.zl (if not cached)
       │       └─> Parse imports:
       │           ├─> <# ./math_utils <= math
       │           │   └─> Load math_utils.zl (if not cached)
       │           │       └─> Execute module body
       │           │       └─> Cache export table
       │           ├─> <# ./text_utils <= text
       │           │   └─> Load text_utils.zl (if not cached)
       │           │       └─> Execute module body
       │           │       └─> Cache export table
       │           └─> <# ../utils/config <= cfg
       │               └─> Load config.zl (if not cached)
       │                   └─> Execute module body
       │                   └─> Cache export table
       │       └─> Execute module body
       │       └─> Build export table (with re-exports)
       │       └─> Cache
       └─> <# ./utils/config <= config
           └─> Already cached (loaded by core_library)
           └─> Reuse cached module
   └─> Execute app.zl body
```

**Key Points**:
- Depth-first loading
- Each module loaded exactly once (cached)
- Import-time execution (module bodies run when loaded)
- Export tables built after imports resolved

---

## Visibility Scope

### What can access what?

```
math_utils.zl:
    ├─ Public (exported):
    │    add, subtract, multiply, PI, E
    │    ├─> Can be used by: core_library, simple_example
    │    └─> Can be re-exported by: core_library
    │
    └─ Private (not exported):
         INTERNAL_PRECISION, internal_round
         ├─> Can be used by: math_utils itself
         └─> ❌ Cannot be used by: any other module

core_library.zl:
    ├─ Public (exported):
    │    Own: process_data, calculate_area, LIBRARY_VERSION
    │    Re-exported: add, multiply, minus, PI, E, uppercase, ...
    │    └─> Can be used by: app
    │
    └─ Private (not exported):
         CACHE_SIZE, validate_input
         └─> ❌ Cannot be used by: app
```

---

## Benefits of This Architecture

### 1. Separation of Concerns
- Math utilities isolated in `math_utils`
- Text utilities isolated in `text_utils`
- Configuration isolated in `config`

### 2. Facade Pattern
- `core_library` provides unified API
- Consumer only needs one import
- Implementation details hidden

### 3. Flexibility
- Can import modules directly (`simple_example`)
- Or use facade (`app`)
- Choice based on needs

### 4. Maintainability
- Clear dependency graph
- No circular dependencies
- Easy to trace data flow

### 5. Reusability
- Leaf modules reusable independently
- Facade can be used as template
- Consistent pattern across examples

---

## Summary

**Total Modules**: 6 (3 leaf, 1 facade, 2 consumers)

**Dependencies**:
- Leaf modules: 0 dependencies each
- Facade: 3 dependencies
- Consumers: 2 dependencies each

**Export Types**:
- Own items: 15 total
- Re-exported items: 14 total

**Module Types**:
- Utility modules: math_utils, text_utils
- Configuration: config
- Facade: core_library
- Applications: simple_example, app

This architecture demonstrates a clean, maintainable module system with clear separation of concerns and no circular dependencies.
