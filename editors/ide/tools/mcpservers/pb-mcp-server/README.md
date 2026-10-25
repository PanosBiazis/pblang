# pb-mcp-server for pblang

This folder is the pblang-native port foundation for the Rust project at
`tools/rust/pb-mcp-server`.

The Rust server is a large systems MCP daemon with hundreds of handlers, async
runtime integration, cross-platform process execution, P2P networking, media
tools, security gates, and gateway binaries. A literal one-shot translation
would be lower quality than a staged port, so this pblang version starts with
the parts that can run safely and correctly in pblang today:

- generated catalog of the Rust `#[tool]` handler surface
- native pblang catalog/status/search APIs
- native pblang safe core file/search/text tools
- native pblang deterministic algorithm tools
- native pblang read-only codebase/find/extract tools
- native pblang bounded workspace scratch tools
- native pblang read-only Git/GitKraken-compatible local introspection tools
- native pblang conversion, JSON data, log-analysis, and SVG/DOT chart tools
- native pblang agent bus, MCP helper, history, consensus, and read-only system
  inspection tools
- native pblang app/workflow/toolchain planning, deterministic review helpers,
  and bounded wait/sleep tools
- native pblang file metadata/stats, shell command intelligence, and
  deterministic ML/neural helper tools
- native pblang deterministic protection/security helpers and tightly gated
  terminal/GitHub/GitLab command probes
- native pblang persisted code-index and manifest tools under `.mint-mcp/`
- native pblang context packing, dry-run debug traces, agent asking, and
  IDE/editor command-plan helpers
- native pblang deterministic AI fallbacks and small quantum educational
  simulations
- path guard and network-profile helpers
- CLI entrypoint
- local HTTP API

## Run

From this folder:

```sh
pblang run main.sapp --allow-all -- status
pblang run main.sapp --allow-all -- tools
pblang run main.sapp --allow-all -- implemented
pblang run main.sapp --allow-all -- search git
```

Call a native pblang tool:

```sh
printf '{"path":"README.md"}' > /tmp/pbmcp-read.json
pblang run main.sapp --allow-all -- call read_file /tmp/pbmcp-read.json
```

Run the local HTTP API:

```sh
pblang run main.sapp --allow-all -- serve 8090
```

Then open:

```text
http://127.0.0.1:8090/
http://127.0.0.1:8090/health
http://127.0.0.1:8090/tools
```

POST `/call` with:

```json
{"tool":"read_file","args":{"path":"README.md"}}
```

## Current Native Handlers

The pblang-native handlers currently implemented are:

- `workspace_context`
- `read_file`
- `read_file_tail`
- `list_files`
- `project_tree`
- `grep_code`
- `text_stats`
- `compress_ws_collapse`
- `compress_simplify_whitespace`
- `summarize_text`
- `format_prefix_lines`
- `json_validate_file`
- `thinking_plan_skeleton`
- `thinking_step_back`
- `thinking_assumption_audit`
- `plan_execution_checklist`
- `plan_handoff_brief`
- `plan_work_buckets`
- `plan_risk_register`
- `plan_definition_of_done`
- `plan_test_strategy`
- `plan_retrospective`
- `reasoning_premortem`
- `reasoning_five_whys`
- `reasoning_chain`
- `reasoning_devils_advocate`
- `reasoning_evidence_matrix`
- most deterministic `compress_*`, `summarize_*`, and `text_*` tools, including
  top terms, duplicate lines, markdown headings/TOC, JSON structure, diff stats,
  redaction, Levenshtein distance, token Jaccard, longest common substring, and
  deterministic text hashing.
- a first native `algo_*` slice: sort/search, graph BFS/DFS/topological sort and
  Dijkstra distances, matrix transpose/multiply/determinant, descriptive stats,
  correlation/regression, n-gram/cosine similarity, deterministic hashing, GCD/LCM,
  Fibonacci, primality/factors, base conversion, combinations/permutations,
  frequency/entropy, moving averages, histograms, percentiles, outliers,
  normalization, rolling stats, label distribution, sieve primes, run-length
  encoding, top-k, set operations, text chunking, and dataset-quality summaries.
