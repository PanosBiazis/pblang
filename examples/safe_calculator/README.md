# Safe Calculator

This example is a pblang-native version of the local `PyCalculator` idea,
expanded after reviewing the calculator projects under `AAP/desktop-apps`
(Python/Tkinter, Python CLI, C#/Avalonia, Java/Swing, Delphi/Lazarus, and
Rust/egui/CLI).

It keeps the useful parts:

- calculator-style arithmetic
- a button/layout concept with memory, action, operator, function, and history controls
- calculation history
- keyboard and mouse input in the native GUI
- a Light/Dark theme toggle in the native GUI
- a PBIDE Designer component preset for visual insertion into a canvas

It improves the unsafe parts:

- no `eval`
- no hard-coded database credentials
- no subprocess launcher requirement
- deterministic parsing for `+`, `-`, `*`, `/`, `^`, `%`/`mod`, parentheses, decimals, unary signs, constants, and common functions
- domain checks for divide-by-zero, modulo-by-zero, square root/log inputs, inverse trig inputs, and factorial inputs
- portable in-memory history that can later be connected to PBIDE storage/database gates

Run:

```sh
./target/debug/pblang run examples/safe_calculator/main.sapp --allow-all
```

Run the native GUI window:

```sh
./target/debug/pblang run examples/safe_calculator/gui.sapp --allow-all
```

The GUI uses `stdlib/graphics.sapp` and the same shared calculator engine as
the CLI example. Click `Theme` to switch between light and dark palettes. It
intentionally keeps clipboard/database persistence as status-only or in-memory
behavior until PBIDE storage and clipboard gates are explicitly wired.

PBIDE integration:

- Examples picker: `Safe calculator` opens the compact runnable code example.
- Designer component library: search `calculator` and insert `Safe calculator`
  to place the same 860x820 native GUI form on the canvas: title/subtitle,
  display, status, memory, full keypad, scientific rows, Theme/History/Clear
  History/Close actions, and the right-side history panel. Then use the normal
  Designer code-generation path.
