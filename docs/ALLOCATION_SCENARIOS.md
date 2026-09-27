# Two-total allocation scenarios

Existing Discovery Recipes and ALLOCATION_WORKFLOWS describe allocating workshop
seats, game resources and test budgets. `allocation_scenario.compare` adds a
read-only comparison of the **same weighted rows** at two totals. It is not a
comparison of two files or a capacity optimizer. No new algorithm is introduced.

`compare(doc, before_total, after_total)` delegates both calculations to
`allocation_table.from_sheet`, retaining its validation and existing largest
remainder method. Each returned Sheet row is Label, Weight, Before (total),
After (total), Change. Change is after minus before, including negative values.
Input order is preserved; duplicate labels remain distinct recipients. The two
headers embed the actual totals so a saved table retains scenario context.

The detached report has format `pblang.allocation-scenario.v1`, document,
before_total, total (the after total), change (after minus before), rows,
increased, decreased, unchanged and method. Direction counts classify rows;
they are not counts of units. The source is not mutated. Both shares sum to
their stated total and changes sum to the difference between totals.

For weights 1,3,3 and totals 3 -> 4, shares change from 1,1,1 to 0,2,2. The first
recipient loses a unit while the total increases. This expected behavior is
visible in the result; no minimum, monotonicity or inferred fairness guarantee
is added. Equal remainders retain input order; zero weights remain zero.

Both totals and all weights must be integers 0..1000000 with a positive weight
sum, even for zero totals. Exact Label,Weight input headers and 1..31 recipients
are required. The five-column output must fit Sheet limits (32 rows including
header, 256 bytes/cell, 8192 cell bytes). A valid source or valid single allocation
can exceed the comparison budget; reject the complete result without truncation.

`allocation_scenario_files.open` reads one bounded table snapshot and returns
the complete comparison. `run` validates first and saves through existing
allocation_files/table_files/secure_file adapters. It accepts the same comma,
semicolon and tab delimiters, .csv/.tsv/.pbsheet formats, and exclusive new-file
rules. The source is read only once per comparison command. Saving a displayed
snapshot does not monitor later external changes to the source. Failed writes
may leave partial new files; no atomic visibility or crash durability guarantee.

PBPortions exposes compare/compare-save commands and a second-total input with
Compare. Ordinary Calculate remains available. Saving uses the displayed mode's
settings: source, first total and delimiter always matter; the second total
matters only for comparisons. Any failed calculation or comparison marks the
previous result and blocks saving until a new successful calculation, including
when fields are restored. Pagination retains the scenario and direction summary.

Both CLI and native UI share this implementation. Existing table readers expose
all five cells of one selected result row for native/web/Android generation.
Readers do not recalculate, edit source files or synchronize scenarios. Physical
Windows/macOS/mobile execution remains unverified for this slice.

No network service, model call, telemetry, private scan content or third-party
dependency was added. Reports and saved files intentionally include supplied
labels and weights. They are not anonymous and remain until the caller deletes
them. CSV remains literal; use a text import mode in external spreadsheet tools
or PBOffice Sheets, which does not evaluate formulas. The library does not
provide general-purpose spreadsheet sanitization.

Evidence: shared allocation_scenario tests cover 169 ordered total pairs,
conservation, reversal, equal/zero/max totals, decreased shares, duplicate labels,
all 31 recipients, rejected output overflow, source detachment and three-target
view compilation. Existing integer/allocation/oracle tests and PBPortions
app/preview/files/recalculate/scenario suites remain relevant. This is a bounded
functional extension; no new performance claim is made.
