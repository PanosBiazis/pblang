# Native compile / backend research (N1 / E5)

Terminology note: this document includes historical `biapryx` command names in
older sections. Current CLI/runtime naming is `pblang`.

Σημειώσεις για πιθανό native/hybrid compilation backend και για το E5
batch (executables, self-extracting/installer presets). Αυτό το αρχείο είναι
σχεδιαστικό — **καμία δέσμευση για άμεση υλοποίηση**· περιγράφει το staged
μονοπάτι από τα σημερινά `pack` / `bundle` μέχρι πραγματικά single-binary
artifacts.

## Current pure-pblang x86-64 backend status (2026-07-12)

The repository now contains a direct pblang-written Linux x86-64 backend in
`stdlib/codegen_x64.sapp` and the broader source compiler in
`stdlib/native/codegen_x64.sapp`. Current verified behavior includes:

- compact `mov r32,imm32`, `add r64,imm8`, and rel8 branch selection when the
  shorter encoding preserves semantics;
- direct argument-register comparisons for shrink-wrapped function guards,
  using `imm8`/`imm32` and short conditional branches without a temporary
  `rax` move;
- conservative control-flow proof that omits the implicit `return 0` epilogue
  only when the function body cannot fall through;
- RX-only ELF load segments for ordinary programs and immutable data, while the
  backward-compatible mutable-buffer builders retain an explicit writable route;
- a checked native-arena `mmap` result before it becomes the allocation pointer;
- configurable page-aligned arena reservation through
  `set_heap_reserve(bytes)` from 1 MiB through 1 GiB. The default remains 1 GiB
  for compatibility and is virtual/lazy; resident memory grows only for touched
  pages;
- bounds-trapping array access, guarded stack-frame sizing, and focused
  hardening/compactness tests.

Measured on this checkout, a constant register-IR executable changed from 139
to 135 bytes. Representative code blobs changed as follows:

| IR sample | Before | Current | Reduction |
|---|---:|---:|---:|
| `seti` | 7 B | 5 B | 28.6% |
| `seti; addi small` | 14 B | 9 B | 35.7% |
| small counted loop | 35 B | 23 B | 34.3% |
| small `ifnz` | 35 B | 22 B | 37.1% |

These are encoding-level measurements, not a claim that every pblang feature is
covered by this backend. The full native smoke suite is the release gate for its
currently supported surface.

### Reproducible direct-codegen matrix row (2026-07-18)

The pure-PBLang reproducible competitor harness now includes
`pblang-codegen-x64` as a distinct native path. Its permanent PBLang driver
imports `stdlib/native/codegen_x64.sapp`, calls `compile_safe`, rejects a failed
safety certificate or malformed ELF header, writes the artifact, and executes
the emitted Linux x86-64 ELF under the same timeout, output, checksum, RSS, and
sampling contract as the foreign compiled rows. It does not transit C99.

On the fixed one-million-iteration integer kernel, seven warm samples measured
5.361 ms median and 5.447 ms interpolated p95, versus 4.011/4.143 ms for C and
15.386/16.338 ms for PBLang hosted C99. Three warm end-to-end compile samples
measured 82.147 ms; this includes PBLang process startup, module imports, code
generation, validation/write, and `chmod`. The emitted ELF was 487 bytes and
the short-process median peak RSS was 264 KiB. The matrix therefore measured
this direct path 1.337x slower than C but 2.870x faster than hosted C99, with an
artifact 96.6% smaller than C's fixture artifact.

These numbers cover only the backend's accepted source subset on this Linux
x86-64 host. They do not establish feature parity, cross-platform performance,
or whole-application superiority. Short-process RSS/CPU granularity, host load,
caches, and CPU frequency remain uncontrolled. Full method and raw samples are
in `bench/results/2026-07-18-reproducible-core-matrix/`.

### Compact x86-64 stack frames (2026-07-13)

Both pure-pblang x86-64 backends now select the shortest semantics-preserving
stack adjustment: no instruction for a zero-byte allocation, signed `imm8` for
1-127 bytes, and `imm32` above that range. The source backend applies the same
rule to framed functions and outgoing areas for calls with more than six SysV
arguments. Its existing scratch pass now determines the final frame before
real emission and rejects any sizing-pass drift instead of backpatching a
fixed-width placeholder.

