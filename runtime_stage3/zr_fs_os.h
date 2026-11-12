/* ============================================================================
 * pblang Sovereign Zero-Rust Native Runtime v2.0.0 — zr_fs_os.h
 * Filesystem, Path, Environment, Process Spawning (execvp / CreateProcessA),
 * Async-Signal-Safe Signal Trapping, Time, and Hardware Introspection.
 * ============================================================================ */
#ifndef PBLANG_ZR_FS_OS_H
#define PBLANG_ZR_FS_OS_H

#include "zr_loader.h"
#include <signal.h>

#if !defined(_WIN32)
#include <sys/sysinfo.h>
#endif

/* ---- Async-Signal-Safe Pending Signal Table ---- */
static volatile sig_atomic_t g_zr_sig_pending[64];

static void zr_c_signal_handler(int sig) {
    if (sig > 0 && sig < 64) g_zr_sig_pending[sig]++;
}

static int zr_resolve_signal_num(const char *raw_name, char canon_out[32]) {
    char up[32];
    size_t i = 0;
    while (*raw_name == ' ' || *raw_name == '\t') raw_name++;
    for (; raw_name[i] && i < 28; i++) {
        char c = raw_name[i];
        up[i] = (c >= 'a' && c <= 'z') ? (char)(c - 32) : c;
    }
    up[i] = '\0';
    const char *core = (strncmp(up, "SIG", 3) == 0) ? (up + 3) : up;
    snprintf(canon_out, 32, "SIG%.28s", core);

    if (strcmp(core, "INT") == 0)  return SIGINT;
    if (strcmp(core, "TERM") == 0) return SIGTERM;
#ifdef SIGHUP
    if (strcmp(core, "HUP") == 0)  return SIGHUP;
#endif
#ifdef SIGUSR1
    if (strcmp(core, "USR1") == 0) return SIGUSR1;
#endif
#ifdef SIGUSR2
    if (strcmp(core, "USR2") == 0) return SIGUSR2;
#endif
#ifdef SIGQUIT
    if (strcmp(core, "QUIT") == 0) return SIGQUIT;
#endif
#ifdef SIGALRM
    if (strcmp(core, "ALRM") == 0) return SIGALRM;
#endif
#ifdef SIGCHLD
    if (strcmp(core, "CHLD") == 0) return SIGCHLD;
#endif
#ifdef SIGPIPE
    if (strcmp(core, "PIPE") == 0) return SIGPIPE;
#endif
    return -1;
}

/* ---- Filesystem Builtins ---- */
static bool bi_fs_read(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (!vm_check_cap(vm, vm->caps.allow_fs, "fs_read", "fs")) return false;
    if (argc != 1 || !val_is_obj(argv[0], OBJ_STR)) { vm_raise(vm, "fs_read() expects 1 path string"); return false; }
    const char *path = ((PbStr *)val_as_obj(argv[0]))->chars;
    uint8_t *buf = NULL; size_t len = 0;
    if (!zr_read_entire_file(path, &buf, &len)) {
        vm_raise(vm, "fs_read('%s'): %s", path, strerror(errno));
        return false;
    }
    *out = val_obj(vm_new_str(vm, (const char *)buf, len));
    free(buf);
    return true;
}

static bool bi_fs_read_bytes(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (!vm_check_cap(vm, vm->caps.allow_fs, "fs_read_bytes", "fs")) return false;
    if (argc != 1 || !val_is_obj(argv[0], OBJ_STR)) { vm_raise(vm, "fs_read_bytes() expects 1 path string"); return false; }
    const char *path = ((PbStr *)val_as_obj(argv[0]))->chars;
    uint8_t *buf = NULL; size_t len = 0;
    if (!zr_read_entire_file(path, &buf, &len)) {
        vm_raise(vm, "fs_read_bytes('%s'): %s", path, strerror(errno));
        return false;
    }
    PbArray *arr = vm_new_array(vm, len);
    for (size_t i = 0; i < len; i++) arr_push(arr, val_num((double)buf[i]));
    free(buf);
    *out = val_obj(arr);
    return true;
}

static bool bi_fs_write(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (!vm_check_cap(vm, vm->caps.allow_fs, "fs_write", "fs")) return false;
    if (argc != 2 || !val_is_obj(argv[0], OBJ_STR) || !val_is_obj(argv[1], OBJ_STR)) {
        vm_raise(vm, "fs_write(path, content) expects 2 string arguments");
        return false;
    }
    const char *path = ((PbStr *)val_as_obj(argv[0]))->chars;
    PbStr *content = (PbStr *)val_as_obj(argv[1]);
    FILE *fp = fopen(path, "wb");
    if (!fp) { vm_raise(vm, "fs_write('%s'): %s", path, strerror(errno)); return false; }
    if (content->len > 0 && fwrite(content->chars, 1, content->len, fp) != content->len) {
        fclose(fp);
        vm_raise(vm, "fs_write('%s'): short write", path);
        return false;
    }
    fclose(fp);
    *out = val_nil();
    return true;
}

