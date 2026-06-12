//! Polyglot builtin implementations.
//!
//! Free functions backing the `polyglot_*` builtins registered in
//! `VM::builtins()` (vm.rs):
//!
//!   - `polyglot_exec(lang, code)` — interpreted-language subprocess
//!     bridge (python / js / bash / ruby / perl / lua / php / r / tcl /
//!     awk / julia / racket / scheme / clojure / elixir / groovy / fennel
//!     / ocaml / scala / csharp / deno / zsh / fish / powershell / raku /
//!     qsharp / sql / sqlite / postgresql / mysql / mongodb / redis /
//!     matlab / octave / prolog / smalltalk / forth / idris / sass / less /
//!     bc / dc / gnuplot / io).
//!   - `polyglot_exec_with_input(lang, code, input)` — stdin filter form.
//!   - `polyglot_exec_compiled(lang, code)` — temp-dir compile-and-run for
//!     rust / go / c / cpp / zig / java / kotlin / typescript / pascal /
//!     nim / crystal / d / haskell / fortran / ada / swift / cobol /
//!     erlang / vala / objc / nasm / gas / v / ocaml-compiled / csharp /
//!     fsharp / vb / qsharp-compiled / haxe / pony / red / verilog /
//!     mercury / reason.
//!   - `polyglot_detect()` — probe host PATH for every known language.
//!   - `polyglot_compile_cached(lang, code)` — build-time cdylib cache for
//!     17 languages (rust, c, cpp, go, pascal, csharp+fsharp NativeAOT,
//!     zig, d, nim, swift, haskell, fortran, objc, ocaml, java GraalVM,
//!     kotlin Kotlin/Native).
//!   - `polyglot_cache_clear()` / `polyglot_cache_info()` — cache mgmt.
//!
//! Visibility: every item is `pub(super)` so vm.rs reaches them via
//! `fun: polyglot::polyglot_exec` in the BuiltinFn table.

use std::sync::atomic::Ordering;

use libloading::Library;
use pblang_ir::{alloc_string, RuntimeError, Value, VmHost};

use super::errors::vm_cap;
use super::handles::{with_ffi_table, FFI_NEXT};
use super::FfiResource;

/// polyglot_exec(lang, code) — Execute code in a foreign language via subprocess.
/// Supported langs: python, js (node), bash (sh), ruby, perl
/// Returns JSON: { success, stdout, stderr, exit_code }
pub(super) fn polyglot_exec(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    if !host.allow_exec() {
        return Err(vm_cap!(host, "polyglot_exec", "exec"));
    }
    if args.len() != 2 {
        return Err(host.err(format!(
            "polyglot_exec() expects 2 arguments (lang, code), got {}",
            args.len()
        )));
    }
    let lang = args[0]
        .as_str()
        .map_err(|m| host.err(format!("polyglot_exec(): {m}")))?;
    let code = args[1]
        .as_str()
        .map_err(|m| host.err(format!("polyglot_exec(): {m}")))?;

    let (interpreter, arg) = match lang.to_lowercase().as_str() {
        "python" | "py" => ("python3", Some("-c")),
        "js" | "javascript" | "node" => ("node", Some("-e")),
        "deno" => ("deno", Some("eval")),
        "bash" | "sh" | "shell" => ("bash", Some("-c")),
        "zsh" => ("zsh", Some("-c")),
        "fish" => ("fish", Some("-c")),
        "powershell" | "pwsh" | "ps1" => ("pwsh", Some("-c")),
        "ruby" | "rb" => ("ruby", Some("-e")),
        "perl" | "pl" => ("perl", Some("-e")),
        "raku" => ("raku", Some("-e")),
        "lua" => ("lua", Some("-e")),
        "php" => ("php", Some("-r")),
        "r" => ("Rscript", Some("-e")),
        "tcl" => ("tclsh", Some("-c")),
        "awk" => ("awk", None),
        "julia" | "jl" => ("julia", Some("-e")),
        "racket" => ("racket", Some("-e")),
        "scheme" | "guile" => ("guile", Some("-c")),
        "clojure" | "clj" => ("clojure", Some("-M")),
        "elixir" | "ex" | "exs" => ("elixir", Some("-e")),
        "groovy" => ("groovy", Some("-e")),
        "tclkit" => ("tclkit", Some("-c")),
        "fennel" => ("fennel", Some("-e")),
        "ocaml" => ("ocaml", Some("-e")),
        "scala" => ("scala", Some("-e")),
        // C# via dotnet-script (REPL-style single-file eval).
        // Requires `dotnet tool install -g dotnet-script`.
        "csharp" | "cs" | "c#" => ("dotnet-script", Some("eval")),
        // ── Q# (Microsoft Quantum) ──────────────────────────────
        "qsharp" | "qs" | "q#" => ("python3", Some("-c")),
        // ── SQL / databases ─────────────────────────────────────
        "sql" | "sqlite" => ("sqlite3", Some(":memory:")),
        // ── MATLAB / Octave ─────────────────────────────────────
        "matlab" | "octave" => ("octave", Some("--eval")),
        // ── Prolog ──────────────────────────────────────────────
        "prolog" | "plg" => ("swipl", Some("-g")),
        // ── Smalltalk ───────────────────────────────────────────
        "smalltalk" | "st" => ("gst", Some("-f")),
        // ── Forth ───────────────────────────────────────────────
        "forth" | "gforth" | "fth" => ("gforth", Some("-e")),
        // ── Idris ───────────────────────────────────────────────
        "idris" | "idr" => ("idris2", Some("--exec")),
        // ── CSS preprocessors ───────────────────────────────────
        "sass" | "scss" => ("sass", Some("--stdin")),
        "less" => ("lessc", Some("-")),
        // ── Database CLIs ───────────────────────────────────────
        "postgresql" | "psql" | "pg" => ("psql", Some("-f")),
        "mysql" | "mariadb" => ("mysql", Some("-e")),
        "mongodb" | "mongo" => ("mongosh", Some("--eval")),
        "redis" => ("redis-cli", Some("--eval")),
        // ── Other scriptable tools ──────────────────────────────
        "bc" => ("bc", Some("-l")),
        "dc" => ("dc", None),
        "gnuplot" | "gp" => ("gnuplot", Some("-e")),
        "io" => ("io", Some("-e")),
        // ── Additional databases ──────────────────────────────────
        "duckdb" => ("duckdb", Some("-c")),
        "cassandra" | "cqlsh" => ("cqlsh", Some("-e")),
        "neo4j" | "cypher" => ("cypher-shell", Some("-e")),
        "clickhouse" => ("clickhouse-client", Some("-q")),
        // ── More scripting languages ─────────────────────────────
        "lisp" | "sbcl" => ("sbcl", Some("--eval")),
        "janet" => ("janet", Some("-e")),
        "wren" => ("wren_cli", None),
        "rholang" | "rho" => ("rho", Some("-e")),
        "maxima" => ("maxima", Some("--batch-string")),
        "gap" => ("gap", Some("-c")),
        "pari" => ("gp", Some("-q")),
        // F# via dotnet fsi --use:- ... we pipe instead; for the
        // single-shot `-e`-style we use `fsi --exec --use:` which
        // doesn't exist, so fsi is interactive-only. Skip here; route
        // F# through polyglot_exec_compiled instead.
        other => {
            return Err(host.err(format!(
                "polyglot_exec(): unsupported language '{}'. Try polyglot_exec_compiled for compiled languages, or see polyglot_detect() for the full list.",
                other
            )));
        }
    };

    let mut cmd = std::process::Command::new(interpreter);
    if let Some(flag) = arg {
        cmd.arg(flag);
    }
    cmd.arg(code)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    let output = match cmd.output() {
        Ok(o) => o,
        Err(e) => {
            // Interpreter not found
            let json = format!(
                "{{\"success\":false,\"stdout\":\"\",\"stderr\":\"polyglot_exec: {interpreter} not found: {e}\",\"exit_code\":-1}}"
            );
            return Ok(alloc_string(json));
        }
    };

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let exit_code = output.status.code().unwrap_or(-1);
    let success = output.status.success();

    // Escape JSON strings
    let stdout_esc = stdout
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t");
    let stderr_esc = stderr
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t");

    let json = format!(
        "{{\"success\":{success},\"stdout\":\"{stdout_esc}\",\"stderr\":\"{stderr_esc}\",\"exit_code\":{exit_code}}}"
    );
    Ok(alloc_string(json))
}

// Shared helper: JSON-escape a string for embedding in a "..." field.
pub(super) fn json_escape_string(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

/// polyglot_exec_with_input(lang, code, input) — like polyglot_exec but
/// pipes `input` to the subprocess's stdin. Lets a polyglot block act
/// as a stdin → stdout filter (e.g. transform JSON with jq-like
/// Python or Ruby snippets, sort with awk, etc.).
pub(super) fn polyglot_exec_with_input(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    use std::io::Write;
    if !host.allow_exec() {
        return Err(vm_cap!(host, "polyglot_exec_with_input", "exec"));
    }
    if args.len() != 3 {
        return Err(host.err(format!(
            "polyglot_exec_with_input() expects 3 arguments (lang, code, input), got {}",
            args.len()
        )));
    }
    let lang = args[0]
        .as_str()
        .map_err(|m| host.err(format!("polyglot_exec_with_input(): {m}")))?;
    let code = args[1]
        .as_str()
        .map_err(|m| host.err(format!("polyglot_exec_with_input(): {m}")))?;
    let input = args[2]
        .as_str()
        .map_err(|m| host.err(format!("polyglot_exec_with_input(): {m}")))?;

    let (interpreter, arg) = match lang.to_lowercase().as_str() {
        "python" | "py" => ("python3", "-c"),
        "js" | "javascript" | "node" => ("node", "-e"),
        "bash" | "sh" | "shell" => ("bash", "-c"),
        "ruby" | "rb" => ("ruby", "-e"),
        "perl" | "pl" => ("perl", "-e"),
        "lua" => ("lua", "-e"),
        "php" => ("php", "-r"),
        "r" => ("Rscript", "-e"),
        "deno" => ("deno", "eval"),
        "zsh" => ("zsh", "-c"),
        "fish" => ("fish", "-c"),
        "powershell" | "pwsh" => ("pwsh", "-c"),
        "raku" => ("raku", "-e"),
        "tcl" => ("tclsh", "-c"),
        "awk" => ("awk", ""),
        "julia" | "jl" => ("julia", "-e"),
        "racket" => ("racket", "-e"),
        "scheme" | "guile" => ("guile", "-c"),
        "clojure" | "clj" => ("clojure", "-M"),
        "elixir" | "ex" | "exs" => ("elixir", "-e"),
        "groovy" => ("groovy", "-e"),
        "fennel" => ("fennel", "-e"),
        "scala" => ("scala", "-e"),
        "qsharp" | "qs" => ("python3", "-c"),
        "sql" | "sqlite" => ("sqlite3", ":memory:"),
        "octave" => ("octave", "--eval"),
        "forth" | "gforth" => ("gforth", "-e"),
        other => {
            return Err(host.err(format!(
                "polyglot_exec_with_input(): unsupported language '{}'",
                other
            )));
        }
    };

    let mut cmd = std::process::Command::new(interpreter);
    cmd.arg(arg)
        .arg(code)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            let stderr_esc = json_escape_string(&format!(
                "polyglot_exec_with_input: {interpreter} not found: {e}"
            ));
            return Ok(alloc_string(format!(
                "{{\"success\":false,\"stdout\":\"\",\"stderr\":\"{stderr_esc}\",\"exit_code\":-1}}"
            )));
        }
    };
    if let Some(mut stdin_pipe) = child.stdin.take() {
        let _ = stdin_pipe.write_all(input.as_bytes());
    }
    let output = match child.wait_with_output() {
        Ok(o) => o,
        Err(e) => {
            let stderr_esc =
                json_escape_string(&format!("polyglot_exec_with_input: wait failed: {e}"));
            return Ok(alloc_string(format!(
                "{{\"success\":false,\"stdout\":\"\",\"stderr\":\"{stderr_esc}\",\"exit_code\":-1}}"
            )));
        }
    };

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let exit_code = output.status.code().unwrap_or(-1);
    let success = output.status.success();

    let json = format!(
        "{{\"success\":{success},\"stdout\":\"{}\",\"stderr\":\"{}\",\"exit_code\":{exit_code}}}",
        json_escape_string(&stdout),
        json_escape_string(&stderr)
    );
    Ok(alloc_string(json))
}

