# Match Expression Design — `??`

**Version target:** v0.0.4
**Status:** Design approved — pending implementation

---

## Problem statement

The current `??` implementation conflates two distinct ideas:

1. **Pattern matching** — structural comparison of a value against concrete patterns
2. **Conditional branching** — boolean guard expressions

The `_?` guard inside `??` was added as a workaround for missing comparison patterns.
It forces the user to repeat the scrutinee name in every arm, which turns `??` into
a dressed-up if-else chain and obscures its real purpose.

```
// CURRENT — wrong: scrutinee repeated, semantics muddled
estado = ?? temperatura {
    _? temperatura < 0  : "hielo"
    _? temperatura < 15 : "frío"
    _                   : "caluroso"
}

// DESIRED — clean: scrutinee stated once, arms are pure patterns
estado = ?? temperatura {
    < 0  : "hielo"
    < 15 : "frío"
    _    : "caluroso"
}
```

---

## Canonical forms

`??` has **three forms** depending on whether each arm returns a value, executes a
block, or both. The scrutinee and arm syntax are identical across all three.

### Form 1 — Assignment (arms return values)

```
result = ?? expr {
    pattern : value
    pattern : value
    _       : default_value
}
```

Use when every arm produces a single value to assign. No `{ }` blocks.

```
calificacion = ?? nota {
    90..100 : "Excelente"
    80..89  : "Muy bien"
    70..79  : "Bien"
    _       : "Reprobado"
}
```

### Form 2 — Execution (arms run blocks)

```
?? expr {
    pattern : { statements }
    pattern : { statements }
    _       : { statements }
}
```

Use when each arm needs to run multiple statements. No return value.

```
?? comando {
    "abrir"  : { >> "Abriendo..." ¶  open_file()  }
    "cerrar" : { >> "Cerrando..." ¶  close_file() }
    _        : { >> "Desconocido" ¶               }
}
```

### Form 3 — Combined (assign + execute)

```
result = ?? expr {
    pattern : value { statements }
    pattern : value { statements }
    _       : default_value
}
```

Use when each arm both produces a value AND runs side effects.

```
estado = ?? codigo {
    200 : "OK"      { log("success") }
    404 : "Not Found" { log("missing") }
    500 : "Error"   { log("critical")  alert() }
    _   : "Unknown"
}
```

---

## Pattern types

### Literal — exact value match

```
?? color {
    "rojo"  : resultado
    'A'     : resultado
    42      : resultado
    #1      : resultado
}
```

### Range — inclusive on both ends (`..`)

```
?? nota {
    90..100 : "Excelente"
    80..89  : "Muy bien"
    0..59   : "Reprobado"
}
```

### Comparison — implicit scrutinee (NEW — v0.0.5)

The scrutinee is implied. The operator is applied as `scrutinee OP value`.

Supported operators: `<`, `>`, `<=`, `>=`, `==`, `<>`

```
?? temperatura {
    < 0  : "Bajo cero"
    < 15 : "Frío"
    < 25 : "Agradable"
    < 35 : "Caluroso"
    _    : "Muy caluroso"
}
```

Arms are evaluated top to bottom. The first arm whose comparison is true wins.
This mirrors how `CASE` behaves in Zenith-Lang.

> **Important:** comparison patterns do NOT repeat the scrutinee. The operator is
> applied to the scrutinee implicitly. Writing `< 0` means `temperatura < 0` when
> the scrutinee is `temperatura`.

### Variable — pattern from a named value (NEW — v0.0.5)

Any identifier that holds a value at runtime can be used as a pattern.
The runtime evaluates the variable and applies the appropriate match rule
based on the combination of scrutinee type and variable type.

```
umbral_error = 500
estado_ok    = "activo"
rangos_vip   = [1, 2, 3]

?? codigo {
    umbral_error : { alert() }    // scalar == scalar  →  exact equality
    _            : { log()   }
}

?? estado {
    estado_ok : { >> "OK" ¶ }    // scalar == scalar  →  exact equality
    _         : { >> "KO" ¶ }
}

?? nivel {
    rangos_vip : "VIP"            // scalar ∈ array   →  containment check
    _          : "regular"
}
```

**Resolution rules at runtime:**