- read-only codebase/find/extract tools: `codebase_path_list`, `find_files`,
  `find_files_by_name`, `codebase_largest_files`, `codebase_duplicate_basenames`,
  `codebase_index`, `codebase_word_index`, `codebase_cargo_dependency_names`,
  `codebase_shallow_hash`, `extract_snippets`, `extract_snippets_from_file`, and
  `grep_snippets`.
- workspace scratch tools under `.mint-mcp/agent-temp`: `workspace_temp_write`,
  `workspace_temp_read`, `workspace_temp_list`, and `workspace_temp_delete`.
- read-only Git tools: `git_status`, `git_remote_list`, `git_branches`,
  `git_tag_list`, `git_worktree_list`, `git_describe`, `git_last_commit`,
  `git_log`, `git_reflog`, `git_shortlog`, `git_ls_files`, `git_diff_stat`,
  `git_diff`, `git_show`, `git_rev_parse`, `git_merge_base`, `git_log_or_diff`,
  `git_diff_snippets`, `git_blame`, plus safe `gk_git_status`,
  `gk_git_log_or_diff`, and `gk_git_blame` aliases.
- deterministic conversion/data tools: `convert_format`, `convert_encoding`,
  `convert_binary_hexdump`, `data_json_flatten`, and `db_json_query`.
- log tools: `log_level_summary`, `log_search`, `log_tail`, `log_parse_json`,
  `log_parse_nginx`, `log_parse_syslog`, `log_error_cluster`,
  `log_time_range_filter`, `log_export`, and `log_audit_query`.
- pure output chart tools: `chart_bar`, `chart_histogram`, `chart_line`,
  `chart_scatter`, `chart_timeseries`, `chart_pie`, `chart_gantt`, and
  `chart_dependency_graph`. These return SVG or DOT text directly; they do not
  shell out to renderers.
- agent bus tools: `agent_ping`, `agent_bus_status`, `agent_recent`,
  `agent_get_message`, `agent_bus_search`, `agent_inbox`, `agent_multi_inbox`,
  `agent_thread`, `agent_list_topics`, `agent_roster`, `agent_bus_compact`,
  `agent_coordination_snapshot`, `agent_coordination_playbook`, `agent_send`,
  `agent_handoff`, `agent_subagent_task`, `agent_subagent_done`,
  `agent_fan_out`, `agent_send_pipeline`, `agent_patch_plan`,
  `agent_prompt_pack`, `agent_task_packet`, and `agent_session_summary`.
- MCP helper tools: `mcp_env_hints`, `mcp_tool_spec_generate`,
  `mcp_server_config_generate`, `mcp_prompt_template_validate`,
  `mcp_resource_template_validate`, `mcp_sampling_request_plan`, and
  `mcp_manifest_validate`.
- history/consensus tools: `history_bus_replay`, `history_tool_calls`,
  `history_search_past_calls`, `history_diff_timeline`, `consensus_propose`,
  `consensus_vote`, `consensus_tally`, `consensus_list`, and `consensus_close`.
- read-only system tools: `system_env_get`, `system_process_list`,
  `system_disk_usage`, `system_network_interfaces`, `sys_hardware_info`,
  `sys_cron_list`, `sys_docker_ps`, `sys_docker_logs`, `sys_journal_tail`,
  `sys_k8s_pods`, and `sys_k8s_logs`.
