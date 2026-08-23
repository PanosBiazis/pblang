use std::process::Command;

fn normalized_stdout(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).replace("\r\n", "\n")
}

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
    let stdout = normalized_stdout(&run.stdout);
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

#[test]
fn ctranspiler_runtime_sha256_bitops_and_str_to_bytes_work() {
    // Regression for the native-runtime builtins added so crypto/security code native-compiles:
    // real SHA-256 (was a zero-returning stub), the bit_* ops, and str_to_bytes — all previously
    // undeclared/broken in the emitted C.
    if !pblang_ctranspiler::is_c_compiler_available() {
        eprintln!("skipping: no C compiler available");
        return;
    }

    let src = r#"
        print sha256("abc");
        print str(bit_and(255, 15)) + "," + str(bit_xor(5, 3)) + "," + str(bit_shl(1, 8));
        let b = str_to_bytes("AB");
        print str(b[0]) + "," + str(b[1]);
    "#;

    let out = pblang_ctranspiler::transpile_to_c(src).expect("transpile pblang to C");
    assert!(
        !out.c_source.contains(
            "return PB_STR(\"0000000000000000000000000000000000000000000000000000000000000000\")"
        ),
        "embedded runtime should not contain the old zero-returning sha256 stub"
    );

    let dir = tempfile::tempdir().expect("tempdir");
    let exe_base = dir.path().join("sha_runtime");
    let exe_base_s = exe_base.to_string_lossy().to_string();
    pblang_ctranspiler::compile_from_c(&out.c_source, &exe_base_s).expect("compile generated C");
    let exe = if cfg!(target_os = "windows") {
        exe_base.with_extension("exe")
    } else {
        exe_base
    };
    let run = Command::new(&exe).output().expect("run generated native executable");
    assert!(
        run.status.success(),
        "generated executable failed: stderr={}",
        String::from_utf8_lossy(&run.stderr)
    );
    let stdout = normalized_stdout(&run.stdout);
    // Canonical SHA-256("abc") — proves the real digest, not the zero stub.
    assert!(
        stdout.contains("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"),
        "native sha256(\"abc\") must be the real digest, stdout={stdout:?}"
    );
    assert!(stdout.contains("15,6,256"), "bit_and/xor/shl must compute, stdout={stdout:?}");
    assert!(stdout.contains("65,66"), "str_to_bytes must yield UTF-8 bytes, stdout={stdout:?}");
}

#[test]
fn ctranspiler_runtime_try_catch_catches_a_thrown_error() {
    // Regression: `try EXPR catch C` was emitted as `pb_try(EXPR, C)` — C evaluates args eagerly,
    // so a try-expr that THROWS ran before any setjmp handler and escaped the catch. Now emitted as
    // an inline setjmp block. This program throws from a called fn and must land in the catch.
    if !pblang_ctranspiler::is_c_compiler_available() {
        eprintln!("skipping: no C compiler available");
        return;
    }
    let src = r#"
        fn boom() { throw "kaboom"; }
        let a = try boom() catch "caught";
        print a;
        let b = try 41 catch "no";
        print str(b);
        let c = try error("ev") catch "caught-errval";
        print c;
    "#;
    let out = pblang_ctranspiler::transpile_to_c(src).expect("transpile");
    let dir = tempfile::tempdir().expect("tempdir");
    let exe_base = dir.path().join("trycatch");
    let exe_base_s = exe_base.to_string_lossy().to_string();
    pblang_ctranspiler::compile_from_c(&out.c_source, &exe_base_s).expect("compile C");
    let exe = if cfg!(target_os = "windows") { exe_base.with_extension("exe") } else { exe_base };
    let run = Command::new(&exe).output().expect("run");
    assert!(run.status.success(), "exe failed: stderr={}", String::from_utf8_lossy(&run.stderr));
    let stdout = normalized_stdout(&run.stdout);
    assert!(stdout.contains("caught\n"), "thrown error must be caught, stdout={stdout:?}");
    assert!(stdout.contains("41\n"), "non-throwing try yields its value, stdout={stdout:?}");
    assert!(stdout.contains("caught-errval\n"), "error value must be caught too, stdout={stdout:?}");
}

