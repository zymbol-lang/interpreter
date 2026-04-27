# Zymbol Formatter Rules (`zymbol fmt`)

> **Design principle** — `zymbol fmt` is a *layout tool*, not a code transformer.
> It adjusts whitespace, indentation, and brace placement. It never alters the
> meaning of the program, never adds or removes tokens that change behavior, and
> never rewrites expressions.

---

## 1. What the formatter IS and IS NOT

| IS | IS NOT |
|----|--------|
| A whitespace normalizer | A linter or code analyzer |
| An indentation enforcer | An expression transformer |
| A brace/block layout tool | A parenthesis adder or remover |
| A comment and blank-line preserver | An optimizer or simplifier |

**Analogy with `rustfmt`:**
`rustfmt` enforces a single canonical style for Rust code — indentation, spacing,
brace placement — but it **never changes the semantics** of any expression.
`zymbol fmt` follows the same contract.

---

## 2. Fundamental constraints

### 2.1 Never change code
The formatter **must not** add, remove, or reorder any token that carries meaning:
- No parentheses added or removed
- No operator changed (e.g., `$^` stays `$^`, not `$^+`)
- No expression rewritten (e.g., `a, b -> x` stays as-is)
- No argument lists invented (e.g., `|> f` stays `|> f`, not `|> f(_)`)
- No trailing commas added to code that does not have them
- No array literals expanded or collapsed beyond what the user wrote

### 2.2 Never delete content
The formatter **must preserve**:
- Every `// line comment`, regardless of position (trailing or standalone)
- Every `/* block comment */`, whether single-line or multi-line
- Every blank line between statements (one blank line is preserved; multiple
  consecutive blank lines may be collapsed to one — see §5.3)

### 2.3 Idempotency
Running `zymbol fmt` twice on the same file must produce identical output:

```
zymbol fmt file.zy --write
zymbol fmt file.zy --write
# No changes on second run
```

---

## 3. Indentation

| Rule | Value |
|------|-------|
| Unit | 4 spaces (configurable via `--indent`) |
| Tabs vs spaces | Spaces by default; `--tabs` for tab mode |
| Level increase | Every block `{ }` opens a new level |
| Level decrease | Closing `}` returns to previous level |

The `}` that closes a block is always at the same indentation level as the
statement that opened the block:

```zy
// Input (any indentation)
? x > 0 {
>> "positive" ¶
}

// Output (normalized)
? x > 0 {
    >> "positive" ¶
}
```

---

## 4. Spacing rules

### 4.1 Around assignment operators
One space before and after `=`, `:=`, `+=`, `-=`, `*=`, `/=`, `%=`, `^=`:

```zy
x = 5           // ✓
PI := 3.14159   // ✓
count += 1      // ✓
```

### 4.2 Around arithmetic and comparison operators
One space before and after `+`, `-`, `*`, `/`, `%`, `==`, `!=`, `<`, `>`,
`<=`, `>=`, `&&`, `||`:

```zy
result = a + b * c      // ✓
? x > 0 && y < 10 {    // ✓
```

### 4.3 Around range operator
**No spaces** around `..`:

```zy
@ i:1..10 {     // ✓  (not  1 .. 10)
arr$[2..5]      // ✓
```

### 4.4 Symbol operators (no leading space)
Collection and string operators attach directly to their left operand:
`$#`, `$+`, `$-`, `$--`, `$?`, `$??`, `$>`, `$|`, `$<`, `$^`,
`$~~`, `$[`, `$++`, `$~`:

```zy
arr$#           // ✓  (not  arr $#)
arr$+ x         // ✓
str$~~["\n":""] // ✓
```

Exception: `$+` (append element) keeps a space after it:

```zy
result = result $+ element   // ✓
```

### 4.5 Namespace separator
**No spaces** around `::`:

```zy
module::function()   // ✓  (not  module :: function())
```

### 4.6 Tuple field access
**No space** around `.`:

```zy
point.x   // ✓  (not  point . x)
```

### 4.7 Lambda arrow
One space before and after `->`:

```zy
double = x -> x * 2          // ✓
add = (a, b) -> a + b        // ✓
```

### 4.8 Pipe operator
One space before and after `|>`:

```zy
result = 5 |> double   // ✓
```

### 4.9 Concatenation operator
One space before `$++`:

```zy
greeting = "Hello" $++ name   // ✓
```

### 4.10 Output statement (`>>`)
One space after `>>`, one space between items:

```zy
>> "value: " x ¶   // ✓
```

The newline token `¶` is joined to the preceding token on the same line
with one space:

```zy
>> x ¶     // ✓  (not  >> x\n¶  on separate lines)
```

---

## 5. Block and brace layout

### 5.1 Opening brace — always same line
The `{` always appears on the same line as the control structure:

```zy
? condition {        // ✓
    ...
}

@ i:1..10 {         // ✓
    ...
}
```

### 5.2 Else / else-if — same line as closing brace
`_` (else) and `_?` (else-if) appear on the same line as the preceding `}`:

```zy
? x > 0 {
    >> "positive" ¶
} _ {
    >> "non-positive" ¶
}
```

### 5.3 Blank lines between top-level declarations
One blank line is inserted before and after every function declaration at
the top level. All other blank lines in the source are preserved as-is.

```zy
x = 5

add(a, b) {
    <~ a + b
}

y = add(2, 3)
```

### 5.4 Single-statement blocks (inline option)
When `inline_single_statement = true` (default) and a block contains exactly
one *simple* statement (assignment, output, break, continue, return), the
formatter may place it on one line:

