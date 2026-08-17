# linux_chperm

PBLang-only migration of `chperm.sh`.

- Original SHA-256: `32faafa74f135576b0f37df3946ffacf6de9a5135839d03c822b4e5d09bf8652`
- Risk classification: `high`
- Replacement: non-recursive allowlisted chmod with exact confirmation.
- The original file is never executed by this example.

Run without arguments for a safe preview or pure in-process demonstration:

```sh
./target/release/pblang run examples/linux_chperm/main.sapp --allow-all
```

Effectful examples remain dry-run until the exact `--apply --confirm=...` pair printed by the preview is supplied.

