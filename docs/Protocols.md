# pblang Protocols — compatibility, a security layer, and pblang's own protocol

> **The idea (yours).** pblang will (1) be **compatible** with current protocols,
> (2) add **security layers above** them, and (3) ship **its own protocols** that are
> better, more secure, faster, and more efficient. This document makes that concrete,
> assesses the idea's gaps honestly, and records what is built today.

---

## The three tiers

| Tier | What | Status |
|------|------|--------|
| **1. Compatibility** | Speak the existing protocols: `http_client.sapp`/`http.sapp`/`http_server.sapp`, `tls.sapp`, `websocket.sapp`, raw TCP via the `tcp_*` builtins. | **today** |
| **2. Security layer** | `stdlib/securelayer.sapp` — authenticated encryption that wraps **any** transport (HTTP body, a TCP payload, a file). | **today (§89)** |
| **3. Own protocol** | `stdlib/pbp.sapp` — **PBP**, a compact, secure-by-default wire protocol with the security layer built in. | **today (§89)** |

All three are written in pblang (per the rust→pblang directive).

## IDE / Studio protocol consumers

The protocol work is not separate from the IDE. pblang Studio has several places where
protocol design matters:

- **IDE gateway traffic** — browser IDE panels call local pblang gateways for pbAI,
  browsing, and tools. These should eventually prefer PBP or a securelayer-wrapped
  payload when talking to pblang peers, while keeping HTTP compatibility for browsers.
- **Native IDE (`pblang --ide`)** — the browser-free IDE removes DOM/JS attack surface,
  but still needs explicit protocols for tool calls, pbAI, package/registry access, and
  future collaboration.
- **Design documents (`.pbui`)** — visual Studio data should serialize through a stable,
  authenticated format: item tree, responsive layout metadata, 2D/3D/nD coordinates,
  event bindings, and generated-code links. The new `stdlib/design_engine.sapp` item
  model is the local in-memory shape; PBP/securelayer is the network/storage direction.
- **Browser panel** — existing HTTP compatibility remains required, but pblang-to-pblang
  pages/services can use PBP as the compact secure transport behind the same UI.

---

## Tier 2 — the security layer (`securelayer.sapp`)

`seal(key, plaintext, seq)` / `open(key, envelope, seq)` provide **authenticated
encryption** built from the VM's primitives (`sha256`, `hmac_sha256`, `random_bytes`):

- **Encrypt-then-MAC.** A `sha256(key‖nonce‖block)` keystream XORs the plaintext (a
  nonce-based stream cipher); then `hmac_sha256` authenticates `version ‖ aad ‖ nonce ‖
  ciphertext`. The tag is checked in **constant time** *before* decryption.
- **Downgrade protection.** The protocol **version is inside the authenticated tag**, so
  an attacker can't silently force a weaker/compatible fallback.
- **Replay protection.** A **sequence number** is in the authenticated `aad`; a receiver
  rejects any envelope whose seq it has already accepted.
- **Fresh nonce per message** — sealing the same plaintext twice yields different
  envelopes (no deterministic ciphertext).
- **Key-material boundary.** `key` must already be high-entropy key material, such as
  random session bytes or output from a vetted KDF. Do **not** pass human
  passwords/passphrases directly to `seal`/`open`; derive keys first with PBKDF2,
  Argon2, or scrypt in the host/plugin layer until pblang ships a native KDF builtin.

Usable *above current protocols*: `seal` a payload and put it in an HTTP body, a
WebSocket frame, or a file — confidentiality + integrity + authenticity travel with the
data, independent of the transport.

## Tier 3 — PBP, pblang's own protocol (`pbp.sapp`)

A compact application protocol with security **built in**:

- **Lean framing.** `PBP1 <type> <seq> <len>\n<payload>` — a request's header is **~14
  bytes** vs **~112 bytes** for the equivalent HTTP request headers (**~8× leaner**,
  measured by `frame_overhead` vs `http_overhead_estimate`).
- **Secure by default.** Every `DATA` frame is sealed by the Tier‑2 layer; **sessions**
  (`session_new`) enforce **strictly increasing sequence numbers** (replay/reorder
  protection) with no separate handshake for the pre-shared-key case.
- **Server** over TCP (`serve_once`, on `tcp_listen`/`accept`). Codec + sessions +
  security are complete and tested in-VM (`tests/semantics/pbp_test.sapp`,
  `securelayer_test.sapp`).

---

## Honest gap analysis — and how each is handled

The idea is sound; these are the gaps that classically sink hand-rolled protocols. Each
is either **fixed in the design** or **flagged** as needing a VM builtin.

