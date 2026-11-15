/* ============================================================================
 * pblang Sovereign Zero-Rust Native Runtime & Compiler — v4.2.0
 *
 * 100% Rust-independent cross-platform & multi-device C99 implementation of:
 *   - Native `.sapp -> .pbck` v1 / `PBSH` v1 preprocessor, Unicode lexer, parser,
 *     SI unit annotations, `requires`/`ensures` contracts, `@nogc`/`@packed`
 *     attributes, sandboxed `comptime`, AST desugarer, N-level lexical upvalues,
 *     algebraic `enum`/`match`/`Generics`/`trait`, bytecode compiler &
 *     zero-overhead polymorphic encrypted/stripped serializer & deserializer
 *   - Computed-Goto threaded VM, peephole superinstructions, monomorphic ICs,
 *     W^X `x86_64` SSE2 + `ARM64` (`A64`) native JIT, two-generation nursery GC,
 *     zero-copy `OBJ_BYTES`/`OBJ_F64BUF` + unboxed `OBJ_STRUCT_BUF` tables,
 *     HTTP/1.1 Keep-Alive/chunked/streaming/multipart + UDP sockets, CSR graph
 *     kernels, 3D PBR rasterizer, per-module call-stack capabilities, VM taint
 *     tracking, deterministic `.pbrec` flight-recorder/replay, zero-overhead
 *     line/branch coverage engine, unrestricted 10-arg FFI + C callbacks,
 *     OS anti-screenshot shield & 335+ host builtins
 *   - Unified native CLI: `run`, `repl`, `test` (`--coverage`), `fuzz`, `check`,
 *     `fmt`, `doc`, `lsp` (workspace v4.2), `debug`, `prof`, `replay`,
 *     `shield-inspect`, `build` (`--aot-native`, `--freestanding`, `--bundle-exe`)
 *
 * Linux build:
 *   gcc -std=c99 -O3 -Wall -Wextra -Wno-unused-parameter \
 *       runtime_stage3/pblang_zero_rust_vm.c -ldl -lpthread -lm -o pblang
 *
 * Windows PE32+ cross-build:
 *   x86_64-w64-mingw32-gcc -std=c99 -O2 -fno-strict-aliasing -mstackrealign \
 *       runtime_stage3/pblang_zero_rust_vm.c \
 *       -lws2_32 -lwinhttp -lgdi32 -luser32 -lcrypt32 -lbcrypt -lm -o pblang.exe
 * ============================================================================ */

#include "zr_builtins_core.h"

#ifdef _WIN32
#include <io.h>
#include <fcntl.h>
#else
#include <dirent.h>
#include <sys/resource.h>
#endif

#define PBLANG_ZR_VERSION_STR "pblang 5.2.0 (sovereign zero-rust runtime; compat 1.5.0)"

static void zr_print_version(void) {
    printf("%s\n", PBLANG_ZR_VERSION_STR);
}

static void zr_print_help(const char *prog) {
    const char *p = (prog && *prog) ? prog : "pblang";
    printf("%s\n\n", PBLANG_ZR_VERSION_STR);
    printf("Usage:\n");
    printf("  %s <file.sapp|file.pbck> [args...]                  Execute script or bytecode\n", p);
    printf("  %s run [flags] <file.sapp|file.pbck> [args...]      Execute with explicit flags\n", p);
    printf("  %s -e <code_string> [args...]                       Evaluate inline pblang code\n", p);
    printf("  %s repl                                             Interactive native PBLang REPL\n", p);
    printf("  %s test [--coverage] [--coverage-json <f>] [--coverage-html <f>] [files_or_dirs...]\n", p);
    printf("  %s fuzz <file.sapp> [--iterations N] [--seed S] [--json]   Property & crash fuzzer\n", p);
    printf("  %s check [--strict] [--syntax-only] <file.sapp>     Compile-check & verify `.sapp` (`@nogc`)\n", p);
    printf("  %s fmt [--write] [--check] <files...>               Format `.sapp` source files\n", p);
    printf("  %s doc [-o <out.md>] <file.sapp>                    Generate Markdown API documentation\n", p);
    printf("  %s lsp                                              Start stdio JSON-RPC 2.0 Language Server\n", p);
    printf("  %s debug <file.sapp> [args...]                      Interactive bytecode/source debugger\n", p);
    printf("  %s prof [--json] [--svg <out.svg>] <file.sapp>      Run hotspot profiler & flame-bar report\n", p);
    printf("  %s replay <trace.pbrec> <file.sapp> [args...]       Deterministic time-travel replay\n", p);
    printf("  %s build <file.sapp> [-o <out>] [--aot-native | --freestanding | --target <t> | --bundle-exe] [--shield | --open-symbols | --anti-debug] [--free | --license <SPDX> | --stealth]\n", p);
    printf("      Targets: pbck | web | aot-native | aot-pe64 | baremetal-x64 | baremetal-x86 | android-apk | android-dex | ios-macho | ios-app | x64 | x64-dyn | pe64 | arm64 | wasm | x86 | pe32 | rv32 | dos16 | cpm80 | c64 | avr8\n");
    printf("  %s shield-inspect <file>                            Inspect PBSH-v1 shield, entropy & integrity\n", p);
    printf("  %s ip-seal <file> [-o <out>] [--free | --license <SPDX> | --copyright-holder <name> | --stealth]\n", p);
    printf("  %s ip-inspect <file> | ip-verify <file> [--sign-env <ENV_VAR>]\n", p);
    printf("  %s license | credits                                Show Apache-2.0 license, NOTICE & creator attribution\n", p);
    printf("  %s doctor | hardware                                Inspect runtime, JIT, stdlib & host caps\n", p);
    printf("  %s update [--check]                                 Show sovereign runtime version & update info\n", p);
    printf("  %s --version | -V                                   Print version\n", p);
    printf("  %s --help | -h                                      Print this help\n\n", p);
    printf("Capability, Security & Flight-Recorder Flags:\n");
    printf("  --allow-all               Enable all host capabilities (default unless --sandbox)\n");
    printf("  --sandbox                 Restrict host I/O unless specific --allow-* flags are given\n");
    printf("  --allow-fs / --deny-fs    Allow or deny filesystem read/write operations\n");
    printf("  --allow-net / --deny-net  Allow or deny TCP/TLS/UDP socket operations\n");
    printf("  --allow-http              Allow HTTP/HTTPS client & AI provider requests\n");
    printf("  --allow-sqlite            Allow native SQLite3 database operations\n");
    printf("  --allow-ffi               Allow dynamic shared-library FFI loading\n");
    printf("  --ffi-allow-all           Allow arbitrary library paths & symbols under FFI\n");
    printf("  --allow-exec              Allow subprocess & polyglot execution\n");
    printf("  --allow-gfx               Allow X11 / Win32GDI / headless graphics windows\n");
    printf("  --allow-spawn             Allow OS thread spawning (`spawn` / `spawn_n`)\n");
    printf("  --allow-env               Allow environment variable access\n");
    printf("  --strict-taint            Enforce VM taint guard on I/O sinks & sensitive env vars\n");
    printf("  --record <file.pbrec>     Record non-deterministic syscalls to `.pbrec` trace\n");
    printf("  --stdlib <dir>            Override stdlib directory path\n");
    printf("  --filter <substr>         Filter `test_*` functions / test file paths\n");
    printf("  --coverage                Collect bytecode line & branch coverage in `pblang test`\n");
    printf("  --coverage-json <file>    Export coverage report as JSON (`pblang-coverage-v1`)\n");
    printf("  --coverage-html <file>    Export self-contained dark-mode interactive HTML coverage report\n");
    printf("  -q, --quiet               Compact test output\n");
}

static bool zr_str_ends_ci(const char *s, const char *suffix) {
    size_t sl = strlen(s), xl = strlen(suffix);
    if (sl < xl) return false;
    for (size_t i = 0; i < xl; i++) {
        if (tolower((unsigned char)s[sl - xl + i]) != tolower((unsigned char)suffix[i])) return false;
    }
    return true;
}

static void zr_configure_vm_paths(PbVM *vm, const char *self_exe, const char *script_path, const char *stdlib_override) {
    if (self_exe && *self_exe) {
        free(vm->exe_path);
        vm->exe_path = zr_xstrdup(self_exe);
    }
    if (script_path && *script_path) {
        char canon[4096];
        zr_canonicalize_path(script_path, canon);
        free(vm->current_file);
        vm->current_file = zr_xstrdup(canon);
    }
    char stdlib_dir[4096];
    stdlib_dir[0] = '\0';
    if (stdlib_override && *stdlib_override) {
        zr_canonicalize_path(stdlib_override, stdlib_dir);
    } else {
        zr_discover_stdlib_root(self_exe, vm->current_file, stdlib_dir);
    }
    if (stdlib_dir[0]) {
        free(vm->stdlib_root);
        vm->stdlib_root = zr_xstrdup(stdlib_dir);
    }
}

static int zr_run_embedded_bundle(const char *self_exe, const uint8_t *pbck, size_t pbck_len, int argc, char **argv) {
    PbVM vm;
    vm_init(&vm);
    zr_caps_allow_all(&vm.caps);
    const char *sb_env = getenv("PBLANG_SANDBOX");
    if (sb_env && strcmp(sb_env, "1") == 0) zr_caps_init_default(&vm.caps);
    zr_configure_vm_paths(&vm, self_exe, self_exe, NULL);
    vm.cli_argc = argc;
    vm.cli_argv = argv;

    char err[256];
    err[0] = '\0';
    PbFunc *fn = pbck_deserialize_script(&vm, pbck, pbck_len, err);
    if (!fn) {
        fprintf(stderr, "pblang bundle error: %s\n", err[0] ? err : "corrupt embedded bytecode");
        vm_free(&vm);
        return 1;
    }
    vm_push_root(&vm, val_obj(fn));
    zr_populate_polyglot_globals(&vm, vm.globals, fn);
    PbValue ret = val_nil();
    bool ok = vm_run_root_func(&vm, fn, NULL, &ret);
    vm_pop_roots(&vm, 1);
    int code = 0;
    if (vm.exited) {
        code = vm.exit_code;
    } else if (!ok) {
        fprintf(stderr, "Runtime error: %s\n", vm.err_msg[0] ? vm.err_msg : "execution failed");
        code = 1;
    }
    zr_flight_cleanup_vm(&vm);
    vm_free(&vm);
    return code;
}

static void zr_apply_cap_flag(ZrCaps *caps, bool *sandbox_seen, const char *arg) {
    if (strcmp(arg, "--sandbox") == 0 || strcmp(arg, "--no-allow-all") == 0) {
        if (!*sandbox_seen) {
            zr_caps_init_default(caps);
            *sandbox_seen = true;
        }
    } else if (strcmp(arg, "--allow-all") == 0) {
        zr_caps_allow_all(caps);
    } else if (strcmp(arg, "--allow-fs") == 0) {
        caps->allow_fs = true;
    } else if (strcmp(arg, "--allow-net") == 0) {
        caps->allow_net = true;
    } else if (strcmp(arg, "--allow-http") == 0) {
        caps->allow_http = true;
        caps->allow_net = true;
    } else if (strcmp(arg, "--allow-sqlite") == 0) {
        caps->allow_sqlite = true;
    } else if (strcmp(arg, "--allow-ffi") == 0) {
        caps->allow_ffi = true;
    } else if (strcmp(arg, "--ffi-allow-all") == 0) {
        caps->allow_ffi = true;
        caps->ffi_allow_all = true;
    } else if (strcmp(arg, "--allow-exec") == 0) {
        caps->allow_exec = true;
    } else if (strcmp(arg, "--allow-gfx") == 0) {
        caps->allow_gfx = true;
    } else if (strcmp(arg, "--allow-import") == 0) {
        caps->allow_import = true;
        caps->allow_stdlib_import = true;
    } else if (strcmp(arg, "--allow-stdlib") == 0) {
        caps->allow_stdlib_import = true;
    } else if (strcmp(arg, "--allow-spawn") == 0) {
        caps->allow_spawn = true;
    } else if (strcmp(arg, "--allow-env") == 0) {
        caps->allow_env = true;
    } else if (strcmp(arg, "--allow-exit") == 0) {
        caps->allow_process_exit = true;
    } else if (strcmp(arg, "--deny-fs") == 0) {
        caps->allow_fs = false;
    } else if (strcmp(arg, "--deny-net") == 0) {
        caps->allow_net = false;
    } else if (strcmp(arg, "--deny-http") == 0) {
        caps->allow_http = false;
    } else if (strcmp(arg, "--deny-sqlite") == 0) {
        caps->allow_sqlite = false;
    } else if (strcmp(arg, "--deny-ffi") == 0) {
        caps->allow_ffi = false;
        caps->ffi_allow_all = false;
    } else if (strcmp(arg, "--deny-exec") == 0) {
        caps->allow_exec = false;
    } else if (strcmp(arg, "--deny-gfx") == 0) {
        caps->allow_gfx = false;
    } else if (strcmp(arg, "--deny-import") == 0) {
        caps->allow_import = false;
    } else if (strcmp(arg, "--deny-spawn") == 0) {
        caps->allow_spawn = false;
    } else if (strcmp(arg, "--deny-env") == 0) {
        caps->allow_env = false;
    }
}

static bool zr_is_cap_flag(const char *arg) {
    return strcmp(arg, "--sandbox") == 0 ||
           strcmp(arg, "--no-allow-all") == 0 ||
           strcmp(arg, "--allow-all") == 0 ||
           strncmp(arg, "--allow-", 8) == 0 ||
           strncmp(arg, "--deny-", 7) == 0 ||
           strcmp(arg, "--ffi-allow-all") == 0;
}

typedef struct {
    bool        strict_taint;
    const char *record_path;
    const char *replay_path;
    bool        prof_enabled;
    bool        prof_json;
    const char *prof_svg_path;
} ZrRunOpts;

static int zr_cmd_eval_ex(const char *self_exe, const char *code_str, const ZrCaps *caps,
                          const char *stdlib_override, const ZrRunOpts *opts,
                          int script_argc, char **script_argv) {
    PbVM vm;
    vm_init(&vm);
    vm.caps = *caps;
    if (opts && opts->strict_taint) vm.taint_guard_enabled = true;
    if (opts && opts->prof_enabled) vm.prof_enabled = true;
    zr_configure_vm_paths(&vm, self_exe, "<eval>", stdlib_override);
    vm.cli_argc = script_argc;
    vm.cli_argv = script_argv;

    if (opts && opts->record_path) {
        if (!zr_flight_start_record_file(&vm, opts->record_path)) {
            fprintf(stderr, "Runtime error: %s\n", vm.err_msg[0] ? vm.err_msg : "cannot open record file");
            vm_free(&vm);
            return 1;
        }
    }
    if (opts && opts->replay_path) {
        if (!zr_flight_start_replay_file(&vm, opts->replay_path)) {
            fprintf(stderr, "Runtime error: %s\n", vm.err_msg[0] ? vm.err_msg : "cannot open replay file");
            vm_free(&vm);
            return 1;
        }
    }

    char cerr[512];
    cerr[0] = '\0';
    PbFunc *fn = zr_compile_sapp(&vm, code_str, strlen(code_str), "<eval>", cerr);
    if (!fn) {
        fprintf(stderr, "Compile error: %s\n", cerr[0] ? cerr : "syntax error");
        zr_flight_cleanup_vm(&vm);
        vm_free(&vm);
        return 1;
    }
    vm_push_root(&vm, val_obj(fn));
    zr_populate_polyglot_globals(&vm, vm.globals, fn);
    PbValue ret = val_nil();
    bool ok = vm_run_root_func(&vm, fn, NULL, &ret);
    vm_pop_roots(&vm, 1);
    int rc = 0;
    if (vm.exited) rc = vm.exit_code;
    else if (!ok) {
        fprintf(stderr, "Runtime error: %s\n", vm.err_msg[0] ? vm.err_msg : "execution failed");
        rc = 1;
    }
    if (opts && opts->prof_enabled) {
        ZrBuf pbuf;
        zr_buf_init(&pbuf);
        zr_prof_collect_and_format(&vm, opts->prof_json, opts->prof_svg_path, &pbuf);
        if (pbuf.data && pbuf.len > 0) fwrite(pbuf.data, 1, pbuf.len, stdout);
        zr_buf_free(&pbuf);
    }
    zr_flight_cleanup_vm(&vm);
    vm_free(&vm);
    return rc;
}

static int zr_cmd_eval(const char *self_exe, const char *code_str, const ZrCaps *caps,
                       const char *stdlib_override, int script_argc, char **script_argv) {
    return zr_cmd_eval_ex(self_exe, code_str, caps, stdlib_override, NULL, script_argc, script_argv);
}

static PbFunc *zr_load_file_pbsh_aware(PbVM *vm, const char *path, char err_buf[512]) {
    FILE *fp = fopen(path, "rb");
    if (fp) {
        uint8_t magic[4];
        size_t n = fread(magic, 1, 4, fp);
        fclose(fp);
        if (n == 4 && (memcmp(magic, "PBSH", 4) == 0 || memcmp(magic, "PBCK", 4) == 0)) {
            uint8_t *raw = NULL;
            size_t raw_len = 0;
            if (!zr_read_entire_file(path, &raw, &raw_len)) {
                snprintf(err_buf, 512, "cannot read '%s'", path);
                return NULL;
            }
            char derr[256];
            derr[0] = '\0';
            PbFunc *fn = pbck_deserialize_script(vm, raw, raw_len, derr);
            free(raw);
            if (!fn) snprintf(err_buf, 512, "%s", derr[0] ? derr : "corrupt bytecode container");
            return fn;
        }
    }
    return zr_load_or_compile_file(vm, path, err_buf);
}