static bool bi_fs_write_bytes(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (!vm_check_cap(vm, vm->caps.allow_fs, "fs_write_bytes", "fs")) return false;
    if (argc != 2 || !val_is_obj(argv[0], OBJ_STR) || !val_is_obj(argv[1], OBJ_ARRAY)) {
        vm_raise(vm, "fs_write_bytes(path, byte_array) expects (string, array)");
        return false;
    }
    const char *path = ((PbStr *)val_as_obj(argv[0]))->chars;
    PbArray *arr = (PbArray *)val_as_obj(argv[1]);
    uint8_t *buf = (uint8_t *)zr_xmalloc(arr->len > 0 ? arr->len : 1);
    for (size_t i = 0; i < arr->len; i++) {
        PbValue v = arr->items[i];
        if (v.tag != VAL_NUM || !isfinite(v.u.num) || v.u.num < 0.0 || v.u.num > 255.0) {
            free(buf);
            vm_raise(vm, "fs_write_bytes(): byte values must be numbers 0..255");
            return false;
        }
        buf[i] = (uint8_t)v.u.num;
    }
    FILE *fp = fopen(path, "wb");
    if (!fp) { free(buf); vm_raise(vm, "fs_write_bytes('%s'): %s", path, strerror(errno)); return false; }
    if (arr->len > 0 && fwrite(buf, 1, arr->len, fp) != arr->len) {
        free(buf);
        fclose(fp);
        vm_raise(vm, "fs_write_bytes('%s'): short write", path);
        return false;
    }
    free(buf);
    fclose(fp);
    *out = val_nil();
    return true;
}

static bool bi_fs_append(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (!vm_check_cap(vm, vm->caps.allow_fs, "fs_append", "fs")) return false;
    if (argc != 2 || !val_is_obj(argv[0], OBJ_STR) || !val_is_obj(argv[1], OBJ_STR)) {
        vm_raise(vm, "fs_append(path, content) expects 2 string arguments");
        return false;
    }
    const char *path = ((PbStr *)val_as_obj(argv[0]))->chars;
    PbStr *content = (PbStr *)val_as_obj(argv[1]);
    FILE *fp = fopen(path, "ab");
    if (!fp) { vm_raise(vm, "fs_append('%s'): %s", path, strerror(errno)); return false; }
    if (content->len > 0 && fwrite(content->chars, 1, content->len, fp) != content->len) {
        fclose(fp);
        vm_raise(vm, "fs_append('%s'): short write", path);
        return false;
    }
    fclose(fp);
    *out = val_nil();
    return true;
}

static bool bi_fs_exists(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (!vm_check_cap(vm, vm->caps.allow_fs, "fs_exists", "fs")) return false;
    if (argc != 1 || !val_is_obj(argv[0], OBJ_STR)) { vm_raise(vm, "fs_exists() expects 1 path string"); return false; }
    const char *path = ((PbStr *)val_as_obj(argv[0]))->chars;
#if defined(_WIN32)
    DWORD attr = GetFileAttributesA(path);
    *out = val_bool(attr != INVALID_FILE_ATTRIBUTES);
#else
    struct stat st;
    *out = val_bool(stat(path, &st) == 0);
#endif
    return true;
}

static bool zr_remove_recursive(const char *path) {
#if defined(_WIN32)
    DWORD attr = GetFileAttributesA(path);
    if (attr == INVALID_FILE_ATTRIBUTES) return false;
    if (!(attr & FILE_ATTRIBUTE_DIRECTORY)) return DeleteFileA(path) != 0;
    char pat[4096];
    snprintf(pat, sizeof(pat), "%s\\*", path);
    WIN32_FIND_DATAA fd;
    HANDLE h = FindFirstFileA(pat, &fd);
    if (h != INVALID_HANDLE_VALUE) {
        do {
            if (strcmp(fd.cFileName, ".") == 0 || strcmp(fd.cFileName, "..") == 0) continue;
            char sub[4096];
            snprintf(sub, sizeof(sub), "%s\\%s", path, fd.cFileName);
            zr_remove_recursive(sub);
        } while (FindNextFileA(h, &fd));
        FindClose(h);
    }
    return RemoveDirectoryA(path) != 0;
#else
    struct stat st;
    if (lstat(path, &st) != 0) return false;
    if (!S_ISDIR(st.st_mode)) return unlink(path) == 0;
    DIR *d = opendir(path);
    if (!d) return false;
    struct dirent *de;
    while ((de = readdir(d)) != NULL) {
        if (strcmp(de->d_name, ".") == 0 || strcmp(de->d_name, "..") == 0) continue;
        char sub[4096];
        snprintf(sub, sizeof(sub), "%s/%s", path, de->d_name);
        zr_remove_recursive(sub);
    }
    closedir(d);
    return rmdir(path) == 0;
#endif
}

static bool bi_fs_remove(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (!vm_check_cap(vm, vm->caps.allow_fs, "fs_remove", "fs")) return false;
    if (argc != 1 || !val_is_obj(argv[0], OBJ_STR)) { vm_raise(vm, "fs_remove() expects 1 path string"); return false; }
    const char *path = ((PbStr *)val_as_obj(argv[0]))->chars;
    if (!zr_remove_recursive(path)) {
        vm_raise(vm, "fs_remove('%s'): failed to remove path", path);
        return false;
    }
    *out = val_nil();
    return true;
}

static bool bi_fs_rename(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (!vm_check_cap(vm, vm->caps.allow_fs, "fs_rename", "fs")) return false;
    if (argc != 2 || !val_is_obj(argv[0], OBJ_STR) || !val_is_obj(argv[1], OBJ_STR)) {
        vm_raise(vm, "fs_rename(src, dst) expects 2 string arguments");
        return false;
    }
    const char *src = ((PbStr *)val_as_obj(argv[0]))->chars;
    const char *dst = ((PbStr *)val_as_obj(argv[1]))->chars;
#if defined(_WIN32)
    if (!MoveFileExA(src, dst, MOVEFILE_REPLACE_EXISTING)) {
        vm_raise(vm, "fs_rename('%s', '%s') failed", src, dst);
        return false;
    }
#else
    if (rename(src, dst) != 0) {
        vm_raise(vm, "fs_rename('%s', '%s'): %s", src, dst, strerror(errno));
        return false;
    }
#endif
    *out = val_nil();
    return true;
}

