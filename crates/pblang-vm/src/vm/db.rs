//! SQLite builtin implementations.
//!
//! Five free functions backing `sqlite_open` / `sqlite_close` / `sqlite_exec`
//! / `sqlite_query` / `sqlite_last_insert_rowid`. Each one:
//!
//! 1. Checks the `sqlite` capability gate (`StdlibConfig.sqlite`) and
//!    errors with `sqlite access denied` if denied.
//! 2. Validates arity (number of args from the script).
//! 3. Coerces the handle / SQL / params using the same value-conversion
//!    rules across all four mutating builtins:
//!    - `Number` → INTEGER (if integral, in i64 range) or REAL
//!    - `Bool`   → INTEGER 0 / 1
//!    - `Obj::String` → TEXT
//!    - `Array<Number 0..=255>` → BLOB
//!    - `Nil`    → NULL
//! 4. Borrows the SQLite connection out of the global handle table, does
//!    the work, and re-inserts it — so two pblang threads can't hold the
//!    same `Connection` concurrently (rusqlite's `Connection` is `!Sync`).
//
// Each function is registered by name in `VM::builtins()` in vm.rs via
// `BuiltinFn { name: "sqlite_open", arity: Some(1), fun: db::sqlite_open }`.

use std::collections::HashMap;
use std::sync::atomic::Ordering;

use rusqlite::Connection;

use pblang_ir::{alloc_string, Obj, RuntimeError, Value, VmHost};

use super::handles::{value_to_handle, with_db_table, DB_NEXT};
use super::{alloc_array, alloc_map, DbResource};

/// Convert a pblang `Value` into a rusqlite bound-parameter value.
/// Shared by `sqlite_exec` and `sqlite_query` so the two stay in sync.
fn pblang_value_to_sqlite_param(
    v: Value,
    host: &mut dyn VmHost,
    what: &str,
) -> Result<rusqlite::types::Value, RuntimeError> {
    match v {
        Value::Nil => Ok(rusqlite::types::Value::Null),
        Value::Bool(b) => Ok(rusqlite::types::Value::Integer(if b { 1 } else { 0 })),
        Value::Number(n) => {
            if n.is_finite()
                && n.fract() == 0.0
                && n >= (i64::MIN as f64)
                && n <= (i64::MAX as f64)
            {
                Ok(rusqlite::types::Value::Integer(n as i64))
            } else {
                Ok(rusqlite::types::Value::Real(n))
            }
        }
        Value::Obj(o) => match o.as_ref() {
            Obj::String(s) => Ok(rusqlite::types::Value::Text(s.clone())),
        },
        Value::Array(a) => {
            // array<number 0..255> → BLOB
            let mut bytes: Vec<u8> = Vec::with_capacity(a.borrow().len());
            for it in a.borrow().iter() {
                match it {
                    Value::Number(n)
                        if n.is_finite() && n.fract() == 0.0 && *n >= 0.0 && *n <= 255.0 =>
                    {
                        bytes.push(*n as u8);
                    }
                    _ => {
                        return Err(host.err(format!(
                            "{what}: blob params must be array of byte numbers 0..255"
                        )))
                    }
                }
            }
            Ok(rusqlite::types::Value::Blob(bytes))
        }
        _ => Err(host.err(format!(
            "{what}: params support number/bool/string/nil/blob-array only"
        ))),
    }
}

fn collect_sqlite_params(
    args2: &Value,
    host: &mut dyn VmHost,
    what: &str,
) -> Result<Vec<rusqlite::types::Value>, RuntimeError> {
    let params_v: Vec<Value> = match args2 {
        Value::Nil => vec![],
        Value::Array(a) => a.borrow().clone(),
        _ => return Err(host.err(format!("{what}: params must be array or nil"))),
    };
    let mut tosql = Vec::with_capacity(params_v.len());
    for v in params_v {
        tosql.push(pblang_value_to_sqlite_param(v, host, what)?);
    }
    Ok(tosql)
}

pub(super) fn sqlite_open(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if !host.allow_sqlite() {
        return Err(host.err("sqlite access denied".into()));
    }
    if args.len() != 1 {
        return Err(host.err(format!(
            "sqlite_open() expects 1 argument (path), got {}",
            args.len()
        )));
    }
    let path = args[0]
        .as_str()
        .map_err(|m| host.err(format!("sqlite_open(): {m}")))?;
    let conn =
        Connection::open(path).map_err(|e| host.err(format!("sqlite_open(): {e}")))?;
    let id = with_db_table(host, |t| {
        let id = DB_NEXT.fetch_add(1, Ordering::Relaxed);
        t.insert(id, DbResource::Sqlite(conn));
        id
    })?;
    Ok(Value::Number(id as f64))
}

pub(super) fn sqlite_close(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if !host.allow_sqlite() {
        return Err(host.err("sqlite access denied".into()));
    }
    if args.len() != 1 {
        return Err(host.err(format!(
            "sqlite_close() expects 1 argument (handle), got {}",
            args.len()
        )));
    }
    let id = value_to_handle(host, args[0].clone(), "sqlite_close")?;
    let _ = with_db_table(host, |t| t.remove(&id));
    Ok(Value::Nil)
}

