#[allow(dead_code)]
mod common;

use common::run_err;

#[test]
fn str_repeat_rejects_oversized_result() {
    let err = run_err(r#"print str_repeat("x", 20000000);"#);
    assert!(
        err.message.contains("maximum"),
        "unexpected error: {}",
        err.message
    );
}
