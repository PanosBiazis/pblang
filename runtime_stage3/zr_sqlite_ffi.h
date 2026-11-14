/* ============================================================================
 * pblang Sovereign Zero-Rust Native Runtime v3.0.0 — zr_sqlite_ffi.h
 * Lazy-dlopen SQLite3 parameterized database engine + Unrestricted C ABI FFI.
 * ============================================================================ */
#ifndef PBLANG_ZR_SQLITE_FFI_H
#define PBLANG_ZR_SQLITE_FFI_H

#include "zr_vm_core.h"

/* ---- Lazy-loaded SQLite3 C API ---- */
typedef struct sqlite3      ZrSqlite3;
typedef struct sqlite3_stmt ZrSqliteStmt;

#define ZR_SQLITE_OK         0
#define ZR_SQLITE_ROW        100
#define ZR_SQLITE_DONE       101
#define ZR_SQLITE_INTEGER    1
#define ZR_SQLITE_FLOAT      2
#define ZR_SQLITE_TEXT       3
#define ZR_SQLITE_BLOB       4
#define ZR_SQLITE_NULL       5
#define ZR_SQLITE_TRANSIENT  ((void (*)(void *))(intptr_t)(-1))

typedef struct {
    bool       tried;
    bool       ready;
    ZrDlHandle lib;

    int         (*sqlite3_open)(const char *, ZrSqlite3 **);
    int         (*sqlite3_close)(ZrSqlite3 *);
    const char *(*sqlite3_errmsg)(ZrSqlite3 *);
    int         (*sqlite3_exec)(ZrSqlite3 *, const char *, void *, void *, char **);
    void        (*sqlite3_free)(void *);
    int         (*sqlite3_changes)(ZrSqlite3 *);
    int64_t     (*sqlite3_last_insert_rowid)(ZrSqlite3 *);
    int         (*sqlite3_prepare_v2)(ZrSqlite3 *, const char *, int, ZrSqliteStmt **, const char **);
    int         (*sqlite3_step)(ZrSqliteStmt *);
    int         (*sqlite3_finalize)(ZrSqliteStmt *);
    int         (*sqlite3_bind_null)(ZrSqliteStmt *, int);
    int         (*sqlite3_bind_int64)(ZrSqliteStmt *, int, int64_t);
    int         (*sqlite3_bind_double)(ZrSqliteStmt *, int, double);
    int         (*sqlite3_bind_text)(ZrSqliteStmt *, int, const char *, int, void (*)(void *));
    int         (*sqlite3_bind_blob)(ZrSqliteStmt *, int, const void *, int, void (*)(void *));
    int         (*sqlite3_column_count)(ZrSqliteStmt *);
    const char *(*sqlite3_column_name)(ZrSqliteStmt *, int);
    int         (*sqlite3_column_type)(ZrSqliteStmt *, int);
    int64_t     (*sqlite3_column_int64)(ZrSqliteStmt *, int);
    double      (*sqlite3_column_double)(ZrSqliteStmt *, int);
    const unsigned char *(*sqlite3_column_text)(ZrSqliteStmt *, int);
    const void *(*sqlite3_column_blob)(ZrSqliteStmt *, int);
    int         (*sqlite3_column_bytes)(ZrSqliteStmt *, int);
} ZrSqliteApi;

static ZrSqliteApi g_zr_sql = {0};
static ZrMutex     g_zr_db_ffi_mtx;
static bool        g_zr_db_ffi_mtx_init = false;

static void zr_db_ffi_ensure_mtx(void) {
    if (!g_zr_db_ffi_mtx_init) {
        zr_mutex_init(&g_zr_db_ffi_mtx);
        g_zr_db_ffi_mtx_init = true;
    }
}

static bool zr_ensure_sqlite(char err_out[256]) {
    zr_db_ffi_ensure_mtx();
    zr_mutex_lock(&g_zr_db_ffi_mtx);
    if (g_zr_sql.tried) {
        bool ok = g_zr_sql.ready;
        zr_mutex_unlock(&g_zr_db_ffi_mtx);
        if (!ok) snprintf(err_out, 256, "SQLite3 shared library (libsqlite3) not found on this host");
        return ok;
    }
    g_zr_sql.tried = true;

#if defined(_WIN32)
    const char *cands[] = { "winsqlite3.dll", "libsqlite3-0.dll", "sqlite3.dll", NULL };
#elif defined(__APPLE__)
    const char *cands[] = { "libsqlite3.dylib", "/usr/lib/libsqlite3.dylib", NULL };
#else
    const char *cands[] = {
        "libsqlite3.so.0",
        "/usr/lib/x86_64-linux-gnu/libsqlite3.so.0",
        "/lib/x86_64-linux-gnu/libsqlite3.so.0",
        "/usr/lib/aarch64-linux-gnu/libsqlite3.so.0",
        "libsqlite3.so",
        NULL
    };
#endif
    for (int i = 0; cands[i]; i++) {
        ZrDlHandle h = zr_dlopen(cands[i]);
        if (!h) continue;
        g_zr_sql.lib = h;
        #define LOAD_SQL_SYM(fn) g_zr_sql.fn = (__typeof__(g_zr_sql.fn))zr_dlsym(h, #fn)
        LOAD_SQL_SYM(sqlite3_open);
        LOAD_SQL_SYM(sqlite3_close);
        LOAD_SQL_SYM(sqlite3_errmsg);
        LOAD_SQL_SYM(sqlite3_exec);
        LOAD_SQL_SYM(sqlite3_free);
        LOAD_SQL_SYM(sqlite3_changes);
        LOAD_SQL_SYM(sqlite3_last_insert_rowid);
        LOAD_SQL_SYM(sqlite3_prepare_v2);
        LOAD_SQL_SYM(sqlite3_step);
        LOAD_SQL_SYM(sqlite3_finalize);
        LOAD_SQL_SYM(sqlite3_bind_null);
        LOAD_SQL_SYM(sqlite3_bind_int64);
        LOAD_SQL_SYM(sqlite3_bind_double);
        LOAD_SQL_SYM(sqlite3_bind_text);
        LOAD_SQL_SYM(sqlite3_bind_blob);
        LOAD_SQL_SYM(sqlite3_column_count);
        LOAD_SQL_SYM(sqlite3_column_name);
        LOAD_SQL_SYM(sqlite3_column_type);
        LOAD_SQL_SYM(sqlite3_column_int64);
        LOAD_SQL_SYM(sqlite3_column_double);
        LOAD_SQL_SYM(sqlite3_column_text);
        LOAD_SQL_SYM(sqlite3_column_blob);
        LOAD_SQL_SYM(sqlite3_column_bytes);
        #undef LOAD_SQL_SYM

        if (g_zr_sql.sqlite3_open && g_zr_sql.sqlite3_close && g_zr_sql.sqlite3_prepare_v2 &&
            g_zr_sql.sqlite3_step && g_zr_sql.sqlite3_finalize &&
            g_zr_sql.sqlite3_bind_null && g_zr_sql.sqlite3_bind_int64 &&
            g_zr_sql.sqlite3_bind_double && g_zr_sql.sqlite3_bind_text &&
            g_zr_sql.sqlite3_bind_blob && g_zr_sql.sqlite3_column_count &&
            g_zr_sql.sqlite3_column_name && g_zr_sql.sqlite3_column_type &&
            g_zr_sql.sqlite3_column_int64 && g_zr_sql.sqlite3_column_double &&
            g_zr_sql.sqlite3_column_text && g_zr_sql.sqlite3_column_blob &&
            g_zr_sql.sqlite3_column_bytes && g_zr_sql.sqlite3_last_insert_rowid) {
            g_zr_sql.ready = true;
            zr_mutex_unlock(&g_zr_db_ffi_mtx);
            return true;
        }
        zr_dlclose(h);
        memset(&g_zr_sql, 0, sizeof(g_zr_sql));
        g_zr_sql.tried = true;
    }

    zr_mutex_unlock(&g_zr_db_ffi_mtx);
    snprintf(err_out, 256, "SQLite3 shared library not found or missing required C symbols");
    return false;
}

typedef struct {
    size_t     id;
    bool       used;
    bool       busy;
    ZrSqlite3 *db;
} ZrDbEntry;

#define ZR_MAX_DB_HANDLES 512
static ZrDbEntry g_zr_db_table[ZR_MAX_DB_HANDLES];
static size_t    g_zr_db_next_id = 1;

static ZrDbEntry *zr_db_checkout(size_t id) {
    zr_db_ffi_ensure_mtx();
    zr_mutex_lock(&g_zr_db_ffi_mtx);
    for (size_t i = 0; i < ZR_MAX_DB_HANDLES; i++) {
        if (g_zr_db_table[i].used && g_zr_db_table[i].id == id && !g_zr_db_table[i].busy) {
            g_zr_db_table[i].busy = true;
            zr_mutex_unlock(&g_zr_db_ffi_mtx);
            return &g_zr_db_table[i];
        }
    }
    zr_mutex_unlock(&g_zr_db_ffi_mtx);
    return NULL;
}

static void zr_db_checkin(ZrDbEntry *e) {
    zr_mutex_lock(&g_zr_db_ffi_mtx);
    if (e) e->busy = false;
    zr_mutex_unlock(&g_zr_db_ffi_mtx);
}

static bool zr_sqlite_bind_params(PbVM *vm, ZrSqliteStmt *stmt, PbValue params_v, const char *fn_name) {
    if (params_v.tag == VAL_NIL) return true;
    if (!val_is_obj(params_v, OBJ_ARRAY)) {
        vm_raise(vm, "%s(): params must be array or nil", fn_name);
        return false;
    }
    PbArray *arr = (PbArray *)val_as_obj(params_v);
    for (size_t i = 0; i < arr->len; i++) {
        int idx = (int)(i + 1);
        PbValue v = arr->items[i];
        int rc = ZR_SQLITE_OK;
        if (v.tag == VAL_NIL) {
            rc = g_zr_sql.sqlite3_bind_null(stmt, idx);
        } else if (v.tag == VAL_BOOL) {
            rc = g_zr_sql.sqlite3_bind_int64(stmt, idx, v.u.b ? 1 : 0);
        } else if (v.tag == VAL_NUM) {
            double n = v.u.num;
            if (isfinite(n) && floor(n) == n && n >= -9007199254740991.0 && n <= 9007199254740991.0) {
                rc = g_zr_sql.sqlite3_bind_int64(stmt, idx, (int64_t)n);
            } else {
                rc = g_zr_sql.sqlite3_bind_double(stmt, idx, n);
            }
        } else if (val_is_obj(v, OBJ_STR)) {
            PbStr *s = (PbStr *)val_as_obj(v);
            rc = g_zr_sql.sqlite3_bind_text(stmt, idx, s->chars, (int)s->len, ZR_SQLITE_TRANSIENT);
        } else if (val_is_obj(v, OBJ_BIGINT) || val_is_obj(v, OBJ_RATIONAL) || val_is_obj(v, OBJ_DECIMAL)) {
            ZrBuf sb;
            zr_buf_init(&sb);
            vm_format_value(v, &sb);
            rc = g_zr_sql.sqlite3_bind_text(stmt, idx, (const char *)(sb.data ? sb.data : ""), (int)sb.len, ZR_SQLITE_TRANSIENT);
            zr_buf_free(&sb);
        } else if (val_is_obj(v, OBJ_BYTES)) {
            PbBytes *bb = (PbBytes *)val_as_obj(v);
            rc = g_zr_sql.sqlite3_bind_blob(stmt, idx, bb->data ? (const void *)bb->data : "", (int)bb->len, ZR_SQLITE_TRANSIENT);
        } else if (val_is_obj(v, OBJ_ARRAY)) {
            PbArray *ba = (PbArray *)val_as_obj(v);
            uint8_t *blob = (uint8_t *)zr_xmalloc(ba->len > 0 ? ba->len : 1);
            for (size_t k = 0; k < ba->len; k++) {
                PbValue bv = ba->items[k];
                if (bv.tag != VAL_NUM || !isfinite(bv.u.num) || floor(bv.u.num) != bv.u.num || bv.u.num < 0.0 || bv.u.num > 255.0) {
                    free(blob);
                    vm_raise(vm, "%s(): blob params must be array of byte numbers 0..255", fn_name);
                    return false;
                }
                blob[k] = (uint8_t)bv.u.num;
            }
            rc = g_zr_sql.sqlite3_bind_blob(stmt, idx, blob, (int)ba->len, ZR_SQLITE_TRANSIENT);
            free(blob);
        } else {
            vm_raise(vm, "%s(): unsupported parameter type '%s'", fn_name, vm_type_name(v));
            return false;
        }
        if (rc != ZR_SQLITE_OK) {
            vm_raise(vm, "%s(): sqlite3_bind failed at param %d (rc=%d)", fn_name, idx, rc);
            return false;
        }
    }
    return true;
}

