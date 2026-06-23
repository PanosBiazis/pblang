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
