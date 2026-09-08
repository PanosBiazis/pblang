# Taint analysis admission and completeness

The PBLang-owned taint analyzer distinguishes analysis execution (`ok`), its
admission decision (`passed`), and coverage flags. Never interpret an incomplete
report as proof of safety, or infer a complete result merely from a finding list
containing no high-severity entries.

## Numeric admission

`security_taint.limits(options)` validates ordinary finite integer budgets:

| Option | Default | Accepted interval |
| --- | ---: | ---: |
| max_taint_findings | 256 | 1..2048 |
| max_taint_steps | 200000 | 100..2000000 |
| max_call_expansions | 20000 | 1..200000 |
| max_taint_source_bytes | 1048576 | 1..134217728 |
| max_taint_tokens | 65536 | 1..1048576 |

Direct calls use defaults for omitted keys. Explicit nil, bool, strings, exact
number variants, containers, functions, fractional/nonfinite or out-of-range
values reject with value-free errors. This intentionally tightens the former
fallback/fractional behavior. The retained host threw an unstructured numeric
comparison error on the baseline NaN probe; unbounded NaN execution was NOT
demonstrated on that host. Missing finite/integer admission is now explicit.

Dependency analysis validates its resolved numeric policy before module reads,
in protect/strict/audit modes. Its existing option resolver treats nil as unset;
this legacy normalization happens before the shared validator. Deep policy
validation now follows numeric limits before the library starts path resolution,
source handling or import discovery. The limits API itself stays numeric-only.

## Custom policy admission and private defaults

Malformed nested policy formerly could produce a successful admission: a sink
severity of HIGH retained a finding but did not increment the high-only blocking
count, and omitted args silently became an empty list. Fractional argument indexes
were also accepted. These are malformed-policy failures, not evidence that source
text alone can rewrite an operator-owned policy.

Custom taint_sources/taint_sanitizers/taint_sinks are now validated before direct
analysis reaches numeric/source/token admission. Constant errors do not echo
the supplied policy or path. The existing top-level field errors remain.

| Consumed field | Contract |
| --- | --- |
| sources / sanitizers | Arrays, at most 128 nonempty names |
| names / sink keys / class | Strings, at most 256 UTF-8 bytes; no ASCII C0 or DEL |
| sinks | Map with at most 128 entries, counted before allocating its key list |
| each sink | Map requiring severity, class and args |
| severity | Exactly high, medium or low |
| args | Array of at most 32 ordinary finite integer indexes in 0..255 |
| control_flow | Optional exact bool; omission retains false |

Scalar length is checked before UTF-8 conversion, limiting that temporary byte
array to at most 1024 bytes. Empty replacement arrays/maps, explicitly empty
args, duplicate indexes and low severity remain intentional operator choices.
Explicit nil is malformed in direct calls; dependency option resolution retains
its existing nil-as-unset behavior. Unknown unused sink fields remain ignored.
Previously accepted malformed/oversized policy now rejects intentionally.

The unchanged built-in defaults are constructed once per imported module and
kept private/read-only. Callers cannot retrieve this table through module exports
or reports. No reader mutates it; custom branches do not overwrite it. This is
an ownership property of this implementation, not a new runtime-enforced deep
immutability guarantee. Caller policy containers are not cloned or protected
against concurrent mutation; snapshot/thread guarantees remain separate work.

The new configuration test covers malformed unused rules, exact caps, UTF-8 and
control input, input preservation, private/non-contaminating defaults, error
precedence and all three dependency modes. See
BENCHMARK_SESSION_2026-09-08_TAINT_CONFIGURATION.md for the original slice.

### Outcome-only preflight before dependency I/O

`security_taint.validate_policy(options=nil)` checks only custom taint policy
structure, using the same bounded validator as direct analyze. It returns exactly
`{ok:true}` or `{ok:false,error:constant}`. A non-map options argument rejects with
`taint policy requires map options`. No defaults, policy containers, source,
path, or approval capability is returned. Numeric budgets still require limits;
successful preflight does not mean a source program is safe or even analyzed.

