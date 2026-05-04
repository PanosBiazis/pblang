use pblang_vm::Value;

mod common;
use common::{run_collect, run_collect_with_config, run_err};
use pblang_core::parse;
use pblang_vm::StdlibConfig;

#[test]
fn block_shadowing_works() {
    let out = run_collect(
        r#"
        let x = 1;
        { let x = 2; print x; }
        print x;
        "#,
    );
    assert_eq!(out, vec![Value::Number(2.0), Value::Number(1.0)]);
}

#[test]
fn for_loop_var_does_not_leak_outside_scope() {
    let err = run_err(
        r#"
        for i 1 { print i; }
        print i;
        "#,
    );
    assert!(
        err.message.contains("undefined global") || err.message.contains("undefined"),
        "unexpected error: {:?}",
        err
    );
}

#[test]
fn closure_captures_outer_local_via_fn_literal() {
    let out = run_collect(
        r#"
        let f = { let x = 41; fn() { return x + 1; } };
        print f();
        "#,
    );
    assert_eq!(out, vec![Value::Number(42.0)]);
}

#[test]
fn builtins_type_and_len_work() {
    let out = run_collect(
        r#"
        print type(1);
        print len("abc");
        print len([1, 2, 3]);
        print len(map{a: 1, b: 2});
        "#,
    );
    assert_eq!(
        out,
        vec![
            Value::Obj(std::rc::Rc::new(pblang_vm::Obj::String(
                "number".to_string()
            ))),
            Value::Number(3.0),
            Value::Number(3.0),
            Value::Number(2.0),
        ]
    );
}

#[test]
fn arrays_maps_indexing_and_assignment_work() {
    let out = run_collect(
        r#"
        let a = [10, 20, 30];
        print a[1];
        a[1] = 99;
        print a[1];

        let m = map{a: 1, "b": 2};
        print m["b"];
        m["b"] = 7;
        print m.b;
        m.c = 42;
        print m["c"];
        "#,
    );
    assert_eq!(
        out,
        vec![
            Value::Number(20.0),
            Value::Number(99.0),
            Value::Number(2.0),
            Value::Number(7.0),
            Value::Number(42.0),
        ]
    );
}

#[test]
fn json_parse_and_stringify_work() {
    let out = run_collect(
        r#"
        let v = json_parse("{\"a\": [1, 2], \"b\": true}");
        print v.a[0];
        print json_stringify(v);
        "#,
    );
    assert_eq!(out[0], Value::Number(1.0));
    // don't assert exact ordering; just ensure it's a string
    assert!(matches!(out[1], Value::Obj(_)));
}

#[test]
fn path_utils_work() {
    let out = run_collect(
        r#"
        print type(cwd());
        print path_basename(path_join("a", "b", "c.txt"));
        print path_dirname(path_join("a", "b", "c.txt"));
        "#,
    );
    assert_eq!(
        out,
        vec![
            Value::Obj(std::rc::Rc::new(pblang_vm::Obj::String(
                "string".to_string()
            ))),
            Value::Obj(std::rc::Rc::new(pblang_vm::Obj::String(
                "c.txt".to_string()
            ))),
            Value::Obj(std::rc::Rc::new(pblang_vm::Obj::String("a/b".to_string()))),
        ]
    );
}

#[test]
fn http_helpers_work() {
    let out = run_collect(
        r#"
        let req = http_parse_request("GET /x HTTP/1.1\r\nHost: example\r\n\r\nbody");
        print req.method;
        print req.path;
        print req.headers.Host;
        print req.body;
        print http_response(200, "ok");
        "#,
    );
    assert_eq!(
        out[0..4],
        [
            Value::Obj(std::rc::Rc::new(pblang_vm::Obj::String("GET".to_string()))),
            Value::Obj(std::rc::Rc::new(pblang_vm::Obj::String("/x".to_string()))),
            Value::Obj(std::rc::Rc::new(pblang_vm::Obj::String(
                "example".to_string()
            ))),
            Value::Obj(std::rc::Rc::new(pblang_vm::Obj::String("body".to_string()))),
        ]
    );
    let resp = out[4].as_str().expect("expected response string");
    assert!(resp.contains("HTTP/1.1 200"));
    assert!(resp.contains("Content-Length: 2"));
}

#[test]
fn http_router_app_handle_works() {
    let out = run_collect(
        r#"
        import "stdlib:http_server.sapp";

        let app = http_server.app_new();
        let app = http_server.app_get(app, "/hi/:name", fn(req) {
            return map{
                status: 200,
                body: "hello " + req.params.name + " x=" + req.query.x,
                content_type: "text/plain"
            };
        });

        let req = http_parse_request("GET /hi/bob?x=1 HTTP/1.1\r\nHost: example\r\n\r\n");
        let res = http_server.app_handle(app, req);
        print res.body;
        print res.status;
        "#,
    );
    assert_eq!(
        out,
        vec![
            Value::Obj(std::rc::Rc::new(pblang_vm::Obj::String(
                "hello bob x=1".to_string()
            ))),
            Value::Number(200.0),
        ]
    );
}

#[test]
fn http_router_middleware_can_block() {
    let out = run_collect(
        r#"
        import "stdlib:http_server.sapp";

        let app = http_server.app_new();
        let app = http_server.app_use(app, http_server.mw_require_header("Auth", "ok"));
        let app = http_server.app_get(app, "/hi", fn(req) { return map{ status: 200, body: "hi" }; });

        let req = http_parse_request("GET /hi HTTP/1.1\r\nHost: example\r\n\r\n");
        let res = http_server.app_handle(app, req);
        print res.status;
        "#,
    );
    assert_eq!(out, vec![Value::Number(401.0)]);
}

#[test]
fn http_res_json_convenience_renders_json_response() {
    let out = run_collect(
        r#"
        import "stdlib:http_server.sapp";
        let resp = http_server.to_http_response(http_server.res.ok_json(map{ a: 1 }));
        print resp;
        "#,
    );
    assert_eq!(out.len(), 1);
    let s = out[0].as_str().expect("expected response string");
    assert!(s.contains("Content-Type: application/json"));
    assert!(s.contains("\"a\":1"));
}

#[test]
fn http_response_supports_custom_headers() {
    let out = run_collect(
        r#"
        import "stdlib:http_server.sapp";
        let headers = map{};
        headers["X-Test"] = "ok";
        let resp = http_server.to_http_response(map{
            status: 200,
            body: "hi",
            content_type: "text/plain",
            headers: headers
        });
        print resp;
        "#,
    );
    assert_eq!(out.len(), 1);
    let s = out[0].as_str().expect("expected response string");
    assert!(s.contains("HTTP/1.1 200"));
    assert!(s.contains("X-Test: ok"));
    assert!(s.contains("Content-Length: 2"));
}

