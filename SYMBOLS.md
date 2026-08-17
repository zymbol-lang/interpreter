# Zymbol — Semiotic and Morphological Reference

> **What this document is.** A description of Zymbol's sign system: the inventory of marks,
> the rules by which marks combine into operators, the meaning each mark contributes, and —
> stated explicitly rather than glossed over — every place where those rules do not hold.
>
> **What it is not.** It is not a tutorial (`GUIDE.md`), not a lookup table of behaviour
> (`REFERENCE.md` §21), and not a grammar (`zymbol-lang.ebnf`). Those three answer *what an
> operator does*. This one answers *why the operator has the shape it has*, and *what shape a
> future operator is allowed to have*.
>
> **How to read it.** Parts I–IV describe the system as it is; Part V says what a future
> operator is allowed to look like. They are written in the present tense and without
> dates, because a sign system is not a changelog. Everything that belongs to time —
> which version coined what, and what this analysis turned up when it was checked against
> the running language — is gathered in [Part VI](#part-vi--diachrony-and-findings) and
> nowhere else.
>
> **Method.** The description is of the implementation, not of earlier documentation. The
> grapheme inventory is the `is_operator_char` predicate in `crates/zymbol-lexer/src/lib.rs`;
> the operator inventory is `TokenKind` in the same file; behavioural claims were executed
> rather than recalled.

---

## Table of contents

**Part I — The sign system**
1. [What kind of sign system Zymbol is](#1-what-kind-of-sign-system-zymbol-is)
2. [The grapheme inventory](#2-the-grapheme-inventory)
3. [Agglutination: the claim and its limits](#3-agglutination-the-claim-and-its-limits)

**Part II — Morphology**
4. [Morpheme classes by position](#4-morpheme-classes-by-position)
5. [Productive processes](#5-productive-processes)
6. [Allomorphy and free variation](#6-allomorphy-and-free-variation)

**Part III — The morpheme lexicon**
7. [How to read an entry](#7-how-to-read-an-entry)
8. [Domain heads](#8-domain-heads)
9. [Operation and modality marks](#9-operation-and-modality-marks)
10. [Structural marks](#10-structural-marks)
11. [Delimiters and literal marks](#11-delimiters-and-literal-marks)

**Part IV — Where the system is not regular**
12. [Allosemy: one mark, host-determined reading](#12-allosemy-one-mark-host-determined-reading)
13. [Declared homographs](#13-declared-homographs)
14. [Opaque signs](#14-opaque-signs)
15. [Natural-language residue](#15-natural-language-residue)
16. [Context restrictions and constraint inheritance](#16-context-restrictions-and-constraint-inheritance)

**Part V — Normative**
17. [Design rules for new operators](#17-design-rules-for-new-operators)
18. [The occupied-combination register](#18-the-occupied-combination-register)

**Part VI — Diachrony and findings**
19. [Diachrony of the sign system](#19-diachrony-of-the-sign-system)
20. [What describing the system found in it](#20-what-describing-the-system-found-in-it)

**Appendices**
- [A. Glossing conventions](#appendix-a--glossing-conventions)
- [B. Grapheme index](#appendix-b--grapheme-index)
- [Notes](#notes)

---

## Part I — The sign system

### 1. What kind of sign system Zymbol is

#### 1.1 Notation beside language

Zymbol's founding constraint — no keywords, in any natural language — is argued in
`GUIDE.md` §0 and not re-argued here. The consequence that matters for this document is
structural: because no construct may be a word, every construct must be a **mark or a
sequence of marks**, and the language therefore needs an explicit account of what its marks
mean and how they combine. Natural-language keywords carry their meaning in from outside;
marks do not. A keyword-free language has to supply that meaning itself, in a document like
this one, or the coherence is only asserted.

#### 1.2 The keyword-free claim, stated precisely

The claim that survives contact with the implementation is:

> **No construct of the grammar is a word.** Control flow, I/O, typing, module structure,
> collection operations and error handling are expressed entirely by marks from a closed
> inventory (§2.1), and that inventory contains no letters.

Three things the claim does **not** cover, all of them real and all of them lexicon rather
than grammar:

| Residue | Example | Why it is not a grammar violation |
|---|---|---|
| Error kind names | `:! ##Index { }` | `##` is the grammar; `Index` fills an open identifier slot |
| Standard-library names | `json::decode_map(s)` | Module paths and function names are identifiers, like any user's |
| Conventional identifiers | `_err` | Not reserved; conventional only |

§15 gives the full residue and argues why eliminating it would cost more than it buys. The
important discipline is that the boundary be **stated**, so that "Zymbol has no keywords"
means something checkable instead of something aspirational.

#### 1.3 Icon, index, symbol

Zymbol's marks are not uniform in how they signify. Using Peirce's three-way distinction as
a classification tool — not as decoration — the inventory splits as follows, and the split
has a practical consequence.

| Mode | How it signifies | Zymbol examples |
|---|---|---|
| **Iconic** | resembles what it means | `>>` `<<` `->` `<~` `\|>` `..` `>` `<` `<>` `><` `##"` `##'` `##]` `##)` `##()` `##->` `#०९#` |
| **Indexical** | points at a context rather than depicting it | `°` `_` `@:label` … `@:label!` |
| **Conventional** | arbitrary; must be learned | `$` `@` `#` `¶` `?` `!` `~` `#1` / `#0` |

The consequence: **iconic signs are self-teaching and conventional ones are not.** A reader
who has never seen Zymbol will guess `>>` and `->` correctly, and will guess `$^-` never.
This document exists for the third row. It is also the reason the third row must stay small:
every conventional mark is a memorisation cost that iconicity does not impose.

Two iconic pairs are worth isolating, because they are minimal pairs that make the principle
falsifiable:

```
>  <          <  >
converging    diverging
= intake at the process boundary    = the two sides differ
= ><  (CLI arguments)               = <>  (not equal)
```

Neither is derivable from a slot template. Both are immediately readable as pictures. §3.4
classifies them accordingly.

---

### 2. The grapheme inventory

#### 2.1 The operator class — closed, 29 characters

This is the complete set of characters that Zymbol reserves for operators. A character in
this set can never appear in an identifier; a character outside it can (subject to §2.3).
The set is the `is_operator_char` predicate, quoted verbatim:

```
>  <  =  !  +  -  *  /  %  ^  &  |  ?  :  .  ,  ;  (  )  [  ]  {  }  @  ~  #  $  ¶  \
```

Twenty-nine marks. Two observations about the composition of the set:

- **Twenty-eight are ASCII punctuation.** The single exception is `¶` (U+00B6). A
  keyboard-reachable inventory was a deliberate constraint, and `¶` is AltGr+R on a Spanish
  layout — which is also why it has an ASCII free variant, `\\` (§6.1).
- **No letters and no digits.** Digits appear inside operators only as *arguments*
  (`#.2\|x\|`, `@~ 500`) or as literal payload (`#1`, `#०९#`), never as the operator itself.

#### 2.2 Three marks outside the operator class

| Mark | Status | Behaviour |
|---|---|---|
| `"` | reserved by scanner precedence | opens a string literal when it *begins* a token |
| `'` | reserved by scanner precedence | opens a char literal when it *begins* a token |
| `°` (U+00B0) | **diacritic on an identifier** | hot-definition marker; see §4.6 |

`°` is the single most unusual mark in the language, and the reason is positional: it is not
an operator token at all. It is not in the set in §2.1, it does not lex as a token, and it
cannot stand alone. It attaches to an identifier — `x°` or `°x` — and modifies where that
identifier's binding is anchored. Morphologically this is a **diacritic**, not an operator,
and the language has exactly one.

#### 2.3 The open class, and a consequence worth declaring

Identifiers are defined negatively: an identifier is any run of characters that is not
whitespace, not a digit of a supported numeral script, and not in the set of §2.1. This is
what makes `変数`, `متغير`, `naïve°` and emoji identifiers work without a per-script
allowlist.

A negative definition draws its boundary somewhere, and here it falls in an unexpected
place:

```zymbol
ab"c" = 5
>> ab"c" ¶        // → 5
```

`"` and `'` are reserved only in **initial** position. Medially they are ordinary identifier
characters, because the negative definition does not claim them.[^quotes] This is a
consequence of the design rather than a defect in it: the alternative is a positive
allowlist, which is the thing the open class exists to avoid.

#### 2.4 How the inventory may grow

The operator class is closed in the sense that adding to it is a deliberate act with a
documented cost, not in the sense that it can never change. §17 rule 5 gives the procedure.
In practice the inventory is close to static: growth happens by combining marks that are
already present, and a genuinely new mark is rare enough that each one is worth naming
individually (§20).

---

### 3. Agglutination: the claim and its limits

#### 3.1 The claim

Zymbol's operators are **agglutinative**: an operator is a sequence of marks, each mark
contributes one meaning, the meanings compose, and the boundaries between marks are visible
in the written form. `<<|?` is not an arbitrary trigraph that happens to mean "poll for a
key"; it is three morphemes.

This is a stronger and more useful claim than "the symbols are consistent", because it is
falsifiable in a specific way: for any operator, either you can segment it and gloss each
segment, or you cannot. Where you cannot, the form is **lexicalized**, and §3.4 says so.

#### 3.2 The slot template

Segmentable operators follow a template. Reading left to right:

```
[BINDER]   DOMAIN   [OPERATION]   [MODALITY]   [ARGUMENT]
```

| Slot | Filled by | Contributes |
|---|---|---|
| BINDER | `:` | "the following domain is bound to a name / a clause" |
| **DOMAIN** | `$` `@` `#` `>>` `<<` `?` `!` | *which world the operation lives in* — required |
| OPERATION | `+ - * / ^ ~ # < > \| . ,` | *what is done in that world* |
| MODALITY | `?` `!` | *how certain / how forceful* |
| ARGUMENT | `[i]`, `(n)`, `\|x\|`, a label, a number | the operand or parameter |

**The modality slot is final.** Across the whole inventory, when `?` or `!` carries modal
force it is the rightmost mark of the operator: `$??`, `$!!`, `<<|?`, `@!`, `##!`, `>>!`,
`>>?`, `@:outer!`. There is no operator in which a modal `?` or `!` is followed by another
operation mark. This is the single most reliable structural generalization in the language.

#### 3.3 The competing principle: iconic placement

The template is not the only ordering rule, and where the two conflict, **iconicity wins**.
Compare:

```
<#   =  IN + META      import        arrow on the left  — flow enters
#>   =  META + OUT     export        arrow on the right — flow leaves
```

Under the template alone this is an inconsistency: one puts the direction mark before the
domain, the other after. Under iconic placement it is a rule: **a direction mark sits on the
edge of the sign that faces the direction it points.** The same rule explains `<~` (returns
leftward, arrow on the left), `->` (enters the body rightward, arrow on the right), `|>`,
`=>`, and the mirrored halves of `<\ … \>`.

Stating both principles, and stating which one wins, is more honest than presenting one
template and treating `<#` as an exception to it.

#### 3.4 Three degrees of transparency

Every operator in the language falls into one of three classes. This classification is what
makes the agglutinative claim precise instead of promotional.

| Class | Definition | Count | Examples |
|---|---|---|---|
| **Transparent** | fully segmentable; meaning = composition of parts | majority | `<<\|?` `$^-` `@:outer!` `##!` `$??` `#.2\|x\|` |
| **Semi-transparent** | segmentable, but the whole means more than the parts | 6 | `!?` `:!` `:>` `\|>` `::` `$++` |
| **Opaque** | not compositional — a single lexical sign, whatever its internal shape | 10 | `¶` `><` `#1` `#0` `0x` `0b` `0o` `0d` `###` `°` |

Worked glosses of the transparent class (conventions in Appendix A):

```
<<      |       ?
IN      UNIT    IRR
"take one unit from the input stream, non-committally"     → poll for a keypress

$       ^       -
COLL    ORDER   REV
"impose an order on the collection, reversed"              → sort descending

@       :outer  !
TEMP    LBL     FRC
"act forcefully on the time-context named outer"           → break the labelled loop

#       #       !
META    TYPE    FRC
"cross to the type level, forcefully"                      → cast to Int, truncating

<<      ##.     (5,2)   "p"     v
IN      TYPE.F  ARG     PROMPT  TARGET
"read inward, constrained to Float with 5 total / 2 decimal digits"
```

Semi-transparent forms, with the surplus stated:

| Form | Segments | Surplus not predictable from the parts |
|---|---|---|
| `!?` | ERR + IRR | that it opens a *block* whose failure is captured |
| `:!` | BND + ERR | that it binds specifically into `_err` |
| `:>` | BND + OUT | that it runs unconditionally after the block |
| `\|>` | GATE + OUT | that the left value is injected as an *argument* |
| `::` | BND + BND | that the left name is a *module namespace* |
| `$++` | COLL + ADD + PL | that it accepts mixed types and stringifies |

---

## Part II — Morphology

### 4. Morpheme classes by position

#### 4.1 Prefix (proclitic to the operand)

`>>` `>>!` `>>?` `>>~` `>>|` `<<` `<<|` `<<|?` `<#` `#>` `<~` `?` `_?` `??` `@` `!?` `##.`
`###` `##!` `#|` `#.N` `#!N` `#,` `#^` `><` `\` `!`

#### 4.2 Postfix (enclitic to the operand)

`$#` `$!` `$!!` `#?` `++` `--` `°` `$~` (on an index expression) `~` and `<~` (on a
parameter name — §9.1)

#### 4.3 Infix

`+ - * / % ^` `== <> < > <= >=` `&& ||` `=` `:=` `+= -= *= /= %= ^=` `=>` `->` `::` `.` `..`
`|>` `:` (range step, for-each, tuple field) `,` `;`

Most `$` operators are **infix in effect though written postfix-then-operand**:
`arr$+ elem`, `s$/ ','`. The collection is on the left, the argument on the right, and the
operator sits between them without whitespace requirements.

#### 4.4 Circumfix (bracketing pairs)

| Pair | Encloses | Note |
|---|---|---|
| `( )` | tuple, call arguments, grouping, `>>~` style slots | |
| `[ ]` | list literal, index, slice, nav path, positional arg to `$+`/`$-` | five roles — see §13.3 |
| `{ }` | block, export list, string interpolation | |
| `\| … \|` | the operand of a format/eval operator: `#.2\|x\|` | a *fence*, not a gate |
| `<\ … \>` | shell command | halves are mirror images |
| `</ … />` | script path | halves are mirror images |
| `#d₀d₉#` | numeral-mode switch | `d₀` and `d₉` stand for the digits *zero* and *nine* of the target script; the payload is a demonstration, see §5.4 |
| `>>\| { }` | TUI region | domain prefix + block |

#### 4.5 Discontinuous with agreement

The labelled-loop construction is the only place where two separated marks must **agree**:

```zymbol
@:timer {          // declaration    @ : LBL
    @~ 60000
    @:timer!       // reference      @ : LBL FRC   — the label must match
}
```

`@:timer` and `@:timer!` are not two independent operators: the second is licensed only by
the first. This is morphological agreement, and it is the reason labelled break/continue is
written `@:timer!` rather than `@!timer` — the latter would put the argument after the
modality slot, violating §3.2.

Agreement is not decoration: a reference whose label matches no enclosing loop is rejected
before the program runs (§16.1). A morphological requirement that nothing enforces is not a
requirement, only a habit.

#### 4.6 Diacritic — the `°` marker

`°` attaches to an identifier and selects the scope its binding is anchored to. Position on
the host is the only distinguishing feature:

| Form | Anchors to | Lifetime |
|---|---|---|
| `x°` (postfix) | nearest enclosing `@` scope | dies with the loop |
| `°x` (prefix) | scope **above** the nearest `@` | survives the loop |

Both auto-initialize to the operation's neutral value on first use (`0`, `1`, `[]`, `""`
depending on context — `GUIDE.md` §4). Outside any loop the two forms coincide.

Two facts make `°` categorically different from every other mark:

1. It is **not in the operator class** (§2.1). It survives inside identifiers by the open-class
   rule, and the lexer strips it after the fact.
2. Its meaning is **purely positional**. No other Zymbol mark changes meaning by moving from
   one side of its host to the other. Writing both (`°x°`) is a diagnosed error.

---

### 5. Productive processes

#### 5.1 Reduplication

Doubling a mark is a productive derivation in exactly **two** domains, `$` and `?`, where it
means *exhaustive / plural / completive*:

| Simplex | Reduplicated | Simplex result | Reduplicated result | Relation |
|---|---|---|---|---|
| `?` if | `??` match | one branch tested | n branches tested | plural |
| `$?` contains | `$??` all indices | `#1`/`#0` | `[2, 3, 5]` | plural |
| `$-` remove first | `$--` remove all | `[1,2,3,2]` | `[1, 3]` | completive |
| `$~` update one site | `$~~` replace | one index | `"banana"` → `"bAnAnA"` | completive |
| `$+` append one | `$++` accumulate | one element | iterated build | iterative |
| `$!` test error | `$!!` propagate | Bool | ejects upward | intensive |

`$!!` is the one member whose surplus is force rather than plurality; it is listed here
because it is derived from `$!` by the same visible process, and flagged so the gloss stays
honest.

**Doubling that is not derivation.** These forms contain a doubled character but are single
lexical signs. In particular `&` alone is *not a token* — `x = 1 & 2` is a lex error — so
`&&` cannot be a derivation of anything.

| Form | Why it is lexical |
|---|---|
| `&&` | `&` does not exist as a simplex |
| `==` `++` `--` `+=` … | inherited arithmetic conventions, not Zymbol derivations |
| `\\` | free variant of `¶`; unrelated to `\` (lifetime end) — see §13.1 |
| `::` | not "twice bound"; a namespace traversal |
| `>>` `<<` | intensification *with category shift*: relation → channel |
| `..` | extension of `.`, but yields a range, not a deeper access |

`##` is the borderline case and belongs in neither table: it *is* compositional — `#` meta,
doubled, gives the type level — but it heads a paradigm of its own rather than deriving
individual operators from `#` ones, so treating it as reduplication would over-claim.

#### 5.2 Modal suffixation

`?` and `!` in final position convert a definite operation into an uncertain or a forced one.
This is the most regular affix in the language:

| Base | `+ ?` | `+ !` |
|---|---|---|
| `<<\|` read a key | `<<\|?` poll for a key | — |
| `>>` write | `>>?` ask the terminal its size | `>>!` force the screen clear |
| `$` on a value | `$?` does it contain | `$!` is it an error |
| `@:label` a loop | — | `@:label!` terminate it |
| `##` a type crossing | — | `##!` truncate rather than round |

#### 5.3 Cross-domain composition

A domain head may take another domain's operator as its argument, without either changing
meaning. `<< ##.(5,2) "p" v` is the input domain hosting a type-domain constraint; the `##.`
means exactly what it means anywhere else.

This is the mechanism by which the language grows without coining anything: a composition
that has never been written still has a meaning, worked out in advance by the marks it is
made of. Adding it is a matter of implementing what the notation already said.

#### 5.4 Elision and demonstration

Two marginal but real processes:

**Elided slots.** `>>~` takes a five-slot tuple in which any slot may be empty, marked only
by its comma: `>>~ (,,, 196) > "red"` sets the foreground and leaves position and style
untouched. The comma is a **zero morpheme** — it holds a position open without filling it.

**Demonstrative payload.** `#d₀d₉#` names a numeral script not by naming it but by
**exhibiting** it: `#०९#` says "Devanagari" by containing Devanagari `०` and `९`; `#09#`
resets by containing ASCII. There is no name to translate and no table to consult, which is
the mechanism working exactly as intended. It is the clearest iconic sign in the language and
the one that best demonstrates the founding principle.

---

### 6. Allomorphy and free variation

Three places where one meaning has more than one form.

#### 6.1 `¶` ~ `\\` — the newline morpheme

Both emit a newline in the output stream; they are interchangeable everywhere. `¶` is the
canonical form and the one the formatter preserves. `\\` exists because `¶` is not reachable
on every keyboard layout. This is free variation in the strict sense: no context distinguishes
them.

#### 6.2 The numeral scripts

`#1` and `#0` accept the digits *of any of the 69 supported scripts* — `#१`, `#١`, `#𝟏` are
the same token as `#1`. Integer literals likewise accept any single script consistently
(`४२` is `42`). One morpheme, sixty-nine graphemic realizations, selected by the author's
script rather than by grammatical context.

#### 6.3 `@label` ~ `@:label` — the fused form

A loop label may be written fused (`@label`) or with the binder made visible (`@:label`).
They lex to distinct tokens (`AtLabel`, `AtColonLabel`) and mean the same thing.

The colon form is canonical, and the reason is morphological rather than aesthetic: it puts
the BINDER slot on the page, so the construction stays segmentable under §3.2. The fused
form hides a morpheme boundary, which is the one thing an agglutinative notation cannot
afford to do often.

Note the whitespace hazard documented in `GUIDE.md` §1b — `@ label` with a space is not a
label at all, but a loop whose first expression is `label`.

---

## Part III — The morpheme lexicon

### 7. How to read an entry

Each domain head below is given as:

- **Gloss** — the abbreviation used in interlinear parses (Appendix A)
- **Contract** — the invariant that every member of the paradigm honours. A proposed operator
  that would break the contract is rejected under §17 rule 2, regardless of how convenient it
  is.
- **Paradigm** — the complete set of forms
- **Restrictions** — where members may and may not appear
- **Exceptions** — members that do not honour the contract, named rather than omitted

---

### 8. Domain heads

#### 8.1 `$` — COLL, the collection domain

**Contract.** `$X` takes a collection on its left, returns a **new** value, and never mutates
the receiver. The mark after `$` names the operation using the same base marks as the rest of
the language.

| Form | Operation | Composition |
|---|---|---|
| `$#` | length | COLL + META → the meta-count |
| `$+` / `$+[i]` | append / insert at index | COLL + ADD (+ position) |
| `$-` / `$--` | remove first / remove all | COLL + SUB (+ PL) |
| `$-[i]` / `$-[i..j]` / `$-[i:n]` | remove at index / range / count | COLL + SUB + position |
| `$?` / `$??` | contains / all indices | COLL + IRR (+ PL) |
| `$[i..j]` / `$[i:n]` | slice inclusive / by count | COLL + span |
| `$^+` / `$^-` / `$^` | sort asc / desc / by comparator | COLL + ORDER (+ direction) |
| `$>` | map | COLL + OUT — each element transformed outward |
| `$\|` | filter | COLL + GATE — only qualifying elements pass |
| `$<` | reduce | COLL + IN — collapse inward to one value |
| `$~~[p:r]` | replace all | COLL + MOD + PL |
| `$/` | split (by char or substring) | COLL + DIV |
| `$*` | repeat (strings) | COLL + MUL |
| `$++` | concat-build | COLL + ADD + PL |
| `$!` / `$!!` | is error / propagate | COLL·value + ERR (+ FRC) |
| `arr[i]$~` | functional update | position + COLL + MOD |

**Exceptions.** `$!` and `$!!` do not take a collection — they take any value, including a
scalar error. They sit in the `$` paradigm only under a weaker reading of `$` as "operate on
the value you have", which no other member needs. One irregular pair in a paradigm of
sixteen is a small price, but it is a price, and the honest way to carry it is to name it
rather than to widen the contract until it covers everything and constrains nothing.

#### 8.2 `@` — TEMP, the temporal domain

**Contract.** Every `@` form operates *within* a time context. `@X` always means "act on the
current time context in way X".

| Form | Operation |
|---|---|
| `@ { }` | infinite loop |
| `@ N { }` | repeat N times |
| `@ cond { }` | while |
| `@ x:arr { }` | for-each |
| `@:label { }` | labelled loop |
| `@!` / `@:label!` | break / labelled break |
| `@>` / `@:label>` | continue / labelled continue |
| `@~ N` | sleep N milliseconds |

**Restrictions.** `@!` and `@>`, labelled or not, are **semantic errors outside a loop**, and
a labelled form is an error unless an *enclosing* loop carries that label. `@~` is not: it
pauses without touching control flow. The line between them, and why it falls there, is
§16.1.

#### 8.3 `#` — META, the meta domain, and `##` — TYPE

**Contract.** `#` signals a boundary crossing: from value-space to type-space, from runtime
value to display representation, or from file to named module. Doubling to `##` moves from
the meta level to the type level proper.

| Form | Operation | Level |
|---|---|---|
| `# name` | module declaration | file → named module |
| `#>` / `<#` | export / import | module surface |
| `#1` / `#0` | Bool literals | typed truth, not integers |
| `#\|x\|` | numeric eval of a string | value → number |
| `x#?` | type metadata → `(symbol, count, display)` | value → meta |
| `#.N\|x\|` / `#!N\|x\|` | round / truncate N decimals | value → display |
| `#,\|x\|` / `#^\|x\|` | comma / scientific format | value → display |
| `#d₀d₉#` | numeral-mode switch | display script |
| `##.` / `###` / `##!` | cast to Float / Int-round / Int-truncate | type crossing |
| `##"` / `##'` | String / Char markers | type, input typespec only |

**The type-symbol paradigm is iconic.** The values returned by `#?` are miniatures of each
type's own notation, which is why they need no table to learn:

| Symbol | Type | Depicts |
|---|---|---|
| `##"` | String | the quote that delimits one |
| `##'` | Char | the quote that delimits one |
| `##]` | Array | the bracket that closes one |
| `##)` | Tuple / NamedTuple | the paren that closes one |
| `##()` | Function | call syntax |
| `##->` | Lambda | definition syntax |
| `##.` | Float | the decimal point |
| `##?` | Bool | the question a Bool answers |
| `##_` | Unit | the non-binding mark |
| `###` | Int | — **not iconic**; Int has no notation to depict |
| `##<Ident>` | Error kind | — **not iconic**; see §15 |

Two of eleven are not iconic, and both are named. `###` is arbitrary and must be memorised;
`##Index` is a word.

#### 8.4 `>>` — OUT, the outward stream

**Contract.** `>>` and its derivations act on the terminal as an output surface. The mark
after `>>` selects *what aspect* of the surface is acted on.

| Form | Operation | Composition |
|---|---|---|
| `>>` | print (juxtaposition, no implicit newline) | OUT |
| `>>!` | clear the screen | OUT + FRC — force the surface to a known state |
| `>>?` | query terminal size (`(H, W) = >>?`) | OUT + IRR — ask the surface a question |
| `>>~ (…) > items` | positioned / styled output | OUT + MOD — modify position and style |
| `>>\| { }` | TUI block: alternate screen + raw mode | OUT + GATE — a controlled region |

The internal `>` in `>>~ (5,10) > "text"` is the direction mark again, introducing the
payload after the style tuple.

#### 8.5 `<<` — IN, the inward stream

**Contract.** `<<` and its derivations pull data into the program. The mark after `<<`
selects the *medium and granularity*.

| Form | Reads | Blocking |
|---|---|---|
| `<<` | one line | yes |
| `<< <typespec> "prompt" var` | one validated value; re-prompts until valid | yes |
| `<<\|` | one keypress | yes |
| `<<\|?` | one keypress if pending, else `'\0'` | **no** |

The typespecs are the `##` cast family placed before the prompt, with an optional size:

| Form | Reads → | Constraint |
|---|---|---|
| `<< ##.(T,D) "p" v` | `Float` | ≤T digits total, ≤D after the point, no exponent |
| `<< ##. "p" v` | `Float` | any valid number |
| `<< ###(N) "p" v` | `Int` | ≤N digits |
| `<< ##"(N) "p" v` | `String` | ≤N characters |
| `<< ##' "p" v` | `Char` | exactly one character |

The size in parentheses is the language's one concession to a named argument on an input
flow. Both engines validate identically; a leading sign does not count toward the digit
budget.

**`<<|` vs `<<|?` — the modal minimal pair.** This is §5.2 applied:

| Form | Reads as | Yields |
|---|---|---|
| `<<\|` | "give me a key" — realis | `Char`, after blocking |
| `<<\|?` | "is there a key?" — irrealis | `Char`, or `'\0'` immediately |

The irrealis form still yields a `Char`; what it cannot do is promise a *meaningful* one, so
it answers with the null character.[^sentinel] Irrealis narrows the guarantee, not the
type — which is what keeps `?` a suffix rather than a different operator.

#### 8.6 `><` — the process boundary

`><` captures the command line into a string array. It is iconic (converging arrows = intake
at the boundary) but **lexicalized**: nothing in the converging shape predicts *command-line
arguments* specifically, as opposed to any other intake. A paradigm of one — the only domain
head that heads nothing.

#### 8.7 `?` — IRR, the irrealis domain

**Contract.** Wherever `?` heads a construct, the outcome is conditional: the result depends
on a question that may return false, empty, or nothing.

| Form | Operation |
|---|---|
| `? cond { }` | if |
| `_? cond { }` | else-if |
| `?? x { pat => val }` | match |
| `$?` / `$??` | contains / all indices |
| `x#?` | type metadata query |
| `!?` | try — the block may or may not throw |
| `<<\|?` | poll for a key |
| `>>?` | query terminal size |

**Exception.** `##?` is the Bool *type symbol*, not a query. It is the answer's type, not a
question — a homograph inside the `#` paradigm rather than a member of this one. See §13.4.

#### 8.8 `!` — the force / error head

`!` is the language's most polysemous mark. Rather than pretend one gloss covers it, §12.1
gives the three readings and the rule that selects between them. As a domain head it appears
in `!?` (try) and licenses `:!` (catch) and `##<Ident>` (error kinds).

---

### 9. Operation and modality marks

#### 9.1 `~` — MOD, modification

**Contract.** `~` marks that something is *changed* or *routed back changed* — never that
something is created.

| Form | What is modified |
|---|---|
| `param~` | the parameter — a **working copy** the body may reassign; the caller's argument is untouched |
| `param<~` | the parameter — by **reference**; the change reaches the caller |
| `<~ value` | the value, routed back to the caller |
| `$~~[p:r]` | the string, by replacement |
| `arr[i]$~` | the collection, at an index (returns a new copy) |
| `@~` | the time flow, paused |
| `>>~` | the output position and style |

**The parameter pair is where the morphology earns its keep.** `a~` and `b<~` differ by one
mark, and that mark is the one that means "flows back":

```zymbol
f(a~)  { a = a + 1  <~ a }     // mutable copy
g(b<~) { b = b + 1 }           // by reference

x = 5
r = f(x)
>> "r=" r " x=" x ¶            // → r=6 x=5   — the caller's x is untouched

y = 5
g(y<~)                         // the mark is required at the call site too (v0.0.9, L36)
>> "y=" y ¶                    // → y=6       — the change travelled back
```

`~` says the parameter may be modified. `<~` says the modification travels back. Nothing
about that has to be memorised separately: it is §9.6 and §3.3 applied to a parameter slot.

**What `~` is for.** It saves the body from opening with a copy it would otherwise have to
write by hand. Without it, a function that needs to work on its argument has to say so:

```zymbol
// the copy, written out
f(a) {
    local = a
    local = local + 1
    <~ local
}

// the same thing, declared
g(a~) {
    a = a + 1
    <~ a
}
```

The two are equivalent, and the second is the point of the mark: a working copy is declared
in the signature rather than assembled in the first line of the body. What it must never do
is reach the call site — that is `<~`'s job, and keeping the two jobs on two marks is why
neither needs a qualifier.

#### 9.2 `|` — GATE, and the fence homograph

**Contract (gate).** `|` controls passage: `$|` filters, `||` admits either alternative,
`|>` passes a value through a function, `<<|` narrows input from lines to one character,
`>>|` opens a controlled screen region.

**Homograph (fence).** In `#.2|x|`, `#,|x|`, `#^|x|`, `#|x|`, the two `|` marks are a
**circumfix delimiter**, not a gate. They enclose an operand. Nothing passes through them.
This is a genuine homograph, resolved by position: a fence always comes in a pair
immediately after a `#`-headed format operator; a gate never does.

#### 9.3 `_` — NBND, non-binding

**Contract.** `_` marks a position that exists syntactically but binds no name. It never
introduces a name into scope.

| Form | Position |
|---|---|
| `_ { }` | else branch |
| `_?` | else-if |
| `_` in `?? x { _ => … }` | match wildcard |
| `[a, _, c] = arr` | destructuring, element skipped |
| `x \|> f(_, 2)` | pipe placeholder |
| `_name` | declared-but-unused identifier prefix |
| `##_` | the Unit type symbol |
| `:! ##_ { }` | catch any error kind |

This is the most regular mark in the language: eight uses, one meaning, no exceptions.

#### 9.4 `:` — BND, binding and naming

**Contract.** `:` introduces or references a *name*, or names a component of a compound
argument.

| Form | Names |
|---|---|
| `:=` | an immutable binding |
| `::` | reach through a module binding |
| `@:label` | a loop |
| `@:label!` / `@:label>` | the loop being targeted |
| `:!` | the error clause |
| `:>` | the cleanup clause |
| `name: value` | a named-tuple field |
| `@ i:arr` | the iteration variable |
| `1..10:2` | the step |
| `$[i:n]` | the count in a slice |
| `$~~[p:r]` | pattern vs replacement |

The last three are weaker: `:` there separates two components of an argument rather than
introducing a name. §13.5 records that as a graded homograph rather than claiming the
contract covers it.

#### 9.5 `=>` — MAP, maps-to

**Contract.** The left side is known internally under one name or shape; the right side is
how it is expressed, matched, or exported. `=` carries the mapping relation, `>` the outward
direction toward the consumer.

| Form | Operation |
|---|---|
| `?? x { pat => val }` | pattern maps to result |
| `<# path => alias` | module becomes known as alias |
| `#> { fn => pub }` | internal name becomes public name |

This completes the arrow paradigm: `->` (into the body), `<~` (back to the caller), `=>`
(across to the consumer).

#### 9.6 `->` and `<~` — the function boundary

`->` points *into* a function body; `<~` points *back out* to the caller. Together they are
the entry and exit marks of the function boundary, and their shapes are iconic of exactly
that.

`<~` occupies two positions, and the reading is the same in both — only what travels back
changes:

| Position | Form | What travels back |
|---|---|---|
| prefix, in the body | `<~ value` | the return value |
| postfix, on a parameter | `f(p<~)` | the modification made to `p` |

**The parameter list may be empty.** `() -> body` is a thunk. This is not a new mark and not
a new reading of `->`: the arrow still points into a body, and what precedes it is still a
parameter list — one that happens to have no members. `()` collides with nothing, because
Zymbol has no empty tuple and a call's parentheses always follow a callable.

#### 9.7 `.` — step into

**Contract.** `.` means "step into": into a structure's member, into the fractional part of a
number, or (doubled) across a span.

| Form | Operation |
|---|---|
| `tuple.field` | step into a field |
| `module.CONST` | step into a module constant |
| `3.14` | step into the fractional part |
| `1..5` | step across a range |

**Note.** Depth navigation into nested collections does *not* use `.` — it uses `>`:
`m[1>2]`, `cubo[1>2>1]`. The mark there is the direction mark, reading "forward into the next
level". This is deliberate (`.` is binary member access; `>` chains) but it means "step into"
has two exponents depending on whether the step is into a *named* member or into an *indexed*
level.

---

### 10. Structural marks

| Mark | Role | Notes |
|---|---|---|
| `=` | assignment | |
| `:=` | constant declaration | |
| `+= -= *= /= %= ^=` | compound assignment | inherited convention (§5.1) |
| `++` `--` | increment / decrement | inherited convention |
| `== <> < > <= >=` | comparison | `<>` iconic: diverging = differing |
| `&& \|\|` | logical AND / OR | `&` alone is not a token |
| `!` | logical NOT | prefix position only |
| `+ - * / % ^` | arithmetic | `+` is numeric only — never string concat |
| `,` | separator; zero morpheme in `>>~` slots | |
| `;` | statement separator; path separator in `arr[p ; q]` | |
| `\ var` | explicit lifetime end | the only *observable* destruction |
| `//`, `/* */` | comments; nesting supported | resolved from `/` by maximal munch |
| `{name}` | string interpolation | identifier only, any script |

---

### 11. Delimiters and literal marks

| Mark | Role |
|---|---|
| `" … "` | String literal; escapes `\n \t \r \" \\ \{ \}`; no `\uXXXX` |
| `' … '` | Char literal — one Unicode character |
| `0x` `0b` `0o` `0d` | base prefixes for character codes: `0x41` → `'A'` |
| `#1` / `#0` | Bool literals, in any of 69 numeral scripts |
| `¶` / `\\` | newline in the output stream (§6.1) |
| `[ … ]` | array literal |
| `( … )` | tuple, named tuple, grouping, arguments |

A tuple is built by the **comma**, not by the parentheses: `(7)` is the Int `7` grouped, not
a one-element tuple. The same pairing governs how a value is taken apart — a `[ … ]` pattern
receives an array, a `( … )` pattern receives a tuple, and the shape a function returns is
the shape that receives it:

```zymbol
f() { <~ (1, 2, 3) }
(a, b, c) = f()      // the receiver mirrors the sender
```

The base prefixes are opaque *and* they are English abbreviations (he**x**, **b**inary,
**o**ctal, **d**ecimal). §15 records them.

---

## Part IV — Where the system is not regular

This part exists because a symbol system whose exceptions are undocumented is not a system —
it is a set of habits. Everything below is a place where "one mark, one meaning" does not
hold, stated with the rule that resolves the ambiguity.

### 12. Allosemy: one mark, host-determined reading

*Allosemy* — the same morpheme read differently depending on what it attaches to — is
distinct from homography, where two unrelated signs share a shape. The marks below are
allosemic: the readings are related, and the **host domain selects** among them.

#### 12.1 `!` — three readings

| Reading | Selected by | Examples |
|---|---|---|
| logical negation | prefix position in an expression | `!flag` |
| force / terminate | word-final after a domain head | `@!` `>>!` `##!` `#!N` |
| error domain | adjacency to `?`, `:`, or `$` | `!?` `:!` `$!` `$!!` |

The three are related (all are decisive rather than tentative) but not interchangeable, and
the selector is the host, not the reader's judgement. Note that force and error are
distinguished purely by domain: in `##!` the `!` truncates, in `$!` it tests for an error, and
the only thing that tells them apart is `#` versus `$`.

#### 12.2 `>` — five readings, one direction

| Reading | Examples |
|---|---|
| comparison — iconic, the wide end faces the larger value | `a > b` |
| outward flow | `>>` `#>` `$>` `\|>` `:>` |
| forward in time | `@>` `@:label>` |
| toward the consumer | `->` `=>` |
| depth step in a nav path | `m[1>2>3]` |

All five are "toward / forward". `>` is polysemous but not homographic: no reading of `>`
contradicts another.

#### 12.3 `~` — modification vs channel

In `$~~`, `arr[i]$~`, `@~` and `>>~`, `~` modifies a thing. In `<~` and `param~` it is closer
to *channel* — the route by which a value travels back. Related, but the second reading is
about transport rather than change, and the contract in §9.1 stretches to cover it.

#### 12.4 `#` vs `##`

`#` is meta level, `##` is type level. Where a form has three, the segmentation is `##` + `#`,
not `#` + `##` — but `###` is the case where segmenting buys nothing: the third `#` does not
mean *Int* by any reading, it is simply the mark that was left. `###` is therefore listed
among the opaque signs (§14) even though its form is decomposable. Segmentability and
compositionality are different properties, and `###` is the language's clearest place where
they come apart. The trigraph is unambiguous in practice only because no other `#`-initial
trigraph exists.

---

### 13. Declared homographs

Unlike §12, these are genuinely unrelated meanings sharing a shape.

#### 13.1 `\` and `\\`

| Form | Meaning |
|---|---|
| `\ var` | destroy the variable now |
| `\\` | emit a newline |

These have nothing in common. `\\` is a free variant of `¶` (§6.1) and `\` is a lifetime
operator. They are told apart by what follows: an identifier versus a second backslash. This
is the sharpest homograph in the language and there is no principled defence of it — it is
the cost of having chosen a keyboard-reachable alternative to `¶`.

#### 13.2 `.` — three meanings

Member access (`t.f`), the decimal point (`3.14`), and — doubled — a range (`1..5`).
Disambiguated by what surrounds it: digits on both sides make it decimal, a second dot makes
it a range, otherwise it is access.

#### 13.3 `[ ]` — five roles

Array literal, index, slice (with `$`), nav path, and positional argument to `$+` / `$-`.
Disambiguated by what precedes the bracket: nothing (literal), an expression (index), `$`
(slice), `$+` / `$-` (position). Inside a nav path the contents follow their own grammar
(`>`, `;`, `..`, nesting).

#### 13.4 `?` in `##?`

`##?` is the Bool type symbol. Every other `?` in the language is irrealis (§8.7). The
justification — "a Bool is what a question yields" — is post-hoc; the real reason is that the
type-symbol paradigm is iconic and `?` was the available mark. Recorded as a homograph.

#### 13.5 `:` in argument-splitting positions

In `1..10:2`, `$[i:n]` and `$~~[p:r]`, `:` separates two components of one argument instead of
introducing a name. Graded homograph: the "names a component" reading is a stretch, and this
document prefers to say so rather than widen the contract until it holds vacuously.

#### 13.6 `^`, `*`, `/`

| Mark | Roles |
|---|---|
| `^` | exponentiation · sort order (`$^`) · scientific notation (`#^`) |
| `*` | multiplication · string repeat (`$*`) · rest pattern (`[a, *rest]`) |
| `/` | division · split (`$/`) · comment (`//`, `/* */`) · script path (`</ … />`) |

Each is resolved by the domain head that precedes it, except `//`, which is resolved by
maximal munch against `/`.

---

### 14. Opaque signs

Ten forms are **not compositional** and must be learned as wholes. Some of them can be cut
into pieces; none of them can be read off those pieces.

| Sign | Meaning | Why it cannot be derived |
|---|---|---|
| `¶` | newline | a logogram; the pilcrow *is* the paragraph mark |
| `><` | CLI arguments | iconic of intake, but nothing predicts "command line" |
| `#1` / `#0` | true / false | `#` + digit is a convention, not a composition |
| `0x` `0b` `0o` `0d` | base prefixes | English abbreviations (§15) |
| `###` | Int cast / Int type symbol | segmentable as `##` + `#`, but the third mark carries no meaning (§12.4) |
| `°` | hot definition | diacritic; meaning is positional, not compositional (§4.6) |

Listing them is the point. Ten opaque forms against the 97 operator forms catalogued in
`REFERENCE.md` §21 is a defensible ratio — and a ratio is only defensible once someone has
counted it. A symbol system that never counts its opaque signs will always believe it has
few.

---

### 15. Natural-language residue

Design rule 4 (§17) forbids natural-language words in the grammar. The rule holds. What
follows is everything in the language that *is* a word, so the rule's scope is unambiguous.

| Residue | Form | Assessment |
|---|---|---|
| **Error kinds** | `##IO` `##Network` `##Parse` `##Index` `##Type` `##Div` `##_` | Six English words plus `##_`, the one symbolic member. `##` is grammar; the name after it is an open identifier slot — the parser accepts *any* identifier, including `##Índice`, which simply never matches at run time. |
| **Standard library** | `std/math` `std/random` `std/json` `std/io` `std/net` `std/term` `std/db`, and every function in them | Module paths and function names. Identifiers, addressed the same way user modules are. |
| **Base prefixes** | `0x` `0b` `0o` `0d` | Abbreviations of hex / binary / octal / decimal. The most avoidable item on this list, and the most entrenched. |
| **Conventional identifier** | `_err` | Not reserved; a convention the catch clause populates. |

**Why this is the right boundary.** Symbolising the residue would mean coining a mark per
error kind and per library function — an inventory that grows without bound, in a system whose
value comes from its inventory being small and closed. The symbol-vs-module rubric already
draws this line for capabilities: *a named operation on an addressed resource* (a path, a URL,
a connection) is a module call; symbols are reserved for ambient process flows (`>>`, `<<`,
`><`, `<\ \>`). Error kinds are named resources by the same test.

**What would be a real violation.** A control-flow construct, a type, an operator or a
declaration spelled with letters. There are none, in any version.

---

### 16. Context restrictions and constraint inheritance

Some marks are legal only in specific contexts. Where the restriction follows from the domain
rather than from the individual operator, it is **inherited** — which is what makes it
predictable for operators that do not exist yet.

#### 16.1 Inherited: the `@` loop-context rule

The rule is not "these operators happen to require a loop". It is: **an `@`-prefixed
statement that acts on the loop's control flow is invalid outside one.** The clause about
control flow is what does the work, and it is what decides which members inherit:

| Statement | What it does | Acts on control flow? | Needs a loop |
|---|---|---|---|
| `@!` | breaks | yes — leaves the loop | **yes** |
| `@>` | continues | yes — jumps to the next iteration | **yes** |
| `@:L!` / `@:L>` | breaks/continues a named loop | yes, and the name must resolve | **yes**, labelled `L` |
| `@~ N` | pauses for N ms | **no** — execution resumes where it was | no |

```zymbol
@:timer {
    @:timer!       // labelled break — needs an enclosing loop named 'timer'
}

@~ 500             // legal at top level: a pause is not a jump
```

`@~` is the member that shows the rule is about control flow and not about the `@` prefix.
It is temporal, it is spelled with `@`, and it inherits nothing — because a pause resumes
where it left off, and a construct that does not move the loop's control flow has no reason
to require one.[^atsleep]

**A function or lambda body is a boundary.** The caller's loops are not in scope inside a
callee, so this is an error even though every call site is inside a loop:

```zymbol
f() { @! }              // error: '@!' outside a loop
@ i:1..3 { f() }
```

This follows from the frames rather than being stipulated on top of them: a callee has its
own scope, and a loop context is part of a scope. Morphology and runtime agree here, which
is the normal case and worth noticing when it holds.

#### 16.2 Per-operator restrictions

| Restriction | Applies to |
|---|---|
| function body only | `<~` |
| requires raw mode from an enclosing `>>\|` | `<<\|`, `<<\|?` |
| requires a TTY; errors on redirected output | `>>\|` |
| input typespec position only | `##"`, `##'` |
| statement position only | `<<`, `<<\|`, `<<\|?`, `><` |
| top level of a match arm only | `\|\|` as an or-pattern — list elements stay primary patterns, so `[1, 2]` is never ambiguous with two alternatives |
| parenthesise postfix operators inside `>>` | `(arr$#)` |

---

## Part V — Normative

### 17. Design rules for new operators

Each rule states what it forbids, why, and how to check a proposal against it.

**1 — Derive, do not invent.**
A new operator must be explainable as a composition of marks already in the inventory.
*Check:* write the interlinear gloss (Appendix A). If every segment has an existing gloss and
the composition yields the intended meaning, the operator is derivable.
*Example:* `<<|?` = IN + UNIT + IRR needs no new mark. Neither do typed input, `||` in
patterns, or `##!` on `Char`.

**2 — One abstract meaning per base mark.**
A new use of an existing mark must fit that mark's contract (Part III).
*Check:* find the contract, apply it to the proposal, and see whether the sentence is true.
`~` means modification; a new `~X` must involve transforming something.

**3 — Context constraints are inherited, not restated.**
If a domain carries a restriction, every new member of that domain carries it.
*Check:* §16.1. A new `@`-statement acting on the time context is invalid outside a loop, and
this is not a decision to be re-made.

**4 — No natural-language words in the grammar.**
Not in English, not in any language. Control flow, types, operators and declarations are
marks.
*Scope:* the grammar, not the lexicon. Identifiers are free; module and function names are
identifiers; error kinds fill an identifier slot. §15 is the exhaustive residue, and any
addition to it is a change requiring the same scrutiny as a new mark.

**5 — No new base mark without a documented abstract character.**
If no existing mark fits, the new mark's meaning is defined in this document *before* it is
implemented.
*Check:* the mark has a gloss, a contract, a paradigm it heads or joins, and a position class.
*Why the order matters:* a mark that ships before it is described acquires its meaning from
whatever the first few uses happen to be, and that meaning is then very hard to correct. The
description is the design; the implementation follows it.

**6 — No mark may carry two unrelated meanings.**
*Check:* if the two readings cannot be stated as one contract, they are homographs, and a
homograph is a defect to be paid down, not a feature to be documented and forgotten.
*Standing debt:* §13 lists six. `\` / `\\` (§13.1) is the one worth retiring.
*Example of payment:* `<=` once meant both "less than or equal" and "known as" in module
aliases. The two readings share no contract, so the second was moved to `=>`, where the
outward arrow says what the mapping does. `<=` is now exclusively comparison.

**7 — Prefer iconic over conventional.**
When two derivable forms are available, choose the one whose shape depicts its meaning.
*Rationale:* §1.3 — iconic signs cost nothing to learn and conventional ones cost a lookup.
*Examples:* `<>` for inequality rather than `!=`; `><` rather than a lettered form; the whole
`##`-type paradigm.

**8 — Modality goes last.**
A modal `?` or `!` is the rightmost mark of the operator (§3.2). An argument or label never
follows it.
*Example:* this is why labelled break is `@:outer!` and not `@!outer`.

---

### 18. The occupied-combination register

Consult before designing a new operator. Every combination the language currently spends
is listed here; anything absent is unspent.

#### `>>` — outward stream
| Form | Meaning |
|---|---|
| `>>` | print |
| `>>!` | clear screen |
| `>>?` | query terminal size |
| `>>~` | positioned / styled output |
| `>>\|` | TUI block |

#### `<<` — inward stream
| Form | Meaning |
|---|---|
| `<<` | read line |
| `<< ##.` / `<< ###` / `<< ##"` / `<< ##'` | typed input |
| `<<\|` | read key, blocking |
| `<<\|?` | read key, non-blocking |

#### `@` — time / loop context
| Form | Meaning |
|---|---|
| `@` | loop (infinite / N / while / for-each) |
| `@!` / `@>` | break / continue |
| `@:label` | labelled loop |
| `@:label!` / `@:label>` | labelled break / continue |
| `@label` | fused label (§6.3) |
| `@~` | sleep |

#### `#` — meta / type
| Form | Meaning |
|---|---|
| `#` | module declaration |
| `#>` / `<#` | export / import |
| `#1` / `#0` | Bool literals |
| `#\|` | numeric eval |
| `#?` | type metadata (postfix) |
| `#.N` / `#!N` | round / truncate N decimals |
| `#,` / `#^` | comma / scientific format |
| `#d₀d₉#` | numeral mode |
| `##.` / `###` / `##!` | casts |
| `##"` / `##'` | String / Char markers — input typespec position only |
| `##]` `##)` `##()` `##->` `##?` `##_` | type symbols, `#?` results only |
| `##<Ident>` | error kind |

#### `$` — collection
| Form | Meaning |
|---|---|
| `$#` | length |
| `$+` / `$+[i]` | append / insert |
| `$-` / `$--` / `$-[i]` / `$-[i..j]` / `$-[i:n]` | remove variants |
| `$?` / `$??` | contains / all indices |
| `$[i..j]` / `$[i:n]` | slice variants |
| `$^` / `$^+` / `$^-` | sort variants |
| `$>` / `$\|` / `$<` | map / filter / reduce |
| `$~~` | replace all |
| `$/` | split |
| `$*` | repeat |
| `$++` | concat-build |
| `$!` / `$!!` | is error / propagate |
| `$~` | functional update (postfix on an index) |

#### `~` — modification
| Form | Meaning |
|---|---|
| `<~` | return / output parameter |
| `param~` | mutable parameter |
| `$~` / `$~~` | functional update / replace |
| `@~` | sleep |
| `>>~` | positioned output |

#### `!` — force / error
| Form | Meaning |
|---|---|
| `!` | logical NOT |
| `@!` | break |
| `!?` / `:!` | try / catch |
| `$!` / `$!!` | is error / propagate |
| `##!` / `#!N` | truncating cast / truncate decimals |
| `>>!` | clear screen |

#### `=>`, `->`, `<~`, `\|>` — arrows
| Form | Meaning |
|---|---|
| `=>` | match arm, import alias, export rename |
| `->` | lambda |
| `<~` | return |
| `\|>` | pipe |

#### Unassigned but reachable
`&` (simplex) is a lex error today and is therefore free. `>>=`, `<<=`, `@?`, `$&`, `#&` and
`##&` are unoccupied. Any of them is available to a proposal that survives §17.

---

## Part VI — Diachrony and findings

Everything with a date in it lives here. Parts I–V describe a system; this part records how
it got that way and what checking it against the running language turned up.

The separation is deliberate. A sign system read as a whole should read as a whole — not as
prose interrupted every few paragraphs by a note about which version got something wrong.
Those notes are worth keeping; they are just not worth reading first.

### 19. Diachrony of the sign system

Only changes to the *sign system* are recorded here; feature and fix history is in
`CHANGELOG.md`. The pattern worth noticing is the second column: growth is almost entirely
recombination, and a genuinely new mark has happened once in five versions.

| Version | Change to the sign system |
|---|---|
| **v0.0.5** | One new base mark — `°`, the hot-definition diacritic, with two positional readings (§4.6) — and seven derived operators: the TUI family `>>!` `>>?` `>>~` `>>\|`, the key-input pair `<<\|` `<<\|?`, and `@~`. |
| **v0.0.6** | No new mark. `=>` unified as the single "maps to" separator across match arms, import aliases and export renames (breaking): `pat : result` → `pat => result`, `<# path <= alias` → `=>`, `#> { fn <= pub }` → `=>`. Retired the `<=` dual role, discharging a rule-6 violation. |
| **v0.0.7** | No new mark. Typed input by composition — `<< ##.(5,2)`, `<< ###(4)`, `<< ##"(20)`, `<< ##'` — which newly occupied `##"` and `##'`. Standard library established as modules rather than symbols, per the symbol-vs-module rubric. |
| **v0.0.8** | No new mark. `\|\|` extended to match arms as an or-pattern, recognised only at the top level of an arm. `##!` extended to `Char` → code point (`##!'A'` → `65`), the only direct Char→Int route. `std/term` added as a module, deliberately not as symbols. `.zyp` packaging added with no language surface at all. |
| **v0.0.9** | No new mark. `->` accepts an empty parameter list: `() -> body` is a thunk (§9.6) — a change to what may fill the slot before the arrow, not to the arrow. Two enforcement changes with no surface at all: `@!`/`@>` and labelled jumps became semantic errors (§16.1), and the browser engine started checking argument counts. |

---

### 20. What describing the system found in it

Parts I–V were written by checking every claim against the four engines rather than against
the previous edition of this document. That turned out to be a way of finding defects, which
was not the intention. They are recorded here because the *kind* of defect is instructive:
each one is a place where the notation said something the implementation did not.

#### 20.1 Defects in the language, found by describing it

| Finding | What the description exposed | Status |
|---|---|---|
| `@:label!` with an unresolvable label | Agreement (§4.5) is a morphological requirement, and nothing enforced it. Four engines, four behaviours — the tree-walker unwound every enclosing loop in silence. | Semantic error in all four (REFERENCE.md L29) |
| `@!` / `@>` outside any loop | The same gap, unlabelled. | Same fix |
| `@~` outside a loop | The reverse: documented as constrained, never constrained by anything. Writing down *why* a member inherits a restriction (§16.1) showed this one has no reason to. | Documentation corrected, not the code |
| `() -> body` | The parameter slot before `->` was described as needing at least one member. Nothing required that, and two engines already ran the empty form. | Legal everywhere (REFERENCE.md L30) |
| Argument counts in `zymbol.js` | Not a notational finding — found by the same four-engine runs. | Checked (REFERENCE.md L31) |
| `->  {` in the formatter | Two spaces after a block lambda's arrow. §4.7 of `FORMATTER_RULES.md` said one. | Fixed |

The common shape: **a rule that is written down but not enforced is not a rule.** Three of
the six were rules this document already stated (label agreement, the loop-context
constraint, and `@~`'s supposed share of it); a fourth was stated by the grammar, and the
implementation did not match it either. Stating a rule and checking it are different acts,
and only the second one holds.

The tooling that made it findable is `tests/scripts/engine_compare.sh`, which runs a program
through every engine at once — the tree-walker, the register VM, `zymbol.js`, and zyml until
it was retired on 2026-08-17. Every existing suite compares a *pair* of engines, and a pair
can hold at most two of four disagreeing answers; having a fourth answer is what made these
findable.

#### 20.2 Defects in this document, found the same way

Recorded so the failure mode is legible rather than tidied away.

- `°` was absent, though it is the most recent base mark the language coined and the only
  diacritic it has.
- The `>>` paradigm listed one member; it has five. `>>!`, `>>?`, `>>~` and `>>|` appeared
  neither in the family section nor in the occupied register — so §18, whose whole purpose
  is to prevent collisions, was advertising four combinations as free.
- `<<|?` was documented as yielding `''`, which is not a valid Zymbol `Char` literal and
  never was the runtime value.
- `><`, `\`, `\\`, `$*`, `&&`, `<>`, `;` and the arithmetic and comparison marks had no
  entry anywhere.
- Design rule 6 said the `<=` dual role was "scheduled for correction" while the section
  above it said the correction had shipped. It had shipped.

Every one of these is the same failure: the document was maintained against itself. The
`Method` note at the top exists to stop that recurring.

---

## Appendix A — Glossing conventions

Interlinear glosses in this document use the following abbreviations. A gloss line aligns one
abbreviation per morpheme, in source order.

| Gloss | Morpheme | Gloss | Morpheme |
|---|---|---|---|
| COLL | `$` collection domain | IRR | `?` irrealis / uncertain |
| TEMP | `@` time domain | FRC | `!` force / terminate |
| META | `#` meta level | ERR | `!` error domain |
| TYPE | `##` type level | PL | reduplication: exhaustive / plural |
| IN | `<` `<<` inward flow | MOD | `~` modification |
| OUT | `>` `>>` outward flow | GATE | `\|` gate |
| UNIT | `\|` single-unit granularity | NBND | `_` non-binding |
| BND | `:` binding | LBL | a loop label |
| MAP | `=>` maps to | DEEP | `>` depth step in a nav path |
| ADD / SUB | `+` / `-` | ORDER / REV | `^` / `-` in sort forms |

Example:

```
arr  $     ?      ?
     COLL  IRR    PL
     "ask the collection where, exhaustively"     → all indices of a value
```

---

## Appendix B — Grapheme index

Where each mark of §2.1 is treated.

| Mark | Primary treatment | Also |
|---|---|---|
| `>` | §12.2 direction | §8.4, §9.7 |
| `<` | §8.5 inward | §12.2 |
| `=` | §10 | §9.5 `=>` |
| `!` | §8.8, §12.1 | §5.2 |
| `+` `-` | §10 arithmetic | §8.1 `$+` `$-` |
| `*` | §13.6 | §8.1 `$*` |
| `/` | §13.6 | §8.1 `$/` |
| `%` | §10 | |
| `^` | §13.6 | §8.1 `$^` |
| `&` | §5.1 — `&&` only; `&` is free | §18 |
| `\|` | §9.2 gate vs fence | §8.5 |
| `?` | §8.7 irrealis | §5.2, §13.4 |
| `:` | §9.4 binding | §13.5 |
| `.` | §9.7 step into | §13.2 |
| `,` | §10, §5.4 zero morpheme | |
| `;` | §10 | |
| `( )` | §4.4, §11 | |
| `[ ]` | §13.3 five roles | §4.4 |
| `{ }` | §4.4 | §10 interpolation |
| `@` | §8.2 temporal | §16.1 |
| `~` | §9.1, §12.3 | |
| `#` | §8.3 meta | §12.4 |
| `$` | §8.1 collection | |
| `¶` | §6.1, §14 | |
| `\` | §13.1 | §6.1 |
| `°` | §4.6 diacritic | §2.2 |
| `"` `'` | §2.2, §2.3 | §11 |

---

## Related documents

| Document | Answers |
|---|---|
| `GUIDE.md` | How to write Zymbol; the authoritative language reference |
| `REFERENCE.md` §21 | What each operator does — the lookup table |
| `REFERENCE.md` §20 | Known limitations and their status |
| `IMPLEMENTATION.md` | EBNF grammar, feature coverage, engine internals |
| `zymbol-lang.ebnf` | The normative grammar |
| `MEMORY_MODEL.md` | Scoping and lifetime semantics behind `°`, `\`, `~` |
| `FORMATTER_RULES.md` | How the marks are laid out on the page — spacing, blocks, blank lines |
| `CHANGELOG.md` | Full version history |

---

## Notes

[^quotes]: `ab"c" = 5` binds an identifier whose name contains two quote marks, and
`>> ab"c" ¶` prints `5`. The scanner reaches its string and char branches before its
identifier branch, so `"` and `'` are reserved at the start of a token; `is_ident_continue`
never excludes them, so medially they are not.

[^sentinel]: `'\0'`, the null character, from `crates/zymbol-interpreter/src/io.rs`. A
program distinguishes "no key" from a real keypress by comparing against it:
`? k <> '\0' { … }`.

[^atsleep]: No engine has ever required a loop around `@~`, in any version. The constraint
was asserted by inheritance from the `@` prefix rather than derived from what the statement
does — see §20.1.
