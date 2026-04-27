# Zymbol — ZeethyCLI Bug & Gap Fix Plan

> Implementation plan derived from real-world stress testing with ZeethyCLI (`Zeethy/`).
> Each entry maps directly to a bug or gap in `Zeethy/BUGS.md` / `Zeethy/GAPS.md`.
> Ordered by complexity: simple → medium → new operators.

---

## Sprint 1 — Simple Parser/Lexer Fixes (Low Risk)

### FIX-01 · BUG-03 — BashExec as void statement

**File**: `crates/zymbol-parser/src/lib.rs`
**Function**: `parse_statement`
**Lines**: ~200 (the final `_ =>` catch-all arm)

**Problem**: `<\ cmd \>` used without assignment produces "unexpected token: BashCommand".
The parser's `parse_statement` has no arm for `TokenKind::BashCommand(_)`.

**Fix**: Add a new match arm before the catch-all `_` arm:

```rust
// In parse_statement, before the final `_ =>` catch-all:
TokenKind::BashCommand(_) => {
    // BashExec as statement (side-effect only, discard return value)
    let expr = self.parse_expr()?;
    let span = expr.span();
    Ok(Statement::Expr(ExprStatement::new(expr, span)))
}
```

**Verification**:
```zymbol
<\ mkdir "-p" /tmp/test \>    // must parse without error
<\ rm "-f" /tmp/file.txt \>   // must parse without error
```

---

### FIX-02 · BUG-04 — Literal `{` in strings via `{{` escape

**File**: `crates/zymbol-lexer/src/literals.rs`
**Function**: `lex_string`
**Lines**: ~56 (the `} else if ch == '{'` branch)

**Problem**: Any `{` in a string is treated as interpolation start.
`"{"` → "unterminated interpolation". Impossible to write JSON or shell code inline.

**Note**: `\{` already works (line ~47 in `lex_string`):
```rust
'{' => '{',  // \{ → literal {  ← already implemented
'}' => '}',  // \} → literal }  ← already implemented
```

The issue is only that `\{` is undocumented and the `{{` double-brace convention is missing.

**Fix**: At the point where `ch == '{'` is detected, peek ahead to check for `{{`:

```rust
} else if ch == '{' {
    // NEW: {{ → literal single {
    if self.peek_char() == Some('{') {
        self.advance(); // consume first {
        self.advance(); // consume second {
        current_text.push('{');
        continue; // skip interpolation logic
    }

    // Existing interpolation logic follows ...
    has_interpolation = true;
    // ...
}
```

Add corresponding `}}` → `}` for symmetry:

```rust
} else if ch == '}' {
    // NEW: }} → literal single }
    if self.peek_char() == Some('}') {
        self.advance(); // consume first }
        self.advance(); // consume second }
        current_text.push('}');
        continue;
    }
    // otherwise just a literal } (closing braces outside interpolation are normal)
    current_text.push(ch);
    self.advance();
} else {
```

**Check `peek_char` method**: Confirm the lexer has a `peek_char()` method or equivalent.
If not, use `self.chars.get(self.pos)` or the existing pattern used elsewhere in the file.

**Verification**:
```zymbol
a = "{{"           // → "{"
b = "}}"           // → "}"
c = "{{key}}"      // → "{key}" (literal, not interpolated)
name = "world"
d = "Hello {{name}}"  // → "Hello {name}" (literal braces, no interpolation)
e = "Hello {name}"    // → "Hello world" (interpolation, unchanged)
```

---

### FIX-03 · GAP G7 — BashExec strips trailing `\n` by default

**File**: `crates/zymbol-interpreter/src/script_exec.rs`
**Function**: `eval_bash_exec`
**Lines**: ~115 (after `result` is built from stdout)

**Problem**: Every BashExec result includes a trailing `\n` that callers must strip manually
via `result$~~["\n":""]`. This is the most repeated pattern in ZeethyCLI.

**Fix**: Trim the trailing newline before returning:

```rust
// After building `result` from stdout (and optionally stderr):
// Trim trailing newline (consistent with shell command substitution behavior)
let result = result.trim_end_matches('\n').to_string();

Ok(Value::String(result))
```

