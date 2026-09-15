# Idea intake: ownership of failure cleanup — 2026-09-16

This is a bounded refresh, not a full reading of every disk, conversation or
marketplace. A selected prior desktop-plugin conversation reaffirmed PBLang-first
implementation and Rust as retained reference. No private conversation contents
are reproduced here. The concrete loader gap came from current source inspection.

Read-only inventory: 253 cached manifests across Codex/Claude cache layouts;
986 callable tool metadata entries; Claude CLI reported 32 installed entries,
30 configured enabled and two disabled. Manifest files are not distinct active
plugins and configured status is not proof of successful loading. No plugin was
installed, enabled, disabled or invoked as a third-party integration. Credentials
and account content were not inspected or published.

## Primary sources and transfers

- [MCP lifecycle, 2025-06-18 revision](https://modelcontextprotocol.io/specification/2025-06-18/basic/lifecycle)
  separates initialization, operation and shutdown. Transfer inferred: the owner
  of an execution boundary must complete teardown when normal execution fails.
  Implemented locally in the existing PBLang loader error boundary; this is not
  a new MCP implementation or an update of the cited protocol revision.
- [LangGraph functional API](https://docs.langchain.com/oss/python/langgraph/functional-api)
  discusses tasks and idempotent effects during re-execution. Transfer inferred:
  memory cleanup must not silently retry a failed script or claim to undo its
  effects. A seeded-global marker regression verifies one execution on failure.
- The prior plugin-discovery intake remains applicable: catalog metadata, installed
  configuration and verified executable behavior are separate evidence. Future
  PBLang capability/automation catalogs should retain those distinctions.

Delivered: deterministic scalar cleanup in loader success/failure paths,
regressions for lifecycle and failure contracts, an observable scratch comparison,
reproducible resource measurements and updated memory-runtime documentation.
Pending: exception-preserving cleanup for direct VM/frontend calls, shared-value
ownership, native/FFI resources, signed evidence, PBIDE and guest platforms.
No autonomous retry system, plugin installer, background task, external AI call
or Specisoft/Hetzner execution was added.
