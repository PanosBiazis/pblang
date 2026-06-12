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
    let dir = std::env::temp_dir().join("pblang_tests").join("fs_listdir");
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
            Value::Obj(std::rc::Rc::new(pblang_vm::Obj::String("demo".to_string()))),
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

        // money.format: amount only, defaults currency="USD" and decimals=2.
        import "stdlib:money.sapp";
        print money.format(1);
    "#;
    let out = run_collect(src);
    assert_eq!(out.len(), 12);
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
    // money.format(1) → "USD 1.00" or similar (we just assert non-empty money string)
    let money_str = out[11].as_str().unwrap();
    assert!(
        money_str.contains("USD") && money_str.contains("1"),
        "expected money.format default to include USD and value, got {money_str:?}"
    );
}

#[test]
fn money_is_exact_decimal_based() {
    // stdlib/money.sapp is built on the arbitrary-precision `decimal` type, so
    // there is no floating-point cents drift and `split` loses no cents.
    let src = r#"
        import "stdlib:money.sapp";
        print str(money.add("0.10", "0.20"));                 // 0.3 (exact, not 0.300...04)
        print money.format(money.add("19.95", "0.04"));       // "19.99 USD"
        let p = money.split("10", 3);                          // [3.34, 3.33, 3.33]
        print str(p[0]) + "," + str(p[1]) + "," + str(p[2]);
        print str(money.add(money.add(p[0], p[1]), p[2]));     // 10 (sums back exactly)
        print money.format(money.tax("100", "8.25"));          // "108.25 USD"
    "#;
    let out = run_collect(src);
    assert_eq!(out[0].as_str().unwrap(), "0.3", "exact decimal add");
    assert_eq!(out[1].as_str().unwrap(), "19.99 USD", "format");
    assert_eq!(
        out[2].as_str().unwrap(),
        "3.34,3.33,3.33",
        "cents-exact split"
    );
    assert_eq!(
        out[3].as_str().unwrap(),
        "10",
        "split parts sum back exactly"
    );
    assert_eq!(out[4].as_str().unwrap(), "108.25 USD", "tax");
}

#[test]
fn bignum_exact_helpers() {
    // stdlib/bignum.sapp — exact big-number math on the `bigint` type.
    let src = r#"
        import "stdlib:bignum.sapp";
        print str(bignum.factorial(30));   // 265252859812191058636308480000000
        print str(bignum.gcd(48, 180));    // 12
        print str(bignum.lcm(4, 6));       // 12
        print str(bignum.fib(100));        // 354224848179261915075
        print str(bignum.comb(52, 5));     // 2598960
        print str(bignum.perm(10, 3));     // 720
    "#;
    let out = run_collect(src);
    assert_eq!(
        out[0].as_str().unwrap(),
        "265252859812191058636308480000000"
    );
    assert_eq!(out[1].as_str().unwrap(), "12");
    assert_eq!(out[2].as_str().unwrap(), "12");
    assert_eq!(out[3].as_str().unwrap(), "354224848179261915075");
    assert_eq!(out[4].as_str().unwrap(), "2598960");
    assert_eq!(out[5].as_str().unwrap(), "720");
}

#[test]
fn fraction_exact_wrapper_over_rational() {
    // stdlib/fraction.sapp — exact fractions over the `rational` type.
    let src = r#"
        import "stdlib:fraction.sapp";
        print str(fraction.add(fraction.of(1, 3), fraction.of(1, 6)));   // 1/2
        print str(fraction.mul(fraction.of(2, 3), fraction.of(3, 4)));   // 1/2
        print fraction.to_mixed(fraction.of(7, 3));                       // "2 1/3"
        print str(fraction.reciprocal(fraction.of(3, 4)));               // 4/3
        print str(fraction.mediant(fraction.of(1, 2), fraction.of(1, 3)));// 2/5
        print str(fraction.approximate(3.14159265, 1000));               // 355/113
        print str(fraction.of("0.25"));                                  // 1/4
    "#;
    let out = run_collect(src);
    assert_eq!(out[0].as_str().unwrap(), "1/2", "exact add");
    assert_eq!(out[1].as_str().unwrap(), "1/2", "exact mul");
    assert_eq!(out[2].as_str().unwrap(), "2 1/3", "mixed number");
    assert_eq!(out[3].as_str().unwrap(), "4/3", "reciprocal");
    assert_eq!(out[4].as_str().unwrap(), "2/5", "mediant");
    assert_eq!(out[5].as_str().unwrap(), "355/113", "pi approximation");
    assert_eq!(
        out[6].as_str().unwrap(),
        "1/4",
        "decimal-string -> fraction"
    );
}

