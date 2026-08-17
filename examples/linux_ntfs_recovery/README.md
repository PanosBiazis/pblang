# linux_ntfs_recovery

PBLang-only migration of `ntfsf.sh`.

- Original SHA-256: `5d0c7eb94a8a4887e5a8fcc7b29d03d787c9f7bf178a27d13a711ed31036c211`
- Risk classification: `critical`
- Replacement: explicit-device NTFS repair with no automatic mount.
- The original file is never executed by this example.

Run without arguments for a safe preview or pure in-process demonstration:

```sh
./target/release/pblang run examples/linux_ntfs_recovery/main.sapp --allow-all
```

Effectful examples remain dry-run until the exact `--apply --confirm=...` pair printed by the preview is supplied.

