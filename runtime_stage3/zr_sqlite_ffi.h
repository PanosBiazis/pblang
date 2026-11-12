/* ============================================================================
 * pblang Sovereign Zero-Rust Native Runtime v2.0.0 — zr_sqlite_ffi.h
 * Lazy-dlopen SQLite3 parameterized database engine + Guarded C ABI FFI.
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
#define ZR_SQLITE_TRANSIENT  ((void (*)(void *))(-1))

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
    const char *cands[] = { "sqlite3.dll", "winsqlite3.dll", "libsqlite3-0.dll", NULL };
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
    for (int i = 0; cands[i] && !g_zr_sql.lib; i++) g_zr_sql.lib = zr_dlopen(cands[i]);
    if (!g_zr_sql.lib) {
        zr_mutex_unlock(&g_zr_db_ffi_mtx);
        snprintf(err_out, 256, "SQLite3 shared library not found");
        return false;
    }

    #define LOAD_SQL_SYM(fn) g_zr_sql.fn = (__typeof__(g_zr_sql.fn))zr_dlsym(g_zr_sql.lib, #fn)
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

    if (!g_zr_sql.sqlite3_open || !g_zr_sql.sqlite3_close || !g_zr_sql.sqlite3_prepare_v2 ||
        !g_zr_sql.sqlite3_step || !g_zr_sql.sqlite3_finalize) {
        zr_mutex_unlock(&g_zr_db_ffi_mtx);
        snprintf(err_out, 256, "SQLite3 library missing required C symbols");
        return false;
    }
    g_zr_sql.ready = true;
    zr_mutex_unlock(&g_zr_db_ffi_mtx);
    return true;
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
    if (!vm->caps.allow_sqlite) { vm_raise(vm, "sqlite access denied"); return false; }
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
    if (!vm->caps.allow_sqlite) { vm_raise(vm, "sqlite access denied"); return false; }
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
    if (!vm->caps.allow_sqlite) { vm_raise(vm, "sqlite access denied"); return false; }
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
    if (!vm->caps.allow_sqlite) { vm_raise(vm, "sqlite access denied"); return false; }
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
    if (!vm->caps.allow_sqlite) { vm_raise(vm, "sqlite access denied"); return false; }
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

/* ---- Guarded Dynamic C ABI FFI (`ffi_load`, `ffi_close`, `ffi_call`, `ffi_call_typed`) ---- */
typedef struct {
    size_t     id;
    bool       used;
    ZrDlHandle lib;
} ZrFfiEntry;

#define ZR_MAX_FFI_HANDLES 128
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
    if (!vm_check_cap(vm, vm->caps.allow_ffi, "ffi_load", "ffi")) return false;
    if (argc != 1 || !val_is_obj(argv[0], OBJ_STR)) { vm_raise(vm, "ffi_load() expects 1 library path string"); return false; }
    const char *path = ((PbStr *)val_as_obj(argv[0]))->chars;
    if (!vm->caps.ffi_allow_all) {
        vm_raise(vm, "ffi_load: library \"%s\" not in allowlist (pass --allow-ffi / --allow-all)", path);
        return false;
    }
    ZrDlHandle lib = zr_dlopen(path);
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
            g_zr_ffi_table[i].lib = lib;
            stored = true;
            break;
        }
    }
    zr_mutex_unlock(&g_zr_db_ffi_mtx);
    if (!stored) {
        zr_dlclose(lib);
        vm_raise(vm, "ffi_load(): maximum loaded FFI library limit (128) reached");
        return false;
    }
    *out = val_num((double)id);
    return true;
}

static bool bi_ffi_close(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (!vm_check_cap(vm, vm->caps.allow_ffi, "ffi_close", "ffi")) return false;
    if (argc != 1 || argv[0].tag != VAL_NUM || argv[0].u.num < 1.0) {
        vm_raise(vm, "ffi_close() expects 1 numeric handle");
        return false;
    }
    size_t id = (size_t)argv[0].u.num;
    zr_db_ffi_ensure_mtx();
    zr_mutex_lock(&g_zr_db_ffi_mtx);
    ZrDlHandle lib = NULL;
    for (size_t i = 0; i < ZR_MAX_FFI_HANDLES; i++) {
        if (g_zr_ffi_table[i].used && g_zr_ffi_table[i].id == id) {
            lib = g_zr_ffi_table[i].lib;
            g_zr_ffi_table[i].used = false;
            g_zr_ffi_table[i].lib = NULL;
            break;
        }
    }
    zr_mutex_unlock(&g_zr_db_ffi_mtx);
    if (!lib) { vm_raise(vm, "ffi_close(): invalid handle"); return false; }
    zr_dlclose(lib);
    *out = val_nil();
    return true;
}

