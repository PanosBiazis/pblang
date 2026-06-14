//! `pblang test` command handler.
//!
//! This is a behavior-preserving extraction from `src/main.rs`: test discovery,
//! filtering, planning, VM execution, and cargo-style reporting stay here so
//! the binary entrypoint can keep delegating command-specific work.

use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use pblang_compiler::compile;
use pblang_core::{parse_strict, strip_comments_mask, Stmt, StmtKind};
use pblang_vm::VM;

use super::diag::format_parse_error;
use crate::parse_capability_flags;

const TEST_WORKER_STACK_BYTES: usize = 16 * 1024 * 1024;

#[derive(Clone)]
struct PlannedTest {
    file_path: PathBuf,
    file_src: Arc<String>,
    file_stmts: Arc<Vec<Stmt>>,
    // None = run the whole file as one test (legacy fallback).
    // Some(name) = call `fn <name>()` after running the file.
    fn_name: Option<String>,
    id: String,
}

#[derive(Clone)]
struct PlannedGroup {
    index: usize,
    file_path: PathBuf,
    file_src: Arc<String>,
    file_stmts: Arc<Vec<Stmt>>,
    tests: Vec<GroupTest>,
}

#[derive(Clone)]
struct GroupTest {
    id: String,
    fn_name: Option<String>,
}

struct GroupRun {
    index: usize,
    lines: Vec<String>,
    passed: usize,
    failed: usize,
    failures: Vec<(String, String)>,
    timings: Vec<TestTiming>,
}

#[derive(Clone)]
struct TestTiming {
    id: String,
    duration: Duration,
}

