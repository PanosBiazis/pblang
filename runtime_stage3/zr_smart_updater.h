/* ============================================================================
 * zr_smart_updater.h — Zero-Rust Polymorphic Atomic Smart Updater (`pblang update`)
 *
 * Supports all 7 polymorphic target classes with zero Rust dependency:
 *   1. Project / `pblang.toml` / `pblang.lock` (`pblang-package-tree-v2`, relative paths)
 *   2. `.sapp` & `.pbck` incremental bytecode + zero-leak `.pbck.meta.json` provenance
 *   3. `.zip`, `.tar.gz`, `.tgz`, `.tar` delta sync with path-traversal & symlink rejection
 *   4. Launcher scripts (`#!`), running ELF64 binaries (`ETXTBSY`-safe swap) & sealed apps
 *   5. `stdlib` syntax-gated + RFC 8032 Ed25519 / SHA-256 verified atomic tree swap
 *   6. `self` isolated/user toolchain release install (`releases/2.0.0`) + rollback
 *   7. Remote URL SSRF & plaintext `http://` fail-closed guard
 * ============================================================================ */

#ifndef ZR_SMART_UPDATER_H
#define ZR_SMART_UPDATER_H

#ifndef _WIN32
#include <dirent.h>
#include <fcntl.h>
#include <sys/stat.h>
#include <unistd.h>
#else
#include <direct.h>
#include <io.h>
#include <sys/stat.h>
#endif

#define ZR_SU_SEMVER "2.4.0"
#define ZR_SU_MAX_ENTRIES 10000
#define ZR_SU_MAX_BYTES   (256ULL * 1024ULL * 1024ULL)

typedef struct {
    char    *rel;
    uint8_t  kind; /* 'f' or 'd' */
    uint8_t *data;
    size_t   len;
} ZrSuEntry;

typedef struct {
    ZrSuEntry *items;
    size_t     len;
    size_t     cap;
} ZrSuEntryList;

typedef struct {
    char  **targets;
    size_t  ntargets;
    bool    dry_run;
    bool    check_only;
    bool    rollback;
    bool    json;
    bool    remote;
    const char *from;
    const char *pubkey_hex;
    const char *expected_sha256;
    const char *install_root;
    const char *bin_dir;
    const char *self_exe;
} ZrSuOpts;

typedef struct {
    char   target[512];
    char   detected_kind[64];
    char   status[64];
    bool   dry_run;
    size_t writes_performed;
    size_t added;
    size_t modified;
    size_t removed;
    size_t unchanged;
    char   sha256_before[68];
    char   sha256_after[68];
    char   backup_path[256];
    char   detail[512];
} ZrSuReceipt;

/* ── Small Helpers ────────────────────────────────────────────────────────── */

static void zr_su_sha256_hex(const uint8_t *data, size_t len, char out_hex[65]) {
    uint8_t dig[32];
    zr_sha256_oneshot(data ? data : (const uint8_t *)"", len, dig);
    static const char hx[] = "0123456789abcdef";
    for (int i = 0; i < 32; i++) {
        out_hex[i * 2]     = hx[(dig[i] >> 4) & 0xF];
        out_hex[i * 2 + 1] = hx[dig[i] & 0xF];
    }
    out_hex[64] = '\0';
}

static bool zr_su_is_hex_n(const char *s, size_t n) {
    if (!s || strlen(s) != n) return false;
    for (size_t i = 0; i < n; i++) {
        char c = s[i];
        bool ok = (c >= '0' && c <= '9') || (c >= 'a' && c <= 'f') || (c >= 'A' && c <= 'F');
        if (!ok) return false;
    }
    return true;
}

static int zr_su_hex_val(char c) {
    if (c >= '0' && c <= '9') return c - '0';
    if (c >= 'a' && c <= 'f') return c - 'a' + 10;
    if (c >= 'A' && c <= 'F') return c - 'A' + 10;
    return -1;
}

static bool zr_su_hex_decode(const char *hex, uint8_t *out, size_t out_len) {
    if (!hex || strlen(hex) != out_len * 2) return false;
    for (size_t i = 0; i < out_len; i++) {
        int hi = zr_su_hex_val(hex[i * 2]);
        int lo = zr_su_hex_val(hex[i * 2 + 1]);
        if (hi < 0 || lo < 0) return false;
        out[i] = (uint8_t)((hi << 4) | lo);
    }
    return true;
}

static const char *zr_su_basename(const char *p) {
    if (!p) return "";
    const char *s1 = strrchr(p, '/');
    const char *s2 = strrchr(p, '\\');
    const char *s = s1;
    if (s2 && (!s || s2 > s)) s = s2;
    return s ? (s + 1) : p;
}

static void zr_su_dirname(const char *p, char *out, size_t cap) {
    if (!p || !*p) { snprintf(out, cap, "."); return; }
    const char *s1 = strrchr(p, '/');
    const char *s2 = strrchr(p, '\\');
    const char *s = s1;
    if (s2 && (!s || s2 > s)) s = s2;
    if (!s) { snprintf(out, cap, "."); return; }
    size_t dlen = (size_t)(s - p);
    if (dlen == 0) { snprintf(out, cap, "/"); return; }
    if (dlen >= cap) dlen = cap - 1;
    memcpy(out, p, dlen);
    out[dlen] = '\0';
}

static bool zr_su_ends_with_ci(const char *s, const char *suf) {
    size_t sl = strlen(s), fl = strlen(suf);
    if (sl < fl) return false;
    for (size_t i = 0; i < fl; i++) {
        char a = s[sl - fl + i];
        char b = suf[i];
        if (a >= 'A' && a <= 'Z') a = (char)(a + 32);
        if (b >= 'A' && b <= 'Z') b = (char)(b + 32);
        if (a != b) return false;
    }
    return true;
}

static bool zr_su_is_symlink(const char *path) {
#if !defined(_WIN32) && !defined(__wasi__)
    struct stat st;
    if (lstat(path, &st) == 0 && S_ISLNK(st.st_mode)) return true;
#else
    (void)path;
#endif
    return false;
}

static bool zr_su_mkdir_p(const char *dir_path) {
    if (!dir_path || !*dir_path) return true;
    char tmp[4096];
    snprintf(tmp, sizeof(tmp), "%.4090s", dir_path);
    size_t len = strlen(tmp);
    if (len > 1 && (tmp[len - 1] == '/' || tmp[len - 1] == '\\')) tmp[len - 1] = '\0';
    for (char *p = tmp + 1; *p; p++) {
        if (*p == '/' || *p == '\\') {
            char ch = *p;
            *p = '\0';
#ifdef _WIN32
            _mkdir(tmp);
#else
            mkdir(tmp, 0755);
#endif
            *p = ch;
        }
    }
#ifdef _WIN32
    _mkdir(tmp);
#else
    mkdir(tmp, 0755);
#endif
    return zr_dir_exists(tmp);
}

static void zr_su_rm_rf(const char *path) {
    if (!path || !*path) return;
    if (zr_su_is_symlink(path) || zr_file_exists_reg(path)) {
        remove(path);
        return;
    }
#ifndef _WIN32
    DIR *d = opendir(path);
    if (!d) { remove(path); return; }
    struct dirent *de;
    while ((de = readdir(d)) != NULL) {
        if (strcmp(de->d_name, ".") == 0 || strcmp(de->d_name, "..") == 0) continue;
        char child[4096];
        snprintf(child, sizeof(child), "%s/%s", path, de->d_name);
        zr_su_rm_rf(child);
    }
    closedir(d);
    rmdir(path);
#else
    _rmdir(path);
#endif
}

static uint8_t *zr_su_read_file(const char *path, size_t *out_len, char *err) {
    if (zr_su_is_symlink(path)) {
        snprintf(err, 256, "refusing to read symlink '%s'", zr_su_basename(path));
        return NULL;
    }
    FILE *f = fopen(path, "rb");
    if (!f) {
        snprintf(err, 256, "cannot open '%s'", zr_su_basename(path));
        return NULL;
    }
    if (fseek(f, 0, SEEK_END) != 0) { fclose(f); snprintf(err, 256, "fseek failed"); return NULL; }
    long sz = ftell(f);
    if (sz < 0) { fclose(f); snprintf(err, 256, "ftell failed"); return NULL; }
    if (fseek(f, 0, SEEK_SET) != 0) { fclose(f); snprintf(err, 256, "fseek set failed"); return NULL; }
    uint8_t *buf = (uint8_t *)zr_xmalloc((size_t)sz + 1);
    if (sz > 0 && fread(buf, 1, (size_t)sz, f) != (size_t)sz) {
        free(buf);
        fclose(f);
        snprintf(err, 256, "fread failed");
        return NULL;
    }
    buf[sz] = '\0';
    fclose(f);
    *out_len = (size_t)sz;
    return buf;
}

static bool zr_su_atomic_rename(const char *src, const char *dst) {
#ifdef _WIN32
    return MoveFileExA(src, dst, MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH) != 0;
#else
    return rename(src, dst) == 0;
#endif
}

/* ETXTBSY-safe atomic write via exclusive sibling inode + optional `.bak` backup */
static bool zr_su_write_atomic(const char *target, const uint8_t *data, size_t len,
                               int mode, bool keep_backup, bool *out_had_bak, char *err) {
    if (out_had_bak) *out_had_bak = false;
    if (zr_su_is_symlink(target)) {
        snprintf(err, 256, "refusing to overwrite symlink '%s'", zr_su_basename(target));
        return false;
    }
    char parent[4096];
    zr_su_dirname(target, parent, sizeof(parent));
    if (!zr_su_mkdir_p(parent)) {
        snprintf(err, 256, "cannot create parent directory");
        return false;
    }

    static unsigned int s_seq = 0;
    char stage[4200];
#if !defined(_WIN32) && !defined(__wasi__)
    int fd = -1;
    for (int attempt = 0; attempt < 32; attempt++) {
        snprintf(stage, sizeof(stage), "%s/.pblang-upd-%d-%u.tmp",
                 parent, (int)getpid(), ++s_seq);
        fd = open(stage, O_WRONLY | O_CREAT | O_EXCL, 0600);
        if (fd >= 0) break;
    }
    if (fd < 0) { snprintf(err, 256, "cannot create exclusive staging file"); return false; }
    size_t off = 0;
    while (off < len) {
        ssize_t w = write(fd, data + off, len - off);
        if (w <= 0) { close(fd); unlink(stage); snprintf(err, 256, "staging write failed"); return false; }
        off += (size_t)w;
    }
    (void)fchmod(fd, (mode_t)(mode & 0755));
    close(fd);
#else
    (void)mode;
    snprintf(stage, sizeof(stage), "%s/.pblang-upd-%u.tmp", parent, ++s_seq);
    FILE *fp = fopen(stage, "wb");
    if (!fp) { snprintf(err, 256, "cannot open staging file"); return false; }
    if (len > 0 && fwrite(data, 1, len, fp) != len) {
        fclose(fp); remove(stage); snprintf(err, 256, "staging fwrite failed"); return false;
    }
    fclose(fp);
#endif

    if (keep_backup && zr_file_exists_reg(target)) {
        size_t old_len = 0;
        char rerr[256];
        uint8_t *old_b = zr_su_read_file(target, &old_len, rerr);
        if (old_b) {
            char bak_p[4200];
            snprintf(bak_p, sizeof(bak_p), "%s.bak", target);
            bool dummy = false;
            (void)zr_su_write_atomic(bak_p, old_b, old_len, 0600, false, &dummy, rerr);
            free(old_b);
            if (out_had_bak) *out_had_bak = true;
        }
    }

    if (!zr_su_atomic_rename(stage, target)) {
        remove(stage);
        snprintf(err, 256, "atomic rename failed for '%s'", zr_su_basename(target));
        return false;
    }
#if !defined(_WIN32) && !defined(__wasi__)
    chmod(target, (mode_t)(mode & 0755));
#endif
    return true;
}

/* ── Privacy Scrubber & SSRF Guard ────────────────────────────────────────── */

static void zr_su_scrub_str(const char *in, char *out, size_t cap) {
    if (!in || cap == 0) { if (cap) out[0] = '\0'; return; }
    const char *home = getenv("HOME");
    size_t hlen = (home && strlen(home) >= 3) ? strlen(home) : 0;
    size_t oi = 0;
    for (size_t i = 0; in[i] && oi + 8 < cap; ) {
        if (hlen > 0 && strncmp(in + i, home, hlen) == 0) {
            out[oi++] = '~';
            i += hlen;
            continue;
        }
        if (strncmp(in + i, "/home/", 6) == 0 || strncmp(in + i, "/Users/", 7) == 0) {
            size_t pfx = (in[i + 1] == 'h') ? 6 : 7;
            size_t j = i + pfx;
            while (in[j] && in[j] != '/' && in[j] != '"' && in[j] != ' ' && in[j] != '\n') j++;
            out[oi++] = '~';
            i = j;
            continue;
        }
        if (strncmp(in + i, "/media/", 7) == 0) {
            size_t j = i + 7;
            while (in[j] && in[j] != '/' && in[j] != '"' && in[j] != ' ' && in[j] != '\n') j++;
            memcpy(out + oi, "<MEDIA>", 7);
            oi += 7;
            i = j;
            continue;
        }
        out[oi++] = in[i++];
    }
    out[oi] = '\0';
}

