# DESIGN — `std/db` (vendor-neutral database access via ODBC)

> Design proposal · pre-implementation · target: v0.0.7
> Status: APPROVED (no code yet).
>
> **Philosophy.** Zymbol is a *language*, not an application. It must not bundle a
> database engine. Instead it defines a uniform DB **contract** and integrates
> with third-party engines through one standard wire protocol: **ODBC**. Zymbol
> compiles **zero** vendor code; the OS provides the driver for whatever engine
> the user runs (SQLite for lightweight, or PostgreSQL / MySQL / MS SQL Server /
> Oracle / anything with an ODBC driver).
>
> Motivation: give SPRE (and any data-backed Zymbol program) real database access
> without the `BashExec → sqlite3 CLI + jq` hacks — no missing-`sqlite3` blocker,
> no G4 quote-sanitizing (parameters are bound), no JSON/jq row re-parsing (rows
> come back as `NamedTuple`s).

---

## 1. Goals & non-goals

**Goals**
- One uniform Zymbol API that is **identical across every engine**.
- Backend chosen by the connection string; nothing engine-specific in the API.
- Zymbol bundles **no vendor database code** — only an ODBC client binding.
- Parameter binding so user data with quotes is safe by construction (kills **G4**).
- Result rows delivered as first-class `NamedTuple`s, no JSON/jq.
- Full SQLite-standard SQL transaction model: atomic `tx`, low-level
  `begin`/`commit`/`rollback`, **and** nested `savepoint`/`release`/`rollback_to`.
- Full **TW == VM** parity, like every other stdlib module.