static bool bi_sqlite_open(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (!vm_effective_cap(vm, CAP_SQLITE)) { vm_raise(vm, "sqlite access denied (capability 'sqlite' disabled)"); return false; }
    if (argc != 1 || !val_is_obj(argv[0], OBJ_STR)) { vm_raise(vm, "sqlite_open() expects 1 path string"); return false; }
    char err[256];
    if (!zr_ensure_sqlite(err)) { vm_raise(vm, "sqlite_open(): %s", err); return false; }

    ZrSqlite3 *db = NULL;
    int rc = g_zr_sql.sqlite3_open(((PbStr *)val_as_obj(argv[0]))->chars, &db);
    if (rc != ZR_SQLITE_OK || !db) {
        const char *emsg = (db && g_zr_sql.sqlite3_errmsg) ? g_zr_sql.sqlite3_errmsg(db) : "open failed";
        vm_raise(vm, "sqlite_open(): %s", emsg);
        if (db) g_zr_sql.sqlite3_close(db);
        return false;
    }

    zr_mutex_lock(&g_zr_db_ffi_mtx);
    size_t id = g_zr_db_next_id++;
    bool stored = false;
    for (size_t i = 0; i < ZR_MAX_DB_HANDLES; i++) {
        if (!g_zr_db_table[i].used) {
            g_zr_db_table[i].id = id;
            g_zr_db_table[i].used = true;
            g_zr_db_table[i].busy = false;
            g_zr_db_table[i].db = db;
            stored = true;
            break;
        }
    }
    zr_mutex_unlock(&g_zr_db_ffi_mtx);
    if (!stored) {
        g_zr_sql.sqlite3_close(db);
        vm_raise(vm, "sqlite_open(): handle table full");
        return false;
    }
    *out = val_num((double)id);
    return true;
}

static bool bi_sqlite_close(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (!vm_effective_cap(vm, CAP_SQLITE)) { vm_raise(vm, "sqlite access denied (capability 'sqlite' disabled)"); return false; }
    if (argc != 1 || argv[0].tag != VAL_NUM || argv[0].u.num < 1.0) {
        vm_raise(vm, "sqlite_close() expects 1 numeric handle");
        return false;
    }
    ZrDbEntry *e = zr_db_checkout((size_t)argv[0].u.num);
    if (e) {
        g_zr_sql.sqlite3_close(e->db);
        zr_mutex_lock(&g_zr_db_ffi_mtx);
        memset(e, 0, sizeof(*e));
        zr_mutex_unlock(&g_zr_db_ffi_mtx);
    }
    *out = val_nil();
    return true;
}

static bool bi_sqlite_exec(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (!vm_effective_cap(vm, CAP_SQLITE)) { vm_raise(vm, "sqlite access denied (capability 'sqlite' disabled)"); return false; }
    if (argc != 3 || argv[0].tag != VAL_NUM || !val_is_obj(argv[1], OBJ_STR)) {
        vm_raise(vm, "sqlite_exec(handle, sql, params) expects 3 arguments");
        return false;
    }
    ZrDbEntry *e = zr_db_checkout((size_t)argv[0].u.num);
    if (!e) { vm_raise(vm, "sqlite_exec(): invalid handle"); return false; }
    const char *sql = ((PbStr *)val_as_obj(argv[1]))->chars;

    bool empty_params = (argv[2].tag == VAL_NIL) ||
                        (val_is_obj(argv[2], OBJ_ARRAY) && ((PbArray *)val_as_obj(argv[2]))->len == 0);
    if (empty_params && g_zr_sql.sqlite3_exec) {
        char *errmsg = NULL;
        int rc = g_zr_sql.sqlite3_exec(e->db, sql, NULL, NULL, &errmsg);
        if (rc != ZR_SQLITE_OK) {
            char copy[256];
            snprintf(copy, sizeof(copy), "%s", errmsg ? errmsg : "SQL execution error");
            if (errmsg && g_zr_sql.sqlite3_free) g_zr_sql.sqlite3_free(errmsg);
            zr_db_checkin(e);
            vm_raise(vm, "sqlite_exec(): %s", copy);
            return false;
        }
        int changed = g_zr_sql.sqlite3_changes ? g_zr_sql.sqlite3_changes(e->db) : 0;
        zr_db_checkin(e);
        *out = val_num((double)changed);
        return true;
    }

    ZrSqliteStmt *stmt = NULL;
    if (g_zr_sql.sqlite3_prepare_v2(e->db, sql, -1, &stmt, NULL) != ZR_SQLITE_OK || !stmt) {
        const char *em = g_zr_sql.sqlite3_errmsg ? g_zr_sql.sqlite3_errmsg(e->db) : "prepare failed";
        char copy[256]; snprintf(copy, sizeof(copy), "%s", em);
        zr_db_checkin(e);
        vm_raise(vm, "sqlite_exec(): %s", copy);
        return false;
    }
    if (!zr_sqlite_bind_params(vm, stmt, argv[2], "sqlite_exec")) {
        g_zr_sql.sqlite3_finalize(stmt);
        zr_db_checkin(e);
        return false;
    }
    int rc = g_zr_sql.sqlite3_step(stmt);
    g_zr_sql.sqlite3_finalize(stmt);
    if (rc != ZR_SQLITE_DONE && rc != ZR_SQLITE_ROW) {
        const char *em = g_zr_sql.sqlite3_errmsg ? g_zr_sql.sqlite3_errmsg(e->db) : "step failed";
        char copy[256]; snprintf(copy, sizeof(copy), "%s", em);
        zr_db_checkin(e);
        vm_raise(vm, "sqlite_exec(): %s", copy);
        return false;
    }
    int changed = g_zr_sql.sqlite3_changes ? g_zr_sql.sqlite3_changes(e->db) : 0;
    zr_db_checkin(e);
    *out = val_num((double)changed);
    return true;
}

static bool bi_sqlite_query(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (!vm_effective_cap(vm, CAP_SQLITE)) { vm_raise(vm, "sqlite access denied (capability 'sqlite' disabled)"); return false; }
    if (argc != 3 || argv[0].tag != VAL_NUM || !val_is_obj(argv[1], OBJ_STR)) {
        vm_raise(vm, "sqlite_query(handle, sql, params) expects 3 arguments");
        return false;
    }
    ZrDbEntry *e = zr_db_checkout((size_t)argv[0].u.num);
    if (!e) { vm_raise(vm, "sqlite_query(): invalid handle"); return false; }
    const char *sql = ((PbStr *)val_as_obj(argv[1]))->chars;

    ZrSqliteStmt *stmt = NULL;
    if (g_zr_sql.sqlite3_prepare_v2(e->db, sql, -1, &stmt, NULL) != ZR_SQLITE_OK || !stmt) {
        const char *em = g_zr_sql.sqlite3_errmsg ? g_zr_sql.sqlite3_errmsg(e->db) : "prepare failed";
        char copy[256]; snprintf(copy, sizeof(copy), "%s", em);
        zr_db_checkin(e);
        vm_raise(vm, "sqlite_query(): %s", copy);
        return false;
    }
    if (!zr_sqlite_bind_params(vm, stmt, argv[2], "sqlite_query")) {
        g_zr_sql.sqlite3_finalize(stmt);
        zr_db_checkin(e);
        return false;
    }

    int ncols = g_zr_sql.sqlite3_column_count(stmt);
    PbArray *rows = vm_new_array(vm, 16);
    vm_temp_push(vm, val_obj(rows));

    int rc = ZR_SQLITE_OK;
    while ((rc = g_zr_sql.sqlite3_step(stmt)) == ZR_SQLITE_ROW) {
        PbMap *row = vm_new_map(vm, (size_t)(ncols > 0 ? ncols * 2 : 4));
        vm_temp_push(vm, val_obj(row));
        for (int c = 0; c < ncols; c++) {
            const char *cname = g_zr_sql.sqlite3_column_name(stmt, c);
            if (!cname) cname = "";
            PbStr *kstr = vm_new_cstr(vm, cname);
            vm_temp_push(vm, val_obj(kstr));

            int ctype = g_zr_sql.sqlite3_column_type(stmt, c);
            PbValue cv = val_nil();
            if (ctype == ZR_SQLITE_INTEGER) {
                cv = val_num((double)g_zr_sql.sqlite3_column_int64(stmt, c));
            } else if (ctype == ZR_SQLITE_FLOAT) {
                cv = val_num(g_zr_sql.sqlite3_column_double(stmt, c));
            } else if (ctype == ZR_SQLITE_TEXT) {
                const unsigned char *txt = g_zr_sql.sqlite3_column_text(stmt, c);
                int blen = g_zr_sql.sqlite3_column_bytes(stmt, c);
                cv = val_obj(vm_new_str(vm, txt ? (const char *)txt : "", blen > 0 ? (size_t)blen : 0));
            } else if (ctype == ZR_SQLITE_BLOB) {
                const uint8_t *blob = (const uint8_t *)g_zr_sql.sqlite3_column_blob(stmt, c);
                int blen = g_zr_sql.sqlite3_column_bytes(stmt, c);
                PbArray *ba = vm_new_array(vm, blen > 0 ? (size_t)blen : 0);
                for (int bi = 0; bi < blen; bi++) arr_push(ba, val_num((double)blob[bi]));
                cv = val_obj(ba);
            }
            map_set(row, kstr, cv);
            vm_temp_pop(vm, 1); /* kstr */
        }
        arr_push(rows, val_obj(row));
        vm_temp_pop(vm, 1); /* row */
    }

    g_zr_sql.sqlite3_finalize(stmt);
    if (rc != ZR_SQLITE_DONE) {
        const char *em = g_zr_sql.sqlite3_errmsg ? g_zr_sql.sqlite3_errmsg(e->db) : "query step failed";
        char copy[256]; snprintf(copy, sizeof(copy), "%s", em);
        zr_db_checkin(e);
        vm_temp_pop(vm, 1);
        vm_raise(vm, "sqlite_query(): %s", copy);
        return false;
    }
    zr_db_checkin(e);
    vm_temp_pop(vm, 1);
    *out = val_obj(rows);
    return true;
}

static bool bi_sqlite_last_insert_rowid(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (!vm_effective_cap(vm, CAP_SQLITE)) { vm_raise(vm, "sqlite access denied (capability 'sqlite' disabled)"); return false; }
    if (argc != 1 || argv[0].tag != VAL_NUM || argv[0].u.num < 1.0) {
        vm_raise(vm, "sqlite_last_insert_rowid() expects 1 numeric handle");
        return false;
    }
    ZrDbEntry *e = zr_db_checkout((size_t)argv[0].u.num);
    if (!e) { vm_raise(vm, "sqlite_last_insert_rowid(): invalid handle"); return false; }
    int64_t rid = g_zr_sql.sqlite3_last_insert_rowid(e->db);
    zr_db_checkin(e);
    *out = val_num((double)rid);
    return true;
}

