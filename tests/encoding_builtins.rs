//! Smoke tests for the native hex/url encoding builtins.

#[allow(dead_code)]
mod common;

use common::run_collect;
use pblang_vm::{Obj, Value};

fn value_str(v: &Value) -> String {
    match v {
        Value::Obj(o) => match o.as_ref() {
            Obj::String(s) => s.clone(),
            other => panic!("expected string, got {:?}", other),
        },
        _ => panic!("expected string, got {:?}", v),
    }
}

#[test]
fn hex_encode_roundtrip() {
    let out = run_collect(
        r#"
        let enc = hex_encode("Hello");
        print enc;
        print hex_decode(enc);
        "#,
    );
    assert_eq!(value_str(&out[0]), "48656c6c6f");
    assert_eq!(value_str(&out[1]), "Hello");
}

#[test]
fn hex_decode_uppercase_works() {
    let out = run_collect(
        r#"
        print hex_decode("48656C6C6F");
        "#,
    );
    assert_eq!(value_str(&out[0]), "Hello");
}

#[test]
fn hex_decode_invalid_errors() {
    let result = std::panic::catch_unwind(|| {
        run_collect(
            r#"
            print hex_decode("xyz");
            "#,
        );
    });
    assert!(
        result.is_err(),
        "expected a runtime panic for bad hex input"
    );
}

#[test]
fn url_encode_unreserved_untouched() {
    let out = run_collect(
        r#"
        print url_encode("Hello-World_._~");
        "#,
    );
    assert_eq!(value_str(&out[0]), "Hello-World_._~");
}

#[test]
fn url_encode_reserved_escaped() {
    let out = run_collect(
        r#"
        print url_encode("hello world!");
        "#,
    );
    // space -> %20 (RFC 3986), '!' -> %21
    assert_eq!(value_str(&out[0]), "hello%20world%21");
}

#[test]
fn url_decode_plus_to_space() {
    let out = run_collect(
        r#"
        print url_decode("a+b%20c");
        "#,
    );
    assert_eq!(value_str(&out[0]), "a b c");
}

#[test]
fn url_decode_roundtrip() {
    let out = run_collect(
        r#"
        let s = "hello world & friends!";
        print url_decode(url_encode(s));
        "#,
    );
    assert_eq!(value_str(&out[0]), "hello world & friends!");
}
