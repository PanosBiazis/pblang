/* ============================================================================
 * pblang Sovereign Zero-Rust Native Runtime & Compiler — v2.6.0
 *
 * 100% Rust-independent cross-platform & multi-device C99 implementation of:
 *   - Native `.sapp -> .pbck` v1 preprocessor, Unicode lexer, parser,
 *     AST desugarer, bytecode compiler & `.pbck` serializer/deserializer
 *   - Stack VM with closures, upvalues, `defer`, `try/catch`, adaptive GC,
 *     `BigInt` / `Rational` / `Decimal`, and 260+ native host builtins
 *   - Automatic `stdlib` discovery, `pblang.toml` / `pblang.lock` / workspace
 *     module resolution, in-VM module cache & SHA-256 `.pbck` disk cache
 *   - Standalone single-file executable bundler (`PBLZREXE` trailer) &
 *     universal `64-bit / 32-bit / 16-bit / 8-bit / WASM` `--target` builder
 *
 * Linux build:
 *   gcc -O3 -std=gnu99 -Wall -Wno-unused-function \
 *       runtime_stage3/pblang_zero_rust_vm.c -ldl -lpthread -lm -s -o pblang
 *
 * Windows PE32+ / PE32 cross-build:
 *   x86_64-w64-mingw32-gcc -O3 -std=gnu99 -s \
 *       runtime_stage3/pblang_zero_rust_vm.c \
 *       -lws2_32 -lwinhttp -lgdi32 -luser32 -lbcrypt -o pblang.exe
 * ============================================================================ */

#include "zr_builtins_core.h"

#ifndef _WIN32
#include <dirent.h>
#endif

#define PBLANG_ZR_VERSION_STR "pblang 2.6.0 (sovereign zero-rust runtime; compat 1.5.0)"

static void zr_print_version(void) {
    printf("%s\n", PBLANG_ZR_VERSION_STR);
}

