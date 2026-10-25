# Application and process names

`stdlib:process_identity.sapp` supplies a one-time presentation label. It does
not change executable contents, argv, signing identity, privileges or process
ownership. Never authorize operations or terminate a process by this label.
Use a retained process handle/pidfd for identity-sensitive operations.

```pblang
let Identity = import("stdlib:process_identity.sapp");
let result = Identity.apply("my-pblang-app");
```

The default `auto` mode tries the current Linux process's fixed
`/proc/self/comm` path, verifies the result, and returns a structured failure
if filesystem access is denied or the platform is unsupported. `keep` performs
no OS access. No background thread, polling loop, FFI or new permission grant
is added by this module. The underlying filesystem runtime may use I/O workers.
This optional presentation update is not a synchronization or security primitive.

Linux labels fit 15 UTF-8 bytes; truncation keeps complete characters. This
kernel display limit does not limit source files, application size or executable
filenames. Invalid paths, control bytes, empty labels and surrounding whitespace
are rejected. The return field `identity_verified` is always false on a planned
or applied name; an error result must never be treated as verified identity.

`tools/pbrun.sapp` defaults to the artifact basename without a recognized
extension. `--process-name NAME` overrides it; `--keep-process-name` opts out.
`--info` does not apply a name. An unavailable naming feature never prevents the
artifact from running. Source-level library imports do not rename the caller.
Plain bootstrap `pblang run file.sapp` is unchanged: applications can opt in at
startup. The PB-MCP operator and worker source entrypoints use dedicated labels;
this does not update already installed or running desktop packages.

Windows/macOS process presentation, PE version resources, Linux launchers and
Apple bundle display names require their own packaging support. A label is not
a standalone executable: native output naming remains a build/package concern.

## Validation

- Pure tests: malformed input, basename privacy, Unicode boundaries, mode handling.
- Dedicated Linux child: real read/write/readback, idempotence and restoration.
- Separate CLI children: a compiled 30,000-iteration artifact retains its output,
  with default/custom/kept names observed through `/proc`; malformed label flags
  exit with code 2; inspection does not execute the artifact.
- The self-hosted VM does not expose every retained-host builtin: directly using
  `fs_read` inside the initial test artifact failed. Naming is deliberately done
  by the runner before VM entry, and the successful test observes the OS label
  externally instead of claiming VM builtin parity.

See [process cleanup policies](PROCESS_SCOPES.md) and
[program signatures and domains](SIGNATURES_AND_DOMAINS.md).
