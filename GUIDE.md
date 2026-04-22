# Zymbol-Lang — Language Guide

> **Authoritative reference** — all examples verified empirically on both execution modes:
> `zymbol run` (tree-walker) and `zymbol run --vm` (register VM).
> If a construct is not documented here, it may not be implemented.

**Interpreter version**: v0.0.4
**Test coverage**: 393/393 parity (TW ↔ VM)

See also: [REFERENCE.md](REFERENCE.md) — limitations, error taxonomy, symbol table  
See also: [IMPLEMENTATION.md](IMPLEMENTATION.md) — EBNF grammar, coverage status, TW/VM internals

---

## Table of Contents

0. [Design Philosophy](#0-design-philosophy)
1. [Running Programs](#1-running-programs)
1b. [Lexical Structure](#1b-lexical-structure)
2. [Data Types](#2-data-types)
3. [Output and Input](#3-output-and-input)
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

### Origin: Genuine Minimalism

Zymbol was born from a simple constraint: **no keywords**. Every construct — conditionals, loops, functions, I/O, error handling, modules — is expressed through symbols. It is a commitment to a different kind of readability: one where the shape of the code carries meaning independently of natural language.

The language grew organically from that constraint. As it became more complete, complexity entered — but the goal remained to contain it within a coherent symbolic grammar rather than letting keywords leak in.

### Symbolic Coherence: Shared Meaning, Similar Spirit

Zymbol does not enforce one symbol per concept. Instead, a symbol may appear in multiple contexts **when the underlying spirit is the same**. The reader learns the symbol's character once and recognizes it across uses.

**`_` — the non-binding marker**

`_` always means *"this position does not matter / is not bound"*:

| Context | Example | Meaning |
|---------|---------|---------|
| else branch | `_ { }` | default case — no condition binds |
| else-if | `_? x > 0 { }` | else-if — extends the non-binding chain |
| wildcard in match | `?? x { _ -> "other" }` | catch-all arm — value not bound |
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
| Module import | `<# ./calc <= c` | brings a module into scope |

Types and modules share `#` because both are about *what something is*, not *what value it holds*.

### Self-Referential Grammar

Zymbol's symbolic vocabulary is its own. The symbols have no external standard to conform to — their meaning is defined by the language itself and built up through consistent use. A programmer learns Zymbol by reading Zymbol, not by mapping it onto another language.

This creates an initial learning curve. It also means the language can evolve its symbol system with full internal consistency, without being constrained by conventions inherited elsewhere.

### Complexity Within Minimalism

The language is no longer small. It has arrays, tuples, modules, closures, error handling, HOFs, a pipe operator, shell integration, and Unicode numeral modes. None of this contradicts the minimalist origin — complexity entered through **depth**, not through **keywords**. Each new construct reuses and extends the existing symbolic grammar rather than introducing new vocabulary.

The measure of minimalism in Zymbol is not line count or feature count. It is: *can a new construct be expressed with existing symbols, or does it require inventing new ones?*

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

Both modes produce **identical output** on 393/393 parity tests.

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

Only simple identifiers (letters, digits, `_`) are allowed inside braces. Expressions must be assigned to a variable first.

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
| Function | named function ref | `##->` | First-class since v0.0.4; display `<function/N>` |
| Lambda | `x -> x * 2` | `##->` | Same type symbol as Function; display `<lambda/N>` |
| Error | _(runtime value)_ | `##<Kind>` | Type IS the kind: `##Index`, `##Div`, `##IO`, … |
| Unit | _(void return)_ | `##_` | Returned by functions with no `<~`; display is empty |

### Non-value Types

These constructs exist in Zymbol but are **not first-class values** — they cannot be stored in variables, inspected with `#?`, or passed as arguments.

| Construct | Usage | Why not a value |
|-----------|-------|-----------------|
| Range (`1..5`) | Loop iterator only: `@ i:1..5 { }` | Storing a range raises a runtime error |
| Module (`<# ./m <= m`) | Namespace only: `m::fn()`, `m.CONST` | Module alias is not a runtime value |

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
| Function | `(##->, N, <function/N>)` | arity |
| Lambda | `(##->, N, <lambda/N>)` | arity |
| Error | `(##Kind, N, ##Kind(msg))` | message length |
| Unit | `(##_, 0, )` | always 0 |

```zymbol
x = 42
>> x#? ¶               // → (###, 2, 42)

f(a, b) { <~ a + b }
fn_ref = f
>> fn_ref#? ¶          // → (##->, 2, <function/2>)

lam = (a, b) -> a + b
>> lam#? ¶             // → (##->, 2, <lambda/2>)

// Extract type symbol
meta = x#?
t = meta[1]
>> t ¶                 // → ###
```

Both named functions and lambdas share the type symbol `##->`. Distinguish them by the display string in field 3: `<function/N>` vs `<lambda/N>`.

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
```

### CLI Arguments

```zymbol
>< args                        // capture CLI args as string array
>> args ¶
// Run: zymbol run script.zy one two three
// → [one, two, three]
```

> **Note**: `><` capture only works in tree-walker mode.

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

### String Interpolation

Works in **any context** — assignments, arguments, array literals, etc.:

```zymbol
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
json = "\{\"key\":\"value\"\}"            // → {"key":"value"}
```

> **⚠ False warning**: `unused variable 'name'` may appear even when `name` is used
> inside an interpolated string. This is a static analyzer bug — ignore it.

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
Ident, and List.

### Literal and Range Patterns

```zymbol
score = 85
grade = ?? score {
    90..100 : 'A'
    80..89  : 'B'
    70..79  : 'C'
    60..69  : 'D'
    _       : 'F'
}
>> "grade: " grade ¶

color = "red"
code = ?? color {
    "red"   : "#FF0000"
    "green" : "#00FF00"
    "blue"  : "#0000FF"
    _       : "#000000"
}
>> code ¶
```

### Comparison Patterns

A comparison pattern (`< expr`, `> expr`, `<= expr`, `>= expr`, `== expr`, `<> expr`) implicitly
compares the scrutinee against `expr`. Arms are tested in order; first match wins.

```zymbol
temperature = -5
state = ?? temperature {
    < 0   : "ice"
    < 20  : "cold"
    < 35  : "warm"
    _     : "hot"
}
>> state ¶    // → ice

n = 42
?? n {
    == 0    : { >> "zero" ¶ }
    < 0     : { >> "negative" ¶ }
    _       : { >> "positive: " n ¶ }
}
```

### Ident Patterns

An identifier used as a pattern looks up the named variable at runtime:
- **Scalar variable** → equality check (`scrutinee == var`)
- **Array variable** → containment check (`scrutinee ∈ var`)

```zymbol
expected = 200
code = 200
?? code {
    expected : "ok"
    _        : "fail"
}
// → ok

weekdays = ["Mon", "Tue", "Wed", "Thu", "Fri"]
day = "Mon"
?? day {
    weekdays : "weekday"
    _        : "weekend"
}
// → weekday
```

### List Patterns

`[...]` patterns have **dual semantics** based on the scrutinee's type at runtime:

- **Array scrutinee** → structural match (length + element-by-element)
- **Scalar scrutinee** → containment: does the scalar appear in the literal list?

```zymbol
// Scalar containment
n = 3
?? n {
    [1, 2] : "low"
    [3, 4] : "mid"
    [5, 6] : "high"
    _      : "other"
}
// → mid

// Structural array match
cmd = ["run", "main.zy"]
?? cmd {
    ["run", _]    : { >> "run command" ¶ }
    ["build", _]  : { >> "build command" ¶ }
    []            : { >> "empty" ¶ }
    _             : { >> "unknown" ¶ }
}
// → run command

// Match on array length/shape
data = [10, 20, 30]
?? data {
    [_]       : { >> "one element" ¶ }
    [_, _]    : { >> "two elements" ¶ }
    [_, _, _] : { >> "three elements" ¶ }
    _         : { >> "more" ¶ }
}
// → three elements
```

> **⚠ Not implemented**: Identifier binding in patterns (`n : n * 2`).

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
// → ZzZzZzZzZz

@ 100 { >> "*" }
// → (100 asterisks)
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
>> ¶

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
>> ¶

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
r = classify(9)              // → "Fizz"

// In output — any position
>> classify(15) ¶            // → FizzBuzz
>> "res=" classify(6) ¶      // → res=Fizz
>> classify(3) " and " classify(5) ¶   // → Fizz and Buzz

// As a condition
? is_big(20) { >> "big" ¶ }

// As match subject
label = ?? classify(6) {
    "Fizz" : "mult of 3"
    "Buzz" : "mult of 5"
    _      : "other"
}

// Nested (composition)
r = double(double(3))        // → 12

// Arithmetic with function calls
r = double(4) + double(3)    // → 14

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

Named functions (`name(params) { }`) execute in a **fully isolated scope** — they do not capture outer variables and cannot read or write the caller's locals. Their only inputs are their parameters (including `<~` output params):

```zymbol
x = 42
peek() { <~ x }    // runtime error: undefined variable: 'x'
```

Use lambdas when you need to close over outer state; use named functions when you want strict isolation.

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
len = arr$#
>> len ¶        // → 5
>> (arr$#) ¶    // ✅ parentheses required in >>
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
>> words$^+ ¶    // → ["apple", "banana", "cherry", "date"]
>> words$^- ¶    // → ["date", "cherry", "banana", "apple"]
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
```

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

> **Known limitation (L14)**: Destructuring does not verify constant immutability. Assigning into a name previously declared with `:=` will silently overwrite it instead of raising an error. See §20 L14.

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

// Rows 1-2 at col 2; rows 2-3 at col 3 → [[2, 5], [6, 9]]
>> m[[1..2>2] ; [2..3>3]] ¶

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

For named tuples, rebuild them explicitly:

```zymbol
person = (name: "Alice", age: 25)
older  = (name: person.name, age: 26)
>> person.age ¶    // → 25
>> older.age ¶     // → 26
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

### Iterating Characters

```zymbol
@ c:"hello" { >> c "-" }
>> ¶    // → h-e-l-l-o-
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

> **⚠ Known limitation**: `$!!` is only supported inside **named functions**. Using it
> inside a lambda does not propagate to the lambda's caller. See [L13](#l13----from-lambdas-not-supported).

```zymbol
process(value) {
    ? value < 0 {
        value$!!    // propagates error up to caller
    }
    <~ value * 2
}
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
    <# ./dep <= d          // imports (must precede re-exports that reference the alias)

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
| `<# path <= alias` | ✓ | Import |
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

### Visibility Model

| Declaration | Exported in `#>` | External access | Persists across calls |
|-------------|------------------|-----------------|-----------------------|
| `PI := 3.14` | yes | `alias.PI` (read-only) | yes (immutable) |
| `count = 0` | no (excluded even if listed) | ✗ error | **yes — write-back** |
| `fn()` | yes | `alias::fn()` | — |
| `private_fn()` | no | ✗ error | — |

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
<# ./counter <= c

c::increment()         // count → 1
c::increment()         // count → 2
n = c::get_value()     // n = 2
>> n ¶

x = c.count            // ✗ Runtime error: Module 'c' has no constant 'count'
```

### Importing and Using

```zymbol
// Import with alias (alias is required)
<# ./lib/utils <= u

// Call exported function
result = u::add(5, 3)
>> result ¶    // → 8

// Access exported constant
pi = u.PI
>> pi ¶        // → 3.14159
```

### Import Paths

```zymbol
<# ./module <= m         // same directory
<# ../shared/lib <= s    // parent directory
<# ./sub/folder <= c     // subdirectory
```

### Export Aliases

```zymbol
// Export with a different public name
#> {
    internal_fn <= public_name
    INTERNAL_CONST <= PUBLIC_CONST
}
```

### Re-export from Another Module

Use `::` to re-export a function imported from another module, and `.` to re-export a constant. Place the `<#` import before `#>` so the alias is in scope. The re-export alias follows `<=`:

```zymbol
// math.zy
# math {
    <# ./core <= c

    #> {
        c::add           // re-export function as-is (callers use m::add)
        c::add <= sum    // re-export function with different public name
        c.PI             // re-export constant
        c.PI <= TAU      // re-export constant with different name
    }
}
```

> **Note**: Re-export of constants via `.` is subject to [L3](#l3----module-aliasconst-does-not-work).

### Subdirectory Module Convention

```zymbol
# .subfolder_file {    // dot prefix for modules inside subfolders
    #> { ... }
    // ...
}
```

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
>> f#? ¶              // → (##->, 1, <function/1>)

lam = (a, b) -> a + b
>> lam#? ¶            // → (##->, 2, <lambda/2>)

// Extract just the type (intermediate variable required)
meta = 42#?
t = meta[1]
>> t ¶    // → ###
```

**Display format**: named functions show as `<function/N>`, anonymous lambdas as `<lambda/N>`, where `N` is the arity.

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
| `##!expr` | ToIntTrunc | Converts Float to Int, truncating toward zero |

> **Convention**: `##.` mirrors `#.N` (round/decimal), `##!` mirrors `#!N` (truncate).
> `###` is a dedicated rounding cast with no decimal-precision argument.

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
Numeral mode only affects **output** (`>>`); internal arithmetic always uses
binary integers and IEEE-754 floats regardless of the active script.

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
    #१ : { >> "हाँ" ¶ }     // → हाँ
    #०  : { >> "नहीं" ¶ }
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

- Mode is **file-local** — each file starts in ASCII mode.
- Mode changes take effect **immediately** at the statement that contains
  `#d0d9#` and persist until the next mode-switch in the same file.
- Importing a module does not inherit or alter the caller's mode.
- The REPL respects the active mode: expression results are displayed in the
  currently active script.

### Rules Summary

| Rule | Detail |
| ---- | ------ |
| Default mode | ASCII (`0`–`9`) |
| Activation token | `#d0d9#` — zero and nine of any supported block |
| Affected output | `>>` for Int, Float, Bool |
| Unaffected | String content, Char, Array brackets, Tuple parentheses |
| Bool prefix | `#` always ASCII; digit adapts to active script |
| Literals | Any script's digits valid as integer literals in source |
| Float decimal point | Always ASCII `.` regardless of active mode |
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
<# ./calc <= c

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