The shared-IR backend also emits `leave; ret` and omits its compatibility
epilogue when a direct final `ret` already proves it unreachable. On the pinned
32-function benchmark, the source ELF changed from 8,124 to 8,022 bytes
(-1.26%) and the shared-IR ELF from 1,943 to 1,584 bytes (-18.48%). Five-run
median shared-IR codegen changed from 2.261 to 2.032 ms (-10.1%); source-codegen
median changed from 69.462 to 70.629 ms (+1.7%, within observed host noise and
not claimed as a speed improvement). Both generated executables were run and
returned their expected results. Reproduction data and limits are in
`bench/results/2026-07-13-x64-frame-encoding/`.

### Exact additive linear recurrence lowering (2026-07-14)

The pure-pblang source backend recognizes the exact one-parameter form
`if n < 2 { return n; }` followed by
`return self(n - 1) + self(n - 2)`. The terms may be swapped. It lowers this
proven recurrence directly to a constant-space SysV AMD64 loop using only
`rax`, `rcx`, and `rdi`, with no frame, heap allocation, spills, or recursive
calls.

The recognizer fails closed: captures, extra parameters/statements, changed
guards or base values, transformed arguments, other operators, and
checked-overflow mode all retain the general recursive path. Checked mode is
excluded because iteration would change recursive overflow-trap ordering.
Focused tests cover the accepted forms, a near-match rejection, and the checked
mode boundary.

Seven alternating CPU-pinned `fib(36)` processes reduced median elapsed time
from 49.738 to 1.466 ms (33.93x, -97.05%), reduced the ELF from 442 to 405
bytes (-8.37%), and retained 264 KiB peak RSS. Thirty compile samples reduced
median source-codegen time from 1.820 to 1.230 ms (-32.39%). A generic AST
rewrite was rejected because it increased the artifact to 452 bytes. Exact
samples, methodology, and limits are in
`bench/results/2026-07-14-x64-linear-recurrence/`.

### Shared IR dead-arithmetic elimination (2026-07-13)

The pblang-written optimizer in `stdlib/codegen_ir.sapp` now uses its structured
backward liveness result to remove dead pure `add`, `sub`, `mul`, and `addi`
writes. It remains conservative around stack/local effects, syscalls, branch
conditions, loop counters, and loop-carried dependencies. One change therefore
reduces x86-64, i386, ARM64, and portable C99 output together.

On the pinned correctness-gated fixture, the optimized loop body changed from
five instructions to one. x86-64/i386/ARM64 artifacts decreased from
170/126/168 bytes to 153/111/152 bytes, while C99 source decreased from 220 to
167 bytes. Seven-run codegen medians improved by 19.1-19.5%. Method and exact
metrics are under `bench/results/2026-07-13-shared-ir-dead-arithmetic/`.

On the recursive `fib(32)` fixture, the function-level changes reduced the ELF
from 441 B to 425 B and changed best-of-five runtime from 11 ms to 10 ms on the
documented 2026-07-12 host. The same run still has optimized Fortran at 2 ms,
GCC at 5 ms, and Rust at 6 ms; this is progress, not a universal-win claim. See
`bench/results/2026-07-12-x86_64/REPORT.md` for method and limits.

### Pure-pblang i386 backend hardening and compactness

`stdlib/codegen_x86.sapp` now chooses `add r32,imm8` and short conditional/
unconditional branches whenever the signed displacement fits, with transparent
near-branch fallback and relocation tests for nested calls. Representative IR
sizes changed from 6 B to 3 B (`addi`), 15 B to 6 B (small `ifnz` with no else),
and 17 B to 9 B (small loop).

All public ELF32 builders now emit an `R|X` load segment. `build_value` stores
`eax` on the writable process stack before `write(2)`, rather than appending a
writable result buffer to executable memory. This removes the previous `RWX`
mapping without changing its four-byte stdout contract. Short/near boundaries,
recursive calls, nested-call relocations, syscalls, and value output are covered
by executable i386 tests.

