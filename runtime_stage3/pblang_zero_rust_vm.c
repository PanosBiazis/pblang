/*
 * runtime_stage3/pblang_zero_rust_vm.c
 *
 * True Stage-3 Zero-Rust Native `.pbck` Bytecode Virtual Machine for PBLang 1.5.0.
 *
 * Portable C99 host runtime executing `.pbck` v1 bytecode containers produced by
 * the pure-PBLang compiler (`stdlib/compiler/native_pbck.sapp` / `tools/pbc.sapp`)
 * or `pblang build` with ZERO Rust crates, ZERO Rust runtime symbols, and ZERO
 * external libraries beyond ISO C99 + libm (`gcc -O3 -std=c99 ... -lm`).
 *
 * Capabilities:
 *   - Bounded `.pbck` v1 binary loader (magic "PBCK", version 1, <= 64 MiB,
 *     canonical 0/1 flags, max function depth 256, trailing-byte rejection).
 *   - Tagged `PbValue` runtime (`number`, `bool`, `nil`, `string`, `array`,
 *     `map`, `fn`, `closure`, `upvalue`, `builtin`).
 *   - Intrusive mark-and-sweep garbage collector rooted at VM stack, call frames,
 *     active closures, open upvalues, module globals/exports, and temp roots.
 *   - Complete `.pbck` v1 opcode execution (tags 0..=47), including lexical
 *     closures with shared mutable upvalues, `OpTailCall`, `OpTry`/`OpEndTry`/
 *     `OpThrow` structured exception unwinding, `OpDefer` LIFO thunks, and
 *     nil-safe indexing/attribute operators.
 *   - Built-in FIPS 180-4 SHA-256, JSON serializer, UTF-8 codepoint indexing,
 *     math, bitwise, string/array/map, filesystem, env, and monotonic clock builtins.
 */

#define _POSIX_C_SOURCE 200809L

#include <ctype.h>
#include <errno.h>
#include <math.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <time.h>

#define PBLANG_ZERO_RUST_VERSION "1.5.0"
#define PBCK_MAX_BYTES (64u * 1024u * 1024u)
#define PBCK_V1_VERSION 1u
#define PBCK_MAX_DEPTH 256u
#define VM_MAX_FRAMES 2048
#define VM_INITIAL_STACK 4096
#define VM_MAX_STACK (1024 * 1024)
#define VM_MAX_HANDLERS 512
#define VM_TEMP_ROOTS_MAX 128

/* ── Opcode Tags (.pbck v1 exact wire contract) ──────────────────────────── */
enum {
    OP_CONSTANT        = 0,
    OP_ADD             = 1,
    OP_SUB             = 2,
    OP_MUL             = 3,
    OP_DIV             = 4,
    OP_EQ              = 5,
    OP_NOT_EQ          = 6,
    OP_GT              = 7,
    OP_GE              = 8,
    OP_LT              = 9,
    OP_LE              = 10,
    OP_POP             = 11,
    OP_PRINT           = 12,
    OP_DEFINE_GLOBAL   = 13,
    OP_GET_GLOBAL      = 14,
    OP_SET_GLOBAL      = 15,
    OP_GET_ATTR        = 16,
    OP_SET_ATTR        = 17,
    OP_EXPORT          = 18,
    OP_MAKE_ARRAY      = 19,
    OP_MAKE_MAP        = 20,
    OP_GET_INDEX       = 21,
    OP_SET_INDEX       = 22,
    OP_TRY             = 23,
    OP_END_TRY         = 24,
    OP_GET_LOCAL       = 25,
    OP_SET_LOCAL       = 26,
    OP_CLOSURE         = 27,
    OP_GET_UPVALUE     = 28,
    OP_SET_UPVALUE     = 29,
    OP_CLOSE_UPVALUE   = 30,
    OP_CALL            = 31,
    OP_TAIL_CALL       = 32,
    OP_RETURN          = 33,
    OP_DEFER           = 34,
    OP_JUMP            = 35,
    OP_JUMP_IF_FALSE   = 36,
    OP_JUMP_IF_TRUE    = 37,
    OP_NOT             = 38,
    OP_NEGATE          = 39,
    OP_MOD             = 40,
    OP_POW             = 41,
    OP_SAFE_GET_ATTR   = 42,
    OP_SAFE_INDEX      = 43,
    OP_RESULT_UNWRAP   = 44,
    OP_IS_NIL          = 45,
    OP_DUP             = 46,
    OP_THROW           = 47
};

/* ── Forward Declarations ────────────────────────────────────────────────── */
typedef struct PbObj PbObj;
typedef struct PbStr PbStr;
typedef struct PbArray PbArray;
typedef struct PbMap PbMap;
typedef struct PbFunc PbFunc;
typedef struct PbClosure PbClosure;
typedef struct PbUpvalue PbUpvalue;
typedef struct PbVM PbVM;

typedef enum {
    VAL_NIL     = 0,
    VAL_BOOL    = 1,
    VAL_NUM     = 2,
    VAL_OBJ     = 3,
    VAL_BUILTIN = 4
} PbValTag;

typedef enum {
    OBJ_STR     = 1,
    OBJ_ARRAY   = 2,
    OBJ_MAP     = 3,
    OBJ_FUNC    = 4,
    OBJ_CLOSURE = 5,
    OBJ_UPVALUE = 6
} PbObjKind;

typedef struct PbValue {
    PbValTag tag;
    union {
        bool b;
        double num;
        PbObj *obj;
        int builtin_id;
    } u;
} PbValue;

struct PbObj {
    PbObjKind kind;
    bool marked;
    PbObj *next;
};

struct PbStr {
    PbObj base;
    uint32_t len;
    uint32_t hash;
    char data[];
};

struct PbArray {
    PbObj base;
    size_t len;
    size_t cap;
    PbValue *items;
};

typedef struct {
    PbStr *key;
    PbValue val;
    bool alive;
} PbMapEntry;

struct PbMap {
    PbObj base;
    size_t count;      /* live entries */
    size_t order_len;  /* length of entries[] insertion vector */
    size_t order_cap;
    PbMapEntry *entries;
    size_t bucket_cap; /* power of 2 hash index into entries[] (+1, 0=empty, -1=tomb) */
    int32_t *buckets;
};

typedef struct {
    uint8_t is_local;
    uint32_t idx;
} PbCaptureSpec;

typedef struct {
    uint8_t op;
    uint32_t arg;
    uint32_t n_caps;
    PbCaptureSpec *caps; /* non-NULL only for OP_CLOSURE with n_caps > 0 */
} PbInstr;

typedef struct {
    PbInstr *code;
    uint32_t code_len;
    PbValue *consts;
    uint32_t const_len;
    PbStr *source_name;
} PbChunk;

struct PbFunc {
    PbObj base;
    PbStr *name;
    uint32_t arity;
    uint32_t min_arity;
    uint32_t n_defaults;
    PbValue *defaults;
    bool is_vararg;
    PbChunk chunk;
};

struct PbUpvalue {
    PbObj base;
    bool is_open;
    size_t stack_idx;
    PbValue closed;
    PbUpvalue *next_open;
};

struct PbClosure {
    PbObj base;
    PbFunc *func;
    uint32_t n_upvalues;
    PbUpvalue **upvalues;
};

typedef struct {
    PbClosure *closure;
    uint32_t ip;
    size_t stack_base;
    PbValue *defers;
    size_t defer_len;
    size_t defer_cap;
} PbFrame;

typedef struct {
    size_t frame_idx;
    uint32_t target_ip;
    size_t stack_len;
} PbTryHandler;

typedef bool (*PbBuiltinFn)(PbVM *vm, int argc, const PbValue *args, PbValue *out);

typedef struct {
    const char *name;
    PbBuiltinFn fn;
} PbBuiltinDef;

struct PbVM {
    PbValue *stack;
    size_t stack_len;
    size_t stack_cap;

    PbFrame frames[VM_MAX_FRAMES];
    size_t frame_len;

    PbTryHandler handlers[VM_MAX_HANDLERS];
    size_t handler_len;

    PbUpvalue *open_upvalues;

    PbMap *globals;
    PbMap *exports;

    PbObj *objects;
    size_t num_objects;
    size_t next_gc;
    size_t total_str_allocs;
    size_t total_arr_allocs;
    size_t total_map_allocs;
    size_t gc_collections;
    int gc_inhibit;

    PbObj *temp_roots[VM_TEMP_ROOTS_MAX];
    size_t temp_root_len;

    int script_argc;
    char **script_argv;

    char error_msg[1024];
    bool has_error;
};

/* ── Value Constructors & Helpers ────────────────────────────────────────── */
static inline PbValue pb_nil(void) {
    PbValue v;
    v.tag = VAL_NIL;
    v.u.num = 0.0;
    return v;
}

static inline PbValue pb_bool(bool b) {
    PbValue v;
    v.tag = VAL_BOOL;
    v.u.b = b;
    return v;
}

static inline PbValue pb_num(double n) {
    PbValue v;
    v.tag = VAL_NUM;
    v.u.num = n;
    return v;
}

static inline PbValue pb_obj_val(PbObj *o) {
    PbValue v;
    v.tag = VAL_OBJ;
    v.u.obj = o;
    return v;
}

static inline PbValue pb_builtin_val(int id) {
    PbValue v;
    v.tag = VAL_BUILTIN;
    v.u.builtin_id = id;
    return v;
}

static inline bool pb_is_obj_kind(PbValue v, PbObjKind k) {
    return v.tag == VAL_OBJ && v.u.obj != NULL && v.u.obj->kind == k;
}

static uint32_t fnv1a_bytes(const uint8_t *data, size_t len) {
    uint32_t h = 2166136261u;
    for (size_t i = 0; i < len; i++) {
        h ^= (uint32_t)data[i];
        h *= 16777619u;
    }
    return h ? h : 1u;
}

static void *xmalloc(size_t sz) {
    void *p = malloc(sz ? sz : 1);
    if (!p) {
        fprintf(stderr, "pblang-zero-rust: out of memory allocating %zu bytes\n", sz);
        exit(2);
    }
    return p;
}

static void *xcalloc(size_t n, size_t sz) {
    void *p = calloc(n ? n : 1, sz ? sz : 1);
    if (!p) {
        fprintf(stderr, "pblang-zero-rust: out of memory allocating %zu*%zu bytes\n", n, sz);
        exit(2);
    }
    return p;
}

static void *xrealloc(void *ptr, size_t sz) {
    void *p = realloc(ptr, sz ? sz : 1);
    if (!p) {
        fprintf(stderr, "pblang-zero-rust: out of memory reallocating %zu bytes\n", sz);
        exit(2);
    }
    return p;
}

static void vm_set_error(PbVM *vm, const char *msg) {
    snprintf(vm->error_msg, sizeof(vm->error_msg), "%s", msg ? msg : "runtime error");
    vm->has_error = true;
}

static void vm_push_root(PbVM *vm, PbObj *o) {
    if (o && vm->temp_root_len < VM_TEMP_ROOTS_MAX) {
        vm->temp_roots[vm->temp_root_len++] = o;
    }
}

static void vm_pop_roots(PbVM *vm, size_t saved_len) {
    if (saved_len <= vm->temp_root_len) {
        vm->temp_root_len = saved_len;
    }
}

/* ── Garbage Collector ───────────────────────────────────────────────────── */
static void gc_mark_obj(PbObj *obj);

static void gc_mark_val(PbValue v) {
    if (v.tag == VAL_OBJ && v.u.obj != NULL) {
        gc_mark_obj(v.u.obj);
    }
}

static void gc_mark_obj(PbObj *obj) {
    if (!obj || obj->marked) return;
    obj->marked = true;
    switch (obj->kind) {
        case OBJ_STR:
            break;
        case OBJ_ARRAY: {
            PbArray *a = (PbArray *)obj;
            for (size_t i = 0; i < a->len; i++) {
                gc_mark_val(a->items[i]);
            }
            break;
        }
        case OBJ_MAP: {
            PbMap *m = (PbMap *)obj;
            for (size_t i = 0; i < m->order_len; i++) {
                if (m->entries[i].alive) {
                    gc_mark_obj((PbObj *)m->entries[i].key);
                    gc_mark_val(m->entries[i].val);
                }
            }
            break;
        }
        case OBJ_FUNC: {
            PbFunc *fn = (PbFunc *)obj;
            if (fn->name) gc_mark_obj((PbObj *)fn->name);
            if (fn->chunk.source_name) gc_mark_obj((PbObj *)fn->chunk.source_name);
            for (uint32_t i = 0; i < fn->n_defaults; i++) {
                gc_mark_val(fn->defaults[i]);
            }
            for (uint32_t i = 0; i < fn->chunk.const_len; i++) {
                gc_mark_val(fn->chunk.consts[i]);
            }
            break;
        }
        case OBJ_CLOSURE: {
            PbClosure *cl = (PbClosure *)obj;
            gc_mark_obj((PbObj *)cl->func);
            for (uint32_t i = 0; i < cl->n_upvalues; i++) {
                gc_mark_obj((PbObj *)cl->upvalues[i]);
            }
            break;
        }
        case OBJ_UPVALUE: {
            PbUpvalue *uv = (PbUpvalue *)obj;
            gc_mark_val(uv->closed);
            break;
        }
    }
}

static void free_obj(PbObj *obj) {
    switch (obj->kind) {
        case OBJ_STR:
            free(obj);
            break;
        case OBJ_ARRAY: {
            PbArray *a = (PbArray *)obj;
            free(a->items);
            free(a);
            break;
        }
        case OBJ_MAP: {
            PbMap *m = (PbMap *)obj;
            free(m->entries);
            free(m->buckets);
            free(m);
            break;
        }
        case OBJ_FUNC: {
            PbFunc *fn = (PbFunc *)obj;
            for (uint32_t i = 0; i < fn->chunk.code_len; i++) {
                free(fn->chunk.code[i].caps);
            }
            free(fn->chunk.code);
            free(fn->chunk.consts);
            free(fn->defaults);
            free(fn);
            break;
        }
        case OBJ_CLOSURE: {
            PbClosure *cl = (PbClosure *)obj;
            free(cl->upvalues);
            free(cl);
            break;
        }
        case OBJ_UPVALUE:
            free(obj);
            break;
    }
}

static size_t vm_gc_collect(PbVM *vm) {
    if (vm->gc_inhibit > 0) return 0;

    for (size_t i = 0; i < vm->stack_len; i++) {
        gc_mark_val(vm->stack[i]);
    }
    for (size_t i = 0; i < vm->frame_len; i++) {
        gc_mark_obj((PbObj *)vm->frames[i].closure);
        for (size_t d = 0; d < vm->frames[i].defer_len; d++) {
            gc_mark_val(vm->frames[i].defers[d]);
        }
    }
    for (PbUpvalue *uv = vm->open_upvalues; uv != NULL; uv = uv->next_open) {
        gc_mark_obj((PbObj *)uv);
    }
    if (vm->globals) gc_mark_obj((PbObj *)vm->globals);
    if (vm->exports) gc_mark_obj((PbObj *)vm->exports);
    for (size_t i = 0; i < vm->temp_root_len; i++) {
        gc_mark_obj(vm->temp_roots[i]);
    }

    size_t freed = 0;
    PbObj **link = &vm->objects;
    while (*link != NULL) {
        PbObj *curr = *link;
        if (!curr->marked) {
            *link = curr->next;
            free_obj(curr);
            vm->num_objects--;
            freed++;
        } else {
            curr->marked = false;
            link = &curr->next;
        }
    }
    vm->gc_collections++;
    size_t next = vm->num_objects * 2;
    if (next < 8192) next = 8192;
    vm->next_gc = next;
    return freed;
}

static inline void vm_maybe_gc(PbVM *vm) {
    if (vm->gc_inhibit == 0 && vm->num_objects >= vm->next_gc) {
        vm_gc_collect(vm);
    }
}

static PbObj *alloc_obj(PbVM *vm, size_t bytes, PbObjKind kind) {
    PbObj *o = (PbObj *)xcalloc(1, bytes);
    o->kind = kind;
    o->marked = false;
    o->next = vm->objects;
    vm->objects = o;
    vm->num_objects++;
    return o;
}

/* ── Heap Object Constructors & Operations ───────────────────────────────── */
static PbStr *pb_new_str(PbVM *vm, const char *bytes, size_t len) {
    PbStr *s = (PbStr *)alloc_obj(vm, sizeof(PbStr) + len + 1, OBJ_STR);
    s->len = (uint32_t)len;
    if (len > 0 && bytes != NULL) {
        memcpy(s->data, bytes, len);
    }
    s->data[len] = '\0';
    s->hash = fnv1a_bytes((const uint8_t *)s->data, len);
    vm->total_str_allocs++;
    return s;
}

