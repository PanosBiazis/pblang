//! pblang-stub — E5-gamma self-extracting binary launcher for pblang bundles.
//!
//! When `pblang bundle --self-extract` is used, the resulting binary is:
//!
//!   [this stub ELF/PE/Mach-O] + [SENTINEL: b"PBLANG_BUNDLE_START\x00"] + [tar.gz bundle]
//!
//! On execution this stub:
//!   1. Reads its own executable bytes
//!   2. Finds the SENTINEL to locate the embedded tar.gz bundle
//!   3. Extracts the bundle to a secure temporary directory
//!   4. Runs `pblang run .` in that temp directory (forwarding all CLI args)
//!   5. Cleans up the temp directory on exit (best-effort via Drop)
//!
//! Special flags (handled before running the bundle):
//!   --extract-only <dir>   Extract the bundle to <dir> and exit (no run).
//!   --bundle-version       Print the embedded bundle's pblang.toml version and exit.
//!
//! If no embedded bundle is found the stub prints usage guidance and exits 1.

use flate2::read::GzDecoder;
use std::path::{Path, PathBuf};
use std::process;
use tar::Archive;

const SENTINEL: &[u8] = b"PBLANG_BUNDLE_START\x00";

// ── Temp directory RAII guard ─────────────────────────────────────────────────

struct TempDir(PathBuf);

impl Drop for TempDir {
    fn drop(&mut self) {
        // Best-effort removal; ignore errors (e.g. already removed, read-only FS).
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

// ── Core helpers ──────────────────────────────────────────────────────────────

/// Search (backwards from EOF) for SENTINEL and return the offset of the first
/// byte *after* it, i.e. where the tar.gz bundle starts.
///
/// We search from the end because the stub ELF/PE may itself contain the
/// string "PBLANG_BUNDLE_START" inside debug symbols; the *real* sentinel is
/// always the last occurrence before the bundle data.
fn find_bundle_offset(exe: &[u8]) -> Option<usize> {
    let slen = SENTINEL.len();
    if exe.len() < slen {
        return None;
    }
    // The embedded bundle is always a gzip stream (magic `1f 8b`). Requiring it
    // here means the SENTINEL constant that lives in this stub's own .rodata
    // (a template with no appended bundle) is not mistaken for one — so callers
    // can show the friendly "no embedded bundle" guidance instead of a cryptic
    // "invalid gzip header".
    const GZIP_MAGIC: [u8; 2] = [0x1f, 0x8b];
    for start in (0..=(exe.len() - slen)).rev() {
        if &exe[start..start + slen] == SENTINEL {
            let payload = start + slen;
            if exe.len() >= payload + 2 && exe[payload..payload + 2] == GZIP_MAGIC {
                return Some(payload);
            }
        }
    }
    None
}

/// Locate the `pblang` binary to use for running the extracted bundle.
/// Priority: (1) embedded `bin/pblang` inside the extracted bundle,
/// (2) same directory as this stub executable, (3) `PATH`.
fn find_pblang(stub_path: &Path, extract_dir: &Path) -> PathBuf {
    let bin_name = if cfg!(windows) {
        "pblang.exe"
    } else {
        "pblang"
    };
    let bundled = extract_dir.join("bin").join(bin_name);
    if bundled.is_file() {
        return bundled;
    }
    if let Some(dir) = stub_path.parent() {
        let candidate = dir.join(bin_name);
        if candidate.is_file() {
            return candidate;
        }
    }
    if let Ok(path_var) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path_var) {
            let candidate = dir.join(bin_name);
            if candidate.is_file() {
                return candidate;
            }
        }
    }
    PathBuf::from(bin_name)
}