The ordinary AArch64 ELF builder now follows the same W^X rule: its code-only
`PT_LOAD` segment is `R|X` instead of `RWX`. The exact program-header flag is
covered by `codegen_arm64_test.sapp`; writable state remains on stack/local
storage rather than in the executable segment.

### Shared loop-invariant and identity optimization (2026-07-16)

The common pblang IR optimizer now carries only read-only register constants
into loop bodies, removes zero-add/zero-sub/one-multiply work, and collapses
identical dynamic branches. The optimizer report exposes per-pass deltas, and
`execution_pipeline.optimization_matrix` verifies bounded raw/optimized result
equivalence before measuring all common classical backends without executing
native files.

On the pinned identity/branch fixture, IR changed from 12 to 5 instructions;
x86-64 ELF from 188 to 157 bytes, i386 from 140 to 114, ARM64 from 188 to 156,
and C99 source from 283 to 179 characters. The 1,000-iteration optimizer phase
measured 121.56 ms in this run (0.122 ms/operation); timing is host/run specific.

---

## Prerequisites (σήμερα)

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

Τα παρακάτω στάδια θεωρούν αυτές τις προϋποθέσεις δεδομένες και στοχεύουν σε
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
  - Embedded `biapryx` binary μέσα στο self-extracting launcher, ή
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

#### Key Prerequisites

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
   requires static module resolution or a runtime bridge for dynamic imports.

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

## Shared IR constant-copy propagation (2026-07-15)

`stdlib/codegen_ir.sapp` now rewrites `movr dst,src` to `seti dst,value` when
forward analysis proves `src` constant. This exposes the original source write
and superseded destination writes to the existing structured DSE pass. The
optimization is shared by every register-IR backend.

The same pass exposed an instruction-contract gap: x86-64 already lowered
`movr`, while i386, ARM64, and portable C99 did not. Those backends now encode
or emit register copies directly, with exact i386/ARM64 instruction tests and
cross-backend artifact gates. On the permanent fixture, IR changed from 7 to 3
instructions; x86-64/i386/ARM64 artifacts changed from `167/121/168` to
`150/110/148` bytes and C99 source from `210` to `154` bytes. These values
measure this bounded fixture only.

## Portable register-IR evaluator stack (2026-07-13)

The reference evaluator in `stdlib/codegen_ir.sapp` is itself written in
pblang. Its `popr` implementation now uses an in-place `array_pop` after an
explicit underflow check. The previous implementation allocated and copied the
remaining array for every pop, turning a complete stack drain into O(n^2) work.

The permanent phase metric `portable_ir_stack_push_pop` correctness-gates a
2,000-value push/drain workload. Seven pinned release-process medians improved
from 1643.439 ms to 10.910 ms (150.63x), with median process peak RSS moving
from 11,568 to 10,780 KiB. This improves the portable reference/emulation path;
it does not change the runtime speed of ELF code emitted by native backends.

## Self-hosted hosted C99 path (2026-07-14)

`stdlib/compiler/ctranspile.sapp` now has a userspace profile in addition to its
function-only, freestanding, and kernel profiles:

- `transpile_program(source)` emits libc preamble, forward declarations,
  functions, top-level statements, numeric/string `print`, and C `main`.
- A pblang `fn main()` is consistently renamed to `pb_main` and called by the C
  entry, avoiding a symbol collision.
- `build_program(source, output, options)` discovers `cc`/GCC/Clang, invokes it
  with argv rather than a shell, and supports `speed`, `size`, and `portable`
  profiles with a portable fallback.
- `run_program(source, options)` uses a bounded watchdog, unique temporary
  artifacts, and cleanup on success, timeout, or compiler failure.
- `transpile_program_int` / `numeric: "integer"` is an explicit optimization
  profile. It is not the default because division and non-integral values differ
  from normal f64 pblang semantics.
- Hosted top-level `let`/`const` now lower to static translation-unit storage,
  with exactly-once source-order assignments in generated `main`. Flattened
  modules are dependency-first, so imported module state is initialized before
  importer expressions call it. Module functions can read/write globals, while
  global closures and named function values keep checked handle/arity calls.
