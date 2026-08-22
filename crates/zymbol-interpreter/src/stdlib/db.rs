//! std/db — vendor-neutral database access for Zymbol-Lang via ODBC.
//!
//! Zymbol compiles **no** database engine. It speaks one standard protocol (ODBC)
//! through the system driver manager (unixODBC); the OS supplies the per-engine
//! driver (SQLite, PostgreSQL, MySQL, MS SQL Server, Oracle, …). The same Zymbol
//! code runs against any of them — only the connection string changes.
//!
//! Failures the driver reports at runtime (connection, SQL syntax, constraint
//! violation, unknown connection name) are returned as a soft `Value::Error` of
//! kind "DB" so they can be caught with try-catch. A wrong argument *type* is a
//! programmer error and aborts with a hard `RuntimeError`.
//!
//!   connect(name, conn_str)            -> Unit | Error
//!   disconnect(name)                   -> Unit | Error
//!   exec(name, sql[, params])          -> Int  | Error    (affected rows)
//!   query(name, sql[, params])         -> Array<NamedTuple> | Error
//!   query_one(name, sql[, params])     -> NamedTuple | Error  (no rows is an Error)
//!   query_value(name, sql[, params])   -> scalar | Unit | Error
//!   tx(name, statements)               -> Unit | Error    (atomic (sql,params) batch)
//!   begin/commit/rollback(name)        -> Unit | Error
//!   savepoint/release/rollback_to(name, sp) -> Unit | Error
//!   exec_script(name, sql)             -> Unit | Error     (multi-statement DDL)
//!   table_exists(name, table)          -> Bool | Error
//!
//! `params` is an Array of bound values (parameter binding — quotes are never
//! interpolated). Rows come back as NamedTuples keyed by column name.
//!
//! For localized names, use the i18n three-layer pattern (Spanish: conectar,
//! ejecutar, consultar, transaccion, …).

use crate::{ErrorValue, FunctionDef, Result, RuntimeError, Value};
use zymbol_common::num;
use base64::Engine as _;
use odbc_api::{
    handles::DataType, parameter::InputParameter, Connection, ConnectionOptions, Cursor,
    Environment, IntoParameter,
};
use once_cell::sync::Lazy;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use zymbol_span::Span;

// --- ODBC environment + connection registry -------------------------------

/// Process-wide ODBC environment, created once. `Environment` is `Sync`; the
/// 'static borrow of this static is what lets connections live in the registry.
static ODBC_ENV: Lazy<std::result::Result<Environment, String>> =
    Lazy::new(|| Environment::new().map_err(|e| e.to_string()));

fn env() -> std::result::Result<&'static Environment, String> {
    match &*ODBC_ENV {
        Ok(e) => Ok(e),
        Err(msg) => Err(msg.clone()),
    }
}

struct ConnEntry {
    conn: Connection<'static>,
    in_tx: bool,
}

thread_local! {
    static DB_CONNS: RefCell<HashMap<String, ConnEntry>> = RefCell::new(HashMap::new());
}

// --- error helpers --------------------------------------------------------

fn db_error(msg: impl Into<String>) -> Value {
    Value::Error(ErrorValue::db(msg))
}

fn odbc_err(e: odbc_api::Error) -> Value {
    db_error(e.to_string())
}

/// The soft error `query_one` gives back when the query ran and matched nothing.
///
/// BUG-ZYB-007: it used to return `Unit`, which is also what a `NULL` column
/// returns, so `$!` answered `#0` for "no such row" exactly as it does for a
/// row that exists. The documented check — `? fila$!` — could never be true,
/// and the branch behind it was dead code that read as live: a program with a
/// perfectly good "no such account" message, translated into four languages,
/// instead died several lines later with `Cannot access member 'moneda' on
/// non-tuple value`, naming a tuple in a line that was written correctly.
///
/// A failure has to be reported where it happens or it is reported somewhere
/// it did not.
fn no_rows() -> Value {
    db_error("query_one matched no rows".to_string())
}

fn type_err(message: impl Into<String>, span: Span) -> RuntimeError {
    RuntimeError::Generic {
        message: message.into(),
        span,
    }
}

/// Run `f` with the named connection's entry, or a soft "unknown connection" error.
fn with_conn<F>(name: &str, f: F) -> Value
where
    F: FnOnce(&mut ConnEntry) -> Value,
{
    DB_CONNS.with(|c| {
        let mut map = c.borrow_mut();
        match map.get_mut(name) {
            Some(entry) => f(entry),
            None => db_error(format!("unknown connection '{}'", name)),
        }
    })
}

