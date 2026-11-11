//! `pblang update` — Smart, Polymorphic, Atomic, Cryptographically Verified &
//! Privacy-Preserving Updater for Projects, `.sapp`/`.pbck` Artifacts, Launchers,
//! ELF Binaries, Sealed Apps, `.zip`/`.tar.gz`/`.tar` Archives, `stdlib`, and `self`.

use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::net::{IpAddr, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use flate2::read::{DeflateDecoder, GzDecoder};
use flate2::write::{DeflateEncoder, GzEncoder};
use flate2::Compression;
use ring::signature::{UnparsedPublicKey, ED25519};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const MAX_ARCHIVE_ENTRIES: usize = 10_000;
const MAX_ARCHIVE_TOTAL_BYTES: u64 = 256 * 1024 * 1024;
const MAX_COMPRESSION_RATIO: u64 = 200;

#[derive(Debug, Clone, Default)]
struct UpdateOpts {
    targets: Vec<String>,
    dry_run: bool,
    check_only: bool,
    rollback: bool,
    from: Option<String>,
    pubkey_hex: Option<String>,
    expected_sha256: Option<String>,
    install_root: Option<PathBuf>,
    bin_dir: Option<PathBuf>,
    json: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PbckProvenanceMeta {
    format: String,
    source: String,
    source_sha256: String,
    pbck_sha256: String,
    compiler_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StdlibManifest {
    format: String,
    compiler_version: String,
    tree_sha256: String,
    file_count: usize,
    files: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize)]
struct TargetReceipt {
    target: String,
    detected_kind: String,
    status: String,
    dry_run: bool,
    writes_performed: usize,
    added: usize,
    modified: usize,
    removed: usize,
    unchanged: usize,
    sha256_before: Option<String>,
    sha256_after: Option<String>,
    backup_path: Option<String>,
    detail: String,
}

pub(crate) fn handle_update(raw_args: Vec<String>) {
    match run_update(raw_args) {
        Ok(code) => {
            if code != 0 {
                std::process::exit(code);
            }
        }
        Err(msg) => {
            let clean = scrub_privacy_text(&msg);
            eprintln!("pblang update: {clean}");
            std::process::exit(1);
        }
    }
}

fn print_update_help() {
    println!(
        "pblang update — Smart Polymorphic Atomic Updater (v{})\n\n\
         USAGE:\n  \
           pblang update [targets...] [FLAGS]\n\n\
         TARGETS (auto-detected):\n  \
           (none) / <dir> / pblang.toml / pblang.lock   Project manifest, portable lockfile & stale .pbck refresh\n  \
           <file.sapp> | <file.pbck>                    Incremental .pbck + .pbck.meta.json provenance rebuild\n  \
           <launcher | elf-binary | sealed-app-dir>     ETXTBSY-safe atomic binary/launcher/app update\n  \
           <archive.zip | .tar.gz | .tgz | .tar>        Bomb/traversal-guarded per-entry SHA-256 archive delta sync\n  \
           stdlib                                       Syntax-gated, Ed25519/SHA-256 atomic stdlib delta update\n  \
           self                                         ETXTBSY-safe atomic pblang runtime + wrapper + stdlib update\n\n\
         FLAGS:\n  \
           --dry-run                  Compute exact cryptographic delta without modifying disk\n  \
           --check                    Exit 0 if already_up_to_date, exit 2 if updates are pending\n  \
           --rollback                 Atomically restore previous .bak / prior release snapshot\n  \
           --from <path|https-url>    Explicit source directory, file, or signed release archive\n  \
           --expected-sha256 <hex>    Verify 64-hex SHA-256 digest before staging\n  \
           --pubkey <hex>             Verify 32-byte RFC 8032 Ed25519 public key against .sig\n  \
           --install-root <dir>       Override release root (default: ~/.local/share/pblang)\n  \
           --bin-dir <dir>            Override launcher bin dir (default: ~/.local/bin)\n  \
           --json                     Emit privacy-scrubbed machine-readable JSON receipt\n  \
           -h, --help                 Show this help message",
        env!("CARGO_PKG_VERSION")
    );
}

fn parse_opts(args: Vec<String>) -> Result<Option<UpdateOpts>, String> {
    let mut opts = UpdateOpts::default();
    let mut i = 0usize;
    while i < args.len() {
        let a = &args[i];
        match a.as_str() {
            "-h" | "--help" => {
                print_update_help();
                return Ok(None);
            }
            "--dry-run" => opts.dry_run = true,
            "--check" => {
                opts.check_only = true;
                opts.dry_run = true;
            }
            "--rollback" => opts.rollback = true,
            "--json" => opts.json = true,
            "--from" => {
                i += 1;
                let v = args.get(i).ok_or_else(|| "--from requires a value".to_string())?;
                opts.from = Some(v.clone());
            }
            "--pubkey" => {
                i += 1;
                let v = args.get(i).ok_or_else(|| "--pubkey requires a 64-hex Ed25519 key".to_string())?;
                opts.pubkey_hex = Some(v.clone());
            }
            "--expected-sha256" => {
                i += 1;
                let v = args.get(i).ok_or_else(|| "--expected-sha256 requires a 64-hex digest".to_string())?;
                opts.expected_sha256 = Some(v.clone());
            }
            "--install-root" => {
                i += 1;
                let v = args.get(i).ok_or_else(|| "--install-root requires a directory path".to_string())?;
                opts.install_root = Some(PathBuf::from(v));
            }
            "--bin-dir" => {
                i += 1;
                let v = args.get(i).ok_or_else(|| "--bin-dir requires a directory path".to_string())?;
                opts.bin_dir = Some(PathBuf::from(v));
            }
            _ if a.starts_with("--from=") => opts.from = Some(a["--from=".len()..].to_string()),
            _ if a.starts_with("--pubkey=") => opts.pubkey_hex = Some(a["--pubkey=".len()..].to_string()),
            _ if a.starts_with("--expected-sha256=") => {
                opts.expected_sha256 = Some(a["--expected-sha256=".len()..].to_string())
            }
            _ if a.starts_with("--install-root=") => {
                opts.install_root = Some(PathBuf::from(&a["--install-root=".len()..]))
            }
            _ if a.starts_with("--bin-dir=") => {
                opts.bin_dir = Some(PathBuf::from(&a["--bin-dir=".len()..]))
            }
            _ if a.starts_with('-') => {
                return Err(format!("unknown flag '{a}' (see `pblang update --help`)"));
            }
            _ => opts.targets.push(a.clone()),
        }
        i += 1;
    }
    if opts.targets.is_empty() {
        opts.targets.push(".".to_string());
    }
    if let Some(ref sha) = opts.expected_sha256 {
        if sha.len() != 64 || !sha.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err("--expected-sha256 must be a 64-character hex SHA-256 digest".to_string());
        }
    }
    if let Some(ref pk) = opts.pubkey_hex {
        if pk.len() != 64 || !pk.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err("--pubkey must be a 64-character hex Ed25519 public key (32 bytes)".to_string());
        }
    }
    Ok(Some(opts))
}

fn run_update(raw_args: Vec<String>) -> Result<i32, String> {
    let opts = match parse_opts(raw_args)? {
        Some(o) => o,
        None => return Ok(0),
    };

    if let Some(ref from_val) = opts.from {
        if is_url_like(from_val) {
            validate_remote_url_ssrf(
                from_val,
                opts.expected_sha256.as_deref(),
                opts.pubkey_hex.as_deref(),
            )?;
            return Err(
                "offline-by-default policy: remote URL validated safe, download archive locally or supply local --from path"
                    .to_string(),
            );
        }
    }

    let mut receipts = Vec::with_capacity(opts.targets.len());
    for target in &opts.targets {
        if is_url_like(target) {
            validate_remote_url_ssrf(
                target,
                opts.expected_sha256.as_deref(),
                opts.pubkey_hex.as_deref(),
            )?;
            return Err(
                "offline-by-default policy: remote target URL validated safe; pass local target path"
                    .to_string(),
            );
        }
        let mut r = dispatch_single_target(target, &opts)?;
        r.target = scrub_privacy_text(&r.target);
        r.detail = scrub_privacy_text(&r.detail);
        r.backup_path = r.backup_path.map(|b| scrub_privacy_text(&b));
        receipts.push(r);
    }

    let any_pending = receipts
        .iter()
        .any(|r| r.status == "update_available" || r.status == "updated");

    if opts.json {
        let payload = if receipts.len() == 1 {
            serde_json::to_string_pretty(&receipts[0]).map_err(|e| e.to_string())?
        } else {
            serde_json::to_string_pretty(&serde_json::json!({
                "ok": true,
                "version": env!("CARGO_PKG_VERSION"),
                "results": receipts,
            }))
            .map_err(|e| e.to_string())?
        };
        println!("{}", scrub_privacy_text(&payload));
    } else {
        for r in &receipts {
            println!(
                "[{}] {} ({}) — {} (writes={}, +{} ~{} -{} ={})",
                r.status,
                r.target,
                r.detected_kind,
                r.detail,
                r.writes_performed,
                r.added,
                r.modified,
                r.removed,
                r.unchanged
            );
        }
    }

    if opts.check_only && any_pending {
        return Ok(2);
    }
    Ok(0)
}

/* ── Privacy Scrubbing & SSRF / Ed25519 Guards ───────────────────────────── */

pub(crate) fn scrub_privacy_text(input: &str) -> String {
    let mut s = input.to_string();
    if let Ok(home) = std::env::var("HOME") {
        let h = home.trim_end_matches('/');
        if h.len() >= 3 {
            s = s.replace(h, "~");
        }
    }
    if let Ok(user) = std::env::var("USER").or_else(|_| std::env::var("USERNAME")) {
        if user.len() >= 3 && user != "root" && user != "user" {
            s = s.replace(&format!("/home/{user}"), "~");
            s = s.replace(&format!("/media/{user}"), "<MEDIA>");
            s = s.replace(&format!("/Users/{user}"), "~");
        }
    }
    if let Ok(host) = std::env::var("HOSTNAME").or_else(|_| {
        fs::read_to_string("/etc/hostname").map(|v| v.trim().to_string())
    }) {
        if host.len() >= 3 && host != "localhost" {
            s = s.replace(&host, "<HOST>");
        }
    }
    s = scrub_os_prefix(&s, "/home/", '/');
    s = scrub_os_prefix(&s, "/media/", '/');
    s = scrub_os_prefix(&s, "/Users/", '/');
    s
}

fn scrub_os_prefix(text: &str, prefix: &str, sep: char) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(idx) = rest.find(prefix) {
        out.push_str(&rest[..idx]);
        let after = &rest[idx + prefix.len()..];
        if let Some(end_u) = after.find(sep) {
            let replacement = if prefix == "/media/" { "<MEDIA>/" } else { "~/" };
            out.push_str(replacement);
            rest = &after[end_u + 1..];
        } else {
            let replacement = if prefix == "/media/" { "<MEDIA>" } else { "~" };
            out.push_str(replacement);
            rest = "";
        }
    }
    out.push_str(rest);
    out
}

fn is_url_like(s: &str) -> bool {
    s.contains("://")
}

fn is_private_or_ssrf_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let o = v4.octets();
            v4.is_loopback()
                || v4.is_unspecified()
                || v4.is_broadcast()
                || v4.is_multicast()
                || o[0] == 10
                || o[0] == 127
                || (o[0] == 172 && (16..=31).contains(&o[1]))
                || (o[0] == 192 && o[1] == 168)
                || (o[0] == 169 && o[1] == 254)
                || (o[0] == 100 && (64..=127).contains(&o[1]))
        }
        IpAddr::V6(v6) => {
            let seg0 = v6.segments()[0];
            v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_multicast()
                || (seg0 & 0xfe00) == 0xfc00
                || (seg0 & 0xffc0) == 0xfe80
                || v6.to_ipv4_mapped().map(IpAddr::V4).is_some_and(is_private_or_ssrf_ip)
        }
    }
}

pub(crate) fn validate_remote_url_ssrf(
    url_str: &str,
    expected_sha256: Option<&str>,
    pubkey_hex: Option<&str>,
) -> Result<(), String> {
    if !url_str.starts_with("https://") {
        return Err("remote update URLs must use explicit https:// (plaintext http:// denied)".to_string());
    }
    let after_scheme = &url_str["https://".len()..];
    let authority = after_scheme.split('/').next().unwrap_or("");
    if authority.is_empty() || authority.contains('@') {
        return Err("invalid or credential-bearing https:// authority".to_string());
    }
    let (host, port) = if authority.starts_with('[') {
        let end = authority.find(']').ok_or_else(|| "malformed IPv6 host in URL".to_string())?;
        let h = &authority[1..end];
        let p = authority[end + 1..]
            .strip_prefix(':')
            .and_then(|s| s.parse::<u16>().ok())
            .unwrap_or(443);
        (h, p)
    } else if let Some((h, p_str)) = authority.rsplit_once(':') {
        let p = p_str.parse::<u16>().map_err(|_| "invalid port in URL".to_string())?;
        (h, p)
    } else {
        (authority, 443)
    };
    let host_low = host.to_ascii_lowercase();
    if host_low == "localhost"
        || host_low.ends_with(".localhost")
        || host_low.ends_with(".local")
        || host_low.ends_with(".internal")
    {
        return Err(format!("SSRF guard blocked loopback/local hostname '{host}'"));
    }
    if let Ok(ip) = host.parse::<IpAddr>() {
        if is_private_or_ssrf_ip(ip) {
            return Err(format!("SSRF guard blocked private/loopback/link-local IP '{ip}'"));
        }
    } else if let Ok(addrs) = (host, port).to_socket_addrs() {
        for sa in addrs {
            if is_private_or_ssrf_ip(sa.ip()) {
                return Err(format!(
                    "SSRF guard blocked hostname '{host}' resolving to private IP {}",
                    sa.ip()
                ));
            }
        }
    }
    if expected_sha256.is_none() && pubkey_hex.is_none() {
        return Err(
            "remote https:// updates require --expected-sha256 <hex> or --pubkey <hex> cryptographic pinning"
                .to_string(),
        );
    }
    Ok(())
}

