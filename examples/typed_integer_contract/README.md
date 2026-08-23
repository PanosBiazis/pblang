# Typed Integer Contract

A native PBLang migration of the two small `RustPython` exercises. The first
source exposed Rust `i32` addition to Python as a decimal string through PyO3;
the second returned the sum of two Rust `u64` values.

## Improvements

- complete `i32` and `u64` range checks with explicit overflow failures;
- exact `bigint` arithmetic for the entire unsigned 64-bit domain;
- decimal-string transport above the IEEE-754 safe-integer boundary;
- a versioned, inert request/response map suitable for serialization;
- bounded ASCII decimal parsing without exponent, radix, or Unicode ambiguity;
- structured fail-closed responses for malformed external requests; and
- no Rust, Python, native extension, compiler, network, or foreign runtime
  dependency.

The language-neutral map contract is not advertised as a Python extension or a
live FFI bridge. A future adapter can serialize the request and response without
changing arithmetic behavior, but adapter execution remains outside this
example.

## Source baseline

The dependency-free Rust crate passed its original single `2 + 2 = 4` test in
an isolated temporary build. The PyO3 crate could not be rebuilt offline because
PyO3 was not available in the local Cargo cache, and its source contained no
test for `sum_as_string`. The PBLang migration therefore records that limitation
instead of claiming reproduced Python-extension execution.

The four source/manifests are represented by a deterministic aggregate SHA-256
in `integer_contract.sapp`; source paths and developer identity are not retained.

## Run

From the PBLang repository root:

```sh
pblang run examples/typed_integer_contract/main.sapp -- i32 -12 7
pblang run examples/typed_integer_contract/main.sapp -- u64 18446744073709551615 0
```

## Verify

```sh
pblang run examples/typed_integer_contract/test.sapp --allow-all
pblang lint examples/typed_integer_contract/integer_contract.sapp
```
