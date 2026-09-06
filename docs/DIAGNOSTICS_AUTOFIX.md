# Scope-aware diagnostic suggestions and conservative autofix

`stdlib:diagnostics.sapp` and its `watch` facade keep the existing public result
shapes. Suggestions now use names visible when each unresolved reference was
visited by the semantic walker, plus builtins. They no longer gather variables
from every child scope. Repeated unresolved uses of one name share a suggestion
only from the intersection of their visible names; ambiguous cases receive none.
Later declarations are not retroactively made visible by mutable scope maps.
Top-level function hoisting retains the existing semantic contract.

```pblang
let D=import("stdlib:diagnostics.sapp");
let result=D.autofix("let value=1; print valeu; print \"valeu\";");
// result.fixed: let value=1; print value; print "valeu";
```

Autofix replaces only explicit unresolved identifier-expression spans. Comments,
strings, property names, map keys and bound same-spelling identifiers are not
edited. Spans are half-open Unicode scalar offsets, not UTF-8 bytes or display
columns, so multibyte text, CRLF and multiline strings do not shift edits.
All edits refer to the original source and are assembled in sorted source order;
renames do not cascade into later replacements.

The edit operation accepts at most 65,536 source scalars and 256 recorded free
reference visits/errors. It declines the entire change for malformed source,
missing/synthetic spans (including shorthand map values), overlapping edits,
unfixable errors, exceeded bounds or a failed final diagnostic recheck. A decline
returns the original `{fixed:source, applied:[], count:0}`. `count` still counts
identifier-name corrections, not individual occurrences. Warnings alone are not
rewritten. This intentionally replaces the old unsafe partial-edit behavior.

Tokenizer span metadata is opt-in through `tokenize(source,true)`; default token
and AST shapes are unchanged. `semantic.analyze(ast,true)` captures detached
use-site name snapshots and bounded reference metadata; its default schema is
unchanged. A snapshot examines at most 4,096 scope symbol entries; if incomplete,
local candidates are withheld. These local limits are not a VM heap/time sandbox,
nor a bound on all existing parser/diagnostic work.

The semantic walker is not a complete proof of runtime behavior. Unsupported
grammar/binding forms are not newly enabled, and missing metadata causes edits
to be withheld. A successful static recheck does not execute or prove equivalent
behavior; callers should show a diff and let developers review intended changes.

`diagnostics_autofix_scope_test` reproduces the original content/scope defects
and covers exact output, bound names, fields, Unicode, repeated corrections,
malformed input, limits, schema compatibility, idempotence and the watch facade.

Validation on 2026-09-06: nine focused tests passed in the working checkout.
Seven relevant tests also passed against an isolated export of the exact staged
source without unrelated uncommitted changes: autofix/scope, diagnostics, watch
diagnostics, Unicode tokenizer, unterminated tokenizer, kernel pragmas and
parameter-default parity. The isolated build and five source checks passed.
Native lint reported style/unused-binding warnings, not zero-warning cleanliness.
No full suite, physical-platform proof or performance improvement is claimed.