pub(super) fn sqlite_exec(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if !host.allow_sqlite() {
        return Err(host.err("sqlite access denied".into()));
    }
    // sqlite_exec(handle, sql, params_array_or_nil) -> rows_changed
    if args.len() != 3 {
        return Err(host.err(format!(
            "sqlite_exec() expects 3 arguments (handle, sql, params), got {}",
            args.len()
        )));
    }
    let id = value_to_handle(host, args[0].clone(), "sqlite_exec")?;
    let sql = args[1]
        .as_str()
        .map_err(|m| host.err(format!("sqlite_exec(): {m}")))?;
    let tosql = collect_sqlite_params(&args[2], host, "sqlite_exec")?;

    let changed = match with_db_table(host, |t| t.remove(&id))? {
        Some(DbResource::Sqlite(conn)) => {
            let out = if tosql.is_empty() {
                // execute_batch lets multi-statement SQL (e.g. migration scripts
                // like "CREATE TABLE t(...); INSERT INTO t VALUES (1);") run
                // fully. It does not return a row count, so we report 0.
                conn.execute_batch(sql)
                    .map_err(|e| host.err(format!("sqlite_exec(): {e}")))?;
                0usize
            } else {
                conn.execute(sql, rusqlite::params_from_iter(tosql.iter()))
                    .map_err(|e| host.err(format!("sqlite_exec(): {e}")))?
            };
            let _ = with_db_table(host, |t| t.insert(id, DbResource::Sqlite(conn)));
            out
        }
        None => return Err(host.err("sqlite_exec(): invalid handle".into())),
    };
    Ok(Value::Number(changed as f64))
}

pub(super) fn sqlite_query(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if !host.allow_sqlite() {
        return Err(host.err("sqlite access denied".into()));
    }
    // sqlite_query(handle, sql, params_array_or_nil) -> array<map{col: value}>
    if args.len() != 3 {
        return Err(host.err(format!(
            "sqlite_query() expects 3 arguments (handle, sql, params), got {}",
            args.len()
        )));
    }
    let id = value_to_handle(host, args[0].clone(), "sqlite_query")?;
    let sql = args[1]
        .as_str()
        .map_err(|m| host.err(format!("sqlite_query(): {m}")))?;
    let tosql = collect_sqlite_params(&args[2], host, "sqlite_query")?;

    let rows = match with_db_table(host, |t| t.remove(&id))? {
        Some(DbResource::Sqlite(conn)) => {
            let out_rows = {
                let mut stmt = conn
                    .prepare(sql)
                    .map_err(|e| host.err(format!("sqlite_query(): {e}")))?;
                let col_count = stmt.column_count();
                let col_names = (0..col_count)
                    .map(|i| stmt.column_name(i).unwrap_or("").to_string())
                    .collect::<Vec<_>>();

                let mut out_rows: Vec<Value> = vec![];
                let mut rows = stmt
                    .query(rusqlite::params_from_iter(tosql.iter()))
                    .map_err(|e| host.err(format!("sqlite_query(): {e}")))?;

                while let Some(row) = rows
                    .next()
                    .map_err(|e| host.err(format!("sqlite_query(): {e}")))?
                {
                    let mut m = HashMap::<String, Value>::new();
                    for (i, name) in col_names.iter().enumerate() {
                        let name = name.clone();
                        let v: rusqlite::types::Value = row
                            .get(i)
                            .map_err(|e| host.err(format!("sqlite_query(): {e}")))?;
                        let sv = match v {
                            rusqlite::types::Value::Null => Value::Nil,
                            rusqlite::types::Value::Integer(n) => Value::Number(n as f64),
                            rusqlite::types::Value::Real(n) => Value::Number(n),
                            rusqlite::types::Value::Text(s) => alloc_string(s),
                            rusqlite::types::Value::Blob(b) => {
                                let mut out = Vec::<Value>::with_capacity(b.len());
                                for byte in b {
                                    out.push(Value::Number(byte as f64));
                                }
                                alloc_array(out)
                            }
                        };
                        m.insert(name, sv);
                    }
                    out_rows.push(alloc_map(m));
                }
                out_rows
            };

            let _ = with_db_table(host, |t| t.insert(id, DbResource::Sqlite(conn)));
            out_rows
        }
        None => return Err(host.err("sqlite_query(): invalid handle".into())),
    };

    Ok(alloc_array(rows))
}

pub(super) fn sqlite_last_insert_rowid(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    if !host.allow_sqlite() {
        return Err(host.err("sqlite access denied".into()));
    }
    if args.len() != 1 {
        return Err(host.err(format!(
            "sqlite_last_insert_rowid() expects 1 argument (handle), got {}",
            args.len()
        )));
    }
    let id = value_to_handle(host, args[0].clone(), "sqlite_last_insert_rowid")?;
    let rowid = match with_db_table(host, |t| t.remove(&id))? {
        Some(DbResource::Sqlite(conn)) => {
            let rid = conn.last_insert_rowid();
            let _ = with_db_table(host, |t| t.insert(id, DbResource::Sqlite(conn)));
            rid
        }
        None => return Err(host.err("sqlite_last_insert_rowid(): invalid handle".into())),
    };
    Ok(Value::Number(rowid as f64))
}