`security_dependency_analysis.analyze_entry` validates the resolved policy after
its existing path/mode/numeric admission but before path normalization, entry
source handling, module reads, import probes, tokenization and pattern analysis.
Malformed policy returns exactly `{ok:false,passed:false,format:FORMAT,error:...}`
instead of a later incomplete traversal report. This intentional early-error
schema change does not alter successful reports. Numeric error precedence,
nested-policy priority, nil-as-unset and explicit empty replacements are retained.

Every per-module and whole-program analyze still validates again. Caller-owned
maps are mutable; an earlier success or a supplied validated flag never skips
validation. No cross-call cache or snapshot/atomicity guarantee is introduced.
The extra check adds cost, especially at the maximum custom-policy boundary.

This guarantee starts at the dependency library call, not process startup:
pbsecure and pbanalyze still load config/entry source before invoking it. CLI
pre-read admission and pattern-policy preflight remain separate work. Existing
non-map outer security fallback is unchanged. General parser diagnostics and
successful dependency reports are not universally path-free.

The focused preflight regression covers exact schemas, no container exposure,
mutated/forged outcomes, direct nil versus dependency fallback, nested priority,
all modes, malformed entry source, missing entry and unresolved-import markers.
Linux syscall observation is recorded separately from semantic assertions in
BENCHMARK_SESSION_2026-09-08_TAINT_POLICY_PREFLIGHT.md.

### Local repeated-class validation reuse

Within each custom-sink traversal, the analyzer retains only the last class
string that passed the existing text validator. An ordinary string equal to
that value need not repeat UTF-8 conversion and the byte/control scan. The
ordinary-string type guard precedes equality, so containers cannot invoke
comparison hooks. A nil sentinel is never a valid class; the first class and
every different class receive the full validator. Keys, schemas, severities,
argument indexes and control_flow are checked for every rule, even on a match.

This adds one local validated string reference, at most 256 UTF-8 bytes, not a
map of policies or a cross-call cache. It neither mutates nor exposes caller or
default containers. Replacing a class between calls triggers fresh validation;
all module/linked passes still revalidate. Concurrent host mutation of policy
containers remains outside the snapshot guarantees, as before.

Repeated labels can benefit; distinct labels still require all old work plus
the comparison. The paired benchmark includes 128 distinct 256-byte labels with
a 253-byte common prefix, as well as repeated labels and complete analyses.
See BENCHMARK_SESSION_2026-09-08_TAINT_CLASS_REUSE.md for measured costs.

Backend boundary: retained VM strings and the PBLang-written VM preserve exact
string content equality. The bounded self-host test checks equality/NUL suffix/
Unicode/reassignment primitives, NOT the complete analyzer on that VM. The
generic C99 runtime's strcmp equality and strlen-based len/str_to_bytes have a
pre-existing embedded-NUL truncation gap: both the original and optimized text
predicate see only the prefix. This optimization does not establish C99 analyzer
safety or fix that gap. The separate PBLang-owned minimal pointer/numeric C
transpiler is not qualified to run the full dynamic analyzer. No C/Rust runtime
source or language string semantics were changed in this slice.

## Fixed-field limit validation

The five static budgets are now resolved and checked directly, in the unchanged
order: tokens, source bytes, findings, AST steps, call expansions. This avoids
reconstructing a specification array and five specification maps for every call.
Validation still precedes analysis; no bound, fallback, error string or explicit
nil behavior changes. The result is a fresh map, not a shared mutable cache.
`security_taint_limits_contract_test.sapp` covers exact results/errors, first-error
precedence, boundaries, malformed values, input preservation and result isolation.
See `BENCHMARK_SESSION_2026-09-08_TAINT_LIMITS.md`: isolated validation improves,
but the complete analyzer has no demonstrated repeatable speedup. This is not
VM-wide heap enforcement or recovery of the earlier analyzer regression.

