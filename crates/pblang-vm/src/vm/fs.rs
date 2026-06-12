//! Filesystem / path builtin implementations.
//!
//! Free functions backing the `fs_*`, `path_*`, and `cwd` builtins
//! registered in `VM::builtins()` (vm.rs).
//!
//!   - `fs_read` / `fs_write` / `fs_append` / `fs_remove` / `fs_rename` /
//!     `fs_copy` / `fs_read_bytes` / `fs_write_bytes` — file I/O.
//!   - `fs_exists` / `fs_mkdirs` / `fs_listdir` — file metadata.
//!   - `path_join` / `path_dirname` / `path_basename` — path manipulation.
//!   - `cwd()` — current working directory.
//!
//! The read/write/read_bytes/write_bytes variants honour
//! `host.io_timeout_ms()` by running the blocking call on a dedicated
//! thread and waiting with `recv_timeout`. The append/copy/remove/rename
//! paths and metadata helpers do not — they are short, system-call-only
//! and not worth the extra thread spin.
//!
//! All gated on `host.allow_fs()`.

use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use pblang_ir::{alloc_string, RuntimeError, Value, VmHost};

use super::alloc_array;
use super::errors::{vm_arity, vm_cap, vm_err};

pub(super) fn to_sapp_path(p: &std::path::Path) -> String {
    p.to_string_lossy().replace('\\', "/")
}

pub(super) fn fs_read(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if !host.allow_fs() {
        return Err(vm_cap!(host, "fs_read", "fs"));
    }
    if args.len() != 1 {
        return Err(vm_arity!(host, "fs_read", 1, args.len()));
    }
    let path = args[0]
        .as_str()
        .map_err(|m| vm_err!(host, "fs_read", "{m}"))?
        .to_string();
    let s = if let Some(ms) = host.io_timeout_ms() {
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let _ = tx.send(std::fs::read_to_string(&path));
        });
        rx.recv_timeout(Duration::from_millis(ms))
            .map_err(|_| vm_err!(host, "fs_read", "timeout"))?
            .map_err(|e| vm_err!(host, "fs_read", "{e}"))?
    } else {
        std::fs::read_to_string(&path).map_err(|e| vm_err!(host, "fs_read", "{e}"))?
    };
    Ok(alloc_string(s))
}

pub(super) fn fs_write(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if !host.allow_fs() {
        return Err(vm_cap!(host, "fs_write", "fs"));
    }
    if args.len() != 2 {
        return Err(vm_arity!(host, "fs_write", 2, args.len()));
    }
    let path = args[0]
        .as_str()
        .map_err(|m| vm_err!(host, "fs_write", "{m}"))?
        .to_string();
    let contents = args[1]
        .as_str()
        .map_err(|m| vm_err!(host, "fs_write", "{m}"))?
        .to_string();
    if let Some(ms) = host.io_timeout_ms() {
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let _ = tx.send(std::fs::write(&path, &contents));
        });
        rx.recv_timeout(Duration::from_millis(ms))
            .map_err(|_| vm_err!(host, "fs_write", "timeout"))?
            .map_err(|e| vm_err!(host, "fs_write", "{e}"))?
    } else {
        std::fs::write(&path, &contents).map_err(|e| vm_err!(host, "fs_write", "{e}"))?
    };
    Ok(Value::Nil)
}

pub(super) fn fs_append(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if !host.allow_fs() {
        return Err(vm_cap!(host, "fs_append", "fs"));
    }
    if args.len() != 2 {
        return Err(vm_arity!(host, "fs_append", 2, args.len()));
    }
    let path = args[0]
        .as_str()
        .map_err(|m| vm_err!(host, "fs_append", "{m}"))?;
    let contents = args[1]
        .as_str()
        .map_err(|m| vm_err!(host, "fs_append", "{m}"))?;
    use std::io::Write;
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| vm_err!(host, "fs_append", "{e}"))?;
    f.write_all(contents.as_bytes())
        .map_err(|e| vm_err!(host, "fs_append", "{e}"))?;
    Ok(Value::Nil)
}

