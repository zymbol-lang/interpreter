# Zymbol Symbol Semantic Map

> **Purpose:** This document defines the *abstract meaning* of every symbol in Zymbol —
> the underlying semantic intent that makes the grammar internally consistent.
>
> The MANUAL documents *what each operator does*. This document documents *why each symbol
> was chosen* and *what contract each symbol carries* across all its uses.

---

## Core Principle

Zymbol has no keywords. Every construct is expressed through symbols. For the symbol system
to be learnable, each base symbol must carry one consistent abstract meaning that holds
across all contexts where it appears. A programmer learns a symbol's character once and
recognizes it everywhere.

This is not coincidence — it is a design invariant. Every new operator must be derivable
from existing symbol meanings, or introduce a new base symbol with a documented character.

---

## The Symbol Families

### `?` — Uncertainty / Query / Maybe

`?` marks a position of **conditional or uncertain outcome**. The result may or may not
hold; the program is *asking a question* rather than stating a fact.

| Symbol | Operation | Why `?` |
|--------|-----------|---------|
| `?` | if (conditional) | executes only if condition holds — uncertain path |
| `_?` | else-if | continues the uncertain chain |
| `??` | match | queries which pattern holds |
| `$?` | contains | asks "is this value present?" → Bool |
| `$??` | find all indices | asks "where is this value?" → positions or empty |
| `x#?` | type metadata | asks "what is this value at the meta level?" |
| `!?` | try block | the block may or may not throw |
| `<<|?` | key read non-blocking | asks "is a key available right now?" → `''` if not |

**Contract:** wherever `?` appears, the result is *conditional* — the outcome depends on
a runtime question that may return false, empty, or uncertain.

---

### `@` — Time / Repetition / Loop Context

`@` marks **temporal constructs** — things that happen over time, repeat, or control the
pace of execution. All `@`-family statements are **only valid inside a loop block**.
Using them outside a loop is a semantic error (same constraint as `<~` inside functions).

| Symbol | Operation | Why `@` |
|--------|-----------|---------|
| `@ { }` | infinite loop | time passes indefinitely |
| `@ N { }` | repeat N times | time passes N iterations |
| `@ cond { }` | while loop | time passes while condition holds |
| `@ x:arr { }` | for-each loop | time passes over each element |
| `@:label` | labeled loop | named point in time |
| `@!` | break | cut time short — exit the loop |
| `@>` | continue | skip forward in time — next iteration |
| `@:label!` | labeled break | cut a named time context short |
| `@:label>` | labeled continue | skip forward in a named time context |
| `@~` | sleep N ms | **pause** time for N milliseconds |

**Contract:** every `@`-prefixed statement operates *within* a temporal context (a loop).
`@!`, `@>`, and `@~` are all semantic errors if used outside a `@` block.
The shape `@X` always means "act on the current time context in way X".

```zymbol
// Example: wait exactly one minute then break
@:timer {
    @~ 60000       // pause 60 000ms — valid only inside @:timer
    @:timer!       // labeled break — also valid only inside @:timer
}
```

---

### `>>` / `<<` — Directional Flow (Out / In)

`>` and `<` encode **direction**. Doubled (`>>`, `<<`), they become *strong flow* — a full
stream moving in one direction. Modified with other symbols, they specify *what* flows and
*how*.

| Symbol | Operation | Why `>>` / `<<` |
|--------|-----------|-----------------|
| `>>` | output (print) | data flows *outward* to the terminal |
| `<<` | input (read line) | data flows *inward* from stdin, line by line |
| `<<|` | read single key (blocking) | one character flows in — waits until it does |
| `<<|?` | read single key (non-blocking) | asks if one character is available *right now* |
| `<#` | module import | a module flows inward into scope |
| `#>` | module export | symbols flow outward from the module |
| `<~` | return / output param | value flows back to the caller |
| `<\ \>` | BashExec | shell output flows inward as a string |
| `</ />` | script exec | another script's output flows inward |