static bool bi_ffi_call(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (!vm_check_cap(vm, vm->caps.allow_ffi, "ffi_call", "ffi")) return false;
    if (argc != 3 || argv[0].tag != VAL_NUM || !val_is_obj(argv[1], OBJ_STR) || !val_is_obj(argv[2], OBJ_ARRAY)) {
        vm_raise(vm, "ffi_call(handle, symbol, args_array) expects 3 arguments");
        return false;
    }
    ZrDlHandle lib = zr_ffi_lookup((size_t)argv[0].u.num);
    if (!lib) { vm_raise(vm, "ffi_call(): invalid handle"); return false; }
    const char *sym = ((PbStr *)val_as_obj(argv[1]))->chars;
    PbArray *arr = (PbArray *)val_as_obj(argv[2]);
    if (arr->len > 6) { vm_raise(vm, "ffi_call(): max 6 arguments supported"); return false; }
    double a[6] = {0};
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
    }
    *out = val_num(r);
    return true;
}

typedef enum {
    FFI_T_I64 = 0, FFI_T_I32, FFI_T_BOOL, FFI_T_PTR, FFI_T_STR, FFI_T_F64, FFI_T_F32, FFI_T_VOID
} ZrFfiType;

static bool zr_parse_ffi_type_tok(const char *s, size_t len, bool is_ret, ZrFfiType *out) {
    while (len > 0 && (*s == ' ' || *s == '\t')) { s++; len--; }
    while (len > 0 && (s[len - 1] == ' ' || s[len - 1] == '\t')) len--;
    if (len == 3 && memcmp(s, "i64", 3) == 0)    { *out = FFI_T_I64;  return true; }
    if (len == 3 && memcmp(s, "u64", 3) == 0)    { *out = FFI_T_I64;  return true; }
    if (len == 3 && memcmp(s, "i32", 3) == 0)    { *out = FFI_T_I32;  return true; }
    if (len == 3 && memcmp(s, "u32", 3) == 0)    { *out = FFI_T_I32;  return true; }
    if (len == 4 && memcmp(s, "bool", 4) == 0)   { *out = FFI_T_BOOL; return true; }
    if (len == 3 && memcmp(s, "ptr", 3) == 0)    { *out = FFI_T_PTR;  return true; }
    if (len == 6 && memcmp(s, "string", 6) == 0) { *out = FFI_T_STR;  return true; }
    if (len == 3 && memcmp(s, "f64", 3) == 0)    { *out = FFI_T_F64;  return true; }
    if (len == 3 && memcmp(s, "f32", 3) == 0)    { *out = FFI_T_F32;  return true; }
    if (is_ret && len == 4 && memcmp(s, "void", 4) == 0) { *out = FFI_T_VOID; return true; }
    return false;
}