- `link_program` and `transpile_program_linked` accept bounded in-memory virtual
  project snapshots. Runtime symbol and lowered C-identifier collisions reject
  the translation rather than producing invalid or ambiguous C.

The accepted local `fib(32)` best-of-5 sample was 10 ms and 15,992 bytes for
both self-hosted C99 profiles, versus 10 ms for GCC `-O2`, 4 ms for Fortran,
and 2 ms / 405 bytes for pblang's direct x86-64 emitter. This is one fixed-input
recursive benchmark including process startup, not a universal ranking.

## Pure-pblang interpreter compact closures (2026-07-14)

`stdlib/compiler/interp.sapp` now stores its private tree-interpreter closures
as fixed arrays `[tag, params, arity, body, env]` instead of five-field maps.
The representation is internal to the evaluator; interpreted programs retain
the same call, recursion, closure-capture, shadowing, and arity semantics.
Small-arity apply paths load metadata through stable numeric slots and continue
to allocate an isolated lexical environment for every call.

Prepared guard-only `if` nodes also cache that they have no `elif`/`else`
fallback, avoiding repeated presence probes on the common recursive false path.
Block evaluation hoists statement-count calculation out of its dispatch loop.

Five fresh CPU-pinned same-process A/B runs measured fixed-array closure
metadata access 4.23-4.70% faster than the old map layout. Full phase runs
measured 5.69-6.16% in the same direction. End-to-end recursive timings remain
frequency-sensitive and are not claimed to improve by that exact percentage.
The rejected scalar return-signal experiment is retained in the evidence: it
passed correctness but slowed the long recursive batch, so it was reverted.

## Pure-pblang interpreter static builtin resolution (2026-07-17)

`stdlib/compiler/interp.sapp` now collects supported binding names before AST
preparation. An identifier call to one of the interpreter's 16 builtins receives
an unshadowed marker only when no variable, named function, parameter,
function-literal parameter, or loop binding can use that builtin name. The
runtime then skips `_env_has` for that proven case. Any matching binding keeps
the previous dynamic scope-chain lookup, including bindings declared after the
call, so lexical and call-before-declaration behavior is unchanged.

The binding map stores only builtin names and therefore remains bounded by the
current builtin registry. Prepared metadata uses version 2; an older or raw AST
is rescanned and rebuilt before evaluation. Member/index callees and unknown
identifiers do not enter this fast path.

Seven fresh CPU-pinned processes alternated a dynamic-control AST and the direct
AST within each sample. The medians of their per-process p50 changes were
-7.87% (`len`), -6.95% (`map_has`), -7.42% (`str_starts_with`), and -11.52%
(mixed calls). The compile-only median increment on the small `fib18` source was
0.0308 ms. A numeric builtin-ID experiment was reverted because its 0-2.4%
movement was inconsistent and did not justify added complexity. The report does
not use drifting absolute process times as evidence; raw samples and limitations
are in `bench/results/2026-07-17-tree-interpreter-builtin-resolution/REPORT.md`.

## Range-proven x86-64 constant division (2026-07-18)

The PBLang-written source backend now tracks conservative non-negative ranges
through straight-line assignments and bounded loops. Facts are capped below
`2^32`; unknown values and branch/loop writes that cannot be merged safely are
invalidated. Range data affects only optimization selection and never language
semantics.

For a constant divisor `d`, a proven `0 <= n < 2^32` uses
`m = floor(2^32 / d)` and computes an initial quotient from
`(n * m) >> 32`. The approximation can be low by at most one, so a remainder
comparison performs one exact correction. The compile-time reciprocal is
verified with integer products no larger than `2^33`, avoiding floating-point
rounding drift. A dividend proven below `d` needs no reduction; one proven below
`2*d` uses a compare and at most one subtraction. Power-of-two divisors retain
their existing shift/mask lowering.