**Contract:** `<` = something flows toward the program; `>` = something flows away from
the program. The second symbol specifies the *medium*: `<` (line), `|` (single char/gate),
`#` (module namespace), `~` (function return channel), `\` (shell).

#### Typed input — a cast marker on the read

A cast symbol placed right after `<<` (before the prompt) constrains and converts the
value as it is read, re-prompting until it is valid: `<< <typespec> "prompt" var`. This
reuses the type/cast symbols rather than inventing new ones — the constraint *is* the
type, with an optional size in parentheses.

| Form | Reads → | Notes |
|------|---------|-------|
| `<< ##.(T,D) "p" v` | `Float` | decimal, ≤T digits total, ≤D after the point |
| `<< ##. "p" v` | `Float` | any number |
| `<< ###(N) "p" v` | `Int` | ≤N digits |
| `<< ##"(N) "p" v` | `String` | ≤N characters |
| `<< ##' "p" v` | `Char` | exactly one character |

The size argument is the one concession to "named arguments" on an input flow; everything
else stays pure symbol. Validated identically in the tree-walker and the VM.

#### `<<|` vs `<<|?` — the blocking distinction

| Form | Meaning | Returns |
|------|---------|---------|
| `<<\|` | "give me a key" — certain, blocks until pressed | `Char` |
| `<<\|?` | "is there a key?" — uncertain, returns immediately | `Char` or `''` |

The `?` suffix follows its family contract: it converts a definite operation into a
conditional query. Compare: `$+` (append, definite) vs `$?` (contains?, uncertain).

---

### `#` — Meta / Type Level

`#` operates **above the value level** — on types, modules, numeric bases, and output
scripts. When you see `#`, the operation is about *what something is*, not *what value
it holds*.

| Symbol | Operation | Why `#` |
|--------|-----------|---------|
| `#1` / `#0` | Boolean true / false | typed truth values — not integers |
| `# name` | module declaration | names the *meta-identity* of the file |
| `#>` | export block | declares the module's public *meta-surface* |
| `<#` | import | pulls a module's meta-surface into scope |
| `##type` | type symbol | the meta-name of a type (`###`, `##.`, `##"`, etc.) |
| `x#?` | type metadata | query the meta-level of a value |
| `##.` | cast to Float | cross the type boundary toward Float |
| `###` | cast to Int (round) | cross the type boundary toward Int |
| `##!` | cast to Int (truncate) | cross the type boundary, truncating |
| `#.N\|x\|` | round N decimals | meta-precision on the value |
| `#!N\|x\|` | truncate N decimals | meta-truncation on the value |
| `#,\|x\|` | comma format | meta-display format |
| `#^\|x\|` | scientific notation | meta-display format |
| `#\|x\|` | numeric eval | evaluate a string at the meta-numeric level |
| `#d0d9#` | numeral mode | switch the meta-display script for all numbers |

**Contract:** `#` always signals a boundary crossing — from value-space to type-space,
from runtime to display representation, or from file to named module.

---

### `$` — Collection Operation

`$` is the **collection operator prefix**. Every `$`-family symbol applies an operation
to a collection (array or string). The symbol after `$` specifies the operation's nature.

| Symbol | Operation | Why that suffix |
|--------|-----------|----------------|
| `$#` | length | `#` = meta → the meta-count of the collection |
| `$+` | append | `+` = addition |
| `$+[i]` | insert at index | `+` = addition at `[i]` |
| `$-` | remove first | `-` = subtraction by value |
| `$--` | remove all | `--` = complete subtraction |
| `$-[i]` | remove at index | `-` = subtraction at `[i]` |
| `$?` | contains | `?` = uncertain query |
| `$??` | find all indices | `??` = deeper uncertain query |
| `$[i..j]` | slice | `[..]` = sub-range |
| `$^+` | sort ascending | `^` = elevate; `+` = forward |
| `$^-` | sort descending | `^` = elevate; `-` = reverse |
| `$^` | sort (comparator) | `^` = elevate with custom order |
| `$>` | map | `>` = transform each element outward |
| `$\|` | filter | `\|` = gate — only pass elements that qualify |
| `$<` | reduce | `<` = collapse everything inward to one value |
| `$~~[p:r]` | string replace | `~~` = transform/modify within |
| `$/` | string split | `/` = divide into parts |
| `$++` | concat-build | `++` = accumulate/grow |
| `$!` | is error | `!` = force-check for error state |
| `$!!` | propagate error | `!!` = force-eject the error upward |
| `arr[i]$~` | functional update | `$~` = collection-modify at index → new copy |

