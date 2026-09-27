# Reversible measurement rows and single-pass results

`measure_document.reverse_row(doc, index)` uses a zero-based integer row index.
It validates and detaches the source worksheet, computes the selected result,
and exchanges source/target units with that result as the new input. Title,
label, order and absolute/difference mode are retained. It validates the complete
candidate before returning, so the input remains unchanged on any error.
Bounds and floating-point approximation still apply; double reversal is not an
exact restoration operation. Undo restores the actual prior document.

Native PBMeasure exposes this as Reverse row after the existing draft guard,
using the existing bounded history and new-copy store. CLI `reverse` accepts a
one-based row and writes one new worksheet. Existing readers and exports use the
same model. No persisted fields or format versions changed. Web/Android remain
readers, not editors; physical device/platform validation is separate.

`results` previously called `validate` (which calculates every conversion) and
then calculated the same conversions a second time. A private checked traversal
now optionally collects results as it validates. `validate` retains its boolean
contract and `results` returns a detached numeric array. Validation is still
linear in row count and label length; conversion work is performed once per
row instead of twice. There is no cache invalidation or persisted derived value.

`measure_reverse_test.sapp` checks ordinary units, absolute/difference
and double-reversal tolerance across all admitted catalog pairs, exact old/new
results, detached source preservation, malformed rows/indices and 31-row output.
Product tests cover the button, pending drafts, undo, failed persistence and
unsaved-close retention. File tests exercise new-copy persistence and table
roundtrips. The old source is retained as the exact benchmark fixture.

Run `pblang run bench/measure_results.sapp --allow-all`: mixed resistance,
absolute temperature and temperature-difference workloads with 1/16/31 rows,
three warmup batches, nine alternating samples and ten calls per sample. Output
is wall-clock milliseconds per results call. Median and nearest-rank p95 (maximum
of nine samples) describe that run only; disk I/O, GUI latency and device execution
are outside this benchmark. No universal speedup is asserted.
