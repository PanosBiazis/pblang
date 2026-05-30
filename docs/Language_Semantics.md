# pblang language semantics (living spec snapshot)

Stability snapshot for core semantics. Used as reference for conformance and compatibility.

## For-in loops

### Array iteration
```pblang
for x in arr {
    // x is each element in order
}
```

### Map key-value iteration
```pblang
for k, v in mymap {
    // k is the key (string), v is the value
}
```

- `for x in arr` iterates array elements from index 0 to len-1
- `for k, v in m` iterates map keys (via `map_keys`) and values
- Both forms create a new binding scope per iteration
- These are syntactic sugar over `while` loops; no generator/yield mechanism
- Original `for i N { body }` (counter loop) is still supported

## String formatting

`str_format(fmt, ...args)` supports Python-style formatting:

| Specifier | Meaning |
|---|---|
| `%s` | string (uses `str()`) |
| `%d` / `%i` | integer (truncates float to i64) |
| `%f` | float (6 decimal places) |
| `%v` | any value (Go-style, uses `str()`) |
| `%%` | literal `%` |

Example:
```pblang
print str_format("Hello %s, you have %d messages", name, count);
```

## Regex

Builtin regex functions (backed by the Rust `regex` crate, RE2-syntax):

| Function | Description |
|---|---|
| `regex_match(str, pat)` | → bool: does str contain a match? |
| `regex_find(str, pat)` | → string \| nil: first match |
| `regex_find_all(str, pat)` | → array\<string\>: all matches |
| `regex_replace(str, pat, rep)` | → string: replace first match |
| `regex_replace_all(str, pat, rep)` | → string: replace all matches |
| `regex_split(str, pat)` | → array\<string\>: split on pattern |
| `regex_capture(str, pat)` | → array\<string\> \| nil: capture groups |

Or import the `stdlib:regex.sapp` module for a higher-level API.


In conditionals and logical expressions, the following are **false**:

- `nil`
- `false`
- Number `0.0`
- Empty string `""`
- Empty array `[]`
- Empty map `map{}`

All other values are **true**.

## String literals

String literals are double-quoted: `"..."`. Inside a string, the following **escape sequences** are supported:

- `\\` — backslash
- `\"` — double quote
- `\n` — newline
- `\t` — tab
- `\r` — carriage return

Any other character (including unescaped `"`) ends or starts the string. The parser rejects invalid escapes.

## Indexing

- **Arrays**: index by number (0-based). Out-of-range or non-integer index is a runtime error.
- **Maps**: index by string key. Missing key returns `nil` (no error). Attribute access `obj.key` is equivalent to `obj["key"]`.

## Method-call sugar

- `obj:method(a, b)` desugars to a method call where `obj` is passed as the first argument (`self`).
- If `obj.__class` exists, method lookup happens on `obj.__class`, so class-backed objects dispatch through their class method map.
- If `obj.__class` is missing, lookup falls back to the method stored directly on `obj` itself.
- There is no implicit global class lookup for unrelated plain maps.

## Match expressions and guards

- **Basic match**: `match expr { case pat: body; ... else: body }`
- **Guarded match**: `case pat if guard # body` (recommended separator with guards)
  - The `#` separator is unambiguous (unlike `:` which conflicts with `obj:method()` postfix)
  - Alternative: `case pat if guard then body` using the `then` keyword
  - `:` with guarded arms is currently parser-recovery-sensitive; use `#` or `then`
- **Wildcard**: `case _: body` matches any value
- **Bind**: `case n: body` binds the scrutinee to `n` in the body
- **Guard**: `case n if n > 0 # body` — guard is any truthy expression
- **No exhaustiveness check**: non-matching match returns `nil`

## Threading patterns

- **Spawning**: closures passed to `spawn()` must have **closed upvalues** (not open stack vars)
- **Wrong**: `let x = 5; spawn(fn() { use(x); })` — `x` is an open upvalue (top-level global)
- **Right**: `fn make_fn(x) { return fn() { use(x); }; } let f = make_fn(5); spawn(f)`
  — `make_fn` returns before `spawn`, closing the upvalue
- **Channels**: `channel()` returns `{sender: id, receiver: id}`
  - `send(ch.sender, value)` — send on sender handle
  - `recv(ch.receiver)` — block-receive on receiver handle
  - `channel_close(ch.sender)` — close sender handle
  - `channel_is_closed(ch.sender)` — check if closed
  - `select([ch_a.receiver, ch_b.receiver])` — returns `{index, value}` or `nil`


- **Runtime error**: a `RuntimeError` value with message, optional span/source, and stack trace. Thrown by builtins and VM (e.g. type mismatch, undefined global, capability denied).
- **Exception handling**: `try { ... } catch e { ... }` catches runtime errors; `e` is the error value. `defer { ... }` runs on scope exit (LIFO).

