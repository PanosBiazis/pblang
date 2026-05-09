//! Regression tests for `sapphire fmt`: trim, operator spacing, indent, final newline.

#[test]
fn fmt_adds_operator_spacing() {
    let bin = env!("CARGO_BIN_EXE_pblang");
    let temp = std::env::temp_dir().join("pblang_fmt_test.sap");
    std::fs::write(&temp, "print 1+2;").expect("write");
    let status = std::process::Command::new(bin)
        .arg("fmt")
        .arg(&temp)
        .status()
        .expect("run sapphire fmt");
    assert!(status.success(), "sapphire fmt should exit 0");
    let out = std::fs::read_to_string(&temp).expect("read");
    assert!(
        out.contains("1 + 2"),
        "fmt should add spaces around +: got {:?}",
        out
    );
    let _ = std::fs::remove_file(&temp);
}

#[test]
fn fmt_preserves_string_content() {
    let bin = env!("CARGO_BIN_EXE_pblang");
    let temp = std::env::temp_dir().join("pblang_fmt_string.sap");
    // String with "1+2" should not get spaces inside the quotes
    std::fs::write(&temp, r#"print "1+2";"#).expect("write");
    let status = std::process::Command::new(bin)
        .arg("fmt")
        .arg(&temp)
        .status()
        .expect("run sapphire fmt");
    assert!(status.success());
    let out = std::fs::read_to_string(&temp).expect("read");
    assert!(
        out.contains(r#""1+2""#),
        "fmt should not change content inside strings: got {:?}",
        out
    );
    let _ = std::fs::remove_file(&temp);
}

#[test]
fn fmt_max_line_length_wraps_long_lines() {
    let bin = env!("CARGO_BIN_EXE_pblang");
    let temp = std::env::temp_dir().join("pblang_fmt_wrap.sap");
    // Line long enough to wrap at 40 chars (break at last space before 40)
    std::fs::write(&temp, "let x = 1 + 2 + 3 + 4 + 5 + 6 + 7 + 8 + 9 + 10;").expect("write");
    let status = std::process::Command::new(bin)
        .arg("fmt")
        .arg("--max-line-length=40")
        .arg(&temp)
        .status()
        .expect("run sapphire fmt");
    assert!(status.success());
    let out = std::fs::read_to_string(&temp).expect("read");
    assert!(
        out.contains('\n') && out.lines().count() >= 2,
        "fmt with --max-line-length should wrap into multiple lines: got {:?}",
        out
    );
    let _ = std::fs::remove_file(&temp);
}

#[test]
fn fmt_adds_space_after_comma() {
    let bin = env!("CARGO_BIN_EXE_pblang");
    let temp = std::env::temp_dir().join("pblang_fmt_comma.sap");
    std::fs::write(&temp, "print f(1,2);").expect("write");
    let status = std::process::Command::new(bin)
        .arg("fmt")
        .arg(&temp)
        .status()
        .expect("run sapphire fmt");
    assert!(status.success());
    let out = std::fs::read_to_string(&temp).expect("read");
    assert!(
        out.contains("1, 2"),
        "fmt should add space after comma in f(1,2): got {:?}",
        out
    );
    let _ = std::fs::remove_file(&temp);
}

#[test]
fn fmt_preserves_comma_inside_string() {
    let bin = env!("CARGO_BIN_EXE_pblang");
    let temp = std::env::temp_dir().join("pblang_fmt_comma_str.sap");
    std::fs::write(&temp, r#"print "a,b";"#).expect("write");
    let status = std::process::Command::new(bin)
        .arg("fmt")
        .arg(&temp)
        .status()
        .expect("run sapphire fmt");
    assert!(status.success());
    let out = std::fs::read_to_string(&temp).expect("read");
    assert!(
        out.contains(r#""a,b""#),
        "fmt should not change comma inside string: got {:?}",
        out
    );
    let _ = std::fs::remove_file(&temp);
}

#[test]
fn fmt_inserts_blank_line_between_toplevel_fns() {
    let bin = env!("CARGO_BIN_EXE_pblang");
    let temp = std::env::temp_dir().join("pblang_fmt_toplevel_fn_blank.sap");
    std::fs::write(
        &temp,
        "fn first() {\n    print 1;\n}\nfn second() {\n    print 2;\n}\n",
    )
    .expect("write");
    let status = std::process::Command::new(bin)
        .arg("fmt")
        .arg(&temp)
        .status()
        .expect("run sapphire fmt");
    assert!(status.success());
    let out = std::fs::read_to_string(&temp).expect("read");
    assert!(
        out.contains("}\n\nfn second"),
        "fmt should insert a blank line between top-level functions: got {:?}",
        out
    );
    let _ = std::fs::remove_file(&temp);
}
