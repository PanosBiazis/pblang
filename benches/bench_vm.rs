use criterion::{criterion_group, criterion_main, Criterion};

use pblang_compiler::compile;
use pblang_core::parse;
use pblang_vm::{StdlibConfig, VM};

fn bench_arithmetic_loop(c: &mut Criterion) {
    // Keep it small & deterministic; we mainly want relative comparisons over time.
    let src = r#"
        let x = 0;
        for i 5000 { let x = x + 1; }
        print x;
    "#;
    let stmts = parse(src).expect("parse failed");
    let chunk = compile(stmts).expect("compile failed");

    c.bench_function("vm/arithmetic_loop", |b| {
        b.iter(|| {
            let mut vm = VM::with_stdlib(StdlibConfig::default());
            let _ = vm.run_collect(chunk.clone()).expect("vm run failed");
        })
    });
}

fn bench_function_calls(c: &mut Criterion) {
    let src = r#"
        fn add1(x) { return x + 1; }
        let x = 0;
        for i 3000 { let x = add1(x); }
        print x;
    "#;
    let stmts = parse(src).expect("parse failed");
    let chunk = compile(stmts).expect("compile failed");

    c.bench_function("vm/function_calls", |b| {
        b.iter(|| {
            let mut vm = VM::with_stdlib(StdlibConfig::default());
            let _ = vm.run_collect(chunk.clone()).expect("vm run failed");
        })
    });
}

fn bench_string_concat(c: &mut Criterion) {
    let src = r#"
        let s = "";
        for i 2000 { let s = s + "a"; }
        print len(s);
    "#;
    let stmts = parse(src).expect("parse failed");
    let chunk = compile(stmts).expect("compile failed");

    c.bench_function("vm/string_concat", |b| {
        b.iter(|| {
            let mut vm = VM::with_stdlib(StdlibConfig::default());
            let _ = vm.run_collect(chunk.clone()).expect("vm run failed");
        })
    });
}

fn bench_json_roundtrip(c: &mut Criterion) {
    // Parse and stringify a small JSON object repeatedly (exercises json + map/array).
    let src = r#"
        let raw = "{\"a\":1,\"b\":[2,3],\"c\":\"x\"}";
        let x = nil;
        for i 500 {
            let x = json_parse(raw);
            let _ = json_stringify(x);
        }
        print len(json_stringify(x));
    "#;
    let stmts = parse(src).expect("parse failed");
    let chunk = compile(stmts).expect("compile failed");

    c.bench_function("vm/json_roundtrip", |b| {
        b.iter(|| {
            let mut vm = VM::with_stdlib(StdlibConfig::default());
            let _ = vm.run_collect(chunk.clone()).expect("vm run failed");
        })
    });
}

criterion_group!(
    benches,
    bench_arithmetic_loop,
    bench_function_calls,
    bench_string_concat,
    bench_json_roundtrip
);
criterion_main!(benches);
