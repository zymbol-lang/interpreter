# Implementation Plan — v0.0.6 Native Standard Library

Native stdlib modules built in Rust, consumed transparently via the existing module system.
No new syntax. Full re-export support for the i18n three-layer pattern.

---

## Design principles

1. **Transparent to the user.** `<# std/env <= env` and `env::get("KEY")` are indistinguishable
   from a `.zy` module. Same syntax, same call convention, same re-export rules.

2. **Re-exportable for i18n.** A translation module can re-export native functions under any
   name: `env::get <= obtener`. The i18n three-layer pattern (`I18N.md`) works unchanged.

3. **Zero parser / AST / lexer changes.** All changes are confined to `zymbol-interpreter`
   and a new `stdlib/` subtree inside it.

4. **Blocking I/O only.** All stdlib functions are synchronous. No async, no tokio.
   Compatible with the single-threaded tree-walker model.

5. **VM parity deferred.** Native stdlib works in tree-walker mode first. VM parity
   (`Instruction::NativeCall`) is a follow-up task tracked at the end of this document.

---

## Feature map

| # | Module     | Functions                                      | Cargo dep        |
|---|------------|------------------------------------------------|------------------|
| 1 | `std/env`  | `get`, `set`, `args`, `home`                   | none (std only)  |
| 2 | `std/io`   | `read`, `write`, `append`, `exists`, `delete`, `list` | none (std only) |
| 3 | `std/json` | `parse`, `encode`                              | `serde_json`     |
| 4 | `std/net`  | `get`, `post`, `post_json`, `head`             | `ureq`           |

Recommended implementation order: 1 → 2 → 3 → 4.
Steps 0–5 are the infrastructure that all modules share; implement them once before any module.

---

## Step 0 — Add cargo dependencies

**File:** `Cargo.toml` (workspace root)

Add to `[workspace.dependencies]`:

```toml
serde_json = { version = "1", features = ["preserve_order"] }
ureq       = { version = "2", features = ["json"] }
```

**File:** `crates/zymbol-interpreter/Cargo.toml`

Add to `[dependencies]`:

```toml
serde_json = { workspace = true }
ureq       = { workspace = true }
```

---

## Step 1 — `NativeFn` wrapper type + field in `LoadedModule`

**File:** `crates/zymbol-interpreter/src/modules.rs`

### 1a. New type `NativeFn`

Add before `pub(crate) struct LoadedModule`:

```rust
/// A Rust function exposed as a Zymbol stdlib function.
/// `Clone` via `Rc::clone` (zero cost). `Debug` prints `<native-fn>`.
#[derive(Clone)]
pub(crate) struct NativeFn(pub Rc<dyn Fn(Vec<Value>, zymbol_span::Span) -> Result<Value>>);

impl std::fmt::Debug for NativeFn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<native-fn>")
    }
}
```

The `Span` parameter is the call-site span, forwarded so error messages point to the
correct source location instead of a zero span.

### 1b. New field in `LoadedModule`

Add `native_fns` as the last field of `LoadedModule`:

```rust
pub(crate) struct LoadedModule {
    #[allow(dead_code)]
    pub(crate) name: String,
    pub(crate) functions: HashMap<String, Rc<FunctionDef>>,
    pub(crate) all_functions: HashMap<String, Rc<FunctionDef>>,
    pub(crate) constants: HashMap<String, Value>,
    pub(crate) all_variables: HashMap<String, Value>,
    pub(crate) import_aliases: HashMap<String, PathBuf>,
    #[allow(dead_code)]
    pub(crate) loaded_modules_refs: HashMap<PathBuf, ()>,
    pub(crate) native_fns: HashMap<String, NativeFn>,   // ← new
}
```

### 1c. Initialize `native_fns` in every existing `LoadedModule` literal

Every place in `modules.rs` that constructs a `LoadedModule` (currently one: inside
`load_module`) must add the new field:

```rust
native_fns: HashMap::new(),
```

---

## Step 2 — Intercept stdlib imports in `load_import`

**File:** `crates/zymbol-interpreter/src/modules.rs`

Detection condition: no path prefix (not relative, not absolute) and first component is `"std"`.

Add at the top of `load_import`, before the circular-import check:

