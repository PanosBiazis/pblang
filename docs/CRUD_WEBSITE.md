# Native CRUD example and product

The canonical runnable example is [examples/crud_website](../examples/crud_website/README.md).
The standalone [crud-website-pblang](https://github.com/PanosBiazis/crud-website-pblang)
product shares the exact same stdlib implementation.

`crud_records.sapp` offers validated detached records, configurable capacity,
numeric-ID ordered pagination/search, revision-checked writes and explicit close.
`crud_preview.sapp` adds generated responsive HTML/CSS, loopback request guards,
strict form decoding and the bounded Linux listener. No polyglot code executes
in the native path. Source being PBLang does not eliminate the retained host
runtime or existing libc FFI boundary.

This is a volatile single-user local learning preview. It is not a production
identity, database or access-control service. Other local processes are trusted.
There are no analytics, remote assets or persistent record files. Private data
must not be placed in it. Public deployment, persistent native storage, account
parity with PHP, multi-process access and non-Linux listeners remain pending.

Focused validation covers malformed/duplicate fields, Unicode, output escaping,
Host/Origin/CSRF, forbidden methods, record aliasing, capacity and stale writes.
A real browser submitted the native form. Same-origin referrer policy preserves
Origin for browser form POSTs without sending referrers to other origins.
No performance improvement is claimed; sorting a page currently scans and sorts
the bounded record set. Existing VM/FFI/GC compatibility limits remain applicable.
