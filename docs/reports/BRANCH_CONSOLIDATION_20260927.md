# Branch consolidation — 27 September 2026

The remaining allocation and atomic-plan-field branches are merged into the full
standard library. Shared portfolio modules for table quality review, subtitle
clipping and file adapters, study agendas and deck files, and loaded voltage
dividers are included with their existing semantic tests. Newer implementations
already present in the full library are retained.

The merged applications keep their current capabilities alongside these additions:

- PBCircuits: loaded-divider analysis alongside unit selection and tolerance.
- PBDataLab: advisory quality/review-row export alongside joins, groups, sorting,
  column selection and document merging.
- PBMedia: clip export alongside timing, reading-density navigation and drafts.
- PBStudy: agenda/due-copy export alongside editing, revision and review history.
- PBPlan: ready-task navigation with atomic draft handling and single render paths.
- PBLesson: explicit weighted allocation and PBStudy-compatible revision exports.
- PBStencil: orientation coverage alongside color and binary-mask interchange.

Dependency manifests must use the exact compatible library commit documented by
the application. Locks remain portable and their content checks stay enabled.
PBWatermarks uses the same tested library tree for PBStencil mask interchange.
The existing runtime is PBLang 0.6.0 on Linux; generated native/web/Android views
are structural previews, not physical-device or signed-release certification.
