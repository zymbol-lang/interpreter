# Formatter Bug Report — `zymbol fmt`

Audit of `crates/zymbol-formatter/` against `FORMATTER_RULES.md`.
Each finding includes file + line, the violated rule, reproduction, and the concrete fix.

---

## BUG-1 — `format_output` adds parens around ALL binary exprs (§11 / §2.1)

**File:** `crates/zymbol-formatter/src/visitor.rs:213-218`  
**Rule violated:** §11 — formatter MAY add `( )` in `>>` only for `&&`/`||` binaries.  
**Severity:** High — adds semantically redundant tokens, breaks idempotency for arithmetic output.

### What the code does

```rust
// visitor.rs:213-217
let needs_parens = matches!(expr, Expr::Binary(_));   // ← too broad
if needs_parens { self.output.write("("); }
self.format_expr(expr);
if needs_parens { self.output.write(")"); }
```

### Symptom

```zy
// Input
>> a + b

// Output (wrong)
>> (a + b)

// Expected (correct)
>> a + b
```

`>> a + b` parses without ambiguity. Only `&&` / `||` cause a parse error in `>>` because the
parser reads `>>` arguments as space-separated items and `&&`/`||` token-break the item boundary.
Arithmetic operators do not have that problem.

### Fix

```rust
// Check only the operators that actually cause parse ambiguity in >>
let needs_parens = matches!(
    expr,
    Expr::Binary(b) if matches!(b.op, BinaryOp::And | BinaryOp::Or)
);
```

---

## BUG-2 — Implicit `|> f` always emits `|> f(_)` (§2.1)

**Files:**
- `crates/zymbol-formatter/src/visitor.rs:1387-1404` — formatter always writes `(args)`
- `crates/zymbol-parser/src/expressions.rs:40-48` — root cause

**Rule violated:** §2.1 — "`|> f` stays `|> f`, not `|> f(_)`".  
**Severity:** High — adds a token the user never wrote; breaks the no-mutation contract.

### Root cause

The parser desugars `5 |> double` (no explicit args) into the same AST node as `5 |> double(_)`:

```rust
// parser/src/expressions.rs:40-48
if !matches!(self.peek().kind, TokenKind::LParen) {
    // "Implicit first-position pipe"
    left = Expr::Pipe(zymbol_ast::PipeExpr {
        arguments: vec![zymbol_ast::PipeArg::Placeholder],  // ← added by parser
        ...
    });
}
```

The formatter then always writes `(args)`, producing `|> double(_)` for both forms.

### Symptom

```zy
// Input
result = 5 |> double

// Output (wrong)
result = 5 |> double(_)

// Expected
result = 5 |> double
```

### Fix (two-part)

**Part A — AST/parser (required for a clean fix):**  
Add `implicit: bool` to `PipeExpr` so the formatter can distinguish the two forms.

```rust
// zymbol-ast/src/expressions.rs
pub struct PipeExpr {
    pub left: Box<Expr>,
    pub callable: Box<Expr>,
    pub arguments: Vec<PipeArg>,
    pub implicit: bool,   // true when user wrote |> f with no ()
    pub span: Span,
}
```

Set `implicit: true` in the parser's no-`LParen` branch (expressions.rs:42).

**Part B — formatter (visitor.rs:1387-1404):**  
Skip the `(args)` block when `pipe.implicit` is true.

```rust
fn format_pipe(&mut self, pipe: &PipeExpr) {
    self.format_expr(&pipe.left);
    self.output.write(" |> ");
    let needs_parens = matches!(pipe.callable.as_ref(), Expr::Lambda(_));
    if needs_parens { self.output.write("("); }
    self.format_expr(&pipe.callable);
    if needs_parens { self.output.write(")"); }

    if !pipe.implicit {                          // ← guard
        self.output.write("(");
        for (i, arg) in pipe.arguments.iter().enumerate() {
            match arg {
                PipeArg::Placeholder => self.output.write("_"),
                PipeArg::Expr(expr) => self.format_expr(expr),
            }
            if i < pipe.arguments.len() - 1 { self.output.write(", "); }
        }
        self.output.write(")");
    }
}
```

---

## BUG-3 — Multi-line block comment re-indentation is inconsistent (§9.3 / §2.2)

**File:** `crates/zymbol-formatter/src/lib.rs:230-253`  
**Rule violated:** §9.3 — block comment content must be preserved in its entirety.  
**Severity:** Medium — opening `/*` line gets re-indented, continuation lines keep original whitespace.

### What the code does

```rust
// lib.rs:230-239 — opening line
if !in_block_comment && trimmed.contains("/*") {
    in_block_comment = true;
    result.push_str(&current_indent);   // ← re-indent with formatter level
    result.push_str(trimmed);           // ← trimmed removes original indentation
    result.push('\n');
    ...
}

// lib.rs:244-252 — continuation lines
} else if in_block_comment {
    result.push_str(orig_line);         // ← original whitespace kept as-is
    result.push('\n');
    ...
}
```

