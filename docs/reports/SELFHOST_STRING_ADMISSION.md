# String allocation admission and exceptional scalar cleanup

The PBLang VM checks the known rejecting argument shapes of `str_repeat`,
`str_replace_all`, `str_pad_left`, `str_pad_right` and `str_join` before
calculating their allocation estimates. Previously `str_repeat(nil, 2)` raised an incidental
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

Join admission counts `max(0, n - 1) * len(separator)` plus the Unicode
scalar lengths of actual string members before calling the retained primitive.
Non-string members remain empty fields, including boxed numbers, arrays, maps,
and cycles; the guard neither stringifies them nor invokes guest callbacks.
Empty arrays produce an empty string and separators still appear between empty
fields. The array count must fit `max_value_items`, and the admission scan charges
one shared VM step per member. This extra work can therefore exhaust a tight
step budget even when the output fits. Exact item and step boundaries pass.
A terminal raw call cannot escape output admission, and a quota failure remains
sticky across guest catches. Invalid array/separator types receive the original
primitive error after eligible cleanup; handled and borrowed state stays live.
Unbounded valid calls skip this size scan. This does not bound all allocations
inside the retained host, nor does it add admission for other string primitives.

The initial admission change measured a +50.8% VM median cost on a local
200-join workload. A follow-up removes redundant generic helper calls for lengths
already known to be nonnegative numbers and caches the private run limit. It
still checks after each string member and charges all member-inspection steps;
no input validation, early rejection, or allocation quota was weakened. Zero/one-member
arrays still validate the separator type but need no separator-length arithmetic.

The unchanged workload joins 64 two-scalar Unicode strings with commas 200 times
(checksum 38200). With one warmup and nine alternating samples per revision,
a three-way same-process comparison of the identical compiled chunk measured:

| VM revision | Bounded median / p95 (ms) | Unbounded median / p95 (ms) |
| --- | --- | --- |
| Original without join admission (`a486aa6`) | 24.372 / 24.683 | 10.422 / 11.723 |
| Guarded baseline (`d5e0cff`, same guard as `6010a8d`) | 40.815 / 42.825 | 10.729 / 11.432 |
| Optimized admission | 32.714 / 35.633 | 10.839 / 11.875 |

This is a 19.8% reduction against the guarded baseline in that comparison,
with 8.342 ms of extra bounded cost still remaining against the original
unguarded reference (+34.2%). The bounded step count remains exactly 18658;
the original reference charged 5858 because it did not perform join admission.
Separate-process samples measured 36.783 -> 29.022 ms bounded median, and a
separate two-module same-process comparison measured 33.547 -> 27.241 ms.
Small unbounded differences vary between runs; they establish neither a general
improvement nor a resolved regression. All datasets, including the first variant
and its worse unbounded sample, remain in the
[cost evidence](evidence/selfhost-join-cost-2026-10-04.json). Process wall time,
CPU and RSS include startup. Nine local samples and nearest-rank p95 do not
establish general performance or other-platform behavior, and this does not
remove the cost of bounded admission.

This change covers neither general host failures, native crashes, arbitrary
FFI/resource cleanup, composite/cyclic ownership, nor process RSS reclamation.
It changes the explicit PBLang VM, not the retained compiler's nested `p:run`
behavior or the global launcher. No alternate PBCK route is substituted.

Verification lives in `tests/semantics/selfhost_string_admission_cleanup_test.sapp`,
`tests/semantics/selfhost_string_bounds_test.sapp`,
`tests/semantics/selfhost_join_bounds_test.sapp`,
`tests/tooling/selfhost_join_admission_parity.py`
and `tests/tooling/selfhost_string_admission_parity.py`. The parity harnesses compare
actual error messages and accepted results through plain, optimized and PBCS
chunks, bounded and unbounded, and inject a secondary cleanup failure.
Run the example with `pblang run examples/selfhost_throw_cleanup/string_admission.sapp`.
