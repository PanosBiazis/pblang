# AI plugin / package pattern

Τελευταία ενημέρωση: Ιούνιος 2026.

## Στόχος

Να τυποποιήσουμε πώς οι δυνατότητες AI (inference, embeddings, LLM APIs) έρχονται στο pblang **ως πακέτα/plugins**, χωρίς να φορτώνουμε τον core runtime με βαριά dependencies ή να αλλάζουμε τη γλώσσα.

## Αρχή: Core μένει μικρός

- Ο **core** (parser, compiler, VM, stdlib builtins) δεν φορτώνει ONNX/CUDA/Python stacks μέσα στο ίδιο process.
- Το **onnx_infer** builtin είναι πλέον capability-gated Python bridge: χρειάζεται `allow_exec`, δοκιμάζει `python3`/`python`, φορτώνει `onnxruntime` εκεί, και επιστρέφει structured map (`success`, `outputs` ή `error`). Αν λείπει Python/onnxruntime ή το model path είναι άκυρο, αποτυγχάνει καθαρά αντί να κρασάρει τον VM.
- Οι πραγματικές AI δυνατότητες έρχονται μέσω:
  1. **Host embedding**: Η εφαρμογή (Rust/C++) φορτώνει pblang και παρέχει capabilities (π.χ. `infer(model, input)` ως host function).
  2. **FFI**: Script φορτώνει dll/so που εκθέτει C ABI (ONNX Runtime, libtorch, κ.λπ.) και καλεί μέσω `ffi_load` / `ffi_call` (με FfiPolicy allowlist).
  3. **Packages**: Πακέτο τύπου `@pblang/onnx` ή `@pblang/openai` που περιέχει pblang code + optional native libs· το script κάνει `import "@pblang/onnx"` και χρησιμοποιεί API που εσωτερικά καλεί FFI ή host-provided capabilities.

## Τυπικό pattern για πακέτο AI

1. **Package root**: pblang module(s) που εκθέτουν συναρτήσεις (π.χ. `onnx.run(model_path, input_map)`).
2. **Υλοποίηση**: Εither (α) το module καλεί builtins που ο host έχει παρασχεθεί (capability), ή (β) το module καλεί `ffi_load` σε συγκεκριμένη βιβλιοθήκη (που το package συμπεριλαμβάνει ή που εγκαθίσταται χωριστά) και στη συνέχεια `ffi_call`.
3. **Registry**: Όταν υπάρχει registry, το πακέτο δημοσιεύεται εκεί· τα projects το προσθέτουν στο `pblang.toml` και κάνουν `pblang install`.
4. **Sandbox**: Σε sandbox mode το `allow_ffi` μπορεί να απενεργοποιείται· ο host αποφασίζει αν επιτρέπει AI plugins (μέσω FFI ή capability).

## Minimal runnable example

- Υπάρχει πλέον μικρό example στο `examples/ai_plugin_minimal/`.
- Το `examples/ai_plugin_minimal/plugin/` δείχνει το package πλευρά: εκθέτει σταθερό API (`plugin_info`, `capabilities`, `build_request`, `invoke`) χωρίς να δένει τον VM πυρήνα με συγκεκριμένο AI provider.
- Το `examples/ai_plugin_minimal/app/` δείχνει τον consumer πλευρά: local path dependency σήμερα, αλλά το ίδιο contract μπορεί αργότερα να δημοσιευτεί στο registry.
- Το example είναι σκόπιμα **contract-first**: το package φτιάχνει normalized request envelope και ο πραγματικός host/FFI bridge μπορεί αργότερα να αναλάβει την εκτέλεση.
- Το envelope περιγράφει ρητά schema/version/provider/operation/options/capabilities ώστε το επόμενο `A2` βήμα να μπορεί να συνδέσει host, FFI ή HTTP provider adapter χωρίς να αλλάξει ο VM πυρήνας.

## HTTP Provider Adapter (A3-alpha)

**File:** `examples/ai_http_provider/main.sapp`

OpenAI-style REST adapter using `stdlib:http.sapp` and `stdlib:json.sapp`. No VM core changes.

### Exported functions

