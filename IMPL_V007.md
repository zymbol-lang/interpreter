# Implementation Plan — v0.0.7 Standard Library Expansion

> **Status: RELEASED in v0.0.7 (2026-07-02).** All in-scope modules shipped with
> full TW+VM parity (`std/json`, `std/io`, `std/net`, plus `std/db` — see
> `DESIGN_STD_DB.md`); `std/env` dropped by design. Kept as the architecture
> reference for adding future stdlib modules.

Native stdlib modules built in Rust, consumed transparently through the existing
module system (`<# std/<name> => alias`, `alias::func(...)`). No new syntax.

This document reflects the **actual shipped architecture** (introduced in v0.0.6 for
`std/math` and `std/random`), not an aspirational design. New modules must follow the
same pattern in both execution engines.

---

## Status

| Module     | Functions                                            | TW | VM | Notes |
|------------|------------------------------------------------------|----|----|-------|
| `std/math` | sqrt, exp, ln, log, pow, sin, cos, tan, …, round     | ✅ | ✅ | + constants `PI`, `E` |
| `std/random` | entero, rango, peso_f64                            | ✅ | ✅ | |
| `std/json` | `decode`, `decode_map`, `encode`                     | ✅ | ✅ | v0.0.7 — decode_map: recursive key rename (data i18n) |
| `std/io`   | read, write, append, exists, delete, list, mkdir     | ✅ | ✅ | v0.0.7 |
| `std/net`  | get, post, post_json, head                           | ✅ | ✅ | v0.0.7 |
| `std/env`  | —                                                    | ❌ | ❌ | dropped — redundant (see below) |

---

## Symbol vs module — the design rubric

Zymbol is symbolic and has no words in its grammar. Before adding any capability, decide whether it is
a **symbol** or a **`std/` module**, using the contract already established by the language:

- **Symbol** if it is a *flow/channel of the running process itself* — anonymous, ambient,
  no named arguments — expressible by composing the directional symbol algebra
  (`>` out, `<` in, doubled = strong flow, `|` gate, `?` query). Existing examples:
  `>>` stdout, `<<` stdin, `<<|`/`<<|?` read key, `><` capture CLI args, `<\ \>` shell,
  `<#` import. See `SYMBOLS.md` → "Design Rules for New Operators".
- **`module::func()`** if it is a *named operation on an addressed resource* that takes
  arguments (a path, a URL, a JSON string) and returns a value. Examples: `math::sin(x)`.

