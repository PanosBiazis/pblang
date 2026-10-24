# PBLang Complete Syntax, Compiler Internals, 240+ Builtins & AAP 112-Project Master Reference

Prepared & Verified on 2026-10-24 from `pblang/stdlib/compiler/{tokenizer,parser,builtins_list,semantic,typeck,codegen}.sapp`, `pblang/stdlib/` (726 modules), `pblang/examples/` (264 examples), `AAP/products/` (64 products), and `AAP/projects/` (48 projects).

---

## 1. PBLang Complete Language Syntax & Grammar (`tokenizer.sapp` & `parser.sapp`)

### 1.1 File Extensions & Entry Conventions
- Supported extensions: `.sapp` (canonical), `.pb`, `.pbx`, `.bp`, `.bpx`.
- Package manifest: `pblang.toml` + `pblang.lock` (`content_hash_algorithm = "pblang-package-tree-v2"`).

### 1.2 Keywords (33 Reserved Words)
- **Declarations & Bindings:** `fn`, `let`, `const`, `struct`, `class`, `enum`, `interface`
- **Control Flow:** `if`, `elif`, `else`, `while`, `for`, `in`, `loop`, `break`, `continue`, `return`, `match`, `case`, `defer`
- **Error Handling:** `try`, `catch`, `throw`
- **Modules:** `import`, `export`
- **Logical & Type Operators (Word Keywords):** `and`, `or`, `not`, `is`
- **Literals:** `true`, `false`, `nil`, `map`
- **Note:** `spawn` and `join` are global builtins (called as `spawn(fn)` / `join(task)`), and `print` is both a statement (`print expr;`) and in `builtin_set()`.

### 1.3 Operators & Critical Lexer Rules
- **Arithmetic:** `+`, `-`, `*`, `/`, `%`, `**`
- **Comparison:** `==`, `!=`, `<`, `>`, `<=`, `>=`
- **Logical:** Prefer keyword operators **`and`**, **`or`**, **`not`** (also `!` for unary negation). Avoid `||` / `&&` in `.sapp` scripts targeted at the Rust VM parser, which expects `and` / `or`.
- **Null-Coalescing & Optional Chaining:** `??`, `?.`, `?[`
- **Assignment:** `=`, `+=`, `-=`, `*=`, `/=`
- **Range & Punctuation:** `..`, `->`, `( )`, `{ }`, `[ ]`, `,`, `;`, `:`, `.`
- **Comments:** `// single-line`, `/* block */`, and `#` (line comment / shebang).
- **Identifiers:** Full ASCII `[A-Za-z_][A-Za-z0-9_]*` plus Unicode UAX#31 letters (Greek `π`, `λ`, `α`, Latin accented, Cyrillic, CJK, etc.).
- **Numbers:** Decimal (`42`, `3.14`, `1e10`) and radix literals (`0xFF`, `0b1010`, `0o77`).
- **String Escapes:** `\n`, `\t`, `\r`, `\"`, `\\`, `\0`, `\{`, `\}`, `\xHH`, `\uXXXX`, `\u{XXXXXX}`.

### 1.4 Statements & Expressions Idioms
```text
// 1. Variable & Constant bindings (with optional type annotations)
let x = 42;
const MAX_ITEMS: number = 1024;

// 2. Functions (with optional default parameters & return types)
fn compute(a, b = 10) {
    return a + b;
}

// 3. Conditional statement AND conditional expression
if x > 10 {
    print "large";
} elif x > 0 {
    print "positive";
} else {
    print "non-positive";
}
let label = if x > 0 { "pos" } else { "neg" };

// 4. Loops
while x > 0 { x = x - 1; }
for item in [10, 20, 30] { print item; }
for i in range(0, 10) { print i; }

// 5. Try / Catch (both statement and inline expression!)
let parsed = try json_parse(raw) catch nil;
let is_dir = try { fs_listdir(path); true } catch false;

// 6. Maps & Arrays
let m = map{ id: "pbcloud", active: true, count: 4 };
if map_has(m, "id") {
    for k in map_keys(m) { print k + "=" + str(m[k]); }
}
// IMPORTANT: Indexing an undefined key `m["missing"]` throws a runtime error!
// Always check `map_has(m, key)` first or wrap in `try m[key] catch nil`.

// 7. Module Import & Export
let Crypto = import("stdlib:crypto.sapp");
let LocalMod = import("src/engine.sapp");
export compute, MAX_ITEMS; // comma-separated or individual `export ident;` statements
```

