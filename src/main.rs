use pblang_compiler::{compile, disasm};

use pblang_core::{
    parse, parse_strict, parse_with_polyglot, Expr, ExprKind, MatchArm, Span, Stmt, StmtKind,
    TypeExpr, TypeKind,
};
use pblang_ir::Chunk;
use pblang_typeck::check as type_check;
use pblang_vm::{StdlibConfig, VmLimits, VM};

use chumsky::error::SimpleReason;
use chumsky::prelude::Simple;

use std::io::{self, Read, Write};
use std::path::{Component, Path, PathBuf};

fn clear_terminal() {
    print!("\x1B[2J\x1B[H\x1B[3J");
    let _ = io::stdout().flush();
}

fn validate_package_relative_path<'a>(path: &'a str, label: &str) -> Result<&'a Path, String> {
    let rel_path = Path::new(path);
    if rel_path.as_os_str().is_empty() {
        return Err(format!("package {} cannot be empty", label));
    }
    if rel_path.is_absolute() {
        return Err(format!(
            "package {} '{}' must be a relative path inside the package root",
            label, path
        ));
    }
    for comp in rel_path.components() {
        match comp {
            Component::CurDir | Component::Normal(_) => {}
            Component::ParentDir => {
                return Err(format!(
                    "package {} '{}' must not escape the package root",
                    label, path
                ))
            }
            Component::Prefix(_) | Component::RootDir => {
                return Err(format!(
                    "package {} '{}' must be a relative path inside the package root",
                    label, path
                ))
            }
        }
    }
    Ok(rel_path)
}

fn resolve_package_entry(man_dir: &Path) -> Result<PathBuf, String> {
    let package_root = man_dir
        .canonicalize()
        .unwrap_or_else(|_| man_dir.to_path_buf());

    let manifest_path = man_dir.join("pblang.toml");
    if manifest_path.is_file() {
        let man_bytes = std::fs::read(&manifest_path)
            .map_err(|e| format!("read manifest '{}': {e}", manifest_path.display()))?;
        let man_str = std::str::from_utf8(&man_bytes)
            .map_err(|_| format!("manifest '{}' is not utf8", manifest_path.display()))?;
        let man = pblang_pm::parse_manifest(man_str)?;
        if let Some(entry) = man.package.as_ref().and_then(|pkg| pkg.entry.as_deref()) {
            let rel_entry = validate_package_relative_path(entry, "entry")?;
            let entry_path = man_dir.join(rel_entry);
            // Harden against symlink traversal inside the relative path.
            // This cannot fully eliminate TOCTOU races without O_NOFOLLOW/openat,
            // but it blocks package entries that traverse symlink components.
            let mut walk = package_root.clone();
            for comp in rel_entry.components() {
                walk.push(comp);
                let md = std::fs::symlink_metadata(&walk).map_err(|e| {
                    format!("package entry component not found '{}': {e}", walk.display())
                })?;
                if md.file_type().is_symlink() {
                    return Err(format!(
                        "package entry '{}' must not include symlink components",
                        entry
                    ));
                }
            }
            let resolved = entry_path
                .canonicalize()
                .map_err(|e| format!("package entry not found '{}': {e}", entry_path.display()))?;
            if !resolved.starts_with(&package_root) {
                return Err(format!(
                    "package entry escapes package root: {}",
                    resolved.display()
                ));
            }
            if !resolved.is_file() {
                return Err(format!(
                    "package entry must be a file: {}",
                    resolved.display()
                ));
            }
            return Ok(resolved);
        }
    }

    let candidate = [
        man_dir.join("main.sap"),
        man_dir.join("main.sapp"),
        man_dir.join("main.bpx"),
        man_dir.join("src").join("main.sap"),
        man_dir.join("src").join("main.sapp"),
        man_dir.join("src").join("main.bpx"),
    ]
    .into_iter()
    .filter_map(|p| p.canonicalize().ok())
    .find(|resolved| resolved.is_file() && resolved.starts_with(&package_root));

    candidate
        .ok_or_else(|| {
            format!(
                "no package entry found (expected [package].entry or main.sap/main.sapp/main.bpx/src/main.sap/src/main.sapp/src/main.bpx) in '{}'",
                man_dir.display()
            )
        })
}

fn read_text_file_with_race_check(path: &Path) -> Result<String, String> {
    let resolved = path
        .canonicalize()
        .map_err(|e| format!("file not found '{}': {e}", path.display()))?;
    let mut file = std::fs::File::open(path)
        .map_err(|e| format!("failed to open '{}': {e}", path.display()))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let opened_meta = file
            .metadata()
            .map_err(|e| format!("failed to stat opened file '{}': {e}", path.display()))?;
        let resolved_meta = std::fs::metadata(&resolved)
            .map_err(|e| format!("failed to stat resolved file '{}': {e}", resolved.display()))?;
        if opened_meta.dev() != resolved_meta.dev() || opened_meta.ino() != resolved_meta.ino() {
            return Err(format!(
                "refusing to read '{}': file changed during validation",
                path.display()
            ));
        }
    }

    let mut src = String::new();
    file.read_to_string(&mut src)
        .map_err(|e| format!("failed to read '{}': {e}", resolved.display()))?;
    Ok(src)
}

fn read_package_entry_source(man_dir: &Path) -> Result<(PathBuf, String), String> {
    let entry_path = resolve_package_entry(man_dir)?;
    let src = read_text_file_with_race_check(&entry_path)?;
    Ok((entry_path, src))
}

/// Loads [package] name, version, description from manifest for pack/bundle success output. Returns None if no manifest or parse error.
fn load_package_display_metadata(man_dir: &Path) -> Option<(String, String, Option<String>)> {
    let manifest_path = man_dir.join("pblang.toml");
    let man_bytes = std::fs::read(&manifest_path).ok()?;
    let man_str = std::str::from_utf8(&man_bytes).ok()?;
    let man = pblang_pm::parse_manifest(man_str).ok()?;
    let pkg = man.package.as_ref()?;
    let name = pkg.name.as_deref().unwrap_or("").to_string();
    let version = pkg.version.as_deref().unwrap_or("").to_string();
    let description = pkg.description.clone();
    Some((name, version, description))
}

fn validate_packaged_local_dependencies(man_dir: &Path) -> Result<Vec<PathBuf>, String> {
    let manifest_path = man_dir.join("pblang.toml");
    if !manifest_path.is_file() {
        return Ok(vec![]);
    }
    let man_bytes = std::fs::read(&manifest_path)
        .map_err(|e| format!("read manifest '{}': {e}", manifest_path.display()))?;
    let man_str = std::str::from_utf8(&man_bytes)
        .map_err(|_| format!("manifest '{}' is not utf8", manifest_path.display()))?;
    let man = pblang_pm::parse_manifest(man_str)?;
    let package_root = man_dir
        .canonicalize()
        .unwrap_or_else(|_| man_dir.to_path_buf());
    let mut rel_deps = Vec::new();
    for (name, spec) in &man.dependencies {
        if name.starts_with('@') || spec.git().is_some() {
            continue;
        }
        let Some(rel) = spec.path() else {
            continue;
        };
        let label = format!("dependency path for '{}'", name);
        let rel_path = validate_package_relative_path(rel, &label)?;
        let dep_path = package_root.join(rel_path);
        let resolved = dep_path.canonicalize().map_err(|e| {
            format!(
                "local dependency '{}' not found: {} ({e})",
                name,
                dep_path.display()
            )
        })?;
        if !resolved.starts_with(&package_root) {
            return Err(format!(
                "local dependency '{}' must stay inside the package root for pack/bundle: {}",
                name,
                resolved.display()
            ));
        }
        if !resolved.is_dir() {
            return Err(format!(
                "local dependency '{}' must be a directory for pack/bundle: {}",
                name,
                resolved.display()
            ));
        }
        rel_deps.push(rel_path.to_path_buf());
    }
    Ok(rel_deps)
}

fn require_registry_sha256(cli_flag: bool) -> bool {
    cli_flag
        || std::env::var("REQUIRE_REGISTRY_SHA256")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
}

fn generate_lockfile_for_cli(
    man_dir: &Path,
    registry_index_url: Option<&str>,
    require_registry_sha256_cli: bool,
) -> Result<pblang_pm::Lockfile, String> {
    if let Some(url) = registry_index_url {
        pblang_pm::generate_lockfile_with_registry(
            man_dir,
            Some(url),
            require_registry_sha256(require_registry_sha256_cli),
        )
    } else {
        pblang_pm::generate_lockfile(man_dir)
    }
}

fn escape_pblang_string_literal(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

fn run_manifest_validator_pblang_file(manifest_path: &Path) -> Result<serde_json::Value, String> {
    let path_str = manifest_path
        .to_string_lossy()
        .replace('\\', "/")
        .to_string();
    let escaped_path = escape_pblang_string_literal(&path_str);
    let src = format!(
        "let mv = import(\"stdlib:manifest_validate.sapp\"); print json_stringify(mv.validate_manifest_file(\"{escaped_path}\"));"
    );
    let (stmts, _blocks) = parse_with_polyglot(&src)
        .map_err(|errs| format!("parse pblang manifest validator snippet failed: {errs:?}"))?;
    let mut chunk = compile(stmts)
        .map_err(|e| format!("compile pblang manifest validator snippet failed: {e}"))?;
    chunk.source = Some(std::rc::Rc::new(src.clone()));
    let mut vm = VM::with_stdlib(StdlibConfig::default());
    let out = vm
        .run_collect(chunk)
        .map_err(|e| format!("run pblang manifest validator snippet failed: {}", e.message))?;
    let rendered = out
        .last()
        .ok_or_else(|| "pblang manifest validator produced no output".to_string())
        .and_then(|v| {
            v.as_str()
                .map(|s| s.to_string())
                .map_err(|e| format!("pblang manifest validator output was not string: {e}"))
        })?;
    serde_json::from_str::<serde_json::Value>(&rendered)
        .map_err(|e| format!("parse pblang manifest validator JSON output failed: {e}"))
}

fn run_manifest_parity_check(manifest_path: &Path) -> Result<serde_json::Value, String> {
    let man_text = std::fs::read_to_string(manifest_path)
        .map_err(|e| format!("read manifest '{}': {e}", manifest_path.display()))?;

    let mut rust_errors = Vec::<String>::new();
    let mut rust_dep_kinds = serde_json::Map::<String, serde_json::Value>::new();
    let rust_package = match pblang_pm::parse_manifest(&man_text) {
        Ok(man) => {
            let pkg = man.package.unwrap_or_default();
            let name = pkg.name.unwrap_or_default();
            let version = pkg.version.unwrap_or_default();
            let entry = pkg.entry.unwrap_or_default();
            for (dep_name, spec) in &man.dependencies {
                let kind = if spec.path().is_some() {
                    "path"
                } else if spec.git().is_some() {
                    "git"
                } else if spec.version_str().is_some() {
                    "version"
                } else {
                    "unknown"
                };
                rust_dep_kinds.insert(dep_name.clone(), serde_json::Value::String(kind.to_string()));
            }
            if name.trim().is_empty() {
                rust_errors.push("[package].name is required".to_string());
            }
            if version.trim().is_empty() {
                rust_errors.push("[package].version is required".to_string());
            }
            serde_json::json!({
                "name": name,
                "version": version,
                "entry": entry,
            })
        }
        Err(e) => {
            rust_errors.push(format!("rust parse_manifest error: {e}"));
            serde_json::json!({
                "name": "",
                "version": "",
                "entry": "",
            })
        }
    };
    let rust_ok = rust_errors.is_empty();

    let pblang = run_manifest_validator_pblang_file(manifest_path)?;
    let pblang_ok = pblang.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);
    let pblang_package = pblang
        .get("package")
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}));

    let rust_name = rust_package
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let rust_version = rust_package
        .get("version")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let rust_entry = rust_package
        .get("entry")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let p_name = pblang_package
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let p_version = pblang_package
        .get("version")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let p_entry = pblang_package
        .get("entry")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let mut pblang_deps = serde_json::Map::<String, serde_json::Value>::new();
    if let Some(items) = pblang.get("dependency_items").and_then(|v| v.as_array()) {
        for item in items {
            let Some(obj) = item.as_object() else { continue };
            let Some(name) = obj.get("name").and_then(|v| v.as_str()) else {
                continue;
            };
            let kind = obj
                .get("kind")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();
            pblang_deps.insert(name.to_string(), serde_json::Value::String(kind));
        }
    }

    let mut parity_errors = Vec::<String>::new();
    if rust_name != p_name {
        parity_errors.push(format!(
            "name mismatch: rust='{}' pblang='{}'",
            rust_name, p_name
        ));
    }
    if rust_version != p_version {
        parity_errors.push(format!(
            "version mismatch: rust='{}' pblang='{}'",
            rust_version, p_version
        ));
    }
    if rust_entry != p_entry {
        parity_errors.push(format!(
            "entry mismatch: rust='{}' pblang='{}'",
            rust_entry, p_entry
        ));
    }
    for (name, rust_kind_v) in &rust_dep_kinds {
        let rust_kind = rust_kind_v.as_str().unwrap_or("unknown");
        match pblang_deps.get(name).and_then(|v| v.as_str()) {
            Some(pk) if pk == rust_kind => {}
            Some(pk) => parity_errors.push(format!(
                "dependency kind mismatch for '{}': rust='{}' pblang='{}'",
                name, rust_kind, pk
            )),
            None => parity_errors.push(format!(
                "dependency missing in pblang validator output: '{}'",
                name
            )),
        }
    }
    for name in pblang_deps.keys() {
        if !rust_dep_kinds.contains_key(name) {
            parity_errors.push(format!(
                "extra dependency in pblang validator output: '{}'",
                name
            ));
        }
    }
    if !rust_ok {
        parity_errors.push("rust manifest validation failed".to_string());
    }
    if !pblang_ok {
        parity_errors.push("pblang manifest validation failed".to_string());
    }

    Ok(serde_json::json!({
        "parity_ok": parity_errors.is_empty(),
        "rust": {
            "ok": rust_ok,
            "errors": rust_errors,
            "package": rust_package,
            "dependencies": rust_dep_kinds,
        },
        "pblang": pblang,
        "parity_errors": parity_errors,
    }))
}

fn strict_manifest_validator_enabled(context: &str) -> bool {
    let strict_global = std::env::var("PBLANG_STRICT_MANIFEST_VALIDATOR")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    if !strict_global {
        return false;
    }
    let selector = std::env::var("PBLANG_STRICT_MANIFEST_VALIDATOR_COMMANDS")
        .unwrap_or_default();
    let selector = selector.trim();
    if selector.is_empty() {
        return true;
    }
    if selector == "*" {
        return true;
    }
    let ctx = context.trim().to_ascii_lowercase();
    selector
        .split(',')
        .map(|s| s.trim().to_ascii_lowercase())
        .any(|s| !s.is_empty() && s == ctx)
}

fn shadow_validate_manifest_with_pblang(man_dir: &Path, context: &str) -> Result<(), String> {
    let manifest_path = man_dir.join("pblang.toml");
    if !manifest_path.is_file() {
        return Ok(());
    }
    let strict = strict_manifest_validator_enabled(context);
    match run_manifest_validator_pblang_file(&manifest_path) {
        Ok(v) => {
            let ok = v.get("ok").and_then(|x| x.as_bool()).unwrap_or(true);
            if !ok {
                let mut errs = Vec::<String>::new();
                if let Some(arr) = v.get("errors").and_then(|x| x.as_array()) {
                    for e in arr {
                        if let Some(s) = e.as_str() {
                            errs.push(s.to_string());
                        }
                    }
                }
                let msg = if errs.is_empty() {
                    "pblang-side manifest validator reported invalid manifest".to_string()
                } else {
                    format!("pblang-side manifest validator: {}", errs.join("; "))
                };
                if strict {
                    return Err(format!(
                        "pblang {}: strict pblang manifest validation failed: {}",
                        context, msg
                    ));
                }
                eprintln!("pblang {}: warning: {}", context, msg);
            }
        }
        Err(e) => {
            if strict {
                return Err(format!(
                    "pblang {}: strict pblang manifest validation failed (validator unavailable): {}",
                    context, e
                ));
            }
            eprintln!("pblang {}: note: pblang-side manifest validator unavailable: {}", context, e);
        }
    }
    Ok(())
}

fn parse_registry_options_and_positionals<'a>(
    command: &str,
    rest: &'a [String],
) -> Result<(Option<&'a str>, bool, Vec<PathBuf>), String> {
    let mut registry_index_url: Option<&'a str> = None;
    let mut require_registry_sha256 = false;
    let mut positionals = Vec::new();
    let mut i = 0;
    while i < rest.len() {
        if rest[i] == "--registry-index" {
            i += 1;
            if i >= rest.len() {
                return Err(format!(
                    "pblang {}: --registry-index requires <url>",
                    command
                ));
            }
            if registry_index_url.is_some() {
                return Err(format!(
                    "pblang {}: --registry-index can only be passed once",
                    command
                ));
            }
            registry_index_url = Some(rest[i].as_str());
        } else if rest[i] == "--require-registry-sha256" {
            require_registry_sha256 = true;
        } else {
            positionals.push(PathBuf::from(&rest[i]));
        }
        i += 1;
    }
    Ok((registry_index_url, require_registry_sha256, positionals))
}