/* ============================================================================
 * Unrestricted Guarded Dynamic C ABI FFI v3.0.0:
 *   - 10-arg mixed GP + FP (`SysV AMD64` & `Win64` inline asm + C99 fallback)
 *   - Zero-copy `OBJ_BYTES` & `OBJ_F64BUF` direct pointer marshaling
 *   - `ffi_struct_pack` / `ffi_struct_unpack` (Python-`struct`-compatible)
 *   - Raw pointer readers/writers (`ffi_bytes_ptr`, `ffi_ptr_read`, `ffi_ptr_write`,
 *     `ffi_read_i32`, `ffi_read_i64`, `ffi_read_f64`, `ffi_read_cstr`)
 *   - 8 static C callback trampolines (`ffi_callback_new`, `ffi_callback_free`)
 * ============================================================================ */

typedef struct {
    size_t     id;
    bool       used;
    bool       no_close;
    ZrDlHandle lib;
} ZrFfiEntry;

#define ZR_MAX_FFI_HANDLES 128
#define ZR_FFI_MAX_ARGS    10

static ZrFfiEntry g_zr_ffi_table[ZR_MAX_FFI_HANDLES];
static size_t     g_zr_ffi_next_id = 1;

static ZrDlHandle zr_ffi_lookup(size_t id) {
    zr_db_ffi_ensure_mtx();
    zr_mutex_lock(&g_zr_db_ffi_mtx);
    ZrDlHandle h = NULL;
    for (size_t i = 0; i < ZR_MAX_FFI_HANDLES; i++) {
        if (g_zr_ffi_table[i].used && g_zr_ffi_table[i].id == id) {
            h = g_zr_ffi_table[i].lib;
            break;
        }
    }
    zr_mutex_unlock(&g_zr_db_ffi_mtx);
    return h;
}

static bool bi_ffi_load(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (!vm_check_cap(vm, vm_effective_cap(vm, CAP_FFI), "ffi_load", "ffi")) return false;
    if (argc != 1 || !val_is_obj(argv[0], OBJ_STR)) { vm_raise(vm, "ffi_load() expects 1 library path string"); return false; }
    const char *path = ((PbStr *)val_as_obj(argv[0]))->chars;
    if (!vm->caps.ffi_allow_all) {
        vm_raise(vm, "ffi_load: library \"%s\" not in allowlist (pass --allow-ffi / --allow-all)", path);
        return false;
    }
    ZrDlHandle lib = NULL;
    bool no_close = false;
    if (path[0] == '\0' || strcmp(path, "self") == 0) {
#if defined(_WIN32)
        lib = zr_dlopen("msvcrt.dll");
#else
        lib = zr_dlopen(NULL);
        no_close = true;
#endif
    } else {
        lib = zr_dlopen(path);
        if (!lib) {
#if defined(_WIN32)
            if (strcmp(path, "libc") == 0 || strcmp(path, "libc.so") == 0 || strcmp(path, "libc.so.6") == 0 ||
                strcmp(path, "libm") == 0 || strcmp(path, "libm.so") == 0 || strcmp(path, "libm.so.6") == 0) {
                lib = zr_dlopen("msvcrt.dll");
            }
#elif defined(__APPLE__)
            if (strcmp(path, "libc") == 0 || strcmp(path, "libc.so") == 0 || strcmp(path, "libc.so.6") == 0 ||
                strcmp(path, "libm") == 0 || strcmp(path, "libm.so") == 0 || strcmp(path, "libm.so.6") == 0) {
                lib = zr_dlopen("libSystem.B.dylib");
            }
#else
            if (strcmp(path, "libc") == 0 || strcmp(path, "libc.so") == 0) {
                lib = zr_dlopen("libc.so.6");
                if (!lib) { lib = zr_dlopen(NULL); no_close = true; }
            } else if (strcmp(path, "libm") == 0 || strcmp(path, "libm.so") == 0) {
                lib = zr_dlopen("libm.so.6");
            }
#endif
        }
    }
    if (!lib) {
        vm_raise(vm, "ffi_load(): failed to load library \"%s\"", path);
        return false;
    }
    zr_db_ffi_ensure_mtx();
    zr_mutex_lock(&g_zr_db_ffi_mtx);
    size_t id = g_zr_ffi_next_id++;
    bool stored = false;
    for (size_t i = 0; i < ZR_MAX_FFI_HANDLES; i++) {
        if (!g_zr_ffi_table[i].used) {
            g_zr_ffi_table[i].id = id;
            g_zr_ffi_table[i].used = true;
            g_zr_ffi_table[i].no_close = no_close;
            g_zr_ffi_table[i].lib = lib;
            stored = true;
            break;
        }
    }
    zr_mutex_unlock(&g_zr_db_ffi_mtx);
    if (!stored) {
        if (!no_close) zr_dlclose(lib);
        vm_raise(vm, "ffi_load(): maximum loaded FFI library limit (128) reached");
        return false;
    }
    *out = val_num((double)id);
    return true;
}

static bool bi_ffi_close(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (!vm_check_cap(vm, vm_effective_cap(vm, CAP_FFI), "ffi_close", "ffi")) return false;
    if (argc != 1 || argv[0].tag != VAL_NUM || argv[0].u.num < 1.0) {
        vm_raise(vm, "ffi_close() expects 1 numeric handle");
        return false;
    }
    size_t id = (size_t)argv[0].u.num;
    zr_db_ffi_ensure_mtx();
    zr_mutex_lock(&g_zr_db_ffi_mtx);
    ZrDlHandle lib = NULL;
    bool no_close = false;
    for (size_t i = 0; i < ZR_MAX_FFI_HANDLES; i++) {
        if (g_zr_ffi_table[i].used && g_zr_ffi_table[i].id == id) {
            lib = g_zr_ffi_table[i].lib;
            no_close = g_zr_ffi_table[i].no_close;
            g_zr_ffi_table[i].used = false;
            g_zr_ffi_table[i].no_close = false;
            g_zr_ffi_table[i].lib = NULL;
            break;
        }
    }
    zr_mutex_unlock(&g_zr_db_ffi_mtx);
    if (!lib) { vm_raise(vm, "ffi_close(): invalid handle"); return false; }
    if (!no_close) zr_dlclose(lib);
    *out = val_nil();
    return true;
}

static bool bi_ffi_call(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (!vm_check_cap(vm, vm_effective_cap(vm, CAP_FFI), "ffi_call", "ffi")) return false;
    if (argc != 3 || argv[0].tag != VAL_NUM || !val_is_obj(argv[1], OBJ_STR) || !val_is_obj(argv[2], OBJ_ARRAY)) {
        vm_raise(vm, "ffi_call(handle, symbol, args_array) expects 3 arguments");
        return false;
    }
    ZrDlHandle lib = zr_ffi_lookup((size_t)argv[0].u.num);
    if (!lib) { vm_raise(vm, "ffi_call(): invalid handle"); return false; }
    const char *sym = ((PbStr *)val_as_obj(argv[1]))->chars;
    PbArray *arr = (PbArray *)val_as_obj(argv[2]);
    if (arr->len > ZR_FFI_MAX_ARGS) { vm_raise(vm, "ffi_call(): max %d arguments supported", ZR_FFI_MAX_ARGS); return false; }
    double a[ZR_FFI_MAX_ARGS] = {0};
    for (size_t i = 0; i < arr->len; i++) {
        if (arr->items[i].tag != VAL_NUM) { vm_raise(vm, "ffi_call(): arguments must be numbers"); return false; }
        a[i] = arr->items[i].u.num;
    }
    void *fp = zr_dlsym(lib, sym);
    if (!fp) { vm_raise(vm, "ffi_call(): symbol \"%s\" not found", sym); return false; }
    double r = 0.0;
    switch (arr->len) {
        case 0: r = ((double (*)(void))fp)(); break;
        case 1: r = ((double (*)(double))fp)(a[0]); break;
        case 2: r = ((double (*)(double, double))fp)(a[0], a[1]); break;
        case 3: r = ((double (*)(double, double, double))fp)(a[0], a[1], a[2]); break;
        case 4: r = ((double (*)(double, double, double, double))fp)(a[0], a[1], a[2], a[3]); break;
        case 5: r = ((double (*)(double, double, double, double, double))fp)(a[0], a[1], a[2], a[3], a[4]); break;
        case 6: r = ((double (*)(double, double, double, double, double, double))fp)(a[0], a[1], a[2], a[3], a[4], a[5]); break;
        case 7: r = ((double (*)(double, double, double, double, double, double, double))fp)(a[0], a[1], a[2], a[3], a[4], a[5], a[6]); break;
        case 8: r = ((double (*)(double, double, double, double, double, double, double, double))fp)(a[0], a[1], a[2], a[3], a[4], a[5], a[6], a[7]); break;
        case 9: r = ((double (*)(double, double, double, double, double, double, double, double, double))fp)(a[0], a[1], a[2], a[3], a[4], a[5], a[6], a[7], a[8]); break;
        case 10: r = ((double (*)(double, double, double, double, double, double, double, double, double, double))fp)(a[0], a[1], a[2], a[3], a[4], a[5], a[6], a[7], a[8], a[9]); break;
    }
    *out = val_num(r);
    return true;
}

typedef enum {
    FFI_T_I8 = 0, FFI_T_U8, FFI_T_I16, FFI_T_U16,
    FFI_T_I32, FFI_T_U32, FFI_T_I64, FFI_T_U64,
    FFI_T_BOOL, FFI_T_PTR, FFI_T_STR,
    FFI_T_F32, FFI_T_F64, FFI_T_VOID
} ZrFfiType;

static bool zr_parse_ffi_type_tok(const char *s, size_t len, bool is_ret, ZrFfiType *out) {
    while (len > 0 && (*s == ' ' || *s == '\t')) { s++; len--; }
    while (len > 0 && (s[len - 1] == ' ' || s[len - 1] == '\t')) len--;
    if ((len == 2 && memcmp(s, "i8", 2) == 0) || (len == 4 && memcmp(s, "char", 4) == 0))  { *out = FFI_T_I8;   return true; }
    if ((len == 2 && memcmp(s, "u8", 2) == 0) || (len == 4 && memcmp(s, "byte", 4) == 0) ||
        (len == 5 && memcmp(s, "uint8", 5) == 0))                                          { *out = FFI_T_U8;   return true; }
    if ((len == 3 && memcmp(s, "i16", 3) == 0) || (len == 5 && memcmp(s, "short", 5) == 0)){ *out = FFI_T_I16;  return true; }
    if ((len == 3 && memcmp(s, "u16", 3) == 0) || (len == 6 && memcmp(s, "ushort", 6) == 0)||
        (len == 6 && memcmp(s, "uint16", 6) == 0))                                         { *out = FFI_T_U16;  return true; }
    if ((len == 3 && memcmp(s, "i32", 3) == 0) || (len == 3 && memcmp(s, "int", 3) == 0))  { *out = FFI_T_I32;  return true; }
    if ((len == 3 && memcmp(s, "u32", 3) == 0) || (len == 4 && memcmp(s, "uint", 4) == 0) ||
        (len == 6 && memcmp(s, "uint32", 6) == 0))                                         { *out = FFI_T_U32;  return true; }
    if ((len == 3 && memcmp(s, "i64", 3) == 0) || (len == 4 && memcmp(s, "long", 4) == 0) ||
        (len == 5 && memcmp(s, "isize", 5) == 0) || (len == 7 && memcmp(s, "ssize_t", 7) == 0) ||
        (len == 6 && memcmp(s, "intptr", 6) == 0))                                         { *out = FFI_T_I64;  return true; }
    if ((len == 3 && memcmp(s, "u64", 3) == 0) || (len == 5 && memcmp(s, "ulong", 5) == 0)||
        (len == 5 && memcmp(s, "usize", 5) == 0) || (len == 6 && memcmp(s, "size_t", 6) == 0) ||
        (len == 7 && memcmp(s, "uintptr", 7) == 0))                                        { *out = FFI_T_U64;  return true; }
    if (len == 4 && memcmp(s, "bool", 4) == 0)                                             { *out = FFI_T_BOOL; return true; }
    if ((len == 3 && memcmp(s, "ptr", 3) == 0) || (len == 7 && memcmp(s, "pointer", 7) == 0) ||
        (len == 5 && memcmp(s, "void*", 5) == 0) || (len == 5 && memcmp(s, "char*", 5) == 0) ||
        (len == 5 && memcmp(s, "bytes", 5) == 0))                                          { *out = FFI_T_PTR;  return true; }
    if ((len == 6 && memcmp(s, "string", 6) == 0) || (len == 3 && memcmp(s, "str", 3) == 0) ||
        (len == 4 && memcmp(s, "cstr", 4) == 0))                                           { *out = FFI_T_STR;  return true; }
    if ((len == 3 && memcmp(s, "f32", 3) == 0) || (len == 5 && memcmp(s, "float", 5) == 0)){ *out = FFI_T_F32;  return true; }
    if ((len == 3 && memcmp(s, "f64", 3) == 0) || (len == 6 && memcmp(s, "double", 6) == 0) ||
        (len == 3 && memcmp(s, "num", 3) == 0))                                            { *out = FFI_T_F64;  return true; }
    if (is_ret && len == 4 && memcmp(s, "void", 4) == 0)                                   { *out = FFI_T_VOID; return true; }
    return false;
}

