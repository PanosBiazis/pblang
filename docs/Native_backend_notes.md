# Native compile / backend research (N1 / E5)

Terminology note: this document includes historical `biapryx` command names in
older sections. Current CLI/runtime naming is `pblang`.

Σημειώσεις για πιθανό native/hybrid compilation backend και για το E5
batch (executables, self-extracting/installer presets). Αυτό το αρχείο είναι
σχεδιαστικό — **καμία δέσμευση για άμεση υλοποίηση**· περιγράφει το staged
μονοπάτι από τα σημερινά `pack` / `bundle` μέχρι πραγματικά single-binary
artifacts.

---

## Blockers (σήμερα)

- Δεν υπάρχει ακόμη σταθερό **bytecode ABI** για "frozen" εκδόσεις· αλλαγές
  στο VM/IR μπορούν να σπάνε παλιά πακέτα.
- Τα tools **`pblang pack`** και **`pblang bundle`** (E4) βασίζονται
  σε ήδη εγκατεστημένο `pblang` binary στο target μηχάνημα.
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

Ένας πρακτικός native/hybrid backend (ή self-extracting wrapper) για pblang
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

### Stage C — Hybrid / Native Backend (N2 research spike)

**Goal**: Eventually eliminate the embedded `pblang` runtime from distributed apps
by compiling pblang bytecode directly to native machine code.

#### IR → LLVM IR Mapping (Top 10 VM Opcodes)

| VM OpCode | LLVM IR Equivalent | Notes |
|-----------|-------------------|-------|
| `OpConstant(idx)` | `alloca` + `store` / constant | Load value from constants pool |
| `OpAdd` / `OpSub` | `fadd` / `fsub` (f64) | pblang uses f64 for all numbers |
| `OpMul` / `OpDiv` | `fmul` / `fdiv` (f64) | |
| `OpCall(argc)` | `call` / indirect call | Dynamic dispatch through closure/function pointer |
| `OpGetGlobal` / `OpSetGlobal` | Global variable ptr | Module globals map to LLVM globals |
| `OpGetLocal` / `OpSetLocal` | Stack frame slot | Locals → LLVM `alloca` in function prologue |
| `OpJump` / `OpJumpIfFalse` | `br` / `cond_br` | Absolute IP → LLVM basic block labels |
| `OpReturn` | `ret` | Requires proper value boxing/unboxing |
| `OpClosure` | `malloc` + function pointer | Most complex: upvalue capture |
| `OpMakeArray` / `OpMakeMap` | `malloc` + struct init | GC-managed heap allocation |

#### Key Blockers

1. **Dynamic typing / Value boxing**: pblang's `Value` enum (Number/Bool/Nil/Obj/Array/Map/Closure)
   requires all values to carry type tags. In LLVM IR, this maps to tagged unions with
   pointer indirection — similar to how Lua/YARV implement values.

2. **Closures / upvalues**: `OpClosure` generates heap-allocated `Closure` structs with
   upvalue capture lists. Each upvalue is either an open stack reference or a closed heap
   value. LLVM equivalent requires environment structs (GEP + load/store chains).

3. **Dynamic dispatch**: Method calls (`obj:method()`) desugar into `try obj.__class catch obj`
   + attribute lookup. LLVM implementation needs inline caches or vtable-like dispatch.

4. **Garbage Collection**: RC + cycle-breaking GC. LLVM IR has no built-in GC; options:
   - Continue using RC (reference count in alloca'd value headers)
   - Shadow stack GC pass (GCRoot intrinsics)
   - Boehm conservative GC

5. **Module system**: `import()` is dynamic/runtime. Compile-time AOT compilation
   requires static module resolution or a runtime stub for dynamic imports.

#### Recommended Approach: Cranelift (Not LLVM)

**Cranelift** (used by Wasmtime, Rust's `cranelift-codegen`) is preferred over full LLVM for pblang's Stage C:

- **Simpler API**: Cranelift IR is closer to pblang's stack VM than LLVM's SSA form
- **Rust-native**: No C++ FFI binding required (pure Rust crate `cranelift-codegen`)
- **Faster compile times**: Cranelift is designed for JIT; Stage C can start as a JIT
- **Incremental path**: Start with JIT-style hot-path compilation, then AOT

**Estimated effort**: 3-6 months part-time for a working Cranelift JIT covering the
top 20 opcodes. Full AOT compilation with GC and closures: 6-12 months.

**Prerequisite**: Spec freeze + conformance suite (to validate the native backend
produces identical semantics to the bytecode interpreter).

#### Stage C Status

Currently: **Research spike only** (N2-alpha). No code changes required at this stage.

Next action: Once Stage A (self-extracting wrapper) is stable, begin Cranelift integration
by implementing a `pblang-cranelift` crate that compiles individual `Function` chunks to
native code for numeric-intensive hot paths, falling back to the bytecode VM for dynamic
features (closures, dynamic dispatch, import).

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

- Προστέθηκε νέο crate `crates/pblang-stub` με απλό launcher (`pblang-stub`) που χρησιμεύει ως template για self-extracting binaries (περιέχει sentinel `PBLANG_BUNDLE_START\x00` και ελέγχει για embedded bundle).
- Το `pblang bundle` δέχεται προαιρετικό flag **`--self-extract`** (E5-beta): πλέον παράγει ένα **self-extracting εκτελέσιμο** αντί για κατάλογο bundle — σταδιοποιεί το package (source, `pblang.toml`, `pblang.lock`) μαζί με το τρέχον `pblang` runtime κάτω από `bin/`, το συμπιέζει σε tarball και το συνενώνει πάνω στο `pblang-stub` πίσω από το sentinel `PBLANG_BUNDLE_START\x00`. Η έξοδος προεπιλέγεται στο `[package].name` (`.exe` σε Windows) ή στη διαδρομή του `--output`.
- Το embed του stub + bundle είναι πλέον υλοποιημένο μέσα στο `pblang bundle --self-extract` (διαβάζει το `pblang-stub` δίπλα στο τρέχον εκτελέσιμο και το συνενώνει με το tarball)· απαιτεί το `pblang-stub` να έχει χτιστεί/εγκατασταθεί δίπλα στο `pblang`.
