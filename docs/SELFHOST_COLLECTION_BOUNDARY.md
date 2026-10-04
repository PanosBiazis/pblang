# Collection errors and exact numbers in the PBLang VM

The PBLang-maintained VM now closes an opted-in scalar ownership context when
an admitted collection call is guaranteed to fail its retained-host type check.
For example, a dynamic call to `map_remove(map{k:7}, 9)` previously threw while
leaving the private owners and borrows open. It now closes owned, unhandled
state before the **single original host primitive call**, which still supplies
the original error message. Argument expressions run once. No call is retried,
no substitute error string is constructed, and existing collection data is not
mutated by these rejected calls.

This is implemented in `stdlib/compiler/vm.sapp`, on both scalar dispatch and
generic dispatch. It covers invalid receivers for `len`, `array_push` / `push`,
`array_pop` / `pop`, `array_sort`, `map_keys`, `map_has`, and `map_remove`, plus
non-string keys for `map_has`, `map_remove`, and `has_member`. A non-map receiver
with a valid string name remains a legitimate `has_member(...)=false` result.
The `OP_LEN` instruction used by `for ... in ...` applies the same ownership
policy. Normal quota admission still takes precedence when a value already
exceeds the configured limit.

No general catch-and-rethrow mechanism has been added. The retained catch
expression does not expose the original host exception as a value. The new
checks use only known type contracts that reject before mutation or callbacks;
allocation failure, arbitrary host/FFI failures, overloaded guest operators,
and errors evaluating the argument expressions are outside this guarantee.
Stack traces may contain different VM source locations; message parity does not
mean identical stack traces.

## Ownership behavior

- `execution_context(true)` closes its private scalar state when a failure
  escapes with no remaining guest handler. Owners and borrows are reclaimed
  once; `close_execution` remains idempotent.
- An active guest catch keeps the state usable. A new error in the catch body
  closes it only if no outer guest handler remains.
- The default caller-owned `execution_context()` remains the caller's
  responsibility. Successful nil/false results never trigger cleanup.
- Contexts must remain private and synchronous. No resource handles, composite
  objects, cycles, host heap reset, or OS memory reclamation are added here.

Bounded `array_push` / `push` previously called `len` on invalid receivers before
reaching the array primitive. Those invalid calls now report the same array-type
error as unbounded/direct host calls, after eligible cleanup; this is an
intentional diagnostic correction. Capacity checks on valid arrays remain.

## Exact numeric values are not strings

The retained `is_string` predicate reports true for boxed numeric objects too.
The VM's previous size checks then invoked `len` on `bigint`, `rational`, or
`decimal` values, so bounded programs could fail even though their unbounded
versions succeeded. Size guards now distinguish actual strings using `type`
while retaining array/map length checks. This applies to stack ticks, local
storage, output, and returns. Map-growth admission also requires a real string
key, allowing invalid boxed numeric indices to reach the existing index-error
cleanup path rather than throwing inside its growth check.

The public `is_string` builtin is unchanged. This patch does not impose new
numeric precision rules or digit/bit-size limits: `max_value_items` measures
strings/arrays/maps, not the internal size of exact numbers. Arithmetic cost,
native allocation failure and process memory are not bounded by that item count.

## Verification and remaining retained boundary

`tests/semantics/selfhost_collection_cleanup_test.sapp` covers owned/borrowed
contexts, active and escaping handlers, argument evaluation, mutation canaries,
raw `OP_LEN`, full-map invalid keys, valid nil/false results, and exact-number
parity through plain, optimized and PBCS code.

`tests/tooling/selfhost_collection_parity.py` compares actual retained-host
messages/results with all six PBLang routes (plain/optimized/PBCS, each bounded
and unbounded). It separately records the still-unfixed direct nested `p:run`
result of 9 versus the explicit PBLang route's 7. It does not call that mismatch
a repaired host compiler. See [the retained boundary](COMPILER_DEFAULT_ARGUMENTS.md#exact-retained-nested-prun-boundary).

Run the example with `pblang run examples/selfhost_throw_cleanup/collection_types.sapp`.
The example confirms closure after an invalid map key, then executes exact
numbers successfully through the bounded PBLang VM. No Rust, global launcher,
implicit compiler reroute, hardware, or external service change is involved.