pub(crate) fn handle_test(mut args: Vec<String>) {
    let stdlib_cfg = parse_capability_flags(&mut args);

    // --filter <substring>: run tests whose `<file_stem>::<fn_name>`
    // identifier contains this substring. --exact <id> requires an exact
    // id match. --skip <substring> excludes matching ids. Accept both
    // `--flag X` and `--flag=X`.
    let mut filter: Option<String> = None;
    let mut exact: Option<String> = None;
    let mut skips: Vec<String> = Vec::new();
    let mut quiet = false;
    let mut use_cache = false;
    let mut jobs = 1usize;
    let mut show_timings = false;
    let mut slow_limit = 0usize;
    let mut fail_fast = false;
    let mut list_only = false;
    let mut positional: Vec<String> = Vec::new();
    let mut i = 1usize;
    while i < args.len() {
        let a = &args[i];
        if let Some(rest) = a.strip_prefix("--filter=") {
            filter = Some(rest.to_string());
            i += 1;
        } else if a == "--filter" && i + 1 < args.len() {
            filter = Some(args[i + 1].clone());
            i += 2;
        } else if let Some(rest) = a.strip_prefix("--exact=") {
            exact = Some(rest.to_string());
            i += 1;
        } else if a == "--exact" && i + 1 < args.len() {
            exact = Some(args[i + 1].clone());
            i += 2;
        } else if let Some(rest) = a.strip_prefix("--skip=") {
            skips.push(rest.to_string());
            i += 1;
        } else if a == "--skip" && i + 1 < args.len() {
            skips.push(args[i + 1].clone());
            i += 2;
        } else if a == "--quiet" {
            quiet = true;
            i += 1;
        } else if a == "--cache" {
            use_cache = true;
            i += 1;
        } else if a == "--timings" {
            show_timings = true;
            i += 1;
        } else if let Some(rest) = a.strip_prefix("--slow=") {
            slow_limit = parse_count_flag("--slow", rest);
            show_timings = true;
            i += 1;
        } else if a == "--slow" && i + 1 < args.len() {
            slow_limit = parse_count_flag("--slow", &args[i + 1]);
            show_timings = true;
            i += 2;
        } else if let Some(rest) = a.strip_prefix("--jobs=") {
            jobs = parse_jobs(rest);
            i += 1;
        } else if a == "--jobs" && i + 1 < args.len() {
            jobs = parse_jobs(&args[i + 1]);
            i += 2;
        } else if a == "--fail-fast" {
            fail_fast = true;
            i += 1;
        } else if a == "--list" {
            list_only = true;
            i += 1;
        } else {
            positional.push(a.clone());
            i += 1;
        }
    }

    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let test_dir = positional
        .first()
        .map(|s| cwd.join(s))
        .unwrap_or_else(|| cwd.join("tests"));

    let mut entries = match std::fs::read_dir(&test_dir) {
        Ok(e) => e,
        Err(e) => {
            eprintln!(
                "pblang test: cannot read directory '{}': {}",
                test_dir.display(),
                e
            );
            std::process::exit(1);
        }
    }
    .filter_map(|e| e.ok())
    .map(|e| e.path())
    .filter(|p| {
        p.extension()
            .and_then(|e| e.to_str())
            .map(pblang_pm::is_module_ext)
            .unwrap_or(false)
    })
    .collect::<Vec<_>>();
    entries.sort_by(|a, b| a.file_name().cmp(&b.file_name()));

    // Plan the work first so we can print the cargo-style
    // "running N tests" line before any test runs.
    let mut plan: Vec<PlannedTest> = Vec::new();
    let mut planning_failures: Vec<(String, String)> = Vec::new();
    let mut discovered_total = 0usize;
    let matches_selection = |id: &str| -> bool {
        let filter_ok = filter.as_deref().map(|f| id.contains(f)).unwrap_or(true);
        let exact_ok = exact.as_deref().map(|e| id == e).unwrap_or(true);
        let skip_ok = skips.iter().all(|s| !id.contains(s));
        filter_ok && exact_ok && skip_ok
    };

    let test_cache_seed = if use_cache {
        Some(test_cache_seed(&cwd))
    } else {
        None
    };

    for path in &entries {
        let path_str = path.to_string_lossy().to_string();
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("test")
            .to_string();
        let src = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                planning_failures.push((stem.clone(), format!("read {path_str}: {e}")));
                continue;
            }
        };
        let masked = strip_comments_mask(&src);
        let stmts = match parse_strict(&masked) {
            Ok(s) => s,
            Err(errs) => {
                let msg = errs
                    .first()
                    .map(format_parse_error)
                    .unwrap_or_else(|| "parse error".into());
                planning_failures.push((stem.clone(), format!("parse: {msg}")));
                continue;
            }
        };

        // Discover top-level `fn test_*`. Backward-compat: if none,
        // run the whole file as a single implicit test.
        let test_fns: Vec<String> = stmts
            .iter()
            .filter_map(|st| match &st.kind {
                StmtKind::Fn(name, _, _, _) if name.starts_with("test_") => Some(name.clone()),
                _ => None,
            })
            .collect();

        if test_fns.is_empty() {
            let id = stem.clone();
            discovered_total += 1;
            if matches_selection(&id) {
                plan.push(PlannedTest {
                    file_path: path.clone(),
                    file_src: Arc::new(src),
                    file_stmts: Arc::new(stmts),
                    fn_name: None,
                    id,
                });
            }
        } else {
            // Share parsed+source state across test entries in the same file;
            // compile still receives an owned Vec once per file.
            let shared_src = Arc::new(src);
            let shared_stmts = Arc::new(stmts);
            for name in &test_fns {
                let id = format!("{stem}::{name}");
                discovered_total += 1;
                if matches_selection(&id) {
                    plan.push(PlannedTest {
                        file_path: path.clone(),
                        file_src: shared_src.clone(),
                        file_stmts: shared_stmts.clone(),
                        fn_name: Some(name.clone()),
                        id,
                    });
                }
            }
        }
    }

    let total = plan.len();
    let filtered_out = discovered_total.saturating_sub(total);
    if list_only {
        if filtered_out > 0 {
            println!("{} tests, {} filtered out", total, filtered_out);
        } else {
            println!("{} tests", total);
        }
        for t in &plan {
            println!("{}: test", t.id);
        }
        if !planning_failures.is_empty() {
            eprintln!();
            eprintln!("planning failures:");
            for (id, msg) in &planning_failures {
                eprintln!("    {id}: {msg}");
            }
            std::process::exit(1);
        }
        return;
    }

    println!();
    println!("running {} tests", total);

    let mut passed = 0usize;
    let mut failed = 0usize;
    let mut failures: Vec<(String, String)> = Vec::new();
    let mut timings: Vec<TestTiming> = Vec::new();
    let cache_path = test_dir.join(".pblang-test-cache");
    let mut cache = if use_cache {
        read_test_cache(&cache_path)
    } else {
        HashMap::new()
    };
    let mut cache_dirty = false;

    let mut parallel_done = false;
    if jobs > 1 && !use_cache && !fail_fast && planning_failures.is_empty() {
        let mut groups = planned_groups(&plan);
        if groups.len() > 1 {
            if jobs == usize::MAX {
                jobs = std::thread::available_parallelism()
                    .map(|n| n.get())
                    .unwrap_or(1);
            }
            jobs = jobs.max(1).min(groups.len());
            let (tx, rx) = std::sync::mpsc::channel::<GroupRun>();
            let mut active = 0usize;
            let mut next = 0usize;
            let mut results: Vec<Option<GroupRun>> = (0..groups.len()).map(|_| None).collect();

            while next < groups.len() || active > 0 {
                while active < jobs && next < groups.len() {
                    let group = groups[next].clone();
                    let cfg = stdlib_cfg.clone();
                    let tx = tx.clone();
                    std::thread::Builder::new()
                        .name(format!("pblang-test-{}", group.index))
                        .stack_size(TEST_WORKER_STACK_BYTES)
                        .spawn(move || {
                            let _ = tx.send(run_group(group, cfg, quiet, show_timings));
                        })
                        .expect("spawn pblang test worker");
                    next += 1;
                    active += 1;
                }
                let result = rx.recv().expect("pblang test worker channel closed");
                active -= 1;
                let index = result.index;
                results[index] = Some(result);
            }
            groups.clear();

            for result in results.into_iter().flatten() {
                for line in result.lines {
                    println!("{line}");
                }
                passed += result.passed;
                failed += result.failed;
                failures.extend(result.failures);
                timings.extend(result.timings);
            }
            parallel_done = true;
        }
    }

    if parallel_done {
        finish_test_run(
            passed,
            failed,
            filtered_out,
            &failures,
            &timings,
            show_timings,
            slow_limit,
        );
        if failed > 0 {
            std::process::exit(1);
        }
        return;
    }

    // Group consecutive entries by file so we only compile + load each file
    // once, then call each discovered test fn on the already-loaded VM.
    let mut idx = 0;
    while idx < plan.len() && !(fail_fast && failed > 0) {
        let file_path = plan[idx].file_path.clone();
        let file_src = plan[idx].file_src.clone();
        let file_stmts = (*plan[idx].file_stmts).clone();
        let group_start = idx;
        let mut group_end = idx;
        while group_end < plan.len() && plan[group_end].file_path == file_path {
            group_end += 1;
        }

        let cache_keys = if let Some(seed) = test_cache_seed.as_ref() {
            (group_start..group_end)
                .map(|pi| {
                    (
                        plan[pi].id.clone(),
                        test_cache_key(seed, &plan[pi].id, &file_path, &file_src),
                    )
                })
                .collect::<HashMap<_, _>>()
        } else {
            HashMap::new()
        };
        if use_cache
            && (group_start..group_end).all(|pi| cached_pass(&cache, &cache_keys, &plan[pi].id))
        {
            while idx < group_end {
                let id = plan[idx].id.clone();
                passed += 1;
                println!("test {id} ... ok");
                idx += 1;
            }
            continue;
        }

        let path_str = file_path.to_string_lossy().to_string();
        let load_start = Instant::now();
        let mut file_chunk = match compile(file_stmts) {
            Ok(c) => c,
            Err(e) => {
                while idx < plan.len() && plan[idx].file_path == file_path {
                    let id = plan[idx].id.clone();
                    failed += 1;
                    failures.push((id.clone(), format!("compile error: {e}")));
                    println!("test {id} ... FAILED");
                    idx += 1;
                    if fail_fast {
                        break;
                    }
                }
                continue;
            }
        };
        file_chunk.source_name = Some(Rc::new(path_str.clone()));
        file_chunk.source = Some(Rc::new((*file_src).clone()));

        let mut vm = VM::with_stdlib(stdlib_cfg.clone());
        let load_result = if quiet {
            vm.run_collect(file_chunk).map(|_| ())
        } else {
            vm.run(file_chunk)
        };
        let load_elapsed = load_start.elapsed();

        while idx < plan.len() && plan[idx].file_path == file_path && !(fail_fast && failed > 0) {
            let id = plan[idx].id.clone();
            let fn_name = plan[idx].fn_name.clone();
            idx += 1;

            if cached_pass(&cache, &cache_keys, &id) {
                passed += 1;
                let elapsed = Duration::ZERO;
                if show_timings {
                    timings.push(TestTiming {
                        id: id.clone(),
                        duration: elapsed,
                    });
                }
                println!("{}", status_line(&id, "ok", elapsed, show_timings));
                continue;
            }

            match (&load_result, &fn_name) {
                (Err(e), None) => {
                    // Legacy whole-file test: file load IS the test.
                    failed += 1;
                    failures.push((id.clone(), e.message.clone()));
                    if show_timings {
                        timings.push(TestTiming {
                            id: id.clone(),
                            duration: load_elapsed,
                        });
                    }
                    println!("{}", status_line(&id, "FAILED", load_elapsed, show_timings));
                }
                (Ok(()), None) => {
                    passed += 1;
                    if let Some(key) = cache_keys.get(&id) {
                        cache.insert(id.clone(), key.clone());
                        cache_dirty = true;
                    }
                    if show_timings {
                        timings.push(TestTiming {
                            id: id.clone(),
                            duration: load_elapsed,
                        });
                    }
                    println!("{}", status_line(&id, "ok", load_elapsed, show_timings));
                }
                (Err(e), Some(_)) => {
                    failed += 1;
                    failures.push((id.clone(), format!("file load error: {}", e.message)));
                    if show_timings {
                        timings.push(TestTiming {
                            id: id.clone(),
                            duration: load_elapsed,
                        });
                    }
                    println!("{}", status_line(&id, "FAILED", load_elapsed, show_timings));
                }
                (Ok(()), Some(name)) => {
                    // Synthesize a tiny chunk that calls `<name>()` on the
                    // already-loaded VM, then returns nil.
                    let mut call_chunk = pblang_ir::chunk::Chunk::default();
                    let name_const = call_chunk.add_const(pblang_ir::alloc_string(name.clone()));
                    let nil_const = call_chunk.add_const(pblang_ir::Value::Nil);
                    let sp = pblang_ir::chunk::Span::default();
                    call_chunk.write(pblang_ir::OpCode::OpGetGlobal(name_const), sp);
                    call_chunk.write(pblang_ir::OpCode::OpCall(0), sp);
                    call_chunk.write(pblang_ir::OpCode::OpPop, sp);
                    call_chunk.write(pblang_ir::OpCode::OpConstant(nil_const), sp);
                    call_chunk.write(pblang_ir::OpCode::OpReturn, sp);
                    call_chunk.source_name = Some(Rc::new(path_str.clone()));

                    let test_start = Instant::now();
                    let call_result = if quiet {
                        vm.run_collect(call_chunk).map(|_| ())
                    } else {
                        vm.run(call_chunk)
                    };
                    let elapsed = test_start.elapsed();
                    match call_result {
                        Ok(()) => {
                            passed += 1;
                            if let Some(key) = cache_keys.get(&id) {
                                cache.insert(id.clone(), key.clone());
                                cache_dirty = true;
                            }
                            if show_timings {
                                timings.push(TestTiming {
                                    id: id.clone(),
                                    duration: elapsed,
                                });
                            }
                            println!("{}", status_line(&id, "ok", elapsed, show_timings));
                        }
                        Err(e) => {
                            failed += 1;
                            failures.push((id.clone(), e.message.clone()));
                            if show_timings {
                                timings.push(TestTiming {
                                    id: id.clone(),
                                    duration: elapsed,
                                });
                            }
                            println!("{}", status_line(&id, "FAILED", elapsed, show_timings));
                        }
                    }
                }
            }
        }
    }

    for (id, msg) in &planning_failures {
        if fail_fast && failed > 0 {
            break;
        }
        failed += 1;
        failures.push((id.clone(), msg.clone()));
        println!("test {id} ... FAILED");
        if fail_fast {
            break;
        }
    }

    finish_test_run(
        passed,
        failed,
        filtered_out,
        &failures,
        &timings,
        show_timings,
        slow_limit,
    );
    if failed > 0 {
        std::process::exit(1);
    }
    if cache_dirty {
        if let Err(e) = write_test_cache(&cache_path, &cache) {
            eprintln!("pblang test: warning: failed to write test cache: {e}");
        }
    }
}

