# PBLang Runnable Examples & Demos (`examples/`)

This directory contains runnable `.sapp` programs demonstrating the capabilities of **PBLang `2.1.0`**, from Zero-Rust systems programming, `@polyglot` orchestration, capability sandboxing, and post-quantum security to native desktop/web UI apps, ONNX CPU inference, quantum statevector simulation, and `8/16/32/64-bit` embedded hardware targets.

---

## Quick Run

```bash
# Run security & taint analysis examples:
pblang run examples/security_analysis_basic.sapp --allow-all
pblang run examples/security_config_redaction.sapp --allow-all
pblang run examples/security_transport_guard.sapp --allow-all

# Export the Sovereign Studio dashboard HTML:
pblang studio --export-html /tmp/pblang_studio.html
```

---

## Featured Categories

| Category | Key Examples | What It Demonstrates |
| :--- | :--- | :--- |
| **1. Security, Privacy & Sandbox** | `security_analysis_basic.sapp`, `security_config_redaction.sapp`, `security_transport_guard.sapp`, `bounded_process/` | Static AST taint analysis, `[REDACTED]` secret scrubbing, carrier-independent TLS/USB transport guard, and bounded Linux child process supervision (`execve`). |
| **2. Desktop, Web & Mobile UI** | `modern_ui_demo/`, `one_line_apps/`, `declarative_browser/`, `crud_website/`, `android_dev_example/` | Single declarative view tree (`stdlib:modern_ui.sapp`) rendered to Desktop (`X11` / `Win32 GDI`), hardened Web (`A+ CSP`), Android `APK`, and TUI. |
| **3. Native Compilation & Cross-Target** | `demos/arm64/`, `demos/wasm/`, `cross_target_artifacts/`, `compiler_batch/` | Native `x86-64 ELF64`, Windows `PE32+`, `ARM64 ELF64` (`fibseq`, `sort`, `palindrome`, `caesar`), WebAssembly (`fib.wasm`), and hosted `C99` transpilation. |
| **4. AI, ONNX & Agent Budgets** | `ai_chat_toolcall/`, `ai_plugin_minimal/`, `ai_turn_budget/` | Deterministic CPU ONNX inference (`stdlib:onnx.sapp`), tool-calling workflows, and bounded agent turn budgets. |
| **5. Data, SQL, Cache & Concurrency** | `data_store/`, `cdn_cache/`, `db_owned.sapp`, `async_pool/`, `concurrent_tasks/` | Native SQLite/Redis stores, regional CDN caching, structured async worker pools, and deterministic `defer` cleanup. |
| **6. Embedded & Hardware Studio** | `embedded_blink/`, `embedded_circuit/`, `embedded_inspector/`, `uno_r4_led_studio/`, `embedded_control_center/` | ATmega328P AVR8 Intel HEX emission, Arduino Uno R4 WiFi LED matrix studio, and deterministic register-level hardware simulation. |
| **7. Showcase & Catalog Runners** | `chat_ideas_omni_showcase.sapp`, `historical_4sources_ideas_engine.sapp` | Lazy-idle module loading, UI theme showcases, and synthetic architecture catalog inspection. |