static PbStr *pb_new_cstr(PbVM *vm, const char *cstr) {
    return pb_new_str(vm, cstr, cstr ? strlen(cstr) : 0);
}

static PbArray *pb_new_array(PbVM *vm, size_t cap) {
    PbArray *a = (PbArray *)alloc_obj(vm, sizeof(PbArray), OBJ_ARRAY);
    a->len = 0;
    a->cap = cap;
    a->items = cap > 0 ? (PbValue *)xcalloc(cap, sizeof(PbValue)) : NULL;
    vm->total_arr_allocs++;
    return a;
}

static void pb_array_push(PbArray *a, PbValue v) {
    if (a->len >= a->cap) {
        size_t ncap = a->cap < 8 ? 8 : a->cap * 2;
        a->items = (PbValue *)xrealloc(a->items, ncap * sizeof(PbValue));
        a->cap = ncap;
    }
    a->items[a->len++] = v;
}

static bool pb_str_eq(const PbStr *a, const PbStr *b) {
    if (a == b) return true;
    if (a->len != b->len || a->hash != b->hash) return false;
    return memcmp(a->data, b->data, a->len) == 0;
}

static PbMap *pb_new_map(PbVM *vm, size_t initial_cap) {
    PbMap *m = (PbMap *)alloc_obj(vm, sizeof(PbMap), OBJ_MAP);
    size_t bcap = 16;
    while (bcap < initial_cap * 2) bcap *= 2;
    m->bucket_cap = bcap;
    m->buckets = (int32_t *)xcalloc(bcap, sizeof(int32_t));
    m->order_cap = initial_cap < 8 ? 8 : initial_cap;
    m->entries = (PbMapEntry *)xcalloc(m->order_cap, sizeof(PbMapEntry));
    m->count = 0;
    m->order_len = 0;
    vm->total_map_allocs++;
    return m;
}

static void pb_map_rehash(PbMap *m, size_t new_bcap) {
    int32_t *nb = (int32_t *)xcalloc(new_bcap, sizeof(int32_t));
    size_t mask = new_bcap - 1;
    for (size_t i = 0; i < m->order_len; i++) {
        if (!m->entries[i].alive) continue;
        size_t slot = m->entries[i].key->hash & mask;
        while (nb[slot] > 0) {
            slot = (slot + 1) & mask;
        }
        nb[slot] = (int32_t)(i + 1);
    }
    free(m->buckets);
    m->buckets = nb;
    m->bucket_cap = new_bcap;
}

static bool pb_map_get_cstr(const PbMap *m, const char *key, size_t klen, uint32_t khash, PbValue *out) {
    if (m->count == 0) return false;
    size_t mask = m->bucket_cap - 1;
    size_t slot = khash & mask;
    for (size_t probes = 0; probes < m->bucket_cap; probes++) {
        int32_t b = m->buckets[slot];
        if (b == 0) return false;
        if (b > 0) {
            const PbMapEntry *e = &m->entries[(size_t)(b - 1)];
            if (e->alive && e->key->hash == khash && e->key->len == klen &&
                memcmp(e->key->data, key, klen) == 0) {
                if (out) *out = e->val;
                return true;
            }
        }
        slot = (slot + 1) & mask;
    }
    return false;
}

static bool pb_map_get(const PbMap *m, const PbStr *key, PbValue *out) {
    return pb_map_get_cstr(m, key->data, key->len, key->hash, out);
}

static void pb_map_set(PbMap *m, PbStr *key, PbValue val) {
    if ((m->order_len + 1) * 10 >= m->bucket_cap * 7) {
        pb_map_rehash(m, m->bucket_cap * 2);
    }
    size_t mask = m->bucket_cap - 1;
    size_t slot = key->hash & mask;
    int32_t first_tomb = -1;
    for (;;) {
        int32_t b = m->buckets[slot];
        if (b == 0) {
            size_t target_slot = (first_tomb >= 0) ? (size_t)first_tomb : slot;
            if (m->order_len >= m->order_cap) {
                size_t ncap = m->order_cap * 2;
                m->entries = (PbMapEntry *)xrealloc(m->entries, ncap * sizeof(PbMapEntry));
                m->order_cap = ncap;
            }
            size_t idx = m->order_len++;
            m->entries[idx].key = key;
            m->entries[idx].val = val;
            m->entries[idx].alive = true;
            m->buckets[target_slot] = (int32_t)(idx + 1);
            m->count++;
            return;
        }
        if (b < 0) {
            if (first_tomb < 0) first_tomb = (int32_t)slot;
        } else {
            PbMapEntry *e = &m->entries[(size_t)(b - 1)];
            if (e->alive && pb_str_eq(e->key, key)) {
                e->val = val;
                return;
            }
        }
        slot = (slot + 1) & mask;
    }
}

static bool pb_map_remove(PbMap *m, const PbStr *key, PbValue *removed_out) {
    if (m->count == 0) return false;
    size_t mask = m->bucket_cap - 1;
    size_t slot = key->hash & mask;
    for (size_t probes = 0; probes < m->bucket_cap; probes++) {
        int32_t b = m->buckets[slot];
        if (b == 0) return false;
        if (b > 0) {
            PbMapEntry *e = &m->entries[(size_t)(b - 1)];
            if (e->alive && pb_str_eq(e->key, key)) {
                if (removed_out) *removed_out = e->val;
                e->alive = false;
                e->val = pb_nil();
                m->buckets[slot] = -1;
                m->count--;
                return true;
            }
        }
        slot = (slot + 1) & mask;
    }
    return false;
}

static PbClosure *pb_new_closure(PbVM *vm, PbFunc *func, uint32_t n_upvals) {
    size_t saved = vm->temp_root_len;
    vm_push_root(vm, (PbObj *)func);
    PbClosure *cl = (PbClosure *)alloc_obj(vm, sizeof(PbClosure), OBJ_CLOSURE);
    cl->func = func;
    cl->n_upvalues = n_upvals;
    cl->upvalues = n_upvals > 0 ? (PbUpvalue **)xcalloc(n_upvals, sizeof(PbUpvalue *)) : NULL;
    vm_pop_roots(vm, saved);
    return cl;
}

static PbUpvalue *vm_capture_upvalue(PbVM *vm, size_t stack_idx) {
    PbUpvalue *prev = NULL;
    PbUpvalue *curr = vm->open_upvalues;
    while (curr != NULL && curr->stack_idx > stack_idx) {
        prev = curr;
        curr = curr->next_open;
    }
    if (curr != NULL && curr->stack_idx == stack_idx) {
        return curr;
    }
    PbUpvalue *created = (PbUpvalue *)alloc_obj(vm, sizeof(PbUpvalue), OBJ_UPVALUE);
    created->is_open = true;
    created->stack_idx = stack_idx;
    created->closed = pb_nil();
    created->next_open = curr;
    if (prev == NULL) {
        vm->open_upvalues = created;
    } else {
        prev->next_open = created;
    }
    return created;
}

static void vm_close_upvalues_from(PbVM *vm, size_t from_stack_idx) {
    while (vm->open_upvalues != NULL && vm->open_upvalues->stack_idx >= from_stack_idx) {
        PbUpvalue *uv = vm->open_upvalues;
        uv->closed = (uv->stack_idx < vm->stack_len) ? vm->stack[uv->stack_idx] : pb_nil();
        uv->is_open = false;
        vm->open_upvalues = uv->next_open;
        uv->next_open = NULL;
    }
}

/* ── UTF-8 Codepoint Helpers ─────────────────────────────────────────────── */
static size_t utf8_codepoint_count(const char *s, size_t byte_len) {
    size_t count = 0;
    for (size_t i = 0; i < byte_len; i++) {
        if (((uint8_t)s[i] & 0xC0u) != 0x80u) count++;
    }
    return count;
}

static size_t utf8_char_to_byte_offset(const char *s, size_t byte_len, size_t char_idx) {
    size_t cur_char = 0;
    size_t i = 0;
    while (i < byte_len && cur_char < char_idx) {
        uint8_t c = (uint8_t)s[i];
        size_t step = 1;
        if ((c & 0x80u) == 0) step = 1;
        else if ((c & 0xE0u) == 0xC0u) step = 2;
        else if ((c & 0xF0u) == 0xE0u) step = 3;
        else if ((c & 0xF8u) == 0xF0u) step = 4;
        if (i + step > byte_len) step = 1;
        i += step;
        cur_char++;
    }
    return i;
}

static uint32_t utf8_decode_at(const char *s, size_t byte_len, size_t byte_off) {
    if (byte_off >= byte_len) return 0;
    uint8_t b0 = (uint8_t)s[byte_off];
    if ((b0 & 0x80u) == 0) return b0;
    if ((b0 & 0xE0u) == 0xC0u && byte_off + 1 < byte_len) {
        return ((uint32_t)(b0 & 0x1Fu) << 6) | ((uint32_t)((uint8_t)s[byte_off + 1] & 0x3Fu));
    }
    if ((b0 & 0xF0u) == 0xE0u && byte_off + 2 < byte_len) {
        return ((uint32_t)(b0 & 0x0Fu) << 12) |
               ((uint32_t)((uint8_t)s[byte_off + 1] & 0x3Fu) << 6) |
               ((uint32_t)((uint8_t)s[byte_off + 2] & 0x3Fu));
    }
    if ((b0 & 0xF8u) == 0xF0u && byte_off + 3 < byte_len) {
        return ((uint32_t)(b0 & 0x07u) << 18) |
               ((uint32_t)((uint8_t)s[byte_off + 1] & 0x3Fu) << 12) |
               ((uint32_t)((uint8_t)s[byte_off + 2] & 0x3Fu) << 6) |
               ((uint32_t)((uint8_t)s[byte_off + 3] & 0x3Fu));
    }
    return b0;
}

static size_t utf8_encode_cp(uint32_t cp, char out[4]) {
    if (cp <= 0x7Fu) {
        out[0] = (char)cp;
        return 1;
    }
    if (cp <= 0x7FFu) {
        out[0] = (char)(0xC0u | (cp >> 6));
        out[1] = (char)(0x80u | (cp & 0x3Fu));
        return 2;
    }
    if (cp <= 0xFFFFu) {
        out[0] = (char)(0xE0u | (cp >> 12));
        out[1] = (char)(0x80u | ((cp >> 6) & 0x3Fu));
        out[2] = (char)(0x80u | (cp & 0x3Fu));
        return 3;
    }
    out[0] = (char)(0xF0u | (cp >> 18));
    out[1] = (char)(0x80u | ((cp >> 12) & 0x3Fu));
    out[2] = (char)(0x80u | ((cp >> 6) & 0x3Fu));
    out[3] = (char)(0x80u | (cp & 0x3Fu));
    return 4;
}

/* ── Dynamic String Builder ──────────────────────────────────────────────── */
typedef struct {
    char *buf;
    size_t len;
    size_t cap;
} StrBuf;

static void sb_init(StrBuf *sb) {
    sb->cap = 64;
    sb->len = 0;
    sb->buf = (char *)xmalloc(sb->cap);
    sb->buf[0] = '\0';
}

static void sb_append_bytes(StrBuf *sb, const char *data, size_t n) {
    if (n == 0) return;
    if (sb->len + n + 1 > sb->cap) {
        size_t ncap = sb->cap * 2;
        while (ncap < sb->len + n + 1) ncap *= 2;
        sb->buf = (char *)xrealloc(sb->buf, ncap);
        sb->cap = ncap;
    }
    memcpy(sb->buf + sb->len, data, n);
    sb->len += n;
    sb->buf[sb->len] = '\0';
}

static void sb_append_cstr(StrBuf *sb, const char *s) {
    sb_append_bytes(sb, s, strlen(s));
}

static void sb_append_char(StrBuf *sb, char c) {
    sb_append_bytes(sb, &c, 1);
}

/* ── Value Equality, Truthiness, Formatting & JSON ───────────────────────── */
static bool pb_val_equal(PbValue a, PbValue b) {
    if (a.tag != b.tag) return false;
    switch (a.tag) {
        case VAL_NIL: return true;
        case VAL_BOOL: return a.u.b == b.u.b;
        case VAL_NUM: return a.u.num == b.u.num;
        case VAL_BUILTIN: return a.u.builtin_id == b.u.builtin_id;
        case VAL_OBJ: {
            if (a.u.obj == b.u.obj) return true;
            if (!a.u.obj || !b.u.obj || a.u.obj->kind != b.u.obj->kind) return false;
            switch (a.u.obj->kind) {
                case OBJ_STR:
                    return pb_str_eq((const PbStr *)a.u.obj, (const PbStr *)b.u.obj);
                case OBJ_ARRAY: {
                    const PbArray *aa = (const PbArray *)a.u.obj;
                    const PbArray *bb = (const PbArray *)b.u.obj;
                    if (aa->len != bb->len) return false;
                    for (size_t i = 0; i < aa->len; i++) {
                        if (!pb_val_equal(aa->items[i], bb->items[i])) return false;
                    }
                    return true;
                }
                case OBJ_MAP: {
                    const PbMap *ma = (const PbMap *)a.u.obj;
                    const PbMap *mb = (const PbMap *)b.u.obj;
                    if (ma->count != mb->count) return false;
                    for (size_t i = 0; i < ma->order_len; i++) {
                        if (!ma->entries[i].alive) continue;
                        PbValue bv;
                        if (!pb_map_get(mb, ma->entries[i].key, &bv)) return false;
                        if (!pb_val_equal(ma->entries[i].val, bv)) return false;
                    }
                    return true;
                }
                default:
                    return false;
            }
        }
    }
    return false;
}

static bool pb_is_truthy(PbValue v) {
    switch (v.tag) {
        case VAL_NIL: return false;
        case VAL_BOOL: return v.u.b;
        case VAL_NUM: return v.u.num != 0.0;
        case VAL_BUILTIN: return true;
        case VAL_OBJ:
            switch (v.u.obj->kind) {
                case OBJ_STR: return ((const PbStr *)v.u.obj)->len > 0;
                case OBJ_ARRAY: return ((const PbArray *)v.u.obj)->len > 0;
                case OBJ_MAP: return ((const PbMap *)v.u.obj)->count > 0;
                default: return true;
            }
    }
    return false;
}

static void pb_format_number(double n, char out[64]) {
    if (isnan(n)) {
        snprintf(out, 64, "NaN");
        return;
    }
    if (isinf(n)) {
        snprintf(out, 64, "%s", signbit(n) ? "-inf" : "inf");
        return;
    }
    if (n == 0.0) {
        snprintf(out, 64, "%s", signbit(n) ? "-0" : "0");
        return;
    }
    if (fabs(n) <= 9007199254740992.0 && trunc(n) == n) {
        snprintf(out, 64, "%lld", (long long)n);
        return;
    }
    snprintf(out, 64, "%.15g", n);
}

static const PbBuiltinDef *get_builtin_table(size_t *count_out);

static void pb_format_value_rec(StrBuf *sb, PbValue v, int depth) {
    if (depth > 64) {
        sb_append_cstr(sb, "...");
        return;
    }
    switch (v.tag) {
        case VAL_NIL:
            sb_append_cstr(sb, "nil");
            break;
        case VAL_BOOL:
            sb_append_cstr(sb, v.u.b ? "true" : "false");
            break;
        case VAL_NUM: {
            char buf[64];
            pb_format_number(v.u.num, buf);
            sb_append_cstr(sb, buf);
            break;
        }
        case VAL_BUILTIN: {
            size_t n_b = 0;
            const PbBuiltinDef *tbl = get_builtin_table(&n_b);
            sb_append_cstr(sb, "<builtin ");
            if (v.u.builtin_id >= 0 && (size_t)v.u.builtin_id < n_b) {
                sb_append_cstr(sb, tbl[v.u.builtin_id].name);
            }
            sb_append_char(sb, '>');
            break;
        }
        case VAL_OBJ:
            switch (v.u.obj->kind) {
                case OBJ_STR: {
                    const PbStr *s = (const PbStr *)v.u.obj;
                    sb_append_bytes(sb, s->data, s->len);
                    break;
                }
                case OBJ_FUNC:
                    sb_append_cstr(sb, "fn");
                    break;
                case OBJ_CLOSURE:
                    sb_append_cstr(sb, "closure");
                    break;
                case OBJ_UPVALUE:
                    sb_append_cstr(sb, "<upvalue>");
                    break;
                case OBJ_ARRAY: {
                    const PbArray *a = (const PbArray *)v.u.obj;
                    sb_append_char(sb, '[');
                    for (size_t i = 0; i < a->len; i++) {
                        if (i > 0) sb_append_cstr(sb, ", ");
                        pb_format_value_rec(sb, a->items[i], depth + 1);
                    }
                    sb_append_char(sb, ']');
                    break;
                }
                case OBJ_MAP: {
                    const PbMap *m = (const PbMap *)v.u.obj;
                    sb_append_cstr(sb, "map{");
                    bool first = true;
                    for (size_t i = 0; i < m->order_len; i++) {
                        if (!m->entries[i].alive) continue;
                        if (!first) sb_append_cstr(sb, ", ");
                        first = false;
                        sb_append_bytes(sb, m->entries[i].key->data, m->entries[i].key->len);
                        sb_append_cstr(sb, ": ");
                        pb_format_value_rec(sb, m->entries[i].val, depth + 1);
                    }
                    sb_append_char(sb, '}');
                    break;
                }
            }
            break;
    }
}

