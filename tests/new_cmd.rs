//! PF1-alpha — `pblang new` scaffold tests

use std::process::Command;

fn pblang_bin() -> std::path::PathBuf {
    if let Ok(p) = std::env::var("CARGO_BIN_EXE_pblang") {
        return std::path::PathBuf::from(p);
    }
    let mut p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("target");
    p.push("debug");
    p.push(if cfg!(windows) {
        "pblang.exe"
    } else {
        "pblang"
    });
    p
}

#[test]
fn new_creates_web_scaffold() {
    let bin = pblang_bin();
    if !bin.exists() {
        eprintln!(
            "Skipping new_cmd test: pblang binary not found at {:?}",
            bin
        );
        return;
    }

    let tmp = std::env::temp_dir().join("pblang_new_test_web");
    let _ = std::fs::remove_dir_all(&tmp);
    let _ = std::fs::create_dir_all(&tmp);

    let output = Command::new(&bin)
        .arg("new")
        .arg("mywebapp")
        .arg("--template")
        .arg("web")
        .current_dir(&tmp)
        .output()
        .expect("run pblang new");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "pblang new failed: stderr={}\nstdout={}",
        stderr,
        stdout
    );

    let manifest = tmp.join("mywebapp").join("pblang.toml");
    let main_sapp = tmp.join("mywebapp").join("src").join("main.sapp");

    assert!(
        manifest.exists(),
        "pblang.toml not created at {:?}",
        manifest
    );
    assert!(
        main_sapp.exists(),
        "src/main.sapp not created at {:?}",
        main_sapp
    );

    let manifest_content = std::fs::read_to_string(&manifest).expect("read pblang.toml");
    assert!(
        manifest_content.contains("mywebapp"),
        "pblang.toml should contain project name"
    );

    let main_content = std::fs::read_to_string(&main_sapp).expect("read main.sapp");
    assert!(
        main_content.contains("http_server"),
        "web template main.sapp should import http_server"
    );

    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn new_creates_cli_scaffold_default() {
    let bin = pblang_bin();
    if !bin.exists() {
        eprintln!("Skipping new_cmd test: pblang binary not found");
        return;
    }

    let tmp = std::env::temp_dir().join("pblang_new_test_cli");
    let _ = std::fs::remove_dir_all(&tmp);
    let _ = std::fs::create_dir_all(&tmp);

    let output = Command::new(&bin)
        .arg("new")
        .arg("mycli")
        .current_dir(&tmp)
        .output()
        .expect("run pblang new mycli");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(output.status.success(), "pblang new failed: {}", stderr);

    let manifest = tmp.join("mycli").join("pblang.toml");
    assert!(manifest.exists(), "pblang.toml should exist: {}", stdout);

    let _ = std::fs::remove_dir_all(&tmp);
}
