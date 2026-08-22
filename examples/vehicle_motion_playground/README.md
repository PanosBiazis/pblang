# Vehicle Motion Playground

This PBLang-only example consolidates an earlier 2D Go car exercise and its
unfinished 3D counterpart. Source identity is retained only through SHA-256:

- 2D: `390a410f2bb237cfb11d06c073159dff3a35caa1e40826ff2c68aa26748e495d`
- 3D: `781fe94c04a952e6e4cab53488c9f4c9fac17a54895b4ed37131ad91c2e24bf1`

## Improvements

- fixed-step, frame-rate-independent vehicle motion with bounded catch-up;
- acceleration, reverse, steering, drag, braking and world collisions;
- immutable state transitions suitable for replay and tests;
- procedural 2D car and complete 3D body/cabin/wheel geometry;
- no missing image, Go runtime, OpenGL dependency or remote asset;
- one responsive, accessible control surface for Web, native and Android;
- explicit 2D/3D renderer plans that share one simulation state.

Run from the PBLang repository root:

```text
pblang run examples/vehicle_motion_playground/main.sapp
pblang run examples/vehicle_motion_playground/test.sapp
pblang run examples/vehicle_motion_playground/benchmark.sapp
```

## Honest boundary

The PBLang core fully executes and tests the vehicle dynamics and produces
backend-neutral `pblang.visual.plan.v1` geometry. A host renderer must still
rasterize that plan and connect continuous keyboard, pointer or touch events to
the normalized controls. Headless compilation is not reported as physical GPU,
browser-canvas or Android-device rendering evidence.
