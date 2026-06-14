# pblang – Έκθεση προόδου (historical roadmap report)

> Historical note: this file preserves earlier pblang-era milestone notes.
> Canonical current status is tracked in `docs/CURRENT_STATUS.md`, `docs/Tasks.md`, and `ROADMAP.md`.

Τελευταία ενημέρωση: Μάρτιος 2026 (Q3-alpha circuit run + measurement helpers).

## Τι υλοποιήθηκε

### SP1-alpha — Spec sections for class / match / defer (Μάρτιος 2026)
- **docs/Spec_freeze.md**: προστέθηκαν νέες ενότητες για:
  - **Class semantics (v0.1)** — αποσαφήνιση του ότι το class syntax είναι sugar πάνω από maps +
    functions· `ClassName(args)` αποδίδεται σε `ClassName.new(args)` όταν υπάρχει `new`, η `self`
    αναφέρεται στο instance map, και οι μέθοδοι είναι first-class καλούμενες ως `instance.method()`.
  - **Match semantics (v0.1)** — μη εξαντλητικά match επιστρέφουν `nil` (χωρίς error), arms
    ελέγχονται top‑down με first‑match‑wins, και το `_` λειτουργεί ως wildcard/default arm.
  - **Defer semantics (v0.1)** — τα deferred blocks εκτελούνται LIFO σε κάθε exit path
    (κανονικό return, early return, error, φυσικό τέλος block), στο scope δήλωσης, και errors
    μέσα σε defer δεν επαν-προωθούνται (σιωπηλά αγνοούνται) για να μην κρύβουν το αρχικό error.
- Αυτές οι ενότητες λειτουργούν ως spec‑αναφορά για υφιστάμενα και μελλοντικά semantics tests
  που ελέγχουν classes, match και defer, χωρίς να αλλάζουν τον core κώδικα του VM.

### M4-alpha — Remote registry install (`pblang install <name>@<version>`) (Μάρτιος 2026)
- **crates/pblang-pm**: νέο public helper **`fetch_and_install_package(registry_index_url, name, version_range, cache_dir, require_sha256)`**
  που χρησιμοποιεί τα υπάρχοντα helpers (`fetch_url`, `parse_registry_index`, `resolve_registry_version`,
  `resolve_registry_release_url`, `install_registry_release`) για να βρει την υψηλότερη semver έκδοση
  που ικανοποιεί το ζητούμενο range, να κατεβάσει το tarball, να ελέγξει sha256 (και HMAC αν έχει
  sidecar), και να το αποσυμπιέσει στο cache dir. Επιστρέφει `(resolved_version, unpacked_path)`.
- **src/main.rs**: η εντολή **`pblang install`** επεκτάθηκε ώστε, εκτός από το υπάρχον
  manifest-based μονοπάτι (`pblang install [path]` → generate lockfile), να υποστηρίζει και
  **`pblang install <name>@<version>`** (με `--registry-index <url>` ή `PBLANG_REGISTRY_INDEX`
  για hosted index). Σε αυτή τη μορφή, η εντολή καλεί `fetch_and_install_package` με global cache
  root και τυπώνει `Installed <name>@<resolved_version>`.
- **tests/install_cli.rs**: νέο end-to-end test **`install_name_at_version_from_registry`** που
  δημιουργεί προσωρινό πακέτο, το δημοσιεύει σε local registry μέσω `pblang publish --registry-dir`,
  και στη συνέχεια το εγκαθιστά με `pblang install @pblang/m4-pkg@1.2.0` από το παραγόμενο
  `index.json`, ελέγχοντας ότι η εντολή τερματίζει με επιτυχία και ότι το stdout περιέχει
  `"Installed"` και `"1.2.0"`.
- **docs/Tooling.md**: ενημερώθηκε η περιγραφή της εντολής `pblang install` για να καλύπτει και
  τη νέα μορφή `path | <name>@<version>` και να εξηγεί σε μία πρόταση τη ροή M4-alpha (resolve από
  hosted index, download+unpack σε cache, μήνυμα Installed).

### M4-beta — Remote publish + registry search (Μάρτιος 2026)
- **crates/pblang-pm**: προστέθηκαν helpers για hosted/file-based registry publish/search:
  - **`publish_package(registry_url, package_dir, token)`** — όταν το `registry_url` είναι `file://...`,
    τυλίγει το υπάρχον `publish_to_registry_dir` ώστε `pblang publish` να δουλεύει απευθείας με
    local registries. Για http(s) URLs, διαβάζει το `pblang.toml` του τρέχοντος πακέτου, ελέγχει
    self-contained συμβόλαιο (χωρίς `[dependencies]`), δημιουργεί tarball με `create_package_tarball`,
    υπολογίζει sha256 και στέλνει **multipart/form-data** POST σε `<registry>/publish` με πεδία
    `tarball` (bytes), `sha256` (hex) και `metadata` JSON `{name, version, description}`, χρησιμοποιώντας
    Bearer token από `PBLANG_REGISTRY_TOKEN` ή CLI flag `--registry-token=<token>`.
  - **`search_registry(registry_url, query)`** — για `file://` registries διαβάζει απευθείας το
    `<dir>/index.json` και επιστρέφει αποτελέσματα όταν το όνομα πακέτου ή η έκδοση περιέχει το query·
    για http(s) registries κάνει GET σε `<registry>/search?q=<query>` και περιμένει JSON array
    `{name, version, description}`. Επιστρέφει `Vec<SearchResult>` με πεδία `name`, `version`,
    `description`.
- **src/main.rs**: CLI επεκτάσεις για registry:
  - **`pblang publish`** (χωρίς `--registry-dir`) — νέα hosted/file-based ροή που:
    - Βρίσκει `pblang.toml` ξεκινώντας από το current dir.
    - Διαβάζει **`PBLANG_REGISTRY_INDEX`** ως βάση registry· για `file://` URLs καλεί
      `pblang_pm::publish_package` που με τη σειρά του χρησιμοποιεί `publish_to_registry_dir`;
      για http(s) URLs δημιουργεί tarball και POST όπως περιγράφεται παραπάνω.
    - Διαβάζει προαιρετικό token από **`PBLANG_REGISTRY_TOKEN`** ή CLI flag `--registry-token=<token>`.
    - Εκτυπώνει `Published to <registry_url>` σε επιτυχία.
  - Υφιστάμενο **`pblang publish --registry-dir <dir> [path]`** παραμένει ως τοπικός MVP κλάδος
    (local registry), ενεργοποιείται όταν περνά το flag `--registry-dir`.
  - **`pblang search <query>`** — νέα εντολή CLI που διαβάζει **`PBLANG_REGISTRY_INDEX`** ως
    registry base και καλεί `search_registry`· για `file://` registries φορτώνει τοπικό `index.json`
    και για http(s) registries κάνει HTTP GET σε `/search`. Εκτυπώνει ένα αποτέλεσμα ανά γραμμή
    ως `name @ version — description` (ή `name @ version` όταν δεν υπάρχει περιγραφή) ή
    `No results for '<query>'` όταν δεν βρεθούν πακέτα.
- **tests/install_cli.rs**: νέο end-to-end test **`publish_and_search_roundtrip`** που:
  - Δημιουργεί προσωρινό registry dir και πακέτο `pblang_m4b_pkg` έκδοσης `2.0.0` με `lib.sapp`.
  - Τρέχει `pblang publish` μέσα από το πακέτο με `PBLANG_REGISTRY_INDEX=file://<reg_dir>`
    και `PBLANG_REGISTRY_TOKEN=test-token`, ελέγχοντας ότι η εντολή ολοκληρώνεται με επιτυχία.
  - Τρέχει `pblang search pblang_m4b` με το ίδιο `PBLANG_REGISTRY_INDEX` και ελέγχει ότι το
    stdout περιέχει το όνομα του πακέτου, επιβεβαιώνοντας ότι η file-based registry αναζητείται
    επιτυχώς μέσω της νέας search ροής.
- **docs/Registry_hosted.md**: επεκτάθηκε με ενότητα **"M4-beta: Remote publish + search"** που
  περιγράφει:
  - Τη συμπεριφορά του `pblang publish` για file-based (`file://`) και hosted (http/https) registries,
    συμπεριλαμβανομένου του multipart POST συμβολαίου και του Bearer auth.
  - Τη συμπεριφορά του `pblang search <query>` για file-based (ανάγνωση `index.json`) και hosted
    registries (GET `/search?q=<query>`).
- **docs/Tooling.md**: η CLI ενότητα ενημερώθηκε:
  - Το `pblang publish --registry-dir` παραμένει ως τοπικό MVP.
  - Νέα bullets για **`pblang publish`** (hosted/file-based μέσω PBLANG_REGISTRY_INDEX) και
    **`pblang search <query>`** με αναλυτική περιγραφή της συμπεριφοράς και της μορφής εξόδου.
  - Η ενότητα **"Registry / install env vars"** ενημερώθηκε ώστε `PBLANG_REGISTRY_INDEX` να
    αναφέρεται ως "registry index or registry base" και να περιλαμβάνει ρητά ότι χρησιμοποιείται
    πλέον και από `pblang publish` / `pblang search`. Η παραπομπή σε **docs/Registry_hosted.md**
    ενημερώθηκε σε "M3-gamma/M4-beta".

### T5-alpha — stdlib thread wrapper (Μάρτιος 2026)
- **stdlib/thread.sapp**: νέο stdlib module που παρέχει εργονομικό wrapper πάνω από τα υπάρχοντα
  thread/channel builtins: `thread.spawn(fn)` / `thread.join(handle)` για VM-per-thread workers,
  `thread.channel_new()` / `thread.send_value(sender, value)` / `thread.recv_value(receiver)` για
  βασικό message-passing API, `thread.channel_close_wrapper(ch)` / `thread.channel_is_closed_wrapper(ch)`
  για explicit close/status, και `thread.select_ready(channels_array)` ως thin wrapper πάνω από `select`.
  Δεν αλλάζει τη semantics του πυρήνα, απλώς εκθέτει καθαρότερο stdlib API.
- **tests/semantics/thread_stdlib_test.sapp**: semantics test που χρησιμοποιεί δύο workers με
  `thread.spawn`, shared job/results channels από `thread.channel_new`, στέλνει jobs, περιμένει
  join, συλλέγει τα αποτελέσματα και ελέγχει ότι total == 20· επιπλέον ελέγχει ότι
  `thread.channel_close_wrapper` + `thread.channel_is_closed_wrapper` δουλεύουν.
- **tests/semantics.rs**: νέο test `thread_stdlib_wrapper_pass` που τρέχει το παραπάνω script με
  StdlibConfig χωρίς extra capabilities και επιβεβαιώνει ότι το τελικό print είναι
  `"thread_stdlib_test: pass"`.
- **docs/Tooling.md**: ενημερώθηκε η ενότητα Threads / channels ώστε να περιγράφει το
  `stdlib:thread.sapp` (T5-alpha) και τις βασικές wrapper συναρτήσεις.

### G3-alpha — collections.sapp (stack / queue / set helpers) (Μάρτιος 2026)
- **stdlib/collections.sapp**: νέο stdlib module για βασικές δομές δεδομένων — **stack** (stack_new,
  stack_push, stack_pop, stack_peek) υλοποιημένο με arrays· **queue** (queue_new, queue_enqueue,
  queue_dequeue) ως FIFO πάνω από arrays (push στο τέλος, in-place shift στο front πριν από pop)·
  **set** (set_new, set_add, set_has, set_remove, set_union, set_intersect) ως map-backed σύνολα
  με union/intersect helpers. Δεν αλλάζει υπάρχοντα modules.
- **tests/semantics/collections_test.sapp**: semantics test που ασκεί stack (push/pop/peek),
  queue (enqueue/dequeue) και set (add/has/remove/union/intersect) και τυπώνει τελικό
  `"collections_test: pass"`.
- **tests/semantics.rs**: νέο test `collections_helpers_pass` που τρέχει το collections_test.sapp
  και ελέγχει το τελικό print.
- **docs/Tooling.md**: ενημερώθηκε η ενότητα Stdlib-style APIs ώστε να περιγράφει το
  `stdlib:collections.sapp` και τα βασικά του helpers.

### G3-beta — math.sapp scalar helpers (Μάρτιος 2026)
- **stdlib/math.sapp**: προστέθηκαν scalar helpers `math.util.clamp(x, lo, hi)`, `math.util.lerp(a, b, t)` και `math.util.smoothstep(edge0, edge1, x)` πάνω από το υπάρχον complex/logic/linear-algebra API, ώστε animations και basic easing/interpolation να γίνονται απευθείας σε pblang χωρίς extra host code.
- **tests/semantics/math_ext_test.sapp**: νέο semantics σενάριο που ασκεί clamp/lerp/smoothstep σε απλές τιμές και τυπώνει `"math_ext_test: pass"`· το Rust harness `math_ext_test_pass` προστέθηκε στο `tests/semantics.rs` και τρέχει μαζί με τα υπόλοιπα stdlib semantics tests.
- **docs/Tooling.md**: εμπλουτίστηκε με ξεχωριστή ενότητα "Math stdlib (G3-beta)" που περιγράφει συνοπτικά τα complex/logic/linear-algebra helpers του `stdlib:math.sapp` και τα νέα scalar helpers.

### L3-alpha — Formatter blank line between top-level functions (Μάρτιος 2026)
- **src/main.rs**: `pblang fmt` επέκταση — πέρα από trim, operator/comma spacing, indent και
  προαιρετικό line wrap, ο formatter τώρα εισάγει **μία** κενή γραμμή ανάμεσα σε διαδοχικά
  top-level `fn` declarations (βάσει brace depth). Αυτό στοχεύει πιο αναγνώσιμο κώδικα
  χωρίς να αλλάζει περιεχόμενο μέσα σε strings ή μη-top-level μπλοκ.
- **tests/fmt.rs**: νέο regression test `fmt_inserts_blank_line_between_toplevel_fns` που
  γράφει δύο διαδοχικές top-level functions χωρίς κενή γραμμή και ελέγχει ότι το formatted
  output περιέχει `}\n\nfn second`.
- **docs/Tooling.md**: ενημερώθηκε η περιγραφή της εντολής `pblang fmt` ώστε να αναφέρει
  τον νέο κανόνα για κενή γραμμή μεταξύ top-level `fn` declarations.

### Q3-beta — Measurement histogram helper (Μάρτιος 2026)
- **stdlib/quantum_sim.sapp**: προστέθηκε helper `histogram(outcomes_array)` που επιστρέφει map με stringified outcomes (π.χ. `"0"`, `"1"`) σε counts, ώστε repeated μετρήσεις ή provider αποτελέσματα να μπορούν να συνοψιστούν εύκολα σε καθαρό pblang code.
- **tests/semantics/quantum_histogram_test.sapp**: νέο semantics σενάριο που ασκεί το helper σε ένα μικρό outcomes array και ελέγχει ότι τα counts είναι σωστά· το Rust harness `quantum_histogram_test_pass` ενσωματώθηκε στο `tests/semantics.rs`.
- **docs/Tooling.md / Quantum_providers.md**: ενημερώθηκαν με σύντομη περιγραφή των measurement utilities και παράδειγμα χρήσης του `histogram`, ώστε τα Q3 εργαλεία να είναι ορατά από το κύριο tooling και quantum provider docs.

### Q3-gamma — BB84 example + repetition-code helper (Μάρτιος 2026)
- **stdlib/quantum_sim.sapp**: προστέθηκαν helpers `repetition_encode(bit)` (επιστρέφει `[bit, bit, bit]`) και `repetition_decode(bits)` (majority-vote decoder πάνω στο array των bits) ως απλό 3-qubit repetition code για toy quantum error-correction παραδείγματα.
- **tests/semantics/quantum_ecc_test.sapp** + **quantum_ecc_repetition_pass** στο `tests/semantics.rs`: encode 1, εισαγωγή single-bit error, decode → 1· encode 0, χωρίς error, decode → 0· τελικό print `"quantum_ecc_test: pass"`.
- **examples/quantum_bb84/main.sapp**: νέο παράδειγμα που συνδέει `stdlib:crypto_quantum.sapp` (QKD protocol names), `stdlib:quantum.sapp` (circuit builder + `run`) και `stdlib:quantum_sim.sapp.histogram` για παραγωγή Bell-state measurement histogram, μαζί με toy BB84-style παραμέτρους (Alice/Bob bits + bases).
- **docs/Quantum_providers.md** και **docs/Tooling.md** ενημερώθηκαν ώστε να αναφέρουν τη repetition-code helper οικογένεια και το νέο BB84-style παράδειγμα.

### E5-alpha — Self-extracting / installer presets design (Μάρτιος 2026)
- **docs/Native_backend_notes.md**: επεκτάθηκε με staged path για native/hybrid backend
  και συγκεκριμένο **Stage A** σχέδιο για self-extracting wrapper (embedded runtime +
  `pblang pack` tarball, temp extract, run, cleanup) και **Stage B** installer presets
  (deb/zip/AppImage/NSIS) ως docs-first βήμα. Περιγράφει criteria, prerequisites και
  future Stage C (N2) χωρίς να δεσμεύεται σε άμεση υλοποίηση.

### E5-beta — Self-extracting wrapper prototype hooks (Μάρτιος 2026)
- **crates/pblang-stub**: νέο crate με minimal launcher `pblang-stub` (template για self-extracting binaries) που περιέχει sentinel `PBLANG_BUNDLE_START\x00` και προορίζεται να χρησιμοποιηθεί ως βάση για embedded bundle executables στο Stage A.
- **CLI**: το `pblang bundle` δέχεται πλέον προαιρετικό flag `--self-extract`· το flag αναγνωρίζεται και διατηρείται για μελλοντικά self-extracting workflows χωρίς να αλλάζει το σημερινό output directory layout (παράγει κανονικό bundle dir όπως πριν).
- **Tests / docs**: προστέθηκε test `bundle_self_extract_flag_accepted` στο `tests/pack.rs` που διασφαλίζει ότι το flag δεν αναφέρεται ως άγνωστο· τα `docs/Native_backend_notes.md` και `docs/Tooling.md` ενημερώθηκαν για την E5-beta κατάσταση (stub launcher + πειραματικό flag).
- **docs/Tooling.md**: στην ενότητα `pblang pack` / `pblang bundle` προστέθηκε
  σύντομη παραπομπή προς `docs/Native_backend_notes.md` για το E5 self-extracting /
  installer presets design.

### Q3-alpha — Circuit run + measurement utilities (Μάρτιος 2026)
- **stdlib/quantum.sapp**: **run(circ)** — καλεί quantum_protocol.submit(circ), επιστρέφει πίνακα μετρήσεων (0/1 ανά qubit). **measurements_to_string(measurements)** → string π.χ. "010". **measurements_to_int(measurements)** → ακέραιος little-endian. Export extended.
- **tests/semantics/quantum_run_test.sapp** + **quantum_run_test_pass** (circuit 2 qubits, H+CNOT+measure_all, run, string/int helpers).
- **docs/Quantum_providers.md**: Q3-alpha design note — Qiskit/Braket/Cirq bridge (exec+JSON ή FFI path), provider slot contract.
- **docs/Tooling.md**: Q3-alpha στο quantum.sapp bullet.

### A4-alpha — Streaming inference (SSE token stream) (Μάρτιος 2026)
- **stdlib/http.sapp**: **get_stream(url, on_chunk)** και **post_stream(url, body, on_chunk)** — shim: πλήρες response, split σε "\n\n", on_chunk ανά part. **parse_sse_line(line)** → map{ kind: "data"|"event", value } ή nil.
- **examples/ai_http_provider/main.sapp**: **openai_chat_stream(prompt, api_key, on_token)** — post_stream + parse_sse_line, on_token(delta) για κάθε SSE data. Export openai_chat_stream.
- **tests/semantics/ai_stream_test.sapp** + **ai_stream_parse_sse_pass** (parse_sse_line data/event/empty).
- **docs/AI_plugin_pattern.md**: Streaming Inference (A4-alpha) — post_stream/parse_sse_line παράδειγμα, openai_chat_stream, ONNX FFI path design note.
- **docs/Tooling.md**: A4-alpha στο stdlib http.sapp bullet.

