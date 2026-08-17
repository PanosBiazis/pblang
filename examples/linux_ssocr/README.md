# linux_ssocr

PBLang-only migration of `ssocr.sh`.

- Original SHA-256: `2c6731c154e63d75aa5b98d33cd7459dc385d90f2c462b53b43b80987958e8c5`
- Risk classification: `high`
- Replacement: PBLang OCR adapter without shell sourcing.
- The original file is never executed by this example.

Run without arguments for a safe preview or pure in-process demonstration:

```sh
./target/release/pblang run examples/linux_ssocr/main.sapp --allow-all
```

Effectful examples remain dry-run until the exact `--apply --confirm=...` pair printed by the preview is supplied.

