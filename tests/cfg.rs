//! R3-gamma — CFG dump tests

use std::process::Command;

fn pblang_bin() -> std::path::PathBuf {
    // Use CARGO_BIN_EXE_pblang if set, else build path
    if let Ok(p) = std::env::var("CARGO_BIN_EXE_pblang") {
        return std::path::PathBuf::from(p);
    }
    let mut p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("target");
    p.push("debug");
    p.push(if cfg!(windows) { "pblang.exe" } else { "pblang" });
    p
}

fn require_pblang_bin() -> std::path::PathBuf {
    let bin = pblang_bin();
    assert!(
        bin.exists(),
        "pblang binary not found at {}. Build it first with `cargo build --bin pblang`.",
        bin.display()
    );
    bin
}

fn fixture_path(name: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

fn ensure_fixture(name: &str, content: &str) -> std::path::PathBuf {
    let p = fixture_path(name);
    if let Some(parent) = p.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    // Always refresh fixture content to avoid stale files masking regressions.
    std::fs::write(&p, content).expect("write fixture");
    p
}

#[test]
fn cfg_output_contains_basic_blocks() {
    // Create a minimal fixture with branches
    let src = r#"
let x = 1;
if x > 0 {
    print "positive";
} else {
    print "other";
}
print "done";
"#;
    let fixture = ensure_fixture("cfg_sample.sapp", src);
    let bin = require_pblang_bin();

    let output = Command::new(&bin)
        .arg("--cfg")
        .arg(&fixture)
        .output()
        .expect("run pblang --cfg");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "pblang --cfg failed: stderr={}\nstdout={}",
        stderr,
        stdout
    );
    assert!(
        stdout.contains("BB0"),
        "Expected BB0 in CFG output, got: {}\nstderr: {}",
        stdout,
        stderr
    );
}

#[test]
fn cfg_dot_output_is_digraph() {
    let src = "let a = 1;\nif a { print a; }\n";
    let fixture = ensure_fixture("cfg_dot_sample.sapp", src);
    let bin = require_pblang_bin();

    let output = Command::new(&bin)
        .arg("--cfg")
        .arg(&fixture)
        .arg("--cfg-dot")
        .output()
        .expect("run pblang --cfg --cfg-dot");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "pblang --cfg --cfg-dot failed: stderr={}\nstdout={}",
        stderr,
        stdout
    );
    assert!(
        stdout.contains("digraph CFG"),
        "Expected digraph CFG in DOT output, got: {}\nstderr: {}",
        stdout,
        stderr
    );
}
