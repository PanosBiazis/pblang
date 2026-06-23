/**
 * pblang_rt.h — pblang C Runtime
 *
 * Portable C99 runtime for programs transpiled from pblang (.sapp) source.
 * Compile with: gcc -O2 -o output output.c -lm
 *
 * Memory model: reference-counted (like pblang's Rust VM).
 * Values are tagged unions (PbVal) — same semantics as the bytecode VM.
 */

#ifndef PBLANG_RT_H
#define PBLANG_RT_H

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <math.h>
#include <stdint.h>
#include <stdbool.h>
#include <stdarg.h>
#include <setjmp.h>
#include <time.h>
#include <ctype.h>

/* ─── Value type tags ──────────────────────────────────────────────── */
#define PB_TAG_NIL    0
#define PB_TAG_BOOL   1
#define PB_TAG_NUM    2
#define PB_TAG_STR    3
#define PB_TAG_ARR    4
#define PB_TAG_MAP    5
#define PB_TAG_FN     6
#define PB_TAG_ERR    7

/* ─── Forward declarations ─────────────────────────────────────────── */
typedef struct PbVal PbVal;
typedef struct PbArr PbArr;
typedef struct PbMap PbMap;
typedef struct PbClosure PbClosure;
/* Uniform closure ABI: (captured env, arg array, arg count). Top-level
   functions get a wrapper of this shape (empty env); fn-literals carry the
   variables they capture in env. */
typedef PbVal (*PbFnPtr)(PbVal*, PbVal*, size_t);

/* ─── Value struct ─────────────────────────────────────────────────── */
struct PbVal {
    uint8_t tag;
    union {
        double      num;
        bool        bval;
        char*       str;    /* heap-allocated, null-terminated, shared */
        PbArr*      arr;    /* reference-counted array */
        PbMap*      map;    /* reference-counted map */
        PbClosure*  clo;    /* function / closure */
    };
};

struct PbClosure {
    PbFnPtr fn;
    size_t  arity;
    size_t  env_len;
    PbVal*  env;        /* captured values, by value */
};

struct PbArr {
    size_t  refcount;
    size_t  len;
    size_t  cap;
    PbVal*  data;
};

typedef struct { char* key; PbVal val; } PbMapEntry;
struct PbMap {
    size_t      refcount;
    size_t      len;
    size_t      cap;
    PbMapEntry* entries;
};

/* ─── Constructor macros ──────────────────────────────────────────── */
static inline PbVal pb_nil(void)             { PbVal v; v.tag=PB_TAG_NIL; v.num=0; return v; }
static inline PbVal pb_bool(bool b)          { PbVal v; v.tag=PB_TAG_BOOL; v.bval=b; return v; }
static inline PbVal pb_num(double n)         { PbVal v; v.tag=PB_TAG_NUM; v.num=n; return v; }

#define PB_NIL   (pb_nil())
#define PB_TRUE  (pb_bool(true))
#define PB_FALSE (pb_bool(false))
#define PB_NUM(n)(pb_num(n))
#define PB_STR(s)(pb_str_lit(s))

/* ─── String allocation ───────────────────────────────────────────── */
static char* pb_strdup(const char* s) {
    size_t n = strlen(s) + 1;
    char* d = (char*)malloc(n);
    memcpy(d, s, n);
    return d;
}

static inline PbVal pb_str_lit(const char* s) {
    PbVal v; v.tag = PB_TAG_STR;
    v.str = pb_strdup(s);
    return v;
}

/* ─── Closures / first-class functions ────────────────────────────── */
static PbVal pb_make_closure(PbFnPtr fn, size_t arity, PbVal* env, size_t env_len) {
    PbClosure* c = (PbClosure*)malloc(sizeof(PbClosure));
    c->fn = fn; c->arity = arity; c->env_len = env_len;
    if (env_len > 0) {
        c->env = (PbVal*)malloc(sizeof(PbVal) * env_len);
        for (size_t i = 0; i < env_len; i++) { c->env[i] = env[i]; }
    } else {
        c->env = NULL;
    }
    PbVal v; v.tag = PB_TAG_FN; v.clo = c; return v;
}

/* Wrap a statically-allocated closure as a value — no heap allocation. Used for
   captureless function values (top-level functions and capture-free literals). */
static inline PbVal pb_fn_as_val(PbClosure* c) {
    PbVal v; v.tag = PB_TAG_FN; v.clo = c; return v;
}

/* Indirect call: invoke any function value with an argument array. */
static PbVal pb_call(PbVal callee, PbVal* args, size_t n) {
    if (callee.tag != PB_TAG_FN || callee.clo == NULL) { return PB_NIL; }
    return callee.clo->fn(callee.clo->env, args, n);
}

/* ─── Array operations ────────────────────────────────────────────── */
static PbArr* pb_arr_alloc(size_t cap) {
    PbArr* a = (PbArr*)calloc(1, sizeof(PbArr));
    a->refcount = 1;
    a->cap = cap > 0 ? cap : 4;
    a->data = (PbVal*)calloc(a->cap, sizeof(PbVal));
    return a;
}

static inline PbVal pb_arr_new(size_t cap) {
    PbVal v; v.tag = PB_TAG_ARR; v.arr = pb_arr_alloc(cap); return v;
}

static PbVal pb_arr_lit(size_t n, ...) {
    PbArr* a = pb_arr_alloc(n);
    a->len = n;
    va_list ap; va_start(ap, n);
    for (size_t i = 0; i < n; i++) a->data[i] = va_arg(ap, PbVal);
    va_end(ap);
    PbVal v; v.tag = PB_TAG_ARR; v.arr = a; return v;
}

static PbVal pb_arr_get(PbVal arr, PbVal idx) {
    if (arr.tag != PB_TAG_ARR) return PB_NIL;
    size_t i = (size_t)(size_t)idx.num;
    if (i >= arr.arr->len) { fprintf(stderr, "pblang: index %zu out of range (len=%zu)\n", i, arr.arr->len); exit(1); }
    return arr.arr->data[i];
}

static void pb_arr_set(PbVal arr, PbVal idx, PbVal val) {
    if (arr.tag != PB_TAG_ARR) return;
    size_t i = (size_t)idx.num;
    if (i >= arr.arr->len) return;
    arr.arr->data[i] = val;
}

static void pb_arr_push(PbVal arr, PbVal val) {
    if (arr.tag != PB_TAG_ARR) return;
    PbArr* a = arr.arr;
    if (a->len >= a->cap) {
        a->cap = a->cap * 2 + 4;
        a->data = (PbVal*)realloc(a->data, a->cap * sizeof(PbVal));
    }
    a->data[a->len++] = val;
}

static size_t pb_arr_len(PbVal arr) {
    if (arr.tag != PB_TAG_ARR) return 0;
    return arr.arr->len;
}

/* ─── Map operations ──────────────────────────────────────────────── */
static PbMap* pb_map_alloc(void) {
    PbMap* m = (PbMap*)calloc(1, sizeof(PbMap));
    m->refcount = 1; m->cap = 8;
    m->entries = (PbMapEntry*)calloc(m->cap, sizeof(PbMapEntry));
    return m;
}

