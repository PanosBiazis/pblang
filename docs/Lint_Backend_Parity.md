# Smart diagnostics and retained native lint

PBLang currently has distinct diagnostic routes; they are not interchangeable.

| Entry point | Analyzer | Relevant contract |
| --- | --- | --- |
| `pblang check file.sapp` | PBLang `check_cli.sapp` and `diagnostics.diagnose_checked` | Parse, names, types, unused bindings; errors fail, warnings alone do not |
| `pblang lint file.sapp` / `clippy` | Retained native CLI lint | Separate traversal and style checks; known unary-`not` reference omission |
| `pblang --check --via-pblang file.sapp` | Source analysis via the bundle guard | Different admission/check contract; not smart lint parity |

The retained native unused-binding walker in `src/cli/lint.rs` does not visit
the operand of `ExprKind::Not`. For example, it reports `value` as unused in:

```sapp
fn test() {
    let value=map{ok:true};
    if not value.ok {return false;}
    return true;
}
print test();
```

The binding is used. Do not remove the binding, its initializer, or the check
based on that warning. Initializers can perform necessary work, and checking a
returned status can be essential. Renaming the variable to suppress a warning
does not repair the analyzer.

The PBLang semantic walker visits unary operands and the smart diagnostics
route recognizes this use. The focused regression suite exercises direct,
member, indexed, logical, closure and catch reads; genuinely unused and
assignment-only bindings; shadowing; comments and strings; exports; and
undefined names under `not`:

```sh
pblang run tests/semantics/diagnostics_unary_use_test.sapp --allow-all
```

It tests both `diagnose` and `diagnose_checked`. It does not fix or certify the
native `lint` route, nor establish complete syntax or diagnostic parity. Unary
minus is a separate control and must not be assumed to have the same defect.
The native omission remains open under the PBLang-only implementation policy.
Installation of a new retained native analyzer or migration of CLI routing
requires separate evidence; changing source documentation is not installation.

Smart diagnostics parse the supplied text without executing the analyzed
program. Source admission limits and parser CPU/memory limits are separate
contracts. See [bounded smart checking](Bounded_Smart_Check.md) for the explicit
Linux source-byte admission profile and its limitations.