#[test]
fn http_redirect_helper_renders_location_header() {
    let out = run_collect(
        r#"
        import "stdlib:http_server.sapp";
        let resp = http_server.to_http_response(http_server.res.redirect("/next"));
        print resp;
        "#,
    );
    assert_eq!(out.len(), 1);
    let s = out[0].as_str().expect("expected response string");
    assert!(s.contains("HTTP/1.1 302"));
    assert!(s.contains("Location: /next"));
}

#[test]
fn http_cookie_helpers_parse_and_get_values() {
    let out = run_collect(
        r#"
        import "stdlib:http_server.sapp";
        let req = http_parse_request("GET / HTTP/1.1\r\nHost: example\r\nCookie: sid=abc; theme=dark\r\n\r\n");
        let cs = http_server.cookies(req);
        print cs.sid;
        print cs.theme;
        print http_server.cookie_get(req, "sid", "none");
        print http_server.cookie_get(req, "missing", "none");
        "#,
    );
    assert_eq!(out.len(), 4);
    assert_eq!(out[0].as_str().expect("sid"), "abc");
    assert_eq!(out[1].as_str().expect("theme"), "dark");
    assert_eq!(out[2].as_str().expect("cookie_get sid"), "abc");
    assert_eq!(out[3].as_str().expect("cookie_get missing"), "none");
}

#[test]
fn http_static_file_response_serves_text_file_under_root() {
    let dir = std::env::temp_dir().join("pblang_tests_static");
    let _ = std::fs::create_dir_all(&dir);
    let hello = dir.join("hello.txt");
    std::fs::write(&hello, "hello").expect("write hello.txt");
    let root_str = dir.to_string_lossy().replace('\\', "/");

    let src = format!(
        r#"
        import "stdlib:http_server.sapp";
        let res = http_server.static_file_response("{root_str}", "/hello.txt");
        print res.status;
        print res.body;
        print res.content_type;
        "#,
    );

    let out = run_collect(&src);
    assert_eq!(out.len(), 3);
    assert_eq!(out[0], Value::Number(200.0));
    assert_eq!(out[1].as_str().expect("body"), "hello");
    assert_eq!(out[2].as_str().expect("content_type"), "text/plain");
}

#[test]
fn http_static_file_response_returns_404_for_missing_file() {
    let dir = std::env::temp_dir().join("pblang_tests_static_404");
    let _ = std::fs::create_dir_all(&dir);
    let root_str = dir.to_string_lossy().replace('\\', "/");

    let src = format!(
        r#"
        import "stdlib:http_server.sapp";
        let res = http_server.static_file_response("{root_str}", "/nonexistent.txt");
        print res.status;
        print res.body;
        "#,
    );

    let out = run_collect(&src);
    assert_eq!(out.len(), 2);
    assert_eq!(out[0], Value::Number(404.0));
    assert_eq!(out[1].as_str().expect("body"), "Not Found");
}

#[test]
fn expression_statement_allows_calls_for_side_effects() {
    let out = run_collect(
        r#"
        fn f() { print 1; return 2; }
        f();
        print 3;
        "#,
    );
    assert_eq!(out, vec![Value::Number(1.0), Value::Number(3.0)]);
}

#[test]
fn switch_statement_dispatches_cases() {
    let out = run_collect(
        r#"
        let x = 2;
        switch x {
            case 1 { print "one"; }
            case 2 { print "two"; }
            default { print "other"; }
        }
        "#,
    );
    assert_eq!(
        out,
        vec![Value::Obj(std::rc::Rc::new(pblang_vm::Obj::String(
            "two".to_string()
        )))]
    );
}

#[test]
fn try_expression_parses_inside_call_arguments() {
    let out = run_collect(
        r#"
        let kv = ["x", "y"];
        let v = str_trim(try kv[1] catch "");
        print v;
        "#,
    );
    assert_eq!(
        out,
        vec![Value::Obj(std::rc::Rc::new(pblang_vm::Obj::String(
            "y".to_string()
        )))]
    );
}

#[test]
fn switch_expression_returns_value() {
    let out = run_collect(
        r#"
        let x = 3;
        let y = switch x {
            case 1 { 10 }
            case 3 { 30 }
            default { 0 }
        };
        print y;
        "#,
    );
    assert_eq!(out, vec![Value::Number(30.0)]);
}

#[test]
fn switch_expression_supports_colon_short_form() {
    let out = run_collect(
        r#"
        let x = 2;
        let y = switch x {
            case 1: 10
            case 2: 20
            default: 0
        };
        print y;
        "#,
    );
    assert_eq!(out, vec![Value::Number(20.0)]);
}

#[test]
fn ffi_load_missing_library_errors() {
    use pblang_vm::StdlibConfig;
    let config = StdlibConfig {
        ffi: true,
        ..StdlibConfig::default()
    };
    let err = common::run_err_with_config(
        config,
        r#"
        import "stdlib:ffi.sapp";
        ffi.load("this_file_should_not_exist_12345.dll");
        "#,
    );
    assert!(
        err.message.contains("ffi_load")
            || err.message.contains("not exist")
            || err.message.contains("failed to load"),
        "unexpected error: {err:?}"
    );
}

#[test]
fn sqlite_db_exec_and_query_work() {
    let dir = std::env::temp_dir().join("pblang_tests_db");
    let _ = std::fs::create_dir_all(&dir);
    let db_path = dir.join("t.sqlite");
    let db_path_str = db_path.to_string_lossy().replace('\\', "/");

    let src = format!(
        r#"
        import "stdlib:db.sapp";
        let c = db.sqlite.open("{db_path_str}");
        c.exec("DROP TABLE IF EXISTS t", []);
        c.exec("CREATE TABLE t (x INTEGER, y TEXT)", []);
        c.exec("INSERT INTO t(x,y) VALUES (?,?)", [1, "a"]);
        c.exec("INSERT INTO t(x,y) VALUES (?,?)", [2, "b"]);
        let rows = c.query("SELECT x,y FROM t WHERE x > ? ORDER BY x", [0]);
        print len(rows);
        print rows[0].x;
        print rows[0].y;
        print rows[1].x;
        print rows[1].y;
        c.close();
        "#,
    );

    let out = run_collect(&src);
    assert_eq!(out.len(), 5);
    assert_eq!(out[0], Value::Number(2.0));
    assert_eq!(out[1], Value::Number(1.0));
    assert_eq!(out[2].as_str().expect("string"), "a");
    assert_eq!(out[3], Value::Number(2.0));
    assert_eq!(out[4].as_str().expect("string"), "b");
}

