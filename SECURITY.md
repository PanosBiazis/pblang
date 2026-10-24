# Security & Privacy Policy (`PBLang 1.0.0-stable`)

## Supported Versions

| Version | Channel | Supported | Security Updates |
| :--- | :--- | :--- | :--- |
| **`1.0.x`** | **`stable`** | ✅ **Yes** | Active support & cryptographic release attestations |
| `< 1.0.0` | `alpha` / `beta` | ❌ No | Superseded by `1.0.0-stable` |

---

## Built-In Security Architecture

PBLang is engineered around **Zero-Trust Capability Isolation** and **By-Default Hardening**:

1. **Capability-Gated Execution Sandbox (`--sandbox`)**:
   - File system (`--allow-read`, `--allow-write`), network (`--allow-net`), subprocess (`--allow-exec`), environment (`--allow-env`), and native FFI (`--ffi-lib`, `--ffi-symbol`) operations are denied by default unless explicitly granted.
2. **Default-On Static Taint & Control-Flow Analysis (`tools/pbsecure.sapp`)**:
   - Every build runs interprocedural taint and sink verification before execution, blocking command injection, path traversal, SSRF, and unsafe deserialization.
3. **Zero-FFI Verified Cryptography & TLS 1.3**:
   - Pure-PBLang OpenBSD `$2b$` Bcrypt (`stdlib/password_bcrypt.sapp`), Rust-independent TLS 1.3 & PEM validation (`stdlib/tls.sapp`), RFC 7636 PKCE OIDC/SSO (`stdlib/oidc_sso.sapp`), and AES-256-GCM authenticated archives (`stdlib/secure_archive.sapp`).
4. **A+ Web Security Headers by Default**:
   - `stdlib/http_server.sapp` and `stdlib/modern_ui_web.sapp` automatically enforce strict Content-Security-Policy (`CSP`), `Strict-Transport-Security` (`HSTS`), `X-Frame-Options: DENY`, `X-Content-Type-Options: nosniff`, and CSRF token verification.
5. **Automated Public Release Hygiene & Privacy Gate (`stdlib/public_release_audit.sapp`)**:
   - Pre-release structural scanning guarantees zero local workstation paths, zero RFC1918 LAN IPs, zero private keys, and zero credential tokens in published artifacts.

---

## Reporting a Vulnerability

Please report suspected vulnerabilities privately via [GitHub Security Advisories](https://github.com/PanosBiazis/pblang/security/advisories/new) rather than opening a public issue. Include a minimal `.sapp` reproduction script and the output of `pblang --version`.
