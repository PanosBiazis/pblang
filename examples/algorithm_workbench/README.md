# Algorithm Workbench

A native PBLang migration of the earlier C sorting, searching, and Fibonacci
exercise. It preserves the six advertised operations as three meaningful
families while reusing the existing PBLang standard library instead of creating
duplicate merge-sort and binary-search implementations.

## Improvements

- non-mutating bubble sort with comparison, swap, and pass metrics;
- early termination for already sorted bubble-sort input;
- cross-checking between bubble sort and both standard-library merge sorts;
- explicit sorted-input enforcement before binary search;
- cross-checking between both standard-library binary searches;
- iterative and memoized exact Fibonacci sequences verified against `bignum`;
- deterministic decimal-string Fibonacci output without C integer overflow;
- strict ASCII integer parsing and bounded input/work sizes; and
- structured JSON reports suitable for automated tests and later UIs.

The original C program accepted unchecked counts, allocated from signed user
input, ignored allocation and `scanf` failures, overflowed Fibonacci `int`
values, and used exponential recursion. Those behaviors are corrected rather
than copied. No C source, executable, password file, or machine path is retained.

## Run

From the PBLang repository root:

```sh
pblang run examples/algorithm_workbench/main.sapp -- sort "5,1,4,2,8"
pblang run examples/algorithm_workbench/main.sapp -- search "1,3,5,7,9" 7
pblang run examples/algorithm_workbench/main.sapp -- fib 20
```

## Verify

```sh
pblang run examples/algorithm_workbench/test.sapp --allow-all
pblang lint examples/algorithm_workbench/workbench.sapp
```
