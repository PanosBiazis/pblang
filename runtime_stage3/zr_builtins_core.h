/* ============================================================================
 * pblang Sovereign Zero-Rust Native Runtime v3.0.0 — zr_builtins_core.h
 * Math, BigNum, Bitwise, Strings, Zero-Copy Buffers, Taint, Capabilities,
 * Replay, JIT, Generational GC, Regex, GIL-Free Threads/Channels, Arenas,
 * Tensors, Quantum, AI, Polyglot & Master Builtin Registration Table.
 * ============================================================================ */
#ifndef PBLANG_ZR_BUILTINS_CORE_H
#define PBLANG_ZR_BUILTINS_CORE_H

#include "zr_vm_core.h"
#include "zr_jit_x64.h"
#include "zr_buffers_taint_replay.h"
#include "zr_pbck_io.h"
#include "zr_compiler.h"
#include "zr_loader.h"
#include "zr_json_crypto.h"
#include "zr_fs_os.h"
#include "zr_net_http_tls.h"
#include "zr_sqlite_ffi.h"
#include "zr_gfx.h"
#include "zr_hpc_compute.h"

/* ============================================================================
 * 1. Math, BigInt / Rational / Decimal Constructors & Bitwise Builtins
 * ============================================================================ */

static inline PbArray *pb_array_new_raw(PbVM *vm, size_t cap) { return pb_array_new(vm, cap); }
static inline bool vm_values_equal(PbValue a, PbValue b) { return val_equal(a, b); }
static inline bool zr_cp_is_ident_cont(uint32_t cp) { return zr_is_ident_continue_cp(cp); }

static inline double zr_bi_to_double(const ZrBigInt *a) {
    if (a->sign == 0 || a->len == 0) return 0.0;
    double v = 0.0;
    for (size_t i = a->len; i > 0; i--) v = v * 1000000000.0 + (double)a->limbs[i - 1];
    return a->sign < 0 ? -v : v;
}
static inline double zr_rat_to_double(const ZrRational *r) {
    double d = zr_bi_to_double(&r->den);
    return d != 0.0 ? zr_bi_to_double(&r->num) / d : 0.0;
}
static inline double zr_dec_to_double(const ZrDecimal *d) {
    return zr_bi_to_double(&d->coeff) * pow(10.0, -(double)d->scale);
}
static inline void zr_rat_init(ZrRational *r) {
    zr_bi_init(&r->num);
    zr_bi_init(&r->den);
    zr_bi_from_i64(&r->den, 1);
}
static inline void zr_rat_normalize(ZrRational *r) {
    ZrBigInt n, d;
    zr_bi_init(&n); zr_bi_init(&d);
    zr_bi_copy(&n, &r->num); zr_bi_copy(&d, &r->den);
    zr_bi_free(&r->num); zr_bi_free(&r->den);
    zr_rat_init_norm(r, &n, &d);
    zr_bi_free(&n); zr_bi_free(&d);
}
static inline void zr_dec_init(ZrDecimal *d) {
    zr_bi_init(&d->coeff);
    d->scale = 0;
}


static bool zr_req_num(PbVM *vm, const char *fn, PbValue v, double *out) {
    if (v.tag == VAL_NUM) {
        *out = v.as.num;
        return true;
    }
    if (is_obj_type(v, OBJ_BIGINT)) {
        *out = zr_bi_to_double(&AS_BIGINT(v)->v);
        return true;
    }
    if (is_obj_type(v, OBJ_RATIONAL)) {
        *out = zr_rat_to_double(&AS_RATIONAL(v)->v);
        return true;
    }
    if (is_obj_type(v, OBJ_DECIMAL)) {
        *out = zr_dec_to_double(&AS_DECIMAL(v)->v);
        return true;
    }
    return vm_error(vm, "%s(): expected number, got %s", fn, zr_val_type_name(v));
}

static bool bi_bigint(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 1) return vm_error(vm, "bigint() expects 1 argument");
    if (is_obj_type(argv[0], OBJ_BIGINT)) { *out = argv[0]; return true; }
    ZrBigInt b;
    zr_bi_init(&b);
    if (argv[0].tag == VAL_NUM) {
        if (!isfinite(argv[0].as.num)) return vm_error(vm, "bigint(): non-finite number");
        zr_bi_from_i64(&b, (int64_t)trunc(argv[0].as.num));
        *out = pb_bigint_new_take(vm, &b);
        return true;
    }
    if (argv[0].tag == VAL_BOOL) {
        zr_bi_from_i64(&b, argv[0].as.b ? 1 : 0);
        *out = pb_bigint_new_take(vm, &b);
        return true;
    }
    if (is_obj_type(argv[0], OBJ_STR)) {
        if (!zr_bi_from_str(&b, AS_STR(argv[0])->chars)) {
            zr_bi_free(&b);
            return vm_error(vm, "bigint(): invalid integer string '%s'", AS_STR(argv[0])->chars);
        }
        *out = pb_bigint_new_take(vm, &b);
        return true;
    }
    return vm_error(vm, "bigint(): unsupported argument type %s", zr_val_type_name(argv[0]));
}

static bool bi_rational(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc < 1 || argc > 2) return vm_error(vm, "rational() expects 1 or 2 arguments");
    if (argc == 1 && is_obj_type(argv[0], OBJ_RATIONAL)) { *out = argv[0]; return true; }
    ZrRational r;
    zr_rat_init(&r);
    if (argc == 1) {
        if (is_obj_type(argv[0], OBJ_STR)) {
            const char *s = AS_STR(argv[0])->chars;
            const char *sl = strchr(s, '/');
            ZrBigInt n, d;
            zr_bi_init(&n);
            zr_bi_init(&d);
            bool ok = false;
            if (sl) {
                char *ns = zr_xstrndup(s, (size_t)(sl - s));
                ok = zr_bi_from_str(&n, ns) && zr_bi_from_str(&d, sl + 1) && d.sign != 0;
                free(ns);
            } else {
                ok = zr_bi_from_str(&n, s);
                zr_bi_from_i64(&d, 1);
            }
            if (!ok) {
                zr_bi_free(&n); zr_bi_free(&d); zr_rat_free(&r);
                return vm_error(vm, "rational(): invalid rational '%s'", s);
            }
            zr_bi_free(&r.num); zr_bi_free(&r.den);
            r.num = n; r.den = d;
            zr_rat_normalize(&r);
            PbRationalObj *ro = (PbRationalObj *)vm_alloc_obj(vm, sizeof(PbRationalObj), OBJ_RATIONAL);
            ro->v = r;
            *out = val_obj(ro);
            return true;
        }
        double x = 0.0;
        if (!zr_req_num(vm, "rational", argv[0], &x)) { zr_rat_free(&r); return false; }
        zr_bi_from_i64(&r.num, (int64_t)trunc(x));
        zr_bi_from_i64(&r.den, 1);
        PbRationalObj *ro = (PbRationalObj *)vm_alloc_obj(vm, sizeof(PbRationalObj), OBJ_RATIONAL);
        ro->v = r;
        *out = val_obj(ro);
        return true;
    }
    ZrBigInt n, d;
    zr_bi_init(&n);
    zr_bi_init(&d);
    if (is_obj_type(argv[0], OBJ_BIGINT)) zr_bi_copy(&n, &AS_BIGINT(argv[0])->v);
    else if (argv[0].tag == VAL_NUM) zr_bi_from_i64(&n, (int64_t)trunc(argv[0].as.num));
    else if (is_obj_type(argv[0], OBJ_STR) && zr_bi_from_str(&n, AS_STR(argv[0])->chars)) {}
    else { zr_bi_free(&n); zr_bi_free(&d); zr_rat_free(&r); return vm_error(vm, "rational(): invalid numerator"); }

    if (is_obj_type(argv[1], OBJ_BIGINT)) zr_bi_copy(&d, &AS_BIGINT(argv[1])->v);
    else if (argv[1].tag == VAL_NUM) zr_bi_from_i64(&d, (int64_t)trunc(argv[1].as.num));
    else if (is_obj_type(argv[1], OBJ_STR) && zr_bi_from_str(&d, AS_STR(argv[1])->chars)) {}
    else { zr_bi_free(&n); zr_bi_free(&d); zr_rat_free(&r); return vm_error(vm, "rational(): invalid denominator"); }

    if (d.sign == 0) {
        zr_bi_free(&n); zr_bi_free(&d); zr_rat_free(&r);
        return vm_error(vm, "rational(): division by zero");
    }
    zr_bi_free(&r.num); zr_bi_free(&r.den);
    r.num = n; r.den = d;
    zr_rat_normalize(&r);
    PbRationalObj *ro = (PbRationalObj *)vm_alloc_obj(vm, sizeof(PbRationalObj), OBJ_RATIONAL);
    ro->v = r;
    *out = val_obj(ro);
    return true;
}

static bool bi_decimal(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 1) return vm_error(vm, "decimal() expects 1 argument");
    if (is_obj_type(argv[0], OBJ_DECIMAL)) { *out = argv[0]; return true; }
    ZrDecimal d;
    zr_dec_init(&d);
    if (is_obj_type(argv[0], OBJ_STR)) {
        if (!zr_dec_from_str(&d, AS_STR(argv[0])->chars)) {
            zr_dec_free(&d);
            return vm_error(vm, "decimal(): invalid decimal '%s'", AS_STR(argv[0])->chars);
        }
        *out = pb_decimal_new_take(vm, &d);
        return true;
    }
    if (argv[0].tag == VAL_NUM) {
        char buf[64];
        snprintf(buf, sizeof(buf), "%.15g", argv[0].as.num);
        if (!zr_dec_from_str(&d, buf)) {
            zr_dec_free(&d);
            return vm_error(vm, "decimal(): invalid float");
        }
        *out = pb_decimal_new_take(vm, &d);
        return true;
    }
    if (is_obj_type(argv[0], OBJ_BIGINT)) {
        zr_bi_copy(&d.coeff, &AS_BIGINT(argv[0])->v);
        d.scale = 0;
        *out = pb_decimal_new_take(vm, &d);
        return true;
    }
    zr_dec_free(&d);
    return vm_error(vm, "decimal(): unsupported type %s", zr_val_type_name(argv[0]));
}

#define ZR_UNARY_MATH(cname, sname, expr) \
static bool cname(PbVM *vm, int argc, PbValue *argv, PbValue *out) { \
    double x = 0.0; \
    if (argc != 1 || !zr_req_num(vm, sname, argv[0], &x)) return argc != 1 ? vm_error(vm, "%s() expects 1 argument", sname) : false; \
    *out = val_num(expr); \
    return true; \
}

ZR_UNARY_MATH(bi_num_abs,   "num_abs",   fabs(x))
ZR_UNARY_MATH(bi_num_floor, "num_floor", floor(x))
ZR_UNARY_MATH(bi_num_ceil,  "num_ceil",  ceil(x))
ZR_UNARY_MATH(bi_num_round, "num_round", round(x))
ZR_UNARY_MATH(bi_num_sqrt,  "num_sqrt",  sqrt(x))
ZR_UNARY_MATH(bi_num_ln,    "num_ln",    log(x))
ZR_UNARY_MATH(bi_num_log2,  "num_log2",  log2(x))
ZR_UNARY_MATH(bi_num_log10, "num_log10", log10(x))
ZR_UNARY_MATH(bi_num_exp,   "num_exp",   exp(x))
ZR_UNARY_MATH(bi_num_sin,   "num_sin",   sin(x))
ZR_UNARY_MATH(bi_num_cos,   "num_cos",   cos(x))
ZR_UNARY_MATH(bi_num_tan,   "num_tan",   tan(x))

#undef ZR_UNARY_MATH

#define ZR_BIN_MATH(cname, sname, expr) \
static bool cname(PbVM *vm, int argc, PbValue *argv, PbValue *out) { \
    double a = 0.0, b = 0.0; \
    if (argc != 2) return vm_error(vm, "%s() expects 2 arguments", sname); \
    if (!zr_req_num(vm, sname, argv[0], &a) || !zr_req_num(vm, sname, argv[1], &b)) return false; \
    *out = val_num(expr); \
    return true; \
}

ZR_BIN_MATH(bi_num_pow,   "num_pow",   pow(a, b))
ZR_BIN_MATH(bi_num_min,   "num_min",   (a < b ? a : b))
ZR_BIN_MATH(bi_num_max,   "num_max",   (a > b ? a : b))
ZR_BIN_MATH(bi_num_atan2, "num_atan2", atan2(a, b))

#undef ZR_BIN_MATH

static bool bi_num_mod(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    double a = 0.0, b = 0.0;
    if (argc != 2) return vm_error(vm, "num_mod() expects 2 arguments");
    if (!zr_req_num(vm, "num_mod", argv[0], &a) || !zr_req_num(vm, "num_mod", argv[1], &b)) return false;
    if (b == 0.0) return vm_error(vm, "num_mod(): modulo by zero");
    *out = val_num(fmod(a, b));
    return true;
}

static bool bi_clamp(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    double x = 0.0, lo = 0.0, hi = 0.0;
    if (argc != 3) return vm_error(vm, "clamp(x, min, max) expects 3 arguments");
    if (!zr_req_num(vm, "clamp", argv[0], &x) ||
        !zr_req_num(vm, "clamp", argv[1], &lo) ||
        !zr_req_num(vm, "clamp", argv[2], &hi)) return false;
    if (x < lo) x = lo;
    if (x > hi) x = hi;
    *out = val_num(x);
    return true;
}

static bool zr_collect_nums(PbVM *vm, const char *fn, int argc, PbValue *argv, const PbValue **items, size_t *len) {
    if (argc == 1 && is_obj_type(argv[0], OBJ_ARRAY)) {
        PbArray *a = AS_ARRAY(argv[0]);
        *items = a->items;
        *len = a->len;
        return true;
    }
    if (argc >= 1) {
        *items = argv;
        *len = (size_t)argc;
        return true;
    }
    return vm_error(vm, "%s() expects an array or numeric arguments", fn);
}

static bool bi_sum(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    const PbValue *it = NULL; size_t n = 0;
    if (!zr_collect_nums(vm, "sum", argc, argv, &it, &n)) return false;
    double acc = 0.0;
    for (size_t i = 0; i < n; i++) {
        double v = 0.0;
        if (!zr_req_num(vm, "sum", it[i], &v)) return false;
        acc += v;
    }
    *out = val_num(acc);
    return true;
}

static bool bi_product(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    const PbValue *it = NULL; size_t n = 0;
    if (!zr_collect_nums(vm, "product", argc, argv, &it, &n)) return false;
    double acc = 1.0;
    for (size_t i = 0; i < n; i++) {
        double v = 0.0;
        if (!zr_req_num(vm, "product", it[i], &v)) return false;
        acc *= v;
    }
    *out = val_num(acc);
    return true;
}

static bool bi_min_of(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    const PbValue *it = NULL; size_t n = 0;
    if (!zr_collect_nums(vm, "min_of", argc, argv, &it, &n)) return false;
    if (n == 0) return vm_error(vm, "min_of(): empty sequence");
    double best = 0.0;
    if (!zr_req_num(vm, "min_of", it[0], &best)) return false;
    for (size_t i = 1; i < n; i++) {
        double v = 0.0;
        if (!zr_req_num(vm, "min_of", it[i], &v)) return false;
        if (v < best) best = v;
    }
    *out = val_num(best);
    return true;
}

static bool bi_max_of(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    const PbValue *it = NULL; size_t n = 0;
    if (!zr_collect_nums(vm, "max_of", argc, argv, &it, &n)) return false;
    if (n == 0) return vm_error(vm, "max_of(): empty sequence");
    double best = 0.0;
    if (!zr_req_num(vm, "max_of", it[0], &best)) return false;
    for (size_t i = 1; i < n; i++) {
        double v = 0.0;
        if (!zr_req_num(vm, "max_of", it[i], &v)) return false;
        if (v > best) best = v;
    }
    *out = val_num(best);
    return true;
}

static PbValue zr_checked_res(PbVM *vm, bool ok, double val, const char *err) {
    PbMap *m = pb_map_new_raw(vm, 4);
    vm_push_root(vm, val_obj(m));
    pb_map_set_cstr(vm, m, "ok", val_bool(ok));
    if (ok) pb_map_set_cstr(vm, m, "value", val_num(val));
    else pb_map_set_cstr(vm, m, "error", val_obj(pb_str_c(vm, err)));
    vm_pop_roots(vm, 1);
    return val_obj(m);
}

static bool bi_checked_add(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    double da, db;
    if (argc != 2 || !zr_req_num(vm, "checked_add", argv[0], &da) || !zr_req_num(vm, "checked_add", argv[1], &db)) return false;
    int64_t a = (int64_t)da, b = (int64_t)db, r = 0;
    if (__builtin_add_overflow(a, b, &r) || fabs((double)r) > 9007199254740991.0) {
        *out = zr_checked_res(vm, false, 0, "integer addition overflow");
    } else {
        *out = zr_checked_res(vm, true, (double)r, NULL);
    }
    return true;
}

static bool bi_checked_sub(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    double da, db;
    if (argc != 2 || !zr_req_num(vm, "checked_sub", argv[0], &da) || !zr_req_num(vm, "checked_sub", argv[1], &db)) return false;
    int64_t a = (int64_t)da, b = (int64_t)db, r = 0;
    if (__builtin_sub_overflow(a, b, &r) || fabs((double)r) > 9007199254740991.0) {
        *out = zr_checked_res(vm, false, 0, "integer subtraction overflow");
    } else {
        *out = zr_checked_res(vm, true, (double)r, NULL);
    }
    return true;
}

static bool bi_checked_mul(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    double da, db;
    if (argc != 2 || !zr_req_num(vm, "checked_mul", argv[0], &da) || !zr_req_num(vm, "checked_mul", argv[1], &db)) return false;
    int64_t a = (int64_t)da, b = (int64_t)db, r = 0;
    if (__builtin_mul_overflow(a, b, &r) || fabs((double)r) > 9007199254740991.0) {
        *out = zr_checked_res(vm, false, 0, "integer multiplication overflow");
    } else {
        *out = zr_checked_res(vm, true, (double)r, NULL);
    }
    return true;
}

static bool bi_checked_div(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    double da, db;
    if (argc != 2 || !zr_req_num(vm, "checked_div", argv[0], &da) || !zr_req_num(vm, "checked_div", argv[1], &db)) return false;
    if (db == 0.0) {
        *out = zr_checked_res(vm, false, 0, "division by zero");
        return true;
    }
    double r = da / db;
    if (!isfinite(r)) *out = zr_checked_res(vm, false, 0, "division overflow");
    else *out = zr_checked_res(vm, true, r, NULL);
    return true;
}

static bool bi_bit_and(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    double a, b;
    if (argc != 2 || !zr_req_num(vm, "bit_and", argv[0], &a) || !zr_req_num(vm, "bit_and", argv[1], &b)) return false;
    *out = val_num((double)((int64_t)a & (int64_t)b));
    return true;
}

static bool bi_bit_or(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    double a, b;
    if (argc != 2 || !zr_req_num(vm, "bit_or", argv[0], &a) || !zr_req_num(vm, "bit_or", argv[1], &b)) return false;
    *out = val_num((double)((int64_t)a | (int64_t)b));
    return true;
}

static bool bi_bit_xor(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    double a, b;
    if (argc != 2 || !zr_req_num(vm, "bit_xor", argv[0], &a) || !zr_req_num(vm, "bit_xor", argv[1], &b)) return false;
    *out = val_num((double)((int64_t)a ^ (int64_t)b));
    return true;
}

static bool bi_bit_not(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    double a;
    if (argc != 1 || !zr_req_num(vm, "bit_not", argv[0], &a)) return false;
    *out = val_num((double)(~(int64_t)a));
    return true;
}

static bool bi_bit_shl(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    double a, b;
    if (argc != 2 || !zr_req_num(vm, "bit_shl", argv[0], &a) || !zr_req_num(vm, "bit_shl", argv[1], &b)) return false;
    uint32_t sh = ((uint32_t)(int64_t)b) & 63u;
    *out = val_num((double)(int64_t)(((uint64_t)(int64_t)a) << sh));
    return true;
}

static bool bi_bit_shr(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    double a, b;
    if (argc != 2 || !zr_req_num(vm, "bit_shr", argv[0], &a) || !zr_req_num(vm, "bit_shr", argv[1], &b)) return false;
    uint32_t sh = ((uint32_t)(int64_t)b) & 63u;
    *out = val_num((double)(((int64_t)a) >> sh));
    return true;
}

static bool bi_num_to_le_bytes(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 1 || argv[0].tag != VAL_NUM) return vm_error(vm, "num_to_le_bytes() expects 1 number");
    uint64_t bits = 0;
    memcpy(&bits, &argv[0].as.num, 8);
    PbArray *a = pb_array_new(vm, 8);
    vm_push_root(vm, val_obj(a));
    for (int i = 0; i < 8; i++) {
        pb_array_push(vm, a, val_num((double)((bits >> (i * 8)) & 0xFFu)));
    }
    vm_pop_roots(vm, 1);
    *out = val_obj(a);
    return true;
}

static bool bi_num_from_le_bytes(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 1 || !is_obj_type(argv[0], OBJ_ARRAY)) return vm_error(vm, "num_from_le_bytes() expects a byte array");
    PbArray *a = AS_ARRAY(argv[0]);
    if (a->len == 8) {
        uint64_t bits = 0;
        for (int i = 0; i < 8; i++) {
            if (a->items[i].tag != VAL_NUM) return vm_error(vm, "num_from_le_bytes(): element must be number");
            bits |= ((uint64_t)((uint8_t)a->items[i].as.num)) << (i * 8);
        }
        double d = 0.0;
        memcpy(&d, &bits, 8);
        *out = val_num(d);
        return true;
    }
    if (a->len >= 1 && a->len <= 4) {
        uint64_t v = 0;
        for (size_t i = 0; i < a->len; i++) {
            if (a->items[i].tag != VAL_NUM) return vm_error(vm, "le_bytes_to_num(): element must be number");
            v |= ((uint64_t)((uint8_t)a->items[i].as.num)) << (i * 8);
        }
        *out = val_num((double)v);
        return true;
    }
    return vm_error(vm, "num_from_le_bytes(): expected 8 bytes, got %zu", a->len);
}

/* ============================================================================
 * 2. String, Parsing, Sequence & Collection Builtins
 * ============================================================================ */

static bool bi_str(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 1) return vm_error(vm, "str() expects 1 argument");
    if (is_obj_type(argv[0], OBJ_STR)) { *out = argv[0]; return true; }
    PbValue r = val_obj(vm_value_to_pbstr(vm, argv[0]));
    r.taint = argv[0].taint;
    *out = r;
    return true;
}

static bool bi_len(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 1) return vm_error(vm, "len() expects 1 argument");
    PbValue v = argv[0];
    if (is_obj_type(v, OBJ_STR)) {
        *out = val_num((double)zr_utf8_char_len(AS_STR(v)->chars, AS_STR(v)->len));
        return true;
    }
    if (is_obj_type(v, OBJ_BYTES)) {
        *out = val_num((double)AS_BYTES(v)->len);
        return true;
    }
    if (is_obj_type(v, OBJ_F64BUF)) {
        *out = val_num((double)AS_F64BUF(v)->len);
        return true;
    }
    if (is_obj_type(v, OBJ_ARRAY)) {
        *out = val_num((double)AS_ARRAY(v)->len);
        return true;
    }
    if (is_obj_type(v, OBJ_MAP)) {
        *out = val_num((double)AS_MAP(v)->len);
        return true;
    }
    if (is_obj_type(v, OBJ_MODULE)) {
        *out = val_num((double)AS_MODULE(v)->exports->len);
        return true;
    }
    return vm_error(vm, "len(): unsupported type %s", zr_val_type_name(v));
}

static bool bi_slice(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc < 2 || argc > 3) return vm_error(vm, "slice(seq, start[, end]) expects 2 or 3 arguments");
    if (argv[1].tag != VAL_NUM) return vm_error(vm, "slice(): start index must be a number");
    if (argc == 3 && argv[2].tag != VAL_NUM && argv[2].tag != VAL_NIL) {
        return vm_error(vm, "slice(): end index must be a number");
    }
    if (is_obj_type(argv[0], OBJ_BYTES)) {
        return bi_bytes_slice(vm, argc, argv, out);
    }
    if (is_obj_type(argv[0], OBJ_ARRAY)) {
        PbArray *src = AS_ARRAY(argv[0]);
        int64_t n = (int64_t)src->len;
        int64_t st = (int64_t)argv[1].as.num;
        int64_t en = (argc == 3 && argv[2].tag == VAL_NUM) ? (int64_t)argv[2].as.num : n;
        if (st < 0) st += n;
        if (en < 0) en += n;
        if (st < 0) st = 0;
        if (st > n) st = n;
        if (en < st) en = st;
        if (en > n) en = n;
        size_t cnt = (size_t)(en - st);
        vm_push_root(vm, argv[0]);
        PbArray *dst = pb_array_new(vm, cnt);
        for (size_t i = 0; i < cnt; i++) pb_array_push(vm, dst, src->items[(size_t)st + i]);
        vm_pop_roots(vm, 1);
        PbValue r = val_obj(dst);
        r.taint = argv[0].taint;
        *out = r;
        return true;
    }
    if (is_obj_type(argv[0], OBJ_STR)) {
        PbStr *s = AS_STR(argv[0]);
        int64_t n = (int64_t)zr_utf8_char_len(s->chars, s->len);
        int64_t st = (int64_t)argv[1].as.num;
        int64_t en = (argc == 3 && argv[2].tag == VAL_NUM) ? (int64_t)argv[2].as.num : n;
        if (st < 0) st += n;
        if (en < 0) en += n;
        if (st < 0) st = 0;
        if (st > n) st = n;
        if (en < st) en = st;
        if (en > n) en = n;
        size_t adv = 0;
        size_t b0 = zr_utf8_nth_offset(s->chars, s->len, (size_t)st, &adv);
        size_t b1 = zr_utf8_nth_offset(s->chars, s->len, (size_t)en, &adv);
        vm_push_root(vm, argv[0]);
        PbStr *sub = pb_str_new(vm, s->chars + b0, b1 - b0);
        vm_pop_roots(vm, 1);
        PbValue r = val_obj(sub);
        r.taint = argv[0].taint;
        *out = r;
        return true;
    }
    return vm_error(vm, "slice(): expected array, bytes, or string, got %s", zr_val_type_name(argv[0]));
}

static bool bi_str_slice(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 3 || !is_obj_type(argv[0], OBJ_STR) || argv[1].tag != VAL_NUM || argv[2].tag != VAL_NUM) {
        return vm_error(vm, "str_slice(s, start, end) expects (string, number, number)");
    }
    return bi_slice(vm, 3, argv, out);
}

static bool bi_str_contains(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 2 || !is_obj_type(argv[0], OBJ_STR) || !is_obj_type(argv[1], OBJ_STR)) {
        return vm_error(vm, "str_contains(s, sub) expects 2 strings");
    }
    PbStr *s = AS_STR(argv[0]), *sub = AS_STR(argv[1]);
    if (sub->len == 0) { *out = val_bool(true); return true; }
    if (sub->len > s->len) { *out = val_bool(false); return true; }
    for (size_t i = 0; i + sub->len <= s->len; i++) {
        if (memcmp(s->chars + i, sub->chars, sub->len) == 0) {
            *out = val_bool(true);
            return true;
        }
    }
    *out = val_bool(false);
    return true;
}

static bool bi_str_starts_with(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 2 || !is_obj_type(argv[0], OBJ_STR) || !is_obj_type(argv[1], OBJ_STR)) {
        return vm_error(vm, "str_starts_with(s, pre) expects 2 strings");
    }
    PbStr *s = AS_STR(argv[0]), *pre = AS_STR(argv[1]);
    *out = val_bool(pre->len <= s->len && memcmp(s->chars, pre->chars, pre->len) == 0);
    return true;
}

static bool bi_str_ends_with(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 2 || !is_obj_type(argv[0], OBJ_STR) || !is_obj_type(argv[1], OBJ_STR)) {
        return vm_error(vm, "str_ends_with(s, suf) expects 2 strings");
    }
    PbStr *s = AS_STR(argv[0]), *suf = AS_STR(argv[1]);
    *out = val_bool(suf->len <= s->len && memcmp(s->chars + (s->len - suf->len), suf->chars, suf->len) == 0);
    return true;
}

static bool bi_str_index_of(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 2 || !is_obj_type(argv[0], OBJ_STR) || !is_obj_type(argv[1], OBJ_STR)) {
        return vm_error(vm, "str_index_of(s, sub) expects 2 strings");
    }
    PbStr *s = AS_STR(argv[0]), *sub = AS_STR(argv[1]);
    if (sub->len == 0) { *out = val_num(0.0); return true; }
    if (sub->len <= s->len) {
        for (size_t i = 0; i + sub->len <= s->len; i++) {
            if (memcmp(s->chars + i, sub->chars, sub->len) == 0) {
                *out = val_num((double)zr_utf8_char_len(s->chars, i));
                return true;
            }
        }
    }
    *out = val_num(-1.0);
    return true;
}

static bool bi_str_split(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 2 || !is_obj_type(argv[0], OBJ_STR) || !is_obj_type(argv[1], OBJ_STR)) {
        return vm_error(vm, "str_split(s, delim) expects 2 strings");
    }
    uint8_t taint = argv[0].taint;
    vm_push_root(vm, argv[0]);
    vm_push_root(vm, argv[1]);
    PbStr *s = AS_STR(argv[0]), *delim = AS_STR(argv[1]);
    PbArray *arr = pb_array_new(vm, 8);
    vm_push_root(vm, val_obj(arr));
    if (delim->len == 0) {
        size_t pos = 0;
        while (pos < s->len) {
            size_t adv = 1;
            (void)zr_utf8_decode(s->chars + pos, s->len - pos, &adv);
            if (adv == 0) adv = 1;
            PbValue ch = val_obj(pb_str_new(vm, s->chars + pos, adv));
            ch.taint = taint;
            pb_array_push(vm, arr, ch);
            pos += adv;
        }
    } else {
        size_t pos = 0;
        for (;;) {
            const char *found = NULL;
            for (size_t i = pos; i + delim->len <= s->len; i++) {
                if (memcmp(s->chars + i, delim->chars, delim->len) == 0) {
                    found = s->chars + i;
                    break;
                }
            }
            if (!found) {
                PbValue part = val_obj(pb_str_new(vm, s->chars + pos, s->len - pos));
                part.taint = taint;
                pb_array_push(vm, arr, part);
                break;
            }
            size_t flen = (size_t)(found - (s->chars + pos));
            PbValue part = val_obj(pb_str_new(vm, s->chars + pos, flen));
            part.taint = taint;
            pb_array_push(vm, arr, part);
            pos += flen + delim->len;
        }
    }
    vm_pop_roots(vm, 3);
    PbValue r = val_obj(arr);
    r.taint = taint;
    *out = r;
    return true;
}

static bool bi_str_join(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 2 || !is_obj_type(argv[0], OBJ_ARRAY) || !is_obj_type(argv[1], OBJ_STR)) {
        return vm_error(vm, "str_join(arr, sep) expects (array, string)");
    }
    PbArray *arr = AS_ARRAY(argv[0]);
    PbStr *sep = AS_STR(argv[1]);
    uint8_t taint = argv[0].taint | (arr->len > 1 ? argv[1].taint : 0);
    ZrBuf b;
    zr_buf_init(&b);
    for (size_t i = 0; i < arr->len; i++) {
        if (i > 0) zr_buf_append(&b, sep->chars, sep->len);
        taint |= arr->items[i].taint;
        if (is_obj_type(arr->items[i], OBJ_STR)) {
            zr_buf_append(&b, AS_STR(arr->items[i])->chars, AS_STR(arr->items[i])->len);
        } else {
            vm_format_value_rec(vm, &b, arr->items[i], 0);
        }
    }
    PbValue r = val_obj(pb_str_new(vm, b.data ? b.data : "", b.len));
    r.taint = taint;
    *out = r;
    zr_buf_free(&b);
    return true;
}

static bool zr_is_unicode_space(uint32_t cp) {
    return cp == ' ' || cp == '\t' || cp == '\n' || cp == '\r' || cp == '\v' || cp == '\f' ||
           cp == 0x00A0 || cp == 0x2000 || cp == 0x2002 || cp == 0x2003 || cp == 0x3000 || cp == 0xFEFF;
}

static bool bi_str_trim(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 1 || !is_obj_type(argv[0], OBJ_STR)) return vm_error(vm, "str_trim() expects 1 string");
    PbStr *s = AS_STR(argv[0]);
    size_t st = 0, en = s->len;
    while (st < en) {
        size_t adv = 1;
        uint32_t cp = zr_utf8_decode(s->chars + st, en - st, &adv);
        if (!zr_is_unicode_space(cp)) break;
        st += adv;
    }
    while (en > st) {
        size_t prev = en - 1;
        while (prev > st && ((uint8_t)s->chars[prev] & 0xC0u) == 0x80u) prev--;
        size_t adv = 1;
        uint32_t cp = zr_utf8_decode(s->chars + prev, en - prev, &adv);
        if (!zr_is_unicode_space(cp)) break;
        en = prev;
    }
    vm_push_root(vm, argv[0]);
    PbStr *res = pb_str_new(vm, s->chars + st, en - st);
    vm_pop_roots(vm, 1);
    PbValue r = val_obj(res);
    r.taint = argv[0].taint;
    *out = r;
    return true;
}

static bool bi_str_upper(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 1 || !is_obj_type(argv[0], OBJ_STR)) return vm_error(vm, "str_upper() expects 1 string");
    PbStr *s = AS_STR(argv[0]);
    ZrBuf b;
    zr_buf_init(&b);
    size_t pos = 0;
    while (pos < s->len) {
        size_t adv = 1;
        uint32_t cp = zr_utf8_decode(s->chars + pos, s->len - pos, &adv);
        pos += adv ? adv : 1;
        char u8[4];
        size_t ul = zr_utf8_encode(zr_cp_to_upper(cp), u8);
        zr_buf_append(&b, u8, ul);
    }
    PbValue r = val_obj(pb_str_new(vm, b.data ? b.data : "", b.len));
    r.taint = argv[0].taint;
    *out = r;
    zr_buf_free(&b);
    return true;
}

