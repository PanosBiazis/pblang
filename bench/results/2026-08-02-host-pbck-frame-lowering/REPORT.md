# Canonical host-PBCK frame lowering benchmark

Date: 2026-08-02

## Scope

This measures the PBLang-only canonical lowering and independent validation of
one verified exact-i32 call graph. The fixture has three simultaneously active
functions, ten instructions, two captures and one local-to-nonlocal shared
cell. It returns the exact oracle value `73`.

## Reproduction

```sh
./target/release/pblang bench/host_pbck_frame_lowering_stages.sapp
```

The benchmark runs nine samples. Each build sample creates and validates 100
images; each validation sample validates one frozen image 1,000 times. Every
iteration contributes the deterministic image checksum to a correctness gate.

## Results

| Stage | Work/sample | Median | p95 | Median/item |
|---|---:|---:|---:|---:|
| Reverify + build + validate | 100 images | 46.580 ms | 47.064 ms | 0.466 ms |
| Independent validate | 1,000 images | 179.205 ms | 182.911 ms | 0.179 ms |

## Verification

- Focused lowering semantics: passed.
- Runnable example: passed with value `42`.
- Complete PBLang semantics: 748/748 passed.
- PBRemote release suite after integration: 63/63 passed.

## Boundary

The image removes recursive maps and stale verifier summaries from platform
backend inputs. It does not execute arbitrary Android PBCK by itself. Android
whole-program dispatch and remaining host capability bindings remain separate
work.
