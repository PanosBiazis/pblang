//! Regression: serializing a cyclic value with `json_stringify` must surface a
//! catchable runtime error, not overflow the native stack and abort the whole
//! process (which is uncatchable from pblang `try`/`catch`).

#[allow(dead_code)]
mod common;

use common::run_err;

#[test]
fn json_stringify_on_a_cyclic_array_errors_instead_of_aborting() {
    // `push(a, a)` makes the array contain itself. The test reaching this
    // assertion at all proves the runtime returned an Err rather than dying
    // on a stack overflow.
    let err = run_err(
        r#"
        let a = [];
        push(a, a);
        print json_stringify(a);
        "#,
    );
    let m = err.message.to_lowercase();
    assert!(
        m.contains("nested") || m.contains("cyclic"),
        "expected a depth/cycle error from json_stringify, got: {}",
        err.message
    );
}
