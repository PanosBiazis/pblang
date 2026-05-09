// T1 – Threads MVP: spawn/join tests (VM-per-thread, transferable values only).

use pblang_compiler::compile;
use pblang_core::parse;
use pblang_vm::vm::{RuntimeError, VM};
use pblang_vm::{StdlibConfig, Value, VmLimits};
use std::rc::Rc;

mod common;
use common::run_collect;

fn run_collect_with_limits(limits: VmLimits, src: &str) -> Result<Vec<Value>, RuntimeError> {
    let stmts = parse(src).expect("parse failed");
    let mut chunk = compile(stmts);
    chunk.source_name = Some(Rc::new("<test>".to_string()));
    chunk.source = Some(Rc::new(src.to_string()));
    let mut vm = VM::with_stdlib(StdlibConfig::default());
    vm.set_limits(Some(limits));
    vm.run_collect(chunk)
}

#[test]
fn spawn_join_returns_value() {
    let out = run_collect(
        r#"
        let h = spawn(fn() { return 42; });
        print join(h);
        "#,
    );
    assert_eq!(out, vec![Value::Number(42.0)]);
}

#[test]
fn spawn_join_propagates_error() {
    let err = common::run_err(
        r#"
        let h = spawn(fn() { let x = nil; return x.y; });
        print join(h);
        "#,
    );
    assert!(
        err.message.contains("nil")
            || err.message.contains("attribute")
            || err.message.contains("y"),
        "expected error about nil/attribute, got: {}",
        err.message
    );
}

#[test]
fn spawned_vm_inherits_limits() {
    let limits = VmLimits {
        max_instructions: Some(100),
        max_stack_depth: None,
        max_memory_bytes: None,
        io_timeout_ms: None,
    };
    let result = run_collect_with_limits(
        limits,
        r#"
        let h = spawn(fn() {
            let n = 0;
            for i 1000 { n = n + 1; }
            return n;
        });
        print join(h);
        "#,
    );
    let err = result.expect_err("expected instruction limit exceeded");
    assert!(
        err.message.contains("instruction limit") || err.message.contains("limit exceeded"),
        "expected limit error, got: {}",
        err.message
    );
}

#[test]
fn not_transferable_handles_error() {
    let err = common::run_err(
        r#"
        let x = clock;
        let f = fn() { return x; };
        let h = spawn(f);
        print join(h);
        "#,
    );
    let ok = err.message.contains("transferable") || err.message.contains("undefined global 'x'");
    assert!(
        ok,
        "expected non-transferable value error, got: {}",
        err.message
    );
}

#[test]
fn spawn_not_allowed_when_sandboxed() {
    let stmts =
        parse(r#" let h = spawn(fn() { return 1; }); print join(h); "#).expect("parse failed");
    let mut chunk = compile(stmts);
    chunk.source_name = Some(Rc::new("<test>".to_string()));
    chunk.source = Some(Rc::new(
        "let h = spawn(fn() { return 1; }); print join(h);".to_string(),
    ));
    let mut config = StdlibConfig::default();
    config.spawn = false;
    let mut vm = VM::with_stdlib(config);
    let result = vm.run_collect(chunk);
    let err = result.expect_err("expected spawn not allowed");
    assert!(
        err.message.contains("spawn not allowed"),
        "expected 'spawn not allowed', got: {}",
        err.message
    );
}

#[test]
fn channel_send_recv_works() {
    let out = run_collect(
        r#"
        let joined = nil;
        let got = nil;
        {
            let ch = channel();
            let tx = ch.sender;
            let rx = ch.receiver;
            let h = spawn(fn() {
                send(tx, map{ok: true, items: [1, 2, 3]});
                return 9;
            });
            joined = join(h);
            got = recv(rx);
        }
        print joined;
        print got;
        "#,
    );
    assert_eq!(out.len(), 2);
    assert_eq!(out[0], Value::Number(9.0));
    match &out[1] {
        Value::Map(m) => {
            let m = m.borrow();
            assert_eq!(m.get("ok"), Some(&Value::Bool(true)));
            match m.get("items") {
                Some(Value::Array(a)) => {
                    let a = a.borrow();
                    assert_eq!(
                        a.as_slice(),
                        &[Value::Number(1.0), Value::Number(2.0), Value::Number(3.0)]
                    );
                }
                other => panic!("expected items array, got {:?}", other),
            }
        }
        other => panic!("expected map result, got {:?}", other),
    }
}

#[test]
fn recv_on_closed_channel_returns_nil_or_closed() {
    let out = run_collect(
        r#"
        let first = nil;
        let second = nil;
        {
            let ch = channel();
            let tx = ch.sender;
            let rx = ch.receiver;
            let h = spawn(fn() {
                send(tx, 42);
                return nil;
            });
            join(h);
            first = recv(rx);
            second = recv(rx);
        }
        print first;
        print second;
        "#,
    );
    assert_eq!(out, vec![Value::Number(42.0), Value::Nil]);
}

#[test]
fn channel_send_recv_is_deterministic() {
    let out = run_collect(
        r#"
        let got = nil;
        {
            let ch = channel();
            let tx = ch.sender;
            let rx = ch.receiver;
            let h = spawn(fn() {
                send(tx, ["alpha", "beta"]);
                return nil;
            });
            join(h);
            got = recv(rx);
        }
        print got;
        "#,
    );
    assert_eq!(out.len(), 1);
    match &out[0] {
        Value::Array(a) => {
            let a = a.borrow();
            assert_eq!(a.len(), 2);
            assert_eq!(a[0].to_string(), "alpha");
            assert_eq!(a[1].to_string(), "beta");
        }
        other => panic!("expected array result, got {:?}", other),
    }
}

#[test]
fn channel_rejects_non_transferable_values() {
    let err = common::run_err(
        r#"
        let ch = channel();
        send(ch.sender, clock);
        "#,
    );
    assert!(
        err.message.contains("transferable"),
        "expected transferable error, got: {}",
        err.message
    );
}

#[test]
fn two_spawns_two_channels_deterministic() {
    let out = run_collect(
        r#"
        let a = nil;
        let b = nil;
        {
            let ch1 = channel();
            let ch2 = channel();
            let h1 = spawn(fn() {
                send(ch1.sender, 100);
                return nil;
            });
            let h2 = spawn(fn() {
                send(ch2.sender, 200);
                return nil;
            });
            join(h1);
            join(h2);
            a = recv(ch1.receiver);
            b = recv(ch2.receiver);
        }
        print a;
        print b;
        "#,
    );
    assert_eq!(out, vec![Value::Number(100.0), Value::Number(200.0)]);
}