// --- argument extraction --------------------------------------------------

fn take_string(v: Option<Value>, what: &str, span: Span) -> Result<String> {
    match v {
        Some(Value::String(s)) => Ok(s),
        _ => Err(type_err(format!("db: expected String {}", what), span)),
    }
}

/// Optional trailing `params` argument → Vec of bound values. Absent → empty.
///
/// SQL parameters are heterogeneous, so the natural Zymbol shape is a **Tuple**
/// `(1, "O'Brien", 9.5)` (Zymbol arrays are homogeneous). A homogeneous `Array`
/// is also accepted, as is a bare scalar for the single-parameter case.
fn take_params(v: Option<Value>, span: Span) -> Result<Vec<Value>> {
    match v {
        None => Ok(Vec::new()),
        Some(Value::Tuple(items)) | Some(Value::Array(items)) => Ok(items),
        Some(
            s @ (Value::Int(_)
            | Value::Float(_)
            | Value::String(_)
            | Value::Bool(_)
            | Value::Char(_)
            | Value::Unit),
        ) => Ok(vec![s]),
        Some(_) => Err(type_err("db: params must be a Tuple, Array, or scalar", span)),
    }
}

/// Convert Zymbol values into ODBC input parameters (heterogeneous, dynamic).
fn bind_params(params: Vec<Value>, span: Span) -> Result<Vec<Box<dyn InputParameter>>> {
    let mut out: Vec<Box<dyn InputParameter>> = Vec::with_capacity(params.len());
    for p in params {
        let boxed: Box<dyn InputParameter> = match p {
            Value::Int(i) => Box::new(i.into_parameter()),
            Value::Float(f) => Box::new(f.into_parameter()),
            // SPRE encodes booleans as integers; bind 0/1.
            Value::Bool(b) => Box::new((if b { 1i64 } else { 0i64 }).into_parameter()),
            Value::String(s) => Box::new(s.into_parameter()),
            Value::Char(c) => Box::new(c.to_string().into_parameter()),
            // NULL of (nominally) text type — drivers coerce as needed.
            Value::Unit => Box::new(Option::<String>::None.into_parameter()),
            other => {
                return Err(type_err(
                    format!("db: cannot bind {:?} as a parameter", other),
                    span,
                ))
            }
        };
        out.push(boxed);
    }
    Ok(out)
}

// --- result extraction ----------------------------------------------------

fn is_binary(dt: &DataType) -> bool {
    matches!(
        dt,
        DataType::Binary { .. } | DataType::Varbinary { .. } | DataType::LongVarbinary { .. }
    )
}

/// Map an ODBC text cell to a Zymbol value using the column's declared type.
fn cell_from_text(text: String, dt: &DataType) -> Value {
    match dt {
        DataType::Integer
        | DataType::SmallInt
        | DataType::BigInt
        | DataType::TinyInt
        // A BIGINT column is 64-bit at the database and wider than a Zymbol
        // integer, so a value past the range stays the String the driver sent —
        // the same fail-safe DECIMAL already gets, and lossless either way. It
        // must not become an Int that only one engine could hold.
        | DataType::Bit => text
            .parse::<i64>()
            .ok()
            .filter(|n| num::in_int_range(*n))
            .map(Value::Int)
            .unwrap_or(Value::String(text)),
        DataType::Real | DataType::Double | DataType::Float { .. } => {
            text.parse::<f64>().map(Value::Float).unwrap_or(Value::String(text))
        }
        // DECIMAL/NUMERIC stay String (lossless, money-safe); dates/text stay String.
        _ => Value::String(text),
    }
}