### Symptom

```zy
// Original (at top level, column 0)
/*
 * first line
 * second line
 */

// After formatting (inside a block, current_indent = "    ")
    /*           ← re-indented ✓
 * first line   ← original indent kept ✗
 * second line  ← original indent kept ✗
 */             ← original indent kept ✗
```

### Fix

Compute the base indentation of the original opening line; strip that prefix from every continuation
line and prepend `current_indent` instead. This makes all lines of the comment move together.

```rust
// Capture how much the original opening was indented
let orig_opening_indent = orig_line.len() - orig_line.trim_start().len();
let indent_prefix = &orig_line[..orig_opening_indent];

// Continuation lines
result.push_str(&current_indent);
if orig_line.starts_with(indent_prefix) {
    result.push_str(&orig_line[orig_opening_indent..]);
} else {
    result.push_str(orig_line.trim_start());
}
result.push('\n');
```

---

## DEAD-1 — `continuation_indent` field declared but never used

**File:** `crates/zymbol-formatter/src/config.rs:15, 41, 71-74`  
**Rule violated:** §12 — not listed in the config reference; dead code pollutes the API.  
**Severity:** Medium — silently does nothing; misleads callers.

```rust
// config.rs — unused field + builder
pub continuation_indent: usize,       // line 15
continuation_indent: 8,               // line 41 (default)
pub fn with_continuation_indent(...)  // line 71-74
```

The formatter's line-break logic in `format_binary` calls `indent()` / `dedent()` (which uses
`indent_size`). `continuation_indent` is never read anywhere in `visitor.rs` or `output.rs`.

**Fix:** Delete the field, the default, and the builder method. No behaviour change.

---

## DEAD-2 — `max_inline_array_elements` field declared but never used

**File:** `crates/zymbol-formatter/src/config.rs:21, 43`  
**Rule violated:** §12 — not listed in the config reference.  
**Severity:** Medium — same as DEAD-1.

```rust
pub max_inline_array_elements: usize,   // line 21
max_inline_array_elements: 5,           // line 43 (default)
```

`format_array_literal` only checks `max_inline_array_length` (character budget). Element count is
never consulted.

**Fix:** Delete the field and its default. No behaviour change.

---

## LATENT-1 — `trailing_commas: true` would violate §2.1

**Files:** `config.rs:26-27, 83-85` / `visitor.rs:802`  
**Rule violated:** §2.1 — "No trailing commas added to code that does not have them."  
**Severity:** Medium — latent; defaults to `false`, but the field is `pub` so external code can enable it.

```rust
// visitor.rs:802 — activates trailing comma in multi-line arrays
if i < arr.elements.len() - 1 || config.trailing_commas {
    self.output.write(",");
}
```

§12's config reference table does not mention `trailing_commas`. The spec forbids it unconditionally.

**Fix:**
1. Remove `pub trailing_commas: bool` from `FormatterConfig`.
2. Remove `trailing_commas: false` from `Default`.
3. Remove `without_trailing_commas()` builder (no-op anyway).
4. Change `visitor.rs:802` to `if i < arr.elements.len() - 1`.

---

## MINOR-1 — `is_simple_statement` includes `Expr` and `Newline` beyond spec (§5.4)

**File:** `crates/zymbol-formatter/src/visitor.rs:514-526`  
**Rule violated:** §5.4 — "simple statement" = assignment, output, break, continue, return.  
**Severity:** Low — `Expr` (standalone function calls) is a reasonable extension; `Newline` (¶) is not.

```rust
fn is_simple_statement(&self, stmt: &Statement) -> bool {
    matches!(stmt,
        Statement::Output(_)
        | Statement::Assignment(_)
        | Statement::ConstDecl(_)
        | Statement::DestructureAssign(_)
        | Statement::Break(_)
        | Statement::Continue(_)
        | Statement::Return(_)
        | Statement::Newline(_)    // ← not in spec; { ¶ } inlines to { ¶ }
        | Statement::Expr(_)       // ← reasonable extension, not in spec
    )
}
```

A block containing only `¶` (`Newline`) being inlined as `? x { ¶ }` is a degenerate case.

**Fix:** Remove `Statement::Newline(_)` from the list. Keep `Statement::Expr(_)`.

---

## MINOR-2 — Multiple consecutive blank lines not collapsed (§2.2)

**File:** `crates/zymbol-formatter/src/lib.rs:256-260`  
**Rule violated:** §2.2 — "multiple consecutive blank lines may be collapsed to one."  
**Severity:** Low — spec uses "may", so current pass-through is not strictly wrong but it is inconsistent with the spirit of normalization.

