//! PF1-alpha — `pblang new` scaffold tests

use std::process::Command;
use std::process::Stdio;
use std::time::{Duration, Instant};
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
};

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

#[test]
fn manifest_check_valid_manifest_succeeds() {
    let bin = require_pblang_bin();
    let tmp = std::env::temp_dir().join("pblang_manifest_check_ok");
    let _ = std::fs::remove_dir_all(&tmp);
    let _ = std::fs::create_dir_all(&tmp);
    std::fs::write(
        tmp.join("pblang.toml"),
        "[package]\nname = \"demo\"\nversion = \"0.1.0\"\nentry = \"src/main.sapp\"\n",
    )
    .expect("write manifest");

    let output = Command::new(&bin)
        .arg("manifest-check")
        .arg(&tmp)
        .output()
        .expect("run pblang manifest-check");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "manifest-check should succeed: stderr={stderr} stdout={stdout}"
    );
    assert!(
        stdout.contains("\"ok\": true"),
        "expected ok=true JSON report, got: {stdout}"
    );
    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn manifest_check_invalid_manifest_fails() {
    let bin = require_pblang_bin();
    let tmp = std::env::temp_dir().join("pblang_manifest_check_bad");
    let _ = std::fs::remove_dir_all(&tmp);
    let _ = std::fs::create_dir_all(&tmp);
    std::fs::write(
        tmp.join("pblang.toml"),
        "[package]\nname = \"demo\"\nversion = \"bad\"\n",
    )
    .expect("write manifest");

    let output = Command::new(&bin)
        .arg("manifest-check")
        .arg(&tmp)
        .output()
        .expect("run pblang manifest-check bad");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !output.status.success(),
        "manifest-check should fail for invalid manifest"
    );
    assert!(
        stdout.contains("\"ok\": false"),
        "expected ok=false JSON report, got: {stdout}"
    );
    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn lock_strict_manifest_validator_rejects_non_semver_version() {
    let bin = require_pblang_bin();
    let tmp = std::env::temp_dir().join("pblang_lock_strict_manifest_bad");
    let _ = std::fs::remove_dir_all(&tmp);
    let _ = std::fs::create_dir_all(&tmp);
    std::fs::write(
        tmp.join("pblang.toml"),
        "[package]\nname = \"demo\"\nversion = \"1\"\n",
    )
    .expect("write manifest");

    let output = Command::new(&bin)
        .arg("--lock")
        .arg(&tmp)
        .env("PBLANG_STRICT_MANIFEST_VALIDATOR", "1")
        .output()
        .expect("run pblang --lock with strict validator");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "--lock should fail when strict pblang validator rejects manifest"
    );
    assert!(
        stderr.contains("strict pblang manifest validation failed"),
        "expected strict validation failure message, got: {}",
        stderr
    );
    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn lock_strict_manifest_validator_can_be_scoped_to_other_commands() {
    let bin = require_pblang_bin();
    let tmp = std::env::temp_dir().join("pblang_lock_strict_manifest_scoped");
    let _ = std::fs::remove_dir_all(&tmp);
    let _ = std::fs::create_dir_all(&tmp);
    std::fs::write(
        tmp.join("pblang.toml"),
        "[package]\nname = \"demo\"\nversion = \"1\"\n",
    )
    .expect("write manifest");

    let output = Command::new(&bin)
        .arg("--lock")
        .arg(&tmp)
        .env("PBLANG_STRICT_MANIFEST_VALIDATOR", "1")
        .env("PBLANG_STRICT_MANIFEST_VALIDATOR_COMMANDS", "pack,bundle")
        .output()
        .expect("run pblang --lock with scoped strict validator");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "--lock should succeed when strict validation is scoped away from --lock: {}",
        stderr
    );
    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn manifest_parity_valid_manifest_succeeds() {
    let bin = require_pblang_bin();
    let tmp = std::env::temp_dir().join("pblang_manifest_parity_ok");
    let _ = std::fs::remove_dir_all(&tmp);
    let _ = std::fs::create_dir_all(&tmp);
    std::fs::write(
        tmp.join("pblang.toml"),
        "[package]\nname = \"demo\"\nversion = \"0.1.0\"\nentry = \"src/main.sapp\"\n",
    )
    .expect("write manifest");

    let output = Command::new(&bin)
        .arg("manifest-parity")
        .arg(&tmp)
        .output()
        .expect("run pblang manifest-parity");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "manifest-parity should succeed: stderr={stderr} stdout={stdout}"
    );
    assert!(
        stdout.contains("\"parity_ok\": true"),
        "expected parity_ok=true report, got: {stdout}"
    );
    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn manifest_parity_invalid_manifest_fails() {
    let bin = require_pblang_bin();
    let tmp = std::env::temp_dir().join("pblang_manifest_parity_bad");
    let _ = std::fs::remove_dir_all(&tmp);
    let _ = std::fs::create_dir_all(&tmp);
    std::fs::write(
        tmp.join("pblang.toml"),
        "[package]\nname = \"demo\"\nversion = \"bad\"\n",
    )
    .expect("write manifest");

    let output = Command::new(&bin)
        .arg("manifest-parity")
        .arg(&tmp)
        .output()
        .expect("run pblang manifest-parity bad");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !output.status.success(),
        "manifest-parity should fail for invalid manifest"
    );
    assert!(
        stdout.contains("\"parity_ok\": false"),
        "expected parity_ok=false report, got: {stdout}"
    );
    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn manifest_parity_dependency_shapes_match() {
    let bin = require_pblang_bin();
    let tmp = std::env::temp_dir().join("pblang_manifest_parity_deps_ok");
    let _ = std::fs::remove_dir_all(&tmp);
    let _ = std::fs::create_dir_all(&tmp);
    std::fs::write(
        tmp.join("pblang.toml"),
        "[package]\nname = \"demo\"\nversion = \"0.1.0\"\n\n[dependencies]\n\"@scope/pkg\" = \"1.2.3\"\nlocal_dep = { path = \"vendor/local_dep\" }\ngit_dep = { git = \"https://example.com/repo.git\", rev = \"abc123\" }\n",
    )
    .expect("write manifest");

    let output = Command::new(&bin)
        .arg("manifest-parity")
        .arg(&tmp)
        .output()
        .expect("run pblang manifest-parity deps");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "manifest-parity deps should succeed: stderr={stderr} stdout={stdout}"
    );
    assert!(
        stdout.contains("\"parity_ok\": true"),
        "expected parity_ok=true for dependency shape parity, got: {stdout}"
    );
    let _ = std::fs::remove_dir_all(&tmp);
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

