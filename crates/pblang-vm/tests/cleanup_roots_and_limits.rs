use pblang_compiler::compile;
use pblang_core::parse;
use pblang_vm::{StdlibConfig, Value, VmLimits, VM};

fn vm_with_program(source: &str, limit: Option<u64>) -> (VM, Result<(), String>) {
    let chunk = compile(parse(source).expect("parse")).expect("compile");
    let mut vm = VM::with_stdlib(StdlibConfig::default());
    vm.set_limits(Some(VmLimits {
        max_instructions: limit,
        ..Default::default()
    }));
    let result = vm.run(chunk).map_err(|e| e.message);
    (vm, result)
}

fn numbers(vm: &VM, name: &str) -> Vec<f64> {
    let value = vm
        .module
        .borrow()
        .globals
        .get(name)
        .cloned()
        .expect("global");
    match value {
        Value::Array(a) => a
            .borrow()
            .iter()
            .map(|v| match v {
                Value::Number(n) => *n,
                other => panic!("expected number: {other:?}"),
            })
            .collect(),
        other => panic!("expected array: {other:?}"),
    }
}

#[test]
fn pending_unwind_captures_survive_gc_list_and_map() {
    for capture in ["let a=[7];", "let a=map{value:7};"] {
        let access = if capture.contains("map") {
            "a.value"
        } else {
            "a[0]"
        };
        let source = format!("let log=[]; fn op(){{ {capture} defer push(log,{access}); defer gc_collect(); throw \"primary\"; }} let caught=try op() catch \"caught\";");
        let (vm, result) = vm_with_program(&source, None);
        assert!(result.is_ok(), "{result:?}");
        assert_eq!(numbers(&vm, "log"), vec![7.0]);
    }
}

#[test]
fn nested_unwind_captures_survive_gc_and_preserve_primary() {
    let (vm, result) = vm_with_program(
        r#"
        let log=[];
        fn inner(){ let a=map{value:8}; defer push(log,a.value); defer gc_collect(); throw "secondary"; }
        fn outer(){ let a=[7]; defer push(log,a[0]); defer inner(); throw "primary"; }
        outer();
    "#,
        None,
    );
    assert_eq!(result.unwrap_err(), "primary");
    assert_eq!(numbers(&vm, "log"), vec![8.0, 7.0]);
}

#[test]
fn registered_frame_defer_and_pending_return_survive_gc() {
    let (vm, result) = vm_with_program(
        r#"
        let log=[];
        fn op(){ {let a=[7]; defer push(log,a[0]);} defer gc_collect(); return [8]; }
        let result=op();
    "#,
        None,
    );
    assert!(result.is_ok(), "{result:?}");
    assert_eq!(numbers(&vm, "log"), vec![7.0]);
    assert_eq!(numbers(&vm, "result"), vec![8.0]);
}

#[test]
fn instruction_limit_runs_cleanup_but_cannot_be_caught() {
    let (vm, result) = vm_with_program(
        r#"
        let log=[];
        defer push(log,3);
        fn op(){ defer push(log,2); defer push(log,1); while true {} }
        let caught=try op() catch push(log,99);
        push(log,100);
    "#,
        Some(150),
    );
    assert_eq!(result.unwrap_err(), "instruction limit exceeded");
    assert_eq!(numbers(&vm, "log"), vec![1.0, 2.0, 3.0]);
}

#[test]
fn limit_inside_unwind_preserves_primary_and_pending_order() {
    let (vm, result) = vm_with_program(
        r#"
        let log=[];
        defer push(log,4);
        fn spin(){ defer push(log,2); while true {} }
        fn op(){ defer push(log,3); defer spin(); throw "primary"; }
        let caught=try op() catch push(log,99);
    "#,
        Some(150),
    );
    assert_eq!(result.unwrap_err(), "primary");
    assert_eq!(numbers(&vm, "log"), vec![2.0, 3.0, 4.0]);
}

#[test]
fn nested_caught_cleanup_error_does_not_reset_emergency_allowance() {
    let (vm, result) = vm_with_program(
        r#"
        let log=[];
        fn spin(){ while true { let ignored=try missing_name catch nil; } }
        fn op(){ defer push(log,99); defer spin(); while true {} }
        op();
    "#,
        Some(150),
    );
    assert_eq!(result.unwrap_err(), "instruction limit exceeded");
    assert!(numbers(&vm, "log").is_empty());
}

#[test]
fn exhausted_cleanup_allowance_preserves_primary() {
    let (vm, result) = vm_with_program(
        r#"
        let log=[];
        fn spin(){ while true {} }
        fn op(){ defer push(log,99); defer spin(); defer spin(); throw "primary"; }
        op();
    "#,
        Some(150),
    );
    assert_eq!(result.unwrap_err(), "primary");
    assert!(numbers(&vm, "log").is_empty());
}

#[test]
fn vm_reuse_after_budget_abort_has_no_stale_handlers() {
    let (mut vm, result) = vm_with_program(
        "let ignored=try missing_name catch nil; while true {}",
        Some(30),
    );
    assert!(result.is_err());
    vm.set_limits(None);
    let chunk = compile(parse("print 42;").unwrap()).unwrap();
    assert_eq!(vm.run_collect(chunk).unwrap(), vec![Value::Number(42.0)]);
}

#[test]
fn budget_conversion_merges_nested_unwinds_in_order() {
    let (vm, result) = vm_with_program(
        r#"
        let log=[];
        defer push(log,6);
        fn spin(){ defer push(log,1); while true {} }
        fn inner(){ defer push(log,2); defer spin(); throw "inner"; }
        fn middle(){ defer push(log,3); let ignored=try inner() catch push(log,99); }
        fn outer(){ defer push(log,5); defer middle(); throw "outer"; }
        let ignored=try outer() catch push(log,100);
    "#,
        Some(200),
    );
    assert_eq!(result.unwrap_err(), "outer");
    assert_eq!(numbers(&vm, "log"), vec![1.0, 2.0, 3.0, 5.0, 6.0]);
}

#[test]
fn emergency_cleanup_keeps_gc_captures_alive() {
    let (vm, result) = vm_with_program(
        r#"
        let log=[];
        fn op(){ let a=[7]; defer push(log,a[0]); defer gc_collect(); while true {} }
        op();
    "#,
        Some(100),
    );
    assert_eq!(result.unwrap_err(), "instruction limit exceeded");
    assert_eq!(numbers(&vm, "log"), vec![7.0]);
}

#[test]
fn gc_keeps_outer_pending_values_during_nested_caught_unwind() {
    let (vm, result) = vm_with_program(
        r#"
        let log=[];
        fn inner(){ let b=map{value:8}; defer push(log,b.value); defer gc_collect(); throw "secondary"; }
        fn middle(){ let ignored=try inner() catch nil; }
        fn outer(){ let a=[7]; defer push(log,a[0]); defer middle(); throw "primary"; }
        outer();
    "#,
        None,
    );
    assert_eq!(result.unwrap_err(), "primary");
    assert_eq!(numbers(&vm, "log"), vec![8.0, 7.0]);
}
