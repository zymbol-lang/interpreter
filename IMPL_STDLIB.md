# Implementation Plan — Zymbol Standard Library Infrastructure + std/math

> **Status: IMPLEMENTED in v0.0.7.** Steps 0–4 shipped: `enum FunctionDef` is in
> `crates/zymbol-interpreter/src/lib.rs`, and `NativeFn` and the `native_fns` field on
> `LoadedModule` are gone from every crate — the migration described under "Relation to
> IMPL_V007.md" was carried out, not deferred. Kept as the architecture reference for
> adding a stdlib module: it is the document that explains *why* the dispatch looks the
> way it does, which `IMPL_V007.md` (the per-module checklist) assumes rather than states.
>
> The stdlib has grown past what this plan names. Today: `math`, `random`, `json`, `io`,
> `net`, `db`, `term`, `time` — `std/env` was dropped by design, and `std/term` (v0.0.8),
> `std/db` (v0.0.7, see `DESIGN_STD_DB.md`) and `std/time` (v0.0.9) arrived after.

> **Architecture:** `enum FunctionDef` — native functions as a variant of the existing
> function definition type. Zero new syntax. Full i18n re-export support.
>
> **Primary driver:** Zofia scientific computing project.
> `std/math` unblocks Zofia Phases 3–5 (activations, attention, positional encoding).
> `std/random` unblocks Zofia Phase 5 weight initialization.
>
> **Relationship with IMPL_V007.md:** this document established the unified
> `enum FunctionDef` architecture (one lookup path for all functions regardless
> of origin). IMPL_V007.md — since rewritten to match this real architecture —
> documents the v0.0.7 modules built on top of it: `std/json`, `std/io`,
> `std/net` (and `std/db`, see `DESIGN_STD_DB.md`), each an additional
> `register()` file with no further architecture changes. `std/env` was
> dropped as redundant (see IMPL_V007.md §Symbol vs module).

---

## Design principles

1. **Transparent to the caller.** `mat::raiz(x)` is indistinguishable from a
   Zymbol-defined function. Same call syntax, same error format, same re-export rules.

2. **Unified dispatch.** `module.functions.get(name)` returns either a Zymbol AST body
   or a native function pointer — the call site does not know or care which one.

3. **Zero new syntax.** All changes are inside `zymbol-interpreter`. The lexer, parser,
   AST, semantic analyzer, formatter, and LSP are untouched.

4. **Function pointers, not closures.** Native functions are `fn(Vec<Value>, Span) ->
   Result<Value>` — plain function pointers. No heap allocation per function definition.
   For `std/math` (pure f64 wrappers) closures are not needed.

5. **Type promotion by adapter.** Each native function handles the
   `Int → promoted to Float` case explicitly. No implicit coercion in the engine.

6. **VM parity deferred.** Native stdlib works in tree-walker mode first.
   VM parity (`Instruction::NativeCall`) is tracked in the final section.

---

## Feature map

| Step | Module | Functions | Cargo dep | Zofia phase |
|------|--------|-----------|-----------|-------------|
| 0–4 | infrastructure | `enum FunctionDef`, detection, dispatcher | none | all |
| 5 | `std/math` | `sqrt exp ln log pow sin cos abs max min floor ceil round` + `PI E` | none (std) | 3–5 |
| 6 | `std/random` | `entero rango peso_f64` | none (std) | 5 |
| — | `std/env`  | `get set args home` | none (std) | — |
| — | `std/io`   | `leer escribir agregar existe borrar listar` | none (std) | — |
| — | `std/json` | `parsear codificar` | `serde_json` | — |
| — | `std/net`  | `get post post_json head` | `ureq` | — |

Steps 0–4 are the shared infrastructure. Implement them before any module.
`std/math` and `std/random` are v0.0.6 targets. The remaining four modules follow
the same pattern and are documented in IMPL_V007.md — update them to use
`FunctionDef::Native` after Step 4 is in place.

---

## Step 0 — Cargo: no new dependencies for math/random

`std/math` and `std/random` use only Rust's standard library (`f64`, `u64`, `std::time`).
No `Cargo.toml` changes are required for these two modules.

For the remaining four modules (`std/env`, `std/io`, `std/json`, `std/net`), follow
IMPL_V007 Step 0 — add `serde_json` and `ureq` to the workspace manifest when those
modules are implemented.

---

## Step 1 — `enum FunctionDef` in `lib.rs`