static bool bi_str_lower(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 1 || !is_obj_type(argv[0], OBJ_STR)) return vm_error(vm, "str_lower() expects 1 string");
    PbStr *s = AS_STR(argv[0]);
    ZrBuf b;
    zr_buf_init(&b);
    size_t pos = 0;
    while (pos < s->len) {
        size_t adv = 1;
        uint32_t cp = zr_utf8_decode(s->chars + pos, s->len - pos, &adv);
        pos += adv ? adv : 1;
        char u8[4];
        size_t ul = zr_utf8_encode(zr_cp_to_lower(cp), u8);
        zr_buf_append(&b, u8, ul);
    }
    PbValue r = val_obj(pb_str_new(vm, b.data ? b.data : "", b.len));
    r.taint = argv[0].taint;
    *out = r;
    zr_buf_free(&b);
    return true;
}

static bool bi_str_replace_all(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 3 || !is_obj_type(argv[0], OBJ_STR) || !is_obj_type(argv[1], OBJ_STR) || !is_obj_type(argv[2], OBJ_STR)) {
        return vm_error(vm, "str_replace_all(s, from, to) expects 3 strings");
    }
    PbStr *s = AS_STR(argv[0]), *pat = AS_STR(argv[1]), *rep = AS_STR(argv[2]);
    if (pat->len == 0) { *out = argv[0]; return true; }
    ZrBuf b;
    zr_buf_init(&b);
    size_t i = 0;
    bool replaced = false;
    while (i < s->len) {
        if (i + pat->len <= s->len && memcmp(s->chars + i, pat->chars, pat->len) == 0) {
            zr_buf_append(&b, rep->chars, rep->len);
            i += pat->len;
            replaced = true;
        } else {
            zr_buf_push(&b, s->chars[i++]);
        }
    }
    PbValue r = val_obj(pb_str_new(vm, b.data ? b.data : "", b.len));
    r.taint = argv[0].taint | (replaced ? argv[2].taint : 0);
    *out = r;
    zr_buf_free(&b);
    return true;
}

static bool bi_str_repeat(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 2 || !is_obj_type(argv[0], OBJ_STR) || argv[1].tag != VAL_NUM || argv[1].as.num < 0.0) {
        return vm_error(vm, "str_repeat(s, n) expects (string, non-negative number)");
    }
    PbStr *s = AS_STR(argv[0]);
    size_t n = (size_t)argv[1].as.num;
    if (n == 0 || s->len == 0) { *out = val_obj(pb_str_c(vm, "")); return true; }
    if (s->len > (64u * 1024u * 1024u) / n) return vm_error(vm, "str_repeat(): result exceeds 64 MiB limit");
    size_t tot = s->len * n;
    vm_push_root(vm, argv[0]);
    PbStr *res = (PbStr *)vm_alloc_obj(vm, sizeof(PbStr) + tot + 1, OBJ_STR);
    s = AS_STR(argv[0]);
    for (size_t i = 0; i < n; i++) memcpy(res->chars + i * s->len, s->chars, s->len);
    res->chars[tot] = '\0';
    res->len = tot;
    res->hash = zr_fnv1a(res->chars, tot);
    vm->string_allocs++;
    vm_pop_roots(vm, 1);
    PbValue r = val_obj(res);
    r.taint = argv[0].taint;
    *out = r;
    return true;
}

static bool zr_str_pad_impl(PbVM *vm, int argc, PbValue *argv, bool left, PbValue *out) {
    if (argc < 2 || argc > 3 || !is_obj_type(argv[0], OBJ_STR) || argv[1].tag != VAL_NUM) {
        return vm_error(vm, "%s(s, width[, pad]) expects (string, number[, string])", left ? "str_pad_left" : "str_pad_right");
    }
    PbStr *s = AS_STR(argv[0]);
    const char *pad_s = " ";
    size_t pad_blen = 1;
    uint8_t taint = argv[0].taint;
    if (argc == 3 && is_obj_type(argv[2], OBJ_STR) && AS_STR(argv[2])->len > 0) {
        size_t adv = 1;
        (void)zr_utf8_decode(AS_STR(argv[2])->chars, AS_STR(argv[2])->len, &adv);
        pad_s = AS_STR(argv[2])->chars;
        pad_blen = adv ? adv : 1;
        taint |= argv[2].taint;
    }
    size_t cur_cp = zr_utf8_char_len(s->chars, s->len);
    size_t target = argv[1].as.num > 0.0 ? (size_t)argv[1].as.num : 0;
    if (cur_cp >= target) { *out = argv[0]; return true; }
    size_t needed = target - cur_cp;
    ZrBuf b;
    zr_buf_init(&b);
    if (!left) zr_buf_append(&b, s->chars, s->len);
    for (size_t i = 0; i < needed; i++) zr_buf_append(&b, pad_s, pad_blen);
    if (left) zr_buf_append(&b, s->chars, s->len);
    PbValue r = val_obj(pb_str_new(vm, b.data ? b.data : "", b.len));
    r.taint = taint;
    *out = r;
    zr_buf_free(&b);
    return true;
}

static bool bi_str_pad_left(PbVM *vm, int argc, PbValue *argv, PbValue *out) { return zr_str_pad_impl(vm, argc, argv, true, out); }
static bool bi_str_pad_right(PbVM *vm, int argc, PbValue *argv, PbValue *out) { return zr_str_pad_impl(vm, argc, argv, false, out); }

static bool bi_str_format(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc < 1 || !is_obj_type(argv[0], OBJ_STR)) return vm_error(vm, "str_format() expects format string as first argument");
    PbStr *fmt = AS_STR(argv[0]);
    uint8_t taint = argv[0].taint;
    ZrBuf b;
    zr_buf_init(&b);
    int arg_i = 1;
    size_t pos = 0;
    while (pos < fmt->len) {
        char c = fmt->chars[pos++];
        if (c == '%' && pos < fmt->len) {
            char sp = fmt->chars[pos++];
            if (sp == '%') {
                zr_buf_push(&b, '%');
            } else if (sp == 's' || sp == 'v') {
                if (arg_i >= argc) { zr_buf_free(&b); return vm_error(vm, "str_format(): not enough arguments"); }
                taint |= argv[arg_i].taint;
                vm_format_value_rec(vm, &b, argv[arg_i++], 0);
            } else if (sp == 'd' || sp == 'i') {
                if (arg_i >= argc) { zr_buf_free(&b); return vm_error(vm, "str_format(): not enough arguments"); }
                taint |= argv[arg_i].taint;
                if (argv[arg_i].tag == VAL_NUM) zr_buf_printf(&b, "%" PRId64, (int64_t)argv[arg_i].as.num);
                else vm_format_value_rec(vm, &b, argv[arg_i], 0);
                arg_i++;
            } else if (sp == 'f') {
                if (arg_i >= argc) { zr_buf_free(&b); return vm_error(vm, "str_format(): not enough arguments"); }
                taint |= argv[arg_i].taint;
                if (argv[arg_i].tag == VAL_NUM) zr_buf_printf(&b, "%.6f", argv[arg_i].as.num);
                else vm_format_value_rec(vm, &b, argv[arg_i], 0);
                arg_i++;
            } else if (sp == 'x' || sp == 'X') {
                if (arg_i >= argc) { zr_buf_free(&b); return vm_error(vm, "str_format(): not enough arguments"); }
                taint |= argv[arg_i].taint;
                if (argv[arg_i].tag == VAL_NUM) zr_buf_printf(&b, sp == 'x' ? "%" PRIx64 : "%" PRIX64, (uint64_t)(int64_t)argv[arg_i].as.num);
                else vm_format_value_rec(vm, &b, argv[arg_i], 0);
                arg_i++;
            } else {
                zr_buf_push(&b, '%');
                zr_buf_push(&b, sp);
            }
        } else if (c == '{' && pos < fmt->len && fmt->chars[pos] == '}') {
            pos++;
            if (arg_i >= argc) { zr_buf_free(&b); return vm_error(vm, "str_format(): not enough arguments"); }
            taint |= argv[arg_i].taint;
            vm_format_value_rec(vm, &b, argv[arg_i++], 0);
        } else {
            zr_buf_push(&b, c);
        }
    }
    PbValue r = val_obj(pb_str_new(vm, b.data ? b.data : "", b.len));
    r.taint = taint;
    *out = r;
    zr_buf_free(&b);
    return true;
}

static bool bi_strfromcharcode(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 1 || argv[0].tag != VAL_NUM) return vm_error(vm, "str_from_char_code() expects 1 number");
    int64_t cp = (int64_t)argv[0].as.num;
    if (cp < 0 || cp > 0x10FFFF) return vm_error(vm, "str_from_char_code(): invalid codepoint %" PRId64, cp);
    char u8[4];
    size_t n = zr_utf8_encode((uint32_t)cp, u8);
    PbValue r = val_obj(pb_str_new(vm, u8, n));
    r.taint = argv[0].taint;
    *out = r;
    return true;
}

static bool bi_strcharcodeat(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 2 || !is_obj_type(argv[0], OBJ_STR) || argv[1].tag != VAL_NUM) {
        return vm_error(vm, "strcharcodeat(s, idx) expects (string, number)");
    }
    PbStr *s = AS_STR(argv[0]);
    int64_t idx = (int64_t)argv[1].as.num;
    size_t clen = zr_utf8_char_len(s->chars, s->len);
    if (idx < 0 || (size_t)idx >= clen) { *out = val_nil(); return true; }
    size_t adv = 0;
    size_t off = zr_utf8_nth_offset(s->chars, s->len, (size_t)idx, &adv);
    uint32_t cp = zr_utf8_decode(s->chars + off, s->len - off, &adv);
    PbValue r = val_num((double)cp);
    r.taint = argv[0].taint;
    *out = r;
    return true;
}

static bool bi_str_to_bytes(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 1 || !is_obj_type(argv[0], OBJ_STR)) return vm_error(vm, "str_to_bytes() expects 1 string");
    uint8_t taint = argv[0].taint;
    vm_push_root(vm, argv[0]);
    PbStr *s = AS_STR(argv[0]);
    PbArray *a = pb_array_new(vm, s->len);
    s = AS_STR(argv[0]);
    for (size_t i = 0; i < s->len; i++) {
        PbValue bv = val_num((double)(uint8_t)s->chars[i]);
        bv.taint = taint;
        pb_array_push(vm, a, bv);
    }
    vm_pop_roots(vm, 1);
    PbValue r = val_obj(a);
    r.taint = taint;
    *out = r;
    return true;
}

static bool bi_bytes_to_str(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 1) return vm_error(vm, "bytes_to_str() expects 1 bytes or byte-array argument");
    if (is_obj_type(argv[0], OBJ_BYTES)) {
        PbBytes *b = AS_BYTES(argv[0]);
        vm_push_root(vm, argv[0]);
        PbStr *s = pb_str_new(vm, b->data ? (const char *)b->data : "", b->len);
        vm_pop_roots(vm, 1);
        PbValue r = val_obj(s);
        r.taint = argv[0].taint;
        *out = r;
        return true;
    }
    if (is_obj_type(argv[0], OBJ_STR)) {
        *out = argv[0];
        return true;
    }
    if (!is_obj_type(argv[0], OBJ_ARRAY)) return vm_error(vm, "bytes_to_str() expects bytes or byte array");
    PbArray *a = AS_ARRAY(argv[0]);
    uint8_t taint = argv[0].taint;
    char *buf = (char *)zr_xmalloc(a->len + 1);
    for (size_t i = 0; i < a->len; i++) {
        if (a->items[i].tag != VAL_NUM || a->items[i].as.num < 0.0 || a->items[i].as.num > 255.0) {
            free(buf);
            return vm_error(vm, "bytes_to_str(): element %zu out of byte range 0..255", i);
        }
        taint |= a->items[i].taint;
        buf[i] = (char)(uint8_t)a->items[i].as.num;
    }
    buf[a->len] = '\0';
    PbStr *s = pb_str_new(vm, buf, a->len);
    free(buf);
    PbValue r = val_obj(s);
    r.taint = taint;
    *out = r;
    return true;
}

static bool bi_parse_number(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 1) return vm_error(vm, "parse_number() expects 1 argument");
    if (argv[0].tag == VAL_NUM) { *out = argv[0]; return true; }
    if (!is_obj_type(argv[0], OBJ_STR)) return vm_error(vm, "parse_number(): expected string or number");
    const char *s = AS_STR(argv[0])->chars;
    while (*s && isspace((unsigned char)*s)) s++;
    if (!*s) { *out = val_nil(); return true; }
    char *endp = NULL;
    if (s[0] == '0' && (s[1] == 'x' || s[1] == 'X')) {
        uint64_t hx = strtoull(s + 2, &endp, 16);
        while (endp && *endp && isspace((unsigned char)*endp)) endp++;
        if (endp && *endp == '\0') { *out = val_num((double)hx); return true; }
    }
    double d = strtod(s, &endp);
    while (endp && *endp && isspace((unsigned char)*endp)) endp++;
    if (!endp || *endp != '\0' || endp == s) {
        *out = val_nil();
        return true;
    }
    *out = val_num(d);
    return true;
}

static bool bi_parse_bool(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 1) return vm_error(vm, "parse_bool() expects 1 argument");
    if (argv[0].tag == VAL_BOOL) { *out = argv[0]; return true; }
    if (argv[0].tag == VAL_NUM) { *out = val_bool(argv[0].as.num != 0.0); return true; }
    if (is_obj_type(argv[0], OBJ_STR)) {
        const char *s = AS_STR(argv[0])->chars;
        if (strcasecmp(s, "true") == 0 || strcmp(s, "1") == 0 || strcasecmp(s, "yes") == 0) { *out = val_bool(true); return true; }
        if (strcasecmp(s, "false") == 0 || strcmp(s, "0") == 0 || strcasecmp(s, "no") == 0) { *out = val_bool(false); return true; }
        *out = val_nil();
        return true;
    }
    *out = val_nil();
    return true;
}

/* ---- Array & Map Builtins ---- */

static bool bi_array_push(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 2 || !is_obj_type(argv[0], OBJ_ARRAY)) return vm_error(vm, "push(arr, val) expects (array, any)");
    PbArray *a = AS_ARRAY(argv[0]);
    if (a->obj.frozen) return vm_error(vm, "cannot mutate frozen array");
    pb_array_push(vm, a, argv[1]);
    *out = argv[0];
    return true;
}

static bool bi_array_pop(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 1 || !is_obj_type(argv[0], OBJ_ARRAY)) return vm_error(vm, "pop(arr) expects 1 array");
    PbArray *a = AS_ARRAY(argv[0]);
    if (a->obj.frozen) return vm_error(vm, "cannot mutate frozen array");
    if (a->len == 0) { *out = val_nil(); return true; }
    *out = a->items[--a->len];
    return true;
}

static int zr_cmp_pbval_sort(const void *pa, const void *pb) {
    const PbValue *a = (const PbValue *)pa;
    const PbValue *b = (const PbValue *)pb;
    if (a->tag == VAL_NUM && b->tag == VAL_NUM) {
        return (a->as.num < b->as.num) ? -1 : ((a->as.num > b->as.num) ? 1 : 0);
    }
    if (is_obj_type(*a, OBJ_STR) && is_obj_type(*b, OBJ_STR)) {
        const PbStr *sa = AS_STR(*a), *sb = AS_STR(*b);
        size_t ml = sa->len < sb->len ? sa->len : sb->len;
        int c = memcmp(sa->chars, sb->chars, ml);
        if (c != 0) return c;
        return (sa->len < sb->len) ? -1 : ((sa->len > sb->len) ? 1 : 0);
    }
    return (int)a->tag - (int)b->tag;
}

static bool bi_array_sort(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc < 1 || argc > 2 || !is_obj_type(argv[0], OBJ_ARRAY)) return vm_error(vm, "array_sort(arr[, cmp_fn]) expects array");
    PbArray *src = AS_ARRAY(argv[0]);
    vm_push_root(vm, argv[0]);
    PbArray *dst = pb_array_new(vm, src->len);
    vm_push_root(vm, val_obj(dst));
    for (size_t i = 0; i < src->len; i++) pb_array_push(vm, dst, src->items[i]);
    if (argc == 1 || argv[1].tag == VAL_NIL) {
        if (dst->len > 1) qsort(dst->items, dst->len, sizeof(PbValue), zr_cmp_pbval_sort);
    } else {
        /* Custom comparator insertion sort (stable & re-entrant VM call safe) */
        vm_push_root(vm, argv[1]);
        for (size_t i = 1; i < dst->len; i++) {
            PbValue key = dst->items[i];
            vm_push_root(vm, key);
            size_t j = i;
            while (j > 0) {
                PbValue cargs[2] = { dst->items[j - 1], key };
                PbValue cres = val_nil();
                if (!vm_call_value_sync(vm, argv[1], 2, cargs, &cres)) {
                    vm_pop_roots(vm, 4);
                    return false;
                }
                bool greater = (cres.tag == VAL_NUM && cres.as.num > 0.0) || (cres.tag == VAL_BOOL && cres.as.b);
                if (!greater) break;
                dst->items[j] = dst->items[j - 1];
                vm_write_barrier(vm, (PbObj *)dst, dst->items[j]);
                j--;
            }
            dst->items[j] = key;
            vm_write_barrier(vm, (PbObj *)dst, key);
            vm_pop_roots(vm, 1);
        }
        vm_pop_roots(vm, 1);
    }
    vm_pop_roots(vm, 2);
    *out = val_obj(dst);
    return true;
}

static bool bi_array_unique(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 1 || !is_obj_type(argv[0], OBJ_ARRAY)) return vm_error(vm, "array_unique(arr) expects 1 array");
    PbArray *src = AS_ARRAY(argv[0]);
    vm_push_root(vm, argv[0]);
    PbArray *dst = pb_array_new(vm, src->len);
    for (size_t i = 0; i < src->len; i++) {
        bool dup = false;
        for (size_t j = 0; j < dst->len; j++) {
            if (vm_values_equal(src->items[i], dst->items[j])) { dup = true; break; }
        }
        if (!dup) pb_array_push(vm, dst, src->items[i]);
    }
    vm_pop_roots(vm, 1);
    *out = val_obj(dst);
    return true;
}

static void zr_flatten_into(PbVM *vm, PbArray *dst, const PbArray *src, int depth) {
    for (size_t i = 0; i < src->len; i++) {
        if (depth > 0 && is_obj_type(src->items[i], OBJ_ARRAY)) {
            zr_flatten_into(vm, dst, AS_ARRAY(src->items[i]), depth - 1);
        } else {
            pb_array_push(vm, dst, src->items[i]);
        }
    }
}

static bool bi_array_flatten(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc < 1 || argc > 2 || !is_obj_type(argv[0], OBJ_ARRAY)) return vm_error(vm, "array_flatten(arr[, depth]) expects array");
    int depth = (argc == 2 && argv[1].tag == VAL_NUM) ? (int)argv[1].as.num : 1;
    vm_push_root(vm, argv[0]);
    PbArray *dst = pb_array_new(vm, AS_ARRAY(argv[0])->len);
    vm_push_root(vm, val_obj(dst));
    zr_flatten_into(vm, dst, AS_ARRAY(argv[0]), depth);
    vm_pop_roots(vm, 2);
    *out = val_obj(dst);
    return true;
}

static bool bi_array_chunk(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 2 || !is_obj_type(argv[0], OBJ_ARRAY) || argv[1].tag != VAL_NUM || argv[1].as.num < 1.0) {
        return vm_error(vm, "array_chunk(arr, size) expects (array, positive integer)");
    }
    size_t csz = (size_t)argv[1].as.num;
    PbArray *src = AS_ARRAY(argv[0]);
    vm_push_root(vm, argv[0]);
    PbArray *outer = pb_array_new(vm, (src->len + csz - 1) / csz);
    vm_push_root(vm, val_obj(outer));
    for (size_t i = 0; i < src->len; i += csz) {
        size_t take = (i + csz <= src->len) ? csz : (src->len - i);
        PbArray *ch = pb_array_new(vm, take);
        for (size_t k = 0; k < take; k++) pb_array_push(vm, ch, src->items[i + k]);
        pb_array_push(vm, outer, val_obj(ch));
    }
    vm_pop_roots(vm, 2);
    *out = val_obj(outer);
    return true;
}

static bool bi_array_zip(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 2 || !is_obj_type(argv[0], OBJ_ARRAY) || !is_obj_type(argv[1], OBJ_ARRAY)) {
        return vm_error(vm, "array_zip(a, b) expects 2 arrays");
    }
    PbArray *a = AS_ARRAY(argv[0]), *b = AS_ARRAY(argv[1]);
    size_t n = a->len < b->len ? a->len : b->len;
    vm_push_root(vm, argv[0]);
    vm_push_root(vm, argv[1]);
    PbArray *res = pb_array_new(vm, n);
    vm_push_root(vm, val_obj(res));
    for (size_t i = 0; i < n; i++) {
        PbArray *pair = pb_array_new(vm, 2);
        pb_array_push(vm, pair, a->items[i]);
        pb_array_push(vm, pair, b->items[i]);
        pb_array_push(vm, res, val_obj(pair));
    }
    vm_pop_roots(vm, 3);
    *out = val_obj(res);
    return true;
}

static bool bi_array_take(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 2 || !is_obj_type(argv[0], OBJ_ARRAY) || argv[1].tag != VAL_NUM) {
        return vm_error(vm, "array_take(arr, n) expects (array, number)");
    }
    PbArray *src = AS_ARRAY(argv[0]);
    size_t n = argv[1].as.num <= 0.0 ? 0 : (size_t)argv[1].as.num;
    if (n > src->len) n = src->len;
    vm_push_root(vm, argv[0]);
    PbArray *dst = pb_array_new(vm, n);
    for (size_t i = 0; i < n; i++) pb_array_push(vm, dst, src->items[i]);
    vm_pop_roots(vm, 1);
    *out = val_obj(dst);
    return true;
}

static bool bi_array_drop(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 2 || !is_obj_type(argv[0], OBJ_ARRAY) || argv[1].tag != VAL_NUM) {
        return vm_error(vm, "array_drop(arr, n) expects (array, number)");
    }
    PbArray *src = AS_ARRAY(argv[0]);
    size_t skip = argv[1].as.num <= 0.0 ? 0 : (size_t)argv[1].as.num;
    if (skip > src->len) skip = src->len;
    vm_push_root(vm, argv[0]);
    PbArray *dst = pb_array_new(vm, src->len - skip);
    for (size_t i = skip; i < src->len; i++) pb_array_push(vm, dst, src->items[i]);
    vm_pop_roots(vm, 1);
    *out = val_obj(dst);
    return true;
}

static bool bi_range(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc < 1 || argc > 3) return vm_error(vm, "range([start,] stop[, step]) expects 1..3 arguments");
    double st = 0.0, en = 0.0, step = 1.0;
    if (argc == 1) {
        if (argv[0].tag != VAL_NUM) return vm_error(vm, "range(): expected numeric bound");
        en = argv[0].as.num;
    } else {
        if (argv[0].tag != VAL_NUM || argv[1].tag != VAL_NUM) return vm_error(vm, "range(): expected numeric bounds");
        st = argv[0].as.num;
        en = argv[1].as.num;
        if (argc == 3) {
            if (argv[2].tag != VAL_NUM || argv[2].as.num == 0.0) return vm_error(vm, "range(): step must be non-zero number");
            step = argv[2].as.num;
        }
    }
    PbArray *arr = pb_array_new(vm, 16);
    vm_push_root(vm, val_obj(arr));
    size_t guard = 0;
    if (step > 0.0) {
        for (double x = st; x < en; x += step) {
            if (++guard > 16777216u) { vm_pop_roots(vm, 1); return vm_error(vm, "range(): exceeds 16M element safety cap"); }
            pb_array_push(vm, arr, val_num(x));
        }
    } else {
        for (double x = st; x > en; x += step) {
            if (++guard > 16777216u) { vm_pop_roots(vm, 1); return vm_error(vm, "range(): exceeds 16M element safety cap"); }
            pb_array_push(vm, arr, val_num(x));
        }
    }
    vm_pop_roots(vm, 1);
    *out = val_obj(arr);
    return true;
}

static bool bi_enumerate(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 1 || !is_obj_type(argv[0], OBJ_ARRAY)) return vm_error(vm, "enumerate(arr) expects 1 array");
    PbArray *src = AS_ARRAY(argv[0]);
    vm_push_root(vm, argv[0]);
    PbArray *res = pb_array_new(vm, src->len);
    vm_push_root(vm, val_obj(res));
    for (size_t i = 0; i < src->len; i++) {
        PbArray *pair = pb_array_new(vm, 2);
        pb_array_push(vm, pair, val_num((double)i));
        pb_array_push(vm, pair, src->items[i]);
        pb_array_push(vm, res, val_obj(pair));
    }
    vm_pop_roots(vm, 2);
    *out = val_obj(res);
    return true;
}

static bool bi_map_keys(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 1) return vm_error(vm, "map_keys(m) expects 1 map");
    PbMap *m = NULL;
    if (is_obj_type(argv[0], OBJ_MAP)) m = AS_MAP(argv[0]);
    else if (is_obj_type(argv[0], OBJ_MODULE)) m = AS_MODULE(argv[0])->exports;
    else return vm_error(vm, "map_keys(): expected map, got %s", zr_val_type_name(argv[0]));
    size_t kc = 0;
    PbStr **ks = pb_map_sorted_keys(m, &kc);
    PbArray *arr = pb_array_new(vm, kc);
    for (size_t i = 0; i < kc; i++) pb_array_push(vm, arr, val_obj(ks[i]));
    free(ks);
    *out = val_obj(arr);
    return true;
}

static bool bi_map_has(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 2) return vm_error(vm, "map_has(m, key) expects 2 arguments");
    PbMap *m = NULL;
    if (is_obj_type(argv[0], OBJ_MAP)) m = AS_MAP(argv[0]);
    else if (is_obj_type(argv[0], OBJ_MODULE)) m = AS_MODULE(argv[0])->exports;
    else { *out = val_bool(false); return true; }
    PbStr *ks = is_obj_type(argv[1], OBJ_STR) ? AS_STR(argv[1]) : vm_value_to_pbstr(vm, argv[1]);
    *out = val_bool(pb_map_get(m, ks, NULL));
    return true;
}

static bool bi_has_member(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 2 || !is_obj_type(argv[1], OBJ_STR)) return vm_error(vm, "has_member(obj, name) expects (any, string)");
    PbStr *ks = AS_STR(argv[1]);
    if (is_obj_type(argv[0], OBJ_MAP)) {
        PbMap *m = AS_MAP(argv[0]);
        if (pb_map_get(m, ks, NULL)) { *out = val_bool(true); return true; }
        PbValue cls_v;
        int depth = 0;
        PbMap *cur = m;
        while (depth++ < 16 && pb_map_get_cstr(cur, "__class", &cls_v) && is_obj_type(cls_v, OBJ_MAP)) {
            cur = AS_MAP(cls_v);
            if (pb_map_get(cur, ks, NULL)) { *out = val_bool(true); return true; }
            PbValue sup_v;
            if (pb_map_get_cstr(cur, "__super", &sup_v) && is_obj_type(sup_v, OBJ_MAP)) cur = AS_MAP(sup_v);
            else break;
        }
        *out = val_bool(false);
        return true;
    }
    if (is_obj_type(argv[0], OBJ_MODULE)) {
        *out = val_bool(pb_map_get(AS_MODULE(argv[0])->exports, ks, NULL));
        return true;
    }
    *out = val_bool(false);
    return true;
}

static bool bi_map_remove(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 2 || !is_obj_type(argv[0], OBJ_MAP)) return vm_error(vm, "map_remove(m, key) expects (map, string)");
    PbMap *m = AS_MAP(argv[0]);
    if (m->obj.frozen) return vm_error(vm, "cannot mutate frozen map");
    PbStr *ks = is_obj_type(argv[1], OBJ_STR) ? AS_STR(argv[1]) : vm_value_to_pbstr(vm, argv[1]);
    PbValue removed = val_nil();
    (void)pb_map_delete(m, ks, &removed);
    *out = removed;
    return true;
}

/* ============================================================================
 * 3. Type Predicates, Immutability Freeze, Assertions, Errors & GC Builtins
 * ============================================================================ */

static bool bi_type(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 1) return vm_error(vm, "type() expects 1 argument");
    *out = val_obj(pb_str_c(vm, zr_val_type_name(argv[0])));
    return true;
}

static bool bi_type_assert(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 2 || !is_obj_type(argv[1], OBJ_STR)) return vm_error(vm, "type_assert(val, expected) expects (any, string)");
    const char *actual = zr_val_type_name(argv[0]);
    const char *expect = AS_STR(argv[1])->chars;
    if (strcmp(expect, "any") != 0 && strcmp(actual, expect) != 0) {
        return vm_error(vm, "TypeError: expected %s, got %s", expect, actual);
    }
    *out = argv[0];
    return true;
}

static void zr_freeze_rec(PbValue v, int depth) {
    if (depth > 64 || v.tag != VAL_OBJ || !v.as.obj) return;
    if (v.as.obj->frozen) return;
    v.as.obj->frozen = true;
    if (v.as.obj->type == OBJ_ARRAY) {
        PbArray *a = AS_ARRAY(v);
        for (size_t i = 0; i < a->len; i++) zr_freeze_rec(a->items[i], depth + 1);
    } else if (v.as.obj->type == OBJ_MAP) {
        PbMap *m = AS_MAP(v);
        for (size_t i = 0; i < m->used; i++) {
            if (m->entries[i].active) zr_freeze_rec(m->entries[i].val, depth + 1);
        }
    }
}

static bool bi_freeze(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 1) return vm_error(vm, "freeze() expects 1 argument");
    zr_freeze_rec(argv[0], 0);
    *out = argv[0];
    return true;
}

static bool bi_is_frozen(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 1) return vm_error(vm, "is_frozen() expects 1 argument");
    if (argv[0].tag != VAL_OBJ || !argv[0].as.obj) { *out = val_bool(true); return true; }
    if (argv[0].as.obj->type == OBJ_STR || argv[0].as.obj->type == OBJ_BIGINT ||
        argv[0].as.obj->type == OBJ_RATIONAL || argv[0].as.obj->type == OBJ_DECIMAL) {
        *out = val_bool(true);
        return true;
    }
    *out = val_bool(argv[0].as.obj->frozen);
    return true;
}

static bool bi_is_nil(PbVM *vm, int argc, PbValue *argv, PbValue *out)    { if (argc != 1) return vm_error(vm, "is_nil() expects 1 arg"); *out = val_bool(argv[0].tag == VAL_NIL); return true; }
static bool bi_is_bool(PbVM *vm, int argc, PbValue *argv, PbValue *out)   { if (argc != 1) return vm_error(vm, "is_bool() expects 1 arg"); *out = val_bool(argv[0].tag == VAL_BOOL); return true; }
static bool bi_is_number(PbVM *vm, int argc, PbValue *argv, PbValue *out) { if (argc != 1) return vm_error(vm, "is_number() expects 1 arg"); *out = val_bool(argv[0].tag == VAL_NUM); return true; }
static bool bi_is_string(PbVM *vm, int argc, PbValue *argv, PbValue *out) { if (argc != 1) return vm_error(vm, "is_string() expects 1 arg"); *out = val_bool(is_obj_type(argv[0], OBJ_STR)); return true; }
static bool bi_is_array(PbVM *vm, int argc, PbValue *argv, PbValue *out)  { if (argc != 1) return vm_error(vm, "is_array() expects 1 arg"); *out = val_bool(is_obj_type(argv[0], OBJ_ARRAY)); return true; }
static bool bi_is_map(PbVM *vm, int argc, PbValue *argv, PbValue *out)    { if (argc != 1) return vm_error(vm, "is_map() expects 1 arg"); *out = val_bool(is_obj_type(argv[0], OBJ_MAP)); return true; }
static bool bi_is_fn(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 1) return vm_error(vm, "is_fn() expects 1 arg");
    bool ok = argv[0].tag == VAL_BUILTIN || is_obj_type(argv[0], OBJ_CLOSURE) || is_obj_type(argv[0], OBJ_FUNC);
    *out = val_bool(ok);
    return true;
}

static bool bi_assert(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc < 1 || argc > 2) return vm_error(vm, "assert(cond[, msg]) expects 1 or 2 arguments");
    if (!val_truthy(argv[0])) {
        if (argc == 2) {
            char *m = vm_value_to_cstr(vm, argv[1], NULL);
            vm_error(vm, "AssertionError: %s", m);
            free(m);
            return false;
        }
        return vm_error(vm, "AssertionError: assertion failed");
    }
    *out = val_bool(true);
    return true;
}

static bool bi_error(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    (void)out;
    if (argc < 1) return vm_error(vm, "RuntimeError");
    char *m = vm_value_to_cstr(vm, argv[0], NULL);
    vm_error(vm, "%s", m);
    free(m);
    return false;
}

static bool bi_gc_collect(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    (void)argc; (void)argv;
    size_t fa = 0, fm = 0, fmod = 0;
    vm_collect_garbage(vm, &fa, &fm, &fmod);
    char buf[160];
    snprintf(buf, sizeof(buf), "gc_collect: cycles arrays=%zu maps=%zu modules=%zu", fa, fm, fmod);
    *out = val_obj(pb_str_c(vm, buf));
    return true;
}

static bool bi_gc_stats(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    (void)argc; (void)argv;
    char buf[320];
    snprintf(buf, sizeof(buf),
             "string_allocs=%zu array_allocs=%zu map_allocs=%zu module_allocs=%zu live_tracked=%zu collections=%zu nursery_collections=%zu full_collections=%zu",
             vm->string_allocs, vm->array_allocs, vm->map_allocs, vm->module_allocs,
             vm->obj_count, vm->gc_collections, vm->nursery_collections, vm->full_collections);
    *out = val_obj(pb_str_c(vm, buf));
    return true;
}

static bool bi_gc_stats_map(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    (void)argc; (void)argv;
    PbMap *m = pb_map_new_raw(vm, 12);
    vm_push_root(vm, val_obj(m));
    pb_map_set_cstr(vm, m, "string_allocs", val_num((double)vm->string_allocs));
    pb_map_set_cstr(vm, m, "array_allocs", val_num((double)vm->array_allocs));
    pb_map_set_cstr(vm, m, "map_allocs", val_num((double)vm->map_allocs));
    pb_map_set_cstr(vm, m, "module_allocs", val_num((double)vm->module_allocs));
    pb_map_set_cstr(vm, m, "live_tracked", val_num((double)vm->obj_count));
    pb_map_set_cstr(vm, m, "collections", val_num((double)vm->gc_collections));
    pb_map_set_cstr(vm, m, "nursery_collections", val_num((double)vm->nursery_collections));
    pb_map_set_cstr(vm, m, "full_collections", val_num((double)vm->full_collections));
    vm_pop_roots(vm, 1);
    *out = val_obj(m);
    return true;
}

/* ============================================================================
 * 4. Portable Zero-Dependency C99 Regex Engine (UTF-8 + Capturing Groups)
 * ============================================================================ */

