# Native compile / backend research (N1 / E5)

Σημειώσεις για πιθανό native/hybrid compilation backend και για το E5
batch (executables, self-extracting/installer presets). Αυτό το αρχείο είναι
σχεδιαστικό — **καμία δέσμευση για άμεση υλοποίηση**· περιγράφει το staged
μονοπάτι από τα σημερινά `pack` / `bundle` μέχρι πραγματικά single-binary
artifacts.

---

## Blockers (σήμερα)

- Δεν υπάρχει ακόμη σταθερό **bytecode ABI** για "frozen" εκδόσεις· αλλαγές
  στο VM/IR μπορούν να σπάνε παλιά πακέτα.
- Τα tools **`sapphire pack`** και **`sapphire bundle`** (E4) βασίζονται
  σε ήδη εγκατεστημένο `sapphire` binary στο target μηχάνημα.
- Δεν υπάρχει cross-platform abstraction για:
  - εύρεση/εκτέλεση προσωρινού extract directory,
  - ασφαλή χειρισμό self-extracting executables (π.χ. temp dirs, cleanup,
    signals).
- Καμία ιστορία ακόμη για **auto-update** ή multi-arch bundles (linux/macOS/
  windows από ένα artifact).

Τα παρακάτω στάδια θεωρούν αυτούς τους blockers δεδομένους και στοχεύουν σε
ρεαλιστικά, μικρά βήματα.

---

## Backend criteria

Ένας πρακτικός native/hybrid backend (ή self-extracting wrapper) για Biapryx
πρέπει να:

- **Μην απαιτεί LLVM/Cranelift στο πρώτο στάδιο** — να μπορεί να βασιστεί
  στη σημερινή Rust υλοποίηση του VM και στα E4 tools.
- **Είναι πλήρως cross-platform** (τουλάχιστον linux + windows, ιδανικά και
  macOS) με σαφή ιστορία για target triples.
- **Διατηρεί sandboxing/capabilities**: ό,τι δικαιώματα έχει σήμερα ένα app
  μέσω allow_fs/allow_exec/allow_net πρέπει να ισχύουν και στο wrapped binary.
- **Είναι debuggable**: λογικό logging, flag για "extract-only", δυνατότητα
  να τρέχει κανείς το πακετάρισμα/εκτέλεση χειροκίνητα.
- **Δεν κλειδώνει τον μελλοντικό N2/N3 backend** (π.χ. LLVM) — το self-
  extracting wrapper πρέπει να αντιμετωπίζεται ως Stage A, όχι τελική λύση.

---

## Prerequisite checklist

Πριν επιχειρηθεί υλοποίηση Stage A πρέπει να υπάρχουν:

- Σταθεροποιημένα semantics για:
  - `biapryx pack` / `biapryx bundle` (E4) — I/O σχήμα, error messages,
    metadata validation.
  - Manifest metadata (`target_platform`, `min_runtime`) και σαφής πολιτική
    για το πώς χρησιμοποιούνται σε bundled / self-extracting apps.
- Βασικό **spec** για το τι σημαίνει "runnable package" (έχει entry, έχει
  lockfile, δεν διαφεύγουν path deps εκτός package root).
- Απόφαση για default **runtime distribution model**:
  - Embedded `biapryx` binary μέσα στο self-extracting stub, ή
  - Λογική "bootstrap installer" που εγκαθιστά/εντοπίζει shared runtime.
- Απλό, portable helper για:
  - δημιουργία temp καταλόγων,
  - ασφαλές unpack `.tar.gz` (υπάρχει ήδη στον package manager),
  - cleanup στο exit (κανονικό ή crash).

---

## Staged path

Το παρακάτω σενάριο είναι το προτεινόμενο staged path, συγχρονισμένο με το
backlog (E5, N2).

### Stage A — Self-extracting wrapper (E5 focus)

**Στόχος:** Να μπορεί ένας developer να παράγει *ένα* binary που:

1. Περιέχει:
   - runtime (`biapryx` VM build),
   - το αποτέλεσμα του `biapryx pack` (tarball με app),
   - μικρό bootstrap κώδικα.
2. Όταν εκτελείται:
   - κάνει self-extract του tarball σε temp dir,
   - τρέχει `biapryx run .` μέσα σε αυτό το temp dir,
   - προωθεί CLI args προς το app,
   - καθαρίζει (βασικά) τα temp αρχεία στον τερματισμό.

**Σχεδιαστικές επιλογές:**

- Wrapper binary = Rust πρόγραμμα που:
  - περιέχει το tarball ως **`include_bytes!`** asset,
  - εμφανίζει `--help`/`--version` βασισμένα σε metadata του app,
  - υποστηρίζει flag `--extract-only <dir>` για debug (χωρίς run).