#[test]
fn db_migrations_apply_in_order_once() {
    let dir = std::env::temp_dir().join("pblang_tests_db");
    let _ = std::fs::create_dir_all(&dir);
    let db_path = dir.join("mig.sqlite");
    let db_path_str = db_path.to_string_lossy().replace('\\', "/");

    let src = format!(
        r#"
        import "stdlib:db.sapp";
        let c = db.sqlite.open("{db_path_str}");
        c.exec("DROP TABLE IF EXISTS t", []);
        c.exec("DROP TABLE IF EXISTS __pblang_migrations", []);

        db.migrate(c, [
            map{{ version: 2, up: "INSERT INTO t(x) VALUES (2);" }},
            map{{ version: 1, up: "CREATE TABLE t(x INTEGER); INSERT INTO t(x) VALUES (1);" }}
        ]);

        // running again should not insert more
        db.migrate(c, [
            map{{ version: 1, up: "INSERT INTO t(x) VALUES (111);" }},
            map{{ version: 2, up: "INSERT INTO t(x) VALUES (222);" }}
        ]);

        let rows = c.query("SELECT x FROM t ORDER BY x", []);
        print len(rows);
        print rows[0].x;
        print rows[1].x;
        c.close();
        "#,
    );

    let out = run_collect(&src);
    assert_eq!(
        out,
        vec![Value::Number(2.0), Value::Number(1.0), Value::Number(2.0)]
    );
}

#[test]
fn db_transaction_rolls_back_on_error() {
    let dir = std::env::temp_dir().join("pblang_tests_db");
    let _ = std::fs::create_dir_all(&dir);
    let db_path = dir.join("rollback.sqlite");
    let db_path_str = db_path.to_string_lossy().replace('\\', "/");

    let src = format!(
        r#"
        import "stdlib:db.sapp";
        let c = db.sqlite.open("{db_path_str}");
        c.exec("DROP TABLE IF EXISTS t", []);
        c.exec("CREATE TABLE t(x INTEGER)", []);

        let out = c.transaction(fn(tx) {{
            tx.exec("INSERT INTO t(x) VALUES (1)", []);
            return tx.missing_method();
        }});

        let rows = c.query("SELECT x FROM t ORDER BY x", []);
        print out;
        print len(rows);
        c.close();
        "#,
    );

    let out = run_collect(&src);
    assert_eq!(out, vec![Value::Nil, Value::Number(0.0)]);
}

#[test]
fn numeric_edge_cases_pass() {
    let src = include_str!("semantics/numeric_edge_test.sapp");
    let out = run_collect(src);
    // Last printed value should be the pass marker; earlier prints may log WARNs.
    let last = out
        .last()
        .and_then(|v: &pblang_vm::Value| v.as_str().ok())
        .unwrap_or("");
    assert!(
        last.contains("numeric_edge_test: pass"),
        "expected numeric_edge_test: pass, got {:?}",
        out
    );
}

#[test]
fn io_fs_append_and_read_bytes_work() {
    let dir = std::env::temp_dir().join("pblang_tests_io");
    let _ = std::fs::create_dir_all(&dir);
    let p = dir.join("a.txt");
    let p2 = dir.join("b.txt");
    let p_str = p.to_string_lossy().replace('\\', "/");
    let p2_str = p2.to_string_lossy().replace('\\', "/");

    let src = format!(
        r#"
        fs_write("{p_str}", "a");
        fs_append("{p_str}", "b");
        fs_copy("{p_str}", "{p2_str}");
        let bs = fs_read_bytes("{p2_str}");
        print len(bs);
        print bs[0];
        print bs[1];
        "#,
    );

    let out = run_collect(&src);
    assert_eq!(
        out,
        vec![Value::Number(2.0), Value::Number(97.0), Value::Number(98.0)]
    );
}

#[test]
fn class_sugar_creates_method_map() {
    let out = run_collect(
        r#"
        class Counter {
            fn inc(self) { self.n = self.n + 1; return self.n; }
        }
        let o = map{ __class: Counter, n: 0 };
        print o:inc();
        print o.n;
        "#,
    );
    assert_eq!(out, vec![Value::Number(1.0), Value::Number(1.0)]);
}

#[test]
fn method_call_sugar_falls_back_to_direct_map_method() {
    let out = run_collect(
        r#"
        let o = map{
            n: 0,
            inc: fn(self) { self.n = self.n + 1; return self.n; }
        };
        print o:inc();
        print o.n;
        "#,
    );
    assert_eq!(out, vec![Value::Number(1.0), Value::Number(1.0)]);
}

#[test]
fn class_auto_new_calls_init() {
    let out = run_collect(
        r#"
        class Point {
            fn init(self, x, y) { self.x = x; self.y = y; return nil; }
        }
        let p = Point.new2(3, 4);
        print p.x;
        print p.y;
        "#,
    );
    assert_eq!(out, vec![Value::Number(3.0), Value::Number(4.0)]);
}

#[test]
fn ptr_box_and_refs_work() {
    let out = run_collect(
        r#"
        import "stdlib:ptr.sapp";
        let p = ptr.new(1);
        print ptr.get(p);
        ptr.set(p, 2);
        print ptr.get(p);

        let m = map{ a: 1 };
        let r = ptr.ref_map(m, "a");
        print r.get();
        r.set(5);
        print m["a"];

        let a = [10, 20];
        let ra = ptr.ref_array(a, 1);
        print ra.get();
        ra.set(99);
        print a[1];
        "#,
    );
    assert_eq!(
        out,
        vec![
            Value::Number(1.0),
            Value::Number(2.0),
            Value::Number(1.0),
            Value::Number(5.0),
            Value::Number(20.0),
            Value::Number(99.0),
        ]
    );
}

#[test]
fn defer_runs_on_return_lifo_order() {
    let out = run_collect(
        r#"
        let log = [];
        fn f() {
            defer { array_push(log, 1); }
            defer { array_push(log, 2); }
            array_push(log, 0);
            return 42;
        }
        print f();
        print log;
        "#,
    );
    assert_eq!(out.len(), 2);
    assert_eq!(out[0], Value::Number(42.0));
    let arr = match &out[1] {
        pblang_vm::Value::Array(a) => a.borrow().clone(),
        _ => panic!("expected array"),
    };
    assert_eq!(arr.len(), 3);
    assert_eq!(arr[0], Value::Number(0.0));
    assert_eq!(arr[1], Value::Number(2.0));
    assert_eq!(arr[2], Value::Number(1.0));
}

