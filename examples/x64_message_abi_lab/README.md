# x64 Message ABI Lab

A PBLang-only migration of four small x86-64 message-output exercises. Source
identity is retained as SHA-256 digests; original message text, comments,
personal paths and foreign source are not copied into this package.

The lab generates two real artifacts directly from PBLang:

- static Linux x86-64 ELF using `write` and `exit` syscalls with an RX segment;
- Windows PE32+ using bounded imports for `GetStdHandle`, `WriteFile` and
  `ExitProcess`, 32-byte shadow space, aligned stack allocation, NX metadata and
  separate executable/data sections.

Three historical source variants mixed or omitted Windows ABI requirements.
The migration records normalized behavior descriptors and reports the wrong
argument register, missing shadow space and missing stack-alignment evidence.
It does not retain or execute the original assembly text.

## Run

```text
cd examples/x64_message_abi_lab
pblang run .
pblang run test.sapp --allow-all
pblang run benchmark.sapp
```

Linux artifact execution is already covered by the shared x64 backend's native
CPU test. This package verifies Windows PE structure and ABI bytes on Linux but
does not claim live Windows execution. The generated PE deliberately does not
claim ASLR because this bounded builder emits no relocation section.
