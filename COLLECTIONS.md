# Collections

Zymbol has three, and this document is the point of record for all of them: what
each one is, the rules that govern them, and why each rule was decided the way it
was. The reasoning lives here so the rules do not have to be re-argued from the
code.

The three were redesigned together in v0.0.9 rather than one at a time, which is
the only reason the result is coherent — most of what follows is a single rule
applied three times.

```text
[1, 2, 3]              array        ordered, resizable, homogeneous, mutable
#[1, "dos", 3.0]       array        the same type, with the mix DECLARED
(1, 2)                 tuple        ordered, fixed size, mixed types, IMMUTABLE
(a: 1, b: 2)           dictionary   keyed, insertion-ordered, mutable
```

`(1, 2)` and `(a: 1, b: 2)` share the parentheses and not the semantics. The
colon is the whole of what separates them, and that is accepted deliberately: the
alternative was a notation of its own, and `{}` is not available — it is the
block delimiter of the entire language.

---

## 1. The rule of the result

One rule, and it governs the whole editing family across all three collections.

> A `$` edit whose result is **used** — assignment, argument, `>>`, condition,
> chaining — **builds**, and the original is untouched.
>
> A `$` edit whose result is **discarded** — the `$` is the whole statement —
> **modifies in place**.

```zymbol
arr = [1, 2, 3]

otro = arr$+ 4       // result used      → builds; arr is still [1, 2, 3]
arr$+ 4              // result discarded → arr becomes [1, 2, 3, 4]
```

The two cases are disjoint and are decided by looking at the syntax. And
discarding the result has no other possible use: if you were going to throw it
away, you meant to modify.

**Why it is worth a rule of its own.** Before it, a bare `arr$+ 4` ran and did
nothing at all, without a warning — a line that looks like it does something and
does not. The alternative designs were a second operator for each edit (nine new
marks) or making every edit mutate and losing the pure forms. This costs nothing:
every program written before it used the result, so every program kept meaning
what it meant.

**`$~` takes its value with the unary rule**, so a compound value needs
parentheses:

```text
arr[1]$~ (arr[1] + 5)     // correct
arr[1]$~ arr[1] + 5       // reads only arr[1]
```

### The family, in two halves

| | |
| --- | --- |
| **Edit** — result used builds, discarded modifies | `$+` `$++` `[i]$~` `$-` `$--` `$-[i]` `$+[i]` `$^` `$^+` `$^-` |
| **Consult** — always build; discarding is dead code | `$#` `$?` `$??` `$[..]` `$>` `$\|` `$<` `$/` `$*` `$~~` |

`$|` (filter) and `$[..]` (slice) are the same kind of operation — selecting a
subset — which is why both build.

---

## 2. `=` never writes into a collection

`arr[i] = v` does not exist. Neither does `m[i][j] = v`, nor `d["k"] = v`.

```zymbol
arr[2] = 99
// error: indexed assignment does not exist: 'arr[…] =' is not a form of Zymbol
//   help: use 'arr[i]$~ value' to modify in place — '=' gives a value to a
//         NAME, '$~' changes part of a collection
```

**Why.** `=` means "this NAME now holds this value". `arr[2] = 99` names nothing:
it reaches inside a structure and changes a part of it. Those are two different
operations, and giving them one sign is what makes `a = b` ambiguous about
whether `b`'s insides can still be reached. Every reference language has the
indexed form; this is Zymbol's deliberate divergence.

**The order this had to happen in** is the interesting part. The withdrawal could
not go first: until `arr[2]$~ 99` worked *as a statement* — which is § 1 — the
language would have had no way at all to change an element. So § 1 was built,
then this. Migration was 107 sites across the corpus, the examples, the course
and three applications, and **every golden matched afterwards**, which is the
evidence that the rewrite preserved behaviour rather than the claim that it did.

Nesting is navigated with `>`, not by chaining brackets:

```zymbol
m = [[1, 2], [3, 4]]
m[1>2]$~ 99          // modifies
m2 = m[1>2]$~ 99     // builds
>> m " " m2 ¶
```

---

## 3. The array — `[…]` and `#[…]`