```rust
// lib.rs:256-260 — blank line case
if trimmed.is_empty() {
    result.push('\n');   // ← no limit; three blank lines → three blank lines
    orig_idx += 1;
    continue;
}
```

**Fix:** Track whether the previous line was already blank and skip emitting a second consecutive blank.

```rust
let last_was_blank = result.ends_with("\n\n");
if trimmed.is_empty() {
    if !last_was_blank {
        result.push('\n');
    }
    orig_idx += 1;
    continue;
}
```

---

## Work order

| Order | ID | Effort | Status | Blocks |
|-------|----|--------|--------|--------|
| 1 | DEAD-1 | trivial | ✓ fixed | — |
| 2 | DEAD-2 | trivial | ✓ fixed | — |
| 3 | LATENT-1 | trivial | ✓ fixed | — |
| 4 | MINOR-1 | trivial | ✓ fixed | — |
| 5 | MINOR-2 | small | ✓ fixed | — |
| 6 | BUG-1 | small | ✓ fixed | — |
| 7 | BUG-3 | medium | ✓ fixed | — |
| 8 | BUG-2 | large | ✓ fixed | zymbol-ast + zymbol-parser + zymbol-formatter |
| 9 | BUG-4 | medium | ✓ fixed | zymbol-lexer + zymbol-interpreter + zymbol-compiler + zymbol-formatter |
| 10 | BUG-5 | small | ✓ fixed | zymbol-formatter (visitor.rs) |
| 11 | BUG-6 | medium | ✓ fixed | zymbol-formatter (lib.rs — merge_comments) |
| 12 | BUG-7 | medium | ✓ fixed | zymbol-formatter (lib.rs — merge_comments) |

---

## BUG-4 — `\}` sentinel missing in lexer (§2.1 / §2.3)

**Files:** `zymbol-lexer/src/literals.rs:52`, `zymbol-interpreter/src/literals.rs`, `zymbol-compiler/src/lib.rs`, `zymbol-formatter/src/visitor.rs`
**Severity:** High — `"\{name\}"` formatted as `"\{name}"` (loses the `\}` backslash), causing `merge_comments` to emit BOTH the original and formatted versions.

The lexer handled `\{` → `\x01` (sentinel, preserved through AST), but `\}` → `}` (sentinel discarded). The formatter's `escape_string` only restored `\x01` → `\{`, so `"\{name\}"` emitted as `"\{name}"`. This caused `merge_comments` to fail matching the original line, and the "remaining formatted lines" loop emitted a duplicate.

**Fix:** Add `\x02` sentinel symmetrically:
- Lexer: `'}' => '\x02'` alongside `'{' => '\x01'`
- Interpreter/compiler: `.replace('\x01', "{").replace('\x02', "}")`
- Formatter escape_string: `'\x02' => result.push_str("\\}")`

---

## BUG-5 — `$++` items with binary expressions not parenthesized (§2.3)

**File:** `zymbol-formatter/src/visitor.rs` — `ConcatBuild` arm
**Severity:** Medium — `"str" $++ (n + 1)` formatted as `"str" $++ n + 1`; second pass parses differently (stops at `n`, leaves `+ 1` for arithmetic), breaking idempotency.

**Fix:** Wrap `Binary` items in `$++` with parens:
```rust
let needs_parens = matches!(item, Expr::Binary(_));
if needs_parens { self.output.write("("); }
self.format_expr(item);
if needs_parens { self.output.write(")"); }
```

---

## BUG-6 — Standalone comment lines inherit wrong indentation (§2.3)

**File:** `zymbol-formatter/src/lib.rs` — Case 2 in `merge_comments`
**Severity:** Medium — After a closing `}`, `current_indent` is still set to the block's inner level; subsequent standalone comments get wrong indentation.

**Fix:** Use the NEXT formatted line's indentation for comment-only lines:
```rust
let upcoming_indent = formatted_lines[fmt_idx..]
    .iter()
    .find(|l| !l.trim().is_empty())
    .map(|l| &l[..l.len() - l.trim_start().len()])
    .unwrap_or("");
result.push_str(upcoming_indent);
```

---

## BUG-7 — Closing `}` fails to match merged `} _ {` / `} :! {` lines (§2.3)

**File:** `zymbol-formatter/src/lib.rs` — `code_contains`
**Severity:** Medium — When a lone `}` in the original is merged with the else/catch keyword into `} _ {` by the formatter, `merge_comments` couldn't match `}` to `} _ {`, causing re-sync to dump both the original code and the formatted code.

**Fix:** Add a starts-with check specifically for `}`:
```rust
if orig == "}" && fmt.starts_with('}') {
    return (true, true, false);
}
```