#[test]
fn match_expression_literal_and_else() {
    let out = run_collect(
        r#"
        print match 1 { case 1: 10; case 2: 20; else: 0 };
        print match 2 { case 1: 10; case 2: 20; else: 0 };
        print match 3 { case 1: 10; case 2: 20; else: 0 };
        "#,
    );
    assert_eq!(
        out,
        vec![Value::Number(10.0), Value::Number(20.0), Value::Number(0.0)]
    );
}

#[test]
fn match_expression_bind_and_wildcard() {
    let out = run_collect(
        r#"
        let x = match "ok" { case "err": 0; case msg: len(msg); else: -1 };
        print x;
        "#,
    );
    assert_eq!(out, vec![Value::Number(2.0)]);
}

#[test]
fn strings_stdlib_helpers_work() {
    let out = run_collect(
        r#"
        import "stdlib:strings.sapp";
        print strings.join(["a","b","c"], "-");
        print strings.replace_all("a,b,c", ",", ";");
        print len(strings.chars("hi"));
        "#,
    );
    assert_eq!(out.len(), 3);
    assert_eq!(out[0].as_str().unwrap(), "a-b-c");
    assert_eq!(out[1].as_str().unwrap(), "a;b;c");
    assert_eq!(out[2], Value::Number(2.0));
}

#[test]
fn goto_block_can_loop_and_end() {
    let out = run_collect(
        r#"
        let st = map{ x: 0 };
        goto {
            label start {
                goto loop;
            }
            label loop {
                st.x = st.x + 1;
                if st.x == 3 { goto done; }
                goto loop;
            }
            label done {
                print st.x;
                end;
            }
        }
        "#,
    );
    assert_eq!(out, vec![Value::Number(3.0)]);
}

#[test]
fn gc_collect_returns_stats_string() {
    let out = run_collect(
        r#"
        print gc_collect();
        "#,
    );
    let s = out[0].as_str().expect("string");
    assert!(s.contains("gc_collect: closed"), "unexpected: {s}");
    assert!(s.contains("cycles"), "unexpected: {s}");
}

#[test]
fn gc_collect_breaks_rc_cycles_in_containers() {
    let out = run_collect(
        r#"
        {
            let a = [];
            a[0] = a;
        }
        {
            let m = map{};
            m.self = m;
        }
        print gc_collect();
        "#,
    );
    let s = out[0].as_str().expect("string");
    assert!(s.contains("cycles"), "unexpected: {s}");
    assert!(s.contains("arrays="), "unexpected: {s}");
    assert!(s.contains("maps="), "unexpected: {s}");
}

#[test]
fn polyglot_blocks_are_extracted_and_exposed() {
    let out = run_collect(
        r#"
        @rust mylib(version="1.0", opt=true) {
            fn add(a: i32, b: i32) -> i32 { a + b }
        }
        print __polyglot_blocks[0].lang;
        print __polyglot_blocks[0].name;
        print __polyglot_blocks[0].attrs.version;
        print __polyglot_blocks[0].attrs.opt;
        print __polyglot.by_name.mylib.lang;
        print len(__polyglot.by_lang.rust);
        print len(__polyglot_blocks);
        "#,
    );
    assert_eq!(out[0].as_str().unwrap(), "rust");
    assert_eq!(out[1].as_str().unwrap(), "mylib");
    assert_eq!(out[2].as_str().unwrap(), "1.0");
    assert_eq!(out[3].as_str().unwrap(), "true");
    assert_eq!(out[4].as_str().unwrap(), "rust");
    assert_eq!(out[5], Value::Number(1.0));
    assert_eq!(out[6], Value::Number(1.0));
}

#[test]
fn stdlib_json_module_parse_and_stringify_work() {
    let out = run_collect(
        r#"
        import "stdlib:json.sapp";
        let x = json.parse("{\"a\": 1}");
        print x.a;
        print json.stringify(map{ ok: true });
        "#,
    );
    assert_eq!(out[0], Value::Number(1.0));
    assert_eq!(out[1].as_str().unwrap(), "{\"ok\":true}");
}

#[test]
fn stdlib_fs_path_os_time_array_map_modules_work() {
    let out = run_collect(
        r#"
        import "stdlib:array.sapp";
        import "stdlib:map.sapp";
        import "stdlib:path.sapp";
        import "stdlib:fs.sapp";
        import "stdlib:os.sapp";
        import "stdlib:time.sapp";

        let a = [];
        print array.push(a, 1);
        print array.pop(a);

        let m = map{ a: 1, b: 2 };
        print map.has(m, "a");
        let ks = map.keys(m);
        print len(ks);
        print map.remove(m, "a");
        print map.has(m, "a");

        // path.join is deterministic; output uses forward slashes due to builtin normalization.
        print path.basename(path.join("a", "b.txt"));
        print path.extension(path.join(path.join("a", "b"), "c.txt"));
        print path.extension("noext");

        // path.exists, path.is_file, path.is_dir (temp file + cwd)
        let tmp = "path_test_tmp_semantics_xyz";
        fs_write(tmp, "x");
        print path.exists(tmp);
        print path.is_file(tmp);
        print path.is_dir(tmp);
        fs_remove(tmp);
        print path.exists(tmp);
        print path.is_dir(os.cwd());

        // path.absolute: "." resolves to cwd; result is a directory
        print path.is_dir(path.absolute("."));

        // time/os: just ensure they are callable and return sensible types.
        print type(time.clock());
        print type(time.now_ms());
        print type(os.cwd());
        print type(os.args());
        print type(time.now_iso());
        let iso = time.now_iso();
        print len(iso) > 0 and str_contains(iso, "T");
        "#,
    );
    assert_eq!(out[0], Value::Number(1.0));
    assert_eq!(out[1], Value::Number(1.0));
    assert_eq!(out[2], Value::Bool(true));
    assert_eq!(out[3], Value::Number(2.0));
    assert_eq!(out[4], Value::Number(1.0));
    assert_eq!(out[5], Value::Bool(false));
    assert_eq!(out[6].as_str().unwrap(), "b.txt");
    assert_eq!(out[7].as_str().unwrap(), "txt");
    assert_eq!(out[8].as_str().unwrap(), "");
    assert_eq!(out[9], Value::Bool(true));
    assert_eq!(out[10], Value::Bool(true));
    assert_eq!(out[11], Value::Bool(false));
    assert_eq!(out[12], Value::Bool(false));
    assert_eq!(out[13], Value::Bool(true));
    assert_eq!(out[14], Value::Bool(true));
    assert_eq!(out[15].as_str().unwrap(), "number");
    assert_eq!(out[16].as_str().unwrap(), "number");
    assert_eq!(out[17].as_str().unwrap(), "string");
    assert_eq!(
        out[18].as_str().unwrap(),
        "array",
        "os.args() must return array"
    );
    assert_eq!(
        out[19].as_str().unwrap(),
        "string",
        "time.now_iso() must return string"
    );
    assert_eq!(
        out[20],
        Value::Bool(true),
        "time.now_iso() must be ISO-8601 (contains T)"
    );
}

