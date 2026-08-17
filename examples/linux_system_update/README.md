# linux_system_update

PBLang-only migration of `update.sh`.

- Original SHA-256: `f59ff1db27fe64654b4793908e5c305bce32c10dfa7c82c7b2de41adc417ab16`
- Risk classification: `critical`
- Replacement: one confirmed package operation at a time without sudo.
- The original file is never executed by this example.

Run without arguments for a safe preview or pure in-process demonstration:

```sh
./target/release/pblang run examples/linux_system_update/main.sapp --allow-all
```

Effectful examples remain dry-run until the exact `--apply --confirm=...` pair printed by the preview is supplied.

