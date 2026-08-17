# linux_compile_native

PBLang-only migration of `compile_native.sh`.

- Original SHA-256: `a302eb36e7a2e1bdf1091b459a87145d493f6be99e35c04b48b7a446d0a62925`
- Risk classification: `medium`
- Replacement: PBLang-native build plan without source-concatenating shell code.
- The original file is never executed by this example.

Run without arguments for a safe preview or pure in-process demonstration:

```sh
./target/release/pblang run examples/linux_compile_native/main.sapp --allow-all
```

Effectful examples remain dry-run until the exact `--apply --confirm=...` pair printed by the preview is supplied.

