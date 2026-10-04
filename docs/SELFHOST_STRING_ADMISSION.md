# String allocation admission and exceptional scalar cleanup

The PBLang VM checks the known rejecting argument shapes of `str_repeat`,
`str_replace_all`, `str_pad_left` and `str_pad_right` before calculating their
allocation estimates. Previously `str_repeat(nil, 2)` raised an incidental
`len()` error and left an opted-in scalar ownership context open. Invalid
replacement arguments could fail inside `len()` as well. Invalid padding
arguments escaped through the primitive, also leaving that state open.

For these predictable rejections the VM now skips allocation arithmetic,
closes eligible scalar state, and invokes the original primitive **once**.
That primitive supplies the first error message. Argument expressions still
run exactly once in language order. No guest callback or operation is replayed.
A cleanup error is suppressed to preserve the primitive diagnostic; reclamation
is incomplete if cleanup itself fails.

Only actual strings pass string admission; boxed bigint/rational/decimal values
are not strings even when the retained `is_string` predicate says otherwise.
Repeat accepts an ordinary number greater than or equal to zero: fractional
counts preserve host truncation, while NaN and negative counts reject. Padding
accepts any ordinary numeric width at the primitive boundary. NaN widths preserve
the host conversion to zero on both bounded and unbounded execution and return the
source unchanged, keeping successful scalar ownership live. The bounded VM
checks that source length without an ordered NaN comparison. Negative widths
retain their host behavior unbounded and the prior allocation rejection bounded.

`execution_context(true)` closes only with no active guest handler. Caught
failures keep scalar handles usable, and nested catches remove their own handler
before executing the catch body. A default caller-owned context remains the
caller's closing responsibility. Normal results do not trigger this cleanup.
Both generic and scalar dispatch use the same guards.

Empty-search replacement admission uses `n + (n + 1) * r`, where `n` and `r`
are source and replacement lengths in Unicode scalars. This includes both ends,
one insertion for an empty source, and combining characters as separate scalars.
The check precedes the allocating primitive even when the result is discarded
or the raw bytecode ends at its call. Exact fitting results pass; over-limit
requests trip the sticky budget, close opted-in ownership even inside guest
catches, and skip catch-body effects. Borrowed contexts remain caller-owned. A
later independent run can reuse the context with a fresh runtime budget.
Nonempty-search replacement retains the conservative existing estimate. These
are scalar item limits, not byte or process-memory limits, and are not a complete
allocation proof. Numeric infinity can exceed safe unbounded host allocation;
only bounded rejection is tested for infinite padding widths.

This change covers neither general host failures, native crashes, arbitrary
FFI/resource cleanup, composite/cyclic ownership, nor process RSS reclamation.
It changes the explicit PBLang VM, not the retained compiler's nested `p:run`
behavior or the global launcher. No alternate PBCK route is substituted.

Verification lives in `tests/semantics/selfhost_string_admission_cleanup_test.sapp`,
`tests/semantics/selfhost_string_bounds_test.sapp`
and `tests/tooling/selfhost_string_admission_parity.py`. The latter compares
actual error messages and accepted results through plain, optimized and PBCS
chunks, bounded and unbounded, and injects a secondary cleanup failure.
Run the example with `pblang run examples/selfhost_throw_cleanup/string_admission.sapp`.
