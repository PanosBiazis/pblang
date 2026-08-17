# linux_fedora_screenshot_ocr

PBLang-only migration of `fedora/screenshot_ocr.py`.

- Original SHA-256: `74eb9010fec952dc31789adf66c5a1a89a31fb3f8e9cbadc489ebd75f798ef58`
- Risk classification: `high`
- Replacement: per-run consent, bounded screenshot bytes, no persisted capture.
- The original file is never executed by this example.

Run without arguments for a safe preview or pure in-process demonstration:

```sh
./target/release/pblang run examples/linux_fedora_screenshot_ocr/main.sapp --allow-all
```

Effectful examples remain dry-run until the exact `--apply --confirm=...` pair printed by the preview is supplied.

