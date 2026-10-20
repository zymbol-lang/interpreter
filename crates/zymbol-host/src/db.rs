//! std/db for the register VM, through ODBC (the `db` feature).
//!
//! Mirrors zymbol-interpreter/src/stdlib/db.rs. Vendor-neutral database access via
//! ODBC. Runtime/SQL failures are soft `Value::Error("##DB(...)")` (catchable); a
//! wrong argument type is a hard `Err`. This crate keeps its own ODBC environment
//! and connection registry (separate from the tree-walker's); a program runs under
//! one engine at a time, so only one registry is ever used.
//!
//! Moved here from zymbol-vm's stdlib_builtins.rs unchanged (3.1): the VM reaches
//! it through `Host::db`.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use base64::Engine as _;
use odbc_api::{
    handles::DataType, parameter::InputParameter, Connection, ConnectionOptions, Cursor,
    Environment, IntoParameter,
};
use once_cell::sync::Lazy;
use zymbol_bytecode::builtins as B;
use zymbol_common::num;
use zymbol_vm::{Value, ZyStr};

static DB_ODBC_ENV: Lazy<Result<Environment, String>> =
    Lazy::new(|| Environment::new().map_err(|e| e.to_string()));

fn db_env() -> Result<&'static Environment, String> {
    match &*DB_ODBC_ENV {
        Ok(e) => Ok(e),
        Err(msg) => Err(msg.clone()),
    }
}

struct DbConnEntry {
    conn: Connection<'static>,
    in_tx: bool,
}

thread_local! {
    static VM_DB_CONNS: RefCell<HashMap<String, DbConnEntry>> = RefCell::new(HashMap::new());
}

fn db_err(msg: impl Into<String>) -> Value {
    Value::Error(ZyStr::new(format!("##DB({})", msg.into())))
}