#[test]
fn web_routing_dispatch_and_path_params() {
    let src = r#"
        import "stdlib:router.sapp";
        let m = router.route_map();
        router.add_route(m, "GET", "/hello", fn(req) { return map{ "status": 200, "body": "hello" }; });
        let res = router.dispatch(m, "GET", "/hello", map{});
        print res["status"];
        print res["body"];
        router.add_route(m, "GET", "/user/:id", fn(req) { return map{ "status": 200, "body": req["params"]["id"] }; });
        let res2 = router.dispatch(m, "GET", "/user/42", map{});
        print res2["status"];
        print res2["body"];
        let notfound = router.dispatch(m, "GET", "/missing", map{});
        print notfound["status"];
    "#;
    let out = run_collect(src);
    assert_eq!(out.len(), 5);
    assert_eq!(out[0], Value::Number(200.0));
    assert_eq!(out[1].as_str().unwrap(), "hello");
    assert_eq!(out[2], Value::Number(200.0));
    assert_eq!(out[3].as_str().unwrap(), "42");
    assert_eq!(out[4], Value::Number(404.0));
}

#[test]
fn web_middleware_wrap_and_chain() {
    let src = r#"
        import "stdlib:router.sapp";
        import "stdlib:middleware.sapp";
        let m = router.route_map();
        router.add_route(m, "GET", "/hello", fn(req) { return map{ "status": 200, "body": "hello" }; });
        let inner = fn(req) { return router.dispatch(m, req["method"], req["path"], req); };
        let wrapped = middleware.wrap(inner, middleware.mw_log_stub());
        let res = wrapped(map{ method: "GET", path: "/hello" });
        print res["status"];
        print res["body"];
        let inner2 = fn(req) { return map{ "status": 200, "body": "ok" }; };
        let with_auth = middleware.chain(inner2, [middleware.mw_require_auth_stub("secret")]);
        let res_bad = with_auth(map{});
        print res_bad["status"];
        let res_ok = with_auth(map{ auth: "secret" });
        print res_ok["status"];
        print res_ok["body"];
    "#;
    let out = run_collect(src);
    assert_eq!(out.len(), 6);
    assert_eq!(out[0].as_str().unwrap(), "GET /hello");
    assert_eq!(out[1], Value::Number(200.0));
    assert_eq!(out[2].as_str().unwrap(), "hello");
    assert_eq!(out[3], Value::Number(401.0));
    assert_eq!(out[4], Value::Number(200.0));
    assert_eq!(out[5].as_str().unwrap(), "ok");
}

#[test]
fn process_spawn_wait_captures_stdout_and_exit_code() {
    let (cmd, args): (&str, Vec<&str>) = if cfg!(windows) {
        ("cmd", vec!["/c", "echo", "hello"])
    } else {
        ("echo", vec!["hello"])
    };
    let args_json = format!("[\"{}\"]", args.join("\", \""));
    let src = format!(
        r#"
        import "stdlib:process.sapp";
        let r = process.spawn_wait("{}", {});
        print r["exit_code"];
        print str_contains(r["stdout"], "hello");
        "#,
        cmd, args_json
    );
    let config = StdlibConfig {
        exec: true,
        ..StdlibConfig::default()
    };
    let out = run_collect_with_config(config, &src);
    assert_eq!(out.len(), 2);
    assert_eq!(out[0], Value::Number(0.0), "exit_code should be 0");
    assert_eq!(out[1], Value::Bool(true), "stdout should contain hello");
}

#[test]
fn multipart_parse_fields_and_file() {
    let src = r#"
        import "stdlib:multipart.sapp";
        let boundary = "----FormBoundary7MA4";
        let body = "------FormBoundary7MA4\r\n"
            + "Content-Disposition: form-data; name=\"field1\"\r\n\r\n"
            + "value_one\r\n"
            + "------FormBoundary7MA4\r\n"
            + "Content-Disposition: form-data; name=\"file1\"; filename=\"hello.txt\"\r\n"
            + "Content-Type: text/plain\r\n\r\n"
            + "hello file content\r\n"
            + "------FormBoundary7MA4--\r\n";
        let parts = multipart.parse(body, boundary);
        print len(parts);
        print parts[0]["name"];
        print parts[0]["body"];
        print parts[1]["name"];
        print parts[1]["filename"];
        print parts[1]["body"];
    "#;
    let out = run_collect(src);
    assert_eq!(out.len(), 6);
    assert_eq!(out[0], Value::Number(2.0));
    assert_eq!(out[1].as_str().unwrap(), "field1");
    assert_eq!(out[2].as_str().unwrap(), "value_one");
    assert_eq!(out[3].as_str().unwrap(), "file1");
    assert_eq!(out[4].as_str().unwrap(), "hello.txt");
    assert_eq!(out[5].as_str().unwrap(), "hello file content");
}

#[test]
fn websocket_upgrade_response_rfc6455() {
    let src = r#"
        import "stdlib:websocket.sapp";
        let rfc_key = "dGhlIHNhbXBsZSBub25jZQ==";
        let resp = websocket.upgrade_response(rfc_key);
        print str_contains(resp, "101");
        print str_contains(resp, "Sec-WebSocket-Accept");
        print str_contains(resp, "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=");
    "#;
    let out = run_collect(src);
    assert_eq!(out.len(), 3);
    assert_eq!(out[0], Value::Bool(true), "101 in response");
    assert_eq!(
        out[1],
        Value::Bool(true),
        "Sec-WebSocket-Accept in response"
    );
    assert_eq!(
        out[2],
        Value::Bool(true),
        "RFC 6455 test vector accept value"
    );
}

#[test]
fn quantum_protocol_submit_and_poll() {
    let src = r#"
        import "stdlib:quantum_protocol.sapp";
        let spec = map{ n: 1, ops: [ map{ gate: "h", q: 0 } ] };
        let out = quantum_protocol.submit(spec);
        print out["status"];
        print len(out["result"]["measurements"]);
        let p = quantum_protocol.poll(out["job_id"]);
        print p["status"];
    "#;
    let out = run_collect(src);
    assert_eq!(out.len(), 3);
    assert_eq!(out[0].as_str().unwrap(), "completed");
    assert_eq!(out[1], Value::Number(1.0));
    assert_eq!(out[2].as_str().unwrap(), "completed");
}

