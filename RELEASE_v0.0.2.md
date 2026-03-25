# Zymbol-Lang — Release Notes v0.0.2

> **Released:** 2026-03-25
> **Test coverage:** 159/159 vm_compare PASS · 94 E2E tests PASS · RosettaStone i18n suite (105 languages) PASS

---

## Overview

v0.0.2 is a language-level release focused on three areas:

1. **Collection API redesign** — unified operator model across arrays, tuples, and strings
2. **Destructuring assignment** — pattern binding for arrays and tuples
3. **Parser fixes and new operators** — arithmetic in `>>`, `^=`, negative indices

All changes are backward-incompatible at the operator level for two retired string operators
(see [Migration Guide](#migration-guide)).

---

## 1. Collection API Redesign

### Problem 1 — Arrays mixed two incompatible mental models

In v0.0.1, the `$` operators for arrays were inconsistent: some expected a **value**,
others expected an **index**, with no predictable rule.

| Operator (v0.0.1) | Argument | Paradigm |
|-------------------|----------|----------|
| `arr$+ val` | value to append | by value |
| `arr$- i` | **index** to remove | by position ← inconsistent |
| `arr$? val` | value to search | by value |
| `arr$~ i` | **index** to update | by position |
| `arr$[i..j]` | range to slice | by position |

A developer could not predict whether `arr$op X` expected a value or an index.
Each operator had to be memorized individually.

There was also no way to:
- Remove a specific **value** from an array (only by index)
- Remove **all** occurrences of a value
- Insert at a specific position
- Find all positions where a value occurs
- Use map/filter/reduce on tuples at all

### Problem 2 — Arrays, tuples, and strings were parallel universes

A string is semantically an array of characters, but v0.0.1 treated them as unrelated
types with separate, incompatible operator sets.

**String-only operators (v0.0.1):**

| Operator | Description |
|----------|-------------|
| `s$++[p:"text"]` | insert "text" at position p |
| `s$--[p:n]` | remove n characters at position p |
| `s$??  val` | all positions of val |
| `s$~~  ["pat":"rep"]` | replace pattern |

None of these worked on arrays. Conversely, `$>` / `$|` / `$<` (map/filter/reduce)
worked on arrays but not on strings. You could loop `@ c:"hello"` but could not
`"hello"$> fn`.

**Tuples (v0.0.1):**

Named tuples supported only `$#` (length). All other `$` operators failed at runtime
with an unsupported type error. Positional tuples had the same array operators but
inherited the same inconsistency.

---

### The unified rule (v0.0.2)

Every collection operator now follows a single predictable model:

```
col$op val      →  operates on a VALUE  (what to find/add/remove)
col$op[i]       →  operates on a POSITION  (where)
col$op[i..j]    →  operates on a RANGE  (where, over multiple positions)
```

The square bracket always means **WHERE**, not **WHAT**.
This rule applies uniformly to arrays, positional tuples, named tuples, and strings.

---

### New operators

| Operator | Description | Example |
|----------|-------------|---------|
| `col$+[i] val` | Insert at position | `arr$+[2] 99` |
| `col$- val` | Remove first occurrence by value | `arr$- 30` |
| `col$-- val` | Remove all occurrences by value | `arr$-- 20` |
| `col$-[i]` | Remove at index | `arr$-[0]` |
| `col$-[i..j]` | Remove range | `arr$-[1..4]` |
| `col$?? val` | All indices of value | `arr$?? 30` → `[2, 5]` |

**Before and after — arrays:**

```zymbol
arr = [10, 20, 30, 20, 40]

// v0.0.1: remove at index 0 (by position)
arr = arr$- 0           // removed element at index 0 → [20,30,20,40]

// v0.0.2: $- is now by value; $-[i] is by position
arr = arr$- 30          // remove first value 30     → [10,20,20,40]
arr = arr$-- 20         // remove all value 20       → [10,40]
arr = arr$-[0]          // remove at index 0         → [40]

// New: insert at position, find all occurrences
arr = [10, 20, 30, 20]
arr = arr$+[1] 99       // insert 99 at index 1      → [10,99,20,30,20]
pos = arr$?? 20         // all indices of value 20   → [2, 4]
```

### Strings are now collections

All collection operators work on strings. The element type is `Char`.
The string-specific insert/remove forms (`$++[p:t]` / `$--[p:n]`) are retired
in favor of the unified positional forms.

```zymbol
// v0.0.1: string-only forms
s = s$++[5:"!!!"]       // insert "!!!" at position 5
s = s$--[0:3]           // remove 3 chars at position 0

// v0.0.2: unified positional forms (same syntax as arrays)
s = s$+[5] "!!!"        // insert "!!!" at position 5
s = s$-[0..3]           // remove range 0..3

// All collection operators now work on strings:
s = "hello world"
s = s$+ '!'             // "hello world!"
s = s$- 'l'             // "helo world!"    (remove first 'l')
s = s$-- 'l'            // "heo word!"      (remove all 'l')
has = s$? 'w'           // #1
pos = s$?? 'o'          // [1, 4]           (all positions of 'o')
sub = s$[0..4]          // "heo "

// map / filter / reduce — previously impossible on strings
vow = "hello"$| (c -> c$? "aeiou")         // "eo"
cnt = "hello"$< (0, (n, c) -> n + 1)       // 5
```

### Tuple support

All new operators work on positional tuples (`(10, 20, 30)`).

In v0.0.1, named tuples supported only `$#`. All other operators failed at runtime
with an unsupported type error. In v0.0.2, named tuples support the full operator set
except `$+` and `$+[i]` (no field name can be inferred for a new element).
Field names are preserved by all read/transform operators (`$>`, `$|`, `$<`, `$[..]`, `$~`).

```zymbol
p = (a: 1, b: 2, c: 1, d: 3)

p$#                         // 4
p$?? 1                      // [0, 2]  (all indices with value 1)
p$- 1                       // (b: 2, c: 1, d: 3)
p$-- 1                      // (b: 2, d: 3)
p$-[0]                      // (b: 2, c: 1, d: 3)
p$> (x -> x * 10)           // (a: 10, b: 20, c: 10, d: 30)
p$| (x -> x > 1)            // (b: 2, d: 3)
p$< (0, (acc, x) -> acc+x)  // 7
```

### Negative indices

`arr[-1]` is now supported in both tree-walker and VM execution modes.

```zymbol
arr = [10, 20, 30]
>> arr[-1] ¶    // 30
>> arr[-2] ¶    // 20
```

### Sort operator — clarification

`$^+` / `$^-` sort primitive arrays (integers, floats, strings) in natural order.
To sort arrays of tuples, use `$^` with a comparator lambda:

```zymbol
nums  = [3, 1, 2]
asc   = nums$^+             // [1, 2, 3]
desc  = nums$^-             // [3, 2, 1]

// sort array of named tuples
people = [(name: "Bob", age: 30), (name: "Alice", age: 25)]
by_age  = people$^ (a, b -> a.age < b.age)    // ascending by age
by_name = people$^ (a, b -> a.name > b.name)  // descending by name
```

---

## 2. Destructuring Assignment

Unpack arrays and tuples directly into variables.

### Array destructuring

```zymbol
arr = [1, 2, 3, 4, 5]

[a, b, c]       = arr    // a=1, b=2, c=3
[x, *rest]      = arr    // x=1, rest=[2,3,4,5]
[first, _, last] = arr   // first=1, last=3 (_ discards)
```

### Named tuple destructuring

```zymbol
person = (name: "Alice", age: 30, city: "NYC")

(name: n, age: a) = person    // n="Alice", a=30
```

### Positional tuple destructuring

```zymbol
point = (10, 20)

(x, y) = point    // x=10, y=20
```

---

## 3. Parser Fixes

### Arithmetic in `>>` output statements

Binary operators inside `>>` now parse correctly as expressions.
Previously, `>> a - b` would print two items (`a` and `b`) instead of their difference.

```zymbol
a = 10
b = 3

>> a - b ¶      // 7    (was: "10 3")
>> a ^ b ¶      // 1000
>> a - b * 2 ¶  // 4    (correct precedence: a - (b*2))
>> -5 ¶         // -5   (unary minus still works)

// Haskell-style juxtaposition (multiple items) still works:
>> "Score: " a ¶   // "Score: 10"   (two items, no operator)
```

The rule: a binary operator between two expressions is always parsed as a single
arithmetic expression. Juxtaposition (two adjacent values with no operator) produces
separate output items.

### `^=` power-assign operator

```zymbol
x = 2
x ^= 10    // x = 2 ^ 10 = 1024
```

Expands at parse time to `x = x ^ 10`. Supported in both tree-walker and VM.

Complete compound assignment set:

| Operator | Meaning |
|----------|---------|
| `+=` | add and assign |
| `-=` | subtract and assign |
| `*=` | multiply and assign |
| `/=` | divide and assign |
| `%=` | modulo and assign |
| `^=` | power and assign |
| `++` | increment (equivalent to `+= 1`) |
| `--` | decrement (equivalent to `-= 1`) |

---

## Migration Guide

Two string operators from v0.0.1 are retired. Both emit a **parse error** with a
migration message.

| v0.0.1 | v0.0.2 | Description |
|--------|--------|-------------|
| `s$++[5:"!!!"]` | `s$+[5] "!!!"` | Insert at position |
| `s$--[0:6]` | `s$-[0..6]` | Remove range |

The old `arr$- i` (remove by **index**) is now `arr$-[i]`.
The new `arr$- v` removes the first element with **value** `v`.

```zymbol
// Remove by index
arr = arr$-[0]        // NEW: remove element at index 0
arr = arr$- 30        // NEW: remove first element with value 30

// String insert at position
s = s$+[5] "!!!"      // NEW (was: s$++[5:"!!!"])

// String remove range
s = s$-[0..6]         // NEW (was: s$--[0:6])
```

---

## VM Parity

All new language features have full parity between tree-walker and register VM.

| Feature | Tree-walker | VM |
|---------|:-----------:|:--:|
| All new collection operators | ✅ | ✅ |
| Destructuring assignment | ✅ | ✅ |
| Negative indices | ✅ | ✅ |
| `^=` operator | ✅ | ✅ |
| `>> a - b` arithmetic | ✅ | ✅ |

---

## Known Gaps (unchanged from v0.0.1)

| Gap | Workaround |
|-----|------------|
| Match multi-value arms (`1, 2 : "low"`) | Use guard: `_? n == 1 \|\| n == 2 : "low"` |
| Named functions as values (`arr$> fn`) | Wrap: `arr$> (x -> fn(x))` |
| CLI args `><` in VM mode | Use tree-walker for CLI arg programs |
| Module constants via `alias.CONST` | Use getter function |