## Source admission before tokenization

Direct analysis defaults to a 1-MiB UTF-8 source cap. Oversized input rejects
before the tokenizer allocates its character array, with ok=false, passed=false,
parser_complete=false, interprocedural_complete=false and limit_exceeded=true.
The constant size error omits source and path. Empty source remains accepted.
This intentionally tightens formerly unbounded direct calls; trusted callers may
select a finite cap up to 128 MiB, not an unlimited mode.

The scalar count first rejects input that cannot fit. If at most cap/4 scalars
are present, valid UTF-8 necessarily fits and no byte array is allocated. Only
the remaining boundary region uses exact str_to_bytes measurement. This bounds
conversion length to at most four times the admitted scalar count, NOT its total
heap cost: boxed byte values and tokenizer/AST objects have additional overhead.
The caller's source string already exists; its creation/read is outside this API.

Dependency analysis derives the implicit cap from max_module_bytes, then uses
the existing linked-output byte budget for whole-program analysis. Explicit
top-level or nested security.max_taint_source_bytes is never widened. Nested
policy wins over the top-level value; nil retains the wrapper's unset semantics.
Examples: T.analyze(source, "fixture.sapp", map{max_taint_source_bytes:65536});
or a dependency security policy containing {"max_taint_source_bytes":65536}.

security_taint_source_bounds_test checks exact ASCII, Greek, emoji and combining
character boundaries, invalid limits, lexical-error ordering, privacy, and all
three dependency modes using a real two-module linked source. No source in the
fixture is executed by the analysis. Token-count, per-token growth, parser depth,
configuration processing and VM-wide heap/time limits remain separate work.

Source-admission validation: 15 targeted suites passed; final project build,
portable-lock publication, touched-file lint, privacy scan and diff check passed.
Lint retained warnings for intentional nonfinite comparisons, explicit boolean
comparison and test bindings. The initial run correctly refused the changed
dependency content hash until build/portable-lock refresh; the final build had
no validator-unavailable note. Graphify update again reached the 20-second
deadline without completing. No owned test/build/benchmark/Graphify process
remained. No full suite or executable replacement was performed.

## Lexical and finding completeness

`compiler/tokenizer.tokenize_bounded(source, max_tokens, identifier_spans=false)`
is an opt-in bounded API. Its result always contains ok, tokens, token_count,
limit_exceeded and error. The token cap includes comments and EOF. A cap of one
accepts empty/whitespace input only. Invalid inputs/limits reject with a constant
error and limit_exceeded=false; exhaustion returns limit_exceeded=true. Both
failures expose tokens=[] and token_count=0, never a truncated success stream.
The guard runs after skipping whitespace but before constructing the next token
or its text, reserving one slot for EOF. Ordinary tokenize retains its previous
signature and array-returning behavior and does not opt into this cap.

Taint analysis now uses this API with max_taint_tokens (default 65536, including
EOF; finite integer 1..1048576). Dependency import enumeration uses the same
validated cap and stops the failed module before pattern/taint passes or linking.
Protect, strict and audit all reject the incomplete graph. The same cap is kept
for linked whole-program taint, including when implicit: it is not automatically
scaled with module count. Larger trusted projects may set a larger finite cap.
This is intentional admission tightening, not a grammar or token-schema change.

The source-byte check remains essential. The character array is created before
token counting; whitespace and a single long token can still consume work
proportional to admitted source. String literals still use repeated decoded
concatenation; identifiers, non-radix numbers and comments now join consumed scalar
spans once (see TOKENIZER_LEXEME_CONSTRUCTION.md). Temporary slices and output
strings still have token-sized memory costs; no hard heap improvement is claimed.
The source inliner's internal tokenizer is still byte-bounded, not covered by
the new aggregate taint token cap before linking. Other legacy tokenizer callers
are unchanged. No VM-wide heap/time bound follows from this feature.

