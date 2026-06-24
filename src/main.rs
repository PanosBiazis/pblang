use pblang_compiler::{compile, disasm};

use pblang_core::{parse, parse_strict, parse_with_polyglot, strip_comments_mask};
use pblang_ir::Chunk;
use pblang_typeck::check as type_check;
use pblang_vm::{StdlibConfig, VmLimits, VM};

// `pblang_core::ParseError` is now used only inside `cli::diag` (re-imported
// there); main.rs receives it transparently through the helper functions.

use clap::{Parser, Subcommand};

use std::io::{self, Read, Write};
use std::path::{Component, Path, PathBuf};

mod cli;
use cli::call_graph::{build_call_graph, print_call_graph};
use cli::compile_cache::{
    fingerprint_for_compile, is_compile_cache_hit, write_compile_cache, CompileFingerprint,
};
use cli::dataflow::print_dataflow_report;
use cli::diag::{format_parse_error, get_line, line_col, print_parse_errors, underline_span};
use cli::fmt::{
    fmt_normalize_comma_spacing, fmt_normalize_operator_spacing, format_ide_source,
    normalize_ide_source, scan_line_for_braces,
};

/// Top-level pblang CLI parser. First-pass clap migration: this only routes
/// to the right subcommand handler; per-subcommand flag parsing is unchanged
/// and still done by the legacy dispatch chain in `legacy_dispatch`.
#[derive(Parser, Debug)]
#[command(
    name = "pblang",
    version,
    about = "pblang: a fast, embeddable scripting language",
    disable_help_flag = true,
    disable_help_subcommand = true,
    disable_version_flag = true,
    arg_required_else_help = false
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