static bool bi_fs_copy(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (!vm_check_cap(vm, vm->caps.allow_fs, "fs_copy", "fs")) return false;
    if (argc != 2 || !val_is_obj(argv[0], OBJ_STR) || !val_is_obj(argv[1], OBJ_STR)) {
        vm_raise(vm, "fs_copy(src, dst) expects 2 string arguments");
        return false;
    }
    const char *src = ((PbStr *)val_as_obj(argv[0]))->chars;
    const char *dst = ((PbStr *)val_as_obj(argv[1]))->chars;
    uint8_t *buf = NULL; size_t len = 0;
    if (!zr_read_entire_file(src, &buf, &len)) {
        vm_raise(vm, "fs_copy(): cannot read '%s': %s", src, strerror(errno));
        return false;
    }
    FILE *fp = fopen(dst, "wb");
    if (!fp) { free(buf); vm_raise(vm, "fs_copy(): cannot open '%s': %s", dst, strerror(errno)); return false; }
    if (len > 0 && fwrite(buf, 1, len, fp) != len) {
        free(buf); fclose(fp);
        vm_raise(vm, "fs_copy(): short write to '%s'", dst);
        return false;
    }
    free(buf);
    fclose(fp);
    *out = val_num((double)len);
    return true;
}

static bool bi_fs_mkdirs(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (!vm_check_cap(vm, vm->caps.allow_fs, "fs_mkdirs", "fs")) return false;
    if (argc != 1 || !val_is_obj(argv[0], OBJ_STR)) { vm_raise(vm, "fs_mkdirs() expects 1 path string"); return false; }
    const char *path = ((PbStr *)val_as_obj(argv[0]))->chars;
    zr_mkdirs_path(path);
    if (!zr_dir_exists(path)) {
        vm_raise(vm, "fs_mkdirs('%s'): failed to create directory", path);
        return false;
    }
    *out = val_nil();
    return true;
}

static int zr_cstr_ptr_cmp(const void *a, const void *b) {
    const PbValue *va = (const PbValue *)a;
    const PbValue *vb = (const PbValue *)b;
    return strcmp(((PbStr *)va->u.obj)->chars, ((PbStr *)vb->u.obj)->chars);
}

static bool bi_fs_listdir(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (!vm_check_cap(vm, vm->caps.allow_fs, "fs_listdir", "fs")) return false;
    if (argc != 1 || !val_is_obj(argv[0], OBJ_STR)) { vm_raise(vm, "fs_listdir() expects 1 path string"); return false; }
    const char *path = ((PbStr *)val_as_obj(argv[0]))->chars;
    PbArray *arr = vm_new_array(vm, 16);
    vm_temp_push(vm, val_obj(arr));
#if defined(_WIN32)
    char pat[4096];
    snprintf(pat, sizeof(pat), "%s\\*", path);
    WIN32_FIND_DATAA fd;
    HANDLE h = FindFirstFileA(pat, &fd);
    if (h == INVALID_HANDLE_VALUE) {
        vm_temp_pop(vm, 1);
        vm_raise(vm, "fs_listdir('%s'): directory not found", path);
        return false;
    }
    do {
        if (strcmp(fd.cFileName, ".") == 0 || strcmp(fd.cFileName, "..") == 0) continue;
        arr_push(arr, val_obj(vm_new_cstr(vm, fd.cFileName)));
    } while (FindNextFileA(h, &fd));
    FindClose(h);
#else
    DIR *d = opendir(path);
    if (!d) {
        vm_temp_pop(vm, 1);
        vm_raise(vm, "fs_listdir('%s'): %s", path, strerror(errno));
        return false;
    }
    struct dirent *de;
    while ((de = readdir(d)) != NULL) {
        if (strcmp(de->d_name, ".") == 0 || strcmp(de->d_name, "..") == 0) continue;
        arr_push(arr, val_obj(vm_new_cstr(vm, de->d_name)));
    }
    closedir(d);
#endif
    if (arr->len > 1) qsort(arr->items, arr->len, sizeof(PbValue), zr_cstr_ptr_cmp);
    vm_temp_pop(vm, 1);
    *out = val_obj(arr);
    return true;
}

static bool bi_path_join(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc == 1 && val_is_obj(argv[0], OBJ_ARRAY)) {
        PbArray *a = (PbArray *)val_as_obj(argv[0]);
        return bi_path_join(vm, (int)a->len, a->items, out);
    }
    if (argc < 1) { vm_raise(vm, "path_join() expects at least 1 argument"); return false; }
    ZrBuf b;
    zr_buf_init(&b);
    for (int i = 0; i < argc; i++) {
        if (!val_is_obj(argv[i], OBJ_STR)) { zr_buf_free(&b); vm_raise(vm, "path_join(): arguments must be strings"); return false; }
        PbStr *s = (PbStr *)val_as_obj(argv[i]);
        if (s->len == 0) continue;
        if (s->chars[0] == '/' || (s->len > 1 && s->chars[1] == ':')) {
            b.len = 0;
            zr_buf_append(&b, s->chars, s->len);
        } else {
            if (b.len > 0 && b.data[b.len - 1] != '/' && b.data[b.len - 1] != '\\') {
                zr_buf_push_u8(&b, (uint8_t)ZR_PATH_SEP);
            }
            zr_buf_append(&b, s->chars, s->len);
        }
    }
    *out = val_obj(vm_new_str(vm, (const char *)(b.data ? b.data : ""), b.len));
    zr_buf_free(&b);
    return true;
}

static bool bi_path_dirname(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 1 || !val_is_obj(argv[0], OBJ_STR)) { vm_raise(vm, "path_dirname() expects 1 string"); return false; }
    char dir[4096];
    zr_path_parent_dir(((PbStr *)val_as_obj(argv[0]))->chars, dir);
    *out = val_obj(vm_new_cstr(vm, dir));
    return true;
}