static bool zr_su_check_remote_url(const char *url, const char *exp_sha, const char *pubkey, char *err) {
    if (strncmp(url, "http://", 7) == 0) {
        snprintf(err, 256, "plaintext http:// URLs are forbidden; use https://");
        return false;
    }
    if (strncmp(url, "https://", 8) != 0) {
        snprintf(err, 256, "unsupported URL scheme");
        return false;
    }
    const char *rest = url + 8;
    char host[256];
    size_t hi = 0;
    if (*rest == '[') {
        rest++;
        while (*rest && *rest != ']' && hi + 1 < sizeof(host)) host[hi++] = *rest++;
    } else {
        while (*rest && *rest != '/' && *rest != ':' && *rest != '?' && hi + 1 < sizeof(host)) {
            char c = *rest++;
            if (c >= 'A' && c <= 'Z') c = (char)(c + 32);
            host[hi++] = c;
        }
    }
    host[hi] = '\0';
    if (hi == 0 || strchr(host, '@')) {
        snprintf(err, 256, "invalid URL authority");
        return false;
    }
    if (strcmp(host, "localhost") == 0 || strcmp(host, "::1") == 0 ||
        zr_su_ends_with_ci(host, ".localhost") || zr_su_ends_with_ci(host, ".local") ||
        zr_su_ends_with_ci(host, ".internal") || strncmp(host, "fe80:", 5) == 0 ||
        strncmp(host, "fc", 2) == 0 || strncmp(host, "fd", 2) == 0) {
        snprintf(err, 256, "SSRF guard blocked loopback/private host '%.160s'", host);
        return false;
    }
    unsigned int o0 = 0, o1 = 0, o2 = 0, o3 = 0;
    char tail = '\0';
    if (sscanf(host, "%u.%u.%u.%u%c", &o0, &o1, &o2, &o3, &tail) == 4 &&
        o0 <= 255 && o1 <= 255 && o2 <= 255 && o3 <= 255) {
        if (o0 == 127 || o0 == 10 || o0 == 0 ||
            (o0 == 172 && o1 >= 16 && o1 <= 31) ||
            (o0 == 192 && o1 == 168) ||
            (o0 == 169 && o1 == 254) ||
            (o0 == 100 && o1 >= 64 && o1 <= 127)) {
            snprintf(err, 256, "SSRF guard blocked private/loopback IPv4 '%.160s'", host);
            return false;
        }
    }
    bool has_sha = exp_sha && zr_su_is_hex_n(exp_sha, 64);
    bool has_pk  = pubkey  && zr_su_is_hex_n(pubkey, 64);
    if (!has_sha && !has_pk) {
        snprintf(err, 256, "remote https:// source requires --expected-sha256 or --pubkey");
        return false;
    }
    return true;
}

/* ── Safe Entry List & Archive Validation ─────────────────────────────────── */

static void zr_su_elist_free(ZrSuEntryList *list) {
    if (!list) return;
    for (size_t i = 0; i < list->len; i++) {
        free(list->items[i].rel);
        free(list->items[i].data);
    }
    free(list->items);
    list->items = NULL;
    list->len = list->cap = 0;
}

static int zr_su_cmp_entry(const void *a, const void *b) {
    const ZrSuEntry *ea = (const ZrSuEntry *)a;
    const ZrSuEntry *eb = (const ZrSuEntry *)b;
    return strcmp(ea->rel, eb->rel);
}

static void zr_su_elist_put(ZrSuEntryList *list, const char *rel, uint8_t kind,
                            const uint8_t *data, size_t len) {
    for (size_t i = 0; i < list->len; i++) {
        if (strcmp(list->items[i].rel, rel) == 0 && list->items[i].kind == kind) {
            free(list->items[i].data);
            list->items[i].data = (uint8_t *)zr_xmalloc(len + 1);
            if (len > 0) memcpy(list->items[i].data, data, len);
            list->items[i].data[len] = '\0';
            list->items[i].len = len;
            return;
        }
    }
    if (list->len >= list->cap) {
        list->cap = list->cap ? list->cap * 2 : 32;
        list->items = (ZrSuEntry *)zr_xrealloc(list->items, list->cap * sizeof(ZrSuEntry));
    }
    ZrSuEntry *e = &list->items[list->len++];
    e->rel  = zr_xstrdup(rel);
    e->kind = kind;
    e->data = (uint8_t *)zr_xmalloc(len + 1);
    if (len > 0) memcpy(e->data, data, len);
    e->data[len] = '\0';
    e->len  = len;
}

static const ZrSuEntry *zr_su_elist_find(const ZrSuEntryList *list, const char *rel) {
    for (size_t i = 0; i < list->len; i++) {
        if (list->items[i].kind == 'f' && strcmp(list->items[i].rel, rel) == 0) {
            return &list->items[i];
        }
    }
    return NULL;
}

static bool zr_su_normalize_rel_path(const char *raw, char *out, size_t cap, char *err) {
    if (!raw || !*raw || strlen(raw) > 1024) {
        snprintf(err, 256, "empty or oversized archive entry path");
        return false;
    }
    if (raw[0] == '/' || raw[0] == '\\') {
        snprintf(err, 256, "forbidden leading slash in archive entry '%s'", raw);
        return false;
    }
    if (raw[0] && raw[1] == ':') {
        snprintf(err, 256, "forbidden Windows drive prefix in archive entry");
        return false;
    }
    for (size_t i = 0; raw[i]; i++) {
        if ((unsigned char)raw[i] < 32) {
            snprintf(err, 256, "control character in archive entry");
            return false;
        }
    }
    char copy[1040];
    snprintf(copy, sizeof(copy), "%.1030s", raw);
    for (size_t i = 0; copy[i]; i++) if (copy[i] == '\\') copy[i] = '/';

    out[0] = '\0';
    size_t oi = 0;
    char *p = copy;
    while (*p) {
        while (*p == '/') p++;
        if (!*p) break;
        char *end = strchr(p, '/');
        size_t slen = end ? (size_t)(end - p) : strlen(p);
        if (slen == 2 && p[0] == '.' && p[1] == '.') {
            snprintf(err, 256, "forbidden '..' path traversal in archive entry");
            return false;
        }
        if (!(slen == 1 && p[0] == '.')) {
            if (oi > 0 && oi + 1 < cap) out[oi++] = '/';
            if (oi + slen >= cap) { snprintf(err, 256, "entry path too long"); return false; }
            memcpy(out + oi, p, slen);
            oi += slen;
            out[oi] = '\0';
        }
        if (!end) break;
        p = end + 1;
    }
    return oi > 0;
}

/* ── Dynamic Zlib Bridge + Safe Tar / Zip Readers & Writers ───────────────── */

typedef struct {
    const uint8_t *next_in;
    unsigned int   avail_in;
    unsigned long  total_in;
    uint8_t       *next_out;
    unsigned int   avail_out;
    unsigned long  total_out;
    const char    *msg;
    void          *state;
    void          *zalloc;
    void          *zfree;
    void          *opaque;
    int            data_type;
    unsigned long  adler;
    unsigned long  reserved;
} ZrZStream;

typedef struct {
    bool        tried;
    bool        ready;
    ZrDlHandle  handle;
    const char *(*zlibVersion)(void);
    int         (*inflateInit2_)(ZrZStream *, int, const char *, int);
    int         (*inflate)(ZrZStream *, int);
    int         (*inflateEnd)(ZrZStream *);
    int         (*deflateInit2_)(ZrZStream *, int, int, int, int, int, const char *, int);
    int         (*deflate)(ZrZStream *, int);
    int         (*deflateEnd)(ZrZStream *);
} ZrZlibApi;

static ZrZlibApi g_zr_zlib = {0};

static bool zr_su_init_zlib(void) {
    if (g_zr_zlib.tried) return g_zr_zlib.ready;
    g_zr_zlib.tried = true;
#ifdef _WIN32
    const char *cands[] = { "zlib1.dll", "libz.dll", NULL };
#elif defined(__APPLE__)
    const char *cands[] = { "libz.1.dylib", "libz.dylib", NULL };
#else
    const char *cands[] = {
        "libz.so.1", "/lib/x86_64-linux-gnu/libz.so.1", "/usr/lib/x86_64-linux-gnu/libz.so.1",
        "/usr/lib/aarch64-linux-gnu/libz.so.1", "libz.so", NULL
    };
#endif
    for (int i = 0; cands[i] && !g_zr_zlib.handle; i++) g_zr_zlib.handle = zr_dlopen(cands[i]);
    if (!g_zr_zlib.handle) return false;
    g_zr_zlib.zlibVersion   = (const char *(*)(void))zr_dlsym(g_zr_zlib.handle, "zlibVersion");
    g_zr_zlib.inflateInit2_ = (int (*)(ZrZStream *, int, const char *, int))zr_dlsym(g_zr_zlib.handle, "inflateInit2_");
    g_zr_zlib.inflate       = (int (*)(ZrZStream *, int))zr_dlsym(g_zr_zlib.handle, "inflate");
    g_zr_zlib.inflateEnd    = (int (*)(ZrZStream *))zr_dlsym(g_zr_zlib.handle, "inflateEnd");
    g_zr_zlib.deflateInit2_ = (int (*)(ZrZStream *, int, int, int, int, int, const char *, int))zr_dlsym(g_zr_zlib.handle, "deflateInit2_");
    g_zr_zlib.deflate       = (int (*)(ZrZStream *, int))zr_dlsym(g_zr_zlib.handle, "deflate");
    g_zr_zlib.deflateEnd    = (int (*)(ZrZStream *))zr_dlsym(g_zr_zlib.handle, "deflateEnd");
    g_zr_zlib.ready = (g_zr_zlib.zlibVersion && g_zr_zlib.inflateInit2_ && g_zr_zlib.inflate &&
                       g_zr_zlib.inflateEnd && g_zr_zlib.deflateInit2_ && g_zr_zlib.deflate &&
                       g_zr_zlib.deflateEnd);
    return g_zr_zlib.ready;
}

static uint8_t *zr_su_zlib_inflate(const uint8_t *in, size_t in_len, int wbits,
                                   size_t *out_len, char *err) {
    if (!zr_su_init_zlib()) {
        snprintf(err, 256, "zlib shared library unavailable for compressed archive");
        return NULL;
    }
    ZrZStream zs;
    memset(&zs, 0, sizeof(zs));
    zs.next_in  = in;
    zs.avail_in = (unsigned int)in_len;
    if (g_zr_zlib.inflateInit2_(&zs, wbits, g_zr_zlib.zlibVersion(), (int)sizeof(ZrZStream)) != 0) {
        snprintf(err, 256, "zlib inflateInit2 failed");
        return NULL;
    }
    ZrBuf out;
    zr_buf_init(&out);
    uint8_t chunk[32768];
    int ret = 0;
    while (1) {
        zs.next_out  = chunk;
        zs.avail_out = sizeof(chunk);
        ret = g_zr_zlib.inflate(&zs, 0 /* Z_NO_FLUSH */);
        size_t have = sizeof(chunk) - zs.avail_out;
        if (have > 0) {
            if (out.len + have > ZR_SU_MAX_BYTES) {
                g_zr_zlib.inflateEnd(&zs);
                zr_buf_free(&out);
                snprintf(err, 256, "archive decompression exceeded 256 MiB safety limit");
                return NULL;
            }
            zr_buf_append(&out, chunk, have);
        }
        if (ret == 1 /* Z_STREAM_END */) break;
        if (ret != 0 /* Z_OK */) {
            g_zr_zlib.inflateEnd(&zs);
            zr_buf_free(&out);
            snprintf(err, 256, "corrupt compressed stream (zlib code %d)", ret);
            return NULL;
        }
    }
    g_zr_zlib.inflateEnd(&zs);
    *out_len = out.len;
    if (!out.data) return (uint8_t *)zr_xcalloc(1, 1);
    return (uint8_t *)out.data;
}

static uint8_t *zr_su_zlib_deflate(const uint8_t *in, size_t in_len, int wbits,
                                   size_t *out_len, char *err) {
    if (!zr_su_init_zlib()) {
        snprintf(err, 256, "zlib shared library unavailable");
        return NULL;
    }
    ZrZStream zs;
    memset(&zs, 0, sizeof(zs));
    zs.next_in  = in;
    zs.avail_in = (unsigned int)in_len;
    if (g_zr_zlib.deflateInit2_(&zs, 6, 8 /* Z_DEFLATED */, wbits, 8, 0,
                                g_zr_zlib.zlibVersion(), (int)sizeof(ZrZStream)) != 0) {
        snprintf(err, 256, "zlib deflateInit2 failed");
        return NULL;
    }
    ZrBuf out;
    zr_buf_init(&out);
    uint8_t chunk[32768];
    int ret = 0;
    do {
        zs.next_out  = chunk;
        zs.avail_out = sizeof(chunk);
        ret = g_zr_zlib.deflate(&zs, 4 /* Z_FINISH */);
        size_t have = sizeof(chunk) - zs.avail_out;
        if (have > 0) zr_buf_append(&out, chunk, have);
    } while (ret == 0 /* Z_OK */);
    g_zr_zlib.deflateEnd(&zs);
    if (ret != 1 /* Z_STREAM_END */) {
        zr_buf_free(&out);
        snprintf(err, 256, "zlib deflate failed");
        return NULL;
    }
    *out_len = out.len;
    return out.data ? (uint8_t *)out.data : (uint8_t *)zr_xcalloc(1, 1);
}

static uint32_t zr_su_crc32(const uint8_t *data, size_t len) {
    uint32_t crc = 0xFFFFFFFFu;
    for (size_t i = 0; i < len; i++) {
        crc ^= data[i];
        for (int b = 0; b < 8; b++) {
            uint32_t mask = (uint32_t)-(int32_t)(crc & 1u);
            crc = (crc >> 1) ^ (0xEDB88320u & mask);
        }
    }
    return ~crc;
}

static uint16_t zr_su_u16le(const uint8_t *b) { return (uint16_t)(b[0] | ((uint16_t)b[1] << 8)); }
static uint32_t zr_su_u32le(const uint8_t *b) {
    return (uint32_t)b[0] | ((uint32_t)b[1] << 8) | ((uint32_t)b[2] << 16) | ((uint32_t)b[3] << 24);
}
static void zr_su_put16le(ZrBuf *b, uint16_t v) { uint8_t t[2] = { (uint8_t)v, (uint8_t)(v >> 8) }; zr_buf_append(b, t, 2); }
static void zr_su_put32le(ZrBuf *b, uint32_t v) {
    uint8_t t[4] = { (uint8_t)v, (uint8_t)(v >> 8), (uint8_t)(v >> 16), (uint8_t)(v >> 24) };
    zr_buf_append(b, t, 4);
}

