use pblang_vm::Value;

mod common;
use common::{run_collect, run_err};

#[test]
fn import_executes_module_code_and_defines_globals() {
    let dir = std::env::temp_dir().join("pblang_tests");
    let _ = std::fs::create_dir_all(&dir);
    let module_path = dir.join("mod1.sapp");

    std::fs::write(
        &module_path,
        r#"
            fn inc() { return 41 + 1; }
            export inc;
        "#,
    )
    .expect("write module file");

    let module_path_str = module_path.to_string_lossy().replace('\\', "/");
    let src = format!(
        r#"
            import "{module_path_str}";
            print mod1.inc();
        "#
    );

    let out = run_collect(&src);
    assert_eq!(out, vec![Value::Number(42.0)]);
}

#[test]
fn import_does_not_pollute_parent_globals() {
    let dir = std::env::temp_dir().join("pblang_tests");
    let _ = std::fs::create_dir_all(&dir);
    let module_path = dir.join("mod2.sapp");

    std::fs::write(
        &module_path,
        r#"
            let x = 123;
            export x;
        "#,
    )
    .expect("write module file");

    let module_path_str = module_path.to_string_lossy().replace('\\', "/");
    let src = format!(
        r#"
            import "{module_path_str}";
            print x;
        "#
    );

    let err = run_err(&src);
    assert!(
        err.message.contains("undefined global"),
        "unexpected error: {err:?}"
    );
}

#[test]
fn fs_read_and_fs_write_work() {
    let dir = std::env::temp_dir().join("pblang_tests");
    let _ = std::fs::create_dir_all(&dir);
    let file_path = dir.join("hello.txt");
    let file_path_str = file_path.to_string_lossy().replace('\\', "/");

    let src = format!(
        r#"
            fs_write("{file_path_str}", "abc");
            print len(fs_read("{file_path_str}"));
        "#
    );

    let out = run_collect(&src);
    assert_eq!(out, vec![Value::Number(3.0)]);
}

#[test]
fn fs_exists_and_listdir_work() {
    let dir = std::env::temp_dir()
        .join("pblang_tests")
        .join("fs_listdir");
    let _ = std::fs::create_dir_all(&dir);
    let a = dir.join("a.txt");
    let b = dir.join("b.txt");
    let _ = std::fs::write(&a, "a");
    let _ = std::fs::write(&b, "b");

    let dir_str = dir.to_string_lossy().replace('\\', "/");
    let src = format!(
        r#"
            print fs_exists("{dir_str}");
            print len(fs_listdir("{dir_str}"));
        "#
    );
    let out = run_collect(&src);
    assert_eq!(out, vec![Value::Bool(true), Value::Number(2.0)]);
}

#[test]
fn module_only_exposes_exports() {
    let dir = std::env::temp_dir().join("pblang_tests");
    let _ = std::fs::create_dir_all(&dir);
    let module_path = dir.join("exports_only.sapp");

    std::fs::write(
        &module_path,
        r#"
            let hidden = 1;
            let shown = 2;
            export shown;
        "#,
    )
    .expect("write module file");

    let module_path_str = module_path.to_string_lossy().replace('\\', "/");
    let src = format!(
        r#"
            import "{module_path_str}";
            print exports_only.shown;
            print exports_only.hidden;
        "#
    );

    let err = run_err(&src);
    assert!(
        err.message.contains("undefined export"),
        "unexpected error: {err:?}"
    );
}

#[test]
fn package_style_import_resolves_from_sapphire_toml() {
    let src = r#"
        import "stdlib:quantum.sapp";
        let c = quantum.circuit(1);
        print quantum.emit_qasm(c);
    "#;
    let out = run_collect(src);
    assert_eq!(out.len(), 1);
}

