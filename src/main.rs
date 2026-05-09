use pblang_compiler::{compile, disasm};
use pblang_core::{
    parse, parse_with_polyglot, Expr, ExprKind, MatchArm, Span, Stmt, StmtKind, TypeExpr, TypeKind,
};
use pblang_ir::Chunk;
use pblang_typeck::check as type_check;
use pblang_vm::{StdlibConfig, VM};

use chumsky::error::SimpleReason;
use chumsky::prelude::Simple;

use std::io::{self, Write};
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
            // SECURITY FIX #8 (TOCTOU): canonicalize() validates the path at time T,
            // but the file is opened/read at T+1. A symlink swap between these two
            // steps can redirect execution outside the package root.
            // Best-effort mitigation: re-check starts_with after canonicalize, and
            // the consumer should open the file with O_NOFOLLOW where the platform
            // supports it. A full fix requires platform-specific open-and-verify.
            let resolved = entry_path
                .canonicalize()
                .map_err(|e| format!("package entry not found '{}': {e}", entry_path.display()))?;
            if !resolved.starts_with(man_dir) {
                return Err(format!(
                    "package entry escapes package root: {}",
                    resolved.display()
                ));
            }
            // Double-check: re-canonicalize to detect any symlink swap (best effort).
            let re_resolved = resolved.canonicalize()
                .map_err(|e| format!("package entry disappeared after validation '{}': {e}", resolved.display()))?;
            if re_resolved != resolved {
                return Err(format!(
                    "package entry changed between checks (possible symlink swap): {}",
                    resolved.display()
                ));
            }
            return Ok(resolved);
        }
    }

    man_dir.join("main.sap").canonicalize().ok()
        .or_else(|| man_dir.join("main.sapp").canonicalize().ok())
        .or_else(|| man_dir.join("main.bpx").canonicalize().ok())
        .or_else(|| man_dir.join("src").join("main.sap").canonicalize().ok())
        .or_else(|| man_dir.join("src").join("main.sapp").canonicalize().ok())
        .or_else(|| man_dir.join("src").join("main.bpx").canonicalize().ok())
        .ok_or_else(|| {
            format!(
                "no package entry found (expected [package].entry or main.sap/main.sapp/main.bpx/src/main.sap/src/main.sapp/src/main.bpx) in '{}'",
                man_dir.display()
            )
        })
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
    println!("  pblang publish --registry-dir <dir> [path]  Pack package into tarball and add to local registry index (file-based registry).");
    println!("  pblang publish            Publish current package to hosted registry (PBLANG_REGISTRY_INDEX, optional PBLANG_REGISTRY_TOKEN or --registry-token=<token>).");
    println!("  pblang search <query>     Search hosted or file-based registry for packages matching query (uses PBLANG_REGISTRY_INDEX).");
    println!("  pblang pack [--registry-index <url>] [--require-registry-sha256] [path] [out_file]  Create a single .tar.gz with manifest, lockfile and local deps (Stage A executable packaging)");
    println!("  pblang bundle [--registry-index <url>] [--require-registry-sha256] [path] [out_dir]  Create portable folder (default dist/) with executable + manifest + entry + deps");
    println!("  pblang verify <tarball>  Verify pack tarball structure and optional HMAC (set PBLANG_SIGNING_KEY when .sha256 sidecar present)");
    println!("  pblang --dump-bytecode <file>   Compile file and print bytecode (disassembly) for debugging / reverse engineering");
    println!("  pblang --annotate <file>   Compile file and print annotated bytecode (source line + instruction side-by-side)");
    println!("  pblang --symbols <file>    Print a simple symbol table for top-level functions and globals (name, arity, bytecode offset)");
    println!("  pblang lsp             Start JSON-RPC LSP server on stdin/stdout (experimental, L2-delta stub)");
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
    in_string: &mut bool,
    block_comment_depth: &mut i32,
    brace_depth: &mut i32,
) {
    let mut chars = line.chars().peekable();
    let mut prev_was_backslash = false;

    while let Some(ch) = chars.next() {
        let next = chars.peek().copied();

        // If we're in a string, only care about \" and closing "
        if *in_string {
            if ch == '"' && !prev_was_backslash {
                *in_string = false;
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
            *in_string = true;
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
    let mut in_string = false;
    let mut block_depth: i32 = 0;

    while i < out.len() {
        let b = out[i];

        // Inside string: only care about escapes and closing "
        if in_string {
            if b == b'\\' {
                i = i.saturating_add(2);
                continue;
            }
            if b == b'"' {
                in_string = false;
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
            in_string = true;
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
            StmtKind::Block(body) => {
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

// ── LSP server (L3-beta: scope-aware completion) ────────────────────────

#[derive(Clone)]
struct LocalDecl {
    name: String,
    line: u32,
    col: u32,
    is_fn: bool,
}

fn take_ident_prefix(s: &str) -> Option<&str> {
    let mut end = 0usize;
    for (i, ch) in s.char_indices() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            end = i + ch.len_utf8();
        } else if end > 0 {
            break;
        } else {
            return None;
        }
    }
    if end == 0 {
        return None;
    }
    Some(&s[..end])
}

fn collect_local_decls(src: &str) -> Vec<LocalDecl> {
    let mut out = Vec::new();
    for (line_idx, line) in src.lines().enumerate() {
        let trimmed = line.trim_start();
        let indent = line.len() - trimmed.len();
        if let Some(rel) = trimmed.find("fn ") {
            let after_fn = indent + rel + 3;
            let rest_slice = &line[after_fn..];
            let dt = rest_slice.trim_start();
            let name_start = after_fn + (rest_slice.len() - dt.len());
            if let Some(name) = take_ident_prefix(dt) {
                out.push(LocalDecl {
                    name: name.to_string(),
                    line: line_idx as u32,
                    col: name_start as u32,
                    is_fn: true,
                });
            }
        }
        if let Some(rel) = trimmed.find("let ") {
            let after_let = indent + rel + 4;
            let rest_slice = &line[after_let..];
            let dt = rest_slice.trim_start();
            let name_start = after_let + (rest_slice.len() - dt.len());
            if let Some(name) = take_ident_prefix(dt) {
                out.push(LocalDecl {
                    name: name.to_string(),
                    line: line_idx as u32,
                    col: name_start as u32,
                    is_fn: false,
                });
            }
        }
    }
    out
}

fn collect_local_names(src: &str) -> Vec<String> {
    use std::collections::HashSet;
    let set: HashSet<String> = collect_local_decls(src)
        .into_iter()
        .map(|d| d.name)
        .collect();
    let mut names: Vec<String> = set.into_iter().collect();
    names.sort();
    names
}

fn lsp_ident_byte_at(line: &str, character: u32) -> Option<String> {
    let bytes = line.as_bytes();
    if bytes.is_empty() {
        return None;
    }
    let col = (character as usize).min(bytes.len());
    let mut idx = col;
    if idx >= bytes.len() {
        idx = bytes.len() - 1;
    } else if idx > 0 && !bytes[idx].is_ascii_alphanumeric() && bytes[idx] != b'_' {
        idx -= 1;
    }
    if !(bytes[idx].is_ascii_alphanumeric() || bytes[idx] == b'_') {
        return None;
    }
    let mut start = idx;
    while start > 0 {
        let b = bytes[start - 1];
        if b.is_ascii_alphanumeric() || b == b'_' {
            start -= 1;
        } else {
            break;
        }
    }
    let mut end = idx + 1;
    while end < bytes.len() {
        let b = bytes[end];
        if b.is_ascii_alphanumeric() || b == b'_' {
            end += 1;
        } else {
            break;
        }
    }
    line.get(start..end).map(|s| s.to_string())
}

fn lsp_parse_td_position(body: &str) -> Option<(String, u32, u32)> {
    let v: serde_json::Value = serde_json::from_str(body).ok()?;
    let params = v.get("params")?;
    let uri = params
        .get("textDocument")?
        .get("uri")?
        .as_str()?
        .to_string();
    let pos = params.get("position")?;
    let line = u32::try_from(pos.get("line")?.as_u64()?).ok()?;
    let character = u32::try_from(pos.get("character")?.as_u64()?).ok()?;
    Some((uri, line, character))
}

fn lsp_did_open_uri_and_text(body: &str) -> Option<(String, String)> {
    let v: serde_json::Value = serde_json::from_str(body).ok()?;
    let td = v.get("params")?.get("textDocument")?;
    let uri = td.get("uri")?.as_str()?.to_string();
    let text = td.get("text")?.as_str()?.to_string();
    Some((uri, text))
}

fn lsp_did_change_uri_and_text(body: &str) -> Option<(String, String)> {
    let v: serde_json::Value = serde_json::from_str(body).ok()?;
    let params = v.get("params")?;
    let uri = params
        .get("textDocument")?
        .get("uri")?
        .as_str()?
        .to_string();
    let text = params
        .get("contentChanges")?
        .as_array()?
        .first()?
        .get("text")?
        .as_str()?
        .to_string();
    Some((uri, text))
}

fn run_lsp_server() {
    use std::collections::HashMap;
    use std::io::{BufRead, BufReader, Read, Write};

    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut reader = BufReader::new(stdin.lock());
    let mut writer = stdout.lock();

    let builtin_completions: Vec<&str> = vec![
        // core
        "print", "printf", "len", "type", "str",
        "parsenumber", "parsebool", "read_line",
        // array
        "array_push", "array_pop", "array_sort",
        // map
        "map_keys", "map_has", "map_remove",
        // number
        "num_sqrt", "num_pow", "num_abs", "num_max", "num_min",
        "num_floor", "num_ceil", "num_mod", "num_ln", "num_exp",
        "num_atan2",
        // random
        "random_bytes", "random_float",
        // string
        "str_contains", "str_starts_with", "str_ends_with",
        "str_trim", "str_split", "str_slice", "str_index_of",
        "str_lower", "str_upper", "strfromcharcode", "strcharcodeat",
        // json / crypto
        "json_parse", "json_stringify",
        "sha256", "base64_encode", "base64_decode", "constant_time_eq",
        // http
        "http_get", "http_post", "http_post_auth",
        // fs
        "fs_read", "fs_write", "fs_append", "fs_exists", "fs_remove",
        "fs_list_dir", "fs_mkdirs", "fs_rename", "fs_copy",
        "fs_read_bytes", "fs_write_bytes",
        "cwd", "path_join", "path_dirname", "path_basename",
        // net
        "tcp_listen", "tcp_accept", "tcp_read", "tcp_write",
        "tcp_close", "tcp_set_timeout",
        // sqlite
        "sqlite_open", "sqlite_exec", "sqlite_query", "sqlite_close",
        "sqlite_last_insert_rowid",
        // ffi
        "ffi_load", "ffi_call", "ffi_close",
        // concurrency
        "spawn", "join", "channel", "send", "recv",
        "channel_close", "channel_is_closed", "select",
        // timing / misc
        "clock", "now_ms", "now_iso", "sleep_ms",
        "gc_stats", "gc_collect",
        "env_get", "env_set", "exec",
        // os
        "import",
        "print",
        // kept for compat
        "parsenumber",
        "arraypush",
        "arraypop",
        "mapkeys",
        "maphas",
        "mapremove",
        "jsonparse",
        "jsonstringify",
        "httpget",
        "httppost",
        "fsread",
        "fswrite",
        "fsexists",
        "fsmkdirs",
        "fslistdir",
        "sha256",
        "base64encode",
        "base64decode",
        "clock",
        "randombytes",
        "import",
        "export",
    ];

    let mut open_files: HashMap<String, String> = HashMap::new();

    loop {
        let mut header = String::new();
        if reader.read_line(&mut header).unwrap_or(0) == 0 {
            break;
        }
        let header = header.trim().to_string();
        if header.is_empty() {
            continue;
        }
        let content_length: usize = if let Some(rest) = header.strip_prefix("Content-Length: ") {
            rest.trim().parse().unwrap_or(0)
        } else {
            continue;
        };
        let mut blank = String::new();
        let _ = reader.read_line(&mut blank);
        let mut body_bytes = vec![0u8; content_length];
        if reader.read_exact(&mut body_bytes).is_err() {
            break;
        }
        let body = String::from_utf8_lossy(&body_bytes).to_string();
        let method = lsp_extract_str(&body, "method");
        let id_val = lsp_extract_id(&body);
        let response: Option<String> = match method.as_deref() {
            Some("initialize") => Some(lsp_response(
                id_val.as_deref(),
                r#"{
                "capabilities": {
                    "completionProvider": { "triggerCharacters": [".", ":"] },
                    "textDocumentSync": 1,
                    "hoverProvider": true,
                    "definitionProvider": true,
                    "workspaceSymbolProvider": true
                },
                "serverInfo": { "name": "pblang-lsp", "version": "0.5.0-stub" }
            }"#,
            )),
            Some("initialized") => None,
            Some("textDocument/didOpen") => {
                if let Some((uri, text)) = lsp_did_open_uri_and_text(&body) {
                    open_files.insert(uri, text);
                }
                None
            }
            Some("textDocument/didChange") => {
                if let Some((uri, text)) = lsp_did_change_uri_and_text(&body) {
                    open_files.insert(uri, text);
                } else if let Some(params_start) = body.find("\"params\"") {
                    if let Some(text_key) = body[params_start..].rfind("\"text\"") {
                        let after_text = &body[params_start + text_key + "\"text\"".len()..];
                        if let Some(colon) = after_text.find(':') {
                            let after_colon = after_text[colon + 1..].trim_start();
                            if let Some(rest) = after_colon.strip_prefix('"') {
                                if let Some(end_quote) = rest.find('"') {
                                    let text = &rest[..end_quote];
                                    let uri = if let Some(uri_key) =
                                        body[params_start..].find("\"uri\"")
                                    {
                                        let after_uri =
                                            &body[params_start + uri_key + "\"uri\"".len()..];
                                        if let Some(colon2) = after_uri.find(':') {
                                            let after_colon2 = after_uri[colon2 + 1..].trim_start();
                                            if let Some(rest2) = after_colon2.strip_prefix('"') {
                                                if let Some(end2) = rest2.find('"') {
                                                    rest2[..end2].to_string()
                                                } else {
                                                    "".to_string()
                                                }
                                            } else {
                                                "".to_string()
                                            }
                                        } else {
                                            "".to_string()
                                        }
                                    } else {
                                        "".to_string()
                                    };
                                    open_files.insert(uri, text.to_string());
                                }
                            }
                        }
                    }
                }
                None
            }
            Some("workspace/symbol") => {
                // Return all symbols from open files
                let q_needle = "\"query\"";
                let query = if let Some(q_pos) = body.find(q_needle) {
                    let after = &body[q_pos + q_needle.len()..];
                    if let Some(c) = after.find(':') {
                        let v = after[c+1..].trim_start();
                        if let Some(r) = v.strip_prefix('"') {
                            r.split('"').next().unwrap_or("").to_string()
                        } else { "".to_string() }
                    } else { "".to_string() }
                } else { "".to_string() };
                let mut symbols: Vec<String> = Vec::new();
                for (uri, src) in &open_files {
                    for line in src.lines() {
                        let line = line.trim();
                        if let Some(rest) = line.strip_prefix("fn ") {
                            if let Some(name_end) = rest.find(|c: char| !c.is_alphanumeric() && c != '_') {
                                let name = &rest[..name_end];
                                if query.is_empty() || name.contains(&query) {
                                    let name_json = serde_json::to_string(name).unwrap_or_default();
                                    let uri_json = serde_json::to_string(uri).unwrap_or_default();
                                    symbols.push(format!(
                                        "{{\"name\":{name_json},\"kind\":12,\"location\":{{\"uri\":{uri_json}}}}}"
                                    ));
                                }
                            }
                        }
                    }
                }
                let arr = format!("[{}]", symbols.join(","));
                Some(lsp_response(id_val.as_deref(), &arr))
            }
            Some("textDocument/completion") => {
                let mut items: Vec<String> = builtin_completions
                    .iter()
                    .map(|name| format!(r#"{{"label":"{}","kind":3,"detail":"builtin"}}"#, name))
                    .collect();
                if let Some(params_start) = body.find("\"params\"") {
                    if let Some(uri_key) = body[params_start..].find("\"uri\"") {
                        let after_uri = &body[params_start + uri_key + "\"uri\"".len()..];
                        if let Some(colon) = after_uri.find(':') {
                            let after_colon = after_uri[colon + 1..].trim_start();
                            if let Some(rest) = after_colon.strip_prefix('"') {
                                if let Some(end) = rest.find('"') {
                                    let uri = &rest[..end];
                                    if let Some(src) = open_files.get(uri) {
                                        for name in collect_local_names(src) {
                                            items.push(format!(
                                                r#"{{"label":"{}","kind":3,"detail":"local"}}"#,
                                                name
                                            ));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                let items_json = format!("[{}]", items.join(","));
                Some(lsp_response(
                    id_val.as_deref(),
                    &format!(r#"{{"isIncomplete":false,"items":{}}}"#, items_json),
                ))
            }
            Some("textDocument/hover") => {
                let resp = if let Some((uri, line, character)) = lsp_parse_td_position(&body) {
                    if let Some(src) = open_files.get(&uri) {
                        let lines: Vec<&str> = src.lines().collect();
                        let line_str = lines.get(line as usize).copied().unwrap_or("");
                        if let Some(word) = lsp_ident_byte_at(line_str, character) {
                            if builtin_completions.contains(&word.as_str()) {
                                let detail = serde_json::json!({
                                    "contents": {
                                        "kind": "markdown",
                                        "value": format!("`{}` — **builtin**", word)
                                    }
                                });
                                Some(lsp_response(id_val.as_deref(), &detail.to_string()))
                            } else {
                                let decl = collect_local_decls(src)
                                    .into_iter()
                                    .find(|d| d.name == word);
                                if let Some(d) = decl {
                                    let kind = if d.is_fn { "local fn" } else { "local let" };
                                    let detail = serde_json::json!({
                                        "contents": {
                                            "kind": "markdown",
                                            "value": format!("`{}` — **{}**", word, kind)
                                        }
                                    });
                                    Some(lsp_response(id_val.as_deref(), &detail.to_string()))
                                } else {
                                    Some(lsp_response(id_val.as_deref(), "null"))
                                }
                            }
                        } else {
                            Some(lsp_response(id_val.as_deref(), "null"))
                        }
                    } else {
                        Some(lsp_response(id_val.as_deref(), "null"))
                    }
                } else {
                    Some(lsp_response(id_val.as_deref(), "null"))
                };
                resp
            }
            Some("textDocument/definition") => {
                let resp = if let Some((uri, line, character)) = lsp_parse_td_position(&body) {
                    if let Some(src) = open_files.get(&uri) {
                        let lines: Vec<&str> = src.lines().collect();
                        let line_str = lines.get(line as usize).copied().unwrap_or("");
                        if let Some(word) = lsp_ident_byte_at(line_str, character) {
                            let decl = collect_local_decls(src)
                                .into_iter()
                                .find(|d| d.name == word);
                            if let Some(d) = decl {
                                let name_len = d.name.len() as u32;
                                let detail = serde_json::json!([{
                                    "uri": uri,
                                    "range": {
                                        "start": { "line": d.line, "character": d.col },
                                        "end": { "line": d.line, "character": d.col + name_len }
                                    }
                                }]);
                                Some(lsp_response(id_val.as_deref(), &detail.to_string()))
                            } else {
                                Some(lsp_response(id_val.as_deref(), "null"))
                            }
                        } else {
                            Some(lsp_response(id_val.as_deref(), "null"))
                        }
                    } else {
                        Some(lsp_response(id_val.as_deref(), "null"))
                    }
                } else {
                    Some(lsp_response(id_val.as_deref(), "null"))
                };
                resp
            }
            Some("shutdown") => Some(lsp_response(id_val.as_deref(), "null")),
            Some("exit") => break,
            _ => None,
        };
        if let Some(resp) = response {
            let msg = format!("Content-Length: {}\r\n\r\n{}", resp.len(), resp);
            let _ = writer.write_all(msg.as_bytes());
            let _ = writer.flush();
        }
    }
}

fn lsp_response(id: Option<&str>, result_json: &str) -> String {
    let id_part = id.unwrap_or("null");
    format!(
        r#"{{"jsonrpc":"2.0","id":{},"result":{}}}"#,
        id_part, result_json
    )
}

fn lsp_extract_str(json: &str, key: &str) -> Option<String> {
    let needle = format!("\"{}\"", key);
    let pos = json.find(&needle)?;
    let after = json[pos + needle.len()..].trim_start();
    let after = after.strip_prefix(':')?.trim_start();
    let after = after.strip_prefix('"')?;
    let end = after.find('"')?;
    Some(after[..end].to_string())
}

fn lsp_extract_id(json: &str) -> Option<String> {
    let needle = "\"id\"";
    let pos = json.find(needle)?;
    let after = json[pos + needle.len()..].trim_start();
    let after = after.strip_prefix(':')?.trim_start();
    if let Some(inner) = after.strip_prefix('"') {
        let end = inner.find('"')?;
        Some(format!("\"{}\"", &inner[..end]))
    } else {
        let end = after
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(after.len());
        Some(after[..end].to_string())
    }
}

const DEFAULT_MANIFEST: &str = r#"[package]
name = "my-app"
version = "0.1.0"

[dependencies]
"#;

fn copy_dir_all(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for e in std::fs::read_dir(src)? {
        let e = e?;
        let ty = e.file_type()?;
        let p = e.path();
        let name = p.file_name().unwrap_or(std::ffi::OsStr::new(""));
        let dest = dst.join(name);
        if ty.is_dir() {
            copy_dir_all(&p, &dest)?;
        } else {
            std::fs::copy(&p, &dest)?;
        }
    }
    Ok(())
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
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
        let registry_url = std::env::var("PBLANG_REGISTRY_INDEX").unwrap_or_else(|_| {
            eprintln!(
                "pblang publish: no registry configured (set PBLANG_REGISTRY_INDEX to registry base URL or file:// directory)"
            );
            std::process::exit(1);
        });
        // SECURITY FIX #28: Prefer the env var to avoid the token appearing in
        // shell history, `ps aux` process listings, and CI logs.
        // The --registry-token=<value> CLI flag is still supported for backward
        // compatibility, but a warning is printed to stderr when it is used.
        let token = std::env::var("PBLANG_REGISTRY_TOKEN").unwrap_or_else(|_| {
            for a in &rest {
                if let Some(t) = a.strip_prefix("--registry-token=") {
                    eprintln!(
                        "WARNING: passing --registry-token on the command line exposes the                          token in shell history and process listings.                          Prefer setting PBLANG_REGISTRY_TOKEN as an environment variable."
                    );
                    return t.to_string();
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
        match parse(&src_for_parse) {
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
        let ast = match parse(&src_for_parse) {
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
        let stmts = match parse(&src_for_parse) {
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
        let stmts = match parse(&src_for_parse) {
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
        let stmts = match parse(&src_for_parse) {
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
        let stmts = match parse(&src_masked) {
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
        let entry_path = resolve_package_entry(&man_dir).unwrap_or_else(|e| {
            eprintln!("pblang build: {}", e);
            std::process::exit(1);
        });
        let path_str = entry_path.to_string_lossy();
        let src = match std::fs::read_to_string(&entry_path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("pblang build: failed to read {}: {}", path_str, e);
                std::process::exit(1);
            }
        };
        match parse(&src) {
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

            // Create tarball of the package
            let tarball_bytes = pblang_pm::create_package_tarball(&man_dir).unwrap_or_else(|e| {
                eprintln!(
                    "pblang bundle --self-extract: failed to create tarball: {}",
                    e
                );
                std::process::exit(1);
            });

            // Find pblang-stub binary (look next to current executable)
            let current_exe = std::env::current_exe().expect("cannot read current executable path");
            let stub_path = current_exe
                .parent()
                .expect("cannot find parent directory of executable")
                .join(if cfg!(target_os = "windows") {
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
            println!("To run: {}", output_path.display());
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
        let path = args.get(1).expect("usage: pblang --check <file>");
        let src = std::fs::read_to_string(path).expect("failed to read file");
        match parse(&src) {
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
        let path = args
            .get(1)
            .expect("usage: pblang --doc <file> [--html] [out_dir]");
        let html_mode = args.get(2).map(|s| s.as_str()) == Some("--html");
        let out_dir = if html_mode {
            args.get(3).map(String::as_str).unwrap_or("docs")
        } else {
            ""
        };
        let src = std::fs::read_to_string(path).expect("failed to read file");

        fn render_ty(t: &TypeExpr) -> String {
            match &t.kind {
                TypeKind::Any => "any".into(),
                TypeKind::Number => "number".into(),
                TypeKind::Bool => "bool".into(),
                TypeKind::String => "string".into(),
                TypeKind::Nil => "nil".into(),
                TypeKind::Array(inner) => format!("array<{}>", render_ty(inner)),
                TypeKind::Map(inner) => format!("map<{}>", render_ty(inner)),
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
                                .map(|(n, t)| match t {
                                    Some(t) => format!("{n}: {}", render_ty(t)),
                                    None => n.clone(),
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
                .map(|e| e == "sap" || e == "sapp")
                .unwrap_or(false)
            {
                match std::fs::read_to_string(&path) {
                    Ok(src) => {
                        let lines: Vec<&str> = src.lines().collect();
                        let mut out = String::new();
                        let mut depth: i32 = 0;
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
                                        break;
                                    }
                                    let break_at =
                                        line_to_output[..max_len].rfind(' ').unwrap_or(max_len);
                                    let break_at = if break_at == 0 { max_len } else { break_at };
                                    let rest = line_to_output.split_off(break_at);
                                    out.push_str(line_to_output.trim_end());
                                    out.push('\n');
                                    line_to_output = format!("{}{}", indent_str, rest.trim_start());
                                }
                            } else {
                                out.push_str(&full_line);
                                out.push('\n');
                            }
                            last_line_was_blank = false;
                            if trimmed.ends_with('{') {
                                depth += 1;
                            }
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
                    let top = scope.pop().expect("scope stack underflow: push/pop mismatch in lint walk");
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
                    let top = scope.pop().expect("scope stack underflow: push/pop mismatch in lint walk");
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
                    scope.last_mut().expect("lint walk: scope is non-empty (push before every nested scope)")
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
                    let top = scope.pop().expect("scope stack underflow: push/pop mismatch in lint walk");
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
                        let top = scope.pop().expect("scope stack underflow: push/pop mismatch in lint walk");
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
                    let top = scope.pop().expect("scope stack underflow: push/pop mismatch in lint walk");
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
                    scope.last_mut().expect("lint walk: scope is non-empty (push before every nested scope)")
                        .insert(name.clone(), (s.span, false, false));
                    scope.push(std::collections::HashMap::new());
                    for s in body {
                        walk_stmt(s, scope, warnings);
                    }
                    let top = scope.pop().expect("scope stack underflow: push/pop mismatch in lint walk");
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
                    let top = scope.pop().expect("scope stack underflow: push/pop mismatch in lint walk");
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
                    let top = scope.pop().expect("scope stack underflow: push/pop mismatch in lint walk");
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
        let top = scope.pop().expect("scope stack underflow: push/pop mismatch in lint walk");
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
            match parse(&src) {
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
        run_lsp_server();
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
            let stmts = match parse(&src_for_parse) {
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
                resolve_package_entry(&man_dir).unwrap_or_else(|e| {
                    eprintln!("pblang run: {}", e);
                    std::process::exit(1);
                })
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
            resolve_package_entry(&man_dir).unwrap_or_else(|e| {
                eprintln!("pblang run: {}", e);
                std::process::exit(1);
            })
        };
        if let Some(man_dir) = script_path
            .parent()
            .and_then(pblang_pm::find_manifest_dir)
        {
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
        let src = match std::fs::read_to_string(&script_path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!(
                    "pblang run: failed to read '{}': {}",
                    script_path.display(),
                    e
                );
                std::process::exit(1);
            }
        };
        let src_for_parse = strip_comments_mask(&src);
        let stmts = match parse(&src_for_parse) {
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
        unsafe { std::env::set_var("PBLANG_SCRIPT_ARGS", script_args.join("\n")) };

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

    let mut vm = VM::with_stdlib(StdlibConfig::default());

    println!("Welcome to Biapryx REPL!");
    println!("Type 'help' for commands.");

    let mut buf = String::new();
    let mut brace_depth: i32 = 0;
    let mut in_string = false;
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
            &mut block_comment_depth,
            &mut brace_depth,
        );

        // Only attempt parse when braces balanced AND we're not inside a string.
        if brace_depth <= 0 && !in_string && block_comment_depth == 0 {
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
                                eprintln!("Runtime error at {src_name}:{ln}:{col}: {}", e.message);
                                let line = get_line(src, ln);
                                eprintln!("{line}");
                                eprintln!("{}^", " ".repeat(col.saturating_sub(1)));
                            } else {
                                eprintln!("Runtime error ({src_name}): {}", e.message);
                            }

                            if !e.stack.is_empty() {
                                eprintln!("Stack trace:");
                                for frame in &e.stack {
                                    let fname = frame.function.as_deref().unwrap_or("<anonymous>");
                                    let fsrc = frame.source_name.as_deref().unwrap_or("<repl>");
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
