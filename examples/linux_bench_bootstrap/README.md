# linux_bench_bootstrap

PBLang-only migration of `bench_bootstrap.sh`.

- Original SHA-256: `b6f02baf1c1adf132a6cae5850f416a082812e244671c519373e697c7c7c56d1`
- Risk classification: `medium`
- Replacement: bounded PBLang benchmark; no Cargo bootstrap or temporary shell driver.
- The original file is never executed by this example.

Run without arguments for a safe preview or pure in-process demonstration:

```sh
./target/release/pblang run examples/linux_bench_bootstrap/main.sapp --allow-all
```

Effectful examples remain dry-run until the exact `--apply --confirm=...` pair printed by the preview is supplied.

