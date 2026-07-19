# pblang Safety Guide

## The Problem

pblang's **runtime (VM, compiler, GC)** is written in Rust and benefits from all of Rust's safety guarantees:
- **Memory safety**: no use-after-free, no buffer overflows, no double-free
- **Thread safety**: `Send`/`Sync` enforce correct sharing
- **No undefined behaviour**: the VM itself cannot crash from bad state

However, **code written in pblang** does not automatically inherit these guarantees. This document explains the gaps and how to close them.

---

## The Safety Gap (Rust vs pblang)

| Rust Feature | pblang Equivalent | Status |
|---|---|---|
| `let` = immutable by default | `let` is mutable | ⚠️ Use `const` |
| `Option<T>` forces nil handling | nil is implicit | ⚠️ Use `?.` or `option.sapp` |
| `Result<T, E>` forces error handling | Errors ignored unless checked | ⚠️ Use `result.sapp` + `?` |
| Borrow checker (no data races) | VM-per-thread isolation | ✅ Already safe |
| Static type system | Optional type annotations | 🔶 Enforced by `pblang lint` |
| `assert!` macro | `assert(cond, msg)` builtin | ✅ Available |
| Integer overflow = panic/checked | f64 saturates to Inf | ⚠️ Use `checked_*` builtins |
| No null pointers | `nil` is implicit | ⚠️ Use `non_null()` or `?.` |
| `freeze`/`const` values | `freeze()` builtin | ✅ Available |
| Move semantics | RC + copy-on-channel | ✅ Channels are safe |

---

## 1. Immutability — `const` keyword

### Problem
In pblang, ALL `let` bindings are mutable by default (like C, not like Rust).

```pblang
// Rust: let x = 5;   x = 6; // ERROR: cannot mutate immutable binding
// pblang (default):
let x = 5;
x = 6;  // ALLOWED — accidental mutation possible
```

### Solution: `const`
Declare bindings that should never change with `const`:

```pblang
const PI = 3.14159265358979;
const MAX_RETRIES = 3;
const CONFIG = map{ host: "localhost", port: 8080 };

PI = 3;         // COMPILE ERROR: cannot assign to `PI`: declared as `const`
MAX_RETRIES = 5; // COMPILE ERROR
```

The compiler enforces immutability at **compile time** — no runtime overhead.

### Best Practice
- Use `const` for configuration, constants, and values that must not change
- Use `let` only for values that will be reassigned
- This mirrors Rust's `let` (immutable) vs `let mut` (mutable) pattern

---

## 2. Nil Safety — `?.` Operator

### Problem
Accessing attributes on `nil` causes a runtime error:

```pblang
let user = nil;
let name = user.name;  // RUNTIME ERROR: attr on nil
```

### Solution: `?.` Nil-safe Access Operator
The `?.` operator returns `nil` instead of crashing:

```pblang
let user = nil;
let name = user?.name;     // nil — no crash
let city = user?.address?.city;  // nil — chains safely

let users = [map{ name: "Alice" }, nil];
let name0 = users[0]?.name;  // "Alice"
let name1 = users[1]?.name;  // nil — no crash
```

The `?.` is equivalent to Rust's `Option::map` or TypeScript's `?.` optional chaining.

### `?[idx]` Nil-safe Indexing

```pblang
let arr = nil;
let first = arr?[0];   // nil — no crash instead of "index nil"
```

---

## 3. Error Handling — `?` Operator and `result.sapp`

### Problem
Functions that return error maps are easy to ignore:

```pblang
let r = http_get("https://bad-url");  // might return "error: ..."
let data = json_parse(r.body);        // crashes if r is an error string!
```

### Solution: `result.sapp` + `?` Operator

#### The `result.sapp` pattern (like Rust's `Result<T,E>`):

```pblang
import "stdlib:result.sapp";

fn safe_fetch(url) {
    let raw = http_get(url);
    if str_starts_with(raw, "error:") {
        return result.err(raw);
    }
    return result.ok(raw);
}

let r = safe_fetch("https://api.example.com/data");
if r.ok {
    print r.value;
} else {
    print "Failed: " + r.error;
}
```

#### The `?` operator (throws on Err, unwraps Ok):

```pblang
import "stdlib:result.sapp";

// `expr?` throws RuntimeError if expr is result.err(...)
// `expr?` returns expr.value if expr is result.ok(...)

let raw = safe_fetch("https://api.example.com/data")?;  // throws on failure
let data = json_parse(raw);  // safe — raw is always a successful value here
```