## Function call arity

- User functions: arity is fixed at definition. Calling with wrong number of arguments is a runtime error ("arity mismatch").
- Builtins: many accept a fixed number of arguments; mismatch produces a runtime error with a message like "expects N arguments, got M".

## Capabilities (embedding / sandbox)

When the VM is created with `VM::with_stdlib(config)`, the host can disable categories of builtins:

- **`fs`**: filesystem and path builtins (e.g. `fs_read`, `fs_write`, `path_join`, `cwd`).
- **`net`**: TCP and HTTP server helpers (`tcp_listen`, `tcp_accept`, `http_parse_request`, etc.).
- **`http`**: HTTP client (`http_get`, `http_post`).
- **`sqlite`**: SQLite builtins.
- **`ffi`**: `ffi_load`, `ffi_call`, `ffi_close`.
- **`exec`**: process execution helpers (e.g. `exec`, `sleep_ms`).
- **`core`**: always-on safe core builtins including `env_get`.

`env_set` is intentionally disabled in current runtime policy (thread-safety) and returns an error.

If a script calls a builtin whose capability is disabled, the VM returns a runtime error (e.g. "fs access denied"). `VM::new()` creates a VM with **no** builtins (embedding-only).

## Resource limits (embedding)

When **VmLimits** is set via `vm.set_limits(Some(limits))`:

- **max_instructions**: After this many VM instructions, execution stops with a runtime error ("instruction limit exceeded"). Used to prevent infinite loops.
- **max_stack_depth**: Maximum call stack depth (number of frames). Exceeding it yields "stack depth limit exceeded".
- **max_memory_bytes**: Optional cap on heap memory (best-effort enforcement when set).
- **io_timeout_ms**: Timeout in ms for IO: enforced for **http_get**, **http_post**, TCP streams (read/write timeout on **tcp_accept**), and for **fs_read**, **fs_write**, **fs_read_bytes**, **fs_write_bytes** (when set, fs I/O runs in a thread and returns an error on timeout).

These are optional; when not set, there is no limit (subject to host memory).

## Runtime memory safety (optional builtins)

The VM exposes an **optional, runtime-only** borrow checker for explicit handles. This is **not** compile-time Rust-style ownership for ordinary pblang values (strings, arrays, maps still use RC + cycle GC). Use these builtins when you want structured allocate/borrow/release semantics in scripts or tests.

| Builtin | Description |
|---------|-------------|
| `allocate(value, is_mutable?)` | Allocate an owned slot; returns handle `>= 1`. Default `is_mutable` = true. |
| `borrow_immutable(handle)` | Shared read borrow; returns borrow id. |
| `borrow_mutable(handle)` | Exclusive mutable borrow; returns borrow id. |
| `borrow_unique(handle)` | Move/consume; further `load` on handle errors. |
| `release_borrow(borrow_id)` | Release by borrow id. |
| `release_borrow(handle, type)` | Release first borrow of type `0` = immutable, `1` = mutable, `2` = unique. |
| `load(handle)` | Read value (unless moved or lifetime-expired). |
| `store(handle, value)` | Write value; requires mutable allocation and **no active borrows**. |
| `deallocate(handle)` | Free slot; fails if still borrowed. |
| `push_lifetime()` / `pop_lifetime()` | Nested scopes; values allocated inside a popped scope become inaccessible. |
| `memory_stats()` | Map with `total_locations`, `active_borrows`, `lifetime_depth`, `ref_count_total`. |

Errors are runtime errors (catchable with `try` / `catch`). Embedders that are not the stock VM receive "not supported by this host" unless they override the `VmHost` memory hooks.

## FFI policy (embedding)

When **FfiPolicy** is set via `vm.set_ffi_policy(Some(policy))` and `allow_all` is false:

- **allowed_libs**: Only these library paths may be passed to `ffi_load`. Otherwise runtime error "ffi: library not in allowlist".
- **allowed_symbols**: Only these symbol names may be passed to `ffi_call`. Otherwise "ffi: symbol not in allowlist".

When `allow_all` is true or policy is not set, no allowlist check is applied (subject to `allow_ffi` capability).

## Embedding

For a full description of the embedding API (VM creation, sandboxing, limits, FFI policy, resource lifecycle), see [Embedding.md](Embedding.md).

---

## Appendix A: Complete Language Reference (v0.5.7)

### Operator Precedence (High → Low)

