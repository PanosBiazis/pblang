// R1 – Regression test for sapphire --dump-bytecode output shape (constants + bytecode).

#[test]
fn dump_bytecode_output_has_constants_and_bytecode_sections() {
    let base = std::env::temp_dir().join("pblang_dump_bytecode");
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();
    let src_path = base.join("script.sapp");
    std::fs::write(&src_path, "let x = 1; print x;\n").unwrap();

    let bin = env!("CARGO_BIN_EXE_pblang");
    let out = std::process::Command::new(bin)
        .arg("--dump-bytecode")
        .arg(&src_path)
        .output()
        .expect("run sapphire --dump-bytecode");
    assert!(
        out.status.success(),
        "sapphire --dump-bytecode should succeed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("=== constants"),
        "disassembly should include constants section: {}",
        stdout
    );
    assert!(
        stdout.contains("=== bytecode ==="),
        "disassembly should include bytecode section: {}",
        stdout
    );
    assert!(
        stdout.contains("OpPrint")
            || stdout.contains("OpConstant")
            || stdout.contains("OpDefineGlobal"),
        "disassembly should include at least one opcode: {}",
        stdout
    );

    let _ = std::fs::remove_dir_all(&base);
}