#[test]
fn set_module_core_operations() {
    // stdlib/set.sapp — a hash Set with full algebra.
    let src = r#"
        import "stdlib:set.sapp";
        let a = set.from_array([1, 2, 2, 3]);
        print set.size(a);                                  // 3 (dedup)
        print set.has(a, 2);                                // true
        print set.has(a, 9);                                // false
        print set.size(set.union(a, set.from_array([3, 4])));        // 4
        print set.size(set.intersection(a, set.from_array([2, 3, 5])));// 2
        print set.size(set.difference(a, set.from_array([2])));       // 2
        print set.is_subset(set.from_array([1, 2]), a);     // true
        print set.is_disjoint(set.from_array([7, 8]), a);   // true
        print set.equals(set.from_array([3, 1, 2]), a);     // true (order-independent)
    "#;
    let out = run_collect(src);
    assert_eq!(out[0], Value::Number(3.0));
    assert_eq!(out[1], Value::Bool(true));
    assert_eq!(out[2], Value::Bool(false));
    assert_eq!(out[3], Value::Number(4.0));
    assert_eq!(out[4], Value::Number(2.0));
    assert_eq!(out[5], Value::Number(2.0));
    assert_eq!(out[6], Value::Bool(true));
    assert_eq!(out[7], Value::Bool(true));
    assert_eq!(out[8], Value::Bool(true));
}

#[test]
fn vector_module_linear_algebra() {
    let src = r#"
        import "stdlib:vector.sapp";
        print vector.norm([3, 4]);              // 5
        print vector.dot([1, 2, 3], [4, 5, 6]); // 32
        let c = vector.cross([1, 0, 0], [0, 1, 0]);
        print c[0]; print c[1]; print c[2];     // 0, 0, 1
        print vector.l1_norm([1, -2, 3]);       // 6
        print vector.equals(vector.add([1, 2], [3, 4]), [4, 6]); // true
    "#;
    let out = run_collect(src);
    assert_eq!(out[0], Value::Number(5.0));
    assert_eq!(out[1], Value::Number(32.0));
    assert_eq!(out[2], Value::Number(0.0));
    assert_eq!(out[3], Value::Number(0.0));
    assert_eq!(out[4], Value::Number(1.0));
    assert_eq!(out[5], Value::Number(6.0));
    assert_eq!(out[6], Value::Bool(true));
}

#[test]
fn matrix_module_determinant_solve_inverse() {
    let src = r#"
        import "stdlib:matrix.sapp";
        print matrix.determinant([[1, 2], [3, 4]]);     // -2 (exact)
        let p = matrix.mul([[1, 2], [3, 4]], matrix.identity(2));
        print p[0][0]; print p[1][1];                   // 1, 4
        let t = matrix.transpose([[1, 2, 3], [4, 5, 6]]);
        print t[0][0]; print t[2][1];                   // 1, 6
        let x = matrix.solve([[2, 0, 0], [0, 3, 0], [0, 0, 5]], [4, 9, 20]);
        print x[0]; print x[1]; print x[2];             // 2, 3, 4
        let inv = matrix.inverse([[2, 0], [0, 4]]);
        print inv[0][0]; print inv[1][1];               // 0.5, 0.25
        print matrix.trace([[1, 2], [3, 4]]);           // 5
    "#;
    let out = run_collect(src);
    assert_eq!(out[0], Value::Number(-2.0));
    assert_eq!(out[1], Value::Number(1.0));
    assert_eq!(out[2], Value::Number(4.0));
    assert_eq!(out[3], Value::Number(1.0));
    assert_eq!(out[4], Value::Number(6.0));
    assert_eq!(out[5], Value::Number(2.0));
    assert_eq!(out[6], Value::Number(3.0));
    assert_eq!(out[7], Value::Number(4.0));
    assert_eq!(out[8], Value::Number(0.5));
    assert_eq!(out[9], Value::Number(0.25));
    assert_eq!(out[10], Value::Number(5.0));
}

