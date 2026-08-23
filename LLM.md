# Zymbol for LLMs — v0.0.9

Dense, executable-verified brief. Everything below was run through `zymbol 0.0.9` in
both engines. Canonical sources: `GUIDE.md` (tutorial), `REFERENCE.md` (limits + full
symbol table), `SYMBOLS.md` (why each mark has its shape).

**Zymbol has no keywords in any human language.** Every construct is a mark: `?` if,
`@` loop, `<~` return, `>>` output, `#1` true. Identifiers may be in any script
(Spanish, 日本語, العربية, pIqaD, emoji). `//` is the only comment form.

```bash
zymbol run f.zy        # tree-walker (default)   zymbol check f.zy   # parse+semantic, follows imports
zymbol run --vm f.zy   # register VM             zymbol fmt f.zy --write
zymbol run app.zyp     # .zyp bundle (--vm default)   zymbol repl
```

Three engines must agree: `zytw` (Rust tree-walker), `zyvm` (Rust register VM),
`zyjs` (browser, `web/src/zymbol/zymbol.js`). A fourth, `zyml` (OCaml), was
retired on 2026-08-17; it appears in this document only as history.

---

## 1. Fifteen rules that make code wrong while still parsing

1. **Indices are 1-based.** `a[1]` first, `a[-1]` last. Slices include **both** ends: `a$[1..3]` = 3 items.
2. **`>>` never adds a newline.** End with `¶` (or `\\`). `>> ¶` = blank line.
3. **Juxtaposition concatenates in `>>`; `+` does not.** `>> "n=" n ¶`. `"a" + b` is a type error.
4. **Unary minus loses to juxtaposition:** `>> "x=" -n ¶` parses as subtraction → error. Write `(-n)`.
5. **Functions see none of the caller's variables.** Isolated scope, not lexical fallback. Only top-level `:=` constants pierce it. Pass everything else as parameters.
6. **`??` is pattern matching, never a boolean chain.** An arm is *operator + value* (`< 0 =>`, `90..100 =>`), subject implicit. Booleans go through `?` / `_?` / `_`.
7. **Booleans are `#1` / `#0`. There is no null.** Absence is `##_` (Unit).
8. **`==` never coerces; ordering does.** `"5" == 5` → `#0`, but `"5" > 4` → `#1` (numeric text in any of 69 digit scripts).
9. **`/` divides, `$/` splits.** No overlap.
10. **A variable never read is a warning.** Prefix `_` when deliberate: `_unused`, `@ _i:1..3`.
11. **`$>` with a named function takes NO parentheses.** `nums$> double` ✓ — `nums$> (double)` is a parse error, because `(` opens a lambda. `nums$> (x -> double(x))` ✓.
12. **`<~` on a parameter is written at the signature AND the call site**, and required in both: `bump(b<~)` → `bump(y<~)`. `bump(y)` is a semantic error. A `<~` slot needs a variable, never an expression.
13. **The last name of a destructuring pattern absorbs the remainder** (see §7). `(a,b,c) = (1,2,3,4,5)` gives `c = (3,4,5)`.
14. **Bracket shape is typed:** `[…]` destructures only arrays, `(…)` only tuples. `>>?` returns a **tuple** → `(H, W) = >>?`.
15. **`Int` is ±(2⁵³−1), fail-closed.** Overflow is a catchable `##Range` error, never a wrap, never a promotion to Float.

---

## 2. Types

| Type | Literal | `#?` symbol | Notes |
| --- | --- | --- | --- |
| Int | `42`, `0x41`, `0b1010`, `0o17`, `0d99` | `###` | safe integer ±(2⁵³−1); `/` on two Ints is integer division |
| Float | `3.14`, `1.0e10` | `##.` | IEEE-754 double; overflow → `inf` (a value, not an error); `==` exact; NaN false in every direction; prints as digits, never exponent |
| String | `"hi"`, `"Hi {name}"` | `##"` | interpolation works everywhere; `\{`/`\}` escape braces |
| Char | `'A'`, `'↑'` | `##'` | |
| Bool | `#1` / `#0` | `##?` | `#0 < #1` |
| Array | `[1,2,3]` | `##]` | homogeneous by design |
| Tuple | `(10, 20)` | `##)` | immutable; `t[1]` |
| NamedTuple | `(name:"Ana", age:25)` | `##)` | `t.name`, `t[2]` |
| Function / Lambda | `f(a){}` / `x -> x*2` | `##()` / `##->` | first-class; `count` = arity |
| Error | — | `##Kind` | the kind *is* the type symbol |
| Unit | — | `##_` | empty absorption, no-value |

