/* ============================================================================
 * pblang Sovereign Zero-Rust Native Runtime v2.0.0 — zr_loader.h
 * Automatic stdlib discovery, relative & package-root module resolution,
 * SHA-256 .pbck v1 disk cache, in-VM module cache, and polyglot block bindings.
 * ============================================================================ */
#ifndef PBLANG_ZR_LOADER_H
#define PBLANG_ZR_LOADER_H

#include "zr_compiler.h"

#define ZR_PBCK_CACHE_SALT "PBLANG-ZR-2.1.0-r1|"

static bool zr_file_exists_reg(const char *path) {
    if (!path || !path[0]) return false;
#if defined(_WIN32)
    DWORD attr = GetFileAttributesA(path);
    return (attr != INVALID_FILE_ATTRIBUTES) && !(attr & FILE_ATTRIBUTE_DIRECTORY);
#else
    struct stat st;
    return stat(path, &st) == 0 && S_ISREG(st.st_mode);
#endif
}

static bool zr_dir_exists(const char *path) {
    if (!path || !path[0]) return false;
#if defined(_WIN32)
    DWORD attr = GetFileAttributesA(path);
    return (attr != INVALID_FILE_ATTRIBUTES) && (attr & FILE_ATTRIBUTE_DIRECTORY);
#else
    struct stat st;
    return stat(path, &st) == 0 && S_ISDIR(st.st_mode);
#endif
}

static void zr_mkdir_single(const char *path) {
    if (!path || !path[0]) return;
#if defined(_WIN32)
    _mkdir(path);
#else
    mkdir(path, 0755);
#endif
}

static void zr_mkdirs_path(const char *path) {
    if (!path || !path[0]) return;
    char tmp[4096];
    snprintf(tmp, sizeof(tmp), "%s", path);
    size_t len = strlen(tmp);
    for (size_t i = 1; i < len; i++) {
        if (tmp[i] == '/' || tmp[i] == '\\') {
            char sv = tmp[i];
            tmp[i] = '\0';
            if (tmp[0] && !(i == 2 && tmp[1] == ':')) zr_mkdir_single(tmp);
            tmp[i] = sv;
        }
    }
    zr_mkdir_single(tmp);
}

static void zr_canonicalize_path(const char *in_path, char out[4096]) {
    if (!in_path || !in_path[0]) { out[0] = '\0'; return; }
#if defined(_WIN32)
    if (_fullpath(out, in_path, 4096)) return;
#elif !defined(__wasi__)
    char *rp = realpath(in_path, NULL);
    if (rp) {
        snprintf(out, 4096, "%s", rp);
        free(rp);
        return;
    }
#endif
    snprintf(out, 4096, "%s", in_path);
}

static void zr_path_parent_dir(const char *path, char out[4096]) {
    if (!path || !path[0]) { snprintf(out, 4096, "."); return; }
    snprintf(out, 4096, "%s", path);
    size_t len = strlen(out);
    while (len > 1 && (out[len - 1] == '/' || out[len - 1] == '\\')) out[--len] = '\0';
    char *s1 = strrchr(out, '/');
    char *s2 = strrchr(out, '\\');
    char *sep = (s1 && s2) ? (s1 > s2 ? s1 : s2) : (s1 ? s1 : s2);
    if (!sep) {
        snprintf(out, 4096, ".");
    } else if (sep == out) {
        out[1] = '\0';
    } else {
        *sep = '\0';
    }
}

static bool zr_read_entire_file(const char *path, uint8_t **out_buf, size_t *out_len) {
    FILE *fp = fopen(path, "rb");
    if (!fp) return false;
    long sz = -1;
    if (fseek(fp, 0, SEEK_END) == 0) {
        sz = ftell(fp);
        if (fseek(fp, 0, SEEK_SET) != 0) sz = -1;
    }
    if (sz > 0) {
        uint8_t *buf = (uint8_t *)zr_xmalloc((size_t)sz + 1);
        size_t got = fread(buf, 1, (size_t)sz, fp);
        if (ferror(fp)) {
            free(buf);
            fclose(fp);
            return false;
        }
        buf[got] = 0;
        fclose(fp);
        *out_buf = buf;
        *out_len = got;
        return true;
    }
    /* Stream read for empty files and virtual filesystems (procfs, sysfs, pipes) */
    size_t cap = 4096, len = 0;
    uint8_t *buf = (uint8_t *)zr_xmalloc(cap + 1);
    for (;;) {
        if (len + 4096 > cap) {
            cap *= 2;
            buf = (uint8_t *)zr_xrealloc(buf, cap + 1);
        }
        size_t n = fread(buf + len, 1, 4096, fp);
        len += n;
        if (n < 4096) {
            if (ferror(fp)) {
                free(buf);
                fclose(fp);
                return false;
            }
            break;
        }
    }
    buf[len] = 0;
    fclose(fp);
    *out_buf = buf;
    *out_len = len;
    return true;
}

