// Fuzz target: `pblang_core::parse` (the recovery-style parser used by
// the LSP / REPL / analyze paths). Goal: prove that no input string —
// random bytes, malformed UTF-8 turned valid, deeply nested constructs,
// huge token streams — can panic the parser. Either we get an AST or a
// `Vec<ParseError>`; we never get a `panic!` / `unwrap` failure /
// arithmetic overflow.
//
// libFuzzer hands us a `&[u8]`. We filter to valid UTF-8 (the parser's
// public surface only accepts `&str`) and feed it in. Inputs that
// already round-trip through the parser become future corpus seeds.

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        // We don't care about the AST or the error vec — only that the
        // call returns without panicking. Drop the result so libFuzzer
        // can't observe any side channel.
        let _ = pblang_core::parse(s);
    }
});
