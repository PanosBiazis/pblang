// Fuzz target: `pblang_pm::parse_manifest`. The manifest is the first
// piece of user-controlled text every package-manager command reads
// (`pblang build`, `pblang install`, `pblang publish`, …), so a panic in
// the TOML decoder or in the serde-derived Manifest deserializer would
// be felt by every workflow that touches a `pblang.toml` file. Goal:
// prove no input — malformed TOML, valid TOML with the wrong shape,
// pathological strings, deeply nested tables — can panic.
//
// `parse_manifest` takes `&str`, so we filter the libFuzzer byte stream
// to valid UTF-8 first. Anything we get back is `Ok(Manifest)` or
// `Err(String)`; only a panic / arithmetic overflow / `unwrap` failure
// is a fuzzing finding.

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let _ = pblang_pm::parse_manifest(s);
    }
});