static void sb_append_json_str(StrBuf *sb, const char *data, size_t len) {
    sb_append_char(sb, '"');
    for (size_t i = 0; i < len; i++) {
        uint8_t c = (uint8_t)data[i];
        switch (c) {
            case '"':  sb_append_cstr(sb, "\\\""); break;
            case '\\': sb_append_cstr(sb, "\\\\"); break;
            case '\b': sb_append_cstr(sb, "\\b"); break;
            case '\f': sb_append_cstr(sb, "\\f"); break;
            case '\n': sb_append_cstr(sb, "\\n"); break;
            case '\r': sb_append_cstr(sb, "\\r"); break;
            case '\t': sb_append_cstr(sb, "\\t"); break;
            default:
                if (c < 0x20u) {
                    char esc[8];
                    snprintf(esc, sizeof(esc), "\\u%04x", (unsigned)c);
                    sb_append_cstr(sb, esc);
                } else {
                    sb_append_char(sb, (char)c);
                }
                break;
        }
    }
    sb_append_char(sb, '"');
}

static bool pb_json_stringify_rec(PbVM *vm, StrBuf *sb, PbValue v, int depth) {
    if (depth > 64) {
        vm_set_error(vm, "json_stringify(): cyclic or excessively deep value");
        return false;
    }
    switch (v.tag) {
        case VAL_NIL:
            sb_append_cstr(sb, "null");
            return true;
        case VAL_BOOL:
            sb_append_cstr(sb, v.u.b ? "true" : "false");
            return true;
        case VAL_NUM: {
            if (!isfinite(v.u.num)) {
                sb_append_cstr(sb, "null");
                return true;
            }
            char buf[64];
            pb_format_number(v.u.num, buf);
            sb_append_cstr(sb, buf);
            return true;
        }
        case VAL_OBJ:
            switch (v.u.obj->kind) {
                case OBJ_STR: {
                    const PbStr *s = (const PbStr *)v.u.obj;
                    sb_append_json_str(sb, s->data, s->len);
                    return true;
                }
                case OBJ_ARRAY: {
                    const PbArray *a = (const PbArray *)v.u.obj;
                    sb_append_char(sb, '[');
                    for (size_t i = 0; i < a->len; i++) {
                        if (i > 0) sb_append_char(sb, ',');
                        if (!pb_json_stringify_rec(vm, sb, a->items[i], depth + 1)) return false;
                    }
                    sb_append_char(sb, ']');
                    return true;
                }
                case OBJ_MAP: {
                    const PbMap *m = (const PbMap *)v.u.obj;
                    sb_append_char(sb, '{');
                    bool first = true;
                    for (size_t i = 0; i < m->order_len; i++) {
                        if (!m->entries[i].alive) continue;
                        if (!first) sb_append_char(sb, ',');
                        first = false;
                        sb_append_json_str(sb, m->entries[i].key->data, m->entries[i].key->len);
                        sb_append_char(sb, ':');
                        if (!pb_json_stringify_rec(vm, sb, m->entries[i].val, depth + 1)) return false;
                    }
                    sb_append_char(sb, '}');
                    return true;
                }
                default:
                    sb_append_cstr(sb, "null");
                    return true;
            }
        case VAL_BUILTIN:
            sb_append_cstr(sb, "null");
            return true;
    }
    return true;
}

/* ── Built-in FIPS 180-4 SHA-256 ─────────────────────────────────────────── */
static const uint32_t SHA256_K[64] = {
    0x428a2f98u, 0x71374491u, 0xb5c0fbcfu, 0xe9b5dba5u, 0x3956c25bu, 0x59f111f1u, 0x923f82a4u, 0xab1c5ed5u,
    0xd807aa98u, 0x12835b01u, 0x243185beu, 0x550c7dc3u, 0x72be5d74u, 0x80deb1feu, 0x9bdc06a7u, 0xc19bf174u,
    0xe49b69c1u, 0xefbe4786u, 0x0fc19dc6u, 0x240ca1ccu, 0x2de92c6fu, 0x4a7484aau, 0x5cb0a9dcu, 0x76f988dau,
    0x983e5152u, 0xa831c66du, 0xb00327c8u, 0xbf597fc7u, 0xc6e00bf3u, 0xd5a79147u, 0x06ca6351u, 0x14292967u,
    0x27b70a85u, 0x2e1b2138u, 0x4d2c6dfcU, 0x53380d13u, 0x650a7354u, 0x766a0abbu, 0x81c2c92eu, 0x92722c85u,
    0xa2bfe8a1u, 0xa81a664bu, 0xc24b8b70u, 0xc76c51a3u, 0xd192e819u, 0xd6990624u, 0xf40e3585u, 0x106aa070u,
    0x19a4c116u, 0x1e376c08u, 0x2748774cu, 0x34b0bcb5u, 0x391c0cb3u, 0x4ed8aa4au, 0x5b9cca4fu, 0x682e6ff3u,
    0x748f82eeu, 0x78a5636fu, 0x84c87814u, 0x8cc70208u, 0x90befffau, 0xa4506cebu, 0xbef9a3f7u, 0xc67178f2u
};

static inline uint32_t rotr32(uint32_t x, uint32_t n) {
    return (x >> n) | (x << (32u - n));
}

static void sha256_block(uint32_t state[8], const uint8_t block[64]) {
    uint32_t w[64];
    for (int i = 0; i < 16; i++) {
        w[i] = ((uint32_t)block[i * 4] << 24) |
               ((uint32_t)block[i * 4 + 1] << 16) |
               ((uint32_t)block[i * 4 + 2] << 8) |
               ((uint32_t)block[i * 4 + 3]);
    }
    for (int i = 16; i < 64; i++) {
        uint32_t s0 = rotr32(w[i - 15], 7) ^ rotr32(w[i - 15], 18) ^ (w[i - 15] >> 3);
        uint32_t s1 = rotr32(w[i - 2], 17) ^ rotr32(w[i - 2], 19) ^ (w[i - 2] >> 10);
        w[i] = w[i - 16] + s0 + w[i - 7] + s1;
    }
    uint32_t a = state[0], b = state[1], c = state[2], d = state[3];
    uint32_t e = state[4], f = state[5], g = state[6], h = state[7];
    for (int i = 0; i < 64; i++) {
        uint32_t S1 = rotr32(e, 6) ^ rotr32(e, 11) ^ rotr32(e, 25);
        uint32_t ch = (e & f) ^ ((~e) & g);
        uint32_t temp1 = h + S1 + ch + SHA256_K[i] + w[i];
        uint32_t S0 = rotr32(a, 2) ^ rotr32(a, 13) ^ rotr32(a, 22);
        uint32_t maj = (a & b) ^ (a & c) ^ (b & c);
        uint32_t temp2 = S0 + maj;
        h = g; g = f; f = e; e = d + temp1;
        d = c; c = b; b = a; a = temp1 + temp2;
    }
    state[0] += a; state[1] += b; state[2] += c; state[3] += d;
    state[4] += e; state[5] += f; state[6] += g; state[7] += h;
}

static void sha256_hex(const uint8_t *data, size_t len, char out_hex[65]) {
    uint32_t state[8] = {
        0x6a09e667u, 0xbb67ae85u, 0x3c6ef372u, 0xa54ff53au,
        0x510e527fu, 0x9b05688cu, 0x1f83d9abu, 0x5be0cd19u
    };
    size_t offset = 0;
    while (offset + 64 <= len) {
        sha256_block(state, data + offset);
        offset += 64;
    }
    uint8_t tail[128];
    memset(tail, 0, sizeof(tail));
    size_t rem = len - offset;
    if (rem > 0) memcpy(tail, data + offset, rem);
    tail[rem] = 0x80u;
    size_t total_blocks = (rem >= 56) ? 2 : 1;
    uint64_t bit_len = (uint64_t)len * 8u;
    size_t len_pos = total_blocks * 64 - 8;
    for (int i = 0; i < 8; i++) {
        tail[len_pos + i] = (uint8_t)((bit_len >> (56 - i * 8)) & 0xFFu);
    }
    sha256_block(state, tail);
    if (total_blocks == 2) sha256_block(state, tail + 64);

    static const char HEX[] = "0123456789abcdef";
    for (int i = 0; i < 8; i++) {
        uint32_t v = state[i];
        for (int b = 0; b < 4; b++) {
            uint8_t byte = (uint8_t)((v >> (24 - b * 8)) & 0xFFu);
            out_hex[i * 8 + b * 2]     = HEX[byte >> 4];
            out_hex[i * 8 + b * 2 + 1] = HEX[byte & 0x0Fu];
        }
    }
    out_hex[64] = '\0';
}

/* ── Host Builtins ───────────────────────────────────────────────────────── */
static bool req_argc(PbVM *vm, const char *name, int got, int min_a, int max_a) {
    if (got < min_a || got > max_a) {
        char buf[128];
        if (min_a == max_a) {
            snprintf(buf, sizeof(buf), "%s() expects %d argument(s), got %d", name, min_a, got);
        } else {
            snprintf(buf, sizeof(buf), "%s() expects %d..%d arguments, got %d", name, min_a, max_a, got);
        }
        vm_set_error(vm, buf);
        return false;
    }
    return true;
}

static bool req_num(PbVM *vm, const char *name, PbValue v, double *out) {
    if (v.tag != VAL_NUM) {
        char buf[128];
        snprintf(buf, sizeof(buf), "%s(): expected number", name);
        vm_set_error(vm, buf);
        return false;
    }
    *out = v.u.num;
    return true;
}

static bool req_str(PbVM *vm, const char *name, PbValue v, PbStr **out) {
    if (!pb_is_obj_kind(v, OBJ_STR)) {
        char buf[128];
        snprintf(buf, sizeof(buf), "%s(): expected string", name);
        vm_set_error(vm, buf);
        return false;
    }
    *out = (PbStr *)v.u.obj;
    return true;
}

static bool b_print_common(PbVM *vm, int argc, const PbValue *args, PbValue *out, bool newline) {
    (void)vm;
    StrBuf sb;
    sb_init(&sb);
    for (int i = 0; i < argc; i++) {
        if (i > 0) sb_append_char(&sb, ' ');
        pb_format_value_rec(&sb, args[i], 0);
    }
    fwrite(sb.buf, 1, sb.len, stdout);
    if (newline) fputc('\n', stdout);
    fflush(stdout);
    free(sb.buf);
    *out = pb_nil();
    return true;
}

static bool b_print(PbVM *vm, int argc, const PbValue *args, PbValue *out) {
    return b_print_common(vm, argc, args, out, false);
}

static bool b_println(PbVM *vm, int argc, const PbValue *args, PbValue *out) {
    return b_print_common(vm, argc, args, out, true);
}

static bool b_len(PbVM *vm, int argc, const PbValue *args, PbValue *out) {
    if (!req_argc(vm, "len", argc, 1, 1)) return false;
    if (pb_is_obj_kind(args[0], OBJ_STR)) {
        PbStr *s = (PbStr *)args[0].u.obj;
        *out = pb_num((double)utf8_codepoint_count(s->data, s->len));
        return true;
    }
    if (pb_is_obj_kind(args[0], OBJ_ARRAY)) {
        *out = pb_num((double)((PbArray *)args[0].u.obj)->len);
        return true;
    }
    if (pb_is_obj_kind(args[0], OBJ_MAP)) {
        *out = pb_num((double)((PbMap *)args[0].u.obj)->count);
        return true;
    }
    vm_set_error(vm, "len() currently supports strings/arrays/maps only");
    return false;
}

static bool b_type(PbVM *vm, int argc, const PbValue *args, PbValue *out) {
    if (!req_argc(vm, "type", argc, 1, 1)) return false;
    const char *t = "nil";
    switch (args[0].tag) {
        case VAL_NIL: t = "nil"; break;
        case VAL_BOOL: t = "bool"; break;
        case VAL_NUM: t = "number"; break;
        case VAL_BUILTIN: t = "fn"; break;
        case VAL_OBJ:
            switch (args[0].u.obj->kind) {
                case OBJ_STR: t = "string"; break;
                case OBJ_ARRAY: t = "array"; break;
                case OBJ_MAP: t = "map"; break;
                case OBJ_FUNC:
                case OBJ_CLOSURE: t = "fn"; break;
                case OBJ_UPVALUE: t = "upvalue"; break;
            }
            break;
    }
    *out = pb_obj_val((PbObj *)pb_new_cstr(vm, t));
    return true;
}

static bool b_to_string(PbVM *vm, int argc, const PbValue *args, PbValue *out) {
    if (!req_argc(vm, "to_string", argc, 1, 1)) return false;
    if (pb_is_obj_kind(args[0], OBJ_STR)) {
        *out = args[0];
        return true;
    }
    StrBuf sb;
    sb_init(&sb);
    pb_format_value_rec(&sb, args[0], 0);
    PbStr *s = pb_new_str(vm, sb.buf, sb.len);
    free(sb.buf);
    *out = pb_obj_val((PbObj *)s);
    return true;
}

static bool b_parse_number(PbVM *vm, int argc, const PbValue *args, PbValue *out) {
    if (!req_argc(vm, "parse_number", argc, 1, 1)) return false;
    if (args[0].tag == VAL_NUM) {
        *out = args[0];
        return true;
    }
    PbStr *s = NULL;
    if (!req_str(vm, "parse_number", args[0], &s)) return false;
    const char *p = s->data;
    while (*p && isspace((unsigned char)*p)) p++;
    if (*p == '\0') {
        *out = pb_nil();
        return true;
    }
    char *end = NULL;
    double v = strtod(p, &end);
    while (end && *end && isspace((unsigned char)*end)) end++;
    if (!end || *end != '\0') {
        *out = pb_nil();
        return true;
    }
    *out = pb_num(v);
    return true;
}

static bool b_num_floor(PbVM *vm, int argc, const PbValue *args, PbValue *out) {
    double x;
    if (!req_argc(vm, "num_floor", argc, 1, 1) || !req_num(vm, "num_floor", args[0], &x)) return false;
    *out = pb_num(floor(x));
    return true;
}

static bool b_num_ceil(PbVM *vm, int argc, const PbValue *args, PbValue *out) {
    double x;
    if (!req_argc(vm, "num_ceil", argc, 1, 1) || !req_num(vm, "num_ceil", args[0], &x)) return false;
    *out = pb_num(ceil(x));
    return true;
}

static bool b_num_sqrt(PbVM *vm, int argc, const PbValue *args, PbValue *out) {
    double x;
    if (!req_argc(vm, "num_sqrt", argc, 1, 1) || !req_num(vm, "num_sqrt", args[0], &x)) return false;
    *out = pb_num(sqrt(x));
    return true;
}

static bool b_num_abs(PbVM *vm, int argc, const PbValue *args, PbValue *out) {
    double x;
    if (!req_argc(vm, "num_abs", argc, 1, 1) || !req_num(vm, "num_abs", args[0], &x)) return false;
    *out = pb_num(fabs(x));
    return true;
}

static bool b_num_pow(PbVM *vm, int argc, const PbValue *args, PbValue *out) {
    double a, b;
    if (!req_argc(vm, "num_pow", argc, 2, 2) ||
        !req_num(vm, "num_pow", args[0], &a) ||
        !req_num(vm, "num_pow", args[1], &b)) return false;
    *out = pb_num(pow(a, b));
    return true;
}