static bool zr_is_valid_stdlib_dir(const char *dir) {
    if (!dir || !dir[0]) return false;
    char probe[4096];
    snprintf(probe, sizeof(probe), "%s%cmath.sapp", dir, ZR_PATH_SEP);
    return zr_file_exists_reg(probe);
}

static bool zr_probe_stdlib_upwards(const char *start_path, char out[4096]) {
    if (!start_path || !start_path[0]) return false;
    char cur[4096];
    if (zr_dir_exists(start_path)) zr_canonicalize_path(start_path, cur);
    else {
        char canon[4096];
        zr_canonicalize_path(start_path, canon);
        zr_path_parent_dir(canon, cur);
    }
    for (int depth = 0; depth < 8 && cur[0]; depth++) {
        char cand[4096];
        /* 1. <cur>/stdlib */
        snprintf(cand, sizeof(cand), "%.4050s%cstdlib", cur, ZR_PATH_SEP);
        if (zr_is_valid_stdlib_dir(cand)) { zr_canonicalize_path(cand, out); return true; }
        /* 2. <cur>/pblang/stdlib (sibling product checkout inside workspace) */
        snprintf(cand, sizeof(cand), "%.4050s%cpblang%cstdlib", cur, ZR_PATH_SEP, ZR_PATH_SEP);
        if (zr_is_valid_stdlib_dir(cand)) { zr_canonicalize_path(cand, out); return true; }
        /* 3. <cur>/share/pblang/stdlib (FHS install layout) */
        snprintf(cand, sizeof(cand), "%.4050s%cshare%cpblang%cstdlib", cur, ZR_PATH_SEP, ZR_PATH_SEP, ZR_PATH_SEP);
        if (zr_is_valid_stdlib_dir(cand)) { zr_canonicalize_path(cand, out); return true; }

        char parent[4096];
        zr_path_parent_dir(cur, parent);
        if (strcmp(parent, cur) == 0) break;
        snprintf(cur, sizeof(cur), "%s", parent);
    }
    return false;
}

static bool zr_probe_project_manifest_stdlib(const char *start_path, char out[4096]) {
    if (!start_path || !start_path[0]) return false;
    char cur[4096];
    if (zr_dir_exists(start_path)) zr_canonicalize_path(start_path, cur);
    else {
        char canon[4096];
        zr_canonicalize_path(start_path, canon);
        zr_path_parent_dir(canon, cur);
    }
    for (int depth = 0; depth < 8 && cur[0]; depth++) {
        char toml_p[4096], lock_p[4096], cand[4096];
        snprintf(toml_p, sizeof(toml_p), "%.4050s%cpblang.toml", cur, ZR_PATH_SEP);
        snprintf(lock_p, sizeof(lock_p), "%.4050s%cpblang.lock", cur, ZR_PATH_SEP);
        bool has_toml = zr_file_exists_reg(toml_p);
        bool has_lock = zr_file_exists_reg(lock_p);
        if (has_toml || has_lock) {
            snprintf(cand, sizeof(cand), "%.4050s%cstdlib", cur, ZR_PATH_SEP);
            if (zr_is_valid_stdlib_dir(cand)) { zr_canonicalize_path(cand, out); return true; }
            if (has_lock) {
                uint8_t *buf = NULL; size_t len = 0;
                if (zr_read_entire_file(lock_p, &buf, &len)) {
                    char *hit = strstr((char *)buf, "name = \"stdlib\"");
                    if (hit) {
                        char *pkw = strstr(hit, "path = \"");
                        char *nxt = strstr(hit + 15, "[[");
                        if (pkw && (!nxt || pkw < nxt)) {
                            pkw += 8;
                            char *qe = strchr(pkw, '"');
                            if (qe && (size_t)(qe - pkw) < 2048) {
                                char rel[2048];
                                memcpy(rel, pkw, (size_t)(qe - pkw));
                                rel[qe - pkw] = '\0';
                                if (rel[0] == '/' || (rel[0] && rel[1] == ':')) {
                                    snprintf(cand, sizeof(cand), "%s", rel);
                                } else {
                                    snprintf(cand, sizeof(cand), "%.2000s%c%.2000s", cur, ZR_PATH_SEP, rel);
                                }
                                if (zr_is_valid_stdlib_dir(cand)) {
                                    free(buf);
                                    zr_canonicalize_path(cand, out);
                                    return true;
                                }
                            }
                        }
                    }
                    free(buf);
                }
            }
        }
        char parent[4096];
        zr_path_parent_dir(cur, parent);
        if (strcmp(parent, cur) == 0) break;
        snprintf(cur, sizeof(cur), "%s", parent);
    }
    return false;
}

