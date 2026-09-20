# Two-vCPU kernel GC validation and interruptible lock waiting

The shared kernel memory lock now restores the caller's original interrupt state
while waiting after a failed acquisition, and disables interrupts again before
the next atomic attempt. Protected memory access still starts only after lock
acquisition. Originally masked/ISR callers stay masked. Originally interruptible
callers can service local interrupts while a different CPU owns the memory lock.
The polling path remains active spinning; fairness and a bounded waiting time
are not provided. The wait helper is separate from the uncontended acquisition
path, without changing public API or object/arena size policies.

## Actual kernel execution

`examples/kernel_gc_smp.sapp` and its PBLang generator
`tests/tooling/kernel_gc_smp_generate.sapp` form a **QEMU-only test fixture**.
The generator writes a kernel C translation plus a small x86 bootstrap test shim;
it does not run that code or provide a general-purpose application-processor API.
Heap/GC algorithms are emitted from the current PBLang library source. No Rust
source or external project code was added.

The shim starts APIC ID 1 with targeted INIT/SIPI messages, enters protected mode
from a real-mode trampoline and uses a separate 32 KiB AP stack. BSP ID 0 and AP
ID 1 are both checked before accepting results. The fixture owns low memory
0x7000..0x8fff, a private flat GDT for the AP, PIT channel 2 for startup delays,
PIT IRQ0 and the BSP's test IDT. It assumes the PC machine and the named QEMU CPU
models, paging disabled, xAPIC with a 32-bit physical base, and exactly two CPUs.
It is not suitable for direct execution on the laptop or a running OS. CPUID
availability is a fixture precondition; unsupported old physical hardware is not
covered by this SMP boot test. Existing 486 single-vCPU tests remain separate.

The startup sequence follows the
[Intel multiprocessor initialization specification](https://cdrdv2-public.intel.com/868137/325462-089-sdm-vol-1-2abcd-3abcd-4.pdf).
PIT delays round upward using integer arithmetic with a conservatively higher
clock rate; invalid delay arguments are rejected before I/O. Wait loops and each
external test run have finite supervisor bounds. These bounds are test guards,
not a hard real-time guarantee for a program using the collector.

The completed fixture verifies:

- Both CPUs execute 2000 rooted object lifetimes against one shared collector,
  with automatic collection, self-cycles and nonwaiting collection attempts.
- An AP try-collect completes with busy while the BSP holds the lock, without
  changing collection counters.
- The BSP services at least three PIT interrupts while waiting for the AP to
  release the lock. AP release depends on that progress, so a silently blocked
  interrupt path cannot pass.
- An already-masked BSP remains masked during a second contended wait; timer
  count is unchanged while the AP holds the lock for a timed interval.
- Both completion counters reach 2000, all roots are removed and the final
  collection leaves no live objects.

A caller must still keep its object rooted throughout use, use only valid owned
pointers and avoid calling private locked helpers directly. Interrupt handlers
can now run during an interruptible wait, but an ISR that itself makes a blocking
memory call can still wait on a different CPU. This is not arbitrary ISR safety.
The lock owner still masks interrupts throughout collection, including traversal
and sweep. NMI/fault reentry, DMA and foreign native writes remain outside the
contract. A fault or noncooperative lock owner can prevent progress.

## Reproduce and evidence

Use an isolated package containing the complete literal imports of the generator,
heap, collector, interrupts and timer. Build and publish the package's lock with
`tools/pblock.sapp`. The emitted linker script comes from
`compiler/ctranspile.kernel_linker_script()`.

```
pblang run tests/tooling/kernel_gc_smp_generate.sapp -- smp.c
cc -m32 -O2 -march=i386 -fno-toplevel-reorder -ffreestanding -nostdlib \
  -fno-pie -mgeneral-regs-only -c smp.c -o smp.o
ld -m elf_i386 -T kernel.ld -o smp.elf smp.o
qemu-system-x86_64 -cpu qemu64 -smp 2 -machine pc -accel tcg,thread=multi \
  -m 32M -kernel smp.elf -display none -serial none -monitor none -nic none \
  -no-reboot -device isa-debug-exit,iobase=0xf4,iosize=0x04
```

Use a 25-second supervisor timeout and terminate the owned QEMU process on
expiry. Exit 85 is success. Other exits and timeouts must fail the test, not be
counted as successful boot. Repeat with `qemu-system-i386 -cpu pentium3` and the
same remaining options. No disk image, network service or host root rights are
needed; do not execute the emitted kernel as a host executable.

[Regression evidence](evidence/kernel-gc-smp-2026-09-21.json) records three passing
runs for each two-vCPU CPU model. The same final fixture with the `1902a69` heap
fails with exit 57 on both models: the AP cannot observe three BSP timer ticks
while holding the lock. This is a direct before/after behavioral regression,
not an inference from a source-pattern check. A single-vCPU negative run must
reject the missing AP handshake rather than report SMP success.

Twenty-one related suites, six existing single-vCPU QEMU cases on qemu64/486,
three native four-thread runs (24000 rooted lifetimes), generator lint, kernel
diagnostics and build/portable-lock checks passed. Diagnostics were also checked
against the committed diagnostics module, preserving unrelated local edits.
These checks do not replace application acceptance across all operating systems.
Some existing kernel images still produce the retained linker's RWX warning;
this change does not establish page-permission hardening.

For the subsequent collection-time optimization and its current measurements,
see [all-live sweep elimination](KERNEL_GC_LIVE_SWEEP.md). The measurements below
describe the earlier interruptible-wait change.

## Performance and remaining integration

The benchmark uses the unchanged 128-object ring with both 50000 literals scaled
to 500000 for both variants. Both builds use the same current emitter and
`ALIGN(4096)` before `.bss`, preserving the previous control for QEMU code/data
placement. Two warmups and five measured samples per variant alternate order.
[Final samples](evidence/kernel-gc-wait-2026-09-21.json) show median QEMU CPU
1.43 / 1.42 s, p95 1.45 / 1.70 s, median process peak RSS 59204 / 59336 KiB, and
ELF sizes 12284 / 12016 bytes. These noisy uncontended process measurements do
not establish a general speed or RAM improvement or a contention/latency bound.
The compiler still emits a helper call in the tested collection entry; separating
source paths did not prove elimination of that overhead.

The initial implementation's [samples](evidence/kernel-gc-wait-initial-2026-09-21.json)
are retained: median CPU 1.05 / 1.42 s, with substantial run variation. They must
not be hidden or directly compared with the final run as a controlled speedup.
The meaningful latency evidence is continued IRQ progress under contention;
maximum, p95 or p99 interrupt delay in physical time has not been measured.

This closes the missing two-vCPU collector test, not complete SMP OS support.
Topology discovery, more CPUs, AP interrupt/scheduling setup, hotplug, fairness,
per-arena concurrency and physical-device acceptance remain open. General
runtime/FFI/PB-MCP/polyglot integration also remains open: `stdlib/runtime_gc.sapp`
still refuses destructive retained-host collection because roots across nested
VMs are incomplete. A 32-bit kernel object graph cannot substitute for those
host ownership/root contracts. No runtime refusal was bypassed, no installed
PB-MCP transport was changed and no EnterpriseOrg server was used.

Graphify update timed out at its 35-second budget; its owned process group was
stopped. Graph output is not semantic verification of these PBLang modules.
