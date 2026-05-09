// Tests for `sapphire --call-graph` (R2-alpha).

use std::process::Command;

fn run_call_graph(path: &str) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_pblang"))
        .args(["--call-graph", path])
        .output()
        .expect("failed to run sapphire --call-graph");
    String::from_utf8_lossy(&out.stdout).into_owned()
}

#[test]
fn call_graph_detects_greet_called_from_run() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/call_graph_sample.sapp");
    let path_str = path.to_string_lossy();
    let out = run_call_graph(&path_str);
    assert!(
        out.contains("run -> greet"),
        "expected 'run -> greet' edge, got:\n{out}"
    );
}

#[test]
fn call_graph_detects_main_calls_run() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/call_graph_sample.sapp");
    let path_str = path.to_string_lossy();
    let out = run_call_graph(&path_str);
    assert!(
        out.contains("<main> -> run"),
        "expected '<main> -> run' edge, got:\n{out}"
    );
}

#[test]
fn call_graph_greet_calls_print() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/call_graph_sample.sapp");
    let path_str = path.to_string_lossy();
    let out = run_call_graph(&path_str);
    assert!(
        out.contains("greet -> print"),
        "expected 'greet -> print' edge, got:\n{out}"
    );
}
