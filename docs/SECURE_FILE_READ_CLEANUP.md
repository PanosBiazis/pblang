# Bounded read parent cleanup

`secure_file.read_bytes` and its `read_text` caller attempt to close their acquired
parent exactly once after a normal backend return or catchable backend exception.
Only boolean `true` confirms close. False, nil, other types and catchable close
exceptions produce a fixed failure map without bytes/text. No retry occurs.
Successful reads and backend error maps are unchanged when close is confirmed.
Backend exceptions now become `ok:false` with a fixed error rather than escaping.

This contract starts after `_open_parent` returns an owned successful handle.
It does not cover partial acquisition, VM termination, process death, native crashes,
or arbitrary cleanup inside the backend. An attempted failed close is not evidence
of physical release. Prefix/optional reads and other facade operations have separate
ownership paths and are not changed by this fix.

The semantic matrix substitutes a counted, resource-free backend into a complete
copy of the facade: 36 read/close combinations plus four admission cases. The tooling
runner uses the existing bounded process/isolation harness. NetVis adds independent
real-descriptor probes and installed-product acceptance. Neither modeled faults nor
Linux acceptance establishes Windows/macOS native behavior.
