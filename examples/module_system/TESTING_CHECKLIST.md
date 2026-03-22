# Module System Testing Checklist

Use this checklist to validate the module system implementation.

## Phase 1: Lexer/Tokenization

### Symbols
- [ ] `#` token recognized (module declaration)
- [ ] `#>` token recognized (export block)
- [ ] `<#` token recognized (import statement)
- [ ] `<=` token recognized (alias operator)
- [ ] `::` token recognized (scope resolution)
- [ ] `.` token recognized (member access)

### Longest Match Rule
- [ ] `#>` tokenizes as single token, not `#` + `>`
- [ ] `#1` tokenizes as boolean true (not affected by `#>`)
- [ ] `#0` tokenizes as boolean false (not affected by `#>`)
- [ ] `#|expr|` numeric eval works (not affected by `#>`)
- [ ] `#?` type metadata works (not affected by export block)

## Phase 2: Parser

### Module Declaration
- [ ] `# identifier` parses correctly
- [ ] `# identifier ;` with semicolon parses
- [ ] `# identifier #> { }` with export block parses
- [ ] Error on `#` without identifier

### Export Block
- [ ] `#> { }` empty export block parses
- [ ] `#> { item }` single item parses
- [ ] `#> { item1, item2 }` multiple items parse
- [ ] `#> { item1; item2; }` semicolons between items parse
- [ ] Mixed commas and semicolons parse

### Export Items
- [ ] `identifier` (own item) parses
- [ ] `alias::function` (re-export function) parses
- [ ] `alias.CONSTANT` (re-export constant) parses
- [ ] `alias::function <= new_name` (renamed function) parses
- [ ] `alias.CONSTANT <= NEW_NAME` (renamed constant) parses

### Import Statement
- [ ] `<# path <= alias` parses
- [ ] `<# ./path <= alias` with `./` parses
- [ ] `<# ../path <= alias` with `../` parses
- [ ] `<# ./dir/subdir/module <= alias` nested paths parse
- [ ] `<# path <= alias ;` with semicolon parses
- [ ] Error on `<# path` without alias

### Module Path
- [ ] Simple identifier: `module`
- [ ] Current directory: `./module`
- [ ] Subdirectory: `./dir/module`
- [ ] Parent directory: `../module`
- [ ] Multiple parent levels: `../../module`
- [ ] Complex: `../../dir/subdir/module`

## Phase 3: AST Construction

### Module Declaration Node
```rust
ModuleDecl {
    name: String,
    export_block: Option<ExportBlock>,
}
```
- [ ] Module name stored correctly
- [ ] Export block optional and stored when present

### Export Block Node
```rust
ExportBlock {
    items: Vec<ExportItem>,
}
```
- [ ] All export items collected
- [ ] Order preserved

### Export Item Nodes
```rust
enum ExportItem {
    Own(String),
    ReExport {
        module_alias: String,
        item_name: String,
        item_type: ItemType,      // Function or Constant
        rename: Option<String>,
    },
}

enum ItemType {
    Function,    // Uses ::
    Constant,    // Uses .
}
```
- [ ] Own items create `Own` variant
- [ ] `alias::func` creates `ReExport` with `ItemType::Function`
- [ ] `alias.CONST` creates `ReExport` with `ItemType::Constant`
- [ ] Rename stored correctly in `ReExport::rename`

### Import Node
```rust
ImportStmt {
    path: ModulePath,
    alias: String,
}

struct ModulePath {
    components: Vec<String>,
    is_relative: bool,      // true for ./ or ../
    parent_levels: usize,   // 0 for ./, 1 for ../, 2 for ../../
}
```
- [ ] Path components extracted correctly
- [ ] Relative vs absolute paths distinguished
- [ ] Parent directory levels counted
- [ ] Alias stored

## Phase 4: Semantic Analysis

### File Name Validation
- [ ] File `math_utils.zl` with `# math_utils` → ✅ OK
- [ ] File `math_utils.zl` with `# math` → ❌ Error E001
- [ ] File `math_utils.zl` with `# MathUtils` → ❌ Error E001 (case-sensitive)