#define ZR_RE_MAX_GROUPS 16
#define ZR_RE_STEP_LIMIT 500000

typedef enum {
    RE_ANY = 1,
    RE_CHAR,
    RE_CLASS,
    RE_BOL,
    RE_EOL,
    RE_WB,
    RE_NWB,
    RE_GROUP,
    RE_ALT,
    RE_SEQ
} ZrReNodeKind;

typedef struct ZrReNode ZrReNode;

typedef struct {
    uint32_t lo;
    uint32_t hi;
} ZrReRange;

struct ZrReNode {
    ZrReNodeKind kind;
    uint32_t     ch;
    bool         neg_class;
    ZrReRange   *ranges;
    size_t       n_ranges;
    uint8_t      shorthand; /* 'd','D','w','W','s','S', or 0 */
    int          group_idx; /* -1 for (?:...), 1..15 for (...) */
    ZrReNode   **kids;
    size_t       n_kids;
    int          qmin;
    int          qmax;      /* -1 = unbounded */
    bool         lazy;
};

typedef struct {
    const char *pat;
    size_t      len;
    size_t      pos;
    int         next_group;
    bool        case_insens;
    ZrArena     arena;
    char        err[128];
} ZrReCompiler;

static bool zr_re_is_word(uint32_t cp) {
    return (cp >= 'a' && cp <= 'z') || (cp >= 'A' && cp <= 'Z') ||
           (cp >= '0' && cp <= '9') || cp == '_' || zr_cp_is_ident_cont(cp);
}

static bool zr_re_is_digit(uint32_t cp) { return cp >= '0' && cp <= '9'; }
static bool zr_re_is_space(uint32_t cp) { return zr_is_unicode_space(cp); }

static ZrReNode *zr_re_alloc(ZrReCompiler *rc, ZrReNodeKind k) {
    ZrReNode *n = (ZrReNode *)zr_arena_alloc(&rc->arena, sizeof(ZrReNode));
    memset(n, 0, sizeof(*n));
    n->kind = k;
    n->qmin = 1;
    n->qmax = 1;
    n->group_idx = -1;
    return n;
}

static uint32_t zr_re_next_cp(const char *s, size_t len, size_t *pos) {
    size_t adv = 1;
    uint32_t cp = zr_utf8_decode(s + *pos, len - *pos, &adv);
    *pos += adv ? adv : 1;
    return cp;
}

static uint32_t zr_re_parse_esc_char(ZrReCompiler *rc, uint8_t *out_short) {
    *out_short = 0;
    if (rc->pos >= rc->len) return '\\';
    uint32_t c = zr_re_next_cp(rc->pat, rc->len, &rc->pos);
    switch (c) {
        case 'n': return '\n';
        case 'r': return '\r';
        case 't': return '\t';
        case '0': return 0;
        case 'd': case 'D': case 'w': case 'W': case 's': case 'S':
            *out_short = (uint8_t)c;
            return 0;
        case 'x': {
            if (rc->pos + 2 <= rc->len) {
                int h1 = zr_hex_val(rc->pat[rc->pos]), h2 = zr_hex_val(rc->pat[rc->pos + 1]);
                if (h1 >= 0 && h2 >= 0) { rc->pos += 2; return (uint32_t)((h1 << 4) | h2); }
            }
            return 'x';
        }
        default: return c;
    }
}

static ZrReNode *zr_re_parse_alt(ZrReCompiler *rc);

static ZrReNode *zr_re_parse_class(ZrReCompiler *rc) {
    ZrReNode *n = zr_re_alloc(rc, RE_CLASS);
    if (rc->pos < rc->len && rc->pat[rc->pos] == '^') {
        n->neg_class = true;
        rc->pos++;
    }
    ZrReRange tmp[128];
    size_t nr = 0;
    bool first = true;
    while (rc->pos < rc->len) {
        if (!first && rc->pat[rc->pos] == ']') { rc->pos++; break; }
        first = false;
        uint8_t sh1 = 0;
        uint32_t c1 = (rc->pat[rc->pos] == '\\') ? (rc->pos++, zr_re_parse_esc_char(rc, &sh1)) : zr_re_next_cp(rc->pat, rc->len, &rc->pos);
        if (sh1) {
            if (sh1 == 'd' && nr < 126) { tmp[nr].lo = '0'; tmp[nr].hi = '9'; nr++; }
            else if (sh1 == 's' && nr < 120) {
                tmp[nr++] = (ZrReRange){' ', ' '}; tmp[nr++] = (ZrReRange){'\t', '\r'};
            } else if (sh1 == 'w' && nr < 120) {
                tmp[nr++] = (ZrReRange){'a', 'z'}; tmp[nr++] = (ZrReRange){'A', 'Z'};
                tmp[nr++] = (ZrReRange){'0', '9'}; tmp[nr++] = (ZrReRange){'_', '_'};
            } else {
                n->shorthand = sh1;
            }
            continue;
        }
        if (rc->pos + 1 < rc->len && rc->pat[rc->pos] == '-' && rc->pat[rc->pos + 1] != ']') {
            rc->pos++;
            uint8_t sh2 = 0;
            uint32_t c2 = (rc->pat[rc->pos] == '\\') ? (rc->pos++, zr_re_parse_esc_char(rc, &sh2)) : zr_re_next_cp(rc->pat, rc->len, &rc->pos);
            if (nr < 127) { tmp[nr].lo = c1 <= c2 ? c1 : c2; tmp[nr].hi = c1 <= c2 ? c2 : c1; nr++; }
        } else {
            if (nr < 127) { tmp[nr].lo = c1; tmp[nr].hi = c1; nr++; }
        }
    }
    if (nr > 0) {
        n->ranges = (ZrReRange *)zr_arena_alloc(&rc->arena, nr * sizeof(ZrReRange));
        memcpy(n->ranges, tmp, nr * sizeof(ZrReRange));
        n->n_ranges = nr;
    }
    return n;
}

static ZrReNode *zr_re_parse_atom(ZrReCompiler *rc) {
    if (rc->pos >= rc->len) return NULL;
    char c = rc->pat[rc->pos];
    if (c == ')' || c == '|') return NULL;
    if (c == '^') { rc->pos++; return zr_re_alloc(rc, RE_BOL); }
    if (c == '$') { rc->pos++; return zr_re_alloc(rc, RE_EOL); }
    if (c == '.') { rc->pos++; return zr_re_alloc(rc, RE_ANY); }
    if (c == '[') { rc->pos++; return zr_re_parse_class(rc); }
    if (c == '(') {
        rc->pos++;
        int gidx = -1;
        if (rc->pos + 1 < rc->len && rc->pat[rc->pos] == '?') {
            if (rc->pat[rc->pos + 1] == ':') {
                rc->pos += 2;
            } else if (rc->pat[rc->pos + 1] == 'i' && rc->pos + 2 < rc->len && rc->pat[rc->pos + 2] == ')') {
                rc->pos += 3;
                rc->case_insens = true;
                return zr_re_alloc(rc, RE_SEQ);
            } else if (rc->pat[rc->pos + 1] == 'i' && rc->pos + 2 < rc->len && rc->pat[rc->pos + 2] == ':') {
                rc->pos += 3;
                rc->case_insens = true;
            }
        } else {
            if (rc->next_group < ZR_RE_MAX_GROUPS) gidx = rc->next_group++;
        }
        ZrReNode *inner = zr_re_parse_alt(rc);
        if (!inner) { snprintf(rc->err, sizeof(rc->err), "invalid regex group"); return NULL; }
        if (rc->pos >= rc->len || rc->pat[rc->pos] != ')') {
            snprintf(rc->err, sizeof(rc->err), "unclosed '(' in regex");
            return NULL;
        }
        rc->pos++;
        ZrReNode *g = zr_re_alloc(rc, RE_GROUP);
        g->group_idx = gidx;
        g->kids = (ZrReNode **)zr_arena_alloc(&rc->arena, sizeof(ZrReNode *));
        g->kids[0] = inner;
        g->n_kids = 1;
        return g;
    }
    if (c == '\\') {
        rc->pos++;
        if (rc->pos < rc->len && (rc->pat[rc->pos] == 'b' || rc->pat[rc->pos] == 'B')) {
            char wb = rc->pat[rc->pos++];
            return zr_re_alloc(rc, wb == 'b' ? RE_WB : RE_NWB);
        }
        uint8_t sh = 0;
        uint32_t ec = zr_re_parse_esc_char(rc, &sh);
        if (sh) {
            ZrReNode *cl = zr_re_alloc(rc, RE_CLASS);
            cl->shorthand = sh;
            return cl;
        }
        ZrReNode *ch = zr_re_alloc(rc, RE_CHAR);
        ch->ch = ec;
        return ch;
    }
    if (c == '*' || c == '+' || c == '?') {
        snprintf(rc->err, sizeof(rc->err), "unquantified '%c' in regex", c);
        return NULL;
    }
    ZrReNode *ch = zr_re_alloc(rc, RE_CHAR);
    ch->ch = zr_re_next_cp(rc->pat, rc->len, &rc->pos);
    return ch;
}

static ZrReNode *zr_re_parse_piece(ZrReCompiler *rc) {
    ZrReNode *a = zr_re_parse_atom(rc);
    if (!a || rc->err[0]) return NULL;
    if (rc->pos < rc->len) {
        char q = rc->pat[rc->pos];
        if (q == '*') { a->qmin = 0; a->qmax = -1; rc->pos++; }
        else if (q == '+') { a->qmin = 1; a->qmax = -1; rc->pos++; }
        else if (q == '?') { a->qmin = 0; a->qmax = 1; rc->pos++; }
        else if (q == '{') {
            size_t save = rc->pos + 1;
            int mn = 0, mx = 0;
            bool has_d = false;
            while (save < rc->len && isdigit((unsigned char)rc->pat[save])) {
                mn = mn * 10 + (rc->pat[save++] - '0');
                has_d = true;
            }
            if (has_d && save < rc->len) {
                if (rc->pat[save] == '}') {
                    a->qmin = mn; a->qmax = mn; rc->pos = save + 1;
                } else if (rc->pat[save] == ',') {
                    save++;
                    if (save < rc->len && rc->pat[save] == '}') {
                        a->qmin = mn; a->qmax = -1; rc->pos = save + 1;
                    } else {
                        bool hd2 = false;
                        while (save < rc->len && isdigit((unsigned char)rc->pat[save])) {
                            mx = mx * 10 + (rc->pat[save++] - '0');
                            hd2 = true;
                        }
                        if (hd2 && save < rc->len && rc->pat[save] == '}') {
                            a->qmin = mn; a->qmax = mx; rc->pos = save + 1;
                        }
                    }
                }
            }
        }
        if ((a->qmax == -1 || a->qmax > a->qmin) && rc->pos < rc->len && rc->pat[rc->pos] == '?') {
            a->lazy = true;
            rc->pos++;
        }
    }
    return a;
}

static ZrReNode *zr_re_parse_seq(ZrReCompiler *rc) {
    ZrReNode *items[256];
    size_t n = 0;
    while (rc->pos < rc->len && rc->pat[rc->pos] != ')' && rc->pat[rc->pos] != '|') {
        ZrReNode *p = zr_re_parse_piece(rc);
        if (rc->err[0]) return NULL;
        if (!p) break;
        if (n < 256) items[n++] = p;
    }
    ZrReNode *seq = zr_re_alloc(rc, RE_SEQ);
    if (n > 0) {
        seq->kids = (ZrReNode **)zr_arena_alloc(&rc->arena, n * sizeof(ZrReNode *));
        memcpy(seq->kids, items, n * sizeof(ZrReNode *));
        seq->n_kids = n;
    }
    return seq;
}

static ZrReNode *zr_re_parse_alt(ZrReCompiler *rc) {
    ZrReNode *branches[64];
    size_t nb = 0;
    ZrReNode *first = zr_re_parse_seq(rc);
    if (!first || rc->err[0]) return NULL;
    branches[nb++] = first;
    while (rc->pos < rc->len && rc->pat[rc->pos] == '|') {
        rc->pos++;
        ZrReNode *nx = zr_re_parse_seq(rc);
        if (!nx || rc->err[0]) return NULL;
        if (nb < 64) branches[nb++] = nx;
    }
    if (nb == 1) return branches[0];
    ZrReNode *alt = zr_re_alloc(rc, RE_ALT);
    alt->kids = (ZrReNode **)zr_arena_alloc(&rc->arena, nb * sizeof(ZrReNode *));
    memcpy(alt->kids, branches, nb * sizeof(ZrReNode *));
    alt->n_kids = nb;
    return alt;
}

typedef struct {
    const char *s;
    size_t      len;
    bool        case_insens;
    int         steps;
    ssize_t     g_start[ZR_RE_MAX_GROUPS];
    ssize_t     g_end[ZR_RE_MAX_GROUPS];
} ZrReExec;

static bool zr_re_match_class(const ZrReNode *n, uint32_t cp, bool ci) {
    uint32_t tcp = ci ? zr_cp_to_lower(cp) : cp;
    bool hit = false;
    if (n->shorthand) {
        switch (n->shorthand) {
            case 'd': hit = zr_re_is_digit(cp); break;
            case 'D': hit = !zr_re_is_digit(cp); break;
            case 'w': hit = zr_re_is_word(cp); break;
            case 'W': hit = !zr_re_is_word(cp); break;
            case 's': hit = zr_re_is_space(cp); break;
            case 'S': hit = !zr_re_is_space(cp); break;
        }
    }
    for (size_t i = 0; i < n->n_ranges && !hit; i++) {
        uint32_t lo = ci ? zr_cp_to_lower(n->ranges[i].lo) : n->ranges[i].lo;
        uint32_t hi = ci ? zr_cp_to_lower(n->ranges[i].hi) : n->ranges[i].hi;
        if (tcp >= lo && tcp <= hi) hit = true;
    }
    return n->neg_class ? !hit : hit;
}

static bool zr_re_at_word_boundary(const char *s, size_t len, size_t pos) {
    bool left_w = false, right_w = false;
    if (pos > 0) {
        size_t p = pos - 1;
        while (p > 0 && ((uint8_t)s[p] & 0xC0u) == 0x80u) p--;
        size_t adv = 1;
        left_w = zr_re_is_word(zr_utf8_decode(s + p, pos - p, &adv));
    }
    if (pos < len) {
        size_t adv = 1;
        right_w = zr_re_is_word(zr_utf8_decode(s + pos, len - pos, &adv));
    }
    return left_w != right_w;
}

static bool zr_re_eval_seq(ZrReExec *ex, ZrReNode **kids, size_t idx, size_t n_kids, size_t pos, size_t *out_end);
static bool zr_re_eval_single(ZrReExec *ex, const ZrReNode *n, size_t pos, size_t *out_end);

static bool zr_re_eval_node_cont(ZrReExec *ex, ZrReNode **kids, size_t idx, size_t n_kids, int rep, size_t pos, size_t *out_end) {
    if (++ex->steps > ZR_RE_STEP_LIMIT) return false;
    const ZrReNode *n = kids[idx];
    if (n->qmax != -1 && rep >= n->qmax) {
        return zr_re_eval_seq(ex, kids, idx + 1, n_kids, pos, out_end);
    }
    if (rep < n->qmin) {
        size_t mid = pos;
        if (!zr_re_eval_single(ex, n, pos, &mid)) return false;
        return zr_re_eval_node_cont(ex, kids, idx, n_kids, rep + 1, mid, out_end);
    }
    ssize_t save_gs[ZR_RE_MAX_GROUPS], save_ge[ZR_RE_MAX_GROUPS];
    memcpy(save_gs, ex->g_start, sizeof(save_gs));
    memcpy(save_ge, ex->g_end, sizeof(save_ge));

    if (n->lazy) {
        if (zr_re_eval_seq(ex, kids, idx + 1, n_kids, pos, out_end)) return true;
        memcpy(ex->g_start, save_gs, sizeof(save_gs));
        memcpy(ex->g_end, save_ge, sizeof(save_ge));
        size_t mid = pos;
        if (zr_re_eval_single(ex, n, pos, &mid) && mid > pos) {
            if (zr_re_eval_node_cont(ex, kids, idx, n_kids, rep + 1, mid, out_end)) return true;
        }
        memcpy(ex->g_start, save_gs, sizeof(save_gs));
        memcpy(ex->g_end, save_ge, sizeof(save_ge));
        return false;
    } else {
        size_t mid = pos;
        if (zr_re_eval_single(ex, n, pos, &mid) && (mid > pos || rep < n->qmin)) {
            if (zr_re_eval_node_cont(ex, kids, idx, n_kids, rep + 1, mid, out_end)) return true;
        }
        memcpy(ex->g_start, save_gs, sizeof(save_gs));
        memcpy(ex->g_end, save_ge, sizeof(save_ge));
        return zr_re_eval_seq(ex, kids, idx + 1, n_kids, pos, out_end);
    }
}

static bool zr_re_eval_seq(ZrReExec *ex, ZrReNode **kids, size_t idx, size_t n_kids, size_t pos, size_t *out_end) {
    if (idx >= n_kids) { *out_end = pos; return true; }
    return zr_re_eval_node_cont(ex, kids, idx, n_kids, 0, pos, out_end);
}

static bool zr_re_eval_single(ZrReExec *ex, const ZrReNode *n, size_t pos, size_t *out_end) {
    if (++ex->steps > ZR_RE_STEP_LIMIT) return false;
    switch (n->kind) {
        case RE_BOL:
            if (pos == 0) { *out_end = pos; return true; }
            return false;
        case RE_EOL:
            if (pos == ex->len) { *out_end = pos; return true; }
            return false;
        case RE_WB:
            if (zr_re_at_word_boundary(ex->s, ex->len, pos)) { *out_end = pos; return true; }
            return false;
        case RE_NWB:
            if (!zr_re_at_word_boundary(ex->s, ex->len, pos)) { *out_end = pos; return true; }
            return false;
        case RE_ANY: {
            if (pos >= ex->len) return false;
            size_t adv = 1;
            uint32_t cp = zr_utf8_decode(ex->s + pos, ex->len - pos, &adv);
            if (cp == '\n') return false;
            *out_end = pos + (adv ? adv : 1);
            return true;
        }
        case RE_CHAR: {
            if (pos >= ex->len) return false;
            size_t adv = 1;
            uint32_t cp = zr_utf8_decode(ex->s + pos, ex->len - pos, &adv);
            bool eq = ex->case_insens ? (zr_cp_to_lower(cp) == zr_cp_to_lower(n->ch)) : (cp == n->ch);
            if (!eq) return false;
            *out_end = pos + (adv ? adv : 1);
            return true;
        }
        case RE_CLASS: {
            if (pos >= ex->len) return false;
            size_t adv = 1;
            uint32_t cp = zr_utf8_decode(ex->s + pos, ex->len - pos, &adv);
            if (!zr_re_match_class(n, cp, ex->case_insens)) return false;
            *out_end = pos + (adv ? adv : 1);
            return true;
        }
        case RE_SEQ:
            return zr_re_eval_seq(ex, n->kids, 0, n->n_kids, pos, out_end);
        case RE_ALT: {
            ssize_t s_gs[ZR_RE_MAX_GROUPS], s_ge[ZR_RE_MAX_GROUPS];
            memcpy(s_gs, ex->g_start, sizeof(s_gs));
            memcpy(s_ge, ex->g_end, sizeof(s_ge));
            for (size_t i = 0; i < n->n_kids; i++) {
                ZrReNode *one[1] = { n->kids[i] };
                if (zr_re_eval_seq(ex, one, 0, 1, pos, out_end)) return true;
                memcpy(ex->g_start, s_gs, sizeof(s_gs));
                memcpy(ex->g_end, s_ge, sizeof(s_ge));
            }
            return false;
        }
        case RE_GROUP: {
            ZrReNode *one[1] = { n->kids[0] };
            size_t ge = pos;
            if (!zr_re_eval_seq(ex, one, 0, 1, pos, &ge)) return false;
            if (n->group_idx >= 1 && n->group_idx < ZR_RE_MAX_GROUPS) {
                ex->g_start[n->group_idx] = (ssize_t)pos;
                ex->g_end[n->group_idx] = (ssize_t)ge;
            }
            *out_end = ge;
            return true;
        }
    }
    return false;
}

static bool zr_re_compile_entry(PbVM *vm, const PbStr *pat, ZrReCompiler *rc, ZrReNode **out_root) {
    memset(rc, 0, sizeof(*rc));
    rc->pat = pat->chars;
    rc->len = pat->len;
    rc->next_group = 1;
    zr_arena_init(&rc->arena);
    ZrReNode *r = zr_re_parse_alt(rc);
    if (!r || rc->err[0] || rc->pos < rc->len) {
        zr_arena_free(&rc->arena);
        return vm_error(vm, "regex error: %s", rc->err[0] ? rc->err : "unexpected ')' in pattern");
    }
    *out_root = r;
    return true;
}

static bool zr_re_search_from(const ZrReCompiler *rc, ZrReNode *root, const char *s, size_t len, size_t start_pos, ZrReExec *ex_out) {
    size_t pos = start_pos;
    for (;;) {
        memset(ex_out, 0, sizeof(*ex_out));
        ex_out->s = s;
        ex_out->len = len;
        ex_out->case_insens = rc->case_insens;
        for (int g = 0; g < ZR_RE_MAX_GROUPS; g++) { ex_out->g_start[g] = -1; ex_out->g_end[g] = -1; }
        ZrReNode *one[1] = { root };
        size_t end_pos = pos;
        if (zr_re_eval_seq(ex_out, one, 0, 1, pos, &end_pos)) {
            ex_out->g_start[0] = (ssize_t)pos;
            ex_out->g_end[0] = (ssize_t)end_pos;
            return true;
        }
        if (pos >= len) break;
        size_t adv = 1;
        (void)zr_utf8_decode(s + pos, len - pos, &adv);
        pos += adv ? adv : 1;
    }
    return false;
}

static bool bi_regex_match(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 2 || !is_obj_type(argv[0], OBJ_STR) || !is_obj_type(argv[1], OBJ_STR)) {
        return vm_error(vm, "regex_match(s, pattern) expects 2 strings");
    }
    ZrReCompiler rc; ZrReNode *root = NULL;
    if (!zr_re_compile_entry(vm, AS_STR(argv[1]), &rc, &root)) return false;
    ZrReExec ex;
    bool ok = zr_re_search_from(&rc, root, AS_STR(argv[0])->chars, AS_STR(argv[0])->len, 0, &ex);
    zr_arena_free(&rc.arena);
    *out = val_bool(ok);
    return true;
}

static bool bi_regex_find(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 2 || !is_obj_type(argv[0], OBJ_STR) || !is_obj_type(argv[1], OBJ_STR)) {
        return vm_error(vm, "regex_find(s, pattern) expects 2 strings");
    }
    ZrReCompiler rc; ZrReNode *root = NULL;
    if (!zr_re_compile_entry(vm, AS_STR(argv[1]), &rc, &root)) return false;
    PbStr *s = AS_STR(argv[0]);
    ZrReExec ex;
    if (zr_re_search_from(&rc, root, s->chars, s->len, 0, &ex)) {
        zr_arena_free(&rc.arena);
        vm_push_root(vm, argv[0]);
        PbStr *m = pb_str_new(vm, s->chars + ex.g_start[0], (size_t)(ex.g_end[0] - ex.g_start[0]));
        vm_pop_roots(vm, 1);
        *out = val_obj(m);
    } else {
        zr_arena_free(&rc.arena);
        *out = val_nil();
    }
    return true;
}

static bool bi_regex_find_all(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 2 || !is_obj_type(argv[0], OBJ_STR) || !is_obj_type(argv[1], OBJ_STR)) {
        return vm_error(vm, "regex_find_all(s, pattern) expects 2 strings");
    }
    ZrReCompiler rc; ZrReNode *root = NULL;
    if (!zr_re_compile_entry(vm, AS_STR(argv[1]), &rc, &root)) return false;
    vm_push_root(vm, argv[0]);
    PbArray *arr = pb_array_new(vm, 4);
    vm_push_root(vm, val_obj(arr));
    size_t pos = 0;
    while (pos <= AS_STR(argv[0])->len) {
        PbStr *s = AS_STR(argv[0]);
        ZrReExec ex;
        if (!zr_re_search_from(&rc, root, s->chars, s->len, pos, &ex)) break;
        PbStr *m = pb_str_new(vm, s->chars + ex.g_start[0], (size_t)(ex.g_end[0] - ex.g_start[0]));
        pb_array_push(vm, arr, val_obj(m));
        if (ex.g_end[0] == (ssize_t)pos) {
            if (pos >= AS_STR(argv[0])->len) break;
            size_t adv = 1;
            (void)zr_utf8_decode(AS_STR(argv[0])->chars + pos, AS_STR(argv[0])->len - pos, &adv);
            pos += adv ? adv : 1;
        } else {
            pos = (size_t)ex.g_end[0];
        }
    }
    zr_arena_free(&rc.arena);
    vm_pop_roots(vm, 2);
    *out = val_obj(arr);
    return true;
}

static void zr_re_append_rep(ZrBuf *b, const char *src, const ZrReExec *ex, int n_groups, const PbStr *rep) {
    for (size_t i = 0; i < rep->len; i++) {
        if (rep->chars[i] == '$' && i + 1 < rep->len) {
            char nx = rep->chars[i + 1];
            if (nx == '$') { zr_buf_push(b, '$'); i++; continue; }
            if (nx == '{') {
                size_t k = i + 2;
                int g = 0;
                bool has_d = false;
                while (k < rep->len && isdigit((unsigned char)rep->chars[k])) {
                    g = g * 10 + (rep->chars[k] - '0');
                    has_d = true;
                    k++;
                }
                if (has_d && k < rep->len && rep->chars[k] == '}') {
                    i = k;
                    if (g < n_groups && ex->g_start[g] >= 0 && ex->g_end[g] >= ex->g_start[g]) {
                        zr_buf_append(b, src + ex->g_start[g], (size_t)(ex->g_end[g] - ex->g_start[g]));
                    }
                    continue;
                }
            }
            if (isdigit((unsigned char)nx)) {
                int g = 0;
                size_t k = i + 1;
                while (k < rep->len && isdigit((unsigned char)rep->chars[k])) {
                    g = g * 10 + (rep->chars[k] - '0');
                    k++;
                }
                i = k - 1;
                if (g < n_groups && ex->g_start[g] >= 0 && ex->g_end[g] >= ex->g_start[g]) {
                    zr_buf_append(b, src + ex->g_start[g], (size_t)(ex->g_end[g] - ex->g_start[g]));
                }
                continue;
            }
        }
        zr_buf_push(b, rep->chars[i]);
    }
}

static bool zr_regex_replace_impl(PbVM *vm, int argc, PbValue *argv, bool all, PbValue *out) {
    if (argc != 3 || !is_obj_type(argv[0], OBJ_STR) || !is_obj_type(argv[1], OBJ_STR) || !is_obj_type(argv[2], OBJ_STR)) {
        return vm_error(vm, "%s(s, pat, rep) expects 3 strings", all ? "regex_replace_all" : "regex_replace");
    }
    ZrReCompiler rc; ZrReNode *root = NULL;
    if (!zr_re_compile_entry(vm, AS_STR(argv[1]), &rc, &root)) return false;
    PbStr *s = AS_STR(argv[0]), *rep = AS_STR(argv[2]);
    ZrBuf b;
    zr_buf_init(&b);
    size_t pos = 0;
    while (pos <= s->len) {
        ZrReExec ex;
        if (!zr_re_search_from(&rc, root, s->chars, s->len, pos, &ex)) {
            zr_buf_append(&b, s->chars + pos, s->len - pos);
            break;
        }
        zr_buf_append(&b, s->chars + pos, (size_t)ex.g_start[0] - pos);
        zr_re_append_rep(&b, s->chars, &ex, rc.next_group, rep);
        if (!all) {
            zr_buf_append(&b, s->chars + ex.g_end[0], s->len - (size_t)ex.g_end[0]);
            break;
        }
        if (ex.g_end[0] == (ssize_t)pos) {
            if (pos >= s->len) break;
            size_t adv = 1;
            (void)zr_utf8_decode(s->chars + pos, s->len - pos, &adv);
            adv = adv ? adv : 1;
            zr_buf_append(&b, s->chars + pos, adv);
            pos += adv;
        } else {
            pos = (size_t)ex.g_end[0];
        }
    }
    zr_arena_free(&rc.arena);
    *out = val_obj(pb_str_new(vm, b.data ? b.data : "", b.len));
    zr_buf_free(&b);
    return true;
}

static bool bi_regex_replace(PbVM *vm, int argc, PbValue *argv, PbValue *out) { return zr_regex_replace_impl(vm, argc, argv, false, out); }
static bool bi_regex_replace_all(PbVM *vm, int argc, PbValue *argv, PbValue *out) { return zr_regex_replace_impl(vm, argc, argv, true, out); }

static bool bi_regex_split(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 2 || !is_obj_type(argv[0], OBJ_STR) || !is_obj_type(argv[1], OBJ_STR)) {
        return vm_error(vm, "regex_split(s, pattern) expects 2 strings");
    }
    ZrReCompiler rc; ZrReNode *root = NULL;
    if (!zr_re_compile_entry(vm, AS_STR(argv[1]), &rc, &root)) return false;
    vm_push_root(vm, argv[0]);
    PbArray *arr = pb_array_new(vm, 4);
    vm_push_root(vm, val_obj(arr));
    size_t pos = 0;
    for (;;) {
        PbStr *s = AS_STR(argv[0]);
        ZrReExec ex;
        if (!zr_re_search_from(&rc, root, s->chars, s->len, pos, &ex) || ex.g_end[0] == ex.g_start[0]) {
            PbStr *tail = pb_str_new(vm, s->chars + pos, s->len - pos);
            pb_array_push(vm, arr, val_obj(tail));
            break;
        }
        PbStr *part = pb_str_new(vm, s->chars + pos, (size_t)ex.g_start[0] - pos);
        pb_array_push(vm, arr, val_obj(part));
        pos = (size_t)ex.g_end[0];
    }
    zr_arena_free(&rc.arena);
    vm_pop_roots(vm, 2);
    *out = val_obj(arr);
    return true;
}

static bool bi_regex_capture(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 2 || !is_obj_type(argv[0], OBJ_STR) || !is_obj_type(argv[1], OBJ_STR)) {
        return vm_error(vm, "regex_capture(s, pattern) expects 2 strings");
    }
    ZrReCompiler rc; ZrReNode *root = NULL;
    if (!zr_re_compile_entry(vm, AS_STR(argv[1]), &rc, &root)) return false;
    ZrReExec ex;
    if (!zr_re_search_from(&rc, root, AS_STR(argv[0])->chars, AS_STR(argv[0])->len, 0, &ex)) {
        zr_arena_free(&rc.arena);
        *out = val_nil();
        return true;
    }
    int ng = rc.next_group;
    zr_arena_free(&rc.arena);
    vm_push_root(vm, argv[0]);
    PbArray *arr = pb_array_new(vm, (size_t)ng);
    vm_push_root(vm, val_obj(arr));
    for (int g = 0; g < ng; g++) {
        if (ex.g_start[g] >= 0 && ex.g_end[g] >= ex.g_start[g]) {
            PbStr * gs = pb_str_new(vm, AS_STR(argv[0])->chars + ex.g_start[g], (size_t)(ex.g_end[g] - ex.g_start[g]));
            pb_array_push(vm, arr, val_obj(gs));
        } else {
            pb_array_push(vm, arr, val_nil());
        }
    }
    vm_pop_roots(vm, 2);
    *out = val_obj(arr);
    return true;
}

/* ============================================================================
 * 5. True GIL-Free OS Multithreading, Channels, Atomics & Synchronization
 * ============================================================================ */

typedef enum {
    XF_NIL = 0,
    XF_BOOL,
    XF_NUM,
    XF_STR,
    XF_ARR,
    XF_MAP,
    XF_BIGINT,
    XF_RATIONAL,
    XF_DECIMAL,
    XF_CLOSURE,
    XF_MODULE,
    XF_BUILTIN,
    XF_BYTES,
    XF_F64BUF
} ZrXferTag;

typedef struct ZrXferVal ZrXferVal;

struct ZrXferVal {
    ZrXferTag tag;
    uint8_t   taint;
    union {
        bool     b;
        double   num;
        struct { char *s; size_t len; } str;
        struct { ZrXferVal *items; size_t len; } arr;
        struct { char **keys; size_t *klens; ZrXferVal *vals; size_t len; } map;
        struct { uint8_t *pbck; size_t pbck_len; ZrXferVal *upvals; size_t n_upvals; char *mod_path; } cl;
        struct { ZrSharedBacking *backing; size_t len; } shared_buf;
        char    *builtin_name;
    } u;
};

static void zr_xfer_free(ZrXferVal *x) {
    if (!x) return;
    switch (x->tag) {
        case XF_STR: case XF_BIGINT: case XF_RATIONAL: case XF_DECIMAL: case XF_MODULE:
            free(x->u.str.s);
            break;
        case XF_ARR:
            for (size_t i = 0; i < x->u.arr.len; i++) zr_xfer_free(&x->u.arr.items[i]);
            free(x->u.arr.items);
            break;
        case XF_MAP:
            for (size_t i = 0; i < x->u.map.len; i++) {
                free(x->u.map.keys[i]);
                zr_xfer_free(&x->u.map.vals[i]);
            }
            free(x->u.map.keys);
            free(x->u.map.klens);
            free(x->u.map.vals);
            break;
        case XF_CLOSURE:
            free(x->u.cl.pbck);
            for (size_t i = 0; i < x->u.cl.n_upvals; i++) zr_xfer_free(&x->u.cl.upvals[i]);
            free(x->u.cl.upvals);
            free(x->u.cl.mod_path);
            break;
        case XF_BUILTIN:
            free(x->u.builtin_name);
            break;
        case XF_BYTES:
        case XF_F64BUF:
            zr_shared_release(x->u.shared_buf.backing);
            break;
        default: break;
    }
    memset(x, 0, sizeof(*x));
}

