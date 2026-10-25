# Optional release channels

PBLang keeps the numeric package version separate from an optional release
channel. Existing manifests and builds are unchanged when these fields are
absent.

An application's exact Semantic Version and channel are also independent from
the PBLang toolchain version. An application at `7.3.1` with a `stable` channel
may be built with PBLang `0.6.0` on its `alpha` channel; neither value is copied
into the other. If build
provenance is recorded, use a separate field such as
`built_with_pblang = "0.6.0"` rather than changing the application
version, channel, license, author, or maturity metadata.

```toml
[package]
name = "my-app"
version = "1.4.0"
entry = "main.sapp"
release_channel = "beta.2"
show_release_channel = true
release_channel_in_artifact = true
```

This configuration keeps the exact package identity `1.4.0`, gives the app the
presentation label `1.4.0 [beta.2]`, and lets distribution builders use the
explicit suffix `--channel-beta.2`. Setting either boolean to
`false`, or omitting it, keeps that surface unchanged.

`version` accepts canonical Semantic Versioning 2.0.0, including prerelease and
build metadata such as `0.5.0-alpha.1+build.001`. The exact string is the
registry, lock, signature and provenance identity. A channel never changes it:
`0.5.0-alpha [nightly]` is package version `0.5.0-alpha` on the independent
`nightly` distribution track. SemVer precedence comparisons are not artifact
identity comparisons because build metadata intentionally does not affect precedence.

Common channels include `stable`, `release`, `lts`, `alpha`, `alpha-beta`,
`alphabeta`, `beta`, `rc`, `preview`, `debug`, `nightly`, `dev`, `canary`,
and `edge`. A developer may also use a custom channel. Channel identifiers are
bounded to 48 ASCII letters, digits, dots, and hyphens so they remain safe in
display badges and explicit artifact suffixes.

PBLang-native Android builders accept the same three options. The numeric
Android `versionCode` remains independent and must continue to increase for
store upgrades. Release-channel resolution happens only during build and adds
no runtime state or branch to the generated application.

## Validation and current boundary

The PBLang-native manifest validator accepts only canonical SemVer 2.0.0,
rejects duplicate package/dependency keys and sections, reads identity only from
the real `[package]` section, and bounds manifest bytes, lines and line length.
Package names use a path- and JSON-safe identifier grammar. Run
`pblang manifest-check` and `pblang manifest-parity` before creating artifacts.

This is not yet a complete supply-chain guarantee. The retained Rust package
publisher and untrusted registry-index ingestion still need the same always-on
strict name/version validator, structured JSON serialization, exact
`{name, version, channel, digest, publisher}` binding and build-metadata
ambiguity rejection. Until those boundaries are migrated and tested, passing
the PBLang-native validator is necessary but not sufficient for production
publication.

`examples/manifest_validation_benchmark/main.sapp` provides a fixed 20-dependency
workload with five warmups, nine raw samples, median/p95 and correctness checks.