- Δεν απαιτείται ακόμη "installer" — η συμπεριφορά είναι
  **"self-extract and run once"**.
- Targeting:
  - linux: ELF binary με embedded data section,
  - windows: PE binary με embedded resource ή απλό include_bytes,
  - macOS: Mach-O με include_bytes (ίδια λογική).

**Συμβατότητα με E4:**

- Ο wrapper δεν επανα-εφευρίσκει packing· καλεί εσωτερικά το ήδη υπάρχον
  pack logic (lib-level API) ή περιμένει ότι ο developer έχει δημιουργήσει
  tarball μέσω `biapryx pack`.
- Το format του tarball μένει ακριβώς όπως σήμερα· το μόνο που αλλάζει είναι
  ο τρόπος διανομής.

**Ελάχιστο API surface για Stage A:**

- CLI (νέο subcommand, μελλοντικό E5-implementation detail):
  - `biapryx make-self-extracting [path] [--out <binary>]`  
    - `path`: manifest dir,
    - παράγει:
      - `pack` tarball,
      - wrapper binary με embedded tarball,
      - run hint: `./myapp` (ή `myapp.exe`).
- Δεν αλλάζει το `pack` / `bundle` interface, μόνο τα χρησιμοποιεί.

### Stage B — Installer presets (deb/zip/AppImage/NSIS)

Αυτό το στάδιο είναι ως επί το πλείστον **docs + tooling glue**, όχι πλήρης
package manager για OS-level installs.

- Στόχος: "**installer presets**" για δημοφιλείς μορφές:
  - deb (Debian/Ubuntu),
  - zip + `.bat` / `.sh`,
  - AppImage (Linux),
  - NSIS-based installer (Windows).
- Προτεινόμενη προσέγγιση:
  - Στο docs/… προστίθεται πίνακας με:
    - ποια αρχεία περιμένουν αυτά τα formats (bin, desktop file, icons),
    - πώς ενσωματώνεις self-extracting binary ή bundle output,
    - links σε upstream docs (dpkg-deb, appimagetool, NSIS υλικό).
- Προαιρετικά helper scripts (όχι άμεσα απαραίτητο για E5-alpha):
  - `biapryx util make-zip-installer`,
  - `biapryx util make-appimage`.

### Stage C — Hybrid / native backend (N2+)

Αυτό το στάδιο μεταφέρεται κυρίως στο N2 backlog, αλλά το αναφέρουμε εδώ για
συνέχεια:

- Stage C1: self-extracting wrapper βελτιστοποιημένος (μικρότερο runtime,
  shared lib, κ.λπ.).
- Stage C2: transpile-to-C ή direct LLVM backend που παράγει native binaries
  χωρίς embedded VM (ή με minimal VM).
- Stage C3: no-std / freestanding mode, OS-level targets.

Η υλοποίηση αυτών είναι εκτός πεδίου του E5, αλλά το Stage A design πρέπει να
είναι **συμβατό** με την ιδέα ότι αργότερα μπορεί να αντικατασταθεί το
embedded `biapryx` με native-compiled runtime.

---

## Σημείωση για E5 acceptance criteria

Για το E5 batch, αυτή η σχεδιαστική σημείωση (μαζί με την E5-beta υλοποίηση) καλύπτει:

- **Self-extracting wrapper design** (Stage A) πάνω στα υπάρχοντα pack/bundle.
- **Installer presets** ως δεύτερο βήμα (docs-first, χωρίς να σπάει E4).
- Σαφή διάκριση ανάμεσα σε:
  - σημερινά εργαλεία (E3/E4),
  - στόχο E5 (single-binary διανομή),
  - μελλοντικό N2 (πλήρης native backend).

Οποιαδήποτε μελλοντική υλοποίηση πρέπει να κρατήσει αυτά τα κριτήρια και να
τεκμηριώνεται με tests/παραδείγματα στο executables pipeline.

### Stage A — E5-beta implementation status

- Προστέθηκε νέο crate `crates/biapryx-stub` με απλό launcher (`biapryx-stub`) που προορίζεται ως template για self-extracting binaries (περιέχει sentinel `PBLANG_BUNDLE_START\x00` και ελέγχει για embedded bundle).
- Το `biapryx bundle` δέχεται πλέον προαιρετικό flag **`--self-extract`** (E5-beta): το flag αναγνωρίζεται και κρατιέται για workflows self-extracting runner + bundle, χωρίς να αλλάζει το υπάρχον σχήμα εξόδου του `bundle`.
- Το πραγματικό embed του stub template + bundle παραμένει σχεδιαστικό βήμα: απαιτεί χειροκίνητο build του `sapphire-stub` και αντιγραφή του παραγόμενου binary πάνω από το `stub_template.bin` (ή μελλοντικό εργαλείο που θα το αυτοματοποιεί).