**Contract:** `$` always operates *on* a collection (left-hand side) and *returns* a new
value (new collection, scalar, or Bool). The symbol after `$` encodes the operation's
semantic character using the same base symbols as the rest of the language.

---

### `~` — Modification / Transformation

`~` marks an operation that **modifies or transforms** its target — either changing a
value in place (semantically), or routing a value back through a modified channel.

| Symbol | Operation | Why `~` |
|--------|-----------|---------|
| `<~` | return / output param | value flows back *modified* to the caller |
| `param~` | mutable parameter | the parameter is a *modifiable* copy of the argument |
| `$~~[p:r]` | string replace | the string is *transformed* by replacement |
| `arr[i]$~` | functional update | the collection is *transformed* at index i |
| `@~` | sleep N ms | the time flow is *paused/modified* |

**Contract:** `~` signals that something is being *changed* — a value returned and
altered, a parameter that flows back to the caller, a collection transformed in-place.
It is **not** about creating new things — it transforms existing ones.

---

### `!` — Force / Negation / Error

`!` marks **forceful, definitive, or error-related operations**. It asserts that something
must happen, must not hold, or signals that an error state is present.

| Symbol | Operation | Why `!` |
|--------|-----------|---------|
| `!` | logical NOT | negates / inverts |
| `@!` | break | *forces* exit from the loop |
| `!?` | try block | *force-attempts* a risky operation |
| `:!` | catch clause | captures the *forced error* |
| `$!` | is error | checks if a value *is* in error state |
| `$!!` | propagate error | *forces* the error upward to the caller |
| `##!` | cast to Int (truncate) | *forcefully* truncates toward zero |
| `#!N\|x\|` | truncate N decimals | *forcefully* cuts precision |

**Contract:** `!` either *forces* an outcome (break, propagate, truncate) or *tests* for
a forced/error condition. It never leaves things uncertain — it acts decisively.

---

### `|` — Gate / Flow Filter

`|` represents a **gate** — something that controls what flows through. A gate either
passes all (open), passes conditionally (filter), or holds until ready (blocking gate).

| Symbol | Operation | Why `\|` |
|--------|-----------|---------|
| `\|\|` | logical OR | true if *either* side passes the gate |
| `\|>` | pipe | value passes *through* a function gate |
| `$\|` | filter | only elements that pass the gate continue |
| `#,\|x\|` | comma format | `\|...\|` = boundary fences enclosing the value |
| `#^\|x\|` | scientific format | same boundary fence pattern |
| `#.N\|x\|` | round | same boundary fence pattern |
| `<<\|` | read single key (blocking) | *one* character passes through the gate |
| `<<\|?` | read single key (non-blocking) | gate queries if a character is available |

**Contract:** `|` controls passage. In format expressions, `|...|` acts as a fence
containing the value. In `$|` and `|>`, it gates the flow of data. In `<<|` and `<<|?`,
it narrows the flow of input from lines down to single characters.

---

### `_` — Non-Binding / Void / Wildcard

`_` marks a position that **is intentionally left unbound** — a slot that exists
syntactically but is not captured or matched.

| Symbol | Operation | Why `_` |
|--------|-----------|---------|
| `_` | else branch | the default non-binding case |
| `_?` | else-if | extends the non-binding chain |
| `_` | match wildcard | catches everything, binds nothing |
| `[a, _, c]` | destructure ignore | middle element not captured |
| `_x` | unused variable prefix | identifier declared but not used (suppresses warning) |
| `_` | pipe placeholder | marks where the piped value is injected |

**Contract:** `_` always means "this position exists but I am not binding it".
It never introduces a name into scope.

---

### `:` — Definition / Binding / Label

