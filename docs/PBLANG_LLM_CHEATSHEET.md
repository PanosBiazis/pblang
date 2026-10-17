# PBLang (`.sapp`) — Authoritative LLM Cheat Sheet & Zero-Warning Idioms

This reference eliminates Large Language Model (LLM / ΜΜΤΝ) syntax hallucinations and linter warnings when writing or reviewing **100% pure PBLang (`.sapp`)** across L1 (`pblang`), L2 (`pb-mcp-server`), L3 (`pbwatermarks-remover`), and L4 products.

---

## 1. Core Syntax & Builtins (Do NOT Use Python/Rust/JS/C# Habits)

| Concept | ❌ Foreign Habit (Forbidden) | ✅ Idiomatic PBLang (`.sapp`) (`0` Warnings) |
|---|---|---|
| **Printing** | `println(x)` / `print(x)` / `console.log(x)` | `print x;` (Statement keyword, no outer call parens needed) |
| **Error Handling** | `try { f(x); } catch (e) { ... }` | `let out = try f(x) catch nil;` (Value expression) |
| **Multi-Branch `if`** | `if a { ... } else if b { ... }` | `if a { ... } elif b { ... } else { ... }` |
| **Inline Conditional** | `a ? b : c` or `if a { 1 } elif b { 2 } else { 3 }` | `let v = if a { 1 } else { 2 };` (No `elif` inside expression `if`) |
| **Rounding** | `num_round(x)` / `Math.round(x)` | `num_floor(x + 0.5)` (`num_floor(x * 100.0 + 0.5) / 100.0` for 2 decimals) |
| **Collapsible `if`** | `if a { if b { do_x(); } }` | `if a and b { do_x(); }` (Prevents collapsible-`if` linter warning) |
| **Boolean Normalization** | `!!val` / `bool(val)` | `fn _is_true(v) { if v { return true; } return false; }` |
| **Array Iteration** | `for (let i = 0; i < arr.length; i++)` | `for item in arr { ... }` or `for i len(arr) { ... }` |

---

## 2. Mandatory Runtime & Security Contracts

### A. O(1) Pre-Enumeration Guard in PB-MCP Tools
Never call `map_keys(args)` or inspect properties before checking `len(args)` in O(1), so wide maps (e.g., `4,098` keys) are rejected with `0` key enumerations:
```sapp
fn _parse_spec(args, ctx) {
    if type(args) != "map" or len(args) != 1 or not map_has(args, "spec_json") {
        throw ctx + ": invalid arguments";
    }
    // ...
}
```

### B. Bounded Map Key Validation (`0` Unused-Variable Warnings)
Validate allowed keys by iterating over the bounded `allowed` array and comparing `matched != len(value)`:
```sapp
fn _keys(value, allowed, ctx) {
    if type(value) != "map" { throw ctx + ": expected map"; }
    let matched = 0;
    for field in allowed {
        if map_has(value, field) { matched = matched + 1; }
    }
    if matched != len(value) { throw ctx + ": unsupported property"; }
}
```

### C. File Descriptor (`fd_delta: 0`) & Atomic Cleanup Contract (`OWN3-J02`)
Never use opaque `try ... catch nil` on file/directory writes or removals. Use `write_text_atomic_result` / `read_text` and verify `parent_close_confirmed`, `cleanup_uncertain: false`, and `defer <expr>;` cleanup.

### D. Deterministic Ordered Input (`ordered_input: true`)
All CLI/GUI product entrypoints (`main.sapp`, `src/app.sapp`) and `stdlib/*_view.sapp` modules must specify `ordered_input: true`.

### E. Sealed Offline Release Hygiene (`python3 -I -B`)
When invoking Python for `stdio_guard.py` or `build_offline_release.sapp`, always pass `-I -B` so `__pycache__` never pollutes `release/transport/` before `MANIFEST.sha256.json` / `SHA256.json` verification.