#[test]
fn quantum_sim_single_and_two_qubit() {
    let src = r#"
        let sim = import("stdlib:quantum_sim.sapp");
        let s0 = sim.zero_state(1);
        let m0 = sim.measure(s0, 0);
        print m0;
        let s1 = sim.zero_state(1);
        s1 = sim.x_gate(s1, 0);
        let m1 = sim.measure(s1, 0);
        print m1;
        let s2 = sim.zero_state(1);
        s2 = sim.h_gate(s2, 0);
        let m2 = sim.measure(s2, 0);
        print m2;
        let s3 = sim.zero_state(2);
        s3 = sim.h_gate(s3, 0);
        s3 = sim.cnot_gate(s3, 0, 1);
        let m3a = sim.measure(s3, 0);
        let m3b = sim.measure(s3, 1);
        print m3a;
        print m3b;
        print "quantum_sim_tests_pass";
    "#;
    let out = run_collect(src);
    assert_eq!(out.len(), 6);
    assert_eq!(out[0], Value::Number(0.0), "|0> measure must be 0");
    assert_eq!(out[1], Value::Number(1.0), "X|0> measure must be 1");
    assert!(
        out[2] == Value::Number(0.0) || out[2] == Value::Number(1.0),
        "H|0> measure must be 0 or 1"
    );
    assert_eq!(out[3], out[4], "Bell state: both qubits must agree");
    assert_eq!(out[5].as_str().unwrap(), "quantum_sim_tests_pass");
}

#[test]
fn logical_ops_short_circuit() {
    let out = run_collect(
        r#"
        fn side() { print 1; return true; }
        print false and side();
        print true or side();
        "#,
    );
    // `side()` must not run for either expression due to short-circuiting.
    assert_eq!(out, vec![Value::Bool(false), Value::Bool(true)]);
}

#[test]
fn runtime_error_includes_span_and_stack() {
    let err = run_err(
        r#"
        fn f(a) { return a; }
        print f();
        "#,
    );
    assert!(
        err.message.contains("arity mismatch"),
        "unexpected error: {err:?}"
    );
    assert!(err.span.is_some(), "expected span to be present: {err:?}");
    assert!(
        !err.stack.is_empty(),
        "expected non-empty stack trace: {err:?}"
    );
}

#[test]
fn try_catch_recovers_from_runtime_error() {
    let out = run_collect(
        r#"
        let x = try missing catch 42;
        print x;
        "#,
    );
    assert_eq!(out, vec![Value::Number(42.0)]);
}

#[test]
fn try_catch_with_block_expressions_works() {
    let out = run_collect(
        r#"
        let x = try { print missing; 1 } catch { 2 };
        print x;
        "#,
    );
    assert_eq!(out, vec![Value::Number(2.0)]);
}

#[test]
fn chunked_read_parse_hex_pass() {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/semantics/chunked_read_test.sapp");
    let src = std::fs::read_to_string(&path).expect("read chunked_read_test.sapp");
    let out = run_collect(&src);
    let got = out
        .last()
        .and_then(|v: &pblang_vm::Value| v.as_str().ok().map(String::from))
        .unwrap_or_default();
    assert_eq!(
        got, "chunked_read_parse_hex_pass",
        "expected final print chunked_read_parse_hex_pass, got: {:?}",
        out
    );
}

#[test]
fn http_server_timeout_serve_once_pass() {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/semantics/http_server_timeout_test.sapp");
    let src = std::fs::read_to_string(&path).expect("read http_server_timeout_test.sapp");
    let out = run_collect(&src);
    let got = out
        .last()
        .and_then(|v: &pblang_vm::Value| v.as_str().ok().map(String::from))
        .unwrap_or_default();
    assert_eq!(
        got, "http_server_timeout_pass",
        "expected final print http_server_timeout_pass, got: {:?}",
        out
    );
}

#[test]
fn http_rate_limit_middleware_pass() {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/semantics/http_rate_limit_test.sapp");
    let src = std::fs::read_to_string(&path).expect("read http_rate_limit_test.sapp");
    let out = run_collect(&src);
    let got = out
        .last()
        .and_then(|v: &pblang_vm::Value| v.as_str().ok().map(String::from))
        .unwrap_or_default();
    assert_eq!(
        got, "http_rate_limit_pass",
        "expected final print http_rate_limit_pass, got: {:?}",
        out
    );
}

#[test]
fn ai_stream_parse_sse_pass() {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/semantics/ai_stream_test.sapp");
    let src = std::fs::read_to_string(&path).expect("read ai_stream_test.sapp");
    let out = run_collect(&src);
    let got = out
        .last()
        .and_then(|v: &pblang_vm::Value| v.as_str().ok().map(String::from))
        .unwrap_or_default();
    assert_eq!(
        got, "ai_stream_test: pass",
        "expected final print ai_stream_test: pass, got: {:?}",
        out
    );
}

#[test]
fn tensor_test_pass() {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/semantics/tensor_test.sapp");
    let src = std::fs::read_to_string(&path).expect("read tensor_test.sapp");
    let out = run_collect(&src);
    let got = out
        .last()
        .and_then(|v: &pblang_vm::Value| v.as_str().ok().map(String::from))
        .unwrap_or_default();
    assert_eq!(
        got, "tensor_test_pass",
        "expected final print tensor_test_pass, got: {:?}",
        out
    );
}

#[test]
fn quantum_circuit_builder_pass() {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/semantics/quantum_circuit_builder_test.sapp");
    let src = std::fs::read_to_string(&path).expect("read quantum_circuit_builder_test.sapp");
    let out = run_collect(&src);
    let got = out
        .last()
        .and_then(|v: &pblang_vm::Value| v.as_str().ok().map(String::from))
        .unwrap_or_default();
    assert_eq!(
        got, "quantum_circuit_builder_pass",
        "expected final print quantum_circuit_builder_pass, got: {:?}",
        out
    );
}

#[test]
fn quantum_run_test_pass() {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/semantics/quantum_run_test.sapp");
    let src = std::fs::read_to_string(&path).expect("read quantum_run_test.sapp");
    let out = run_collect(&src);
    let got = out
        .last()
        .and_then(|v: &pblang_vm::Value| v.as_str().ok().map(String::from))
        .unwrap_or_default();
    assert_eq!(
        got, "quantum_run_test: pass",
        "expected final print quantum_run_test: pass, got: {:?}",
        out
    );
}