static int zr_cmd_run_file_ex(const char *self_exe, const char *target_file, const ZrCaps *caps,
                              const char *stdlib_override, const ZrRunOpts *opts,
                              int script_argc, char **script_argv) {
    if (!zr_file_exists_reg(target_file)) {
        fprintf(stderr, "Error: file not found: %s\n", target_file);
        return 1;
    }
    PbVM *vmp = (PbVM *)zr_xcalloc(1, sizeof(PbVM));
    vm_init(vmp);
    vmp->caps = *caps;
    if (opts && opts->strict_taint) vmp->taint_guard_enabled = true;
    if (opts && opts->prof_enabled) vmp->prof_enabled = true;
    zr_configure_vm_paths(vmp, self_exe, target_file, stdlib_override);
    vmp->cli_argc = script_argc;
    vmp->cli_argv = script_argv;

    if (opts && opts->record_path) {
        if (!zr_flight_start_record_file(vmp, opts->record_path)) {
            fprintf(stderr, "Runtime error: %s\n", vmp->err_msg[0] ? vmp->err_msg : "cannot open record file");
            vm_free(vmp);
            free(vmp);
            return 1;
        }
    }
    if (opts && opts->replay_path) {
        if (!zr_flight_start_replay_file(vmp, opts->replay_path)) {
            fprintf(stderr, "Runtime error: %s\n", vmp->err_msg[0] ? vmp->err_msg : "cannot open replay file");
            vm_free(vmp);
            free(vmp);
            return 1;
        }
    }

    char lerr[512];
    lerr[0] = '\0';
    PbFunc *fn = zr_load_file_pbsh_aware(vmp, vmp->current_file ? vmp->current_file : target_file, lerr);
    if (!fn) {
        fprintf(stderr, "%s\n", lerr[0] ? lerr : "Failed to load or compile script");
        zr_flight_cleanup_vm(vmp);
        vm_free(vmp);
        free(vmp);
        return 1;
    }
    vm_push_root(vmp, val_obj(fn));
    zr_populate_polyglot_globals(vmp, vmp->globals, fn);
    PbValue ret = val_nil();
    bool ok = vm_run_root_func(vmp, fn, NULL, &ret);
    vm_pop_roots(vmp, 1);
    int rc = 0;
    if (vmp->exited) {
        rc = vmp->exit_code;
    } else if (!ok) {
        fprintf(stderr, "Runtime error: %s\n", vmp->err_msg[0] ? vmp->err_msg : "execution failed");
        rc = 1;
    }
    if (opts && opts->prof_enabled) {
        ZrBuf pbuf;
        zr_buf_init(&pbuf);
        zr_prof_collect_and_format(vmp, opts->prof_json, opts->prof_svg_path, &pbuf);
        if (pbuf.data && pbuf.len > 0) fwrite(pbuf.data, 1, pbuf.len, stdout);
        zr_buf_free(&pbuf);
    }
    zr_flight_cleanup_vm(vmp);
    vm_free(vmp);
    free(vmp);
    return rc;
}

static int zr_cmd_run_file(const char *self_exe, const char *target_file, const ZrCaps *caps,
                           const char *stdlib_override, int script_argc, char **script_argv) {
    return zr_cmd_run_file_ex(self_exe, target_file, caps, stdlib_override, NULL, script_argc, script_argv);
}

/* ---- `pblang check [--strict] [--syntax-only] <file.sapp>` ---- */

static void zr_strict_collect_defs(const PbFunc *fn, PbMap *defs, PbVM *vm) {
    if (!fn) return;
    if (fn->name && fn->name->len > 0) {
        pb_map_set(vm, defs, fn->name, val_bool(true));
    }
    for (uint32_t i = 0; i < fn->code_len; i++) {
        uint8_t op = fn->code[i].orig_op ? fn->code[i].orig_op : fn->code[i].op;
        if (op == OP_DEFINE_GLOBAL || op == OP_SET_GLOBAL) {
            uint32_t idx = fn->code[i].arg;
            if (idx < fn->const_len && is_obj_type(fn->consts[idx], OBJ_STR)) {
                pb_map_set(vm, defs, AS_STR(fn->consts[idx]), val_bool(true));
            }
        }
    }
    for (uint32_t c = 0; c < fn->const_len; c++) {
        if (is_obj_type(fn->consts[c], OBJ_FUNC)) {
            zr_strict_collect_defs(AS_FUNC(fn->consts[c]), defs, vm);
        }
    }
}

static int zr_strict_verify_refs(const PbFunc *fn, PbMap *defs, PbVM *vm, const char *file_label) {
    if (!fn) return 0;
    int errs = 0;
    for (uint32_t i = 0; i < fn->code_len; i++) {
        uint8_t op = fn->code[i].orig_op ? fn->code[i].orig_op : fn->code[i].op;
        if (op == OP_GET_GLOBAL) {
            uint32_t idx = fn->code[i].arg;
            if (idx < fn->const_len && is_obj_type(fn->consts[idx], OBJ_STR)) {
                PbStr *nm = AS_STR(fn->consts[idx]);
                PbValue tmp = val_nil();
                if (!pb_map_get(defs, nm, &tmp) &&
                    !pb_map_get(vm->builtins, nm, &tmp) &&
                    !pb_map_get(vm->globals, nm, &tmp) &&
                    strcmp(nm->chars, "result") != 0) {
                    fprintf(stderr, "Strict check error (%s:%u): unresolved identifier '%s'\n",
                            file_label, fn->code[i].line, nm->chars);
                    errs++;
                }
            }
        }
    }
    for (uint32_t c = 0; c < fn->const_len; c++) {
        if (is_obj_type(fn->consts[c], OBJ_FUNC)) {
            errs += zr_strict_verify_refs(AS_FUNC(fn->consts[c]), defs, vm, file_label);
        }
    }
    return errs;
}

static int zr_cmd_check_file_ex(const char *self_exe, const char *target_file,
                                const char *stdlib_override, bool strict_mode, bool syntax_only, bool json_mode) {
    uint8_t *raw = NULL;
    size_t raw_len = 0;
    if (!zr_read_entire_file(target_file, &raw, &raw_len)) {
        fprintf(stderr, "check: cannot read '%s'\n", target_file);
        return 1;
    }
    bool is_pbck = (raw_len >= 4 && (memcmp(raw, "PBCK", 4) == 0 || memcmp(raw, "PBSH", 4) == 0));
    PbVM vm;
    vm_init(&vm);
    zr_configure_vm_paths(&vm, self_exe, target_file, stdlib_override);
    PbFunc *fn = NULL;
    char err[512];
    err[0] = '\0';
    if (is_pbck) {
        char derr[256];
        derr[0] = '\0';
        fn = pbck_deserialize_script(&vm, raw, raw_len, derr);
        if (!fn) snprintf(err, sizeof(err), "%s", derr);
    } else {
        fn = zr_compile_sapp(&vm, (const char *)raw, raw_len, target_file, err);
    }
    free(raw);
    if (!fn) {
        fprintf(stderr, "Check failed (%s): %s\n", target_file, err[0] ? err : "invalid syntax");
        vm_free(&vm);
        return 1;
    }
    if (syntax_only || is_pbck) {
        printf("OK (syntax): %s (%u instructions)\n", target_file, fn->code_len);
        vm_free(&vm);
        return 0;
    }

    /* Delegate to full 5-pass static checker (9D SI units, EUR/USD, CAD/Presburger contracts, Quantum No-Cloning) if stdlib is present */
    char sc_cand[4160];
    sc_cand[0] = '\0';
    if (vm.stdlib_root && *vm.stdlib_root) {
        snprintf(sc_cand, sizeof(sc_cand), "%.4080s/static_checker.sapp", vm.stdlib_root);
    }
    uint32_t code_len_saved = fn->code_len;
    uint32_t const_len_saved = fn->const_len;
    if (sc_cand[0] && zr_file_exists_reg(sc_cand)) {
        vm_free(&vm);
        ZrCaps scaps;
        zr_caps_allow_all(&scaps);
        char *cargs[4];
        int cn = 0;
        if (strict_mode) cargs[cn++] = (char *)"--strict";
        if (json_mode) cargs[cn++] = (char *)"--json";
        cargs[cn++] = (char *)target_file;
        const char *eval_sc = "let SC = import(\"stdlib:static_checker.sapp\"); SC.cli_check_entry(os_args());";
        int rc = zr_cmd_eval(self_exe, eval_sc, &scaps, stdlib_override, cn, cargs);
        if (rc == 0 && !json_mode) {
            printf("OK: %s (%u instructions, %u constants%s)\n",
                   target_file, code_len_saved, const_len_saved, strict_mode ? ", strict + SI/contracts verified" : ", SI/contracts verified");
        }
        return rc;
    }

    if (strict_mode) {
        vm_push_root(&vm, val_obj(fn));
        zr_populate_polyglot_globals(&vm, vm.globals, fn);
        PbMap *defs = pb_map_new_raw(&vm, 64);
        vm_push_root(&vm, val_obj(defs));
        zr_strict_collect_defs(fn, defs, &vm);
        int serrs = zr_strict_verify_refs(fn, defs, &vm, target_file);
        vm_pop_roots(&vm, 2);
        if (serrs > 0) {
            vm_free(&vm);
            return 1;
        }
    }
    printf("OK: %s (%u instructions, %u constants%s)\n",
           target_file, fn->code_len, fn->const_len, strict_mode ? ", strict verified" : "");
    vm_free(&vm);
    return 0;
}

static int zr_cmd_check_file(const char *self_exe, const char *target_file, const char *stdlib_override) {
    return zr_cmd_check_file_ex(self_exe, target_file, stdlib_override, false, false, false);
}

/* ---- WASM Binary Emitter for `pblang build <file.sapp> --target web` ---- */

static void zr_wasm_uleb(ZrBuf *b, uint32_t v) {
    do {
        uint8_t byte = (uint8_t)(v & 0x7Fu);
        v >>= 7;
        if (v != 0) byte |= 0x80u;
        zr_buf_push_u8(b, byte);
    } while (v != 0);
}

static void zr_wasm_sleb(ZrBuf *b, int32_t v) {
    bool more = true;
    while (more) {
        uint8_t byte = (uint8_t)(v & 0x7F);
        v >>= 7;
        if ((v == 0 && (byte & 0x40) == 0) || (v == -1 && (byte & 0x40) != 0)) {
            more = false;
        } else {
            byte |= 0x80u;
        }
        zr_buf_push_u8(b, byte);
    }
}

static void zr_wasm_section(ZrBuf *out, uint8_t sec_id, const ZrBuf *payload) {
    zr_buf_push_u8(out, sec_id);
    zr_wasm_uleb(out, (uint32_t)payload->len);
    if (payload->len > 0) zr_buf_append(out, payload->data, payload->len);
}

static void zr_wasm_export_entry(ZrBuf *sec, const char *name, uint8_t kind, uint32_t idx) {
    size_t nl = strlen(name);
    zr_wasm_uleb(sec, (uint32_t)nl);
    zr_buf_append(sec, name, nl);
    zr_buf_push_u8(sec, kind);
    zr_wasm_uleb(sec, idx);
}

static void zr_wasm_emit_const_fn(ZrBuf *sec, int32_t val) {
    ZrBuf body;
    zr_buf_init(&body);
    zr_buf_push_u8(&body, 0x00); /* 0 local declarations */
    zr_buf_push_u8(&body, 0x41); /* i32.const */
    zr_wasm_sleb(&body, val);
    zr_buf_push_u8(&body, 0x0B); /* end */
    zr_wasm_uleb(sec, (uint32_t)body.len);
    zr_buf_append(sec, body.data, body.len);
    zr_buf_free(&body);
}

static bool zr_emit_web_bundle(const char *input_path, const uint8_t *pbck_bytes, size_t pbck_len, const char *out_dir) {
    zr_mkdirs_path(out_dir);

    /* 1. Assemble valid WebAssembly v1 module (`app.wasm`) embedding `.pbck` at offset 1024 */
    ZrBuf wasm;
    zr_buf_init(&wasm);
    static const uint8_t magic_ver[8] = { 0x00, 0x61, 0x73, 0x6D, 0x01, 0x00, 0x00, 0x00 };
    zr_buf_append(&wasm, magic_ver, 8);

    /* Type section (id=1): 1 type () -> i32 */
    ZrBuf sec;
    zr_buf_init(&sec);
    zr_wasm_uleb(&sec, 1);
    zr_buf_push_u8(&sec, 0x60);
    zr_buf_push_u8(&sec, 0x00);
    zr_buf_push_u8(&sec, 0x01);
    zr_buf_push_u8(&sec, 0x7F);
    zr_wasm_section(&wasm, 1, &sec);
    zr_buf_free(&sec);

    /* Function section (id=3): 3 functions of type 0 */
    zr_buf_init(&sec);
    zr_wasm_uleb(&sec, 3);
    zr_wasm_uleb(&sec, 0);
    zr_wasm_uleb(&sec, 0);
    zr_wasm_uleb(&sec, 0);
    zr_wasm_section(&wasm, 3, &sec);
    zr_buf_free(&sec);

    /* Memory section (id=5): 1 linear memory large enough for 1024 + pbck_len */
    uint32_t min_pages = (uint32_t)((1024u + pbck_len + 65535u) / 65536u);
    if (min_pages < 1u) min_pages = 1u;
    zr_buf_init(&sec);
    zr_wasm_uleb(&sec, 1);
    zr_buf_push_u8(&sec, 0x00);
    zr_wasm_uleb(&sec, min_pages);
    zr_wasm_section(&wasm, 5, &sec);
    zr_buf_free(&sec);

    /* Export section (id=7) */
    zr_buf_init(&sec);
    zr_wasm_uleb(&sec, 4);
    zr_wasm_export_entry(&sec, "memory", 0x02, 0);
    zr_wasm_export_entry(&sec, "pblang_abi_version", 0x00, 0);
    zr_wasm_export_entry(&sec, "pblang_pbck_ptr", 0x00, 1);
    zr_wasm_export_entry(&sec, "pblang_pbck_len", 0x00, 2);
    zr_wasm_section(&wasm, 7, &sec);
    zr_buf_free(&sec);

    /* Code section (id=10) */
    zr_buf_init(&sec);
    zr_wasm_uleb(&sec, 3);
    zr_wasm_emit_const_fn(&sec, 300);
    zr_wasm_emit_const_fn(&sec, 1024);
    zr_wasm_emit_const_fn(&sec, (int32_t)pbck_len);
    zr_wasm_section(&wasm, 10, &sec);
    zr_buf_free(&sec);

    /* Data section (id=11): active segment at mem 0, offset 1024 */
    zr_buf_init(&sec);
    zr_wasm_uleb(&sec, 1);
    zr_buf_push_u8(&sec, 0x00);
    zr_buf_push_u8(&sec, 0x41);
    zr_wasm_sleb(&sec, 1024);
    zr_buf_push_u8(&sec, 0x0B);
    zr_wasm_uleb(&sec, (uint32_t)pbck_len);
    zr_buf_append(&sec, pbck_bytes, pbck_len);
    zr_wasm_section(&wasm, 11, &sec);
    zr_buf_free(&sec);

    char wasm_path[4096], js_path[4096], html_path[4096];
    snprintf(wasm_path, sizeof(wasm_path), "%s/app.wasm", out_dir);
    snprintf(js_path, sizeof(js_path), "%s/pblang_web_host.js", out_dir);
    snprintf(html_path, sizeof(html_path), "%s/index.html", out_dir);

    FILE *wf = fopen(wasm_path, "wb");
    if (!wf || fwrite(wasm.data, 1, wasm.len, wf) != wasm.len) {
        if (wf) fclose(wf);
        zr_buf_free(&wasm);
        return false;
    }
    fclose(wf);

    /* 2. Emit `pblang_web_host.js` */
    static const char *js_code =
        "// PBLang v4.2.0 WebAssembly Host Loader\n"
        "export async function loadPBLangWasm(wasmUrl = './app.wasm') {\n"
        "  const resp = await fetch(wasmUrl);\n"
        "  const bytes = await resp.arrayBuffer();\n"
        "  const { instance } = await WebAssembly.instantiate(bytes, {});\n"
        "  const { memory, pblang_abi_version, pblang_pbck_ptr, pblang_pbck_len } = instance.exports;\n"
        "  const ptr = pblang_pbck_ptr();\n"
        "  const len = pblang_pbck_len();\n"
        "  const pbck = new Uint8Array(memory.buffer, ptr, len);\n"
        "  const magic = String.fromCharCode(pbck[0], pbck[1], pbck[2], pbck[3]);\n"
        "  return { abiVersion: pblang_abi_version(), bytecodeLength: len, magic, pbck, instance };\n"
        "}\n";
    FILE *jf = fopen(js_path, "wb");
    if (!jf) { zr_buf_free(&wasm); return false; }
    fwrite(js_code, 1, strlen(js_code), jf);
    fclose(jf);

    /* 3. Emit `index.html` */
    FILE *hf = fopen(html_path, "wb");
    if (!hf) { zr_buf_free(&wasm); return false; }
    fprintf(hf,
        "<!doctype html>\n"
        "<html lang=\"en\">\n"
        "<head><meta charset=\"utf-8\"/><title>PBLang 4.2.0 Web App</title></head>\n"
        "<body style=\"background:#0f172a;color:#f8fafc;font-family:monospace;padding:24px\">\n"
        "  <h2>PBLang v4.2.0 WebAssembly Target</h2>\n"
        "  <pre id=\"out\">Loading app.wasm (%s)...</pre>\n"
        "  <script type=\"module\">\n"
        "    import { loadPBLangWasm } from './pblang_web_host.js';\n"
        "    loadPBLangWasm().then(info => {\n"
        "      document.getElementById('out').textContent =\n"
        "        `Loaded ${info.magic} v1 payload (${info.bytecodeLength} bytes, ABI ${info.abiVersion})`;\n"
        "    }).catch(err => { document.getElementById('out').textContent = String(err); });\n"
        "  </script>\n"
        "</body>\n"
        "</html>\n",
        input_path ? input_path : "app.sapp");
    fclose(hf);

    printf("Built web bundle in %s (app.wasm: %zu bytes, bytecode: %zu bytes)\n",
           out_dir, wasm.len, pbck_len);
    zr_buf_free(&wasm);
    return true;
}