static inline PbVal pb_map_new(void) {
    PbVal v; v.tag = PB_TAG_MAP; v.map = pb_map_alloc(); return v;
}

static PbVal pb_map_lit(size_t n, ...) {
    PbMap* m = pb_map_alloc();
    va_list ap; va_start(ap, n);
    for (size_t i = 0; i < n; i++) {
        PbVal k = va_arg(ap, PbVal);
        PbVal v = va_arg(ap, PbVal);
        if (m->len >= m->cap) {
            m->cap *= 2;
            m->entries = (PbMapEntry*)realloc(m->entries, m->cap * sizeof(PbMapEntry));
        }
        m->entries[m->len].key = pb_strdup(k.str);
        m->entries[m->len].val = v;
        m->len++;
    }
    va_end(ap);
    PbVal v; v.tag = PB_TAG_MAP; v.map = m; return v;
}

static PbVal pb_map_get(PbVal map, PbVal key) {
    if (map.tag != PB_TAG_MAP || key.tag != PB_TAG_STR) return PB_NIL;
    for (size_t i = 0; i < map.map->len; i++) {
        if (strcmp(map.map->entries[i].key, key.str) == 0)
            return map.map->entries[i].val;
    }
    return PB_NIL;
}

static void pb_map_set(PbVal map, PbVal key, PbVal val) {
    if (map.tag != PB_TAG_MAP || key.tag != PB_TAG_STR) return;
    PbMap* m = map.map;
    for (size_t i = 0; i < m->len; i++) {
        if (strcmp(m->entries[i].key, key.str) == 0) { m->entries[i].val = val; return; }
    }
    if (m->len >= m->cap) {
        m->cap *= 2;
        m->entries = (PbMapEntry*)realloc(m->entries, m->cap * sizeof(PbMapEntry));
    }
    m->entries[m->len].key = pb_strdup(key.str);
    m->entries[m->len].val = val;
    m->len++;
}

static PbVal pb_map_keys(PbVal map) {
    if (map.tag != PB_TAG_MAP) return pb_arr_new(0);
    PbArr* a = pb_arr_alloc(map.map->len);
    a->len = map.map->len;
    for (size_t i = 0; i < map.map->len; i++) a->data[i] = pb_str_lit(map.map->entries[i].key);
    PbVal v; v.tag = PB_TAG_ARR; v.arr = a; return v;
}

/* ─── String operations ───────────────────────────────────────────── */
/* Shortest decimal that round-trips back to the same double — matches the VM's
   (Rust) float Display: 1.0/3.0 -> "0.3333333333333333" not "0.333333", and
   plain decimal (never exponential), so 0.000001 -> "0.000001" not "1e-06". */
static void pb_fmt_double(char* buf, size_t bufsz, double n) {
    int prec = 1;
    while (prec <= 17) {
        snprintf(buf, bufsz, "%.*g", prec, n);
        if (strtod(buf, NULL) == n) break;
        prec++;
    }
    if (prec > 17) { snprintf(buf, bufsz, "%.17g", n); return; }
    /* %g may pick exponential form (e.g. 1e-06); Rust's Display never does.
       Reformat in plain decimal with the fewest fractional digits that round-trip. */
    if (strchr(buf, 'e') || strchr(buf, 'E')) {
        char tmp[64];
        for (int f = 0; f <= 30; f++) {
            snprintf(tmp, sizeof(tmp), "%.*f", f, n);
            if (strtod(tmp, NULL) == n) {
                char* dot = strchr(tmp, '.');
                if (dot) {
                    char* end = tmp + strlen(tmp) - 1;
                    while (end > dot && *end == '0') { *end-- = 0; }
                    if (end == dot) { *end = 0; }
                }
                snprintf(buf, bufsz, "%s", tmp);
                return;
            }
        }
    }
}

static PbVal pb_to_str(PbVal v) {
    char buf[64];
    switch (v.tag) {
        case PB_TAG_NIL:  return PB_STR("nil");
        case PB_TAG_BOOL: return PB_STR(v.bval ? "true" : "false");
        case PB_TAG_STR:  return v;
        case PB_TAG_NUM:
            if (v.num == (double)(long long)v.num && !isinf(v.num) && !isnan(v.num))
                snprintf(buf, sizeof(buf), "%lld", (long long)v.num);
            else
                pb_fmt_double(buf, sizeof(buf), v.num);
            return pb_str_lit(buf);
        case PB_TAG_ARR:  return PB_STR("[array]");
        case PB_TAG_MAP:  return PB_STR("[map]");
        case PB_TAG_FN:   return PB_STR("[fn]");
        case PB_TAG_ERR:  { char eb[256]; snprintf(eb,256,"[error: %s]",v.str); return pb_str_lit(eb); }
        default:          return PB_STR("[unknown]");
    }
}

static PbVal pb_str_concat(PbVal a, PbVal b) {
    PbVal as = pb_to_str(a); PbVal bs = pb_to_str(b);
    size_t la = strlen(as.str), lb = strlen(bs.str);
    char* s = (char*)malloc(la + lb + 1);
    memcpy(s, as.str, la); memcpy(s + la, bs.str, lb); s[la+lb] = 0;
    PbVal v; v.tag = PB_TAG_STR; v.str = s; return v;
}

static PbVal pb_str_concat_n(size_t n, ...) {
    va_list ap; va_start(ap, n);
    PbVal result = PB_STR("");
    for (size_t i = 0; i < n; i++) {
        PbVal x = va_arg(ap, PbVal);
        result = pb_str_concat(result, x);
    }
    va_end(ap); return result;
}

/* ─── Arithmetic ──────────────────────────────────────────────────── */
static PbVal pb_add(PbVal a, PbVal b) {
    if (a.tag == PB_TAG_NUM && b.tag == PB_TAG_NUM) return PB_NUM(a.num + b.num);
    if (a.tag == PB_TAG_STR || b.tag == PB_TAG_STR) return pb_str_concat(a, b);
    fprintf(stderr, "pblang: type error in +\n"); exit(1);
}
static PbVal pb_sub(PbVal a, PbVal b) { return PB_NUM(a.num - b.num); }
static PbVal pb_mul(PbVal a, PbVal b) { return PB_NUM(a.num * b.num); }
static PbVal pb_div(PbVal a, PbVal b) { if (b.num == 0.0) return PB_NUM(1.0/0.0); return PB_NUM(a.num / b.num); }
static PbVal pb_mod(PbVal a, PbVal b) { return PB_NUM(fmod(a.num, b.num)); }
static PbVal pb_pow(PbVal a, PbVal b) { return PB_NUM(pow(a.num, b.num)); }