**File:** `crates/zymbol-interpreter/src/lib.rs`

### 1a. Replace the struct with an enum

Locate the private struct (line 84):

```rust
struct FunctionDef {
    parameters: Vec<zymbol_ast::Parameter>,
    body: zymbol_ast::Block,
    origin_module_path: Option<PathBuf>,
}
```

Replace it with:

```rust
enum FunctionDef {
    Zymbol {
        parameters: Vec<zymbol_ast::Parameter>,
        body: zymbol_ast::Block,
        origin_module_path: Option<PathBuf>,
    },
    Native {
        name:  &'static str,
        arity: i8,    // number of expected arguments; -1 = variadic
        func:  fn(Vec<Value>, zymbol_span::Span) -> Result<Value>,
    },
}
```

`fn(...)` is a plain function pointer: `Copy`, zero-size, no heap allocation.
`name` is a `&'static str` for error messages; it does not need to match the
exported symbol name (which lives in the HashMap key).

### 1b. Update the one construction site in `lib.rs`

Locate the `Statement::FunctionDecl` arm (around line 972):

```rust
let func_def = FunctionDef {
    parameters: func_decl.parameters.clone(),
    body: func_decl.body.clone(),
    origin_module_path: self.current_file.clone(),
};
```

Add the variant tag:

```rust
let func_def = FunctionDef::Zymbol {
    parameters: func_decl.parameters.clone(),
    body: func_decl.body.clone(),
    origin_module_path: self.current_file.clone(),
};
```

---

## Step 2 — Update `functions_lambda.rs` for the new enum

**File:** `crates/zymbol-interpreter/src/functions_lambda.rs`

Four sites need changes.

### 2a. `eval_call` — module call path (line ≈184)

This is the critical dispatch point. Before the existing `module.functions.get()` call,
add the native check. The two lookups share the same HashMap, so no ordering issue arises —
both Zymbol and Native variants are stored together.

Locate the `Expr::MemberAccess` arm in `eval_call`. The existing code after obtaining
`module` is:

```rust
let func_def = module.functions.get(func_name).cloned().ok_or_else(|| {
    RuntimeError::FunctionNotExported {
        module: module_alias.clone(),
        function: func_name.clone(),
    }
})?;

return self.eval_traditional_function_call(
    func_def, &call.arguments, &call.span,
    Some((module_alias.clone(), module_path.clone())), None
);
```

`eval_traditional_function_call` will be updated in step 2b to handle both variants,
so this call site needs no change beyond recompiling cleanly.

### 2b. `eval_traditional_function_call` — dispatch on variant

Locate `eval_traditional_function_call` (around line 241). Its first action is to check
argument count against `func_def.parameters.len()`. With the enum, add a dispatch at the
very top before the arity check:

```rust
pub(crate) fn eval_traditional_function_call(
    &mut self,
    func_def: Rc<FunctionDef>,
    arguments: &[zymbol_ast::Expr],
    span: &Span,
    module_info: Option<(String, std::path::PathBuf)>,
    func_name: Option<&str>,
) -> Result<Value> {
    // Native functions: evaluate arguments, call the function pointer, return
    if let FunctionDef::Native { arity, func, .. } = func_def.as_ref() {
        let expected = *arity;
        let got = arguments.len() as i8;
        if expected >= 0 && got != expected {
            return Err(RuntimeError::Generic {
                message: format!(
                    "function expects {} argument(s), got {}",
                    expected, got
                ),
                span: *span,
            });
        }
        let mut arg_values = Vec::with_capacity(arguments.len());
        for arg in arguments {
            arg_values.push(self.eval_expr(arg)?);
        }
        return func(arg_values, *span);
    }

    // Zymbol functions: existing code unchanged below this point
    let FunctionDef::Zymbol { parameters, body, origin_module_path } =
        func_def.as_ref() else { unreachable!() };

    if arguments.len() != parameters.len() {
        // ... existing arity error (update to use `parameters` local) ...
    }
    // ... rest of existing code, replacing all `func_def.parameters` with `parameters`
    //     and `func_def.body` with `body`, `func_def.origin_module_path` with `origin_module_path` ...
```

The compiler will flag every `func_def.field` access inside the Zymbol branch — fix each
one to use the destructured local binding.

### 2c. `func_def_to_value` — guard against native (line ≈227)

`func_def_to_value` converts a named function into a first-class `FunctionValue` for HOF
use. Native functions cannot become first-class values (they have no Zymbol body to wrap).
Add an early guard:

```rust
pub(crate) fn func_def_to_value(&self, func_def: &Rc<FunctionDef>) -> FunctionValue {
    let FunctionDef::Zymbol { parameters, body, .. } = func_def.as_ref() else {
        // Native functions as first-class values: not supported in v0.0.6.
        // Return a zero-parameter unit function as a safe placeholder.
        return FunctionValue {
            params: vec![],
            body: zymbol_ast::LambdaBody::Expr(Box::new(zymbol_ast::Expr::Literal(
                zymbol_ast::LiteralExpr { value: zymbol_common::Literal::Unit,
                                          span: zymbol_span::Span::default() }
            ))),
            captures: Rc::new(std::collections::HashMap::new()),
            is_named_fn: false,
        };
    };

    let mut refs = std::collections::HashSet::new();
    let mut locals: std::collections::HashSet<String> =
        parameters.iter().map(|p| p.name.clone()).collect();
    collect_refs_in_stmts(&body.statements, &mut locals, &mut refs);
    let captures = self.capture_only(&refs);
    FunctionValue {
        params: parameters.iter().map(|p| p.name.clone()).collect(),
        body: zymbol_ast::LambdaBody::Block(body.clone()),
        captures: Rc::new(captures),
        is_named_fn: true,
    }
}
```

### 2d. Standalone function call path (line ≈153)

The `Expr::Identifier` arm looks up `self.functions`. Top-level native functions are not
stored there (only module functions are native in v0.0.6), so this path is unaffected.
No changes needed.

---

## Step 3 — stdlib import detection in `modules.rs`

**File:** `crates/zymbol-interpreter/src/modules.rs`

### 3a. Detection condition

A stdlib import is a bare path (not `./relative` and not `/absolute`) whose first component
is `"std"`. Add the intercept at the top of `load_import`, before the circular-import
guard:

```rust
pub(crate) fn load_import(&mut self, import: &zymbol_ast::ImportStmt) -> Result<()> {
    // Intercept stdlib: bare path with first component "std"
    if !import.path.is_relative
        && !import.path.is_absolute
        && import.path.components.first().map(|s| s == "std").unwrap_or(false)
    {
        let module_key = import.path.components.join("/");   // "std/math"
        return self.load_stdlib_module(&module_key, &import.alias);
    }

    // --- existing code unchanged below this line ---
    let module_path = self.resolve_module_path(&import.path)?;
    // ...
}
```

### 3b. `load_stdlib_module` method

Add as a new method on `impl<W: Write> Interpreter<W>`:

```rust
fn load_stdlib_module(&mut self, module_key: &str, alias: &str) -> Result<()> {
    // Synthetic key: never collides with real filesystem paths
    let synthetic = PathBuf::from(format!("__stdlib__/{}", module_key));

    if !self.loaded_modules.contains_key(&synthetic) {
        let module = crate::stdlib::build_module(module_key)
            .ok_or_else(|| RuntimeError::ModuleNotFound {
                path: module_key.to_string(),
            })?;
        self.loaded_modules.insert(synthetic.clone(), module);
    }

    self.import_aliases.insert(alias.to_string(), synthetic);
    Ok(())
}
```

### 3c. `build_module` helper

`crate::stdlib::build_module` returns a `LoadedModule` whose `functions` map contains
`FunctionDef::Native` entries. `LoadedModule` needs no new fields — the existing
`functions: HashMap<String, Rc<FunctionDef>>` holds both Zymbol and Native variants.

The re-export mechanism (`ExportItem::ReExport` arm in `load_module`) already clones
from `imported_module.functions` — native entries are cloned exactly like Zymbol ones.
**No changes to the re-export path are needed.**

---

## Step 4 — Stdlib registry: `stdlib/mod.rs`

**File:** `crates/zymbol-interpreter/src/stdlib/mod.rs` *(new file)*

```rust
use crate::modules::LoadedModule;
use std::collections::HashMap;
use std::path::PathBuf;

mod math;
mod random;
// future: mod env; mod io; mod json; mod net;

/// Build a LoadedModule for the requested stdlib path.
/// Returns None if the module is not recognized.
pub(crate) fn build_module(name: &str) -> Option<LoadedModule> {
    let fns = match name {
        "std/math"   => math::register(),
        "std/random" => random::register(),
        _            => return None,
    };

    Some(LoadedModule {
        name: name.to_string(),
        functions: fns,
        all_functions: HashMap::new(),
        constants: HashMap::new(),
        all_variables: HashMap::new(),
        import_aliases: HashMap::new(),
        loaded_modules_refs: HashMap::new(),
    })
}
```

