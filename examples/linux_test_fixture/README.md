# linux_test_fixture

PBLang-only migration of `test/test.txt`.

- Original SHA-256: `f2ca1bb6c7e907d06dafe4687e579fce76b37e4e93b7605022da52e6ccc26fd2`
- Risk classification: `low`
- Replacement: PBLang fixture contract.
- The original file is never executed by this example.

Run without arguments for a safe preview or pure in-process demonstration:

```sh
./target/release/pblang run examples/linux_test_fixture/main.sapp --allow-all
```

Effectful examples remain dry-run until the exact `--apply --confirm=...` pair printed by the preview is supplied.

