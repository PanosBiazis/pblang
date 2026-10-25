# macOS Unicode Input

`stdlib:macos_unicode_input.sapp` provides layout-independent text actuation on
macOS through CoreGraphics. It is implemented in PBLang and loads only the fixed
system ApplicationServices, CoreFoundation and libSystem paths. It invokes no
shell, AppleScript or host-language helper.

## Contract

- Text passes the shared 4,096-scalar bound and is lowered to at most 8,192
  UTF-16 units. Supplementary-plane scalars use checked surrogate pairs.
- Tab and Return use their native semantic key codes. NUL and other control
  scalars fail closed.
- `CGPreflightPostEventAccess` is required on current macOS. Older systems fall
  back to `AXIsProcessTrusted`; a missing or denied permission gate stops before
  event creation.
- Each down/up `CGEvent` and native UTF-16 buffer has explicit cleanup.
- Actuation remains behind `PB_INPUT_CONTROL`, master-control approval,
  nonce-bound authorization, audit and the kill switch.

## Usage

```sapp
let Input = import("stdlib:input_control.sapp");
let pending = Input.type_text("Ελληνικά 中文 😀");
// The local operator grants Accessibility permission and approves the request.
```

Linux-host tests verify scalar validation, exact UTF-16 lowering, semantic
Tab/Return mapping, capability selection and fail-closed non-macOS execution.
The current environment cannot execute ApplicationServices. A real permissioned
macOS target run remains mandatory before production verification.