static bool zr_parse_ffi_sig(PbVM *vm, const char *fn_name, const char *sig, size_t max_args,
                             ZrFfiType *arg_types, size_t *out_nargs, ZrFfiType *out_ret) {
    const char *args_beg = NULL, *args_end = NULL;
    const char *ret_beg = NULL;
    size_t ret_len = 0;

    const char *arrow = strstr(sig, "->");
    if (arrow) {
        args_beg = sig;
        args_end = arrow;
        while (args_beg < args_end && (*args_beg == ' ' || *args_beg == '\t')) args_beg++;
        while (args_end > args_beg && (args_end[-1] == ' ' || args_end[-1] == '\t')) args_end--;
        if (args_end - args_beg >= 2 && args_beg[0] == '(' && args_end[-1] == ')') {
            args_beg++;
            args_end--;
        }
        ret_beg = arrow + 2;
        ret_len = strlen(ret_beg);
    } else {
        const char *lp = strchr(sig, '(');
        const char *rp = lp ? strrchr(lp, ')') : NULL;
        if (!lp || !rp || rp < lp) {
            vm_raise(vm, "%s(): signature '%s' must use 'a, b -> ret' or 'ret(a, b)'", fn_name, sig);
            return false;
        }
        ret_beg = sig;
        ret_len = (size_t)(lp - sig);
        args_beg = lp + 1;
        args_end = rp;
    }

    if (!zr_parse_ffi_type_tok(ret_beg, ret_len, true, out_ret)) {
        vm_raise(vm, "%s(): unsupported return type in signature '%s'", fn_name, sig);
        return false;
    }

    while (args_beg < args_end && (*args_beg == ' ' || *args_beg == '\t')) args_beg++;
    while (args_end > args_beg && (args_end[-1] == ' ' || args_end[-1] == '\t')) args_end--;

    *out_nargs = 0;
    if (args_beg >= args_end || ((size_t)(args_end - args_beg) == 4 && memcmp(args_beg, "void", 4) == 0)) {
        return true;
    }

    const char *ap = args_beg;
    for (;;) {
        if (*out_nargs >= max_args) {
            vm_raise(vm, "%s(): max %zu arguments supported", fn_name, max_args);
            return false;
        }
        const char *comma = (const char *)memchr(ap, ',', (size_t)(args_end - ap));
        size_t tlen = comma ? (size_t)(comma - ap) : (size_t)(args_end - ap);
        if (!zr_parse_ffi_type_tok(ap, tlen, false, &arg_types[*out_nargs])) {
            vm_raise(vm, "%s(): unsupported argument type in signature '%s'", fn_name, sig);
            return false;
        }
        (*out_nargs)++;
        if (!comma) break;
        ap = comma + 1;
    }
    return true;
}

static bool zr_ffi_val_to_ptr(PbVM *vm, const char *fn_name, PbValue v, uintptr_t *out_p) {
    if (v.tag == VAL_NIL) { *out_p = 0; return true; }
    if (v.tag == VAL_NUM) { *out_p = (uintptr_t)(uint64_t)v.u.num; return true; }
    if (val_is_obj(v, OBJ_BYTES)) {
        PbBytes *b = (PbBytes *)val_as_obj(v);
        *out_p = (uintptr_t)b->data;
        return true;
    }
    if (val_is_obj(v, OBJ_F64BUF)) {
        PbF64Buf *fb = (PbF64Buf *)val_as_obj(v);
        *out_p = (uintptr_t)fb->data;
        return true;
    }
    if (val_is_obj(v, OBJ_STR)) {
        PbStr *s = (PbStr *)val_as_obj(v);
        *out_p = (uintptr_t)s->chars;
        return true;
    }
    vm_raise(vm, "%s(): expected pointer, bytes, f64buf, string, or nil", fn_name);
    return false;
}

typedef struct {
    void    *fn;          /* 0   */
    uint64_t gp[6];       /* 8..48   */
    uint64_t xmm[8];      /* 56..112 */
    uint64_t stk[10];     /* 120..192 */
    uint64_t n_stk;       /* 200 */
    uint64_t n_xmm;       /* 208 */
    uint64_t ret_rax;     /* 216 */
    uint64_t ret_xmm0;    /* 224 */
} ZrX64CallCtx;