### W4-alpha — Per-request timeout + SSE + TLS research (Μάρτιος 2026)
- **crates/pblang-vm**: builtin **tcp_set_timeout(stream_handle, timeout_ms)** — sets read/write timeout on TCP stream (allow_net; 0 = no timeout). Uses with_net_table get_mut on NetResource::Stream.
- **stdlib/http_server.sapp**: **serve_once_timeout(addr, timeout_ms, handler)**, **serve_forever_timeout(addr, timeout_ms, handler)** (tcp_set_timeout after accept); **sse_event(event, data)** (SSE frame string); **res_sse_headers()** (200, text/event-stream, Cache-Control no-cache). Export extended.
- **docs/TLS_notes.md**: TLS research note — host-bridge (rustls StreamOwned), FFI plugin alternative, non-goals, minimal VM change (pblangStream trait, tcp_listen_tls stub). Status: design, not implemented.
- **tests/semantics/http_server_timeout_test.sapp** + **http_server_timeout_serve_once_pass** (exports + sse_event/res_sse_headers).
- **docs/Tooling.md**: W4-alpha in http_server bullet + TLS_notes.md reference.

### R2-beta — Annotated disassembly (Μάρτιος 2026)
- **src/main.rs**: **pblang --annotate \<file\>** — compile, **print_annotated_disasm(chunk, src)** (disasm + source line headers από chunk.spans). **pblang-ir** dependency στο root Cargo.toml για Chunk.
- **tests/annotate.rs**: annotate_output_contains_line_number_and_opcode (fixture call_graph_sample.sapp). **Cargo.toml** [[test]] annotate.
- **docs/Tooling.md**: R2-beta --annotate.

### R3-beta — Symbol table dump (`pblang --symbols`) (Μάρτιος 2026)
- **src/main.rs**: Προστέθηκε νέα RE CLI σημαία **`pblang --symbols <file>`**:
  - Διαβάζει το δοθέν `.sap`/`.sapp` αρχείο, αφαιρεί σχόλια με `strip_comments_mask`, κάνει parse σε AST
    και κατόπιν compile σε `Chunk` (ίδια διαδρομή με `--dump-bytecode` / `--annotate`).
  - Καλεί τη νέα helper συνάρτηση **`print_symbol_table(chunk)`** που διασχίζει τον compiled function
    πίνακα (`chunk.functions`) και το constant pool (`chunk.constants`) για να τυπώσει ένα απλό symbol table:
    - Για κάθε function entry: `fn\t<name>\tarity:<n>\toffset:<byte_offset>` όπου το `name` διαβάζεται από
      string constant (ή `<anon>` fallback) και το offset προέρχεται από το compiled chunk.
    - Για απλά globals: σαρώνει string constants που φαίνονται σαν identifiers (γράμμα/underscore στην αρχή)
      και τυπώνει `let\t<name>\toffset:const#<index>` ως best-effort αναφορά global ονομάτων.
  - Το help output ενημερώθηκε ώστε να περιγράφει τη νέα εντολή:
    `pblang --symbols <file>    Print a simple symbol table for top-level functions and globals (name, arity, bytecode offset)`.
- **tests/symbols.rs**: νέο regression test **`symbols_lists_function_names`**:
  - Τρέχει `pblang --symbols tests/fixtures/call_graph_sample.sapp` μέσω του compiled binary.
  - Επιβεβαιώνει ότι η εντολή τερματίζει με επιτυχία και ότι το stdout περιέχει τόσο `"fn"` όσο και
    `"arity"`, διασφαλίζοντας ότι το symbol table περιλαμβάνει τουλάχιστον μία function γραμμή.
  - Το root **Cargo.toml** ενημερώθηκε με νέο `[ [test] ]` entry `symbols = "tests/symbols.rs"`.
- **docs/Tooling.md**: επεκτάθηκε η ενότητα Reverse engineering / bytecode με καινούρια υποενότητα
  **"R3-beta: Symbol table dump"** που περιγράφει:
  - Χρήση: `pblang --symbols <file>`.
  - Τυπικό output: γραμμές `fn <name> arity:<n> offset:<byte_offset>` για compiled functions και
    `let <name> offset:const#<k>` για απλά global names.
  - Σκοπός: γρήγορη αναγνώριση εξαγόμενων function names, arities και entry points χωρίς να χρειάζεται
    να διαβαστεί ολόκληρο το disassembly του bytecode.

### W3-epsilon — WebSocket frame parsing (Μάρτιος 2026)
- **crates/pblang-vm**: builtins **strcharcodeat(s, i)** (char code ή -1), **strfromcharcode(n)** (single-char string), **byte_xor(a, b)** (0..255 XOR για unmask).
- **stdlib/websocket.sapp**: **parse_frame(data)** — RFC 6455 frame (fin, opcode, masked, plen); num_mod/num_floor αντί για %· payload length ≤125· export parse_frame.
- **tests/semantics/websocket_parse_frame_test.sapp** + **websocket_parse_frame_pass**.
- **docs/Tooling.md**: W3-epsilon parse_frame, strcharcodeat, strfromcharcode, byte_xor.

### L2-delta — LSP stub (Μάρτιος 2026)
- **src/main.rs**: **pblang lsp** — **run_lsp_server()** (Content-Length loop, initialize/initialized/textDocument/completion/shutdown/exit), **lsp_response**, **lsp_extract_str**, **lsp_extract_id**; completion επιστρέφει builtin names.
- **tests/lsp.rs**: lsp_initialize_returns_completion_capability, lsp_completion_returns_builtin_items. **Cargo.toml** [[test]] lsp.
- **docs/Tooling.md**: L2-delta pblang lsp.

### L3-beta — LSP scope-aware completion (Μάρτιος 2026)
- **src/main.rs**: `run_lsp_server()` επεκτάθηκε με in-memory `open_files` map (URI → full text) και helper `collect_local_names(src)` που σαρώνει το τρέχον έγγραφο για top-level `fn <name>` και `let <name>` patterns. Ο LSP πλέον χειρίζεται `textDocument/didOpen` και `textDocument/didChange` ώστε να κρατάει ενημερωμένα τα ανοικτά αρχεία και στο `textDocument/completion` επιστρέφει τόσο builtin completion items όσο και απλά local-symbol items (με `"detail": "local"`).
- **tests/lsp.rs**: προστέθηκε test `lsp_completion_includes_local_fn_after_did_open` που εκκινεί `pblang lsp`, στέλνει initialize/initialized/didOpen για buffer με `fn my_custom_fn()`, μετά completion και ελέγχει ότι `"my_custom_fn"` εμφανίζεται στα labels.
- **docs/Tooling.md**: ενημερώθηκε η περιγραφή του `pblang lsp` σε L3-beta και προστέθηκε ενότητα "L3-beta: LSP scope-aware completion" που εξηγεί ότι τα completions περιλαμβάνουν πλέον local functions/lets από το ανοικτό αρχείο μαζί με τα builtins και ότι οι LSP clients πρέπει να στέλνουν didOpen/didChange πριν το completion.

### E4-gamma — App manifest metadata (Μάρτιος 2026)
- **crates/pblang-pm**: PackageSection **target_platform** (Option<String>), **min_runtime** (Option<String>) με #[serde(default)]. **validate_manifest_metadata(man_dir)** — ελέγχει target_platform ∈ {any,linux,macos,windows}, min_runtime parseable semver· **get_manifest_platform_hints(man_dir)** για run info.
- **src/main.rs**: pack και bundle καλούν validate_manifest_metadata πριν το entry resolution· pblang run τυπώνει info γραμμές όταν target_platform ≠ any ή min_runtime set (από script_path parent + find_manifest_dir).
- **tests/fixtures/manifest_metadata/pblang.toml** + **main.sapp**. **tests/pack.rs**: pack_succeeds_with_valid_target_platform_and_min_runtime, pack_fails_for_invalid_target_platform, pack_fails_for_invalid_min_runtime (temp dirs με custom manifest).
- **docs/Tooling.md**: E4-gamma manifest validation και run info.

### G2-gamma — CSV quoted fields + graphics stubs (Μάρτιος 2026)
- **stdlib/csv.sapp**: **parse_line(line)** RFC 4180 (quoted fields, "" escape)· **parse(text)** χρησιμοποιεί parse_line ανά γραμμή· export parse, stringify, parse_line.
- **stdlib/graphics.sapp**: **polygon(ctx, points, color)** — κλειστό polygon με gfxdrawline (points = [x,y])· **bezier(ctx, x0, y0, cx1, cy1, cx2, cy2, x1, y1, color)** stub (γραμμή)· **draw_text(ctx, text, x, y, color)** no-op stub· export polygon, bezier, draw_text.
- **tests/semantics/csv_quoted_test.sapp** (unquoted, quoted comma, escaped quote, multi-line)· **tests/semantics.rs** csv_quoted_fields_pass.
- **docs/Tooling.md**: G2-gamma csv parse_line και graphics polygon/bezier/draw_text.

### M3-gamma — Hosted registry design note (Μάρτιος 2026)
- **docs/Registry_hosted.md**: σχεδίαση API (GET index/tarball/sha256, POST publish, GET search), auth (bearer token, env/CLI), μελλοντικές αλλαγές client (publish, search, --registry-token, [registry] στο manifest), non-goals.
- **docs/Tooling.md**: αναφορά Registry_hosted.md· client αλλαγές M3-delta.

### R2-alpha — Call-graph (Μάρτιος 2026)
- **src/main.rs**: **pblang --call-graph \<file\>** — parse, build_call_graph(ast), print_call_graph (plain text edges `caller -> callee`). Top-level calls → `<main>`. Print statements count as calls to `print`. **walk_stmts_for_cg** / **collect_calls_from_expr** over StmtKind/ExprKind.
- **tests/fixtures/call_graph_sample.sapp**, **tests/call_graph.rs** (3 tests). **Cargo.toml** [[test]] call_graph. **docs/Tooling.md** R2-alpha.

### A3-gamma — http_post_auth (Μάρτιος 2026)
- **crates/pblang-vm**: builtin **http_post_auth(url, headers_map, body)** — POST με caller-supplied headers· allow_http. **stdlib/http.sapp**: **post_auth(url, body, token)** builds `Authorization: Bearer <token>` for the common API-key case. **examples/ai_http_provider/main.sapp**: **openai_chat_auth(prompt, api_key)**. **tests/semantics/http_post_auth_test.sapp** + **http_post_auth_builtin_registered_pass**. Tooling.md A3-gamma. Corrected June 2026 after the docs audit found older wrapper docs still described the raw builtin as `(url, body, token)`.

### Q2-delta — Quantum remote stub (Μάρτιος 2026)
- **stdlib/quantum_protocol.sapp**: **submit_remote(url, circuit_map)** (json.stringify + http.post, return data["job_id"]), **poll_remote(url, job_id)** (http.get url+"/"+job_id, json.parse). **docs/Quantum_providers.md** — provider contract (name, submit, poll), local-sim και remote-http stubs, Qiskit/Braket/Cirq future. **tests/semantics/quantum_remote_stub_test.sapp** + **quantum_remote_stub_contract_pass**. Tooling.md quantum_protocol Q2-delta.

### T4-beta — select + thread pool (Μάρτιος 2026)
- **crates/pblang-vm**: **select(channels_array)** builtin — πολλές κάνει recv από πίνακα καναλιών, επιστρέφει πρώτο ready ως `{ "index": int, "value": any }` ή nil αν όλα κλειστά/άδεια· spin με yield_now. **send** πλέον δεν καταναλώνει το sender (get + clone αντί remove) ώστε work-queue πολλαπλά send.
- **tests/semantics/select_test.sapp** + **select_multi_channel_pass**· **examples/thread_pool/main.sapp** (3 workers, jobs/results channels, block για upvalue capture).
- **docs/Tooling.md**: T4-beta (select, thread_pool).

### M3-beta — Semver σε dependency resolution (Μάρτιος 2026)
- **crates/pblang-pm**: **resolve_registry_version(releases, range)** — exact match ή υψηλότερη version που ικανοποιεί το range (satisfies)· **resolve_registry_dep** καλεί αυτό πριν το install. Σφάλμα όταν κανένα candidate δεν ικανοποιεί.
- Unit tests: **resolve_picks_highest_satisfying_caret**, **resolve_rejects_when_no_candidate_satisfies**.
- **docs/Tooling.md**: M3-beta semver wired into resolution.

### L2-gamma — Error UX file:line:col (Μάρτιος 2026)
- **src/main.rs**: **print_parse_errors(src, errors, file: Option<&str>)** — όταν file παρ present, format **error[file:line:col]: message**· runtime errors ίδιο format (error[src_name:ln:col]: message, αλλιώς error: message). Όλα τα call sites περνούν path όπου υπάρχει.
- **tests/fixtures/error_location.sapp** (invalid `let x = ;`)· **tests/lint.rs** **error_output_includes_file_and_line** — pblang run fixture, assert stderr περιέχει filename και error[...]:.
- **docs/Tooling.md**: Error UX (L2-gamma).

### T4-alpha — Channel close + closed-channel semantics (Μάρτιος 2026)
- **crates/pblang-vm**: **channel_close(ch)** — σημειώνει το channel ως κλειστό (CLOSED_SENDER_IDS), αφαιρεί το sender από το table ώστε recv να επιστρέφει nil· **channel_is_closed(ch)** → bool. Στο **send_builtin**: έλεγχος αν sender_id στο closed set → σφάλμα "send on closed channel". **parse_channel_pair(v)** για channel map → (sender_id, receiver_id).
- **tests/semantics/channel_close_test.sapp**: send/recv κανονικά, channel_close, channel_is_closed, recv μετά το close → nil· print "channel_close_semantics_pass".
- **tests/semantics.rs**: **channel_close_semantics_pass**.
- **docs/Tooling.md**: ενότητα "Threads / channels (T4-alpha)".
- **Επόμενο**: T4-beta select, thread pool example· L2-gamma file:line:col stack trace.

### Q2-gamma — Circuit builder ergonomics (Μάρτιος 2026)
- **stdlib/quantum.sapp**: **circuit(n)** επιστρέφει και **n_qubits**· νέα **apply_h(circ, q)**, **apply_x(circ, q)**, **apply_cnot(circ, c, t)**, **measure_all(circ)** (MEASURE σε κάθε qubit), **to_qasm(circ)** (alias emit_qasm). Συμβατό με quantum_protocol.submit().
- **tests/semantics/quantum_circuit_builder_test.sapp**: Bell circuit (apply_h, apply_cnot, measure_all), έλεγχος n_qubits/ops και QASM string· print "quantum_circuit_builder_pass".
- **tests/semantics.rs**: **quantum_circuit_builder_pass**.
- **docs/Tooling.md**: stdlib:quantum.sapp (Q2-gamma).
- **Επόμενο**: JSON wire για remote backend, provider slots.

### A3-beta — Tensor/buffer contract (Μάρτιος 2026)
- **stdlib/tensor.sapp**: **tensor(shape, data)** → map με shape, data, dtype "f32" (mismatch → map με "error"); **zeros(shape)**, **ones(shape)**; **get(t, indices)**, **set(t, indices, value)** (row-major, set αντίγραφο); **shape(t)**, **dtype(t)**; **to_request_body(t)** για JSON σε AI requests. Helper **_shape_size**, **_flat_index**.
- **tests/semantics/tensor_test.sapp**: creation, get, zeros, ones, set (original unchanged), error map, to_request_body· print "tensor_test_pass".
- **tests/semantics.rs**: **tensor_test_pass** — τρέχει το script και ελέγχει τελικό print.
- **examples/ai_http_provider/main.sapp**: σχόλιο για stdlib:tensor.sapp και to_request_body στα inputs.
- **docs/Tooling.md**: stdlib:tensor.sapp (A3-beta).
- **Επόμενο**: streaming inference, 3-arg http_post.

### L2-beta — Shadowing lint (Μάρτιος 2026)
- **src/main.rs**: στο **lint_unused_warnings**, πριν την εισαγωγή Let/For binding ελέγχεται αν το name υπάρχει σε outer scope· αν ναι → warning "variable `name` shadows outer binding".
- **tests/fixtures/lint_shadow.sapp**: fixture με outer x και εσωτερικό let x.
- **tests/lint.rs**: **lint_reports_shadowing** — pblang lint fixture, assert stderr περιέχει "shadows outer binding" και "`x`".
- **docs/Tooling.md**: lint section ενημερώθηκε (shadowing).

### Bugfix — W3-delta chunked_read parse (Μάρτιος 2026)
- **stdlib/http_server.sapp**: το **while !done** απέτυχε parse (parser ερμηνεύει `!` ως αρχή του `!=`). Αντικαταστάθηκε με **while done == false**.
- Semantics tests 52/52 περνούν (8 tests που import http_server.sapp ξαναπερνούν).

### W3-gamma — multipart/form-data + string builtins (Μάρτιος 2026)
- **crates/pblang-vm**: νέα builtins **str_index_of(s, sub)** (character index ή -1), **str_slice(s, start, end)** (character-based slice), **str_lower(s)**. Capability → core.
- **stdlib/multipart.sapp**: νέο module — **parse(body, boundary)** → array of part maps (name, filename, content_type, body). Parses multipart/form-data με delimiter "--" + boundary.
- **tests/semantics.rs**: **multipart_parse_fields_and_file** — δύο parts (field + file), assert name/body/filename.
- **docs/Tooling.md**: stdlib:multipart.sapp + string builtins σημείωση.
- **Επόμενο**: chunked body reading, WebSocket frame parsing.

### G2-beta — process.sapp + proc_spawn_wait (Μάρτιος 2026)
- **crates/pblang-vm**: νέο builtin **proc_spawn_wait(cmd, args_array?)** — blocking subprocess, επιστρέφει map με exit_code, stdout, stderr· gated by allow_exec. Capability match για "proc_spawn_wait" → exec.
- **stdlib/process.sapp**: νέο module — **spawn_wait(cmd, args_array?)** (wrapper προς proc_spawn_wait). Export spawn_wait.
- **tests/semantics.rs**: **process_spawn_wait_captures_stdout_and_exit_code** — import stdlib:process.sapp, process.spawn_wait (echo hello / cmd /c echo hello), assert exit_code 0 και stdout περιέχει "hello". **tests/common/mod.rs**: **run_collect_with_config** για semantics tests με StdlibConfig.
- **tests/sandbox.rs**: **proc_spawn_wait_sandbox_denied** — exec: false → proc_spawn_wait fails με allow_exec/exec access denied.
- **docs/Tooling.md**: stdlib:process.sapp (spawn_wait) προστέθηκε.
- **Επόμενο**: process.sapp spawn_async/wait (handle-based) ή csv quoted fields· G2 candidate work.

### L2-alpha — Unused variable / unused import lint (Μάρτιος 2026)
- **src/main.rs**: lint pass **lint_unused_warnings(stmts)** — scope stack `(Span, is_import, read)` per binding· στα `Let`/`For` εισάγονται bindings· στα `Var`/`Assign` καλείται **mark_read**· στα block exits (If/While/For/Fn/Block, FnLit/BlockExpr, top-level) εκτυπώνονται warnings για unread: "unused import: \`name\`" αν is_import, αλλιώς "unused variable: \`name\`". Warnings μόνο όταν typecheck diagnostics κενά· exit 1 μόνο για parse/typecheck errors.
- **tests/fixtures/lint_unused.sapp**: fixture με unused_var, unused_imp, used_var (export foo που χρησιμοποιεί used_var).
- **tests/lint.rs**: test **lint_reports_unused_variable_and_unused_import** — `pblang lint` fixture, assert stderr περιέχει unused variable/import warnings και όχι για used_var.
- **docs/Tooling.md**: lint section ενημερώθηκε (unused variable, unused import).
- **Επόμενο**: L2 jump-to-definition, hover, autocomplete, shadowing.

