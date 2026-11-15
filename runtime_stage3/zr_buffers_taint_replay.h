/* ============================================================================
 * pblang Sovereign Zero-Rust Native Runtime v3.0.0 — zr_buffers_taint_replay.h
 * Zero-Copy Shared Buffers (OBJ_BYTES / OBJ_F64BUF), VM Taint-Tracking Guards,
 * Per-Module Capability Map Parsing, Deterministic Flight-Recorder & Replay,
 * and VM Hotspot Profiler Reporting (Table / JSON / Flamegraph SVG).
 * ============================================================================ */
#ifndef PBLANG_ZR_BUFFERS_TAINT_REPLAY_H
#define PBLANG_ZR_BUFFERS_TAINT_REPLAY_H

#include "zr_vm_core.h"

/* ============================================================================
 * 1. Zero-Copy Typed Shared Buffers (`OBJ_BYTES`, `OBJ_F64BUF`)
 * ============================================================================ */

#ifndef IS_NIL
#define IS_NIL(v)    ((v).tag == VAL_NIL)
#define IS_BOOL(v)   ((v).tag == VAL_BOOL)
#define IS_NUM(v)    ((v).tag == VAL_NUM)
#define IS_STR(v)    is_obj_type((v), OBJ_STR)
#define IS_ARRAY(v)  is_obj_type((v), OBJ_ARRAY)
#define IS_MAP(v)    is_obj_type((v), OBJ_MAP)
#define IS_BYTES(v)  is_obj_type((v), OBJ_BYTES)
#define IS_F64BUF(v) is_obj_type((v), OBJ_F64BUF)
#endif

static bool bi_bytes_new(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc < 1 || argc > 2) {
        return vm_error(vm, "bytes_new(n_or_arr[, shared_bool]) expects 1 or 2 arguments");
    }
    bool shared = (argc >= 2) ? val_truthy(argv[1]) : true;
    if (IS_NUM(argv[0])) {
        int64_t n = (int64_t)argv[0].as.num;
        if (n < 0 || (uint64_t)n > (uint64_t)536870912u) {
            return vm_error(vm, "bytes_new(): length %lld out of range", (long long)n);
        }
        PbBytes *b = pb_bytes_new(vm, (size_t)n, shared);
        *out = val_obj(b);
        return true;
    }
    if (IS_ARRAY(argv[0])) {
        PbArray *arr = AS_ARRAY(argv[0]);
        PbBytes *b = pb_bytes_new(vm, arr->len, shared);
        uint8_t taint = argv[0].taint;
        for (size_t i = 0; i < arr->len; i++) {
            PbValue el = arr->items[i];
            if (!IS_NUM(el)) {
                return vm_error(vm, "bytes_new(): array element %zu is not a number", i);
            }
            taint |= el.taint;
            b->data[i] = (uint8_t)(((int64_t)el.as.num) & 0xFF);
        }
        *out = val_with_taint(val_obj(b), taint);
        return true;
    }
    if (IS_BYTES(argv[0])) {
        PbBytes *src = AS_BYTES(argv[0]);
        PbBytes *b = pb_bytes_new(vm, src->len, shared);
        if (src->len > 0) memcpy(b->data, src->data, src->len);
        *out = val_with_taint(val_obj(b), argv[0].taint);
        return true;
    }
    if (IS_STR(argv[0])) {
        PbStr *s = AS_STR(argv[0]);
        PbBytes *b = pb_bytes_new(vm, s->len, shared);
        if (s->len > 0) memcpy(b->data, s->chars, s->len);
        *out = val_with_taint(val_obj(b), argv[0].taint);
        return true;
    }
    return vm_error(vm, "bytes_new() expects byte count number or array");
}

static bool bi_bytes_from_str(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 1 || !IS_STR(argv[0])) {
        return vm_error(vm, "bytes_from_str(s) expects 1 string");
    }
    PbStr *s = AS_STR(argv[0]);
    PbBytes *b = pb_bytes_new(vm, s->len, true);
    if (s->len > 0) memcpy(b->data, s->chars, s->len);
    *out = val_with_taint(val_obj(b), argv[0].taint);
    return true;
}

static bool bi_bytes_get(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 2 || !IS_NUM(argv[1])) {
        return vm_error(vm, "bytes_get(buf, idx) expects (bytes, number)");
    }
    int64_t idx = (int64_t)argv[1].as.num;
    if (IS_BYTES(argv[0])) {
        PbBytes *b = AS_BYTES(argv[0]);
        if (idx < 0 || (size_t)idx >= b->len) {
            return vm_error(vm, "bytes_get(): index %lld out of bounds (len %zu)", (long long)idx, b->len);
        }
        *out = val_num((double)b->data[(size_t)idx]);
        return true;
    }
    if (IS_ARRAY(argv[0])) {
        PbArray *a = AS_ARRAY(argv[0]);
        if (idx < 0 || (size_t)idx >= a->len) {
            return vm_error(vm, "bytes_get(): index %lld out of bounds (len %zu)", (long long)idx, a->len);
        }
        PbValue el = a->items[(size_t)idx];
        if (!IS_NUM(el)) return vm_error(vm, "bytes_get(): array element is not numeric");
        *out = val_num((double)(((int64_t)el.as.num) & 0xFF));
        return true;
    }
    return vm_error(vm, "bytes_get() expects bytes buffer as argument 1");
}

