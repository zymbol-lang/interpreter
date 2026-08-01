# Zymbol-Lang — Language Guide

> **Authoritative reference** — all examples verified empirically on both execution modes:
> `zymbol run` (tree-walker) and `zymbol run --vm` (register VM).
> If a construct is not documented here, it may not be implemented.

**Interpreter version**: v0.0.8
**Test coverage**: golden-file pairs verified on both engines (`vm_compare`); `@vm-skip` files excluded from VM parity

**New in v0.0.8**: `std/term` (terminal display metrics — column-accurate `width`, padding
and truncation), `##!` on a `Char` (its Unicode code point), match or-patterns
(`'p' || 'P' => …`, alternatives in one arm), `.zyp` packages
([§ Distributing a Multi-File Program](#distributing-a-multi-file-program-zyp)), and
automatic destruction at last use (auto-free — unobservable; it only lowers peak memory).
See [§17 Standard Library Modules](#standard-library-modules-std).

**New in v0.0.7**: typed/validated input (`<< ##.(5,2) "p" var`, see [§3 Input](#input-)) and
native standard-library modules `std/json`, `std/io`, `std/net`, `std/db` (see
[§17 Standard Library Modules](#standard-library-modules-std)). v0.0.6 added `std/math` and
`std/random`.

See also: [REFERENCE.md](REFERENCE.md) — limitations, error taxonomy, symbol table  
See also: [IMPLEMENTATION.md](IMPLEMENTATION.md) — EBNF grammar, coverage status, TW/VM internals

---

## Table of Contents

0. [Design Philosophy](#0-design-philosophy)
1. [Running Programs](#1-running-programs)
1b. [Lexical Structure](#1b-lexical-structure)
2. [Data Types](#2-data-types)
3. [Output and Input](#3-output-and-input)
3b. [TUI Primitives](#3b-tui-primitives)
4. [Variables and Constants](#4-variables-and-constants)
5. [Operators](#5-operators)
6. [Control Flow](#6-control-flow)
7. [Match](#7-match)
8. [Loops](#8-loops)
9. [Functions](#9-functions)
10. [Lambdas and Closures](#10-lambdas-and-closures)
10b. [Evaluation Order and Capture Semantics](#10b-evaluation-order-and-capture-semantics)
11. [Arrays](#11-arrays)
11b. [Destructuring Assignment](#11b-destructuring-assignment)
11c. [Multi-dimensional Indexing](#11c-multi-dimensional-indexing)
12. [Tuples](#12-tuples)
13. [Strings](#13-strings)
14. [Higher-Order Functions](#14-higher-order-functions)
15. [Pipe Operator](#15-pipe-operator)
16. [Error Handling](#16-error-handling)
17. [Modules](#17-modules)
18. [Data Operators](#18-data-operators)
18b. [Numeral Modes](#18b-numeral-modes)
19. [Shell Integration](#19-shell-integration)
22. [Verified Examples](#22-verified-examples)

---

## 0. Design Philosophy

### Origin: An Esolang That Grew

Zymbol started as an **esoteric programming language** — a small, experimental construct with a single guiding question: *what happens if you remove every keyword from a programming language?* No `if`, no `while`, no `function`, no `return`. Nothing borrowed from English or any other natural language. Just symbols.

That constraint was minimalist by design, in the tradition of esolangs: a tight idea taken seriously, with no ambition beyond exploring whether it worked. The original publication is on [esolangs.org](https://esolangs.org). It was a toy with a point.

Then the idea grew — like a little monster. Not because features were added for their own sake, but because the founding constraint turned out to have more depth than expected. Once you commit to "no keywords", you discover that symbols can carry consistent meaning across very different contexts (`_` is always non-binding, `#` is always meta-level), and that Unicode support is not an afterthought but a natural consequence of the same principle. The language kept growing as each piece clicked into place.

**The founding question**, now stated plainly: every mainstream language — Python, Java, Ruby, Go, Rust — shares an invisible assumption that the programmer reads English. Keywords like `if`, `while`, `function`, `return` are English words. A developer in Spanish, Arabic, or Devanagari is permanently coding in a second language at the syntactic level, even when identifiers and strings can be localized.

Removing keywords entirely is the minimum change needed to break that assumption. A symbol carries no etymology. `?` does not say *if* in English — it says *condition* in the visual grammar of the program. A developer writing `? edad >= 18` and one writing `? age >= 18` are doing exactly the same thing, and neither is translating.

The practical result: any human language can be the *native* language of a Zymbol program. Spanish with full accents (`función`, `índice`), Devanagari (`सक्रिय`, `फलन`), Arabic (`متغير`, `دالة`), Korean (`변수`, `함수`), and yes — Klingon pIqaD for the ones who want to program in the language of the Empire. The digit block is registered (CSUR U+F8F0–U+F8F9) and the interpreter supports it completely. No judgment. It is the logical endpoint of the principle.

This is **not** a reaction to APL or J or K. Those languages are dense because they optimize for mathematical array notation. Zymbol is dense because it refuses to reserve any identifier for the runtime. The convergence in symbol count is incidental; the motivations are orthogonal.

### Symbolic Minimalism, Not Minimal Language

Zymbol is no longer a small language. It has arrays, tuples, closures, modules, HOFs, pattern matching, a pipe operator, shell integration, 69 Unicode numeral scripts, and multi-dimensional indexing. The esolang became a general-purpose language.

What remained minimal is the **mechanism of growth**: every new construct is expressed through existing symbols, or through a new symbol that the programmer learns once and recognizes everywhere. No construct ever borrows a word from any natural language. The constraint is not "few features" — it is "no vocabulary debt to any human tongue."

The measure of Zymbol's minimalism is: *can this new construct be expressed with existing symbols, or does it require coining a new one?* If the answer is "new symbol", it enters the grammar reluctantly, with a clear consistent meaning. That discipline is the minimalism. The feature count is a separate axis.

### Symbolic Coherence: Shared Meaning, Similar Spirit

Zymbol does not enforce one symbol per concept. Instead, a symbol may appear in multiple contexts **when the underlying spirit is the same**. The reader learns the symbol's character once and recognizes it across uses.

**`_` — the non-binding marker**

`_` always means *"this position does not matter / is not bound"*:

| Context | Example | Meaning |
|---------|---------|---------|
| else branch | `_ { }` | default case — no condition binds |
| else-if | `_? x > 0 { }` | else-if — extends the non-binding chain |
| wildcard in match | `?? x { _ => "other" }` | catch-all arm — value not bound |
| destructuring ignore | `[a, _, c] = arr` | middle element not bound |
| pipe placeholder | `x \|> f(_, 2)` | position of piped value in args |
| unused variable prefix | `_i:1..5` | iterator declared but not used in body |

All are the same idea: *this slot is intentionally left unbound*.

**`#` — the meta-level marker**

`#` marks constructs that operate at the **meta level** — above individual values:

| Context | Example | Meaning |
|---------|---------|---------|
| Boolean literals | `#1` / `#0` | typed truth values (not integers) |
| Type reflection | `x#?`, `##->`, `###`, `##.` | inspect the type of a value; type symbols are pure symbol sequences |
| Precision / cast | `#.2\|x\|`, `##.x`, `###x` | numeric transformations at the type boundary |
| Module declaration | `# calc` | names the file as a module (meta-identifier) |
| Module export | `#> { }` | declares the public surface of a module |
| Module import | `<# ./calc => c` | brings a module into scope |
| Numeral mode | `#०९#` | switches output digit script |

Types, modules, and numeral modes share `#` because all three are about *what something is or how it is represented*, not *what value it holds*.

### Self-Referential Grammar

Zymbol's symbolic vocabulary is its own. The symbols have no external standard to conform to — their meaning is defined by the language itself and built up through consistent use. A programmer learns Zymbol by reading Zymbol, not by mapping it onto another language.

This creates an initial learning curve. It also means the language can evolve its symbol system with full internal consistency, without being constrained by conventions inherited from English-based predecessors.

### The Numeral Modes as Proof of Concept

The 69 Unicode digit scripts (`#०९#` Devanagari, `#٠٩#` Arabic-Indic, `#๐๙#` Thai, `#𝟎𝟗#` Mathematical Bold, Klingon pIqaD, and 64 others) are not a curiosity. They are the most explicit demonstration of the founding principle: a program written entirely in Devanagari — identifiers, literals, output — is a first-class Zymbol program. No special mode, no pragma, no flag. That is what "no hegemony" means in practice.

---

## 1. Running Programs

```bash
zymbol run program.zy              # tree-walker (canonical, best error messages)
zymbol run --vm program.zy         # register VM (faster for compute-heavy programs)

zymbol --help
zymbol run --help
```

**When to use each mode:**
- **Tree-walker**: canonical behavior, descriptive error messages, debugging
- **VM**: production, ~1.1–1.5× faster than Python for most workloads

Both modes produce **identical output** on the full parity suite
(`bash tests/scripts/vm_compare.sh`; 507/507 as of v0.0.7).

**Diagnostic tiers.** The same analyzers back every entry point, with one
deliberate difference in coverage:

| Tier | Reports |
|------|---------|
| `zymbol run` | Fatal errors (semantic + type) and usage warnings (unused variables, type mismatches) — then executes. Module problems surface at import time. |
| `zymbol check` | Everything `run` reports **plus** static module analysis (E001/E002/E009, export validation) and ambiguous-lifetime warnings, without executing. |
| LSP (editor) | Same findings as `check`, as you type. On files with parse errors the editor keeps analyzing the recovered AST (so it may show advisory warnings where `check` stops at the parse error). |

---

## 1b. Lexical Structure

### Source Encoding

Zymbol source files are UTF-8. All Unicode scripts are supported in identifiers, string literals, and numeral literals. Grapheme clusters are tracked for accurate error positions.

### Identifiers

An identifier begins with a Unicode letter or `_`, followed by zero or more Unicode letters, digits, or `_`.

```
identifier ::= (letter | '_') (letter | digit | '_')*
letter     ::= any character for which Unicode is_alphabetic() returns true
digit      ::= any character for which Unicode is_alphanumeric() returns true (but not alphabetic)
```

All scripts are allowed: `camelCase`, `snake_case`, `PascalCase`, `café`, `αβγ`, `変数`, `متغير` are all valid identifiers.

Identifiers must not collide with symbolic operators (e.g., `$>`, `@`, `?` are not identifiers).

### Comments

```
// single-line comment — extends to end of line

/* multi-line comment
   can span multiple lines
   /* nesting is supported */
   still inside the outer comment */
```

Both forms are preserved by the formatter. There are no doc-comments.

### Whitespace

Whitespace (spaces, tabs, newlines) is **not significant** as a token separator — operators and identifiers may appear adjacent to each other without spaces. Newlines do not terminate statements; all statements must be explicitly terminated.

Exception: `@label` — the `@` loop operator and a following identifier are lexed as a single `AtLabel` token with no intervening space. Adding a space changes the meaning: `@ label` starts a new loop iteration with `label` as the first expression.

### String Literals

String literals are delimited by double quotes `"..."`.

**Escape sequences:**

| Escape | Result |
|--------|--------|
| `\n`   | newline (U+000A) |
| `\t`   | horizontal tab (U+0009) |
| `\r`   | carriage return (U+000D) |
| `\"`   | double quote |
| `\\`   | backslash |
| `\{`   | literal `{` (suppresses interpolation) |
| `\}`   | literal `}` |

Any other `\X` sequence passes `X` through unchanged.

There are no Unicode escape sequences (`\uXXXX` is not supported).

**String interpolation:**

Embed variable values directly in a string with `{varname}`:

```
name = "Alice"
>> "Hello, {name}!" ¶       // Hello, Alice!
```

Only a simple identifier is allowed inside the braces — expressions must be
assigned to a variable first. "Identifier" means exactly what it means
everywhere else in the language: any Unicode letter, `_`, and any non-operator
symbol, including scripts outside the letter categories. A name written in
kanji, in Hangul, in Private Use Area glyphs such as pIqaD, or with an emoji
interpolates like any other.

```zymbol
整 = 7
>> "kanji: {整}" ¶              // kanji: 7
```

> Before v0.0.8 this position used a narrower rule than the lexer's, so a
> program whose identifiers were valid everywhere else could still fail to
> interpolate them.

### Numeric Literals

Integer literals may use any Unicode digit script, but a single literal must use one script consistently:

```
x = 42         // ASCII digits
y = ४२         // Devanagari digits — same value
```

Floating-point literals use ASCII decimal notation: `3.14`, `2.5e10`.

Character literals use single quotes: `'a'`, `'\n'`, `'\t'`. Numeric character codes: `0x41` (hex), `0b01000001` (binary), `0o0101` (octal), `0d65` (decimal).

Boolean literals: `#1` (true), `#0` (false).

### Explicit Newline Tokens

Zymbol has two ways to emit a newline in output — both produce a literal newline character in the program's output stream, not in the source:

- `¶` (pilcrow, U+00B6) — newline token
- `\\` (double backslash) — alternative newline token

### Reserved Symbols

Zymbol is keyword-free — there are no reserved English words. All control-flow, I/O, and type constructs use symbolic operators. The complete operator set is listed in §21.

The following identifiers have conventional meaning but are not reserved: `_err` (caught error in `:!` blocks).

---

## 2. Data Types

### Value Types

| Type | Literal / source | `#?` symbol | Notes |
|------|-----------------|-------------|-------|
| Int | `42`, `-7` | `###` | 64-bit signed |
| Float | `3.14`, `1.5e10` | `##.` | Scientific notation supported |
| String | `"text"` | `##"` | Interpolation: `"Hello {name}"` |
| Char | `'A'` | `##'` | Single Unicode character |
| Bool | `#1`, `#0` | `##?` | NOT numeric — `#1` ≠ `1` |
| Array | `[1, 2, 3]` | `##]` | Homogeneous (same type) |
| Tuple | `(a, b)` | `##)` | Positional |
| NamedTuple | `(x: 1, y: 2)` | `##)` | Named fields |
| Function | named function ref | `##()` | First-class since v0.0.4; display `<funct/N>` |
| Lambda | `x -> x * 2` | `##->` | Lambda definition symbol; display `<lambd/N>` |
| Error | _(runtime value)_ | `##<Kind>` | Type IS the kind: `##Index`, `##Div`, `##IO`, … |
| Unit | _(void return)_ | `##_` | Returned by functions with no `<~`; display is empty |

### Non-value Types

These constructs exist in Zymbol but are **not first-class values** — they cannot be stored in variables, inspected with `#?`, or passed as arguments.

| Construct | Usage | Why not a value |
|-----------|-------|-----------------|
| Range (`1..5`) | Loop iterator only: `@ i:1..5 { }` | Storing a range raises a runtime error |
| Module (`<# ./m => m`) | Namespace only: `m::fn()`, `m.CONST` | Module alias is not a runtime value |

### Type Inspection with `#?`

The `#?` postfix operator returns a 3-tuple: `(type_symbol, count, display)`.

| Type | `#?` result | `count` meaning |
|------|------------|-----------------|
| Int | `(###, N, val)` | digit count |
| Float | `(##., N, val)` | digit count of display |
| String | `(##", N, val)` | character length |
| Char | `(##', 1, val)` | always 1 |
| Bool | `(##?, 1, val)` | always 1 |
| Array | `(##], N, val)` | element count |
| Tuple / NamedTuple | `(##), N, val)` | field count |
| Function | `(##(), N, <funct/N>)` | arity |
| Lambda | `(##->, N, <lambd/N>)` | arity |
| Error | `(##Kind, N, ##Kind(msg))` | message length |
| Unit | `(##_, 0, )` | always 0 |

```zymbol
x = 42
>> x#? ¶               // → (###, 2, 42)

f(a, b) { <~ a + b }
fn_ref = f
>> fn_ref#? ¶          // → (##(), 2, <funct/2>)

lam = (a, b) -> a + b
>> lam#? ¶             // → (##->, 2, <lambd/2>)

// Extract type symbol
meta = x#?
t = meta[1]
>> t ¶                 // → ###
```

Named functions use `##()` — the call-syntax symbol. Lambdas use `##->` — their definition syntax. This distinction is visible in both the type symbol (field 1) and the display string (field 3): `<funct/N>` vs `<lambd/N>`.

Error values use their **kind** as the type symbol — there is no generic `##error` symbol:

```zymbol
get_err() { !? { <~ [1, 2][99] } :! { <~ _err } }
e = get_err()
>> e#? ¶               // → (##Index, 57, ##Index(array index out of bounds: ...))
t = (e#?)[1]
>> t ¶                 // → ##Index
```

---

## 3. Output and Input

### Output `>>`

`>>` does **not** add a newline automatically. Use `¶` (pilcrow, AltGr+R on Spanish keyboard) or `\\` explicitly.

```zymbol
>> "Hello" ¶                        // explicit newline
>> "a=" a " b=" b ¶                 // multiple items by juxtaposition (Haskell-style)
>> a b c ¶                          // identifiers directly
>> add(2, 3) ¶                       // function call in any position
>> "sum=" add(1, 2) " double=" double(5) ¶   // mixed
>> (arr$#) ¶                        // postfix operators require parentheses in >>
```

Output uses **juxtaposition** (Haskell-style) — values separated by spaces are printed in sequence. `+` is for numeric addition only; using it with strings is a type error:

```zymbol
>> "Score: " score ¶               // ✅ juxtaposition — canonical form
>> 10 + 5 ¶                        // ✅ numeric addition in output → 15
>> "Score: " + score ¶             // ✗ type error — + is not string concat
```

**Parenthesized expressions** can be used as output items directly:

```zymbol
ok = a == b
>> "Equal: " ok ¶                  // ✅ variable
>> "Equal: " (a == b) ¶            // ✅ parenthesized expression — two separate items
>> "Sum: " (x + y) ¶               // ✅ arithmetic in parens
```

> **Note**: `identifier(args)` is a function call in `>>`. `"literal"(expr)` is two
> separate items — the literal and the parenthesized expression — never a call.
> Literals (strings, numbers, booleans) are not callable.

### Newline

```zymbol
>> "text" ¶       // ¶ pilcrow
>> "text" \\      // \\ also works
>> ¶              // blank line
```

### Input `<<`

```zymbol
<< name                        // read into variable (no prompt)
<< "Enter name: " name         // with prompt string
<< "Hello {name}: " response   // interpolated prompt
<< #|n|                        // numeric: parse to Int/Float, else String
```

#### Typed / validated input

A type marker placed **before** the prompt constrains and converts the value at read
time. The markers reuse the cast symbols, with an optional size in parentheses, and the
target variable comes **last**: `<< <typespec> "prompt" var`. On invalid input the prompt
is shown again (it re-prompts until the value is valid); end-of-input aborts.

```zymbol
<< ##.(5,2) "Decimal: " monto   // Float, ≤5 total digits, ≤2 decimals (e.g. 999.99)
<< ##.      "Float: "   f        // Float, any valid number
<< ###(4)   "Entero: "  n        // Int, ≤4 digits (max 9999)
<< ###      "Entero: "  k        // Int, any size
<< ##"(20)  "Texto: "   s        // String, ≤20 characters
<< ##'      "Char: "    c        // exactly one character → Char
```

| Typespec | Reads | Validates | Type |
|---|---|---|---|
| `##.` | free float | parses as a number | `Float` |
| `##.(T,D)` | decimal | ≤T digits total, ≤D decimals, no exponent | `Float` |
| `###` / `###(N)` | integer | integer; `(N)` caps digit count | `Int` |
| `##"` / `##"(N)` | text | `(N)` caps character count | `String` |
| `##'` | one character | length must be exactly 1 | `Char` |

Both engines (tree-walker and `--vm`) validate identically. A leading sign is allowed for
`###`/`##.` and does not count toward the digit budget.

### CLI Arguments

```zymbol
>< args                        // capture CLI args as string array
>> args ¶
// Run: zymbol run script.zy one two three
// → [one, two, three]
```

`><` works in both engines (tree-walker and `--vm`).

---

## 3b. TUI Primitives

Terminal control operators for building interactive terminal UIs.
Some TUI primitives are tree-walker only; see the per-primitive notes below for VM support status.

> Most TUI primitives require an enclosing `>>| { }` block (raw mode + alternate screen).
> `>>!` and `>>?` work standalone. `<<|` and `<<|?` require raw mode provided by `>>|`.

### Sleep — `@~`

```zymbol
@~ 500        // pause execution for 500 milliseconds
@~ 1000       // 1 second
```

### Clear Screen — `>>!`

```zymbol
>>!           // clear the terminal and move cursor to home (row 1, col 1)
```

### Query Terminal Size — `>>?`

Returns a `[rows, cols]` array with the current terminal dimensions:

```zymbol
[H, W] = >>?
>> "Terminal: " H "x" W ¶     // e.g. Terminal: 40x120
```

### Positioned Output — `>>~`

Print at a specific terminal position. Cursor is moved but not restored after printing.

```zymbol
// Full form: (row, col, BKS, fg, bg) where BKS = Bold(1)+Italic(2)+Underline(4)
>>~ (5, 10, 0, 255, 0) > "hello"       // row 5, col 10, default style, fg=255 bg=0

// Position only — no style change
>>~ (3, 1) > "header"

// Sparse form — omit any slot with a comma
>>~ (,,, 196) > "red text"             // fg=196, no cursor move
>>~ (1,, 1) > "bold at row 1"          // bold, column stays unchanged
```

Variable-based position (tuple variable):

```zymbol
pos = (10, 5)
>>~ pos > "at pos"
```

ANSI color indices (0–255): standard 16 system colors, then 6×6×6 cube (16–231),
then grayscale (232–255). `0` = use terminal default.

### Key Input

`<<|` reads one keypress (blocking). `<<|?` polls without blocking.

```zymbol
// Blocking — waits until a key is pressed
<<| k
>> "key: " k ¶

// Non-blocking — '\0' if no key is pending
<<|? k
? k <> '\0' { >> "pressed: " k ¶ }
```

Special keys are mapped to single-character symbols:

| Key | Value |
|-----|-------|
| Arrow Up | `'↑'` (U+2191) |
| Arrow Down | `'↓'` (U+2193) |
| Arrow Left | `'←'` (U+2190) |
| Arrow Right | `'→'` (U+2192) |
| Enter | `'\n'` |
| Escape | `'\x1b'` |
| Other | the character as-is |

> The arrows come back as the arrow glyphs themselves, not as letters. Match
> them directly — `? k == '↑' { }` — and note that this leaves every ASCII
> letter free for commands, uppercase included.

### TUI Block — `>>|`

Enters a full-screen TUI context: alternate screen + raw mode. Cleans up on exit.

```zymbol
>>| {
    >>!                            // clear alternate screen
    >>~ (1, 1) > "Press q to quit"
    @ {                            // game loop
        <<| k
        ? k == 'q' { @! }
        // render frame
    }
}
// terminal restored to normal after the block
```

> `>>|` errors if the process is not attached to a TTY (e.g. redirected output).
> Use it only for interactive programs.

---

## 4. Variables and Constants

```zymbol
x = 10              // mutable variable
PI := 3.14159       // constant (immutable — reassignment is a runtime error)
name = "Alice"
active = #1

// Explicit destruction
\ x                 // releases x from current scope
```

### Constant Scope

Constants follow the same lexical rules as variables, with one deliberate
exception — **top-level constants are global**:

- A `:=` declared at the top level of a script is visible **everywhere** in
  that script: inside any function at any call depth, through recursion and
  lambda frames (v0.0.8 — previously the tree-walker lost them at call
  depth ≥ 2). It stays immutable everywhere.
- A `:=` declared inside a block dies when the block ends, like a variable.
- Redeclaring a constant visible in the current scope is an error.
- Module code never sees the importing script's constants — modules only see
  their own state (see section 17).

### Compound Assignment Operators

```zymbol
x = 10
x += 5    // x = 15
x -= 3    // x = 12
x *= 2    // x = 24
x /= 3    // x = 8
x %= 3    // x = 2
x ^= 2    // x = 4  (x = x ^ 2)
x++       // x = 5  (equivalent to x += 1)
x--       // x = 4  (equivalent to x -= 1)
```

### Variable Scope

Regular variables follow **lexical scoping**: a variable declared in an outer block is
visible and writable from any inner block. A variable declared inside a block is
destroyed automatically when that block ends — it is not visible outside.

```zymbol
x = 10

? x > 0 {
    y = x * 2    // x is visible here (outer → inner: allowed)
    >> y ¶       // → 20
}

// y no longer exists here — destroyed when the block ended
// x is still alive
>> x ¶           // → 10
```

This applies to `? {}`, `_? {}`, `_ {}`, `@ {}`, and any other block construct.

```zymbol
total = 0
@ i:1..5 {
    partial = i * 10    // partial lives only for this iteration
    total = total + i   // total is outer — writable from here
}
>> total ¶   // → 15
// partial no longer exists
```

### Underscore Variables (`_name`)

A variable whose name begins with `_` has **exact block scope**: it exists only within
the block where it is declared. It is not visible from inner blocks, outer blocks, or
sibling blocks.

```zymbol
// Valid — _temp used only in its own block
? #1 {
    _temp = expensive_call()
    >> _temp ¶
}   // _temp destroyed here

// Valid — independent _temp in a sibling block
? #1 {
    _temp = other_call()
    >> _temp ¶
}
```

```zymbol
// ERROR — _outer declared in outer block, accessed from inner block
? #1 {
    _outer = 42
    ? #1 {
        >> _outer ¶   // semantic error: cannot access underscore variable from inner scope
    }
}
```

```zymbol
// ERROR — _counter declared in outer scope, modified from loop body
_counter = 0
@ i:1..5 {
    _counter = _counter + 1   // semantic error: cannot access underscore variable from inner scope
}
```

Use a regular variable when you need to read or mutate a value across scope boundaries:

```zymbol
// Correct pattern: pre-declare as a regular variable
cmd  = ""
args = ""
? has_space {
    cmd  = input$[1..p-1]
    args = input$[p+1..-1]
}
// cmd and args are still alive here
```

The `_` prefix is intended for short-lived temporaries that must not leak outside their
block. The compiler enforces this at the semantic analysis phase.

### Explicit Lifetime End

`\ var` destroys a variable before its block ends:

```zymbol
? #1 {
    _resource = load_data()
    process(_resource)
    \ _resource           // released here, before block exit
    do_other_work()       // _resource no longer exists
}
```

This works for both regular and `_`-prefixed variables.

### Variable Lifecycle

The complete life of a variable, as implemented in both engines (v0.0.8):

| Phase | When | Mechanism |
|-------|------|-----------|
| Birth | First assignment (`=`, destructuring, `<<`, `<<\|`, `><`) | Created in the innermost scope; if the name is visible in an outer scope, that variable is updated instead (no shadowing) |
| Block death | Enclosing block ends | Scope popped; block-local variables released |
| Frame death | Function returns | The whole call frame is released |
| **Auto-free** | **Right after the statement containing its last use** | Last-use analysis releases the value early — see below |
| Explicit death | `\ var` | Immediate destruction; reassignment resurrects the name |
| Program end | Last statement | Everything remaining is released |

**Automatic destruction at last use (auto-free)** — since v0.0.8, both engines
release a variable's memory right after the last statement that mentions it,
instead of waiting for its scope to end. This is an **invisible optimization**:
it never changes what a correct program prints or returns — it only lowers peak
memory (e.g. a large array processed early in a long script is reclaimed
immediately after its last use).

The analysis is deliberately conservative. A variable is **never** auto-freed
when it is: a constant (`:=`), hot (`x°`/`°x`), `_`-prefixed, a module-level
binding, an output/mutable parameter (`<~`/`~`), or a free variable of a named
function that is used as a first-class value. Mentions inside string
interpolations (`"{var}"`), lambda bodies, nested blocks, and loop bodies all
count as uses. When in doubt, the variable simply lives until its scope ends,
as before.

> `\ var` remains the only *observable* destruction: using a variable after
> `\` is a lifetime error. Auto-free never produces that error in a correct
> program — if you ever see `internal: use after auto-destruction`, it is an
> interpreter bug: please report it.

### String Interpolation

Works in **any context** — assignments, arguments, array literals, etc.:

```zymbol
greet(s) { <~ s }

name = "World"
msg = "Hello {name}!"           // in assignment
greet("Hello {name}")           // as argument
arr = ["item {name}", "x"]      // in array literal
x = 42
combined = "val={x}, name={name}"
>> combined ¶                   // → val=42, name=World
```

To include a **literal `{` or `}`** in a string (without triggering interpolation), escape with a backslash:

```zymbol
>> "Use \{ and \} as literal braces" ¶   // → Use { and } as literal braces
json = "\{\"key\":\"value\"\}"
>> json ¶                                // → {"key":"value"}
```

> **⚠ False warning**: `unused variable 'name'` may appear even when `name` is used
> inside an interpolated string. This is a static analyzer bug — ignore it.

### Hot Definition Operator `°` (U+00B0)

Two forms control where the auto-initialized variable lives:

| Form | Position | Anchors to |
|------|----------|-----------|
| Postfix `x°` | LHS or RHS | Nearest enclosing `@` scope — dies when the loop ends |
| Prefix `°x` | LHS or RHS | Scope **above** the nearest `@` — survives the loop |

Both forms auto-initialize to the **neutral value** on the very first use.

**Prefix `°x` on RHS** — the cleanest form for loop accumulators:

```zymbol
// °total in RHS: auto-init above @ on first use, survives loop
@ item:[10, 20, 30] {
    total = °total + item    // first use: total = 0 + 10 = 10
}
>> total ¶                   // → 60

// °arr in RHS: auto-init to [] above @, survives loop
@ x:[1, 2, 3] {
    arr = °arr$+ x           // first use: arr = []$+ x = [x]
}
>> arr ¶                     // → [1, 2, 3]

// °x on LHS: equivalent for compound operators
@ item:[10, 20, 30] {
    °sum += item             // same semantics as sum = °sum + item
}
>> sum ¶                     // → 60

// x° (postfix): lives only while the loop runs
@ i:[1, 2, 3] {
    i° += 1                  // visible inside the loop only
}
// i is NOT accessible here — it died with the loop scope
```

**String accumulation** — use `°x` on LHS with plain RHS:

```zymbol
@ ch:["a", "b", "c"] {
    °resultado = resultado ch    // neutral "" on first use; survives loop
}
>> resultado ¶                   // → abc
```

**Prefix `°x` in nested scopes:**

```zymbol
// °pares inside ? inside @ — anchors above the @, survives to global
@ i:[1, 2, 3, 4, 5] {
    ? i % 2 == 0 {
        °pares += i
    }
}
>> pares ¶               // → 6

// °acum_k inside nested @ k — anchors to outer @ j scope
@ _j:[1, 2] {
    @ k:[10, 20] {
        °acum_k += k
    }
    >> acum_k ¶          // → 30  (after _j=1)
                         // → 60  (after _j=2, persists across iterations)
}
```

**Outside any loop**, both `x°` and `°x` anchor to the global/function scope:

```zymbol
x° *= 7    // outside any loop → global scope → x = 7
>> x ¶     // → 7
```

**Neutral values by context:**

| Operation | Neutral |
|-----------|---------|
| `+=` `-=` | `0` (Int) or `0.0` (Float) |
| `*=` `/=` | `1` |
| `$+` (array append) | `[]` |
| juxtaposition (string) | `""` |

**Semantic warnings** — emitted but not errors:
- `x° ^= 2` — hot-def power: always `0` on first use

> Both `x°` and `°x` work in **both engines** (tree-walker and `--vm`).
> Inside a function that contains no `@` loop of its own, they anchor to the
> function's scope — the variable dies when the function returns, even when the
> function is called from inside a caller's loop (fixed in v0.0.8; previously
> this panicked the tree-walker).

---

## 5. Operators

### Arithmetic

```zymbol
a = 10
b = 3
>> a + b ¶   // 13
>> a - b ¶   // 7
>> a * b ¶   // 30
>> a / b ¶   // 3  (integer division when both operands are Int)
>> a % b ¶   // 1  (modulo)
>> a ^ b ¶   // 1000 (exponentiation)
>> -a ¶      // -10 (unary negation)
```

### Comparison

```zymbol
a == b    // equal
a <> b    // not equal
a < b     // less than
a <= b    // less than or equal
a > b     // greater than
a >= b    // greater than or equal
```

**Ordering (`<`, `<=`, `>`, `>=`) follows one rule:**

- **Numeric** when *both* sides are numbers. A string counts as a number when
  `#|…|` would convert it — digits from **any** of the 69 supported scripts:

  ```zymbol
  >> ("10" > "9") ¶      // #1 — 10 > 9, not codepoint order
  >> ("१०" > "९") ¶      // #1 — same comparison, Devanagari digits
  >> ("४२" > 5) ¶        // #1
  >> ("५" > "४") ¶       // #1
  ```

  No script is privileged: whatever ASCII digits do, every other script does.
- **Lexicographic** when both sides are non-numeric text (`"abc" < "abd"` → `#1`).
- **An error** when a number meets text that is not a number
  (`"abc" > 5` → *cannot compare string 'abc' with integer 5*).

Chars compare by code point and Bools order `#0 < #1`.

Equality is *not* part of this rule: `==` never coerces, so `"5" == 5` is `#0`
in every engine, and so is `"५" == 5`.

### Logical

```zymbol
#1 && #0   // #0 (false)
#1 || #0   // #1 (true)
!#1        // #0 (not)
```

Logical operators always return a Bool. Under an active numeral mode the result
is displayed with the active script digit:

```zymbol
#०९#
>> (#1 && #0) ¶   // → #०  (false in Devanagari)
>> (#1 || #0) ¶   // → #१  (true  in Devanagari)
>> !(#0) ¶        // → #१
```

### String Concatenation

Two correct forms — use the one that fits the context:

```zymbol
name = "Alice"
n = 42

// 1. Juxtaposition in >> (canonical output form)
>> "Hello " name " you have " n " items" ¶

// 2. Interpolation (most readable for complex strings)
desc = "Hello {name}, you have {n} items"
```

> **Note**: `+` is for **numeric addition only**. `"text" + value` is a type error.
> Use juxtaposition or interpolation for strings.

---

## 6. Control Flow

```zymbol
x = 7

// Simple if
? x > 0 { >> "positive" ¶ }

// if-else
? x > 0 {
    >> "positive" ¶
} _ {
    >> "not positive" ¶
}

// if-elseif-else
? x > 100 {
    >> "large" ¶
} _? x > 0 {
    >> "positive" ¶
} _? x == 0 {
    >> "zero" ¶
} _ {
    >> "negative" ¶
}
```

`{ }` braces are **required** even for single-statement bodies.

---

## 7. Match

`??` is **pure pattern matching** — it does not evaluate boolean conditions (use `?`/`_?` for
conditional branching). Six pattern types are available: Literal, Range, Comparison, Wildcard,
Ident, and List. Any of them can be combined with `||` into alternatives.

### Literal and Range Patterns

```zymbol
score = 85
grade = ?? score {
    90..100 => 'A'
    80..89  => 'B'
    70..79  => 'C'
    60..69  => 'D'
    _       => 'F'
}
>> "grade: " grade ¶

color = "red"
code = ?? color {
    "red"   => "#FF0000"
    "green" => "#00FF00"
    "blue"  => "#0000FF"
    _       => "#000000"
}
>> code ¶
```

### Comparison Patterns

A comparison pattern (`< expr`, `> expr`, `<= expr`, `>= expr`, `== expr`, `<> expr`) implicitly
compares the scrutinee against `expr`. Arms are tested in order; first match wins.

```zymbol
temperature = -5
state = ?? temperature {
    < 0   => "ice"
    < 20  => "cold"
    < 35  => "warm"
    _     => "hot"
}
>> state ¶    // → ice

n = 42
?? n {
    == 0    => { >> "zero" ¶ }
    < 0     => { >> "negative" ¶ }
    _       => { >> "positive: " n ¶ }    // → positive: 42
}
```

### Ident Patterns

An identifier used as a pattern looks up the named variable at runtime:
- **Scalar variable** → equality check (`scrutinee == var`)
- **Array variable** → containment check (`scrutinee ∈ var`)

```zymbol
expected = 200
code = 200
r1 = ?? code {
    expected => "ok"
    _        => "fail"
}
>> r1 ¶    // → ok

weekdays = ["Mon", "Tue", "Wed", "Thu", "Fri"]
day = "Mon"
r2 = ?? day {
    weekdays => "weekday"
    _        => "weekend"
}
>> r2 ¶    // → weekday
```

### List Patterns

`[...]` patterns have **dual semantics** based on the scrutinee's type at runtime:

- **Array scrutinee** → structural match (length + element-by-element)
- **Scalar scrutinee** → containment: does the scalar appear in the literal list?

```zymbol
// Scalar containment
n = 3
label = ?? n {
    [1, 2] => "low"
    [3, 4] => "mid"
    [5, 6] => "high"
    _      => "other"
}
>> label ¶    // → mid

// Structural array match
cmd = ["run", "main.zy"]
?? cmd {
    ["run", _]    => { >> "run command" ¶ }
    ["build", _]  => { >> "build command" ¶ }
    []            => { >> "empty" ¶ }
    _             => { >> "unknown" ¶ }
}
// → run command

// Match on array length/shape
data = [10, 20, 30]
?? data {
    [_]       => { >> "one element" ¶ }
    [_, _]    => { >> "two elements" ¶ }
    [_, _, _] => { >> "three elements" ¶ }
    _         => { >> "more" ¶ }
}
// → three elements
```

### Or Patterns — Alternatives with `||`

Any two patterns can be joined with `||`. The arm matches when **any** alternative matches;
alternatives are tested left to right and the first one that matches wins.

```zymbol
key = 'P'
?? key {
    'p' || 'P' => { >> "pause" ¶ }
    'q' || 'Q' => { >> "quit" ¶ }
    _          => { >> "other" ¶ }
}
// → pause
```

Alternatives are not limited to literals — ranges, comparisons, idents and list patterns all
combine freely, and an arm may chain three or more:

```zymbol
n = 25
zone = ?? n {
    1..10 || 20..30 => "in range"
    0               => "zero"
    _               => "outside"
}
>> zone ¶    // → in range

v = 150
state = ?? v {
    < 0 || > 100 => "extreme"
    _            => "normal"
}
>> state ¶    // → extreme

d = 6
kind = ?? d {
    1 || 3 || 5 || 7 => "odd"
    2 || 4 || 6 || 8 => "even"
    _                => "out of range"
}
>> kind ¶    // → even

cmd = ["build", "main.zy"]
?? cmd {
    ["run", _] || ["build", _] => { >> "known command" ¶ }
    _                          => { >> "unknown command" ¶ }
}
// → known command
```

> **Note**: For a plain "scalar is one of these literals" test, the list pattern `['p', 'P']`
> (containment, see above) is equivalent to `'p' || 'P'`. `||` is the more general form — it is
> the only way to mix pattern *kinds* in a single arm.

`||` binds only at the top level of an arm, so list elements stay unambiguous: `[1, 2]` is one
list pattern, never two alternatives. To express alternatives inside a list, use a separate arm.

> **⚠ Not implemented**: Identifier binding in patterns (`n => n * 2`).

---

## 8. Loops

### Infinite Loop

```zymbol
i = 0
@ {
    i++
    ? i >= 5 { @! }    // break fires before printing → 5 never prints
    >> i " "
}
>> ¶    // → 1 2 3 4
```

### Times Loop — repeat exactly N times

When the loop specifier is a positive integer literal, the body executes **exactly N times**. The condition is evaluated once and never re-evaluated:

```zymbol
@ 5 { >> "Zz" }
>> ¶    // → ZzZzZzZzZz

@ 100 { >> "*" }
>> ¶    // → (100 asterisks)
```

The counter is implicit — no iterator variable is exposed. Use `@!` to break early if needed:

```zymbol
@ 10 {
    >> "tick " ¶
}
// prints "tick " exactly 10 times
```

> **Note**: The analyzer emits `loop condition should be Bool, got Int` because the grammar shares the `expr` production with While. This warning is expected and harmless — the runtime correctly identifies the form as a TIMES loop.

### While Loop

```zymbol
n = 1
@ n <= 100 {
    n *= 2
}
>> n ¶    // → 128
```

### For-each over Array

```zymbol
fruits = ["apple", "pear", "grape"]
@ fruit:fruits {
    >> "  - " fruit ¶
}
```

### The Iterator Variable and Outer Variables

The iterator of `@ var:iterable` lives in the loop's scope and disappears when
the loop ends — **unless a variable with the same name already exists outside**.
In that case the loop reuses (and overwrites) the outer variable, which then
survives the loop:

```zymbol
@ i:1..3 { >> i ¶ }
// >> i ¶            // ❌ semantic error: 'i' does not exist here

i = 99
@ i:1..3 { >> i ¶ }
>> i ¶               // the outer i was overwritten by the loop
```

> The leftover value is the **last executed** iteration value in both engines
> (v0.0.8 — previously the VM left the first out-of-range value, REFERENCE
> L24). Writes to the iterator variable inside the body do not alter the
> iteration — the loop advances an internal counter. Still, prefer reading the
> value you need inside the loop, or use a different name for the iterator.

### Range Loop (inclusive on both ends)

```zymbol
// 0..N iterates from 0 to N inclusive
@ i:0..4 { >> i " " }
>> ¶    // → 0 1 2 3 4

@ i:1..5 { >> i " " }
>> ¶    // → 1 2 3 4 5
```

### Range with Step

```zymbol
@ i:1..9:2 { >> i " " }
>> ¶    // → 1 3 5 7 9

@ i:0..10:3 { >> i " " }
>> ¶    // → 0 3 6 9
```

### Reverse Range with Step

```zymbol
@ i:10..1:3 { >> i " " }
>> ¶    // → 10 7 4 1

@ i:5..0:1 { >> i " " }
>> ¶    // → 5 4 3 2 1 0
```

### For-each over String (char by char)

```zymbol
@ c:"hello" { >> c "-" }
>> ¶    // → h-e-l-l-o-
```

### Break and Continue

```zymbol
@ i:1..10 {
    ? i % 2 == 0 { @> }    // @> continue
    ? i > 7 { @! }          // @! break
    >> i " "
}
>> ¶    // → 1 3 5 7
```

### Labeled Loops

Labels use the `@:name` prefix — the colon is required. Break out with `@:name!`, continue with `@:name>`.

```zymbol
// Labeled infinite loop
count = 0
@:outer {
    count++
    ? count >= 3 { @:outer! }
}
>> count ¶    // → 3

// Labeled for-each — @:outer> skips the rest of the outer body
@:outer i:1..4 {
    @ j:1..4 {
        ? j == 2 { @:outer> }
        >> "{i}{j} "
    }
}
>> ¶    // → 11 21 31 41

// Multiple nested labels
@:a i:1..3 {
    @:b j:1..3 {
        ? j == 2 { @:b> }        // continue @:b
        @:c k:1..3 {
            ? i == 2 && k == 2 { @:a! }  // break @:a
            >> "{i}{j}{k} "
        }
    }
}
>> ¶    // → 111 112 113 131 132 133 211

// Without explicit labels (nested break via flag)
found = #0
@ i:0..4 {
    @ j:0..4 {
        ? i + j == 6 {
            found = #1
            @!
        }
    }
    ? found { @! }
}
>> found ¶    // → #1
```

| Syntax | Meaning |
|--------|---------|
| `@:name { }` | Labeled loop declaration |
| `@:name!` | Break out of loop `name` |
| `@:name>` | Continue (next iteration of) loop `name` |
| `@!` | Break innermost loop |
| `@>` | Continue innermost loop |

---

## 9. Functions

### Declaration

```zymbol
// Simple function with return
add(a, b) { <~ a + b }

// Multiple statements
factorial(n) {
    ? n <= 1 { <~ 1 }
    <~ n * factorial(n - 1)
}

>> add(3, 4) ¶         // → 7
>> factorial(5) ¶      // → 120
```

### Output Parameters `<~`

Output params are passed by reference — the function can modify them:

```zymbol
// Output param only (modifies caller's variable)
increment(counter<~) {
    counter = counter + 1
}

x = 0
increment(x)
>> x ¶    // → 1

// Output param + return value (simultaneous)
get_and_increment(val<~) {
    val = val + 1
    <~ val
}

n = 5
result = get_and_increment(n)
>> "result=" result " n=" n ¶    // → result=6 n=6

// Multiple output params
swap(a<~, b<~) {
    tmp = a
    a = b
    b = tmp
}

x = 10
y = 20
swap(x, y)
>> "x=" x " y=" y ¶    // → x=20 y=10
```

### Function Scope

Functions called **directly by name** have isolated scope — only their parameters are in scope:

```zymbol
global = 100

test() {
    // 'global' is not accessible here when called directly
    x = 42        // local
    <~ x
}

>> test() ¶    // → 42
```

> **Exception — constants pierce the isolation.** Top-level `:=` constants are
> globally scoped by design: they are readable (never writable) inside any
> function, at any call depth. See "Constant Scope" in section 4.
>
> ```zymbol
> PI := 3.14
> area(r) { <~ r * r * PI }   // ✓ PI is visible; r * r * PI works
> >> area(2) ¶                 // → 12.56
> ```

Functions used **as first-class values** capture the scope at the point of assignment (like lambdas):

```zymbol
base = 10
adder(n) { <~ n + base }   // 'base' is out of scope in direct call

f = adder          // captures current scope: { base: 10 }
>> f(5) ¶          // → 15

// Changing base after assignment does NOT affect f (capture is by value)
base = 99
>> f(5) ¶          // → 15  (captured base=10 is unchanged)
```

> See section 10 for lambdas, which always capture scope at definition time.

### Where Functions Can Be Called

All patterns below are verified in both tree-walker and VM:

```zymbol
classify(n) {
    ? n % 15 == 0 { <~ "FizzBuzz" }
    _? n % 3  == 0 { <~ "Fizz" }
    _? n % 5  == 0 { <~ "Buzz" }
    _ { <~ n }
}
double(x) { <~ x * 2 }
is_big(x) { <~ x > 10 }

// Direct assignment
r = classify(9)              // = "Fizz"

// In output — any position
>> classify(15) ¶            // → FizzBuzz
>> "res=" classify(6) ¶      // → res=Fizz
>> classify(3) " and " classify(5) ¶   // → Fizz and Buzz

// As a condition
? is_big(20) { >> "big" ¶ }    // → big

// As match subject
label = ?? classify(6) {
    "Fizz" => "mult of 3"
    "Buzz" => "mult of 5"
    _      => "other"
}

// Nested (composition)
r = double(double(3))        // = 12

// Arithmetic with function calls
r = double(4) + double(3)    // = 14

// Inside loop body
sum = 0
@ i:1..5 { sum = sum + double(i) }
>> sum ¶    // → 30

// Factory (function returning lambda)
make_adder(n) { <~ x -> x + n }
add5 = make_adder(5)
>> add5(10) ¶    // → 15

// Inside HOF — named functions accepted directly
nums = [1, 2, 3, 4, 5, 6]
r = nums$> double                    // ✅ direct reference
r = nums$| is_big                    // ✅ direct reference
r = nums$> (x -> double(x))         // ✅ wrapper also valid
```

### Anti-patterns

```zymbol
// Postfix operators in >> require parentheses
>> arr$# ¶               // ❌ "DollarHash unexpected"
>> (arr$#) ¶             // ✅
n = arr$#                // ✅ intermediate variable
```

### Named Function vs Lambda — When to Use Each

| Need | Use |
|------|-----|
| Reusable logic | Named function `fn(params) { }` |
| Recursion | Named function (lambdas cannot self-reference) |
| Capture outer scope at definition | Lambda `x -> expr` |
| Capture scope at point of use | Named function assigned to variable |
| Pass as argument (first-class) | Named function directly OR lambda |
| Return from another function | Named function OR lambda |
| HOF operand | Named function directly: `arr$> double` |

---

## 10. Lambdas and Closures

### Basic Lambda

```zymbol
double = x -> x * 2
add = (a, b) -> a + b
square = x -> x * x

>> double(5) ¶    // → 10
>> add(3, 7) ¶    // → 10
```

### Block Lambda (explicit return)

```zymbol
describe = x -> {
    ? x > 0 { <~ "positive" }
    _? x < 0 { <~ "negative" }
    <~ "zero"
}

>> describe(5) ¶     // → positive
>> describe(-3) ¶    // → negative
>> describe(0) ¶     // → zero
```

### Closures — Capturing Outer Scope

Lambdas capture variables from the scope where they are created:

```zymbol
multiplier = 3
triple = x -> x * multiplier   // captures 'multiplier'

>> triple(7) ¶    // → 21

// Closure factory
make_adder(n) { <~ x -> x + n }

add10 = make_adder(10)
add20 = make_adder(20)
>> "add10(5)=" add10(5) ¶    // → add10(5)=15
>> "add20(5)=" add20(5) ¶    // → add20(5)=25
```

### Lambdas as First-Class Values

```zymbol
// Store in variable
fn_ref = x -> x * x

// Store in array
ops = [x -> x+1, x -> x*2, x -> x*x]
>> ops[1](5) ¶    // → 6
>> ops[2](5) ¶    // → 10
>> ops[3](5) ¶    // → 25

// Pass as argument
apply(f, x) { <~ f(x) }
>> apply(x -> x * 3, 7) ¶    // → 21
```

---

## 10b. Evaluation Order and Capture Semantics

### Argument Evaluation Order

Function and lambda arguments are always evaluated **left-to-right**:

```zymbol
log = ""
tag = (s -> { log = "{log}{s}"  <~ s })

concat(a, b) { <~ "{a}{b}" }
result = concat(tag("A"), tag("B"))
>> result ¶    // → AB  (A evaluated first, then B)
```

This applies to all call forms: named functions, lambda calls, method calls, and collection operators.

### Lambda Capture: By Value at Creation

When a lambda is created, it captures a **snapshot** of each referenced outer variable. Subsequent mutations to those outer variables do not affect the captured copies:

```zymbol
a = 5
getA = (dummy -> a)    // captures a = 5
a = 99
>> getA(0) ¶           // → 5  (snapshot, not a live reference)
```

Only variables actually **referenced** inside the lambda body are captured — unreferenced outer variables are not copied.

### Loop Closures — Each Iteration Gets Its Own Snapshot

Because capture is by value at creation time, lambdas created in different loop iterations capture the loop variable's value at that moment — not a shared mutable reference:

```zymbol
fns = []
@ i:1..3 {
    f = (x -> x + i)    // captures the current value of i
    fns = fns$+ f
}
f1 = fns[1]
f2 = fns[2]
f3 = fns[3]
>> f1(10) ¶    // → 11  (captured i = 1)
>> f2(10) ¶    // → 12  (captured i = 2)
>> f3(10) ¶    // → 13  (captured i = 3)
```

This contrasts with Python's late-binding default loops, where all closures would share the final value of `i`.

### Writes to Captured Variables Stay Local

Assigning to a captured variable inside a lambda modifies the lambda's **local copy** only — it does not write back to the outer scope:

```zymbol
counter = 0
bump = (dummy -> { counter = counter + 1  <~ counter })
>> bump(0) ¶    // → 1  (local copy goes from 0 to 1)
>> counter ¶    // → 0  (outer counter unchanged)
```

To share mutable state across calls, use a named function with a module-level variable or pass the value as an output parameter (`<~`).

### Named Functions vs Lambdas

Named functions (`name(params) { }`) called **directly by name** execute in a **fully isolated scope** — they cannot read or write outer variables. Their only inputs are their parameters:

```zymbol
x = 42
peek() { <~ x }    // runtime error: undefined variable: 'x'
```

> ⚠ **Asymmetric capture**: a named function's behavior depends on how it is used, not only on how it is defined.
>
> | Usage | Scope | Outer variables |
> |-------|-------|-----------------|
> | `fn(args)` — direct call | isolated | not accessible |
> | `f = fn` then `f(args)` — as first-class value | captures at assignment | snapshot, read-only |
>
> ```zymbol
> base = 10
> adder(n) { <~ n + base }
>
> adder(5)       // runtime error: undefined variable: 'base'
>
> f = adder      // captures current scope: { base: 10 }
> >> f(5) ¶      // → 15
>
> base = 99
> >> f(5) ¶      // → 15  (snapshot — change to base does not affect f)
> ```
>
> This means `adder(5)` and `(f = adder)(5)` are **not equivalent** when the function body references outer names. If you need a function that always has access to outer state regardless of how it is called, use a lambda.

Use lambdas when you need to close over outer state; use named functions when you want strict isolation on direct calls.

---

## 11. Arrays

### Creation and Access

```zymbol
arr = [10, 20, 30, 40, 50]
>> arr ¶           // → [10, 20, 30, 40, 50]
>> arr[1] ¶        // → 10 (1-indexed: first element)
>> arr[3] ¶        // → 30
```

> **Index rules**: Zymbol uses **1-based indexing**. `arr[1]` is the first element,
> `arr[2]` the second, etc. **Index 0 is a runtime error** (`runtime error: index 0 is invalid`).
>
> **Negative indices**: `arr[-1]` returns the last element, `arr[-2]` the second-to-last, etc.
> Negative indices are symmetric mirrors of positive ones: `arr[1]` and `arr[-1]` are the
> first and last elements respectively.

### Why 1-based Indexing

Zymbol uses 1-based indexing by deliberate design choice, not as an oversight.

**Mathematical alignment.** Sequences in mathematics, linear algebra, and statistics are conventionally 1-indexed. A vector `v` has components `v₁, v₂, …, vₙ`. Zymbol follows that convention so that translating formulas to code requires no mental offset adjustment.

**Human readability.** "The first element" maps directly to index `1`. There is no conceptual gap between the ordinal position a person names and the index they write.

**Symmetry of positive and negative indices.** The positive and negative index spaces are symmetric mirrors:

```
arr = [A, B, C, D, E]
       1  2  3  4  5    (positive)
      -5 -4 -3 -2 -1    (negative)
```

`arr[1]` and `arr[-5]` both refer to `A`; `arr[5]` and `arr[-1]` both refer to `E`. This holds for any length: `arr[arr$#]` and `arr[-1]` are always the last element.

In 0-based systems, negative indices require a separate offset calculation. Here the symmetry is exact.

**Natural loop patterns.** Iterating over an array reads without adjustment:

```zymbol
arr = [10, 20, 30]
@ i:1..arr$# {
    >> arr[i] ¶    // i=1 → 10, i=2 → 20, i=3 → 30
}
```

In 0-based systems, the same loop would require `0..(arr$#-1)` or similar.

**Index 0 is always an error.** There is no "zero-th element". Accessing `arr[0]` raises `##Index` immediately, which makes accidental off-by-one bugs explicit rather than silently returning a wrong value.

### Length

```zymbol
arr = [10, 20, 30, 40, 50]
len = arr$#
>> len ¶        // → 5
>> (arr$#) ¶    // → 5  (parentheses required in >>)
```

### Append, Insert, Remove, Contains, Slice

```zymbol
arr = [1, 2, 3, 4, 5]

// $+ — append, returns new collection
arr = arr$+ 6
>> arr ¶    // → [1, 2, 3, 4, 5, 6]

// $+[i] — insert at position (1-based)
arr2 = arr$+[2] 99
>> arr2 ¶    // → [1, 99, 2, 3, 4, 5, 6]

// $- val — remove first occurrence by value
arr3 = arr$- 3
>> arr3 ¶    // → [1, 2, 4, 5, 6]

// $-- val — remove all occurrences by value
arr4 = [1, 2, 3, 2, 4]$-- 2
>> arr4 ¶    // → [1, 3, 4]

// $-[i] — remove at index (1-based)
arr5 = arr$-[1]
>> arr5 ¶    // → [2, 3, 4, 5, 6]

// $-[start..end] — remove range, 1-based inclusive start, inclusive end
arr6 = arr$-[2..3]
>> arr6 ¶    // → [1, 4, 5, 6]

// $-[start:count] — remove range, count-based (alternative syntax)
arr6b = arr$-[2:2]
>> arr6b ¶    // → [1, 4, 5, 6]  (identical result to $-[2..3])

// $? — contains
has = arr$? 3
>> has ¶    // → #1

// $?? — find all indices (returns 1-based positions)
pos = [1, 2, 1, 3, 1]$?? 1
>> pos ¶    // → [1, 3, 5]

// $[..] — slice, 1-based inclusive start, inclusive end
sl = arr$[1..3]
>> sl ¶    // → [1, 2, 3]

// $[start:count] — slice count-based (alternative syntax)
sl2 = arr$[1:3]
>> sl2 ¶    // → [1, 2, 3]  (identical result)
```

### Negative Indices and Symmetric Slices

Negative indices count from the end. `arr[-1]` is the last element, symmetric to `arr[1]`
(the first). This makes end-relative access natural without knowing the length in advance.

```zymbol
arr = [10, 20, 30, 40, 50]

>> arr[1] ¶        // → 10 — first element
>> arr[-1] ¶       // → 50 — last element  (mirror of arr[1])
>> arr[-2] ¶       // → 40 — second-to-last
```

> Accessing `arr[0]` is a **runtime error**: `index 0 is invalid — Zymbol uses 1-based indexing`.

Combining a positive start with a negative end gives **symmetric slices** `arr$[k..-k]`:

```zymbol
arr = [10, 20, 30, 40, 50]

>> arr$[1..-1] ¶   // → [10, 20, 30, 40, 50] — full array
>> arr$[2..-2] ¶   // → [20, 30, 40]          — strip first and last
>> arr$[3..-3] ¶   // → [30]                  — center element only
```

The pattern `$[k..-k]` naturally expresses "drop k elements from each end". When the window
collapses to nothing (e.g. `$[4..-4]` on a 5-element array), the result is an empty array.

> **Note**: All collection operators return a new collection. Assign back to the
> same variable: `arr = arr$+ 4`. `$+` can be chained directly:
> ```zymbol
> arr = arr$+ 5$+ 6$+ 7    // ✅ chains left-to-right → [1,2,3,5,6,7]
> ```

### Sort

`$^+` sorts ascending and `$^-` sorts descending. Both return a **new array**; the
original is unchanged. The `^` prefix means "order"; `+` and `-` indicate direction.

```zymbol
arr = [3, 1, 4, 1, 5, 9, 2, 6]

// Natural ascending order
asc = arr$^+
>> asc ¶    // → [1, 1, 2, 3, 4, 5, 6, 9]

// Natural descending order
desc = arr$^-
>> desc ¶   // → [9, 6, 5, 4, 3, 2, 1, 1]
```

Works on strings too — lexicographic order:

```zymbol
words = ["banana", "apple", "cherry", "date"]
>> words$^+ ¶    // → [apple, banana, cherry, date]
>> words$^- ¶    // → [date, cherry, banana, apple]
```

**Custom comparator** — use `$^` (no `+`/`-`) with a two-argument lambda that returns
`#1` if the first element should come before the second. The direction is encoded
entirely in the comparator (`<` for ascending, `>` for descending). Required for
sorting named or positional tuple arrays by field:

```zymbol
db = [
    (name: "Carla", age: 28),
    (name: "Ana",   age: 25),
    (name: "Bob",   age: 30)
]

// Sort by age ascending (< means ascending)
by_age = db$^ (a, b -> a.age < b.age)
>> by_age[1].name ¶    // → Ana

// Sort by name descending (> means descending)
by_name_desc = db$^ (a, b -> a.name > b.name)
>> by_name_desc[1].name ¶    // → Carla
```

> **Note**: `$^+` and `$^-` are for **primitive arrays** (numbers, strings) without a
> custom comparator. For named or positional tuple arrays, use `$^` with a lambda.
> `$^` with a lambda on a primitive array is also valid when you need custom ordering.

### Direct Element Update

Arrays are mutable. Elements can be replaced or updated in-place using index syntax:

```zymbol
arr = [10, 20, 30, 40, 50]

// Direct assignment (1-based index)
arr[2] = 99
>> arr ¶    // → [10, 99, 30, 40, 50]

// Compound indexed assignment (+=, -=, *=, /=, %=, ^=)
arr[1] += 5
>> arr ¶    // → [15, 99, 30, 40, 50]

arr[3] *= 2
>> arr ¶    // → [15, 99, 60, 40, 50]

// Functional form — returns a new array; original is unchanged
arr2 = arr[2]$~ 0
>> arr ¶    // → [15, 99, 60, 40, 50]  (unchanged)
>> arr2 ¶   // → [15, 0, 60, 40, 50]

// Deep functional update — nav path [i>j>…] selects a nested element
m  = [[1, 2], [3, 4]]
m2 = m[1>2]$~ 99
>> m ¶      // → [[1, 2], [3, 4]]   (unchanged)
>> m2 ¶     // → [[1, 99], [3, 4]]
```

> The deep form works in both engines (compiled to the `DeepSet` instruction in the
> VM). Ranges (`..`) are not supported in a `$~` path — only scalar steps.

> **Value semantics**: assigning an array to a new variable creates an independent
> copy. Modifying one does not affect the other:
> ```zymbol
> a = [1, 2, 3]
> b = a
> a[1] = 99
> >> a ¶    // → [99, 2, 3]
> >> b ¶    // → [1, 2, 3]   ← b is unaffected
> ```

### Iterating

```zymbol
nums = [10, 20, 30]
@ n:nums {
    >> n " "
}
>> ¶    // → 10 20 30
```

### Nested Arrays (Matrices)

```zymbol
matrix = [[1,2,3], [4,5,6], [7,8,9]]
>> matrix[2] ¶       // → [4, 5, 6]
>> matrix[2][3] ¶    // → 6
```

> **⚠ Arrays must be homogeneous** — all elements must be the same type.
> See [Known Limitations](#20-known-limitations-and-workarounds) for workarounds.

---

## 11b. Destructuring Assignment

Unpack arrays or tuples into individual variables in a single statement.

### Array Destructuring

```zymbol
arr = [10, 20, 30, 40, 50]

// Basic — bind by position
[a, b, c] = arr          // a=10  b=20  c=30

// Rest collector — *name captures remaining elements
[first, *rest] = arr     // first=10  rest=[20, 30, 40, 50]

// Discard with _
[x, _, z] = [1, 2, 3]   // x=1  z=3
```

### Positional Tuple Destructuring

```zymbol
point = (100, 200)
(px, py) = point         // px=100  py=200

triple = (1, 2, 3)
(h, *tail) = triple      // h=1  tail=[2, 3]
```

### Named Tuple Destructuring

```zymbol
person = (name: "Ana", age: 25, city: "Madrid")

// Bind each field to a local variable
(name: n, age: a) = person    // n="Ana"  a=25

// Rename fields freely
(name: who, city: where) = person   // who="Ana"  where="Madrid"
```

### Semantics on Existing Variables

Destructuring **overwrites** any variable that already exists in the current scope — it does not shadow and does not produce an error:

```zymbol
a = 99
[a, b] = [10, 20]
>> a ¶    // 10  — a was overwritten
>> b ¶    // 20  — b was created
```

Positions discarded with `_` leave all other existing variables unchanged:

```zymbol
a = 99
b = 88
[a, _, c] = [10, 20, 30]
>> a ¶    // 10  — overwritten
>> b ¶    // 88  — untouched (not in the pattern)
>> c ¶    // 30  — created
```

Inside a function, destructuring operates on the function's isolated local scope — it does not affect outer variables with the same name:

```zymbol
x = 999
f() {
    [x, y] = [1, 2]
    >> x ¶    // 1  — local x
}
f()
>> x ¶        // 999  — outer x unchanged
```

> **Constants are protected**: destructuring into a name declared with `:=` is a
> semantic error (`cannot reassign constant`), the same as direct reassignment.
> Use a different name in the pattern.

All patterns are matched positionally (arrays, positional tuples) or by field name (named tuples).

---

## 11c. Multi-dimensional Indexing

Zymbol provides a coherent, symbol-first system for navigating nested arrays. Inside a
postfix `[...]`, the `>` character is always a **depth separator**, not a comparison operator.

### Overview

| Syntax | Returns | Description |
|---|---|---|
| `arr[i]` | value | 1-D access (unchanged, 1-based) |
| `arr[i>j]` | value | Scalar deep access — row i, col j |
| `arr[i>j>k]` | value | Depth 3+ — any nesting level |
| `arr[(expr)>j]` | value | Computed index — expression in `()` |
| `arr[-1>-1]` | value | Negative indices — last row, last col |
| `arr[[i>j]]` | `[value]` | Flat extraction — single path wrapped |
| `arr[p ; q ; r]` | `[v, v, v]` | Flat extraction — multiple paths |
| `arr[[g] ; [g]]` | `[[…], […]]` | Structured extraction — array of arrays |
| `arr[[p,q] ; [r,s]]` | `[[…], […]]` | Multiple values per group |
| `arr[i>r1..r2]` | `[v, …]` | Range on last step — expand along final axis |
| `arr[r1..r2>j]` | `[v, …]` | Range on intermediate step — fan-out |

All forms are fully supported by **both** the tree-walker and the register VM (`--vm`).

> **Design note**: using `>` as depth separator inside `[...]` is intentional. Context resolves any ambiguity: `arr[a>b]` (no spaces, plain identifiers) is always navigation; `arr[(a > b)]` is a parenthesized comparison. Alternatives evaluated (`:`, `>>`, `,`) conflicted with other grammar rules or added more visual noise. The current syntax is the most readable form achievable within the keyword-free constraint.

---

### Scalar Deep Access

`>` navigates one level deeper per separator. All indices are 1-based.

```zymbol
m = [[1,2,3], [4,5,6], [7,8,9]]

>> m[2>3] ¶        // → 6    (row 2, col 3)
>> m[1>1] ¶        // → 1    (row 1, col 1)
>> m[-1>-1] ¶      // → 9    (last row, last col)

// Depth 3
cubo = [[[1,2],[3,4]], [[5,6],[7,8]]]
>> cubo[1>2>1] ¶   // → 3
>> cubo[2>2>2] ¶   // → 8
```

### Computed Indices

Plain identifiers work directly as nav atoms. Expressions with operators require `(expr)`:

```zymbol
m = [[1,2,3,4], [5,6,7,8], [9,10,11,12], [13,14,15,16]]
n = 4
mitad = 2

>> m[n>n] ¶             // → 16  (plain variables, no parens needed)
>> m[(mitad)>(n)] ¶     // → 8   (explicit grouping — equivalent)
>> m[(mitad+1)>n] ¶     // → 12  (expression requires parens)
>> m[3>(mitad*2)] ¶     // → 12  (arithmetic in atom)
```

> **Rule**: `arr[a>b]` where `a` and `b` are identifiers is **navigation** (their values
> are used as depth indices). `arr[(a>b)]` is a 1-D index where `(a>b)` evaluates to Bool
> — which causes a runtime type error, as expected.

### Flat Extraction

Returns a **flat array** of values collected from multiple paths.

```zymbol
m = [[1,2,3], [4,5,6], [7,8,9]]

// Single path wrapped → [value]
>> m[[2>3]] ¶                    // → [6]

// Multiple paths → [v1, v2, v3]
>> m[1>1 ; 2>3 ; 3>2] ¶         // → [1, 6, 8]

// Assign and use
diag = m[1>1 ; 2>2 ; 3>3]
>> diag ¶                        // → [1, 5, 9]
>> (diag$#) ¶                    // → 3
```

### Structured Extraction

Returns an **array of arrays**. Each group `[...]` becomes one sub-array.

```zymbol
m = [[1,2,3], [4,5,6], [7,8,9]]

// Each single path → [[v1], [v2], [v3]]
>> m[[1>1] ; [2>3] ; [3>2]] ¶         // → [[1], [6], [8]]

// Multiple values per group → [[v1, v2], [v3, v4]]
>> m[[1>1, 1>3] ; [3>1, 3>3]] ¶       // → [[1, 3], [7, 9]]

// Corners of the matrix
corners = m[[1>1, 1>3] ; [3>1, 3>3]]
>> corners[1] ¶                        // → [1, 3]
>> corners[2] ¶                        // → [7, 9]
```

### Ranges (`..`) on Navigation Steps

The `..` range can appear on **any** step. Its position determines which dimension expands.

#### Range on the last step — expands columns

```zymbol
m = [[1,2,3], [4,5,6], [7,8,9]]

// Row 1, cols 2 to 3
>> m[[1>2..3]] ¶                  // → [2, 3]

// Two groups with col ranges → sub-matrices
>> m[[1>2..3] ; [2>2..3]] ¶       // → [[2, 3], [5, 6]]

// Reconstruct full matrix
>> m[[1>1..3] ; [2>1..3] ; [3>1..3]] ¶   // → [[1, 2, 3], [4, 5, 6], [7, 8, 9]]
```

#### Range on an intermediate step — fan-out

The range expands that dimension; remaining steps apply to each element in the range.

```zymbol
m = [[1,2,3], [4,5,6], [7,8,9]]

// Rows 1-2 at col 2; rows 2-3 at col 3
>> m[[1..2>2] ; [2..3>3]] ¶    // → [[2, 5], [6, 9]]

// Layer 1, rows 1..3, col 2 (3D cube example)
cubo = [
    [[1,2,3], [4,5,6], [7,8,9]],
    [[10,11,12], [13,14,15], [16,17,18]]
]
>> cubo[1>1..3>2] ¶               // → [2, 5, 8]
```

#### Ranges with variable bounds

Range ends can be any nav atom — literal, identifier, or `(expr)`:

```zymbol
m = [[1,2,3,4], [5,6,7,8], [9,10,11,12], [13,14,15,16]]
inicio = 2
fin = 4
mitad = 2

>> m[1>inicio..fin] ¶             // → [2, 3, 4]
>> m[[1>1..(mitad)] ; [(mitad+1)>1..(mitad)]] ¶   // → [[1, 2], [9, 10]]
```

### Nested Ranges (Double Fan-out)

A single path can carry ranges on multiple steps. Each range emits an inner loop:

```zymbol
cubo = [
    [[1,2,3], [4,5,6], [7,8,9]],
    [[10,11,12], [13,14,15], [16,17,18]]
]

// Layers 1-2, rows 1-2 — four rows total (flat)
>> cubo[1..2>1..2] ¶
// → [[1, 2, 3], [4, 5, 6], [10, 11, 12], [13, 14, 15]]
```

### Deprecated: Chained `arr[i][j]`

The old C/Python-style chained index `arr[i][j]` still parses, but `arr[i>j]` is the
canonical form. A semantic warning may be added in a future version.

```zymbol
m = [[1,2,3], [4,5,6], [7,8,9]]
>> m[2][3] ¶    // → 6  (still works, deprecated)
>> m[2>3] ¶     // → 6  (canonical form)
```

### Error Cases

```zymbol
m = [[1,2], [3,4]]

// Index 0 is always invalid in nav paths
!? { >> m[1>0] ¶ } :! { >> "caught: index 0 is invalid" ¶ }

// Out of bounds
!? { >> m[5>1] ¶ } :! { >> "caught: out of bounds" ¶ }
```

---

## 12. Tuples

Tuples are **immutable** ordered containers. Once created, their elements cannot be
modified. They can hold values of different types (unlike arrays, which are homogeneous).
Use tuples to represent fixed records; use arrays for dynamic, same-type collections.

### Positional Tuple

```zymbol
point = (10, 20)
>> point[1] ¶    // → 10
>> point[2] ¶    // → 20

// Tuples allow mixed types
data = (42, "hello", #1, 3.14)
>> data[3] ¶    // → #1
```

### Named Tuple

```zymbol
person = (name: "Alice", age: 25, active: #1)

// Access by field name (recommended)
>> person.name ¶    // → Alice
>> person.age ¶     // → 25

// Access by positional index (1-based)
>> person[1] ¶      // → Alice
>> person[2] ¶      // → 25

// Nested named tuples
pos = (x: 10, y: 20)
p = (pos: pos, label: "origin")
>> p.label ¶        // → origin
>> p.pos.x ¶        // → 10
```

### Immutability

Tuples cannot be modified after creation. Any attempt to assign to an element
produces a runtime error:

```zymbol
t = (10, 20, 30)
t[1] = 99    // ❌ runtime error: cannot modify tuple 't': tuples are immutable
t[1] += 5    // ❌ same error
```

To derive a new tuple with one element changed, use the functional update operator `$~`.
The original tuple is never touched:

```zymbol
t = (10, 20, 30)
t2 = t[2]$~ 999
>> t ¶     // → (10, 20, 30)   ← original unchanged
>> t2 ¶    // → (10, 999, 30)  ← new tuple
```

Named tuples support `$~` too (v0.0.6), addressed by 1-based position **or by
field-name string** (useful when the field is chosen at runtime):

```zymbol
person = (name: "Alice", age: 25)
older  = person["age"]$~ 26       // by field name
upper  = person[1]$~ "ALICE"      // by position (1-based; negative allowed)
>> person.age ¶    // → 25   ← original unchanged
>> older.age ¶     // → 26
>> upper.name ¶    // → ALICE
```

Rebuilding explicitly remains valid when several fields change at once:

```zymbol
person = (name: "Alice", age: 25)
other  = (name: person.name, age: 26)
```

> **Constants vs immutability**: `:=` makes the *variable binding* constant (the name
> cannot be rebound at all). Tuples make the *value* immutable (elements cannot change).
> Both mechanisms are independent and complementary.

---

## 13. Strings

### Basic Operations

```zymbol
s = "Hello World"

// Length
n = s$#
>> n ¶    // → 11

// Contains (char or substring)
>> (s$? 'W') ¶         // → #1
>> (s$? "World") ¶     // → #1

// Slice — 1-based inclusive on both ends
sub = s$[1..5]
>> sub ¶    // → Hello

// Slice count-based (alternative syntax)
sub2 = s$[1:5]
>> sub2 ¶    // → Hello  (identical result)

// Split by char or substring — $/ operator
parts = "a,b,c,d" $/ ','
>> parts ¶    // → [a, b, c, d]

parts2 = "one::two::three" $/ "::"
>> parts2 ¶   // → [one, two, three]
```

### Advanced String Operators

```zymbol
s = "hello world"

// $+ — append char or string
s2 = s$+ "!"
>> s2 ¶    // → hello world!

// $+[i] — insert before char position i (1-based)
ins = s$+[6] "!!!"
>> ins ¶    // → hello!!! world

// $- val — remove first occurrence of char or substring
rem1 = s$- 'l'
>> rem1 ¶    // → helo world

// $-- val — remove all occurrences
rem2 = s$-- 'l'
>> rem2 ¶    // → heo word

// $-[i] — remove char at index (1-based)
rem3 = s$-[1]
>> rem3 ¶    // → ello world

// $-[start..end] — remove char range, 1-based inclusive start, inclusive end
rem4 = s$-[1..5]
>> rem4 ¶    // → world

// $-[start:count] — remove char range, count-based (alternative syntax)
rem4b = s$-[1:5]
>> rem4b ¶    // → world  (identical result)

// $?? — find all positions of a pattern (returns 1-based positions)
pos = s$?? "o"
>> pos ¶    // → [5, 8]  (1-based char positions)

// $~~[pattern:replacement] — replace all occurrences
rep = s$~~["l":"L"]
>> rep ¶    // → heLLo worLd

// $~~[pattern:replacement:N] — replace only first N occurrences
rep1 = s$~~["l":"L":1]
>> rep1 ¶   // → heLlo world

// $/ — split by char or substring
parts = "a,b,c,d" $/ ','
>> parts ¶    // → [a, b, c, d]

parts2 = "one::two::three" $/ "::"
>> parts2 ¶   // → [one, two, three]
```

### Build Strings with `$++`

`$++` builds a string (or array) by appending items to a base. All items must
be on the same line. Non-string values are converted to their string representation:

```zymbol
n = 42
pi = 3.14
flag = #1

// String base — append any number of values
s = "n=" $++ n " pi=" pi " ok=" flag
>> s ¶    // → n=42 pi=3.14 ok=#1

// Equivalent to interpolation, but useful when values are computed
// Note: (expr) closes the juxtaposition chain — use an intermediate variable
label = "result"
tmp = 100 * 2
out = label $++ "=" tmp
>> out ¶    // → result=200

// Array base — append elements
arr = [1, 2, 3] $++ 4 5 6
>> arr ¶    // → [1, 2, 3, 4, 5, 6]
```

### Concatenation — Two Correct Forms

```zymbol
name = "Alice"
n = 42

// 1. Juxtaposition in >> (canonical)
>> "Hello " name " you have " n " items" ¶

// 2. String interpolation (most readable)
desc = "Hello {name}, you have {n} items"
>> desc ¶
```

Juxtaposition also works **inside** call arguments, array elements, tuple
elements and grouped expressions — so a composed string can be handed straight
to a function without an intermediate variable. A comma always separates;
juxtaposition never swallows one:

```zymbol
name = "Alice"
n = 42

label(s) { <~ "[" s "]" }
pair(a, b) { <~ a "/" b }

// call arguments — no intermediate variable needed
>> label("v" n) ¶                // → [v42]
>> label(name " v" n) ¶          // → [Alice v42]
>> label("<" label("v" n) ">") ¶ // → [<[v42]>]

// the comma still separates: two arguments, not one
>> pair("a" 1, "b" 2) ¶          // → a1/b2

// array elements, grouped expressions and tuple elements
lista = [name " one", "two " n]
>> lista[1] ¶                    // → Alice one
grupo = (name " v" n)
>> grupo ¶                       // → Alice v42
```

One difference from statement level: a following `(` never continues the chain
inside these positions, because there a parenthesis is ambiguous with a lambda,
a tuple and a grouped expression. Bind it to a variable first, or reach for
interpolation.

### Iterating Characters

```zymbol
@ c:"hello" { >> c "-" }
>> ¶    // → h-e-l-l-o-
```

### String Repeat — `$*`

`"string" $* N` repeats a string N times and returns the result. N must be a non-negative Int.

```zymbol
cols   = 40
line   = "=" $* 20
sep    = "-" $* 10
border = "|" $* (cols - 2)

>> line ¶    // → ====================
>> sep ¶     // → ----------

// N = 0 → empty string (nothing printed before ¶)
>> ("x" $* 0) ¶

// Useful for padding and TUI borders
titulo = "Score"
>> "[" titulo "]" ¶              // → [Score]
>> "=" $* (titulo$# + 2) ¶      // → =======
```

---

## 14. Higher-Order Functions

HOF operators accept **inline lambdas** or **named function references** directly.

```zymbol
nums = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]

// $> — map
doubled = nums$> (x -> x * 2)
>> doubled ¶    // → [2, 4, 6, 8, 10, 12, 14, 16, 18, 20]

// $| — filter
evens = nums$| (x -> x % 2 == 0)
>> evens ¶    // → [2, 4, 6, 8, 10]

// $< — reduce: (initial, (acc, x) -> expr)
sum = nums$< (0, (acc, x) -> acc + x)
>> sum ¶    // → 55

// Chaining via intermediate variables (direct chaining is not supported)
step1 = nums$| (x -> x > 3)
step2 = step1$> (x -> x * x)
>> step2 ¶    // → [16, 25, 36, 49, 64, 81, 100]
```

### Named Functions as First-Class HOF Arguments

Named functions are first-class values and can be passed directly to HOF operators:

```zymbol
double(x) { <~ x * 2 }
is_big(x) { <~ x > 5 }

nums = [1, 2, 3, 4, 5, 6, 7, 8]

// Pass named function directly — no wrapper lambda needed
r = nums$> double
>> r ¶    // → [2, 4, 6, 8, 10, 12, 14, 16]

filtered = nums$| is_big
>> filtered ¶    // → [6, 7, 8]

// Assign to variable and reuse
f = double
>> f(5) ¶        // → 10
>> [1,2,3]$> f ¶  // → [2, 4, 6]  (via intermediate variable)
```

When a named function is used as a value, it captures the current scope (like a lambda).
The captured scope is fixed at the point of assignment, not at the point of call.

### Reduce with Block Lambda

```zymbol
data = [3, 1, 4, 1, 5, 9, 2, 6]
maximum = data$< (data[1], (max, x) -> {
    ? x > max { <~ x }
    <~ max
})
>> maximum ¶    // → 9
```

---

## 15. Pipe Operator

Pipes a value into a function. When the function takes the piped value as its **only** argument, `_` is optional — `x |> f` is equivalent to `x |> f(_)`:

```zymbol
double = x -> x * 2
add = (a, b) -> a + b
inc = x -> x + 1

// Implicit first-position: x |> f  ≡  f(x)
r1 = 5 |> double
>> r1 ¶    // → 10

r2 = 5 |> (x -> x * 3)
>> r2 ¶    // → 15

// Explicit placeholder required when pipe value is NOT in first position
r3 = 10 |> add(_, 5)
>> r3 ¶    // → 15

r4 = 5 |> add(2, _)
>> r4 ¶    // → 7

// Chained pipe — implicit and explicit can be mixed
r5 = 5 |> double |> inc |> double
>> r5 ¶    // → 22  (5→10→11→22)

r5b = 5 |> double(_) |> inc(_) |> double(_)
>> r5b ¶   // → 22  (equivalent)

// Pipe with closure
factor = 3
r6 = 7 |> (x -> x * factor)
>> r6 ¶    // → 21
```

---

## 16. Error Handling

### Try / Catch / Finally

```zymbol
!? {
    x = 10 / 0
    >> "never reaches here" ¶
} :! ##Div {
    >> "division by zero caught" ¶
} :! ##IO {
    >> "IO error" ¶
} :! {
    >> "other error: " _err ¶    // _err holds the error message
} :> {
    >> "always runs (finally)" ¶
}
```

### Error Types for `:! ##Type`

| Type | When |
|------|------|
| `##IO` | File / network operations |
| `##Div` | Division by zero |
| `##Index` | Index out of bounds |
| `##Type` | Type mismatch |
| `##Parse` | Data parsing failure |
| `##Network` | Network errors |
| `##DB` | Database errors (`std/db`) |
| `##_` | Generic catch-all |

```zymbol
// Typed catch example
!? {
    arr = [1, 2, 3]
    v = arr[10]
} :! ##Index {
    >> "index out of bounds" ¶
} :! {
    >> "other: " _err ¶
}
// → index out of bounds
```

### `$!` — Check if Value is an Error

```zymbol
x = 42
is_err = x$!
>> is_err ¶    // → #0 (not an error)
```

### `$!!` — Propagate Error to Caller

If the value is an error, `$!!` returns it **early** to the caller (the rest of the
body never runs); if it is not an error, execution continues. Works identically in
named functions and lambdas, in both engines:

```zymbol
process(value) {
    ? value < 0 {
        value$!!    // propagates error up to caller
    }
    <~ value * 2
}

// Same semantics inside a lambda:
handler = (x -> { x$!! <~ "ok" })   // error in → error out; otherwise "ok"
```

### Nested Try Blocks

```zymbol
!? {
    !? {
        x = 10 / 0
    } :! ##Div {
        >> "inner: div zero" ¶
    }
    >> "continues after inner try" ¶
} :! {
    >> "outer error" ¶
}
// → inner: div zero
// → continues after inner try
```

### Exception Flow vs Value Flow

Zymbol has two distinct error-handling mechanisms. Choose based on how the error should travel.

#### Exception flow — `!?` / `:!` / `:>`

Errors propagate **as exceptions** through the call stack. `!?` intercepts them at a boundary.

```zymbol
safe_get(arr, idx) {
    !? {
        <~ arr[idx]    // throws ##Index if out of bounds
    } :! {
        <~ -1          // convert exception to sentinel value
    }
}

>> safe_get([10, 20], 2) ¶    // → 20
>> safe_get([10, 20], 99) ¶   // → -1
```

**Use when**: catching errors at a boundary, performing cleanup (`:>`), or returning a sentinel on failure.

#### Value flow — `$!` / `$!!`

Errors travel as **ordinary return values**. The caller receives them and decides what to do.

```zymbol
risky(arr, idx) {
    !? {
        <~ arr[idx]
    } :! {
        <~ _err        // return the error as a value (not an exception)
    }
}

process(arr, idx) {
    result = risky(arr, idx)
    ? result$! { result$!! }   // early-return the error to our own caller
    <~ result * 10
}

r = process([5, 10, 15], 2)
>> r$! ¶    // → #0 (not an error)
>> r ¶      // → 100

r2 = process([5, 10, 15], 99)
>> r2$! ¶   // → #1 (is an error)
>> r2 ¶     // → ##Index(array index out of bounds: index 99 for array of length 3)
```

`$!!` is an **early return** — it causes the function to return the error value to its caller. It does **not** throw an exception, so it cannot be caught with `!?/:!`.

**Use when**: chaining multiple operations where any step can fail, or building pipelines that defer error handling to the top level.

#### Decision guide

| Situation | Use |
|-----------|-----|
| Intercept a runtime error at a boundary | `!? / :!` |
| Always run cleanup regardless of outcome | `!? / :>` |
| Return a safe default on failure | `!? / :! { <~ default }` |
| Pass an error up through a call chain | `<~ _err` then `$!! ` |
| Check if a return value is an error | `val$!` |
| Re-propagate an error value early | `val$!!` |

---

## 17. Modules

### Module File Structure

A module file contains exactly one closed block: `# name { ... }`. Everything inside the braces is the module body. Nothing is allowed before `#` or after the closing `}`.

```zymbol
// file: lib/utils.zy
# utils {
    <# ./dep => d          // imports (must precede re-exports that reference the alias)

    #> {                   // export block
        add
        PI                 // constant — accessible as alias.PI
        get_count          // getter for private mutable state
        set_count
    }

    PI    := 3.14159       // exported constant — immutable
    count = 0              // private mutable state — persists across calls

    add(a, b) { <~ a + b }

    get_count() { <~ count }
    set_count(n) { count = n }

    private_fn(x) { <~ x * 2 }    // not in #> — inaccessible from outside
}
```

**Recommended ordering inside the block**: `<#` imports → `#>` export block → constants/variables → function definitions. The parser accepts any ordering, but `<#` aliases used in `#>` re-exports must appear before the `#>` block.

### Allowed and Forbidden Inside a Module Body

| Element | Allowed | Notes |
|---------|---------|-------|
| `<# path => alias` | ✓ | Import |
| `#> { ... }` | ✓ | Export block |
| `NAME := literal` | ✓ | Exported constant (literal RHS only) |
| `var = literal` | ✓ | Private mutable state (literal RHS only) |
| `fn(params) { }` | ✓ | Function definition |
| `>> expr` | ✗ | **E013** — output not allowed in module body |
| `<< var` | ✗ | **E013** — input not allowed in module body |
| `fn_call()` standalone | ✗ | **E013** — call not allowed at module top-level |
| `x = fn_call()` | ✗ | **E013** — non-literal initializer |
| `? / @ / ?? / !?` | ✗ | **E013** — control flow not allowed in module body |
| `! "shell"` | ✗ | **E013** — shell exec not allowed in module body |
| `<~ expr` | ✗ | **E013** — return not allowed outside function |

**E013** is raised whenever an executable statement appears at the module top-level. Function bodies are unrestricted — the limitation only applies to the module block itself.

> Since v0.0.8, importing a module also runs the full **semantic analysis** on
> it (both engines): reassigning a module constant or violating scope rules
> inside a module function fails at import time with a semantic error. Module
> constants are additionally protected at runtime — reassignment from a module
> function is a runtime error even if static analysis was bypassed.

### Visibility Model

| Declaration | Exported in `#>` | External access | Persists across calls |
|-------------|------------------|-----------------|-----------------------|
| `PI := 3.14` | yes | `alias.PI` (read-only) | yes (immutable) |
| `count = 0` | no (excluded even if listed) | ✗ error | **yes — write-back** |
| `fn()` | yes | `alias::fn()` | — |
| `private_fn()` | no | ✗ error | — |

**Module state identity is per file path**: importing the same module file
several times — even under different aliases, even from different importers
(diamond dependencies) — shares **one** state in both engines. Two aliases to
`./counter` increment the same counter.

Since v0.0.8, mutations made by **intra-module calls** persist too: an exported
function may delegate state changes to a private helper, and the calling frame
observes the helper's mutation immediately after the call.

**Private mutable state** (`=` variables) persists between calls and is only reachable through exported getter/setter functions:

```zymbol
// counter.zy
# counter {
    #> { increment, get_value }

    count = 0

    increment() { count = count + 1 }
    get_value() { <~ count }
}
```

```zymbol
// main.zy
<# ./counter => c

c::increment()         // count → 1
c::increment()         // count → 2
n = c::get_value()     // n = 2
>> n ¶

x = c.count            // ✗ Runtime error: Module 'c' has no constant 'count'
```

### Importing and Using

```zymbol
// Import with alias (alias is required)
<# ./lib/utils => u

// Call exported function
result = u::add(5, 3)
>> result ¶    // → 8

// Access exported constant
pi = u.PI
>> pi ¶        // → 3.14159
```

### Import Paths

```zymbol
<# ./module => m         // same directory
<# ../shared/lib => s    // parent directory
<# ./sub/folder => c     // subdirectory
```

### Export Aliases

```zymbol
// Export with a different public name
#> {
    internal_fn => public_name
    INTERNAL_CONST => PUBLIC_CONST
}
```

### Re-export from Another Module

Use `::` to re-export a function imported from another module, and `.` to re-export a constant. Place the `<#` import before `#>` so the alias is in scope. The new public name follows `=>`:

```zymbol
// math.zy
# math {
    <# ./core => c

    #> {
        c::add           // re-export function as-is (callers use m::add)
        c::add => sum    // re-export function with different public name
        c.PI             // re-export constant
        c.PI => TAU      // re-export constant with different name
    }
}
```

> **Note**: Re-export of constants via `.` works in both engines. The old L3 limitation
> (`alias.CONST` failing analysis) was fixed — see REFERENCE.md.

### Subdirectory Module Convention

```zymbol
# .subfolder_file {    // dot prefix for modules inside subfolders
    #> { ... }
    // ...
}
```

### Standard Library Modules (`std/*`)

Zymbol ships native modules written in Rust, consumed through the **same** module system —
import a `std/<name>` path with an alias, then call `alias::func(...)`. No filesystem lookup,
no new syntax. They work in both engines (tree-walker and `--vm`).

```zymbol
<# std/math => m
<# std/json => j

>> m::sqrt(2.0) ¶              // → 1.4142135623730951
>> m.PI ¶                      // → 3.141592653589793 (exported constant)
datos = j::decode("[1,2,3]")  // JSON text → Array
>> datos ¶                    // → [1, 2, 3]
```

Because native functions live in the same table as user functions, they re-export through
the i18n pattern with no special handling:

```zymbol
# json_es {
    <# std/json => _j
    #> {
        _j::decode => decodificar
        _j::encode => codificar
    }
}
```

| Module | Functions | Since |
|--------|-----------|-------|
| `std/math` | `sqrt` `exp` `ln` `log` `pow` `abs` `ceil` `floor` `round` `min` `max` `sin` `cos` `tan` `asin` `acos` `atan` `atan2` `sinh` `cosh` `tanh` `sigmoid` · constants `PI`, `E` | v0.0.6 |
| `std/random` | `entero` `rango` `peso_f64` | v0.0.6 |
| `std/json` | `decode(text)` `decode_map(text, map)` `encode(value)` | v0.0.7 |
| `std/io` | `read` `write` `append` `exists` `delete` `list` `mkdir` | v0.0.7 |
| `std/net` | `get` `post` `post_json` `head` | v0.0.7 |
| `std/db` | `connect` `disconnect` `exec` `query` `query_one` `query_value` `tx` `begin` `commit` `rollback` `savepoint` `release` `rollback_to` `exec_script` `table_exists` | v0.0.7 |
| `std/term` | `width` `pad_left` `pad_right` `center` `truncate` | v0.0.8 |

**`std/term` — display width in terminal columns.** `width` counts **columns**, not
graphemes: CJK ideographs, kana, hangul and most emoji take two columns each, so a
framed panel drifts if you lay it out with `$#`. `width` accepts a String or a single
`Char`; `pad_left`/`pad_right`/`center` pad with spaces to an exact column count (an
already-wide string is returned untouched, and `center` gives a spare column to the
right); `truncate` cuts to at most N columns without splitting a wide glyph.

```zymbol
<# std/term => t
>> t::width("手番") ¶                  // → 4  (two columns each), while "手番"$# is 2
>> "[" t::pad_right("go", 6) "]" ¶     // → [go    ]
>> "[" t::center("go", 6) "]" ¶        // → [  go  ]
>> "[" t::truncate("形勢判断", 4) "]" ¶ // → [形勢]  (never half a glyph)
```

This is a **screen** metric. Operating on a string's *content* — split, slice, replace,
repeat — stays in the language's symbols (`$/`, `$[..]`, `$~~`, `$*`); `std/term` never
duplicates them.

**Error convention.** Type/arity mistakes raise a hard `RuntimeError` (the program is
malformed). Recoverable environmental failures — file not found, network timeout, malformed
JSON, SQL errors — come back as a **soft `Error` value** (`##IO(...)`, `##Network(...)`,
`##Parse(...)`, `##DB(...)`) that you test with `$!` or catch with `!?`, rather than aborting:

```zymbol
<# std/io => io
txt = io::read("no-existe.txt")
? txt$! {
    >> "no se pudo leer" ¶          // soft error captured, no crash
} _ {
    >> txt ¶
}
```

`std/net` is synchronous (no async). `get`/`post`/`post_json` accept an optional trailing
`headers` argument — an array of 2-element `(String, String)` tuples — to reach authenticated
APIs. JSON object ↔ `NamedTuple` (key order preserved), JSON array ↔ `Array`, null ↔ `Unit`.

> When writing JSON **literals** in source, escape `{` as `\{` (an unescaped `{` starts string
> interpolation). JSON read from a file or the network needs no escaping.

**Data-level i18n with `decode_map`.** The re-export pattern translates *function names*, but
the **keys** of decoded JSON come from the external API and stay in its language
(`数据.candidates[1].content.parts[1].text`). `decode_map(text, map)` decodes **and** renames
object keys recursively, at any depth, so the resulting structure reads in the consumer's
language. The map is a `NamedTuple` whose field names are the source keys and whose String
values are the new names; keys absent from the map are kept verbatim, and an empty `()` map
makes `decode_map` behave like `decode`.

```zymbol
<# std/json => json

datos = json::decode_map(respuesta,
    (candidates: "候选", content: "内容", parts: "片段", text: "文本"))
>> datos.候选[1].内容.片段[1].文本 ¶   // no English API key leaks into the logic
```

#### `std/db` — vendor-neutral database access (ODBC)

Zymbol bundles **no database engine**: `std/db` speaks **ODBC**, and the OS supplies the
per-engine driver (SQLite, PostgreSQL, MySQL, MS SQL Server, …). The API is identical
across engines — only the connection string changes. SQLite and PostgreSQL are validated
end-to-end in v0.0.7.

> **Availability.** `std/db` requires linking against the system's ODBC driver manager,
> which the prebuilt **Linux and macOS** binaries cannot do (the driver manager loads
> engine drivers with `dlopen`, impossible in a fully static binary) — on those builds
> `<# std/db` reports *module not found*. It **is** included in the Windows binaries
> (ODBC ships with the OS) and in any source build (`cargo build --release`, default
> `db` feature; needs `unixodbc-dev` at build time and `unixodbc` + a driver at runtime).

```zymbol
<# std/db => db

db::connect("c", "Driver={SQLite3};Database=/tmp/demo.db;")
db::exec("c", "CREATE TABLE socios(cod INTEGER PRIMARY KEY, nombre TEXT)")
db::exec("c", "INSERT INTO socios(cod, nombre) VALUES(?, ?)", (1, "O'Brien & Co."))

fila = db::query_one("c", "SELECT cod, nombre FROM socios WHERE cod = ?", (1,))
>> fila.nombre ¶                                        // → O'Brien & Co.
>> db::query_value("c", "SELECT COUNT(*) FROM socios") ¶  // → 1
db::disconnect("c")
```

- **Connection registry**: `connect(name, conn_string)` registers a named connection;
  every other function takes that name as its first argument.
- **Parameter binding**: `exec`/`query`/`query_one`/`query_value` take an optional trailing
  positional-tuple of parameters bound to `?` placeholders — quotes in data are safe by
  construction (no SQL injection by string concatenation).
- **Rows are `NamedTuple`s** keyed by column name; `query` returns an array of rows,
  `query_one` a single row (or soft error), `query_value` a single scalar.
- **Transactions**: `tx(name, batch)` runs an array of `(sql, params)` tuples atomically;
  low-level `begin`/`commit`/`rollback` plus nested `savepoint`/`release`/`rollback_to`.
- **Utilities**: `exec_script` (multi-statement SQL), `table_exists`.
- SQL failures return a **soft `##DB(...)` error** (testable with `$!`, catchable with
  `!? … :! ##DB`); wrong argument types abort hard, like every stdlib module.

### Distributing a Multi-File Program (`.zyp`)

A project with more than one script and shared modules (imports, `</ file.zy />` targets)
can be packaged into a single portable `.zyp` archive — a ZIP of *source*, not a compiled
binary. `zymbol build` is the separate, unrelated feature that produces a native
executable; a `.zyp` still needs a `zymbol` binary to run it.

```bash
zymbol package DIR --script main.zy -o out.zyp   # write the archive
zymbol package DIR --script main.zy --dry-run    # list the closure + warnings, write nothing
zymbol run out.zyp                                # extract to a temp dir and run
zymbol run out.zyp --script 囲碁 --tw             # pick an entry point and an engine
```

#### The manifest (`zyp.toml`)

`DIR` needs a `zyp.toml` declaring one or more `[[script]]` entry points. Without one,
`--script` synthesizes a manifest for the run and prints it so it can be saved for next
time:

```toml
[package]
name = "go"
version = "1.2.0"
engine = ">=0.0.8"   # semver REQUIREMENT — see the warning below
mode = "vm"          # default engine for this package's scripts

[[script]]
name = "go"
path = "go.zy"
default = true
desc = "English"

[[script]]
name = "囲碁"
path = "囲碁.zy"
desc = "日本語"
```

> **Always write `engine = ">=0.0.8"`, never a bare `"0.0.8"`.** A bare version is a *caret*
> requirement, and pre-1.0 a caret matches only that exact version — `"0.0.8"` would refuse
> to run on 0.0.9. `zymbol package` always synthesizes the `>=` form.

`zymbol run` picks the script named by `--script`; failing that, the one marked
`default = true`; failing that, the only entry if there is exactly one.

#### What gets packaged

Packaging is **strict about what it includes** and **permissive about what it can't
resolve**. Starting from the declared scripts, the closure follows module imports and
`</ file.zy />` targets. A `.zy` file that is neither listed nor reachable is never
packaged — an unused file left in the directory stays out, and `--dry-run` says so (W008).

Anything that cannot be resolved statically becomes a **warning, not a failure**, so
`--dry-run` always produces something inspectable:

| Code | Meaning |
|------|---------|
| `W001` | Absolute or `~`-relative import — not reproducible on another machine |
| `W002` | An import resolves to a file that doesn't exist |
| `W003` | `<\ shell \>` present — its arguments are arbitrary expressions, so any `.zy` it runs can't be traced |
| `W004` | A module with `</ />` was reached from more than one entry, whose base directories differ |
| `W005` | A `</ />` target doesn't exist on disk |
| `W006` | The file has lex/parse errors — packaged anyway, but its own dependencies weren't traced |
| `W007` | A dependency reached via `../` lives above the first entry's directory |
| `W008` | `.zy` files in the entry's directory that nothing reaches — not packaged |
| `W009` | `std/db` imported — never packaged (the stdlib is synthetic), but it needs an ODBC driver at run time |
| `W010` | The archive exceeds the recommended size ceiling |
| `W011` | The same file was reached through two different lexical paths |

The one **hard error** is a `[[script]]` that turns out to be a module file: a package whose
entry point can't run isn't permissive, it's broken.

#### Running a package

`zymbol run pkg.zyp` extracts to an ephemeral temp directory and runs from there — it
**never `chdir`s**. This split is deliberate:

- **Code** is read from the temp dir and disappears when the process exits.
- **Data the script writes** does not. A `std/io` write to a relative path lands in your
  real working directory, because it resolves against the process's actual cwd.

Use `--keep-temp` to retain the extraction directory and print its path when debugging.

A `.zyp` defaults to the **register VM**; loose `.zy` files still default to the
tree-walker, so nothing changes for ordinary scripts. Precedence is
`--tw` > `--vm` > manifest `mode` > VM.

#### In the browser

The web playground loads a `.zyp` directly: one tab per source file, named by full relative
path (e.g. `核/盤.zy`), plus a script picker populated from the manifest. The archive
carries a `zyp.json` alongside `zyp.toml` — the same manifest, pre-serialized — so the
browser never has to parse TOML.

---

## 18. Data Operators

### Numeric Eval `#|expr|` — Parse String to Number

Converts a string to its numeric value. Accepts ASCII digits and **any of the 69
Unicode digit scripts** supported by the lexer (Thai, Devanagari, Arabic-Indic,
Klingon pIqaD, etc.). Fail-safe: returns the original string unchanged if conversion
fails, without raising an error.

```zymbol
v1 = #|"42"|
>> v1 ¶    // → 42  (Int)

v2 = #|"3.14"|
>> v2 ¶    // → 3.14  (Float)

v3 = #|"abc"|
>> v3 ¶    // → abc  (original string — fail-safe, no error)

v4 = #|99|
>> v4 ¶    // → 99  (pass-through if already a number)

// Unicode digit strings — same result as ASCII equivalents
v5 = #|"๔๒"|
>> v5 ¶    // → 42  (Thai digits U+0E54, U+0E52)

v6 = #|"४२"|
>> v6 ¶    // → 42  (Devanagari digits U+0967, U+0966)

v7 = #|"٣.١٤"|
>> v7 ¶    // → 3.14  (Arabic-Indic float)
```

> **Note**: `#|"๔๒"| == #|"42"|` — both evaluate to the integer `42`.
> The conversion uses the same normalization as the lexer, so every script
> that the lexer recognizes as integer literals also works inside `#|…|`.

### Type Metadata `expr#?`

Returns tuple `(type_symbol, count, value)` where `count` meaning depends on type:

| Type | `count` meaning |
|------|----------------|
| Int, Float | number of characters in the string representation |
| String | character length |
| Char, Bool | always `1` |
| Array, Tuple, NamedTuple | number of elements / fields |
| Function | arity (number of parameters) |
| Error | length of the error message |
| Unit | `0` |

```zymbol
ti = 42#?
>> ti ¶    // → (###, 2, 42)

tf = 3.14#?
>> tf ¶    // → (##., 4, 3.14)

ts = "hello"#?
>> ts ¶    // → (##", 5, hello)

tc = 'A'#?
>> tc ¶    // → (##', 1, A)

// Functions and lambdas — count is arity
double(x) { <~ x * 2 }
f = double
>> f#? ¶              // → (##(), 1, <funct/1>)

lam = (a, b) -> a + b
>> lam#? ¶            // → (##->, 2, <lambd/2>)

// Extract just the type (intermediate variable required)
meta = 42#?
t = meta[1]
>> t ¶    // → ###
```

**Display format**: named functions show as `<funct/N>`, anonymous lambdas as `<lambd/N>`, where `N` is the arity.

### Precision: Rounding and Truncation

```zymbol
pi = 3.14159265

r2 = #.2|pi|
>> r2 ¶    // → 3.14  (round to 2 decimal places)

r4 = #.4|pi|
>> r4 ¶    // → 3.1416

t2 = #!2|pi|
>> t2 ¶    // → 3.14  (truncate, not round)

// Also works on numeric strings
rstr = #.2|"19.876"|
>> rstr ¶    // → 19.88

// Rounding to 0 decimals — result is Float but displayed without .0
r0 = #.0|19.9|
>> r0 ¶    // → 20
t0 = #!0|19.9|
>> t0 ¶    // → 19
```

### Type Conversion Casts

Three prefix operators convert between Int and Float:

| Operator | Name | Behaviour |
|----------|------|-----------|
| `##.expr` | ToFloat | Converts Int or Float to Float |
| `###expr` | ToIntRound | Converts Float to Int, rounding (half away from zero) |
| `##!expr` | ToIntTrunc | Converts Float to Int truncating toward zero; a `Char` to its code point |

> **Convention**: `##.` mirrors `#.N` (round/decimal), `##!` mirrors `#!N` (truncate).
> `###` is a dedicated rounding cast with no decimal-precision argument.

`##!` also accepts a `Char`, giving its Unicode code point — the only direct Char→Int
route, and the way to classify a character by range (`Char` is otherwise neither
comparable nor castable):

```zymbol
>> ##!'A' ¶      // → 65
>> ##!'あ' ¶     // → 12354
c = 'M'
p = ##!c
? p >= 65 && p <= 90 { >> "upper" ¶ }   // → upper
```

```zymbol
i = 42
f = 3.7

// Int → Float
fi = ##.i
>> fi ¶    // → 42  (Float type — displayed without .0 when integer-valued)

// Float → Int (round — 3.7 rounds to 4)
ir = ###f
>> ir ¶    // → 4

// Float → Int (truncate — 3.7 truncates to 3)
it = ##!f
>> it ¶    // → 3

// Negative values
nf = -2.9
>> ###nf ¶    // → -3  (rounds away from zero)
>> ##!nf ¶    // → -2  (truncates toward zero)

// Works on any expression
>> ###(7 / 2.0) ¶    // → 4  (3.5 rounds to 4)
>> ##!(7 / 2.0) ¶    // → 3  (3.5 truncates to 3)
```

### Number Formatting

```zymbol
// Comma-separated format for large numbers
nfmt = 1234567
fmt = #,|nfmt|
>> fmt ¶    // → 1,234,567

// With inline precision: round (.N) or truncate (!N)
pi = 3141592.653
>> #,.2|pi| ¶    // → 3,141,592.65  (round to 2 decimal places)
>> #,!2|pi| ¶    // → 3,141,592.65  (truncate to 2 decimal places)

// Scientific notation
xsci = 12345.678
sci = #^|xsci|
>> sci ¶    // → 1.2345678e4

// With inline precision: round (.N) or truncate (!N)
>> #^.3|xsci| ¶    // → 1.235e4  (round to 3 significant digits)
>> #^!3|xsci| ¶    // → 1.234e4  (truncate to 3 significant digits)
```

### Base Literals and Conversions

```zymbol
// Literals in different bases (result: Char if ASCII range, Int otherwise)
a = 0x41        // hexadecimal → 'A'
b = 0b01000001  // binary → 'A'
c = 0o101       // octal → 'A'
d = 0d65        // explicit decimal → 'A'

>> a ¶    // → A
>> b ¶    // → A

// Convert expression to base string
hex = 0x|255|    // Int → hex string → "0x00FF"
bin = 0b|65|     // Int → binary string → "0b1000001"
oct = 0o|8|      // Int → octal string → "0o10"
dec = 0d|255|    // Int → decimal string → "0d0255"
```

---

## 18b. Numeral Modes

Zymbol can display numbers in any of **69 Unicode digit scripts** — Devanagari,
Arabic-Indic, Thai, Klingon pIqaD, Mathematical Bold, LCD segments, and more.
Numeral mode only affects how a value is turned into **displayed text** — `>>`,
`>>~`, string interpolation, juxtaposition and `$++` all format Int/Float/Bool
through the active script; internal arithmetic always uses binary integers and
IEEE-754 floats regardless of the active script.

### Mode-Switch Token `#d0d9#`

Write the digit `0` and digit `9` of the target script, enclosed in `#…#`:

```zymbol
#०९#    // activate Devanagari  (U+0966–U+096F)
#٠٩#    // activate Arabic-Indic (U+0660–U+0669)
#๐๙#    // activate Thai         (U+0E50–U+0E59)
#09#    // restore ASCII (always safe — never display-affected)
```

The token is **purely a runtime directive** — it emits no output and leaves no
variable. One mode-switch persists until the next one in the same file.

### Output Under an Active Mode

Once a mode is active, `>>` formats all numeric values through it:

```zymbol
n = 42
>> n ¶          // → 42  (ASCII, default)

#०९#
>> n ¶          // → ४२  (Devanagari)
>> 3.14 ¶       // → ३.१४
>> 1 + 2 ¶      // → ३

#09#
>> n ¶          // → 42  (back to ASCII)
```

### Every String-Building Path, Not Just `>>`

A number rarely stays on its own — it gets folded into a label, a HUD readout,
a chat line. Interpolation, juxtaposition (`"a" b`) and `$++` all reach for the
same numeral-aware conversion `>>` uses, so a number baked into a composed
string still comes out in the active script:

```zymbol
#०९#
n = 42

y = "{n}"          // interpolation
>> y ¶             // → ४२

z = "n=" n         // juxtaposition (BinaryOp::Concat)
>> z ¶             // → n=४२

w = "n=" $++ n     // $++
>> w ¶             // → n=४२

>>~ (1, 1) > n     // positioned output

>> [1, 2, 3] ¶     // → [१, २, ३]  (elements, not brackets)
>> (7, 8) ¶        // → (७, ८)
```

The mode reaches *inside* collections: a number does not stop being a number by
sitting in a list. Brackets, parentheses, commas, the `-` sign and the decimal
`.` stay ASCII — only digits change.

Only `Value::to_display_string()` itself — the bare, context-free conversion
with no access to which mode is active — has no numeral awareness. Every
runtime call site that turns a value into text goes through the active mode
instead of that bare conversion, so there is no place left where a number
silently reverts to ASCII while the mode is on.

### Intent and Responsibility

`#d0d9#` is a statement about how *this program* writes numbers, and Zymbol
takes it literally: the mode applies to text the program later uses as data, not
only to text a human reads.

```zymbol
#०९#
n = 42

io::write("dato{n}.txt", "…")   // creates dato४२.txt — not dato42.txt
r = <\ "echo {n}" \>            // runs: echo ४२
b = ("{n}" == "42")             // #० — different strings, same number
```

This is intent, not a leak: the language does not second-guess which of your
strings are labels and which are file names. What the mode never touches is a
serialization format with its own grammar — `json::encode` always emits ASCII
digits, so encoded data stays parseable by everything else.

Two practical consequences, both the developer's to manage:

- If a value must stay ASCII, build it while the mode is off (or hand ASCII back
  with `#09#` first). Juxtaposing a raw `Int` into a `<\ … \>` command
  (`<\ "echo " n \>`) also keeps ASCII: the shell path converts values itself.
- Reading the digits back always works — see *Reading Numerals Back* below.

### Reading Numerals Back

Output and input are symmetric: every numeric cast accepts digits from any of
the 69 supported scripts, so a program can re-read what it just printed and a
user can type what they were just shown.

```zymbol
#०९#
n = 120
s = "{n}"          // ← "१२०"

>> #|s| ¶          // → १२०  (parsed as the Int 120, rendered in the mode)
>> #.0|s| ¶        // → १२०  (round: same normalization)
<<### edad         // accepts ४२ and 42 alike
```

The `-` sign and the decimal `.` are read as ASCII, matching how they are
written. A string that is not a number at all is still rejected — `#.1|"abc"|`
raises `cannot convert string 'abc' to number for rounding` in both engines.

### Boolean Output

Booleans always print with an ASCII `#` prefix followed by the **active digit**
for `0` (false) or `1` (true). This guarantees `#0` (false) is always visually
distinct from `0` (integer zero) in every script:

```zymbol
>> #1 ¶         // → #1   (ASCII default)
>> #0 ¶         // → #0

#०९#
>> #1 ¶         // → #१   (Devanagari — # stays ASCII)
>> #0 ¶         // → #०

x = 28 > 4
>> x ¶          // → #१   (comparison result follows active mode)
```

See [§18b — Booleans Across Numeral Scripts](#booleans-across-numeral-scripts)
for the complete reference including native literals, conditions, match, and all
supported scripts.

### Native Digit Literals in Source Code

Digit characters from any supported block are valid **numeric literals** in
source code — in loop ranges, modulo operands, comparisons, and assignments:

```zymbol
#०९#

// All of these are valid integer literals:
n = ४२         // same as n = 42
@ i:१..१५ {   // range 1..15 in Devanagari digits
    ? i % १५ == ० { >> "FizzBuzz" ¶ }
    _? i % ३  == ० { >> "Fizz" ¶ }
    _? i % ५  == ० { >> "Buzz" ¶ }
    _ { >> i ¶ }
}
```

Native digit literals and ASCII digit literals are interchangeable — the
lexer normalises both to the same internal integer value.

### Booleans Across Numeral Scripts

#### Writing boolean literals

`#` followed by the digit `0` or `1` of **any** supported script lexes as a
boolean literal identical to ASCII `#0` / `#1`. The `#` prefix is always an
ASCII `#` — only the digit after it varies:

| Script | False | True | Mode token |
| ------ | ----- | ---- | ---------- |
| ASCII (default) | `#0` | `#1` | `#09#` |
| Devanagari | `#०` | `#१` | `#०९#` |
| Arabic-Indic | `#٠` | `#١` | `#٠٩#` |
| Ext. Arabic-Indic | `#۰` | `#۱` | `#۰۹#` |
| Bengali | `#০` | `#১` | `#০৯#` |
| Gurmukhi | `#੦` | `#੧` | `#੦੯#` |
| Gujarati | `#૦` | `#૧` | `#૦૯#` |
| Tamil | `#௦` | `#௧` | `#௦௯#` |
| Telugu | `#౦` | `#౧` | `#౦౯#` |
| Kannada | `#೦` | `#೧` | `#೦೯#` |
| Thai | `#๐` | `#๑` | `#๐๙#` |
| Myanmar | `#၀` | `#၁` | `#၀၉#` |
| Math Bold | `#𝟎` | `#𝟏` | `#𝟎𝟗#` |
| Klingon pIqaD | `#`+U+F8F0 | `#`+U+F8F1 | `#`+U+F8F0+U+F8F9+`#` |

#### Boolean literals in conditions and expressions

Native-script boolean literals can be used anywhere `#0`/`#1` is valid —
conditions, logical operators, assignments, comparisons:

```zymbol
#०९#

// Condition
? #१ {
    >> "सत्य" ¶     // → सत्य  (true branch taken)
}

// Assignment
सक्रिय = #१
>> सक्रिय ¶        // → #१

// Logical operators (input and output both in active script)
>> (#१ && #०) ¶    // → #०
>> (#१ || #०) ¶    // → #१
>> !#० ¶           // → #१
```

```zymbol
#٠٩#

// Arabic-Indic example
? #١ {
    >> "صحيح" ¶    // → صحيح
}
نشط = #١
>> نشط ¶           // → #١
>> (#١ && #٠) ¶   // → #٠
```

#### Comparison results follow the active mode

All comparison operators (`==`, `<>`, `<`, `>`, `<=`, `>=`) return a Bool.
Under an active numeral mode, the result is displayed in the active script:

```zymbol
a = 28
b = 4

// ASCII (default)
>> (a > b) ¶     // → #1
>> (a < b) ¶     // → #0
>> (a == b) ¶    // → #0

#๐๙#
>> (a > b) ¶     // → #๑   (true  in Thai)
>> (a < b) ¶     // → #๐   (false in Thai)

#০৯#   // activate Bengali digits
// the comparison value itself is still Bool — only display changes
বড় = a > b
>> বড় ¶       // → #১   (Bengali true)
```

#### Match on booleans in any script

Boolean values can be matched with `??` using any script's `#0`/`#1`:

```zymbol
#०९#

x = ५ > ३     // Bool — evaluates to true (#१)

?? x {
    #१ => { >> "हाँ" ¶ }     // → हाँ
    #०  => { >> "नहीं" ¶ }
}
```

#### Key invariant: `#` prefix always ASCII

No matter which numeral mode is active, the `#` separator is always the
ASCII `#` (U+0023). This means:

- `#0` and `#०` are the same boolean (false) — both lex identically
- The printed representation `#` + native-digit is never ambiguous with an
  integer: `0` (integer zero) vs `#0` (boolean false) remain visually distinct
  in every script

### Supported Digit Scripts — 69 Blocks

| Script | Range | Digits |
| ------ | ----- | ------ |
| ASCII | U+0030–U+0039 | `0123456789` |
| Arabic-Indic | U+0660–U+0669 | `٠١٢٣٤٥٦٧٨٩` |
| Ext. Arabic-Indic | U+06F0–U+06F9 | `۰۱۲۳۴۵۶۷۸۹` |
| NKo | U+07C0–U+07C9 | `߀߁߂߃߄߅߆߇߈߉` |
| Devanagari | U+0966–U+096F | `०१२३४५६७८९` |
| Bengali | U+09E6–U+09EF | `০১২৩৪৫৬৭৮৯` |
| Gurmukhi | U+0A66–U+0A6F | `੦੧੨੩੪੫੬੭੮੯` |
| Gujarati | U+0AE6–U+0AEF | `૦૧૨૩૪૫૬૭૮૯` |
| Oriya | U+0B66–U+0B6F | `୦୧୨୩୪୫୬୭୮୯` |
| Tamil | U+0BE6–U+0BEF | `௦௧௨௩௪௫௬௭௮௯` |
| Telugu | U+0C66–U+0C6F | `౦౧౨౩౪౫౬౭౮౯` |
| Kannada | U+0CE6–U+0CEF | `೦೧೨೩೪೫೬೭೮೯` |
| Malayalam | U+0D66–U+0D6F | `൦൧൨൩൪൫൬൭൮൯` |
| Sinhala Archaic | U+0DE6–U+0DEF | `𑇐𑇑𑇒𑇓𑇔𑇕𑇖𑇗𑇘𑇙` |
| Thai | U+0E50–U+0E59 | `๐๑๒๓๔๕๖๗๘๙` |
| Lao | U+0ED0–U+0ED9 | `໐໑໒໓໔໕໖໗໘໙` |
| Tibetan | U+0F20–U+0F29 | `༠༡༢༣༤༥༦༧༨༩` |
| Myanmar | U+1040–U+1049 | `၀၁၂၃၄၅၆၇၈၉` |
| Myanmar Shan | U+1090–U+1099 | `႐႑႒႓႔႕႖႗႘႙` |
| Khmer | U+17E0–U+17E9 | `០១២៣៤៥៦៧៨៩` |
| Mongolian | U+1810–U+1819 | `᠐᠑᠒᠓᠔᠕᠖᠗᠘᠙` |
| Mathematical Bold | U+1D7CE–U+1D7D7 | `𝟎𝟏𝟐𝟑𝟒𝟓𝟔𝟕𝟖𝟗` |
| Mathematical Double-struck | U+1D7D8–U+1D7E1 | `𝟘𝟙𝟚𝟛𝟜𝟝𝟞𝟟𝟠𝟡` |
| Mathematical Sans-serif | U+1D7E2–U+1D7EB | `𝟢𝟣𝟤𝟥𝟦𝟧𝟨𝟩𝟪𝟫` |
| Math Sans-serif Bold | U+1D7EC–U+1D7F5 | `𝟬𝟭𝟮𝟯𝟰𝟱𝟲𝟳𝟴𝟵` |
| Mathematical Monospace | U+1D7F6–U+1D7FF | `𝟶𝟷𝟸𝟹𝟺𝟻𝟼𝟽𝟾𝟿` |
| Segmented/LCD | U+1FBF0–U+1FBF9 | `🯰🯱🯲🯳🯴🯵🯶🯷🯸🯹` |
| Klingon pIqaD ¹ | U+F8F0–U+F8F9 | _(CSUR PUA — requires pIqaD font)_ |
| _(+43 additional BMP and SMP scripts)_ | | _(see `interpreter/crates/zymbol-lexer/src/digit_blocks.rs`)_ |

> ¹ Klingon pIqaD digits live in the ConScript Unicode Registry (CSUR) Private
> Use Area. They render correctly only with a pIqaD-capable font such as
> _pIqaD-qolqoS_.

### Scope and Persistence

- Mode is **interpreter-global, not file-local** — a program starts in ASCII
  mode, and a `#d0d9#` anywhere changes the mode for *everything* executed
  afterwards, including the caller that imported the module which switched it.
  Both engines agree on this.
- Mode changes take effect **immediately** at the statement that contains
  `#d0d9#` and persist until the next mode-switch — there is no implicit reset
  at a file, module, or function boundary.
- Therefore a module function that renders in a non-ASCII script **must reset
  the mode itself**, or it silently reformats every number the rest of the
  program prints:

  ```zymbol
  mI'(n) {
      #<d0><d9>#        // activate the target script
      s = "{n}"         // interpolation now renders in it
      #09#              // MANDATORY: hand ASCII back to the caller
      <~ s
  }
  ```

  Without the `#09#` line, a caller doing `>> 120 ¶` after calling `mI'`
  prints in the callee's script. This is what makes a per-locale number
  formatter possible at all — see `Hol/tlhIngan.zy` in zyKlingonGalaxy.
- The REPL respects the active mode: expression results are displayed in the
  currently active script.

### Rules Summary

| Rule | Detail |
| ---- | ------ |
| Default mode | ASCII (`0`–`9`) |
| Activation token | `#d0d9#` — zero and nine of any supported block |
| Affected output | `>>`, `>>~`, interpolation, juxtaposition, `$++` — for Int, Float, Bool, including the ones inside arrays and tuples |
| Unaffected | String content itself, Char, Array brackets, Tuple parentheses, commas, `json::encode` |
| Bool prefix | `#` always ASCII; digit adapts to active script |
| Literals | Any script's digits valid as integer literals in source |
| Numeric casts | `#\|…\|`, `#.N\|…\|`, `#!N\|…\|`, `<<###`, `<<#.` read digits from any script |
| Float decimal point | Always ASCII `.` regardless of active mode |
| Text used as data | Follows the mode too (file names, shell commands) — intended, and the developer's to validate |
| Reset to ASCII | `#09#` |

---

## 19. Shell Integration

### BashExec `<\ cmd \>`

Executes a system command and captures stdout + stderr:

```zymbol
// Capture result as string
date = <\ "date +%Y-%m-%d" \>
>> "Today: " date ¶

// Variable in command (identifier or string interpolation)
file = "data.txt"
content = <\ "cat " file \>
>> content

// String interpolation inside command string
dir = "/tmp"
listing = <\ "ls {dir}" \>
>> listing ¶

// Arithmetic via shell
result = <\ "echo 'scale=2; 355/113' | bc" \>
>> result ¶
```

> **Note**: Trailing `\n` is stripped automatically (consistent with shell `$(...)` substitution).
> Internal newlines are preserved. Add `¶` explicitly when needed.

### Execute Script `</ file.zy />`

Executes another Zymbol script and captures its output:

```zymbol
output = </ ./subscript.zy />
>> output
```

> For a list of bugs fixed in each version, see [CHANGELOG.md](CHANGELOG.md).

---

## 22. Verified Examples

### FizzBuzz

```zymbol
@ i:1..100 {
    ? i % 15 == 0 { >> "FizzBuzz" ¶ }
    _? i % 3 == 0 { >> "Fizz" ¶ }
    _? i % 5 == 0 { >> "Buzz" ¶ }
    _ { >> i ¶ }
}
```

### Fibonacci (iterative)

```zymbol
fib(n) {
    ? n <= 1 { <~ n }
    a = 0
    b = 1
    @ i:2..n {
        tmp = a + b
        a = b
        b = tmp
    }
    <~ b
}
>> fib(10) ¶    // → 55
>> fib(30) ¶    // → 832040
```

### Bubble Sort

```zymbol
bsort(arr<~) {
    n = arr$#
    @ i:1..(n-1) {
        @ j:1..(n-i) {
            ? arr[j] > arr[j+1] {
                tmp = arr[j]
                arr[j] = arr[j+1]
                arr[j+1] = tmp
            }
        }
    }
}

data = [64, 34, 25, 12, 22, 11, 90]
bsort(data)
>> data ¶    // → [11, 12, 22, 25, 34, 64, 90]
```

### Functional Pipeline

```zymbol
// Filter passing grades, compute average
scores = [45, 78, 92, 33, 88, 67, 55, 91, 42, 76]

passing = scores$| (x -> x >= 60)
total = passing$< (0, (acc, x) -> acc + x)
count = passing$#
average = total / count
n_scores = scores$#

>> "Total scores: " n_scores ¶
>> "Passing: " count ¶
>> "Average (passing): " average ¶
```

### Complete Module Example

```zymbol
// file: calc.zy
# calc {
    #> {
        add
        subtract
        multiply
        get_version
    }

    _VERSION := "1.0"

    add(a, b)      { <~ a + b }
    subtract(a, b) { <~ a - b }
    multiply(a, b) { <~ a * b }
    get_version()  { <~ _VERSION }
}
```

```zymbol
// file: main.zy
<# ./calc => c

>> c::add(10, 5) ¶          // → 15
>> c::subtract(10, 5) ¶     // → 5
>> c::multiply(3, 7) ¶      // → 21
ver = c::get_version()
>> "version: " ver ¶        // → version: 1.0
```

### Error Handling with Type Parsing

```zymbol
parse_number(s) {
    n = #|s|
    meta = n#?
    type = meta[1]
    ? type == "##\"" {
        <~ "not a number: " + s
    }
    <~ n
}

!? {
    r1 = parse_number("42")
    >> "r1=" r1 ¶
    r2 = parse_number("abc")
    >> "r2=" r2 ¶
} :! {
    >> "error: " _err ¶
} :> {
    >> "done" ¶
}
```

---