fn show_help() {
    println!("Biapryx REPL commands:");
    println!("  help | h | ?             Show this help");
    println!("  clear | cls              Clear the screen");
    println!("  exit | quit              Exit the REPL");
    println!();

    println!("CLI helpers (run without entering the REPL):");
    println!("  pblang init            Create pblang.toml in current directory");
    println!("  pblang run [file.sapp|file.bpx|dir]  Execute a script, or run the package entry from a manifest dir/current dir");
    println!("  pblang test [dir]       Run all .sap/.sapp/.bpx files in dir (default: tests)");
    println!("  pblang fmt [--max-line-length=N] [file.sap|file.bpx …] Format .sap/.sapp/.bpx (trim, operator spacing, indent 4 spaces, optional line wrap, final newline)");
    println!(
        "  pblang lint [file.sap|file.bpx …] Parse + typecheck files, report diagnostics (exit 1 if any)"
    );
    println!("  pblang --check <file>  Parse + typecheck a file (prints diagnostics)");
    println!(
        "  pblang --doc <file> [--html]  Print module docs (exports, functions, polyglot blocks)"
    );
    println!("  pblang --lock [--registry-index <url>] [--require-registry-sha256] [path]  Generate pblang.lock from pblang.toml (path optional)");
    println!("  pblang build [--registry-index <url>] [--require-registry-sha256] [path]  Generate lockfile + typecheck main entry (main.sap/main.bpx or src/main.sap/src/main.bpx)");
    println!("  pblang install [--registry-index <url>] [--require-registry-sha256] [path | <name>@<version>]  Resolve deps and generate pblang.lock (manifest path) or install a single registry package into cache (name@version).");
    println!("  pblang manifest-check [path]  Validate pblang.toml using pblang stdlib validator and print JSON report");
    println!("  pblang manifest-parity [path]  Compare Rust and pblang manifest parsing/validation and print parity JSON report");
    println!("  env PBLANG_STRICT_MANIFEST_VALIDATOR=1  Fail manifest-based commands when pblang-side validator reports errors");
    println!("  env PBLANG_STRICT_MANIFEST_VALIDATOR_COMMANDS=\"--lock,install,pack\"  Limit strict mode to specific commands");
    println!("  pblang publish --registry-dir <dir> [path]  Pack package into tarball and add to local registry index (file-based registry).");
    println!("  pblang publish            Publish current package to hosted registry (set PBLANG_REGISTRY_INDEX and PBLANG_REGISTRY_TOKEN env vars).");
    println!("  pblang search <query>     Search hosted or file-based registry for packages matching query (uses PBLANG_REGISTRY_INDEX).");
    println!("  pblang pack [--registry-index <url>] [--require-registry-sha256] [path] [out_file]  Create a single .tar.gz with manifest, lockfile and local deps (Stage A executable packaging)");
    println!("  pblang bundle [--registry-index <url>] [--require-registry-sha256] [path] [out_dir]  Create portable folder (default dist/) with executable + manifest + entry + deps");
    println!("  pblang verify <tarball>  Verify pack tarball structure and optional HMAC (set PBLANG_SIGNING_KEY when .sha256 sidecar present)");
    println!("  pblang --dump-bytecode <file>   Compile file and print bytecode (disassembly) for debugging / reverse engineering");
    println!("  pblang --annotate <file>   Compile file and print annotated bytecode (source line + instruction side-by-side)");
    println!("  pblang --symbols <file>    Print a simple symbol table for top-level functions and globals (name, arity, bytecode offset)");
    println!("  pblang lsp             Start LSP server on stdin/stdout (parse/typecheck diagnostics, hover, goto-def)");
    println!("  pblang serve [--port N]  Start local IDE backend server (default: 8090)");
    println!();

    println!("Language quick tour (examples):");
    println!("  # Expressions / print");
    println!("  print 1 + 2;");
    println!("  printf 2 1/3;");
    println!();

    println!("  # Variables");
    println!("  let x = 42;");
    println!("  print x;");
    println!();

    println!("  # Functions");
    println!("  fn add(a, b) {{ return a + b; }}");
    println!("  print add(2, 3);");
    println!();

    println!("Modules / imports / exports:");
    println!("  # Import a module by path (builtin)");
    println!("  let m = import(\"./my_module.sap\");");
    println!("  # Access exports (module member access)");
    println!("  print m.some_export;");
    println!("  # Exporting from a module file (conceptual)");
    println!("  export foo;");
    println!();

    println!("Collections: arrays + maps:");
    println!("  # Arrays: literal / push / pop / len / indexing");
    println!("  let a = [1, 2, 3];");
    println!("  arraypush(a, 4);");
    println!("  print len(a);");
    println!("  print a[0];");
    println!("  print arraypop(a);");
    println!();

    println!("  # Maps: literal / keys / has / remove / indexing");
    println!("  let m = {{\"x\": 1, \"y\": 2}};");
    println!("  print m[\"x\"];");
    println!("  print mapkeys(m);");
    println!("  print maphas(m, \"y\");");
    println!("  print mapremove(m, \"y\");");
    println!();

    println!("Runtime helpers (builtins):");
    println!("  # Types / conversion");
    println!("  print type(123);");
    println!("  print str(123);");
    println!("  print parsenumber(\"3.14\");");
    println!("  print parsebool(\"true\");");
    println!();

    println!("  # Time / randomness / crypto");
    println!("  print clock();");
    println!("  print sha256(\"hello\");");
    println!("  let b = randombytes(16);  # array of byte numbers");
    println!("  print constanttimeeq(\"a\", \"b\");");
    println!();

    println!("  # Base64");
    println!("  print base64encode(\"hi\");");
    println!("  print base64decode(\"aGk=\");");
    println!();

    println!("  # Strings");
    println!("  print strcontains(\"hello\", \"ell\");");
    println!("  print strstartswith(\"hello\", \"he\");");
    println!("  print strtrim(\"  hi  \");");
    println!("  print strsplit(\"a,b,c\", \",\");");
    println!();

    println!("Filesystem / paths (builtins):");
    println!("  print cwd();");
    println!("  print fsexists(\"./file.txt\");");
    println!("  print fsread(\"./file.txt\");");
    println!("  fswrite(\"./out.txt\", \"hello\");");
    println!("  fsappend(\"./out.txt\", \"\\nmore\");");
    println!("  fsrename(\"./out.txt\", \"./renamed.txt\");");
    println!("  fscopy(\"./renamed.txt\", \"./copy.txt\");");
    println!("  fsremove(\"./copy.txt\");");
    println!("  fsmkdirs(\"./a/b/c\");");
    println!("  print fslistdir(\".\");");
    println!("  print pathjoin(\"a\", \"b\", \"c.txt\");");
    println!("  print pathdirname(\"a/b/c.txt\");");
    println!("  print pathbasename(\"a/b/c.txt\");");
    println!();

    println!("JSON / HTTP (builtins):");
    println!("  let v = jsonparse(\"{{\\\"x\\\": 1}}\");");
    println!("  print jsonstringify(v);");
    println!("  print httpget(\"https://example.com\");");
    println!("  print httppost(\"https://httpbin.org/post\", \"{{\\\"x\\\":1}}\");");
    println!();

    println!("TCP (builtins):");
    println!("  let l = tcplisten(\"127.0.0.1:8080\");");
    println!("  let s = tcpaccept(l);");
    println!("  tcpwrite(s, \"hello\\n\");");
    println!("  print tcpread(s, 5);");
    println!("  tcpclose(s);");
    println!();

    println!("SQLite (builtins):");
    println!("  let db = sqliteopen(\"./test.db\");");
    println!("  sqliteexec(db, \"CREATE TABLE IF NOT EXISTS t(x INTEGER)\", nil);");
    println!("  sqliteexec(db, \"INSERT INTO t(x) VALUES (?)\", [123]);");
    println!("  let rows = sqlitequery(db, \"SELECT x FROM t\", nil);");
    println!("  print rows;");
    println!("  print sqlitelastinsertrowid(db);");
    println!("  sqliteclose(db);");
    println!();

    println!("FFI / exec / env (builtins):");
    println!("  print envget(\"PATH\");");
    println!("  envset(\"X\", \"123\");");
    println!("  print exec(\"whoami\");");
    println!("  # FFI (numbers only for now):");
    println!("  let h = ffiload(\"./mylib.dll\");");
    println!("  let out = fficall(h, \"add\", [1, 2]);");
    println!("  fficlose(h);");
    println!();

    println!("GC / diagnostics (builtins):");
    println!("  print gcstats();");
    println!("  print gccollect();");
    println!();

    println!("Error handling / cleanup (conceptual):");
    println!("  # try/endtry is supported by the VM (surface syntax may differ)");
    println!("  # defer is supported: deferred 0-arg closures run LIFO on scope exit");
    println!("  # Example sketch:");
    println!("  # try {{ ... }} catch {{ ... }}");
    println!("  # defer {{ print \"cleanup\"; }}");
    println!();

    println!("Polyglot blocks:");
    println!("  # When source contains polyglot blocks, they are exposed as globals:");
    println!("  #   polyglotblocks  -> array of blocks");
    println!("  #   polyglot        -> map {{ blocks, byname, bylang }}");
    println!("  # Use `pblang --doc <file>` to list detected polyglot blocks and spans.");
}

