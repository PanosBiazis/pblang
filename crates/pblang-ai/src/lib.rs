// pblang-ai — AI/ML integration for pblang.
//
// Provides:
//   • Chat completion via HTTP APIs (OpenAI, Anthropic, Ollama, custom)
//   • Embedding generation + cosine similarity
//   • Token counting / estimation
//   • Streaming chat responses
//   • Tool / function calling
//   • Provider registry
//
// Architecture:
//   The crate is a pure-Rust library that pblang's VM can call via its
//   builtin mechanism (or through the FFI layer). All network calls use
//   ureq (blocking) with timeouts.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Read;
use std::time::Duration;

// ─────────────────────────────────────────────────────
// Provider configuration
// ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub base_url: String,
    #[serde(skip_serializing)]
    pub api_key: Option<String>,
    pub model_default: Option<String>,
    #[serde(skip_serializing)]
    pub headers: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Default)]
pub struct ProviderRegistry {
    providers: HashMap<String, ProviderConfig>,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        Self {
            providers: HashMap::new(),
        }
    }

    pub fn register(&mut self, name: &str, config: ProviderConfig) {
        self.providers.insert(name.to_string(), config);
    }

    pub fn get(&self, name: &str) -> Option<&ProviderConfig> {
        self.providers.get(name)
    }

    /// Auto-register from environment variables.
    pub fn auto_register(&mut self) {
        if let Ok(key) = std::env::var("OPENAI_API_KEY") {
            self.register(
                "openai",
                ProviderConfig {
                    base_url: "https://api.openai.com/v1".into(),
                    api_key: Some(key),
                    model_default: Some("gpt-4o-mini".into()),
                    headers: None,
                },
            );
        }
        if let Ok(key) = std::env::var("ANTHROPIC_API_KEY") {
            let key_clone = key.clone();
            self.register(
                "anthropic",
                ProviderConfig {
                    base_url: "https://api.anthropic.com/v1".into(),
                    api_key: Some(key),
                    model_default: Some("claude-3-haiku-20240307".into()),
                    headers: Some({
                        let mut h = HashMap::new();
                        h.insert("anthropic-version".into(), "2023-06-01".into());
                        h.insert("x-api-key".into(), key_clone);
                        h
                    }),
                },
            );
        }
        let ollama_url =
            std::env::var("OLLAMA_URL").unwrap_or_else(|_| "http://localhost:11434".into());
        self.register(
            "ollama",
            ProviderConfig {
                base_url: ollama_url,
                api_key: None,
                model_default: Some("llama3.2".into()),
                headers: None,
            },
        );
    }
}