**File:** `crates/zymbol-interpreter/src/lib.rs`

Add module declaration alongside the existing `mod stdlib;` comment location (or near
other `mod` declarations):

```rust
mod stdlib;
```

---

## Step 5 — `std/math`

**File:** `crates/zymbol-interpreter/src/stdlib/math.rs` *(new file)*

### 5a. Error helper

```rust
use crate::{Result, RuntimeError, Value};
use crate::FunctionDef;
use std::rc::Rc;
use std::collections::HashMap;
use zymbol_span::Span;

fn type_err(fname: &str, expected: &str, got: &[Value], span: Span) -> RuntimeError {
    let types: Vec<&str> = got.iter().map(|v| v.type_name()).collect();
    RuntimeError::Generic {
        message: format!(
            "mat::{}: expected {} argument(s) of type {}, got {:?}",
            fname, expected, expected, types
        ),
        span,
    }
}
```

### 5b. `Value::type_name()` helper

Add to `impl Value` in `lib.rs` if not already present:

```rust
pub fn type_name(&self) -> &'static str {
    match self {
        Value::Int(_)        => "###",
        Value::Float(_)      => "##.",
        Value::String(_)     => "##\"",
        Value::Char(_)       => "##'",
        Value::Bool(_)       => "##?",
        Value::Array(_)      => "##[]",
        Value::Tuple(_)      => "##()",
        Value::NamedTuple(_) => "##(name:)",
        Value::Function(_)   => "##fn",
        Value::Error(_)      => "##!",
        Value::Unit          => "##_",
    }
}
```

### 5c. Numeric coercion macro

Define once at the top of `math.rs` to reduce boilerplate in every function:

```rust
/// Extract f64 from Float or Int (with promotion). Returns None on wrong type.
#[inline]
fn as_f64(v: &Value) -> Option<f64> {
    match v {
        Value::Float(x) => Some(*x),
        Value::Int(x)   => Some(*x as f64),
        _               => None,
    }
}

/// Extract i64 from Int only.
#[inline]
fn as_i64(v: &Value) -> Option<i64> {
    match v {
        Value::Int(x) => Some(*x),
        _             => None,
    }
}
```

### 5d. Individual native functions