/// Read every row of a cursor into `Vec<NamedTuple>` (one column read for
/// `query_value` is handled by the caller). Returns a soft DB error Value on
/// failure, wrapped in `Err` so the caller can short-circuit.
#[allow(clippy::result_large_err)] // Err is a soft DB error Value by design; cold path
fn rows_from_cursor(
    cursor: &mut impl Cursor,
    only_first: bool,
    single_col: bool,
) -> std::result::Result<Vec<Value>, Value> {
    let names: Vec<String> = cursor
        .column_names()
        .map_err(odbc_err)?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(odbc_err)?;
    let ncols = names.len() as u16;
    let mut types: Vec<DataType> = Vec::with_capacity(names.len());
    for col in 1..=ncols {
        types.push(cursor.col_data_type(col).map_err(odbc_err)?);
    }

    let mut rows: Vec<Value> = Vec::new();
    let mut buf: Vec<u8> = Vec::new();
    while let Some(mut row) = cursor.next_row().map_err(odbc_err)? {
        let mut fields: Vec<(String, Value)> = Vec::with_capacity(names.len());
        let take_cols = if single_col { 1 } else { ncols };
        for idx in 0..take_cols {
            let col = idx + 1;
            let dt = &types[idx as usize];
            let value = if is_binary(dt) {
                buf.clear();
                let not_null = row.get_binary(col, &mut buf).map_err(odbc_err)?;
                if !not_null {
                    Value::Unit
                } else {
                    Value::String(base64::engine::general_purpose::STANDARD.encode(&buf))
                }
            } else {
                buf.clear();
                let not_null = row.get_text(col, &mut buf).map_err(odbc_err)?;
                if !not_null {
                    Value::Unit
                } else {
                    cell_from_text(String::from_utf8_lossy(&buf).into_owned(), dt)
                }
            };
            fields.push((names[idx as usize].clone(), value));
        }
        rows.push(Value::NamedTuple(fields));
        if only_first {
            break;
        }
    }
    Ok(rows)
}

// --- builtins -------------------------------------------------------------

/// db::connect(name, conn_str) -> Unit | Error
fn db_connect(args: Vec<Value>, span: Span) -> Result<Value> {
    let mut it = args.into_iter();
    let name = take_string(it.next(), "name", span)?;
    let conn_str = take_string(it.next(), "connection string", span)?;
    let env = match env() {
        Ok(e) => e,
        Err(msg) => return Ok(db_error(msg)),
    };
    match env.connect_with_connection_string(&conn_str, ConnectionOptions::default()) {
        Ok(conn) => {
            DB_CONNS.with(|c| {
                c.borrow_mut()
                    .insert(name, ConnEntry { conn, in_tx: false })
            });
            Ok(Value::Unit)
        }
        Err(e) => Ok(odbc_err(e)),
    }
}

/// db::disconnect(name) -> Unit | Error
fn db_disconnect(args: Vec<Value>, span: Span) -> Result<Value> {
    let name = take_string(args.into_iter().next(), "name", span)?;
    DB_CONNS.with(|c| {
        if c.borrow_mut().remove(&name).is_some() {
            Value::Unit
        } else {
            db_error(format!("unknown connection '{}'", name))
        }
    });
    Ok(Value::Unit)
}

/// db::exec(name, sql[, params]) -> Int (affected rows) | Error
fn db_exec(args: Vec<Value>, span: Span) -> Result<Value> {
    let mut it = args.into_iter();
    let name = take_string(it.next(), "name", span)?;
    let sql = take_string(it.next(), "sql", span)?;
    let bound = bind_params(take_params(it.next(), span)?, span)?;
    Ok(with_conn(&name, |entry| {
        let mut stmt = match entry.conn.preallocate() {
            Ok(s) => s,
            Err(e) => return odbc_err(e),
        };
        // Run the statement in an inner scope that yields a borrow-free
        // `Result<(), Error>`, so the cursor (which borrows `stmt`) is dropped
        // before we call row_count() for the affected-row count.
        let outcome: std::result::Result<(), odbc_api::Error> = {
            let res = if bound.is_empty() {
                stmt.execute(&sql, ())
            } else {
                stmt.execute(&sql, bound.as_slice())
            };
            res.map(|_| ())
        };
        if let Err(e) = outcome {
            return odbc_err(e);
        }
        let n = stmt.row_count().ok().flatten().unwrap_or(0);
        Value::Int(n as i64)
    }))
}

/// Shared SELECT path for query / query_one / query_value.
fn run_query(
    name: &str,
    sql: &str,
    bound: Vec<Box<dyn InputParameter>>,
    only_first: bool,
    single_col: bool,
) -> Value {
    with_conn(name, |entry| {
        let res = if bound.is_empty() {
            entry.conn.execute(sql, (), None)
        } else {
            entry.conn.execute(sql, bound.as_slice(), None)
        };
        match res {
            Ok(Some(mut cursor)) => match rows_from_cursor(&mut cursor, only_first, single_col) {
                Ok(rows) => Value::Array(rows),
                Err(e) => e,
            },
            // No result set (e.g. a statement that returns nothing): empty.
            Ok(None) => Value::Array(Vec::new()),
            Err(e) => odbc_err(e),
        }
    })
}

