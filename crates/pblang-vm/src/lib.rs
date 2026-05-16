pub mod chunk;
pub mod vm;
pub mod memory_safety;
pub mod concurrency;

pub use pblang_ir::{
    BuiltinFn, Chunk, Closure, FrameTrace, Function, Module, Obj, ObjRef, RuntimeError, Upvalue,
    Value, VmHost,
};
pub use vm::{FfiPolicy, StdlibConfig, VmLimits, VM};
pub use memory_safety::{MemoryTracker, SafeValue, MemorySafetyStats, MemorySafetyError, BorrowId, BorrowType, LifetimeId};
pub use concurrency::{
    SharedValue, SafeMutex, SafeRwLock, SafeChannel, ThreadPool, Atomic, Barrier, Once, Timer,
    ConcurrencyStats, VmHandleRegistry, VmChannelBuf,
};