static bool b_num_ln(PbVM *vm, int argc, const PbValue *args, PbValue *out) {
    double x;
    if (!req_argc(vm, "num_ln", argc, 1, 1) || !req_num(vm, "num_ln", args[0], &x)) return false;
    *out = pb_num(log(x));
    return true;
}

static bool b_num_exp(PbVM *vm, int argc, const PbValue *args, PbValue *out) {
    double x;
    if (!req_argc(vm, "num_exp", argc, 1, 1) || !req_num(vm, "num_exp", args[0], &x)) return false;
    *out = pb_num(exp(x));
    return true;
}

static bool b_bit_and(PbVM *vm, int argc, const PbValue *args, PbValue *out) {
    double a, b;
    if (!req_argc(vm, "bit_and", argc, 2, 2) ||
        !req_num(vm, "bit_and", args[0], &a) ||
        !req_num(vm, "bit_and", args[1], &b)) return false;
    *out = pb_num((double)((int64_t)a & (int64_t)b));
    return true;
}

static bool b_bit_or(PbVM *vm, int argc, const PbValue *args, PbValue *out) {
    double a, b;
    if (!req_argc(vm, "bit_or", argc, 2, 2) ||
        !req_num(vm, "bit_or", args[0], &a) ||
        !req_num(vm, "bit_or", args[1], &b)) return false;
    *out = pb_num((double)((int64_t)a | (int64_t)b));
    return true;
}

static bool b_bit_xor(PbVM *vm, int argc, const PbValue *args, PbValue *out) {
    double a, b;
    if (!req_argc(vm, "bit_xor", argc, 2, 2) ||
        !req_num(vm, "bit_xor", args[0], &a) ||
        !req_num(vm, "bit_xor", args[1], &b)) return false;
    *out = pb_num((double)((int64_t)a ^ (int64_t)b));
    return true;
}

static bool b_bit_not(PbVM *vm, int argc, const PbValue *args, PbValue *out) {
    double a;
    if (!req_argc(vm, "bit_not", argc, 1, 1) || !req_num(vm, "bit_not", args[0], &a)) return false;
    *out = pb_num((double)(~(int64_t)a));
    return true;
}

static bool b_bit_shl(PbVM *vm, int argc, const PbValue *args, PbValue *out) {
    double a, b;
    if (!req_argc(vm, "bit_shl", argc, 2, 2) ||
        !req_num(vm, "bit_shl", args[0], &a) ||
        !req_num(vm, "bit_shl", args[1], &b)) return false;
    uint32_t sh = ((uint32_t)(int64_t)b) & 63u;
    *out = pb_num((double)(int64_t)(((uint64_t)(int64_t)a) << sh));
    return true;
}

static bool b_bit_shr(PbVM *vm, int argc, const PbValue *args, PbValue *out) {
    double a, b;
    if (!req_argc(vm, "bit_shr", argc, 2, 2) ||
        !req_num(vm, "bit_shr", args[0], &a) ||
        !req_num(vm, "bit_shr", args[1], &b)) return false;
    uint32_t sh = ((uint32_t)(int64_t)b) & 63u;
    *out = pb_num((double)(((int64_t)a) >> sh));
    return true;
}

static bool b_str_split(PbVM *vm, int argc, const PbValue *args, PbValue *out) {
    PbStr *s = NULL, *delim = NULL;
    if (!req_argc(vm, "str_split", argc, 2, 2) ||
        !req_str(vm, "str_split", args[0], &s) ||
        !req_str(vm, "str_split", args[1], &delim)) return false;
    size_t saved = vm->temp_root_len;
    PbArray *arr = pb_new_array(vm, 8);
    vm_push_root(vm, (PbObj *)arr);
    if (delim->len == 0) {
        size_t n_cp = utf8_codepoint_count(s->data, s->len);
        for (size_t i = 0; i < n_cp; i++) {
            size_t b0 = utf8_char_to_byte_offset(s->data, s->len, i);
            size_t b1 = utf8_char_to_byte_offset(s->data, s->len, i + 1);
            PbStr *ch = pb_new_str(vm, s->data + b0, b1 - b0);
            pb_array_push(arr, pb_obj_val((PbObj *)ch));
        }
    } else {
        size_t pos = 0;
        while (pos <= s->len) {
            const char *found = NULL;
            for (size_t i = pos; i + delim->len <= s->len; i++) {
                if (memcmp(s->data + i, delim->data, delim->len) == 0) {
                    found = s->data + i;
                    break;
                }
            }
            if (!found) {
                PbStr *part = pb_new_str(vm, s->data + pos, s->len - pos);
                pb_array_push(arr, pb_obj_val((PbObj *)part));
                break;
            }
            size_t flen = (size_t)(found - (s->data + pos));
            PbStr *part = pb_new_str(vm, s->data + pos, flen);
            pb_array_push(arr, pb_obj_val((PbObj *)part));
            pos += flen + delim->len;
            if (pos == s->len) {
                PbStr *empty = pb_new_str(vm, "", 0);
                pb_array_push(arr, pb_obj_val((PbObj *)empty));
                break;
            }
        }
    }
    vm_pop_roots(vm, saved);
    *out = pb_obj_val((PbObj *)arr);
    return true;
}

static bool b_str_join(PbVM *vm, int argc, const PbValue *args, PbValue *out) {
    if (!req_argc(vm, "str_join", argc, 2, 2)) return false;
    if (!pb_is_obj_kind(args[0], OBJ_ARRAY)) {
        vm_set_error(vm, "str_join(): first argument must be array");
        return false;
    }
    PbStr *sep = NULL;
    if (!req_str(vm, "str_join", args[1], &sep)) return false;
    PbArray *arr = (PbArray *)args[0].u.obj;
    StrBuf sb;
    sb_init(&sb);
    for (size_t i = 0; i < arr->len; i++) {
        if (i > 0) sb_append_bytes(&sb, sep->data, sep->len);
        if (pb_is_obj_kind(arr->items[i], OBJ_STR)) {
            PbStr *elem = (PbStr *)arr->items[i].u.obj;
            sb_append_bytes(&sb, elem->data, elem->len);
        }
    }
    PbStr *res = pb_new_str(vm, sb.buf, sb.len);
    free(sb.buf);
    *out = pb_obj_val((PbObj *)res);
    return true;
}

static bool b_str_trim(PbVM *vm, int argc, const PbValue *args, PbValue *out) {
    PbStr *s = NULL;
    if (!req_argc(vm, "str_trim", argc, 1, 1) || !req_str(vm, "str_trim", args[0], &s)) return false;
    size_t start = 0;
    size_t end = s->len;
    while (start < end && isspace((unsigned char)s->data[start])) start++;
    while (end > start && isspace((unsigned char)s->data[end - 1])) end--;
    *out = pb_obj_val((PbObj *)pb_new_str(vm, s->data + start, end - start));
    return true;
}

static bool b_str_contains(PbVM *vm, int argc, const PbValue *args, PbValue *out) {
    PbStr *s = NULL, *sub = NULL;
    if (!req_argc(vm, "str_contains", argc, 2, 2) ||
        !req_str(vm, "str_contains", args[0], &s) ||
        !req_str(vm, "str_contains", args[1], &sub)) return false;
    if (sub->len == 0) {
        *out = pb_bool(true);
        return true;
    }
    if (sub->len > s->len) {
        *out = pb_bool(false);
        return true;
    }
    for (size_t i = 0; i + sub->len <= s->len; i++) {
        if (memcmp(s->data + i, sub->data, sub->len) == 0) {
            *out = pb_bool(true);
            return true;
        }
    }
    *out = pb_bool(false);
    return true;
}

static bool b_str_starts_with(PbVM *vm, int argc, const PbValue *args, PbValue *out) {
    PbStr *s = NULL, *pre = NULL;
    if (!req_argc(vm, "str_starts_with", argc, 2, 2) ||
        !req_str(vm, "str_starts_with", args[0], &s) ||
        !req_str(vm, "str_starts_with", args[1], &pre)) return false;
    *out = pb_bool(pre->len <= s->len && memcmp(s->data, pre->data, pre->len) == 0);
    return true;
}

static bool b_str_ends_with(PbVM *vm, int argc, const PbValue *args, PbValue *out) {
    PbStr *s = NULL, *suf = NULL;
    if (!req_argc(vm, "str_ends_with", argc, 2, 2) ||
        !req_str(vm, "str_ends_with", args[0], &s) ||
        !req_str(vm, "str_ends_with", args[1], &suf)) return false;
    *out = pb_bool(suf->len <= s->len && memcmp(s->data + (s->len - suf->len), suf->data, suf->len) == 0);
    return true;
}

static bool b_str_index_of(PbVM *vm, int argc, const PbValue *args, PbValue *out) {
    PbStr *s = NULL, *sub = NULL;
    if (!req_argc(vm, "str_index_of", argc, 2, 2) ||
        !req_str(vm, "str_index_of", args[0], &s) ||
        !req_str(vm, "str_index_of", args[1], &sub)) return false;
    if (sub->len == 0) {
        *out = pb_num(0.0);
        return true;
    }
    if (sub->len <= s->len) {
        for (size_t i = 0; i + sub->len <= s->len; i++) {
            if (memcmp(s->data + i, sub->data, sub->len) == 0) {
                *out = pb_num((double)utf8_codepoint_count(s->data, i));
                return true;
            }
        }
    }
    *out = pb_num(-1.0);
    return true;
}

static bool b_str_replace_all(PbVM *vm, int argc, const PbValue *args, PbValue *out) {
    PbStr *s = NULL, *pat = NULL, *rep = NULL;
    if (!req_argc(vm, "str_replace_all", argc, 3, 3) ||
        !req_str(vm, "str_replace_all", args[0], &s) ||
        !req_str(vm, "str_replace_all", args[1], &pat) ||
        !req_str(vm, "str_replace_all", args[2], &rep)) return false;
    if (pat->len == 0) {
        *out = args[0];
        return true;
    }
    StrBuf sb;
    sb_init(&sb);
    size_t i = 0;
    while (i < s->len) {
        if (i + pat->len <= s->len && memcmp(s->data + i, pat->data, pat->len) == 0) {
            sb_append_bytes(&sb, rep->data, rep->len);
            i += pat->len;
        } else {
            sb_append_char(&sb, s->data[i]);
            i++;
        }
    }
    PbStr *res = pb_new_str(vm, sb.buf, sb.len);
    free(sb.buf);
    *out = pb_obj_val((PbObj *)res);
    return true;
}

static bool b_str_slice(PbVM *vm, int argc, const PbValue *args, PbValue *out) {
    PbStr *s = NULL;
    double d_start, d_end;
    if (!req_argc(vm, "str_slice", argc, 3, 3) ||
        !req_str(vm, "str_slice", args[0], &s) ||
        !req_num(vm, "str_slice", args[1], &d_start) ||
        !req_num(vm, "str_slice", args[2], &d_end)) return false;
    size_t n_cp = utf8_codepoint_count(s->data, s->len);
    int64_t st = (int64_t)d_start;
    int64_t en = (int64_t)d_end;
    if (st < 0) st = 0;
    if (en < st) en = st;
    if ((size_t)st > n_cp) st = (int64_t)n_cp;
    if ((size_t)en > n_cp) en = (int64_t)n_cp;
    size_t b0 = utf8_char_to_byte_offset(s->data, s->len, (size_t)st);
    size_t b1 = utf8_char_to_byte_offset(s->data, s->len, (size_t)en);
    *out = pb_obj_val((PbObj *)pb_new_str(vm, s->data + b0, b1 - b0));
    return true;
}

static bool b_str_upper(PbVM *vm, int argc, const PbValue *args, PbValue *out) {
    PbStr *s = NULL;
    if (!req_argc(vm, "str_upper", argc, 1, 1) || !req_str(vm, "str_upper", args[0], &s)) return false;
    PbStr *r = pb_new_str(vm, s->data, s->len);
    for (uint32_t i = 0; i < r->len; i++) {
        if ((unsigned char)r->data[i] < 128) r->data[i] = (char)toupper((unsigned char)r->data[i]);
    }
    r->hash = fnv1a_bytes((const uint8_t *)r->data, r->len);
    *out = pb_obj_val((PbObj *)r);
    return true;
}

static bool b_str_lower(PbVM *vm, int argc, const PbValue *args, PbValue *out) {
    PbStr *s = NULL;
    if (!req_argc(vm, "str_lower", argc, 1, 1) || !req_str(vm, "str_lower", args[0], &s)) return false;
    PbStr *r = pb_new_str(vm, s->data, s->len);
    for (uint32_t i = 0; i < r->len; i++) {
        if ((unsigned char)r->data[i] < 128) r->data[i] = (char)tolower((unsigned char)r->data[i]);
    }
    r->hash = fnv1a_bytes((const uint8_t *)r->data, r->len);
    *out = pb_obj_val((PbObj *)r);
    return true;
}

static bool b_str_to_bytes(PbVM *vm, int argc, const PbValue *args, PbValue *out) {
    PbStr *s = NULL;
    if (!req_argc(vm, "str_to_bytes", argc, 1, 1) || !req_str(vm, "str_to_bytes", args[0], &s)) return false;
    PbArray *a = pb_new_array(vm, s->len);
    for (uint32_t i = 0; i < s->len; i++) {
        pb_array_push(a, pb_num((double)(uint8_t)s->data[i]));
    }
    *out = pb_obj_val((PbObj *)a);
    return true;
}

static bool b_bytes_to_str(PbVM *vm, int argc, const PbValue *args, PbValue *out) {
    if (!req_argc(vm, "bytes_to_str", argc, 1, 1) || !pb_is_obj_kind(args[0], OBJ_ARRAY)) {
        vm_set_error(vm, "bytes_to_str(): expected byte array");
        return false;
    }
    PbArray *a = (PbArray *)args[0].u.obj;
    char *buf = (char *)xmalloc(a->len ? a->len : 1);
    for (size_t i = 0; i < a->len; i++) {
        if (a->items[i].tag != VAL_NUM) {
            free(buf);
            vm_set_error(vm, "bytes_to_str(): array elements must be numbers");
            return false;
        }
        buf[i] = (char)((uint8_t)((int64_t)a->items[i].u.num & 0xFF));
    }
    PbStr *s = pb_new_str(vm, buf, a->len);
    free(buf);
    *out = pb_obj_val((PbObj *)s);
    return true;
}

static bool b_strcharcodeat(PbVM *vm, int argc, const PbValue *args, PbValue *out) {
    PbStr *s = NULL;
    double idx;
    if (!req_argc(vm, "strcharcodeat", argc, 2, 2) ||
        !req_str(vm, "strcharcodeat", args[0], &s) ||
        !req_num(vm, "strcharcodeat", args[1], &idx)) return false;
    int64_t i = (int64_t)idx;
    size_t n_cp = utf8_codepoint_count(s->data, s->len);
    if (i < 0 || (size_t)i >= n_cp) {
        *out = pb_nil();
        return true;
    }
    size_t boff = utf8_char_to_byte_offset(s->data, s->len, (size_t)i);
    *out = pb_num((double)utf8_decode_at(s->data, s->len, boff));
    return true;
}

static bool b_strfromcharcode(PbVM *vm, int argc, const PbValue *args, PbValue *out) {
    double code;
    if (!req_argc(vm, "strfromcharcode", argc, 1, 1) || !req_num(vm, "strfromcharcode", args[0], &code)) return false;
    char utf8[4];
    size_t n = utf8_encode_cp((uint32_t)(int64_t)code, utf8);
    *out = pb_obj_val((PbObj *)pb_new_str(vm, utf8, n));
    return true;
}

static bool b_array_push(PbVM *vm, int argc, const PbValue *args, PbValue *out) {
    if (!req_argc(vm, "array_push", argc, 2, 2) || !pb_is_obj_kind(args[0], OBJ_ARRAY)) {
        vm_set_error(vm, "array_push(): first argument must be an array");
        return false;
    }
    PbArray *a = (PbArray *)args[0].u.obj;
    pb_array_push(a, args[1]);
    *out = pb_num((double)a->len);
    return true;
}

static bool b_array_pop(PbVM *vm, int argc, const PbValue *args, PbValue *out) {
    if (!req_argc(vm, "array_pop", argc, 1, 1) || !pb_is_obj_kind(args[0], OBJ_ARRAY)) {
        vm_set_error(vm, "array_pop(): argument must be an array");
        return false;
    }
    PbArray *a = (PbArray *)args[0].u.obj;
    if (a->len == 0) {
        *out = pb_nil();
        return true;
    }
    *out = a->items[--a->len];
    return true;
}

