# PBLang Stage-3 Zero-Rust Native Host Runtime & Deep Kernel (`runtime_stage3/`)

This directory contains the **Stage-3 Zero-Rust Direct-Syscall Native Runtime** and **Deep Runtime Kernel** implemented in 100% Pure PBLang. It emits standalone Linux `x86-64` and `ARM64` `ELF64` binaries that invoke Linux kernel syscalls directly (`sys_mmap`, `sys_write`, `sys_munmap`, `sys_exit_group`) with **zero Rust and zero `libc` dependencies**.

---

## Quick Run

```bash
pblang run tools/pblang_stage3_selfhost_driver.sapp --allow-all
```

---

## Modules

| File | Description |
| :--- | :--- |
| [stage3_native_host_runtime.sapp](stage3_native_host_runtime.sapp) | Direct Linux `x86-64` (`syscall` `0x0F 0x05`) and `ARM64` (`svc #0`) `ELF64` emitter with `sys_mmap` arena initialization and `sys_write` output. |
| [deep_runtime_kernel.sapp](deep_runtime_kernel.sapp) | 4KB Page/Slab Arena with generation tags (`Zero-UAF`), Tricolor Mark-Sweep-Compact Cycle GC + SMP Safepoints, LIFO FFI Unwind, 2-Qubit Bell State $\|\Phi^+\rangle$ quantum register, and Leaky Integrate-and-Fire (`LIF`) spiking neural kernel. |