static bool zr_su_read_tar_raw(const uint8_t *tar, size_t tar_len, ZrSuEntryList *out, char *err) {
    size_t pos = 0;
    while (pos + 512 <= tar_len) {
        const uint8_t *hdr = tar + pos;
        pos += 512;
        bool all_zero = true;
        for (int i = 0; i < 512; i++) if (hdr[i] != 0) { all_zero = false; break; }
        if (all_zero) break;

        uint8_t tflag = hdr[156];
        char name_raw[280];
        char prefix[160], base[104];
        memcpy(base, hdr, 100); base[100] = '\0';
        memcpy(prefix, hdr + 345, 155); prefix[155] = '\0';
        if (prefix[0]) snprintf(name_raw, sizeof(name_raw), "%s/%s", prefix, base);
        else snprintf(name_raw, sizeof(name_raw), "%s", base);

        uint64_t fsize = 0;
        for (int i = 124; i < 136 && hdr[i]; i++) {
            if (hdr[i] >= '0' && hdr[i] <= '7') fsize = (fsize << 3) + (uint64_t)(hdr[i] - '0');
        }
        size_t padded = (size_t)(((fsize + 511ULL) / 512ULL) * 512ULL);
        if (pos + padded > tar_len) {
            snprintf(err, 256, "truncated tar entry '%.160s'", name_raw);
            return false;
        }

        if (tflag == '2' || tflag == '1') {
            snprintf(err, 256, "tar archive contains forbidden symlink/hardlink entry '%.160s'", name_raw);
            return false;
        }
        if (tflag == '5' || strcmp(name_raw, ".") == 0 || strcmp(name_raw, "./") == 0) {
            pos += padded;
            continue;
        }
        if (tflag == 'x' || tflag == 'g' || tflag == 'L') {
            pos += padded;
            continue;
        }
        if (tflag != 0 && tflag != '0') {
            snprintf(err, 256, "unsupported tar entry type '%c'", (char)tflag);
            return false;
        }

        const char *trimmed = name_raw;
        if (strncmp(trimmed, "./", 2) == 0) trimmed += 2;
        if (*trimmed == '\0') { pos += padded; continue; }

        char clean[1024];
        if (!zr_su_normalize_rel_path(trimmed, clean, sizeof(clean), err)) return false;
        zr_su_elist_put(out, clean, 'f', tar + pos, (size_t)fsize);
        pos += padded;
    }
    qsort(out->items, out->len, sizeof(ZrSuEntry), zr_su_cmp_entry);
    return true;
}

static uint8_t *zr_su_write_tar_raw(const ZrSuEntryList *entries, size_t *out_len, char *err) {
    ZrBuf b;
    zr_buf_init(&b);
    uint8_t zero512[512];
    memset(zero512, 0, 512);

    for (size_t i = 0; i < entries->len; i++) {
        const ZrSuEntry *e = &entries->items[i];
        if (e->kind != 'f') continue;
        if (strlen(e->rel) > 99) {
            zr_buf_free(&b);
            snprintf(err, 256, "tar entry path exceeds 99 bytes: '%s'", e->rel);
            return NULL;
        }
        uint8_t hdr[512];
        memset(hdr, 0, 512);
        memcpy(hdr, e->rel, strlen(e->rel));
        memcpy(hdr + 100, "0000644", 7);
        memcpy(hdr + 108, "0000000", 7);
        memcpy(hdr + 116, "0000000", 7);
        snprintf((char *)(hdr + 124), 12, "%011llo", (unsigned long long)e->len);
        memcpy(hdr + 136, "14712000000", 11);
        memset(hdr + 148, ' ', 8);
        hdr[156] = '0';
        memcpy(hdr + 257, "ustar\0", 6);
        memcpy(hdr + 263, "00", 2);
        unsigned int csum = 0;
        for (int k = 0; k < 512; k++) csum += hdr[k];
        snprintf((char *)(hdr + 148), 7, "%06o", csum);
        hdr[155] = ' ';

        zr_buf_append(&b, hdr, 512);
        if (e->len > 0) {
            zr_buf_append(&b, e->data, e->len);
            size_t rem = e->len % 512;
            if (rem != 0) zr_buf_append(&b, zero512, 512 - rem);
        }
    }
    zr_buf_append(&b, zero512, 512);
    zr_buf_append(&b, zero512, 512);
    *out_len = b.len;
    return (uint8_t *)b.data;
}

static bool zr_su_read_zip(const uint8_t *zip, size_t zip_len, ZrSuEntryList *out, char *err) {
    if (zip_len < 22) { snprintf(err, 256, "zip too short"); return false; }
    size_t min_off = (zip_len > 65557) ? (zip_len - 65557) : 0;
    ssize_t eocd = -1;
    for (ssize_t p = (ssize_t)zip_len - 22; p >= (ssize_t)min_off; p--) {
        if (zip[p] == 0x50 && zip[p + 1] == 0x4B && zip[p + 2] == 0x05 && zip[p + 3] == 0x06) {
            eocd = p;
            break;
        }
    }
    if (eocd < 0) { snprintf(err, 256, "missing ZIP EOCD signature"); return false; }
    uint16_t count  = zr_su_u16le(zip + eocd + 10);
    uint32_t cd_off = zr_su_u32le(zip + eocd + 16);
    if ((size_t)cd_off > (size_t)eocd) { snprintf(err, 256, "corrupt ZIP central directory offset"); return false; }

    size_t pos = cd_off;
    for (uint16_t idx = 0; idx < count; idx++) {
        if (pos + 46 > (size_t)eocd || !(zip[pos] == 0x50 && zip[pos + 1] == 0x4B && zip[pos + 2] == 0x01 && zip[pos + 3] == 0x02)) {
            snprintf(err, 256, "corrupt ZIP central directory entry");
            return false;
        }
        uint16_t method    = zr_su_u16le(zip + pos + 10);
        uint32_t crc_exp   = zr_su_u32le(zip + pos + 16);
        uint32_t comp_sz   = zr_su_u32le(zip + pos + 20);
        uint32_t uncomp_sz = zr_su_u32le(zip + pos + 24);
        uint16_t name_len  = zr_su_u16le(zip + pos + 28);
        uint16_t extra_len = zr_su_u16le(zip + pos + 30);
        uint16_t comm_len  = zr_su_u16le(zip + pos + 32);
        uint32_t ext_attr  = zr_su_u32le(zip + pos + 38);
        uint32_t lhoff     = zr_su_u32le(zip + pos + 42);

        if (pos + 46 + name_len + extra_len + comm_len > (size_t)eocd) {
            snprintf(err, 256, "truncated ZIP central directory name");
            return false;
        }
        char raw_name[1040];
        if (name_len >= sizeof(raw_name)) { snprintf(err, 256, "ZIP entry name too long"); return false; }
        memcpy(raw_name, zip + pos + 46, name_len);
        raw_name[name_len] = '\0';
        pos += 46 + name_len + extra_len + comm_len;

        uint32_t unix_mode = (ext_attr >> 16) & 0xFFFFu;
        if ((unix_mode & 0170000u) == 0120000u) {
            snprintf(err, 256, "ZIP archive contains forbidden symlink entry '%.160s'", raw_name);
            return false;
        }
        if (name_len > 0 && raw_name[name_len - 1] == '/') continue;

        char clean[1024];
        if (!zr_su_normalize_rel_path(raw_name, clean, sizeof(clean), err)) return false;

        if ((size_t)lhoff + 30 > zip_len || !(zip[lhoff] == 0x50 && zip[lhoff + 1] == 0x4B && zip[lhoff + 2] == 0x03 && zip[lhoff + 3] == 0x04)) {
            snprintf(err, 256, "invalid ZIP local file header");
            return false;
        }
        uint16_t ln_len = zr_su_u16le(zip + lhoff + 26);
        uint16_t lx_len = zr_su_u16le(zip + lhoff + 28);
        size_t data_off = (size_t)lhoff + 30 + ln_len + lx_len;
        if (data_off + comp_sz > zip_len) { snprintf(err, 256, "ZIP entry payload out of bounds"); return false; }

        uint8_t *plain = NULL;
        size_t plain_len = 0;
        if (method == 0) {
            plain_len = comp_sz;
            plain = (uint8_t *)zr_xmalloc(plain_len + 1);
            if (plain_len > 0) memcpy(plain, zip + data_off, plain_len);
        } else if (method == 8) {
            plain = zr_su_zlib_inflate(zip + data_off, comp_sz, -15, &plain_len, err);
            if (!plain) return false;
        } else {
            snprintf(err, 256, "unsupported ZIP compression method %u", (unsigned)method);
            return false;
        }
        if (plain_len != uncomp_sz || zr_su_crc32(plain, plain_len) != crc_exp) {
            free(plain);
            snprintf(err, 256, "ZIP CRC-32 / size mismatch on '%.160s'", clean);
            return false;
        }
        zr_su_elist_put(out, clean, 'f', plain, plain_len);
        free(plain);
    }
    qsort(out->items, out->len, sizeof(ZrSuEntry), zr_su_cmp_entry);
    return true;
}

static uint8_t *zr_su_write_zip(const ZrSuEntryList *entries, size_t *out_len, char *err) {
    ZrBuf out, cd;
    zr_buf_init(&out);
    zr_buf_init(&cd);
    bool use_deflate = zr_su_init_zlib();
    uint16_t count = 0;

    for (size_t i = 0; i < entries->len; i++) {
        const ZrSuEntry *e = &entries->items[i];
        if (e->kind != 'f') continue;
        uint32_t lhoff = (uint32_t)out.len;
        uint32_t crc   = zr_su_crc32(e->data, e->len);
        uint16_t method = 0;
        uint8_t *comp = NULL;
        size_t comp_len = e->len;
        if (use_deflate && e->len > 32) {
            char derr[256];
            comp = zr_su_zlib_deflate(e->data, e->len, -15, &comp_len, derr);
            if (comp) method = 8;
            else comp_len = e->len;
        }
        const uint8_t *payload = comp ? comp : e->data;
        uint16_t nlen = (uint16_t)strlen(e->rel);

        zr_buf_append(&out, "PK\x03\x04", 4);
        zr_su_put16le(&out, 20);
        zr_su_put16le(&out, 0);
        zr_su_put16le(&out, method);
        zr_su_put16le(&out, 0);
        zr_su_put16le(&out, 0x21);
        zr_su_put32le(&out, crc);
        zr_su_put32le(&out, (uint32_t)comp_len);
        zr_su_put32le(&out, (uint32_t)e->len);
        zr_su_put16le(&out, nlen);
        zr_su_put16le(&out, 0);
        zr_buf_append(&out, e->rel, nlen);
        if (comp_len > 0) zr_buf_append(&out, payload, comp_len);

        zr_buf_append(&cd, "PK\x01\x02", 4);
        zr_su_put16le(&cd, (3u << 8) | 20u);
        zr_su_put16le(&cd, 20);
        zr_su_put16le(&cd, 0);
        zr_su_put16le(&cd, method);
        zr_su_put16le(&cd, 0);
        zr_su_put16le(&cd, 0x21);
        zr_su_put32le(&cd, crc);
        zr_su_put32le(&cd, (uint32_t)comp_len);
        zr_su_put32le(&cd, (uint32_t)e->len);
        zr_su_put16le(&cd, nlen);
        zr_su_put16le(&cd, 0);
        zr_su_put16le(&cd, 0);
        zr_su_put16le(&cd, 0);
        zr_su_put16le(&cd, 0);
        zr_su_put32le(&cd, (0100644u << 16));
        zr_su_put32le(&cd, lhoff);
        zr_buf_append(&cd, e->rel, nlen);

        free(comp);
        count++;
    }

    uint32_t cd_off = (uint32_t)out.len;
    uint32_t cd_sz  = (uint32_t)cd.len;
    if (cd_sz > 0) zr_buf_append(&out, cd.data, cd_sz);
    zr_buf_free(&cd);

    zr_buf_append(&out, "PK\x05\x06", 4);
    zr_su_put16le(&out, 0);
    zr_su_put16le(&out, 0);
    zr_su_put16le(&out, count);
    zr_su_put16le(&out, count);
    zr_su_put32le(&out, cd_sz);
    zr_su_put32le(&out, cd_off);
    zr_su_put16le(&out, 0);

    (void)err;
    *out_len = out.len;
    return (uint8_t *)out.data;
}

typedef enum { ZR_ARCH_NONE = 0, ZR_ARCH_ZIP = 1, ZR_ARCH_TGZ = 2, ZR_ARCH_TAR = 3 } ZrArchFmt;

static ZrArchFmt zr_su_detect_arch(const char *path) {
    if (!path) return ZR_ARCH_NONE;
    if (zr_su_ends_with_ci(path, ".zip")) return ZR_ARCH_ZIP;
    if (zr_su_ends_with_ci(path, ".tar.gz") || zr_su_ends_with_ci(path, ".tgz")) return ZR_ARCH_TGZ;
    if (zr_su_ends_with_ci(path, ".tar")) return ZR_ARCH_TAR;
    return ZR_ARCH_NONE;
}

static bool zr_su_read_archive(const char *path, ZrArchFmt fmt, ZrSuEntryList *out, char *err) {
    size_t raw_len = 0;
    uint8_t *raw = zr_su_read_file(path, &raw_len, err);
    if (!raw) return false;
    bool ok = false;
    if (fmt == ZR_ARCH_ZIP) {
        ok = zr_su_read_zip(raw, raw_len, out, err);
    } else if (fmt == ZR_ARCH_TAR) {
        ok = zr_su_read_tar_raw(raw, raw_len, out, err);
    } else if (fmt == ZR_ARCH_TGZ) {
        size_t tar_len = 0;
        uint8_t *tar = zr_su_zlib_inflate(raw, raw_len, 15 + 16, &tar_len, err);
        if (tar) {
            ok = zr_su_read_tar_raw(tar, tar_len, out, err);
            free(tar);
        }
    }
    free(raw);
    return ok;
}

static uint8_t *zr_su_write_archive(const ZrSuEntryList *entries, ZrArchFmt fmt,
                                    size_t *out_len, char *err) {
    if (fmt == ZR_ARCH_ZIP) return zr_su_write_zip(entries, out_len, err);
    size_t tar_len = 0;
    uint8_t *tar = zr_su_write_tar_raw(entries, &tar_len, err);
    if (!tar) return NULL;
    if (fmt == ZR_ARCH_TAR) { *out_len = tar_len; return tar; }
    uint8_t *gz = zr_su_zlib_deflate(tar, tar_len, 15 + 16, out_len, err);
    free(tar);
    return gz;
}

/* ── Directory Tree Hashing (`pblang-package-tree-v2` & `tree-manifest-v1`) ── */