fn verify_ed25519_signature(payload: &[u8], pubkey_hex: &str, sig_path: &Path) -> Result<(), String> {
    let pk_bytes = hex::decode(pubkey_hex)
        .map_err(|e| format!("invalid Ed25519 --pubkey hex: {e}"))?;
    if pk_bytes.len() != 32 {
        return Err("Ed25519 public key must decode to exactly 32 bytes".to_string());
    }
    let raw_sig = fs::read(sig_path).map_err(|e| {
        format!(
            "Ed25519 verification requested (--pubkey) but signature file '{}' could not be read: {e}",
            scrub_privacy_text(&sig_path.display().to_string())
        )
    })?;
    let sig_bytes = if raw_sig.len() == 64 {
        raw_sig
    } else {
        let trimmed = std::str::from_utf8(&raw_sig)
            .map_err(|_| "signature file must be 64 raw bytes or 128-hex UTF-8".to_string())?
            .trim();
        hex::decode(trimmed).map_err(|e| format!("invalid hex in signature file: {e}"))?
    };
    if sig_bytes.len() != 64 {
        return Err(format!(
            "Ed25519 signature must be 64 bytes (got {})",
            sig_bytes.len()
        ));
    }
    let peer_key = UnparsedPublicKey::new(&ED25519, &pk_bytes);
    peer_key
        .verify(payload, &sig_bytes)
        .map_err(|_| "Ed25519 cryptographic signature verification FAILED".to_string())
}

/* ── Exclusive Atomic File Helpers & Permissions Guard ───────────────────── */

fn sha256_bytes(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn unique_nonce() -> u128 {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    nanos ^ ((std::process::id() as u128) << 64)
}

fn set_safe_unix_mode(path: &Path, mode: u32) -> Result<(), String> {
    if (mode & 0o022) != 0 {
        return Err(format!("refusing insecure world/group-writable mode {:o}", mode));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(mode))
            .map_err(|e| format!("chmod {:o} failed: {e}", mode))?;
    }
    Ok(())
}

fn create_exclusive_temp_in(dir: &Path, prefix: &str, create_mode: u32) -> Result<(PathBuf, File), String> {
    fs::create_dir_all(dir).map_err(|e| format!("create_dir_all failed: {e}"))?;
    let base_nonce = unique_nonce();
    for attempt in 0u32..64 {
        let cand = dir.join(format!(
            ".{}.staging.{}.{:x}.{}",
            prefix,
            std::process::id(),
            base_nonce,
            attempt
        ));
        let mut oo = OpenOptions::new();
        oo.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            oo.mode(create_mode & !0o022);
        }
        match oo.open(&cand) {
            Ok(f) => return Ok((cand, f)),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(format!("exclusive temp open failed: {e}")),
        }
    }
    Err("exhausted exclusive staging filename retries".to_string())
}

/// ETXTBSY-safe atomic file replacement: writes `bytes` to a brand-new `O_CREAT|O_EXCL`
/// inode in the same directory, optionally writes a `0600`/`0700` `.bak` copy of the
/// previous file, sets `final_mode` (`mode & 0o022 == 0`), and `rename`s into place.
fn write_atomic_with_backup(
    target: &Path,
    bytes: &[u8],
    final_mode: u32,
    keep_backup: bool,
) -> Result<Option<PathBuf>, String> {
    if (final_mode & 0o022) != 0 {
        return Err(format!("refusing group/world-writable mode {:o}", final_mode));
    }
    let parent = target
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let fname = target
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("artifact");

    let mut backup_written = None;
    if keep_backup && target.is_file() {
        let old_bytes = fs::read(target).map_err(|e| format!("read existing file for backup: {e}"))?;
        let bak_path = target.with_file_name(format!("{fname}.bak"));
        let bak_mode = if (final_mode & 0o100) != 0 { 0o700 } else { 0o600 };
        let (tmp_bak, mut bf) = create_exclusive_temp_in(parent, &format!("{fname}.bak"), bak_mode)?;
        bf.write_all(&old_bytes)
            .and_then(|_| bf.sync_all())
            .map_err(|e| {
                let _ = fs::remove_file(&tmp_bak);
                format!("write backup failed: {e}")
            })?;
        drop(bf);
        set_safe_unix_mode(&tmp_bak, bak_mode)?;
        fs::rename(&tmp_bak, &bak_path).map_err(|e| {
            let _ = fs::remove_file(&tmp_bak);
            format!("rename backup failed: {e}")
        })?;
        backup_written = Some(bak_path);
    }

    let stage_mode = if (final_mode & 0o100) != 0 { 0o700 } else { 0o600 };
    let (tmp_path, mut tf) = create_exclusive_temp_in(parent, fname, stage_mode)?;
    tf.write_all(bytes)
        .and_then(|_| tf.sync_all())
        .map_err(|e| {
            let _ = fs::remove_file(&tmp_path);
            format!("write staged file failed: {e}")
        })?;
    drop(tf);
    set_safe_unix_mode(&tmp_path, final_mode)?;
    fs::rename(&tmp_path, target).map_err(|e| {
        let _ = fs::remove_file(&tmp_path);
        format!("atomic rename into place failed: {e}")
    })?;
    Ok(backup_written)
}

/* ── Syntax & Bidi Source Validation ─────────────────────────────────────── */

fn check_sapp_syntax_clean(src: &str, label: &str) -> Result<(), String> {
    for ch in src.chars() {
        let cp = ch as u32;
        if (0x202A..=0x202E).contains(&cp) || (0x2066..=0x2069).contains(&cp) {
            return Err(format!(
                "Trojan Source Bidi override character U+{cp:04X} rejected in '{label}'"
            ));
        }
    }
    pblang_core::parse_strict(src)
        .map(|_| ())
        .map_err(|errs| format!("syntax check failed in '{label}': {errs:?}"))
}

/* ── Archive Safety: Traversal, Symlink & Zip/Tar Bomb Defenses ──────────── */

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ArchiveFormat {
    Zip,
    TarGz,
    Tar,
}

fn detect_archive_format(path: &Path) -> Option<ArchiveFormat> {
    let name = path.file_name()?.to_str()?.to_ascii_lowercase();
    if name.ends_with(".zip") {
        Some(ArchiveFormat::Zip)
    } else if name.ends_with(".tar.gz") || name.ends_with(".tgz") {
        Some(ArchiveFormat::TarGz)
    } else if name.ends_with(".tar") {
        Some(ArchiveFormat::Tar)
    } else {
        None
    }
}

pub(crate) fn validate_archive_entry_rel(raw: &str) -> Result<String, String> {
    if raw.is_empty() || raw.len() > 1024 {
        return Err("archive entry has empty or oversized path".to_string());
    }
    if raw.bytes().any(|b| b == 0 || b < 0x20) {
        return Err("archive entry contains NUL or control characters".to_string());
    }
    if raw.starts_with('/') || raw.starts_with('\\') {
        return Err(format!("archive entry '{}' has forbidden leading slash", raw));
    }
    if raw.len() >= 2 && raw.as_bytes()[0].is_ascii_alphabetic() && raw.as_bytes()[1] == b':' {
        return Err(format!("archive entry '{}' has forbidden drive prefix", raw));
    }
    let unified = raw.replace('\\', "/");
    let mut parts = Vec::new();
    for seg in unified.split('/') {
        if seg.is_empty() || seg == "." {
            continue;
        }
        if seg == ".." {
            return Err(format!("archive entry '{}' contains path traversal '..'", raw));
        }
        parts.push(seg);
    }
    if parts.is_empty() {
        return Err("archive entry normalized to empty path".to_string());
    }
    Ok(parts.join("/"))
}

fn crc32_ieee(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &b in data {
        crc ^= b as u32;
        for _ in 0..8 {
            if (crc & 1) != 0 {
                crc = (crc >> 1) ^ 0xEDB8_8320;
            } else {
                crc >>= 1;
            }
        }
    }
    !crc
}

fn u16_le(b: &[u8], off: usize) -> Result<u16, String> {
    if off + 2 > b.len() {
        return Err("zip: truncated u16".to_string());
    }
    Ok(u16::from_le_bytes([b[off], b[off + 1]]))
}

fn u32_le(b: &[u8], off: usize) -> Result<u32, String> {
    if off + 4 > b.len() {
        return Err("zip: truncated u32".to_string());
    }
    Ok(u32::from_le_bytes([b[off], b[off + 1], b[off + 2], b[off + 3]]))
}

fn read_zip_entries_safe(zip_bytes: &[u8]) -> Result<BTreeMap<String, Vec<u8>>, String> {
    if (zip_bytes.len() as u64) > MAX_ARCHIVE_TOTAL_BYTES {
        return Err("zip archive exceeds 256 MiB safety bound".to_string());
    }
    if zip_bytes.len() < 22 {
        return Err("zip: truncated archive (smaller than EOCD)".to_string());
    }
    let search_min = zip_bytes.len().saturating_sub(22 + 65535);
    let mut eocd_pos = None;
    for p in (search_min..=zip_bytes.len() - 22).rev() {
        if &zip_bytes[p..p + 4] == b"PK\x05\x06" {
            eocd_pos = Some(p);
            break;
        }
    }
    let eocd = eocd_pos.ok_or_else(|| "zip: End of Central Directory signature not found".to_string())?;
    let total_entries = u16_le(zip_bytes, eocd + 10)? as usize;
    if total_entries > MAX_ARCHIVE_ENTRIES {
        return Err(format!(
            "zip bomb guard: entry count {} exceeds limit {}",
            total_entries, MAX_ARCHIVE_ENTRIES
        ));
    }
    let cd_size = u32_le(zip_bytes, eocd + 12)? as usize;
    let cd_offset = u32_le(zip_bytes, eocd + 16)? as usize;
    if cd_offset.saturating_add(cd_size) > eocd {
        return Err("zip: invalid Central Directory bounds".to_string());
    }

    let mut out = BTreeMap::new();
    let mut total_uncompressed: u64 = 0;
    let mut pos = cd_offset;
    for _ in 0..total_entries {
        if pos + 46 > cd_offset + cd_size || &zip_bytes[pos..pos + 4] != b"PK\x01\x02" {
            return Err("zip: malformed Central Directory header".to_string());
        }
        let version_made_by = u16_le(zip_bytes, pos + 4)?;
        let flags = u16_le(zip_bytes, pos + 8)?;
        if (flags & 0x01) != 0 {
            return Err("zip: encrypted entries are unsupported".to_string());
        }
        let method = u16_le(zip_bytes, pos + 10)?;
        let expected_crc = u32_le(zip_bytes, pos + 16)?;
        let comp_size = u32_le(zip_bytes, pos + 20)? as usize;
        let uncomp_size = u32_le(zip_bytes, pos + 24)? as u64;
        let name_len = u16_le(zip_bytes, pos + 28)? as usize;
        let extra_len = u16_le(zip_bytes, pos + 30)? as usize;
        let comment_len = u16_le(zip_bytes, pos + 32)? as usize;
        let external_attr = u32_le(zip_bytes, pos + 38)?;
        let lfh_offset = u32_le(zip_bytes, pos + 42)? as usize;

        if pos + 46 + name_len + extra_len + comment_len > cd_offset + cd_size {
            return Err("zip: truncated Central Directory entry name/extra".to_string());
        }
        let raw_name = std::str::from_utf8(&zip_bytes[pos + 46..pos + 46 + name_len])
            .map_err(|_| "zip: entry name is not valid UTF-8".to_string())?;
        pos += 46 + name_len + extra_len + comment_len;

        // Unix creator OS = 3: check S_IFMT (upper 16 bits) for symlinks/devices
        if (version_made_by >> 8) == 3 {
            let unix_mode = (external_attr >> 16) & 0o170000;
            if unix_mode == 0o120000 {
                return Err(format!("zip security guard: symlink entry '{}' rejected", raw_name));
            }
            if unix_mode != 0 && unix_mode != 0o100000 && unix_mode != 0o040000 {
                return Err(format!("zip security guard: special file entry '{}' rejected", raw_name));
            }
        }

        if raw_name.ends_with('/') {
            let _ = validate_archive_entry_rel(raw_name.trim_end_matches('/'))?;
            continue;
        }
        let clean_name = validate_archive_entry_rel(raw_name)?;

        if comp_size > 4096 && uncomp_size > (comp_size as u64).saturating_mul(MAX_COMPRESSION_RATIO) {
            return Err(format!(
                "zip bomb guard: entry '{}' exceeds {}:1 compression ratio",
                clean_name, MAX_COMPRESSION_RATIO
            ));
        }
        total_uncompressed = total_uncompressed.saturating_add(uncomp_size);
        if total_uncompressed > MAX_ARCHIVE_TOTAL_BYTES {
            return Err("zip bomb guard: cumulative uncompressed size exceeds 256 MiB".to_string());
        }

        if lfh_offset + 30 > zip_bytes.len() || &zip_bytes[lfh_offset..lfh_offset + 4] != b"PK\x03\x04" {
            return Err("zip: invalid Local File Header offset".to_string());
        }
        let lfh_name_len = u16_le(zip_bytes, lfh_offset + 26)? as usize;
        let lfh_extra_len = u16_le(zip_bytes, lfh_offset + 28)? as usize;
        let data_start = lfh_offset + 30 + lfh_name_len + lfh_extra_len;
        let data_end = data_start.saturating_add(comp_size);
        if data_end > cd_offset {
            return Err("zip: compressed payload overlaps Central Directory".to_string());
        }
        let comp_slice = &zip_bytes[data_start..data_end];

        let data = match method {
            0 => {
                if comp_slice.len() as u64 != uncomp_size {
                    return Err("zip: stored size mismatch".to_string());
                }
                comp_slice.to_vec()
            }
            8 => {
                let max_entry_read = (MAX_ARCHIVE_TOTAL_BYTES - (total_uncompressed - uncomp_size))
                    .min(uncomp_size.saturating_add(1));
                let mut dec = DeflateDecoder::new(comp_slice).take(max_entry_read + 1);
                let mut buf = Vec::new();
                dec.read_to_end(&mut buf)
                    .map_err(|e| format!("zip: deflate error in '{}': {e}", clean_name))?;
                if buf.len() as u64 != uncomp_size {
                    return Err(format!(
                        "zip bomb/size mismatch in '{}': declared {}, decoded {}",
                        clean_name,
                        uncomp_size,
                        buf.len()
                    ));
                }
                if comp_size <= 4096 && buf.len() > 4 * 1024 * 1024 {
                    return Err(format!("zip bomb guard: suspicious small-payload expansion in '{}'", clean_name));
                }
                buf
            }
            other => return Err(format!("zip: unsupported compression method {other}")),
        };

        if crc32_ieee(&data) != expected_crc {
            return Err(format!("zip: CRC-32 mismatch for '{}'", clean_name));
        }
        out.insert(clean_name, data);
    }
    Ok(out)
}

