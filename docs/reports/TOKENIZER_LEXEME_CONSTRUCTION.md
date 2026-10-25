# Token text construction

The PBLang tokenizer already holds a scalar-character array. Identifiers,
non-radix numeric lexemes and line/block comments now construct their text once
from the consumed array span using slice + str_join, rather than repeatedly
growing a string character by character. Single-scalar identifiers/numbers reuse
the existing character directly. This avoids source-prefix rescans and removes
repeated text growth for these token classes, not for string literals.

Numeric spelling preserves unary minus, leading zeroes, dot, e/E and exponent
sign exactly, including lexically retained malformed suffixes. Radix literals
still follow their previous numeric normalization. Identifiers retain Unicode
scalar spans. Comments exclude delimiters, retain CR where previously retained,
and preserve physical line/column and unterminated flags. Strings and escape
decoding are unchanged. Source/token admission and EOF accounting are unchanged.

Independent golden tests cover subtraction/ranges, numeric spelling/radix,
Unicode identifiers, empty/multiline/unterminated comments, long synthetic text
and rejection before text construction. The self-hosted bootstrap probe checks
joined comment/number/identifier values and scalar offsets inside the PBLang VM.
Twenty-two targeted suites passed. No full suite or native executable refresh.

Final build, portable-lock publication, touched-file lint, privacy scan and diff
check passed. Lint retained nonfinite-comparison/style warnings plus local
binding/shadow warnings in the benchmark and numeric branch. Independent
defensive review found no blocking defect. Graphify stopped at its 20-second
deadline without completion. No owned test/benchmark/build/Graphify workers or
temporary reference copies remained; unrelated dirty work was preserved.

This is a focused long-token optimization, not a proven end-to-end speedup.
Two final alternating reference/candidate runs improve long token medians but
retain a small positive paired taint overhead; that regression concern is OPEN.
See BENCHMARK_SESSION_2026-09-08_TOKENIZER_LEXEMES.md for all intermediate and final
samples, including unfavorable measurements. Do not present this as resolving
the previously observed 5.97-to-7.22-ms analyzer regression.

A multi-scalar token now temporarily owns an array slice plus its output string.
Peak allocation was not measured and is not claimed lower. Initial str_split,
string-literal concatenation, legacy callers without admission limits and VM-wide
hard heap/time enforcement remain unresolved resource boundaries.