- app/toolchain/workflow helpers: `ai_inference_integration_guide`,
  `coding_agent_toolchains_catalog`, `gitkraken_cloud_catalog`,
  `gitkraken_hosted_tool_redirect`, `app_agent_mode_plan`,
  `app_capability_catalog`, `app_capability_search`, `app_command_palette`,
  `app_command_registry_generate`, `app_context_budget_plan`,
  `app_eval_prompt_pack`, `app_hooks_policy_validate`,
  `app_model_profile_select`, `app_node_suggest`,
  `app_permission_policy_evaluate`, `app_plugin_capability_matrix`,
  `app_plugin_manifest_validate`, `app_plugin_route_plan`,
  `app_skill_manifest_validate`, `app_skill_route_match`,
  `app_skill_template_generate`, `app_slash_command_expand`,
  `app_task_queue_plan`, `app_tool_recipe`, `app_worktree_session_plan`,
  `workflow_template_generate`, `workflow_validate`, and
  `workflow_execution_plan`.
- deterministic review helpers: `review_diff_risk`, `review_focused_review`,
  `review_missing_tests`, `review_pr_packet`, and `review_rulebook_check`.
- bounded wait helpers: `server_sleep`, `wait_until_timestamp`, and
  `wait_for_path`.
- file/repository/sandbox helpers: `file_info`, `file_stats`,
  `batch_file_info`, `repository_get_file_content`, `sandbox_checksum`, and
  `sandbox_diff`. Revision-backed repository reads and non-SHA256 checksums
  remain catalog-only.
- shell command intelligence tools: `shell_command_catalog`,
  `shell_command_safety_check`, `shell_command_translate`, and
  `shell_script_template_generate`. These classify or generate command text;
  they do not execute it.
- deterministic ML/neural helpers: `ml_token_count`, `ml_tokenize`,
  `ml_dataset_validate`, `ml_model_info`, `ml_perplexity_score`,
  `ml_benchmark_model`, `ml_model_compare_benchmark`, `ml_fine_tune_prep`,
  `ml_convert_to_gguf`, `neural_model_info`, `neural_tokenize`,
  `neural_embed_cosine`, `neural_tensor_stats`, and `neural_inference`.
  Runtime-backed benchmark/inference/conversion tools return validated plans
  until pblang has a native model runtime or audited external wrapper.
- project command planners: `project_cargo_check`, `project_cargo_test`, and
  `project_pytest`. These produce bounded argv plans and gate requirements;
  they do not execute cargo/pytest in the pure pblang port.
- deterministic protection helpers: `protect_secret_scan`,
  `protect_file_reputation`, `protect_dependency_audit`,
  `protect_integrity_check`, `protect_yara_scan`, `protect_scan_file`,
  `protect_scan_directory`, `protect_permissions_scan`,
  `protect_world_writable_find`, and `protect_suid_find`. Permission/SUID scans
  report an honest unsupported result until pblang exposes portable file-mode
  metadata.
- terminal/GitHub/GitLab probes: `terminal_which`,
  `terminal_program_version`, `terminal_run`, `terminal_run_stdin`,
  `ollama_list_models`, `gh_pr_list`, `gh_pr_view`, `glab_mr_list`, and
  `glab_mr_show`. `terminal_which` and version probes can execute through the
  existing gate; run/stdin/Ollama entries currently return validated plans.
  These are gated by `MINT_TERMINAL_ENABLE`/`PB_TERMINAL_ENABLE` and
  `MINT_TERMINAL_ALLOW`/`PB_TERMINAL_ALLOW`.
- persisted code-index tools: `code_index_build`, `code_index_status`,
  `code_index_search`, `code_index_file_symbols`, `code_index_paths_prefix`,
  `code_index_path_substring`, `code_index_symbol_prefix`,
  `code_index_paths_without_symbols`, `code_index_extension_histogram`,
  `code_index_symbol_frequency`, `code_index_duplicate_symbols`,
  `code_index_aggregate_stats`, and `codebase_manifest_write`.
- context/debug/agent helpers: `pack_context`, `ask_agents`,
  `debug_trace_run`, `debug_replay_run`, `debug_failure_cluster`, and
  `debug_root_cause_report`.
