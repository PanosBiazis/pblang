# Complete new ZIP publication

`stdlib:archive_publication.sapp` exports `save(path, bytes, options=nil)`.
It publishes a caller-supplied ZIP byte array to one new `.zip` path. It never
extracts members, walks directories, replaces an existing name or creates parents.
The existing `secure_archive` computation API remains effect-free.
The additional `extract_member` entry point is documented in
[selected-member extraction](ARCHIVE_EXTRACTION.md); `save` retains its ZIP-only contract.

```pblang
let Archive = import("stdlib:secure_archive.sapp");
let Publication = import("stdlib:archive_publication.sapp");
let bytes = Archive.pack([map{name:"hello.txt",data:str_to_bytes("Hello")}],"fast");
let result = Publication.save("new.zip",bytes);
print json_stringify(result);
```

The opt-in CLI reads one existing input with the bounded no-follow descriptor
reader, closes its wrapper root, then invokes the same API:

```sh
pblang run tools/archive_publish.sapp --allow-all -- --publish INPUT.zip NEW.zip
pblang run examples/secure_archive/publish.sapp --allow-all -- NEW.zip
```

CLI success exits zero; rejected or uncertain results exit one. Errors and results
omit caller paths, member names, payloads, hashes, descriptors and raw native errors.
`--allow-all` is for trusted local code with the necessary filesystem, FFI and child
process authority; it is not a filesystem grant to the offline MCP worker. No MCP
tool, grant, shared runtime executable, network or hardware behavior changes.

## Validation and publication

The API rejects non-arrays and arrays over 4 MiB before copying. It owns a shallow
copy of the scalar byte array and calls `secure_archive.inspect` before opening
the output parent. Every member is decompressed/CRC checked, with the existing
STORE/DEFLATE, path, collision, encryption and ZIP64 rejection policies. The same
`max_entries`, `max_total_bytes`, `max_file_bytes` options and hard ceilings apply;
invalid options fail before publication. This is bounded ZIP validation, not
archive authentication, encryption, trusted authorship or malware inspection.

Output paths must end in `.zip` (case-insensitive), be at most 4096 characters and
4096 UTF-8 bytes, and contain no NUL or backslash. The existing backend further
rejects invalid components and symlink parents. Unsupported formats fail closed.
The adapter does not reinterpret bytes from another format based on its suffix.

The existing Linux backend opens every parent component with `O_NOFOLLOW`, seals
the validated snapshot in a memfd, copies it into a private mode-0600 anonymous
`O_TMPFILE` inode, verifies the length/mode and fsyncs before publication. Its
PBLang-generated x86-64 helper links the held inode through `/proc/self/fd` with
`linkat(AT_SYMLINK_FOLLOW)` and a held destination-directory descriptor. There is
no mutable source staging pathname, no replacement and no path-based cleanup.
The directory is fsynced after publication. Short writes are handled by the
existing bounded copy loop; incomplete writes never publish this operation's
partial payload under the new destination name.

## Result evidence

All API results have `format:"pblang.archive-publication.v1"`, `ok`,
`publication_attempted`, `publication_confirmed`, `output_may_exist`,
`partial_output_possible`, `durability_verified`, `wrapper_handles_closed`,
`metadata_only:true`, `safe_to_log:false`. Failures add fixed `code`/`error`.

| Situation | Evidence |
| --- | --- |
| Validation/platform/root/snapshot rejection | `publication_attempted:false`, `written:false`, `output_may_exist:false` |
| Copy/publication called, outcome unconfirmed | `publication_attempted:true`, `output_may_exist:true`; `written` omitted |
| Backend confirms publication | `publication_confirmed:true`, `written:true`, `atomic_visibility:true` |
| Directory fsync fails and backend reports `published:true` | Publication evidence retained; `ok:false`, `durability_verified:false` |
| Wrapper root close fails | `ok:false`, `wrapper_handles_closed:false`, fixed `cleanup_error`; earlier publication/error evidence retained |

`written:false`/`output_may_exist:false` describe effects of this invocation before
the publication call, not absence of a pre-existing destination. Likewise,
`partial_output_possible:false` means this operation cannot publish a partial
**new** file. It does not certify completeness of a collided file or prevent
another actor from changing files later. Do not automatically retry or delete a
destination on failure. A complete archive can exist after an uncertain result.

`wrapper_handles_closed` reports only the one root-close operation consuming the
adapter's root and registered child. A failed root acquisition returns false
conservatively. It does not certify all internal native allocations/descriptors:
the existing backend has separate FFI exception/cleanup boundaries, including
internal closes whose result is not propagated. Durability means fsync
acknowledgment, not a power-loss or storage-hardware guarantee.

## Availability and verification

Supported execution is the existing Linux x86-64 backend with `/proc`, sealable
memfd, O_TMPFILE filesystem support and its helper execution authority. Other
platforms/capability failures reject without a weaker direct-write fallback.
Input snapshots are bounded observations, not transactional snapshots against
concurrent in-place source mutation. Caller byte-array mutation during copying,
host timeouts/crashes, generic FFI cleanup and hostile in-process code are not
solved by this adapter.

An internal anonymous-inode close failure after linking can make the reused
backend return a generic error without `published:true`, including when directory
fsync also fails. The wrapper then correctly reports `publication_confirmed:false`,
`output_may_exist:true`, omits `written`, and may still report
`wrapper_handles_closed:true`. That last field does not certify the internal
inode close. The modeled combined failure concerns wrapper closure; it is distinct
from the native internal-close fault tests.

Run the semantic suite with a fresh existing scratch directory:

```sh
pblang run tests/semantics/archive_publication_test.sapp --allow-all -- SCRATCH
python3 tests/semantics/archive_publication_cli_test.py /absolute/pblang-native
python3 tests/semantics/archive_publication_evidence_test.py /absolute/pblang-native
```

The semantic test checks pack/publication/inspect/unpack, empty ZIP, malformed/CRC
and limit failures. The CLI harness uses independent Python ZIP fixtures/readback,
symlink/hardlink/directory/same-input collisions, byte/inode preservation, bounded
input rejection, real injected fsync errors before/after linking, and ENOSPC after
the first actual 65536-byte output chunk. It requires Linux `strace`.
The evidence test separately models seven backend outcomes plus validation before
root acquisition. Its deterministic parent-swap interposer moves the already-open
synthetic parent and replaces its pathname with a symlink; real publication must
reach the held original directory and leave the replacement untouched. This is
controlled interposition, not proof over every possible concurrent schedule.
