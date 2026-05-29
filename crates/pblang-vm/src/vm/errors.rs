//! Error-construction macros for VM builtins.
//!
//! Goal: every user-facing error from a builtin reads `"<name>(): <reason>"`
//! (or `"<name>() expects N argument(s), got M"` for arity) so the failing
//! builtin can be identified at a glance and grepped in logs. Before these
//! macros existed the same three patterns were re-spelled in every submodule
//! and a handful of capability errors (`"fs access denied"`,
//! `"exec access denied"`, …) carried no builtin prefix at all.
//!
//! Usage from a sibling submodule (e.g. `vm/strings.rs`):
//!
//! ```ignore
//! use super::errors::{vm_arity, vm_cap, vm_err};
//!
//! pub(super) fn str_contains(host: &mut dyn VmHost, args: Vec<Value>)
//!     -> Result<Value, RuntimeError>
//! {
//!     if args.len() != 2 {
//!         return Err(vm_arity!(host, "str_contains", 2, args.len()));
//!     }
//!     let s = args[0].as_str().map_err(|m| vm_err!(host, "str_contains", "{m}"))?;
//!     // ...
//! }
//! ```
//!
//! The macros expand to a `RuntimeError` (via `host.err(...)`); they do not
//! `return` themselves so the caller still spells `Err(...)` / `?` /
//! `map_err(...)` as it sees fit.

/// Build a `RuntimeError` with the canonical `"<builtin>(): <reason>"` prefix.
///
/// Arguments after the builtin name are forwarded to `format_args!`, so this
/// behaves like a `format!` with an automatic prefix:
///
/// ```ignore
/// vm_err!(host, "polyglot_exec", "{}", inner)
/// // → host.err("polyglot_exec(): <inner>")
///
/// vm_err!(host, "fs_read", "expected string path, got {:?}", got)
/// // → host.err("fs_read(): expected string path, got <got>")
/// ```
macro_rules! vm_err {
    ($host:expr, $builtin:literal, $($arg:tt)*) => {
        $host.err(format!("{}(): {}", $builtin, format_args!($($arg)*)))
    };
}

/// Build a `RuntimeError` describing an arity mismatch.
///
/// Style preserved from the pre-macro codebase: no colon after the parens, and
/// the word "argument" pluralises on `expected`.
///
/// ```ignore
/// vm_arity!(host, "str_contains", 2, args.len())
/// // → host.err("str_contains() expects 2 arguments, got <N>")
/// ```
macro_rules! vm_arity {
    ($host:expr, $builtin:literal, $expected:expr, $got:expr) => {{
        let exp: usize = $expected;
        let got: usize = $got;
        let plural = if exp == 1 { "argument" } else { "arguments" };
        $host.err(format!(
            "{}() expects {} {}, got {}",
            $builtin, exp, plural, got
        ))
    }};
}

/// Build a `RuntimeError` for a capability-denied path.
///
/// ```ignore
/// vm_cap!(host, "fs_read", "fs")
/// // → host.err("fs_read(): fs access denied")
/// ```
///
/// Replaces bare `"fs access denied"` / `"exec access denied"` strings that
/// omitted the builtin name. The capability label is whatever the gate is
/// called in `VmHost` (`fs`, `exec`, `ffi`, `net`, `http`).
macro_rules! vm_cap {
    ($host:expr, $builtin:literal, $cap:literal) => {
        $host.err(format!("{}(): {} access denied", $builtin, $cap))
    };
}

pub(super) use vm_arity;
pub(super) use vm_cap;
pub(super) use vm_err;