/// db::query(name, sql[, params]) -> Array<NamedTuple> | Error
fn db_query(args: Vec<Value>, span: Span) -> Result<Value> {
    let mut it = args.into_iter();
    let name = take_string(it.next(), "name", span)?;
    let sql = take_string(it.next(), "sql", span)?;
    let bound = bind_params(take_params(it.next(), span)?, span)?;
    Ok(run_query(&name, &sql, bound, false, false))
}

/// db::query_one(name, sql[, params]) -> NamedTuple | Error — no rows is a soft error
fn db_query_one(args: Vec<Value>, span: Span) -> Result<Value> {
    let mut it = args.into_iter();
    let name = take_string(it.next(), "name", span)?;
    let sql = take_string(it.next(), "sql", span)?;
    let bound = bind_params(take_params(it.next(), span)?, span)?;
    Ok(match run_query(&name, &sql, bound, true, false) {
        Value::Array(mut rows) => rows.drain(..).next().unwrap_or_else(no_rows),
        other => other, // soft error passes through
    })
}

/// db::query_value(name, sql[, params]) -> scalar | Unit | Error
fn db_query_value(args: Vec<Value>, span: Span) -> Result<Value> {
    let mut it = args.into_iter();
    let name = take_string(it.next(), "name", span)?;
    let sql = take_string(it.next(), "sql", span)?;
    let bound = bind_params(take_params(it.next(), span)?, span)?;
    Ok(match run_query(&name, &sql, bound, true, true) {
        Value::Array(mut rows) => match rows.drain(..).next() {
            Some(Value::NamedTuple(mut fields)) => {
                fields.drain(..).next().map(|(_, v)| v).unwrap_or(Value::Unit)
            }
            _ => Value::Unit,
        },
        other => other,
    })
}

/// db::tx(name, statements) -> Unit | Error  — atomic batch of (sql, params) tuples.
fn db_tx(args: Vec<Value>, span: Span) -> Result<Value> {
    let mut it = args.into_iter();
    let name = take_string(it.next(), "name", span)?;
    let statements = match it.next() {
        Some(Value::Array(items)) => items,
        _ => return Err(type_err("db::tx: statements must be an Array", span)),
    };

    // Validate + pre-bind every statement before opening the transaction.
    let mut prepared: Vec<(String, Vec<Box<dyn InputParameter>>)> = Vec::with_capacity(statements.len());
    for st in statements {
        match st {
            Value::Tuple(pair) if pair.len() == 2 => {
                let mut p = pair.into_iter();
                let sql = match p.next() {
                    Some(Value::String(s)) => s,
                    _ => return Err(type_err("db::tx: each statement is (String sql, Array params)", span)),
                };
                let params = take_params(p.next(), span)?;
                prepared.push((sql, bind_params(params, span)?));
            }
            _ => return Err(type_err("db::tx: each statement must be a (sql, params) tuple", span)),
        }
    }

    Ok(with_conn(&name, |entry| {
        if let Err(e) = entry.conn.set_autocommit(false) {
            return odbc_err(e);
        }
        for (sql, bound) in &prepared {
            let res = if bound.is_empty() {
                entry.conn.execute(sql, (), None)
            } else {
                entry.conn.execute(sql, bound.as_slice(), None)
            };
            if let Err(e) = res {
                let _ = entry.conn.rollback();
                let _ = entry.conn.set_autocommit(true);
                return odbc_err(e);
            }
        }
        let result = match entry.conn.commit() {
            Ok(_) => Value::Unit,
            Err(e) => {
                let _ = entry.conn.rollback();
                odbc_err(e)
            }
        };
        let _ = entry.conn.set_autocommit(true);
        result
    }))
}

/// db::begin(name) -> Unit | Error
fn db_begin(args: Vec<Value>, span: Span) -> Result<Value> {
    let name = take_string(args.into_iter().next(), "name", span)?;
    Ok(with_conn(&name, |entry| {
        if entry.in_tx {
            return db_error("transaction already active (use savepoints to nest)");
        }
        match entry.conn.set_autocommit(false) {
            Ok(_) => {
                entry.in_tx = true;
                Value::Unit
            }
            Err(e) => odbc_err(e),
        }
    }))
}

/// db::commit(name) -> Unit | Error
fn db_commit(args: Vec<Value>, span: Span) -> Result<Value> {
    let name = take_string(args.into_iter().next(), "name", span)?;
    Ok(with_conn(&name, |entry| {
        let result = match entry.conn.commit() {
            Ok(_) => Value::Unit,
            Err(e) => odbc_err(e),
        };
        let _ = entry.conn.set_autocommit(true);
        entry.in_tx = false;
        result
    }))
}