/* ─── Comparisons ────────────────────────────────────────────────── */
static bool pb_vals_equal(PbVal a, PbVal b) {
    if (a.tag != b.tag) return false;
    switch (a.tag) {
        case PB_TAG_NIL:  return true;
        case PB_TAG_BOOL: return a.bval == b.bval;
        case PB_TAG_NUM:  return a.num == b.num;
        case PB_TAG_STR:  return strcmp(a.str, b.str) == 0;
        default:          return a.arr == b.arr; /* pointer equality for composite types */
    }
}
static PbVal pb_eq(PbVal a, PbVal b)  { return pb_bool(pb_vals_equal(a, b)); }
static PbVal pb_lt(PbVal a, PbVal b)  { return pb_bool(a.num < b.num); }
static PbVal pb_le(PbVal a, PbVal b)  { return pb_bool(a.num <= b.num); }
static PbVal pb_gt(PbVal a, PbVal b)  { return pb_bool(a.num > b.num); }
static PbVal pb_ge(PbVal a, PbVal b)  { return pb_bool(a.num >= b.num); }

/* ─── Logical ─────────────────────────────────────────────────────── */
static bool pb_truthy(PbVal v) {
    switch (v.tag) {
        case PB_TAG_NIL:  return false;
        case PB_TAG_BOOL: return v.bval;
        case PB_TAG_NUM:  return v.num != 0.0 && !isnan(v.num);
        case PB_TAG_STR:  return v.str != NULL && v.str[0] != '\0';
        case PB_TAG_ARR:  return v.arr && v.arr->len > 0;
        case PB_TAG_MAP:  return v.map && v.map->len > 0;
        default:          return true;
    }
}
static PbVal pb_not(PbVal v)            { return pb_bool(!pb_truthy(v)); }
static PbVal pb_and(PbVal a, PbVal b)   { return pb_truthy(a) ? b : a; }
static PbVal pb_or(PbVal a, PbVal b)    { return pb_truthy(a) ? a : b; }
static PbVal pb_coalesce(PbVal a, PbVal b) { return (a.tag == PB_TAG_NIL) ? b : a; }

/* ─── Nil / type checks ──────────────────────────────────────────── */
static inline bool pb_is_nil(PbVal v) { return v.tag == PB_TAG_NIL; }

/* ─── Generic index ──────────────────────────────────────────────── */
static PbVal pb_index(PbVal obj, PbVal idx) {
    if (obj.tag == PB_TAG_ARR) return pb_arr_get(obj, idx);
    if (obj.tag == PB_TAG_MAP) return pb_map_get(obj, idx);
    if (obj.tag == PB_TAG_STR) {
        /* codepoint-indexed: s[i] returns the i-th UTF-8 codepoint as a string
           (matches the VM, and is consistent with len() counting codepoints). */
        if (idx.tag != PB_TAG_NUM || idx.num < 0) return PB_NIL;
        long want = (long)idx.num;
        const unsigned char* p = (const unsigned char*)obj.str;
        long cp = 0;
        while (*p) {
            if ((*p & 0xC0) != 0x80) {            /* lead byte of a codepoint */
                if (cp == want) {
                    int n = 1;
                    if      ((*p & 0x80) == 0x00) n = 1;
                    else if ((*p & 0xE0) == 0xC0) n = 2;
                    else if ((*p & 0xF0) == 0xE0) n = 3;
                    else if ((*p & 0xF8) == 0xF0) n = 4;
                    char* out = (char*)malloc(n + 1);
                    memcpy(out, p, n);
                    out[n] = 0;
                    PbVal v; v.tag = PB_TAG_STR; v.str = out; return v;
                }
                cp++;
            }
            p++;
        }
        return PB_NIL;                            /* out of range */
    }
    return PB_NIL;
}
static PbVal pb_safe_index(PbVal obj, PbVal idx) {
    if (obj.tag == PB_TAG_NIL) return PB_NIL;
    return pb_index(obj, idx);
}

/* Polymorphic indexed assignment obj[idx] = val: array slot or map key. */
static void pb_set_index(PbVal obj, PbVal idx, PbVal val) {
    if (obj.tag == PB_TAG_ARR) pb_arr_set(obj, idx, val);
    else if (obj.tag == PB_TAG_MAP) pb_map_set(obj, idx, val);
}
static PbVal pb_safe_map_get(PbVal map, PbVal key) {
    if (map.tag == PB_TAG_NIL) return PB_NIL;
    return pb_map_get(map, key);
}

/* ─── Error handling (longjmp-based) ─────────────────────────────── */
static jmp_buf* pb__catch_buf = NULL;
static PbVal    pb__error_val;

static PbVal pb_try(PbVal try_val, PbVal catch_val) {
    /* Eager evaluation: both operands are already computed. If the try
       expression produced an error VALUE (e.g. error("x")), yield the catch
       fallback — matching the VM. (A try-expr that THROWS/longjmps before this
       point is a separate, deeper limitation.) */
    if (try_val.tag == PB_TAG_ERR) return catch_val;
    return try_val;
}

static void pb_throw(PbVal v) {
    pb__error_val = v;
    if (pb__catch_buf) longjmp(*pb__catch_buf, 1);
    else {
        PbVal s = pb_to_str(v);
        fprintf(stderr, "pblang: uncaught error: %s\n", s.str);
        exit(1);
    }
}

static PbVal pb_result_unwrap(PbVal v) { return v; }

