# Zymbol Formatter Rules (`zymbol fmt`)

> **Design principle** — `zymbol fmt` is a *layout tool*, not a code
> transformer. It adjusts whitespace, indentation, and brace placement. It
> never alters the meaning of a program — and since the safety-gate rework
> this is **enforced mechanically**, not just promised.

---

## 1. What the formatter IS and IS NOT

| IS | IS NOT |
|----|--------|
| A whitespace normalizer | A linter or code analyzer |
| An indentation enforcer | An expression transformer |
| A brace/block layout tool | A parenthesis adder or remover |
| A comment and blank-line preserver | An optimizer or simplifier |

**Analogy with `rustfmt`:** `rustfmt` enforces a single canonical style —
indentation, spacing, brace placement — but never changes the semantics of any
expression. `zymbol fmt` follows the same contract, with one addition: a
built-in safety gate refuses to emit output that is not equivalent to the
input (see §2.4).

---

## 2. Fundamental constraints

### 2.1 The token contract

The formatted output must lex to **exactly the same significant token
sequence** as the source. "Significant" excludes only:

- comments (`//`, `/* */`) — preserved separately, see §9
- `;` statement separators — `a; b` is reprinted as two lines
- physical line breaks — layout is the formatter's job

Everything else is untouchable. In particular:

- **User parentheses are always preserved.** The parser keeps them in the AST
  (`Expr::Group`), so `(a + b)`, `(x -> x * 2)`, `m[(i)>(j)]`, `(f)(x)` all
  reprint exactly as written.
- **The formatter never inserts parentheses** into code that parsed without
  them.
- Surface sugar reprints as written: `x += 1`, `x++`, `x--`, `arr[i] = v`,
  `arr[i] += v` (recorded by the parser as `AssignSugar`), hot-def markers
  `x°` / `°x`, mutable params `name~`, input typespecs (`##.(5,2)`, `###(n)`,
  `##"`, `##'`, `#|var|`), interpolated strings `"a {b} c"`, `¶` vs `\\`,
  1-tuples `(1,)`, single vs double bracket extraction (`arr[i>a..b]` vs
  `arr[[path]]`), and export-block comma separators.

### 2.2 Never delete content

Every `//` and `/* */` comment in the source appears in the output (G4 in the
safety gate enforces the count). Blank lines between statements are preserved:
a gap of one or more blank lines in the source becomes exactly one blank line.

### 2.3 Idempotency

`zymbol fmt` twice produces the same output as once. The property test suite
(`tests/scripts/fmt_property.sh`, property P2) enforces this over the whole
corpus.

### 2.4 The safety gate (fail closed)

After producing output, the formatter verifies — inside
`crates/zymbol-formatter/src/gate.rs` — that:

- **G1**: the significant token stream is unchanged (§2.1),
- **G2**: the output still parses,
- **G3**: the statement tree has the same pre-order shape,
- **G4**: the comment count is unchanged.

On any mismatch `zymbol fmt` returns an error naming the first divergent
token and **leaves the file untouched**. A formatter bug can therefore refuse
to format a file, but it can never corrupt one.

---

## 3. Indentation

| Rule | Value |
|------|-------|
| Unit | 4 spaces (configurable via `--indent`) |
| Tabs vs spaces | Spaces by default; tabs via config |
| Level increase | Every block `{ }` opens a new level |
| Level decrease | Closing `}` returns to previous level |

The `}` that closes a block sits at the same indentation level as the
statement that opened it.

---

## 4. Spacing rules

### 4.1 Around assignment operators
One space before and after `=`, `:=`, `+=`, `-=`, `*=`, `/=`, `%=`, `^=`.

### 4.2 Around arithmetic and comparison operators
One space before and after `+`, `-`, `*`, `/`, `%`, `==`, `<>`, `<`, `>`,
`<=`, `>=`, `&&`, `||`.

> Note: Zymbol's not-equal operator is `<>`. The lexer intentionally rejects
> `!=` with a hint to use `<>`.

### 4.3 Range operator — **no spaces** around `..` (`1..10`, `arr$[2..5]`)

### 4.4 Symbol operators attach to their left operand
`$#`, `$+`, `$-`, `$--`, `$?`, `$??`, `$>`, `$|`, `$<`, `$^`, `$~~`, `$[`,
`$++`, `$~`: no space before (`arr$#`). Exception: `$+` keeps a space after
it (`result $+ element`).

### 4.5 `::` — no spaces (`module::function()`)

### 4.6 Tuple field access `.` — no spaces (`point.x`)