```rust
// --- Unary Float → Float --------------------------------------------------

fn math_raiz(args: Vec<Value>, span: Span) -> Result<Value> {
    match args.first().and_then(as_f64) {
        Some(x) => Ok(Value::Float(x.sqrt())),
        None    => Err(type_err("raiz", "###/##.", &args, span)),
    }
}

fn math_exp(args: Vec<Value>, span: Span) -> Result<Value> {
    match args.first().and_then(as_f64) {
        Some(x) => Ok(Value::Float(x.exp())),
        None    => Err(type_err("exp", "###/##.", &args, span)),
    }
}

fn math_ln(args: Vec<Value>, span: Span) -> Result<Value> {
    match args.first().and_then(as_f64) {
        Some(x) if x > 0.0 => Ok(Value::Float(x.ln())),
        Some(_) => Err(RuntimeError::Generic {
            message: "mat::ln: argument must be positive".into(), span,
        }),
        None => Err(type_err("ln", "###/##.", &args, span)),
    }
}

fn math_sen(args: Vec<Value>, span: Span) -> Result<Value> {
    match args.first().and_then(as_f64) {
        Some(x) => Ok(Value::Float(x.sin())),
        None    => Err(type_err("sen", "###/##.", &args, span)),
    }
}

fn math_cos(args: Vec<Value>, span: Span) -> Result<Value> {
    match args.first().and_then(as_f64) {
        Some(x) => Ok(Value::Float(x.cos())),
        None    => Err(type_err("cos", "###/##.", &args, span)),
    }
}

fn math_piso(args: Vec<Value>, span: Span) -> Result<Value> {
    match args.first().and_then(as_f64) {
        Some(x) => Ok(Value::Float(x.floor())),
        None    => Err(type_err("piso", "###/##.", &args, span)),
    }
}

fn math_techo(args: Vec<Value>, span: Span) -> Result<Value> {
    match args.first().and_then(as_f64) {
        Some(x) => Ok(Value::Float(x.ceil())),
        None    => Err(type_err("techo", "###/##.", &args, span)),
    }
}

fn math_redondear(args: Vec<Value>, span: Span) -> Result<Value> {
    match args.first().and_then(as_f64) {
        Some(x) => Ok(Value::Float(x.round())),
        None    => Err(type_err("redondear", "###/##.", &args, span)),
    }
}

// --- abs: polymorphic (Int → Int, Float → Float) --------------------------

fn math_abs(args: Vec<Value>, span: Span) -> Result<Value> {
    match args.first() {
        Some(Value::Int(x))   => Ok(Value::Int(x.abs())),
        Some(Value::Float(x)) => Ok(Value::Float(x.abs())),
        _ => Err(type_err("abs", "###/##.", &args, span)),
    }
}

// --- Binary Float × Float → Float -----------------------------------------

fn math_log(args: Vec<Value>, span: Span) -> Result<Value> {
    // log(x, base): natural log if one arg, log_base if two args
    match (args.first().and_then(as_f64), args.get(1).and_then(as_f64)) {
        (Some(x), Some(base)) if x > 0.0 && base > 0.0 && base != 1.0 => {
            Ok(Value::Float(x.log(base)))
        }
        (Some(x), None) if x > 0.0 => Ok(Value::Float(x.ln())),
        (Some(_), Some(_)) => Err(RuntimeError::Generic {
            message: "mat::log: x and base must be positive; base ≠ 1".into(), span,
        }),
        _ => Err(type_err("log", "###/##. [, ###/##.]", &args, span)),
    }
}

fn math_pot(args: Vec<Value>, span: Span) -> Result<Value> {
    // pot(base, exp): Float result. Complements the ^ operator for fractional exponents.
    match (args.first().and_then(as_f64), args.get(1).and_then(as_f64)) {
        (Some(base), Some(exp)) => Ok(Value::Float(base.powf(exp))),
        _ => Err(type_err("pot", "(###/##., ###/##.)", &args, span)),
    }
}

fn math_max(args: Vec<Value>, span: Span) -> Result<Value> {
    match (args.first(), args.get(1)) {
        (Some(Value::Int(a)),   Some(Value::Int(b)))   => Ok(Value::Int(*a.max(b))),
        (Some(a), Some(b)) => match (as_f64(a), as_f64(b)) {
            (Some(fa), Some(fb)) => Ok(Value::Float(fa.max(fb))),
            _ => Err(type_err("max", "(###/##., ###/##.)", &args, span)),
        },
        _ => Err(type_err("max", "(###/##., ###/##.)", &args, span)),
    }
}

fn math_min(args: Vec<Value>, span: Span) -> Result<Value> {
    match (args.first(), args.get(1)) {
        (Some(Value::Int(a)),   Some(Value::Int(b)))   => Ok(Value::Int(*a.min(b))),
        (Some(a), Some(b)) => match (as_f64(a), as_f64(b)) {
            (Some(fa), Some(fb)) => Ok(Value::Float(fa.min(fb))),
            _ => Err(type_err("min", "(###/##., ###/##.)", &args, span)),
        },
        _ => Err(type_err("min", "(###/##., ###/##.)", &args, span)),
    }
}
```

### 5e. `register()` — build the module function map + constants

```rust
pub(crate) fn register() -> HashMap<String, Rc<FunctionDef>> {
    let mut m: HashMap<String, Rc<FunctionDef>> = HashMap::new();

    macro_rules! native {
        ($name:literal, $arity:expr, $fn:expr) => {
            m.insert($name.into(), Rc::new(FunctionDef::Native {
                name: $name, arity: $arity, func: $fn,
            }));
        };
    }

    native!("sqrt",  1,  math_sqrt);
    native!("exp",   1,  math_exp);
    native!("ln",    1,  math_ln);
    native!("log",  -1,  math_log);     // 1 or 2 args — variadic
    native!("pow",   2,  math_pow);
    native!("sin",   1,  math_sin);
    native!("cos",   1,  math_cos);
    native!("abs",   1,  math_abs);
    native!("max",   2,  math_max);
    native!("min",   2,  math_min);
    native!("floor", 1,  math_floor);
    native!("ceil",  1,  math_ceil);
    native!("round", 1,  math_round);

    m
}
```

### 5f. Constants PI and E

Constants are stored in `LoadedModule.constants`, not in `functions`. Update
`build_module` in `stdlib/mod.rs` to inject them after building the module:

```rust
"std/math" => {
    let mut module = LoadedModule {
        name: "std/math".to_string(),
        functions: math::register(),
        // ... other fields: HashMap::new()
    };
    module.constants.insert("PI".into(), Value::Float(std::f64::consts::PI));
    module.constants.insert("E".into(),  Value::Float(std::f64::consts::E));
    Some(module)
}
```

Access pattern in Zymbol (constants via `alias.CONST`, not `alias::CONST`):

```zymbol
<# std/math => mat

radio = 3.5
area  = mat.PI * radio * radio
>> area ¶
```

> Note: `alias.CONST` access has a known open bug (`module constant access` in
> ROADMAP.md Known Gaps). Until that is fixed, the workaround is to export constants
> through a getter function or to define them locally in a Zymbol module that wraps
> `std/math`. Both patterns are shown in the Zofia integration section below.

---

## Step 6 — `std/random`

**File:** `crates/zymbol-interpreter/src/stdlib/random.rs` *(new file)*

Purpose: replace Zofia's `matematica.zy` LCG workaround with a stdlib RNG based on
xoshiro256++ (fast, statistically sound, no `unsafe`, no external crate).

### 6a. Architecture

The RNG state lives inside the native function via a `thread_local` cell. This keeps the
API stateless from Zymbol's perspective — no seed object to pass around.

```rust
use std::cell::Cell;

thread_local! {
    static STATE: Cell<[u64; 4]> = Cell::new([0u64; 4]);
    static SEEDED: Cell<bool> = Cell::new(false);
}
```

### 6b. xoshiro256++ core

```rust
fn xoshiro_next() -> u64 {
    STATE.with(|s| {
        let mut st = s.get();

        // Auto-seed from system time on first call
        if !SEEDED.with(|b| b.get()) {
            let ns = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.subsec_nanos())
                .unwrap_or(12345) as u64;
            st = [ns ^ 0xdeadbeef, ns.wrapping_mul(6364136223846793005),
                  ns ^ 0xc0ffee, ns.wrapping_add(1442695040888963407)];
            SEEDED.with(|b| b.set(true));
        }

        // xoshiro256++ step
        let result = st[0].wrapping_add(st[3]).rotate_left(23).wrapping_add(st[0]);
        let t = st[1] << 17;
        st[2] ^= st[0]; st[3] ^= st[1]; st[1] ^= st[2]; st[0] ^= st[3];
        st[2] ^= t;
        st[3] = st[3].rotate_left(45);
        s.set(st);
        result
    })
}
```

### 6c. Native functions

```rust
fn random_entero(args: Vec<Value>, span: Span) -> Result<Value> {
    // random::entero(min, max) -> Int in [min, max]
    match (args.first(), args.get(1)) {
        (Some(Value::Int(lo)), Some(Value::Int(hi))) if hi >= lo => {
            let range = (hi - lo + 1) as u64;
            let val = *lo + (xoshiro_next() % range) as i64;
            Ok(Value::Int(val))
        }
        _ => Err(RuntimeError::Generic {
            message: "random::entero: expected (###, ###) with max >= min".into(), span,
        }),
    }
}

fn random_rango(args: Vec<Value>, span: Span) -> Result<Value> {
    // random::rango(n) -> Int in [0, n-1]
    match args.first() {
        Some(Value::Int(n)) if *n > 0 => {
            Ok(Value::Int((xoshiro_next() % (*n as u64)) as i64))
        }
        _ => Err(RuntimeError::Generic {
            message: "random::rango: expected positive ###".into(), span,
        }),
    }
}

fn random_peso_f64(args: Vec<Value>, span: Span) -> Result<Value> {
    // random::peso_f64() -> Float in [-0.1, 0.1] for weight initialization
    let _args = args; let _span = span;
    let raw = xoshiro_next();
    let val = ((raw % 201) as f64 - 100.0) / 1000.0;
    Ok(Value::Float(val))
}

pub(crate) fn register() -> HashMap<String, Rc<FunctionDef>> {
    let mut m = HashMap::new();
    macro_rules! native {
        ($name:literal, $arity:expr, $fn:expr) => {
            m.insert($name.into(), Rc::new(FunctionDef::Native {
                name: $name, arity: $arity, func: $fn,
            }));
        };
    }
    native!("entero",    2, random_entero);
    native!("rango",     1, random_rango);
    native!("peso_f64",  0, random_peso_f64);
    m
}
```