static int zr_cmd_build(const char *self_exe, int argc, char **argv) {
    const char *input_path = NULL;
    const char *out_path = NULL;
    const char *stdlib_override = NULL;
    const char *target_arch = NULL;
    bool bundle_exe = false;
    bool safe_flag = false;
    bool overflow_flag = false;
    bool aot_native_flag = false;
    bool hybrid_flag = false;
    bool freestanding_flag = false;
    bool open_symbols_flag = false;
    bool shield_flag = false;
    bool anti_debug_flag = false;
    bool ip_explicit = false;
    bool ip_disable = false;
    const char *ip_mode_flag = NULL;
    const char *ip_license = NULL;
    const char *ip_holder = NULL;
    const char *ip_sign_env = NULL;

    for (int i = 0; i < argc; i++) {
        if (strcmp(argv[i], "--bundle-exe") == 0 || strcmp(argv[i], "--exe") == 0 || strcmp(argv[i], "--standalone") == 0) {
            bundle_exe = true;
        } else if (strcmp(argv[i], "--aot-native") == 0) {
            aot_native_flag = true;
        } else if (strcmp(argv[i], "--hybrid") == 0) {
            hybrid_flag = true;
        } else if (strcmp(argv[i], "--freestanding") == 0 || strcmp(argv[i], "--baremetal") == 0) {
            freestanding_flag = true;
        } else if (strcmp(argv[i], "--open-symbols") == 0 || strcmp(argv[i], "--no-obfuscate") == 0 || strcmp(argv[i], "--no-shield") == 0) {
            open_symbols_flag = true;
        } else if (strcmp(argv[i], "--obfuscate") == 0 || strcmp(argv[i], "--shield") == 0) {
            shield_flag = true;
        } else if (strcmp(argv[i], "--anti-debug") == 0) {
            anti_debug_flag = true;
        } else if (strcmp(argv[i], "--ip-seal") == 0) {
            ip_explicit = true;
        } else if (strcmp(argv[i], "--no-ip-seal") == 0) {
            ip_disable = true;
        } else if (strcmp(argv[i], "--free") == 0) {
            ip_mode_flag = "--free";
            ip_explicit = true;
        } else if (strcmp(argv[i], "--free-attribution") == 0) {
            ip_mode_flag = "--free-attribution";
            ip_explicit = true;
        } else if (strcmp(argv[i], "--stealth") == 0 || strcmp(argv[i], "--stealth-ip") == 0) {
            ip_mode_flag = "--stealth";
            ip_explicit = true;
        } else if (strcmp(argv[i], "--license") == 0 && i + 1 < argc) {
            ip_license = argv[++i];
            ip_explicit = true;
        } else if (strcmp(argv[i], "--copyright-holder") == 0 && i + 1 < argc) {
            ip_holder = argv[++i];
            ip_explicit = true;
        } else if (strcmp(argv[i], "--sign-env") == 0 && i + 1 < argc) {
            ip_sign_env = argv[++i];
            ip_explicit = true;
        } else if ((strcmp(argv[i], "-o") == 0 || strcmp(argv[i], "--out") == 0 || strcmp(argv[i], "--output") == 0) && i + 1 < argc) {
            out_path = argv[++i];
        } else if (strcmp(argv[i], "--target") == 0 && i + 1 < argc) {
            target_arch = argv[++i];
        } else if (strcmp(argv[i], "--stdlib") == 0 && i + 1 < argc) {
            stdlib_override = argv[++i];
        } else if (strcmp(argv[i], "--safe") == 0) {
            safe_flag = true;
        } else if (strcmp(argv[i], "--overflow") == 0 || strcmp(argv[i], "--checked-arith") == 0) {
            overflow_flag = true;
        } else if (argv[i][0] != '-' && !input_path) {
            input_path = argv[i];
        } else if (argv[i][0] != '-' && !out_path) {
            out_path = argv[i];
        }
    }
    if (!input_path) {
        fprintf(stderr, "Usage: pblang build <input.sapp> [-o <output>] [--aot-native | --freestanding | --target <arch> | --bundle-exe] [--shield | --open-symbols | --anti-debug] [--free | --license <SPDX> | --stealth]\n");
        return 1;
    }

    if (aot_native_flag) {
        if (!target_arch || strcmp(target_arch, "x64") == 0 || strcmp(target_arch, "elf64") == 0) {
            target_arch = "aot-native";
        } else if (strcmp(target_arch, "pe64") == 0 || strcmp(target_arch, "win64") == 0) {
            target_arch = "aot-pe64";
        }
    }
    if (freestanding_flag && !target_arch) {
        target_arch = "baremetal-x64";
    }

    if (target_arch && strcmp(target_arch, "pbck") != 0 && strcmp(target_arch, "web") != 0) {
        PbVM dvm;
        vm_init(&dvm);
        zr_configure_vm_paths(&dvm, self_exe, input_path, stdlib_override);
        char pbc_path[4096];
        pbc_path[0] = '\0';
        if (zr_file_exists_reg("tools/pbc.sapp")) {
            snprintf(pbc_path, sizeof(pbc_path), "tools/pbc.sapp");
        } else if (dvm.stdlib_root && *dvm.stdlib_root) {
            char cand[4160];
            snprintf(cand, sizeof(cand), "%s/../tools/pbc.sapp", dvm.stdlib_root);
            if (zr_file_exists_reg(cand)) snprintf(pbc_path, sizeof(pbc_path), "%.4090s", cand);
        }
        vm_free(&dvm);

        ZrCaps pcaps;
        zr_caps_allow_all(&pcaps);

        if (!pbc_path[0] && (strcmp(target_arch, "aot-native") == 0 || strcmp(target_arch, "aot-pe64") == 0 ||
                             strcmp(target_arch, "baremetal-x64") == 0 || strcmp(target_arch, "baremetal-x86") == 0)) {
            char def_out[4096];
            if (!out_path) {
                const char *base = input_path;
                for (const char *p = input_path; *p; p++) {
                    if (*p == '/' || *p == '\\') base = p + 1;
                }
                size_t bl = strlen(base);
                if (bl > 5 && zr_str_ends_ci(base, ".sapp")) bl -= 5;
                if (strcmp(target_arch, "aot-pe64") == 0) {
                    snprintf(def_out, sizeof(def_out), "%.*s.exe", (int)bl, base);
                } else if (strncmp(target_arch, "baremetal-", 10) == 0) {
                    snprintf(def_out, sizeof(def_out), "%.*s.elf", (int)bl, base);
                } else {
                    snprintf(def_out, sizeof(def_out), "%.*s", (int)bl, base);
                }
                out_path = def_out;
            }
            char *eargs[4];
            eargs[0] = (char *)input_path;
            eargs[1] = (char *)out_path;
            eargs[2] = (char *)target_arch;
            eargs[3] = (char *)(hybrid_flag ? "1" : "0");
            const char *eval_code =
                (strncmp(target_arch, "baremetal-", 10) == 0)
                ? "let A = os_args(); let BM = import(\"stdlib:native/baremetal_kernel_x64.sapp\"); "
                  "let src = fs_read(A[0]); "
                  "let bin = if A[2] == \"baremetal-x86\" { BM.compile_baremetal_x86(src) } else { BM.compile_baremetal_x64(src) }; "
                  "fs_write_bytes(A[1], bin); "
                  "print(\"Built Ring-0 bare-metal kernel: \" + A[1] + \" (\" + str(len(bin)) + \" bytes, \" + A[2] + \")\");"
                : "let A = os_args(); let AOT = import(\"stdlib:native/aot_unboxed_native.sapp\"); "
                  "let r = AOT.build_aot_native(A[0], A[1], map{\"target\": A[2], \"hybrid\": A[3] == \"1\"}); "
                  "print(\"Built pure-PBLang native AOT executable: \" + A[1] + \" (\" + str(r.bytes) + \" bytes, \" + r.mode + \")\");";
            int rc = zr_cmd_eval(self_exe, eval_code, &pcaps, stdlib_override, 4, eargs);
#ifndef _WIN32
            if (rc == 0 && strcmp(target_arch, "aot-native") == 0) chmod(out_path, 0755);
#endif
            return rc;
        }

        if (!pbc_path[0]) {
            fprintf(stderr, "build --target %s: tools/pbc.sapp not found\n", target_arch);
            return 1;
        }
        char *pargs[26];
        int pn = 0;
        pargs[pn++] = pbc_path;
        pargs[pn++] = (char *)input_path;
        pargs[pn++] = (char *)"--target";
        pargs[pn++] = (char *)target_arch;
        if (out_path) {
            pargs[pn++] = (char *)"--out";
            pargs[pn++] = (char *)out_path;
        }
        if (safe_flag) pargs[pn++] = (char *)"--safe";
        if (overflow_flag) pargs[pn++] = (char *)"--overflow";
        if (hybrid_flag) pargs[pn++] = (char *)"--hybrid";
        if (ip_disable) pargs[pn++] = (char *)"--no-ip-seal";
        else if (ip_explicit) pargs[pn++] = (char *)"--ip-seal";
        if (ip_mode_flag) pargs[pn++] = (char *)ip_mode_flag;
        if (ip_license) { pargs[pn++] = (char *)"--license"; pargs[pn++] = (char *)ip_license; }
        if (ip_holder) { pargs[pn++] = (char *)"--copyright-holder"; pargs[pn++] = (char *)ip_holder; }
        if (ip_sign_env) { pargs[pn++] = (char *)"--sign-env"; pargs[pn++] = (char *)ip_sign_env; }
        int rc = zr_cmd_run_file(self_exe, pbc_path, &pcaps, stdlib_override, pn, pargs);
#ifndef _WIN32
        if (rc == 0 && out_path && (strcmp(target_arch, "aot-native") == 0 || strcmp(target_arch, "x64") == 0 ||
                                    strcmp(target_arch, "x64-dyn") == 0 || strcmp(target_arch, "x86") == 0)) {
            chmod(out_path, 0755);
        }
#endif
        return rc;
    }

    bool is_open_mode =
        (ip_mode_flag && (strcmp(ip_mode_flag, "--free") == 0 || strcmp(ip_mode_flag, "--free-attribution") == 0)) ||
        (ip_license != NULL) ||
        open_symbols_flag;

    bool is_web_target = (target_arch && strcmp(target_arch, "web") == 0);
    bool use_shield = false;
    if (is_web_target) {
        use_shield = shield_flag || anti_debug_flag;
    } else if (bundle_exe) {
        use_shield = shield_flag || anti_debug_flag || !is_open_mode;
    } else {
        use_shield = shield_flag || anti_debug_flag;
    }

    uint8_t *raw = NULL;
    size_t raw_len = 0;
    if (!zr_read_entire_file(input_path, &raw, &raw_len)) {
        fprintf(stderr, "build: cannot read '%s'\n", input_path);
        return 1;
    }

    PbVM vm;
    vm_init(&vm);
    zr_configure_vm_paths(&vm, self_exe, input_path, stdlib_override);

    uint8_t *pbck_bytes = NULL;
    size_t pbck_len = 0;
    bool input_is_pbc = (raw_len >= 4 && (memcmp(raw, "PBCK", 4) == 0 || memcmp(raw, "PBSH", 4) == 0));
    if (input_is_pbc) {
        bool already_pbsh = (memcmp(raw, "PBSH", 4) == 0);
        if (already_pbsh == use_shield && !anti_debug_flag) {
            pbck_bytes = raw;
            pbck_len = raw_len;
            raw = NULL;
        } else {
            char derr[256];
            derr[0] = '\0';
            PbFunc *fn = pbck_deserialize_script(&vm, raw, raw_len, derr);
            free(raw);
            raw = NULL;
            if (!fn) {
                fprintf(stderr, "build error (%s): %s\n", input_path, derr[0] ? derr : "corrupt bytecode");
                vm_free(&vm);
                return 1;
            }
            vm_push_root(&vm, val_obj(fn));
            pbck_bytes = use_shield
                ? pbck_serialize_shielded(fn, anti_debug_flag, &pbck_len)
                : pbck_serialize_script(fn, &pbck_len);
            vm_pop_roots(&vm, 1);
        }
    } else {
        char cerr[512];
        cerr[0] = '\0';
        PbFunc *fn = zr_compile_sapp(&vm, (const char *)raw, raw_len, input_path, cerr);
        free(raw);
        raw = NULL;
        if (!fn) {
            fprintf(stderr, "build error (%s): %s\n", input_path, cerr[0] ? cerr : "compile failed");
            vm_free(&vm);
            return 1;
        }
        vm_push_root(&vm, val_obj(fn));
        pbck_bytes = use_shield
            ? pbck_serialize_shielded(fn, anti_debug_flag, &pbck_len)
            : pbck_serialize_script(fn, &pbck_len);
        vm_pop_roots(&vm, 1);
    }

    if (is_web_target) {
        const char *web_dir = out_path ? out_path : "dist_web";
        char w_cand[4160];
        w_cand[0] = '\0';
        if (vm.stdlib_root && *vm.stdlib_root) {
            snprintf(w_cand, sizeof(w_cand), "%.4060s/native/wasm_web_pwa.sapp", vm.stdlib_root);
        }
        if (w_cand[0] && zr_file_exists_reg(w_cand)) {
            ZrCaps wcaps;
            zr_caps_allow_all(&wcaps);
            char *wargs[5];
            int wn = 0;
            wargs[wn++] = (char *)input_path;
            wargs[wn++] = (char *)"--target";
            wargs[wn++] = (char *)"web";
            wargs[wn++] = (char *)"--out";
            wargs[wn++] = (char *)web_dir;
            const char *eval_web =
                "let W = import(\"stdlib:native/wasm_web_pwa.sapp\"); "
                "let r = W.cli_build_web_entry(os_args()); "
                "if not r[\"ok\"] { os_exit(2); } "
                "println(\"Built standalone WebAssembly + PWA bundle in \" + r[\"out_dir\"] + \" (app.wasm: \" + str(r[\"wasm_byte_length\"]) + \" bytes, sha256: \" + str_slice(r[\"wasm_sha256\"], 0, 12) + \")\");";
            int wrc = zr_cmd_eval(self_exe, eval_web, &wcaps, stdlib_override, wn, wargs);
            if (wrc == 0) {
                free(pbck_bytes);
                vm_free(&vm);
                return 0;
            }
        }
        bool wok = zr_emit_web_bundle(input_path, pbck_bytes, pbck_len, web_dir);
        free(pbck_bytes);
        vm_free(&vm);
        return wok ? 0 : 1;
    }

    char default_out[4096];
    if (!out_path) {
        size_t il = strlen(input_path);
        size_t base_len = il;
        if (zr_str_ends_ci(input_path, ".sapp") || zr_str_ends_ci(input_path, ".pbck")) {
            base_len = il - 5;
        }
        if (bundle_exe) {
#ifdef _WIN32
            snprintf(default_out, sizeof(default_out), "%.*s.exe", (int)base_len, input_path);
#else
            snprintf(default_out, sizeof(default_out), "%.*s", (int)base_len, input_path);
            if (strcmp(default_out, input_path) == 0) {
                snprintf(default_out, sizeof(default_out), "%s.bin", input_path);
            }
#endif
        } else {
            snprintf(default_out, sizeof(default_out), "%.*s.pbck", (int)base_len, input_path);
        }
        out_path = default_out;
    }

    if (bundle_exe) {
        if (!self_exe || !*self_exe) {
            fprintf(stderr, "build --bundle-exe: could not locate host executable path\n");
            free(pbck_bytes);
            vm_free(&vm);
            return 1;
        }
        char berr[256];
        berr[0] = '\0';
        if (!zr_write_bundled_exe(self_exe, pbck_bytes, pbck_len, out_path, berr)) {
            fprintf(stderr, "build --bundle-exe failed: %s\n", berr);
            free(pbck_bytes);
            vm_free(&vm);
            return 1;
        }
        if (!ip_disable && vm.stdlib_root && *vm.stdlib_root) {
            char ip_cand[4160];
            snprintf(ip_cand, sizeof(ip_cand), "%.4060s/ip_rights_provenance.sapp", vm.stdlib_root);
            if (zr_file_exists_reg(ip_cand)) {
                ZrCaps icaps;
                zr_caps_allow_all(&icaps);
                const char *mode_val = "proprietary";
                if (ip_mode_flag && strcmp(ip_mode_flag, "--free") == 0) mode_val = "free";
                else if (ip_mode_flag && strcmp(ip_mode_flag, "--free-attribution") == 0) mode_val = "free-attribution";
                else if (ip_mode_flag && strcmp(ip_mode_flag, "--stealth") == 0) mode_val = "stealth-private";
                else if (ip_license) mode_val = "open-source";
                char *iargs[5] = {
                    (char *)out_path,
                    (char *)mode_val,
                    (char *)(ip_license ? ip_license : ""),
                    (char *)(ip_holder ? ip_holder : ""),
                    (char *)(ip_sign_env ? ip_sign_env : "")
                };
                const char *eval_seal =
                    "let A = os_args(); let IP = import(\"stdlib:ip_rights_provenance.sapp\"); "
                    "let opts = map{ \"mode\": A[1] }; "
                    "if len(A[2]) > 0 { opts[\"license\"] = A[2]; } "
                    "if len(A[3]) > 0 { opts[\"holder\"] = A[3]; } "
                    "let pol = IP.build_ip_policy(opts); "
                    "let sec = if len(A[4]) > 0 { env_get(A[4]) } else { nil }; "
                    "let sealed = IP.seal_artifact_bytes(fs_read_bytes(A[0]), \"auto\", pol, sec); "
                    "fs_write_bytes(A[0], sealed);";
                (void)zr_cmd_eval(self_exe, eval_seal, &icaps, stdlib_override, 5, iargs);
#ifndef _WIN32
                chmod(out_path, 0755);
#endif
            }
        }
        printf("Built standalone executable: %s (%zu bytes %s bytecode embedded)\n",
               out_path, pbck_len, use_shield ? "PBSH-v1 shielded" : "PBCK-v1 open");
    } else {
        FILE *fp = fopen(out_path, "wb");
        if (!fp || fwrite(pbck_bytes, 1, pbck_len, fp) != pbck_len) {
            if (fp) fclose(fp);
            fprintf(stderr, "build: failed writing '%s'\n", out_path);
            free(pbck_bytes);
            vm_free(&vm);
            return 1;
        }
        fclose(fp);
        printf("Built bytecode: %s (%zu bytes, %s)\n",
               out_path, pbck_len, use_shield ? "PBSH-v1 shielded" : "PBCK-v1");
    }

    free(pbck_bytes);
    vm_free(&vm);
    return 0;
}

/* ---- `pblang repl` Native Interactive Shell ---- */

static int zr_brace_balance(const char *s) {
    int bal = 0;
    bool in_str = false;
    char q = 0;
    for (size_t i = 0; s[i]; i++) {
        char c = s[i];
        if (in_str) {
            if (c == '\\' && s[i + 1]) { i++; continue; }
            if (c == q) in_str = false;
            continue;
        }
        if (c == '/' && s[i + 1] == '/') {
            while (s[i] && s[i] != '\n') i++;
            continue;
        }
        if (c == '"' || c == '\'') { in_str = true; q = c; continue; }
        if (c == '{' || c == '(' || c == '[') bal++;
        else if (c == '}' || c == ')' || c == ']') bal--;
    }
    return bal;
}