| Gap in the bare idea | Risk | Disposition |
|----------------------|------|-------------|
| "Compatibility **and** our own protocol" | **Downgrade attack** (force the weaker fallback) | **Fixed** — protocol version bound into the authenticated tag. |
| "More secure" with no anti-replay | **Replay/reorder** of captured messages | **Fixed** — sequence number in authenticated AAD + monotonic session seq. |
| Rolling a cipher from `sha256` | Hand-rolled crypto is fragile | **Mitigated** — sound construction (encrypt-then-MAC, constant-time verify). **Flagged**: a **vetted AEAD** (`AES-GCM` / `ChaCha20-Poly1305`) belongs in the VM as a native builtin — it would also remove the hex-encoding payload bloat. |
| Password passed directly as key | Offline guessing if a low-entropy human secret is used as the AEAD key | **Documented boundary** — `securelayer` now requires high-entropy key material; password-derived keys must come from PBKDF2, Argon2, or scrypt before `seal`/`open`. A native KDF builtin remains a future ergonomics improvement. |
| No key establishment named | Pre-shared keys don't scale; **no forward secrecy** | **Flagged** — needs an **X25519** key-agreement builtin for ephemeral session keys (forward secrecy); PSK is the bootstrap. |
| "Faster/efficient" unquantified | Unfalsifiable claims | **Fixed** — framing overhead is **measured** (14 vs 112 bytes); honest note: the *security* envelope currently inflates the payload (from-primitives hex), which the native AEAD above resolves. |
| Live client transport | Can't open a connection | **Flagged** — the VM has `tcp_listen`/`accept` and only `tcp_connect_tls`; a plain **`tcp_connect`** builtin is needed for a pblang PBP *client* (the server + codec + crypto are done). |
| Authn/identity | Anyone with the key is trusted | **Future** — per-peer identities/certs over the security layer (build on `security.sapp` JWT + signatures). |

**Net:** the design now resists downgrade and replay and quantifies its efficiency; the
remaining items are concrete VM builtins (native AEAD, X25519, `tcp_connect`) rather than
design holes.

---

## Tier 2b — Tor + layered security, and careful network permissions

Directives: *the IDE browser can route via **Tor** (opt-in, off by default); a Tor connect
**adds security layers**; and when an app is granted internet permission we **protect
everything**.*

- **Tor (opt-in).** `browser.fetch_page(url, via_tor=false)` / `submit(…, via_tor=false)`
  default to a direct connection; the IDE's `🧅 Tor` toggle (off by default) sends
  `tor:true`. Routing uses **`stdlib/socks5.sapp`** — the SOCKS5 client protocol (no-auth
  greeting + CONNECT request/reply) in pblang, tested in the VM (`socks5_test`). **Gap:**
  the transport needs a plain **`tcp_connect`** builtin (the VM has `tcp_listen`/`accept`
  + only `tcp_connect_tls`) and **TLS-inside-SOCKS** for `https` targets; until then Tor
  mode reports the requirement rather than silently failing.
- **Layered security ("a Tor connect adds security layers" — defense in depth).** Tor
  composes with the other tiers, not replaces them:
  1. **Tor** — anonymity / unlinkable transport.
  2. **TLS** — encryption to the destination (web).
  3. **securelayer** (Tier 2) — authenticated encryption for **pblang peers** (PBP),
     carried *inside* the tunnel: even a compromised exit can't read or forge it.
  Each layer is independent, so a break in one doesn't collapse the others.
- **Careful network permissions.** Network access is **capability-gated** (the VM's
  `http`/`net` capabilities; nothing reaches the network without an explicit grant).
  The principle: *a granted internet permission is a contract to protect everything that
  flows under it* — so network-permitted code should default to the security layers above
  (TLS always; securelayer for pblang peers; Tor available), and the gateway that fronts
  the IDE is itself an explicit, auditable boundary. Secure-by-default, not opt-in.

## Roadmap

1. **(done, §89)** Security layer + PBP codec/sessions/server + measured efficiency, tested.
2. Native **AEAD** + **X25519** + **`tcp_connect`** builtins → live PBP client, forward
   secrecy, compact binary frames, vetted cipher.
3. PBP features: multiplexed streams, HELLO/ACK handshake with negotiated version, flow
   control; a compatibility **bridge** (PBP ⇄ HTTP) so PBP services are reachable from
   the current web.
4. Wire the security layer through the IDE gateway and the pblang web browser.
5. Define a Studio design-document protocol (`.pbui`) for 2D/3D/nD layouts, event
   bindings, and code links; store locally as readable data and transmit over
   securelayer/PBP for collaboration.

*Next per the plan: continue the browser, then anything else.*
