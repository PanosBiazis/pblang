# Secure Configuration Inspector

This PBLang-only example migrates an earlier Python environment-loading script
into a bounded configuration validation workflow. The original script loaded a
`.env` file from the current directory and printed configuration values,
including a database URL. The migration does not retain automatic file access,
host-environment access or raw sensitive-value output.

```text
pblang run examples/secure_configuration_inspector/main.sapp
pblang run examples/secure_configuration_inspector/benchmark.sapp
```

`stdlib:config.sapp` now provides an additive `parse_dotenv_strict` API. It
checks aggregate, line, key and value bounds; rejects malformed keys, duplicate
assignments, unterminated quotes and control bytes; and returns structured
diagnostics without external effects. The legacy permissive parser remains
available for backward compatibility.

`redacted_values` classifies password, token, API-key, connection, session,
credential and key-bearing names. Sensitive values are replaced with a fixed
marker before the result reaches JSON, diagnostics or the shared UI. The
example validates required fields and basic bool, integer, service-URL and
HTTPS-URL types. One declarative UI tree compiles to compact Web, native desktop
and Android records.

The example validates supplied text only. It does not discover `.env` files,
read the process environment, test database connectivity, rotate credentials,
or prove that a value is authentic. Applications that intentionally access
host configuration still require the environment/filesystem capabilities and
must keep raw secrets outside logs and UI state.

The original source is represented only by SHA-256
`da69a2e4d2ea60dc03ea001f9b971db1ac8116f25db432b948fae3e18d65fbd9`.