#[test]
fn heap_module_priority_queue() {
    let src = r#"
        import "stdlib:heap.sapp";
        let h = heap.new_min();
        heap.push(h, 5); heap.push(h, 3); heap.push(h, 8); heap.push(h, 1);
        print heap.pop(h);   // 1
        print heap.pop(h);   // 3
        print heap.peek(h);  // 5
        print heap.size(h);  // 2
        let s = heap.heapsort([5, 3, 8, 1, 9, 2], true);
        print s[0]; print s[5];                 // 1, 9
        print heap.heapsort([5, 3, 8, 1], false)[0];  // 8 (max-heap)
    "#;
    let out = run_collect(src);
    assert_eq!(out[0], Value::Number(1.0));
    assert_eq!(out[1], Value::Number(3.0));
    assert_eq!(out[2], Value::Number(5.0));
    assert_eq!(out[3], Value::Number(2.0));
    assert_eq!(out[4], Value::Number(1.0));
    assert_eq!(out[5], Value::Number(9.0));
    assert_eq!(out[6], Value::Number(8.0));
}

#[test]
fn deque_module_double_ended() {
    let src = r#"
        import "stdlib:deque.sapp";
        let dq = deque.new();
        deque.push_back(dq, 1); deque.push_front(dq, 0); deque.push_back(dq, 2);
        let arr = deque.to_array(dq);
        print arr[0]; print arr[1]; print arr[2];   // 0, 1, 2
        print deque.pop_front(dq);                   // 0
        print deque.pop_back(dq);                    // 2
        print deque.size(dq);                        // 1
        print deque.peek_front(dq);                  // 1
    "#;
    let out = run_collect(src);
    assert_eq!(out[0], Value::Number(0.0));
    assert_eq!(out[1], Value::Number(1.0));
    assert_eq!(out[2], Value::Number(2.0));
    assert_eq!(out[3], Value::Number(0.0));
    assert_eq!(out[4], Value::Number(2.0));
    assert_eq!(out[5], Value::Number(1.0));
    assert_eq!(out[6], Value::Number(1.0));
}

#[test]
fn base32_module_rfc4648_vectors() {
    // RFC 4648 §10 test vectors.
    let src = r#"
        import "stdlib:base32.sapp";
        print base32.encode("f");
        print base32.encode("fo");
        print base32.encode("foo");
        print base32.encode("foobar");
        print base32.decode("MZXW6YTBOI======");
        print base32.encode("");
    "#;
    let out = run_collect(src);
    assert_eq!(out[0].as_str().unwrap(), "MY======");
    assert_eq!(out[1].as_str().unwrap(), "MZXQ====");
    assert_eq!(out[2].as_str().unwrap(), "MZXW6===");
    assert_eq!(out[3].as_str().unwrap(), "MZXW6YTBOI======");
    assert_eq!(out[4].as_str().unwrap(), "foobar");
    assert_eq!(out[5].as_str().unwrap(), "");
}

#[test]
fn complex_module_arithmetic() {
    let src = r#"
        import "stdlib:complex.sapp";
        print complex.abs(complex.new(3, 4));                              // 5
        print complex.to_string(complex.mul(complex.new(1, 2), complex.new(3, 4))); // -5+10i
        print complex.to_string(complex.sqrt(complex.new(3, 4)));          // 2+1i
        let d = complex.div(complex.new(4, 2), complex.new(1, 1));
        print complex.re(d); print complex.im(d);                          // 3, -1
        let p = complex.pow_int(complex.new(1, 1), 2);
        print complex.re(p); print complex.im(p);                          // 0, 2
        print complex.to_string(complex.conj(complex.new(2, 3)));          // 2-3i
    "#;
    let out = run_collect(src);
    assert_eq!(out[0], Value::Number(5.0));
    assert_eq!(out[1].as_str().unwrap(), "-5+10i");
    assert_eq!(out[2].as_str().unwrap(), "2+1i");
    assert_eq!(out[3], Value::Number(3.0));
    assert_eq!(out[4], Value::Number(-1.0));
    assert_eq!(out[5], Value::Number(0.0));
    assert_eq!(out[6], Value::Number(2.0));
    assert_eq!(out[7].as_str().unwrap(), "2-3i");
}