fn write_zip_entries(entries: &BTreeMap<String, Vec<u8>>) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    let mut cd = Vec::new();
    let count = u16::try_from(entries.len())
        .map_err(|_| "zip writer: too many entries for standard ZIP16".to_string())?;

    for (rel, data) in entries {
        let clean = validate_archive_entry_rel(rel)?;
        let name_bytes = clean.as_bytes();
        let crc = crc32_ieee(data);
        let uncomp_len = u32::try_from(data.len()).map_err(|_| "zip entry > 4 GiB".to_string())?;

        let mut enc = DeflateEncoder::new(Vec::new(), Compression::default());
        enc.write_all(data).map_err(|e| format!("zip deflate: {e}"))?;
        let compressed = enc.finish().map_err(|e| format!("zip deflate finish: {e}"))?;
        let (method, payload): (u16, &[u8]) = if compressed.len() < data.len() {
            (8, compressed.as_slice())
        } else {
            (0, data.as_slice())
        };
        let comp_len = u32::try_from(payload.len()).map_err(|_| "zip compressed > 4 GiB".to_string())?;
        let lfh_offset = u32::try_from(out.len()).map_err(|_| "zip archive > 4 GiB".to_string())?;

        // Local File Header
        out.extend_from_slice(b"PK\x03\x04");
        out.extend_from_slice(&20u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&method.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&0x5421u16.to_le_bytes());
        out.extend_from_slice(&crc.to_le_bytes());
        out.extend_from_slice(&comp_len.to_le_bytes());
        out.extend_from_slice(&uncomp_len.to_le_bytes());
        out.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(name_bytes);
        out.extend_from_slice(payload);

        // Central Directory Header (Unix regular file 0100644)
        let ext_attr: u32 = (0o100644u32) << 16;
        cd.extend_from_slice(b"PK\x01\x02");
        cd.extend_from_slice(&((3u16 << 8) | 20u16).to_le_bytes());
        cd.extend_from_slice(&20u16.to_le_bytes());
        cd.extend_from_slice(&0u16.to_le_bytes());
        cd.extend_from_slice(&method.to_le_bytes());
        cd.extend_from_slice(&0u16.to_le_bytes());
        cd.extend_from_slice(&0x5421u16.to_le_bytes());
        cd.extend_from_slice(&crc.to_le_bytes());
        cd.extend_from_slice(&comp_len.to_le_bytes());
        cd.extend_from_slice(&uncomp_len.to_le_bytes());
        cd.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
        cd.extend_from_slice(&0u16.to_le_bytes());
        cd.extend_from_slice(&0u16.to_le_bytes());
        cd.extend_from_slice(&0u16.to_le_bytes());
        cd.extend_from_slice(&0u16.to_le_bytes());
        cd.extend_from_slice(&ext_attr.to_le_bytes());
        cd.extend_from_slice(&lfh_offset.to_le_bytes());
        cd.extend_from_slice(name_bytes);
    }

    let cd_offset = u32::try_from(out.len()).map_err(|_| "zip cd offset > 4 GiB".to_string())?;
    let cd_size = u32::try_from(cd.len()).map_err(|_| "zip cd size > 4 GiB".to_string())?;
    out.extend_from_slice(&cd);

    // EOCD
    out.extend_from_slice(b"PK\x05\x06");
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&count.to_le_bytes());
    out.extend_from_slice(&count.to_le_bytes());
    out.extend_from_slice(&cd_size.to_le_bytes());
    out.extend_from_slice(&cd_offset.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    Ok(out)
}

fn read_tar_stream_safe<R: Read>(reader: R, compressed_len: Option<u64>) -> Result<BTreeMap<String, Vec<u8>>, String> {
    let mut ar = tar::Archive::new(reader);
    let mut out = BTreeMap::new();
    let mut entry_count = 0usize;
    let mut total_bytes = 0u64;

    let entries = ar
        .entries()
        .map_err(|e| format!("tar: cannot read archive entries: {e}"))?;
    for ent_res in entries {
        let mut ent = ent_res.map_err(|e| format!("tar: malformed entry: {e}"))?;
        entry_count += 1;
        if entry_count > MAX_ARCHIVE_ENTRIES {
            return Err(format!(
                "tar bomb guard: entry count exceeds {}",
                MAX_ARCHIVE_ENTRIES
            ));
        }
        let etype = ent.header().entry_type();
        if etype.is_symlink() || etype.is_hard_link() {
            return Err("tar security guard: symlink/hardlink entry rejected".to_string());
        }
        if etype.is_dir() {
            let raw = ent.path_bytes();
            let s = std::str::from_utf8(&raw).map_err(|_| "tar: non-UTF8 path".to_string())?;
            let trimmed = s.trim_end_matches('/');
            if !trimmed.is_empty() && trimmed != "." {
                let _ = validate_archive_entry_rel(trimmed)?;
            }
            continue;
        }
        if !etype.is_file() {
            return Err(format!("tar security guard: unsupported entry type {:?}", etype));
        }

        let raw_path = {
            let pb = ent.path_bytes();
            std::str::from_utf8(&pb)
                .map_err(|_| "tar: non-UTF8 entry path".to_string())?
                .to_string()
        };
        let clean_name = validate_archive_entry_rel(&raw_path)?;
        let decl_size = ent
            .header()
            .size()
            .map_err(|e| format!("tar: invalid size header: {e}"))?;
        total_bytes = total_bytes.saturating_add(decl_size);
        if total_bytes > MAX_ARCHIVE_TOTAL_BYTES {
            return Err("tar bomb guard: cumulative uncompressed size exceeds 256 MiB".to_string());
        }
        if let Some(clen) = compressed_len {
            if clen > 4096 && total_bytes > clen.saturating_mul(MAX_COMPRESSION_RATIO) {
                return Err(format!(
                    "tar.gz bomb guard: decompression ratio exceeds {}:1",
                    MAX_COMPRESSION_RATIO
                ));
            }
        }
        let mut buf = Vec::new();
        ent.by_ref()
            .take(decl_size.saturating_add(1))
            .read_to_end(&mut buf)
            .map_err(|e| format!("tar: read entry '{}': {e}", clean_name))?;
        if buf.len() as u64 != decl_size {
            return Err(format!("tar: truncated entry '{}'", clean_name));
        }
        out.insert(clean_name, buf);
    }
    Ok(out)
}

fn read_archive_entries_safe(path: &Path, fmt: ArchiveFormat) -> Result<BTreeMap<String, Vec<u8>>, String> {
    let raw = fs::read(path).map_err(|e| format!("read archive failed: {e}"))?;
    if (raw.len() as u64) > MAX_ARCHIVE_TOTAL_BYTES {
        return Err("archive file exceeds 256 MiB limit".to_string());
    }
    match fmt {
        ArchiveFormat::Zip => read_zip_entries_safe(&raw),
        ArchiveFormat::TarGz => {
            let gz = GzDecoder::new(raw.as_slice());
            read_tar_stream_safe(gz, Some(raw.len() as u64))
        }
        ArchiveFormat::Tar => read_tar_stream_safe(raw.as_slice(), None),
    }
}

fn write_archive_entries(entries: &BTreeMap<String, Vec<u8>>, fmt: ArchiveFormat) -> Result<Vec<u8>, String> {
    match fmt {
        ArchiveFormat::Zip => write_zip_entries(entries),
        ArchiveFormat::TarGz => {
            let tar_bytes = write_tar_raw(entries)?;
            let mut enc = GzEncoder::new(Vec::new(), Compression::default());
            enc.write_all(&tar_bytes).map_err(|e| format!("gzip write: {e}"))?;
            enc.finish().map_err(|e| format!("gzip finish: {e}"))
        }
        ArchiveFormat::Tar => write_tar_raw(entries),
    }
}

fn write_tar_raw(entries: &BTreeMap<String, Vec<u8>>) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    {
        let mut builder = tar::Builder::new(&mut out);
        for (rel, data) in entries {
            let clean = validate_archive_entry_rel(rel)?;
            let mut hdr = tar::Header::new_gnu();
            hdr.set_path(&clean).map_err(|e| format!("tar set_path: {e}"))?;
            hdr.set_size(data.len() as u64);
            let mode = if clean.starts_with("bin/") { 0o755 } else { 0o644 };
            hdr.set_mode(mode);
            hdr.set_mtime(0);
            hdr.set_cksum();
            builder
                .append(&hdr, data.as_slice())
                .map_err(|e| format!("tar append: {e}"))?;
        }
        builder.finish().map_err(|e| format!("tar finish: {e}"))?;
    }
    Ok(out)
}

/* ── Directory Tree Collection & Canonical Digest ────────────────────────── */

fn collect_dir_files_safe(root: &Path) -> Result<BTreeMap<String, Vec<u8>>, String> {
    let meta = fs::symlink_metadata(root).map_err(|e| format!("stat dir: {e}"))?;
    if meta.file_type().is_symlink() || !meta.is_dir() {
        return Err("source tree root must be a regular non-symlink directory".to_string());
    }
    let mut files = BTreeMap::new();
    collect_dir_rec(root, root, &mut files)?;
    Ok(files)
}

fn collect_dir_rec(root: &Path, cur: &Path, out: &mut BTreeMap<String, Vec<u8>>) -> Result<(), String> {
    for ent in fs::read_dir(cur).map_err(|e| format!("read_dir: {e}"))? {
        let ent = ent.map_err(|e| format!("dir entry: {e}"))?;
        let p = ent.path();
        let name = ent.file_name();
        let name_s = name.to_string_lossy();
        if name_s.starts_with(".staging.") || name_s == "MANIFEST.sha256.json" || name_s == "MANIFEST.sha256.json.sig" {
            continue;
        }
        let meta = fs::symlink_metadata(&p).map_err(|e| format!("symlink_metadata: {e}"))?;
        if meta.file_type().is_symlink() {
            return Err(format!(
                "security guard: symlink rejected inside tree: {}",
                scrub_privacy_text(&p.display().to_string())
            ));
        }
        if meta.is_dir() {
            collect_dir_rec(root, &p, out)?;
        } else if meta.is_file() {
            let rel = p
                .strip_prefix(root)
                .map_err(|_| "strip_prefix failed".to_string())?
                .to_str()
                .ok_or_else(|| "non-UTF8 path in tree".to_string())?
                .replace('\\', "/");
            let clean = validate_archive_entry_rel(&rel)?;
            let bytes = fs::read(&p).map_err(|e| format!("read file '{clean}': {e}"))?;
            out.insert(clean, bytes);
            if out.len() > MAX_ARCHIVE_ENTRIES {
                return Err("tree exceeds maximum entry count".to_string());
            }
        }
    }
    Ok(())
}

fn compute_tree_digest(files: &BTreeMap<String, Vec<u8>>) -> String {
    let mut h = Sha256::new();
    h.update(b"pblang-tree-manifest-v1\0");
    for (rel, content) in files {
        h.update((rel.len() as u64).to_be_bytes());
        h.update(rel.as_bytes());
        h.update((content.len() as u64).to_be_bytes());
        h.update(content);
    }
    hex::encode(h.finalize())
}

