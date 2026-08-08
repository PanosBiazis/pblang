# Windows Unicode Input Backend Benchmark

Date: 2026-08-07

This slice measures PBLang-native planning and PE32+ artifact generation for a
37-scalar Unicode sample. It does not claim live Windows input latency because
the current host is Linux and no Windows target was available.

## Results

| Stage | Samples | Median | p95 |
| --- | ---: | ---: | ---: |
| Unicode/UTF-16 event planning | 25 | 0.0603 ms | 0.0688 ms |
| Batched executable generation | 25 | 4.9761 ms | 5.0464 ms |

The batched artifact was 4,608 bytes. Relative to the initial per-event-call
implementation measured in the same session, generation improved from 5.7095
ms to 4.9761 ms median (12.84%) and artifact size fell from 5,632 to 4,608 bytes
(18.18%). The generated program uses one `SendInput` call and requires the
returned count to match every requested event.

## Verification

- `windows_pe_test.sapp`: PE sections, imports, W^X/NX metadata, bounds and
  deterministic layout passed.
- `windows_unicode_input_test.sapp`: scalar validation, surrogate lowering,
  batching, NUL rejection and non-Windows fail-closed behavior passed.
- The generated sample was identified as PE32+ x86-64 and imported only
  `USER32.dll!SendInput` and `KERNEL32.dll!ExitProcess`.
- The complete PBLang semantic suite passed 755/755 serially.

Live target actuation, UIPI behavior under different integrity levels and
Windows accessibility verification remain separate release gates.
