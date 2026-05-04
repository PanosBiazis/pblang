# Help.md — GLM-5 Code-Change Reference for Biapryx

This file tells **GLM-5** (ZhipuAI) **exactly which files to touch, what code to write/change, and what tests to add** for the next recommended batch. This is NOT a backlog — that lives in `docs/Tasks.md`. Rewrite this file after every run to reflect the next batch.

---

## Status of Last Batch — COMPLETED ✓

| Task | What landed |
|------|-------------|
| **SP1-gamma** | OOP + Match conformance fixes complete: all 76 semantics tests passing, `class_basic_pass`, `class_method_dispatch_pass`, `match_exhaustive_pass`, `match_guard_pass` verified, Spec_freeze.md v0.2 documented. |

Do **not** re-touch any of the above unless fixing a confirmed bug or explicit regression.

---

## Active Priority Order

```
1. W4
2. A4
3. Q3
4. T5
5. G3
6. L3
7. M4
8. R3
9. PL1
10. ED1
11. PF1
12. N2
13. OS1
```

---

## Next Recommended Batch — W4-gamma — WebSocket Full-Duplex Send

### Goal

Implement the WebSocket write_frame functionality for full-duplex communication in the stdlib web module.

### What to do

1. **Implement WebSocket write_frame** (`stdlib/web.sapp`):
   - Function `websocket_write_frame(socket, opcode, payload, mask?)`
   - Support opcodes: text (0x01), binary (0x02), close (0x08), ping (0x09), pong (0x0A)
   - Client-side frames should be masked by default
   - Handle fragmentation for large payloads

2. **Add tests** (`tests/semantics/websocket_write_frame_test.sapp`):
   - Test unmasked text frame encoding
   - Test masked binary frame encoding
   - Test close frame with status code

3. **Update docs**:
   - `docs/Tooling.md`: Add W4-gamma section
   - `docs/Report.md`: Add W4-gamma completion entry

### Expected Result

- WebSocket write_frame functionality working
- Tests passing
- Documentation updated

---

## Rules for GLM-5

1. Read `docs/Tasks.md` and `docs/Report.md` before every run.
2. Touch **only files listed** for the chosen batch.
3. One primary task + up to 3 related follow-ups (same area, same/adjacent files).
4. Every behavior change needs a test. Every test must be cross-platform safe.
5. **Done = tests pass + no unrelated file changes + `docs/Report.md` + `docs/Tasks.md` + `docs/Help.md` updated.**

---

## File Map

| Area | Key files |
|------|-----------|
| VM core | `crates/biapryx-vm/src/vm.rs`, `crates/biapryx-compiler/src/lib.rs`, `crates/biapryx-ir/src/lib.rs` |
| CLI entry | `src/main.rs` |
| Package manager | `crates/biapryx-pm/src/lib.rs`, `crates/biapryx-pm/src/semver.rs` |
| Stdlib | `stdlib/*.sapp` |
| Tests (semantics) | `tests/semantics/*.sapp` + `tests/semantics.rs` |
| Tests (other) | `tests/lsp.rs`, `tests/pack.rs`, `tests/install_cli.rs` |
| Docs | `docs/Tooling.md`, `docs/Report.md`, `docs/Tasks.md`, `docs/Help.md`, `docs/Spec_freeze.md` |