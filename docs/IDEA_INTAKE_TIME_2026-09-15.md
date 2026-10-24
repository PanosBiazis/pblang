# Idea intake: declared versus executable capability — 2026-09-15

Scope: bounded metadata and source-document refresh, not every disk/file,
conversation, plugin implementation or marketplace. A recent page from the prior
scheduled-information conversation was sampled for developer-tool ideas; its
news claims were not used as verified technical evidence or copied into source.
The current cache/clock failure provided the concrete implementation priority.

253 cached plugin manifest files were read (219 in the Codex cache and 34 in the
Claude cache), covering both `.claude-plugin` and `.codex-plugin` manifest layouts.
This is a file count, not 253 distinct installed or enabled plugins. Missing
manifest versions and duplicated formats prevent interpreting raw name/version
differences as newly installed plugins. Current callable tool metadata contained
986 entries. A separate read-only Claude CLI inventory reported 32 installed
entries, 30 configured enabled and two disabled; actual loading was not tested.
No plugins were installed, enabled or disabled; no credentials/account contents
were inspected or published.

## Primary sources and proposed transfers

- [MCP Registry](https://modelcontextprotocol.io/registry/about) separates published
  metadata from executable packages. Transfer: treat declared availability and
  verified execution as distinct evidence. Concrete correction in this slice:
  bridge the already-known time names to the PBLang VM dispatcher and test them.
- [MCP discovery draft](https://modelcontextprotocol.io/specification/draft/server/discover)
  describes version/capability discovery and says server identity is self-reported.
  This is draft material, not a compatibility requirement adopted by PBLang.
  Proposed transfer: a versioned backend capability matrix backed by executable
  smoke tests, with unsupported/deterministic/effectful categories kept explicit.
- [Claude plugin discovery](https://code.claude.com/docs/en/discover-plugins) separates
  marketplace discovery and local plugin management. Proposed transfer: distinguish
  known, installed, enabled and tested states in future PBLang tooling.
- [VS Code agent tools](https://code.visualstudio.com/docs/agents/run/tools) separates
  built-in, MCP and extension tools and supports selecting tool groups. Proposed
  transfer: present only verified capabilities for the selected PBLang backend,
  while retaining discoverable unsupported features with explicit status.

Delivered: strict zero-arity time dispatch, alias/shadowing compatibility,
automated clock and cache TTL regressions, current resource controls and canonical
documentation. The existing cache get_or_set automation now works with expiry on
the tested frontend. A general capability matrix/plugin catalog, monotonic clock
and deterministic injected-clock facility remain proposals, not delivered features.
No external AI service was called for inference, no user content was uploaded,
and no EnterpriseCore/Hetzner system was used for PBLang execution.
