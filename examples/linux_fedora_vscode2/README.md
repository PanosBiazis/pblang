# linux_fedora_vscode2

PBLang-only migration of `fedora/vscode2.sh`.

- Original SHA-256: `9346ff666d8cfb652a87c5750f1de5b47aa6390b366f1c4733a610851f5f2263`
- Risk classification: `high`
- Replacement: validated Flatpak launch without privilege escalation.
- The original file is never executed by this example.

Run without arguments for a safe preview or pure in-process demonstration:

```sh
./target/release/pblang run examples/linux_fedora_vscode2/main.sapp --allow-all
```

Effectful examples remain dry-run until the exact `--apply --confirm=...` pair printed by the preview is supplied.