static bool b_map_has(PbVM *vm, int argc, const PbValue *args, PbValue *out) {
    PbStr *k = NULL;
    if (!req_argc(vm, "map_has", argc, 2, 2)) return false;
    if (!pb_is_obj_kind(args[0], OBJ_MAP)) {
        vm_set_error(vm, "map_has(): first argument must be a map");
        return false;
    }
    if (!req_str(vm, "map_has", args[1], &k)) return false;
    *out = pb_bool(pb_map_get((PbMap *)args[0].u.obj, k, NULL));
    return true;
}

static bool b_map_keys(PbVM *vm, int argc, const PbValue *args, PbValue *out) {
    if (!req_argc(vm, "map_keys", argc, 1, 1) || !pb_is_obj_kind(args[0], OBJ_MAP)) {
        vm_set_error(vm, "map_keys(): argument must be a map");
        return false;
    }
    PbMap *m = (PbMap *)args[0].u.obj;
    PbArray *arr = pb_new_array(vm, m->count);
    for (size_t i = 0; i < m->order_len; i++) {
        if (m->entries[i].alive) {
            pb_array_push(arr, pb_obj_val((PbObj *)m->entries[i].key));
        }
    }
    *out = pb_obj_val((PbObj *)arr);
    return true;
}

static bool b_map_remove(PbVM *vm, int argc, const PbValue *args, PbValue *out) {
    PbStr *k = NULL;
    if (!req_argc(vm, "map_remove", argc, 2, 2)) return false;
    if (!pb_is_obj_kind(args[0], OBJ_MAP)) {
        vm_set_error(vm, "map_remove(): first argument must be a map");
        return false;
    }
    if (!req_str(vm, "map_remove", args[1], &k)) return false;
    PbValue rem = pb_nil();
    pb_map_remove((PbMap *)args[0].u.obj, k, &rem);
    *out = rem;
    return true;
}

static bool b_is_number(PbVM *vm, int argc, const PbValue *args, PbValue *out) {
    if (!req_argc(vm, "is_number", argc, 1, 1)) return false;
    *out = pb_bool(args[0].tag == VAL_NUM);
    return true;
}
static bool b_is_string(PbVM *vm, int argc, const PbValue *args, PbValue *out) {
    if (!req_argc(vm, "is_string", argc, 1, 1)) return false;
    *out = pb_bool(pb_is_obj_kind(args[0], OBJ_STR));
    return true;
}
static bool b_is_array(PbVM *vm, int argc, const PbValue *args, PbValue *out) {
    if (!req_argc(vm, "is_array", argc, 1, 1)) return false;
    *out = pb_bool(pb_is_obj_kind(args[0], OBJ_ARRAY));
    return true;
}
static bool b_is_map(PbVM *vm, int argc, const PbValue *args, PbValue *out) {
    if (!req_argc(vm, "is_map", argc, 1, 1)) return false;
    *out = pb_bool(pb_is_obj_kind(args[0], OBJ_MAP));
    return true;
}
static bool b_is_bool(PbVM *vm, int argc, const PbValue *args, PbValue *out) {
    if (!req_argc(vm, "is_bool", argc, 1, 1)) return false;
    *out = pb_bool(args[0].tag == VAL_BOOL);
    return true;
}
static bool b_is_nil(PbVM *vm, int argc, const PbValue *args, PbValue *out) {
    if (!req_argc(vm, "is_nil", argc, 1, 1)) return false;
    *out = pb_bool(args[0].tag == VAL_NIL);
    return true;
}
static bool b_is_fn(PbVM *vm, int argc, const PbValue *args, PbValue *out) {
    if (!req_argc(vm, "is_fn", argc, 1, 1)) return false;
    *out = pb_bool(args[0].tag == VAL_BUILTIN ||
                   pb_is_obj_kind(args[0], OBJ_FUNC) ||
                   pb_is_obj_kind(args[0], OBJ_CLOSURE));
    return true;
}

static bool b_assert(PbVM *vm, int argc, const PbValue *args, PbValue *out) {
    if (!req_argc(vm, "assert", argc, 1, 2)) return false;
    if (!pb_is_truthy(args[0])) {
        if (argc == 2 && pb_is_obj_kind(args[1], OBJ_STR)) {
            char buf[512];
            snprintf(buf, sizeof(buf), "AssertionError: %s", ((PbStr *)args[1].u.obj)->data);
            vm_set_error(vm, buf);
        } else {
            vm_set_error(vm, "AssertionError: assertion failed");
        }
        return false;
    }
    *out = pb_nil();
    return true;
}

static bool b_error(PbVM *vm, int argc, const PbValue *args, PbValue *out) {
    (void)out;
    if (argc == 0) {
        vm_set_error(vm, "error");
        return false;
    }
    StrBuf sb;
    sb_init(&sb);
    pb_format_value_rec(&sb, args[0], 0);
    vm_set_error(vm, sb.buf);
    free(sb.buf);
    return false;
}

static bool b_json_stringify(PbVM *vm, int argc, const PbValue *args, PbValue *out) {
    if (!req_argc(vm, "json_stringify", argc, 1, 1)) return false;
    StrBuf sb;
    sb_init(&sb);
    if (!pb_json_stringify_rec(vm, &sb, args[0], 0)) {
        free(sb.buf);
        return false;
    }
    PbStr *s = pb_new_str(vm, sb.buf, sb.len);
    free(sb.buf);
    *out = pb_obj_val((PbObj *)s);
    return true;
}

static bool b_fs_read(PbVM *vm, int argc, const PbValue *args, PbValue *out) {
    PbStr *path = NULL;
    if (!req_argc(vm, "fs_read", argc, 1, 1) || !req_str(vm, "fs_read", args[0], &path)) return false;
    FILE *f = fopen(path->data, "rb");
    if (!f) {
        char buf[512];
        snprintf(buf, sizeof(buf), "fs_read(): cannot open '%s': %s", path->data, strerror(errno));
        vm_set_error(vm, buf);
        return false;
    }
    if (fseek(f, 0, SEEK_END) != 0) {
        fclose(f);
        vm_set_error(vm, "fs_read(): fseek failed");
        return false;
    }
    long sz = ftell(f);
    if (sz < 0 || (uint64_t)sz > PBCK_MAX_BYTES) {
        fclose(f);
        vm_set_error(vm, "fs_read(): file size invalid or exceeds 64 MiB limit");
        return false;
    }
    rewind(f);
    char *buf = (char *)xmalloc((size_t)sz + 1);
    size_t n = sz > 0 ? fread(buf, 1, (size_t)sz, f) : 0;
    fclose(f);
    PbStr *s = pb_new_str(vm, buf, n);
    free(buf);
    *out = pb_obj_val((PbObj *)s);
    return true;
}

static bool b_fs_write(PbVM *vm, int argc, const PbValue *args, PbValue *out) {
    PbStr *path = NULL, *content = NULL;
    if (!req_argc(vm, "fs_write", argc, 2, 2) ||
        !req_str(vm, "fs_write", args[0], &path) ||
        !req_str(vm, "fs_write", args[1], &content)) return false;
    FILE *f = fopen(path->data, "wb");
    if (!f) {
        char buf[512];
        snprintf(buf, sizeof(buf), "fs_write(): cannot open '%s': %s", path->data, strerror(errno));
        vm_set_error(vm, buf);
        return false;
    }
    if (content->len > 0 && fwrite(content->data, 1, content->len, f) != content->len) {
        fclose(f);
        vm_set_error(vm, "fs_write(): short write");
        return false;
    }
    fclose(f);
    *out = pb_bool(true);
    return true;
}

static bool b_fs_exists(PbVM *vm, int argc, const PbValue *args, PbValue *out) {
    PbStr *path = NULL;
    if (!req_argc(vm, "fs_exists", argc, 1, 1) || !req_str(vm, "fs_exists", args[0], &path)) return false;
    struct stat st;
    *out = pb_bool(stat(path->data, &st) == 0);
    return true;
}

static bool b_env_get(PbVM *vm, int argc, const PbValue *args, PbValue *out) {
    PbStr *name = NULL;
    if (!req_argc(vm, "env_get", argc, 1, 1) || !req_str(vm, "env_get", args[0], &name)) return false;
    const char *v = getenv(name->data);
    if (!v) {
        *out = pb_nil();
    } else {
        *out = pb_obj_val((PbObj *)pb_new_cstr(vm, v));
    }
    return true;
}

static bool b_time_ms(PbVM *vm, int argc, const PbValue *args, PbValue *out) {
    (void)args;
    if (!req_argc(vm, "time_ms", argc, 0, 0)) return false;
    struct timespec ts;
    clock_gettime(CLOCK_REALTIME, &ts);
    double ms = (double)ts.tv_sec * 1000.0 + (double)ts.tv_nsec / 1000000.0;
    *out = pb_num(floor(ms));
    return true;
}

static bool b_sha256(PbVM *vm, int argc, const PbValue *args, PbValue *out) {
    if (!req_argc(vm, "sha256", argc, 1, 1)) return false;
    char hex[65];
    if (pb_is_obj_kind(args[0], OBJ_STR)) {
        PbStr *s = (PbStr *)args[0].u.obj;
        sha256_hex((const uint8_t *)s->data, s->len, hex);
    } else if (pb_is_obj_kind(args[0], OBJ_ARRAY)) {
        PbArray *a = (PbArray *)args[0].u.obj;
        uint8_t *buf = (uint8_t *)xmalloc(a->len ? a->len : 1);
        for (size_t i = 0; i < a->len; i++) {
            if (a->items[i].tag != VAL_NUM) {
                free(buf);
                vm_set_error(vm, "sha256(): byte array elements must be numbers");
                return false;
            }
            buf[i] = (uint8_t)((int64_t)a->items[i].u.num & 0xFF);
        }
        sha256_hex(buf, a->len, hex);
        free(buf);
    } else {
        vm_set_error(vm, "sha256(): expected string or byte array");
        return false;
    }
    *out = pb_obj_val((PbObj *)pb_new_str(vm, hex, 64));
    return true;
}

static bool b_args(PbVM *vm, int argc, const PbValue *args, PbValue *out) {
    (void)args;
    if (!req_argc(vm, "args", argc, 0, 0)) return false;
    size_t saved = vm->temp_root_len;
    PbArray *arr = pb_new_array(vm, (size_t)vm->script_argc);
    vm_push_root(vm, (PbObj *)arr);
    for (int i = 0; i < vm->script_argc; i++) {
        PbStr *s = pb_new_cstr(vm, vm->script_argv[i]);
        pb_array_push(arr, pb_obj_val((PbObj *)s));
    }
    vm_pop_roots(vm, saved);
    *out = pb_obj_val((PbObj *)arr);
    return true;
}

static bool b_gc_collect(PbVM *vm, int argc, const PbValue *args, PbValue *out) {
    (void)args;
    if (!req_argc(vm, "gc_collect", argc, 0, 0)) return false;
    size_t freed = vm_gc_collect(vm);
    char buf[160];
    snprintf(buf, sizeof(buf), "gc_collect: cycles arrays=%zu maps=0 modules=0", freed);
    *out = pb_obj_val((PbObj *)pb_new_cstr(vm, buf));
    return true;
}

static bool b_gc_stats(PbVM *vm, int argc, const PbValue *args, PbValue *out) {
    (void)args;
    if (!req_argc(vm, "gc_stats", argc, 0, 0)) return false;
    char buf[256];
    snprintf(buf, sizeof(buf),
             "string_allocs=%zu array_allocs=%zu map_allocs=%zu module_allocs=0 live_tracked=%zu collections=%zu",
             vm->total_str_allocs, vm->total_arr_allocs, vm->total_map_allocs,
             vm->num_objects, vm->gc_collections);
    *out = pb_obj_val((PbObj *)pb_new_cstr(vm, buf));
    return true;
}

static const PbBuiltinDef BUILTINS[] = {
    { "print",           b_print },
    { "println",         b_println },
    { "len",             b_len },
    { "type",            b_type },
    { "str",             b_to_string },
    { "to_string",       b_to_string },
    { "parse_number",    b_parse_number },
    { "num_floor",       b_num_floor },
    { "num_ceil",        b_num_ceil },
    { "num_sqrt",        b_num_sqrt },
    { "num_abs",         b_num_abs },
    { "num_pow",         b_num_pow },
    { "num_ln",          b_num_ln },
    { "num_exp",         b_num_exp },
    { "bit_and",         b_bit_and },
    { "bit_or",          b_bit_or },
    { "bit_xor",         b_bit_xor },
    { "bit_not",         b_bit_not },
    { "bit_shl",         b_bit_shl },
    { "bit_shr",         b_bit_shr },
    { "str_split",       b_str_split },
    { "str_join",        b_str_join },
    { "str_trim",        b_str_trim },
    { "str_contains",    b_str_contains },
    { "str_starts_with", b_str_starts_with },
    { "str_ends_with",   b_str_ends_with },
    { "str_index_of",    b_str_index_of },
    { "str_replace_all", b_str_replace_all },
    { "str_slice",       b_str_slice },
    { "str_upper",       b_str_upper },
    { "str_lower",       b_str_lower },
    { "str_to_bytes",    b_str_to_bytes },
    { "bytes_to_str",    b_bytes_to_str },
    { "strcharcodeat",   b_strcharcodeat },
    { "strfromcharcode", b_strfromcharcode },
    { "array_push",      b_array_push },
    { "push",            b_array_push },
    { "array_pop",       b_array_pop },
    { "pop",             b_array_pop },
    { "map_has",         b_map_has },
    { "map_keys",        b_map_keys },
    { "map_remove",      b_map_remove },
    { "is_number",       b_is_number },
    { "is_string",       b_is_string },
    { "is_array",        b_is_array },
    { "is_map",          b_is_map },
    { "is_bool",         b_is_bool },
    { "is_nil",          b_is_nil },
    { "is_fn",           b_is_fn },
    { "assert",          b_assert },
    { "error",           b_error },
    { "json_stringify",  b_json_stringify },
    { "fs_read",         b_fs_read },
    { "fs_write",        b_fs_write },
    { "fs_exists",       b_fs_exists },
    { "env_get",         b_env_get },
    { "time_ms",         b_time_ms },
    { "now_ms",          b_time_ms },
    { "sha256",          b_sha256 },
    { "args",            b_args },
    { "os_args",         b_args },
    { "gc_collect",      b_gc_collect },
    { "gc_stats",        b_gc_stats }
};

static const PbBuiltinDef *get_builtin_table(size_t *count_out) {
    *count_out = sizeof(BUILTINS) / sizeof(BUILTINS[0]);
    return BUILTINS;
}

/* ── Bounded `.pbck` v1 Binary Reader ────────────────────────────────────── */
typedef struct {
    const uint8_t *buf;
    size_t len;
    size_t pos;
    char err[256];
} PbckReader;

static bool r_need(PbckReader *r, size_t n) {
    if (n > r->len - r->pos) {
        snprintf(r->err, sizeof(r->err), "pbck: unexpected EOF");
        return false;
    }
    return true;
}

static bool r_u8(PbckReader *r, uint8_t *out) {
    if (!r_need(r, 1)) return false;
    *out = r->buf[r->pos++];
    return true;
}

static bool r_flag(PbckReader *r, const char *what, bool *out) {
    uint8_t b;
    if (!r_u8(r, &b)) return false;
    if (b == 0) { *out = false; return true; }
    if (b == 1) { *out = true; return true; }
    snprintf(r->err, sizeof(r->err), "pbck: invalid %s flag %u (expected 0 or 1)", what, (unsigned)b);
    return false;
}

static bool r_u32(PbckReader *r, uint32_t *out) {
    if (!r_need(r, 4)) return false;
    const uint8_t *p = r->buf + r->pos;
    *out = (uint32_t)p[0] |
           ((uint32_t)p[1] << 8) |
           ((uint32_t)p[2] << 16) |
           ((uint32_t)p[3] << 24);
    r->pos += 4;
    return true;
}

static bool r_f64(PbckReader *r, double *out) {
    if (!r_need(r, 8)) return false;
    uint64_t bits = 0;
    for (int i = 0; i < 8; i++) {
        bits |= ((uint64_t)r->buf[r->pos + i]) << (i * 8);
    }
    r->pos += 8;
    memcpy(out, &bits, sizeof(double));
    return true;
}

static bool r_str(PbckReader *r, PbVM *vm, PbStr **out) {
    uint32_t len;
    if (!r_u32(r, &len)) return false;
    if (!r_need(r, (size_t)len)) return false;
    *out = pb_new_str(vm, (const char *)(r->buf + r->pos), (size_t)len);
    r->pos += (size_t)len;
    return true;
}