| Precedence | Operators | Description |
|-----------|-----------|-------------|
| 7 | `!` (prefix) | Logical NOT |
| 6 | `*`, `/`, `%` | Multiplication, Division, Modulo |
| 5 | `+`, `-` | Addition, Subtraction |
| 4 | `<`, `<=`, `>`, `>=` | Comparison |
| 3 | `==`, `!=` | Equality |
| 2 | `and` | Logical AND |
| 1 | `or` | Logical OR |
| 0 | `??` | Null coalescing |

### Variable Scoping Rules

Variables are lexically scoped. Each `{...}` block creates a new scope.

```
let x = 1;
{
    let x = 2;   // shadows outer x in this block
    print x;     // prints 2
}
print x;         // prints 1 — outer x unchanged
```

Function parameters and `for` loop variables are scoped to their block.
Closures capture variables by reference (upvalue), so mutations inside a closure affect the outer binding.

### Module System

Import resolution order:
1. `stdlib:name.sapp` — from the built-in standard library
2. `./relative/path.sapp` — relative to the importing file
3. `pkg:subpath/file.sapp` — from a locked dependency in `pblang.lock`
4. Registry packages — declared in `pblang.toml` and installed with `pblang install`

**Package manifest (`pblang.toml`)**

```toml
[package]
name = "my-app"
version = "0.1.0"
entry = "src/main.sapp"   # optional; default: main.sapp / lib.sapp

[dependencies]
"@acme/http" = "1.2.0"
"utils" = { path = "./vendor/utils" }
"lib" = { git = "https://example.com/lib.git", rev = "abc123def456" }
```

**Import examples**

```sapp
import "stdlib:json.sapp";
import "./helpers.sapp";
let http = import("@acme/http");              // package root / [package].entry
let util = import("utils:src/util.sapp");     // subpath inside path/git/registry dep
```

Run `pblang install` (or `pblang build`) to generate `pblang.lock`. The lockfile pins each dependency path and a **content hash** of the unpacked tree. If a cached dependency is missing, pblang can **rehydrate** registry/git deps when `PBLANG_REGISTRY_INDEX` is set. Tampering with installed files causes `content hash mismatch` on import.

Circular imports are detected and produce a runtime error.
Module-level code runs once; the exported map is cached.

### Number Type

All numbers are `f64` (IEEE 754 double-precision float).
- Integer literals: `42`, `-7`
- Float literals: `3.14`, `-0.001`
- Special values: `0.0/0.0` → `NaN`, `1.0/0.0` → `Infinity`
- Arithmetic operations: `+`, `-`, `*`, `/`  
- Integer division: use `num_floor(a / b)` 
- Modulo: `num_mod(a, b)` or `a % b`

### String Operations

Strings are UTF-8 and the whole string API is **codepoint-indexed** (not byte-indexed): `len`, indexing `s[i]`, `slice`/`s[a..b]`, `str_slice`, `str_index_of`, and `strcharcodeat` all count Unicode codepoints. For raw byte access use `str_to_bytes(s)` → `Array<u8>` (and `bytes_to_str` to decode). String methods:
- `len(s)` — codepoint (character) count
- `s + t` — concatenation  
- `s[i]` — the character (single-codepoint string) at codepoint index `i`
- `str_slice(s, start, end)` — substring, codepoint-indexed (half-open `[start, end)`; same as `s[start..end]`)
- `strcharcodeat(s, i)` — the full Unicode scalar value of the character at codepoint index `i` (e.g. `strcharcodeat("é", 0) == 233`)
- `strfromcharcode(n)` — the one-character string for Unicode scalar `n`; inverse of `strcharcodeat`, so `strfromcharcode(strcharcodeat(s, i))` round-trips
- `str_split(s, delim)` — split to array
- `str_contains(s, needle)` — boolean search
- `str_starts_with(s, prefix)` — boolean
- `str_ends_with(s, suffix)` — boolean
- `str_trim(s)` — strip whitespace
- `str_upper(s)`, `str_lower(s)` — case conversion
- `str_replace_all(s, from, to)` — replace all occurrences
- `str_index_of(s, needle)` — find position (-1 if not found)
- `str_pad_left(s, n, char)` — left-pad to width n
- `str_pad_right(s, n, char)` — right-pad to width n
- `str_repeat(s, n)` — repeat string n times
- `str_format(fmt, ...args)` — printf-style formatting

### Array Operations

Arrays are 0-indexed, dynamically typed, reference semantics.
- `[a, b, c]` — literal  
- `arr[i]` — get element (error if out of bounds)
- `arr[i] = v` — set element  
- `array_push(arr, v)` — append (modifies in-place)
- `array_pop(arr)` — remove + return last element
- `len(arr)` — length
- `range(n)` — [0, 1, ..., n-1]
- `range(start, end)` — [start, ..., end-1]
- `range(start, end, step)` — stepped range
- `array_unique(arr)` — remove duplicates
- `array_zip(a, b)` — zip two arrays
- `array_chunk(arr, n)` — split into chunks of size n
- `array_flatten(arr)` — flatten one level
- `array_take(arr, n)` — first n elements
- `array_drop(arr, n)` — all but first n elements
- `enumerate(arr)` — [{index, value}, ...] pairs
- `sum(arr)` — sum of numeric elements
- `min_of(arr)`, `max_of(arr)` — min/max values

