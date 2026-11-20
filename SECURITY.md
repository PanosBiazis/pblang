# Security & Privacy Policy (`PBLang 5.4.1`)

## Supported Versions

| Version | Channel | Supported | Security Updates |
| :--- | :--- | :--- | :--- |
| **`5.4.x`** | **`stable`** | ✅ **Yes** | Active support — `MorphBoot` UEFI `PE32+` & bounded `.vault` parser, `TPM 2.0` binary marshaler + anti-rollback, Zero-JIT `Ring-0` AOT verifier (`stdlib/os/morphboot_uefi_tpm2_ring0_aot.sapp`), `Virtualized Ring-0 Micro-VM Prism` (`/dev/kvm`/`WHP`) + `Foreign ABI Prism` & `Non-Breaking Egress Containment` (`stdlib/os/microvm_prism_foreign_abi.sapp`), `240Hz` `Sovereign Morphic Glass` + `Trusted Path` + `Virtual Privacy Shutter`, `--strict-taint` + PII redaction + backoff quotas, and **`Morphic Sovereign Elastic Fabric`** (Attested Scale Up/Down/Out/In + One-Way Quarantine + Crypto-Shredding, `stdlib/os/morphic_glass_240hz_taint_elastic.sapp`) |
| `5.3.x` | `maintenance` | ✅ Yes | Previous `5.3` Quantum-Native `Q-MORPH` & Physics-Bound Crypto release; upgrade to `5.4.1` recommended |
| `5.2.x` | `maintenance` | ✅ Yes | Previous `5.2` Post-Transformer release; upgrade to `5.4.1` recommended |
| `5.1.x` | `maintenance` | ✅ Yes | Previous `5.1` pure-microkernel release; upgrade to `5.4.1` recommended |
| `5.0.x` | `maintenance` | ✅ Yes | Sovereign `5.0` series; upgrade to `5.4.1` recommended |
| `4.0.x – 4.2.x` | `maintenance` | ⚠️ Security fixes only | `4.x` Dual-Lock / Closed-Shield series; upgrade to `5.4.1` recommended |
| `2.0.x – 3.0.x` | `legacy` | ❌ No | Superseded by `5.4.1` |
| `< 2.0.0` | `legacy` | ❌ No | Unsupported historical series |

---

## Built-In Security Architecture (`PBLang 5.4.1`)

PBLang `5.4.1` is engineered around **100% Pure-PBLang Sovereign Auditable Subsystems**, **Zero-Trust Capability Isolation**, **Unprivileged OS Kernel Sandboxing**, **Constant-Time & Post-Quantum Cryptography**, and **By-Default Supply-Chain & Public Privacy Hardening**:

1. **Capability-Gated Execution & Unprivileged OS Kernel Sandbox (`--sandbox`, `stdlib/vm/sovereign_pure_microkernel.sapp`, `stdlib/concurrency/green_fibers_doh_sandbox.sapp`)**:
   - `pblang run script.sapp --sandbox` permits canonicalized, symlink-free `stdlib:...` imports while denying ambient file system (`--deny-fs`), network (`--deny-net`), HTTP (`--deny-http`), SQLite, subprocess (`--deny-exec`), environment, and native FFI (`--deny-ffi`) capabilities unless explicitly granted.
   - Unprivileged kernel sandboxing compiles and probes Linux `Landlock` (`LANDLOCK_CREATE_RULESET_VERSION`), `Seccomp-BPF` syscall filters, `PR_SET_NO_NEW_PRIVS`, and Windows `JobObject` process isolation.
2. **Cryptographic Anti-HashDoS, Linear-Time Anti-ReDoS & Bomb-Proof Archives**:
   - Dynamic maps in `stdlib/vm/sovereign_pure_microkernel.sapp` use per-process `128-bit` CSPRNG-seeded **`SipHash-1-3` / `SipHash-2-4`**, neutralizing collision-flooding HashDoS attacks.
   - `stdlib/text/pikevm_linear_regex.sapp` guarantees strict `O(M * N)` Thompson NFA / `PikeVM` matching with zero catastrophic backtracking.
   - `stdlib/compression/deflate_gzip_zstd_lz4_archive.sapp` enforces strict decompression-bomb byte ceilings and blocks `Zip-Slip` (`../` and symlink escapes) before extraction.
3. **Compile-Time `@constant_time` / `@zeroize` Verifier & Zero-OpenSSL Hybrid Post-Quantum TLS 1.3 (`stdlib/security/ct_zeroize_stdlib_index.sapp`, `stdlib/net/zero_openssl_tls13_pqc_quic.sapp`)**:
   - Static AST/IR verification rejects secret-dependent branches, table lookups, and variable-time divisions inside `@constant_time` functions and verifies `@zeroize` buffer wiping on every exit path.
   - Pure-PBLang `TLS 1.3` (`RFC 8446`) and `QUIC` (`RFC 9000/9001`) implement hybrid post-quantum key exchange (`X25519MLKEM768`: NIST `FIPS 203` `ML-KEM-768` + `X25519`), `FIPS 204` `ML-DSA`, `ChaCha20-Poly1305`, `Ed25519`, `FIDO2 / WebAuthn CTAP2`, Hardware `TPM 2.0`, and `PKCS#11 v3.2` non-extractable HSM session policies (`CKA_EXTRACTABLE = false`).
4. **Unicode `UTS #39` Anti-Spoofing & Taint-Safe Polyglot/Web/DB Surfaces (`stdlib/text/unicode16_greek_sovereign.sapp`, `tools/pbsecure.sapp`)**:
   - Detects `CVE-2021-42574` Trojan Source Bidi overrides and `UTS #39` Cyrillic/Greek/Latin mixed-script homoglyph spoofing, enforces parameterized SQL/PBDB queries, and sets `A+` HTTP security headers (`CSP`, `HSTS`, `X-Frame-Options: DENY`, `nosniff`, CSRF tokens).
5. **Signed Lockfiles, `CycloneDX 1.5` / `SPDX 2.3` SBOM & Pure-PBLang Public Privacy Gate (`stdlib/devtools/pkg_init_bench_sbom.sapp`, `tools/ci_docs_privacy_audit.sapp`)**:
   - Every release produces verifiable `SHA256SUMS.txt` and `CycloneDX 1.5` SBOM manifests, while `tools/ci_docs_privacy_audit.sapp` blocks local workstation paths, RFC1918 LAN IPs, private keys, and credentials across all tracked sources and release archives.

---

## Reporting a Vulnerability

Please report suspected vulnerabilities privately via [GitHub Security Advisories](https://github.com/PanosBiazis/pblang/security/advisories/new) rather than opening a public issue. Include a minimal `.sapp` reproduction script and the output of `pblang --version`.
