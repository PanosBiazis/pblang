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