```zy
? found { @! }            // inline single break
? x > 0 { >> "yes" ¶ }   // inline single output
```

Multi-statement blocks are always expanded to multiple lines.

> **Note:** this is a *layout* change, not a code change. The semantics
> are identical. Set `inline_single_statement = false` to disable.

---

## 6. Match expressions (`??`)

Each arm is one line: `pattern : value` or `pattern : { block }`.
Arms are **not** aligned — no padding added to align the `:`.

```zy
?? cmd {
    "start" : start_fn()
    "stop" : stop_fn()
    _ : show_error("unknown")
}
```

---

## 7. Module files (`# name { }`)

The module declaration header, imports, and export block follow the same
indentation and brace rules as any other block:

```zy
# math {
    <# ./util <= u

    #> {
        add
        mul
    }

    add(a, b) { <~ a + b }
    mul(a, b) { <~ a * b }
}
```

---

## 8. Labeled loops

Canonical form: `@:label`, `@:label!`, `@:label>` (colon between `@` and label):

```zy
@:outer {
    count = count + 1
    ? count >= 3 { @:outer! }
}
```

---

## 9. Comments

### 9.1 Trailing line comments
A trailing `// comment` is kept on the same line as the code, separated by
one space. Alignment padding (multiple spaces) in the original is collapsed
to one space:

```zy
// Original:
x = 5    // this is five

// Formatted (same):
x = 5 // this is five
```

### 9.2 Standalone line comments
A `//` comment on its own line is preserved at its current indentation
(re-indented to match the surrounding block level):

```zy
? flag {
    // this comment stays here
    >> "yes" ¶
}
```

### 9.3 Block comments (`/* */`)
Block comments — whether single-line or spanning multiple lines — are
preserved in their entirety. The formatter does **not** inspect, reformat,
or remove any content inside a block comment.

```zy
/*
This whole block is untouched.
x = old_code()   ← not executed, not reformatted
*/
```

---

## 10. What the formatter does NOT change

The following are explicitly out of scope. If the formatter currently
changes any of these, it is a **bug**:

| Category | Examples |
|----------|---------|
| Parentheses | `(a + b)` must stay `(a + b)` |
| Operator symbols | `$^` must not become `$^+` |
| Lambda syntax | `a, b -> x` must not become `(a, b) -> x` |
| Explicit placeholders | `f(_)` must not become `f` or vice-versa |
| Array literals | `[1, 2, 3]` must not expand to multi-line if it fits |
| Named tuple spacing | `(x: 1, y: 2)` — internal spacing not changed |
| Trailing commas | No commas added to arrays/tuples that lack them |
| Increment/decrement | `count++` must stay `count++`, not `count = count + 1` |
| Compound assignment | `x += 1` must stay `x += 1` |

---

## 11. Architectural limitation: parentheses and the AST

`zymbol fmt` reconstructs source code from the AST. The Zymbol parser discards
grouping parentheses when building the AST, so the formatter cannot always
distinguish:

```zy
a + (b * c)   ← parens in source, same AST as below
a + b * c     ← no parens in source
```

### What the formatter MAY add back

In a small set of contexts, the Zymbol parser **requires** parentheses and
without them the output would either fail to parse or change semantics.
The formatter adds parens back in exactly these cases:

| Context | Problem without parens | Formatter action |
|---------|----------------------|-----------------|
| `>> (x && y)` | `>> x && y` is a **parse error** (parser sees two items) | Add `(` `)` around `&&`/`\|\|` binary in `>>` |
| `###(a / b)` | `###a / b` divides after cast, not before — **different result** | Add `(` `)` around binary operand of cast `###`/`##!`/`##.` |
| `arr $+ (a * b)` | `arr $+ a * b` may bind `*` to `arr $+ a` — **different result** | Add `(` `)` around binary operand of `$+` |
| `(expr)[i]` deep nav | `sort_expr[i]` attaches `[i]` to the lambda body — **different result** | Add `(` `)` around collection-operation base |
| `arr[i>j]` multi-step nav | `arr[a+1>b]` parses `a + (1>b)` (comparison) — **wrong navigation** | Add `(` `)` around binary step index in multi-step path |

All other paren changes are bugs. Specifically, the formatter must **never**:

- Add parens around a simple identifier: `$> double` must stay `$> double`
- Add parens around a lambda that already has its own parens
- Remove parens from any expression the user wrote

### What cannot be done without AST changes

If the user writes `(a + b)` as a standalone group (not in any of the forced
contexts above), the formatter cannot know the parens existed and will output
`a + b`. This is a known limitation. The fix would require the parser to
preserve a `GroupExpr` node.

---

## 12. Configuration reference

| Option | Default | CLI flag | Description |
|--------|---------|----------|-------------|
| `indent_size` | 4 | `--indent N` | Spaces per indent level |
| `use_spaces` | true | `--tabs` | Use tabs instead of spaces |
| `max_line_length` | 100 | `--line-length N` | Target line length |
| `inline_single_statement` | true | `--no-inline` | Collapse single-stmt blocks |
| `brace_same_line` | true | — | Opening brace placement |

---

## 13. Non-goals (explicit)

- **Linting** — detecting unused variables, type errors, etc. → use `zymbol check`
- **Code style enforcement beyond layout** — naming conventions, idioms
- **Auto-import or auto-fix** — the formatter never adds new imports
- **Semantic analysis** — the formatter does not evaluate expressions
