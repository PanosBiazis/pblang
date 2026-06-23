//! Control-flow forms and struct/enum data types on the native (C) path.
//! Transpile -> cc -> run -> assert stdout. Locks in the `for i COUNT` loop form
//! and native struct/enum/match support (parser desugars them to fns + maps).

use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn run(src: &str) -> String {
    let out = pblang_ctranspiler::transpile_to_c(src).expect("transpile");
    let id = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir();
    let c_path = dir.join(format!("pbct_{}_{}.c", std::process::id(), id));
    let bin_path = dir.join(format!("pbct_{}_{}", std::process::id(), id));
    std::fs::write(&c_path, &out.c_source).expect("write c");
    let cc = ["cc", "gcc", "clang"]
        .iter()
        .find(|c| Command::new(c).arg("--version").output().map(|o| o.status.success()).unwrap_or(false))
        .expect("a C compiler");
    let st = Command::new(cc).arg(&c_path).arg("-o").arg(&bin_path).arg("-lm").output().expect("cc");
    assert!(st.status.success(), "compile failed:\n{}\n{}", String::from_utf8_lossy(&st.stderr), out.c_source);
    let r = Command::new(&bin_path).output().expect("run");
    let _ = std::fs::remove_file(&c_path);
    let _ = std::fs::remove_file(&bin_path);
    String::from_utf8_lossy(&r.stdout).trim().to_string()
}

#[test]
fn for_count_loop_sums_array() {
    assert_eq!(run("let a = [10, 20, 30]; let t = 0; for i len(a) { t = t + a[i]; } print t;"), "60");
}

#[test]
fn for_count_loop_indexes() {
    assert_eq!(run("let s = 0; for i 5 { s = s + i; } print s;"), "10");
}

#[test]
fn struct_construct_and_fields() {
    assert_eq!(
        run("struct Point { x: number, y: number } let p = Point(3, 4); print p.x + p.y; print p.__struct;"),
        "7\nPoint"
    );
}

#[test]
fn struct_nested_and_array() {
    // nested struct field access + array-of-structs iterated with `for i COUNT`
    let src = "struct Point { x: number, y: number } \
               struct Person { name: string, loc: Point } \
               let pp = Person(\"A\", Point(1, 2)); print pp.loc.x + pp.loc.y; \
               let pts = [Point(1,1), Point(2,2), Point(3,3)]; \
               let t = 0; for i len(pts) { t = t + pts[i].x; } print t;";
    assert_eq!(run(src), "3\n6");
}

#[test]
fn enum_unit_and_data_variants() {
    assert_eq!(
        run("enum Color { Red, Green, Blue } print Color.Red.__variant; print Color.Blue.__enum;"),
        "Red\nColor"
    );
    assert_eq!(
        run("enum Shape { Empty, Circle(number), Rect(number, number) } let c = Shape.Circle(5); print c.__variant; print c.values[0];"),
        "Circle\n5"
    );
}

#[test]
fn match_bind_arm() {
    // a bare-identifier arm binds the scrutinee; the binding must be declared in C
    assert_eq!(run("let x = 42; let r = match x { case n: n * 2 }; print r;"), "84");
}

#[test]
fn match_bind_guard_arm() {
    assert_eq!(
        run("let x = 7; let r = match x { case n if n > 5: \"big\"; case m: \"small\" }; print r;"),
        "big"
    );
}

#[test]
fn float_formatting_shortest_roundtrip() {
    // VM prints Rust's shortest round-trip; native must match (not %g 6-digit)
    assert_eq!(run("print 1.0 / 3.0;"), "0.3333333333333333");
    assert_eq!(run("print 0.1; print 1.5; print 2.0 / 4.0;"), "0.1\n1.5\n0.5");
    assert_eq!(run("print 0.1 + 0.2;"), "0.30000000000000004");
    // plain decimal, never exponential (matches Rust Display)
    assert_eq!(run("print 0.000001; print 123456.789;"), "0.000001\n123456.789");
}

#[test]
fn default_parameters() {
    // calling with fewer args must fill defaults (direct call pads with PB_NIL)
    assert_eq!(
        run("fn greet(name, greeting = \"Hi\") { return greeting + \" \" + name; } print greet(\"Al\"); print greet(\"Bo\", \"Yo\");"),
        "Hi Al\nYo Bo"
    );
    assert_eq!(run("fn f(a, b = 10, c = 100) { return a + b + c; } print f(1); print f(1, 2); print f(1, 2, 3);"), "111\n103\n6");
}

#[test]
fn try_catch_error_value() {
    // try-expr that yields an error value -> the catch fallback (matches the VM)
    assert_eq!(run("let r = try error(\"boom\") catch 99; print r;"), "99");
    assert_eq!(run("let r = try (10 / 2) catch 0; print r;"), "5");
}

#[test]
fn user_function_shadows_builtin() {
    // `sum` is also a builtin; a user fn named `sum` must win (matches the VM),
    // else the recursion dispatches to pb_builtin_sum and returns 0.
    assert_eq!(
        run("fn sum(n) { if n == 0 { return 0; } return n + sum(n - 1); } print sum(100);"),
        "5050"
    );
}

#[test]
fn logical_short_circuit() {
    // the right operand must NOT be evaluated when the left settles the result
    assert_eq!(run("fn boom() { throw \"x\"; } let r = false and boom(); print r;"), "false");
    assert_eq!(run("fn boom() { throw \"x\"; } let r = true or boom(); print r;"), "true");
    // and/or return the operand value (truthy semantics), not a bare bool
    assert_eq!(run("print true and 5; print false or 9; print 7 and 8;"), "5\n9\n8");
}

#[test]
fn string_indexing_codepoints() {
    // s[i] returns the i-th codepoint as a string (matches the VM)
    assert_eq!(run("let s = \"abc\"; print s[0]; print s[2];"), "a\nc");
    assert_eq!(run("let s = \"héllo\"; print s[1]; print len(s[1]);"), "é\n1");
}

#[test]
fn len_counts_codepoints() {
    // len on a string counts Unicode codepoints, not UTF-8 bytes (matches the VM)
    assert_eq!(run("print len(\"café\"); print len(\"naïve\"); print len(\"abc\");"), "4\n5\n3");
}

#[test]
fn enum_match_dispatch() {
    let src = "enum Shape { Empty, Circle(number), Rect(number, number) } \
               fn area(s) { return match s.__variant { \
                   case \"Empty\": 0; \
                   case \"Circle\": s.values[0] * s.values[0] * 3; \
                   case \"Rect\": s.values[0] * s.values[1]; \
                   else: 0 - 1 }; } \
               print area(Shape.Circle(2)); print area(Shape.Rect(3, 4)); print area(Shape.Empty);";
    assert_eq!(run(src), "12\n12\n0");
}