**Note**: This is a **breaking change** for code that relies on the trailing `\n`.
All existing Zeethy code currently strips it manually, so this will allow removing those
`$~~["\n":""]` calls. After implementing, update `Zeethy/lib/*.zy` accordingly.

**Verification**:
```zymbol
r = <\ echo "hello" \>
>> r ¶               // must print "hello" (not "hello\n")
r2 = <\ printf "no newline" \>
>> r2 ¶              // must print "no newline"
```

---

## Sprint 2 — Interpreter: Module Function Visibility (BUG-01)

### FIX-04 · BUG-01 — Intra-module function calls

**Root cause**: When a module function is called, the interpreter restores the module's
`all_variables` and `import_aliases` into the execution context, but does NOT load the
module's function definitions into `self.functions`. So a module function calling another
function in the same module fails with "undefined function: 'bar'".

**Files modified**:
1. `crates/zymbol-interpreter/src/modules.rs` — Add `all_functions` field to `LoadedModule`
2. `crates/zymbol-interpreter/src/functions_lambda.rs` — Restore module functions on module call

---

#### Step A — Add `all_functions` to `LoadedModule`

**File**: `crates/zymbol-interpreter/src/modules.rs`
**Struct**: `LoadedModule` (~line 21)

```rust
pub(crate) struct LoadedModule {
    pub(crate) name: String,
    /// Exported functions only (for external callers via alias::fn)
    pub(crate) functions: HashMap<String, Rc<FunctionDef>>,
    /// ALL module functions: exported + private (for intra-module calls)
    pub(crate) all_functions: HashMap<String, Rc<FunctionDef>>,  // NEW
    pub(crate) constants: HashMap<String, Value>,
    pub(crate) all_variables: HashMap<String, Value>,
    pub(crate) import_aliases: HashMap<String, PathBuf>,
    pub(crate) loaded_modules_refs: HashMap<PathBuf, ()>,
}
```

**Populate `all_functions`** in `load_module` (~line 140):
After building `loaded_module`, before returning:

```rust
// Always store ALL functions (for intra-module call resolution)
loaded_module.all_functions = module_interp.functions.clone();
```

This line must be added regardless of whether the module has an export block or not.
Currently `module_interp.functions` contains every function defined in the module file.

---

#### Step B — Restore module functions when entering a module call

**File**: `crates/zymbol-interpreter/src/functions_lambda.rs`
**Function**: `eval_traditional_function_call` (~line 219)
**Location**: The block starting at ~line 258: `if let Some((_, module_path)) = &module_info`

**Current code** (~lines 258-265):
```rust
if let Some((_, module_path)) = &module_info {
    if let Some(module) = self.loaded_modules.get(module_path).cloned() {
        for (name, value) in &module.all_variables {
            self.set_variable(name, value.clone());
        }
        self.import_aliases = module.import_aliases.clone();
    }
}
```

**Replace with**:
```rust
let saved_functions = if let Some((_, module_path)) = &module_info {
    if let Some(module) = self.loaded_modules.get(module_path).cloned() {
        for (name, value) in &module.all_variables {
            self.set_variable(name, value.clone());
        }
        self.import_aliases = module.import_aliases.clone();
        // NEW: expose all module functions so intra-module calls resolve correctly
        let saved = std::mem::replace(&mut self.functions, module.all_functions.clone());
        Some(saved)
    } else {
        None
    }
} else {
    None
};
```

**After `restore_call_state(saved)`** (at the end of the function, ~line 346):
```rust
// Restore outer function table (undoes the module function swap above)
if let Some(saved_fns) = saved_functions {
    self.functions = saved_fns;
}
```

**This must happen AFTER `restore_call_state`** because `restore_call_state` doesn't
touch `self.functions`.

---

#### Verification for BUG-01

```zymbol
# mymod
#> { foo, bar }

foo() { <~ bar() }
bar() { <~ 42 }
```

```zymbol
<# ./mymod <= m
result = m::foo()
>> result ¶    // must print: 42
```

```zymbol
# counter
#> { increment }

get_base() { <~ 10 }      // private function
increment(n) { <~ get_base() + n }
```

```zymbol
<# ./counter <= c
>> c::increment(5) ¶   // must print: 15
```