The `?` operator is like Rust's `?` — it forces you to handle errors at the call site.

---

## 4. Type Safety — `type_assert` and Type Annotations

### Problem
pblang is dynamically typed — wrong types cause runtime errors, not compile-time errors.

### Solution A: Type annotations (enforced by `pblang lint`)

```pblang
fn add(a: number, b: number) -> number {
    return a + b;
}

let x: string = "hello";
let y: number = 42;
```

Run `pblang lint file.sapp` to catch type mismatches before running.

### Solution B: Runtime type assertions

```pblang
fn process_order(order) {
    type_assert(order, "map");         // throws TypeError if not a map
    type_assert(order.amount, "number"); // throws TypeError if not a number
    require_type(order.id, "string");  // like type_assert but returns the value
    
    // Now safe to use order.amount and order.id
    let total = order.amount * 1.1;
}
```

### Solution C: `const` enforces type stability

```pblang
const HOST: string = "localhost";  // type annotation enforced by typechecker
```

---

## 5. Frozen (Immutable) Data

### Problem
In pblang, maps and arrays are passed by reference. Any function can mutate them:

```pblang
let config = map{ host: "localhost", port: 8080 };
dangerous_fn(config);  // might mutate config!
```

### Solution: `freeze()`

> **Note:** `freeze()` enforcement was added in the v0.5 security audit.
> Both `SetAttr` (`.field =`) and `SetIndex` (`[key] =`) now raise a `FrozenError`.

```pblang
let config = freeze(map{ host: "localhost", port: 8080 });
config.host = "evil.com";  // RUNTIME ERROR: FrozenError: cannot set attribute 'host' on a frozen map

// Check if frozen:
if is_frozen(config) {
    print "config is immutable";
}
```

Frozen maps protect against accidental mutation — like Rust's immutable references.

### `frozen_copy` — Deep Copy and Freeze

```pblang
import "stdlib:safety.sapp";

// Create a frozen copy of a map (deep copy via JSON round-trip + freeze)
let safe_config = safety.frozen_copy(raw_config);
```

---

## 6. Arithmetic Safety

### Problem
f64 arithmetic in pblang silently produces `Inf` or `NaN` on overflow/division by zero:

```pblang
let x = 1e308 * 10;  // Inf — silent overflow
let y = 1 / 0;       // Inf — silent division by zero (not an error in f64)
```

### Solution: `checked_*` builtins

```pblang
let r = checked_div(10, 0);
if r.ok == false {
    print "Division by zero: " + r.error;
} else {
    print r.value;
}

let r2 = checked_add(1e308, 1e308);
if r2.ok == false {
    print "Overflow detected";
}

// Or use the safety module:
import "stdlib:safety.sapp";
let result = safety.safe_div(total, count);
let value = result?;  // throws if division by zero

// Clamp values to safe ranges:
let safe_index = safety.clamp(user_input, 0, len(array) - 1);
let item = array[safe_index];  // guaranteed safe!
```

---

## 7. Thread Safety

pblang's threading model is **VM-per-thread** — each spawned task gets its own VM with no shared heap. This is similar to Rust's `Send` guarantee applied to ALL values.

### What's safe to send over channels:
- `number`, `bool`, `string`, `nil` ✅
- `array` (deep-copied) ✅
- `map` (deep-copied) ✅

### What's NOT safe to send:
- TCP/SQLite/FFI handles — these are thread-local
- Module references

```pblang
let ch = channel();

let h = spawn(fn() {
    let data = recv(ch.receiver);
    // data is a DEEP COPY — no shared state
    print data.value;
});

send(ch.sender, map{ value: 42 });
join(h);
```

### Safe channel usage with `safety.sapp`:

```pblang
import "stdlib:safety.sapp";

// Doesn't throw if channel is already closed
let ok = safety.safe_send(ch.sender, data);
if ok.ok == false {
    print "Send failed: " + ok.error;
}
```

### Bounded worker supervision

`stdlib:thread.sapp` provides a bounded supervisor for worker lifecycle and
deadline accounting:

```pblang
let thread = import("stdlib:thread.sapp");
let sup = thread.supervisor();
let id = sup.spawn_cooperative(fn(token) {
    while not token.requested(token) {
        do_one_bounded_step();
    }
}, 5000);

sup.cancel(id, "shutdown");
let status = sup.wait(id, 1000);
```

