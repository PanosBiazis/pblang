//! AI / ONNX builtin implementations.
//!
//! Free functions backing the `ai_*` and `onnx_*` builtins registered in
//! `VM::builtins()` (vm.rs):
//!
//!   - `ai_chat(prompt[, options_json])` — chat completion via the
//!     `pblang_ai::ProviderRegistry` (OpenAI / Anthropic / Ollama / etc.;
//!     auto-registered from env vars).
//!   - `ai_embed(text[, options_json])` — embedding vector via the same
//!     registry.
//!   - `ai_token_count(text)` — heuristic token estimate (no network).
//!   - `onnx_infer(model_path[, input[, options]])` — Python bridge that
//!     loads `onnxruntime` in a subprocess and runs the model. Gated on
//!     `host.allow_exec()`.
//!
//! Network-fronted builtins gate on `host.allow_http()`. `onnx_infer`
//! gates on `host.allow_exec()`. `ai_token_count` is pure.
//!
//! Caller-supplied `options_json` for `ai_chat` is capped at
//! `host.max_external_json_bytes()` to mitigate DoS from large prompts.

use pblang_ir::{alloc_string, RuntimeError, Value, VmHost};
use serde_json as sj;

use super::json::{json_to_value, value_to_json};

pub(super) fn onnx_infer(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if !host.allow_exec() {
        return Err(host.err("exec access denied".into()));
    }
    if args.is_empty() {
        return Err(
            host.err("onnx_infer() expects at least 1 argument (model_path)".into())
        );
    }
    let model_path = args[0]
        .as_str()
        .map_err(|m| host.err(format!("onnx_infer(): {m}")))?;
    let input = if args.len() >= 2 {
        match &args[1] {
            Value::Obj(_) | Value::Array(_) | Value::Map(_) => value_to_json(&args[1])
                .map_err(|m| host.err(format!("onnx_infer(): {m}")))?,
            _ => {
                let s = args[1]
                    .as_str()
                    .map_err(|m| host.err(format!("onnx_infer(): {m}")))?;
                sj::from_str::<sj::Value>(s).map_err(|e| {
                    host.err(format!(
                        "onnx_infer(): input JSON parse failed: {e} (expected map/array JSON)"
                    ))
                })?
            }
        }
    } else {
        sj::json!({})
    };
    let options = if args.len() >= 3 {
        match &args[2] {
            Value::Obj(_) | Value::Array(_) | Value::Map(_) => value_to_json(&args[2])
                .map_err(|m| host.err(format!("onnx_infer(): {m}")))?,
            _ => {
                let s = args[2]
                    .as_str()
                    .map_err(|m| host.err(format!("onnx_infer(): {m}")))?;
                sj::from_str::<sj::Value>(s).map_err(|e| {
                    host.err(format!(
                        "onnx_infer(): options JSON parse failed: {e} (expected map JSON)"
                    ))
                })?
            }
        }
    } else {
        sj::json!({})
    };

    const ONNX_BRIDGE_SCRIPT: &str = r#"
import json
import sys

def _json_error(msg: str):
    print(json.dumps({"success": False, "error": msg}))

try:
    payload_raw = sys.stdin.read()
    payload = json.loads(payload_raw if payload_raw else "{}")
except Exception as e:
    _json_error(f"invalid payload JSON: {e}")
    sys.exit(0)

model_path = payload.get("model_path")
input_data = payload.get("input", {})
options = payload.get("options", {}) or {}

if not model_path:
    _json_error("missing model_path")
    sys.exit(0)

try:
    import numpy as np
    import onnxruntime as ort
except Exception as e:
    _json_error(f"onnxruntime bridge unavailable: {e}. Install with: pip install onnxruntime numpy")
    sys.exit(0)

def _to_numpy(value, dtype):
    try:
        arr = np.array(value, dtype=dtype)
    except Exception:
        arr = np.array(value, dtype=np.float32)
    return arr

try:
    providers_opt = options.get("providers")
    if isinstance(providers_opt, list) and providers_opt:
        session = ort.InferenceSession(model_path, providers=providers_opt)
    else:
        session = ort.InferenceSession(model_path)

    input_meta = session.get_inputs()
    if len(input_meta) == 0:
        _json_error("model has no inputs")
        sys.exit(0)

    feeds = {}
    if isinstance(input_data, dict):
        for meta in input_meta:
            if meta.name not in input_data:
                _json_error(f"missing input '{meta.name}'")
                sys.exit(0)
            dtype = options.get("dtype", "float32")
            arr = _to_numpy(input_data[meta.name], dtype)
            shape = meta.shape
            # Best-effort reshape only when shape is fully concrete.
            if isinstance(shape, list) and all((isinstance(d, int) and d > 0) for d in shape):
                if arr.size == int(np.prod(shape)):
                    arr = arr.reshape(shape)
            feeds[meta.name] = arr
    else:
        meta = input_meta[0]
        dtype = options.get("dtype", "float32")
        arr = _to_numpy(input_data, dtype)
        shape = meta.shape
        if isinstance(shape, list) and all((isinstance(d, int) and d > 0) for d in shape):
            if arr.size == int(np.prod(shape)):
                arr = arr.reshape(shape)
        feeds[meta.name] = arr

    outputs = session.run(None, feeds)
    output_meta = session.get_outputs()
    out_map = {}
    for idx, out in enumerate(outputs):
        name = output_meta[idx].name if idx < len(output_meta) else f"output_{idx}"
        try:
            out_map[name] = out.tolist()
        except Exception:
            out_map[name] = str(out)

    print(json.dumps({
        "success": True,
        "inputs": [m.name for m in input_meta],
        "outputs": out_map,
        "providers": session.get_providers(),
    }))
except Exception as e:
    _json_error(str(e))
"#;

    let payload = sj::json!({
        "model_path": model_path,
        "input": input,
        "options": options,
    })
    .to_string();

    let mut last_spawn_err: Option<std::io::Error> = None;
    for python in ["python3", "python"] {
        let mut child = match std::process::Command::new(python)
            .arg("-c")
            .arg(ONNX_BRIDGE_SCRIPT)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
        {
            Ok(c) => c,
            Err(e) => {
                last_spawn_err = Some(e);
                continue;
            }
        };

        if let Some(stdin) = child.stdin.as_mut() {
            use std::io::Write;
            stdin.write_all(payload.as_bytes()).map_err(|e| {
                host.err(format!("onnx_infer(): write bridge payload: {e}"))
            })?;
        }
        let output = child
            .wait_with_output()
            .map_err(|e| host.err(format!("onnx_infer(): wait bridge process: {e}")))?;
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

        if stdout.is_empty() {
            return Err(host.err(format!(
                "onnx_infer(): bridge produced no output (python={python}, stderr={stderr})"
            )));
        }
        let parsed = sj::from_str::<sj::Value>(&stdout).map_err(|e| {
            host.err(format!(
                "onnx_infer(): invalid bridge JSON: {e}; stdout={stdout}; stderr={stderr}"
            ))
        })?;
        return Ok(json_to_value(&parsed));
    }

    Err(host.err(format!(
        "onnx_infer(): Python interpreter not found (tried python3, python): {}",
        last_spawn_err
            .map(|e| e.to_string())
            .unwrap_or_else(|| "spawn failed".to_string())
    )))
}