Ordered, resizable, indexed **from 1**, and mutable.

```text
numeros[1]       // first
numeros[-1]      // last
numeros[0]       // error: index 0 is invalid — Zymbol uses 1-based indexing
```

Positive and negative indices are exact mirrors and `0` is not an element. Python
has `a[0]` as first and `a[-1]` as last — asymmetric, with no mirror for 0. JS has
no negative subscript at all.

### `[…]` is homogeneous; `#[…]` declares a mix

```text
[1, 2, 3]                       // checked
[1, "dos", 3.0]                 // error: array element 2 has type String…
#[#0, 1, '2', "tres", 4.0]      // the mix is DECLARED, and not checked
```

**They are the same type.** `#?` answers `##]` for both and every operator behaves
the same. `#` is the meta/type mark and `##]` is already the array type's symbol,
so declaring that an array has an open element type *is* a statement about its
type.

**Why `#[…]` exists at all.** `json::decode` returns a heterogeneous array, so the
language could produce a value it had no way to *write*. The rule had a hole that
could be entered by one door and not the other. Three alternatives were weighed:
make `decode` fail on mixed JSON (the language then cannot read arbitrary JSON),
hand it back as a positional tuple (fixed size, immutable, not walkable — a
500-element JSON would be a 500-element tuple), or drop the homogeneity rule
entirely. `#[…]` keeps the rule and gives the exception a name.

A `#[…]` that turns out homogeneous **warns**. An opt-out of a type discipline
gets used where it is not needed — it happened to `Object` in Java and to `any` in
TypeScript — and the warning keeps it in its place with the same mechanism that
already flags an unused variable.

### The decoder's rule

```
uniform JSON array  →  [ … ]
mixed JSON array    →  #[ … ]
```

---

## 4. The tuple — several values that travel together

Ordered, **fixed size**, mixed types, **immutable**. It is not a small array (an
array resizes and its elements change) and not a record with names (that is the
dictionary).

```zymbol
dividir(a, b) { <~ (a / b, a % b) }
(cociente, resto) = dividir(17, 5)
```

That is what it is for, and it is what the codebase uses it for: 96 multi-value
returns and 118 destructurings.

### Immutable, and the check is on the receiver

```zymbol
t = (1, 2, 3)
t[1] = 99      // error: indexed assignment does not exist
t[1]$~ 99      // error: cannot modify tuple 't': tuples are immutable
t$+ 4          // the same error

u = t[1]$~ 99  // fine — the result is used, so this BUILDS a second tuple
v = t$+ 4      // fine, as (1,2) + (3,) is in Python
```

Immutability is checked **once, on the value**, not taught to each operator
separately. That is why `$+`, `$-`, `$^` and every future edit are covered
without a line each.

### The last name absorbs

```zymbol
larga = (1, 2, 3, 4, 5)
(primero, cola) = larga        // cola = (2, 3, 4, 5)
(p, q, r, cola2) = larga       // cola2 = (4, 5)
```

The remainder **keeps the container's shape** — a tuple leaves a tuple, an array
leaves an array — so the pattern fits again on the next pass. This is the `cons`
pattern, doctrine outside the three reference languages since 1958: `car`/`cdr`
in Lisp, `(x:xs)` in Haskell, `[H|T]` in Erlang, `head :: tail` in Scala,
`[first, rest @ ..]` in Rust. Python and JS have it explicitly (`*rest`,
`...rest`); Zymbol has it implicitly in the last position **and** explicitly.

Here Zymbol is better than Python: `x, *y, z = (0,1,2,3)` leaves `y` a **list**
even though it came from a tuple. And JS cannot do it at all — `const [a, ...b, c]`
is a syntax error.

### `*rest` explicitly, and only one

```zymbol
[a0, *aR, a9] = [0,1,2,3,4,5,6,7,8,9]     // takes from both ends
(t0, *tR, t9) = (0,1,2,"3",'4')            // the rest keeps the shape: a tuple

[a, *r, *s, z] = [1,2,3,4,5]               // error: only one '*rest' …
```