Not values: ranges (`1..5`, loop headers only) and module aliases.
`x#?` → `(type_symbol, count, display)`. `#|"४२"|` → `42` (69 digit scripts, fail-safe:
returns the input unchanged on failure). Casts: `##.x` Float, `###x` Int (round),
`##!x` Int (truncate; `Char`→code point). Format: `#.2|x|` round, `#!2|x|` truncate,
`#,|x|` commas, `#^|x|` scientific.

---

## 3. Variables, scope, lifetime

```zymbol
x = 10            // mutable; assignment to a name visible outside updates it (no shadowing)
PI := 3.14        // constant; top-level ones are global and readable inside any function
_tmp = 1          // '_' prefix = exact block scope: invisible to inner AND outer blocks
\ x               // explicit destruction; use after this is a lifetime error
x += 5   x++      // also -= *= /= %= ^= --
```

Blocks (`?`, `@`, `!?`, …) are lexical scopes; a block-local dies at `}`. Functions are
frames. Auto-free releases a value right after its last use — invisible, never observable.

**Hot definition `°`** auto-initialises on first use to the neutral value of the operation
(`0`/`0.0` for `+= -=`, `1` for `*= /=`, `[]` for `$+`, `""` for juxtaposition):

```zymbol
@ item:[10,20,30] { °total += item }   // °x (prefix): anchors ABOVE the loop, survives it
>> total ¶                             // → 60
@ i:[1,2,3] { i° += 1 }                // x° (postfix): dies with the loop
```

---

## 4. I/O

```zymbol
>> "a=" a " b=" b ¶          // juxtaposition; postfix on a NAME needs no parens: >> "len=" arr$# ¶
                             // postfix on a literal does not parse: [1,2]#? → bind it first
>> "eq=" (a == b) ¶          // parenthesised expressions are single items
<< name                      // read
<< "Name: " name             // with prompt
<< ###(4) "n: " n            // typed input, re-prompts until valid:
                             //   ##. float | ##.(T,D) decimal | ###(N) int | ##"(N) text | ##' char
>< args                      // CLI args → string array
```

TUI: `@~ 500` sleep ms · `>>!` clear · `(H,W) = >>?` size (`(24,80)` with no tty) ·
`>>~ (row,col,BKS,fg,bg) > "text"` positioned/styled (any slot omissible: `>>~ (,,,196) > "red"`) ·
`<<| k` blocking key · `<<|? k` polling (`'\0'` if none) · `>>| { }` alternate screen + raw
mode (errors without a tty; `<<|` needs it). Arrows arrive as `'↑' '↓' '←' '→'`.

Shell: `out = <\ "ls {dir}" \>` (stdout+stderr, trailing `\n` stripped).
Sub-script: `out = </ ./sub.zy />`.

---

## 5. Control flow

```zymbol
? x > 100 { … } _? x > 0 { … } _ { … }      // braces always required
```

`??` match — six pattern kinds, first match wins, `||` joins alternatives of any kinds:

```zymbol
g = ?? score {
    90..100 => 'A'            // range
    == 0 || < 0 => "none"     // comparison (< > <= >= == <>), or-chained
    weekdays => "wk"          // ident: scalar → equality, array variable → containment
    [1, 2] => "low"           // list: array subject → structural; scalar subject → containment
    ["run", _] => { … }       // structural with wildcard; block arm allowed
    _ => 'F'                  // wildcard
}
```

No binding patterns (`n => n * 2` is not implemented).

---

## 6. Loops — `@` is every loop; the header decides the kind

```zymbol
@ i:1..3 { }        // inclusive range        @ i:1..9:2 { }   // step
@ i:3..1:1 { }      // descending comes from the bounds; a negative step is a runtime error
@ f:fruits { }      // for-each               @ c:"hola" { }   // per character
@ n <= 64 { }       // while — Bool specifier, re-evaluated
@ 5 { }             // times — Int specifier, evaluated ONCE; ≤0 runs zero times
@ items$# { }       // any Int expression is a count
@ { }               // infinite
```

