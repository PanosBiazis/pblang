// Fuzz target: `pblang_core::parse_strict` (the batch-compile parser
// used by `pblang run`, `pblang compile`, `pblang lint`). Same panic-
// hardening guarantee as the `parse` target — for any input, either we
// get an AST or a `Vec<ParseError>`; never a `panic!` /
// arithmetic overflow / `unwrap` failure.
//
// The `parse` target exercises the recovery path; this one drives the
// strict path. Both layers share most of the chumsky grammar but
// diverge on the error-reporting tail, so it's worth fuzzing both.

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let _ = pblang_core::parse_strict(s);
    }
});