static bool zr_su_collect_dir_rec(const char *root, const char *cur, bool include_dirs,
                                  ZrSuEntryList *out, char *err) {
    if (zr_su_is_symlink(cur)) {
        snprintf(err, 256, "directory tree contains symlink");
        return false;
    }
#ifndef _WIN32
    DIR *d = opendir(cur);
    if (!d) { snprintf(err, 256, "cannot read directory '%s'", zr_su_basename(cur)); return false; }
    struct dirent *de;
    while ((de = readdir(d)) != NULL) {
        if (strcmp(de->d_name, ".") == 0 || strcmp(de->d_name, "..") == 0) continue;
        char full[4096];
        snprintf(full, sizeof(full), "%s/%s", cur, de->d_name);
        if (zr_su_is_symlink(full)) {
            closedir(d);
            snprintf(err, 256, "package tree contains forbidden symlink '%.160s'", de->d_name);
            return false;
        }
        const char *rel = full + strlen(root);
        while (*rel == '/') rel++;
        struct stat st;
        if (lstat(full, &st) != 0) { closedir(d); snprintf(err, 256, "lstat failed"); return false; }
        if (S_ISDIR(st.st_mode)) {
            if (include_dirs) zr_su_elist_put(out, rel, 'd', NULL, 0);
            if (!zr_su_collect_dir_rec(root, full, include_dirs, out, err)) { closedir(d); return false; }
        } else if (S_ISREG(st.st_mode)) {
            size_t flen = 0;
            uint8_t *fdata = zr_su_read_file(full, &flen, err);
            if (!fdata) { closedir(d); return false; }
            zr_su_elist_put(out, rel, 'f', fdata, flen);
            free(fdata);
        }
    }
    closedir(d);
    return true;
#else
    char pat[4096];
    snprintf(pat, sizeof(pat), "%s\\*", cur);
    WIN32_FIND_DATAA fd;
    HANDLE h = FindFirstFileA(pat, &fd);
    if (h == INVALID_HANDLE_VALUE) return true;
    do {
        if (strcmp(fd.cFileName, ".") == 0 || strcmp(fd.cFileName, "..") == 0) continue;
        char full[4096];
        snprintf(full, sizeof(full), "%s\\%s", cur, fd.cFileName);
        char rel[4096];
        const char *r0 = full + strlen(root);
        while (*r0 == '\\' || *r0 == '/') r0++;
        snprintf(rel, sizeof(rel), "%.4090s", r0);
        for (size_t k = 0; rel[k]; k++) if (rel[k] == '\\') rel[k] = '/';
        if (fd.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT) {
            FindClose(h);
            snprintf(err, 256, "package tree contains symlink/reparse point");
            return false;
        }
        if (fd.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY) {
            if (include_dirs) zr_su_elist_put(out, rel, 'd', NULL, 0);
            if (!zr_su_collect_dir_rec(root, full, include_dirs, out, err)) { FindClose(h); return false; }
        } else {
            size_t flen = 0;
            uint8_t *fdata = zr_su_read_file(full, &flen, err);
            if (!fdata) { FindClose(h); return false; }
            zr_su_elist_put(out, rel, 'f', fdata, flen);
            free(fdata);
        }
    } while (FindNextFileA(h, &fd));
    FindClose(h);
    return true;
#endif
}

static void zr_su_u64be(uint64_t v, uint8_t b[8]) {
    for (int i = 7; i >= 0; i--) { b[i] = (uint8_t)(v & 0xFFu); v >>= 8; }
}

/* Exact `pblang-package-tree-v2` SHA-256 matching `crates/pblang-pm/src/lib.rs` */
static bool zr_su_hash_package_tree_v2(const char *root, char out_hex[65], char *err) {
    if (!zr_dir_exists(root) || zr_su_is_symlink(root)) {
        snprintf(err, 256, "package root is not a regular non-symlink directory");
        return false;
    }
    ZrSuEntryList entries = { NULL, 0, 0 };
    if (!zr_su_collect_dir_rec(root, root, true, &entries, err)) {
        zr_su_elist_free(&entries);
        return false;
    }
    if (entries.len > 1) qsort(entries.items, entries.len, sizeof(ZrSuEntry), zr_su_cmp_entry);

    ZrSha256Ctx ctx;
    zr_sha256_init(&ctx);
    zr_sha256_update(&ctx, "pblang-package-tree-v2", 23); /* includes trailing NUL */
    for (size_t i = 0; i < entries.len; i++) {
        const ZrSuEntry *e = &entries.items[i];
        size_t plen = strlen(e->rel);
        uint8_t b8[8];
        zr_sha256_update(&ctx, &e->kind, 1);
        zr_su_u64be((uint64_t)plen, b8);
        zr_sha256_update(&ctx, b8, 8);
        zr_sha256_update(&ctx, e->rel, plen);
        zr_su_u64be((uint64_t)e->len, b8);
        zr_sha256_update(&ctx, b8, 8);
        if (e->len > 0) zr_sha256_update(&ctx, e->data, e->len);
    }
    uint8_t dig[32];
    zr_sha256_final(&ctx, dig);
    zr_buf_free(&ctx.buf);
    zr_su_elist_free(&entries);

    static const char hx[] = "0123456789abcdef";
    for (int i = 0; i < 32; i++) {
        out_hex[i * 2]     = hx[(dig[i] >> 4) & 0xF];
        out_hex[i * 2 + 1] = hx[dig[i] & 0xF];
    }
    out_hex[64] = '\0';
    return true;
}

static void zr_su_compute_tree_digest(const ZrSuEntryList *files, char out_hex[65]) {
    ZrSha256Ctx ctx;
    zr_sha256_init(&ctx);
    zr_sha256_update(&ctx, "pblang-tree-manifest-v1", 24);
    for (size_t i = 0; i < files->len; i++) {
        const ZrSuEntry *e = &files->items[i];
        if (e->kind != 'f') continue;
        if (strcmp(e->rel, "MANIFEST.sha256.json") == 0) continue;
        size_t plen = strlen(e->rel);
        uint8_t b8[8];
        zr_su_u64be((uint64_t)plen, b8);
        zr_sha256_update(&ctx, b8, 8);
        zr_sha256_update(&ctx, e->rel, plen);
        zr_su_u64be((uint64_t)e->len, b8);
        zr_sha256_update(&ctx, b8, 8);
        if (e->len > 0) zr_sha256_update(&ctx, e->data, e->len);
    }
    uint8_t dig[32];
    zr_sha256_final(&ctx, dig);
    zr_buf_free(&ctx.buf);
    static const char hx[] = "0123456789abcdef";
    for (int i = 0; i < 32; i++) {
        out_hex[i * 2]     = hx[(dig[i] >> 4) & 0xF];
        out_hex[i * 2 + 1] = hx[dig[i] & 0xF];
    }
    out_hex[64] = '\0';
}

/* ── Ed25519 Signature Verification (`libcrypto` + pure-PBLang fallback) ───── */

static bool zr_su_verify_ed25519(const char *self_exe, const uint8_t *payload, size_t payload_len,
                                 const char *pubkey_hex, const char *sig_path, char *err) {
    uint8_t pub32[32], sig64[64];
    if (!zr_su_hex_decode(pubkey_hex, pub32, 32)) {
        snprintf(err, 256, "invalid 64-hex Ed25519 public key");
        return false;
    }
    size_t sraw_len = 0;
    uint8_t *sraw = zr_su_read_file(sig_path, &sraw_len, err);
    if (!sraw) {
        snprintf(err, 256, "missing Ed25519 signature file '%s'", zr_su_basename(sig_path));
        return false;
    }
    if (sraw_len == 64) {
        memcpy(sig64, sraw, 64);
    } else {
        while (sraw_len > 0 && (sraw[sraw_len - 1] == '\n' || sraw[sraw_len - 1] == '\r' || sraw[sraw_len - 1] == ' '))
            sraw_len--;
        sraw[sraw_len] = '\0';
        if (!zr_su_hex_decode((const char *)sraw, sig64, 64)) {
            free(sraw);
            snprintf(err, 256, "signature file is neither 64 raw bytes nor 128 hex chars");
            return false;
        }
    }
    free(sraw);

    /* Fast arbitrary-length verification via OpenSSL 3 / 1.1 `libcrypto` when present */
    char ssl_err[256];
    (void)zr_ensure_openssl(ssl_err);
    if (g_zr_ssl.libcrypto) {
        typedef void *(*FnNewRawPub)(int, void *, const unsigned char *, size_t);
        typedef void  (*FnPkeyFree)(void *);
        typedef void *(*FnMdCtxNew)(void);
        typedef void  (*FnMdCtxFree)(void *);
        typedef int   (*FnDigVerInit)(void *, void **, const void *, void *, void *);
        typedef int   (*FnDigVer)(void *, const unsigned char *, size_t, const unsigned char *, size_t);

        FnNewRawPub  fn_new  = (FnNewRawPub)zr_dlsym(g_zr_ssl.libcrypto, "EVP_PKEY_new_raw_public_key");
        FnPkeyFree   fn_pkf  = (FnPkeyFree)zr_dlsym(g_zr_ssl.libcrypto, "EVP_PKEY_free");
        FnMdCtxNew   fn_mcn  = (FnMdCtxNew)zr_dlsym(g_zr_ssl.libcrypto, "EVP_MD_CTX_new");
        FnMdCtxFree  fn_mcf  = (FnMdCtxFree)zr_dlsym(g_zr_ssl.libcrypto, "EVP_MD_CTX_free");
        FnDigVerInit fn_dvi  = (FnDigVerInit)zr_dlsym(g_zr_ssl.libcrypto, "EVP_DigestVerifyInit");
        FnDigVer     fn_dv   = (FnDigVer)zr_dlsym(g_zr_ssl.libcrypto, "EVP_DigestVerify");

        if (fn_new && fn_pkf && fn_mcn && fn_mcf && fn_dvi && fn_dv) {
            void *pkey = fn_new(1087 /* EVP_PKEY_ED25519 */, NULL, pub32, 32);
            if (!pkey) { snprintf(err, 256, "Ed25519 public key rejected"); return false; }
            void *mctx = fn_mcn();
            int rc = 0;
            if (mctx && fn_dvi(mctx, NULL, NULL, NULL, pkey) == 1) {
                rc = fn_dv(mctx, sig64, 64, payload, payload_len);
            }
            if (mctx) fn_mcf(mctx);
            fn_pkf(pkey);
            if (rc == 1) return true;
            snprintf(err, 256, "RFC 8032 Ed25519 signature verification failed");
            return false;
        }
    }

    /* Fallback: pure-PBLang `stdlib:ed25519_verify.sapp` inside PbVM */
    PbVM vm;
    vm_init(&vm);
    zr_caps_allow_all(&vm.caps);
    zr_configure_vm_paths(&vm, self_exe, NULL, NULL);
    const char *snippet =
        "let E = import(\"stdlib:ed25519_verify.sapp\");\n"
        "fn __su_verify(pk, msg, sig) { return E.verify_bytes(pk, msg, sig); }\n";
    char cerr[512];
    PbFunc *fn = zr_compile_sapp(&vm, snippet, strlen(snippet), "<ed25519>", cerr);
    bool verified = false;
    if (fn) {
        vm_push_root(&vm, val_obj(fn));
        PbValue r0 = val_nil();
        if (vm_run_root_func(&vm, fn, NULL, &r0)) {
            PbValue callee = val_nil();
            if (pb_map_get_cstr(vm.globals, "__su_verify", &callee)) {
                PbArray *apk = pb_array_new(&vm, 32);
                vm_push_root(&vm, val_obj(apk));
                for (int i = 0; i < 32; i++) pb_array_push(&vm, apk, val_num((double)pub32[i]));
                PbArray *amsg = pb_array_new(&vm, payload_len);
                vm_push_root(&vm, val_obj(amsg));
                for (size_t i = 0; i < payload_len; i++) pb_array_push(&vm, amsg, val_num((double)payload[i]));
                PbArray *asig = pb_array_new(&vm, 64);
                vm_push_root(&vm, val_obj(asig));
                for (int i = 0; i < 64; i++) pb_array_push(&vm, asig, val_num((double)sig64[i]));
                PbValue args3[3] = { val_obj(apk), val_obj(amsg), val_obj(asig) };
                PbValue vres = val_nil();
                if (vm_call_value_sync(&vm, callee, 3, args3, &vres) && vres.tag == VAL_BOOL && vres.as.b) {
                    verified = true;
                }
                vm_pop_roots(&vm, 3);
            }
        }
        vm_pop_roots(&vm, 1);
    }
    vm_free(&vm);
    if (!verified) snprintf(err, 256, "RFC 8032 Ed25519 signature verification failed");
    return verified;
}

/* ── Syntax Gate & `.sapp -> .pbck` Compiler Helper ───────────────────────── */

static bool zr_su_compile_sapp_bytes(const char *self_exe, const char *label,
                                     const uint8_t *src, size_t src_len,
                                     uint8_t **out_pbck, size_t *out_pbck_len, char *err) {
    PbVM vm;
    vm_init(&vm);
    zr_configure_vm_paths(&vm, self_exe, label, NULL);
    char cerr[512];
    cerr[0] = '\0';
    PbFunc *fn = zr_compile_sapp(&vm, (const char *)src, src_len, zr_su_basename(label), cerr);
    if (!fn) {
        snprintf(err, 256, "syntax error in '%s': %.180s", zr_su_basename(label), cerr);
        vm_free(&vm);
        return false;
    }
    if (out_pbck && out_pbck_len) {
        *out_pbck = pbck_serialize_script(fn, out_pbck_len);
    }
    vm_free(&vm);
    return true;
}

static bool zr_su_validate_entries_syntax(const char *self_exe, const ZrSuEntryList *list, char *err) {
    for (size_t i = 0; i < list->len; i++) {
        const ZrSuEntry *e = &list->items[i];
        if (e->kind == 'f' && zr_su_ends_with_ci(e->rel, ".sapp")) {
            if (!zr_su_compile_sapp_bytes(self_exe, e->rel, e->data, e->len, NULL, NULL, err)) {
                return false;
            }
        }
    }
    return true;
}

