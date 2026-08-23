# Language Fundamentals Workbook

This native PBLang example turns an early multi-topic C# console exercise into
a deterministic, testable workbook. It demonstrates:

- arithmetic with explicit division semantics;
- text slicing and Unicode codepoint round trips;
- all comparison branches;
- bounded nested and `while` loops;
- copied, validated attendance arrays;
- fully initialized numeric collections;
- a native PBLang class with a correct constructor; and
- explicit result maps instead of language-specific `out` parameters.

Run the default synthetic fixture:

```text
pblang run examples/language_fundamentals_workbook/main.sapp
```

Optional command-line arguments become normalized student labels. Inputs are
limited to 64 entries and 128 UTF-8 bytes per entry. The example performs no
filesystem, network, subprocess, or credential operations.

The source hashes in `workbook.sapp` identify the inspected C# source, project,
and solution snapshots without retaining workstation paths or personal data.
