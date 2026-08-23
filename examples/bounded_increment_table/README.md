# Bounded Increment Table

A native PBLang migration of an earlier C variable-length table exercise. The
program creates an exact decimal sequence from an initial value, an increment,
and a bounded number of repetitions.

## Improvements

- `capacity` is a real 1..4,096 allocation contract;
- `times + 1` must fit before any values are generated;
- every returned element is initialized and within bounds;
- exact arbitrary-precision base-10 addition through PBLang `decimal`;
- strict decimal strings prevent pre-existing floating-point precision loss;
- malformed, exponent, fractional-count, and oversized inputs fail explicitly;
- structured capacity and correctness metadata; and
- no C compiler, Windows executable, object file, SFML dependency, or foreign fallback.

The original C source indexed `num[size]`, one element beyond the allocated
variable-length array, then printed uninitialized entries and ignored input
failures. Those undefined behaviors are corrected rather than reproduced. The
machine-specific Dev-C++ build configuration is provenance material only and
is not copied into this example.

## Run

```sh
pblang run examples/bounded_increment_table/main.sapp -- 5 1.5 0.25 4
```

## Verify

```sh
pblang run examples/bounded_increment_table/test.sapp --allow-all
pblang lint examples/bounded_increment_table/table.sapp
```
