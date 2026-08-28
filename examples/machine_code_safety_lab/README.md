# Machine Code Safety Lab

A PBLang-only migration of six historical Windows batch exercises that created
or executed raw machine-code bytes. Source identity is retained only through
SHA-256 digests; original paths, comments and machine-specific information are
not copied into the package.

The original exercises intended to return `42`, return `100`, or calculate
`5 + 5`. Their execution mechanisms included writable-executable memory,
PowerShell reflection, temporary executables, malformed hexadecimal data,
an x86 PE container carrying x86-64 instructions, a mismatched DEBUG byte count
and a DOS `INT 20h` sequence that could not report the claimed exit code.

This edition preserves the intended computations through validated PBLang IR:

- bounded portable evaluation supplies the reference result;
- native PBLang backends generate DOS 8086 COM, Linux x86-64 ELF and Windows
  x86-64 PE artifacts;
- the historical recipes are reviewed as data and are never executed;
- unsupported programs and targets fail closed;
- one accessible UI tree compiles for Web, native desktop and Android records.

## Run

```text
cd examples/machine_code_safety_lab
pblang run .
pblang run test.sapp --allow-all
pblang run benchmark.sapp
```

Linux x86-64 execution is covered by the shared backend tests. DOS is verified
by the bounded 8086 execution model in `codegen_dos_test.sapp`. Windows PE
format/import structure is verified on this Linux host, but live Windows
execution is still a target-host gate. The package does not expose arbitrary
machine-code execution or claim that policy findings determine malicious intent.
