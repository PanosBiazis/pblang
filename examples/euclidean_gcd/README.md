# Euclidean GCD

This native PBLang example evolves a small interactive algorithm exercise into
a reusable, validated Euclidean greatest-common-divisor implementation.

The earlier exercise returned during its first loop iteration, so many inputs
produced only one division step instead of the final GCD. This edition keeps the
complete trace and cross-checks the answer against `stdlib:algo.sapp`.

## Run

From the PBLang repository:

```sh
./target/release/pblang run examples/euclidean_gcd/main.sapp -- 1071 462
```

Expected result:

```text
Euclidean algorithm
  1071 = 462 * 2 + 147
  462 = 147 * 3 + 21
  147 = 21 * 7 + 0
GCD(1071, 462) = 21
Standard library cross-check: passed
```

## Design

- `gcd.sapp` contains the reusable parser, complete trace, and formatting API.
- `main.sapp` is a small command-line adapter.
- `test.sapp` covers normal, negative, zero, malformed, fractional, oversized,
  and exact-integer-boundary inputs.

Inputs are limited to PBLang's exact numeric integer range. For larger integer
workloads, use `stdlib:bignum.sapp` rather than silently accepting rounded
floating-point values.