---

## 2. Authoritative 240+ Host Builtin Functions (`stdlib/compiler/builtins_list.sapp`)

### 2.1 Type Inspection & Assertions
- `type(v)` (`"number"`, `"string"`, `"bool"`, `"array"`, `"map"`, `"fn"`, `"nil"`)
- `is_number(v)`, `is_string(v)`, `is_bool(v)`, `is_array(v)`, `is_map(v)`, `is_fn(v)`, `is_nil(v)`, `is_frozen(v)`
- `require_type(v, t)`, `type_assert(v, t)`, `assert(cond, msg)`, `error(msg)`, `freeze(v)`

### 2.2 Strings & Regex
- `str(v)`, `len(s)`, `slice(s, start, end)`, `str_slice(s, start, end)`
- `str_contains(s, sub)`, `str_starts_with(s, prefix)`, `str_ends_with(s, suffix)`, `str_index_of(s, sub)`
- `str_split(s, delim)`, `str_join(arr, delim)`, `str_replace_all(s, from, to)`, `str_trim(s)`
- `str_lower(s)`, `str_upper(s)`, `str_pad_left(s, len, ch)`, `str_pad_right(s, len, ch)`, `str_repeat(s, n)`, `str_format(fmt, ...)`
- `str_to_bytes(s)`, `bytes_to_str(bytes)`, `strcharcodeat(s, idx)`, `strfromcharcode(code)`, `str_from_char_code(code)`
- `regex_match(pat, s)`, `regex_find(pat, s)`, `regex_find_all(pat, s)`, `regex_capture(pat, s)`, `regex_replace(pat, s, rep)`, `regex_replace_all(pat, s, rep)`, `regex_split(pat, s)`
- `parse_number(s)`, `parse_bool(s)`, `url_encode(s)`, `url_decode(s)`, `json_parse(s)`, `json_stringify(v)`

### 2.3 Arrays & Maps
- **Arrays:** `len(a)`, `push(a, v)`, `pop(a)`, `array_push(a, v)`, `array_pop(a)`, `array_sort(a)`, `array_unique(a)`, `array_chunk(a, n)`, `array_flatten(a)`, `array_take(a, n)`, `array_drop(a, n)`, `array_zip(a, b)`, `range(start, end)`, `enumerate(a)`, `sum(a)`, `product(a)`, `min_of(a)`, `max_of(a)`
- **Maps:** `map_has(m, k)`, `map_keys(m)`, `map_remove(m, k)`, `has_member(m, k)`

### 2.4 Math, Checked Arithmetic, Exact Decimal, BigInt & Bitwise
- `num_abs(x)`, `num_floor(x)`, `num_ceil(x)`, `num_sqrt(x)`, `num_pow(x, y)`, `num_exp(x)`, `num_ln(x)`, `num_atan2(y, x)`, `num_min(a, b)`, `num_max(a, b)`, `num_mod(a, b)`, `clamp(x, lo, hi)`
- `checked_add(a, b)`, `checked_sub(a, b)`, `checked_mul(a, b)`, `checked_div(a, b)`
- `decimal(v)`, `bigint(v)`, `rational(n, d)`, `num_to_le_bytes(x)`, `num_from_le_bytes(b)`
- `bit_and(a, b)`, `bit_or(a, b)`, `bit_xor(a, b)`, `bit_not(a)`, `bit_shl(a, n)`, `bit_shr(a, n)`