static int zr_cmd_repl(const char *self_exe, int argc, char **argv) {
    const char *stdlib_ov = NULL;
    bool quiet_banner = false;
    for (int i = 0; i < argc; i++) {
        if (strcmp(argv[i], "--stdlib") == 0 && i + 1 < argc) stdlib_ov = argv[++i];
        else if (strcmp(argv[i], "-q") == 0 || strcmp(argv[i], "--quiet") == 0) quiet_banner = true;
    }

    PbVM vm;
    vm_init(&vm);
    zr_caps_allow_all(&vm.caps);
    zr_configure_vm_paths(&vm, self_exe, "<repl>", stdlib_ov);
    zr_populate_polyglot_globals(&vm, vm.globals, NULL);

    if (!quiet_banner) {
        printf("%s — interactive REPL (:help, :gc, :jit, :quit)\n", PBLANG_ZR_VERSION_STR);
    }

    ZrBuf acc;
    zr_buf_init(&acc);
    char line[4096];

    for (;;) {
        if (!quiet_banner) {
            fputs(acc.len == 0 ? "pblang> " : "   ...> ", stdout);
            fflush(stdout);
        }
        if (!fgets(line, sizeof(line), stdin)) break;

        if (acc.len == 0) {
            const char *p = line;
            while (*p == ' ' || *p == '\t') p++;
            if (strncmp(p, ":quit", 5) == 0 || strncmp(p, ":exit", 5) == 0 || (p[0] == ':' && p[1] == 'q' && (p[2] == '\n' || p[2] == '\r' || p[2] == '\0'))) {
                break;
            }
            if (strncmp(p, ":help", 5) == 0) {
                printf("Commands: :help  :gc  :jit  :quit\n");
                continue;
            }
            if (strncmp(p, ":gc", 3) == 0) {
                PbValue st = val_nil();
                if (bi_gc_stats(&vm, 0, NULL, &st) && is_obj_type(st, OBJ_STR)) {
                    printf("%s\n", AS_STR(st)->chars);
                }
                continue;
            }
            if (strncmp(p, ":jit", 4) == 0) {
                PbValue jt = val_nil(), js = val_nil();
                if (bi_jit_stats(&vm, 0, NULL, &jt) && bi_json_stringify(&vm, 1, &jt, &js) && is_obj_type(js, OBJ_STR)) {
                    printf("%s\n", AS_STR(js)->chars);
                }
                continue;
            }
        }

        zr_buf_append_cstr(&acc, line);
        if (zr_brace_balance(acc.data ? acc.data : "") > 0) continue;

        const char *src = acc.data ? acc.data : "";
        const char *tp = src;
        while (*tp == ' ' || *tp == '\t' || *tp == '\r' || *tp == '\n') tp++;
        if (!*tp) {
            acc.len = 0;
            if (acc.data) acc.data[0] = '\0';
            continue;
        }

        /* Try compiling as expression `return (<src>);` first unless it starts with a statement keyword */
        bool is_stmt_kw =
            strncmp(tp, "let ", 4) == 0 || strncmp(tp, "const ", 6) == 0 ||
            strncmp(tp, "var ", 4) == 0 || strncmp(tp, "fn ", 3) == 0 ||
            strncmp(tp, "if ", 3) == 0 || strncmp(tp, "if(", 3) == 0 ||
            strncmp(tp, "while ", 6) == 0 || strncmp(tp, "for ", 4) == 0 ||
            strncmp(tp, "class ", 6) == 0 || strncmp(tp, "struct ", 7) == 0 ||
            strncmp(tp, "print ", 6) == 0 || strncmp(tp, "print(", 6) == 0 ||
            strncmp(tp, "import ", 7) == 0 || strncmp(tp, "return ", 7) == 0 ||
            strncmp(tp, "try ", 4) == 0 || strncmp(tp, "try{", 4) == 0;

        PbFunc *fn = NULL;
        bool expr_mode = false;
        char cerr[512];
        cerr[0] = '\0';

        if (!is_stmt_kw) {
            ZrBuf wrap;
            zr_buf_init(&wrap);
            zr_buf_append_cstr(&wrap, "return (\n");
            /* Strip trailing semicolon if single expression */
            size_t sl = strlen(src);
            while (sl > 0 && (src[sl - 1] == '\n' || src[sl - 1] == '\r' || src[sl - 1] == ' ' || src[sl - 1] == '\t')) sl--;
            if (sl > 0 && src[sl - 1] == ';') sl--;
            zr_buf_append(&wrap, src, sl);
            zr_buf_append_cstr(&wrap, "\n);");
            fn = zr_compile_sapp(&vm, wrap.data, wrap.len, "<repl>", cerr);
            zr_buf_free(&wrap);
            if (fn) expr_mode = true;
        }
        if (!fn) {
            cerr[0] = '\0';
            fn = zr_compile_sapp(&vm, src, strlen(src), "<repl>", cerr);
        }

        if (!fn) {
            fprintf(stderr, "Compile error: %s\n", cerr[0] ? cerr : "syntax error");
        } else {
            vm.err_msg[0] = '\0';
            vm.exited = false;
            vm_push_root(&vm, val_obj(fn));
            if (fn->poly_count > 0) zr_populate_polyglot_globals(&vm, vm.globals, fn);
            PbValue ret = val_nil();
            bool ok = vm_run_root_func(&vm, fn, NULL, &ret);
            vm_pop_roots(&vm, 1);
            if (!ok) {
                fprintf(stderr, "Runtime error: %s\n", vm.err_msg[0] ? vm.err_msg : "execution failed");
            } else if (expr_mode && ret.tag != VAL_NIL) {
                size_t rlen = 0;
                char *rs = vm_value_to_cstr(&vm, ret, &rlen);
                if (rs) {
                    fwrite(rs, 1, rlen, stdout);
                    fputc('\n', stdout);
                    free(rs);
                }
            }
            if (vm.exited) break;
        }

        acc.len = 0;
        if (acc.data) acc.data[0] = '\0';
    }

    zr_buf_free(&acc);
    vm_free(&vm);
    return 0;
}

/* ---- `pblang fmt [--write] [--check] <files...>` ---- */

static char *zr_format_sapp_source(const char *src, size_t len, size_t *out_len) {
    ZrBuf out;
    zr_buf_init(&out);
    int indent = 0;
    int blank_streak = 0;
    bool in_block_comment = false;

    size_t pos = 0;
    while (pos < len) {
        size_t line_start = pos;
        while (pos < len && src[pos] != '\n') pos++;
        size_t line_end = pos;
        if (pos < len && src[pos] == '\n') pos++;
        while (line_end > line_start && (src[line_end - 1] == '\r' || src[line_end - 1] == ' ' || src[line_end - 1] == '\t')) {
            line_end--;
        }

        if (in_block_comment) {
            zr_buf_append(&out, src + line_start, line_end - line_start);
            zr_buf_push_u8(&out, '\n');
            for (size_t k = line_start; k + 1 < line_end; k++) {
                if (src[k] == '*' && src[k + 1] == '/') { in_block_comment = false; k++; }
            }
            continue;
        }

        size_t trimmed = line_start;
        while (trimmed < line_end && (src[trimmed] == ' ' || src[trimmed] == '\t')) trimmed++;

        if (trimmed == line_end) {
            blank_streak++;
            if (blank_streak <= 1 && out.len > 0) zr_buf_push_u8(&out, '\n');
            continue;
        }
        blank_streak = 0;

        /* Count leading closing delimiters to dedent current line */
        int lead_dedent = 0;
        size_t p = trimmed;
        while (p < line_end && (src[p] == '}' || src[p] == ']' || src[p] == ')')) {
            lead_dedent++;
            p++;
        }
        int cur_indent = indent - lead_dedent;
        if (cur_indent < 0) cur_indent = 0;

        for (int sp = 0; sp < cur_indent * 4; sp++) zr_buf_push_u8(&out, ' ');
        zr_buf_append(&out, src + trimmed, line_end - trimmed);
        zr_buf_push_u8(&out, '\n');

        /* Scan net brace/bracket delta across line ignoring strings & comments */
        int net = 0;
        bool in_str = false;
        char q = 0;
        for (size_t k = trimmed; k < line_end; k++) {
            char c = src[k];
            if (in_str) {
                if (c == '\\' && k + 1 < line_end) { k++; continue; }
                if (c == q) in_str = false;
                continue;
            }
            if (c == '/' && k + 1 < line_end && src[k + 1] == '/') break;
            if (c == '/' && k + 1 < line_end && src[k + 1] == '*') {
                k += 2;
                bool closed = false;
                while (k + 1 < line_end) {
                    if (src[k] == '*' && src[k + 1] == '/') { closed = true; k++; break; }
                    k++;
                }
                if (!closed) { in_block_comment = true; break; }
                continue;
            }
            if (c == '"' || c == '\'') { in_str = true; q = c; continue; }
            if (c == '{' || c == '[' || c == '(') net++;
            else if (c == '}' || c == ']' || c == ')') net--;
        }
        indent += net;
        if (indent < 0) indent = 0;
    }

    return zr_buf_take(&out, out_len);
}

static int zr_cmd_fmt(int argc, char **argv) {
    bool write_in_place = false;
    bool check_only = false;
    int files_seen = 0;
    int unformatted = 0;

    for (int i = 0; i < argc; i++) {
        if (strcmp(argv[i], "--write") == 0 || strcmp(argv[i], "-w") == 0) write_in_place = true;
        else if (strcmp(argv[i], "--check") == 0) check_only = true;
    }

    for (int i = 0; i < argc; i++) {
        if (argv[i][0] == '-') continue;
        files_seen++;
        uint8_t *raw = NULL;
        size_t raw_len = 0;
        if (!zr_read_entire_file(argv[i], &raw, &raw_len)) {
            fprintf(stderr, "fmt: cannot read '%s'\n", argv[i]);
            return 1;
        }
        size_t flen = 0;
        char *formatted = zr_format_sapp_source((const char *)raw, raw_len, &flen);
        bool same = (flen == raw_len && memcmp(raw, formatted, flen) == 0);
        if (check_only) {
            if (!same) {
                fprintf(stderr, "fmt --check: '%s' needs formatting\n", argv[i]);
                unformatted++;
            }
        } else if (write_in_place) {
            if (!same) {
                FILE *fp = fopen(argv[i], "wb");
                if (!fp || fwrite(formatted, 1, flen, fp) != flen) {
                    if (fp) fclose(fp);
                    fprintf(stderr, "fmt: cannot write '%s'\n", argv[i]);
                    free(raw);
                    free(formatted);
                    return 1;
                }
                fclose(fp);
            }
            printf("Formatted %s\n", argv[i]);
        } else {
            fwrite(formatted, 1, flen, stdout);
        }
        free(raw);
        free(formatted);
    }

    if (files_seen == 0) {
        fprintf(stderr, "Usage: pblang fmt [--write] [--check] <file.sapp...>\n");
        return 1;
    }
    return unformatted > 0 ? 1 : 0;
}

/* ---- `pblang doc [-o <out.md>] <file.sapp>` ---- */

static int zr_cmd_doc(int argc, char **argv) {
    const char *input = NULL;
    const char *out_path = NULL;
    for (int i = 0; i < argc; i++) {
        if ((strcmp(argv[i], "-o") == 0 || strcmp(argv[i], "--out") == 0) && i + 1 < argc) {
            out_path = argv[++i];
        } else if (argv[i][0] != '-' && !input) {
            input = argv[i];
        }
    }
    if (!input) {
        fprintf(stderr, "Usage: pblang doc [-o <out.md>] <file.sapp>\n");
        return 1;
    }
    uint8_t *raw = NULL;
    size_t raw_len = 0;
    if (!zr_read_entire_file(input, &raw, &raw_len)) {
        fprintf(stderr, "doc: cannot read '%s'\n", input);
        return 1;
    }

    const char *base = input;
    for (const char *p = input; *p; p++) {
        if (*p == '/' || *p == '\\') base = p + 1;
    }

    ZrBuf md;
    zr_buf_init(&md);
    zr_buf_printf(&md, "# Module `%s`\n\nGenerated by `%s`.\n\n## Declarations\n\n", base, PBLANG_ZR_VERSION_STR);

    ZrBuf doc_acc;
    zr_buf_init(&doc_acc);
    size_t pos = 0;
    size_t items = 0;
    const char *src = (const char *)raw;

    while (pos < raw_len) {
        size_t ls = pos;
        while (pos < raw_len && src[pos] != '\n') pos++;
        size_t le = pos;
        if (pos < raw_len && src[pos] == '\n') pos++;
        while (le > ls && (src[le - 1] == '\r' || src[le - 1] == ' ' || src[le - 1] == '\t')) le--;
        size_t ts = ls;
        while (ts < le && (src[ts] == ' ' || src[ts] == '\t')) ts++;

        if (ts == le) {
            doc_acc.len = 0;
            if (doc_acc.data) doc_acc.data[0] = '\0';
            continue;
        }
        if (ts + 2 <= le && src[ts] == '/' && src[ts + 1] == '/') {
            size_t cs = ts + 2;
            if (cs < le && src[cs] == '/') cs++;
            if (cs < le && src[cs] == ' ') cs++;
            zr_buf_append(&doc_acc, src + cs, le - cs);
            zr_buf_push_u8(&doc_acc, '\n');
            continue;
        }

        size_t decl_s = ts;
        bool exported = false;
        if (le - decl_s >= 7 && strncmp(src + decl_s, "export ", 7) == 0) {
            exported = true;
            decl_s += 7;
            while (decl_s < le && src[decl_s] == ' ') decl_s++;
        }
        if (le - decl_s >= 6 && strncmp(src + decl_s, "async ", 6) == 0) {
            decl_s += 6;
            while (decl_s < le && src[decl_s] == ' ') decl_s++;
        }

        bool is_decl =
            (le - decl_s >= 3 && strncmp(src + decl_s, "fn ", 3) == 0) ||
            (le - decl_s >= 6 && strncmp(src + decl_s, "class ", 6) == 0) ||
            (le - decl_s >= 7 && strncmp(src + decl_s, "struct ", 7) == 0) ||
            (le - decl_s >= 5 && strncmp(src + decl_s, "enum ", 5) == 0) ||
            (exported && le - decl_s >= 4 && (strncmp(src + decl_s, "let ", 4) == 0 || strncmp(src + decl_s, "const ", 6) == 0));

        if (is_decl) {
            /* Capture header up to `{` across continuation lines (captures `requires`/`ensures` contracts!) */
            size_t sig_end = le;
            bool found_brace = false;
            for (size_t k = ts; k < le; k++) {
                if (src[k] == '{') { sig_end = k; found_brace = true; break; }
            }
            ZrBuf sig;
            zr_buf_init(&sig);
            while (sig_end > ts && (src[sig_end - 1] == ' ' || src[sig_end - 1] == '\t')) sig_end--;
            zr_buf_append(&sig, src + ts, sig_end - ts);

            while (!found_brace && pos < raw_len) {
                size_t nls = pos;
                while (pos < raw_len && src[pos] != '\n') pos++;
                size_t nle = pos;
                if (pos < raw_len && src[pos] == '\n') pos++;
                size_t nts = nls;
                while (nts < nle && (src[nts] == ' ' || src[nts] == '\t')) nts++;
                if (nts == nle) break;
                size_t nend = nle;
                for (size_t k = nts; k < nle; k++) {
                    if (src[k] == '{') { nend = k; found_brace = true; break; }
                    if (src[k] == ';') { nend = k; found_brace = true; break; }
                }
                while (nend > nts && (src[nend - 1] == ' ' || src[nend - 1] == '\r' || src[nend - 1] == '\t')) nend--;
                if (nend > nts) {
                    zr_buf_append_cstr(&sig, "\n  ");
                    zr_buf_append(&sig, src + nts, nend - nts);
                }
            }

            zr_buf_printf(&md, "### `%.*s`\n\n```pblang\n%.*s\n```\n\n",
                          (int)(sig_end - ts), src + ts,
                          (int)sig.len, sig.data ? sig.data : "");
            if (doc_acc.len > 0 && doc_acc.data) {
                zr_buf_append(&md, doc_acc.data, doc_acc.len);
                zr_buf_push_u8(&md, '\n');
            }
            zr_buf_free(&sig);
            items++;
        }

        doc_acc.len = 0;
        if (doc_acc.data) doc_acc.data[0] = '\0';
    }

    if (items == 0) {
        zr_buf_append_cstr(&md, "_No top-level declarations found._\n");
    }

    int rc = 0;
    if (out_path) {
        FILE *fp = fopen(out_path, "wb");
        if (!fp || fwrite(md.data, 1, md.len, fp) != md.len) {
            if (fp) fclose(fp);
            fprintf(stderr, "doc: failed writing '%s'\n", out_path);
            rc = 1;
        } else {
            fclose(fp);
            printf("Generated documentation: %s (%zu items)\n", out_path, items);
        }
    } else {
        fwrite(md.data, 1, md.len, stdout);
    }

    zr_buf_free(&doc_acc);
    zr_buf_free(&md);
    free(raw);
    return rc;
}

/* ---- `pblang lsp` Native Stdio JSON-RPC 2.0 Language Server ---- */

static bool zr_lsp_send(const char *json_payload) {
    size_t len = strlen(json_payload);
    if (fprintf(stdout, "Content-Length: %zu\r\n\r\n", len) < 0) return false;
    if (len > 0 && fwrite(json_payload, 1, len, stdout) != len) return false;
    if (fflush(stdout) != 0 || ferror(stdout)) return false;
    return true;
}

static void zr_lsp_json_escape(ZrBuf *b, const char *s) {
    zr_buf_push_u8(b, '"');
    for (size_t i = 0; s && s[i]; i++) {
        unsigned char c = (unsigned char)s[i];
        switch (c) {
            case '"':  zr_buf_append_cstr(b, "\\\""); break;
            case '\\': zr_buf_append_cstr(b, "\\\\"); break;
            case '\n': zr_buf_append_cstr(b, "\\n"); break;
            case '\r': zr_buf_append_cstr(b, "\\r"); break;
            case '\t': zr_buf_append_cstr(b, "\\t"); break;
            default:
                if (c >= 0x20) zr_buf_push_u8(b, c);
                break;
        }
    }
    zr_buf_push_u8(b, '"');
}