Fine discriminator: ambient/anonymous source (the process's own stdin) → symbol;
named/addressed source (a path, a URL) → module.

Applied to v0.0.7:
- **json, net, io = modules.** No symbol collision (json/net are not process flows;
  file-by-path I/O is addressed, distinct from the ambient `<<`/`>>`).
- **env = DROPPED (not built).** Its capabilities are already reachable through existing
  language features, so a module would be a redundant second form — exactly what breaks the
  symbolic coherence:
  - environment variables → `<\ "printenv API_KEY" \>` (shell-exec returns a clean string;
    the trailing newline is already trimmed by `eval_bash_exec`),
  - current working directory → `<\ "pwd" \>`,
  - home directory → `<\ "printenv HOME" \>`,
  - CLI args → `><` (already a symbol).

  Accessing the process/OS context is the system's job, and the language already bridges to
  it through the shell channel `<\ \>`. Known trade-offs vs a hypothetical native module
  (all judged acceptable because `<\ \>` is shell-coupled by design anyway): less portable
  (`printenv` is POSIX), spawns a shell per read, and cannot distinguish an unset variable
  from an empty one. None justify a second form.

---

## Error convention (applies to every module)

Two channels, used consistently:

- **Hard `RuntimeError`** (aborts execution) for *programmer mistakes*: wrong argument
  type, wrong arity. Aborts because the program is malformed.
- **Soft `Value::Error`** (catchable with try-catch) for *recoverable environmental
  failures*: file not found, network timeout, malformed JSON. Returned as a value so the
  program can handle it.

`math` uses only hard errors (type mismatches). `json`/`io`/`net` add soft errors for the
outside world.

---

## Architecture — tree-walker (default engine)

Stdlib modules live under `crates/zymbol-interpreter/src/stdlib/`.

### 1. Per-module `register()` — `stdlib/<name>.rs`

Each module exposes:

```rust
pub(crate) fn register() -> HashMap<String, Rc<FunctionDef>> { … }
```

Functions are plain top-level `fn`s with the native signature, wrapped in
`FunctionDef::Native`:

```rust
fn <name>(args: Vec<Value>, span: Span) -> Result<Value> { … }

// in register():
m.insert("name".into(), Rc::new(FunctionDef::Native {
    name: "name", arity: 1, func: <name>,
}));
```

`arity` is validated centrally by the dispatcher (`-1` = variadic). Functions only need to
check argument *types*, not count. The call-site `Span` is forwarded for hard-error
messages.

### 2. Registry — `stdlib/mod.rs`

`build_module(name: &str) -> Option<LoadedModule>` has one match arm per module that
assembles a `LoadedModule` from `register()` (and inserts constants if any, as `std/math`
does for `PI`/`E`).

```rust
"std/json" => Some(LoadedModule {
    name: "std/json".to_string(),
    functions: json::register(),
    all_functions: HashMap::new(),
    constants: HashMap::new(),
    all_variables: HashMap::new(),
    import_aliases: HashMap::new(),
    loaded_modules_refs: HashMap::new(),
}),
```

### 3. Routing — `modules.rs`

`load_import` intercepts any bare import path whose first component is `std` and calls
`load_stdlib_module`, which keys `loaded_modules` by a synthetic `__stdlib__/<name>` path.
No filesystem access. Already implemented — new modules need no change here.

### 4. Re-export (i18n) — automatic

Because native functions live in `LoadedModule::functions` (same map as user functions),
the existing re-export machinery handles them with **zero changes**. The i18n three-layer
pattern works out of the box:

```zymbol
# json_es {
    <# std/json => _json
    #> {
        _json::decode => decodificar
        _json::encode => codificar
    }
}
```

---

## Architecture — register VM (`--vm`)

The VM does **not** load stdlib modules from `functions`; it maps `alias::func` calls to a
numeric builtin id at compile time and dispatches them at run time. New modules must add
parity in three places, or they only work in the tree-walker.

### 1. Builtin ids — `crates/zymbol-bytecode/src/lib.rs` (`mod builtins`)

Add `u16` constants. Convention: math = 0-block, random = 100-block, json = 200-block,
io = 300-block, net = 400-block.

```rust
// std/json functions
pub const JSON_DECODE: u16 = 200;
pub const JSON_ENCODE: u16 = 201;
```

### 2. Compiler emit site — `crates/zymbol-compiler/src/lib.rs`

Add a match arm to `stdlib_builtin_entries(module_key)` mapping `(func_name, builtin_id)`:

```rust
"std/json" => Some(vec![
    ("decode", B::JSON_DECODE),
    ("encode", B::JSON_ENCODE),
]),
```

`compile_import` detects bare `std/*` paths and registers these in `builtin_map`.
Constants (if any) are inserted there too (see the `std/math` branch). Re-exports of
stdlib builtins through adapter modules are already propagated.

### 3. VM dispatch — `crates/zymbol-vm/src/stdlib_builtins.rs`

Implement the builtins against the **VM `Value`** type (note: `String(ZyStr)`,
`Array(Rc<Vec<Value>>)`, `NamedTuple(Rc<Vec<(String, Value)>>)`, `Error(ZyStr)`), and add a
dispatch arm in `call(builtin_id, args)`:

```rust
B::JSON_DECODE => json_decode(args),
B::JSON_ENCODE => json_encode(args),
```

VM builtins return `Result<Value, String>`: `Err` becomes a hard runtime error; soft
errors are returned as `Ok(Value::Error(ZyStr::new(format!("##{Kind}({msg})"))))` to match
the tree-walker's `Value::Error(ErrorValue)` display.

### 4. External crates

Add any crate dependency to **both** `zymbol-interpreter` and `zymbol-vm` (and the
workspace `[workspace.dependencies]`). `serde_json` was added in v0.0.7 for json.

---

## Reference: `std/json` (v0.0.7, done)

- `decode(text)  -> Value | Error` — parse JSON; malformed input → soft `##Parse(...)`.
- `encode(value) -> String | Error` — serialize; failure → soft `##Parse(...)`.

Value mapping (both directions): JSON object ↔ `NamedTuple` (key order preserved via
serde_json `preserve_order`), JSON array ↔ `Array`, JSON null ↔ `Unit`, number → `Int`
when integral else `Float`. Tuples encode as arrays; functions/errors encode as null.

Naming: `decode`/`encode` (symmetric inverse pair), not `parse`/`encode`.

Display note: a JSON `null` decoded into a collection renders as `()` in both engines
(unified 2026-06-12 — previously the tree-walker printed an empty hole; see
`tests/collections/unit_display_nested.zy`).

---

## `std/io` (v0.0.7, done)

Filesystem operations on a path. All return soft `Value::Error` of kind `IO` on failure;
hard `RuntimeError` only on wrong argument type.

| Function            | Signature                          | Result on success |
|---------------------|------------------------------------|-------------------|
| `read(path)`        | String → …                         | file contents (String) |
| `write(path, text)` | (String, String) → …               | Unit |
| `append(path, text)`| (String, String) → …               | Unit |
| `exists(path)`      | String → …                         | Bool |
| `delete(path)`      | String → …                         | Unit (file or dir) |
| `list(dir)`         | String → …                         | Array<String> of entry names |
| `mkdir(path)`       | String → …                         | Unit (creates parents) |

`exists` never fails (returns `#0` for missing). `list` order is OS-dependent — tests must
not assert raw order (check length / membership / a sorted view).

Builtin ids: `IO_READ = 300 …`. TW in `stdlib/io.rs`, VM in `stdlib_builtins.rs`.

---

## `std/net` (v0.0.7, done)

Synchronous HTTP via `ureq` 2.x (no async/tokio; rustls TLS). All return soft
`Value::Error` of kind `Network` (`##Network(...)`) on failure; wrong arg type aborts hard.

- `get(url[, headers]) -> String | Error`
- `post(url, body[, headers]) -> String | Error`  (Content-Type: text/plain)
- `post_json(url, body[, headers]) -> String | Error`  (Content-Type: application/json)
- `head(url) -> Bool`  (reachable? — `#1` on 2xx, `#0` on any error)

`ureq` is a dependency of the workspace, `zymbol-interpreter`, and `zymbol-vm`.

**Custom headers.** `get`/`post`/`post_json` take an optional trailing `headers`
argument — an Array of 2-element `(String, String)` tuples, e.g.
`[("x-api-key", key), ("anthropic-version", "2023-06-01")]`. Tuples (not named
tuples) because header names like `x-api-key` contain hyphens, which are not valid
field identifiers. These functions register with `arity -1` (variadic) and validate
arg count internally. This is what lets the module reach authenticated APIs
(Claude, OpenAI, Groq); Gemini needs no header (key goes in the URL). A malformed
`headers` value is a hard `RuntimeError`. Verified TW == VM against httpbin's header
echo. See `examples/zethy_cli/` for an end-to-end consumer (net + json + io).

**Testing note:** live HTTP output is non-deterministic, so the committed golden test
(`stdlib_net_type_err.zy`) only covers the deterministic, offline hard-error path (wrong
arg type), matching `stdlib_math_type_err`. The live get/post/post_json/head success and
soft-error paths were verified manually (TW == VM) against `example.com` / `httpbin.org`.
Do not add live-network golden tests — they would be flaky in `vm_compare`/CI.

---

## Tests

New `.zy` + `.expected` pairs under `interpreter/tests/stdlib/`. Generate goldens with:

```bash
bash tests/scripts/expected_compare.sh stdlib --regen   # write .expected
bash tests/scripts/expected_compare.sh stdlib           # verify
bash tests/scripts/vm_compare.sh                        # TW == VM parity
```

Every stdlib module ships with VM parity, so tests run under both engines with **no
`@vm-skip`**. Design tests to produce identical TW/VM output (avoid the `Unit`-in-array
divergence; for `io`, use deterministic temp paths and clean up within the script).

Adapter module files (e.g. `json_es.zy`) get an empty `.expected` (running a module file
directly only emits a stripped warning).

JSON literals in source must escape `{` as `\{` (an unescaped `{` starts string
interpolation). This does not affect JSON read from files or the network.

---

## Summary of the per-module checklist

1. `stdlib/<name>.rs` — `register()` + native `fn`s (TW).
2. `stdlib/mod.rs` — `build_module` match arm.
3. `zymbol-bytecode` `builtins` — id constants (VM).
4. `zymbol-compiler` `stdlib_builtin_entries` — `(name, id)` arm (VM).
5. `zymbol-vm` `stdlib_builtins.rs` — builtin impls + dispatch arm (VM).
6. Cargo deps in workspace + interpreter + vm if an external crate is needed.
7. Tests in `tests/stdlib/` with regenerated `.expected`; verify both engines.

Parser · AST · Lexer · CLI: **no changes required**.