A specifier that is neither Int nor Bool (array, float) is a runtime error in all four
engines — there is no truthiness. `@!` break, `@>` continue. Labels: `@:outer i:1..4 { … @:outer! … @:outer> }`.
Both are checked statically: they need an enclosing loop, a label must resolve to an
**ancestor** (a sibling is an error), and a function/lambda body is a hard boundary.

Two warnings are normal here and neither is an error:

- `ambiguous lifetime for 'i'` — emitted for **every** bare iterator name. Prefix it
  (`@ _i:1..3`) when it is not needed after the loop; that silences it.
- `range direction is decided at runtime` — emitted when a bound is not a literal
  (`1..MAX`): if the end turns out lower than the start the loop counts **down** instead
  of not running. Guard the empty case yourself.

---

## 7. Collections

```zymbol
a$#            length            a$+ v      append (returns new)     a$+[2] v   insert at 2
a$- v          remove first      a$-- v     remove all               a$-[1]     remove index
a$-[2..3]      remove range      a$-[2:2]   remove count-based       a$? v      contains
a$?? v         all indices       a$[1..3]   slice (both ends)        a$[1:2]    slice by count
a$^+  a$^-     sort asc/desc     a$^ (x,y -> x.f < y.f)              // comparator sort
a[2]$~ 99      the ONE update form. `a[2] = 99` does not exist, in any collection
               result used   → builds, original untouched:  b = a[2]$~ 99
               result thrown → modifies in place:            a[2]$~ 99
```

**Deep access / update — `>` navigates, and is the intended form; `a[i][j]` is deprecated:**

```zymbol
m[2>3]              // scalar at row 2, col 3        m[row>col]   m[-1>-1]   m[(n)>(n)]
m[1>2]$~ 99         // deep functional update → new matrix
m[[2>3]]            // flat extraction → [6]         m[1>1 ; 2>3]        → [1, 6]
m[[1>1] ; [2>3]]    // structured → [[1], [6]]       m[[1>2..3]]  range on a step
```

**Destructuring** — the pattern's bracket shape is enforced, and the **last name absorbs
the remainder**, taking the container's shape, or `##_` when nothing is left:

```zymbol
(a, b, c) = (1,2,3,4,5)    // c = (3,4,5)      [a, b, c] = [1,2,3,4,5]   // c = [3,4,5]
(a, b, c) = (1,2)          // c = ##_          (a, b, c) = (1,2,3)       // c = 3  (scalar)
(a, b, *c) = (1,2,3)       // c = (3) — '*' forces a collection, always
[solo] = [1,2,3]           // solo = [1,2,3] — a single name absorbs too
(name: n, age: y) = person // named-tuple destructuring
```

Strings share the `$` operators, plus: `s$~~["l":"L"]` replace all, `s$~~["l":"L":1]` first N,
`s$/ ','` split (char or substring), `s$* 3` repeat, `"x=" $++ n " y=" m` build.

---

## 8. Functions, lambdas, HOF

```zymbol
add(a, b) { <~ a + b }              // named; scope is isolated
double = x -> x * 2                 // lambda; captures scope BY VALUE at creation
sum2   = (a, b) -> a + b
thunk  = () -> 42
block  = x -> { ? x > 0 { <~ "pos" }  <~ "neg" }

work(p~)  { p = p * 2 }             // '~'  working copy — caller untouched
bump(b<~) { b = b + 100 }           // '<~' output parameter — writes back
bump(y<~)                           // mark required at the call site too (also m::f(x<~))
```

Use a tuple return when the values are new (`(v, next) = step(3)`); use `<~` parameters
when the caller already owns the variables. A named function used as a value captures the
scope at the point of assignment.

```zymbol
nums$> (x -> x * 2)          // map      | named fn: nums$> double  (NO parentheses)
nums$| (x -> x % 2 == 0)     // filter
nums$< (0, (acc,x) -> acc+x) // reduce (initial, lambda)
5 |> double |> inc           // pipe; explicit slot when not first: 10 |> add(_, 5)
```

`$>`/`$|`/`$<` chain directly: `nums$| (x -> x > 2)$> (x -> x * 10)$< (0, (a,x) -> a+x)`.

---

## 9. Errors

```zymbol
!? { v = 10 / 0 } :! ##Div { >> "div" ¶ } :! { >> _err ¶ } :> { >> "finally" ¶ }
```