#[test]
fn ctranspiler_runtime_param_and_builtin_name_shadowing() {
    // Regressions: (1) function params were named positionally `a0,a1,...` in C, so a pblang param
    // named `a1` collided ("redeclared"); renamed to `__pba{i}`. (2) a local variable named like a
    // builtin (`sum`/`max`) was emitted as the builtin function pointer instead of the variable
    // (a C type error); scope is now checked before the builtin table.
    if !pblang_ctranspiler::is_c_compiler_available() {
        eprintln!("skipping: no C compiler available");
        return;
    }
    let src = r#"
        fn compute(x, a1) {
            let sum = 0;
            let vals = [x, a1, x + a1];
            let i = 0;
            while i < len(vals) { sum = sum + vals[i]; i = i + 1; }
            let max = 0;
            let j = 0;
            while j < len(vals) { if vals[j] > max { max = vals[j]; } j = j + 1; }
            return sum + max;
        }
        print str(compute(3, 5));
    "#;
    let out = pblang_ctranspiler::transpile_to_c(src).expect("transpile");
    let dir = tempfile::tempdir().expect("tempdir");
    let exe_base = dir.path().join("shadow");
    let exe_base_s = exe_base.to_string_lossy().to_string();
    pblang_ctranspiler::compile_from_c(&out.c_source, &exe_base_s).expect("compile C");
    let exe = if cfg!(target_os = "windows") { exe_base.with_extension("exe") } else { exe_base };
    let run = Command::new(&exe).output().expect("run");
    assert!(run.status.success(), "exe failed: stderr={}", String::from_utf8_lossy(&run.stderr));
    // vals=[3,5,8]: sum=16, max=8 -> 24
    assert!(normalized_stdout(&run.stdout).contains("24\n"), "expected 24");
}

#[test]
fn ctranspiler_runtime_tensor_matmul_argmax_and_spawn() {
    // Native tensor kernels + sequential spawn/join_all (the last pieces for AI code natively).
    if !pblang_ctranspiler::is_c_compiler_available() {
        eprintln!("skipping: no C compiler available");
        return;
    }
    let src = r#"
        let A = map{ shape: [2,3], data: [1,2,3, 4,5,6] };
        let B = map{ shape: [3,2], data: [7,8, 9,10, 11,12] };
        let C = tensor_matmul(A, B);
        print str(C.data[0]) + "," + str(C.data[1]) + "," + str(C.data[2]) + "," + str(C.data[3]);
        print str(tensor_argmax(map{ shape: [4], data: [3,9,2,5] }));
        let h = [];
        push(h, spawn(fn() { return 10; }));
        push(h, spawn(fn() { return 20; }));
        let r = join_all(h);
        print str(r[0] + r[1]);
    "#;
    let out = pblang_ctranspiler::transpile_to_c(src).expect("transpile");
    let dir = tempfile::tempdir().expect("tempdir");
    let exe_base = dir.path().join("tensor");
    let exe_base_s = exe_base.to_string_lossy().to_string();
    pblang_ctranspiler::compile_from_c(&out.c_source, &exe_base_s).expect("compile C");
    let exe = if cfg!(target_os = "windows") { exe_base.with_extension("exe") } else { exe_base };
    let run = Command::new(&exe).output().expect("run");
    assert!(run.status.success(), "exe failed: stderr={}", String::from_utf8_lossy(&run.stderr));
    let stdout = normalized_stdout(&run.stdout);
    assert!(stdout.contains("58,64,139,154\n"), "matmul, stdout={stdout:?}"); // [[58,64],[139,154]]
    assert!(stdout.contains("1\n"), "argmax of [3,9,2,5] is 1, stdout={stdout:?}");
    assert!(stdout.contains("30\n"), "spawn/join_all run the thunks (10+20), stdout={stdout:?}");
}

