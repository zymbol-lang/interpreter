<p align="center">
  <img src="logo.png" alt="Zymbol-Lang" width="180"/>
</p>

<h1 align="center">Zymbol-Lang — Interpreter</h1>

<p align="center">
  A minimalist symbolic programming language with no keywords.<br/>
  Pure symbols for every construct. Full Unicode. Built in Rust.
</p>

<p align="center">
  <img src="https://img.shields.io/badge/language-Rust-orange?style=flat-square"/>
  <img src="https://img.shields.io/badge/license-AGPL--3.0-blue?style=flat-square"/>
  <img src="https://img.shields.io/badge/status-active-brightgreen?style=flat-square"/>
</p>

---

## What is Zymbol-Lang?

Zymbol-Lang is a programming language with **no reserved keywords**. Every construct —
conditionals, loops, I/O, functions, error handling — is expressed with symbols:

```zymbol
// Hello World
>> "Hello, World!" ¶

// Variables and output
name = "Alice"
>> "Hello, " name ¶

// Functions
greet(name) {
    >> "Hi, " name "!" ¶
}
greet("Bob")
```

The language is designed to be **language-agnostic**: booleans are `#1`/`#0` instead of
`true`/`false`, and Unicode identifiers mean you can write code in any language.

---

## Features

- **No keywords** — pure symbolic syntax (`?` if, `@` loop, `>>` output, `->` lambda)
- **Dual execution** — tree-walker interpreter and register-based VM (`--vm`)
- **Full Unicode** — identifiers, strings, and operators support emoji and any Unicode
- **Closures** — lambdas capture outer scope variables
- **Pattern matching** — `??` operator with ranges and guard patterns
- **Module system** — file-based imports with aliases (`<# ./lib/math <= m`)
- **Error handling** — `!?` try / `:!` catch / `:>` finally with typed errors
- **Interactive REPL** — with history and variable inspection
- **LSP server** — diagnostics, go-to-definition, hover (VS Code extension available)
- **Formatter** — built-in code formatter (`zymbol fmt`)
- **Turing complete** — recursion, HOF, closures, arbitrary computation

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
# Tree-walker (default)
zymbol run hello.zy

# Register VM (faster for compute-heavy programs)
zymbol run hello.zy --vm

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
| Output / Input | `>>` (print), `<<` (read), `¶` (newline) |
| Control flow | `?` (if), `_?` (else if), `_` (else) |
| Match | `??` |
| Loops | `@` (loop/while/for), `@!` (break), `@>` (continue) |
| Functions | `->` (lambda), `<~` (return) |
| Collections | `$#` (length), `$+` (append), `$-` (remove), `$~` (update), `$?` (contains), `$[..]` (slice) |
| Errors | `!?` (try), `:!` (catch), `:>` (finally), `$!` (is error), `$!!` (propagate) |
| Modules | `#` (declare), `#>` (export), `<#` (import), `<=` (alias), `::` (call), `.` (access) |
| Types | `#1`/`#0` (bool), `'c'` (char), `"s"` (string) |
| Format | `e\|x\|` (scientific), `c\|x\|` (comma sep), `#.N\|x\|` (round), `#!N\|x\|` (truncate) |
| Base | `0b` `0o` `0d` `0x` (literals and conversions) |

### Variables and Types

```zymbol
x = 42              // Int
pi = 3.14159        // Float
name = "Zymbol"     // String
active = #1         // Bool (true = #1, false = #0)
letter = 'Z'        // Char
PI := 3.14159       // Const (immutable)
```

### Output (no auto-newline — explicit `¶`)

```zymbol
>> "Hello" ¶                    // newline
>> "Score: " score ¶            // string + variable
>> "a=" a " b=" b ¶             // multiple values by juxtaposition
>> (arr$#) ¶                    // postfix ops need parentheses
```

### Control Flow

```zymbol
? age >= 18 {
    >> "Adult" ¶
}
_? age >= 13 {
    >> "Teenager" ¶
}
_{
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
>> "Grade: " grade ¶
```

### Loops

```zymbol
// Infinite loop
@ {
    >> "Forever" ¶
    @!
}

// While
@ x < 10 {
    x = x + 1
}

// For-each
@ item:[1, 2, 3, 4, 5] {
    >> item ¶
}

// Range
@ i:0..9 {
    >> i ¶
}
```

### Functions and Lambdas

```zymbol
// Traditional function
factorial(n) {
    ? n <= 1 { <~ 1 }
    _{ <~ n * factorial(n - 1) }
}

// Lambda (implicit return)
double = x -> x * 2

// Multi-param lambda
add = (a, b) -> a + b

// Block lambda (explicit return)
clamp = (val, lo, hi) -> {
    ? val < lo { <~ lo }
    ? val > hi { <~ hi }
    <~ val
}

>> factorial(10) ¶
>> double(21) ¶
>> add(3, 4) ¶
```

### Collections

```zymbol
// Arrays
nums = [1, 2, 3, 4, 5]
len  = nums$#           // 5
nums = nums$+ 6         // append
has  = nums$? 3         // #1 (contains)
sub  = nums$[1..3]      // [2, 3]

// Named tuples
person = (name: "Alice", age: 25)
>> person.name ¶        // "Alice"
>> person[1] ¶          // 25
```

### Strings

```zymbol
msg = "Hello", ", ", "World"        // concat with comma
>> "Score: " #.2|98.7654| ¶         // "Score: 98.77"
words = "a,b,c" / ','               // ["a", "b", "c"]
has = "hello"$? 'e'                 // #1
```

### Error Handling

```zymbol
!? {
    data = read_file("config.txt")
} :! ##IO {
    >> "File not found" ¶
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
>> m::sqrt(16) ¶        // 4.0
>> m.PI ¶               // 3.14159
```

---

## Architecture

The interpreter is a Rust workspace of 17 crates:

```
Foundation:   zymbol-span  zymbol-error  zymbol-common
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

Benchmarks vs CPython 3 (release build):

| Benchmark | Tree-walker | VM | Python |
|-----------|:-----------:|:--:|:------:|
| Stress loop | ~200ms | **67ms** | 77ms |
| Match | ~165ms | **50ms** | 75ms |
| Collections | ~14s | **33ms** | 44ms |
| Recursion | ~1480ms | 308ms | **218ms** |

The VM is 4.4× faster than the tree-walker on `fib(35)`.

---

## Testing

```bash
# Unit tests (all crates)
cargo test

# End-to-end tests
cd tests && bash run.sh

# Tree-walker vs VM parity check
bash tests/scripts/vm_compare.sh
```

Current status: **88/88 E2E tests passing** (44 tree-walker + 44 VM).
VM parity: **99/99 PASS**.

---

## Project Layout

```
interpreter/
├── Cargo.toml          # Workspace (17 crates)
├── zymbol.ebnf         # Formal grammar (EBNF)
├── install-zymbol.sh   # Install script
├── crates/             # Rust source crates
├── tests/              # End-to-end test suite
├── docs/               # Extended documentation
├── LICENSE             
├── LICENSE-AGPL-3.0    # AGPL-3.0
└── LICENSE-CC-BY-SA-4.0
```

---

## Documentation

- [ARCHITECTURE.md](./ARCHITECTURE.md) — Interpreter architecture and pipelines
- [I18N.md](./I18N.md) — Multilingual code architecture: writing libraries in any natural language and bridging them with translation modules
- [zymbol.ebnf](./zymbol.ebnf) — Formal grammar specification
- [docs/](./docs/) — Extended documentation

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