### 4.7 Lambda arrow `->` — one space each side

### 4.8 Pipe `|>` — one space each side

### 4.9 Concatenation `$++` — one space before

### 4.10 Output statement `>>`
One space after `>>`, one space between items. The `¶` (or `\\`) token joins
the preceding token on the same line — unless that line ends in a trailing
`//` comment, in which case the `¶` keeps its own line. Chained outputs
written on one source line (`>> a >> b ¶`) stay on one line.

---

## 5. Block and brace layout

### 5.1 Opening brace — always on the same line as the construct
### 5.2 `_` (else) and `_?` (else-if) — on the same line as the preceding `}`

### 5.3 Blank lines
One blank line is inserted before and after every top-level function
declaration. Source blank-line gaps elsewhere are preserved as exactly one
blank line (runs collapse).

### 5.4 Single-statement blocks (inline option)
With `inline_single_statement = true` (default), a block holding exactly one
*simple* statement (assignment, output, break, continue, return, expression)
may collapse to one line: `? found { @! }`. A block that contains a comment
never collapses — the comment needs a line of its own.

---

## 6. Match expressions (`??`)
Each arm is one line. Arms are not column-aligned.

## 7. Module files (`# name { }`)
Header, imports, and export block follow normal indentation rules. The export
block reprints the user's optional `,` separators, and a single-line export
block (`#> { add, PI }`) stays on one line.

## 8. Labeled loops
Canonical form `@:label`, `@:label!`, `@:label>`.

---

## 9. Comments

Comments are re-emitted by **source position** (span interleaving): the
formatter walks the AST in source order and inserts each comment before the
first statement that follows it, or at the end of the line it trails. The old
line-matching merge pass is gone — comment placement can no longer duplicate
or reorder code.

### 9.1 Trailing line comments
Stay on their line, separated by one space; alignment padding collapses to
one space.

### 9.2 Standalone line comments
Keep their own line and are re-indented to the surrounding block level.

### 9.3 Block comments
Preserved in full. Continuation lines lose the opening line's original
indentation and inherit the current block indentation, so the whole comment
moves together.

### 9.4 Known limitation
A comment in the middle of a multi-line *expression* migrates to the nearest
statement boundary. It is never lost (G4), but it can move.

---

## 10. Known normalizations

These are the only intentional differences between input and output, besides
whitespace:

| Normalization | Example |
|---------------|---------|
| `;`-separated statements split to lines | `a = 1; b = 2` → two lines |
| Blank-line runs collapse to one | `\n\n\n` → `\n\n` |
| Comment alignment padding collapses | `x = 5    // c` → `x = 5 // c` |
| Blank line added around top-level functions | §5.3 |
| `¶` join to the previous output line | §4.10 |

If `fmt` changes anything not in this table, it is a bug — and the safety
gate will normally have refused to emit it.

---

## 11. Configuration reference

| Option | Default | CLI flag | Description |
|--------|---------|----------|-------------|
| `indent_size` | 4 | `--indent N` | Spaces per indent level |
| `use_spaces` | true | — | Tabs via `FormatterConfig::with_tabs()` |
| `max_line_length` | 100 | — | Target line length |
| `max_inline_array_length` | — | — | Character budget for inline arrays |
| `inline_single_statement` | true | — | Collapse single-stmt blocks (§5.4) |
| `brace_same_line` | true | — | Opening brace placement |

---

## 12. Syntax coverage policy

The formatter's `format_statement` / `format_expr` matches are **exhaustive**:
adding a `Statement` or `Expr` variant fails compilation until the formatter
learns to print it. This is deliberate.

**Process rule:** a PR that adds parser syntax must, in the same PR:

1. add the formatter arm (the compiler enforces this),
2. make sure the parser records any surface form the AST would otherwise
   lose (see `AssignSugar`, `Newline.backslash`, `FlatExtractExpr.double_bracket`,
   `ExportBlock.commas`, `Expr::Group` for precedents),
3. add at least one corpus file exercising the new syntax and keep
   `tests/scripts/fmt_property.sh` green.

The property harness runs P1 (reparse), P2 (idempotence), P3 (runtime output
equality) and P4 (comment counts) over every `.zy` file in `tests/` and
`examples/`; `--baseline` mode gates CI on regressions.

---

## 13. Non-goals (explicit)

- **Linting** — use `zymbol check`
- **Style enforcement beyond layout** — naming conventions, idioms
- **Auto-import or auto-fix** — the formatter never adds new code
