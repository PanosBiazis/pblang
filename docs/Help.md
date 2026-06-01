# Help.md — Cursor AI Code-Change Reference for pblang

> ⚠️ **STALE BATCH (as of 2026-05-30).** The "Batch Option 1: SP1-gamma — OOP +
> Functional Conformance Fixes" recommended below is **obsolete**: `class` and
> `match` are complete, native, self-hosting language features and there are no
> failing class/match semantics tests (semantics is 189/0/1; the only ignored
> case is `os_env_set_pass`). Treat the class/match "fix the failing tests"
> sections as historical. For the real current frontier (self-host breadth/perf),
> see `docs/CURRENT_STATUS.md` and `docs/Future_plans.md` (Updates 15–19).

This file tells Cursor AI **exactly which files to touch, what code to write/change,
and what tests to add** for the next recommended batch.
NOT a backlog — that lives in `docs/Tasks.md`.
Rewrite this file after every run to reflect the next batch.


---


## Status of Last Batch — COMPLETED ✓


| Task | What landed |
|------|-------------|
| **M4-beta** | `crates/pblang-pm/src/lib.rs`: `publish_package`, `search_registry`, `SearchResult`; `src/main.rs`: `pblang publish` + `pblang search <query>` subcommands; `tests/install_cli.rs`: `publish_and_search_roundtrip`; `docs/Registry_hosted.md` M4-beta section; `docs/Tooling.md` updated. |
| **R3-beta** | `src/main.rs`: `pblang --symbols <file>` symbol table dump (fn + let + bytecode offsets); `tests/symbols.rs`: `symbols_lists_function_names`; `Cargo.toml` [[test]] symbols; `docs/Tooling.md` R3-beta section. |
| **SP1-beta** | `tests/semantics/numeric_edge_test.sapp` + `numeric_edge_cases_pass`; `tests/semantics.rs` updated; `docs/Spec_freeze.md` numeric edge cases section (Infinity/NaN/f64 limits); `docs/Tooling.md` SP1-beta note. |
| **E5-beta** | `crates/pblang-stub/` new crate; `pblang bundle --self-extract`; `tests/pack.rs`; `docs/Native_backend_notes.md` Stage A status; `docs/Tooling.md` updated. |
| **L3-beta** | LSP `open_files` map + scope-aware completion (didOpen/didChange + `collect_local_names`); `tests/lsp.rs`; `docs/Tooling.md` updated. |
| **Q3-gamma** | `stdlib/quantum_sim.sapp`: `repetition_encode`, `repetition_decode`; `examples/quantum_bb84/main.sapp`; `tests/semantics/quantum_ecc_test.sapp` + `quantum_ecc_repetition_pass`; docs updated. |
| **Q3-beta** | `stdlib/quantum_sim.sapp`: `histogram`; `tests/semantics/quantum_histogram_test.sapp`; docs updated. |
| **G3-beta** | `stdlib/math.sapp`: `clamp`, `lerp`, `smoothstep`; `tests/semantics/math_ext_test.sapp`; docs updated. |
| **A4-beta** | `stdlib/http.sapp`: `post_stream_auth`; `stdlib/provider_registry.sapp`; test; docs updated. |
| **W4-beta+** | `stdlib/http_server.sapp`: `mw_rate_limit`; test; docs updated. |
| **W4-beta** | `stdlib/websocket.sapp`: `write_frame`; test; docs updated. |
| **R3-alpha** | `pblang --dataflow`; `tests/dataflow.rs`; docs updated. |
| **M4-alpha** | `fetch_and_install_package`; `pblang install <name>@<version>`; `tests/install_cli.rs`; docs updated. |
| **T5-alpha** | `stdlib/thread.sapp` wrapper; test; docs updated. |
| **G3-alpha** | `stdlib/collections.sapp`; test; docs updated. |
| **L3-alpha** | `pblang fmt` blank line between `fn`; test; docs updated. |
| **E5-alpha** | `docs/Native_backend_notes.md` Stage A+B design; cross-ref. |
| **W4-alpha** | `tcp_set_timeout`; http_server timeout/SSE; `docs/TLS_notes.md`; test. |
| **A4-alpha** | `get_stream`, `post_stream`, `parse_sse_line`; example; test. |
| **Q3-alpha** | `run(circ)`, `measurements_to_string`, `measurements_to_int`; test. |