static bool bi_bytes_set(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 3 || !IS_NUM(argv[1]) || !IS_NUM(argv[2])) {
        return vm_error(vm, "bytes_set(buf, idx, byte) expects (bytes, number, number)");
    }
    int64_t idx = (int64_t)argv[1].as.num;
    uint8_t byte_val = (uint8_t)(((int64_t)argv[2].as.num) & 0xFF);
    if (IS_BYTES(argv[0])) {
        PbBytes *b = AS_BYTES(argv[0]);
        if (idx < 0 || (size_t)idx >= b->len) {
            return vm_error(vm, "bytes_set(): index %lld out of bounds (len %zu)", (long long)idx, b->len);
        }
        b->data[(size_t)idx] = byte_val;
        *out = argv[0];
        return true;
    }
    if (IS_ARRAY(argv[0])) {
        PbArray *a = AS_ARRAY(argv[0]);
        if (idx < 0 || (size_t)idx >= a->len) {
            return vm_error(vm, "bytes_set(): index %lld out of bounds (len %zu)", (long long)idx, a->len);
        }
        a->items[(size_t)idx] = val_num((double)byte_val);
        *out = argv[0];
        return true;
    }
    return vm_error(vm, "bytes_set() expects bytes buffer as argument 1");
}

static bool bi_bytes_slice(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc < 2 || argc > 3 || !IS_BYTES(argv[0]) || !IS_NUM(argv[1]) || (argc == 3 && !IS_NUM(argv[2]))) {
        return vm_error(vm, "bytes_slice(buf, start[, end]) expects (bytes, number[, number])");
    }
    PbBytes *b = AS_BYTES(argv[0]);
    int64_t len = (int64_t)b->len;
    int64_t s = (int64_t)argv[1].as.num;
    int64_t e = (argc == 3) ? (int64_t)argv[2].as.num : len;
    if (s < 0) s += len;
    if (e < 0) e += len;
    if (s < 0) s = 0;
    if (s > len) s = len;
    if (e < s) e = s;
    if (e > len) e = len;
    size_t slen = (size_t)(e - s);
    PbBytes *res = pb_bytes_new(vm, slen, true);
    if (slen > 0) memcpy(res->data, b->data + s, slen);
    *out = val_with_taint(val_obj(res), argv[0].taint);
    return true;
}

static bool bi_f64buf_new(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc < 1 || argc > 2) {
        return vm_error(vm, "f64buf_new(n_or_arr[, shared_bool]) expects 1 or 2 arguments");
    }
    bool shared = (argc >= 2) ? val_truthy(argv[1]) : true;
    if (IS_NUM(argv[0])) {
        int64_t n = (int64_t)argv[0].as.num;
        if (n < 0 || (uint64_t)n > (uint64_t)134217728u) {
            return vm_error(vm, "f64buf_new(): length %lld out of range", (long long)n);
        }
        PbF64Buf *b = pb_f64buf_new(vm, (size_t)n, shared);
        *out = val_obj(b);
        return true;
    }
    if (IS_ARRAY(argv[0])) {
        PbArray *arr = AS_ARRAY(argv[0]);
        PbF64Buf *b = pb_f64buf_new(vm, arr->len, shared);
        for (size_t i = 0; i < arr->len; i++) {
            PbValue el = arr->items[i];
            if (!IS_NUM(el)) {
                return vm_error(vm, "f64buf_new(): array element %zu is not a number", i);
            }
            b->data[i] = el.as.num;
        }
        *out = val_obj(b);
        return true;
    }
    if (IS_F64BUF(argv[0])) {
        PbF64Buf *src = AS_F64BUF(argv[0]);
        PbF64Buf *b = pb_f64buf_new(vm, src->len, shared);
        if (src->len > 0) memcpy(b->data, src->data, src->len * sizeof(double));
        *out = val_obj(b);
        return true;
    }
    return vm_error(vm, "f64buf_new() expects element count number or numeric array");
}

static bool bi_f64buf_get(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 2 || !IS_F64BUF(argv[0]) || !IS_NUM(argv[1])) {
        return vm_error(vm, "f64buf_get(buf, idx) expects (f64buf, number)");
    }
    PbF64Buf *b = AS_F64BUF(argv[0]);
    int64_t idx = (int64_t)argv[1].as.num;
    if (idx < 0 || (size_t)idx >= b->len) {
        return vm_error(vm, "f64buf_get(): index %lld out of bounds (len %zu)", (long long)idx, b->len);
    }
    *out = val_num(b->data[(size_t)idx]);
    return true;
}

static bool bi_f64buf_set(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 3 || !IS_F64BUF(argv[0]) || !IS_NUM(argv[1]) || !IS_NUM(argv[2])) {
        return vm_error(vm, "f64buf_set(buf, idx, val) expects (f64buf, number, number)");
    }
    PbF64Buf *b = AS_F64BUF(argv[0]);
    int64_t idx = (int64_t)argv[1].as.num;
    if (idx < 0 || (size_t)idx >= b->len) {
        return vm_error(vm, "f64buf_set(): index %lld out of bounds (len %zu)", (long long)idx, b->len);
    }
    b->data[(size_t)idx] = argv[2].as.num;
    *out = argv[0];
    return true;
}

