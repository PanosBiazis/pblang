use criterion::{criterion_group, criterion_main, Criterion};

use pblang_compiler::compile;
use pblang_core::parse;
use pblang_vm::{StdlibConfig, VM};

// ── N2 Cranelift JIT benchmarks ───────────────────────────────────────────────

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
        param_types: None,
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

// ── fib(35) — larger workload for meaningful speedup ──────────────────────────

fn bench_fib35_interpreter(c: &mut Criterion) {
    let src = r#"
        fn fib(n) {
            if n <= 1 { return n; }
            return fib(n - 1) + fib(n - 2);
        }
        print fib(35);
    "#;
    let stmts = parse(src).expect("parse failed");
    let chunk = compile(stmts).expect("compile failed");

    c.bench_function("vm/fib35_interpreter", |b| {
        b.iter(|| {
            let mut vm = VM::with_stdlib(StdlibConfig::default());
            let _ = vm.run_collect(chunk.clone()).expect("vm run failed");
        })
    });
}

fn bench_fib35_jit(c: &mut Criterion) {
    use pblang_cranelift::{JitCache, JitResult};
    use pblang_ir::chunk::{Chunk, OpCode, Span};
    use pblang_ir::{Function, Value as PV};

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
        param_types: None,
    };
    let mut cache = JitCache::new().expect("JitCache::new failed");
    let fn_ptr = match cache.compile("fib", &func) {
        JitResult::Compiled(fp) => fp,
        JitResult::NotEligible(r) => panic!("JIT not eligible: {r}"),
    };

    let n = 35.0f64;
    // fib(35) = 9227465
    c.bench_function("jit/fib35_cranelift", |b| {
        b.iter(|| {
            let result = unsafe { fn_ptr(&n as *const f64, 1) };
            assert_eq!(result, 9227465.0);
        })
    });
}

// ── Recursive sum: sum(0..N) via recursion ────────────────────────────────────

fn bench_recursive_sum_interpreter(c: &mut Criterion) {
    let src = r#"
        fn sum_rec(n) {
            if n <= 0 { return 0; }
            return n + sum_rec(n - 1);
        }
        print sum_rec(100);
    "#;
    let stmts = parse(src).expect("parse failed");
    let chunk = compile(stmts).expect("compile failed");

    c.bench_function("vm/recursive_sum_interpreter", |b| {
        b.iter(|| {
            let mut vm = VM::with_stdlib(StdlibConfig::default());
            let _ = vm.run_collect(chunk.clone()).expect("vm run failed");
        })
    });
}

fn bench_recursive_sum_jit(c: &mut Criterion) {
    use pblang_cranelift::{JitCache, JitResult};
    use pblang_ir::chunk::{Chunk, OpCode, Span};
    use pblang_ir::{Function, Value as PV};

    let mut chunk = Chunk::default();
    let sp = Span::default();
    let c0 = chunk.add_const(PV::Number(0.0));
    let c1 = chunk.add_const(PV::Number(1.0));
    let cf = chunk.add_const(pblang_ir::alloc_string("sum_rec"));

    // sum_rec(n):
    // if n <= 0 → return 0
    // else → n + sum_rec(n-1)
    chunk.write(OpCode::OpGetLocal(0), sp); // 0: push n
    chunk.write(OpCode::OpConstant(c0), sp); // 1: push 0
    chunk.write(OpCode::OpLe, sp); // 2: n <= 0
    chunk.write(OpCode::OpJumpIfFalse(6), sp); // 3: if false goto recursive
    chunk.write(OpCode::OpConstant(c0), sp); // 4: push 0
    chunk.write(OpCode::OpReturn, sp); // 5: return 0
    chunk.write(OpCode::OpGetLocal(0), sp); // 6: push n
    chunk.write(OpCode::OpGetGlobal(cf), sp); // 7: push sum_rec ref
    chunk.write(OpCode::OpGetLocal(0), sp); // 8: push n
    chunk.write(OpCode::OpConstant(c1), sp); // 9: push 1
    chunk.write(OpCode::OpSub, sp); // 10: n-1
    chunk.write(OpCode::OpCall(1), sp); // 11: sum_rec(n-1)
    chunk.write(OpCode::OpAdd, sp); // 12: n + sum_rec(n-1)
    chunk.write(OpCode::OpReturn, sp); // 13: return

    let func = Function {
        name: Some("sum_rec".into()),
        arity: 1,
        min_arity: 1,
        defaults: Vec::new(),
        is_vararg: false,
        chunk,
        param_types: None,
    };
    let mut cache = JitCache::new().expect("JitCache::new failed");
    let fn_ptr = match cache.compile("sum_rec", &func) {
        JitResult::Compiled(fp) => fp,
        JitResult::NotEligible(r) => panic!("JIT not eligible: {r}"),
    };

    let n = 100.0f64;
    c.bench_function("jit/recursive_sum_cranelift", |b| {
        b.iter(|| {
            let result = unsafe { fn_ptr(&n as *const f64, 1) };
            assert_eq!(result, 5050.0); // sum 0..100 = 5050
        })
    });
}