static bool bi_path_basename(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 1 || !val_is_obj(argv[0], OBJ_STR)) { vm_raise(vm, "path_basename() expects 1 string"); return false; }
    const char *p = ((PbStr *)val_as_obj(argv[0]))->chars;
    size_t len = strlen(p);
    while (len > 1 && (p[len - 1] == '/' || p[len - 1] == '\\')) len--;
    size_t start = len;
    while (start > 0 && p[start - 1] != '/' && p[start - 1] != '\\') start--;
    *out = val_obj(vm_new_str(vm, p + start, len - start));
    return true;
}

static bool bi_cwd(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    (void)argv;
    if (!vm_check_cap(vm, vm->caps.allow_fs, "cwd", "fs")) return false;
    if (argc != 0) { vm_raise(vm, "cwd() takes no arguments"); return false; }
    char buf[4096];
#if defined(_WIN32)
    if (!_getcwd(buf, sizeof(buf))) { vm_raise(vm, "cwd() failed"); return false; }
#else
    if (!getcwd(buf, sizeof(buf))) { vm_raise(vm, "cwd(): %s", strerror(errno)); return false; }
#endif
    *out = val_obj(vm_new_cstr(vm, buf));
    return true;
}

/* ---- Environment, CLI Args, Signals, Time & OS ---- */
static bool bi_env_get(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (!vm_check_cap(vm, vm->caps.allow_env, "env_get", "env")) return false;
    if (argc != 1 || !val_is_obj(argv[0], OBJ_STR)) { vm_raise(vm, "env_get() expects 1 string"); return false; }
    const char *v = getenv(((PbStr *)val_as_obj(argv[0]))->chars);
    *out = v ? val_obj(vm_new_cstr(vm, v)) : val_nil();
    return true;
}

static bool bi_env_set(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (!vm_check_cap(vm, vm->caps.allow_env, "env_set", "env")) return false;
    if (argc != 2 || !val_is_obj(argv[0], OBJ_STR) || !val_is_obj(argv[1], OBJ_STR)) {
        vm_raise(vm, "env_set(key, val) expects 2 strings");
        return false;
    }
    const char *k = ((PbStr *)val_as_obj(argv[0]))->chars;
    const char *v = ((PbStr *)val_as_obj(argv[1]))->chars;
#if defined(_WIN32)
    _putenv_s(k, v);
#else
    setenv(k, v, 1);
#endif
    *out = val_nil();
    return true;
}

static bool bi_os_args(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    (void)argv;
    if (argc != 0) { vm_raise(vm, "os_args() takes no arguments"); return false; }
    PbArray *arr = vm_new_array(vm, (size_t)(vm->cli_argc > 0 ? vm->cli_argc : 4));
    for (int i = 0; i < vm->cli_argc; i++) {
        arr_push(arr, val_obj(vm_new_cstr(vm, vm->cli_argv[i])));
    }
    *out = val_obj(arr);
    return true;
}

static bool bi_os_exit(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    (void)out;
    if (argc != 1 || argv[0].tag != VAL_NUM) { vm_raise(vm, "os_exit(code) expects 1 number"); return false; }
    if (!vm->caps.allow_process_exit) {
        vm_raise(vm, "os_exit(): process exit is not permitted in this runtime");
        return false;
    }
    exit((int)argv[0].u.num);
}

static bool bi_os_which(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (!vm_check_cap(vm, vm->caps.allow_exec, "os_which", "exec")) return false;
    if (argc != 1 || !val_is_obj(argv[0], OBJ_STR)) { vm_raise(vm, "os_which() expects 1 string"); return false; }
    const char *cmd = ((PbStr *)val_as_obj(argv[0]))->chars;
    if (!cmd[0]) { vm_raise(vm, "os_which(): empty command name"); return false; }
    for (const char *p = cmd; *p; p++) {
        if (strchr("'/\\;&| \t\n><`$(){}!#~\"", *p)) {
            vm_raise(vm, "os_which(): unsafe command name '%s'", cmd);
            return false;
        }
    }
    const char *path_env = getenv("PATH");
    if (!path_env) { *out = val_nil(); return true; }
    char *copy = zr_strdup(path_env);
#if defined(_WIN32)
    const char *delim = ";";
#else
    const char *delim = ":";
#endif
    char *saveptr = NULL;
#if defined(_WIN32)
    for (char *dir = strtok_s(copy, delim, &saveptr); dir; dir = strtok_s(NULL, delim, &saveptr)) {
#else
    for (char *dir = strtok_r(copy, delim, &saveptr); dir; dir = strtok_r(NULL, delim, &saveptr)) {
#endif
        char cand[4096];
        snprintf(cand, sizeof(cand), "%s%c%s", dir, ZR_PATH_SEP, cmd);
#if defined(_WIN32)
        if (zr_file_exists_reg(cand)) { free(copy); *out = val_obj(vm_new_cstr(vm, cand)); return true; }
        snprintf(cand, sizeof(cand), "%s\\%s.exe", dir, cmd);
        if (zr_file_exists_reg(cand)) { free(copy); *out = val_obj(vm_new_cstr(vm, cand)); return true; }
#else
        if (access(cand, X_OK) == 0 && zr_file_exists_reg(cand)) {
            free(copy);
            *out = val_obj(vm_new_cstr(vm, cand));
            return true;
        }
#endif
    }
    free(copy);
    *out = val_nil();
    return true;
}

static bool bi_os_signal_trap(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 2 || !val_is_obj(argv[0], OBJ_STR)) {
        vm_raise(vm, "os_signal_trap(name, handler) expects (string, fn|nil)");
        return false;
    }
    const char *raw = ((PbStr *)val_as_obj(argv[0]))->chars;
    char canon[32];
    int signum = zr_resolve_signal_num(raw, canon);
    if (signum < 0) { vm_raise(vm, "os_signal_trap(): unsupported signal '%s'", raw); return false; }
    signal(signum, zr_c_signal_handler);
    map_set(vm->signal_handlers, vm_new_cstr(vm, canon), argv[1]);

    PbMap *m = vm_new_map(vm, 4);
    vm_temp_push(vm, val_obj(m));
    map_set(m, vm_const_str(vm, "ok"), val_bool(true));
    map_set(m, vm_const_str(vm, "signal"), argv[0]);
    map_set(m, vm_const_str(vm, "signum"), val_num((double)signum));
    vm_temp_pop(vm, 1);
    *out = val_obj(m);
    return true;
}