Do **not** re-touch any of the above unless fixing a confirmed bug or explicit regression.


---


## Active Priority Order


1.W4 2.A4 3.Q3 4.E5 5.L3 6.G3 7.M4 8.T5 9.R3 10.SP1 11.PL1 12.ED1 13.PF1 14.N2 15.OS1

text


---


## Next Recommended Batch — Pick ONE


**Completed this run:** M4-beta (publish + search) + R3-beta (--symbols) + SP1-beta (numeric edge cases). **Next:** SP1-gamma or T5-beta or A4-gamma (pick from options below).


Complete **one primary** (optionally + up to 3 related follow-ups from the same area).
After completion: update Report.md, Tasks.md, rewrite Help.md.


---


## 🥇 Batch Option 1 (RECOMMENDED): SP1-gamma — OOP + Functional Conformance Fixes


### Goal
Fix the known failing semantics tests for **class** (×2) and **match** (×2), and strengthen
`docs/Spec_freeze.md` with OOP + functional sections. These are the highest-value
production-hardening items that unblock the conformance suite.


### 1a. Investigate & fix failing class semantics tests


First, locate the 2 failing class tests from batch 37. They are in `tests/semantics/` — likely
`class_basic_test.sapp` and `class_method_test.sapp` or similar. Run the suite to confirm names.


Common failure patterns to fix in `crates/pblang-vm/src/vm.rs` and/or `crates/pblang-compiler/src/`:

- **`auto_new` sugar / constructor dispatch:** if `MyClass(args)` isn't resolving to a constructor,
  add a `call_constructor` dispatch path that calls `fn new(...)` inside the class body.
- **Method resolution:** `obj.method(args)` must look up `method` in the class's method table,
  not the global scope. Fix the property-access + call combo to check `class_methods[class_name][method_name]`.
- **`self` binding:** inside a method call, `self` must be bound to the receiver object.
  Add `self` as an implicit first parameter in class method frames.


After the fix, add a regression guard in `tests/semantics.rs`:
```rust
#[test]
fn class_basic_pass() {
    run_semantics_test("tests/semantics/class_basic_test.sapp", "class_basic_test: pass");
}

#[test]
fn class_method_dispatch_pass() {
    run_semantics_test("tests/semantics/class_method_test.sapp", "class_method_test: pass");
}
1b. Investigate & fix failing match semantics tests
Locate the 2 failing match tests (likely match_exhaustive_test.sapp and match_guard_test.sapp).

Common failure patterns:

Match on map/struct values: match obj.field { ... } — ensure the match
expression evaluates nested property access before comparing arms.

Wildcard arm fallthrough: _ => arm must always execute if no prior arm matched;
check that the compiler emits a correct fallthrough jump.

Guard conditions: case x if x > 0 => — ensure guard is evaluated after arm binding,
and that a failed guard falls through to the next arm (not to the default).

After fix, add in tests/semantics.rs:

rust
#[test]
fn match_exhaustive_pass() {
    run_semantics_test("tests/semantics/match_exhaustive_test.sapp", "match_exhaustive_test: pass");
}

#[test]
fn match_guard_pass() {
    run_semantics_test("tests/semantics/match_guard_test.sapp", "match_guard_test: pass");
}
1c. docs/Spec_freeze.md — Add OOP + Functional sections (v0.2)
Append new sections:

text
## OOP Semantics (v0.2)


### Class declaration
```pblang
class MyClass {
    fn new(x) { self.x = x; }
    fn get_x() { return self.x; }
}
let obj = MyClass(42);
print obj.get_x(); // 42
class declares a named type with methods.

fn new(...) is the constructor; called when ClassName(args) is invoked.

self is implicitly bound to the receiver instance inside any method call.

Methods are looked up on the class, not the global scope.

Instance fields are set via self.field = value inside any method.

Method dispatch
obj.method(args) → resolves method from obj's class method table → calls with self = obj.

