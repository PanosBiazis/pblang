# Ordered sample smoothing

`smoothing_table.create(labels,values,window)` composes the existing
`sample_series.moving_mean` with Sheet outputs. `from_sheet(doc,window)` accepts
exactly Label,Value headers and 1–31 samples. `window(text)` parses 1–31 using
one or two ASCII digits after trimming, within 16 input characters. Windows
must not exceed the actual sample count. No time spacing is inferred.

Every complete trailing window is returned in input order: First and Last are
one-based data-sample indices, followed by From label, To label and Mean.
Labels remain literal, including blanks/duplicates. Values must be finite within
+/-1e9. Arithmetic is ordinary floating point; no exact decimal, statistical
confidence, anomaly detection, forecast or measured speedup is claimed.

`measure_table.parse_value(text)` exposes the existing measurement decimal
parser for reuse, without changing its grammar: optional sign, decimal point,
optional exponent, up to 64 characters, finite +/-1e12; surrounding whitespace,
commas, formulas and underflow-to-zero from nonzero text reject. Smoothing adds
its stricter +/-1e9 sample bound. Numeric input spelling is not retained in means.

`measure_smoothing.create(worksheet,window)` recomputes conversion results and
requires every row's exact target-unit symbol and mode to match. Different input
units are accepted when converted to that common target. Mixed symbols/modes
reject, including absolute-vs-difference temperatures. Results add Unit and Mode
columns. Source worksheet, order and conversions remain unchanged. Choosing
whether observations belong in one series remains the caller's responsibility.

All input and output Sheets retain their existing 32-row/8-column/256-byte-cell/
8192-byte-total limits. Repeated endpoint labels can exceed output byte limits;
the complete operation then rejects before saving. No partial windows, padding,
truncation, hidden sorting or source overwrite. `smoothing_files` uses table_files
for bounded 64 KiB reads and exclusive new outputs. Failed writes may leave a
partial new file; atomic visibility/crash durability is not promised.

`smoothing_view.create(source,window)` builds a renderer-neutral numeric overview
of every computed window, four per page, with static page selection actions.
Labels are omitted from this overview and remain in the saved table. Generated
Web/Android editions contain computed data but cannot reload files or recalculate.
No model, sensor, device, network, telemetry or automatic data retention is added.
Export files remain until explicitly deleted by their owner. CSV text may be
interpreted differently by third-party spreadsheet software.

The idea extends the sensor/build-duration recipes in DISCOVERY_RECIPES.md.
The sample-series kernel is existing work, now included in the published dependency
snapshot; it is not a newly invented algorithm. PBSmooth supplies the native file
workflow and PBMeasure supplies the unit-aware command. Windows/macOS and installed
Android are unverified. Generated Android classes are not a tested APK.