static void zr_lsp_publish_diagnostics(PbVM *vm, const char *uri, const char *text) {
    PbVM cvm;
    vm_init(&cvm);
    zr_caps_init_default(&cvm.caps);
    if (vm->stdlib_root) cvm.stdlib_root = zr_xstrdup(vm->stdlib_root);

    char cerr[512];
    cerr[0] = '\0';
    PbFunc *fn = zr_compile_sapp(&cvm, text, strlen(text), uri ? uri : "<lsp>", cerr);

    ZrBuf msg;
    zr_buf_init(&msg);
    zr_buf_append_cstr(&msg, "{\"jsonrpc\":\"2.0\",\"method\":\"textDocument/publishDiagnostics\",\"params\":{\"uri\":");
    zr_lsp_json_escape(&msg, uri ? uri : "");
    zr_buf_append_cstr(&msg, ",\"diagnostics\":[");

    if (!fn && cerr[0]) {
        int line0 = 0;
        const char *lp = strstr(cerr, "line ");
        if (lp) {
            int ln = atoi(lp + 5);
            if (ln > 0) line0 = ln - 1;
        }
        zr_buf_printf(&msg,
            "{\"range\":{\"start\":{\"line\":%d,\"character\":0},\"end\":{\"line\":%d,\"character\":80}},\"severity\":1,\"source\":\"pblang\",\"message\":",
            line0, line0);
        zr_lsp_json_escape(&msg, cerr);
        zr_buf_push_u8(&msg, '}');
    }

    zr_buf_append_cstr(&msg, "]}}");
    (void)zr_lsp_send(msg.data ? msg.data : "{}");
    zr_buf_free(&msg);
    vm_free(&cvm);
}

static bool zr_mod_get_export(PbValue mod_val, const char *name, PbValue *out) {
    if (is_obj_type(mod_val, OBJ_MODULE)) {
        PbModule *m = (PbModule *)val_as_obj(mod_val);
        if (m->exports && pb_map_get_cstr(m->exports, name, out)) return true;
        if (m->globals && pb_map_get_cstr(m->globals, name, out)) return true;
    } else if (is_obj_type(mod_val, OBJ_MAP)) {
        if (pb_map_get_cstr(AS_MAP(mod_val), name, out)) return true;
    }
    return false;
}

static int zr_cmd_lsp(const char *self_exe) {
#ifdef _WIN32
    _setmode(_fileno(stdin), _O_BINARY);
    _setmode(_fileno(stdout), _O_BINARY);
#elif defined(SIGPIPE)
    signal(SIGPIPE, SIG_IGN);
#endif
    PbVM vm;
    vm_init(&vm);
    zr_caps_allow_all(&vm.caps);
    zr_configure_vm_paths(&vm, self_exe, NULL, NULL);

    /* Attempt pure-PBLang v4.2 Workspace LSP engine (`stdlib:devtools/workspace_lsp_v42.sapp`) */
    bool has_pure_lsp = false;
    PbValue lsp_mod = val_nil(), lsp_create_fn = val_nil(), lsp_handle_fn = val_nil(), lsp_state = val_nil();
    if (zr_vm_import_module_caps(&vm, "stdlib:devtools/workspace_lsp_v42", NULL, CAP_ALL, &lsp_mod)) {
        vm_push_root(&vm, lsp_mod);
        if (zr_mod_get_export(lsp_mod, "create_workspace_lsp", &lsp_create_fn) &&
            zr_mod_get_export(lsp_mod, "lsp_handle_message", &lsp_handle_fn)) {
            vm_push_root(&vm, lsp_handle_fn);
            if (vm_call_value_sync(&vm, lsp_create_fn, 0, NULL, &lsp_state) && is_obj_type(lsp_state, OBJ_MAP)) {
                vm_push_root(&vm, lsp_state);
                has_pure_lsp = true;
            } else {
                vm_pop_roots(&vm, 2);
                vm.err_msg[0] = '\0';
            }
        } else {
            vm_pop_roots(&vm, 1);
            vm.err_msg[0] = '\0';
        }
    } else {
        vm.err_msg[0] = '\0';
    }

    char hdr[512];
    bool shutdown_seen = false;

    for (;;) {
        size_t content_len = 0;
        bool got_header = false;
        while (fgets(hdr, sizeof(hdr), stdin)) {
            got_header = true;
            if (strcmp(hdr, "\r\n") == 0 || strcmp(hdr, "\n") == 0) break;
            if (zr_strncasecmp_ascii(hdr, "Content-Length:", 15) == 0) {
                content_len = (size_t)strtoull(hdr + 15, NULL, 10);
            }
        }
        if (!got_header || content_len == 0 || content_len > (32u * 1024u * 1024u)) break;

        char *body = (char *)zr_xmalloc(content_len + 1);
        size_t rd = fread(body, 1, content_len, stdin);
        body[rd] = '\0';
        if (rd < content_len) { free(body); break; }

        PbValue req = val_nil();
        PbValue sarg = val_obj(pb_str_new(&vm, body, rd));
        vm_push_root(&vm, sarg);
        bool pok = bi_json_parse(&vm, 1, &sarg, &req);
        vm_pop_roots(&vm, 1);
        free(body);
        if (!pok || !is_obj_type(req, OBJ_MAP)) {
            vm.err_msg[0] = '\0';
            continue;
        }

        vm_push_root(&vm, req);
        PbMap *rm = AS_MAP(req);
        PbValue method_v = val_nil(), id_v = val_nil(), params_v = val_nil();
        pb_map_get_cstr(rm, "method", &method_v);
        bool has_id = pb_map_get_cstr(rm, "id", &id_v);
        pb_map_get_cstr(rm, "params", &params_v);
        const char *method = is_obj_type(method_v, OBJ_STR) ? AS_STR(method_v)->chars : "";

        if (has_pure_lsp) {
            bool is_exit = (strcmp(method, "exit") == 0);
            bool pipe_ok = true;
            if (strcmp(method, "shutdown") == 0) shutdown_seen = true;
            PbValue call_args[2] = { lsp_state, req };
            PbValue resp_val = val_nil();
            vm.err_msg[0] = '\0';
            if (vm_call_value_sync(&vm, lsp_handle_fn, 2, call_args, &resp_val)) {
                if (resp_val.tag != VAL_NIL) {
                    vm_push_root(&vm, resp_val);
                    PbValue jstr = val_nil();
                    if (bi_json_stringify(&vm, 1, &resp_val, &jstr) && is_obj_type(jstr, OBJ_STR)) {
                        pipe_ok = zr_lsp_send(AS_STR(jstr)->chars);
                    }
                    vm_pop_roots(&vm, 1);
                }
                PbValue out_notifs = val_nil();
                if (pb_map_get_cstr(AS_MAP(lsp_state), "outgoing_notifications", &out_notifs) &&
                    is_obj_type(out_notifs, OBJ_ARRAY)) {
                    AS_ARRAY(out_notifs)->len = 0;
                }
            } else {
                vm.err_msg[0] = '\0';
            }
            vm_pop_roots(&vm, 1); /* req */
            if (is_exit || !pipe_ok) break;
            continue;
        }

        ZrBuf id_json;
        zr_buf_init(&id_json);
        if (has_id) {
            if (id_v.tag == VAL_NUM) zr_buf_printf(&id_json, "%.0f", id_v.as.num);
            else if (is_obj_type(id_v, OBJ_STR)) zr_lsp_json_escape(&id_json, AS_STR(id_v)->chars);
            else zr_buf_append_cstr(&id_json, "null");
        }

        if (strcmp(method, "initialize") == 0 && has_id) {
            ZrBuf resp;
            zr_buf_init(&resp);
            zr_buf_printf(&resp,
                "{\"jsonrpc\":\"2.0\",\"id\":%s,\"result\":{\"capabilities\":{"
                "\"textDocumentSync\":1,"
                "\"definitionProvider\":true,"
                "\"referencesProvider\":true,"
                "\"renameProvider\":{\"prepareProvider\":true},"
                "\"hoverProvider\":true,"
                "\"documentSymbolProvider\":true,"
                "\"inlayHintProvider\":true,"
                "\"signatureHelpProvider\":{\"triggerCharacters\":[\"(\",\",\"]},"
                "\"completionProvider\":{\"triggerCharacters\":[\".\",\":\",\"@\",\"\\\"\"]}"
                "},\"serverInfo\":{\"name\":\"pblang-workspace-lsp\",\"version\":\"%s\"}}}",
                id_json.data ? id_json.data : "0", PBLANG_ZR_VERSION);
            zr_lsp_send(resp.data);
            zr_buf_free(&resp);
        } else if (strcmp(method, "shutdown") == 0 && has_id) {
            shutdown_seen = true;
            ZrBuf resp;
            zr_buf_init(&resp);
            zr_buf_printf(&resp, "{\"jsonrpc\":\"2.0\",\"id\":%s,\"result\":null}",
                          id_json.data ? id_json.data : "0");
            zr_lsp_send(resp.data);
            zr_buf_free(&resp);
        } else if (strcmp(method, "exit") == 0) {
            zr_buf_free(&id_json);
            vm_pop_roots(&vm, 1);
            break;
        } else if (strcmp(method, "textDocument/didOpen") == 0 && is_obj_type(params_v, OBJ_MAP)) {
            PbValue td = val_nil();
            if (pb_map_get_cstr(AS_MAP(params_v), "textDocument", &td) && is_obj_type(td, OBJ_MAP)) {
                PbValue uri_v = val_nil(), txt_v = val_nil();
                pb_map_get_cstr(AS_MAP(td), "uri", &uri_v);
                pb_map_get_cstr(AS_MAP(td), "text", &txt_v);
                if (is_obj_type(uri_v, OBJ_STR) && is_obj_type(txt_v, OBJ_STR)) {
                    zr_lsp_publish_diagnostics(&vm, AS_STR(uri_v)->chars, AS_STR(txt_v)->chars);
                }
            }
        } else if (strcmp(method, "textDocument/didChange") == 0 && is_obj_type(params_v, OBJ_MAP)) {
            PbValue td = val_nil(), ch = val_nil();
            if (pb_map_get_cstr(AS_MAP(params_v), "textDocument", &td) && is_obj_type(td, OBJ_MAP) &&
                pb_map_get_cstr(AS_MAP(params_v), "contentChanges", &ch) && is_obj_type(ch, OBJ_ARRAY) &&
                AS_ARRAY(ch)->len > 0) {
                PbValue uri_v = val_nil(), txt_v = val_nil();
                pb_map_get_cstr(AS_MAP(td), "uri", &uri_v);
                PbValue last_ch = AS_ARRAY(ch)->items[AS_ARRAY(ch)->len - 1];
                if (is_obj_type(last_ch, OBJ_MAP)) pb_map_get_cstr(AS_MAP(last_ch), "text", &txt_v);
                if (is_obj_type(uri_v, OBJ_STR) && is_obj_type(txt_v, OBJ_STR)) {
                    zr_lsp_publish_diagnostics(&vm, AS_STR(uri_v)->chars, AS_STR(txt_v)->chars);
                }
            }
        } else if (strcmp(method, "textDocument/completion") == 0 && has_id) {
            static const char *kws[] = {
                "fn", "let", "const", "if", "else", "while", "for", "in", "return",
                "import", "with", "caps", "requires", "ensures", "comptime",
                "try", "catch", "defer", "class", "struct", "enum", "trait", "interface", "match", "spawn"
            };
            ZrBuf resp;
            zr_buf_init(&resp);
            zr_buf_printf(&resp, "{\"jsonrpc\":\"2.0\",\"id\":%s,\"result\":[",
                          id_json.data ? id_json.data : "0");
            bool first = true;
            for (size_t k = 0; k < sizeof(kws) / sizeof(kws[0]); k++) {
                if (!first) zr_buf_push_u8(&resp, ',');
                first = false;
                zr_buf_printf(&resp, "{\"label\":\"%s\",\"kind\":14}", kws[k]);
            }
            for (uint16_t b = 0; b < vm.builtin_count; b++) {
                if (!vm.builtin_defs[b].name) continue;
                zr_buf_push_u8(&resp, ',');
                zr_buf_printf(&resp, "{\"label\":\"%s\",\"kind\":3}", vm.builtin_defs[b].name);
            }
            zr_buf_append_cstr(&resp, "]}");
            zr_lsp_send(resp.data);
            zr_buf_free(&resp);
        } else if (strcmp(method, "textDocument/hover") == 0 && has_id) {
            ZrBuf resp;
            zr_buf_init(&resp);
            zr_buf_printf(&resp,
                "{\"jsonrpc\":\"2.0\",\"id\":%s,\"result\":{\"contents\":{\"kind\":\"markdown\",\"value\":\"**PBLang v%s** Sovereign Native Runtime (%u builtins)\"}}}",
                id_json.data ? id_json.data : "0", PBLANG_ZR_VERSION, (unsigned)vm.builtin_count);
            zr_lsp_send(resp.data);
            zr_buf_free(&resp);
        } else if (has_id) {
            ZrBuf resp;
            zr_buf_init(&resp);
            zr_buf_printf(&resp, "{\"jsonrpc\":\"2.0\",\"id\":%s,\"result\":null}",
                          id_json.data ? id_json.data : "0");
            zr_lsp_send(resp.data);
            zr_buf_free(&resp);
        }

        zr_buf_free(&id_json);
        vm_pop_roots(&vm, 1);
    }

    if (has_pure_lsp) vm_pop_roots(&vm, 3);
    vm_free(&vm);
    return shutdown_seen ? 0 : 0;
}

/* ---- `pblang debug <file.sapp> [args...]` Interactive CLI Debugger ---- */

static int zr_cmd_debug(const char *self_exe, int argc, char **argv) {
    const char *target = NULL;
    const char *stdlib_ov = NULL;
    int script_start = argc;
    for (int i = 0; i < argc; i++) {
        if (strcmp(argv[i], "--stdlib") == 0 && i + 1 < argc) stdlib_ov = argv[++i];
        else if (argv[i][0] != '-' && !target) { target = argv[i]; script_start = i; break; }
    }
    if (!target) {
        fprintf(stderr, "Usage: pblang debug <file.sapp> [args...]\n");
        return 1;
    }

    PbVM vm;
    vm_init(&vm);
    zr_caps_allow_all(&vm.caps);
    zr_configure_vm_paths(&vm, self_exe, target, stdlib_ov);
    vm.cli_argc = argc - script_start;
    vm.cli_argv = argv + script_start;

    char lerr[512];
    lerr[0] = '\0';
    PbFunc *fn = zr_load_file_pbsh_aware(&vm, vm.current_file ? vm.current_file : target, lerr);
    if (!fn) {
        fprintf(stderr, "debug: %s\n", lerr[0] ? lerr : "compile failed");
        vm_free(&vm);
        return 1;
    }
    vm_push_root(&vm, val_obj(fn));
    zr_populate_polyglot_globals(&vm, vm.globals, fn);

    printf("PBLang v%s Debugger — loaded %s (%u instructions, %u constants)\n",
           PBLANG_ZR_VERSION, target, fn->code_len, fn->const_len);
    printf("Type 'run' (r), 'disasm' (l), 'globals', 'print <expr>', 'break <line>', or 'quit' (q).\n");

    uint32_t bps[64];
    size_t bp_cnt = 0;
    bool has_run = false;
    char cmd[1024];

    for (;;) {
        fputs("(pbdb) ", stdout);
        fflush(stdout);
        if (!fgets(cmd, sizeof(cmd), stdin)) {
            if (!has_run) {
                PbValue ret = val_nil();
                if (!vm_run_root_func(&vm, fn, NULL, &ret) && !vm.exited) {
                    fprintf(stderr, "Runtime error: %s\n", vm.err_msg);
                }
            }
            break;
        }
        char *p = cmd;
        while (*p == ' ' || *p == '\t') p++;
        size_t cl = strlen(p);
        while (cl > 0 && (p[cl - 1] == '\n' || p[cl - 1] == '\r' || p[cl - 1] == ' ')) p[--cl] = '\0';
        if (cl == 0) continue;

        if (strcmp(p, "q") == 0 || strcmp(p, "quit") == 0 || strcmp(p, "exit") == 0) {
            break;
        } else if (strcmp(p, "h") == 0 || strcmp(p, "help") == 0) {
            printf("Commands: run (r), continue (c), disasm (l), break <line> (b), globals, bt, print <expr> (p), quit (q)\n");
        } else if (strncmp(p, "b ", 2) == 0 || strncmp(p, "break ", 6) == 0) {
            const char *arg = (p[1] == ' ') ? p + 2 : p + 6;
            uint32_t ln = (uint32_t)strtoul(arg, NULL, 10);
            if (ln > 0 && bp_cnt < 64) {
                bps[bp_cnt++] = ln;
                printf("Breakpoint #%zu set at %s:%u\n", bp_cnt, target, ln);
            }
        } else if (strcmp(p, "l") == 0 || strcmp(p, "list") == 0 || strcmp(p, "disasm") == 0) {
            uint32_t lim = fn->code_len < 64 ? fn->code_len : 64;
            for (uint32_t i = 0; i < lim; i++) {
                char mark = ' ';
                for (size_t bi = 0; bi < bp_cnt; bi++) {
                    if (bps[bi] == fn->code[i].line) { mark = '*'; break; }
                }
                printf(" %c[%04u] line %-4u op=%-3u (orig=%-2u) arg=%u\n",
                       mark, i, fn->code[i].line, (unsigned)fn->code[i].op,
                       (unsigned)fn->code[i].orig_op, fn->code[i].arg);
            }
        } else if (strcmp(p, "r") == 0 || strcmp(p, "run") == 0 || strcmp(p, "c") == 0 || strcmp(p, "continue") == 0 ||
                   strcmp(p, "s") == 0 || strcmp(p, "step") == 0 || strcmp(p, "n") == 0 || strcmp(p, "next") == 0) {
            has_run = true;
            vm.err_msg[0] = '\0';
            vm.exited = false;
            PbValue ret = val_nil();
            bool ok = vm_run_root_func(&vm, fn, NULL, &ret);
            if (!ok && !vm.exited) {
                printf("Stopped with runtime error: %s\n", vm.err_msg);
            } else {
                printf("Program finished (return=%s)\n", ret.tag == VAL_NIL ? "nil" : "value");
            }
        } else if (strcmp(p, "globals") == 0 || strcmp(p, "locals") == 0) {
            for (size_t i = 0; i < vm.globals->used; i++) {
                if (!vm.globals->entries[i].active || !vm.globals->entries[i].key) continue;
                size_t vl = 0;
                char *vs = vm_value_to_cstr(&vm, vm.globals->entries[i].val, &vl);
                printf("  %s = %s\n", vm.globals->entries[i].key->chars, vs ? vs : "nil");
                free(vs);
            }
        } else if (strcmp(p, "bt") == 0) {
            printf("  #0 <root> at %s (%zu active frames)\n", target, vm.frame_count);
        } else if (strncmp(p, "p ", 2) == 0 || strncmp(p, "print ", 6) == 0 || strncmp(p, "eval ", 5) == 0) {
            const char *expr = strchr(p, ' ') + 1;
            ZrBuf wrap;
            zr_buf_init(&wrap);
            zr_buf_printf(&wrap, "return (%s);", expr);
            char cerr[512];
            cerr[0] = '\0';
            PbFunc *efn = zr_compile_sapp(&vm, wrap.data, wrap.len, "<pbdb>", cerr);
            zr_buf_free(&wrap);
            if (!efn) {
                printf("Eval compile error: %s\n", cerr);
            } else {
                vm_push_root(&vm, val_obj(efn));
                PbValue eret = val_nil();
                if (vm_run_root_func(&vm, efn, NULL, &eret)) {
                    size_t rl = 0;
                    char *rs = vm_value_to_cstr(&vm, eret, &rl);
                    printf("$ = %s\n", rs ? rs : "nil");
                    free(rs);
                } else {
                    printf("Eval runtime error: %s\n", vm.err_msg);
                    vm.err_msg[0] = '\0';
                }
                vm_pop_roots(&vm, 1);
            }
        } else {
            printf("Unknown command '%s' (type 'help')\n", p);
        }
    }

    vm_pop_roots(&vm, 1);
    vm_free(&vm);
    return 0;
}