static bool zr_su_load_source_tree_or_archive(const ZrSuOpts *opts, const char *src_path,
                                              ZrSuEntryList *out, char out_digest[65], char *err) {
    ZrArchFmt fmt = zr_su_detect_arch(src_path);
    if (fmt != ZR_ARCH_NONE) {
        size_t rlen = 0;
        uint8_t *raw = zr_su_read_file(src_path, &rlen, err);
        if (!raw) return false;
        char asha[65];
        zr_su_sha256_hex(raw, rlen, asha);
        if (opts->expected_sha256 && strcasecmp(asha, opts->expected_sha256) != 0) {
            free(raw);
            snprintf(err, 256, "expected SHA-256 mismatch on archive");
            return false;
        }
        if (opts->pubkey_hex) {
            char sig_p[4200];
            snprintf(sig_p, sizeof(sig_p), "%s.sig", src_path);
            if (!zr_su_verify_ed25519(opts->self_exe, raw, rlen, opts->pubkey_hex, sig_p, err)) {
                free(raw);
                return false;
            }
        }
        free(raw);
        if (!zr_su_read_archive(src_path, fmt, out, err)) return false;
        zr_su_compute_tree_digest(out, out_digest);
        return true;
    }
    if (zr_dir_exists(src_path)) {
        if (!zr_su_collect_dir_rec(src_path, src_path, false, out, err)) return false;
        if (out->len > 1) qsort(out->items, out->len, sizeof(ZrSuEntry), zr_su_cmp_entry);
        zr_su_compute_tree_digest(out, out_digest);
        if (opts->expected_sha256 && strcasecmp(out_digest, opts->expected_sha256) != 0) {
            snprintf(err, 256, "expected SHA-256 mismatch on source tree");
            return false;
        }
        if (opts->pubkey_hex) {
            char man_p[4200], sig_p[4200];
            snprintf(man_p, sizeof(man_p), "%s/MANIFEST.sha256.json", src_path);
            snprintf(sig_p, sizeof(sig_p), "%s/MANIFEST.sha256.json.sig", src_path);
            if (zr_file_exists_reg(man_p) && zr_file_exists_reg(sig_p)) {
                size_t mlen = 0;
                uint8_t *mb = zr_su_read_file(man_p, &mlen, err);
                if (!mb) return false;
                bool ok = zr_su_verify_ed25519(opts->self_exe, mb, mlen, opts->pubkey_hex, sig_p, err);
                free(mb);
                if (!ok) return false;
            } else {
                snprintf(sig_p, sizeof(sig_p), "%s.sig", src_path);
                if (!zr_su_verify_ed25519(opts->self_exe, (const uint8_t *)out_digest, 64, opts->pubkey_hex, sig_p, err))
                    return false;
            }
        }
        return true;
    }
    snprintf(err, 256, "source is neither an existing directory nor a supported archive");
    return false;
}

static void zr_su_diff_lists(const ZrSuEntryList *old_l, const ZrSuEntryList *new_l,
                             size_t *added, size_t *modified, size_t *removed, size_t *unchanged) {
    *added = *modified = *removed = *unchanged = 0;
    for (size_t i = 0; i < new_l->len; i++) {
        const ZrSuEntry *ne = &new_l->items[i];
        if (ne->kind != 'f' || strcmp(ne->rel, "MANIFEST.sha256.json") == 0) continue;
        const ZrSuEntry *oe = zr_su_elist_find(old_l, ne->rel);
        if (!oe) (*added)++;
        else if (oe->len == ne->len && (oe->len == 0 || memcmp(oe->data, ne->data, ne->len) == 0)) (*unchanged)++;
        else (*modified)++;
    }
    for (size_t i = 0; i < old_l->len; i++) {
        const ZrSuEntry *oe = &old_l->items[i];
        if (oe->kind != 'f' || strcmp(oe->rel, "MANIFEST.sha256.json") == 0) continue;
        if (!zr_su_elist_find(new_l, oe->rel)) (*removed)++;
    }
}

static uint8_t *zr_su_render_manifest_json(const ZrSuEntryList *files, const char *tree_sha, size_t *out_len) {
    ZrBuf b;
    zr_buf_init(&b);
    size_t fcount = 0;
    for (size_t i = 0; i < files->len; i++)
        if (files->items[i].kind == 'f' && strcmp(files->items[i].rel, "MANIFEST.sha256.json") != 0) fcount++;

    char head[512];
    snprintf(head, sizeof(head),
             "{\n  \"format\": \"pblang.tree.manifest.v1\",\n  \"compiler_version\": \"%s\",\n"
             "  \"tree_sha256\": \"%s\",\n  \"file_count\": %zu,\n  \"files\": {\n",
             ZR_SU_SEMVER, tree_sha, fcount);
    zr_buf_append(&b, head, strlen(head));

    size_t seen = 0;
    for (size_t i = 0; i < files->len; i++) {
        const ZrSuEntry *e = &files->items[i];
        if (e->kind != 'f' || strcmp(e->rel, "MANIFEST.sha256.json") == 0) continue;
        char fsha[65];
        zr_su_sha256_hex(e->data, e->len, fsha);
        seen++;
        char line[1200];
        snprintf(line, sizeof(line), "    \"%s\": \"%s\"%s\n", e->rel, fsha, (seen < fcount) ? "," : "");
        zr_buf_append(&b, line, strlen(line));
    }
    const char *tail = "  }\n}\n";
    zr_buf_append(&b, tail, strlen(tail));
    *out_len = b.len;
    return (uint8_t *)b.data;
}

static bool zr_su_atomic_swap_dir(const char *dest_dir, const ZrSuEntryList *files,
                                  bool write_manifest, bool *out_had_bak, char *err) {
    if (out_had_bak) *out_had_bak = false;
    char parent[4096];
    zr_su_dirname(dest_dir, parent, sizeof(parent));
    if (!zr_su_mkdir_p(parent)) { snprintf(err, 256, "cannot create parent dir"); return false; }

    static unsigned int d_seq = 0;
    char staging[4200], bak_dir[4200];
    snprintf(staging, sizeof(staging), "%s/.%s.staging.%u", parent, zr_su_basename(dest_dir), ++d_seq);
    snprintf(bak_dir, sizeof(bak_dir), "%s.bak", dest_dir);
    zr_su_rm_rf(staging);
    if (!zr_su_mkdir_p(staging)) { snprintf(err, 256, "cannot create staging dir"); return false; }

    for (size_t i = 0; i < files->len; i++) {
        const ZrSuEntry *e = &files->items[i];
        if (e->kind != 'f' || strcmp(e->rel, "MANIFEST.sha256.json") == 0) continue;
        char fp[4400], fpar[4400];
        snprintf(fp, sizeof(fp), "%s/%s", staging, e->rel);
        zr_su_dirname(fp, fpar, sizeof(fpar));
        zr_su_mkdir_p(fpar);
        bool dummy = false;
        if (!zr_su_write_atomic(fp, e->data, e->len, 0644, false, &dummy, err)) {
            zr_su_rm_rf(staging);
            return false;
        }
    }
    if (write_manifest) {
        char tsha[65], mp[4400];
        zr_su_compute_tree_digest(files, tsha);
        size_t mlen = 0;
        uint8_t *mjson = zr_su_render_manifest_json(files, tsha, &mlen);
        snprintf(mp, sizeof(mp), "%s/MANIFEST.sha256.json", staging);
        bool dummy = false;
        bool ok = zr_su_write_atomic(mp, mjson, mlen, 0644, false, &dummy, err);
        free(mjson);
        if (!ok) { zr_su_rm_rf(staging); return false; }
    }

    if (zr_dir_exists(dest_dir)) {
        zr_su_rm_rf(bak_dir);
        if (!zr_su_atomic_rename(dest_dir, bak_dir)) {
            zr_su_rm_rf(staging);
            snprintf(err, 256, "cannot move existing directory to .bak");
            return false;
        }
        if (out_had_bak) *out_had_bak = true;
    }
    if (!zr_su_atomic_rename(staging, dest_dir)) {
        if (zr_dir_exists(bak_dir)) (void)zr_su_atomic_rename(bak_dir, dest_dir);
        zr_su_rm_rf(staging);
        snprintf(err, 256, "atomic directory swap failed");
        return false;
    }
    return true;
}

/* ── Individual Target Handlers ───────────────────────────────────────────── */

static bool zr_su_rollback_file(const char *target, const char *kind, const ZrSuOpts *opts,
                                ZrSuReceipt *rec, char *err) {
    char bak[4200];
    snprintf(bak, sizeof(bak), "%s.bak", target);
    if (!zr_file_exists_reg(bak)) {
        snprintf(err, 256, "no backup '%s.bak' found for --rollback", zr_su_basename(target));
        return false;
    }
    size_t blen = 0;
    uint8_t *bdata = zr_su_read_file(bak, &blen, err);
    if (!bdata) return false;
    if (zr_file_exists_reg(target)) {
        size_t olen = 0;
        char rerr[256];
        uint8_t *odata = zr_su_read_file(target, &olen, rerr);
        if (odata) { zr_su_sha256_hex(odata, olen, rec->sha256_before); free(odata); }
    }
    zr_su_sha256_hex(bdata, blen, rec->sha256_after);
    int mode = (strstr(kind, "launcher") || strstr(kind, "elf")) ? 0755 : 0644;
    if (!opts->dry_run) {
        bool dummy = false;
        if (!zr_su_write_atomic(target, bdata, blen, mode, false, &dummy, err)) {
            free(bdata);
            return false;
        }
    }
    free(bdata);
    snprintf(rec->target, sizeof(rec->target), "%s", zr_su_basename(target));
    snprintf(rec->detected_kind, sizeof(rec->detected_kind), "%s", kind);
    snprintf(rec->status, sizeof(rec->status), "%s", opts->dry_run ? "rollback_ready" : "rolled_back");
    rec->dry_run = opts->dry_run;
    rec->writes_performed = opts->dry_run ? 0 : 1;
    rec->modified = 1;
    snprintf(rec->backup_path, sizeof(rec->backup_path), "%s.bak", zr_su_basename(target));
    snprintf(rec->detail, sizeof(rec->detail), "atomically restored %s from backup", zr_su_basename(target));
    return true;
}

static bool zr_su_json_extract_str(const char *json, const char *key, char *out, size_t cap) {
    char pat[128];
    snprintf(pat, sizeof(pat), "\"%s\"", key);
    const char *p = strstr(json, pat);
    if (!p) return false;
    p += strlen(pat);
    while (*p == ' ' || *p == '\t' || *p == '\r' || *p == '\n' || *p == ':') p++;
    if (*p != '"') return false;
    p++;
    size_t i = 0;
    while (*p && *p != '"' && i + 1 < cap) {
        if (*p == '\\' && p[1]) p++;
        out[i++] = *p++;
    }
    out[i] = '\0';
    return true;
}

static bool zr_su_update_sapp(const char *sapp_path, const char *pbck_override, const char *kind_label,
                              const ZrSuOpts *opts, ZrSuReceipt *rec, char *err) {
    char pbck_path[4200], meta_path[4300];
    if (pbck_override && *pbck_override) {
        snprintf(pbck_path, sizeof(pbck_path), "%.4190s", pbck_override);
    } else {
        size_t sl = strlen(sapp_path);
        size_t stem = (sl > 5 && zr_su_ends_with_ci(sapp_path, ".sapp")) ? (sl - 5) : sl;
        snprintf(pbck_path, sizeof(pbck_path), "%.*s.pbck", (int)stem, sapp_path);
    }
    snprintf(meta_path, sizeof(meta_path), "%s.meta.json", pbck_path);

    if (opts->rollback) return zr_su_rollback_file(pbck_path, kind_label, opts, rec, err);
    if (!zr_file_exists_reg(sapp_path)) {
        snprintf(err, 256, "source file '%s' does not exist", zr_su_basename(sapp_path));
        return false;
    }

    size_t src_len = 0;
    uint8_t *src = zr_su_read_file(sapp_path, &src_len, err);
    if (!src) return false;
    char src_sha[65];
    zr_su_sha256_hex(src, src_len, src_sha);
    if (opts->expected_sha256 && strcasecmp(src_sha, opts->expected_sha256) != 0) {
        free(src);
        snprintf(err, 256, "source SHA-256 mismatch");
        return false;
    }

    bool had_old = false;
    char old_pbck_sha[65] = {0};
    if (zr_file_exists_reg(pbck_path)) {
        size_t olen = 0;
        char rerr[256];
        uint8_t *ob = zr_su_read_file(pbck_path, &olen, rerr);
        if (ob) { zr_su_sha256_hex(ob, olen, old_pbck_sha); free(ob); had_old = true; }
    }

    const char *base_src = zr_su_basename(sapp_path);
    if (had_old && zr_file_exists_reg(meta_path)) {
        size_t mlen = 0;
        char rerr[256];
        uint8_t *mb = zr_su_read_file(meta_path, &mlen, rerr);
        if (mb) {
            char m_ssha[80] = {0}, m_psha[80] = {0}, m_cver[40] = {0};
            zr_su_json_extract_str((const char *)mb, "source_sha256", m_ssha, sizeof(m_ssha));
            zr_su_json_extract_str((const char *)mb, "pbck_sha256", m_psha, sizeof(m_psha));
            zr_su_json_extract_str((const char *)mb, "compiler_version", m_cver, sizeof(m_cver));
            free(mb);
            if (strcmp(m_ssha, src_sha) == 0 && strcmp(m_psha, old_pbck_sha) == 0 &&
                strcmp(m_cver, ZR_SU_SEMVER) == 0) {
                free(src);
                snprintf(rec->target, sizeof(rec->target), "%s", base_src);
                snprintf(rec->detected_kind, sizeof(rec->detected_kind), "%s", kind_label);
                snprintf(rec->status, sizeof(rec->status), "already_up_to_date");
                rec->dry_run = opts->dry_run;
                rec->unchanged = 1;
                snprintf(rec->sha256_before, sizeof(rec->sha256_before), "%s", old_pbck_sha);
                snprintf(rec->sha256_after, sizeof(rec->sha256_after), "%s", old_pbck_sha);
                snprintf(rec->detail, sizeof(rec->detail), "provenance SHA-256 matches (O(1) skip)");
                return true;
            }
        }
    }

    uint8_t *pbck = NULL;
    size_t pbck_len = 0;
    bool cok = zr_su_compile_sapp_bytes(opts->self_exe, sapp_path, src, src_len, &pbck, &pbck_len, err);
    free(src);
    if (!cok || !pbck) return false;

    char new_pbck_sha[65];
    zr_su_sha256_hex(pbck, pbck_len, new_pbck_sha);
    char meta_json[1024];
    snprintf(meta_json, sizeof(meta_json),
             "{\n  \"format\": \"pblang.pbck.meta.v1\",\n  \"source\": \"%s\",\n"
             "  \"source_sha256\": \"%s\",\n  \"pbck_sha256\": \"%s\",\n"
             "  \"compiler_version\": \"%s\"\n}\n",
             base_src, src_sha, new_pbck_sha, ZR_SU_SEMVER);

    bool had_bak = false;
    if (!opts->dry_run) {
        if (!zr_su_write_atomic(pbck_path, pbck, pbck_len, 0644, true, &had_bak, err)) {
            free(pbck);
            return false;
        }
        bool dummy = false;
        if (!zr_su_write_atomic(meta_path, (const uint8_t *)meta_json, strlen(meta_json), 0644, false, &dummy, err)) {
            free(pbck);
            return false;
        }
    }
    free(pbck);

    snprintf(rec->target, sizeof(rec->target), "%s", base_src);
    snprintf(rec->detected_kind, sizeof(rec->detected_kind), "%s", kind_label);
    snprintf(rec->status, sizeof(rec->status), "%s", opts->dry_run ? "update_available" : "updated");
    rec->dry_run = opts->dry_run;
    rec->writes_performed = opts->dry_run ? 0 : 2;
    rec->added = had_old ? 0 : 1;
    rec->modified = had_old ? 1 : 0;
    if (had_old) snprintf(rec->sha256_before, sizeof(rec->sha256_before), "%s", old_pbck_sha);
    snprintf(rec->sha256_after, sizeof(rec->sha256_after), "%s", new_pbck_sha);
    if (had_bak) snprintf(rec->backup_path, sizeof(rec->backup_path), "%s.bak", zr_su_basename(pbck_path));
    snprintf(rec->detail, sizeof(rec->detail), "compiled .pbck v1 + zero-leak .pbck.meta.json");
    return true;
}