```rust
pub(crate) fn load_import(&mut self, import: &zymbol_ast::ImportStmt) -> Result<()> {
    // Route stdlib: bare path whose first component is "std"
    if !import.path.is_relative
        && !import.path.is_absolute
        && import.path.components.first().map(|s| s == "std").unwrap_or(false)
    {
        return self.load_stdlib_module(&import.path, &import.alias);
    }

    // ... rest of existing code unchanged ...
}
```

---

## Step 3 — `load_stdlib_module` method

**File:** `crates/zymbol-interpreter/src/modules.rs`

Add as a new `impl` method on `Interpreter<W>`:

```rust
fn load_stdlib_module(
    &mut self,
    path: &zymbol_ast::ModulePath,
    alias: &str,
) -> Result<()> {
    let module_key = path.components.join("/");   // e.g. "std/env"

    // Look up the registry; error if the module does not exist
    let native_fns = crate::stdlib::get_module(&module_key)
        .ok_or_else(|| RuntimeError::ModuleNotFound {
            path: module_key.clone(),
        })?;

    // Synthetic PathBuf used as the key in loaded_modules.
    // The "__stdlib__/" prefix never collides with real filesystem paths.
    let synthetic_path = PathBuf::from(format!("__stdlib__/{}", module_key));

    // Skip if already loaded (same alias registered twice is fine)
    if !self.loaded_modules.contains_key(&synthetic_path) {
        self.loaded_modules.insert(
            synthetic_path.clone(),
            LoadedModule {
                name: module_key,
                functions: HashMap::new(),
                all_functions: HashMap::new(),
                constants: HashMap::new(),
                all_variables: HashMap::new(),
                import_aliases: HashMap::new(),
                loaded_modules_refs: HashMap::new(),
                native_fns,
            },
        );
    }

    self.import_aliases
        .insert(alias.to_string(), synthetic_path);
    Ok(())
}
```

---

## Step 4 — Re-export of native functions

**File:** `crates/zymbol-interpreter/src/modules.rs`

This is the **critical gap**: the existing `ReExport` arm (around line 243) only checks
`imported_module.functions` and `imported_module.constants`. Native functions are invisible
to the re-export mechanism without this fix.

Replace the inner body of the `ExportItem::ReExport` arm:

```rust
zymbol_ast::ExportItem::ReExport {
    module_alias,
    item_name,
    rename,
    ..
} => {
    let export_name = rename.as_ref().unwrap_or(item_name);

    if let Some(imported_path) = module_interp.import_aliases.get(module_alias) {
        if let Some(imported_module) = module_interp.loaded_modules.get(imported_path) {
            if let Some(func) = imported_module.functions.get(item_name) {
                loaded_module.functions.insert(export_name.clone(), func.clone());
            } else if let Some(native) = imported_module.native_fns.get(item_name) {
                // Re-export native function: Rc::clone, zero cost
                loaded_module.native_fns.insert(export_name.clone(), native.clone());
            } else if let Some(val) = imported_module.constants.get(item_name) {
                loaded_module.constants.insert(export_name.clone(), val.clone());
            }
        }
    }
}
```

This makes the i18n three-layer pattern work for stdlib modules:

```zymbol
# .entorno_es {
    #> {
        env::get  <= obtener
        env::set  <= establecer
        env::args <= argumentos
    }
    <# std/env <= env
}
```

A consumer can then write `entorno::obtener("KEY")` — the `NativeFn` Rc is cloned once at
load time and dispatched at call time with zero indirection overhead.

---

## Step 5 — Call dispatch for native functions

**File:** `crates/zymbol-interpreter/src/functions_lambda.rs`

In the `Expr::MemberAccess` branch of `eval_call` (around line 177), add a native check
**before** the existing `module.functions.get(func_name)` lookup:

```rust
// Existing: get module reference
let module = self.loaded_modules.get(module_path).ok_or_else(|| {
    RuntimeError::Generic {
        message: format!("module '{}' not loaded", module_alias),
        span: call.span,
    }
})?;

// NEW: check native functions first
if let Some(native) = module.native_fns.get(func_name).cloned() {
    let mut arg_values = Vec::with_capacity(call.arguments.len());
    for arg in &call.arguments {
        arg_values.push(self.eval_expr(arg)?);
    }
    return (native.0)(arg_values, call.span);
}

// Existing: look up Zymbol FunctionDef (unchanged)
let func_def = module.functions.get(func_name).cloned().ok_or_else(|| {
    RuntimeError::FunctionNotExported {
        module: module_alias.clone(),
        function: func_name.clone(),
    }
})?;
```