static bool zr_xfer_pack(PbVM *vm, PbValue v, ZrXferVal *out, int depth) {
    memset(out, 0, sizeof(*out));
    out->taint = v.taint;
    if (depth > 48) { out->tag = XF_NIL; return true; }
    switch (v.tag) {
        case VAL_NIL:  out->tag = XF_NIL; return true;
        case VAL_BOOL: out->tag = XF_BOOL; out->u.b = v.as.b; return true;
        case VAL_NUM:  out->tag = XF_NUM; out->u.num = v.as.num; return true;
        case VAL_BUILTIN:
            out->tag = XF_BUILTIN;
            out->u.builtin_name = zr_xstrdup(v.as.builtin_id < vm->builtin_count ? vm->builtin_defs[v.as.builtin_id].name : "");
            return true;
        case VAL_OBJ:
            switch (v.as.obj->type) {
                case OBJ_STR:
                    out->tag = XF_STR;
                    out->u.str.len = AS_STR(v)->len;
                    out->u.str.s = zr_xstrndup(AS_STR(v)->chars, AS_STR(v)->len);
                    return true;
                case OBJ_BYTES: {
                    PbBytes *bb = AS_BYTES(v);
                    out->tag = XF_BYTES;
                    out->u.shared_buf.len = bb->len;
                    if (bb->backing) {
                        zr_shared_retain(bb->backing);
                        out->u.shared_buf.backing = bb->backing;
                    } else {
                        ZrSharedBacking *bk = zr_shared_backing_new(1, bb->len ? bb->len : 1);
                        if (bb->len > 0 && bb->data) memcpy(bk->raw, bb->data, bb->len);
                        out->u.shared_buf.backing = bk;
                    }
                    return true;
                }
                case OBJ_F64BUF: {
                    PbF64Buf *fb = AS_F64BUF(v);
                    out->tag = XF_F64BUF;
                    out->u.shared_buf.len = fb->len;
                    if (fb->backing) {
                        zr_shared_retain(fb->backing);
                        out->u.shared_buf.backing = fb->backing;
                    } else {
                        size_t bl = (fb->len ? fb->len : 1) * sizeof(double);
                        ZrSharedBacking *bk = zr_shared_backing_new(2, bl);
                        if (fb->len > 0 && fb->data) memcpy(bk->raw, fb->data, fb->len * sizeof(double));
                        out->u.shared_buf.backing = bk;
                    }
                    return true;
                }
                case OBJ_BIGINT:
                    out->tag = XF_BIGINT;
                    out->u.str.s = zr_bi_to_str(&AS_BIGINT(v)->v);
                    out->u.str.len = strlen(out->u.str.s);
                    return true;
                case OBJ_RATIONAL:
                    out->tag = XF_RATIONAL;
                    out->u.str.s = zr_rat_to_str(&AS_RATIONAL(v)->v);
                    out->u.str.len = strlen(out->u.str.s);
                    return true;
                case OBJ_DECIMAL:
                    out->tag = XF_DECIMAL;
                    out->u.str.s = zr_dec_to_str(&AS_DECIMAL(v)->v);
                    out->u.str.len = strlen(out->u.str.s);
                    return true;
                case OBJ_ARRAY: {
                    PbArray *a = AS_ARRAY(v);
                    out->tag = XF_ARR;
                    out->u.arr.len = a->len;
                    out->u.arr.items = a->len ? (ZrXferVal *)zr_xcalloc(a->len, sizeof(ZrXferVal)) : NULL;
                    for (size_t i = 0; i < a->len; i++) {
                        if (!zr_xfer_pack(vm, a->items[i], &out->u.arr.items[i], depth + 1)) return false;
                    }
                    return true;
                }
                case OBJ_MAP: {
                    PbMap *m = AS_MAP(v);
                    out->tag = XF_MAP;
                    out->u.map.len = m->len;
                    if (m->len > 0) {
                        out->u.map.keys = (char **)zr_xcalloc(m->len, sizeof(char *));
                        out->u.map.klens = (size_t *)zr_xcalloc(m->len, sizeof(size_t));
                        out->u.map.vals = (ZrXferVal *)zr_xcalloc(m->len, sizeof(ZrXferVal));
                        size_t k = 0;
                        for (size_t i = 0; i < m->used && k < m->len; i++) {
                            if (!m->entries[i].active) continue;
                            out->u.map.klens[k] = m->entries[i].key->len;
                            out->u.map.keys[k] = zr_xstrndup(m->entries[i].key->chars, m->entries[i].key->len);
                            if (!zr_xfer_pack(vm, m->entries[i].val, &out->u.map.vals[k], depth + 1)) return false;
                            k++;
                        }
                        out->u.map.len = k;
                    }
                    return true;
                }
                case OBJ_MODULE: {
                    PbModule *mod = AS_MODULE(v);
                    if (mod->path && mod->path->len > 0) {
                        out->tag = XF_MODULE;
                        out->u.str.len = mod->path->len;
                        out->u.str.s = zr_xstrndup(mod->path->chars, mod->path->len);
                    } else {
                        out->tag = XF_NIL;
                    }
                    return true;
                }
                case OBJ_FUNC:
                case OBJ_CLOSURE: {
                    PbFunc *fn = is_obj_type(v, OBJ_CLOSURE) ? AS_CLOSURE(v)->fn : AS_FUNC(v);
                    /* Wrap fn inside a tiny wrapper script chunk or serialize fn via temporary wrapper */
                    ZrBuf wb;
                    zr_buf_init(&wb);
                    zr_buf_append(&wb, "PBCK", 4);
                    zr_pw_u8(&wb, ZR_PBCK_VERSION);
                    zr_pbck_write_func(&wb, fn);
                    out->tag = XF_CLOSURE;
                    out->u.cl.pbck = (uint8_t *)zr_buf_take(&wb, &out->u.cl.pbck_len);
                    if (is_obj_type(v, OBJ_CLOSURE)) {
                        PbClosure *cl = AS_CLOSURE(v);
                        if (cl->module && cl->module->path && cl->module->path->len > 0) {
                            out->u.cl.mod_path = zr_xstrndup(cl->module->path->chars, cl->module->path->len);
                        }
                        if (cl->upvalue_count > 0) {
                            out->u.cl.n_upvals = cl->upvalue_count;
                            out->u.cl.upvals = (ZrXferVal *)zr_xcalloc(cl->upvalue_count, sizeof(ZrXferVal));
                            for (uint32_t u = 0; u < cl->upvalue_count; u++) {
                                PbUpvalue *uv = cl->upvalues[u];
                                PbValue uv_v = uv ? (uv->is_open ? vm->stack[uv->stack_idx] : uv->closed) : val_nil();
                                zr_xfer_pack(vm, uv_v, &out->u.cl.upvals[u], depth + 1);
                            }
                        }
                    }
                    return true;
                }
                default:
                    out->tag = XF_NIL;
                    return true;
            }
    }
    return true;
}

static PbValue zr_xfer_unpack(PbVM *vm, const ZrXferVal *x) {
    PbValue res = val_nil();
    switch (x->tag) {
        case XF_NIL:  res = val_nil(); break;
        case XF_BOOL: res = val_bool(x->u.b); break;
        case XF_NUM:  res = val_num(x->u.num); break;
        case XF_STR:  res = val_obj(pb_str_new(vm, x->u.str.s, x->u.str.len)); break;
        case XF_BYTES:
            res = val_obj(pb_bytes_from_backing(vm, x->u.shared_buf.backing, x->u.shared_buf.len));
            break;
        case XF_F64BUF:
            res = val_obj(pb_f64buf_from_backing(vm, x->u.shared_buf.backing, x->u.shared_buf.len));
            break;
        case XF_BIGINT: {
            ZrBigInt b; zr_bi_init(&b); zr_bi_from_str(&b, x->u.str.s);
            res = pb_bigint_new_take(vm, &b);
            break;
        }
        case XF_RATIONAL: {
            PbValue sarg = val_obj(pb_str_c(vm, x->u.str.s));
            (void)bi_rational(vm, 1, &sarg, &res);
            break;
        }
        case XF_DECIMAL: {
            ZrDecimal d; zr_dec_init(&d); zr_dec_from_str(&d, x->u.str.s);
            res = pb_decimal_new_take(vm, &d);
            break;
        }
        case XF_BUILTIN: {
            PbValue bv = val_nil();
            if (pb_map_get_cstr(vm->builtins, x->u.builtin_name, &bv)) res = bv;
            break;
        }
        case XF_MODULE: {
            PbValue mv = val_nil();
            if (x->u.str.s && zr_vm_import_module(vm, x->u.str.s, vm->current_file, &mv)) res = mv;
            else vm->err_msg[0] = '\0';
            break;
        }
        case XF_ARR: {
            PbArray *a = pb_array_new(vm, x->u.arr.len);
            vm_push_root(vm, val_obj(a));
            for (size_t i = 0; i < x->u.arr.len; i++) {
                pb_array_push(vm, a, zr_xfer_unpack(vm, &x->u.arr.items[i]));
            }
            vm_pop_roots(vm, 1);
            res = val_obj(a);
            break;
        }
        case XF_MAP: {
            PbMap *m = pb_map_new_raw(vm, x->u.map.len);
            vm_push_root(vm, val_obj(m));
            for (size_t i = 0; i < x->u.map.len; i++) {
                PbStr *ks = pb_str_new(vm, x->u.map.keys[i], x->u.map.klens[i]);
                vm_push_root(vm, val_obj(ks));
                PbValue vv = zr_xfer_unpack(vm, &x->u.map.vals[i]);
                pb_map_set(vm, m, ks, vv);
                vm_pop_roots(vm, 1);
            }
            vm_pop_roots(vm, 1);
            res = val_obj(m);
            break;
        }
        case XF_CLOSURE: {
            if (x->u.cl.pbck_len < 5) break;
            PbModule *cl_mod = NULL;
            if (x->u.cl.mod_path && x->u.cl.mod_path[0]) {
                PbValue mv = val_nil();
                if (zr_vm_import_module(vm, x->u.cl.mod_path, vm->current_file, &mv) && is_obj_type(mv, OBJ_MODULE)) {
                    cl_mod = AS_MODULE(mv);
                } else {
                    vm->err_msg[0] = '\0';
                }
            }
            if (cl_mod) vm_push_root(vm, val_obj(cl_mod));
            ZrPbckReader r;
            r.buf = x->u.cl.pbck;
            r.len = x->u.cl.pbck_len;
            r.pos = 5;
            r.err[0] = '\0';
            PbFunc *fn = NULL;
            vm->gc_inhibit++;
            bool ok = zr_pbck_read_func(&r, vm, &fn, 0);
            vm->gc_inhibit--;
            if (!ok || !fn) {
                if (cl_mod) vm_pop_roots(vm, 1);
                break;
            }
            PbClosure *cl = pb_closure_new(vm, fn, (uint32_t)x->u.cl.n_upvals, cl_mod);
            if (cl_mod) vm_pop_roots(vm, 1);
            vm_push_root(vm, val_obj(cl));
            for (size_t u = 0; u < x->u.cl.n_upvals; u++) {
                PbUpvalue *uv = (PbUpvalue *)vm_alloc_obj(vm, sizeof(PbUpvalue), OBJ_UPVALUE);
                uv->is_open = false;
                uv->closed = zr_xfer_unpack(vm, &x->u.cl.upvals[u]);
                cl->upvalues[u] = uv;
            }
            vm_pop_roots(vm, 1);
            res = val_obj(cl);
            break;
        }
    }
    res.taint = x->taint;
    return res;
}

static void zr_xfer_clone(const ZrXferVal *src, ZrXferVal *dst) {
    memset(dst, 0, sizeof(*dst));
    dst->tag = src->tag;
    dst->taint = src->taint;
    switch (src->tag) {
        case XF_NIL: break;
        case XF_BOOL: dst->u.b = src->u.b; break;
        case XF_NUM: dst->u.num = src->u.num; break;
        case XF_STR: case XF_BIGINT: case XF_RATIONAL: case XF_DECIMAL: case XF_MODULE:
            dst->u.str.len = src->u.str.len;
            dst->u.str.s = zr_xstrndup(src->u.str.s, src->u.str.len);
            break;
        case XF_BYTES:
        case XF_F64BUF:
            dst->u.shared_buf.len = src->u.shared_buf.len;
            dst->u.shared_buf.backing = src->u.shared_buf.backing;
            zr_shared_retain(dst->u.shared_buf.backing);
            break;
        case XF_BUILTIN:
            dst->u.builtin_name = zr_xstrdup(src->u.builtin_name);
            break;
        case XF_ARR:
            dst->u.arr.len = src->u.arr.len;
            dst->u.arr.items = src->u.arr.len ? (ZrXferVal *)zr_xcalloc(src->u.arr.len, sizeof(ZrXferVal)) : NULL;
            for (size_t i = 0; i < src->u.arr.len; i++) zr_xfer_clone(&src->u.arr.items[i], &dst->u.arr.items[i]);
            break;
        case XF_MAP:
            dst->u.map.len = src->u.map.len;
            if (src->u.map.len > 0) {
                dst->u.map.keys = (char **)zr_xcalloc(src->u.map.len, sizeof(char *));
                dst->u.map.klens = (size_t *)zr_xcalloc(src->u.map.len, sizeof(size_t));
                dst->u.map.vals = (ZrXferVal *)zr_xcalloc(src->u.map.len, sizeof(ZrXferVal));
                for (size_t i = 0; i < src->u.map.len; i++) {
                    dst->u.map.klens[i] = src->u.map.klens[i];
                    dst->u.map.keys[i] = zr_xstrndup(src->u.map.keys[i], src->u.map.klens[i]);
                    zr_xfer_clone(&src->u.map.vals[i], &dst->u.map.vals[i]);
                }
            }
            break;
        case XF_CLOSURE:
            dst->u.cl.pbck_len = src->u.cl.pbck_len;
            dst->u.cl.pbck = (uint8_t *)zr_xmalloc(src->u.cl.pbck_len);
            memcpy(dst->u.cl.pbck, src->u.cl.pbck, src->u.cl.pbck_len);
            dst->u.cl.mod_path = src->u.cl.mod_path ? zr_xstrdup(src->u.cl.mod_path) : NULL;
            dst->u.cl.n_upvals = src->u.cl.n_upvals;
            if (src->u.cl.n_upvals > 0) {
                dst->u.cl.upvals = (ZrXferVal *)zr_xcalloc(src->u.cl.n_upvals, sizeof(ZrXferVal));
                for (size_t u = 0; u < src->u.cl.n_upvals; u++) zr_xfer_clone(&src->u.cl.upvals[u], &dst->u.cl.upvals[u]);
            }
            break;
    }
}

/* ---- Global Sync / Thread / Channel Registry ---- */

#define ZR_MAX_SYNC_OBJS 1024

typedef struct {
    bool      used;
    bool      joined;
    bool      done;
    bool      ok;
    bool      taint_guard;
    char      err[512];
    ZrThread  thr;
    ZrCaps    caps;
    char     *stdlib_root;
    char     *current_file;
    ZrXferVal fn_xf;
    ZrXferVal globals_xf;
    ZrXferVal arg_xf;
    bool      has_arg;
    ZrXferVal res_xf;
} ZrSpawnTask;

typedef struct {
    bool       used;
    bool       closed;
    size_t     cap; /* 0 = unbounded MPMC queue */
    ZrXferVal *q;
    size_t     q_head;
    size_t     q_len;
    size_t     q_alloc;
    ZrMutex    mu;
    ZrCond     cv_not_empty;
    ZrCond     cv_not_full;
} ZrChannel;

typedef struct { bool used; int64_t val; ZrMutex mu; } ZrAtomicSlot;
typedef struct { bool used; ZrMutex mu; } ZrUserMutex;
typedef struct { bool used; int readers; bool writer; ZrMutex mu; ZrCond cv; } ZrUserRwLock;
typedef struct { bool used; int total; int count; int gen; ZrMutex mu; ZrCond cv; } ZrUserBarrier;
typedef struct { bool used; bool done; ZrXferVal val; ZrMutex mu; } ZrUserOnce;

static ZrMutex       g_zr_sync_mu;
static ZrCond        g_zr_select_cv;
static bool          g_zr_sync_inited = false;
static ZrSpawnTask   g_zr_tasks[ZR_MAX_SYNC_OBJS];
static ZrChannel     g_zr_chans[ZR_MAX_SYNC_OBJS];
static ZrAtomicSlot  g_zr_atomics[ZR_MAX_SYNC_OBJS];
static ZrUserMutex   g_zr_mutexes[ZR_MAX_SYNC_OBJS];
static ZrUserRwLock  g_zr_rwlocks[ZR_MAX_SYNC_OBJS];
static ZrUserBarrier g_zr_barriers[ZR_MAX_SYNC_OBJS];
static ZrUserOnce    g_zr_onces[ZR_MAX_SYNC_OBJS];

static void zr_sync_ensure_init(void) {
    if (!g_zr_sync_inited) {
        zr_mutex_init(&g_zr_sync_mu);
        zr_cond_init(&g_zr_select_cv);
        g_zr_sync_inited = true;
    }
}

static void *zr_spawn_worker_entry(void *raw) {
    ZrSpawnTask *t = (ZrSpawnTask *)raw;
    PbVM *cvm = (PbVM *)zr_xcalloc(1, sizeof(PbVM));
    vm_init(cvm);
    cvm->caps = t->caps;
    cvm->taint_guard_enabled = t->taint_guard;
    cvm->caps.ffi_allowed_libs = NULL;
    cvm->caps.ffi_allowed_libs_len = 0;
    cvm->caps.ffi_allowed_syms = NULL;
    cvm->caps.ffi_allowed_syms_len = 0;
    if (t->caps.ffi_allowed_libs_len > 0 && t->caps.ffi_allowed_libs) {
        cvm->caps.ffi_allowed_libs = (char **)zr_xmalloc(t->caps.ffi_allowed_libs_len * sizeof(char *));
        cvm->caps.ffi_allowed_libs_len = t->caps.ffi_allowed_libs_len;
        for (size_t i = 0; i < t->caps.ffi_allowed_libs_len; i++)
            cvm->caps.ffi_allowed_libs[i] = zr_xstrdup(t->caps.ffi_allowed_libs[i]);
    }
    if (t->caps.ffi_allowed_syms_len > 0 && t->caps.ffi_allowed_syms) {
        cvm->caps.ffi_allowed_syms = (char **)zr_xmalloc(t->caps.ffi_allowed_syms_len * sizeof(char *));
        cvm->caps.ffi_allowed_syms_len = t->caps.ffi_allowed_syms_len;
        for (size_t i = 0; i < t->caps.ffi_allowed_syms_len; i++)
            cvm->caps.ffi_allowed_syms[i] = zr_xstrdup(t->caps.ffi_allowed_syms[i]);
    }
    if (t->stdlib_root) cvm->stdlib_root = zr_xstrdup(t->stdlib_root);
    if (t->current_file) cvm->current_file = zr_xstrdup(t->current_file);

    /* Restore parent transferable globals */
    if (t->globals_xf.tag == XF_MAP) {
        for (size_t i = 0; i < t->globals_xf.u.map.len; i++) {
            PbStr *ks = pb_str_new(cvm, t->globals_xf.u.map.keys[i], t->globals_xf.u.map.klens[i]);
            vm_push_root(cvm, val_obj(ks));
            PbValue gv = zr_xfer_unpack(cvm, &t->globals_xf.u.map.vals[i]);
            pb_map_set(cvm, cvm->globals, ks, gv);
            vm_pop_roots(cvm, 1);
        }
    }

    PbValue callee = zr_xfer_unpack(cvm, &t->fn_xf);
    vm_push_root(cvm, callee);
    PbValue arg_v = t->has_arg ? zr_xfer_unpack(cvm, &t->arg_xf) : val_nil();
    if (t->has_arg) vm_push_root(cvm, arg_v);

    PbValue ret = val_nil();
    bool ok = vm_call_value_sync(cvm, callee, t->has_arg ? 1 : 0, t->has_arg ? &arg_v : NULL, &ret);
    if (ok) {
        t->ok = true;
        zr_xfer_pack(cvm, ret, &t->res_xf, 0);
    } else {
        t->ok = false;
        snprintf(t->err, sizeof(t->err), "%.*s",
                 (int)(sizeof(t->err) - 1),
                 cvm->err_msg[0] ? cvm->err_msg : "spawned thread runtime error");
    }
    vm_free(cvm);
    free(cvm);
    zr_mutex_lock(&g_zr_sync_mu);
    t->done = true;
    zr_mutex_unlock(&g_zr_sync_mu);
    return NULL;
}

static bool zr_spawn_one(PbVM *vm, PbValue fn_val, bool has_arg, PbValue arg_val, int *out_tid) {
    if (!vm_check_cap(vm, vm_effective_cap(vm, CAP_SPAWN), "spawn", "--allow-spawn")) return false;
    zr_sync_ensure_init();
    zr_mutex_lock(&g_zr_sync_mu);
    int slot = -1;
    for (int i = 1; i < ZR_MAX_SYNC_OBJS; i++) {
        if (!g_zr_tasks[i].used) {
            memset(&g_zr_tasks[i], 0, sizeof(ZrSpawnTask));
            g_zr_tasks[i].used = true;
            slot = i;
            break;
        }
    }
    zr_mutex_unlock(&g_zr_sync_mu);
    if (slot < 0) return vm_error(vm, "spawn(): thread table full");

    ZrSpawnTask *t = &g_zr_tasks[slot];
    t->caps = vm->caps;
    t->caps.allow_fs     = vm_effective_cap(vm, CAP_FS);
    t->caps.allow_net    = vm_effective_cap(vm, CAP_NET);
    t->caps.allow_http   = vm_effective_cap(vm, CAP_HTTP);
    t->caps.allow_sqlite = vm_effective_cap(vm, CAP_SQLITE);
    t->caps.allow_ffi    = vm_effective_cap(vm, CAP_FFI);
    t->caps.allow_exec   = vm_effective_cap(vm, CAP_EXEC);
    t->caps.allow_gfx    = vm_effective_cap(vm, CAP_GFX);
    t->caps.allow_spawn  = vm_effective_cap(vm, CAP_SPAWN);
    t->caps.allow_env    = vm_effective_cap(vm, CAP_ENV);
    t->taint_guard = vm->taint_guard_enabled;
    t->stdlib_root = vm->stdlib_root ? zr_xstrdup(vm->stdlib_root) : NULL;
    t->current_file = vm->current_file ? zr_xstrdup(vm->current_file) : NULL;
    zr_xfer_pack(vm, fn_val, &t->fn_xf, 0);
    zr_xfer_pack(vm, val_obj(vm->globals), &t->globals_xf, 0);
    t->has_arg = has_arg;
    if (has_arg) zr_xfer_pack(vm, arg_val, &t->arg_xf, 0);

    if (!zr_thread_create(&t->thr, zr_spawn_worker_entry, t)) {
        zr_xfer_free(&t->fn_xf);
        zr_xfer_free(&t->globals_xf);
        zr_xfer_free(&t->arg_xf);
        free(t->stdlib_root);
        free(t->current_file);
        t->used = false;
        return vm_error(vm, "spawn(): OS thread creation failed");
    }
    *out_tid = slot;
    return true;
}

static bool bi_spawn(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc < 1 || argc > 2) return vm_error(vm, "spawn(fn[, arg]) expects 1 or 2 arguments");
    int tid = 0;
    if (!zr_spawn_one(vm, argv[0], argc == 2, argc == 2 ? argv[1] : val_nil(), &tid)) return false;
    *out = val_num((double)tid);
    return true;
}

static bool bi_spawn_n(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 2 || argv[0].tag != VAL_NUM) return vm_error(vm, "spawn_n(n, fn) expects (number, fn)");
    int n = (int)argv[0].as.num;
    if (n < 0 || n > 256) return vm_error(vm, "spawn_n(): count must be 0..256");
    PbArray *arr = pb_array_new(vm, (size_t)n);
    vm_push_root(vm, val_obj(arr));
    for (int i = 0; i < n; i++) {
        int tid = 0;
        if (!zr_spawn_one(vm, argv[1], true, val_num((double)i), &tid)) {
            vm_pop_roots(vm, 1);
            return false;
        }
        pb_array_push(vm, arr, val_num((double)tid));
    }
    vm_pop_roots(vm, 1);
    *out = val_obj(arr);
    return true;
}

static bool zr_join_one(PbVM *vm, int tid, PbValue *out) {
    zr_sync_ensure_init();
    if (tid <= 0 || tid >= ZR_MAX_SYNC_OBJS) return vm_error(vm, "join(): invalid thread handle %d", tid);
    zr_mutex_lock(&g_zr_sync_mu);
    if (!g_zr_tasks[tid].used || g_zr_tasks[tid].joined) {
        zr_mutex_unlock(&g_zr_sync_mu);
        return vm_error(vm, "join(): thread handle %d already joined or invalid", tid);
    }
    g_zr_tasks[tid].joined = true;
    ZrThread thr = g_zr_tasks[tid].thr;
    zr_mutex_unlock(&g_zr_sync_mu);

    zr_thread_join(thr);
    ZrSpawnTask *t = &g_zr_tasks[tid];
    bool ok = t->ok;
    char err_copy[512];
    snprintf(err_copy, sizeof(err_copy), "%s", t->err);
    PbValue res = ok ? zr_xfer_unpack(vm, &t->res_xf) : val_nil();

    zr_xfer_free(&t->fn_xf);
    zr_xfer_free(&t->globals_xf);
    zr_xfer_free(&t->arg_xf);
    zr_xfer_free(&t->res_xf);
    free(t->stdlib_root);
    free(t->current_file);
    zr_mutex_lock(&g_zr_sync_mu);
    memset(t, 0, sizeof(*t));
    zr_mutex_unlock(&g_zr_sync_mu);

    if (!ok) return vm_error(vm, "%s", err_copy);
    *out = res;
    return true;
}

static bool bi_join(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 1 || argv[0].tag != VAL_NUM) return vm_error(vm, "join(handle) expects 1 thread handle");
    return zr_join_one(vm, (int)argv[0].as.num, out);
}

static bool bi_join_all(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 1 || !is_obj_type(argv[0], OBJ_ARRAY)) return vm_error(vm, "join_all(handles) expects 1 array");
    PbArray *hs = AS_ARRAY(argv[0]);
    vm_push_root(vm, argv[0]);
    PbArray *res = pb_array_new(vm, hs->len);
    vm_push_root(vm, val_obj(res));
    for (size_t i = 0; i < hs->len; i++) {
        if (hs->items[i].tag != VAL_NUM) { vm_pop_roots(vm, 2); return vm_error(vm, "join_all(): invalid handle"); }
        PbValue r = val_nil();
        if (!zr_join_one(vm, (int)hs->items[i].as.num, &r)) { vm_pop_roots(vm, 2); return false; }
        pb_array_push(vm, res, r);
    }
    vm_pop_roots(vm, 2);
    *out = val_obj(res);
    return true;
}

static bool bi_gil_enabled(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    (void)vm; (void)argc; (void)argv;
    *out = val_bool(false); /* True GIL-free VM-per-OS-thread architecture */
    return true;
}

/* ---- Channels ---- */

static int zr_alloc_channel(size_t cap) {
    zr_sync_ensure_init();
    zr_mutex_lock(&g_zr_sync_mu);
    int cid = -1;
    for (int i = 1; i < ZR_MAX_SYNC_OBJS; i++) {
        if (!g_zr_chans[i].used) {
            ZrChannel *c = &g_zr_chans[i];
            memset(c, 0, sizeof(*c));
            c->used = true;
            c->cap = cap;
            c->q_alloc = cap > 0 && cap < 64 ? cap : 16;
            c->q = (ZrXferVal *)zr_xcalloc(c->q_alloc, sizeof(ZrXferVal));
            zr_mutex_init(&c->mu);
            zr_cond_init(&c->cv_not_empty);
            zr_cond_init(&c->cv_not_full);
            cid = i;
            break;
        }
    }
    zr_mutex_unlock(&g_zr_sync_mu);
    return cid;
}

static int zr_extract_chan_id(PbValue v) {
    if (v.tag == VAL_NUM) return (int)v.as.num;
    if (is_obj_type(v, OBJ_MAP)) {
        PbValue hv;
        if (pb_map_get_cstr(AS_MAP(v), "sender", &hv) && hv.tag == VAL_NUM) return (int)hv.as.num;
        if (pb_map_get_cstr(AS_MAP(v), "receiver", &hv) && hv.tag == VAL_NUM) return (int)hv.as.num;
        if (pb_map_get_cstr(AS_MAP(v), "id", &hv) && hv.tag == VAL_NUM) return (int)hv.as.num;
    }
    return -1;
}

static void zr_chan_q_push(ZrChannel *c, const ZrXferVal *xv) {
    if (c->q_len == c->q_alloc) {
        size_t na = c->q_alloc * 2;
        ZrXferVal *nq = (ZrXferVal *)zr_xcalloc(na, sizeof(ZrXferVal));
        for (size_t i = 0; i < c->q_len; i++) {
            nq[i] = c->q[(c->q_head + i) % c->q_alloc];
        }
        free(c->q);
        c->q = nq;
        c->q_head = 0;
        c->q_alloc = na;
    }
    size_t tail = (c->q_head + c->q_len) % c->q_alloc;
    c->q[tail] = *xv;
    c->q_len++;
}

static ZrXferVal zr_chan_q_pop(ZrChannel *c) {
    ZrXferVal r = c->q[c->q_head];
    memset(&c->q[c->q_head], 0, sizeof(ZrXferVal));
    c->q_head = (c->q_head + 1) % c->q_alloc;
    c->q_len--;
    return r;
}

static bool bi_channel(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    (void)argv;
    if (argc != 0) return vm_error(vm, "channel() takes 0 arguments");
    int cid = zr_alloc_channel(0);
    if (cid < 0) return vm_error(vm, "channel(): table full");
    PbMap *m = pb_map_new_raw(vm, 4);
    vm_push_root(vm, val_obj(m));
    pb_map_set_cstr(vm, m, "sender", val_num((double)cid));
    pb_map_set_cstr(vm, m, "receiver", val_num((double)cid));
    vm_pop_roots(vm, 1);
    *out = val_obj(m);
    return true;
}

static bool bi_channel_buffered(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 1 || argv[0].tag != VAL_NUM || argv[0].as.num < 1.0) {
        return vm_error(vm, "channel_buffered(cap) expects positive integer capacity");
    }
    size_t cap = (size_t)argv[0].as.num;
    int cid = zr_alloc_channel(cap);
    if (cid < 0) return vm_error(vm, "channel_buffered(): table full");
    PbMap *m = pb_map_new_raw(vm, 4);
    vm_push_root(vm, val_obj(m));
    pb_map_set_cstr(vm, m, "sender", val_num((double)cid));
    pb_map_set_cstr(vm, m, "receiver", val_num((double)cid));
    pb_map_set_cstr(vm, m, "capacity", val_num((double)cap));
    vm_pop_roots(vm, 1);
    *out = val_obj(m);
    return true;
}

static bool bi_send(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 2) return vm_error(vm, "send(ch, val) expects 2 arguments");
    int cid = zr_extract_chan_id(argv[0]);
    if (cid <= 0 || cid >= ZR_MAX_SYNC_OBJS || !g_zr_chans[cid].used) return vm_error(vm, "send(): invalid channel");
    ZrXferVal xv;
    zr_xfer_pack(vm, argv[1], &xv, 0);
    ZrChannel *c = &g_zr_chans[cid];
    zr_mutex_lock(&c->mu);
    while (!c->closed && c->cap > 0 && c->q_len >= c->cap) {
        zr_cond_wait(&c->cv_not_full, &c->mu);
    }
    if (c->closed) {
        zr_mutex_unlock(&c->mu);
        zr_xfer_free(&xv);
        return vm_error(vm, "send(): channel is closed");
    }
    zr_chan_q_push(c, &xv);
    zr_cond_signal(&c->cv_not_empty);
    zr_mutex_unlock(&c->mu);

    zr_mutex_lock(&g_zr_sync_mu);
    zr_cond_broadcast(&g_zr_select_cv);
    zr_mutex_unlock(&g_zr_sync_mu);
    *out = val_bool(true);
    return true;
}

static bool bi_recv(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 1) return vm_error(vm, "recv(ch) expects 1 argument");
    int cid = zr_extract_chan_id(argv[0]);
    if (cid <= 0 || cid >= ZR_MAX_SYNC_OBJS || !g_zr_chans[cid].used) return vm_error(vm, "recv(): invalid channel");
    ZrChannel *c = &g_zr_chans[cid];
    zr_mutex_lock(&c->mu);
    while (c->q_len == 0 && !c->closed) {
        zr_cond_wait(&c->cv_not_empty, &c->mu);
    }
    if (c->q_len == 0) {
        zr_mutex_unlock(&c->mu);
        *out = val_nil();
        return true;
    }
    ZrXferVal xv = zr_chan_q_pop(c);
    zr_cond_signal(&c->cv_not_full);
    zr_mutex_unlock(&c->mu);
    *out = zr_xfer_unpack(vm, &xv);
    zr_xfer_free(&xv);
    return true;
}

static bool bi_recv_timeout(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 2 || argv[1].tag != VAL_NUM) return vm_error(vm, "recv_timeout(ch, ms) expects (channel, number)");
    int cid = zr_extract_chan_id(argv[0]);
    if (cid <= 0 || cid >= ZR_MAX_SYNC_OBJS || !g_zr_chans[cid].used) return vm_error(vm, "recv_timeout(): invalid channel");
    uint64_t ms = argv[1].as.num > 0.0 ? (uint64_t)argv[1].as.num : 0;
    ZrChannel *c = &g_zr_chans[cid];
    double deadline = zr_mono_sec() * 1000.0 + (double)ms;
    zr_mutex_lock(&c->mu);
    while (c->q_len == 0 && !c->closed) {
        double rem = deadline - zr_mono_sec() * 1000.0;
        if (rem <= 0.0) break;
        (void)zr_cond_timedwait_ms(&c->cv_not_empty, &c->mu, (uint64_t)(rem + 0.999));
    }
    if (c->q_len == 0) {
        zr_mutex_unlock(&c->mu);
        *out = val_nil();
        return true;
    }
    ZrXferVal xv = zr_chan_q_pop(c);
    zr_cond_signal(&c->cv_not_full);
    zr_mutex_unlock(&c->mu);
    *out = zr_xfer_unpack(vm, &xv);
    zr_xfer_free(&xv);
    return true;
}

static bool bi_channel_close(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 1) return vm_error(vm, "channel_close(ch) expects 1 argument");
    int cid = zr_extract_chan_id(argv[0]);
    if (cid <= 0 || cid >= ZR_MAX_SYNC_OBJS || !g_zr_chans[cid].used) return vm_error(vm, "channel_close(): invalid channel");
    ZrChannel *c = &g_zr_chans[cid];
    zr_mutex_lock(&c->mu);
    c->closed = true;
    zr_cond_broadcast(&c->cv_not_empty);
    zr_cond_broadcast(&c->cv_not_full);
    zr_mutex_unlock(&c->mu);

    zr_mutex_lock(&g_zr_sync_mu);
    zr_cond_broadcast(&g_zr_select_cv);
    zr_mutex_unlock(&g_zr_sync_mu);
    *out = val_bool(true);
    return true;
}

