# linux_fedora_ssocr

PBLang-only migration of `fedora/ssocr.sh`.

- Original SHA-256: `68b134e20a7134d18ef4a01b3a02aa8b5a81d72ae678577de8896859616c5025`
- Risk classification: `high`
- Replacement: PBLang OCR adapter without virtualenv shell sourcing.
- The original file is never executed by this example.

Run without arguments for a safe preview or pure in-process demonstration:

```sh
./target/release/pblang run examples/linux_fedora_ssocr/main.sapp --allow-all
```

Effectful examples remain dry-run until the exact `--apply --confirm=...` pair printed by the preview is supplied.

