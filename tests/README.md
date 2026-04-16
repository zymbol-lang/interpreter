# Zymbol Test Suite

End-to-end tests for the Zymbol interpreter. Each test is a `.zy` source file paired with a `.expected` golden output file. Tests run against the tree-walker interpreter unless noted.

## Directory Structure

```
tests/
├── arithmetic/          # Numeric operators, precedence, int/float arithmetic
├── casts/               # ##. (ToFloat), ### (ToIntRound), ##! (ToIntTrunc)
├── collections/         # Array CRUD, slices, HOF ($>, $|, $<), index navigation
├── functions/           # Declarations, recursion, output params, scope
├── i18n/                # Unicode numerals, non-ASCII identifiers, i18n modes
├── index_nav/           # Deep nested access, ranges (..), fan-out extraction
├── lambdas/             # Closures, block lambdas, first-class usage
├── match/               # Pattern matching, guards, destructuring
├── modules_scope/       # Module import/export, scope isolation
├── named_tuples/        # Tuple creation, access, destructuring
├── output/              # >> output, newline ¶, formatting operators
├── safe_access/         # $? safe navigation, $! propagation
├── strings/             # String operators: $/, $++, #|x|, format, precision
├── analysis/            # Static analysis (unused vars, circular imports)
├── gaps/                # Language gap regression tests — one test per GAP documented in ZeethyCLI/GAPS.md
├── bugs/                # Bug regression tests — one test per BUG; must never regress
└── scripts/             # Test runner scripts and benchmarks
```

### `gaps/` — Language gap regression tests

Each file documents and exercises a specific language limitation discovered during
real-world usage (ZeethyCLI). Naming: `g<ID>_<short_description>.zy`.

The test proves the **intended behavior** — either that the workaround works correctly,
or that a formerly broken behavior is now fixed. Adding a test here is mandatory when
a GAP is resolved so the fix can never silently regress.

| File | GAP | What it tests |
|------|-----|---------------|
| `g07_bashexec_trailing_newline.zy` | G7 | BashExec auto-trims trailing `\n`; result usable in comparisons directly |
| `g09_module_const_access.zy` | G9 | `alias.CONST` dot access works; TypeChecker no longer blocks module aliases |
| `g11_module_void_call.zy` | G11 | `alias::fn()` valid as void statement; no `disc =` workaround needed |
| `g12_bashexec_expression.zy` | G12 | BashExec as first-class expression in match arms, conditionals, assignments |
| `g13_module_private_state.zy` | G13 | Module `=` variables persist across calls via write-back mechanism |
| `g14_export_block_position.zy` | G14 | `#>` valid after `<#` imports (not just immediately after `# name`) |
| `g17_script_toplevel_functions.zy` | G17 | Script-level functions inherit import aliases from caller |
| `g21_string_escape_braces.zy` | G21 | `\{` produces `{` literally, never interpolates; `{var}` still interpolates |

### `bugs/` — Bug regression tests

Each file is a regression guard for a specific runtime bug. Naming: `bug_<description>.zy`.
These tests are **high priority** — a failure here means a silent behavioral regression
in code that previously worked correctly.

| File | Bug | What it guards |
|------|-----|----------------|
| `bug01_module_intra_calls.zy` | BUG-01 | Module functions can call each other (exported and private) |
| `bug03_bashexec_void_statement.zy` | BUG-03 | `<\ cmd \>` valid as standalone void statement without assignment |
| `bug04_literal_brace_string.zy` | BUG-04 / BUG-07 | `\{` and `\}` produce literal braces; never trigger interpolation |
| `bug06_output_literal_callable.zy` | BUG-06 | `>> "literal" (expr)` outputs two items, not a function call crash |
| `bug_double_interpolation.zy` | Double-interpolation | `\{var\}` must never interpolate; only `{var}` does |

Top-level `.zy` + `.expected` pairs cover cross-cutting scenarios (scope, error handling, memory model).

## Testing Paradigms

### 1. Golden-file comparison — `expected_compare.sh`

Runs each `.zy` file and compares its stdout against the matching `.expected` file.

```bash
# Run all golden tests
bash tests/scripts/expected_compare.sh

# Scope to a single directory
bash tests/scripts/expected_compare.sh strings
bash tests/scripts/expected_compare.sh casts

# Regenerate .expected files from current interpreter output
bash tests/scripts/expected_compare.sh --regen
bash tests/scripts/expected_compare.sh strings --regen
```

Output:
```
  PASS  strings/14_split_operator.zy
  PASS  strings/15_concat_build.zy
  FAIL  strings/16_unicode_eval.zy
        --- expected
        +++ actual
        @@ -3 +3 @@
        -42
        +๔๒
```

### 2. Tree-walker vs VM parity — `vm_compare.sh`

Runs every `.zy` file under `tests/` twice (tree-walker and `--vm`) and reports divergences. Use this to track VM feature parity.

```bash
bash tests/scripts/vm_compare.sh
```

## `.zy` + `.expected` Convention

- The `.expected` file contains **exact stdout** the interpreter must produce, with no trailing newline.
- Interpreter warnings (lines starting with `[warn]`) are stripped before comparison.
- One `.expected` per `.zy`; both share the same base name and directory.
- Comments in `.zy` files document the expected value inline (`// 42`) as a cross-check for readers.

## Adding a New Test

1. Create `tests/<category>/NN_description.zy` with the scenario.
2. Run `bash tests/scripts/expected_compare.sh <category> --regen` to generate the golden file.
3. Inspect the generated `.expected` file to confirm the output is correct.
4. Commit both files together.

## Feature Coverage

| Category | Files | Operators / Features |
|----------|-------|----------------------|
| `arithmetic` | 8 | `+` `-` `*` `/` `%` `**`, precedence, int/float |
| `casts` | 4 | `##.` `###` `##!` — ToFloat, ToIntRound, ToIntTrunc |
| `collections` | 14 | Array ops, `$>` `$|` `$<` HOF, index nav `arr[i..j]` |
| `functions` | 8 | Named functions, recursion, `<~` output params |
| `i18n` | 6 | `#d0d9#` mode switch, Unicode booleans, digit scripts |
| `index_nav` | 13 | Deep access, ranges `..`, fan-out, `$?` safe access |
| `lambdas` | 10 | `->` closures, block lambdas, pipe `\|>` |
| `match` | 8 | `~` patterns, guards, array/tuple destructuring |
| `modules_scope` | 6 | `#>` export, `#<` import, aliases |
| `named_tuples` | 5 | `(:k v)` creation, access, destructuring |
| `output` | 6 | `>>` `¶` formatting `#,\|x\|` `#^\|x\|` precision `#.N\|x\|` |
| `safe_access` | 6 | `$?` `$!` `$!!` error propagation |
| `strings` | 16 | `$/` split, `$++` concat-build, `#\|x\|` Unicode eval, format |

Total: **317** golden-file test pairs.