### Map Operations

Maps are string-keyed, reference semantics.
- `map{ key: value }` — literal  
- `m.key` or `m["key"]` — get value (nil if missing)
- `m.key = v` or `m["key"] = v` — set value  
- `map_keys(m)` — array of keys
- `map_has(m, key)` — boolean existence check
- `map_remove(m, key)` — remove key, returns removed value
- `freeze(m)` — mark map as immutable

### Control Flow

```
// While loop
while condition {
    // body
}

// For loop (0..N)
for i 10 { print i; }  // 0, 1, ..., 9

// For-in loop (array iteration)
for item in array { print item; }

// For-in loop (map key-value)
for key, value in some_map { print key + "=" + str(value); }

// Loop (infinite, exit via break)
loop {
    if done { break; }
    continue;  // jump to next iteration
}
```

### Match Expressions

```
let result = match value {
    case 1: "one",
    case 2: "two",
    case n if n > 100: "big",  // guard (use # instead of : if : conflicts)
    case _: "other"             // wildcard/default
};
```

No exhaustiveness checking. The `else` expression is optional (returns nil if no case matches).

### Error Handling

```
// try/catch: catches runtime errors and throws
let v = try { risky_operation() } catch "default_value";

// throw: raise an error
throw "something went wrong";
throw map{ code: 404, message: "not found" };

// defer: runs on scope exit (LIFO order, like Go's defer)
defer { cleanup(); }
defer { print "runs second"; }
defer { print "runs first"; }

// assert: runtime assertion
assert(condition, "error message if false");
```

### Functions and Closures

```
// Named function
fn add(a, b) { return a + b; }

// Function with type annotations
fn greet(name: string, times: number) -> string {
    return str_repeat("Hello " + name + "! ", times);
}

// Anonymous function / lambda
let f = fn(x) { return x * 2; };

// Closure: captures outer variables by reference
fn make_counter() {
    let n = 0;
    return fn() { n = n + 1; return n; };
}
let c = make_counter();
c();  // 1
c();  // 2

// Const declaration (no reassignment)
const PI = 3.14159;
```

### Object-Oriented Programming

pblang uses the `class.sapp` module for OOP:

```
let oop = import("stdlib:class.sapp");

let Animal = oop.define("Animal", ["name"], map{
    speak: fn(self) { return self.name + " speaks!"; }
});

let Dog = oop.extend(Animal, "Dog", [], map{
    speak: fn(self) { return self.name + " barks!"; }
});

let d = oop.new(Dog, map{ name: "Rex" });
let r = oop.call(d, "speak");  // "Rex barks!"
oop.instanceof(d, Dog);         // true
oop.instanceof(d, Animal);      // true
```

### Concurrency

```
// Spawn a goroutine
let t = spawn(fn() { return expensive(); });
let result = join(t);

// Typed channels
let ch = channel();           // unbounded
let bch = channel(10);        // buffered, capacity 10
send(ch.sender, 42);
let v = recv(ch.receiver);    // blocks until value available

// Select (receive from first ready channel)
let sel = select([ch1.receiver, ch2.receiver]);
// sel = { index: 0_or_1, value: received_value }

// Close a channel (senders only)
channel_close(ch.sender);
channel_is_closed(ch.sender);  // true
```

### Structs (Value Types via `struct.sapp`)

```
let s = import("stdlib:struct.sapp");
let Point = s.define("Point", ["x", "y"]);
let p = s.new(Point, map{ x: 3, y: 4 });
let p2 = s.with(p, map{ x: 10 });  // {x:10, y:4}
s.instanceof(p, Point);             // true
```

### Enums / Sum Types (via `enum.sapp`)

```
let en = import("stdlib:enum.sapp");

// Option type
let some_val = en.some(42);
let no_val = en.none();
en.is_some(some_val);           // true
en.unwrap_or(no_val, 0);        // 0

// Result type
let ok = en.ok(100);
let bad = en.err("failed");
en.is_ok(ok);                   // true
en.unwrap_ok(ok);               // 100

// Custom enum
let Color = en.define("Color", ["Red", "Green", "Blue"]);
let c = en.variant(Color, "Red", nil);
en.match(c, map{
    Red:   fn() { return "red"; },
    _:     fn() { return "other"; }
});
```