If method not found: runtime error "method '<name>' not found on <class>".

Functional Semantics
Closures
fn declared inside another fn captures outer bindings **by reference (upvalue)**.

Mutations to a captured variable are shared between the closure and the outer binding
in both directions (e.g. `make_counter()` returns a closure that increments and
returns a shared count). This matches `docs/Language_Semantics.md` ("Closures capture
variables by reference (upvalue), so mutations inside a closure affect the outer
binding"). (Note: closures passed to `spawn()` must have **closed** upvalues — see the
spawn rules in Language_Semantics.)

map / filter / reduce (stdlib:array.sapp)
array.map(arr, fn) → new array where each element is fn(element).

array.filter(arr, fn) → new array of elements where fn(element) is truthy.

array.reduce(arr, fn, init) → single value: fn(accumulator, element) for each element.

text


### 1d. `docs/Tooling.md` — Add SP1-gamma note


Append:


```markdown
## SP1-gamma: OOP + match conformance fixes


- Fixed class constructor dispatch (`ClassName(args)` → `fn new`) and method resolution
  (`obj.method()` → class method table with `self` binding).
- Fixed match fallthrough and guard evaluation.
- `docs/Spec_freeze.md` v0.2: OOP semantics, closure capture rules, map/filter/reduce contracts.
- Tests: `class_basic_pass`, `class_method_dispatch_pass`, `match_exhaustive_pass`, `match_guard_pass`.
🥈 Batch Option 2: T5-beta — Thread Pool Stdlib Helper + Deadlock Diagnostic
Goal
Add a thread_pool(n_workers, tasks) stdlib helper in stdlib/thread.sapp (builds on the
existing thread_pool example) and a deadlock diagnostic: detect recv-on-empty with all
senders closed, print a clear error instead of hanging forever.

2a. stdlib/thread.sapp — Add thread_pool
Append before export:

text
// thread_pool(n_workers, tasks_array, worker_fn)
// Spawns n_workers threads; each pulls a task from a shared channel and calls worker_fn(task).
// Returns an array of results (in completion order, not submission order).
// Constraint: values in tasks_array and results must be transferable (no closures/handles).
fn thread_pool(n_workers, tasks_array, worker_fn) {
    let task_ch = channel_new();
    let result_ch = channel_new();
    // Spawn workers
    for i n_workers {
        spawn(fn() {
            loop {
                let task = recv(task_ch);
                if task == nil { break; }  // nil = shutdown sentinel
                send(result_ch, worker_fn(task));
            }
        });
    }
    // Feed tasks
    for i len(tasks_array) {
        send(task_ch, tasks_array[i]);
    }
    // Send shutdown sentinels
    for i n_workers { send(task_ch, nil); }
    // Collect results
    let results = [];
    for i len(tasks_array) {
        results[i] = recv(result_ch);
    }
    return results;
}
Update export — add: thread_pool

2b. tests/semantics/thread_pool_stdlib_test.sapp — New test
text
// thread_pool_stdlib_test.sapp
import "stdlib:thread.sapp";


let tasks = ;
let results = thread.thread_pool(2, tasks, fn(x) { return x * 2; });
// Results may be unordered; check sum
let s = 0;
for i len(results) { s = s + results[i]; }
// Expected: 2+4+6+8+10 = 30
if s != 30 { print "FAIL thread_pool sum: " + str(s); }
print "thread_pool_stdlib_test: pass";
Add in tests/semantics.rs:

rust
#[test]
fn thread_pool_stdlib_pass() {
    run_semantics_test(
        "tests/semantics/thread_pool_stdlib_test.sapp",
        "thread_pool_stdlib_test: pass",
    );
}
2c. docs/Tooling.md — Add T5-beta section
Append under the threads section:

text
## T5-beta: Thread pool stdlib helper


```pblang
import "stdlib:thread.sapp";
let results = thread.thread_pool(4, tasks_array, fn(task) { return process(task); });
thread.thread_pool(n_workers, tasks_array, worker_fn) — spawns n_workers threads,
distributes tasks via an internal channel, collects results.

Results are returned in completion order (not submission order).

All tasks and results must be transferable values (numbers, strings, arrays, maps).