static void zr_discover_stdlib_root(const char *exe_path, const char *script_path, char out[4096]) {
    out[0] = '\0';
    const char *env_std = getenv("PBLANG_STDLIB");
    if (env_std && env_std[0] && zr_is_valid_stdlib_dir(env_std)) {
        zr_canonicalize_path(env_std, out);
        return;
    }
    if (script_path && zr_probe_project_manifest_stdlib(script_path, out)) return;

    char cwd_buf[4096];
    bool have_cwd = false;
#if defined(_WIN32)
    if (_getcwd(cwd_buf, sizeof(cwd_buf))) have_cwd = true;
#else
    if (getcwd(cwd_buf, sizeof(cwd_buf))) have_cwd = true;
#endif
    if (have_cwd && zr_probe_project_manifest_stdlib(cwd_buf, out)) return;

    if (exe_path && zr_probe_stdlib_upwards(exe_path, out)) return;
    if (script_path && zr_probe_stdlib_upwards(script_path, out)) return;
    if (have_cwd && zr_probe_stdlib_upwards(cwd_buf, out)) return;
}

static bool zr_try_resolve_sapp_or_pbck(const char *base, char out[4096]) {
    if (zr_file_exists_reg(base)) {
        zr_canonicalize_path(base, out);
        return true;
    }
    char cand[4096];
    snprintf(cand, sizeof(cand), "%.4070s.sapp", base);
    if (zr_file_exists_reg(cand)) {
        zr_canonicalize_path(cand, out);
        return true;
    }
    snprintf(cand, sizeof(cand), "%.4070s%cmod.sapp", base, ZR_PATH_SEP);
    if (zr_file_exists_reg(cand)) {
        zr_canonicalize_path(cand, out);
        return true;
    }
    snprintf(cand, sizeof(cand), "%.4070s.pbck", base);
    if (zr_file_exists_reg(cand)) {
        zr_canonicalize_path(cand, out);
        return true;
    }
    return false;
}