---

## Stage A — E5-gamma implementation status (COMPLETED)

**Status:** `implemented (prototype)` — April 2025

### Implementation Details

The self-extracting binary feature is now fully implemented:

1. **Stub Implementation** (`crates/biapryx-stub/src/main.rs`):
   - Scans own binary for SENTINEL marker (`PBLANG_BUNDLE_START\x00`)
   - Extracts the appended gzip tarball to `$TMPDIR/biapryx-run-<hash>/`
   - Executes `biapryx run .` inside that directory
   - Cleanup temp dir on exit (registered cleanup handler)
   - CLI flags: `--help`, `--version`, `--extract-only <dir>`

2. **Bundle Command** (`src/main.rs`):
   - `biapryx bundle --self-extract --output <file> <path>` creates a self-extracting binary
   - After creating bundle tarball, appends SENTINEL + tarball after stub binary bytes
   - Sets output file executable bit on Unix
   - Requires `biapryx-stub` binary to be built alongside `biapryx`

3. **Tests** (`tests/pack.rs`):
   - `self_extract_binary_contains_sentinel` — verifies output file contains SENTINEL sequence
   - `self_extract_runs_hello` — verifies minimal hello app can be bundled and contains correct structure

### Usage Example

```bash
# Build the stub first
cargo build --release --bin biapryx-stub

# Create a self-extracting binary
biapryx bundle --self-extract --output myapp ./myproject

# Run the self-extracting binary
./myapp

# Or extract without running
./myapp --extract-only ./extracted
```

### Binary Structure

```
[biapryx-stub binary bytes]
[SENTINEL: "PBLANG_BUNDLE_START\x00"]
[gzip-compressed tarball containing the app]
```

### Next Steps (Stage B)

- Installer presets for deb/zip/AppImage/NSIS
- Multi-arch bundles
- Auto-update mechanism

## Stage C — IR-to-Native mapping (N2-alpha research spike)

This section documents the path from the current bytecode VM to native code emission.
**No code changes required** — this is a research spike for future planning.

### IR → LLVM IR mapping for the 10 most common VM opcodes

| VM Opcode | LLVM IR equivalent | Notes |
|-----------|-------------------|-------|
| `Push(const)` | `store <const>, %slot` | Load constant from pool |
| `Pop` | — | Discard top; no IR needed |
| `Add/Sub/Mul/Div` | `fadd/fsub/fmul/fdiv double` | pblang numbers are f64 |
| `Jump(target)` | `br label %BBn` | Unconditional branch |
| `JumpIfFalse(target)` | `br i1 <cond>, label %true, label %false` | Needs truthiness check |
| `Call(arity)` | `call <ret> @fn(args...)` | Dynamic dispatch for closures |
| `Return` | `ret <value>` | May need GC value wrapping |
| `GetGlobal(name)` | `load %module_globals[name]` | Via pointer to module |
| `SetGlobal(name)` | `store <value>, %module_globals[name]` | |
| `Index` | `call @rt_index(<obj>, <key>)` | Runtime helper needed |

### Key blockers

1. **Closures/upvalues** — Need heap-allocated closure structs `{fn_ptr, env_ptr}`. Effort: HIGH.
2. **Dynamic dispatch** — `Call` on runtime value needs vtable/indirect call. Effort: HIGH.
3. **GC integration** — LLVM must annotate GC roots (`gc "statepoint"` or shadow stack). Effort: MEDIUM.
4. **Value type** — All values are `Value` enum; native code needs NaN-boxing or tagged pointers. Recommended: NaN-boxing (used by LuaJIT, JavaScriptCore).

### Cranelift vs LLVM

| Criterion | Cranelift | LLVM |
|-----------|-----------|------|
| Compile speed | Fast (incremental) | Slow (full pipeline) |
| Output quality | Good (JIT) | Best (AOT) |
| Rust integration | Native (`cranelift` crate) | Via `inkwell`/`llvm-sys` |
| GC support | Manual root tracking | `gc "statepoint"` built-in |
| Recommendation | JIT / fast startup | Release AOT builds |

**Recommendation**: Start with Cranelift for JIT, add LLVM backend for release builds.

### Estimated effort

| Stage | Effort |
|-------|--------|
| C1: Arithmetic + branches | 2–4 weeks |
| C2: Functions + closures | 4–8 weeks |
| C3: Full value type + GC | 8–16 weeks |
| C4: Stdlib interop | 4–8 weeks |
| C5: Production-ready | 8–16 weeks |

**Total**: 6–12 months of focused engineering for a production-quality native backend.