#[test]
fn import_builtin_caches_by_path_identity() {
    let dir = std::env::temp_dir().join("pblang_tests");
    let _ = std::fs::create_dir_all(&dir);
    let module_path = dir.join("cache_me.sapp");

    std::fs::write(
        &module_path,
        r#"
            fn forty_two() { return 42; }
            export forty_two;
        "#,
    )
    .expect("write module file");

    let module_path_str = module_path.to_string_lossy().replace('\\', "/");
    let src = format!(
        r#"
            let a = import("{module_path_str}");
            let b = import("{module_path_str}");
            print a == b;
            print a.forty_two();
        "#
    );

    let out = run_collect(&src);
    assert_eq!(out, vec![Value::Bool(true), Value::Number(42.0)]);
}

#[test]
fn import_cycle_is_detected_and_errors() {
    let dir = std::env::temp_dir().join("pblang_tests");
    let _ = std::fs::create_dir_all(&dir);
    let a_path = dir.join("a_cycle.sapp");
    let b_path = dir.join("b_cycle.sapp");

    let a_str = a_path.to_string_lossy().replace('\\', "/");
    let b_str = b_path.to_string_lossy().replace('\\', "/");

    std::fs::write(
        &a_path,
        format!(
            r#"
            import "{b_str}";
            fn a() {{ return 1; }}
            "#
        ),
    )
    .expect("write module A");

    std::fs::write(
        &b_path,
        format!(
            r#"
            import "{a_str}";
            fn b() {{ return 2; }}
            "#
        ),
    )
    .expect("write module B");

    let host_src = format!(
        r#"
            let m = import("{a_str}");
            print m.a();
        "#
    );

    let err = run_err(&host_src);
    assert!(
        err.message.contains("import cycle detected"),
        "unexpected error: {err:?}"
    );
    assert!(err.span.is_some(), "expected span to be present: {err:?}");
}

#[test]
fn quantum_stdlib_emits_qasm() {
    let src = r#"
        import "stdlib:quantum.sapp";
        let c = quantum.circuit(2);
        let c = quantum.h(c, 0);
        let c = quantum.cx(c, 0, 1);
        print quantum.emit_qasm(c);
    "#;

    let out = run_collect(src);
    assert_eq!(out.len(), 1);
    let s = out[0].as_str().expect("expected string output");
    assert!(s.contains("qreg q[2]"));
    assert!(s.contains("h q[0]"));
    assert!(s.contains("cx q[0],q[1]"));
}

#[test]
fn ai_plugin_example_builds_normalized_request_envelope() {
    let plugin_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("examples")
        .join("ai_plugin_minimal")
        .join("plugin")
        .join("lib.sapp");
    let plugin_path_str = plugin_path.to_string_lossy().replace('\\', "/");
    let src = format!(
        r#"
        let ai = import("{plugin_path_str}");
        let caps = ai.capabilities();
        let call = ai.invoke("hello", map{{ provider: "demo", model: "demo-llm", max_tokens: 64 }});
        print caps.chat;
        print call.request.provider;
        print call.request.options.max_tokens;
        print call.response.message;
        "#
    );
    let out = run_collect(&src);
    assert_eq!(
        out,
        vec![
            Value::Bool(true),
            Value::Obj(std::rc::Rc::new(pblang_vm::Obj::String(
                "demo".to_string()
            ))),
            Value::Number(64.0),
            Value::Obj(std::rc::Rc::new(pblang_vm::Obj::String(
                "stubbed by example bridge".to_string()
            ))),
        ]
    );
}