/* Parse pblang.lock if present in package root to resolve sibling package paths */
static bool zr_try_resolve_lockfile_dep(const char *pkg_root, const char *spec, char out[4096]) {
    char lock_path[4096];
    snprintf(lock_path, sizeof(lock_path), "%.4070s%cpblang.lock", pkg_root, ZR_PATH_SEP);
    uint8_t *buf = NULL; size_t len = 0;
    if (!zr_read_entire_file(lock_path, &buf, &len)) return false;

    /* Extract leading package segment `<dep_name>/<rest>` or `<dep_name>` */
    const char *slash = strchr(spec, '/');
    char dep_name[256];
    const char *sub = "";
    if (slash) {
        size_t n = (size_t)(slash - spec);
        if (n >= sizeof(dep_name)) { free(buf); return false; }
        memcpy(dep_name, spec, n);
        dep_name[n] = '\0';
        sub = slash + 1;
    } else {
        snprintf(dep_name, sizeof(dep_name), "%s", spec);
    }

    /* Simple TOML scan for `name = "<dep_name>"` followed by `path = "<rel_or_abs>"` */
    char needle[300];
    snprintf(needle, sizeof(needle), "name = \"%s\"", dep_name);
    char *hit = strstr((char *)buf, needle);
    if (hit) {
        char *path_kw = strstr(hit, "path = \"");
        char *next_pkg = strstr(hit + strlen(needle), "[[");
        if (path_kw && (!next_pkg || path_kw < next_pkg)) {
            path_kw += 8;
            char *qend = strchr(path_kw, '"');
            if (qend && (size_t)(qend - path_kw) < 2048) {
                char dep_path[2048];
                memcpy(dep_path, path_kw, (size_t)(qend - path_kw));
                dep_path[qend - path_kw] = '\0';
                char base_dir[4096];
                if (dep_path[0] == '/' || (dep_path[0] && dep_path[1] == ':')) {
                    snprintf(base_dir, sizeof(base_dir), "%s", dep_path);
                } else {
                    snprintf(base_dir, sizeof(base_dir), "%.2000s%c%.2000s", pkg_root, ZR_PATH_SEP, dep_path);
                }
                char target[4096];
                if (sub[0]) {
                    snprintf(target, sizeof(target), "%.2000s%c%.2000s", base_dir, ZR_PATH_SEP, sub);
                    if (zr_try_resolve_sapp_or_pbck(target, out)) { free(buf); return true; }
                    snprintf(target, sizeof(target), "%.2000s%csrc%c%.2000s", base_dir, ZR_PATH_SEP, ZR_PATH_SEP, sub);
                    if (zr_try_resolve_sapp_or_pbck(target, out)) { free(buf); return true; }
                } else {
                    snprintf(target, sizeof(target), "%.4000s%csrc%clib.sapp", base_dir, ZR_PATH_SEP, ZR_PATH_SEP);
                    if (zr_try_resolve_sapp_or_pbck(target, out)) { free(buf); return true; }
                }
            }
        }
    }
    free(buf);
    return false;
}

static bool zr_resolve_import_path(PbVM *vm, const char *spec, const char *caller_file, char out_path[4096]) {
    if (!spec || !spec[0]) {
        vm_raise(vm, "import(): empty module specifier");
        return false;
    }

    /* 1. stdlib:<name> or stdlib/<name> */
    bool is_stdlib = (strncmp(spec, "stdlib:", 7) == 0) || (strncmp(spec, "stdlib/", 7) == 0);
    if (is_stdlib) {
        if (!vm->caps.allow_stdlib_import) {
            vm_raise(vm, "stdlib import access denied (pass --allow-stdlib or --allow-all)");
            return false;
        }
        const char *sub = spec + 7;
        if (strstr(sub, "..") != NULL) {
            vm_raise(vm, "import(): '..' traversal denied in stdlib specifier '%s'", spec);
            return false;
        }
        if (!vm->stdlib_root || !vm->stdlib_root[0]) {
            char disc[4096];
            zr_discover_stdlib_root(NULL, caller_file, disc);
            if (disc[0]) {
                free(vm->stdlib_root);
                vm->stdlib_root = zr_strdup(disc);
            }
        }
        if (!vm->stdlib_root || !vm->stdlib_root[0]) {
            vm_raise(vm, "import('%s'): could not locate stdlib directory (set PBLANG_STDLIB)", spec);
            return false;
        }
        char cand[4096];
        snprintf(cand, sizeof(cand), "%.2000s%c%.2000s", vm->stdlib_root, ZR_PATH_SEP, sub);
        if (zr_try_resolve_sapp_or_pbck(cand, out_path)) return true;
        if (strncmp(spec, "stdlib/", 7) != 0 || !vm->caps.allow_import) {
            vm_raise(vm, "import('%s'): stdlib module not found under '%s'", spec, vm->stdlib_root);
            return false;
        }
    }

    /* 2. User / project import */
    if (!vm->caps.allow_import) {
        vm_raise(vm, "import access denied for '%s' (pass --allow-import or --allow-all)", spec);
        return false;
    }

    /* Direct / absolute path check */
    if (spec[0] == '/' || (spec[0] && spec[1] == ':')) {
        if (zr_try_resolve_sapp_or_pbck(spec, out_path)) return true;
    }

    /* Relative to caller's directory, then walk up to nearest package root (`pblang.toml`) */
    if (caller_file && caller_file[0] && caller_file[0] != '<') {
        char caller_canon[4096], caller_dir[4096];
        zr_canonicalize_path(caller_file, caller_canon);
        zr_path_parent_dir(caller_canon, caller_dir);

        char cand[4096];
        snprintf(cand, sizeof(cand), "%.2000s%c%.2000s", caller_dir, ZR_PATH_SEP, spec);
        if (zr_try_resolve_sapp_or_pbck(cand, out_path)) return true;

        /* Walk ancestors looking for pblang.toml package root */
        char cur[4096];
        snprintf(cur, sizeof(cur), "%s", caller_dir);
        for (int depth = 0; depth < 8 && cur[0]; depth++) {
            char manifest[4096];
            snprintf(manifest, sizeof(manifest), "%.4000s%cpblang.toml", cur, ZR_PATH_SEP);
            if (zr_file_exists_reg(manifest)) {
                snprintf(cand, sizeof(cand), "%.2000s%c%.2000s", cur, ZR_PATH_SEP, spec);
                if (zr_try_resolve_sapp_or_pbck(cand, out_path)) return true;
                snprintf(cand, sizeof(cand), "%.2000s%csrc%c%.2000s", cur, ZR_PATH_SEP, ZR_PATH_SEP, spec);
                if (zr_try_resolve_sapp_or_pbck(cand, out_path)) return true;
                if (zr_try_resolve_lockfile_dep(cur, spec, out_path)) return true;
                /* Also check sibling workspace directory `../<spec>` */
                char ws_parent[4096];
                zr_path_parent_dir(cur, ws_parent);
                if (ws_parent[0] && strcmp(ws_parent, cur) != 0) {
                    snprintf(cand, sizeof(cand), "%.2000s%c%.2000s", ws_parent, ZR_PATH_SEP, spec);
                    if (zr_try_resolve_sapp_or_pbck(cand, out_path)) return true;
                }
            }
            char parent[4096];
            zr_path_parent_dir(cur, parent);
            if (strcmp(parent, cur) == 0) break;
            snprintf(cur, sizeof(cur), "%s", parent);
        }
    }

    /* Relative to CWD */
    if (zr_try_resolve_sapp_or_pbck(spec, out_path)) return true;

    /* Fallback: if spec matches a stdlib module name without prefix and allow_stdlib_import is true */
    if (vm->caps.allow_stdlib_import && vm->stdlib_root && vm->stdlib_root[0] && strstr(spec, "..") == NULL) {
        char cand[4096];
        snprintf(cand, sizeof(cand), "%s%c%s", vm->stdlib_root, ZR_PATH_SEP, spec);
        if (zr_try_resolve_sapp_or_pbck(cand, out_path)) return true;
    }

    vm_raise(vm, "import('%s'): module file not found", spec);
    return false;
}