- IDE/editor helpers: `ide_detect`, `ide_find_references`,
  `ide_goto_definition`, `ide_format_document`, `ide_run_test`,
  `vscode_open_file`, `vscode_open_diff`, `vscode_get_diagnostics`,
  `vscode_list_extensions`, `vscode_install_extension`, `cursor_open_file`,
  `cursor_composer_send`, `zed_open_file`, `zed_context_attach`,
  `zed_search_workspace`, `zed_get_diagnostics`, `jetbrains_open_file`,
  `jetbrains_device_list`, `jetbrains_logcat_tail`, `jetbrains_adb_shell`,
  `jetbrains_lsp_code_action`, and `jetbrains_run_configuration`. External
  editor/device actions return command plans or gated results rather than
  silently executing UI/ADB commands.
- deterministic AI fallbacks: `ai_zero_shot_classify`,
  `ai_few_shot_classify`, `ai_semantic_search`, `ai_cluster_texts`,
  `ai_embed_text`, `ai_structured_extract`, `ai_verify_factual`,
  `ai_rewrite_style`, `ai_self_critique`, `ai_chain_of_thought`,
  `ai_debate`, `ai_multi_model_compare`, `ai_context_pack_smart`, and
  `ai_stack_probe`. These are lexical/rule-based pblang fallbacks and do not
  call Ollama or a provider.
- quantum helpers: `quantum_circuit_simulate`, `quantum_grover_search`, and
  `quantum_shor_factor`. These are small educational simulations/fallbacks, not
  a high-performance quantum runtime.
- web/research/notification planners: `web_dns_lookup`, `web_tls_inspect`,
  `web_search`, `web_api_call`, `web_download_file`, `web_rss_fetch`,
  `web_screenshot_url`, `research_arxiv_search`, `research_arxiv_download`,
  `notify_desktop`, `notify_email`, and `notify_webhook`. These validate
  inputs, build URLs/argv plans, surface public-profile blocking, and mark
  private-host SSRF risk; they do not make network, mail, desktop, or browser
  calls yet.
- media/OCR/speech/screenshot planners: all catalogued `audio_*`, `image_*`,
  `ocr_*`, `screenshot_*`, and `speech_*` tools now have native pblang
  handlers. Metadata/info-style tools return file hints and command plans;
  transform/capture/transcription tools return bounded ffmpeg/ImageMagick/
  Tesseract/speech-engine plans with gate requirements. They do not execute
  external binaries yet.
- network/IoT planners: all catalogued `net_*` and `iot_*` tools now have
  native pblang handlers. They validate hostnames, ports, interfaces, serial
  devices, GPIO/I2C/CAN parameters, and produce command/protocol plans with
  private-host risk markers where useful. They do not scan networks, open
  sockets, write GPIO/serial state, send SMS, or touch hardware directly.
- reverse-engineering helpers: all catalogued `re_*` tools now have native
  pblang handlers. `re_entropy`, `re_strings`, and `re_pattern_search` perform
  local bounded file analysis; the remaining binary-analysis actions return
  bounded `file`/`readelf`/`objdump`/`binwalk`/`xxd`/`strace` style plans.
- sandbox execution planners: `sandbox_docker_run`, `sandbox_eval_code`,
  `sandbox_exec`, `sandbox_file_watch`, `sandbox_network_request`, and
  `sandbox_script` now validate interpreters/paths/programs and return gated
  plans. They do not execute untrusted code in this pure pblang port.
- final catalog-completion handlers: the remaining agent wait/sync tools,
  thinking-session log tools, Ollama plans, advanced `algo_*` helpers,
  `code_*` heuristics, gzip/archive/PDF/QR/document conversion plans,
  CSV/JSONL/timeseries/parquet data helpers, SQLite plans, file crypto/signing
  plans, git write/stash/worktree plans, protection checks, and privileged
  `sys_*` action plans. The generated catalog reports **100% native pblang
  coverage (`629/629` tools)**.
