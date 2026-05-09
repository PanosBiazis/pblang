// DEAD CODE REMOVED — Fix #10 from audit.
//
// This file previously contained a logos-based Token enum that was never used.
// The actual parser (parser.rs) uses chumsky character-level combinators directly.
// The logos crate dependency has also been removed from Cargo.toml (#25).
//
// This file is retained only to avoid breaking `pub mod lexer` callers.
// It will be fully removed in the next cleanup pass.