fn normalize_ide_source(src: &str) -> String {
    // IDE ergonomics: users often write JS/Python-style print("x");
    // Normalize simple line-based forms to pblang's print statement.
    let mut normalized = src
        .lines()
        .map(|line| {
            let trimmed = line.trim();
            if trimmed.starts_with("print(") && trimmed.ends_with(");") {
                let indent_len = line.chars().take_while(|c| c.is_whitespace()).count();
                let indent = &line[..indent_len];
                let inner = &trimmed["print(".len()..trimmed.len() - 2];
                format!("{indent}print {inner};")
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    if src.ends_with('\n') {
        normalized.push('\n');
    }
    normalized
}

fn format_ide_source(src: &str, max_line_length: Option<usize>) -> String {
    const INDENT: &str = "    ";
    let lines: Vec<&str> = src.lines().collect();
    let mut out = String::new();
    let mut depth: i32 = 0;
    let mut in_string = false;
    let mut in_single_string = false;
    let mut block_comment_depth = 0;

    for line in &lines {
        let mut trimmed = line.trim().to_string();
        trimmed = fmt_normalize_operator_spacing(&trimmed);
        trimmed = fmt_normalize_comma_spacing(&trimmed);
        if trimmed.is_empty() {
            out.push('\n');
            continue;
        }
        if trimmed.starts_with('}') {
            depth = depth.saturating_sub(1);
        }
        let indent_str: String = (0..depth.max(0) as usize).map(|_| INDENT).collect();
        let full_line = format!("{indent_str}{trimmed}");
        if let Some(max_len) = max_line_length {
            let mut line_to_output = full_line;
            while !line_to_output.is_empty() {
                if line_to_output.len() <= max_len {
                    out.push_str(&line_to_output);
                    out.push('\n');
                    scan_line_for_braces(
                        &line_to_output,
                        &mut in_string,
                        &mut in_single_string,
                        &mut block_comment_depth,
                        &mut depth,
                    );
                    break;
                }
                let break_at = line_to_output[..max_len].rfind(' ').unwrap_or(max_len);
                let break_at = if break_at == 0 { max_len } else { break_at };
                let rest = line_to_output.split_off(break_at);
                let emitted = line_to_output.trim_end().to_string();
                out.push_str(&emitted);
                out.push('\n');
                scan_line_for_braces(
                    &emitted,
                    &mut in_string,
                    &mut in_single_string,
                    &mut block_comment_depth,
                    &mut depth,
                );
                line_to_output = format!("{}{}", INDENT, rest.trim_start());
            }
        } else {
            out.push_str(&full_line);
            out.push('\n');
            scan_line_for_braces(
                &full_line,
                &mut in_string,
                &mut in_single_string,
                &mut block_comment_depth,
                &mut depth,
            );
        }
    }

    if !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod ide_source_tests {
    use super::{format_ide_source, normalize_ide_source};

    #[test]
    fn rewrites_print_call_style_lines() {
        let src = "print(\"Hello\");\nprint(1 + 2);\n";
        let got = normalize_ide_source(src);
        assert_eq!(got, "print \"Hello\";\nprint 1 + 2;\n");
    }

    #[test]
    fn keeps_statement_style_unchanged() {
        let src = "print \"Hello\";\nlet x = 1;\n";
        let got = normalize_ide_source(src);
        assert_eq!(got, "print \"Hello\";\nlet x = 1;\n");
    }

    #[test]
    fn formats_simple_block_indentation() {
        let src = "fn x(){\nprint 1;\n}\n";
        let got = format_ide_source(src, None);
        assert!(got.contains("fn x("));
        assert!(got.contains("\n    print 1;\n"));
    }

    #[test]
    fn keeps_braces_inside_single_quoted_strings_from_affecting_indent() {
        let src = "fn x(){\nlet s = '{';\nprint 1;\n}\n";
        let got = format_ide_source(src, None);
        assert!(got.contains("\n    print 1;\n"));
        assert!(!got.contains("\n        print 1;\n"));
    }
}

fn print_symbol_table(chunk: &Chunk) {
    // AUDIT NOTE #24: This symbol table is a best-effort heuristic.
    // It will produce FALSE POSITIVES (string literals that look like identifiers)
    // and MISS all local variables (which are slot-indexed, not string-named).
    // Use --dump-bytecode for accurate introspection of the compiled output.
    // A proper symbol table requires tracking declaration sites in the compiler.
    //
    // Best-effort symbol table based on constants and bytecode.
    // Functions: look for OpClosure(fn_const=idx) and treat the referenced constant string as a function name.
    let mut seen_fns = std::collections::HashSet::new();
    for (ip, op) in chunk.instructions.iter().enumerate() {
        if let pblang_ir::OpCode::OpClosure { fn_const, .. } = op {
            let idx = *fn_const;
            let name = chunk
                .constants
                .get(idx)
                .and_then(|v| v.as_str().ok())
                .unwrap_or("<anon>");
            if seen_fns.insert((name.to_string(), idx)) {
                println!("fn\t{}\tarity:? \toffset:{}", name, ip);
            }
        }
    }
    // Globals: scan constant table for identifier-like strings.
    for (idx, v) in chunk.constants.iter().enumerate() {
        if let pblang_ir::Value::Obj(o) = v {
            let pblang_ir::Obj::String(s) = o.as_ref();
            if !s.is_empty()
                && s.chars()
                    .next()
                    .map(|ch| ch.is_ascii_alphabetic() || ch == '_')
                    .unwrap_or(false)
            {
                println!("let\t{}\toffset:const#{}", s, idx);
            }
        }
    }
}

fn print_cfg(chunk: &Chunk, dot: bool) {
    use pblang_ir::OpCode;
    let instrs = &chunk.instructions;
    let n = instrs.len();
    if n == 0 {
        if dot {
            println!("digraph CFG {{");
            println!("  BB0 [label=\"BB0 (empty)\"];");
            println!("}}");
        } else {
            println!("BB0 (empty)");
        }
        return;
    }

    // Identify basic block leaders
    let mut leaders = vec![false; n];
    leaders[0] = true;
    for (i, op) in instrs.iter().enumerate() {
        match op {
            OpCode::OpJump(target)
            | OpCode::OpJumpIfFalse(target)
            | OpCode::OpJumpIfTrue(target) => {
                let t = *target;
                if t < n {
                    leaders[t] = true;
                }
                if i + 1 < n {
                    leaders[i + 1] = true;
                }
            }
            OpCode::OpReturn => {
                if i + 1 < n {
                    leaders[i + 1] = true;
                }
            }
            _ => {}
        }
    }

    // Assign block numbers
    let mut bb_of: Vec<usize> = vec![0; n];
    let mut cur_bb = 0usize;
    for i in 0..n {
        if leaders[i] {
            cur_bb += 1;
        }
        bb_of[i] = cur_bb - 1;
    }
    let num_bbs = cur_bb;

    // Collect edges
    let mut edges: Vec<(usize, usize)> = Vec::new();
    for (i, op) in instrs.iter().enumerate() {
        let src_bb = bb_of[i];
        match op {
            OpCode::OpJump(target) => {
                let t = *target;
                if t < n {
                    edges.push((src_bb, bb_of[t]));
                }
            }
            OpCode::OpJumpIfFalse(target) | OpCode::OpJumpIfTrue(target) => {
                let t = *target;
                if t < n {
                    edges.push((src_bb, bb_of[t]));
                }
                if i + 1 < n {
                    edges.push((src_bb, bb_of[i + 1]));
                }
            }
            OpCode::OpReturn => {}
            _ => {
                if i + 1 < n && leaders[i + 1] {
                    edges.push((src_bb, bb_of[i + 1]));
                }
            }
        }
    }

    // Deduplicate edges
    edges.sort();
    edges.dedup();

    if dot {
        println!("digraph CFG {{");
        for bb in 0..num_bbs {
            println!("  BB{bb} [label=\"BB{bb}\"];");
        }
        for (from, to) in &edges {
            println!("  BB{from} -> BB{to};");
        }
        println!("}}");
    } else {
        for (from, to) in &edges {
            println!("BB{from} -> BB{to}");
        }
        if edges.is_empty() {
            for bb in 0..num_bbs {
                println!("BB{bb} -> (no outgoing edges)");
            }
        }
    }
}

fn scan_line_for_braces(
    line: &str,
    in_double_string: &mut bool,
    in_single_string: &mut bool,
    block_comment_depth: &mut i32,
    brace_depth: &mut i32,
) {
    let mut chars = line.chars().peekable();
    let mut prev_was_backslash = false;

    while let Some(ch) = chars.next() {
        let next = chars.peek().copied();

        // If we're in a string, only care about escapes and matching close quote.
        if *in_double_string || *in_single_string {
            if *in_double_string && ch == '"' && !prev_was_backslash {
                *in_double_string = false;
            } else if *in_single_string && ch == '\'' && !prev_was_backslash {
                *in_single_string = false;
            }
            prev_was_backslash = ch == '\\' && !prev_was_backslash;
            continue;
        }

        // If we're in a block comment, look for nested /* and closing */
        if *block_comment_depth > 0 {
            if ch == '/' && next == Some('*') {
                chars.next(); // consume '*'
                *block_comment_depth += 1;
                continue;
            }
            if ch == '*' && next == Some('/') {
                chars.next(); // consume '/'
                *block_comment_depth -= 1;
                continue;
            }
            continue;
        }

        // Not in string/comment: detect start of line comment //
        if ch == '/' && next == Some('/') {
            break; // ignore rest of line
        }

        // Not in string/comment: detect start of block comment /*
        if ch == '/' && next == Some('*') {
            chars.next(); // consume '*'
            *block_comment_depth += 1;
            continue;
        }

        // Not in string/comment: entering string
        if ch == '"' {
            *in_double_string = true;
            prev_was_backslash = false;
            continue;
        }
        if ch == '\'' {
            *in_single_string = true;
            prev_was_backslash = false;
            continue;
        }

        // Count braces only in normal code
        match ch {
            '{' => *brace_depth += 1,
            '}' => *brace_depth -= 1,
            _ => {}
        }
    }
}

fn strip_comments_mask(src: &str) -> String {
    let bytes = src.as_bytes();
    let mut out = bytes.to_vec(); // same length as input (byte offsets preserved)
    let mut i = 0usize;
    let mut in_double_string = false;
    let mut in_single_string = false;
    let mut block_depth: i32 = 0;

    while i < out.len() {
        let b = out[i];

        // Inside string: only care about escapes and matching close quote.
        if in_double_string || in_single_string {
            if b == b'\\' {
                i = i.saturating_add(2);
                continue;
            }
            if in_double_string && b == b'"' {
                in_double_string = false;
            } else if in_single_string && b == b'\'' {
                in_single_string = false;
            }
            i += 1;
            continue;
        }

        // Inside block comment (nested): mask everything except newlines
        if block_depth > 0 {
            if i + 1 < out.len() && out[i] == b'/' && out[i + 1] == b'*' {
                out[i] = b' ';
                out[i + 1] = b' ';
                block_depth += 1;
                i += 2;
                continue;
            }
            if i + 1 < out.len() && out[i] == b'*' && out[i + 1] == b'/' {
                out[i] = b' ';
                out[i + 1] = b' ';
                block_depth -= 1;
                i += 2;
                continue;
            }
            if out[i] != b'\n' {
                out[i] = b' ';
            }
            i += 1;
            continue;
        }

        // Not in string/comment: entering string
        if b == b'"' {
            in_double_string = true;
            i += 1;
            continue;
        }
        if b == b'\'' {
            in_single_string = true;
            i += 1;
            continue;
        }

        // line comment //
        if i + 1 < out.len() && out[i] == b'/' && out[i + 1] == b'/' {
            out[i] = b' ';
            out[i + 1] = b' ';
            i += 2;
            while i < out.len() && out[i] != b'\n' {
                out[i] = b' ';
                i += 1;
            }
            continue;
        }

        // block comment /* ... */ (nested)
        if i + 1 < out.len() && out[i] == b'/' && out[i + 1] == b'*' {
            out[i] = b' ';
            out[i + 1] = b' ';
            block_depth = 1;
            i += 2;
            continue;
        }

        i += 1;
    }

    // SAFETY: We only push ASCII bytes in the fmt loop above.
    // Use lossy conversion as a fallback rather than panicking.
    String::from_utf8(out.clone()).unwrap_or_else(|_| String::from_utf8_lossy(&out).to_string())
}

fn line_col(src: &str, byte_idx: usize) -> (usize, usize) {
    let mut line = 1usize;
    let mut col = 1usize;
    for (i, ch) in src.char_indices() {
        if i >= byte_idx {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    (line, col)
}

fn get_line(src: &str, line_no: usize) -> &str {
    src.lines().nth(line_no.saturating_sub(1)).unwrap_or("")
}

fn underline_span(src: &str, sp: std::ops::Range<usize>) -> (usize, usize, String, String) {
    let start = sp.start.min(src.len());
    let end = sp.end.min(src.len()).max(start.saturating_add(1));

    let (ln1, col1) = line_col(src, start);
    let (ln2, col2) = line_col(src, end);

    let line = get_line(src, ln1).to_string();

    // If the span is on one line, draw ^^^ under the range.
    // If it spans multiple lines, fall back to a single caret at the start.
    let len = if ln1 == ln2 {
        col2.saturating_sub(col1).max(1)
    } else {
        1
    };

    let marker = format!("{}{}", " ".repeat(col1.saturating_sub(1)), "^".repeat(len));

    (ln1, col1, line, marker)
}

fn format_parse_error(err: &Simple<char>) -> String {
    // Keep this robust across chumsky versions: prefer structured info when available,
    // but fall back to Debug if needed.
    let label = err.label();
    let found = err.found();

    let mut expected = err.expected().copied().flatten().collect::<Vec<char>>();
    expected.sort();
    expected.dedup();

    let base = match err.reason() {
        SimpleReason::Unclosed { span: _, delimiter } => {
            format!("unclosed delimiter '{delimiter}'")
        }
        SimpleReason::Custom(msg) => msg.to_string(),
        SimpleReason::Unexpected => match (found, expected.is_empty()) {
            (Some(f), false) => format!("unexpected '{f}', expected one of: {:?}", expected),
            (Some(f), true) => format!("unexpected '{f}'"),
            (None, false) => format!("unexpected end of input, expected one of: {:?}", expected),
            (None, true) => "parse error".to_string(),
        },
    };

    if let Some(l) = label {
        format!("{base} (while parsing {l})")
    } else {
        base
    }
}

/// Normalize spacing around binary operators (outside string literals). Ensures one space
/// before and after + - * / == != < > <= >=. Does not change content inside "...".
fn fmt_normalize_operator_spacing(line: &str) -> String {
    let mut in_string = false;
    let mut escape_next = false;
    let mut i = 0;
    let bytes = line.as_bytes();
    let mut out = String::with_capacity(line.len() + 16);
    while i < bytes.len() {
        let c = bytes[i] as char;
        if in_string {
            if escape_next {
                escape_next = false;
                out.push(c);
            } else if c == '\\' {
                escape_next = true;
                out.push(c);
            } else if c == '"' {
                in_string = false;
                out.push(c);
            } else {
                out.push(c);
            }
            i += 1;
            continue;
        }
        if c == '"' {
            in_string = true;
            out.push(c);
            i += 1;
            continue;
        }
        let need_space_before = !out.is_empty()
            && !out.ends_with(' ')
            && !out.ends_with('(')
            && !out.ends_with('[')
            && !out.ends_with('=')
            && !out.ends_with('!')
            && !out.ends_with('<')
            && !out.ends_with('>')
            && !out.ends_with(',')
            && !out.ends_with(';');
        // Multi-char operators (longest first)
        let (op_len, need_space_after) = if i + 2 <= bytes.len() {
            let two = std::str::from_utf8(&bytes[i..i + 2]).unwrap_or("");
            if two == "==" || two == "!=" || two == "<=" || two == ">=" {
                (2, true)
            } else {
                (0, false)
            }
        } else {
            (0, false)
        };
        let (op_len, need_space_after) = if op_len > 0 {
            (op_len, need_space_after)
        } else if matches!(c, '+' | '*' | '/' | '<' | '>') {
            (1, true)
        } else if c == '-' {
            // Unary minus (-1): no space after if next is digit and we didn't add space before
            let next_is_digit =
                i + 1 < bytes.len() && matches!(bytes[i + 1] as char, '0'..='9' | '.');
            let space_after = !next_is_digit || need_space_before;
            (1, space_after)
        } else {
            out.push(c);
            i += 1;
            continue;
        };
        let prev_ok = out.ends_with(' ')
            || out.is_empty()
            || out.ends_with('(')
            || out.ends_with('[')
            || out.ends_with(',')
            || out.ends_with(';');
        if need_space_before && !prev_ok {
            out.push(' ');
        }
        for j in 0..op_len {
            out.push(bytes[i + j] as char);
        }
        i += op_len;
        if need_space_after && i < bytes.len() {
            let next = bytes[i] as char;
            if next != ' '
                && next != ')'
                && next != ']'
                && next != ';'
                && next != ','
                && next != '\n'
            {
                out.push(' ');
            }
        }
    }
    out
}

/// Ensure comma is followed by a single space when not inside string literals.
fn fmt_normalize_comma_spacing(line: &str) -> String {
    let mut in_string = false;
    let mut escape_next = false;
    let mut i = 0;
    let bytes = line.as_bytes();
    let mut out = String::with_capacity(line.len() + 8);
    while i < bytes.len() {
        let c = bytes[i] as char;
        if in_string {
            if escape_next {
                escape_next = false;
                out.push(c);
            } else if c == '\\' {
                escape_next = true;
                out.push(c);
            } else if c == '"' {
                in_string = false;
                out.push(c);
            } else {
                out.push(c);
            }
            i += 1;
            continue;
        }
        if c == '"' {
            in_string = true;
            out.push(c);
            i += 1;
            continue;
        }
        if c == ',' {
            out.push(c);
            i += 1;
            if i < bytes.len() {
                let next = bytes[i] as char;
                if next != ' ' && next != '\n' && next != '\r' {
                    out.push(' ');
                }
            }
            continue;
        }
        out.push(c);
        i += 1;
    }
    out
}

fn print_parse_errors(src: &str, errors: Vec<Simple<char>>, file: Option<&str>) {
    let max = 8usize;
    for (i, err) in errors.iter().take(max).enumerate() {
        let sp = err.span();
        let (ln, col, line, marker) = underline_span(src, sp);
        let msg = format_parse_error(err);
        if let Some(path) = file {
            eprintln!("error[{}:{}:{}]: {}", path, ln, col, msg);
        } else {
            eprintln!("error[{}:{}]: {}", ln, col, msg);
        }
        eprintln!("{line}");
        eprintln!("{marker}");

        if i + 1 < errors.len().min(max) {
            eprintln!();
        }
    }

    if errors.len() > max {
        eprintln!("(and {} more parse errors)", errors.len() - max);
    }
}

// ── Call-graph (R2-alpha) ─────────────────────────────────────────────────

/// Caller → set of callees (function names called inside that function).
type CallGraph = std::collections::BTreeMap<String, std::collections::BTreeSet<String>>;

fn build_call_graph(ast: &[Stmt], _file: &str) -> CallGraph {
    let mut graph: CallGraph = std::collections::BTreeMap::new();
    let mut current_fn = "<main>".to_string();
    graph.entry(current_fn.clone()).or_default();
    walk_stmts_for_cg(ast, &mut graph, &mut current_fn);
    graph
}

fn walk_stmts_for_cg(stmts: &[Stmt], graph: &mut CallGraph, current_fn: &mut String) {
    for stmt in stmts {
        match &stmt.kind {
            StmtKind::Fn(name, _params, _ret, body) => {
                let prev = current_fn.clone();
                *current_fn = name.clone();
                graph.entry(name.clone()).or_default();
                walk_stmts_for_cg(body, graph, current_fn);
                *current_fn = prev;
            }
            StmtKind::Expr(e) | StmtKind::Return(Some(e)) => {
                collect_calls_from_expr(e, graph, current_fn);
            }
            StmtKind::Let(_, _, e) => {
                collect_calls_from_expr(e, graph, current_fn);
            }
            StmtKind::If(cond, then_branch, else_branch) => {
                collect_calls_from_expr(cond, graph, current_fn);
                walk_stmts_for_cg(then_branch, graph, current_fn);
                if let Some(eb) = else_branch {
                    walk_stmts_for_cg(eb, graph, current_fn);
                }
            }
            StmtKind::While(cond, body) => {
                collect_calls_from_expr(cond, graph, current_fn);
                walk_stmts_for_cg(body, graph, current_fn);
            }
            StmtKind::For(_, count, body) => {
                collect_calls_from_expr(count, graph, current_fn);
                walk_stmts_for_cg(body, graph, current_fn);
            }
            StmtKind::ForIn(_, col, body) => {
                collect_calls_from_expr(col, graph, current_fn);
                walk_stmts_for_cg(body, graph, current_fn);
            }
            StmtKind::ForInKV(_, _, col, body) => {
                collect_calls_from_expr(col, graph, current_fn);
                walk_stmts_for_cg(body, graph, current_fn);
            }
            StmtKind::Print(e) | StmtKind::PrintF(_, e) => {
                graph
                    .entry(current_fn.clone())
                    .or_default()
                    .insert("print".to_string());
                collect_calls_from_expr(e, graph, current_fn);
            }
            StmtKind::Defer(e) => {
                collect_calls_from_expr(e, graph, current_fn);
            }
            _ => {}
        }
    }
}

fn collect_calls_from_expr(expr: &Expr, graph: &mut CallGraph, current_fn: &str) {
    match &expr.kind {
        ExprKind::Call(callee, args) => {
            if let ExprKind::Var(name) = &callee.kind {
                graph
                    .entry(current_fn.to_string())
                    .or_default()
                    .insert(name.clone());
            }
            collect_calls_from_expr(callee, graph, current_fn);
            for a in args {
                collect_calls_from_expr(a, graph, current_fn);
            }
        }
        ExprKind::Add(l, r)
        | ExprKind::Sub(l, r)
        | ExprKind::Mul(l, r)
        | ExprKind::Div(l, r)
        | ExprKind::Eq(l, r)
        | ExprKind::NotEq(l, r)
        | ExprKind::Gt(l, r)
        | ExprKind::Ge(l, r)
        | ExprKind::Lt(l, r)
        | ExprKind::Le(l, r) => {
            collect_calls_from_expr(l, graph, current_fn);
            collect_calls_from_expr(r, graph, current_fn);
        }
        ExprKind::Logical { left, right, .. } => {
            collect_calls_from_expr(left, graph, current_fn);
            collect_calls_from_expr(right, graph, current_fn);
        }
        ExprKind::Index(obj, ix) => {
            collect_calls_from_expr(obj, graph, current_fn);
            collect_calls_from_expr(ix, graph, current_fn);
        }
        ExprKind::GetAttr(obj, _) => {
            collect_calls_from_expr(obj, graph, current_fn);
        }
        ExprKind::Try(try_e, catch_e) => {
            collect_calls_from_expr(try_e, graph, current_fn);
            collect_calls_from_expr(catch_e, graph, current_fn);
        }
        ExprKind::FnLit(opt_name, _, _, body) => {
            let mut inner = opt_name
                .clone()
                .unwrap_or_else(|| "<anonymous>".to_string());
            graph.entry(inner.clone()).or_default();
            walk_stmts_for_cg(body, graph, &mut inner);
        }
        ExprKind::BlockExpr(stmts, opt_e) => {
            let mut block_ctx = current_fn.to_string();
            walk_stmts_for_cg(stmts, graph, &mut block_ctx);
            if let Some(e) = opt_e {
                collect_calls_from_expr(e, graph, current_fn);
            }
        }
        ExprKind::Match(scrut, arms, else_e) => {
            collect_calls_from_expr(scrut, graph, current_fn);
            for (_arm, e) in arms {
                collect_calls_from_expr(e, graph, current_fn);
            }
            if let Some(e) = else_e {
                collect_calls_from_expr(e, graph, current_fn);
            }
        }
        ExprKind::Array(exprs) => {
            for e in exprs {
                collect_calls_from_expr(e, graph, current_fn);
            }
        }
        ExprKind::Map(pairs) => {
            for (_, v) in pairs {
                collect_calls_from_expr(v, graph, current_fn);
            }
        }
        _ => {}
    }
}

fn print_call_graph(graph: &CallGraph) {
    for (caller, callees) in graph {
        if callees.is_empty() {
            println!("{caller} -> (no calls)");
        } else {
            for callee in callees {
                println!("{caller} -> {callee}");
            }
        }
    }
}

// ── Data-flow / use-def analysis (R3-alpha) ────────────────────────────────

#[derive(Default)]
struct VarDefInfo {
    line: usize,
    used: bool,
}

fn print_dataflow_report(ast: &[Stmt], src: &str) {
    use std::collections::BTreeMap;

    let mut defs: BTreeMap<String, VarDefInfo> = BTreeMap::new();
    let mut uses: Vec<(String, usize)> = Vec::new();

    walk_stmts_for_dataflow(ast, src, &mut defs, &mut uses);

    // Mark used flags.
    for (name, use_line) in &uses {
        if let Some(info) = defs.get_mut(name) {
            info.used = true;
        }
        println!("use {} line:{}", name, use_line);
    }

    for (name, info) in &defs {
        println!("def {} line:{}", name, info.line);
    }

    for (name, info) in &defs {
        if !info.used {
            println!("WARN {} assigned but never read", name);
        }
    }
}

fn walk_stmts_for_dataflow(
    stmts: &[Stmt],
    src: &str,
    defs: &mut std::collections::BTreeMap<String, VarDefInfo>,
    uses: &mut Vec<(String, usize)>,
) {
    for stmt in stmts {
        match &stmt.kind {
            StmtKind::Let(name, _ty, expr) => {
                let (line, _) = line_col(src, stmt.span.start);
                defs.entry(name.clone())
                    .or_insert(VarDefInfo { line, used: false });
                walk_expr_for_dataflow(expr, src, defs, uses);
            }
            StmtKind::Assign(name, expr) => {
                // Treat assignment as a definition site as well.
                let (line, _) = line_col(src, stmt.span.start);
                defs.entry(name.clone())
                    .or_insert(VarDefInfo { line, used: false });
                walk_expr_for_dataflow(expr, src, defs, uses);
            }
            StmtKind::For(var, count_expr, body) => {
                let (line, _) = line_col(src, stmt.span.start);
                defs.entry(var.clone())
                    .or_insert(VarDefInfo { line, used: false });
                walk_expr_for_dataflow(count_expr, src, defs, uses);
                walk_stmts_for_dataflow(body, src, defs, uses);
            }
            StmtKind::ForIn(var, col_expr, body) => {
                let (line, _) = line_col(src, stmt.span.start);
                defs.entry(var.clone())
                    .or_insert(VarDefInfo { line, used: false });
                walk_expr_for_dataflow(col_expr, src, defs, uses);
                walk_stmts_for_dataflow(body, src, defs, uses);
            }
            StmtKind::ForInKV(k, v, col_expr, body) => {
                let (line, _) = line_col(src, stmt.span.start);
                defs.entry(k.clone())
                    .or_insert(VarDefInfo { line, used: false });
                defs.entry(v.clone())
                    .or_insert(VarDefInfo { line, used: false });
                walk_expr_for_dataflow(col_expr, src, defs, uses);
                walk_stmts_for_dataflow(body, src, defs, uses);
            }
            StmtKind::Expr(e)
            | StmtKind::Print(e)
            | StmtKind::PrintF(_, e)
            | StmtKind::Return(Some(e))
            | StmtKind::Defer(e) => {
                walk_expr_for_dataflow(e, src, defs, uses);
            }
            StmtKind::If(cond, then_branch, else_branch) => {
                walk_expr_for_dataflow(cond, src, defs, uses);
                walk_stmts_for_dataflow(then_branch, src, defs, uses);
                if let Some(eb) = else_branch {
                    walk_stmts_for_dataflow(eb, src, defs, uses);
                }
            }
            StmtKind::While(cond, body) => {
                walk_expr_for_dataflow(cond, src, defs, uses);
                walk_stmts_for_dataflow(body, src, defs, uses);
            }
            StmtKind::Fn(_, _params, _ret, body) => {
                walk_stmts_for_dataflow(body, src, defs, uses);
            }
            StmtKind::Block(body) | StmtKind::Seq(body) => {
                walk_stmts_for_dataflow(body, src, defs, uses);
            }
            StmtKind::SetIndex(target, index, value) => {
                walk_expr_for_dataflow(target, src, defs, uses);
                walk_expr_for_dataflow(index, src, defs, uses);
                walk_expr_for_dataflow(value, src, defs, uses);
            }
            StmtKind::SetAttr(target, _name, value) => {
                walk_expr_for_dataflow(target, src, defs, uses);
                walk_expr_for_dataflow(value, src, defs, uses);
            }
            StmtKind::Import(_) | StmtKind::Export(_) | StmtKind::Return(None) => {}
            // const behaves like let for dataflow analysis
            StmtKind::Const(name, _ty, expr) => {
                let (line, _) = line_col(src, stmt.span.start);
                defs.entry(name.clone())
                    .or_insert(VarDefInfo { line, used: false });
                walk_expr_for_dataflow(expr, src, defs, uses);
            }
            // New control flow
            StmtKind::Break | StmtKind::Continue => {}
            StmtKind::Throw(e) => {
                walk_expr_for_dataflow(e, src, defs, uses);
            }
            StmtKind::Loop(body) => {
                walk_stmts_for_dataflow(body, src, defs, uses);
            }
        }
    }
}

fn walk_expr_for_dataflow(
    expr: &Expr,
    src: &str,
    defs: &mut std::collections::BTreeMap<String, VarDefInfo>,
    uses: &mut Vec<(String, usize)>,
) {
    match &expr.kind {
        ExprKind::Var(name) => {
            let (line, _) = line_col(src, expr.span.start);
            uses.push((name.clone(), line));
        }
        ExprKind::Call(callee, args) => {
            walk_expr_for_dataflow(callee, src, defs, uses);
            for a in args {
                walk_expr_for_dataflow(a, src, defs, uses);
            }
        }
        ExprKind::GetAttr(target, _) => {
            walk_expr_for_dataflow(target, src, defs, uses);
        }
        ExprKind::Index(target, index) => {
            walk_expr_for_dataflow(target, src, defs, uses);
            walk_expr_for_dataflow(index, src, defs, uses);
        }
        ExprKind::Array(items) => {
            for e in items {
                walk_expr_for_dataflow(e, src, defs, uses);
            }
        }
        ExprKind::Map(entries) => {
            for (_k, v) in entries {
                walk_expr_for_dataflow(v, src, defs, uses);
            }
        }
        ExprKind::Try(body, handler) => {
            walk_expr_for_dataflow(body, src, defs, uses);
            walk_expr_for_dataflow(handler, src, defs, uses);
        }
        ExprKind::FnLit(_name, _params, _ret, body) => {
            walk_stmts_for_dataflow(body, src, defs, uses);
        }
        ExprKind::BlockExpr(body, tail) => {
            walk_stmts_for_dataflow(body, src, defs, uses);
            if let Some(t) = tail {
                walk_expr_for_dataflow(t, src, defs, uses);
            }
        }
        ExprKind::IfExpr(cond, then_br, else_br) => {
            walk_expr_for_dataflow(cond, src, defs, uses);
            walk_expr_for_dataflow(then_br, src, defs, uses);
            walk_expr_for_dataflow(else_br, src, defs, uses);
        }
        ExprKind::Add(l, r)
        | ExprKind::Sub(l, r)
        | ExprKind::Mul(l, r)
        | ExprKind::Div(l, r)
        | ExprKind::Eq(l, r)
        | ExprKind::NotEq(l, r)
        | ExprKind::Gt(l, r)
        | ExprKind::Ge(l, r)
        | ExprKind::Lt(l, r)
        | ExprKind::Le(l, r) => {
            walk_expr_for_dataflow(l, src, defs, uses);
            walk_expr_for_dataflow(r, src, defs, uses);
        }
        ExprKind::Logical { left, right, .. } => {
            walk_expr_for_dataflow(left, src, defs, uses);
            walk_expr_for_dataflow(right, src, defs, uses);
        }
        ExprKind::Match(scrutinee, arms, else_expr) => {
            walk_expr_for_dataflow(scrutinee, src, defs, uses);
            for (pat, body) in arms {
                match pat {
                    MatchArm::Literal(e) => walk_expr_for_dataflow(e, src, defs, uses),
                    MatchArm::Bind(name) => {
                        let (line, _) = line_col(src, expr.span.start);
                        defs.entry(name.clone())
                            .or_insert(VarDefInfo { line, used: false });
                    }
                    MatchArm::Wildcard => {}
                    MatchArm::BindGuard(name, guard) => {
                        let (line, _) = line_col(src, expr.span.start);
                        defs.entry(name.clone())
                            .or_insert(VarDefInfo { line, used: false });
                        walk_expr_for_dataflow(guard, src, defs, uses);
                    }
                }
                walk_expr_for_dataflow(body, src, defs, uses);
            }
            if let Some(e) = else_expr {
                walk_expr_for_dataflow(e, src, defs, uses);
            }
        }
        ExprKind::Number(_) | ExprKind::Str(_) | ExprKind::Bool(_) | ExprKind::Nil => {}
        // Safety operators — walk their inner expressions
        ExprKind::Not(inner) => walk_expr_for_dataflow(inner, src, defs, uses),
        ExprKind::SafeGetAttr(obj, _) => walk_expr_for_dataflow(obj, src, defs, uses),
        ExprKind::SafeIndex(obj, idx) => {
            walk_expr_for_dataflow(obj, src, defs, uses);
            walk_expr_for_dataflow(idx, src, defs, uses);
        }
        ExprKind::ResultUnwrap(inner) => walk_expr_for_dataflow(inner, src, defs, uses),
        // New expression kinds
        ExprKind::NullCoalesce(a, b) | ExprKind::Pow(a, b) | ExprKind::Mod(a, b) => {
            walk_expr_for_dataflow(a, src, defs, uses);
            walk_expr_for_dataflow(b, src, defs, uses);
        }
        ExprKind::FString(parts) => {
            for part in parts {
                if let pblang_core::FStringPart::Interp(e) = part {
                    walk_expr_for_dataflow(e, src, defs, uses);
                }
            }
        }
    }
}

// ── Annotated disassembly (R2-beta) ──────────────────────────────────────

fn print_annotated_disasm(chunk: &Chunk, src: &str) {
    let raw = disasm(chunk);
    let src_lines: Vec<&str> = src.lines().collect();
    let mut last_line: Option<usize> = None;

    for instr_line in raw.lines() {
        let offset = instr_line
            .split_whitespace()
            .next()
            .and_then(|s| s.parse::<usize>().ok());

        let src_line = offset.and_then(|off| {
            chunk.spans.get(off).map(|sp| {
                let (ln, _) = line_col(src, sp.start);
                ln
            })
        });

        if let Some(ln) = src_line {
            if last_line != Some(ln) {
                let text = src_lines.get(ln.saturating_sub(1)).copied().unwrap_or("");
                println!("-- line {} -------  {}", ln, text.trim());
                last_line = Some(ln);
            }
        }

        println!("{}", instr_line);
    }
}

// LSP: see `pblang lsp` → `pblang_lsp::run_stdio_server()` (tower-lsp + parse/typecheck).

const DEFAULT_MANIFEST: &str = r#"[package]
name = "my-app"
version = "0.1.0"

[dependencies]
"#;

fn copy_dir_all(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
    let src_root = src.canonicalize()?;
    std::fs::create_dir_all(dst)?;
    let dst_root = dst.to_path_buf();

    fn copy_dir_all_inner(
        src_root: &Path,
        cur_src: &Path,
        dst_root: &Path,
        cur_dst: &Path,
    ) -> std::io::Result<()> {
        std::fs::create_dir_all(cur_dst)?;
        for e in std::fs::read_dir(cur_src)? {
            let e = e?;
            let ty = e.file_type()?;
            let src_path = e.path();
            let src_name = e.file_name();
            let dest_path = cur_dst.join(&src_name);
            if !dest_path.starts_with(dst_root) {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!(
                        "refusing to copy outside destination root: {}",
                        dest_path.display()
                    ),
                ));
            }
            if ty.is_symlink() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("refusing to copy symlink: {}", src_path.display()),
                ));
            }

            let src_canon = src_path.canonicalize()?;
            if !src_canon.starts_with(src_root) {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!(
                        "refusing to copy path outside source root: {}",
                        src_canon.display()
                    ),
                ));
            }
            if ty.is_dir() {
                copy_dir_all_inner(src_root, &src_canon, dst_root, &dest_path)?;
            } else if ty.is_file() {
                std::fs::copy(&src_canon, &dest_path)?;
            }
        }
        Ok(())
    }
    copy_dir_all_inner(&src_root, &src_root, &dst_root, &dst_root)
}