static bool bi_f64buf_sum(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 1 || !IS_F64BUF(argv[0])) {
        return vm_error(vm, "f64buf_sum(buf) expects 1 f64buf");
    }
    PbF64Buf *b = AS_F64BUF(argv[0]);
    const double *p = b->data;
    size_t n = b->len;
    size_t i = 0;
    double s0 = 0.0, s1 = 0.0, s2 = 0.0, s3 = 0.0;
    for (; i + 4 <= n; i += 4) {
        s0 += p[i];
        s1 += p[i + 1];
        s2 += p[i + 2];
        s3 += p[i + 3];
    }
    double total = (s0 + s1) + (s2 + s3);
    for (; i < n; i++) total += p[i];
    *out = val_num(total);
    return true;
}

static bool bi_f64buf_dot(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 2 || !IS_F64BUF(argv[0]) || !IS_F64BUF(argv[1])) {
        return vm_error(vm, "f64buf_dot(a, b) expects 2 f64bufs");
    }
    PbF64Buf *a = AS_F64BUF(argv[0]);
    PbF64Buf *b = AS_F64BUF(argv[1]);
    if (a->len != b->len) {
        return vm_error(vm, "f64buf_dot(): length mismatch (%zu vs %zu)", a->len, b->len);
    }
    const double *pa = a->data;
    const double *pb = b->data;
    size_t n = a->len;
    size_t i = 0;
    double s0 = 0.0, s1 = 0.0, s2 = 0.0, s3 = 0.0;
    for (; i + 4 <= n; i += 4) {
        s0 += pa[i] * pb[i];
        s1 += pa[i + 1] * pb[i + 1];
        s2 += pa[i + 2] * pb[i + 2];
        s3 += pa[i + 3] * pb[i + 3];
    }
    double total = (s0 + s1) + (s2 + s3);
    for (; i < n; i++) total += pa[i] * pb[i];
    *out = val_num(total);
    return true;
}

static bool bi_is_bytes(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    (void)vm;
    *out = val_bool(argc >= 1 && IS_BYTES(argv[0]));
    return true;
}

static bool bi_is_f64buf(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    (void)vm;
    *out = val_bool(argc >= 1 && IS_F64BUF(argv[0]));
    return true;
}

/* ============================================================================
 * 2. VM-Level Taint Tracking (`TAINT_SECRET`, `TAINT_PII`)
 * ============================================================================ */

static uint8_t zr_collect_value_taint(PbValue v, int depth) {
    uint8_t t = v.taint;
    if (depth > 8 || v.tag != VAL_OBJ || !v.as.obj) return t;
    if (v.as.obj->type == OBJ_ARRAY) {
        PbArray *a = (PbArray *)v.as.obj;
        for (size_t i = 0; i < a->len; i++) {
            t |= zr_collect_value_taint(a->items[i], depth + 1);
            if (t == (TAINT_SECRET | TAINT_PII)) break;
        }
    } else if (v.as.obj->type == OBJ_MAP) {
        PbMap *m = (PbMap *)v.as.obj;
        for (size_t i = 0; i < m->used; i++) {
            if (m->entries[i].active && m->entries[i].key) {
                t |= zr_collect_value_taint(m->entries[i].val, depth + 1);
                if (t == (TAINT_SECRET | TAINT_PII)) break;
            }
        }
    }
    return t;
}

static const char *zr_taint_label(uint8_t t) {
    uint8_t m = t & (TAINT_SECRET | TAINT_PII);
    if (m == (TAINT_SECRET | TAINT_PII)) return "secret+pii";
    if (m == TAINT_SECRET) return "secret";
    if (m == TAINT_PII) return "pii";
    return "clean";
}

static bool zr_check_taint_sink(PbVM *vm, PbValue v, const char *sink_name) {
    if (!vm->taint_guard_enabled) return true;
    uint8_t t = zr_collect_value_taint(v, 0);
    if (t != 0) {
        return vm_error(vm,
            "Taint violation: attempted to output unredacted TAINT_SECRET/TAINT_PII value (%s) in %s",
            zr_taint_label(t), sink_name ? sink_name : "sink");
    }
    return true;
}

static bool zr_env_name_is_sensitive(const char *name) {
    if (!name || !*name) return false;
    return zr_str_contains_ci_ascii(name, "KEY") ||
           zr_str_contains_ci_ascii(name, "TOKEN") ||
           zr_str_contains_ci_ascii(name, "SECRET") ||
           zr_str_contains_ci_ascii(name, "PASSWORD") ||
           zr_str_contains_ci_ascii(name, "CRN");
}

static bool bi_taint_secret(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 1) return vm_error(vm, "taint_secret(v) expects 1 argument");
    *out = val_with_taint(argv[0], (uint8_t)(argv[0].taint | TAINT_SECRET));
    return true;
}

static bool bi_taint_pii(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 1) return vm_error(vm, "taint_pii(v) expects 1 argument");
    *out = val_with_taint(argv[0], (uint8_t)(argv[0].taint | TAINT_PII));
    return true;
}

static bool bi_taint_get(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 1) return vm_error(vm, "taint_get(v) expects 1 argument");
    uint8_t t = zr_collect_value_taint(argv[0], 0);
    *out = val_obj(pb_str_c(vm, zr_taint_label(t)));
    return true;
}

