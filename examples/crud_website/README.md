# CRUD website example

A native PBLang record editor with create, read, update, delete, search and
pagination. It shares `stdlib:crud_records.sapp` and `stdlib:crud_preview.sapp`
with the standalone [CRUD product](https://github.com/PanosBiazis/crud-website-pblang).

From the PBLang checkout, refresh the local dependency lock with `pblang install`
after updating the stdlib. Then:

```
pblang run examples/crud_website/main.sapp
pblang run examples/crud_website/main.sapp --allow-all -- --serve
```

Open `http://127.0.0.1:8765` on this computer. Stop with Ctrl+C. The default command
runs a synthetic CRUD demonstration and exits; it opens no listener.

The UI and logic are PBLang source. HTML/CSS are generated strings, not executed
polyglot blocks. The retained host still provides runtime primitives; the bounded
listener currently supports Linux libc only and refuses unsupported platforms.
The non-network model can be tested separately on other runtimes.

This is an **in-memory local preview**, not a multi-user production service.
Restarting erases records. There is no account authentication; other local
processes can access it. It checks Host, Origin, CSRF, duplicate fields, revisions,
input budgets and output escaping. It has no telemetry or remote assets. Use
synthetic data, not confidential records. 10,000 records is the preview capacity;
the core constructor accepts an explicit capacity. These are application budgets,
not PBLang language limits. Concurrent callers must serialize access.

The standalone repository also holds an optional PHP/JS/CSS/HTML edition in
`.sapp` polyglot source containers. It is never executed by this native example.

The current retained runtime requires an explicit FFI library/symbol policy.
The commands above use `--allow-all` only for this reviewed local example; this
is broad runtime permission, not a sandbox. Do not reuse it for untrusted code.