#[test]
fn quantum_histogram_test_pass() {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/semantics/quantum_histogram_test.sapp");
    let src = std::fs::read_to_string(&path).expect("read quantum_histogram_test.sapp");
    let out = run_collect(&src);
    let got = out
        .last()
        .and_then(|v: &pblang_vm::Value| v.as_str().ok().map(String::from))
        .unwrap_or_default();
    assert_eq!(
        got, "quantum_histogram_pass",
        "expected final print quantum_histogram_pass, got: {:?}",
        out
    );
}

#[test]
fn quantum_ecc_repetition_pass() {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/semantics/quantum_ecc_test.sapp");
    let src = std::fs::read_to_string(&path).expect("read quantum_ecc_test.sapp");
    let out = run_collect(&src);
    let got = out
        .last()
        .and_then(|v: &pblang_vm::Value| v.as_str().ok().map(String::from))
        .unwrap_or_default();
    assert_eq!(
        got, "quantum_ecc_test: pass",
        "expected final print quantum_ecc_test: pass, got: {:?}",
        out
    );
}

#[test]
fn channel_close_semantics_pass() {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/semantics/channel_close_test.sapp");
    let src = std::fs::read_to_string(&path).expect("read channel_close_test.sapp");
    let out = run_collect(&src);
    let got = out
        .last()
        .and_then(|v: &pblang_vm::Value| v.as_str().ok().map(String::from))
        .unwrap_or_default();
    assert_eq!(
        got, "channel_close_semantics_pass",
        "expected final print channel_close_semantics_pass, got: {:?}",
        out
    );
}

#[test]
fn select_multi_channel_pass() {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/semantics/select_test.sapp");
    let src = std::fs::read_to_string(&path).expect("read select_test.sapp");
    let out = run_collect(&src);
    let got = out
        .last()
        .and_then(|v: &pblang_vm::Value| v.as_str().ok().map(String::from))
        .unwrap_or_default();
    assert_eq!(
        got, "select_multi_channel_pass",
        "expected final print select_multi_channel_pass, got: {:?}",
        out
    );
}

#[test]
fn http_post_auth_builtin_registered_pass() {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/semantics/http_post_auth_test.sapp");
    let src = std::fs::read_to_string(&path).expect("read http_post_auth_test.sapp");
    let out = run_collect(&src);
    let got = out
        .last()
        .and_then(|v: &pblang_vm::Value| v.as_str().ok().map(String::from))
        .unwrap_or_default();
    assert_eq!(
        got, "http_post_auth_builtin_registered_pass",
        "expected final print http_post_auth_builtin_registered_pass, got: {:?}",
        out
    );
}

#[test]
fn quantum_remote_stub_contract_pass() {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/semantics/quantum_remote_stub_test.sapp");
    let src = std::fs::read_to_string(&path).expect("read quantum_remote_stub_test.sapp");
    let out = run_collect(&src);
    let got = out
        .last()
        .and_then(|v: &pblang_vm::Value| v.as_str().ok().map(String::from))
        .unwrap_or_default();
    assert_eq!(
        got, "quantum_remote_stub_contract_pass",
        "expected final print quantum_remote_stub_contract_pass, got: {:?}",
        out
    );
}

#[test]
fn csv_quoted_fields_pass() {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/semantics/csv_quoted_test.sapp");
    let src = std::fs::read_to_string(&path).expect("read csv_quoted_test.sapp");
    let out = run_collect(&src);
    let got = out
        .last()
        .and_then(|v: &pblang_vm::Value| v.as_str().ok().map(String::from))
        .unwrap_or_default();
    assert_eq!(
        got, "csv_quoted_fields_pass",
        "expected final print csv_quoted_fields_pass, got: {:?}",
        out
    );
}

#[test]
fn collections_helpers_pass() {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/semantics/collections_test.sapp");
    let src = std::fs::read_to_string(&path).expect("read collections_test.sapp");
    let out = run_collect(&src);
    let got = out
        .last()
        .and_then(|v: &pblang_vm::Value| v.as_str().ok().map(String::from))
        .unwrap_or_default();
    assert_eq!(
        got, "collections_test: pass",
        "expected final print collections_test: pass, got: {:?}",
        out
    );
}

#[test]
fn math_ext_test_pass() {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/semantics/math_ext_test.sapp");
    let src = std::fs::read_to_string(&path).expect("read math_ext_test.sapp");
    let out = run_collect(&src);
    let got = out
        .last()
        .and_then(|v: &pblang_vm::Value| v.as_str().ok().map(String::from))
        .unwrap_or_default();
    assert_eq!(
        got, "math_ext_test: pass",
        "expected final print math_ext_test: pass, got: {:?}",
        out
    );
}

#[test]
fn thread_stdlib_wrapper_pass() {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/semantics/thread_stdlib_test.sapp");
    let src = std::fs::read_to_string(&path).expect("read thread_stdlib_test.sapp");
    let out = run_collect_with_config(StdlibConfig::default(), &src);
    let got = out
        .last()
        .and_then(|v: &pblang_vm::Value| v.as_str().ok().map(String::from))
        .unwrap_or_default();
    assert_eq!(
        got, "thread_stdlib_test: pass",
        "expected final print thread_stdlib_test: pass, got: {:?}",
        out
    );
}

#[test]
fn websocket_parse_frame_pass() {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/semantics/websocket_parse_frame_test.sapp");
    let src = std::fs::read_to_string(&path).expect("read websocket_parse_frame_test.sapp");
    let out = run_collect(&src);
    let got = out
        .last()
        .and_then(|v: &pblang_vm::Value| v.as_str().ok().map(String::from))
        .unwrap_or_default();
    assert_eq!(
        got, "websocket_parse_frame_pass",
        "expected final print websocket_parse_frame_pass, got: {:?}",
        out
    );
}

#[test]
fn websocket_write_frame_pass() {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/semantics/websocket_write_frame_test.sapp");
    let src = std::fs::read_to_string(&path).expect("read websocket_write_frame_test.sapp");
    let out = run_collect(&src);
    let got = out
        .last()
        .and_then(|v: &pblang_vm::Value| v.as_str().ok().map(String::from))
        .unwrap_or_default();
    assert_eq!(
        got, "websocket_write_frame_test: pass",
        "expected final print websocket_write_frame_test: pass, got: {:?}",
        out
    );
}