static void zr_print_help(const char *prog) {
    const char *p = (prog && *prog) ? prog : "pblang";
    printf("%s\n\n", PBLANG_ZR_VERSION_STR);
    printf("Usage:\n");
    printf("  %s <file.sapp|file.pbck> [args...]            Execute script or bytecode\n", p);
    printf("  %s run [flags] <file.sapp|file.pbck> [args..] Execute with explicit flags\n", p);
    printf("  %s -e <code_string> [args...]                 Evaluate inline pblang code\n", p);
    printf("  %s test [flags] [files_or_dirs...]            Run test suites (`fn test_*` & scripts)\n", p);
    printf("  %s check <file.sapp>                          Compile-check `.sapp` without running\n", p);
    printf("  %s build <file.sapp> [-o <out>] [--target <t> | --bundle-exe] Compile `.pbck` or native target\n", p);
    printf("      Targets: pbck | x64 | x64-dyn | pe64 | arm64 | wasm | x86 | pe32 | rv32 | dos16 | cpm80 | c64 | avr8\n");
    printf("  %s doctor | hardware                          Inspect runtime, stdlib & host capabilities\n", p);
    printf("  %s update [--check]                           Show sovereign runtime version & update info\n", p);
    printf("  %s --version | -V                             Print version\n", p);
    printf("  %s --help | -h                                Print this help\n\n", p);
    printf("Capability & Runtime Flags:\n");
    printf("  --allow-all               Enable all host capabilities (default unless --sandbox)\n");
    printf("  --sandbox                 Restrict host I/O unless specific --allow-* flags are given\n");
    printf("  --allow-fs                Allow filesystem read/write operations\n");
    printf("  --allow-net               Allow TCP/TLS socket operations\n");
    printf("  --allow-http              Allow HTTP/HTTPS client & AI provider requests\n");
    printf("  --allow-sqlite            Allow native SQLite3 database operations\n");
    printf("  --allow-ffi               Allow dynamic shared-library FFI loading\n");
    printf("  --ffi-allow-all           Allow arbitrary library paths & symbols under FFI\n");
    printf("  --allow-exec              Allow subprocess & polyglot execution\n");
    printf("  --allow-gfx               Allow X11 / Win32GDI / headless graphics windows\n");
    printf("  --allow-spawn             Allow OS thread spawning (`spawn` / `spawn_n`)\n");
    printf("  --allow-env               Allow environment variable access\n");
    printf("  --stdlib <dir>            Override stdlib directory path\n");
    printf("  --filter <substr>         Filter `test_*` functions / test file paths\n");
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

static int zr_cmd_eval(const char *self_exe, const char *code_str, const ZrCaps *caps,
                       const char *stdlib_override, int script_argc, char **script_argv) {
    PbVM vm;
    vm_init(&vm);
    vm.caps = *caps;
    zr_configure_vm_paths(&vm, self_exe, "<eval>", stdlib_override);
    vm.cli_argc = script_argc;
    vm.cli_argv = script_argv;

    char cerr[512];
    cerr[0] = '\0';
    PbFunc *fn = zr_compile_sapp(&vm, code_str, strlen(code_str), "<eval>", cerr);
    if (!fn) {
        fprintf(stderr, "Compile error: %s\n", cerr[0] ? cerr : "syntax error");
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
    vm_free(&vm);
    return rc;
}

static int zr_cmd_run_file(const char *self_exe, const char *target_file, const ZrCaps *caps,
                           const char *stdlib_override, int script_argc, char **script_argv) {
    if (!zr_file_exists_reg(target_file)) {
        fprintf(stderr, "Error: file not found: %s\n", target_file);
        return 1;
    }
    PbVM vm;
    vm_init(&vm);
    vm.caps = *caps;
    zr_configure_vm_paths(&vm, self_exe, target_file, stdlib_override);
    vm.cli_argc = script_argc;
    vm.cli_argv = script_argv;

    char lerr[512];
    lerr[0] = '\0';
    PbFunc *fn = zr_load_or_compile_file(&vm, vm.current_file ? vm.current_file : target_file, lerr);
    if (!fn) {
        fprintf(stderr, "%s\n", lerr[0] ? lerr : "Failed to load or compile script");
        vm_free(&vm);
        return 1;
    }
    vm_push_root(&vm, val_obj(fn));
    zr_populate_polyglot_globals(&vm, vm.globals, fn);
    PbValue ret = val_nil();
    bool ok = vm_run_root_func(&vm, fn, NULL, &ret);
    vm_pop_roots(&vm, 1);
    int rc = 0;
    if (vm.exited) {
        rc = vm.exit_code;
    } else if (!ok) {
        fprintf(stderr, "Runtime error: %s\n", vm.err_msg[0] ? vm.err_msg : "execution failed");
        rc = 1;
    }
    vm_free(&vm);
    return rc;
}

static int zr_cmd_check_file(const char *self_exe, const char *target_file, const char *stdlib_override) {
    uint8_t *raw = NULL;
    size_t raw_len = 0;
    if (!zr_read_entire_file(target_file, &raw, &raw_len)) {
        fprintf(stderr, "check: cannot read '%s'\n", target_file);
        return 1;
    }
    PbVM vm;
    vm_init(&vm);
    zr_configure_vm_paths(&vm, self_exe, target_file, stdlib_override);
    PbFunc *fn = NULL;
    char err[512];
    err[0] = '\0';
    if (raw_len >= 4 && memcmp(raw, "PBCK", 4) == 0) {
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
    printf("OK: %s (%u instructions, %u constants)\n", target_file, fn->code_len, fn->const_len);
    vm_free(&vm);
    return 0;
}

static int zr_cmd_build(const char *self_exe, int argc, char **argv) {
    const char *input_path = NULL;
    const char *out_path = NULL;
    const char *stdlib_override = NULL;
    const char *target_arch = NULL;
    bool bundle_exe = false;
    bool safe_flag = false;
    bool overflow_flag = false;

    for (int i = 0; i < argc; i++) {
        if (strcmp(argv[i], "--bundle-exe") == 0 || strcmp(argv[i], "--exe") == 0 || strcmp(argv[i], "--standalone") == 0) {
            bundle_exe = true;
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
        fprintf(stderr, "Usage: pblang build <input.sapp> [-o <output>] [--target <arch> | --bundle-exe]\n");
        return 1;
    }

    if (target_arch && strcmp(target_arch, "pbck") != 0) {
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
        if (!pbc_path[0]) {
            fprintf(stderr, "build --target %s: tools/pbc.sapp not found\n", target_arch);
            return 1;
        }
        ZrCaps pcaps;
        zr_caps_allow_all(&pcaps);
        char *pargs[10];
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
        return zr_cmd_run_file(self_exe, pbc_path, &pcaps, stdlib_override, pn, pargs);
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
    if (raw_len >= 4 && memcmp(raw, "PBCK", 4) == 0) {
        pbck_bytes = raw;
        pbck_len = raw_len;
        raw = NULL;
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
        pbck_bytes = pbck_serialize_script(fn, &pbck_len);
        vm_pop_roots(&vm, 1);
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
        printf("Built standalone executable: %s (%zu bytes bytecode embedded)\n", out_path, pbck_len);
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
        printf("Built bytecode: %s (%zu bytes)\n", out_path, pbck_len);
    }

    free(pbck_bytes);
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
        if (root_fn->code[i].op == OP_GET_GLOBAL) {
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

static int zr_cmd_test(const char *self_exe, int argc, char **argv) {
    ZrCaps caps;
    zr_caps_allow_all(&caps);
    bool sandbox_seen = false;
    const char *stdlib_override = NULL;
    const char *filter = NULL;
    bool quiet = false;
    ZrTestFileList inputs = { NULL, 0, 0 };

    for (int i = 0; i < argc; i++) {
        if (zr_is_cap_flag(argv[i])) {
            zr_apply_cap_flag(&caps, &sandbox_seen, argv[i]);
        } else if (strcmp(argv[i], "--stdlib") == 0 && i + 1 < argc) {
            stdlib_override = argv[++i];
        } else if (strcmp(argv[i], "--filter") == 0 && i + 1 < argc) {
            filter = argv[++i];
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

    size_t passed_units = 0, failed_units = 0;
    double t0 = zr_mono_sec();

    for (size_t fi = 0; fi < files.len; fi++) {
        const char *fpath = files.paths[fi];
        PbVM vm;
        vm_init(&vm);
        vm.caps = caps;
        zr_configure_vm_paths(&vm, self_exe, fpath, stdlib_override);
        vm.cli_argc = 1;
        vm.cli_argv = &files.paths[fi];

        char lerr[512];
        lerr[0] = '\0';
        PbFunc *fn = zr_load_or_compile_file(&vm, vm.current_file ? vm.current_file : fpath, lerr);
        if (!fn) {
            fprintf(stderr, "FAIL %s (compile): %s\n", fpath, lerr);
            failed_units++;
            vm_free(&vm);
            continue;
        }

        vm_push_root(&vm, val_obj(fn));
        zr_populate_polyglot_globals(&vm, vm.globals, fn);
        PbValue ret = val_nil();
        bool top_ok = vm_run_root_func(&vm, fn, NULL, &ret);
        if (!top_ok || (vm.exited && vm.exit_code != 0)) {
            fprintf(stderr, "FAIL %s: %s\n", fpath, vm.err_msg[0] ? vm.err_msg : "top-level exited non-zero");
            failed_units++;
            vm_pop_roots(&vm, 1);
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

        vm_pop_roots(&vm, 1);
        vm_free(&vm);
    }

    for (size_t i = 0; i < files.len; i++) free(files.paths[i]);
    free(files.paths);

    double elapsed_ms = (zr_mono_sec() - t0) * 1000.0;
    printf("Test summary: %zu passed, %zu failed (%.1f ms)\n", passed_units, failed_units, elapsed_ms);
    return failed_units == 0 ? 0 : 1;
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

int main(int argc, char **argv) {
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
    if (strcmp(sub, "check") == 0 || strcmp(sub, "--check") == 0) {
        const char *target = NULL;
        const char *stdlib_ov = NULL;
        for (int i = 2; i < argc; i++) {
            if (strcmp(argv[i], "--stdlib") == 0 && i + 1 < argc) stdlib_ov = argv[++i];
            else if (argv[i][0] != '-' && !target) target = argv[i];
        }
        if (!target) {
            fprintf(stderr, "Usage: pblang check <file.sapp>\n");
            free(self_exe);
            return 1;
        }
        int rc = zr_cmd_check_file(self_exe, target, stdlib_ov);
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
        } else if (strcmp(a, "--stdlib") == 0 && i + 1 < argc) {
            stdlib_override = argv[++i];
        } else if ((strcmp(a, "-e") == 0 || strcmp(a, "eval") == 0) && i + 1 < argc) {
            eval_code = argv[++i];
            script_arg_start = i + 1;
            for (int j = i + 1; j < argc; j++) {
                if (strcmp(argv[j], "--") == 0) { script_arg_start = j + 1; break; }
                if (zr_is_cap_flag(argv[j])) zr_apply_cap_flag(&caps, &sandbox_seen, argv[j]);
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
                else if (strcmp(argv[j], "--stdlib") == 0 && j + 1 < argc) stdlib_override = argv[++j];
            }
            break;
        }
    }

    if (eval_code) {
        int rc = zr_cmd_eval(self_exe, eval_code, &caps, stdlib_override,
                             argc - script_arg_start, argv + script_arg_start);
        free(self_exe);
        return rc;
    }

    if (!target_file) {
        fprintf(stderr, "Error: missing script path (try `pblang --help`)\n");
        free(self_exe);
        return 1;
    }

    int rc = zr_cmd_run_file(self_exe, target_file, &caps, stdlib_override,
                             argc - script_arg_start, argv + script_arg_start);
    free(self_exe);
    return rc;
}