| Scrutinee type | Variable type | Semantic |
|---|---|---|
| scalar | scalar | exact equality: `scrutinee == variable` |
| scalar | array | containment: `scrutinee ∈ variable` |
| array | array | structural match: element-by-element equality |

> **Note:** the variable is evaluated once per match arm evaluation.
> Mutations inside a block arm do not affect subsequent arms in the same match.

---

### Array — multi-value OR / containment (NEW — v0.0.5)

When the scrutinee is a **scalar**, an array pattern performs a **containment
check**: the arm fires if the scrutinee equals any element in the array.
This is the multi-value OR, dynamic and inline.

#### Inline array (literal values)

```
?? codigo_http {
    [200, 201, 202] : "Éxito"
    [400, 422]      : "Bad Request"
    [401, 403]      : "Sin acceso"
    [500, 503]      : "Error"      { alert() }
    _               : "Desconocido"
}
```

#### Dynamic array (variable)

```
codigos_ok     = [200, 201, 202]
codigos_redir  = [301, 302, 307]

?? codigo {
    codigos_ok    : "Éxito"
    codigos_redir : "Redirección"
    _             : "Otro"
}
```

The array can be built at runtime before the match executes:

```
dias_laborales = ["lunes", "martes", "miércoles", "jueves", "viernes"]

?? hoy {
    dias_laborales : { >> "Día laboral" ¶ }
    _              : { >> "Fin de semana" ¶ }
}
```

#### When scrutinee IS an array — structural match (existing behavior)

When the **scrutinee itself is an array**, an array pattern performs structural
matching (element-by-element). `_` inside is a positional wildcard.

```
?? comando {
    ["abrir",  _] : { open_file()  }
    ["cerrar", _] : { close_file() }
    []            : { >> "vacío" ¶ }
    _             : { >> "desconocido" ¶ }
}
```

#### Disambiguation rule

| Scrutinee | Pattern | Semantic |
|---|---|---|
| scalar (int, string, char, bool) | `[v1, v2, ...]` | containment: `scrutinee ∈ [v1,v2,...]` |
| scalar | identifier (array variable) | containment: `scrutinee ∈ variable` |
| array | `[p1, p2, ...]` | structural: shape + element match |
| array | identifier (array variable) | structural: full equality |

The rule is determined at **runtime** by the type of the scrutinee — no special
syntax needed. The same `[...]` notation works for both semantics.

### Wildcard `_` — default / catch-all

`_` matches any value not caught by a prior pattern. It is **not** an `else` from
conditionals — it is the `OTHER CASE` / `default:` of pattern matching.

```
?? valor {
    1 : "uno"
    2 : "dos"
    _ : "otro"    // any value not matched above
}
```

---

## What `_` is NOT

`_` in `??` is a **pattern** — the wildcard. It has nothing to do with the `_`
block in conditionals (which is the else branch). They look identical but operate
in completely different contexts:

| Context | Symbol | Meaning |
|---|---|---|
| `? cond { } _ { }` | `_` | else — runs when condition is false |
| `?? val { _ : x }` | `_` | wildcard — matches when no other pattern did |

---

## Guard patterns `_?` — DEPRECATED in `??`

The `_?` guard inside `??` is removed from the language spec. It was a stopgap
for missing comparison patterns and introduced semantic confusion.

**Before (deprecated):**
```
// BAD: looks like if-else, repeats scrutinee
?? n {
    _? n < 0 : "negativo"
    _? n > 0 : "positivo"
    _        : "cero"
}
```

**After (correct):**
```
// GOOD: clean comparison patterns
?? n {
    < 0 : "negativo"
    > 0 : "positivo"
    _   : "cero"
}
```

If a use case genuinely requires complex boolean expressions (`&&`, `||`, function
calls), use `?`/`_?` conditionals — that is what they are for.

---

## Decision rules — `??` vs `?`

| Use case | Correct construct |
|---|---|
| Match exact values | `??` with literals |
| Match numeric ranges | `??` with `..` |
| Match ordered comparisons | `??` with `<`, `>`, `<=`, `>=` |
| Match list shape | `??` with array patterns |
| Complex boolean logic (`&&`, `\|\|`) | `?` / `_?` |
| Single condition | `?` |

---

## Implementation plan

### Files to modify