#[test]
fn provider_registry_register_and_call_pass() {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/semantics/provider_registry_test.sapp");
    let src = std::fs::read_to_string(&path).expect("read provider_registry_test.sapp");
    let out = run_collect(&src);
    let got = out
        .last()
        .and_then(|v: &pblang_vm::Value| v.as_str().ok().map(String::from))
        .unwrap_or_default();
    assert_eq!(
        got, "provider_registry_test: pass",
        "expected final print provider_registry_test: pass, got: {:?}",
        out
    );
}

#[test]
fn class_basic_pass() {
    let out = run_collect(
        r#"
        class Point {
            fn init(self, x, y) { self.x = x; self.y = y; return nil; }
            fn get_x(self) { return self.x; }
            fn get_y(self) { return self.y; }
        }
        let p = Point(3, 4);
        print p:get_x();
        print p:get_y();
        "#,
    );
    assert_eq!(out, vec![Value::Number(3.0), Value::Number(4.0)]);
}

#[test]
fn class_method_dispatch_pass() {
    let out = run_collect(
        r#"
        class Counter {
            fn init(self, start) { self.count = start; return nil; }
            fn increment(self) { self.count = self.count + 1; return self.count; }
            fn get(self) { return self.count; }
        }
        let c = Counter(10);
        print c:get();
        print c:increment();
        print c:increment();
        print c:get();
        "#,
    );
    assert_eq!(
        out,
        vec![
            Value::Number(10.0),
            Value::Number(11.0),
            Value::Number(12.0),
            Value::Number(12.0)
        ]
    );
}

#[test]
fn match_exhaustive_pass() {
    let src = r#"
        let x = 2;
        let result = match x {
            case 1: "one";
            case 2: "two";
            case 3: "three";
            else: "other";
        };
        print result;
        "#;
    if let Err(e) = parse(src) {
        eprintln!("Parse error: {:?}", e);
        panic!("Parse failed");
    }
    let out = run_collect(src);
    assert_eq!(
        out,
        vec![Value::Obj(std::rc::Rc::new(pblang_vm::Obj::String(
            "two".to_string()
        )))]
    );
}

#[test]
fn match_guard_pass() {
    let src = r#"
        let x = 5;
        let result = match x {
            case n if n < 0: "negative";
            case n if n == 0: "zero";
            case n if n > 0 && n < 10: "small positive";
            case n if n >= 10: "large positive";
            else: "unknown";
        };
        print result;
        "#;
    if let Err(e) = parse(src) {
        eprintln!("Parse error: {:?}", e);
        panic!("Parse failed");
    }
    let out = run_collect(src);
    assert_eq!(
        out,
        vec![Value::Obj(std::rc::Rc::new(pblang_vm::Obj::String(
            "small positive".to_string()
        )))]
    );
}

#[test]
fn tls_stub_test_pass() {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/semantics/tls_stub_test.sapp");
    let src = std::fs::read_to_string(&path).expect("read tls_stub_test.sapp");
    let out = run_collect(&src);
    let got = out
        .last()
        .and_then(|v: &pblang_vm::Value| v.as_str().ok().map(String::from))
        .unwrap_or_default();
    assert_eq!(
        got, "tls_stub_test: pass",
        "expected final print tls_stub_test: pass, got: {:?}",
        out
    );
}

#[test]
fn tensor_ops_pass() {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/semantics/tensor_ops_test.sapp");
    let src = std::fs::read_to_string(&path).expect("read tensor_ops_test.sapp");
    let out = run_collect(&src);
    let got = out
        .last()
        .and_then(|v: &pblang_vm::Value| v.as_str().ok().map(String::from))
        .unwrap_or_default();
    assert_eq!(
        got, "tensor_ops_pass",
        "expected final print tensor_ops_pass, got: {:?}",
        out
    );
}

#[test]
fn quantum_noise_depolarizing_pass() {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/semantics/quantum_noise_test.sapp");
    let src = std::fs::read_to_string(&path).expect("read quantum_noise_test.sapp");
    let out = run_collect(&src);
    let got = out
        .last()
        .and_then(|v: &pblang_vm::Value| v.as_str().ok().map(String::from))
        .unwrap_or_default();
    assert_eq!(
        got, "quantum_noise_depolarizing_pass",
        "expected final print quantum_noise_depolarizing_pass, got: {:?}",
        out
    );
}

#[test]
fn nd_linalg_inv_solve_pass() {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/semantics/nd_linalg_test.sapp");
    let src = std::fs::read_to_string(&path).expect("read nd_linalg_test.sapp");
    let out = run_collect(&src);
    let got = out
        .last()
        .and_then(|v: &pblang_vm::Value| v.as_str().ok().map(String::from))
        .unwrap_or_default();
    assert_eq!(
        got, "nd_linalg_inv_solve_pass",
        "expected final print nd_linalg_inv_solve_pass, got: {:?}",
        out
    );
}

#[test]
fn thread_pool_stdlib_pass() {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/semantics/thread_pool_stdlib_test.sapp");
    let src = std::fs::read_to_string(&path).expect("read thread_pool_stdlib_test.sapp");
    let out = run_collect_with_config(StdlibConfig::default(), &src);
    let got = out
        .last()
        .and_then(|v: &pblang_vm::Value| v.as_str().ok().map(String::from))
        .unwrap_or_default();
    assert_eq!(
        got, "thread_pool_stdlib_pass",
        "expected final print thread_pool_stdlib_pass, got: {:?}",
        out
    );
}

#[test]
fn polyglot_exec_python_pass() {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/semantics/polyglot_exec_test.sapp");
    let src = std::fs::read_to_string(&path).expect("read polyglot_exec_test.sapp");
    // polyglot needs exec capability
    let config = StdlibConfig {
        exec: true,
        fs: true,
        ..StdlibConfig::default()
    };
    let out = run_collect_with_config(config, &src);
    let got = out
        .last()
        .and_then(|v: &pblang_vm::Value| v.as_str().ok().map(String::from))
        .unwrap_or_default();
    assert_eq!(
        got, "polyglot_exec_python_pass",
        "expected final print polyglot_exec_python_pass, got: {:?}",
        out
    );
}

#[test]
fn os_env_set_pass() {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/semantics/os_signal_test.sapp");
    let src = std::fs::read_to_string(&path).expect("read os_signal_test.sapp");
    let config = StdlibConfig {
        exec: true,
        ..StdlibConfig::default()
    };
    let out = run_collect_with_config(config, &src);
    let got = out
        .last()
        .and_then(|v: &pblang_vm::Value| v.as_str().ok().map(String::from))
        .unwrap_or_default();
    assert_eq!(
        got, "os_env_set_pass",
        "expected final print os_env_set_pass, got: {:?}",
        out
    );
}
