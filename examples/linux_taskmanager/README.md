# linux_taskmanager

PBLang-only migration of `taskmanager.sh`.

- Original SHA-256: `bf85f46f79b1aa9d64cad964c7048a0e39052379153f967d7c6aef24301d721e`
- Risk classification: `high`
- Replacement: bounded process listing and PID-specific SIGTERM.
- The original file is never executed by this example.

Run without arguments for a safe preview or pure in-process demonstration:

```sh
./target/release/pblang run examples/linux_taskmanager/main.sapp --allow-all
```

Effectful examples remain dry-run until the exact `--apply --confirm=...` pair printed by the preview is supplied.

