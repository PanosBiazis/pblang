# linux_start_ide_backend

PBLang-only migration of `start-ide-backend.sh`.

- Original SHA-256: `ec97d8be4bd99e1c0e12fa6cbda5581632a38e78066588148d1e2b3509af0ee3`
- Risk classification: `medium`
- Replacement: validated loopback backend plan.
- The original file is never executed by this example.

Run without arguments for a safe preview or pure in-process demonstration:

```sh
./target/release/pblang run examples/linux_start_ide_backend/main.sapp --allow-all
```

Effectful examples remain dry-run until the exact `--apply --confirm=...` pair printed by the preview is supplied.