Uses nil sentinel for graceful worker shutdown.

text


---


## 🥉 Batch Option 3: A4-gamma — Chat Tool-call Example + Tensor Slice/Dot


### Goal
Add a concrete chat tool-call example (`examples/ai_chat_toolcall/`) with `function_call`
field handling, and extend `stdlib/tensor.sapp` with `slice`, `reshape`, and `dot` operations
(pure pblang, no new builtins needed).


### 3a. `examples/ai_chat_toolcall/main.sapp` — New file


```pblang
// examples/ai_chat_toolcall/main.sapp
// Demonstrates OpenAI-style tool-call request + response parsing.
import "stdlib:http.sapp";
import "stdlib:provider_registry.sapp";


// Register a provider
provider_registry.register("openai", "https://api.openai.com/v1/chat/completions",
    os.getenv("OPENAI_API_KEY"));


// Build a tool-call request
let tools = [map{
    type: "function",
    function: map{
        name: "get_weather",
        description: "Get current weather for a city",
        parameters: map{
            type: "object",
            properties: map{
                city: map{ type: "string" }
            },
            required: ["city"]
        }
    }
}];


let body = json_stringify(map{
    model: "gpt-4o",
    messages: [map{ role: "user", content: "What is the weather in Athens?" }],
    tools: tools,
    tool_choice: "auto"
});


let res = http.post_auth(
    "https://api.openai.com/v1/chat/completions",
    body,
    os.getenv("OPENAI_API_KEY")
);


let parsed = json_parse(res);
let choice = parsed.choices;
if choice.finish_reason == "tool_calls" {
    let call = choice.message.tool_calls;
    print "Tool called: " + call.function.name;
    print "Arguments: " + call.function.arguments;
} else {
    print "Response: " + choice.message.content;
}
3b. stdlib/tensor.sapp — Add slice, reshape, dot
Append before export:

text
// slice(t, start, end) -> new 1-D tensor with elements [start..end)
fn slice(t, start, end_idx) {
    let rows = end_idx - start;
    let cols = t.shape;
    let out = zeros([rows, cols]);
    for i rows {
        for j cols {
            set(out, i, j, get(t, start + i, j));
        }
    }
    return out;
}


// reshape(t, new_rows, new_cols) -> new tensor with same data, different shape
// Requires new_rows * new_cols == t.shape * t.shape.
fn reshape(t, new_rows, new_cols) {
    let out = zeros([new_rows, new_cols]);
    let total = new_rows * new_cols;
    let old_cols = t.shape;
    for k total {
        let old_r = num_div(k, old_cols);
        let old_c = num_mod(k, old_cols);
        let new_r = num_div(k, new_cols);
        let new_c = num_mod(k, new_cols);
        set(out, new_r, new_c, get(t, old_r, old_c));
    }
    return out;
}


// dot(a, b) -> matrix product (a: [m×k], b: [k×n]) -> [m×n]
fn dot(a, b) {
    let m = a.shape;
    let k = a.shape;
    let n = b.shape;
    let out = zeros([m, n]);
    for i m {
        for j n {
            let s = 0.0;
            for p k { s = s + get(a, i, p) * get(b, p, j); }
            set(out, i, j, s);
        }
    }
    return out;
}
Update export — add: slice, reshape, dot

3c. tests/semantics/tensor_ops_test.sapp — New test
text
// tensor_ops_test.sapp
import "stdlib:tensor.sapp";


// dot: [,] × [,] = [,]
let a = tensor.zeros();
tensor.set(a,0,0,1.0); tensor.set(a,0,1,2.0);
tensor.set(a,1,0,3.0); tensor.set(a,1,1,4.0);
let b = tensor.zeros();
tensor.set(b,0,0,5.0); tensor.set(b,0,1,6.0);
tensor.set(b,1,0,7.0); tensor.set(b,1,1,8.0);
let c = tensor.dot(a, b);
if tensor.get(c,0,0) != 19.0 { print "FAIL dot "; }
if tensor.get(c,1,1) != 50.0 { print "FAIL dot "; }


// reshape: 2×2 → 1×4
let r = tensor.reshape(a, 1, 4);
if r.shape != 1 { print "FAIL reshape rows"; }
if r.shape != 4 { print "FAIL reshape cols"; }


print "tensor_ops_test: pass";
Add in tests/semantics.rs:

rust
#[test]
fn tensor_ops_pass() {
    run_semantics_test(
        "tests/semantics/tensor_ops_test.sapp",
        "tensor_ops_test: pass",
    );
}
3d. docs/Tooling.md — Add A4-gamma section
Append under the AI section:

text
## A4-gamma: Chat tool-call example + tensor slice/reshape/dot


- **`examples/ai_chat_toolcall/main.sapp`** — OpenAI tool-call (function_call) request/response
  demo using `http.post_auth` and `provider_registry`. Parses `finish_reason == "tool_calls"`.
- **`tensor.slice(t, start, end)`** — extract row slice from a 2-D tensor.
- **`tensor.reshape(t, new_rows, new_cols)`** — reinterpret shape (same total elements).
- **`tensor.dot(a, b)`** — matrix product `[m×k] × [k×n] → [m×n]` (pure pblang).
Rules for Cursor AI
Read docs/Tasks.md and docs/Report.md before every run.

Touch only files listed for the chosen batch.

One primary task + up to 3 related follow-ups (same area, same/adjacent files).

Every behavior change needs a test. Every test must be cross-platform safe.

Do not touch unless fixing a bug:

stdlib/websocket.sapp (parse_frame, write_frame — existing)

stdlib/quantum_protocol.sapp (submit_remote/poll_remote)

src/main.rs (--annotate, --call-graph, lsp, --dataflow, --symbols — existing)

crates/pblang-pm/src/semver.rs

stdlib/csv.sapp, stdlib/graphics.sapp (existing functions)

crates/pblang-pm PackageSection (target_platform, min_runtime, validate_manifest_metadata)

examples/thread_pool/main.sapp, examples/quantum_bb84/main.sapp

docs/Registry_hosted.md, docs/Quantum_providers.md, docs/Embedding.md

docs/Registry_MVP.md, docs/Native_backend_notes.md

stdlib/provider_registry.sapp, stdlib/http_server.sapp, stdlib/math.sapp

stdlib/quantum_sim.sapp (existing functions), crates/pblang-stub/

crates/pblang-pm/src/lib.rs (publish_package, search_registry — existing)

Done = tests pass + no unrelated file changes + Report.md + Tasks.md + Help.md updated.

No uncontrolled refactors — only if they directly unblock the current batch.

No shared-heap threads; no AI/quantum as monolithic VM builtins; no syntax changes without decision + tests + docs.

File Map
Area	Key files
Area	Key files
VM core	crates/pblang-vm/src/vm.rs, crates/pblang-compiler/src/, crates/pblang-ir/src/
CLI entry	src/main.rs
Package manager	crates/pblang-pm/src/ (lib.rs, semver.rs)
Stub launcher	crates/pblang-stub/src/main.rs
Stdlib	stdlib/*.sapp
Tests (semantics)	tests/semantics/*.sapp + tests/semantics.rs
Tests (other)	tests/lsp.rs, tests/annotate.rs, tests/call_graph.rs, tests/lint.rs, tests/pack.rs, tests/install_cli.rs, tests/dataflow.rs, tests/symbols.rs
Docs	docs/Tooling.md, docs/Report.md, docs/Tasks.md, docs/Help.md
Examples	examples/ai_http_provider/, examples/ai_chat_toolcall/, examples/quantum_bridge_minimal/, examples/thread_pool/, examples/quantum_bb84/
Spec / Design	docs/Spec_freeze.md, docs/Embedding.md, docs/Registry_hosted.md, docs/Quantum_providers.md, docs/Native_backend_notes.md, docs/TLS_notes.md, docs/AI_plugin_pattern.md
Output Format Expected from Cursor AI (after each run)
Changed files (list)

Primary task completed

Follow-up tasks completed

Tests added

How to run / test commands

What was intentionally not done

What remains next

What changed in docs/Tasks.md

What changed in docs/Report.md

text
undefined
Prepared using GPT-5.4 Thinking