fn db_odbc_err(e: odbc_api::Error) -> Value {
    db_err(e.to_string())
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
fn db_no_rows() -> Value {
    db_err("query_one matched no rows".to_string())
}

fn db_with_conn<F>(name: &str, f: F) -> Value
where
    F: FnOnce(&mut DbConnEntry) -> Value,
{
    VM_DB_CONNS.with(|c| {
        let mut map = c.borrow_mut();
        match map.get_mut(name) {
            Some(entry) => f(entry),
            None => db_err(format!("unknown connection '{}'", name)),
        }
    })
}

fn db_take_string(v: Option<Value>, what: &str) -> Result<String, String> {
    match v {
        Some(Value::String(s)) => Ok(s.as_str().to_string()),
        _ => Err(format!("db: expected String {}", what)),
    }
}

fn db_take_params(v: Option<Value>) -> Result<Vec<Value>, String> {
    match v {
        None => Ok(Vec::new()),
        Some(Value::Tuple(items)) | Some(Value::Array(items)) => Ok(items.as_ref().clone()),
        Some(
            s @ (Value::Int(_)
            | Value::Float(_)
            | Value::String(_)
            | Value::Bool(_)
            | Value::Char(_)
            | Value::Unit),
        ) => Ok(vec![s]),
        Some(_) => Err("db: params must be a Tuple, Array, or scalar".into()),
    }
}

fn db_bind_params(params: Vec<Value>) -> Result<Vec<Box<dyn InputParameter>>, String> {
    let mut out: Vec<Box<dyn InputParameter>> = Vec::with_capacity(params.len());
    for p in params {
        let boxed: Box<dyn InputParameter> = match p {
            Value::Int(i) => Box::new(i.into_parameter()),
            Value::Float(f) => Box::new(f.into_parameter()),
            Value::Bool(b) => Box::new((if b { 1i64 } else { 0i64 }).into_parameter()),
            Value::String(s) => Box::new(s.as_str().to_string().into_parameter()),
            Value::Char(c) => Box::new(c.to_string().into_parameter()),
            Value::Unit => Box::new(Option::<String>::None.into_parameter()),
            other => return Err(format!("db: cannot bind {} as a parameter", other.type_label())),
        };
        out.push(boxed);
    }
    Ok(out)
}

fn db_is_binary(dt: &DataType) -> bool {
    matches!(
        dt,
        DataType::Binary { .. } | DataType::Varbinary { .. } | DataType::LongVarbinary { .. }
    )
}

fn db_cell_from_text(text: String, dt: &DataType) -> Value {
    match dt {
        DataType::Integer
        | DataType::SmallInt
        | DataType::BigInt
        | DataType::TinyInt
        // A BIGINT column is wider than a Zymbol integer; past the range the
        // value stays the String the driver sent (as the tree-walker does).
        | DataType::Bit => match num::parse(text.trim()) {
            num::Num::Int(n) => Value::Int(n),
            _ => Value::String(ZyStr::new(text)),
        },
        DataType::Real | DataType::Double | DataType::Float { .. } => text
            .parse::<f64>()
            .map(Value::Float)
            .unwrap_or_else(|_| Value::String(ZyStr::new(text))),
        _ => Value::String(ZyStr::new(text)),
    }
}

fn db_rows_from_cursor(
    cursor: &mut impl Cursor,
    only_first: bool,
    single_col: bool,
) -> Result<Vec<Value>, Value> {
    let names: Vec<String> = cursor
        .column_names()
        .map_err(db_odbc_err)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(db_odbc_err)?;
    let ncols = names.len() as u16;
    let mut types: Vec<DataType> = Vec::with_capacity(names.len());
    for col in 1..=ncols {
        types.push(cursor.col_data_type(col).map_err(db_odbc_err)?);
    }

    let mut rows: Vec<Value> = Vec::new();
    let mut buf: Vec<u8> = Vec::new();
    while let Some(mut row) = cursor.next_row().map_err(db_odbc_err)? {
        let mut fields: zymbol_common::Dict<Value> = zymbol_common::Dict::with_capacity_and_hasher(names.len(), Default::default());
        let take_cols = if single_col { 1 } else { ncols };
        for idx in 0..take_cols {
            let col = idx + 1;
            let dt = &types[idx as usize];
            let value = if db_is_binary(dt) {
                buf.clear();
                let not_null = row.get_binary(col, &mut buf).map_err(db_odbc_err)?;
                if !not_null {
                    Value::Unit
                } else {
                    Value::String(ZyStr::new(
                        base64::engine::general_purpose::STANDARD.encode(&buf),
                    ))
                }
            } else {
                buf.clear();
                let not_null = row.get_text(col, &mut buf).map_err(db_odbc_err)?;
                if !not_null {
                    Value::Unit
                } else {
                    db_cell_from_text(String::from_utf8_lossy(&buf).into_owned(), dt)
                }
            };
            fields.insert(names[idx as usize].clone(), value);
        }
        rows.push(Value::NamedTuple(Rc::new(fields)));
        if only_first {
            break;
        }
    }
    Ok(rows)
}

fn db_connect(args: Vec<Value>) -> Result<Value, String> {
    let mut it = args.into_iter();
    let name = db_take_string(it.next(), "name")?;
    let conn_str = db_take_string(it.next(), "connection string")?;
    let env = match db_env() {
        Ok(e) => e,
        Err(msg) => return Ok(db_err(msg)),
    };
    match env.connect_with_connection_string(&conn_str, ConnectionOptions::default()) {
        Ok(conn) => {
            VM_DB_CONNS.with(|c| {
                c.borrow_mut()
                    .insert(name, DbConnEntry { conn, in_tx: false })
            });
            Ok(Value::Unit)
        }
        Err(e) => Ok(db_odbc_err(e)),
    }
}

fn db_disconnect(args: Vec<Value>) -> Result<Value, String> {
    let name = db_take_string(args.into_iter().next(), "name")?;
    VM_DB_CONNS.with(|c| {
        c.borrow_mut().remove(&name);
    });
    Ok(Value::Unit)
}

fn db_exec(args: Vec<Value>) -> Result<Value, String> {
    let mut it = args.into_iter();
    let name = db_take_string(it.next(), "name")?;
    let sql = db_take_string(it.next(), "sql")?;
    let bound = db_bind_params(db_take_params(it.next())?)?;
    Ok(db_with_conn(&name, |entry| {
        let mut stmt = match entry.conn.preallocate() {
            Ok(s) => s,
            Err(e) => return db_odbc_err(e),
        };
        let outcome: Result<(), odbc_api::Error> = {
            let res = if bound.is_empty() {
                stmt.execute(&sql, ())
            } else {
                stmt.execute(&sql, bound.as_slice())
            };
            res.map(|_| ())
        };
        if let Err(e) = outcome {
            return db_odbc_err(e);
        }
        let n = stmt.row_count().ok().flatten().unwrap_or(0);
        Value::Int(n as i64)
    }))
}

fn db_run_query(
    name: &str,
    sql: &str,
    bound: Vec<Box<dyn InputParameter>>,
    only_first: bool,
    single_col: bool,
) -> Value {
    db_with_conn(name, |entry| {
        let res = if bound.is_empty() {
            entry.conn.execute(sql, (), None)
        } else {
            entry.conn.execute(sql, bound.as_slice(), None)
        };
        match res {
            Ok(Some(mut cursor)) => match db_rows_from_cursor(&mut cursor, only_first, single_col) {
                Ok(rows) => Value::Array(Rc::new(rows)),
                Err(e) => e,
            },
            Ok(None) => Value::Array(Rc::new(Vec::new())),
            Err(e) => db_odbc_err(e),
        }
    })
}

fn db_query(args: Vec<Value>) -> Result<Value, String> {
    let mut it = args.into_iter();
    let name = db_take_string(it.next(), "name")?;
    let sql = db_take_string(it.next(), "sql")?;
    let bound = db_bind_params(db_take_params(it.next())?)?;
    Ok(db_run_query(&name, &sql, bound, false, false))
}

fn db_query_one(args: Vec<Value>) -> Result<Value, String> {
    let mut it = args.into_iter();
    let name = db_take_string(it.next(), "name")?;
    let sql = db_take_string(it.next(), "sql")?;
    let bound = db_bind_params(db_take_params(it.next())?)?;
    Ok(match db_run_query(&name, &sql, bound, true, false) {
        Value::Array(rows) => rows.first().cloned().unwrap_or_else(db_no_rows),
        other => other,
    })
}

fn db_query_value(args: Vec<Value>) -> Result<Value, String> {
    let mut it = args.into_iter();
    let name = db_take_string(it.next(), "name")?;
    let sql = db_take_string(it.next(), "sql")?;
    let bound = db_bind_params(db_take_params(it.next())?)?;
    Ok(match db_run_query(&name, &sql, bound, true, true) {
        Value::Array(rows) => match rows.first() {
            Some(Value::NamedTuple(fields)) => {
                fields.first().map(|(_, v)| v.clone()).unwrap_or(Value::Unit)
            }
            _ => Value::Unit,
        },
        other => other,
    })
}

fn db_tx(args: Vec<Value>) -> Result<Value, String> {
    let mut it = args.into_iter();
    let name = db_take_string(it.next(), "name")?;
    let statements = match it.next() {
        Some(Value::Array(items)) => items.as_ref().clone(),
        _ => return Err("db::tx: statements must be an Array".into()),
    };

    let mut prepared: Vec<(String, Vec<Box<dyn InputParameter>>)> =
        Vec::with_capacity(statements.len());
    for st in statements {
        match st {
            Value::Tuple(pair) if pair.len() == 2 => {
                let sql = match &pair[0] {
                    Value::String(s) => s.as_str().to_string(),
                    _ => return Err("db::tx: each statement is (String sql, Array params)".into()),
                };
                let params = db_take_params(Some(pair[1].clone()))?;
                prepared.push((sql, db_bind_params(params)?));
            }
            _ => return Err("db::tx: each statement must be a (sql, params) tuple".into()),
        }
    }

    Ok(db_with_conn(&name, |entry| {
        if let Err(e) = entry.conn.set_autocommit(false) {
            return db_odbc_err(e);
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
                return db_odbc_err(e);
            }
        }
        let result = match entry.conn.commit() {
            Ok(_) => Value::Unit,
            Err(e) => {
                let _ = entry.conn.rollback();
                db_odbc_err(e)
            }
        };
        let _ = entry.conn.set_autocommit(true);
        result
    }))
}