#[test]
fn ctranspiler_runtime_missing_key_throws_and_str_renders_containers() {
    // Round 26 regressions (root-caused via a native-vs-interpreter sweep on pbos_project_intake):
    //  1. user-level `m[k]` / `m.field` on a MISSING key must THROW (VM: "undefined key"), not
    //     return nil — else `try m[k] catch fallback` yields nil natively and crashes later.
    //  2. `str()` must RECURSIVELY render arrays/maps (`[map{k: v}, ...]`, bare strings), not emit
    //     `[array]`/`[map]` placeholders — else `str_contains(str(arr_of_maps), id)` fails.
    if !pblang_ctranspiler::is_c_compiler_available() {
        eprintln!("skipping: no C compiler available");
        return;
    }
    let src = r#"
        let m = map{ a: 1 };
        print str(try m["zzz"] catch 99);          // missing key throws -> caught -> 99
        let d = try m["zzz"] catch 0;
        print str(d + 5);                          // fallback is a real number: 0 + 5 = 5
        let xs = [map{ id: "nostd_kernel_mode", phase: 0 }, map{ id: "other" }];
        print str(str_contains(str(xs), "nostd_kernel_mode"));  // recursive str() -> true
        print str([1, 2, 3]);                      // -> [1, 2, 3]
    "#;
    let out = pblang_ctranspiler::transpile_to_c(src).expect("transpile");
    let dir = tempfile::tempdir().expect("tempdir");
    let exe_base = dir.path().join("keys_str");
    let exe_base_s = exe_base.to_string_lossy().to_string();
    pblang_ctranspiler::compile_from_c(&out.c_source, &exe_base_s).expect("compile C");
    let exe = if cfg!(target_os = "windows") { exe_base.with_extension("exe") } else { exe_base };
    let run = Command::new(&exe).output().expect("run");
    assert!(run.status.success(), "exe failed: stderr={}", String::from_utf8_lossy(&run.stderr));
    let stdout = normalized_stdout(&run.stdout);
    assert!(stdout.contains("99\n"), "missing key must throw and be caught -> 99, stdout={stdout:?}");
    assert!(stdout.contains("5\n"), "caught fallback is a number (0+5), stdout={stdout:?}");
    assert!(stdout.contains("true\n"), "str() renders array-of-maps so str_contains finds the id, stdout={stdout:?}");
    assert!(stdout.contains("[1, 2, 3]\n"), "str([1,2,3]) renders elements, stdout={stdout:?}");
}

#[test]
fn ctranspiler_runtime_str_split_empty_delim_and_slice() {
    // Round 27 (self-hosting-compiler unblocker): `str_split(s, "")` must split into individual
    // codepoints (it returned an EMPTY array natively), and `slice(seq, start, end)` must work on
    // strings AND arrays. The pblang tokenizer does `str_split(source, "")` then `slice(..)` per
    // char; the empty-array bug made it emit one empty punct token per character instead of lexing.
    if !pblang_ctranspiler::is_c_compiler_available() {
        eprintln!("skipping: no C compiler available");
        return;
    }
    let src = r#"
        let cs = str_split("ab c", "");
        print "split=" + str(len(cs)) + ":" + str_join(cs, "|");   // 4:a|b| |c
        print "sslice=" + slice("hello", 1, 4);                     // ell
        let a = slice([10, 20, 30, 40], 1, 3);
        print "aslice=" + str(a[0]) + "," + str(a[1]) + ":" + str(len(a));  // 20,30:2
        print "nilbound=" + slice("hello", 3, nil);                 // lo (end=nil -> len)
    "#;
    let out = pblang_ctranspiler::transpile_to_c(src).expect("transpile");
    let dir = tempfile::tempdir().expect("tempdir");
    let exe_base = dir.path().join("split_slice");
    let exe_base_s = exe_base.to_string_lossy().to_string();
    pblang_ctranspiler::compile_from_c(&out.c_source, &exe_base_s).expect("compile C");
    let exe = if cfg!(target_os = "windows") { exe_base.with_extension("exe") } else { exe_base };
    let run = Command::new(&exe).output().expect("run");
    assert!(run.status.success(), "exe failed: stderr={}", String::from_utf8_lossy(&run.stderr));
    let stdout = normalized_stdout(&run.stdout);
    assert!(stdout.contains("split=4:a|b| |c\n"), "str_split(s,\"\") splits into chars, stdout={stdout:?}");
    assert!(stdout.contains("sslice=ell\n"), "slice on a string, stdout={stdout:?}");
    assert!(stdout.contains("aslice=20,30:2\n"), "slice on an array, stdout={stdout:?}");
    assert!(stdout.contains("nilbound=lo\n"), "slice end=nil defaults to len, stdout={stdout:?}");
}