### 2.5 Filesystem, Paths, OS, Time & Subprocesses
- `fs_exists(p)`, `fs_read(p)`, `fs_read_bytes(p)`, `fs_write(p, s)`, `fs_write_bytes(p, b)`, `fs_append(p, s)`, `fs_listdir(p)`, `fs_mkdirs(p)`, `fs_remove(p)`, `fs_rename(from, to)`, `fs_copy(from, to)`
- `path_join(a, b)`, `path_dirname(p)`, `path_basename(p)`, `cwd()`, `platform()`
- `os_args()`, `os_exit(code)`, `os_which(bin)`, `env_get(k)`, `env_set(k, v)`, `proc_spawn_wait(cmd, args)`, `exec(cmd)`
- `os_signal_trap(sig)`, `os_signal_poll()`, `os_signal_pending()`, `os_signal_handler_get(sig)`
- `now_ms()`, `now_iso()`, `clock()`, `sleep_ms(ms)`, `read_line()`

### 2.6 Cryptography, Constant-Time Security & Encoding
- `sha256(s)`, `hmac_sha256(key, msg)`, `constant_time_eq(a, b)`
- `random_bytes(n)`, `random_float()`
- `base64_encode(s)`, `base64_decode(s)`, `base64_encode_bytes(b)`, `base64_decode_bytes(s)`, `hex_encode(b)`, `hex_decode(s)`
- Plus `stdlib:crypto.sapp` (`ChaCha20` RFC 8439, One-Time Pad) and `stdlib:aes_gcm.sapp`.

### 2.7 Networking, HTTP, TLS, WebSockets & SQLite
- `tcp_listen(addr)`, `tcp_listen_tls(addr, cert, key)`, `tcp_accept(srv)`, `tcp_connect(addr)`, `tcp_connect_tls(addr)`, `tcp_wrap_tls(sock)`, `tcp_read(sock, n)`, `tcp_read_until(sock, delim)`, `tcp_write(sock, data)`, `tcp_close(sock)`, `tcp_local_addr(sock)`
- `http_get(url)`, `http_get_auth(url, hdrs)`, `http_post(url, body)`, `http_post_auth(url, body, hdrs)`, `http_parse_request(raw)`, `http_response(code, body)`, `http_response_ct(code, ct, body)`, `http_response_json(code, map)`
- `websocket_accept_key(k)`, `websocket_parse_frame(b)`, `websocket_write_frame(op, b)`, `parse_sse_line(s)`
- `sqlite_open(path)`, `sqlite_exec(db, sql)`, `sqlite_query(db, sql)`, `sqlite_last_insert_rowid(db)`, `sqlite_close(db)`

### 2.8 Concurrency, Channels, Atomics, Memory & Lifetimes
- `spawn(fn)`, `spawn_n(n, fn)`, `join(t)`, `join_all(tasks)`, `gil_enabled()`
- `channel()`, `channel_buffered(cap)`, `send(ch, v)`, `recv(ch)`, `recv_timeout(ch, ms)`, `channel_close(ch)`, `channel_is_closed(ch)`, `channel_size(ch)`, `select(chs)`, `select_with_timeout(chs, ms)`, `ticker(ms)`, `timeout_channel(ms)`
- `mutex_new()`, `mutex_lock(m)`, `mutex_try_lock(m)`, `mutex_unlock(m)`, `rwlock_new()`, `rwlock_read(rw)`, `rwlock_write(rw)`, `rwlock_unlock(rw)`, `barrier_new(n)`, `barrier_wait(b)`, `once_new()`, `once_is_done(o)`
- `atomic_new(v)`, `atomic_get(a)`, `atomic_set(a, v)`, `atomic_add(a, d)`, `atomic_sub(a, d)`, `atomic_swap(a, v)`, `atomic_cas(a, exp, new)`
- `allocate(sz)`, `deallocate(ptr)`, `memory_alloc(sz)`, `memory_free(p)`, `memory_get(p, off)`, `memory_set(p, off, v)`, `memory_borrow(p)`, `memory_release(p)`, `memory_stats()`
- `push_lifetime()`, `pop_lifetime()`, `borrow_immutable(v)`, `borrow_mutable(v)`, `borrow_unique(v)`, `release_borrow(b)`, `gc_collect()`, `gc_stats()`