pub(super) fn fs_remove(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if !host.allow_fs() {
        return Err(vm_cap!(host, "fs_remove", "fs"));
    }
    if args.len() != 1 {
        return Err(vm_arity!(host, "fs_remove", 1, args.len()));
    }
    let path = args[0]
        .as_str()
        .map_err(|m| vm_err!(host, "fs_remove", "{m}"))?;
    // try file, then dir
    if std::fs::remove_file(path).is_ok() {
        return Ok(Value::Nil);
    }
    std::fs::remove_dir_all(path).map_err(|e| vm_err!(host, "fs_remove", "{e}"))?;
    Ok(Value::Nil)
}

pub(super) fn fs_rename(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if !host.allow_fs() {
        return Err(vm_cap!(host, "fs_rename", "fs"));
    }
    if args.len() != 2 {
        return Err(vm_arity!(host, "fs_rename", 2, args.len()));
    }
    let from = args[0]
        .as_str()
        .map_err(|m| vm_err!(host, "fs_rename", "{m}"))?;
    let to = args[1]
        .as_str()
        .map_err(|m| vm_err!(host, "fs_rename", "{m}"))?;
    std::fs::rename(from, to).map_err(|e| vm_err!(host, "fs_rename", "{e}"))?;
    Ok(Value::Nil)
}

pub(super) fn fs_copy(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if !host.allow_fs() {
        return Err(vm_cap!(host, "fs_copy", "fs"));
    }
    if args.len() != 2 {
        return Err(vm_arity!(host, "fs_copy", 2, args.len()));
    }
    let from = args[0]
        .as_str()
        .map_err(|m| vm_err!(host, "fs_copy", "{m}"))?;
    let to = args[1]
        .as_str()
        .map_err(|m| vm_err!(host, "fs_copy", "{m}"))?;
    std::fs::copy(from, to).map_err(|e| vm_err!(host, "fs_copy", "{e}"))?;
    Ok(Value::Nil)
}

pub(super) fn fs_read_bytes(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    if !host.allow_fs() {
        return Err(vm_cap!(host, "fs_read_bytes", "fs"));
    }
    if args.len() != 1 {
        return Err(vm_arity!(host, "fs_read_bytes", 1, args.len()));
    }
    let path = args[0]
        .as_str()
        .map_err(|m| vm_err!(host, "fs_read_bytes", "{m}"))?
        .to_string();
    let bytes = if let Some(ms) = host.io_timeout_ms() {
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let _ = tx.send(std::fs::read(&path));
        });
        rx.recv_timeout(Duration::from_millis(ms))
            .map_err(|_| vm_err!(host, "fs_read_bytes", "timeout"))?
            .map_err(|e| vm_err!(host, "fs_read_bytes", "{e}"))?
    } else {
        std::fs::read(&path).map_err(|e| vm_err!(host, "fs_read_bytes", "{e}"))?
    };
    let mut out: Vec<Value> = Vec::with_capacity(bytes.len());
    for b in bytes {
        out.push(Value::Number(b as f64));
    }
    Ok(alloc_array(out))
}

pub(super) fn fs_write_bytes(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    if !host.allow_fs() {
        return Err(vm_cap!(host, "fs_write_bytes", "fs"));
    }
    if args.len() != 2 {
        return Err(vm_arity!(host, "fs_write_bytes", 2, args.len()));
    }
    let path = args[0]
        .as_str()
        .map_err(|m| vm_err!(host, "fs_write_bytes", "{m}"))?;
    let arr = match &args[1] {
        Value::Array(a) => a.clone(),
        _ => return Err(vm_err!(host, "fs_write_bytes", "expected byte array")),
    };
    let mut bytes: Vec<u8> = Vec::with_capacity(arr.borrow().len());
    for v in arr.borrow().iter() {
        match v {
            Value::Number(n) if n.is_finite() && n.fract() == 0.0 && *n >= 0.0 && *n <= 255.0 => {
                bytes.push(*n as u8);
            }
            _ => {
                return Err(vm_err!(
                    host,
                    "fs_write_bytes",
                    "expected byte numbers 0..255"
                ))
            }
        }
    }
    let path = path.to_string();
    if let Some(ms) = host.io_timeout_ms() {
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let _ = tx.send(std::fs::write(&path, &bytes));
        });
        rx.recv_timeout(Duration::from_millis(ms))
            .map_err(|_| vm_err!(host, "fs_write_bytes", "timeout"))?
            .map_err(|e| vm_err!(host, "fs_write_bytes", "{e}"))?
    } else {
        std::fs::write(&path, &bytes).map_err(|e| vm_err!(host, "fs_write_bytes", "{e}"))?
    };
    Ok(Value::Nil)
}