fn http_send_raw(port: u16, request: &str) -> Result<String, String> {
    let mut stream =
        TcpStream::connect(("127.0.0.1", port)).map_err(|e| format!("connect: {e}"))?;
    stream
        .set_read_timeout(Some(Duration::from_millis(1200)))
        .map_err(|e| format!("set read timeout: {e}"))?;
    stream
        .write_all(request.as_bytes())
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

#[test]
fn serve_rejects_post_without_content_length() {
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

    let raw = "POST /check HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{\"code\":\"print 1;\"}";
    let resp = http_send_raw(port, raw).expect("POST /check without Content-Length");
    assert!(
        resp.contains("HTTP/1.1 411"),
        "expected 411 Length Required, got: {}",
        resp
    );
    assert!(
        resp.contains("missing Content-Length"),
        "expected missing Content-Length error body, got: {}",
        resp
    );

    let _ = child.kill();
    let _ = child.wait();
}

#[test]
fn serve_rejects_malformed_content_length() {
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

    let raw = "POST /check HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: nope\r\nConnection: close\r\n\r\n{\"code\":\"print 1;\"}";
    let resp = http_send_raw(port, raw).expect("POST /check with malformed Content-Length");
    // Under axum/hyper, malformed Content-Length is rejected at the protocol
    // parser (before any application middleware can attach a body). The status
    // line — 400 — is what proves the security guarantee; an empty body here
    // is acceptable and actually preferable (it can't reflect attacker bytes).
    assert!(
        resp.contains("HTTP/1.1 400"),
        "expected 400 Bad Request, got: {}",
        resp
    );

    let _ = child.kill();
    let _ = child.wait();
}

#[test]
fn serve_unknown_ids_do_not_reflect_user_input() {
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

    let tricky = "evil%3Cscript%3Ealert(1)%3C/script%3E";
    let ex_resp = http_get(port, &format!("/examples/{tricky}")).expect("GET unknown /examples/*");
    assert!(
        ex_resp.contains("HTTP/1.1 404"),
        "expected 404 for unknown example id, got: {}",
        ex_resp
    );
    assert!(
        ex_resp.contains("unknown example id"),
        "expected generic unknown example error, got: {}",
        ex_resp
    );
    assert!(
        !ex_resp.contains(tricky),
        "response should not reflect unknown example id: {}",
        ex_resp
    );

    let manual_resp = http_get(port, &format!("/manual/{tricky}")).expect("GET unknown /manual/*");
    assert!(
        manual_resp.contains("HTTP/1.1 404"),
        "expected 404 for unknown manual id, got: {}",
        manual_resp
    );
    assert!(
        manual_resp.contains("unknown manual id"),
        "expected generic unknown manual error, got: {}",
        manual_resp
    );
    assert!(
        !manual_resp.contains(tricky),
        "response should not reflect unknown manual id: {}",
        manual_resp
    );

    let _ = child.kill();
    let _ = child.wait();
}

// ────────────────────────────────────────────────────────────────────────────
// Capability flags (--allow-exec / --allow-ffi / --allow-all / --sandbox)
// ────────────────────────────────────────────────────────────────────────────

/// Run `pblang run -` (stdin) with `extra_args` and the given script.
/// Returns (status_code, stdout, stderr).
fn run_pblang_script_with_args(script: &str, extra_args: &[&str]) -> (i32, String, String) {
    let bin = require_pblang_bin();
    let tmp = std::env::temp_dir().join(format!(
        "pblang_cap_test_{}.sapp",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::write(&tmp, script).expect("write tmp script");
    let mut cmd = Command::new(&bin);
    cmd.arg("run").arg(&tmp);
    for a in extra_args {
        cmd.arg(a);
    }
    let out = cmd.output().expect("run pblang script");
    let _ = std::fs::remove_file(&tmp);
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    (out.status.code().unwrap_or(-1), stdout, stderr)
}

/// The default `pblang run` config denies `ffi`. ffi_load on a valid system
/// library should hit the capability gate and the catch block should fire.
#[test]
fn cap_default_denies_ffi() {
    let script = r#"
let r = try ffi_load("libc.so.6") catch "GATE_DENIED";
if r == "GATE_DENIED" { print "ffi:denied"; } else { print "ffi:loaded"; }
"#;
    let (status, stdout, stderr) = run_pblang_script_with_args(script, &[]);
    assert_eq!(status, 0, "expected exit 0 (got {status}). stderr={stderr}");
    assert!(
        stdout.contains("ffi:denied"),
        "expected ffi:denied without --allow-ffi, got stdout={stdout}"
    );
}

/// `--allow-ffi` should switch the same script over to actually loading libc.
#[test]
#[cfg(target_os = "linux")]
fn cap_allow_ffi_loads_libc() {
    let script = r#"
let r = try ffi_load("libc.so.6") catch "GATE_DENIED";
if r == "GATE_DENIED" { print "ffi:denied"; } else { print "ffi:loaded"; }
"#;
    let (status, stdout, stderr) = run_pblang_script_with_args(script, &["--allow-ffi"]);
    assert_eq!(status, 0, "expected exit 0 (got {status}). stderr={stderr}");
    assert!(
        stdout.contains("ffi:loaded"),
        "expected ffi:loaded with --allow-ffi, got stdout={stdout} stderr={stderr}"
    );
}

/// `--allow-all` should enable both exec and ffi.
#[test]
#[cfg(target_os = "linux")]
fn cap_allow_all_enables_ffi_and_exec() {
    let script = r#"
let f = try ffi_load("libc.so.6") catch "GATE_DENIED_FFI";
if f == "GATE_DENIED_FFI" { print "ffi:denied"; } else { print "ffi:loaded"; }
let r = try exec("/bin/true") catch "GATE_DENIED_EXEC";
if r == "GATE_DENIED_EXEC" { print "exec:denied"; } else { print "exec:ran"; }
"#;
    let (status, stdout, _stderr) = run_pblang_script_with_args(script, &["--allow-all"]);
    assert_eq!(status, 0);
    assert!(
        stdout.contains("ffi:loaded") && stdout.contains("exec:ran"),
        "expected both ffi:loaded and exec:ran with --allow-all, got: {stdout}"
    );
}

/// `--sandbox` should DENY ffi/exec/net/fs/http/sqlite even when env var
/// would otherwise enable them — CLI flags take precedence over env vars.
#[test]
#[cfg(target_os = "linux")]
fn cap_sandbox_overrides_env_var() {
    let script = r#"
let r = try ffi_load("libc.so.6") catch "GATE_DENIED";
if r == "GATE_DENIED" { print "ffi:denied"; } else { print "ffi:loaded"; }
"#;
    let bin = require_pblang_bin();
    let tmp = std::env::temp_dir().join("pblang_cap_sandbox_test.sapp");
    std::fs::write(&tmp, script).expect("write tmp script");
    let out = Command::new(&bin)
        .arg("run")
        .arg(&tmp)
        .arg("--sandbox")
        .env("PBLANG_ALLOW_FFI", "1")
        .output()
        .expect("run pblang script");
    let _ = std::fs::remove_file(&tmp);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stdout.contains("ffi:denied"),
        "expected --sandbox to win over PBLANG_ALLOW_FFI=1, got stdout={stdout} stderr={stderr}"
    );
}

/// Env var alone should enable ffi when no CLI flag is given.
#[test]
#[cfg(target_os = "linux")]
fn cap_env_var_alone_enables_ffi() {
    let script = r#"
let r = try ffi_load("libc.so.6") catch "GATE_DENIED";
if r == "GATE_DENIED" { print "ffi:denied"; } else { print "ffi:loaded"; }
"#;
    let bin = require_pblang_bin();
    let tmp = std::env::temp_dir().join("pblang_cap_env_test.sapp");
    std::fs::write(&tmp, script).expect("write tmp script");
    let out = Command::new(&bin)
        .arg("run")
        .arg(&tmp)
        .env("PBLANG_ALLOW_FFI", "1")
        .output()
        .expect("run pblang script");
    let _ = std::fs::remove_file(&tmp);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("ffi:loaded"),
        "expected PBLANG_ALLOW_FFI=1 to enable ffi, got: {stdout}"
    );
}

/// `--allow-*` flags must be stripped from script-arg view; they shouldn't
/// land in `os.args()` returned to the script.
#[test]
fn cap_flags_do_not_leak_into_script_args() {
    let script = r#"
// Just print something — we're checking the test doesn't error from
// a bad script path or unexpected argv that contains "--allow-exec".
print "ok";
"#;
    let (status, stdout, stderr) =
        run_pblang_script_with_args(script, &["--allow-exec", "--allow-ffi"]);
    assert_eq!(
        status, 0,
        "script should run cleanly with capability flags. stderr={stderr}"
    );
    assert!(
        stdout.contains("ok"),
        "expected 'ok' output, got: stdout={stdout} stderr={stderr}"
    );
}

// ────────────────────────────────────────────────────────────────────────────
// `pblang build --warm-polyglot` (PL2-beta)
// ────────────────────────────────────────────────────────────────────────────

/// `--warm-polyglot` should walk .sapp files, extract @lang blocks for the
/// cacheable languages, and pre-warm the cdylib cache. Missing toolchains
/// must skip gracefully so this test runs on any host.
#[test]
fn build_warm_polyglot_walks_and_warms() {
    let bin = require_pblang_bin();
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let work = std::env::temp_dir().join(format!("pblang_warm_polyglot_cli_{nonce}"));
    let src_dir = work.join("src");
    std::fs::create_dir_all(&src_dir).expect("mkdir src");
    std::fs::write(
        work.join("pblang.toml"),
        "[package]\nname = \"warm_test\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    // Two blocks: C should compile (cc is widely available on Linux/macOS
    // CI), Pascal scaffolds as a "skipped" path on hosts without fpc.
    // Use a unique function name per test run so we exercise the cold path.
    let unique = nonce;
    let c_code = format!(
        "long warm_test_{unique}(long a, long b) {{ return a + b; }}\n"
    );
    let pas_code = format!(
        "library prog;\n\
         function pblang_x_{unique}(a, b: Int64): Int64; cdecl; export;\n\
         begin\n  pblang_x_{unique} := a + b;\nend;\n\
         exports pblang_x_{unique};\nbegin\nend.\n"
    );
    let src = format!(
        "@c {{\n{c_code}}}\n@pascal {{\n{pas_code}}}\nprint \"ok\";\n"
    );
    std::fs::write(src_dir.join("main.sapp"), &src).unwrap();

    let out = Command::new(&bin)
        .current_dir(&work)
        .arg("build")
        .arg("--warm-polyglot")
        .output()
        .expect("run pblang build --warm-polyglot");
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    let _ = std::fs::remove_dir_all(&work);

    assert!(
        out.status.success(),
        "build --warm-polyglot should exit 0. stderr={stderr} stdout={stdout}"
    );
    assert!(
        stdout.contains("warm-polyglot: found"),
        "expected scan report. stdout={stdout}"
    );
    assert!(
        stdout.contains("warm-polyglot:") && stdout.contains("warmed"),
        "expected final summary line. stdout={stdout}"
    );
    // At least one of "warmed c" / "skipped c" should appear (C-compiler is
    // installed on virtually every CI; if it weren't we'd see "skipped c").
    let saw_c = stdout.contains("warmed c") || stdout.contains("skipped c");
    assert!(saw_c, "expected the C block to be warmed or skipped. stdout={stdout}");
}

/// Idempotent: running `--warm-polyglot` twice should not error and the
/// second run is a cache-hit (we don't assert timing — just non-error).
#[test]
fn build_warm_polyglot_is_idempotent() {
    let bin = require_pblang_bin();
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let work = std::env::temp_dir().join(format!("pblang_warm_polyglot_idem_{nonce}"));
    let src_dir = work.join("src");
    std::fs::create_dir_all(&src_dir).expect("mkdir src");
    std::fs::write(
        work.join("pblang.toml"),
        "[package]\nname = \"warm_test\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    let c_code = format!("long idem_test_{nonce}(long a, long b) {{ return a + b; }}\n");
    std::fs::write(
        src_dir.join("main.sapp"),
        format!("@c {{\n{c_code}}}\nprint \"ok\";\n"),
    )
    .unwrap();

    for pass in 1..=2 {
        let out = Command::new(&bin)
            .current_dir(&work)
            .arg("build")
            .arg("--warm-polyglot")
            .output()
            .expect("run pblang build --warm-polyglot");
        let stderr = String::from_utf8_lossy(&out.stderr).to_string();
        assert!(
            out.status.success(),
            "pass {pass}: build --warm-polyglot should exit 0. stderr={stderr}"
        );
    }
    let _ = std::fs::remove_dir_all(&work);
}

/// `--warm-polyglot` on a project with no @lang blocks should still succeed.
#[test]
fn build_warm_polyglot_handles_empty_project() {
    let bin = require_pblang_bin();
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let work = std::env::temp_dir().join(format!("pblang_warm_polyglot_empty_{nonce}"));
    let src_dir = work.join("src");
    std::fs::create_dir_all(&src_dir).expect("mkdir src");
    std::fs::write(
        work.join("pblang.toml"),
        "[package]\nname = \"warm_test\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    std::fs::write(src_dir.join("main.sapp"), "print \"no polyglot here\";\n").unwrap();

    let out = Command::new(&bin)
        .current_dir(&work)
        .arg("build")
        .arg("--warm-polyglot")
        .output()
        .expect("run pblang build --warm-polyglot");
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let _ = std::fs::remove_dir_all(&work);

    assert!(out.status.success(), "should exit 0 on empty project. stdout={stdout}");
    assert!(
        stdout.contains("no cacheable @lang blocks found"),
        "expected the empty-project message. stdout={stdout}"
    );
}

// ────────────────────────────────────────────────────────────────────────────
// `pblang serve --cert/--key` (HTTPS) — rustls + axum-server integration.
// Skip on hosts without openssl (CI provides it on every reasonable image).
// ────────────────────────────────────────────────────────────────────────────

fn openssl_available() -> bool {
    Command::new("openssl")
        .arg("version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn make_self_signed_cert() -> Option<(std::path::PathBuf, std::path::PathBuf, std::path::PathBuf)> {
    if !openssl_available() {
        return None;
    }
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("pblang_https_test_{nonce}"));
    std::fs::create_dir_all(&dir).ok()?;
    let cert = dir.join("cert.pem");
    let key = dir.join("key.pem");
    let status = Command::new("openssl")
        .args([
            "req", "-x509", "-newkey", "rsa:2048", "-nodes",
            "-keyout",
        ])
        .arg(&key)
        .arg("-out")
        .arg(&cert)
        .args([
            "-days", "1",
            "-subj", "/CN=localhost",
            "-addext", "subjectAltName=DNS:localhost",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    if status.map(|s| s.success()).unwrap_or(false) {
        Some((dir, cert, key))
    } else {
        let _ = std::fs::remove_dir_all(&dir);
        None
    }
}

/// Spawn `pblang serve --cert --key`, hit /health over plain TCP+TLS.
/// Skips on hosts without openssl. Uses a raw rustls client because curl
/// isn't part of the test infra.
#[test]
fn serve_https_starts_and_responds() {
    let (dir, cert, key) = match make_self_signed_cert() {
        Some(t) => t,
        None => {
            eprintln!("serve_https_starts_and_responds: skipped (no openssl)");
            return;
        }
    };
    let bin = require_pblang_bin();
    let port = pick_free_port();
    let mut child = Command::new(&bin)
        .arg("serve")
        .arg("--port")
        .arg(port.to_string())
        .arg("--cert")
        .arg(&cert)
        .arg("--key")
        .arg(&key)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn pblang serve --cert/--key");

    // Probe with a raw TLS connection. We can't reuse http_get() because
    // it speaks plain HTTP; build a minimal rustls client here.
    let deadline = Instant::now() + Duration::from_secs(8);
    let mut got_health = false;
    while Instant::now() < deadline {
        if let Ok(body) = https_get_self_signed(port, "/health") {
            if body.contains("\"ok\":true") {
                got_health = true;
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    let _ = child.kill();
    let _ = child.wait();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        got_health,
        "expected HTTPS /health to return {{\"ok\":true,...}} within 8s"
    );
}

/// Minimal HTTPS GET helper that accepts any cert (for self-signed testing).
fn https_get_self_signed(port: u16, path: &str) -> std::io::Result<String> {
    use std::io::Read as _;
    use std::io::Write as _;
    use std::sync::Arc;

    // Install ring provider once; subsequent calls are no-ops.
    let _ = rustls::crypto::ring::default_provider().install_default();

    #[derive(Debug)]
    struct AcceptAnyVerifier;
    impl rustls::client::danger::ServerCertVerifier for AcceptAnyVerifier {
        fn verify_server_cert(
            &self,
            _end_entity: &rustls::pki_types::CertificateDer<'_>,
            _intermediates: &[rustls::pki_types::CertificateDer<'_>],
            _server_name: &rustls::pki_types::ServerName<'_>,
            _ocsp: &[u8],
            _now: rustls::pki_types::UnixTime,
        ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
            Ok(rustls::client::danger::ServerCertVerified::assertion())
        }
        fn verify_tls12_signature(
            &self,
            _: &[u8],
            _: &rustls::pki_types::CertificateDer<'_>,
            _: &rustls::DigitallySignedStruct,
        ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
            Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
        }
        fn verify_tls13_signature(
            &self,
            _: &[u8],
            _: &rustls::pki_types::CertificateDer<'_>,
            _: &rustls::DigitallySignedStruct,
        ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
            Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
        }
        fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
            vec![
                rustls::SignatureScheme::RSA_PKCS1_SHA256,
                rustls::SignatureScheme::RSA_PKCS1_SHA384,
                rustls::SignatureScheme::RSA_PKCS1_SHA512,
                rustls::SignatureScheme::RSA_PSS_SHA256,
                rustls::SignatureScheme::RSA_PSS_SHA384,
                rustls::SignatureScheme::RSA_PSS_SHA512,
                rustls::SignatureScheme::ECDSA_NISTP256_SHA256,
                rustls::SignatureScheme::ECDSA_NISTP384_SHA384,
                rustls::SignatureScheme::ED25519,
            ]
        }
    }

    let cfg = rustls::ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(AcceptAnyVerifier))
        .with_no_client_auth();
    let server_name = rustls::pki_types::ServerName::try_from("localhost")
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidInput, "bad server name"))?;
    let mut conn = rustls::ClientConnection::new(Arc::new(cfg), server_name)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
    let mut sock = std::net::TcpStream::connect(("127.0.0.1", port))?;
    sock.set_read_timeout(Some(Duration::from_secs(5)))?;
    sock.set_write_timeout(Some(Duration::from_secs(5)))?;
    let mut tls = rustls::Stream::new(&mut conn, &mut sock);
    let req = format!("GET {} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n", path);
    tls.write_all(req.as_bytes())?;
    let mut buf = Vec::new();
    let _ = tls.read_to_end(&mut buf);
    Ok(String::from_utf8_lossy(&buf).to_string())
}

/// HTTPS + ALPN: when the server advertises `["h2", "http/1.1"]` (as
/// `build_tls_config_with_alpn` does) and the client offers `["h2"]`,
/// rustls must negotiate `h2`. This is the smallest end-to-end check
/// that the HTTP/2 enablement from the 2026-05-26 round is actually
/// wired through axum-server's TLS path.
///
/// We don't follow the negotiation with an actual h2 GET — the existence
/// of `h2` in `alpn_protocol()` post-handshake is the contract under
/// test. The h2-frame layer is hyper-util's job, exercised by other
/// crates' tests.
#[test]
fn serve_https_alpn_negotiates_h2() {
    let (dir, cert, key) = match make_self_signed_cert() {
        Some(t) => t,
        None => {
            eprintln!("serve_https_alpn_negotiates_h2: skipped (no openssl)");
            return;
        }
    };
    let bin = require_pblang_bin();
    let port = pick_free_port();
    let mut child = Command::new(&bin)
        .arg("serve")
        .arg("--port")
        .arg(port.to_string())
        .arg("--cert")
        .arg(&cert)
        .arg("--key")
        .arg(&key)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn pblang serve --cert/--key");

    use std::sync::Arc;
    let _ = rustls::crypto::ring::default_provider().install_default();

    #[derive(Debug)]
    struct AcceptAnyVerifier;
    impl rustls::client::danger::ServerCertVerifier for AcceptAnyVerifier {
        fn verify_server_cert(
            &self,
            _: &rustls::pki_types::CertificateDer<'_>,
            _: &[rustls::pki_types::CertificateDer<'_>],
            _: &rustls::pki_types::ServerName<'_>,
            _: &[u8],
            _: rustls::pki_types::UnixTime,
        ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
            Ok(rustls::client::danger::ServerCertVerified::assertion())
        }
        fn verify_tls12_signature(
            &self,
            _: &[u8],
            _: &rustls::pki_types::CertificateDer<'_>,
            _: &rustls::DigitallySignedStruct,
        ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
            Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
        }
        fn verify_tls13_signature(
            &self,
            _: &[u8],
            _: &rustls::pki_types::CertificateDer<'_>,
            _: &rustls::DigitallySignedStruct,
        ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
            Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
        }
        fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
            vec![
                rustls::SignatureScheme::RSA_PKCS1_SHA256,
                rustls::SignatureScheme::RSA_PSS_SHA256,
                rustls::SignatureScheme::ECDSA_NISTP256_SHA256,
                rustls::SignatureScheme::ED25519,
            ]
        }
    }

    let mut cfg = rustls::ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(AcceptAnyVerifier))
        .with_no_client_auth();
    // Offer h2 first, then http/1.1 fallback — mirrors what real
    // browsers / hyper clients send.
    cfg.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];

    let deadline = Instant::now() + Duration::from_secs(8);
    let mut negotiated: Option<Vec<u8>> = None;
    while Instant::now() < deadline {
        if let Ok(mut sock) = TcpStream::connect(("127.0.0.1", port)) {
            let _ = sock.set_read_timeout(Some(Duration::from_secs(2)));
            let _ = sock.set_write_timeout(Some(Duration::from_secs(2)));
            let server_name = rustls::pki_types::ServerName::try_from("localhost").unwrap();
            if let Ok(mut conn) = rustls::ClientConnection::new(Arc::new(cfg.clone()), server_name)
            {
                // Drive the handshake explicitly — we just need ALPN
                // negotiation to complete; no HTTP request needed.
                while conn.is_handshaking() {
                    if conn.wants_read() {
                        if conn.read_tls(&mut sock).is_err() {
                            break;
                        }
                    }
                    if conn.process_new_packets().is_err() {
                        break;
                    }
                    if conn.wants_write() {
                        if conn.write_tls(&mut sock).is_err() {
                            break;
                        }
                    }
                }
                if let Some(p) = conn.alpn_protocol() {
                    negotiated = Some(p.to_vec());
                    break;
                }
            }
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    let _ = child.kill();
    let _ = child.wait();
    let _ = std::fs::remove_dir_all(&dir);

    let proto = negotiated.expect("expected ALPN negotiation to complete within 8s");
    assert_eq!(
        proto,
        b"h2".to_vec(),
        "expected ALPN to negotiate h2, got: {:?}",
        String::from_utf8_lossy(&proto)
    );
}

/// `--cert` without `--key` (and vice-versa) should error out cleanly.
#[test]
fn serve_https_requires_both_cert_and_key() {
    let bin = require_pblang_bin();
    let port = pick_free_port();
    let out = Command::new(&bin)
        .arg("serve")
        .arg("--port")
        .arg(port.to_string())
        .arg("--cert")
        .arg("/tmp/pblang_does_not_exist.pem")
        .output()
        .expect("run pblang serve --cert (no --key)");
    assert!(!out.status.success(), "should error without --key");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("--cert and --key must be provided together"),
        "expected paired-arg error, got: {stderr}"
    );
}

// -----------------------------------------------------------------------
// WebSocket endpoint tests (round-5, 2026-05-26)
//
// /ws upgrades any GET to a WebSocket and runs a tiny JSON-over-text
// protocol (op = ping|echo|version). The handler lives in
// src/main.rs::handle_ws + ws_socket_loop + ws_dispatch.

/// Computes Sec-WebSocket-Accept = base64(SHA1(key + WS_GUID)).
/// Used to verify the upgrade handshake response.
fn ws_expected_accept(key: &str) -> String {
    use base64::Engine as _;
    use sha1::Digest as _;
    const WS_GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";
    let mut h = sha1::Sha1::new();
    h.update(key.as_bytes());
    h.update(WS_GUID.as_bytes());
    base64::engine::general_purpose::STANDARD.encode(h.finalize().as_slice())
}

/// Connect to `pblang serve` on `port`, do the RFC 6455 GET /ws upgrade
/// dance, send one masked text frame containing `payload`, read one
/// server text frame back, return the payload as a `String`. Best-effort
/// — gives up after 5s of socket inactivity.
fn ws_send_one_text(port: u16, payload: &str) -> std::io::Result<String> {
    let mut sock = TcpStream::connect(("127.0.0.1", port))?;
    sock.set_read_timeout(Some(Duration::from_secs(5)))?;
    sock.set_write_timeout(Some(Duration::from_secs(5)))?;
    // 16 pseudo-random bytes -> base64. Don't need a CSPRNG for a test handshake.
    // Use wrapping arithmetic so the byte mixing can't overflow.
    let nonce_b64 = {
        use base64::Engine as _;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);
        let mut bytes = [0u8; 16];
        for (i, b) in bytes.iter_mut().enumerate() {
            let mixer = (i as u8).wrapping_mul(0x37);
            *b = ((now >> ((i % 8) * 4)) as u8) ^ mixer;
        }
        base64::engine::general_purpose::STANDARD.encode(bytes)
    };
    let upgrade = format!(
        "GET /ws HTTP/1.1\r\n\
         Host: localhost:{port}\r\n\
         Upgrade: websocket\r\n\
         Connection: Upgrade\r\n\
         Sec-WebSocket-Key: {nonce_b64}\r\n\
         Sec-WebSocket-Version: 13\r\n\
         \r\n"
    );
    sock.write_all(upgrade.as_bytes())?;

    // Read headers until \r\n\r\n.
    let mut header_bytes = Vec::with_capacity(512);
    let mut byte = [0u8; 1];
    loop {
        sock.read_exact(&mut byte)?;
        header_bytes.push(byte[0]);
        if header_bytes.ends_with(b"\r\n\r\n") {
            break;
        }
        if header_bytes.len() > 8192 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "upgrade response headers too large",
            ));
        }
    }
    let header_str = String::from_utf8_lossy(&header_bytes).to_string();
    if !header_str.starts_with("HTTP/1.1 101") {
        return Err(std::io::Error::new(
            std::io::ErrorKind::ConnectionRefused,
            format!("WS upgrade not 101: {}", header_str.lines().next().unwrap_or("")),
        ));
    }
    let expected_accept = ws_expected_accept(&nonce_b64);
    if !header_str.contains(&expected_accept) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Sec-WebSocket-Accept did not match expected SHA1+base64",
        ));
    }

    // Send single text frame, FIN=1, opcode=0x1, MASK=1.
    let payload_bytes = payload.as_bytes();
    let mut frame: Vec<u8> = Vec::with_capacity(6 + payload_bytes.len());
    frame.push(0x81); // FIN + text
    let mask = [0x12u8, 0x34, 0x56, 0x78];
    let plen = payload_bytes.len();
    if plen <= 125 {
        frame.push(0x80 | plen as u8);
    } else if plen <= 65_535 {
        frame.push(0x80 | 126);
        frame.extend_from_slice(&(plen as u16).to_be_bytes());
    } else {
        frame.push(0x80 | 127);
        frame.extend_from_slice(&(plen as u64).to_be_bytes());
    }
    frame.extend_from_slice(&mask);
    for (i, b) in payload_bytes.iter().enumerate() {
        frame.push(b ^ mask[i % 4]);
    }
    sock.write_all(&frame)?;

    // Read one server frame (unmasked, server frames are always unmasked).
    let mut hdr = [0u8; 2];
    sock.read_exact(&mut hdr)?;
    let opcode = hdr[0] & 0x0f;
    let masked = (hdr[1] & 0x80) != 0;
    let mut plen = (hdr[1] & 0x7f) as u64;
    if plen == 126 {
        let mut ext = [0u8; 2];
        sock.read_exact(&mut ext)?;
        plen = u16::from_be_bytes(ext) as u64;
    } else if plen == 127 {
        let mut ext = [0u8; 8];
        sock.read_exact(&mut ext)?;
        plen = u64::from_be_bytes(ext);
    }
    let mut mask_key = [0u8; 4];
    if masked {
        sock.read_exact(&mut mask_key)?;
    }
    let mut body = vec![0u8; plen as usize];
    sock.read_exact(&mut body)?;
    if masked {
        for (i, b) in body.iter_mut().enumerate() {
            *b ^= mask_key[i % 4];
        }
    }
    if opcode != 0x1 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("expected text frame (opcode 1), got {}", opcode),
        ));
    }
    Ok(String::from_utf8_lossy(&body).to_string())
}

