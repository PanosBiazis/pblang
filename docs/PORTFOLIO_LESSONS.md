# Portfolio lessons and presentations foundations

The third slice closes a concrete PBOffice gap and implements the lesson-workbench idea in `PRODUCT_PORTFOLIO_ROADMAP.md`. The migration inventory's shared-presentation examples and existing Modern UI action IR guided the architecture; this increment imports no external source code or private scan contents.

## Shared modules

- `text_pages.sapp`: input <=8192 UTF-8 bytes, 1–120 display cells per row, 1–32 rows/page and <=256 pages. Existing text clusters preserve supported combining/ZWJ groups, with deterministic display-only control normalization. This is hard cell wrapping, not a complete Unicode or pixel layout engine.
- `slide_document.sapp`: bounded text slides, detached edit/insert/delete/reorder, strict native JSON and explicit Writer conversion. See PBOffice `docs/PRESENTATIONS.md` for exact schema/limits.
- `slide_view.sapp`: one selector built from portable set-text/sequence actions, including all slides and excluding speaker notes. Eight slides fit the existing action/widget limits.
- `study_view.sapp`: four-card paged Reveal/Hide view and compact eight-card lesson reader. Local answers are included in generated output; no review persistence is implied.
- `lesson_bundle.sapp`: strict common-source bundle. It validates both derived formats before accepting an import and stores only original notes/date. Exported cards/slides are detached and evolve independently.
- `study_cards.sapp`: due-date ordinals cached before insertion-sort comparisons. Stable ties and detached output are unchanged. The sorter remains bounded O(n^2), now without repeated date parsing inside comparisons.

## Measurements

Same retained runtime and Linux host, all cards due in reverse date order. Deck construction outside the timer, two warmups, seven samples, three due-list calls/sample, runtime `clock()` elapsed ms. p95 uses nearest rank (maximum for seven samples). Reproduce using `bench/study_due.sapp`; compare against the pre-change module in an isolated copy. This measures due-list creation, not the entire application or other platforms.

| Cards | Before median / p95 ms | After median / p95 ms |
|---|---:|---:|
| 16 | 6.482 / 7.580 | 1.180 / 1.245 |
| 64 | 98.244 / 103.623 | 5.669 / 5.811 |

Physical Windows/macOS and installed Android verification, complete interactive editing/persistence parity, wider slide formats/layouts and a unified office launcher remain outstanding. Full lessons do not require AI, GPU, foreign application runtimes or source execution.
