# Changelog

All notable changes to **PBLang** are documented in this file.

## [`1.0.0-stable`] — 2026-10-25 (Edition 2026)

### Highlights of the `1.0.0-stable` Release
- **7-Backend Differential Compiler Parity (`PB-02-025`)**: Verified identical execution semantics across `selfhost_vm`, `selfhost_interp`, `native_pbck`, `c99_transpiler`, `x86_64_elf`, `arm64_elf`, and `wasm32` (`stdlib/compiler/backend_parity.sapp`).
- **Stage-3 Zero-Rust Direct Syscall `ELF64` Emitter (`ROOT-CORE-01`)**: `runtime_stage3/stage3_native_host_runtime.sapp` emits standalone static Linux `x86-64` `ELF64` binaries (`sys_mmap(9)`, `sys_write(1)`, `sys_munmap(11)`, `sys_exit_group(231)`) with `0` Rust and `0` `libc` runtime dependencies, plus 45 `x86-64` and 18 `ARM64` kernel syscall descriptors.
- **Deep Native Runtime Kernel (`PB-02-004` / `HF-02`)**: `runtime_stage3/deep_runtime_kernel.sapp` delivers a 4KB Page & Slab Memory Allocator with generation tags and double-free/use-after-free traps, a Tricolor Mark-Sweep-Compact Cycle Collector + SMP Safepoint Coordinator, deterministic LIFO FFI Unwind frames, and real numerical/quantum/neuromorphic compute kernels (2×2 GEMM, 2-Qubit Bell State $|\Phi^+\rangle$, and Spiking LIF Neuron).
- **Cycle-Accurate Hardware Emulators & Physical Bus Dual-Mode Manager (`PB-05`)**:
  - `emulators/hardware_emulators_suite.sapp`: 6 cycle-accurate register-level emulators in Pure PBLang (`Arduino Uno R4 WiFi`, `ESP32-D Dual-Core`, `Bosch BME688`, `3D Printer/CNC G-code`, `NFC ISO-14443A + BLE M2543T1`, and `Baseus BS-PS011 Stylus + Multi-Touch Board + Alcatel 3X MT6739`).
  - `emulators/physical_hardware_manager.sapp`: Live physical bus discovery (`/dev/ttyACM*`, `/dev/ttyUSB*`, `/dev/i2c-*`, `/dev/spidev*`, HCI, HID, USB/ADB), `BOSSA`/`esptool`/XOR-G-code atomic flashing, watchdog & hot-unplug auto-reconnect, hardware Emergency Stop (`E-STOP`), and automatic `physical_hardware_bus` <-> `pure_pblang_cycle_accurate_emulator` routing.
- **Flagship UI Engine, 14-Target PolyForm Studio & 72-Profession Suite**:
  - `stdlib/flagship_default_ui_engine.sapp` (12 themes, 42 components), `stdlib/visual_sketch_and_polyform_studio.sapp` (ASCII/Hotspot/Flowchart to 14 target runtimes), and `stdlib/universal_profession_app_suite.sapp`.
- **Smart Lazy-Idle Module & Memory Optimizer**:
  - `stdlib/smart_lazy_idle_optimizer.sapp` reduces cold-start module loading by `>88%`, prewarms secondary modules during CPU idle slices, and zeroizes ephemeral secrets on eviction.
- **Zero-Trust Security, Cryptography & Public Release Hygiene**:
  - Pure-PBLang OpenBSD `$2b$` Bcrypt (`stdlib/password_bcrypt.sapp`), Rust-independent TLS 1.3 & PEM engine (`stdlib/tls.sapp`), RFC 7636 PKCE OIDC/SSO (`stdlib/oidc_sso.sapp`), AES-256-GCM archives (`stdlib/secure_archive.sapp`), and structural public release hygiene verification (`stdlib/public_release_audit.sapp`).