/* ---- `pblang test` Suite Discovery & Runner ---- */

typedef struct {
    char  **paths;
    size_t  len;
    size_t  cap;
} ZrTestFileList;

static void zr_tlist_push(ZrTestFileList *l, const char *p) {
    if (l->len >= l->cap) {
        l->cap = l->cap ? l->cap * 2 : 32;
        l->paths = (char **)zr_xrealloc(l->paths, l->cap * sizeof(char *));
    }
    l->paths[l->len++] = zr_xstrdup(p);
}

static int zr_cmp_cstr_ptr(const void *a, const void *b) {
    return strcmp(*(const char *const *)a, *(const char *const *)b);
}

static void zr_collect_tests_recursive(const char *path, ZrTestFileList *list) {
    if (zr_file_exists_reg(path)) {
        if (zr_str_ends_ci(path, ".sapp") || zr_str_ends_ci(path, ".pbck")) {
            zr_tlist_push(list, path);
        }
        return;
    }
    if (!zr_dir_exists(path)) return;
#ifdef _WIN32
    char pat[4096];
    snprintf(pat, sizeof(pat), "%s\\*", path);
    WIN32_FIND_DATAA fd;
    HANDLE h = FindFirstFileA(pat, &fd);
    if (h == INVALID_HANDLE_VALUE) return;
    do {
        if (strcmp(fd.cFileName, ".") == 0 || strcmp(fd.cFileName, "..") == 0 || fd.cFileName[0] == '.') continue;
        char child[4096];
        snprintf(child, sizeof(child), "%s\\%s", path, fd.cFileName);
        if (fd.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY) {
            zr_collect_tests_recursive(child, list);
        } else if (zr_str_ends_ci(fd.cFileName, "_test.sapp") || zr_str_ends_ci(fd.cFileName, ".test.sapp")) {
            zr_tlist_push(list, child);
        }
    } while (FindNextFileA(h, &fd));
    FindClose(h);
#else
    DIR *d = opendir(path);
    if (!d) return;
    struct dirent *de;
    while ((de = readdir(d)) != NULL) {
        if (strcmp(de->d_name, ".") == 0 || strcmp(de->d_name, "..") == 0 || de->d_name[0] == '.') continue;
        char child[4096];
        snprintf(child, sizeof(child), "%s/%s", path, de->d_name);
        if (zr_dir_exists(child)) {
            zr_collect_tests_recursive(child, list);
        } else if (zr_str_ends_ci(de->d_name, "_test.sapp") || zr_str_ends_ci(de->d_name, ".test.sapp")) {
            zr_tlist_push(list, child);
        }
    }
    closedir(d);
#endif
}

static bool zr_root_chunk_calls_global(const PbFunc *root_fn, const char *name) {
    for (uint32_t i = 0; i < root_fn->code_len; i++) {
        uint8_t op = root_fn->code[i].orig_op ? root_fn->code[i].orig_op : root_fn->code[i].op;
        if (op == OP_GET_GLOBAL) {
            uint32_t cidx = root_fn->code[i].arg;
            if (cidx < root_fn->const_len && is_obj_type(root_fn->consts[cidx], OBJ_STR)) {
                if (strcmp(AS_STR(root_fn->consts[cidx])->chars, name) == 0) {
                    return true;
                }
            }
        }
    }
    return false;
}

static bool zr_write_cstr_file(const char *path, const char *data, size_t len) {
    FILE *fp = fopen(path, "wb");
    if (!fp) return false;
    bool ok = (len == 0) || (fwrite(data, 1, len, fp) == len);
    fclose(fp);
    return ok;
}

static void zr_write_fallback_cov_html(const char *html_path, const ZrCovTable *tbl,
                                       uint64_t tot_l, uint64_t cov_l, double lpct,
                                       uint64_t tot_b, uint64_t cov_b, double bpct) {
    ZrBuf hb;
    zr_buf_init(&hb);
    zr_buf_printf(&hb,
        "<!DOCTYPE html><html lang=\"en\"><head><meta charset=\"utf-8\">"
        "<title>PBLang Coverage Report</title>"
        "<style>body{background:#0f172a;color:#e2e8f0;font-family:ui-monospace,monospace;padding:24px}"
        "table{border-collapse:collapse;width:100%%;margin-top:16px}"
        "th,td{border:1px solid #334155;padding:8px 12px;text-align:left}"
        "th{background:#1e293b}.ok{color:#4ade80}.warn{color:#facc15}</style></head><body>"
        "<h1>PBLang v%s Coverage Report</h1>"
        "<p>Lines: <b>%llu / %llu (%.1f%%)</b> &nbsp;|&nbsp; Branches: <b>%llu / %llu (%.1f%%)</b></p>"
        "<table><tr><th>File</th><th>Lines</th><th>Line %%</th><th>Branches</th><th>Branch %%</th></tr>",
        PBLANG_ZR_VERSION,
        (unsigned long long)cov_l, (unsigned long long)tot_l, lpct,
        (unsigned long long)cov_b, (unsigned long long)tot_b, bpct);
    if (tbl) {
        for (size_t fi = 0; fi < tbl->file_len; fi++) {
            const ZrCovFile *cf = &tbl->files[fi];
            if (!cf->path) continue;
            uint64_t fl_tot = 0, fl_cov = 0;
            for (size_t ln = 1; ln < cf->line_cap; ln++) {
                if (cf->line_exec[ln] || cf->line_hits[ln] > 0) {
                    fl_tot++;
                    if (cf->line_hits[ln] > 0) fl_cov++;
                }
            }
            uint64_t fb_tot = (uint64_t)cf->branch_len * 2ULL, fb_cov = 0;
            for (size_t bi = 0; bi < cf->branch_len; bi++) {
                if (cf->branches[bi].taken_hits > 0) fb_cov++;
                if (cf->branches[bi].not_taken_hits > 0) fb_cov++;
            }
            double flp = fl_tot > 0 ? (100.0 * (double)fl_cov / (double)fl_tot) : 100.0;
            double fbp = fb_tot > 0 ? (100.0 * (double)fb_cov / (double)fb_tot) : 100.0;
            zr_buf_printf(&hb,
                "<tr><td>%s</td><td>%llu/%llu</td><td>%.1f%%</td><td>%llu/%llu</td><td>%.1f%%</td></tr>",
                cf->path,
                (unsigned long long)fl_cov, (unsigned long long)fl_tot, flp,
                (unsigned long long)fb_cov, (unsigned long long)fb_tot, fbp);
        }
    }
    zr_buf_append_cstr(&hb, "</table></body></html>\n");
    if (hb.data) zr_write_cstr_file(html_path, hb.data, hb.len);
    zr_buf_free(&hb);
}

static int zr_cmd_test(const char *self_exe, int argc, char **argv) {
    ZrCaps caps;
    zr_caps_allow_all(&caps);
    bool sandbox_seen = false;
    const char *stdlib_override = NULL;
    const char *filter = NULL;
    bool quiet = false;
    bool strict_taint = false;
    bool enable_cov = false;
    const char *cov_json_path = NULL;
    const char *cov_html_path = NULL;
    ZrTestFileList inputs = { NULL, 0, 0 };

    for (int i = 0; i < argc; i++) {
        if (zr_is_cap_flag(argv[i])) {
            zr_apply_cap_flag(&caps, &sandbox_seen, argv[i]);
        } else if (strcmp(argv[i], "--strict-taint") == 0) {
            strict_taint = true;
        } else if (strcmp(argv[i], "--stdlib") == 0 && i + 1 < argc) {
            stdlib_override = argv[++i];
        } else if (strcmp(argv[i], "--filter") == 0 && i + 1 < argc) {
            filter = argv[++i];
        } else if (strcmp(argv[i], "--coverage") == 0) {
            enable_cov = true;
        } else if (strcmp(argv[i], "--coverage-json") == 0 && i + 1 < argc) {
            cov_json_path = argv[++i];
            enable_cov = true;
        } else if (strcmp(argv[i], "--coverage-html") == 0 && i + 1 < argc) {
            cov_html_path = argv[++i];
            enable_cov = true;
        } else if (strcmp(argv[i], "-q") == 0 || strcmp(argv[i], "--quiet") == 0) {
            quiet = true;
        } else if (argv[i][0] != '-') {
            zr_tlist_push(&inputs, argv[i]);
        }
    }

    if (inputs.len == 0) {
        if (zr_dir_exists("tests")) zr_tlist_push(&inputs, "tests");
        else zr_tlist_push(&inputs, ".");
    }

    ZrTestFileList files = { NULL, 0, 0 };
    for (size_t i = 0; i < inputs.len; i++) {
        zr_collect_tests_recursive(inputs.paths[i], &files);
        free(inputs.paths[i]);
    }
    free(inputs.paths);

    if (files.len > 1) {
        qsort(files.paths, files.len, sizeof(char *), zr_cmp_cstr_ptr);
    }

    if (files.len == 0) {
        fprintf(stderr, "pblang test: no test files found\n");
        return 1;
    }

    ZrCovTable shared_cov;
    zr_cov_table_init(&shared_cov);
    char discovered_stdlib[4096];
    discovered_stdlib[0] = '\0';

    size_t passed_units = 0, failed_units = 0;
    double t0 = zr_mono_sec();

    for (size_t fi = 0; fi < files.len; fi++) {
        const char *fpath = files.paths[fi];
        PbVM vm;
        vm_init(&vm);
        vm.caps = caps;
        if (strict_taint) vm.taint_guard_enabled = true;
        zr_configure_vm_paths(&vm, self_exe, fpath, stdlib_override);
        if (discovered_stdlib[0] == '\0' && vm.stdlib_root && vm.stdlib_root[0]) {
            snprintf(discovered_stdlib, sizeof(discovered_stdlib), "%s", vm.stdlib_root);
        }
        if (enable_cov) {
            vm.cov_table = &shared_cov;
            vm.cov_owns_table = false;
            vm.cov_enabled = true;
        }
        vm.cli_argc = 1;
        vm.cli_argv = &files.paths[fi];

        char lerr[512];
        lerr[0] = '\0';
        PbFunc *fn = zr_load_file_pbsh_aware(&vm, vm.current_file ? vm.current_file : fpath, lerr);
        if (!fn) {
            fprintf(stderr, "FAIL %s (compile): %s\n", fpath, lerr);
            failed_units++;
            vm_free(&vm);
            continue;
        }
        if (enable_cov) zr_cov_register_func(&shared_cov, fn);

        vm_push_root(&vm, val_obj(fn));
        zr_populate_polyglot_globals(&vm, vm.globals, fn);
        PbValue ret = val_nil();
        bool top_ok = vm_run_root_func(&vm, fn, NULL, &ret);
        if (!top_ok || (vm.exited && vm.exit_code != 0)) {
            fprintf(stderr, "FAIL %s: %s\n", fpath, vm.err_msg[0] ? vm.err_msg : "top-level exited non-zero");
            failed_units++;
            if (enable_cov) zr_cov_scan_vm_objects(&vm);
            vm_pop_roots(&vm, 1);
            zr_flight_cleanup_vm(&vm);
            vm_free(&vm);
            continue;
        }

        /* Collect uncalled `fn test_*()` functions defined in `vm.globals` */
        char **fn_names = NULL;
        size_t fn_count = 0, fn_cap = 0;
        for (size_t ei = 0; ei < vm.globals->used; ei++) {
            PbMapEntry *me = &vm.globals->entries[ei];
            if (!me->active || !me->key) continue;
            if (strncmp(me->key->chars, "test_", 5) != 0) continue;
            if (!(is_obj_type(me->val, OBJ_CLOSURE) || is_obj_type(me->val, OBJ_FUNC))) continue;
            if (zr_root_chunk_calls_global(fn, me->key->chars)) continue;
            if (filter && !strstr(me->key->chars, filter) && !strstr(fpath, filter)) continue;
            if (fn_count >= fn_cap) {
                fn_cap = fn_cap ? fn_cap * 2 : 16;
                fn_names = (char **)zr_xrealloc(fn_names, fn_cap * sizeof(char *));
            }
            fn_names[fn_count++] = zr_xstrdup(me->key->chars);
        }

        if (fn_count == 0) {
            if (!filter || strstr(fpath, filter)) {
                if (!quiet) printf("PASS %s\n", fpath);
                passed_units++;
            }
        } else {
            qsort(fn_names, fn_count, sizeof(char *), zr_cmp_cstr_ptr);
            for (size_t ti = 0; ti < fn_count; ti++) {
                PbValue callee = val_nil();
                if (pb_map_get_cstr(vm.globals, fn_names[ti], &callee)) {
                    vm.err_msg[0] = '\0';
                    vm.exited = false;
                    PbValue tret = val_nil();
                    bool tok = vm_call_value_sync(&vm, callee, 0, NULL, &tret);
                    if (tok && (!vm.exited || vm.exit_code == 0)) {
                        if (!quiet) printf("  ok  %s :: %s\n", fpath, fn_names[ti]);
                        passed_units++;
                    } else {
                        fprintf(stderr, "FAIL  %s :: %s — %s\n",
                                fpath, fn_names[ti], vm.err_msg[0] ? vm.err_msg : "test failed");
                        failed_units++;
                    }
                }
                free(fn_names[ti]);
            }
            free(fn_names);
        }

        if (enable_cov) zr_cov_scan_vm_objects(&vm);
        vm_pop_roots(&vm, 1);
        zr_flight_cleanup_vm(&vm);
        vm_free(&vm);
    }

    /* Filter out internal stdlib helper files when none of the target test files reside in stdlib */
    if (enable_cov && discovered_stdlib[0] != '\0') {
        size_t slen = strlen(discovered_stdlib);
        bool testing_stdlib = false;
        for (size_t i = 0; i < files.len; i++) {
            char cpath[4096];
            zr_canonicalize_path(files.paths[i], cpath);
            if (strncmp(cpath, discovered_stdlib, slen) == 0) { testing_stdlib = true; break; }
        }
        if (!testing_stdlib) {
            size_t non_std = 0;
            for (size_t fi = 0; fi < shared_cov.file_len; fi++) {
                if (shared_cov.files[fi].path && strncmp(shared_cov.files[fi].path, discovered_stdlib, slen) != 0) {
                    non_std++;
                }
            }
            if (non_std > 0) {
                size_t dst = 0;
                for (size_t fi = 0; fi < shared_cov.file_len; fi++) {
                    if (shared_cov.files[fi].path && strncmp(shared_cov.files[fi].path, discovered_stdlib, slen) == 0) {
                        free(shared_cov.files[fi].path);
                        free(shared_cov.files[fi].line_hits);
                        free(shared_cov.files[fi].line_exec);
                        free(shared_cov.files[fi].branches);
                    } else {
                        if (dst != fi) shared_cov.files[dst] = shared_cov.files[fi];
                        dst++;
                    }
                }
                shared_cov.file_len = dst;
                shared_cov.last_file_idx = -1;
            }
        }
    }

    for (size_t i = 0; i < files.len; i++) free(files.paths[i]);
    free(files.paths);

    double elapsed_ms = (zr_mono_sec() - t0) * 1000.0;
    printf("Test summary: %zu passed, %zu failed (%.1f ms)\n", passed_units, failed_units, elapsed_ms);

    if (enable_cov) {
        PbVM rep_vm;
        vm_init(&rep_vm);
        zr_caps_allow_all(&rep_vm.caps);
        zr_configure_vm_paths(&rep_vm, self_exe, NULL, stdlib_override);

        PbValue snap = zr_build_coverage_snapshot_map(&rep_vm, &shared_cov, true);
        vm_push_root(&rep_vm, snap);

        PbMap *sm = AS_MAP(snap);
        PbValue v_lt = val_nil(), v_lc = val_nil(), v_lp = val_nil();
        PbValue v_bt = val_nil(), v_bc = val_nil(), v_bp = val_nil();
        pb_map_get_cstr(sm, "lines_total", &v_lt);
        pb_map_get_cstr(sm, "lines_covered", &v_lc);
        pb_map_get_cstr(sm, "line_pct", &v_lp);
        pb_map_get_cstr(sm, "branches_total", &v_bt);
        pb_map_get_cstr(sm, "branches_covered", &v_bc);
        pb_map_get_cstr(sm, "branch_pct", &v_bp);

        uint64_t tot_l = v_lt.tag == VAL_NUM ? (uint64_t)v_lt.as.num : 0;
        uint64_t cov_l = v_lc.tag == VAL_NUM ? (uint64_t)v_lc.as.num : 0;
        double   lpct  = v_lp.tag == VAL_NUM ? v_lp.as.num : 100.0;
        uint64_t tot_b = v_bt.tag == VAL_NUM ? (uint64_t)v_bt.as.num : 0;
        uint64_t cov_b = v_bc.tag == VAL_NUM ? (uint64_t)v_bc.as.num : 0;
        double   bpct  = v_bp.tag == VAL_NUM ? v_bp.as.num : 100.0;

        printf("Coverage: %llu/%llu lines (%.1f%%), %llu/%llu branches (%.1f%%)\n",
               (unsigned long long)cov_l, (unsigned long long)tot_l, lpct,
               (unsigned long long)cov_b, (unsigned long long)tot_b, bpct);

        if (cov_json_path || cov_html_path) {
            bool wrote_json = false, wrote_html = false;
            PbValue cov_mod = val_nil();
            if (zr_vm_import_module_caps(&rep_vm, "stdlib:devtools/coverage_and_fuzzer", NULL, CAP_ALL, &cov_mod)) {
                vm_push_root(&rep_vm, cov_mod);
                PbValue fn_an = val_nil(), fn_js = val_nil(), fn_ht = val_nil(), rep_obj = val_nil();
                if (zr_mod_get_export(cov_mod, "analyze_coverage_data", &fn_an) &&
                    vm_call_value_sync(&rep_vm, fn_an, 1, &snap, &rep_obj)) {
                    vm_push_root(&rep_vm, rep_obj);
                    if (cov_json_path && zr_mod_get_export(cov_mod, "generate_coverage_json", &fn_js)) {
                        PbValue js_out = val_nil();
                        if (vm_call_value_sync(&rep_vm, fn_js, 1, &rep_obj, &js_out) && is_obj_type(js_out, OBJ_STR)) {
                            wrote_json = zr_write_cstr_file(cov_json_path, AS_STR(js_out)->chars, AS_STR(js_out)->len);
                        }
                    }
                    if (cov_html_path && zr_mod_get_export(cov_mod, "generate_coverage_html", &fn_ht)) {
                        PbValue ht_out = val_nil();
                        if (vm_call_value_sync(&rep_vm, fn_ht, 1, &rep_obj, &ht_out) && is_obj_type(ht_out, OBJ_STR)) {
                            wrote_html = zr_write_cstr_file(cov_html_path, AS_STR(ht_out)->chars, AS_STR(ht_out)->len);
                        }
                    }
                    vm_pop_roots(&rep_vm, 1); /* rep_obj */
                }
                vm_pop_roots(&rep_vm, 1); /* cov_mod */
            }
            rep_vm.err_msg[0] = '\0';

            if (cov_json_path && !wrote_json) {
                PbValue jpretty = val_nil();
                if (bi_json_stringify(&rep_vm, 1, &snap, &jpretty) && is_obj_type(jpretty, OBJ_STR)) {
                    zr_write_cstr_file(cov_json_path, AS_STR(jpretty)->chars, AS_STR(jpretty)->len);
                }
            }
            if (cov_html_path && !wrote_html) {
                zr_write_fallback_cov_html(cov_html_path, &shared_cov, tot_l, cov_l, lpct, tot_b, cov_b, bpct);
            }
        }

        vm_pop_roots(&rep_vm, 1); /* snap */
        vm_free(&rep_vm);
    }
    zr_cov_table_free(&shared_cov);

    return failed_units == 0 ? 0 : 1;
}

