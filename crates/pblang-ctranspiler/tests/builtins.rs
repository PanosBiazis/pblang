//! New native builtins in the C transpiler — widening the set of programs that
//! compile to a standalone binary. Transpile -> cc -> run -> assert stdout.

use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn run(src: &str) -> String {
    let out = pblang_ctranspiler::transpile_to_c(src).expect("transpile");
    let id = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir();
    let c_path = dir.join(format!("pbbi_{}_{}.c", std::process::id(), id));
    let bin_path = dir.join(format!("pbbi_{}_{}", std::process::id(), id));
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
fn builtin_map_has() {
    assert_eq!(run("let m = map{a: 1, b: 2}; print map_has(m, \"a\"); print map_has(m, \"zzz\");"), "true\nfalse");
}

#[test]
fn builtin_str_index_of() {
    assert_eq!(run("print str_index_of(\"hello world\", \"world\");"), "6");
    assert_eq!(run("print str_index_of(\"abc\", \"z\");"), "-1");
}

#[test]
fn builtin_num_min_max() {
    assert_eq!(run("print num_min(3, 7); print num_max(3, 7);"), "3\n7");
}

#[test]
fn builtin_push_alias() {
    assert_eq!(run("let a = [1, 2]; push(a, 3); push(a, 4); print len(a);"), "4");
}

#[test]
fn builtin_num_mod() {
    assert_eq!(run("print num_mod(17, 5); print num_mod(10, 2);"), "2\n0");
}

#[test]
fn builtin_aggregations() {
    assert_eq!(run("print sum([1, 2, 3, 4]);"), "10");
    assert_eq!(run("print product([1, 2, 3, 4]);"), "24");
    assert_eq!(run("print min_of([3, 1, 2]); print max_of([3, 1, 2]);"), "1\n3");
}

#[test]
fn builtin_clamp() {
    assert_eq!(run("print clamp(15, 0, 10); print clamp(0 - 3, 0, 10); print clamp(5, 0, 10);"), "10\n0\n5");
}

#[test]
fn builtin_str_repeat() {
    // str_repeat(_, 0) → "" prints a blank line, trimmed away by the harness.
    assert_eq!(run("print str_repeat(\"ab\", 3); print str_repeat(\"x\", 0);"), "ababab");
    assert_eq!(run("print str_repeat(\"x\", 0) == \"\";"), "true");
}

#[test]
fn builtin_str_join() {
    assert_eq!(run("print str_join([\"a\", \"b\", \"c\"], \"-\");"), "a-b-c");
}

#[test]
fn builtin_map_remove() {
    assert_eq!(run("let m = map{a: 1, b: 2}; print map_remove(m, \"a\"); print map_has(m, \"a\"); print map_has(m, \"b\");"), "1\nfalse\ntrue");
}

#[test]
fn builtin_range() {
    assert_eq!(run("print sum(range(5));"), "10");
    assert_eq!(run("print sum(range(2, 5));"), "9");
    assert_eq!(run("print len(range(0, 10, 2));"), "5");
}

#[test]
fn builtin_num_ln_exp() {
    assert_eq!(run("print num_ln(1); print num_exp(0);"), "0\n1");
}

#[test]
fn builtin_array_sort_numbers() {
    assert_eq!(run("let a = array_sort([3, 1, 2]); print a[0]; print a[1]; print a[2];"), "1\n2\n3");
}

#[test]
fn builtin_array_sort_strings() {
    assert_eq!(run("let a = array_sort([\"banana\", \"apple\", \"cherry\"]); print a[0]; print a[2];"), "apple\ncherry");
}

#[test]
fn builtin_array_unique() {
    assert_eq!(run("let u = array_unique([1, 2, 2, 3, 1]); print len(u); print u[0]; print u[2];"), "3\n1\n3");
}

#[test]
fn builtin_str_pad() {
    assert_eq!(run("print str_pad_left(\"42\", 5, \"0\");"), "00042");
    assert_eq!(run("print str_pad_right(\"42\", 5, \"-\");"), "42---");
    assert_eq!(run("print str_pad_left(\"longer\", 3, \"0\");"), "longer");
}
