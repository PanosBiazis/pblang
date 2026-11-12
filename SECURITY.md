# Security & Privacy Policy (`PBLang 2.2.0`)

## Supported Versions

| Version | Channel | Supported | Security Updates |
| :--- | :--- | :--- | :--- |
| **`2.2.x`** | **`stable`** | ✅ **Yes** | Active support — Sovereign Zero-Rust Host VM, Omni-Polyglot Engine & Universal `64/32/16/8-bit` Toolchain |
| `2.0.x – 2.1.x` | `maintenance` | ✅ Yes | Sovereign Zero-Rust `2.x` series; upgrade to `2.2.0` recommended |
| `1.4.x – 1.5.x` | `maintenance` | ⚠️ Security fixes only | Legacy transition series; upgrade to `2.2.0` recommended |
| `1.0.x – 1.3.x` | `legacy` | ❌ No | Superseded by `2.2.0` |
| `< 1.0.0` | `alpha` / `beta` | ❌ No | Unsupported historical snapshots |

---

## Built-In Security Architecture

PBLang `2.2.0` is engineered around **Zero-Trust Capability Isolation**, **Post-Quantum Cryptography**, and **By-Default Hardening**:

1. **Capability-Gated Execution Sandbox (`--sandbox`, `--deny-*`, `--allow-*`)**:
   - `pblang run script.sapp --sandbox` permits canonicalized, symlink-free `stdlib:...` imports from the trusted standard library while denying ambient file system (`--deny-fs`), network (`--deny-net`), HTTP (`--deny-http`), SQLite, subprocess (`--deny-exec`), environment, and native FFI (`--deny-ffi`) operations unless explicitly granted (`--allow-fs`, `--allow-net`, `--allow-http`, `--allow-exec`, `--allow-ffi`, `--ffi-lib`, `--ffi-symbol`).
2. **Default-On Static Taint, Unicode & Supply-Chain Analysis (`tools/pbsecure.sapp`, `stdlib/parseltongue.sapp`, `stdlib/manifest_validate.sapp`)**:
   - Interprocedural taint and sink verification blocks command injection, path traversal, SSRF (including `169.254.169.254` cloud-metadata blocking), Trojan Source Unicode Bidi overrides (`CVE-2021-42574`), and install-hook supply-chain execution (`pblang-package-tree-v2` portable relative lockfiles).
3. **Post-Quantum & Modern Cryptography (`stdlib/pqc_mlkem768.sapp`, `stdlib/tls.sapp`)**:
   - Pure-PBLang NIST FIPS 202 `SHA-3`/`SHAKE` and FIPS 203 `ML-KEM-768` lattice key encapsulation (`stdlib/pqc_mlkem768.sapp`), OpenSSL TLS 1.2/1.3 & Windows `WinHTTP` HTTPS (`runtime_stage3/zr_net_http_tls.h`, `stdlib/tls.sapp`), RFC 8032 `Ed25519` (`stdlib/ed25519_verify.sapp`), RFC 8439 `ChaCha20` (`stdlib/chacha20.sapp`), OpenBSD `$2b$` Bcrypt (`stdlib/password_bcrypt.sapp`), RFC 7636 PKCE OIDC/SSO (`stdlib/oidc_sso.sapp`), and AES-256-GCM authenticated archives (`stdlib/secure_archive.sapp`).
4. **A+ Web Security Headers & Descriptor-Anchored Linux I/O**:
   - `stdlib/http_server.sapp` and `stdlib/modern_ui_web.sapp` enforce strict Content-Security-Policy (`CSP`), `Strict-Transport-Security` (`HSTS`), `X-Frame-Options: DENY`, `X-Content-Type-Options: nosniff`, and CSRF token verification; `stdlib/secure_file.sapp` and `stdlib/linux_dirfs.sapp` prevent symlink/TOCTOU path escapes via `openat`/`O_NOFOLLOW`.
5. **Automated Public Release Hygiene & Privacy Gate (`stdlib/public_release_audit.sapp`, `stdlib/smart_updater.sapp`)**:
   - Pre-release and `pblang update` structural scanning blocks local workstation paths, RFC1918 LAN IPs, private keys, and credential tokens from entering lockfiles or published artifacts.

---

## Reporting a Vulnerability

Please report suspected vulnerabilities privately via [GitHub Security Advisories](https://github.com/PanosBiazis/pblang/security/advisories/new) rather than opening a public issue. Include a minimal `.sapp` reproduction script and the output of `pblang --version`.
