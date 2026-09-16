# Idea intake and implementation — 2026-09-16

## Bounded coverage

Reviewed the current request history, the task catalog and the latest turn of
“Βρες plugin για έλεγχο οθόνης”. Its explicit PBLang-first / retiring-Rust direction
remains applicable. This was not an exhaustive reading of every conversation.

Read metadata from 253 cached plugin manifests (219 Codex, 34 Claude), plus the
986 callable tool metadata entries exposed in this session. Counts are files or
entries, not unique plugins or proof of active connections. A read-only Claude
plugin listing reports 32 installed entries, 30 enabled and 2 disabled. Catalogs,
installed entries, enabled configuration and callable capabilities remain distinct.
No plugin was installed, enabled, disabled or invoked for external account data.
No fresh whole-disk content scan was performed; this pass focused on plugin
metadata and the complete Android emulator module and its relevant tests/example.

## Sources and transfers

- [Claude marketplace documentation](https://code.claude.com/docs/en/discover-plugins)
  distinguishes catalog discovery from individual installation. Proposed transfer:
  PBIDE/pbAI should show discovered, installed and enabled as separate states.
- [Gemini CLI extensions](https://geminicli.com/docs/extensions/) package multiple
  capability kinds and expose a listing/status workflow. Proposed transfer:
  a PBLang-native capability inventory should retain kind, provenance and state
  instead of counting a bundle as a single executable guarantee.
- [VS Code Disposable API](https://code.visualstudio.com/api/references/vscode-api#Disposable)
  provides explicit resource disposal. Our analogous implementation choice is to
  make the existing Android execution boundary own and close its VM context.
  This is a design analogy, not copied code, API interoperability or automatic
  translation of an extension.

## Delivered and next work

Delivered: Android `run_pbcs` now closes scalar ownership state after success and
caught failure, before any event-recording branch. `compile_and_run` inherits
this behavior; the existing Android example demonstrates containment and a fresh
subsequent execution. No automatic retries or transaction rollback were added.

Next: direct VM/frontend failure cleanup requires preserving original error
semantics; the current catch expression does not bind arbitrary thrown values.
Composite/shared ownership, native runtime/FFI, backend parity, libraries,
audits/release authenticity, PBIDE Run/Designer and guest-OS tests remain open.
A capability inventory UI is a proposal, not implemented in this slice.
Version 1.0.0 is not ready. No Specisoft server or remote device was used.
