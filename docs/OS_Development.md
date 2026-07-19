# OS development with pblang

> **Status: GOAL / DESIGN (2026-05-30).** Making pblang capable of **operating-system
> and bare-metal development** — kernels, bootable images, drivers — is an explicit
> project goal. Some of the *API surface* exists today (`stdlib/systems.sapp`), but it
> is currently a **modeling sketch**, not real machine codegen: e.g. `asm_hlt()`
> returns `map{desc: "asm hlt"}` and `port_out_byte(p, v)` returns `map{p, v}` rather
> than emitting a `hlt`/`out` instruction. This document is the honest roadmap from
> that sketch to a real freestanding toolchain. Items are marked **(today)**,
> **(sketch)**, or **(planned)**.

---

## 1. The goal

pblang should be usable **all the way down** — not just as a scripting/app language on
top of an OS, but as a language you can write an OS *in*. The end state: compile a
pblang program to a **freestanding, bare-metal binary** (no host OS, no libc), boot it,
and have it talk to hardware directly — and, ultimately, compile that OS **with the
pblang compiler itself** (see [§7](#7-relationship-to-self-hosting)).

This is the natural top of pblang's ambition ladder: it already self-hosts its compiler
(`docs/CURRENT_STATUS.md`) and ships multiple codegen backends; OS-dev is the frontier
where those meet real hardware.

---

## 2. What exists today

| Piece | State | Notes |
|-------|-------|-------|
| `stdlib/systems.sapp` (28 fns) | **sketch** | The intended OS-primitive API: `asm_hlt/nop/cli/sti`, `port_out_byte`/`port_in_byte`, `mmio_read32`/`mmio_write32`, `ptr_null`/`ptr_is_null`, `interrupt_disable/enable`, `page_table_entry`, `gdt_kernel_code`/`gdt_kernel_data`, `multiboot_header`, `detect_cpu_features`, `detect_memory_map`, plus byte/buffer/pack helpers. **They model the shapes; instruction lowering is a separate backend contract.** |
| Byte/buffer primitives | **today** | `buffer_new`, `buffer_from/to_string`, `buffer_to/from_hex`, `pack_u`/`unpack_u`, `str_to_bytes`/`bytes_to_str` — real, and the basis for driver/struct (de)serialization. |
| `pblang-ctranspiler` (AST → C99) | **today (hosted)** | Emits C with a runtime header (`pblang_rt.h`). Today it targets a hosted runtime; a **freestanding** mode is the key enabler for OS-dev (see [§4](#4-the-compilation-path)). |
| `pblang-llvm` (LLVM IR) | **today (prototype)** | Text LLVM IR → clang. A second viable freestanding route. |
| `pblang-cranelift` (JIT/AOT) | **today** | Native object emission; relevant for AOT, less so for freestanding kernels initially. |
| FFI (`pblang-ffi`) | **today** | C ABI calls — useful for linking against existing freestanding C (bootloader shims, asm routines). |

**Takeaway:** the *vocabulary* of OS-dev is sketched in `systems.sapp`, and pblang
already lowers to C and LLVM. The missing pieces are a **freestanding codegen mode**, a
**kernel-grade runtime**, and **real low-level intrinsics**.

### Checking kernel code with `pblang check`

Kernel modules call intrinsics (`outb`, `kmalloc`, `poke32`, …) that only exist in the
ctranspiler's freestanding preamble — not in the VM — and several modules are linked
into one translation unit (`gc.sapp` uses `heap.sapp`'s `heap_alloc`). Two source
pragmas keep the checker sound for them (see `stdlib/diagnostics.sapp`):

```
// pblang:target=kernel          -> the ctranspile intrinsics are defined names
// pblang:extern heap_alloc ...  -> declared cross-unit symbols are defined
```

Typos still flag (`kmaloc` stays an error), and files WITHOUT the pragma keep the
strict rules — `pblang check stdlib/kernel/*.sapp` runs clean today.

---

## 3. The three things that must become real

1. **Freestanding output** — produce a binary with **no host OS / no libc / no
   syscalls**: `no_std`-style, custom entry point, custom linker script, `panic = halt`.
2. **A kernel runtime** — pblang's normal runtime assumes a heap + RC/cycle GC + host
   I/O. A kernel build needs a **minimal runtime**: a tiny bump/freelist allocator (or
   a no-alloc subset), static data only where possible, and no host dependencies. Decide
   per build what language features are available in "kernel mode" (likely: no threads,
   no host `fs`/`net`, GC optional/replaced).
3. **Real intrinsics** — `asm_*`, `port_*`, `mmio_*`, interrupts, control registers must
   **lower to actual instructions** (inline asm in the C/LLVM backend), replacing the
   current `systems.sapp` sketch layer. This is where `systems.sapp` graduates from sketch to
   contract.

---

## 4. The compilation path (planned)

The lowest-friction route to real bare-metal code reuses what already works:

```
  app.sapp ──▶ pblang frontend (self-hosted) ──▶ IR
                                                  │
                          ┌───────────────────────┴───────────────────────┐
                          ▼ (primary)                                      ▼ (alt)
            pblang-ctranspiler --freestanding                   pblang-llvm --freestanding
            → C99 (no libc) + intrinsics as inline asm          → LLVM IR (target triple
                          │                                       x86_64-unknown-none)
                          ▼                                                 │
            cross-compile (gcc/clang -ffreestanding -nostdlib)             ▼
            + linker script + boot stub  ───────────────────────▶  kernel.elf → bootable image
```

- **Primary route — C transpiler in freestanding mode.** Add a `--freestanding` (no-libc,
  no-GC, custom `_start`) mode to `pblang-ctranspiler`, lower `systems.sapp` intrinsics to
  `__asm__` blocks / `out`/`in` / `volatile` MMIO, and link with a small boot stub +
  linker script. C is the pragmatic substrate: every OSdev toolchain already supports
  `-ffreestanding -nostdlib`.
- **Alternate route — LLVM backend** with a `*-none` target triple for projects that
  prefer a direct IR path.
- **Boot** via Multiboot2 (GRUB) or Limine; `multiboot_header()` already sketches the
  header shape.

---

## 5. Milestones (planned)

A conventional OSdev ladder, pblang-flavored:

| Phase | Milestone |
|-------|-----------|
| **K0 — Freestanding hello** | `pblang build --target=freestanding` produces a `.elf` that boots under QEMU and writes "hello from pblang" to the **serial port / VGA text buffer** via real `port_out`/`mmio_write`. |
| **K1 — Real intrinsics** | `systems.sapp` `asm_*`/`port_*`/`mmio_*` lower to actual instructions; add a tiny no-GC runtime + panic=halt. |
| **K2 — CPU tables** | GDT + IDT set up from pblang; handle a first exception/interrupt. |
| **K3 — Interrupts & timer** | PIC/APIC remap, timer tick, keyboard input. |
| **K4 — Memory** | Parse the multiboot memory map, a physical frame allocator, paging, a kernel heap allocator (so GC-free `map`/array work in kernel mode). |
| **K5 — Drivers & tasks** | Serial/VGA/keyboard drivers, a cooperative scheduler, then a minimal shell. |
| **K6 — Self-hosted kernel build** | Build the kernel using the **pblang-written** compiler end-to-end. |

Each phase is gated the pblang way: locked by tests (QEMU smoke tests + a new
`tests/os_*` suite) and verified green before moving on.

---

## 6. Honesty about scope

OS-dev is the **hardest** item on the plan and the furthest from complete:

- The current `systems.sapp` functions are **contracts that model the API** — they let you
  write OS-shaped pblang and reason about it, but running them does nothing to hardware.
- There is **no freestanding codegen yet**; that backend mode is the gating work.
- The kernel runtime (allocator, no-GC subset, panic handler) does not exist yet.

Documenting this here keeps the project honest: the **design and API surface are real
and intentional**, the **codegen and runtime are the work**. Treat K0 (freestanding
serial "hello") as the first concrete proof point.

---

## 7. Relationship to self-hosting

OS-dev and self-hosting reinforce each other:

- pblang already **self-hosts its compiler** (`stdlib/compiler/`), so the toolchain that
  must learn freestanding codegen is itself pblang — improvements compound.
- The long-term vision is a **pblang OS compiled by the pblang compiler** — the same
  "all the way down" property the self-hosting milestones established for the language,
  extended to the bare metal beneath it.
- A freestanding pblang runtime is also the natural substrate for embedded targets and,
  eventually, for running pblang (and tools like **pblang Studio**, see
  `docs/Visual_Designer.md`) on a pblang-built base.

---

## 8. Relationship to existing pieces

- **`stdlib/systems.sapp`** — the OS-primitive API surface (today a sketch; the K1
  contract).
- **`pblang-ctranspiler` / `pblang-llvm`** — the codegen backends to extend with
  freestanding modes.
- **`pblang-ffi`** — linking against freestanding C/asm boot stubs.
- **`stdlib/compiler/` (self-hosted)** — the frontend that feeds both backends.
- **`docs/CURRENT_STATUS.md` / `docs/Future_plans.md`** — where OS-dev phases (K0…K6)
  will be tracked as they ship.

---

*OS development is pblang's deepest target: from a scripting language down to a language
you can boot. The API is sketched, the backends exist — the freestanding codegen and the
kernel runtime are the road ahead.*