static bool bi_os_signal_pending(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 1 || !val_is_obj(argv[0], OBJ_STR)) { vm_raise(vm, "os_signal_pending() expects 1 string"); return false; }
    char canon[32];
    int signum = zr_resolve_signal_num(((PbStr *)val_as_obj(argv[0]))->chars, canon);
    if (signum < 0) { vm_raise(vm, "os_signal_pending(): unsupported signal"); return false; }
    sig_atomic_t cnt = g_zr_sig_pending[signum];
    g_zr_sig_pending[signum] = 0;
    *out = val_num((double)cnt);
    return true;
}

static bool bi_os_signal_poll(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    (void)argv;
    if (argc != 0) { vm_raise(vm, "os_signal_poll() takes no arguments"); return false; }
    PbMap *res = vm_new_map(vm, 4);
    vm_temp_push(vm, val_obj(res));
    for (size_t i = 0; i < vm->signal_handlers->used; i++) {
        PbMapEntry *e = &vm->signal_handlers->entries[i];
        if (e->active && e->key) {
            char canon[32];
            int signum = zr_resolve_signal_num(e->key->chars, canon);
            if (signum > 0 && signum < 64) {
                sig_atomic_t cnt = g_zr_sig_pending[signum];
                g_zr_sig_pending[signum] = 0;
                if (cnt > 0) map_set(res, e->key, val_num((double)cnt));
            }
        }
    }
    vm_temp_pop(vm, 1);
    *out = val_obj(res);
    return true;
}

static bool bi_os_signal_handler_get(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 1 || !val_is_obj(argv[0], OBJ_STR)) { vm_raise(vm, "os_signal_handler_get() expects 1 string"); return false; }
    char canon[32];
    zr_resolve_signal_num(((PbStr *)val_as_obj(argv[0]))->chars, canon);
    PbValue h;
    if (map_get_cstr(vm->signal_handlers, canon, &h)) *out = h;
    else *out = val_nil();
    return true;
}

static bool bi_platform(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    (void)argv;
    if (argc != 0) { vm_raise(vm, "platform() takes no arguments"); return false; }
#if defined(_WIN32)
    *out = val_obj(vm_const_str(vm, "windows"));
#elif defined(__APPLE__)
    *out = val_obj(vm_const_str(vm, "macos"));
#else
    *out = val_obj(vm_const_str(vm, "linux"));
#endif
    return true;
}

static bool bi_os_arch(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    (void)argv;
    if (argc != 0) { vm_raise(vm, "os_arch() takes no arguments"); return false; }
#if defined(__aarch64__) || defined(_M_ARM64)
    *out = val_obj(vm_const_str(vm, "aarch64"));
#elif defined(__x86_64__) || defined(_M_X64)
    *out = val_obj(vm_const_str(vm, "x86_64"));
#elif defined(__i386__) || defined(_M_IX86)
    *out = val_obj(vm_const_str(vm, "x86"));
#else
    *out = val_obj(vm_const_str(vm, "unknown"));
#endif
    return true;
}

static bool bi_clock(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    (void)argv;
    if (argc != 0) { vm_raise(vm, "clock() takes no arguments"); return false; }
    *out = val_num(zr_monotonic_sec());
    return true;
}

static bool bi_now_ms(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    (void)argv;
    if (argc != 0) { vm_raise(vm, "now_ms() takes no arguments"); return false; }
    *out = val_num((double)zr_now_ms());
    return true;
}

static bool bi_now_iso(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    (void)argv;
    if (argc != 0) { vm_raise(vm, "now_iso() takes no arguments"); return false; }
    char iso[64];
    zr_now_iso(iso);
    *out = val_obj(vm_new_cstr(vm, iso));
    return true;
}

static bool bi_sleep_ms(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 1 || argv[0].tag != VAL_NUM || argv[0].u.num < 0.0) {
        vm_raise(vm, "sleep_ms(ms) expects non-negative number");
        return false;
    }
    zr_sleep_ms((uint64_t)argv[0].u.num);
    *out = val_nil();
    return true;
}

static bool bi_read_line(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    (void)argv;
    if (argc != 0) { vm_raise(vm, "read_line() takes no arguments"); return false; }
    ZrBuf b;
    zr_buf_init(&b);
    int ch = EOF;
    bool got_any = false;
    while ((ch = fgetc(stdin)) != EOF) {
        got_any = true;
        if (ch == '\n') break;
        if (ch != '\r') zr_buf_push_u8(&b, (uint8_t)ch);
    }
    if (!got_any && b.len == 0) {
        zr_buf_free(&b);
        *out = val_nil();
        return true;
    }
    *out = val_obj(vm_new_str(vm, (const char *)(b.data ? b.data : ""), b.len));
    zr_buf_free(&b);
    return true;
}