fn load_source_tree_or_archive(
    src_path: &Path,
    expected_sha256: Option<&str>,
    pubkey_hex: Option<&str>,
) -> Result<(BTreeMap<String, Vec<u8>>, String), String> {
    if let Some(fmt) = detect_archive_format(src_path) {
        let raw = fs::read(src_path).map_err(|e| format!("read source archive: {e}"))?;
        let archive_sha = sha256_bytes(&raw);
        if let Some(exp) = expected_sha256 {
            if !archive_sha.eq_ignore_ascii_case(exp) {
                return Err(format!(
                    "expected SHA-256 mismatch on archive: expected {exp}, got {archive_sha}"
                ));
            }
        }
        if let Some(pk) = pubkey_hex {
            let sig_path = PathBuf::from(format!("{}.sig", src_path.display()));
            verify_ed25519_signature(&raw, pk, &sig_path)?;
        }
        let entries = read_archive_entries_safe(src_path, fmt)?;
        let digest = compute_tree_digest(&entries);
        Ok((entries, digest))
    } else if src_path.is_dir() {
        let entries = collect_dir_files_safe(src_path)?;
        let digest = compute_tree_digest(&entries);
        if let Some(exp) = expected_sha256 {
            if !digest.eq_ignore_ascii_case(exp) {
                return Err(format!(
                    "expected SHA-256 mismatch on source tree: expected {exp}, got {digest}"
                ));
            }
        }
        if let Some(pk) = pubkey_hex {
            let manifest_path = src_path.join("MANIFEST.sha256.json");
            let sig_path = src_path.join("MANIFEST.sha256.json.sig");
            if manifest_path.is_file() && sig_path.is_file() {
                let man_bytes = fs::read(&manifest_path).map_err(|e| e.to_string())?;
                verify_ed25519_signature(&man_bytes, pk, &sig_path)?;
            } else {
                let ext_sig = PathBuf::from(format!("{}.sig", src_path.display()));
                verify_ed25519_signature(digest.as_bytes(), pk, &ext_sig)?;
            }
        }
        Ok((entries, digest))
    } else {
        Err(format!(
            "source '{}' is neither a directory nor a supported archive",
            scrub_privacy_text(&src_path.display().to_string())
        ))
    }
}

fn diff_maps(
    old_m: &BTreeMap<String, Vec<u8>>,
    new_m: &BTreeMap<String, Vec<u8>>,
) -> (usize, usize, usize, usize) {
    let mut added = 0usize;
    let mut modified = 0usize;
    let mut unchanged = 0usize;
    for (k, nv) in new_m {
        match old_m.get(k) {
            None => added += 1,
            Some(ov) if ov == nv => unchanged += 1,
            Some(_) => modified += 1,
        }
    }
    let removed = old_m.keys().filter(|k| !new_m.contains_key(*k)).count();
    (added, modified, removed, unchanged)
}

/* ── Target Dispatcher ───────────────────────────────────────────────────── */

fn dispatch_single_target(target: &str, opts: &UpdateOpts) -> Result<TargetReceipt, String> {
    if target == "stdlib" {
        return update_stdlib_target(opts);
    }
    if target == "self" {
        return update_self_target(opts);
    }

    let p = Path::new(target);
    if detect_archive_format(p).is_some() {
        return update_archive_target(p, opts);
    }
    if target.ends_with(".sapp") || target.ends_with(".sap") {
        return update_sapp_target(p, opts);
    }
    if target.ends_with(".pbck") {
        return update_pbck_target(p, opts);
    }

    let file_name = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
    if file_name == "pblang.toml" || file_name == "pblang.lock" {
        let dir = p
            .parent()
            .filter(|d| !d.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        return update_project_target(dir, target, opts);
    }

    if p.is_file() {
        let head = read_head_bytes(p, 128)?;
        if head.starts_with(b"\x7fELF") {
            return update_elf_binary_target(p, opts);
        }
        if head.starts_with(b"#!") {
            return update_launcher_script_target(p, opts);
        }
    }

    if p.is_dir() {
        if p.join("pblang.toml").is_file() {
            return update_project_target(p, target, opts);
        }
        if p.join("installation.json").is_file() || p.join("MANIFEST.sha256.json").is_file() {
            return update_sealed_app_dir_target(p, opts);
        }
        if let Some(man_dir) = pblang_pm::find_manifest_dir(p) {
            return update_project_target(&man_dir, target, opts);
        }
    }

    Err(format!(
        "could not resolve update target '{}'",
        scrub_privacy_text(target)
    ))
}

fn read_head_bytes(p: &Path, max_n: usize) -> Result<Vec<u8>, String> {
    let mut f = File::open(p).map_err(|e| format!("open failed: {e}"))?;
    let mut buf = vec![0u8; max_n];
    let n = f.read(&mut buf).map_err(|e| format!("read failed: {e}"))?;
    buf.truncate(n);
    Ok(buf)
}

/* ── 1. Project / Manifest / Lockfile Updater ────────────────────────────── */

fn update_project_target(man_dir: &Path, label: &str, opts: &UpdateOpts) -> Result<TargetReceipt, String> {
    let man_path = man_dir.join("pblang.toml");
    let lock_path = man_dir.join("pblang.lock");
    if !man_path.is_file() {
        return Err("project directory has no pblang.toml".to_string());
    }

    if opts.rollback {
        let bak_path = man_dir.join("pblang.lock.bak");
        if !bak_path.is_file() {
            return Err("no pblang.lock.bak backup found for rollback".to_string());
        }
        let before = lock_path.is_file().then(|| fs::read(&lock_path).ok()).flatten().map(|b| sha256_bytes(&b));
        let bak_bytes = fs::read(&bak_path).map_err(|e| e.to_string())?;
        let after = sha256_bytes(&bak_bytes);
        if !opts.dry_run {
            write_atomic_with_backup(&lock_path, &bak_bytes, 0o644, false)?;
        }
        return Ok(TargetReceipt {
            target: label.to_string(),
            detected_kind: "project".to_string(),
            status: if opts.dry_run { "rollback_ready" } else { "rolled_back" }.to_string(),
            dry_run: opts.dry_run,
            writes_performed: usize::from(!opts.dry_run),
            added: 0,
            modified: 1,
            removed: 0,
            unchanged: 0,
            sha256_before: before,
            sha256_after: Some(after),
            backup_path: Some("pblang.lock.bak".to_string()),
            detail: "restored pblang.lock from backup".to_string(),
        });
    }

    // Validate manifest syntax & zero-hook hermeticity
    let man_bytes = fs::read(&man_path).map_err(|e| format!("read pblang.toml: {e}"))?;
    let man_str = std::str::from_utf8(&man_bytes).map_err(|_| "pblang.toml is not UTF-8".to_string())?;
    let parsed_man = pblang_pm::parse_manifest(man_str)?;
    if let Some(entry_rel) = parsed_man
        .package
        .as_ref()
        .and_then(|p| p.entry.as_deref())
    {
        let entry_path = man_dir.join(entry_rel);
        if entry_path.is_file() {
            let entry_src = fs::read_to_string(&entry_path)
                .map_err(|e| format!("read entry '{entry_rel}': {e}"))?;
            check_sapp_syntax_clean(&entry_src, entry_rel)?;
        }
    }

    let reg_url = std::env::var("PBLANG_REGISTRY_INDEX").ok();
    let new_lock = pblang_pm::generate_lockfile_with_registry_policy(
        man_dir,
        reg_url.as_deref(),
        false,
        false,
    )?;
    let new_lock_str = scrub_privacy_text(&pblang_pm::write_lockfile(&new_lock)?);
    let new_lock_bytes = new_lock_str.as_bytes();
    let new_sha = sha256_bytes(new_lock_bytes);

    let old_bytes = if lock_path.is_file() {
        Some(fs::read(&lock_path).map_err(|e| format!("read pblang.lock: {e}"))?)
    } else {
        None
    };
    let old_sha = old_bytes.as_ref().map(|b| sha256_bytes(b));
    let lock_changed = old_sha.as_deref() != Some(new_sha.as_str());

    // Also scan immediate project directory (and build/) for stale .pbck files with .pbck.meta.json
    let mut pbck_refreshed = 0usize;
    for sub in [man_dir.to_path_buf(), man_dir.join("src"), man_dir.join("build")] {
        if sub.is_dir() {
            if let Ok(rd) = fs::read_dir(&sub) {
                for ent in rd.flatten() {
                    let ep = ent.path();
                    if ep.extension().and_then(|s| s.to_str()) == Some("pbck") {
                        let meta_p = PathBuf::from(format!("{}.meta.json", ep.display()));
                        let sib_sapp = ep.with_extension("sapp");
                        if meta_p.is_file() || sib_sapp.is_file() {
                            let r = update_pbck_target(&ep, opts)?;
                            if r.status == "updated" || r.status == "update_available" {
                                pbck_refreshed += 1;
                            }
                        }
                    }
                }
            }
        }
    }

    if !lock_changed && pbck_refreshed == 0 {
        return Ok(TargetReceipt {
            target: label.to_string(),
            detected_kind: "project".to_string(),
            status: "already_up_to_date".to_string(),
            dry_run: opts.dry_run,
            writes_performed: 0,
            added: 0,
            modified: 0,
            removed: 0,
            unchanged: 1 + new_lock.dependencies.len(),
            sha256_before: old_sha,
            sha256_after: Some(new_sha),
            backup_path: None,
            detail: "pblang.lock and project .pbck artifacts are already up to date".to_string(),
        });
    }

    let mut writes = 0usize;
    let mut bak_rel = None;
    if lock_changed && !opts.dry_run {
        let bak = write_atomic_with_backup(&lock_path, new_lock_bytes, 0o644, true)?;
        if bak.is_some() {
            bak_rel = Some("pblang.lock.bak".to_string());
        }
        writes += 1;
    }
    if !opts.dry_run {
        writes += pbck_refreshed;
    }

    Ok(TargetReceipt {
        target: label.to_string(),
        detected_kind: "project".to_string(),
        status: if opts.dry_run { "update_available" } else { "updated" }.to_string(),
        dry_run: opts.dry_run,
        writes_performed: writes,
        added: usize::from(old_bytes.is_none() && lock_changed),
        modified: usize::from(old_bytes.is_some() && lock_changed) + pbck_refreshed,
        removed: 0,
        unchanged: usize::from(!lock_changed),
        sha256_before: old_sha,
        sha256_after: Some(new_sha),
        backup_path: bak_rel,
        detail: format!(
            "lockfile={} refreshed_pbck={}",
            if lock_changed { "updated_relative_paths" } else { "unchanged" },
            pbck_refreshed
        ),
    })
}

/* ── 2. `.sapp` & `.pbck` Incremental Provenance Compiler ────────────────── */

fn compile_sapp_to_pbck_bytes(sapp_path: &Path, src: &str) -> Result<Vec<u8>, String> {
    let fname = sapp_path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("module.sapp");
    check_sapp_syntax_clean(src, fname)?;
    let (stmts, _) = pblang_core::parse_with_polyglot(src)
        .map_err(|errs| format!("parse error in '{fname}': {errs:?}"))?;
    let mut chunk = pblang_compiler::compile(stmts)
        .map_err(|e| format!("compile error in '{fname}': {e}"))?;
    chunk.source_name = Some(std::rc::Rc::new(fname.to_string()));
    Ok(pblang_ir::pbck_serialize(&chunk))
}

fn update_sapp_target(sapp_path: &Path, opts: &UpdateOpts) -> Result<TargetReceipt, String> {
    if !sapp_path.is_file() {
        return Err(format!(
            "source file '{}' does not exist",
            scrub_privacy_text(&sapp_path.display().to_string())
        ));
    }
    let pbck_path = sapp_path.with_extension("pbck");
    let meta_path = PathBuf::from(format!("{}.meta.json", pbck_path.display()));

    if opts.rollback {
        return rollback_single_file(&pbck_path, "sapp_bytecode", opts);
    }

    let src_bytes = fs::read(sapp_path).map_err(|e| format!("read source: {e}"))?;
    if let Some(ref exp) = opts.expected_sha256 {
        let got = sha256_bytes(&src_bytes);
        if !got.eq_ignore_ascii_case(exp) {
            return Err(format!("source SHA-256 mismatch: expected {exp}, got {got}"));
        }
    }
    let src_str = std::str::from_utf8(&src_bytes).map_err(|_| "source is not UTF-8".to_string())?;
    let src_sha = sha256_bytes(&src_bytes);
    let compiler_ver = env!("CARGO_PKG_VERSION").to_string();
    let rel_source_name = sapp_path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("source.sapp")
        .to_string();

    let old_pbck_sha = if pbck_path.is_file() {
        Some(sha256_bytes(&fs::read(&pbck_path).map_err(|e| e.to_string())?))
    } else {
        None
    };

    if let (Some(ref cur_pbck_sha), true) = (&old_pbck_sha, meta_path.is_file()) {
        if let Ok(meta_str) = fs::read_to_string(&meta_path) {
            if let Ok(meta) = serde_json::from_str::<PbckProvenanceMeta>(&meta_str) {
                if meta.source_sha256 == src_sha
                    && meta.compiler_version == compiler_ver
                    && &meta.pbck_sha256 == cur_pbck_sha
                {
                    return Ok(TargetReceipt {
                        target: rel_source_name,
                        detected_kind: "sapp_source".to_string(),
                        status: "already_up_to_date".to_string(),
                        dry_run: opts.dry_run,
                        writes_performed: 0,
                        added: 0,
                        modified: 0,
                        removed: 0,
                        unchanged: 1,
                        sha256_before: old_pbck_sha.clone(),
                        sha256_after: old_pbck_sha,
                        backup_path: None,
                        detail: "provenance source_sha256 + compiler_version + pbck_sha256 match (O(1) skip)"
                            .to_string(),
                    });
                }
            }
        }
    }

    let pbck_bytes = compile_sapp_to_pbck_bytes(sapp_path, src_str)?;
    let new_pbck_sha = sha256_bytes(&pbck_bytes);
    let meta = PbckProvenanceMeta {
        format: "pblang.pbck.meta.v1".to_string(),
        source: rel_source_name.clone(),
        source_sha256: src_sha,
        pbck_sha256: new_pbck_sha.clone(),
        compiler_version: compiler_ver,
    };
    let meta_json = serde_json::to_string_pretty(&meta).map_err(|e| e.to_string())?;

    let mut writes = 0usize;
    let mut bak_name = None;
    if !opts.dry_run {
        let bak = write_atomic_with_backup(&pbck_path, &pbck_bytes, 0o644, true)?;
        if let Some(bp) = bak {
            bak_name = bp.file_name().map(|s| s.to_string_lossy().to_string());
        }
        write_atomic_with_backup(&meta_path, meta_json.as_bytes(), 0o644, false)?;
        writes = 2;
    }

    Ok(TargetReceipt {
        target: rel_source_name,
        detected_kind: "sapp_source".to_string(),
        status: if opts.dry_run { "update_available" } else { "updated" }.to_string(),
        dry_run: opts.dry_run,
        writes_performed: writes,
        added: usize::from(old_pbck_sha.is_none()),
        modified: usize::from(old_pbck_sha.is_some()),
        removed: 0,
        unchanged: 0,
        sha256_before: old_pbck_sha,
        sha256_after: Some(new_pbck_sha),
        backup_path: bak_name,
        detail: "compiled .pbck v1 + scrubbed .pbck.meta.json provenance".to_string(),
    })
}

fn update_pbck_target(pbck_path: &Path, opts: &UpdateOpts) -> Result<TargetReceipt, String> {
    if opts.rollback {
        return rollback_single_file(pbck_path, "pbck_artifact", opts);
    }
    let meta_path = PathBuf::from(format!("{}.meta.json", pbck_path.display()));
    let parent = pbck_path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));

    if let Some(ref explicit_from) = opts.from {
        let sp = Path::new(explicit_from);
        if sp.extension().and_then(|s| s.to_str()) == Some("sapp") {
            return update_sapp_from_into_pbck(sp, pbck_path, opts);
        }
    }

    if meta_path.is_file() {
        if let Ok(meta_str) = fs::read_to_string(&meta_path) {
            if let Ok(meta) = serde_json::from_str::<PbckProvenanceMeta>(&meta_str) {
                let cand = parent.join(&meta.source);
                if cand.is_file() {
                    return update_sapp_target(&cand, opts);
                }
            }
        }
    }

    let sibling = pbck_path.with_extension("sapp");
    if sibling.is_file() {
        return update_sapp_target(&sibling, opts);
    }

    if !pbck_path.is_file() {
        return Err("target .pbck file does not exist and no source .sapp found".to_string());
    }
    let raw = fs::read(pbck_path).map_err(|e| format!("read .pbck: {e}"))?;
    pblang_ir::pbck_deserialize(&raw)?;
    let sha = sha256_bytes(&raw);
    Ok(TargetReceipt {
        target: pbck_path
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "artifact.pbck".to_string()),
        detected_kind: "pbck_artifact".to_string(),
        status: "already_up_to_date".to_string(),
        dry_run: opts.dry_run,
        writes_performed: 0,
        added: 0,
        modified: 0,
        removed: 0,
        unchanged: 1,
        sha256_before: Some(sha.clone()),
        sha256_after: Some(sha),
        backup_path: None,
        detail: "verified standalone .pbck v1 binary container integrity".to_string(),
    })
}