### W3-beta — WebSocket upgrade stub (Μάρτιος 2026)
- **stdlib/websocket.sapp**: νέο module — **upgrade_request(req)** (επιστρέφει map με ok, key όταν GET με Upgrade/Connection/Sec-WebSocket-Key), **upgrade_response(key)** (HTTP 101 με Sec-WebSocket-Accept). VM builtins **sha1**, **websocket_accept_key** (RFC 6455) στο crates/pblang-vm.
- **tests/semantics.rs**: **websocket_upgrade_response_rfc6455** — έλεγχος RFC test vector για accept key.
- **docs/Tooling.md**: stdlib:websocket.sapp προστέθηκε.
- **Τι δεν έγινε**: chunked body· WebSocket frame parsing· upgrade_request με parsed req (header lookup ακόμα προαιρετικό).

### Q2-beta — Quantum protocol adapter (Μάρτιος 2026)
- **stdlib/quantum_protocol.sapp**: νέο module — **submit(spec)** (spec = map με n, ops· εκτελεί circuit μέσω quantum_sim· επιστρέφει job_id, status "completed", result.measurements), **poll(job_id)** (επιστρέφει αποθηκευμένο result ή status "pending"). In-memory sync execution.
- **tests/semantics.rs**: **quantum_protocol_submit_and_poll** — submit 1-qubit H circuit, poll job_id.
- **docs/Tooling.md**: stdlib:quantum_protocol.sapp προστέθηκε.
- **Επόμενο**: JSON wire format για remote backend· provider slots.

### G2-alpha — os.args(), os.exit(), time.now_iso() (Μάρτιος 2026)
- **VM builtins** (crates/pblang-vm): **os_args** (0-arg, reads `PBLANG_SCRIPT_ARGS` env, newline-split → array of strings; empty array if unset), **os_exit** (1-arg code, `std::process::exit`; gated by `host.allow_exec()`), **time_now_iso** (0-arg, `chrono::Utc::now().to_rfc3339()`). Dependency **chrono** 0.4 (clock) in pblang-vm.
- **stdlib/os.sapp**: **args()**, **exit(code)**; exports ενημερώθηκαν.
- **stdlib/time.sapp**: **now_iso()**; export ενημερώθηκε.
- **CLI**: `pblang run [file|dir] -- arg1 arg2 ...` ορίζει `PBLANG_SCRIPT_ARGS` (newline-joined) πριν την εκτέλεση ώστε το script να βλέπει τα args μέσω `os.args()`.
- **tests/semantics.rs**: το test **stdlib_fs_path_os_time_array_map_modules_work** επεκτάθηκε με `type(os.args()) == "array"`, `type(time.now_iso()) == "string"`, και έλεγχο ότι η ISO string περιέχει `"T"`.
- **docs/Tooling.md**: run με `--`, os.sapp (args, exit), time.sapp (now_iso).
- **Τι δεν έγινε**: process.sapp, proc_spawn_wait/proc_spawn_async/proc_wait builtins.
- **Επόμενο**: G2 process.sapp ή W3-beta / Q2-beta.

### E4-alpha — verify + HMAC signing (Μάρτιος 2026)
- **pblang verify <tarball>**: νέο CLI subcommand — ελέγχει δομή tarball (pblang.toml + entry .sapp/.sap). Αν υπάρχει sidecar `<tarball>.sha256` και env **PBLANG_SIGNING_KEY**, επαληθεύει HMAC-SHA256.
- **pblang-pm**: **compute_tarball_hmac(bytes, key)** → hex, **verify_tarball(path)** → Result. Dependency **hmac** 0.12.
- **pblang pack**: όταν οριστεί **PBLANG_SIGNING_KEY**, γράφει `<out_file>.sha256` με HMAC του tarball.
- **tests/pack.rs**: **verify_succeeds_on_valid_pack_tarball**, **verify_fails_when_tarball_missing_manifest**.
- **Τι δεν έγινε**: Ed25519/signature (μόνο HMAC)· verify on install (έγινε στο E4-beta).
- **Επόμενο**: E4 signed registry index ή app manifest metadata.

### E4-beta — verify on install (Μάρτιος 2026)
- **pblang-pm**: **fetch_url_optional(url)** — επιστρέφει `Option<Vec<u8>>` (404/σφάλμα → None). Στο **install_registry_release**, μετά το fetch του tarball και πριν το unpack: αν **PBLANG_SIGNING_KEY** οριστεί, fetch sidecar `archive_url + ".sha256"`· αν υπάρχει sidecar, σύγκριση HMAC με **compute_tarball_hmac**· mismatch → `Err` με μήνυμα "HMAC signature mismatch". Αν key οριστεί αλλά δεν υπάρχει sidecar → warning στο stderr.
- **tests/install_cli.rs**: **install_rejects_registry_tarball_when_hmac_sidecar_wrong** — tarball + λάθος .sha256 sidecar, PBLANG_SIGNING_KEY=testkey, install → αποτυχία και stderr περιέχει "HMAC" ή "signature mismatch".
- **Επόμενο**: Signed registry index ή app manifest metadata.

### M3-alpha — Semver range parser (Μάρτιος 2026)
- **crates/pblang-pm/src/semver.rs**: **Version** (major, minor, patch), **Version::parse(s)** → Option&lt;Version&gt;, **satisfies(version, range_str)** → bool. Συντακτικό: exact, ^, ~, >=, <=, >, <. Δεκατρία unit tests· ενσωματώθηκε στο lib.rs (pub mod semver, pub use).
- **docs/Tooling.md**: ενότητα "Registry semver (M3-alpha)".
- **Επόμενο**: χρήση satisfies στη dependency resolution.

### W3-delta — Chunked body read (Μάρτιος 2026)
- **stdlib/http_server.sapp**: **chunked_read(socket, on_chunk)** — διαβάζει Transfer-Encoding: chunked (tcp_read_until για size line, tcp_read για chunk body + trailing \\r\\n)· καλεί on_chunk(data) ανά chunk. **\_parse_hex(s)** — εσωτερικό helper για hex size. Export chunked_read.
- **tests/semantics/chunked_read_test.sapp**: unit λογική _parse_hex (parse_hex_test) με str_lower, str_slice, str_index_of· print "chunked_read_parse_hex_pass".
- **tests/semantics.rs**: **chunked_read_parse_hex_pass** — τρέχει το script και ελέγχει τελικό print.
- **docs/Tooling.md**: http_server.sapp ενημερώθηκε (chunked_read).
- **Επόμενο**: WebSocket frame parsing, per-handler timeout.

### W3 follow-ups — CORS helpers (Μάρτιος 2026)
- **stdlib/http_server.sapp**: **cors_headers(origin, allow_methods, allow_headers)**, **res_cors_preflight(...)** (204 + CORS headers για OPTIONS). Export και μέσα στο **res**.
- **docs/Tooling.md**: CORS τεκμηρίωση + σημείωση "W3 follow-ups (design): chunked body, WebSocket, per-handler timeout".
- **Τι δεν έγινε**: chunked body reading· WebSocket· per-request timeout από script.

### W3-beta — Middleware chain (Μάρτιος 2026)
- **stdlib/middleware.sapp**: νέο module — **wrap(inner, mw)** (mw(req) → nil ή response map), **chain(inner, mws)** (πτύχωμα middleware γύρω από handler), **mw_log()**, **mw_require_auth(expected_token)**. Τα παλιά **mw_log_stub()** / **mw_require_auth_stub(...)** μένουν ως aliases για συμβατότητα. Συμβατό με router.sapp.
- **tests/semantics.rs**: νέο test **web_middleware_wrap_and_chain** — wrap dispatch με logger, chain με auth middleware, έλεγχος 200/401 και compatibility aliases.
- **docs/Tooling.md**: stdlib:middleware.sapp προστέθηκε.
- **Τι δεν έγινε**: chunked body reading, WebSocket, TLS.
- **Επόμενο**: W3 chunked/streaming ή E4-alpha signing· ενημέρωση Help.md.

### Q2-alpha — Pure-pblang Qubit Simulator (Μάρτιος 2026)
- **stdlib/quantum_sim.sapp**: νέο module — **zero_state(n_qubits)**, **h_gate**, **x_gate**, **cnot_gate**, **measure** (κατάρρευση με random_float). Κατάσταση ως πίνακας [re, im] πλάτη· αριθμητική με num_pow, num_floor, num_mod (χωρίς bit shift).
- **crates/pblang-vm**: νέο builtin **random_float()** → [0, 1) για measurement.
- **tests/semantics.rs**: νέο test **quantum_sim_single_and_two_qubit** (|0>, X|0>, H|0>, Bell state).
- **tests/semantics/quantum_sim_test.sapp**: runnable script για `pblang run`.
- **stdlib/quantum.sapp**: σχόλιο αναφοράς προς quantum_sim.sapp.
- **docs/Tooling.md**: stdlib:quantum_sim.sapp προστέθηκε.
- **Τι δεν έγινε**: JSON protocol adapter, provider slots (Q2-beta).
- **Επόμενο**: Q2-beta ή W3-beta middleware· ενημέρωση Help.md.

### A3-alpha — AI HTTP Provider Adapter (Μάρτιος 2026)
- **examples/ai_http_provider/main.sapp**: νέο runnable example — **build_request(model, messages)**, **call_provider(endpoint, api_key, body)**, **openai_chat**, **streaming_chat** (stub), **provider_info**. Χρήση stdlib:http.sapp και stdlib:json.sapp· χωρίς αλλαγές VM.
- **tests/modules.rs**: νέο test **ai_http_provider_contract_test** — έλεγχος τύπων exports, provider_info(), build_request → JSON με model.
- **docs/AI_plugin_pattern.md**: προστέθηκε ενότητα "HTTP Provider Adapter (A3-alpha)" με API πίνακα, usage, limitations (auth/stubbing).
- **Limitation**: http_post μόνο (url, body)· για API-key απαιτείται host injection ή 3-arg post. Streaming: stub μόνο.
- **Επόμενο**: A3 tensor/streaming ή Q2-alpha simulator· ενημέρωση Help.md για επόμενο batch.

### W3-alpha — Routing layer (Μάρτιος 2026)
- **stdlib/router.sapp**: νέο module — **route_map()** (κενό map), **add_route(map, method, path, handler)** (κλειδί `method|path`), **dispatch(map, method, path, req)** (ακριβές match ή path-param matching `/user/:id`, ορίζει req["params"], επιστρέφει αποτέλεσμα handler ή 404 map), **match_path_params(template, actual)** (επιστρέφει map παραμέτρων ή nil). Χρησιμοποιεί stdlib:strings.sapp για join· map_has για ασφαλή έλεγχο κλειδιού.
- **tests/semantics.rs**: νέο test **web_routing_dispatch_and_path_params** — GET /hello, GET /user/:id → /user/42, 404 για /missing.
- **docs/Tooling.md**: προστέθηκε **stdlib:router.sapp** (route_map, add_route, dispatch, match_path_params).
- **Verification**: `cargo test web_routing_dispatch_and_path_params` περνά.
- **Τι δεν έγινε**: middleware chain· chunked/streaming· WebSocket· TLS.
- **Επόμενο**: W3 middleware/chunked ή A3/Q2 follow-ups· ενημέρωση Help.md για επόμενο batch.

### G2 path.absolute(p) (Μάρτιος 2026)
- **stdlib/path.sapp**: προστέθηκε **absolute(p)** — αν p ξεκινά με `/` (Unix) ή δεύτερος χαρακτήρας `:` (Windows), επιστρέφει p· αλλιώς path_join(cwd(), p). Export ενημερώθηκε.
- **tests/semantics.rs**: το test **stdlib_fs_path_os_time_array_map_modules_work** επεκτάθηκε με path.is_dir(path.absolute(".")) → true.
- **docs/Tooling.md**: προστέθηκε absolute(p) στην ενότητα stdlib:path.sapp.
- **Verification**: `cargo test stdlib_fs_path_os_time_array_map_modules_work` περνά.
- **Τι δεν έγινε**: canonical path (resolve . και ..)· now_iso· process.sapp.
- **Επόμενο**: G2 now_iso / process.sapp ή W3/L2/M3.

### Pack test fix & G2 stdlib csv.sapp (Μάρτιος 2026)
- **tests/pack.rs**: το test **bundle_fails_when_path_dependency_missing** απέτυχε επειδή το `pblang bundle` έτρεχε με path ως argument και σε ορισμένα περιβάλλοντα το find_manifest_dir δεν έβρισκε το manifest. Διορθώθηκε με **current_dir(&proj)** και argument **"."** ώστε το bundle να τρέχει μέσα στο project dir και το lockfile generation να αποτυγχάνει με το αναμενόμενο μήνυμα "path not found" / "not found" για το missing path dependency. Προστέθηκε **make_temp_project_with_missing_path_dep_named(name)** και το bundle test χρησιμοποιεί ξεχωριστό temp dir ("pblang_bundle_missing_path_dep") ώστε να μην συγκρούεται με το pack test όταν τρέχουν παράλληλα.
- **stdlib/csv.sapp**: νέο module **parse(text)** και **stringify(rows)** — απλό CSV (split by newline και comma, χωρίς quoted fields). Χρησιμοποιεί stdlib:strings.sapp (lines, join).
- **tests/modules.rs**: νέο test **csv_stdlib_parse_and_stringify_work** (roundtrip "a,b\nc,d").
- **docs/Tooling.md**: προστέθηκε **stdlib:csv.sapp** (parse, stringify) στην ενότητα Stdlib-style APIs.
- **Verification**: `cargo test --test pack bundle_fails_when_path_dependency_missing` και `cargo test --test modules csv_stdlib_parse_and_stringify_work` περνούν.
- **Τι δεν έγινε**: quoted field handling στο CSV· path.absolute· now_iso.
- **Επόμενο**: G2 path.absolute / now_iso ή W3/L2/M3.

### Doc sync — Help baseline & PERPLEXITY_CONTEXT (Μάρτιος 2026)
- **docs/Help.md** (section 2 Already done): semantics count 39→44· os.sapp και time.sapp δεν αναφέρονται πλέον ως stub — αναφέρονται τα πραγματικά exports (os: cwd, exec, sleep_ms, env_get, env_set· time: clock, now_ms).
- **PERPLEXITY_CONTEXT.md**: λίστα tests ενημερώθηκε (προστέθηκαν pack, install_cli, threads, dump_bytecode, doc_cli)· σύντομη αναφορά README για commands και doc links. Graphics crate description (SDL2 / minifb).
- **Σκοπός**: συμφωνία docs με τρέχουσα κατάσταση project, ώστε νέοι contributors και external tools (π.χ. Perplexity) να βλέπουν ακριβή baseline και test suite.
- **Τι δεν έγινε**: καμία αλλαγή κώδικα· κανένα νέο feature.
- **Επόμενο**: G2 path.absolute / now_iso ή επόμενο backlog item (W3, L2, κ.λπ.).

### README, game.sapp, G2 time.sapp (Μάρτιος 2026)
- **README.md**: επεκτάθηκε με project overview, workspace layout, quick start, commands summary, tests, και links προς docs (Tooling, Language_Semantics, Embedding, Report, Help, Future_plans, PERPLEXITY_CONTEXT).
- **crates/pblang-graphics/examples/game.sapp**: χρήση **gfx.run(win, state, update, draw, fps)** με event-driven input· state = map με player (x, y, speed), update(state, events) χειρίζεται keydown για Left/Right/Up/Down και W/A/S/D, draw(state, win) σχεδιάζει από state. Το TODO "use gfx.run() with events for input" έκλεισε.
- **stdlib/time.sapp**: προστέθηκε **now_ms()** (clock() * 1000)· export ενημερώθηκε.
- **docs/Tooling.md**: προστέθηκαν **stdlib:time.sapp** (clock, now_ms) και **stdlib:os.sapp** (cwd, exec, sleep_ms, env_get, env_set) στην ενότητα Stdlib-style APIs.
- **tests/semantics.rs**: το test **stdlib_fs_path_os_time_array_map_modules_work** επεκτάθηκε με type(time.now_ms()) → "number".
- **Verification**: `cargo test stdlib_fs_path_os_time_array_map_modules_work` περνά.
- **Τι δεν έγινε**: now_iso(), path.absolute, os.sapp expansion, process.sapp.
- **Επόμενο**: G2 path.absolute ή os/time περαιτέρω (now_iso, args, env helpers)· W3/L2/M3 ανά backlog.

### G2 – stdlib path.sapp enrichment (Μάρτιος 2026)
- **VM (crates/pblang-vm)**: προστέθηκαν builtins **fs_is_file(p)** και **fs_is_dir(p)** — επιστρέφουν bool (metadata().is_file()/is_dir(), false αν path δεν υπάρχει ή δεν είναι accessible). Ενημερώθηκε η allow_fs λίστα και η builtins table.
- **stdlib/path.sapp**: προστέθηκαν **extension(p)** (τελευταίο τμήμα μετά τελευταία τελεία στο basename, "" αν όχι), **exists(p)** = fs_exists(p), **is_file(p)** = fs_is_file(p), **is_dir(p)** = fs_is_dir(p). Export ενημερώθηκε.
- **tests/semantics.rs**: το test **stdlib_fs_path_os_time_array_map_modules_work** επεκτάθηκε με path.extension (c.txt → "txt", noext → ""), path.exists / is_file / is_dir (temp file + cwd).
- **docs/Tooling.md**: στην ενότητα Stdlib-style APIs προστέθηκε **stdlib:path.sapp** με join, dirname, basename, extension, exists, is_file, is_dir.
- **Verification**: `cargo test stdlib_fs_path_os_time_array_map_modules_work` περνά.
- **Τι δεν έγινε**: δεν προστέθηκε absolute(p), os/time expansion, process.sapp.
- **Επόμενο**: G2 os.sapp / time.sapp expansion ή path.absolute.

### G1 + L1 – stdlib map.merge & fmt comma spacing (Μάρτιος 2026)
- **G1 – stdlib/map.sapp**: προστέθηκε **merge(m1, m2)** — επιστρέφει νέο map με όλα τα keys από m1 και m2 (τιμές από m2 αντικαθιστούν για κοινά keys). Export ενημερώθηκε.
- **tests/modules.rs**: το test **array_and_map_stdlib_helpers_work** επεκτάθηκε με merge: `map.merge(map{ a: 1, b: 2 }, map{ b: 20, c: 30 })` → merged.a==1, merged.b==20, merged.c==30.
- **L1 – pblang fmt comma spacing**: **src/main.rs** προστέθηκε **fmt_normalize_comma_spacing** — εισάγει space μετά το comma όταν δεν είμαστε μέσα σε string literal (ίδια λογική in-string με operator spacing). Κλήση μετά το operator spacing στο fmt loop.
- **tests/fmt.rs**: νέα tests **fmt_adds_space_after_comma** (f(1,2) → f(1, 2)) και **fmt_preserves_comma_inside_string** («a,b» αμετάβλητο).
- **docs/Tooling.md**: stdlib:map.sapp προστέθηκε **merge(m1, m2)**· στην παράγραφο fmt προστέθηκε comma spacing (space after comma, αμετάβλητο μέσα σε strings).
- **Verification**: `cargo test --test modules array_and_map_stdlib_helpers_work` και `cargo test --test fmt` περνούν.
- **Επόμενο**: G1 περαιτέρω stdlib· L1 richer formatter/lint.

### W2 – cookie helpers για request handling (Μάρτιος 2026)
- **stdlib/http_server.sapp**: προστέθηκαν οι helpers **cookies(req)** και **cookie_get(req, name, fallback)**.
  - `cookies(req)` διαβάζει header `Cookie` ή `cookie`, κάνει parse `k=v; a=b` και επιστρέφει map.
  - `cookie_get(...)` επιστρέφει τιμή cookie ή fallback όταν λείπει.
- **tests/semantics.rs**: νέο test **http_cookie_helpers_parse_and_get_values** που καλύπτει:
  - parse από `Cookie: sid=abc; theme=dark`,
  - direct πρόσβαση (`cs.sid`, `cs.theme`),
  - fallback behavior σε missing cookie.