---

## Step 6 — Stdlib registry

**File:** `crates/zymbol-interpreter/src/stdlib/mod.rs` *(new file)*

```rust
use std::collections::HashMap;
use crate::modules::NativeFn;

mod env;
mod io;
mod json;
mod net;

/// Return the native function map for a stdlib module path, or None if unknown.
pub(crate) fn get_module(name: &str) -> Option<HashMap<String, NativeFn>> {
    match name {
        "std/env"  => Some(env::register()),
        "std/io"   => Some(io::register()),
        "std/json" => Some(json::register()),
        "std/net"  => Some(net::register()),
        _          => None,
    }
}
```

**File:** `crates/zymbol-interpreter/src/lib.rs`

Add module declaration (with the other `mod` declarations near the top):

```rust
mod stdlib;
```

---

## Step 7 — `std/env`

**File:** `crates/zymbol-interpreter/src/stdlib/env.rs` *(new file)*

```rust
use std::collections::HashMap;
use std::rc::Rc;
use crate::{Result, RuntimeError, Value};
use crate::modules::NativeFn;

pub(crate) fn register() -> HashMap<String, NativeFn> {
    let mut m = HashMap::new();

    // env::get("KEY") -> String | Unit
    m.insert("get".into(), NativeFn(Rc::new(|args, span| {
        match args.into_iter().next() {
            Some(Value::String(key)) => Ok(match std::env::var(&key) {
                Ok(val) => Value::String(val),
                Err(_)  => Value::Unit,
            }),
            _ => Err(RuntimeError::Generic {
                message: "env::get: expected String".into(),
                span,
            }),
        }
    })));

    // env::set("KEY", "VALUE") -> Unit
    m.insert("set".into(), NativeFn(Rc::new(|args, span| {
        let mut it = args.into_iter();
        match (it.next(), it.next()) {
            (Some(Value::String(k)), Some(Value::String(v))) => {
                std::env::set_var(&k, &v);
                Ok(Value::Unit)
            }
            _ => Err(RuntimeError::Generic {
                message: "env::set: expected (String, String)".into(),
                span,
            }),
        }
    })));

    // env::args() -> Array<String>
    m.insert("args".into(), NativeFn(Rc::new(|_args, _span| {
        let values: Vec<Value> = std::env::args().skip(1).map(Value::String).collect();
        Ok(Value::Array(values))
    })));

    // env::home() -> String | Unit
    m.insert("home".into(), NativeFn(Rc::new(|_args, _span| {
        Ok(match std::env::var("HOME") {
            Ok(h) => Value::String(h),
            Err(_) => Value::Unit,
        })
    })));

    m
}
```

---

## Step 8 — `std/io`

**File:** `crates/zymbol-interpreter/src/stdlib/io.rs` *(new file)*

