//! Integration test for the `.pbck` bytecode container round-trip:
//!   pblang emit-pbck file.sapp -o file.pbck   (compile -> lossless bytecode)
//!   pblang run file.pbck                       (load + execute, no re-parsing)
//! The `.pbck` execution must be byte-identical to running the source — this is
//! the "regenerate the runnable program from pblang bytecode without the Rust
//! front-end" path. Exercises functions, recursion, closures, maps, loops, and
//! a Unicode identifier so nested-function constants + the full opcode set are
//! all covered.

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_pblang")
}

#[test]
fn pbck_cli_rejects_trailing_bytes_before_execution() {
    let base = tempfile::tempdir().unwrap();
    let path = base.path().join("trailing.pbck");
    let mut bytes = pblang_ir::pbck_serialize(&pblang_ir::Chunk::default());
    bytes.push(0);
    std::fs::write(&path, bytes).unwrap();
    let output = std::process::Command::new(bin()).arg("run").arg(path).output().unwrap();
    assert_eq!(output.status.code(), Some(3));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8(output.stderr).unwrap().contains("pbck: trailing bytes"));
}

#[test]
fn pbck_cli_rejects_oversized_file_before_deserialization() {
    let base = tempfile::tempdir().unwrap();
    let path = base.path().join("oversized.pbck");
    // Sparse trusted fixture; no hostile bytecode is constructed or executed.
    let file = std::fs::File::create(&path).unwrap();
    file.set_len(pblang_ir::PBCK_MAX_BYTES as u64 + 1).unwrap();
    drop(file);
    let output = std::process::Command::new(bin()).arg("run").arg(path).output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8(output.stderr).unwrap().contains("pbck: artifact byte limit exceeded"));
}

#[test]
fn pbck_emit_and_run_matches_source() {
    // Each invocation owns its fixtures, including when tests run concurrently.
    let base = tempfile::tempdir().unwrap();
    let src_path = base.path().join("prog.sapp");
    let pbck_path = base.path().join("prog.pbck");

    let program = r#"
        fn fib(n) { if n < 2 { return n; } return fib(n-1) + fib(n-2); }
        fn 変数(z) { return z * z; }
        fn make_adder(x) { return fn(y) { return x + y; }; }
        let add10 = make_adder(10);
        let m = map{ a: 1, b: 2 };
        let total = 0;
        for i in 1 to 5 { total = total + i; }
        print fib(10);
        print add10(7);
        print m.a + m.b;
        print total;
        print 変数(9);
    "#;
    std::fs::write(&src_path, program).unwrap();

    // 1. Run from source.
    let src_out = std::process::Command::new(bin())
        .arg("run")
        .arg(&src_path)
        .arg("--allow-all")
        .output()
        .expect("run source");
    assert!(
        src_out.status.success(),
        "source run failed: {}",
        String::from_utf8_lossy(&src_out.stderr)
    );

    // 2. Emit a .pbck container.
    let emit = std::process::Command::new(bin())
        .arg("emit-pbck")
        .arg(&src_path)
        .arg("-o")
        .arg(&pbck_path)
        .output()
        .expect("emit-pbck");
    assert!(
        emit.status.success(),
        "emit-pbck failed: {}",
        String::from_utf8_lossy(&emit.stderr)
    );
    assert!(pbck_path.exists(), "emit-pbck must produce the .pbck file");

    // 3. Run from .pbck.
    let pbck_out = std::process::Command::new(bin())
        .arg("run")
        .arg(&pbck_path)
        .arg("--allow-all")
        .output()
        .expect("run pbck");
    assert!(
        pbck_out.status.success(),
        "pbck run failed: {}",
        String::from_utf8_lossy(&pbck_out.stderr)
    );

    // 4. Output must be byte-identical.
    assert_eq!(
        src_out.stdout,
        pbck_out.stdout,
        "running the .pbck must produce identical output to running the source"
    );
    assert_eq!(
        String::from_utf8_lossy(&src_out.stdout).trim(),
        "55\n17\n3\n15\n81",
        "expected program output"
    );
}