#[test]
fn ai_http_provider_contract_test() {
    let provider_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("examples")
        .join("ai_http_provider")
        .join("main.sapp");
    let provider_path_str = provider_path.to_string_lossy().replace('\\', "/");
    let src = format!(
        r#"
        let provider = import("{provider_path_str}");
        import "stdlib:json.sapp";
        print type(provider.build_request);
        print type(provider.call_provider);
        print type(provider.openai_chat);
        print type(provider.streaming_chat);
        print type(provider.provider_info);
        let info = provider.provider_info();
        print info.name;
        print info.supports_streaming;
        let body_str = provider.build_request("gpt-4o", []);
        let parsed = json.parse(body_str);
        print parsed.model;
        "#
    );
    let out = run_collect(&src);
    assert_eq!(out.len(), 8, "expected 8 printed values");
    for i in 0..5 {
        assert_eq!(
            out[i].as_str().unwrap(),
            "closure",
            "expected closure for export {}",
            i
        );
    }
    assert_eq!(out[5].as_str().unwrap(), "openai-compat");
    assert_eq!(out[6], Value::Bool(false));
    assert_eq!(out[7].as_str().unwrap(), "gpt-4o");
}

#[test]
fn quantum_bridge_example_builds_provider_job_envelope() {
    let plugin_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("examples")
        .join("quantum_bridge_minimal")
        .join("plugin")
        .join("lib.sapp");
    let plugin_path_str = plugin_path.to_string_lossy().replace('\\', "/");
    let src = format!(
        r#"
        import "stdlib:quantum.sapp";
        let bridge = import("{plugin_path_str}");
        let c = quantum.circuit(2);
        let c = quantum.h(c, 0);
        let job = bridge.build_job(c, map{{ provider: "sim", shots: 256 }});
        print job.provider;
        print job.shots;
        print job.metadata.qubits;
        print str_contains(job.qasm, "OPENQASM 2.0;");
        "#
    );
    let out = run_collect(&src);
    assert_eq!(
        out,
        vec![
            Value::Obj(std::rc::Rc::new(pblang_vm::Obj::String("sim".to_string()))),
            Value::Number(256.0),
            Value::Number(2.0),
            Value::Bool(true),
        ]
    );
}

#[test]
fn array_and_map_stdlib_helpers_work() {
    let src = r#"
        import "stdlib:array.sapp";
        import "stdlib:map.sapp";
        let m = map{ answer: 42 };
        print array.contains([1,2,3], 2);
        print array.index_of([1,2,3], 3);
        print map.get_or(m, "answer", 0);
        print map.get_or(m, "missing", 7);
        print array.first([10, 20, 30]);
        print array.first([]);
        print array.last([10, 20, 30]);
        print array.last([]);
        let vs = map.values(map{ a: 1, b: 2 });
        print len(vs);
        print vs[0] + vs[1];
        let s = array.slice([10, 20, 30, 40], 1, 3);
        print len(s);
        print s[0];
        print s[1];
        let r = array.reverse([1, 2, 3]);
        print len(r);
        print r[0];
        print r[2];
        let merged = map.merge(map{ a: 1, b: 2 }, map{ b: 20, c: 30 });
        print merged.a;
        print merged.b;
        print merged.c;
    "#;
    let out = run_collect(src);
    assert_eq!(
        out,
        vec![
            Value::Bool(true),
            Value::Number(2.0),
            Value::Number(42.0),
            Value::Number(7.0),
            Value::Number(10.0),
            Value::Nil,
            Value::Number(30.0),
            Value::Nil,
            Value::Number(2.0),
            Value::Number(3.0),
            Value::Number(2.0),
            Value::Number(20.0),
            Value::Number(30.0),
            Value::Number(3.0),
            Value::Number(3.0),
            Value::Number(1.0),
            Value::Number(1.0),
            Value::Number(20.0),
            Value::Number(30.0),
        ]
    );
}

#[test]
fn csv_stdlib_parse_and_stringify_work() {
    let src = r#"
        import "stdlib:csv.sapp";
        let text = "a,b\nc,d";
        let rows = csv.parse(text, ",", false);
        print len(rows);
        print rows[0][0];
        print rows[0][1];
        print len(rows[1]);
        print rows[1][0];
        print rows[1][1];
        let back = csv.stringify(rows);
        print back;
    "#;
    let out = run_collect(src);
    assert_eq!(out.len(), 7);
    assert_eq!(out[0], Value::Number(2.0));
    assert_eq!(out[1].as_str().unwrap(), "a");
    assert_eq!(out[2].as_str().unwrap(), "b");
    assert_eq!(out[3], Value::Number(2.0));
    assert_eq!(out[4].as_str().unwrap(), "c");
    assert_eq!(out[5].as_str().unwrap(), "d");
    // csv.stringify terminates every row with a newline; plain fields are
    // emitted unquoted (RFC 4180 — only quote when the field needs it).
    assert_eq!(out[6].as_str().unwrap(), "a,b\nc,d\n");
}