static bool pbck_read_chunk(PbckReader *r, PbVM *vm, PbChunk *chunk, uint32_t depth);
static bool pbck_read_func(PbckReader *r, PbVM *vm, PbFunc **out_fn, uint32_t depth);

static bool pbck_read_opcode(PbckReader *r, PbInstr *ins) {
    memset(ins, 0, sizeof(*ins));
    uint8_t tag;
    if (!r_u8(r, &tag)) return false;
    ins->op = tag;
    switch (tag) {
        case OP_CONSTANT:
        case OP_DEFINE_GLOBAL:
        case OP_GET_GLOBAL:
        case OP_SET_GLOBAL:
        case OP_GET_ATTR:
        case OP_SET_ATTR:
        case OP_EXPORT:
        case OP_MAKE_ARRAY:
        case OP_MAKE_MAP:
        case OP_TRY:
        case OP_GET_LOCAL:
        case OP_SET_LOCAL:
        case OP_GET_UPVALUE:
        case OP_SET_UPVALUE:
        case OP_CALL:
        case OP_TAIL_CALL:
        case OP_JUMP:
        case OP_JUMP_IF_FALSE:
        case OP_JUMP_IF_TRUE:
        case OP_SAFE_GET_ATTR:
            return r_u32(r, &ins->arg);
        case OP_CLOSURE: {
            if (!r_u32(r, &ins->arg)) return false;
            uint32_t n_caps;
            if (!r_u32(r, &n_caps)) return false;
            if ((size_t)n_caps * 5u > r->len - r->pos) {
                snprintf(r->err, sizeof(r->err), "pbck: truncated closure captures");
                return false;
            }
            ins->n_caps = n_caps;
            ins->caps = n_caps > 0 ? (PbCaptureSpec *)xcalloc(n_caps, sizeof(PbCaptureSpec)) : NULL;
            for (uint32_t i = 0; i < n_caps; i++) {
                bool is_loc = false;
                uint32_t idx = 0;
                if (!r_flag(r, "closure capture", &is_loc) || !r_u32(r, &idx)) {
                    free(ins->caps);
                    ins->caps = NULL;
                    return false;
                }
                ins->caps[i].is_local = is_loc ? 1u : 0u;
                ins->caps[i].idx = idx;
            }
            return true;
        }
        case OP_ADD: case OP_SUB: case OP_MUL: case OP_DIV:
        case OP_EQ: case OP_NOT_EQ: case OP_GT: case OP_GE: case OP_LT: case OP_LE:
        case OP_POP: case OP_PRINT: case OP_GET_INDEX: case OP_SET_INDEX:
        case OP_END_TRY: case OP_CLOSE_UPVALUE: case OP_RETURN: case OP_DEFER:
        case OP_NOT: case OP_NEGATE: case OP_MOD: case OP_POW:
        case OP_SAFE_INDEX: case OP_RESULT_UNWRAP: case OP_IS_NIL:
        case OP_DUP: case OP_THROW:
            return true;
        default:
            snprintf(r->err, sizeof(r->err), "pbck: unknown opcode tag %u", (unsigned)tag);
            return false;
    }
}

static bool pbck_read_value(PbckReader *r, PbVM *vm, PbValue *out, uint32_t depth) {
    uint8_t tag;
    if (!r_u8(r, &tag)) return false;
    switch (tag) {
        case 0: {
            double d;
            if (!r_f64(r, &d)) return false;
            *out = pb_num(d);
            return true;
        }
        case 1: {
            bool b;
            if (!r_flag(r, "boolean", &b)) return false;
            *out = pb_bool(b);
            return true;
        }
        case 2:
            *out = pb_nil();
            return true;
        case 3: {
            PbStr *s = NULL;
            if (!r_str(r, vm, &s)) return false;
            *out = pb_obj_val((PbObj *)s);
            return true;
        }
        case 4: {
            PbStr *s = NULL;
            if (!r_str(r, vm, &s)) return false;
            *out = pb_num(strtod(s->data, NULL));
            return true;
        }
        case 5: {
            PbStr *s = NULL;
            if (!r_str(r, vm, &s)) return false;
            const char *slash = strchr(s->data, '/');
            if (slash) {
                double num = strtod(s->data, NULL);
                double den = strtod(slash + 1, NULL);
                *out = pb_num(den != 0.0 ? num / den : 0.0);
            } else {
                *out = pb_num(strtod(s->data, NULL));
            }
            return true;
        }
        case 7: {
            PbFunc *fn = NULL;
            if (!pbck_read_func(r, vm, &fn, depth)) return false;
            *out = pb_obj_val((PbObj *)fn);
            return true;
        }
        default:
            snprintf(r->err, sizeof(r->err), "pbck: unsupported constant value tag %u", (unsigned)tag);
            return false;
    }
}

static bool pbck_read_func(PbckReader *r, PbVM *vm, PbFunc **out_fn, uint32_t depth) {
    if (depth >= PBCK_MAX_DEPTH) {
        snprintf(r->err, sizeof(r->err), "pbck: function nesting exceeds %u", PBCK_MAX_DEPTH);
        return false;
    }
    PbFunc *fn = (PbFunc *)alloc_obj(vm, sizeof(PbFunc), OBJ_FUNC);
    bool has_name = false;
    if (!r_flag(r, "function-name", &has_name)) return false;
    if (has_name) {
        if (!r_str(r, vm, &fn->name)) return false;
    }
    if (!r_u32(r, &fn->arity) || !r_u32(r, &fn->min_arity)) return false;
    uint32_t n_def = 0;
    if (!r_u32(r, &n_def)) return false;
    if ((size_t)n_def > r->len - r->pos) {
        snprintf(r->err, sizeof(r->err), "pbck: truncated function defaults");
        return false;
    }
    fn->n_defaults = n_def;
    fn->defaults = n_def > 0 ? (PbValue *)xcalloc(n_def, sizeof(PbValue)) : NULL;
    for (uint32_t i = 0; i < n_def; i++) {
        if (!pbck_read_value(r, vm, &fn->defaults[i], depth + 1)) return false;
    }
    if (!r_flag(r, "vararg", &fn->is_vararg)) return false;
    bool has_pt = false;
    if (!r_flag(r, "parameter-types", &has_pt)) return false;
    if (has_pt) {
        uint32_t n_pt = 0;
        if (!r_u32(r, &n_pt)) return false;
        if (!r_need(r, n_pt)) return false;
        for (uint32_t i = 0; i < n_pt; i++) {
            uint8_t pt = 0;
            if (!r_u8(r, &pt) || pt > 2) {
                snprintf(r->err, sizeof(r->err), "pbck: invalid param type %u", (unsigned)pt);
                return false;
            }
        }
    }
    if (!pbck_read_chunk(r, vm, &fn->chunk, depth + 1)) return false;
    *out_fn = fn;
    return true;
}

static bool pbck_read_chunk(PbckReader *r, PbVM *vm, PbChunk *chunk, uint32_t depth) {
    memset(chunk, 0, sizeof(*chunk));
    uint32_t n_ins = 0;
    if (!r_u32(r, &n_ins)) return false;
    if ((size_t)n_ins > r->len - r->pos) {
        snprintf(r->err, sizeof(r->err), "pbck: truncated instructions");
        return false;
    }
    chunk->code_len = n_ins;
    chunk->code = n_ins > 0 ? (PbInstr *)xcalloc(n_ins, sizeof(PbInstr)) : NULL;
    for (uint32_t i = 0; i < n_ins; i++) {
        if (!pbck_read_opcode(r, &chunk->code[i])) return false;
    }

    uint32_t n_const = 0;
    if (!r_u32(r, &n_const)) return false;
    if ((size_t)n_const > r->len - r->pos) {
        snprintf(r->err, sizeof(r->err), "pbck: truncated constants");
        return false;
    }
    chunk->const_len = n_const;
    chunk->consts = n_const > 0 ? (PbValue *)xcalloc(n_const, sizeof(PbValue)) : NULL;
    for (uint32_t i = 0; i < n_const; i++) {
        if (!pbck_read_value(r, vm, &chunk->consts[i], depth)) return false;
    }

    uint32_t n_spans = 0;
    if (!r_u32(r, &n_spans)) return false;
    if ((size_t)n_spans * 8u > r->len - r->pos) {
        snprintf(r->err, sizeof(r->err), "pbck: truncated spans");
        return false;
    }
    r->pos += (size_t)n_spans * 8u;

    bool has_src = false;
    if (!r_flag(r, "source-name", &has_src)) return false;
    if (has_src) {
        if (!r_str(r, vm, &chunk->source_name)) return false;
    }
    return true;
}

static PbFunc *pbck_deserialize_script(PbVM *vm, const uint8_t *buf, size_t len, char err_out[256]) {
    if (len > PBCK_MAX_BYTES) {
        snprintf(err_out, 256, "pbck: artifact byte limit exceeded");
        return NULL;
    }
    if (len < 5 || memcmp(buf, "PBCK", 4) != 0) {
        snprintf(err_out, 256, "pbck: bad magic (not a PBCK file)");
        return NULL;
    }
    if (buf[4] != PBCK_V1_VERSION) {
        snprintf(err_out, 256, "pbck: unsupported version %u (expected v1)", (unsigned)buf[4]);
        return NULL;
    }
    PbckReader r;
    r.buf = buf;
    r.len = len;
    r.pos = 5;
    r.err[0] = '\0';

    vm->gc_inhibit++;
    PbFunc *script_fn = (PbFunc *)alloc_obj(vm, sizeof(PbFunc), OBJ_FUNC);
    script_fn->name = pb_new_cstr(vm, "<script>");
    bool ok = pbck_read_chunk(&r, vm, &script_fn->chunk, 0);
    vm->gc_inhibit--;

    if (!ok) {
        snprintf(err_out, 256, "%s", r.err[0] ? r.err : "pbck: malformed container");
        return NULL;
    }
    if (r.pos != r.len) {
        snprintf(err_out, 256, "pbck: trailing bytes");
        return NULL;
    }
    return script_fn;
}

/* ── VM Stack, Call, Unwind & Bytecode Dispatch ──────────────────────────── */
static void vm_init(PbVM *vm, int script_argc, char **script_argv) {
    memset(vm, 0, sizeof(*vm));
    vm->stack_cap = VM_INITIAL_STACK;
    vm->stack = (PbValue *)xcalloc(vm->stack_cap, sizeof(PbValue));
    vm->next_gc = 16384;
    vm->script_argc = script_argc;
    vm->script_argv = script_argv;

    vm->gc_inhibit++;
    vm->globals = pb_new_map(vm, 128);
    vm->exports = pb_new_map(vm, 32);
    size_t n_b = 0;
    const PbBuiltinDef *tbl = get_builtin_table(&n_b);
    for (size_t i = 0; i < n_b; i++) {
        pb_map_set(vm->globals, pb_new_cstr(vm, tbl[i].name), pb_builtin_val((int)i));
    }
    vm->gc_inhibit--;
}

static void vm_destroy(PbVM *vm) {
    for (size_t i = 0; i < vm->frame_len; i++) {
        free(vm->frames[i].defers);
    }
    PbObj *curr = vm->objects;
    while (curr != NULL) {
        PbObj *next = curr->next;
        free_obj(curr);
        curr = next;
    }
    free(vm->stack);
}

static inline bool vm_push(PbVM *vm, PbValue v) {
    if (vm->stack_len >= vm->stack_cap) {
        if (vm->stack_cap >= VM_MAX_STACK) {
            vm_set_error(vm, "VM value stack overflow");
            return false;
        }
        size_t ncap = vm->stack_cap * 2;
        if (ncap > VM_MAX_STACK) ncap = VM_MAX_STACK;
        vm->stack = (PbValue *)xrealloc(vm->stack, ncap * sizeof(PbValue));
        vm->stack_cap = ncap;
    }
    vm->stack[vm->stack_len++] = v;
    return true;
}

static inline bool vm_pop(PbVM *vm, PbValue *out) {
    if (vm->stack_len == 0) {
        vm_set_error(vm, "stack underflow");
        return false;
    }
    *out = vm->stack[--vm->stack_len];
    return true;
}

static bool vm_prepare_closure_args(PbVM *vm, PbFunc *fn, uint32_t argc, size_t callee_slot) {
    if (fn->is_vararg) {
        uint32_t fixed = fn->arity > 0 ? fn->arity - 1 : 0;
        if (argc < fn->min_arity && argc < fixed) {
            vm_set_error(vm, "too few arguments for variadic function");
            return false;
        }
        while (argc < fixed) {
            uint32_t def_i = argc - fn->min_arity;
            PbValue dv = (def_i < fn->n_defaults) ? fn->defaults[def_i] : pb_nil();
            if (!vm_push(vm, dv)) return false;
            argc++;
        }
        uint32_t rest_n = argc - fixed;
        size_t saved = vm->temp_root_len;
        PbArray *rest = pb_new_array(vm, rest_n);
        vm_push_root(vm, (PbObj *)rest);
        size_t rest_start = callee_slot + 1 + fixed;
        for (uint32_t i = 0; i < rest_n; i++) {
            pb_array_push(rest, vm->stack[rest_start + i]);
        }
        vm->stack_len = rest_start;
        vm_pop_roots(vm, saved);
        return vm_push(vm, pb_obj_val((PbObj *)rest));
    }

    if (argc < fn->min_arity || argc > fn->arity) {
        char buf[160];
        snprintf(buf, sizeof(buf), "function '%s' expected %u..%u args, got %u",
                 fn->name ? fn->name->data : "<anon>", fn->min_arity, fn->arity, argc);
        vm_set_error(vm, buf);
        return false;
    }
    while (argc < fn->arity) {
        uint32_t def_i = argc - fn->min_arity;
        PbValue dv = (def_i < fn->n_defaults) ? fn->defaults[def_i] : pb_nil();
        if (!vm_push(vm, dv)) return false;
        argc++;
    }
    return true;
}

static bool vm_call_value(PbVM *vm, uint32_t argc, bool is_tail) {
    if (vm->stack_len < (size_t)argc + 1) {
        vm_set_error(vm, "stack underflow on call");
        return false;
    }
    size_t callee_slot = vm->stack_len - 1 - (size_t)argc;
    PbValue callee = vm->stack[callee_slot];

    if (callee.tag == VAL_BUILTIN) {
        size_t n_b = 0;
        const PbBuiltinDef *tbl = get_builtin_table(&n_b);
        if (callee.u.builtin_id < 0 || (size_t)callee.u.builtin_id >= n_b) {
            vm_set_error(vm, "invalid builtin id");
            return false;
        }
        PbValue res = pb_nil();
        bool ok = tbl[callee.u.builtin_id].fn(vm, (int)argc, &vm->stack[callee_slot + 1], &res);
        if (!ok) return false;
        vm->stack_len = callee_slot;
        if (!vm_push(vm, res)) return false;
        if (is_tail && vm->frame_len > 0) {
            PbFrame *cur = &vm->frames[vm->frame_len - 1];
            vm_close_upvalues_from(vm, cur->stack_base + 1);
            free(cur->defers);
            cur->defers = NULL;
            cur->defer_len = cur->defer_cap = 0;
            size_t base = cur->stack_base;
            vm->frame_len--;
            vm->stack_len = base;
            return vm_push(vm, res);
        }
        return true;
    }

    PbClosure *cl = NULL;
    if (pb_is_obj_kind(callee, OBJ_CLOSURE)) {
        cl = (PbClosure *)callee.u.obj;
    } else if (pb_is_obj_kind(callee, OBJ_FUNC)) {
        cl = pb_new_closure(vm, (PbFunc *)callee.u.obj, 0);
        vm->stack[callee_slot] = pb_obj_val((PbObj *)cl);
    } else {
        vm_set_error(vm, "attempted to call non-callable value");
        return false;
    }

    if (!vm_prepare_closure_args(vm, cl->func, argc, callee_slot)) {
        return false;
    }

    if (is_tail && vm->frame_len > 0 && vm->frames[vm->frame_len - 1].defer_len == 0) {
        bool handler_in_frame = false;
        for (size_t h = 0; h < vm->handler_len; h++) {
            if (vm->handlers[h].frame_idx == vm->frame_len - 1) {
                handler_in_frame = true;
                break;
            }
        }
        if (!handler_in_frame) {
            PbFrame *cur = &vm->frames[vm->frame_len - 1];
            vm_close_upvalues_from(vm, cur->stack_base + 1);
            size_t new_count = vm->stack_len - callee_slot;
            memmove(&vm->stack[cur->stack_base], &vm->stack[callee_slot], new_count * sizeof(PbValue));
            vm->stack_len = cur->stack_base + new_count;
            cur->closure = cl;
            cur->ip = 0;
            return true;
        }
    }

    if (vm->frame_len >= VM_MAX_FRAMES) {
        vm_set_error(vm, "call stack overflow");
        return false;
    }
    PbFrame *fr = &vm->frames[vm->frame_len++];
    fr->closure = cl;
    fr->ip = 0;
    fr->stack_base = callee_slot;
    fr->defers = NULL;
    fr->defer_len = 0;
    fr->defer_cap = 0;
    return true;
}

