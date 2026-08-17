# linux_test_runner

PBLang-only migration of `t`.

- Original SHA-256: `9098bc8678192625011ff3cf81d9278f5d9f4223551ac55a61c32737902b9eca`
- Risk classification: `high`
- Replacement: bounded package-test plan without wildcard deletion or allow-all.
- The original file is never executed by this example.

Run without arguments for a safe preview or pure in-process demonstration:

```sh
./target/release/pblang run examples/linux_test_runner/main.sapp --allow-all
```

Effectful examples remain dry-run until the exact `--apply --confirm=...` pair printed by the preview is supplied.

