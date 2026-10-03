# Controlled modularity for PBLang tools and products

A project keeps its own install/run/test/release identity in `pblang.toml` and
its content integrity in `pblang.lock`. A separate versioned composition
contract describes which **declared interfaces** it expects from other modules.
The contract does not replace package installation, verify an implementation's
exports, or grant permission to use a device, network, filesystem or process.

The PBLang implementation reuses strict SemVer validation, bounded/duplicate-key
JSON admission from `serialization`, and Kahn topological ordering from `graph`.
It never discovers repositories, searches source checkouts, downloads a provider,
or executes an interface. Host/product adapters must observe actual providers
and capabilities, validate real data, and enforce execution permissions.

## Contract v1

`stdlib:controlled_modularity.sapp` exports:

- `validate(document) -> bool`: exact closed schema, all entries bounded.
- `parse(text) -> {ok, code, document}`: bounded strict JSON; invalid input returns
  `code:"invalid_contract", document:nil` without returning private input.
- `resolve(document, root, available=nil) -> plan`: selects only the named root's
  dependency closure. Omitted/null availability means an empty list.

A document has exactly `format:"pblang.controlled-modularity.v1"` and `modules`.
Each module has exactly:

```json
{
  "id": "example_app",
  "version": "1.0.0",
  "interfaces": {},
  "capabilities": [],
  "dependencies": [{
    "slot": "value",
    "module": "value_provider",
    "version": "1.0.0",
    "interface": "value",
    "interface_version": "1.0.0",
    "optional": false,
    "fallback": null
  }]
}
```

The provider declares `interfaces:{"value":"1.0.0"}` and its own independent
package version. Pins are exact canonical SemVer strings, including any build
metadata; no implicit major compatibility or version range is inferred. Unicode
JSON is accepted, but module/interface/capability/slot IDs are deliberately
restricted to 1–64 ASCII letters, digits, `_`, `-`, `.`. IDs are not source paths.

Every field above is mandatory. Bounds: 1–64 modules, 16 interfaces/capabilities/
dependencies per module, 256 dependency declarations total, and 64 available
capability names. IDs and slots must be unique in their scopes. Serialized input
is limited to 256 KiB UTF-8, depth 12, 16,384 JSON items and 8,192 lines. Unknown
fields, duplicate JSON keys, malformed versions and out-of-bound data fail closed.
Direct map validation is iterative over this finite fixed schema, without
recursive traversal of arbitrary values. Plans contain copied scalar metadata,
not borrowed mutable module records.

## Required, optional and fallback behavior

Required dependencies must be present and compatible. Optional absence returns
an explicit disabled slot, or selects the one explicitly named fallback. A
fallback must satisfy the same exact package/interface pins. A required edge
cannot declare a fallback; a fallback cannot name the primary again.

Fallback is **only** for an absent primary. A present provider with an incompatible
version, missing interface, unavailable declared capability or cyclic selected
graph causes rejection. There is no silent fallback after a provider failure,
no arbitrary callback, and no provider search. Absence is determined solely from
the supplied catalog, so a host must build that catalog from trustworthy evidence.
Malformed entries are rejected even if unselected; graph compatibility, capability
availability and cycles are checked for the selected root closure. Other valid,
unselected projects do not have to be runnable for this root to work.

A successful `pblang.module-plan.v1` contains dependencies-first `order` (dependency-depth layers, then sorted IDs) and
`bindings` sorted by consumer/slot. A binding contains `consumer`, `slot`,
`provider` (or null), `status` (`primary`, `fallback`, `disabled`), `interface`,
`interface_version`, and exact requested `version`. Input catalog order does not
change a successful plan. `capability_claims_verified:false` and
`execution_performed:false` are always explicit.

Failures contain empty `order`/`bindings`, a fixed `code`, and only validated
module/slot IDs where appropriate. Codes are `invalid_contract`, `invalid_request`,
`missing_root`, `missing_required`, `missing_fallback`, `package_version_mismatch`,
`missing_interface`, `interface_version_mismatch`, `missing_capability`, and
`dependency_cycle`. A plan is caller-owned metadata, not an authorization token.

## Real tool integration