/* Compute disk cache path for `.pbck` keyed by SHA-256 of compiler version + source bytes */
static bool zr_get_pbck_cache_path(const uint8_t *src, size_t src_len, char out_cache_file[4096]) {
    const char *disable = getenv("PBLANG_NO_CACHE");
    if (disable && strcmp(disable, "1") == 0) return false;

    char cache_root[3800];
    cache_root[0] = '\0';
#if defined(_WIN32)
    const char *la = getenv("LOCALAPPDATA");
    if (la && la[0]) snprintf(cache_root, sizeof(cache_root), "%s\\pblang\\zr_pbck_v2", la);
#else
    const char *xdg = getenv("XDG_CACHE_HOME");
    if (xdg && xdg[0]) {
        snprintf(cache_root, sizeof(cache_root), "%s/pblang/zr_pbck_v2", xdg);
    } else {
        const char *hm = getenv("HOME");
        if (hm && hm[0]) snprintf(cache_root, sizeof(cache_root), "%s/.cache/pblang/zr_pbck_v2", hm);
    }
#endif
    if (!cache_root[0]) return false;
    zr_mkdirs_path(cache_root);

    ZrSha256Ctx ctx;
    zr_sha256_init(&ctx);
    zr_sha256_update(&ctx, ZR_PBCK_CACHE_SALT, strlen(ZR_PBCK_CACHE_SALT));
    zr_sha256_update(&ctx, src, src_len);
    uint8_t digest[32];
    zr_sha256_final(&ctx, digest);
    char hex[65];
    zr_hex_encode_raw(digest, 32, hex);

    snprintf(out_cache_file, 4096, "%s%c%s.pbck", cache_root, ZR_PATH_SEP, hex);
    return true;
}

