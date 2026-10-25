# Literal table reshaping

`stdlib:table_reshape.sapp` supplies `transpose(doc)`, `unpivot(doc,columns)` and
`apply(doc,action,columns=nil)`. Actions are exactly `transpose` and `unpivot`.
Transpose accepts omitted/nil/empty columns only. Column indices in the library
are zero-based; product input uses the existing one-based `table_columns.parse`.

Transpose exchanges every cell, including any header, without special treatment.
The source must have at most eight rows so the result fits the Sheet column limit.
Applying transpose twice reproduces the source exactly.

Unpivot requires unique, nonblank literal headers and at least one measure and
one retained identifier column. Identifiers remain in source order; measure
columns expand in caller order within each data row. Output headers are retained
identifier names followed by `Variable,Value`. Retained identifier names cannot
collide with these exact names. Blank measure values and repeated identifiers
are preserved. Header-only input yields a header-only output. There is no type
conversion, grouping, aggregation, sorting, name normalization or reverse pivot.

Both operations return detached Sheets. The existing 32-row, eight-column,
256-byte-cell and 8192-byte-total limits apply. Expanded dimensions and repeated
UTF-8 byte cost are checked before output rows are constructed. Any unsupported
input/output rejects in full; nothing is truncated and the source is unchanged.

`reshape_files.open/save/run` compose this pure model with `table_files`.
Reads are bounded to 64 KiB; outputs require a new filename in an existing folder.
The input delimiter is also the CSV output delimiter. `.pbsheet` preserves the
literal Sheet model. Save failures can leave a partial new file; no atomic or
crash-durable success is claimed. No network, telemetry, shell, executable recipe
or hidden persistence is added. Successful exported files contain caller data;
they remain until the caller removes them. Formula-looking values remain literal
here, but another spreadsheet application may interpret exported CSV differently.

PBReshape provides the native file workflow, and PBDataLab adds the same commands.
Web/Android exports use the existing one-row, all-column reader: no file picker,
reshape computation or editing is embedded in those snapshots. Other rows require
separate exports. Linux was the local execution target; generated target code is
not evidence of installed Windows/macOS/Android support. No speedup is claimed.

The product idea extends the data/measurement recipes in DISCOVERY_RECIPES.md:
reshape synthetic repeated readings or survey fields for existing selection,
profiling and count tools. No private source/data was copied from prior scans.
