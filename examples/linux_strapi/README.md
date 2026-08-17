# linux_strapi

PBLang-only migration of `strapi.sh`.

- Original SHA-256: `c74eb58ecfdf723719615726a66af7afe9b704c75667ed56b4c601f6e3dafc15`
- Risk classification: `medium`
- Replacement: loopback development-server plan.
- The original file is never executed by this example.

Run without arguments for a safe preview or pure in-process demonstration:

```sh
./target/release/pblang run examples/linux_strapi/main.sapp --allow-all
```

Effectful examples remain dry-run until the exact `--apply --confirm=...` pair printed by the preview is supplied.