/* Populate `__polyglot_blocks` and `__polyglot` globals from compiled PbFunc metadata */
static void zr_populate_polyglot_globals(PbVM *vm, PbMap *target_globals, const PbFunc *fn) {
    PbArray *blocks_arr = vm_new_array(vm, fn->poly_count);
    vm_temp_push(vm, val_obj(blocks_arr));
    PbMap *by_name = vm_new_map(vm, 8);
    vm_temp_push(vm, val_obj(by_name));
    PbMap *by_lang = vm_new_map(vm, 8);
    vm_temp_push(vm, val_obj(by_lang));

    for (size_t i = 0; i < fn->poly_count; i++) {
        const PbPolyBlock *pb = &fn->poly_blocks[i];
        PbMap *bm = vm_new_map(vm, 8);
        vm_temp_push(vm, val_obj(bm));
        PbStr *s_lang = pb_str_c(vm, pb->lang ? pb->lang : "");
        PbStr *s_name = pb_str_c(vm, pb->name ? pb->name : "");
        PbStr *s_code = pb_str_c(vm, pb->code ? pb->code : "");
        PbMap *attrs_m = vm_new_map(vm, pb->attr_count);
        vm_temp_push(vm, val_obj(attrs_m));
        for (size_t a = 0; a < pb->attr_count; a++) {
            map_set(attrs_m, pb_str_c(vm, pb->attr_keys[a]), val_obj(pb_str_c(vm, pb->attr_vals[a])));
        }

        map_set(bm, vm_const_str(vm, "lang"), val_obj(s_lang));
        map_set(bm, vm_const_str(vm, "name"), val_obj(s_name));
        map_set(bm, vm_const_str(vm, "code"), val_obj(s_code));
        map_set(bm, vm_const_str(vm, "attrs"), val_obj(attrs_m));
        map_set(bm, vm_const_str(vm, "line"), val_num((double)pb->line));
        arr_push(blocks_arr, val_obj(bm));

        if (s_name->len > 0) map_set(by_name, s_name, val_obj(bm));

        PbValue lang_list_v;
        PbArray *lang_list = NULL;
        if (map_get(by_lang, s_lang, &lang_list_v) && val_is_obj(lang_list_v, OBJ_ARRAY)) {
            lang_list = (PbArray *)val_as_obj(lang_list_v);
        } else {
            lang_list = vm_new_array(vm, 4);
            map_set(by_lang, s_lang, val_obj(lang_list));
        }
        arr_push(lang_list, val_obj(bm));
        vm_temp_pop(vm, 2);
    }

    PbMap *poly_root = vm_new_map(vm, 8);
    vm_temp_push(vm, val_obj(poly_root));
    map_set(poly_root, vm_const_str(vm, "blocks"), val_obj(blocks_arr));
    map_set(poly_root, vm_const_str(vm, "by_name"), val_obj(by_name));
    map_set(poly_root, vm_const_str(vm, "by_lang"), val_obj(by_lang));

    map_set(target_globals, vm_const_str(vm, "__polyglot_blocks"), val_obj(blocks_arr));
    map_set(target_globals, vm_const_str(vm, "__polyglot"), val_obj(poly_root));
    vm_temp_pop(vm, 4);
}