Two rests are ambiguous by definition: nothing says where the first ends and the
second begins. No engine refused it and each invented a different split — one of
them returning the same element **twice**, which cannot be right under any
reading. Python refuses it in analysis; so does Zymbol.

**When the values run out**, destructuring goes left to right and what is not
reached stays empty:

```zymbol
[d0, *dR, d9] = [1, 2]        // d0=1  dR=[2]  d9=##_
```

### `_` discards a position

```zymbol
[a, _, c] = [1, 2, 3]
(x, _, z) = (1, 2, 3)
>> a c x z ¶

filas = [(1, "uno", "I"), (2, "dos", "II")]
@ (_, texto, _):filas {
    >> texto " " ¶
}
```

It works in **both** patterns. It used to work only in the array one, which was
an inconsistency between two patterns that say the same thing. All three
reference languages have it.

---

## 5. The dictionary — addressed by key, and only by key

A tuple with **named** fields. There is no new type and no new notation: what
changed in v0.0.9 is the vocabulary, because "named tuple" stopped being a
defensible name the moment the thing could change — a tuple is immutable by
definition.

```zymbol
u = (nombre: "Ana", edad: 30)
```

Keys are unique, insertion order is preserved (as in Python's `dict` since 3.7
and in a JS object), and values mix types freely — the opposite of the array.

### Reading, including a computed key

```zymbol
u = (nombre: "Ana", edad: 30)
>> u.nombre ¶          // the dot reaches keys that are identifiers
>> u["nombre"] ¶       // the bracket reaches ANY key

clave = "edad"
>> u[clave] ¶          // ← computed
```

The computed key is what makes this a dictionary rather than a record: without
it, a dictionary can only be read by a program that already knows what it holds,
which is the definition of a record. The bracket is also what JSON needs, since a
JSON key can be any string — `u["dos palabras"]` cannot be spelled with the dot.

### An absent key is `##Key`

```text
u["sueldo"]
Runtime error: no key 'sueldo' in dictionary — available: nombre, edad
```

Python's `KeyError`, not JavaScript's `undefined`. It is coherent with `a[0]`,
which is also an error rather than a silently wrong answer — and it is *why*
`$?` has to exist:

```zymbol
u = (nombre: "Ana", edad: 30)
? u$? "sueldo" {
    >> u["sueldo"] ¶
}
```

On a dictionary `$?` asks about the **key**, as `in` does in Python and in JS.
Asking about a value is a different operation and would need its own sign.

### Modifying, adding, removing

```zymbol
u = (nombre: "Ana", edad: 30)
u["edad"]$~ 31            // modifies in place (§ 1)
otro = u["edad"]$~ 32     // builds; u untouched
>> u " " otro ¶

u["ciudad"]$~ "Lima"      // a key that is NOT there gets ADDED
u$-["ciudad"]             // removed by its address, which IS the key
>> u ¶
```

**The contrast with the array is deliberate**: `arr[7]$~ v` on an absent element
*fails*. An array is addressed by POSITION, so writing past the end would leave a
hole — JavaScript's `<3 empty items>` is what that looks like. A dictionary is
addressed by KEY and has no holes to leave.

`$-[…]` already meant "remove by address" for the array (`arr$-[1]`, by
position). In a dictionary the address is the key, so it is the same operator
with the same sense — which leaves `$- value` free to keep meaning "by value" in
both collections.

### Walking

```zymbol
d = (alfa: 10, beta: 20)
@ clave:d {                       // keys, in insertion order
    >> clave " = " d[clave] ¶
}
@ (clave, valor):d {
    >> clave " → " valor ¶
}
```

`@ k:d` yields the **keys** — `for k in d`, as Python spells it. With `d[k]`
available the key is enough to reach the value, so no destructuring pattern had
to be forced into `@`. The pattern form is how you ask for both halves.

### Nesting — this is already JSON

```zymbol
config = (
    servidor: (host: "localhost", puerto: 8080),
    etiquetas: ["web", "api"]
)

>> config.servidor.host ¶
>> config["servidor"]["puerto"] ¶

k1 = "servidor"
config[k1>"puerto"]$~ 9090
```