pub(super) fn cwd(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if !host.allow_fs() {
        return Err(vm_cap!(host, "cwd", "fs"));
    }
    if !args.is_empty() {
        return Err(host.err(format!("cwd() takes no arguments, got {}", args.len())));
    }
    let p = std::env::current_dir().map_err(|e| vm_err!(host, "cwd", "{e}"))?;
    Ok(alloc_string(to_sapp_path(&p)))
}

pub(super) fn fs_exists(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if !host.allow_fs() {
        return Err(vm_cap!(host, "fs_exists", "fs"));
    }
    if args.len() != 1 {
        return Err(vm_arity!(host, "fs_exists", 1, args.len()));
    }
    let path = args[0]
        .as_str()
        .map_err(|m| vm_err!(host, "fs_exists", "{m}"))?;
    Ok(Value::Bool(std::path::Path::new(path).exists()))
}

pub(super) fn fs_mkdirs(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if !host.allow_fs() {
        return Err(vm_cap!(host, "fs_mkdirs", "fs"));
    }
    if args.len() != 1 {
        return Err(vm_arity!(host, "fs_mkdirs", 1, args.len()));
    }
    let path = args[0]
        .as_str()
        .map_err(|m| vm_err!(host, "fs_mkdirs", "{m}"))?;
    std::fs::create_dir_all(path).map_err(|e| vm_err!(host, "fs_mkdirs", "{e}"))?;
    Ok(Value::Nil)
}

pub(super) fn fs_listdir(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if !host.allow_fs() {
        return Err(vm_cap!(host, "fs_listdir", "fs"));
    }
    if args.len() != 1 {
        return Err(vm_arity!(host, "fs_listdir", 1, args.len()));
    }
    let path = args[0]
        .as_str()
        .map_err(|m| vm_err!(host, "fs_listdir", "{m}"))?;
    let mut names = vec![];
    let rd = std::fs::read_dir(path).map_err(|e| vm_err!(host, "fs_listdir", "{e}"))?;
    for ent in rd {
        let ent = ent.map_err(|e| vm_err!(host, "fs_listdir", "{e}"))?;
        let name = ent.file_name().to_string_lossy().to_string();
        names.push(alloc_string(name));
    }
    // stable order for deterministic programs/tests
    names.sort_by_key(|a| a.to_string());
    Ok(alloc_array(names))
}

pub(super) fn path_join(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if !host.allow_fs() {
        return Err(vm_cap!(host, "path_join", "fs"));
    }
    if args.len() < 2 {
        return Err(host.err(format!(
            "path_join() expects at least 2 arguments, got {}",
            args.len()
        )));
    }
    let mut pb = std::path::PathBuf::new();
    for a in args {
        let s = a.as_str().map_err(|m| vm_err!(host, "path_join", "{m}"))?;
        pb.push(s);
    }
    Ok(alloc_string(to_sapp_path(&pb)))
}

pub(super) fn path_dirname(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if !host.allow_fs() {
        return Err(vm_cap!(host, "path_dirname", "fs"));
    }
    if args.len() != 1 {
        return Err(vm_arity!(host, "path_dirname", 1, args.len()));
    }
    let s = args[0]
        .as_str()
        .map_err(|m| vm_err!(host, "path_dirname", "{m}"))?;
    let p = std::path::Path::new(s);
    let out = p
        .parent()
        .unwrap_or_else(|| std::path::Path::new(""))
        .to_path_buf();
    Ok(alloc_string(to_sapp_path(&out)))
}

pub(super) fn path_basename(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    if !host.allow_fs() {
        return Err(vm_cap!(host, "path_basename", "fs"));
    }
    if args.len() != 1 {
        return Err(vm_arity!(host, "path_basename", 1, args.len()));
    }
    let s = args[0]
        .as_str()
        .map_err(|m| vm_err!(host, "path_basename", "{m}"))?;
    let p = std::path::Path::new(s);
    let out = p
        .file_name()
        .and_then(|x| x.to_str())
        .unwrap_or("")
        .to_string();
    Ok(alloc_string(out))
}
