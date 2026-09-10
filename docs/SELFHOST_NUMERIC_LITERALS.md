# Self-hosted numeric literal diagnostics

The PBLang-owned tokenizer rejects empty radix literals (`0x`, `0b`, `0o`,
including uppercase prefixes and negative forms), contiguous invalid radix
suffixes (`0b102`, `0o8`, `0x1G`), and exponents without an ASCII digit after the
optional sign (`1e`, `1E+`, `1.e-`). Previously empty radix input became zero and
an incomplete exponent could reach code generation and throw a conversion error.

Malformed numbers produce one token with `kind="invalid"`, fixed
`text="invalid number literal"`, and the original physical line/column. The
malformed spelling is not copied into diagnostic text. An invalid radix's
contiguous identifier/digit suffix belongs to that token. Following operators,
comments, whitespace and newlines remain separate. Bounded tokenization counts
invalid tokens normally, including the reserved EOF slot; its `ok` still means
token allocation succeeded, not that source is syntactically valid.

Checked frontend parsing returns an empty AST and a fixed location diagnostic
before type checking or code generation. Raw parser users receive parser errors.
The guard preprocessor leaves lexically invalid source untouched, so rewriting
cannot erase the failure. Compile, file, batch, CLI serialization, lint and Run
routes inherit rejection. Existing unterminated string/comment recovery is
unchanged. Direct low-level AST/codegen calls still require validated inputs.

PBIDE Check reports the same error; syntax preview falls back to the escaped
original source. The mandatory security scan reports incomplete coverage and
blocks regardless of configurable severity or suppression settings. Taint
analysis rejects the parser error rather than analyzing a fabricated zero.

Valid radix values, decimal spelling, scientific notation, unary signs and
source positions remain unchanged. Numeric-looking strings/comments are inert.
Examples accepted by the self-hosted frontend include `0Xff`, `-0o17`, `1E+2`
and `-2.5e-1`.

## Compatibility and remaining scope

This intentionally tightens malformed-input acceptance in the self-hosted
frontend. The retained bootstrap is unchanged: it still accepts an empty radix
prefix in the exercised `let x=0x;` case and does not share the self-hosted
scientific-notation grammar. This is not a claim of complete frontend parity.
Radix overflow/rounding, general numeric suffix adjacency (`123name`, `1e2name`),
and broader backend compatibility remain separate work. No runtime resource or
isolation guarantee is added.

Regressions cover positive values, malformed tokens, fixed diagnostics and
Unicode positions; checked and direct parser consumers; raw legacy-guard input;
file/CLI/batch paths; PBIDE Check/preview; mandatory security and taint rejection.
The malformed file fixture is data read by the test, never executed by the host.