/// ai_chat(prompt, options_json) — Chat completion via registered AI providers.
/// options_json: JSON string with optional { provider, model, system_prompt, temperature, max_tokens }
pub(super) fn ai_chat(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if !host.allow_http() {
        return Err(host.err("http access denied".into()));
    }
    if args.is_empty() {
        return Err(host.err("ai_chat() expects at least 1 argument (prompt)".into()));
    }
    let prompt = args[0]
        .as_str()
        .map_err(|m| host.err(format!("ai_chat(): {m}")))?;
    let options_str = if args.len() >= 2 {
        args[1]
            .as_str()
            .map_err(|m| host.err(format!("ai_chat(): {m}")))?
    } else {
        "{}"
    };

    let mut registry = pblang_ai::ProviderRegistry::new();
    registry.auto_register();

    let options: pblang_ai::ChatOptions =
        if options_str.trim().is_empty() || options_str == "{}" {
            pblang_ai::ChatOptions::default()
        } else {
            // Cap the options payload; ai_chat is a network-fronted
            // builtin and options often come from user-controlled
            // strings derived from external sources.
            let cap = host.max_external_json_bytes();
            if options_str.len() > cap {
                return Err(host.err(format!(
                    "ai_chat(): options JSON of {} bytes exceeds cap of {} bytes",
                    options_str.len(),
                    cap
                )));
            }
            let v: serde_json::Value = serde_json::from_str(options_str)
                .map_err(|e| host.err(format!("ai_chat(): invalid options JSON: {e}")))?;
            let mut opts = pblang_ai::ChatOptions::default();
            if let Some(p) = v.get("provider").and_then(|x| x.as_str()) {
                opts.provider = Some(p.to_string());
            }
            if let Some(m) = v.get("model").and_then(|x| x.as_str()) {
                opts.model = Some(m.to_string());
            }
            if let Some(s) = v.get("system_prompt").and_then(|x| x.as_str()) {
                opts.system_prompt = Some(s.to_string());
            }
            if let Some(t) = v.get("temperature").and_then(|x| x.as_f64()) {
                opts.temperature = Some(t);
            }
            if let Some(mt) = v.get("max_tokens").and_then(|x| x.as_u64()) {
                opts.max_tokens = Some(mt as u32);
            }
            opts
        };

    match pblang_ai::chat(prompt, &options, &registry) {
        Ok(resp) => {
            let json = pblang_ai::chat_response_to_json(&resp);
            Ok(alloc_string(json))
        }
        Err(e) => Err(host.err(format!("ai_chat(): {e}"))),
    }
}