fn update_sapp_from_into_pbck(
    sapp_path: &Path,
    pbck_path: &Path,
    opts: &UpdateOpts,
) -> Result<TargetReceipt, String> {
    let src_bytes = fs::read(sapp_path).map_err(|e| format!("read --from .sapp: {e}"))?;
    let src_str = std::str::from_utf8(&src_bytes).map_err(|_| "source not UTF-8".to_string())?;
    let pbck_bytes = compile_sapp_to_pbck_bytes(sapp_path, src_str)?;
    let new_sha = sha256_bytes(&pbck_bytes);
    let old_sha = pbck_path
        .is_file()
        .then(|| fs::read(pbck_path).ok())
        .flatten()
        .map(|b| sha256_bytes(&b));
    if old_sha.as_deref() == Some(new_sha.as_str()) {
        return Ok(TargetReceipt {
            target: pbck_path.file_name().unwrap_or_default().to_string_lossy().to_string(),
            detected_kind: "pbck_artifact".to_string(),
            status: "already_up_to_date".to_string(),
            dry_run: opts.dry_run,
            writes_performed: 0,
            added: 0,
            modified: 0,
            removed: 0,
            unchanged: 1,
            sha256_before: old_sha,
            sha256_after: Some(new_sha),
            backup_path: None,
            detail: ".pbck already matches compiled --from source".to_string(),
        });
    }
    let meta_path = PathBuf::from(format!("{}.meta.json", pbck_path.display()));
    let meta = PbckProvenanceMeta {
        format: "pblang.pbck.meta.v1".to_string(),
        source: sapp_path.file_name().unwrap_or_default().to_string_lossy().to_string(),
        source_sha256: sha256_bytes(&src_bytes),
        pbck_sha256: new_sha.clone(),
        compiler_version: env!("CARGO_PKG_VERSION").to_string(),
    };
    let mut writes = 0usize;
    if !opts.dry_run {
        write_atomic_with_backup(pbck_path, &pbck_bytes, 0o644, true)?;
        write_atomic_with_backup(
            &meta_path,
            serde_json::to_string_pretty(&meta).unwrap().as_bytes(),
            0o644,
            false,
        )?;
        writes = 2;
    }
    Ok(TargetReceipt {
        target: pbck_path.file_name().unwrap_or_default().to_string_lossy().to_string(),
        detected_kind: "pbck_artifact".to_string(),
        status: if opts.dry_run { "update_available" } else { "updated" }.to_string(),
        dry_run: opts.dry_run,
        writes_performed: writes,
        added: usize::from(old_sha.is_none()),
        modified: usize::from(old_sha.is_some()),
        removed: 0,
        unchanged: 0,
        sha256_before: old_sha,
        sha256_after: Some(new_sha),
        backup_path: Some(format!(
            "{}.bak",
            pbck_path.file_name().unwrap_or_default().to_string_lossy()
        )),
        detail: "updated .pbck from explicit .sapp source".to_string(),
    })
}

fn rollback_single_file(target: &Path, kind: &str, opts: &UpdateOpts) -> Result<TargetReceipt, String> {
    let fname = target
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("artifact");
    let bak_path = target.with_file_name(format!("{fname}.bak"));
    if !bak_path.is_file() {
        return Err(format!("no backup '{fname}.bak' available for --rollback"));
    }
    let before = target
        .is_file()
        .then(|| fs::read(target).ok())
        .flatten()
        .map(|b| sha256_bytes(&b));
    let bak_bytes = fs::read(&bak_path).map_err(|e| format!("read backup: {e}"))?;
    let after = sha256_bytes(&bak_bytes);
    let is_exec = kind.contains("launcher") || kind.contains("elf");
    let mode = if is_exec { 0o755 } else { 0o644 };
    if !opts.dry_run {
        write_atomic_with_backup(target, &bak_bytes, mode, false)?;
    }
    Ok(TargetReceipt {
        target: fname.to_string(),
        detected_kind: kind.to_string(),
        status: if opts.dry_run { "rollback_ready" } else { "rolled_back" }.to_string(),
        dry_run: opts.dry_run,
        writes_performed: usize::from(!opts.dry_run),
        added: 0,
        modified: 1,
        removed: 0,
        unchanged: 0,
        sha256_before: before,
        sha256_after: Some(after),
        backup_path: Some(format!("{fname}.bak")),
        detail: format!("atomically restored {fname} from {fname}.bak"),
    })
}

/* ── 3. Compressed Package / Archive Updater (`.zip`, `.tar.gz`, `.tar`) ── */

fn update_archive_target(archive_path: &Path, opts: &UpdateOpts) -> Result<TargetReceipt, String> {
    let fmt = detect_archive_format(archive_path)
        .ok_or_else(|| "unsupported archive format".to_string())?;
    let fname = archive_path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("archive")
        .to_string();

    if opts.rollback {
        return rollback_single_file(archive_path, "archive", opts);
    }
    if !archive_path.is_file() {
        return Err(format!("archive '{}' does not exist", fname));
    }

    let raw_old = fs::read(archive_path).map_err(|e| format!("read archive: {e}"))?;
    let old_archive_sha = sha256_bytes(&raw_old);
    let mut cur_entries = read_archive_entries_safe(archive_path, fmt)?;

    // Validate any pblang.toml and .sapp files already inside archive
    validate_entries_syntax(&cur_entries)?;

    let (target_entries, detail_note) = if let Some(ref from_raw) = opts.from {
        let from_p = Path::new(from_raw);
        let (incoming, _) = load_source_tree_or_archive(
            from_p,
            opts.expected_sha256.as_deref(),
            opts.pubkey_hex.as_deref(),
        )?;
        validate_entries_syntax(&incoming)?;
        (incoming, "delta-synced archive from --from source")
    } else {
        // Refresh any stale .pbck inside the archive whose matching .sapp is present
        let mut updated_map = cur_entries.clone();
        let sapp_keys: Vec<String> = cur_entries
            .keys()
            .filter(|k| k.ends_with(".sapp"))
            .cloned()
            .collect();
        for sk in sapp_keys {
            let pbck_key = format!("{}.pbck", &sk[..sk.len() - 5]);
            if cur_entries.contains_key(&pbck_key) {
                let src_str = std::str::from_utf8(&cur_entries[&sk])
                    .map_err(|_| format!("archive entry '{sk}' is not UTF-8"))?;
                let fresh_pbck = compile_sapp_to_pbck_bytes(Path::new(&sk), src_str)?;
                updated_map.insert(pbck_key, fresh_pbck);
            }
        }
        (updated_map, "verified archive entries & refreshed internal .pbck artifacts")
    };

    let (added, modified, removed, unchanged) = diff_maps(&cur_entries, &target_entries);
    if added == 0 && modified == 0 && removed == 0 {
        return Ok(TargetReceipt {
            target: fname,
            detected_kind: "archive".to_string(),
            status: "already_up_to_date".to_string(),
            dry_run: opts.dry_run,
            writes_performed: 0,
            added: 0,
            modified: 0,
            removed: 0,
            unchanged,
            sha256_before: Some(old_archive_sha.clone()),
            sha256_after: Some(old_archive_sha),
            backup_path: None,
            detail: "all archive entry SHA-256 digests match (0 rewrites)".to_string(),
        });
    }

    let rebuilt = write_archive_entries(&target_entries, fmt)?;
    // Verify rebuilt archive can be re-read cleanly before touching original!
    let reread = match fmt {
        ArchiveFormat::Zip => read_zip_entries_safe(&rebuilt)?,
        ArchiveFormat::TarGz => {
            read_tar_stream_safe(GzDecoder::new(rebuilt.as_slice()), Some(rebuilt.len() as u64))?
        }
        ArchiveFormat::Tar => read_tar_stream_safe(rebuilt.as_slice(), None)?,
    };
    if reread.len() != target_entries.len() {
        return Err("rebuilt archive verification entry-count mismatch".to_string());
    }
    let new_archive_sha = sha256_bytes(&rebuilt);

    let mut writes = 0usize;
    let mut bak_rel = None;
    if !opts.dry_run {
        let bak = write_atomic_with_backup(archive_path, &rebuilt, 0o644, true)?;
        if let Some(bp) = bak {
            bak_rel = bp.file_name().map(|s| s.to_string_lossy().to_string());
        }
        writes = 1;
    }
    cur_entries.clear();

    Ok(TargetReceipt {
        target: fname,
        detected_kind: "archive".to_string(),
        status: if opts.dry_run { "update_available" } else { "updated" }.to_string(),
        dry_run: opts.dry_run,
        writes_performed: writes,
        added,
        modified,
        removed,
        unchanged,
        sha256_before: Some(old_archive_sha),
        sha256_after: Some(new_archive_sha),
        backup_path: bak_rel,
        detail: detail_note.to_string(),
    })
}

fn validate_entries_syntax(entries: &BTreeMap<String, Vec<u8>>) -> Result<(), String> {
    for (rel, bytes) in entries {
        if rel == "pblang.toml" || rel.ends_with("/pblang.toml") {
            let s = std::str::from_utf8(bytes).map_err(|_| format!("'{rel}' not UTF-8"))?;
            let _ = pblang_pm::parse_manifest(s)?;
        } else if rel.ends_with(".sapp") {
            let s = std::str::from_utf8(bytes).map_err(|_| format!("'{rel}' not UTF-8"))?;
            check_sapp_syntax_clean(s, rel)?;
        }
    }
    Ok(())
}