| File | Change |
|---|---|
| `zymbol-ast/src/match_stmt.rs` | Add `Pattern::Comparison(BinaryOp, Box<Expr>, Span)` and `Pattern::Ident(String, Span)` |
| `zymbol-parser/src/match_stmt.rs` | Parse `Lt`/`Gt`/`Le`/`Ge`/`Eq`/`Neq` as `Comparison`; parse bare identifiers as `Ident` |
| `zymbol-interpreter/src/match_stmt.rs` | Evaluate `Comparison` and `Ident`; dispatch `List` by scrutinee type (structural vs containment) |
| `zymbol-compiler/src/lib.rs` | VM: compile `Comparison`, `Ident`, and updated `List` semantics |
| `zymbol-formatter/src/visitor.rs` | Format `Comparison` as `op value`; `Ident` as identifier name |
| `interpreter/MANUAL.md` | Update `??` section — remove `_?` guard, document all new patterns |
| `aprende_zymbol/08_match.md` | Rewrite lesson with correct semantics |

### AST change

```rust
pub enum Pattern {
    Literal(Literal, Span),
    Range(Box<Expr>, Box<Expr>, Span),
    List(Vec<Pattern>, Span),               // structural OR containment — resolved at runtime
    Wildcard(Span),
    Comparison(BinaryOp, Box<Expr>, Span),  // NEW: < value, > value, etc.
    Ident(String, Span),                    // NEW: variable reference — exact eq or containment
    // Guard(Box<Pattern>, Box<Expr>, Span), // DEPRECATED — removed
}
```

### Parser change — `parse_pattern()`

```rust
TokenKind::Lt | TokenKind::Gt | TokenKind::Le |
TokenKind::Ge | TokenKind::Eq | TokenKind::Neq => {
    let op = match &token.kind {
        TokenKind::Lt  => BinaryOp::Lt,
        TokenKind::Gt  => BinaryOp::Gt,
        TokenKind::Le  => BinaryOp::Le,
        TokenKind::Ge  => BinaryOp::Ge,
        TokenKind::Eq  => BinaryOp::Eq,
        TokenKind::Neq => BinaryOp::Neq,
        _ => unreachable!(),
    };
    self.advance(); // consume operator
    let rhs = Box::new(self.parse_expr()?);
    let span = token.span.to(&rhs.span());
    Pattern::Comparison(op, rhs, span)
}
```

### Interpreter change — `pattern_matches()`

```rust
Pattern::Comparison(op, rhs_expr, span) => {
    let rhs = self.eval_expr(rhs_expr)?;
    let result = match op {
        BinaryOp::Lt  => self.value_lt(value, &rhs, *span)?,
        BinaryOp::Gt  => self.value_lt(&rhs, value, *span)?,
        BinaryOp::Le  => self.value_le(value, &rhs, *span)?,
        BinaryOp::Ge  => self.value_le(&rhs, value, *span)?,
        BinaryOp::Eq  => self.values_equal(value, &rhs),
        BinaryOp::Neq => !self.values_equal(value, &rhs),
        _ => return Err(RuntimeError::Generic {
            message: format!("invalid comparison operator in pattern"),
            span: *span,
        }),
    };
    if result { Ok(Some(true)) } else { Ok(None) }
}
```

### Formatter change — `format_pattern()`

```rust
Pattern::Comparison(op, rhs, _) => {
    let op_str = match op {
        BinaryOp::Lt  => "<",
        BinaryOp::Gt  => ">",
        BinaryOp::Le  => "<=",
        BinaryOp::Ge  => ">=",
        BinaryOp::Eq  => "==",
        BinaryOp::Neq => "<>",
        _ => unreachable!(),
    };
    self.output.write(op_str);
    self.output.write(" ");
    self.format_expr(rhs);
}
Pattern::Ident(name, _) => {
    self.output.write(name);
}
```

### Parser change — `Ident` pattern

```rust
TokenKind::Identifier(name) => {
    let name = name.clone();
    self.advance(); // consume identifier
    Pattern::Ident(name, token.span)
}
```

### Interpreter change — `Ident` pattern + updated `List` semantics