The token-local poll function avoids capturing an imported module across the VM
boundary; `thread.cancellation_requested(token)` is the equivalent direct API.
Supervisor state is owned and updated by the parent VM; worker VMs receive
cancellation over a buffered channel and send bounded completion records that
state whether cancellation was actually observed. Timeout and cancellation
metrics are counted once, malformed
tokens fail closed, cancellation reasons are bounded, and at most 1,024 records
are retained. A deadline cannot safely preempt an arbitrary VM thread:
`stats.detached` counts timed-out workers that have not exited, and `kill_all`
reports `preemptive: false`. Untrusted code or blocking filesystem, network,
FFI, driver, or device calls require a bounded API at that boundary or an OS
process that can be terminated independently.

---

## 8. Bounds Safety

```pblang
import "stdlib:safety.sapp";

let arr = [1, 2, 3];

// Safe — returns nil instead of crashing
let v = safety.bounds_check(arr, 10);  // nil
let first = safety.first_safe(arr);    // 1
let last  = safety.last_safe(arr);     // 3

// nil-safe chaining with ?.
let users = get_users();          // might return nil
let name = users?[0]?.name;      // nil if users is nil, array empty, or user has no name
```

---

## 9. Non-Null Contract Enforcement

```pblang
import "stdlib:safety.sapp";

fn process(user) {
    // Enforce non-null contract at function boundary
    user = safety.non_null(user, "user");
    
    // Use expect for fields that must exist
    let name = safety.expect(user?.name, "user.name is required");
    
    // Now safe to use name
    print "Hello, " + name;
}
```

---

## 10. Debug Assertions and Invariants

```pblang
import "stdlib:safety.sapp";

fn binary_search(arr, target) {
    // Precondition invariant (like Rust's debug_assert!)
    safety.invariant(type(arr) == "array", "binary_search: arr must be an array");
    safety.invariant(len(arr) > 0, "binary_search: arr must not be empty");
    
    let lo = 0;
    let hi = len(arr) - 1;
    
    while lo <= hi {
        let mid = num_floor((lo + hi) / 2);
        // Loop invariant
        safety.invariant(mid >= 0 and mid < len(arr), "mid out of bounds");
        
        if arr[mid] == target { return mid; }
        if arr[mid] < target { lo = mid + 1; }
        else { hi = mid - 1; }
    }
    return -1;
}

fn not_yet_implemented() {
    safety.todo("implement binary_search for strings");
}

fn handle(event_type) {
    if event_type == "click" { ... }
    else if event_type == "hover" { ... }
    else {
        // Should never reach here
        safety.unreachable("unknown event type: " + event_type);
    }
}
```

---

## 11. Complete Example: Safe HTTP Client

```pblang
import "stdlib:result.sapp";
import "stdlib:safety.sapp";
import "stdlib:validate.sapp";

// Type-safe, null-safe, error-safe HTTP request
fn fetch_user(user_id) {
    // Input validation
    safety.invariant(type(user_id) == "number", "user_id must be a number");
    safety.invariant(user_id > 0, "user_id must be positive");
    
    // Safe HTTP call
    let raw = http_get("https://api.example.com/users/" + str(user_id));
    
    // Error check
    if raw == nil {
        return result.err("Network error: no response");
    }
    
    // Parse response
    let data = json_parse(raw);
    if data == nil {
        return result.err("Parse error: invalid JSON");
    }
    
    // Validate structure
    let schema = validate.schema(map{
        id:    map{ type: "number", required: true },
        name:  map{ type: "string", required: true },
        email: map{ type: "string", pattern: "@" }
    });
    let validation = validate.check(schema, data);
    if validation.ok == false {
        return result.err("Validation failed: " + str(validation.errors));
    }
    
    return result.ok(data);
}

// Caller uses ? operator for clean error propagation
fn main() {
    try {
        let user = fetch_user(42)?;  // throws if error
        print "Got user: " + user?.name;
    } catch "user not found" {
        print "User 42 not found";
    }
}
```

---

## 12. Self-Hosting (in place today)

pblang **already self-hosts**: the compiler under `stdlib/compiler/` (tokenizer → parser → semantic → typeck → ir → optimizer → codegen, plus `selfhost_frontend.sapp` and the `vm.sapp` interpreter) tokenizes, parses, compiles, and runs pblang end-to-end, all written in pblang. As that pblang-written compiler is maintained and extended, safety is maintained through:

1. **`const` everywhere** for VM state that must not change
2. **`freeze()`** for immutable data structures (AST nodes, IR chunks)
3. **`result.sapp`** for all error-returning operations (parse, compile, execute)
4. **`type_assert()`** at all public function boundaries
5. **`invariant()`** for loop invariants and preconditions
6. **`safety.sapp`** throughout for safe operations
7. **`validate.sapp`** for all external input validation (source files, config)

The runtime's memory safety (no leaks, no cycles) is already handled by the RC + cycle-breaking GC.

---

## 13. Bounded memory and device-facing work

The runtime ownership tracker is complemented by pblang-only admission APIs:

```pblang
let mem = import("stdlib:memory_safety.sapp");
let budget = import("stdlib:resource_budget.sapp");
let device = import("stdlib:device.sapp");
let runtime = import("stdlib:product_runtime.sapp");

let arena = mem.bounded_arena("packet-decode", 1024, 1048576);
let checkpoint = mem.arena_checkpoint(arena);
let packet = mem.arena_alloc_sized(arena, decoded, packet_bytes);
mem.arena_rewind(arena, checkpoint); // releases speculative children

let ledger = budget.ledger(budget.profile("micro"));
if not budget.ledger_try_reserve(ledger, map{ memory_bytes: 4096, items: 1 }) {
    throw "resource budget exceeded";
}

let policy = device.policy("readonly");
let decision = device.decide(map{
    path: "/proc/version", action: "read", policy: policy
});

let guarded = runtime.run_once("pb-mcp-server", "request",
    map{ workers: 1, tokens: 128 },
    fn() { return handle_request(); });
```

`scope_result` always restores lifetime depth and preserves success/failure as a
Result. `harden.with_secret_result` scrubs mutable byte arrays on both callback
success and failure. Resource arithmetic uses subtraction-first admission and
rejects quantities beyond pblang's exact integer range. Device policy is
deny-by-default, uses lexical allowlists, validates output/request size, and
rejects streaming `/dev` reads from the high-level safe reader.

These mechanisms are portable policy code, but they do **not** prove every
physical device or native driver safe. Raw FFI, generated syscalls, `ioctl`,
`mmap`, DMA, vendor GPU/NPU libraries, kernels, firmware, and hardware remain
separate trust boundaries. The current file builtin also obtains a finite file
before the pblang policy can inspect its returned length; genuinely bounded
streaming-device reads require a sized native read backend.

`product_runtime.sapp` supplies shared profiles for pblang compile/codegen,
PBIDE preview/run, pbAI inference/learning, pbOS services/restarts, and
pb-mcp-server dispatch. Unknown product/workload pairs fail closed. Per-call
overrides may only tighten enabled limits. `run` reserves resources before the
callback and always releases both the ledger reservation and memory lifetime.
Snapshots defensively copy mutable usage/limit maps so later accounting cannot
rewrite previously returned diagnostics. Use `try_admit` in hot loops and
`admit` when a full violation report is needed.

The guard is deterministic pblang policy. It cannot preempt a hostile process
or enforce physical RSS, CPU, GPU, DMA, storage, or network quotas by itself;
PBIDE and production launchers must pair it with OS process limits, timeouts,
capabilities, and device isolation.

`compute.sapp` adds bounded, deterministic routing rather than treating detected
hardware as permission to execute. CPU workers are capped and selected only
when caller-provided work hints amortize launch cost. GPU/NPU/SIMD/distributed
routes additionally require a registered equivalent runtime, compatible
workload, bounded estimated transfer, and enough estimated work. These hints
prevent obvious counterproductive offloads but do not measure driver queues,
thermal throttling, NUMA, physical memory, or device isolation. The controlled
storage reader bounds path metadata and worker count; because `fs_read` returns
whole files, it does not impose a byte limit on file contents.

`cache.sapp` provides `bounded(ttl_ms, max_entries, max_units)` for long-lived
servers, IDEs, and model processes. It uses O(1) entry/unit counters, safe TTL
deadline arithmetic, generation-tagged FIFO records, atomic oversized-value
rejection, and observable hit/miss/eviction/expiry metrics. The compatible
default remains FIFO; `bounded_policy(..., "lru")` adds exact recency eviction
at a measured per-hit bookkeeping cost. Caller-defined
`units` must represent the resource relevant to that cache; this avoids an
unbounded deep-size traversal but does not turn units into measured physical
RSS. Unbounded `new(ttl_ms)` remains for compatibility and should not receive
untrusted-cardinality keys without an external limit.

