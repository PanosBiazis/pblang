# Perplexity ⟷ Cursor ⟷ Project (tools/plugins)

```
    Perplexity  <======  something  ======>  Cursor
                              ||
                              \/
                          Project (Sapphire)
```

## What “something” can be

### 1. **Cursor → Perplexity (project context, no API key)**

| What | How |
|------|-----|
| **Bridge** | Script that builds/updates `PERPLEXITY_CONTEXT.md` from the project and (optional) copies it to clipboard. |
| **Flow** | You run the script in the project → open Perplexity in browser → paste. Perplexity “sees” the project. |
| **In Cursor** | Run the script from terminal or add a task (e.g. “refresh context for Perplexity”). |

Use **`scripts/refresh-perplexity-context.ps1`** (or `.cmd`) to refresh the context file and copy it.

---

### 2. **Perplexity → Cursor (Perplexity as a tool inside Cursor)**

| What | How |
|------|-----|
| **Tool/Plugin** | **MCP (Model Context Protocol)** server: “Perplexity” MCP in Cursor. |
| **Flow** | Cursor has the project; Composer/Agent can call Perplexity (search/ask) when you ask. Project stays in Cursor; Perplexity is an extra tool. |
| **Needs** | Perplexity API key. Add MCP server in **Cursor Settings → Features → MCP**. |

Examples:

- **Perplexity Ask MCP**: `npx -y server-perplexity-ask` (set `PERPLEXITY_API_KEY`).
- **MCPCursor / Cursor Directory**: search “Perplexity” for one-line setup.

---

## Summary

| Direction | Tool/plugin | API key? |
|-----------|-------------|----------|
| **Project → Perplexity** (so Perplexity sees the project) | Script + `PERPLEXITY_CONTEXT.md` + paste | No |
| **Perplexity → Cursor** (Perplexity as a tool in Cursor) | Perplexity MCP server in Cursor | Yes (Perplexity) |

So: **tools/plugins** = (1) script + context file for Cursor/Project → Perplexity, and (2) Perplexity MCP in Cursor for Perplexity → Cursor; both point at the same **Project**.