/// polyglot_exec_compiled(lang, code) — compile-then-run path for
/// languages that aren't trivially interpreted. Writes `code` to a
/// temp file, invokes the language's compiler, runs the result,
/// captures stdout/stderr, then cleans up the temp tree.
///
/// Supported languages and toolchains:
///   rust  → rustc (single-file edition 2021, --edition 2021 -O)
///   go    → `go run` (single-file program)
///   c     → cc / gcc / clang -O2
///   cpp/c++ → c++ / g++ / clang++ -O2 -std=c++17
///   zig   → `zig run`
///   java  → javac + java (assumes public class Main)
///   ts/typescript → tsc + node (writes .ts, compiles to .js)
///
/// Returns the same JSON shape as polyglot_exec:
///   { success, stdout, stderr, exit_code }
pub(super) fn polyglot_exec_compiled(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    if !host.allow_exec() {
        return Err(vm_cap!(host, "polyglot_exec_compiled", "exec"));
    }
    if args.len() != 2 {
        return Err(host.err(format!(
            "polyglot_exec_compiled() expects 2 arguments (lang, code), got {}",
            args.len()
        )));
    }
    let lang = args[0]
        .as_str()
        .map_err(|m| host.err(format!("polyglot_exec_compiled(): {m}")))?;
    let code = args[1]
        .as_str()
        .map_err(|m| host.err(format!("polyglot_exec_compiled(): {m}")))?;

    let lang_lc = lang.to_lowercase();
    let lang_norm = match lang_lc.as_str() {
        "rs" => "rust",
        "cxx" | "c++" => "cpp",
        "ts" => "typescript",
        "pas" | "delphi" | "freepascal" => "pascal",
        "kt" => "kotlin",
        "hs" => "haskell",
        "f90" | "f95" | "f03" | "f08" | "f" => "fortran",
        "adb" => "ada",
        "cob" => "cobol",
        "erl" => "erlang",
        "vala" => "vala",
        "objc" | "m" | "objective-c" => "objc",
        "asm" | "s" | "nasm" => "nasm",
        "gas" => "gas",
        "cs" | "c#" => "csharp",
        "fs" | "fsx" | "f#" => "fsharp",
        "vbnet" | "visualbasic" => "vb",
        "chpl" => "chapel",
        "purs" => "purescript",
        "sol" => "solidity",
        "odin" => "odin",
        "mojo" => "mojo",
        "carbon" => "carbon",
        other => other,
    };

    // Make a per-invocation temp dir so we can RAII-delete it.
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let work_dir = std::env::temp_dir().join(format!("pblang_polyglot_{lang_norm}_{nonce}"));
    if let Err(e) = std::fs::create_dir_all(&work_dir) {
        let stderr_esc = json_escape_string(&format!(
            "polyglot_exec_compiled: cannot create temp dir: {e}"
        ));
        return Ok(alloc_string(format!(
            "{{\"success\":false,\"stdout\":\"\",\"stderr\":\"{stderr_esc}\",\"exit_code\":-1}}"
        )));
    }
    struct DirCleanup(std::path::PathBuf);
    impl Drop for DirCleanup {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let _cleanup = DirCleanup(work_dir.clone());

    // Try a list of candidate toolchain commands; first one on PATH wins.
    fn first_available(cands: &[&str]) -> Option<String> {
        for c in cands {
            let probe = std::process::Command::new(c)
                .arg("--version")
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status();
            if probe.map(|s| s.success()).unwrap_or(false) {
                return Some((*c).to_string());
            }
        }
        None
    }

    // Build & run plan: (src filename, compile argv builder, run argv).
    // We materialize the source, then optionally compile, then run.
    let (src_name, compile_cmd, run_cmd_path): (&str, Option<(String, Vec<String>)>, Vec<String>) =
        match lang_norm {
            "rust" => {
                let cc = match first_available(&["rustc"]) {
                    Some(c) => c,
                    None => {
                        let stderr_esc =
                            json_escape_string("polyglot_exec_compiled: rustc not found in PATH");
                        return Ok(alloc_string(format!(
                        "{{\"success\":false,\"stdout\":\"\",\"stderr\":\"{stderr_esc}\",\"exit_code\":-1}}"
                    )));
                    }
                };
                let bin = work_dir.join(if cfg!(windows) { "prog.exe" } else { "prog" });
                let src = work_dir.join("main.rs");
                (
                    "main.rs",
                    Some((
                        cc,
                        vec![
                            "--edition".into(),
                            "2021".into(),
                            "-O".into(),
                            src.to_string_lossy().into_owned(),
                            "-o".into(),
                            bin.to_string_lossy().into_owned(),
                        ],
                    )),
                    vec![bin.to_string_lossy().into_owned()],
                )
            }
            "go" => {
                let cc = match first_available(&["go"]) {
                    Some(c) => c,
                    None => {
                        let stderr_esc =
                            json_escape_string("polyglot_exec_compiled: go not found in PATH");
                        return Ok(alloc_string(format!(
                        "{{\"success\":false,\"stdout\":\"\",\"stderr\":\"{stderr_esc}\",\"exit_code\":-1}}"
                    )));
                    }
                };
                let src = work_dir.join("main.go");
                (
                    "main.go",
                    None,
                    vec![cc, "run".into(), src.to_string_lossy().into_owned()],
                )
            }
            "c" => {
                let cc = match first_available(&["cc", "gcc", "clang"]) {
                    Some(c) => c,
                    None => {
                        let stderr_esc = json_escape_string(
                            "polyglot_exec_compiled: no C compiler found (tried cc, gcc, clang)",
                        );
                        return Ok(alloc_string(format!(
                        "{{\"success\":false,\"stdout\":\"\",\"stderr\":\"{stderr_esc}\",\"exit_code\":-1}}"
                    )));
                    }
                };
                let bin = work_dir.join(if cfg!(windows) { "prog.exe" } else { "prog" });
                let src = work_dir.join("main.c");
                (
                    "main.c",
                    Some((
                        cc,
                        vec![
                            "-O2".into(),
                            src.to_string_lossy().into_owned(),
                            "-o".into(),
                            bin.to_string_lossy().into_owned(),
                        ],
                    )),
                    vec![bin.to_string_lossy().into_owned()],
                )
            }
            "cpp" => {
                let cc = match first_available(&["c++", "g++", "clang++"]) {
                    Some(c) => c,
                    None => {
                        let stderr_esc = json_escape_string(
                        "polyglot_exec_compiled: no C++ compiler found (tried c++, g++, clang++)",
                    );
                        return Ok(alloc_string(format!(
                        "{{\"success\":false,\"stdout\":\"\",\"stderr\":\"{stderr_esc}\",\"exit_code\":-1}}"
                    )));
                    }
                };
                let bin = work_dir.join(if cfg!(windows) { "prog.exe" } else { "prog" });
                let src = work_dir.join("main.cpp");
                (
                    "main.cpp",
                    Some((
                        cc,
                        vec![
                            "-O2".into(),
                            "-std=c++17".into(),
                            src.to_string_lossy().into_owned(),
                            "-o".into(),
                            bin.to_string_lossy().into_owned(),
                        ],
                    )),
                    vec![bin.to_string_lossy().into_owned()],
                )
            }
            "zig" => {
                let cc = match first_available(&["zig"]) {
                    Some(c) => c,
                    None => {
                        let stderr_esc =
                            json_escape_string("polyglot_exec_compiled: zig not found in PATH");
                        return Ok(alloc_string(format!(
                        "{{\"success\":false,\"stdout\":\"\",\"stderr\":\"{stderr_esc}\",\"exit_code\":-1}}"
                    )));
                    }
                };
                let src = work_dir.join("main.zig");
                (
                    "main.zig",
                    None,
                    vec![cc, "run".into(), src.to_string_lossy().into_owned()],
                )
            }
            "java" => {
                let javac = match first_available(&["javac"]) {
                    Some(c) => c,
                    None => {
                        let stderr_esc =
                            json_escape_string("polyglot_exec_compiled: javac not found in PATH");
                        return Ok(alloc_string(format!(
                        "{{\"success\":false,\"stdout\":\"\",\"stderr\":\"{stderr_esc}\",\"exit_code\":-1}}"
                    )));
                    }
                };
                let java = first_available(&["java"]).unwrap_or_else(|| "java".to_string());
                let src = work_dir.join("Main.java");
                (
                    "Main.java",
                    Some((javac, vec![src.to_string_lossy().into_owned()])),
                    vec![
                        java,
                        "-cp".into(),
                        work_dir.to_string_lossy().into_owned(),
                        "Main".into(),
                    ],
                )
            }
            "typescript" => {
                let tsc = match first_available(&["tsc"]) {
                    Some(c) => c,
                    None => {
                        let stderr_esc =
                            json_escape_string("polyglot_exec_compiled: tsc not found in PATH");
                        return Ok(alloc_string(format!(
                        "{{\"success\":false,\"stdout\":\"\",\"stderr\":\"{stderr_esc}\",\"exit_code\":-1}}"
                    )));
                    }
                };
                let node = first_available(&["node"]).unwrap_or_else(|| "node".to_string());
                let src = work_dir.join("main.ts");
                let js = work_dir.join("main.js");
                (
                    "main.ts",
                    Some((
                        tsc,
                        vec![
                            "--target".into(),
                            "es2020".into(),
                            "--module".into(),
                            "commonjs".into(),
                            src.to_string_lossy().into_owned(),
                        ],
                    )),
                    vec![node, js.to_string_lossy().into_owned()],
                )
            }
            "pascal" => {
                // Free Pascal Compiler. Output is named `prog`.
                let cc = match first_available(&["fpc"]) {
                    Some(c) => c,
                    None => {
                        let stderr_esc = json_escape_string(
                            "polyglot_exec_compiled: fpc (Free Pascal) not found in PATH",
                        );
                        return Ok(alloc_string(format!(
                        "{{\"success\":false,\"stdout\":\"\",\"stderr\":\"{stderr_esc}\",\"exit_code\":-1}}"
                    )));
                    }
                };
                let bin = work_dir.join(if cfg!(windows) { "prog.exe" } else { "prog" });
                let src = work_dir.join("main.pas");
                (
                    "main.pas",
                    Some((
                        cc,
                        vec![
                            format!("-o{}", bin.to_string_lossy()),
                            src.to_string_lossy().into_owned(),
                        ],
                    )),
                    vec![bin.to_string_lossy().into_owned()],
                )
            }
            "kotlin" => {
                // Kotlin: kotlinc compiles to JAR, then java -jar runs it.
                // Assumes top-level `fun main() { ... }` in the user's code.
                let kc = match first_available(&["kotlinc"]) {
                    Some(c) => c,
                    None => {
                        let stderr_esc =
                            json_escape_string("polyglot_exec_compiled: kotlinc not found in PATH");
                        return Ok(alloc_string(format!(
                        "{{\"success\":false,\"stdout\":\"\",\"stderr\":\"{stderr_esc}\",\"exit_code\":-1}}"
                    )));
                    }
                };
                let java = first_available(&["java"]).unwrap_or_else(|| "java".to_string());
                let src = work_dir.join("main.kt");
                let jar = work_dir.join("prog.jar");
                (
                    "main.kt",
                    Some((
                        kc,
                        vec![
                            src.to_string_lossy().into_owned(),
                            "-include-runtime".into(),
                            "-d".into(),
                            jar.to_string_lossy().into_owned(),
                        ],
                    )),
                    vec![java, "-jar".into(), jar.to_string_lossy().into_owned()],
                )
            }
            "nim" => {
                let cc = match first_available(&["nim"]) {
                    Some(c) => c,
                    None => {
                        let stderr_esc =
                            json_escape_string("polyglot_exec_compiled: nim not found in PATH");
                        return Ok(alloc_string(format!(
                        "{{\"success\":false,\"stdout\":\"\",\"stderr\":\"{stderr_esc}\",\"exit_code\":-1}}"
                    )));
                    }
                };
                let src = work_dir.join("main.nim");
                // `nim r` = compile + run, single invocation.
                (
                    "main.nim",
                    None,
                    vec![
                        cc,
                        "r".into(),
                        "--hints:off".into(),
                        src.to_string_lossy().into_owned(),
                    ],
                )
            }
            "crystal" => {
                let cc = match first_available(&["crystal"]) {
                    Some(c) => c,
                    None => {
                        let stderr_esc =
                            json_escape_string("polyglot_exec_compiled: crystal not found in PATH");
                        return Ok(alloc_string(format!(
                        "{{\"success\":false,\"stdout\":\"\",\"stderr\":\"{stderr_esc}\",\"exit_code\":-1}}"
                    )));
                    }
                };
                let src = work_dir.join("main.cr");
                (
                    "main.cr",
                    None,
                    vec![cc, "run".into(), src.to_string_lossy().into_owned()],
                )
            }
            "d" => {
                let cc = match first_available(&["dmd", "ldc2", "gdc"]) {
                    Some(c) => c,
                    None => {
                        let stderr_esc = json_escape_string(
                            "polyglot_exec_compiled: no D compiler found (tried dmd, ldc2, gdc)",
                        );
                        return Ok(alloc_string(format!(
                        "{{\"success\":false,\"stdout\":\"\",\"stderr\":\"{stderr_esc}\",\"exit_code\":-1}}"
                    )));
                    }
                };
                let bin = work_dir.join(if cfg!(windows) { "prog.exe" } else { "prog" });
                let src = work_dir.join("main.d");
                // dmd: -of=path; ldc2: -of=path; both accept this.
                (
                    "main.d",
                    Some((
                        cc,
                        vec![
                            format!("-of={}", bin.to_string_lossy()),
                            src.to_string_lossy().into_owned(),
                        ],
                    )),
                    vec![bin.to_string_lossy().into_owned()],
                )
            }
            "haskell" => {
                // runghc = compile-and-run in one shot.
                let cc = match first_available(&["runghc", "runhaskell", "ghc"]) {
                    Some(c) => c,
                    None => {
                        let stderr_esc = json_escape_string(
                        "polyglot_exec_compiled: no Haskell runner found (tried runghc, runhaskell, ghc)",
                    );
                        return Ok(alloc_string(format!(
                        "{{\"success\":false,\"stdout\":\"\",\"stderr\":\"{stderr_esc}\",\"exit_code\":-1}}"
                    )));
                    }
                };
                let src = work_dir.join("Main.hs");
                if cc.ends_with("ghc") && !cc.contains("runghc") && !cc.contains("runhaskell") {
                    // Plain ghc → compile then run separately.
                    let bin = work_dir.join(if cfg!(windows) { "prog.exe" } else { "prog" });
                    (
                        "Main.hs",
                        Some((
                            cc,
                            vec![
                                src.to_string_lossy().into_owned(),
                                "-o".into(),
                                bin.to_string_lossy().into_owned(),
                            ],
                        )),
                        vec![bin.to_string_lossy().into_owned()],
                    )
                } else {
                    (
                        "Main.hs",
                        None,
                        vec![cc, src.to_string_lossy().into_owned()],
                    )
                }
            }
            "fortran" => {
                let cc = match first_available(&["gfortran", "flang"]) {
                    Some(c) => c,
                    None => {
                        let stderr_esc = json_escape_string(
                        "polyglot_exec_compiled: no Fortran compiler found (tried gfortran, flang)",
                    );
                        return Ok(alloc_string(format!(
                        "{{\"success\":false,\"stdout\":\"\",\"stderr\":\"{stderr_esc}\",\"exit_code\":-1}}"
                    )));
                    }
                };
                let bin = work_dir.join(if cfg!(windows) { "prog.exe" } else { "prog" });
                let src = work_dir.join("main.f90");
                (
                    "main.f90",
                    Some((
                        cc,
                        vec![
                            "-O2".into(),
                            src.to_string_lossy().into_owned(),
                            "-o".into(),
                            bin.to_string_lossy().into_owned(),
                        ],
                    )),
                    vec![bin.to_string_lossy().into_owned()],
                )
            }
            "ada" => {
                let cc = match first_available(&["gnatmake"]) {
                    Some(c) => c,
                    None => {
                        let stderr_esc = json_escape_string(
                            "polyglot_exec_compiled: gnatmake (GNAT) not found in PATH",
                        );
                        return Ok(alloc_string(format!(
                        "{{\"success\":false,\"stdout\":\"\",\"stderr\":\"{stderr_esc}\",\"exit_code\":-1}}"
                    )));
                    }
                };
                let src = work_dir.join("main.adb");
                let bin = work_dir.join(if cfg!(windows) { "main.exe" } else { "main" });
                (
                    "main.adb",
                    Some((cc, vec![src.to_string_lossy().into_owned()])),
                    vec![bin.to_string_lossy().into_owned()],
                )
            }
            "swift" => {
                let cc = match first_available(&["swift"]) {
                    Some(c) => c,
                    None => {
                        let stderr_esc =
                            json_escape_string("polyglot_exec_compiled: swift not found in PATH");
                        return Ok(alloc_string(format!(
                        "{{\"success\":false,\"stdout\":\"\",\"stderr\":\"{stderr_esc}\",\"exit_code\":-1}}"
                    )));
                    }
                };
                let src = work_dir.join("main.swift");
                (
                    "main.swift",
                    None,
                    vec![cc, src.to_string_lossy().into_owned()],
                )
            }
            "cobol" => {
                let cc = match first_available(&["cobc"]) {
                    Some(c) => c,
                    None => {
                        let stderr_esc = json_escape_string(
                            "polyglot_exec_compiled: cobc (GnuCOBOL) not found in PATH",
                        );
                        return Ok(alloc_string(format!(
                        "{{\"success\":false,\"stdout\":\"\",\"stderr\":\"{stderr_esc}\",\"exit_code\":-1}}"
                    )));
                    }
                };
                let bin = work_dir.join(if cfg!(windows) { "prog.exe" } else { "prog" });
                let src = work_dir.join("main.cob");
                (
                    "main.cob",
                    Some((
                        cc,
                        vec![
                            "-x".into(),
                            "-o".into(),
                            bin.to_string_lossy().into_owned(),
                            src.to_string_lossy().into_owned(),
                        ],
                    )),
                    vec![bin.to_string_lossy().into_owned()],
                )
            }
            "erlang" => {
                let cc = match first_available(&["escript"]) {
                    Some(c) => c,
                    None => {
                        let stderr_esc = json_escape_string(
                            "polyglot_exec_compiled: escript (Erlang) not found in PATH",
                        );
                        return Ok(alloc_string(format!(
                        "{{\"success\":false,\"stdout\":\"\",\"stderr\":\"{stderr_esc}\",\"exit_code\":-1}}"
                    )));
                    }
                };
                let src = work_dir.join("main.escript");
                (
                    "main.escript",
                    None,
                    vec![cc, src.to_string_lossy().into_owned()],
                )
            }
            "vala" => {
                let cc = match first_available(&["valac"]) {
                    Some(c) => c,
                    None => {
                        let stderr_esc =
                            json_escape_string("polyglot_exec_compiled: valac not found in PATH");
                        return Ok(alloc_string(format!(
                        "{{\"success\":false,\"stdout\":\"\",\"stderr\":\"{stderr_esc}\",\"exit_code\":-1}}"
                    )));
                    }
                };
                let bin = work_dir.join(if cfg!(windows) { "prog.exe" } else { "prog" });
                let src = work_dir.join("main.vala");
                (
                    "main.vala",
                    Some((
                        cc,
                        vec![
                            src.to_string_lossy().into_owned(),
                            "-o".into(),
                            bin.to_string_lossy().into_owned(),
                        ],
                    )),
                    vec![bin.to_string_lossy().into_owned()],
                )
            }
            "objc" => {
                let cc = match first_available(&["clang", "gcc"]) {
                    Some(c) => c,
                    None => {
                        let stderr_esc = json_escape_string(
                        "polyglot_exec_compiled: clang/gcc not found in PATH (Objective-C needs an Obj-C-capable compiler)",
                    );
                        return Ok(alloc_string(format!(
                        "{{\"success\":false,\"stdout\":\"\",\"stderr\":\"{stderr_esc}\",\"exit_code\":-1}}"
                    )));
                    }
                };
                let bin = work_dir.join(if cfg!(windows) { "prog.exe" } else { "prog" });
                let src = work_dir.join("main.m");
                // -ObjC + -lobjc on Linux/GNUstep; macOS auto-detects via .m.
                let mut comp_args = vec![
                    "-O2".into(),
                    src.to_string_lossy().into_owned(),
                    "-o".into(),
                    bin.to_string_lossy().into_owned(),
                ];
                if cfg!(target_os = "linux") {
                    comp_args.push("-lobjc".into());
                }
                (
                    "main.m",
                    Some((cc, comp_args)),
                    vec![bin.to_string_lossy().into_owned()],
                )
            }
            "nasm" => {
                // x86_64 ELF assembly via nasm + ld. Linux-only path.
                let cc = match first_available(&["nasm"]) {
                    Some(c) => c,
                    None => {
                        let stderr_esc =
                            json_escape_string("polyglot_exec_compiled: nasm not found in PATH");
                        return Ok(alloc_string(format!(
                        "{{\"success\":false,\"stdout\":\"\",\"stderr\":\"{stderr_esc}\",\"exit_code\":-1}}"
                    )));
                    }
                };
                let ld = first_available(&["ld"]).unwrap_or_else(|| "ld".to_string());
                let src = work_dir.join("main.asm");
                let obj = work_dir.join("main.o");
                let bin = work_dir.join("prog");
                // Single "compile" step is just nasm; we tack on the ld step
                // as the run argv by chaining sh -c. The driver below treats
                // run_cmd_path as a single execvp call, so use a shell.
                (
                    "main.asm",
                    Some((
                        cc,
                        vec![
                            "-f".into(),
                            "elf64".into(),
                            src.to_string_lossy().into_owned(),
                            "-o".into(),
                            obj.to_string_lossy().into_owned(),
                        ],
                    )),
                    vec![
                        "sh".into(),
                        "-c".into(),
                        format!(
                            "{} {} -o {} && {}",
                            ld,
                            obj.to_string_lossy(),
                            bin.to_string_lossy(),
                            bin.to_string_lossy()
                        ),
                    ],
                )
            }
            "gas" => {
                // GNU assembler + ld. Same shape as nasm.
                let cc = match first_available(&["as"]) {
                    Some(c) => c,
                    None => {
                        let stderr_esc = json_escape_string(
                            "polyglot_exec_compiled: as (GNU assembler) not found in PATH",
                        );
                        return Ok(alloc_string(format!(
                        "{{\"success\":false,\"stdout\":\"\",\"stderr\":\"{stderr_esc}\",\"exit_code\":-1}}"
                    )));
                    }
                };
                let ld = first_available(&["ld"]).unwrap_or_else(|| "ld".to_string());
                let src = work_dir.join("main.s");
                let obj = work_dir.join("main.o");
                let bin = work_dir.join("prog");
                (
                    "main.s",
                    Some((
                        cc,
                        vec![
                            src.to_string_lossy().into_owned(),
                            "-o".into(),
                            obj.to_string_lossy().into_owned(),
                        ],
                    )),
                    vec![
                        "sh".into(),
                        "-c".into(),
                        format!(
                            "{} {} -o {} && {}",
                            ld,
                            obj.to_string_lossy(),
                            bin.to_string_lossy(),
                            bin.to_string_lossy()
                        ),
                    ],
                )
            }
            "v" => {
                let cc = match first_available(&["v"]) {
                    Some(c) => c,
                    None => {
                        let stderr_esc =
                            json_escape_string("polyglot_exec_compiled: v not found in PATH");
                        return Ok(alloc_string(format!(
                        "{{\"success\":false,\"stdout\":\"\",\"stderr\":\"{stderr_esc}\",\"exit_code\":-1}}"
                    )));
                    }
                };
                let src = work_dir.join("main.v");
                (
                    "main.v",
                    None,
                    vec![cc, "run".into(), src.to_string_lossy().into_owned()],
                )
            }
            "ocaml-compiled" => {
                // Bytecode compile + run via ocamlfind/ocamlc, since the
                // plain `ocaml` toplevel is interpreter-style.
                let cc = match first_available(&["ocamlfind", "ocamlc"]) {
                    Some(c) => c,
                    None => {
                        let stderr_esc = json_escape_string(
                            "polyglot_exec_compiled: ocamlfind/ocamlc not found in PATH",
                        );
                        return Ok(alloc_string(format!(
                        "{{\"success\":false,\"stdout\":\"\",\"stderr\":\"{stderr_esc}\",\"exit_code\":-1}}"
                    )));
                    }
                };
                let src = work_dir.join("main.ml");
                let bin = work_dir.join(if cfg!(windows) { "prog.exe" } else { "prog" });
                (
                    "main.ml",
                    Some((
                        cc,
                        vec![
                            src.to_string_lossy().into_owned(),
                            "-o".into(),
                            bin.to_string_lossy().into_owned(),
                        ],
                    )),
                    vec![bin.to_string_lossy().into_owned()],
                )
            }
            "csharp" => {
                // C# via the .NET SDK: scaffold `Program.cs` + a minimal
                // `prog.csproj`, then `dotnet run` which builds & runs.
                // Skips the explicit compile_cmd step — `dotnet run` does
                // both. The csproj is dropped after the standard source
                // write below (see special-case for lang_norm == "csharp").
                let dotnet = match first_available(&["dotnet"]) {
                    Some(c) => c,
                    None => {
                        let stderr_esc = json_escape_string(
                            "polyglot_exec_compiled: dotnet (the .NET SDK) not found in PATH",
                        );
                        return Ok(alloc_string(format!(
                        "{{\"success\":false,\"stdout\":\"\",\"stderr\":\"{stderr_esc}\",\"exit_code\":-1}}"
                    )));
                    }
                };
                (
                    "Program.cs",
                    None,
                    vec![
                        dotnet,
                        "run".into(),
                        "--project".into(),
                        work_dir.to_string_lossy().into_owned(),
                        "-c".into(),
                        "Release".into(),
                        "--verbosity".into(),
                        "quiet".into(),
                        "--nologo".into(),
                    ],
                )
            }
            "fsharp" => {
                // F# via dotnet fsi for single-file scripts.
                let fsi = match first_available(&["dotnet"]) {
                    Some(c) => c,
                    None => {
                        let stderr_esc = json_escape_string(
                            "polyglot_exec_compiled: dotnet (the .NET SDK) not found in PATH",
                        );
                        return Ok(alloc_string(format!(
                        "{{\"success\":false,\"stdout\":\"\",\"stderr\":\"{stderr_esc}\",\"exit_code\":-1}}"
                    )));
                    }
                };
                let src = work_dir.join("main.fsx");
                (
                    "main.fsx",
                    None,
                    vec![fsi, "fsi".into(), src.to_string_lossy().into_owned()],
                )
            }
            "vb" | "vbnet" | "visualbasic" => {
                // Visual Basic .NET — same csproj-scaffold trick as C#.
                let dotnet = match first_available(&["dotnet"]) {
                    Some(c) => c,
                    None => {
                        let stderr_esc = json_escape_string(
                            "polyglot_exec_compiled: dotnet (the .NET SDK) not found in PATH",
                        );
                        return Ok(alloc_string(format!(
                        "{{\"success\":false,\"stdout\":\"\",\"stderr\":\"{stderr_esc}\",\"exit_code\":-1}}"
                    )));
                    }
                };
                (
                    "Program.vb",
                    None,
                    vec![
                        dotnet,
                        "run".into(),
                        "--project".into(),
                        work_dir.to_string_lossy().into_owned(),
                        "-c".into(),
                        "Release".into(),
                        "--verbosity".into(),
                        "quiet".into(),
                        "--nologo".into(),
                    ],
                )
            }
            // ── Q# compiled (via dotnet) ───────────────────────────
            "qsharp-compiled" | "qs-compiled" => {
                let dotnet = match first_available(&["dotnet"]) {
                    Some(c) => c,
                    None => {
                        let stderr_esc = json_escape_string("polyglot_exec_compiled: dotnet (the .NET SDK) not found in PATH for Q#");
                        return Ok(alloc_string(format!("{{\"success\":false,\"stdout\":\"\",\"stderr\":\"{stderr_esc}\",\"exit_code\":-1}}")));
                    }
                };
                (
                    "Program.qs",
                    None,
                    vec![
                        dotnet,
                        "run".into(),
                        "--project".into(),
                        work_dir.to_string_lossy().into_owned(),
                    ],
                )
            }
            // ── Haxe ───────────────────────────────────────────────
            "haxe" | "hx" => {
                let cc = match first_available(&["haxe"]) {
                    Some(c) => c,
                    None => {
                        let stderr_esc =
                            json_escape_string("polyglot_exec_compiled: haxe not found in PATH");
                        return Ok(alloc_string(format!("{{\"success\":false,\"stdout\":\"\",\"stderr\":\"{stderr_esc}\",\"exit_code\":-1}}")));
                    }
                };
                let _src = work_dir.join("Main.hx");
                (
                    "Main.hx",
                    None,
                    vec![
                        cc,
                        "--run".into(),
                        "Main".into(),
                        "--cwd".into(),
                        work_dir.to_string_lossy().into_owned(),
                    ],
                )
            }
            // ── Pony ───────────────────────────────────────────────
            "pony" => {
                let cc = match first_available(&["ponyc"]) {
                    Some(c) => c,
                    None => {
                        let stderr_esc =
                            json_escape_string("polyglot_exec_compiled: ponyc not found in PATH");
                        return Ok(alloc_string(format!("{{\"success\":false,\"stdout\":\"\",\"stderr\":\"{stderr_esc}\",\"exit_code\":-1}}")));
                    }
                };
                let bin = work_dir.join("prog");
                let src = work_dir.join("main.pony");
                (
                    "main.pony",
                    Some((
                        cc,
                        vec![
                            src.to_string_lossy().into_owned(),
                            "-o".into(),
                            bin.to_string_lossy().into_owned(),
                        ],
                    )),
                    vec![bin.to_string_lossy().into_owned()],
                )
            }
            // ── Red ────────────────────────────────────────────────
            "red" => {
                let cc = match first_available(&["red"]) {
                    Some(c) => c,
                    None => {
                        let stderr_esc =
                            json_escape_string("polyglot_exec_compiled: red not found in PATH");
                        return Ok(alloc_string(format!("{{\"success\":false,\"stdout\":\"\",\"stderr\":\"{stderr_esc}\",\"exit_code\":-1}}")));
                    }
                };
                let src = work_dir.join("main.red");
                (
                    "main.red",
                    None,
                    vec![cc, src.to_string_lossy().into_owned()],
                )
            }
            // ── Verilog ────────────────────────────────────────────
            "verilog" | "vlog" => {
                let cc = match first_available(&["iverilog"]) {
                    Some(c) => c,
                    None => {
                        let stderr_esc = json_escape_string(
                            "polyglot_exec_compiled: iverilog not found in PATH",
                        );
                        return Ok(alloc_string(format!("{{\"success\":false,\"stdout\":\"\",\"stderr\":\"{stderr_esc}\",\"exit_code\":-1}}")));
                    }
                };
                let vvp = first_available(&["vvp"]).unwrap_or_else(|| "vvp".to_string());
                let src = work_dir.join("main.v");
                let bin = work_dir.join("prog.vvp");
                (
                    "main.v",
                    Some((
                        cc,
                        vec![
                            src.to_string_lossy().into_owned(),
                            "-o".into(),
                            bin.to_string_lossy().into_owned(),
                        ],
                    )),
                    vec![vvp, bin.to_string_lossy().into_owned()],
                )
            }
            // ── Mercury ────────────────────────────────────────────
            "mercury" => {
                let cc = match first_available(&["mmc"]) {
                    Some(c) => c,
                    None => {
                        let stderr_esc = json_escape_string(
                            "polyglot_exec_compiled: mmc (Mercury) not found in PATH",
                        );
                        return Ok(alloc_string(format!("{{\"success\":false,\"stdout\":\"\",\"stderr\":\"{stderr_esc}\",\"exit_code\":-1}}")));
                    }
                };
                let src = work_dir.join("main.m");
                let bin = work_dir.join("prog");
                (
                    "main.m",
                    Some((
                        cc,
                        vec![
                            "--make".into(),
                            src.to_string_lossy().into_owned(),
                            "-o".into(),
                            bin.to_string_lossy().into_owned(),
                        ],
                    )),
                    vec![bin.to_string_lossy().into_owned()],
                )
            }
            // ── ReasonML / BuckleScript ────────────────────────────
            "reason" | "re" => {
                let cc = match first_available(&["bsb"]) {
                    Some(c) => c,
                    None => {
                        let stderr_esc = json_escape_string(
                            "polyglot_exec_compiled: bsb (BuckleScript) not found in PATH",
                        );
                        return Ok(alloc_string(format!("{{\"success\":false,\"stdout\":\"\",\"stderr\":\"{stderr_esc}\",\"exit_code\":-1}}")));
                    }
                };
                let node = first_available(&["node"]).unwrap_or_else(|| "node".to_string());
                let _src = work_dir.join("Main.re");
                (
                    "Main.re",
                    Some((cc, vec!["-make-world".into()])),
                    vec![node, "src/Main.bs.js".into()],
                )
            }
            // ── Odin ────────────────────────────────────────────────
            "odin" => {
                let cc = match first_available(&["odin"]) {
                    Some(c) => c,
                    None => {
                        let stderr_esc =
                            json_escape_string("polyglot_exec_compiled: odin not found in PATH");
                        return Ok(alloc_string(format!("{{\"success\":false,\"stdout\":\"\",\"stderr\":\"{stderr_esc}\",\"exit_code\":-1}}")));
                    }
                };
                let src = work_dir.join("main.odin");
                (
                    "main.odin",
                    None,
                    vec![cc, "run".into(), src.to_string_lossy().into_owned()],
                )
            }
            // ── Carbon ──────────────────────────────────────────────
            "carbon" => {
                let cc = match first_available(&["carbon"]) {
                    Some(c) => c,
                    None => {
                        let stderr_esc =
                            json_escape_string("polyglot_exec_compiled: carbon not found in PATH");
                        return Ok(alloc_string(format!("{{\"success\":false,\"stdout\":\"\",\"stderr\":\"{stderr_esc}\",\"exit_code\":-1}}")));
                    }
                };
                let src = work_dir.join("main.carbon");
                (
                    "main.carbon",
                    None,
                    vec![cc, "run".into(), src.to_string_lossy().into_owned()],
                )
            }
            // ── Mojo ───────────────────────────────────────────────
            "mojo" => {
                let cc = match first_available(&["mojo"]) {
                    Some(c) => c,
                    None => {
                        let stderr_esc =
                            json_escape_string("polyglot_exec_compiled: mojo not found in PATH");
                        return Ok(alloc_string(format!("{{\"success\":false,\"stdout\":\"\",\"stderr\":\"{stderr_esc}\",\"exit_code\":-1}}")));
                    }
                };
                let src = work_dir.join("main.mojo");
                (
                    "main.mojo",
                    None,
                    vec![cc, src.to_string_lossy().into_owned()],
                )
            }
            // ── Chapel ──────────────────────────────────────────────
            "chapel" | "chpl" => {
                let cc = match first_available(&["chpl"]) {
                    Some(c) => c,
                    None => {
                        let stderr_esc = json_escape_string(
                            "polyglot_exec_compiled: chpl (Chapel) not found in PATH",
                        );
                        return Ok(alloc_string(format!("{{\"success\":false,\"stdout\":\"\",\"stderr\":\"{stderr_esc}\",\"exit_code\":-1}}")));
                    }
                };
                let bin = work_dir.join(if cfg!(windows) { "prog.exe" } else { "prog" });
                let src = work_dir.join("main.chpl");
                (
                    "main.chpl",
                    Some((
                        cc,
                        vec![
                            src.to_string_lossy().into_owned(),
                            "-o".into(),
                            bin.to_string_lossy().into_owned(),
                        ],
                    )),
                    vec![bin.to_string_lossy().into_owned()],
                )
            }
            // ── Factor ──────────────────────────────────────────────
            "factor" => {
                let cc = match first_available(&["factor"]) {
                    Some(c) => c,
                    None => {
                        let stderr_esc =
                            json_escape_string("polyglot_exec_compiled: factor not found in PATH");
                        return Ok(alloc_string(format!("{{\"success\":false,\"stdout\":\"\",\"stderr\":\"{stderr_esc}\",\"exit_code\":-1}}")));
                    }
                };
                let src = work_dir.join("main.factor");
                (
                    "main.factor",
                    None,
                    vec![cc, src.to_string_lossy().into_owned()],
                )
            }
            // ── PureScript ──────────────────────────────────────────
            "purescript" | "purs" => {
                let cc = match first_available(&["purs"]) {
                    Some(c) => c,
                    None => {
                        let stderr_esc = json_escape_string(
                            "polyglot_exec_compiled: purs (PureScript) not found in PATH",
                        );
                        return Ok(alloc_string(format!("{{\"success\":false,\"stdout\":\"\",\"stderr\":\"{stderr_esc}\",\"exit_code\":-1}}")));
                    }
                };
                let node = first_available(&["node"]).unwrap_or_else(|| "node".to_string());
                let src = work_dir.join("Main.purs");
                (
                    "Main.purs",
                    Some((
                        cc,
                        vec![
                            "compile".into(),
                            src.to_string_lossy().into_owned(),
                            "-o".into(),
                            work_dir.to_string_lossy().into_owned(),
                        ],
                    )),
                    vec![
                        node,
                        "-e".into(),
                        "require(047./output/Main/index.js047).main()".into(),
                    ],
                )
            }
            // ── Solidity (compiled) ─────────────────────────────────
            "solidity" | "sol" => {
                let cc = match first_available(&["solc"]) {
                    Some(c) => c,
                    None => {
                        let stderr_esc = json_escape_string(
                            "polyglot_exec_compiled: solc (Solidity) not found in PATH",
                        );
                        return Ok(alloc_string(format!("{{\"success\":false,\"stdout\":\"\",\"stderr\":\"{stderr_esc}\",\"exit_code\":-1}}")));
                    }
                };
                let src = work_dir.join("main.sol");
                let bin = work_dir.join("main.bin");
                (
                    "main.sol",
                    Some((
                        cc.clone(),
                        vec![
                            src.to_string_lossy().into_owned(),
                            "--bin".into(),
                            "-o".into(),
                            work_dir.to_string_lossy().into_owned(),
                        ],
                    )),
                    vec!["cat".into(), bin.to_string_lossy().into_owned()],
                )
            }
            other => {
                return Err(host.err(format!(
                "polyglot_exec_compiled(): unsupported language '{}'. Supported: rust, go, c, cpp, zig, java, typescript, pascal, kotlin, nim, crystal, d, haskell, fortran, ada, swift, cobol, erlang, vala, objc, nasm, gas, v, ocaml-compiled, csharp, fsharp, vb, qsharp-compiled, haxe, pony, red, verilog, mercury, reason, odin, carbon, mojo, chapel, factor, purescript, solidity",
                other
            )));
            }
        };

    // Write source.
    let src_path = work_dir.join(src_name);
    if let Err(e) = std::fs::write(&src_path, code) {
        let stderr_esc = json_escape_string(&format!("polyglot_exec_compiled: write source: {e}"));
        return Ok(alloc_string(format!(
            "{{\"success\":false,\"stdout\":\"\",\"stderr\":\"{stderr_esc}\",\"exit_code\":-1}}"
        )));
    }

    // Languages that need a sibling project manifest get it dropped
    // alongside the source here. Kept out of the main scaffolding
    // tuple to avoid widening that contract for one-off cases.
    match lang_norm {
        "csharp" => {
            let csproj = "<Project Sdk=\"Microsoft.NET.Sdk\">\n  <PropertyGroup>\n    <OutputType>Exe</OutputType>\n    <TargetFramework>net8.0</TargetFramework>\n    <RootNamespace>PblangPolyglot</RootNamespace>\n    <Nullable>enable</Nullable>\n    <ImplicitUsings>enable</ImplicitUsings>\n  </PropertyGroup>\n</Project>\n";
            if let Err(e) = std::fs::write(work_dir.join("prog.csproj"), csproj) {
                let stderr_esc = json_escape_string(&format!(
                    "polyglot_exec_compiled(csharp): write csproj: {e}"
                ));
                return Ok(alloc_string(format!(
                    "{{\"success\":false,\"stdout\":\"\",\"stderr\":\"{stderr_esc}\",\"exit_code\":-1}}"
                )));
            }
        }
        "vb" => {
            let vbproj = "<Project Sdk=\"Microsoft.NET.Sdk\">\n  <PropertyGroup>\n    <OutputType>Exe</OutputType>\n    <TargetFramework>net8.0</TargetFramework>\n    <RootNamespace>PblangPolyglot</RootNamespace>\n  </PropertyGroup>\n</Project>\n";
            if let Err(e) = std::fs::write(work_dir.join("prog.vbproj"), vbproj) {
                let stderr_esc =
                    json_escape_string(&format!("polyglot_exec_compiled(vb): write vbproj: {e}"));
                return Ok(alloc_string(format!(
                    "{{\"success\":false,\"stdout\":\"\",\"stderr\":\"{stderr_esc}\",\"exit_code\":-1}}"
                )));
            }
        }
        _ => {}
    }

    // Compile (if any).
    if let Some((compiler, comp_args)) = compile_cmd {
        let compile_output = std::process::Command::new(&compiler)
            .args(&comp_args)
            .current_dir(&work_dir)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .output();
        let compile_output = match compile_output {
            Ok(o) => o,
            Err(e) => {
                let stderr_esc =
                    json_escape_string(&format!("polyglot_exec_compiled: spawn {compiler}: {e}"));
                return Ok(alloc_string(format!(
                    "{{\"success\":false,\"stdout\":\"\",\"stderr\":\"{stderr_esc}\",\"exit_code\":-1}}"
                )));
            }
        };
        if !compile_output.status.success() {
            let stderr_text = String::from_utf8_lossy(&compile_output.stderr).to_string();
            let stdout_text = String::from_utf8_lossy(&compile_output.stdout).to_string();
            let json = format!(
                "{{\"success\":false,\"stdout\":\"{}\",\"stderr\":\"{}\",\"exit_code\":{}}}",
                json_escape_string(&stdout_text),
                json_escape_string(&format!("[compile error]\n{stderr_text}")),
                compile_output.status.code().unwrap_or(-1)
            );
            return Ok(alloc_string(json));
        }
    }

    // Run.
    let (run_prog, run_rest) = run_cmd_path
        .split_first()
        .map(|(a, b)| (a.clone(), b.to_vec()))
        .unwrap_or_else(|| ("true".to_string(), vec![]));
    let run_output = std::process::Command::new(&run_prog)
        .args(&run_rest)
        .current_dir(&work_dir)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output();
    let run_output = match run_output {
        Ok(o) => o,
        Err(e) => {
            let stderr_esc =
                json_escape_string(&format!("polyglot_exec_compiled: spawn {run_prog}: {e}"));
            return Ok(alloc_string(format!(
                "{{\"success\":false,\"stdout\":\"\",\"stderr\":\"{stderr_esc}\",\"exit_code\":-1}}"
            )));
        }
    };

    let stdout = String::from_utf8_lossy(&run_output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&run_output.stderr).to_string();
    let exit_code = run_output.status.code().unwrap_or(-1);
    let success = run_output.status.success();
    let json = format!(
        "{{\"success\":{success},\"stdout\":\"{}\",\"stderr\":\"{}\",\"exit_code\":{exit_code}}}",
        json_escape_string(&stdout),
        json_escape_string(&stderr)
    );
    Ok(alloc_string(json))
}

/// polyglot_detect() — probe which polyglot toolchains are available
/// on the host PATH. Returns a JSON object mapping language name to
/// `{ available: bool, kind: "interpreter"|"compiler", command: "..." }`.
/// Useful for examples/docs to skip unavailable languages gracefully
/// instead of failing on missing toolchains.
pub(super) fn polyglot_detect(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    if !host.allow_exec() {
        return Err(vm_cap!(host, "polyglot_detect", "exec"));
    }
    if !args.is_empty() {
        return Err(host.err(format!(
            "polyglot_detect() expects 0 arguments, got {}",
            args.len()
        )));
    }
    fn probe(cands: &[&str]) -> Option<String> {
        for c in cands {
            let ok = std::process::Command::new(c)
                .arg("--version")
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .map(|s| s.success())
                .unwrap_or(false);
            if ok {
                return Some((*c).to_string());
            }
        }
        None
    }
    let langs: &[(&str, &str, &[&str])] = &[
        // Interpreted / scripting
        ("python", "interpreter", &["python3", "python"]),
        ("node", "interpreter", &["node"]),
        ("deno", "interpreter", &["deno"]),
        ("bash", "interpreter", &["bash"]),
        ("zsh", "interpreter", &["zsh"]),
        ("fish", "interpreter", &["fish"]),
        ("powershell", "interpreter", &["pwsh"]),
        ("ruby", "interpreter", &["ruby"]),
        ("perl", "interpreter", &["perl"]),
        ("raku", "interpreter", &["raku"]),
        ("lua", "interpreter", &["lua"]),
        ("php", "interpreter", &["php"]),
        ("r", "interpreter", &["Rscript"]),
        ("tcl", "interpreter", &["tclsh"]),
        ("awk", "interpreter", &["awk"]),
        ("julia", "interpreter", &["julia"]),
        ("racket", "interpreter", &["racket"]),
        ("scheme", "interpreter", &["guile"]),
        ("clojure", "interpreter", &["clojure"]),
        ("elixir", "interpreter", &["elixir"]),
        ("groovy", "interpreter", &["groovy"]),
        ("fennel", "interpreter", &["fennel"]),
        ("ocaml", "interpreter", &["ocaml"]),
        ("scala", "interpreter", &["scala"]),
        // Compiled / native
        ("rust", "compiler", &["rustc"]),
        ("go", "compiler", &["go"]),
        ("c", "compiler", &["cc", "gcc", "clang"]),
        ("cpp", "compiler", &["c++", "g++", "clang++"]),
        ("zig", "compiler", &["zig"]),
        ("java", "compiler", &["javac"]),
        ("kotlin", "compiler", &["kotlinc"]),
        ("typescript", "compiler", &["tsc"]),
        ("pascal", "compiler", &["fpc"]),
        ("nim", "compiler", &["nim"]),
        ("crystal", "compiler", &["crystal"]),
        ("d", "compiler", &["dmd", "ldc2", "gdc"]),
        ("haskell", "compiler", &["runghc", "ghc"]),
        ("fortran", "compiler", &["gfortran", "flang"]),
        ("ada", "compiler", &["gnatmake"]),
        ("swift", "compiler", &["swift"]),
        ("cobol", "compiler", &["cobc"]),
        ("erlang", "compiler", &["escript"]),
        ("vala", "compiler", &["valac"]),
        ("objc", "compiler", &["clang", "gcc"]),
        ("nasm", "compiler", &["nasm"]),
        ("gas", "compiler", &["as"]),
        ("v", "compiler", &["v"]),
        ("ocaml-compiled", "compiler", &["ocamlfind", "ocamlc"]),
        ("csharp", "compiler", &["dotnet"]),
        ("fsharp", "compiler", &["dotnet"]),
        ("vb", "compiler", &["dotnet"]),
        ("dotnet-script", "interpreter", &["dotnet-script"]),
        ("native-image", "compiler", &["native-image"]),
        ("kotlin-native", "compiler", &["kotlinc-native"]),
        // ── New interpreted languages ────────────────────────────
        ("qsharp", "interpreter", &["python3"]),
        ("sqlite", "interpreter", &["sqlite3"]),
        ("psql", "interpreter", &["psql"]),
        ("mysql", "interpreter", &["mysql"]),
        ("mongosh", "interpreter", &["mongosh"]),
        ("redis-cli", "interpreter", &["redis-cli"]),
        ("octave", "interpreter", &["octave"]),
        ("prolog", "interpreter", &["swipl"]),
        ("smalltalk", "interpreter", &["gst"]),
        ("forth", "interpreter", &["gforth"]),
        ("idris", "interpreter", &["idris2"]),
        ("sass", "interpreter", &["sass"]),
        ("lessc", "interpreter", &["lessc"]),
        ("bc", "interpreter", &["bc"]),
        ("dc", "interpreter", &["dc"]),
        ("gnuplot", "interpreter", &["gnuplot"]),
        ("io", "interpreter", &["io"]),
        // ── New compiled languages ──────────────────────────────
        ("qsharp-compiled", "compiler", &["dotnet"]),
        ("haxe", "compiler", &["haxe"]),
        ("pony", "compiler", &["ponyc"]),
        ("red", "compiler", &["red"]),
        ("verilog", "compiler", &["iverilog"]),
        ("mercury", "compiler", &["mmc"]),
        ("reason", "compiler", &["bsb"]),
        // ── New interpreted languages ───────────────────────────
        ("duckdb", "interpreter", &["duckdb"]),
        ("cassandra", "interpreter", &["cqlsh"]),
        ("neo4j", "interpreter", &["cypher-shell"]),
        ("clickhouse", "interpreter", &["clickhouse-client"]),
        ("lisp", "interpreter", &["sbcl"]),
        ("janet", "interpreter", &["janet"]),
        ("maxima", "interpreter", &["maxima"]),
        ("gap", "interpreter", &["gap"]),
        ("pari", "interpreter", &["gp"]),
        // ── New compiled languages ─────────────────────────────
        ("odin", "compiler", &["odin"]),
        ("carbon", "compiler", &["carbon"]),
        ("mojo", "compiler", &["mojo"]),
        ("chapel", "compiler", &["chpl"]),
        ("factor", "compiler", &["factor"]),
        ("purescript", "compiler", &["purs"]),
        ("solidity", "compiler", &["solc"]),
    ];
    let mut json = String::from("{");
    let mut first = true;
    for (name, kind, cands) in langs {
        if !first {
            json.push(',');
        }
        first = false;
        let (avail, cmd) = match probe(cands) {
            Some(c) => (true, c),
            None => (false, String::new()),
        };
        json.push_str(&format!(
            "\"{}\":{{\"available\":{},\"kind\":\"{}\",\"command\":\"{}\"}}",
            name,
            avail,
            kind,
            json_escape_string(&cmd)
        ));
    }
    json.push('}');
    Ok(alloc_string(json))
}

// ── Cached cdylib loader ────────────────────────────────────────────
//
// `polyglot_compile_cached(lang, code)` is the build-time complement to
// `polyglot_exec_compiled`: instead of spawning a subprocess each time
// a foreign block runs, it compiles the block into a shared library
// (cdylib / .so / .dll / .dylib) **once**, caches the artifact
// content-hashed under `~/.cache/pblang/polyglot/`, and returns a
// libloading handle that the existing `ffi_call_typed(handle, sym,
// sig, args)` machinery can call directly.
//
// The user's snippet is responsible for exporting symbols with C ABI
// (`#[no_mangle] extern "C"` in Rust, plain function in C/C++, `//
// export Foo` in Go, etc.) — we don't try to auto-generate wrappers.
//
// Supported languages (all produce cdylib via the host toolchain):
//   rust  → rustc --crate-type=cdylib --edition 2021 -O
//   c     → cc/gcc/clang -shared -fPIC -O2
//   cpp   → c++/g++/clang++ -shared -fPIC -O2 -std=c++17
//   go    → go build -buildmode=c-shared (requires CGO_ENABLED=1)
//   pascal → fpc with shared-library directive
//
// Cache key = sha256(lang_norm || "\x00" || code).hex.
// Cache invalidation: caller calls polyglot_cache_clear().

pub(super) fn polyglot_cache_dir() -> std::path::PathBuf {
    // XDG_CACHE_HOME → ~/.cache → tempdir fallback.
    if let Ok(p) = std::env::var("PBLANG_CACHE_DIR") {
        return std::path::PathBuf::from(p).join("polyglot");
    }
    if let Ok(p) = std::env::var("XDG_CACHE_HOME") {
        return std::path::PathBuf::from(p).join("pblang").join("polyglot");
    }
    if let Ok(home) = std::env::var("HOME") {
        return std::path::PathBuf::from(home)
            .join(".cache")
            .join("pblang")
            .join("polyglot");
    }
    std::env::temp_dir().join("pblang_polyglot_cache")
}

pub(super) fn polyglot_cache_key(lang: &str, code: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(lang.as_bytes());
    hasher.update([0u8]);
    hasher.update(code.as_bytes());
    // Version tag — bump this if the compile flags change so old
    // cache entries get bypassed instead of being reused incorrectly.
    hasher.update(b"\x00v1");
    let digest = hasher.finalize();
    let mut s = String::with_capacity(64);
    for b in digest {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

pub(super) fn cdylib_ext() -> &'static str {
    if cfg!(target_os = "windows") {
        "dll"
    } else if cfg!(target_os = "macos") {
        "dylib"
    } else {
        "so"
    }
}

pub(super) fn polyglot_compile_cached(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    if !host.allow_exec() {
        return Err(vm_cap!(host, "polyglot_compile_cached", "exec"));
    }
    if !host.allow_ffi() {
        return Err(host.err(
            "polyglot_compile_cached(): ffi access denied (polyglot_compile_cached returns an FFI handle)".into(),
        ));
    }
    if args.len() != 2 {
        return Err(host.err(format!(
            "polyglot_compile_cached() expects 2 arguments (lang, code), got {}",
            args.len()
        )));
    }
    let lang = args[0]
        .as_str()
        .map_err(|m| host.err(format!("polyglot_compile_cached(): {m}")))?;
    let code = args[1]
        .as_str()
        .map_err(|m| host.err(format!("polyglot_compile_cached(): {m}")))?;

    let lang_lc = lang.to_lowercase();
    let lang_norm = match lang_lc.as_str() {
        "rs" => "rust",
        "cxx" | "c++" => "cpp",
        "pas" | "delphi" | "freepascal" => "pascal",
        "cs" | "c#" => "csharp",
        "fs" | "fsx" | "f#" => "fsharp",
        "hs" => "haskell",
        "f90" | "f95" | "f03" | "f08" | "f" => "fortran",
        "m" | "objective-c" => "objc",
        "ml" => "ocaml",
        "kt" => "kotlin",
        other => other,
    };
    match lang_norm {
        "rust" | "c" | "cpp" | "go" | "pascal" | "csharp" | "fsharp" | "zig" | "d" | "nim"
        | "swift" | "haskell" | "fortran" | "objc" | "ocaml" | "java" | "kotlin" => {}
        _ => {
            return Err(host.err(format!(
                "polyglot_compile_cached(): unsupported language '{}'. Supported: rust, c, cpp, go, pascal, csharp, fsharp, zig, d, nim, swift, haskell, fortran, objc, ocaml, java, kotlin (must produce cdylib)",
                lang
            )));
        }
    }

    // .NET RID for NativeAOT publish path (csharp/fsharp).
    #[allow(dead_code)]
    fn dotnet_rid() -> &'static str {
        #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
        {
            return "linux-x64";
        }
        #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
        {
            return "linux-arm64";
        }
        #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
        {
            return "osx-x64";
        }
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        {
            return "osx-arm64";
        }
        #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
        {
            return "win-x64";
        }
        #[cfg(all(target_os = "windows", target_arch = "aarch64"))]
        {
            return "win-arm64";
        }
        #[allow(unreachable_code)]
        {
            "linux-x64"
        }
    }

    let key = polyglot_cache_key(lang_norm, code);
    let cache_dir = polyglot_cache_dir();
    if let Err(e) = std::fs::create_dir_all(&cache_dir) {
        return Err(host.err(format!(
            "polyglot_compile_cached(): cannot create cache dir {}: {e}",
            cache_dir.display()
        )));
    }
    let lib_name = format!("pblang_polyglot_{key}.{ext}", ext = cdylib_ext());
    let cached_path = cache_dir.join(&lib_name);

    // Cache hit → load and return.
    if cached_path.is_file() {
        if !host.ffi_allowed_library(&cached_path.to_string_lossy()) {
            return Err(host.err(format!(
                "polyglot_compile_cached(): cached library {} not in FFI allowlist; set FfiPolicy.allow_all=true or add the cache dir to allowed_libs",
                cached_path.display()
            )));
        }
        // SAFETY: cached_path is a content-hashed file written by us
        // in polyglot_cache_dir(); it is `.is_file()` per the check
        // immediately above. The library is opened with `dlopen` (via
        // libloading); any constructor/initializer it runs executes in
        // the current process — that's the documented contract of the
        // cdylib cache, gated by `host.allow_ffi()` + the FFI allowlist
        // check above. We hold the Library by RAII so unloading is
        // explicit and serial.
        let lib = unsafe { Library::new(&cached_path) }
            .map_err(|e| host.err(format!("polyglot_compile_cached(): {e}")))?;
        let id = with_ffi_table(host, |t| {
            let id = FFI_NEXT.fetch_add(1, Ordering::Relaxed);
            t.insert(id, FfiResource::Library(lib));
            id
        })?;
        return Ok(Value::Number(id as f64));
    }

    // Cache miss → compile into a per-invocation work dir, then move
    // the built artifact into the cache.
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let work_dir = std::env::temp_dir().join(format!("pblang_polyglot_build_{lang_norm}_{nonce}"));
    if let Err(e) = std::fs::create_dir_all(&work_dir) {
        return Err(host.err(format!(
            "polyglot_compile_cached(): cannot create work dir: {e}"
        )));
    }
    struct DirCleanup(std::path::PathBuf);
    impl Drop for DirCleanup {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let _cleanup = DirCleanup(work_dir.clone());

    fn first_available_cmd(cands: &[&str]) -> Option<String> {
        for c in cands {
            let ok = std::process::Command::new(c)
                .arg("--version")
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .map(|s| s.success())
                .unwrap_or(false);
            if ok {
                return Some((*c).to_string());
            }
        }
        None
    }

    let staged_lib = work_dir.join(format!("staged.{ext}", ext = cdylib_ext()));
    let (compiler, comp_args, src_name) = match lang_norm {
        "rust" => {
            let cc = first_available_cmd(&["rustc"]).ok_or_else(|| {
                host.err("polyglot_compile_cached(rust): rustc not found in PATH".into())
            })?;
            let src = work_dir.join("src.rs");
            (
                cc,
                vec![
                    "--crate-type=cdylib".into(),
                    "--edition".into(),
                    "2021".into(),
                    "-O".into(),
                    src.to_string_lossy().into_owned(),
                    "-o".into(),
                    staged_lib.to_string_lossy().into_owned(),
                ],
                "src.rs",
            )
        }
        "c" => {
            let cc = first_available_cmd(&["cc", "gcc", "clang"]).ok_or_else(|| {
                host.err(
                    "polyglot_compile_cached(c): no C compiler found (tried cc, gcc, clang)".into(),
                )
            })?;
            let src = work_dir.join("src.c");
            (
                cc,
                vec![
                    "-shared".into(),
                    "-fPIC".into(),
                    "-O2".into(),
                    src.to_string_lossy().into_owned(),
                    "-o".into(),
                    staged_lib.to_string_lossy().into_owned(),
                ],
                "src.c",
            )
        }
        "cpp" => {
            let cc = first_available_cmd(&["c++", "g++", "clang++"]).ok_or_else(|| {
                host.err(
                    "polyglot_compile_cached(cpp): no C++ compiler found (tried c++, g++, clang++)"
                        .into(),
                )
            })?;
            let src = work_dir.join("src.cpp");
            (
                cc,
                vec![
                    "-shared".into(),
                    "-fPIC".into(),
                    "-O2".into(),
                    "-std=c++17".into(),
                    src.to_string_lossy().into_owned(),
                    "-o".into(),
                    staged_lib.to_string_lossy().into_owned(),
                ],
                "src.cpp",
            )
        }
        "go" => {
            let cc = first_available_cmd(&["go"]).ok_or_else(|| {
                host.err("polyglot_compile_cached(go): go not found in PATH".into())
            })?;
            let src = work_dir.join("src.go");
            (
                cc,
                vec![
                    "build".into(),
                    "-buildmode=c-shared".into(),
                    "-o".into(),
                    staged_lib.to_string_lossy().into_owned(),
                    src.to_string_lossy().into_owned(),
                ],
                "src.go",
            )
        }
        "pascal" => {
            let cc = first_available_cmd(&["fpc"]).ok_or_else(|| {
                host.err("polyglot_compile_cached(pascal): fpc not found in PATH".into())
            })?;
            // -CD enables shared library output (Position Independent Code).
            let src = work_dir.join("src.pas");
            (
                cc,
                vec![
                    "-CD".into(),
                    "-fPIC".into(),
                    format!("-o{}", staged_lib.to_string_lossy()),
                    src.to_string_lossy().into_owned(),
                ],
                "src.pas",
            )
        }
        // ── .NET NativeAOT family ────────────────────────────────────
        // C#/F# are special: `dotnet publish -p:PublishAot=true` writes
        // the .so/.dll/.dylib into bin/Release/<TFM>/<RID>/publish/.
        // After the build succeeds we copy from that path into
        // staged_lib (see the post-build special-case below). The
        // sibling .csproj/.fsproj is dropped after the standard source
        // write step.
        //
        // User's code MUST mark each exported function with
        // [UnmanagedCallersOnly(EntryPoint = "name")] so the symbol
        // appears in the dlsym table.
        "csharp" => {
            let dotnet = first_available_cmd(&["dotnet"]).ok_or_else(|| {
                host.err(
                    "polyglot_compile_cached(csharp): dotnet (the .NET SDK 8+) not found in PATH"
                        .into(),
                )
            })?;
            (
                dotnet,
                vec![
                    "publish".into(),
                    "-c".into(),
                    "Release".into(),
                    "-r".into(),
                    dotnet_rid().into(),
                    "--nologo".into(),
                    "--verbosity".into(),
                    "quiet".into(),
                    "-p:UseAppHost=false".into(),
                    work_dir.to_string_lossy().into_owned(),
                ],
                "Program.cs",
            )
        }
        "fsharp" => {
            let dotnet = first_available_cmd(&["dotnet"]).ok_or_else(|| {
                host.err(
                    "polyglot_compile_cached(fsharp): dotnet (the .NET SDK 8+) not found in PATH"
                        .into(),
                )
            })?;
            (
                dotnet,
                vec![
                    "publish".into(),
                    "-c".into(),
                    "Release".into(),
                    "-r".into(),
                    dotnet_rid().into(),
                    "--nologo".into(),
                    "--verbosity".into(),
                    "quiet".into(),
                    "-p:UseAppHost=false".into(),
                    work_dir.to_string_lossy().into_owned(),
                ],
                "Program.fs",
            )
        }
        // ── Other compiled languages with direct shared-lib output ───
        "zig" => {
            let cc = first_available_cmd(&["zig"]).ok_or_else(|| {
                host.err("polyglot_compile_cached(zig): zig not found in PATH".into())
            })?;
            let src = work_dir.join("src.zig");
            (
                cc,
                vec![
                    "build-lib".into(),
                    "-dynamic".into(),
                    "-O".into(),
                    "ReleaseFast".into(),
                    src.to_string_lossy().into_owned(),
                    format!("-femit-bin={}", staged_lib.to_string_lossy()),
                ],
                "src.zig",
            )
        }
        "d" => {
            let cc = first_available_cmd(&["dmd", "ldc2", "gdc"]).ok_or_else(|| {
                host.err(
                    "polyglot_compile_cached(d): no D compiler found (tried dmd, ldc2, gdc)".into(),
                )
            })?;
            let src = work_dir.join("src.d");
            (
                cc,
                vec![
                    "-shared".into(),
                    "-fPIC".into(),
                    format!("-of={}", staged_lib.to_string_lossy()),
                    src.to_string_lossy().into_owned(),
                ],
                "src.d",
            )
        }
        "nim" => {
            let cc = first_available_cmd(&["nim"]).ok_or_else(|| {
                host.err("polyglot_compile_cached(nim): nim not found in PATH".into())
            })?;
            let src = work_dir.join("src.nim");
            (
                cc,
                vec![
                    "c".into(),
                    "--app:lib".into(),
                    "--opt:speed".into(),
                    "--hints:off".into(),
                    "--noMain:on".into(),
                    format!("--out:{}", staged_lib.to_string_lossy()),
                    src.to_string_lossy().into_owned(),
                ],
                "src.nim",
            )
        }
        "swift" => {
            let cc = first_available_cmd(&["swiftc", "swift"]).ok_or_else(|| {
                host.err("polyglot_compile_cached(swift): swiftc/swift not found in PATH".into())
            })?;
            let src = work_dir.join("src.swift");
            (
                cc,
                vec![
                    "-emit-library".into(),
                    "-O".into(),
                    "-o".into(),
                    staged_lib.to_string_lossy().into_owned(),
                    src.to_string_lossy().into_owned(),
                ],
                "src.swift",
            )
        }
        "haskell" => {
            let cc = first_available_cmd(&["ghc"]).ok_or_else(|| {
                host.err("polyglot_compile_cached(haskell): ghc not found in PATH".into())
            })?;
            let src = work_dir.join("Src.hs");
            // -shared -dynamic -fPIC -lHSrts produces a callable .so;
            // user must use FFI export pragmas:
            //   foreign export ccall my_fn :: CInt -> CInt
            (
                cc,
                vec![
                    "-shared".into(),
                    "-dynamic".into(),
                    "-fPIC".into(),
                    "-O2".into(),
                    "-o".into(),
                    staged_lib.to_string_lossy().into_owned(),
                    src.to_string_lossy().into_owned(),
                    "-lHSrts-ghc".into(),
                ],
                "Src.hs",
            )
        }
        "fortran" => {
            let cc = first_available_cmd(&["gfortran", "flang"]).ok_or_else(|| {
                host.err(
                    "polyglot_compile_cached(fortran): no Fortran compiler found (tried gfortran, flang)"
                        .into(),
                )
            })?;
            let src = work_dir.join("src.f90");
            // bind(c, name="...") on the user's subroutine/function
            // makes the symbol C-ABI-callable.
            (
                cc,
                vec![
                    "-shared".into(),
                    "-fPIC".into(),
                    "-O2".into(),
                    "-o".into(),
                    staged_lib.to_string_lossy().into_owned(),
                    src.to_string_lossy().into_owned(),
                ],
                "src.f90",
            )
        }
        "objc" => {
            let cc = first_available_cmd(&["clang", "gcc"]).ok_or_else(|| {
                host.err(
                    "polyglot_compile_cached(objc): clang/gcc not found (Objective-C needs an ObjC-capable compiler)"
                        .into(),
                )
            })?;
            let src = work_dir.join("src.m");
            let mut comp_args = vec![
                "-shared".into(),
                "-fPIC".into(),
                "-O2".into(),
                "-o".into(),
                staged_lib.to_string_lossy().into_owned(),
                src.to_string_lossy().into_owned(),
            ];
            if cfg!(target_os = "linux") {
                // GNUstep / libobjc2 on Linux needs explicit linkage.
                comp_args.push("-lobjc".into());
            }
            (cc, comp_args, "src.m")
        }
        "ocaml" => {
            let cc = first_available_cmd(&["ocamlopt"]).ok_or_else(|| {
                host.err("polyglot_compile_cached(ocaml): ocamlopt not found in PATH".into())
            })?;
            let src = work_dir.join("src.ml");
            // -shared produces a position-independent shared library;
            // exports rely on the OCaml `external` FFI declarations.
            (
                cc,
                vec![
                    "-shared".into(),
                    "-O3".into(),
                    "-o".into(),
                    staged_lib.to_string_lossy().into_owned(),
                    src.to_string_lossy().into_owned(),
                ],
                "src.ml",
            )
        }
        "java" => {
            // Java cdylib via GraalVM native-image. Two-step build:
            // `javac Main.java` produces Main.class, then
            // `native-image --shared -H:Name=prog -cp <dir> Main`
            // produces prog.so/.dll/.dylib (with `lib`-prefix on Unix).
            // Wrapped in `sh -c` because two commands.
            //
            // User code must use GraalVM's `@CEntryPoint` annotation:
            //   import org.graalvm.nativeimage.IsolateThread;
            //   import org.graalvm.nativeimage.c.function.CEntryPoint;
            //   public class Main {
            //     @CEntryPoint(name = "pblang_add")
            //     static long add(IsolateThread t, long a, long b) { return a + b; }
            //   }
            let javac = first_available_cmd(&["javac"]).ok_or_else(|| {
                host.err("polyglot_compile_cached(java): javac not found in PATH".into())
            })?;
            let native_image = first_available_cmd(&["native-image"]).ok_or_else(|| {
                host.err(
                    "polyglot_compile_cached(java): native-image (GraalVM) not found in PATH. \
                     Install GraalVM and run `gu install native-image`."
                        .into(),
                )
            })?;
            // sh -c "javac src && native-image --shared ... -H:Name=prog Main"
            let src = work_dir.join("Main.java");
            let script = format!(
                "{javac} '{src}' && {ni} --shared --no-fallback -O3 \
                 -H:Name=prog -H:Path='{wd}' -cp '{wd}' Main",
                javac = javac,
                src = src.to_string_lossy(),
                ni = native_image,
                wd = work_dir.to_string_lossy(),
            );
            ("sh".to_string(), vec!["-c".into(), script], "Main.java")
        }
        "kotlin" => {
            // Kotlin/Native cdylib via `kotlinc-native -produce dynamic`.
            // User code marks exports with `@CName("name")`.
            let kc = first_available_cmd(&["kotlinc-native"]).ok_or_else(|| {
                host.err(
                    "polyglot_compile_cached(kotlin): kotlinc-native not found in PATH. \
                     Install Kotlin/Native (separate from kotlinc on the JVM)."
                        .into(),
                )
            })?;
            let src = work_dir.join("Main.kt");
            // Output naming is platform-dependent (lib-prefix on Unix);
            // the post-build fixup below renames into staged_lib.
            let out_base = work_dir.join("staged_kn");
            (
                kc,
                vec![
                    src.to_string_lossy().into_owned(),
                    "-produce".into(),
                    "dynamic".into(),
                    "-opt".into(),
                    "-o".into(),
                    out_base.to_string_lossy().into_owned(),
                ],
                "Main.kt",
            )
        }
        _ => unreachable!(),
    };

    let src_path = work_dir.join(src_name);
    if let Err(e) = std::fs::write(&src_path, code) {
        return Err(host.err(format!("polyglot_compile_cached(): write source: {e}")));
    }

    // Languages that need a sibling project manifest get it dropped
    // alongside the source here (kept out of the main scaffolding
    // tuple so the contract stays narrow).
    match lang_norm {
        "csharp" => {
            let csproj = "<Project Sdk=\"Microsoft.NET.Sdk\">\n  <PropertyGroup>\n    <OutputType>Library</OutputType>\n    <TargetFramework>net8.0</TargetFramework>\n    <PublishAot>true</PublishAot>\n    <NativeLib>Shared</NativeLib>\n    <RootNamespace>PblangPolyglot</RootNamespace>\n    <Nullable>enable</Nullable>\n    <ImplicitUsings>enable</ImplicitUsings>\n    <InvariantGlobalization>true</InvariantGlobalization>\n  </PropertyGroup>\n</Project>\n";
            if let Err(e) = std::fs::write(work_dir.join("prog.csproj"), csproj) {
                return Err(host.err(format!(
                    "polyglot_compile_cached(csharp): write csproj: {e}"
                )));
            }
        }
        "fsharp" => {
            let fsproj = "<Project Sdk=\"Microsoft.NET.Sdk\">\n  <PropertyGroup>\n    <OutputType>Library</OutputType>\n    <TargetFramework>net8.0</TargetFramework>\n    <PublishAot>true</PublishAot>\n    <NativeLib>Shared</NativeLib>\n    <RootNamespace>PblangPolyglot</RootNamespace>\n    <InvariantGlobalization>true</InvariantGlobalization>\n  </PropertyGroup>\n  <ItemGroup>\n    <Compile Include=\"Program.fs\" />\n  </ItemGroup>\n</Project>\n";
            if let Err(e) = std::fs::write(work_dir.join("prog.fsproj"), fsproj) {
                return Err(host.err(format!(
                    "polyglot_compile_cached(fsharp): write fsproj: {e}"
                )));
            }
        }
        _ => {}
    }

    let mut build_cmd = std::process::Command::new(&compiler);
    build_cmd
        .args(&comp_args)
        .current_dir(&work_dir)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    // Go cdylib requires CGO_ENABLED=1.
    if lang_norm == "go" {
        build_cmd.env("CGO_ENABLED", "1");
    }
    let build_output = build_cmd
        .output()
        .map_err(|e| host.err(format!("polyglot_compile_cached(): spawn {compiler}: {e}")))?;
    if !build_output.status.success() {
        let stderr = String::from_utf8_lossy(&build_output.stderr).to_string();
        let stdout = String::from_utf8_lossy(&build_output.stdout).to_string();
        return Err(host.err(format!(
            "polyglot_compile_cached({lang_norm}): compile failed (exit {:?})\n--- stdout ---\n{stdout}\n--- stderr ---\n{stderr}",
            build_output.status.code()
        )));
    }

    // NativeAOT (C#/F#) writes the .so/.dll/.dylib into a deep
    // bin/Release/<TFM>/<RID>/publish/ tree — not into staged_lib.
    // Copy it across before the staged_lib existence check.
    if lang_norm == "csharp" || lang_norm == "fsharp" {
        let publish_dir = work_dir
            .join("bin")
            .join("Release")
            .join("net8.0")
            .join(dotnet_rid())
            .join("publish");
        let ext = cdylib_ext();
        // NativeAOT names the output after the assembly name, which
        // defaults to the project file's stem ("prog" → prog.so/.dll/.dylib).
        // Linux often prepends "lib": libprog.so. Cover both names + any
        // other .so/.dll/.dylib in the publish dir as a fallback.
        let candidates = [
            publish_dir.join(format!("prog.{ext}")),
            publish_dir.join(format!("libprog.{ext}")),
        ];
        let mut found: Option<std::path::PathBuf> = None;
        for c in &candidates {
            if c.is_file() {
                found = Some(c.clone());
                break;
            }
        }
        if found.is_none() {
            if let Ok(entries) = std::fs::read_dir(&publish_dir) {
                for entry in entries.flatten() {
                    let p = entry.path();
                    if p.extension().and_then(|s| s.to_str()) == Some(ext) {
                        found = Some(p);
                        break;
                    }
                }
            }
        }
        let produced = match found {
            Some(p) => p,
            None => {
                return Err(host.err(format!(
                    "polyglot_compile_cached({lang_norm}): NativeAOT publish completed but no .{ext} found under {}. \
                     Ensure your project uses [UnmanagedCallersOnly] exports and that PublishAot+NativeLib=Shared are in the csproj.",
                    publish_dir.display()
                )));
            }
        };
        if let Err(e) = std::fs::copy(&produced, &staged_lib) {
            return Err(host.err(format!(
                "polyglot_compile_cached({lang_norm}): copy {} → {}: {e}",
                produced.display(),
                staged_lib.display()
            )));
        }
    }

    // Nim names the produced library after the source basename in
    // shared-lib mode unless we override; we passed --out:staged_lib
    // already, so the file should be at staged_lib directly. Some
    // older Nim versions still emit `libsrc.so` regardless — fall
    // back to that name.
    if lang_norm == "nim" && !staged_lib.is_file() {
        let fallback = work_dir.join(format!("libsrc.{ext}", ext = cdylib_ext()));
        if fallback.is_file() {
            let _ = std::fs::rename(&fallback, &staged_lib);
        }
    }

    // GraalVM native-image and Kotlin/Native both place output in
    // work_dir with a name we asked for, but on Unix they prepend
    // `lib`. Fall back to that name if the bare one isn't there.
    if (lang_norm == "java" || lang_norm == "kotlin") && !staged_lib.is_file() {
        let stem = if lang_norm == "java" {
            "prog"
        } else {
            "staged_kn"
        };
        let ext = cdylib_ext();
        let candidates = [
            work_dir.join(format!("{stem}.{ext}")),
            work_dir.join(format!("lib{stem}.{ext}")),
        ];
        for c in &candidates {
            if c.is_file() {
                let _ = std::fs::rename(c, &staged_lib);
                break;
            }
        }
        // Last-ditch: scan work_dir for *any* .so/.dll/.dylib that
        // wasn't an intermediate (e.g. *.o).
        if !staged_lib.is_file() {
            if let Ok(entries) = std::fs::read_dir(&work_dir) {
                for entry in entries.flatten() {
                    let p = entry.path();
                    if p.extension().and_then(|s| s.to_str()) == Some(ext) && p != staged_lib {
                        let _ = std::fs::rename(&p, &staged_lib);
                        break;
                    }
                }
            }
        }
    }

    if !staged_lib.is_file() {
        return Err(host.err(format!(
            "polyglot_compile_cached({lang_norm}): expected output {} not produced",
            staged_lib.display()
        )));
    }

    // Atomically promote into the cache (rename within the same FS is
    // atomic on Linux/macOS; fall back to copy on EXDEV).
    if let Err(e) = std::fs::rename(&staged_lib, &cached_path) {
        if let Err(e2) = std::fs::copy(&staged_lib, &cached_path) {
            return Err(host.err(format!(
                "polyglot_compile_cached(): promote to cache failed (rename: {e}; copy: {e2})"
            )));
        }
    }

    if !host.ffi_allowed_library(&cached_path.to_string_lossy()) {
        return Err(host.err(format!(
            "polyglot_compile_cached(): freshly-built library {} not in FFI allowlist; set FfiPolicy.allow_all=true or add the cache dir to allowed_libs",
            cached_path.display()
        )));
    }
    // SAFETY: same contract as the cache-hit path — we just built the
    // shared library ourselves (verified `staged_lib.is_file()`,
    // promoted to `cached_path` via rename/copy, ran the host FFI
    // allowlist check). dlopen runs the library's initializers in this
    // process; that's intentional and gated by `host.allow_ffi()` +
    // `host.allow_exec()` together (both required for this builtin).
    let lib = unsafe { Library::new(&cached_path) }
        .map_err(|e| host.err(format!("polyglot_compile_cached(): load: {e}")))?;
    let id = with_ffi_table(host, |t| {
        let id = FFI_NEXT.fetch_add(1, Ordering::Relaxed);
        t.insert(id, FfiResource::Library(lib));
        id
    })?;
    Ok(Value::Number(id as f64))
}

/// polyglot_cache_clear() — wipe the cdylib cache directory.
/// Returns the number of files removed.
pub(super) fn polyglot_cache_clear(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    if !args.is_empty() {
        return Err(host.err(format!(
            "polyglot_cache_clear() expects 0 arguments, got {}",
            args.len()
        )));
    }
    let dir = polyglot_cache_dir();
    if !dir.is_dir() {
        return Ok(Value::Number(0.0));
    }
    let mut count = 0u64;
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_file() {
                let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
                if name.starts_with("pblang_polyglot_") {
                    if std::fs::remove_file(&p).is_ok() {
                        count += 1;
                    }
                }
            }
        }
    }
    Ok(Value::Number(count as f64))
}

/// polyglot_cache_info() — return JSON about the cache:
///   { dir, count, size_bytes }
pub(super) fn polyglot_cache_info(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    if !args.is_empty() {
        return Err(host.err(format!(
            "polyglot_cache_info() expects 0 arguments, got {}",
            args.len()
        )));
    }
    let dir = polyglot_cache_dir();
    let mut count = 0u64;
    let mut total: u64 = 0;
    if dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_file() {
                    let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
                    if name.starts_with("pblang_polyglot_") {
                        count += 1;
                        if let Ok(md) = std::fs::metadata(&p) {
                            total += md.len();
                        }
                    }
                }
            }
        }
    }
    let json = format!(
        "{{\"dir\":\"{}\",\"count\":{},\"size_bytes\":{}}}",
        json_escape_string(&dir.to_string_lossy()),
        count,
        total
    );
    Ok(alloc_string(json))
}