/* ── 4. Launcher Script, ELF Binary & Sealed App Directory Updaters ──────── */

fn update_launcher_script_target(script_path: &Path, opts: &UpdateOpts) -> Result<TargetReceipt, String> {
    if opts.rollback {
        return rollback_single_file(script_path, "launcher_script", opts);
    }
    let fname = script_path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("launcher")
        .to_string();
    let old_bytes = fs::read(script_path).map_err(|e| format!("read launcher: {e}"))?;
    let old_sha = sha256_bytes(&old_bytes);
    let old_str = std::str::from_utf8(&old_bytes).map_err(|_| "launcher is not UTF-8".to_string())?;

    let new_bytes = if let Some(ref from_raw) = opts.from {
        let fp = Path::new(from_raw);
        let b = fs::read(fp).map_err(|e| format!("read --from launcher: {e}"))?;
        if let Some(ref exp) = opts.expected_sha256 {
            let got = sha256_bytes(&b);
            if !got.eq_ignore_ascii_case(exp) {
                return Err(format!("launcher SHA-256 mismatch: expected {exp}, got {got}"));
            }
        }
        b
    } else {
        // Refresh release version reference inside launcher if an active release exists
        let install_root = resolve_install_root(opts)?;
        let active_ver = env!("CARGO_PKG_VERSION");
        let target_rel_dir = install_root.join("releases").join(active_ver);
        if target_rel_dir.is_dir() && old_str.contains("/releases/") {
            upgrade_release_version_in_script(old_str, active_ver).into_bytes()
        } else {
            old_bytes.clone()
        }
    };

    let new_sha = sha256_bytes(&new_bytes);
    if new_sha == old_sha {
        return Ok(TargetReceipt {
            target: fname,
            detected_kind: "launcher_script".to_string(),
            status: "already_up_to_date".to_string(),
            dry_run: opts.dry_run,
            writes_performed: 0,
            added: 0,
            modified: 0,
            removed: 0,
            unchanged: 1,
            sha256_before: Some(old_sha.clone()),
            sha256_after: Some(old_sha),
            backup_path: None,
            detail: "launcher script is already up to date".to_string(),
        });
    }

    #[cfg(unix)]
    let perm_mode = {
        use std::os::unix::fs::PermissionsExt;
        let cur = fs::metadata(script_path)
            .map(|m| m.permissions().mode() & 0o777)
            .unwrap_or(0o755);
        let safe = cur & !0o022;
        if (safe & 0o100) == 0 { 0o755 } else { safe }
    };
    #[cfg(not(unix))]
    let perm_mode = 0o755u32;

    let mut writes = 0usize;
    if !opts.dry_run {
        write_atomic_with_backup(script_path, &new_bytes, perm_mode, true)?;
        writes = 1;
    }
    Ok(TargetReceipt {
        target: fname.clone(),
        detected_kind: "launcher_script".to_string(),
        status: if opts.dry_run { "update_available" } else { "updated" }.to_string(),
        dry_run: opts.dry_run,
        writes_performed: writes,
        added: 0,
        modified: 1,
        removed: 0,
        unchanged: 0,
        sha256_before: Some(old_sha),
        sha256_after: Some(new_sha),
        backup_path: Some(format!("{fname}.bak")),
        detail: "atomically upgraded launcher script preserving safe owner permissions".to_string(),
    })
}

fn upgrade_release_version_in_script(script: &str, new_ver: &str) -> String {
    let marker = "/releases/";
    let mut out = String::with_capacity(script.len());
    let mut rest = script;
    while let Some(idx) = rest.find(marker) {
        out.push_str(&rest[..idx + marker.len()]);
        let after = &rest[idx + marker.len()..];
        if let Some(slash) = after.find('/') {
            out.push_str(new_ver);
            rest = &after[slash..];
        } else {
            out.push_str(after);
            rest = "";
        }
    }
    out.push_str(rest);
    out
}

fn update_elf_binary_target(bin_path: &Path, opts: &UpdateOpts) -> Result<TargetReceipt, String> {
    if opts.rollback {
        return rollback_single_file(bin_path, "elf_executable", opts);
    }
    let fname = bin_path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("binary")
        .to_string();
    let old_bytes = fs::read(bin_path).map_err(|e| format!("read ELF binary: {e}"))?;
    let old_sha = sha256_bytes(&old_bytes);

    let new_elf_bytes = if let Some(ref from_raw) = opts.from {
        let fp = Path::new(from_raw);
        if fp.extension().and_then(|s| s.to_str()) == Some("sapp") {
            compile_sapp_to_native_elf_bytes(fp)?
        } else {
            let b = fs::read(fp).map_err(|e| format!("read --from binary: {e}"))?;
            if let Some(ref exp) = opts.expected_sha256 {
                let got = sha256_bytes(&b);
                if !got.eq_ignore_ascii_case(exp) {
                    return Err(format!("ELF SHA-256 mismatch: expected {exp}, got {got}"));
                }
            }
            if let Some(ref pk) = opts.pubkey_hex {
                let sig_p = PathBuf::from(format!("{}.sig", fp.display()));
                verify_ed25519_signature(&b, pk, &sig_p)?;
            }
            b
        }
    } else {
        let sibling_sapp = bin_path.with_extension("sapp");
        if sibling_sapp.is_file() {
            compile_sapp_to_native_elf_bytes(&sibling_sapp)?
        } else {
            return Ok(TargetReceipt {
                target: fname,
                detected_kind: "elf_executable".to_string(),
                status: "already_up_to_date".to_string(),
                dry_run: opts.dry_run,
                writes_performed: 0,
                added: 0,
                modified: 0,
                removed: 0,
                unchanged: 1,
                sha256_before: Some(old_sha.clone()),
                sha256_after: Some(old_sha),
                backup_path: None,
                detail: "verified ELF64 binary header integrity".to_string(),
            });
        }
    };

    if new_elf_bytes.len() < 64 || &new_elf_bytes[0..4] != b"\x7fELF" {
        return Err("refusing to install non-ELF replacement into ELF executable target".to_string());
    }
    let new_sha = sha256_bytes(&new_elf_bytes);
    if new_sha == old_sha {
        return Ok(TargetReceipt {
            target: fname,
            detected_kind: "elf_executable".to_string(),
            status: "already_up_to_date".to_string(),
            dry_run: opts.dry_run,
            writes_performed: 0,
            added: 0,
            modified: 0,
            removed: 0,
            unchanged: 1,
            sha256_before: Some(old_sha.clone()),
            sha256_after: Some(old_sha),
            backup_path: None,
            detail: "ELF executable SHA-256 already matches replacement".to_string(),
        });
    }

    let mut writes = 0usize;
    if !opts.dry_run {
        write_atomic_with_backup(bin_path, &new_elf_bytes, 0o755, true)?;
        writes = 1;
    }
    Ok(TargetReceipt {
        target: fname.clone(),
        detected_kind: "elf_executable".to_string(),
        status: if opts.dry_run { "update_available" } else { "updated" }.to_string(),
        dry_run: opts.dry_run,
        writes_performed: writes,
        added: 0,
        modified: 1,
        removed: 0,
        unchanged: 0,
        sha256_before: Some(old_sha),
        sha256_after: Some(new_sha),
        backup_path: Some(format!("{fname}.bak")),
        detail: "ETXTBSY-safe atomic ELF64 replacement completed".to_string(),
    })
}

fn compile_sapp_to_native_elf_bytes(sapp_path: &Path) -> Result<Vec<u8>, String> {
    let src = fs::read_to_string(sapp_path).map_err(|e| format!("read .sapp: {e}"))?;
    check_sapp_syntax_clean(&src, "elf_source.sapp")?;
    let output = pblang_ctranspiler::transpile_to_c(&src)
        .map_err(|e| format!("C99 transpile failed: {e}"))?;
    let tmp_dir = tempfile::tempdir().map_err(|e| format!("tempdir: {e}"))?;
    let c_path = tmp_dir.path().join("prog.c");
    let out_path = tmp_dir.path().join("prog.elf");
    fs::write(&c_path, &output.c_source).map_err(|e| format!("write temp C: {e}"))?;
    let st = Command::new("gcc")
        .args(["-O3", "-std=c99"])
        .arg(&c_path)
        .arg("-lm")
        .arg("-o")
        .arg(&out_path)
        .output()
        .map_err(|e| format!("gcc spawn failed: {e}"))?;
    if !st.status.success() {
        return Err(format!(
            "gcc native build failed: {}",
            String::from_utf8_lossy(&st.stderr)
        ));
    }
    fs::read(&out_path).map_err(|e| format!("read compiled ELF: {e}"))
}

fn update_sealed_app_dir_target(app_dir: &Path, opts: &UpdateOpts) -> Result<TargetReceipt, String> {
    let label = app_dir
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("sealed_app")
        .to_string();
    let cur_files = collect_dir_files_safe(app_dir)?;
    let old_digest = compute_tree_digest(&cur_files);

    if let Some(ref from_raw) = opts.from {
        let (incoming, new_digest) = load_source_tree_or_archive(
            Path::new(from_raw),
            opts.expected_sha256.as_deref(),
            opts.pubkey_hex.as_deref(),
        )?;
        validate_entries_syntax(&incoming)?;
        let (added, modified, removed, unchanged) = diff_maps(&cur_files, &incoming);
        if added == 0 && modified == 0 && removed == 0 {
            return Ok(TargetReceipt {
                target: label,
                detected_kind: "sealed_app".to_string(),
                status: "already_up_to_date".to_string(),
                dry_run: opts.dry_run,
                writes_performed: 0,
                added: 0,
                modified: 0,
                removed: 0,
                unchanged,
                sha256_before: Some(old_digest.clone()),
                sha256_after: Some(old_digest),
                backup_path: None,
                detail: "sealed application tree matches source SHA-256".to_string(),
            });
        }
        let mut writes = 0usize;
        if !opts.dry_run {
            atomic_swap_directory_tree(app_dir, &incoming, true)?;
            writes = added + modified + 1;
        }
        return Ok(TargetReceipt {
            target: label,
            detected_kind: "sealed_app".to_string(),
            status: if opts.dry_run { "update_available" } else { "updated" }.to_string(),
            dry_run: opts.dry_run,
            writes_performed: writes,
            added,
            modified,
            removed,
            unchanged,
            sha256_before: Some(old_digest),
            sha256_after: Some(new_digest),
            backup_path: Some(format!("{}.bak", app_dir.file_name().unwrap_or_default().to_string_lossy())),
            detail: "atomically updated sealed application directory & MANIFEST.sha256.json".to_string(),
        });
    }

    let man_path = app_dir.join("MANIFEST.sha256.json");
    let manifest = build_tree_manifest(&cur_files, &old_digest);
    let man_bytes = serde_json::to_vec_pretty(&manifest).map_err(|e| e.to_string())?;
    let existing_same = man_path
        .is_file()
        .then(|| fs::read(&man_path).ok())
        .flatten()
        .is_some_and(|b| b == man_bytes);
    if existing_same {
        return Ok(TargetReceipt {
            target: label,
            detected_kind: "sealed_app".to_string(),
            status: "already_up_to_date".to_string(),
            dry_run: opts.dry_run,
            writes_performed: 0,
            added: 0,
            modified: 0,
            removed: 0,
            unchanged: cur_files.len(),
            sha256_before: Some(old_digest.clone()),
            sha256_after: Some(old_digest),
            backup_path: None,
            detail: "verified MANIFEST.sha256.json integrity".to_string(),
        });
    }
    if !opts.dry_run {
        write_atomic_with_backup(&man_path, &man_bytes, 0o644, false)?;
    }
    Ok(TargetReceipt {
        target: label,
        detected_kind: "sealed_app".to_string(),
        status: if opts.dry_run { "update_available" } else { "updated" }.to_string(),
        dry_run: opts.dry_run,
        writes_performed: usize::from(!opts.dry_run),
        added: 1,
        modified: 0,
        removed: 0,
        unchanged: cur_files.len(),
        sha256_before: Some(old_digest.clone()),
        sha256_after: Some(old_digest),
        backup_path: None,
        detail: "refreshed MANIFEST.sha256.json".to_string(),
    })
}

/* ── 5. `pblang update stdlib` ───────────────────────────────────────────── */

fn resolve_install_root(opts: &UpdateOpts) -> Result<PathBuf, String> {
    if let Some(ref r) = opts.install_root {
        return Ok(r.clone());
    }
    if let Ok(env_r) = std::env::var("PBLANG_INSTALL_ROOT") {
        if !env_r.trim().is_empty() {
            return Ok(PathBuf::from(env_r));
        }
    }
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map_err(|_| "HOME environment variable is not set".to_string())?;
    Ok(PathBuf::from(home).join(".local").join("share").join("pblang"))
}

fn resolve_bin_dir(opts: &UpdateOpts) -> Result<PathBuf, String> {
    if let Some(ref b) = opts.bin_dir {
        return Ok(b.clone());
    }
    if let Ok(env_b) = std::env::var("PBLANG_BIN_DIR") {
        if !env_b.trim().is_empty() {
            return Ok(PathBuf::from(env_b));
        }
    }
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map_err(|_| "HOME environment variable is not set".to_string())?;
    Ok(PathBuf::from(home).join(".local").join("bin"))
}