fn parse_jobs(value: &str) -> usize {
    if value == "auto" {
        return usize::MAX;
    }
    match value.parse::<usize>() {
        Ok(0) => usize::MAX,
        Ok(n) => n,
        Err(_) => {
            eprintln!("pblang test: --jobs expects a positive integer, 0, or auto");
            std::process::exit(1);
        }
    }
}

fn parse_count_flag(flag: &str, value: &str) -> usize {
    match value.parse::<usize>() {
        Ok(n) => n,
        Err(_) => {
            eprintln!("pblang test: {flag} expects a non-negative integer");
            std::process::exit(1);
        }
    }
}

fn status_line(id: &str, status: &str, elapsed: Duration, show_timings: bool) -> String {
    if show_timings {
        format!(
            "test {id} ... {status} ({:.3} ms)",
            elapsed.as_secs_f64() * 1000.0
        )
    } else {
        format!("test {id} ... {status}")
    }
}

fn planned_groups(plan: &[PlannedTest]) -> Vec<PlannedGroup> {
    let mut groups = Vec::new();
    let mut idx = 0usize;
    while idx < plan.len() {
        let file_path = plan[idx].file_path.clone();
        let file_src = plan[idx].file_src.clone();
        let file_stmts = plan[idx].file_stmts.clone();
        let mut tests = Vec::new();
        while idx < plan.len() && plan[idx].file_path == file_path {
            tests.push(GroupTest {
                id: plan[idx].id.clone(),
                fn_name: plan[idx].fn_name.clone(),
            });
            idx += 1;
        }
        groups.push(PlannedGroup {
            index: groups.len(),
            file_path,
            file_src,
            file_stmts,
            tests,
        });
    }
    groups
}