static bool bi_ffi_call_typed(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (!vm_check_cap(vm, vm_effective_cap(vm, CAP_FFI), "ffi_call_typed", "ffi")) return false;
    if (argc != 4 || argv[0].tag != VAL_NUM || !val_is_obj(argv[1], OBJ_STR) ||
        !val_is_obj(argv[2], OBJ_STR) || !val_is_obj(argv[3], OBJ_ARRAY)) {
        vm_raise(vm, "ffi_call_typed(handle, symbol, sig, args_array) expects 4 arguments");
        return false;
    }
    ZrDlHandle lib = zr_ffi_lookup((size_t)argv[0].u.num);
    if (!lib) { vm_raise(vm, "ffi_call_typed(): invalid handle"); return false; }
    const char *sym = ((PbStr *)val_as_obj(argv[1]))->chars;
    const char *sig = ((PbStr *)val_as_obj(argv[2]))->chars;
    PbArray *args = (PbArray *)val_as_obj(argv[3]);

    ZrFfiType arg_types[ZR_FFI_MAX_ARGS];
    size_t n_sig_args = 0;
    ZrFfiType ret_type = FFI_T_VOID;
    if (!zr_parse_ffi_sig(vm, "ffi_call_typed", sig, ZR_FFI_MAX_ARGS, arg_types, &n_sig_args, &ret_type)) {
        return false;
    }
    if (args->len != n_sig_args) {
        vm_raise(vm, "ffi_call_typed(): signature expects %zu args, got %zu", n_sig_args, args->len);
        return false;
    }

    void *fp = zr_dlsym(lib, sym);
    if (!fp) { vm_raise(vm, "ffi_call_typed(): symbol \"%s\" not found", sym); return false; }

    uint64_t arg_gp_bits[ZR_FFI_MAX_ARGS] = {0};
    double   arg_fp_d[ZR_FFI_MAX_ARGS] = {0};
    uint64_t arg_fp_bits[ZR_FFI_MAX_ARGS] = {0};
    bool     arg_is_fp[ZR_FFI_MAX_ARGS] = {false};
    bool     any_fp_arg = false;
    bool     all_fp_arg = (n_sig_args > 0);

    for (size_t i = 0; i < n_sig_args; i++) {
        PbValue v = args->items[i];
        ZrFfiType t = arg_types[i];
        if (t == FFI_T_F64) {
            if (v.tag != VAL_NUM) { vm_raise(vm, "ffi_call_typed(): arg %zu expected number", i); return false; }
            double dv = v.u.num;
            uint64_t ub = 0;
            memcpy(&ub, &dv, 8);
            arg_fp_d[i] = dv;
            arg_fp_bits[i] = ub;
            arg_is_fp[i] = true;
            any_fp_arg = true;
        } else if (t == FFI_T_F32) {
            if (v.tag != VAL_NUM) { vm_raise(vm, "ffi_call_typed(): arg %zu expected number", i); return false; }
            float fv = (float)v.u.num;
            uint32_t fb = 0;
            memcpy(&fb, &fv, 4);
            arg_fp_d[i] = (double)fv;
            arg_fp_bits[i] = (uint64_t)fb;
            arg_is_fp[i] = true;
            any_fp_arg = true;
        } else {
            all_fp_arg = false;
            if (t == FFI_T_STR) {
                if (v.tag == VAL_NIL) arg_gp_bits[i] = 0;
                else if (val_is_obj(v, OBJ_STR)) arg_gp_bits[i] = (uint64_t)(uintptr_t)((PbStr *)val_as_obj(v))->chars;
                else if (val_is_obj(v, OBJ_BYTES)) arg_gp_bits[i] = (uint64_t)(uintptr_t)((PbBytes *)val_as_obj(v))->data;
                else { vm_raise(vm, "ffi_call_typed(): arg %zu expected string", i); return false; }
            } else if (t == FFI_T_PTR) {
                uintptr_t p = 0;
                if (!zr_ffi_val_to_ptr(vm, "ffi_call_typed", v, &p)) return false;
                arg_gp_bits[i] = (uint64_t)p;
            } else if (t == FFI_T_BOOL) {
                arg_gp_bits[i] = vm_is_truthy(v) ? 1u : 0u;
            } else {
                if (v.tag != VAL_NUM) { vm_raise(vm, "ffi_call_typed(): arg %zu expected integer", i); return false; }
                int64_t iv = (int64_t)v.u.num;
                switch (t) {
                    case FFI_T_I8:  arg_gp_bits[i] = (uint64_t)(int64_t)(int8_t)iv; break;
                    case FFI_T_U8:  arg_gp_bits[i] = (uint64_t)(uint8_t)iv; break;
                    case FFI_T_I16: arg_gp_bits[i] = (uint64_t)(int64_t)(int16_t)iv; break;
                    case FFI_T_U16: arg_gp_bits[i] = (uint64_t)(uint16_t)iv; break;
                    case FFI_T_I32: arg_gp_bits[i] = (uint64_t)(int64_t)(int32_t)iv; break;
                    case FFI_T_U32: arg_gp_bits[i] = (uint64_t)(uint32_t)iv; break;
                    case FFI_T_U64: arg_gp_bits[i] = (uint64_t)v.u.num; break;
                    default:        arg_gp_bits[i] = (uint64_t)iv; break;
                }
            }
        }
    }

    int64_t raw_i = 0;
    double  raw_d = 0.0;

#if (defined(__x86_64__) || defined(__amd64__)) && !defined(_WIN32) && defined(__GNUC__)
    (void)any_fp_arg; (void)all_fp_arg; (void)arg_fp_d;
    ZrX64CallCtx ctx;
    memset(&ctx, 0, sizeof(ctx));
    ctx.fn = fp;
    size_t gp_i = 0, xm_i = 0, st_i = 0;
    for (size_t i = 0; i < n_sig_args; i++) {
        if (arg_is_fp[i]) {
            if (xm_i < 8) ctx.xmm[xm_i++] = arg_fp_bits[i];
            else ctx.stk[st_i++] = arg_fp_bits[i];
        } else {
            if (gp_i < 6) ctx.gp[gp_i++] = arg_gp_bits[i];
            else ctx.stk[st_i++] = arg_gp_bits[i];
        }
    }
    ctx.n_stk = (uint64_t)st_i;
    ctx.n_xmm = (uint64_t)xm_i;
    uint64_t cptr = (uint64_t)(uintptr_t)&ctx;
    __asm__ __volatile__(
        "movq %%rax, %%r13\n\t"
        "movq %%rsp, %%r12\n\t"
        "andq $-16, %%rsp\n\t"
        "subq $80, %%rsp\n\t"
        "movq 120(%%r13), %%rax\n\t" "movq %%rax, 0(%%rsp)\n\t"
        "movq 128(%%r13), %%rax\n\t" "movq %%rax, 8(%%rsp)\n\t"
        "movq 136(%%r13), %%rax\n\t" "movq %%rax, 16(%%rsp)\n\t"
        "movq 144(%%r13), %%rax\n\t" "movq %%rax, 24(%%rsp)\n\t"
        "movq 152(%%r13), %%rax\n\t" "movq %%rax, 32(%%rsp)\n\t"
        "movq 160(%%r13), %%rax\n\t" "movq %%rax, 40(%%rsp)\n\t"
        "movq 168(%%r13), %%rax\n\t" "movq %%rax, 48(%%rsp)\n\t"
        "movq 176(%%r13), %%rax\n\t" "movq %%rax, 56(%%rsp)\n\t"
        "movq 184(%%r13), %%rax\n\t" "movq %%rax, 64(%%rsp)\n\t"
        "movq 192(%%r13), %%rax\n\t" "movq %%rax, 72(%%rsp)\n\t"
        "movq 56(%%r13), %%xmm0\n\t"
        "movq 64(%%r13), %%xmm1\n\t"
        "movq 72(%%r13), %%xmm2\n\t"
        "movq 80(%%r13), %%xmm3\n\t"
        "movq 88(%%r13), %%xmm4\n\t"
        "movq 96(%%r13), %%xmm5\n\t"
        "movq 104(%%r13), %%xmm6\n\t"
        "movq 112(%%r13), %%xmm7\n\t"
        "movq 8(%%r13), %%rdi\n\t"
        "movq 16(%%r13), %%rsi\n\t"
        "movq 24(%%r13), %%rdx\n\t"
        "movq 32(%%r13), %%rcx\n\t"
        "movq 40(%%r13), %%r8\n\t"
        "movq 48(%%r13), %%r9\n\t"
        "movq 208(%%r13), %%rax\n\t"
        "call *0(%%r13)\n\t"
        "movq %%rax, 216(%%r13)\n\t"
        "movq %%xmm0, 224(%%r13)\n\t"
        "movq %%r12, %%rsp\n\t"
        : "+a"(cptr)
        :
        : "rcx", "rdx", "rsi", "rdi", "r8", "r9", "r10", "r11", "r12", "r13",
          "xmm0", "xmm1", "xmm2", "xmm3", "xmm4", "xmm5", "xmm6", "xmm7",
          "xmm8", "xmm9", "xmm10", "xmm11", "xmm12", "xmm13", "xmm14", "xmm15",
          "memory", "cc"
    );
    raw_i = (int64_t)ctx.ret_rax;
    if (ret_type == FFI_T_F64) {
        memcpy(&raw_d, &ctx.ret_xmm0, 8);
    } else if (ret_type == FFI_T_F32) {
        uint32_t fb = (uint32_t)ctx.ret_xmm0;
        float fv = 0.0f;
        memcpy(&fv, &fb, 4);
        raw_d = (double)fv;
    }
#elif defined(_WIN64) && (defined(__x86_64__) || defined(__amd64__) || defined(_M_X64)) && defined(__GNUC__)
    (void)any_fp_arg; (void)all_fp_arg; (void)arg_fp_d;
    ZrX64CallCtx ctx;
    memset(&ctx, 0, sizeof(ctx));
    ctx.fn = fp;
    for (size_t i = 0; i < n_sig_args; i++) {
        uint64_t bits = arg_is_fp[i] ? arg_fp_bits[i] : arg_gp_bits[i];
        if (i < 4) {
            ctx.gp[i] = bits;
            ctx.xmm[i] = bits;
        } else {
            ctx.stk[i - 4] = bits;
        }
    }
    uint64_t cptr = (uint64_t)(uintptr_t)&ctx;
    __asm__ __volatile__(
        "movq %%rax, %%r13\n\t"
        "movq %%rsp, %%r12\n\t"
        "andq $-16, %%rsp\n\t"
        "subq $80, %%rsp\n\t"
        "movq 120(%%r13), %%rax\n\t" "movq %%rax, 32(%%rsp)\n\t"
        "movq 128(%%r13), %%rax\n\t" "movq %%rax, 40(%%rsp)\n\t"
        "movq 136(%%r13), %%rax\n\t" "movq %%rax, 48(%%rsp)\n\t"
        "movq 144(%%r13), %%rax\n\t" "movq %%rax, 56(%%rsp)\n\t"
        "movq 152(%%r13), %%rax\n\t" "movq %%rax, 64(%%rsp)\n\t"
        "movq 160(%%r13), %%rax\n\t" "movq %%rax, 72(%%rsp)\n\t"
        "movq 56(%%r13), %%xmm0\n\t"
        "movq 64(%%r13), %%xmm1\n\t"
        "movq 72(%%r13), %%xmm2\n\t"
        "movq 80(%%r13), %%xmm3\n\t"
        "movq 8(%%r13), %%rcx\n\t"
        "movq 16(%%r13), %%rdx\n\t"
        "movq 24(%%r13), %%r8\n\t"
        "movq 32(%%r13), %%r9\n\t"
        "call *0(%%r13)\n\t"
        "movq %%rax, 216(%%r13)\n\t"
        "movq %%xmm0, 224(%%r13)\n\t"
        "movq %%r12, %%rsp\n\t"
        : "+a"(cptr)
        :
        : "rcx", "rdx", "r8", "r9", "r10", "r11", "r12", "r13",
          "xmm0", "xmm1", "xmm2", "xmm3", "xmm4", "xmm5",
          "memory", "cc"
    );
    raw_i = (int64_t)ctx.ret_rax;
    if (ret_type == FFI_T_F64) {
        memcpy(&raw_d, &ctx.ret_xmm0, 8);
    } else if (ret_type == FFI_T_F32) {
        uint32_t fb = (uint32_t)ctx.ret_xmm0;
        float fv = 0.0f;
        memcpy(&fv, &fb, 4);
        raw_d = (double)fv;
    }
#else
    (void)arg_fp_bits; (void)arg_is_fp;
    int64_t gp[ZR_FFI_MAX_ARGS];
    for (size_t i = 0; i < ZR_FFI_MAX_ARGS; i++) gp[i] = (int64_t)arg_gp_bits[i];
    bool ret_is_fp = (ret_type == FFI_T_F64 || ret_type == FFI_T_F32);
    if (!any_fp_arg) {
        if (ret_type == FFI_T_VOID) {
            switch (n_sig_args) {
                case 0: ((void (*)(void))fp)(); break;
                case 1: ((void (*)(int64_t))fp)(gp[0]); break;
                case 2: ((void (*)(int64_t, int64_t))fp)(gp[0], gp[1]); break;
                case 3: ((void (*)(int64_t, int64_t, int64_t))fp)(gp[0], gp[1], gp[2]); break;
                case 4: ((void (*)(int64_t, int64_t, int64_t, int64_t))fp)(gp[0], gp[1], gp[2], gp[3]); break;
                case 5: ((void (*)(int64_t, int64_t, int64_t, int64_t, int64_t))fp)(gp[0], gp[1], gp[2], gp[3], gp[4]); break;
                case 6: ((void (*)(int64_t, int64_t, int64_t, int64_t, int64_t, int64_t))fp)(gp[0], gp[1], gp[2], gp[3], gp[4], gp[5]); break;
                case 7: ((void (*)(int64_t, int64_t, int64_t, int64_t, int64_t, int64_t, int64_t))fp)(gp[0], gp[1], gp[2], gp[3], gp[4], gp[5], gp[6]); break;
                case 8: ((void (*)(int64_t, int64_t, int64_t, int64_t, int64_t, int64_t, int64_t, int64_t))fp)(gp[0], gp[1], gp[2], gp[3], gp[4], gp[5], gp[6], gp[7]); break;
                case 9: ((void (*)(int64_t, int64_t, int64_t, int64_t, int64_t, int64_t, int64_t, int64_t, int64_t))fp)(gp[0], gp[1], gp[2], gp[3], gp[4], gp[5], gp[6], gp[7], gp[8]); break;
                case 10: ((void (*)(int64_t, int64_t, int64_t, int64_t, int64_t, int64_t, int64_t, int64_t, int64_t, int64_t))fp)(gp[0], gp[1], gp[2], gp[3], gp[4], gp[5], gp[6], gp[7], gp[8], gp[9]); break;
            }
        } else if (ret_type == FFI_T_F64) {
            switch (n_sig_args) {
                case 0: raw_d = ((double (*)(void))fp)(); break;
                case 1: raw_d = ((double (*)(int64_t))fp)(gp[0]); break;
                case 2: raw_d = ((double (*)(int64_t, int64_t))fp)(gp[0], gp[1]); break;
                case 3: raw_d = ((double (*)(int64_t, int64_t, int64_t))fp)(gp[0], gp[1], gp[2]); break;
                case 4: raw_d = ((double (*)(int64_t, int64_t, int64_t, int64_t))fp)(gp[0], gp[1], gp[2], gp[3]); break;
                default: raw_d = ((double (*)(int64_t, int64_t, int64_t, int64_t, int64_t, int64_t))fp)(gp[0], gp[1], gp[2], gp[3], gp[4], gp[5]); break;
            }
        } else if (ret_type == FFI_T_F32) {
            raw_d = (double)((float (*)(int64_t, int64_t, int64_t, int64_t))fp)(gp[0], gp[1], gp[2], gp[3]);
        } else {
            switch (n_sig_args) {
                case 0: raw_i = ((int64_t (*)(void))fp)(); break;
                case 1: raw_i = ((int64_t (*)(int64_t))fp)(gp[0]); break;
                case 2: raw_i = ((int64_t (*)(int64_t, int64_t))fp)(gp[0], gp[1]); break;
                case 3: raw_i = ((int64_t (*)(int64_t, int64_t, int64_t))fp)(gp[0], gp[1], gp[2]); break;
                case 4: raw_i = ((int64_t (*)(int64_t, int64_t, int64_t, int64_t))fp)(gp[0], gp[1], gp[2], gp[3]); break;
                case 5: raw_i = ((int64_t (*)(int64_t, int64_t, int64_t, int64_t, int64_t))fp)(gp[0], gp[1], gp[2], gp[3], gp[4]); break;
                case 6: raw_i = ((int64_t (*)(int64_t, int64_t, int64_t, int64_t, int64_t, int64_t))fp)(gp[0], gp[1], gp[2], gp[3], gp[4], gp[5]); break;
                case 7: raw_i = ((int64_t (*)(int64_t, int64_t, int64_t, int64_t, int64_t, int64_t, int64_t))fp)(gp[0], gp[1], gp[2], gp[3], gp[4], gp[5], gp[6]); break;
                case 8: raw_i = ((int64_t (*)(int64_t, int64_t, int64_t, int64_t, int64_t, int64_t, int64_t, int64_t))fp)(gp[0], gp[1], gp[2], gp[3], gp[4], gp[5], gp[6], gp[7]); break;
                case 9: raw_i = ((int64_t (*)(int64_t, int64_t, int64_t, int64_t, int64_t, int64_t, int64_t, int64_t, int64_t))fp)(gp[0], gp[1], gp[2], gp[3], gp[4], gp[5], gp[6], gp[7], gp[8]); break;
                case 10: raw_i = ((int64_t (*)(int64_t, int64_t, int64_t, int64_t, int64_t, int64_t, int64_t, int64_t, int64_t, int64_t))fp)(gp[0], gp[1], gp[2], gp[3], gp[4], gp[5], gp[6], gp[7], gp[8], gp[9]); break;
            }
        }
    } else if (all_fp_arg) {
        if (ret_is_fp) {
            switch (n_sig_args) {
                case 1: raw_d = ((double (*)(double))fp)(arg_fp_d[0]); break;
                case 2: raw_d = ((double (*)(double, double))fp)(arg_fp_d[0], arg_fp_d[1]); break;
                case 3: raw_d = ((double (*)(double, double, double))fp)(arg_fp_d[0], arg_fp_d[1], arg_fp_d[2]); break;
                case 4: raw_d = ((double (*)(double, double, double, double))fp)(arg_fp_d[0], arg_fp_d[1], arg_fp_d[2], arg_fp_d[3]); break;
                case 5: raw_d = ((double (*)(double, double, double, double, double))fp)(arg_fp_d[0], arg_fp_d[1], arg_fp_d[2], arg_fp_d[3], arg_fp_d[4]); break;
                case 6: raw_d = ((double (*)(double, double, double, double, double, double))fp)(arg_fp_d[0], arg_fp_d[1], arg_fp_d[2], arg_fp_d[3], arg_fp_d[4], arg_fp_d[5]); break;
                default: raw_d = ((double (*)(double, double, double, double, double, double, double, double))fp)(arg_fp_d[0], arg_fp_d[1], arg_fp_d[2], arg_fp_d[3], arg_fp_d[4], arg_fp_d[5], arg_fp_d[6], arg_fp_d[7]); break;
            }
        } else {
            raw_i = ((int64_t (*)(double, double, double, double))fp)(arg_fp_d[0], arg_fp_d[1], arg_fp_d[2], arg_fp_d[3]);
        }
    } else if (n_sig_args == 2 && !arg_is_fp[0] && arg_is_fp[1]) {
        if (ret_is_fp) raw_d = ((double (*)(int64_t, double))fp)(gp[0], arg_fp_d[1]);
        else raw_i = ((int64_t (*)(int64_t, double))fp)(gp[0], arg_fp_d[1]);
    } else if (n_sig_args == 2 && arg_is_fp[0] && !arg_is_fp[1]) {
        if (ret_is_fp) raw_d = ((double (*)(double, int64_t))fp)(arg_fp_d[0], gp[1]);
        else raw_i = ((int64_t (*)(double, int64_t))fp)(arg_fp_d[0], gp[1]);
    } else {
        vm_raise(vm, "ffi_call_typed(): mixed GP/FP signature '%s' requires x86_64 host", sig);
        return false;
    }
#endif

    switch (ret_type) {
        case FFI_T_VOID: *out = val_nil(); break;
        case FFI_T_F64:
        case FFI_T_F32:  *out = val_num(raw_d); break;
        case FFI_T_BOOL: *out = val_bool((raw_i & 0xff) != 0); break;
        case FFI_T_I8:   *out = val_num((double)(int8_t)raw_i); break;
        case FFI_T_U8:   *out = val_num((double)(uint8_t)raw_i); break;
        case FFI_T_I16:  *out = val_num((double)(int16_t)raw_i); break;
        case FFI_T_U16:  *out = val_num((double)(uint16_t)raw_i); break;
        case FFI_T_I32:  *out = val_num((double)(int32_t)raw_i); break;
        case FFI_T_U32:  *out = val_num((double)(uint32_t)raw_i); break;
        case FFI_T_I64:  *out = val_num((double)raw_i); break;
        case FFI_T_U64:  *out = val_num((double)(uint64_t)raw_i); break;
        case FFI_T_PTR:  *out = val_num((double)(uintptr_t)raw_i); break;
        case FFI_T_STR: {
            const char *cs = (const char *)(uintptr_t)raw_i;
            *out = cs ? val_obj(vm_new_cstr(vm, cs)) : val_nil();
            break;
        }
    }
    return true;
}

