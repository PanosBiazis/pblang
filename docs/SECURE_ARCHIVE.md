# Bounded ZIP computation

`stdlib:secure_archive.sapp` provides `pack(files, mode="fast", options=nil)`,
`inspect(bytes, options=nil)`, `unpack(bytes, options=nil)` and
`view(bytes, name, max_bytes=4096, options=nil)` in PBLang source.
See `examples/secure_archive/main.sapp` for a binary/directory round trip.

Files are opaque byte arrays. A tree is an explicit array of `{name,data}` maps;
empty directories end in `/`. No directory walking, file writing, process,
network access, active preview rendering or implicit recursion is performed.
The format is ordinary ZIP, not an encrypted or authenticated container. CRC32
checks accidental corruption, not trusted authorship.

The facade accepts STORE and DEFLATE and rejects unsupported flags, encryption,
ZIP64/multi-disk entries, links/special files, traversal, ambiguous portable names,
case collisions and file/directory conflicts. Inspection and preview decompress
and verify every member, including those not displayed. Central/local metadata,
exact DEFLATE consumption, declared output sizes and CRCs are checked. A preview
may be truncated, but verification is not. Unsupported archives fail closed.
Archive comments/extra fields are not interpreted as instructions or displayed.

Default limits: 256 entries, 4 MiB archive, 4 MiB expanded total, 1 MiB per file.
Options `max_entries`, `max_total_bytes`, `max_file_bytes` have hard ceilings of
1024, 16 MiB and 4 MiB respectively. Preview output is at most 64 KiB. Names are
bounded to 512 characters and 32 components. These are logical byte budgets;
VM array storage and temporary allocations require more physical memory.
All content remains untrusted. Returned names are not authorization to extract
onto a filesystem; secure destination handling is a separate operation.

`fast` compares fixed-Huffman DEFLATE against STORE and avoids dynamic-Huffman
construction; `auto` retains the existing three-way compression choice; `store`
skips compression. No mode is claimed fastest or smallest for every workload.
The lower-level `zipfile.create(files)` keeps its default `auto` behavior and
accepts the optional mode. Existing low-level APIs do not inherit every facade
path policy. `flate.inflate` accepts optional `exact=true` for strict framing.
The codec is PBLang; execution still uses the retained runtime.

Format references: [PKWARE ZIP APPNOTE](https://pkware.cachefly.net/webdocs/casestudies/APPNOTE.TXT)
and [RFC1951](https://www.rfc-editor.org/info/rfc1951/). The supported subset is
explicitly narrower than the entire ZIP specification; this is not a claim to
read every archive format or filesystem metadata convention.
