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
                snprintf(buf, sizeof(buf), "%g", v.num);
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
    return PB_NIL;
}
static PbVal pb_safe_index(PbVal obj, PbVal idx) {
    if (obj.tag == PB_TAG_NIL) return PB_NIL;
    return pb_index(obj, idx);
}
static PbVal pb_safe_map_get(PbVal map, PbVal key) {
    if (map.tag == PB_TAG_NIL) return PB_NIL;
    return pb_map_get(map, key);
}

/* ─── Error handling (longjmp-based) ─────────────────────────────── */
static jmp_buf* pb__catch_buf = NULL;
static PbVal    pb__error_val;

static PbVal pb_try(PbVal try_val, PbVal catch_val) {
    /* Note: pb_try in C is evaluated eagerly — complex try/catch  */
    /* requires restructuring to macro or statement form.           */
    (void)catch_val;
    return try_val; /* simplified: no exception propagation */
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
    if (v.tag == PB_TAG_STR) return PB_NUM((double)strlen(v.str));
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
