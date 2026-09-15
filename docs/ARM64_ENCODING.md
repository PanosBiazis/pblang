# ARM64 instruction encoding and regression checks

## Correctness and compatibility

The AST backend now expands large stack-frame adjustments into valid AArch64
ADD/SUB immediate instructions. For example, a 4,096-byte frame previously
spilled beyond the unshifted immediate field; it now uses immediate 1 shifted
by 12. A 4,112-byte frame uses that instruction followed by an adjustment of 16.
Frame teardown uses the same encoder, preserving the caller's stack pointer.

Frame slots beyond the compact load/store range use reserved scratch register
x16 (IP0) to form the address. No call occurs between address formation and
access. Stores from x16 reject rather than overwrite their own value. Incoming
stack arguments use this same addressing path. Current callers derive exact,
nonnegative slot indices from compiler-owned metadata.

Signed branch offsets and compact load/store/arithmetic immediates are validated
before encoding. An unrepresentable value now produces a fixed compiler error
instead of silently changing instruction fields. CLI failure preserves an
existing output artifact, and diagnostics do not echo the source.

These are machine-instruction representation checks, not new source-size or
execution-time limits for the language. This slice does not widen ARM64's
existing strict source-admission API. Large array/map literal heap bumps and
outgoing argument areas now use expanded lowering too. Conditional source branches now expand beyond imm19; branches still
reject beyond the signed imm26 range of the resulting B instruction. Wider source admission still needs
stack-probing/runtime work. The existing fixed heap arena, allocation-failure
handling and lack of automatic runtime heap growth remain unresolved. This is
not a native isolation or general memory-safety guarantee.

Small-frame/load/store instructions keep their prior bytes. Seven representative
complete ELF artifacts (constants, recursion, loop, ten arguments, arrays, maps,
and strings) are byte-identical to baseline `8ce9793`, including artifact size.
This establishes emitted-code identity for that corpus, not universal performance
or language/ABI compatibility.

## Large literals and calls

Array/map allocation sizes and outgoing argument allocation/cleanup use the same
expanded adjustment encoder as frames. Base-relative word loads/stores keep
compact encoding through offset 4,095 and use x16 for larger offsets. Offsets are
validated as exact nonnegative word counts before multiplying by eight; wide
access rejects an x16 base, and wide stores also reject an x16 source register.
Existing frame helpers and runtime heap helpers are unchanged.

Arguments are still evaluated once, left to right, into 16-byte staging cells.
The outgoing area is aligned to 16 bytes, disjoint from those cells, and stack
arguments retain their ABI layout. x16 address formation does not overwrite the
x0–x7 argument registers or the x0 return value. Arrays/maps finish evaluating
all nested expressions before establishing their allocation base in x9.

The first formerly rejected cases now compile: call173 (4,096-byte cleanup),
array512 (4,096-byte element area) and map256 (4,096-byte pair area). Counts still
use the existing 16-bit materialization: current strict token admission keeps
literal counts representable. A future local/unbounded ARM API must independently
validate or widen count materialization; this invariant must not be assumed there.

Three additional complete artifacts just below those old boundaries—call172,
array511 and map255—remain byte-identical to `95d6bb1`, alongside the original
seven goldens. This checks compatibility and artifact size for ten cases. No new
compiler-speed, runtime-memory, GPU/NPU or peripheral-performance claim is made.

## Long source branches

Source `if`/`elif`, `while`, `and` and `or` branches retain original word positions
until function layout is complete. A conditional displacement outside signed
imm19 becomes an inverse condition skipping two words followed by an unconditional
B to the original target. The B displacement is relative to its own instruction,
not the preceding condition. This preserves x0, other registers, stack and flags.
The first formerly rejected positive displacement, 262,144 words, now compiles;
262,143 retains its original single instruction.

Every source-relative B is relocated too, including forward if-arm exits and
backward loop edges. Call placeholders and string address loads move with the
function; global function and string-data addresses are computed from final sizes.
Labels at an expanded branch still target its first instruction. Compiler-owned
positions, opcodes and placeholders are validated before patching. Unsupported
imm26 displacement still fails rather than silently wrapping.

The ordinary path patches the original array without a layout copy. Expanded
functions use sorted sites, prefix insertion counts and binary-search mapping,
then move instructions backwards inside the original array. Relaxation batches newly long edges and
converges monotonically without a preset pass limit. Today's forward conditional
intervals are nested, disjoint or touching, so an initially short edge cannot
contain an initially long edge: all required expansions are selected initially.
The convergence loop remains for future syntax; crossing control flow would
require revisiting worst-case iteration cost. No arbitrary language size or
execution-time limit was introduced, and strict source admission is unchanged.

