# Interpreter call paths and local competitors — 2026-09-08

Baseline `ee97db2` plus this commit, on the existing dirty development checkout.
No release, universal-language or platform ranking is claimed.

## PBLang-written VM: before and after

`bench/selfhost_call_arity_stages.sapp` compiles each inline fixture once through
the PBLang tokenizer/parser/codegen, without optimizer. Each execution performs
200 identical iterations. Six workloads (closure arities 0–3, builtin control,
arithmetic control) run both bounded and unbounded. Two warmups and seven retained
samples per row; all output checksums and bounded instruction counts are checked.
Bounded VM options: 20,000 steps and 16 frames. Parsing/compilation is outside
the timed interval; VM initialization and result validation are inside it.

The PBLang VM is itself run by the existing PBLang 0.6.0 retained-host interpreter.
This is neither the CLI interpreter's own dispatch benchmark nor a native-compiled
self-host VM measurement. No Rust implementation source changed.

Four sequential batches are retained, without discarding samples: original VM,
initial candidate, simplified classifier candidate, final typed-array candidate.
The workload was identical throughout. The final candidate initializes an empty
array and replaces it for nonzero small arities; this additional bounded allocation
is a remaining optimization opportunity, not an allocation-free claim.
An attempted if-expression alternative failed lint and was not retained or timed.

| Workload | Bounded | Before median ms | Retained median ms | Retained p95 ms | Median change |
| --- | --- | ---: | ---: | ---: | ---: |
| closure0 | false | 5.578 | 5.645 | 6.019 | 1.2% |
| closure0 | true | 12.229 | 12.316 | 13.321 | 0.7% |
| closure1 | false | 6.708 | 6.861 | 7.434 | 2.3% |
| closure1 | true | 14.259 | 13.951 | 16.062 | -2.2% |
| closure2 | false | 6.588 | 6.919 | 7.283 | 5.0% |
| closure2 | true | 15.067 | 15.227 | 15.799 | 1.1% |
| closure3 | false | 8.458 | 7.906 | 9.040 | -6.5% |
| closure3 | true | 19.611 | 19.788 | 19.975 | 0.9% |
| builtin1_control | false | 6.147 | 5.774 | 7.003 | -6.1% |
| builtin1_control | true | 13.194 | 14.562 | 16.886 | 10.4% |
| arithmetic_control | false | 4.534 | 4.227 | 4.872 | -6.8% |
| arithmetic_control | true | 11.695 | 9.815 | 11.459 | -16.1% |

Positive change means slower. Results are mixed: the three-argument unbounded
case is faster in this observation, while other call cases regress or move little.
The arithmetic control (whose bytecode has no call opcode) also moves substantially.
This is insufficient evidence of a general speedup; safety fixes and semantic
coverage are the established improvements. Do not advertise a performance win.

Raw samples, all four phases, stats and whole-process wall/RSS metrics:
`bench/results/selfhost-calls-2026-09-08.jsonl`. Whole-process metrics include
compilation, warmups and every workload and cannot establish per-call allocation
or memory improvement. Statistical labels from the helper do not override the
uncontrolled between-batch drift. Batches were not randomized/interleaved.
Seven-sample interpolated p95 is not production tail-latency evidence.

Run from repository root:
```bash
ulimit -c 0
ulimit -v 2097152
ulimit -f 1024
timeout --kill-after=2s 40s /usr/bin/time -f 'PROCESS_METRIC|%e|%M' target/release/pblang run bench/selfhost_call_arity_stages.sapp --allow-all
```

## Separate competitor check

Linux 7.0.0-31-generic x86-64, Intel i7-1195G7, eight logical CPUs;
PBLang 0.6.0 retained-host executable, GCC/G++ 13.3.0, Go 1.22.2,
Rust 1.94.0, Python 3.12.3, Node 18.19.1. Tool versions were checked locally.
No tools installed; no other device accessed. No tests, Graphify or other
benchmark rows were launched concurrently with measured runs. OS background
activity, frequency, temperature and caches were not controlled.

Nine sequential existing `integer_kernel` rows: 1,000,000 modulo-recurrence
iterations, exact checksum 474518 on every run. One first build plus three repeat
builds for compiled rows, one first run, one warmup, five retained runs.
Flags and fixture unchanged: C/C99/C++ -O3/-DNDEBUG/stripped (C99 gnu99/-lm,
C++17); Rust opt-level=3/debuginfo=0/strip=symbols; Go trimpath/ldflags=-s -w.
PBLang C99/direct-x64 use their existing PBLang drivers.

| Execution path | Run median ms | Run p95 ms | Peak RSS median KiB | Artifact bytes | Repeat-build median ms |
| --- | ---: | ---: | ---: | ---: | ---: |
| pblang-interpreter | 263.897 | 267.901 | 8736 | N/A | N/A |
| pblang-c99 | 8.499 | 8.973 | 1572 | 14472 | 1427.275 |
| pblang-codegen-x64 | 3.795 | 4.109 | 264 | 507 | 890.876 |
| c | 8.015 | 8.950 | 1572 | 14472 | 53.558 |
| cpp | 7.687 | 8.215 | 3372 | 14472 | 363.053 |
| go | 8.824 | 9.422 | 1500 | 1228952 | 37.718 |
| rust | 7.681 | 7.866 | 1932 | 350560 | 75.536 |
| python | 203.861 | 220.097 | 10264 | N/A | N/A |
| javascript | 129.450 | 135.144 | 51340 | N/A | N/A |

All nine rows passed. Raw first values, all retained samples, CPU/RSS/build stats:
`bench/results/session-2026-09-08-vm-calls.jsonl`.
The retained-host PBLang interpreter remains slower than Python and Node here.
Node may use JIT compilation; this is an implementation comparison, not a
classification of all three as pure interpreters. This numeric fixture does not
load the modified self-host VM, so its timings cannot measure this fix.

Process startup/wrappers are included. Peak RSS is not total allocated memory;
artifact size excludes runtimes/libraries/packaging and N/A does not mean zero.
Direct ELF and libc binaries have different dependencies. Cached repeat builds,
especially Go, are not clean builds. CPU timer quantization can produce zero.
Raw RSS/CPU summary fields named *_ms actually represent KiB/percent.

Run each language id above sequentially:
```bash
ulimit -c 0
ulimit -v 2097152
ulimit -f 65536
PB_BENCH_LANGUAGE=python PB_BENCH_WORKLOAD=integer_kernel timeout --kill-after=2s 60s target/release/pblang run bench/session_compare.sapp --allow-all
```

Each child has a ten-second timeout. External process limits are not aggregate
containment or a hostile-code sandbox. Other applications, language implementations,
native targets, physical devices and embedded multi-language polyglot combinations
remain unmeasured. See `BENCHMARK_COVERAGE_PROGRAM.md`.