/* ─── Builtins ───────────────────────────────────────────────────── */
static PbVal pb_builtin_len(PbVal v) {
    if (v.tag == PB_TAG_ARR) return PB_NUM((double)v.arr->len);
    if (v.tag == PB_TAG_STR) {
        /* count UTF-8 codepoints (not bytes) to match the VM: a continuation
           byte is 0b10xxxxxx, so count every byte that isn't one. */
        size_t cnt = 0;
        for (const unsigned char* p = (const unsigned char*)v.str; *p; p++)
            if ((*p & 0xC0) != 0x80) cnt++;
        return PB_NUM((double)cnt);
    }
    if (v.tag == PB_TAG_MAP) return PB_NUM((double)v.map->len);
    return PB_NUM(0.0);
}
static PbVal pb_builtin_str(PbVal v) { return pb_to_str(v); }
static PbVal pb_builtin_type(PbVal v) {
    switch (v.tag) {
        case PB_TAG_NIL:  return PB_STR("nil");
        case PB_TAG_BOOL: return PB_STR("bool");
        case PB_TAG_NUM:  return PB_STR("number");
        case PB_TAG_STR:  return PB_STR("string");
        case PB_TAG_ARR:  return PB_STR("array");
        case PB_TAG_MAP:  return PB_STR("map");
        case PB_TAG_FN:   return PB_STR("function");
        default:          return PB_STR("unknown");
    }
}
static void pb_print(PbVal v) {
    PbVal s = pb_to_str(v);
    printf("%s\n", s.str);
}
static PbVal pb_builtin_array_push(PbVal arr, PbVal val) {
    pb_arr_push(arr, val); return PB_NUM((double)pb_arr_len(arr));
}
static PbVal pb_builtin_array_pop(PbVal arr) {
    if (arr.tag != PB_TAG_ARR || arr.arr->len == 0) return PB_NIL;
    PbVal v = arr.arr->data[--arr.arr->len];
    return v;
}
static PbVal pb_builtin_map_keys(PbVal map) { return pb_map_keys(map); }
static PbVal pb_builtin_clock(void)    { return PB_NUM((double)clock() / CLOCKS_PER_SEC); }
static PbVal pb_builtin_now_ms(void)   { return PB_NUM((double)clock() * 1000.0 / CLOCKS_PER_SEC); }
static PbVal pb_builtin_sleep_ms(PbVal ms) {
    /* Best effort: many platforms have nanosleep; fall back to a busy-wait for portability */
    volatile double end = (double)clock() / CLOCKS_PER_SEC + ms.num / 1000.0;
    while ((double)clock() / CLOCKS_PER_SEC < end) {}
    return PB_NIL;
}
static PbVal pb_builtin_parse_number(PbVal s) {
    if (s.tag != PB_TAG_STR) return PB_NIL;
    char* end; double d = strtod(s.str, &end);
    if (end == s.str) return PB_NIL;
    return PB_NUM(d);
}
static PbVal pb_builtin_num_sqrt(PbVal v) { return PB_NUM(sqrt(v.num)); }
static PbVal pb_builtin_num_floor(PbVal v){ return PB_NUM(floor(v.num)); }
static PbVal pb_builtin_num_ceil(PbVal v) { return PB_NUM(ceil(v.num)); }
static PbVal pb_builtin_num_abs(PbVal v)  { return PB_NUM(fabs(v.num)); }
static PbVal pb_builtin_num_pow(PbVal a, PbVal b) { return PB_NUM(pow(a.num, b.num)); }
static PbVal pb_builtin_str_contains(PbVal s, PbVal needle) {
    if (s.tag!=PB_TAG_STR || needle.tag!=PB_TAG_STR) return PB_FALSE;
    return pb_bool(strstr(s.str, needle.str) != NULL);
}
static PbVal pb_builtin_map_has(PbVal m, PbVal key) {
    if (m.tag != PB_TAG_MAP || key.tag != PB_TAG_STR) return PB_FALSE;
    for (size_t i = 0; i < m.map->len; i++) {
        if (strcmp(m.map->entries[i].key, key.str) == 0) return PB_TRUE;
    }
    return PB_FALSE;
}
static PbVal pb_builtin_str_index_of(PbVal s, PbVal needle) {
    if (s.tag != PB_TAG_STR || needle.tag != PB_TAG_STR) return PB_NUM(-1.0);
    char* found = strstr(s.str, needle.str);
    if (found == NULL) return PB_NUM(-1.0);
    return PB_NUM((double)(found - s.str));
}
static PbVal pb_builtin_num_min(PbVal a, PbVal b) {
    if (a.tag != PB_TAG_NUM || b.tag != PB_TAG_NUM) return PB_NIL;
    return PB_NUM(a.num < b.num ? a.num : b.num);
}
static PbVal pb_builtin_num_max(PbVal a, PbVal b) {
    if (a.tag != PB_TAG_NUM || b.tag != PB_TAG_NUM) return PB_NIL;
    return PB_NUM(a.num > b.num ? a.num : b.num);
}
/* num_mod(a, b) → a % b (IEEE remainder via fmod), matching the VM. */
static PbVal pb_builtin_num_mod(PbVal a, PbVal b) {
    if (a.tag != PB_TAG_NUM || b.tag != PB_TAG_NUM) return PB_NIL;
    return PB_NUM(fmod(a.num, b.num));
}
/* clamp(v, lo, hi) → v.max(lo).min(hi), matching the VM's fold order. */
static PbVal pb_builtin_clamp(PbVal v, PbVal lo, PbVal hi) {
    if (v.tag != PB_TAG_NUM || lo.tag != PB_TAG_NUM || hi.tag != PB_TAG_NUM) return PB_NIL;
    double x = v.num;
    if (x < lo.num) x = lo.num;
    if (x > hi.num) x = hi.num;
    return PB_NUM(x);
}
/* sum(arr) → sum of numeric elements (non-numbers skipped); empty → 0. */
static PbVal pb_builtin_sum(PbVal arr) {
    if (arr.tag != PB_TAG_ARR) return PB_NUM(0.0);
    double s = 0.0;
    for (size_t i = 0; i < arr.arr->len; i++)
        if (arr.arr->data[i].tag == PB_TAG_NUM) s += arr.arr->data[i].num;
    return PB_NUM(s);
}
/* product(arr) → product of numeric elements; empty → 1. */
static PbVal pb_builtin_product(PbVal arr) {
    if (arr.tag != PB_TAG_ARR) return PB_NUM(1.0);
    double p = 1.0;
    for (size_t i = 0; i < arr.arr->len; i++)
        if (arr.arr->data[i].tag == PB_TAG_NUM) p *= arr.arr->data[i].num;
    return PB_NUM(p);
}
/* min_of(arr) → minimum numeric element, or nil when none. */
static PbVal pb_builtin_min_of(PbVal arr) {
    if (arr.tag != PB_TAG_ARR) return PB_NIL;
    bool seen = false; double m = 0.0;
    for (size_t i = 0; i < arr.arr->len; i++) {
        if (arr.arr->data[i].tag != PB_TAG_NUM) continue;
        double x = arr.arr->data[i].num;
        if (!seen || x < m) { m = x; seen = true; }
    }
    return seen ? PB_NUM(m) : PB_NIL;
}
/* max_of(arr) → maximum numeric element, or nil when none. */
static PbVal pb_builtin_max_of(PbVal arr) {
    if (arr.tag != PB_TAG_ARR) return PB_NIL;
    bool seen = false; double m = 0.0;
    for (size_t i = 0; i < arr.arr->len; i++) {
        if (arr.arr->data[i].tag != PB_TAG_NUM) continue;
        double x = arr.arr->data[i].num;
        if (!seen || x > m) { m = x; seen = true; }
    }
    return seen ? PB_NUM(m) : PB_NIL;
}
/* str_repeat(s, n) → s concatenated n times (n>=0). */
static PbVal pb_builtin_str_repeat(PbVal s, PbVal n) {
    if (s.tag != PB_TAG_STR || n.tag != PB_TAG_NUM || n.num < 0.0) return PB_STR("");
    size_t reps = (size_t)n.num;
    size_t sl = strlen(s.str);
    char* out = (char*)malloc(sl * reps + 1);
    char* op = out;
    for (size_t i = 0; i < reps; i++) { memcpy(op, s.str, sl); op += sl; }
    *op = 0;
    PbVal v; v.tag = PB_TAG_STR; v.str = out; return v;
}
/* str_join(arr, sep) → join string elements with sep; non-strings → "". */
static PbVal pb_builtin_str_join(PbVal arr, PbVal sep) {
    if (arr.tag != PB_TAG_ARR || sep.tag != PB_TAG_STR) return PB_STR("");
    size_t sepl = strlen(sep.str);
    size_t total = 1;
    for (size_t i = 0; i < arr.arr->len; i++) {
        if (arr.arr->data[i].tag == PB_TAG_STR) total += strlen(arr.arr->data[i].str);
        if (i + 1 < arr.arr->len) total += sepl;
    }
    char* out = (char*)malloc(total);
    char* op = out;
    for (size_t i = 0; i < arr.arr->len; i++) {
        if (arr.arr->data[i].tag == PB_TAG_STR) {
            size_t l = strlen(arr.arr->data[i].str);
            memcpy(op, arr.arr->data[i].str, l); op += l;
        }
        if (i + 1 < arr.arr->len) { memcpy(op, sep.str, sepl); op += sepl; }
    }
    *op = 0;
    PbVal v; v.tag = PB_TAG_STR; v.str = out; return v;
}
/* map_remove(m, key) → remove key (in place), return its value or nil. */
static PbVal pb_builtin_map_remove(PbVal m, PbVal key) {
    if (m.tag != PB_TAG_MAP || key.tag != PB_TAG_STR) return PB_NIL;
    PbMap* mm = m.map;
    for (size_t i = 0; i < mm->len; i++) {
        if (strcmp(mm->entries[i].key, key.str) == 0) {
            PbVal v = mm->entries[i].val;
            for (size_t j = i + 1; j < mm->len; j++) mm->entries[j - 1] = mm->entries[j];
            mm->len--;
            return v;
        }
    }
    return PB_NIL;
}
/* range(...) → [start, end) stepped; arity-specialized to keep the fast
   positional call path. step!=0; ascending while i<end else descending. */
