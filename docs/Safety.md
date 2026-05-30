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

---

## Summary

| Guarantee | Mechanism |
|-----------|-----------|
| **No VM crashes** from pblang code | Rust runtime, `Result`-based error propagation |
| **No memory leaks** | RC + cycle-breaking GC |
| **No data races** | VM-per-thread isolation |
| **No unhandled nil** | `?.` operator, `safety.non_null()` |
| **No accidental mutation** | `const` keyword, `freeze()` |
| **No ignored errors** | `result.sapp` + `?` operator |
| **No type confusion** | `type_assert()`, type annotations |
| **No overflow/div-zero** | `checked_*` builtins |
| **No out-of-bounds** | `?.` operator, `bounds_check()` |
| **No broken invariants** | `safety.invariant()` |

pblang gives you the tools to write code as safe as Rust at the language level.
Use them, especially in production code and when self-hosting.