/* ---- Direct Zero-Copy Buffer & Raw Pointer Helpers ---- */
static bool bi_ffi_bytes_ptr(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (!vm_check_cap(vm, vm_effective_cap(vm, CAP_FFI), "ffi_bytes_ptr", "ffi")) return false;
    if (argc != 1) { vm_raise(vm, "ffi_bytes_ptr(buf) expects 1 argument"); return false; }
    uintptr_t p = 0;
    if (!zr_ffi_val_to_ptr(vm, "ffi_bytes_ptr", argv[0], &p)) return false;
    *out = val_num((double)p);
    return true;
}

static bool bi_ffi_ptr_read(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (!vm_check_cap(vm, vm_effective_cap(vm, CAP_FFI), "ffi_ptr_read", "ffi")) return false;
    if (argc != 2 || argv[1].tag != VAL_NUM || argv[1].u.num < 0.0) {
        vm_raise(vm, "ffi_ptr_read(ptr, len) expects (pointer, non-negative len)");
        return false;
    }
    uintptr_t p = 0;
    if (!zr_ffi_val_to_ptr(vm, "ffi_ptr_read", argv[0], &p)) return false;
    size_t len = (size_t)argv[1].u.num;
    if (len > 0 && p == 0) { vm_raise(vm, "ffi_ptr_read(): NULL pointer"); return false; }
    PbBytes *b = pb_bytes_new(vm, len, true);
    if (len > 0) memcpy(b->data, (const void *)p, len);
    *out = val_obj(b);
    return true;
}

static bool bi_ffi_ptr_write(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (!vm_check_cap(vm, vm_effective_cap(vm, CAP_FFI), "ffi_ptr_write", "ffi")) return false;
    if (argc < 2 || argc > 3) {
        vm_raise(vm, "ffi_ptr_write(ptr, bytes_or_str[, len]) expects 2 or 3 arguments");
        return false;
    }
    uintptr_t p = 0;
    if (!zr_ffi_val_to_ptr(vm, "ffi_ptr_write", argv[0], &p)) return false;
    const uint8_t *src = NULL;
    size_t slen = 0;
    if (val_is_obj(argv[1], OBJ_BYTES)) {
        src = ((PbBytes *)val_as_obj(argv[1]))->data;
        slen = ((PbBytes *)val_as_obj(argv[1]))->len;
    } else if (val_is_obj(argv[1], OBJ_STR)) {
        src = (const uint8_t *)((PbStr *)val_as_obj(argv[1]))->chars;
        slen = ((PbStr *)val_as_obj(argv[1]))->len;
    } else {
        vm_raise(vm, "ffi_ptr_write(): 2nd arg must be bytes or string");
        return false;
    }
    if (argc == 3 && argv[2].tag == VAL_NUM && argv[2].u.num >= 0.0 && (size_t)argv[2].u.num < slen) {
        slen = (size_t)argv[2].u.num;
    }
    if (slen > 0 && p == 0) { vm_raise(vm, "ffi_ptr_write(): NULL pointer"); return false; }
    if (slen > 0 && src) memcpy((void *)p, src, slen);
    *out = val_num((double)slen);
    return true;
}

static bool bi_ffi_read_i32(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (!vm_check_cap(vm, vm_effective_cap(vm, CAP_FFI), "ffi_read_i32", "ffi")) return false;
    if (argc < 1 || argc > 2) { vm_raise(vm, "ffi_read_i32(ptr[, byte_offset]) expects 1 or 2 args"); return false; }
    uintptr_t p = 0;
    if (!zr_ffi_val_to_ptr(vm, "ffi_read_i32", argv[0], &p) || p == 0) {
        if (p == 0) vm_raise(vm, "ffi_read_i32(): NULL pointer");
        return false;
    }
    int64_t off = (argc == 2 && argv[1].tag == VAL_NUM) ? (int64_t)argv[1].u.num : 0;
    int32_t v = 0;
    memcpy(&v, (const uint8_t *)p + off, sizeof(v));
    *out = val_num((double)v);
    return true;
}

static bool bi_ffi_read_i64(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (!vm_check_cap(vm, vm_effective_cap(vm, CAP_FFI), "ffi_read_i64", "ffi")) return false;
    if (argc < 1 || argc > 2) { vm_raise(vm, "ffi_read_i64(ptr[, byte_offset]) expects 1 or 2 args"); return false; }
    uintptr_t p = 0;
    if (!zr_ffi_val_to_ptr(vm, "ffi_read_i64", argv[0], &p) || p == 0) {
        if (p == 0) vm_raise(vm, "ffi_read_i64(): NULL pointer");
        return false;
    }
    int64_t off = (argc == 2 && argv[1].tag == VAL_NUM) ? (int64_t)argv[1].u.num : 0;
    int64_t v = 0;
    memcpy(&v, (const uint8_t *)p + off, sizeof(v));
    *out = val_num((double)v);
    return true;
}

static bool bi_ffi_read_f64(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (!vm_check_cap(vm, vm_effective_cap(vm, CAP_FFI), "ffi_read_f64", "ffi")) return false;
    if (argc < 1 || argc > 2) { vm_raise(vm, "ffi_read_f64(ptr[, byte_offset]) expects 1 or 2 args"); return false; }
    uintptr_t p = 0;
    if (!zr_ffi_val_to_ptr(vm, "ffi_read_f64", argv[0], &p) || p == 0) {
        if (p == 0) vm_raise(vm, "ffi_read_f64(): NULL pointer");
        return false;
    }
    int64_t off = (argc == 2 && argv[1].tag == VAL_NUM) ? (int64_t)argv[1].u.num : 0;
    double v = 0.0;
    memcpy(&v, (const uint8_t *)p + off, sizeof(v));
    *out = val_num(v);
    return true;
}

static bool bi_ffi_read_cstr(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (!vm_check_cap(vm, vm_effective_cap(vm, CAP_FFI), "ffi_read_cstr", "ffi")) return false;
    if (argc != 1) { vm_raise(vm, "ffi_read_cstr(ptr) expects 1 argument"); return false; }
    uintptr_t p = 0;
    if (!zr_ffi_val_to_ptr(vm, "ffi_read_cstr", argv[0], &p)) return false;
    if (p == 0) { *out = val_nil(); return true; }
    *out = val_obj(vm_new_cstr(vm, (const char *)p));
    return true;
}

/* ---- Struct Packing & Unpacking (`ffi_struct_pack` / `ffi_struct_unpack`) ---- */
static void zr_put_endian(uint8_t *dst, uint64_t v, size_t sz, bool big_endian) {
    if (big_endian) {
        for (size_t i = 0; i < sz; i++) dst[sz - 1 - i] = (uint8_t)((v >> (i * 8)) & 0xFFu);
    } else {
        for (size_t i = 0; i < sz; i++) dst[i] = (uint8_t)((v >> (i * 8)) & 0xFFu);
    }
}

static uint64_t zr_get_endian(const uint8_t *src, size_t sz, bool big_endian) {
    uint64_t v = 0;
    if (big_endian) {
        for (size_t i = 0; i < sz; i++) v = (v << 8) | (uint64_t)src[i];
    } else {
        for (size_t i = 0; i < sz; i++) v |= ((uint64_t)src[i]) << (i * 8);
    }
    return v;
}

