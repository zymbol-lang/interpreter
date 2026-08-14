<p align="center">
  <img src="logo.png" alt="Zymbol-Lang" width="180"/>
</p>

<h1 align="center">Zymbol-Lang — Interpreter</h1>

<p align="center">
  A minimalist symbolic programming language with no keywords.<br/>
  Pure symbols for every construct. Full Unicode. Built in Rust.
</p>

<p align="center">
  <img src="https://img.shields.io/badge/version-v0.0.8-informational?style=flat-square"/>
  <img src="https://img.shields.io/badge/language-Rust-orange?style=flat-square"/>
  <img src="https://img.shields.io/badge/license-AGPL--3.0-blue?style=flat-square"/>
  <img src="https://img.shields.io/badge/status-active-brightgreen?style=flat-square"/>
</p>

---

## What is Zymbol-Lang?

Zymbol started as an **esoteric programming language** — a single tight question taken seriously:
*what happens if you remove every keyword?* No `if`, no `while`, no `function`, no `return`.
The original experiment is on [esolangs.org](https://esolangs.org). Then the idea grew.

The reason the constraint matters is an old and uncontroversial one: notation travels.
Mathematics writes `∑` and `∫`; a musical stave fixes pitch and duration in a single mark; a road
sign is read correctly at speed by a driver who has never studied the local language. None of
these replace words or compete with them — they sit *beside* language, and the same mark carries
the same meaning to everyone who has learned the notation.

Zymbol applies that idea to program structure. A symbol carries no etymology: `?` does not say
*if*, `@` does not say *while*, `->` does not say *lambda*. Nothing in the syntax has to be
pronounced to be understood, so the whole naming budget goes where it belongs — to variables,
functions and modules, named in whatever language the person writing them thinks in.

This is a symbolic vocabulary, not a new paradigm. APL and its descendants use glyphs to express
array programming, where each symbol is a dense operator with its own algebra; learning the
notation *is* learning the paradigm. Zymbol's symbols do ordinary work — conditionals, loops,
functions, modules, error handling — and a programmer coming from any imperative or functional
language will recognize every construct behind the marks. Any human language can be native:

```zymbol
// Spanish — no translation at the syntax level
edad = 25
? edad >= 18 {
    >> "adulto" ¶
}

// Devanagari — first-class program, no flag or special mode required
#०९#                    // digits are written and printed in the local script
सक्रिय = #१
@ i:१..५ { >> i " " }   // → १ २ ३ ४ ५
```

Spanish with full accents, Devanagari, Arabic, Korean — or Klingon pIqaD for the ones who
program in the language of the Empire (CSUR U+F8F0–U+F8F9, fully supported, requires pIqaD font).

The esolang became a general-purpose language. What stayed minimal is the growth mechanism:
no new construct ever borrows a word from any natural language.

### An agglutinative notation

The marks are not a flat table to be memorised. An operator is a **sequence of marks, each
contributing one meaning**, with the boundaries between them visible in the written form —
the way an agglutinative language builds a word by stacking morphemes. `<<|?` is not a
trigraph that happens to mean "poll the keyboard"; it is three morphemes:

```text
<<      |       ?              $       ^       -              @       :outer  !
IN      UNIT    IRR            COLL    ORDER   REV            TEMP    LBL     FRC
"one unit from the input       "impose an order on the        "act forcefully on the
 stream, non-committally"       collection, reversed"          time-context named outer"
→ poll for a keypress          → sort descending              → break the labelled loop
```

Segmentable operators fill one slot template — `[BINDER] DOMAIN [OPERATION] [MODALITY]
[ARGUMENT]`, where the domain head (`$` collection, `@` time, `#` meta, `>>` out, `<<` in,
`?` irrealis, `!` force/error) says *which world* the operation lives in, and a modal `?` or
`!` is always the rightmost mark. So a combination that has never been written already has a
meaning, worked out in advance by the marks it is made of; implementing it is a matter of
building what the notation already said. That is why the language grows by recombination:
across v0.0.5–v0.0.9 it coined exactly **one** new base mark (`°`, the hot-definition
diacritic) and derived everything else from marks already in the inventory.

Stating it this way makes it falsifiable, which is the point: for any operator you either can
segment and gloss it, or you cannot. [SYMBOLS.md](./SYMBOLS.md) does that count for the whole
inventory — transparent forms (the majority), semi-transparent (6, where the whole means more
than the parts), and **opaque** (10, which must simply be learned: `¶`, `><`, `#1`/`#0`, the
base prefixes, `###`, `°`). It also names the six declared homographs, the exact
natural-language residue that "no keywords" does not cover (error kinds, `std/` names, `0x`),
and the eight rules a proposed operator has to pass before it can exist. A reader who has
never seen Zymbol will guess `>>` and `->` correctly and will guess `$^-` never — the second
kind is a memorisation cost, so the design keeps counting it instead of assuming it is small.

---

## Features

- **No keywords** — pure symbolic syntax (`?` if, `@` loop, `>>` output, `->` lambda)
- **Dual execution** — tree-walker interpreter and register-based VM (`--vm`)
- **Full Unicode** — identifiers, strings, and numerals support any Unicode script
- **First-class functions** — named functions as values, HOF arguments, and closures
- **Pattern matching** — `??` with literals, ranges, comparisons, ident, list and or-patterns (`'p' || 'P'`)
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
- **Standard library** — `std/math`, `std/random`, `std/io`, `std/json`, `std/net`, `std/db` (ODBC), `std/term` (display width)
- **Packages** — `.zyp` archives bundle a multi-file program into one portable file (`zymbol package` / `zymbol run pkg.zyp`)
- **Auto-free** — memory is released at a variable's last use, not at scope end; unobservable, lowers peak memory

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
# Full setup: https://zymbol-lang.github.io/aprende-zymbol/#/avanzado/05_herramientas
zymbol build program.zy -o myprogram --release

# Bundle a multi-file project into one portable .zyp archive (source, not a binary)
zymbol package myproject/ --script main.zy -o myproject.zyp
zymbol run myproject.zyp
```

> `build` and `package` are different things: `build` makes a native executable that embeds
> the interpreter; `package` makes a `.zyp` archive of source that still needs a `zymbol`
> binary to run — but works on any platform and stays readable.

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
| Collections | `$#` (len), `$+` (append), `$-` (remove), `$[..]` (slice), `$?` (contains), `$??` (find all), `$^+`/`$^-` (sort), `$^` (custom sort), `$>` (map), `$\|` (filter), `$<` (reduce) |
| Strings | `$~~[p:r]` (replace), `$/` (split), `$++` (build), `$*` (repeat N times) |
| TUI / Terminal | `@~` (sleep ms), `>>!` (clear screen), `>>?` (query size → `[rows,cols]`), `>>~` (positioned print), `<<\|` (blocking keypress), `<<\|?` (non-blocking keypress), `>>\|` (TUI block) |
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

The interpreter is a Rust workspace of 19 crates:

```
Foundation:   zymbol-span  zymbol-error  zymbol-common  zymbol-intrinsics
Frontend:     zymbol-lexer  zymbol-ast  zymbol-parser
Analysis:     zymbol-semantic
Tree-walker:  zymbol-interpreter
VM:           zymbol-bytecode  zymbol-compiler  zymbol-vm
Tooling:      zymbol-formatter  zymbol-analyzer  zymbol-lsp
              zymbol-repl  zymbol-standalone  zymbol-package
Entry point:  zymbol-cli
```

See [ARCHITECTURE.md](./ARCHITECTURE.md) for the full pipeline, data structures, and
performance benchmarks.

---

## Performance

Microbenchmarks (`zyquality/bench/`, release build, re-measured for v0.0.9,
best of 3, process startup subtracted):

| Benchmark | Tree-walker | VM | VM speedup |
|-----------|:-----------:|:--:|:----------:|
| Strings | 76ms | **51ms** | 1.4× |
| Collections | 71ms | **37ms** | 1.9× |
| Stress loop | 236ms | **78ms** | 3.0× |
| Match | 171ms | **54ms** | 3.1× |
| Recursion (`fib`) | 1566ms | **253ms** | 6.1× |

Those figures are arithmetic-bound and shallow. On real programs the gap is much
wider, because the tree-walker's cost is per call frame and per scope:

| Real workload | Tree-walker | VM | VM speedup |
|---------------|:-----------:|:--:|:----------:|
| zy-GO, full 19x19 game | 6m40s | **49.8s** | 8-14x |
| Chaturanga, `perft(3)`, 4448 nodes | 6.11s | **0.146s** | 42x |
| Chaturanga, full alpha-beta suite | 43.5s | **0.949s** | 46x |

(The go engine is [囲碁](https://github.com/zymbol-lang/zy-GO); the chess-ancestor
engine is चतुरङ्गम्.)

**Quote the workload, not a single number.** "~4×" circulated for a long time as
if it were the language's speedup; it is the low end of the microbenchmarks and
roughly a tenth of what a search-shaped program sees.

---

## Testing

**QA for this project lives in [ZyQuality](https://github.com/zymbol-lang/zyquality).**
The `.zy` corpus and its golden files are no longer in `tests/`: they were there
*and* in zyquality, and the two copies had drifted 28 files apart. The scripts
below keep their names, flags and exit codes and delegate to `zyq`. They exit
**2** if it is absent — a gate must not read "nothing ran" as "nothing failed".

```bash
git clone https://github.com/zymbol-lang/zyquality.git ../zyquality
make -C ../zyquality
```

```bash
# Unit tests (all 19 crates) — unaffected, these live inside the crates
cargo test

# Tree-walker vs VM parity          → zyq consensus --engines zytw,zyvm
bash tests/scripts/vm_compare.sh

# All four engines                  → zyq consensus
bash tests/scripts/engine_compare.sh
bash tests/scripts/engine_compare.sh loops/labels

# Golden expected-output tests      → zyq expect
bash tests/scripts/expected_compare.sh

# Semantic diagnostics E001–E013    → zyq expect --via check
bash tests/scripts/semantic_compare.sh

# Formatter properties — stays here: only this engine has a formatter.
# Reads the shared corpus plus this repository's examples/.
bash tests/scripts/fmt_property.sh --baseline tests/scripts/fmt_property_baseline.txt
```

Or ask the whole question at once, from the zyquality checkout:

```bash
./zyq suite     # selftest + audit + reject + goldens + consensus, one verdict
```

Current status (v0.0.9 branch — 0.0.8 is the latest published release),
re-measured 2026-08-12 against the unified corpus of **585 files**:

- **969 `#[test]` functions** across the 19 crates via `cargo test`.
- Tree-walker vs VM: **583 agree, 0 diverge**, 2 files excused for every engine
  (a re-export module, and an interactive tool that waits for a person). This is
  32 more files than the 551 the old runner saw: the corpus gained `arity/`,
  `loops/labels/` and zyml's 22-file smoke suite, and the 14 `input/` tests are
  compared now that the runner feeds each engine its `.input`.
- Golden files: **583/583 match, nothing unchecked**, via `zyq expect`. The two
  that used to fail were stale fixtures written before the output filter
  existed, not regressions; 47 more carried the path the corpus had when they
  were recorded and would have failed in any other checkout. All were
  re-recorded once, and `zyq` now strips the corpus root before comparing, so a
  golden says the same thing everywhere.
- Formatter properties: **627 PASS / 0 FAIL** over 682 files, 55 skipped.
- Benchmarks: **14/14 within tolerance** of the recorded baseline via
  `bench_gate.sh`. The programs moved to `../zyquality/bench/` — they print
  elapsed wall time, so they are not tests and never were; the only suite that
  had been running them was the browser parity runner, where all of them
  failed.

`ZYMBOL_BIN=/usr/bin/zymbol` still points the suite at an installed package
rather than the build tree. `VM_COMPARE_EXCLUDE` is gone — exclusions are
declared in `../zyquality/corpus.toml` and selected by tag:

```bash
bash tests/scripts/vm_compare.sh --without STD_DB
```

> **The parity number counts only versioned files.** During v0.0.8 this README said
> 544/544 while the release notes said 536/536, and both were "measured": the larger
> figure came from a working tree holding test files that `.gitignore` kept out of the
> repository, so a clean clone — and the `.deb` gate built from one — saw a different
> suite. The corpus now lives in `../zyquality/corpus/`, so re-derive it there, in a
> fresh clone, before quoting it in release notes — and disable git's path quoting when
> you do:
>
> ```bash
> cd ../zyquality
> comm -3 <(find corpus -name '*.zy' | sort) \
>         <(git -c core.quotePath=false ls-files 'corpus/**/*.zy' 'corpus/*.zy' | sort)
> ```
>
> Without `core.quotePath=false`, git escapes the non-ASCII names in `corpus/i18n/`
> (`中文_应用.zy`, `한국_앱.zy`, `עִברִית.zy`, …) as octal, and the comparison reports eight
> phantom differences on both sides while the counts match — a mismatch in the check, not
> in the repository.
>
> Moving the corpus removed the *other* half of this problem: there is now one copy, so
> "which corpus was measured" has one answer.

---

## Language-Driven Validation

Each release is validated by building a non-trivial application entirely in Zymbol, in a domain
the language has not been asked to serve before. The application is the test and **the language
is the unit under test**: it is written *as if the language already supported it*, and every
place it cannot say what it means — or says it and returns a silently wrong answer — is a
finding against the interpreter, not a defect in the program. Closing one means changing the
language: an operator derived under the [SYMBOLS.md](./SYMBOLS.md) rules, a semantic fix, a
TW/VM divergence, or a `std/` module.

This is validation, not verification, and the difference is the whole point. `cargo test`, the
parity runs and the golden files verify — they take a program that exists and check that the
implementation handles it correctly. None of them can report that the language cannot express a
Go board, because a missing capability produces no failing test; only writing the board does.
The cost is inverted from ordinary TDD: here the test is expensive and cannot be rerun, so a
project is a *discovery* mechanism and never a gate. Every finding is distilled into a minimal
`.zy` case, a golden and a unit test, and it is that cheap layer — not the application — that
protects against regression afterwards.

Each project keeps a **gap log**: every friction, bug, missing capability and idea, with an ID,
a reproduction and a status. The log closes against the release — 囲碁's eleven findings were
all fixed in v0.0.8, each with its own regression test. चतुरङ्गम्'s five are open against
v0.0.9. The method, its decalogue, and the index of the seven logs are in
**[LDV.md](./LDV.md)**.

The projects carry a second load at the same time. Each is written in a different natural
language — English, Mandarin Chinese, Spanish, Klingon pIqaD, Japanese, Sanskrit — which is what
turns "keyword-free means language-neutral" from a claim into a result: no flags, no special
modes, no translation layer at the syntax level. Sanskrit adds the case the earlier six could
not make: Devanagari is the first script where a single identifier needs combining marks to be
spelled at all, so `मन्त्री` and `अश्वः` are not "Unicode support" in the sense CJK was — they
are a grapheme cluster question that reaches the lexer, the analyzer and the display layer at
once.

### Summary

| Project | Version | Code language | What it put under test |
|---------|---------|---------------|------------------------|
| [ZethyCLI](https://github.com/zymbol-lang/zy-ZethyCLI) | **v0.0.3** | English | Modules, `<\cmd\>` shell exec, HTTP via Ollama, multi-turn state, string building |
| [ZyAudit](https://github.com/zymbol-lang/zy-ZyAudit) | **v0.0.4** | 中文 (Mandarin) | CJK identifiers as first-class citizens, named tuples, HOF pipeline, `$~~` replace |
| [Serpiente](https://github.com/zymbol-lang/zy-Serpiente) | **v0.0.5** | Español | TUI primitives, register VM, hot-definition `°`, tuple equality, labeled loops |
| [Hov veS](https://github.com/zymbol-lang/zyKlingonGalaxy) | **v0.0.5** | pIqaD (Klingon) | Multi-module orchestration, Galaxian formation AI, delta rendering, dual projectiles, 3-language i18n |
| [Zofía](https://github.com/zymbol-lang/zy-Zofia) | **v0.0.6** | Español | Scientific computing, transformer AI from scratch, `^` float exponents, global `:=` scope fix, `#.N\|x\|` formatting |
| [囲碁 (Igo)](https://github.com/zymbol-lang/zy-GO) | **v0.0.8** | 日本語 (Japanese) | Recursive flood fill at depth, state threading across modules, double-width glyph grid, application-level i18n in 5 languages, `std/term` |
| [चतुरङ्गम् (Chaturanga)](https://github.com/zymbol-lang/zyChaturanga) | **v0.0.9** | संस्कृतम् (Sanskrit) | Devanagari identifiers with conjuncts and visarga, alpha-beta search over make/unmake, mixed-script module names, numeral script as an i18n axis |

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

### 囲碁 (Igo) — v0.0.8 · 日本語 (Japanese)

The game of Go for the terminal, with an AI opponent, full rule enforcement (ko, suicide,
occupied points), and automatic area scoring. Written entirely in Japanese — every
identifier, module name and file name is kanji or kana, so the rule vocabulary in the
source is the vocabulary a player already knows (*komi*, *atari*, *ko*, *dame*, *jigo*).

It validates a different class of capability from the earlier games: a **large persistent
data structure** (up to 361 points) threaded through cooperating modules across isolated
function scopes, **recursive graph traversal** (group and liberty detection by flood fill),
a heuristic decision engine, and a **double-width glyph grid** where every cell is exactly
two terminal columns — the structural fix for the alignment class of bug found in Serpiente.

The UI ships in **five languages** (日本語 / 한국어 / 中文 / English / Español) with four
entry points that preselect one. This is application-level i18n — locale as module state,
measured layout, and a completeness gate — and it is the reference implementation behind
[USERAPPI18N.md](./USERAPPI18N.md).

Building it drove a substantial part of v0.0.8. Findings that became interpreter changes:

- **`std/term`** — the display-metrics module (`width`, `pad_left`, `pad_right`, `center`,
  `truncate`) plus `##!` over a `Char` (→ code point). It replaced a hand-written
  East-Asian width table inside the game — a differential test showed zero divergence
  against `unicode-width` on every glyph the game renders, and the layer that survives is
  a thin Japanese-named wrapper over the module.
- **Juxtaposition inside delimited positions** — `f(a " " b)`, `[a " " b]`, `(a " " b)`
  now concatenate as they always did at statement level. Parser-only; `BinaryOp::Concat`
  already existed.
- **Unused-variable false positive on ranges** — the analyzer treated `Expr::Range` as a
  no-op, so a variable used only as a loop bound (`@ i:1..総`) was reported unused, and
  non-deterministically so. Fixed in `variable_analysis.rs`.
- **VM correctness under module state** — output parameters (`<~`) of module functions
  were dropped, `String` was truncated inside a module, and `"{CONST}"` interpolation was
  compiled to literal text inside a function. Each was a silent wrong answer, not a crash.

```zymbol
// 核/盤.zy — group detection by flood fill (連 = chain, ダメ = liberty)
連(局面, 路, 起点) {
    色 = 局面[起点]
    ? 色 == 0 { <~ [] }
    訪問 = 新規(路)
    結果 = []
    _探索(局面, 路, 起点, 色, 訪問, 結果)
    <~ 結果
}

// output parameters carry the accumulator down the recursion
_探索(局面, 路, 点, 色, 訪問<~, 結果<~) {
    ? 訪問[点] == 1 { <~ 0 }
    ? 局面[点] <> 色 { <~ 0 }
    訪問[点] = 1
    結果 = 結果 $+ 点
    @ 隣点 : 隣(路, 点) { _探索(局面, 路, 隣点, 色, 訪問, 結果) }
    <~ 0
}
```

The project also ships an instrumented AI-vs-AI benchmark (`棋戦.zy`) that records one
reproducible game per file — the AI seeds an explicit LCG through an output parameter,
because `std/random` keeps hidden state and cannot be replayed. See
[BENCHMARK.md](https://github.com/zymbol-lang/zy-GO/blob/main/BENCHMARK.md) for what it
measures and for two documented misreadings of small samples.

---

### चतुरङ्गम् (Chaturanga) — v0.0.9 · संस्कृतम् (Sanskrit)

Chaturanga — the sixth-century Indian ancestor of chess — for the terminal, with the
historical rules, an alpha-beta opponent, and an interface in five languages. Written
entirely in Sanskrit: the game's own name is the compound **चतुर्-अङ्ग**, *four limbs*, and
the pieces are those four divisions of an army (पत्तिः foot, अश्वः horse, रथः chariot,
गजः elephant), so the vocabulary in the source is the vocabulary the game was first
described with.

It pushes on a different axis from 囲碁. Where the go engine proved that a **large** state
could be threaded through modules, this one proves that state can be **searched**: go offers
three hundred moves a position and search is hopeless, chaturanga offers about twenty and
alpha-beta is the right instrument. The board is never copied — every move is played and
taken back on one array — so `कृति`/`प्रत्यावर्तनम्` being exact inverses across thousands
of nodes is a language-level property the suite asserts directly.

Three capabilities were under test for the first time:

- **Devanagari identifiers.** Not "Unicode support" in the sense CJK already proved: `मन्त्री`
  and `अश्वः` need combining marks — virama, matras, visarga — to be spelled at all. They
  work unchanged in the lexer, the VM, `zymbol check` and the LSP, and a module name mixing
  two scripts (`# .भाषा_فارسی`, a Persian locale inside a Devanagari tree) checks clean.
- **Grapheme cluster ≠ display column, in a script where both vary.** `रा` is two graphemes
  and two columns, `र` is one of each, `कृ` is two graphemes and *one* column. `std/term`
  answers all three correctly, which is what lets one padding function hold a board where
  emoji, chess symbols and Devanagari letters share a grid.
- **The numeral mode as an i18n axis.** `#d0d9#` turns out to be process-global, not
  per-file, so a locale dispatcher can switch the digit script along with the language. The
  same line of drawing code yields `e४`, `e۴` and `e4`, and nothing below it knows which.
  This is a third i18n mechanism beyond the two in [I18N.md](./I18N.md), now written up in
  [USERAPPI18N.md](./USERAPPI18N.md) §14.

Its log held five entries, **all closed against v0.0.9**:

- **`@ <expr>` picked the loop form differently in each engine.** Logged as the `Bool` case,
  it turned out to be three: the VM decided from the *syntactic shape* of the specifier
  while the tree-walker, zyml and the JavaScript engine decided from the *value*, so
  `@ <Bool>` aborted under the VM and `@ f()` / `@ arr$#` looped forever there. A fourth,
  where the tree-walker was the odd one out: a negative `Int` count spun forever instead of
  running zero times. Fixed by fixing the rule — **an `Int` is a count, anything else is a
  condition** — with the VM asking at runtime (`IsInt` + `compile_adaptive_loop`, one body
  emitted for both paths) and the tree-walker dropping its `n > 0` guard.
  A second pass closed the other half: the *condition* path still read the specifier
  through truthiness, which no two engines agreed on — `@ []` ran zero times in the
  tree-walker, forever in the VM and raised in zyml. **A specifier is a count or a
  condition; anything else is refused at run time**, with one message across all four
  engines. `zyquality/corpus/loops/13_specifier_forms.zy` holds them to the forms that
  run and `zyquality/reject/loops/` to the ones that must not; there was no corpus file
  writing any of these forms before.
- **A range infers its direction, so `@ i:2..n` counts *down* when `n < 2`** instead of not
  iterating. The semantics were left alone — making the descending range empty would
  silence loops that currently run, and a bug that stops warning is worse than one that
  crashes. `zymbol check` now warns when a range's endpoints are not both integer literals,
  and stays quiet when an enclosing `?` already guards the bound.
- **GUIDE.md said the numeral mode persists "in the same file".** It is global to the
  process — which is the useful behaviour, and the one the game depends on. The guide says
  so now, and the technique is written up as a third i18n mechanism in
  [USERAPPI18N.md](./USERAPPI18N.md) §14, traps included.
- **The VM is 42–46× the tree-walker on this workload**, not the ~4× that had been quoted
  for years. A search is recursion with output parameters and array indexing in the
  innermost loop, which is where the tree-walker pays per frame. Every performance figure
  in this repository was re-measured; see [Performance](#performance).

```zymbol
// मूल/मतिः.zy — negamax with alpha-beta, scored from the mover's side.
// The board goes down as an output parameter and comes back untouched:
// every move is played and taken back, never copied.
गवेषणम्(स्थितिः<~, वर्णः, गभीरता, अल्फा, बीटा, सारणी, पदानि<~) {
    पदानि = पदानि + १
    ? गभीरता <= ० { <~ आ::मूल्याङ्कनम्(स्थितिः, वर्णः, सारणी) }

    चालाः = नि::वैधचालाः(स्थितिः, वर्णः)
    // मातः and गतिरोधः are both losses — this game keeps the old
    // shatranj rule, and it touches the search in exactly this line
    ? (चालाः$#) == ० { <~ ० - १०००० - गभीरता }

    रिपुवर्णः = अ::रिपुः(वर्णः)
    श्रेष्ठम् = ० - ९९९९९
    @ चालः : क्रमणम्(स्थितिः, चालाः) {
        अङ्गम् = स्थितिः[अ::आदिपदम्(चालः)]
        गृहीतम् = ०
        नि::कृति(स्थितिः, चालः, गृहीतम्)
        मूल्यम् = ० - गवेषणम्(स्थितिः, रिपुवर्णः, गभीरता - १,
                              ० - बीटा, ० - अल्फा, सारणी, पदानि)
        नि::प्रत्यावर्तनम्(स्थितिः, चालः, अङ्गम्, गृहीतम्)

        ? मूल्यम् > श्रेष्ठम् { श्रेष्ठम् = मूल्यम् }
        ? मूल्यम् > अल्फा { अल्फा = मूल्यम् }
        ? अल्फा >= बीटा { <~ श्रेष्ठम् }
    }
    <~ श्रेष्ठम्
}
```

```zymbol
// भाषा/प्रेषकः.zy — choosing a language chooses a digit script.
// One directive, and every number the program will print follows it.
निर्धारणम्(संकेतः) {
    वर्तमानाभाषा = संकेतः
    ?? संकेतः {
        "fa" => { #۰۹# }      // Extended Arabic-Indic, U+06F0 — not U+0660
        "en" => { #09# }
        "es" => { #09# }
        _    => { #०९# }      // Devanagari, U+0966
    }
    <~ ०
}
```

Six suites produce **byte-identical output under the tree-walker and the register VM**,
which is the property that makes the node counts meaningful: the search is deterministic, so
both engines must visit the same 16 / 272 / 1 646 nodes at levels 1 / 2 / 3 — a divergence
there would not show in the chosen move, because the choice is random among the survivors.

---

## Project Layout

```
interpreter/
├── Cargo.toml           # Workspace (19 crates)
├── zymbol-lang.ebnf     # Formal grammar (EBNF, v3.1.0)
├── install-zymbol.sh    # Install script
├── crates/              # Rust source crates
├── tests/               # End-to-end test suite (544 vm-compare files; 525 golden .expected pairs)
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
- [I18N.md](./I18N.md) — Internationalization: multilingual code via re-export layers, and runtime text via dispatcher modules
- [USERAPPI18N.md](./USERAPPI18N.md) — Building a multilingual application: measured layout, runtime language switching, per-language entry points, and the completeness gate
- [MEMORY_MODEL.md](./MEMORY_MODEL.md) — Memory and scoping model: design vs implementation audit (findings MM-1 … MM-11)
- [SYMBOLS.md](./SYMBOLS.md) — Semiotic and morphological reference: the grapheme inventory, how marks agglutinate into operators, the declared homographs and opaque signs, and the rules a new operator must satisfy
- [LDV.md](./LDV.md) — Language-Driven Validation: the method behind the validation projects, its decalogue, why validation is not verification, and the index of the seven gap logs
- [ROADMAP.md](./ROADMAP.md) — What's done, known gaps, and planned work
- [CHANGELOG.md](./CHANGELOG.md) — Version history

### Beyond this repository

- [zymbol-lang.org](https://zymbol-lang.org) — the website, with the manual in 110 languages
- [Playground](https://zymbol-lang.org/playground.html) — run Zymbol in the browser, no
  install; multi-file projects and `.zyp` packages included ([source](https://github.com/zymbol-lang/web))
- [Aprende Zymbol](https://zymbol-lang.github.io/aprende-zymbol/) — structured course from
  zero, in Spanish ([source](https://github.com/zymbol-lang/aprende-zymbol))
- [VS Code extension](https://github.com/zymbol-lang/vscode) — syntax highlighting, LSP
  client, 46 snippets, themes and file icons

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