/// Create a uniquely-named temporary directory under the system temp directory.
fn make_temp_dir() -> std::io::Result<PathBuf> {
    let base = std::env::temp_dir().join(format!(
        "pblang-bundle-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&base)?;
    Ok(base)
}

// ── Entry point ───────────────────────────────────────────────────────────────

fn main() {
    let args: Vec<String> = std::env::args().collect();

    // ── 1. Read own executable ────────────────────────────────────────────────
    let exe_path = match std::env::current_exe() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("pblang-stub: cannot determine executable path: {e}");
            process::exit(1);
        }
    };

    let exe_bytes = match std::fs::read(&exe_path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!(
                "pblang-stub: cannot read executable '{}': {e}",
                exe_path.display()
            );
            process::exit(1);
        }
    };

    // ── 2. Locate embedded bundle ─────────────────────────────────────────────
    let bundle_offset = match find_bundle_offset(&exe_bytes) {
        Some(off) => off,
        None => {
            eprintln!("pblang-stub: no embedded bundle found in this binary.");
            eprintln!("  This executable is a pblang self-extracting stub template.");
            eprintln!("  Create a self-extracting app with:");
            eprintln!("    pblang bundle --self-extract [<manifest-dir>] [--out <name>]");
            process::exit(1);
        }
    };

    let tarball = &exe_bytes[bundle_offset..];

    // ── 3. Parse special flags ────────────────────────────────────────────────

    // --bundle-version: print version from manifest and exit
    if args.iter().any(|a| a == "--bundle-version") {
        // Extract to temp, read pblang.toml, print version, clean up
        let tmp = make_temp_dir().unwrap_or_else(|e| {
            eprintln!("pblang-stub: cannot create temp dir: {e}");
            process::exit(1);
        });
        let _cleanup = TempDir(tmp.clone());
        extract_bundle(tarball, &tmp).unwrap_or_else(|e| {
            eprintln!("pblang-stub: {e}");
            process::exit(1);
        });

        let manifest = tmp.join("pblang.toml");
        if let Ok(contents) = std::fs::read_to_string(&manifest) {
            // Naively extract `version = "..."` line
            for line in contents.lines() {
                let line = line.trim();
                if line.starts_with("version") {
                    println!("{}", line);
                    return;
                }
            }
        }
        eprintln!("pblang-stub: could not read version from embedded pblang.toml");
        process::exit(1);
    }

    // --extract-only <dir>: just extract, print path, and exit
    let extract_only_idx = args.iter().position(|a| a == "--extract-only");
    if let Some(idx) = extract_only_idx {
        let dest = match args.get(idx + 1) {
            Some(d) => PathBuf::from(d),
            None => {
                eprintln!("pblang-stub: --extract-only requires a directory argument");
                process::exit(1);
            }
        };
        std::fs::create_dir_all(&dest).unwrap_or_else(|e| {
            eprintln!("pblang-stub: cannot create '{}': {e}", dest.display());
            process::exit(1);
        });
        extract_bundle(tarball, &dest).unwrap_or_else(|e| {
            eprintln!("pblang-stub: {e}");
            process::exit(1);
        });
        println!("{}", dest.display());
        return;
    }

    // ── 4. Extract bundle to temp directory ───────────────────────────────────
    let tmp = make_temp_dir().unwrap_or_else(|e| {
        eprintln!("pblang-stub: cannot create temp dir: {e}");
        process::exit(1);
    });
    let _cleanup = TempDir(tmp.clone()); // removed on Drop (even on panic)
    extract_bundle(tarball, &tmp).unwrap_or_else(|e| {
        eprintln!("pblang-stub: {e}");
        process::exit(1);
    });

    // ── 5. Find pblang and run the bundle ──────────────────────────────────────
    let pblang_bin = find_pblang(&exe_path, &tmp);

    // Forward args that come after `--`; everything before (stub flags) is dropped
    let extra_args: Vec<&str> = args
        .iter()
        .position(|a| a == "--")
        .map(|i| args[(i + 1)..].iter().map(String::as_str).collect())
        .unwrap_or_default();

    let mut cmd = process::Command::new(&pblang_bin);
    cmd.arg("run").arg(".").current_dir(&tmp);
    if !extra_args.is_empty() {
        cmd.arg("--");
        for a in &extra_args {
            cmd.arg(a);
        }
    }

    let status = cmd.status().unwrap_or_else(|e| {
        eprintln!("pblang-stub: failed to run '{}': {e}", pblang_bin.display());
        process::exit(1);
    });

    // _cleanup Drop fires here (before exit) thanks to Rust's drop order
    let code = status.code().unwrap_or(1);
    process::exit(code);
}

// ── Bundle extraction ─────────────────────────────────────────────────────────