/* ---- Subprocess Spawning (Zero Shell Injection) ---- */
static bool bi_exec(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (!vm_check_cap(vm, vm->caps.allow_exec, "exec", "exec")) return false;
    if (argc != 1 || !val_is_obj(argv[0], OBJ_STR)) { vm_raise(vm, "exec() expects 1 string"); return false; }
    const char *cmd = ((PbStr *)val_as_obj(argv[0]))->chars;
    if (!cmd[0]) { vm_raise(vm, "exec(): empty command"); return false; }
    for (const char *p = cmd; *p; p++) {
        if ((unsigned char)*p <= ' ' || strchr(";&|$`><!{}()'\"\\*?[]#~", *p)) {
            vm_raise(vm, "exec(): command contains shell metacharacters or whitespace — use proc_spawn_wait(cmd, args_array) instead.");
            return false;
        }
    }
#if defined(_WIN32)
    STARTUPINFOA si; PROCESS_INFORMATION pi;
    memset(&si, 0, sizeof(si)); si.cb = sizeof(si);
    memset(&pi, 0, sizeof(pi));
    char cmd_mut[1024];
    snprintf(cmd_mut, sizeof(cmd_mut), "\"%s\"", cmd);
    if (!CreateProcessA(NULL, cmd_mut, NULL, NULL, FALSE, 0, NULL, NULL, &si, &pi)) {
        vm_raise(vm, "exec('%s'): CreateProcessA failed", cmd);
        return false;
    }
    WaitForSingleObject(pi.hProcess, INFINITE);
    DWORD code = 1;
    GetExitCodeProcess(pi.hProcess, &code);
    CloseHandle(pi.hProcess);
    CloseHandle(pi.hThread);
    *out = val_num((double)code);
    return true;
#else
    pid_t pid = fork();
    if (pid < 0) { vm_raise(vm, "exec(): fork failed: %s", strerror(errno)); return false; }
    if (pid == 0) {
        char *cargv[2] = { (char *)cmd, NULL };
        execvp(cmd, cargv);
        _exit(127);
    }
    int status = 0;
    while (waitpid(pid, &status, 0) < 0 && errno == EINTR) {}
    int code = WIFEXITED(status) ? WEXITSTATUS(status) : 1;
    *out = val_num((double)code);
    return true;
#endif
}

#if defined(_WIN32)
static void zr_win_append_quoted_arg(ZrBuf *cmd, const char *arg) {
    if (cmd->len > 0) zr_buf_push_u8(cmd, ' ');
    bool need_q = (arg[0] == '\0') || strpbrk(arg, " \t\n\v\"") != NULL;
    if (!need_q) { zr_buf_append_cstr(cmd, arg); return; }
    zr_buf_push_u8(cmd, '"');
    for (const char *p = arg; *p; ) {
        size_t bs = 0;
        while (*p == '\\') { bs++; p++; }
        if (*p == '\0') {
            for (size_t i = 0; i < bs * 2; i++) zr_buf_push_u8(cmd, '\\');
            break;
        } else if (*p == '"') {
            for (size_t i = 0; i < bs * 2 + 1; i++) zr_buf_push_u8(cmd, '\\');
            zr_buf_push_u8(cmd, '"');
            p++;
        } else {
            for (size_t i = 0; i < bs; i++) zr_buf_push_u8(cmd, '\\');
            zr_buf_push_u8(cmd, (uint8_t)*p++);
        }
    }
    zr_buf_push_u8(cmd, '"');
}

typedef struct { HANDLE h; ZrBuf *buf; } ZrPipeReaderArg;
static unsigned __stdcall zr_win_pipe_reader(void *arg) {
    ZrPipeReaderArg *pa = (ZrPipeReaderArg *)arg;
    uint8_t tmp[4096];
    DWORD n = 0;
    while (ReadFile(pa->h, tmp, sizeof(tmp), &n, NULL) && n > 0) {
        zr_buf_append(pa->buf, tmp, (size_t)n);
    }
    return 0;
}
#endif