/* Load either `.pbck` binary or `.sapp` source (using SHA-256 `.pbck` disk cache) */
static PbFunc *zr_load_or_compile_file(PbVM *vm, const char *path, char err_buf[512]) {
    uint8_t *raw = NULL;
    size_t raw_len = 0;
    if (!zr_read_entire_file(path, &raw, &raw_len)) {
        snprintf(err_buf, 512, "cannot read file '%s'", path);
        return NULL;
    }

    /* Direct .pbck file check */
    if (raw_len >= 9 && memcmp(raw, PBCK_MAGIC_BYTES, 4) == 0) {
        char derr[256];
        derr[0] = '\0';
        PbFunc *fn = pbck_deserialize_script(vm, raw, raw_len, derr);
        if (!fn) snprintf(err_buf, 512, "%s", derr);
        else if (!fn->source_name || fn->source_name->len == 0) fn->source_name = vm_new_cstr(vm, path);
        free(raw);
        return fn;
    }

    /* Check SHA-256 .pbck disk cache (only when source has no @polyglot blocks so polyglot metadata stays fresh) */
    bool has_at_poly = (memchr(raw, '@', raw_len) != NULL);
    char cache_file[4096];
    bool have_cache_path = (!has_at_poly) && zr_get_pbck_cache_path(raw, raw_len, cache_file);
    if (have_cache_path && zr_file_exists_reg(cache_file)) {
        uint8_t *cbuf = NULL;
        size_t clen = 0;
        if (zr_read_entire_file(cache_file, &cbuf, &clen)) {
            char cerr[256];
            PbFunc *cached_fn = pbck_deserialize_script(vm, cbuf, clen, cerr);
            free(cbuf);
            if (cached_fn) {
                cached_fn->source_name = vm_new_cstr(vm, path);
                free(raw);
                return cached_fn;
            }
        }
    }

    /* Compile .sapp source directly in C99 */
    PbFunc *fn = zr_compile_sapp(vm, (const char *)raw, raw_len, path, err_buf);
    free(raw);
    if (!fn) return NULL;

    /* Persist compiled .pbck to SHA-256 disk cache (atomic temp + rename) */
    if (have_cache_path) {
        vm_temp_push(vm, val_obj(fn));
        size_t plen = 0;
        uint8_t *pbytes = pbck_serialize_script(fn, &plen);
        if (pbytes && plen > 0) {
            char tmp_path[4128];
            snprintf(tmp_path, sizeof(tmp_path), "%s.tmp.%u", cache_file, (unsigned)((uint64_t)zr_now_ms() & 0xffffu));
            FILE *cf = fopen(tmp_path, "wb");
            if (cf) {
                if (fwrite(pbytes, 1, plen, cf) == plen) {
                    fclose(cf);
#if defined(_WIN32)
                    MoveFileExA(tmp_path, cache_file, MOVEFILE_REPLACE_EXISTING);
#else
                    rename(tmp_path, cache_file);
#endif
                } else {
                    fclose(cf);
                    remove(tmp_path);
                }
            }
        }
        free(pbytes);
        vm_temp_pop(vm, 1);
    }
    return fn;
}

static bool zr_vm_import_module(PbVM *vm, const char *spec, const char *caller_file, PbValue *out_mod) {
    char resolved[4096];
    if (!zr_resolve_import_path(vm, spec, caller_file, resolved)) return false;

    PbStr *key = vm_new_cstr(vm, resolved);
    vm_temp_push(vm, val_obj(key));

    PbValue existing;
    if (map_get(vm->modules, key, &existing) && val_is_obj(existing, OBJ_MODULE)) {
        PbModule *m = (PbModule *)val_as_obj(existing);
        if (m->loading) {
            vm_temp_pop(vm, 1);
            vm_raise(vm, "circular import detected while loading '%s'", resolved);
            return false;
        }
        if (m->loaded) {
            vm_temp_pop(vm, 1);
            *out_mod = val_obj(m);
            return true;
        }
    }

    char err_buf[512];
    err_buf[0] = '\0';
    PbFunc *fn = zr_load_or_compile_file(vm, resolved, err_buf);
    if (!fn) {
        vm_temp_pop(vm, 1);
        vm_raise(vm, "import('%s'): %s", spec, err_buf);
        return false;
    }
    vm_temp_push(vm, val_obj(fn));

    PbModule *mod = vm_new_module(vm, key);
    mod->loading = true;
    map_set(vm->modules, key, val_obj(mod));
    zr_populate_polyglot_globals(vm, mod->globals, fn);

    bool ok = vm_run_root_func(vm, fn, mod, NULL);
    vm_temp_pop(vm, 2); /* fn, key */
    if (!ok) {
        mod->loading = false;
        map_delete(vm->modules, key);
        return false;
    }
    mod->loading = false;
    mod->loaded = true;
    *out_mod = val_obj(mod);
    return true;
}

static bool bi_import(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 1) { vm_raise(vm, "import() expects 1 argument, got %d", argc); return false; }
    if (!val_is_obj(argv[0], OBJ_STR)) { vm_raise(vm, "import(): expected module path string"); return false; }
    const char *spec = ((PbStr *)val_as_obj(argv[0]))->chars;

    const char *caller_file = NULL;
    if (vm->frame_count > 0) {
        PbFunc *cf = vm->frames[vm->frame_count - 1].closure->fn;
        if (cf && cf->source_name) caller_file = cf->source_name->chars;
    }
    if (!caller_file) caller_file = vm->entry_script_path;
    return zr_vm_import_module(vm, spec, caller_file, out);
}

#endif /* PBLANG_ZR_LOADER_H */