static bool bi_is_tainted(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 1) return vm_error(vm, "is_tainted(v) expects 1 argument");
    *out = val_bool(zr_collect_value_taint(argv[0], 0) != 0);
    return true;
}

static PbValue zr_sanitize_value_recursive(PbVM *vm, PbValue v, int depth) {
    v.taint = TAINT_CLEAN;
    if (depth > 8 || v.tag != VAL_OBJ || !v.as.obj) return v;
    if (v.as.obj->type == OBJ_ARRAY) {
        PbArray *a = (PbArray *)v.as.obj;
        for (size_t i = 0; i < a->len; i++) {
            a->items[i] = zr_sanitize_value_recursive(vm, a->items[i], depth + 1);
        }
    } else if (v.as.obj->type == OBJ_MAP) {
        PbMap *m = (PbMap *)v.as.obj;
        for (size_t i = 0; i < m->used; i++) {
            if (m->entries[i].active && m->entries[i].key) {
                m->entries[i].val = zr_sanitize_value_recursive(vm, m->entries[i].val, depth + 1);
            }
        }
    }
    return v;
}

static bool bi_sanitize_taint(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 1) return vm_error(vm, "sanitize_taint(v) expects 1 argument");
    *out = zr_sanitize_value_recursive(vm, argv[0], 0);
    return true;
}

static bool bi_redact_secret(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 1) return vm_error(vm, "redact_secret(v) expects 1 argument");
    if (!IS_STR(argv[0])) {
        *out = val_obj(pb_str_c(vm, "***"));
        return true;
    }
    PbStr *s = AS_STR(argv[0]);
    if (s->len <= 6) {
        *out = val_obj(pb_str_c(vm, "***"));
        return true;
    }
    char buf[16];
    memcpy(buf, s->chars, 2);
    memcpy(buf + 2, "***", 3);
    memcpy(buf + 5, s->chars + s->len - 2, 2);
    buf[7] = '\0';
    *out = val_obj(pb_str_new(vm, buf, 7));
    return true;
}

static bool bi_vm_set_taint_guard(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 1) return vm_error(vm, "vm_set_taint_guard(enabled) expects 1 boolean");
    bool prev = vm->taint_guard_enabled;
    vm->taint_guard_enabled = val_truthy(argv[0]);
    *out = val_bool(prev);
    return true;
}

/* ============================================================================
 * 3. Per-Module Capability Map Helper (`caps{ ... }`)
 * ============================================================================ */

static uint32_t zr_cap_bit_from_key(const char *k) {
    if (!k) return 0;
    if (strcmp(k, "fs") == 0)     return CAP_FS;
    if (strcmp(k, "net") == 0)    return CAP_NET;
    if (strcmp(k, "http") == 0)   return CAP_HTTP;
    if (strcmp(k, "sqlite") == 0) return CAP_SQLITE;
    if (strcmp(k, "ffi") == 0)    return CAP_FFI;
    if (strcmp(k, "exec") == 0)   return CAP_EXEC;
    if (strcmp(k, "gfx") == 0)    return CAP_GFX;
    if (strcmp(k, "spawn") == 0)  return CAP_SPAWN;
    if (strcmp(k, "env") == 0)    return CAP_ENV;
    if (strcmp(k, "all") == 0)    return CAP_ALL;
    return 0;
}

static bool zr_cap_val_is_deny(PbValue v) {
    if (IS_NIL(v)) return true;
    if (IS_BOOL(v)) return !v.as.b;
    if (IS_NUM(v)) return v.as.num == 0.0;
    if (IS_STR(v)) {
        const char *s = AS_STR(v)->chars;
        return zr_strcasecmp_ascii(s, "none") == 0 ||
               zr_strcasecmp_ascii(s, "deny") == 0 ||
               zr_strcasecmp_ascii(s, "false") == 0 ||
               zr_strcasecmp_ascii(s, "off") == 0 ||
               strcmp(s, "0") == 0;
    }
    return false;
}

static bool zr_parse_caps_map(PbVM *vm, PbValue map_val, uint32_t *out_mask) {
    if (!IS_MAP(map_val)) {
        return vm_error(vm, "capabilities specification must be a map");
    }
    PbMap *m = AS_MAP(map_val);
    if (m->len == 0) {
        *out_mask = 0u;
        return true;
    }
    bool has_positive = false;
    bool has_deny = false;
    bool explicit_http = false;
    for (size_t i = 0; i < m->used; i++) {
        if (!m->entries[i].active || !m->entries[i].key) continue;
        const char *k = m->entries[i].key->chars;
        if (strcmp(k, "http") == 0) explicit_http = true;
        if (zr_cap_val_is_deny(m->entries[i].val)) has_deny = true;
        else has_positive = true;
    }
    /*
     * Support both allowlist style (`caps{ fs: true }`) and denylist style
     * (`caps{ fs: "none", net: "none" }`):
     * - Pure positive grants start from 0 and enable only listed capabilities.
     * - Any explicit "none"/false entry starts from CAP_ALL and strips denied bits
     *   (while keeping explicitly granted bits).
     */
    uint32_t mask = (has_positive && !has_deny) ? 0u : CAP_ALL;
    for (size_t i = 0; i < m->used; i++) {
        if (!m->entries[i].active || !m->entries[i].key) continue;
        const char *k = m->entries[i].key->chars;
        uint32_t bit = zr_cap_bit_from_key(k);
        if (bit == 0) {
            return vm_error(vm, "unknown capability '%s' in caps map", k);
        }
        bool deny = zr_cap_val_is_deny(m->entries[i].val);
        if (deny) {
            mask &= ~bit;
            if (bit == CAP_NET && !explicit_http) {
                mask &= ~CAP_HTTP;
            }
        } else {
            mask |= bit;
            if (bit == CAP_NET && !explicit_http && (has_positive && !has_deny)) {
                mask |= CAP_HTTP;
            }
        }
    }
    *out_mask = mask;
    return true;
}