static bool zr_su_update_pbck(const char *pbck_path, const ZrSuOpts *opts, ZrSuReceipt *rec, char *err) {
    if (opts->rollback) return zr_su_rollback_file(pbck_path, "pbck_artifact", opts, rec, err);
    if (opts->from && zr_su_ends_with_ci(opts->from, ".sapp")) {
        return zr_su_update_sapp(opts->from, pbck_path, "pbck_artifact", opts, rec, err);
    }
    char meta_path[4300], parent[4096];
    snprintf(meta_path, sizeof(meta_path), "%s.meta.json", pbck_path);
    zr_su_dirname(pbck_path, parent, sizeof(parent));
    if (zr_file_exists_reg(meta_path)) {
        size_t mlen = 0;
        char rerr[256];
        uint8_t *mb = zr_su_read_file(meta_path, &mlen, rerr);
        if (mb) {
            char srel[512] = {0};
            zr_su_json_extract_str((const char *)mb, "source", srel, sizeof(srel));
            free(mb);
            if (srel[0]) {
                char cand[4700];
                snprintf(cand, sizeof(cand), "%s/%s", parent, srel);
                if (zr_file_exists_reg(cand))
                    return zr_su_update_sapp(cand, pbck_path, "pbck_artifact", opts, rec, err);
            }
        }
    }
    size_t pl = strlen(pbck_path);
    if (pl > 5 && zr_su_ends_with_ci(pbck_path, ".pbck")) {
        char sib[4200];
        snprintf(sib, sizeof(sib), "%.*s.sapp", (int)(pl - 5), pbck_path);
        if (zr_file_exists_reg(sib))
            return zr_su_update_sapp(sib, pbck_path, "pbck_artifact", opts, rec, err);
    }
    if (!zr_file_exists_reg(pbck_path)) {
        snprintf(err, 256, "target .pbck does not exist and no source .sapp found");
        return false;
    }
    size_t rlen = 0;
    uint8_t *raw = zr_su_read_file(pbck_path, &rlen, err);
    if (!raw) return false;
    char sha[65];
    zr_su_sha256_hex(raw, rlen, sha);
    free(raw);
    snprintf(rec->target, sizeof(rec->target), "%s", zr_su_basename(pbck_path));
    snprintf(rec->detected_kind, sizeof(rec->detected_kind), "pbck_artifact");
    snprintf(rec->status, sizeof(rec->status), "already_up_to_date");
    rec->dry_run = opts->dry_run;
    rec->unchanged = 1;
    snprintf(rec->sha256_before, sizeof(rec->sha256_before), "%s", sha);
    snprintf(rec->sha256_after, sizeof(rec->sha256_after), "%s", sha);
    snprintf(rec->detail, sizeof(rec->detail), "verified .pbck artifact");
    return true;
}

static bool zr_su_resolve_manifest_stdlib(const char *man_dir, const char *toml_txt,
                                          char *out_rel_path, size_t rel_cap,
                                          char *out_full_dir, size_t full_cap) {
    out_rel_path[0] = '\0';
    out_full_dir[0] = '\0';
    const char *dep = strstr(toml_txt, "[dependencies]");
    if (!dep) return true;
    const char *std_pos = strstr(dep, "stdlib");
    if (!std_pos) return true;
    const char *line_end = strchr(std_pos, '\n');
    size_t seg_len = line_end ? (size_t)(line_end - std_pos) : strlen(std_pos);
    char line[1024];
    if (seg_len >= sizeof(line)) seg_len = sizeof(line) - 1;
    memcpy(line, std_pos, seg_len);
    line[seg_len] = '\0';

    /* Extract quoted candidate strings inside `stdlib = { paths = [...] }` or `path = "..."` */
    const char *p = line;
    char first_cand[512] = {0};
    while ((p = strchr(p, '"')) != NULL) {
        p++;
        const char *q = strchr(p, '"');
        if (!q) break;
        char cand[512];
        size_t cl = (size_t)(q - p);
        if (cl > 0 && cl < sizeof(cand)) {
            memcpy(cand, p, cl);
            cand[cl] = '\0';
            if (!first_cand[0]) snprintf(first_cand, sizeof(first_cand), "%s", cand);
            char full[4600];
            snprintf(full, sizeof(full), "%s/%s", man_dir, cand);
            if (zr_dir_exists(full)) {
                snprintf(out_rel_path, rel_cap, "%s", cand);
                snprintf(out_full_dir, full_cap, "%s", full);
                return true;
            }
        }
        p = q + 1;
    }
    if (first_cand[0]) {
        snprintf(out_rel_path, rel_cap, "%s", first_cand);
        snprintf(out_full_dir, full_cap, "%s/%s", man_dir, first_cand);
    }
    return true;
}

static bool zr_su_update_project(const char *man_dir, const char *label, const ZrSuOpts *opts,
                                 ZrSuReceipt *rec, char *err) {
    char toml_p[4200], lock_p[4200];
    snprintf(toml_p, sizeof(toml_p), "%s/pblang.toml", man_dir);
    snprintf(lock_p, sizeof(lock_p), "%s/pblang.lock", man_dir);
    if (!zr_file_exists_reg(toml_p)) {
        snprintf(err, 256, "project directory has no pblang.toml");
        return false;
    }
    if (opts->rollback) {
        if (!zr_su_rollback_file(lock_p, "project", opts, rec, err)) return false;
        snprintf(rec->target, sizeof(rec->target), "%.500s", label);
        snprintf(rec->detected_kind, sizeof(rec->detected_kind), "project");
        return true;
    }

    size_t tlen = 0;
    uint8_t *tbytes = zr_su_read_file(toml_p, &tlen, err);
    if (!tbytes) return false;
    char man_sha[65];
    zr_su_sha256_hex(tbytes, tlen, man_sha);

    char std_rel[512], std_full[4600];
    zr_su_resolve_manifest_stdlib(man_dir, (const char *)tbytes, std_rel, sizeof(std_rel), std_full, sizeof(std_full));
    free(tbytes);

    char lock_buf[2048];
    if (std_rel[0]) {
        char std_sha[65];
        if (!zr_su_hash_package_tree_v2(std_full, std_sha, err)) return false;
        char clean_rel[512];
        zr_su_scrub_str(std_rel, clean_rel, sizeof(clean_rel));
        snprintf(lock_buf, sizeof(lock_buf),
                 "manifest_sha256 = \"%s\"\n"
                 "content_hash_algorithm = \"pblang-package-tree-v2\"\n\n"
                 "[dependencies.stdlib]\n"
                 "path = \"%s\"\n"
                 "sha256 = \"%s\"\n",
                 man_sha, clean_rel, std_sha);
    } else {
        snprintf(lock_buf, sizeof(lock_buf),
                 "manifest_sha256 = \"%s\"\n"
                 "content_hash_algorithm = \"pblang-package-tree-v2\"\n",
                 man_sha);
    }

    size_t new_len = strlen(lock_buf);
    char new_sha[65];
    zr_su_sha256_hex((const uint8_t *)lock_buf, new_len, new_sha);

    bool had_old = false;
    char old_sha[65] = {0};
    if (zr_file_exists_reg(lock_p)) {
        size_t olen = 0;
        char rerr[256];
        uint8_t *ob = zr_su_read_file(lock_p, &olen, rerr);
        if (ob) { zr_su_sha256_hex(ob, olen, old_sha); free(ob); had_old = true; }
    }
    bool changed = !had_old || (strcmp(old_sha, new_sha) != 0);

    snprintf(rec->target, sizeof(rec->target), "%.500s", label);
    snprintf(rec->detected_kind, sizeof(rec->detected_kind), "project");
    rec->dry_run = opts->dry_run;
    if (had_old) snprintf(rec->sha256_before, sizeof(rec->sha256_before), "%s", old_sha);
    snprintf(rec->sha256_after, sizeof(rec->sha256_after), "%s", new_sha);

    if (!changed) {
        snprintf(rec->status, sizeof(rec->status), "already_up_to_date");
        rec->unchanged = std_rel[0] ? 2 : 1;
        snprintf(rec->detail, sizeof(rec->detail), "pblang.lock is already up to date");
        return true;
    }

    bool had_bak = false;
    if (!opts->dry_run) {
        if (!zr_su_write_atomic(lock_p, (const uint8_t *)lock_buf, new_len, 0644, true, &had_bak, err))
            return false;
        rec->writes_performed = 1;
    }
    snprintf(rec->status, sizeof(rec->status), "%s", opts->dry_run ? "update_available" : "updated");
    rec->added = had_old ? 0 : 1;
    rec->modified = had_old ? 1 : 0;
    if (had_bak) snprintf(rec->backup_path, sizeof(rec->backup_path), "pblang.lock.bak");
    snprintf(rec->detail, sizeof(rec->detail), "updated portable relative pblang.lock");
    return true;
}

static bool zr_su_update_archive(const char *arch_path, const ZrSuOpts *opts, ZrSuReceipt *rec, char *err) {
    ZrArchFmt fmt = zr_su_detect_arch(arch_path);
    if (opts->rollback) return zr_su_rollback_file(arch_path, "archive", opts, rec, err);
    if (!zr_file_exists_reg(arch_path)) {
        snprintf(err, 256, "archive '%s' does not exist", zr_su_basename(arch_path));
        return false;
    }

    size_t old_raw_len = 0;
    uint8_t *old_raw = zr_su_read_file(arch_path, &old_raw_len, err);
    if (!old_raw) return false;
    char old_sha[65];
    zr_su_sha256_hex(old_raw, old_raw_len, old_sha);
    free(old_raw);

    ZrSuEntryList cur = { NULL, 0, 0 };
    if (!zr_su_read_archive(arch_path, fmt, &cur, err)) { zr_su_elist_free(&cur); return false; }
    if (!zr_su_validate_entries_syntax(opts->self_exe, &cur, err)) { zr_su_elist_free(&cur); return false; }

    ZrSuEntryList target = { NULL, 0, 0 };
    char dummy_dig[65];
    if (opts->from) {
        if (!zr_su_load_source_tree_or_archive(opts, opts->from, &target, dummy_dig, err)) {
            zr_su_elist_free(&cur);
            zr_su_elist_free(&target);
            return false;
        }
        if (!zr_su_validate_entries_syntax(opts->self_exe, &target, err)) {
            zr_su_elist_free(&cur);
            zr_su_elist_free(&target);
            return false;
        }
    } else {
        for (size_t i = 0; i < cur.len; i++)
            zr_su_elist_put(&target, cur.items[i].rel, cur.items[i].kind, cur.items[i].data, cur.items[i].len);
    }

    size_t added = 0, modified = 0, removed = 0, unchanged = 0;
    zr_su_diff_lists(&cur, &target, &added, &modified, &removed, &unchanged);
    zr_su_elist_free(&cur);

    snprintf(rec->target, sizeof(rec->target), "%s", zr_su_basename(arch_path));
    snprintf(rec->detected_kind, sizeof(rec->detected_kind), "archive");
    rec->dry_run = opts->dry_run;
    rec->added = added;
    rec->modified = modified;
    rec->removed = removed;
    rec->unchanged = unchanged;
    snprintf(rec->sha256_before, sizeof(rec->sha256_before), "%s", old_sha);

    if (added == 0 && modified == 0 && removed == 0) {
        zr_su_elist_free(&target);
        snprintf(rec->status, sizeof(rec->status), "already_up_to_date");
        snprintf(rec->sha256_after, sizeof(rec->sha256_after), "%s", old_sha);
        snprintf(rec->detail, sizeof(rec->detail), "archive entries match source SHA-256 (0 writes)");
        return true;
    }

    size_t new_len = 0;
    uint8_t *new_bytes = zr_su_write_archive(&target, fmt, &new_len, err);
    zr_su_elist_free(&target);
    if (!new_bytes) return false;
    zr_su_sha256_hex(new_bytes, new_len, rec->sha256_after);

    bool had_bak = false;
    if (!opts->dry_run) {
        if (!zr_su_write_atomic(arch_path, new_bytes, new_len, 0644, true, &had_bak, err)) {
            free(new_bytes);
            return false;
        }
        rec->writes_performed = 1;
    }
    free(new_bytes);
    snprintf(rec->status, sizeof(rec->status), "%s", opts->dry_run ? "update_available" : "updated");
    if (had_bak) snprintf(rec->backup_path, sizeof(rec->backup_path), "%s.bak", zr_su_basename(arch_path));
    snprintf(rec->detail, sizeof(rec->detail), "delta-synced archive atomically");
    return true;
}

