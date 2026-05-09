// Tests for `sapphire lint`: unused variable and unused import warnings.

#[test]
fn lint_reports_unused_variable_and_unused_import() {
    let bin = env!("CARGO_BIN_EXE_pblang");
    let fixture =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/lint_unused.sapp");
    assert!(
        fixture.exists(),
        "fixture tests/fixtures/lint_unused.sapp must exist"
    );
    let out = std::process::Command::new(bin)
        .arg("lint")
        .arg(&fixture)
        .output()
        .expect("run sapphire lint");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("unused variable: `unused_var`"),
        "stderr should warn unused variable unused_var: {}",
        stderr
    );
    assert!(
        stderr.contains("unused import: `unused_imp`"),
        "stderr should warn unused import unused_imp: {}",
        stderr
    );
    assert!(
        !stderr.contains("unused variable: `used_var`")
            && !stderr.contains("unused import: `used_var`"),
        "used_var must not be reported as unused: {}",
        stderr
    );
}

#[test]
fn lint_reports_shadowing() {
    let bin = env!("CARGO_BIN_EXE_pblang");
    let fixture =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/lint_shadow.sapp");
    assert!(
        fixture.exists(),
        "fixture tests/fixtures/lint_shadow.sapp must exist"
    );
    let out = std::process::Command::new(bin)
        .arg("lint")
        .arg(&fixture)
        .output()
        .expect("run sapphire lint");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("shadows outer binding"),
        "stderr should warn about shadowing: {}",
        stderr
    );
    assert!(
        stderr.contains("`x`"),
        "stderr should mention shadowed name x: {}",
        stderr
    );
}

#[test]
fn error_output_includes_file_and_line() {
    let bin = env!("CARGO_BIN_EXE_pblang");
    let fixture =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/error_location.sapp");
    assert!(
        fixture.exists(),
        "fixture tests/fixtures/error_location.sapp must exist"
    );
    let out = std::process::Command::new(bin)
        .arg("run")
        .arg(&fixture)
        .output()
        .expect("run sapphire run");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(!out.status.success(), "expected parse failure");
    assert!(
        stderr.contains("error_location.sapp"),
        "expected filename in error output, got:\n{}",
        stderr
    );
    assert!(
        stderr.contains("error[") && stderr.contains("]:"),
        "expected error[file:line:col]: format in output, got:\n{}",
        stderr
    );
}
