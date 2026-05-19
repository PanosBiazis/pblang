//! Smoke tests for OS-signal builtins added in 0.5.
//!
//! We avoid actually raising a real signal in the test (that would deliver
//! SIGINT to the test runner). Instead we check that:
//!   * `os_signal_trap` accepts canonical signal names and returns an ok map
//!   * `os_signal_handler_get` returns nil for unregistered signals
//!   * `os_signal_pending` / `os_signal_poll` return zero-valued shapes when
//!     no signal has been delivered yet
//!
//! These exercise the full FFI / registry round-trip without race conditions.

#[allow(dead_code)]
mod common;

use common::run_collect;
use pblang_vm::{Obj, Value};

fn value_str(v: &Value) -> &str {
    match v {
        Value::Obj(o) => match o.as_ref() {
            Obj::String(s) => s.as_str(),
        },
        _ => panic!("expected string, got {:?}", v),
    }
}

#[test]
fn signal_trap_registers_sigint() {
    let out = run_collect(
        r#"
        let r = os_signal_trap("SIGINT", fn() { return 42; });
        print r["ok"];
        print r["signal"];
        "#,
    );
    assert_eq!(out.len(), 2);
    assert!(matches!(out[0], Value::Bool(true)));
    assert_eq!(value_str(&out[1]), "SIGINT");
}

#[test]
fn signal_handler_get_returns_nil_for_unregistered() {
    let out = run_collect(
        r#"
        // Pick a benign signal we don't trap in this test:
        print os_signal_handler_get("SIGUSR2");
        "#,
    );
    assert_eq!(out.len(), 1);
    assert!(matches!(out[0], Value::Nil));
}

#[test]
fn signal_pending_is_zero_when_no_signal() {
    let out = run_collect(
        r#"
        os_signal_trap("SIGUSR1", fn() { return 1; });
        print os_signal_pending("SIGUSR1");
        "#,
    );
    assert_eq!(out.len(), 1);
    if let Value::Number(n) = &out[0] {
        assert_eq!(*n as i64, 0);
    } else {
        panic!("expected number, got {:?}", out[0]);
    }
}
