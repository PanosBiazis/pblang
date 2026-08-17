# linux_language_basics

PBLang-only migration of `linuxscript.sh`.

- Original SHA-256: `19c392b8ba138824ee6dab8228f90270050a1ea1e1368c684da094775d804a7c`
- Risk classification: `low`
- Replacement: pure PBLang language-basics tutorial without terminal clearing.
- The original file is never executed by this example.

Run without arguments for a safe preview or pure in-process demonstration:

```sh
./target/release/pblang run examples/linux_language_basics/main.sapp --allow-all
```

Effectful examples remain dry-run until the exact `--apply --confirm=...` pair printed by the preview is supplied.