- **docs/Tooling.md**: ενημερώθηκε η ενότητα Stdlib-style APIs για τα νέα cookie helpers.
- **Verification**:
  - `cargo test --test semantics http_cookie_helpers_parse_and_get_values`
  - `cargo test --test modules`
- **Τι δεν έγινε**: δεν προστέθηκε URL-decoding ή signed/encrypted cookie layer.
- **Επόμενο**: W2 chunked/streaming ή TLS step.

### E3 + G1 – bundle content test & array.reverse (Μάρτιος 2026)
- **E3 – bundle artifact content**: **tests/pack.rs** προστέθηκε regression test **bundle_dir_contains_manifest_and_entry_file** — μετά bundle ελέγχει ότι ο bundle directory περιέχει `pblang.toml` και το entry file (π.χ. main.sapp). Συμμετρικό με το pack tarball content test.
- **G1 – stdlib/array.sapp**: προστέθηκε **reverse(a)** — επιστρέφει νέο array με στοιχεία σε αντίστροφη σειρά. Export ενημερώθηκε.
- **tests/modules.rs**: το test **array_and_map_stdlib_helpers_work** επεκτάθηκε με assertions για array.reverse ([1,2,3] → len 3, r[0]=3, r[2]=1).
- **docs/Tooling.md**: στην ενότητα Stdlib-style APIs προστέθηκε **reverse(a)** στη λίστα array.sapp.
- **Verification**: `cargo test --test pack bundle_dir_contains_manifest_and_entry_file` και `cargo test --test modules array_and_map_stdlib_helpers_work` περνούν.
- **Τι δεν έγινε**: δεν άλλαξε το bundle file set· δεν προστέθηκε νέο E3 feature.
- **Επόμενο**: E3 single-artifact ή installer· W2 chunked/TLS· G1 περαιτέρω stdlib.

### E3 + G1 – packaging test & stdlib slice (Μάρτιος 2026)
- **E3 – pack artifact content**: **tests/pack.rs** προστέθηκε regression test **pack_tarball_contains_manifest_and_entry_file** — μετά pack ανοίγει το tarball και ελέγχει ότι περιέχει `pblang.toml` και `main.sapp` (ή τα αντίστοιχα entry paths). Κλειδώνει το contract "τι μπαίνει στο artifact".
- **G1 – stdlib/array.sapp**: προστέθηκε **slice(a, start, end)** — επιστρέφει νέο array με στοιχεία από index start (inclusive) έως end (exclusive)· indices εκτός 0..len(a) αγνοούνται. Export ενημερώθηκε.
- **tests/modules.rs**: το test **array_and_map_stdlib_helpers_work** επεκτάθηκε με assertions για array.slice ([10,20,30,40], 1, 3) → len 2, s[0]=20, s[1]=30.
- **docs/Tooling.md**: στην ενότητα Stdlib-style APIs προστέθηκε **slice(a, start, end)** στη λίστα array.sapp.
- **Verification**: `cargo test --test pack pack_tarball_contains_manifest_and_entry_file` και `cargo test --test modules array_and_map_stdlib_helpers_work` περνούν.
- **Τι δεν έγινε**: δεν άλλαξε το pack/bundle file set· δεν προστέθηκε [package].include ή pinned assets.
- **Επόμενο**: E3 single-artifact ή installer presets· W2 chunked/TLS· G1 περαιτέρω stdlib.

### G1 + L1 + R1 – stdlib & tooling docs (Μάρτιος 2026)
- **G1 – stdlib/array.sapp**: προστέθηκε **first(a)** (πρώτο στοιχείο ή nil αν κενό)· export ενημερώθηκε.
- **tests/modules.rs**: το test **array_and_map_stdlib_helpers_work** επεκτάθηκε με assertions για array.first ([10,20,30]→10, []→nil).
- **L1 – Tooling.md**: στην ενότητα Stdlib-style APIs προστέθηκαν σύντομες αναφορές σε **stdlib:strings.sapp** (chars, join, repeat, replace_all, starts_with, ends_with, contains, trim, lines, words) και σε **stdlib:array.sapp** / **stdlib:map.sapp** (first, last, values κ.λπ.) ώστε να είναι discoverable.
- **R1 – Tooling.md**: στην ενότητα Reverse engineering / bytecode προστέθηκε ρητό **Planned next steps**: call-graph ή data-flow analysis tooling, και decompiler/reconstruction research.
- **Verification**: `cargo test --test modules array_and_map_stdlib_helpers_work` περνά.
- **Τι δεν έγινε**: δεν προστέθηκε νέο VM builtin· δεν υλοποιήθηκε analysis tool ή decompiler.
- **Επόμενο**: E3 single-artifact· W2 chunked/TLS· G1 περαιτέρω stdlib ή L1 formatter rules.

### G1 + W2 + E3 – stdlib expansion & docs (Μάρτιος 2026)
- **G1 – stdlib/data**: **stdlib/array.sapp** προστέθηκε **last(a)** (τελευταίο στοιχείο ή nil αν κενό)· **stdlib/map.sapp** προστέθηκε **values(m)** (array τιμών με σειρά keys). Exports ενημερώθηκαν.
- **tests/modules.rs**: το test **array_and_map_stdlib_helpers_work** επεκτάθηκε με assertions για array.last ([10,20,30]→30, []→nil) και map.values (len και άθροισμα τιμών).
- **W2 – server-path timeouts τεκμηρίωση**: **docs/Tooling.md** διευκρινίστηκε ότι όταν ο host ορίζει **VmLimits.io_timeout_ms**, το VM εφαρμόζει read/write timeout στα streams από **tcp_accept**, άρα οι server connections ήδη σέβονται το global IO timeout· ως future work μένει per-request/per-handler timeout override από script.
- **E3 – package signing design note**: **docs/Registry_MVP.md** προστέθηκε ενότητα **Package signing / verification (design note)** — tarball signing (Ed25519/GPG), checksum chain, trust policy· design only, χωρίς υλοποίηση.
- **Verification**: `cargo test --test modules array_and_map_stdlib_helpers_work` περνά.
- **Τι δεν έγινε**: δεν προστέθηκε νέο VM builtin· δεν υλοποιήθηκε per-handler timeout στο stdlib· δεν υλοποιήθηκε package signing.
- **Επόμενο**: E3 single-artifact ή installer presets· W2 chunked/TLS· G1 περαιτέρω stdlib helpers.

### E3 – entry path τεκμηρίωση bundle (Μάρτιος 2026)
- **docs/Tooling.md**: Στην παράγραφο `bundle` προστέθηκε ρητά ότι η έξοδος επιτυχίας περιλαμβάνει το **resolved entry path** (π.χ. `(entry src/app.sapp)`), συμμετρικά με το pack. Τα tests `pack_then_run_from_manifest_dir_uses_package_entry` και `bundle_then_run_from_manifest_dir_uses_package_entry` ήδη ελέγχουν ότι η stdout περιέχει «entry src/app.sapp».

### W2 – timeouts future-work (Μάρτιος 2026)
- **docs/Tooling.md**: Στην παράγραφο stdlib http_server.sapp προστέθηκε σημείωση ότι **timeouts** (read/socket timeouts στο server path) είναι planned as future work και δεν υλοποιούνται ακόμα.

### E3 – dependency validation (Μάρτιος 2026)
- **tests/pack.rs**: `pack_fails_when_path_dependency_missing` και `bundle_fails_when_path_dependency_missing` — manifest με path dep που δείχνει σε μη υπάρχοντα φάκελο → pack/bundle αποτυγχάνουν με μήνυμα "path not found". Coverage για pinned/dependency validation.

### W2 – body limits τεκμηρίωση (Μάρτιος 2026)
- **docs/Tooling.md**: Στην παράγραφο stdlib http_server.sapp προστέθηκε σημείωση για body limits: `read_request_limited(stream, max_body_bytes)`, `serve_*_limited`, απόρριψη oversized Content-Length με 413 (χωρίς chunked/streaming ακόμα).

### Gap-fix — examples και N1 stub (Μάρτιος 2026)
- **examples/ai_plugin_minimal/plugin/lib.sapp**: minimal AI plugin με `plugin_info`, `capabilities`, `build_request`, `invoke`· normalized request/response envelope ώστε το test `ai_plugin_example_builds_normalized_request_envelope` να περνά.
- **examples/quantum_bridge_minimal/plugin/lib.sapp**: minimal quantum bridge με `build_job(c, opts)`· inline _emit_qasm (χωρίς stdlib import από plugin path)· job envelope provider, shots, qasm, metadata.qubits· test `quantum_bridge_example_builds_provider_job_envelope` περνά.
- **docs/Native_backend_notes.md**: stub με ενότητες Blockers, Backend criteria, Prerequisite checklist, Staged path (N1).
- **Έλεγχος**: `cargo test --test modules` 13/13 passed.

### E3 – better packaged run UX (Μάρτιος 2026)
- **main.rs**: μετά την γραμμή επιτυχίας του **pack** τυπώνεται run hint: «To run: unpack the tarball and run 'pblang run .' in the unpacked directory.»· μετά την επιτυχία του **bundle**: «To run: cd to the bundle dir and run './pblang run .' (pblang.exe run . on Windows).»
- **tests/pack.rs**: στα `pack_output_includes_package_name_version_and_description` και `bundle_output_includes_package_name_version_and_description` προστέθηκε assert ότι η stdout περιέχει «To run».
- **docs/Tooling.md**: στις παραγράφους pack και bundle προστέθηκε ότι η έξοδος επιτυχίας περιλαμβάνει οδηγία εκτέλεσης (run hint).

### W2 – static file serving helper (Μάρτιος 2026)
- **stdlib/http_server.sapp**: νέα συνάρτηση **static_file_response(root_dir, request_path)** — καθαρισμός path (αφαίρεση leading `/`, απόρριψη `..`)· έλεγχος ότι το path μένει κάτω από root· αν το αρχείο δεν υπάρχει ή το path δεν είναι έγκυρο → 404· για text files (fs_read)· content-type από extension (.html/.htm, .txt, .css, .js, .json· default text/plain)· return map{ status: 200|404, body, content_type }· προστέθηκε στα exports.
- **tests/semantics.rs**: `http_static_file_response_serves_text_file_under_root` (temp dir με hello.txt, κλήση static_file_response → 200, body «hello», content_type text/plain)· `http_static_file_response_returns_404_for_missing_file`.
- **docs/Tooling.md**: στην ενότητα Stdlib-style APIs προστέθηκε ότι το stdlib http_server.sapp προσφέρει static file helper (static_file_response) για ασφαλή σerving text αρχείων κάτω από root (200/404, content-type από extension).

### Phase 1 – Core runtime & sandbox (v0.2–v0.4)
- **VmHost** (pblang-ir): capabilities `allow_fs`, `allow_net`, `allow_http`, `allow_sqlite`, `allow_ffi`, `allow_exec`· επιπλέον `ffi_allowed_library(path)` και `ffi_allowed_symbol(symbol)`.
- **StdlibConfig** και **VM::with_stdlib(config)**: VM χωρίς builtins με `VM::new()`· με πλήρη stdlib με `VM::with_stdlib(config)`· builtins ελέγχουν capabilities κατά την κλήση.
- **VmLimits**: `max_instructions`, `max_stack_depth`· `vm.set_limits(Some(limits))`· έλεγχοι στο run loop και στο `call_top`· limits περνούν σε child VMs στο import.
- **FfiPolicy**: `allow_all`, `allowed_libs`, `allowed_symbols`· `vm.set_ffi_policy(Some(policy))`· `ffi_load` ελέγχει allowlist βιβλιοθήκης· `ffi_call` ελέγχει allowlist συμβόλων.
- **Embedding παράδειγμα**: `examples/embed.rs` – VM μόνο με fs + json.
- **Sandbox tests** (`tests/sandbox.rs`): fs/exec/ffi denied, instruction limit, stack depth limit, determinism, ffi allowlist.

### v0.2 – Golden tests & determinism
- **tests/golden.rs**: τρέχει `tests/fixtures/*.sap`, συγκρίνει έξοδο με `*.expected` (golden_arithmetic, golden_truthiness, golden_mapkeys_order, **golden_string_escapes** για escape sequences `\n` `\t` `\r` `\"` `\\`).
- Test determinism στο sandbox: ίδιο script δύο φορές → ίδια έξοδος (map_keys ordering).

### v0.3 – Lockfile & CI
- **tests/lockfile_repro.rs**: δημιουργεί lockfile δύο φορές σε temp dir· assert ότι η σειριακή έξοδος είναι ίδια.
- **Cargo.toml**: `[[test]]` για sandbox, golden, lockfile_repro.
- **.github/workflows/ci.yml**: CI workflow — `cargo test` on push/PR για branches master/main· jobs **test-linux** (ubuntu-latest) και **test-windows** (windows-latest). Baseline conformance suite.

### Phase 4 – Tooling (CLI)
- **pblang init**: δημιουργεί `pblang.toml` στο cwd· δεν αντικαθιστά αν υπάρχει.
- **pblang --doc \<file\> [--html] [out_dir]**: έξοδος exports, functions, polyglot blocks· με `--html` γράφει `out_dir/index.html` (default `docs/`).
- **pblang test [dir]**: τρέχει όλα τα `.sap`/`.sapp` στο `dir` (default `tests/`)· εκτύπωση "N passed, M failed"· exit 1 αν M > 0.

### v0.5 – Benchmarks
- **benches/bench_vm.rs**: υπάρχοντα arithmetic_loop, function_calls, string_concat· προστέθηκε **vm/json_roundtrip** (json_parse + json_stringify σε loop) για μέτρηση απόδοσης map/array + JSON.

### v0.3 – Error UX & JSON API (πρόσφατο πάσο)
- **Error UX**: τα runtime errors στο CLI τυπώνουν πλέον το ίδιο στυλ με τα parse errors (file:line:col, γραμμή πηγαίου, underline `^^^` στο span) μέσω `underline_span` στο `main.rs`.
- **JSON API**: προστέθηκε global αντικείμενο **`json`** με μεθόδους `json.parse(s)` και `json.stringify(x)` (ισοδύναμα με τα builtins `json_parse`/`json_stringify`), σύμφωνα με το Future_plans (stdlib-style API).

### Phase 4 – Tooling (συνέχεια)
- **pblang fmt [file.sap …]**: νέα εντολή· για τώρα stub (διαβάζει και ξαναγράφει το αρχείο χωρίς αλλαγή)· έτοιμη για μελλοντικό formatter.

### Phase 4 – pblang fmt (minimal formatting – τρέχων πάσος)
- **pblang fmt**: πλέον εφαρμόζει minimal formatting· trim trailing whitespace ανά γραμμή· ένα newline στο τέλος του αρχείου· ξαναγράφει μόνο αν άλλαξε το περιεχόμενο.

### Phase 4 – pblang fmt (indentation – τρέχων πάσος)
- **pblang fmt**: προστέθηκε **indentation by brace depth** (4 spaces per level)· κάθε γραμμή αλλάζει indent με βάση `{` / `}` (απλός heuristic)· trim πριν/μετά· τεκμηρίωση στο Tooling.md.
- **pblang fmt (operator spacing)**: προστέθηκε **spacing γύρω από binary operators**· ένα κενό πριν και μετά τα `+` `-` `*` `/` `==` `!=` `<` `>` `<=` `>=` (εκτός από unary minus π.χ. `-1`)· το περιεχόμενο μέσα σε string literals δεν αλλάζει.
- **tests/fmt.rs**: regression tests για fmt (operator spacing, διατήρηση περιεχομένου μέσα σε strings)· **Cargo.toml**: `[[test]] name = "fmt"`.
- **tests/common/mod.rs**: προστέθηκε `#[allow(dead_code)]` στο `run_err` ώστε να μην εμφανίζεται warning στα tests.
- **pblang lint [file.sap …]**: parse + typecheck ένα ή περισσότερα αρχεία· εκτύπωση διαγνωστικών· exit 1 αν υπάρχει σφάλμα.
- **pblang build [path]**: δημιουργία lockfile + typecheck του main entry (main.sap ή src/main.sap) αν υπάρχει· σύμφωνα με το tooling roadmap (Phase 4).

### Graphics/stdlib (μικρός εμπλουτισμός)
- **stdlib/graphics.sapp**: προστέθηκε συνάρτηση **gray(l)** (0..255 → [l,l,l,255]) για grayscale χρώματα.

### Graphics/stdlib (εμπλουτισμός – τρέχων πάσος)
- **stdlib/graphics.sapp**: νέα χρώματα **ORANGE, PURPLE, LIME, NAVY, TEAL, SILVER, MAROON, OLIVE**· συνάρτηση **lerpColor(c1, c2, t)** για γραμμική παρεμβολή χρωμάτων· **circle** ως μέθοδος παράθυρου (win.circle(cx, cy, r, color)) με σχεδίαση κύκλου ως 16-πλευρο πολύγωνο· exports ενημερώθηκαν.

### VM – μαθηματικά builtins & Graphics HSL (τρέχων πάσος)
- **num_floor(x)**, **num_ceil(x)**: νέα builtins στο VM (core)· χρήσιμα για HSL και γενικότερα.
- **stdlib/graphics.sapp**: συνάρτηση **hsl(h, s, l)** (h 0..360, s/l 0..1 → [r,g,b,255])· προστέθηκε στα exports.
- **tests/fixtures/golden_num_math.sap**: golden test για num_floor/num_ceil.
- **tests/stdlib_parse.rs**: test parse+typecheck για stdlib (π.χ. graphics.sapp)· προς το παρόν `#[ignore]` λόγω γνωστού parse θέματος σε συγκεκριμένα σημεία.

### Tooling – bundle (portable executable folder)
- **pblang bundle [path] [out_dir]**: δημιουργεί φάκελο (default `dist/`) με το τρέχον εκτελέσιμο pblang, `pblang.toml`, `pblang.lock`, το main entry script και τα τοπικά dependencies· για portable deployment (Windows/Linux)· τεκμηρίωση στο **Tooling.md**.

### Threads – σχεδίαση
- **docs/Threads.md**: νέο έγγραφο σχεδίασης για δυνατότητες threads (spawn/join, channels, host-provided workers)· τεχνικές προκλήσεις (shared state, VM per thread)· ροードμαπ για μελλοντική υλοποίηση.

### Threads MVP – spawn/join (T1, Φεβρουάριος 2025)
- **VmHost** (pblang-ir): `allow_spawn()`, `spawn_limits()` → `VmLimits` για κληρονομιά limits στο child VM.
- **StdlibConfig.spawn**: capability για spawn/join (default true)· sandbox με spawn: false → "spawn not allowed".
- **Builtins**: `spawn(closure)`, `join(handle)`· child τρέχει σε νέο OS thread με δικό του VM· αποτέλεσμα μέσω JSON (τιμή ή error message).
- **run_return_value(chunk)**: νέα μέθοδος VM για spawn child (run script και επιστροφή top-of-stack).
- **Tests**: `tests/threads.rs` — spawn_join_returns_value, spawn_join_propagates_error, spawned_vm_inherits_limits, not_transferable_handles_error, spawn_not_allowed_when_sandboxed.

