<p align="center">
  <img src="logo.png" alt="Zymbol-Lang" width="180"/>
</p>

<h1 align="center">Zymbol-Lang — Interpreter</h1>

<p align="center">
  A minimalist symbolic programming language with no keywords.<br/>
  Pure symbols for every construct. Full Unicode. Built in Rust.
</p>

<p align="center">
  <img src="https://img.shields.io/badge/version-v0.0.6-informational?style=flat-square"/>
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

# Package into standalone executable
# Note: bundles the source code and Zymbol interpreter into one binary — not native compilation.
# Requires: Rust/Cargo installed, full repo checkout, and must be run from interpreter/.
# See aprende_zymbol/avanzado/05_herramientas.md for full setup instructions.
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
| Strings | `$~~[p:r]` (replace), `$/` (split), `$++` (build), `$*` (repeat N times) |
| TUI / Terminal | `@~` (sleep ms), `>>!` (clear screen), `>>?` (query size → `[rows,cols]`), `>>~` (positioned print), `<<\|` (blocking keypress), `<<\|?` (non-blocking keypress), `>>|` (TUI block) |
| Multi-dim index | `arr[i>j]` (scalar), `arr[p;q]` (flat), `arr[[g];[g]]` (structured) |
| Pipe | `\|>` with `_` placeholder |
| Errors | `!?` (try), `:!` (catch), `:>` (finally), `$!` (is error), `$!!` (propagate) |
| Modules | `#` (declare), `#>` (export), `<#` (import), `=>` (alias / re-export rename), `::` (call), `.` (access) |
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
    90..100 => 'A'
    80..89  => 'B'
    70..79  => 'C'
    60..69  => 'D'
    _       => 'F'
}

// Comparison patterns
state = ?? temperature {
    < 0  => "ice"
    < 20 => "cold"
    < 35 => "warm"
    _    => "hot"
}

// List containment
label = ?? n {
    [1, 2] => "low"
    [3, 4] => "mid"
    _      => "other"
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

// Repeat string N times
line  = "=" $* 20                // "===================="
sep   = "-" $* 10                // "----------"

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
// lib/math.zy  (block syntax — all content inside braces)
# math {
    #> { sqrt, PI }

    PI := 3.14159
    sqrt(x) { <~ x ^ 0.5 }
}

// main.zy
<# ./lib/math => m
>> m::sqrt(16) ¶        // → 4.0
>> m.PI ¶               // → 3.14159
```

### Multilingual Code (i18n)

Zymbol's module system enables writing libraries in any natural language and bridging
them via zero-cost translation modules. A Spanish math library can be consumed in Greek,
Korean, Hebrew, or Mandarin without any changes to the original:

```zymbol
// Consumer in Greek — never reads the original Spanish source
<# ./matematicas/ελληνικά => μαθ
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

Current status: **820 tests passing** via `cargo test` (0 failed, 0 ignored).  
VM parity: **478/478 PASS** (478 files, 0 `@vm-skip` — all TUI/input tests now run in both TW and VM).  
Golden files: **464/464 PASS** via `expected_compare.sh` (includes 8 TUI + 8 input category tests).

---

## Real-World Validation Projects

Each release milestone is stress-tested by building a non-trivial program entirely in Zymbol.
Bugs discovered during construction feed back directly into the language.

The projects below also serve as cross-language proof: each is written in a different
natural language (English, Mandarin Chinese, Spanish, Klingon pIqaD, and Spanish again
for scientific computing), demonstrating that Zymbol's keyword-free design is genuinely
language-neutral — no flags, no special modes, no translation layer at the syntax level.

### Summary

