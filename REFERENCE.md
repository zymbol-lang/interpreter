# Zymbol-Lang — Language Reference

Complete lookup reference: known limitations, error taxonomy, and symbol table.

**Interpreter version**: v0.0.4

See also: [GUIDE.md](GUIDE.md) — full language guide with examples  
See also: [IMPLEMENTATION.md](IMPLEMENTATION.md) — EBNF grammar and internals

---

## Table of Contents

20. [Known Limitations and Workarounds](#20-known-limitations-and-workarounds)
20b. [Error Taxonomy](#20b-error-taxonomy)
21. [Complete Symbol Reference](#21-complete-symbol-reference)

---

## 20. Known Limitations and Workarounds

Limitations are classified in two categories:

- **By design** — intentional constraints that reflect a deliberate language decision. They will not change without a redesign.
- **Implementation gap** — behaviors that diverge from intent due to incomplete implementation. Subject to change in future versions.

---

### By Design

---

### L1 — Postfix operators directly in `>>` *(implementation gap)*

**Symptom**: `>> "len=" arr$# ¶` → parser error (`DollarHash unexpected`).

Postfix operators (`$#`, `$?`, `$!`, `#?`, `$[..]`) are not recognized as items
in `>>` juxtaposition.

```zymbol
>> (arr$#) ¶         // ✅ wrap in parentheses
n = arr$#            // ✅ intermediate variable
>> "len=" n ¶
>> "has=" (arr$? 3) ¶
```

### ~~L3 — Module alias.CONST does not work~~ Fixed

`alias.CONST` access now works correctly:

```zymbol
<# ./math <= m
pi = m.PI    // ✅ works
e  = m.E     // ✅ works
```

**Root cause fixed**: the TypeChecker was emitting a fatal "undefined variable" error
for the module alias identifier before the interpreter could evaluate the member access.
Fix: `TypeChecker` now registers import aliases from `program.imports` before analysis passes.

### ~~L4 — `#>` export block must come before definitions~~ Fixed

`#>` can now appear in any of these positions — all are valid:

```zymbol
// ✅ Position 1 — right after # declaration (always worked):
# module_name
#> { add, PI }
PI := 3.14
add(a, b) { <~ a + b }

// ✅ Position 2 — after imports (G14 fix):
# module_name
<# ./dep <= d
#> { add, PI }
PI := 3.14
add(a, b) { <~ a + b }
```

The only remaining restriction: `#>` must come before executable statements and function definitions (not at the end of the file).

### ~~L5 — Named functions are not first-class values~~ Fixed in v0.0.4

`fn = myFunc` and `arr$> myFunc` now work directly.

```zymbol
double = (x -> x * 2)
fn = double          // ✅ assign lambda to variable
>> fn(5) ¶           // → 10
```

### ~~L6 — HOF `$>`, `$|`, `$<` require inline lambdas~~ Fixed in v0.0.4

Named functions and lambda variables are now accepted directly:

```zymbol
double = (x -> x * 2)
nums = [1, 2, 3]
>> (nums$> double) ¶    // ✅ → [2, 4, 6]
```

### ~~L7 — Match multi-value arms not implemented~~ Fixed in v0.0.4

Multi-value arms are supported via list containment patterns:

```zymbol
?? y {
    [1, 2] : "low"
    _      : "other"
}  // ✅
```

### L8 — ~~Negative array indices: WT vs VM behavior differs~~ Fixed in v0.0.2

Negative indices are now normalized in both tree-walker and VM:

```zymbol
arr = [10, 20, 30, 40, 50]
>> arr[-1] ¶    // → 50 (last element)
>> arr[-2] ¶    // → 40
```

### L9 — False positive warnings *(implementation gap)*

| Warning | Cause | Action |
|---------|-------|--------|
| `unused variable 'x'` when `x` is used in `"{x}"` interpolation | Static analyzer does not track interpolation usage | Ignore |
| `unused variable 'x'` when `x` is used in `<\ bash {x} \>` | Analyzer does not track BashExec variable usage | Ignore, or prefix with `_`: `_x` and `{_x}` |

### ~~L10 — Collection operators cannot be chained~~ Fixed in v0.0.4

`$+` now chains left-to-right:

```zymbol
arr = [1, 2, 3]$+ 4$+ 5$+ 6    // ✅ → [1, 2, 3, 4, 5, 6]
```

The argument to `$+` is parsed at structural postfix level (index, call, member access) but stops before the next `$` operator, enabling the chain. If the argument itself needs a collection op, wrap it in parentheses:

```zymbol
arr = base$+ (other$#)$+ 0    // appends length of other, then 0
```

---

### Implementation Gaps

---

### L12 — `do-while` (`~>`) not implemented *(implementation gap)*

A post-condition loop (execute body at least once, then repeat) is defined in the EBNF
but not yet implemented.

```zymbol
// ❌ Not implemented:
// { body } ~> condition

// ✅ Workaround — infinite loop with break at the end:
@ {
    // body runs at least once
    body_here()
    ? !condition { @! }
}
```

### L13 — `$!!` from lambdas not supported *(implementation gap)*

`$!!` error propagation only works inside **named functions**. Placing it inside a
lambda does not propagate to the lambda's caller.

```zymbol
// ❌ Inside lambda — propagation does not reach outer caller:
handler = x -> { x$!! }

// ✅ Wrap the logic in a named function:
handle(x) {
    x$!!
}
```

### L11 — Arrays must be homogeneous *(by design)*

All elements of an array must share the same type. This is enforced by the semantic checker:

```zymbol
record = ["English", "en.zy", #0]    // ❌ String + String + Bool
```

**Why**: arrays are Zymbol's ordered mutable collection for uniform data — sequences of the same kind of value. This constraint enables type-safe collection operations (`$>`, `$|`, `$<`, `$^`) without runtime type dispatch.

**Heterogeneous records belong in named tuples**, which are immutable and field-named:

```zymbol
// ✅ Named tuple — heterogeneous, immutable, field-addressed:
record = (lang: "English", file: "en.zy", active: #1)
>> record.lang ¶
>> record.active ¶

// ✅ Array of named tuples — uniform container of heterogeneous records:
langs = [
    (lang: "English", file: "en.zy",  active: #1),
    (lang: "Spanish", file: "es.zy",  active: #1),
    (lang: "Chinese", file: "zh.zy",  active: #0)
]
@ entry:langs {
    ? entry.active { >> entry.lang " → " entry.file ¶ }
}
```

The design distinction maps cleanly: **arrays = typed sequences**, **named tuples = structured records**.

---

### L14 — Destructuring does not enforce constant immutability *(implementation gap)*

Destructuring a pattern that includes a name previously declared with `:=` silently overwrites the constant instead of raising an error:

```zymbol
limit := 100        // constant
[limit, extra] = [200, 300]
>> limit ¶          // 200 — constant was silently overwritten ⚠
```

**By design or implementation gap?** Implementation gap — the constant-check path in the interpreter is not reached during destructuring. A future fix should detect `:=`-declared names in the destructuring pattern and raise a semantic error.

**Workaround**: Use distinct names for destructuring targets if you need to preserve a constant in the same scope:

```zymbol
limit := 100
[new_limit, extra] = [200, 300]
>> limit ¶          // 100  — original constant preserved
```

---

## 20b. Error Taxonomy

Zymbol errors are classified into three categories based on when and how they are detected.

---

### Parser Errors

Detected during the parsing phase — the source text does not conform to the grammar. Execution never begins.

```
Error [line N, col M]: unexpected token '...' — expected '...'
```

**Common triggers:**
- Unmatched braces, brackets, or parentheses
- Operator with missing operand (e.g., `+` with no right-hand side)
- Invalid label syntax
- Malformed string literal or interpolation

Parser errors are always fatal. They cannot be caught with error-handling syntax (`!?`, `$!`).

---

### Semantic Errors

Detected after parsing, during the semantic analysis phase. The grammar is valid but the code violates a language rule.

```
Error [line N]: undefined variable 'x'
Error [line N]: module 'mod' is private
```

**Common triggers:**
- Reference to an undefined variable or function
- Accessing a private module from outside
- Circular imports

Semantic errors are always fatal and cannot be caught at runtime. They are reported before execution starts.

---

### Runtime Errors

Detected during execution. The code is grammatically and semantically valid, but a condition fails at runtime.

```
RuntimeError: ##kind(message)
```

Runtime errors in Zymbol are **values** — they propagate through the call stack until caught or they terminate execution. They can be caught with:

- `!? { } :! { }` — try/catch block; `_err` holds the error as `##Kind(message)`
- `:! ##Kind { }` — typed catch clause, matches a specific error kind
- `:> { }` — finally block (always executes, regardless of error)

Related operators:
- `$!` — returns `#1` if the value is an error, `#0` otherwise
- `$!!` — re-propagates an error value from within a named function to its caller (see §16)

**Common sources:**
- Index out of bounds: `arr[99]` when array has fewer elements
- Division by zero: `x / 0`
- Named tuple field not found: `t.nonexistent`

Runtime errors carry a **kind** (e.g., `##Index`, `##Div`, `##Type`) and a **message** string. The value in `_err` has the format `##Kind(message)`. The `#?` type symbol of an error value is the kind itself — `(##Index, N, ...)` — there is no generic error type symbol.

```zymbol
!? {
    v = arr[99]
} :! ##Index {
    >> _err ¶   // ##Index(array index out of bounds: index 99 for array of length 3)
}
```

---

### Fail-safe Operations

Some operations are intentionally **fail-safe**: they never raise a runtime error; instead, they return a neutral value on failure.

| Operation | Failure result |
|-----------|---------------|
| `#\|"abc"\|` — numeral conversion | original string unchanged |
| `arr$? val` — contains | `false` |
| `#?val` — safe type check | `false` (never errors) |

Fail-safe operations are distinguished from error-handling by the absence of any error path — they are guaranteed to return a valid value of a predictable type.

---

## 21. Complete Symbol Reference

| Symbol | Operation | Example |
|--------|-----------|---------|
| `=` | Assignment | `x = 5` |
| `[..] =` | Array destructure | `[a, b, *rest] = arr` |
| `(..) =` | Tuple destructure | `(name: n, age: a) = t` |
| `:=` | Constant | `PI := 3.14` |
| `>>` | Output | `>> "hello" ¶` |
| `<<` | Input | `<< "prompt: " var` |
| `¶` / `\\` | Newline in output | `>> msg ¶` |
| `?` | If | `? x > 0 { }` |
| `_?` | Else-if | `_? x < 0 { }` |
| `_` | Else / wildcard | `_{ }` |
| `??` | Match | `?? x { pat : val }` |
| `[p, q]` | Match list pattern | `?? arr { [_, _] : ... }` |
| `@` | Loop (while) | `@ cond { }` |
| `@` | Loop (times) | `@ N { }` — repeats exactly N times when N is a positive Int |
| `@` | Loop (infinite) | `@ { }` |
| `@!` | Break | `@!` or `@! label` |
| `@>` | Continue | `@>` or `@> label` |
| `->` | Lambda | `x -> x * 2` |
| `<~` | Return / output param | `<~ value` |
| `\|>` | Pipe | `val \|> fn` or `val \|> fn(_)` |
| `$#` | Length | `arr$#` |
| `$+` | Append by value | `arr$+ elem` |
| `$+[i]` | Insert at position | `arr$+[2] elem` |
| `$-` | Remove first by value | `arr$- val` |
| `$--` | Remove all by value | `arr$-- val` |
| `$-[i]` | Remove at index | `arr$-[1]` |
| `$-[i..j]` | Remove range (1-based inclusive) | `arr$-[2..3]` |
| `$-[i:n]` | Remove range (count-based) | `arr$-[2:2]` |
| `$?` | Contains | `arr$? val` |
| `$??` | Find all indices of value | `arr$?? val` |
| `arr[i] = val` | Direct element update (arrays only) | `arr[2] = 99` |
| `arr[i] += val` | Compound element update (arrays only) | `arr[1] += 5` |
| `arr[i]$~` | Functional update — returns new collection | `arr[2]$~ 99` |
| `arr[i>j]` | Scalar deep access (row i, col j) | `m[2>3]` → `6` |
| `arr[i>j>k]` | Scalar deep access depth 3+ | `cubo[1>2>1]` |
| `arr[(e)>j]` | Computed first step | `m[(n)>(n)]` |
| `arr[a>b]` | Variable indices as nav atoms | `m[row>col]` |
| `arr[-1>-1]` | Negative indices in nav path | last row, last col |
| `arr[[i>j]]` | Flat extraction — single path wrapped | `m[[2>3]]` → `[6]` |
| `arr[p ; q]` | Flat extraction — multiple paths | `m[1>1 ; 2>3]` → `[1, 6]` |
| `arr[[g] ; [g]]` | Structured extraction | `m[[1>1] ; [2>3]]` → `[[1], [6]]` |
| `arr[[p,q] ; [r,s]]` | Structured, multi-value groups | `m[[1>1,1>3] ; [3>1,3>3]]` |
| `arr[i>r1..r2]` | Range on last step (expand axis) | `m[[1>2..3]]` → `[2, 3]` |
| `arr[r1..r2>j]` | Range on intermediate step (fan-out) | `m[[1..2>3]]` |
| `$[i..j]` | Slice (1-based inclusive) | `arr$[1..3]` |
| `$[i:n]` | Slice (count-based) | `arr$[1:2]` |
| `$^+` | Sort ascending (primitives) | `arr$^+` |
| `$^-` | Sort descending (primitives) | `arr$^-` |
| `$^` | Sort with comparator (tuples) | `arr$^ (a,b -> a.f < b.f)` |
| `$>` | Map | `arr$> (x -> f(x))` |
| `$\|` | Filter | `arr$\| (x -> cond)` |
| `$<` | Reduce | `arr$< (0, (a,x) -> a+x)` |
| `$~~[p:r]` | String replace | `s$~~["o":"0"]` |
| `$/` | String split by char or substring | `"a,b" $/ ','` |
| `$++` | ConcatBuild — append to string or array | `"x=" $++ n flag` |
| `!?` | Try | `!? { } :! { }` |
| `:!` | Catch | `:! ##Div { }` |
| `:>` | Finally | `:> { }` |
| `$!` | Is error | `val$!` |
| `$!!` | Propagate error | `val$!!` |
| `#\|x\|` | Numeric eval (ASCII + 69 Unicode scripts) | `#\|"42"\|`, `#\|"๔๒"\|` |
| `x#?` | Type metadata | `42#?` |
| `#.N\|x\|` | Round N decimals | `#.2\|3.14159\|` |
| `#!N\|x\|` | Truncate N decimals | `#!2\|3.14159\|` |
| `##.expr` | Cast to Float | `##.42` → `42` (Float) |
| `###expr` | Cast to Int (rounding) | `###3.7` → `4` |
| `##!expr` | Cast to Int (truncating) | `##!3.7` → `3` |
| `#,\|x\|` | Comma format | `#,\|1234567\|` |
| `#^\|x\|` | Scientific notation | `#^\|12345.0\|` |
| `0x`, `0b`, `0o`, `0d` | Base literals | `0x41` → `'A'` |
| `#` | Module declaration | `# name` |
| `#>` | Module export | `#> { fn, CONST }` |
| `<#` | Module import | `<# ./mod <= alias` |
| `<=` | Alias | (used in `<#` and `#>`) |
| `::` | Module function call | `m::func(args)` |
| `.` | Member access | `tuple.field` |
| `<\ cmd \>` | BashExec | `<\ ls -la \>` |
| `</ f.zy />` | Execute script | `</ ./sub.zy />` |
| `>< args` | CLI args capture | `>< args` |
| `\ var` | Explicit lifetime end | `\ x` |
| `#1` / `#0` | Bool true / false | `? #1 { }` |
| `#d0d9#` | Numeral mode switch | `#०९#` (Devanagari), `#09#` (reset) |
| `++` / `--` | Increment / decrement | `x++` |
| `+=` `-=` `*=` `/=` `%=` `^=` | Compound assignment | `x += 5` |

---