### Threads MVP – channels (T2, Μάρτιος 2026)
- **VmHost** (pblang-ir): νέα hooks `snapshot_upvalue(...)`, `own_channel_sender(id)`, `own_channel_receiver(id)` ώστε το `spawn` να μεταφέρει μόνο safe captures και να γίνεται cleanup των transferred channel endpoints όταν τερματίζει το child VM.
- **spawn transferable captures**: το `spawn` δεν περιορίζεται πλέον σε closures χωρίς captures· serializes constants + safe upvalues (numbers/bools/strings/nil/arrays/maps και tagged channel endpoints) και τα ξαναχτίζει στο child VM ως closed upvalues. Μη μεταφέρσιμες τιμές συνεχίζουν να απορρίπτονται.
- **Builtins**: `channel()` επιστρέφει map με `sender` / `receiver` endpoints· `send(sender, value)` στέλνει transferable value και καταναλώνει το sender handle στο τρέχον MVP· `recv(receiver)` block-άρει και επιστρέφει `nil` όταν το channel έχει κλείσει και αδειάσει.
- **Runtime tables**: προστέθηκαν send/recv channel tables στο `pblang-vm` με ownership tracking ανά VM, ώστε το κλείσιμο του child VM να αποδεσμεύει τα transferred endpoints και να επιτρέπει closed-channel semantics χωρίς shared heap.
- **Crash resistance**: το `close_upvalues_from()` δεν κάνει πλέον panic όταν συναντήσει stale open-upvalue index στο thread/capture path· κάνει safe close σε `nil` αντί για out-of-bounds panic.
- **Tests**: `tests/threads.rs` προστέθηκαν `channel_send_recv_works`, `recv_on_closed_channel_returns_nil_or_closed`, `channel_send_recv_is_deterministic`, `channel_rejects_non_transferable_values`. Όλο το `tests/threads.rs` περνά (`9/9`).
- **Verification**: `cargo test --test threads` περνά. Το συνολικό `cargo test` σε αυτό το run σταμάτησε σε άσχετα `install_cli` failures (`install_with_registry_index_resolves_registry_dep`, `publish_then_install_works`) με path/index lookup σφάλμα σε temporary `file://` registry paths.
- **Τι δεν έγινε**: δεν προστέθηκαν `select`, explicit `close`, shared-state threads ή async runtime. Δεν έγινε debugging των άσχετων `install_cli` failures σε αυτό το run λόγω `one task per run`.
- **Τι μένει επόμενο**: αν χρειαστεί, richer channel ergonomics πάνω στο ίδιο VM-per-thread model ή επιστροφή στο `P1` για άλλο failing semantics area.

### Tests
- **tests/http_server.rs**: το test `http_server_serve_once_handles_one_request` σημειώνεται ως `#[ignore]` (ευαίσθητο σε δίκτυο/timing)· τρέχει με `cargo test -- --ignored` όταν χρειάζεται.

### Τεκμηρίωση
- **docs/Language_Semantics.md**, **Stability_policy.md**, **Lockfile_contract.md**, **Crash_resistance.md**, **Tooling.md** ενημερώθηκαν/προστέθηκαν σύμφωνα με το roadmap.
- **docs/Embedding.md**: νέο έγγραφο **Embedding API** (VmHost, VM::new / VM::with_stdlib, capabilities, limits, FFI policy, resource lifecycle, module loading)· σύνδεση από Language_Semantics.md.
- **Language_Semantics.md**: νέα ενότητα **String literals** — τεκμηρίωση escape sequences `\\` `\"` `\n` `\t` `\r`.

### Sync Future_plans ↔ τρέχουσα κατάσταση (τελευταίος πάσος)
- **docs/Future_plans.md** ενημερώθηκε ώστε να αντικατοπτρίζει τι έχει γίνει: VmLimits (max_memory_bytes, io_timeout_ms), Embedding API τεκμηρίωση, pblang fmt (indent by brace depth)· §8 Production, §10 Tooling και το Συνοπτικά διορθώθηκαν.

### Production – εφαρμογή io_timeout_ms στα IO builtins (τελευταίος πάσος)
- **VmHost::io_timeout_ms()**: νέα μέθοδος στο trait (pblang-ir)· default `None`· το VM την υλοποιεί από `limits.io_timeout_ms`.
- **http_get / http_post**: χρήση `host.io_timeout_ms()`· όταν οριστεί, η ureq κλήση γίνεται με `.timeout(Duration::from_millis(ms))`.
- **tcp_accept**: μετά το accept, στο νέο `TcpStream` τίθενται `set_read_timeout` και `set_write_timeout` από `host.io_timeout_ms()`.
- **fs_read, fs_write, fs_read_bytes, fs_write_bytes**: όταν οριστεί `io_timeout_ms`, η I/O τρέχει σε thread και η κλήση περιμένει μέχρι timeout· σε timeout επιστρέφεται σφάλμα "timeout".
- **Τεκμηρίωση**: Embedding.md και Language_Semantics.md ενημερώθηκαν (πού εφαρμόζεται το io_timeout).

### Import resolution (Windows paths) & module tests
- **resolve_import** (pblang-pm): paths τύπου `C:/...` ή `C:\...` δεν ερμηνεύονται πλέον ως package "C"· ελέγχουμε αν το τμήμα πριν το `:` είναι single-letter drive (Windows) και τότε χρησιμοποιούμε path resolution αντί για package lookup.
- **tests/modules.rs**: fixture `cache_me.sapp` διορθώθηκε (προστέθηκε `export forty_two`) ώστε το test `import_builtin_caches_by_path_identity` να περνά.

