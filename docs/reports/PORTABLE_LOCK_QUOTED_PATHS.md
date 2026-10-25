# Quoted local dependency paths — 2026-09-23

`portable_lock` previously split inline dependency fields at every comma and
recognized only double quotes while removing comments. A valid literal path such
as `'../lib,a#b=c'` was truncated or rejected. Quoted `path` keys could also be
ignored. The original implementation fails the new regression with
`portable_lock: expected a quoted TOML string`.

The PBLang implementation now tracks both quote types, basic-string escapes and
matched nested array/table delimiters. It separates only outer unquoted commas,
recognizes quoted keys/names, and rejects duplicate paths and malformed delimiter
structure. Declared relative path validation, observed-path identity checks,
content digests, document limits and filesystem publication policy remain intact.
This is a scoped parser for the supported declaration subset, not full TOML support.

Regression tests exercise literal/basic strings, escaped quotes, commas/hash/equals
inside paths, nested metadata, comment suffixes, quoted names/keys, mismatched or
missing delimiters, duplicate path keys, absolute paths, identity mismatch and
idempotence. Linux passes: `portable_lock_test`, `portable_lock_current_test`,
`portable_lock_quoted_test`, and `resource_scope_test`; the first includes real
local atomic publication. The synthetic example also passes. The quoted-path
normalization test passes with the retained Windows executable on the authorized
workstation. The Windows fixture remaps stdlib imports locally and writes no lock.

Owning packages and the frozen desktop package were built, then their locks were
published through `tools/pblock.sapp`. These tests use isolated committed import
closures plus this change, preserving unrelated working-tree edits. This is a
correctness fix; no parser performance improvement or universal compatibility
is claimed. The retained loader's pre-existing same-directory `stdlib` path
resolution limitation remains; the fixtures use `../pblang/stdlib`.

Windows publication is still incomplete: the publisher uses Linux directory
handling, and Windows private-root/ACL and native bridge guarantees are not yet
available. This change does not enable that route or bypass its checks. Broader
runtime/FFI, GC/cycle, polyglot and hardware work remains separate.

Final review checked quoted delimiters, path identity/digest preservation, viewer
state invalidation and cleanup/consent boundaries. No new native code or external
capability was introduced. Existing foreign/runtime boundaries are not certified
by this scoped review. All 367 pre-existing changed files across the three repositories
matched the preservation snapshot. Graphify completed for PB-MCP; the PBLang refresh
was stopped after 35 seconds. Graphify does not provide semantic `.sapp` coverage.