static bool bi_proc_spawn_wait(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (!vm_check_cap(vm, vm->caps.allow_exec, "proc_spawn_wait", "exec")) return false;
    if (argc != 2 || !val_is_obj(argv[0], OBJ_STR) || !val_is_obj(argv[1], OBJ_ARRAY)) {
        vm_raise(vm, "proc_spawn_wait(cmd, args) expects (string, array)");
        return false;
    }
    const char *prog = ((PbStr *)val_as_obj(argv[0]))->chars;
    PbArray *args_arr = (PbArray *)val_as_obj(argv[1]);
    for (size_t i = 0; i < args_arr->len; i++) {
        if (!val_is_obj(args_arr->items[i], OBJ_STR)) {
            vm_raise(vm, "proc_spawn_wait(): all elements of args array must be strings");
            return false;
        }
    }

    ZrBuf out_b, err_b;
    zr_buf_init(&out_b);
    zr_buf_init(&err_b);
    int exit_code = 1;

#if defined(_WIN32)
    SECURITY_ATTRIBUTES sa;
    sa.nLength = sizeof(sa); sa.bInheritHandle = TRUE; sa.lpSecurityDescriptor = NULL;
    HANDLE out_r = NULL, out_w = NULL, err_r = NULL, err_w = NULL;
    if (!CreatePipe(&out_r, &out_w, &sa, 0) || !CreatePipe(&err_r, &err_w, &sa, 0)) {
        zr_buf_free(&out_b); zr_buf_free(&err_b);
        vm_raise(vm, "proc_spawn_wait(): CreatePipe failed");
        return false;
    }
    SetHandleInformation(out_r, HANDLE_FLAG_INHERIT, 0);
    SetHandleInformation(err_r, HANDLE_FLAG_INHERIT, 0);

    ZrBuf cmdline;
    zr_buf_init(&cmdline);
    zr_win_append_quoted_arg(&cmdline, prog);
    for (size_t i = 0; i < args_arr->len; i++) {
        zr_win_append_quoted_arg(&cmdline, ((PbStr *)val_as_obj(args_arr->items[i]))->chars);
    }
    zr_buf_push_u8(&cmdline, 0);

    STARTUPINFOA si; PROCESS_INFORMATION pi;
    memset(&si, 0, sizeof(si)); si.cb = sizeof(si);
    si.dwFlags = STARTF_USESTDHANDLES;
    si.hStdInput = GetStdHandle(STD_INPUT_HANDLE);
    si.hStdOutput = out_w;
    si.hStdError = err_w;
    memset(&pi, 0, sizeof(pi));

    BOOL ok = CreateProcessA(NULL, (LPSTR)cmdline.data, NULL, NULL, TRUE, 0, NULL, NULL, &si, &pi);
    zr_buf_free(&cmdline);
    CloseHandle(out_w);
    CloseHandle(err_w);
    if (!ok) {
        CloseHandle(out_r); CloseHandle(err_r);
        zr_buf_free(&out_b); zr_buf_free(&err_b);
        vm_raise(vm, "proc_spawn_wait('%s'): CreateProcessA failed", prog);
        return false;
    }
    ZrPipeReaderArg pra_o = { out_r, &out_b }, pra_e = { err_r, &err_b };
    HANDLE th_o = (HANDLE)_beginthreadex(NULL, 0, zr_win_pipe_reader, &pra_o, 0, NULL);
    HANDLE th_e = (HANDLE)_beginthreadex(NULL, 0, zr_win_pipe_reader, &pra_e, 0, NULL);
    WaitForSingleObject(pi.hProcess, INFINITE);
    if (th_o) { WaitForSingleObject(th_o, INFINITE); CloseHandle(th_o); }
    if (th_e) { WaitForSingleObject(th_e, INFINITE); CloseHandle(th_e); }
    CloseHandle(out_r); CloseHandle(err_r);
    DWORD dcode = 1;
    GetExitCodeProcess(pi.hProcess, &dcode);
    exit_code = (int)dcode;
    CloseHandle(pi.hProcess);
    CloseHandle(pi.hThread);
#else
    int p_out[2], p_err[2], p_exec_err[2];
    if (pipe(p_out) != 0 || pipe(p_err) != 0 || pipe(p_exec_err) != 0) {
        zr_buf_free(&out_b); zr_buf_free(&err_b);
        vm_raise(vm, "proc_spawn_wait(): pipe() failed");
        return false;
    }
    fcntl(p_exec_err[1], F_SETFD, FD_CLOEXEC);

    char **cargv = (char **)zr_xmalloc((args_arr->len + 2) * sizeof(char *));
    cargv[0] = (char *)prog;
    for (size_t i = 0; i < args_arr->len; i++) {
        cargv[i + 1] = ((PbStr *)val_as_obj(args_arr->items[i]))->chars;
    }
    cargv[args_arr->len + 1] = NULL;

    pid_t pid = fork();
    if (pid < 0) {
        free(cargv);
        close(p_out[0]); close(p_out[1]); close(p_err[0]); close(p_err[1]);
        close(p_exec_err[0]); close(p_exec_err[1]);
        zr_buf_free(&out_b); zr_buf_free(&err_b);
        vm_raise(vm, "proc_spawn_wait(): fork() failed: %s", strerror(errno));
        return false;
    }
    if (pid == 0) {
        close(p_out[0]); close(p_err[0]); close(p_exec_err[0]);
        dup2(p_out[1], STDOUT_FILENO);
        dup2(p_err[1], STDERR_FILENO);
        close(p_out[1]); close(p_err[1]);
        execvp(prog, cargv);
        int err = errno;
        ssize_t wr = write(p_exec_err[1], &err, sizeof(err));
        (void)wr;
        _exit(127);
    }
    free(cargv);
    close(p_out[1]); close(p_err[1]); close(p_exec_err[1]);

    int child_errno = 0;
    ssize_t er = read(p_exec_err[0], &child_errno, sizeof(child_errno));
    close(p_exec_err[0]);
    if (er == (ssize_t)sizeof(child_errno) && child_errno != 0) {
        close(p_out[0]); close(p_err[0]);
        int st = 0; waitpid(pid, &st, 0);
        zr_buf_free(&out_b); zr_buf_free(&err_b);
        vm_raise(vm, "proc_spawn_wait('%s'): %s", prog, strerror(child_errno));
        return false;
    }

    /* Drain both stdout and stderr concurrently via poll() to avoid pipe deadlock */
    bool open_o = true, open_e = true;
    uint8_t tmp[4096];
    while (open_o || open_e) {
        struct pollfd fds[2];
        fds[0].fd = open_o ? p_out[0] : -1; fds[0].events = POLLIN; fds[0].revents = 0;
        fds[1].fd = open_e ? p_err[0] : -1; fds[1].events = POLLIN; fds[1].revents = 0;
        int pr = poll(fds, 2, 1000);
        if (pr < 0 && errno == EINTR) continue;
        if (pr < 0) break;
        if (open_o && (fds[0].revents & (POLLIN | POLLHUP | POLLERR))) {
            ssize_t n = read(p_out[0], tmp, sizeof(tmp));
            if (n > 0) zr_buf_append(&out_b, tmp, (size_t)n);
            else if (n == 0 || errno != EINTR) { close(p_out[0]); open_o = false; }
        }
        if (open_e && (fds[1].revents & (POLLIN | POLLHUP | POLLERR))) {
            ssize_t n = read(p_err[0], tmp, sizeof(tmp));
            if (n > 0) zr_buf_append(&err_b, tmp, (size_t)n);
            else if (n == 0 || errno != EINTR) { close(p_err[0]); open_e = false; }
        }
    }
    if (open_o) close(p_out[0]);
    if (open_e) close(p_err[0]);
    int status = 0;
    while (waitpid(pid, &status, 0) < 0 && errno == EINTR) {}
    exit_code = WIFEXITED(status) ? WEXITSTATUS(status) : 1;
#endif

    PbMap *res = vm_new_map(vm, 4);
    vm_temp_push(vm, val_obj(res));
    map_set(res, vm_const_str(vm, "exit_code"), val_num((double)exit_code));
    map_set(res, vm_const_str(vm, "stdout"), val_obj(vm_new_str(vm, (const char *)(out_b.data ? out_b.data : ""), out_b.len)));
    map_set(res, vm_const_str(vm, "stderr"), val_obj(vm_new_str(vm, (const char *)(err_b.data ? err_b.data : ""), err_b.len)));
    vm_temp_pop(vm, 1);
    zr_buf_free(&out_b);
    zr_buf_free(&err_b);
    *out = val_obj(res);
    return true;
}