static PbVal pb_builtin_range3(PbVal start, PbVal end, PbVal step) {
    double s = start.tag == PB_TAG_NUM ? start.num : 0.0;
    double e = end.tag == PB_TAG_NUM ? end.num : 0.0;
    double st = (step.tag == PB_TAG_NUM && step.num != 0.0) ? step.num : 1.0;
    PbVal arr = pb_arr_new(8);
    if (st > 0.0) { for (double i = s; i < e; i += st) pb_arr_push(arr, PB_NUM(i)); }
    else          { for (double i = s; i > e; i += st) pb_arr_push(arr, PB_NUM(i)); }
    return arr;
}
static PbVal pb_builtin_range2(PbVal start, PbVal end) { return pb_builtin_range3(start, end, PB_NUM(1.0)); }
static PbVal pb_builtin_range1(PbVal end) { return pb_builtin_range3(PB_NUM(0.0), end, PB_NUM(1.0)); }

/* num_ln(x) → natural log; num_exp(x) → e^x; num_atan2(y,x) → atan2. */
static PbVal pb_builtin_num_ln(PbVal v)  { return PB_NUM(log(v.num)); }
static PbVal pb_builtin_num_exp(PbVal v) { return PB_NUM(exp(v.num)); }
static PbVal pb_builtin_num_atan2(PbVal y, PbVal x) { return PB_NUM(atan2(y.num, x.num)); }

/* strfromcharcode(code) → the one Unicode codepoint, UTF-8 encoded. */
static PbVal pb_builtin_strfromcharcode(PbVal code) {
    if (code.tag != PB_TAG_NUM || code.num < 0) return PB_STR("");
    long cp = (long)code.num;
    char b[5]; int n;
    if      (cp < 0x80)    { b[0] = (char)cp; n = 1; }
    else if (cp < 0x800)   { b[0] = (char)(0xC0 | (cp >> 6)); b[1] = (char)(0x80 | (cp & 0x3F)); n = 2; }
    else if (cp < 0x10000) { b[0] = (char)(0xE0 | (cp >> 12)); b[1] = (char)(0x80 | ((cp >> 6) & 0x3F)); b[2] = (char)(0x80 | (cp & 0x3F)); n = 3; }
    else                   { b[0] = (char)(0xF0 | (cp >> 18)); b[1] = (char)(0x80 | ((cp >> 12) & 0x3F)); b[2] = (char)(0x80 | ((cp >> 6) & 0x3F)); b[3] = (char)(0x80 | (cp & 0x3F)); n = 4; }
    b[n] = 0;
    char* out = (char*)malloc(n + 1); memcpy(out, b, n + 1);
    PbVal v; v.tag = PB_TAG_STR; v.str = out; return v;
}

/* strcharcodeat(s, i) → codepoint value at codepoint index i (out of range → nil). */
static PbVal pb_builtin_strcharcodeat(PbVal s, PbVal idx) {
    if (s.tag != PB_TAG_STR || idx.tag != PB_TAG_NUM || idx.num < 0) return PB_NIL;
    long want = (long)idx.num;
    const unsigned char* p = (const unsigned char*)s.str;
    long cp = 0;
    while (*p) {
        if ((*p & 0xC0) != 0x80) {
            if (cp == want) {
                unsigned long c; int n;
                if      ((*p & 0x80) == 0x00) { c = *p; n = 1; }
                else if ((*p & 0xE0) == 0xC0) { c = *p & 0x1F; n = 2; }
                else if ((*p & 0xF0) == 0xE0) { c = *p & 0x0F; n = 3; }
                else                          { c = *p & 0x07; n = 4; }
                for (int k = 1; k < n; k++) if ((p[k] & 0xC0) == 0x80) c = (c << 6) | (p[k] & 0x3F);
                return PB_NUM((double)c);
            }
            cp++;
        }
        p++;
    }
    return PB_NIL;
}

/* Value equality for numbers/strings/bools/nil; arrays/maps by identity.
   Mirrors the common cases of the VM's Value == Value. */
static bool pb_val_eq(PbVal a, PbVal b) {
    if (a.tag != b.tag) return false;
    if (a.tag == PB_TAG_NUM)  return a.num == b.num;
    if (a.tag == PB_TAG_BOOL) return a.bval == b.bval;
    if (a.tag == PB_TAG_STR)  return strcmp(a.str, b.str) == 0;
    if (a.tag == PB_TAG_NIL)  return true;
    if (a.tag == PB_TAG_ARR)  return a.arr == b.arr;
    if (a.tag == PB_TAG_MAP)  return a.map == b.map;
    return false;
}

/* array_sort(arr) → NEW array: numbers ascending, strings lexicographic, mixed
   left in place. Stable insertion sort to match the VM's sort_by stability. */
static int pb_sort_cmp(PbVal x, PbVal y) {
    if (x.tag == PB_TAG_NUM && y.tag == PB_TAG_NUM) {
        if (x.num < y.num) return -1;
        if (x.num > y.num) return 1;
        return 0;
    }
    if (x.tag == PB_TAG_STR && y.tag == PB_TAG_STR) return strcmp(x.str, y.str);
    return 0;
}
static PbVal pb_builtin_array_sort(PbVal arr) {
    if (arr.tag != PB_TAG_ARR) return pb_arr_new(0);
    size_t n = arr.arr->len;
    PbVal out = pb_arr_new(n);
    for (size_t i = 0; i < n; i++) pb_arr_push(out, arr.arr->data[i]);
    for (size_t i = 1; i < n; i++) {
        PbVal cur = out.arr->data[i];
        size_t j = i;
        while (j > 0 && pb_sort_cmp(out.arr->data[j - 1], cur) > 0) {
            out.arr->data[j] = out.arr->data[j - 1];
            j--;
        }
        out.arr->data[j] = cur;
    }
    return out;
}

