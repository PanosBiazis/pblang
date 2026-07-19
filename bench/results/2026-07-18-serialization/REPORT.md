# Serialization pipeline benchmark

Date: 2026-07-18  
Host: Linux 6.17.0-40-generic x86_64, Intel Core i7-1195G7, 8 logical CPUs,
15,151 MiB RAM  
Runtime: `pblang 0.5.7`, release binary SHA-256
`9723af14255a627e3bfa05fd2d496831246909c2e781a55c9f11f322d9c34ca5`

## Method

`bench/serialization_pipeline_stages.sapp` was run in seven fresh processes.
Every stage validates a checksum so dead-code or failed parses cannot produce a
passing sample. Times are stage totals from `now_ms`; p95 is the nearest-rank
maximum for seven samples. `/usr/bin/time` records whole-process elapsed time
and peak RSS.

The pre-change baseline was captured with the same release runtime and the same
five legacy stages before editing `serialization.sapp`. Safe stages did not
exist in the baseline. The final process performs four additional safe stages,
so whole-process elapsed time is not compared to the shorter baseline process.

## Legacy-path results

| Phase | Rounds/process | Baseline median ms | Final median ms | Final p95 ms | Median delta |
|---|---:|---:|---:|---:|---:|
| JSON parse/dispatch | 4,000 | 9.171 | 10.155 | 10.969 | +10.73% |
| TOML parse | 2,000 | 164.913 | 177.059 | 180.090 | +7.37% |
| YAML parse | 2,000 | 209.665 | 216.567 | 221.378 | +3.29% |
| TOML stringify | 2,000 | 54.276 | 39.791 | 40.568 | **-26.69%** |
| YAML stringify | 2,000 | 59.178 | 44.246 | 44.864 | **-25.23%** |

The parse cost buys correct backslash/quote/newline round trips and canonical
format normalization. Fast exits for strings without escapes reduced the first
hardening draft substantially. Stringify improved because common strings avoid
character-by-character escaping while sorted map keys retain deterministic
output.

## Bounded safe-path results

| Phase | Rounds/process | Median stage ms | p95 stage ms | Median microseconds/op |
|---|---:|---:|---:|---:|
| Safe JSON parse | 1,000 | 101.587 | 102.675 | 101.587 |
| Safe TOML parse | 500 | 218.788 | 222.064 | 437.576 |
| Safe YAML parse | 500 | 223.196 | 225.079 | 446.393 |
| Safe JSON to YAML | 500 | 91.434 | 94.227 | 182.867 |

Final whole-process median/p95 elapsed time was 1.27/1.27 seconds; median/p95
peak RSS was 20,256/20,412 KiB.

Safe and legacy paths are not equivalent competitors. The safe path resolves
bounded policy, checks input/line/depth/item limits, validates strict subset
syntax and duplicates, parses, validates the resulting value shape, and emits a
versioned result. Trusted callers can retain the faster legacy parser under its
absolute hard ceilings; trust-boundary callers should pay the measured safe
validation cost.

## Reproduce

```bash
for i in 1 2 3 4 5 6 7; do
  /usr/bin/time -f 'PROCESS|%e|%M' \
    ./target/release/pblang bench/serialization_pipeline_stages.sapp
done
```

These are local microbenchmark results for the disclosed payloads, not claims
about all serialization workloads or other languages.
