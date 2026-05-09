use std::process::Command;

mod common;

#[test]
fn symbols_lists_function_names() {
    let bin = common::pblang_bin();
    let out = Command::new(bin)
        .args(["--symbols", "tests/fixtures/call_graph_sample.sapp"])
        .output()
        .expect("run sapphire --symbols");
    assert!(
        out.status.success(),
        "sapphire --symbols should exit 0, stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("fn") && stdout.contains("arity"),
        "Expected symbol table output, got: {}",
        stdout
    );
}
