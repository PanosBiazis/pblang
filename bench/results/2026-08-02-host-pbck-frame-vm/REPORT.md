# Host PBCK Frame VM Benchmark

Date: 2026-08-02

## Method

`bench/host_pbck_frame_vm_stages.sapp` executes a correctness-gated four-step
program that calls one injected adapter and returns 42. Each stage uses 9
samples of 500 runs. The report records median and worst observed sample as p95
for this small fixed sample set. All paths use the same PBLang release binary
and VM limits.

## Results

| Stage | Median total | p95 total | Median/run |
|---|---:|---:|---:|
| Prepared PBFP bridge | 72.118 ms | 75.788 ms | 0.144235 ms |
| Direct recursive VM | 62.996 ms | 68.684 ms | 0.125993 ms |
| Decode + prepare + run | 372.952 ms | 387.621 ms | 0.745903 ms |

The prepared bridge is 14.48% slower than direct recursive execution on this
tiny workload. Decode and preparation are 5.92x the direct execution latency,
so frame decoding belongs at a load boundary, not in the request hot path.

## Large-artifact gate

The PBRemote contract processes the real release through inspect, recursive
decode, frame-plan construction, PBFP encode/decode, full frame reconstruction,
retained-probe selection and generated-runtime descriptor checks. After linear
slot indexes, compact reconstructed chunks and direct validated-array reads, it
measured 48.44 s and 444,808 KiB peak RSS. The first complete implementation
measured 51.44 s and 556,468 KiB, so peak memory fell 20.07% and elapsed time
fell 5.83%. The dedicated PBRemote analyzer measured 42.31 s and 429,800 KiB.

These are build/verification measurements, not application startup figures.
The remaining memory cost is dominated by simultaneously retained recursive,
flat, encoded and reconstructed representations of the 147,696-instruction
artifact. Future work should stream validation or release intermediate forms
between stages before using this gate in constrained build environments.