static bool bi_ffi_call_typed(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (!vm_check_cap(vm, vm->caps.allow_ffi, "ffi_call_typed", "ffi")) return false;
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

    const char *arrow = strstr(sig, "->");
    if (!arrow) { vm_raise(vm, "ffi_call_typed(): signature must contain '->'"); return false; }

    ZrFfiType arg_types[6];
    size_t n_sig_args = 0;
    const char *ap = sig;
    while (ap < arrow && (*ap == ' ' || *ap == '\t')) ap++;
    if (ap < arrow) {
        for (;;) {
            if (n_sig_args >= 6) { vm_raise(vm, "ffi_call_typed(): max 6 arguments supported"); return false; }
            const char *comma = (const char *)memchr(ap, ',', (size_t)(arrow - ap));
            size_t tlen = comma ? (size_t)(comma - ap) : (size_t)(arrow - ap);
            if (!zr_parse_ffi_type_tok(ap, tlen, false, &arg_types[n_sig_args])) {
                vm_raise(vm, "ffi_call_typed(): unsupported argument type in signature '%s'", sig);
                return false;
            }
            n_sig_args++;
            if (!comma) break;
            ap = comma + 1;
        }
    }
    ZrFfiType ret_type;
    if (!zr_parse_ffi_type_tok(arrow + 2, strlen(arrow + 2), true, &ret_type)) {
        vm_raise(vm, "ffi_call_typed(): unsupported return type in signature '%s'", sig);
        return false;
    }
    if (args->len != n_sig_args) {
        vm_raise(vm, "ffi_call_typed(): signature expects %zu args, got %zu", n_sig_args, args->len);
        return false;
    }

    void *fp = zr_dlsym(lib, sym);
    if (!fp) { vm_raise(vm, "ffi_call_typed(): symbol \"%s\" not found", sym); return false; }

    /* Marshal arguments into GP (intptr_t/int64_t) and FP (double) slots */
    int64_t gp[6] = {0};
    double  fp_arg[6] = {0};
    bool    any_fp_arg = false;
    bool    all_fp_arg = (n_sig_args > 0);
    for (size_t i = 0; i < n_sig_args; i++) {
        PbValue v = args->items[i];
        ZrFfiType t = arg_types[i];
        if (t == FFI_T_F64 || t == FFI_T_F32) {
            if (v.tag != VAL_NUM) { vm_raise(vm, "ffi_call_typed(): arg %zu expected number", i); return false; }
            fp_arg[i] = (t == FFI_T_F32) ? (double)(float)v.u.num : v.u.num;
            any_fp_arg = true;
        } else {
            all_fp_arg = false;
            if (t == FFI_T_STR) {
                if (v.tag == VAL_NIL) gp[i] = 0;
                else if (val_is_obj(v, OBJ_STR)) gp[i] = (int64_t)(uintptr_t)((PbStr *)val_as_obj(v))->chars;
                else { vm_raise(vm, "ffi_call_typed(): arg %zu expected string", i); return false; }
            } else if (t == FFI_T_BOOL) {
                gp[i] = vm_is_truthy(v) ? 1 : 0;
            } else if (t == FFI_T_I32) {
                if (v.tag != VAL_NUM) { vm_raise(vm, "ffi_call_typed(): arg %zu expected integer", i); return false; }
                gp[i] = (int64_t)(int32_t)(int64_t)v.u.num;
            } else { /* I64 / PTR */
                if (v.tag == VAL_NIL && t == FFI_T_PTR) gp[i] = 0;
                else if (v.tag == VAL_NUM) gp[i] = (int64_t)v.u.num;
                else { vm_raise(vm, "ffi_call_typed(): arg %zu expected number/pointer", i); return false; }
            }
        }
    }

    bool ret_is_fp = (ret_type == FFI_T_F64 || ret_type == FFI_T_F32);
    int64_t raw_i = 0;
    double  raw_d = 0.0;

    if (!any_fp_arg) {
        /* 0..6 GP-register / pointer / string arguments (covers 100% of libc, crypto, sqlite, POSIX/Win32 syscalls) */
        if (ret_type == FFI_T_VOID) {
            switch (n_sig_args) {
                case 0: ((void (*)(void))fp)(); break;
                case 1: ((void (*)(int64_t))fp)(gp[0]); break;
                case 2: ((void (*)(int64_t, int64_t))fp)(gp[0], gp[1]); break;
                case 3: ((void (*)(int64_t, int64_t, int64_t))fp)(gp[0], gp[1], gp[2]); break;
                case 4: ((void (*)(int64_t, int64_t, int64_t, int64_t))fp)(gp[0], gp[1], gp[2], gp[3]); break;
                case 5: ((void (*)(int64_t, int64_t, int64_t, int64_t, int64_t))fp)(gp[0], gp[1], gp[2], gp[3], gp[4]); break;
                case 6: ((void (*)(int64_t, int64_t, int64_t, int64_t, int64_t, int64_t))fp)(gp[0], gp[1], gp[2], gp[3], gp[4], gp[5]); break;
            }
        } else if (ret_type == FFI_T_F64) {
            switch (n_sig_args) {
                case 0: raw_d = ((double (*)(void))fp)(); break;
                case 1: raw_d = ((double (*)(int64_t))fp)(gp[0]); break;
                case 2: raw_d = ((double (*)(int64_t, int64_t))fp)(gp[0], gp[1]); break;
                case 3: raw_d = ((double (*)(int64_t, int64_t, int64_t))fp)(gp[0], gp[1], gp[2]); break;
                case 4: raw_d = ((double (*)(int64_t, int64_t, int64_t, int64_t))fp)(gp[0], gp[1], gp[2], gp[3]); break;
                case 5: raw_d = ((double (*)(int64_t, int64_t, int64_t, int64_t, int64_t))fp)(gp[0], gp[1], gp[2], gp[3], gp[4]); break;
                case 6: raw_d = ((double (*)(int64_t, int64_t, int64_t, int64_t, int64_t, int64_t))fp)(gp[0], gp[1], gp[2], gp[3], gp[4], gp[5]); break;
            }
        } else if (ret_type == FFI_T_F32) {
            switch (n_sig_args) {
                case 0: raw_d = (double)((float (*)(void))fp)(); break;
                case 1: raw_d = (double)((float (*)(int64_t))fp)(gp[0]); break;
                case 2: raw_d = (double)((float (*)(int64_t, int64_t))fp)(gp[0], gp[1]); break;
                default: raw_d = (double)((float (*)(int64_t, int64_t, int64_t, int64_t))fp)(gp[0], gp[1], gp[2], gp[3]); break;
            }
        } else {
            switch (n_sig_args) {
                case 0: raw_i = ((int64_t (*)(void))fp)(); break;
                case 1: raw_i = ((int64_t (*)(int64_t))fp)(gp[0]); break;
                case 2: raw_i = ((int64_t (*)(int64_t, int64_t))fp)(gp[0], gp[1]); break;
                case 3: raw_i = ((int64_t (*)(int64_t, int64_t, int64_t))fp)(gp[0], gp[1], gp[2]); break;
                case 4: raw_i = ((int64_t (*)(int64_t, int64_t, int64_t, int64_t))fp)(gp[0], gp[1], gp[2], gp[3]); break;
                case 5: raw_i = ((int64_t (*)(int64_t, int64_t, int64_t, int64_t, int64_t))fp)(gp[0], gp[1], gp[2], gp[3], gp[4]); break;
                case 6: raw_i = ((int64_t (*)(int64_t, int64_t, int64_t, int64_t, int64_t, int64_t))fp)(gp[0], gp[1], gp[2], gp[3], gp[4], gp[5]); break;
            }
        }
    } else if (all_fp_arg) {
        if (ret_is_fp) {
            switch (n_sig_args) {
                case 1: raw_d = ((double (*)(double))fp)(fp_arg[0]); break;
                case 2: raw_d = ((double (*)(double, double))fp)(fp_arg[0], fp_arg[1]); break;
                case 3: raw_d = ((double (*)(double, double, double))fp)(fp_arg[0], fp_arg[1], fp_arg[2]); break;
                case 4: raw_d = ((double (*)(double, double, double, double))fp)(fp_arg[0], fp_arg[1], fp_arg[2], fp_arg[3]); break;
                case 5: raw_d = ((double (*)(double, double, double, double, double))fp)(fp_arg[0], fp_arg[1], fp_arg[2], fp_arg[3], fp_arg[4]); break;
                case 6: raw_d = ((double (*)(double, double, double, double, double, double))fp)(fp_arg[0], fp_arg[1], fp_arg[2], fp_arg[3], fp_arg[4], fp_arg[5]); break;
            }
        } else {
            switch (n_sig_args) {
                case 1: raw_i = ((int64_t (*)(double))fp)(fp_arg[0]); break;
                case 2: raw_i = ((int64_t (*)(double, double))fp)(fp_arg[0], fp_arg[1]); break;
                default: raw_i = ((int64_t (*)(double, double, double, double))fp)(fp_arg[0], fp_arg[1], fp_arg[2], fp_arg[3]); break;
            }
        }
    } else if (n_sig_args == 2 && (arg_types[0] != FFI_T_F64 && arg_types[0] != FFI_T_F32)) {
        if (ret_is_fp) raw_d = ((double (*)(int64_t, double))fp)(gp[0], fp_arg[1]);
        else raw_i = ((int64_t (*)(int64_t, double))fp)(gp[0], fp_arg[1]);
    } else if (n_sig_args == 2) {
        if (ret_is_fp) raw_d = ((double (*)(double, int64_t))fp)(fp_arg[0], gp[1]);
        else raw_i = ((int64_t (*)(double, int64_t))fp)(fp_arg[0], gp[1]);
    } else {
        vm_raise(vm, "ffi_call_typed(): mixed GP/FP signature '%s' not supported", sig);
        return false;
    }

    switch (ret_type) {
        case FFI_T_VOID: *out = val_nil(); break;
        case FFI_T_F64:
        case FFI_T_F32:  *out = val_num(raw_d); break;
        case FFI_T_BOOL: *out = val_bool((raw_i & 0xff) != 0); break;
        case FFI_T_I32:  *out = val_num((double)(int32_t)raw_i); break;
        case FFI_T_I64:  *out = val_num((double)raw_i); break;
        case FFI_T_PTR:  *out = val_num((double)(uintptr_t)raw_i); break;
        case FFI_T_STR: {
            const char *cs = (const char *)(uintptr_t)raw_i;
            *out = cs ? val_obj(vm_new_cstr(vm, cs)) : val_nil();
            break;
        }
    }
    return true;
}

#endif /* PBLANG_ZR_SQLITE_FFI_H */