Existing parser expression-depth (256) and type-depth (64) guards are already
implemented; generic prior references to parser nesting as remaining work must
not be read as absence of these guards. The targeted expression-depth regression
passes. Comprehensive coverage of recursive paths and resource costs is still
separate work; no parser implementation was changed in this slice.

Token-admission validation: 21 targeted suites passed. The extended bootstrap
probe additionally passed the bounded tokenizer inside the PBLang-written VM,
including exact-cap success and overflow without partial tokens. Final build,
portable-lock publication, touched-file lint, privacy and diff checks passed.
Lint reports deliberate nonfinite-number comparisons, boolean style and test
binding warnings. Graphify reached its 20-second deadline without completion;
no graph-freshness claim is made. No owned workers remained, and unrelated dirty
work was preserved. No full suite, Rust change or executable replacement ran.
See BENCHMARK_SESSION_2026-09-08_TAINT_TOKEN_BOUNDS.md for the higher analyzer median
and nine separate numeric rows; no performance improvement is claimed.

Tokenization runs once; failure or an unterminated string/comment rejects before
parsing, with parser_complete=false and a constant lexical diagnostic. Existing
general parser diagnostics may still contain source token text; this is not a
path-free or universally redacted diagnostic API.

Finding deduplication precedes the retained-record cap. A repeated retained
identity is not truncation. A new unique finding that cannot fit sets truncated,
adds a blocking reason and makes interprocedural_complete=false. Omitted
identities are not accumulated in a second unbounded index. Finding_count remains
the number of retained records, not a newly invented total count.

This closes a reproduced fail-open: with a cap of one, a medium HTTP finding
could fill the index, a later high file-write finding could be omitted, and the
old report still said passed=true. The new direct and dependency tests require
rejection. Exact capacity with no lost evidence remains valid; duplicate testing
forces two different call-flow expansions that generate the same finding.

AST-step and call-expansion exhaustion also reject and report incomplete
interprocedural coverage. Dependency callers count both per-module and
whole-program taint failures as blockers in protect/strict; audit additionally
requires complete coverage. A failed analysis cannot be converted into approval.
Blocking counts may increase because incomplete analysis is a reason independent
of retained high-severity counts.

## Validation and boundaries

Sixteen focused suites pass, including numeric/type/boundary/privacy cases,
lexical errors, medium-before-high truncation, deduplication, AST/call exhaustion,
all three dependency modes, existing source/taint regressions, and security CLI
report/config behavior. No full suite ran. Independent defensive review found
no concrete regression.

Final project build and portable-lock publication passed. Touched-file lint
completed with warnings: the deliberate nonfinite-number comparisons trigger
eq_op, explicit token-flag comparison triggers a style warning, and test/benchmark
bindings trigger unused-variable warnings. Owned-file privacy checks and
git diff --check passed. Graphify update reached its 20-second deadline without
completion; no refreshed-graph claim is made. No test, benchmark, build or
Graphify worker remained. Pre-existing unrelated work was preserved.

No sink severity was lowered, no control-only cleanup was exempted, and no cache
or analysis budget was enlarged. Control-only cache cleanup remains conservative
blocking evidence; classifier metadata and summary-cache optimization are
separate work.

These are analysis-budget and completeness fixes, not VM-wide hard heap or
wall-clock enforcement. Direct analyze now has a pre-tokenization source-byte
ceiling and an EOF-inclusive token ceiling. Dependency callers provide
module/aggregate source limits, with token admission at imports and final taint.
Nested flow structures, recursive summaries, full language/alias/effect coverage,
cache confinement, same-object cleanup and complete compiler bootstrap remain
unqualified. No full native compiler rebuild was retried in this slice and the
host executable was not changed. PBLang 1.0.0 remains NOT READY.

Measurements and raw samples are in
`BENCHMARK_SESSION_2026-09-08_TAINT_ADMISSION.md`; no speedup, universal benchmark
win or cross-device readiness is claimed.
