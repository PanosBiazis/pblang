# linux_refresh_context

PBLang-only migration of `refresh-perplexity-context.ps1`.

- Original SHA-256: `839c0b2048b9e0942e1e18bff06637149671e749ed4f0cca58f5bf9b5a082316`
- Risk classification: `medium`
- Replacement: descriptor-oriented refresh plan with explicit clipboard opt-in.
- The original file is never executed by this example.

Run without arguments for a safe preview or pure in-process demonstration:

```sh
./target/release/pblang run examples/linux_refresh_context/main.sapp --allow-all
```

Effectful examples remain dry-run until the exact `--apply --confirm=...` pair printed by the preview is supplied.