/// db::rollback(name) -> Unit | Error
fn db_rollback(args: Vec<Value>, span: Span) -> Result<Value> {
    let name = take_string(args.into_iter().next(), "name", span)?;
    Ok(with_conn(&name, |entry| {
        let result = match entry.conn.rollback() {
            Ok(_) => Value::Unit,
            Err(e) => odbc_err(e),
        };
        let _ = entry.conn.set_autocommit(true);
        entry.in_tx = false;
        result
    }))
}

/// A savepoint identifier must be a bare SQL identifier (no injection).
fn valid_savepoint(sp: &str) -> bool {
    !sp.is_empty()
        && sp.chars().next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && sp.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn savepoint_op(args: Vec<Value>, span: Span, verb: &str) -> Result<Value> {
    let mut it = args.into_iter();
    let name = take_string(it.next(), "name", span)?;
    let sp = take_string(it.next(), "savepoint name", span)?;
    if !valid_savepoint(&sp) {
        return Ok(db_error(format!("invalid savepoint name '{}'", sp)));
    }
    let sql = format!("{} {}", verb, sp);
    Ok(with_conn(&name, |entry| match entry.conn.execute(&sql, (), None) {
        Ok(_) => Value::Unit,
        Err(e) => odbc_err(e),
    }))
}

fn db_savepoint(args: Vec<Value>, span: Span) -> Result<Value> {
    savepoint_op(args, span, "SAVEPOINT")
}
fn db_release(args: Vec<Value>, span: Span) -> Result<Value> {
    savepoint_op(args, span, "RELEASE")
}
fn db_rollback_to(args: Vec<Value>, span: Span) -> Result<Value> {
    savepoint_op(args, span, "ROLLBACK TO")
}

/// db::exec_script(name, sql) -> Unit | Error  — naive `;`-split DDL, one transaction.
fn db_exec_script(args: Vec<Value>, span: Span) -> Result<Value> {
    let mut it = args.into_iter();
    let name = take_string(it.next(), "name", span)?;
    let script = take_string(it.next(), "sql", span)?;
    let statements: Vec<String> = script
        .split(';')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    Ok(with_conn(&name, |entry| {
        if let Err(e) = entry.conn.set_autocommit(false) {
            return odbc_err(e);
        }
        for stmt in &statements {
            if let Err(e) = entry.conn.execute(stmt, (), None) {
                let _ = entry.conn.rollback();
                let _ = entry.conn.set_autocommit(true);
                return odbc_err(e);
            }
        }
        let result = match entry.conn.commit() {
            Ok(_) => Value::Unit,
            Err(e) => odbc_err(e),
        };
        let _ = entry.conn.set_autocommit(true);
        result
    }))
}

/// db::table_exists(name, table) -> Bool | Error  — via the ODBC SQLTables catalog.
fn db_table_exists(args: Vec<Value>, span: Span) -> Result<Value> {
    let mut it = args.into_iter();
    let name = take_string(it.next(), "name", span)?;
    let table = take_string(it.next(), "table", span)?;
    Ok(with_conn(&name, |entry| {
        match entry.conn.tables("", "", &table, "") {
            Ok(mut iter) => match iter.next() {
                Some(Ok(_)) => Value::Bool(true),
                Some(Err(e)) => odbc_err(e),
                None => Value::Bool(false),
            },
            Err(e) => odbc_err(e),
        }
    }))
}

// --- Registry -------------------------------------------------------------

pub(crate) fn register() -> HashMap<String, Rc<FunctionDef>> {
    let mut m: HashMap<String, Rc<FunctionDef>> = HashMap::new();

    macro_rules! native {
        ($name:literal, $arity:expr, $fn:expr) => {
            m.insert(
                $name.into(),
                Rc::new(FunctionDef::Native {
                    name: $name,
                    arity: $arity,
                    func: $fn,
                }),
            );
        };
    }

    native!("connect", 2, db_connect);
    native!("disconnect", 1, db_disconnect);
    // arity -1 (variadic): optional trailing `params` arg.
    native!("exec", -1, db_exec);
    native!("query", -1, db_query);
    native!("query_one", -1, db_query_one);
    native!("query_value", -1, db_query_value);
    native!("tx", 2, db_tx);
    native!("begin", 1, db_begin);
    native!("commit", 1, db_commit);
    native!("rollback", 1, db_rollback);
    native!("savepoint", 2, db_savepoint);
    native!("release", 2, db_release);
    native!("rollback_to", 2, db_rollback_to);
    native!("exec_script", 2, db_exec_script);
    native!("table_exists", 2, db_table_exists);

    m
}
