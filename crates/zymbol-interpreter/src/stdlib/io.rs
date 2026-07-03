//! std/io — filesystem operations for Zymbol-Lang.
//!
//! Operations on a path. Failures (missing file, permissions, …) are returned as
//! a soft `Value::Error` of kind "IO" so they can be caught with try-catch.
//! A wrong argument type is a programmer error and aborts with a hard `RuntimeError`.
//!
//!   read(path)         -> String | Error
//!   write(path, text)  -> Unit | Error
//!   append(path, text) -> Unit | Error
//!   exists(path)       -> Bool            (never fails)
//!   delete(path)       -> Unit | Error    (file or directory)
//!   list(dir)          -> Array<String> | Error
//!   mkdir(path)        -> Unit | Error    (creates parent directories)
//!
//! For localized names, use the i18n three-layer pattern to re-export under the
//! target language's names (e.g. Spanish: leer, escribir, agregar, existe, …).

use crate::{ErrorValue, FunctionDef, Result, RuntimeError, Value};
use std::collections::HashMap;
use std::rc::Rc;
use zymbol_span::Span;

/// io::read("path") -> String | Error
fn io_read(args: Vec<Value>, span: Span) -> Result<Value> {
    match args.into_iter().next() {
        Some(Value::String(path)) => match std::fs::read_to_string(&path) {
            Ok(content) => Ok(Value::String(content)),
            Err(e) => Ok(Value::Error(ErrorValue::io(e.to_string()))),
        },
        _ => Err(RuntimeError::Generic {
            message: "io::read: expected String path".into(),
            span,
        }),
    }
}

/// io::write("path", "content") -> Unit | Error
fn io_write(args: Vec<Value>, span: Span) -> Result<Value> {
    let mut it = args.into_iter();
    match (it.next(), it.next()) {
        (Some(Value::String(path)), Some(Value::String(content))) => {
            match std::fs::write(&path, content.as_bytes()) {
                Ok(_) => Ok(Value::Unit),
                Err(e) => Ok(Value::Error(ErrorValue::io(e.to_string()))),
            }
        }
        _ => Err(RuntimeError::Generic {
            message: "io::write: expected (String, String)".into(),
            span,
        }),
    }
}

/// io::append("path", "content") -> Unit | Error
fn io_append(args: Vec<Value>, span: Span) -> Result<Value> {
    use std::io::Write;
    let mut it = args.into_iter();
    match (it.next(), it.next()) {
        (Some(Value::String(path)), Some(Value::String(content))) => {
            match std::fs::OpenOptions::new().append(true).create(true).open(&path) {
                Ok(mut f) => match f.write_all(content.as_bytes()) {
                    Ok(_) => Ok(Value::Unit),
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
}

/// io::exists("path") -> Bool
fn io_exists(args: Vec<Value>, span: Span) -> Result<Value> {
    match args.into_iter().next() {
        Some(Value::String(path)) => Ok(Value::Bool(std::path::Path::new(&path).exists())),
        _ => Err(RuntimeError::Generic {
            message: "io::exists: expected String path".into(),
            span,
        }),
    }
}

/// io::delete("path") -> Unit | Error  (file or directory)
fn io_delete(args: Vec<Value>, span: Span) -> Result<Value> {
    match args.into_iter().next() {
        Some(Value::String(path)) => {
            let p = std::path::Path::new(&path);
            let result = if p.is_dir() {
                std::fs::remove_dir_all(p)
            } else {
                std::fs::remove_file(p)
            };
            match result {
                Ok(_) => Ok(Value::Unit),
                Err(e) => Ok(Value::Error(ErrorValue::io(e.to_string()))),
            }
        }
        _ => Err(RuntimeError::Generic {
            message: "io::delete: expected String path".into(),
            span,
        }),
    }
}

/// io::list("dir") -> Array<String> | Error
fn io_list(args: Vec<Value>, span: Span) -> Result<Value> {
    match args.into_iter().next() {
        Some(Value::String(path)) => match std::fs::read_dir(&path) {
            Ok(entries) => {
                let mut names = Vec::new();
                for entry in entries.flatten() {
                    names.push(Value::String(entry.file_name().to_string_lossy().into_owned()));
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
}

/// io::mkdir("path") -> Unit | Error  (creates parent directories)
fn io_mkdir(args: Vec<Value>, span: Span) -> Result<Value> {
    match args.into_iter().next() {
        Some(Value::String(path)) => match std::fs::create_dir_all(&path) {
            Ok(_) => Ok(Value::Unit),
            Err(e) => Ok(Value::Error(ErrorValue::io(e.to_string()))),
        },
        _ => Err(RuntimeError::Generic {
            message: "io::mkdir: expected String path".into(),
            span,
        }),
    }
}

// --- Registry -------------------------------------------------------------

pub(crate) fn register() -> HashMap<String, Rc<FunctionDef>> {
    let mut m: HashMap<String, Rc<FunctionDef>> = HashMap::new();

    macro_rules! native {
        ($name:literal, $arity:expr, $fn:expr) => {
            m.insert($name.into(), Rc::new(FunctionDef::Native {
                name: $name, arity: $arity, func: $fn,
            }));
        };
    }

    native!("read",   1, io_read);
    native!("write",  2, io_write);
    native!("append", 2, io_append);
    native!("exists", 1, io_exists);
    native!("delete", 1, io_delete);
    native!("list",   1, io_list);
    native!("mkdir",  1, io_mkdir);

    m
}
