# Program signatures and domain authentication — reviewed 2026-09-24

These are different trust mechanisms. A filename, icon, process label, checksum
or successfully decoded signature alone does not authenticate a publisher.
A valid signature also does not establish that a program is harmless.

| Surface | Evidence inspected | Remaining boundary |
| --- | --- | --- |
| PBLang release statements | Ed25519 verification, pinned publisher trust, canonical claims, version/sequence, digest and signed size tests pass | Key enrollment/rotation/revocation, trusted publisher provisioning and authoritative rollback state need a complete release lifecycle |
| Actual release bytes | Artifact admission checks digest and size using an owned immutable snapshot | Snapshot admission does not authorize execution or prove every later handoff uses those bytes |
| Native Windows/macOS programs | Scoped stdlib/tools/docs scan found no integrated general Authenticode or Apple signing pipeline | Sign final platform artifacts, verify chains/timestamps/platform policy and protect signing keys; no production key or certificate was created here |
| TLS domains | Retained host configures certificate roots and hostname/SNI validation | PBLang TLS facade remains default-unavailable because cryptographic provider selection is unresolved; an environment marker alone is not provider initialization proof |
| DNS domains | DNSSEC is a separate trust-chain mechanism | No integrated DNSSEC validation chain was established by this review; DNS resolution and HTTPS do not imply DNSSEC validation |

`product_release_evidence.sapp`, `publisher_artifact_admission.sapp`,
`publisher_trust_store.sapp`, `publisher_release_floor.sapp`, `tls.sapp` and the
retained TLS connection boundary informed this scoped review. TLS handshakes,
certificate expiry/revocation/renewal, DNS delegation and production release
signing were not tested against live domains. No DNS records or servers changed.
The TLS suites exercise unavailable/error behavior, not successful live TLS.

Seven suites passed: release publisher attestation, publisher trust store,
artifact admission, release floor, product release evidence, TLS stub and TLS
enrichment. These results are narrower than a cryptographic implementation audit.
Existing claims such as `authorizes_execution:false` remain meaningful and must
not be replaced with a general 'trusted' badge.

## Release acceptance criteria

1. Bind final distributed bytes to a reviewed source revision and reproducible
   build evidence. Keep private keys outside source, logs and artifacts.
2. Verify using independently provisioned trust, platform certificate policy,
   timestamps and revoked/expired-key cases. Test modified binaries and wrong keys.
3. Preserve verified bytes through installation/execution; prevent path swaps and
   replay of older accepted releases. Check failure and concurrent publication.
4. For domains, verify TLS hostname/certificate chains; manage renewal separately.
   Add DNSSEC only with explicit trust anchors, authenticated denial and rollover
   tests. Neither layer substitutes for application authorization.

Primary references: [Microsoft signing tools](https://learn.microsoft.com/en-us/windows/win32/seccrypto/cryptography-tools)
and [DNSSEC overview, RFC 4033](https://www.rfc-editor.org/info/rfc4033/).