/* ============================================================================
 * 4. Deterministic Flight-Recorder & Time-Travel Replay (`.pbrec` JSONL)
 * ============================================================================ */

static void zr_flight_free_events(PbVM *vm) {
    if (vm->flight_events) {
        for (size_t i = 0; i < vm->flight_event_count; i++) {
            free(vm->flight_events[i].str_val);
        }
        free(vm->flight_events);
        vm->flight_events = NULL;
    }
    vm->flight_event_count = 0;
    vm->flight_replay_cursor = 0;
}

static void zr_flight_cleanup_vm(PbVM *vm) {
    if (vm->flight_rec_fp) {
        fflush(vm->flight_rec_fp);
        fclose(vm->flight_rec_fp);
        vm->flight_rec_fp = NULL;
    }
    vm->flight_recording = false;
    vm->flight_replaying = false;
    zr_flight_free_events(vm);
}

static void zr_flight_write_json_str(FILE *fp, const char *s, size_t len) {
    fputc('"', fp);
    for (size_t i = 0; i < len; i++) {
        unsigned char c = (unsigned char)s[i];
        switch (c) {
            case '"':  fputs("\\\"", fp); break;
            case '\\': fputs("\\\\", fp); break;
            case '\n': fputs("\\n", fp); break;
            case '\r': fputs("\\r", fp); break;
            case '\t': fputs("\\t", fp); break;
            default:
                if (c < 0x20) fprintf(fp, "\\u%04x", (unsigned)c);
                else fputc((int)c, fp);
                break;
        }
    }
    fputc('"', fp);
}

static void zr_flight_record_num(PbVM *vm, const char *fn_name, double val) {
    if (!vm->flight_recording || !vm->flight_rec_fp) return;
    uint64_t seq = vm->flight_seq++;
    fprintf(vm->flight_rec_fp, "{\"seq\":%llu,\"fn\":\"%s\",\"type\":\"num\",\"ret\":%.17g}\n",
            (unsigned long long)seq, fn_name, val);
    fflush(vm->flight_rec_fp);
}

static void zr_flight_record_nil(PbVM *vm, const char *fn_name) {
    if (!vm->flight_recording || !vm->flight_rec_fp) return;
    uint64_t seq = vm->flight_seq++;
    fprintf(vm->flight_rec_fp, "{\"seq\":%llu,\"fn\":\"%s\",\"type\":\"nil\",\"ret\":null}\n",
            (unsigned long long)seq, fn_name);
    fflush(vm->flight_rec_fp);
}

static void zr_flight_record_str(PbVM *vm, const char *fn_name, const char *type_tag, const char *s, size_t len) {
    if (!vm->flight_recording || !vm->flight_rec_fp) return;
    uint64_t seq = vm->flight_seq++;
    fprintf(vm->flight_rec_fp, "{\"seq\":%llu,\"fn\":\"%s\",\"type\":\"%s\",\"ret\":",
            (unsigned long long)seq, fn_name, type_tag ? type_tag : "str");
    zr_flight_write_json_str(vm->flight_rec_fp, s ? s : "", len);
    fputs("}\n", vm->flight_rec_fp);
    fflush(vm->flight_rec_fp);
}

static bool zr_flight_replay_next(PbVM *vm, const char *expected_fn, const ZrFlightEvent **out_ev) {
    if (vm->flight_replay_cursor >= vm->flight_event_count) {
        return vm_error(vm,
            "Flight replay divergence: trace exhausted at seq %zu while calling '%s'",
            vm->flight_replay_cursor, expected_fn);
    }
    const ZrFlightEvent *ev = &vm->flight_events[vm->flight_replay_cursor];
    if (strcmp(ev->fn, expected_fn) != 0) {
        return vm_error(vm,
            "Flight replay divergence at seq %llu: trace expected '%s', program called '%s'",
            (unsigned long long)ev->seq, ev->fn, expected_fn);
    }
    vm->flight_replay_cursor++;
    *out_ev = ev;
    return true;
}

static const char *zr_pbrec_skip_ws(const char *p) {
    while (*p == ' ' || *p == '\t' || *p == '\r' || *p == '\n') p++;
    return p;
}