fn db_begin(args: Vec<Value>) -> Result<Value, String> {
    let name = db_take_string(args.into_iter().next(), "name")?;
    Ok(db_with_conn(&name, |entry| {
        if entry.in_tx {
            return db_err("transaction already active (use savepoints to nest)");
        }
        match entry.conn.set_autocommit(false) {
            Ok(_) => {
                entry.in_tx = true;
                Value::Unit
            }
            Err(e) => db_odbc_err(e),
        }
    }))
}

fn db_commit(args: Vec<Value>) -> Result<Value, String> {
    let name = db_take_string(args.into_iter().next(), "name")?;
    Ok(db_with_conn(&name, |entry| {
        let result = match entry.conn.commit() {
            Ok(_) => Value::Unit,
            Err(e) => db_odbc_err(e),
        };
        let _ = entry.conn.set_autocommit(true);
        entry.in_tx = false;
        result
    }))
}

fn db_rollback(args: Vec<Value>) -> Result<Value, String> {
    let name = db_take_string(args.into_iter().next(), "name")?;
    Ok(db_with_conn(&name, |entry| {
        let result = match entry.conn.rollback() {
            Ok(_) => Value::Unit,
            Err(e) => db_odbc_err(e),
        };
        let _ = entry.conn.set_autocommit(true);
        entry.in_tx = false;
        result
    }))
}

