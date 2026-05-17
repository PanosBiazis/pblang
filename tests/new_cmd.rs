//! PF1-alpha — `pblang new` scaffold tests

use std::process::Command;
use std::process::Stdio;
use std::time::{Duration, Instant};
use std::{io::{Read, Write}, net::{TcpListener, TcpStream}};

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

fn require_pblang_bin() -> std::path::PathBuf {
    let bin = pblang_bin();
    assert!(
        bin.exists(),
        "pblang binary not found at {}. Build it first with `cargo build --bin pblang`.",
        bin.display()
    );
    bin
}

#[test]
fn version_flag_prints_package_version() {
    let bin = require_pblang_bin();

    let output = Command::new(&bin)
        .arg("--version")
        .output()
        .expect("run pblang --version");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "--version should succeed: stderr={stderr}\nstdout={stdout}"
    );
    assert!(
        stdout.contains(env!("CARGO_PKG_VERSION")),
        "expected version output to contain {}, got: {}",
        env!("CARGO_PKG_VERSION"),
        stdout
    );
}

fn pick_free_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind free port probe");
    listener.local_addr().expect("probe local addr").port()
}

fn http_get(port: u16, path: &str) -> Result<String, String> {
    let mut stream =
        TcpStream::connect(("127.0.0.1", port)).map_err(|e| format!("connect: {e}"))?;
    stream
        .set_read_timeout(Some(Duration::from_millis(500)))
        .map_err(|e| format!("set read timeout: {e}"))?;
    let req = format!(
        "GET {} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
        path
    );
    stream
        .write_all(req.as_bytes())
        .map_err(|e| format!("write req: {e}"))?;
    let mut s = String::new();
    stream
        .read_to_string(&mut s)
        .map_err(|e| format!("read resp: {e}"))?;
    Ok(s)
}

fn http_post_json(port: u16, path: &str, body: &str) -> Result<String, String> {
    let mut stream =
        TcpStream::connect(("127.0.0.1", port)).map_err(|e| format!("connect: {e}"))?;
    stream
        .set_read_timeout(Some(Duration::from_millis(1200)))
        .map_err(|e| format!("set read timeout: {e}"))?;
    let req = format!(
        "POST {} HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        path,
        body.len(),
        body
    );
    stream
        .write_all(req.as_bytes())
        .map_err(|e| format!("write req: {e}"))?;
    let mut s = String::new();
    stream
        .read_to_string(&mut s)
        .map_err(|e| format!("read resp: {e}"))?;
    Ok(s)
}

#[test]
fn new_creates_web_scaffold() {
    let bin = require_pblang_bin();

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
    let bin = require_pblang_bin();

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

#[test]
fn compile_rejects_conflicting_llvm_and_wasm_flags() {
    let bin = require_pblang_bin();

    let tmp = std::env::temp_dir().join("pblang_compile_flag_conflict");
    let _ = std::fs::remove_dir_all(&tmp);
    let _ = std::fs::create_dir_all(&tmp);
    let script = tmp.join("main.sapp");
    std::fs::write(&script, "print 1;\n").expect("write script");

    let output = Command::new(&bin)
        .arg("compile")
        .arg(&script)
        .arg("--llvm")
        .arg("--wasm")
        .output()
        .expect("run pblang compile with conflicting flags");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "compile should fail when both --llvm and --wasm are passed"
    );
    assert!(
        stderr.contains("mutually exclusive"),
        "expected mutually exclusive flag error, got stderr: {}",
        stderr
    );

    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn serve_exposes_examples_catalog_and_content() {
    let bin = require_pblang_bin();

    let port = pick_free_port();
    let mut child = Command::new(&bin)
        .arg("serve")
        .arg("--port")
        .arg(port.to_string())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn pblang serve");

    let deadline = Instant::now() + Duration::from_secs(5);
    let mut healthy = false;
    while Instant::now() < deadline {
        if let Ok(resp) = http_get(port, "/health") {
            if resp.contains("\"ok\":true") {
                healthy = true;
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(healthy, "serve backend did not become healthy in time");

    let catalog = http_get(port, "/examples").expect("GET /examples");
    assert!(
        catalog.contains("\"examples\""),
        "expected examples list in catalog response: {}",
        catalog
    );
    assert!(
        catalog.contains("tutorial_cli") || catalog.contains("hello_world"),
        "expected at least one known example id in catalog response: {}",
        catalog
    );

    let ex = http_get(port, "/examples/tutorial_cli").expect("GET /examples/tutorial_cli");
    assert!(
        ex.contains("\"ok\":true") && ex.contains("\"code\""),
        "expected example payload with code: {}",
        ex
    );

    let manual = http_get(port, "/manual/ide").expect("GET /manual/ide");
    assert!(
        manual.contains("\"ok\":true") && manual.contains("\"content\""),
        "expected manual payload with content: {}",
        manual
    );
    assert!(
        manual.contains("IDE Manual") || manual.contains("pblang IDE Manual"),
        "expected manual title/content in response: {}",
        manual
    );

    let _ = child.kill();
    let _ = child.wait();
}

#[test]
fn serve_accepts_large_json_body_for_check() {
    let bin = require_pblang_bin();

    let port = pick_free_port();
    let mut child = Command::new(&bin)
        .arg("serve")
        .arg("--port")
        .arg(port.to_string())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn pblang serve");

    let deadline = Instant::now() + Duration::from_secs(5);
    let mut healthy = false;
    while Instant::now() < deadline {
        if let Ok(resp) = http_get(port, "/health") {
            if resp.contains("\"ok\":true") {
                healthy = true;
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(healthy, "serve backend did not become healthy in time");

    // Build >8KB source body; previously this was truncated by single-read request handling.
    let mut code = String::from("print \"start\";\n");
    for i in 0..1400 {
        code.push_str(&format!("// filler line {i}\n"));
    }
    code.push_str("print \"end\";\n");
    let payload = serde_json::json!({ "code": code }).to_string();
    assert!(payload.len() > 9000, "payload should exceed old 8KB limit");

    let resp = http_post_json(port, "/check", &payload).expect("POST /check");
    assert!(
        resp.contains("\"ok\":true"),
        "expected successful check for large body, got: {}",
        resp
    );
    assert!(
        !resp.contains("invalid request body"),
        "large body should not be truncated/invalid: {}",
        resp
    );

    let _ = child.kill();
    let _ = child.wait();
}
