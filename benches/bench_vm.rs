use criterion::{criterion_group, criterion_main, Criterion};

use pblang_compiler::compile;
use pblang_core::parse;
use pblang_vm::{StdlibConfig, VM};

// ── N2 Cranelift JIT benchmark ────────────────────────────────────────────────

fn bench_fib_interpreter(c: &mut Criterion) {
    // Fibonacci via pblang bytecode interpreter.
    let src = r#"
        fn fib(n) {
            if n <= 1 { return n; }
            return fib(n - 1) + fib(n - 2);
        }
        print fib(28);
    "#;
    let stmts = parse(src).expect("parse failed");
    let chunk = compile(stmts).expect("compile failed");

    c.bench_function("vm/fib28_interpreter", |b| {
        b.iter(|| {
            let mut vm = VM::with_stdlib(StdlibConfig::default());
            let _ = vm.run_collect(chunk.clone()).expect("vm run failed");
        })
    });
}

fn bench_fib_jit(c: &mut Criterion) {
    use pblang_cranelift::{JitCache, JitResult};
    use pblang_ir::chunk::{Chunk, OpCode, Span};
    use pblang_ir::{Function, Value as PV};

    // Build fibonacci bytecode directly for the JIT (same as the VM would compile).
    let mut chunk = Chunk::default();
    let sp = Span::default();
    let c1 = chunk.add_const(PV::Number(1.0));
    let cfib = chunk.add_const(pblang_ir::alloc_string("fib"));
    let c2 = chunk.add_const(PV::Number(2.0));

    chunk.write(OpCode::OpGetLocal(0), sp);
    chunk.write(OpCode::OpConstant(c1), sp);
    chunk.write(OpCode::OpLe, sp);
    chunk.write(OpCode::OpJumpIfFalse(6), sp);
    chunk.write(OpCode::OpGetLocal(0), sp);
    chunk.write(OpCode::OpReturn, sp);
    chunk.write(OpCode::OpGetGlobal(cfib), sp);
    chunk.write(OpCode::OpGetLocal(0), sp);
    chunk.write(OpCode::OpConstant(c1), sp);
    chunk.write(OpCode::OpSub, sp);
    chunk.write(OpCode::OpCall(1), sp);
    chunk.write(OpCode::OpGetGlobal(cfib), sp);
    chunk.write(OpCode::OpGetLocal(0), sp);
    chunk.write(OpCode::OpConstant(c2), sp);
    chunk.write(OpCode::OpSub, sp);
    chunk.write(OpCode::OpCall(1), sp);
    chunk.write(OpCode::OpAdd, sp);
    chunk.write(OpCode::OpReturn, sp);

    let func = Function {
        name: Some("fib".into()),
        arity: 1,
        min_arity: 1,
        defaults: Vec::new(),
        is_vararg: false,
        chunk,
    };
    let mut cache = JitCache::new().expect("JitCache::new failed");
    let fn_ptr = match cache.compile("fib", &func) {
        JitResult::Compiled(fp) => fp,
        JitResult::NotEligible(r) => panic!("JIT not eligible: {r}"),
    };

    let n = 28.0f64;
    c.bench_function("jit/fib28_cranelift", |b| {
        b.iter(|| {
            let result = unsafe { fn_ptr(&n as *const f64, 1) };
            assert_eq!(result, 317811.0);
        })
    });
}

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
    bench_fib_interpreter,
    bench_fib_jit,
    bench_arithmetic_loop,
    bench_function_calls,
    bench_string_concat,
    bench_json_roundtrip
);
criterion_main!(benches);