static bool bi_ffi_struct_pack(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 2 || !val_is_obj(argv[0], OBJ_STR) || !val_is_obj(argv[1], OBJ_ARRAY)) {
        vm_raise(vm, "ffi_struct_pack(fmt, values_array) expects (string, array)");
        return false;
    }
    const char *fmt = ((PbStr *)val_as_obj(argv[0]))->chars;
    PbArray *vals = (PbArray *)val_as_obj(argv[1]);
    bool big_endian = false;
    bool natural_align = false;
    size_t fi = 0;
    if (fmt[0] == '>' || fmt[0] == '!') { big_endian = true; fi = 1; }
    else if (fmt[0] == '<' || fmt[0] == '=') { big_endian = false; fi = 1; }
    else if (fmt[0] == '@') { natural_align = true; fi = 1; }

    ZrBuf buf;
    zr_buf_init(&buf);
    size_t vi = 0;
    uint8_t taint = argv[0].taint | argv[1].taint;

    while (fmt[fi]) {
        if (fmt[fi] == ' ' || fmt[fi] == '\t' || fmt[fi] == ',') { fi++; continue; }
        size_t count = 0;
        bool has_count = false;
        while (fmt[fi] >= '0' && fmt[fi] <= '9') {
            count = count * 10 + (size_t)(fmt[fi] - '0');
            has_count = true;
            fi++;
        }
        if (!has_count) count = 1;
        char code = fmt[fi++];
        if (!code) break;

        if (code == 'x') {
            for (size_t k = 0; k < count; k++) zr_buf_push(&buf, 0);
            continue;
        }
        if (code == 's') {
            if (vi >= vals->len) { zr_buf_free(&buf); vm_raise(vm, "ffi_struct_pack(): too few values for format '%s'", fmt); return false; }
            PbValue v = vals->items[vi++];
            taint |= v.taint;
            const uint8_t *sp = NULL;
            size_t sl = 0;
            if (val_is_obj(v, OBJ_STR)) { sp = (const uint8_t *)((PbStr *)val_as_obj(v))->chars; sl = ((PbStr *)val_as_obj(v))->len; }
            else if (val_is_obj(v, OBJ_BYTES)) { sp = ((PbBytes *)val_as_obj(v))->data; sl = ((PbBytes *)val_as_obj(v))->len; }
            else { zr_buf_free(&buf); vm_raise(vm, "ffi_struct_pack(): 's' expects string or bytes"); return false; }
            size_t copy_n = sl < count ? sl : count;
            if (copy_n > 0 && sp) zr_buf_append(&buf, (const char *)sp, copy_n);
            for (size_t k = copy_n; k < count; k++) zr_buf_push(&buf, 0);
            continue;
        }

        size_t fsz = 0;
        switch (code) {
            case 'b': case 'B': case '?': fsz = 1; break;
            case 'h': case 'H':           fsz = 2; break;
            case 'i': case 'I': case 'f': fsz = 4; break;
            case 'l': case 'L':           fsz = natural_align ? sizeof(long) : 4; break;
            case 'q': case 'Q': case 'd': fsz = 8; break;
            case 'n': case 'N': case 'P': case 'p': fsz = sizeof(uintptr_t); break;
            default:
                zr_buf_free(&buf);
                vm_raise(vm, "ffi_struct_pack(): unknown format specifier '%c' in '%s'", code, fmt);
                return false;
        }

        for (size_t k = 0; k < count; k++) {
            if (natural_align && fsz > 1) {
                while ((buf.len % fsz) != 0) zr_buf_push(&buf, 0);
            }
            if (vi >= vals->len) {
                zr_buf_free(&buf);
                vm_raise(vm, "ffi_struct_pack(): too few values for format '%s'", fmt);
                return false;
            }
            PbValue v = vals->items[vi++];
            taint |= v.taint;
            uint8_t raw[8] = {0};
            if (code == '?') {
                raw[0] = vm_is_truthy(v) ? 1u : 0u;
            } else if (code == 'f') {
                if (v.tag != VAL_NUM) { zr_buf_free(&buf); vm_raise(vm, "ffi_struct_pack(): 'f' expects number"); return false; }
                float fv = (float)v.u.num;
                uint32_t ub = 0;
                memcpy(&ub, &fv, 4);
                zr_put_endian(raw, (uint64_t)ub, 4, big_endian);
            } else if (code == 'd') {
                if (v.tag != VAL_NUM) { zr_buf_free(&buf); vm_raise(vm, "ffi_struct_pack(): 'd' expects number"); return false; }
                double dv = v.u.num;
                uint64_t ub = 0;
                memcpy(&ub, &dv, 8);
                zr_put_endian(raw, ub, 8, big_endian);
            } else if (code == 'P' || code == 'p') {
                uintptr_t p = 0;
                if (!zr_ffi_val_to_ptr(vm, "ffi_struct_pack", v, &p)) { zr_buf_free(&buf); return false; }
                zr_put_endian(raw, (uint64_t)p, fsz, big_endian);
            } else {
                if (v.tag != VAL_NUM) { zr_buf_free(&buf); vm_raise(vm, "ffi_struct_pack(): '%c' expects number", code); return false; }
                uint64_t uv = (code == 'Q' || code == 'I' || code == 'L' || code == 'H' || code == 'B' || code == 'N')
                              ? (uint64_t)(v.u.num < 0.0 ? (uint64_t)(int64_t)v.u.num : (uint64_t)v.u.num)
                              : (uint64_t)(int64_t)v.u.num;
                zr_put_endian(raw, uv, fsz, big_endian);
            }
            zr_buf_append(&buf, (const char *)raw, fsz);
        }
    }

    PbBytes *res = pb_bytes_new(vm, buf.len, true);
    if (buf.len > 0 && buf.data) memcpy(res->data, buf.data, buf.len);
    zr_buf_free(&buf);
    PbValue rv = val_obj(res);
    rv.taint = taint;
    *out = rv;
    return true;
}

static bool bi_ffi_struct_unpack(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc < 2 || argc > 3 || !val_is_obj(argv[0], OBJ_STR)) {
        vm_raise(vm, "ffi_struct_unpack(fmt, bytes_or_array[, offset]) expects 2 or 3 arguments");
        return false;
    }
    const char *fmt = ((PbStr *)val_as_obj(argv[0]))->chars;
    const uint8_t *src = NULL;
    size_t slen = 0;
    uint8_t *owned = NULL;

    if (val_is_obj(argv[1], OBJ_BYTES)) {
        src = ((PbBytes *)val_as_obj(argv[1]))->data;
        slen = ((PbBytes *)val_as_obj(argv[1]))->len;
    } else if (val_is_obj(argv[1], OBJ_STR)) {
        src = (const uint8_t *)((PbStr *)val_as_obj(argv[1]))->chars;
        slen = ((PbStr *)val_as_obj(argv[1]))->len;
    } else if (val_is_obj(argv[1], OBJ_ARRAY)) {
        PbArray *ba = (PbArray *)val_as_obj(argv[1]);
        slen = ba->len;
        owned = (uint8_t *)zr_xmalloc(slen > 0 ? slen : 1);
        for (size_t i = 0; i < slen; i++) {
            if (ba->items[i].tag != VAL_NUM) { free(owned); vm_raise(vm, "ffi_struct_unpack(): byte array must contain numbers"); return false; }
            owned[i] = (uint8_t)(int64_t)ba->items[i].u.num;
        }
        src = owned;
    } else {
        vm_raise(vm, "ffi_struct_unpack(): 2nd arg must be bytes, string, or byte array");
        return false;
    }

    size_t pos = (argc == 3 && argv[2].tag == VAL_NUM && argv[2].u.num > 0.0) ? (size_t)argv[2].u.num : 0;
    if (pos > slen) pos = slen;

    bool big_endian = false;
    bool natural_align = false;
    size_t fi = 0;
    if (fmt[0] == '>' || fmt[0] == '!') { big_endian = true; fi = 1; }
    else if (fmt[0] == '<' || fmt[0] == '=') { big_endian = false; fi = 1; }
    else if (fmt[0] == '@') { natural_align = true; fi = 1; }

    PbArray *arr = vm_new_array(vm, 8);
    vm_temp_push(vm, val_obj(arr));
    uint8_t taint = argv[1].taint;

    while (fmt[fi]) {
        if (fmt[fi] == ' ' || fmt[fi] == '\t' || fmt[fi] == ',') { fi++; continue; }
        size_t count = 0;
        bool has_count = false;
        while (fmt[fi] >= '0' && fmt[fi] <= '9') {
            count = count * 10 + (size_t)(fmt[fi] - '0');
            has_count = true;
            fi++;
        }
        if (!has_count) count = 1;
        char code = fmt[fi++];
        if (!code) break;

        if (code == 'x') {
            if (pos + count > slen) { free(owned); vm_temp_pop(vm, 1); vm_raise(vm, "ffi_struct_unpack(): buffer too short"); return false; }
            pos += count;
            continue;
        }
        if (code == 's') {
            if (pos + count > slen) { free(owned); vm_temp_pop(vm, 1); vm_raise(vm, "ffi_struct_unpack(): buffer too short for '%zus'", count); return false; }
            size_t actual = count;
            while (actual > 0 && src[pos + actual - 1] == 0) actual--;
            PbValue sv = val_obj(vm_new_str(vm, (const char *)(src + pos), actual));
            sv.taint = taint;
            arr_push(arr, sv);
            pos += count;
            continue;
        }

        size_t fsz = 0;
        switch (code) {
            case 'b': case 'B': case '?': fsz = 1; break;
            case 'h': case 'H':           fsz = 2; break;
            case 'i': case 'I': case 'f': fsz = 4; break;
            case 'l': case 'L':           fsz = natural_align ? sizeof(long) : 4; break;
            case 'q': case 'Q': case 'd': fsz = 8; break;
            case 'n': case 'N': case 'P': case 'p': fsz = sizeof(uintptr_t); break;
            default:
                free(owned);
                vm_temp_pop(vm, 1);
                vm_raise(vm, "ffi_struct_unpack(): unknown format specifier '%c'", code);
                return false;
        }

        for (size_t k = 0; k < count; k++) {
            if (natural_align && fsz > 1) {
                while ((pos % fsz) != 0) pos++;
            }
            if (pos + fsz > slen) {
                free(owned);
                vm_temp_pop(vm, 1);
                vm_raise(vm, "ffi_struct_unpack(): buffer too short at byte %zu", pos);
                return false;
            }
            uint64_t raw = zr_get_endian(src + pos, fsz, big_endian);
            pos += fsz;
            PbValue item = val_nil();
            switch (code) {
                case '?': item = val_bool(raw != 0); break;
                case 'b': item = val_num((double)(int8_t)raw); break;
                case 'B': item = val_num((double)(uint8_t)raw); break;
                case 'h': item = val_num((double)(int16_t)raw); break;
                case 'H': item = val_num((double)(uint16_t)raw); break;
                case 'i': item = val_num((double)(int32_t)raw); break;
                case 'I': item = val_num((double)(uint32_t)raw); break;
                case 'l': item = val_num(fsz == 8 ? (double)(int64_t)raw : (double)(int32_t)raw); break;
                case 'L': item = val_num(fsz == 8 ? (double)(uint64_t)raw : (double)(uint32_t)raw); break;
                case 'q': case 'n': item = val_num((double)(int64_t)raw); break;
                case 'Q': case 'N': case 'P': case 'p': item = val_num((double)raw); break;
                case 'f': {
                    uint32_t fb = (uint32_t)raw;
                    float fv = 0.0f;
                    memcpy(&fv, &fb, 4);
                    item = val_num((double)fv);
                    break;
                }
                case 'd': {
                    double dv = 0.0;
                    memcpy(&dv, &raw, 8);
                    item = val_num(dv);
                    break;
                }
            }
            item.taint = taint;
            arr_push(arr, item);
        }
    }

    free(owned);
    vm_temp_pop(vm, 1);
    *out = val_obj(arr);
    return true;
}