#[test]
fn strings_module_extended_utilities() {
    let src = r#"
        import "stdlib:strings.sapp";
        print strings.title_case("hello world");        // Hello World
        print strings.to_pascal_case("hello_world");     // HelloWorld
        print strings.to_kebab_case("helloWorld");        // hello-world
        print strings.slugify("Hello, World!");           // hello-world
        print strings.levenshtein("kitten", "sitting");   // 3
        print strings.swapcase("Hello");                  // hELLO
        print strings.center("hi", 6, "*");               // **hi**
        print strings.zfill("-7", 4);                     // -007
        print strings.ord("A");                            // 65
        print strings.chr(66);                             // B
        print strings.last_index_of("a.b.c", ".");        // 3
        print strings.common_prefix("foobar", "foobaz");   // fooba
        print strings.is_digit("12345");                   // true
        let w = strings.wrap("the quick brown fox", 9);
        print w[0]; print len(w);                          // "the quick", 2
    "#;
    let out = run_collect(src);
    assert_eq!(out[0].as_str().unwrap(), "Hello World");
    assert_eq!(out[1].as_str().unwrap(), "HelloWorld");
    assert_eq!(out[2].as_str().unwrap(), "hello-world");
    assert_eq!(out[3].as_str().unwrap(), "hello-world");
    assert_eq!(out[4], Value::Number(3.0));
    assert_eq!(out[5].as_str().unwrap(), "hELLO");
    assert_eq!(out[6].as_str().unwrap(), "**hi**");
    assert_eq!(out[7].as_str().unwrap(), "-007");
    assert_eq!(out[8], Value::Number(65.0));
    assert_eq!(out[9].as_str().unwrap(), "B");
    assert_eq!(out[10], Value::Number(3.0));
    assert_eq!(out[11].as_str().unwrap(), "fooba");
    assert_eq!(out[12], Value::Bool(true));
    assert_eq!(out[13].as_str().unwrap(), "the quick");
    assert_eq!(out[14], Value::Number(2.0));
}

#[test]
fn resources_module_memory_and_capture() {
    // stdlib/resources.sapp — unified memory/resource snapshots, diffs, live
    // capture, per-op tracking and GC, built on gc_stats/memory_stats/clocks.
    let src = r#"
        import "stdlib:resources.sapp";
        let before = resources.snapshot();
        let tmp = [];
        let i = 0;
        while i < 100 { array_push(tmp, map{ k: i }); i = i + 1; }
        let after = resources.snapshot();
        let d = resources.diff(before, after);
        print d.map_allocs >= 100;              // we allocated >= 100 maps
        print d.elapsed_ms >= 0;                // monotonic clock
        print d.total_allocs >= d.map_allocs;   // total includes the maps

        // track one operation's allocation cost
        let t = resources.track(fn() {
            let s = "";
            let j = 0;
            while j < 30 { s = s + "x"; j = j + 1; }
            return j;
        });
        print t.result;                          // 30
        print t.string_allocs >= 30;             // concat allocated strings

        // live capture: baseline + one snapshot per step
        let tl = resources.live_capture(4, fn(n) { let a = [n, n, n]; });
        print resources.summary(tl).samples;     // 5
        print len(resources.sample(4, fn(n) { let a = []; }));  // 4 per-step diffs

        // GC returns numeric reclaim counts; format yields a readable string
        print resources.gc().arrays_cleared >= 0;
        print str_contains(resources.format(after), "allocs");
    "#;
    let out = run_collect(src);
    assert_eq!(out[0], Value::Bool(true), "map allocations tracked");
    assert_eq!(out[1], Value::Bool(true), "elapsed_ms non-negative");
    assert_eq!(out[2], Value::Bool(true), "total >= map allocs");
    assert_eq!(out[3], Value::Number(30.0), "tracked op return value");
    assert_eq!(out[4], Value::Bool(true), "string allocations tracked");
    assert_eq!(
        out[5],
        Value::Number(5.0),
        "live_capture baseline + 4 steps"
    );
    assert_eq!(out[6], Value::Number(4.0), "sample yields per-step diffs");
    assert_eq!(out[7], Value::Bool(true), "gc reclaim counts numeric");
    assert_eq!(
        out[8],
        Value::Bool(true),
        "format produces a readable string"
    );
}
