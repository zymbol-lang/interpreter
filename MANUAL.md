# Zymbol-Lang — Language Manual

> **Authoritative reference** — all examples verified empirically on both execution modes:
> `zymbol run` (tree-walker) and `zymbol run --vm` (register VM).
> If a construct is not documented here, it may not be implemented.

**Interpreter version**: v0.0.2
**Test coverage**: 159/159 vm_compare PASS

---

## Table of Contents

1. [Running Programs](#1-running-programs)
2. [Data Types](#2-data-types)
3. [Output and Input](#3-output-and-input)
4. [Variables and Constants](#4-variables-and-constants)
5. [Operators](#5-operators)
6. [Control Flow](#6-control-flow)
7. [Match](#7-match)
8. [Loops](#8-loops)
9. [Functions](#9-functions)
10. [Lambdas and Closures](#10-lambdas-and-closures)
11. [Arrays](#11-arrays)
11b. [Destructuring Assignment](#11b-destructuring-assignment)
12. [Tuples](#12-tuples)
13. [Strings](#13-strings)
14. [Higher-Order Functions](#14-higher-order-functions)
15. [Pipe Operator](#15-pipe-operator)
16. [Error Handling](#16-error-handling)
17. [Modules](#17-modules)
18. [Data Operators](#18-data-operators)
19. [Shell Integration](#19-shell-integration)
20. [Known Limitations and Workarounds](#20-known-limitations-and-workarounds)
21. [Complete Symbol Reference](#21-complete-symbol-reference)
22. [Verified Examples](#22-verified-examples)
23. [EBNF Coverage Status](#23-ebnf-coverage-status)

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

Both modes produce **identical output** on 159/159 parity tests.

---

## 2. Data Types

| Type | Literal | `#?` symbol | Notes |
|------|---------|-------------|-------|
| Int | `42`, `-7` | `###` | 64-bit signed |
| Float | `3.14`, `1.5e10` | `##.` | Scientific notation supported |
| String | `"text"` | `##"` | Interpolation: `"Hello {name}"` |
| Char | `'A'` | `##'` | Single Unicode character |
| Bool | `#1`, `#0` | `##?` | NOT numeric — `#1` ≠ `1` |
| Array | `[1, 2, 3]` | `##]` | Must be homogeneous (same type) |
| Tuple | `(a, b)` | `##)` | Positional |
| NamedTuple | `(x: 1, y: 2)` | `##)` | Named fields |
| Unit | _(no value)_ | `##_` | Empty return |

```zymbol
// Inspect the type of a value
x = 42
meta = x#?
>> meta ¶
// → (###, 2, 42)
//    ^type ^digits ^value

// Extract just the type symbol (intermediate variable required)
t = meta[0]
>> t ¶    // → ###
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

String concatenation with `+` in output works but generates a type warning. Prefer juxtaposition:

```zymbol
>> "Score: " score ¶               // ✅ canonical
>> "Score: " + score ¶             // ⚠ works but triggers warning
```

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
x++       // x = 3  (equivalent to x += 1)
x--       // x = 2  (equivalent to x -= 1)
```

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

### String Concatenation

Three correct forms — use the one that fits the context:

```zymbol
name = "Alice"
n = 42

// 1. Juxtaposition in >> (canonical output form)
>> "Hello " name " you have " n " items" ¶

// 2. Comma operator in assignments (spec-correct for = and :=)
msg = "Hello ", name, "!"
TITLE := "Welcome, ", name

// 3. Interpolation (most readable for complex strings)
desc = "Hello {name}, you have {n} items"
```

> **Spec note**: `+` is defined for **numbers only**. Using `"text" + value` works as
> an extension but triggers `arithmetic operation on non-numeric type` warning.
> Use `,`, juxtaposition, or interpolation for strings.

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

// Match on string literals
color = "red"
code = ?? color {
    "red"   : "#FF0000"
    "green" : "#00FF00"
    "blue"  : "#0000FF"
    _       : "#000000"
}
>> code ¶

// Guard patterns with _?
temperature = -5
state = ?? temperature {
    _? temperature < 0   : "ice"
    _? temperature < 20  : "cold"
    _? temperature < 35  : "warm"
    _                    : "hot"
}
>> state ¶    // → ice

// Match as statement (block arms)
n = 42
?? n {
    0 : { >> "zero" ¶ }
    _? n < 0 : { >> "negative" ¶ }
    _ : {
        >> "positive: " n ¶
    }
}
```

> **⚠ Not implemented**: Multi-value arms (`1, 2 : "low"`) are not supported.
> Workaround: use guard `_? n == 1 || n == 2 : "low"`.

> **⚠ Not implemented**: Identifier binding in patterns (`n : n * 2`).

---

## 8. Loops

### Infinite Loop

```zymbol
i = 0
@ {
    i++
    ? i >= 5 { @! }
    >> i " "
}
>> ¶    // → 1 2 3 4 5
```

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

```zymbol
// Manual label simulation (nested break)
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

// Explicit label syntax
count = 0
@ @outer {
    count++
    ? count >= 3 { @! outer }
}
>> count ¶    // → 3
```

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

Functions have **isolated scope** — they cannot access outer variables:

```zymbol
global = 100

test() {
    // 'global' does not exist here — only params are in scope
    x = 42        // local
    <~ x
}

>> test() ¶    // → 42
```

> **Lambdas** DO capture the outer scope (see section 10).

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

// Inside HOF (named functions must be wrapped in lambda)
nums = [1, 2, 3, 4, 5, 6]
r = nums$> (x -> double(x))         // ✅ wrapper required
r = nums$| (x -> is_big(x))         // ✅ wrapper required
```

### Anti-patterns

```zymbol
// Named functions are NOT first-class values
fn = double              // ❌ "undefined variable: 'double'"
fn = x -> double(x)      // ✅ wrap in lambda

// HOF does not accept lambda variable directly as operand
fn = x -> x * 2
nums$> fn                // ❌ parser error
nums$> (x -> fn(x))      // ✅ inline lambda always works

// Postfix operators in >> require parentheses
>> arr$# ¶               // ❌ "DollarHash unexpected"
>> (arr$#) ¶             // ✅
n = arr$#                // ✅ intermediate variable
```

### Named Function vs Lambda — When to Use Each

| Need | Use |
|------|-----|
| Reusable logic, no external state | Named function `fn(params) { }` |
| Recursion | Named function (lambdas cannot self-reference) |
| Capture outer scope variable | Lambda `x -> expr` |
| Pass as argument (first-class) | Lambda |
| Store in array | Lambda |
| Return from another function | Lambda |
| Call a named function in HOF | Lambda wrapper: `(x -> named(x))` |

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
>> ops[0](5) ¶    // → 6
>> ops[1](5) ¶    // → 10
>> ops[2](5) ¶    // → 25

// Pass as argument
apply(f, x) { <~ f(x) }
>> apply(x -> x * 3, 7) ¶    // → 21
```

---

## 11. Arrays

### Creation and Access

```zymbol
arr = [10, 20, 30, 40, 50]
>> arr ¶           // → [10, 20, 30, 40, 50]
>> arr[0] ¶        // → 10 (0-indexed)
>> arr[2] ¶        // → 30
```

> **Negative indices**: `arr[-1]` returns the last element, `arr[-2]` the second-to-last, etc.
> Supported in both tree-walker and VM (v0.0.2).

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

// $+[i] — insert at position
arr2 = arr$+[2] 99
>> arr2 ¶    // → [1, 2, 99, 3, 4, 5, 6]

// $- val — remove first occurrence by value
arr3 = arr$- 3
>> arr3 ¶    // → [1, 2, 4, 5, 6]

// $-- val — remove all occurrences by value
arr4 = [1, 2, 3, 2, 4]$-- 2
>> arr4 ¶    // → [1, 3, 4]

// $-[i] — remove at index
arr5 = arr$-[0]
>> arr5 ¶    // → [2, 3, 4, 5, 6]

// $-[start..end] — remove range (end EXCLUSIVE)
arr6 = arr$-[1..3]
>> arr6 ¶    // → [1, 4, 5, 6]

// $-[start:count] — remove range, count-based (alternative syntax)
arr6b = arr$-[1:2]
>> arr6b ¶    // → [1, 4, 5, 6]  (identical result to $-[1..3])

// $? — contains
has = arr$? 3
>> has ¶    // → #1

// $?? — find all indices
pos = [1, 2, 1, 3, 1]$?? 1
>> pos ¶    // → [0, 2, 4]

// $[..] — slice [start..end) — end is EXCLUSIVE
sl = arr$[0..3]
>> sl ¶    // → [1, 2, 3]

// $[start:count] — slice count-based (alternative syntax)
sl2 = arr$[0:3]
>> sl2 ¶    // → [1, 2, 3]  (identical result)
```

> **Note**: All collection operators return a new collection. Assign back to the
> same variable: `arr = arr$+ 4`. Operators **cannot be chained directly**:
> ```zymbol
> arr = arr$+ 5$+ 6    // ❌ not supported
> arr = arr$+ 5        // ✅ intermediate assignment
> arr = arr$+ 6
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

**Custom comparator** — a two-argument lambda that returns a bool (`#1` if first
element should come before second). Required when sorting named tuples by field:

```zymbol
db = [
    (name: "Carla", age: 28),
    (name: "Ana",   age: 25),
    (name: "Bob",   age: 30)
]

// Sort by age ascending
by_age = db$^+ (a, b -> a.age < b.age)
>> by_age[0].name ¶    // → Ana

// Sort by name descending
by_name_desc = db$^- (a, b -> a.name < b.name)
>> by_name_desc[0].name ¶    // → Carla
```

> **Note**: When a custom comparator is provided, the `+`/`-` sign still documents
> intent but the lambda defines the actual ordering. `$^+` and `$^-` with the same
> lambda produce opposite orderings.

### Direct Element Update

```zymbol
arr = [10, 20, 30, 40, 50]
arr[2] = 99
>> arr ¶    // → [10, 20, 99, 40, 50]

// Functional form (generates new array)
arr = arr[2]$~ 99
```

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
>> matrix[1] ¶       // → [4, 5, 6]
>> matrix[1][2] ¶    // → 6
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

> **Note**: Destructuring always creates new variables — it does not update existing ones.
> All patterns are matched positionally (arrays, positional tuples) or by field name (named tuples).

---

## 12. Tuples

### Positional Tuple

```zymbol
point = (10, 20)
>> point[0] ¶    // → 10
>> point[1] ¶    // → 20
```

### Named Tuple

```zymbol
person = (name: "Alice", age: 25, active: #1)

// Access by field name (recommended)
>> person.name ¶    // → Alice
>> person.age ¶     // → 25

// Access by positional index
>> person[0] ¶      // → Alice
>> person[1] ¶      // → 25

// Nested named tuples
pos = (x: 10, y: 20)
p = (pos: pos, label: "origin")
>> p.label ¶        // → origin
>> p.pos.x ¶        // → 10
```

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

// Slice [start..end) — end is EXCLUSIVE
sub = s$[0..5]
>> sub ¶    // → Hello

// Slice count-based (alternative syntax)
sub2 = s$[0:5]
>> sub2 ¶    // → Hello  (identical result)

// Split by char
parts = "a,b,c,d" / ','
>> parts ¶    // → [a, b, c, d]
// ⚠ false warning: "arithmetic on non-numeric" — ignore it
```

### Advanced String Operators

```zymbol
s = "hello world"

// $+ — append char or string
s2 = s$+ "!"
>> s2 ¶    // → hello world!

// $+[i] — insert at char position
ins = s$+[5] "!!!"
>> ins ¶    // → hello!!! world

// $- val — remove first occurrence of char or substring
rem1 = s$- 'l'
>> rem1 ¶    // → helo world

// $-- val — remove all occurrences
rem2 = s$-- 'l'
>> rem2 ¶    // → heo word

// $-[i] — remove char at index
rem3 = s$-[0]
>> rem3 ¶    // → ello world

// $-[start..end] — remove char range (end EXCLUSIVE)
rem4 = s$-[0..6]
>> rem4 ¶    // → world

// $-[start:count] — remove char range, count-based (alternative syntax)
rem4b = s$-[0:6]
>> rem4b ¶    // → world  (identical result)

// $?? — find all positions of a pattern
pos = s$?? "o"
>> pos ¶    // → [4, 7]  (0-based char indices)

// $~~[pattern:replacement] — replace all occurrences
rep = s$~~["l":"L"]
>> rep ¶    // → heLLo worLd

// $~~[pattern:replacement:N] — replace only first N occurrences
rep1 = s$~~["l":"L":1]
>> rep1 ¶   // → heLlo world
```

### Concatenation — Three Correct Forms

```zymbol
name = "Alice"
n = 42

// 1. Juxtaposition in >> (canonical)
>> "Hello " name " you have " n " items" ¶

// 2. Comma operator in assignments (spec-correct)
msg = "Hello ", name, "!"
GREETING := "Welcome, ", name

// 3. String interpolation (most readable)
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

HOF operators require **inline lambdas** — lambda variables passed directly do not work.

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

### Named Functions Inside HOF Lambdas

```zymbol
double(x) { <~ x * 2 }
is_big(x) { <~ x > 5 }

nums = [1, 2, 3, 4, 5, 6, 7, 8]

// Named functions are callable INSIDE a lambda wrapper
r = nums$> (x -> double(x))     // ✅
>> r ¶    // → [2, 4, 6, 8, 10, 12, 14, 16]

filtered = nums$| (x -> is_big(x))
>> filtered ¶    // → [6, 7, 8]
```

### Reduce with Block Lambda

```zymbol
data = [3, 1, 4, 1, 5, 9, 2, 6]
maximum = data$< (data[0], (max, x) -> {
    ? x > max { <~ x }
    <~ max
})
>> maximum ¶    // → 9
```

---

## 15. Pipe Operator

The RHS **always** requires `_` as a placeholder for the piped value:

```zymbol
double = x -> x * 2
add = (a, b) -> a + b
inc = x -> x + 1

r1 = 5 |> double(_)
>> r1 ¶    // → 10

r2 = 5 |> (x -> x * 3)(_)
>> r2 ¶    // → 15

// With extra arguments — _ marks position of piped value
r3 = 10 |> add(_, 5)
>> r3 ¶    // → 15

r4 = 5 |> add(2, _)
>> r4 ¶    // → 7

// Chained pipe
r5 = 5 |> double(_) |> inc(_) |> double(_)
>> r5 ¶    // → 22  (5→10→11→22)

// Pipe with closure
factor = 3
r6 = 7 |> (x -> x * factor)(_)
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

---

## 17. Modules

### Module File Structure

> **Important**: The `#>` export block **must come first**, before all definitions.

```zymbol
// file: lib/utils.zy
# utils                // module declaration (must be at absolute start)

#> {                   // exports — MUST come before function definitions
    add
    PI
    VERSION
}

PI := 3.14159
VERSION := "1.0.0"

add(a, b) { <~ a + b }
private_fn(x) { <~ x * 2 }    // not exported — inaccessible from outside
```

### Importing and Using

```zymbol
// Import with alias (alias is required)
<# ./lib/utils <= u

// Call exported function via ::
result = u::add(5, 3)
>> result ¶    // → 8
```

> **⚠ Known limitation**: Direct constant access `alias.CONST` does not work currently:
> ```zymbol
> pi = u.PI    // ❌ "undefined variable 'u'"
> ```
> **Workaround**: Use a getter function in the module:
> ```zymbol
> // In module:
> get_PI() { <~ PI }
> // In main:
> pi = u::get_PI()    // ✅
> ```

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

### Subdirectory Module Convention

```zymbol
# .subfolder_file    // dot convention for modules inside subfolders
```

---

## 18. Data Operators

### Numeric Eval `#|expr|` — Parse String to Number

```zymbol
v1 = #|"42"|
>> v1 ¶    // → 42  (Int)

v2 = #|"3.14"|
>> v2 ¶    // → 3.14  (Float)

v3 = #|"abc"|
>> v3 ¶    // → abc  (original string — fail-safe, no error)

v4 = #|99|
>> v4 ¶    // → 99  (pass-through if already a number)
```

### Type Metadata `expr#?`

Returns tuple `(type, digits, value)`:

```zymbol
ti = 42#?
>> ti ¶    // → (###, 2, 42)

tf = 3.14#?
>> tf ¶    // → (##., 4, 3.14)

ts = "hello"#?
>> ts ¶    // → (##", 5, hello)

tc = 'A'#?
>> tc ¶    // → (##', 1, A)

// Extract just the type (intermediate variable required)
meta = 42#?
t = meta[0]
>> t ¶    // → ###
```

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
```

### Number Formatting

```zymbol
// Comma-separated format for large numbers
nfmt = 1234567
fmt = c|nfmt|
>> fmt ¶    // → 1,234,567

// Scientific notation
xsci = 12345.678
sci = e|xsci|
>> sci ¶    // → 1.2345678e4
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
bin = 0b|65|     // Int → binary string
oct = 0o|8|      // Int → octal string
```

---

## 19. Shell Integration

### BashExec `<\ cmd \>`

Executes a system command and captures stdout + stderr:

```zymbol
// Capture result as string (includes trailing \n)
date = <\ date +%Y-%m-%d \>
>> "Today: " date    // no ¶ needed — date already contains \n

// Interpolation in commands
file = "data.txt"
content = <\ cat {file} \>
>> content

// Arithmetic via shell
result = <\ echo "scale=2; 355/113" | bc \>
>> result
```

> **Note**: Output always includes a trailing `\n`. Account for this when concatenating.

### Execute Script `</ file.zy />`

Executes another Zymbol script and captures its output:

```zymbol
output = </ ./subscript.zy />
>> output
```

---

## 20. Known Limitations and Workarounds

### L1 — Postfix operators directly in `>>`

**Symptom**: `>> "len=" arr$# ¶` → parser error (`DollarHash unexpected`).

Postfix operators (`$#`, `$?`, `$!`, `#?`, `$[..]`) are not recognized as items
in `>>` juxtaposition.

```zymbol
>> (arr$#) ¶         // ✅ wrap in parentheses
n = arr$#            // ✅ intermediate variable
>> "len=" n ¶
>> "has=" (arr$? 3) ¶
```

### L3 — Module alias.CONST does not work

**Symptom**: `x = m.PI` → "undefined variable 'm'".

```zymbol
// In module:
get_PI() { <~ PI }
// In main:
pi = m::get_PI()    // ✅
```

### L4 — `#>` export block must come before definitions

```zymbol
// ❌ Incorrect — #> at the end:
PI := 3.14
add(a, b) { <~ a + b }
#> { add, PI }

// ✅ Correct — #> immediately after # declaration:
# module_name
#> { add, PI }
PI := 3.14
add(a, b) { <~ a + b }
```

### L5 — Named functions are not first-class values

**Symptom**: `fn = myFunc` → "undefined variable 'myFunc'".

```zymbol
fn = x -> myFunc(x)          // ✅ wrap in lambda
fn = (a, b) -> myFunc(a, b)  // ✅ multiple args
```

### L6 — HOF `$>`, `$|`, `$<` require inline lambdas

```zymbol
fn = x -> x * 2
nums$> fn              // ❌ not accepted
nums$> (x -> fn(x))    // ✅ always works
```

### L7 — Match multi-value arms not implemented

```zymbol
?? y { 1, 2 : "low"  _ : "other" }    // ❌ parser error

// Workaround:
?? y {
    _? y == 1 || y == 2 : "low"
    _ : "other"
}  // ✅
```

### L8 — ~~Negative array indices: WT vs VM behavior differs~~ Fixed in v0.0.2

Negative indices are now normalized in both tree-walker and VM:

```zymbol
arr = [10, 20, 30, 40, 50]
>> arr[-1] ¶    // → 50 (last element)
>> arr[-2] ¶    // → 40
```

### L9 — False positive warnings

| Warning | Cause | Action |
|---------|-------|--------|
| `unused variable 'x'` when `x` is used in `"{x}"` interpolation | Static analyzer does not track interpolation usage | Ignore |
| `unused variable 'x'` when `x` is used in `<\ bash {x} \>` | Analyzer does not track BashExec variable usage | Ignore, or prefix with `_`: `_x` and `{_x}` |
| `arithmetic on non-numeric type` when using `/` for string split | Analyzer cannot distinguish string `/` from arithmetic `/` | Ignore |
| `type mismatch: 'arr' was [Int] but assigned Int` on `arr[i] = val` | Analyzer does not understand indexed update | Ignore |

### L10 — Collection operators cannot be chained

```zymbol
arr = [1,2,3]$+ 4$+ 5    // ❌ not supported

arr = [1, 2, 3]
arr = arr$+ 4             // ✅
arr = arr$+ 5
```

### L11 — Arrays must be homogeneous (same type for all elements)

```zymbol
// ❌ Mixed types — parser error:
record = ["English", "en.zy", #0]     // String + String + Bool
matrix = [[1, 2], [3, "four"]]         // Int + String in sub-array

// ✅ Workaround A — encode booleans as strings:
record = ["English", "en.zy", "false"]

// ✅ Workaround B — use 0/1 Int for boolean flags:
flags = [1, 0, 1, 1, 0]

// ✅ Workaround C — parallel arrays by type:
labels = ["English", "Spanish", "Chinese"]
files  = ["en.zy", "es.zy", "zh.zy"]
active = [#1, #1, #0]
@ i:0..(labels$# - 1) {
    >> labels[i] " → " files[i] ¶
}
```

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
| `@` | Loop | `@ cond { }` |
| `@!` | Break | `@!` or `@! label` |
| `@>` | Continue | `@>` |
| `->` | Lambda | `x -> x * 2` |
| `<~` | Return / output param | `<~ value` |
| `\|>` | Pipe | `val \|> fn(_)` |
| `$#` | Length | `arr$#` |
| `$+` | Append by value | `arr$+ elem` |
| `$+[i]` | Insert at position | `arr$+[2] elem` |
| `$-` | Remove first by value | `arr$- val` |
| `$--` | Remove all by value | `arr$-- val` |
| `$-[i]` | Remove at index | `arr$-[0]` |
| `$-[i..j]` | Remove range (exclusive end) | `arr$-[1..3]` |
| `$-[i:n]` | Remove range (count-based) | `arr$-[1:2]` |
| `$?` | Contains | `arr$? val` |
| `$??` | Find all indices of value | `arr$?? val` |
| `$~` | Functional update | `arr[i]$~ val` |
| `$[i..j]` | Slice (exclusive end) | `arr$[1..3]` |
| `$[i:n]` | Slice (count-based) | `arr$[1:2]` |
| `$^+` | Sort ascending | `arr$^+` · `arr$^+ (a,b -> a.f < b.f)` |
| `$^-` | Sort descending | `arr$^-` · `arr$^- (a,b -> a.f < b.f)` |
| `$>` | Map | `arr$> (x -> f(x))` |
| `$\|` | Filter | `arr$\| (x -> cond)` |
| `$<` | Reduce | `arr$< (0, (a,x) -> a+x)` |
| `$~~[p:r]` | String replace | `s$~~["o":"0"]` |
| `/` | String split | `"a,b" / ','` |
| `!?` | Try | `!? { } :! { }` |
| `:!` | Catch | `:! ##Div { }` |
| `:>` | Finally | `:> { }` |
| `$!` | Is error | `val$!` |
| `$!!` | Propagate error | `val$!!` |
| `#\|x\|` | Numeric eval | `#\|"42"\|` |
| `x#?` | Type metadata | `42#?` |
| `#.N\|x\|` | Round N decimals | `#.2\|3.14159\|` |
| `#!N\|x\|` | Truncate N decimals | `#!2\|3.14159\|` |
| `c\|x\|` | Comma format | `c\|1234567\|` |
| `e\|x\|` | Scientific notation | `e\|12345.0\|` |
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
| `,` | String concat in assignments | `msg = "a", "b"` |
| `++` / `--` | Increment / decrement | `x++` |
| `+=` `-=` `*=` `/=` `%=` | Compound assignment | `x += 5` |

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
    @ i:0..(n-2) {           // outer: n-2 (not n-1) to avoid negative range
        @ j:0..(n-i-2) {
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

>> "Total scores: " (scores$#) ¶
>> "Passing: " count ¶
>> "Average (passing): " average ¶
```

### Complete Module Example

```zymbol
// file: calc.zy
# calc

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
get_version()  { <~ _VERSION }    // getter workaround for L3
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
    type = meta[0]
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

## 23. EBNF Coverage Status

| Feature | Tree-walker | VM | Notes |
|---------|:-----------:|:--:|-------|
| Variables / constants | ✅ | ✅ | |
| Primitive types | ✅ | ✅ | |
| String interpolation (any context) | ✅ | ✅ | Sprint 5I |
| Multi-item output `>>` | ✅ | ✅ | All expression types valid |
| Input with prompt | ✅ | ✅ | |
| Arithmetic / comparison / logical | ✅ | ✅ | |
| Compound assignment operators | ✅ | ✅ | |
| if / else-if / else | ✅ | ✅ | |
| match (literal, range, guard, wildcard) | ✅ | ✅ | |
| match multi-value arm | ❌ | ❌ | Not implemented |
| match identifier binding | ❌ | ❌ | Not implemented |
| Loops (all types) | ✅ | ✅ | |
| Range with step and reverse | ✅ | ✅ | Sprint 5I |
| Labeled loops | ✅ | ✅ | Sprint 5I |
| Functions + output params | ✅ | ✅ | |
| Lambdas / closures | ✅ | ✅ | |
| Arrays (full CRUD) | ✅ | ✅ | |
| `arr[i] = val` (direct update) | ✅ | ✅ | Sprint 5I |
| Named tuples | ✅ | ✅ | |
| HOF: map / filter / reduce | ✅ | ✅ | |
| Pipe `\|>` | ✅ | ✅ | |
| Error handling (full) | ✅ | ✅ | |
| Typed catch `:! ##Type` | ✅ | ✅ | |
| Modules (functions via `::`) | ✅ | ✅ | |
| Modules (constants via `.`) | ❌ | ❌ | Known gap |
| Advanced string operators | ✅ | ✅ | |
| Numeric eval / type metadata | ✅ | ✅ | |
| Precision / format / base conversion | ✅ | ✅ | |
| BashExec / Execute script | ✅ | ✅ | |
| CLI args capture `><` | ✅ | — | VM not supported |
| Negative array indices | ✅ | ✅ | `arr[-1]` normalized in both modes (v0.0.2) |
| Destructuring assignment | ✅ | ✅ | `[a, b] = arr`, `(name: n) = t` (v0.0.2) |
