// Fuzz target: `pblang_pm::parse_registry_index`. This decoder consumes
// JSON fetched over HTTP from a package registry (the `index_url` end
// of `pblang install`) — i.e. attacker-controlled bytes if the registry
// host is malicious or the connection is MITM'd before TLS terminates.
// The decoder is already size-capped (`MAX_REGISTRY_INDEX_BYTES`), but
// malformed *inner* JSON should still degrade to an `Err(String)`, not
// a panic in `serde_json` or in the post-deserialization validation.
//
// The function takes `&[u8]` directly, so we feed libFuzzer's bytes in
// without UTF-8 filtering — that mirrors what the HTTP path actually
// hands the decoder.

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = pblang_pm::parse_registry_index(data);
});
