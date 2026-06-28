//! `pblang check` — smart diagnostics (errors, warnings, "did you mean", autofix hints).
//!
//! All analysis lives in the self-hosted engine `stdlib/diagnostics.sapp`, driven by
//! `stdlib/check_cli.sapp`. This Rust command only locates that driver, runs it through
//! the same `pblang` binary, and maps the reported error count to a process exit code.
//! No diagnostic logic is duplicated in Rust — the engine is the single source of truth,
//! shared with the watchers (`watch.diagnose`/`suggest`/`autofix`).

use std::path::PathBuf;
use std::process::Command;

const DRIVER_SRC: &str = include_str!("../../stdlib/check_cli.sapp");

/// Find the on-disk driver (so it shares the live stdlib); fall back to writing the
/// embedded copy to a temp file.
fn resolve_driver() -> PathBuf {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let candidate = cwd.join("stdlib").join("check_cli.sapp");
    if candidate.exists() {
        return candidate;
    }
    if let Ok(exe) = std::env::current_exe() {
        let mut dir = exe.parent().map(|p| p.to_path_buf());
        for _ in 0..4 {
            if let Some(d) = &dir {
                let c = d.join("stdlib").join("check_cli.sapp");
                if c.exists() {
                    return c;
                }
                dir = d.parent().map(|p| p.to_path_buf());
            }
        }
    }
    let tmp = std::env::temp_dir().join("pblang_check_cli.sapp");
    let _ = std::fs::write(&tmp, DRIVER_SRC);
    tmp
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

    let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("pblang"));
    let driver = resolve_driver();
    let status_file =
        std::env::temp_dir().join(format!("pblang_check_status_{}", std::process::id()));
    let _ = std::fs::remove_file(&status_file);

    let mut cmd = Command::new(exe);
    cmd.arg("run").arg(&driver).arg("--allow-all").arg("--");
    for f in &files {
        cmd.arg(f);
    }
    cmd.env("PBLANG_CHECK_STATUS_FILE", &status_file);
    let run_ok = cmd.status().map(|s| s.success()).unwrap_or(false);

    // The driver writes the total error count to the status file; absent that, fall
    // back to the child's success status.
    let errors: i64 = std::fs::read_to_string(&status_file)
        .ok()
        .and_then(|s| s.trim().parse::<i64>().ok())
        .unwrap_or(if run_ok { 0 } else { 1 });
    let _ = std::fs::remove_file(&status_file);

    if errors > 0 {
        std::process::exit(1);
    }
}
