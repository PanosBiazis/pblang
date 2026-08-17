# Linux script ports

This PBLang-only migration covers every file under `Documents/codes/linux` without executing the originals.

Safety rules shared by all translated examples:

- dry-run is the default for host or filesystem effects;
- effectful commands require `--apply` and the exact action-specific `--confirm=...` token shown by the dry-run;
- no shell, `eval`, `sudo`, `--no-sandbox`, command-name process killing, wildcard cleanup, hard-coded disk, public web bind, or embedded database credential;
- host adapters use fixed absolute executable candidates, descriptor handoff, process-group cancellation, bounded output, time, and file writes;
- destructive operations were narrowed: permissions are non-recursive, PID termination uses `SIGTERM`, package-manager actions run one at a time, and NTFS repair requires an explicit device;
- the AIPY mini-project is now the single `products/aipy` PBLang product, exposed through one example folder and implemented with seeded data, parameterized SQLite, escaped responsive HTML, loopback binding, and security headers.
- personal paths, usernames, device identifiers, credentials, clipboard contents, captures, and host output are never embedded or persisted; privacy-sensitive read-only adapters remain explicit per-run actions.

Run the aggregate contract test:

```sh
./target/release/pblang run examples/linux_script_ports/test.sapp --allow-all
```

Each original file also has a separate runnable example directory. Its README records the source hash, risk, and replacement behavior.