```rust
use std::collections::HashMap;
use std::rc::Rc;
use crate::{Result, RuntimeError, Value};
use crate::interpreter::ErrorValue;
use crate::modules::NativeFn;

pub(crate) fn register() -> HashMap<String, NativeFn> {
    let mut m = HashMap::new();

    // io::read("path") -> String | Error
    m.insert("read".into(), NativeFn(Rc::new(|args, span| {
        match args.into_iter().next() {
            Some(Value::String(path)) => match std::fs::read_to_string(&path) {
                Ok(content) => Ok(Value::String(content)),
                Err(e)      => Ok(Value::Error(ErrorValue::io(e.to_string()))),
            },
            _ => Err(RuntimeError::Generic {
                message: "io::read: expected String path".into(),
                span,
            }),
        }
    })));

    // io::write("path", "content") -> Unit | Error
    m.insert("write".into(), NativeFn(Rc::new(|args, span| {
        let mut it = args.into_iter();
        match (it.next(), it.next()) {
            (Some(Value::String(path)), Some(Value::String(content))) => {
                match std::fs::write(&path, content.as_bytes()) {
                    Ok(_)  => Ok(Value::Unit),
                    Err(e) => Ok(Value::Error(ErrorValue::io(e.to_string()))),
                }
            }
            _ => Err(RuntimeError::Generic {
                message: "io::write: expected (String, String)".into(),
                span,
            }),
        }
    })));

    // io::append("path", "content") -> Unit | Error
    m.insert("append".into(), NativeFn(Rc::new(|args, span| {
        let mut it = args.into_iter();
        match (it.next(), it.next()) {
            (Some(Value::String(path)), Some(Value::String(content))) => {
                use std::io::Write;
                match std::fs::OpenOptions::new().append(true).create(true).open(&path) {
                    Ok(mut f) => match f.write_all(content.as_bytes()) {
                        Ok(_)  => Ok(Value::Unit),
                        Err(e) => Ok(Value::Error(ErrorValue::io(e.to_string()))),
                    },
                    Err(e) => Ok(Value::Error(ErrorValue::io(e.to_string()))),
                }
            }
            _ => Err(RuntimeError::Generic {
                message: "io::append: expected (String, String)".into(),
                span,
            }),
        }
    })));

    // io::exists("path") -> Bool
    m.insert("exists".into(), NativeFn(Rc::new(|args, span| {
        match args.into_iter().next() {
            Some(Value::String(path)) => Ok(Value::Bool(std::path::Path::new(&path).exists())),
            _ => Err(RuntimeError::Generic {
                message: "io::exists: expected String path".into(),
                span,
            }),
        }
    })));

    // io::delete("path") -> Unit | Error
    m.insert("delete".into(), NativeFn(Rc::new(|args, span| {
        match args.into_iter().next() {
            Some(Value::String(path)) => {
                let p = std::path::Path::new(&path);
                let result = if p.is_dir() {
                    std::fs::remove_dir_all(p)
                } else {
                    std::fs::remove_file(p)
                };
                match result {
                    Ok(_)  => Ok(Value::Unit),
                    Err(e) => Ok(Value::Error(ErrorValue::io(e.to_string()))),
                }
            }
            _ => Err(RuntimeError::Generic {
                message: "io::delete: expected String path".into(),
                span,
            }),
        }
    })));

    // io::list("dir") -> Array<String> | Error
    m.insert("list".into(), NativeFn(Rc::new(|args, span| {
        match args.into_iter().next() {
            Some(Value::String(path)) => match std::fs::read_dir(&path) {
                Ok(entries) => {
                    let mut names = Vec::new();
                    for entry in entries.flatten() {
                        names.push(Value::String(
                            entry.file_name().to_string_lossy().into_owned(),
                        ));
                    }
                    Ok(Value::Array(names))
                }
                Err(e) => Ok(Value::Error(ErrorValue::io(e.to_string()))),
            },
            _ => Err(RuntimeError::Generic {
                message: "io::list: expected String path".into(),
                span,
            }),
        }
    })));

    m
}
```

---

## Step 9 — `std/json`

**File:** `crates/zymbol-interpreter/src/stdlib/json.rs` *(new file)*

```rust
use std::collections::HashMap;
use std::rc::Rc;
use crate::{Result, RuntimeError, Value};
use crate::interpreter::ErrorValue;
use crate::modules::NativeFn;

/// Convert a serde_json Value into a Zymbol Value (best-effort).
/// JSON objects become NamedTuple. JSON arrays become Array.
fn json_to_value(v: serde_json::Value) -> Value {
    match v {
        serde_json::Value::Null             => Value::Unit,
        serde_json::Value::Bool(b)          => Value::Bool(b),
        serde_json::Value::Number(n)        => {
            if let Some(i) = n.as_i64() { Value::Int(i) }
            else { Value::Float(n.as_f64().unwrap_or(f64::NAN)) }
        }
        serde_json::Value::String(s)        => Value::String(s),
        serde_json::Value::Array(arr)       =>
            Value::Array(arr.into_iter().map(json_to_value).collect()),
        serde_json::Value::Object(map)      =>
            Value::NamedTuple(
                map.into_iter()
                   .map(|(k, v)| (k, json_to_value(v)))
                   .collect()
            ),
    }
}

/// Convert a Zymbol Value into a serde_json Value (best-effort).
fn value_to_json(v: Value) -> serde_json::Value {
    match v {
        Value::Unit            => serde_json::Value::Null,
        Value::Bool(b)         => serde_json::Value::Bool(b),
        Value::Int(i)          => serde_json::json!(i),
        Value::Float(f)        => serde_json::json!(f),
        Value::String(s)       => serde_json::Value::String(s),
        Value::Char(c)         => serde_json::Value::String(c.to_string()),
        Value::Array(arr)      =>
            serde_json::Value::Array(arr.into_iter().map(value_to_json).collect()),
        Value::Tuple(fields)   =>
            serde_json::Value::Array(fields.into_iter().map(value_to_json).collect()),
        Value::NamedTuple(pairs) =>
            serde_json::Value::Object(
                pairs.into_iter()
                     .map(|(k, v)| (k, value_to_json(v)))
                     .collect()
            ),
        Value::Function(_) | Value::Error(_) => serde_json::Value::Null,
    }
}

pub(crate) fn register() -> HashMap<String, NativeFn> {
    let mut m = HashMap::new();

    // json::parse("text") -> Value | Error
    m.insert("parse".into(), NativeFn(Rc::new(|args, span| {
        match args.into_iter().next() {
            Some(Value::String(text)) => match serde_json::from_str::<serde_json::Value>(&text) {
                Ok(v)  => Ok(json_to_value(v)),
                Err(e) => Ok(Value::Error(ErrorValue::parse(e.to_string()))),
            },
            _ => Err(RuntimeError::Generic {
                message: "json::parse: expected String".into(),
                span,
            }),
        }
    })));

    // json::encode(value) -> String | Error
    m.insert("encode".into(), NativeFn(Rc::new(|args, span| {
        match args.into_iter().next() {
            Some(v) => match serde_json::to_string(&value_to_json(v)) {
                Ok(s)  => Ok(Value::String(s)),
                Err(e) => Ok(Value::Error(ErrorValue::parse(e.to_string()))),
            },
            None => Err(RuntimeError::Generic {
                message: "json::encode: expected one argument".into(),
                span,
            }),
        }
    })));

    m
}
```

