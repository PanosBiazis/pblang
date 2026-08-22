# Numerical Series Lab

A native PBLang migration of three small Python exercises: decimal digits of
pi, decimal digits of e, and exact sum of integer squares.

## Improvements

- no Python, `mpmath`, package installation, or foreign-source fallback;
- bounded support for 1 to 1,000 significant decimal digits;
- exact `bigint` fixed-point arithmetic with 12 internal guard digits;
- Machin's identity for pi and a factorial series for e;
- exact sums for signed decimal integers beyond the usual safe-number range;
- strict ASCII decimal parsing and bounded list/integer sizes;
- structured output with method, iteration, and precision metadata; and
- no implicit file overwrite or persistence of generated values.

The arithmetic operations are exact, but the implementation deliberately does
not claim a formal proof of correctly rounded transcendental output. Prefix and
requested-length regression tests cover the supported implementation. Values
can be redirected to a file explicitly when persistence is actually desired.

## Run

From the PBLang repository root:

```sh
pblang run examples/numerical_series_lab/main.sapp -- pi 100
pblang run examples/numerical_series_lab/main.sapp -- e 100
pblang run examples/numerical_series_lab/main.sapp -- squares "3,-4,12"
```

## Verify

```sh
pblang run examples/numerical_series_lab/test.sapp --allow-all
pblang lint examples/numerical_series_lab/series.sapp
```