`data_store.sapp` applies those bounds to process memory and enforces equivalent
per-namespace entry/unit admission in SQLite. Its Redis backend uses bounded
RESP2 framing without shell execution. Redis-wide maxmemory, durability,
replication, ACLs, and eviction remain server-side policy; the client does not
claim to enforce them. See `docs/Data_Storage.md` for the full trust boundary.

`cdn_cache.sapp` adds a separate bounded public-response layer. It includes the
host, path, sorted query, and configured Vary dimensions in a SHA-256 key;
bypasses Authorization/Cookie/Range by default; rejects private/no-store,
Set-Cookie, unsafe Vary, oversized, and hostile-header responses; and bounds
edge/regional entries, units, key memoization, objects, queries, and tags.
Sensitive bypasses are marked `private, no-store`. Purge scans are bounded by
cache capacity. Geographic POP placement, provider credentials, TLS termination,
DDoS controls, and cross-node consistency remain external trust boundaries.
See `docs/CDN_Caching.md`.

`render_cache.sapp` caps queued dirty regions (128 by default, 512 hard maximum),
deduplicates identical invalidations, normalizes non-finite geometry/FPS input,
and promotes excess region streams to one full repaint. This bounds retained
region metadata and avoids eager coalescing after every mark. It improves CPU
render scheduling but is not GPU memory isolation; native graphics backends and
drivers remain separate trust boundaries.

---

## Quick Reference

| Need | pblang Solution |
|------|----------------|
| Immutable variable | `const x = value;` |
| Nil-safe access | `obj?.field` or `arr?[i]` |
| Error propagation | `result?` (throws on Err) |
| Type check at runtime | `type_assert(val, "number")` |
| Immutable data | `freeze(map_val)` |
| Non-null assertion | `safety.non_null(val, "name")` |
| Safe division | `checked_div(a, b)` |
| Bounds-safe index | `safety.bounds_check(arr, i)` |
| Debug assertion | `assert(cond, "message")` |
| Named invariant | `safety.invariant(cond, "description")` |
| Unimplemented | `safety.todo("description")` |
| Unreachable code | `safety.unreachable("description")` |
| Coerce to number | `safety.coerce_number(val, "name")` |
| Validate input | `validate.check(schema, data)` |
| Type-safe result | `result.ok(val)` / `result.err(msg)` |
| Safe channel send | `safety.safe_send(ch.sender, val)` |
| Bounded worker supervision | `thread.supervisor()` + cooperative cancellation token |
| Result-safe index | `safety.checked_index(arr, i)` |
| Bounded arena | `memory_safety.bounded_arena(name, allocations, units)` |
| Speculative rollback | `arena_checkpoint` + `arena_rewind` |
| Hot-loop quota | `resource_budget.ledger_try_reserve(...)` |
| Product operation guard | `product_runtime.run(...)` / `run_once(...)` |
| Bounded in-memory cache | `cache.bounded(...)` + `cache.stats(...)` |
| Bounded dirty regions | `render_cache.new(w, h, max_rects)` |
| Device allowlist | `device.policy(...)` + `device.decide(...)` |
| Failure-safe secret cleanup | `harden.with_secret_result(...)` |

---

## Summary

| Guarantee | Mechanism |
|-----------|-----------|
| **Core managed-memory protection** | Safe runtime paths plus checked ownership/borrow APIs |
| **Cycle-leak reduction** | RC + cycle-breaking collection; native handles still require lifecycle discipline |
| **No data races** | VM-per-thread isolation |
| **No unhandled nil** | `?.` operator, `safety.non_null()` |
| **No accidental mutation** | `const` keyword, `freeze()` |
| **No ignored errors** | `result.sapp` + `?` operator |
| **No type confusion** | `type_assert()`, type annotations |
| **No overflow/div-zero** | `checked_*` builtins |
| **No out-of-bounds** | `?.` operator, `bounds_check()` |
| **No broken invariants** | `safety.invariant()` |

pblang provides strong runtime checks and fail-closed libraries, but safety is a
system property rather than a universal language claim. Production code must
combine these APIs with capability isolation, backend verification, resource
limits, driver/firmware review, and tests on each supported target.
