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
accepts any ordinary numeric width at the primitive boundary. Negative and NaN
widths retain their host behavior on unbounded execution; the bounded VM keeps
its existing allocation policy, including rejection of negative widths. Its
ordered size comparison can also reject a NaN width; that existing bounded
limitation is not corrected here.

`execution_context(true)` closes only with no active guest handler. Caught
failures keep scalar handles usable, and nested catches remove their own handler
before executing the catch body. A default caller-owned context remains the
caller's closing responsibility. Normal results do not trigger this cleanup.
Both generic and scalar dispatch use the same guards.

Existing allocation guards remain in place for valid inputs. They are not a
complete allocation proof: the existing replacement estimate undercounts empty
search strings, which insert at both end boundaries as well as between
characters. That adjacent quota defect is outside this cleanup change. Numeric
infinity can also exceed what an unbounded host allocation can safely perform;
only bounded rejection is tested for infinite padding widths.

This change covers neither general host failures, native crashes, arbitrary
FFI/resource cleanup, composite/cyclic ownership, nor process RSS reclamation.
It changes the explicit PBLang VM, not the retained compiler's nested `p:run`
behavior or the global launcher. No alternate PBCK route is substituted.

Verification lives in `tests/semantics/selfhost_string_admission_cleanup_test.sapp`
and `tests/tooling/selfhost_string_admission_parity.py`. The latter compares
actual error messages and accepted results through plain, optimized and PBCS
chunks, bounded and unbounded, and injects a secondary cleanup failure.
Run the example with `pblang run examples/selfhost_throw_cleanup/string_admission.sapp`.