- **Embedded `pbAI` Cognitive & Agentic Suite (`ai_tools.sapp` + `editors/ide/tools/ai/pbai_embedded_engine.sapp`)**:
  `pbai_intent_and_skill_dispatch`, `pbai_embedded_vs_standalone_bridge`,
  `pbai_polyglot_migrate`, `pbai_supply_chain_and_taint_audit`,
  `pbai_brain_planner_verifier` (4-stage verified DAG + SMT contracts),
  `pbai_deq_and_lora_trainer` (Joint Embed-Classify-Retrieve + DEQ Fixed-Point Solver + Continual Online LoRA Feedback),
  `pbai_rag_memory_and_grounding` (3-Tier Memory Store L1/L2/L3 + Hybrid Vector/BM25 RRF + Epistemic Grounding Gate),
  and `pbai_bpe_quant_and_tool_broker` (Von-Neumann BPE Subword Shield + INT8 Quantizer + Zero-IPC MCP Tool Broker).
- **Embedded `pbIDE` Multi-OS & Multi-Device Suite (`ide_tools.sapp`)**:
  `pbide_all_os_device_editions` (`10` OS targets × `8` device classes = `80` combinations),
  `pbide_polyform_14target_designer`, `pbide_multi_arch_matrix` (`8-bit` to `64-bit`),
  and `pbide_self_heal_and_rope_engine`.
- **LLM Cognition & Ecosystem Wave 1..12 Suite (`llm_cognition_tools.sapp` — `51` tools)**:
  Full pure-offline PBLang suite covering `llm_*` cognition/epistemic/topology/BPE/PQC gates
  and `pblang_*` compiler/FFI/GC/LSP/Stage-2/HIL/Cleanroom/PolyForm/Flagship-UI/Professions/Lazy-Idle/Omni-Supremacy/AAP-Fabric/Wave-12 engines.
- **Reverse-Engineering, Decompilation & DOM Scraping Suite (`rea_tools.sapp` — `6` tools)**:
  `rea_binary_audit`, `rea_decompile_cfg`, `rea_application_graph`,
  `rea_protocol_reconstruct`, `rea_dom_scrape_extract`, and `rea_evidence_ledger`.
- **Embedded `pb-mcp-server` Bridge & Live Compiler Suite (`mcp_tools.sapp`)**:
  `pb_mcp_lazy_router_telemetry`, `pb_mcp_sealed_offline_148_bridge`,
  `pb_mcp_rea_and_cleanroom_suite`, and `pb_mcp_pblang_compiler_live_suite`.

## Performance Notes

pblang already has a transparent Cranelift JIT path in `stdlib/jit.sapp`, and `src/router.sapp` uses a **Smart On-Demand Lazy Memoized Router (`_mod(file_name)`)** so tool modules are loaded on demand rather than eagerly at startup. For eligible numeric bytecode functions, run with:

```sh
PBLANG_JIT=1 pblang run main.sapp --allow-all -- status
```

`pblang run main.sapp --allow-all -- status` reports:

- `generated_tool_count: 629`
- `native_catalog_tool_count: 629`
- `catalog_completion_percent: 100`

## Remaining Depth Work

The catalog is now fully covered by native pblang handlers, but not every
handler has Rust-equivalent execution depth. The remaining work is deeper
runtime parity:

1. Replace command-plan handlers with real execution only after pblang has the
   Rust server's command allowlist, argv validation, profile gates, and audit
   logging fully ported.
2. Replace heuristic/fallback algorithms (`algo_fft`, simple MinHash, baseline
   ROC AUC, round-robin k-means) with complete numerical implementations.
3. Add native gzip/archive/PDF/QR/document/SQLite/parquet libraries or audited
   external-binary wrappers.
4. Add true async polling primitives for blocking wait tools.
5. Add stronger binary byte APIs for reverse-engineering tools so pblang can
   analyze arbitrary binary data without treating it as text.
6. Keep the semantic suite as the gate: no generated catalog entry should ever
   regress to `catalog-only`.

## Boundary

This is not a wrapper around the Rust binary. It is pblang code. The generated
catalog is copied from Rust source only to keep the port aligned while modules
are translated one by one.