### 2.9 AI, Tensors, Quantum, Polyglot/FFI & Native Graphics
- **AI & Tensors:** `ai_chat`, `ai_embed`, `ai_token_count`, `onnx_infer`, `tensor_backend`, `tensor_add`, `tensor_sub`, `tensor_mul`, `tensor_scale`, `tensor_dot`, `tensor_matmul`, `tensor_transpose2d`, `tensor_conv2d`, `tensor_maxpool2d`, `tensor_avgpool2d`, `tensor_batch_norm`, `tensor_layer_norm`, `tensor_embedding_lookup`, `tensor_relu`, `tensor_leaky_relu`, `tensor_gelu`, `tensor_sigmoid`, `tensor_tanh`, `tensor_softmax`, `tensor_log_softmax`, `tensor_cross_entropy`, `tensor_sum`, `tensor_mean`, `tensor_argmax`
- **Quantum:** `quantum_backends()`, `quantum_simulate(circ)`, `quantum_statevector(circ)`, `quantum_run(circ, shots)`
- **Polyglot & FFI:** `ffi_load(lib)`, `ffi_call(h, fn, args)`, `ffi_call_typed(h, fn, sig, args)`, `ffi_close(h)`, `polyglot_detect(src)`, `polyglot_exec(lang, src)`, `polyglot_exec_with_input(lang, src, inp)`, `polyglot_compile_cached(lang, src)`, `polyglot_exec_compiled(art)`, `polyglot_cache_info()`, `polyglot_cache_clear()`
- **Native Graphics (`minifb`/SDL2/WGPU):** `gfxcreatewindow`, `gfxclosewindow`, `gfxwindowopen`, `gfxpollevents`, `gfxclear`, `gfxpresent`, `gfxdrawpoint`, `gfxdrawline`, `gfxdrawrect`, `gfxdrawrects`, `gfxdrawtextaa`, `gfxsize`, `gfxisopen`, `gfxclose`, `gfxtextprofile`, `gfxbackendprofile`

---

## 3. Corrected & Complete Catalog of All 112 Programs/Projects in `AAP`

### 3.1 `AAP/products/` — **64 Product Directories** (`59` Standalone Pure-PBLang Packages with `pblang.toml` + `5` Additional Modules)