---

## Step 10 — `std/net`

**File:** `crates/zymbol-interpreter/src/stdlib/net.rs` *(new file)*

Uses `ureq` — synchronous, no async, minimal overhead.

```rust
use std::collections::HashMap;
use std::rc::Rc;
use crate::{Result, RuntimeError, Value};
use crate::interpreter::ErrorValue;
use crate::modules::NativeFn;

pub(crate) fn register() -> HashMap<String, NativeFn> {
    let mut m = HashMap::new();

    // net::get("url") -> String | Error
    m.insert("get".into(), NativeFn(Rc::new(|args, span| {
        match args.into_iter().next() {
            Some(Value::String(url)) => match ureq::get(&url).call() {
                Ok(resp) => match resp.into_string() {
                    Ok(body) => Ok(Value::String(body)),
                    Err(e)   => Ok(Value::Error(ErrorValue::new("Network", e.to_string()))),
                },
                Err(e) => Ok(Value::Error(ErrorValue::new("Network", e.to_string()))),
            },
            _ => Err(RuntimeError::Generic {
                message: "net::get: expected String url".into(),
                span,
            }),
        }
    })));

    // net::post("url", "body") -> String | Error
    m.insert("post".into(), NativeFn(Rc::new(|args, span| {
        let mut it = args.into_iter();
        match (it.next(), it.next()) {
            (Some(Value::String(url)), Some(Value::String(body))) => {
                match ureq::post(&url)
                    .set("Content-Type", "text/plain")
                    .send_string(&body)
                {
                    Ok(resp) => match resp.into_string() {
                        Ok(s)  => Ok(Value::String(s)),
                        Err(e) => Ok(Value::Error(ErrorValue::new("Network", e.to_string()))),
                    },
                    Err(e) => Ok(Value::Error(ErrorValue::new("Network", e.to_string()))),
                }
            }
            _ => Err(RuntimeError::Generic {
                message: "net::post: expected (String, String)".into(),
                span,
            }),
        }
    })));

    // net::post_json("url", "json_string") -> String | Error
    m.insert("post_json".into(), NativeFn(Rc::new(|args, span| {
        let mut it = args.into_iter();
        match (it.next(), it.next()) {
            (Some(Value::String(url)), Some(Value::String(body))) => {
                match ureq::post(&url)
                    .set("Content-Type", "application/json")
                    .send_string(&body)
                {
                    Ok(resp) => match resp.into_string() {
                        Ok(s)  => Ok(Value::String(s)),
                        Err(e) => Ok(Value::Error(ErrorValue::new("Network", e.to_string()))),
                    },
                    Err(e) => Ok(Value::Error(ErrorValue::new("Network", e.to_string()))),
                }
            }
            _ => Err(RuntimeError::Generic {
                message: "net::post_json: expected (String, String)".into(),
                span,
            }),
        }
    })));

    // net::head("url") -> Bool  (#1 if reachable, #0 on error)
    m.insert("head".into(), NativeFn(Rc::new(|args, span| {
        match args.into_iter().next() {
            Some(Value::String(url)) => {
                Ok(Value::Bool(ureq::head(&url).call().is_ok()))
            }
            _ => Err(RuntimeError::Generic {
                message: "net::head: expected String url".into(),
                span,
            }),
        }
    })));

    m
}
```

