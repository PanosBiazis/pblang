# Safe serialization

Run:

```bash
./target/release/pblang examples/safe_serialization/main.sapp
```

The example parses a bounded YAML configuration, emits bounded pretty JSON,
and converts a strict TOML-subset document to YAML. Production callers should
use `parse_safe`, `stringify_safe`, and `convert_safe` for untrusted data.
