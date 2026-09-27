# Product branch integration and source validation

The product release, product workflows, tally, lookup, workday, palette and word-location branches are integrated without replacing newer shared UI behavior. Equivalent files with differing executable bits or line endings were resolved without changing their semantics.

Integration validation found a missing `algorithms_advanced.levenshtein_within` export. It now delegates to the already tested bounded implementation. The measurement reverse-row extension is shared with the portfolio-library distribution so PBMeasure can use the full PBLang source tree as well.

Validation uses the installed PBLang 0.6.0 bootstrap with this checkout's libraries and regenerated, repository-relative dependency locks. The 62 integration suites pass after the missing export repair; the existing algorithms regression and the 820-pair measurement reversal suite pass as well. Companion product suites and isolated file roundtrips are validated separately. This does not claim a rebuilt bootstrap or Windows/macOS/mobile execution.

The new Source validation workflow checks source conflict markers, package metadata, declared entrypoints and published checksum lists. It is explicitly separate from runtime tests. Hosted Actions were blocked by the account billing/spending restriction during this work; no financial settings were changed, and source checks do not replace the existing CI or VM matrix.