---

## I18N compatibility — three-layer pattern with stdlib

Because Step 4 wires `native_fns` into the re-export path, the full three-layer pattern
from `I18N.md` works for stdlib modules with zero additional changes.

### Example: Spanish adapter for `std/env`

```zymbol
# .entorno_es {
    #> {
        env::get    <= obtener
        env::set    <= establecer
        env::args   <= argumentos
        env::home   <= inicio
    }
    <# std/env <= env
}
```

### Consumer in Spanish

```zymbol
<# ./entorno_es <= entorno

url  = entorno::obtener("DATABASE_URL")
ruta = entorno::inicio()
args = entorno::argumentos()
```

The `NativeFn` Rc is cloned once at module load time (Step 4). Call dispatch (Step 5)
reaches the Rust closure in one `HashMap::get` lookup. No overhead compared to a direct import.

---

## Test cases

New test files in `interpreter/tests/stdlib/`:

| File                              | Tests                                          |
|-----------------------------------|------------------------------------------------|
| `stdlib_env.zy`                   | `get` known key, `get` missing key → Unit, `args`, `home` |
| `stdlib_io_read_write.zy`         | write → read roundtrip, exists #1/#0, delete  |
| `stdlib_io_append.zy`             | append twice, read back concatenated           |
| `stdlib_io_list.zy`               | list dir, check array non-empty                |
| `stdlib_json_parse.zy`            | parse object → NamedTuple, array → Array       |
| `stdlib_json_encode.zy`           | encode NamedTuple → JSON string roundtrip      |
| `stdlib_net_head.zy`              | head known URL #1, head bad URL #0             |
| `stdlib_i18n_env_es.zy`           | re-export adapter + Spanish consumer           |

The `vm_compare.sh` script should **skip** stdlib tests for now (VM parity deferred).
Add a `# vm-skip` comment at the top of each stdlib test file; update the script to
honour that marker.

---

## VM parity (deferred — post v0.0.6)

The VM does not support stdlib modules in v0.0.6. A later sprint adds:

1. **`Instruction::NativeCall(Reg, NativeFnId, Vec<Reg>)`** in `zymbol-bytecode`
2. **`NativeFnTable`** in `CompiledProgram` — Vec of `NativeFn` indexed by `NativeFnId`
3. **Compiler**: when compiling `module::func()` where module is a stdlib path, emit
   `NativeCall` instead of `Call`
4. **VM executor**: `NativeCall` evaluates register arguments, calls the closure, stores result

Until then, programs that use `<# std/*` must run with the tree-walker (default mode,
no `--vm` flag). The `zymbol check` command should emit a warning if `--vm` is combined
with a stdlib import.

---

## Summary of changed files

| File | Type | Change |
|------|------|--------|
| `Cargo.toml` (workspace) | edit | add `serde_json`, `ureq` to workspace deps |
| `crates/zymbol-interpreter/Cargo.toml` | edit | add `serde_json`, `ureq` as deps |
| `crates/zymbol-interpreter/src/lib.rs` | edit | add `mod stdlib;` |
| `crates/zymbol-interpreter/src/modules.rs` | edit | `NativeFn` type, `native_fns` field, `load_stdlib_module`, re-export fix |
| `crates/zymbol-interpreter/src/functions_lambda.rs` | edit | native dispatch before Zymbol dispatch |
| `crates/zymbol-interpreter/src/stdlib/mod.rs` | new | registry |
| `crates/zymbol-interpreter/src/stdlib/env.rs` | new | `std/env` |
| `crates/zymbol-interpreter/src/stdlib/io.rs` | new | `std/io` |
| `crates/zymbol-interpreter/src/stdlib/json.rs` | new | `std/json` |
| `crates/zymbol-interpreter/src/stdlib/net.rs` | new | `std/net` |
| `interpreter/tests/stdlib/*.zy` | new | test suite |

Parser · AST · Lexer · VM · CLI: **no changes required**.
