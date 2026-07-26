# PBLang Android userspace emulator baseline - 2026-07-20

## Environment

- OS: Linux 7.0.0-28-generic x86-64
- CPU: Intel Core i7-1195G7, 8 logical CPUs
- PBLang: 0.5.7 release build
- Android profile: API 34, ARM64 smartphone

## Command

```sh
./target/release/pblang bench/android_emulator_stages.sapp
```

The benchmark runs nine measured samples per stage. It validates every result
before recording the duration and reports median, percentiles, dispersion and
outliers through `stdlib/benchmark.sapp`.

## Results

| Stage | Median | p90 | p95 | Range | CV |
|---|---:|---:|---:|---:|---:|
| Install plus create/start/resume/pause/stop/destroy | 0.240 ms | 0.261 ms | 0.286 ms | 0.224-0.312 ms | 10.50% |
| Bounded PBCS 1,000-iteration integer loop | 42.183 ms | 42.934 ms | 43.059 ms | 40.447-43.185 ms | 2.15% |

- Serialized PBCS artifact: 238 bytes.
- Expected loop result: 499,500, checked on every sample.
- Lifecycle stage stability: noisy, with one reported outlier.
- PBCS execution stability: excellent, with no reported outliers.

## Scope

This measures the PBLang Android userspace conformance model, not an Android OS
or hardware emulator. The model covers package isolation, application-private
storage, permissions, Activity lifecycle, explicit intents, protected services,
foreground input, TLS-only network plans and bounded PBCS execution. It does not
measure or claim Linux kernel, ART, Binder wire protocol, radio, GPU driver or
vendor-framework emulation.