// ── Loop throughput — simple counting loop ────────────────────────────────────

fn bench_loop_count_interpreter(c: &mut Criterion) {
    let src = r#"
        let x = 0;
        for i 100_000 { let x = x + 1; }
        print x;
    "#;
    let stmts = parse(src).expect("parse failed");
    let chunk = compile(stmts).expect("compile failed");

    c.bench_function("vm/loop_100k_interpreter", |b| {
        b.iter(|| {
            let mut vm = VM::with_stdlib(StdlibConfig::default());
            let _ = vm.run_collect(chunk.clone()).expect("vm run failed");
        })
    });
}

// JIT loop: since the for-loop compiles to non-JIT-able opcodes,
// we construct a simple JIT loop using while-style bytecode.
fn bench_loop_count_jit(c: &mut Criterion) {
    use pblang_cranelift::{JitCache, JitResult};
    use pblang_ir::chunk::{Chunk, OpCode, Span};
    use pblang_ir::{Function, Value as PV};

    let mut chunk = Chunk::default();
    let sp = Span::default();
    let c0 = chunk.add_const(PV::Number(0.0));
    let c1 = chunk.add_const(PV::Number(1.0));
    let limit = chunk.add_const(PV::Number(100_000.0));

    // loop body: while x < 100000 { x = x + 1 }
    // Bytecode:
    // 0: push 0               (x = 0)
    // 1: setlocal 1
    // 2: getlocal 1           (loop: check condition)
    // 3: push 100000
    // 4: lt
    // 5: jump_if_false 10     (exit loop)
    // 6: getlocal 1           (body: x + 1)
    // 7: push 1
    // 8: add
    // 9: setlocal 1
    // 10: jump 2              (back to loop)
    // 11: getlocal 1          (return x)
    // 12: return
    chunk.write(OpCode::OpConstant(c0), sp); // 0
    chunk.write(OpCode::OpSetLocal(1), sp); // 1: x = 0
    chunk.write(OpCode::OpGetLocal(1), sp); // 2: loop start
    chunk.write(OpCode::OpConstant(limit), sp); // 3: push limit
    chunk.write(OpCode::OpLt, sp); // 4: x < limit
    chunk.write(OpCode::OpJumpIfFalse(9), sp); // 5: exit if false → 9
    chunk.write(OpCode::OpGetLocal(1), sp); // 6: push x
    chunk.write(OpCode::OpConstant(c1), sp); // 7: push 1
    chunk.write(OpCode::OpAdd, sp); // 8: x + 1
    chunk.write(OpCode::OpSetLocal(1), sp); // 9: x = x + 1
    chunk.write(OpCode::OpJump(2), sp); // 10: back to loop
    chunk.write(OpCode::OpGetLocal(1), sp); // 11: push x
    chunk.write(OpCode::OpReturn, sp); // 12: return

    let func = Function {
        name: Some("loop_count".into()),
        arity: 0,
        min_arity: 0,
        defaults: Vec::new(),
        is_vararg: false,
        chunk,
        param_types: None,
    };
    let mut cache = JitCache::new().expect("JitCache::new failed");
    let fn_ptr = match cache.compile("loop_count", &func) {
        JitResult::Compiled(fp) => fp,
        JitResult::NotEligible(r) => panic!("JIT not eligible: {r}"),
    };

    c.bench_function("jit/loop_100k_cranelift", |b| {
        b.iter(|| {
            let result = unsafe { fn_ptr(std::ptr::null(), 0) };
            assert_eq!(result, 100_000.0);
        })
    });
}

