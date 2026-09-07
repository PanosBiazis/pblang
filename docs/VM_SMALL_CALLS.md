# PBLang VM small-call admission and dispatch

The PBLang-written bytecode interpreter in `stdlib/compiler/vm.sapp` now uses
fixed-shape argument arrays for pure closures with zero through three arguments.
Two-argument calls already had a specialized path. Larger arities and raw function
constants retain the generic path; builtin dispatch remains distinct.

## Correctness and admission

Raw OP_CALL operands must be numbers, nonnegative integers and leave a callee
beneath all supplied arguments. Checks happen before indexing/popping/argument
allocation. The existing bounded max-stack violation still trips the sticky
runtime state before structural validation. Errors do not include raw operands.
Serialized bytecode already has a validator; these checks also protect callers
of the raw-chunk VM API.

A descriptor containing both __bi and __fn always takes builtin precedence,
matching the generic path. Previously a two-argument import dual descriptor
could fall into the closure path. This is a deliberate correction for ambiguous
raw descriptors, not a change to normal compiler-emitted calls.

Argument evaluation order, min/max arity checks, defaults, captured upvalues,
lexical globals, non-overwriting import-cache propagation and runtime/depth
sharing are preserved. The small path skips generic fill/reverse-pop loops,
but still allocates arrays and does not establish a measured general speedup.

## Verified scope

Nine targeted suites passed on the final implementation:
selfhost_vm_small_calls, selfhost_vm_call_admission,
selfhost_vm_two_arg_closure, selfhost_vm_bounded, selfhost_vm_builtin_registry,
selfhost_vm_try_arity, selfhost_closure, selfhost_try_capture, selfhost_super_owner.

The new tests cover arities 0–4, defaults, captures/mutation, evaluation order,
exceptions/nil, builtin shadowing, serialized roundtrip, optimized frontend,
exact step boundaries and sticky frame rejection. Raw tests add malformed
counts, stack underflow, arity failure before body effects, ambiguous descriptors,
raw-function fallback, lexical globals and cache propagation/nonoverwrite.

Defensive-security analysis and final static review found no blocking issue in
this change. This is not proof of a secure VM. Closures remain mutable descriptor
maps; malformed __fn/__upvalues/__globals, direct nonfinite-operand coverage and
exact peak-counter parity need further testing. Hard aggregate heap/wall-clock
limits, typed FFI/raw bypass closure and hostile-code isolation are not supplied
by this optimization.

The retained Rust CLI interpreter, PBLang tree interpreter and other backends
were not modified. No opcode numbers or serialized format changed. Broad
backward compatibility and physical-platform qualification remain unverified.
See `BENCHMARK_SESSION_2026-09-08_VM_CALLS.md` for all timing evidence and limits.