static bool bi_channel_is_closed(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 1) return vm_error(vm, "channel_is_closed(ch) expects 1 argument");
    int cid = zr_extract_chan_id(argv[0]);
    if (cid <= 0 || cid >= ZR_MAX_SYNC_OBJS || !g_zr_chans[cid].used) return vm_error(vm, "channel_is_closed(): invalid channel");
    ZrChannel *c = &g_zr_chans[cid];
    zr_mutex_lock(&c->mu);
    bool cl = c->closed;
    zr_mutex_unlock(&c->mu);
    *out = val_bool(cl);
    return true;
}

static bool bi_channel_size(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 1) return vm_error(vm, "channel_size(ch) expects 1 argument");
    int cid = zr_extract_chan_id(argv[0]);
    if (cid <= 0 || cid >= ZR_MAX_SYNC_OBJS || !g_zr_chans[cid].used) return vm_error(vm, "channel_size(): invalid channel");
    ZrChannel *c = &g_zr_chans[cid];
    zr_mutex_lock(&c->mu);
    size_t n = c->q_len;
    zr_mutex_unlock(&c->mu);
    *out = val_num((double)n);
    return true;
}

static bool zr_select_impl(PbVM *vm, PbArray *ch_arr, bool has_timeout, uint64_t timeout_ms, PbValue *out) {
    zr_sync_ensure_init();
    if (ch_arr->len == 0) return vm_error(vm, "select(): empty channel array");
    double deadline = has_timeout ? (zr_mono_sec() * 1000.0 + (double)timeout_ms) : 0.0;
    for (;;) {
        bool any_open = false;
        for (size_t i = 0; i < ch_arr->len; i++) {
            int cid = zr_extract_chan_id(ch_arr->items[i]);
            if (cid <= 0 || cid >= ZR_MAX_SYNC_OBJS || !g_zr_chans[cid].used) {
                return vm_error(vm, "select(): invalid channel at index %zu", i);
            }
            ZrChannel *c = &g_zr_chans[cid];
            zr_mutex_lock(&c->mu);
            if (c->q_len > 0) {
                ZrXferVal xv = zr_chan_q_pop(c);
                zr_cond_signal(&c->cv_not_full);
                zr_mutex_unlock(&c->mu);
                PbValue val = zr_xfer_unpack(vm, &xv);
                zr_xfer_free(&xv);
                vm_push_root(vm, val);
                PbMap *m = pb_map_new_raw(vm, 4);
                vm_push_root(vm, val_obj(m));
                pb_map_set_cstr(vm, m, "index", val_num((double)i));
                pb_map_set_cstr(vm, m, "value", val);
                pb_map_set_cstr(vm, m, "timeout", val_bool(false));
                vm_pop_roots(vm, 2);
                *out = val_obj(m);
                return true;
            }
            if (!c->closed) any_open = true;
            zr_mutex_unlock(&c->mu);
        }
        if (!any_open) {
            *out = val_nil();
            return true;
        }
        if (has_timeout) {
            double rem = deadline - zr_mono_sec() * 1000.0;
            if (rem <= 0.0) {
                *out = val_nil();
                return true;
            }
            zr_mutex_lock(&g_zr_sync_mu);
            (void)zr_cond_timedwait_ms(&g_zr_select_cv, &g_zr_sync_mu, rem > 10.0 ? 10 : (uint64_t)(rem + 0.999));
            zr_mutex_unlock(&g_zr_sync_mu);
        } else {
            zr_mutex_lock(&g_zr_sync_mu);
            (void)zr_cond_timedwait_ms(&g_zr_select_cv, &g_zr_sync_mu, 10);
            zr_mutex_unlock(&g_zr_sync_mu);
        }
    }
}

static bool bi_select(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 1 || !is_obj_type(argv[0], OBJ_ARRAY)) return vm_error(vm, "select(channels) expects 1 array");
    return zr_select_impl(vm, AS_ARRAY(argv[0]), false, 0, out);
}

static bool bi_select_with_timeout(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 2 || !is_obj_type(argv[0], OBJ_ARRAY) || argv[1].tag != VAL_NUM) {
        return vm_error(vm, "select_with_timeout(channels, ms) expects (array, number)");
    }
    uint64_t ms = argv[1].as.num > 0.0 ? (uint64_t)argv[1].as.num : 0;
    return zr_select_impl(vm, AS_ARRAY(argv[0]), true, ms, out);
}

typedef struct { int cid; double ms; bool once; } ZrTimerChanArg;

static void *zr_timer_chan_worker(void *raw) {
    ZrTimerChanArg *a = (ZrTimerChanArg *)raw;
    int cid = a->cid;
    double ms = a->ms;
    bool once = a->once;
    free(a);
    int64_t tick = 0;
    for (;;) {
        zr_sleep_ms(ms);
        ZrChannel *c = &g_zr_chans[cid];
        zr_mutex_lock(&c->mu);
        if (c->closed) { zr_mutex_unlock(&c->mu); break; }
        ZrXferVal xv;
        memset(&xv, 0, sizeof(xv));
        xv.tag = XF_NUM;
        xv.u.num = once ? zr_now_ms() : (double)(++tick);
        if (c->cap == 0 || c->q_len < c->cap) {
            zr_chan_q_push(c, &xv);
            zr_cond_signal(&c->cv_not_empty);
        }
        if (once) c->closed = true;
        zr_mutex_unlock(&c->mu);
        zr_mutex_lock(&g_zr_sync_mu);
        zr_cond_broadcast(&g_zr_select_cv);
        zr_mutex_unlock(&g_zr_sync_mu);
        if (once) break;
    }
    return NULL;
}

static bool bi_ticker(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 1 || argv[0].tag != VAL_NUM || argv[0].as.num <= 0.0) return vm_error(vm, "ticker(ms) expects positive number");
    int cid = zr_alloc_channel(16);
    if (cid < 0) return vm_error(vm, "ticker(): channel table full");
    ZrTimerChanArg *a = (ZrTimerChanArg *)zr_xmalloc(sizeof(ZrTimerChanArg));
    a->cid = cid; a->ms = argv[0].as.num; a->once = false;
    ZrThread th;
    if (!zr_thread_create(&th, zr_timer_chan_worker, a)) { free(a); return vm_error(vm, "ticker(): thread failed"); }
    *out = val_num((double)cid);
    return true;
}

static bool bi_timeout_channel(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 1 || argv[0].tag != VAL_NUM || argv[0].as.num < 0.0) return vm_error(vm, "timeout_channel(ms) expects number");
    int cid = zr_alloc_channel(1);
    if (cid < 0) return vm_error(vm, "timeout_channel(): channel table full");
    ZrTimerChanArg *a = (ZrTimerChanArg *)zr_xmalloc(sizeof(ZrTimerChanArg));
    a->cid = cid; a->ms = argv[0].as.num; a->once = true;
    ZrThread th;
    if (!zr_thread_create(&th, zr_timer_chan_worker, a)) { free(a); return vm_error(vm, "timeout_channel(): thread failed"); }
    *out = val_num((double)cid);
    return true;
}

/* ---- Atomics, Mutex, RwLock, Barrier & Once ---- */

static bool bi_atomic_new(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    int64_t init = (argc >= 1 && argv[0].tag == VAL_NUM) ? (int64_t)argv[0].as.num : 0;
    zr_sync_ensure_init();
    zr_mutex_lock(&g_zr_sync_mu);
    for (int i = 1; i < ZR_MAX_SYNC_OBJS; i++) {
        if (!g_zr_atomics[i].used) {
            g_zr_atomics[i].used = true;
            g_zr_atomics[i].val = init;
            zr_mutex_init(&g_zr_atomics[i].mu);
            zr_mutex_unlock(&g_zr_sync_mu);
            *out = val_num((double)i);
            return true;
        }
    }
    zr_mutex_unlock(&g_zr_sync_mu);
    return vm_error(vm, "atomic_new(): table full");
}

static ZrAtomicSlot *zr_get_atomic(PbVM *vm, const char *fn, PbValue h) {
    if (h.tag != VAL_NUM) { vm_error(vm, "%s(): invalid atomic handle", fn); return NULL; }
    int id = (int)h.as.num;
    if (id <= 0 || id >= ZR_MAX_SYNC_OBJS || !g_zr_atomics[id].used) {
        vm_error(vm, "%s(): unknown atomic handle %d", fn, id);
        return NULL;
    }
    return &g_zr_atomics[id];
}

static bool bi_atomic_get(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 1) return vm_error(vm, "atomic_get(h) expects 1 arg");
    ZrAtomicSlot *a = zr_get_atomic(vm, "atomic_get", argv[0]);
    if (!a) return false;
    zr_mutex_lock(&a->mu);
    int64_t v = a->val;
    zr_mutex_unlock(&a->mu);
    *out = val_num((double)v);
    return true;
}

static bool bi_atomic_set(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 2 || argv[1].tag != VAL_NUM) return vm_error(vm, "atomic_set(h, v) expects (handle, number)");
    ZrAtomicSlot *a = zr_get_atomic(vm, "atomic_set", argv[0]);
    if (!a) return false;
    zr_mutex_lock(&a->mu);
    a->val = (int64_t)argv[1].as.num;
    zr_mutex_unlock(&a->mu);
    *out = val_nil();
    return true;
}

static bool bi_atomic_add(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 2 || argv[1].tag != VAL_NUM) return vm_error(vm, "atomic_add(h, delta) expects (handle, number)");
    ZrAtomicSlot *a = zr_get_atomic(vm, "atomic_add", argv[0]);
    if (!a) return false;
    zr_mutex_lock(&a->mu);
    int64_t prev = a->val;
    a->val += (int64_t)argv[1].as.num;
    zr_mutex_unlock(&a->mu);
    *out = val_num((double)prev);
    return true;
}

static bool bi_atomic_sub(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 2 || argv[1].tag != VAL_NUM) return vm_error(vm, "atomic_sub(h, delta) expects (handle, number)");
    ZrAtomicSlot *a = zr_get_atomic(vm, "atomic_sub", argv[0]);
    if (!a) return false;
    zr_mutex_lock(&a->mu);
    int64_t prev = a->val;
    a->val -= (int64_t)argv[1].as.num;
    zr_mutex_unlock(&a->mu);
    *out = val_num((double)prev);
    return true;
}

static bool bi_atomic_swap(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 2 || argv[1].tag != VAL_NUM) return vm_error(vm, "atomic_swap(h, v) expects (handle, number)");
    ZrAtomicSlot *a = zr_get_atomic(vm, "atomic_swap", argv[0]);
    if (!a) return false;
    zr_mutex_lock(&a->mu);
    int64_t prev = a->val;
    a->val = (int64_t)argv[1].as.num;
    zr_mutex_unlock(&a->mu);
    *out = val_num((double)prev);
    return true;
}

static bool bi_atomic_cas(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 3 || argv[1].tag != VAL_NUM || argv[2].tag != VAL_NUM) {
        return vm_error(vm, "atomic_cas(h, expected, desired) expects 3 numbers");
    }
    ZrAtomicSlot *a = zr_get_atomic(vm, "atomic_cas", argv[0]);
    if (!a) return false;
    int64_t exp_v = (int64_t)argv[1].as.num, des_v = (int64_t)argv[2].as.num;
    zr_mutex_lock(&a->mu);
    bool ok = (a->val == exp_v);
    if (ok) a->val = des_v;
    zr_mutex_unlock(&a->mu);
    *out = val_bool(ok);
    return true;
}

static bool bi_mutex_new(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    (void)argc; (void)argv;
    zr_sync_ensure_init();
    zr_mutex_lock(&g_zr_sync_mu);
    for (int i = 1; i < ZR_MAX_SYNC_OBJS; i++) {
        if (!g_zr_mutexes[i].used) {
            g_zr_mutexes[i].used = true;
            zr_mutex_init(&g_zr_mutexes[i].mu);
            zr_mutex_unlock(&g_zr_sync_mu);
            *out = val_num((double)i);
            return true;
        }
    }
    zr_mutex_unlock(&g_zr_sync_mu);
    return vm_error(vm, "mutex_new(): table full");
}

static bool bi_mutex_lock(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 1 || argv[0].tag != VAL_NUM) return vm_error(vm, "mutex_lock(h) expects handle");
    int id = (int)argv[0].as.num;
    if (id <= 0 || id >= ZR_MAX_SYNC_OBJS || !g_zr_mutexes[id].used) return vm_error(vm, "mutex_lock(): invalid handle");
    zr_mutex_lock(&g_zr_mutexes[id].mu);
    *out = val_bool(true);
    return true;
}

static bool bi_mutex_try_lock(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 1 || argv[0].tag != VAL_NUM) return vm_error(vm, "mutex_try_lock(h) expects handle");
    int id = (int)argv[0].as.num;
    if (id <= 0 || id >= ZR_MAX_SYNC_OBJS || !g_zr_mutexes[id].used) return vm_error(vm, "mutex_try_lock(): invalid handle");
    *out = val_bool(zr_mutex_trylock(&g_zr_mutexes[id].mu));
    return true;
}

static bool bi_mutex_unlock(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 1 || argv[0].tag != VAL_NUM) return vm_error(vm, "mutex_unlock(h) expects handle");
    int id = (int)argv[0].as.num;
    if (id <= 0 || id >= ZR_MAX_SYNC_OBJS || !g_zr_mutexes[id].used) return vm_error(vm, "mutex_unlock(): invalid handle");
    zr_mutex_unlock(&g_zr_mutexes[id].mu);
    *out = val_bool(true);
    return true;
}

static bool bi_rwlock_new(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    (void)argc; (void)argv;
    zr_sync_ensure_init();
    zr_mutex_lock(&g_zr_sync_mu);
    for (int i = 1; i < ZR_MAX_SYNC_OBJS; i++) {
        if (!g_zr_rwlocks[i].used) {
            memset(&g_zr_rwlocks[i], 0, sizeof(ZrUserRwLock));
            g_zr_rwlocks[i].used = true;
            zr_mutex_init(&g_zr_rwlocks[i].mu);
            zr_cond_init(&g_zr_rwlocks[i].cv);
            zr_mutex_unlock(&g_zr_sync_mu);
            *out = val_num((double)i);
            return true;
        }
    }
    zr_mutex_unlock(&g_zr_sync_mu);
    return vm_error(vm, "rwlock_new(): table full");
}

static bool bi_rwlock_read(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 1 || argv[0].tag != VAL_NUM) return vm_error(vm, "rwlock_read(h) expects handle");
    int id = (int)argv[0].as.num;
    if (id <= 0 || id >= ZR_MAX_SYNC_OBJS || !g_zr_rwlocks[id].used) return vm_error(vm, "rwlock_read(): invalid handle");
    ZrUserRwLock *rw = &g_zr_rwlocks[id];
    zr_mutex_lock(&rw->mu);
    while (rw->writer) zr_cond_wait(&rw->cv, &rw->mu);
    rw->readers++;
    zr_mutex_unlock(&rw->mu);
    *out = val_bool(true);
    return true;
}

static bool bi_rwlock_write(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 1 || argv[0].tag != VAL_NUM) return vm_error(vm, "rwlock_write(h) expects handle");
    int id = (int)argv[0].as.num;
    if (id <= 0 || id >= ZR_MAX_SYNC_OBJS || !g_zr_rwlocks[id].used) return vm_error(vm, "rwlock_write(): invalid handle");
    ZrUserRwLock *rw = &g_zr_rwlocks[id];
    zr_mutex_lock(&rw->mu);
    while (rw->writer || rw->readers > 0) zr_cond_wait(&rw->cv, &rw->mu);
    rw->writer = true;
    zr_mutex_unlock(&rw->mu);
    *out = val_bool(true);
    return true;
}

static bool bi_rwlock_unlock(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 1 || argv[0].tag != VAL_NUM) return vm_error(vm, "rwlock_unlock(h) expects handle");
    int id = (int)argv[0].as.num;
    if (id <= 0 || id >= ZR_MAX_SYNC_OBJS || !g_zr_rwlocks[id].used) return vm_error(vm, "rwlock_unlock(): invalid handle");
    ZrUserRwLock *rw = &g_zr_rwlocks[id];
    zr_mutex_lock(&rw->mu);
    if (rw->writer) rw->writer = false;
    else if (rw->readers > 0) rw->readers--;
    zr_cond_broadcast(&rw->cv);
    zr_mutex_unlock(&rw->mu);
    *out = val_bool(true);
    return true;
}

static bool bi_barrier_new(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 1 || argv[0].tag != VAL_NUM || argv[0].as.num < 1.0) return vm_error(vm, "barrier_new(n) expects positive int");
    zr_sync_ensure_init();
    zr_mutex_lock(&g_zr_sync_mu);
    for (int i = 1; i < ZR_MAX_SYNC_OBJS; i++) {
        if (!g_zr_barriers[i].used) {
            memset(&g_zr_barriers[i], 0, sizeof(ZrUserBarrier));
            g_zr_barriers[i].used = true;
            g_zr_barriers[i].total = (int)argv[0].as.num;
            zr_mutex_init(&g_zr_barriers[i].mu);
            zr_cond_init(&g_zr_barriers[i].cv);
            zr_mutex_unlock(&g_zr_sync_mu);
            *out = val_num((double)i);
            return true;
        }
    }
    zr_mutex_unlock(&g_zr_sync_mu);
    return vm_error(vm, "barrier_new(): table full");
}

static bool bi_barrier_wait(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 1 || argv[0].tag != VAL_NUM) return vm_error(vm, "barrier_wait(h) expects handle");
    int id = (int)argv[0].as.num;
    if (id <= 0 || id >= ZR_MAX_SYNC_OBJS || !g_zr_barriers[id].used) return vm_error(vm, "barrier_wait(): invalid handle");
    ZrUserBarrier *b = &g_zr_barriers[id];
    zr_mutex_lock(&b->mu);
    int g = b->gen;
    if (++b->count >= b->total) {
        b->count = 0;
        b->gen++;
        zr_cond_broadcast(&b->cv);
        zr_mutex_unlock(&b->mu);
        *out = val_bool(true); /* leader */
        return true;
    }
    while (g == b->gen) zr_cond_wait(&b->cv, &b->mu);
    zr_mutex_unlock(&b->mu);
    *out = val_bool(false);
    return true;
}

static bool bi_once_new(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    (void)argc; (void)argv;
    zr_sync_ensure_init();
    zr_mutex_lock(&g_zr_sync_mu);
    for (int i = 1; i < ZR_MAX_SYNC_OBJS; i++) {
        if (!g_zr_onces[i].used) {
            memset(&g_zr_onces[i], 0, sizeof(ZrUserOnce));
            g_zr_onces[i].used = true;
            zr_mutex_init(&g_zr_onces[i].mu);
            zr_mutex_unlock(&g_zr_sync_mu);
            *out = val_num((double)i);
            return true;
        }
    }
    zr_mutex_unlock(&g_zr_sync_mu);
    return vm_error(vm, "once_new(): table full");
}

static bool bi_once_is_done(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc < 1 || argv[0].tag != VAL_NUM) return vm_error(vm, "once_is_done(h[, init_fn]) expects handle");
    int id = (int)argv[0].as.num;
    if (id <= 0 || id >= ZR_MAX_SYNC_OBJS || !g_zr_onces[id].used) return vm_error(vm, "once_is_done(): invalid handle");
    ZrUserOnce *o = &g_zr_onces[id];
    zr_mutex_lock(&o->mu);
    if (argc == 2 && !o->done) {
        PbValue r = val_nil();
        if (!vm_call_value_sync(vm, argv[1], 0, NULL, &r)) {
            zr_mutex_unlock(&o->mu);
            return false;
        }
        zr_xfer_pack(vm, r, &o->val, 0);
        o->done = true;
        zr_mutex_unlock(&o->mu);
        *out = r;
        return true;
    }
    bool d = o->done;
    zr_mutex_unlock(&o->mu);
    *out = val_bool(d);
    return true;
}

/* ============================================================================
 * 6. Memory Arena & Borrow-Checker Builtins
 * ============================================================================ */

#define ZR_MAX_MEM_CELLS 4096

typedef struct {
    bool      alive;
    int       immut_borrows;
    bool      mut_borrow;
    int       lifetime_depth;
    ZrXferVal val;
} ZrMemCell;

static ZrMutex   g_zr_mem_mu;
static bool      g_zr_mem_inited = false;
static ZrMemCell g_zr_mem[ZR_MAX_MEM_CELLS];
static int       g_zr_lifetime_depth = 0;

static void zr_mem_ensure(void) {
    if (!g_zr_mem_inited) { zr_mutex_init(&g_zr_mem_mu); g_zr_mem_inited = true; }
}

static bool bi_memory_alloc(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    zr_mem_ensure();
    zr_mutex_lock(&g_zr_mem_mu);
    for (int i = 1; i < ZR_MAX_MEM_CELLS; i++) {
        if (!g_zr_mem[i].alive) {
            memset(&g_zr_mem[i], 0, sizeof(ZrMemCell));
            g_zr_mem[i].alive = true;
            g_zr_mem[i].lifetime_depth = g_zr_lifetime_depth;
            if (argc >= 1) zr_xfer_pack(vm, argv[0], &g_zr_mem[i].val, 0);
            zr_mutex_unlock(&g_zr_mem_mu);
            *out = val_num((double)i);
            return true;
        }
    }
    zr_mutex_unlock(&g_zr_mem_mu);
    return vm_error(vm, "allocate(): arena full");
}

static bool bi_memory_free(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    zr_mem_ensure();
    if (argc != 1 || argv[0].tag != VAL_NUM) return vm_error(vm, "deallocate(ptr) expects handle");
    int id = (int)argv[0].as.num;
    zr_mutex_lock(&g_zr_mem_mu);
    if (id <= 0 || id >= ZR_MAX_MEM_CELLS || !g_zr_mem[id].alive) {
        zr_mutex_unlock(&g_zr_mem_mu);
        return vm_error(vm, "deallocate(): double-free or invalid pointer %d", id);
    }
    if (g_zr_mem[id].immut_borrows > 0 || g_zr_mem[id].mut_borrow) {
        zr_mutex_unlock(&g_zr_mem_mu);
        return vm_error(vm, "deallocate(): cannot free borrowed allocation %d", id);
    }
    zr_xfer_free(&g_zr_mem[id].val);
    g_zr_mem[id].alive = false;
    zr_mutex_unlock(&g_zr_mem_mu);
    *out = val_bool(true);
    return true;
}

static bool bi_memory_get(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    zr_mem_ensure();
    if (argc != 1 || argv[0].tag != VAL_NUM) return vm_error(vm, "load(ptr) expects handle");
    int id = (int)argv[0].as.num;
    zr_mutex_lock(&g_zr_mem_mu);
    if (id <= 0 || id >= ZR_MAX_MEM_CELLS || !g_zr_mem[id].alive) {
        zr_mutex_unlock(&g_zr_mem_mu);
        return vm_error(vm, "load(): use-after-free or invalid pointer %d", id);
    }
    ZrXferVal copy;
    zr_xfer_clone(&g_zr_mem[id].val, &copy);
    zr_mutex_unlock(&g_zr_mem_mu);
    *out = zr_xfer_unpack(vm, &copy);
    zr_xfer_free(&copy);
    return true;
}

static bool bi_memory_set(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    zr_mem_ensure();
    if (argc != 2 || argv[0].tag != VAL_NUM) return vm_error(vm, "store(ptr, val) expects (handle, value)");
    int id = (int)argv[0].as.num;
    zr_mutex_lock(&g_zr_mem_mu);
    if (id <= 0 || id >= ZR_MAX_MEM_CELLS || !g_zr_mem[id].alive) {
        zr_mutex_unlock(&g_zr_mem_mu);
        return vm_error(vm, "store(): use-after-free or invalid pointer %d", id);
    }
    if (g_zr_mem[id].immut_borrows > 0) {
        zr_mutex_unlock(&g_zr_mem_mu);
        return vm_error(vm, "store(): cannot mutate immutably borrowed allocation %d", id);
    }
    zr_xfer_free(&g_zr_mem[id].val);
    zr_xfer_pack(vm, argv[1], &g_zr_mem[id].val, 0);
    zr_mutex_unlock(&g_zr_mem_mu);
    *out = val_bool(true);
    return true;
}

static bool zr_mem_borrow_impl(PbVM *vm, PbValue h, bool mut, PbValue *out) {
    zr_mem_ensure();
    if (h.tag != VAL_NUM) return vm_error(vm, "borrow(): expected pointer handle");
    int id = (int)h.as.num;
    zr_mutex_lock(&g_zr_mem_mu);
    if (id <= 0 || id >= ZR_MAX_MEM_CELLS || !g_zr_mem[id].alive) {
        zr_mutex_unlock(&g_zr_mem_mu);
        return vm_error(vm, "borrow(): invalid pointer %d", id);
    }
    if (mut) {
        if (g_zr_mem[id].mut_borrow || g_zr_mem[id].immut_borrows > 0) {
            zr_mutex_unlock(&g_zr_mem_mu);
            return vm_error(vm, "borrow_mutable(): conflicting active borrow on %d", id);
        }
        g_zr_mem[id].mut_borrow = true;
    } else {
        if (g_zr_mem[id].mut_borrow) {
            zr_mutex_unlock(&g_zr_mem_mu);
            return vm_error(vm, "borrow_immutable(): active mutable borrow on %d", id);
        }
        g_zr_mem[id].immut_borrows++;
    }
    zr_mutex_unlock(&g_zr_mem_mu);
    *out = val_num((double)id);
    return true;
}

static bool bi_borrow_immutable(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 1) return vm_error(vm, "borrow_immutable(ptr) expects 1 arg");
    return zr_mem_borrow_impl(vm, argv[0], false, out);
}

static bool bi_borrow_mutable(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 1) return vm_error(vm, "borrow_mutable(ptr) expects 1 arg");
    return zr_mem_borrow_impl(vm, argv[0], true, out);
}

static bool bi_memory_borrow(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc < 1 || argc > 2) return vm_error(vm, "memory_borrow(ptr[, mode]) expects 1 or 2 args");
    bool mut = (argc == 2 && ((argv[1].tag == VAL_BOOL && argv[1].as.b) ||
               (is_obj_type(argv[1], OBJ_STR) && (strcmp(AS_STR(argv[1])->chars, "mut") == 0 || strcmp(AS_STR(argv[1])->chars, "mutable") == 0))));
    return zr_mem_borrow_impl(vm, argv[0], mut, out);
}

static bool bi_memory_release(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    zr_mem_ensure();
    if (argc != 1 || argv[0].tag != VAL_NUM) return vm_error(vm, "release_borrow(ptr) expects handle");
    int id = (int)argv[0].as.num;
    zr_mutex_lock(&g_zr_mem_mu);
    if (id > 0 && id < ZR_MAX_MEM_CELLS && g_zr_mem[id].alive) {
        if (g_zr_mem[id].mut_borrow) g_zr_mem[id].mut_borrow = false;
        else if (g_zr_mem[id].immut_borrows > 0) g_zr_mem[id].immut_borrows--;
    }
    zr_mutex_unlock(&g_zr_mem_mu);
    *out = val_bool(true);
    return true;
}

static bool bi_push_lifetime(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    (void)vm; (void)argc; (void)argv;
    zr_mem_ensure();
    zr_mutex_lock(&g_zr_mem_mu);
    int d = ++g_zr_lifetime_depth;
    zr_mutex_unlock(&g_zr_mem_mu);
    *out = val_num((double)d);
    return true;
}

static bool bi_pop_lifetime(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    (void)vm; (void)argc; (void)argv;
    zr_mem_ensure();
    zr_mutex_lock(&g_zr_mem_mu);
    size_t freed = 0;
    if (g_zr_lifetime_depth > 0) {
        int d = g_zr_lifetime_depth--;
        for (int i = 1; i < ZR_MAX_MEM_CELLS; i++) {
            if (g_zr_mem[i].alive && g_zr_mem[i].lifetime_depth >= d) {
                zr_xfer_free(&g_zr_mem[i].val);
                memset(&g_zr_mem[i], 0, sizeof(ZrMemCell));
                freed++;
            }
        }
    }
    zr_mutex_unlock(&g_zr_mem_mu);
    *out = val_num((double)freed);
    return true;
}

static bool bi_memory_stats(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    (void)argc; (void)argv;
    zr_mem_ensure();
    zr_mutex_lock(&g_zr_mem_mu);
    size_t live = 0, borrows = 0;
    for (int i = 1; i < ZR_MAX_MEM_CELLS; i++) {
        if (g_zr_mem[i].alive) {
            live++;
            borrows += (size_t)g_zr_mem[i].immut_borrows + (g_zr_mem[i].mut_borrow ? 1 : 0);
        }
    }
    int depth = g_zr_lifetime_depth;
    zr_mutex_unlock(&g_zr_mem_mu);
    PbMap *m = pb_map_new_raw(vm, 6);
    vm_push_root(vm, val_obj(m));
    pb_map_set_cstr(vm, m, "live_allocations", val_num((double)live));
    pb_map_set_cstr(vm, m, "active_borrows", val_num((double)borrows));
    pb_map_set_cstr(vm, m, "lifetime_depth", val_num((double)depth));
    pb_map_set_cstr(vm, m, "gc_live_bytes", val_num((double)vm->live_bytes));
    vm_pop_roots(vm, 1);
    *out = val_obj(m);
    return true;
}

/* ============================================================================
 * 7. Native Tensor Builtins (`tensor_*` — 25 operations)
 * Tensor representation: map{ "shape": [d0, ...], "data": [...], "dtype": "f32" }
 * ============================================================================ */

#define ZR_MAX_TENSOR_ELEMENTS 25000000ULL

static bool zr_tensor_extract(PbVM *vm, const char *fn, PbValue v,
                              size_t **out_shape, size_t *out_ndim,
                              double **out_data, size_t *out_len) {
    if (!is_obj_type(v, OBJ_MAP)) {
        return vm_error(vm, "%s(): expected tensor map, got %s", fn, zr_val_type_name(v));
    }
    PbMap *m = AS_MAP(v);
    PbValue sv, dv;
    if (!pb_map_get_cstr(m, "shape", &sv) || !is_obj_type(sv, OBJ_ARRAY)) {
        return vm_error(vm, "%s(): tensor missing 'shape' array", fn);
    }
    if (!pb_map_get_cstr(m, "data", &dv) || !is_obj_type(dv, OBJ_ARRAY)) {
        return vm_error(vm, "%s(): tensor missing 'data' array", fn);
    }
    PbArray *sa = AS_ARRAY(sv);
    PbArray *da = AS_ARRAY(dv);
    size_t ndim = sa->len;
    size_t *shape = (size_t *)zr_xmalloc((ndim ? ndim : 1) * sizeof(size_t));
    uint64_t total = ndim ? 1ULL : 0ULL;
    for (size_t i = 0; i < ndim; i++) {
        if (sa->items[i].tag != VAL_NUM || sa->items[i].as.num < 0.0) {
            free(shape);
            return vm_error(vm, "%s(): invalid tensor shape dimension", fn);
        }
        size_t dim = (size_t)sa->items[i].as.num;
        shape[i] = dim;
        if (dim > 0 && total > ZR_MAX_TENSOR_ELEMENTS / (uint64_t)dim) {
            free(shape);
            return vm_error(vm, "%s(): tensor shape exceeds maximum element limit", fn);
        }
        total *= (uint64_t)dim;
    }
    if (da->len != (size_t)total) {
        free(shape);
        return vm_error(vm, "%s(): tensor data length (%zu) does not match shape product (%llu)",
                        fn, da->len, (unsigned long long)total);
    }
    double *data = (double *)zr_xmalloc((da->len ? da->len : 1) * sizeof(double));
    for (size_t i = 0; i < da->len; i++) {
        if (!zr_req_num(vm, fn, da->items[i], &data[i])) {
            free(shape);
            free(data);
            return false;
        }
    }
    *out_shape = shape;
    *out_ndim = ndim;
    *out_data = data;
    *out_len = da->len;
    return true;
}

static PbValue zr_tensor_make(PbVM *vm, const size_t *shape, size_t ndim,
                              const double *data, size_t len) {
    PbArray *sa = pb_array_new_raw(vm, ndim ? ndim : 1);
    vm_push_root(vm, val_obj(sa));
    for (size_t i = 0; i < ndim; i++) {
        pb_array_push(vm, sa, val_num((double)shape[i]));
    }
    PbArray *da = pb_array_new_raw(vm, len ? len : 1);
    vm_push_root(vm, val_obj(da));
    for (size_t i = 0; i < len; i++) {
        pb_array_push(vm, da, val_num(data[i]));
    }
    PbMap *m = pb_map_new_raw(vm, 8);
    vm_push_root(vm, val_obj(m));
    pb_map_set_cstr(vm, m, "shape", val_obj(sa));
    pb_map_set_cstr(vm, m, "data", val_obj(da));
    pb_map_set_cstr(vm, m, "dtype", val_obj(pb_str_new(vm, "f32", 3)));
    vm_pop_roots(vm, 3);
    return val_obj(m);
}

static bool zr_tensor_same_shape(const size_t *a, size_t na, const size_t *b, size_t nb) {
    if (na != nb) return false;
    for (size_t i = 0; i < na; i++) {
        if (a[i] != b[i]) return false;
    }
    return true;
}

static bool zr_tensor_binop(PbVM *vm, const char *fn, int argc, PbValue *argv, PbValue *out, int op) {
    if (argc != 2) return vm_error(vm, "%s() expects 2 arguments", fn);
    size_t *sa = NULL, *sb = NULL, na = 0, nb = 0, la = 0, lb = 0;
    double *da = NULL, *db = NULL;
    if (!zr_tensor_extract(vm, fn, argv[0], &sa, &na, &da, &la)) return false;
    if (!zr_tensor_extract(vm, fn, argv[1], &sb, &nb, &db, &lb)) {
        free(sa); free(da);
        return false;
    }
    bool bias_bcast = (!zr_tensor_same_shape(sa, na, sb, nb) &&
                       na >= 2 && nb == 1 && sb[0] > 0 && sa[na - 1] == sb[0]);
    if (!zr_tensor_same_shape(sa, na, sb, nb) && !bias_bcast) {
        free(sa); free(da); free(sb); free(db);
        return vm_error(vm, "%s(): shape mismatch", fn);
    }
    double *res = (double *)zr_xmalloc((la ? la : 1) * sizeof(double));
    size_t last = bias_bcast ? sb[0] : 1;
    for (size_t i = 0; i < la; i++) {
        double bv = bias_bcast ? db[i % last] : db[i];
        if (op == 0) res[i] = da[i] + bv;
        else if (op == 1) res[i] = da[i] - bv;
        else res[i] = da[i] * bv;
    }
    *out = zr_tensor_make(vm, sa, na, res, la);
    free(sa); free(da); free(sb); free(db); free(res);
    return true;
}

