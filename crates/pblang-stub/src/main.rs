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
    for start in (0..=(exe.len() - slen)).rev() {
        if &exe[start..start + slen] == SENTINEL {
            return Some(start + slen);
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
        extract_bundle(tarball, &tmp);

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
        extract_bundle(tarball, &dest);
        println!("{}", dest.display());
        return;
    }

    // ── 4. Extract bundle to temp directory ───────────────────────────────────
    let tmp = make_temp_dir().unwrap_or_else(|e| {
        eprintln!("pblang-stub: cannot create temp dir: {e}");
        process::exit(1);
    });
    let _cleanup = TempDir(tmp.clone()); // removed on Drop (even on panic)
    extract_bundle(tarball, &tmp);

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

fn extract_bundle(tarball: &[u8], dest: &Path) {
    let dest_root = std::fs::canonicalize(dest).unwrap_or_else(|_| dest.to_path_buf());
    let gz = GzDecoder::new(std::io::Cursor::new(tarball));
    let mut archive = Archive::new(gz);
    let entries = archive.entries().unwrap_or_else(|e| {
        eprintln!("pblang-stub: failed to read bundle entries: {e}");
        process::exit(1);
    });
    for entry_res in entries {
        let mut entry = entry_res.unwrap_or_else(|e| {
            eprintln!("pblang-stub: failed to read a bundle entry: {e}");
            process::exit(1);
        });
        let entry_path = entry.path().map(|p| p.into_owned()).unwrap_or_else(|e| {
            eprintln!("pblang-stub: invalid entry path in bundle: {e}");
            process::exit(1);
        });
        if entry_path.is_absolute()
            || entry_path
                .components()
                .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            eprintln!(
                "pblang-stub: refusing to extract unsafe bundle path '{}'",
                entry_path.display()
            );
            process::exit(1);
        }
        let entry_type = entry.header().entry_type();
        if matches!(entry_type, tar::EntryType::Symlink | tar::EntryType::Link) {
            eprintln!(
                "pblang-stub: refusing to extract link entry '{}'",
                entry_path.display()
            );
            process::exit(1);
        }
        let target = dest_root.join(&entry_path);
        if !target.starts_with(&dest_root) {
            eprintln!(
                "pblang-stub: refusing to extract outside target dir: '{}'",
                entry_path.display()
            );
            process::exit(1);
        }
        entry.unpack_in(&dest_root).unwrap_or_else(|e| {
            eprintln!(
                "pblang-stub: failed to extract '{}': {e}",
                entry_path.display()
            );
            process::exit(1);
        });
    }
}
