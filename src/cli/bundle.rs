use std::path::PathBuf;

use crate::{
    copy_dir_all, generate_lockfile_for_cli, load_package_display_metadata,
    parse_registry_options_and_positionals, resolve_package_entry,
    shadow_validate_manifest_with_pblang, validate_packaged_local_dependencies,
};

pub(crate) fn handle_bundle(args: Vec<String>) {
    // Experimental: support an optional --self-extract flag (E5-beta).
    // We strip it from the arg list before feeding the rest into the
    // registry option parser so it is never reported as an unknown flag.
    let mut rest: Vec<String> = args.iter().skip(1).cloned().collect();
    let mut self_extract = false;
    if let Some(pos) = rest.iter().position(|a| a == "--self-extract") {
        self_extract = true;
        rest.remove(pos);
    }

    // Extract --output flag (only relevant for --self-extract)
    let mut self_extract_output: Option<String> = None;
    if let Some(pos) = rest.iter().position(|a| a == "--output") {
        if pos + 1 < rest.len() {
            self_extract_output = Some(rest[pos + 1].clone());
            rest.remove(pos + 1);
            rest.remove(pos);
        }
    }

    let (registry_index_url, require_registry_sha256_cli, positionals) =
        parse_registry_options_and_positionals("bundle", &rest).unwrap_or_else(|e| {
            eprintln!("{}", e);
            std::process::exit(1);
        });
    if positionals.len() > 2 {
        eprintln!("pblang bundle: expected [path] [out_dir]");
        std::process::exit(1);
    }
    let start = positionals
        .first()
        .cloned()
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| ".".into()));
    let man_dir = pblang_pm::find_manifest_dir(&start).unwrap_or_else(|| {
        eprintln!(
            "pblang bundle: could not find pblang.toml from '{}'",
            start.display()
        );
        std::process::exit(1);
    });
    shadow_validate_manifest_with_pblang(&man_dir, "bundle").unwrap_or_else(|e| {
        eprintln!("{}", e);
        std::process::exit(1);
    });
    let lock = generate_lockfile_for_cli(&man_dir, registry_index_url, require_registry_sha256_cli)
        .unwrap_or_else(|e| {
            eprintln!("pblang bundle: failed to generate lockfile: {}", e);
            std::process::exit(1);
        });
    let lock_str = pblang_pm::write_lockfile(&lock).unwrap_or_else(|e| {
        eprintln!("pblang bundle: failed to serialize lockfile: {}", e);
        std::process::exit(1);
    });
    let lock_path = man_dir.join("pblang.lock");
    std::fs::write(&lock_path, lock_str).unwrap_or_else(|e| {
        eprintln!(
            "pblang bundle: failed to write {}: {}",
            lock_path.display(),
            e
        );
        std::process::exit(1);
    });

    // Handle --self-extract mode: create self-extracting binary
    if self_extract {
        // Determine output path from --output flag or default
        let output_path = if let Some(output) = self_extract_output {
            PathBuf::from(output)
        } else {
            let pkg_name = load_package_display_metadata(&man_dir)
                .map(|(n, _, _)| n)
                .unwrap_or_else(|| "package".to_string());
            let exe_name = if cfg!(target_os = "windows") {
                format!("{}.exe", pkg_name)
            } else {
                pkg_name
            };
            man_dir.join(exe_name)
        };

        // Find current pblang executable (to embed as bin/pblang in the tarball)
        let current_exe = std::env::current_exe().unwrap_or_else(|e| {
            eprintln!(
                "pblang bundle --self-extract: cannot read current executable path: {}",
                e
            );
            std::process::exit(1);
        });

        // Create a temp staging directory that includes:
        //   - all package files (source, manifest, lockfile)
        //   - bin/pblang (or bin/pblang.exe) — the embedded runtime
        let staging = std::env::temp_dir().join(format!(
            "pblang-self-extract-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&staging).unwrap_or_else(|e| {
            eprintln!(
                "pblang bundle --self-extract: cannot create staging dir: {}",
                e
            );
            std::process::exit(1);
        });

        // Copy package files into staging
        for entry in walkdir::WalkDir::new(&man_dir)
            .follow_links(false)
            .into_iter()
            .filter_entry(|e| {
                // Always allow the root entry through. Otherwise, when the
                // user passes a relative path like `.`, walkdir reports
                // the root's `file_name()` as `"."` which starts with `.`
                // and the dotfile filter below would skip the entire
                // tree, silently producing an empty staging dir.
                if e.depth() == 0 {
                    return true;
                }
                let name = e.file_name().to_string_lossy();
                if e.file_type().is_dir() {
                    name != "target" && name != ".git" && !name.starts_with('.')
                } else {
                    true
                }
            })
        {
            let entry = entry.unwrap_or_else(|e| {
                eprintln!("pblang bundle --self-extract: walk error: {}", e);
                std::process::exit(1);
            });
            let rel = entry.path().strip_prefix(&man_dir).unwrap_or_else(|_| {
                eprintln!(
                    "pblang bundle --self-extract: invalid path outside package root: {}",
                    entry.path().display()
                );
                std::process::exit(1);
            });
            let dest = staging.join(rel);
            if entry.file_type().is_dir() {
                let _ = std::fs::create_dir_all(&dest);
            } else if entry.file_type().is_file() {
                if let Some(parent) = dest.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                if let Err(e) = std::fs::copy(entry.path(), &dest) {
                    eprintln!(
                        "pblang bundle --self-extract: failed to copy {} → {}: {}",
                        entry.path().display(),
                        dest.display(),
                        e
                    );
                    std::process::exit(1);
                }
            }
        }

        // Embed pblang runtime at bin/pblang
        let bin_dir = staging.join("bin");
        std::fs::create_dir_all(&bin_dir).unwrap_or_else(|e| {
            eprintln!("pblang bundle --self-extract: cannot create bin dir: {}", e);
            std::process::exit(1);
        });
        let runtime_name = if cfg!(target_os = "windows") {
            "pblang.exe"
        } else {
            "pblang"
        };
        let dest_runtime = bin_dir.join(runtime_name);
        std::fs::copy(&current_exe, &dest_runtime).unwrap_or_else(|e| {
            eprintln!(
                "pblang bundle --self-extract: failed to copy runtime to {}: {}",
                dest_runtime.display(),
                e
            );
            std::process::exit(1);
        });

        // Also copy pblang.toml if present
        let man_toml = man_dir.join("pblang.toml");
        if man_toml.is_file() {
            let _ = std::fs::copy(&man_toml, staging.join("pblang.toml"));
        }

        // Create tarball of the staging directory (package + embedded runtime)
        let tarball_bytes = pblang_pm::create_package_tarball(&staging).unwrap_or_else(|e| {
            eprintln!(
                "pblang bundle --self-extract: failed to create tarball: {}",
                e
            );
            std::process::exit(1);
        });

        // Clean up staging
        let _ = std::fs::remove_dir_all(&staging);

        // Find pblang-stub binary (look next to current executable)
        let Some(current_exe_parent) = current_exe.parent() else {
            eprintln!("pblang bundle --self-extract: cannot find parent directory of executable");
            std::process::exit(1);
        };
        let stub_path = current_exe_parent.join(if cfg!(target_os = "windows") {
            "pblang-stub.exe"
        } else {
            "pblang-stub"
        });

        let stub_bytes = std::fs::read(&stub_path).unwrap_or_else(|e| {
            eprintln!(
                "pblang bundle --self-extract: failed to read stub binary at {}: {}",
                stub_path.display(),
                e
            );
            eprintln!(
                "Make sure pblang-stub is built and installed alongside the pblang executable."
            );
            std::process::exit(1);
        });

        // Concatenate: stub + SENTINEL + tarball
        const SENTINEL: &[u8] = b"PBLANG_BUNDLE_START\x00";
        let mut output_bytes =
            Vec::with_capacity(stub_bytes.len() + SENTINEL.len() + tarball_bytes.len());
        output_bytes.extend_from_slice(&stub_bytes);
        output_bytes.extend_from_slice(SENTINEL);
        output_bytes.extend_from_slice(&tarball_bytes);

        // Write to output file
        std::fs::write(&output_path, &output_bytes).unwrap_or_else(|e| {
            eprintln!(
                "pblang bundle --self-extract: failed to write {}: {}",
                output_path.display(),
                e
            );
            std::process::exit(1);
        });

        // Set executable permissions on Unix
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&output_path, std::fs::Permissions::from_mode(0o755))
                .unwrap_or_else(|e| {
                    eprintln!(
                        "pblang bundle --self-extract: failed to set permissions: {}",
                        e
                    );
                    std::process::exit(1);
                });
        }

        println!("Created self-extracting binary: {}", output_path.display());
        println!(
            "To run: {} (includes embedded pblang runtime; no separate install required)",
            output_path.display()
        );
        return;
    }

    let out_dir = positionals
        .get(1)
        .cloned()
        .unwrap_or_else(|| man_dir.join("dist"));
    if let Err(e) = std::fs::create_dir_all(&out_dir) {
        eprintln!(
            "pblang bundle: failed to create {}: {}",
            out_dir.display(),
            e
        );
        std::process::exit(1);
    }
    let exe_name = if cfg!(target_os = "windows") {
        "pblang.exe"
    } else {
        "pblang"
    };
    let dest_exe = out_dir.join(exe_name);
    if let Ok(current_exe) = std::env::current_exe() {
        if let Err(e) = std::fs::copy(&current_exe, &dest_exe) {
            eprintln!(
                "pblang bundle: failed to copy executable to {}: {}",
                dest_exe.display(),
                e
            );
            std::process::exit(1);
        }
    }
    for f in &["pblang.toml", "pblang.lock"] {
        let src = man_dir.join(f);
        if src.is_file() {
            let _ = std::fs::copy(&src, out_dir.join(f));
        }
    }
    if let Err(e) = pblang_pm::validate_manifest_metadata(&man_dir) {
        eprintln!("pblang bundle: {}", e);
        std::process::exit(1);
    }
    let entry_path = resolve_package_entry(&man_dir).unwrap_or_else(|e| {
        eprintln!("pblang bundle: {}", e);
        std::process::exit(1);
    });
    let entry_rel = entry_path
        .strip_prefix(&man_dir)
        .unwrap_or(&entry_path)
        .to_path_buf();
    let bundled_entry = out_dir.join(&entry_rel);
    if let Some(parent) = bundled_entry.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            eprintln!(
                "pblang bundle: failed to create {}: {}",
                parent.display(),
                e
            );
            std::process::exit(1);
        }
    }
    if let Err(e) = std::fs::copy(&entry_path, &bundled_entry) {
        eprintln!(
            "pblang bundle: failed to copy entry {}: {}",
            entry_path.display(),
            e
        );
        std::process::exit(1);
    }
    let local_dep_paths = validate_packaged_local_dependencies(&man_dir).unwrap_or_else(|e| {
        eprintln!("pblang bundle: {}", e);
        std::process::exit(1);
    });
    for rel in local_dep_paths {
        let src = man_dir.join(&rel);
        let dest = out_dir.join(&rel);
        if let Err(e) = copy_dir_all(&src, &dest) {
            eprintln!("pblang bundle: failed to copy dep {}: {}", rel.display(), e);
            std::process::exit(1);
        }
    }
    let mut msg = format!(
        "bundled to {} (entry {})",
        out_dir.display(),
        entry_rel.to_string_lossy().replace('\\', "/")
    );
    if let Some((name, version, desc)) = load_package_display_metadata(&man_dir) {
        if !name.is_empty() && !version.is_empty() {
            msg.push_str(&format!(" — {}@{}", name, version));
        }
        if let Some(d) = desc.as_deref().filter(|s| !s.is_empty()) {
            msg.push_str(&format!(" — {}", d));
        }
    }
    println!("{}", msg);
    println!(
        "To run: {} run {}",
        out_dir.display(),
        entry_rel.to_string_lossy().replace('\\', "/")
    );
}