### Path Resolution
- [ ] `<# ./math_utils <= m` finds `./math_utils.zl`
- [ ] `<# math_utils <= m` finds `./math_utils.zl` (implicit `./`)
- [ ] `<# ./lib/math <= m` finds `./lib/math.zl`
- [ ] `<# ../utils/config <= c` finds `../utils/config.zl`
- [ ] Non-existent path → ❌ Error E003

### Import Validation
- [ ] Imported module exists
- [ ] Imported module has valid syntax
- [ ] Alias is unique (no duplicate aliases in same file)
- [ ] No circular imports detected

### Export Validation - Own Items
- [ ] Own item must be defined in module (function or constant)
- [ ] No duplicate own items in export block
- [ ] Own item name conflicts checked

### Export Validation - Re-Exports
- [ ] Module alias is imported
- [ ] Item exists in source module
- [ ] Item is public in source module (in source's `#>` block)
- [ ] Symbol matches item type:
  - [ ] Function uses `::` → ✅ OK
  - [ ] Function uses `.` → ❌ Error E010
  - [ ] Constant uses `.` → ✅ OK
  - [ ] Constant uses `::` → ❌ Error E010
- [ ] Rename is valid identifier (if present)
- [ ] No name conflicts in export block

### Symbol Resolution
- [ ] `alias::function()` resolves to imported module's function
- [ ] `alias.CONSTANT` resolves to imported module's constant
- [ ] Undefined module → ❌ Error E011
- [ ] Undefined function in module → ❌ Error
- [ ] Undefined constant in module → ❌ Error

### Circular Import Detection
Test chain: `A → B → C → A`
- [ ] Direct circular: `A → B → A` detected
- [ ] Indirect circular: `A → B → C → A` detected
- [ ] Error reports full chain
- [ ] No false positives on valid DAG

### Export Table Construction
For each module, build export table:
```rust
HashMap<String, ExportEntry>

struct ExportEntry {
    name: String,           // Name visible to consumers
    source: ExportSource,
}

enum ExportSource {
    Own,
    ReExported {
        from_module: String,
        original_name: String,
    },
}
```
- [ ] Own items added with `ExportSource::Own`
- [ ] Re-exports added with source module and original name
- [ ] Renamed re-exports use new name as key
- [ ] Table used for consumer validation

## Phase 5: Runtime/Interpreter

### Module Loading
- [ ] Module loaded only once (cached)
- [ ] Import statements execute before module body
- [ ] Module-level constants initialized
- [ ] Module-level variables initialized

### Function Calls
- [ ] `alias::function()` calls correct function
- [ ] Re-exported function works
- [ ] Re-exported renamed function works
- [ ] Arguments passed correctly
- [ ] Return values handled

### Constant Access
- [ ] `alias.CONSTANT` returns correct value
- [ ] Re-exported constant accessible
- [ ] Re-exported renamed constant accessible
- [ ] Constants are immutable

### Visibility Enforcement
- [ ] Can access exported items
- [ ] Cannot access non-exported items
- [ ] Re-exported items behave as if defined locally

## Phase 6: Integration Tests

### Test 1: Basic Module Usage
File: `simple_example.zl`
- [ ] Runs without errors
- [ ] Produces expected output
- [ ] All function calls work
- [ ] All constant accesses work

### Test 2: Re-Export Facade
File: `app.zl`
- [ ] Runs without errors
- [ ] Re-exported functions (original names) work
- [ ] Re-exported functions (renamed) work
- [ ] Re-exported constants (original names) work
- [ ] Re-exported constants (renamed) work
- [ ] Own facade functions work

### Test 3: Path Resolution
- [ ] Current directory imports work
- [ ] Subdirectory imports work
- [ ] Parent directory imports work
- [ ] Complex relative paths work

### Test 4: Private Items
- [ ] Private functions not accessible from consumers
- [ ] Private constants not accessible from consumers
- [ ] Attempting to access private items → error

### Test 5: Multiple Imports
- [ ] Single file imports multiple modules
- [ ] No alias conflicts
- [ ] All modules accessible independently

## Phase 7: Error Handling

For each error in `ERROR_EXAMPLES.md`:

- [ ] E001: Module name mismatch detected and reported
- [ ] E002: Invalid module name detected
- [ ] E003: Module file not found detected
- [ ] E004: Circular import detected
- [ ] E005: Invalid path syntax detected
- [ ] E006: Re-export non-existent item detected
- [ ] E007: Re-export private item detected
- [ ] E008: Export name conflict detected
- [ ] E009: Missing import alias detected
- [ ] E010: Wrong re-export symbol detected
- [ ] E011: Undefined module detected
- [ ] E012: Invalid rename identifier detected

### Error Message Quality
- [ ] Error messages include line numbers
- [ ] Error messages include file names
- [ ] Error messages explain the problem
- [ ] Error messages suggest fixes
- [ ] Error messages are user-friendly

## Phase 8: Edge Cases

### Empty Modules
- [ ] Module with no exports works
- [ ] Module with empty export block `#> { }` works
- [ ] Module with only imports works

### Re-Export Chains
Module A exports `func1`
Module B re-exports A's `func1`
Module C re-exports B's `func1`
- [ ] Consumer can use C's re-exported `func1`
- [ ] Function executes correctly

### Namespace Collisions
- [ ] Module name different from alias works
- [ ] Multiple modules with same name in different paths
- [ ] Alias prevents name conflicts

### Unicode Support
- [ ] Module names with Unicode identifiers
- [ ] Item names with Unicode identifiers
- [ ] Emoji in identifiers (as per Zymbol spec)

### Case Sensitivity
- [ ] `Math` vs `math` treated as different
- [ ] File name case must match module name case

## Phase 9: Performance

### Module Loading
- [ ] Each module loaded exactly once
- [ ] Circular import detection is O(n) not O(n!)
- [ ] Large module hierarchies don't cause stack overflow

### Export Table Lookup
- [ ] Export table uses hash map for O(1) lookup
- [ ] No linear searches in hot paths

## Phase 10: Documentation Validation

- [ ] All examples in `MODULE_SYSTEM_PROPOSAL.md` parse correctly
- [ ] All examples in `MODULE_RE_EXPORT.md` parse correctly
- [ ] All examples in `MODULE_SYNTAX_QUICK_REF.md` parse correctly
- [ ] All examples in `CLAUDE.md` parse correctly
- [ ] Grammar in `zymbol.ebnf` matches implementation

## Completion Criteria

The module system is complete when:

✅ **Parser**
- All syntax cases parse correctly
- AST nodes constructed properly
- Error cases rejected with syntax errors

✅ **Semantic Analyzer**
- All validation rules implemented
- All error codes (E001-E012) detected
- Export tables built correctly
- Circular imports detected

✅ **Runtime**
- Module loading works
- Function calls work (own and re-exported)
- Constant access works (own and re-exported)
- Visibility enforced correctly

✅ **Testing**
- All examples run successfully
- All error cases detected
- Edge cases handled
- Documentation validated

✅ **Documentation**
- User guide complete
- Error reference complete
- Examples comprehensive
- Migration guide for v1.0.0 users

## Test Execution Order

1. **Lexer tests** (Phase 1) - Run first
2. **Parser tests** (Phase 2) - Requires working lexer
3. **AST tests** (Phase 3) - Requires working parser
4. **Semantic tests** (Phase 4) - Requires working AST
5. **Runtime tests** (Phase 5) - Requires working semantic analysis
6. **Integration tests** (Phase 6) - Requires complete implementation
7. **Error tests** (Phase 7) - Can run throughout
8. **Edge case tests** (Phase 8) - After integration tests pass
9. **Performance tests** (Phase 9) - After functionality complete
10. **Documentation validation** (Phase 10) - Final validation

## Success Metrics

- [ ] 100% of valid syntax examples parse
- [ ] 100% of error cases detected
- [ ] 0% false positives in error detection
- [ ] All integration tests pass
- [ ] All documentation examples work
