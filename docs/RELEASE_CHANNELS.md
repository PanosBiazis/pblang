# Optional release channels

PBLang keeps the numeric package version separate from an optional release
channel. Existing manifests and builds are unchanged when these fields are
absent.

An application's version and channel are also independent from the PBLang
toolchain version. A `7.3.1-stable` application may be built with
`pblang 0.6.0-alpha`; neither value is copied into the other. If build
provenance is recorded, use a separate field such as
`built_with_pblang = "0.6.0-alpha"` rather than changing the application
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

This configuration gives the app the display version `1.4.0-beta.2` and lets
distribution builders use the suffix `-beta.2`. Setting either boolean to
`false`, or omitting it, keeps that surface unchanged.

Common channels include `stable`, `release`, `lts`, `alpha`, `alpha-beta`,
`alphabeta`, `beta`, `rc`, `preview`, `debug`, `nightly`, `dev`, `canary`,
and `edge`. A developer may also use a custom channel. Channel identifiers are
bounded to 48 ASCII letters, digits, dots, and hyphens so they remain safe in
version labels and artifact filenames.

PBLang-native Android builders accept the same three options. The numeric
Android `versionCode` remains independent and must continue to increase for
store upgrades. Release-channel resolution happens only during build and adds
no runtime state or branch to the generated application.
