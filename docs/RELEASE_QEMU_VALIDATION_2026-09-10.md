# Release admission and QEMU validation — 2026-09-10

This continuation publishes bounded release verification and emulator-report
improvements on top of `09988d6`. Release admission was committed as `42c4668`.
All new product logic is PBLang. This is selected regression evidence, not
production readiness or complete platform coverage.

## Release admission

The v2 publisher signature binds the artifact digest interpretation, signed byte
count, release identity and scope. Admission verifies a bounded canonical key
store, optionally matches its caller-supplied raw-byte SHA-256, and measures one
sealed Linux artifact snapshot. The snapshot copy limit is the signed byte
count, preventing an oversized source from consuming unreserved capacity.
There are eight admission slots and a 64 MiB aggregate snapshot ceiling, with
16 MiB per artifact. These are snapshot bounds, not a VM heap limit.

The new regression points a valid 26-byte signed request at a 27-byte file and
requires rejection before snapshot allocation, with unchanged slot/byte counts.
Open/digest exceptions are caught at the admission boundary; a returned reader
is closed, or its slot remains poisoned if cleanup cannot be confirmed. Both
handle generators stop after eight attempts. Exception/collision behavior was
reviewed statically; deterministic backend-fault injection was not performed.

The optional cooperative floor uses private compare-and-swap, signed increasing
sequence numbers and nondecreasing policy/store generations before publishing
a handle. Its root and the expected trust-store digest remain caller-selected.
Operator trust, fresh revocation, authoritative rollback prevention and handoff
to installation/execution are not established. Handles remain copyable and all
execution-authorization and production-readiness flags remain false.

## Emulator reports and external execution

Differential reports now use `pblang.emulator-differential.v2`. Legacy v1 reports
must be regenerated. Missing declared fields fail comparison even when absent
on both sides; explicitly present `nil` values can still compare equal. A JSON
round-trip regression confirms missing results cannot satisfy coverage.

The QEMU tool supports the fixed system executable or a paired explicit path
and digest. The latter measures and holds a sealed executable snapshot before
launch. Five generated AArch64 programs passed through the existing cached
QEMU in this continuation:

| Case | Expected exit | Observed exit |
|---|---:|---:|
| add | 12 | 12 |
| multiply | 42 | 42 |
| bounded-loop | 11 | 11 |
| zero-branch | 19 | 19 |
| signed-compare | 1 | 1 |

The tool returned zero, all five lifecycle comparisons passed, and no new
campaign directory remained. Its JSON report passed the PBLang coverage-matrix
schema and digest round trip. `structurally_complete=true` coexists with
`complete=false` and `independent_execution_verified=false`: the executable's
caller-selected digest does not authenticate its publisher or independence.
QEMU receives a mutable ELF pathname, so same-object handoff remains unverified.
No guest operating system or physical device was exercised.

## Validation

Two isolated checkouts contained only the selected publication files and
committed dependencies. Both built successfully; portable lockfiles were
regenerated with `tools/pblock.sapp`. After combining both slices, the final
checkout was built again and the nine affected suites were rerun.

- Release suites: `publisher_trust_store`, `publisher_artifact_admission`,
  `publisher_release_floor`, `release_publisher_attestation`,
  `product_release_evidence`.
- QEMU suites: `emulator_diff`, `codegen_arm64`, `codegen_arm64_exec`,
  `pbemucheck_options_live` (five negative invocations).
- Supporting suites passed in the isolated release checkout before the final
  admission hardening: `secure_file_capability`, `secure_file_transaction`,
  `ed25519_verify`, `serialization_hardening`.
- Seventeen emulator/device-model suites passed on the broader working tree
  with the fresh retained binary before the final differential-report fix.
  The affected comparison/ARM64 suites were then rerun in isolation.
- Source checks: 13 files, zero errors, one existing ARM64 warning. Lint exited
  successfully with style warnings. Security review, privacy scanning and
  whitespace checks covered the selected publication.

Tests received a 60-second outer deadline, 2 GiB address-space ceiling, 32 MiB
per-file ceiling and disabled core dumps; the artifact-capacity fixture needs
16 MiB files. They ran as trusted maintenance with `--allow-all`, not in a
hostile-code sandbox. QEMU children additionally used their own 7-second wall,
3-second CPU, 512 MiB address-space and 4096-byte captured-output limits.
The live QEMU report was below 256 KiB and contained no private oracle path or
digest. The 2048-task child limit is shared with the host UID.

The retained binary SHA-256 was
`7f1b97c0037736553ab39d165c13c36f3fd8497bc6aca9ca94a4c680858e6500`.
`pblang check` selected its original checkout's driver when invoked from an
isolated tree, triggering a dependency hash mismatch. Validation therefore ran
the documented local `stdlib/check_cli.sapp` driver and checked its explicit
error-count output; hash validation was retained. This bootstrap driver-location
issue remains open.

## Remaining work

Hard VM memory/time enforcement, safe native ABI/isolation, authenticated audit
reports, operator-owned release trust and immutable execution handoff remain
open. Compiler compatibility needs broader backend/runtime coverage. PBIDE has
bounded draft tabs and Check, while file-backed editing, highlighting, Run,
Designer and code synchronization remain incomplete. The VM lab still needs an
operational disposable-guest controller and actual guest OS validation.
Other local UI, inventory and platform changes remain outside this publication.
Graphify regeneration exceeded its 45-second deadline and remains incomplete.