static bool zr_pbrec_parse_json_str(const char **pp, char **out_s, size_t *out_len) {
    const char *p = zr_pbrec_skip_ws(*pp);
    if (*p != '"') return false;
    p++;
    ZrBuf b;
    zr_buf_init(&b);
    while (*p && *p != '"') {
        if (*p == '\\') {
            p++;
            if (!*p) { zr_buf_free(&b); return false; }
            switch (*p) {
                case '"':  zr_buf_push_u8(&b, '"'); p++; break;
                case '\\': zr_buf_push_u8(&b, '\\'); p++; break;
                case '/':  zr_buf_push_u8(&b, '/'); p++; break;
                case 'n':  zr_buf_push_u8(&b, '\n'); p++; break;
                case 'r':  zr_buf_push_u8(&b, '\r'); p++; break;
                case 't':  zr_buf_push_u8(&b, '\t'); p++; break;
                case 'u': {
                    p++;
                    unsigned code = 0;
                    for (int k = 0; k < 4; k++) {
                        char hc = *p;
                        if (!hc) { zr_buf_free(&b); return false; }
                        code <<= 4;
                        if (hc >= '0' && hc <= '9') code |= (unsigned)(hc - '0');
                        else if (hc >= 'a' && hc <= 'f') code |= (unsigned)(hc - 'a' + 10);
                        else if (hc >= 'A' && hc <= 'F') code |= (unsigned)(hc - 'A' + 10);
                        else { zr_buf_free(&b); return false; }
                        p++;
                    }
                    zr_buf_push_u8(&b, (uint8_t)(code & 0xFFu));
                    break;
                }
                default:
                    zr_buf_push_u8(&b, (uint8_t)*p++);
                    break;
            }
        } else {
            zr_buf_push_u8(&b, (uint8_t)*p++);
        }
    }
    if (*p != '"') { zr_buf_free(&b); return false; }
    p++;
    *pp = p;
    size_t n = 0;
    *out_s = zr_buf_take(&b, &n);
    *out_len = n;
    return true;
}

static bool zr_pbrec_parse_line(const char *line, ZrFlightEvent *ev) {
    memset(ev, 0, sizeof(*ev));
    const char *p = zr_pbrec_skip_ws(line);
    if (*p != '{') return false;
    p++;
    while (*p && *p != '}') {
        p = zr_pbrec_skip_ws(p);
        if (*p == ',') { p++; continue; }
        if (*p == '}') break;
        char *key = NULL;
        size_t klen = 0;
        if (!zr_pbrec_parse_json_str(&p, &key, &klen)) return false;
        p = zr_pbrec_skip_ws(p);
        if (*p != ':') { free(key); return false; }
        p = zr_pbrec_skip_ws(p + 1);

        if (strcmp(key, "seq") == 0) {
            char *endp = NULL;
            ev->seq = (uint64_t)strtoull(p, &endp, 10);
            if (endp == p) { free(key); return false; }
            p = endp;
        } else if (strcmp(key, "fn") == 0) {
            char *sv = NULL; size_t sl = 0;
            if (!zr_pbrec_parse_json_str(&p, &sv, &sl)) { free(key); return false; }
            snprintf(ev->fn, sizeof(ev->fn), "%s", sv);
            free(sv);
        } else if (strcmp(key, "type") == 0) {
            char *sv = NULL; size_t sl = 0;
            if (!zr_pbrec_parse_json_str(&p, &sv, &sl)) { free(key); return false; }
            snprintf(ev->type, sizeof(ev->type), "%s", sv);
            free(sv);
        } else if (strcmp(key, "ret") == 0) {
            if (strncmp(p, "null", 4) == 0) {
                ev->is_nil = true;
                p += 4;
            } else if (*p == '"') {
                if (!zr_pbrec_parse_json_str(&p, &ev->str_val, &ev->str_len)) {
                    free(key);
                    return false;
                }
            } else {
                char *endp = NULL;
                ev->num_val = strtod(p, &endp);
                if (endp == p) { free(key); return false; }
                p = endp;
            }
        } else {
            /* Skip unknown field */
            if (*p == '"') {
                char *tmp = NULL; size_t tl = 0;
                if (!zr_pbrec_parse_json_str(&p, &tmp, &tl)) { free(key); return false; }
                free(tmp);
            } else {
                while (*p && *p != ',' && *p != '}') p++;
            }
        }
        free(key);
    }
    return ev->fn[0] != '\0';
}

static bool zr_flight_start_record_file(PbVM *vm, const char *path) {
    if (vm->flight_replaying) {
        return vm_error(vm, "cannot start flight recording while replay is active");
    }
    if (vm->flight_rec_fp) {
        fclose(vm->flight_rec_fp);
        vm->flight_rec_fp = NULL;
    }
    FILE *fp = fopen(path, "wb");
    if (!fp) {
        return vm_error(vm, "flight_record_start(): cannot open '%s' for writing", path);
    }
    vm->flight_rec_fp = fp;
    vm->flight_recording = true;
    vm->flight_seq = 0;
    return true;
}