The optimization fails closed to the prior unsigned 64-bit `div` sequence for
unknown parameters, negative/overflowing ranges, non-integral constants, zero,
or constants outside the supported immediate range. The executable semantic
test covers reciprocal correction boundaries, the single-subtract path, the
one-million-iteration checksum, emitted opcodes, and the unknown-range fallback.
Reproducible multi-toolchain measurements live under
`bench/results/2026-07-18-x64-constant-div/`.

## Verified artifact cache and measured cold boundary (2026-07-18)

`stdlib/native/artifact_cache.sapp` is a PBLang-written incremental cache for
the direct Linux x86-64 backend. It hashes source, profile, and backend input
files, bounds every read and artifact, validates metadata plus SHA-256 content,
and verifies the ELF machine, class, entry, program header, and file size before
staged publication. Corruption and dependency changes miss safely. Same-user
malicious cache replacement remains outside this cache's trust boundary.

The direct benchmark separates process startup, dynamic backend import,
in-process emission, uncached publication, cache-cold publication, and verified
cache-warm publication. The initial same-turn uncached median improved from
120.057 to 78.954 ms; an independent saved run measured 97.660 ms uncached and
23.998 ms warm. Backend import remained the major cold cost. This boundary
motivated the verified module cache and batch session described below; artifact
reuse is still not mislabeled as cold compilation.

## Precompiled direct-driver cache (2026-07-29)

`tools/pbcodegen_cache.sapp` now links and freezes the PBLang-written x86-64
driver as a content-addressed PBCK. Its v2 identity binds the exact compiler
executable SHA-256, compiler version, linked source and complete dependency
graph. Reuse verifies the graph, PBCK digest and format; initial publication
also requires the cached driver to generate a structurally valid ELF.

The source linker now uses UTF-8 byte indexes for ASCII syntax scanning instead
of allocating a one-codepoint string at every input position. A compiler-sized
link that previously exceeded a 120-second bound completed in about one second.
The complete phase suite measured 43.804 ms median source-import publication
versus 7.169 ms through the precompiled driver (6.11x), with peak RSS reduced
from 19,472 to 12,720 KiB. The 716,463-byte cached driver is a storage tradeoff,
and `emit-pbck` remains an explicit host bootstrap boundary. Full evidence is in
`bench/results/2026-07-29-cold-codegen-driver/REPORT.md`.

## One-process x86-64 builds and verified module bytecode (2026-07-19)

`stdlib/native/compile_session.sapp` now imports the direct backend once,
fingerprints codegen/tokenizer/parser/ELF inputs once, and compiles up to 256
bounded jobs. All jobs are safety-certified and ELF-validated before any output
publication. The content-addressed artifact cache is format-versioned and
verified; cache I/O failure is observable but does not alter compiler semantics.

On six distinct source files, nine samples measured 445.675 ms median for six
independent processes, 102.972 ms for one process with cold artifacts (4.33x),
and 81.758 ms for one process with verified warm artifacts (5.45x). The current
single-file phase run measured 2.983 ms startup, 70.479 ms backend import,
2.911 ms in-process emission, and 74.673 ms full uncached publication. The
session therefore amortizes the measured import cost; it does not remove that
cost for a single fresh process.

`stdlib/compiler/module_cache.sapp` is an opt-in persistent cache for imported
self-hosted chunks. Its key includes processed source, resolved path, PBCS
format, and the current frontend/codegen/VM dependency revision. Hits verify
metadata, digest, size, and the complete PBCS structure. On a 300-function
module, warm load measured 84.549 ms versus 128.271 ms uncached (1.52x), while a
cold verified store measured 271.464 ms (2.12x the uncached median). It is useful
for repeated builds, not one-shot compilation. Evidence is under
`bench/results/2026-07-19-compiler-caches/`.

Hosted C99 modulo lowering now keeps safe integer literals visible to the C
optimizer and routes dynamic hosted modulo through a helper that rejects zero
and `LONG_MIN % -1`. Default f64 semantics remain unchanged; integer mode is an
explicit profile. The PBLang-written tree interpreter also prepares compact
plans for safe binary leaves and uses them in plain assignments/while guards,
with a generic fallback for all complex cases. Full evidence is in
`bench/results/2026-07-18-compile-c99-interpreter/`.