static bool bi_tensor_add(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    return zr_tensor_binop(vm, "tensor_add", argc, argv, out, 0);
}
static bool bi_tensor_sub(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    return zr_tensor_binop(vm, "tensor_sub", argc, argv, out, 1);
}
static bool bi_tensor_mul(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    return zr_tensor_binop(vm, "tensor_mul", argc, argv, out, 2);
}

static bool bi_tensor_scale(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 2) return vm_error(vm, "tensor_scale() expects 2 arguments (tensor, factor)");
    double s = 0.0;
    if (!zr_req_num(vm, "tensor_scale", argv[1], &s)) return false;
    size_t *sa = NULL, na = 0, la = 0;
    double *da = NULL;
    if (!zr_tensor_extract(vm, "tensor_scale", argv[0], &sa, &na, &da, &la)) return false;
    for (size_t i = 0; i < la; i++) da[i] *= s;
    *out = zr_tensor_make(vm, sa, na, da, la);
    free(sa); free(da);
    return true;
}

static bool bi_tensor_matmul(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 2) return vm_error(vm, "tensor_matmul() expects 2 arguments");
    size_t *sa = NULL, *sb = NULL, na = 0, nb = 0, la = 0, lb = 0;
    double *da = NULL, *db = NULL;
    if (!zr_tensor_extract(vm, "tensor_matmul", argv[0], &sa, &na, &da, &la)) return false;
    if (!zr_tensor_extract(vm, "tensor_matmul", argv[1], &sb, &nb, &db, &lb)) {
        free(sa); free(da);
        return false;
    }
    if (na != 2 || nb != 2 || sa[1] != sb[0]) {
        free(sa); free(da); free(sb); free(db);
        return vm_error(vm, "tensor_matmul(): incompatible 2D shapes");
    }
    size_t M = sa[0], K = sa[1], N = sb[1];
    if ((uint64_t)M * (uint64_t)N > ZR_MAX_TENSOR_ELEMENTS) {
        free(sa); free(da); free(sb); free(db);
        return vm_error(vm, "tensor_matmul(): output tensor too large");
    }
    size_t out_len = M * N;
    double *dc = (double *)zr_xcalloc(out_len ? out_len : 1, sizeof(double));
    for (size_t i = 0; i < M; i++) {
        const double *arow = da + i * K;
        double *crow = dc + i * N;
        for (size_t k = 0; k < K; k++) {
            double aik = arow[k];
            const double *brow = db + k * N;
            for (size_t j = 0; j < N; j++) {
                crow[j] += aik * brow[j];
            }
        }
    }
    size_t sc[2] = { M, N };
    *out = zr_tensor_make(vm, sc, 2, dc, out_len);
    free(sa); free(da); free(sb); free(db); free(dc);
    return true;
}

static bool bi_tensor_dot(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 2) return vm_error(vm, "tensor_dot() expects 2 arguments");
    size_t *sa = NULL, *sb = NULL, na = 0, nb = 0, la = 0, lb = 0;
    double *da = NULL, *db = NULL;
    if (!zr_tensor_extract(vm, "tensor_dot", argv[0], &sa, &na, &da, &la)) return false;
    if (!zr_tensor_extract(vm, "tensor_dot", argv[1], &sb, &nb, &db, &lb)) {
        free(sa); free(da);
        return false;
    }
    if (la != lb) {
        free(sa); free(da); free(sb); free(db);
        return vm_error(vm, "tensor_dot(): mismatched element counts (%zu vs %zu)", la, lb);
    }
    double acc = 0.0;
    for (size_t i = 0; i < la; i++) acc += da[i] * db[i];
    free(sa); free(da); free(sb); free(db);
    *out = val_num(acc);
    return true;
}

static bool bi_tensor_transpose2d(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 1) return vm_error(vm, "tensor_transpose2d() expects 1 argument");
    size_t *sa = NULL, na = 0, la = 0;
    double *da = NULL;
    if (!zr_tensor_extract(vm, "tensor_transpose2d", argv[0], &sa, &na, &da, &la)) return false;
    if (na != 2) {
        free(sa); free(da);
        return vm_error(vm, "tensor_transpose2d(): expected 2D tensor");
    }
    size_t R = sa[0], C = sa[1];
    double *dt = (double *)zr_xmalloc((la ? la : 1) * sizeof(double));
    for (size_t r = 0; r < R; r++) {
        for (size_t c = 0; c < C; c++) {
            dt[c * R + r] = da[r * C + c];
        }
    }
    size_t st[2] = { C, R };
    *out = zr_tensor_make(vm, st, 2, dt, la);
    free(sa); free(da); free(dt);
    return true;
}

static bool zr_tensor_unary(PbVM *vm, const char *fn, int argc, PbValue *argv, PbValue *out, int kind, double param) {
    if (argc != 1) return vm_error(vm, "%s() expects 1 argument", fn);
    size_t *sa = NULL, na = 0, la = 0;
    double *da = NULL;
    if (!zr_tensor_extract(vm, fn, argv[0], &sa, &na, &da, &la)) return false;
    const double sqrt_2_over_pi = 0.7978845608028654;
    for (size_t i = 0; i < la; i++) {
        double x = da[i];
        switch (kind) {
            case 0: da[i] = x > 0.0 ? x : 0.0; break;
            case 1: da[i] = x > 0.0 ? x : param * x; break;
            case 2: da[i] = 1.0 / (1.0 + exp(-x)); break;
            case 3: da[i] = tanh(x); break;
            case 4: da[i] = 0.5 * x * (1.0 + tanh(sqrt_2_over_pi * (x + 0.044715 * x * x * x))); break;
        }
    }
    *out = zr_tensor_make(vm, sa, na, da, la);
    free(sa); free(da);
    return true;
}

static bool bi_tensor_relu(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    return zr_tensor_unary(vm, "tensor_relu", argc, argv, out, 0, 0.0);
}
static bool bi_tensor_leaky_relu(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc < 1 || argc > 2) return vm_error(vm, "tensor_leaky_relu() expects 1 or 2 arguments");
    double alpha = 0.01;
    if (argc == 2 && !zr_req_num(vm, "tensor_leaky_relu", argv[1], &alpha)) return false;
    return zr_tensor_unary(vm, "tensor_leaky_relu", 1, argv, out, 1, alpha);
}
static bool bi_tensor_sigmoid(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    return zr_tensor_unary(vm, "tensor_sigmoid", argc, argv, out, 2, 0.0);
}
static bool bi_tensor_tanh(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    return zr_tensor_unary(vm, "tensor_tanh", argc, argv, out, 3, 0.0);
}
static bool bi_tensor_gelu(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    return zr_tensor_unary(vm, "tensor_gelu", argc, argv, out, 4, 0.0);
}

static bool zr_tensor_softmax_impl(PbVM *vm, const char *fn, int argc, PbValue *argv, PbValue *out, bool log_mode) {
    if (argc != 1) return vm_error(vm, "%s() expects 1 argument", fn);
    size_t *sa = NULL, na = 0, la = 0;
    double *da = NULL;
    if (!zr_tensor_extract(vm, fn, argv[0], &sa, &na, &da, &la)) return false;
    size_t last = na > 0 ? sa[na - 1] : 0;
    if (last == 0) {
        free(sa); free(da);
        return vm_error(vm, "%s(): empty last dimension", fn);
    }
    size_t outer = la / last;
    for (size_t o = 0; o < outer; o++) {
        double *row = da + o * last;
        double mx = row[0];
        for (size_t j = 1; j < last; j++) if (row[j] > mx) mx = row[j];
        double s = 0.0;
        for (size_t j = 0; j < last; j++) s += exp(row[j] - mx);
        if (log_mode) {
            double lse = mx + log(s);
            for (size_t j = 0; j < last; j++) row[j] -= lse;
        } else {
            for (size_t j = 0; j < last; j++) row[j] = exp(row[j] - mx) / s;
        }
    }
    *out = zr_tensor_make(vm, sa, na, da, la);
    free(sa); free(da);
    return true;
}

static bool bi_tensor_softmax(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    return zr_tensor_softmax_impl(vm, "tensor_softmax", argc, argv, out, false);
}
static bool bi_tensor_log_softmax(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    return zr_tensor_softmax_impl(vm, "tensor_log_softmax", argc, argv, out, true);
}

static bool bi_tensor_sum(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 1) return vm_error(vm, "tensor_sum() expects 1 argument");
    size_t *sa = NULL, na = 0, la = 0;
    double *da = NULL;
    if (!zr_tensor_extract(vm, "tensor_sum", argv[0], &sa, &na, &da, &la)) return false;
    double s = 0.0;
    for (size_t i = 0; i < la; i++) s += da[i];
    free(sa); free(da);
    *out = val_num(s);
    return true;
}

static bool bi_tensor_mean(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 1) return vm_error(vm, "tensor_mean() expects 1 argument");
    size_t *sa = NULL, na = 0, la = 0;
    double *da = NULL;
    if (!zr_tensor_extract(vm, "tensor_mean", argv[0], &sa, &na, &da, &la)) return false;
    if (la == 0) {
        free(sa); free(da);
        return vm_error(vm, "tensor_mean(): empty tensor");
    }
    double s = 0.0;
    for (size_t i = 0; i < la; i++) s += da[i];
    free(sa); free(da);
    *out = val_num(s / (double)la);
    return true;
}

static bool bi_tensor_argmax(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 1) return vm_error(vm, "tensor_argmax() expects 1 argument");
    size_t *sa = NULL, na = 0, la = 0;
    double *da = NULL;
    if (!zr_tensor_extract(vm, "tensor_argmax", argv[0], &sa, &na, &da, &la)) return false;
    if (la == 0) {
        free(sa); free(da);
        return vm_error(vm, "tensor_argmax(): empty tensor");
    }
    size_t best = 0;
    double bv = da[0];
    for (size_t i = 1; i < la; i++) {
        if (da[i] > bv) { bv = da[i]; best = i; }
    }
    free(sa); free(da);
    *out = val_num((double)best);
    return true;
}

static bool bi_tensor_layer_norm(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc < 1 || argc > 2) return vm_error(vm, "tensor_layer_norm() expects 1 or 2 arguments (tensor[, eps])");
    double eps = 1e-5;
    if (argc == 2 && !zr_req_num(vm, "tensor_layer_norm", argv[1], &eps)) return false;
    size_t *sa = NULL, na = 0, la = 0;
    double *da = NULL;
    if (!zr_tensor_extract(vm, "tensor_layer_norm", argv[0], &sa, &na, &da, &la)) return false;
    size_t last = na > 0 ? sa[na - 1] : 0;
    if (last == 0) {
        free(sa); free(da);
        return vm_error(vm, "tensor_layer_norm(): empty last dimension");
    }
    size_t outer = la / last;
    for (size_t o = 0; o < outer; o++) {
        double *row = da + o * last;
        double mean = 0.0;
        for (size_t j = 0; j < last; j++) mean += row[j];
        mean /= (double)last;
        double var = 0.0;
        for (size_t j = 0; j < last; j++) {
            double d = row[j] - mean;
            var += d * d;
        }
        var /= (double)last;
        double inv = 1.0 / sqrt(var + eps);
        for (size_t j = 0; j < last; j++) row[j] = (row[j] - mean) * inv;
    }
    *out = zr_tensor_make(vm, sa, na, da, la);
    free(sa); free(da);
    return true;
}

static bool bi_tensor_batch_norm(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc < 1 || argc > 2) return vm_error(vm, "tensor_batch_norm() expects 1 or 2 arguments (tensor[, eps])");
    double eps = 1e-5;
    if (argc == 2 && !zr_req_num(vm, "tensor_batch_norm", argv[1], &eps)) return false;
    size_t *sa = NULL, na = 0, la = 0;
    double *da = NULL;
    if (!zr_tensor_extract(vm, "tensor_batch_norm", argv[0], &sa, &na, &da, &la)) return false;
    if (na < 2 || sa[0] == 0 || sa[1] == 0) {
        free(sa); free(da);
        return vm_error(vm, "tensor_batch_norm(): tensor must have shape [N, C, ...] with N,C > 0");
    }
    size_t N = sa[0], C = sa[1];
    size_t spatial = la / (N * C);
    double count = (double)(N * spatial);
    for (size_t c = 0; c < C; c++) {
        double mean = 0.0;
        for (size_t n = 0; n < N; n++) {
            double *ptr = da + (n * C + c) * spatial;
            for (size_t s = 0; s < spatial; s++) mean += ptr[s];
        }
        mean /= count;
        double var = 0.0;
        for (size_t n = 0; n < N; n++) {
            double *ptr = da + (n * C + c) * spatial;
            for (size_t s = 0; s < spatial; s++) {
                double d = ptr[s] - mean;
                var += d * d;
            }
        }
        var /= count;
        double inv = 1.0 / sqrt(var + eps);
        for (size_t n = 0; n < N; n++) {
            double *ptr = da + (n * C + c) * spatial;
            for (size_t s = 0; s < spatial; s++) ptr[s] = (ptr[s] - mean) * inv;
        }
    }
    *out = zr_tensor_make(vm, sa, na, da, la);
    free(sa); free(da);
    return true;
}

static bool bi_tensor_embedding_lookup(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 2) return vm_error(vm, "tensor_embedding_lookup() expects 2 arguments (table, indices)");
    size_t *st = NULL, nt = 0, lt = 0;
    double *dt = NULL;
    if (!zr_tensor_extract(vm, "tensor_embedding_lookup", argv[0], &st, &nt, &dt, &lt)) return false;
    if (nt != 2) {
        free(st); free(dt);
        return vm_error(vm, "tensor_embedding_lookup(): table must be 2D [V, D]");
    }
    size_t V = st[0], D = st[1];
    PbArray *idx_arr = NULL;
    if (is_obj_type(argv[1], OBJ_ARRAY)) {
        idx_arr = AS_ARRAY(argv[1]);
    } else if (is_obj_type(argv[1], OBJ_MAP)) {
        PbValue dv;
        if (pb_map_get_cstr(AS_MAP(argv[1]), "data", &dv) && is_obj_type(dv, OBJ_ARRAY)) {
            idx_arr = AS_ARRAY(dv);
        }
    }
    if (!idx_arr) {
        free(st); free(dt);
        return vm_error(vm, "tensor_embedding_lookup(): indices must be an array or 1D tensor");
    }
    size_t N = idx_arr->len;
    if ((uint64_t)N * (uint64_t)D > ZR_MAX_TENSOR_ELEMENTS) {
        free(st); free(dt);
        return vm_error(vm, "tensor_embedding_lookup(): output tensor too large");
    }
    double *res = (double *)zr_xmalloc(((N > 0 && D > 0) ? (N * D) : 1) * sizeof(double));
    for (size_t i = 0; i < N; i++) {
        double iv = 0.0;
        if (!zr_req_num(vm, "tensor_embedding_lookup", idx_arr->items[i], &iv) ||
            iv < 0.0 || (size_t)iv >= V) {
            free(st); free(dt); free(res);
            return vm_error(vm, "tensor_embedding_lookup(): index out of range [0, %zu)", V);
        }
        memcpy(res + i * D, dt + ((size_t)iv) * D, D * sizeof(double));
    }
    size_t so[2] = { N, D };
    *out = zr_tensor_make(vm, so, 2, res, N * D);
    free(st); free(dt); free(res);
    return true;
}

static bool bi_tensor_cross_entropy(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 2) return vm_error(vm, "tensor_cross_entropy() expects 2 arguments (logits, targets)");
    size_t *sl = NULL, nl = 0, ll = 0;
    double *dl = NULL;
    if (!zr_tensor_extract(vm, "tensor_cross_entropy", argv[0], &sl, &nl, &dl, &ll)) return false;
    if (nl != 2 || sl[0] == 0 || sl[1] == 0) {
        free(sl); free(dl);
        return vm_error(vm, "tensor_cross_entropy(): logits must be 2D [N, C]");
    }
    size_t N = sl[0], C = sl[1];
    double total_loss = 0.0;
    if (is_obj_type(argv[1], OBJ_ARRAY)) {
        PbArray *ta = AS_ARRAY(argv[1]);
        if (ta->len != N) {
            free(sl); free(dl);
            return vm_error(vm, "tensor_cross_entropy(): target length (%zu) != batch size (%zu)", ta->len, N);
        }
        for (size_t n = 0; n < N; n++) {
            double tv = 0.0;
            if (!zr_req_num(vm, "tensor_cross_entropy", ta->items[n], &tv) ||
                tv < 0.0 || (size_t)tv >= C) {
                free(sl); free(dl);
                return vm_error(vm, "tensor_cross_entropy(): target class out of range");
            }
            const double *row = dl + n * C;
            double mx = row[0];
            for (size_t c = 1; c < C; c++) if (row[c] > mx) mx = row[c];
            double s = 0.0;
            for (size_t c = 0; c < C; c++) s += exp(row[c] - mx);
            total_loss += (mx + log(s)) - row[(size_t)tv];
        }
    } else {
        size_t *st = NULL, nt = 0, lt = 0;
        double *dt = NULL;
        if (!zr_tensor_extract(vm, "tensor_cross_entropy", argv[1], &st, &nt, &dt, &lt)) {
            free(sl); free(dl);
            return false;
        }
        if (nt == 1 && st[0] == N) {
            for (size_t n = 0; n < N; n++) {
                if (dt[n] < 0.0 || (size_t)dt[n] >= C) {
                    free(sl); free(dl); free(st); free(dt);
                    return vm_error(vm, "tensor_cross_entropy(): target class out of range");
                }
                const double *row = dl + n * C;
                double mx = row[0];
                for (size_t c = 1; c < C; c++) if (row[c] > mx) mx = row[c];
                double s = 0.0;
                for (size_t c = 0; c < C; c++) s += exp(row[c] - mx);
                total_loss += (mx + log(s)) - row[(size_t)dt[n]];
            }
        } else if (nt == 2 && st[0] == N && st[1] == C) {
            for (size_t n = 0; n < N; n++) {
                const double *row = dl + n * C;
                const double *trow = dt + n * C;
                double mx = row[0];
                for (size_t c = 1; c < C; c++) if (row[c] > mx) mx = row[c];
                double s = 0.0;
                for (size_t c = 0; c < C; c++) s += exp(row[c] - mx);
                double lse = mx + log(s);
                for (size_t c = 0; c < C; c++) total_loss -= trow[c] * (row[c] - lse);
            }
        } else {
            free(sl); free(dl); free(st); free(dt);
            return vm_error(vm, "tensor_cross_entropy(): targets must be [N] or [N, C]");
        }
        free(st); free(dt);
    }
    free(sl); free(dl);
    *out = val_num(total_loss / (double)N);
    return true;
}

static bool bi_tensor_conv2d(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc < 2 || argc > 4) return vm_error(vm, "tensor_conv2d() expects 2-4 arguments (x, kernel[, stride, padding])");
    double stride_d = 1.0, pad_d = 0.0;
    if (argc >= 3 && !zr_req_num(vm, "tensor_conv2d", argv[2], &stride_d)) return false;
    if (argc >= 4 && !zr_req_num(vm, "tensor_conv2d", argv[3], &pad_d)) return false;
    if (stride_d < 1.0 || pad_d < 0.0) return vm_error(vm, "tensor_conv2d(): invalid stride or padding");
    size_t stride = (size_t)stride_d, pad = (size_t)pad_d;
    size_t *sx = NULL, *sk = NULL, nx = 0, nk = 0, lx = 0, lk = 0;
    double *dx = NULL, *dk = NULL;
    if (!zr_tensor_extract(vm, "tensor_conv2d", argv[0], &sx, &nx, &dx, &lx)) return false;
    if (!zr_tensor_extract(vm, "tensor_conv2d", argv[1], &sk, &nk, &dk, &lk)) {
        free(sx); free(dx);
        return false;
    }
    if (nx != 4 || nk != 4 || sx[1] != sk[1]) {
        free(sx); free(dx); free(sk); free(dk);
        return vm_error(vm, "tensor_conv2d(): expected [N,Ci,H,W] input and [Co,Ci,KH,KW] kernel");
    }
    size_t N = sx[0], Ci = sx[1], H = sx[2], W = sx[3];
    size_t Co = sk[0], KH = sk[2], KW = sk[3];
    if (H + 2 * pad < KH || W + 2 * pad < KW) {
        free(sx); free(dx); free(sk); free(dk);
        return vm_error(vm, "tensor_conv2d(): kernel larger than padded input");
    }
    size_t Ho = (H + 2 * pad - KH) / stride + 1;
    size_t Wo = (W + 2 * pad - KW) / stride + 1;
    uint64_t total = (uint64_t)N * (uint64_t)Co * (uint64_t)Ho * (uint64_t)Wo;
    if (total > ZR_MAX_TENSOR_ELEMENTS) {
        free(sx); free(dx); free(sk); free(dk);
        return vm_error(vm, "tensor_conv2d(): output exceeds maximum tensor elements");
    }
    double *res = (double *)zr_xcalloc(total ? (size_t)total : 1, sizeof(double));
    for (size_t n = 0; n < N; n++) {
        for (size_t co = 0; co < Co; co++) {
            for (size_t ho = 0; ho < Ho; ho++) {
                for (size_t wo = 0; wo < Wo; wo++) {
                    double acc = 0.0;
                    for (size_t ci = 0; ci < Ci; ci++) {
                        for (size_t kh = 0; kh < KH; kh++) {
                            ptrdiff_t ih = (ptrdiff_t)(ho * stride + kh) - (ptrdiff_t)pad;
                            if (ih < 0 || (size_t)ih >= H) continue;
                            for (size_t kw = 0; kw < KW; kw++) {
                                ptrdiff_t iw = (ptrdiff_t)(wo * stride + kw) - (ptrdiff_t)pad;
                                if (iw < 0 || (size_t)iw >= W) continue;
                                acc += dx[((n * Ci + ci) * H + (size_t)ih) * W + (size_t)iw] *
                                       dk[((co * Ci + ci) * KH + kh) * KW + kw];
                            }
                        }
                    }
                    res[((n * Co + co) * Ho + ho) * Wo + wo] = acc;
                }
            }
        }
    }
    size_t so[4] = { N, Co, Ho, Wo };
    *out = zr_tensor_make(vm, so, 4, res, (size_t)total);
    free(sx); free(dx); free(sk); free(dk); free(res);
    return true;
}

static bool zr_tensor_pool2d(PbVM *vm, const char *fn, int argc, PbValue *argv, PbValue *out, bool is_max) {
    if (argc < 2 || argc > 3) return vm_error(vm, "%s() expects 2 or 3 arguments (x, k[, stride])", fn);
    double kd = 0.0;
    if (!zr_req_num(vm, fn, argv[1], &kd) || kd < 1.0) return vm_error(vm, "%s(): kernel size must be >= 1", fn);
    size_t K = (size_t)kd;
    size_t stride = K;
    if (argc == 3) {
        double sd = 0.0;
        if (!zr_req_num(vm, fn, argv[2], &sd) || sd < 1.0) return vm_error(vm, "%s(): stride must be >= 1", fn);
        stride = (size_t)sd;
    }
    size_t *sx = NULL, nx = 0, lx = 0;
    double *dx = NULL;
    if (!zr_tensor_extract(vm, fn, argv[0], &sx, &nx, &dx, &lx)) return false;
    if (nx != 4 || sx[2] < K || sx[3] < K) {
        free(sx); free(dx);
        return vm_error(vm, "%s(): expected [N,C,H,W] tensor with H,W >= kernel", fn);
    }
    size_t N = sx[0], C = sx[1], H = sx[2], W = sx[3];
    size_t Ho = (H - K) / stride + 1;
    size_t Wo = (W - K) / stride + 1;
    size_t total = N * C * Ho * Wo;
    double *res = (double *)zr_xmalloc((total ? total : 1) * sizeof(double));
    double inv_area = 1.0 / (double)(K * K);
    for (size_t n = 0; n < N; n++) {
        for (size_t c = 0; c < C; c++) {
            for (size_t ho = 0; ho < Ho; ho++) {
                for (size_t wo = 0; wo < Wo; wo++) {
                    double acc = is_max ? -INFINITY : 0.0;
                    for (size_t kh = 0; kh < K; kh++) {
                        for (size_t kw = 0; kw < K; kw++) {
                            double v = dx[((n * C + c) * H + (ho * stride + kh)) * W + (wo * stride + kw)];
                            if (is_max) { if (v > acc) acc = v; }
                            else acc += v;
                        }
                    }
                    res[((n * C + c) * Ho + ho) * Wo + wo] = is_max ? acc : acc * inv_area;
                }
            }
        }
    }
    size_t so[4] = { N, C, Ho, Wo };
    *out = zr_tensor_make(vm, so, 4, res, total);
    free(sx); free(dx); free(res);
    return true;
}

static bool bi_tensor_maxpool2d(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    return zr_tensor_pool2d(vm, "tensor_maxpool2d", argc, argv, out, true);
}
static bool bi_tensor_avgpool2d(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    return zr_tensor_pool2d(vm, "tensor_avgpool2d", argc, argv, out, false);
}
static bool bi_tensor_backend(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    (void)argc; (void)argv;
    *out = val_obj(pb_str_new(vm, "c99-native-simd", 15));
    return true;
}

/* ============================================================================
 * 8. Quantum Statevector Simulator, AI & Polyglot Builtins
 * ============================================================================ */

static void zr_q_pair_1q(double *r0, double *i0, double *r1, double *i1,
                         const char *gate, double theta) {
    double a_r = *r0, a_i = *i0, b_r = *r1, b_i = *i1;
    const double inv_s2 = 0.70710678118654752440;
    if (strcmp(gate, "X") == 0) {
        *r0 = b_r; *i0 = b_i; *r1 = a_r; *i1 = a_i;
    } else if (strcmp(gate, "Y") == 0) {
        *r0 = b_i; *i0 = -b_r; *r1 = -a_i; *i1 = a_r;
    } else if (strcmp(gate, "Z") == 0) {
        *r1 = -b_r; *i1 = -b_i;
    } else if (strcmp(gate, "H") == 0) {
        *r0 = (a_r + b_r) * inv_s2; *i0 = (a_i + b_i) * inv_s2;
        *r1 = (a_r - b_r) * inv_s2; *i1 = (a_i - b_i) * inv_s2;
    } else if (strcmp(gate, "S") == 0) {
        *r1 = -b_i; *i1 = b_r;
    } else if (strcmp(gate, "T") == 0) {
        *r1 = (b_r - b_i) * inv_s2; *i1 = (b_r + b_i) * inv_s2;
    } else if (strcmp(gate, "RX") == 0) {
        double c = cos(theta * 0.5), s = sin(theta * 0.5);
        *r0 = c * a_r + s * b_i; *i0 = c * a_i - s * b_r;
        *r1 = s * a_i + c * b_r; *i1 = -s * a_r + c * b_i;
    } else if (strcmp(gate, "RY") == 0) {
        double c = cos(theta * 0.5), s = sin(theta * 0.5);
        *r0 = c * a_r - s * b_r; *i0 = c * a_i - s * b_i;
        *r1 = s * a_r + c * b_r; *i1 = s * a_i + c * b_i;
    } else if (strcmp(gate, "RZ") == 0) {
        double c = cos(theta * 0.5), s = sin(theta * 0.5);
        *r0 = c * a_r + s * a_i; *i0 = c * a_i - s * a_r;
        *r1 = c * b_r - s * b_i; *i1 = c * b_i + s * b_r;
    }
}

static bool zr_q_apply_gate(PbVM *vm, const char *gate, const size_t *q, size_t nq_args,
                            bool has_theta, double theta, size_t n_qubits,
                            double *re, double *im) {
    size_t dim = (size_t)1 << n_qubits;
    if (strcmp(gate, "I") == 0) return true;
    if (strcmp(gate, "X") == 0 || strcmp(gate, "Y") == 0 || strcmp(gate, "Z") == 0 ||
        strcmp(gate, "H") == 0 || strcmp(gate, "S") == 0 || strcmp(gate, "T") == 0 ||
        strcmp(gate, "RX") == 0 || strcmp(gate, "RY") == 0 || strcmp(gate, "RZ") == 0) {
        if (nq_args != 1) return vm_error(vm, "quantum_simulate(): %s expects 1 qubit", gate);
        if ((gate[0] == 'R') && !has_theta) return vm_error(vm, "quantum_simulate(): %s requires 'theta'", gate);
        size_t bit = (size_t)1 << q[0];
        for (size_t i = 0; i < dim; i++) {
            if ((i & bit) == 0) {
                size_t j = i | bit;
                zr_q_pair_1q(&re[i], &im[i], &re[j], &im[j], gate, theta);
            }
        }
        return true;
    }
    if (strcmp(gate, "CX") == 0 || strcmp(gate, "CNOT") == 0) {
        if (nq_args != 2 || q[0] == q[1]) return vm_error(vm, "quantum_simulate(): CX expects 2 distinct qubits");
        size_t cb = (size_t)1 << q[0], tb = (size_t)1 << q[1];
        for (size_t i = 0; i < dim; i++) {
            if ((i & cb) && !(i & tb)) {
                size_t j = i | tb;
                double tr = re[i], ti = im[i];
                re[i] = re[j]; im[i] = im[j]; re[j] = tr; im[j] = ti;
            }
        }
        return true;
    }
    if (strcmp(gate, "CZ") == 0) {
        if (nq_args != 2 || q[0] == q[1]) return vm_error(vm, "quantum_simulate(): CZ expects 2 distinct qubits");
        size_t m = ((size_t)1 << q[0]) | ((size_t)1 << q[1]);
        for (size_t i = 0; i < dim; i++) {
            if ((i & m) == m) { re[i] = -re[i]; im[i] = -im[i]; }
        }
        return true;
    }
    if (strcmp(gate, "SWAP") == 0) {
        if (nq_args != 2 || q[0] == q[1]) return vm_error(vm, "quantum_simulate(): SWAP expects 2 distinct qubits");
        for (size_t i = 0; i < dim; i++) {
            size_t ba = (i >> q[0]) & 1, bb = (i >> q[1]) & 1;
            if (ba != bb) {
                size_t j = i ^ ((size_t)1 << q[0]) ^ ((size_t)1 << q[1]);
                if (i < j) {
                    double tr = re[i], ti = im[i];
                    re[i] = re[j]; im[i] = im[j]; re[j] = tr; im[j] = ti;
                }
            }
        }
        return true;
    }
    if (strcmp(gate, "CCX") == 0 || strcmp(gate, "TOFFOLI") == 0) {
        if (nq_args != 3) return vm_error(vm, "quantum_simulate(): CCX expects 3 qubits");
        size_t cm = ((size_t)1 << q[0]) | ((size_t)1 << q[1]);
        size_t tb = (size_t)1 << q[2];
        for (size_t i = 0; i < dim; i++) {
            if ((i & cm) == cm && !(i & tb)) {
                size_t j = i | tb;
                double tr = re[i], ti = im[i];
                re[i] = re[j]; im[i] = im[j]; re[j] = tr; im[j] = ti;
            }
        }
        return true;
    }
    if (strcmp(gate, "CCZ") == 0) {
        if (nq_args != 3) return vm_error(vm, "quantum_simulate(): CCZ expects 3 qubits");
        size_t m = ((size_t)1 << q[0]) | ((size_t)1 << q[1]) | ((size_t)1 << q[2]);
        for (size_t i = 0; i < dim; i++) {
            if ((i & m) == m) { re[i] = -re[i]; im[i] = -im[i]; }
        }
        return true;
    }
    if (strcmp(gate, "CSWAP") == 0 || strcmp(gate, "FREDKIN") == 0) {
        if (nq_args != 3) return vm_error(vm, "quantum_simulate(): CSWAP expects 3 qubits");
        size_t cb = (size_t)1 << q[0];
        for (size_t i = 0; i < dim; i++) {
            if (!(i & cb)) continue;
            size_t ba = (i >> q[1]) & 1, bb = (i >> q[2]) & 1;
            if (ba != bb) {
                size_t j = i ^ ((size_t)1 << q[1]) ^ ((size_t)1 << q[2]);
                if (i < j) {
                    double tr = re[i], ti = im[i];
                    re[i] = re[j]; im[i] = im[j]; re[j] = tr; im[j] = ti;
                }
            }
        }
        return true;
    }
    return vm_error(vm, "quantum_simulate(): unsupported gate '%s'", gate);
}