The separate `controlled_modularity_file.check(path, root, available=nil)` uses
`secure_file.read_text` with a 256 KiB bound and no-follow handling. It returns
`contract_unavailable` when secure file access is unavailable/denied; it never
falls back to an unrestricted file read. Parser errors stay generic. Explicit
file paths belong only to this I/O adapter, not the composition schema.

```sh
# From a functioning PBLang installation/project root:
pblang run tools/pbmodules.sapp --allow-all -- examples/controlled_modularity/contract.json value_provider
pblang run tools/pbmodules.sapp --allow-all -- examples/controlled_modularity/contract.json example_app
pblang run tools/pbc.sapp --allow-all -- examples/controlled_modularity/integrated.sapp --target pbck --bundle --out integrated.pbck --module-contract examples/controlled_modularity/contract.json --module-root example_app
pblang run integrated.pbck --sandbox
```

The provider can be planned independently without a consumer. The integrated
example uses an explicit packaged static import and prints `7` from the compiled
provider. The existing compiler performs real source/bytecode checks and static
linking. The optional contract preflight runs **before source reads and output
writes**; refusal preserves an existing artifact. `--module-capability NAME`
passes an explicit caller claim and can be repeated with distinct names. It does
not enable a compiler flag or runtime permission. Existing invocations without
module options retain their behavior.

This preflight verifies the supplied composition declaration, **not** that every
actual source import/export matches it. It does not attest the compiler/runtime
binary, replace native source admission, enforce teardown, or make an untrusted
module safe. Trusted product integrations should obtain descriptors through
fixed provider adapters and validate operations independently before execution.

## Toolchain versus product requirements

The same schema can describe a compiler frontend, a PBCK runtime and a product.
`toolchain-contract.json` is an illustrative declaration for the PBLang 0.6.0
retained toolchain: the frontend depends on an explicitly versioned runtime
interface and a caller-observed `native_pbck` capability. Resolve it with root
`pbc_frontend` and that capability only after separately qualifying the runtime.
These sample IDs/versions are contract declarations, not a new binary ABI promise.

A product's descriptor declares its own version and interfaces independently.
Its `pblang.toml` still controls package dependencies; portable lock publication
and [installed dependency materialization](INSTALLED_DEPENDENCY_LOCKS.md) keep
installation roots explicit. Ship the required library/runtime files inside a
reviewed installation or a supported self-contained artifact. Do not infer a
provider from a neighboring source checkout. A composition plan alone proves
neither installability nor standalone execution.

## Platform/capability matrix

| Layer | Mechanism | Current qualification / refusal |
|---|---|---|
| Pure validation/planning | PBLang maps, arrays, strict SemVer and JSON, graph traversal | Tested on retained Linux runtime; other conforming runtimes need the same fixtures |
| Contract-file adapter | Existing secure-file backend | Linux integration tested; macOS/Windows implementations exist but are not qualified by this slice |
| Web/mobile/embedded hosts | Host supplies an already acquired bounded JSON string to `parse` | Explicit portable entry point; actual host acquisition/permissions must be provided and tested |
| Static PBCK compiler integration | Existing `pbc --target pbck --bundle` plus opt-in preflight | Linux retained compiler/VM tested; no universal bootstrap compatibility claim |
| Device/accelerator/remote adapter | Product-owned probe + validation + permission checks | Unavailable required capability rejects; absent optional provider only uses declared fallback |

There is no OS/version whitelist pretending to prove support. `available` names
must come from the host's actual probes; they remain claims from the resolver's
perspective. A deterministic software fallback is a separately declared provider
with the same required contract; its actual output must be tested by the product.

## Tests and limitations

Unit tests cover standalone/integrated plans, optional disablement, explicit
fallback, exact version/interface failures, capability denial, cycles, schema and
JSON bounds, detached results, catalog ordering and diagnostic privacy. Real CLI
tests cover rejection before a missing source, output preservation, actual bundle
compilation/execution after deleting source/provider/contract files, bad options,
symlink contracts and denied secure-file authority. The portability probe copies
only the transitive library closure and a retained runtime into an isolated
package, removes ambient `PBLANG_PATH`, and executes plans from a foreign cwd.
That probe does not hide the entire host filesystem or qualify other OSes.
