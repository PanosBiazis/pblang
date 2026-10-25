# PBLang-owned frontend/build continuation — 2026-09-08

Baseline `9274342`. Three reproduced defects are corrected without editing
Rust, the existing dirty frontend/semantic modules, or device-facing linux_dirfs.

## Contextual match names

The PBLang parser accepted `let match = ...` but treated `match.admitted`
as a match expression. It now recognizes unambiguous `.`, `?.` and `?[`
postfixes as access on the contextual identifier. Existing member/index lowering
and runtime behavior are reused; no new AST node or bytecode instruction.

Tests cover assignment, nested access, optional access/index, keyword members,
comments between name and postfix, parameters, calls named match, and genuine
match expressions (including a subject read through match.value). Checked
compilation, plain/optimized PBLang VM execution and PBCS roundtrips agree.
Malformed postfixes, missing bindings and unterminated input still reject.
The whole current linux_dirfs source parses successfully; it is read as a
trusted repository fixture, not imported or executed against devices.

Parenthesized match subjects remain an explicit unsupported ambiguity in this
parser. This patch does not claim complete contextual-name/operator parity,
general pattern coverage, or native backend compatibility.

## Physical source positions

Physical LF inside a string now advances the tokenizer's source line and resets
its column, both raw and immediately following a backslash. Decoded newline
escapes do not fabricate source lines. String values, scalar offsets and
unterminated-token flags are preserved. Tests cover LF, CRLF, escaped physical
newlines, decoded escapes, Unicode, repeated lines, following identifiers and
comments, and EOF. This also repairs security directive attribution after
multiline strings.

## Default dynamic-eval rule

The exact default rule ID/needle pair now checks an eval identifier followed by
a significant opening parenthesis, reusing the mandatory tokenizer pass. It
ignores comment/string/suffix-name examples but still flags bare, member,
optional-member and whitespace/comment-separated calls, once per callee line.
This is lexical call-shape detection, not proof that a symbol evaluates code;
an eval declaration can still be conservatively reported. Aliased, computed and
parenthesized callees are not exhaustively resolved by this rule.

Different custom rule IDs/needles retain literal matching. The exact default
ID/needle pair inherits token matching even when its severity/message is
overridden. Configured disable and severity thresholds remain supported.
Suppression scope/reason/audit rules and immutable runtime checks are unchanged.
Tokenizer failure or unterminated input remains mandatory-blocking; malformed
syntax completeness still requires the separate parser/dependency analysis.

Focused tests verify split calls, one finding per line, physical line positions,
suppression before/after multiline strings, no retroactive suppression,
malformed source, custom policies, raw builtins and payload privacy.
Independent defensive review found no concrete regression in the three diffs.

## Executable refresh is still incomplete

A PBLang pbbuild direct-x64 attempt targeting tools/pbselfc.sapp ran with cache
and fallback disabled under a 40-second deadline, 2-GiB address-space limit and
64-MiB file limit. It timed out (124) without a final report or candidate artifact.
No complete refreshed dependency-analysis result or backend progress is inferred
from silence. The empty private staging directory was removed; the active host
executable's digest, byte size and modification timestamp remained unchanged.
No Rust build, bootstrap snapshot overwrite or native probe substitution occurred.

The specific linux_dirfs parse failure and comment-based eval false positive
from the preceding attempt are addressed at their owning implementation, not
suppressed. Full build completion is not established. The previously reviewed
module-cache control-only taint classification and conditional non-Linux cache
confinement/cleanup boundary remain unresolved. Full standalone CLI/runtime
parity, reproducible self-bootstrap and release qualification remain open.

Sixteen targeted suites passed; no full suite ran. The parser control and nine
numeric comparison rows are recorded with raw samples and limitations in
`BENCHMARK_SESSION_2026-09-08_SELFHOST_LEXICAL.md`. No interpreter superiority,
cross-device qualification or 1.0.0 readiness is claimed.

Final project build and PBLang-native repository-relative lock publication
passed. Touched-file lint completed with warnings (existing parser style/
shadowing and conservative unused-variable diagnostics included), not a
warning-free claim. Added-file/added-line privacy and diff checks passed.
Graphify stopped at its 20-second deadline (124); graph freshness is not claimed.
No owned test, benchmark, native-build or Graphify workers remain. Unrelated
dirty work and the locally refreshed lock are excluded from this change.
