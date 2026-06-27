//! Drift guard for `stdlib/compiler/builtins_list.sapp` vs. the real VM builtin
//! registration. That file is a GENERATED mirror of every native builtin name,
//! used by the self-hosted linter for SOUND undefined-name detection — if it
//! falls behind the Rust table, the linter falsely reports valid builtin calls
//! as "undefined name". (Same spirit as `fmt_drift.rs` for the formatter.)
//!
//! Authoritative sources (the registration `name: "..."` entries):
//!   * crates/pblang-vm/src/vm/builtins_table.rs — the always-on table
//!   * crates/pblang-graphics/src/graphics.rs    — gfx_builtins(), extended
//!     unconditionally into the VM table by vm.rs (minifb default backend)
//!
//! Every registered builtin name MUST appear in builtins_list.sapp. The reverse
//! is intentionally NOT required: the .sapp set also lists literal/keyword names
//! the linter must not flag (`true`, `false`, `nil`, `self`, `print`).

use std::collections::BTreeSet;
use std::path::PathBuf;

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Extract every `name: "<ident>"` registration name from a Rust source file.
fn rust_builtin_names(rel_path: &str) -> BTreeSet<String> {
    let path = manifest_dir().join(rel_path);
    let src = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    extract_after_marker(&src, "name:")
}

/// Extract every `"<ident>": true` key from the self-hosted builtins list.
fn sapp_builtin_names() -> BTreeSet<String> {
    let path = manifest_dir().join("stdlib/compiler/builtins_list.sapp");
    let src = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let mut out = BTreeSet::new();
    // Match `"<ident>": true` (the map entries). A simple scan: find each quoted
    // string immediately followed (after optional spaces) by `:` then `true`.
    let bytes = src.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'"' {
            if let Some(end) = src[i + 1..].find('"') {
                let ident = &src[i + 1..i + 1 + end];
                let rest = src[i + 1 + end + 1..].trim_start();
                if rest.starts_with(": true") && is_ident(ident) {
                    out.insert(ident.to_string());
                }
                i = i + 1 + end + 1;
                continue;
            }
        }
        i += 1;
    }
    out
}

/// Pull the quoted identifier that follows each occurrence of `marker`.
fn extract_after_marker(src: &str, marker: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for chunk in src.split(marker).skip(1) {
        let rest = chunk.trim_start();
        if let Some(stripped) = rest.strip_prefix('"') {
            if let Some(end) = stripped.find('"') {
                let ident = &stripped[..end];
                if is_ident(ident) {
                    out.insert(ident.to_string());
                }
            }
        }
    }
    out
}

fn is_ident(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_')
        && !s.chars().next().unwrap().is_ascii_digit()
}

#[test]
fn builtins_list_covers_every_registered_builtin() {
    let mut rust: BTreeSet<String> = rust_builtin_names("crates/pblang-vm/src/vm/builtins_table.rs");
    rust.extend(rust_builtin_names("crates/pblang-graphics/src/graphics.rs"));

    // Sanity: the extraction actually found a substantial table (guards against a
    // path/format change silently making this test vacuous).
    assert!(
        rust.len() > 200,
        "expected >200 registered builtins, found {} — extraction likely broke",
        rust.len()
    );

    let sapp = sapp_builtin_names();
    let missing: Vec<&String> = rust.iter().filter(|n| !sapp.contains(*n)).collect();
    assert!(
        missing.is_empty(),
        "stdlib/compiler/builtins_list.sapp is missing {} registered builtin(s): {:?}\n\
         Regenerate it from crates/pblang-vm/src/vm/builtins_table.rs (+ gfx_builtins).",
        missing.len(),
        missing
    );
}
