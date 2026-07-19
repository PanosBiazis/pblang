# Self-hosted VM direct builtin descriptors

The pure-pblang bytecode VM previously allocated a fresh `{__bi: name}` map
every time `OP_GET_GLOBAL` resolved a builtin, including direct calls that
immediately consumed the descriptor. Codegen now attaches an optional private
descriptor to direct calls whose names are in the shared pure-pblang VM builtin
registry. The opcode and operand are unchanged.

## Correctness and compatibility

Runtime globals are checked before metadata, so user functions still shadow
builtins. Ordinary first-class lookups still allocate independent mutable maps.
The descriptor is selected before argument evaluation, preserving callee-first
bytecode semantics if an argument subsequently installs a global shadow. The
safe optimizer preserves metadata in top-level and nested chunks. Rust VM and
PBCK consumers can ignore the additional map field.

## Method

`bench/selfhost_vm_direct_builtin_stages.sapp` compiles two copies of each
10,000-call fixture and recursively removes only the optional metadata from the
fallback copy. Every process warms both copies and runs six alternating pairs,
reversing order each pair. Seven processes were pinned to logical CPU 3. The
fallback/direct elapsed ratio controls for severe frequency drift visible in
the raw totals. Separate fresh processes measured peak RSS with `/usr/bin/time`;
their elapsed values are retained but are not compared across the thermally
different baseline/final batches.

| Metric | Baseline/fallback | Direct metadata | Change |
|---|---:|---:|---:|
| `len` throughput | 1.0000x | 1.0419x | **4.19% higher** |
| `bit_shr` throughput | 1.0000x | 1.0444x | **4.44% higher** |
| Median process peak RSS | 22,612 KiB | 13,500 KiB | **40.30% lower** |

These results cover direct builtin calls in the pure-pblang bytecode VM. They
are not a universal pblang or cross-language performance claim.