fn build_tree_manifest(files: &BTreeMap<String, Vec<u8>>, tree_sha: &str) -> StdlibManifest {
    let mut file_hashes = BTreeMap::new();
    for (rel, data) in files {
        file_hashes.insert(rel.clone(), sha256_bytes(data));
    }
    StdlibManifest {
        format: "pblang.stdlib.manifest.v1".to_string(),
        compiler_version: env!("CARGO_PKG_VERSION").to_string(),
        tree_sha256: tree_sha.to_string(),
        file_count: file_hashes.len(),
        files: file_hashes,
    }
}

fn atomic_swap_directory_tree(
    dest_dir: &Path,
    new_files: &BTreeMap<String, Vec<u8>>,
    write_manifest: bool,
) -> Result<Option<PathBuf>, String> {
    let parent = dest_dir
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).map_err(|e| format!("create parent dir: {e}"))?;
    let dname = dest_dir
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("stdlib");

    let staging = parent.join(format!(
        "{dname}.staging.{}.{:x}",
        std::process::id(),
        unique_nonce()
    ));
    if staging.exists() {
        let _ = fs::remove_dir_all(&staging);
    }
    fs::create_dir_all(&staging).map_err(|e| format!("create staging dir: {e}"))?;
    set_safe_unix_mode(&staging, 0o700)?;

    let write_res = (|| -> Result<(), String> {
        for (rel, bytes) in new_files {
            let clean = validate_archive_entry_rel(rel)?;
            let out_p = staging.join(&clean);
            if let Some(op) = out_p.parent() {
                fs::create_dir_all(op).map_err(|e| format!("mkdir '{}': {e}", clean))?;
                set_safe_unix_mode(op, 0o755)?;
            }
            let mut oo = OpenOptions::new();
            oo.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                oo.mode(0o644);
            }
            let mut f = oo.open(&out_p).map_err(|e| format!("create '{clean}': {e}"))?;
            f.write_all(bytes).map_err(|e| format!("write '{clean}': {e}"))?;
        }
        if write_manifest {
            let digest = compute_tree_digest(new_files);
            let man = build_tree_manifest(new_files, &digest);
            let json = serde_json::to_vec_pretty(&man).map_err(|e| e.to_string())?;
            fs::write(staging.join("MANIFEST.sha256.json"), json)
                .map_err(|e| format!("write MANIFEST.sha256.json: {e}"))?;
            set_safe_unix_mode(&staging.join("MANIFEST.sha256.json"), 0o644)?;
        }
        set_safe_unix_mode(&staging, 0o755)?;
        Ok(())
    })();

    if let Err(e) = write_res {
        let _ = fs::remove_dir_all(&staging);
        return Err(e);
    }

    let bak_dir = parent.join(format!("{dname}.bak"));
    let mut had_backup = None;
    if dest_dir.exists() {
        if bak_dir.exists() {
            fs::remove_dir_all(&bak_dir)
                .map_err(|e| format!("clean previous backup dir: {e}"))?;
        }
        fs::rename(dest_dir, &bak_dir).map_err(|e| {
            let _ = fs::remove_dir_all(&staging);
            format!("atomic move existing tree to .bak failed: {e}")
        })?;
        let _ = set_safe_unix_mode(&bak_dir, 0o700);
        had_backup = Some(bak_dir.clone());
    }

    if let Err(e) = fs::rename(&staging, dest_dir) {
        if let Some(ref b) = had_backup {
            let _ = fs::rename(b, dest_dir);
        }
        let _ = fs::remove_dir_all(&staging);
        return Err(format!("atomic swap staging -> target failed: {e}"));
    }
    Ok(had_backup)
}

fn update_stdlib_target(opts: &UpdateOpts) -> Result<TargetReceipt, String> {
    let install_root = resolve_install_root(opts)?;
    let ver = env!("CARGO_PKG_VERSION");
    let dest_stdlib = install_root.join("releases").join(ver).join("stdlib");

    if opts.rollback {
        let bak_dir = dest_stdlib.with_file_name("stdlib.bak");
        if !bak_dir.is_dir() {
            return Err("no stdlib.bak directory available for --rollback".to_string());
        }
        let bak_files = collect_dir_files_safe(&bak_dir)?;
        validate_entries_syntax(&bak_files)?;
        let after_sha = compute_tree_digest(&bak_files);
        let before_sha = dest_stdlib
            .is_dir()
            .then(|| collect_dir_files_safe(&dest_stdlib).ok())
            .flatten()
            .map(|m| compute_tree_digest(&m));

        if !opts.dry_run {
            let tmp_old = dest_stdlib.with_file_name(format!("stdlib.pre_rollback.{}", std::process::id()));
            if dest_stdlib.exists() {
                fs::rename(&dest_stdlib, &tmp_old).map_err(|e| format!("move current stdlib: {e}"))?;
            }
            set_safe_unix_mode(&bak_dir, 0o755)?;
            if let Err(e) = fs::rename(&bak_dir, &dest_stdlib) {
                if tmp_old.exists() {
                    let _ = fs::rename(&tmp_old, &dest_stdlib);
                }
                return Err(format!("restore stdlib.bak failed: {e}"));
            }
            if tmp_old.exists() {
                let _ = set_safe_unix_mode(&tmp_old, 0o700);
                let _ = fs::rename(&tmp_old, &bak_dir);
            }
        }
        return Ok(TargetReceipt {
            target: "stdlib".to_string(),
            detected_kind: "stdlib".to_string(),
            status: if opts.dry_run { "rollback_ready" } else { "rolled_back" }.to_string(),
            dry_run: opts.dry_run,
            writes_performed: usize::from(!opts.dry_run),
            added: 0,
            modified: bak_files.len(),
            removed: 0,
            unchanged: 0,
            sha256_before: before_sha,
            sha256_after: Some(after_sha),
            backup_path: Some("stdlib.bak".to_string()),
            detail: "atomically rolled back stdlib from stdlib.bak".to_string(),
        });
    }

    let source_path = if let Some(ref f) = opts.from {
        PathBuf::from(f)
    } else if Path::new("stdlib").is_dir() {
        PathBuf::from("stdlib")
    } else {
        return Err("specify --from <source_stdlib_or_archive> (or run from a pblang checkout containing ./stdlib)".to_string());
    };

    let (incoming_files, new_tree_sha) = load_source_tree_or_archive(
        &source_path,
        opts.expected_sha256.as_deref(),
        opts.pubkey_hex.as_deref(),
    )?;
    if incoming_files.is_empty() {
        return Err("refusing to update stdlib from an empty source tree".to_string());
    }

    // Syntax-check EVERY incoming .sapp file BEFORE staging so corrupt code never poisons stdlib!
    for (rel, bytes) in &incoming_files {
        if rel.ends_with(".sapp") {
            let src = std::str::from_utf8(bytes)
                .map_err(|_| format!("incoming stdlib file '{rel}' is not valid UTF-8"))?;
            check_sapp_syntax_clean(src, rel)?;
        }
    }

    let cur_files = if dest_stdlib.is_dir() {
        collect_dir_files_safe(&dest_stdlib)?
    } else {
        BTreeMap::new()
    };
    let old_tree_sha = (!cur_files.is_empty()).then(|| compute_tree_digest(&cur_files));
    let (added, modified, removed, unchanged) = diff_maps(&cur_files, &incoming_files);

    if old_tree_sha.as_deref() == Some(new_tree_sha.as_str())
        && dest_stdlib.join("MANIFEST.sha256.json").is_file()
    {
        return Ok(TargetReceipt {
            target: "stdlib".to_string(),
            detected_kind: "stdlib".to_string(),
            status: "already_up_to_date".to_string(),
            dry_run: opts.dry_run,
            writes_performed: 0,
            added: 0,
            modified: 0,
            removed: 0,
            unchanged,
            sha256_before: old_tree_sha,
            sha256_after: Some(new_tree_sha),
            backup_path: None,
            detail: "stdlib root tree SHA-256 matches installed MANIFEST.sha256.json (0 writes)"
                .to_string(),
        });
    }

    let mut writes = 0usize;
    let mut bak_rel = None;
    if !opts.dry_run {
        let bak = atomic_swap_directory_tree(&dest_stdlib, &incoming_files, true)?;
        if bak.is_some() {
            bak_rel = Some("stdlib.bak".to_string());
        }
        writes = added + modified + 1;
    }

    Ok(TargetReceipt {
        target: "stdlib".to_string(),
        detected_kind: "stdlib".to_string(),
        status: if opts.dry_run { "update_available" } else { "updated" }.to_string(),
        dry_run: opts.dry_run,
        writes_performed: writes,
        added,
        modified,
        removed,
        unchanged,
        sha256_before: old_tree_sha,
        sha256_after: Some(new_tree_sha),
        backup_path: bak_rel,
        detail: format!(
            "syntax-verified {} stdlib files and committed atomic delta swap",
            incoming_files.len()
        ),
    })
}

/* ── 6. `pblang update self` (ETXTBSY-Safe Runtime + Wrapper + Stdlib) ───── */

fn render_release_bin_wrapper() -> String {
    "#!/bin/sh\n\
     # Linux exec/argv bridge only. Admission policy lives in the sandboxed PBLang guard.\n\
     # Installed layout: bin/pblang, bin/pblang-native, libexec/native_bundle_guard.sapp.\n\
     launcher=$(/usr/bin/readlink -f -- \"$0\") || exit 126\n\
     bindir=${launcher%/*}\n\
     native=$bindir/pblang-native\n\
     guard=$bindir/../libexec/native_bundle_guard.sapp\n\
     if [ ! -x \"$native\" ] || [ ! -f \"$guard\" ]; then\n\
         printf '%s\\n' 'pblang launcher: native runtime or admission guard is missing' >&2\n\
         exit 126\n\
     fi\n\
     # The guard cannot consume the application stdin. An empty/damaged guard must\n\
     # not authorize dispatch merely by exiting successfully without a decision.\n\
     if admission=$(\"$native\" run \"$guard\" --sandbox -- pblang-launch-argv-v2 \"$@\" </dev/null); then\n\
         case \"$admission\" in\n\
             pblang-launch-admit-v1) ;;\n\
             pblang-launch-check-v1) exec \"$native\" run \"$guard\" \"$@\" ;;\n\
             pblang-launch-check-usage-v1)\n\
                 printf '%s\\n' 'pblang --check: expected --check --via-pblang (or --self-host), supported capability options, optional --retained-source compatibility preflight, optional --bounded-source=BYTES (1..1048576; Linux no-follow reader requiring explicit --ffi-lib/--ffi-symbol grants), and exactly one filename; use -- before a filename starting with --' >&2\n\
                 exit 2 ;;\n\
             *)\n\
                 printf '%s\\n' 'pblang launcher: no valid admission decision' >&2\n\
                 exit 126 ;;\n\
         esac\n\
     else\n\
         exit \"$?\"\n\
     fi\n\
     # Preserve argument boundaries, environment, cwd, streams and native exit status.\n\
     exec \"$native\" \"$@\"\n"
        .to_string()
}

fn render_user_launcher_script(ver: &str) -> String {
    format!(
        "#!/bin/sh\n\
         # Installed PBLang {ver} launcher\n\
         REAL_PBLANG=\"${{PBLANG_INSTALL_ROOT:-$HOME/.local/share/pblang}}/releases/{ver}/bin/pblang\"\n\
         if [ -z \"${{PBAI_HOME:-}}\" ] && [ -d \"$HOME/.local/share/pbai\" ]; then\n\
             export PBAI_HOME=\"$HOME/.local/share/pbai\"\n\
         fi\n\
         if [ \"$#\" -ge 2 ] && {{ [ \"$1\" = \"run\" ] || [ \"$1\" = \"test\" ]; }}; then\n\
             cmd=\"$1\"\n\
             target=\"$2\"\n\
             if [ -f \"$target\" ] && [ ! -f \"$PWD/pblang.toml\" ]; then\n\
                 abs_target=$(/usr/bin/readlink -f -- \"$target\" 2>/dev/null || printf \"%s\" \"$target\")\n\
                 dir=${{abs_target%/*}}\n\
                 pkg_root=\"\"\n\
                 while [ -n \"$dir\" ] && [ \"$dir\" != \"/\" ]; do\n\
                     if [ -f \"$dir/pblang.toml\" ]; then\n\
                         pkg_root=\"$dir\"\n\
                         break\n\
                     fi\n\
                     dir=${{dir%/*}}\n\
                 done\n\
                 if [ -n \"$pkg_root\" ]; then\n\
                     rel_target=${{abs_target#\"$pkg_root\"/}}\n\
                     shift 2\n\
                     cd \"$pkg_root\" || exit 1\n\
                     exec \"$REAL_PBLANG\" \"$cmd\" \"$rel_target\" \"$@\"\n\
                 fi\n\
             fi\n\
         fi\n\
         exec \"$REAL_PBLANG\" \"$@\"\n"
    )
}