/// ai_embed(text, options_json) — Generate embedding vector from text.
pub(super) fn ai_embed(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if !host.allow_http() {
        return Err(host.err("http access denied".into()));
    }
    if args.is_empty() {
        return Err(host.err("ai_embed() expects at least 1 argument (text)".into()));
    }
    let text = args[0]
        .as_str()
        .map_err(|m| host.err(format!("ai_embed(): {m}")))?;
    let options_str = if args.len() >= 2 {
        args[1]
            .as_str()
            .map_err(|m| host.err(format!("ai_embed(): {m}")))?
    } else {
        "{}"
    };

    let mut registry = pblang_ai::ProviderRegistry::new();
    registry.auto_register();

    let options: pblang_ai::ChatOptions =
        if options_str.trim().is_empty() || options_str == "{}" {
            pblang_ai::ChatOptions::default()
        } else {
            let v: serde_json::Value = serde_json::from_str(options_str)
                .map_err(|e| host.err(format!("ai_embed(): invalid options JSON: {e}")))?;
            let mut opts = pblang_ai::ChatOptions::default();
            if let Some(p) = v.get("provider").and_then(|x| x.as_str()) {
                opts.provider = Some(p.to_string());
            }
            if let Some(m) = v.get("model").and_then(|x| x.as_str()) {
                opts.model = Some(m.to_string());
            }
            opts
        };

    match pblang_ai::embed(text, &options, &registry) {
        Ok(resp) => {
            let json = pblang_ai::embedding_response_to_json(&resp);
            Ok(alloc_string(json))
        }
        Err(e) => Err(host.err(format!("ai_embed(): {e}"))),
    }
}

/// ai_token_count(text) — Estimate token count for text.
pub(super) fn ai_token_count(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 1 {
        return Err(host.err(format!(
            "ai_token_count() expects 1 argument, got {}",
            args.len()
        )));
    }
    let text = args[0]
        .as_str()
        .map_err(|m| host.err(format!("ai_token_count(): {m}")))?;
    let tc = pblang_ai::estimate_tokens(text);
    let json = pblang_ai::token_count_to_json(&tc);
    Ok(alloc_string(json))
}
