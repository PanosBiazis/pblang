# pblang shared-IR constant propagation and compact increments

This pure-pblang compiler pass carries proven register constants forward across
unrelated instructions, resolves known `ifnz` branches, substitutes known
`addi` results with constants, and then lets structured dead-store elimination
remove obsolete definitions. Unknown branches retain only constants equal on
both exits. Loops are optimized conservatively and invalidate every register
that the body or loop counter may write. Syscalls invalidate `r0`.

The pass is recursively gated by the presence of `ifnz`, so loop-only programs
do not pay for a branch analysis they cannot use. The x86-64 and i386 backends
also select compact `inc` encodings for `addi r, 1`; exact byte tests lock these
encodings to 3 bytes on x86-64 and 1 byte on i386.

## Release measurements

`bench/codegen_ir_optimizer_stages.sapp` was executed in five fresh release
runs. Latencies below are medians. Size metrics are deterministic.

| Metric | Before | After | Change |
|---|---:|---:|---:|
| Non-adjacent branch IR | 5 instructions | 3 instructions | -40.0% |
| x86-64 ELF | 173 B | 153 B | -11.6% |
| i386 ELF | 128 B | 111 B | -13.3% |
| ARM64 ELF | 172 B | 152 B | -11.6% |
| Portable C99 source | 232 B | 167 B | -28.0% |
| Optimizer latency | - | 0.0927 ms | median |
| x86-64 codegen | - | 0.2203 ms | median |
| i386 codegen | - | 0.1790 ms | median |
| ARM64 codegen | - | 0.2098 ms | median |

The broader structured kernel reaches 6 to 3 IR instructions and reduces its
x86-64/i386/ARM64 artifacts by 15.5%, 17.2%, and 15.6%. Its portable C99 output
falls from 256 to 167 bytes. These are scoped compiler-kernel results, not a
claim that pblang already wins every workload or every language benchmark.