`tests/fixtures/arm64_branch_sources.sapp` supplies deterministic long programs.
The opt-in suite executes both condition outcomes, long short-circuit map
expressions, 24 nested conditions, loopback relocation, forward B across a later
expanded elif, moved global function targets and relocated strings. Ten existing
whole-artifact goldens retain their bytes and size.

[The independent QEMU record](ARM64_BRANCH_QEMU_RESULTS.json) binds the current
emitter and fixture hashes to twelve unchanged generated ELF artifacts. All twelve
passed local QEMU with the same expected exits/output as the PBLang reference.
Initial harness runs had no ELF execute permission and returned exit 1 before
execution; adding owner execute permission and rerunning the same bytes fixed
that harness issue. No compiler change was made for it. The package provenance
and local resource supervision follow the procedure below. These results do not
establish native ARM64, guest-OS support or isolation.

### Current allocation and layout optimization

Slot counting now uses `len(map)`, whose size is maintained by the runtime,
rather than allocating all map keys for each parameter/local. Slot numbers still
follow first occurrence, before compiler metadata is inserted. This removes
quadratic key enumeration for large functions without changing scope semantics.
`_patch` writes the four bytes directly and no longer allocates a temporary array.

Expanded layout validates every final displacement and relocation range before
extending the original code array. It moves instructions backwards using prefix
counts; all four source bytes are captured before any destination byte is written.
Every destination is at or after its old source, so unread instructions survive.
Final length and complete branch consumption are checked. No second full result
array or additional per-branch encoded-word fields are retained. A dynamic array
may still reallocate its backing storage while growing; this is not an allocation-
free compilation claim. The exact-size guard prevents arithmetic corruption and
is not a new discretionary source/runtime limit.

[All 18 measured processes](../bench/results/arm64_layout_2026-09-15.json) compare
`61e6d46` against this emitter, alternating baseline/candidate order across three
pairs per workload on the same allowed CPU. No task-owned test/analyzer ran
concurrently with these measurements; unrelated host load and power policy were
not controlled. No measured run was discarded. The small corpus uses the existing
seven-source benchmark: two warmups, nine samples of 140 compilations. Each large
workload uses one warmup and five single-compilation samples. Process peak RSS
includes startup and warmup; compilation timings exclude source construction.

| Workload / pair | Before median / p95 (ms) | After median / p95 (ms) | Before / after peak RSS (KiB) |
| --- | ---: | ---: | ---: |
| Small corpus / 1 | 319.37 / 430.41 | 417.60 / 428.40 | 38,804 / 38,904 |
| Small corpus / 2 | 410.24 / 424.41 | 414.15 / 483.25 | 38,920 / 38,868 |
| Small corpus / 3 | 409.07 / 480.73 | 396.89 / 405.84 | 38,876 / 38,856 |
| Nested 24 / 1 | 2806.08 / 3089.87 | 2578.56 / 2695.25 | 208,968 / 199,672 |
| Nested 24 / 2 | 2645.70 / 2764.51 | 2533.01 / 3139.23 | 208,284 / 198,928 |
| Nested 24 / 3 | 2684.08 / 3121.90 | 2574.39 / 3079.11 | 208,136 / 198,848 |
| 4,100 locals / 1 | 12928.59 / 13084.65 | 795.20 / 816.33 | 92,480 / 93,924 |
| 4,100 locals / 2 | 13256.75 / 13591.68 | 786.66 / 816.52 | 92,816 / 91,456 |
| 4,100 locals / 3 | 12557.24 / 13033.68 | 795.86 / 819.28 | 92,948 / 93,896 |

The 4,100-local source compiled about 15.8–16.9 times faster. Its peak RSS varied
in both directions, so no memory reduction is established for that workload.
The nested long-branch source used about 4.4%–4.5% less peak compiler RSS and had
about 4.1%–8.1% lower median compilation time. Small-program timings varied,
including a substantially slower first candidate pair; universal acceleration
and strict CPU/RAM non-regression are not established. Generated instructions,
artifact sizes and runtime behavior are preserved for the validated corpus. [The current validation
record](ARM64_LAYOUT_RESULTS.json) verifies identical baseline/candidate bytes for
4,100 locals, 24 nested branches and 1,000 short conditions inside one long branch.
All twelve long ELF fixtures match their previous SHA-256 values and pass current
local QEMU with unchanged expected output/exit codes. These are correctness tests;
the branch-heavy case was not separately benchmarked for performance.

