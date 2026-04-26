<p align="center">
  <img src="logo.png" alt="Zymbol-Lang" width="180"/>
</p>

<h1 align="center">Zymbol-Lang — Interpreter</h1>

<p align="center">
  A minimalist symbolic programming language with no keywords.<br/>
  Pure symbols for every construct. Full Unicode. Built in Rust.
</p>

<p align="center">
  <img src="https://img.shields.io/badge/version-v0.0.4-informational?style=flat-square"/>
  <img src="https://img.shields.io/badge/language-Rust-orange?style=flat-square"/>
  <img src="https://img.shields.io/badge/license-AGPL--3.0-blue?style=flat-square"/>
  <img src="https://img.shields.io/badge/status-active-brightgreen?style=flat-square"/>
</p>

---

## What is Zymbol-Lang?

Zymbol started as an **esoteric programming language** — a single tight question taken seriously:
*what happens if you remove every keyword?* No `if`, no `while`, no `function`, no `return`.
The original experiment is on [esolangs.org](https://esolangs.org). Then the idea grew.

The reason the constraint matters: every mainstream language assumes the programmer reads English.
Keywords are English words. A developer writing in Spanish, Arabic, or Devanagari is permanently
coding in a second language at the syntactic level, even when identifiers can be localized.

Removing keywords entirely is the minimum change to break that assumption. A symbol carries no
etymology — `?` does not say *if*, `@` does not say *while*. Any human language can be native:

```zymbol
// Spanish — no translation at the syntax level
edad = 25
? edad >= 18 {
    >> "adulto" ¶
}

// Devanagari — first-class program, no flag or special mode required
सक्रिय = #१
@ i:१..५ { >> i " " }   // → १ २ ३ ४ ५
```

Spanish with full accents, Devanagari, Arabic, Korean — or Klingon pIqaD for the ones who
program in the language of the Empire (CSUR U+F8F0–U+F8F9, fully supported, requires pIqaD font).

The esolang became a general-purpose language. What stayed minimal is the growth mechanism:
no new construct ever borrows a word from any natural language.

---

## Features

- **No keywords** — pure symbolic syntax (`?` if, `@` loop, `>>` output, `->` lambda)
- **Dual execution** — tree-walker interpreter and register-based VM (`--vm`)
- **Full Unicode** — identifiers, strings, and numerals support any Unicode script
- **First-class functions** — named functions as values, HOF arguments, and closures
- **Pattern matching** — `??` with literals, ranges, comparisons, ident, and list patterns
- **Multi-dimensional indexing** — `arr[i>j]`, flat/structured extraction, ranges on nav steps
- **Destructuring** — `[a, *rest] = arr`, `(name: n, age: a) = tuple`
- **Module system** — file-based imports with aliases, re-exports, and i18n translation layers
- **Error handling** — `!?` try / `:!` catch (typed or generic) / `:>` finally
- **Higher-order functions** — `$>` map, `$|` filter, `$<` reduce, `$^` sort with comparator
- **Pipe operator** — `|>` with `_` placeholder: `x |> f(_, 2)`
- **Type metadata** — `x#?` returns `(type_symbol, count, display)`
- **Interactive REPL** — with history and variable inspection
- **LSP server** — diagnostics, go-to-definition, hover (VS Code extension available)
- **Formatter** — built-in code formatter (`zymbol fmt`)
- **Shell integration** — `<\ cmd \>` bash execution, `</ script.zy />` sub-script

---

## Quick Start

### Prerequisites

- Rust 1.75+ — install from [rustup.rs](https://rustup.rs)

### Build and Install

```bash
git clone https://github.com/zymbol-lang/interpreter.git
cd interpreter

# Build release binary
cargo build --release

# Install to PATH
cp target/release/zymbol ~/.local/bin/
# or use the install script
bash install-zymbol.sh
```

### Run

```bash
# Tree-walker (default, best error messages)
zymbol run hello.zy

# Register VM (faster for compute-heavy programs)
zymbol run --vm hello.zy

# Interactive REPL
zymbol repl

# Check syntax without running
zymbol check program.zy

# Format code
zymbol fmt program.zy --write

# Compile to standalone executable
zymbol build program.zy -o myprogram --release
```

---

## Language at a Glance

### Operators Reference

| Category | Operators |
|----------|-----------|
| Assignment | `=` (mutable), `:=` (const) |
| Output / Input | `>>` (print), `<<` (read), `¶` or `\\` (newline) |
| Control flow | `?` (if), `_?` (else if), `_` (else) |
| Match | `??` with literal, range, comparison `< expr`, ident, list `[a,b]`, wildcard `_` |
| Loops | `@` (infinite/while/times/for), `@!` (break), `@>` (continue), `@:label` (labeled) |
| Functions | `->` (lambda), `<~` (return / output param) |
| Collections | `$#` (len), `$+` (append), `$-` (remove), `$[..]` (slice), `$?` (contains), `$??` (find all), `$^+`/`$^-` (sort), `$^` (custom sort), `$>` (map), `$|` (filter), `$<` (reduce) |
| Strings | `$~~[p:r]` (replace), `$/` (split), `$++` (build) |
| Multi-dim index | `arr[i>j]` (scalar), `arr[p;q]` (flat), `arr[[g];[g]]` (structured) |
| Pipe | `\|>` with `_` placeholder |
| Errors | `!?` (try), `:!` (catch), `:>` (finally), `$!` (is error), `$!!` (propagate) |
| Modules | `#` (declare), `#>` (export), `<#` (import), `<=` (alias), `::` (call), `.` (access) |
| Types | `#1`/`#0` (bool), `'c'` (char), `"s"` (string), `x#?` (type metadata) |
| Casts | `##.expr` (→Float), `###expr` (→Int round), `##!expr` (→Int truncate) |
| Format | `#.N\|x\|` (round), `#!N\|x\|` (truncate), `#,\|x\|` (comma sep), `#^\|x\|` (scientific) |
| Base | `0b` `0o` `0d` `0x` (literals and conversions) |
| Numeral mode | `#d0d9#` — switch output script; `#09#` restores ASCII |

### Variables and Types

```zymbol
x = 42              // Int (64-bit signed)
pi = 3.14159        // Float
name = "Zymbol"     // String (interpolation: "Hello {name}")
active = #1         // Bool  (#1 = true, #0 = false)
letter = 'Z'        // Char
PI := 3.14159       // Const (immutable — reassignment is a runtime error)
```

### Output (no auto-newline — explicit `¶`)

```zymbol
>> "Hello" ¶                    // with newline
>> "Score: " score ¶            // string + variable (juxtaposition)
>> "a=" a " b=" b ¶             // multiple values
>> (arr$#) ¶                    // postfix ops need parentheses in >>
>> "Sum: " (x + y) ¶            // parenthesized expression
```

### Control Flow

```zymbol
? age >= 18 {
    >> "Adult" ¶
} _? age >= 13 {
    >> "Teenager" ¶
} _ {
    >> "Child" ¶
}
```

### Pattern Matching

```zymbol
grade = ?? score {
    90..100 : 'A'
    80..89  : 'B'
    70..79  : 'C'
    60..69  : 'D'
    _       : 'F'
}

// Comparison patterns
state = ?? temperature {
    < 0  : "ice"
    < 20 : "cold"
    < 35 : "warm"
    _    : "hot"
}

// List containment
?? n {
    [1, 2] : "low"
    [3, 4] : "mid"
    _      : "other"
}
```

### Loops

```zymbol
// Infinite loop
@ {
    >> "Forever" ¶
    @!
}

// While
@ x < 10 { x++ }

// Repeat exactly N times
@ 5 { >> "*" }     // → *****

// For-each over array
@ item:[1, 2, 3, 4, 5] { >> item ¶ }

// Range (inclusive both ends)
@ i:1..5 { >> i " " }    // → 1 2 3 4 5

// Range with step
@ i:1..9:2 { >> i " " }  // → 1 3 5 7 9

// Labeled loops (break outer from inner)
@:outer i:1..4 {
    @ j:1..4 {
        ? j == 2 { @:outer> }
        >> "{i}{j} "
    }
}
```

### Functions and Lambdas

```zymbol
// Named function
factorial(n) {
    ? n <= 1 { <~ 1 }
    <~ n * factorial(n - 1)
}

// Lambda (implicit return)
double = x -> x * 2

// Multi-param lambda
add = (a, b) -> a + b

// Block lambda (explicit return)
describe = x -> {
    ? x > 0 { <~ "positive" }
    _? x < 0 { <~ "negative" }
    <~ "zero"
}

// Output parameters (pass by reference)
swap(a<~, b<~) {
    tmp = a
    a = b
    b = tmp
}
x = 10
y = 20
swap(x, y)    // x=20, y=10

>> factorial(10) ¶
>> double(21) ¶
>> add(3, 4) ¶
```

### Collections

```zymbol
// Arrays (1-based indexing)
nums = [1, 2, 3, 4, 5]
len  = nums$#           // 5
nums = nums$+ 6         // append → [1,2,3,4,5,6]
has  = nums$? 3         // #1
sub  = nums$[2..4]      // [2,3,4]
srt  = nums$^+          // sort ascending

// Array element update
nums[1] = 99
nums[2] += 10

// Destructuring
[first, *rest] = nums    // first=99, rest=[...remaining]

// Named tuples
person = (name: "Alice", age: 25)
>> person.name ¶         // Alice
>> person.age ¶          // 25

// Array of named tuples
people = [
    (name: "Alice", age: 25),
    (name: "Bob",   age: 30)
]
sorted = people$^ (a, b -> a.age < b.age)
```

### Multi-dimensional Indexing

```zymbol
m = [[1,2,3], [4,5,6], [7,8,9]]

>> m[2>3] ¶              // → 6  (row 2, col 3)
>> m[-1>-1] ¶            // → 9  (last row, last col)

// Flat extraction — multiple paths → [v1, v2, v3]
diag = m[1>1 ; 2>2 ; 3>3]    // → [1, 5, 9]

// Structured extraction — array of arrays
corners = m[[1>1, 1>3] ; [3>1, 3>3]]
>> corners[1] ¶          // → [1, 3]
```

### Higher-Order Functions and Pipe

```zymbol
nums = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]

doubled = nums$> (x -> x * 2)
evens   = nums$| (x -> x % 2 == 0)
sum     = nums$< (0, (acc, x) -> acc + x)

// Named functions work directly as HOF arguments
double(x) { <~ x * 2 }
is_big(x) { <~ x > 5 }

r = nums$> double      // no wrapper lambda needed
f = nums$| is_big

// Pipe operator
result = 16 |> double |> double    // 64
```

### Strings

```zymbol
s = "Hello World"

// Length, contains, slice
n     = s$#               // 11
found = s$? "World"       // #1
sub   = s$[1..5]          // "Hello"

// Split, replace, build
parts = "a,b,c" $/ ','           // ["a", "b", "c"]
rep   = s$~~["l":"L"]            // "HeLLo WorLd"
out   = "n=" $++ 42 " flag=" #1  // "n=42 flag=#1"

// Iteration
@ c:"hello" { >> c "-" }         // h-e-l-l-o-
```

### Numeral Modes

Output digits in any of **69 Unicode scripts** at runtime. The mode-switch token
takes the zero-digit and nine-digit of the target script enclosed in `#…#`:

```zymbol
n = 42

#०९#   // activate Devanagari (U+0966–U+096F)
>> n ¶          // → ४२
>> 3.14 ¶       // → ३.१४
>> #1 ¶         // → #१   (# stays ASCII; digit adapts)

#٠٩#   // activate Arabic-Indic (U+0660–U+0669)
>> n ¶          // → ٤٢

#09#   // restore ASCII
>> n ¶          // → 42
```

Native-script digits are valid **integer literals** in source code — in loop
ranges, conditions, and assignments — and normalise to the same internal value:

```zymbol
#०९#
@ i:१..१५ {
    ? i % १५ == ० { >> "FizzBuzz" ¶ }
    _? i % ३  == ० { >> "Fizz" ¶ }
    _? i % ५  == ० { >> "Buzz" ¶ }
    _ { >> i ¶ }
}
```

Selected scripts (25 of 69): Arabic-Indic, Devanagari, Bengali, Gujarati, Tamil,
Telugu, Thai, Tibetan, Myanmar, Khmer, Mongolian, Mathematical Bold/Monospace,
Segmented/LCD, **Klingon pIqaD** (CSUR PUA, requires pIqaD font), and more.  
See `crates/zymbol-lexer/src/digit_blocks.rs` for the full registry.

### Error Handling

```zymbol
!? {
    data = risky_operation()
} :! ##IO {
    >> "I/O error: " _err ¶
} :! ##Index {
    >> "Index out of bounds" ¶
} :! {
    >> "Unexpected error: " _err ¶
} :> {
    cleanup()
}

// Check and propagate
? result$! { result$!! }
```

### Modules

```zymbol
// lib/math.zy
# math
PI := 3.14159
sqrt(x) { <~ x ^ 0.5 }
#> { sqrt, PI }

// main.zy
<# ./lib/math <= m
>> m::sqrt(16) ¶        // → 4.0
>> m.PI ¶               // → 3.14159
```

### Multilingual Code (i18n)

Zymbol's module system enables writing libraries in any natural language and bridging
them via zero-cost translation modules. A Spanish math library can be consumed in Greek,
Korean, Hebrew, or Mandarin without any changes to the original:

```zymbol
// Consumer in Greek — never reads the original Spanish source
<# ./matematicas/ελληνικά <= μαθ
>> μαθ::προσθέτω(10, 5) ¶    // → 15
>> μαθ.ΠΙ ¶                   // → 3.14159
```

See [I18N.md](./I18N.md) for the full three-layer pattern.

---

## Architecture

The interpreter is a Rust workspace of 18 crates:

```
Foundation:   zymbol-span  zymbol-error  zymbol-common  zymbol-intrinsics
Frontend:     zymbol-lexer  zymbol-ast  zymbol-parser
Analysis:     zymbol-semantic
Tree-walker:  zymbol-interpreter
VM:           zymbol-bytecode  zymbol-compiler  zymbol-vm
Tooling:      zymbol-formatter  zymbol-analyzer  zymbol-lsp
              zymbol-repl  zymbol-standalone
Entry point:  zymbol-cli
```

See [ARCHITECTURE.md](./ARCHITECTURE.md) for the full pipeline, data structures, and
performance benchmarks.

---

## Performance

Benchmarks (release build):

| Benchmark | Tree-walker | VM |
|-----------|:-----------:|:--:|
| Stress loop | ~200ms | **67ms** |
| Match | ~165ms | **50ms** |
| Collections | ~14s | **33ms** |
| Recursion | ~1480ms | 308ms |

The VM is 4.4× faster than the tree-walker on `fib(35)`.

---

## Testing

```bash
# Unit tests (all 18 crates)
cargo test

# Tree-walker vs VM parity check
bash tests/scripts/vm_compare.sh
```

Current status: **717 unit tests passing** across all crates.  
VM parity: **403/405 PASS** (2 vm-skip for TW-only analysis tests, 0 failures).

---

## Project Layout

```
interpreter/
├── Cargo.toml           # Workspace (17 crates)
├── zymbol-lang.ebnf     # Formal grammar (EBNF, v2.3.0)
├── install-zymbol.sh    # Install script
├── crates/              # Rust source crates
├── tests/               # End-to-end test suite (405 files)
├── docs/                # Extended documentation
├── LICENSE
├── LICENSE-AGPL-3.0     # AGPL-3.0 (interpreter source)
└── LICENSE-CC-BY-SA-4.0 # CC-BY-SA-4.0 (documentation)
```

---

## Documentation

- [GUIDE.md](./GUIDE.md) — Full language guide with verified examples (all constructs)
- [REFERENCE.md](./REFERENCE.md) — Known limitations, error taxonomy, complete symbol table
- [IMPLEMENTATION.md](./IMPLEMENTATION.md) — EBNF grammar, coverage table, TW/VM internals
- [ARCHITECTURE.md](./ARCHITECTURE.md) — Interpreter architecture and performance benchmarks
- [I18N.md](./I18N.md) — Multilingual code: writing and bridging libraries across natural languages

---

## License

This project is available under multiple licenses:

- **READ LICENSE** — [`LICENSE`](./LICENSE)
- **AGPL-3.0** — [`LICENSE-AGPL-3.0`](./LICENSE-AGPL-3.0) (interpreter source)
- **CC-BY-SA-4.0** — [`LICENSE-CC-BY-SA-4.0`](./LICENSE-CC-BY-SA-4.0) (documentation)

---

<p align="center">
  Made with Rust · <a href="https://github.com/zymbol-lang">github.com/zymbol-lang</a>
</p>