/* ---- Native Hardware Introspection (`hardware_probe()`) ---- */
static void zr_collect_hw_info(int *out_cores, uint64_t *out_total_mb, uint64_t *out_avail_mb,
                               const char **out_disp, bool *out_gpu) {
    *out_cores = 1;
    *out_total_mb = 512;
    *out_avail_mb = 256;
    *out_disp = "headless";
    *out_gpu = false;
#if defined(_WIN32)
    SYSTEM_INFO si;
    GetSystemInfo(&si);
    if (si.dwNumberOfProcessors > 0) *out_cores = (int)si.dwNumberOfProcessors;
    MEMORYSTATUSEX ms;
    ms.dwLength = sizeof(ms);
    if (GlobalMemoryStatusEx(&ms)) {
        *out_total_mb = (uint64_t)(ms.ullTotalPhys / (1024ULL * 1024ULL));
        *out_avail_mb = (uint64_t)(ms.ullAvailPhys / (1024ULL * 1024ULL));
    }
    *out_disp = "win32-gdi";
    *out_gpu = true;
#else
    long ncpu = sysconf(_SC_NPROCESSORS_ONLN);
    if (ncpu > 0) *out_cores = (int)ncpu;

    FILE *mi = fopen("/proc/meminfo", "r");
    if (mi) {
        char line[256];
        uint64_t kb_tot = 0, kb_av = 0;
        while (fgets(line, sizeof(line), mi)) {
            unsigned long long v = 0;
            if (sscanf(line, "MemTotal: %llu kB", &v) == 1) kb_tot = (uint64_t)v;
            else if (sscanf(line, "MemAvailable: %llu kB", &v) == 1) kb_av = (uint64_t)v;
        }
        fclose(mi);
        if (kb_tot > 0) *out_total_mb = kb_tot / 1024ULL;
        if (kb_av > 0)  *out_avail_mb = kb_av / 1024ULL;
    } else {
        struct sysinfo sinfo;
        if (sysinfo(&sinfo) == 0) {
            *out_total_mb = ((uint64_t)sinfo.totalram * (uint64_t)sinfo.mem_unit) / (1024ULL * 1024ULL);
            *out_avail_mb = ((uint64_t)sinfo.freeram * (uint64_t)sinfo.mem_unit) / (1024ULL * 1024ULL);
        }
    }
    const char *wl = getenv("WAYLAND_DISPLAY");
    const char *x11 = getenv("DISPLAY");
    if (wl && wl[0] && x11 && x11[0]) *out_disp = "wayland+x11";
    else if (wl && wl[0]) *out_disp = "wayland";
    else if (x11 && x11[0]) *out_disp = "x11";
    else *out_disp = "headless";

    struct stat st;
    if (stat("/dev/dri/renderD128", &st) == 0 || stat("/dev/dri/card0", &st) == 0) *out_gpu = true;
#endif
}

static bool bi_hardware_probe(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    (void)argv;
    if (argc != 0) { vm_raise(vm, "hardware_probe() takes no arguments"); return false; }
    int cores = 1;
    uint64_t total_mb = 0, avail_mb = 0;
    const char *disp = "headless";
    bool has_gpu = false;
    zr_collect_hw_info(&cores, &total_mb, &avail_mb, &disp, &has_gpu);

    const char *tier = "minimal";
    if (cores >= 8 && total_mb >= 16384) tier = "workstation";
    else if (cores >= 4 && total_mb >= 4096) tier = "standard";
    else if (cores >= 2 && total_mb >= 1024) tier = "compact";

    PbMap *m = vm_new_map(vm, 16);
    vm_temp_push(vm, val_obj(m));
    PbValue plat_v, arch_v;
    bi_platform(vm, 0, NULL, &plat_v);
    bi_os_arch(vm, 0, NULL, &arch_v);
    map_set(m, vm_const_str(vm, "os"), plat_v);
    map_set(m, vm_const_str(vm, "arch"), arch_v);
    map_set(m, vm_const_str(vm, "cpu_cores"), val_num((double)cores));
    map_set(m, vm_const_str(vm, "cpu_logical_cores"), val_num((double)cores));
    map_set(m, vm_const_str(vm, "ram_total_mb"), val_num((double)total_mb));
    map_set(m, vm_const_str(vm, "ram_available_mb"), val_num((double)avail_mb));
    map_set(m, vm_const_str(vm, "display_server"), val_obj(vm_new_cstr(vm, disp)));
    map_set(m, vm_const_str(vm, "has_gpu"), val_bool(has_gpu));
    map_set(m, vm_const_str(vm, "tier"), val_obj(vm_new_cstr(vm, tier)));
    map_set(m, vm_const_str(vm, "runtime"), val_obj(vm_const_str(vm, "zero-rust-c99-v2")));
    vm_temp_pop(vm, 1);
    *out = val_obj(m);
    return true;
}

#endif /* PBLANG_ZR_FS_OS_H */
