# Physics Motion Lab

This PBLang-only example replaces a Tkinter, NumPy, and Matplotlib motion demo
with one bounded numerical backend and portable UI/visual models. It covers:

- ideal projectile motion without drag;
- sinusoidal simple harmonic motion;
- an explicitly limited small-angle pendulum approximation;
- uniform circular motion; and
- an ideal travelling sine-wave profile.

Every simulation accepts a shared `2..512` sample budget. The default suite
produces responsive Web, native desktop, and Android UI records from one view
tree, plus portable line-chart and timeline plans from the same numerical data.

Run the summary:

```text
pblang run examples/physics_motion_lab/main.sapp
```

The current visual engine output is a portable render plan. Actual animation
depends on a compatible host renderer and is not claimed by this headless test.
The models are educational idealizations, not engineering or safety simulators.
No network, subprocess, credential, personal path, or foreign runtime is used.
