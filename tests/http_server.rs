use pblang_compiler::compile;
use pblang_core::parse;
use pblang_vm::{StdlibConfig, VM};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::Duration;

fn http_test_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

#[test]
#[ignore] // Network/timing sensitive; enable with cargo test -- --ignored when needed
fn http_server_serve_once_handles_one_request() {
    let _guard = http_test_lock().lock().unwrap();
    // Script writes bound address to a temp file before blocking on accept.
    let port_file = std::env::temp_dir().join("pblang_http_test_port");
    let port_file_str = port_file.to_string_lossy().to_string();
    let _ = std::fs::remove_file(&port_file);
    // SAFETY: test-only; set before spawning the server thread.
    #[allow(unused_unsafe)]
    unsafe { std::env::set_var("PBLANG_TEST_PORT_FILE", &port_file_str) };

    let server = thread::spawn(move || {
        let src = r#"
            import "stdlib:http_server.sapp";
            fn handler(req) {
                return map{ status: 200, body: "hello" };
            }
            let l = tcp_listen("127.0.0.1:0");
            let actual = tcp_local_addr(l);
            fs_write(env_get("PBLANG_TEST_PORT_FILE"), actual);
            let s = tcp_accept(l);
            let raw = tcp_read_until(s, "\r\n\r\n", 65536);
            let req = http_parse_request(raw);
            let out = handler(req);
            let resp = http_response(200, out.body);
            tcp_write(s, resp);
            tcp_close(s);
            tcp_close(l);
        "#;

        let stmts = parse(src).expect("parse failed");
        let chunk = compile(stmts).expect("compile failed");
        let mut vm = VM::with_stdlib(StdlibConfig {
            exec: true,
            ..StdlibConfig::default()
        });
        vm.run_collect(chunk).expect("vm run failed");
    });

    for _ in 0..50 {
        thread::sleep(Duration::from_millis(100));
        if port_file.exists() {
            break;
        }
    }
    let addr = std::fs::read_to_string(&port_file).expect("read port file");
    let mut s = TcpStream::connect(addr.trim()).expect("connect failed");
    s.write_all(b"GET / HTTP/1.1\r\nHost: x\r\n\r\n").unwrap();
    s.set_read_timeout(Some(Duration::from_secs(3))).ok();
    let mut buf = String::new();
    s.read_to_string(&mut buf).unwrap();

    assert!(buf.contains("HTTP/1.1 200"));
    assert!(buf.ends_with("hello") || buf.contains("\r\n\r\nhello"));

    let _ = server.join();
}

#[test]
#[ignore] // Network/timing sensitive; enable with cargo test --test http_server -- --ignored
fn http_server_read_request_reads_fixed_length_body() {
    let _guard = http_test_lock().lock().unwrap();
    let port_file = std::env::temp_dir().join("pblang_http_body_test_port");
    let port_file_str = port_file.to_string_lossy().to_string();
    let _ = std::fs::remove_file(&port_file);
    // SAFETY: test-only; set before spawning the server thread.
    #[allow(unused_unsafe)]
    unsafe { std::env::set_var("PBLANG_TEST_PORT_FILE", &port_file_str) };

    let server = thread::spawn(move || {
        let src = r#"
            import "stdlib:http_server.sapp";
            let l = tcp_listen("127.0.0.1:0");
            let actual = tcp_local_addr(l);
            fs_write(env_get("PBLANG_TEST_PORT_FILE"), actual);
            let s = tcp_accept(l);
            let req = http_server.read_request(s);
            let resp = http_response(200, req.body);
            tcp_write(s, resp);
            tcp_close(s);
            tcp_close(l);
        "#;

        let stmts = parse(src).expect("parse failed");
        let chunk = compile(stmts).expect("compile failed");
        let mut vm = VM::with_stdlib(StdlibConfig {
            exec: true,
            ..StdlibConfig::default()
        });
        vm.run_collect(chunk).expect("vm run failed");
    });

    for _ in 0..50 {
        thread::sleep(Duration::from_millis(100));
        if port_file.exists() {
            break;
        }
    }
    let addr = std::fs::read_to_string(&port_file).expect("read port file");
    let mut s = TcpStream::connect(addr.trim()).expect("connect failed");
    s.write_all(b"POST / HTTP/1.1\r\nHost: x\r\nContent-Length: 5\r\n\r\nhello")
        .unwrap();
    s.set_read_timeout(Some(Duration::from_secs(3))).ok();
    let mut buf = String::new();
    s.read_to_string(&mut buf).unwrap();

    assert!(buf.contains("HTTP/1.1 200"));
    assert!(buf.ends_with("hello") || buf.contains("\r\n\r\nhello"));

    let _ = server.join();
}

#[test]
#[ignore] // Network/timing sensitive; enable with cargo test --test http_server -- --ignored
fn http_server_serve_once_limited_rejects_oversized_body() {
    let _guard = http_test_lock().lock().unwrap();
    let port_file = std::env::temp_dir().join("pblang_http_limit_test_port");
    let port_file_str = port_file.to_string_lossy().to_string();
    let _ = std::fs::remove_file(&port_file);
    // SAFETY: test-only; set before spawning the server thread.
    #[allow(unused_unsafe)]
    unsafe { std::env::set_var("PBLANG_TEST_PORT_FILE", &port_file_str) };

    let server = thread::spawn(move || {
        let src = r#"
            import "stdlib:http_server.sapp";
            fn result_to_response(result) {
                if result.ok {
                    return map{ status: 200, body: result.req.body };
                }
                return result.response;
            }
            let l = tcp_listen("127.0.0.1:0");
            let actual = tcp_local_addr(l);
            fs_write(env_get("PBLANG_TEST_PORT_FILE"), actual);
            let s = tcp_accept(l);
            let result = http_server.read_request_limited(s, 4);
            let out = result_to_response(result);
            let resp = http_server.to_http_response(out);
            tcp_write(s, resp);
            tcp_close(s);
            tcp_close(l);
        "#;

        let stmts = parse(src).expect("parse failed");
        let chunk = compile(stmts).expect("compile failed");
        let mut vm = VM::with_stdlib(StdlibConfig {
            exec: true,
            ..StdlibConfig::default()
        });
        vm.run_collect(chunk).expect("vm run failed");
    });

    for _ in 0..50 {
        thread::sleep(Duration::from_millis(100));
        if port_file.exists() {
            break;
        }
    }
    let addr = std::fs::read_to_string(&port_file).expect("read port file");
    let mut s = TcpStream::connect(addr.trim()).expect("connect failed");
    s.write_all(b"POST / HTTP/1.1\r\nHost: x\r\nContent-Length: 5\r\n\r\nhello")
        .unwrap();
    s.set_read_timeout(Some(Duration::from_secs(3))).ok();
    let mut buf = String::new();
    s.read_to_string(&mut buf).unwrap();

    assert!(buf.contains("HTTP/1.1 413"));
    assert!(buf.contains("payload too large"));

    let _ = server.join();
}