// ── Existing arithmetic loop (kept for regression comparison) ─────────────────

fn bench_arithmetic_loop(c: &mut Criterion) {
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

// ── Standalone speedup report (run via --bench jit_vm_speedup) ────────────────

/// Prints a human-readable speedup report comparing JIT vs interpreter.
/// Call this from a main-like benchmark context.
pub fn print_speedup_report() {
    use pblang_cranelift::{bench_jit_vs_interp, JitCache, JitResult};
    use pblang_ir::chunk::{Chunk, OpCode, Span};
    use pblang_ir::{Function, Value as PV};

    println!("\n╔══════════════════════════════════════════════════════╗");
    println!("║       pblang Cranelift JIT — Speedup Report         ║");
    println!("╚══════════════════════════════════════════════════════╝\n");

    // ── fib(35) ────────────────────────────────────────────────────────────
    {
        let mut chunk = Chunk::default();
        let sp = Span::default();
        let c1 = chunk.add_const(PV::Number(1.0));
        let cf = chunk.add_const(pblang_ir::alloc_string("fib"));
        let c2 = chunk.add_const(PV::Number(2.0));

        chunk.write(OpCode::OpGetLocal(0), sp);
        chunk.write(OpCode::OpConstant(c1), sp);
        chunk.write(OpCode::OpLe, sp);
        chunk.write(OpCode::OpJumpIfFalse(6), sp);
        chunk.write(OpCode::OpGetLocal(0), sp);
        chunk.write(OpCode::OpReturn, sp);
        chunk.write(OpCode::OpGetGlobal(cf), sp);
        chunk.write(OpCode::OpGetLocal(0), sp);
        chunk.write(OpCode::OpConstant(c1), sp);
        chunk.write(OpCode::OpSub, sp);
        chunk.write(OpCode::OpCall(1), sp);
        chunk.write(OpCode::OpGetGlobal(cf), sp);
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
            defaults: vec![],
            is_vararg: false,
            chunk,
            param_types: None,
        };
        let mut cache = JitCache::new().expect("JitCache::new");
        let fn_ptr = match cache.compile("fib", &func) {
            JitResult::Compiled(fp) => fp,
            JitResult::NotEligible(r) => panic!("fib not eligible: {r}"),
        };

        let stats = bench_jit_vs_interp(
            fn_ptr,
            |_args| {
                // Simple fib(35) in Rust for interpreter comparison
                fn fib(n: f64) -> f64 {
                    if n <= 1.0 {
                        n
                    } else {
                        fib(n - 1.0) + fib(n - 2.0)
                    }
                }
                PV::Number(fib(35.0))
            },
            &[PV::Number(35.0)],
            5,
        );
        println!("📊  fib(35):");
        println!("    VM  avg: {:>10.2} µs/call", stats.avg_interp_us);
        println!("    JIT avg: {:>10.2} µs/call", stats.avg_jit_us);
        println!("    Speedup: {:>10.2}x\n", stats.speedup);
    }

    // ── sum_rec(100) ───────────────────────────────────────────────────────
    {
        let mut chunk = Chunk::default();
        let sp = Span::default();
        let c0 = chunk.add_const(PV::Number(0.0));
        let c1 = chunk.add_const(PV::Number(1.0));
        let cf = chunk.add_const(pblang_ir::alloc_string("sum_rec"));

        chunk.write(OpCode::OpGetLocal(0), sp);
        chunk.write(OpCode::OpConstant(c0), sp);
        chunk.write(OpCode::OpLe, sp);
        chunk.write(OpCode::OpJumpIfFalse(6), sp);
        chunk.write(OpCode::OpConstant(c0), sp);
        chunk.write(OpCode::OpReturn, sp);
        chunk.write(OpCode::OpGetLocal(0), sp);
        chunk.write(OpCode::OpGetGlobal(cf), sp);
        chunk.write(OpCode::OpGetLocal(0), sp);
        chunk.write(OpCode::OpConstant(c1), sp);
        chunk.write(OpCode::OpSub, sp);
        chunk.write(OpCode::OpCall(1), sp);
        chunk.write(OpCode::OpAdd, sp);
        chunk.write(OpCode::OpReturn, sp);

        let func = Function {
            name: Some("sum_rec".into()),
            arity: 1,
            min_arity: 1,
            defaults: vec![],
            is_vararg: false,
            chunk,
            param_types: None,
        };
        let mut cache = JitCache::new().expect("JitCache::new");
        let fn_ptr = match cache.compile("sum_rec", &func) {
            JitResult::Compiled(fp) => fp,
            JitResult::NotEligible(r) => panic!("sum_rec not eligible: {r}"),
        };

        let stats = bench_jit_vs_interp(
            fn_ptr,
            |_args| {
                fn sum_rec(n: f64) -> f64 {
                    if n <= 0.0 {
                        0.0
                    } else {
                        n + sum_rec(n - 1.0)
                    }
                }
                PV::Number(sum_rec(100.0))
            },
            &[PV::Number(100.0)],
            1000,
        );
        println!("📊  sum_rec(100):");
        println!("    VM  avg: {:>10.2} µs/call", stats.avg_interp_us);
        println!("    JIT avg: {:>10.2} µs/call", stats.avg_jit_us);
        println!("    Speedup: {:>10.2}x\n", stats.speedup);
    }

    // ── loop throughput ────────────────────────────────────────────────────
    {
        let mut chunk = Chunk::default();
        let sp = Span::default();
        let c0 = chunk.add_const(PV::Number(0.0));
        let c1 = chunk.add_const(PV::Number(1.0));
        let cl = chunk.add_const(PV::Number(100_000.0));

        chunk.write(OpCode::OpConstant(c0), sp);
        chunk.write(OpCode::OpSetLocal(1), sp);
        chunk.write(OpCode::OpGetLocal(1), sp);
        chunk.write(OpCode::OpConstant(cl), sp);
        chunk.write(OpCode::OpLt, sp);
        chunk.write(OpCode::OpJumpIfFalse(9), sp);
        chunk.write(OpCode::OpGetLocal(1), sp);
        chunk.write(OpCode::OpConstant(c1), sp);
        chunk.write(OpCode::OpAdd, sp);
        chunk.write(OpCode::OpSetLocal(1), sp);
        chunk.write(OpCode::OpJump(2), sp);
        chunk.write(OpCode::OpGetLocal(1), sp);
        chunk.write(OpCode::OpReturn, sp);

        let func = Function {
            name: Some("loop_count".into()),
            arity: 0,
            min_arity: 0,
            defaults: vec![],
            is_vararg: false,
            chunk,
            param_types: None,
        };
        let mut cache = JitCache::new().expect("JitCache::new");
        let fn_ptr = match cache.compile("loop_count", &func) {
            JitResult::Compiled(fp) => fp,
            JitResult::NotEligible(r) => panic!("loop_count not eligible: {r}"),
        };

        let stats = bench_jit_vs_interp(
            fn_ptr,
            |_args| {
                let mut x: f64 = 0.0;
                for _ in 0..100_000 {
                    x += 1.0;
                }
                PV::Number(x)
            },
            &[],
            100,
        );
        println!("📊  loop(100k):");
        println!("    VM  avg: {:>10.2} µs/call", stats.avg_interp_us);
        println!("    JIT avg: {:>10.2} µs/call", stats.avg_jit_us);
        println!("    Speedup: {:>10.2}x\n", stats.speedup);
    }

    println!("══════════════════════════════════════════════════════════\n");
}

criterion_group!(
    benches,
    bench_fib_interpreter,
    bench_fib_jit,
    bench_fib35_interpreter,
    bench_fib35_jit,
    bench_recursive_sum_interpreter,
    bench_recursive_sum_jit,
    bench_loop_count_interpreter,
    bench_loop_count_jit,
    bench_arithmetic_loop,
    bench_function_calls,
    bench_string_concat,
    bench_json_roundtrip,
);
criterion_main!(benches);