```sh
pblang run bench/arm64_codegen_regression.sapp
pblang run bench/arm64_layout_regression.sapp
pblang run bench/arm64_slots_regression.sapp
pblang run tests/semantics/arm64_slot_identity_test.sapp
```

### Historical compiler cost check — initial branch relaxation


[Six alternating measured processes](../bench/results/arm64_branches_2026-09-15.json)
compare baseline `5860ec8` with the branch emitter using the unchanged
`bench/arm64_codegen_regression.sapp`: seven sources, two warmup batches, nine
samples of 140 compilations, same allowed CPU. All measured processes and their
statistics are retained; artifacts match in every run. Unrelated host load and
power policy were not controlled. These timings concern ordinary small programs,
not the newly supported large branches.

| Pair | Before median / p95 (ms) | After median / p95 (ms) | Before / after peak RSS (KiB) |
| --- | ---: | ---: | ---: |
| 1 | 415.61 / 598.65 | 414.42 / 425.89 | 38,216 / 38,672 |
| 2 | 407.95 / 419.25 | 408.80 / 417.69 | 38,616 / 38,972 |
| 3 | 404.21 / 407.28 | 409.46 / 431.25 | 38,580 / 39,140 |

Median times are similar (about -0.3% to +1.3%); measured peak compiler RSS is
about 0.9%–1.5% higher. This does not prove compiler acceleration or strict resource
non-regression. Small generated programs have unchanged machine instructions.
Each expanded conditional adds four artifact bytes and, when the original branch
is taken, one executed instruction. Large-function layout temporarily holds both
original and expanded byte arrays. No runtime-RAM, GPU/NPU or peripheral speed
claim follows from these compiler measurements.

## Local regression checks

The ordinary semantics tests add exact frame encodings and baseline artifact
fingerprints. The existing ARM64 execution test now shares its test-only
instruction interpreter with extended tests; known architecture byte sequences
check its shifted-immediate decoder independently of the emitter.

```sh
pblang run tests/semantics/arm64_frame_encoding_test.sapp
pblang run tests/semantics/arm64_encoding_stability_test.sapp
pblang run tests/semantics/arm64_wide_encoding_test.sapp
pblang run tests/semantics/codegen_arm64_ast_test.sapp
```

Expensive probes are opt-in, outside the regular semantics directory:

```sh
pblang run tests/extended/arm64_large_frames.sapp
pblang run tests/extended/arm64_large_arrays.sapp
pblang run tests/extended/arm64_large_maps.sapp
pblang run tests/extended/arm64_large_calls.sapp
pblang run tests/extended/arm64_long_branches.sapp
PBLANG_BIN="$PWD/target/release/pblang" pblang run tests/extended/arm64_encoding_rejection_live.sapp --allow-exec
```

The large-frame probe executes a 512-local function and a function with 4,100
locals and nine incoming parameters in the PBLang test reference. The Linux-only
live probe checks the exact positive imm19 endpoint, the next expanded offset,
large literal/argument compilation, exact CLI artifact bytes and preservation of
an existing output when malformed source is rejected.
It requires `/usr/bin/timeout`. A platform skip is not an executed test.
These are trusted local developer tests, not a sandbox for third-party suites.
Require the declared success line as well as exit status 0 when collecting
results. No standalone process runner is introduced: a new runner must verify
actual test completion and use an available, audited bounded process backend.
The existing portable bounded facade currently reports unavailable; it must not
be bypassed or replaced silently with unbounded child execution.

## Historical compiler allocation measurement (frame-encoding slice)

The word emitter appends four bytes directly instead of allocating a temporary
four-element array for every instruction. `_patch` retains its previous helper.
This reduces compiler allocations without adding instructions to compiled code.

```sh
pblang run bench/arm64_codegen_regression.sapp
```

Both versions used the identical benchmark file and retained runtime, seven
sources, two warmup batches, nine timed samples per process and 140 compilations
per sample. Timing excludes artifact fingerprint setup; process CPU and peak RSS
include startup, warmups and fingerprints. `/usr/bin/time` measured process
resources. Baseline is `8ce9793`; measured candidate is `95d6bb1`. The subsequent wide
literal/call extension was not benchmarked. These
are local Linux x86-64 compiler-host measurements, not native ARM64 execution.

