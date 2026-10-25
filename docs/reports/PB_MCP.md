# pb_mcp — the native pblang MCP tool surface

`stdlib/pb_mcp.sapp` is the self-hosted, single-source tool surface behind the
pb-mcp-server family. It is pure pblang: every tool is a function, dispatch is
`call(tool, args)`, and there is **no shell, no exec, no network** anywhere in
the module.

```pblang
let M = import("stdlib/pb_mcp.sapp");
M.call("grep_context", map{ pattern: "fn call(", dir: "stdlib" });
```

## Profiles

| Profile     | Purpose |
|-------------|---------|
| `core-code` | read-only workspace navigation: read/list/grep/outline/stats/hash |
| `watchers`  | VM + system observability (heap, bytecode, hardware, binary triage) |
| `meta`      | tool discovery, batching, usage stats, dry-run validation |
| `ai`        | pbai bridges: sampling recommendation, reply normalization, obfuscation detection |

Enumerate with `profiles()`, `tools_by_profile(p)`, `tool_names()`, `tool_info(name)`.

## Discovery → validate → execute

The intended agent loop, entirely offline:

1. `suggest_tool("search text in files")` — keyword-ranked candidates.
2. `validate_call(tool, args)` — `{ok}` or `{ok:false, missing:[...]}` **without executing**.
3. `call(tool, args)` or `call_batch([{tool, args}, ...])` — per-entry failure isolation.
4. `call_stats()` — usage counters for observability.

## Wave-2 tools (2026-07-04)

| Tool | What it adds |
|------|--------------|
| `grep_context` | matches with N lines of leading/trailing context (grep -B/-A), one call instead of grep + read_file |
| `search_symbols` | where a `fn`/`struct`/`class`/`enum` is **defined** across `.sapp` files (grep_code finds uses) |
| `dir_stats` | recursive `{files, dirs, bytes, by_ext}` in one call |
| `file_hash` | djb2-32 content hash (8 hex chars) for cheap change detection between calls |
| `tool_info` | full catalog entry for a tool |
| `validate_call` | dry-run: unknown-tool + required-arg check before paying for execution |

## Security model

- **`_safe_path` gates every filesystem tool.** It refuses unix-absolute (`/`),
  Windows root/UNC (`\`), drive-qualified (`C:`), any `..` (checked after
  normalizing backslashes, so `a\..\b` cannot slip through), and embedded NUL.
- **No command execution.** Nothing in the module shells out; watcher tools are
  read-only probes.
- **Bounded everything.** Reads truncate at `MAX_READ_CHARS`, listings cap at
  `MAX_FILES`, grep at `MAX_GREP`; recursion depth is capped at 8.
- **Batch nesting guard.** `call_batch` may dispatch `call_batch`, so
  self-nesting is capped at depth 4 — a hostile batch gets a deterministic
  `{ok:false}` entry instead of winding the VM stack to its recursion ceiling.
- **Memory safety by construction.** pblang out-of-bounds reads throw a
  catchable error; parser-style tools cannot corrupt memory.

**Honest boundary:** `_safe_path` is a lexical check. A *symlink inside the
workspace pointing outside it* is followed by the underlying `FS` layer — pure
pblang has no `realpath` builtin to resolve it. Run the server with a workspace
that does not contain untrusted symlinks, or add an OS-level sandbox (the
Rust pb-mcp-server wrapper additionally enforces `PB_PROJECT_ROOT`).

## Regression tests

`tests/semantics/pb_mcp_*` — bridge, enrichment, extended, router, smart
discovery, **server security** (locks the path guard), and **wave2** (locks the
new tools + the batch depth guard). Run them with `scripts/t pb_mcp_wave2` etc.
