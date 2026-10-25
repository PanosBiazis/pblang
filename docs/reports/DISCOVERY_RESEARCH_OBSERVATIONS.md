# Bounded research observation review

`research_observations.sapp` validates one measured variable across up to 256 rows,
converts declared units, and returns pooled descriptive statistics and missingness.
It composes existing `units.convert_checked` and the full-window mean from
`sample_series`; it does not introduce a conversion engine or statistics backend.
The example also uses the existing `recipe_manifest` contract.

## Contract

`manifest(spec)` and `summarize(spec, rows)` require exactly these spec fields:

| Field | Meaning |
|---|---|
| value_unit | Exact symbol from the existing checked units catalog, such as cm or degF |
| summary_unit | Compatible output symbol, such as m or degC |
| value_mode | absolute or difference; explicit affine temperature semantics |
| time_unit | A unit whose category is time |
| timebase | Exactly relative; no implicit epoch, local timezone or clock lookup |

Each row has exactly `subject`, `time`, `value`. Subject is a caller-assigned integer
slot 1–64, independent of species or person. It is not an identifier generator,
anonymizer or verified identity. Time is finite, nonnegative and at most 1e9 in the
declared input unit; its conversion to seconds must be at most 1e12. Times must
increase strictly within each subject, including rows with missing values. Subjects
may interleave. All relative times must share a caller-declared origin externally;
this function cannot verify origin alignment or clock accuracy.

Values are `nil` or finite numbers within ±1e9 both before and after conversion.
`nil` is the only missing marker. Unknown fields, strings masquerading as numbers,
incompatible units, invalid modes, nonfinite values, duplicate/reversed subject
times, too many rows and invalid absolute temperatures are rejected. An invalid
late row throws; no partial report is returned. Even empty/all-missing datasets
validate the unit pair and mode.

## Output and interpretation

The summary contains counts, missing fraction, min/max/mean and global time span
in seconds, but no original rows, subject slots, per-subject statistics or timestamps.
Missing values count toward rows and missingness, but never value statistics. No
imputation or interpolation occurs. All-missing values produce nil min/max/mean;
an empty dataset also has nil fraction/span. One row has a zero span.

The mean weights each observed row equally, not each subject or elapsed interval.
Repeated observations may be dependent. The global span assumes a common origin;
it is not cumulative observation effort. Summary values use floating-point
arithmetic, not exact decimals, uncertainty estimates, causal/statistical inference
or evidence of scientific validity. Aggregates may still disclose information,
especially for a single subject. `safe_to_log` remains false. No k-anonymity or
differential privacy is claimed. Caller scripts containing private literals can
expose them through runtime diagnostic source excerpts.

The deterministic method manifest records the schema/method version, unit choices,
missingness and weighting rules, primitive names and bounds. It contains no dataset
fingerprint, runtime/source pin or authenticated provenance. Repeatability requires
the caller to retain the exact input, module/runtime versions and provenance under
appropriate access controls. The manifest does not prove reproducibility.

## Run

`pblang run tests/semantics/research_observations_test.sapp`

`pblang run examples/discovery_research_observations.sapp`

The synthetic example reviews plant-length observations and environmental
temperature values, then checks a declared recipe against derived counts. This is
usable infrastructure for PBDataLab/PBLesson workflows, not a clinical tool, sensor
driver, completed research product or claim to cover every organism or purpose.
No files, network services, sensors, external models or private datasets are used.