/* array_unique(arr) → NEW array, first-occurrence order, dedup by value. */
static PbVal pb_builtin_array_unique(PbVal arr) {
    if (arr.tag != PB_TAG_ARR) return pb_arr_new(0);
    PbVal out = pb_arr_new(8);
    for (size_t i = 0; i < arr.arr->len; i++) {
        PbVal v = arr.arr->data[i];
        bool seen = false;
        for (size_t k = 0; k < out.arr->len && !seen; k++)
            if (pb_val_eq(out.arr->data[k], v)) seen = true;
        if (!seen) pb_arr_push(out, v);
    }
    return out;
}

/* str_pad_left/right(s, width, pad) → pad s to `width` bytes with pad's first
   char; if already at/over width, return s unchanged. */
static PbVal pb_builtin_str_pad_left(PbVal s, PbVal width, PbVal pad) {
    if (s.tag != PB_TAG_STR || width.tag != PB_TAG_NUM) return s;
    size_t w = (size_t)(width.num < 0 ? 0 : width.num);
    size_t cur = strlen(s.str);
    if (cur >= w) return s;
    char pc = (pad.tag == PB_TAG_STR && pad.str[0]) ? pad.str[0] : ' ';
    char* out = (char*)malloc(w + 1);
    size_t fill = w - cur;
    for (size_t i = 0; i < fill; i++) out[i] = pc;
    memcpy(out + fill, s.str, cur);
    out[w] = 0;
    PbVal v; v.tag = PB_TAG_STR; v.str = out; return v;
}
static PbVal pb_builtin_str_pad_right(PbVal s, PbVal width, PbVal pad) {
    if (s.tag != PB_TAG_STR || width.tag != PB_TAG_NUM) return s;
    size_t w = (size_t)(width.num < 0 ? 0 : width.num);
    size_t cur = strlen(s.str);
    if (cur >= w) return s;
    char pc = (pad.tag == PB_TAG_STR && pad.str[0]) ? pad.str[0] : ' ';
    char* out = (char*)malloc(w + 1);
    memcpy(out, s.str, cur);
    for (size_t i = cur; i < w; i++) out[i] = pc;
    out[w] = 0;
    PbVal v; v.tag = PB_TAG_STR; v.str = out; return v;
}

/* array_take(arr, n) → first n elements (capped at len). */
static PbVal pb_builtin_array_take(PbVal arr, PbVal n) {
    if (arr.tag != PB_TAG_ARR || n.tag != PB_TAG_NUM) return pb_arr_new(0);
    size_t k = (size_t)(n.num < 0 ? 0 : n.num);
    if (k > arr.arr->len) k = arr.arr->len;
    PbVal out = pb_arr_new(k);
    for (size_t i = 0; i < k; i++) pb_arr_push(out, arr.arr->data[i]);
    return out;
}

/* array_drop(arr, n) → elements after the first n. */
static PbVal pb_builtin_array_drop(PbVal arr, PbVal n) {
    if (arr.tag != PB_TAG_ARR || n.tag != PB_TAG_NUM) return pb_arr_new(0);
    size_t k = (size_t)(n.num < 0 ? 0 : n.num);
    PbVal out = pb_arr_new(8);
    for (size_t i = k; i < arr.arr->len; i++) pb_arr_push(out, arr.arr->data[i]);
    return out;
}

/* array_chunk(arr, size) → array of sub-arrays of `size` (last may be smaller). */
static PbVal pb_builtin_array_chunk(PbVal arr, PbVal size) {
    if (arr.tag != PB_TAG_ARR || size.tag != PB_TAG_NUM || size.num < 1.0) return pb_arr_new(0);
    size_t sz = (size_t)size.num;
    PbVal out = pb_arr_new(8);
    size_t i = 0;
    while (i < arr.arr->len) {
        PbVal chunk = pb_arr_new(sz);
        size_t j = 0;
        while (j < sz && i < arr.arr->len) { pb_arr_push(chunk, arr.arr->data[i]); i++; j++; }
        pb_arr_push(out, chunk);
    }
    return out;
}

/* array_flatten(arr) → one level of nesting expanded; non-arrays kept. */
static PbVal pb_builtin_array_flatten(PbVal arr) {
    if (arr.tag != PB_TAG_ARR) return pb_arr_new(0);
    PbVal out = pb_arr_new(8);
    for (size_t i = 0; i < arr.arr->len; i++) {
        PbVal v = arr.arr->data[i];
        if (v.tag == PB_TAG_ARR) {
            for (size_t j = 0; j < v.arr->len; j++) pb_arr_push(out, v.arr->data[j]);
        } else {
            pb_arr_push(out, v);
        }
    }
    return out;
}

/* array_zip(a, b) → [a_i, b_i] pairs, stopping at the shorter length. */
static PbVal pb_builtin_array_zip(PbVal a, PbVal b) {
    if (a.tag != PB_TAG_ARR || b.tag != PB_TAG_ARR) return pb_arr_new(0);
    size_t n = a.arr->len < b.arr->len ? a.arr->len : b.arr->len;
    PbVal out = pb_arr_new(n);
    for (size_t i = 0; i < n; i++) {
        PbVal pair = pb_arr_new(2);
        pb_arr_push(pair, a.arr->data[i]);
        pb_arr_push(pair, b.arr->data[i]);
        pb_arr_push(out, pair);
    }
    return out;
}

