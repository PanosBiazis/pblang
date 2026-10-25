# Literal relative directory paths

`stdlib:linux_dirfs.sapp` provides two explicit read-only operations for callers
that need to address arbitrary ordinary child names, including `root`:

```sapp
let Dir = import("stdlib:linux_dirfs.sapp");
let held = Dir.open_absolute(project_directory, false, false);
if not held.ok { throw held.error; }
let children = Dir.list_relative(held);           // list the held directory
let nested = Dir.list_relative(held, "root");     // list the actual child root
let payload = Dir.read_bytes_relative(held, "root/data.bin", 4096);
let closed = Dir.close(held);
```

The ordinary result maps match `list` and `read_bytes`: listings provide `ok`,
`path`, `entries`, and `truncated`; reads provide `ok`, `path`, `bytes`, and `size`.
Failures provide `ok: false` and an error. Check results and close the held handle
when finished. These calls do not confer general exception-safe lifetime control.

| Operation | Empty string | `.` | `root` |
|---|---|---|---|
| Existing `list` | held root | held root | held root alias |
| New `list_relative` | held root (also the default) | rejected | actual child directory |
| Existing `read_bytes` | rejected | rejected | rejected alias |
| New `read_bytes_relative` | rejected | rejected | actual child file |

Nested paths such as `root/root/value.sapp` have literal components. Absolute
paths, `..`, interior `.`, empty components, trailing separators, NUL bytes and
oversized components fail. No path normalization is performed to make an invalid
path acceptable. Existing operations keep their previous aliases and semantics.

Both API pairs share their lower-level implementation. Component traversal uses
the existing held directory descriptor and no-follow opens. Renaming/replacing
the pathname used to open the root does not redirect that held descriptor.
Content reads retain the same anonymous-file snapshot and byte limit (default
4 MiB, maximum 64 MiB). Listings retain the default 512/maximum 4,096 entry cap
and report truncation, **but still materialize the host listing first**. These
APIs do not inherit the separate `enumerate_bounded` token, mount-admission or
pre-allocation guarantees, and they do not claim filesystem-wide atomicity.

`locked_source_snapshot` now uses these APIs for both project module discovery
and dependency verification. This removes its previous top-level `root`
restriction without changing the package-tree-v2 digest contract. Other callers
retain their own existing profiles; this change does not remove reserved-path
rules from `package_folder` or change native `bundle` behavior.

`linux_dirfs_relative_paths_test.sapp` checks aliases versus literal names,
binary content, limits, invalid paths, links, closed handles and a renamed root.
The locked-snapshot tooling test checks native lock compatibility and real
source-hidden bundle execution for `root` as file, populated directory and
empty directory. All production implementation is PBLang, using the existing
Linux backend; no new FFI symbols or libraries are introduced.