static bool vm_unwind_error(PbVM *vm) {
    if (vm->handler_len == 0) {
        return false;
    }
    PbTryHandler h = vm->handlers[--vm->handler_len];
    while (vm->frame_len > h.frame_idx + 1) {
        PbFrame *top = &vm->frames[--vm->frame_len];
        free(top->defers);
        top->defers = NULL;
    }
    vm_close_upvalues_from(vm, h.stack_len);
    if (vm->stack_len > h.stack_len) {
        vm->stack_len = h.stack_len;
    }
    vm->frames[h.frame_idx].ip = h.target_ip;
    PbStr *emsg = pb_new_cstr(vm, vm->error_msg);
    vm->has_error = false;
    vm->error_msg[0] = '\0';
    return vm_push(vm, pb_obj_val((PbObj *)emsg));
}

static bool vm_exec_from_frame(PbVM *vm, size_t stop_below_frame);

static void vm_run_frame_defers(PbVM *vm, PbFrame *fr) {
    while (fr->defer_len > 0) {
        PbValue d = fr->defers[--fr->defer_len];
        size_t saved_stack = vm->stack_len;
        size_t target_frame_len = vm->frame_len;
        if (vm_push(vm, d) && vm_call_value(vm, 0, false)) {
            if (vm->frame_len > target_frame_len) {
                (void)vm_exec_from_frame(vm, target_frame_len);
            }
        }
        vm->has_error = false;
        vm->stack_len = saved_stack;
    }
    free(fr->defers);
    fr->defers = NULL;
    fr->defer_cap = 0;
}

static const PbStr *chunk_const_str(PbVM *vm, const PbChunk *c, uint32_t idx) {
    if (idx >= c->const_len || !pb_is_obj_kind(c->consts[idx], OBJ_STR)) {
        vm_set_error(vm, "invalid string constant index");
        return NULL;
    }
    return (const PbStr *)c->consts[idx].u.obj;
}

static bool vm_exec_from_frame(PbVM *vm, size_t stop_below_frame) {
    for (;;) {
        if (vm->frame_len <= stop_below_frame) {
            return true;
        }
        PbFrame *frame = &vm->frames[vm->frame_len - 1];
        const PbChunk *chunk = &frame->closure->func->chunk;

        if (frame->ip >= chunk->code_len) {
            /* Implicit nil return at end of chunk */
            vm_run_frame_defers(vm, frame);
            size_t base = frame->stack_base;
            vm_close_upvalues_from(vm, base + 1);
            vm->frame_len--;
            vm->stack_len = base;
            if (!vm_push(vm, pb_nil())) return false;
            continue;
        }

        const PbInstr *ins = &chunk->code[frame->ip++];
        bool ok = true;

        switch (ins->op) {
            case OP_CONSTANT: {
                if (ins->arg >= chunk->const_len) {
                    vm_set_error(vm, "constant index out of bounds");
                    ok = false;
                } else {
                    ok = vm_push(vm, chunk->consts[ins->arg]);
                }
                break;
            }
            case OP_ADD: {
                if (vm->stack_len >= 2 &&
                    vm->stack[vm->stack_len - 2].tag == VAL_NUM &&
                    vm->stack[vm->stack_len - 1].tag == VAL_NUM) {
                    vm->stack[vm->stack_len - 2].u.num += vm->stack[vm->stack_len - 1].u.num;
                    vm->stack_len--;
                } else {
                    PbValue b, a;
                    if (!vm_pop(vm, &b) || !vm_pop(vm, &a)) { ok = false; break; }
                    if (pb_is_obj_kind(a, OBJ_STR) && pb_is_obj_kind(b, OBJ_STR)) {
                        size_t saved = vm->temp_root_len;
                        vm_push_root(vm, a.u.obj);
                        vm_push_root(vm, b.u.obj);
                        PbStr *sa = (PbStr *)a.u.obj;
                        PbStr *sb = (PbStr *)b.u.obj;
                        PbStr *out = (PbStr *)alloc_obj(vm, sizeof(PbStr) + sa->len + sb->len + 1, OBJ_STR);
                        out->len = sa->len + sb->len;
                        memcpy(out->data, sa->data, sa->len);
                        memcpy(out->data + sa->len, sb->data, sb->len);
                        out->data[out->len] = '\0';
                        out->hash = fnv1a_bytes((const uint8_t *)out->data, out->len);
                        vm->total_str_allocs++;
                        vm_pop_roots(vm, saved);
                        ok = vm_push(vm, pb_obj_val((PbObj *)out));
                    } else {
                        vm_set_error(vm, "type error: + expects (number, number) or (string, string)");
                        ok = false;
                    }
                }
                break;
            }
            case OP_SUB:
            case OP_MUL:
            case OP_DIV:
            case OP_MOD:
            case OP_POW: {
                if (vm->stack_len >= 2 &&
                    vm->stack[vm->stack_len - 2].tag == VAL_NUM &&
                    vm->stack[vm->stack_len - 1].tag == VAL_NUM) {
                    double a = vm->stack[vm->stack_len - 2].u.num;
                    double b = vm->stack[vm->stack_len - 1].u.num;
                    double r = 0.0;
                    if (ins->op == OP_SUB) r = a - b;
                    else if (ins->op == OP_MUL) r = a * b;
                    else if (ins->op == OP_DIV) r = a / b;
                    else if (ins->op == OP_MOD) r = fmod(a, b);
                    else r = pow(a, b);
                    vm->stack[vm->stack_len - 2].u.num = r;
                    vm->stack_len--;
                } else {
                    PbValue b, a;
                    (void)vm_pop(vm, &b);
                    (void)vm_pop(vm, &a);
                    vm_set_error(vm, "type error: arithmetic expects numbers");
                    ok = false;
                }
                break;
            }
            case OP_EQ:
            case OP_NOT_EQ: {
                PbValue b, a;
                if (!vm_pop(vm, &b) || !vm_pop(vm, &a)) { ok = false; break; }
                bool eq = pb_val_equal(a, b);
                ok = vm_push(vm, pb_bool(ins->op == OP_EQ ? eq : !eq));
                break;
            }
            case OP_GT:
            case OP_GE:
            case OP_LT:
            case OP_LE: {
                PbValue b, a;
                if (!vm_pop(vm, &b) || !vm_pop(vm, &a)) { ok = false; break; }
                if (a.tag == VAL_NUM && b.tag == VAL_NUM) {
                    bool r = false;
                    if (ins->op == OP_GT) r = a.u.num > b.u.num;
                    else if (ins->op == OP_GE) r = a.u.num >= b.u.num;
                    else if (ins->op == OP_LT) r = a.u.num < b.u.num;
                    else r = a.u.num <= b.u.num;
                    ok = vm_push(vm, pb_bool(r));
                } else if (pb_is_obj_kind(a, OBJ_STR) && pb_is_obj_kind(b, OBJ_STR)) {
                    PbStr *sa = (PbStr *)a.u.obj;
                    PbStr *sb = (PbStr *)b.u.obj;
                    size_t ml = sa->len < sb->len ? sa->len : sb->len;
                    int cmp = memcmp(sa->data, sb->data, ml);
                    if (cmp == 0) cmp = (sa->len < sb->len) ? -1 : (sa->len > sb->len ? 1 : 0);
                    bool r = false;
                    if (ins->op == OP_GT) r = cmp > 0;
                    else if (ins->op == OP_GE) r = cmp >= 0;
                    else if (ins->op == OP_LT) r = cmp < 0;
                    else r = cmp <= 0;
                    ok = vm_push(vm, pb_bool(r));
                } else {
                    vm_set_error(vm, "type error: ordering comparison requires numbers or strings");
                    ok = false;
                }
                break;
            }
            case OP_POP: {
                PbValue v;
                ok = vm_pop(vm, &v);
                break;
            }
            case OP_PRINT: {
                PbValue v;
                if (!vm_pop(vm, &v)) { ok = false; break; }
                StrBuf sb;
                sb_init(&sb);
                pb_format_value_rec(&sb, v, 0);
                fwrite(sb.buf, 1, sb.len, stdout);
                fputc('\n', stdout);
                fflush(stdout);
                free(sb.buf);
                break;
            }
            case OP_DEFINE_GLOBAL: {
                PbValue v;
                if (!vm_pop(vm, &v)) { ok = false; break; }
                const PbStr *name = chunk_const_str(vm, chunk, ins->arg);
                if (!name) { ok = false; break; }
                pb_map_set(vm->globals, (PbStr *)name, v);
                break;
            }
            case OP_GET_GLOBAL: {
                const PbStr *name = chunk_const_str(vm, chunk, ins->arg);
                if (!name) { ok = false; break; }
                PbValue v;
                if (!pb_map_get(vm->globals, name, &v)) {
                    char buf[256];
                    snprintf(buf, sizeof(buf), "undefined global '%s'", name->data);
                    vm_set_error(vm, buf);
                    ok = false;
                } else {
                    ok = vm_push(vm, v);
                }
                break;
            }
            case OP_SET_GLOBAL: {
                PbValue v;
                if (!vm_pop(vm, &v)) { ok = false; break; }
                const PbStr *name = chunk_const_str(vm, chunk, ins->arg);
                if (!name) { ok = false; break; }
                if (!pb_map_get(vm->globals, name, NULL)) {
                    char buf[256];
                    snprintf(buf, sizeof(buf), "undefined global '%s'", name->data);
                    vm_set_error(vm, buf);
                    ok = false;
                } else {
                    pb_map_set(vm->globals, (PbStr *)name, v);
                }
                break;
            }
            case OP_GET_ATTR: {
                PbValue obj;
                if (!vm_pop(vm, &obj)) { ok = false; break; }
                const PbStr *attr = chunk_const_str(vm, chunk, ins->arg);
                if (!attr) { ok = false; break; }
                if (!pb_is_obj_kind(obj, OBJ_MAP)) {
                    vm_set_error(vm, "member access only supported on maps");
                    ok = false;
                    break;
                }
                PbValue v;
                if (!pb_map_get((PbMap *)obj.u.obj, attr, &v)) {
                    char buf[256];
                    snprintf(buf, sizeof(buf), "undefined key '%s'", attr->data);
                    vm_set_error(vm, buf);
                    ok = false;
                } else {
                    ok = vm_push(vm, v);
                }
                break;
            }
            case OP_SET_ATTR: {
                PbValue val, obj;
                if (!vm_pop(vm, &val) || !vm_pop(vm, &obj)) { ok = false; break; }
                const PbStr *attr = chunk_const_str(vm, chunk, ins->arg);
                if (!attr) { ok = false; break; }
                if (!pb_is_obj_kind(obj, OBJ_MAP)) {
                    vm_set_error(vm, "attr set only supported on maps");
                    ok = false;
                    break;
                }
                pb_map_set((PbMap *)obj.u.obj, (PbStr *)attr, val);
                ok = vm_push(vm, val);
                break;
            }
            case OP_EXPORT: {
                const PbStr *name = chunk_const_str(vm, chunk, ins->arg);
                if (!name) { ok = false; break; }
                PbValue v;
                if (!pb_map_get(vm->globals, name, &v)) {
                    char buf[256];
                    snprintf(buf, sizeof(buf), "cannot export undefined '%s'", name->data);
                    vm_set_error(vm, buf);
                    ok = false;
                } else {
                    pb_map_set(vm->exports, (PbStr *)name, v);
                }
                break;
            }
            case OP_MAKE_ARRAY: {
                uint32_t n = ins->arg;
                if (vm->stack_len < n) {
                    vm_set_error(vm, "stack underflow (make array)");
                    ok = false;
                    break;
                }
                PbArray *arr = pb_new_array(vm, n);
                size_t start = vm->stack_len - n;
                for (uint32_t i = 0; i < n; i++) {
                    pb_array_push(arr, vm->stack[start + i]);
                }
                vm->stack_len = start;
                ok = vm_push(vm, pb_obj_val((PbObj *)arr));
                break;
            }
            case OP_MAKE_MAP: {
                uint32_t n = ins->arg;
                size_t need = (size_t)n * 2;
                if (vm->stack_len < need) {
                    vm_set_error(vm, "stack underflow (make map)");
                    ok = false;
                    break;
                }
                PbMap *m = pb_new_map(vm, n < 4 ? 4 : n);
                size_t start = vm->stack_len - need;
                for (uint32_t i = 0; i < n; i++) {
                    PbValue kv = vm->stack[start + (size_t)i * 2];
                    PbValue vv = vm->stack[start + (size_t)i * 2 + 1];
                    if (!pb_is_obj_kind(kv, OBJ_STR)) {
                        vm_set_error(vm, "map key must be string");
                        ok = false;
                        break;
                    }
                    pb_map_set(m, (PbStr *)kv.u.obj, vv);
                }
                if (!ok) break;
                vm->stack_len = start;
                ok = vm_push(vm, pb_obj_val((PbObj *)m));
                break;
            }
            case OP_GET_INDEX: {
                PbValue idx, obj;
                if (!vm_pop(vm, &idx) || !vm_pop(vm, &obj)) { ok = false; break; }
                if (pb_is_obj_kind(obj, OBJ_ARRAY)) {
                    if (idx.tag != VAL_NUM) {
                        vm_set_error(vm, "array index must be a number");
                        ok = false;
                        break;
                    }
                    PbArray *a = (PbArray *)obj.u.obj;
                    int64_t i = (int64_t)idx.u.num;
                    if (i < 0 || (size_t)i >= a->len) {
                        vm_set_error(vm, "array index out of bounds");
                        ok = false;
                    } else {
                        ok = vm_push(vm, a->items[(size_t)i]);
                    }
                } else if (pb_is_obj_kind(obj, OBJ_MAP)) {
                    if (!pb_is_obj_kind(idx, OBJ_STR)) {
                        vm_set_error(vm, "map key must be a string");
                        ok = false;
                        break;
                    }
                    PbStr *k = (PbStr *)idx.u.obj;
                    PbValue v;
                    if (!pb_map_get((PbMap *)obj.u.obj, k, &v)) {
                        char buf[256];
                        snprintf(buf, sizeof(buf), "undefined key '%s'", k->data);
                        vm_set_error(vm, buf);
                        ok = false;
                    } else {
                        ok = vm_push(vm, v);
                    }
                } else if (pb_is_obj_kind(obj, OBJ_STR)) {
                    if (idx.tag != VAL_NUM) {
                        vm_set_error(vm, "string index must be a number");
                        ok = false;
                        break;
                    }
                    PbStr *s = (PbStr *)obj.u.obj;
                    int64_t i = (int64_t)idx.u.num;
                    size_t n_cp = utf8_codepoint_count(s->data, s->len);
                    if (i < 0 || (size_t)i >= n_cp) {
                        vm_set_error(vm, "string index out of bounds");
                        ok = false;
                    } else {
                        size_t b0 = utf8_char_to_byte_offset(s->data, s->len, (size_t)i);
                        size_t b1 = utf8_char_to_byte_offset(s->data, s->len, (size_t)i + 1);
                        size_t saved = vm->temp_root_len;
                        vm_push_root(vm, (PbObj *)s);
                        PbStr *ch = pb_new_str(vm, s->data + b0, b1 - b0);
                        vm_pop_roots(vm, saved);
                        ok = vm_push(vm, pb_obj_val((PbObj *)ch));
                    }
                } else {
                    vm_set_error(vm, "indexing only supported on arrays, maps, and strings");
                    ok = false;
                }
                break;
            }
            case OP_SET_INDEX: {
                PbValue val, idx, obj;
                if (!vm_pop(vm, &val) || !vm_pop(vm, &idx) || !vm_pop(vm, &obj)) { ok = false; break; }
                if (pb_is_obj_kind(obj, OBJ_ARRAY)) {
                    if (idx.tag != VAL_NUM) {
                        vm_set_error(vm, "array index must be a number");
                        ok = false;
                        break;
                    }
                    PbArray *a = (PbArray *)obj.u.obj;
                    int64_t i = (int64_t)idx.u.num;
                    if (i < 0 || (size_t)i > a->len) {
                        vm_set_error(vm, "array index out of bounds");
                        ok = false;
                        break;
                    }
                    if ((size_t)i == a->len) {
                        pb_array_push(a, val);
                    } else {
                        a->items[(size_t)i] = val;
                    }
                    ok = vm_push(vm, val);
                } else if (pb_is_obj_kind(obj, OBJ_MAP)) {
                    if (!pb_is_obj_kind(idx, OBJ_STR)) {
                        vm_set_error(vm, "map key must be a string");
                        ok = false;
                        break;
                    }
                    pb_map_set((PbMap *)obj.u.obj, (PbStr *)idx.u.obj, val);
                    ok = vm_push(vm, val);
                } else {
                    vm_set_error(vm, "index assignment only supported on arrays and maps");
                    ok = false;
                }
                break;
            }
            case OP_TRY: {
                if (vm->handler_len >= VM_MAX_HANDLERS) {
                    vm_set_error(vm, "try handler stack overflow");
                    ok = false;
                    break;
                }
                PbTryHandler *h = &vm->handlers[vm->handler_len++];
                h->frame_idx = vm->frame_len - 1;
                h->target_ip = ins->arg;
                h->stack_len = vm->stack_len;
                break;
            }
            case OP_END_TRY: {
                if (vm->handler_len > 0) vm->handler_len--;
                break;
            }
            case OP_GET_LOCAL: {
                size_t idx = frame->stack_base + 1 + (size_t)ins->arg;
                if (idx >= vm->stack_len) {
                    vm_set_error(vm, "local slot out of bounds");
                    ok = false;
                } else {
                    ok = vm_push(vm, vm->stack[idx]);
                }
                break;
            }
            case OP_SET_LOCAL: {
                PbValue v;
                if (!vm_pop(vm, &v)) { ok = false; break; }
                size_t idx = frame->stack_base + 1 + (size_t)ins->arg;
                if (idx >= vm->stack_len) {
                    vm_set_error(vm, "local slot out of bounds on set");
                    ok = false;
                } else {
                    vm->stack[idx] = v;
                }
                break;
            }
            case OP_CLOSURE: {
                if (ins->arg >= chunk->const_len || !pb_is_obj_kind(chunk->consts[ins->arg], OBJ_FUNC)) {
                    vm_set_error(vm, "OpClosure expects function constant");
                    ok = false;
                    break;
                }
                PbFunc *fn = (PbFunc *)chunk->consts[ins->arg].u.obj;
                size_t saved = vm->temp_root_len;
                PbClosure *cl = pb_new_closure(vm, fn, ins->n_caps);
                vm_push_root(vm, (PbObj *)cl);
                for (uint32_t i = 0; i < ins->n_caps; i++) {
                    if (ins->caps[i].is_local) {
                        size_t slot = frame->stack_base + 1 + (size_t)ins->caps[i].idx;
                        cl->upvalues[i] = vm_capture_upvalue(vm, slot);
                    } else {
                        if (ins->caps[i].idx >= frame->closure->n_upvalues) {
                            vm_pop_roots(vm, saved);
                            vm_set_error(vm, "outer upvalue index out of bounds");
                            ok = false;
                            break;
                        }
                        cl->upvalues[i] = frame->closure->upvalues[ins->caps[i].idx];
                    }
                }
                vm_pop_roots(vm, saved);
                if (ok) ok = vm_push(vm, pb_obj_val((PbObj *)cl));
                break;
            }
            case OP_GET_UPVALUE: {
                if (ins->arg >= frame->closure->n_upvalues) {
                    vm_set_error(vm, "upvalue index out of bounds");
                    ok = false;
                    break;
                }
                PbUpvalue *uv = frame->closure->upvalues[ins->arg];
                PbValue v = uv->is_open ? vm->stack[uv->stack_idx] : uv->closed;
                ok = vm_push(vm, v);
                break;
            }
            case OP_SET_UPVALUE: {
                PbValue v;
                if (!vm_pop(vm, &v)) { ok = false; break; }
                if (ins->arg >= frame->closure->n_upvalues) {
                    vm_set_error(vm, "upvalue index out of bounds");
                    ok = false;
                    break;
                }
                PbUpvalue *uv = frame->closure->upvalues[ins->arg];
                if (uv->is_open) {
                    vm->stack[uv->stack_idx] = v;
                } else {
                    uv->closed = v;
                }
                break;
            }
            case OP_CLOSE_UPVALUE: {
                if (vm->stack_len == 0) {
                    vm_set_error(vm, "stack underflow (close upvalue)");
                    ok = false;
                    break;
                }
                vm_close_upvalues_from(vm, vm->stack_len - 1);
                vm->stack_len--;
                break;
            }
            case OP_CALL:
                ok = vm_call_value(vm, ins->arg, false);
                if (ok) vm_maybe_gc(vm);
                break;
            case OP_TAIL_CALL:
                ok = vm_call_value(vm, ins->arg, true);
                if (ok) vm_maybe_gc(vm);
                break;
            case OP_RETURN: {
                PbValue ret;
                if (!vm_pop(vm, &ret)) { ok = false; break; }
                size_t saved = vm->temp_root_len;
                if (ret.tag == VAL_OBJ) vm_push_root(vm, ret.u.obj);
                vm_run_frame_defers(vm, frame);
                size_t base = frame->stack_base;
                vm_close_upvalues_from(vm, base + 1);
                vm->frame_len--;
                vm->stack_len = base;
                ok = vm_push(vm, ret);
                vm_pop_roots(vm, saved);
                break;
            }
            case OP_DEFER: {
                PbValue thunk;
                if (!vm_pop(vm, &thunk)) { ok = false; break; }
                if (frame->defer_len >= frame->defer_cap) {
                    size_t ncap = frame->defer_cap < 4 ? 4 : frame->defer_cap * 2;
                    frame->defers = (PbValue *)xrealloc(frame->defers, ncap * sizeof(PbValue));
                    frame->defer_cap = ncap;
                }
                frame->defers[frame->defer_len++] = thunk;
                break;
            }
            case OP_JUMP:
                frame->ip = ins->arg;
                vm_maybe_gc(vm);
                break;
            case OP_JUMP_IF_FALSE: {
                if (vm->stack_len == 0) {
                    vm_set_error(vm, "stack underflow (jump_if_false)");
                    ok = false;
                    break;
                }
                if (!pb_is_truthy(vm->stack[vm->stack_len - 1])) {
                    frame->ip = ins->arg;
                }
                break;
            }
            case OP_JUMP_IF_TRUE: {
                if (vm->stack_len == 0) {
                    vm_set_error(vm, "stack underflow (jump_if_true)");
                    ok = false;
                    break;
                }
                if (pb_is_truthy(vm->stack[vm->stack_len - 1])) {
                    frame->ip = ins->arg;
                }
                break;
            }
            case OP_NOT: {
                PbValue v;
                if (!vm_pop(vm, &v)) { ok = false; break; }
                ok = vm_push(vm, pb_bool(!pb_is_truthy(v)));
                break;
            }
            case OP_NEGATE: {
                PbValue v;
                if (!vm_pop(vm, &v)) { ok = false; break; }
                if (v.tag != VAL_NUM) {
                    vm_set_error(vm, "type error: cannot negate non-number");
                    ok = false;
                } else {
                    ok = vm_push(vm, pb_num(-v.u.num));
                }
                break;
            }
            case OP_SAFE_GET_ATTR: {
                PbValue obj;
                if (!vm_pop(vm, &obj)) { ok = false; break; }
                const PbStr *attr = chunk_const_str(vm, chunk, ins->arg);
                if (!attr) { ok = false; break; }
                if (obj.tag == VAL_NIL) {
                    ok = vm_push(vm, pb_nil());
                } else if (pb_is_obj_kind(obj, OBJ_MAP)) {
                    PbValue v = pb_nil();
                    (void)pb_map_get((PbMap *)obj.u.obj, attr, &v);
                    ok = vm_push(vm, v);
                } else {
                    vm_set_error(vm, "?.attr: expected map or nil");
                    ok = false;
                }
                break;
            }
            case OP_SAFE_INDEX: {
                PbValue idx, obj;
                if (!vm_pop(vm, &idx) || !vm_pop(vm, &obj)) { ok = false; break; }
                if (obj.tag == VAL_NIL) {
                    ok = vm_push(vm, pb_nil());
                } else if (pb_is_obj_kind(obj, OBJ_ARRAY)) {
                    if (idx.tag != VAL_NUM) { vm_set_error(vm, "array?[]: index must be number"); ok = false; break; }
                    PbArray *a = (PbArray *)obj.u.obj;
                    int64_t i = (int64_t)idx.u.num;
                    ok = vm_push(vm, (i < 0 || (size_t)i >= a->len) ? pb_nil() : a->items[(size_t)i]);
                } else if (pb_is_obj_kind(obj, OBJ_MAP)) {
                    if (!pb_is_obj_kind(idx, OBJ_STR)) { vm_set_error(vm, "map?[]: key must be string"); ok = false; break; }
                    PbValue v = pb_nil();
                    (void)pb_map_get((PbMap *)obj.u.obj, (PbStr *)idx.u.obj, &v);
                    ok = vm_push(vm, v);
                } else if (pb_is_obj_kind(obj, OBJ_STR)) {
                    if (idx.tag != VAL_NUM) { vm_set_error(vm, "string?[]: index must be number"); ok = false; break; }
                    PbStr *s = (PbStr *)obj.u.obj;
                    int64_t i = (int64_t)idx.u.num;
                    size_t n_cp = utf8_codepoint_count(s->data, s->len);
                    if (i < 0 || (size_t)i >= n_cp) {
                        ok = vm_push(vm, pb_nil());
                    } else {
                        size_t b0 = utf8_char_to_byte_offset(s->data, s->len, (size_t)i);
                        size_t b1 = utf8_char_to_byte_offset(s->data, s->len, (size_t)i + 1);
                        size_t saved = vm->temp_root_len;
                        vm_push_root(vm, (PbObj *)s);
                        PbStr *ch = pb_new_str(vm, s->data + b0, b1 - b0);
                        vm_pop_roots(vm, saved);
                        ok = vm_push(vm, pb_obj_val((PbObj *)ch));
                    }
                } else {
                    ok = vm_push(vm, pb_nil());
                }
                break;
            }
            case OP_RESULT_UNWRAP: {
                PbValue v;
                if (!vm_pop(vm, &v)) { ok = false; break; }
                if (v.tag == VAL_NIL) {
                    vm_set_error(vm, "? operator: cannot unwrap nil");
                    ok = false;
                } else if (pb_is_obj_kind(v, OBJ_MAP)) {
                    PbMap *m = (PbMap *)v.u.obj;
                    PbValue ok_val;
                    if (pb_map_get_cstr(m, "ok", 2, fnv1a_bytes((const uint8_t *)"ok", 2), &ok_val) &&
                        ok_val.tag == VAL_BOOL) {
                        if (!ok_val.u.b) {
                            PbValue err_v;
                            if (pb_map_get_cstr(m, "error", 5, fnv1a_bytes((const uint8_t *)"error", 5), &err_v) &&
                                pb_is_obj_kind(err_v, OBJ_STR)) {
                                vm_set_error(vm, ((PbStr *)err_v.u.obj)->data);
                            } else {
                                vm_set_error(vm, "error (from ? operator)");
                            }
                            ok = false;
                        } else {
                            PbValue res_v = pb_nil();
                            (void)pb_map_get_cstr(m, "value", 5, fnv1a_bytes((const uint8_t *)"value", 5), &res_v);
                            ok = vm_push(vm, res_v);
                        }
                    } else {
                        ok = vm_push(vm, v);
                    }
                } else {
                    ok = vm_push(vm, v);
                }
                break;
            }
            case OP_IS_NIL: {
                PbValue v;
                if (!vm_pop(vm, &v)) { ok = false; break; }
                ok = vm_push(vm, pb_bool(v.tag == VAL_NIL));
                break;
            }
            case OP_DUP: {
                if (vm->stack_len == 0) {
                    vm_set_error(vm, "OpDup: stack underflow");
                    ok = false;
                } else {
                    ok = vm_push(vm, vm->stack[vm->stack_len - 1]);
                }
                break;
            }
            case OP_THROW: {
                PbValue v;
                if (!vm_pop(vm, &v)) { ok = false; break; }
                StrBuf sb;
                sb_init(&sb);
                pb_format_value_rec(&sb, v, 0);
                vm_set_error(vm, sb.buf);
                free(sb.buf);
                ok = false;
                break;
            }
            default:
                vm_set_error(vm, "unknown opcode during execution");
                ok = false;
                break;
        }

        if (!ok) {
            if (!vm_unwind_error(vm)) {
                return false;
            }
        }
    }
}