---

## Sprint 3 — Module Function Call as Statement (GAP G11)

### FIX-05 · GAP G11 — `alias::fn()` as void statement

**Problem**: `disc = ui::show_info("hello")` works, but `ui::show_info("hello")` alone
(without assignment) fails with parser error.

**Files modified**:
1. `crates/zymbol-parser/src/lib.rs` — Dispatch `Ident` → `::` → `(` to void statement handler
2. `crates/zymbol-parser/src/functions.rs` — Accept `FunctionCall` with `MemberAccess` callable

---

#### Step A — Recognize `alias::fn()` in `parse_statement`

**File**: `crates/zymbol-parser/src/lib.rs`
**Function**: `parse_statement`
**Location**: `TokenKind::Ident(_)` arm (~line 125)

The `Ident` arm currently checks `Ident → LParen` for function calls. It needs to also
handle `Ident → DoubleColon → Ident → LParen`.

Add a look-ahead check for `::` at position +1:

```rust
TokenKind::Ident(_) => {
    // ... existing const decl check ...

    // NEW: check for module call: alias::fn(...)
    if self.peek_ahead(1).map(|t| matches!(t.kind, TokenKind::DoubleColon)).unwrap_or(false) {
        return self.parse_module_call_statement();
    }

    // existing Ident → LParen check follows ...
}
```

**Check `TokenKind::DoubleColon`**: verify the exact token name in `zymbol-lexer`.
It may be `TokenKind::ColonColon` or `TokenKind::Scope`. Grep with:
```bash
grep -rn "ColonColon\|DoubleColon\|Scope" crates/zymbol-lexer/src/
```

---

#### Step B — Parse and discard module function call

Add `parse_module_call_statement` to the parser (in `functions.rs` or `lib.rs`):

```rust
pub(crate) fn parse_module_call_statement(&mut self) -> Result<Statement, Diagnostic> {
    // Parse the full expression (will produce FunctionCall with MemberAccess callable)
    let expr = self.parse_expr()?;
    let span = expr.span();

    match &expr {
        Expr::FunctionCall(call) => {
            match call.callable.as_ref() {
                Expr::MemberAccess(_) => {
                    // module::fn() — valid void statement
                    Ok(Statement::Expr(ExprStatement::new(expr, span)))
                }
                _ => Err(Diagnostic::error("expected module function call")
                    .with_span(span)
                    .with_help("syntax: alias::function(args)")),
            }
        }
        _ => Err(Diagnostic::error("expected module function call")
            .with_span(span)
            .with_help("syntax: alias::function(args)")),
    }
}
```

**Alternatively** (simpler): extend `parse_function_call_statement` to also accept
`FunctionCall` whose callable is `MemberAccess`:

```rust
pub(crate) fn parse_function_call_statement(&mut self) -> Result<Statement, Diagnostic> {
    let expr = self.parse_expr()?;
    let span = expr.span();

    match &expr {
        Expr::FunctionCall(call) => {
            match call.callable.as_ref() {
                Expr::Identifier(_) | Expr::MemberAccess(_) => {  // MemberAccess added
                    Ok(Statement::Expr(ExprStatement::new(expr, span)))
                }
                _ => Err(Diagnostic::error("expected function call")
                    .with_span(span)
                    .with_help("only function calls can be used as statements")),
            }
        }
        _ => Err(Diagnostic::error("expected function call")
            .with_span(span)
            .with_help("only function calls can be used as statements")),
    }
}
```

**Verification**:
```zymbol
<# ./lib/ui <= ui
ui::show_info("hello")      // must not require: disc = ui::show_info("hello")
ui::banner()                 // void call as statement
```

---

## Sprint 4 — New String/Array Operators (GAP G5, G6)

### FIX-06 · GAP G5 — `arr$join(sep)` operator

**Description**: Join array elements into a string with a separator.
```zymbol
parts = ["a", "b", "c"]
result = parts$join(", ")   // → "a, b, c"
```

**Files to modify** (6 files, follow the `$#` / `$??` pattern):

