//! CLI helper modules carved out of `src/main.rs`. These modules are pure
//! code-motion extractions — same behavior, just located in focused files so
//! `main.rs` stops being a 6000-line monolith.

pub(crate) mod call_graph;
pub(crate) mod dataflow;
pub(crate) mod diag;
pub(crate) mod fmt;