fn rewrite_or_render_user_launcher(existing: Option<&[u8]>, ver: &str) -> Vec<u8> {
    if let Some(raw) = existing {
        if let Ok(text) = std::str::from_utf8(raw) {
            let mut replaced = false;
            let mut out_lines = Vec::new();
            for line in text.split('\n') {
                if let Some(rest) = line.strip_prefix("REAL_PBLANG=") {
                    if let (Some(rel_idx), Some(bin_idx)) =
                        (rest.find("/releases/"), rest.find("/bin/pblang"))
                    {
                        if bin_idx > rel_idx + "/releases/".len() {
                            let prefix = &rest[..rel_idx + "/releases/".len()];
                            let suffix = &rest[bin_idx..];
                            out_lines.push(format!("REAL_PBLANG={prefix}{ver}{suffix}"));
                            replaced = true;
                            continue;
                        }
                    }
                }
                out_lines.push(line.to_string());
            }
            if replaced {
                return out_lines.join("\n").into_bytes();
            }
        }
    }
    render_user_launcher_script(ver).into_bytes()
}

fn update_self_target(opts: &UpdateOpts) -> Result<TargetReceipt, String> {
    let install_root = resolve_install_root(opts)?;
    let bin_dir = resolve_bin_dir(opts)?;
    let launcher_path = bin_dir.join("pblang");
    let prev_marker = install_root.join("ACTIVE_RELEASE.prev");

    if opts.rollback {
        let bak_launcher = bin_dir.join("pblang.bak");
        if !bak_launcher.is_file() {
            return Err("no pblang.bak launcher found for `pblang update self --rollback`".to_string());
        }
        let before = launcher_path
            .is_file()
            .then(|| fs::read(&launcher_path).ok())
            .flatten()
            .map(|b| sha256_bytes(&b));
        let bak_bytes = fs::read(&bak_launcher).map_err(|e| format!("read pblang.bak: {e}"))?;
        let after = sha256_bytes(&bak_bytes);
        if !opts.dry_run {
            write_atomic_with_backup(&launcher_path, &bak_bytes, 0o755, false)?;
            if prev_marker.is_file() {
                let prev_ver = fs::read_to_string(&prev_marker).unwrap_or_default();
                let _ = write_atomic_with_backup(
                    &install_root.join("ACTIVE_RELEASE"),
                    prev_ver.trim().as_bytes(),
                    0o644,
                    false,
                );
            }
        }
        return Ok(TargetReceipt {
            target: "self".to_string(),
            detected_kind: "self_toolchain".to_string(),
            status: if opts.dry_run { "rollback_ready" } else { "rolled_back" }.to_string(),
            dry_run: opts.dry_run,
            writes_performed: usize::from(!opts.dry_run),
            added: 0,
            modified: 1,
            removed: 0,
            unchanged: 0,
            sha256_before: before,
            sha256_after: Some(after),
            backup_path: Some("pblang.bak".to_string()),
            detail: "atomically rolled back active pblang launcher to previous release".to_string(),
        });
    }

    let source_root = if let Some(ref f) = opts.from {
        PathBuf::from(f)
    } else {
        std::env::current_dir().map_err(|e| format!("cwd: {e}"))?
    };

    // Determine native binary + stdlib source from repo checkout, release dir, or archive
    let temp_extracted;
    let effective_src_dir = if detect_archive_format(&source_root).is_some() {
        let (entries, _) = load_source_tree_or_archive(
            &source_root,
            opts.expected_sha256.as_deref(),
            opts.pubkey_hex.as_deref(),
        )?;
        temp_extracted = tempfile::tempdir().map_err(|e| format!("tempdir: {e}"))?;
        for (rel, data) in &entries {
            let p = temp_extracted.path().join(rel);
            if let Some(par) = p.parent() {
                fs::create_dir_all(par).map_err(|e| e.to_string())?;
            }
            fs::write(&p, data).map_err(|e| e.to_string())?;
        }
        temp_extracted.path().to_path_buf()
    } else {
        if let Some(ref pk) = opts.pubkey_hex {
            let sig_p = PathBuf::from(format!("{}.sig", source_root.display()));
            let man_p = source_root.join("MANIFEST.sha256.json");
            if man_p.is_file() && sig_p.is_file() {
                let mb = fs::read(&man_p).map_err(|e| e.to_string())?;
                verify_ed25519_signature(&mb, pk, &sig_p)?;
            }
        }
        source_root
    };

    let cand_native = [
        effective_src_dir.join("target/release/pblang"),
        effective_src_dir.join("bin/pblang-native"),
        effective_src_dir.join("bin/pblang"),
    ]
    .into_iter()
    .find(|p| p.is_file())
    .or_else(|| std::env::current_exe().ok())
    .ok_or_else(|| "could not locate replacement pblang-native binary in --from".to_string())?;

    let native_bytes = fs::read(&cand_native).map_err(|e| format!("read native binary: {e}"))?;
    if native_bytes.len() < 64 || &native_bytes[0..4] != b"\x7fELF" {
        return Err("replacement pblang-native binary failed ELF64 header check".to_string());
    }
    if let Some(ref exp) = opts.expected_sha256 {
        if detect_archive_format(Path::new(opts.from.as_deref().unwrap_or(""))).is_none() {
            let got = sha256_bytes(&native_bytes);
            if !got.eq_ignore_ascii_case(exp) {
                return Err(format!(
                    "expected SHA-256 mismatch on pblang-native: expected {exp}, got {got}"
                ));
            }
        }
    }

    let cand_zero_rust = [
        effective_src_dir.join("target/release/pblang-zero-rust"),
        effective_src_dir.join("bin/pblang-zero-rust"),
    ]
    .into_iter()
    .find(|p| p.is_file());

    let src_stdlib = effective_src_dir.join("stdlib");
    if !src_stdlib.is_dir() {
        return Err("replacement source must contain a valid stdlib/ directory".to_string());
    }
    let stdlib_files = collect_dir_files_safe(&src_stdlib)?;
    for (rel, bytes) in &stdlib_files {
        if rel.ends_with(".sapp") {
            let s = std::str::from_utf8(bytes)
                .map_err(|_| format!("stdlib '{rel}' not UTF-8"))?;
            check_sapp_syntax_clean(s, rel)?;
        }
    }

    // Smoke-test staged binary in an exclusive temporary inode BEFORE installing
    let target_ver = detect_binary_version(&native_bytes, &install_root)?;
    let rel_dir = install_root.join("releases").join(&target_ver);
    let dest_native = rel_dir.join("bin/pblang-native");
    let new_native_sha = sha256_bytes(&native_bytes);
    let old_native_sha = dest_native
        .is_file()
        .then(|| fs::read(&dest_native).ok())
        .flatten()
        .map(|b| sha256_bytes(&b));

    let cur_stdlib_same = rel_dir.join("stdlib").is_dir()
        && collect_dir_files_safe(&rel_dir.join("stdlib"))
            .map(|m| compute_tree_digest(&m) == compute_tree_digest(&stdlib_files))
            .unwrap_or(false);
    let existing_launcher = launcher_path
        .is_file()
        .then(|| fs::read(&launcher_path).ok())
        .flatten();
    let wrapper_bytes = rewrite_or_render_user_launcher(existing_launcher.as_deref(), &target_ver);
    let cur_launcher_same = existing_launcher
        .as_deref()
        .is_some_and(|b| b == wrapper_bytes.as_slice());

    if old_native_sha.as_deref() == Some(new_native_sha.as_str())
        && cur_stdlib_same
        && cur_launcher_same
    {
        return Ok(TargetReceipt {
            target: "self".to_string(),
            detected_kind: "self_toolchain".to_string(),
            status: "already_up_to_date".to_string(),
            dry_run: opts.dry_run,
            writes_performed: 0,
            added: 0,
            modified: 0,
            removed: 0,
            unchanged: 3,
            sha256_before: old_native_sha,
            sha256_after: Some(new_native_sha),
            backup_path: None,
            detail: format!("pblang {target_ver} runtime, stdlib, and launcher are already up to date"),
        });
    }

    let mut writes = 0usize;
    if !opts.dry_run {
        fs::create_dir_all(rel_dir.join("bin")).map_err(|e| format!("mkdir bin: {e}"))?;
        fs::create_dir_all(rel_dir.join("libexec")).map_err(|e| format!("mkdir libexec: {e}"))?;

        // Record previous active release if any
        let active_file = install_root.join("ACTIVE_RELEASE");
        if active_file.is_file() {
            if let Ok(prev) = fs::read(&active_file) {
                let _ = write_atomic_with_backup(&prev_marker, &prev, 0o600, false);
            }
        }

        // ETXTBSY-safe atomic install of pblang-native (new O_CREAT|O_EXCL inode + rename)
        write_atomic_with_backup(&dest_native, &native_bytes, 0o755, true)?;
        writes += 1;

        if let Some(zr_path) = cand_zero_rust {
            let zr_bytes = fs::read(&zr_path).map_err(|e| format!("read pblang-zero-rust: {e}"))?;
            if zr_bytes.starts_with(b"\x7fELF") {
                write_atomic_with_backup(&rel_dir.join("bin/pblang-zero-rust"), &zr_bytes, 0o755, true)?;
                writes += 1;
            }
        }

        let guard_src = [
            effective_src_dir.join("tools/native_bundle_guard.sapp"),
            effective_src_dir.join("libexec/native_bundle_guard.sapp"),
        ]
        .into_iter()
        .find(|p| p.is_file());
        if let Some(gp) = guard_src {
            let gb = fs::read(&gp).map_err(|e| e.to_string())?;
            write_atomic_with_backup(&rel_dir.join("libexec/native_bundle_guard.sapp"), &gb, 0o644, false)?;
            writes += 1;
        }

        let rel_wrapper = render_release_bin_wrapper();
        write_atomic_with_backup(&rel_dir.join("bin/pblang"), rel_wrapper.as_bytes(), 0o755, false)?;
        writes += 1;

        atomic_swap_directory_tree(&rel_dir.join("stdlib"), &stdlib_files, true)?;
        writes += 1;

        for extra_dir in ["tools", "runtime_stage3", "docs", "examples", "tests"] {
            let sd = effective_src_dir.join(extra_dir);
            if sd.is_dir() {
                if let Ok(extra_files) = collect_dir_files_safe(&sd) {
                    if !extra_files.is_empty() {
                        let _ = atomic_swap_directory_tree(&rel_dir.join(extra_dir), &extra_files, false);
                    }
                }
            }
        }
        let main_sapp = effective_src_dir.join("src/main.sapp");
        if main_sapp.is_file() {
            fs::create_dir_all(rel_dir.join("src")).map_err(|e| format!("mkdir src: {e}"))?;
            let mb = fs::read(&main_sapp).map_err(|e| e.to_string())?;
            write_atomic_with_backup(&rel_dir.join("src/main.sapp"), &mb, 0o644, false)?;
        }

        for meta_file in [
            "pblang.toml",
            "pblang.lock",
            "README.md",
            "CHANGELOG.md",
            "LICENSE",
            "SOURCE_REVISION",
        ] {
            let mp = effective_src_dir.join(meta_file);
            if mp.is_file() {
                let mb = fs::read(&mp).map_err(|e| e.to_string())?;
                write_atomic_with_backup(&rel_dir.join(meta_file), &mb, 0o644, false)?;
            }
        }

        fs::create_dir_all(&bin_dir).map_err(|e| format!("mkdir bin_dir: {e}"))?;
        write_atomic_with_backup(&launcher_path, &wrapper_bytes, 0o755, true)?;
        write_atomic_with_backup(&active_file, target_ver.as_bytes(), 0o644, false)?;
        writes += 2;
    }

    Ok(TargetReceipt {
        target: "self".to_string(),
        detected_kind: "self_toolchain".to_string(),
        status: if opts.dry_run { "update_available" } else { "updated" }.to_string(),
        dry_run: opts.dry_run,
        writes_performed: writes,
        added: usize::from(old_native_sha.is_none()),
        modified: usize::from(old_native_sha.is_some()) + 2,
        removed: 0,
        unchanged: 0,
        sha256_before: old_native_sha,
        sha256_after: Some(new_native_sha),
        backup_path: Some("pblang.bak".to_string()),
        detail: format!(
            "ETXTBSY-safe atomic self-update to release {} ({} stdlib files verified)",
            target_ver,
            stdlib_files.len()
        ),
    })
}

fn detect_binary_version(native_bytes: &[u8], install_root: &Path) -> Result<String, String> {
    fs::create_dir_all(install_root).map_err(|e| format!("create install_root: {e}"))?;
    let (probe_path, mut pf) = create_exclusive_temp_in(install_root, "pblang-self-probe", 0o700)?;
    pf.write_all(native_bytes)
        .and_then(|_| pf.sync_all())
        .map_err(|e| {
            let _ = fs::remove_file(&probe_path);
            format!("write self-test probe binary: {e}")
        })?;
    drop(pf);
    set_safe_unix_mode(&probe_path, 0o700)?;

    let out = Command::new(&probe_path).arg("--version").output();
    let _ = fs::remove_file(&probe_path);
    let out = out.map_err(|e| format!("self-test execution of staged binary failed: {e}"))?;
    if !out.status.success() {
        return Err("staged pblang binary failed `--version` smoke self-test".to_string());
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let ver = text
        .split_whitespace()
        .nth(1)
        .unwrap_or(env!("CARGO_PKG_VERSION"))
        .trim();
    if ver.is_empty()
        || ver.contains('/')
        || ver.contains("..")
        || !ver
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-' || b == b'_')
    {
        return Err("staged binary reported invalid version identifier".to_string());
    }
    Ok(ver.to_string())
}
