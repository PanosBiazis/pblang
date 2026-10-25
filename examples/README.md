# PBLang Runnable Examples & Demos (`examples/`)

This directory contains runnable `.sapp` programs demonstrating every major capability of **PBLang `1.0.0-stable`**, from standalone systems programming and zero-trust security to native GUI apps, AI/ONNX inference, quantum/neuromorphic simulation, and embedded hardware control.

---

## Quick Run

```bash
# Run the unified flagship showcase:
pblang run examples/chat_ideas_omni_showcase.sapp --allow-all

# Run security & taint analysis examples:
pblang run examples/security_analysis_basic.sapp --allow-all
pblang run examples/security_config_redaction.sapp --allow-all
pblang run examples/security_transport_guard.sapp --allow-all
```

---

## Featured Categories

| Category | Key Examples | What It Demonstrates |
| :--- | :--- | :--- |
| **1. Flagship All-in-One Showcase** | `chat_ideas_omni_showcase.sapp`, `historical_4sources_ideas_engine.sapp` | Smart O(1) lazy-idle module loader, Omni-Supremacy benchmark verifier, Flagship UI themes, and strict `[REDACTED]` confidentiality. |
| **2. Security, Privacy & Sandbox** | `security_analysis_basic.sapp`, `security_config_redaction.sapp`, `security_transport_guard.sapp`, `bounded_process/` | Static AST taint analysis, secret redaction, carrier-independent TLS/USB transport guard, and bounded Linux child process supervision (`execve`). |
| **3. Desktop, Web & Mobile UI** | `modern_ui_demo/`, `declarative_browser/`, `crud_website/`, `android_dev_example/` | Single declarative view tree (`stdlib:modern_ui.sapp`) rendered to Desktop `X11`, hardened Web (`A+ CSP`), Android `APK`, and TUI. |
| **4. Native Compilation & Cross-Target** | `demos/arm64/`, `demos/wasm/`, `cross_target_artifacts/`, `compiler_batch/` | Pre-verified ARM64 `ELF64` binaries (`fibseq`, `sort`, `palindrome`, `caesar`), WebAssembly modules (`fib.wasm`), and C99 transpilation. |
| **5. AI, ONNX & Cognitive Agents** | `ai_chat_toolcall/`, `ai_plugin_minimal/`, `ai_turn_budget/`, `pbai_modularity.sapp` | Deterministic ONNX inference, tool-calling agents, turn budgets, and `pbAI` student checkpoint verification. |
| **6. Data, SQL, Cache & Concurrency** | `data_store/`, `cdn_cache/`, `db_owned.sapp`, `async_pool/`, `concurrent_tasks/` | Native SQLite/Redis stores, regional CDN caching, structured async worker pools, and automatic lifetime cleanup. |
| **7. Embedded & Hardware Studio** | `uno_r4_led_studio/`, `embedded_control_center/` | Arduino Uno R4 WiFi LED matrix studio, ESP32-D firmware generation, and cycle-accurate hardware simulation. |