#[test]
fn math_stdlib_complex_and_algorithms_work() {
    let src = r#"
        import "stdlib:math.sapp";

        let c = math.complex.new(1, 2);
        let d = math.complex.new(3, 4);
        let e = math.complex.add(c, d);

        print e.re;
        print e.im;
        print math.algorithms.gcd(48, 18);
        print math.la.vec_dot([1,2,3], [4,5,6]);
    "#;

    let out = run_collect(src);
    assert_eq!(
        out,
        vec![
            Value::Number(4.0),
            Value::Number(6.0),
            Value::Number(6.0),
            Value::Number(32.0)
        ]
    );
}

/// Verifies the stdlib's new optional-parameter ergonomics: a wide set of
/// stdlib functions now declare default values in their signatures
/// (`fn f(a, b = 10)`) instead of relying on caller-must-pass-nil
/// workarounds. Callers can omit trailing args and get the documented
/// defaults — this test would have failed at "arity mismatch" before the
/// migration.
#[test]
fn stdlib_optional_params_omit_trailing_args() {
    let src = r#"
        import "stdlib:csv.sapp";
        import "stdlib:ml.sapp";
        import "stdlib:async_io.sapp";
        import "stdlib:class.sapp";
        import "stdlib:http_client.sapp";

        // csv.parse: text only, delimiter defaults to ",", has_header to false
        let rows = csv.parse("a,b\nc,d");
        print len(rows);
        print rows[0][1];

        // csv.stringify: data only, delimiter "," and headers nil by default
        print csv.stringify([["x","y"]]);

        // ml.train_test_split: data only, test_ratio defaults to 0.2
        let split = ml.train_test_split([1,2,3,4,5,6,7,8,9,10]);
        print len(split.train) + len(split.test);

        // ml.normalize_range: values only, range defaults to [0.0, 1.0]
        let nr = ml.normalize_range([0.0, 5.0, 10.0]);
        print nr[0];
        print nr[2];

        // async_io.create_semaphore: no args, max_concurrent defaults to 4
        let s = async_io.create_semaphore();
        print s._max;

        // class.define: only name required, fields and methods default to nil
        let cls = class.define("Tag");
        print len(cls.__fields);

        // http_client.new: no args, base_url "" and headers map{} by default
        let c = http_client.new();
        print c._base_url;

        // graph.new defaults to directed=true; add_edge defaults weight to 1.0
        import "stdlib:graph.sapp";
        let g = graph.new();
        print g.directed;
        let g = graph.add_node(g, "a");
        let g = graph.add_node(g, "b");
        let g = graph.add_edge(g, "a", "b");
        print g._edge_list[0].weight;
    "#;
    let out = run_collect(src);
    assert_eq!(out.len(), 11);
    assert_eq!(out[0], Value::Number(2.0));
    assert_eq!(out[1].as_str().unwrap(), "b");
    assert_eq!(out[2].as_str().unwrap(), "x,y\n");
    assert_eq!(out[3], Value::Number(10.0));
    assert_eq!(out[4], Value::Number(0.0));
    assert_eq!(out[5], Value::Number(1.0));
    assert_eq!(out[6], Value::Number(4.0));
    assert_eq!(out[7], Value::Number(0.0));
    assert_eq!(out[8].as_str().unwrap(), "");
    assert_eq!(out[9], Value::Bool(true));
    assert_eq!(out[10], Value::Number(1.0));
}