fn write_http_response_with_local_cors(
    stream: &mut std::net::TcpStream,
    response: &str,
    cors_origin: &str,
) {
    use std::io::Write;
    let patched = response.replace(
        "Access-Control-Allow-Origin: *",
        &format!("Access-Control-Allow-Origin: {cors_origin}"),
    );
    let _ = stream.write_all(patched.as_bytes());
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if matches!(
        args.first().map(|s| s.as_str()),
        Some("--help") | Some("-h") | Some("help")
    ) {
        show_help();
        return;
    }
    if matches!(
        args.first().map(|s| s.as_str()),
        Some("--version") | Some("-V") | Some("version")
    ) {
        println!("pblang {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    if args.first().map(|s| s.as_str()) == Some("init") {
        let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        let manifest_path = cwd.join("pblang.toml");
        if manifest_path.exists() {
            eprintln!("pblang init: {} already exists", manifest_path.display());
            std::process::exit(1);
        }
        match std::fs::write(&manifest_path, DEFAULT_MANIFEST) {
            Ok(()) => println!("created {}", manifest_path.display()),
            Err(e) => {
                eprintln!(
                    "pblang init: failed to write {}: {}",
                    manifest_path.display(),
                    e
                );
                std::process::exit(1);
            }
        }
        return;
    }
    if args.first().map(|s| s.as_str()) == Some("new") {
        let name = match args.get(1) {
            Some(n) if !n.starts_with('-') => n.clone(),
            _ => {
                eprintln!("usage: pblang new <name> [--template web|lib|cli]");
                std::process::exit(1);
            }
        };
        let template = args
            .iter()
            .position(|a| a == "--template")
            .and_then(|i| args.get(i + 1))
            .map(|s| s.as_str())
            .unwrap_or("cli");
        let valid_templates = ["web", "lib", "cli"];
        if !valid_templates.contains(&template) {
            eprintln!(
                "pblang new: unknown template '{}'; valid: web, lib, cli",
                template
            );
            std::process::exit(1);
        }
        let proj_dir = std::env::current_dir()
            .unwrap_or_else(|_| std::path::PathBuf::from("."))
            .join(&name);
        if proj_dir.exists() {
            eprintln!("pblang new: directory '{}' already exists", name);
            std::process::exit(1);
        }
        let src_dir = proj_dir.join("src");
        std::fs::create_dir_all(&src_dir).unwrap_or_else(|e| {
            eprintln!("pblang new: failed to create directory: {}", e);
            std::process::exit(1);
        });
        // Write pblang.toml
        let manifest = format!(
            "[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nentry = \"src/main.sapp\"\n\n[dependencies]\n"
        );
        std::fs::write(proj_dir.join("pblang.toml"), &manifest).unwrap_or_else(|e| {
            eprintln!("pblang new: failed to write pblang.toml: {}", e);
            std::process::exit(1);
        });
        // Write .gitignore
        std::fs::write(proj_dir.join(".gitignore"), "target/\n.pblang/\n").unwrap_or_default();
        // Write main.sapp based on template
        let main_src = match template {
            "web" => format!(
                "// {name} \u{2014} web template\nimport \"stdlib:http_server.sapp\";\n\nlet app = http_server.app_new();\nlet app = http_server.app_get(app, \"/\", fn(req) {{\n    return map{{ status: 200, body: \"Hello from {name}!\", content_type: \"text/plain\" }};\n}});\n\nprint \"Listening on :8080\";\nhttp_server.serve_forever(app, 8080);\n"
            ),
            "lib" => format!(
                "// {name} \u{2014} library template\n\n// Export your public functions here\nexport fn greet(name) {{\n    return \"Hello, \" + name + \"!\";\n}}\n"
            ),
            _ => format!(
                "// {name} \u{2014} CLI template\nimport \"stdlib:os.sapp\";\n\nlet args = os.args();\nif len(args) > 0 {{\n    print \"Hello, \" + args[0] + \"!\";\n}} else {{\n    print \"Hello, World!\";\n}}\n"
            ),
        };
        std::fs::write(src_dir.join("main.sapp"), &main_src).unwrap_or_else(|e| {
            eprintln!("pblang new: failed to write src/main.sapp: {}", e);
            std::process::exit(1);
        });
        println!("Created project '{name}' with template '{template}'");
        println!("  {name}/pblang.toml");
        println!("  {name}/src/main.sapp");
        println!("  {name}/.gitignore");
        println!("To run: cd {name} && pblang run .");
        return;
    }
    if args.first().map(|s| s.as_str()) == Some("manifest-check") {
        if args.len() > 2 {
            eprintln!("pblang manifest-check: expected at most one [path] argument");
            std::process::exit(1);
        }
        let start = args
            .get(1)
            .map(PathBuf::from)
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| ".".into()));
        let man_dir = pblang_pm::find_manifest_dir(&start).unwrap_or_else(|| {
            eprintln!(
                "pblang manifest-check: could not find pblang.toml from '{}'",
                start.display()
            );
            std::process::exit(1);
        });
        let manifest_path = man_dir.join("pblang.toml");
        let report = run_manifest_validator_pblang_file(&manifest_path).unwrap_or_else(|e| {
            eprintln!("pblang manifest-check: {}", e);
            std::process::exit(1);
        });
        println!(
            "{}",
            serde_json::to_string_pretty(&report).unwrap_or_else(|_| report.to_string())
        );
        if !report
            .get("ok")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            std::process::exit(1);
        }
        return;
    }
    if args.first().map(|s| s.as_str()) == Some("manifest-parity") {
        if args.len() > 2 {
            eprintln!("pblang manifest-parity: expected at most one [path] argument");
            std::process::exit(1);
        }
        let start = args
            .get(1)
            .map(PathBuf::from)
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| ".".into()));
        let man_dir = pblang_pm::find_manifest_dir(&start).unwrap_or_else(|| {
            eprintln!(
                "pblang manifest-parity: could not find pblang.toml from '{}'",
                start.display()
            );
            std::process::exit(1);
        });
        let manifest_path = man_dir.join("pblang.toml");
        let report = run_manifest_parity_check(&manifest_path).unwrap_or_else(|e| {
            eprintln!("pblang manifest-parity: {}", e);
            std::process::exit(1);
        });
        println!(
            "{}",
            serde_json::to_string_pretty(&report).unwrap_or_else(|_| report.to_string())
        );
        if !report
            .get("parity_ok")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            std::process::exit(1);
        }
        return;
    }
    if args.first().map(|s| s.as_str()) == Some("--lock") {
        let rest: Vec<String> = args.iter().skip(1).cloned().collect();
        let (registry_index_url, require_registry_sha256_cli, positionals) =
            parse_registry_options_and_positionals("--lock", &rest).unwrap_or_else(|e| {
                eprintln!("{}", e);
                std::process::exit(1);
            });
        if positionals.len() > 1 {
            eprintln!("pblang --lock: expected at most one [path] argument");
            std::process::exit(1);
        }
        let start = positionals
            .first()
            .cloned()
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| ".".into()));
        let man_dir = pblang_pm::find_manifest_dir(&start).unwrap_or_else(|| {
            eprintln!(
                "pblang --lock: could not find pblang.toml from '{}'",
                start.display()
            );
            std::process::exit(1);
        });
        shadow_validate_manifest_with_pblang(&man_dir, "--lock").unwrap_or_else(|e| {
            eprintln!("{}", e);
            std::process::exit(1);
        });
        let lock =
            generate_lockfile_for_cli(&man_dir, registry_index_url, require_registry_sha256_cli)
                .unwrap_or_else(|e| {
                    eprintln!("pblang --lock: failed to generate lockfile: {}", e);
                    std::process::exit(1);
                });
        let lock_str = pblang_pm::write_lockfile(&lock).unwrap_or_else(|e| {
            eprintln!("pblang --lock: failed to serialize lockfile: {}", e);
            std::process::exit(1);
        });
        let lock_path = man_dir.join("pblang.lock");
        std::fs::write(&lock_path, lock_str).unwrap_or_else(|e| {
            eprintln!(
                "pblang --lock: failed to write {}: {}",
                lock_path.display(),
                e
            );
            std::process::exit(1);
        });
        println!("wrote {}", lock_path.display());
        return;
    }
    if args.first().map(|s| s.as_str()) == Some("install") {
        let rest: Vec<String> = args.iter().skip(1).cloned().collect();
        let (registry_index_url, require_registry_sha256_cli, positionals) =
            parse_registry_options_and_positionals("install", &rest).unwrap_or_else(|e| {
                eprintln!("{}", e);
                std::process::exit(1);
            });
        if positionals.len() > 1 {
            eprintln!("pblang install: expected at most one [path|name@version] argument");
            std::process::exit(1);
        }
        // name@version mode: install a single registry package into cache.
        if let Some(arg) = positionals.first() {
            let arg_str = arg.to_string_lossy();
            if arg_str.contains('@') && !arg.exists() {
                // Use the last '@' to separate name from version, supporting scoped names like @scope/name@version
                // SAFETY: we just checked `arg_str.contains('@')` above, so rfind is guaranteed to return Some.
                let at_pos = match arg_str.rfind('@') {
                    Some(p) => p,
                    None => unreachable!("rfind('@') can't be None: contains('@') was true"),
                };
                let name = &arg_str[..at_pos];
                let version_range = &arg_str[at_pos + 1..];
                if name.is_empty() || version_range.is_empty() {
                    eprintln!(
                        "pblang install: expected <name>@<version>, got '{}'",
                        arg_str
                    );
                    std::process::exit(1);
                }
                let registry_url = if let Some(url) = registry_index_url {
                    url.to_string()
                } else if let Ok(env_url) = std::env::var("PBLANG_REGISTRY_INDEX") {
                    env_url
                } else {
                    eprintln!(
                        "pblang install: no registry index configured (pass --registry-index <url> or set PBLANG_REGISTRY_INDEX)"
                    );
                    std::process::exit(1);
                };
                let require_sha = require_registry_sha256(require_registry_sha256_cli);
                let cache_dir = pblang_pm::cache_root(None);
                let (resolved, _path) = pblang_pm::fetch_and_install_package(
                    &registry_url,
                    name,
                    version_range,
                    &cache_dir,
                    require_sha,
                )
                .unwrap_or_else(|e| {
                    eprintln!("pblang install: {}", e);
                    std::process::exit(1);
                });
                println!("Installed {}@{}", name, resolved);
                return;
            }
        }
        // Existing manifest-based install: generate lockfile for registry/path dependencies.
        let start = positionals
            .first()
            .cloned()
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| ".".into()));
        let man_dir = pblang_pm::find_manifest_dir(&start).unwrap_or_else(|| {
            eprintln!(
                "pblang install: could not find pblang.toml from '{}'",
                start.display()
            );
            std::process::exit(1);
        });
        shadow_validate_manifest_with_pblang(&man_dir, "install").unwrap_or_else(|e| {
            eprintln!("{}", e);
            std::process::exit(1);
        });
        let lock =
            generate_lockfile_for_cli(&man_dir, registry_index_url, require_registry_sha256_cli);
        let lock = lock.unwrap_or_else(|e| {
            eprintln!("pblang install: failed to generate lockfile: {}", e);
            std::process::exit(1);
        });
        let lock_str = pblang_pm::write_lockfile(&lock).unwrap_or_else(|e| {
            eprintln!("pblang install: failed to serialize lockfile: {}", e);
            std::process::exit(1);
        });
        let lock_path = man_dir.join("pblang.lock");
        std::fs::write(&lock_path, lock_str).unwrap_or_else(|e| {
            eprintln!(
                "pblang install: failed to write {}: {}",
                lock_path.display(),
                e
            );
            std::process::exit(1);
        });
        println!("dependencies ready ({})", lock_path.display());
        return;
    }
    if args.first().map(|s| s.as_str()) == Some("publish") {
        let rest: Vec<&str> = args.iter().skip(1).map(|s| s.as_str()).collect();
        // If --registry-dir is present, use legacy local-registry publish.
        if rest.contains(&"--registry-dir") {
            let mut registry_dir: Option<std::path::PathBuf> = None;
            let mut path_arg: Option<std::path::PathBuf> = None;
            let mut i = 0;
            while i < rest.len() {
                if rest[i] == "--registry-dir" {
                    i += 1;
                    if i < rest.len() {
                        registry_dir = Some(std::path::PathBuf::from(rest[i]));
                        i += 1;
                    } else {
                        eprintln!("pblang publish: --registry-dir requires <dir>");
                        std::process::exit(1);
                    }
                } else {
                    path_arg = Some(std::path::PathBuf::from(rest[i]));
                    break;
                }
            }
            let registry_dir = registry_dir.unwrap_or_else(|| {
                eprintln!("pblang publish: --registry-dir <dir> required");
                std::process::exit(1);
            });
            let start =
                path_arg.unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| ".".into()));
            let man_dir = pblang_pm::find_manifest_dir(&start).unwrap_or_else(|| {
                eprintln!(
                    "pblang publish: could not find pblang.toml from '{}'",
                    start.display()
                );
                std::process::exit(1);
            });
            shadow_validate_manifest_with_pblang(&man_dir, "publish").unwrap_or_else(|e| {
                eprintln!("{}", e);
                std::process::exit(1);
            });
            if let Err(e) = pblang_pm::publish_to_registry_dir(&man_dir, &registry_dir) {
                eprintln!("pblang publish: {}", e);
                std::process::exit(1);
            }
            println!("published to {}", registry_dir.display());
            return;
        }

        // Hosted or file:// registry publish (M4-beta).
        let start = std::env::current_dir().unwrap_or_else(|_| ".".into());
        let man_dir = pblang_pm::find_manifest_dir(&start).unwrap_or_else(|| {
            eprintln!(
                "pblang publish: could not find pblang.toml from '{}'",
                start.display()
            );
            std::process::exit(1);
        });
        shadow_validate_manifest_with_pblang(&man_dir, "publish").unwrap_or_else(|e| {
            eprintln!("{}", e);
            std::process::exit(1);
        });
        let registry_url = std::env::var("PBLANG_REGISTRY_INDEX").unwrap_or_else(|_| {
            eprintln!(
                "pblang publish: no registry configured (set PBLANG_REGISTRY_INDEX to registry base URL or file:// directory)"
            );
            std::process::exit(1);
        });
        // SECURITY FIX #28: Require env var tokens to avoid leaking secrets in
        // shell history, `ps aux` process listings, and CI logs.
        let token = std::env::var("PBLANG_REGISTRY_TOKEN").unwrap_or_else(|_| {
            for a in &rest {
                if a.starts_with("--registry-token=") {
                    eprintln!(
                        "pblang publish: --registry-token is disabled for security. Set PBLANG_REGISTRY_TOKEN in the environment."
                    );
                    std::process::exit(2);
                }
            }
            String::new()
        });
        if let Err(e) = pblang_pm::publish_package(&registry_url, &man_dir, &token) {
            eprintln!("pblang publish: {}", e);
            std::process::exit(1);
        }
        println!("Published to {}", registry_url);
        return;
    }
    if args.first().map(|s| s.as_str()) == Some("search") {
        let query = args.get(1).map(|s| s.as_str()).unwrap_or("");
        let registry_url = std::env::var("PBLANG_REGISTRY_INDEX").unwrap_or_else(|_| {
            eprintln!(
                "pblang search: no registry configured (set PBLANG_REGISTRY_INDEX to registry base URL or file:// directory)"
            );
            std::process::exit(1);
        });
        let results = pblang_pm::search_registry(&registry_url, query).unwrap_or_else(|e| {
            eprintln!("pblang search: {}", e);
            std::process::exit(1);
        });
        if results.is_empty() {
            println!("No results for '{}'", query);
        } else {
            for r in &results {
                if r.description.trim().is_empty() {
                    println!("{} @ {}", r.name, r.version);
                } else {
                    println!("{} @ {} — {}", r.name, r.version, r.description);
                }
            }
        }
        return;
    }
    if args.first().map(|s| s.as_str()) == Some("pack") {
        // Stage A executables: create a single .tar.gz artifact with manifest, lockfile and local deps.
        let rest: Vec<String> = args.iter().skip(1).cloned().collect();
        let (registry_index_url, require_registry_sha256_cli, positionals) =
            parse_registry_options_and_positionals("pack", &rest).unwrap_or_else(|e| {
                eprintln!("{}", e);
                std::process::exit(1);
            });
        if positionals.len() > 2 {
            eprintln!("pblang pack: expected [path] [out_file]");
            std::process::exit(1);
        }
        let start = positionals
            .first()
            .cloned()
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| ".".into()));
        let man_dir = pblang_pm::find_manifest_dir(&start).unwrap_or_else(|| {
            eprintln!(
                "pblang pack: could not find pblang.toml from '{}'",
                start.display()
            );
            std::process::exit(1);
        });
        shadow_validate_manifest_with_pblang(&man_dir, "pack").unwrap_or_else(|e| {
            eprintln!("{}", e);
            std::process::exit(1);
        });
        // Ensure we have an up-to-date lockfile before packing.
        let lock =
            generate_lockfile_for_cli(&man_dir, registry_index_url, require_registry_sha256_cli)
                .unwrap_or_else(|e| {
                    eprintln!("pblang pack: failed to generate lockfile: {}", e);
                    std::process::exit(1);
                });
        let lock_str = pblang_pm::write_lockfile(&lock).unwrap_or_else(|e| {
            eprintln!("pblang pack: failed to serialize lockfile: {}", e);
            std::process::exit(1);
        });
        let lock_path = man_dir.join("pblang.lock");
        std::fs::write(&lock_path, lock_str).unwrap_or_else(|e| {
            eprintln!(
                "pblang pack: failed to write {}: {}",
                lock_path.display(),
                e
            );
            std::process::exit(1);
        });

        let default_out = man_dir.join("dist").join("app.tar.gz");
        let out_path = positionals.get(1).cloned().unwrap_or(default_out);
        if let Some(parent) = out_path.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                eprintln!("pblang pack: failed to create {}: {}", parent.display(), e);
                std::process::exit(1);
            }
        }

        if let Err(e) = pblang_pm::validate_manifest_metadata(&man_dir) {
            eprintln!("pblang pack: {}", e);
            std::process::exit(1);
        }
        let entry_path = resolve_package_entry(&man_dir).unwrap_or_else(|e| {
            eprintln!("pblang pack: {}", e);
            std::process::exit(1);
        });
        if let Err(e) = validate_packaged_local_dependencies(&man_dir) {
            eprintln!("pblang pack: {}", e);
            std::process::exit(1);
        }
        let entry_rel = entry_path
            .strip_prefix(&man_dir)
            .unwrap_or(&entry_path)
            .to_string_lossy()
            .replace('\\', "/");

        let bytes = pblang_pm::create_package_tarball(&man_dir).unwrap_or_else(|e| {
            eprintln!("pblang pack: failed to build tarball: {}", e);
            std::process::exit(1);
        });
        if let Err(e) = std::fs::write(&out_path, &bytes) {
            eprintln!("pblang pack: failed to write {}: {}", out_path.display(), e);
            std::process::exit(1);
        }
        let mut msg = format!("packed to {} (entry {})", out_path.display(), entry_rel);
        if let Some((name, version, desc)) = load_package_display_metadata(&man_dir) {
            if !name.is_empty() && !version.is_empty() {
                msg.push_str(&format!(" — {}@{}", name, version));
            }
            if let Some(d) = desc.as_deref().filter(|s| !s.is_empty()) {
                msg.push_str(&format!(" — {}", d));
            }
        }
        println!("{}", msg);
        if let Ok(key) = std::env::var("PBLANG_SIGNING_KEY") {
            if key.trim().is_empty() {
                eprintln!("pblang pack: skipping signing because PBLANG_SIGNING_KEY is empty");
            } else {
                let hmac_hex = pblang_pm::compute_tarball_hmac(&bytes, key.as_bytes());
                let sidecar = out_path.parent().unwrap_or(std::path::Path::new(".")).join(
                    out_path
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default()
                        + ".sha256",
                );
                if let Err(e) = std::fs::write(&sidecar, hmac_hex) {
                    eprintln!(
                        "pblang pack: failed to write HMAC sidecar {}: {}",
                        sidecar.display(),
                        e
                    );
                } else {
                    println!("signed (HMAC sidecar {})", sidecar.display());
                }
            }
        }
        println!("To run: unpack the tarball and run 'pblang run .' in the unpacked directory.");
        return;
    }
    if args.first().map(|s| s.as_str()) == Some("verify") {
        let rest: Vec<String> = args.iter().skip(1).cloned().collect();
        let tarball_path = match rest.first() {
            Some(p) => std::path::PathBuf::from(p),
            None => {
                eprintln!("pblang verify: expected <tarball>");
                std::process::exit(1);
            }
        };
        if !tarball_path.is_file() {
            eprintln!("pblang verify: not a file: {}", tarball_path.display());
            std::process::exit(1);
        }
        match pblang_pm::verify_tarball(&tarball_path) {
            Ok(()) => println!("verify ok: {}", tarball_path.display()),
            Err(e) => {
                eprintln!("pblang verify: {}", e);
                std::process::exit(1);
            }
        }
        return;
    }
    if args.first().map(|s| s.as_str()) == Some("--call-graph") {
        let path = match args.get(1) {
            Some(p) => p.as_str(),
            None => {
                eprintln!("usage: pblang --call-graph <file.sapp|file.bpx>");
                std::process::exit(1);
            }
        };
        let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        let file_path = cwd.join(path);
        let src = std::fs::read_to_string(&file_path).unwrap_or_else(|e| {
            eprintln!("error reading {}: {}", path, e);
            std::process::exit(1);
        });
        let src_for_parse = strip_comments_mask(&src);
        match parse_strict(&src_for_parse) {
            Ok(ast) => {
                let graph = build_call_graph(&ast, path);
                print_call_graph(&graph);
            }
            Err(errs) => {
                print_parse_errors(&src_for_parse, errs, Some(path));
                std::process::exit(2);
            }
        }
        return;
    }
    if args.first().map(|s| s.as_str()) == Some("--dataflow") {
        let path = match args.get(1) {
            Some(p) => p.as_str(),
            None => {
                eprintln!("usage: pblang --dataflow <file.sapp|file.bpx>");
                std::process::exit(1);
            }
        };
        let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        let file_path = cwd.join(path);
        let src = std::fs::read_to_string(&file_path).unwrap_or_else(|e| {
            eprintln!("error reading {}: {}", path, e);
            std::process::exit(1);
        });
        let src_for_parse = strip_comments_mask(&src);
        let ast = match parse_strict(&src_for_parse) {
            Ok(ast) => ast,
            Err(errs) => {
                print_parse_errors(&src_for_parse, errs, Some(path));
                std::process::exit(2);
            }
        };
        print_dataflow_report(&ast, &src_for_parse);
        return;
    }
    if args.first().map(|s| s.as_str()) == Some("--dump-bytecode") {
        let path = match args.get(1) {
            Some(p) => p,
            None => {
                eprintln!("pblang --dump-bytecode: pass a .sap, .sapp, or .bpx file");
                std::process::exit(1);
            }
        };
        let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        let file_path = cwd.join(path);
        let src = std::fs::read_to_string(&file_path).unwrap_or_else(|e| {
            eprintln!(
                "pblang --dump-bytecode: failed to read {}: {}",
                file_path.display(),
                e
            );
            std::process::exit(1);
        });
        let src_for_parse = strip_comments_mask(&src);
        let stmts = match parse_strict(&src_for_parse) {
            Ok(s) => s,
            Err(errs) => {
                print_parse_errors(&src_for_parse, errs, Some(path));
                std::process::exit(2);
            }
        };
        let chunk = match compile(stmts) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("pblang: compile error: {}", e);
                std::process::exit(3);
            }
        };
        println!("{}", disasm(&chunk));
        return;
    }
    if args.first().map(|s| s.as_str()) == Some("--annotate") {
        let path = match args.get(1) {
            Some(p) => p,
            None => {
                eprintln!("pblang --annotate: pass a .sap, .sapp, or .bpx file");
                std::process::exit(1);
            }
        };
        let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        let file_path = cwd.join(path);
        let src = std::fs::read_to_string(&file_path).unwrap_or_else(|e| {
            eprintln!(
                "pblang --annotate: failed to read {}: {}",
                file_path.display(),
                e
            );
            std::process::exit(1);
        });
        let src_for_parse = strip_comments_mask(&src);
        let stmts = match parse_strict(&src_for_parse) {
            Ok(s) => s,
            Err(errs) => {
                print_parse_errors(&src_for_parse, errs, Some(path));
                std::process::exit(2);
            }
        };
        let chunk = match compile(stmts) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("pblang: compile error: {}", e);
                std::process::exit(3);
            }
        };
        print_annotated_disasm(&chunk, &src_for_parse);
        return;
    }
    if args.first().map(|s| s.as_str()) == Some("--symbols") {
        let path = match args.get(1) {
            Some(p) => p,
            None => {
                eprintln!("pblang --symbols: pass a .sap, .sapp, or .bpx file");
                std::process::exit(1);
            }
        };
        let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        let file_path = cwd.join(path);
        let src = std::fs::read_to_string(&file_path).unwrap_or_else(|e| {
            eprintln!(
                "pblang --symbols: failed to read {}: {}",
                file_path.display(),
                e
            );
            std::process::exit(1);
        });
        let src_for_parse = strip_comments_mask(&src);
        let stmts = match parse_strict(&src_for_parse) {
            Ok(s) => s,
            Err(errs) => {
                print_parse_errors(&src_for_parse, errs, path.as_str().into());
                std::process::exit(2);
            }
        };
        let chunk = match compile(stmts) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("pblang: compile error: {}", e);
                std::process::exit(3);
            }
        };
        print_symbol_table(&chunk);
        return;
    }
    if args.first().map(|s| s.as_str()) == Some("--cfg") {
        let use_dot = args.iter().any(|a| a == "--cfg-dot");
        let path = match args.iter().find(|a| !a.starts_with('-')) {
            Some(p) => p,
            None => {
                eprintln!("pblang --cfg: pass a .sap/.sapp file (optionally with --cfg-dot)");
                std::process::exit(1);
            }
        };
        let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        let file_path = cwd.join(path);
        let src = std::fs::read_to_string(&file_path).unwrap_or_else(|e| {
            eprintln!(
                "pblang --cfg: failed to read {}: {}",
                file_path.display(),
                e
            );
            std::process::exit(1);
        });
        let src_masked = strip_comments_mask(&src);
        let stmts = match parse_strict(&src_masked) {
            Ok(s) => s,
            Err(errs) => {
                print_parse_errors(&src_masked, errs, path.as_str().into());
                std::process::exit(2);
            }
        };
        let chunk = match compile(stmts) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("pblang: compile error: {}", e);
                std::process::exit(3);
            }
        };
        print_cfg(&chunk, use_dot);
        return;
    }
    if args.first().map(|s| s.as_str()) == Some("build") {
        let rest: Vec<String> = args.iter().skip(1).cloned().collect();
        let (registry_index_url, require_registry_sha256_cli, positionals) =
            parse_registry_options_and_positionals("build", &rest).unwrap_or_else(|e| {
                eprintln!("{}", e);
                std::process::exit(1);
            });
        if positionals.len() > 1 {
            eprintln!("pblang build: expected at most one [path] argument");
            std::process::exit(1);
        }
        let start = positionals
            .first()
            .cloned()
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| ".".into()));
        let man_dir = pblang_pm::find_manifest_dir(&start).unwrap_or_else(|| {
            eprintln!(
                "pblang build: could not find pblang.toml from '{}'",
                start.display()
            );
            std::process::exit(1);
        });
        shadow_validate_manifest_with_pblang(&man_dir, "build").unwrap_or_else(|e| {
            eprintln!("{}", e);
            std::process::exit(1);
        });
        let lock =
            generate_lockfile_for_cli(&man_dir, registry_index_url, require_registry_sha256_cli)
                .unwrap_or_else(|e| {
                    eprintln!("pblang build: failed to generate lockfile: {}", e);
                    std::process::exit(1);
                });
        let lock_str = pblang_pm::write_lockfile(&lock).unwrap_or_else(|e| {
            eprintln!("pblang build: failed to serialize lockfile: {}", e);
            std::process::exit(1);
        });
        let lock_path = man_dir.join("pblang.lock");
        std::fs::write(&lock_path, lock_str).unwrap_or_else(|e| {
            eprintln!(
                "pblang build: failed to write {}: {}",
                lock_path.display(),
                e
            );
            std::process::exit(1);
        });
        let (entry_path, src) = read_package_entry_source(&man_dir).unwrap_or_else(|e| {
            eprintln!("pblang build: {}", e);
            std::process::exit(1);
        });
        let path_str = entry_path.to_string_lossy();
        match parse_strict(&src) {
            Ok(stmts) => {
                let diags = type_check(&stmts);
                if !diags.is_empty() {
                    for d in diags {
                        if let Some(sp) = d.span {
                            eprintln!("{}:{}..{}: {}", path_str, sp.start, sp.end, d.message);
                        } else {
                            eprintln!("{}: {}", path_str, d.message);
                        }
                    }
                    std::process::exit(1);
                }
            }
            Err(errs) => {
                print_parse_errors(&src, errs, Some(&path_str));
                std::process::exit(2);
            }
        }
        println!("build ok");
        return;
    }
    if args.first().map(|s| s.as_str()) == Some("bundle") {
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
        let lock =
            generate_lockfile_for_cli(&man_dir, registry_index_url, require_registry_sha256_cli)
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
                std::path::PathBuf::from(output)
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
                    let _ = std::fs::copy(entry.path(), &dest);
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
                eprintln!(
                    "pblang bundle --self-extract: cannot find parent directory of executable"
                );
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
        return;
    }

    if args.first().map(|s| s.as_str()) == Some("--check") {
        let Some(path) = args.get(1) else {
            eprintln!("usage: pblang --check <file>");
            std::process::exit(2);
        };
        let src = match std::fs::read_to_string(path) {
            Ok(src) => src,
            Err(e) => {
                eprintln!("pblang --check: failed to read '{}': {}", path, e);
                std::process::exit(1);
            }
        };
        match parse_strict(&src) {
            Ok(stmts) => {
                let diags = type_check(&stmts);
                if diags.is_empty() {
                    println!("typecheck ok");
                    return;
                }
                for d in diags {
                    if let Some(sp) = d.span {
                        eprintln!("{path}:{}..{}: {}", sp.start, sp.end, d.message);
                    } else {
                        eprintln!("{path}: {}", d.message);
                    }
                }
                std::process::exit(1);
            }
            Err(errs) => {
                print_parse_errors(&src, errs, Some(path));
                std::process::exit(2);
            }
        }
    }
    if args.first().map(|s| s.as_str()) == Some("--doc") {
        let Some(path) = args.get(1) else {
            eprintln!("usage: pblang --doc <file> [--html] [out_dir]");
            std::process::exit(2);
        };
        let html_mode = args.get(2).map(|s| s.as_str()) == Some("--html");
        let out_dir = if html_mode {
            args.get(3).map(String::as_str).unwrap_or("docs")
        } else {
            ""
        };
        let src = match std::fs::read_to_string(path) {
            Ok(src) => src,
            Err(e) => {
                eprintln!("pblang --doc: failed to read '{}': {}", path, e);
                std::process::exit(1);
            }
        };

        fn render_ty(t: &TypeExpr) -> String {
            match &t.kind {
                TypeKind::Any => "any".into(),
                TypeKind::Number => "number".into(),
                TypeKind::Bool => "bool".into(),
                TypeKind::String => "string".into(),
                TypeKind::Nil => "nil".into(),
                TypeKind::Array(inner) => format!("array<{}>", render_ty(inner)),
                TypeKind::Map(k, v) => format!("map<{}, {}>", render_ty(k), render_ty(v)),
                TypeKind::Option(inner) => format!("{}?", render_ty(inner)),
                TypeKind::Result(ok, err) => {
                    format!("result<{}, {}>", render_ty(ok), render_ty(err))
                }
                TypeKind::Generic(name) => name.clone(),
                TypeKind::NonNull(inner) => format!("{}!", render_ty(inner)),
            }
        }

        fn render_polyglot_attrs(attrs: &std::collections::HashMap<String, String>) -> String {
            if attrs.is_empty() {
                return "(none)".into();
            }
            let mut pairs = attrs
                .iter()
                .map(|(k, v)| format!("{k}={v}"))
                .collect::<Vec<_>>();
            pairs.sort();
            pairs.join(", ")
        }

        fn polyglot_code_line_count(code: &str) -> usize {
            if code.is_empty() {
                0
            } else {
                code.lines().count()
            }
        }

        match parse_with_polyglot(&src) {
            Ok((stmts, blocks)) => {
                let mut exports: Vec<String> = vec![];
                let mut fns: Vec<String> = vec![];

                for s in &stmts {
                    match &s.kind {
                        StmtKind::Export(names) => {
                            exports.extend(names.iter().cloned());
                        }
                        StmtKind::Fn(name, params, ret, _body) => {
                            let ps = params
                                .iter()
                                .map(|p| match &p.ty {
                                    Some(t) => format!("{}: {}", p.name, render_ty(t)),
                                    None => p.name.clone(),
                                })
                                .collect::<Vec<_>>()
                                .join(", ");
                            let r = ret.as_ref().map(render_ty);
                            let sig = if let Some(r) = r {
                                format!("fn {name}({ps}) -> {r}")
                            } else {
                                format!("fn {name}({ps})")
                            };
                            fns.push(sig);
                        }
                        _ => {}
                    }
                }

                exports.sort();
                exports.dedup();
                fns.sort();

                if html_mode {
                    let mut html = String::from(
                        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <title>Biapryx module docs</title>
  <style>
    body { font-family: system-ui, sans-serif; max-width: 800px; margin: 1rem auto; padding: 0 1rem; }
    h1 { font-size: 1.5rem; }
    h2 { font-size: 1.2rem; margin-top: 1.5rem; }
    code { background: #f0f0f0; padding: 0.1em 0.3em; border-radius: 3px; }
    ul { list-style: none; padding-left: 0; }
    li { margin: 0.3em 0; }
  </style>
</head>
<body>
  <h1>Module docs</h1>
  <p><strong>file</strong>: <code>"#,
                    );
                    html.push_str(path);
                    html.push_str(
                        r#"</code></p>
  <h2>Exports</h2>
  <ul>
"#,
                    );
                    if exports.is_empty() {
                        html.push_str("    <li>(none)</li>\n");
                    } else {
                        for e in &exports {
                            html.push_str("    <li><code>");
                            html.push_str(&e.replace('<', "&lt;").replace('>', "&gt;"));
                            html.push_str("</code></li>\n");
                        }
                    }
                    html.push_str("  </ul>\n  <h2>Top-level functions</h2>\n  <ul>\n");
                    if fns.is_empty() {
                        html.push_str("    <li>(none)</li>\n");
                    } else {
                        for f in &fns {
                            html.push_str("    <li><code>");
                            html.push_str(&f.replace('<', "&lt;").replace('>', "&gt;"));
                            html.push_str("</code></li>\n");
                        }
                    }
                    html.push_str("  </ul>\n  <h2>Polyglot blocks</h2>\n  <ul>\n");
                    if blocks.is_empty() {
                        html.push_str("    <li>(none)</li>\n");
                    } else {
                        for b in blocks {
                            let name = b.name.as_deref().unwrap_or("<unnamed>");
                            let block_args = b.args.as_deref().unwrap_or("");
                            let attrs = render_polyglot_attrs(&b.attrs);
                            let lines = polyglot_code_line_count(&b.code);
                            html.push_str(&format!(
                                "    <li><code>@{} {} {}</code> (span {}..{}, attrs: {}, code lines: {})</li>\n",
                                b.lang, name, block_args, b.span.start, b.span.end, attrs, lines
                            ));
                        }
                    }
                    html.push_str("  </ul>\n</body>\n</html>\n");

                    let out_path = std::path::Path::new(out_dir);
                    let _ = std::fs::create_dir_all(out_path);
                    let index = out_path.join("index.html");
                    if let Err(e) = std::fs::write(&index, html) {
                        eprintln!(
                            "pblang --doc --html: failed to write {}: {}",
                            index.display(),
                            e
                        );
                        std::process::exit(1);
                    }
                    println!("wrote {}", index.display());
                } else {
                    println!("## Module docs");
                    println!();
                    println!("- **file**: `{path}`");
                    println!();
                    println!("### Exports");
                    if exports.is_empty() {
                        println!();
                        println!("(none)");
                    } else {
                        for e in &exports {
                            println!("- `{e}`");
                        }
                    }
                    println!();
                    println!("### Top-level functions");
                    if fns.is_empty() {
                        println!();
                        println!("(none)");
                    } else {
                        for f in &fns {
                            println!("- `{f}`");
                        }
                    }
                    println!();
                    println!("### Polyglot blocks");
                    if blocks.is_empty() {
                        println!();
                        println!("(none)");
                    } else {
                        for b in blocks {
                            let name = b.name.as_deref().unwrap_or("<unnamed>");
                            let block_args = b.args.as_deref().unwrap_or("");
                            let attrs = render_polyglot_attrs(&b.attrs);
                            let lines = polyglot_code_line_count(&b.code);
                            println!(
                                "- `@{} {}` `{}` (span {}..{}, attrs: {}, code lines: {})",
                                b.lang, name, block_args, b.span.start, b.span.end, attrs, lines
                            );
                        }
                    }
                }
                return;
            }
            Err(errs) => {
                print_parse_errors(&src, errs, Some(path));
                std::process::exit(2);
            }
        }
    }

    // Format .sap/.sapp files: trim, spacing around operators, indent by brace depth (4 spaces), optional line wrap, ensure final newline

    // ── pblang serve — Start IDE backend server ────────────────────────
    if args.first().map(|s| s.as_str()) == Some("serve") {
        let mut port: u16 = 8090;
        let mut i = 1usize;
        while i < args.len() {
            match args[i].as_str() {
                "--port" | "-p" => {
                    i += 1;
                    if i >= args.len() {
                        eprintln!("pblang serve: --port requires a value");
                        eprintln!("usage: pblang serve [--port <1-65535>]");
                        std::process::exit(2);
                    }
                    match args[i].parse::<u16>() {
                        Ok(p) if p > 0 => port = p,
                        _ => {
                            eprintln!("pblang serve: invalid port '{}'", args[i]);
                            eprintln!("usage: pblang serve [--port <1-65535>]");
                            std::process::exit(2);
                        }
                    }
                }
                other => match other.parse::<u16>() {
                    Ok(p) if p > 0 => port = p,
                    _ => {
                        eprintln!("pblang serve: unrecognized argument '{}'", other);
                        eprintln!("usage: pblang serve [--port <1-65535>]");
                        std::process::exit(2);
                    }
                },
            }
            i += 1;
        }
        println!("pblang IDE Server starting on http://localhost:{}", port);
        println!("Open editors/ide/index.html in your browser");

        let listener =
            std::net::TcpListener::bind(format!("127.0.0.1:{}", port)).unwrap_or_else(|e| {
                eprintln!("Cannot bind to port {}: {}", port, e);
                std::process::exit(1);
            });
        let cors_origin = format!("http://localhost:{port}");
        let ide_examples = [
            (
                "hello_world",
                "Hello World",
                "examples/hello_world/main.sapp",
            ),
            ("tutorial_cli", "Tutorial: CLI", "examples/tutorial_cli/main.sapp"),
            (
                "tutorial_concurrency",
                "Tutorial: Concurrency",
                "examples/tutorial_concurrency/main.sapp",
            ),
            (
                "tutorial_http",
                "Tutorial: HTTP",
                "examples/tutorial_http/main.sapp",
            ),
            (
                "tutorial_user_input",
                "Tutorial: User Input",
                "examples/tutorial_user_input/main.sapp",
            ),
            ("thread_pool", "Thread Pool", "examples/thread_pool/main.sapp"),
            ("web_server", "Web Server", "examples/web_server/main.sapp"),
            ("orm_example", "ORM Example", "examples/orm_example/main.sapp"),
            ("quantum_bb84", "Quantum BB84", "examples/quantum_bb84/main.sapp"),
            (
                "concurrent_tasks",
                "Concurrent Tasks",
                "examples/concurrent_tasks/main.sapp",
            ),
            (
                "ai_http_provider",
                "AI HTTP Provider",
                "examples/ai_http_provider/main.sapp",
            ),
            (
                "ai_chat_toolcall",
                "AI Chat Toolcall",
                "examples/ai_chat_toolcall/main.sapp",
            ),
        ];
        let manual_docs = [
            ("ide", "IDE Manual", "docs/IDE_MANUAL.md"),
            ("project", "Project Manual", "docs/PROJECT_MANUAL.md"),
        ];
        let max_workers = std::thread::available_parallelism()
            .map(|n| n.get().clamp(2, 16))
            .unwrap_or(4);
        let serve_pool = threadpool::ThreadPool::new(max_workers);
        let max_inflight = max_workers * 8;
        let inflight = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        struct InflightPermit {
            counter: std::sync::Arc<std::sync::atomic::AtomicUsize>,
        }
        impl Drop for InflightPermit {
            fn drop(&mut self) {
                self.counter
                    .fetch_sub(1, std::sync::atomic::Ordering::AcqRel);
            }
        }
        println!("pblang serve: concurrency limit set to {}", max_workers);
        println!("pblang serve: inflight cap set to {}", max_inflight);

        for stream in listener.incoming() {
            match stream {
                Ok(mut stream) => {
                    let acquired = loop {
                        let cur = inflight.load(std::sync::atomic::Ordering::Acquire);
                        if cur >= max_inflight {
                            break false;
                        }
                        if inflight
                            .compare_exchange_weak(
                                cur,
                                cur + 1,
                                std::sync::atomic::Ordering::AcqRel,
                                std::sync::atomic::Ordering::Acquire,
                            )
                            .is_ok()
                        {
                            break true;
                        }
                    };
                    if !acquired {
                        let resp_body = serde_json::json!({
                            "ok": false,
                            "error": "server busy; retry later"
                        })
                        .to_string();
                        let response = format!(
                            "HTTP/1.1 503 Service Unavailable\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: *\r\nRetry-After: 1\r\nContent-Length: {}\r\n\r\n{}",
                            resp_body.len(),
                            resp_body
                        );
                        write_http_response_with_local_cors(&mut stream, &response, &cors_origin);
                        continue;
                    }

                    let cors_origin = cors_origin.clone();
                    let inflight = inflight.clone();
                    serve_pool.execute(move || {
                        let _permit = InflightPermit { counter: inflight };
                        for _req in 0..1 {
                            use std::io::Read;
                            const MAX_HEADER_BYTES: usize = 64 * 1024;
                            const MAX_BODY_BYTES: usize = 10 * 1024 * 1024;
                            const READ_TIMEOUT_MS: u64 = 5_000;
                            let _ = stream.set_read_timeout(Some(std::time::Duration::from_millis(
                                READ_TIMEOUT_MS,
                            )));
                            let mut request_bytes: Vec<u8> = Vec::new();
                            let mut chunk = [0u8; 4096];
                            let mut header_end: Option<usize> = None;
                            let mut content_length: usize = 0;
                            let mut saw_content_length = false;
                            let mut malformed_content_length = false;
                            let mut header_too_large = false;
                            let mut body_too_large = false;
                            loop {
                                match stream.read(&mut chunk) {
                                    Ok(0) => break,
                                    Ok(n) => {
                                        request_bytes.extend_from_slice(&chunk[..n]);
                                        if header_end.is_none()
                                            && request_bytes.len() > MAX_HEADER_BYTES
                                        {
                                            header_too_large = true;
                                            break;
                                        }
                                        if header_end.is_none() {
                                            if let Some(pos) =
                                                request_bytes.windows(4).position(|w| w == b"\r\n\r\n")
                                            {
                                                header_end = Some(pos + 4);
                                                let header =
                                                    String::from_utf8_lossy(&request_bytes[..pos + 4]);
                                                for line in header.lines() {
                                                    let line_trimmed = line.trim();
                                                    if let Some(v) =
                                                        line_trimmed.strip_prefix("Content-Length:")
                                                    {
                                                        saw_content_length = true;
                                                        match v.trim().parse::<usize>() {
                                                            Ok(n) => content_length = n,
                                                            Err(_) => {
                                                                malformed_content_length = true;
                                                                break;
                                                            }
                                                        }
                                                        break;
                                                    }
                                                    let lower = line_trimmed.to_ascii_lowercase();
                                                    if let Some(v) =
                                                        lower.strip_prefix("content-length:")
                                                    {
                                                        saw_content_length = true;
                                                        match v.trim().parse::<usize>() {
                                                            Ok(n) => content_length = n,
                                                            Err(_) => {
                                                                malformed_content_length = true;
                                                                break;
                                                            }
                                                        }
                                                        break;
                                                    }
                                                }
                                                if malformed_content_length {
                                                    break;
                                                }
                                                if content_length > MAX_BODY_BYTES {
                                                    body_too_large = true;
                                                    break;
                                                }
                                            }
                                        }
                                        if let Some(h_end) = header_end {
                                            if request_bytes.len() >= h_end + content_length {
                                                break;
                                            }
                                        }
                                        if request_bytes.len() > MAX_HEADER_BYTES + MAX_BODY_BYTES {
                                            break;
                                        }
                                    }
                                    Err(_) => break,
                                }
                            }
                            if !request_bytes.is_empty() {
                    if header_too_large {
                        let resp_body = serde_json::json!({
                            "ok": false,
                            "error": "request headers too large"
                        })
                        .to_string();
                        let response = format!(
                            "HTTP/1.1 431 Request Header Fields Too Large\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: {}\r\n\r\n{}",
                            resp_body.len(), resp_body
                        );
                        write_http_response_with_local_cors(&mut stream, &response, &cors_origin);
                        continue;
                    }
                    if malformed_content_length {
                        let resp_body = serde_json::json!({
                            "ok": false,
                            "error": "invalid Content-Length header"
                        })
                        .to_string();
                        let response = format!(
                            "HTTP/1.1 400 Bad Request\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: {}\r\n\r\n{}",
                            resp_body.len(), resp_body
                        );
                        write_http_response_with_local_cors(&mut stream, &response, &cors_origin);
                        continue;
                    }
                    if body_too_large {
                        let resp_body = serde_json::json!({
                            "ok": false,
                            "error": format!("request body too large (max {} bytes)", MAX_BODY_BYTES)
                        })
                        .to_string();
                        let response = format!(
                            "HTTP/1.1 413 Payload Too Large\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: {}\r\n\r\n{}",
                            resp_body.len(), resp_body
                        );
                        write_http_response_with_local_cors(&mut stream, &response, &cors_origin);
                        continue;
                    }
                    let request = String::from_utf8_lossy(&request_bytes);
                    let request_line = request.lines().next().unwrap_or("");
                    if request_line.starts_with("POST ") && !saw_content_length {
                        let resp_body = serde_json::json!({
                            "ok": false,
                            "error": "missing Content-Length for POST request"
                        })
                        .to_string();
                        let response = format!(
                            "HTTP/1.1 411 Length Required\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: {}\r\n\r\n{}",
                            resp_body.len(), resp_body
                        );
                        write_http_response_with_local_cors(&mut stream, &response, &cors_origin);
                        continue;
                    }
                    let request_path = request_line.split_whitespace().nth(1).unwrap_or("/");
                    let body_start = request.find("\r\n\r\n").map(|n| n + 4).unwrap_or(request.len());
                    let body = &request[body_start..];

                    if request.starts_with("POST /run") {
                        let started = std::time::Instant::now();
                        let mut json_resp = serde_json::json!({
                            "ok": false,
                            "stage": "request",
                            "output": "",
                            "diagnostics": [],
                            "error": "invalid request body",
                            "duration_ms": 0
                        });
                        if let Ok(json) = serde_json::from_str::<serde_json::Value>(body.trim()) {
                            if let Some(code) = json.get("code").and_then(|c| c.as_str()) {
                                let code = normalize_ide_source(code);
                                let stdin_lines: Vec<String> = json
                                    .get("stdin_lines")
                                    .and_then(|v| v.as_array())
                                    .map(|arr| {
                                        arr.iter()
                                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                                            .collect()
                                    })
                                    .unwrap_or_default();
                                match pblang_core::parse_with_polyglot(&code) {
                                    Ok((stmts, _)) => {
                                        let type_diags = type_check(&stmts);
                                        if !type_diags.is_empty() {
                                            let diagnostics: Vec<serde_json::Value> = type_diags
                                                .into_iter()
                                                .map(|d| {
                                                    if let Some(sp) = d.span {
                                                        let (line, col) = line_col(&code, sp.start);
                                                        serde_json::json!({
                                                            "stage": "type",
                                                            "message": d.message,
                                                            "span": { "start": sp.start, "end": sp.end },
                                                            "line": line,
                                                            "col": col,
                                                            "line_text": get_line(&code, line)
                                                        })
                                                    } else {
                                                        serde_json::json!({
                                                            "stage": "type",
                                                            "message": d.message
                                                        })
                                                    }
                                                })
                                                .collect();
                                            json_resp = serde_json::json!({
                                                "ok": false,
                                                "stage": "type",
                                                "output": "",
                                                "error": "Type error",
                                                "diagnostics": diagnostics
                                            });
                                        } else {
                                            match pblang_compiler::compile(stmts) {
                                                Ok(chunk) => {
                                                    let mut vm = pblang_vm::VM::with_stdlib(
                                                        pblang_vm::StdlibConfig::default(),
                                                    );
                                                    vm.set_limits(Some(VmLimits {
                                                        max_instructions: Some(5_000_000),
                                                        max_stack_depth: Some(2048),
                                                        max_memory_bytes: Some(64 * 1024 * 1024),
                                                        io_timeout_ms: Some(5_000),
                                                    }));
                                                    if !stdin_lines.is_empty() {
                                                        vm.set_stdin_lines(stdin_lines.clone());
                                                    }
                                                    match vm.run_collect(chunk) {
                                                        Ok(values) => {
                                                            let mut output = String::new();
                                                            for v in values {
                                                                output.push_str(&format!("{}\n", v));
                                                            }
                                                            json_resp = serde_json::json!({
                                                                "ok": true,
                                                                "stage": "ok",
                                                                "output": output.trim(),
                                                                "diagnostics": []
                                                            });
                                                        }
                                                        Err(e) => {
                                                            let mut diagnostics = Vec::new();
                                                            if let Some(sp) = e.span {
                                                                let (line, col) = line_col(&code, sp.start);
                                                                diagnostics.push(serde_json::json!({
                                                                    "stage": "runtime",
                                                                    "message": e.message,
                                                                    "span": { "start": sp.start, "end": sp.end },
                                                                    "line": line,
                                                                    "col": col,
                                                                    "line_text": get_line(&code, line)
                                                                }));
                                                            }
                                                            json_resp = serde_json::json!({
                                                                "ok": false,
                                                                "stage": "runtime",
                                                                "output": "",
                                                                "error": format!("Runtime error: {}", e.message),
                                                                "diagnostics": diagnostics
                                                            });
                                                        }
                                                    }
                                                }
                                                Err(e) => {
                                                    json_resp = serde_json::json!({
                                                        "ok": false,
                                                        "stage": "compile",
                                                        "output": "",
                                                        "error": format!("Compile error: {}", e),
                                                        "diagnostics": []
                                                    });
                                                }
                                            }
                                        }
                                    }
                                    Err(errs) => {
                                        let diagnostics: Vec<serde_json::Value> = errs
                                            .into_iter()
                                            .map(|err| {
                                                let sp = err.span();
                                                let (line, col) = line_col(&code, sp.start);
                                                serde_json::json!({
                                                    "stage": "parse",
                                                    "message": format_parse_error(&err),
                                                    "span": { "start": sp.start, "end": sp.end },
                                                    "line": line,
                                                    "col": col,
                                                    "line_text": get_line(&code, line)
                                                })
                                            })
                                            .collect();
                                        json_resp = serde_json::json!({
                                            "ok": false,
                                            "stage": "parse",
                                            "output": "",
                                            "error": "Parse error",
                                            "diagnostics": diagnostics
                                        });
                                    }
                                }
                            }
                        }

                        let elapsed_ms = started.elapsed().as_millis() as u64;
                        if let Some(obj) = json_resp.as_object_mut() {
                            obj.insert("duration_ms".into(), serde_json::json!(elapsed_ms));
                        }
                        let resp_body = serde_json::to_string(&json_resp).unwrap_or_default();
                        let response = format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: {}\r\n\r\n{}",
                            resp_body.len(), resp_body
                        );
                        write_http_response_with_local_cors(&mut stream, &response, &cors_origin);
                    } else if request.starts_with("POST /check") {
                        // Parse + typecheck only (no execution), for IDE diagnostics.
                        let mut json_resp = serde_json::json!({
                            "ok": false,
                            "stage": "request",
                            "diagnostics": [],
                            "error": "invalid request body"
                        });
                        if let Ok(json) = serde_json::from_str::<serde_json::Value>(body.trim()) {
                            if let Some(code) = json.get("code").and_then(|c| c.as_str()) {
                                let code = normalize_ide_source(code);
                                match pblang_core::parse_with_polyglot(&code) {
                                    Ok((stmts, _)) => {
                                        let diags = type_check(&stmts);
                                        if diags.is_empty() {
                                            json_resp = serde_json::json!({
                                                "ok": true,
                                                "stage": "ok",
                                                "diagnostics": []
                                            });
                                        } else {
                                            let diagnostics: Vec<serde_json::Value> = diags
                                                .into_iter()
                                                .map(|d| {
                                                    if let Some(sp) = d.span {
                                                        let (line, col) = line_col(&code, sp.start);
                                                        serde_json::json!({
                                                            "stage": "type",
                                                            "message": d.message,
                                                            "span": { "start": sp.start, "end": sp.end },
                                                            "line": line,
                                                            "col": col,
                                                            "line_text": get_line(&code, line)
                                                        })
                                                    } else {
                                                        serde_json::json!({
                                                            "stage": "type",
                                                            "message": d.message
                                                        })
                                                    }
                                                })
                                                .collect();
                                            json_resp = serde_json::json!({
                                                "ok": false,
                                                "stage": "type",
                                                "diagnostics": diagnostics
                                            });
                                        }
                                    }
                                    Err(errs) => {
                                        let diagnostics: Vec<serde_json::Value> = errs
                                            .into_iter()
                                            .map(|err| {
                                                let sp = err.span();
                                                let (line, col) = line_col(&code, sp.start);
                                                serde_json::json!({
                                                    "stage": "parse",
                                                    "message": format_parse_error(&err),
                                                    "span": { "start": sp.start, "end": sp.end },
                                                    "line": line,
                                                    "col": col,
                                                    "line_text": get_line(&code, line)
                                                })
                                            })
                                            .collect();
                                        json_resp = serde_json::json!({
                                            "ok": false,
                                            "stage": "parse",
                                            "diagnostics": diagnostics
                                        });
                                    }
                                }
                            }
                        }
                        let resp_body = serde_json::to_string(&json_resp).unwrap_or_default();
                        let response = format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: {}\r\n\r\n{}",
                            resp_body.len(), resp_body
                        );
                        write_http_response_with_local_cors(&mut stream, &response, &cors_origin);
                    } else if request.starts_with("POST /format") {
                        let mut json_resp = serde_json::json!({
                            "ok": false,
                            "error": "invalid request body",
                            "code": ""
                        });
                        if let Ok(json) = serde_json::from_str::<serde_json::Value>(body.trim()) {
                            if let Some(code) = json.get("code").and_then(|c| c.as_str()) {
                                let max_line_length = json
                                    .get("max_line_length")
                                    .and_then(|v| v.as_u64())
                                    .and_then(|n| usize::try_from(n).ok());
                                let normalized = normalize_ide_source(code);
                                let formatted = format_ide_source(&normalized, max_line_length);
                                json_resp = serde_json::json!({
                                    "ok": true,
                                    "code": formatted
                                });
                            }
                        }
                        let resp_body = serde_json::to_string(&json_resp).unwrap_or_default();
                        let response = format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: {}\r\n\r\n{}",
                            resp_body.len(), resp_body
                        );
                        write_http_response_with_local_cors(&mut stream, &response, &cors_origin);
                    } else if request.starts_with("POST /normalize") {
                        let mut json_resp = serde_json::json!({
                            "ok": false,
                            "error": "invalid request body",
                            "code": ""
                        });
                        if let Ok(json) = serde_json::from_str::<serde_json::Value>(body.trim()) {
                            if let Some(code) = json.get("code").and_then(|c| c.as_str()) {
                                let normalized = normalize_ide_source(code);
                                json_resp = serde_json::json!({
                                    "ok": true,
                                    "code": normalized
                                });
                            }
                        }
                        let resp_body = serde_json::to_string(&json_resp).unwrap_or_default();
                        let response = format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: {}\r\n\r\n{}",
                            resp_body.len(), resp_body
                        );
                        write_http_response_with_local_cors(&mut stream, &response, &cors_origin);
                    } else if request_path == "/examples" {
                        let mut arr = Vec::new();
                        for (id, title, rel_path) in ide_examples {
                            if std::path::Path::new(rel_path).exists() {
                                arr.push(serde_json::json!({
                                    "id": id,
                                    "title": title
                                }));
                            }
                        }
                        let json_resp = serde_json::json!({
                            "ok": true,
                            "examples": arr
                        });
                        let resp_body = serde_json::to_string(&json_resp).unwrap_or_default();
                        let response = format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: {}\r\n\r\n{}",
                            resp_body.len(), resp_body
                        );
                        write_http_response_with_local_cors(&mut stream, &response, &cors_origin);
                    } else if request_path.starts_with("/examples/") {
                        let id = request_path.trim_start_matches("/examples/").trim_end_matches('/');
                        let selected = ide_examples
                            .iter()
                            .find(|(example_id, _, _)| *example_id == id);
                        if let Some((example_id, title, rel_path)) = selected {
                            match std::fs::read_to_string(rel_path) {
                                Ok(code) => {
                                    let json_resp = serde_json::json!({
                                        "ok": true,
                                        "id": example_id,
                                        "title": title,
                                        "code": code
                                    });
                                    let resp_body =
                                        serde_json::to_string(&json_resp).unwrap_or_default();
                                    let response = format!(
                                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: {}\r\n\r\n{}",
                                        resp_body.len(), resp_body
                                    );
                                    write_http_response_with_local_cors(
                                        &mut stream,
                                        &response,
                                        &cors_origin,
                                    );
                                }
                                Err(e) => {
                                    let json_resp = serde_json::json!({
                                        "ok": false,
                                        "error": format!("failed to read selected example: {}", e)
                                    });
                                    let resp_body =
                                        serde_json::to_string(&json_resp).unwrap_or_default();
                                    let response = format!(
                                        "HTTP/1.1 500 Internal Server Error\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: {}\r\n\r\n{}",
                                        resp_body.len(), resp_body
                                    );
                                    write_http_response_with_local_cors(
                                        &mut stream,
                                        &response,
                                        &cors_origin,
                                    );
                                }
                            }
                        } else {
                            let json_resp = serde_json::json!({
                                "ok": false,
                                "error": "unknown example id"
                            });
                            let resp_body = serde_json::to_string(&json_resp).unwrap_or_default();
                            let response = format!(
                                "HTTP/1.1 404 Not Found\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: {}\r\n\r\n{}",
                                resp_body.len(), resp_body
                            );
                            write_http_response_with_local_cors(&mut stream, &response, &cors_origin);
                        }
                    } else if request_path.starts_with("/manual/") {
                        let id = request_path
                            .trim_start_matches("/manual/")
                            .trim_end_matches('/');
                        let selected = manual_docs.iter().find(|(doc_id, _, _)| *doc_id == id);
                        if let Some((doc_id, title, rel_path)) = selected {
                            match std::fs::read_to_string(rel_path) {
                                Ok(content) => {
                                    let json_resp = serde_json::json!({
                                        "ok": true,
                                        "id": doc_id,
                                        "title": title,
                                        "content": content
                                    });
                                    let resp_body =
                                        serde_json::to_string(&json_resp).unwrap_or_default();
                                    let response = format!(
                                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: {}\r\n\r\n{}",
                                        resp_body.len(), resp_body
                                    );
                                    write_http_response_with_local_cors(
                                        &mut stream,
                                        &response,
                                        &cors_origin,
                                    );
                                }
                                Err(e) => {
                                    let json_resp = serde_json::json!({
                                        "ok": false,
                                        "error": format!("failed to read selected manual: {}", e)
                                    });
                                    let resp_body =
                                        serde_json::to_string(&json_resp).unwrap_or_default();
                                    let response = format!(
                                        "HTTP/1.1 500 Internal Server Error\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: {}\r\n\r\n{}",
                                        resp_body.len(), resp_body
                                    );
                                    write_http_response_with_local_cors(
                                        &mut stream,
                                        &response,
                                        &cors_origin,
                                    );
                                }
                            }
                        } else {
                            let json_resp = serde_json::json!({
                                "ok": false,
                                "error": "unknown manual id"
                            });
                            let resp_body = serde_json::to_string(&json_resp).unwrap_or_default();
                            let response = format!(
                                "HTTP/1.1 404 Not Found\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: {}\r\n\r\n{}",
                                resp_body.len(), resp_body
                            );
                            write_http_response_with_local_cors(&mut stream, &response, &cors_origin);
                        }
                    } else if request.starts_with("GET /health") {
                        let json_resp = serde_json::json!({"ok": true, "service": "pblang-ide-backend"});
                        let resp_body = serde_json::to_string(&json_resp).unwrap_or_default();
                        let response = format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: {}\r\n\r\n{}",
                            resp_body.len(), resp_body
                        );
                        write_http_response_with_local_cors(&mut stream, &response, &cors_origin);
                    } else if request.starts_with("OPTIONS") {
                        let response = "HTTP/1.1 204 No Content\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: GET, POST, OPTIONS\r\nAccess-Control-Allow-Headers: Content-Type\r\n\r\n";
                        write_http_response_with_local_cors(&mut stream, response, &cors_origin);
                    } else {
                        // Serve IDE HTML for GET /
                        let html = include_str!("../editors/ide/index.html");
                        let response = format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: {}\r\n\r\n{}",
                            html.len(), html
                        );
                        write_http_response_with_local_cors(&mut stream, &response, &cors_origin);
                    }
                            }
                        }
                    });
                }
                Err(e) => {
                    eprintln!("pblang serve: failed to accept connection: {}", e);
                }
            }
        }
        return;
    }

    if args.first().map(|s| s.as_str()) == Some("fmt") {
        let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        let mut max_line_length: Option<usize> = None;
        let mut files: Vec<std::path::PathBuf> = Vec::new();
        for a in args.iter().skip(1) {
            if let Some(rest) = a.strip_prefix("--max-line-length=") {
                max_line_length = Some(rest.parse().unwrap_or(100));
            } else if a.as_str() != "--max-line-length" {
                files.push(cwd.join(a));
            }
        }
        if files.is_empty() {
            eprintln!(
                "pblang fmt: pass one or more .sap, .sapp, or .bpx files (optional: --max-line-length=N)"
            );
            std::process::exit(1);
        }
        const INDENT: &str = "    ";

        fn is_top_level_fn_header(trimmed: &str) -> bool {
            trimmed.starts_with("fn ") || trimmed.starts_with("export fn ")
        }

        for path in files {
            let path = path.canonicalize().unwrap_or_else(|_| path.clone());
            if path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e == "sap" || e == "sapp" || e == "bpx")
                .unwrap_or(false)
            {
                match std::fs::read_to_string(&path) {
                    Ok(src) => {
                        let lines: Vec<&str> = src.lines().collect();
                        let mut out = String::new();
                        let mut depth: i32 = 0;
                        let mut in_string = false;
                        let mut in_single_string = false;
                        let mut block_comment_depth = 0;
                        let mut last_line_was_blank = false;
                        let mut seen_toplevel_fn = false;
                        for line in &lines {
                            let mut trimmed = line.trim().to_string();
                            trimmed = fmt_normalize_operator_spacing(&trimmed);
                            trimmed = fmt_normalize_comma_spacing(&trimmed);
                            if trimmed.is_empty() {
                                out.push('\n');
                                last_line_was_blank = true;
                                continue;
                            }
                            if depth == 0 && is_top_level_fn_header(&trimmed) {
                                if seen_toplevel_fn && !last_line_was_blank {
                                    out.push('\n');
                                }
                                seen_toplevel_fn = true;
                            }
                            if trimmed.starts_with('}') {
                                depth = depth.saturating_sub(1);
                            }
                            let indent_str: String =
                                (0..depth.max(0) as usize).map(|_| INDENT).collect();
                            let full_line = format!("{}{}", indent_str, trimmed);
                            if let Some(max_len) = max_line_length {
                                let mut line_to_output = full_line;
                                while !line_to_output.is_empty() {
                                    if line_to_output.len() <= max_len {
                                        out.push_str(&line_to_output);
                                        out.push('\n');
                                        scan_line_for_braces(
                                            &line_to_output,
                                            &mut in_string,
                                            &mut in_single_string,
                                            &mut block_comment_depth,
                                            &mut depth,
                                        );
                                        break;
                                    }
                                    let break_at =
                                        line_to_output[..max_len].rfind(' ').unwrap_or(max_len);
                                    let break_at = if break_at == 0 { max_len } else { break_at };
                                    let rest = line_to_output.split_off(break_at);
                                    let emitted = line_to_output.trim_end().to_string();
                                    out.push_str(&emitted);
                                    out.push('\n');
                                    scan_line_for_braces(
                                        &emitted,
                                        &mut in_string,
                                        &mut in_single_string,
                                        &mut block_comment_depth,
                                        &mut depth,
                                    );
                                    line_to_output = format!("{}{}", indent_str, rest.trim_start());
                                }
                            } else {
                                out.push_str(&full_line);
                                out.push('\n');
                                scan_line_for_braces(
                                    &full_line,
                                    &mut in_string,
                                    &mut in_single_string,
                                    &mut block_comment_depth,
                                    &mut depth,
                                );
                            }
                            last_line_was_blank = false;
                        }
                        if !out.is_empty() && !out.ends_with('\n') {
                            out.push('\n');
                        }
                        if out != src {
                            if let Err(e) = std::fs::write(&path, &out) {
                                eprintln!("pblang fmt: failed to write {}: {}", path.display(), e);
                                std::process::exit(1);
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("pblang fmt: failed to read {}: {}", path.display(), e);
                        std::process::exit(1);
                    }
                }
            }
        }
        return;
    }

    fn lint_unused_warnings(stmts: &[Stmt]) -> Vec<(Span, String)> {
        let mut warnings = vec![];
        // (span, is_import, read)
        let mut scope: Vec<std::collections::HashMap<String, (Span, bool, bool)>> = vec![];
        scope.push(std::collections::HashMap::new());

        fn mark_read(
            name: &str,
            scope: &mut [std::collections::HashMap<String, (Span, bool, bool)>],
        ) {
            for frame in scope.iter_mut().rev() {
                if let Some((_, _, read)) = frame.get_mut(name) {
                    *read = true;
                    return;
                }
            }
        }

        fn is_import_call(e: &Expr) -> bool {
            if let ExprKind::Call(callee, _) = &e.kind {
                if let ExprKind::Var(s) = &callee.kind {
                    return s == "import";
                }
            }
            false
        }

        fn walk_expr(
            e: &Expr,
            scope: &mut Vec<std::collections::HashMap<String, (Span, bool, bool)>>,
            warnings: &mut Vec<(Span, String)>,
        ) {
            match &e.kind {
                ExprKind::Var(name) => mark_read(name, scope),
                ExprKind::Call(callee, args) => {
                    walk_expr(callee, scope, warnings);
                    for a in args {
                        walk_expr(a, scope, warnings);
                    }
                }
                ExprKind::GetAttr(obj, _) => walk_expr(obj, scope, warnings),
                ExprKind::Index(obj, ix) => {
                    walk_expr(obj, scope, warnings);
                    walk_expr(ix, scope, warnings);
                }
                ExprKind::Add(a, b)
                | ExprKind::Sub(a, b)
                | ExprKind::Mul(a, b)
                | ExprKind::Div(a, b) => {
                    walk_expr(a, scope, warnings);
                    walk_expr(b, scope, warnings);
                }
                ExprKind::Eq(a, b)
                | ExprKind::NotEq(a, b)
                | ExprKind::Gt(a, b)
                | ExprKind::Ge(a, b)
                | ExprKind::Lt(a, b)
                | ExprKind::Le(a, b) => {
                    walk_expr(a, scope, warnings);
                    walk_expr(b, scope, warnings);
                }
                ExprKind::Logical { left, right, .. } => {
                    walk_expr(left, scope, warnings);
                    walk_expr(right, scope, warnings);
                }
                ExprKind::Try(try_e, catch_e) => {
                    walk_expr(try_e, scope, warnings);
                    walk_expr(catch_e, scope, warnings);
                }
                ExprKind::Match(scrut, arms, else_e) => {
                    walk_expr(scrut, scope, warnings);
                    for (_arm, e) in arms {
                        walk_expr(e, scope, warnings);
                    }
                    if let Some(ee) = else_e {
                        walk_expr(ee, scope, warnings);
                    }
                }
                ExprKind::Array(arr) => {
                    for a in arr {
                        walk_expr(a, scope, warnings);
                    }
                }
                ExprKind::Map(entries) => {
                    for (_, v) in entries {
                        walk_expr(v, scope, warnings);
                    }
                }
                ExprKind::FnLit(_, _, _, body) => {
                    scope.push(std::collections::HashMap::new());
                    for s in body {
                        walk_stmt(s, scope, warnings);
                    }
                    let top = scope
                        .pop()
                        .expect("scope stack underflow: push/pop mismatch in lint walk");
                    for (name, (sp, is_import, read)) in top {
                        if !read {
                            let msg = if is_import {
                                format!("unused import: `{}`", name)
                            } else {
                                format!("unused variable: `{}`", name)
                            };
                            warnings.push((sp, msg));
                        }
                    }
                }
                ExprKind::BlockExpr(stmts_in, opt_final) => {
                    scope.push(std::collections::HashMap::new());
                    for s in stmts_in {
                        walk_stmt(s, scope, warnings);
                    }
                    if let Some(e) = opt_final {
                        walk_expr(e, scope, warnings);
                    }
                    let top = scope
                        .pop()
                        .expect("scope stack underflow: push/pop mismatch in lint walk");
                    for (name, (sp, is_import, read)) in top {
                        if !read {
                            let msg = if is_import {
                                format!("unused import: `{}`", name)
                            } else {
                                format!("unused variable: `{}`", name)
                            };
                            warnings.push((sp, msg));
                        }
                    }
                }
                _ => {}
            }
        }

        fn walk_stmt(
            s: &Stmt,
            scope: &mut Vec<std::collections::HashMap<String, (Span, bool, bool)>>,
            warnings: &mut Vec<(Span, String)>,
        ) {
            match &s.kind {
                StmtKind::Let(name, _, init) => {
                    if scope.len() > 1 {
                        for frame in scope[..scope.len() - 1].iter() {
                            if frame.contains_key(name) {
                                warnings.push((
                                    s.span,
                                    format!("variable `{}` shadows outer binding", name),
                                ));
                                break;
                            }
                        }
                    }
                    let is_import = is_import_call(init);
                    scope
                        .last_mut()
                        .expect("lint walk: scope is non-empty (push before every nested scope)")
                        .insert(name.clone(), (s.span, is_import, false));
                    walk_expr(init, scope, warnings);
                }
                StmtKind::Assign(name, rhs) => {
                    mark_read(name, scope);
                    walk_expr(rhs, scope, warnings);
                }
                StmtKind::SetIndex(t, ix, val) => {
                    walk_expr(t, scope, warnings);
                    walk_expr(ix, scope, warnings);
                    walk_expr(val, scope, warnings);
                }
                StmtKind::SetAttr(obj, _, val) => {
                    walk_expr(obj, scope, warnings);
                    walk_expr(val, scope, warnings);
                }
                StmtKind::Expr(e) => walk_expr(e, scope, warnings),
                StmtKind::Print(e)
                | StmtKind::PrintF(_, e)
                | StmtKind::Return(Some(e))
                | StmtKind::Defer(e) => {
                    walk_expr(e, scope, warnings);
                }
                StmtKind::If(cond, then_b, else_b) => {
                    walk_expr(cond, scope, warnings);
                    scope.push(std::collections::HashMap::new());
                    for s in then_b {
                        walk_stmt(s, scope, warnings);
                    }
                    let top = scope
                        .pop()
                        .expect("scope stack underflow: push/pop mismatch in lint walk");
                    for (name, (sp, is_import, read)) in top {
                        if !read {
                            let msg = if is_import {
                                format!("unused import: `{}`", name)
                            } else {
                                format!("unused variable: `{}`", name)
                            };
                            warnings.push((sp, msg));
                        }
                    }
                    if let Some(else_stmts) = else_b {
                        scope.push(std::collections::HashMap::new());
                        for s in else_stmts {
                            walk_stmt(s, scope, warnings);
                        }
                        let top = scope
                            .pop()
                            .expect("scope stack underflow: push/pop mismatch in lint walk");
                        for (name, (sp, is_import, read)) in top {
                            if !read {
                                let msg = if is_import {
                                    format!("unused import: `{}`", name)
                                } else {
                                    format!("unused variable: `{}`", name)
                                };
                                warnings.push((sp, msg));
                            }
                        }
                    }
                }
                StmtKind::While(cond, body) => {
                    walk_expr(cond, scope, warnings);
                    scope.push(std::collections::HashMap::new());
                    for s in body {
                        walk_stmt(s, scope, warnings);
                    }
                    let top = scope
                        .pop()
                        .expect("scope stack underflow: push/pop mismatch in lint walk");
                    for (name, (sp, is_import, read)) in top {
                        if !read {
                            let msg = if is_import {
                                format!("unused import: `{}`", name)
                            } else {
                                format!("unused variable: `{}`", name)
                            };
                            warnings.push((sp, msg));
                        }
                    }
                }
                StmtKind::For(name, count, body) => {
                    walk_expr(count, scope, warnings);
                    if scope.len() > 1 {
                        for frame in scope[..scope.len() - 1].iter() {
                            if frame.contains_key(name) {
                                warnings.push((
                                    s.span,
                                    format!("variable `{}` shadows outer binding", name),
                                ));
                                break;
                            }
                        }
                    }
                    scope
                        .last_mut()
                        .expect("lint walk: scope is non-empty (push before every nested scope)")
                        .insert(name.clone(), (s.span, false, false));
                    scope.push(std::collections::HashMap::new());
                    for s in body {
                        walk_stmt(s, scope, warnings);
                    }
                    let top = scope
                        .pop()
                        .expect("scope stack underflow: push/pop mismatch in lint walk");
                    for (n, (sp, is_import, read)) in top {
                        if !read {
                            let msg = if is_import {
                                format!("unused import: `{}`", n)
                            } else {
                                format!("unused variable: `{}`", n)
                            };
                            warnings.push((sp, msg));
                        }
                    }
                }
                StmtKind::ForIn(name, col, body) => {
                    walk_expr(col, scope, warnings);
                    scope.push(std::collections::HashMap::new());
                    scope
                        .last_mut()
                        .expect("lint walk: scope non-empty")
                        .insert(name.clone(), (s.span, false, false));
                    for st in body {
                        walk_stmt(st, scope, warnings);
                    }
                    let top = scope.pop().expect("scope stack underflow");
                    for (n, (sp, is_import, read)) in top {
                        if !read {
                            let msg = if is_import {
                                format!("unused import: `{}`", n)
                            } else {
                                format!("unused variable: `{}`", n)
                            };
                            warnings.push((sp, msg));
                        }
                    }
                }
                StmtKind::ForInKV(k, v, col, body) => {
                    walk_expr(col, scope, warnings);
                    scope.push(std::collections::HashMap::new());
                    scope
                        .last_mut()
                        .expect("lint walk: scope non-empty")
                        .insert(k.clone(), (s.span, false, false));
                    scope
                        .last_mut()
                        .expect("lint walk: scope non-empty")
                        .insert(v.clone(), (s.span, false, false));
                    for st in body {
                        walk_stmt(st, scope, warnings);
                    }
                    let top = scope.pop().expect("scope stack underflow");
                    for (n, (sp, is_import, read)) in top {
                        if !read {
                            let msg = if is_import {
                                format!("unused import: `{}`", n)
                            } else {
                                format!("unused variable: `{}`", n)
                            };
                            warnings.push((sp, msg));
                        }
                    }
                }
                StmtKind::Fn(_name, _params, _ret, body) => {
                    scope.push(std::collections::HashMap::new());
                    for s in body {
                        walk_stmt(s, scope, warnings);
                    }
                    let top = scope
                        .pop()
                        .expect("scope stack underflow: push/pop mismatch in lint walk");
                    for (name, (sp, is_import, read)) in top {
                        if !read {
                            let msg = if is_import {
                                format!("unused import: `{}`", name)
                            } else {
                                format!("unused variable: `{}`", name)
                            };
                            warnings.push((sp, msg));
                        }
                    }
                }
                StmtKind::Block(body) => {
                    scope.push(std::collections::HashMap::new());
                    for s in body {
                        walk_stmt(s, scope, warnings);
                    }
                    let top = scope
                        .pop()
                        .expect("scope stack underflow: push/pop mismatch in lint walk");
                    for (name, (sp, is_import, read)) in top {
                        if !read {
                            let msg = if is_import {
                                format!("unused import: `{}`", name)
                            } else {
                                format!("unused variable: `{}`", name)
                            };
                            warnings.push((sp, msg));
                        }
                    }
                }
                _ => {}
            }
        }

        for s in stmts {
            walk_stmt(s, &mut scope, &mut warnings);
        }
        let top = scope
            .pop()
            .expect("scope stack underflow: push/pop mismatch in lint walk");
        for (name, (sp, is_import, read)) in top {
            if !read {
                let msg = if is_import {
                    format!("unused import: `{}`", name)
                } else {
                    format!("unused variable: `{}`", name)
                };
                warnings.push((sp, msg));
            }
        }
        warnings
    }

    // Lint: parse + typecheck one or more files, report diagnostics + unused var/import warnings
    if args.first().map(|s| s.as_str()) == Some("lint") {
        let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        let files: Vec<_> = args.iter().skip(1).map(|s| cwd.join(s)).collect();
        if files.is_empty() {
            eprintln!("pblang lint: pass one or more .sap, .sapp, or .bpx files");
            std::process::exit(1);
        }
        let mut has_error = false;
        for path in &files {
            let path = path.canonicalize().unwrap_or_else(|_| path.clone());
            let path_str = path.to_string_lossy();
            let src = match std::fs::read_to_string(&path) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("pblang lint: {}: {}", path_str, e);
                    has_error = true;
                    continue;
                }
            };
            match parse_strict(&src) {
                Ok(stmts) => {
                    let diags = type_check(&stmts);
                    for d in &diags {
                        has_error = true;
                        if let Some(sp) = d.span {
                            eprintln!("{}:{}..{}: {}", path_str, sp.start, sp.end, d.message);
                        } else {
                            eprintln!("{}: {}", path_str, d.message);
                        }
                    }
                    if diags.is_empty() {
                        for (sp, msg) in lint_unused_warnings(&stmts) {
                            eprintln!("{}:{}..{}: warning: {}", path_str, sp.start, sp.end, msg);
                        }
                    }
                }
                Err(errs) => {
                    has_error = true;
                    print_parse_errors(&src, errs, Some(&path_str));
                }
            }
        }
        if has_error {
            std::process::exit(1);
        }
        return;
    }
    if args.first().map(|s| s.as_str()) == Some("lsp") {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap_or_else(|e| {
                eprintln!("pblang lsp: failed to initialize tokio runtime: {}", e);
                std::process::exit(1);
            });
        rt.block_on(pblang_lsp::run_stdio_server());
        return;
    }

    // Run test scripts (convention: tests/*.sap, tests/*.sapp, or tests/*.bpx)
    if args.first().map(|s| s.as_str()) == Some("test") {
        let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        let test_dir = args
            .get(1)
            .map(|s| cwd.join(s))
            .unwrap_or_else(|| cwd.join("tests"));
        let mut passed = 0usize;
        let mut failed = 0usize;
        let mut entries = match std::fs::read_dir(&test_dir) {
            Ok(e) => e,
            Err(e) => {
                eprintln!(
                    "pblang test: cannot read directory '{}': {}",
                    test_dir.display(),
                    e
                );
                std::process::exit(1);
            }
        }
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.extension()
                .and_then(|e| e.to_str())
                .map(|e| e == "sap" || e == "sapp" || e == "bpx")
                .unwrap_or(false)
        })
        .collect::<Vec<_>>();
        entries.sort_by(|a, b| a.file_name().cmp(&b.file_name()));

        for path in entries {
            let path_str = path.to_string_lossy().to_string();
            let src = match std::fs::read_to_string(&path) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("FAIL {}: {}", path_str, e);
                    failed += 1;
                    continue;
                }
            };
            let src_for_parse = strip_comments_mask(&src);
            let stmts = match parse_strict(&src_for_parse) {
                Ok(s) => s,
                Err(errs) => {
                    eprintln!("FAIL {}: parse error", path_str);
                    for err in errs.iter().take(1) {
                        eprintln!("  {}", format_parse_error(err));
                    }
                    failed += 1;
                    continue;
                }
            };
            let mut chunk = match compile(stmts) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("FAIL {}: compile error: {}", path_str, e);
                    failed += 1;
                    continue;
                }
            };
            chunk.source_name = Some(std::rc::Rc::new(path_str.clone()));
            chunk.source = Some(std::rc::Rc::new(src));
            let mut vm = VM::with_stdlib(StdlibConfig::default());
            match vm.run(chunk) {
                Ok(()) => {
                    passed += 1;
                    println!("ok {}", path_str);
                }
                Err(e) => {
                    failed += 1;
                    eprintln!("FAIL {}: {}", path_str, e.message);
                }
            }
        }

        println!();
        println!("{} passed, {} failed", passed, failed);
        if failed > 0 {
            std::process::exit(1);
        }
        return;
    }

    // Run a script file and exit (no REPL)
    if args.first().map(|s| s.as_str()) == Some("run") {
        let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        let mut script_src_preloaded: Option<String> = None;
        let script_path = if let Some(path_arg) = args.get(1) {
            let candidate = PathBuf::from(path_arg);
            let candidate_abs = if candidate.is_absolute() {
                candidate.clone()
            } else {
                cwd.join(&candidate)
            };
            if candidate == Path::new(".") || candidate_abs.is_dir() {
                let man_dir = pblang_pm::find_manifest_dir(&candidate_abs).unwrap_or_else(|| {
                    eprintln!(
                        "pblang run: could not find pblang.toml from '{}'",
                        candidate_abs.display()
                    );
                    std::process::exit(1);
                });
                let (entry_path, src) = read_package_entry_source(&man_dir).unwrap_or_else(|e| {
                    eprintln!("pblang run: {}", e);
                    std::process::exit(1);
                });
                script_src_preloaded = Some(src);
                entry_path
            } else {
                candidate_abs.canonicalize().unwrap_or_else(|_| {
                    // When run from target/release, path is relative to repo root
                    pblang_pm::find_manifest_dir(&cwd)
                        .and_then(|man| man.join(path_arg).canonicalize().ok())
                        .unwrap_or_else(|| cwd.join(path_arg))
                })
            }
        } else {
            let man_dir = pblang_pm::find_manifest_dir(&cwd).unwrap_or_else(|| {
                eprintln!("usage: pblang run [file.sapp|file.bpx|dir]");
                std::process::exit(1);
            });
            let (entry_path, src) = read_package_entry_source(&man_dir).unwrap_or_else(|e| {
                eprintln!("pblang run: {}", e);
                std::process::exit(1);
            });
            script_src_preloaded = Some(src);
            entry_path
        };
        if let Some(man_dir) = script_path.parent().and_then(pblang_pm::find_manifest_dir) {
            if let Some((platform, min_rt)) = pblang_pm::get_manifest_platform_hints(&man_dir) {
                if platform.as_deref().map(|s| s != "any").unwrap_or(false) {
                    eprintln!(
                        "info: target_platform = {}",
                        platform.as_deref().unwrap_or("")
                    );
                }
                if min_rt.is_some() {
                    eprintln!("info: min_runtime = {}", min_rt.as_deref().unwrap_or(""));
                }
            }
        }

        let path_str = script_path.to_string_lossy().to_string();
        let src = if let Some(s) = script_src_preloaded {
            s
        } else {
            match std::fs::read_to_string(&script_path) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!(
                        "pblang run: failed to read '{}': {}",
                        script_path.display(),
                        e
                    );
                    std::process::exit(1);
                }
            }
        };
        let src_for_parse = strip_comments_mask(&src);
        let stmts = match parse_strict(&src_for_parse) {
            Ok(s) => s,
            Err(errs) => {
                print_parse_errors(&src_for_parse, errs, Some(&path_str));
                std::process::exit(2);
            }
        };
        let mut chunk = match compile(stmts) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("pblang: compile error: {}", e);
                std::process::exit(3);
            }
        };
        chunk.source_name = Some(std::rc::Rc::new(path_str.clone()));
        chunk.source = Some(std::rc::Rc::new(src.clone()));

        // Arguments after "--" are passed to the script as os.args() (newline-separated in env).
        let script_args: Vec<String> = args
            .iter()
            .position(|s| s == "--")
            .map(|pos| args[(pos + 1)..].to_vec())
            .unwrap_or_default();
        // SAFETY: Set before spawning any VM threads; single-threaded at this point.
        #[allow(unused_unsafe)]
        unsafe {
            std::env::set_var("PBLANG_SCRIPT_ARGS", script_args.join("\n"))
        };

        let mut vm = VM::with_stdlib(StdlibConfig::default());
        if let Err(e) = vm.run(chunk) {
            let src_ref = e.source.as_deref().unwrap_or(&src);
            let src_name = e.source_name.as_deref().unwrap_or(&path_str);
            if let Some(sp) = e.span {
                let (ln, col, line, marker) = underline_span(src_ref, sp.start..sp.end);
                eprintln!("error[{}:{}:{}]: {}", src_name, ln, col, e.message);
                eprintln!("{line}");
                eprintln!("{marker}");
            } else {
                eprintln!("error: {}", e.message);
            }
            if !e.stack.is_empty() {
                eprintln!("Stack trace:");
                for frame in &e.stack {
                    let fname = frame.function.as_deref().unwrap_or("<anonymous>");
                    let fsrc = frame.source_name.as_deref().unwrap_or(&path_str);
                    if let Some(fsp) = frame.span {
                        let (ln, col) = line_col(src_ref, fsp.start);
                        eprintln!("  at {fname} ({fsrc}:{ln}:{col})");
                    } else {
                        eprintln!("  at {fname} ({fsrc})");
                    }
                }
            }
            std::process::exit(1);
        }
        return;
    }

    // Compile a .sapp file to C and optionally to a native binary
    if args.first().map(|s| s.as_str()) == Some("compile") {
        if args.len() < 2 {
            eprintln!(
                "usage: pblang compile <file.sapp> [--out <name>] [--cc <compiler>] [--opt <level>] [--llvm|--wasm]"
            );
            eprintln!("  Default: transpile pblang source to C99, then compile to a native binary.");
            eprintln!("  --out <name>   Output binary name (default: a.out / a.exe)");
            eprintln!("  --cc <compiler> C compiler to use (default: gcc, fallback: clang, cc)");
            eprintln!("  --opt <level>  Optimization level 0-3 (default: 2)");
            eprintln!("  --llvm          Use LLVM backend for native compilation");
            eprintln!("  --wasm          Emit WebAssembly text (.wat) instead of native binary");
            eprintln!("  --keep-c        Keep the intermediate .c file");
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
            eprintln!("pblang compile: cannot find '{}': {e}", args[1]);
            std::process::exit(1);
        });
        let src = match std::fs::read_to_string(&script_path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!(
                    "pblang compile: failed to read '{}': {e}",
                    script_path.display()
                );
                std::process::exit(1);
            }
        };
        let src_for_parse = strip_comments_mask(&src);
        let stmts = match parse_strict(&src_for_parse) {
            Ok(s) => s,
            Err(errs) => {
                print_parse_errors(&src_for_parse, errs, Some(&script_path.to_string_lossy()));
                std::process::exit(2);
            }
        };

        let default_out = if cfg!(windows) { "a.exe" } else { "a.out" };
        let out_name = args
            .iter()
            .position(|a| a == "--out")
            .and_then(|i| args.get(i + 1))
            .map(|s| s.as_str())
            .unwrap_or(default_out);
        let keep_c = args.iter().any(|a| a == "--keep-c");
        let opt_level = args
            .iter()
            .position(|a| a == "--opt")
            .and_then(|i| args.get(i + 1))
            .and_then(|s| s.parse::<u8>().ok())
            .unwrap_or(2);
        let use_llvm = args.iter().any(|a| a == "--llvm");
        let use_wasm = args.iter().any(|a| a == "--wasm" || a == "--wat");

        if use_llvm && use_wasm {
            eprintln!("pblang compile: --llvm and --wasm are mutually exclusive; choose one backend");
            std::process::exit(2);
        }

        if use_wasm {
            match pblang_wasm::compile_to_wat(stmts.clone()) {
                Ok((wat, warnings)) => {
                    for w in warnings {
                        eprintln!("warning: {w}");
                    }
                    let out_path = args
                        .iter()
                        .position(|a| a == "--out")
                        .and_then(|i| args.get(i + 1))
                        .map(PathBuf::from)
                        .unwrap_or_else(|| script_path.with_extension("wat"));
                    std::fs::write(&out_path, wat).unwrap_or_else(|e| {
                        eprintln!("pblang compile --wasm: cannot write '{}': {e}", out_path.display());
                        std::process::exit(1);
                    });
                    eprintln!(
                        "WASM-emitted '{}' -> '{}'",
                        script_path.display(),
                        out_path.display()
                    );
                }
                Err(e) => {
                    eprintln!("pblang compile --wasm: {e}");
                    std::process::exit(1);
                }
            }
            return;
        }

        if use_llvm {
            // LLVM IR path (with C transpiler fallback on backend errors)
            let _ir_opts = pblang_ctranspiler::TranspileOptions::default();
            match pblang_llvm::compile_to_native(stmts.clone(), out_name, opt_level) {
                Ok(()) => {
                    eprintln!(
                        "LLVM-compiled '{}' → '{}' (via LLVM IR, O{opt_level})",
                        script_path.display(),
                        out_name
                    );
                    return;
                }
                Err(e) => {
                    eprintln!("pblang compile --llvm: {e}");
                    eprintln!(
                        "pblang compile --llvm: falling back to C99 backend for compatibility"
                    );
                }
            }
        }

        // C transpiler path (default)
        let transpile_opts = pblang_ctranspiler::TranspileOptions {
            embed_runtime: true,
            source_maps: false,
            opt_level,
        };
        let output = pblang_ctranspiler::transpile(stmts, &transpile_opts);
        if !output.warnings.is_empty() {
            for w in &output.warnings {
                eprintln!("warning: {w}");
            }
        }

        let c_path = script_path.with_extension("c");
        std::fs::write(&c_path, &output.c_source).unwrap_or_else(|e| {
            eprintln!("pblang compile: cannot write '{}': {e}", c_path.display());
            std::process::exit(1);
        });

        let cc = args
            .iter()
            .position(|a| a == "--cc")
            .and_then(|i| args.get(i + 1))
            .cloned()
            .unwrap_or_else(|| {
                for c in &["gcc", "clang", "cc"] {
                    if std::process::Command::new(c)
                        .arg("--version")
                        .stdout(std::process::Stdio::null())
                        .stderr(std::process::Stdio::null())
                        .status()
                        .is_ok()
                    {
                        return c.to_string();
                    }
                }
                "gcc".to_string()
            });

        let opt_flag = format!("-O{opt_level}");
        let status = std::process::Command::new(&cc)
            .arg(&c_path)
            .arg("-o")
            .arg(out_name)
            .arg(&opt_flag)
            .arg("-std=c99")
            .arg("-lm")
            .status()
            .unwrap_or_else(|e| {
                eprintln!("pblang compile: failed to run '{cc}': {e}");
                std::process::exit(1);
            });

        if !status.success() {
            eprintln!("pblang compile: C compilation failed");
            if !keep_c {
                let _ = std::fs::remove_file(&c_path);
            }
            std::process::exit(1);
        }

        eprintln!(
            "Compiled '{}' → '{}' (via C99, O{opt_level})",
            script_path.display(),
            out_name
        );
        if !keep_c {
            let _ = std::fs::remove_file(&c_path);
        } else {
            eprintln!("  C source kept at '{}'", c_path.display());
        }
        return;
    }

    // Extract and build polyglot blocks from a .sapp file
    if args.first().map(|s| s.as_str()) == Some("build-polyglot") {
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

            // Attempt to compile if possible
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
                // Python: compile to .pyc
                let status = std::process::Command::new("python3")
                    .arg("-m")
                    .arg("py_compile")
                    .arg(&out_file)
                    .status();
                match status {
                    Ok(s) if s.success() => eprintln!("    → compiled .pyc"),
                    _ => { /* py_compile not critical */ }
                }
            }
        }

        // Write manifest
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
        // Remove trailing [[blocks]]
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
        return;
    }

    let mut vm = VM::with_stdlib(StdlibConfig::default());

    println!("Welcome to Biapryx REPL!");
    println!("Type 'help' for commands.");

    let mut buf = String::new();
    let mut brace_depth: i32 = 0;
    let mut in_string = false;
    let mut in_single_string = false;
    let mut block_comment_depth: i32 = 0;

    loop {
        if buf.is_empty() {
            print!("> ");
        } else {
            print!("... ");
        }
        if io::stdout().flush().is_err() {
            break;
        }

        let mut line = String::new();
        if io::stdin().read_line(&mut line).is_err() {
            break;
        }

        let trimmed_end = line.trim_end();

        // Commands only when starting fresh (not mid-block).
        if buf.is_empty() {
            let raw = trimmed_end.trim();
            if raw.is_empty() {
                continue;
            }

            let cmd = raw.strip_prefix(':').unwrap_or(raw);
            let cmd = cmd.strip_prefix('/').unwrap_or(cmd);

            if cmd.eq_ignore_ascii_case("exit") || cmd.eq_ignore_ascii_case("quit") {
                break;
            }
            if cmd.eq_ignore_ascii_case("help") || cmd.eq_ignore_ascii_case("h") || cmd == "?" {
                show_help();
                continue;
            }
            if cmd.eq_ignore_ascii_case("clear") || cmd.eq_ignore_ascii_case("cls") {
                clear_terminal();
                continue;
            }
        }

        buf.push_str(trimmed_end);
        buf.push('\n');

        scan_line_for_braces(
            trimmed_end,
            &mut in_string,
            &mut in_single_string,
            &mut block_comment_depth,
            &mut brace_depth,
        );

        // Only attempt parse when braces balanced AND we're not inside a string.
        if brace_depth <= 0 && !in_string && !in_single_string && block_comment_depth == 0 {
            // Keep byte offsets stable: never trim the string we parse/execute.
            let src_for_spans = strip_comments_mask(&buf);
            if !src_for_spans.trim().is_empty() {
                match parse(&src_for_spans) {
                    Ok(stmts) => {
                        match compile(stmts) {
                            Err(ce) => {
                                eprintln!("Compile error: {ce}");
                            }
                            Ok(mut chunk) => {
                                // Attach source for accurate runtime diagnostics.
                                chunk.source_name = Some(std::rc::Rc::new("<repl>".to_string()));
                                chunk.source = Some(std::rc::Rc::new(buf.clone()));
                                if let Err(e) = vm.run(chunk) {
                                    let src = e.source.as_deref().unwrap_or(&buf);
                                    let src_name = e.source_name.as_deref().unwrap_or("<repl>");
                                    if let Some(sp) = e.span {
                                        let (ln, col) = line_col(src, sp.start);
                                        eprintln!(
                                            "Runtime error at {src_name}:{ln}:{col}: {}",
                                            e.message
                                        );
                                        let line = get_line(src, ln);
                                        eprintln!("{line}");
                                        eprintln!("{}^", " ".repeat(col.saturating_sub(1)));
                                    } else {
                                        eprintln!("Runtime error ({src_name}): {}", e.message);
                                    }

                                    if !e.stack.is_empty() {
                                        eprintln!("Stack trace:");
                                        for frame in &e.stack {
                                            let fname =
                                                frame.function.as_deref().unwrap_or("<anonymous>");
                                            let fsrc =
                                                frame.source_name.as_deref().unwrap_or("<repl>");
                                            if let Some(fsp) = frame.span {
                                                let (ln, col) = line_col(src, fsp.start);
                                                eprintln!("  at {fname} ({fsrc}:{ln}:{col})");
                                            } else {
                                                eprintln!("  at {fname} ({fsrc})");
                                            }
                                        }
                                    }
                                }
                            } // end Ok(mut chunk)
                        } // end match compile
                    }
                    Err(e) => {
                        print_parse_errors(&buf, e, None);
                    }
                }
            }

            buf.clear();
            brace_depth = 0;
            in_string = false;
            block_comment_depth = 0;
        }
    }
}