| Project | Version | Code language | Features validated |
|---------|---------|---------------|--------------------|
| [ZethyCLI](https://github.com/zymbol-lang/zy-ZethyCLI) | **v0.0.3** | English | Modules, `<\cmd\>` shell exec, HTTP via Ollama, multi-turn state, string building |
| [ZyAudit](https://github.com/zymbol-lang/zy-ZyAudit) | **v0.0.4** | 中文 (Mandarin) | CJK identifiers as first-class citizens, named tuples, HOF pipeline, `$~~` replace |
| [Serpiente](https://github.com/zymbol-lang/zy-Serpiente) | **v0.0.5** | Español | TUI primitives, register VM, hot-definition `°`, tuple equality, labeled loops |
| [Hov veS](https://github.com/zymbol-lang/zyKlingonGalaxy) | **v0.0.5** | pIqaD (Klingon) | Multi-module orchestration, Galaxian formation AI, delta rendering, dual projectiles, 3-language i18n |
| [Zofía](https://github.com/zymbol-lang/zy-Zofia) | **v0.0.6** | Español | Scientific computing, transformer AI from scratch, `^` float exponents, global `:=` scope fix, `#.N\|x\|` formatting |

---

### ZethyCLI — v0.0.3 · English

Multi-turn AI chat CLI for Ollama. Stress-tests the module system, HTTP via bash-exec,
string interpolation, and persistent state across loop iterations.

```zymbol
// ZethyCLI — multi-turn AI chat
<# ./lib/ollama => ai
<# ./lib/http   => net

MODEL   := "llama3"
history = []

@:chat {
    << prompt
    ? prompt == "quit" { @:chat! }
    history = history$+ (role: "user", content: prompt)
    response = ai::complete(MODEL, history)
    history  = history$+ (role: "assistant", content: response)
    >> response ¶
}
```

---

### ZyAudit — v0.0.4 · 中文 (Mandarin)

Static code auditing tool. Written entirely in Mandarin identifiers — validates that
CJK characters work as first-class symbols in every language construct: functions,
named tuples, HOF arguments, and string operators.

```zymbol
// ZyAudit — 代码审计工具
<# ./模块/词法 => 词法

审计(源码路径) {
    内容  = <\ "cat {源码路径}" \>
    符号  = 词法::分析(内容)
    问题  = 符号$| (项 -> 项.类型 == "警告")
    <~ (路径: 源码路径, 问题数: 问题$#, 列表: 问题)
}

@ 文件:目标列表 {
    报告 = 审计(文件)
    >> 报告.路径 ": " 报告.问题数 " 个问题" ¶
}
```

---

### Serpiente — v0.0.5 · Español

Snake game running in the terminal. Stress-tests TUI primitives (`>>!`, `>>~`, `<<|`),
the register VM (`--vm`), hot-definition variables (`°`), and tuple equality under both
execution backends.

```zymbol
// Serpiente — juego de Snake en TUI
ANCHO := 40
ALTO  := 20

serpiente = [(10, 10), (10, 9), (10, 8)]
dirección = "abajo"
puntos    = 0

mover(serp<~, dir) {
    cabeza = serp[serp$#]
    nueva  = ?? dir {
        "arriba"    => (cabeza[1] - 1, cabeza[2])
        "abajo"     => (cabeza[1] + 1, cabeza[2])
        "izquierda" => (cabeza[1],     cabeza[2] - 1)
        "derecha"   => (cabeza[1],     cabeza[2] + 1)
    }
    serp = (serp$+ nueva)$-[1]
}

@:juego {
    tecla = <<|?
    ? tecla == "q" { @:juego! }
    dirección = ?? tecla {
        "w" => "arriba"
        "s" => "abajo"
        "a" => "izquierda"
        "d" => "derecha"
        _   => dirección
    }
    mover(serpiente, dirección)
    >>! ¶
    dibujar(serpiente, puntos)
}
```

---

### Hov veS — v0.0.5 · pIqaD (Klingon)

Galaxian-style space shooter for the terminal, set in the Klingon universe.
Written entirely in Klingon pIqaD script (CSUR U+F8D0–F8FF) — validates
multi-module orchestration, Galaxian formation AI with drift and dive attacks,
delta rendering, a dual projectile system (disruptor + rapid fire), and
3-language i18n (pIqaD / English / Spanish) threaded as a parameter through
all five cooperating modules.

The i18n approach is notable: pIqaD text renders first; EN/ES overrides
rewrite the same terminal row via a second `>>~` positioned output call — no
lookup tables, no per-string branching at every callsite.

```zymbol
// Hov veS — entry point (pIqaD identifiers shown in Roman transliteration)
<# ./Duj  => nav     // player ship
<# ./jagh => flota   // enemy fleet
<# ./bach => bach    // projectiles
<# ./HUD  => HUD     // display

[AN, AL] = >>?                              // query real terminal size
// seed from three independent BashExec entropy sources
mIS = (nS1 + nS2 * 1009 + nS3 * 6271) % 2147483647

>>| {
    °partidas = 0                           // hot-def: persist across games
    °historial = []
    idioma = HUD::sel_Hol(AN, AL)          // returns pIqaD/EN/ES token

    @:bucle {
        retardo = HUD::menu_HeH(AN, AL, idioma)
        [ghom, mIS] = flota::chen_ghom(AN, AL, 1, mIS)
        @:oleada {
            <<|? tecla
            [ghom, jaHDu, mIS, hubo_drift] =
                flota::Suy_mIw(ghom, jaHDu, AN, AL, HoS, mIS)
            [jagh_bachDu, mIS] =
                bach::jagh_tagh(jagh_bachDu, ghom, jaHDu, mIS, HoS)
            HUD::chou_bID(...)              // delta render — only changed cells
            @~ retardo
        }
        °partidas += 1
        °historial = °historial$+ nob
        res = HUD::Hegh_nav(nob, HoS, °partidas, °historial, AN, AL, idioma)
        ? res == 's' { @:bucle! }
    }
}
```

---

### Zofía — v0.0.6 · Español (scientific computing)

Transformer AI encoder built from scratch in Zymbol — tensors, gradients, attention,
and positional encoding, all in pure Zymbol with no external math library.
The project is designed as an educational resource for Spanish-speaking learners:
every identifier, comment, and document is in Spanish, with English references
to the academic literature inline.

Zofía is the primary driver of v0.0.6. Building it exposed two language issues
that were fixed during development:

- **Global `:=` scope** — constants declared at script level were invisible inside
  function bodies. Fixed in `functions_lambda.rs`: constants from the saved
  `const_vars_stack` are now injected into each fresh function call scope.
- **Float formatting** — `#.4|x|` (round) and `#!4|x|` (truncate) were already
  in the EBNF but unverified; confirmed working, closing GAP-Z004.

Discovery: `^` already handles float exponents internally via `f64::powf`, making
`sqrt(x) = x ^ 0.5` and `exp(x) = E ^ x` work natively — reducing the planned
`std/matematica` module to only `sin`, `cos`, and `ln`.

```zymbol
// Zofía — sigmoide, softmax, and positional encoding in pure Zymbol
PI := 3.14159265358979323846
E  := 2.71828182845904523536

sigmoide(x) {
    fx = ##. x
    <~ 1.0 / (1.0 + E ^ (0.0 - fx))
}

seno(x) {
    fx = ##. x
    @ fx > PI  { fx -= 2.0 * PI }
    @ fx < 0.0 - PI { fx += 2.0 * PI }
    suma = fx
    pot = fx
    fact = 1.0
    signo = -1.0
    @ n : 1..14 {
        en = ##. n
        pot = pot * fx * fx
        fact = fact * (2.0 * en) * (2.0 * en + 1.0)
        suma += signo * pot / fact
        signo = 0.0 - signo
    }
    <~ suma
}

codificacion_posicional(pos, dim, i) {
    fp = ##. pos
    fd = ##. dim
    fi = ##. i
    ?? (i % 2 == 0) {
        #1 => seno(fp / (10000.0 ^ (2.0 * fi / fd)))
        _  => coseno(fp / (10000.0 ^ (2.0 * (fi - 1.0) / fd)))
    }
}
```

---

## Project Layout

```
interpreter/
├── Cargo.toml           # Workspace (18 crates)
├── zymbol-lang.ebnf     # Formal grammar (EBNF, v3.0.0)
├── install-zymbol.sh    # Install script
├── crates/              # Rust source crates
├── tests/               # End-to-end test suite (478 vm-compare files; 464 golden .expected pairs)
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

## Authorship & AI Collaboration

Zymbol-Lang is designed by **[OscarE.EspinozaB](https://github.com/zymbol-lang/interpreter/commits?author=OscarEEspinozaB)**. Every decision about the language — its philosophy, syntax, operator semantics, type system, module design, execution model, and the verification environment used to validate correctness — originates from and is controlled by its author.

The implementation was built using **[Claude Code](https://claude.ai/code)** (Anthropic) as the engineering team: writing Rust code, tests, and tooling under the author's direction and specifications. The use of AI is transparent and intentional — it is not concealed or minimized.

What AI does not replace: the design rationale, the specification that guides each feature, the test suite that defines correctness, the judgment calls on what to build and what to reject, and the final say on every merged change. Those remain entirely with the author.

This collaboration model made it possible for a single person to deliver a complete language toolchain — interpreter, register VM, LSP server, formatter, REPL, VS Code extension, and web playground — that would otherwise require a full team. The result is not AI-generated filler: it is a meticulous, carefully guided project where AI serves as a capable and disciplined engineering partner.

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