static bool zr_flight_start_replay_file(PbVM *vm, const char *path) {
    if (vm->flight_recording) {
        return vm_error(vm, "cannot start flight replay while recording is active");
    }
    FILE *fp = fopen(path, "rb");
    if (!fp) {
        return vm_error(vm, "flight_replay_start(): cannot open '%s'", path);
    }
    zr_flight_free_events(vm);
    size_t cap = 0;
    size_t line_cap = 4096;
    char *line = (char *)zr_xmalloc(line_cap);
    size_t len = 0;
    int ch;
    bool ok = true;
    while ((ch = fgetc(fp)) != EOF) {
        if (ch == '\n') {
            line[len] = '\0';
            const char *t = zr_pbrec_skip_ws(line);
            if (*t) {
                ZrFlightEvent ev;
                if (!zr_pbrec_parse_line(t, &ev)) {
                    ok = false;
                    break;
                }
                if (vm->flight_event_count == cap) {
                    cap = cap == 0 ? 64 : cap * 2;
                    vm->flight_events = (ZrFlightEvent *)zr_xrealloc(vm->flight_events, cap * sizeof(ZrFlightEvent));
                }
                vm->flight_events[vm->flight_event_count++] = ev;
            }
            len = 0;
        } else {
            if (len + 2 >= line_cap) {
                line_cap *= 2;
                line = (char *)zr_xrealloc(line, line_cap);
            }
            line[len++] = (char)ch;
        }
    }
    if (ok && len > 0) {
        line[len] = '\0';
        const char *t = zr_pbrec_skip_ws(line);
        if (*t) {
            ZrFlightEvent ev;
            if (!zr_pbrec_parse_line(t, &ev)) ok = false;
            else {
                if (vm->flight_event_count == cap) {
                    cap = cap == 0 ? 64 : cap * 2;
                    vm->flight_events = (ZrFlightEvent *)zr_xrealloc(vm->flight_events, cap * sizeof(ZrFlightEvent));
                }
                vm->flight_events[vm->flight_event_count++] = ev;
            }
        }
    }
    free(line);
    fclose(fp);
    if (!ok) {
        zr_flight_free_events(vm);
        return vm_error(vm, "flight_replay_start(): malformed .pbrec trace '%s'", path);
    }
    vm->flight_replaying = true;
    vm->flight_replay_cursor = 0;
    return true;
}

static bool bi_flight_record_start(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (!vm_effective_cap(vm, CAP_FS)) {
        return vm_error(vm, "capability denied: flight_record_start requires --allow-fs");
    }
    if (argc != 1 || !IS_STR(argv[0])) {
        return vm_error(vm, "flight_record_start(path) expects 1 string");
    }
    if (!zr_flight_start_record_file(vm, AS_STR(argv[0])->chars)) return false;
    *out = val_bool(true);
    return true;
}

static bool bi_flight_record_stop(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    (void)argc; (void)argv;
    uint64_t n = vm->flight_seq;
    if (vm->flight_rec_fp) {
        fflush(vm->flight_rec_fp);
        fclose(vm->flight_rec_fp);
        vm->flight_rec_fp = NULL;
    }
    vm->flight_recording = false;
    *out = val_num((double)n);
    return true;
}

static bool bi_flight_replay_start(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (!vm_effective_cap(vm, CAP_FS)) {
        return vm_error(vm, "capability denied: flight_replay_start requires --allow-fs");
    }
    if (argc != 1 || !IS_STR(argv[0])) {
        return vm_error(vm, "flight_replay_start(path) expects 1 string");
    }
    if (!zr_flight_start_replay_file(vm, AS_STR(argv[0])->chars)) return false;
    *out = val_num((double)vm->flight_event_count);
    return true;
}

static bool bi_flight_replay_stop(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    (void)argc; (void)argv;
    size_t cur = vm->flight_replay_cursor;
    vm->flight_replaying = false;
    zr_flight_free_events(vm);
    *out = val_num((double)cur);
    return true;
}

/* ============================================================================
 * 5. Hotspot Profiler Builtins & Report Generator (`pblang prof`)
 * ============================================================================ */

typedef struct {
    const char *name;
    const char *file;
    uint64_t    calls;
    uint64_t    self_ns;
    uint64_t    total_ns;
    bool        jitted;
} ZrProfEntry;

static int zr_prof_cmp_desc(const void *a, const void *b) {
    const ZrProfEntry *pa = (const ZrProfEntry *)a;
    const ZrProfEntry *pb = (const ZrProfEntry *)b;
    if (pa->self_ns > pb->self_ns) return -1;
    if (pa->self_ns < pb->self_ns) return 1;
    if (pa->total_ns > pb->total_ns) return -1;
    if (pa->total_ns < pb->total_ns) return 1;
    return 0;
}

static void zr_prof_scan_list(PbObj *head, ZrProfEntry **entries, size_t *cnt, size_t *cap) {
    for (PbObj *o = head; o != NULL; o = o->next) {
        if (o->type == OBJ_FUNC) {
            PbFunc *fn = (PbFunc *)o;
            if (fn->prof_calls > 0) {
                if (*cnt == *cap) {
                    *cap = (*cap == 0) ? 32 : (*cap * 2);
                    *entries = (ZrProfEntry *)zr_xrealloc(*entries, (*cap) * sizeof(ZrProfEntry));
                }
                ZrProfEntry *e = &(*entries)[(*cnt)++];
                e->name = (fn->name && fn->name->len > 0) ? fn->name->chars : "<anon>";
                e->file = (fn->source_name && fn->source_name->len > 0) ? fn->source_name->chars : "<script>";
                e->calls = fn->prof_calls;
                e->self_ns = fn->prof_self_ns;
                e->total_ns = fn->prof_total_ns;
                e->jitted = (fn->jit_fn != NULL);
            }
        }
    }
}