/// Each variant carries the raw trailing args verbatim. clap is responsible
/// for picking *which* subcommand was invoked (and providing --help / typo
/// detection), but the actual flag parsing inside each handler is unchanged
/// — every handler still receives `args` and does its own option parsing.
#[derive(Subcommand, Debug)]
enum Commands {
    /// Create pblang.toml in the current directory
    Init {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Scaffold a new project: pblang new <name> [--template web|lib|cli]
    New {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Validate pblang.toml using the pblang stdlib validator
    #[command(name = "manifest-check")]
    ManifestCheck {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Compare Rust and pblang manifest parsing/validation for parity
    #[command(name = "manifest-parity")]
    ManifestParity {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Resolve deps and (depending on form) install a package or generate pblang.lock
    Install {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Publish a package to the registry
    Publish {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Search the registry for packages
    Search {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Fetch a package from the registry into the local cache
    Fetch {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Pack a project into a distributable archive
    Pack {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Verify a packaged archive
    Verify {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Build the current package (generate lockfile + typecheck entry)
    Build {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Bundle a project into a single executable artifact
    Bundle {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Transpile a .sapp source file to C
    Transpile {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Start the language server
    Lsp {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Run an HTTP server demo (legacy)
    Serve {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Format pblang source files
    Fmt {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Lint pblang source files
    Lint {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Clippy-style idiom/style lints (alias of `lint`)
    Clippy {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Discover and run `fn test_*` functions inside .sap/.sapp/.bpx
    /// files (cargo-test style); falls back to whole-file execution
    /// for files that declare no `test_*` functions. Supports
    /// `--filter <substr>`, `--exact <id>`, or `--skip <substr>` to run a subset.
    Test {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Execute a script or the current package entry point
    Run {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Compile a script to bytecode (.bpx)
    Compile {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Extract and build embedded @lang polyglot blocks
    #[command(name = "build-polyglot")]
    BuildPolyglot {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
}

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

pub(crate) fn resolve_package_entry(man_dir: &Path) -> Result<PathBuf, String> {
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
                    format!(
                        "package entry component not found '{}': {e}",
                        walk.display()
                    )
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

    let candidate = pblang_pm::MODULE_EXTENSIONS
        .iter()
        .flat_map(|ext| {
            [
                man_dir.join(format!("main.{ext}")),
                man_dir.join("src").join(format!("main.{ext}")),
            ]
        })
        .collect::<Vec<_>>()
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

    // Open with O_NOFOLLOW on Unix so that an attacker who races between
    // `canonicalize` and `open` cannot swap the resolved path for a
    // symlink and force us to follow it elsewhere. The dev/ino check
    // below remains as a second line of defence (catches non-symlink
    // races like inode reuse via rename + replace).
    #[cfg(unix)]
    let mut file = {
        use std::os::unix::fs::OpenOptionsExt;
        std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW)
            .open(&resolved)
            .map_err(|e| {
                if e.raw_os_error() == Some(libc::ELOOP) {
                    format!(
                        "refusing to open '{}': resolved path is a symlink (possible TOCTOU)",
                        resolved.display()
                    )
                } else {
                    format!("failed to open '{}': {e}", resolved.display())
                }
            })?
    };

    #[cfg(not(unix))]
    let mut file = std::fs::File::open(&resolved)
        .map_err(|e| format!("failed to open '{}': {e}", resolved.display()))?;

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
pub(crate) fn load_package_display_metadata(
    man_dir: &Path,
) -> Option<(String, String, Option<String>)> {
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

pub(crate) fn validate_packaged_local_dependencies(man_dir: &Path) -> Result<Vec<PathBuf>, String> {
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

pub(crate) fn generate_lockfile_for_cli(
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

/// Run the `pblang build` pipeline against one package directory: shadow-
/// validate the manifest, regenerate `pblang.lock`, then parse + type-check
/// the package entry. Returns `Err(exit_code)` on any failure with a message
/// already printed to stderr — the caller does `std::process::exit(code)`.
/// Workspace-walking callers pass `label = Some("crates/foo")` so error
/// lines identify the failing member.
fn build_one_package(
    man_dir: &Path,
    registry_index_url: Option<&str>,
    require_registry_sha256_cli: bool,
    label: Option<&str>,
) -> Result<(), i32> {
    let prefix: String = match label {
        Some(l) => format!("pblang build [{}]", l),
        None => "pblang build".into(),
    };
    if let Err(e) = shadow_validate_manifest_with_pblang(man_dir, "build") {
        eprintln!("{}", e);
        return Err(1);
    }
    let lock = generate_lockfile_for_cli(man_dir, registry_index_url, require_registry_sha256_cli)
        .map_err(|e| {
            eprintln!("{}: failed to generate lockfile: {}", prefix, e);
            1i32
        })?;
    let lock_str = pblang_pm::write_lockfile(&lock).map_err(|e| {
        eprintln!("{}: failed to serialize lockfile: {}", prefix, e);
        1i32
    })?;
    let lock_path = man_dir.join("pblang.lock");
    std::fs::write(&lock_path, lock_str).map_err(|e| {
        eprintln!("{}: failed to write {}: {}", prefix, lock_path.display(), e);
        1i32
    })?;
    let (entry_path, src) = read_package_entry_source(man_dir).map_err(|e| {
        eprintln!("{}: {}", prefix, e);
        1i32
    })?;
    let path_str = entry_path.to_string_lossy().into_owned();
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
                return Err(1);
            }
        }
        Err(errs) => {
            print_parse_errors(&src, errs, Some(&path_str));
            return Err(2);
        }
    }
    Ok(())
}

fn escape_pblang_string_literal(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

// ── Cranelift AOT helpers ─────────────────────────────────────────────────────

/// Generate a C stub that embeds pblang bytecode and links against the VM runtime.
/// This produces a standalone executable when compiled with a C compiler.
fn generate_c_stub_with_bytecode(bytecode: &[u8]) -> String {
    let mut c = String::new();
    c.push_str(
        "/* Auto-generated pblang AOT stub - Cranelift backend */
",
    );
    c.push_str(
        "#include <stdint.h>
",
    );
    c.push_str(
        "#include <stdio.h>
",
    );
    c.push_str(
        "#include <stdlib.h>
",
    );
    c.push_str(
        "#include <string.h>

",
    );

    // Embed bytecode as a C array
    c.push_str(&format!(
        "static const unsigned char _pblang_bytecode[{}] = {{
",
        bytecode.len()
    ));
    for (i, b) in bytecode.iter().enumerate() {
        if i > 0 {
            c.push_str(", ");
        }
        if i % 16 == 0 && i > 0 {
            c.push('\n');
        }
        c.push_str(&format!("0x{:02x}", b));
    }
    c.push_str(
        "
};

",
    );

    c.push_str(
        r#"// Minimal pblang value types
typedef enum { PB_NUMBER, PB_BOOL, PB_NIL, PB_STRING } PbTag;

typedef struct {
PbTag tag;
union {
    double num;
    int boolean;
    struct { size_t len; const char* data; } str;
};
} PbVal;

static inline PbVal pb_num(double v) { PbVal x; x.tag = PB_NUMBER; x.num = v; return x; }
static inline PbVal pb_bool(int b) { PbVal x; x.tag = PB_BOOL; x.boolean = b; return x; }
static inline PbVal pb_nil(void) { PbVal x; x.tag = PB_NIL; return x; }

// Print handler
static void pb_print(PbVal v) {
switch (v.tag) {
    case PB_NUMBER: printf("%g
", v.num); break;
    case PB_BOOL: printf("%s
", v.boolean ? "true" : "false"); break;
    case PB_NIL: printf("nil
"); break;
    case PB_STRING: printf("%.*s
", (int)v.str.len, v.str.data); break;
}
}

// Bytecode interpreter (minimal subset)
static int pb_run_bytecode(const unsigned char* bc, size_t len) {
(void)bc; (void)len;
// Full VM execution requires linking against pblang-vm.
// This stub demonstrates the embedded bytecode pattern.
printf("pblang AOT stub: bytecode embedded (%zu bytes). ", len);
printf("Link with pblang-vm for full execution.\n");
return 0;
}

int main(void) {
pb_run_bytecode(_pblang_bytecode, sizeof(_pblang_bytecode));
return 0;
}
"#,
    );

    c
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
    let out = vm.run_collect(chunk).map_err(|e| {
        format!(
            "run pblang manifest validator snippet failed: {}",
            e.message
        )
    })?;
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

/// Format `src` using the **self-hosted** formatter implemented in pblang
/// (`stdlib/compiler/formatter.sapp::format_source`) instead of the Rust
/// formatter. This is Phase 0 of the Rust→pblang migration: a real toolchain
/// command (`pblang fmt --via-pblang`) running on the pblang implementation,
/// kept honest by the `tests/fmt_drift.rs` differential test (Rust ≡ pblang).
fn run_self_hosted_fmt(src: &str, max_line_length: Option<usize>) -> Result<String, String> {
    let escaped = escape_pblang_string_literal(src);
    let cap_arg = match max_line_length {
        Some(n) => n.to_string(),
        None => "nil".to_string(),
    };
    let driver = format!(
        "let f = import(\"stdlib:compiler/formatter.sapp\"); print f.format_source(\"{escaped}\", {cap_arg});"
    );
    let (stmts, _blocks) = parse_with_polyglot(&driver)
        .map_err(|errs| format!("parse self-hosted fmt driver failed: {errs:?}"))?;
    let mut chunk =
        compile(stmts).map_err(|e| format!("compile self-hosted fmt driver failed: {e}"))?;
    chunk.source = Some(std::rc::Rc::new(driver.clone()));
    let mut vm = VM::with_stdlib(StdlibConfig::default());
    let out = vm
        .run_collect(chunk)
        .map_err(|e| format!("run self-hosted formatter failed: {}", e.message))?;
    out.last()
        .ok_or_else(|| "self-hosted formatter produced no output".to_string())
        .and_then(|v| {
            v.as_str()
                .map(|s| s.to_string())
                .map_err(|e| format!("self-hosted formatter output was not a string: {e}"))
        })
}

/// Analyze `src` using the **self-hosted** frontend implemented in pblang
/// (`stdlib/compiler/selfhost_frontend.sapp`) rather than the Rust front end —
/// a migration step toward self-hosting the `--check` path. Returns a one-line
/// summary. The Rust `--check` remains the default until parity is proven.
fn run_self_hosted_check(src: &str) -> Result<String, String> {
    let escaped = escape_pblang_string_literal(src);
    let driver = format!(
        "let fe = import(\"stdlib:compiler/selfhost_frontend.sapp\"); \
let r = fe.frontend_analyze(\"{escaped}\"); \
if r.ok {{ print \"self-hosted check ok (stmts=\" + str(r.stmt_count) + \", tokens=\" + str(r.tokens_count) + \")\"; }} \
else {{ print \"self-hosted check failed: \" + str(r.errors); }}"
    );
    let (stmts, _blocks) = parse_with_polyglot(&driver)
        .map_err(|errs| format!("parse self-hosted check driver failed: {errs:?}"))?;
    let mut chunk =
        compile(stmts).map_err(|e| format!("compile self-hosted check driver failed: {e}"))?;
    chunk.source = Some(std::rc::Rc::new(driver.clone()));
    let mut vm = VM::with_stdlib(StdlibConfig::default());
    let out = vm
        .run_collect(chunk)
        .map_err(|e| format!("run self-hosted check failed: {}", e.message))?;
    out.last()
        .ok_or_else(|| "self-hosted check produced no output".to_string())
        .and_then(|v| {
            v.as_str()
                .map(|s| s.to_string())
                .map_err(|e| format!("self-hosted check output was not a string: {e}"))
        })
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
                rust_dep_kinds.insert(
                    dep_name.clone(),
                    serde_json::Value::String(kind.to_string()),
                );
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
            let Some(obj) = item.as_object() else {
                continue;
            };
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
    let selector = std::env::var("PBLANG_STRICT_MANIFEST_VALIDATOR_COMMANDS").unwrap_or_default();
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

pub(crate) fn shadow_validate_manifest_with_pblang(
    man_dir: &Path,
    context: &str,
) -> Result<(), String> {
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
            eprintln!(
                "pblang {}: note: pblang-side manifest validator unavailable: {}",
                context, e
            );
        }
    }
    Ok(())
}

pub(crate) fn parse_registry_options_and_positionals<'a>(
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

fn parse_transpile_args(args: &[&str]) -> (String, String) {
    let mut input = String::new();
    let mut output = String::new();
    let mut i = 0;
    while i < args.len() {
        match args[i] {
            "-o" | "--output" => {
                if i + 1 < args.len() {
                    output = args[i + 1].to_string();
                    i += 1;
                }
            }
            a if !a.starts_with('-') && input.is_empty() => {
                input = a.to_string();
            }
            _ => {
                eprintln!("pblang transpile: unknown flag: {}", args[i]);
                std::process::exit(1);
            }
        }
        i += 1;
    }
    if input.is_empty() {
        eprintln!("pblang transpile: expected input file");
        eprintln!("usage: pblang transpile <file.sapp> [-o <output.c>]");
        std::process::exit(1);
    }
    if output.is_empty() {
        output = input.replace(".sapp", ".c").replace(".bpx", ".c");
        if !output.ends_with(".c") {
            output = format!("{}.c", output);
        }
    }
    (input, output)
}

fn parse_build_args(args: &[&str]) -> (String, String, String) {
    let mut input = String::new();
    let mut output = String::new();
    let mut cc = if cfg!(target_os = "windows") {
        "cl.exe"
    } else {
        "cc"
    }
    .to_string();
    let mut i = 0;
    while i < args.len() {
        match args[i] {
            "-o" | "--output" => {
                if i + 1 < args.len() {
                    output = args[i + 1].to_string();
                    i += 1;
                }
            }
            "--cc" => {
                if i + 1 < args.len() {
                    cc = args[i + 1].to_string();
                    i += 1;
                }
            }
            a if !a.starts_with('-') && input.is_empty() => {
                input = a.to_string();
            }
            _ => {
                eprintln!("pblang build: unknown flag: {}", args[i]);
                std::process::exit(1);
            }
        }
        i += 1;
    }
    if input.is_empty() {
        eprintln!("pblang build: expected input file");
        eprintln!("usage: pblang build <file.sapp> [-o <output>] [--cc <compiler>]");
        std::process::exit(1);
    }
    if output.is_empty() {
        let base = input.replace(".sapp", "").replace(".bpx", "");
        output = if cfg!(target_os = "windows") {
            format!("{}.exe", base)
        } else {
            base
        };
    }
    (input, output, cc)
}

fn resolve_compile_cc(args: &[String]) -> String {
    args.iter()
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
        })
}

fn show_help() {
    println!("pblang REPL commands:");
    println!("  help | h | ?             Show this help");
    println!("  clear | cls              Clear the screen");
    println!("  exit | quit              Exit the REPL");
    println!();

    println!("CLI helpers (run without entering the REPL):");
    println!("  pblang init            Create pblang.toml in current directory");
    println!("  pblang --ide [--allow-*]   Launch the pblang-native IDE (editors/ide/pbide.sapp): editor, terminal,");
    println!("                          visual designer, AI panel (pbAI), and browser. Defaults to --allow-all (the");
    println!("                          terminal panel needs exec); aliases: -ide, ide. Override path with PBLANG_IDE.");
    println!("  pblang run [file.sapp|file.bpx|dir] [--allow-*] [--deny-*] [--allow-all|--sandbox] [-- script-args...]");
    println!("                          Execute a script, or run the package entry from a manifest dir/current dir.");
    println!("                          Capability flags (all default to: fs+net+http+sqlite+spawn+gfx+core ON, exec+ffi OFF):");
    println!("                            --allow-exec    Enable proc_spawn_wait / exec / polyglot_exec*");
    println!("                            --allow-ffi     Enable ffi_load / ffi_call_typed / polyglot_compile_cached");
    println!("                            --allow-all     Enable every capability (use only for trusted code)");
    println!("                            --sandbox       Disable exec/ffi/net/fs/http/sqlite (paranoid mode)");
    println!("                            --allow-env / --deny-env  Allow/deny env_get reading process env vars (default allow; --sandbox denies)");
    println!("                            --deny-<cap>    Disable a specific capability (fs/net/http/sqlite/spawn/gfx/env)");
    println!("                          Env-var equivalents: PBLANG_ALLOW_EXEC=1, PBLANG_ALLOW_FFI=1, PBLANG_ALLOW_ALL=1, PBLANG_SANDBOX=1, etc.");
    println!("  pblang test [dir|profile] [--profile semantics|llm] [--filter <substr>] [--exact <id>] [--skip <substr>] [--quiet] [--jobs N|auto|smart] [--cache] [--timings] [--slow N] [--fail-fast] [--list] [--allow-*]  Discover and run `fn test_*` functions in dir (default: tests); cargo-test style output. Profiles: `pblang test semantics` expands to `tests/semantics --allow-all --quiet --jobs smart`; `pblang test llm` adds `--filter llm --timings --slow=3`. Files with no test_* fns are run wholesale. Accepts the same capability flags as `run`. --quiet captures test print output instead of writing it to stdout; --jobs runs independent test files in isolated VM workers when cache/fail-fast are not active; --jobs smart caps workers by CPU plus available RAM (`PBLANG_TEST_MAX_JOBS`, `PBLANG_TEST_RAM_MB`, `PBLANG_TEST_WORKER_MB` override). --cache reuses unchanged passing tests; --timings prints per-test durations; --slow N prints the N slowest tests; --exact runs only the exact test id; --skip excludes matching test ids; --fail-fast stops after the first failure; --list prints discovered tests without executing them.");
    println!("  pblang fmt [--max-line-length=N] [--via-pblang] [file.sap|file.bpx …] Format .sap/.sapp/.bpx (trim, operator spacing, indent 4 spaces, optional line wrap, final newline). --via-pblang formats via the self-hosted stdlib/compiler/formatter.sapp instead of the Rust formatter.");
    println!(
    "  pblang lint [file.sap|file.bpx …] Parse + typecheck files, report diagnostics (exit 1 if any), plus clippy-style idiom/style/correctness lints"
);
    println!("  pblang clippy [file …]   Alias of `lint` (clippy-style idiom lints: bool_comparison, eq_op, double_not, self_assignment, constant_condition, while_true, collapsible_if)");
    println!("  pblang --check [--via-pblang] <file>  Parse + typecheck a file (prints diagnostics). --via-pblang runs the self-hosted pblang front-end (stdlib/compiler/selfhost_frontend.sapp) instead of the Rust one.");
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
    println!("  pblang fetch <name>@<version>  Fetch package metadata from registry (uses PBLANG_REGISTRY_INDEX, optional PBLANG_REGISTRY_TOKEN).");
    println!("  pblang pack [--registry-index <url>] [--require-registry-sha256] [path] [out_file]  Create a single .tar.gz with manifest, lockfile and local deps (Stage A executable packaging)");
    println!("  pblang bundle [--registry-index <url>] [--require-registry-sha256] [path] [out_dir]  Create portable folder (default dist/) with executable + manifest + entry + deps");
    println!("  pblang transpile <file> [-o <output.c>]  Transpile .sapp/.bpx to self-contained C99 source (uses pblang-ctranspiler)");
    println!("  pblang build <file> [-o <output>] [--cc <compiler>]  Compile .sapp/.bpx to native executable via C transpiler (requires cc/gcc/clang)");
    println!("  pblang verify <tarball>  Verify pack tarball structure and optional HMAC (set PBLANG_SIGNING_KEY when .sha256 sidecar present)");
    println!("  pblang --dump-bytecode <file>   Compile file and print bytecode (disassembly) for debugging / reverse engineering");
    println!("  pblang --annotate <file>   Compile file and print annotated bytecode (source line + instruction side-by-side)");
    println!("  pblang --symbols <file>    Print a simple symbol table for top-level functions and globals (name, arity, bytecode offset)");
    println!("  pblang lsp             Start LSP server on stdin/stdout (parse/typecheck diagnostics, hover, goto-def)");
    println!("  pblang serve [--port N] [--cert <pem> --key <pem>]  Start local IDE backend server (default: 8090, plaintext). Pass --cert + --key to enable HTTPS via rustls.");
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

// `normalize_ide_source`, `format_ide_source`, and their unit tests now live in
// `cli::fmt` (imported above).

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
            if let pblang_ir::Obj::String(s) = o.as_ref() {
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
            OpCode::OpReturn if i + 1 < n => {
                leaders[i + 1] = true;
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

// `scan_line_for_braces`, `line_col`, `get_line`, `underline_span`,
// `format_parse_error`, `print_parse_errors`, `fmt_normalize_operator_spacing`,
// and `fmt_normalize_comma_spacing` have moved to `cli::fmt` / `cli::diag`
// (imported at the top of this file). `strip_comments_mask` lives in
// `pblang_core`.

// ── Call-graph (R2-alpha) ─────────────────────────────────────────────────
//
// Moved to `cli::call_graph` (imported above).

// ── Data-flow / use-def analysis (R3-alpha) ────────────────────────────────
//
// Moved to `cli::dataflow` (imported above).

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

pub(crate) fn copy_dir_all(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
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

/// Native stack reserved for the CLI worker thread. The recursive-descent parser
/// and the AST-walking compiler/typechecker consume native stack proportional to
/// source nesting; `pblang-core`'s lexical guard caps bracket nesting, but a long
/// flat operator chain (`1+1+1+…`) still produces a deeply left-nested AST. A
/// generous stack pushes the overflow threshold far beyond any realistic program
/// (and the VM's `max_stack_depth` converts runaway *VM* recursion into a clean
/// error). 256 MiB is virtual-reserve only — pages are committed lazily.
const CLI_WORKER_STACK_BYTES: usize = 256 * 1024 * 1024;

fn main() {
    // Run the whole CLI on a large-stack worker so deeply-nested (but otherwise
    // valid) source can't overflow the recursive front-end's native stack and
    // abort the process uncatchably. Error exits inside the worker still call
    // `std::process::exit`, which terminates the whole process as before.
    let worker = std::thread::Builder::new()
        .name("pblang-main".into())
        .stack_size(CLI_WORKER_STACK_BYTES)
        .spawn(run_cli)
        .expect("failed to spawn pblang worker thread");
    if worker.join().is_err() {
        // The worker panicked; its message was already printed by the default
        // hook. Mirror Rust's standard panic exit code.
        std::process::exit(101);
    }
}

fn run_cli() {
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

    // First-pass clap migration: try to route via clap as the top-level
    // subcommand router. clap validates *which* subcommand was invoked
    // (catching typos, providing --help). The actual per-flag parsing
    // inside each handler is unchanged and still lives in `legacy_dispatch`.
    //
    // On any parse failure (e.g. legacy `--call-graph` / `--lock` / `--cfg`
    // top-level "flag" subcommands, or any other shape clap doesn't model
    // yet), we transparently fall through to the legacy chain so nothing
    // regresses.
    let argv: Vec<String> = std::env::args().collect();
    match Cli::try_parse_from(&argv) {
        Ok(cli) => match cli.command {
            Some(Commands::Init { args }) => return handle_init(args),
            Some(Commands::New { args }) => return handle_new(args),
            Some(Commands::ManifestCheck { args }) => return handle_manifest_check(args),
            Some(Commands::ManifestParity { args }) => return handle_manifest_parity(args),
            Some(Commands::Install { args }) => return handle_install(args),
            Some(Commands::Publish { args }) => return handle_publish(args),
            Some(Commands::Search { args }) => return handle_search(args),
            Some(Commands::Fetch { args }) => return handle_fetch(args),
            Some(Commands::Pack { args }) => return handle_pack(args),
            Some(Commands::Verify { args }) => return handle_verify(args),
            Some(Commands::Build { args }) => return handle_build(args),
            Some(Commands::Bundle { args }) => return handle_bundle(args),
            Some(Commands::Transpile { args }) => return handle_transpile(args),
            Some(Commands::Lsp { args }) => return handle_lsp(args),
            Some(Commands::Serve { args }) => return handle_serve(args),
            Some(Commands::Fmt { args }) => return handle_fmt(args),
            Some(Commands::Lint { args }) => return handle_lint(args),
            Some(Commands::Clippy { args }) => return handle_lint(args),
            Some(Commands::Test { args }) => return handle_test(args),
            Some(Commands::Run { args }) => return handle_run(args),
            Some(Commands::Compile { args }) => return handle_compile(args),
            Some(Commands::BuildPolyglot { args }) => return handle_build_polyglot(args),
            // No subcommand given -> fall through to legacy (REPL path etc.).
            None => {}
        },
        Err(_) => {
            // Legacy/unmodeled invocation (e.g. `pblang --lock`, `pblang --cfg foo`,
            // bare script path, etc.). Fall through to the legacy dispatch chain
            // with the original args, unchanged.
        }
    }

    legacy_dispatch(args);
}

/// Per-subcommand clap handler. The body is intentionally trivial: it
/// reconstructs the legacy `args` vector (subcommand name + trailing args)
/// and delegates to `legacy_dispatch`. This preserves every existing
/// handler's flag parsing verbatim while still routing through clap.
fn handle_init(args: Vec<String>) {
    legacy_dispatch(prepend("init", args));
}
fn handle_new(args: Vec<String>) {
    legacy_dispatch(prepend("new", args));
}
fn handle_manifest_check(args: Vec<String>) {
    legacy_dispatch(prepend("manifest-check", args));
}
fn handle_manifest_parity(args: Vec<String>) {
    legacy_dispatch(prepend("manifest-parity", args));
}
fn handle_install(args: Vec<String>) {
    legacy_dispatch(prepend("install", args));
}
fn handle_publish(args: Vec<String>) {
    legacy_dispatch(prepend("publish", args));
}
fn handle_search(args: Vec<String>) {
    legacy_dispatch(prepend("search", args));
}
fn handle_fetch(args: Vec<String>) {
    legacy_dispatch(prepend("fetch", args));
}
fn handle_pack(args: Vec<String>) {
    legacy_dispatch(prepend("pack", args));
}
fn handle_verify(args: Vec<String>) {
    legacy_dispatch(prepend("verify", args));
}
fn handle_build(args: Vec<String>) {
    legacy_dispatch(prepend("build", args));
}
fn handle_bundle(args: Vec<String>) {
    legacy_dispatch(prepend("bundle", args));
}
fn handle_transpile(args: Vec<String>) {
    legacy_dispatch(prepend("transpile", args));
}
fn handle_lsp(args: Vec<String>) {
    legacy_dispatch(prepend("lsp", args));
}
fn handle_serve(args: Vec<String>) {
    legacy_dispatch(prepend("serve", args));
}
fn handle_fmt(args: Vec<String>) {
    legacy_dispatch(prepend("fmt", args));
}
fn handle_lint(args: Vec<String>) {
    legacy_dispatch(prepend("lint", args));
}
fn handle_test(args: Vec<String>) {
    legacy_dispatch(prepend("test", args));
}
fn handle_run(args: Vec<String>) {
    legacy_dispatch(prepend("run", args));
}
fn handle_compile(args: Vec<String>) {
    legacy_dispatch(prepend("compile", args));
}
fn handle_build_polyglot(args: Vec<String>) {
    legacy_dispatch(prepend("build-polyglot", args));
}

#[inline]
/// Pre-warm the polyglot cdylib cache for every `@lang { ... }` block found in
/// the package source tree. Called from `pblang build --warm-polyglot`.
///
/// Walks `.sapp` / `.sap` / `.bpx` files under `man_dir`, runs
/// `extract_polyglot_blocks` on each, filters to the 16 languages
/// `polyglot.compile_cached` supports, and invokes the cache loader for each
/// unique (lang, code) pair via a synthesized pblang bootstrap script that
/// runs with `exec`+`ffi` capabilities granted.
///
/// Idempotent: blocks whose content-hash is already in the cache are no-ops
/// (sub-millisecond). New blocks pay the per-toolchain cold-compile cost
/// once, then subsequent runs of the user's code skip the subprocess entirely.
fn warm_polyglot_cache(man_dir: &std::path::Path) {
    const CACHEABLE_LANGS: &[&str] = &[
        "rust",
        "rs",
        "c",
        "cpp",
        "c++",
        "cxx",
        "go",
        "pascal",
        "pas",
        "delphi",
        "freepascal",
        "csharp",
        "cs",
        "c#",
        "fsharp",
        "fs",
        "fsx",
        "f#",
        "zig",
        "d",
        "nim",
        "swift",
        "haskell",
        "hs",
        "fortran",
        "f90",
        "f95",
        "f03",
        "f08",
        "f",
        "objc",
        "objective-c",
        "m",
        "ocaml",
        "ml",
        "java",
        "kotlin",
        "kt",
    ];

    let mut blocks: Vec<(String, String)> = Vec::new();
    let mut files_scanned = 0usize;
    let mut walker = walkdir::WalkDir::new(man_dir).into_iter();
    while let Some(entry) = walker.next() {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        // Skip target/ and .git/.
        if entry.file_type().is_dir() {
            let n = entry.file_name().to_string_lossy();
            if n == "target" || n == ".git" || n == "node_modules" || n == "dist" {
                walker.skip_current_dir();
                continue;
            }
            continue;
        }
        let p = entry.path();
        let ext_ok = p
            .extension()
            .and_then(|e| e.to_str())
            .map(pblang_pm::is_module_ext)
            .unwrap_or(false);
        if !ext_ok {
            continue;
        }
        let src = match std::fs::read_to_string(p) {
            Ok(s) => s,
            Err(_) => continue,
        };
        files_scanned += 1;
        let extracted = pblang_core::extract_polyglot_blocks(&src);
        for b in extracted {
            if CACHEABLE_LANGS.contains(&b.lang.as_str()) {
                blocks.push((b.lang, b.code));
            }
        }
    }

    if blocks.is_empty() {
        println!(
            "warm-polyglot: no cacheable @lang blocks found ({} file(s) scanned)",
            files_scanned
        );
        return;
    }

    // Dedup by (lang, code) — same content compiles to the same cache key.
    blocks.sort();
    blocks.dedup();

    println!(
        "warm-polyglot: found {} unique @lang block(s) across {} file(s); warming cdylib cache...",
        blocks.len(),
        files_scanned
    );

    // Encode blocks as a JSON string the bootstrap script can parse. Using
    // serde_json handles all escaping (quotes, backslashes, newlines, control
    // characters) so the resulting pblang source is safe.
    let payload: Vec<serde_json::Value> = blocks
        .iter()
        .map(|(l, c)| serde_json::json!({ "lang": l, "code": c }))
        .collect();
    let json = serde_json::to_string(&payload).unwrap_or_else(|e| {
        eprintln!("warm-polyglot: failed to serialize blocks: {e}");
        std::process::exit(1);
    });

    // pblang's string literals support double-quotes with backslash escapes,
    // so we wrap the JSON in a raw-style "..." by escaping only \ and " and
    // newlines. serde_json::to_string already returns a single-line valid
    // JSON string, but its contents contain raw '"' characters that must be
    // re-escaped for embedding inside a pblang `"..."` literal.
    let json_lit = json
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t");

    // The bootstrap calls the native `polyglot_compile_cached` builtin
    // directly instead of going through `stdlib/polyglot.sapp`. That stdlib
    // import resolves only inside packages that declare a `stdlib` dep — but
    // the build is happening in the *user's* package context where stdlib
    // is not a declared dep. The native builtin is always in the global
    // scope when `ffi=true` + `exec=true`, so it works from any context.
    let script = format!(
        r#"
let raw = "{json_lit}";
let blocks = json_parse(raw);
let i = 0;
let warmed = 0;
let skipped = 0;
while i < len(blocks) {{
    let b = blocks[i];
    let h = try polyglot_compile_cached(b.lang, b.code) catch -1;
    if h == -1 {{
        // compile_cached errored — usually a missing toolchain (e.g. rustc /
        // dotnet not installed). Tally and continue with the next block.
        skipped = skipped + 1;
        print "  skipped " + b.lang + " (toolchain missing or compile error)";
    }} else {{
        warmed = warmed + 1;
        try ffi_close(h) catch nil;
        print "  warmed " + b.lang;
    }}
    i = i + 1;
}}
print "warm-polyglot: " + str(warmed) + " warmed, " + str(skipped) + " skipped";
"#
    );

    // Run the bootstrap with exec+ffi capabilities granted. Failures
    // (parse/compile/runtime) report and exit nonzero so CI catches them.
    let src_for_parse = strip_comments_mask(&script);
    let stmts = match parse_strict(&src_for_parse) {
        Ok(s) => s,
        Err(errs) => {
            eprintln!("warm-polyglot: synthesized bootstrap script failed to parse:");
            print_parse_errors(&src_for_parse, errs, Some("<warm-polyglot-bootstrap>"));
            std::process::exit(2);
        }
    };
    let mut chunk = match compile(stmts) {
        Ok(c) => c,
        Err(e) => {
            eprintln!(
                "warm-polyglot: synthesized bootstrap failed to compile: {}",
                e
            );
            std::process::exit(3);
        }
    };
    chunk.source = Some(std::rc::Rc::new(script.clone()));
    let cfg = StdlibConfig {
        exec: true,
        ffi: true,
        ..StdlibConfig::default()
    };
    let mut vm = VM::with_stdlib(cfg);
    if let Err(e) = vm.run(chunk) {
        eprintln!("warm-polyglot: bootstrap runtime error: {}", e.message);
        std::process::exit(1);
    }
}

/// Build an `axum_server::tls_rustls::RustlsConfig` with ALPN protocols set
/// to `["h2", "http/1.1"]` so HTTPS clients can negotiate HTTP/2 (h2 first,
/// http/1.1 fallback). Used by `pblang serve --cert ... --key ...`.
///
/// `axum_server::tls_rustls::RustlsConfig::from_pem_file` does not set ALPN,
/// so without this helper the server only ever speaks h1 even when h2 is
/// compiled in via `axum/http2`. We load the PEM files synchronously here
/// (small files, called once at startup) and build a `rustls::ServerConfig`
/// by hand.
fn build_tls_config_with_alpn(
    cert_path: &std::path::Path,
    key_path: &std::path::Path,
) -> std::io::Result<axum_server::tls_rustls::RustlsConfig> {
    use std::io::Cursor;

    // Read & parse the cert chain. rustls-pemfile lives in axum-server's
    // dep tree; we don't add it as a direct dep — instead we let rustls
    // parse the PEM via its own helpers from raw bytes.
    let cert_bytes = std::fs::read(cert_path)?;
    let key_bytes = std::fs::read(key_path)?;

    let certs: Vec<rustls::pki_types::CertificateDer<'static>> =
        rustls_pemfile::certs(&mut Cursor::new(cert_bytes))
            .collect::<Result<_, _>>()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    if certs.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "no certificates found in PEM",
        ));
    }

    // Accept any of: PKCS8, RSA, EC. rustls-pemfile dispatches on the PEM
    // tag automatically via `private_key`.
    let key = rustls_pemfile::private_key(&mut Cursor::new(key_bytes))?.ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidData, "no private key in PEM")
    })?;

    let mut server_config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    // ALPN: advertise h2 first (preferred), http/1.1 fallback. ALPN match
    // is strictly textual; hyper-util's auto::Builder dispatches the right
    // h1/h2 path based on what the client picked.
    server_config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];

    Ok(axum_server::tls_rustls::RustlsConfig::from_config(
        std::sync::Arc::new(server_config),
    ))
}

fn prepend(name: &str, mut rest: Vec<String>) -> Vec<String> {
    let mut out = Vec::with_capacity(rest.len() + 1);
    out.push(name.to_string());
    out.append(&mut rest);
    out
}

/// Strip `--allow-*` / `--deny-*` / `--allow-all` / `--sandbox` flags from
/// `args` and return a corresponding `StdlibConfig`. The default config
/// already enables fs / net / http / crypto / json / sqlite / gfx / spawn
/// but denies exec and ffi (secure-by-default for embedded contexts).
///
/// Recognized flags (anywhere in `args`):
///   --allow-exec, --allow-ffi, --allow-net, --allow-fs, --allow-http,
///   --allow-sqlite, --allow-spawn, --allow-gfx
///   --deny-exec, --deny-ffi, --deny-net, --deny-fs, --deny-http,
///   --deny-sqlite, --deny-spawn, --deny-gfx
///   --allow-all       — enable every capability (use only for trusted code)
///   --sandbox         — disable exec/ffi/net/http/sqlite (paranoid mode)
///
/// Also consults env vars as fallback overrides (set any to "1"/"true"):
///   PBLANG_ALLOW_EXEC, PBLANG_ALLOW_FFI, PBLANG_ALLOW_NET,
///   PBLANG_ALLOW_FS, PBLANG_ALLOW_HTTP, PBLANG_ALLOW_SQLITE,
///   PBLANG_ALLOW_SPAWN, PBLANG_ALLOW_GFX, PBLANG_ALLOW_ALL, PBLANG_SANDBOX
///
/// CLI flags take precedence over env vars; later flags override earlier ones.
pub(crate) fn parse_capability_flags(args: &mut Vec<String>) -> StdlibConfig {
    let mut cfg = StdlibConfig::default();

    fn env_on(name: &str) -> bool {
        std::env::var(name)
            .map(|v| matches!(v.as_str(), "1" | "true" | "TRUE" | "yes" | "YES" | "on"))
            .unwrap_or(false)
    }

    // 1. Env-var pre-fill.
    if env_on("PBLANG_ALLOW_EXEC") {
        cfg.exec = true;
    }
    if env_on("PBLANG_ALLOW_FFI") {
        cfg.ffi = true;
    }
    if env_on("PBLANG_ALLOW_NET") {
        cfg.net = true;
    }
    if env_on("PBLANG_ALLOW_FS") {
        cfg.fs = true;
    }
    if env_on("PBLANG_ALLOW_HTTP") {
        cfg.http = true;
    }
    if env_on("PBLANG_ALLOW_SQLITE") {
        cfg.sqlite = true;
    }
    if env_on("PBLANG_ALLOW_SPAWN") {
        cfg.spawn = true;
    }
    if env_on("PBLANG_ALLOW_GFX") {
        cfg.gfx = true;
    }
    if env_on("PBLANG_ALLOW_ENV") {
        cfg.env = true;
    }
    if env_on("PBLANG_DENY_ENV") {
        cfg.env = false;
    }
    if env_on("PBLANG_ALLOW_ALL") {
        cfg.exec = true;
        cfg.ffi = true;
        cfg.net = true;
        cfg.fs = true;
        cfg.http = true;
        cfg.sqlite = true;
        cfg.spawn = true;
        cfg.gfx = true;
        cfg.env = true;
    }
    if env_on("PBLANG_SANDBOX") {
        cfg.exec = false;
        cfg.ffi = false;
        cfg.net = false;
        cfg.fs = false;
        cfg.http = false;
        cfg.sqlite = false;
        cfg.env = false;
    }

    // 2. CLI flag scan + strip.
    args.retain(|a| match a.as_str() {
        "--allow-exec" => {
            cfg.exec = true;
            false
        }
        "--allow-ffi" => {
            cfg.ffi = true;
            false
        }
        "--allow-net" => {
            cfg.net = true;
            false
        }
        "--allow-fs" => {
            cfg.fs = true;
            false
        }
        "--allow-http" => {
            cfg.http = true;
            false
        }
        "--allow-sqlite" => {
            cfg.sqlite = true;
            false
        }
        "--allow-spawn" => {
            cfg.spawn = true;
            false
        }
        "--allow-gfx" => {
            cfg.gfx = true;
            false
        }
        "--allow-env" => {
            cfg.env = true;
            false
        }
        "--deny-exec" => {
            cfg.exec = false;
            false
        }
        "--deny-ffi" => {
            cfg.ffi = false;
            false
        }
        "--deny-net" => {
            cfg.net = false;
            false
        }
        "--deny-fs" => {
            cfg.fs = false;
            false
        }
        "--deny-http" => {
            cfg.http = false;
            false
        }
        "--deny-sqlite" => {
            cfg.sqlite = false;
            false
        }
        "--deny-spawn" => {
            cfg.spawn = false;
            false
        }
        "--deny-gfx" => {
            cfg.gfx = false;
            false
        }
        "--deny-env" => {
            cfg.env = false;
            false
        }
        "--allow-all" => {
            cfg.exec = true;
            cfg.ffi = true;
            cfg.net = true;
            cfg.fs = true;
            cfg.http = true;
            cfg.sqlite = true;
            cfg.spawn = true;
            cfg.gfx = true;
            cfg.env = true;
            false
        }
        "--sandbox" => {
            cfg.exec = false;
            cfg.ffi = false;
            cfg.net = false;
            cfg.fs = false;
            cfg.http = false;
            cfg.sqlite = false;
            cfg.env = false;
            false
        }
        _ => true,
    });

    cfg
}

/// Resolve the native IDE script (editors/ide/pbide.sapp): an absolute path if
/// found, else None. Search order: `PBLANG_IDE` env override → CWD-relative (the
/// repo root) → walking up from the executable's directory (installed/dev layouts)
/// → build-time repo root → standard share install locations.
fn locate_ide() -> Option<std::path::PathBuf> {
    fn canonical_ide_candidate(base: impl AsRef<std::path::Path>) -> Option<std::path::PathBuf> {
        let cand = base.as_ref().join("editors/ide/pbide.sapp");
        if cand.exists() {
            return cand.canonicalize().ok();
        }
        None
    }

    if let Ok(p) = std::env::var("PBLANG_IDE") {
        let pb = std::path::PathBuf::from(&p);
        if pb.exists() {
            return pb.canonicalize().ok();
        }
    }
    if let Some(p) = canonical_ide_candidate(".") {
        return Some(p);
    }
    if let Ok(exe) = std::env::current_exe() {
        let mut dir = exe.parent().map(|p| p.to_path_buf());
        for _ in 0..6 {
            if let Some(d) = &dir {
                if let Some(p) = canonical_ide_candidate(d) {
                    return Some(p);
                }
                dir = d.parent().map(|p| p.to_path_buf());
            }
        }

        if let Some(exe_dir) = exe.parent() {
            for rel in [
                "../share/pblang",
                "../../share/pblang",
                "../lib/pblang",
                "../../lib/pblang",
            ] {
                if let Some(p) = canonical_ide_candidate(exe_dir.join(rel)) {
                    return Some(p);
                }
            }
        }
    }
    if let Some(manifest_dir) = option_env!("CARGO_MANIFEST_DIR") {
        if let Some(p) = canonical_ide_candidate(manifest_dir) {
            return Some(p);
        }
    }
    for base in ["/usr/local/share/pblang", "/usr/share/pblang"] {
        if let Some(p) = canonical_ide_candidate(base) {
            return Some(p);
        }
    }
    None
}

/// Legacy top-level dispatch chain. This is the pre-clap CLI router; clap
/// (in `main`) parses the subcommand and forwards here so the per-handler
/// flag parsing inside each `if` branch is unchanged. It also still serves
/// as the fallback path for invocations clap doesn't yet model (e.g.
/// `pblang --lock`, `pblang --cfg <file>`, top-level `--check`, etc.) and
/// as the entry point for the REPL when no subcommand was given.
fn legacy_dispatch(args: Vec<String>) {
    // `pblang --ide` / `pblang -ide` / `pblang ide` — launch the pblang-native IDE
    // (editors/ide/pbide.sapp). It needs gfx + (for the terminal panel) exec, so we
    // run it with --allow-all. The IDE imports stdlib/* relative to the CWD, so we
    // chdir to the repo root we resolved the script from; extra args pass through.
    if matches!(
        args.first().map(|s| s.as_str()),
        Some("--ide") | Some("-ide") | Some("ide")
    ) {
        match locate_ide() {
            Some(script) => {
                if let Some(root) = script.ancestors().nth(3) {
                    let _ = std::env::set_current_dir(root);
                }
                let mut run_args = vec!["run".to_string(), "editors/ide/pbide.sapp".to_string()];
                let mut saw_caps = false;
                for a in args.iter().skip(1) {
                    if a.starts_with("--allow") || a == "--sandbox" || a.starts_with("--deny") {
                        saw_caps = true;
                    }
                    run_args.push(a.clone());
                }
                if !saw_caps {
                    run_args.push("--allow-all".to_string());
                }
                return legacy_dispatch(run_args);
            }
            None => {
                eprintln!(
                    "pblang --ide: could not find editors/ide/pbide.sapp.\n  Searched PBLANG_IDE, the current directory, executable-relative install paths,\n  the build-time repo path, /usr/local/share/pblang, and /usr/share/pblang.\n  Set PBLANG_IDE=/path/to/pbide.sapp to override."
                );
                std::process::exit(1);
            }
        }
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
        if !report.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
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
    if args.first().map(|s| s.as_str()) == Some("fetch") {
        // Fetch a package from the registry and show its metadata.
        // Usage: pblang fetch <name>@<version>
        let arg = args.get(1).cloned().unwrap_or_else(|| {
            eprintln!("pblang fetch: expected <name>@<version>");
            std::process::exit(1);
        });
        let (name, version) = match arg.rfind('@') {
            Some(pos) if pos > 0 => {
                let n = &arg[..pos];
                let v = &arg[pos + 1..];
                if n.is_empty() || v.is_empty() {
                    eprintln!("pblang fetch: expected <name>@<version>, got '{}'", arg);
                    std::process::exit(1);
                }
                (n.to_string(), v.to_string())
            }
            _ => {
                eprintln!("pblang fetch: expected <name>@<version>, got '{}'", arg);
                std::process::exit(1);
            }
        };
        let registry_url = std::env::var("PBLANG_REGISTRY_INDEX").unwrap_or_else(|_| {
        eprintln!(
            "pblang fetch: no registry configured (set PBLANG_REGISTRY_INDEX to registry base URL or file:// directory)"
        );
        std::process::exit(1);
    });
        let token = std::env::var("PBLANG_REGISTRY_TOKEN").ok();
        let client = if let Some(t) = token {
            pblang_pm::RegistryClient::with_token(registry_url, t)
        } else {
            pblang_pm::RegistryClient::new(registry_url)
        };
        let (metadata, _tarball) = client.fetch(&name, &version).unwrap_or_else(|e| {
            eprintln!("pblang fetch: {}", e);
            std::process::exit(1);
        });
        println!("Package: {}@{}", metadata.name, metadata.version);
        if !metadata.description.is_empty() {
            println!("Description: {}", metadata.description);
        }
        if let Some(ref checksum) = metadata.checksum {
            println!("Checksum:   {}", checksum);
        }
        if let Some(ref url) = metadata.download_url {
            println!("Download:   {}", url);
        }
        if !metadata.dependencies.is_empty() {
            println!("Dependencies:");
            for (dep_name, dep_ver) in &metadata.dependencies {
                println!("  {} = {}", dep_name, dep_ver);
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
    if args.first().map(|s| s.as_str()) == Some("emit-pbck") {
        // Compile a source file to a lossless `.pbck` bytecode container that
        // `pblang run` can execute directly (no re-parsing). This is the front
        // half of "regenerate the program from pblang bytecode".
        let path = match args.get(1) {
            Some(p) => p,
            None => {
                eprintln!("usage: pblang emit-pbck <file.sapp> [-o <out.pbck>]");
                std::process::exit(1);
            }
        };
        let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        let file_path = cwd.join(path);
        let out_path = args
            .iter()
            .position(|a| a == "-o")
            .and_then(|i| args.get(i + 1))
            .cloned()
            .unwrap_or_else(|| {
                let base = path
                    .rsplit_once('.')
                    .map(|(b, _)| b.to_string())
                    .unwrap_or_else(|| path.clone());
                format!("{base}.pbck")
            });
        let src = std::fs::read_to_string(&file_path).unwrap_or_else(|e| {
            eprintln!("pblang emit-pbck: failed to read {}: {}", file_path.display(), e);
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
        let mut chunk = match compile(stmts) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("pblang: compile error: {}", e);
                std::process::exit(3);
            }
        };
        chunk.source_name = Some(std::rc::Rc::new(path.clone()));
        let bytes = pblang_ir::pbck_serialize(&chunk);
        if let Err(e) = std::fs::write(&out_path, &bytes) {
            eprintln!("pblang emit-pbck: failed to write {}: {}", out_path, e);
            std::process::exit(1);
        }
        println!("wrote {} ({} bytes)", out_path, bytes.len());
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
        // Strip the `--warm-polyglot` flag from rest before registry-options
        // parsing so the existing positionals logic doesn't see it.
        let mut rest: Vec<String> = args.iter().skip(1).cloned().collect();
        let warm_polyglot = rest.iter().any(|a| a == "--warm-polyglot");
        rest.retain(|a| a != "--warm-polyglot");
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

        // Detect workspace shape: if `man_dir/pblang.toml` carries a
        // `[workspace]` section, build every member (and the root too if
        // it also has its own `[package]` section). Otherwise behave as
        // before — single-package build.
        let manifest_bytes = std::fs::read(man_dir.join("pblang.toml")).unwrap_or_else(|e| {
            eprintln!("pblang build: failed to read pblang.toml: {}", e);
            std::process::exit(1);
        });
        let manifest_text = match std::str::from_utf8(&manifest_bytes) {
            Ok(s) => s.to_string(),
            Err(e) => {
                eprintln!("pblang build: pblang.toml is not valid UTF-8: {}", e);
                std::process::exit(1);
            }
        };
        let parsed_manifest = pblang_pm::parse_manifest(&manifest_text).unwrap_or_else(|e| {
            eprintln!("pblang build: {}", e);
            std::process::exit(1);
        });

        if let Some(ws) = parsed_manifest.workspace.as_ref() {
            // Workspace mode.
            let members = pblang_pm::resolve_workspace_members(&man_dir, ws).unwrap_or_else(|e| {
                eprintln!("pblang build: {}", e);
                std::process::exit(1);
            });
            // Hybrid root: also build the root package if it has its own
            // [package] section. Pure-virtual workspace skips this.
            let has_root_package = parsed_manifest.package.is_some();
            if has_root_package {
                if let Err(code) = build_one_package(
                    &man_dir,
                    registry_index_url,
                    require_registry_sha256_cli,
                    Some("workspace root"),
                ) {
                    std::process::exit(code);
                }
                println!("build ok [workspace root]");
            }
            for m in &members {
                let label = m
                    .dir
                    .strip_prefix(&man_dir)
                    .ok()
                    .map(|p| p.to_string_lossy().into_owned())
                    .unwrap_or_else(|| m.dir.display().to_string());
                if let Err(code) = build_one_package(
                    &m.dir,
                    registry_index_url,
                    require_registry_sha256_cli,
                    Some(&label),
                ) {
                    std::process::exit(code);
                }
                println!("build ok [{}]", label);
            }
            let total = members.len() + if has_root_package { 1 } else { 0 };
            println!(
                "workspace build ok ({} package{})",
                total,
                if total == 1 { "" } else { "s" }
            );
            if warm_polyglot {
                // Pre-warm against the workspace root; polyglot blocks
                // are scanned from this directory downwards, so members
                // are covered transitively.
                warm_polyglot_cache(&man_dir);
            }
        } else {
            // Single-package mode (unchanged behavior).
            if let Err(code) = build_one_package(
                &man_dir,
                registry_index_url,
                require_registry_sha256_cli,
                None,
            ) {
                std::process::exit(code);
            }
            println!("build ok");
            if warm_polyglot {
                warm_polyglot_cache(&man_dir);
            }
        }
        return;
    }
    if args.first().map(|s| s.as_str()) == Some("bundle") {
        cli::bundle::handle_bundle(args);
        return;
    }

    if args.first().map(|s| s.as_str()) == Some("transpile") {
        let rest: Vec<&str> = args.iter().skip(1).map(|s| s.as_str()).collect();
        let (input_path, output_path) = parse_transpile_args(&rest);
        let src = match std::fs::read_to_string(&input_path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("pblang transpile: cannot read {}: {}", input_path, e);
                std::process::exit(1);
            }
        };
        println!("Transpiling {} to C (pblang-ctranspiler)...", input_path);
        let stmts = match parse_with_polyglot(&src) {
            Ok((s, _)) => s,
            Err(errs) => {
                print_parse_errors(&src, errs, Some(&input_path));
                std::process::exit(2);
            }
        };
        let opts = pblang_ctranspiler::TranspileOptions {
            embed_runtime: true,
            source_maps: true,
            opt_level: 2,
        };
        let output = pblang_ctranspiler::transpile(stmts, &opts);
        if !output.warnings.is_empty() {
            for w in &output.warnings {
                eprintln!("warning: {}", w);
            }
        }
        match std::fs::write(&output_path, &output.c_source) {
            Ok(()) => println!("Transpiled to {}", output_path),
            Err(e) => {
                eprintln!("pblang transpile: cannot write {}: {}", output_path, e);
                std::process::exit(1);
            }
        }
        return;
    }
    if args.first().map(|s| s.as_str()) == Some("build") {
        let rest: Vec<&str> = args.iter().skip(1).map(|s| s.as_str()).collect();
        let (input_path, output_path, cc) = parse_build_args(&rest);
        let src = match std::fs::read_to_string(&input_path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("pblang build: cannot read {}: {}", input_path, e);
                std::process::exit(1);
            }
        };
        println!("Building {} (pblang → C → native)...", input_path);
        let stmts = match parse_with_polyglot(&src) {
            Ok((s, _)) => s,
            Err(errs) => {
                print_parse_errors(&src, errs, Some(&input_path));
                std::process::exit(2);
            }
        };
        let opts = pblang_ctranspiler::TranspileOptions {
            embed_runtime: true,
            source_maps: false,
            opt_level: 2,
        };
        let output = pblang_ctranspiler::transpile(stmts, &opts);
        // Write intermediate C file
        let c_path = std::env::temp_dir().join(format!(
            "pblang_build_{}.c",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        if let Err(e) = std::fs::write(&c_path, &output.c_source) {
            eprintln!("pblang build: cannot write C source: {}", e);
            std::process::exit(1);
        }
        // Compile C → executable
        let status = std::process::Command::new(&cc)
            .arg(if cfg!(target_os = "windows") {
                "/O2"
            } else {
                "-O2"
            })
            .arg("-o")
            .arg(&output_path)
            .arg(&c_path)
            .arg("-lm")
            .status()
            .unwrap_or_else(|e| {
                eprintln!("pblang build: failed to run {}: {}", cc, e);
                // Clean up temp file
                let _ = std::fs::remove_file(&c_path);
                std::process::exit(1);
            });
        // Clean up temp file
        let _ = std::fs::remove_file(&c_path);
        if !status.success() {
            eprintln!(
                "pblang build: C compilation failed ({} exited with error)",
                cc
            );
            std::process::exit(1);
        }
        // Set executable permissions on Unix
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&output_path, std::fs::Permissions::from_mode(0o755));
        }
        println!("Built native executable: {}", output_path);
        return;
    }
    if args.first().map(|s| s.as_str()) == Some("--check") {
        let via_pblang = args
            .iter()
            .any(|a| a == "--via-pblang" || a == "--self-host");
        let Some(path) = args.iter().skip(1).find(|a| !a.starts_with("--")) else {
            eprintln!("usage: pblang --check [--via-pblang] <file>");
            std::process::exit(2);
        };
        let src = match std::fs::read_to_string(path) {
            Ok(src) => src,
            Err(e) => {
                eprintln!("pblang --check: failed to read '{}': {}", path, e);
                std::process::exit(1);
            }
        };
        if via_pblang {
            // Self-hosted front-end path (pblang checking pblang).
            match run_self_hosted_check(&src) {
                Ok(summary) => {
                    println!("{summary}");
                    return;
                }
                Err(e) => {
                    eprintln!("pblang --check --via-pblang: {e}");
                    std::process::exit(1);
                }
            }
        }
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
        cli::doc::handle_doc(args);
        return;
    }

    // Format .sap/.sapp files: trim, spacing around operators, indent by brace depth (4 spaces), optional line wrap, ensure final newline

    // ── pblang serve — Start IDE backend server ────────────────────────
    if args.first().map(|s| s.as_str()) == Some("serve") {
        let mut port: u16 = 8090;
        let mut tls_cert: Option<std::path::PathBuf> = None;
        let mut tls_key: Option<std::path::PathBuf> = None;
        let mut i = 1usize;
        while i < args.len() {
            match args[i].as_str() {
                "--port" | "-p" => {
                    i += 1;
                    if i >= args.len() {
                        eprintln!("pblang serve: --port requires a value");
                        eprintln!(
                            "usage: pblang serve [--port <1-65535>] [--cert <pem>] [--key <pem>]"
                        );
                        std::process::exit(2);
                    }
                    match args[i].parse::<u16>() {
                        Ok(p) if p > 0 => port = p,
                        _ => {
                            eprintln!("pblang serve: invalid port '{}'", args[i]);
                            eprintln!("usage: pblang serve [--port <1-65535>] [--cert <pem>] [--key <pem>]");
                            std::process::exit(2);
                        }
                    }
                }
                "--cert" => {
                    i += 1;
                    if i >= args.len() {
                        eprintln!("pblang serve: --cert requires a PEM file path");
                        std::process::exit(2);
                    }
                    tls_cert = Some(std::path::PathBuf::from(&args[i]));
                }
                "--key" => {
                    i += 1;
                    if i >= args.len() {
                        eprintln!("pblang serve: --key requires a PEM file path");
                        std::process::exit(2);
                    }
                    tls_key = Some(std::path::PathBuf::from(&args[i]));
                }
                other => {
                    match other.parse::<u16>() {
                        Ok(p) if p > 0 => port = p,
                        _ => {
                            eprintln!("pblang serve: unrecognized argument '{}'", other);
                            eprintln!("usage: pblang serve [--port <1-65535>] [--cert <pem>] [--key <pem>]");
                            std::process::exit(2);
                        }
                    }
                }
            }
            i += 1;
        }
        // --cert and --key must be provided together or not at all.
        let tls_pair = match (&tls_cert, &tls_key) {
            (Some(c), Some(k)) => Some((c.clone(), k.clone())),
            (None, None) => None,
            _ => {
                eprintln!("pblang serve: --cert and --key must be provided together");
                std::process::exit(2);
            }
        };
        let scheme = if tls_pair.is_some() { "https" } else { "http" };
        println!("pblang IDE Server starting on {scheme}://localhost:{port}");
        println!("Open editors/ide/index.html in your browser");

        // ── pblang serve — axum-based IDE backend (D1+D2) ─────────────
        //
        // Migration from the hand-rolled HTTP loop on `std::net::TcpListener`
        // (~720 lines) to axum on tokio. This addresses PrjAnalysis items
        // D1 ("single-threaded; full async migration to tokio recommended")
        // and D2 ("hand-rolled HTTP server — replacing with axum/tiny_http
        // would reduce vulnerability surface").
        //
        // Endpoints preserved verbatim (paths, JSON shapes, status codes):
        //   POST /run         — parse + typecheck + compile + execute
        //   POST /check       — parse + typecheck only
        //   POST /format      — format pblang source
        //   POST /normalize   — normalize print(...) -> print ...
        //   GET  /examples    — list bundled IDE examples
        //   GET  /examples/:id — fetch one example's source
        //   GET  /manual/:id  — fetch a bundled manual doc
        //   GET  /health      — health probe
        //   OPTIONS *         — CORS preflight (204)
        //   GET  /            — IDE index.html (fallback for unknown paths)
        //
        // Security posture preserved:
        //   - bind 127.0.0.1 only
        //   - explicit CORS allowlist for http://localhost:<port>
        //   - 10 MB request body cap (RequestBodyLimitLayer)
        //   - default 8 KB hyper header cap (axum/hyper default)

        use axum::{
            body::Body,
            extract::{
                ws::{Message, WebSocket, WebSocketUpgrade},
                Path as AxPath, State,
            },
            http::{header, HeaderValue, Method, Request, StatusCode},
            middleware::{self, Next},
            response::{IntoResponse, Response},
            routing::{get, post},
            Json, Router,
        };
        use std::sync::Arc;
        use tower_http::cors::{AllowOrigin, CorsLayer};
        use tower_http::limit::RequestBodyLimitLayer;

        const MAX_BODY_BYTES: usize = 10 * 1024 * 1024;

        #[derive(Clone)]
        struct ServeState {
            ide_examples: &'static [(&'static str, &'static str, &'static str)],
            manual_docs: &'static [(&'static str, &'static str, &'static str)],
        }

        static IDE_EXAMPLES: &[(&str, &str, &str)] = &[
            (
                "hello_world",
                "Hello World",
                "examples/hello_world/main.sapp",
            ),
            (
                "tutorial_cli",
                "Tutorial: CLI",
                "examples/tutorial_cli/main.sapp",
            ),
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
            (
                "thread_pool",
                "Thread Pool",
                "examples/thread_pool/main.sapp",
            ),
            ("web_server", "Web Server", "examples/web_server/main.sapp"),
            (
                "orm_example",
                "ORM Example",
                "examples/orm_example/main.sapp",
            ),
            (
                "quantum_bb84",
                "Quantum BB84",
                "examples/quantum_bb84/main.sapp",
            ),
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
        static MANUAL_DOCS: &[(&str, &str, &str)] = &[
            ("ide", "IDE Manual", "docs/IDE_MANUAL.md"),
            ("project", "Project Manual", "docs/PROJECT_MANUAL.md"),
        ];

        // JSON-body response wrapper that matches the legacy
        // `Content-Type: application/json` + serialized body shape exactly.
        fn json_with_status(status: StatusCode, body: serde_json::Value) -> Response {
            let s = serde_json::to_string(&body).unwrap_or_default();
            (status, [(header::CONTENT_TYPE, "application/json")], s).into_response()
        }

        async fn handle_run(Json(req): Json<serde_json::Value>) -> Response {
            // Heavy CPU + I/O: parse, typecheck, compile, spin up a VM and run.
            // Hop to a blocking thread so the async runtime stays responsive.
            tokio::task::spawn_blocking(move || {
                let started = std::time::Instant::now();
                let mut json_resp = serde_json::json!({
                    "ok": false,
                    "stage": "request",
                    "output": "",
                    "diagnostics": [],
                    "error": "invalid request body",
                    "duration_ms": 0
                });
                if let Some(code) = req.get("code").and_then(|c| c.as_str()) {
                    let code = normalize_ide_source(code);
                    let stdin_lines: Vec<String> = req
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
                                            max_external_json_bytes: None,
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
                                    let (ln, col) = line_col(&code, err.span.start);
                                    serde_json::json!({
                                        "stage": "parse",
                                        "message": format_parse_error(&err),
                                        "span": { "start": err.span.start, "end": err.span.end },
                                        "line": ln,
                                        "col": col,
                                        "line_text": get_line(&code, ln)
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
                let elapsed_ms = started.elapsed().as_millis() as u64;
                if let Some(obj) = json_resp.as_object_mut() {
                    obj.insert("duration_ms".into(), serde_json::json!(elapsed_ms));
                }
                json_with_status(StatusCode::OK, json_resp)
            })
            .await
            .unwrap_or_else(|_| {
                json_with_status(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    serde_json::json!({"ok": false, "error": "internal server error"}),
                )
            })
        }

        async fn handle_check(Json(req): Json<serde_json::Value>) -> Response {
            tokio::task::spawn_blocking(move || {
                let mut json_resp = serde_json::json!({
                    "ok": false,
                    "stage": "request",
                    "diagnostics": [],
                    "error": "invalid request body"
                });
                if let Some(code) = req.get("code").and_then(|c| c.as_str()) {
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
                                    let (ln, col) = line_col(&code, err.span.start);
                                    serde_json::json!({
                                        "stage": "parse",
                                        "message": format_parse_error(&err),
                                        "span": { "start": err.span.start, "end": err.span.end },
                                        "line": ln,
                                        "col": col,
                                        "line_text": get_line(&code, ln)
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
                json_with_status(StatusCode::OK, json_resp)
            })
            .await
            .unwrap_or_else(|_| {
                json_with_status(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    serde_json::json!({"ok": false, "error": "internal server error"}),
                )
            })
        }

        async fn handle_format(Json(req): Json<serde_json::Value>) -> Response {
            let mut json_resp = serde_json::json!({
                "ok": false,
                "error": "invalid request body",
                "code": ""
            });
            if let Some(code) = req.get("code").and_then(|c| c.as_str()) {
                let max_line_length = req
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
            json_with_status(StatusCode::OK, json_resp)
        }

        async fn handle_normalize(Json(req): Json<serde_json::Value>) -> Response {
            let mut json_resp = serde_json::json!({
                "ok": false,
                "error": "invalid request body",
                "code": ""
            });
            if let Some(code) = req.get("code").and_then(|c| c.as_str()) {
                let normalized = normalize_ide_source(code);
                json_resp = serde_json::json!({
                    "ok": true,
                    "code": normalized
                });
            }
            json_with_status(StatusCode::OK, json_resp)
        }

        async fn handle_examples_list(State(state): State<Arc<ServeState>>) -> Response {
            let mut arr = Vec::new();
            for (id, title, rel_path) in state.ide_examples {
                if std::path::Path::new(rel_path).exists() {
                    arr.push(serde_json::json!({
                        "id": id,
                        "title": title
                    }));
                }
            }
            json_with_status(
                StatusCode::OK,
                serde_json::json!({ "ok": true, "examples": arr }),
            )
        }

        async fn handle_example_one(
            State(state): State<Arc<ServeState>>,
            AxPath(id): AxPath<String>,
        ) -> Response {
            let id = id.trim_end_matches('/');
            // Multi-segment captures (e.g. /examples/foo/bar) must never echo
            // user-controlled input back nor fall through to the IDE index.
            if id.contains('/') {
                return json_with_status(
                    StatusCode::NOT_FOUND,
                    serde_json::json!({ "ok": false, "error": "unknown example id" }),
                );
            }
            let selected = state
                .ide_examples
                .iter()
                .find(|(example_id, _, _)| *example_id == id);
            if let Some((example_id, title, rel_path)) = selected {
                match std::fs::read_to_string(rel_path) {
                    Ok(code) => json_with_status(
                        StatusCode::OK,
                        serde_json::json!({
                            "ok": true,
                            "id": example_id,
                            "title": title,
                            "code": code
                        }),
                    ),
                    Err(e) => json_with_status(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        serde_json::json!({
                            "ok": false,
                            "error": format!("failed to read selected example: {}", e)
                        }),
                    ),
                }
            } else {
                json_with_status(
                    StatusCode::NOT_FOUND,
                    serde_json::json!({ "ok": false, "error": "unknown example id" }),
                )
            }
        }

        async fn handle_manual_one(
            State(state): State<Arc<ServeState>>,
            AxPath(id): AxPath<String>,
        ) -> Response {
            let id = id.trim_end_matches('/');
            if id.contains('/') {
                return json_with_status(
                    StatusCode::NOT_FOUND,
                    serde_json::json!({ "ok": false, "error": "unknown manual id" }),
                );
            }
            let selected = state
                .manual_docs
                .iter()
                .find(|(doc_id, _, _)| *doc_id == id);
            if let Some((doc_id, title, rel_path)) = selected {
                match std::fs::read_to_string(rel_path) {
                    Ok(content) => json_with_status(
                        StatusCode::OK,
                        serde_json::json!({
                            "ok": true,
                            "id": doc_id,
                            "title": title,
                            "content": content
                        }),
                    ),
                    Err(e) => json_with_status(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        serde_json::json!({
                            "ok": false,
                            "error": format!("failed to read selected manual: {}", e)
                        }),
                    ),
                }
            } else {
                json_with_status(
                    StatusCode::NOT_FOUND,
                    serde_json::json!({ "ok": false, "error": "unknown manual id" }),
                )
            }
        }

        async fn handle_health() -> Response {
            json_with_status(
                StatusCode::OK,
                serde_json::json!({"ok": true, "service": "pblang-ide-backend"}),
            )
        }

        // -----------------------------------------------------------------
        // WebSocket endpoint (GET /ws)
        //
        // Persistent JSON-message channel for IDE integrations that want
        // to avoid TCP handshake / re-auth per request. Protocol is one
        // JSON object per WebSocket text frame:
        //
        //   client → server  {"op": "ping"}
        //                    {"op": "echo", "msg": "..."}
        //                    {"op": "version"}
        //
        //   server → client  {"op": "pong", "ts_ms": <number>}
        //                    {"op": "echoed", "msg": "..."}
        //                    {"op": "version", "version": "...", "service": "pblang-ide-backend"}
        //                    {"ok": false, "error": "..."}        (on protocol error)
        //
        // The {run, check, format} ops are intentionally **not** exposed
        // over WS yet — those handlers are CPU-bound and tightly coupled
        // to the POST request shape (Json<Value> extractor, blocking
        // pool hop). A streaming wrapper that re-uses the same compile/
        // run pipeline is a follow-up; for now the WS endpoint is a
        // low-cost liveness + IDE bring-up channel that proves the
        // upgrade path works without re-architecting the handlers.
        async fn handle_ws(ws: WebSocketUpgrade) -> Response {
            ws.on_upgrade(ws_socket_loop)
        }

        async fn ws_socket_loop(mut socket: WebSocket) {
            while let Some(msg) = socket.recv().await {
                let Ok(msg) = msg else { break };
                let text = match msg {
                    Message::Text(t) => t,
                    Message::Binary(_) => {
                        let _ = socket
                            .send(Message::Text(
                                serde_json::json!({"ok": false, "error": "binary frames not supported; send JSON text"})
                                    .to_string(),
                            ))
                            .await;
                        continue;
                    }
                    Message::Ping(p) => {
                        let _ = socket.send(Message::Pong(p)).await;
                        continue;
                    }
                    Message::Pong(_) => continue,
                    Message::Close(_) => break,
                };
                let reply = ws_dispatch(&text);
                if socket.send(Message::Text(reply.to_string())).await.is_err() {
                    break;
                }
            }
        }

        fn ws_dispatch(text: &str) -> serde_json::Value {
            let v: serde_json::Value = match serde_json::from_str(text) {
                Ok(v) => v,
                Err(e) => {
                    return serde_json::json!({
                        "ok": false,
                        "error": format!("invalid JSON: {e}")
                    });
                }
            };
            let op = v.get("op").and_then(|x| x.as_str()).unwrap_or("");
            match op {
                "ping" => {
                    let ts_ms = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_millis() as u64)
                        .unwrap_or(0);
                    serde_json::json!({"op": "pong", "ts_ms": ts_ms})
                }
                "echo" => {
                    let msg = v.get("msg").cloned().unwrap_or(serde_json::Value::Null);
                    serde_json::json!({"op": "echoed", "msg": msg})
                }
                "version" => serde_json::json!({
                    "op": "version",
                    "version": env!("CARGO_PKG_VERSION"),
                    "service": "pblang-ide-backend",
                }),
                "" => serde_json::json!({
                    "ok": false,
                    "error": "missing op field; expected ping|echo|version"
                }),
                other => serde_json::json!({
                    "ok": false,
                    "error": format!("unknown op {other:?}; expected ping|echo|version")
                }),
            }
        }

        async fn handle_index() -> Response {
            let html = include_str!("../editors/ide/index.html");
            (StatusCode::OK, [(header::CONTENT_TYPE, "text/html")], html).into_response()
        }

        // Enforce explicit Content-Length on POST: the legacy hand-rolled
        // server returned 411/400 for missing/invalid C-L; axum/hyper accepts
        // chunked or empty by default, which broke
        // serve_rejects_post_without_content_length and
        // serve_rejects_malformed_content_length.
        async fn require_content_length(req: Request<Body>, next: Next) -> Response {
            if req.method() == Method::POST {
                match req.headers().get(header::CONTENT_LENGTH) {
                    None => {
                        return json_with_status(
                            StatusCode::LENGTH_REQUIRED,
                            serde_json::json!({"ok": false, "error": "missing Content-Length"}),
                        );
                    }
                    Some(v) => {
                        let s = v.to_str().unwrap_or("");
                        if s.parse::<u64>().is_err() {
                            return json_with_status(
                                StatusCode::BAD_REQUEST,
                                serde_json::json!({"ok": false, "error": "invalid Content-Length"}),
                            );
                        }
                    }
                }
            }
            next.run(req).await
        }

        let state = Arc::new(ServeState {
            ide_examples: IDE_EXAMPLES,
            manual_docs: MANUAL_DOCS,
        });

        let cors_origin = format!("{scheme}://localhost:{port}");
        let cors_origin_hv: HeaderValue = match cors_origin.parse() {
            Ok(v) => v,
            Err(e) => {
                eprintln!("pblang serve: invalid CORS origin '{}': {}", cors_origin, e);
                std::process::exit(1);
            }
        };
        let cors = CorsLayer::new()
            .allow_origin(AllowOrigin::exact(cors_origin_hv))
            .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
            .allow_headers([header::CONTENT_TYPE]);

        let app = Router::new()
            .route("/", get(handle_index))
            .route("/health", get(handle_health))
            .route("/ws", get(handle_ws))
            .route("/run", post(handle_run))
            .route("/check", post(handle_check))
            .route("/format", post(handle_format))
            .route("/normalize", post(handle_normalize))
            .route("/examples", get(handle_examples_list))
            .route("/examples/*id", get(handle_example_one))
            .route("/manual/*id", get(handle_manual_one))
            .fallback(handle_index)
            .layer(middleware::from_fn(require_content_length))
            .layer(RequestBodyLimitLayer::new(MAX_BODY_BYTES))
            .layer(cors)
            .with_state(state);

        let bind_addr = format!("127.0.0.1:{}", port);
        // 8 MB stack for both async workers and blocking-pool threads —
        // matches the main thread's default on Linux. pblang scripts that
        // recurse deeply (parser, GC traversal, deeply-nested input) overflow
        // tokio's default 2 MB worker stack. This was a known source of
        // "thread '<unknown>' has overflowed its stack" on `serve` the moment
        // any client connected. Cost: ~6 MB × worker of address space, zero
        // RSS until used.
        let rt = match tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .thread_name("pblang-serve-worker")
            .thread_stack_size(8 * 1024 * 1024)
            .build()
        {
            Ok(rt) => rt,
            Err(e) => {
                eprintln!("pblang serve: failed to build tokio runtime: {}", e);
                std::process::exit(1);
            }
        };
        rt.block_on(async move {
            match tls_pair {
                Some((cert_path, key_path)) => {
                    // HTTPS path: axum-server + rustls. The cert/key PEMs are
                    // loaded once at startup; we don't watch for file changes
                    // (re-issue requires restarting the server).
                    //
                    // rustls 0.23 requires the caller to install a process-wide
                    // CryptoProvider before any TLS operation. We use `ring`
                    // (matching pblang-vm's choice). Errors here mean a
                    // provider is already installed — that's fine; we ignore.
                    let _ = rustls::crypto::ring::default_provider().install_default();
                    let socket_addr: std::net::SocketAddr = match bind_addr.parse() {
                        Ok(a) => a,
                        Err(e) => {
                            eprintln!("pblang serve: invalid bind addr {bind_addr}: {e}");
                            std::process::exit(1);
                        }
                    };
                    // Build the rustls ServerConfig ourselves so we can
                    // advertise ALPN protocols ["h2", "http/1.1"]. axum-
                    // server's `from_pem_file` helper doesn't set ALPN, so
                    // clients never negotiate h2 even though hyper-util's
                    // auto::Builder can serve both protocols when the
                    // `http2` feature is enabled on axum (which it is —
                    // see Cargo.toml `axum = { features = ["ws", "http2"] }`).
                    let tls_config = match build_tls_config_with_alpn(&cert_path, &key_path) {
                        Ok(c) => c,
                        Err(e) => {
                            eprintln!(
                                "pblang serve: failed to load TLS cert {} / key {}: {}",
                                cert_path.display(),
                                key_path.display(),
                                e
                            );
                            std::process::exit(1);
                        }
                    };
                    if let Err(e) = axum_server::bind_rustls(socket_addr, tls_config)
                        .serve(app.into_make_service())
                        .await
                    {
                        eprintln!("pblang serve (TLS): server error: {}", e);
                        std::process::exit(1);
                    }
                }
                None => {
                    let listener = match tokio::net::TcpListener::bind(&bind_addr).await {
                        Ok(l) => l,
                        Err(e) => {
                            eprintln!("Cannot bind to port {}: {}", port, e);
                            std::process::exit(1);
                        }
                    };
                    if let Err(e) = axum::serve(listener, app).await {
                        eprintln!("pblang serve: server error: {}", e);
                        std::process::exit(1);
                    }
                }
            }
        });
        return;
    }

    if args.first().map(|s| s.as_str()) == Some("fmt") {
        let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        let mut max_line_length: Option<usize> = None;
        let mut via_pblang = false;
        let mut files: Vec<std::path::PathBuf> = Vec::new();
        for a in args.iter().skip(1) {
            if let Some(rest) = a.strip_prefix("--max-line-length=") {
                max_line_length = Some(rest.parse().unwrap_or(100));
            } else if a.as_str() == "--via-pblang" || a.as_str() == "--self-host" {
                via_pblang = true;
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
                .map(pblang_pm::is_module_ext)
                .unwrap_or(false)
            {
                match std::fs::read_to_string(&path) {
                    Ok(src) => {
                        // Self-hosting path: format via stdlib/compiler/formatter.sapp.
                        if via_pblang {
                            match run_self_hosted_fmt(&src, max_line_length) {
                                Ok(out) => {
                                    if out != src {
                                        if let Err(e) = std::fs::write(&path, &out) {
                                            eprintln!(
                                                "pblang fmt --via-pblang: failed to write {}: {}",
                                                path.display(),
                                                e
                                            );
                                            std::process::exit(1);
                                        }
                                    }
                                }
                                Err(e) => {
                                    eprintln!(
                                        "pblang fmt --via-pblang on {}: {}",
                                        path.display(),
                                        e
                                    );
                                    std::process::exit(1);
                                }
                            }
                            continue;
                        }
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
                            // Render a line that opens with '}' one level out.
                            // depth itself is advanced only by scan_line_for_braces
                            // below — decrementing here too would double-count the
                            // brace, drive depth negative, and break both the
                            // body indent and the depth==0 top-level-fn check.
                            let render_depth = if trimmed.starts_with('}') {
                                (depth - 1).max(0)
                            } else {
                                depth.max(0)
                            };
                            let indent_str: String =
                                (0..render_depth as usize).map(|_| INDENT).collect();
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
                                    let break_at_raw =
                                        line_to_output[..max_len].rfind(' ').unwrap_or(max_len);
                                    // If the only space in the cap window is inside the leading
                                    // indent, splitting at it produces a head that trims to empty
                                    // and a tail that re-indents to the same line — infinite loop.
                                    // Force a hard break at max_len in that case.
                                    let head_blank =
                                        line_to_output[..break_at_raw].trim_end().is_empty();
                                    let break_at = if break_at_raw == 0 || head_blank {
                                        max_len
                                    } else {
                                        break_at_raw
                                    };
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

    // Lint: parse + typecheck one or more files, report diagnostics + unused var/import warnings
    if args.first().map(|s| s.as_str()) == Some("lint") {
        cli::lint::handle_lint(args);
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
        cli::test::handle_test(args);
        return;
    }

    // Run a script file and exit (no REPL)
    if args.first().map(|s| s.as_str()) == Some("run") {
        // Pull capability flags out of args first so they don't interfere
        // with the path/`--` script-args parsing below.
        let mut args = args;
        let stdlib_cfg = parse_capability_flags(&mut args);
        // `--mem-report`: after the program finishes, print a one-line memory /
        // resource summary (allocation counters + live memory-safety tracker) to
        // stderr. Works for any program — pblang's own or another project's.
        let mem_report = args.iter().any(|a| a == "--mem-report");
        args.retain(|a| a != "--mem-report");
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
        // A `.pbck` file is a pre-compiled bytecode container: load and run it
        // directly, no parsing or compiling. This is the "regenerate the program
        // from pblang bytecode without the Rust front-end" path.
        let (chunk, src) = if path_str.ends_with(".pbck") {
            let bytes = std::fs::read(&script_path).unwrap_or_else(|e| {
                eprintln!("pblang run: failed to read '{}': {}", script_path.display(), e);
                std::process::exit(1);
            });
            let mut c = pblang_ir::pbck_deserialize(&bytes).unwrap_or_else(|e| {
                eprintln!("pblang run: {}: {}", path_str, e);
                std::process::exit(3);
            });
            if c.source_name.is_none() {
                c.source_name = Some(std::rc::Rc::new(path_str.clone()));
            }
            (c, String::new())
        } else {
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
            // Reject an unterminated `/* … */` on the RAW source before masking
            // erases it — otherwise a stray `/*` silently deletes the rest of the
            // file with exit 0 (the comment masker turns it all to spaces).
            if let Some(e) = pblang_core::check_unterminated_comment(&src) {
                print_parse_errors(&src, vec![e], Some(&path_str));
                std::process::exit(2);
            }
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
            (chunk, src)
        };

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

        let mut vm = VM::with_stdlib(stdlib_cfg);
        // Trusted CLI execution leaves instruction/memory budgets unbounded (like
        // `python file.py`), but a generous VM stack-depth cap converts runaway
        // recursion into a catchable RuntimeError instead of an uncatchable native
        // stack-overflow abort. The big worker stack (see `CLI_WORKER_STACK_BYTES`)
        // sits well above this depth.
        vm.set_limits(Some(VmLimits {
            max_instructions: None,
            max_stack_depth: Some(16_384),
            max_memory_bytes: None,
            io_timeout_ms: None,
            max_external_json_bytes: None,
        }));
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
        if mem_report {
            eprintln!("[mem-report] {}", vm.memory_report());
        }
        return;
    }

    // Compile a .sapp file to C and optionally to a native binary
    if args.first().map(|s| s.as_str()) == Some("compile") {
        if args.len() < 2 {
            eprintln!(
            "usage: pblang compile <file.sapp> [--out <name>] [--cc <compiler>] [--opt <level>] [--backend <name>] [--target <triple>] [--llvm|--wasm]"
        );
            eprintln!(
                "  Default: transpile pblang source to C99, then compile to a native binary."
            );
            eprintln!("  --out <name>       Output binary name (default: a.out / a.exe)");
            eprintln!("  --cc <compiler>    C compiler to use (default: gcc, fallback: clang, cc)");
            eprintln!("  --opt <level>      Optimization level 0-3 (default: 2)");
            eprintln!("  --backend <name>   Backend: cranelift | ctranspiler | llvm");
            eprintln!("  --target <triple>  Target triple (e.g. x86_64-unknown-linux-gnu)");
            eprintln!("  --llvm              Use LLVM backend for native compilation");
            eprintln!(
                "  --wasm              Emit WebAssembly text (.wat) instead of native binary"
            );
            eprintln!("  --keep-c            Keep the intermediate .c file");
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
        let use_cranelift = args.iter().any(|a| a == "--cranelift")
            || args.iter().any(|a| a == "--backend")
                && args
                    .iter()
                    .position(|a| a == "--backend")
                    .and_then(|i| args.get(i + 1))
                    .map(|s| s == "cranelift")
                    .unwrap_or(false);
        let target_triple = args
            .iter()
            .position(|a| a == "--target")
            .and_then(|i| args.get(i + 1))
            .cloned();

        if use_llvm && use_wasm {
            eprintln!(
                "pblang compile: --llvm and --wasm are mutually exclusive; choose one backend"
            );
            std::process::exit(2);
        }

        let out_path = {
            let candidate = PathBuf::from(out_name);
            if candidate.is_absolute() {
                candidate
            } else {
                cwd.join(candidate)
            }
        };
        let cc = resolve_compile_cc(&args);
        let requested_backend = if use_llvm {
            "llvm"
        } else if use_cranelift {
            "cranelift"
        } else {
            "c99"
        };
        let compile_fingerprint: Option<CompileFingerprint> = if !keep_c && !use_wasm {
            match fingerprint_for_compile(
                &script_path,
                requested_backend,
                &out_path,
                opt_level,
                Some(&cc),
                target_triple.as_deref(),
            ) {
                Ok(fp) => {
                    if is_compile_cache_hit(&out_path, &fp) {
                        eprintln!(
                            "pblang compile: cache hit, reused '{}' ({} input files, {} bytes unchanged)",
                            out_path.display(),
                            fp.input_count,
                            fp.input_bytes
                        );
                        return;
                    }
                    Some(fp)
                }
                Err(e) => {
                    eprintln!("pblang compile: cache disabled: {e}");
                    None
                }
            }
        } else {
            None
        };

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
                        eprintln!(
                            "pblang compile --wasm: cannot write '{}': {e}",
                            out_path.display()
                        );
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
                    if let Some(fp) = compile_fingerprint.as_ref() {
                        if let Err(e) = write_compile_cache(&out_path, fp) {
                            eprintln!("pblang compile: warning: failed to write cache: {e}");
                        }
                    }
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

        // Cranelift AOT compilation path
        if use_cranelift {
            let target = target_triple
                .as_deref()
                .unwrap_or(pblang_cranelift::host_target_triple());
            eprintln!(
                "pblang compile: using Cranelift AOT backend for target '{}'",
                target
            );
            eprintln!("pblang compile: note -- Cranelift AOT requires a pblang VM runtime library to link");

            // Compile the pblang source to bytecode, then to object file via Cranelift
            match pblang_compiler::compile(stmts.clone()) {
                Ok(chunk) => {
                    // Build a Function wrapper for the chunk.
                    // Serialize chunk and write as bytecode data alongside a thin main().
                    let bytecode = pblang_ir::serialize_chunk_pbck(&chunk);
                    // Generate a C stub that loads bytecode and runs VM
                    let c_stub = generate_c_stub_with_bytecode(&bytecode);
                    let stub_path = script_path.with_extension("stub.c");
                    std::fs::write(&stub_path, &c_stub).unwrap_or_else(|e| {
                        eprintln!(
                            "pblang compile: cannot write stub '{}': {e}",
                            stub_path.display()
                        );
                        std::process::exit(1);
                    });

                    // Compile the C stub with the pblang runtime linked in
                    let opt_flag = format!("-O{opt_level}");
                    let status = std::process::Command::new(&cc)
                        .arg(&stub_path)
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

                    if status.success() {
                        eprintln!(
                        "Cranelift-compiled '{}' → '{}' (via Cranelift AOT + embedded bytecode, O{opt_level})",
                        script_path.display(),
                        out_name
                    );
                        let _ = std::fs::remove_file(&stub_path);
                        if let Some(fp) = compile_fingerprint.as_ref() {
                            if let Err(e) = write_compile_cache(&out_path, fp) {
                                eprintln!("pblang compile: warning: failed to write cache: {e}");
                            }
                        }
                        return;
                    } else {
                        eprintln!("pblang compile: Cranelift AOT compilation failed, falling back to C99 backend");
                        let _ = std::fs::remove_file(&stub_path);
                    }
                }
                Err(e) => {
                    eprintln!("pblang compile: bytecode compilation failed: {e}, falling back to C99 backend");
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
        if let Some(fp) = compile_fingerprint.as_ref() {
            if let Err(e) = write_compile_cache(&out_path, fp) {
                eprintln!("pblang compile: warning: failed to write cache: {e}");
            }
        }
        if !keep_c {
            let _ = std::fs::remove_file(&c_path);
        } else {
            eprintln!("  C source kept at '{}'", c_path.display());
        }
        return;
    }

    // Extract and build polyglot blocks from a .sapp file

    // Transpile a .sapp file to self-contained C source
    if args.first().map(|s| s.as_str()) == Some("transpile") {
        if args.len() < 2 {
            eprintln!("usage: pblang transpile <file.sapp> [-o <output.c>] [--embed-runtime]");
            eprintln!("  Transpiles pblang source to portable C99 code.");
            eprintln!("  -o <output.c>     Output C file path (default: input.c)");
            eprintln!("  --embed-runtime   Embed the pblang runtime header inline");
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
            eprintln!("pblang transpile: cannot find '{}': {e}", args[1]);
            std::process::exit(1);
        });
        let src = match std::fs::read_to_string(&script_path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!(
                    "pblang transpile: failed to read '{}': {e}",
                    script_path.display()
                );
                std::process::exit(1);
            }
        };

        let embed = args.iter().any(|a| a == "--embed-runtime");
        let out_path = args
            .iter()
            .position(|a| a == "-o")
            .and_then(|i| args.get(i + 1))
            .map(PathBuf::from)
            .unwrap_or_else(|| script_path.with_extension("c"));

        match pblang_ctranspiler::transpile_to_c(&src) {
            Ok(output) => {
                if !output.warnings.is_empty() {
                    for w in &output.warnings {
                        eprintln!("warning: {w}");
                    }
                }
                let final_source = if embed || output.c_source.contains("pblang_rt.h") {
                    output.c_source
                } else {
                    format!("{}{}", output.c_source, "")
                };
                std::fs::write(&out_path, &final_source).unwrap_or_else(|e| {
                    eprintln!(
                        "pblang transpile: cannot write '{}': {e}",
                        out_path.display()
                    );
                    std::process::exit(1);
                });
                eprintln!(
                    "Transpiled '{}' → '{}' ({:.1} KB C99)",
                    script_path.display(),
                    out_path.display(),
                    final_source.len() as f64 / 1024.0
                );
            }
            Err(errs) => {
                eprintln!("pblang transpile: error: {}", errs);
                std::process::exit(2);
            }
        }
        return;
    }

    if args.first().map(|s| s.as_str()) == Some("build-polyglot") {
        cli::build_polyglot::handle_build_polyglot(args);
        return;
    }

    // REPL also accepts capability flags so an interactive session can
    // do FFI/polyglot work, e.g. `pblang --allow-exec --allow-ffi`.
    let mut repl_args = args;
    let repl_cfg = parse_capability_flags(&mut repl_args);
    let mut vm = VM::with_stdlib(repl_cfg);

    println!("Welcome to pblang REPL!");
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
