//! `pblang check` — smart diagnostics (errors, warnings, "did you mean", autofix hints).
//!
//! All analysis lives in the self-hosted engine `stdlib/diagnostics.sapp`, driven by
//! `stdlib/check_cli.sapp`. This Rust command only locates that driver, runs it through
//! the same `pblang` binary, and maps the reported error count to a process exit code.
//! No diagnostic logic is duplicated in Rust — the engine is the single source of truth,
//! shared with the watchers (`watch.diagnose`/`suggest`/`autofix`).
//!
//! Security notes:
//!   * The driver is loaded ONLY from the trusted, executable-relative stdlib (the
//!     installed/bundled location), never from a temp file or a cwd-first search — so
//!     `pblang check` in an attacker-controlled directory cannot execute a planted
//!     `stdlib/check_cli.sapp` with `--allow-all`. (The `stdlib:` import inside the
//!     driver also requires the project/install tree, so an exe-relative anchor is the
//!     correct one regardless.)
//!   * The status file used to pass the error count back from the child lives inside a
//!     freshly created private temp directory (mode 0700, unguessable name, owned by
//!     us, auto-removed on drop), eliminating symlink / TOCTOU / shared-/tmp races.

use std::path::PathBuf;
use std::process::Command;

/// Locate the driver next to the running executable's stdlib. Walks up a few levels
/// from the exe so it works both for an installed layout (`.../bin/pblang` +
/// `.../stdlib`) and the dev tree (`target/debug/pblang` + repo `stdlib`). Returns
/// `None` rather than ever materializing code in a world-writable location.
fn resolve_driver() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let mut dir = exe.parent().map(|p| p.to_path_buf());
    for _ in 0..5 {
        let d = dir?;
        let cand = d.join("stdlib").join("check_cli.sapp");
        if cand.is_file() {
            return Some(cand);
        }
        dir = d.parent().map(|p| p.to_path_buf());
    }
    None
}

pub(crate) fn handle_check(args: Vec<String>) {
    // args[0] == "check"; remaining non-flag args are files to check.
    let files: Vec<String> = args
        .iter()
        .skip(1)
        .filter(|a| !a.starts_with('-'))
        .cloned()
        .collect();
    if files.is_empty() {
        eprintln!("pblang check: pass one or more .sap or .sapp files");
        std::process::exit(1);
    }

    let driver = match resolve_driver() {
        Some(d) => d,
        None => {
            eprintln!(
                "pblang check: could not locate stdlib/check_cli.sapp next to the pblang \
                 executable; is the installation complete?"
            );
            std::process::exit(1);
        }
    };

    let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("pblang"));

    // Absolutize the target files against the caller's cwd, then run the child with its
    // cwd set to the driver's project root (the dir holding stdlib/ + pblang.toml). This
    // gives the self-hosted frontend a consistent module context so diagnostics are
    // identical no matter where `pblang check` is invoked from.
    let caller_cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let abs_files: Vec<PathBuf> = files
        .iter()
        .map(|f| {
            let p = PathBuf::from(f);
            if p.is_absolute() {
                p
            } else {
                caller_cwd.join(p)
            }
        })
        .collect();
    // driver = <root>/stdlib/check_cli.sapp -> project root is two parents up.
    let project_root = driver
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.to_path_buf());

    // Private, securely-created temp dir for the status hand-off (the child has no
    // exit() builtin, so it writes its error count to a file we then read).
    let tmp = match tempfile::Builder::new().prefix("pblang-check-").tempdir() {
        Ok(t) => t,
        Err(e) => {
            eprintln!("pblang check: failed to create secure temp dir: {e}");
            std::process::exit(1);
        }
    };
    let status_file = tmp.path().join("status");

    let mut cmd = Command::new(exe);
    if let Some(root) = &project_root {
        cmd.current_dir(root);
    }
    cmd.arg("run").arg(&driver).arg("--allow-all").arg("--");
    for f in &abs_files {
        cmd.arg(f);
    }
    cmd.env("PBLANG_CHECK_STATUS_FILE", &status_file);
    let run_ok = cmd.status().map(|s| s.success()).unwrap_or(false);

    // The status file lives in a directory only we can write to, so reading it is safe.
    let errors: i64 = std::fs::read_to_string(&status_file)
        .ok()
        .and_then(|s| s.trim().parse::<i64>().ok())
        .unwrap_or(if run_ok { 0 } else { 1 });
    // `tmp` (TempDir) is removed on drop.

    if errors > 0 {
        std::process::exit(1);
    }
}