static bool bi_quantum_simulate(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 1 || !is_obj_type(argv[0], OBJ_MAP)) {
        return vm_error(vm, "quantum_simulate() expects 1 circuit map argument");
    }
    PbMap *circ = AS_MAP(argv[0]);
    PbValue nqv, opsv;
    if (!pb_map_get_cstr(circ, "n_qubits", &nqv) || nqv.tag != VAL_NUM || nqv.as.num < 0.0) {
        return vm_error(vm, "quantum_simulate(): circuit missing 'n_qubits'");
    }
    size_t n_qubits = (size_t)nqv.as.num;
    if (n_qubits > 20) return vm_error(vm, "quantum_simulate(): max 20 qubits supported (got %zu)", n_qubits);
    if (!pb_map_get_cstr(circ, "ops", &opsv) || !is_obj_type(opsv, OBJ_ARRAY)) {
        return vm_error(vm, "quantum_simulate(): circuit missing 'ops' array");
    }
    size_t dim = (size_t)1 << n_qubits;
    double *re = (double *)zr_xcalloc(dim, sizeof(double));
    double *im = (double *)zr_xcalloc(dim, sizeof(double));
    re[0] = 1.0;

    PbArray *ops = AS_ARRAY(opsv);
    for (size_t oi = 0; oi < ops->len; oi++) {
        if (!is_obj_type(ops->items[oi], OBJ_MAP)) {
            free(re); free(im);
            return vm_error(vm, "quantum_simulate(): op must be a map");
        }
        PbMap *opm = AS_MAP(ops->items[oi]);
        PbValue gv, qv, tv;
        if (!pb_map_get_cstr(opm, "gate", &gv) || !is_obj_type(gv, OBJ_STR)) {
            free(re); free(im);
            return vm_error(vm, "quantum_simulate(): op missing 'gate' string");
        }
        if (!pb_map_get_cstr(opm, "qubits", &qv) || !is_obj_type(qv, OBJ_ARRAY)) {
            free(re); free(im);
            return vm_error(vm, "quantum_simulate(): op missing 'qubits' array");
        }
        char gname[32];
        size_t gl = AS_STR(gv)->len < 31 ? AS_STR(gv)->len : 31;
        for (size_t k = 0; k < gl; k++) gname[k] = (char)toupper((unsigned char)AS_STR(gv)->chars[k]);
        gname[gl] = '\0';

        PbArray *qa = AS_ARRAY(qv);
        if (qa->len > 8) {
            free(re); free(im);
            return vm_error(vm, "quantum_simulate(): too many gate qubits");
        }
        size_t qidx[8];
        for (size_t k = 0; k < qa->len; k++) {
            if (qa->items[k].tag != VAL_NUM || qa->items[k].as.num < 0.0 ||
                (size_t)qa->items[k].as.num >= n_qubits) {
                free(re); free(im);
                return vm_error(vm, "quantum_simulate(): qubit index out of range [0, %zu)", n_qubits);
            }
            qidx[k] = (size_t)qa->items[k].as.num;
        }
        bool has_theta = pb_map_get_cstr(opm, "theta", &tv) && tv.tag == VAL_NUM;
        double theta = has_theta ? tv.as.num : 0.0;
        if (!zr_q_apply_gate(vm, gname, qidx, qa->len, has_theta, theta, n_qubits, re, im)) {
            free(re); free(im);
            return false;
        }
    }

    PbArray *probs = pb_array_new_raw(vm, dim);
    vm_push_root(vm, val_obj(probs));
    PbArray *ra = pb_array_new_raw(vm, dim);
    vm_push_root(vm, val_obj(ra));
    PbArray *ia = pb_array_new_raw(vm, dim);
    vm_push_root(vm, val_obj(ia));
    for (size_t i = 0; i < dim; i++) {
        pb_array_push(vm, probs, val_num(re[i] * re[i] + im[i] * im[i]));
        pb_array_push(vm, ra, val_num(re[i]));
        pb_array_push(vm, ia, val_num(im[i]));
    }
    free(re); free(im);
    PbMap *res = pb_map_new_raw(vm, 8);
    vm_push_root(vm, val_obj(res));
    pb_map_set_cstr(vm, res, "probs", val_obj(probs));
    pb_map_set_cstr(vm, res, "real", val_obj(ra));
    pb_map_set_cstr(vm, res, "imag", val_obj(ia));
    pb_map_set_cstr(vm, res, "n_qubits", val_num((double)n_qubits));
    vm_pop_roots(vm, 4);
    *out = val_obj(res);
    return true;
}

static bool zr_qasm_simulate(PbVM *vm, const char *qasm, size_t *out_nq, double **out_re, double **out_im) {
    size_t n_qubits = 0;
    const char *p = qasm;
    while (*p) {
        while (*p == ' ' || *p == '\t' || *p == '\r' || *p == '\n') p++;
        if (strncmp(p, "//", 2) == 0) {
            while (*p && *p != '\n') p++;
            continue;
        }
        if (strncmp(p, "qreg", 4) == 0 && isspace((unsigned char)p[4])) {
            const char *lb = strchr(p, '[');
            const char *semi = strchr(p, ';');
            if (lb && (!semi || lb < semi)) {
                int n = atoi(lb + 1);
                if (n > 0) n_qubits += (size_t)n;
            }
        }
        const char *semi = strchr(p, ';');
        const char *nl = strchr(p, '\n');
        if (semi && (!nl || semi < nl)) p = semi + 1;
        else if (nl) p = nl + 1;
        else break;
    }
    if (n_qubits == 0) n_qubits = 1;
    if (n_qubits > 20) return vm_error(vm, "QASM circuit exceeds 20 qubits (%zu)", n_qubits);
    size_t dim = (size_t)1 << n_qubits;
    double *re = (double *)zr_xcalloc(dim, sizeof(double));
    double *im = (double *)zr_xcalloc(dim, sizeof(double));
    re[0] = 1.0;

    p = qasm;
    while (*p) {
        while (*p && (isspace((unsigned char)*p) || *p == ';')) p++;
        if (!*p) break;
        if (strncmp(p, "//", 2) == 0) { while (*p && *p != '\n') p++; continue; }
        const char *end = p;
        while (*end && *end != ';' && *end != '\n') end++;
        char stmt[256];
        size_t sl = (size_t)(end - p) < sizeof(stmt) - 1 ? (size_t)(end - p) : sizeof(stmt) - 1;
        memcpy(stmt, p, sl);
        stmt[sl] = '\0';
        p = *end ? end + 1 : end;

        if (strncmp(stmt, "OPENQASM", 8) == 0 || strncmp(stmt, "include", 7) == 0 ||
            strncmp(stmt, "qreg", 4) == 0 || strncmp(stmt, "creg", 4) == 0 ||
            strncmp(stmt, "barrier", 7) == 0 || strncmp(stmt, "measure", 7) == 0) {
            continue;
        }
        char gname[32];
        size_t gi = 0, si = 0;
        while (stmt[si] && isalpha((unsigned char)stmt[si]) && gi < 31) {
            gname[gi++] = (char)toupper((unsigned char)stmt[si++]);
        }
        gname[gi] = '\0';
        if (gi == 0) continue;
        bool has_theta = false;
        double theta = 0.0;
        if (stmt[si] == '(') {
            si++;
            if (strncmp(stmt + si, "pi", 2) == 0) {
                theta = 3.14159265358979323846;
                si += 2;
                if (stmt[si] == '/') { theta /= atof(stmt + si + 1); }
                else if (stmt[si] == '*') { theta *= atof(stmt + si + 1); }
            } else {
                theta = atof(stmt + si);
            }
            has_theta = true;
            while (stmt[si] && stmt[si] != ')') si++;
            if (stmt[si] == ')') si++;
        }
        size_t qidx[8];
        size_t nq = 0;
        const char *qp = stmt + si;
        while ((qp = strchr(qp, '[')) != NULL && nq < 8) {
            int idx = atoi(qp + 1);
            if (idx >= 0 && (size_t)idx < n_qubits) qidx[nq++] = (size_t)idx;
            qp++;
        }
        if (nq > 0) {
            if (!zr_q_apply_gate(vm, gname, qidx, nq, has_theta, theta, n_qubits, re, im)) {
                free(re); free(im);
                return false;
            }
        }
    }
    *out_nq = n_qubits;
    *out_re = re;
    *out_im = im;
    return true;
}

static bool bi_quantum_backends(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    (void)argc; (void)argv;
    const char *js = "[{\"name\":\"c99-statevector\",\"available\":true},"
                     "{\"name\":\"qiskit\",\"available\":true},"
                     "{\"name\":\"cirq\",\"available\":true}]";
    *out = val_obj(pb_str_new(vm, js, strlen(js)));
    return true;
}

static bool bi_quantum_statevector(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (!vm_check_cap(vm, vm->caps.allow_exec, "quantum_statevector", "--allow-exec")) return false;
    if (argc < 1 || argc > 2 || !is_obj_type(argv[0], OBJ_STR)) {
        return vm_error(vm, "quantum_statevector(qasm[, backend]) expects 1-2 arguments");
    }
    size_t nq = 0;
    double *re = NULL, *im = NULL;
    if (!zr_qasm_simulate(vm, AS_STR(argv[0])->chars, &nq, &re, &im)) return false;
    size_t dim = (size_t)1 << nq;
    ZrBuf sb;
    zr_buf_init(&sb);
    zr_buf_append_cstr(&sb, "{\"backend\":\"c99-statevector\",\"n_qubits\":");
    zr_buf_printf(&sb, "%zu,\"statevector\":[", nq);
    for (size_t i = 0; i < dim; i++) {
        if (i > 0) zr_buf_push(&sb, ',');
        zr_buf_printf(&sb, "[%.12g,%.12g]", re[i], im[i]);
    }
    zr_buf_append_cstr(&sb, "]}");
    free(re); free(im);
    *out = val_obj(pb_str_new(vm, sb.data ? sb.data : "{}", sb.len));
    zr_buf_free(&sb);
    return true;
}

static bool bi_quantum_run(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (!vm_check_cap(vm, vm_effective_cap(vm, CAP_EXEC), "quantum_run", "--allow-exec")) return false;
    if (argc < 1 || argc > 3 || !is_obj_type(argv[0], OBJ_STR)) {
        return vm_error(vm, "quantum_run(qasm[, backend, shots]) expects 1-3 arguments");
    }
    uint32_t shots = 1024;
    if (argc >= 3) {
        double sd = 0.0;
        if (!zr_req_num(vm, "quantum_run", argv[2], &sd) || sd < 1.0) return false;
        shots = (uint32_t)sd;
    }
    size_t nq = 0;
    double *re = NULL, *im = NULL;
    if (!zr_qasm_simulate(vm, AS_STR(argv[0])->chars, &nq, &re, &im)) return false;
    size_t dim = (size_t)1 << nq;
    uint32_t *counts = (uint32_t *)zr_xcalloc(dim, sizeof(uint32_t));
    uint64_t rng = 0x9e3779b97f4a7c15ULL ^ (uint64_t)shots;
    for (uint32_t s = 0; s < shots; s++) {
        rng ^= rng >> 12; rng ^= rng << 25; rng ^= rng >> 27;
        double u = (double)((rng * 2685821657736338717ULL) >> 11) * (1.0 / 9007199254740992.0);
        double cum = 0.0;
        size_t chosen = dim - 1;
        for (size_t i = 0; i < dim; i++) {
            cum += re[i] * re[i] + im[i] * im[i];
            if (u <= cum) { chosen = i; break; }
        }
        counts[chosen]++;
    }
    free(re); free(im);
    ZrBuf sb;
    zr_buf_init(&sb);
    zr_buf_printf(&sb, "{\"backend\":\"c99-statevector\",\"shots\":%u,\"counts\":{", shots);
    bool first = true;
    for (size_t i = 0; i < dim; i++) {
        if (counts[i] == 0) continue;
        if (!first) zr_buf_push(&sb, ',');
        first = false;
        zr_buf_push(&sb, '"');
        for (size_t b = nq; b > 0; b--) {
            zr_buf_push(&sb, ((i >> (b - 1)) & 1) ? '1' : '0');
        }
        zr_buf_printf(&sb, "\":%u", counts[i]);
    }
    zr_buf_append_cstr(&sb, "}}");
    free(counts);
    *out = val_obj(pb_str_new(vm, sb.data ? sb.data : "{}", sb.len));
    zr_buf_free(&sb);
    return true;
}

static bool bi_ai_token_count(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 1 || !is_obj_type(argv[0], OBJ_STR)) {
        return vm_error(vm, "ai_token_count() expects 1 string argument");
    }
    PbStr *s = AS_STR(argv[0]);
    if (s->len == 0) { *out = val_num(0.0); return true; }
    size_t words = 0;
    bool in_w = false;
    for (size_t i = 0; i < s->len; i++) {
        unsigned char c = (unsigned char)s->chars[i];
        if (isalnum(c) || c >= 0x80) {
            if (!in_w) { words++; in_w = true; }
        } else {
            in_w = false;
            if (!isspace(c)) words++;
        }
    }
    size_t char_est = (s->len + 3) / 4;
    size_t est = words > char_est ? words : char_est;
    *out = val_num((double)(est == 0 ? 1 : est));
    return true;
}

static bool bi_ai_chat(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (!vm_check_cap(vm, vm_effective_cap(vm, CAP_HTTP), "ai_chat", "--allow-http")) return false;
    if (argc < 1 || argc > 2 || !is_obj_type(argv[0], OBJ_STR)) {
        return vm_error(vm, "ai_chat(prompt[, options_json]) expects 1 or 2 arguments");
    }
    const char *ollama_host = getenv("OLLAMA_HOST");
    if (!ollama_host || !*ollama_host) ollama_host = "http://127.0.0.1:11434";
    char url[512];
    snprintf(url, sizeof(url), "%s/api/generate", ollama_host);

    PbMap *req = pb_map_new_raw(vm, 8);
    vm_push_root(vm, val_obj(req));
    const char *model = getenv("PBLANG_AI_MODEL");
    if (!model || !*model) model = "llama3.2";
    pb_map_set_cstr(vm, req, "model", val_obj(pb_str_new(vm, model, strlen(model))));
    pb_map_set_cstr(vm, req, "prompt", argv[0]);
    pb_map_set_cstr(vm, req, "stream", val_bool(false));

    PbValue body_val = val_nil();
    PbValue req_v = val_obj(req);
    if (!bi_json_stringify(vm, 1, &req_v, &body_val)) {
        vm_pop_roots(vm, 1);
        return false;
    }
    vm_push_root(vm, body_val);
    PbValue hargs[3];
    hargs[0] = val_obj(pb_str_new(vm, url, strlen(url)));
    vm_push_root(vm, hargs[0]);
    hargs[1] = body_val;
    hargs[2] = val_obj(pb_str_new(vm, "application/json", 16));
    vm_push_root(vm, hargs[2]);
    bool ok = bi_http_post(vm, 3, hargs, out);
    vm_pop_roots(vm, 4);
    return ok;
}

static bool bi_ai_embed(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (!vm_check_cap(vm, vm_effective_cap(vm, CAP_HTTP), "ai_embed", "--allow-http")) return false;
    if (argc < 1 || argc > 2 || !is_obj_type(argv[0], OBJ_STR)) {
        return vm_error(vm, "ai_embed(text[, options_json]) expects 1 or 2 arguments");
    }
    const char *remote = getenv("PBLANG_AI_EMBED_URL");
    if (remote && *remote) {
        PbMap *req = pb_map_new_raw(vm, 4);
        vm_push_root(vm, val_obj(req));
        pb_map_set_cstr(vm, req, "input", argv[0]);
        PbValue body = val_nil(), rv = val_obj(req);
        if (!bi_json_stringify(vm, 1, &rv, &body)) { vm_pop_roots(vm, 1); return false; }
        vm_push_root(vm, body);
        PbValue hargs[3];
        hargs[0] = val_obj(pb_str_new(vm, remote, strlen(remote)));
        vm_push_root(vm, hargs[0]);
        hargs[1] = body;
        hargs[2] = val_obj(pb_str_new(vm, "application/json", 16));
        vm_push_root(vm, hargs[2]);
        bool ok = bi_http_post(vm, 3, hargs, out);
        vm_pop_roots(vm, 4);
        return ok;
    }
    PbStr *s = AS_STR(argv[0]);
    double vec[64];
    memset(vec, 0, sizeof(vec));
    for (size_t i = 0; i < s->len; i++) {
        uint32_t h = 2166136261u ^ (unsigned char)s->chars[i];
        if (i + 1 < s->len) h = (h ^ (unsigned char)s->chars[i + 1]) * 16777619u;
        vec[h & 63u] += (h & 64u) ? 1.0 : -1.0;
    }
    double norm = 0.0;
    for (int i = 0; i < 64; i++) norm += vec[i] * vec[i];
    norm = norm > 0.0 ? 1.0 / sqrt(norm) : 0.0;
    PbArray *arr = pb_array_new_raw(vm, 64);
    vm_push_root(vm, val_obj(arr));
    for (int i = 0; i < 64; i++) pb_array_push(vm, arr, val_num(vec[i] * norm));
    vm_pop_roots(vm, 1);
    *out = val_obj(arr);
    return true;
}

static bool bi_onnx_infer(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (!vm_check_cap(vm, vm_effective_cap(vm, CAP_EXEC), "onnx_infer", "--allow-exec")) return false;
    if (argc < 1 || !is_obj_type(argv[0], OBJ_STR)) {
        return vm_error(vm, "onnx_infer(model_path[, input[, options]]) expects model_path string");
    }
    PbValue in_js = val_obj(pb_str_new(vm, "{}", 2));
    if (argc >= 2) {
        if (is_obj_type(argv[1], OBJ_STR)) in_js = argv[1];
        else if (!bi_json_stringify(vm, 1, &argv[1], &in_js)) return false;
    }
    vm_push_root(vm, in_js);
    const char *py_script =
        "import json, sys\n"
        "import onnxruntime as ort, numpy as np\n"
        "sess = ort.InferenceSession(sys.argv[1])\n"
        "inp = json.loads(sys.argv[2])\n"
        "feeds = {k: np.array(v, dtype=np.float32) for k, v in inp.items()}\n"
        "outs = sess.run(None, feeds)\n"
        "print(json.dumps([o.tolist() for o in outs]))\n";
    PbArray *cmd_args = pb_array_new_raw(vm, 4);
    vm_push_root(vm, val_obj(cmd_args));
    pb_array_push(vm, cmd_args, val_obj(pb_str_new(vm, "-c", 2)));
    pb_array_push(vm, cmd_args, val_obj(pb_str_new(vm, py_script, strlen(py_script))));
    pb_array_push(vm, cmd_args, argv[0]);
    pb_array_push(vm, cmd_args, in_js);
    PbValue sp_args[2];
    sp_args[0] = val_obj(pb_str_new(vm, "python3", 7));
    sp_args[1] = val_obj(cmd_args);
    vm_push_root(vm, sp_args[0]);
    PbValue res = val_nil();
    bool ok = bi_proc_spawn_wait(vm, 2, sp_args, &res);
    vm_pop_roots(vm, 3);
    if (!ok) return false;
    vm_push_root(vm, res);
    PbValue code_v = val_nil(), out_v = val_nil(), err_v = val_nil();
    if (is_obj_type(res, OBJ_MAP)) {
        pb_map_get_cstr(AS_MAP(res), "exit_code", &code_v);
        pb_map_get_cstr(AS_MAP(res), "stdout", &out_v);
        pb_map_get_cstr(AS_MAP(res), "stderr", &err_v);
    }
    if (code_v.tag != VAL_NUM || (int)code_v.as.num != 0) {
        const char *es = is_obj_type(err_v, OBJ_STR) ? AS_STR(err_v)->chars : "onnxruntime execution failed";
        vm_pop_roots(vm, 1);
        return vm_error(vm, "onnx_infer(): %s", es);
    }
    bool pok = bi_json_parse(vm, 1, &out_v, out);
    vm_pop_roots(vm, 1);
    return pok;
}

/* Polyglot bridge builtins */
static void zr_polyglot_cache_root(char out_dir[512]) {
    out_dir[0] = '\0';
#ifdef _WIN32
    const char *la = getenv("LOCALAPPDATA");
    if (!la || !*la) la = getenv("TEMP");
    if (la && *la) snprintf(out_dir, 512, "%s\\pblang\\zr_polyglot_v2", la);
#else
    const char *xdg = getenv("XDG_CACHE_HOME");
    if (xdg && *xdg) {
        snprintf(out_dir, 512, "%s/pblang/zr_polyglot_v2", xdg);
    } else {
        const char *hm = getenv("HOME");
        if (hm && *hm) snprintf(out_dir, 512, "%s/.cache/pblang/zr_polyglot_v2", hm);
        else snprintf(out_dir, 512, "/tmp/pblang_zr_polyglot_v2");
    }
#endif
}

static void zr_sha256_short_hex(const char *s, size_t len, char out_hex[25]) {
    static const char hx[] = "0123456789abcdef";
    uint8_t dig[32];
    zr_sha256((const uint8_t *)s, len, dig);
    for (int i = 0; i < 12; i++) {
        out_hex[i * 2]     = hx[(dig[i] >> 4) & 0xF];
        out_hex[i * 2 + 1] = hx[dig[i] & 0xF];
    }
    out_hex[24] = '\0';
}

static void zr_polyglot_lang_cmd(const char *lang, const char **bin, const char **flag) {
    char low[32];
    size_t i = 0;
    while (lang[i] && i < 31) { low[i] = (char)tolower((unsigned char)lang[i]); i++; }
    low[i] = '\0';
    *flag = "-c";
    if (strcmp(low, "python") == 0 || strcmp(low, "py") == 0 || strcmp(low, "python3") == 0) { *bin = "python3"; *flag = "-c"; }
    else if (strcmp(low, "js") == 0 || strcmp(low, "javascript") == 0 || strcmp(low, "node") == 0) { *bin = "node"; *flag = "-e"; }
    else if (strcmp(low, "deno") == 0) { *bin = "deno"; *flag = "eval"; }
    else if (strcmp(low, "bun") == 0) { *bin = "bun"; *flag = "-e"; }
    else if (strcmp(low, "bash") == 0) { *bin = "bash"; *flag = "-c"; }
    else if (strcmp(low, "sh") == 0 || strcmp(low, "shell") == 0 || strcmp(low, "posix-sh") == 0) { *bin = "sh"; *flag = "-c"; }
    else if (strcmp(low, "zsh") == 0) { *bin = "zsh"; *flag = "-c"; }
    else if (strcmp(low, "fish") == 0) { *bin = "fish"; *flag = "-c"; }
    else if (strcmp(low, "pwsh") == 0 || strcmp(low, "powershell") == 0 || strcmp(low, "ps1") == 0) { *bin = "pwsh"; *flag = "-c"; }
    else if (strcmp(low, "ruby") == 0 || strcmp(low, "rb") == 0) { *bin = "ruby"; *flag = "-e"; }
    else if (strcmp(low, "perl") == 0 || strcmp(low, "pl") == 0) { *bin = "perl"; *flag = "-e"; }
    else if (strcmp(low, "raku") == 0 || strcmp(low, "perl6") == 0) { *bin = "raku"; *flag = "-e"; }
    else if (strcmp(low, "lua") == 0) { *bin = "lua"; *flag = "-e"; }
    else if (strcmp(low, "luajit") == 0) { *bin = "luajit"; *flag = "-e"; }
    else if (strcmp(low, "php") == 0) { *bin = "php"; *flag = "-r"; }
    else if (strcmp(low, "r") == 0 || strcmp(low, "rscript") == 0) { *bin = "Rscript"; *flag = "-e"; }
    else if (strcmp(low, "tcl") == 0 || strcmp(low, "tclsh") == 0) { *bin = "tclsh"; *flag = "-c"; }
    else if (strcmp(low, "awk") == 0 || strcmp(low, "gawk") == 0) { *bin = "awk"; *flag = ""; }
    else if (strcmp(low, "sed") == 0) { *bin = "sed"; *flag = "-e"; }
    else if (strcmp(low, "m4") == 0) { *bin = "m4"; *flag = ""; }
    else if (strcmp(low, "jq") == 0) { *bin = "jq"; *flag = "-r"; }
    else if (strcmp(low, "bc") == 0) { *bin = "bc"; *flag = "-l"; }
    else if (strcmp(low, "dc") == 0) { *bin = "dc"; *flag = "-e"; }
    else if (strcmp(low, "gnuplot") == 0) { *bin = "gnuplot"; *flag = "-e"; }
    else if (strcmp(low, "julia") == 0 || strcmp(low, "jl") == 0) { *bin = "julia"; *flag = "-e"; }
    else if (strcmp(low, "octave") == 0 || strcmp(low, "matlab") == 0) { *bin = "octave"; *flag = "--eval"; }
    else if (strcmp(low, "maxima") == 0) { *bin = "maxima"; *flag = "--batch-string"; }
    else if (strcmp(low, "gap") == 0) { *bin = "gap"; *flag = "-c"; }
    else if (strcmp(low, "pari") == 0 || strcmp(low, "gp") == 0) { *bin = "gp"; *flag = "-q"; }
    else if (strcmp(low, "lisp") == 0 || strcmp(low, "commonlisp") == 0 || strcmp(low, "sbcl") == 0) { *bin = "sbcl"; *flag = "--eval"; }
    else if (strcmp(low, "scheme") == 0 || strcmp(low, "guile") == 0) { *bin = "guile"; *flag = "-c"; }
    else if (strcmp(low, "racket") == 0 || strcmp(low, "rkt") == 0) { *bin = "racket"; *flag = "-e"; }
    else if (strcmp(low, "clojure") == 0 || strcmp(low, "clj") == 0) { *bin = "clojure"; *flag = "-e"; }
    else if (strcmp(low, "janet") == 0) { *bin = "janet"; *flag = "-e"; }
    else if (strcmp(low, "fennel") == 0) { *bin = "fennel"; *flag = "-e"; }
    else if (strcmp(low, "elixir") == 0 || strcmp(low, "ex") == 0 || strcmp(low, "exs") == 0) { *bin = "elixir"; *flag = "-e"; }
    else if (strcmp(low, "groovy") == 0) { *bin = "groovy"; *flag = "-e"; }
    else if (strcmp(low, "ocaml") == 0) { *bin = "ocaml"; *flag = "-e"; }
    else if (strcmp(low, "scala") == 0) { *bin = "scala"; *flag = "-e"; }
    else if (strcmp(low, "prolog") == 0 || strcmp(low, "swipl") == 0) { *bin = "swipl"; *flag = "-g"; }
    else if (strcmp(low, "smalltalk") == 0 || strcmp(low, "gst") == 0) { *bin = "gst"; *flag = "-e"; }
    else if (strcmp(low, "forth") == 0 || strcmp(low, "gforth") == 0 || strcmp(low, "fth") == 0) { *bin = "gforth"; *flag = "-e"; }
    else if (strcmp(low, "idris") == 0 || strcmp(low, "idris2") == 0) { *bin = "idris2"; *flag = "--exec"; }
    else if (strcmp(low, "qsharp") == 0 || strcmp(low, "qs") == 0 || strcmp(low, "q#") == 0) { *bin = "python3"; *flag = "-c"; }
    else if (strcmp(low, "sql") == 0 || strcmp(low, "sqlite") == 0) { *bin = "sqlite3"; *flag = ":memory:"; }
    else if (strcmp(low, "postgresql") == 0 || strcmp(low, "psql") == 0) { *bin = "psql"; *flag = "-c"; }
    else if (strcmp(low, "mysql") == 0 || strcmp(low, "mariadb") == 0) { *bin = "mysql"; *flag = "-e"; }
    else if (strcmp(low, "duckdb") == 0) { *bin = "duckdb"; *flag = "-c"; }
    else if (strcmp(low, "mongodb") == 0 || strcmp(low, "mongosh") == 0) { *bin = "mongosh"; *flag = "--eval"; }
    else if (strcmp(low, "redis") == 0) { *bin = "redis-cli"; *flag = "--eval"; }
    else if (strcmp(low, "sass") == 0 || strcmp(low, "scss") == 0) { *bin = "sass"; *flag = "--stdin"; }
    else if (strcmp(low, "less") == 0) { *bin = "lessc"; *flag = "-"; }
    else { *bin = NULL; *flag = NULL; }
}

static bool bi_polyglot_detect(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    (void)argc; (void)argv;
    static const struct { const char *name; const char *bin; } probes[] = {
        {"python", "python3"}, {"js", "node"}, {"deno", "deno"}, {"bun", "bun"},
        {"bash", "bash"}, {"sh", "sh"}, {"zsh", "zsh"}, {"fish", "fish"}, {"pwsh", "pwsh"},
        {"ruby", "ruby"}, {"perl", "perl"}, {"raku", "raku"}, {"lua", "lua"}, {"php", "php"},
        {"r", "Rscript"}, {"tcl", "tclsh"}, {"awk", "awk"}, {"sed", "sed"}, {"m4", "m4"},
        {"jq", "jq"}, {"bc", "bc"}, {"dc", "dc"}, {"gnuplot", "gnuplot"},
        {"julia", "julia"}, {"octave", "octave"}, {"lisp", "sbcl"}, {"scheme", "guile"},
        {"racket", "racket"}, {"clojure", "clojure"}, {"elixir", "elixir"}, {"erlang", "erl"},
        {"prolog", "swipl"}, {"smalltalk", "gst"}, {"forth", "gforth"}, {"ocaml", "ocaml"},
        {"scala", "scala"}, {"groovy", "groovy"}, {"haskell", "ghc"}, {"idris2", "idris2"},
        {"lean", "lean"}, {"coq", "coqc"}, {"z3", "z3"},
        {"c", "gcc"}, {"clang", "clang"}, {"cpp", "g++"}, {"rust", "rustc"}, {"go", "go"},
        {"zig", "zig"}, {"pascal", "fpc"}, {"fortran", "gfortran"}, {"cobol", "cobc"},
        {"ada", "gnatmake"}, {"d", "gdc"}, {"nim", "nim"}, {"crystal", "crystal"},
        {"swift", "swiftc"}, {"java", "javac"}, {"kotlin", "kotlinc"}, {"dotnet", "dotnet"},
        {"nasm", "nasm"}, {"gas", "as"}, {"avr-gcc", "avr-gcc"},
        {"verilog", "iverilog"}, {"vhdl", "ghdl"}, {"spice", "ngspice"}, {"openscad", "openscad"},
        {"sqlite", "sqlite3"}, {"psql", "psql"}, {"mysql", "mysql"}, {"duckdb", "duckdb"},
        {"latex", "pdflatex"}, {"graphviz", "dot"}, {"lilypond", "lilypond"}
    };
    PbMap *m = pb_map_new_raw(vm, 128);
    vm_push_root(vm, val_obj(m));
    for (size_t i = 0; i < sizeof(probes) / sizeof(probes[0]); i++) {
        PbValue arg = val_obj(pb_str_new(vm, probes[i].bin, strlen(probes[i].bin)));
        vm_push_root(vm, arg);
        PbValue wv = val_nil();
        bool found = bi_os_which(vm, 1, &arg, &wv) && is_obj_type(wv, OBJ_STR) && AS_STR(wv)->len > 0;
        vm->err_msg[0] = '\0';
        vm_pop_roots(vm, 1);
        pb_map_set_cstr(vm, m, probes[i].name, val_bool(found));
    }
    PbValue mv = val_obj(m);
    bool ok = bi_json_stringify(vm, 1, &mv, out);
    vm_pop_roots(vm, 1);
    return ok;
}

static bool zr_polyglot_format_result(PbVM *vm, PbValue proc_map, PbValue *out) {
    if (!is_obj_type(proc_map, OBJ_MAP)) return false;
    PbMap *pm = AS_MAP(proc_map);
    PbValue ec = val_num(1.0), so = val_nil(), se = val_nil();
    pb_map_get_cstr(pm, "exit_code", &ec);
    pb_map_get_cstr(pm, "stdout", &so);
    pb_map_get_cstr(pm, "stderr", &se);
    PbMap *rm = pb_map_new_raw(vm, 8);
    vm_push_root(vm, val_obj(rm));
    pb_map_set_cstr(vm, rm, "success", val_bool(ec.tag == VAL_NUM && (int)ec.as.num == 0));
    pb_map_set_cstr(vm, rm, "exit_code", ec);
    pb_map_set_cstr(vm, rm, "stdout", is_obj_type(so, OBJ_STR) ? so : val_obj(pb_str_new(vm, "", 0)));
    pb_map_set_cstr(vm, rm, "stderr", is_obj_type(se, OBJ_STR) ? se : val_obj(pb_str_new(vm, "", 0)));
    PbValue rv = val_obj(rm);
    bool ok = bi_json_stringify(vm, 1, &rv, out);
    vm_pop_roots(vm, 1);
    return ok;
}

static bool bi_polyglot_exec(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (!vm_check_cap(vm, vm_effective_cap(vm, CAP_EXEC), "polyglot_exec", "--allow-exec")) return false;
    if (argc != 2 || !is_obj_type(argv[0], OBJ_STR) || !is_obj_type(argv[1], OBJ_STR)) {
        return vm_error(vm, "polyglot_exec(lang, code) expects 2 string arguments");
    }
    const char *bin = NULL, *flag = NULL;
    zr_polyglot_lang_cmd(AS_STR(argv[0])->chars, &bin, &flag);
    if (!bin) return vm_error(vm, "polyglot_exec(): unsupported language '%s'", AS_STR(argv[0])->chars);
    PbArray *ca = pb_array_new_raw(vm, 2);
    vm_push_root(vm, val_obj(ca));
    if (flag && flag[0] != '\0') {
        pb_array_push(vm, ca, val_obj(pb_str_new(vm, flag, strlen(flag))));
    }
    pb_array_push(vm, ca, argv[1]);
    PbValue sargs[2];
    sargs[0] = val_obj(pb_str_new(vm, bin, strlen(bin)));
    sargs[1] = val_obj(ca);
    vm_push_root(vm, sargs[0]);
    PbValue pm = val_nil();
    bool ok = bi_proc_spawn_wait(vm, 2, sargs, &pm);
    vm_pop_roots(vm, 2);
    if (!ok) return false;
    vm_push_root(vm, pm);
    bool fok = zr_polyglot_format_result(vm, pm, out);
    vm_pop_roots(vm, 1);
    return fok;
}

static bool bi_polyglot_exec_with_input(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (!vm_check_cap(vm, vm_effective_cap(vm, CAP_EXEC), "polyglot_exec_with_input", "--allow-exec")) return false;
    if (argc != 3 || !is_obj_type(argv[0], OBJ_STR) || !is_obj_type(argv[1], OBJ_STR) || !is_obj_type(argv[2], OBJ_STR)) {
        return vm_error(vm, "polyglot_exec_with_input(lang, code, input) expects 3 string arguments");
    }
    const char *bin = NULL, *flag = NULL;
    zr_polyglot_lang_cmd(AS_STR(argv[0])->chars, &bin, &flag);
    if (!bin) return vm_error(vm, "polyglot_exec_with_input(): unsupported language '%s'", AS_STR(argv[0])->chars);
    PbArray *ca = pb_array_new_raw(vm, 2);
    vm_push_root(vm, val_obj(ca));
    if (flag && flag[0] != '\0') {
        pb_array_push(vm, ca, val_obj(pb_str_new(vm, flag, strlen(flag))));
    }
    pb_array_push(vm, ca, argv[1]);
    PbMap *opts = pb_map_new_raw(vm, 4);
    vm_push_root(vm, val_obj(opts));
    pb_map_set_cstr(vm, opts, "stdin", argv[2]);
    PbValue sargs[3];
    sargs[0] = val_obj(pb_str_new(vm, bin, strlen(bin)));
    sargs[1] = val_obj(ca);
    sargs[2] = val_obj(opts);
    vm_push_root(vm, sargs[0]);
    PbValue pm = val_nil();
    bool ok = bi_proc_spawn_wait(vm, 3, sargs, &pm);
    vm_pop_roots(vm, 3);
    if (!ok) return false;
    vm_push_root(vm, pm);
    bool fok = zr_polyglot_format_result(vm, pm, out);
    vm_pop_roots(vm, 1);
    return fok;
}

