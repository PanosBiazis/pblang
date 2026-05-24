// Re-export the bytecode chunk type from pblang-ir so VM consumers can
// `use pblang_vm::chunk::Chunk` without taking a direct pblang-ir dep.
pub use pblang_ir::chunk::*;
