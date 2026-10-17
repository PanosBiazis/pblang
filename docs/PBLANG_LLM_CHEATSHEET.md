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

---

## 3. Native PBLang LLM Cognition & Cyber/AI Suite (`stdlib:llm_cognition_suite.sapp`)

1. `inspect_pblang_idioms` (`pblang.llm.idiom-mentor.v1`): Detects and rewrites foreign-language syntax habits and enforces `ordered_input: true`, O(1) `len(args)` guards, and `OWN3-J02` cleanup contracts.
2. `compile_orientation_brief` (`pblang.llm.orientation-brief.v1`): Generates the `< 30`-line `NOW.md` handoff, decodes internal shorthand (`L1..L4`, `R8-LOCAL-02`, `OWN3-J02`, `QZ-NEXT-03`), and enforces Home Linux vs Corporate Windows isolation.
3. `audit_ecosystem_topology` (`pblang.llm.ecosystem-topology.v1`): Verifies `L1 -> L2 -> L3 -> L4` `pblang.lock` synchronization, sealed `pb-mcp-offline` `MANIFEST.sha256.json`, and `--tmpfs` source-hidden isolation.
4. `evaluate_epistemic_claims` (`pblang.llm.epistemic-gate.v1`): Blocks conflation of modeled/static analysis with runtime execution, hosted GitHub Actions CI (billing-blocked), physical `BME688` hardware, or Windows/macOS execution.
5. `verify_dualstack_and_commercial` (`pblang.llm.dualstack-commercial.v1`): Verifies `.sapp <-> C#` DTO parity, Greek 9-digit AFM Modulo-11 (`000000000` retail-only), 11-column running-balance ledger, `6,000` email quota (`90%` warning / `100%` hard-stop), `pbduallock` opt-in `second_lock`, and white-label leak checks.
6. `audit_gui_viewport_and_plan` (`pblang.llm.viewport-dag-planner.v1`): Prevents vertical/horizontal viewport clipping on `1366x768` (`1200x920` `152px` overflow) and validates `L1 -> L2 -> L3 -> L4` DAG execution order.
7. `migrate_delphi_to_blazor` (`pblang.llm.delphi-blazor-migrator.v1`): Extracts VCL `OnChange`/`OnClick`/`OnCalcFields`/`BeforePost` event-coupled business logic, flags synchronous `FieldByName` / `Application.ProcessMessages` / unit globals, and verifies 2-decimal EUR VAT (`24%`, `13%`, `6%`) & 12-digit GEMI equivalence.
8. `enforce_von_neumann_and_bpe_shield` (`pblang.llm.von-neumann-bpe-shield.v1`): Enforces NX/DEP separation between `SYSTEM_INSTRUCTION_PLANE` and `UNTRUSTED_DATA_PLANE`, provides deterministic character counting, string reversal, and arbitrary-precision string BigInt addition, and refutes false user premises (Anti-Sycophancy Gate).
9. `guard_attention_and_continual_memory` (`pblang.llm.attention-continual-guard.v1`): Hoists critical invariants from the `25%..75%` middle-context zone ("Lost in the Middle"), blocks catastrophic forgetting of `prior_invariants`, and detects synthetic AI-slop / Model Collapse.
10. `evaluate_causal_world_and_alignment` (`pblang.llm.causal-world-alignment.v1`): Distinguishes spurious correlation from `do(X)` causal intervention, flags Out-of-Distribution (OOD) Z-score anomalies, validates physical hardware tolerances, emits a 4-step white-box proof trace, and blocks Goodhart's Law reward hacking.
11. `audit_cyber_pqc_and_user_shield` (`pblang.llm.cyber-pqc-user-shield.v1`): Detects lookalike phishing domains, unwatermarked voice-clone deepfakes, session hijack risks, UI dark patterns, unpinned dependencies, C/C++ memory hazards, and Post-Quantum Cryptography (`Harvest Now, Decrypt Later` `RSA`/`ECDSA` vs NIST FIPS 203/204 `ML-KEM-768` / `ML-DSA-65`).
12. `resolve_historical_cycle_vs_live` (`pblang.llm.historical-cycle-resolver.v1`): Quarantines historical claims from earlier cycles (`Cycles 1..27`) when mistakenly treated as live runtime authority in `Cycle 28`.