static int zr_cmd_fuzz(const char *self_exe, int argc, char **argv) {
    const char *stdlib_override = NULL;
    const char *target_file = NULL;
    int max_iters = 300;
    uint64_t seed = 1337;

    for (int i = 0; i < argc; i++) {
        if (strcmp(argv[i], "--stdlib") == 0 && i + 1 < argc) {
            stdlib_override = argv[++i];
        } else if ((strcmp(argv[i], "--iterations") == 0 || strcmp(argv[i], "--iters") == 0) && i + 1 < argc) {
            max_iters = atoi(argv[++i]);
            if (max_iters < 1) max_iters = 1;
        } else if (strcmp(argv[i], "--seed") == 0 && i + 1 < argc) {
            seed = (uint64_t)strtoull(argv[++i], NULL, 10);
        } else if (argv[i][0] != '-' && !target_file) {
            target_file = argv[i];
        }
    }

    if (!target_file) {
        fprintf(stderr, "Usage: pblang fuzz <file.sapp> [--iterations N] [--seed S] [--json]\n");
        return 1;
    }

    PbVM vm;
    vm_init(&vm);
    zr_caps_allow_all(&vm.caps);
    zr_configure_vm_paths(&vm, self_exe, target_file, stdlib_override);

    PbArray *fargs = pb_array_new(&vm, (size_t)argc + 2);
    vm_push_root(&vm, val_obj(fargs));
    for (int i = 0; i < argc; i++) {
        if (strcmp(argv[i], "--stdlib") == 0 && i + 1 < argc) { i++; continue; }
        if (strcmp(argv[i], "--iters") == 0) {
            pb_array_push(&vm, fargs, val_obj(pb_str_c(&vm, "--iterations")));
        } else {
            pb_array_push(&vm, fargs, val_obj(pb_str_c(&vm, argv[i])));
        }
    }

    PbValue cov_mod = val_nil();
    if (zr_vm_import_module_caps(&vm, "stdlib:devtools/coverage_and_fuzzer", NULL, CAP_ALL, &cov_mod)) {
        vm_push_root(&vm, cov_mod);
        PbValue entry_fn = val_nil();
        if (zr_mod_get_export(cov_mod, "cli_fuzz_entry", &entry_fn)) {
            PbValue arg_v = val_obj(fargs);
            PbValue rep_v = val_nil();
            if (!vm_call_value_sync(&vm, entry_fn, 1, &arg_v, &rep_v)) {
                fprintf(stderr, "pblang fuzz error: %s\n", vm.err_msg);
                vm_pop_roots(&vm, 2);
                vm_free(&vm);
                return 1;
            }
            int rc = 0;
            if (is_obj_type(rep_v, OBJ_MAP)) {
                PbValue ok_v = val_nil(), bug_v = val_nil(), err_v = val_nil();
                if (pb_map_get_cstr(AS_MAP(rep_v), "ok", &ok_v) && ok_v.tag == VAL_BOOL && !ok_v.as.b) {
                    if (pb_map_get_cstr(AS_MAP(rep_v), "error", &err_v) && is_obj_type(err_v, OBJ_STR)) {
                        fprintf(stderr, "%s\n", AS_STR(err_v)->chars);
                    }
                    rc = 1;
                }
                if (pb_map_get_cstr(AS_MAP(rep_v), "bug_found", &bug_v) && bug_v.tag == VAL_BOOL && bug_v.as.b) {
                    rc = 1;
                }
            }
            vm_pop_roots(&vm, 2);
            vm_free(&vm);
            return rc;
        }
        vm_pop_roots(&vm, 1);
    }

    /* Standalone fallback when stdlib is absent */
    vm.err_msg[0] = '\0';
    char lerr[512];
    lerr[0] = '\0';
    PbFunc *fn = zr_load_file_pbsh_aware(&vm, vm.current_file ? vm.current_file : target_file, lerr);
    if (!fn) {
        fprintf(stderr, "pblang fuzz: compile failed for '%s': %s\n", target_file, lerr);
        vm_pop_roots(&vm, 1);
        vm_free(&vm);
        return 1;
    }
    vm_push_root(&vm, val_obj(fn));
    PbValue top_ret = val_nil();
    if (!vm_run_root_func(&vm, fn, NULL, &top_ret)) {
        fprintf(stderr, "pblang fuzz: top-level failure in '%s': %s\n", target_file, vm.err_msg);
        vm_pop_roots(&vm, 2);
        vm_free(&vm);
        return 1;
    }
    uint64_t rng = seed ? seed : 1337ULL;
    size_t targets = 0;
    for (size_t ei = 0; ei < vm.globals->used; ei++) {
        PbMapEntry *me = &vm.globals->entries[ei];
        if (!me->active || !me->key || !is_obj_type(me->val, OBJ_CLOSURE)) continue;
        PbClosure *cl = AS_CLOSURE(me->val);
        if (!cl->fn || cl->fn->arity == 0 || cl->fn->arity > 4) continue;
        targets++;
        for (int it = 0; it < max_iters; it++) {
            PbValue args_buf[4];
            for (uint32_t a = 0; a < cl->fn->arity; a++) {
                rng ^= rng << 13; rng ^= rng >> 7; rng ^= rng << 17;
                args_buf[a] = val_num((double)((int64_t)(rng % 2001ULL) - 1000));
            }
            PbValue out_v = val_nil();
            vm.err_msg[0] = '\0';
            if (!vm_call_value_sync(&vm, me->val, (int)cl->fn->arity, args_buf, &out_v)) {
                fprintf(stderr, "FUZZ FAILURE in %s :: %s: %s\n", target_file, me->key->chars, vm.err_msg);
                vm_pop_roots(&vm, 2);
                vm_free(&vm);
                return 1;
            }
        }
    }
    printf("FUZZ OK: %zu target(s) passed in %s\n", targets, target_file);
    vm_pop_roots(&vm, 2);
    vm_free(&vm);
    return 0;
}

static int zr_cmd_doctor(const char *self_exe) {
    PbVM vm;
    vm_init(&vm);
    zr_caps_allow_all(&vm.caps);
    zr_configure_vm_paths(&vm, self_exe, NULL, NULL);

    PbValue hw = val_nil();
    bi_hardware_probe(&vm, 0, NULL, &hw);
    vm_push_root(&vm, hw);
    PbValue gfxp = val_nil();
    bi_gfxbackendprofile(&vm, 0, NULL, &gfxp);
    vm_push_root(&vm, gfxp);

    char cache_sample[4096];
    cache_sample[0] = '\0';
    const uint8_t probe_src[] = "doctor";
    bool has_cache = zr_get_pbck_cache_path(probe_src, sizeof(probe_src) - 1, cache_sample);
    char cache_dir[4096];
    cache_dir[0] = '\0';
    if (has_cache) zr_path_parent_dir(cache_sample, cache_dir);

    char ssl_err[256] = {0}, sql_err[256] = {0};
    bool ssl_ok = zr_ensure_openssl(ssl_err);
    bool sql_ok = zr_ensure_sqlite(sql_err);

    printf("%s\n", PBLANG_ZR_VERSION_STR);
    printf("  executable      : %s\n", self_exe ? self_exe : "(unknown)");
    printf("  stdlib_root     : %s\n", vm.stdlib_root ? vm.stdlib_root : "(not found)");
    printf("  pbck_disk_cache : %s\n", cache_dir[0] ? cache_dir : "(disabled)");
    printf("  builtins_count  : %u\n", (unsigned)vm.builtin_count);
#if ZR_JIT_ARCH_AVAILABLE
    printf("  jit_engine      : x86_64 SSE2 W^X native JIT + computed-goto VM\n");
#else
    printf("  jit_engine      : portable C99 interpreter + peephole superinstructions\n");
#endif
#ifdef _WIN32
    printf("  tls_https_stack : WinHTTP HTTPS + OpenSSL dynamic (%s)\n", ssl_ok ? "loaded" : "winhttp-primary");
#else
    printf("  tls_https_stack : OpenSSL dynamic (%s)\n", ssl_ok ? "available" : "not found");
#endif
    printf("  sqlite3_engine  : %s\n", sql_ok ? "available (dynamic)" : "not found");
    if (is_obj_type(hw, OBJ_MAP)) {
        PbValue os_v = val_nil(), arch_v = val_nil(), cores_v = val_nil(), ram_v = val_nil();
        pb_map_get_cstr(AS_MAP(hw), "os", &os_v);
        pb_map_get_cstr(AS_MAP(hw), "arch", &arch_v);
        pb_map_get_cstr(AS_MAP(hw), "cpu_cores", &cores_v);
        pb_map_get_cstr(AS_MAP(hw), "ram_total_mb", &ram_v);
        printf("  host_platform   : %s/%s (%d cores, %.0f MiB RAM)\n",
               is_obj_type(os_v, OBJ_STR) ? AS_STR(os_v)->chars : "unknown",
               is_obj_type(arch_v, OBJ_STR) ? AS_STR(arch_v)->chars : "unknown",
               cores_v.tag == VAL_NUM ? (int)cores_v.as.num : 1,
               ram_v.tag == VAL_NUM ? ram_v.as.num : 0.0);
    }
    if (is_obj_type(gfxp, OBJ_MAP)) {
        PbValue b_v = val_nil();
        pb_map_get_cstr(AS_MAP(gfxp), "backend", &b_v);
        printf("  gfx_backend     : %s\n", is_obj_type(b_v, OBJ_STR) ? AS_STR(b_v)->chars : "headless");
    }

    vm_pop_roots(&vm, 2);
    vm_free(&vm);
    return 0;
}

#include "zr_smart_updater.h"

static int zr_cmd_license_credits(void) {
    printf("%s\n", PBLANG_ZR_VERSION_STR);
    printf("Creator & Original Author : Panagiotis (Panos) Biazis (@PanosBiazis)\n");
    printf("Official Repository       : https://github.com/PanosBiazis/pblang\n");
    printf("Engine & Stdlib License   : Apache License 2.0 (SPDX: Apache-2.0) + Binding NOTICE (Section 4(d))\n");
    printf("Compiler Output Policy    : Sovereign Developer Choice (Apache-2.0 never infects user binaries)\n");
    printf("  - Default binary policy : All Rights Reserved + Zero-Overhead PBSH-v1 Polymorphic Shield + PBIPSEAL\n");
    printf("  - Open-Source choice    : --free / --license <SPDX> / --open-symbols (MIT, Apache-2.0, GPL-3.0-only, BSD-3-Clause, ...)\n");
    printf("  - Attribution-free/anon : --stealth (Redacted holder, SOURCE_DATE_EPOCH=0, PBSH-v1 encrypted, zero leaks)\n");
    return 0;
}

static int zr_cmd_shield_inspect(int argc, char **argv) {
    const char *target_file = NULL;
    for (int i = 0; i < argc; i++) {
        if (argv[i][0] != '-' && !target_file) {
            target_file = argv[i];
        }
    }
    if (!target_file) {
        fprintf(stderr, "Usage: pblang shield-inspect <artifact_or_pbck_file>\n");
        return 1;
    }

    PbVM vm;
    vm_init(&vm);
    zr_caps_allow_all(&vm.caps);

    PbValue arg = val_obj(pb_str_c(&vm, target_file));
    vm_push_root(&vm, arg);
    PbValue info_val = val_nil();
    if (!bi_pbck_shield_inspect(&vm, 1, &arg, &info_val)) {
        fprintf(stderr, "shield-inspect: %s\n", vm.err_msg[0] ? vm.err_msg : "inspection failed");
        vm_pop_roots(&vm, 1);
        vm_free(&vm);
        return 1;
    }
    vm_push_root(&vm, info_val);

    PbValue json_val = val_nil();
    if (bi_json_stringify(&vm, 1, &info_val, &json_val) && is_obj_type(json_val, OBJ_STR)) {
        printf("%s\n", AS_STR(json_val)->chars);
    }

    bool ok_flag = false;
    if (is_obj_type(info_val, OBJ_MAP)) {
        PbValue ok_v = val_nil();
        if (pb_map_get_cstr(AS_MAP(info_val), "ok", &ok_v) && ok_v.tag == VAL_BOOL) {
            ok_flag = ok_v.as.b;
        }
    }

    vm_pop_roots(&vm, 2);
    vm_free(&vm);
    return ok_flag ? 0 : 1;
}

static int zr_cmd_ip_tool(const char *self_exe, const char *action, int argc, char **argv) {
    const char *target_file = NULL;
    const char *out_file = NULL;
    const char *stdlib_ov = NULL;
    const char *mode_val = "proprietary";
    const char *spdx_val = "";
    const char *holder_val = "";
    const char *sign_env = "";

    for (int i = 0; i < argc; i++) {
        if ((strcmp(argv[i], "-o") == 0 || strcmp(argv[i], "--out") == 0) && i + 1 < argc) {
            out_file = argv[++i];
        } else if (strcmp(argv[i], "--stdlib") == 0 && i + 1 < argc) {
            stdlib_ov = argv[++i];
        } else if (strcmp(argv[i], "--free") == 0) {
            mode_val = "free";
        } else if (strcmp(argv[i], "--free-attribution") == 0) {
            mode_val = "free-attribution";
        } else if (strcmp(argv[i], "--stealth") == 0 || strcmp(argv[i], "--stealth-ip") == 0) {
            mode_val = "stealth-private";
        } else if (strcmp(argv[i], "--license") == 0 && i + 1 < argc) {
            spdx_val = argv[++i];
            if (strcmp(mode_val, "proprietary") == 0) mode_val = "open-source";
        } else if (strcmp(argv[i], "--copyright-holder") == 0 && i + 1 < argc) {
            holder_val = argv[++i];
        } else if (strcmp(argv[i], "--sign-env") == 0 && i + 1 < argc) {
            sign_env = argv[++i];
        } else if (argv[i][0] != '-' && !target_file) {
            target_file = argv[i];
        }
    }
    if (!target_file) {
        fprintf(stderr, "Usage: pblang %s <artifact_file> [-o <out>] [--free | --license <SPDX> | --copyright-holder <name> | --stealth] [--sign-env <ENV_VAR>]\n", action);
        return 1;
    }
    if (!out_file) out_file = target_file;

    ZrCaps icaps;
    zr_caps_allow_all(&icaps);
    char *eargs[7] = {
        (char *)action,
        (char *)target_file,
        (char *)out_file,
        (char *)mode_val,
        (char *)spdx_val,
        (char *)holder_val,
        (char *)sign_env
    };
    const char *eval_code =
        "let A = os_args(); let IP = import(\"stdlib:ip_rights_provenance.sapp\"); "
        "let act = A[0]; let raw = fs_read_bytes(A[1]); "
        "if act == \"ip-seal\" { "
        "  let raw_clean = IP.strip_artifact_ip_seal(raw); "
        "  let opts = map{ \"mode\": A[3] }; "
        "  if len(A[4]) > 0 { opts[\"license\"] = A[4]; } "
        "  if len(A[5]) > 0 { opts[\"holder\"] = A[5]; } "
        "  let pol = IP.build_ip_policy(opts); "
        "  let sec = if len(A[6]) > 0 { env_get(A[6]) } else { nil }; "
        "  let sealed = IP.seal_artifact_bytes(raw_clean, \"auto\", pol, sec); "
        "  fs_write_bytes(A[2], sealed); "
        "  print json_stringify(IP.inspect_artifact_ip(sealed)); "
        "} elif act == \"ip-inspect\" { "
        "  print json_stringify(IP.inspect_artifact_ip(raw)); "
        "} else { "
        "  let sec = if len(A[6]) > 0 { env_get(A[6]) } else { nil }; "
        "  let v = IP.verify_artifact_ip(raw, sec); "
        "  print json_stringify(v); "
        "  if not v.ok { os_exit(1); } "
        "}";
    return zr_cmd_eval(self_exe, eval_code, &icaps, stdlib_ov, 7, eargs);
}

