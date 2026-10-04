# Explicit checkpoint file binding

`neural_specialist_checkpoint_bind(provider)` is the supported file route from a
working directory different from the trusted application installation. Both actual
public facades expose it. Computation remains a separate `neural_specialist_bind`.

```pblang
let AI=import("stdlib/pbai.sapp");
let IO=import("stdlib:pbai_student_checkpoint_io.sapp");
let storage=AI.neural_specialist_checkpoint_bind(IO);
// Explicit effects only, with the existing grants and an existing private parent.
let receipt=storage.save_new(path,checkpoint);
let restored=storage.load_file(path);
```

Use trusted top-level imports with the application manifest and materialized lock.
The shared file owner now imports `secure_file.sapp` at its own top level, alongside
the neural codec. Its returned functions retain these selected modules. It does
not discover a provider from the caller directory, change cwd, retry with a
caller-selected dependency, or fall back to ordinary file I/O.

Binding accepts only a native module with callable `save_new` and `load_file`
exports. It returns exactly those two captured functions plus `report()`, without
calling the provider or probing filesystem capability. A missing export, noncallable
export, path, map, callback or other value is rejected without a partial binding.
Every report is detached and contains only fixed operation names and flags.
`storage_guarantees_verified:false` and `provider_authenticated:false` are deliberate:
structural binding does not prove provider identity, signature semantics, sandbox
module initialization, or validate actual storage guarantees. Importing a chosen
module executes its initialization before binding; choose trusted packaged code.
Do not expose arbitrary provider selection to remote requests or untrusted models.

Importing the facade alone loads only the small binding helper, not the optional
file owner. The facade already depends on `secure_file` through compiler tooling;
removing that shared module therefore also rejects facade import. It is not an
optional backend for the whole facade. A missing or broken selected file owner, neural module or secure-file
module fails explicit setup, without caller-directory substitution. The supported
route also works when importing the file owner directly at application top level.

## Unchanged persistence contract

`save_new(path,artifact)` validates the complete checkpoint before creating a file.
The current backend requires Linux, filesystem/FFI/process authority, and an existing
current-user-owned mode-0700 directory. It creates mode-0600 files exclusively,
refuses symlink components and existing destinations, and creates no parents.
`load_file(path)` reads at most 4096 bytes before the existing strict decoder,
then returns the same private diagnostic handle. Denied capabilities, malformed
UTF-8/JSON, duplicate keys, altered digests, oversized files, FIFOs and unsafe
parents fail without an unguarded fallback. Receipt and adapter errors exclude
caller paths. Path selection remains the caller's explicit authority; this binding
is not a new permission grant or a replacement for process sandbox policy.

The payload, quantization, inference, continuation, review and close contracts
are unchanged. There is no automatic persistence, training or global activation.
No atomic visibility, crash durability, encryption, authenticated provenance or
boundary against privileged/same-user malicious processes is claimed. See
[student checkpoints](pbAI_Student_Checkpoints.md) for the full existing contract.

Legacy `neural_specialist_student_save_new` / `load_file` wrappers retain their
signatures and trusted-project-cwd behavior. They still lazily select the file
owner and are not the supported foreign-cwd route. This fixes the explicit file
owner and adds a bound facade route; it does not repair every loader/lazy import.

## Reproduce

- `tests/semantics/pbai_checkpoint_binding_test.sapp`: actual helper and facade,
  22 invalid-binding cases per product, compatible inert module, no provider
  invocation, isolated report mutation and unchanged validation.
- `examples/pbai_student_checkpoint_file.sapp` in PBLang and
  `demo/pbai_student_checkpoint_file.sapp` in standalone: explicit
  `PBAI_CHECKPOINT_TEST_DIR` and `PBAI_CHECKPOINT_TEST_PHASE=write|read`.
  Supply a fresh private directory for write; read runs in a separate process.
- `tests/semantics/pbai_checkpoint_origin_installed_test.sapp`: Linux copied
  libraries with original source directories hidden and a network-isolated child.
  Set `PBLANG_BIN` to a trusted retained-runtime executable. Run separate bounded
  phases through `PBAI_CHECKPOINT_ORIGIN_PHASE`: `files` (foreign cwd without a
  manifest), `trusted` (also tests legacy compatibility), `decoy` (foreign valid
  manifest/lock with eight decoy modules), `guards` (file rejection and permission
  denial), `unavailable-io`, `unavailable-storage`, `unavailable-neural` (missing
  and broken selected dependencies). Each phase checks both actual facades and
  cleans its owned fixture. `PBAI_SOURCE_HIDE_ROOT` optionally hides another source
  tree. The outer harness needs fixture/process permissions and its own minimal `/dev` for child `/dev/null`;
file children retain
  the existing filesystem/FFI/exec grants while denying network, HTTP and SQLite.

The three positive directory phases compare exact checkpoint bytes and public
example output between products; loaded prediction is checked in a fresh process.
Tests use synthetic data only. Linux retained-runtime evidence is not universal
platform support, native-bundle validation, global installation, AGI/ASI or general
model quality. Timings are local diagnostics with shared-desktop noise.

Run `bench/pbai_checkpoint_file_origin.sapp` with a fresh private
`PBAI_CHECKPOINT_TEST_DIR` for one warmup and seven samples of five file-load plus
predict operations. Keep the benchmark/runtime/checkpoint unchanged for baseline
and candidate comparisons. Import/setup/training/save time is excluded; report
raw samples, median and nearest-rank p95 without claiming a general speedup.

Recorded local medians for five load+predict calls were 124.155 -> 36.792 ms
(shared PBLang) and 207.540 -> 25.848 ms (standalone); nearest-rank p95 was
126.984 -> 42.097 ms and 375.286 -> 27.256 ms respectively. The 464-byte artifact,
checkpoint digest and prediction stayed identical. The standalone baseline was
noisy, so these are bounded workload observations, not a portable speedup claim.
See the [raw evidence](evidence/pbai_checkpoint_origin-2026-10-04.json).
