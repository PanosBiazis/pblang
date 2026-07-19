# Hardware-aware routing

This example dry-runs deterministic CPU and GPU decisions without requiring a
GPU or registering a fake execution runtime.

```bash
./target/release/pblang examples/hardware_routing/main.sapp
```

Expected shape:

```text
cheap=cpu/sequential workers=1
heavy=cpu/parallel workers=8
tensor=gpu/accelerator work=8192 required=2049
```

`work_units_per_item` and transfer/overhead options are relative routing hints.
Set them from representative local benchmarks. They do not measure hardware and
do not prove that two device kernels are semantically equivalent.