| File | Change |
|------|--------|
| `crates/zymbol-lexer/src/lib.rs` | Add `DollarJoin` token, lex `$join` |
| `crates/zymbol-ast/src/collection_ops.rs` | Add `CollectionJoinExpr { array, separator, span }` |
| `crates/zymbol-ast/src/lib.rs` | Add `Expr::CollectionJoin(CollectionJoinExpr)` variant |
| `crates/zymbol-parser/src/collection_ops.rs` | Parse `expr $join (sep_expr)` |
| `crates/zymbol-interpreter/src/collection_ops.rs` | Evaluate: join Vec<Value> with sep string |
| `crates/zymbol-interpreter/src/expr_eval.rs` | Dispatch `Expr::CollectionJoin` |

**Implementation notes**:
- `$join` takes one argument (the separator string)
- Elements are converted to string with `Value::to_display_string()` or similar
- Empty array → empty string

---

### FIX-07 · GAP G6 — `str$split(sep)` and `str$split(sep, limit)` operator

**Description**: Split a string by separator, returning an array.
```zymbol
parts = "hello world"$split(" ")       // → ["hello", "world"]
parts = "/mode fast"$split(" ", 2)     // → ["/mode", "fast"]
```

**Files to modify** (same 6-file pattern as FIX-06):

| File | Change |
|------|--------|
| `crates/zymbol-lexer/src/lib.rs` | Add `DollarSplit` token, lex `$split` |
| `crates/zymbol-ast/src/string_ops.rs` | Add `StringSplitExpr { string, separator, limit, span }` |
| `crates/zymbol-ast/src/lib.rs` | Add `Expr::StringSplit(StringSplitExpr)` variant |
| `crates/zymbol-parser/src/string_ops.rs` | Parse `expr $split (sep)` and `expr $split (sep, limit)` |
| `crates/zymbol-interpreter/src/string_ops.rs` | Evaluate: `str.splitn(n, sep)` → `Value::Array` |
| `crates/zymbol-interpreter/src/expr_eval.rs` | Dispatch `Expr::StringSplit` |

**Implementation notes**:
- Without limit: `str.split(sep).collect()`
- With limit N: `str.splitn(N, sep).collect()`
- Returns `Value::Array(Vec<Value::String>)`
- Separator is a string (not regex)

---

## Summary Table

| Fix | Bug/Gap | Priority | Complexity | Sprint |
|-----|---------|----------|------------|--------|
| FIX-01 | BUG-03 BashExec void statement | HIGH | Low — 4 lines | 1 |
| FIX-02 | BUG-04 `{{` literal brace escape | HIGH | Low — 10 lines | 1 |
| FIX-03 | GAP G7 BashExec trims `\n` | MEDIUM | Low — 1 line | 1 |
| FIX-04 | BUG-01 Intra-module function calls | CRITICAL | Medium — 3 files | 2 |
| FIX-05 | GAP G11 `alias::fn()` as statement | HIGH | Medium — 2 files | 3 |
| FIX-06 | GAP G5 `arr$join(sep)` | MEDIUM | High — 6 files | 4 |
| FIX-07 | GAP G6 `str$split(sep)` | MEDIUM | High — 6 files | 4 |

---

## Post-Fix: ZeethyCLI Cleanup

After implementing the fixes above, the following ZeethyCLI files can be simplified:

- `lib/history.zy` — Remove all `disc =` assignments for void calls; use intra-module helpers for count logic; remove `$~~["\n":""]` strips (FIX-03)
- `lib/ollama.zy` — Remove `$~~["\n":""]` strips
- `main.zy` — Remove `disc = ""` pre-declaration; replace `disc = ui::fn()` calls with `ui::fn()` (FIX-05)
- All modules — Use `{{` and `}}` for JSON construction instead of Python `dict()` (FIX-02)

---

## Notes

- BUG-02 (`_` variable scoping) is intentional behavior — `_` prefix means "block-local".
  Documented as expected behavior; workaround is to use regular variable names.
- BUG-05 (`@>` in infinite loops) is unconfirmed; skip until reproduced with a test.
- GAP G8 (BashExec safe interpolation via env vars) is a security improvement — defer to a later release.
- GAP G1 (std/net), GAP G2 (std/json), GAP G3 (std/io) are full stdlib modules — track in ROADMAP.md, not here.