```rust
Pattern::Ident(name, span) => {
    let var_val = self.scope.get(name).ok_or_else(|| RuntimeError::Generic {
        message: format!("undefined variable '{name}' in pattern"),
        span: *span,
    })?;
    match &var_val {
        // scalar variable → exact equality
        Value::Int(_) | Value::Float(_) | Value::String(_)
        | Value::Char(_) | Value::Bool(_) => {
            if self.values_equal(value, &var_val) { Ok(Some(true)) } else { Ok(None) }
        }
        // array variable: dispatch on scrutinee type
        Value::Array(arr) => {
            match value {
                // scalar scrutinee + array variable → containment
                Value::Int(_) | Value::Float(_) | Value::String(_)
                | Value::Char(_) | Value::Bool(_) => {
                    let found = arr.iter().any(|el| self.values_equal(value, el));
                    if found { Ok(Some(true)) } else { Ok(None) }
                }
                // array scrutinee + array variable → structural equality
                Value::Array(_) => {
                    if self.values_equal(value, &var_val) { Ok(Some(true)) } else { Ok(None) }
                }
                _ => Ok(None),
            }
        }
        _ => Ok(None),
    }
}

// Updated Pattern::List — dispatch by scrutinee type
Pattern::List(patterns, _span) => {
    match value {
        // array scrutinee → structural match (existing behavior)
        Value::Array(arr) => {
            if patterns.len() != arr.len() { return Ok(None); }
            for (pat, val) in patterns.iter().zip(arr.iter()) {
                if self.pattern_matches(pat, val)?.unwrap_or(false) == false {
                    return Ok(None);
                }
            }
            Ok(Some(true))
        }
        // scalar scrutinee → containment: scrutinee ∈ [literals]
        Value::Int(_) | Value::Float(_) | Value::String(_)
        | Value::Char(_) | Value::Bool(_) => {
            for pat in patterns {
                if let Pattern::Literal(lit, _) = pat {
                    let pat_val = literal_to_value(lit);
                    if self.values_equal(value, &pat_val) {
                        return Ok(Some(true));
                    }
                }
            }
            Ok(None)
        }
        _ => Ok(None),
    }
}
```

---

## Test cases

```
// ── Comparison patterns ──────────────────────────────────
temperatura = -5
estado = ?? temperatura {
    < 0  : "Bajo cero"
    < 15 : "Frío"
    < 25 : "Agradable"
    < 35 : "Caluroso"
    _    : "Muy caluroso"
}
>> estado ¶    // → Bajo cero

// Mixed: ranges + comparisons
n = 150
categoria = ?? n {
    < 0    : "negativo"
    0..9   : "un dígito"
    10..99 : "dos dígitos"
    >= 100 : "tres o más"
}
>> categoria ¶    // → tres o más

// ── Ident pattern — scalar variable (exact equality) ─────
umbral = 42
x = 42
resultado = ?? x {
    umbral : "exacto"
    _      : "otro"
}
>> resultado ¶    // → exacto

// ── Ident pattern — array variable (containment) ─────────
dias_lab = ["lunes", "martes", "miércoles", "jueves", "viernes"]
hoy = "martes"
tipo = ?? hoy {
    dias_lab : "laboral"
    _        : "fin de semana"
}
>> tipo ¶    // → laboral

// ── Array literal (containment) — scalar scrutinee ───────
codigo = 404
msg = ?? codigo {
    [200, 201, 202] : "Éxito"
    [400, 422]      : "Bad Request"
    [401, 403]      : "Sin acceso"
    [404]           : "No encontrado"
    [500, 503]      : "Error"
    _               : "Desconocido"
}
>> msg ¶    // → No encontrado

// ── Array literal (structural) — array scrutinee ─────────
cmd = ["abrir", "doc.zy"]
?? cmd {
    ["abrir",  _] : { >> "Abriendo..." ¶ }
    ["cerrar", _] : { >> "Cerrando..." ¶  }
    []            : { >> "vacío" ¶        }
    _             : { >> "desconocido" ¶  }
}
// → Abriendo...

// ── Combined: variable + block ────────────────────────────
codigos_ok   = [200, 201, 202]
codigos_err  = [500, 502, 503]
codigo2      = 201

?? codigo2 {
    codigos_ok  : { >> "Éxito — código " codigo2 ¶ }
    codigos_err : { >> "Error — código " codigo2 ¶  alert() }
    _           : { >> "Código desconocido" ¶ }
}
// → Éxito — código 201
```