**Non-goals (v0.0.7)**
- Compiling any vendor driver into the binary (that is the OS's job, via ODBC).
- A native Zymbol *bytes* type. Binary columns are read losslessly as base64
  `String`; binding a value *as* binary waits for a bytes type (§5).
- Async / concurrent access from multiple Zymbol threads (interpreter is
  single-threaded; `Rc` everywhere).
- An engine-specific `last_id` in the core contract — it is vendor-specific and
  is handled by SQL idiom instead (§4.3).

**Validated engines in v0.0.7** (the interface is engine-agnostic; these two are
exercised end-to-end): **SQLite** (via the SQLite ODBC driver) and **PostgreSQL**
(via psqlODBC). MySQL / MS SQL Server / Oracle work through the same code path the
moment their ODBC driver is installed — no Zymbol changes — and are simply not
part of the v0.0.7 test matrix.

---

## 2. Backend decision: ODBC via the `odbc-api` crate

Add `odbc-api = "28"` to the workspace, `zymbol-interpreter`, and `zymbol-vm`.
`odbc-api` is a safe Rust binding over the system **ODBC driver manager**
(`unixODBC` on Linux/macOS, the built-in manager on Windows). It is light and
links the driver manager *dynamically* — no vendor code is compiled in.

**Runtime prerequisites (installed by the user / sysadmin, not by Zymbol):**
- the ODBC driver manager: `unixodbc` (Linux), already present on Windows;
- the ODBC driver for each target engine, e.g.
  `libsqlite3-mod-odbc` / `sqliteodbc` (SQLite), `odbc-postgresql` (PostgreSQL),
  `msodbcsql18` (MS SQL Server), Oracle Instant Client ODBC, etc.

**Build prerequisite (CI + contributors):** `unixodbc-dev` (the `libodbc` headers).

This is the correct, inherent trade for a *language*: you cannot talk to Oracle
without Oracle's client regardless — ODBC just makes that one standard interface
instead of N bespoke drivers baked into `zymbol`.

Rejected alternatives:
- *Bundle SQLite (`rusqlite` + `bundled`)* — turns the language into an app with a
  database welded inside; locks out every other engine. (This was the prior draft;
  superseded by the user's "language, not application" correction.)
- *One native crate per engine behind Cargo features* — every engine becomes a
  heavy compile-time dependency, and the shipped `zymbol` only speaks the engines
  it was built with. ODBC keeps one code path and defers engine choice to runtime.

---

## 3. API surface

**Connection-registry** model (not path-based). Remote engines need to be opened
once — auth, network round-trips, pooling — then referenced by a short **name**.
SQLite uses the same model for uniformity. There is no connection-handle *value*
exposed to Zymbol; the name is the handle (a `String`), and the real connection
lives in an internal registry (§6).

Alias shown as `db`. Spanish i18n re-exports via the three-layer pattern:
`conectar, desconectar, ejecutar, consultar, consultar_una, consultar_valor,
transaccion, iniciar, confirmar, revertir, punto_guardado, liberar, revertir_a,
ejecutar_script, existe_tabla`.

| Function | Arity | Returns | Purpose |
|----------|:-----:|---------|---------|
| `connect(name, conn_str)` | 2 | `Unit \| Error` | Open a connection under `name`. `conn_str` is an ODBC connection string or DSN (§3.1). Sets `PRAGMA`/session defaults where applicable. |
| `disconnect(name)` | 1 | `Unit \| Error` | Close and drop the named connection. |
| `exec(name, sql[, params])` | -1 | `Int \| Error` | DDL / INSERT / UPDATE / DELETE. Returns affected-row count. |
| `query(name, sql[, params])` | -1 | `Array<NamedTuple> \| Error` | SELECT → one `NamedTuple` per row, keyed by column name. `[]` if empty. |
| `query_one(name, sql[, params])` | -1 | `NamedTuple \| Unit \| Error` | First row, or `Unit` if none. |
| `query_value(name, sql[, params])` | -1 | `scalar \| Unit \| Error` | First column of first row (count, MAX(id), `RETURNING id`, …), or `Unit`. |
| `tx(name, statements)` | 2 | `Unit \| Error` | Atomic blind batch. `statements` = `Array` of `(sql, params)` tuples. autocommit off → all → commit; any failure → rollback. |
| `begin(name)` | 1 | `Unit \| Error` | Start a transaction on the named connection (autocommit off). Enables interleaved read-then-write. |
| `commit(name)` | 1 | `Unit \| Error` | Commit and restore autocommit. |
| `rollback(name)` | 1 | `Unit \| Error` | Roll back and restore autocommit. |
| `savepoint(name, sp)` | 2 | `Unit \| Error` | `SAVEPOINT sp` (standard SQL; SQLite + PostgreSQL both support it). |
| `release(name, sp)` | 2 | `Unit \| Error` | `RELEASE sp`. |
| `rollback_to(name, sp)` | 2 | `Unit \| Error` | `ROLLBACK TO sp` — undo to savepoint, transaction stays open. |
| `exec_script(name, sql)` | 2 | `Unit \| Error` | Run a multi-statement DDL script (schema init), inside one transaction (§4.4). |
| `table_exists(name, table)` | 2 | `Bool \| Error` | Uses the ODBC `SQLTables` catalog call — vendor-neutral, no `information_schema` dialect branching. |

Functions with optional trailing `params` register with **arity `-1`** (variadic)
and validate the count internally — the same pattern `net::get` uses for `headers`.

### 3.1 The `params` shape — a Tuple, not an Array

SQL parameters are heterogeneous (`1, "O'Brien", 99.5`), but **Zymbol arrays are
homogeneous** (`[1, "x"]` is a compile error). So `params` is a **Tuple**, the
positional heterogeneous type:

```
db::exec("t", "INSERT INTO socios(cod, nombre, saldo) VALUES(?, ?, ?)", (1, "O'Brien & Co.", 99.5))
db::query_one("t", "SELECT * FROM socios WHERE cod = ?", (1,))   -- 1-tuple: note the trailing comma
db::query_value("t", "SELECT COUNT(*) FROM socios")              -- no params: omit entirely
```

For convenience `params` also accepts a homogeneous `Array` (when every parameter
is the same type) and a bare scalar (single parameter, `… WHERE cod = ?", 1`).
Internally all three normalize to a positional list bound to `?` placeholders.
Type mapping of each bound value is in §5.

### 3.1b Connection strings

`conn_str` is passed to ODBC essentially verbatim, because the *driver name* is
system-specific and only the sysadmin knows it. Two accepted forms:

```
-- a) Full ODBC connection string (driver named explicitly).
--    NOTE: a Zymbol string starts interpolation at `{`, and ODBC wraps the driver
--    name in braces — so escape them as \{ \} (required when the name has a space):
db::conectar("ventas", "Driver=\{SQLite3\};Database=./spre.db;")
db::conectar("erp",    "Driver=\{PostgreSQL Unicode\};Server=10.0.0.5;Port=5432;Database=erp;Uid=app;Pwd=secret;")

-- b) A pre-registered DSN from odbc.ini — no braces, so no escaping (preferred):
db::conectar("erp", "DSN=erp_prod")
```

**Brace gotcha.** `{` opens string interpolation in Zymbol. `\{ … \}` produces a
literal brace. A single-identifier form like `{SQLite3}` happens to survive (an
undefined-identifier interpolation falls back to literal text), but a name with a
space like `{PostgreSQL Unicode}` is a hard lex error — so **always escape**, or
use a DSN. Verified live: the identical Zymbol program ran against SQLite *and*
PostgreSQL by only changing this string.

A friendlier `sqlite://` / `postgres://` URL sugar that *generates* these strings
is intentionally **out of scope**: driver names vary per machine, so guessing them
is fragile. URL sugar, if ever wanted, is a pure-Zymbol helper layered on top and
needs no engine changes — additive, no rework.

### 3.2 Why binding + NamedTuple rows is the headline

```
-- Values are bound, never interpolated → G4 cannot occur, on any engine:
db::ejecutar("erp", "INSERT INTO socios(nombre) VALUES(?)", ("O'Brien & Co.",))

-- Rows come back as NamedTuples → no jq, no JSON round-trip:
filas := db::consultar("erp", "SELECT cod, nombre FROM socios WHERE pais = ?", ("CL",))
@ filas -> fila { >> fila.cod " — " fila.nombre ¶ }
```

---

## 4. Transactions

### 4.1 `tx` — atomic blind batch (SPRE's `ejecutar_atomico`)

```
db::transaccion("spre.db", [
    ("INSERT INTO asientos(num, fecha) VALUES(?, ?)", (num, fecha)),
    ("INSERT INTO gc_imputacion(asiento, centro, monto) VALUES(?, ?, ?)", (num, centro, monto)),
])
-- autocommit off; both inserts; commit. If the second fails, the first is rolled back.
```

### 4.2 Low-level (interleaved read-then-write)

When the program must read mid-transaction and branch on the value:

```
db::iniciar("erp")
saldo := db::consultar_valor("erp", "SELECT saldo FROM cuentas WHERE id = ?", (id,))
? saldo >= monto {
    db::ejecutar("erp", "UPDATE cuentas SET saldo = saldo - ? WHERE id = ?", (monto, id))
    db::confirmar("erp")
} : {
    db::revertir("erp")
}
```

All `db::*` calls on that name between `iniciar` and `confirmar`/`revertir` run on
the same connection, hence the same transaction. A `begin` while a transaction is
already open returns a soft `##DB` error ("transaction already active") rather than
silently nesting — use savepoints to nest.

### 4.3 Savepoints (nested / partial rollback)

Implemented as standard SQL (`SAVEPOINT` / `RELEASE` / `ROLLBACK TO`), which both
SQLite and PostgreSQL support — so it stays vendor-neutral with no driver branching:

```
db::iniciar("spre.db")
db::ejecutar("spre.db", "INSERT INTO asientos ...", [...])
db::punto_guardado("spre.db", "imputacion")
? imputacion_falla {
    db::revertir_a("spre.db", "imputacion")   -- undo only the imputation
} : {
    db::liberar("spre.db", "imputacion")
}
db::confirmar("spre.db")
```

### 4.4 `last_id` is deliberately absent

`last_insert_rowid()` (SQLite) and sequences / `RETURNING` (PostgreSQL) are
vendor-specific; a single `last_id` would leak the engine into the contract. Use
the engine's own idiom via `query_value`:

```
-- PostgreSQL:
id := db::consultar_valor("erp", "INSERT INTO socios(nombre) VALUES(?) RETURNING cod", (n,))
-- SQLite:
db::ejecutar("ventas", "INSERT INTO socios(nombre) VALUES(?)", (n,))
id := db::consultar_valor("ventas", "SELECT last_insert_rowid()", [])
```

### 4.5 `exec_script`

ODBC `SQLExecDirect` runs one statement; scripts are split on `;` (naive split,
intended for trusted DDL files such as SPRE's `04_tablas.sql`) and executed in
order inside one transaction, so a failure leaves no half-built schema. Statements
containing a literal `;` inside a string are not supported by the naive splitter
(acceptable for DDL; documented).

---

## 5. Type mapping

ODBC reports each column's SQL type; Zymbol maps both directions:

| ODBC / SQL type | → Zymbol (read) | Zymbol (bind) → SQL |
|-----------------|-----------------|----------------------|
| `INTEGER`/`BIGINT`/`SMALLINT` | `Int` | `Int` |
| `REAL`/`DOUBLE`/`FLOAT` | `Float` | `Float` |
| `CHAR`/`VARCHAR`/`TEXT` | `String` | `String` |
| `DECIMAL`/`NUMERIC` | `String` (lossless) | `String` |
| `BOOLEAN`/`BIT` | `Int` 0/1 | `Bool` → `0`/`1` |
| `DATE`/`TIME`/`TIMESTAMP` | `String` (ISO 8601) | `String` |
| `NULL` | `Unit` | `Unit` |
| `BINARY`/`BLOB`/`BYTEA` | `String` (**base64**, lossless) | binds as text — see note |

Notes:
- **`DECIMAL`/`NUMERIC` → `String`** to preserve exact precision — critical for an
  ERP's money. The caller parses/rounds per `monedas.decimales` (SPRE
  `docs/00_monedas.md`). Mapping to `Float` would silently lose cents; rejected.
- **`Bool` binds to `0`/`1`** (SPRE encodes booleans as integers; many engines lack
  native BOOLEAN). Reads of 0/1 columns come back as `Int`; compare `== 1`.
- A `NULL` cell → `Unit`. Beware the known `Unit`-in-`Array` TW/VM display gap —
  tests must avoid printing a raw `Unit`-bearing row to keep `vm_compare` green.
- **Binary read is lossless base64** (encoded inside the native fn). Binding a value
  *as* binary is the one deferred case (waits for a Zymbol bytes type) and is
  additive: a future `Value::Bytes` binder arm changes no existing signature.

---

## 6. State management (internal)

A process-wide ODBC `Environment` is created once and leaked to `'static`
(`OnceCell` / `once_cell`), because `odbc_api::Connection<'env>` borrows the
environment and the registry must hold connections for the program's lifetime:

```rust
static ODBC_ENV: OnceCell<odbc_api::Environment> = OnceCell::new();

thread_local! {
    // name -> open connection (borrows the 'static ODBC_ENV)
    static DB_CONNS: RefCell<HashMap<String, odbc_api::Connection<'static>>> =
        RefCell::new(HashMap::new());
}
```

- `connect` inserts; `disconnect` removes; every other call looks up by name and
  returns a soft `##DB` (`unknown connection 'NAME'`) if absent.
- The interpreter is single-threaded (`Rc`), and ODBC connections are `!Sync`, so a
  `thread_local` registry is the correct fit.
- TW and VM each own their registry (separate code paths). Parity is about
  observable results, not shared handles — both open their own connection to the
  same DB and see the same data.
- Connections are dropped (closed) at process exit; a program that `begin`s and
  never commits simply never persists that transaction.

---

## 7. Error model

Consistent with `std/io` / `std/net`:

- **Soft `Value::Error` of kind `DB`** (`##DB(...)`) for anything the driver reports
  at runtime: connection failure, SQL syntax error, constraint/FK violation, unknown
  connection name, type-mismatch on bind. The message includes the ODBC **SQLSTATE**
  (e.g. `[23000] UNIQUE constraint failed`). Catchable with try-catch.
- **Hard `RuntimeError`** for programmer errors: wrong argument *type* (non-String
  name/sql, a `params` that is not a Tuple/Array/scalar, a `statements` element
  that is not a `(String, params)` tuple).

Add one constructor:

```rust
impl ErrorValue {
    pub fn db(message: impl Into<String>) -> Self { Self::new("DB", message) }
}
```

(`error_type` doc comment in `lib.rs` gains `"DB"`.)

---

## 8. Implementation checklist (per IMPL_V007.md §"per-module checklist")

1. **`crates/zymbol-interpreter/src/stdlib/db.rs`** — `register()` + native fns
   (`db_connect`, `db_disconnect`, `db_exec`, `db_query`, `db_query_one`,
   `db_query_value`, `db_tx`, `db_begin`, `db_commit`, `db_rollback`,
   `db_savepoint`, `db_release`, `db_rollback_to`, `db_exec_script`,
   `db_table_exists`) + the `OnceCell` env + `thread_local` registry + bind/extract
   helpers (incl. base64 for binary, SQLSTATE-formatted errors).
2. **`stdlib/mod.rs`** — add `mod db;` and a `"std/db" => …` arm in `build_module`.
3. **`crates/zymbol-bytecode/src/lib.rs`** `builtins` — id constants in the **500
   block** (next free after NET 400–403):
   ```
   DB_CONNECT=500 DB_DISCONNECT=501 DB_EXEC=502 DB_QUERY=503 DB_QUERY_ONE=504
   DB_QUERY_VALUE=505 DB_TX=506 DB_BEGIN=507 DB_COMMIT=508 DB_ROLLBACK=509
   DB_SAVEPOINT=510 DB_RELEASE=511 DB_ROLLBACK_TO=512 DB_EXEC_SCRIPT=513
   DB_TABLE_EXISTS=514
   ```
4. **`crates/zymbol-compiler/src/lib.rs`** `stdlib_builtin_entries` — `(name, id)`
   arms for `std/db`.
5. **`crates/zymbol-vm/src/stdlib_builtins.rs`** — builtin impls + dispatch arms
   (mirror the TW logic exactly; share the bind/extract + registry helpers).
6. **Cargo deps** — `odbc-api = "28"`, `base64 = "0.22"`, `once_cell` in workspace +
   interpreter + vm. CI image needs `unixodbc-dev` + the SQLite ODBC driver.
7. **Tests** in `tests/stdlib/` (see §9).
8. **i18n adapter** `db_es.zy` re-exporting the Spanish names.

Parser · AST · Lexer · CLI: **no changes required** (same as every prior module).

---

## 9. Testing

`.zy` + `.expected` pairs under `interpreter/tests/stdlib/`. Tests run against
**SQLite over ODBC**, which is file-based and deterministic, so they can cover the
happy path (unlike `net`):

- `stdlib_db_basic.zy` — connect, create table, insert a value containing `'`
  (proves G4 is gone), select it back, assert rows; disconnect; delete the DB file.
- `stdlib_db_tx.zy` — atomic batch that commits; a batch that violates a constraint
  and rolls back (assert table unchanged → soft `##DB`); a savepoint partial rollback.
- `stdlib_db_type_err.zy` — wrong arg type → hard error (offline, deterministic),
  matching `stdlib_net_type_err`.

PostgreSQL is verified **manually** (TW == VM) against a local server — it needs a
running daemon, so no PostgreSQL golden test is committed (would be flaky in
`vm_compare`/CI, same rule as live HTTP for `net`).

**CI/build note:** the test host must have `unixodbc` + a SQLite ODBC driver
registered; otherwise `connect` returns a soft `##DB` and the golden tests fail to
set up. Document this in the test script and CI image.

Regenerate + verify both engines:
```bash
bash tests/scripts/expected_compare.sh stdlib --regen
bash tests/scripts/expected_compare.sh stdlib
bash tests/scripts/vm_compare.sh
```

---

## 10. Impact on SPRE

`std/db` collapses three planned SPRE layers into one native, vendor-neutral surface:

| SPRE LIBRERIAS.md plan | With `std/db` |
|------------------------|---------------|
| `bd/sqlite.zy` over BashExec → `sqlite3` | thin wrapper over `db::*` (or dropped) |
| `lib/json.zy` + `jq` to parse rows | gone — `query` returns `NamedTuple`s |
| G4 quote sanitization everywhere | gone — parameter binding |
| `bd/transaccion.zy` temp-`.sql` builder | `db::transaccion(name, [(sql,params),…])` |
| SQLite-only | any ODBC engine — SPRE can grow into PostgreSQL unchanged |

After `std/db` lands, SPRE's `docs/LIBRERIAS.md` Tier 1 should be rewritten to
target `db::*`, drop the `sqlite3`/`jq` prerequisites, and add the ODBC
driver-manager prerequisite instead.

---

## 11. Decisions (resolved)

1. **Architecture** → vendor-neutral **ODBC** via `odbc-api`. Zymbol bundles no
   engine; the OS supplies the driver. (Supersedes the bundled-SQLite draft.)
2. **Engines validated in v0.0.7** → **SQLite + PostgreSQL**. MySQL / MS SQL Server
   / Oracle work on the same path once their ODBC driver is installed; not in the
   v0.0.7 test matrix.
3. **Connection model** → named **connection registry** (`connect(name, conn_str)`),
   required for remote engines; uniform for SQLite too.
4. **`tx` shape** → `Array<(String, params)>` — an Array of `(sql, params)` pairs,
   each `params` a Tuple (§3.1). Keeps G4 closed inside transactions.
5. **Transaction model** → complete: `tx` + `begin`/`commit`/`rollback` +
   `savepoint`/`release`/`rollback_to` (savepoints as standard SQL).
6. **`last_id`** → **omitted** from the core contract (vendor-specific); use
   `query_value` with `RETURNING` / `last_insert_rowid()` per engine.
7. **DECIMAL/NUMERIC** → `String` (lossless, money-safe); **binary** → base64
   `String` on read; binding-as-binary deferred until a Zymbol bytes type (additive).
