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

/// Compile under AddressSanitizer and run; returns (clean, combined_output).
/// `clean` is false if ASan reported an error or the program crashed.
/// Returns None when no ASan-capable compiler is available (skip).
fn run_asan(src: &str) -> Option<(bool, String)> {
    let out = pblang_ctranspiler::transpile_to_c(src).expect("transpile");
    let id = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir();
    let c_path = dir.join(format!("pbasan_{}_{}.c", std::process::id(), id));
    let bin_path = dir.join(format!("pbasan_{}_{}", std::process::id(), id));
    std::fs::write(&c_path, &out.c_source).expect("write c");
    let cc = ["clang", "gcc", "cc"].iter().find(|c| {
        Command::new(c).arg("--version").output().map(|o| o.status.success()).unwrap_or(false)
    })?;
    let st = Command::new(cc)
        .arg("-fsanitize=address")
        .arg(&c_path).arg("-o").arg(&bin_path).arg("-lm")
        .output().expect("cc");
    if !st.status.success() {
        let _ = std::fs::remove_file(&c_path);
        return None; // ASan not supported here → skip
    }
    // We are checking for heap CORRUPTION (out-of-bounds, bad malloc), not for
    // leaks — the no-GC C runtime intentionally never frees transient strings,
    // so LeakSanitizer noise is out of scope here.
    let r = Command::new(&bin_path)
        .env("ASAN_OPTIONS", "detect_leaks=0")
        .output()
        .expect("run");
    let _ = std::fs::remove_file(&c_path);
    let _ = std::fs::remove_file(&bin_path);
    let mut combined = String::from_utf8_lossy(&r.stdout).to_string();
    combined.push_str(&String::from_utf8_lossy(&r.stderr));
    let clean = r.status.success() && !combined.contains("AddressSanitizer");
    Some((clean, combined))
}

#[test]
fn str_repeat_huge_count_is_memory_safe() {
    // SECURITY: str_repeat malloc'd `len*reps+1` with no overflow/size guard, so
    // a large count overflowed size_t → undersized buffer → heap overflow (or a
    // too-big malloc / NULL deref). It must instead return safely.
    match run_asan("print str_repeat(\"ab\", 9300000000000000000);") {
        Some((clean, out)) => assert!(clean, "str_repeat overflow must be safe under ASan:\n{out}"),
        None => eprintln!("no ASan compiler; skipping"),
    }
}

#[test]
fn str_repeat_normal_still_works() {
    assert_eq!(run("print str_repeat(\"ab\", 3);"), "ababab");
    assert_eq!(run("print str_repeat(\"x\", 0);"), "");
}

#[test]
fn str_pad_huge_width_is_memory_safe() {
    // str_pad malloc'd `width+1` from a user double with no bound/NULL check →
    // huge width = too-big malloc / NULL deref / w+1 overflow.
    match run_asan("print str_pad_left(\"x\", 9300000000000000000, \"0\"); print str_pad_right(\"y\", 9300000000000000000, \"0\");") {
        Some((clean, out)) => assert!(clean, "str_pad huge width must be safe under ASan:\n{out}"),
        None => eprintln!("no ASan compiler; skipping"),
    }
}

#[test]
fn str_pad_normal_still_works() {
    assert_eq!(run("print str_pad_left(\"7\", 3, \"0\");"), "007");
    assert_eq!(run("print str_pad_right(\"7\", 3, \"0\");"), "700");
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
fn builtin_char_codes() {
    assert_eq!(run("print strfromcharcode(65); print strfromcharcode(97);"), "A\na");
    assert_eq!(run("print strcharcodeat(\"ABC\", 1);"), "66");
    // round-trip incl. a multi-byte codepoint
    assert_eq!(run("print strfromcharcode(233); print strcharcodeat(strfromcharcode(233), 0);"), "é\n233");
}

#[test]
fn builtin_num_atan2() {
    assert_eq!(run("print num_atan2(0.0, 1.0);"), "0");
}

#[test]
fn builtin_str_pad() {
    assert_eq!(run("print str_pad_left(\"42\", 5, \"0\");"), "00042");
    assert_eq!(run("print str_pad_right(\"42\", 5, \"-\");"), "42---");
    assert_eq!(run("print str_pad_left(\"longer\", 3, \"0\");"), "longer");
}

#[test]
fn builtin_array_take_drop() {
    assert_eq!(run("let t = array_take([1,2,3,4,5], 2); print len(t); print t[1];"), "2\n2");
    assert_eq!(run("let d = array_drop([1,2,3,4,5], 2); print len(d); print d[0];"), "3\n3");
    assert_eq!(run("print len(array_take([1,2], 9));"), "2");
}

#[test]
fn builtin_array_chunk() {
    assert_eq!(run("let c = array_chunk([1,2,3,4,5], 2); print len(c); print c[0][1]; print c[2][0];"), "3\n2\n5");
}

#[test]
fn builtin_array_flatten() {
    assert_eq!(run("let f = array_flatten([[1,2],[3],[4,5]]); print len(f); print f[0]; print f[4];"), "5\n1\n5");
    assert_eq!(run("let f = array_flatten([[1],2,[3]]); print len(f); print f[1];"), "3\n2");
}

#[test]
fn builtin_array_zip() {
    assert_eq!(run("let z = array_zip([1,2,3],[10,20]); print len(z); print z[0][0]; print z[0][1]; print z[1][1];"), "2\n1\n10\n20");
}

#[test]
fn builtin_enumerate() {
    assert_eq!(run("let e = enumerate([\"a\",\"b\"]); print len(e); print e[0][0]; print e[1][1];"), "2\n0\nb");
}