fn run_group(
    group: PlannedGroup,
    stdlib_cfg: pblang_vm::StdlibConfig,
    quiet: bool,
    show_timings: bool,
) -> GroupRun {
    let mut lines = Vec::new();
    let mut passed = 0usize;
    let mut failed = 0usize;
    let mut failures = Vec::new();
    let mut timings = Vec::new();
    let path_str = group.file_path.to_string_lossy().to_string();
    let load_start = Instant::now();
    let mut file_chunk = match compile((*group.file_stmts).clone()) {
        Ok(c) => c,
        Err(e) => {
            let elapsed = load_start.elapsed();
            for test in &group.tests {
                failed += 1;
                failures.push((test.id.clone(), format!("compile error: {e}")));
                if show_timings {
                    timings.push(TestTiming {
                        id: test.id.clone(),
                        duration: elapsed,
                    });
                }
                lines.push(status_line(&test.id, "FAILED", elapsed, show_timings));
            }
            return GroupRun {
                index: group.index,
                lines,
                passed,
                failed,
                failures,
                timings,
            };
        }
    };
    file_chunk.source_name = Some(Rc::new(path_str.clone()));
    file_chunk.source = Some(Rc::new((*group.file_src).clone()));

    let mut vm = VM::with_stdlib(stdlib_cfg);
    let load_result = vm.run_collect(file_chunk);
    let load_elapsed = load_start.elapsed();
    match load_result {
        Ok(out) => {
            if !quiet {
                lines.extend(out.into_iter().map(|v| v.to_string()));
            }
        }
        Err(e) => {
            for test in &group.tests {
                failed += 1;
                let msg = if test.fn_name.is_none() {
                    e.message.clone()
                } else {
                    format!("file load error: {}", e.message)
                };
                failures.push((test.id.clone(), msg));
                if show_timings {
                    timings.push(TestTiming {
                        id: test.id.clone(),
                        duration: load_elapsed,
                    });
                }
                lines.push(status_line(&test.id, "FAILED", load_elapsed, show_timings));
            }
            return GroupRun {
                index: group.index,
                lines,
                passed,
                failed,
                failures,
                timings,
            };
        }
    }

    for test in &group.tests {
        match &test.fn_name {
            None => {
                passed += 1;
                if show_timings {
                    timings.push(TestTiming {
                        id: test.id.clone(),
                        duration: load_elapsed,
                    });
                }
                lines.push(status_line(&test.id, "ok", load_elapsed, show_timings));
            }
            Some(name) => {
                let mut call_chunk = pblang_ir::chunk::Chunk::default();
                let name_const = call_chunk.add_const(pblang_ir::alloc_string(name.clone()));
                let nil_const = call_chunk.add_const(pblang_ir::Value::Nil);
                let sp = pblang_ir::chunk::Span::default();
                call_chunk.write(pblang_ir::OpCode::OpGetGlobal(name_const), sp);
                call_chunk.write(pblang_ir::OpCode::OpCall(0), sp);
                call_chunk.write(pblang_ir::OpCode::OpPop, sp);
                call_chunk.write(pblang_ir::OpCode::OpConstant(nil_const), sp);
                call_chunk.write(pblang_ir::OpCode::OpReturn, sp);
                call_chunk.source_name = Some(Rc::new(path_str.clone()));

                let test_start = Instant::now();
                match vm.run_collect(call_chunk) {
                    Ok(out) => {
                        let elapsed = test_start.elapsed();
                        if !quiet {
                            lines.extend(out.into_iter().map(|v| v.to_string()));
                        }
                        passed += 1;
                        if show_timings {
                            timings.push(TestTiming {
                                id: test.id.clone(),
                                duration: elapsed,
                            });
                        }
                        lines.push(status_line(&test.id, "ok", elapsed, show_timings));
                    }
                    Err(e) => {
                        let elapsed = test_start.elapsed();
                        failed += 1;
                        failures.push((test.id.clone(), e.message.clone()));
                        if show_timings {
                            timings.push(TestTiming {
                                id: test.id.clone(),
                                duration: elapsed,
                            });
                        }
                        lines.push(status_line(&test.id, "FAILED", elapsed, show_timings));
                    }
                }
            }
        }
    }

    GroupRun {
        index: group.index,
        lines,
        passed,
        failed,
        failures,
        timings,
    }
}