| Function | Signature | Returns |
|----------|-----------|---------|
| `build_request` | `(model, messages)` | JSON string |
| `call_provider` | `(endpoint, api_key, body)` | parsed response map |
| `openai_chat` | `(api_key, model, messages)` | parsed response map |
| `streaming_chat` | `(api_key, model, messages, on_chunk)` | void |
| `provider_info` | `()` | capability map |

### Usage

```pblang
let p = import("examples/ai_http_provider/main.sapp");
let res = p.openai_chat(my_key, "gpt-4o", [map{ role: "user", content: "Hello!" }]);
print res["choices"][0]["message"]["content"];
```

### Current HTTP capability

- **Provider auth:** use `stdlib:http.sapp` `post_auth(url, body, token)`. It builds the provider auth header on top of the lower-level builtin `http_post_auth(url, headers_map, body)`.
- **Streaming:** `get_stream`, `post_stream`, and `post_stream_auth` are SSE-style shims today: they read the full response, split on blank lines, and invoke `on_chunk` for each event-sized chunk. This preserves the public API while leaving true socket-level chunk streaming as future runtime work.

### Extending to other providers

Swap the endpoint in `call_provider` for Anthropic, Mistral, Ollama, etc. No VM changes.

---

## Streaming Inference (A4-alpha)

### SSE token stream (OpenAI-style)

Use `http.post_stream_auth(url, body, token, on_chunk)` + `http.parse_sse_line(line)`:

```pblang
import "stdlib:http.sapp";
http.post_stream_auth(url, body, api_key, fn(chunk) {
    let lines = str_split(chunk, "\n");
    for i len(lines) {
        let p = http.parse_sse_line(lines[i]);
        if p != nil and p.kind == "data" and p.value != "[DONE]" {
            print p.value;
        }
    }
});
```

- **get_stream(url, on_chunk)** — GET then split on `\n\n`, call on_chunk per part (shim; full body in memory).
- **post_stream(url, body, on_chunk)** — same for POST.
- **post_stream_auth(url, body, token, on_chunk)** — same for authenticated Bearer POST.
- **parse_sse_line(line)** — returns `map{ kind: "data"|"event", value: string }` or nil.

Example: **openai_chat_stream(prompt, api_key, on_token)** in `examples/ai_http_provider/main.sapp` parses SSE data lines and calls on_token(delta) for each content delta.

### Multi-provider routing (A4-beta)

For simple multi-provider setups, use `stdlib:provider_registry.sapp`:

```pblang
import "stdlib:provider_registry.sapp";

provider_registry.register("primary", "https://api.primary-llm.com/v1/chat", primary_key);
provider_registry.register("backup", "https://api.backup-llm.com/v1/chat", backup_key);

// Best-provider call (first registered)
let r1 = provider_registry.call_best("Hello!", "gpt-4o");

// Fallback chain: try providers in registration order, skip error maps,
// and return the first non-error result or { error: "all providers failed" }.
let r2 = provider_registry.call_with_fallback("Hello!", "gpt-4o");
if r2.error != nil {
    print "all providers failed";
}
```

### ONNX local inference path

For local model inference today, use `stdlib:ai.sapp`:

```pblang
import "stdlib:ai.sapp";
let result = ai.onnx_run("model.onnx", map{ input: [1.0, 2.0] });
if result.success == false {
    print result.error;
}
```

This wraps `onnx_infer(model_path, input_json, options_json)`. It is intentionally
isolated behind process execution instead of linking ONNX Runtime into the VM. For
production plugin packaging, the FFI path is still valid:

1. Build a thin Rust crate `pblang-onnx` that wraps `ort` (ONNX Runtime bindings) and exposes a C ABI: `pblang_onnx_load(model_path)`, `pblang_onnx_run(session, input_json)`.
2. Load via FFI: `let onnx = ffi.load("pblang_onnx.dll");`
3. Call: `let result = onnx.pblang_onnx_run(session, json_stringify(inputs));`

---

## Σχετικά

- [Embedding.md](Embedding.md) — πώς ο host παρέχει capabilities και ελέγχει FFI.
- [Registry_MVP.md](Registry_MVP.md) — πώς θα διανέμονται τα πακέτα.
- [Future_plans.md](Future_plans.md) — AI-ready ως interop-first (FFI/plugins/host).
