# MIPS Assembly Workbench

A PBLang-only migration of three small MIPS assembly exercises. The original
files are represented by content digests; comments and raw source files are not
copied into this package.

The workbench parses a bounded MIPS32 subset, encodes real 32-bit instruction
words and executes them through the shared PBLang emulator. It supports
`ADD`, `ADDU`, `ADDI`, `ADDIU`, `SW`, `NOP` and `SYSCALL`, named or numeric
registers, decimal/hex immediates, signed overflow detection, aligned bounded
memory and explicit snippet fallthrough.

Legacy two-operand `addi` syntax is disabled by default. Enabling
`legacy_shorthand` normalizes it and emits one structured warning per affected
instruction. Diagnostics contain line numbers and safe opcode metadata but do
not echo source lines.

## Run

```text
cd examples/mips_assembly_workbench
pblang run .
pblang run test.sapp --allow-all
pblang run benchmark.sapp
```

Run these commands from the package directory. The retained bootstrap currently
resolves a portable local-dependency path from the process working directory
instead of consistently from the package lock directory. Running the same file
by a repository-root path therefore fails closed; no absolute machine path is
stored as a workaround.

Labels, branches in text form, directives, macros, relocations, object formats,
dynamic linking and complete MIPS operating-system ABIs remain unsupported.
The shared UI compiles to Web, native and Android records; passing headless
tests are not evidence of packaged physical-device renderers.