1. **Ασφάλεια, Αδειοδότηση, Φυσική/Ψηφιακή Φύλαξη, Κρυπτογραφία & Zero-Knowledge Cloud (4):**
   - [`pbduallock`](file://../pbduallock/pblang.toml) — Dual-Key Sovereign Vault, Partner-Safeguard, Canary Tripwires & 720h Autonomous Continuity Control Plane
   - [`pblicense`](file://../pblicense/pblang.toml) — Multi-Layer ERP, Hardware, Heartbeat, Manifest & SaaS Licensing Engine
   - [`pbspotit`](file://../pbspotit/pblang.toml) (+ `spotit`) — Release-Signing, Authenticated-State & Physical Motion/Tamper Alarm Security Guard (`Controller.arm`, pattern guard, bounded alert policy)
   - [`pbcloud`](file://../pbcloud/pblang.toml) — **ΝΕΟ:** Zero-Knowledge Google Cloud Server Control Plane (`AMD_SEV_SNP`, `ChaCha20` Envelope, SSRF `169.254.169.254` blocker) connected to `pbduallock` & `pbspotit`
2. **Πυρήνας Γλώσσας, IDE, OS, AI & MCP (7):**
   - [`pblang`](file://../pblang/pblang.toml), [`pblang-synced`](file://../pblang-synced/pblang.toml), [`pbide`](file://../pbide/pblang.toml), [`pbos`](file://../pbos/pblang.toml), [`pbai`](file://../pbai/pblang.toml), [`pb-mcp-server`](file://../pb-mcp-server/pblang.toml), [`aipy`](file://../aipy/pblang.toml)
3. **Ναυαρχίδες, Διαχείριση Αρχείων & Καθαρισμός Περιεχομένου (6):**
   - [`pbcosmodata`](file://../pbcosmodata/pblang.toml), [`pbprofessions`](file://../pbprofessions/pblang.toml), [`pbexplorer`](file://../pbexplorer/pblang.toml), [`pbwatermarks-remover`](file://../pbwatermarks-remover/pblang.toml), [`pbremote`](file://../pbremote/pblang.toml), [`crud-website-pblang`](file://../crud-website-pblang/pblang.toml)
4. **Δεδομένα, Πίνακες, Λογιστική Συμφωνία & Στατιστική (12):**
   - `pbdatalab`, `pbgroupstats`, `pbreshape`, `pbportions`, `pbreconcile`, `pbtablecheck`, `pbcompare`, `pbjoin`, `pbmerge`, `pbtally`, `pbinterpolate`, `pbsmooth`
5. **Επιστημονικά, Μαθηματικά, Κυκλώματα, Υλικό & Μετρήσεις (5):**
   - `pbcalc`, `pbmath` (PBMath Studio `app/pblang.toml`), `pbcircuits`, `pbmeasure`, `pbloadlab`
6. **Έγγραφα, Γραφείο, Παρουσιάσεις & Κείμενο (11):**
   - `pboffice`, `pbbrief`, `pbdeckmix`, `pbexcerpt`, `pboffer`, `pbquote`, `pbreview`, `pbsections`, `pbstencil`, `pbwords`, `pbideas`
7. **Πολυμέσα, Υπότιτλοι, Εικόνα & Σχεδίαση (5):**
   - `pbmedia`, `pbcaptionclip` (SRT/WebVTT subtitle clipping), `pbcorners`, `pbpalette`, `pbtile`
8. **Εκπαίδευση & Μελέτη (4):**
   - `pbquiz`, `pbstudy`, `pbstudyagenda`, `pblesson`
9. **Προγραμματισμός Έργων, Χρόνου & Εργασιών (6):**
   - `pbplan`, `pbtaskscope`, `pbtimeline`, `pbjobtracker`, `pbslotfinder`, `pbworkdays`
10. **Λοιπά Modules/Placeholders (4):**
    - `pbcomms`, `pbgitcomms`, `pbnexus`, `spotit`

### 3.2 `AAP/projects/` — **48 Additional Projects across 8 Domains**
- `ai` (8): `AI`, `Delphi_AI`, `Sp_AIModel`, `WEB`, `aipy`, `max`, `takisbot`, `tests`
- `desktop` (8): `c#`, `delphi`, `java`, `lazarus`, `python`, `rust`, `tests`, `visual-basic`
- `game-dev` (7): `c#`, `c++`, `java`, `pong`, `python`, `tests`, `unity`
- `lang-dev` (3): `biapryx`, `sapphire`, `pblang-backup`
- `os` (1): `AnankeOS`
- `robotics` (8): `ROS`, `arduino`, `c++`, `delphi`, `python`, `raspberry_pi`, `simulation`, `tests`
- `scripting` (4): `lua`, `python`, `tests`, `vbscript`
- `web` (9): `Laravel_site`, `asprj`, `blazor`, `clothes-site`, `django`, `jQuery_Site`, `job-tracker`, `laravel`, `netvis-app`