All 20 processes are retained in
[the measurement record](../bench/results/arm64_codegen_2026-09-15.json).
Initial runs measured three baseline processes and then three candidate
processes. Subsequent pairs alternated order; the final four pairs pinned both
versions to the same allowed CPU. No other task-owned tests or analyzers ran
concurrently with those benchmark processes; unrelated host load and CPU power
policy were not controlled. No run or sample was discarded.

| Series / pair | Before median / p95 (ms) | After median / p95 (ms) | Before / after peak RSS (KiB) |
| --- | ---: | ---: | ---: |
| initial 1 | 250.77 / 262.09 | 239.20 / 249.79 | 61680 / 38592 |
| initial 2 | 254.13 / 257.63 | 238.05 / 255.28 | 62136 / 38472 |
| initial 3 | 255.72 / 258.21 | 398.99 / 409.55 | 61588 / 38104 |
| paired 1 | 262.24 / 306.94 | 371.38 / 423.08 | 61692 / 38312 |
| paired 2 | 436.77 / 743.69 | 408.61 / 417.47 | 61776 / 38228 |
| paired 3 | 417.36 / 427.82 | 393.21 / 411.47 | 61628 / 38292 |
| same_core_alternating 1 | 262.96 / 430.94 | 442.56 / 471.80 | 62216 / 38512 |
| same_core_alternating 2 | 447.74 / 888.65 | 408.71 / 440.22 | 62184 / 38524 |
| same_core_alternating 3 | 431.42 / 455.39 | 402.09 / 425.89 | 62088 / 38592 |
| same_core_alternating 4 | 410.35 / 421.16 | 449.27 / 636.73 | 62344 / 38200 |

Peak compiler RSS fell consistently from 61,588–62,344 KiB to 38,104–38,592 KiB
(about 38% for this workload). CPU time and elapsed-time results vary substantially,
including slower candidate runs: speed improvement and CPU non-regression are
**not established**. Identical normal artifacts have identical size and machine
instructions; no runtime storage/RAM benchmark was run. No GPU, NPU, peripheral,
guest OS or real ARM64/QEMU performance claim follows from these measurements.

## Architecture references and remaining coverage

- [Arm A64 instruction set](https://documentation-service.arm.com/static/5f3fa899428f7a6b3328fd44)
- [Armv8 instruction-set overview](https://developer.arm.com/-/media/Files/pdf/graphics-and-multimedia/ARMv8_InstructionSetOverview.pdf)

Current evidence includes fixed instruction words, a reference interpreter,
positive imm19 boundary compilation/expansion, local malformed-source rejection and the local QEMU
executions described in the retained records. The negative imm19
endpoint, imm26 endpoints, maximum shifted chunks and multi-chunk adjustments
were reviewed but are not reached by these public-source regressions. Additional
independent boundary decoding, native ARM64 hardware and guest OS execution
remain necessary for broader platform acceptance.

## Historical independent emulator validation — wide literals/calls

[The retained result record](ARM64_WIDE_QEMU_RESULTS.json) binds eight generated
ELFs to the emitter and fixture source hashes. `tests/fixtures/arm64_wide_sources.sapp`
provides the deterministic input generators with `observed=true`:

| Generator | Sizes | Expected process result |
| --- | --- | --- |
| `array_source` | 512, 4095 | exit 42, empty output |
| `map_source` | 256, 2048 | exit 42, empty output |
| `call_source` | 173, 520, 1369, 4105 | exit 42, output `1\n26\n` |

All eight passed both the PBLang reference and local QEMU 8.2.2. The calls verify
nested evaluation order, preserved caller state, large cleanup, staged loads,
outgoing stores and incoming frame access. Literals check first/last values,
nested evaluation and a subsequent allocation that would expose an incorrect
bump pointer. Generated ELF bytes remained unchanged across QEMU execution.

For this run, the exact Ubuntu `qemu-user` package was downloaded as an
unprivileged user and extracted into a temporary directory. The cached Ubuntu
InRelease signature, Packages checksum, package identity and package SHA-256
were checked before execution. No system package or binary-format handler was
installed. The result record includes package and executable digests; it is a
local evidence record, not an authenticated release or sandbox attestation.

QEMU ran only the fixed reviewed programs, from scratch storage with a minimal
environment, file-backed output, 20-second outer wall supervision, 10-second
CPU limit, 2 GiB address-space limit and 1 MiB output-file limit. These are test
harness settings, not language/runtime limits. They do not isolate filesystem
or network access. This is independent user-mode emulation, not a booted guest
operating system or native ARM64 hardware test. No EnterpriseOrg server was used.
