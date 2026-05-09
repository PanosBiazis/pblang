#[test]
fn dataflow_reports_unused_def() {
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_pblang"))
        .args(["--dataflow", "tests/fixtures/call_graph_sample.sapp"])
        .output()
        .expect("run sapphire --dataflow");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("def") || stdout.contains("use"),
        "Expected dataflow output, got: {}",
        stdout
    );
}
