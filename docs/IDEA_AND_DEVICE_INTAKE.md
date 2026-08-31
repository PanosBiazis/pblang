# Idea and device evidence intake

Metadata scans are discovery inputs, not automatic training corpora or project
creation authority. `stdlib/knowledge_intake_contract.sapp` validates bounded,
length-framed claim metadata with explicit ownership, license, consent and
redaction state. Caller claims remain independently unverified, so the result is
eligible for review but never authorizes routing, content ingestion, training,
publication or repository creation.

`stdlib/device_evidence_contract.sapp` is the identifier-minimized,
existing-module-first foundation
for the proposed device laboratory. Evidence names a device class and exact test
artifact but intentionally has no serial number, IP address, account, path or
remote-control capability. Physical, attended, consented and unexpired evidence
becomes only schema-eligible for PBLang, PBRemote or PBSpotIt review. A verifier
identity/signature and trusted clock are still required before evidence is usable;
the contract cannot authorize an effect.

The `pbknowledge` and `pbdevice-lab` repository candidates therefore remain
deferred. A separate private repository requires a stable interface, independent
lifecycle, owned code/tests, provenance review and explicit approval.

`stdlib/project_inventory.sapp` pages large metadata scans through internal,
process-local state. Open/resume/close and cursor formats are deliberately not
exported, so callers cannot copy or replay a continuation across VM workers. A
PBLang in-flight guard serializes the entire collector; pending queues have
per-session and aggregate ceilings, and consumed queue prefixes are compacted.
Collected reports still contain caller-requested raw paths and are explicitly
`safe_to_log=false`; neither cursor nor internal continuation/queue path state is
returned or persisted, although the requested final report still contains raw paths.
