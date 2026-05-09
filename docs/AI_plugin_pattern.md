# AI plugin / package pattern

Τελευταία ενημέρωση: Φεβρουάριος 2025.

## Στόχος

Να τυποποιήσουμε πώς οι δυνατότητες AI (inference, embeddings, LLM APIs) έρχονται στο Sapphire **ως πακέτα/plugins**, χωρίς να φορτώνουμε τον core runtime με βαριά dependencies ή να αλλάζουμε τη γλώσσα.

## Αρχή: Core μένει μικρός

- Ο **core** (parser, compiler, VM, stdlib builtins) δεν περιλαμβάνει ONNX/CUDA/Python/HTTP AI stacks.
- Το **onnx_infer** (αν υπάρχει ως builtin) παραμένει **stub**: επιστρέφει σφάλμα ότι δεν είναι built-in και προτείνει host embedding ή FFI/plugin.
- Οι πραγματικές AI δυνατότητες έρχονται μέσω:
  1. **Host embedding**: Η εφαρμογή (Rust/C++) φορτώνει Sapphire και παρέχει capabilities (π.χ. `infer(model, input)` ως host function).
  2. **FFI**: Script φορτώνει dll/so που εκθέτει C ABI (ONNX Runtime, libtorch, κ.λπ.) και καλεί μέσω `ffi_load` / `ffi_call` (με FfiPolicy allowlist).
  3. **Packages**: Πακέτο τύπου `@sapphire/onnx` ή `@sapphire/openai` που περιέχει Sapphire code + optional native libs· το script κάνει `import "@sapphire/onnx"` και χρησιμοποιεί API που εσωτερικά καλεί FFI ή host-provided capabilities.

## Τυπικό pattern για πακέτο AI

1. **Package root**: Sapphire module(s) που εκθέτουν συναρτήσεις (π.χ. `onnx.run(model_path, input_map)`).
2. **Υλοποίηση**: Εither (α) το module καλεί builtins που ο host έχει παρασχεθεί (capability), ή (β) το module καλεί `ffi_load` σε συγκεκριμένη βιβλιοθήκη (που το package συμπεριλαμβάνει ή που εγκαθίσταται χωριστά) και στη συνέχεια `ffi_call`.
3. **Registry**: Όταν υπάρχει registry, το πακέτο δημοσιεύεται εκεί· τα projects το προσθέτουν στο `pblang.toml` και κάνουν `sapphire install`.
4. **Sandbox**: Σε sandbox mode το `allow_ffi` μπορεί να απενεργοποιείται· ο host αποφασίζει αν επιτρέπει AI plugins (μέσω FFI ή capability).

## Minimal runnable example

- Υπάρχει πλέον μικρό example στο `examples/ai_plugin_minimal/`.
- Το `examples/ai_plugin_minimal/plugin/` δείχνει το package πλευρά: εκθέτει σταθερό API (`plugin_info`, `capabilities`, `build_request`, `invoke`) χωρίς να δένει τον VM πυρήνα με συγκεκριμένο AI provider.
- Το `examples/ai_plugin_minimal/app/` δείχνει τον consumer πλευρά: local path dependency σήμερα, αλλά το ίδιο contract μπορεί αργότερα να δημοσιευτεί στο registry.
- Το example είναι σκόπιμα **stub-first**: το package φτιάχνει normalized request envelope και ο πραγματικός host/FFI bridge μπορεί αργότερα να αναλάβει την εκτέλεση.
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

```sapphire
let p = import("examples/ai_http_provider/main.sapp");
let res = p.openai_chat(my_key, "gpt-4o", [map{ role: "user", content: "Hello!" }]);
print res["choices"][0]["message"]["content"];
```

### Limitations

- **Authorization:** `http_post` currently takes `(url, body)` only. For API-key auth the host must inject headers or a future 3-arg `http_post` is required.
- **Streaming:** `streaming_chat` is a stub (calls `on_chunk` once with full result). Will wire to chunked HTTP when W3 streaming exists.

### Extending to other providers

Swap the endpoint in `call_provider` for Anthropic, Mistral, Ollama, etc. No VM changes.

---

## Streaming Inference (A4-alpha)

### SSE token stream (OpenAI-style)

Use `http.post_stream(url, body, on_chunk)` + `http.parse_sse_line(line)`:

```sapphire
import "stdlib:http.sapp";
http.post_stream(url, body, fn(chunk) {
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
- **parse_sse_line(line)** — returns `map{ kind: "data"|"event", value: string }` or nil.

Example: **openai_chat_stream(prompt, api_key, on_token)** in `examples/ai_http_provider/main.sapp` parses SSE data lines and calls on_token(delta) for each content delta. Note: current post_stream does not send Bearer auth; for production use a streaming post_auth variant when available.

### Multi-provider routing (A4-beta)

For simple multi-provider setups, use `stdlib:provider_registry.sapp`:

```sapphire
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

### ONNX FFI path (design note)

For local model inference, `onnx_infer` is currently a stub. The correct path:

1. Build a thin Rust crate `sapphire-onnx` that wraps `ort` (ONNX Runtime bindings) and exposes a C ABI: `sapphire_onnx_load(model_path)`, `sapphire_onnx_run(session, input_json)`.
2. Load via FFI: `let onnx = ffi.load("sapphire_onnx.dll");`
3. Call: `let result = onnx.sapphire_onnx_run(session, json_stringify(inputs));`

---

## Σχετικά

- [Embedding.md](Embedding.md) — πώς ο host παρέχει capabilities και ελέγχει FFI.
- [Registry_MVP.md](Registry_MVP.md) — πώς θα διανέμονται τα πακέτα.
- [Future_plans.md](Future_plans.md) — AI-ready ως interop-first (FFI/plugins/host).