static void zr_apply_host_oom_protection(int argc, char **argv) {
    const char *disable = getenv("PBLANG_NO_OOM_SHIELD");
    if (disable && (strcmp(disable, "1") == 0 || strcmp(disable, "true") == 0)) return;

    bool is_daemon = false;
    for (int i = 1; i < argc; i++) {
        if (!argv[i]) continue;
        if (strstr(argv[i], "serve") != NULL || strstr(argv[i], "stdio") != NULL || strcmp(argv[i], "lsp") == 0) {
            is_daemon = true;
            break;
        }
    }

    uint64_t cap_bytes = is_daemon ? 1073741824ULL : 2684354560ULL;
    const char *env_cap = getenv("PBLANG_MAX_AS");
    if (!env_cap || !*env_cap) env_cap = getenv("PBLANG_MAX_MEM_BYTES");
    if (env_cap && *env_cap) {
        unsigned long long v = strtoull(env_cap, NULL, 10);
        if (v >= (64ULL * 1024ULL * 1024ULL)) cap_bytes = (uint64_t)v;
    }

#if defined(__linux__)
    if (!is_daemon) {
        int target_adj = 850;
        const char *env_adj = getenv("PBLANG_OOM_SCORE_ADJ");
        if (env_adj && *env_adj) target_adj = atoi(env_adj);
        if (target_adj < 0) target_adj = 0;
        if (target_adj > 1000) target_adj = 1000;
        int cur_adj = 0;
        FILE *rf = fopen("/proc/self/oom_score_adj", "rb");
        if (rf) {
            char abuf[32];
            size_t nr = fread(abuf, 1, sizeof(abuf) - 1, rf);
            abuf[nr] = '\0';
            cur_adj = atoi(abuf);
            fclose(rf);
        }
        if (cur_adj < target_adj) {
            FILE *wf = fopen("/proc/self/oom_score_adj", "wb");
            if (wf) {
                fprintf(wf, "%d\n", target_adj);
                fclose(wf);
            }
        }
    }
    {
        struct rlimit rl;
        if (getrlimit(RLIMIT_AS, &rl) == 0) {
            uint64_t safe_cap = cap_bytes;
            if (sizeof(rlim_t) <= 4 && safe_cap > 0x7FFFFFFFULL) safe_cap = 0x7FFFFFFFULL;
            rlim_t want = (rlim_t)safe_cap;
            if (rl.rlim_max != RLIM_INFINITY && want > rl.rlim_max) want = rl.rlim_max;
            if (rl.rlim_cur == RLIM_INFINITY || want < rl.rlim_cur) {
                rl.rlim_cur = want;
                (void)setrlimit(RLIMIT_AS, &rl);
            }
        }
    }
#elif defined(_WIN32)
    {
        HANDLE hJob = CreateJobObjectA(NULL, NULL);
        if (hJob) {
            JOBOBJECT_EXTENDED_LIMIT_INFORMATION jeli;
            memset(&jeli, 0, sizeof(jeli));
            jeli.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_PROCESS_MEMORY;
            jeli.ProcessMemoryLimit = (SIZE_T)cap_bytes;
            if (SetInformationJobObject(hJob, JobObjectExtendedLimitInformation, &jeli, sizeof(jeli))) {
                (void)AssignProcessToJobObject(hJob, GetCurrentProcess());
            } else {
                CloseHandle(hJob);
            }
        }
    }
#else
    (void)cap_bytes;
#endif
}

int main(int argc, char **argv) {
    zr_apply_host_oom_protection(argc, argv);
    zr_net_subsystem_init();

    char *self_exe = zr_resolve_self_exe(argc > 0 ? argv[0] : NULL);

    /* 1. Check for self-contained single-file `PBLZREXE` bundled payload */
    if (self_exe && *self_exe) {
        uint8_t *embedded_pbck = NULL;
        size_t embedded_len = 0;
        if (zr_check_embedded_pbck(self_exe, &embedded_pbck, &embedded_len)) {
            int rc = zr_run_embedded_bundle(self_exe, embedded_pbck, embedded_len, argc, argv);
            free(embedded_pbck);
            free(self_exe);
            return rc;
        }
    }

    if (argc < 2) {
        zr_print_help(argc > 0 ? argv[0] : "pblang");
        free(self_exe);
        return 0;
    }

    const char *sub = argv[1];

    if (strcmp(sub, "--version") == 0 || strcmp(sub, "-V") == 0 || strcmp(sub, "version") == 0) {
        zr_print_version();
        free(self_exe);
        return 0;
    }
    if (strcmp(sub, "license") == 0 || strcmp(sub, "--license-info") == 0 || strcmp(sub, "credits") == 0 || strcmp(sub, "attribution") == 0) {
        int rc = zr_cmd_license_credits();
        free(self_exe);
        return rc;
    }
    if (strcmp(sub, "shield-inspect") == 0 || strcmp(sub, "--shield-inspect") == 0) {
        int rc = zr_cmd_shield_inspect(argc - 2, argv + 2);
        free(self_exe);
        return rc;
    }
    if (strcmp(sub, "ip-seal") == 0 || strcmp(sub, "ip-inspect") == 0 || strcmp(sub, "ip-verify") == 0) {
        int rc = zr_cmd_ip_tool(self_exe, sub, argc - 2, argv + 2);
        free(self_exe);
        return rc;
    }
    if (strcmp(sub, "--help") == 0 || strcmp(sub, "-h") == 0 || strcmp(sub, "help") == 0) {
        zr_print_help(argv[0]);
        free(self_exe);
        return 0;
    }
    if (strcmp(sub, "doctor") == 0 || strcmp(sub, "hardware") == 0) {
        int rc = zr_cmd_doctor(self_exe);
        free(self_exe);
        return rc;
    }
    if (strcmp(sub, "repl") == 0) {
        int rc = zr_cmd_repl(self_exe, argc - 2, argv + 2);
        free(self_exe);
        return rc;
    }
    if (strcmp(sub, "fmt") == 0) {
        int rc = zr_cmd_fmt(argc - 2, argv + 2);
        free(self_exe);
        return rc;
    }
    if (strcmp(sub, "doc") == 0) {
        int rc = zr_cmd_doc(argc - 2, argv + 2);
        free(self_exe);
        return rc;
    }
    if (strcmp(sub, "lsp") == 0) {
        int rc = zr_cmd_lsp(self_exe);
        free(self_exe);
        return rc;
    }
    if (strcmp(sub, "debug") == 0) {
        int rc = zr_cmd_debug(self_exe, argc - 2, argv + 2);
        free(self_exe);
        return rc;
    }
    if (strcmp(sub, "dap") == 0 || strcmp(sub, "init") == 0 || strcmp(sub, "pkg") == 0 || strcmp(sub, "bench") == 0 || strcmp(sub, "cross-bundle") == 0) {
        ZrCaps scaps; zr_caps_allow_all(&scaps);
        char expr[192]; snprintf(expr, sizeof(expr), "import(\"stdlib:devtools/cli_router.sapp\").dispatch_cli(\"%s\", os_args());", sub);
        int rc = zr_cmd_eval(self_exe, expr, &scaps, NULL, argc - 2, argv + 2);
        free(self_exe);
        return rc;
    }
    if (strcmp(sub, "prof") == 0) {
        ZrCaps pcaps;
        zr_caps_allow_all(&pcaps);
        bool sb = false;
        ZrRunOpts ropts;
        memset(&ropts, 0, sizeof(ropts));
        ropts.prof_enabled = true;
        const char *stdlib_ov = NULL;
        const char *target = NULL;
        int sstart = argc;
        for (int i = 2; i < argc; i++) {
            if (strcmp(argv[i], "--") == 0) { sstart = i + 1; break; }
            if (zr_is_cap_flag(argv[i])) zr_apply_cap_flag(&pcaps, &sb, argv[i]);
            else if (strcmp(argv[i], "--json") == 0) ropts.prof_json = true;
            else if (strcmp(argv[i], "--svg") == 0 && i + 1 < argc) ropts.prof_svg_path = argv[++i];
            else if (strcmp(argv[i], "--strict-taint") == 0) ropts.strict_taint = true;
            else if (strcmp(argv[i], "--stdlib") == 0 && i + 1 < argc) stdlib_ov = argv[++i];
            else if (argv[i][0] != '-' && !target) { target = argv[i]; sstart = i; break; }
        }
        if (!target) {
            fprintf(stderr, "Usage: pblang prof [--json] [--svg <out.svg>] <file.sapp> [args...]\n");
            free(self_exe);
            return 1;
        }
        int rc = zr_cmd_run_file_ex(self_exe, target, &pcaps, stdlib_ov, &ropts, argc - sstart, argv + sstart);
        free(self_exe);
        return rc;
    }
    if (strcmp(sub, "replay") == 0) {
        ZrCaps rcaps;
        zr_caps_allow_all(&rcaps);
        bool sb = false;
        ZrRunOpts ropts;
        memset(&ropts, 0, sizeof(ropts));
        const char *stdlib_ov = NULL;
        const char *trace_path = NULL;
        const char *target = NULL;
        int sstart = argc;
        for (int i = 2; i < argc; i++) {
            if (strcmp(argv[i], "--") == 0) { sstart = i + 1; break; }
            if (zr_is_cap_flag(argv[i])) zr_apply_cap_flag(&rcaps, &sb, argv[i]);
            else if (strcmp(argv[i], "--strict-taint") == 0) ropts.strict_taint = true;
            else if (strcmp(argv[i], "--stdlib") == 0 && i + 1 < argc) stdlib_ov = argv[++i];
            else if (argv[i][0] != '-' && !trace_path) trace_path = argv[i];
            else if (argv[i][0] != '-' && !target) { target = argv[i]; sstart = i; break; }
        }
        if (!trace_path || !target) {
            fprintf(stderr, "Usage: pblang replay <trace.pbrec> <file.sapp> [args...]\n");
            free(self_exe);
            return 1;
        }
        ropts.replay_path = trace_path;
        int rc = zr_cmd_run_file_ex(self_exe, target, &rcaps, stdlib_ov, &ropts, argc - sstart, argv + sstart);
        free(self_exe);
        return rc;
    }
    if (strcmp(sub, "studio") == 0 || strcmp(sub, "--studio") == 0) {
        ZrCaps scaps;
        zr_caps_allow_all(&scaps);
        const char *html_out = NULL;
        for (int i = 2; i < argc; i++) {
            if (strcmp(argv[i], "--export-html") == 0 && i + 1 < argc) html_out = argv[++i];
        }
        char code[1024];
        if (html_out) {
            snprintf(code, sizeof(code),
                     "let S = import(\"stdlib:sovereign_studio_ui.sapp\"); let r = S.export_studio_html(os_args()[0]); print json_stringify(r);");
            char *sargs[1] = { (char *)html_out };
            int rc = zr_cmd_eval(self_exe, code, &scaps, NULL, 1, sargs);
            free(self_exe);
            return rc;
        }
        snprintf(code, sizeof(code),
                 "let S = import(\"stdlib:sovereign_studio_ui.sapp\"); S.run_native_studio_window(map{});");
        int rc = zr_cmd_eval(self_exe, code, &scaps, NULL, 0, NULL);
        free(self_exe);
        return rc;
    }
    if (strcmp(sub, "--ide") == 0 || strcmp(sub, "ide") == 0) {
        ZrCaps icaps;
        zr_caps_allow_all(&icaps);
        char ide_path[4096];
        ide_path[0] = '\0';
        if (zr_file_exists_reg("editors/ide/pbide.sapp")) {
            snprintf(ide_path, sizeof(ide_path), "editors/ide/pbide.sapp");
        } else if (self_exe && *self_exe) {
            char dir[4096];
            snprintf(dir, sizeof(dir), "%.4000s", self_exe);
            char *sl = strrchr(dir, '/');
            char *bsl = strrchr(dir, '\\');
            if (bsl && (!sl || bsl > sl)) sl = bsl;
            if (sl) *sl = '\0';
            char cand[4160];
            snprintf(cand, sizeof(cand), "%s/../editors/ide/pbide.sapp", dir);
            if (zr_file_exists_reg(cand)) snprintf(ide_path, sizeof(ide_path), "%.4090s", cand);
        }
        if (!ide_path[0]) {
            fprintf(stderr, "pblang --ide: editors/ide/pbide.sapp not found; launching Sovereign Studio UI instead.\n");
            const char *code = "let S = import(\"stdlib:sovereign_studio_ui.sapp\"); S.run_native_studio_window(map{});";
            int rc = zr_cmd_eval(self_exe, code, &icaps, NULL, 0, NULL);
            free(self_exe);
            return rc;
        }
        int rc = zr_cmd_run_file(self_exe, ide_path, &icaps, NULL, argc - 2, argv + 2);
        free(self_exe);
        return rc;
    }
    if (strcmp(sub, "update") == 0) {
        int rc = zr_cmd_update(self_exe, argc - 2, argv + 2);
        free(self_exe);
        return rc;
    }
    if (strcmp(sub, "build") == 0 || strcmp(sub, "compile") == 0) {
        int rc = zr_cmd_build(self_exe, argc - 2, argv + 2);
        free(self_exe);
        return rc;
    }
    if (strcmp(sub, "test") == 0) {
        int rc = zr_cmd_test(self_exe, argc - 2, argv + 2);
        free(self_exe);
        return rc;
    }
    if (strcmp(sub, "fuzz") == 0) {
        int rc = zr_cmd_fuzz(self_exe, argc - 2, argv + 2);
        free(self_exe);
        return rc;
    }
    if (strcmp(sub, "check") == 0 || strcmp(sub, "--check") == 0) {
        const char *target = NULL;
        const char *stdlib_ov = NULL;
        bool strict_mode = false;
        bool syntax_only = false;
        bool json_mode = false;
        for (int i = 2; i < argc; i++) {
            if (strcmp(argv[i], "--stdlib") == 0 && i + 1 < argc) stdlib_ov = argv[++i];
            else if (strcmp(argv[i], "--strict") == 0) strict_mode = true;
            else if (strcmp(argv[i], "--syntax-only") == 0) syntax_only = true;
            else if (strcmp(argv[i], "--json") == 0) json_mode = true;
            else if (argv[i][0] != '-' && !target) target = argv[i];
        }
        if (!target) {
            fprintf(stderr, "Usage: pblang check [--strict] [--json] [--syntax-only] <file.sapp>\n");
            free(self_exe);
            return 1;
        }
        int rc = zr_cmd_check_file_ex(self_exe, target, stdlib_ov, strict_mode, syntax_only, json_mode);
        free(self_exe);
        return rc;
    }

    /* `run` subcommand, `-e` / `eval`, or direct `<script.sapp|script.pbck>` invocation */
    int start_idx = (strcmp(sub, "run") == 0) ? 2 : 1;
    ZrCaps caps;
    zr_caps_allow_all(&caps);
    bool sandbox_seen = false;
    const char *sb_env = getenv("PBLANG_SANDBOX");
    if (sb_env && strcmp(sb_env, "1") == 0) {
        zr_caps_init_default(&caps);
        sandbox_seen = true;
    }

    ZrRunOpts ropts;
    memset(&ropts, 0, sizeof(ropts));
    const char *stdlib_override = NULL;
    const char *eval_code = NULL;
    const char *target_file = NULL;
    int script_arg_start = argc;

    for (int i = start_idx; i < argc; i++) {
        const char *a = argv[i];
        if (strcmp(a, "--") == 0) {
            script_arg_start = i + 1;
            break;
        }
        if (zr_is_cap_flag(a)) {
            zr_apply_cap_flag(&caps, &sandbox_seen, a);
        } else if (strcmp(a, "--strict-taint") == 0) {
            ropts.strict_taint = true;
        } else if (strcmp(a, "--record") == 0 && i + 1 < argc) {
            ropts.record_path = argv[++i];
        } else if (strcmp(a, "--stdlib") == 0 && i + 1 < argc) {
            stdlib_override = argv[++i];
        } else if ((strcmp(a, "-e") == 0 || strcmp(a, "eval") == 0) && i + 1 < argc) {
            eval_code = argv[++i];
            script_arg_start = i + 1;
            for (int j = i + 1; j < argc; j++) {
                if (strcmp(argv[j], "--") == 0) { script_arg_start = j + 1; break; }
                if (zr_is_cap_flag(argv[j])) zr_apply_cap_flag(&caps, &sandbox_seen, argv[j]);
                else if (strcmp(argv[j], "--strict-taint") == 0) ropts.strict_taint = true;
                else if (strcmp(argv[j], "--record") == 0 && j + 1 < argc) ropts.record_path = argv[++j];
                else if (strcmp(argv[j], "--stdlib") == 0 && j + 1 < argc) stdlib_override = argv[++j];
            }
            break;
        } else if (strcmp(a, "--check") == 0 && i + 1 < argc) {
            int rc = zr_cmd_check_file(self_exe, argv[i + 1], stdlib_override);
            free(self_exe);
            return rc;
        } else if (a[0] == '-') {
            fprintf(stderr, "Unknown flag: %s (try --help)\n", a);
            free(self_exe);
            return 1;
        } else {
            target_file = a;
            script_arg_start = i;
            for (int j = i + 1; j < argc; j++) {
                if (strcmp(argv[j], "--") == 0) break;
                if (zr_is_cap_flag(argv[j])) zr_apply_cap_flag(&caps, &sandbox_seen, argv[j]);
                else if (strcmp(argv[j], "--strict-taint") == 0) ropts.strict_taint = true;
                else if (strcmp(argv[j], "--record") == 0 && j + 1 < argc) ropts.record_path = argv[++j];
                else if (strcmp(argv[j], "--stdlib") == 0 && j + 1 < argc) stdlib_override = argv[++j];
            }
            break;
        }
    }

    if (eval_code) {
        int rc = zr_cmd_eval_ex(self_exe, eval_code, &caps, stdlib_override, &ropts,
                                argc - script_arg_start, argv + script_arg_start);
        free(self_exe);
        return rc;
    }

    if (!target_file) {
        fprintf(stderr, "Error: missing script path (try `pblang --help`)\n");
        free(self_exe);
        return 1;
    }

    int rc = zr_cmd_run_file_ex(self_exe, target_file, &caps, stdlib_override, &ropts,
                                argc - script_arg_start, argv + script_arg_start);
    free(self_exe);
    return rc;
}