---

## Step 7 — i18n compatibility (three-layer pattern)

Because native functions live in `LoadedModule.functions` (same map as Zymbol functions),
the existing `ExportItem::ReExport` arm already handles them correctly — it calls
`imported_module.functions.get(item_name)` and clones the `Rc<FunctionDef>`.
An `Rc::clone` on a `FunctionDef::Native` is a pointer copy: zero cost.

### Spanish adapter for std/math (for Zofia)

```zymbol
# modulos/matematica_std {
    <# std/math => _mat

    #> {
        _mat::sqrt  => raiz
        _mat::exp   => exp
        _mat::ln    => ln
        _mat::log   => log
        _mat::pow   => pot
        _mat::sin   => sen
        _mat::cos   => cos
        _mat::abs   => abs
        _mat::max   => max
        _mat::min   => min
        _mat::floor => piso
        _mat::ceil  => techo
        _mat::round => redondear
    }
}
```

Constants workaround (until `alias.CONST` bug is fixed — see ROADMAP Known Gaps):

```zymbol
# modulos/constantes_mat {
    PI := 3.141592653589793
    E  := 2.718281828459045

    #> { PI E }
}
```

Consumer (Zofia `activacion.zy`, `atencion.zy`, etc.):

```zymbol
<# modulos/matematica_std => mat
<# modulos/constantes_mat => cte

-- sigmoid
sigmoide(x) {
    <~ 1.0 / (1.0 + mat::exp(-x))
}

-- softmax (single vector)
softmax(vec) {
    exps  = vec$> (v -> mat::exp(v))
    total = exps$< (0.0, (acc, v) -> acc + v)
    <~ exps$> (v -> v / total)
}
```

---

## Step 8 — Semantic analyzer and LSP: no changes required

The semantic analyzer validates `module_alias::function_name` by checking whether the
module alias is in scope. It does not inspect the module's content — it emits at most
a "module not found" warning if the import path is unresolvable at analysis time.

Stdlib imports start with `std/` which the analyzer cannot resolve to a `.zy` file. It
will emit a false-positive "module not found" warning for `<# std/math`. Two options:

- **Short term (v0.0.6):** suppress the warning for bare paths starting with `std/`.
  One-line fix in `zymbol-semantic/src/modules.rs` where the import path is validated.
- **Long term:** the analyzer learns the stdlib manifest.

Add to `zymbol-semantic/src/modules.rs` (exact location TBD at implementation time):

```rust
// Suppress "module not found" for stdlib paths
if import.path.components.first().map(|s| s == "std").unwrap_or(false)
    && !import.path.is_relative && !import.path.is_absolute
{
    return;   // stdlib import — valid, skip filesystem check
}
```

---

## Step 9 — Test cases

New test files in `zyquality/corpus/stdlib/`:

| File | Tests |
|------|-------|
| `stdlib_math_unary.zy` | `raiz(4.0)→2.0`, `exp(0.0)→1.0`, `ln(1.0)→0.0`, `sen(0.0)→0.0`, `cos(0.0)→1.0`, `abs(-3)→3`, `abs(-2.5)→2.5`, `piso(2.9)→2.0`, `techo(2.1)→3.0`, `redondear(2.5)→3.0` |
| `stdlib_math_binary.zy` | `pot(2.0, 10.0)→1024.0`, `log(100.0, 10.0)→2.0`, `log(1.0)→0.0` (natural), `max(3,5)→5`, `min(3,5)→3` |
| `stdlib_math_constants.zy` | Workaround import of `constantes_mat`, verify PI ≈ 3.14159, E ≈ 2.71828 |
| `stdlib_math_type_err.zy` | `raiz("x")` → runtime error with `mat::raiz` in message |
| `stdlib_math_promotion.zy` | `raiz(4)` (Int arg) → `2.0` Float (Int → Float promotion) |
| `stdlib_random_entero.zy` | `entero(1, 6)` returns Int in [1,6] over 100 calls |
| `stdlib_random_rango.zy` | `rango(10)` returns Int in [0,9] over 100 calls |
| `stdlib_random_peso.zy` | `peso_f64()` returns Float in [-0.1, 0.1] over 100 calls |
| `stdlib_i18n_math_es.zy` | Spanish adapter import, call `mat::sen(0.0)` → `0.0` |