A **navigation step is an ordinary expression, and its value says how to
address**: an Int is a position, a string is a key. It has to be the value and
not the spelling — a bare identifier inside `[…]` is a *variable*, and that is
exactly what makes a computed key possible. If `config[servidor>…]` meant the key
named `servidor`, then `d[clave]` would mean the key named `clave`.

This is why no new type was needed for JSON.

### A table belongs in a module

A collection literal initialises module state, at any nesting depth. This is
where a lookup table goes: a module is the language's only unit of shared state,
and its functions read the table without anyone passing it down the call chain —
which also avoids the copy that passing a collection to a function costs.

```zymbol
# catalogo {
    #> { IDIOMAS, texto, claves }

    IDIOMAS := ["es", "en"]
    tabla = (es: "hola", en: "hi")

    texto(k) {
        c = k
        ? (tabla$? c) { <~ tabla[c] }
        <~ c
    }

    // The key catalogue is derived from the table, not kept beside it.
    claves() {
        fuera = []
        @ k : tabla { fuera$+ k }
        <~ fuera
    }
}
```

Until v0.0.9 only a scalar was accepted there (E013), so a table had to be
written as a `??` chain inside a function — which cannot be asked what keys it
holds, so every application that had one also maintained a second, hand-written
list of them. See REFERENCE.md L41.

What is still refused is anything that **computes**: `tabla = json::decode(…)`
is a call, and a module body runs nothing. Load it in a function instead.

### The position is not an address

Everything positional is refused:

```zymbol
d[2]     d[-1]     d[2]$~ v     d$-[2]     d$[1:2]     d$[2..3]     d$-[1:2]
// error: a dictionary is addressed by key, not by position
```

**Why the whole family and not just the read.** Adding a key changes what sits at
each position, so a program that depended on `d[2]` stops being correct with
nothing to say so. There is no principled line between "the second key" and "the
first two keys", and a positional **write** — `d[2]$~ v` — is strictly worse than
a positional read: it corrupts data rather than returning the wrong value. This is
Python's position, where `dict` has neither indexing nor slicing.

The slice gets **no** key-based replacement, and that is deliberate: "the first
two keys" is not a question a dictionary should answer.

The **positional tuple keeps the whole family** — there the index is the only
address there is, and the size is fixed.

---

## 6. Value semantics

Assignment copies. Zymbol has no aliasing:

```zymbol
e = [1, 2, 3]
f = e
e[1]$~ 99
>> f ¶        // [1, 2, 3]  ← unaffected
```

Of the three reference languages only Swift does this; Python and JS share. And
it is not expensive: the register VM shares the memory until someone writes.

This is also why § 1's in-place form can be implemented as `name = <the same
expression>` and be *observably* identical — nobody else holds the old value.

---

## 7. Choosing one

```
several values of the SAME type, variable count      → array        [ … ]
several values MIXED in an array, on purpose          → array        #[ … ]
several values that TRAVEL TOGETHER, fixed count      → tuple        ( … )
values with NAMES                                     → dictionary   (k: v)
```

There is no separate `List` type, and there will not be one. No dynamic language
distinguishes List from Array — Python, JS, Ruby, PHP and Lua each have one. The
ones that do distinguish (Java, Swift, TypeScript) check types before running and
buy speed with it; all three Zymbol engines store tagged values, so a second type
would buy no performance and cost a rule.

Sets are not in the language yet, and are a separate question.

---

## 8. Where the evidence is

Every rule here was decided against measurement, and the measurements are kept:

| | |
| --- | --- |
| The decisions, numbered, with their date and cost | `Divergente_ES/forma/README.md` |
| The three collections as executable specifications | `Divergente_ES/forma/{arreglos,tuplas,diccionarios}.zy` |
| The array, measured against Python, JS and Swift | `Divergente_ES/ARREGLOS.md` |
| The positional tuple, same method | `Divergente_ES/TUPLAS.md` |
| The gate: what each engine does with each form | `zyquality/corpus/collections/`, `zyquality/reject/` |

The executable specifications are checked to give **identical output in all three
engines**; a rule that only one engine implements is not a rule yet, and the file
says so where that is the case.
