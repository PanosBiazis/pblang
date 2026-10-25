# Installed compiler CLI portability

`tools/pbc.sapp` binds its compiler backends and optional contract checker at
script load time. `selfhost_bundle.sapp` likewise binds its guarded native bridge
at module load time. This preserves the installed module origin on the retained
0.6.0 host; deferred imports inside functions otherwise resolve against the
caller's working directory. No Rust rebuild or working-directory change is used.

With a complete installation and a materialized installed dependency lock:

```sh
# Run from the user's project directory; RELEASE is the installed package root.
pblang run "$RELEASE/tools/pbc.sapp" --allow-all -- \
  "program with spaces.sapp" --target pbck --out "program.pbck"
pblang run "program.pbck" --sandbox
pblang check "program with spaces.sapp"
# Explicit PBLang-owned, single-file source checker:
pblang run "$RELEASE/tools/pbcheck.sapp" -- "program with spaces.sapp"
```

Input, explicit output, default output, contract and filesystem bundle import
paths keep their caller-working-directory meaning. The compiler does not change
cwd to the installation or reinterpret relative imports against the entry file's
parent. `--bundle` uses the existing trusted-project filesystem resolver. It is
not a new sandbox, recursive package resolver, or capability grant.

All declared tool backends now load during startup, including those not selected
by the command. A complete standard library is therefore required. This is an
intentional correctness tradeoff on the retained host, not a performance claim.
Compilation options, admission failures and runtime permissions remain separate;
rejected options/source/contract checks do not replace an existing output.

## Checks and the legacy flag

Use `pblang check FILE` for public diagnostics, or the explicit `pbcheck.sapp`
entry for its PBLang-only source analysis. Both return a failing process status
for diagnosed errors and do not execute the checked source. They are distinct
analysis profiles, not claimed to have identical diagnostics for every program.

The qualified Linux launcher now routes **`pblang --check --via-pblang FILE`**
(and `--self-host`) through PBLang-owned structured source analysis with a
nonzero process status on diagnostics. See the [exact argv, permissions and
installation contract](PUBLIC_SELFHOST_CHECK.md). This requires deploying the
new launcher and guard together with a matching installed stdlib and lock;
a source checkout change alone does not update the public command.

The direct retained **`pblang-native --check --via-pblang FILE` remains unreliable
as a CI gate**: its Rust driver can print failure and return success. The shared
`frontend_analyze` structured-result API remains unchanged. This scoped launcher
route is not a native-host or nested `p:run` fix.

## Evidence and scope

`python3 tests/tooling/pbc_portability.py` stages a real standalone installation
and runs it in a minimal Linux bubblewrap filesystem, with source checkouts
unavailable and networking isolated. It covers both a manifestless caller and a
locked caller containing decoy backend modules; PBCK with explicit nil defaults;
plain/cleanup/lifecycle bundles; direct guarded library use; module admission;
x64 execution; WASM instantiation/execution; ARM64 ELF machine header; paths with
spaces; output preservation; denied runtime filesystem access; and source-only
checker success/failure. ARM64 execution, Windows/macOS and other host builds are
not qualified by this Linux test. Temporary fixtures are removed on completion.
