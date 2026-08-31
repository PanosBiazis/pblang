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
The aggregate portfolio entry point rebuilds a path-free result with canonical
project names and reports `contains_raw_paths=false`. Counts and category presence
can still fingerprint a private workspace, so it deliberately remains
`safe_to_log=false`; `terminal_text_safe=true` means only that its fixed text is
control-sanitized, not that it is approved for publication.

Inventory coverage is now separated from idea admission. Both portfolio entry
points suppress derived tasks and proposals unless the scan is complete,
non-truncated, allocation-bounded, hard-heap-bounded, confined to one filesystem,
and able to distinguish files, directories, symlinks and enumeration errors.
The current directory backend does not yet prove the last four guarantees, so
`idea_admission.eligible=false`: counts remain observable, while PBOS, pbAI,
training, repository creation, implementation and external effects receive no
scan-derived ideas. BACKUP replicas and boot/remediation media must additionally
remain non-corroborating even after the lower-level prerequisites are implemented.

Proposal reviews now reconstruct one of the four canonical proposal definitions,
compare every scalar plus ordered array, and bind the exact definition with a
domain-separated, type-tagged, byte-length-framed SHA-256 digest. This is an
integrity/definition binding only; the review carries the explicit
`sha256-pblang-project-proposal-v1` algorithm ID. Both proposal-definition and publisher
authentication remain false until an independent trust mechanism exists.
