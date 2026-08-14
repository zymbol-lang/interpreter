# Building a Multilingual Application in Zymbol

[`I18N.md`](I18N.md) answers two language questions: how a team reads and writes
*code* in their own language (re-export layers), and how a program looks up *text*
in the user's language (a dispatcher module). Both are mechanisms.

This document answers a third question, which is not about mechanism but about
architecture: **what does an application have to do so that changing its language
does not break it?** A lookup table is the easy part. The hard parts are the
consequences — a translated string is a different width, a different grammar, a
different number format, and it arrives at runtime, after the layout was decided.

The reference implementation is [囲碁](https://github.com/zymbol-lang/zy-GO), a
terminal Go game in five languages (Japanese, Korean, Mandarin, English, Spanish).
Every rule below is either taken from it or was found by auditing the two projects
that came before it.

> **Every code example in this document has been executed** in both the
> tree-walker and the register VM, and every module file shown passes
> `zymbol check` with no errors or warnings.

---

## Contents

1. [The three axes](#1-the-three-axes)
2. [The dispatcher contract](#2-the-dispatcher-contract)
3. [Keys](#3-keys)
4. [Composed messages](#4-composed-messages)
5. [Column metrics](#5-column-metrics)
6. [Frames that fit any language](#6-frames-that-fit-any-language)
7. [Switching language at runtime](#7-switching-language-at-runtime)
8. [Entry points per language](#8-entry-points-per-language)
9. [Identifier-level API layers](#9-identifier-level-api-layers)
10. [The completeness gate](#10-the-completeness-gate)
11. [Documentation and locale codes](#11-documentation-and-locale-codes)
12. [Audit checklist](#12-audit-checklist)
13. [Case studies](#13-case-studies)
14. [Third mechanism: the digit script](#14-third-mechanism-the-digit-script)

---

## 1. The three axes

An application can be internationalized along three independent axes. Most
projects do the second and stop.

| Axis | Question | Mechanism | Covered by |
|------|----------|-----------|------------|
| **Code** | Can a developer work against this engine without reading its base language? | Re-export layers | [`I18N.md`](I18N.md) Part 1, §9 below |
| **Text** | Can the user read this application in their language? | Dispatcher + key catalogue | [`I18N.md`](I18N.md) Part 2, §2–§4 below |
| **Presentation** | Does the application still *work* when the text changes? | Measured layout, hot switching, a gate | This document, §5–§10 |

Numbers ride along the Text axis, but through a mechanism of their own: the
digit script is a process-global runtime mode the dispatcher can set once, so
`42` prints as `४२` or `۴۲` without any call site knowing. See §14.

The third axis is the one that gets skipped, and it is the one that decides whether
adding a fourth language is an afternoon or a rewrite. In a terminal application it
is not a cosmetic concern: a panel that is two columns too wide corrupts the frame
around it.

---

## 2. The dispatcher contract

One module owns the active locale. It holds it as **module state**, which Zymbol
shares per file path — so every module that imports the dispatcher sees the same
selection, and no function has to carry a language parameter.

```zymbol
# .言語_取次 {

    <# ./日本語 => 日
    <# ./English => 英
    <# ./Español => 西

    #> { 設定, 現在, 言語一覧, 鍵一覧, 語, 路盤名, 結果文, 取石文 }

    現在言語 = "ja"

    言語一覧() { <~ ["ja", "en", "es"] }
    設定(コード) { 現在言語 = コード }
    現在() { <~ 現在言語 }

    語(鍵) {
        ?? 現在言語 {
            "en" => { <~ 英::語(鍵) }
            "es" => { <~ 西::語(鍵) }
            _    => { <~ 日::語(鍵) }
        }
    }
}
```

*(`GO/言語/取次.zy`, trimmed to three locales.)*

Every locale module implements the **same function contract** — one static lookup
plus one function per composed sentence — and the dispatcher is the only place that
knows which locale is active.

### Never thread the locale as a parameter

This is the single most consequential rule, and the one both earlier projects broke.

```zymbol
// ✗ the locale is not part of the domain
dibujar_panel(puntos, vidas, ancho, alto, idioma)

// ✓ the locale is ambient
dibujar_panel(puntos, vidas, ancho, alto)
```

A threaded parameter looks harmless in the first module and becomes structural by
the fifth. Every signature grows an argument that says nothing about the game;
every new screen is one more chance to forget to pass it; and the language can no
longer be changed from inside the application without unwinding the call stack.
Module state costs nothing and removes the whole class of problem.

> Measured on the reference project: `表示/描画.zy` uses the dispatcher 22 times,
> `対局.zy` 31 times, `棋戦.zy` 11 times. Not one of those 64 call sites passes a
> locale.

---

## 3. Keys

### Keys are written in the base language

The base language is the language the program is *written* in — the one its
identifiers use. If the code is Japanese, the keys are Japanese. Do not invent a
neutral ASCII key layer: it adds a fourth language that nobody speaks and that
every translator has to learn.

The base language's own locale module is then a table that unfolds a concept into
that language's prose. It is not an identity function, and it must not be.

### Every key carries a domain prefix

`終局.石`, never plain `石`. `menú.título`, never `título`.

This is not tidiness. It is what makes the completeness check **decidable**. The
missing-translation fallback returns the key itself (§10), so "translation equals
key" is the test for "missing". Without a prefix, a Japanese key `終局` whose
Japanese translation is legitimately `終局` is indistinguishable from a missing
entry — and the check silently stops working in the base language, the one language
where a missing string is hardest to notice by eye.

### One catalogue, published by the dispatcher

```zymbol
鍵一覧() {
    <~ [
        "言語.名前",
        "品書.表題", "品書.路盤", "品書.棋力", "品書.手番",
        "区画.手番", "区画.手数", "区画.アゲハマ",
        "終局.表題", "終局.石", "終局.地", "終局.合計",
        "操作.移動", "操作.着手", "操作.パス", "操作.投了"
    ]
}
```

The catalogue is the contract every locale must satisfy, and the input to the gate.
A key that is not in it is not tested and will eventually be wrong.

---

## 4. Composed messages

A static table cannot produce a sentence whose grammar depends on its own values.
Any string with a number in it is such a sentence.

```zymbol
// English locale
_点数(差) {
    整 = ##!差
    残 = 差 - ##.整
    ? 残 > 0.4 { <~ "{整}.5 points" }
    ? 整 == 1 { <~ "1 point" }
    <~ "{整} points"
}

結果文(勝色, 差, 中押) {
    ? 中押 { ? 勝色 == 1 { <~ "Black wins by resignation" }
             <~ "White wins by resignation" }
    ? 差 == 0 { <~ "Draw" }
    点 = _点数(差)
    ? 勝色 == 1 { <~ "Black wins by {点}" }
    <~ "White wins by {点}"
}
```

Each locale composes the sentence its own way. The same three arguments produce:

| Call | ja | ko | zh | en | es |
|------|----|----|----|----|----|
| `路盤名(9)` | 九路盤 | 9줄 바둑판 | 九路棋盘 | 9×9 | 9×9 |
| `結果文(2, 1.5, #0)` | 白の1目半勝ち | 백 1집반승 | 白胜1目半 | White wins by 1.5 points | las blancas ganan por 1,5 puntos |
| `取石文(3)` | 3子を取った | 3점을 따냈습니다 | 提3子 | captured 3 stones | captura 3 piedras |

No table could have done this. Japanese and Chinese write the half point as 半
rather than as a decimal, Korean counts in 집, English inflects the plural, Spanish
uses the decimal comma. **Anything you would be tempted to build with string
concatenation at the call site belongs in the locale as a function.**

### Numbers can be a locale concern too

A number is not always ASCII. Klingon writes its digits in pIqaD (U+F8F0–F8F9), so
Hov veS carries a third contract function beside the lookup and the sentence:

```zymbol
mI'(n)      // 7 → "7" in English and Spanish, → ⟨pIqaD seven⟩ in Klingon
```

The caller never has to know which locale is active, which is the whole point.

> Interpolation accepts exactly what the lexer accepts as an identifier anywhere
> else, so `"{⟨pIqaD name⟩}"` works like `"{整}"` does. Before v0.0.8 it used a
> narrower rule and rejected anything outside Unicode's `L*`/`N*` categories,
> which meant a program written in pIqaD had to compose with `$++` instead —
> see HLZ-KL-001.

---

## 5. Column metrics

> **Grapheme count is not column count.** `$#` counts graphemes. CJK ideographs,
> kana, Hangul syllables and most emoji occupy **two terminal columns each**.

Since v0.0.8 the standard library answers the column question:

```zymbol
<# std/term => t

t::width("abc")        // 3
t::width("手番")       // 4   ← $# would say 2
t::width("go碁🌑")     // 6   ← 1 + 1 + 2 + 2
t::pad_right("負ける", 12)   // 6 columns of content + 6 spaces
t::center("go", 10)          // "    go    " — a spare column goes right
t::truncate("形勢判断形勢", 6)  // "形勢判" — never splits a wide glyph
```

**Every width calculation in a multilingual application goes through
`std/term::width`.** Not `$#`, and never a constant typed by counting characters in
an editor.

If the application is written in a language other than English, wrap the standard
library in a thin localized layer rather than sprinkling English identifiers
through the render code:

```zymbol
# .標準_端末 {
    #> {
        端::width     => 幅
        端::pad_left  => 左詰
        端::pad_right => 右詰
        端::center    => 中央
        端::truncate  => 切詰
    }
    <# std/term => 端
}
```

*(`GO/標準/端末.zy` — the code-i18n mechanism of [`I18N.md`](I18N.md) Part 1 applied
to the standard library. Zero runtime cost.)*

---

## 6. Frames that fit any language

This is the section the two earlier projects needed and did not have.

### The anti-pattern

```zymbol
>>~ (fila+4, col, 0, C) > "│   Elige tu velocidad:        │"
>>~ (fila+6, col, 0, C) > "│   [1]  Lento      160 ms     │"
? sel == 1 {
    >>~ (fila+6, col, 0, V) > "│ ► [1]  Lento      160 ms     │"
}
```

The padding lives inside the same string as the text. Translating the text
desynchronizes the frame, so either every row of every screen is retyped for every
language, or the translation is cut down until it fits — `(difícil)` becomes
`(difíc)` because there was no room. Five options in two selection states is ten
literals for a five-line menu.

### The pattern

Measure the content, then build the frame around it.

```zymbol
# .marco {

    <# std/term => t

    #> { construir, ancho_de }

    // Widest line in a list, in terminal columns.
    ancho_de(líneas) {
        w = 0
        @ l : líneas {
            c = t::width(l)
            ? c > w { w = c }
        }
        <~ w
    }

    // Frame a list of lines. Returns rows that are all exactly the same
    // column count, whatever language the content is in.
    construir(líneas, relleno) {
        w = ancho_de(líneas)
        borde = "─" $* (w + relleno * 2)
        filas = ["╭" $+ borde $+ "╮"]
        hueco = " " $* relleno
        @ l : líneas {
            filas = filas $+ ("│" $+ hueco $+ t::pad_right(l, w) $+ hueco $+ "│")
        }
        filas = filas $+ ("╰" $+ borde $+ "╯")
        <~ filas
    }
}
```

A menu is then **data**, not drawing code. Prepend the selection marker *before*
measuring, so the frame has room for it in every language and the rows never shift
when the cursor moves:

```zymbol
filas_de_menú(título, opciones, sel) {
    líneas = [título, ""]
    @ i : 1..(opciones$#) {
        marca = "  "
        ? i == sel { marca = "► " }
        líneas = líneas $+ (marca $+ "[" $+ "{i}" $+ "] " $+ opciones[i])
    }
    <~ m::construir(líneas, 2)
}
```

Output, with `opciones` taken from the active locale:

```
╭───────────────────────────╮ 29     ╭───────────────────────────╮ 29
│  Elige tu velocidad       │ 29     │  Choose your speed        │ 29
│                           │ 29     │                           │ 29
│    [1] Lento      160 ms  │ 29     │  ► [1] Slow       160 ms  │ 29
│    [2] Normal     130 ms  │ 29     │    [2] Normal     130 ms  │ 29
│  ► [3] Rápido     100 ms  │ 29     │    [3] Fast       100 ms  │ 29
│    [4] Infernal    70 ms  │ 29     │    [4] Infernal    70 ms  │ 29
│    [5] Demencial   40 ms  │ 29     │    [5] Insane      40 ms  │ 29
╰───────────────────────────╯ 29     ╰───────────────────────────╯ 29
```

And the same code with a CJK locale, where every glyph is two columns:

```
╭──────────────────────────╮ 28
│   へび                   │ 28
│   速さを選ぶ:            │ 28
│   [1] ゆっくり  160 ms   │ 28
│   [5] 狂気     40 ms     │ 28
╰──────────────────────────╯ 28
```

Nothing was retyped. The trailing number is `t::width` of the row: within a locale
every row is identical, which is the invariant to assert in tests.

### Two things that are easy to get wrong

**Centre inside the builder, not before it.** The frame width is not known until
every line has been measured, so a title centred against a guessed width goes
off-centre the moment another line in the same panel gets longer — which is
exactly what a translation does. Mark the line instead and let the builder centre
it once the width is known:

```zymbol
líneas = [
    marco.CENTRADO $+ idioma::texto("menú.título"),
    marco.SEPARADOR,
    idioma::texto("menú.velocidad")
]
```

**Inner columns need measuring too, not just the outer frame.** A menu row of the
shape `label … value` has a second alignment point inside the box. Hov veS padded
its labels to a constant 26 columns; `Hab SoSlI' (medium)` is wider than that, so
the delay ran into the label. Measure that column against the active locale as
well — and measure it with the selection marker applied, so the row does not shift
when the cursor moves:

```zymbol
ancho_etiqueta = 0
@ i : 1..(opciones$#) {
    w = t::width(fila_menú(i, opciones[i], #1))   // #1 = con marca
    ? w > ancho_etiqueta { ancho_etiqueta = w }
}
```

### Consequences for positioning

Anything positioned relative to text must be positioned relative to its **measured**
width. A score badge centred over a border with

```zymbol
col = AN / 2 - 7          // ✗ derived from the length of " ✦ PUNTOS "
```

is correct in exactly one language. Compose the badge as one translated string and
centre *that*:

```zymbol
insignia = 言::texto_puntaje(puntos)          // " ✦ PUNTOS 12 ✦ "
col = (AN - t::width(insignia)) / 2 + 1       // ✓ correct in every language
```

---

## 7. Switching language at runtime

Because the locale is module state and not a parameter, the application can change
it mid-session from its own settings screen:

```zymbol
言::設定("ko")
描::全画面(状態)        // full redraw — every string is re-fetched
```

> **Split the computation from the effect**, even though the language no longer
> forces you to:
>
> ```zymbol
> _siguiente() { … <~ lista[pos + 1] }   // pure: computes
> rotar() { fijar(_siguiente()) }        // void: writes
> ```
>
> The one-function version — write the state and return it — was silently wrong
> before v0.0.8: the tree-walker dropped the write and returned the new value
> anyway, while the register VM got it right (HLZ-SRP-001). It is fixed, but the
> failure was invisible, so both projects kept the split rather than depend on
> which binary a reader happens to have.
>
> Either way, **have the gate walk the full rotation** and assert it comes back to
> the first locale. That check is what caught it.

Two rules:

- **Redraw everything, not the changed rows.** The new language has different
  widths, so a delta redraw leaves fragments of the previous language on screen.
- **Re-measure, do not cache.** Any width computed before the switch is stale. If
  the layout is expensive to recompute, recompute it anyway; it happens once per
  switch, not once per frame.

Offering the language selector *inside* the application also means the user is not
required to know the launch incantation for their language — which matters most
precisely for the users who cannot read the current one. Two consequences:

- **The language names in the selector come from each locale, not from the active
  one.** `Español` is spelled `Español` whether the application is currently in
  Klingon or in English. A language menu written in a language the reader cannot
  read is the one menu that must not need translating.
- **Keyboard shortcuts stay fixed.** If `N` starts a new game, it keeps doing so in
  every language even where the translated label starts with another letter. A
  shortcut that moves when the language changes is worse than one that never
  matched the label in the first place. Number keys are best of all: `[1]` in the
  menu is a *key*, not a quantity, so it stays ASCII even in a locale that writes
  its digits some other way.

---

## 8. Entry points per language

Ship one entry file per language whose only job is to preselect the locale:

```zymbol
// go.zy — entry point, English
<# ./対局 => 対局
対局::開始("en")
```

```zymbol
// 囲碁.zy — entry point, Japanese
<# ./対局 => 対局
対局::開始("ja")
```

They differ in one string. The value is discoverability: `zymbol run go.zy` for
terminals where typing CJK is inconvenient, `zymbol run 바둑.zy` for a Korean
player who would not have guessed the Japanese filename. This decides only which
language the *first* screen is in — everything remains switchable per §7.

---

## 9. Identifier-level API layers

The text axis makes the application readable by users. The code axis makes the
engine usable by developers who do not read its base language. It is a pure
re-export layer, resolved at load time, with zero runtime cost:

```zymbol
# .api_english {
    <# ../核/盤 => b
    <# ../核/規則 => r
    <# ../核/計算 => s

    #> {
        b::新規   => new_board
        b::着手   => play
        b::連     => chain
        b::ダメ数 => liberties
        r::合法   => is_legal
        r::終局   => is_over
        s::目算   => score
    }
}
```

A front-end written against `api/english` never opens a file in the base language.
See [`I18N.md`](I18N.md) Part 1 for the full three-layer pattern, the re-export
syntax, and the naming conventions.

---

## 10. The completeness gate

Make the missing-translation fallback return the **key**, not an empty string:

```zymbol
語(鍵) {
    <~ ?? 鍵 {
        "終局.石" => "Stones"
        …
        _         => 鍵          // identity fallback
    }
}
```

A missing entry is then visible on screen instead of silent — and, because keys
carry a domain prefix and can never equal their own translation (§3), completeness
becomes a decidable property that a test can check:

```zymbol
<# ../言語/取次 => 言

鍵 = 言::鍵一覧()
欠落合計 = 0

@ コード : 言::言語一覧() {
    言::設定(コード)
    欠落 = 0
    @ 一鍵 : 鍵 {
        ? (言::語(一鍵)) == 一鍵 {
            >> "      MISSING  " 一鍵 ¶
            欠落 = 欠落 + 1
        }
    }
    欠落合計 = 欠落合計 + 欠落
}

? 欠落合計 == 0 { >> "PASS — every key resolves in every locale" ¶ }
_ { >> "FAIL — " 欠落合計 " missing translations" ¶ }
```

```
  [ja] 日本語        51/51  OK
  [ko] 한국어        51/51  OK
  [zh] 中文          51/51  OK
  [en] English       51/51  OK
  [es] Español       51/51  OK
PASS — every key resolves in every locale
```

**Exercise the composed messages too.** A static lookup can only be missing; a
composed sentence can be *wrong* — a broken number format, an unhandled grammatical
branch. Call every composing function in every locale with the values that select
each branch (singular, plural, zero, the half-point case, the draw case), and print
the results. It costs ten lines and it moves those failures off the end-of-game
screen and into the test.

Wire the gate into the project's test runner so it runs with everything else:

```bash
for suite in 試験/文字試験.zy 試験/言語検証.zy 試験/盤試験.zy … ; do
```

And run the gate under both engines. Module support is where the register VM is
most likely to diverge, and i18n code is module code almost by definition:

```bash
zymbol run       試験/言語検証.zy
zymbol run --vm  試験/言語検証.zy
```

---

## 11. Documentation and locale codes

- **Locale codes stay ASCII.** ISO 639-1 (`ja`, `ko`, `zh`, `en`, `es`) is an
  international convention; it is not a string the user reads and it is not
  translated. Order the list meaningfully — the reference project leads with the
  language the program is written in and then follows the game's historical path
  across Asia and west.
- **One README per language**, `README_XX.md`, cross-linked from the top of each.
  A multilingual application whose documentation is monolingual is only half
  translated.
- **State the base language and why**, once, near the top. Readers will otherwise
  assume the choice was arbitrary.
- **Record what it costs to add a language.** If the answer is "three edits — a
  locale file, an import, one arm in each dispatch", say so; it is the strongest
  possible evidence that the architecture is right, and it is also a promise the
  gate keeps you honest about.

---

## 12. Audit checklist

Run this against any Zymbol application that shows text to a user.

| # | Check | Failure looks like |
|---|-------|--------------------|
| 1 | Locale is module state, not a parameter | A locale argument in a render signature |
| 2 | Keys are in the base language | An ASCII key layer nobody speaks |
| 3 | Every key has a domain prefix | `"終局"` instead of `"終局.表題"` |
| 4 | Missing translation falls back to the key | Blank labels in production |
| 5 | Sentences with numbers are locale functions | `"Score: " $+ n $+ " points"` at the call site |
| 6 | Every width goes through `std/term::width` | `$#` used on a string, or a hand-counted constant |
| 7 | Frames are built from measured content | A row of a box typed as one literal with its padding |
| 8 | Positions derive from measured widths | `AN / 2 - 7` |
| 9 | Language is switchable inside the application, at any point the user returns to | A locale selector shown once per session, or fixed at launch |
| 10 | One entry point per language | Only the base language is discoverable |
| 11 | A key catalogue exists and the dispatcher publishes it | Nothing to test against |
| 12 | A gate walks catalogue × locales, in both engines | Missing translations found by users |
| 13 | The gate exercises composed messages | A plural bug that only appears on the end screen |
| 14 | Documentation exists per language | `README.md` only |

Items 1, 6 and 7 are the ones that make the difference between a fourth language
costing an afternoon and costing a rewrite.

---

## 13. Case studies

Three projects in this workspace, in the order they were written.

All three now follow this document. The columns show what each looked like when it
was written and what it looks like after the audit.

| | Serpiente | Hov veS | 囲碁 |
|---|---|---|---|
| Written for | v0.0.5 → **v0.0.8** | v0.0.5 → **v0.0.8** | v0.0.8 |
| Languages | 1 → **2** | 3 | 5 |
| Locale held as | — → **module state** | parameter → **module state** | module state |
| Key catalogue | — → **31 keys** | — → **27 keys** | 51 keys |
| Composed messages | — → **2 per locale** | — → **2 per locale** | 3 per locale |
| Widths | hand-counted → **`std/term`** | hand-counted → **`std/term`** | `std/term` |
| Frames | literals → **measured** | literals → **measured** | measured |
| Runtime switching | — → **`L`, any menu** | first screen → **`L`, any menu** | setup screen |
| Entry points | 1 → **2** | 1 (+ a locale selector as its first screen) | 4 |
| API layer | — | — | `api/english`, `api/espanol` |
| Gate | — → **31 × 2** | — → **27 × 3** | 51 × 5 + 50 × 3 |
| Docs per language | 2 of 2 | **2 of 3** | **3 of 5** |

Every gate runs in both engines and every one of them walks the full locale
rotation, which is what catches the trap in §7. Keys carry a domain prefix in all
three, and in each case they are written in the language the program itself is
written in: Spanish for Serpiente, Klingon in pIqaD for Hov veS, Japanese for 囲碁.

The bold rows are where the checklist bit hardest, and the last one is the item
still open in two of the three: **documentation lags the interface**. It is the
cheapest item to skip and the easiest to forget, because nothing fails when you
do. Both audits record it rather than pretend otherwise.

**Serpiente** — had no i18n at all: about forty Spanish strings embedded in
fixed-width boxes, and a score badge positioned by constants derived from the
length of the Spanish word for "points". Written before `std/term` existed. The
retrofit forced one structural change worth noting: the game body had to move out
of `serpiente.zy` into a `juego.zy` module, because an entry point that preselects
a locale cannot also *be* the game. Audit:
[`serpiente/AUDITORIA_I18N_ES.md`](https://github.com/zymbol-lang/zy-Serpiente).

**Hov veS** — three real languages including a constructed one, but built with the
first-generation mechanism: the locale was a parameter, and each visible string was
a `>>~` line duplicated once per language inside the render. 79 such lines; a
20-row menu took 48 lines of code; the Spanish `(difícil)` had been abbreviated to
`(difíc)` because the box had no room. After the rewrite the parameter appears
**zero** times across all five modules and `(difícil)` fits. It is also the project
that proved numbers can be a locale concern (§4). Audit:
[`klingon_galaxy/auditoria_i18n_es.md`](https://github.com/zymbol-lang/zyKlingonGalaxy).

**囲碁** — the reference. Five languages, every rule in this document, and adding a
sixth is three edits. Its own remaining gaps are recorded in
[`GO/AUDITORIA_I18N_ES.md`](https://github.com/zymbol-lang/zy-GO) — the reference
is not exempt from its own checklist.

---

## 14. Third mechanism: the digit script

`I18N.md` documents two mechanisms — re-export layers for code, dispatcher
modules for runtime text. There is a third, and it is the only one that needs no
catalogue at all: **choosing a language can choose the script the digits are
written in.**

The numeral mode-switch (`#०९#`, GUIDE.md §"Mode-Switch Token") is a runtime
directive that is **global to the process**, not to the file. So the dispatcher
that already knows the current language can set it in the same place:

```zymbol
set_language(code) {
    current_language = code
    ?? code {
        "fa" => { #۰۹# }      // Extended Arabic-Indic  U+06F0
        "hi" => { #०९# }      // Devanagari             U+0966
        "en" => { #09# }      // ASCII
        "es" => { #09# }
        _    => { #09# }
    }
    <~ 0
}
```

**Nothing downstream has to know.** No drawing code, no formatting helper and no
key in the catalogue changes. The same line that composes a square's name emits
`e४`, `e۴` or `e4`, and every number the application prints — scores, counters,
coordinates, menu indices — follows the language without a single call site
being touched. That is what separates this from the other two mechanisms: they
need an entry per string, this one needs an entry per *language*.

चतुरङ्गम् uses it across four scripts; the verification is in its
`परीक्षा/भाषापरीक्षा.zy` and `परीक्षा/चित्रपरीक्षा.zy` suites.

### Two traps

Both are documented in the guide as intended behaviour, and neither is obvious
when the mode is being driven by a language setting rather than written inline.

1. **The mode reaches `io::write` and `<\ … \>`.** A program that writes a data
   file while a non-ASCII mode is active will create `dato४२.txt`, and a shell
   command built with a number in it will carry that number in the active
   script. A game log, a save file, an export — anything that *names* a file or
   builds a command — must switch back to `#09#` first:

   ```zymbol
   save_game(n) {
       #09#                       // filenames are not user-facing text
       io::write("game" n ".log", body)
       set_language(current_language)   // restore the user's script
   }
   ```

2. **`json::encode` always emits ASCII**, so serialized data is safe as it
   stands. This is the asymmetry worth remembering: the *display* path is
   localized and the *data* path is not, which is the right way round, but it
   means you cannot test one by looking at the other.

The rule that falls out of both: **the mode belongs to output the user reads.**
Switch back before anything a machine will read back.

---

## See Also

- [`I18N.md`](I18N.md) — the two language mechanisms: re-export layers for code,
  dispatcher modules for runtime text
- [`GUIDE.md`](GUIDE.md) — modules, imports, re-exports, `std/term`
- [`REFERENCE.md`](REFERENCE.md) — complete symbol table and limitations
- [zy-GO](https://github.com/zymbol-lang/zy-GO) — the reference implementation;
  `DESIGN.md` §8–§9 covers its i18n internals