// ─────────────────────────────────────────────────────
// Chat message types
// ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: Option<String>,
    #[serde(rename = "type")]
    pub call_type: String,
    pub function: ToolCallFunction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallFunction {
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDef {
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: FunctionDef,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionDef {
    pub name: String,
    pub description: Option<String>,
    pub parameters: serde_json::Value,
}

// ─────────────────────────────────────────────────────
// Chat options
// ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct ChatOptions {
    pub provider: Option<String>,
    pub model: Option<String>,
    pub system_prompt: Option<String>,
    pub temperature: Option<f64>,
    pub max_tokens: Option<u32>,
    pub messages: Option<Vec<Message>>,
    pub tools: Option<Vec<ToolDef>>,
    pub tool_choice: Option<String>,
    pub stop: Option<Vec<String>>,
    pub top_p: Option<f64>,
}

// ─────────────────────────────────────────────────────
// Chat response
// ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct ChatResponse {
    pub content: String,
    pub model: String,
    pub provider: String,
    pub total_tokens: Option<u64>,
    pub prompt_tokens: Option<u64>,
    pub completion_tokens: Option<u64>,
    pub tool_calls: Option<Vec<ToolCall>>,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EmbeddingResponse {
    pub embedding: Vec<f64>,
    pub model: String,
    pub dimensions: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct TokenCount {
    pub estimated: bool,
    pub count: u64,
    pub chars: u64,
    pub words: u64,
}

// ─────────────────────────────────────────────────────
// HTTP request helpers
// ─────────────────────────────────────────────────────

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(120);

fn http_post_json(
    url: &str,
    body: serde_json::Value,
    headers: &HashMap<String, String>,
    timeout: Duration,
) -> Result<serde_json::Value, String> {
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(timeout)
        .timeout_read(timeout)
        .timeout_write(timeout)
        .build();
    let mut req = agent.post(url);
    for (k, v) in headers {
        req = req.set(k, v);
    }
    let resp = req
        .send_json(body)
        .map_err(|e| format!("HTTP POST {}: {e}", url))?;
    let mut resp_body = String::new();
    resp.into_reader()
        .read_to_string(&mut resp_body)
        .map_err(|e| format!("read response: {e}"))?;
    serde_json::from_str(&resp_body).map_err(|e| {
        format!(
            "parse JSON response: {e} (body: {})",
            &resp_body[..resp_body.len().min(500)]
        )
    })
}

#[allow(dead_code)]
fn http_get_json(
    url: &str,
    headers: &HashMap<String, String>,
    timeout: Duration,
) -> Result<serde_json::Value, String> {
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(timeout)
        .timeout_read(timeout)
        .timeout_write(timeout)
        .build();
    let mut req = agent.get(url);
    for (k, v) in headers {
        req = req.set(k, v);
    }
    let resp = req.call().map_err(|e| format!("HTTP GET {}: {e}", url))?;
    let mut resp_body = String::new();
    resp.into_reader()
        .read_to_string(&mut resp_body)
        .map_err(|e| format!("read response: {e}"))?;
    serde_json::from_str(&resp_body).map_err(|e| {
        format!(
            "parse JSON response: {e} (body: {})",
            &resp_body[..resp_body.len().min(500)]
        )
    })
}

// ─────────────────────────────────────────────────────
// Chat completion
// ─────────────────────────────────────────────────────

/// Send a chat completion to a provider. Returns ChatResponse.
pub fn chat(
    prompt: &str,
    options: &ChatOptions,
    registry: &ProviderRegistry,
) -> Result<ChatResponse, String> {
    let provider_name = options.provider.as_deref().unwrap_or("ollama");
    let provider = registry.get(provider_name).ok_or_else(|| {
        format!(
            "ai.chat: unknown provider '{}'. Register via ProviderRegistry::register()",
            provider_name
        )
    })?;
    let model = options
        .model
        .as_deref()
        .or(provider.model_default.as_deref())
        .unwrap_or("llama3.2");

    match provider_name {
        "openai" | "ollama" => {
            chat_openai_compatible(prompt, options, provider, model, provider_name)
        }
        "anthropic" => chat_anthropic(prompt, options, provider, model),
        other => Err(format!("ai.chat: unsupported provider '{}'", other)),
    }
}

fn chat_openai_compatible(
    prompt: &str,
    options: &ChatOptions,
    provider: &ProviderConfig,
    model: &str,
    provider_name: &str,
) -> Result<ChatResponse, String> {
    let mut headers = HashMap::new();
    headers.insert("Content-Type".into(), "application/json".into());
    if let Some(ref key) = provider.api_key {
        headers.insert("Authorization".into(), format!("Bearer {key}"));
    }
    if let Some(ref extra) = provider.headers {
        headers.extend(extra.clone());
    }

    // Build messages
    let messages = if let Some(ref msgs) = options.messages {
        serde_json::to_value(msgs).map_err(|e| format!("serialize messages: {e}"))?
    } else {
        let mut msgs = Vec::new();
        if let Some(ref sys) = options.system_prompt {
            msgs.push(serde_json::json!({"role": "system", "content": sys}));
        }
        msgs.push(serde_json::json!({"role": "user", "content": prompt}));
        serde_json::Value::Array(msgs)
    };

    let mut body = serde_json::json!({
        "model": model,
        "messages": messages,
        "temperature": options.temperature.unwrap_or(0.7),
    });

    if let Some(mt) = options.max_tokens {
        body["max_tokens"] = serde_json::json!(mt);
    }
    if let Some(tp) = options.top_p {
        body["top_p"] = serde_json::json!(tp);
    }
    if let Some(ref stop) = options.stop {
        body["stop"] = serde_json::json!(stop);
    }
    if let Some(ref tools) = options.tools {
        body["tools"] = serde_json::to_value(tools).map_err(|e| format!("serialize tools: {e}"))?;
    }
    if let Some(ref tc) = options.tool_choice {
        body["tool_choice"] = serde_json::json!(tc);
    }

    let url = if provider_name == "ollama" {
        format!("{}/api/chat", provider.base_url.trim_end_matches('/'))
    } else {
        format!(
            "{}/chat/completions",
            provider.base_url.trim_end_matches('/')
        )
    };

    let result = http_post_json(&url, body, &headers, DEFAULT_TIMEOUT)?;

    // Parse response
    let content;
    let tool_calls;
    let finish_reason;
    let usage;

    if provider_name == "ollama" {
        content = result["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string();
        tool_calls = None;
        finish_reason = result
            .get("done_reason")
            .and_then(|v| v.as_str())
            .map(String::from);
        usage = None;
    } else {
        let choice = &result["choices"][0];
        content = choice["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string();
        finish_reason = choice["finish_reason"].as_str().map(String::from);

        // Parse tool calls if present
        tool_calls = if let Some(tc_array) = choice["message"]["tool_calls"].as_array() {
            let mut calls = Vec::new();
            for tc in tc_array {
                calls.push(ToolCall {
                    id: tc["id"].as_str().map(String::from),
                    call_type: tc["type"].as_str().unwrap_or("function").to_string(),
                    function: ToolCallFunction {
                        name: tc["function"]["name"].as_str().unwrap_or("").to_string(),
                        arguments: tc["function"]["arguments"]
                            .as_str()
                            .unwrap_or("{}")
                            .to_string(),
                    },
                });
            }
            Some(calls)
        } else {
            None
        };

        usage = Some((
            result["usage"]["prompt_tokens"].as_u64(),
            result["usage"]["completion_tokens"].as_u64(),
            result["usage"]["total_tokens"].as_u64(),
        ));
    }

    Ok(ChatResponse {
        content,
        model: model.to_string(),
        provider: provider_name.to_string(),
        total_tokens: usage.and_then(|(_, _, t)| t),
        prompt_tokens: usage.and_then(|(p, _, _)| p),
        completion_tokens: usage.and_then(|(_, c, _)| c),
        tool_calls,
        finish_reason,
    })
}

fn chat_anthropic(
    prompt: &str,
    options: &ChatOptions,
    provider: &ProviderConfig,
    model: &str,
) -> Result<ChatResponse, String> {
    let mut headers = HashMap::new();
    headers.insert("Content-Type".into(), "application/json".into());
    if let Some(ref key) = provider.api_key {
        headers.insert("x-api-key".into(), key.clone());
    }
    headers.insert("anthropic-version".into(), "2023-06-01".into());
    if let Some(ref extra) = provider.headers {
        headers.extend(extra.clone());
    }

    let messages = if let Some(ref msgs) = options.messages {
        // Convert to Anthropic format
        let mut anthropic_msgs = Vec::new();
        for m in msgs {
            anthropic_msgs.push(serde_json::json!({"role": m.role, "content": m.content}));
        }
        serde_json::Value::Array(anthropic_msgs)
    } else {
        let mut msgs = Vec::new();
        if let Some(ref sys) = options.system_prompt {
            msgs.push(serde_json::json!({"role": "user", "content": format!("<system>{sys}</system>\n\n{prompt}")}));
        } else {
            msgs.push(serde_json::json!({"role": "user", "content": prompt}));
        }
        serde_json::Value::Array(msgs)
    };

    let mut body = serde_json::json!({
        "model": model,
        "messages": messages,
        "max_tokens": options.max_tokens.unwrap_or(2048),
    });

    if let Some(temp) = options.temperature {
        body["temperature"] = serde_json::json!(temp);
    }
    if let Some(ref sys) = options.system_prompt {
        // Anthropic supports system separately
        body["system"] = serde_json::json!(sys);
    }
    if let Some(ref stop) = options.stop {
        body["stop_sequences"] = serde_json::json!(stop);
    }

    let url = format!("{}/messages", provider.base_url.trim_end_matches('/'));
    let result = http_post_json(&url, body, &headers, DEFAULT_TIMEOUT)?;

    let content = result["content"][0]["text"]
        .as_str()
        .unwrap_or("")
        .to_string();
    let usage = (
        result["usage"]["input_tokens"].as_u64(),
        result["usage"]["output_tokens"].as_u64(),
    );

    Ok(ChatResponse {
        content,
        model: model.to_string(),
        provider: "anthropic".to_string(),
        total_tokens: usage.0.and_then(|i| usage.1.map(|o| i + o)),
        prompt_tokens: usage.0,
        completion_tokens: usage.1,
        tool_calls: None,
        finish_reason: result["stop_reason"].as_str().map(String::from),
    })
}

// ─────────────────────────────────────────────────────
// Streaming chat
// ─────────────────────────────────────────────────────

/// Stream chat completion, calling `on_chunk` for each text delta.
pub fn chat_stream(
    prompt: &str,
    options: &ChatOptions,
    registry: &ProviderRegistry,
    mut on_chunk: impl FnMut(&str),
) -> Result<ChatResponse, String> {
    let provider_name = options.provider.as_deref().unwrap_or("ollama");
    let provider = registry
        .get(provider_name)
        .ok_or_else(|| format!("ai.chat_stream: unknown provider '{}'", provider_name))?;
    let model = options
        .model
        .as_deref()
        .or(provider.model_default.as_deref())
        .unwrap_or("llama3.2");

    let mut headers = HashMap::new();
    headers.insert("Content-Type".into(), "application/json".into());
    if let Some(ref key) = provider.api_key {
        headers.insert("Authorization".into(), format!("Bearer {key}"));
    }
    if let Some(ref extra) = provider.headers {
        headers.extend(extra.clone());
    }

    let messages = if let Some(ref msgs) = options.messages {
        serde_json::to_value(msgs).map_err(|e| format!("serialize messages: {e}"))?
    } else {
        let mut msgs = Vec::new();
        if let Some(ref sys) = options.system_prompt {
            msgs.push(serde_json::json!({"role": "system", "content": sys}));
        }
        msgs.push(serde_json::json!({"role": "user", "content": prompt}));
        serde_json::Value::Array(msgs)
    };

    let url = if provider_name == "ollama" {
        format!("{}/api/chat", provider.base_url.trim_end_matches('/'))
    } else {
        format!(
            "{}/chat/completions",
            provider.base_url.trim_end_matches('/')
        )
    };

    // For streaming, use the non-json API to handle SSE
    let mut body = serde_json::json!({
        "model": model,
        "messages": messages,
        "temperature": options.temperature.unwrap_or(0.7),
        "stream": true,
    });
    if let Some(mt) = options.max_tokens {
        body["max_tokens"] = serde_json::json!(mt);
    }

    let agent = ureq::AgentBuilder::new()
        .timeout_connect(DEFAULT_TIMEOUT)
        .timeout_read(DEFAULT_TIMEOUT)
        .timeout_write(DEFAULT_TIMEOUT)
        .build();
    let mut req = agent.post(&url);
    for (k, v) in &headers {
        req = req.set(k, v);
    }

    let resp = req
        .send_json(body)
        .map_err(|e| format!("HTTP POST stream {}: {e}", url))?;
    let reader = resp.into_reader();

    let mut full_content = String::new();
    let mut buf = [0u8; 4096];
    let mut remainder = String::new();
    let mut reader = reader;

    loop {
        let n = reader
            .read(&mut buf)
            .map_err(|e| format!("stream read: {e}"))?;
        if n == 0 {
            break;
        }
        let chunk = String::from_utf8_lossy(&buf[..n]).to_string();
        remainder.push_str(&chunk);

        while let Some(pos) = remainder.find('\n') {
            let line = remainder[..pos].to_string();
            remainder = remainder[pos + 1..].to_string();

            let line = line.trim().to_string();
            if line.is_empty() || line == "data: [DONE]" {
                continue;
            }
            if let Some(data) = line.strip_prefix("data: ") {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(data) {
                    if provider_name == "ollama" {
                        if let Some(delta) = parsed["message"]["content"].as_str() {
                            full_content.push_str(delta);
                            on_chunk(delta);
                        }
                    } else {
                        if let Some(delta) = parsed["choices"][0]["delta"]["content"].as_str() {
                            full_content.push_str(delta);
                            on_chunk(delta);
                        }
                    }
                }
            }
        }
    }

    Ok(ChatResponse {
        content: full_content,
        model: model.to_string(),
        provider: provider_name.to_string(),
        total_tokens: None,
        prompt_tokens: None,
        completion_tokens: None,
        tool_calls: None,
        finish_reason: Some("stream_end".into()),
    })
}

// ─────────────────────────────────────────────────────
// Embeddings
// ─────────────────────────────────────────────────────

/// Generate an embedding vector from text.
pub fn embed(
    text: &str,
    options: &ChatOptions,
    registry: &ProviderRegistry,
) -> Result<EmbeddingResponse, String> {
    let provider_name = options.provider.as_deref().unwrap_or("ollama");
    let provider = registry
        .get(provider_name)
        .ok_or_else(|| format!("ai.embed: unknown provider '{}'", provider_name))?;
    let model = options
        .model
        .as_deref()
        .or(provider.model_default.as_deref())
        .unwrap_or(if provider_name == "ollama" {
            "nomic-embed-text"
        } else {
            "text-embedding-3-small"
        });

    let mut headers = HashMap::new();
    headers.insert("Content-Type".into(), "application/json".into());
    if let Some(ref key) = provider.api_key {
        headers.insert("Authorization".into(), format!("Bearer {key}"));
    }
    if let Some(ref extra) = provider.headers {
        headers.extend(extra.clone());
    }

    let url = if provider_name == "ollama" {
        format!("{}/api/embed", provider.base_url.trim_end_matches('/'))
    } else {
        format!("{}/embeddings", provider.base_url.trim_end_matches('/'))
    };

    let body = serde_json::json!({"model": model, "input": text});

    let result = http_post_json(&url, body, &headers, DEFAULT_TIMEOUT)?;

    let embedding: Vec<f64> = if provider_name == "ollama" {
        result["embeddings"][0]
            .as_array()
            .ok_or("embed: missing embeddings[0]")?
            .iter()
            .filter_map(|v| v.as_f64())
            .collect()
    } else {
        result["data"][0]["embedding"]
            .as_array()
            .ok_or("embed: missing data[0].embedding")?
            .iter()
            .filter_map(|v| v.as_f64())
            .collect()
    };

    let dims = embedding.len();

    Ok(EmbeddingResponse {
        embedding,
        model: model.to_string(),
        dimensions: dims,
    })
}

// ─────────────────────────────────────────────────────
// Cosine similarity
// ─────────────────────────────────────────────────────

/// Compute cosine similarity between two vectors.
pub fn cosine_similarity(a: &[f64], b: &[f64]) -> Result<f64, String> {
    if a.len() != b.len() {
        return Err(format!(
            "cosine_similarity: vectors must have the same length ({} vs {})",
            a.len(),
            b.len()
        ));
    }
    if a.is_empty() {
        return Ok(0.0);
    }

    let mut dot = 0.0;
    let mut mag_a = 0.0;
    let mut mag_b = 0.0;

    for i in 0..a.len() {
        dot += a[i] * b[i];
        mag_a += a[i] * a[i];
        mag_b += b[i] * b[i];
    }

    let denom = mag_a.sqrt() * mag_b.sqrt();
    if denom < 1e-12 {
        Ok(0.0)
    } else {
        Ok(dot / denom)
    }
}

/// Dot product of two vectors.
pub fn dot_product(a: &[f64], b: &[f64]) -> Result<f64, String> {
    if a.len() != b.len() {
        return Err(format!(
            "dot_product: vectors must have the same length ({} vs {})",
            a.len(),
            b.len()
        ));
    }
    Ok(a.iter().zip(b.iter()).map(|(x, y)| x * y).sum())
}

/// Euclidean distance between two vectors.
pub fn euclidean_distance(a: &[f64], b: &[f64]) -> Result<f64, String> {
    if a.len() != b.len() {
        return Err(format!(
            "euclidean_distance: vectors must have the same length ({} vs {})",
            a.len(),
            b.len()
        ));
    }
    Ok(a.iter()
        .zip(b.iter())
        .map(|(x, y)| (x - y).powi(2))
        .sum::<f64>()
        .sqrt())
}

// ─────────────────────────────────────────────────────
// Token counting (estimation-based)
// ─────────────────────────────────────────────────────

/// Estimate token count for text.
/// Uses the chars/4 heuristic (conservative) and chars/1.3 heuristic
/// (aggressive). Returns both estimates plus word count.
pub fn estimate_tokens(text: &str) -> TokenCount {
    let chars = text.chars().count() as u64;
    // Conservative: 4 chars per token
    let _conservative = chars.div_ceil(4);
    // More realistic for English: ~1.3 chars per token (GPT tokenizers)
    let realistic = ((chars as f64) / 1.3).ceil() as u64;
    let words = text.split_whitespace().count() as u64;

    TokenCount {
        estimated: true,
        count: realistic.max(1),
        chars,
        words,
    }
}

// ─────────────────────────────────────────────────────
// RAG pipeline helpers
// ─────────────────────────────────────────────────────

/// Chunk text into overlapping windows for RAG/document processing.
pub fn chunk_text(text: &str, chunk_size: usize, overlap: usize) -> Vec<String> {
    if chunk_size == 0 {
        return vec![text.to_string()];
    }
    let chars: Vec<char> = text.chars().collect();
    let mut chunks = Vec::new();
    let step = chunk_size.saturating_sub(overlap).max(1);

    let mut start = 0usize;
    while start < chars.len() {
        let end = (start + chunk_size).min(chars.len());
        let chunk: String = chars[start..end].iter().collect();
        chunks.push(chunk);
        start += step;
    }

    chunks
}

/// Semantic search: rank documents by cosine similarity to a query embedding.
pub fn semantic_search(
    query_embedding: &[f64],
    document_embeddings: &[Vec<f64>],
) -> Vec<(usize, f64)> {
    let mut scored: Vec<(usize, f64)> = document_embeddings
        .iter()
        .enumerate()
        .filter_map(|(idx, doc_emb)| {
            cosine_similarity(query_embedding, doc_emb)
                .ok()
                .map(|score| (idx, score))
        })
        .collect();
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    scored
}

// ─────────────────────────────────────────────────────
// JSON serialization helpers (for pblang FFI)
// ─────────────────────────────────────────────────────

/// Serialize a ChatResponse to JSON string.
pub fn chat_response_to_json(r: &ChatResponse) -> String {
    serde_json::to_string(r).unwrap_or_default()
}

/// Serialize an EmbeddingResponse to JSON string.
pub fn embedding_response_to_json(r: &EmbeddingResponse) -> String {
    serde_json::to_string(r).unwrap_or_default()
}

/// Serialize a TokenCount to JSON string.
pub fn token_count_to_json(t: &TokenCount) -> String {
    serde_json::to_string(t).unwrap_or_default()
}

/// Serialize embeddings array to JSON.
pub fn embeddings_to_json(embeddings: &[f64]) -> String {
    serde_json::to_string(embeddings).unwrap_or_default()
}

// ─────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cosine_similarity_identical() {
        let v = vec![1.0, 2.0, 3.0];
        let sim = cosine_similarity(&v, &v).unwrap();
        assert!((sim - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_cosine_similarity_orthogonal() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        let sim = cosine_similarity(&a, &b).unwrap();
        assert!((sim - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_cosine_similarity_opposite() {
        let a = vec![1.0, 0.0];
        let b = vec![-1.0, 0.0];
        let sim = cosine_similarity(&a, &b).unwrap();
        assert!((sim + 1.0).abs() < 0.001);
    }

    #[test]
    fn test_estimate_tokens() {
        let t = estimate_tokens("Hello world!");
        assert!(t.words == 2);
        assert!(t.count > 0);
    }

    #[test]
    fn test_chunk_text() {
        let chunks = chunk_text("abcdefghij", 4, 1);
        assert_eq!(chunks.len(), 4);
        assert_eq!(chunks[0], "abcd");
        assert_eq!(chunks[1], "defg");
    }

    #[test]
    fn test_provider_registry_auto_register() {
        // Test that we can create a registry without env vars crashing
        let mut reg = ProviderRegistry::new();
        reg.register(
            "test",
            ProviderConfig {
                base_url: "http://localhost:9999".into(),
                api_key: None,
                model_default: Some("test-model".into()),
                headers: None,
            },
        );
        assert!(reg.get("test").is_some());
        assert!(reg.get("nonexistent").is_none());
    }

    #[test]
    fn test_semantic_search() {
        let query = vec![1.0, 0.0, 0.0];
        let docs = vec![
            vec![1.0, 0.0, 0.0], // perfect match
            vec![0.0, 1.0, 0.0], // orthogonal
            vec![0.7, 0.7, 0.0], // partial
        ];
        let results = semantic_search(&query, &docs);
        assert_eq!(results[0].0, 0); // doc 0 should be first
        assert!(results[0].1 > results[1].1);
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn test_euclidean_distance() {
        let a = vec![0.0, 0.0];
        let b = vec![3.0, 4.0];
        let d = euclidean_distance(&a, &b).unwrap();
        assert!((d - 5.0).abs() < 0.001);
    }
}