/* enumerate(arr) → [index, value] pairs. */
static PbVal pb_builtin_enumerate(PbVal arr) {
    if (arr.tag != PB_TAG_ARR) return pb_arr_new(0);
    PbVal out = pb_arr_new(arr.arr->len);
    for (size_t i = 0; i < arr.arr->len; i++) {
        PbVal pair = pb_arr_new(2);
        pb_arr_push(pair, PB_NUM((double)i));
        pb_arr_push(pair, arr.arr->data[i]);
        pb_arr_push(out, pair);
    }
    return out;
}
static PbVal pb_builtin_str_starts_with(PbVal s, PbVal prefix) {
    if (s.tag!=PB_TAG_STR || prefix.tag!=PB_TAG_STR) return PB_FALSE;
    return pb_bool(strncmp(s.str, prefix.str, strlen(prefix.str)) == 0);
}
static PbVal pb_builtin_str_ends_with(PbVal s, PbVal suffix) {
    if (s.tag!=PB_TAG_STR || suffix.tag!=PB_TAG_STR) return PB_FALSE;
    size_t slen = strlen(s.str), pflen = strlen(suffix.str);
    if (pflen > slen) return PB_FALSE;
    return pb_bool(strcmp(s.str + slen - pflen, suffix.str) == 0);
}
static PbVal pb_builtin_str_trim(PbVal s) {
    if (s.tag != PB_TAG_STR) return s;
    const char* p = s.str;
    while (*p == ' ' || *p == '\t' || *p == '\n' || *p == '\r') p++;
    size_t len = strlen(p);
    while (len > 0 && (p[len-1]==' '||p[len-1]=='\t'||p[len-1]=='\n'||p[len-1]=='\r')) len--;
    char* out = (char*)malloc(len + 1);
    memcpy(out, p, len); out[len] = 0;
    PbVal v; v.tag = PB_TAG_STR; v.str = out; return v;
}
static PbVal pb_builtin_str_upper(PbVal s) {
    if (s.tag != PB_TAG_STR) return s;
    char* out = pb_strdup(s.str);
    for (char* p = out; *p; p++) if (*p >= 'a' && *p <= 'z') *p -= 32;
    PbVal v; v.tag = PB_TAG_STR; v.str = out; return v;
}
static PbVal pb_builtin_str_lower(PbVal s) {
    if (s.tag != PB_TAG_STR) return s;
    char* out = pb_strdup(s.str);
    for (char* p = out; *p; p++) if (*p >= 'A' && *p <= 'Z') *p += 32;
    PbVal v; v.tag = PB_TAG_STR; v.str = out; return v;
}
static PbVal pb_builtin_str_slice(PbVal s, PbVal start, PbVal end) {
    if (s.tag != PB_TAG_STR) return PB_STR("");
    size_t slen = strlen(s.str);
    size_t i = (size_t)start.num; if (i > slen) i = slen;
    size_t j = (size_t)end.num;   if (j > slen) j = slen; if (j < i) j = i;
    char* out = (char*)malloc(j - i + 1);
    memcpy(out, s.str + i, j - i); out[j-i] = 0;
    PbVal v; v.tag = PB_TAG_STR; v.str = out; return v;
}
static PbVal pb_builtin_str_split(PbVal s, PbVal delim) {
    if (s.tag != PB_TAG_STR || delim.tag != PB_TAG_STR) return pb_arr_new(0);
    PbArr* a = pb_arr_alloc(8);
    const char* p = s.str;
    const char* d = delim.str;
    size_t dl = strlen(d);
    if (dl == 0) { PbVal v; v.tag=PB_TAG_ARR; v.arr=a; return v; }
    const char* found;
    while ((found = strstr(p, d)) != NULL) {
        size_t len = found - p;
        char* seg = (char*)malloc(len + 1);
        memcpy(seg, p, len); seg[len] = 0;
        PbVal sv; sv.tag = PB_TAG_STR; sv.str = seg;
        pb_arr_push((PbVal){.tag=PB_TAG_ARR,.arr=a}, sv);
        p = found + dl;
    }
    PbVal tail; tail.tag = PB_TAG_STR; tail.str = pb_strdup(p);
    pb_arr_push((PbVal){.tag=PB_TAG_ARR,.arr=a}, tail);
    PbVal v; v.tag = PB_TAG_ARR; v.arr = a; return v;
}
static PbVal pb_builtin_str_replace_all(PbVal s, PbVal from, PbVal to) {
    if (s.tag!=PB_TAG_STR||from.tag!=PB_TAG_STR||to.tag!=PB_TAG_STR) return s;
    const char* src = s.str;
    const char* f = from.str;
    const char* t = to.str;
    size_t flen = strlen(f), tlen = strlen(t);
    if (flen == 0) return s;
    /* Count replacements */
    size_t count = 0;
    const char* p = src;
    const char* found;
    while ((found = strstr(p, f)) != NULL) { count++; p = found + flen; }
    size_t slen = strlen(src);
    size_t out_len = slen + count * (tlen > flen ? tlen - flen : 0) + count * (flen > tlen ? 0 : tlen) + 1;
    char* out = (char*)malloc(slen * 4 + 64); /* generous estimate */
    char* op = out; p = src;
    while ((found = strstr(p, f)) != NULL) {
        size_t pre = found - p;
        memcpy(op, p, pre); op += pre;
        memcpy(op, t, tlen); op += tlen;
        p = found + flen;
    }
    strcpy(op, p);
    (void)out_len;
    PbVal v; v.tag = PB_TAG_STR; v.str = out; return v;
}
static PbVal pb_builtin_assert(PbVal cond, PbVal msg) {
    if (!pb_truthy(cond)) {
        PbVal ms = pb_to_str(msg);
        fprintf(stderr, "pblang: AssertionError: %s\n", ms.str);
        exit(1);
    }
    return PB_NIL;
}
static PbVal pb_builtin_error(PbVal msg) {
    PbVal v; v.tag = PB_TAG_ERR; v.str = pb_strdup(pb_to_str(msg).str); return v;
}
static void pb_defer_push(PbVal fn) { (void)fn; /* stub */ }
static PbVal pb_builtin_print(PbVal v) { pb_print(v); return PB_NIL; }
static void pb_printf(PbVal v) { pb_print(v); }

typedef struct { const char* p; } PbJsonParser;

typedef struct {
    char* data;
    size_t len;
    size_t cap;
} PbBuf;

static void pb_buf_init(PbBuf* b) {
    b->len = 0;
    b->cap = 128;
    b->data = (char*)malloc(b->cap);
    b->data[0] = '\0';
}

static void pb_buf_reserve(PbBuf* b, size_t extra) {
    size_t need = b->len + extra + 1;
    if (need <= b->cap) return;
    while (b->cap < need) b->cap *= 2;
    b->data = (char*)realloc(b->data, b->cap);
}

static void pb_buf_append_n(PbBuf* b, const char* s, size_t n) {
    pb_buf_reserve(b, n);
    memcpy(b->data + b->len, s, n);
    b->len += n;
    b->data[b->len] = '\0';
}

static void pb_buf_append(PbBuf* b, const char* s) { pb_buf_append_n(b, s, strlen(s)); }

static void pb_json_skip_ws(PbJsonParser* jp) {
    while (*jp->p && isspace((unsigned char)*jp->p)) jp->p++;
}

static bool pb_json_match(PbJsonParser* jp, const char* word) {
    size_t n = strlen(word);
    if (strncmp(jp->p, word, n) == 0) { jp->p += n; return true; }
    return false;
}

static PbVal pb_json_parse_value(PbJsonParser* jp);

