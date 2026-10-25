# Path-preserving module analysis

`stdlib:code_graph_paths.sapp` analyzes supplied import records using complete
canonical virtual paths. `a/util.sapp` and `b/util.sapp` remain different nodes.
It performs no filesystem discovery, source execution, or network access.

```sapp
let paths = import("stdlib:code_graph_paths.sapp");
let result = paths.module_graph_paths(map{
    "main.sapp": map{ imports: ["a/util.sapp", "b/util.sapp"] },
    "a/util.sapp": map{ imports: [] },
    "b/util.sapp": map{ imports: [] }
});
if result.ok { print len(result.graph.order); } // 3
```

`module_resolution(analyses)` returns `ok`, `complete`, `corpus`, `resolutions`,
`unresolved`, `ambiguous`, `invalid`, and `basis: "supplied-imports"`.
`module_graph_paths(analyses)` also returns `graph`:

- `ok` means valid, unambiguous input. `complete` additionally means every
  supplied import has a target in the supplied map.
- Valid missing targets are `unresolved` records and dangling graph nodes.
  They are not silently treated as external packages known to exist.
- Malformed inputs or duplicate canonical identities produce `ok: false`,
  an empty corpus, and `graph: nil`. Diagnostic resolution records may remain;
  callers must inspect `ok` before treating them as an accepted graph.
- Resolved and unresolved records preserve importer, raw spelling, target and
  status. Collision records identify the canonical target and supplied aliases.
  Invalid input diagnostics omit arbitrary raw values. No file contents are
  copied into the corpus. Caller-supplied names remain caller-controlled data.

The source linker's additive `resolve_module_identity(raw, importer, candidates)`
API supplies the same rules as existing linking: project-rooted bare imports
win, with importer-relative compatibility fallback only if the rooted candidate
is absent. Explicit `./` and `../`, package schemes, and the `stdlib/` alias are
preserved. Matching is case-sensitive with no suffix search, extension inference,
filesystem probe, or basename matching. Candidate values are ignored. The public
wrapper requires an empty or already canonical importer and a candidate map;
invalid input returns `nil`, while a valid missing target returns its identity.

Graph identifiers currently use `[A-Za-z0-9_/:.-]`, matching the legacy lexical
scanner's path vocabulary. Other names are explicitly rejected by this graph
representation, even if they are valid language imports. Future wider filename
support requires reversible graph-ID encoding. The shared resolver retains its
existing 4,096-character raw-path check. Neither restriction introduces a new
compiler source-size, duration, or project-module-count limit.

Resolution uses an index and at most two candidate lookups per import. Corpus
construction joins link fragments once per module. Work and output scale with
supplied paths and imports; no fixed project count cap is imposed here. Tools
accepting untrusted projects must set their own admission/work budgets. Loading
this module also loads the existing source inliner and its dependencies; this
change does not claim lower startup RAM or improved compilation speed.

Existing `code_graph.module_corpus`, `module_graph`, `report` and call-graph APIs
retain their old behavior, including basename/function-name collisions. Existing
CLI/MCP disk walkers have not been switched to this API. `code_graph.analyze_all`
can provide its input, but that scanner is lexical: comments and strings may
contain apparent imports. An accepted graph is not a compiler dependency proof,
an authenticated report, or evidence that a program runs. Read-only knowledge
graph queries/JSON work with these identities; vault export is not verified for
nested paths or package schemes.

Run the regression suite with:

```sh
pblang run tests/semantics/code_graph_paths_test.sapp --allow-all
```

It covers duplicate stems, canonical collisions, malformed inputs, graph markup,
relative/root/package resolution, explicit missing nodes, key-order independence,
legacy corpus compatibility and 2,400 supplied modules.