Add `# vm-skip` at the top of each file. Update `tests/scripts/vm_compare.sh`
to skip files containing that marker (same mechanism as any existing vm-skip tests).

---

## Step 10 — VM parity (deferred — post v0.0.6)

The VM does not support stdlib modules in v0.0.6. Programs using `<# std/*` must run
with the default tree-walker (no `--vm` flag). A later sprint adds:

### 10a. New instruction in `zymbol-bytecode`

```rust
// In the Instruction enum
NativeCall {
    dst:  Reg,           // register to store the result
    id:   u32,           // index into CompiledProgram.native_fn_table
    args: Vec<Reg>,      // argument registers
},
```

### 10b. Native function table in `CompiledProgram`

```rust
pub struct CompiledProgram {
    // ... existing fields ...
    pub native_fn_table: Vec<NativeFnEntry>,
}

pub struct NativeFnEntry {
    pub module_key: String,   // "std/math"
    pub fn_name:    String,   // "raiz"
    pub func:       fn(Vec<Value>, Span) -> Result<Value>,
}
```

### 10c. Compiler: emit `NativeCall` instead of `Call`

When the compiler encounters `module_alias::func_name` and the alias resolves to a
`__stdlib__/` path, look up the function in the native fn table and emit `NativeCall`
with the corresponding id.

### 10d. VM executor: dispatch `NativeCall`

```rust
Instruction::NativeCall { dst, id, args } => {
    let entry = &program.native_fn_table[*id as usize];
    let arg_vals: Vec<Value> = args.iter()
        .map(|r| self.reg(*r).clone())
        .collect();
    let result = (entry.func)(arg_vals, Span::default())?;
    self.set_reg(*dst, result);
}
```

### 10e. `zymbol check` warning

When `--vm` is combined with a `<# std/*` import, emit:

```
warning: std/math uses native functions — '--vm' mode not yet supported.
         run without '--vm' or the call will fail at runtime.
```

---

## Summary of changed files

| File | Type | Change |
|------|------|--------|
| `crates/zymbol-interpreter/src/lib.rs` | edit | `enum FunctionDef` (Step 1), `mod stdlib`, `Value::type_name()` |
| `crates/zymbol-interpreter/src/functions_lambda.rs` | edit | Native dispatch in `eval_traditional_function_call`, guard in `func_def_to_value` (Step 2) |
| `crates/zymbol-interpreter/src/modules.rs` | edit | stdlib intercept in `load_import`, new `load_stdlib_module` (Step 3) |
| `crates/zymbol-interpreter/src/stdlib/mod.rs` | new | registry + `build_module` (Step 4) |
| `crates/zymbol-interpreter/src/stdlib/math.rs` | new | `std/math` functions + `register()` (Step 5) |
| `crates/zymbol-interpreter/src/stdlib/random.rs` | new | `std/random` functions + `register()` (Step 6) |
| `crates/zymbol-semantic/src/modules.rs` | edit | suppress false-positive for `std/` imports (Step 8) |
| `interpreter/tests/stdlib/*.zy` | new | test suite (Step 9) |

Parser · AST · Lexer · VM · CLI · Formatter · LSP: **no changes required.**

---

## Relation to IMPL_V007.md

> **Done.** The three steps below were carried out; `IMPL_V007.md` has since been rewritten
> against the resulting architecture rather than the `NativeFn` one it originally described.
> Verified 2026-08-31: `enum FunctionDef` exists at `crates/zymbol-interpreter/src/lib.rs`,
> and `grep -rn "native_fns\|NativeFn" crates/` returns nothing. The list is kept because it
> records what the migration actually consisted of.

IMPL_V007 documents `std/env`, `std/io`, `std/json`, `std/net` using a `NativeFn`
field added to `LoadedModule`. After this document's Steps 0–4 were implemented,
those four modules were ported by:

1. Replacing each `NativeFn(Rc::new(|args, span| { ... }))` with a top-level
   `fn module_funcname(args: Vec<Value>, span: Span) -> Result<Value>` and
   a `FunctionDef::Native { ... }` entry in `register()`.
2. Removing the `native_fns` field from `LoadedModule` — it is no longer needed.
3. Removing the `NativeFn` type and the native-first check in `functions_lambda.rs`
   that IMPL_V007 added (Step 5 of that document) — the enum dispatch already covers it.

The IMPL_V007 plan remains valid as a reference for the function signatures and
argument validation logic of those four modules. Only the wiring changed.
