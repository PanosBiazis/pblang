// tests/annotate.rs — R2-beta: --annotate output contains source line and opcode
use std::process::Command;

#[test]
fn annotate_output_contains_line_number_and_opcode() {
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture = manifest_dir.join("tests/fixtures/call_graph_sample.sapp");
    if !fixture.exists() {
        return;
    }
    let out = Command::new(env!("CARGO_BIN_EXE_pblang"))
        .arg("--annotate")
        .arg(&fixture)
        .output()
        .expect("failed to run sapphire --annotate");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("-- line "),
        "expected '-- line N' in annotate output, got:\n{}",
        stdout
    );
    assert!(
        stdout.contains("Op")
            && (stdout.contains("OpConstant")
                || stdout.contains("OpCall")
                || stdout.contains("OpClosure")
                || stdout.contains("OpGetGlobal")),
        "expected an opcode mnemonic (Op*) in annotate output, got:\n{}",
        stdout
    );
}
