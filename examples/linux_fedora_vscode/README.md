# linux_fedora_vscode

PBLang-only migration of `fedora/vscode.sh`.

- Original SHA-256: `dc247850272565a514d1524bcbe7e193936d02cfe03657fc071068f903e7c021`
- Risk classification: `high`
- Replacement: validated Flatpak launch without eval, sudo, or no-sandbox.
- The original file is never executed by this example.

Run without arguments for a safe preview or pure in-process demonstration:

```sh
./target/release/pblang run examples/linux_fedora_vscode/main.sapp --allow-all
```

Effectful examples remain dry-run until the exact `--apply --confirm=...` pair printed by the preview is supplied.

