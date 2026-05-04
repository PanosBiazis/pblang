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

## Σχετικά

- [Embedding.md](Embedding.md) — πώς ο host παρέχει capabilities και ελέγχει FFI.
- [Registry_MVP.md](Registry_MVP.md) — πώς θα διανέμονται τα πακέτα.
- [Future_plans.md](Future_plans.md) — AI-ready ως interop-first (FFI/plugins/host).