fn finish_test_run(
    passed: usize,
    failed: usize,
    filtered_out: usize,
    failures: &[(String, String)],
    timings: &[TestTiming],
    show_timings: bool,
    slow_limit: usize,
) {
    if !failures.is_empty() {
        println!();
        println!("failures:");
        println!();
        for (id, msg) in failures {
            println!("---- {id} stdout ----");
            println!("{msg}");
            println!();
        }
        println!("failures:");
        for (id, _) in failures {
            println!("    {id}");
        }
    }

    println!();
    let verdict = if failed == 0 { "ok" } else { "FAILED" };
    println!(
        "test result: {verdict}. {passed} passed; {failed} failed; \
         0 ignored; 0 measured; {filtered_out} filtered out"
    );
    if show_timings && !timings.is_empty() && slow_limit > 0 {
        let mut slow = timings.to_vec();
        slow.sort_by(|a, b| b.duration.cmp(&a.duration).then_with(|| a.id.cmp(&b.id)));
        slow.truncate(slow_limit.min(slow.len()));
        println!();
        println!("slow tests:");
        for t in slow {
            println!("    {:.3} ms  {}", t.duration.as_secs_f64() * 1000.0, t.id);
        }
    }
}

fn read_test_cache(path: &std::path::Path) -> HashMap<String, String> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return HashMap::new();
    };
    let mut out = HashMap::new();
    for line in text.lines() {
        let Some((id, key)) = line.split_once('\t') else {
            continue;
        };
        if !id.is_empty() && !key.is_empty() {
            out.insert(id.to_string(), key.to_string());
        }
    }
    out
}

