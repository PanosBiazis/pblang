# Windows Unicode Input

`stdlib:windows_unicode_input.sapp` provides layout-independent text actuation for
x86-64 Windows. It is implemented in PBLang and uses the operating-system
`SendInput` boundary. It does not invoke C, Rust, PowerShell, a shell, or a
persistent helper.

## Contract

- Text is validated through `stdlib:input_events.sapp`.
- Unicode scalars are lowered to UTF-16. Supplementary-plane scalars become a
  checked high/low surrogate pair.
- Tab and newline use `VK_TAB` and `VK_RETURN`; other text uses
  `KEYEVENTF_UNICODE`.
- All key-down/up records are submitted in one bounded `INPUT[]` call. The
  returned count must equal the complete array length. Partial injection, UIPI
  rejection, process failure, and unavailable platforms fail closed.
- Execution remains behind `PB_INPUT_CONTROL`, master-control approval,
  nonce-bound authorization, auditing, and the kill switch.

The reusable `stdlib:windows_pe.sapp` builder emits PE32+ executables with
explicit multi-DLL imports, an RX `.text` section, an RW `.idata` section, and
NX compatibility. It deliberately does not claim ASLR because it has no base
relocation section yet.

## Usage

```sapp
let Input = import("stdlib:input_control.sapp");
let pending = Input.type_text("Ελληνικά 中文 😀");
// The local operator approves `pending` through the normal master-control flow.
```

To inspect an artifact without actuating input:

```bash
PB_WINDOWS_INPUT_ARTIFACT=/tmp/pblang-input.exe \
  ./target/release/pblang examples/windows_unicode_input/main.sapp
objdump -x /tmp/pblang-input.exe
```

## Verification Boundary

Linux verification covers parsing, UTF-16 lowering, exact PE/import structure,
W^X section flags, NX metadata, deterministic failure paths, and backend
selection. Actual input injection requires an x86-64 Windows session and a
foreground target at an equal or lower integrity level. Windows UIPI may block
injection and does not provide detailed error reporting; PBRemote treats a zero
or partial `SendInput` result as failure.