static void zr_prof_collect_and_format(PbVM *vm, bool as_json, const char *svg_out_path, ZrBuf *out_text) {
    ZrProfEntry *entries = NULL;
    size_t cnt = 0, cap = 0;
    zr_prof_scan_list(vm->objects, &entries, &cnt, &cap);
    zr_prof_scan_list(vm->nursery_objects, &entries, &cnt, &cap);
    if (cnt > 1) qsort(entries, cnt, sizeof(ZrProfEntry), zr_prof_cmp_desc);

    uint64_t sum_self_ns = 0;
    for (size_t i = 0; i < cnt; i++) sum_self_ns += entries[i].self_ns;

    if (as_json) {
        zr_buf_append_cstr(out_text, "{\"functions\":[");
        for (size_t i = 0; i < cnt; i++) {
            if (i > 0) zr_buf_push_u8(out_text, ',');
            char row[512];
            snprintf(row, sizeof(row),
                "{\"name\":\"%.120s\",\"calls\":%llu,\"self_ms\":%.4f,\"total_ms\":%.4f,\"jit\":%s}",
                entries[i].name,
                (unsigned long long)entries[i].calls,
                (double)entries[i].self_ns / 1e6,
                (double)entries[i].total_ns / 1e6,
                entries[i].jitted ? "true" : "false");
            zr_buf_append_cstr(out_text, row);
        }
        char tail[256];
        snprintf(tail, sizeof(tail),
            "],\"total_self_ms\":%.4f,\"jit_compiled\":%zu,\"nursery_gc\":%zu,\"full_gc\":%zu}\n",
            (double)sum_self_ns / 1e6, vm->jit_compiled_functions,
            vm->nursery_collections, vm->full_collections);
        zr_buf_append_cstr(out_text, tail);
    } else {
        char hdr[256];
        snprintf(hdr, sizeof(hdr),
            "=== PBLang v%s Hotspot Profile (total self: %.3f ms, JIT fns: %zu) ===\n"
            "%-28s %10s %12s %12s %6s\n",
            PBLANG_ZR_VERSION, (double)sum_self_ns / 1e6, vm->jit_compiled_functions,
            "Function", "Calls", "Self (ms)", "Total (ms)", "JIT");
        zr_buf_append_cstr(out_text, hdr);
        for (size_t i = 0; i < cnt; i++) {
            char row[256];
            snprintf(row, sizeof(row), "%-28.28s %10llu %12.3f %12.3f %6s\n",
                entries[i].name,
                (unsigned long long)entries[i].calls,
                (double)entries[i].self_ns / 1e6,
                (double)entries[i].total_ns / 1e6,
                entries[i].jitted ? "yes" : "no");
            zr_buf_append_cstr(out_text, row);
        }
    }

    if (svg_out_path && *svg_out_path) {
        FILE *sf = fopen(svg_out_path, "wb");
        if (sf) {
            size_t rows = cnt > 20 ? 20 : cnt;
            unsigned height = (unsigned)(80 + rows * 30);
            fprintf(sf, "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"820\" height=\"%u\" viewBox=\"0 0 820 %u\">\n", height, height);
            fprintf(sf, "<rect width=\"100%%\" height=\"100%%\" fill=\"#0f172a\"/>\n");
            fprintf(sf, "<text x=\"20\" y=\"32\" fill=\"#f8fafc\" font-family=\"monospace\" font-size=\"16\">PBLang v%s Hotspot Flame Bars (total %.3f ms)</text>\n",
                    PBLANG_ZR_VERSION, (double)sum_self_ns / 1e6);
            double denom = sum_self_ns > 0 ? (double)sum_self_ns : 1.0;
            for (size_t i = 0; i < rows; i++) {
                double frac = (double)entries[i].self_ns / denom;
                int bw = (int)(frac * 560.0);
                if (bw < 4) bw = 4;
                unsigned y = (unsigned)(50 + i * 30);
                const char *color = entries[i].jitted ? "#22c55e" : "#f97316";
                fprintf(sf, "<rect x=\"220\" y=\"%u\" width=\"%d\" height=\"22\" rx=\"3\" fill=\"%s\"/>\n", y, bw, color);
                fprintf(sf, "<text x=\"20\" y=\"%u\" fill=\"#e2e8f0\" font-family=\"monospace\" font-size=\"13\">%.24s</text>\n",
                        y + 15, entries[i].name);
                fprintf(sf, "<text x=\"%d\" y=\"%u\" fill=\"#cbd5e1\" font-family=\"monospace\" font-size=\"12\">%.2f ms (%llu calls)</text>\n",
                        228 + bw, y + 15, (double)entries[i].self_ns / 1e6, (unsigned long long)entries[i].calls);
            }
            fprintf(sf, "</svg>\n");
            fclose(sf);
        }
    }

    free(entries);
}

static bool bi_vm_set_prof(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 1) return vm_error(vm, "vm_set_prof(enabled) expects 1 boolean");
    bool prev = vm->prof_enabled;
    vm->prof_enabled = val_truthy(argv[0]);
    *out = val_bool(prev);
    return true;
}

static bool bi_vm_prof_stats(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    bool as_json = (argc >= 1) ? val_truthy(argv[0]) : true;
    ZrBuf buf;
    zr_buf_init(&buf);
    zr_prof_collect_and_format(vm, as_json, NULL, &buf);
    size_t len = 0;
    char *s = zr_buf_take(&buf, &len);
    *out = val_obj(pb_str_new(vm, s, len));
    free(s);
    return true;
}

#endif /* PBLANG_ZR_BUFFERS_TAINT_REPLAY_H */
