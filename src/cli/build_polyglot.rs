//! `pblang build-polyglot` command handler.
//!
//! Extracts embedded `@lang { ... }` blocks to files and opportunistically
//! compiles supported languages when the host toolchain is available.

use std::path::PathBuf;

pub(crate) fn handle_build_polyglot(args: Vec<String>) {
    if args.len() < 2 {
        eprintln!("usage: pblang build-polyglot <file.sapp> [--out-dir <dir>]");
        eprintln!("  Extracts @lang {{ }} polyglot blocks from source and builds them.");
        eprintln!(
            "  --out-dir <dir>  Output directory for built artifacts (default: ./polyglot-out)"
        );
        std::process::exit(1);
    }
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let script_path = {
        let candidate = PathBuf::from(&args[1]);
        if candidate.is_absolute() {
            candidate
        } else {
            cwd.join(&candidate)
        }
    };
    let script_path = script_path.canonicalize().unwrap_or_else(|e| {
        eprintln!("build-polyglot: cannot find '{}': {e}", args[1]);
        std::process::exit(1);
    });
    let out_dir = args
        .iter()
        .position(|a| a == "--out-dir")
        .and_then(|i| args.get(i + 1))
        .map(PathBuf::from)
        .unwrap_or_else(|| cwd.join("polyglot-out"));
    std::fs::create_dir_all(&out_dir).unwrap_or_else(|e| {
        eprintln!("build-polyglot: cannot create '{}': {e}", out_dir.display());
        std::process::exit(1);
    });

    let src = std::fs::read_to_string(&script_path).unwrap_or_else(|e| {
        eprintln!(
            "build-polyglot: cannot read '{}': {e}",
            script_path.display()
        );
        std::process::exit(1);
    });

    let blocks = pblang_core::extract_polyglot_blocks(&src);
    if blocks.is_empty() {
        eprintln!(
            "build-polyglot: no polyglot blocks found in '{}'",
            script_path.display()
        );
        eprintln!("  Use @lang {{ }} syntax to embed foreign code: @python {{ print('hi') }}");
        return;
    }

    eprintln!("Found {} polyglot block(s):", blocks.len());
    for b in &blocks {
        let name = b.name.as_deref().unwrap_or("unnamed");
        let out_file = out_dir.join(format!(
            "{}_{}.{}",
            script_path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy(),
            name,
            match b.lang.as_str() {
                "python" | "py" => "py",
                "rust" | "rs" => "rs",
                "js" | "javascript" => "js",
                "bash" | "sh" => "sh",
                "ruby" | "rb" => "rb",
                "lua" => "lua",
                other => other,
            }
        ));
        std::fs::write(&out_file, &b.code).unwrap_or_else(|e| {
            eprintln!("build-polyglot: cannot write '{}': {e}", out_file.display());
            std::process::exit(1);
        });
        eprintln!("  @{} {} → {}", b.lang, name, out_file.display());

        // Attempt to compile if possible.
        if b.lang == "rust" || b.lang == "rs" {
            let out_so = out_dir.join(format!("lib{}.so", name));
            let status = std::process::Command::new("rustc")
                .arg(&out_file)
                .arg("--crate-type")
                .arg("cdylib")
                .arg("-o")
                .arg(&out_so)
                .status();
            match status {
                Ok(s) if s.success() => eprintln!("    → compiled {}", out_so.display()),
                _ => eprintln!("    (rustc not available, keeping source)"),
            }
        }

        if b.lang == "python" || b.lang == "py" {
            let status = std::process::Command::new("python3")
                .arg("-m")
                .arg("py_compile")
                .arg(&out_file)
                .status();
            match status {
                Ok(s) if s.success() => eprintln!("    → compiled .pyc"),
                _ => {}
            }
        }
    }

    let manifest = out_dir.join("polyglot.toml");
    let mut toml = String::from("[polyglot]\n");
    toml.push_str(&format!("source = \"{}\"\n", script_path.display()));
    toml.push_str(&format!("blocks = {}\n", blocks.len()));
    toml.push_str("\n[[blocks]]\n");
    for b in &blocks {
        let name = b.name.as_deref().unwrap_or("unnamed");
        toml.push_str(&format!("lang = \"{}\"\n", b.lang));
        toml.push_str(&format!("name = \"{}\"\n", name));
        toml.push_str(&format!("lines = {}\n", b.code.lines().count()));
        toml.push_str("\n[[blocks]]\n");
    }
    if toml.ends_with("[[blocks]]\n") {
        toml = toml[..toml.len() - "[[blocks]]\n".len()].to_string();
    }
    std::fs::write(&manifest, toml).unwrap();
    eprintln!("Manifest: {}", manifest.display());
    eprintln!(
        "Done. {} blocks extracted to '{}'",
        blocks.len(),
        out_dir.display()
    );
}