`:` establishes a **named relationship** — between an identifier and a value, a function
and its body, or a loop and its label.

| Symbol | Operation | Why `:` |
|--------|-----------|---------|
| `:=` | constant declaration | defines an immutable binding |
| `::` | module function call | calls through the named module binding |
| `@:label` | labeled loop | *names* the loop for targeted break/continue |
| `@:label!` | labeled break | targets the named loop |
| `@:label>` | labeled continue | targets the named loop |
| `:!` | catch clause | binds the error type being caught |
| `:>` | finally clause | defines the cleanup binding |
| `name: value` | named tuple field | names a field within the tuple |
| `@ i:arr` | for-each loop variable | *names* the iteration variable |
| `1..10:2` | range step | *names* the increment |
| `$~~[p:r]` | string replace separator | *names* replacement vs pattern |
| `$[i:n]` | count-based slice | *names* start:count bounds |

**Contract:** `:` always introduces or references a *name* — a definition relationship
between a symbol and what it stands for. Alias and rename operations now use `=>`
(which adds the outward direction: the name *maps toward* the consumer).

---

### Resolved: `<=` dual-role retired (v0.0.5)

`<=` is now used **exclusively** as the less-than-or-equal comparison operator.
All module alias and export rename uses of `<=` have been replaced with `=>`:

```
<# ./math => m          // import math, named m
#> { _add => sum }      // export _add as public name sum
#> { other::func => alias }
```

(The v0.0.5 development cycle went through an intermediate step using `:` before settling
on `=>`. That history is documented in `IMPL_V005.md §Design history`.)

---

### `=>` — Maps To / Becomes / Is Exported As

`=>` marks a **renaming or mapping relationship** — the left side is known internally
by one name, but is expressed, matched against, or exported under the right side.

| Symbol | Operation | Why `=>` |
|--------|-----------|---------|
| `?? x { pat => val }` | match arm separator | pattern *maps to* result |
| `<# ruta => alias` | import alias | module *becomes known as* alias |
| `#> { fn => pub }` | export rename | internal name *becomes* public name |

**Contract:** `=>` always describes a transformation of identity — one name or pattern
becomes another in the consumer's view. `=` encodes the mapping/equality relationship;
`>` encodes the outward direction (the mapping resolves toward the consumer).
This completes the arrow family alongside `->` (into body) and `<~` (back to caller).

---

### `->` and `<~` — Lambda and Return Arrows

These encode the **directionality of function flow**.

| Symbol | Operation | Character |
|--------|-----------|-----------|
| `->` | lambda / function body | "body comes *after* this" — defines the flow |
| `<~` | return / output param | value flows *back* to the caller |

**Contract:** `->` points *into* the function body; `<~` points *back out* to the caller.
Together they form the entry and exit of function boundaries.

---

### `.` — Access / Decimal / Depth Separator

`.` means **step into** — accessing a sub-part of a structure.

| Symbol | Operation | Why `.` |
|--------|-----------|---------|
| `tuple.field` | named tuple access | *step into* the tuple's field |
| `module.CONST` | module constant access | *step into* the module's constant |
| `3.14` | float literal | *decimal point* — the fractional part |
| `1..5` | range | *step from* start *to* end |
| `arr[i>j]` | nav-index depth | `>` is the depth separator in nav context |

**Contract:** `.` (and `..`) always means "step deeper into" — into a structure's member,
into the decimal component of a number, or across a range.

---

## New Symbols Introduced in v0.0.5

These symbols shipped in v0.0.5 and are fully implemented (tree-walker + VM).

| Symbol | Family | Operation | Constraint |
|--------|--------|-----------|------------|
| `<<\|` | `<<` + `\|` | Read one key — blocks until pressed | Only in statement position |
| `<<\|?` | `<<` + `\|` + `?` | Read one key — returns `''` immediately if none | Only in statement position |
| `@~` | `@` + `~` | Pause execution for N milliseconds | **Only inside a `@` loop block** |

### `@~` loop-context constraint

`@~` joins `@!` and `@>` in the family of statements that are **semantically illegal
outside a loop**. The `@` prefix marks all three as "time-context operations":