/* ---- 8 Static Native C Callback Trampolines (`ffi_callback_new` / `ffi_callback_free`) ---- */
#define ZR_FFI_CB_SLOTS 8

typedef struct {
    bool      used;
    uint8_t   tramp_kind; /* 0 = GP(4x intptr_t -> intptr_t), 1 = FP1(double->double), 2 = FP2(double,double->double) */
    PbVM     *vm;
    PbValue   closure;
    ZrFfiType ret_type;
    ZrFfiType arg_types[4];
    size_t    n_args;
    void     *code_ptr;
} ZrFfiCbSlot;

static ZrFfiCbSlot g_zr_ffi_cbs[ZR_FFI_CB_SLOTS];

static intptr_t zr_ffi_cb_dispatch_gp(int slot, intptr_t a0, intptr_t a1, intptr_t a2, intptr_t a3) {
    ZrFfiCbSlot *s = &g_zr_ffi_cbs[slot];
    if (!s->used || !s->vm || s->vm->err_msg[0] != '\0' || s->vm->exited) return 0;
    intptr_t raw[4] = { a0, a1, a2, a3 };
    PbValue cb_args[4];
    for (size_t i = 0; i < s->n_args && i < 4; i++) {
        switch (s->arg_types[i]) {
            case FFI_T_I8:   cb_args[i] = val_num((double)(int8_t)raw[i]); break;
            case FFI_T_U8:   cb_args[i] = val_num((double)(uint8_t)raw[i]); break;
            case FFI_T_I16:  cb_args[i] = val_num((double)(int16_t)raw[i]); break;
            case FFI_T_U16:  cb_args[i] = val_num((double)(uint16_t)raw[i]); break;
            case FFI_T_I32:  cb_args[i] = val_num((double)(int32_t)raw[i]); break;
            case FFI_T_U32:  cb_args[i] = val_num((double)(uint32_t)raw[i]); break;
            case FFI_T_BOOL: cb_args[i] = val_bool((raw[i] & 0xff) != 0); break;
            case FFI_T_STR:  cb_args[i] = raw[i] ? val_obj(vm_new_cstr(s->vm, (const char *)(uintptr_t)raw[i])) : val_nil(); break;
            case FFI_T_PTR:
            case FFI_T_U64:  cb_args[i] = val_num((double)(uintptr_t)raw[i]); break;
            default:         cb_args[i] = val_num((double)(int64_t)raw[i]); break;
        }
    }
    PbValue ret = val_nil();
    if (!vm_call_value_sync(s->vm, s->closure, (int)s->n_args, cb_args, &ret)) return 0;
    if (s->ret_type == FFI_T_VOID) return 0;
    if (s->ret_type == FFI_T_BOOL) return vm_is_truthy(ret) ? 1 : 0;
    if (ret.tag == VAL_NUM) return (intptr_t)(int64_t)ret.u.num;
    if (ret.tag == VAL_BOOL) return ret.u.b ? 1 : 0;
    return 0;
}

static double zr_ffi_cb_dispatch_fp(int slot, double d0, double d1) {
    ZrFfiCbSlot *s = &g_zr_ffi_cbs[slot];
    if (!s->used || !s->vm || s->vm->err_msg[0] != '\0' || s->vm->exited) return 0.0;
    PbValue cb_args[2] = { val_num(d0), val_num(d1) };
    PbValue ret = val_nil();
    if (!vm_call_value_sync(s->vm, s->closure, (int)s->n_args, cb_args, &ret)) return 0.0;
    return ret.tag == VAL_NUM ? ret.u.num : 0.0;
}

#define ZR_DEF_FFI_CB_SLOT(N) \
    static intptr_t zr_ffi_cb_gp_##N(intptr_t a0, intptr_t a1, intptr_t a2, intptr_t a3) { \
        return zr_ffi_cb_dispatch_gp(N, a0, a1, a2, a3); \
    } \
    static double zr_ffi_cb_fp1_##N(double d0) { return zr_ffi_cb_dispatch_fp(N, d0, 0.0); } \
    static double zr_ffi_cb_fp2_##N(double d0, double d1) { return zr_ffi_cb_dispatch_fp(N, d0, d1); }

ZR_DEF_FFI_CB_SLOT(0)
ZR_DEF_FFI_CB_SLOT(1)
ZR_DEF_FFI_CB_SLOT(2)
ZR_DEF_FFI_CB_SLOT(3)
ZR_DEF_FFI_CB_SLOT(4)
ZR_DEF_FFI_CB_SLOT(5)
ZR_DEF_FFI_CB_SLOT(6)
ZR_DEF_FFI_CB_SLOT(7)
#undef ZR_DEF_FFI_CB_SLOT

static void *zr_ffi_cb_get_fnptr(int slot, uint8_t tramp_kind) {
    static intptr_t (*const gp_fns[ZR_FFI_CB_SLOTS])(intptr_t, intptr_t, intptr_t, intptr_t) = {
        zr_ffi_cb_gp_0, zr_ffi_cb_gp_1, zr_ffi_cb_gp_2, zr_ffi_cb_gp_3,
        zr_ffi_cb_gp_4, zr_ffi_cb_gp_5, zr_ffi_cb_gp_6, zr_ffi_cb_gp_7
    };
    static double (*const fp1_fns[ZR_FFI_CB_SLOTS])(double) = {
        zr_ffi_cb_fp1_0, zr_ffi_cb_fp1_1, zr_ffi_cb_fp1_2, zr_ffi_cb_fp1_3,
        zr_ffi_cb_fp1_4, zr_ffi_cb_fp1_5, zr_ffi_cb_fp1_6, zr_ffi_cb_fp1_7
    };
    static double (*const fp2_fns[ZR_FFI_CB_SLOTS])(double, double) = {
        zr_ffi_cb_fp2_0, zr_ffi_cb_fp2_1, zr_ffi_cb_fp2_2, zr_ffi_cb_fp2_3,
        zr_ffi_cb_fp2_4, zr_ffi_cb_fp2_5, zr_ffi_cb_fp2_6, zr_ffi_cb_fp2_7
    };
    if (tramp_kind == 1) return (void *)(uintptr_t)fp1_fns[slot];
    if (tramp_kind == 2) return (void *)(uintptr_t)fp2_fns[slot];
    return (void *)(uintptr_t)gp_fns[slot];
}

static bool bi_ffi_callback_new(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (!vm_check_cap(vm, vm_effective_cap(vm, CAP_FFI), "ffi_callback_new", "ffi")) return false;
    if (argc != 2 || !val_is_obj(argv[0], OBJ_STR)) {
        vm_raise(vm, "ffi_callback_new(sig, fn) expects (signature_string, callable)");
        return false;
    }
    if (!(argv[1].tag == VAL_BUILTIN || val_is_obj(argv[1], OBJ_CLOSURE) || val_is_obj(argv[1], OBJ_FUNC))) {
        vm_raise(vm, "ffi_callback_new(): 2nd argument must be a function/closure");
        return false;
    }
    const char *sig = ((PbStr *)val_as_obj(argv[0]))->chars;
    ZrFfiType arg_types[4];
    size_t n_args = 0;
    ZrFfiType ret_type = FFI_T_VOID;
    if (!zr_parse_ffi_sig(vm, "ffi_callback_new", sig, 4, arg_types, &n_args, &ret_type)) return false;

    bool any_fp = (ret_type == FFI_T_F64 || ret_type == FFI_T_F32);
    bool all_f64 = (ret_type == FFI_T_F64);
    for (size_t i = 0; i < n_args; i++) {
        if (arg_types[i] == FFI_T_F64 || arg_types[i] == FFI_T_F32) any_fp = true;
        if (arg_types[i] != FFI_T_F64) all_f64 = false;
    }
    uint8_t tramp_kind = 0;
    if (any_fp) {
        if (all_f64 && n_args == 1) tramp_kind = 1;
        else if (all_f64 && n_args == 2) tramp_kind = 2;
        else {
            vm_raise(vm, "ffi_callback_new(): callback signature '%s' must be GP/pointer (0..4 args) or f64->f64 / f64,f64->f64", sig);
            return false;
        }
    }

    zr_db_ffi_ensure_mtx();
    zr_mutex_lock(&g_zr_db_ffi_mtx);
    int slot = -1;
    for (int i = 0; i < ZR_FFI_CB_SLOTS; i++) {
        if (!g_zr_ffi_cbs[i].used) {
            slot = i;
            g_zr_ffi_cbs[i].used = true;
            g_zr_ffi_cbs[i].tramp_kind = tramp_kind;
            g_zr_ffi_cbs[i].vm = vm;
            g_zr_ffi_cbs[i].closure = argv[1];
            g_zr_ffi_cbs[i].ret_type = ret_type;
            g_zr_ffi_cbs[i].n_args = n_args;
            for (size_t k = 0; k < n_args; k++) g_zr_ffi_cbs[i].arg_types[k] = arg_types[k];
            g_zr_ffi_cbs[i].code_ptr = zr_ffi_cb_get_fnptr(i, tramp_kind);
            break;
        }
    }
    zr_mutex_unlock(&g_zr_db_ffi_mtx);
    if (slot < 0) {
        vm_raise(vm, "ffi_callback_new(): all %d static callback trampoline slots in use", ZR_FFI_CB_SLOTS);
        return false;
    }
    *out = val_num((double)(uintptr_t)g_zr_ffi_cbs[slot].code_ptr);
    return true;
}

static bool bi_ffi_callback_free(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (!vm_check_cap(vm, vm_effective_cap(vm, CAP_FFI), "ffi_callback_free", "ffi")) return false;
    if (argc != 1 || argv[0].tag != VAL_NUM) {
        vm_raise(vm, "ffi_callback_free(cb_ptr) expects 1 pointer argument");
        return false;
    }
    void *ptr = (void *)(uintptr_t)(uint64_t)argv[0].u.num;
    zr_db_ffi_ensure_mtx();
    zr_mutex_lock(&g_zr_db_ffi_mtx);
    for (int i = 0; i < ZR_FFI_CB_SLOTS; i++) {
        if (g_zr_ffi_cbs[i].used && g_zr_ffi_cbs[i].code_ptr == ptr && g_zr_ffi_cbs[i].vm == vm) {
            memset(&g_zr_ffi_cbs[i], 0, sizeof(g_zr_ffi_cbs[i]));
            break;
        }
    }
    zr_mutex_unlock(&g_zr_db_ffi_mtx);
    *out = val_nil();
    return true;
}

static void zr_ffi_gc_mark_roots(PbVM *vm) {
    for (int i = 0; i < ZR_FFI_CB_SLOTS; i++) {
        if (g_zr_ffi_cbs[i].used && g_zr_ffi_cbs[i].vm == vm) {
            gc_mark_val(g_zr_ffi_cbs[i].closure);
        }
    }
}

static void zr_ffi_gc_mark_nursery_roots(PbVM *vm) {
    for (int i = 0; i < ZR_FFI_CB_SLOTS; i++) {
        if (g_zr_ffi_cbs[i].used && g_zr_ffi_cbs[i].vm == vm) {
            gc_mark_nursery_val(g_zr_ffi_cbs[i].closure);
        }
    }
}

static void zr_ffi_cleanup_vm(PbVM *vm) {
    if (!g_zr_db_ffi_mtx_init) return;
    zr_mutex_lock(&g_zr_db_ffi_mtx);
    for (int i = 0; i < ZR_FFI_CB_SLOTS; i++) {
        if (g_zr_ffi_cbs[i].used && g_zr_ffi_cbs[i].vm == vm) {
            memset(&g_zr_ffi_cbs[i], 0, sizeof(g_zr_ffi_cbs[i]));
        }
    }
    zr_mutex_unlock(&g_zr_db_ffi_mtx);
}

#endif /* PBLANG_ZR_SQLITE_FFI_H */