/// Extract the embedded tar.gz `tarball` into `dest`, enforcing that nothing can
/// escape `dest`. Returns `Err` (never panics/exits) on any malformed or unsafe
/// entry so callers control the failure, and so the safety properties are unit
/// testable.
///
/// Defenses (defense-in-depth):
///   - reject absolute paths and any `..` component;
///   - **allow-list** entry types — only `Regular` files and `Directory`s are
///     extracted; symlinks, hardlinks, and every exotic type (devices, fifos,
///     etc.) are rejected (a deny-list would miss future/exotic types);
///   - verify the joined target stays under `dest`;
///   - `unpack_in`, which independently enforces containment.
fn extract_bundle(tarball: &[u8], dest: &Path) -> Result<(), String> {
    let dest_root = std::fs::canonicalize(dest).unwrap_or_else(|_| dest.to_path_buf());
    let gz = GzDecoder::new(std::io::Cursor::new(tarball));
    let mut archive = Archive::new(gz);
    let entries = archive
        .entries()
        .map_err(|e| format!("failed to read bundle entries: {e}"))?;
    for entry_res in entries {
        let mut entry = entry_res.map_err(|e| format!("failed to read a bundle entry: {e}"))?;
        let entry_path = entry
            .path()
            .map(|p| p.into_owned())
            .map_err(|e| format!("invalid entry path in bundle: {e}"))?;
        if entry_path.is_absolute()
            || entry_path
                .components()
                .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            return Err(format!(
                "refusing to extract unsafe bundle path '{}'",
                entry_path.display()
            ));
        }
        // Allow-list: only plain files and directories. Symlink/Link/Char/Block/
        // Fifo/Continuous/GNU* etc. are all rejected so a crafted bundle can't
        // create a link or device node.
        let entry_type = entry.header().entry_type();
        if !matches!(entry_type, tar::EntryType::Regular | tar::EntryType::Directory) {
            return Err(format!(
                "refusing to extract non-regular entry '{}' (type {:?})",
                entry_path.display(),
                entry_type
            ));
        }
        let target = dest_root.join(&entry_path);
        if !target.starts_with(&dest_root) {
            return Err(format!(
                "refusing to extract outside target dir: '{}'",
                entry_path.display()
            ));
        }
        entry
            .unpack_in(&dest_root)
            .map_err(|e| format!("failed to extract '{}': {e}", entry_path.display()))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::write::GzEncoder;
    use flate2::Compression;
    use std::io::Write;

    /// Write `val` as a right-justified, NUL-terminated octal string into `buf`.
    fn octal(buf: &mut [u8], val: u64) {
        let s = format!("{:0width$o}", val, width = buf.len() - 1);
        let b = s.as_bytes();
        let n = b.len().min(buf.len() - 1);
        buf[..n].copy_from_slice(&b[b.len() - n..]);
        buf[buf.len() - 1] = 0;
    }

    /// Build a raw ustar 512-byte header. We hand-craft it because the `tar`
    /// crate's Builder deliberately refuses to *create* unsafe archives (`..`,
    /// absolute paths) — but a real attacker uses GNU tar, which does. The whole
    /// point is to feed extract_bundle the kind of archive it must defend against.
    fn header(name: &str, typeflag: u8, size: u64, linkname: &str) -> [u8; 512] {
        let mut h = [0u8; 512];
        let nb = name.as_bytes();
        let nn = nb.len().min(100);
        h[..nn].copy_from_slice(&nb[..nn]);
        // Directories need the execute (search) bit or files can't be created
        // inside them once unpack applies the mode.
        let mode = if typeflag == b'5' { 0o755 } else { 0o644 };
        octal(&mut h[100..108], mode); // mode
        octal(&mut h[108..116], 0); // uid
        octal(&mut h[116..124], 0); // gid
        octal(&mut h[124..136], size); // size
        octal(&mut h[136..148], 0); // mtime
        h[156] = typeflag;
        let lb = linkname.as_bytes();
        let ln = lb.len().min(100);
        h[157..157 + ln].copy_from_slice(&lb[..ln]);
        h[257..263].copy_from_slice(b"ustar\0");
        h[263..265].copy_from_slice(b"00");
        // checksum: field treated as spaces while summing.
        for b in &mut h[148..156] {
            *b = b' ';
        }
        let sum: u32 = h.iter().map(|&b| b as u32).sum();
        let cks = format!("{sum:06o}");
        h[148..148 + cks.len()].copy_from_slice(cks.as_bytes());
        h[154] = 0;
        h[155] = b' ';
        h
    }

    /// `(name, typeflag, data, linkname)` → gzip'd raw tar bytes.
    fn raw_targz(entries: &[(&str, u8, &[u8], &str)]) -> Vec<u8> {
        let mut tar = Vec::new();
        for (name, tf, data, link) in entries {
            tar.extend_from_slice(&header(name, *tf, data.len() as u64, link));
            tar.extend_from_slice(data);
            let pad = (512 - (data.len() % 512)) % 512;
            tar.extend(std::iter::repeat(0u8).take(pad));
        }
        tar.extend(std::iter::repeat(0u8).take(1024)); // end-of-archive
        let mut gz = GzEncoder::new(Vec::new(), Compression::fast());
        gz.write_all(&tar).unwrap();
        gz.finish().unwrap()
    }

    const REG: u8 = b'0';
    const DIR: u8 = b'5';
    const SYM: u8 = b'2';
    const FIFO: u8 = b'6';

    fn tmp() -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "pbstub-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn find_bundle_offset_requires_gzip_magic() {
        // A bare stub template contains the SENTINEL constant in its own .rodata;
        // that must NOT be mistaken for an embedded bundle (which is always a
        // gzip stream, magic 1f 8b). Otherwise the friendly "no bundle" guidance
        // is unreachable and the user sees a confusing "invalid gzip header".
        let mut just_sentinel = b"prefix".to_vec();
        just_sentinel.extend_from_slice(SENTINEL);
        just_sentinel.extend_from_slice(&[0x00, 0x01, 0x02]); // not gzip
        assert_eq!(find_bundle_offset(&just_sentinel), None);

        let mut real = b"stub-bytes".to_vec();
        real.extend_from_slice(SENTINEL);
        let payload_at = real.len();
        real.extend_from_slice(&[0x1f, 0x8b, 0x08, 0x00]); // gzip magic + header
        assert_eq!(find_bundle_offset(&real), Some(payload_at));
    }

    #[test]
    fn extracts_a_normal_bundle() {
        let dest = tmp();
        let gz = raw_targz(&[
            ("pblang.toml", REG, b"version = \"1.0\"\n", ""),
            ("sub/", DIR, b"", ""),
            ("sub/main.sapp", REG, b"print 1;\n", ""),
        ]);
        extract_bundle(&gz, &dest).expect("normal bundle extracts");
        assert_eq!(
            std::fs::read(dest.join("pblang.toml")).unwrap(),
            b"version = \"1.0\"\n"
        );
        assert_eq!(
            std::fs::read(dest.join("sub/main.sapp")).unwrap(),
            b"print 1;\n"
        );
        let _ = std::fs::remove_dir_all(&dest);
    }

    #[test]
    fn rejects_parent_dir_traversal() {
        let dest = tmp();
        let gz = raw_targz(&[("../escape.txt", REG, b"pwned", "")]);
        assert!(extract_bundle(&gz, &dest).is_err(), "`..` path must be rejected");
        assert!(
            !dest.parent().unwrap().join("escape.txt").exists(),
            "no file escaped via .."
        );
        let _ = std::fs::remove_dir_all(&dest);
    }

    #[test]
    fn rejects_absolute_path() {
        let dest = tmp();
        let evil = std::env::temp_dir().join("pbstub_abs_escape.txt");
        let _ = std::fs::remove_file(&evil);
        let gz = raw_targz(&[(evil.to_str().unwrap(), REG, b"pwned", "")]);
        assert!(
            extract_bundle(&gz, &dest).is_err(),
            "absolute path must be rejected"
        );
        assert!(!evil.exists(), "absolute-path file must not be written");
        let _ = std::fs::remove_dir_all(&dest);
    }

    #[test]
    fn rejects_symlink_entry() {
        let dest = tmp();
        let gz = raw_targz(&[("link", SYM, b"", "/etc/passwd")]);
        assert!(
            extract_bundle(&gz, &dest).is_err(),
            "symlink entry must be rejected"
        );
        assert!(!dest.join("link").exists(), "symlink must not be created");
        let _ = std::fs::remove_dir_all(&dest);
    }

    #[test]
    fn rejects_non_regular_special_entry() {
        // Allow-list: only Regular + Directory. A fifo (or any exotic type) must
        // be rejected, not handed to unpack.
        let dest = tmp();
        let gz = raw_targz(&[("dev", FIFO, b"", "")]);
        assert!(
            extract_bundle(&gz, &dest).is_err(),
            "fifo/special entry must be rejected"
        );
        let _ = std::fs::remove_dir_all(&dest);
    }
}