fn cached_pass(
    cache: &HashMap<String, String>,
    cache_keys: &HashMap<String, String>,
    id: &str,
) -> bool {
    cache_keys
        .get(id)
        .and_then(|key| cache.get(id).map(|cached| cached == key))
        .unwrap_or(false)
}

fn write_test_cache(
    path: &std::path::Path,
    cache: &HashMap<String, String>,
) -> std::io::Result<()> {
    let mut entries = cache.iter().collect::<Vec<_>>();
    entries.sort_by(|a, b| a.0.cmp(b.0));
    let mut out = String::new();
    for (id, key) in entries {
        out.push_str(id);
        out.push('\t');
        out.push_str(key);
        out.push('\n');
    }
    std::fs::write(path, out)
}

fn test_cache_seed(cwd: &std::path::Path) -> String {
    let mut h = TestHasher::new();
    h.update_str("pblang-test-cache-v1");
    h.update_str(env!("CARGO_PKG_VERSION"));
    if let Ok(exe) = std::env::current_exe() {
        h.update_str(&exe.to_string_lossy());
        if let Ok(meta) = std::fs::metadata(exe) {
            h.update_str(&format!("exe_len={}", meta.len()));
            if let Ok(modified) = meta.modified() {
                if let Ok(dur) = modified.duration_since(std::time::UNIX_EPOCH) {
                    h.update_str(&format!("exe_mtime={}", dur.as_nanos()));
                }
            }
        }
    }
    let stdlib = cwd.join("stdlib");
    if stdlib.is_dir() {
        let mut files = walkdir::WalkDir::new(stdlib)
            .follow_links(false)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|e| e.file_type().is_file())
            .map(|e| e.path().to_path_buf())
            .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("sapp"))
            .collect::<Vec<_>>();
        files.sort();
        for path in files {
            h.update_str(&path.to_string_lossy());
            if let Ok(bytes) = std::fs::read(path) {
                h.update(&bytes);
            }
        }
    }
    h.finish()
}

fn test_cache_key(seed: &str, id: &str, file_path: &std::path::Path, file_src: &str) -> String {
    let mut h = TestHasher::new();
    h.update_str(seed);
    h.update_str(id);
    h.update_str(&file_path.to_string_lossy());
    h.update(file_src.as_bytes());
    h.finish()
}

struct TestHasher {
    a: u64,
    b: u64,
}

impl TestHasher {
    fn new() -> Self {
        Self {
            a: 0xcbf29ce484222325,
            b: 0x84222325cbf29ce4,
        }
    }

    fn update(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.a ^= u64::from(*byte);
            self.a = self.a.wrapping_mul(0x100000001b3);
            self.b ^= u64::from(*byte).rotate_left(1);
            self.b = self.b.wrapping_mul(0x9e3779b185ebca87);
        }
    }

    fn update_str(&mut self, value: &str) {
        self.update(value.as_bytes());
        self.update(&[0]);
    }

    fn finish(&self) -> String {
        format!("{:016x}{:016x}", self.a, self.b)
    }
}