/* ── CLI Entry Point ─────────────────────────────────────────────────────── */
static uint8_t *read_file_bounded(const char *path, size_t *out_len) {
    FILE *f = fopen(path, "rb");
    if (!f) {
        fprintf(stderr, "pblang-zero-rust: cannot open '%s': %s\n", path, strerror(errno));
        return NULL;
    }
    if (fseek(f, 0, SEEK_END) != 0) {
        fclose(f);
        fprintf(stderr, "pblang-zero-rust: fseek failed on '%s'\n", path);
        return NULL;
    }
    long sz = ftell(f);
    if (sz < 0 || (uint64_t)sz > PBCK_MAX_BYTES) {
        fclose(f);
        fprintf(stderr, "pblang-zero-rust: '%s' exceeds 64 MiB limit\n", path);
        return NULL;
    }
    rewind(f);
    uint8_t *buf = (uint8_t *)xmalloc((size_t)sz + 1);
    size_t n = sz > 0 ? fread(buf, 1, (size_t)sz, f) : 0;
    fclose(f);
    *out_len = n;
    return buf;
}

static void print_usage(FILE *s) {
    fprintf(s, "pblang-zero-rust %s — Stage-3 Zero-Rust Native .pbck Bytecode VM\n", PBLANG_ZERO_RUST_VERSION);
    fprintf(s, "Usage:\n");
    fprintf(s, "  pblang-zero-rust [run] <file.pbck> [-- args...]\n");
    fprintf(s, "  pblang-zero-rust --version | --help\n");
}

int main(int argc, char **argv) {
    if (argc < 2) {
        print_usage(stderr);
        return 1;
    }
    if (strcmp(argv[1], "--version") == 0 || strcmp(argv[1], "-V") == 0) {
        printf("pblang-zero-rust %s (zero-rust C99 .pbck v1 VM)\n", PBLANG_ZERO_RUST_VERSION);
        return 0;
    }
    if (strcmp(argv[1], "--help") == 0 || strcmp(argv[1], "-h") == 0) {
        print_usage(stdout);
        return 0;
    }

    int arg_idx = 1;
    if (strcmp(argv[1], "run") == 0) {
        arg_idx = 2;
    }
    if (arg_idx >= argc) {
        print_usage(stderr);
        return 1;
    }
    const char *pbck_path = argv[arg_idx++];
    if (arg_idx < argc && strcmp(argv[arg_idx], "--") == 0) {
        arg_idx++;
    }

    size_t pbck_len = 0;
    uint8_t *pbck_bytes = read_file_bounded(pbck_path, &pbck_len);
    if (!pbck_bytes) return 1;

    PbVM vm;
    vm_init(&vm, argc - arg_idx, &argv[arg_idx]);

    char load_err[256];
    PbFunc *script_fn = pbck_deserialize_script(&vm, pbck_bytes, pbck_len, load_err);
    free(pbck_bytes);
    if (!script_fn) {
        fprintf(stderr, "pblang-zero-rust: %s\n", load_err);
        vm_destroy(&vm);
        return 1;
    }

    PbClosure *script_cl = pb_new_closure(&vm, script_fn, 0);
    (void)vm_push(&vm, pb_obj_val((PbObj *)script_cl));
    vm.frames[0].closure = script_cl;
    vm.frames[0].ip = 0;
    vm.frames[0].stack_base = 0;
    vm.frames[0].defers = NULL;
    vm.frames[0].defer_len = 0;
    vm.frames[0].defer_cap = 0;
    vm.frame_len = 1;

    bool ok = vm_exec_from_frame(&vm, 0);
    int exit_code = 0;
    if (!ok) {
        fprintf(stderr, "Runtime error: %s\n", vm.error_msg);
        exit_code = 1;
    }

    vm_destroy(&vm);
    return exit_code;
}
