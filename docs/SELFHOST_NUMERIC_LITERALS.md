# Self-hosted numeric literal diagnostics

The PBLang-owned tokenizer rejects empty radix literals (`0x`, `0b`, `0o`,
including uppercase prefixes and negative forms), contiguous invalid radix
suffixes (`0b102`, `0o8`, `0x1G`), and exponents without an ASCII digit after the
optional sign (`1e`, `1E+`, `1.e-`). Decimal and exponent literals also reject
contiguous word/identifier suffixes such as `123name`, `1.2name`, `1e2name`,
`1e2e3`, underscores and Unicode identifier continuations. Previously empty radix input became zero and
an incomplete exponent could reach code generation and throw a conversion error.

Malformed numbers produce one token with `kind="invalid"`, fixed
`text="invalid number literal"`, and the original physical line/column. The
malformed spelling is not copied into diagnostic text. A malformed number's
contiguous identifier/digit suffix belongs to that token. Following operators,
comments, whitespace and newlines remain separate. Bounded tokenization counts
invalid tokens normally, including the reserved EOF slot; its `ok` still means
token allocation succeeded, not that source is syntactically valid.

Checked frontend parsing returns an empty AST and a fixed location diagnostic
before type checking or code generation. The raw parser also returns zero
statements and one fixed diagnostic for the first invalid numeric token, before
filtering comments or constructing any AST. This applies even inside defaults,
type annotations, names, and otherwise valid surrounding statements.
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
Numbers must be separated from following words, including word operators:
write `1 and true`, not `1and true`. This deliberately tightens older acceptance
of adjacent word tokens (the retained bootstrap still accepts some such forms).
Arithmetic operators, range punctuation, comments and whitespace remain valid
boundaries; identifiers such as `name1` are unchanged. Unsupported digit
separators/suffixes are rejected, not interpreted as another statement.

Radix overflow/rounding and broader backend compatibility remain separate work.
Only invalid numeric tokens get the raw parser's empty-AST guarantee here;
ordinary syntax errors and unterminated-token recovery remain unchanged. Direct
WASM and ARM64 source backends now additionally use
[checked source admission](BACKEND_SOURCE_ADMISSION.md) to reject ordinary
syntax errors and unterminated tokens before generation. Other direct parser
consumers still need separate review. No runtime resource or isolation guarantee
is added.

Regressions cover positive values, malformed tokens, fixed diagnostics and
Unicode positions; checked and direct parser consumers; raw legacy-guard input;
file/CLI/batch paths; PBIDE Check/preview; mandatory security and taint rejection.
The malformed file fixture is data read by the test, never executed by the host.

## Direct backend regression

The former raw parser returned recovered statements despite numeric errors.
With that parser, both direct backends emitted bytes for an invalid default
(`fn f(x=0x){return 1;} return f(2);`), a variable type (`let x:0x=1;return x;`)
and a return type (`fn main():0x{return 7;}`). The backends ignored those parts
of the AST. The numeric preflight now returns no statements, so both WASM and
ARM64 reject these sources before emitting bytes through their existing
missing-entry checks. Tests also retain positive compilation controls and check
that a rejected parse does not affect the next valid parse.

Validation for the boundary/preflight change passed 43 focused suites, including
17 emulator suites, direct ARM64 execution in the test emulator, and generated
WASM execution in Node/V8. The V8 suite required a 32-GiB virtual-address limit:
its initial 2-GiB test limit rejected a WASM memory reservation. The same suite
and expectations then passed unchanged. This is not guest-OS or physical-device
validation. Touched-file checking passed; lint completed with existing style
and conservative unused-value warnings.