static PbVal pb_json_parse_string(PbJsonParser* jp) {
    if (*jp->p != '"') return PB_NIL;
    jp->p++;
    PbBuf out; pb_buf_init(&out);
    while (*jp->p && *jp->p != '"') {
        char ch = *jp->p++;
        if (ch == '\\') {
            char esc = *jp->p++;
            if (esc == '"' || esc == '\\' || esc == '/') {
                pb_buf_append_n(&out, &esc, 1);
            } else if (esc == 'b') {
                char c = '\b'; pb_buf_append_n(&out, &c, 1);
            } else if (esc == 'f') {
                char c = '\f'; pb_buf_append_n(&out, &c, 1);
            } else if (esc == 'n') {
                char c = '\n'; pb_buf_append_n(&out, &c, 1);
            } else if (esc == 'r') {
                char c = '\r'; pb_buf_append_n(&out, &c, 1);
            } else if (esc == 't') {
                char c = '\t'; pb_buf_append_n(&out, &c, 1);
            } else if (esc == 'u') {
                /* Minimal C99 runtime: keep JSON valid without full UTF-16 decoding. */
                for (int i = 0; i < 4 && isxdigit((unsigned char)jp->p[i]); i++) {}
                int ok = 1;
                for (int i = 0; i < 4; i++) if (!isxdigit((unsigned char)jp->p[i])) ok = 0;
                if (!ok) { free(out.data); return PB_NIL; }
                jp->p += 4;
                char q = '?'; pb_buf_append_n(&out, &q, 1);
            } else {
                free(out.data);
                return PB_NIL;
            }
        } else {
            pb_buf_append_n(&out, &ch, 1);
        }
    }
    if (*jp->p != '"') { free(out.data); return PB_NIL; }
    jp->p++;
    PbVal v; v.tag = PB_TAG_STR; v.str = out.data; return v;
}

static PbVal pb_json_parse_array(PbJsonParser* jp) {
    if (*jp->p != '[') return PB_NIL;
    jp->p++;
    PbVal arr = pb_arr_new(4);
    pb_json_skip_ws(jp);
    if (*jp->p == ']') { jp->p++; return arr; }
    while (*jp->p) {
        PbVal item = pb_json_parse_value(jp);
        pb_arr_push(arr, item);
        pb_json_skip_ws(jp);
        if (*jp->p == ',') { jp->p++; pb_json_skip_ws(jp); continue; }
        if (*jp->p == ']') { jp->p++; return arr; }
        return PB_NIL;
    }
    return PB_NIL;
}

static PbVal pb_json_parse_object(PbJsonParser* jp) {
    if (*jp->p != '{') return PB_NIL;
    jp->p++;
    PbVal map = pb_map_new();
    pb_json_skip_ws(jp);
    if (*jp->p == '}') { jp->p++; return map; }
    while (*jp->p) {
        PbVal key = pb_json_parse_string(jp);
        if (key.tag != PB_TAG_STR) return PB_NIL;
        pb_json_skip_ws(jp);
        if (*jp->p != ':') return PB_NIL;
        jp->p++;
        PbVal val = pb_json_parse_value(jp);
        pb_map_set(map, key, val);
        pb_json_skip_ws(jp);
        if (*jp->p == ',') { jp->p++; pb_json_skip_ws(jp); continue; }
        if (*jp->p == '}') { jp->p++; return map; }
        return PB_NIL;
    }
    return PB_NIL;
}

static PbVal pb_json_parse_number(PbJsonParser* jp) {
    char* end = NULL;
    double n = strtod(jp->p, &end);
    if (end == jp->p) return PB_NIL;
    jp->p = end;
    return PB_NUM(n);
}

static PbVal pb_json_parse_value(PbJsonParser* jp) {
    pb_json_skip_ws(jp);
    if (*jp->p == '"') return pb_json_parse_string(jp);
    if (*jp->p == '[') return pb_json_parse_array(jp);
    if (*jp->p == '{') return pb_json_parse_object(jp);
    if (*jp->p == '-' || (*jp->p >= '0' && *jp->p <= '9')) return pb_json_parse_number(jp);
    if (pb_json_match(jp, "true")) return PB_TRUE;
    if (pb_json_match(jp, "false")) return PB_FALSE;
    if (pb_json_match(jp, "null")) return PB_NIL;
    return PB_NIL;
}

static PbVal pb_builtin_json_parse(PbVal s) {
    if (s.tag != PB_TAG_STR) return PB_NIL;
    PbJsonParser jp; jp.p = s.str;
    PbVal v = pb_json_parse_value(&jp);
    pb_json_skip_ws(&jp);
    if (*jp.p != '\0') return PB_NIL;
    return v;
}

static void pb_json_stringify_value(PbBuf* out, PbVal v, int depth);

static void pb_json_stringify_string(PbBuf* out, const char* s) {
    pb_buf_append(out, "\"");
    for (const char* p = s; *p; p++) {
        char ch = *p;
        if (ch == '"') pb_buf_append(out, "\\\"");
        else if (ch == '\\') pb_buf_append(out, "\\\\");
        else if (ch == '\n') pb_buf_append(out, "\\n");
        else if (ch == '\r') pb_buf_append(out, "\\r");
        else if (ch == '\t') pb_buf_append(out, "\\t");
        else if ((unsigned char)ch < 32) {
            char tmp[8]; snprintf(tmp, sizeof(tmp), "\\u%04x", (unsigned char)ch);
            pb_buf_append(out, tmp);
        } else {
            pb_buf_append_n(out, &ch, 1);
        }
    }
    pb_buf_append(out, "\"");
}

static void pb_json_stringify_value(PbBuf* out, PbVal v, int depth) {
    if (depth > 64) { pb_buf_append(out, "null"); return; }
    char numbuf[64];
    switch (v.tag) {
        case PB_TAG_NIL:  pb_buf_append(out, "null"); break;
        case PB_TAG_BOOL: pb_buf_append(out, v.bval ? "true" : "false"); break;
        case PB_TAG_NUM:
            if (isfinite(v.num)) {
                if (v.num == (double)(long long)v.num)
                    snprintf(numbuf, sizeof(numbuf), "%lld", (long long)v.num);
                else
                    snprintf(numbuf, sizeof(numbuf), "%.17g", v.num);
                pb_buf_append(out, numbuf);
            } else {
                pb_buf_append(out, "null");
            }
            break;
        case PB_TAG_STR:
            pb_json_stringify_string(out, v.str ? v.str : "");
            break;
        case PB_TAG_ARR:
            pb_buf_append(out, "[");
            for (size_t i = 0; v.arr && i < v.arr->len; i++) {
                if (i > 0) pb_buf_append(out, ",");
                pb_json_stringify_value(out, v.arr->data[i], depth + 1);
            }
            pb_buf_append(out, "]");
            break;
        case PB_TAG_MAP:
            pb_buf_append(out, "{");
            for (size_t i = 0; v.map && i < v.map->len; i++) {
                if (i > 0) pb_buf_append(out, ",");
                pb_json_stringify_string(out, v.map->entries[i].key);
                pb_buf_append(out, ":");
                pb_json_stringify_value(out, v.map->entries[i].val, depth + 1);
            }
            pb_buf_append(out, "}");
            break;
        default:
            pb_buf_append(out, "null");
            break;
    }
}

static PbVal pb_builtin_json_stringify(PbVal v) {
    PbBuf out; pb_buf_init(&out);
    pb_json_stringify_value(&out, v, 0);
    PbVal s; s.tag = PB_TAG_STR; s.str = out.data; return s;
}
static PbVal pb_builtin_sha256(PbVal s) { (void)s; return PB_STR("0000000000000000000000000000000000000000000000000000000000000000"); }

/* ─── Init / Shutdown ────────────────────────────────────────────── */
static void pb_init(void)     { /* no-op for now */ }
static void pb_shutdown(void) { /* no-op for now */ }

#endif /* PBLANG_RT_H */
