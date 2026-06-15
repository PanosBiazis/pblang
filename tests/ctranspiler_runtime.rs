use std::process::Command;

#[test]
fn ctranspiler_runtime_json_parse_and_stringify_work() {
    if !pblang_ctranspiler::is_c_compiler_available() {
        eprintln!("skipping: no C compiler available");
        return;
    }

    let src = r#"
        fn pick_first(obj) {
            let picked = obj.a[0];
            return picked;
        }

        let v = json_parse("{\"a\":[1,2],\"b\":true,\"s\":\"x\\ny\"}");
        print v.a[1];
        print v.b;
        print v.s;
        print pick_first(v);
        print json_stringify(map{ ok: true, nums: [1, 2], none: nil });
    "#;

    let out = pblang_ctranspiler::transpile_to_c(src).expect("transpile pblang to C");
    assert!(
        !out.c_source.contains("return PB_NIL; /* stub */"),
        "embedded runtime should not contain the old json_parse stub"
    );
    assert!(
        !out.c_source.contains("return PB_STR(\"{}\"); /* stub */"),
        "embedded runtime should not contain the old json_stringify stub"
    );

    let dir = tempfile::tempdir().expect("tempdir");
    let exe_base = dir.path().join("json_runtime");
    let exe_base_s = exe_base.to_string_lossy().to_string();
    pblang_ctranspiler::compile_from_c(&out.c_source, &exe_base_s).expect("compile generated C");

    let exe = if cfg!(target_os = "windows") {
        exe_base.with_extension("exe")
    } else {
        exe_base
    };
    let run = Command::new(&exe)
        .output()
        .expect("run generated native executable");
    assert!(
        run.status.success(),
        "generated executable failed: stderr={}",
        String::from_utf8_lossy(&run.stderr)
    );
    let stdout = String::from_utf8_lossy(&run.stdout);
    assert!(
        stdout.contains("2\n"),
        "array value should print, stdout={stdout:?}"
    );
    assert!(
        stdout.contains("true\n"),
        "bool value should print, stdout={stdout:?}"
    );
    assert!(
        stdout.contains("x\\ny\n") || stdout.contains("x\ny\n"),
        "escaped string should print, stdout={stdout:?}"
    );
    assert!(
        stdout.contains("1\n"),
        "function parameter/local variable access should print, stdout={stdout:?}"
    );
    assert!(
        stdout.contains(r#"{"ok":true,"nums":[1,2],"none":null}"#),
        "json_stringify should emit structured JSON, stdout={stdout:?}"
    );
}