static bool bi_polyglot_exec_compiled(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (!vm_check_cap(vm, vm_effective_cap(vm, CAP_EXEC), "polyglot_exec_compiled", "--allow-exec")) return false;
    if (argc != 2 || !is_obj_type(argv[0], OBJ_STR) || !is_obj_type(argv[1], OBJ_STR)) {
        return vm_error(vm, "polyglot_exec_compiled(lang, code) expects 2 string arguments");
    }
    char low[32];
    size_t li = 0;
    const char *raw_lang = AS_STR(argv[0])->chars;
    while (raw_lang[li] && li < 31) { low[li] = (char)tolower((unsigned char)raw_lang[li]); li++; }
    low[li] = '\0';

    const char *ext = ".c", *compiler = "gcc", *runner = NULL;
    int mode = 0; /* 0: compiler src -o bin; 1: compiler run src (go/zig); 2: fpc -FEdir -obin src; 3: cobc -x -o bin src; 4: iverilog -o bin src + vvp bin */
    if (strcmp(low, "c") == 0 || strcmp(low, "c99") == 0 || strcmp(low, "c11") == 0) { ext = ".c"; compiler = "gcc"; }
    else if (strcmp(low, "cpp") == 0 || strcmp(low, "c++") == 0 || strcmp(low, "cxx") == 0) { ext = ".cpp"; compiler = "g++"; }
    else if (strcmp(low, "rust") == 0 || strcmp(low, "rs") == 0) { ext = ".rs"; compiler = "rustc"; }
    else if (strcmp(low, "fortran") == 0 || strcmp(low, "f90") == 0 || strcmp(low, "f95") == 0 || strcmp(low, "f77") == 0) { ext = ".f90"; compiler = "gfortran"; }
    else if (strcmp(low, "d") == 0) { ext = ".d"; compiler = "gdc"; }
    else if (strcmp(low, "haskell") == 0 || strcmp(low, "hs") == 0) { ext = ".hs"; compiler = "ghc"; }
    else if (strcmp(low, "objc") == 0 || strcmp(low, "objective-c") == 0) { ext = ".m"; compiler = "gcc"; }
    else if (strcmp(low, "ada") == 0 || strcmp(low, "adb") == 0) { ext = ".adb"; compiler = "gnatmake"; }
    else if (strcmp(low, "swift") == 0) { ext = ".swift"; compiler = "swiftc"; }
    else if (strcmp(low, "vala") == 0) { ext = ".vala"; compiler = "valac"; }
    else if (strcmp(low, "go") == 0 || strcmp(low, "golang") == 0) { ext = ".go"; compiler = "go"; mode = 1; }
    else if (strcmp(low, "zig") == 0) { ext = ".zig"; compiler = "zig"; mode = 1; }
    else if (strcmp(low, "nim") == 0) { ext = ".nim"; compiler = "nim"; mode = 1; }
    else if (strcmp(low, "crystal") == 0 || strcmp(low, "cr") == 0) { ext = ".cr"; compiler = "crystal"; mode = 1; }
    else if (strcmp(low, "odin") == 0) { ext = ".odin"; compiler = "odin"; mode = 1; }
    else if (strcmp(low, "v") == 0 || strcmp(low, "vlang") == 0) { ext = ".v"; compiler = "v"; mode = 1; }
    else if (strcmp(low, "mojo") == 0) { ext = ".mojo"; compiler = "mojo"; mode = 1; }
    else if (strcmp(low, "pascal") == 0 || strcmp(low, "pas") == 0 || strcmp(low, "delphi") == 0 || strcmp(low, "fpc") == 0) { ext = ".pas"; compiler = "fpc"; mode = 2; }
    else if (strcmp(low, "cobol") == 0 || strcmp(low, "cob") == 0) { ext = ".cob"; compiler = "cobc"; mode = 3; }
    else if (strcmp(low, "verilog") == 0 || strcmp(low, "sv") == 0 || strcmp(low, "systemverilog") == 0) { ext = ".v"; compiler = "iverilog"; runner = "vvp"; mode = 4; }
    else {
        return vm_error(vm, "polyglot_exec_compiled(): unsupported compiled language '%s'", raw_lang);
    }
    const char *tmpd = getenv("TMPDIR");
#ifdef _WIN32
    if (!tmpd || !*tmpd) tmpd = getenv("TEMP");
    if (!tmpd || !*tmpd) tmpd = ".";
#else
    if (!tmpd || !*tmpd) tmpd = "/tmp";
#endif
    uint8_t rnd[8];
    zr_csprng_bytes(rnd, sizeof(rnd));
    char rnd_hex[17];
    for (int r = 0; r < 8; r++) snprintf(rnd_hex + r * 2, 3, "%02x", rnd[r]);
    char src_path[512], bin_path[512];
#ifdef _WIN32
    snprintf(src_path, sizeof(src_path), "%s\\pb_poly_%s%s", tmpd, rnd_hex, ext);
    snprintf(bin_path, sizeof(bin_path), "%s\\pb_poly_%s.exe", tmpd, rnd_hex);
#else
    snprintf(src_path, sizeof(src_path), "%s/pb_poly_%s%s", tmpd, rnd_hex, ext);
    snprintf(bin_path, sizeof(bin_path), "%s/pb_poly_%s", tmpd, rnd_hex);
#endif
    FILE *fp = fopen(src_path, "wb");
    if (!fp) return vm_error(vm, "polyglot_exec_compiled(): cannot write temp source");
    fwrite(AS_STR(argv[1])->chars, 1, AS_STR(argv[1])->len, fp);
    fclose(fp);

    if (mode == 1) {
        PbArray *ca = pb_array_new_raw(vm, 2);
        vm_push_root(vm, val_obj(ca));
        const char *subcmd = (strcmp(compiler, "nim") == 0) ? "r" : "run";
        pb_array_push(vm, ca, val_obj(pb_str_new(vm, subcmd, strlen(subcmd))));
        pb_array_push(vm, ca, val_obj(pb_str_new(vm, src_path, strlen(src_path))));
        PbValue cargs[2];
        cargs[0] = val_obj(pb_str_new(vm, compiler, strlen(compiler)));
        cargs[1] = val_obj(ca);
        vm_push_root(vm, cargs[0]);
        PbValue rm = val_nil();
        bool rok = bi_proc_spawn_wait(vm, 2, cargs, &rm);
        vm_pop_roots(vm, 2);
        remove(src_path);
        if (!rok) return false;
        vm_push_root(vm, rm);
        bool fok = zr_polyglot_format_result(vm, rm, out);
        vm_pop_roots(vm, 1);
        return fok;
    }

    PbArray *ca = pb_array_new_raw(vm, 5);
    vm_push_root(vm, val_obj(ca));
    if (mode == 2) {
        char oarg[540];
        snprintf(oarg, sizeof(oarg), "-o%s", bin_path);
        pb_array_push(vm, ca, val_obj(pb_str_new(vm, oarg, strlen(oarg))));
        pb_array_push(vm, ca, val_obj(pb_str_new(vm, src_path, strlen(src_path))));
    } else if (mode == 3) {
        pb_array_push(vm, ca, val_obj(pb_str_new(vm, "-x", 2)));
        pb_array_push(vm, ca, val_obj(pb_str_new(vm, "-o", 2)));
        pb_array_push(vm, ca, val_obj(pb_str_new(vm, bin_path, strlen(bin_path))));
        pb_array_push(vm, ca, val_obj(pb_str_new(vm, src_path, strlen(src_path))));
    } else {
        pb_array_push(vm, ca, val_obj(pb_str_new(vm, src_path, strlen(src_path))));
        pb_array_push(vm, ca, val_obj(pb_str_new(vm, "-o", 2)));
        pb_array_push(vm, ca, val_obj(pb_str_new(vm, bin_path, strlen(bin_path))));
    }
    PbValue cargs[2];
    cargs[0] = val_obj(pb_str_new(vm, compiler, strlen(compiler)));
    cargs[1] = val_obj(ca);
    vm_push_root(vm, cargs[0]);
    PbValue cm = val_nil();
    bool cok = bi_proc_spawn_wait(vm, 2, cargs, &cm);
    vm_pop_roots(vm, 2);
    remove(src_path);
    if (!cok) return false;
    PbValue cec = val_num(1.0);
    if (is_obj_type(cm, OBJ_MAP)) pb_map_get_cstr(AS_MAP(cm), "exit_code", &cec);
    if (cec.tag != VAL_NUM || (int)cec.as.num != 0) {
        vm_push_root(vm, cm);
        bool fok = zr_polyglot_format_result(vm, cm, out);
        vm_pop_roots(vm, 1);
        return fok;
    }
    PbArray *ra = pb_array_new_raw(vm, 1);
    vm_push_root(vm, val_obj(ra));
    PbValue rargs[2];
    if (runner) {
        pb_array_push(vm, ra, val_obj(pb_str_new(vm, bin_path, strlen(bin_path))));
        rargs[0] = val_obj(pb_str_new(vm, runner, strlen(runner)));
    } else {
        rargs[0] = val_obj(pb_str_new(vm, bin_path, strlen(bin_path)));
    }
    rargs[1] = val_obj(ra);
    vm_push_root(vm, rargs[0]);
    PbValue rm = val_nil();
    bool rok = bi_proc_spawn_wait(vm, 2, rargs, &rm);
    vm_pop_roots(vm, 2);
    remove(bin_path);
    if (!rok) return false;
    vm_push_root(vm, rm);
    bool fok = zr_polyglot_format_result(vm, rm, out);
    vm_pop_roots(vm, 1);
    return fok;
}

static bool bi_polyglot_compile_cached(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (!vm_check_cap(vm, vm_effective_cap(vm, CAP_EXEC | CAP_FFI), "polyglot_compile_cached", "--allow-exec --allow-ffi")) return false;
    if (argc != 2 || !is_obj_type(argv[0], OBJ_STR) || !is_obj_type(argv[1], OBJ_STR)) {
        return vm_error(vm, "polyglot_compile_cached(lang, code) expects 2 string arguments");
    }
    const char *lang = AS_STR(argv[0])->chars;
    const char *ext = ".c", *compiler = "gcc";
    if (strcmp(lang, "cpp") == 0 || strcmp(lang, "c++") == 0) { ext = ".cpp"; compiler = "g++"; }
    else if (strcmp(lang, "fortran") == 0 || strcmp(lang, "f90") == 0) { ext = ".f90"; compiler = "gfortran"; }
    else if (strcmp(lang, "c") != 0) {
        return vm_error(vm, "polyglot_compile_cached(): unsupported shared-lib language '%s'", lang);
    }
    char cdir[512];
    zr_polyglot_cache_root(cdir);
    if (!cdir[0]) return vm_error(vm, "polyglot_compile_cached(): cache dir unavailable");
    zr_mkdirs_path(cdir);
    char hex[25];
    zr_sha256_short_hex(AS_STR(argv[1])->chars, AS_STR(argv[1])->len, hex);
    char src_path[768], so_path[768];
#ifdef _WIN32
    snprintf(src_path, sizeof(src_path), "%s\\poly_%s%s", cdir, hex, ext);
    snprintf(so_path, sizeof(so_path), "%s\\poly_%s.dll", cdir, hex);
#else
    snprintf(src_path, sizeof(src_path), "%s/poly_%s%s", cdir, hex, ext);
    snprintf(so_path, sizeof(so_path), "%s/poly_%s.so", cdir, hex);
#endif
    if (!zr_file_exists_reg(so_path)) {
        FILE *fp = fopen(src_path, "wb");
        if (!fp) return vm_error(vm, "polyglot_compile_cached(): cannot write source");
        fwrite(AS_STR(argv[1])->chars, 1, AS_STR(argv[1])->len, fp);
        fclose(fp);
        PbArray *ca = pb_array_new_raw(vm, 6);
        vm_push_root(vm, val_obj(ca));
        pb_array_push(vm, ca, val_obj(pb_str_new(vm, "-shared", 7)));
#ifndef _WIN32
        pb_array_push(vm, ca, val_obj(pb_str_new(vm, "-fPIC", 5)));
#endif
        pb_array_push(vm, ca, val_obj(pb_str_new(vm, "-O2", 3)));
        pb_array_push(vm, ca, val_obj(pb_str_new(vm, src_path, strlen(src_path))));
        pb_array_push(vm, ca, val_obj(pb_str_new(vm, "-o", 2)));
        pb_array_push(vm, ca, val_obj(pb_str_new(vm, so_path, strlen(so_path))));
        PbValue cargs[2];
        cargs[0] = val_obj(pb_str_new(vm, compiler, strlen(compiler)));
        cargs[1] = val_obj(ca);
        vm_push_root(vm, cargs[0]);
        PbValue cm = val_nil();
        bool cok = bi_proc_spawn_wait(vm, 2, cargs, &cm);
        vm_pop_roots(vm, 2);
        remove(src_path);
        if (!cok) return false;
    }
    PbValue farg = val_obj(pb_str_new(vm, so_path, strlen(so_path)));
    vm_push_root(vm, farg);
    bool lok = bi_ffi_load(vm, 1, &farg, out);
    vm_pop_roots(vm, 1);
    return lok;
}

static bool bi_polyglot_cache_info(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    (void)argc; (void)argv;
    char cdir[512];
    zr_polyglot_cache_root(cdir);
    PbMap *m = pb_map_new_raw(vm, 4);
    vm_push_root(vm, val_obj(m));
    pb_map_set_cstr(vm, m, "dir", val_obj(pb_str_new(vm, cdir, strlen(cdir))));
    pb_map_set_cstr(vm, m, "enabled", val_bool(cdir[0] != '\0'));
    PbValue mv = val_obj(m);
    bool ok = bi_json_stringify(vm, 1, &mv, out);
    vm_pop_roots(vm, 1);
    return ok;
}

static bool bi_polyglot_cache_clear(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    (void)vm; (void)argc; (void)argv;
    *out = val_num(0.0);
    return true;
}

/* ============================================================================
 * 9. Standard Console I/O & Master Builtin Registration (`zr_register_all_builtins`)
 * ============================================================================ */

static bool bi_print(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    for (int i = 0; i < argc; i++) {
        if (!zr_check_taint_sink(vm, argv[i], "print")) return false;
    }
    for (int i = 0; i < argc; i++) {
        if (i > 0) fputc(' ', stdout);
        size_t slen = 0;
        char *s = vm_value_to_cstr(vm, argv[i], &slen);
        if (s && slen > 0) fwrite(s, 1, slen, stdout);
        free(s);
    }
    fputc('\n', stdout);
    fflush(stdout);
    *out = val_nil();
    return true;
}

static bool bi_eprint(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    for (int i = 0; i < argc; i++) {
        if (!zr_check_taint_sink(vm, argv[i], "eprint")) return false;
    }
    for (int i = 0; i < argc; i++) {
        if (i > 0) fputc(' ', stderr);
        size_t slen = 0;
        char *s = vm_value_to_cstr(vm, argv[i], &slen);
        if (s && slen > 0) fwrite(s, 1, slen, stderr);
        free(s);
    }
    fputc('\n', stderr);
    fflush(stderr);
    *out = val_nil();
    return true;
}

static void zr_register_all_builtins(PbVM *vm) {
    static const struct { const char *name; PbBuiltinFn fn; } table[] = {
        /* Console & Core */
        {"print", bi_print},
        {"println", bi_print},
        {"eprint", bi_eprint},
        {"eprintln", bi_eprint},
        {"import", bi_import},
        {"import_with_caps", bi_import_with_caps},
        {"assert", bi_assert},
        {"error", bi_error},

        /* Type Predicates, Freeze, GC, JIT & Profiler */
        {"type", bi_type},
        {"type_assert", bi_type_assert},
        {"require_type", bi_type_assert},
        {"freeze", bi_freeze},
        {"is_frozen", bi_is_frozen},
        {"is_nil", bi_is_nil},
        {"is_bool", bi_is_bool},
        {"is_number", bi_is_number},
        {"is_string", bi_is_string},
        {"is_array", bi_is_array},
        {"is_map", bi_is_map},
        {"is_fn", bi_is_fn},
        {"is_bytes", bi_is_bytes},
        {"is_f64buf", bi_is_f64buf},
        {"gc_collect", bi_gc_collect},
        {"gc_stats", bi_gc_stats},
        {"gc_stats_map", bi_gc_stats_map},
        {"jit_compile", bi_jit_compile},
        {"jit_stats", bi_jit_stats},
        {"vm_set_prof", bi_vm_set_prof},
        {"vm_prof_stats", bi_vm_prof_stats},

        /* Zero-Copy Shared Buffers (`OBJ_BYTES` & `OBJ_F64BUF`) */
        {"bytes_new", bi_bytes_new},
        {"bytes_from_str", bi_bytes_from_str},
        {"bytes_get", bi_bytes_get},
        {"bytes_set", bi_bytes_set},
        {"bytes_slice", bi_bytes_slice},
        {"f64buf_new", bi_f64buf_new},
        {"f64buf_get", bi_f64buf_get},
        {"f64buf_set", bi_f64buf_set},
        {"f64buf_sum", bi_f64buf_sum},
        {"f64buf_dot", bi_f64buf_dot},

        /* VM-Level Taint Tracking & Deterministic Flight-Recorder */
        {"taint_secret", bi_taint_secret},
        {"taint_pii", bi_taint_pii},
        {"taint_get", bi_taint_get},
        {"is_tainted", bi_is_tainted},
        {"sanitize_taint", bi_sanitize_taint},
        {"redact_secret", bi_redact_secret},
        {"vm_set_taint_guard", bi_vm_set_taint_guard},
        {"flight_record_start", bi_flight_record_start},
        {"flight_record_stop", bi_flight_record_stop},
        {"flight_replay_start", bi_flight_replay_start},
        {"flight_replay_stop", bi_flight_replay_stop},

        /* Math, BigNum, Checked Arithmetic, Bitwise & LE Bytes */
        {"bigint", bi_bigint},
        {"rational", bi_rational},
        {"decimal", bi_decimal},
        {"num_abs", bi_num_abs},
        {"abs", bi_num_abs},
        {"num_floor", bi_num_floor},
        {"floor", bi_num_floor},
        {"num_ceil", bi_num_ceil},
        {"ceil", bi_num_ceil},
        {"num_round", bi_num_round},
        {"round", bi_num_round},
        {"num_sqrt", bi_num_sqrt},
        {"sqrt", bi_num_sqrt},
        {"num_ln", bi_num_ln},
        {"num_log2", bi_num_log2},
        {"num_log10", bi_num_log10},
        {"num_exp", bi_num_exp},
        {"num_sin", bi_num_sin},
        {"sin", bi_num_sin},
        {"num_cos", bi_num_cos},
        {"cos", bi_num_cos},
        {"num_tan", bi_num_tan},
        {"tan", bi_num_tan},
        {"num_pow", bi_num_pow},
        {"pow", bi_num_pow},
        {"num_min", bi_num_min},
        {"min", bi_num_min},
        {"num_max", bi_num_max},
        {"max", bi_num_max},
        {"num_atan2", bi_num_atan2},
        {"atan2", bi_num_atan2},
        {"num_mod", bi_num_mod},
        {"clamp", bi_clamp},
        {"sum", bi_sum},
        {"product", bi_product},
        {"min_of", bi_min_of},
        {"max_of", bi_max_of},
        {"checked_add", bi_checked_add},
        {"checked_sub", bi_checked_sub},
        {"checked_mul", bi_checked_mul},
        {"checked_div", bi_checked_div},
        {"bit_and", bi_bit_and},
        {"bit_or", bi_bit_or},
        {"bit_xor", bi_bit_xor},
        {"bit_not", bi_bit_not},
        {"bit_shl", bi_bit_shl},
        {"bit_shr", bi_bit_shr},
        {"num_to_le_bytes", bi_num_to_le_bytes},
        {"num_from_le_bytes", bi_num_from_le_bytes},
        {"le_bytes_to_num", bi_num_from_le_bytes},

        /* Strings & Byte Conversions */
        {"str", bi_str},
        {"len", bi_len},
        {"slice", bi_slice},
        {"str_slice", bi_str_slice},
        {"str_contains", bi_str_contains},
        {"str_starts_with", bi_str_starts_with},
        {"str_ends_with", bi_str_ends_with},
        {"str_index_of", bi_str_index_of},
        {"str_split", bi_str_split},
        {"str_join", bi_str_join},
        {"str_trim", bi_str_trim},
        {"str_upper", bi_str_upper},
        {"str_lower", bi_str_lower},
        {"str_replace_all", bi_str_replace_all},
        {"str_repeat", bi_str_repeat},
        {"str_pad_left", bi_str_pad_left},
        {"str_pad_right", bi_str_pad_right},
        {"str_format", bi_str_format},
        {"strfromcharcode", bi_strfromcharcode},
        {"str_from_char_code", bi_strfromcharcode},
        {"strcharcodeat", bi_strcharcodeat},
        {"str_to_bytes", bi_str_to_bytes},
        {"bytes_to_str", bi_bytes_to_str},
        {"parse_number", bi_parse_number},
        {"parse_bool", bi_parse_bool},

        /* Arrays & Maps */
        {"push", bi_array_push},
        {"array_push", bi_array_push},
        {"pop", bi_array_pop},
        {"array_pop", bi_array_pop},
        {"array_sort", bi_array_sort},
        {"array_unique", bi_array_unique},
        {"array_flatten", bi_array_flatten},
        {"array_chunk", bi_array_chunk},
        {"array_zip", bi_array_zip},
        {"array_take", bi_array_take},
        {"array_drop", bi_array_drop},
        {"range", bi_range},
        {"enumerate", bi_enumerate},
        {"map_keys", bi_map_keys},
        {"keys", bi_map_keys},
        {"map_has", bi_map_has},
        {"has", bi_map_has},
        {"has_member", bi_has_member},
        {"map_remove", bi_map_remove},

        /* JSON, Crypto & Encodings */
        {"json_parse", bi_json_parse},
        {"json_stringify", bi_json_stringify},
        {"sha1", bi_sha1},
        {"sha256", bi_sha256},
        {"sha512", bi_sha512},
        {"hmac_sha256", bi_hmac_sha256},
        {"constant_time_eq", bi_constant_time_eq},
        {"random_bytes", bi_random_bytes},
        {"random_float", bi_random_float},
        {"rand_float", bi_random_float},
        {"random", bi_random_float},
        {"csprng_hex", bi_csprng_hex},
        {"base64_encode", bi_base64_encode},
        {"base64_encode_bytes", bi_base64_encode_bytes},
        {"base64_decode", bi_base64_decode},
        {"base64_decode_bytes", bi_base64_decode_bytes},
        {"hex_encode", bi_hex_encode},
        {"hex_decode", bi_hex_decode},
        {"url_encode", bi_url_encode},
        {"url_decode", bi_url_decode},

        /* Regex */
        {"regex_match", bi_regex_match},
        {"regex_find", bi_regex_find},
        {"regex_find_all", bi_regex_find_all},
        {"regex_replace", bi_regex_replace},
        {"regex_replace_all", bi_regex_replace_all},
        {"regex_split", bi_regex_split},
        {"regex_capture", bi_regex_capture},

        /* Filesystem, Paths, OS, Time, Signals, Processes & Hardware */
        {"fs_read", bi_fs_read},
        {"fs_read_bytes", bi_fs_read_bytes},
        {"fs_write", bi_fs_write},
        {"fs_write_bytes", bi_fs_write_bytes},
        {"fs_append", bi_fs_append},
        {"fs_exists", bi_fs_exists},
        {"fs_remove", bi_fs_remove},
        {"fs_rename", bi_fs_rename},
        {"fs_copy", bi_fs_copy},
        {"fs_mkdirs", bi_fs_mkdirs},
        {"fs_listdir", bi_fs_listdir},
        {"path_join", bi_path_join},
        {"path_dirname", bi_path_dirname},
        {"path_basename", bi_path_basename},
        {"cwd", bi_cwd},
        {"env_get", bi_env_get},
        {"env_set", bi_env_set},
        {"os_args", bi_os_args},
        {"args", bi_os_args},
        {"os_exit", bi_os_exit},
        {"os_which", bi_os_which},
        {"os_signal_trap", bi_os_signal_trap},
        {"os_signal_pending", bi_os_signal_pending},
        {"os_signal_poll", bi_os_signal_poll},
        {"os_signal_handler_get", bi_os_signal_handler_get},
        {"platform", bi_platform},
        {"os_arch", bi_os_arch},
        {"clock", bi_clock},
        {"now_ms", bi_now_ms},
        {"time_ms", bi_now_ms},
        {"time_now_ms", bi_now_ms},
        {"now_iso", bi_now_iso},
        {"sleep_ms", bi_sleep_ms},
        {"sleep", bi_sleep_ms},
        {"read_line", bi_read_line},
        {"exec", bi_exec},
        {"proc_spawn_wait", bi_proc_spawn_wait},
        {"hardware_probe", bi_hardware_probe},

        /* TCP, TLS, HTTP/HTTPS, SSE & WebSocket */
        {"tcp_listen", bi_tcp_listen},
        {"tcp_connect", bi_tcp_connect},
        {"tcp_accept", bi_tcp_accept},
        {"tcp_read", bi_tcp_read},
        {"tcp_read_until", bi_tcp_read_until},
        {"tcp_write", bi_tcp_write},
        {"tcp_close", bi_tcp_close},
        {"tcp_local_addr", bi_tcp_local_addr},
        {"tcp_listen_tls", bi_tcp_listen_tls},
        {"tcp_connect_tls", bi_tcp_connect_tls},
        {"tcp_wrap_tls", bi_tcp_wrap_tls},
        {"net_poll", bi_net_poll},
        {"http_get", bi_http_get},
        {"http_post", bi_http_post},
        {"http_get_auth", bi_http_get_auth},
        {"http_post_auth", bi_http_post_auth},
        {"http_request", bi_http_request},
        {"http_parse_request", bi_http_parse_request},
        {"http_response", bi_http_response},
        {"http_response_ct", bi_http_response_ct},
        {"http_response_json", bi_http_response_json},
        {"parse_sse_line", bi_parse_sse_line},
        {"websocket_accept_key", bi_websocket_accept_key},
        {"websocket_parse_frame", bi_ws_parse_frame},
        {"ws_parse_frame", bi_ws_parse_frame},
        {"websocket_write_frame", bi_ws_write_frame},
        {"ws_write_frame", bi_ws_write_frame},

        /* SQLite, Unrestricted Dynamic FFI, Struct Packing & C Callbacks */
        {"sqlite_open", bi_sqlite_open},
        {"sqlite_close", bi_sqlite_close},
        {"sqlite_exec", bi_sqlite_exec},
        {"sqlite_query", bi_sqlite_query},
        {"sqlite_last_insert_rowid", bi_sqlite_last_insert_rowid},
        {"ffi_load", bi_ffi_load},
        {"ffi_open", bi_ffi_load},
        {"ffi_dlopen", bi_ffi_load},
        {"ffi_close", bi_ffi_close},
        {"ffi_call", bi_ffi_call},
        {"ffi_call_typed", bi_ffi_call_typed},
        {"ffi_struct_pack", bi_ffi_struct_pack},
        {"ffi_struct_unpack", bi_ffi_struct_unpack},
        {"ffi_bytes_ptr", bi_ffi_bytes_ptr},
        {"ffi_ptr_read", bi_ffi_ptr_read},
        {"ffi_ptr_write", bi_ffi_ptr_write},
        {"ffi_read_i32", bi_ffi_read_i32},
        {"ffi_read_i64", bi_ffi_read_i64},
        {"ffi_read_f64", bi_ffi_read_f64},
        {"ffi_read_cstr", bi_ffi_read_cstr},
        {"ffi_callback_new", bi_ffi_callback_new},
        {"ffi_callback_free", bi_ffi_callback_free},

        /* Native Graphics & Anti-Aliased Unicode Text */
        {"gfxcreatewindow", bi_gfxcreatewindow},
        {"gfxwindowopen", bi_gfxcreatewindow},
        {"gfxisopen", bi_gfxisopen},
        {"gfxclose", bi_gfxclose},
        {"gfxclosewindow", bi_gfxclose},
        {"gfxsize", bi_gfxsize},
        {"gfxclear", bi_gfxclear},
        {"gfxdrawpoint", bi_gfxdrawpoint},
        {"gfxdrawline", bi_gfxdrawline},
        {"gfxdrawrect", bi_gfxdrawrect},
        {"gfxdrawrects", bi_gfxdrawrects},
        {"gfxpresent", bi_gfxpresent},
        {"gfxpollevents", bi_gfxpollevents},
        {"gfxdrawtextaa", bi_gfxdrawtextaa},
        {"gfxtextprofile", bi_gfxtextprofile},
        {"gfxbackendprofile", bi_gfxbackendprofile},

        /* Threads, Channels, Atomics & Sync */
        {"spawn", bi_spawn},
        {"spawn_n", bi_spawn_n},
        {"join", bi_join},
        {"join_all", bi_join_all},
        {"gil_enabled", bi_gil_enabled},
        {"channel", bi_channel},
        {"channel_buffered", bi_channel_buffered},
        {"send", bi_send},
        {"recv", bi_recv},
        {"recv_timeout", bi_recv_timeout},
        {"channel_close", bi_channel_close},
        {"channel_is_closed", bi_channel_is_closed},
        {"channel_size", bi_channel_size},
        {"select", bi_select},
        {"select_with_timeout", bi_select_with_timeout},
        {"ticker", bi_ticker},
        {"timeout_channel", bi_timeout_channel},
        {"atomic_new", bi_atomic_new},
        {"atomic_get", bi_atomic_get},
        {"atomic_set", bi_atomic_set},
        {"atomic_add", bi_atomic_add},
        {"atomic_sub", bi_atomic_sub},
        {"atomic_swap", bi_atomic_swap},
        {"atomic_cas", bi_atomic_cas},
        {"mutex_new", bi_mutex_new},
        {"mutex_lock", bi_mutex_lock},
        {"mutex_try_lock", bi_mutex_try_lock},
        {"mutex_unlock", bi_mutex_unlock},
        {"rwlock_new", bi_rwlock_new},
        {"rwlock_read", bi_rwlock_read},
        {"rwlock_write", bi_rwlock_write},
        {"rwlock_unlock", bi_rwlock_unlock},
        {"barrier_new", bi_barrier_new},
        {"barrier_wait", bi_barrier_wait},
        {"once_new", bi_once_new},
        {"once_is_done", bi_once_is_done},

        /* Memory Arenas & Borrow-Checker */
        {"allocate", bi_memory_alloc},
        {"memory_alloc", bi_memory_alloc},
        {"deallocate", bi_memory_free},
        {"memory_free", bi_memory_free},
        {"load", bi_memory_get},
        {"memory_get", bi_memory_get},
        {"store", bi_memory_set},
        {"memory_set", bi_memory_set},
        {"borrow_immutable", bi_borrow_immutable},
        {"borrow_mutable", bi_borrow_mutable},
        {"borrow_unique", bi_borrow_mutable},
        {"memory_borrow", bi_memory_borrow},
        {"release_borrow", bi_memory_release},
        {"memory_release", bi_memory_release},
        {"push_lifetime", bi_push_lifetime},
        {"pop_lifetime", bi_pop_lifetime},
        {"memory_stats", bi_memory_stats},

        /* Tensors (25 builtins) */
        {"tensor_add", bi_tensor_add},
        {"tensor_sub", bi_tensor_sub},
        {"tensor_mul", bi_tensor_mul},
        {"tensor_scale", bi_tensor_scale},
        {"tensor_matmul", bi_tensor_matmul},
        {"tensor_dot", bi_tensor_dot},
        {"tensor_transpose2d", bi_tensor_transpose2d},
        {"tensor_relu", bi_tensor_relu},
        {"tensor_leaky_relu", bi_tensor_leaky_relu},
        {"tensor_sigmoid", bi_tensor_sigmoid},
        {"tensor_tanh", bi_tensor_tanh},
        {"tensor_gelu", bi_tensor_gelu},
        {"tensor_softmax", bi_tensor_softmax},
        {"tensor_log_softmax", bi_tensor_log_softmax},
        {"tensor_sum", bi_tensor_sum},
        {"tensor_mean", bi_tensor_mean},
        {"tensor_argmax", bi_tensor_argmax},
        {"tensor_layer_norm", bi_tensor_layer_norm},
        {"tensor_batch_norm", bi_tensor_batch_norm},
        {"tensor_embedding_lookup", bi_tensor_embedding_lookup},
        {"tensor_cross_entropy", bi_tensor_cross_entropy},
        {"tensor_conv2d", bi_tensor_conv2d},
        {"tensor_maxpool2d", bi_tensor_maxpool2d},
        {"tensor_avgpool2d", bi_tensor_avgpool2d},
        {"tensor_backend", bi_tensor_backend},

        /* Quantum, AI & Polyglot */
        {"quantum_simulate", bi_quantum_simulate},
        {"quantum_backends", bi_quantum_backends},
        {"quantum_statevector", bi_quantum_statevector},
        {"quantum_run", bi_quantum_run},
        {"ai_token_count", bi_ai_token_count},
        {"ai_chat", bi_ai_chat},
        {"ai_embed", bi_ai_embed},
        {"onnx_infer", bi_onnx_infer},
        {"polyglot_detect", bi_polyglot_detect},
        {"polyglot_exec", bi_polyglot_exec},
        {"polyglot_exec_with_input", bi_polyglot_exec_with_input},
        {"polyglot_exec_compiled", bi_polyglot_exec_compiled},
        {"polyglot_compile_cached", bi_polyglot_compile_cached},
        {"polyglot_cache_info", bi_polyglot_cache_info},
        {"polyglot_cache_clear", bi_polyglot_cache_clear},

        /* Native C99 SIMD + Multi-Core + OpenCL GPU HPC Engine (8 builtins) */
        {"hpc_probe", bi_hpc_probe},
        {"hpc_gemm", bi_hpc_gemm},
        {"hpc_csr_spmv", bi_hpc_csr_spmv},
        {"hpc_csr_cg_solve", bi_hpc_csr_cg_solve},
        {"hpc_cg_poisson_solve", bi_hpc_cg_poisson_solve},
        {"hpc_fft_1d", bi_hpc_fft_1d},
        {"hpc_mlp_train_batch", bi_hpc_mlp_train_batch},
        {"hpc_mlp_forward", bi_hpc_mlp_forward}
    };

    for (size_t i = 0; i < sizeof(table) / sizeof(table[0]); i++) {
        vm_register_builtin(vm, table[i].name, table[i].fn);
    }

    /* Pre-bind `json` namespace map (`json.parse`, `json.stringify`) */
    PbMap *jm = pb_map_new_raw(vm, 4);
    vm_push_root(vm, val_obj(jm));
    PbValue jparse = val_nil(), jstr = val_nil();
    if (pb_map_get_cstr(vm->builtins, "json_parse", &jparse)) {
        pb_map_set_cstr(vm, jm, "parse", jparse);
    }
    if (pb_map_get_cstr(vm->builtins, "json_stringify", &jstr)) {
        pb_map_set_cstr(vm, jm, "stringify", jstr);
    }
    pb_map_set_cstr(vm, vm->builtins, "json", val_obj(jm));
    vm_pop_roots(vm, 1);
}

#endif /* PBLANG_ZR_BUILTINS_CORE_H */
