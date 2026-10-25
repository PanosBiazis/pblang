# Idea intake across products and AI extension ecosystems

This is a bounded discovery record and an implementation queue, not a security
certification, a claim of complete disk coverage, or approval to install/run
third-party plugins. External instructions, hooks and tools were not executed.

## Sources examined

- Current PBLang memory/runtime modules, their owning tests and the previous
  scalar-runtime validation report.
- The available conversation catalog and two recent turns of the prior
  PB-MCP/PBRemote desktop-control discussion. That discussion distinguishes
  PBLang from the Rust implementation being retired, and calls for session-bound
  approval, revocation and a combined capture/input Stop operation. This is
  retrieved prior context, not a new live validation of the whole desktop path.
- 986 callable tool descriptions supplied in the current session, the available
  skills catalog, and the user-visible recommended-but-not-installed plugin
  list. Availability in this session does not prove an entire plugin is active,
  installed everywhere, safe, or useful for a particular PBLang task.
- 249 local cached plugin manifests: 215 in the Codex cache and 34 in the Claude
  cache, counting versions/duplicates. Only manifest name/description/version
  metadata was examined. Cache presence is not an activation-state audit.
- A lexical source scan read 373 selected files: 245 under PBLang's stdlib tree,
  44 under PBRemote core and 84 under PB-MCP src. The scan considered at most
  250 sorted source paths per root and skipped files over 64 KiB or symlinks.
  Nested source copies may be included; counts are not unique product modules.
  Allocation/cleanup keywords select ideas, not confirmed defects.

The 64 KiB and file-count limits apply only to this exploratory scan. They do
not impose limits on PBLang programs, the compiler or users' project sizes.
No private conversations, credentials, source bodies, machine paths or account
configuration are reproduced in this record.

## External primary references and adaptation ideas

These are PBLang design proposals inferred from capabilities, not claims that
those systems use PBLang or that their behavior can be copied automatically.

| Source | Capability examined | PBLang adaptation |
| --- | --- | --- |
| [Claude Code marketplaces](https://code.claude.com/docs/en/plugin-marketplaces) | Catalogs, versions, sources and packages containing skills/agents/hooks/MCP/LSP | Declarative extension metadata with pinned revisions and a separate activation step; connect existing package/release validation rather than inventing a second trust path |
| [Gemini CLI extensions](https://geminicli.com/docs/extensions/) | Packaged prompts, MCP integrations and custom commands | Portable capability descriptions with small host adapters; retain deterministic local implementations where available |
| [Official MCP Registry](https://github.com/modelcontextprotocol/registry) | Machine-readable registry and API | Import catalog metadata into a local discovery index with provenance; registry presence must not become execution authorization |
| [VS Code MCP integration](https://code.visualstudio.com/docs/agent-customization/mcp-servers) | Server management and tool integration | PBIDE capability browser, explicit tool selection, startup/stop state and inspectable results; reuse PB-MCP contracts |

No third-party source implementation was copied or relicensed. Actual imports
of third-party code would require provenance/license review and targeted tests.

## Concrete conversion queue

| Priority | Idea | Existing PBLang/product foothold | Required proof before claiming completion |
| --- | --- | --- | --- |
| 1 | Automatic cleanup for one callback-owned allocation | `memory_owned.sapp`, `memory.sapp`, PBLang VM scalar engine | Success, callback failure, nested use, immutable access, no owner/borrow growth on retained and self-host routes |
| 1 | Reliable explicit borrow guard | `MemoryTracker`, `SafeRef` | Correct explicit receiver, no reacquisition leak, fresh/explicitly released access rejected, scope-staleness limitation documented, no temporary borrow bypass to enable writes |
| 1 | Session-scoped desktop resources | PBRemote desktop/target-frame services and PB-MCP computer tools | Exact session generation, stop/revoke behavior and local end-to-end tests; existing validators remain validators |
| 2 | Generic cleanup state for files, transactions and streams | `strict_cleanup_transition.sapp`, `db.sapp`, file/stream libraries | Cleanup errors stay visible; do not generalize native exactly-once guarantees from a pure state model |
| 2 | Local extension capability index | Existing MCP routing, package manifest and release validation | Metadata-only intake, duplicate/version handling, provenance and separation of discovery from activation |
| 2 | Reusable document transformations | Existing sheet/form/PDF/graphics libraries plus document/media tool categories | Small format-neutral models, streaming where supported, round-trip fixtures and measured memory/disk costs |
| 2 | Reproducible evaluation and test selection | Existing audits, emulators, compiler and resource benchmarks | Exact source/artifact identity, affected-test selection with broad regression gates, honest coverage labels |
| 3 | Local search with evidence pointers | Existing inventory, Graphify and project audit tools; research/search plugins | Incremental indexing without storing unnecessary source content, stable references, measurable query cost |
| 3 | Model/tool routing for pbAI | Existing specialist/capability direction and model catalogs | Output-quality and resource benchmarks, CPU fallback, explicit reversible learning; catalog popularity is not evidence |

The next implementation is selected for correctness and testability, not for
marketplace popularity or number of features. Published progress must say which
row became code, which remains a proposal, and which existing code was merely
read. The remaining 1.0.0 runtime, backend, library, release, PBIDE and guest-OS
work is not closed by this inventory.