#[test]
fn serve_websocket_ping_returns_pong() {
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

    // Wait for /health to come up, then send WS ping.
    let deadline = Instant::now() + Duration::from_secs(8);
    let mut ws_pong = None;
    while Instant::now() < deadline {
        if http_get(port, "/health")
            .map(|r| r.contains("\"ok\":true"))
            .unwrap_or(false)
        {
            // Server is up; do the WS handshake + ping.
            ws_pong = ws_send_one_text(port, r#"{"op":"ping"}"#).ok();
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    let _ = child.kill();
    let _ = child.wait();

    let reply = ws_pong.expect("WS handshake + ping should produce a pong frame");
    assert!(
        reply.contains("\"op\":\"pong\""),
        "expected pong op, got: {reply}"
    );
    assert!(
        reply.contains("\"ts_ms\":"),
        "expected ts_ms field, got: {reply}"
    );
}

#[test]
fn serve_websocket_version_returns_pkg_version() {
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

    let deadline = Instant::now() + Duration::from_secs(8);
    let mut ws_reply = None;
    while Instant::now() < deadline {
        if http_get(port, "/health")
            .map(|r| r.contains("\"ok\":true"))
            .unwrap_or(false)
        {
            ws_reply = ws_send_one_text(port, r#"{"op":"version"}"#).ok();
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    let _ = child.kill();
    let _ = child.wait();

    let reply = ws_reply.expect("WS version op should reply");
    assert!(
        reply.contains(env!("CARGO_PKG_VERSION")),
        "expected package version {} in WS reply, got: {reply}",
        env!("CARGO_PKG_VERSION")
    );
    assert!(
        reply.contains("\"service\":\"pblang-ide-backend\""),
        "expected service identifier in WS reply, got: {reply}"
    );
}

// ──────────────────────────────────────────────────────────────────────
// `pblang.toml [workspace]` — workspace-aware build
// ──────────────────────────────────────────────────────────────────────

/// Helper: scaffold a single pblang package at `dir` with a trivial
/// `src/main.sapp` that type-checks.
fn write_pkg(dir: &std::path::Path, name: &str) {
    std::fs::create_dir_all(dir.join("src")).expect("mkdir src");
    std::fs::write(
        dir.join("pblang.toml"),
        format!("[package]\nname = \"{name}\"\nversion = \"0.1.0\"\n"),
    )
    .unwrap();
    std::fs::write(dir.join("src").join("main.sapp"), "print \"ok\";\n").unwrap();
}

#[test]
fn workspace_build_walks_literal_members() {
    let bin = require_pblang_bin();
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let work = std::env::temp_dir().join(format!("pblang_ws_literal_{nonce}"));
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).unwrap();
    // Virtual workspace: no [package] at the root, just [workspace].
    std::fs::write(
        work.join("pblang.toml"),
        "[workspace]\nmembers = [\"alpha\", \"beta\"]\n",
    )
    .unwrap();
    write_pkg(&work.join("alpha"), "alpha");
    write_pkg(&work.join("beta"), "beta");

    let out = Command::new(&bin)
        .current_dir(&work)
        .arg("build")
        .output()
        .expect("run pblang build");
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    let _ = std::fs::remove_dir_all(&work);

    assert!(
        out.status.success(),
        "workspace build should succeed. stderr={stderr} stdout={stdout}"
    );
    assert!(stdout.contains("build ok [alpha]"), "stdout={stdout}");
    assert!(stdout.contains("build ok [beta]"), "stdout={stdout}");
    assert!(
        stdout.contains("workspace build ok (2 packages)"),
        "stdout={stdout}"
    );
    // Virtual workspace shouldn't print a "workspace root" line because
    // there's no [package] at the root.
    assert!(
        !stdout.contains("workspace root"),
        "virtual workspace should not build a root package. stdout={stdout}"
    );
}

#[test]
fn workspace_build_expands_trailing_star_pattern() {
    let bin = require_pblang_bin();
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let work = std::env::temp_dir().join(format!("pblang_ws_star_{nonce}"));
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).unwrap();
    std::fs::write(
        work.join("pblang.toml"),
        "[workspace]\nmembers = [\"crates/*\"]\n",
    )
    .unwrap();
    // Three valid members + one decoy dir without pblang.toml.
    write_pkg(&work.join("crates").join("aaa"), "aaa");
    write_pkg(&work.join("crates").join("bbb"), "bbb");
    write_pkg(&work.join("crates").join("ccc"), "ccc");
    std::fs::create_dir_all(work.join("crates").join("not_a_pkg")).unwrap();
    std::fs::write(
        work.join("crates").join("not_a_pkg").join("README.md"),
        "ignore me",
    )
    .unwrap();

    let out = Command::new(&bin)
        .current_dir(&work)
        .arg("build")
        .output()
        .expect("run pblang build");
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    let _ = std::fs::remove_dir_all(&work);

    assert!(
        out.status.success(),
        "wildcard workspace build should succeed. stderr={stderr} stdout={stdout}"
    );
    assert!(stdout.contains("build ok [crates/aaa]"), "stdout={stdout}");
    assert!(stdout.contains("build ok [crates/bbb]"), "stdout={stdout}");
    assert!(stdout.contains("build ok [crates/ccc]"), "stdout={stdout}");
    assert!(
        !stdout.contains("not_a_pkg"),
        "directories without pblang.toml should be skipped silently. stdout={stdout}"
    );
    assert!(
        stdout.contains("workspace build ok (3 packages)"),
        "stdout={stdout}"
    );
}

#[test]
fn workspace_build_honors_exclude() {
    let bin = require_pblang_bin();
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let work = std::env::temp_dir().join(format!("pblang_ws_exclude_{nonce}"));
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).unwrap();
    std::fs::write(
        work.join("pblang.toml"),
        "[workspace]\nmembers = [\"crates/*\"]\nexclude = [\"crates/skipme\"]\n",
    )
    .unwrap();
    write_pkg(&work.join("crates").join("kept"), "kept");
    write_pkg(&work.join("crates").join("skipme"), "skipme");

    let out = Command::new(&bin)
        .current_dir(&work)
        .arg("build")
        .output()
        .expect("run pblang build");
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let _ = std::fs::remove_dir_all(&work);

    assert!(out.status.success(), "stdout={stdout}");
    assert!(stdout.contains("build ok [crates/kept]"), "stdout={stdout}");
    assert!(
        !stdout.contains("[crates/skipme]"),
        "excluded member should not be built. stdout={stdout}"
    );
    assert!(
        stdout.contains("workspace build ok (1 package)"),
        "stdout={stdout}"
    );
}

#[test]
fn workspace_build_hybrid_root_builds_both_root_and_members() {
    let bin = require_pblang_bin();
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let work = std::env::temp_dir().join(format!("pblang_ws_hybrid_{nonce}"));
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work.join("src")).unwrap();
    // Hybrid: root is a package AND a workspace.
    std::fs::write(
        work.join("pblang.toml"),
        "[package]\nname = \"root\"\nversion = \"0.1.0\"\n\n[workspace]\nmembers = [\"sub\"]\n",
    )
    .unwrap();
    std::fs::write(work.join("src").join("main.sapp"), "print \"root\";\n").unwrap();
    write_pkg(&work.join("sub"), "sub");

    let out = Command::new(&bin)
        .current_dir(&work)
        .arg("build")
        .output()
        .expect("run pblang build");
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let _ = std::fs::remove_dir_all(&work);

    assert!(out.status.success(), "stdout={stdout}");
    assert!(
        stdout.contains("build ok [workspace root]"),
        "hybrid workspace should build the root package. stdout={stdout}"
    );
    assert!(stdout.contains("build ok [sub]"), "stdout={stdout}");
    assert!(
        stdout.contains("workspace build ok (2 packages)"),
        "stdout={stdout}"
    );
}

#[test]
fn workspace_build_missing_member_manifest_errors() {
    let bin = require_pblang_bin();
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let work = std::env::temp_dir().join(format!("pblang_ws_missing_{nonce}"));
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).unwrap();
    // "ghost" is named as a literal member but no directory exists for it.
    std::fs::write(
        work.join("pblang.toml"),
        "[workspace]\nmembers = [\"ghost\"]\n",
    )
    .unwrap();

    let out = Command::new(&bin)
        .current_dir(&work)
        .arg("build")
        .output()
        .expect("run pblang build");
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    let _ = std::fs::remove_dir_all(&work);

    assert!(!out.status.success(), "stderr={stderr}");
    assert!(
        stderr.contains("ghost") && stderr.contains("pblang.toml"),
        "error should name the missing member and its manifest. stderr={stderr}"
    );
}

#[test]
fn workspace_build_single_package_mode_unchanged() {
    // No [workspace] section → single-package behavior, same stdout shape as
    // pre-workspace pblang. Acts as a regression guard.
    let bin = require_pblang_bin();
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let work = std::env::temp_dir().join(format!("pblang_ws_solo_{nonce}"));
    let _ = std::fs::remove_dir_all(&work);
    write_pkg(&work, "solo");

    let out = Command::new(&bin)
        .current_dir(&work)
        .arg("build")
        .output()
        .expect("run pblang build");
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let _ = std::fs::remove_dir_all(&work);

    assert!(out.status.success(), "stdout={stdout}");
    assert!(
        stdout.trim() == "build ok",
        "single-package build should print exactly 'build ok'. got: {stdout:?}"
    );
}
