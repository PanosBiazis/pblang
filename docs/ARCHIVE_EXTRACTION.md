# Extract one verified ZIP member

`stdlib:archive_publication.sapp` exports
`extract_member(path, bytes, name, options=nil)`. It verifies the whole ZIP, then
publishes one selected **nonempty regular file** to a new caller-chosen path.
`name` is an exact case-sensitive archive selector, never a destination path.
There is no automatic directory-tree extraction, parent creation or overwrite.

```pblang
let A=import("stdlib:secure_archive.sapp");
let P=import("stdlib:archive_publication.sapp");
let zip=A.pack([map{name:"nested/binary.bin",data:[0,128,255]}],"store");
let result=P.extract_member("new.bin",zip,"nested/binary.bin");
print json_stringify(result);
```

Run from the project/candidate directory with the retained runtime:

```sh
pblang run tools/archive_extract.sapp --allow-all -- --extract INPUT.zip MEMBER NEW_FILE
pblang run examples/secure_archive/extract.sapp --allow-all -- NEW_FILE
```

Quote paths/selectors containing spaces. Exactly one JSON result is printed;
success exits 0, rejection or uncertain completion exits 1. Missing `--extract`
prints fixed usage and performs no input read. Names, contents, paths and raw
backend errors are omitted from results. No content is executed or previewed.
Arguments can still be visible to local process observers and shell history.
The CLI is a trusted local tool requiring filesystem/FFI/helper authority;
`--allow-all` is not a sandbox or an offline MCP filesystem grant.

## Verification before output effects

The CLI reads at most 4 MiB using the held-directory no-follow bounded reader,
then requires confirmed input-wrapper closure. The API owns a byte-array copy,
checks all names, counts, sizes, local ranges, ZIP flags/methods and every member's
CRC/decompression before opening the output directory. Corruption in an
**unselected** member rejects the operation. The pure computation API
`secure_archive.read_member(bytes,name,options=nil)` implements this selection
and returns the full selected payload (not the 64 KiB preview), `size`, `name`,
`directory`, `truncated:false`, `verified:true`, `effects_performed:false`.
Unlike `unpack`, its result retains only one payload. This does not promise a
particular VM garbage-collection schedule or physical memory ceiling.

Defaults: 256 entries, 4 MiB input/expanded total, 1 MiB per file. Explicit options
have ceilings 1024 entries, 16 MiB total and 4 MiB per file. Selected output is
therefore at most 4 MiB, with no truncation. Binary bytes are opaque. STORE and
DEFLATE are supported; encrypted/ZIP64/multi-disk archives, links, traversal,
case collisions and ambiguous portable member names are rejected. CRC is not
cryptographic authentication or malware detection.

The filesystem publication and uncertainty rules are exactly those of
[archive publication](ARCHIVE_PUBLICATION.md): held no-follow output parent,
sealed selected bytes, mode-0600 anonymous `O_TMPFILE`, complete bounded copy,
file fsync, exclusive link of the held inode, directory fsync. The input archive
is never modified. Existing targets, including symlinks, hard links and folders,
are preserved. The output suffix is unrestricted. Paths have a 4096-character
and UTF-8-byte ceiling, reject NUL/backslash/trailing slash; the backend rejects
invalid components and symlink parents. No weaker fallback is used.

## Recovery and limitations

Results use `format:"pblang.archive-extraction.v1"` with the publication evidence
fields documented above. `invalid_selection` covers archive/limits/selector
failure without disclosing private names. `unsupported_member` explicitly
rejects selected empty files and directory entries **before output effects**:
the reused anonymous-range primitive currently requires at least one byte.
Empty/unselected directory members remain valid within the ZIP.

`output_may_exist:true` means inspect the destination before any retry. A failure
after linking may leave the entire selected file in place; internal close errors
can prevent confirmed publication evidence. Never automatically retry or delete
on an uncertain result. `partial_output_possible:false` concerns only a new file
published by this invocation, not existing files, later modifications or crash
recovery. Wrapper closure and fsync acknowledgments have the documented limits.

Execution requires the Linux x86-64 backend, `/proc`, memfd sealing, helper
execution and filesystem O_TMPFILE support. No GUI renderer, Windows/macOS
implementation, full-tree transaction, deletion, encryption, authenticity,
other archive formats or content viewers are included. The older low-level
`zipfile.extract` is incremental: it can create earlier files before a later
entry fails and does not inherit this facade's full-validation/budget contract.

## Tests

```sh
pblang run tests/semantics/archive_extraction_test.sapp --allow-all -- SCRATCH
python3 tests/semantics/archive_extraction_cli_test.py /absolute/pblang-native
python3 tests/semantics/archive_publication_cli_test.py /absolute/pblang-native all EVIDENCE_DIR extract
python3 tests/semantics/archive_publication_evidence_test.py /absolute/pblang-native extract
```

Synthetic tests cover binary/full-length STORE/DEFLATE, all-member corruption and
policy rejection, absent/empty/directory selection, limits, real no-replace and
symlink behavior, a deterministic parent swap, two concurrent writers, injected
ENOSPC after 65536 actual output bytes, fsync failures before/after linking and
internal-close uncertainty. The model tests describe result projection; strace
fault tests exercise real I/O. Neither is proof of every concurrent schedule.