`!?` try · `:!` catch (bare or typed) · `:>` finally · `_err` holds the caught error ·
`v$!` is-error · `v$!!` propagate to caller.

Kinds: `##Div` `##Index` `##Type` `##Range` (numeric limits) `##Parse` `##IO` `##Network`
`##DB` `##_`. Std-library environmental failures come back as **soft error values** to test
with `$!`, not as raised errors; type/arity mistakes raise.

---

## 10. Modules

One file, one closed block. Only imports, the export block, literal-initialised bindings
and function definitions may appear in a module body — any executable statement is **E013**.
A collection literal counts as a literal (`tabla = #(es: "hola", en: "hi")`), recursively;
anything that computes does not.

```zymbol
// lib.zy
# lib {
    <# ./dep => d              // imports first
    #> { add, PI, get, internal_fn => public_name, d::helper, d.CONST => TAU }
    PI    := 3.14              // exported constant (literal RHS)  → alias.PI
    count = 0                  // private state, persists across calls, never exported
    add(a, b) { <~ a + b }
    get() { <~ count }
}
```

```zymbol
<# ./lib => L        // relative: ./ ../ ; alias mandatory
<# std/math => M
>> L::add(2,3) ¶     // :: calls a function
>> L.PI ¶            // .  reads a constant
```

Module state identity is **per file path**: every alias and every importer shares one
state. Modules never see the importing script's constants. A dotted module name
(`# .sub_file`) is the convention for subdirectories. Translating a module = a re-export
layer (`_j::decode => decodificar`).

Stdlib (same module system, native): `std/math` (`sqrt exp ln log pow abs ceil floor round
min max sin cos tan asin acos atan atan2 sinh cosh tanh sigmoid`, `PI` `E`) · `std/random`
(`entero rango peso_f64`) · `std/json` (`decode decode_map encode`) · `std/io` (`read write
append exists delete list mkdir`) · `std/net` (`get post post_json head`) · `std/db`
(`connect exec query query_one query_value tx begin commit rollback …`) · `std/term`
(`width pad_left pad_right center truncate` — **display columns**, not graphemes: CJK and
most emoji are 2 columns, so lay TUI out with `t::width`, never `$#`) · `std/time`
(`now today parts of format add diff`).

`std/time`: an instant is **milliseconds** since the epoch, always UTC; a date is a
*reading* of one, so every function takes an optional trailing zone — `"UTC"` (default),
`"local"`, or `"+1000"`/`"-0400"`. `of(y, m, d [, h, mi, s])` builds one, `parts` reads one
into a dictionary (`year month day hour minute second millisecond weekday offset`, weekday
1 = Monday), `format` renders POSIX codes (`%Y %m %d %H %M %S %L %j %u %z %F %T %%`) in
**ASCII digits whatever the numeral mode** — a localized date is built from `parts` instead.
`add`/`diff` take a unit in full (`millisecond second minute hour day week month year`):
below a day it is duration, from a day up it is calendar, so a month lands on the same day
of the month (clamped: 31 Jan + 1 month = 28 Feb) and a day across a daylight-saving change
is still a day. A date that does not exist is a soft `##Time`, not a crash.

---

## 11. Numeral modes

`#०९#` switches the digit script used for **output** process-wide (69 scripts: Devanagari,
Arabic-Indic, Thai, pIqaD, …); `#09#` restores ASCII. It affects printed digits and
booleans (`#१` / `#०`), not arithmetic. A function that switches must hand ASCII back
before returning.

---

## 12. Reference program

```zymbol
<# std/math => M

MAX := 20

classify(n) {
    ? n % 15 == 0 { <~ "FizzBuzz" }
    _? n % 3 == 0 { <~ "Fizz" }
    _? n % 5 == 0 { <~ "Buzz" }
    <~ n
}

tally(hits<~, label) {
    ? label == "Fizz" { hits = hits + 1 }
}

fizz = 0
@ _i:1..MAX {                   // '_i': iterator not needed after the loop
    label = classify(_i)
    tally(fizz<~, label)
    >> label " "
}
>> ¶
>> "fizz=" fizz " root=" M::sqrt(2.0) ¶

nums = [3, 1, 4, 1, 5]
>> (nums$^+) " " (nums$> (x -> x * 2)) " " (nums$< (0, (a,x) -> a + x)) ¶

!? {
    _v = nums[99]
} :! ##Index {
    >> "out of range" ¶
}
```
