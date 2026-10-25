# Master-control audit privacy profile

Set `PB_MASTER_CONTROL_AUDIT=events` in the trusted local process environment to
retain event names and timestamps while omitting all caller-provided details
from both `audit_log()` and the appended audit file. This covers requested and
executed actions as well as policy refusals, which may otherwise include text,
paths or commands. The PB-MCP desktop launcher selects this profile explicitly.

An absent/empty value or `detailed` preserves the existing detailed behavior.
Unknown nonempty values fail during module initialization, before authority is
granted. The profile does not change policy, unlock credentials, approval IDs,
expiry, execution results or cleanup. It is not a memory-erasure guarantee and
does not authenticate the resulting audit file or make it tamper-evident.

Run `tests/semantics/master_control_audit_test.sapp` in the PBLang repository.
It starts isolated child processes with synthetic credentials and checks both
memory and disk records under events/detailed/empty profiles, unknown-profile
rejection, preserved results, policy refusals and credential omission. The
master-control lifecycle, policy/security/hardening and input-control suites
also pass with the variable absent. No real input backend is used by these tests.
