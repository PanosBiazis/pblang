# Self-hosted C99 and compact app facade

Date: 2026-07-14. Host/toolchain details and complete raw output are under
`competitors/`. These measurements include process startup and represent only
the installed toolchains on this machine.

## Delivered

- Complete pblang-written hosted C99 translation units with top-level `main`.
- Speed, size, portable, f64, and explicit integer build profiles.
- Argv-only compiler launch, bounded execution, unique temp paths, and cleanup.
- Compact one-line CLI, GUI, web, and fullstack helpers over the existing secure facade.
- Staged C99, app, polyglot, IR optimization, and competitor benchmarks.

## Verification

- Semantics: **652 passed, 0 failed, 0 ignored, 0 filtered**.
- Native codegen smoke: **254/254 passed**.
- C99 focused gates: ordinary, integer, C-identifier, hosted compile/run, string
  output, explicit integer output, timeout, and cleanup all passed.
- Emulator matrix: 6502, AVR, Z80, MIPS, MSP430, PowerPC, RISC-V, SPARC, WASM,
  and quantum tests all passed in `competitors/pblang-phases/emulators.csv`.

## C99 stages

From `competitors/pblang-phases/selfhost_c99.txt`:

| Metric | Result |
|---|---:|
| hosted transpile average, 100 iterations | 1.104 ms |
| speed compile | 116.104 ms |
| size compile | 90.511 ms |
| portable compile | 76.916 ms |
| speed artifact | 15,992 B |
| size artifact | 14,464 B |
| portable artifact | 16,088 B |
| f64 fib32 average, 5 spawned runs | 6.412 ms |
| integer fib32 average, 5 spawned runs | 7.040 ms |

The five-run averages are startup-sensitive. The corrected common shootout
therefore uses best-of-5 for every language.

## Corrected fib(32) shootout

Source: `competitors/fib-corrected.txt`.

| Target | Best time | Result | Artifact |
|---|---:|---:|---:|
| pblang direct x86-64 | 2 ms | 2,178,309 | 405 B |
| Fortran `-O2` | 4 ms | 2,178,309 | 16,240 B |
| pblang self-host C99 f64 | 10 ms | 2,178,309 | 15,992 B |
| pblang self-host C99 integer | 10 ms | 2,178,309 | 15,992 B |
| C GCC `-O2` | 10 ms | 2,178,309 | 15,992 B |
| C++ G++ `-O2` | 9 ms | 2,178,309 | 16,000 B |
| Rust `-O` | 12 ms | 2,178,309 | 3,954,888 B |
| pblang reference CLI C99 | 32 ms | 2,178,309 | 25,224 B |
| pblang interpreter | 1,434 ms | 2,178,309 | n/a |

The integer profile did not improve this startup-sensitive sample, so no speed
claim is made for it. Its value is explicit integer semantics and a platform for
future integer-specific lowering. Fixed `N` permits specialization and these
numbers do not establish that any language wins every benchmark category.

## Feature/control-plane samples

Pinned sequential samples are stored in `app-facade.txt`, `polyglot.txt`,
`ir-optimizer.txt`, and `ir-stack.txt`. The compact helpers remain close to the
full facade: compact CLI averaged 0.033 ms, compact GUI preview 0.119 ms, and
compact web preview 0.239 ms in the preserved run. Polyglot registry/alias,
guard, policy, and routing gates all passed.

## Remaining limits

The pblang-written hosted C99 backend is a numeric/aggregate subset, not full
dynamic pblang parity. Closures, GC-managed object identity, dynamic imports,
exceptions, and broad stdlib lowering still require the VM or another backend.
The hosted f64 aggregate representation also inherits the existing pointer-as-
number limitation; it is not presented as a universal memory-safe ABI.
