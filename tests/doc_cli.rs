#[test]
fn doc_cli_renders_polyglot_attrs_in_text_and_html() {
    let base = std::env::temp_dir().join("pblang_doc_cli_polyglot");
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();
    let src_path = base.join("module.sapp");
    let html_dir = base.join("doc-out");
    std::fs::write(
        &src_path,
        r#"
        @rust mylib(version="1.0", artifact="cdylib") {
            pub fn answer() -> i32 { 42 }
        }

        fn answer() { return 42; }
        export answer;
        "#,
    )
    .unwrap();

    let bin = env!("CARGO_BIN_EXE_pblang");
    let text = std::process::Command::new(bin)
        .arg("--doc")
        .arg(&src_path)
        .output()
        .expect("run sapphire --doc");
    assert!(
        text.status.success(),
        "sapphire --doc should succeed: {}",
        String::from_utf8_lossy(&text.stderr)
    );
    let stdout = String::from_utf8_lossy(&text.stdout);
    assert!(
        stdout.contains("artifact=cdylib") && stdout.contains("version=1.0"),
        "text docs should render polyglot attrs: {}",
        stdout
    );
    assert!(
        stdout.contains("code lines: 3"),
        "text docs should render polyglot code line count: {}",
        stdout
    );

    let html = std::process::Command::new(bin)
        .arg("--doc")
        .arg(&src_path)
        .arg("--html")
        .arg(&html_dir)
        .output()
        .expect("run sapphire --doc --html");
    assert!(
        html.status.success(),
        "sapphire --doc --html should succeed: {}",
        String::from_utf8_lossy(&html.stderr)
    );
    let html_out = std::fs::read_to_string(html_dir.join("index.html")).unwrap();
    assert!(
        html_out.contains("artifact=cdylib") && html_out.contains("version=1.0"),
        "html docs should render polyglot attrs: {}",
        html_out
    );
    assert!(
        html_out.contains("code lines: 3"),
        "html docs should render polyglot code line count: {}",
        html_out
    );

    let _ = std::fs::remove_dir_all(&base);
}