| Statement | What it does to time | Valid context |
|-----------|---------------------|---------------|
| `@!` | cuts time short (break) | inside `@` block only |
| `@>` | skips forward in time (continue) | inside `@` block only |
| `@~` | pauses time for N ms (sleep) | inside `@` block only |

```zymbol
// Wait 1 minute, then break — all three @-family members in context
@:timer {
    @~ 60000      // pause 60 000ms
    @:timer!      // labeled break
}
```

Using `@~` outside a loop is a semantic error, same as `@!` outside a loop.

---

## Occupied Symbol Combinations Reference

Use this table when designing new operators to avoid conflicts.

### `=>` (maps to / renames as)
| Used | Meaning |
|------|---------|
| `=>` | match arm separator, import alias, export rename |



### `<<` prefix (flow inward)
| Used | Meaning |
|------|---------|
| `<<` | read line (input) |
| `<<\|` | read key blocking |
| `<<\|?` | read key non-blocking |

### `>>` prefix (flow outward)
| Used | Meaning |
|------|---------|
| `>>` | output / print |

### `@` prefix (time / loop context)
| Used | Meaning |
|------|---------|
| `@` | loop |
| `@!` | break |
| `@>` | continue |
| `@:label` | label |
| `@:label!` | labeled break |
| `@:label>` | labeled continue |
| `@~` | sleep |

### `#` prefix (meta / type)
| Used | Meaning |
|------|---------|
| `#` | module declare |
| `#>` | export |
| `<#` | import |
| `#1` / `#0` | bool literals |
| `##` | type symbols / casts |
| `#.N` | round |
| `#!N` | truncate |
| `#,` | comma format |
| `#^` | scientific |
| `#\|` | numeric eval |
| `#d0d9#` | numeral mode |

### `$` prefix (collection)
| Used | Meaning |
|------|---------|
| `$#` | length |
| `$+` / `$+[i]` | append / insert |
| `$-` / `$--` / `$-[i]` / `$-[i..j]` / `$-[i:n]` | remove variants |
| `$?` / `$??` | contains / find all |
| `$[..]` / `$[i:n]` | slice variants |
| `$^` / `$^+` / `$^-` | sort variants |
| `$>` | map |
| `$\|` | filter |
| `$<` | reduce |
| `$~~` | string replace |
| `$/` | string split |
| `$++` | concat-build |
| `$!` / `$!!` | error check / propagate |
| `$~` (postfix on index) | functional update |

### `~` (modification)
| Used | Meaning |
|------|---------|
| `<~` | return / output param |
| `param~` | mutable parameter |
| `$~~` | string replace (as `$` operator) |
| `$~` | functional update (as `$` operator) |
| `@~` | sleep — time modification |

### `!` (force / error)
| Used | Meaning |
|------|---------|
| `!` | logical NOT |
| `@!` | break |
| `!?` | try |
| `:!` | catch |
| `$!` | is error |
| `$!!` | propagate error |
| `##!` | cast to Int (truncate) |
| `#!N` | truncate N decimals |

---

## Design Rules for New Operators

1. **Derive, don't invent.** A new operator must be explainable as a combination of
   existing symbol meanings. `<<|?` = flow-inward + gate + query. No new character needed.

2. **One abstract meaning per base symbol.** Adding a new use of an existing symbol must
   fit its established contract. `~` means modification — a new `~X` must involve
   transformation of something.

3. **Context constraints are inherited.** `@~` inherits the loop-context constraint from
   `@!` and `@>`. Any `@`-prefixed statement that acts on the time context must be
   invalid outside a loop.

4. **No natural language words.** Not in any language. Identifiers are free; language
   constructs are symbols only.

5. **No new base symbols without a documented abstract character.** If none of the
   existing symbols fits, define the new symbol's abstract meaning explicitly in this
   document before implementing it.

6. **No symbol may carry two unrelated abstract meanings.** If a symbol is already used
   for meaning A, it cannot also mean B in a different context. The `<=` alias-in-modules
   violation of this rule is documented above and scheduled for correction.
