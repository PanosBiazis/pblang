# Wayland Unicode Input

`stdlib:wayland_unicode_input.sapp` provides layout-independent text actuation
for Linux Wayland compositors that authorize
`zwp_virtual_keyboard_manager_v1`. The client, wire codec, Unix socket lifecycle,
XKB keymap generation and FD passing are implemented in PBLang. No `wtype`,
`ydotool`, C, Rust or shell helper participates in runtime actuation.

## Contract

- `wl_registry` discovery is bounded to 256 KiB and requires both `wl_seat` and
  `zwp_virtual_keyboard_manager_v1`.
- Text passes the shared 4,096-scalar input bound. NUL and unsupported control
  characters fail closed; Tab and Return remain semantic keysyms.
- Each XKB keymap contains at most 247 distinct Unicode keysyms. Larger texts
  are split into deterministic batches without changing scalar order.
- Keymaps use private close-on-exec `memfd` descriptors transferred with
  `SCM_RIGHTS`. Every allocation and descriptor has explicit cleanup.
- Socket connect, read, write, FD transfer and final compositor acknowledgement
  have deadlines. Partial writes, malformed frames and display errors fail.
- Actuation remains behind `PB_INPUT_CONTROL`, master-control approval,
  nonce-bound authorization, audit and the kill switch.

## Usage

```sapp
let Input = import("stdlib:input_control.sapp");
let pending = Input.type_text("Ελληνικά 中文 😀");
// The local operator approves `pending` through the normal control flow.
```

Inspect the deterministic plan and independently compile its generated XKB map:

```bash
./target/release/pblang examples/wayland_unicode_input.sapp --allow-all
./target/release/pblang tools/verify_wayland_keymap.sapp --allow-all
```

## Verification Boundary

Parser tests cover complete, empty, malformed and fragmented registry streams;
planner tests cover repeated scalars, multiple scripts, supplementary planes,
controls and multi-keymap batching. `/usr/bin/xkbcomp` independently accepts the
generated keymap. The current development host runs X11 and has no
`WAYLAND_DISPLAY`, so this is not live compositor actuation evidence. Runtime
selection remains fail closed until a real compositor advertises the privileged
manager, and production status still requires a live Wayland target run.