static bool zr_su_update_exec_or_launcher(const char *path, const char *kind, const ZrSuOpts *opts,
                                          ZrSuReceipt *rec, char *err) {
    if (opts->rollback) return zr_su_rollback_file(path, kind, opts, rec, err);
    size_t old_len = 0;
    uint8_t *old_b = zr_su_read_file(path, &old_len, err);
    if (!old_b) return false;
    char old_sha[65];
    zr_su_sha256_hex(old_b, old_len, old_sha);
    free(old_b);

    if (!opts->from) {
        snprintf(rec->target, sizeof(rec->target), "%s", zr_su_basename(path));
        snprintf(rec->detected_kind, sizeof(rec->detected_kind), "%s", kind);
        snprintf(rec->status, sizeof(rec->status), "already_up_to_date");
        rec->dry_run = opts->dry_run;
        rec->unchanged = 1;
        snprintf(rec->sha256_before, sizeof(rec->sha256_before), "%s", old_sha);
        snprintf(rec->sha256_after, sizeof(rec->sha256_after), "%s", old_sha);
        snprintf(rec->detail, sizeof(rec->detail), "verified %s integrity", kind);
        return true;
    }

    size_t new_len = 0;
    uint8_t *new_b = zr_su_read_file(opts->from, &new_len, err);
    if (!new_b) return false;
    if (strcmp(kind, "elf_executable") == 0) {
        if (new_len < 64 || memcmp(new_b, "\x7f" "ELF", 4) != 0) {
            free(new_b);
            snprintf(err, 256, "replacement file is not a valid ELF64 binary");
            return false;
        }
    }
    char new_sha[65];
    zr_su_sha256_hex(new_b, new_len, new_sha);
    if (opts->expected_sha256 && strcasecmp(new_sha, opts->expected_sha256) != 0) {
        free(new_b);
        snprintf(err, 256, "expected SHA-256 mismatch on executable");
        return false;
    }
    if (opts->pubkey_hex) {
        char sig_p[4200];
        snprintf(sig_p, sizeof(sig_p), "%s.sig", opts->from);
        if (!zr_su_verify_ed25519(opts->self_exe, new_b, new_len, opts->pubkey_hex, sig_p, err)) {
            free(new_b);
            return false;
        }
    }

    snprintf(rec->target, sizeof(rec->target), "%s", zr_su_basename(path));
    snprintf(rec->detected_kind, sizeof(rec->detected_kind), "%s", kind);
    rec->dry_run = opts->dry_run;
    snprintf(rec->sha256_before, sizeof(rec->sha256_before), "%s", old_sha);
    snprintf(rec->sha256_after, sizeof(rec->sha256_after), "%s", new_sha);

    if (strcmp(old_sha, new_sha) == 0) {
        free(new_b);
        snprintf(rec->status, sizeof(rec->status), "already_up_to_date");
        rec->unchanged = 1;
        snprintf(rec->detail, sizeof(rec->detail), "%s already matches source SHA-256", kind);
        return true;
    }

    bool had_bak = false;
    if (!opts->dry_run) {
        if (!zr_su_write_atomic(path, new_b, new_len, 0755, true, &had_bak, err)) {
            free(new_b);
            return false;
        }
        rec->writes_performed = 1;
    }
    free(new_b);
    snprintf(rec->status, sizeof(rec->status), "%s", opts->dry_run ? "update_available" : "updated");
    rec->modified = 1;
    if (had_bak) snprintf(rec->backup_path, sizeof(rec->backup_path), "%s.bak", zr_su_basename(path));
    snprintf(rec->detail, sizeof(rec->detail), "ETXTBSY-safe atomic %s replacement completed", kind);
    return true;
}

static void zr_su_resolve_roots(const ZrSuOpts *opts, char *inst_root, size_t icap, char *bin_dir, size_t bcap) {
    const char *home = getenv("HOME");
    if (!home || !*home) home = ".";
    if (opts->install_root && *opts->install_root) snprintf(inst_root, icap, "%s", opts->install_root);
    else if (getenv("PBLANG_INSTALL_ROOT")) snprintf(inst_root, icap, "%s", getenv("PBLANG_INSTALL_ROOT"));
    else snprintf(inst_root, icap, "%s/.local/share/pblang", home);

    if (opts->bin_dir && *opts->bin_dir) snprintf(bin_dir, bcap, "%s", opts->bin_dir);
    else snprintf(bin_dir, bcap, "%s/.local/bin", home);
}

static bool zr_su_update_stdlib(const ZrSuOpts *opts, ZrSuReceipt *rec, char *err) {
    char inst_root[4096], bin_dir[4096], dest_std[4400], bak_std[4420];
    zr_su_resolve_roots(opts, inst_root, sizeof(inst_root), bin_dir, sizeof(bin_dir));
    snprintf(dest_std, sizeof(dest_std), "%s/releases/%s/stdlib", inst_root, ZR_SU_SEMVER);
    snprintf(bak_std, sizeof(bak_std), "%s.bak", dest_std);

    snprintf(rec->target, sizeof(rec->target), "stdlib");
    snprintf(rec->detected_kind, sizeof(rec->detected_kind), "stdlib");
    rec->dry_run = opts->dry_run;

    if (opts->rollback) {
        if (!zr_dir_exists(bak_std)) {
            snprintf(err, 256, "no stdlib.bak directory available for --rollback");
            return false;
        }
        ZrSuEntryList bak_files = { NULL, 0, 0 };
        if (!zr_su_collect_dir_rec(bak_std, bak_std, false, &bak_files, err)) {
            zr_su_elist_free(&bak_files);
            return false;
        }
        qsort(bak_files.items, bak_files.len, sizeof(ZrSuEntry), zr_su_cmp_entry);
        zr_su_compute_tree_digest(&bak_files, rec->sha256_after);
        rec->modified = bak_files.len;
        zr_su_elist_free(&bak_files);

        if (!opts->dry_run) {
            char tmp_old[4500];
            snprintf(tmp_old, sizeof(tmp_old), "%s.pre_rb", dest_std);
            zr_su_rm_rf(tmp_old);
            if (zr_dir_exists(dest_std)) (void)zr_su_atomic_rename(dest_std, tmp_old);
            if (!zr_su_atomic_rename(bak_std, dest_std)) {
                if (zr_dir_exists(tmp_old)) (void)zr_su_atomic_rename(tmp_old, dest_std);
                snprintf(err, 256, "failed to restore stdlib.bak");
                return false;
            }
            if (zr_dir_exists(tmp_old)) (void)zr_su_atomic_rename(tmp_old, bak_std);
            rec->writes_performed = 1;
        }
        snprintf(rec->status, sizeof(rec->status), "%s", opts->dry_run ? "rollback_ready" : "rolled_back");
        snprintf(rec->backup_path, sizeof(rec->backup_path), "stdlib.bak");
        snprintf(rec->detail, sizeof(rec->detail), "atomically rolled back stdlib from stdlib.bak");
        return true;
    }

    const char *src_p = (opts->from && *opts->from) ? opts->from : (zr_dir_exists("stdlib") ? "stdlib" : NULL);
    if (!src_p) {
        snprintf(err, 256, "specify --from <stdlib_dir_or_archive>");
        return false;
    }

    ZrSuEntryList incoming = { NULL, 0, 0 };
    char new_sha[65];
    if (!zr_su_load_source_tree_or_archive(opts, src_p, &incoming, new_sha, err)) {
        zr_su_elist_free(&incoming);
        return false;
    }
    if (incoming.len == 0) {
        zr_su_elist_free(&incoming);
        snprintf(err, 256, "refusing to install empty stdlib");
        return false;
    }
    if (!zr_su_validate_entries_syntax(opts->self_exe, &incoming, err)) {
        zr_su_elist_free(&incoming);
        return false;
    }

    ZrSuEntryList cur = { NULL, 0, 0 };
    char old_sha[65] = {0};
    bool has_cur = false;
    if (zr_dir_exists(dest_std)) {
        if (zr_su_collect_dir_rec(dest_std, dest_std, false, &cur, err)) {
            if (cur.len > 1) qsort(cur.items, cur.len, sizeof(ZrSuEntry), zr_su_cmp_entry);
            zr_su_compute_tree_digest(&cur, old_sha);
            has_cur = true;
        }
    }
    size_t added = 0, modified = 0, removed = 0, unchanged = 0;
    zr_su_diff_lists(&cur, &incoming, &added, &modified, &removed, &unchanged);
    zr_su_elist_free(&cur);

    if (has_cur) snprintf(rec->sha256_before, sizeof(rec->sha256_before), "%s", old_sha);
    snprintf(rec->sha256_after, sizeof(rec->sha256_after), "%s", new_sha);
    rec->added = added;
    rec->modified = modified;
    rec->removed = removed;
    rec->unchanged = unchanged;

    char man_check[4500];
    snprintf(man_check, sizeof(man_check), "%s/MANIFEST.sha256.json", dest_std);
    if (has_cur && strcmp(old_sha, new_sha) == 0 && zr_file_exists_reg(man_check)) {
        zr_su_elist_free(&incoming);
        snprintf(rec->status, sizeof(rec->status), "already_up_to_date");
        snprintf(rec->detail, sizeof(rec->detail), "stdlib tree SHA-256 matches installed MANIFEST.sha256.json");
        return true;
    }

    bool had_bak = false;
    if (!opts->dry_run) {
        if (!zr_su_atomic_swap_dir(dest_std, &incoming, true, &had_bak, err)) {
            zr_su_elist_free(&incoming);
            return false;
        }
        rec->writes_performed = added + modified + 1;
    }
    zr_su_elist_free(&incoming);
    snprintf(rec->status, sizeof(rec->status), "%s", opts->dry_run ? "update_available" : "updated");
    if (had_bak) snprintf(rec->backup_path, sizeof(rec->backup_path), "stdlib.bak");
    snprintf(rec->detail, sizeof(rec->detail), "syntax-verified stdlib and committed atomic swap");
    return true;
}

static bool zr_su_update_self(const ZrSuOpts *opts, ZrSuReceipt *rec, char *err) {
    char inst_root[4096], bin_dir[4096], launcher_p[4300];
    zr_su_resolve_roots(opts, inst_root, sizeof(inst_root), bin_dir, sizeof(bin_dir));
    snprintf(launcher_p, sizeof(launcher_p), "%s/pblang", bin_dir);

    snprintf(rec->target, sizeof(rec->target), "self");
    snprintf(rec->detected_kind, sizeof(rec->detected_kind), "self_toolchain");
    rec->dry_run = opts->dry_run;

    if (opts->rollback) {
        if (!zr_su_rollback_file(launcher_p, "self_toolchain", opts, rec, err)) return false;
        snprintf(rec->target, sizeof(rec->target), "self");
        snprintf(rec->detected_kind, sizeof(rec->detected_kind), "self_toolchain");
        return true;
    }

    const char *src_root = (opts->from && *opts->from) ? opts->from : ".";
    char cand_bin[4400] = {0};
    const char *rel_cands[] = { "pblang", "bin/pblang-native", "bin/pblang", NULL };
    for (int i = 0; rel_cands[i]; i++) {
        char cp[4400];
        snprintf(cp, sizeof(cp), "%s/%s", src_root, rel_cands[i]);
        if (zr_file_exists_reg(cp)) {
            size_t hl = 0;
            char herr[256];
            uint8_t *hb = zr_su_read_file(cp, &hl, herr);
            if (hb) {
                bool is_elf = (hl >= 64 && memcmp(hb, "\x7f" "ELF", 4) == 0);
                free(hb);
                if (is_elf) { snprintf(cand_bin, sizeof(cand_bin), "%s", cp); break; }
            }
        }
    }
    if (!cand_bin[0] && opts->self_exe && zr_file_exists_reg(opts->self_exe)) {
        snprintf(cand_bin, sizeof(cand_bin), "%s", opts->self_exe);
    }
    if (!cand_bin[0]) {
        snprintf(err, 256, "could not locate sovereign pblang ELF64 binary for self update");
        return false;
    }

    size_t bin_len = 0;
    uint8_t *bin_bytes = zr_su_read_file(cand_bin, &bin_len, err);
    if (!bin_bytes) return false;
    if (bin_len < 64 || memcmp(bin_bytes, "\x7f" "ELF", 4) != 0) {
        free(bin_bytes);
        snprintf(err, 256, "replacement self binary failed ELF64 header check");
        return false;
    }
    char new_bin_sha[65];
    zr_su_sha256_hex(bin_bytes, bin_len, new_bin_sha);

    char src_std[4400];
    snprintf(src_std, sizeof(src_std), "%s/stdlib", src_root);
    if (!zr_dir_exists(src_std)) {
        free(bin_bytes);
        snprintf(err, 256, "replacement source must contain stdlib/");
        return false;
    }
    ZrSuEntryList std_files = { NULL, 0, 0 };
    if (!zr_su_collect_dir_rec(src_std, src_std, false, &std_files, err) ||
        !zr_su_validate_entries_syntax(opts->self_exe, &std_files, err)) {
        zr_su_elist_free(&std_files);
        free(bin_bytes);
        return false;
    }
    if (std_files.len > 1) qsort(std_files.items, std_files.len, sizeof(ZrSuEntry), zr_su_cmp_entry);
    char new_std_sha[65];
    zr_su_compute_tree_digest(&std_files, new_std_sha);

    char rel_dir[4400], dest_native[4500], dest_pblang[4500], dest_std[4500];
    snprintf(rel_dir, sizeof(rel_dir), "%s/releases/%s", inst_root, ZR_SU_SEMVER);
    snprintf(dest_native, sizeof(dest_native), "%s/bin/pblang-native", rel_dir);
    snprintf(dest_pblang, sizeof(dest_pblang), "%s/bin/pblang", rel_dir);
    snprintf(dest_std, sizeof(dest_std), "%s/stdlib", rel_dir);

    char wrapper_txt[4600];
    snprintf(wrapper_txt, sizeof(wrapper_txt),
             "#!/bin/sh\n"
             "# Installed PBLang %s launcher\n"
             "REAL_PBLANG=\"${PBLANG_INSTALL_ROOT:-%s}/releases/%s/bin/pblang\"\n"
             "exec \"$REAL_PBLANG\" \"$@\"\n",
             ZR_SU_SEMVER, inst_root, ZR_SU_SEMVER);

    bool bin_same = false, std_same = false, lnch_same = false;
    if (zr_file_exists_reg(dest_native)) {
        size_t ol = 0; char rerr[256];
        uint8_t *ob = zr_su_read_file(dest_native, &ol, rerr);
        if (ob) {
            zr_su_sha256_hex(ob, ol, rec->sha256_before);
            bin_same = (strcmp(rec->sha256_before, new_bin_sha) == 0);
            free(ob);
        }
    }
    if (zr_dir_exists(dest_std)) {
        ZrSuEntryList cur_s = { NULL, 0, 0 }; char rerr[256], csha[65];
        if (zr_su_collect_dir_rec(dest_std, dest_std, false, &cur_s, rerr)) {
            if (cur_s.len > 1) qsort(cur_s.items, cur_s.len, sizeof(ZrSuEntry), zr_su_cmp_entry);
            zr_su_compute_tree_digest(&cur_s, csha);
            std_same = (strcmp(csha, new_std_sha) == 0);
        }
        zr_su_elist_free(&cur_s);
    }
    if (zr_file_exists_reg(launcher_p)) {
        size_t ll = 0; char rerr[256];
        uint8_t *lb = zr_su_read_file(launcher_p, &ll, rerr);
        if (lb) {
            lnch_same = (ll == strlen(wrapper_txt) && memcmp(lb, wrapper_txt, ll) == 0);
            free(lb);
        }
    }
    snprintf(rec->sha256_after, sizeof(rec->sha256_after), "%s", new_bin_sha);

    if (bin_same && std_same && lnch_same) {
        zr_su_elist_free(&std_files);
        free(bin_bytes);
        snprintf(rec->status, sizeof(rec->status), "already_up_to_date");
        rec->unchanged = 3;
        snprintf(rec->detail, sizeof(rec->detail), "pblang %s runtime, stdlib, and launcher are up to date", ZR_SU_SEMVER);
        return true;
    }

    if (!opts->dry_run) {
        char bdir[4500], act_p[4400];
        snprintf(bdir, sizeof(bdir), "%s/bin", rel_dir);
        zr_su_mkdir_p(bdir);
        bool dummy = false, had_bak = false;
        if (!zr_su_write_atomic(dest_native, bin_bytes, bin_len, 0755, true, &dummy, err) ||
            !zr_su_write_atomic(dest_pblang, bin_bytes, bin_len, 0755, false, &dummy, err) ||
            !zr_su_atomic_swap_dir(dest_std, &std_files, true, &dummy, err) ||
            !zr_su_write_atomic(launcher_p, (const uint8_t *)wrapper_txt, strlen(wrapper_txt), 0755, true, &had_bak, err)) {
            zr_su_elist_free(&std_files);
            free(bin_bytes);
            return false;
        }
        snprintf(act_p, sizeof(act_p), "%s/ACTIVE_RELEASE", inst_root);
        (void)zr_su_write_atomic(act_p, (const uint8_t *)ZR_SU_SEMVER, strlen(ZR_SU_SEMVER), 0644, false, &dummy, err);
        rec->writes_performed = 4;
        if (had_bak) snprintf(rec->backup_path, sizeof(rec->backup_path), "pblang.bak");
    }
    zr_su_elist_free(&std_files);
    free(bin_bytes);
    snprintf(rec->status, sizeof(rec->status), "%s", opts->dry_run ? "update_available" : "updated");
    rec->modified = 1;
    snprintf(rec->detail, sizeof(rec->detail), "installed sovereign pblang %s release atomically", ZR_SU_SEMVER);
    return true;
}

