# Form checker integration validation

Validated on local Linux with the retained PBLang runtime. This is a scoped form
preflight integration, not full Pascal application conversion or PBLang 1.0 readiness.

## Behavior and coverage

- Shared pure schema/renderer used by CLI, PBIDE selected-buffer checks and the
  PB-MCP `pblang_form_check` tool. Source is data; imported handlers stay inactive.
- Web PBIDE: actual browser submissions of a synthetic supported DFM and an
  unsupported binary-marker input produced the expected summaries. Listener stopped.
- Native PBIDE: F6 caller wiring, adapter tests and lint checked; no live native
  GUI session was exercised. The F5 execution-check route remains unchanged.
- Offline MCP: exact configured launch command tested; 15 tools listed, positive,
  malformed/oversized, source-privacy, recovery and clean-close checks passed.
  Package includes 20 pinned files, with no new worker effects enabled. Existing
  Python OS transport packaging and the retained host are still present.
- 17 distinct semantic suites passed across form import/document/overlay/web,
  PBIDE editing/history/import/checking, MCP registry/protocol/source graph and
  neural preprocessing/training. Eight offline packaging/transport tests passed.
  Source-graph verification additionally compared 25 generated source fixtures.
- Lint on 18 touched source/test/boundary files passed with warnings. Five focused
  entry-only security scans passed; these are static checks, not complete audits.
  The existing loopback HTTP trust-boundary finding remains explicitly scoped.
- Isolated web-entry build and portable-lock publication passed. A subsequent
  manifest check passed. The minimal offline builder reports an unavailable
  manifest-validator module; its fixed inventory, hashes, relative locks and
  actual guarded execution were checked separately.

The cooldown test requires the retained runtime's execution capability only for
`sleep_ms`; the offline checker itself was exercised with worker effects denied.
The full canonical MCP registry test needs its declared watermarks dependency;
missing fixture dependency/lock failures were resolved before its passing rerun.

## Reproducible scoped measurement

Compare the parent PBLang commit `2f63f14841cfdc6fdf1dd730dc3e2af8d53a131a`
with this change using the same retained executable and isolated manifests.
Import `ui_form_check.sapp`, then run `analyze_dfm` 200 times over an identical
synthetic TForm containing one TEdit and one TButton with an OnClick name. Assert
`ok` and exactly two widgets every iteration. Deny exec/FFI/network/filesystem/
HTTP/SQLite/spawn/graphics/environment effects and allow imports. Measure complete
process CPU, wall time and peak RSS with `/usr/bin/time`. One warmup per version,
then seven samples per version, alternating which version runs first. p95 below
is nearest-rank, therefore the maximum of these seven samples; it is not a
statistically established tail-latency guarantee.

| Metric | Parent median | Changed median | Parent sample p95 | Changed sample p95 |
| --- | ---: | ---: | ---: | ---: |
| CPU seconds | 1.34 | 0.82 | 1.96 | 1.03 |
| Peak RSS KiB | 32,716 | 23,828 | 32,972 | 24,132 |
| Wall seconds | 1.34 | 0.83 | 1.99 | 1.04 |

This workload includes startup/import costs. Removing unnecessary HTTP/OS imports
accounts for part of the gain: approximately 39% lower median CPU and 27% lower
peak RSS here. It does not establish whole-application, Live View, GPU/NPU or
steady-state server acceleration. The added web rejection policy has not received
its own isolated performance benchmark.

## Rejected-request policy and remaining work

`throttle_rejected_requests` is ON by default in `ui_form_web`. Eight responses
with status 400/403/404/405/415 in a fixed minute trigger a 30-second cooldown.
Tests cover default/independent enablement, opt-out, invalid options, preserved
Host checking, close during cooldown and real expiry. It does not identify a
pentest or guarantee attack prevention. One local client can consume the shared
preview budget. It is not a public authenticated multi-user deployment boundary.

Full Pascal/other-format behavior conversion, spatial/multidimensional interfaces,
persistent accounts, general runtime/FFI/GC work and wider platform verification
remain open. Scraping discouragement and internal-name minimization do not stop
all scraping or reverse engineering. The separately installed desktop-control
PB-MCP was not modified; no Live View benefit was measured. Graphify refresh timed
out and its process group was stopped. No EnterpriseOrg server was used.
