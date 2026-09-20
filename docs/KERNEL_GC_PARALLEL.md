# Shared kernel memory lock and nonwaiting collection admission

This is a PBLang kernel-library change with atomic x86 instructions emitted by the
existing C backend. It does not install a global collector in the retained VM,
PB-MCP, FFI or polyglot runtimes, and it is not a completed SMP operating system.

## Behavior

The free-list heap and collector now use one shared lock. Every public operation
saves/disables local maskable interrupts, acquires the lock, accesses protected
state, releases the lock, then restores the saved IF bit. Internal `_guarded_*`
helpers require existing lock ownership and must not be called independently.
They do not reacquire it, including automatic collection and heap metadata growth.
Manual outer IRQ masking still works: an incoming disabled state remains disabled.
There are no user callbacks or intentional sleeps inside these critical sections.

The existing backend emits an aligned 32-bit lock word, atomic XCHG acquisition
and release with compiler memory barriers, a read-only state probe and a
`rep; nop` spin hint. After an unsuccessful exchange, waiters poll before trying
another exchange; they do not repeatedly issue locked writes while it remains
owned. This follows the architecture guidance in the
[Intel system programming manual](https://www.intel.com/content/dam/www/public/us/en/documents/manuals/64-ia-32-architectures-software-developer-vol-3a-part-1-manual.pdf).
This is kernel synchronization, not a recommendation to busy-wait in ordinary
user applications. The instructions remain a temporary CPU boundary emitted
from PBLang; no Rust source was added.

`kernel_gc_try_collect(g)` has these results:

| Result | Meaning |
| --- | --- |
| 1 | Lock busy; no waiting, graph traversal, collection-count change or implicit retry |
| 0 | Lock acquired and collection completed |
| -1 | Lock acquired but collection could not complete, preserving existing no-sweep-on-failure behavior |

All paths release an acquired lock and restore the incoming IF bit. A busy
caller chooses when to retry; successful admission still performs the entire
collection. No new object-count or source-size policy was added.

`gc_alloc_rooted` continues to publish its root before unlock. In contrast,
`gc_alloc` followed by `gc_add_root` is not an atomic pair. Values must remain
rooted throughout their use. Access through raw pointers, other collectors'
edges, hidden VM roots or arbitrary `poke32` calls is not validated by the lock.

The collector control block now also comes from the supplied free-list arena.
If control/object/root metadata allocation fails, all earlier allocations from
that construction are returned. The existing control payload remains 48 bytes;
it now consumes a free-list block header as well. Code that manually accounts
for arena capacity must include that header. Successful collector destruction
is still not implemented; callers must not manually free a live collector.

## Verification and reproduction

Use an isolated package with the complete literal import closure; build it and
publish its lock with `tools/pblock.sapp`. Repository lock/source changes from
other work must be preserved. No test needs a network service.

The existing three QEMU fixtures passed on qemu64 and 486 (six cases). They cover
cyclic reachability, growth, generation retry/wrap, actual PIT IRQ0 collection,
root-publication failure rollback, allocator overflow and restoration of IF.
The IRQ fixture additionally checks busy collection admission; the cycles fixture
now makes 10000 failed constructions before reusing the full small arena, plus
1000 failures after control/object metadata succeeded but root-table allocation
failed, verifying complete rollback of both earlier blocks. Use
`transpile_kernel_int` and the compiler/linker/QEMU options documented in
`KERNEL_GC_LIFETIME.md`; prepend interrupts/timer for the IRQ fixture. Exit 85
means success. Bound each run to 25 seconds and stop its owned process on timeout.

For real host-thread interleavings, the checked-in PBLang generator produces a
native test harness from the current heap/collector source and copies the exact
emitted atomic primitives:

```
pblang run tests/tooling/kernel_gc_parallel_generate.sapp -- parallel.c
cc -O2 -pthread -no-pie -fno-pie parallel.c -o parallel
./parallel
```

Run these inside the disposable package. The generated pthread/IF shim is test
infrastructure, not a replacement runtime. It checks low-address static arenas,
uses a 64-bit host `long` ABI with 32-bit stored pointers, and substitutes only
local IRQ save/restore with thread-local state. Four threads exercise two shared
collectors with automatic collection, self-cycles, explicit yields, rooted
publication and nonwaiting collection. A separate probe calls try-collect while
another thread holds the lock; it must finish with busy and unchanged counters.
Three completed runs verify 24000 rooted lifetimes and final empty graphs.
Use a 30-second supervisor bound and check status 0. No generated C or binaries
need to be committed. This does not emulate kernel interrupt privilege or prove
SMP AP startup; the separate 32-bit QEMU fixtures cover the IRQ primitive.

Twenty-one relevant suites, three native lint checks, four kernel diagnostics
checks, build and portable-lock publication passed. Native compiler lint retains
its pre-existing unused-variable warning. The linker still warns about the old
RWX segment layout; page-permission hardening was not silently claimed.

## Measurements and limits

The identical 128-object ring/500000-collection benchmark compares `a1a38cd`
heap+collector to this implementation, including all new locking. Both variants
use the same current emitter and `ALIGN(4096)` before `.bss` to control the known
QEMU layout artifact. Two warmups and five measured runs alternate order.
[Raw samples and flags](evidence/kernel-gc-parallel-2026-09-21.json) show median
QEMU CPU 0.94 / 0.94 seconds, p95 1.42 / 1.37 seconds. Median QEMU peak RSS is
59544 / 59568 KiB; ELF sizes are 11192 / 12280 bytes. No throughput or general RAM
improvement is demonstrated. This benchmark has no competing owner and measures
QEMU process resources, not physical-device power, GPU/NPU or contention scaling.

The global lock serializes independent arenas. Blocking acquisition spins with
interrupts masked, with no fairness or bounded waiting. Collection still masks
interrupts throughout traversal/sweep. Full SMP boot, fairness, per-arena locking,
pause-latency measurements and physical devices remain open; NMI/DMA and arbitrary
native memory access are not covered. A fault or noncooperative owner may block
all users. No general runtime/FFI/PB-MCP integration or hard real-time safety is
claimed. No EnterpriseOrg server was used.

Graphify update was attempted with an owned 35-second budget and timed out.
Its incomplete output is not semantic verification of these PBLang modules.