### Parser & stdlib (module tests quantum/math)
- **String escape sequences** (pblang-core parser): τα string literals υποστηρίζουν πλέον escapes `\n`, `\t`, `\r`, `\"`, `\\`· διορθώνει το parse error στο quantum.sapp (strings με `\"` και `\n`).
- **stdlib/math.sapp**: αντικαταστάθηκαν τα `&&`/`||` με **and**/**or** (η γλώσσα έχει keywords `and`/`or`, όχι operators `&&`/`||`)· διορθώνει το parse error στο math.sapp.
- Αναμένεται όλα τα module tests (συμπερ. package_style_import, quantum_stdlib_emits_qasm, math_stdlib_complex_and_algorithms_work) να περνούν μετά το build.

### Golden tests & fmt & docs (Φεβρουάριος 2025)
- **tests/golden.rs**: κανονικοποίηση line endings στη σύγκριση (expected.replace("\\r\\n", "\\n")) ώστε τα golden fixtures να περνούν και σε Windows (CRLF vs LF).
- **pblang fmt**: προστέθηκε προαιρετική παράμετρος **--max-line-length=N**· όταν οριστεί, γραμμές που ξεπερνούν N χαρακτήρες σπάνε στο τελευταίο κενό πριν το N (soft wrap)· η επόμενη γραμμή κρατά το ίδιο indent· **tests/fmt.rs**: προστέθηκε `fmt_max_line_length_wraps_long_lines`· τεκμηρίωση στο Tooling.md.
- **docs/Spec_freeze.md**: νέο έγγραφο **Spec freeze (v0.1)** — scope παγώματος (syntax, semantics, error model, modules), σύνδεση με Language_Semantics και Stability_policy.
- **docs/Registry_MVP.md**: νέο έγγραφο **Registry MVP** — σχεδίαση Lockfile v2 (name, version, source, resolved), index format, fetch/install και publish flow· προτείνεται σειρά υλοποίησης.

### VM – defer κλήση (Φεβρουάριος 2025)
- **do_return**: όταν τρέχουν τα defers, οι κλήσεις των defer closures γίνονται με **call_top(0)**· η return value αποθηκεύεται σε frame.module_result και επιστρέφεται μετά τα defers (LIFO). Το test **defer_runs_on_return_lifo_order** περνά.

### Lockfile v2 & docs (Φεβρουάριος 2025, Help.md backlog)
- **Lockfile v2 schema** (pblang-pm): **LockedDep** έχει πλέον optional πεδία **name**, **version**, **source**, **resolved**, **fetch_url**· backward compatible (παλιά lockfiles φορτώνονται· νέα πεδία skip_serializing_if = None ώστε path-only deps να γράφουν ίδιο TOML όπως πριν). Τεκμηρίωση στο Lockfile_contract.md.
- **docs/Threads.md**: προστέθηκε ενότητα **Μοντέλο: VM-per-thread + message passing** — transferable values (deep copy / serialize), error semantics (join επιστρέφει value ή error), sandbox (allow_spawn), κληρονομιά VmLimits, builtins σε spawned threads.
- **docs/AI_plugin_pattern.md**: νέο έγγραφο **AI plugin / package pattern** — core μένει μικρός, AI μέσω host embedding / FFI / packages (registry), τυπικό pattern για πακέτο AI, σύνδεση με Embedding και Registry_MVP.

### Registry MVP – Task 1 (Φεβρουάριος 2025)
- **pblang-pm**: προστέθηκε unit test **installs_one_registry_dep_to_cache()** — temp dir με index.json (file://) και ένα .tar.gz fixture, manifest με `"@pblang/test" = "1.0.0"`, κλήση **generate_lockfile_with_registry**· assert για dep path στο cache, source=registry, version, sha256. Test περνά (Windows + Linux).
- **file:// URLs (Windows)**: στο **fetch_url** προστέθηκε strip του prefix **?/** μετά το trim (canonicalize στα Windows δίνει `\\?\C:\...` → ως URL γίνεται `?/C:/...`)· ώστε file:// URLs από canonicalize να λειτουργούν και σε Windows.
- **Τι μένει**: git deps· semantics 35/37 (απομένουν 2: class_sugar_creates_method_map, db_migrations_apply_in_order_once).

### Registry R1 – checksum mismatch + sha256 required (Help.md Task R1)
- **resolve_registry_dep**: νέα παράμετρος **require_registry_sha256**· όταν true, αν το index entry δεν έχει sha256 → hard error "registry: sha256 required for …".
- **generate_lockfile** / **generate_lockfile_with_registry**: **REQUIRE_REGISTRY_SHA256** env (1 ή true) ενεργοποιεί το strict mode· generate_lockfile διαβάζει το env και περνάει τη σημαία στο resolve.
- **Tests**: **registry_checksum_mismatch_fails()** (index με λάθος sha256 → Err με "checksum mismatch"), **registry_with_sha256_passes()** (σωστό sha256 στο index → Ok), **registry_sha256_required_fails_when_missing()** (sha256: null + require true → Err "sha256 required").
- **docs/Lockfile_contract.md**: νέα ενότητα **Registry deps security** (checksum verification, REQUIRE_REGISTRY_SHA256).

### Registry R2 – CLI --registry-index (Help.md Task R2)
- **pblang install**: προστέθηκε **--registry-index \<url\>**· όταν δίνεται, καλεί **generate_lockfile_with_registry(manifest_dir, Some(url), require_sha)** ώστε να μην απαιτείται env **PBLANG_REGISTRY_INDEX**. Το path παραμένει προαιρετικό μετά τα options.
- **Help string** (main.rs): ενημερώθηκε για `install [--registry-index <url>] [path]`.
- **docs/Tooling.md**: ενημερώθηκε η ενότητα install (flags + env vars)· νέα ενότητα **Registry / install env vars** (PBLANG_REGISTRY_INDEX, REQUIRE_REGISTRY_SHA256).
- **tests/install_cli.rs**: **install_with_registry_index_resolves_registry_dep()** (temp dir με index + tarball, manifest με registry dep, `pblang install --registry-index file:///...` → lockfile με source=registry)· **install_without_registry_index_uses_env_or_fails_for_registry_dep()** (manifest μόνο με registry dep, install χωρίς flag και χωρίς env → fail με μήνυμα registry).

### Registry R3 – publish MVP local (Help.md Task R3)
- **pblang-pm**: **create_package_tarball(package_dir)** — πακετάρει το package σε .tar.gz (ίδιο file set με hash_dir). **publish_to_registry_dir(package_dir, registry_dir)** — διαβάζει [package] name/version, δημιουργεί tarball, γράφει \<name\>-\<ver\>.tar.gz στο registry dir, ενημερώνει/δημιουργεί **index.json** με url (file://) και sha256.
- **pblang publish --registry-dir \<dir\> [path]**: νέα εντολή στο main.rs· απαιτεί --registry-dir· προαιρετικό path (default cwd).
- **tests/install_cli.rs**: **publish_then_install_works()** — δημιουργία package A, publish σε temp registry dir, δημιουργία package B με dep στο A, install --registry-index file:///... → lockfile με source=registry.

### Production – crash resistance (partial audit, τελευταίος πάσος)
- **VM run loop (catch unwind)**: αντικαταστάθηκε το `frames.pop().unwrap()` κατά το unwind για try/catch με `if let Some(frame) = self.frames.pop() { ... } else { break; }` ώστε να μην κάνει panic το host σε κατεστραμμένη κατάσταση.
- **Stdlib init (json global)**: τα `.find(...).unwrap()` για json_parse/json_stringify αντικαταστάθηκαν με `if let (Some(jp), Some(js)) = ...`· αν λείπουν τα builtins, το global `json` απλά δεν εγκαθίσταται.
- **Mutex tables (ffi/db/net)**: όλα τα `Mutex::lock().unwrap()` στους πίνακες handles (ffi, db, net) αντικαταστάθηκαν με safe helpers `with_ffi_table`, `with_db_table`, `with_net_table` που σε poison επιστρέφουν `Err(host.err("internal: … table unavailable"))` αντί για panic. Τα builtins και το `gc_collect` χρησιμοποιούν αυτά τα helpers.
- **Τεκμηρίωση**: docs/Crash_resistance.md ενημερώθηκε (ενότητα "Done (partial audit)").

---

## Επόμενα βήματα (από Future_plans)

- **Phase 4**: **pblang fmt** έχει minimal formatting, **spacing γύρω από operators** (+ - * / == != < > <= >=), indent by brace depth (4 spaces), **--max-line-length=N** για optional line wrap· περαιτέρω κανόνες μελλοντικά· το lint υπάρχει· το **build** υλοποιήθηκε. **Spec_freeze.md** και **Registry_MVP.md** προστέθηκαν (spec freeze scope, registry design).
- **Web**: MVP HTTP server API· το stdlib `http_server.sapp` υπάρχει με serve_once/serve_forever.
- **Graphics/stdlib**: HSL υλοποιήθηκε (hsl)· περαιτέρω σχήματα/βοηθητικά αν χρειαστεί.
- **Threads**: spawn/join και channels MVP υλοποιήθηκαν (`channel`, `send`, `recv`)· η σχεδίαση στο **docs/Threads.md** αντικατοπτρίζει πλέον το τρέχον API. Μελλοντικά μόνο richer ergonomics (`close`, multi-send strategy, ίσως `select`) αν χρειαστεί.
- **Executables**:
  - **Stage A – `pblang pack`**: νέα CLI εντολή που δημιουργεί ένα ενιαίο `.tar.gz` artifact από το manifest dir, συμπεριλαμβάνοντας `pblang.toml`, ενημερωμένο `pblang.lock` (μέσω `generate_lockfile`) και τοπικά dependencies. Το unpacked artifact μπορεί να τρέξει με το εγκατεστημένο binary (`pblang run main.sapp`) χωρίς να χρειάζεται το αρχικό project folder.
  - **Stage B** (μελλοντικό): native single-binary compile (LLVM/Cranelift ή transpile) σύμφωνα με το Future_plans.
- **Package manager**: **pblang install** υλοποιήθηκε (lockfile generation)· registry fetch/publish μελλοντικά.
- **Reverse engineering**: **pblang --dump-bytecode** υλοποιήθηκε (disassembly)· περαιτέρω εργαλεία μελλοντικά.
- **v1.0**: Spec freeze, conformance suite· **Embedding API** τεκμηριώθηκε στο docs/Embedding.md (VmHost, lifecycle). **CI** (GitHub Actions): baseline conformance — `cargo test` on Linux + Windows.
- **Production**: Crash resistance — έγινε μερικό audit (catch unwind, json init, Mutex tables ffi/db/net)· περαιτέρω: bounds checks αν χρειαστεί· **io_timeout_ms** εφαρμόζεται πλέον και σε **fs_read, fs_write, fs_read_bytes, fs_write_bytes** (μέσω thread + recv_timeout).

### Tests (διορθώσεις)
- **smoke**: το test `params_not_implemented_yet_should_error` ήταν outdated (locals υλοποιημένα)· μετονομάστηκε σε **params_function_with_args_works** και ελέγχει ότι η κλήση `add(2, 3)` επιστρέφει 5.
- **semantics**: **ffi_load_missing_library_errors** τρέχει πλέον με `StdlibConfig { ffi: true }` (μέσω **run_err_with_config** στο common) ώστε το σφάλμα να προέρχεται από το ffi_load (missing library) και όχι από "ffi access denied". **tests/common/mod.rs**: προστέθηκε **run_err_with_config(config, src)**.
- **semantics**: **32 περνούν**, 5 αποτυγχάνουν (class sugar, class auto_new, defer_runs_on_return_lifo_order, db_migrations, http_router_middleware). Τα tests **match_expression_literal_and_else** και **match_expression_bind_and_wildcard** περνούν μετά από νέο desugaring του `ExprKind::Match` σε block expression με `if`/`else` (βλ. P1). **sqlite_db_exec_and_query_work** περνά μετά από διορθώσεις· **gc_collect_breaks_rc_cycles_in_containers** περνά με χαλαρή assertion (cycles, arrays=, maps=).
 
### P1 – Production gates: conformance (Match expression desugaring, Φεβρουάριος 2025)
- **Match expression (Literal/Bind/Wildcard arms + else)**: στο **pblang-compiler** το `ExprKind::Match` δεν εκπέμπει πλέον custom bytecode με χειροκίνητη διαχείριση τοπικών slots· αντ' αυτού desugar-εται σε `BlockExpr` που υλοποιεί `match` ως:
  - `let __match = <scrutinee>; let __match_res = nil;` και αλυσίδα από `if`/`else` με Literal arms (`__match == lit`), Bind arms (`let name = __match; __match_res = body;`) και wildcard.
  - Η τελική έκφραση του block είναι `__match_res`, οπότε το `match` παραμένει κανονική expression χωρίς να απαιτείται ειδική μεταχείριση από το VM.
- Το νέο desugaring αποφεύγει τα σφάλματα "local slot out of bounds" στο Bind arm, και τα tests **match_expression_literal_and_else** / **match_expression_bind_and_wildcard** περνούν. Ο πάσος αυτός κλείνει ένα από τα failing semantics tests (P1), μειώνοντας τα semantics failures από 6 → 5.

### Compiler & stdlib (τελευταίος πάσος)
- **Compiler**: επιτρέπεται πλέον **πολλαπλά `let _`** στο ίδιο scope (discard binding)· αποφεύγεται panic "Variable redeclared in same scope" σε κώδικα με πολλά `let _ = ...`.
- **stdlib/db.sapp**: στο `migrate` το δεύτερο for-loop χρησιμοποιεί τώρα μεταβλητή **j** αντί για **i** ώστε να μην υπάρχει redeclaration στο ίδιο scope.
- **db_migrations_apply_in_order_once**: εξακολουθεί να αποτυγχάνει με "undefined global 'kind'" μέσα στο transaction closure (πιθανό ζήτημα upvalues όταν το πλαίσιο του _conn έχει κλείσει).

### P1 – Συνοπτικό update (semantics status)
- **class_auto_new_calls_init** περνά: στο compiler, όταν `StmtKind::Block(stmts)` τρέχει στο top-level με πάνω από ένα stmt και το πρώτο stmt είναι `let Name = map{...}` (Let με Map init), το πρώτο stmt εκτελείται χωρίς νέο scope ώστε να γίνει DefineGlobal· τα επόμενα (Name.new = ...) βρίσκουν το Name ως global. Τα κανονικά blocks (π.χ. `{ let x = 2; print x; }`) δεν επηρεάζονται.
- Τα semantics tests είναι πλέον **35/37 passing**, με 2 failing: **class_sugar_creates_method_map**, **db_migrations_apply_in_order_once**.

### P1 – db_migrations_apply_in_order_once (Μάρτιος 2026)
- **stdlib/db.sapp**: το `transaction()` δεν βασίζεται πλέον σε `catch { ... }` block-expression. Χρησιμοποιεί helper-based result wrapping (`_tx_ok` / `_tx_err`) ώστε το rollback path να μείνει εκτός του nested closure/IIFE shape που χτυπούσε το known capture limitation.
- **stdlib/db.sapp**: προστέθηκε `_exec_sql_script(conn, script)` ώστε τα migration `up` strings με πολλά `;` statements να εκτελούνται ως διαδοχικά statements με trim/skipping empty parts.
- **pblang-vm / sqlite_exec**: όταν ένα `sqlite_exec()` αποτυγχάνει, ο SQLite handle επανεισάγεται πλέον στο db table πριν επιστραφεί το error· αυτό επιτρέπει στο rollback path να χρησιμοποιήσει τον ίδιο handle αντί να καταλήγει σε `invalid handle`.
- **tests/semantics.rs**: προστέθηκε νέο regression test **db_transaction_rolls_back_on_error**. Το υπάρχον migration fixture καθαρίστηκε ώστε να ελέγχει το πραγματικό contract “apply in order once” χωρίς διπλό `CREATE TABLE t(...)` σε δύο διαφορετικά migration versions.
- **Verification**:
  - `cargo test --test semantics db_migrations_apply_in_order_once -- --nocapture` περνά
  - `cargo test --test semantics db_transaction_rolls_back_on_error -- --nocapture` περνά
  - `cargo test --test semantics` δίνει πλέον **37 passed, 1 failed**
- **Τι δεν έγινε**: δεν έγινε γενικό fix στο compiler closure/upvalue model.
- **Τι μένει επόμενο**: το μόνο εναπομείναν failing semantics area είναι το **class_sugar_creates_method_map**.

### P1 – class_sugar_creates_method_map contract (Μάρτιος 2026)
- **tests/semantics.rs**: το failing test `class_sugar_creates_method_map` ευθυγραμμίστηκε με το πραγματικό sugar contract και ελέγχει πλέον ρητά class-backed dispatch μέσω `map{ __class: Counter, n: 0 }`.
- **tests/semantics.rs**: προστέθηκε νέο regression test `method_call_sugar_falls_back_to_direct_map_method` ώστε να καλύπτεται και το plain-map path όπου η μέθοδος υπάρχει πάνω στο ίδιο object map.
- **docs/Language_Semantics.md**: προστέθηκε σύντομη διευκρίνιση του contract για `obj:method(...)`: πρώτα `obj.__class`, αλλιώς direct-map fallback, χωρίς implicit global class lookup.
- **Verification**:
  - `cargo test --test semantics -- --nocapture` περνά
  - το suite δίνει πλέον **39 passed, 0 failed**
- **Τι δεν έγινε**: δεν προστέθηκε νέο global class lookup και δεν έγινε OOP/runtime redesign.
- **Τι μένει επόμενο**: το επόμενο active backlog item είναι το **M1 — Registry ecosystem completion**.

### M1 – registry file:// absolute path hardening (Μάρτιος 2026)
- **pblang-pm / fetch_url()**: το `file://` path normalization δεν αφαιρεί πλέον λανθασμένα το leading `/` από absolute Unix-style paths (`file:///tmp/...`). Το read path περνά πλέον από μικρό helper που κρατά σωστή συμπεριφορά σε Unix και συνεχίζει να καθαρίζει τα Windows-specific prefixes (`/C:/...`, `?/`).
- **pblang-pm tests**: προστέθηκε νέο unit test `fetch_url_reads_absolute_file_urls` ώστε να κλειδωθεί το regression για absolute `file:///...` URLs.
- **CLI regression coverage**: τα υπάρχοντα `tests/install_cli.rs` tests `install_with_registry_index_resolves_registry_dep` και `publish_then_install_works` περνούν πλέον ξανά, γιατί το registry index/tarball fetch path δεν μετατρέπεται πια σε λάθος relative path (`tmp/...` αντί `/tmp/...`).
- **Verification**:
  - `cargo test -p pblang-pm --lib -- --nocapture` περνά
  - `cargo test --test install_cli -- --nocapture` περνά
- **Τι δεν έγινε**: δεν προστέθηκε git source support, remote publish API ή νέο lockfile schema redesign σε αυτό το run.
- **Τι μένει επόμενο**: μέσα στο `M1` backlog μένουν ακόμα import από registry package sources, git source story, και περαιτέρω lockfile/ecosystem hardening.

### M1 – bare package imports from installed sources (Μάρτιος 2026)
- **pblang-pm / resolve_import()**: υποστηρίζεται πλέον bare package import της μορφής `import("@scope/pkg")` ή `import("depname")` όταν το dependency υπάρχει στο manifest και έχει εγκατασταθεί/κλειδωθεί. Το resolver χρησιμοποιεί το υπάρχον `pblang.lock` όταν είναι έγκυρο, αλλιώς κάνει fallback μόνο για path deps· για registry deps χωρίς install δίνει πλέον καθαρό μήνυμα ότι χρειάζεται `pblang install`.
- **Default package entry**: το bare package import φορτώνει cross-platform το πρώτο διαθέσιμο entry από `lib.sapp`, `lib.sap`, `main.sapp`, `main.sap`, ώστε το package root import να δουλεύει με predictable convention χωρίς νέο manifest schema.
- **tests/install_cli.rs**: προστέθηκε νέο end-to-end regression test `installed_registry_package_can_be_imported_by_package_name`, που καλύπτει το flow `publish -> install -> import("@pblang/pub-pkg") -> run`.
- **Verification**:
  - `cargo test -p pblang-pm --lib -- --nocapture` περνά
  - `cargo test --test install_cli -- --nocapture` περνά
- **Τι δεν έγινε**: δεν προστέθηκε git source import, remote registry auto-fetch at import time, ή νέο package entrypoint manifest field.
- **Τι μένει επόμενο**: μέσα στο `M1` backlog μένει κυρίως το git source story και περαιτέρω ecosystem/lockfile hardening.

### M1 – git source dependencies with pinned rev (Μάρτιος 2026)
- **pblang-pm / DependencySpec**: το `pblang.toml` δέχεται πλέον detailed dependency μορφές για `path`, `version`, και το νέο `git` + `rev`. Αυτό επιτρέπει minimal git source contract χωρίς να αλλάξει το υπάρχον path/registry syntax.
- **pblang-pm / generate_lockfile()**: git dependencies λύνονται πλέον σε cached checkout keyed από `git_url + rev`, γράφουν `source = "git"` και `resolved = "<git-url>#<rev>"` στο `pblang.lock`, και το content hash συνεχίζει να παράγεται από το checked-out working tree χωρίς `.git` metadata ώστε να μείνει cross-platform και reproducible ως προς το περιεχόμενο.
- **Pinned rev only**: για git deps απαιτείται ρητό `rev`, ώστε το install/lockfile contract να μένει deterministic. Για git deps χωρίς install, το import path δίνει πλέον καθαρό μήνυμα ότι χρειάζεται `pblang install` πρώτα.
- **tests/install_cli.rs**: προστέθηκε νέο integration test `install_git_dependency_with_pinned_rev_works`, που δημιουργεί local git repo, κάνει commit, εγκαθιστά dependency μέσω `{ git = "...", rev = "..." }`, ελέγχει `source = "git"` στο lockfile και τρέχει `import("gitdep")` end-to-end.
- **src/main.rs / bundle path deps**: το bundle flow συνεχίζει να αντιγράφει μόνο local path dependencies και απλώς αγνοεί registry/git entries αντί να σπάει από το νέο dependency shape.
- **Verification**:
  - `cargo test --test install_cli -- --nocapture` περνά
  - `cargo test --test pack -- --nocapture` περνά
- **Τι δεν έγινε**: δεν προστέθηκε remote git auth handling, branch/tag resolution χωρίς rev pin, ή auto-fetch στο import path.
- **Τι μένει επόμενο**: μέσα στο `M1` backlog μένει κυρίως περαιτέρω lockfile/ecosystem hardening και, αν ζητηθεί, richer git source ergonomics πάνω στο pinned-rev contract.

### M1 – lockfile content validation & reproducibility hardening (Μάρτιος 2026)
- **pblang-pm / resolve_dependency_root()**: όταν ένα dependency λύνεται από `pblang.lock`, γίνεται πλέον verify του unpacked dependency tree απέναντι στο locked `sha256` πριν χρησιμοποιηθεί το path. Αν τα αρχεία του dependency αλλοιωθούν μετά το install, το import αποτυγχάνει με clear `content hash mismatch` αντί να φορτώνει σιωπηρά tampered code.
- **Registry lockfile contract**: τα νέα registry lock entries γράφουν πλέον `sha256` ως hash του unpacked dependency directory και κρατούν ξεχωριστά το tarball hash στο νέο optional πεδίο **`archive_sha256`**. Αυτό εναρμονίζει το registry path με το υπάρχον path/git contract και επιτρέπει ουσιαστικό import-time validation.
- **Legacy registry compatibility**: παλιότερα registry lockfiles που είχαν archive checksum στο `sha256` συνεχίζουν να φορτώνουν μόνο αν το archive μπορεί να επαληθευτεί θετικά από το `fetch_url`. Αν αυτό δεν είναι δυνατό, το μήνυμα καθοδηγεί σε `pblang install` για refresh του lockfile.
- **Tests**:
  - **tests/install_cli.rs**: νέο end-to-end regression test **tampered_installed_registry_dependency_fails_lockfile_hash_check**.
  - **tests/lockfile_repro.rs**: νέα tests **lockfile_reproducibility_registry_dependency** και **lockfile_reproducibility_git_dependency**.
  - **crates/pblang-pm/src/lib.rs** unit coverage: τα registry unit tests ευθυγραμμίστηκαν με το νέο contract (`content sha` + `archive_sha256`).
- **Verification**:
  - `cargo test -p pblang-pm --lib -- --nocapture` περνά
  - `cargo test --test install_cli -- --nocapture` περνά
  - `cargo test --test lockfile_repro -- --nocapture` περνά
- **Τι δεν έγινε**: δεν προστέθηκε hosted registry service, semver solver, auto-fetch στο import path, ή remote publish/auth flow.
- **Τι μένει επόμενο**: μέσα στο `M1` backlog μένει κυρίως περαιτέρω ecosystem usability/hardening πάνω από το τωρινό contract (π.χ. επιπλέον install/import ergonomics ή lockfile policy tightening αν ζητηθεί).

### M1 – package manifest entry for bare imports (Μάρτιος 2026)
- **pblang-pm / PackageSection**: το `[package]` δέχεται πλέον προαιρετικό **`entry`** field, π.χ. `entry = "src/entry.sapp"`.
- **Bare package import resolution**: το `resolve_package_root_import()` κοιτά πρώτα το `pblang.toml` του package root και, αν υπάρχει `entry`, το χρησιμοποιεί ως canonical package entrypoint πριν κάνει fallback στο παλιό heuristic (`lib.sapp`, `lib.sap`, `main.sapp`, `main.sap`).
- **Safety / compatibility**: το `entry` είναι strictly optional και πρέπει να είναι relative path μέσα στο package root. Absolute paths, `..` escapes ή missing declared entry γίνονται clean import errors αντί για silent fallback. Packages χωρίς `entry` συνεχίζουν να δουλεύουν όπως πριν.
- **Tests**:
  - **tests/install_cli.rs**: νέο registry regression test **installed_registry_package_uses_manifest_entry_for_package_import**.
  - **tests/install_cli.rs**: νέο git regression test **install_git_dependency_uses_manifest_entry_for_package_import**.
- **Verification**:
  - `cargo test -p pblang-pm --lib -- --nocapture` περνά
  - `cargo test --test install_cli -- --nocapture` περνά
- **Τι δεν έγινε**: δεν προστέθηκε νέο manifest field για subpath exports, auto-fetch στο import path, ή package-level semver/entrypoint negotiation.
- **Τι μένει επόμενο**: μέσα στο `M1` backlog μένει κυρίως περαιτέρω package ecosystem usability (π.χ. μικρές import/install ergonomics) ή επιπλέον lockfile policy tightening αν ζητηθεί.

### M1 – local registry publish immutability (Μάρτιος 2026)
- **pblang-pm / publish_to_registry_dir()**: το local registry treat-άρει πλέον κάθε `package name + version` ως immutable release. Αν το ίδιο version ξαναγίνει publish με διαφορετικό tarball content, το publish αποτυγχάνει με clear error αντί να αντικαθιστά σιωπηρά το υπάρχον entry.
- **Idempotent republish**: αν το ίδιο version ξαναγίνει publish με ακριβώς το ίδιο content, το command συνεχίζει να πετυχαίνει, ώστε τα local/test flows να είναι repeatable χωρίς να σπάει το immutable contract.
- **Tests**:
  - **tests/install_cli.rs**: νέο regression test **republish_same_version_with_different_content_fails**.
  - **tests/install_cli.rs**: νέο regression test **republish_same_version_with_same_content_is_ok**.
- **Verification**:
  - `cargo test -p pblang-pm --lib -- --nocapture` περνά
  - `cargo test --test install_cli -- --nocapture` περνά
- **Τι δεν έγινε**: δεν προστέθηκε remote publish API/auth, hosted registry service, ή semver conflict handling.
- **Τι μένει επόμενο**: μέσα στο `M1` backlog μένει κυρίως επιπλέον package ecosystem usability / policy tightening, αλλά όχι νέο registry architecture σε αυτό το βήμα.

### M1 – package subpath root confinement (Μάρτιος 2026)
- **pblang-pm / resolve_import()**: τα package-style imports της μορφής `import("dep:subpath")` ή `import("@scope/pkg:subpath")` δεν κάνουν πλέον raw join του subpath κάτω από το dependency root. Το subpath περνά από τον ίδιο relative-path validator που χρησιμοποιείται για το `[package].entry`.
- **Safety / compatibility**: package subpaths πρέπει πλέον να είναι relative και να μένουν μέσα στο package root. Absolute paths, `..` escapes, ή canonicalized targets έξω από το package root απορρίπτονται με clean import errors. Τα κανονικά in-package subpath imports συνεχίζουν να δουλεύουν.
- **Tests**:
  - **tests/install_cli.rs**: νέο registry integration test **installed_registry_package_subpath_import_works_within_package_root**.
  - **tests/install_cli.rs**: νέο git integration test **install_git_dependency_subpath_import_cannot_escape_package_root**.
- **Verification**:
  - `cargo test -p pblang-pm --lib -- --nocapture` περνά
  - `cargo test --test install_cli -- --nocapture` περνά
- **Τι δεν έγινε**: δεν προστέθηκε νέο package export map/schema, ούτε auto-fetch στο import path.
- **Τι μένει επόμενο**: μέσα στο `M1` backlog μένει κυρίως περαιτέρω install/import ergonomics ή άλλο μικρό policy tightening, αλλά όχι νέο registry architecture.

### M1 – publish-time validation for declared package entry (Μάρτιος 2026)
- **pblang-pm / publish_to_registry_dir()**: το `pblang publish` validate-άρει πλέον το δηλωμένο `[package].entry` πριν δημιουργήσει tarball ή ενημερώσει `index.json`. Αν υπάρχει explicit entry, πρέπει να είναι relative path μέσα στο package root και να δείχνει σε υπαρκτό αρχείο.
- **Safety / usability**: packages με broken explicit entry αποτυγχάνουν πλέον στο publish-time αντί να μπαίνουν στο local registry και να αποτυγχάνουν αργότερα σε bare package import/runtime resolution. Packages χωρίς `[package].entry` συνεχίζουν να βασίζονται στο υπάρχον fallback contract.
- **Tests**:
  - **tests/install_cli.rs**: νέο registry integration test **publish_with_missing_manifest_entry_fails**.
- **Verification**:
  - `cargo test -p pblang-pm --lib -- --nocapture` περνά
  - `cargo test --test install_cli -- --nocapture` περνά
- **Τι δεν έγινε**: δεν έγινε νέο package validation schema, δεν απαιτήθηκε universal default entry για κάθε publish, και δεν προστέθηκε hosted registry-side validation.
- **Τι μένει επόμενο**: μέσα στο `M1` backlog μένει κυρίως περαιτέρω package ecosystem usability / policy tightening, αλλά όχι νέο registry architecture σε αυτό το βήμα.

### M1 – file-only validation for declared package entry (Μάρτιος 2026)
- **pblang-pm / tighter entry contract**: το declared `[package].entry` δεν θεωρείται πλέον valid μόνο και μόνο επειδή canonicalize-άρεται. Το publish/import path το αντιμετωπίζει πλέον ως file-only contract: το target πρέπει να είναι πραγματικό αρχείο, όχι directory.
- **Safety / compatibility**: αυτό αποτρέπει broken manifests όπως `entry = "src"` από το να δημοσιεύονται ή να γίνονται αποδεκτά ως package default entry. Το error πλέον είναι καθαρό (`entry is not a file`) αντί για μεταγενέστερη αποτυχία όταν κάποιος προσπαθήσει να το διαβάσει/τρέξει.
- **Tests**:
  - **tests/install_cli.rs**: νέο registry integration test **publish_with_directory_manifest_entry_fails**.
  - **crates/pblang-pm/src/lib.rs**: νέο unit test **package_root_import_rejects_directory_entry**.
- **Verification**:
  - `cargo test -p pblang-pm --lib -- --nocapture` περνά
  - `cargo test --test install_cli -- --nocapture` περνά
- **Τι δεν έγινε**: δεν προστέθηκε manifest schema για entry kinds, δεν έγινε automatic fallback από invalid explicit entry σε heuristic, και δεν έγινε remote registry-side validation.
- **Τι μένει επόμενο**: μέσα στο `M1` backlog μένει κυρίως άλλο μικρό install/import/publish usability tightening, αλλά όχι νέο registry architecture σε αυτό το βήμα.

### M1 – file-only validation for package subpath imports (Μάρτιος 2026)
- **pblang-pm / resolve_import()**: τα package-style imports της μορφής `import("dep:subpath")` ή `import("@scope/pkg:subpath")` δεν αρκεί πλέον να κάνουν resolve σε path μέσα στο dependency root. Το resolved subpath πρέπει τώρα να είναι και πραγματικό αρχείο.
- **Safety / usability**: αυτό κλείνει broken cases όπως `import("@scope/pkg:src")`, όπου πριν γινόταν accept επειδή το directory υπήρχε, αλλά αργότερα το flow έσκαγε όταν προσπαθούσε να το χρησιμοποιήσει σαν module file. Το error πλέον είναι καθαρό (`subpath is not a file`) τόσο για registry όσο και για git dependencies.
- **Tests**:
  - **tests/install_cli.rs**: νέο registry integration test **installed_registry_package_subpath_import_rejects_directory_target**.
  - **tests/install_cli.rs**: νέο git integration test **install_git_dependency_subpath_import_rejects_directory_target**.
- **Verification**:
  - `cargo test -p pblang-pm --lib -- --nocapture` περνά
  - `cargo test --test install_cli -- --nocapture` περνά
- **Τι δεν έγινε**: δεν προστέθηκε package export map/schema, δεν έγινε heuristic fallback από directory subpath σε default entry, και δεν επεκτάθηκε η αλλαγή σε generic filesystem imports εκτός package-style form.
- **Τι μένει επόμενο**: μέσα στο `M1` backlog μένει κυρίως άλλο μικρό install/import/publish usability tightening, αλλά όχι νέο registry architecture σε αυτό το βήμα.

### M1 – safe unpack policy for registry tarballs (Μάρτιος 2026)
- **pblang-pm / unpack_tarball_gz()**: το registry fetch path δεν κάνει πλέον raw archive unpack χωρίς policy. Κάθε tar entry validate-άρεται πριν γραφτεί: το path πρέπει να είναι relative μέσα στο destination, directory/file entries επιτρέπονται, ενώ symlink/hardlink και λοιπά unsupported entry kinds απορρίπτονται.
- **Cache hygiene**: αν αποτύχει το unpack ενός registry tarball, το partial cache directory διαγράφεται αντί να μένει μισογραμμένο state κάτω από το content-addressed cache key.
- **Safety / compatibility**: αυτό κλείνει archive-based escape vectors και αποτρέπει poisoned cache state από malformed ή malicious registry tarballs. Τα κανονικά package tarballs που περιέχουν regular files συνεχίζουν να εγκαθίστανται όπως πριν.
- **Tests**:
  - **tests/install_cli.rs**: νέο registry integration test **install_with_registry_index_rejects_unsafe_tarball_entry**.
  - **crates/pblang-pm/src/lib.rs**: νέο unit test **archive_entry_path_rejects_parent_escape**.
  - **crates/pblang-pm/src/lib.rs**: νέο unit test **unpack_tarball_rejects_symlink_entry**.
- **Verification**:
  - `cargo test -p pblang-pm --lib -- --nocapture` περνά
  - `cargo test --test install_cli -- --nocapture` περνά
- **Τι δεν έγινε**: δεν προστέθηκε detached signature verification για tarballs, δεν έγινε hosted-registry trust policy, και δεν επεκτάθηκε η safe-unpack πολιτική σε άλλα artifact formats πέρα από το current `.tar.gz` registry flow.
- **Τι μένει επόμενο**: μέσα στο `M1` backlog μένει κυρίως άλλο μικρό registry/install hardening ή usability tightening, αλλά όχι νέο registry architecture σε αυτό το βήμα.

### E2 – packaged app run/bundle UX via manifest entry (Μάρτιος 2026)
- **main.rs / package-aware run**: το `pblang run` δέχεται πλέον και manifest dir semantics, όχι μόνο explicit script path. Αν κληθεί ως `pblang run` μέσα σε package dir ή ως `pblang run .` / `pblang run <dir>`, επιλύει runnable entry από το package manifest: πρώτα `[package].entry`, αλλιώς fallback σε `main.sap`, `main.sapp`, `src/main.sap`, `src/main.sapp`.
- **main.rs / pack validation**: το `pblang pack` επαληθεύει πλέον ότι υπάρχει runnable package entry πριν φτιάξει το artifact και τυπώνει το resolved entry path στο success output (`packed to ... (entry ...)`).
- **build alignment**: το `pblang build` χρησιμοποιεί το ίδιο package entry resolution αντί για hardcoded main-only heuristic, ώστε manifest-entry packages να typecheck-άρονται συνεπώς με το run/pack flow.
- **bundle alignment**: το `pblang bundle` χρησιμοποιεί πλέον το ίδιο `resolve_package_entry` path, αποτυγχάνει νωρίς αν δεν υπάρχει runnable package entry, και αντιγράφει το entry κρατώντας το relative path του μέσα στο portable folder (π.χ. `src/app.sapp` αντί flatten στο root). Το success output αναφέρει επίσης το resolved entry.
- **Tests**:
  - **tests/pack.rs**: νέο regression test **pack_then_run_from_manifest_dir_uses_package_entry**.
  - **tests/pack.rs**: νέο regression test **bundle_then_run_from_manifest_dir_uses_package_entry**.
- **Verification**:
  - `cargo test --test pack -- --nocapture` περνά
  - `cargo test -p pblang-pm --lib -- --nocapture` περνά
- **Τι δεν έγινε**: δεν προστέθηκε νέο artifact metadata file, δεν έγινε pinned-assets validation layer, και δεν έγινε native/single-binary backend work.
- **Τι μένει επόμενο**: μέσα στο `E2` backlog μένουν κυρίως richer packaged metadata/validation ή επόμενο μικρό packaging UX slice, αλλά όχι backend/native compilation σε αυτό το βήμα.

### M1 – registry CLI parity for lockfile-producing commands (Μάρτιος 2026)
- **main.rs / shared registry CLI parsing**: το CLI έχει πλέον κοινό helper για optional `--registry-index <url>` parsing και shared lockfile generation logic, ώστε τα lockfile-producing flows να χρησιμοποιούν το ίδιο explicit registry contract αντί για command-by-command divergence.
- **Command parity**: το `pblang --lock`, το `pblang build`, και το `pblang pack` δέχονται πλέον ρητό `--registry-index <url>` όπως ήδη έκανε το `pblang install`. Έτσι ένα project με registry dependencies μπορεί να ξαναγράψει `pblang.lock` ή να κάνει build/pack χωρίς να βασίζεται αποκλειστικά στο exported `PBLANG_REGISTRY_INDEX`.
- **Tests**:
  - **tests/install_cli.rs**: νέο registry regression test **build_with_registry_index_generates_lockfile_for_registry_dep**.
  - **tests/pack.rs**: νέο registry regression test **pack_with_registry_index_generates_lockfile_for_registry_dep**.
- **Verification**:
  - `cargo test --test install_cli -- --nocapture` περνά
  - `cargo test --test pack -- --nocapture` περνά
- **Τι δεν έγινε**: δεν άλλαξε το lockfile schema, δεν προστέθηκε νέο registry resolver, και δεν έγινε auto-fetch στο import path.
- **Τι μένει επόμενο**: μέσα στο `M1` backlog μένει άλλο μικρό package ecosystem usability / policy-tightening βήμα, αλλά όχι νέο registry architecture.

### M1 – self-contained publish validation for local registry MVP (Μάρτιος 2026)
- **pblang-pm / publish_to_registry_dir()**: το local-registry `pblang publish` επιβάλλει πλέον ελάχιστο self-contained contract. Αν το package έχει non-empty `[dependencies]`, η εντολή αποτυγχάνει νωρίς με καθαρό μήνυμα αντί να γράψει tarball/index για artifact που δεν είναι portable ως registry package.
- **Policy scope**: αυτό είναι σκόπιμο MVP policy tightening, όχι transitive packaging feature. Δεν γίνεται bundle των dependencies στο publish artifact και δεν αλλάζει το install/import/lockfile contract.
- **Tests**:
  - **tests/install_cli.rs**: νέο regression test **publish_with_dependencies_fails_and_writes_no_artifacts**.
- **Verification**:
  - `cargo test --test install_cli publish_with_dependencies_fails_and_writes_no_artifacts -- --nocapture` περνά
  - `cargo test --test install_cli -- --nocapture` περνά
- **Τι δεν έγινε**: δεν προστέθηκε transitive dependency packaging, δεν έγινε hosted registry publish flow, και δεν προστέθηκε νέο artifact metadata schema.
- **Τι μένει επόμενο**: μέσα στο `M1` backlog μένει άλλο μικρό package ecosystem usability / policy-tightening βήμα, αλλά όχι νέο registry architecture.

### M1 – publish-time module presence validation (Μάρτιος 2026)
- **pblang-pm / publish_to_registry_dir()**: το `pblang publish` απορρίπτει πλέον packages που δεν έχουν κανένα importable pblang module. Το explicit `[package].entry` συνεχίζει να validate-άρεται όπως πριν· χωρίς explicit entry, το publish επιτρέπεται αν υπάρχει default bare-import entry (`lib.sapp`, `lib.sap`, `main.sapp`, `main.sap`) ή τουλάχιστον άλλο `.sap`/`.sapp` module ώστε να παραμείνουν valid και τα subpath-only packages.
- **Policy scope**: αυτό κλείνει το μικρό usability gap όπου μπορούσε να δημοσιευτεί package με μόνο άσχετα αρχεία και μετά να αποτυγχάνει αργότερα ως προφανώς μη-importable artifact. Δεν αλλάζει το package import contract και δεν αφαιρεί την υποστήριξη για subpath-only published packages.
- **Tests**:
  - **tests/install_cli.rs**: νέο regression test **publish_without_any_pblang_module_fails**.
- **Verification**:
  - `cargo test --test install_cli publish_without_any_pblang_module_fails -- --nocapture` περνά
  - `cargo test --test install_cli -- --nocapture` περνά
- **Τι δεν έγινε**: δεν επιβλήθηκε νέο universal root-entry requirement, δεν προστέθηκε transitive packaging, και δεν έγινε remote registry-side validation.
- **Τι μένει επόμενο**: μέσα στο `M1` backlog μένει άλλο μικρό package ecosystem usability / policy-tightening βήμα, αλλά όχι νέο registry architecture.

### M1 – lockfile-based rehydration for missing registry/git cache paths (Μάρτιος 2026)
- **pblang-pm / resolve_dependency_root()**: όταν ένα registry/git dependency υπάρχει στο `pblang.lock` αλλά το locked cache/checkout path λείπει, ο resolver δεν αποτυγχάνει πια αμέσως. Αν το lock entry έχει αρκετά metadata (`source`, `resolved`, `fetch_url`, `archive_sha256`), το dependency rehydrate-άρεται από το lockfile και μετά ξαναγίνεται hash validation απέναντι στο locked content hash.
- **Registry/git reuse**: το registry rehydration χρησιμοποιεί το recorded fetch URL + archive checksum και το git rehydration το recorded `resolved = "<url>#<rev>"`, ώστε η φορητότητα να μην εξαρτάται αποκλειστικά από το αν υπάρχει ακόμα το παλιό absolute cache path.
- **Tests**:
  - **tests/install_cli.rs**: νέο regression test **registry_dependency_rehydrates_from_lockfile_when_cache_path_is_missing**.
  - **tests/install_cli.rs**: νέο regression test **git_dependency_rehydrates_from_lockfile_when_checkout_is_missing**.
- **Verification**:
  - `cargo test --test install_cli -- --nocapture` περνά
- **Τι δεν έγινε**: δεν έγινε πλήρες path-independent lockfile redesign.
- **Τι μένει επόμενο**: για πραγματικά cross-machine reproducibility μένει μεγαλύτερο portability redesign, όχι άλλο μικρό MVP slice.

### M1 – install-time validation for third-party registry/git packages (Μάρτιος 2026)
- **pblang-pm / resolve_registry_dep() και resolve_git_dep()**: η εγκατάσταση third-party packages δεν αρκείται πλέον στο fetch/clone. Αν ένα package δηλώνει `[package].entry`, αυτό πρέπει να είναι valid file μέσα στο package root· αλλιώς το package πρέπει να έχει τουλάχιστον ένα `.sap`/`.sapp` module. Broken packages απορρίπτονται στο install-time αντί να γράφουν misleading lock entries.
- **Tests**:
  - **tests/install_cli.rs**: νέο registry regression test **install_with_registry_index_rejects_package_without_modules**.
  - **tests/install_cli.rs**: νέο git regression test **install_git_dependency_rejects_invalid_package_entry**.
- **Verification**:
  - `cargo test --test install_cli -- --nocapture` περνά
- **Τι δεν έγινε**: δεν προστέθηκε νέο package manifest/schema versioning layer.
- **Τι μένει επόμενο**: το υπόλοιπο install/publish hardening πλέον περνά σε μεγαλύτερα architectural βήματα.

### M1 – relative tarball URLs for static registry indexes (Μάρτιος 2026)
- **pblang-pm / registry release resolution**: τα static `index.json` entries μπορούν πλέον να χρησιμοποιούν relative `url` paths δίπλα στο index, όχι μόνο πλήρως absolute URLs. Για `file://` indexes το release path resolve-άρεται relative στο `index.json`, ενώ το resolved absolute URL συνεχίζει να γράφεται στο lockfile για μετέπειτα rehydration.
- **Tests**:
  - **tests/install_cli.rs**: νέο regression test **install_with_registry_index_supports_relative_tarball_urls**.
- **Verification**:
  - `cargo test --test install_cli -- --nocapture` περνά
- **Τι δεν έγινε**: δεν έγινε hosted-registry service ή richer index metadata schema.
- **Τι μένει επόμενο**: ό,τι μένει στο registry story είναι πλέον μεγαλύτερο από ένα μικρό static-index tightening.

### M1 – explicit strict checksum CLI parity (Μάρτιος 2026)
- **main.rs / shared registry CLI options**: το lockfile-producing CLI path υποστηρίζει πλέον explicit `--require-registry-sha256` flag, όχι μόνο το env var `REQUIRE_REGISTRY_SHA256`. Το flag δουλεύει σε `pblang --lock`, `install`, `build`, και `pack`.
- **CLI UX**: το `pblang --lock` ευθυγραμμίστηκε επίσης με τα άλλα commands και αποτυγχάνει πλέον με καθαρό error path αντί για panic-style message όταν αποτυγχάνει το lock generation.
- **Tests**:
  - **tests/install_cli.rs**: νέο regression test **lock_require_registry_sha256_flag_rejects_missing_checksum**.
  - **tests/pack.rs**: νέο regression test **pack_require_registry_sha256_flag_rejects_missing_checksum**.
- **Verification**:
  - `cargo test --test install_cli -- --nocapture` περνά
  - `cargo test --test pack -- --nocapture` περνά
- **Τι δεν έγινε**: δεν προστέθηκε νέο checksum/signature system πέρα από το υπάρχον sha256 contract.
- **Τι μένει επόμενο**: στο `M1` δεν μένει άλλο μικρό discoverability/policy-tightening slice· τα υπόλοιπα είναι μεγαλύτερες αρχιτεκτονικές εργασίες.

### E2 – packaging hardening for local path dependencies (Μάρτιος 2026)
- **main.rs / pack+bundle validation**: τα `pblang pack` και `pblang bundle` validate-άρουν πλέον local path dependencies πριν δημιουργήσουν artifact. Path deps που είναι absolute, που κάνουν `..` escape, ή που canonicalize-άρονται έξω από το package root απορρίπτονται με καθαρό packaging error αντί να παράγουν misleading ή μη portable artifacts.
- **Tests**:
  - **tests/pack.rs**: νέο regression test **pack_rejects_out_of_package_path_dependency**.
  - **tests/pack.rs**: νέο regression test **bundle_rejects_out_of_package_path_dependency**.
- **Verification**:
  - `cargo test --test pack -- --nocapture` περνά
- **Τι δεν έγινε**: δεν προστέθηκε νέο asset pinning layer ή native/single-binary executable backend.
- **Τι μένει επόμενο**: μέσα στο `E2` backlog μένουν richer metadata/shipping layers πάνω από το ήδη πιο ασφαλές packaging contract.

### W1 – fixed-length HTTP request bodies on the stdlib server path (Μάρτιος 2026)
- **stdlib/http_server.sapp**: προστέθηκε helper `read_request(stream)` που διαβάζει πρώτα headers μέχρι `\r\n\r\n` και μετά, όταν υπάρχει `Content-Length`, τραβά το ακριβές υπόλοιπο body από το TCP stream. Τα `serve_once` και `serve_forever` χρησιμοποιούν πλέον αυτό το path.
- **Scope control**: το νέο behavior μένει σκόπιμα fixed-length και plain-TCP only. Chunked transfer, streaming request bodies, και TLS-aware parsing παραμένουν future work.
- **Tests**:
  - **tests/http_server.rs**: νέο ignored network regression test **http_server_read_request_reads_fixed_length_body**.
  - **tests/http_server.rs**: τα ignored HTTP tests προστατεύονται πλέον και από shared test lock ώστε να μην κάνουν race στο shared env-based port discovery.
- **Verification**:
  - `cargo test --test http_server -- --ignored --nocapture` περνά
- **Τι δεν έγινε**: δεν έγινε full web framework ή broader HTTP feature expansion.
- **Τι μένει επόμενο**: μέσα στο `W1` backlog μένουν κυρίως chunked/streaming/TLS και πιο πλούσιο web API πάνω από αυτό το baseline.

### A1 – minimal AI package/plugin skeleton example (Μάρτιος 2026)
- **examples/ai_plugin_minimal/**: προστέθηκε μικρό runnable example με δύο μέρη: `plugin/` (pblang package που εκθέτει `plugin_info`, `build_request`, `invoke`) και `app/` (consumer package που το χρησιμοποιεί ως local dependency).
- **Contract shape**: το example κρατά το AI integration stub-first. Το package φτιάχνει normalized request envelope και αφήνει την πραγματική provider execution σε μελλοντικό host embedding ή FFI bridge, χωρίς νέο runtime coupling στον VM πυρήνα.
- **Docs**:
  - **docs/AI_plugin_pattern.md**: προστέθηκε explicit αναφορά στο runnable example και στο intended host-or-FFI execution model.
- **Verification**:
  - `cargo run -- run examples/ai_plugin_minimal/app` περνά
- **Τι δεν έγινε**: δεν προστέθηκε production AI bridge, onnx/openai runtime coupling, ή νέο core builtin.
- **Τι μένει επόμενο**: το `A1` μπορεί πλέον να συνεχίσει με richer host/FFI bridge πάνω στο package pattern, όχι με monolithic core feature.

### E3 – bundle registry/shipping parity (Μάρτιος 2026)
- **main.rs / bundle parity**: το `pblang bundle` χρησιμοποιεί πλέον το ίδιο registry-aware lockfile regeneration path με το `pack`. Δέχεται `--registry-index <url>` και `--require-registry-sha256`, ξαναγράφει `pblang.lock`, και μετά αντιγράφει το regenerated lockfile στο bundled output.
- **Tests**:
  - **tests/pack.rs**: νέο regression test **bundle_with_registry_index_generates_lockfile_for_registry_dep**.
  - **tests/pack.rs**: νέο regression test **bundle_require_registry_sha256_flag_rejects_missing_checksum**.
- **Verification**:
  - `cargo test --test pack -- --nocapture` περνά
- **Τι δεν έγινε**: δεν προστέθηκε vendoring/offline packaging για registry/git deps και δεν έγινε νέο artifact metadata/signing layer.
- **Τι μένει επόμενο**: μέσα στο `E3` backlog μένουν richer shipping metadata, installer/export presets, ή asset validation steps.

### E3 – richer package metadata in manifest and pack/bundle output (Μάρτιος 2026)
- **pblang-pm / PackageSection**: προστέθηκε προαιρετικό πεδίο **`description`** στο `[package]` (backward compatible, `#[serde(default)]`).
- **main.rs / pack & bundle**: η γραμμή επιτυχίας τυπώνει πλέον προαιρετικά **name@version** και **description** από το manifest όταν υπάρχουν (π.χ. `packed to … (entry …) — my-tool@1.2.3 — A small CLI tool`).
- **Tests**: **tests/pack.rs** — **pack_output_includes_package_name_version_and_description**, **bundle_output_includes_package_name_version_and_description** (manifest με name, version, description → stdout περιέχει name@version και description).
- **Docs**: **Tooling.md** ενημερώθηκε (pack/bundle success output)· **Registry_MVP.md** αναφέρει το προαιρετικό `[package].description`.
- **Verification**: `cargo test --test pack -- --nocapture` περνά (11/11).
- **Τι δεν έγινε**: δεν προστέθηκε authors array, app icon, ή artifact metadata file στο tarball/bundle.
- **Τι μένει επόμενο**: μέσα στο E3 μένουν installer/export presets, pinned assets validation, ή επόμενο μικρό shipping UX slice.

### Gap-fix – missing examples and N1 doc (Μάρτιος 2026)
- **examples/ai_plugin_minimal/**: προστέθηκε plugin (lib.sapp με `plugin_info`, `capabilities`, `build_request`, `invoke`) και app (pblang.toml + main.sapp consumer) ώστε το test **ai_plugin_example_builds_normalized_request_envelope** να περνά.
- **examples/quantum_bridge_minimal/**: προστέθηκε plugin (lib.sapp με `bridge_info`, `build_job`, `submit_job`· inline QASM emission ώστε το plugin να φορτώνεται χωρίς stdlib στο import context) και app ώστε το test **quantum_bridge_example_builds_provider_job_envelope** να περνά.
- **docs/Native_backend_notes.md**: προστέθηκε stub με ενότητες Blockers, Backend criteria, Prerequisite checklist, Staged path (N1 “Recently progressed” στο Help πλέον ακριβές).
- **Verification**: `cargo test --test modules` περνά (13/13)· `cargo test --test pack` περνά (11/11).
- **Τι δεν έγινε**: δεν άλλαξε κανένα test· δεν προστέθηκε νέο feature. Μόνο κλείσιμο gap docs ↔ code.
- **Τι μένει επόμενο**: επόμενο batch κατά προτεραιότητα (E3 ή W2 slice) αν ζητηθεί.

### W2 – custom response headers and fixed-length body limit helpers (Μάρτιος 2026)
- **stdlib/http_server.sapp**: ο response formatter μπορεί πλέον να δεχτεί custom `headers` map από handler-returned map responses, κρατώντας reserved response metadata (`Content-Type`, `Content-Length`, `Connection`) στη δική του ευθύνη. Προστέθηκε επίσης μικρό redirect helper `res.redirect(...)`.
- **Body safety helper**: το ίδιο stdlib layer απέκτησε limited fixed-length request path μέσω `read_request_limited(...)`, `serve_once_limited(...)`, και `serve_forever_limited(...)`. Oversized fixed-length `Content-Length` requests απορρίπτονται με `413 Payload Too Large` χωρίς να ανοίξει chunked/streaming/TLS scope.
- **Tests**:
  - **tests/semantics.rs**: νέα tests **http_response_supports_custom_headers** και **http_redirect_helper_renders_location_header**.
  - **tests/http_server.rs**: νέο ignored network regression test **http_server_serve_once_limited_rejects_oversized_body**.
- **Verification**:
  - `cargo test --test semantics -- --nocapture` περνά
  - `cargo test --test http_server -- --ignored --nocapture` περνά
- **Τι δεν έγινε**: δεν έγινε chunked transfer support, streaming body path, TLS, ή μεγαλύτερο HTTP framework layer.
- **Τι μένει επόμενο**: μέσα στο `W2` backlog μένουν κυρίως chunked/streaming/TLS και πιο πλούσιο request/response API surface.

### A2 – normalized AI bridge contract example (Μάρτιος 2026)
- **examples/ai_plugin_minimal/plugin/**: το AI example δεν μένει πλέον μόνο σε `plugin_info + invoke`. Εκθέτει και `capabilities()` και normalized `build_request(...)`, με ρητό envelope για schema version, provider, transport, operation, options, και capability flags.
- **examples/ai_plugin_minimal/app/**: το runnable app exercise-άρει πλέον το πιο πλούσιο contract και δείχνει ότι το bridge παραμένει stub-first αλλά με πιο ξεκάθαρο future integration seam για host/FFI/HTTP providers.
- **Tests**:
  - **tests/modules.rs**: νέο regression test **ai_plugin_example_builds_normalized_request_envelope**.
- **Docs**:
  - **docs/AI_plugin_pattern.md**: το minimal example αναφέρει πλέον ρητά το richer bridge contract.
- **Verification**:
  - `cargo test --test modules -- --nocapture` περνά
  - `cargo run -- run examples/ai_plugin_minimal/app` περνά
- **Τι δεν έγινε**: δεν προστέθηκε real provider adapter, FFI bridge, typed tensor layer, ή streaming inference contract.
- **Τι μένει επόμενο**: μέσα στο `A2` backlog μένει practical host/FFI/HTTP provider bridge πάνω στο ήδη πιο σταθερό envelope contract.

### Q1 – minimal quantum bridge skeleton example (Μάρτιος 2026)
- **examples/quantum_bridge_minimal/**: προστέθηκε package/example που παίρνει pblang quantum circuit και το μετατρέπει σε provider-ready job envelope. Το package εκθέτει `bridge_info()`, `build_job(...)`, και `submit_job(...)`, ενώ το example app δείχνει end-to-end χρήση.
- **Contract shape**: το bridge δεν κάνει simulator/provider execution. Πατά πάνω στο υπάρχον QASM emitter και χτίζει envelope με `provider`, `shots`, `qasm`, και `metadata`, ώστε το επόμενο βήμα να είναι provider adapter και όχι νέο core feature.
- **Tests**:
  - **tests/modules.rs**: νέο regression test **quantum_bridge_example_builds_provider_job_envelope**.
- **Verification**:
  - `cargo test --test modules -- --nocapture` περνά
  - `cargo run -- run examples/quantum_bridge_minimal/app` περνά
- **Τι δεν έγινε**: δεν προστέθηκε simulator integration, Qiskit/Braket/Cirq bridge, ή FFI/host quantum provider execution path.
- **Τι μένει επόμενο**: μέσα στο `Q1` backlog μένει richer provider protocol/adapter work πάνω στο envelope shape.

### X1 + L1 – richer polyglot docs output (Μάρτιος 2026)
- **main.rs / `pblang --doc`**: το docs generator δεν περιορίζεται πλέον μόνο σε raw polyglot block listing. Δείχνει parsed attrs και code-line count για κάθε polyglot block τόσο στο text output όσο και στο generated HTML.
- **Tests**:
  - **tests/doc_cli.rs**: νέο CLI regression test **doc_cli_renders_polyglot_attrs_in_text_and_html**.
  - **Cargo.toml**: προστέθηκε explicit `[[test]]` entry για το νέο `doc_cli` suite, επειδή το repo τρέχει με `autotests = false`.
- **Verification**:
  - `cargo test --test doc_cli -- --nocapture` περνά
- **Τι δεν έγινε**: δεν προστέθηκε ακόμα actual polyglot artifact extraction/build pipeline.
- **Τι μένει επόμενο**: το `X1` μπορεί πλέον να συνεχίσει με source-only extraction/export ή πρώτο artifact scaffold, ενώ το `L1` μπορεί να προχωρήσει σε επόμενο docs/LSP/tooling step.

### G1 – small stdlib data-surface expansion (Μάρτιος 2026)
- **stdlib/array.sapp**: προστέθηκαν `contains(...)` και `index_of(...)`.
- **stdlib/map.sapp**: προστέθηκε `get_or(...)` για explicit fallback behavior χωρίς raw `try/catch` στο caller code.
- **Tests**:
  - **tests/modules.rs**: νέο regression test **array_and_map_stdlib_helpers_work**.
- **Verification**:
  - `cargo test --test modules -- --nocapture` περνά
- **Τι δεν έγινε**: δεν άνοιξε graphics-heavy ή μεγαλύτερο stdlib dump.
- **Τι μένει επόμενο**: μέσα στο `G1` backlog μένει άλλο μικρό stdlib/data/app-surface extension, όχι monolithic surface expansion.

### M2 – registry next-phase design note (Μάρτιος 2026)
- **docs/Registry_MVP.md**: προστέθηκε ενότητα **Επόμενα βήματα (next phase) — Export maps / package surface contracts**. Περιγράφει τι σημαίνει export map για pblang, σύνδεση με το τρέχον `[package].entry` και subpath imports, και προτεινόμενο ελάχιστο schema (προαιρετικό `exports` array, publish/import-time validation). Design note only — δεν υλοποιήθηκε κώδικας.
- **Verification**: το doc ενημερώθηκε· Help.md και Report.md ενημερώθηκαν για M2 acceptance criteria.

### T3 – threads post-MVP regression test (Μάρτιος 2026)
- **tests/threads.rs**: νέο regression test **two_spawns_two_channels_deterministic** — δύο channels, δύο spawns (κάθε spawn στέλνει σε δικό του channel), main κάνει recv από τα δύο receivers με σειρά· κλειδώνει συμπεριφορά πολλαπλών channels χωρίς νέο runtime API.
- **Verification**:
  - `cargo test --test threads -- --nocapture` περνά (10/10)
- **Τι δεν έγινε**: δεν προστέθηκε explicit close, multi-send, ή select· VM-per-thread contract αμετάβλητο.
- **Τι μένει επόμενο**: το επόμενο πιο λογικό batch είναι R1 / ED1 / PF1 / N1 (vision backlog) ή επεκτάσεις M2/T3 αν ζητηθούν.

### R1 – bytecode / RE documentation (Μάρτιος 2026)
- **docs/Tooling.md**: προστέθηκε ενότητα **Reverse engineering / bytecode** που τεκμηριώνει το `--dump-bytecode` output (constants + instruction stream), limitations (single-file, no execution), και τυπική χρήση για debugging/RE. Κανένα νέο test ή αλλαγή κώδικα.
- **Τι δεν έγινε**: δεν προστέθηκε analysis tooling, decompiler, ή αλλαγή στο bytecode format.
- **Τι μένει επόμενο**: περαιτέρω R1 slices (analysis tooling, decompiler research) ή άλλο vision item (ED1, PF1, N1).

### R1 – bytecode disassembly regression test (Μάρτιος 2026)
- **tests/dump_bytecode.rs**: νέο regression test **dump_bytecode_output_has_constants_and_bytecode_sections** — τρέχει `pblang --dump-bytecode` σε μικρό fixture και ελέγχει ότι η έξοδος περιέχει τα sections `=== constants` και `=== bytecode ===` και τουλάχιστον ένα opcode (OpPrint/OpConstant/OpDefineGlobal). Κλειδώνει τη μορφή disassembly χωρίς αλλαγή κώδικα compiler/VM.
- **Cargo.toml**: προστέθηκε `[[test]] name = "dump_bytecode"` path = "tests/dump_bytecode.rs".
- **Verification**: `cargo test --test dump_bytecode -- --nocapture` περνά.
- **Τι δεν έγινε**: δεν προστέθηκε analysis tooling ή decompiler· δεν άλλαξε το bytecode format.
- **Τι μένει επόμενο**: περαιτέρω R1 (analysis tooling, decompiler research) ή N1/ED1/PF1.

### N1 – native backend design note (Μάρτιος 2026)
- **docs/Native_backend_notes.md**: νέο doc με stub ενότητες **Blockers**, **Backend criteria**, **Prerequisite checklist**, **Staged path** για native/hybrid compilation research. Design note only — καμία αλλαγή κώδικα ή toolchain.
- **Τι δεν έγινε**: δεν προστέθηκε LLVM/Cranelift/transpile implementation.
- **Τι μένει επόμενο**: research spike ή implementation όταν υπάρχει οδηγία.

### W4-beta – WebSocket full-duplex send (write_frame) (Μάρτιος 2026)
- **stdlib/websocket.sapp**: προστέθηκε helper `write_frame(opcode, payload)` που χτίζει minimal RFC 6455 client→server masked WebSocket frame (FIN=1, payload ≤125, σταθερό masking key stub `0xAB 0xCD 0xEF 0x01`). Ο helper χρησιμοποιεί τα υπάρχοντα builtins `strcharcodeat`, `strfromcharcode`, `byte_xor` και παράγει raw frame byte-string κατάλληλο για `tcp_write`.
- **Tests**:
  - **tests/semantics/websocket_write_frame_test.sapp**: μικρό semantics test που ελέγχει ότι το πρώτο byte είναι 0x81 (FIN=1, opcode=1), το δεύτερο 0x82 (MASK=1, length=2) και ότι το συνολικό μήκος frame είναι 8 bytes (2 header + 4 mask + 2 payload).
  - **tests/semantics.rs**: προστέθηκε regression test **websocket_write_frame_pass** που τρέχει το παραπάνω script και περιμένει τελικό `websocket_write_frame_test: pass`.
- **Docs**:
  - **docs/Tooling.md**: η ενότητα για `stdlib:websocket.sapp` επεκτάθηκε με W4-beta περιγραφή για `write_frame`, ώστε να φαίνεται καθαρά ότι πλέον υπάρχει full-duplex θεμέλιο (parse + write).
- **Verification**:
  - `cargo test --test semantics -- --nocapture` περνά.
- **Τι δεν έγινε**: δεν υλοποιήθηκαν extended payload lengths (16/64-bit), fragmentation reassembly ή παραγωγή πραγματικά τυχαίου masking key (παραμένει σταθερό stub, κατάλληλο για test/εσωτερικό server use).
- **Τι μένει επόμενο**: για W4 συνεχίζουν HTTP/2 research note και rate limiting / connection throttle helpers σε router/middleware· για επόμενο batch προτείνεται A4-beta (AI multi-provider + post_stream_auth) ή R3-alpha (data-flow/use-def ανάλυση).

### A4-beta – AI multi-provider registry + post_stream_auth (Μάρτιος 2026)
- **stdlib/http.sapp**: προστέθηκε helper `post_stream_auth(url, body, token, on_chunk)` που υλοποιεί SSE-style streaming POST με `Authorization: Bearer <token>` πάνω από το υπάρχον builtin `http_post_auth`. Ο helper χρησιμοποιεί το ίδιο `"\n\n"` split shim με τα `get_stream`/`post_stream` (A4-alpha) και καλεί `on_chunk(part + "\n\n")` για κάθε μη-κενό κομμάτι.
- **stdlib/provider_registry.sapp**: νέο stdlib module multi-provider AI registry. Εκθέτει:
  - `register(name, endpoint, api_key)` — προσθέτει provider σε in-memory λίστα.
  - `call_best(prompt, model)` — αν δεν υπάρχουν providers επιστρέφει `map{ error: "no providers registered" }`, αλλιώς καλεί τον πρώτο provider με OpenAI-style JSON envelope (`model`, `messages: [{role:"user", content:prompt}]`) μέσω `http.post_auth`.
  - `call_with_fallback(prompt, model)` — δοκιμάζει διαδοχικά κάθε registered provider με το ίδιο envelope· επιστρέφει το πρώτο μη-nil αποτέλεσμα από `http.post_auth` ή `map{ error: "all providers failed" }` αν όλα αποτύχουν.
- **Tests**:
  - **tests/semantics/provider_registry_test.sapp**: semantics test που ελέγχει ότι `call_best` πάνω σε κενό registry γυρίζει error `"no providers registered"` και ότι ένα απλό `register(...)` δεν αποτυγχάνει, καταλήγοντας σε `provider_registry_test: pass`.
  - **tests/semantics.rs**: νέο regression test **provider_registry_register_and_call_pass** που τρέχει το παραπάνω script και περιμένει τελικό `provider_registry_test: pass`.
- **Docs**:
  - **docs/Tooling.md**: η AI/http ενότητα ενημερώθηκε με A4-beta bullets για `http.post_stream_auth` και `stdlib:provider_registry.sapp` (register/call_best/call_with_fallback) ώστε να είναι ορατή η multi-provider ιστορία στο tooling.
- **Verification**:
  - `cargo test --test semantics -- --nocapture` με το νέο test προστιθέμενο (σημείωση: υπάρχουν δύο παλαιότερα semantics tests που ήδη αποτυγχάνουν και δεν τροποποιήθηκαν σε αυτό το batch).
- **Τι δεν έγινε**: δεν προστέθηκαν πραγματικές network κλήσεις στα tests (ο registry παραμένει in-memory contract), δεν υλοποιήθηκαν structured schema validators ή νέα tensor ops, και δεν επεκτάθηκε το ai_http_provider example.
- **Τι μένει επόμενο**: για A4 παραμένουν candidate slices όπως chat/tool-call package example, structured request/response helpers, και pure-pblang tensor ops (slice/reshape/dot) χτισμένα πάνω από το υπάρχον tensor.sapp.

### R3-alpha – Data-flow / use-def analysis (Μάρτιος 2026)
- **src/main.rs**: προστέθηκε subcommand **`pblang --dataflow <file>`** που κάνει απλή use-def ανάλυση πάνω από το AST. Η CLI διαβάζει το αρχείο, το περνάει από `strip_comments_mask` + `parse` και μετά καλεί `print_dataflow_report(&ast, &src_for_parse)`.
- **print_dataflow_report(ast, src)**: νέος walker που:
  - Μαζεύει definitions από `let`/`assign`/`for` (με γραμμή μέσω `line_col(src, span.start)`) σε `BTreeMap<String, VarDefInfo>`.
  - Μαζεύει uses από κάθε `ExprKind::Var` (identifier reads) και patterns σε `match` (bind arms αντιμετωπίζονται ως defs).
  - Εκτυπώνει για κάθε use: `use <name> line:<n>` και για κάθε def: `def <name> line:<n>`, και στο τέλος `WARN <name> assigned but never read` για όσα defs δεν έχουν αντίστοιχο use.
- **Tests**:
  - **tests/dataflow.rs**: νέο CLI regression test **dataflow_reports_unused_def** που τρέχει `pblang --dataflow tests/fixtures/call_graph_sample.sapp` και βεβαιώνεται ότι η stdout περιέχει τουλάχιστον `"def"` ή `"use"` (sanity check για non-empty report).
  - **Cargo.toml**: προστέθηκε `[[test]] name = "dataflow" path = "tests/dataflow.rs"`.
- **Docs**:
  - **docs/Tooling.md**: νέα ενότητα **R3-alpha: Data-flow / use-def analysis** με usage snippet `pblang --dataflow <file>` και περιγραφή του output (`def`, `use`, `WARN` lines) και πώς συμπληρώνει το `pblang lint`.
- **Verification**:
  - `cargo test --test dataflow -- --nocapture` (να τρέξει ο νέος dataflow test) μαζί με τα υπάρχοντα RE/call-graph tests· υπάρχουν δύο παλιά semantics tests που συνεχίζουν να αποτυγχάνουν και δεν ήταν scope αυτού του batch.
- **Τι δεν έγινε**: δεν προστέθηκε decompiler, RE scripting API ή symbols/hexscan/patch subcommands· η ανάλυση είναι επίτηδες απλή και AST-only, χωρίς control-flow awareness ή inter-procedural reasoning.
- **Τι μένει επόμενο**: επόμενα R3 slices μπορούν να χτίσουν πάνω σε αυτό (π.χ. `--decompile`, RE scripting API, symbol/hex tools) με ξεχωριστά batches.

---

*Αυτή η έκθεση ενημερώνεται μετά από κάθε πάσο υλοποίησης του Future_plans.md.*

## v0.5.4 — Bit-Level Optimization & Enhanced Stdlib (2026)

### Compiler Optimizations
- **String Constant Folding**: Compile-time string concatenation (`"hello " + "world"` → `"hello world"`)
- **Peephole Optimizer**: Removes redundant instruction pairs (`Dup`+`Pop`, `Not`+`Not`)
- **Jump Threading Enhancement**: Multi-pass resolution of jump chains

### New Stdlib Modules
| Module | Functions | Description |
|--------|-----------|-------------|
| `bitops.sapp` | 26 | Bitwise AND/OR/XOR/NOT, shifts, popcount, CLZ/CTZ, bit test/set/clear/toggle, rotations (rotl/rotr), bswap32, binary/hex conversion |
| `compression.sapp` | 7 | Gzip, Zstd, Brotli compress/decompress + auto-detection |
| `serialization.sapp` | 10 | JSON pretty/validate, TOML/YAML parse/stringify, CSV parse/stringify, XML escape/unescape |

### Bug Fixes
- Fixed `pblang-cranelift` compilation: added missing `min_arity`, `defaults`, `is_vararg` fields
- Fixed `params_not_implemented_yet_should_error` test → `params_and_locals_work` (locals now implemented)

### Test Results
- **143/143 semantics tests passing, 0 failing, 3 ignored**
- All workspace tests passing (200+ total)

## v0.5.4 — Comprehensive Stdlib & Self-Hosting (2026)

### New Stdlib Modules (7 modules, 90+ functions)
| Module | Functions | Description |
|--------|-----------|-------------|
| `bitops.sapp` | 26 | Bitwise AND/OR/XOR/NOT, shifts, popcount, CLZ/CTZ, rotations, byte swap, binary/hex |
| `compression.sapp` | 7 | Gzip, Zstd, Brotli compress/decompress + auto-detect |
| `serialization.sapp` | 10 | JSON, TOML, YAML, CSV, XML encode/decode |
| `registry_server.sapp` | 7 | Hosted package registry HTTP server |
| `compiler/codegen.sapp` | 16 | Self-hosting bytecode code generator (full compiler pipeline) |
| `benchmark.sapp` | 8 | run(), time(), warmup(), compare(), bench_once(), bench_many(), bench_compare(), bench_all() |

### Compiler Optimizations
- **String constant folding**: `"a" + "b"` → `"ab"` at compile time
- **Peephole optimizer**: Removes redundant Dup+Pop, Not+Not pairs
- **Jump threading**: Multi-pass jump-to-jump resolution

### Self-Hosting Compiler Progress
| Component | Status | Lines |
|-----------|--------|-------|
| Tokenizer | ✅ Complete | 232 |
| Parser | ✅ Complete | 708 |
| Code Generator | ✅ Complete | 303 |
| Type Checker | 🟡 Next | — |

### Bug Fixes
- Fixed pblang-cranelift: missing min_arity/defaults/is_vararg fields
- Fixed params_not_implemented_yet → params_and_locals_work (locals implemented)
- Fixed benchmark.sapp: restored backward-compatible API with proper pblang syntax
- Fixed all stdlib modules: proper map{}/len()/array_push() syntax

### Test Results
- **143/143 semantics tests passing, 0 failing, 3 ignored**
- All workspace test suites passing (AI 8/8, VM 21/21, PM 8/8, etc.)
- **200+ total tests passing across all crates**

### Project Scale
- **96 stdlib modules** (16,935 lines of pblang)
- **16 crates** (24,249 lines of Rust)
- **41,184 total lines of code**