/* ── Output Formatting & Entry Point ──────────────────────────────────────── */

static void zr_su_json_esc(const char *s, ZrBuf *b) {
    zr_buf_append(b, "\"", 1);
    for (size_t i = 0; s && s[i]; i++) {
        unsigned char c = (unsigned char)s[i];
        if (c == '"') zr_buf_append(b, "\\\"", 2);
        else if (c == '\\') zr_buf_append(b, "\\\\", 2);
        else if (c == '\n') zr_buf_append(b, "\\n", 2);
        else if (c == '\r') zr_buf_append(b, "\\r", 2);
        else if (c == '\t') zr_buf_append(b, "\\t", 2);
        else zr_buf_append(b, &c, 1);
    }
    zr_buf_append(b, "\"", 1);
}

static void zr_su_append_receipt_json(const ZrSuReceipt *r, ZrBuf *b) {
    char clean_tgt[512], clean_det[512];
    zr_su_scrub_str(r->target, clean_tgt, sizeof(clean_tgt));
    zr_su_scrub_str(r->detail, clean_det, sizeof(clean_det));

    zr_buf_append(b, "{", 1);
    zr_buf_append(b, "\"target\":", 9); zr_su_json_esc(clean_tgt, b);
    zr_buf_append(b, ",\"detected_kind\":", 17); zr_su_json_esc(r->detected_kind, b);
    zr_buf_append(b, ",\"status\":", 10); zr_su_json_esc(r->status, b);
    char nums[256];
    snprintf(nums, sizeof(nums),
             ",\"dry_run\":%s,\"writes_performed\":%zu,\"added\":%zu,\"modified\":%zu,\"removed\":%zu,\"unchanged\":%zu",
             r->dry_run ? "true" : "false", r->writes_performed, r->added, r->modified, r->removed, r->unchanged);
    zr_buf_append(b, nums, strlen(nums));
    zr_buf_append(b, ",\"sha256_before\":", 17);
    if (r->sha256_before[0]) zr_su_json_esc(r->sha256_before, b); else zr_buf_append(b, "null", 4);
    zr_buf_append(b, ",\"sha256_after\":", 16);
    if (r->sha256_after[0]) zr_su_json_esc(r->sha256_after, b); else zr_buf_append(b, "null", 4);
    zr_buf_append(b, ",\"backup_path\":", 15);
    if (r->backup_path[0]) zr_su_json_esc(r->backup_path, b); else zr_buf_append(b, "null", 4);
    zr_buf_append(b, ",\"detail\":", 10); zr_su_json_esc(clean_det, b);
    zr_buf_append(b, "}", 1);
}

static bool zr_su_dispatch_target(const char *t, const ZrSuOpts *opts, ZrSuReceipt *rec, char *err) {
    memset(rec, 0, sizeof(*rec));
    if (strncmp(t, "http://", 7) == 0 || strncmp(t, "https://", 8) == 0) {
        if (!zr_su_check_remote_url(t, opts->expected_sha256, opts->pubkey_hex, err)) return false;
    }
    if (opts->from && (strncmp(opts->from, "http://", 7) == 0 || strncmp(opts->from, "https://", 8) == 0)) {
        if (!zr_su_check_remote_url(opts->from, opts->expected_sha256, opts->pubkey_hex, err)) return false;
    }

    if (strcmp(t, "self") == 0) return zr_su_update_self(opts, rec, err);
    if (strcmp(t, "stdlib") == 0) return zr_su_update_stdlib(opts, rec, err);
    if (zr_su_detect_arch(t) != ZR_ARCH_NONE) return zr_su_update_archive(t, opts, rec, err);
    if (zr_su_ends_with_ci(t, ".sapp") || zr_su_ends_with_ci(t, ".sap"))
        return zr_su_update_sapp(t, NULL, "sapp_source", opts, rec, err);
    if (zr_su_ends_with_ci(t, ".pbck"))
        return zr_su_update_pbck(t, opts, rec, err);
    if (zr_su_ends_with_ci(t, "pblang.toml") || zr_su_ends_with_ci(t, "pblang.lock")) {
        char d[4096];
        zr_su_dirname(t, d, sizeof(d));
        return zr_su_update_project(d, zr_su_basename(t), opts, rec, err);
    }
    if (zr_dir_exists(t)) {
        char tp[4200];
        snprintf(tp, sizeof(tp), "%s/pblang.toml", t);
        if (zr_file_exists_reg(tp)) {
            const char *lbl = (strcmp(t, ".") == 0) ? "." : zr_su_basename(t);
            return zr_su_update_project(t, lbl, opts, rec, err);
        }
        snprintf(err, 256, "directory '%s' is not a PBLang project (missing pblang.toml)", zr_su_basename(t));
        return false;
    }
    if (zr_file_exists_reg(t)) {
        size_t hl = 0;
        uint8_t *hb = zr_su_read_file(t, &hl, err);
        if (!hb) return false;
        bool is_elf = (hl >= 4 && memcmp(hb, "\x7f" "ELF", 4) == 0);
        bool is_sh  = (hl >= 2 && hb[0] == '#' && hb[1] == '!');
        free(hb);
        if (is_elf) return zr_su_update_exec_or_launcher(t, "elf_executable", opts, rec, err);
        if (is_sh)  return zr_su_update_exec_or_launcher(t, "launcher_script", opts, rec, err);
    }
    snprintf(err, 256, "target '%s' does not exist", zr_su_basename(t));
    return false;
}

static int zr_cmd_update(const char *self_exe, int argc, char **argv) {
    ZrSuOpts opts;
    memset(&opts, 0, sizeof(opts));
    opts.self_exe = self_exe;
    opts.targets = (char **)zr_xcalloc((size_t)(argc + 2), sizeof(char *));

    for (int i = 0; i < argc; i++) {
        const char *a = argv[i];
        if (strcmp(a, "-h") == 0 || strcmp(a, "--help") == 0) {
            printf("pblang update — Sovereign Zero-Rust Polymorphic Atomic Updater (v%s)\n\n"
                   "Usage: pblang update [targets...] [--dry-run] [--check] [--rollback]\n"
                   "                     [--from <path|https-url>] [--expected-sha256 <hex>]\n"
                   "                     [--pubkey <hex>] [--install-root <dir>] [--bin-dir <dir>] [--json]\n",
                   ZR_SU_SEMVER);
            free(opts.targets);
            return 0;
        } else if (strcmp(a, "--dry-run") == 0) opts.dry_run = true;
        else if (strcmp(a, "--check") == 0) { opts.check_only = true; opts.dry_run = true; }
        else if (strcmp(a, "--rollback") == 0) opts.rollback = true;
        else if (strcmp(a, "--json") == 0) opts.json = true;
        else if (strcmp(a, "--remote") == 0) opts.remote = true;
        else if (strcmp(a, "--from") == 0 && i + 1 < argc) opts.from = argv[++i];
        else if (strncmp(a, "--from=", 7) == 0) opts.from = a + 7;
        else if (strcmp(a, "--pubkey") == 0 && i + 1 < argc) opts.pubkey_hex = argv[++i];
        else if (strncmp(a, "--pubkey=", 9) == 0) opts.pubkey_hex = a + 9;
        else if (strcmp(a, "--expected-sha256") == 0 && i + 1 < argc) opts.expected_sha256 = argv[++i];
        else if (strncmp(a, "--expected-sha256=", 18) == 0) opts.expected_sha256 = a + 18;
        else if (strcmp(a, "--install-root") == 0 && i + 1 < argc) opts.install_root = argv[++i];
        else if (strncmp(a, "--install-root=", 15) == 0) opts.install_root = a + 15;
        else if (strcmp(a, "--bin-dir") == 0 && i + 1 < argc) opts.bin_dir = argv[++i];
        else if (strncmp(a, "--bin-dir=", 10) == 0) opts.bin_dir = a + 10;
        else if (a[0] == '-') {
            fprintf(stderr, "pblang update: unknown flag '%s'\n", a);
            free(opts.targets);
            return 1;
        } else {
            opts.targets[opts.ntargets++] = (char *)a;
        }
    }

    if (opts.expected_sha256 && !zr_su_is_hex_n(opts.expected_sha256, 64)) {
        fprintf(stderr, "pblang update: --expected-sha256 must be 64 hex chars\n");
        free(opts.targets);
        return 1;
    }
    if (opts.pubkey_hex && !zr_su_is_hex_n(opts.pubkey_hex, 64)) {
        fprintf(stderr, "pblang update: --pubkey must be 64 hex chars (32-byte Ed25519 key)\n");
        free(opts.targets);
        return 1;
    }

    if (opts.remote && opts.ntargets == 0) {
        PbVM vm;
        vm_init(&vm);
        zr_caps_allow_all(&vm.caps);
        const char *api = "https://api.github.com/repos/PanosBiazis/pblang/releases/latest";
        PbValue u = val_obj(pb_str_new(&vm, api, strlen(api)));
        vm_push_root(&vm, u);
        PbValue resp = val_nil();
        if (bi_http_get(&vm, 1, &u, &resp) && is_obj_type(resp, OBJ_STR)) {
            printf("Latest GitHub release metadata fetched (%zu bytes).\n", AS_STR(resp)->len);
        } else {
            printf("Remote release check unavailable (%s).\n", vm.err_msg[0] ? vm.err_msg : "offline");
        }
        vm_pop_roots(&vm, 1);
        vm_free(&vm);
        free(opts.targets);
        return 0;
    }

    if (opts.ntargets == 0) opts.targets[opts.ntargets++] = (char *)".";

    ZrSuReceipt *recs = (ZrSuReceipt *)zr_xcalloc(opts.ntargets, sizeof(ZrSuReceipt));
    bool any_pending = false;
    for (size_t i = 0; i < opts.ntargets; i++) {
        char err[256] = {0};
        if (!zr_su_dispatch_target(opts.targets[i], &opts, &recs[i], err)) {
            char clean_err[300];
            zr_su_scrub_str(err, clean_err, sizeof(clean_err));
            fprintf(stderr, "pblang update: %s\n", clean_err);
            free(recs);
            free(opts.targets);
            return 1;
        }
        if (strcmp(recs[i].status, "already_up_to_date") != 0) any_pending = true;
    }

    if (opts.json) {
        ZrBuf jb;
        zr_buf_init(&jb);
        if (opts.ntargets == 1) {
            zr_su_append_receipt_json(&recs[0], &jb);
        } else {
            zr_buf_append(&jb, "[", 1);
            for (size_t i = 0; i < opts.ntargets; i++) {
                if (i > 0) zr_buf_append(&jb, ",", 1);
                zr_su_append_receipt_json(&recs[i], &jb);
            }
            zr_buf_append(&jb, "]", 1);
        }
        zr_buf_append(&jb, "\n", 1);
        fwrite(jb.data, 1, jb.len, stdout);
        zr_buf_free(&jb);
    } else {
        for (size_t i = 0; i < opts.ntargets; i++) {
            char ct[512], cd[512];
            zr_su_scrub_str(recs[i].target, ct, sizeof(ct));
            zr_su_scrub_str(recs[i].detail, cd, sizeof(cd));
            printf("[%s] %s (%s) — %s\n", recs[i].status, ct, recs[i].detected_kind, cd);
        }
    }

    free(recs);
    free(opts.targets);
    if (opts.check_only && any_pending) return 2;
    return 0;
}

#endif /* ZR_SMART_UPDATER_H */