fn db_valid_savepoint(sp: &str) -> bool {
    !sp.is_empty()
        && sp.chars().next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && sp.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn db_savepoint_op(args: Vec<Value>, verb: &str) -> Result<Value, String> {
    let mut it = args.into_iter();
    let name = db_take_string(it.next(), "name")?;
    let sp = db_take_string(it.next(), "savepoint name")?;
    if !db_valid_savepoint(&sp) {
        return Ok(db_err(format!("invalid savepoint name '{}'", sp)));
    }
    let sql = format!("{} {}", verb, sp);
    Ok(db_with_conn(&name, |entry| {
        match entry.conn.execute(&sql, (), None) {
            Ok(_) => Value::Unit,
            Err(e) => db_odbc_err(e),
        }
    }))
}

fn db_exec_script(args: Vec<Value>) -> Result<Value, String> {
    let mut it = args.into_iter();
    let name = db_take_string(it.next(), "name")?;
    let script = db_take_string(it.next(), "sql")?;
    let statements: Vec<String> = script
        .split(';')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    Ok(db_with_conn(&name, |entry| {
        if let Err(e) = entry.conn.set_autocommit(false) {
            return db_odbc_err(e);
        }
        for stmt in &statements {
            if let Err(e) = entry.conn.execute(stmt, (), None) {
                let _ = entry.conn.rollback();
                let _ = entry.conn.set_autocommit(true);
                return db_odbc_err(e);
            }
        }
        let result = match entry.conn.commit() {
            Ok(_) => Value::Unit,
            Err(e) => db_odbc_err(e),
        };
        let _ = entry.conn.set_autocommit(true);
        result
    }))
}

fn db_table_exists(args: Vec<Value>) -> Result<Value, String> {
    let mut it = args.into_iter();
    let name = db_take_string(it.next(), "name")?;
    let table = db_take_string(it.next(), "table")?;
    Ok(db_with_conn(&name, |entry| {
        match entry.conn.tables("", "", &table, "") {
            Ok(mut iter) => match iter.next() {
                Some(Ok(_)) => Value::Bool(true),
                Some(Err(e)) => db_odbc_err(e),
                None => Value::Bool(false),
            },
            Err(e) => db_odbc_err(e),
        }
    }))
}


pub(crate) fn db_dispatch(builtin_id: u16, args: Vec<Value>) -> Result<Value, String> {
    match builtin_id {
        B::DB_CONNECT      => db_connect(args),
        B::DB_DISCONNECT   => db_disconnect(args),
        B::DB_EXEC         => db_exec(args),
        B::DB_QUERY        => db_query(args),
        B::DB_QUERY_ONE    => db_query_one(args),
        B::DB_QUERY_VALUE  => db_query_value(args),
        B::DB_TX           => db_tx(args),
        B::DB_BEGIN        => db_begin(args),
        B::DB_COMMIT       => db_commit(args),
        B::DB_ROLLBACK     => db_rollback(args),
        B::DB_SAVEPOINT    => db_savepoint_op(args, "SAVEPOINT"),
        B::DB_RELEASE      => db_savepoint_op(args, "RELEASE"),
        B::DB_ROLLBACK_TO  => db_savepoint_op(args, "ROLLBACK TO"),
        B::DB_EXEC_SCRIPT  => db_exec_script(args),
        B::DB_TABLE_EXISTS => db_table_exists(args),
        other => Err(format!("unknown db builtin id {}", other)),
    }
}
