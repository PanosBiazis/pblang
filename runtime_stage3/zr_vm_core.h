/*
 * zr_vm_core.h — Sovereign Zero-Rust Runtime v3.0.0 Bytecode VM, Objects, GC & Dispatch
 *
 * Uses the exact .pbck v1 instruction model (opcodes 0..=47 on disk, plus in-memory
 * superinstructions 48..=54, monomorphic inline caches, generational nursery GC,
 * zero-copy binary/float64 buffers, call-stack capabilities, taint tracking, and
 * W^X x86_64 native JIT) so external .pbck artifacts and C99-compiled .sapp modules
 * share one unified, zero-translation engine.
 */
#ifndef PBLANG_ZR_VM_CORE_H
#define PBLANG_ZR_VM_CORE_H

#include "zr_common.h"

/* ---- Exact .pbck v1 Wire Opcode Tags (0..=47) + In-Memory Superinstructions (48..=54) ---- */
enum {
    OP_CONSTANT            = 0,
    OP_ADD                 = 1,
    OP_SUB                 = 2,
    OP_MUL                 = 3,
    OP_DIV                 = 4,
    OP_EQ                  = 5,
    OP_NOT_EQ              = 6,
    OP_GT                  = 7,
    OP_GE                  = 8,
    OP_LT                  = 9,
    OP_LE                  = 10,
    OP_POP                 = 11,
    OP_PRINT               = 12,
    OP_DEFINE_GLOBAL       = 13,
    OP_GET_GLOBAL          = 14,
    OP_SET_GLOBAL          = 15,
    OP_GET_ATTR            = 16,
    OP_SET_ATTR            = 17,
    OP_EXPORT              = 18,
    OP_MAKE_ARRAY          = 19,
    OP_MAKE_MAP            = 20,
    OP_GET_INDEX           = 21,
    OP_SET_INDEX           = 22,
    OP_TRY                 = 23,
    OP_END_TRY             = 24,
    OP_GET_LOCAL           = 25,
    OP_SET_LOCAL           = 26,
    OP_CLOSURE             = 27,
    OP_GET_UPVALUE         = 28,
    OP_SET_UPVALUE         = 29,
    OP_CLOSE_UPVALUE       = 30,
    OP_CALL                = 31,
    OP_TAIL_CALL           = 32,
    OP_RETURN              = 33,
    OP_DEFER               = 34,
    OP_JUMP                = 35,
    OP_JUMP_IF_FALSE       = 36,
    OP_JUMP_IF_TRUE        = 37,
    OP_NOT                 = 38,
    OP_NEGATE              = 39,
    OP_MOD                 = 40,
    OP_POW                 = 41,
    OP_SAFE_GET_ATTR       = 42,
    OP_SAFE_INDEX          = 43,
    OP_RESULT_UNWRAP       = 44,
    OP_IS_NIL              = 45,
    OP_DUP                 = 46,
    OP_THROW               = 47,
    /* In-memory peephole superinstructions (never written to .pbck; orig_op stays 0..=47) */
    OP_INC_LOCAL_1         = 48,
    OP_ADD_LOCAL_LOCAL     = 49,
    OP_SUB_LOCAL_CONST     = 50,
    OP_LT_LOCAL_CONST_JMPF = 51,
    OP_INC_GLOBAL_1        = 52,
    OP_LT_GLOBAL_CONST_JMPF= 53,
    OP_LT_LOCAL_LOCAL_JMPF = 54
};

/* ---- Taint Bits & Per-Module / Call-Stack Capability Bits ---- */

enum {
    TAINT_CLEAN  = 0,
    TAINT_SECRET = 1,
    TAINT_PII    = 2
};

enum {
    CAP_FS      = (1u << 0),
    CAP_NET     = (1u << 1),
    CAP_HTTP    = (1u << 2),
    CAP_SQLITE  = (1u << 3),
    CAP_FFI     = (1u << 4),
    CAP_EXEC    = (1u << 5),
    CAP_GFX     = (1u << 6),
    CAP_SPAWN   = (1u << 7),
    CAP_ENV     = (1u << 8)
};
#define CAP_ALL 0xFFFFFFFFu

/* ---- Object & Value Model ---- */

typedef enum {
    OBJ_STR      = 1,
    OBJ_ARRAY    = 2,
    OBJ_MAP      = 3,
    OBJ_FUNC     = 4,
    OBJ_CLOSURE  = 5,
    OBJ_UPVALUE  = 6,
    OBJ_MODULE   = 7,
    OBJ_BIGINT   = 8,
    OBJ_RATIONAL = 9,
    OBJ_DECIMAL  = 10,
    OBJ_BYTES    = 11,
    OBJ_F64BUF   = 12
} PbObjType;

typedef struct PbObj PbObj;
typedef struct PbVM  PbVM;

typedef enum {
    VAL_NIL     = 0,
    VAL_BOOL    = 1,
    VAL_NUM     = 2,
    VAL_OBJ     = 3,
    VAL_BUILTIN = 4
} PbValTag;

typedef union {
    bool     b;
    double   num;
    PbObj   *obj;
    uint16_t builtin_id;
} PbValUnion;

typedef struct {
    PbValTag tag;
    uint8_t  taint;
    union {
        PbValUnion as;
        PbValUnion u;
    };
} PbValue;

struct PbObj {
    PbObjType type;
    uint8_t   marked;
    uint8_t   frozen;
    uint8_t   gc_gen;  /* 0 = nursery (young), 1 = old (tenured) */
    uint8_t   gc_rem;  /* 1 = old object in remembered set */
    size_t    tracked_bytes;
    PbObj    *next;
};

typedef struct {
    PbObj    obj;
    uint64_t hash;
    size_t   len;
    char     chars[];
} PbStr;

typedef struct {
    PbObj    obj;
    PbValue *items;
    size_t   len;
    size_t   cap;
} PbArray;

typedef struct {
    volatile int32_t refcount;
    uint8_t          kind; /* 1 = bytes, 2 = f64buf */
    size_t           byte_len;
    void            *raw;
} ZrSharedBacking;

static inline ZrSharedBacking *zr_shared_backing_new(uint8_t kind, size_t byte_len) {
    ZrSharedBacking *bk = (ZrSharedBacking *)zr_xcalloc(1, sizeof(ZrSharedBacking));
    bk->refcount = 1;
    bk->kind = kind;
    bk->byte_len = byte_len;
    bk->raw = zr_xcalloc(byte_len ? byte_len : 1, 1);
    return bk;
}

static inline ZrSharedBacking *zr_shared_retain(ZrSharedBacking *bk) {
    if (bk) zr_atomic_inc32(&bk->refcount);
    return bk;
}

static inline void zr_shared_release(ZrSharedBacking *bk) {
    if (!bk) return;
    if (zr_atomic_dec32(&bk->refcount) <= 0) {
        free(bk->raw);
        free(bk);
    }
}

typedef struct {
    PbObj            obj;
    uint8_t         *data;
    size_t           len;
    size_t           cap;
    bool             shared;
    ZrSharedBacking *backing;
} PbBytes;

typedef struct {
    PbObj            obj;
    double          *data;
    size_t           len;
    size_t           cap;
    bool             shared;
    ZrSharedBacking *backing;
} PbF64Buf;

typedef struct {
    PbStr  *key;
    PbValue val;
    bool    active;
} PbMapEntry;

typedef struct {
    PbObj       obj;
    PbMapEntry *entries;
    union {
        size_t  len;      /* active count */
        size_t  count;
    };
    size_t      used;     /* appended slots */
    size_t      cap;      /* entries capacity */
    int32_t    *buckets;  /* open-addressing hash table: -1 empty, -2 tombstone, >=0 index */
    size_t      bcap;     /* power-of-2 bucket count */
} PbMap;

typedef struct {
    uint8_t  is_local;
    uint32_t idx;
} PbCaptureSpec;

typedef struct {
    uint8_t        op;
    uint8_t        orig_op;
    uint8_t        fused_skip;
    uint8_t        pad_u8;
    uint32_t       arg;
    uint32_t       arg2;
    size_t         ic_bcap;
    int32_t        ic_slot;
    uint32_t       n_caps;
    PbCaptureSpec *caps; /* non-NULL only for OP_CLOSURE when n_caps > 0 */
    uint32_t       line;
} PbInstr;

typedef struct {
    char  *lang;
    char  *name;      /* nullable */
    char **attr_keys;
    char **attr_vals;
    size_t attr_count;
    char  *code;
    size_t line;
} PbPolyBlock;

typedef double (*ZrJitEntryFn)(const double *args, int *bailout);

typedef struct {
    PbObj        obj;
    PbStr       *name;
    PbStr       *source_name;
    uint32_t     arity;      /* fixed param count (excluding ...rest when is_vararg) */
    uint32_t     min_arity;  /* required param count */
    uint32_t     n_defaults; /* defaults[0 .. n_defaults-1] cover params [min_arity .. arity-1] */
    PbValue     *defaults;
    bool         is_vararg;
    bool         is_async;
    PbInstr     *code;
    uint32_t     code_len;
    uint32_t     code_cap;
    PbValue     *consts;
    uint32_t     const_len;
    uint32_t     const_cap;
    /* JIT & peephole state */
    uint32_t     call_count;
    bool         peephole_done;
    bool         jit_attempted;
    bool         jit_ret_bool;
    int8_t       jit_self_kind; /* 0 = none, 1 = self global, 2 = self upvalue[0] */
    uint32_t     jit_self_arg;
    void        *jit_code;
    size_t       jit_code_bytes;
    ZrJitEntryFn jit_fn;
    /* Hotspot profiler counters */
    uint64_t     prof_calls;
    uint64_t     prof_self_ns;
    uint64_t     prof_total_ns;
    /* Top-level metadata for pblang test discovery & @polyglot blocks */
    char       **top_test_fns;
    size_t       top_test_fn_count;
    bool         has_top_test_calls;
    PbPolyBlock *poly_blocks;
    size_t       poly_count;
} PbFunc;

typedef struct PbUpvalue {
    PbObj             obj;
    bool              is_open;
    size_t            stack_idx;
    PbValue           closed;
    struct PbUpvalue *next_open;
} PbUpvalue;

typedef struct PbModule {
    PbObj    obj;
    PbStr   *name;
    PbStr   *path;
    PbMap   *globals;
    PbMap   *exports;
    uint32_t cap_mask;
    bool     loading;
    bool     loaded;
} PbModule;

typedef struct {
    PbObj       obj;
    PbFunc     *fn;
    PbModule   *module; /* NULL = root VM globals/exports */
    PbUpvalue **upvalues;
    uint32_t    upvalue_count;
} PbClosure;

typedef struct {
    PbObj    obj;
    ZrBigInt v;
} PbBigIntObj;

typedef struct {
    PbObj      obj;
    ZrRational v;
} PbRationalObj;

typedef struct {
    PbObj     obj;
    ZrDecimal v;
} PbDecimalObj;

/* ---- Value Constructors & Exact Truthiness ---- */

static inline PbValue val_nil(void)            { PbValue v; v.tag = VAL_NIL; v.taint = 0; v.as.num = 0; return v; }
static inline PbValue val_bool(bool b)         { PbValue v; v.tag = VAL_BOOL; v.taint = 0; v.as.b = b; return v; }
static inline PbValue val_num(double n)        { PbValue v; v.tag = VAL_NUM; v.taint = 0; v.as.num = n; return v; }
static inline PbValue val_obj(void *o)         { PbValue v; v.tag = VAL_OBJ; v.taint = 0; v.as.obj = (PbObj *)o; return v; }
static inline PbValue val_builtin(uint16_t id) { PbValue v; v.tag = VAL_BUILTIN; v.taint = 0; v.as.builtin_id = id; return v; }
static inline PbValue val_with_taint(PbValue v, uint8_t t) { v.taint = t; return v; }

static inline bool is_obj_type(PbValue v, PbObjType t) {
    return v.tag == VAL_OBJ && v.as.obj != NULL && v.as.obj->type == t;
}

#define AS_STR(v)      ((PbStr *)(v).as.obj)
#define AS_ARRAY(v)    ((PbArray *)(v).as.obj)
#define AS_MAP(v)      ((PbMap *)(v).as.obj)
#define AS_FUNC(v)     ((PbFunc *)(v).as.obj)
#define AS_CLOSURE(v)  ((PbClosure *)(v).as.obj)
#define AS_MODULE(v)   ((PbModule *)(v).as.obj)
#define AS_BIGINT(v)   ((PbBigIntObj *)(v).as.obj)
#define AS_RATIONAL(v) ((PbRationalObj *)(v).as.obj)
#define AS_DECIMAL(v)  ((PbDecimalObj *)(v).as.obj)
#define AS_BYTES(v)    ((PbBytes *)(v).as.obj)
#define AS_F64BUF(v)   ((PbF64Buf *)(v).as.obj)

/* Matches VM::is_truthy in crates/pblang-vm/src/vm.rs */
static inline bool val_truthy(PbValue v) {
    switch (v.tag) {
        case VAL_NIL:     return false;
        case VAL_BOOL:    return v.as.b;
        case VAL_NUM:     return v.as.num != 0.0;
        case VAL_BUILTIN: return true;
        case VAL_OBJ:
            switch (v.as.obj->type) {
                case OBJ_STR:      return AS_STR(v)->len > 0;
                case OBJ_ARRAY:    return AS_ARRAY(v)->len > 0;
                case OBJ_MAP:      return AS_MAP(v)->len > 0;
                case OBJ_BIGINT:   return AS_BIGINT(v)->v.sign != 0;
                case OBJ_RATIONAL: return AS_RATIONAL(v)->v.num.sign != 0;
                case OBJ_DECIMAL:  return AS_DECIMAL(v)->v.coeff.sign != 0;
                case OBJ_BYTES:    return AS_BYTES(v)->len > 0;
                case OBJ_F64BUF:   return AS_F64BUF(v)->len > 0;
                default:           return true;
            }
    }
    return false;
}

static inline const char *zr_val_type_name(PbValue v) {
    switch (v.tag) {
        case VAL_NIL:     return "nil";
        case VAL_BOOL:    return "bool";
        case VAL_NUM:     return "number";
        case VAL_BUILTIN: return "fn";
        case VAL_OBJ:
            switch (v.as.obj->type) {
                case OBJ_STR:      return "string";
                case OBJ_ARRAY:    return "array";
                case OBJ_MAP:      return "map";
                case OBJ_FUNC:
                case OBJ_CLOSURE:  return "fn";
                case OBJ_MODULE:   return "module";
                case OBJ_BIGINT:   return "bigint";
                case OBJ_RATIONAL: return "rational";
                case OBJ_DECIMAL:  return "decimal";
                case OBJ_BYTES:    return "bytes";
                case OBJ_F64BUF:   return "f64buf";
                default:           return "object";
            }
    }
    return "nil";
}

/* ---- Capabilities, Frames, Try Handlers & VM State ---- */

typedef struct {
    bool   allow_fs;
    bool   allow_net;
    bool   allow_http;
    bool   allow_sqlite;
    bool   allow_ffi;
    bool   ffi_allow_all;
    bool   allow_exec;
    bool   allow_gfx;
    bool   allow_import;
    bool   allow_stdlib_import;
    bool   allow_spawn;
    bool   allow_env;
    bool   allow_process_exit;
    char **ffi_allowed_libs;
    size_t ffi_allowed_libs_len;
    char **ffi_allowed_syms;
    size_t ffi_allowed_syms_len;
    uint32_t io_timeout_ms;
} ZrCaps;

static inline void zr_caps_init_default(ZrCaps *c) {
    memset(c, 0, sizeof(*c));
    c->allow_stdlib_import = true;
    c->allow_import = true;
    c->allow_spawn = true;
    c->allow_env = true;
    c->allow_process_exit = true;
    c->io_timeout_ms = 30000;
}

static inline void zr_caps_allow_all(ZrCaps *c) {
    memset(c, 0, sizeof(*c));
    c->allow_fs = true;
    c->allow_net = true;
    c->allow_http = true;
    c->allow_sqlite = true;
    c->allow_ffi = true;
    c->ffi_allow_all = true;
    c->allow_exec = true;
    c->allow_gfx = true;
    c->allow_import = true;
    c->allow_stdlib_import = true;
    c->allow_spawn = true;
    c->allow_env = true;
    c->allow_process_exit = true;
    c->io_timeout_ms = 30000;
}

typedef bool (*PbBuiltinFn)(PbVM *vm, int argc, PbValue *argv, PbValue *out);

typedef struct {
    const char *name;
    PbBuiltinFn fn;
} PbBuiltinDef;

typedef struct {
    PbClosure *closure;
    uint32_t   ip;
    size_t     base;
    uint32_t   cap_mask;
    uint64_t   prof_enter_ns;
    uint64_t   prof_child_ns;
    PbValue   *defers;
    size_t     defer_len;
    size_t     defer_cap;
} CallFrame;

typedef struct {
    size_t   frame_idx;
    size_t   stack_depth;
    uint32_t target_ip;
    size_t   defer_depth;
} TryHandler;

typedef struct {
    uint64_t seq;
    char     fn[32];
    char     type[16];
    bool     is_nil;
    double   num_val;
    char    *str_val;
    size_t   str_len;
} ZrFlightEvent;

#define ZR_STACK_INIT     4096
#define ZR_MAX_FRAMES     2048
#define ZR_MAX_HANDLERS   512
#define ZR_MAX_BUILTINS   768
#define ZR_MAX_TEMP_ROOTS 256

struct PbVM {
    PbValue     *stack;
    size_t       sp;
    size_t       stack_cap;

    CallFrame    frames[ZR_MAX_FRAMES];
    size_t       frame_count;

    TryHandler   handlers[ZR_MAX_HANDLERS];
    size_t       handler_count;

    PbUpvalue   *open_upvalues;

    PbMap       *globals;
    PbMap       *exports;
    PbMap       *builtins;
    PbMap       *modules;       /* canonical path -> OBJ_MODULE */
    PbMap       *signal_handlers;

    PbBuiltinDef builtin_defs[ZR_MAX_BUILTINS];
    uint16_t     builtin_count;

    /* Temporary GC root stack for multi-allocation C operations */
    PbValue      temp_roots[ZR_MAX_TEMP_ROOTS];
    size_t       temp_root_count;

    /* Two-Generation Nursery + Adaptive Mark-and-Sweep GC */
    PbObj       *objects;          /* tenured / old generation */
    PbObj       *nursery_objects;  /* young generation */
    PbObj      **rem_set;          /* remembered set of tenured parents */
    size_t       rem_len;
    size_t       rem_cap;
    size_t       nursery_bytes;
    size_t       nursery_threshold;
    size_t       live_bytes;
    size_t       next_gc_bytes;
    size_t       obj_count;
    size_t       gc_collections;
    size_t       nursery_collections;
    size_t       full_collections;
    uint32_t     minor_since_full;
    bool         nursery_enabled;
    size_t       string_allocs;
    size_t       array_allocs;
    size_t       map_allocs;
    size_t       module_allocs;
    int          gc_inhibit;

    /* JIT statistics */
    size_t       jit_compiled_functions;
    size_t       jit_total_code_bytes;
    uint64_t     jit_invocations;

    /* Taint-tracking guard */
    bool         taint_guard_enabled;

    /* Flight-recorder & deterministic replay */
    bool           flight_recording;
    FILE          *flight_rec_fp;
    uint64_t       flight_seq;
    bool           flight_replaying;
    ZrFlightEvent *flight_events;
    size_t         flight_event_count;
    size_t         flight_replay_cursor;

    /* Hotspot profiler */
    bool         prof_enabled;

    /* Paths & CLI arguments */
    union {
        char    *current_file;
        char    *entry_script_path;
    };
    char        *stdlib_root;
    char        *exe_path;
    int          cli_argc;
    char       **cli_argv;

    /* Capability configuration */
    ZrCaps       caps;
    uint32_t     pending_import_cap_mask;
    bool         has_pending_import_caps;

    /* Error & process exit state */
    char         err_msg[2048];
    bool         exited;
    int          exit_code;
};

/* Forward Hooks */
static bool zr_vm_import_module(PbVM *vm, const char *spec, const char *caller_file, PbValue *out_mod);
static void zr_register_all_builtins(PbVM *vm);
static bool vm_call_value_sync(PbVM *vm, PbValue callee, int argc, const PbValue *argv, PbValue *out_ret);
static void vm_collect_garbage(PbVM *vm, size_t *out_arr, size_t *out_map, size_t *out_mod);
static void vm_collect_nursery(PbVM *vm);
static void zr_populate_polyglot_globals(PbVM *vm, PbMap *target_globals, const PbFunc *fn);
static void zr_peephole_optimize_func(PbFunc *fn);
static bool zr_jit_compile_func(PbVM *vm, PbFunc *fn);
static void zr_jit_free_code(PbFunc *fn);
static void zr_ffi_gc_mark_roots(PbVM *vm);
static void zr_ffi_gc_mark_nursery_roots(PbVM *vm);
static void zr_ffi_cleanup_vm(PbVM *vm);
static void zr_flight_cleanup_vm(PbVM *vm);
static bool zr_check_taint_sink(PbVM *vm, PbValue v, const char *sink_name);

/* ---- Error Reporting & Temporary Roots ---- */

static bool vm_error(PbVM *vm, const char *fmt, ...) {
    va_list ap;
    va_start(ap, fmt);
    vsnprintf(vm->err_msg, sizeof(vm->err_msg), fmt, ap);
    va_end(ap);
    return false;
}

static inline void vm_push_root(PbVM *vm, PbValue v) {
    if (vm->temp_root_count < ZR_MAX_TEMP_ROOTS) {
        vm->temp_roots[vm->temp_root_count++] = v;
    }
}

static inline void vm_pop_roots(PbVM *vm, size_t n) {
    if (n >= vm->temp_root_count) vm->temp_root_count = 0;
    else vm->temp_root_count -= n;
}

static inline bool vm_root_cap_allowed(const PbVM *vm, uint32_t cap_bit) {
    if (cap_bit == CAP_FS)     return vm->caps.allow_fs;
    if (cap_bit == CAP_NET)    return vm->caps.allow_net;
    if (cap_bit == CAP_HTTP)   return vm->caps.allow_http;
    if (cap_bit == CAP_SQLITE) return vm->caps.allow_sqlite;
    if (cap_bit == CAP_FFI)    return vm->caps.allow_ffi;
    if (cap_bit == CAP_EXEC)   return vm->caps.allow_exec;
    if (cap_bit == CAP_GFX)    return vm->caps.allow_gfx;
    if (cap_bit == CAP_SPAWN)  return vm->caps.allow_spawn;
    if (cap_bit == CAP_ENV)    return vm->caps.allow_env;
    return true;
}

static inline bool vm_effective_cap(const PbVM *vm, uint32_t cap_bit) {
    if (!vm_root_cap_allowed(vm, cap_bit)) return false;
    for (size_t i = 0; i < vm->frame_count; i++) {
        const CallFrame *f = &vm->frames[i];
        if ((f->cap_mask & cap_bit) == 0) return false;
        if (f->closure && f->closure->module && (f->closure->module->cap_mask & cap_bit) == 0) {
            return false;
        }
    }
    return true;
}

static inline bool vm_check_cap(PbVM *vm, bool allowed, const char *cap_name, const char *flag_hint) {
    if (allowed) return true;
    return vm_error(vm, "permission denied: capability '%s' is disabled (pass %s or --allow-all)", cap_name, flag_hint);
}

/* ---- Stack Growth ---- */

static inline void vm_ensure_stack(PbVM *vm, size_t needed) {
    if (vm->sp + needed < vm->stack_cap) return;
    size_t ncap = vm->stack_cap ? vm->stack_cap * 2 : ZR_STACK_INIT;
    while (ncap <= vm->sp + needed) ncap *= 2;
    vm->stack = (PbValue *)zr_xrealloc(vm->stack, ncap * sizeof(PbValue));
    vm->stack_cap = ncap;
}

static inline void vm_push(PbVM *vm, PbValue v) {
    vm_ensure_stack(vm, 2);
    vm->stack[vm->sp++] = v;
}

static inline PbValue vm_pop(PbVM *vm) {
    return vm->stack[--vm->sp];
}

/* ---- GC Object Allocation & Generational Write Barriers ---- */

static volatile int32_t g_zr_null_vm_barrier = 0;

static inline void vm_write_barrier_obj(PbVM *vm, PbObj *parent, PbObj *child_obj) {
    if (!parent || !child_obj) return;
    if (parent->gc_gen == 1 && child_obj->gc_gen == 0 && !parent->gc_rem) {
        parent->gc_rem = 1;
        if (vm) {
            if (vm->rem_len >= vm->rem_cap) {
                size_t ncap = vm->rem_cap ? vm->rem_cap * 2 : 128;
                vm->rem_set = (PbObj **)zr_xrealloc(vm->rem_set, ncap * sizeof(PbObj *));
                vm->rem_cap = ncap;
            }
            vm->rem_set[vm->rem_len++] = parent;
        } else {
            zr_atomic_inc32(&g_zr_null_vm_barrier);
        }
    }
}

static inline void vm_write_barrier(PbVM *vm, PbObj *parent, PbValue child) {
    if (child.tag == VAL_OBJ && child.as.obj != NULL) {
        vm_write_barrier_obj(vm, parent, child.as.obj);
    }
}

static void *vm_alloc_obj(PbVM *vm, size_t sz, PbObjType type) {
    PbObj *o = (PbObj *)zr_xcalloc(1, sz);
    o->type = type;
    o->marked = 0;
    o->frozen = 0;
    o->gc_rem = 0;
    o->tracked_bytes = sz;
    if (vm->nursery_enabled && vm->gc_inhibit == 0 && type != OBJ_FUNC && type != OBJ_MODULE) {
        o->gc_gen = 0;
        o->next = vm->nursery_objects;
        vm->nursery_objects = o;
        vm->nursery_bytes += sz;
    } else {
        o->gc_gen = 1;
        o->next = vm->objects;
        vm->objects = o;
    }
    vm->obj_count++;
    vm->live_bytes += sz;
    return o;
}

static PbStr *pb_str_new(PbVM *vm, const char *chars, size_t len) {
    PbStr *s = (PbStr *)vm_alloc_obj(vm, sizeof(PbStr) + len + 1, OBJ_STR);
    if (len > 0 && chars) memcpy(s->chars, chars, len);
    s->chars[len] = '\0';
    s->len = len;
    s->hash = zr_fnv1a(s->chars, len);
    vm->string_allocs++;
    return s;
}

static inline PbStr *pb_str_c(PbVM *vm, const char *cstr) {
    if (!cstr) cstr = "";
    return pb_str_new(vm, cstr, strlen(cstr));
}

static PbBytes *pb_bytes_new(PbVM *vm, size_t len, bool shared) {
    PbBytes *b = (PbBytes *)vm_alloc_obj(vm, sizeof(PbBytes), OBJ_BYTES);
    b->len = len;
    b->cap = len ? len : 1;
    b->shared = shared;
    if (shared) {
        b->backing = zr_shared_backing_new(1, b->cap);
        b->data = (uint8_t *)b->backing->raw;
    } else {
        b->backing = NULL;
        b->data = (uint8_t *)zr_xcalloc(b->cap, 1);
    }
    vm->live_bytes += b->cap;
    if (b->obj.gc_gen == 0) vm->nursery_bytes += b->cap;
    b->obj.tracked_bytes += b->cap;
    return b;
}

static PbBytes *pb_bytes_from_backing(PbVM *vm, ZrSharedBacking *bk, size_t len) {
    PbBytes *b = (PbBytes *)vm_alloc_obj(vm, sizeof(PbBytes), OBJ_BYTES);
    b->len = len;
    b->cap = bk ? bk->byte_len : len;
    b->shared = true;
    b->backing = zr_shared_retain(bk);
    b->data = bk ? (uint8_t *)bk->raw : NULL;
    return b;
}

static PbF64Buf *pb_f64buf_new(PbVM *vm, size_t len, bool shared) {
    PbF64Buf *b = (PbF64Buf *)vm_alloc_obj(vm, sizeof(PbF64Buf), OBJ_F64BUF);
    b->len = len;
    b->cap = len ? len : 1;
    b->shared = shared;
    size_t blen = b->cap * sizeof(double);
    if (shared) {
        b->backing = zr_shared_backing_new(2, blen);
        b->data = (double *)b->backing->raw;
    } else {
        b->backing = NULL;
        b->data = (double *)zr_xcalloc(b->cap, sizeof(double));
    }
    vm->live_bytes += blen;
    if (b->obj.gc_gen == 0) vm->nursery_bytes += blen;
    b->obj.tracked_bytes += blen;
    return b;
}

static PbF64Buf *pb_f64buf_from_backing(PbVM *vm, ZrSharedBacking *bk, size_t len) {
    PbF64Buf *b = (PbF64Buf *)vm_alloc_obj(vm, sizeof(PbF64Buf), OBJ_F64BUF);
    b->len = len;
    b->cap = bk ? (bk->byte_len / sizeof(double)) : len;
    b->shared = true;
    b->backing = zr_shared_retain(bk);
    b->data = bk ? (double *)bk->raw : NULL;
    return b;
}

static PbArray *pb_array_new(PbVM *vm, size_t cap) {
    PbArray *a = (PbArray *)vm_alloc_obj(vm, sizeof(PbArray), OBJ_ARRAY);
    vm->array_allocs++;
    if (cap > 0) {
        vm_push_root(vm, val_obj(a));
        a->items = (PbValue *)zr_xcalloc(cap, sizeof(PbValue));
        a->cap = cap;
        size_t extra = cap * sizeof(PbValue);
        vm->live_bytes += extra;
        if (a->obj.gc_gen == 0) vm->nursery_bytes += extra;
        a->obj.tracked_bytes += extra;
        vm_pop_roots(vm, 1);
    }
    return a;
}

static void pb_array_push(PbVM *vm, PbArray *a, PbValue v) {
    if (a->len >= a->cap) {
        size_t old_cap = a->cap;
        size_t ncap = old_cap ? old_cap * 2 : 8;
        a->items = (PbValue *)zr_xrealloc(a->items, ncap * sizeof(PbValue));
        a->cap = ncap;
        size_t added = (ncap - old_cap) * sizeof(PbValue);
        if (vm) {
            vm->live_bytes += added;
            if (a->obj.gc_gen == 0) vm->nursery_bytes += added;
        }
        a->obj.tracked_bytes += added;
    }
    a->items[a->len++] = v;
    vm_write_barrier(vm, (PbObj *)a, v);
}

/* ---- Open-Addressing FNV-1a PbMap ---- */

static PbMap *pb_map_new_raw(PbVM *vm, size_t hint) {
    PbMap *m = (PbMap *)vm_alloc_obj(vm, sizeof(PbMap), OBJ_MAP);
    vm->map_allocs++;
    if (hint > 0) {
        size_t bcap = 16;
        while (bcap < hint * 2) bcap *= 2;
        m->entries = (PbMapEntry *)zr_xcalloc(hint, sizeof(PbMapEntry));
        m->cap = hint;
        m->buckets = (int32_t *)zr_xmalloc(bcap * sizeof(int32_t));
        for (size_t i = 0; i < bcap; i++) m->buckets[i] = -1;
        m->bcap = bcap;
        size_t extra = hint * sizeof(PbMapEntry) + bcap * sizeof(int32_t);
        vm->live_bytes += extra;
        if (m->obj.gc_gen == 0) vm->nursery_bytes += extra;
        m->obj.tracked_bytes += extra;
    }
    return m;
}

static inline PbMap *pb_map_new(PbVM *vm) {
    return pb_map_new_raw(vm, 0);
}

static void pb_map_rebuild_buckets(PbVM *vm, PbMap *m, size_t new_bcap) {
    size_t old_bcap = m->bcap;
    free(m->buckets);
    m->buckets = (int32_t *)zr_xmalloc(new_bcap * sizeof(int32_t));
    for (size_t i = 0; i < new_bcap; i++) m->buckets[i] = -1;
    m->bcap = new_bcap;
    if (new_bcap > old_bcap) {
        size_t added = (new_bcap - old_bcap) * sizeof(int32_t);
        if (vm) {
            vm->live_bytes += added;
            if (m->obj.gc_gen == 0) vm->nursery_bytes += added;
        }
        m->obj.tracked_bytes += added;
    }
    if (m->used > m->len * 2 && m->used > 16) {
        size_t w = 0;
        for (size_t i = 0; i < m->used; i++) {
            if (m->entries[i].active) {
                if (w != i) m->entries[w] = m->entries[i];
                w++;
            }
        }
        m->used = w;
    }
    size_t mask = new_bcap - 1;
    for (size_t i = 0; i < m->used; i++) {
        if (!m->entries[i].active) continue;
        size_t slot = (size_t)m->entries[i].key->hash & mask;
        while (m->buckets[slot] >= 0) slot = (slot + 1) & mask;
        m->buckets[slot] = (int32_t)i;
    }
}

static int32_t pb_map_find_entry_idx(const PbMap *m, const char *key, size_t klen, uint64_t hash) {
    if (!m || m->len == 0 || m->bcap == 0) return -1;
    size_t mask = m->bcap - 1;
    size_t slot = (size_t)hash & mask;
    for (size_t probes = 0; probes < m->bcap; probes++) {
        int32_t idx = m->buckets[slot];
        if (idx == -1) return -1;
        if (idx >= 0) {
            const PbMapEntry *e = &m->entries[idx];
            if (e->active && e->key->hash == hash && e->key->len == klen &&
                memcmp(e->key->chars, key, klen) == 0) {
                return idx;
            }
        }
        slot = (slot + 1) & mask;
    }
    return -1;
}

static bool pb_map_get_raw(const PbMap *m, const char *key, size_t klen, uint64_t hash, PbValue *out) {
    int32_t idx = pb_map_find_entry_idx(m, key, klen, hash);
    if (idx < 0) return false;
    if (out) *out = m->entries[idx].val;
    return true;
}

static inline bool pb_map_get(const PbMap *m, const PbStr *key, PbValue *out) {
    return pb_map_get_raw(m, key->chars, key->len, key->hash, out);
}

static inline bool pb_map_get_cstr(const PbMap *m, const char *cstr, PbValue *out) {
    size_t n = strlen(cstr);
    return pb_map_get_raw(m, cstr, n, zr_fnv1a(cstr, n), out);
}

static int32_t pb_map_set_idx(PbVM *vm, PbMap *m, PbStr *key, PbValue val) {
    if (m->bcap == 0 || (m->used + 1) * 10 >= m->bcap * 7) {
        size_t nb = m->bcap ? m->bcap * 2 : 16;
        pb_map_rebuild_buckets(vm, m, nb);
    }
    size_t mask = m->bcap - 1;
    size_t slot = (size_t)key->hash & mask;
    ssize_t first_tomb = -1;
    for (;;) {
        int32_t idx = m->buckets[slot];
        if (idx == -1) {
            size_t target_slot = (first_tomb >= 0) ? (size_t)first_tomb : slot;
            if (m->used >= m->cap) {
                size_t old_cap = m->cap;
                size_t ncap = old_cap ? old_cap * 2 : 8;
                m->entries = (PbMapEntry *)zr_xrealloc(m->entries, ncap * sizeof(PbMapEntry));
                m->cap = ncap;
                size_t added = (ncap - old_cap) * sizeof(PbMapEntry);
                if (vm) {
                    vm->live_bytes += added;
                    if (m->obj.gc_gen == 0) vm->nursery_bytes += added;
                }
                m->obj.tracked_bytes += added;
            }
            size_t eidx = m->used++;
            m->entries[eidx].key = key;
            m->entries[eidx].val = val;
            m->entries[eidx].active = true;
            m->buckets[target_slot] = (int32_t)eidx;
            m->len++;
            vm_write_barrier_obj(vm, (PbObj *)m, (PbObj *)key);
            vm_write_barrier(vm, (PbObj *)m, val);
            return (int32_t)eidx;
        } else if (idx == -2) {
            if (first_tomb < 0) first_tomb = (ssize_t)slot;
        } else {
            PbMapEntry *e = &m->entries[idx];
            if (e->active && e->key->hash == key->hash && e->key->len == key->len &&
                memcmp(e->key->chars, key->chars, key->len) == 0) {
                e->val = val;
                vm_write_barrier(vm, (PbObj *)m, val);
                return idx;
            }
        }
        slot = (slot + 1) & mask;
    }
}

static inline void pb_map_set(PbVM *vm, PbMap *m, PbStr *key, PbValue val) {
    (void)pb_map_set_idx(vm, m, key, val);
}

static inline void pb_map_set_cstr(PbVM *vm, PbMap *m, const char *k, PbValue val) {
    vm_push_root(vm, val);
    vm_push_root(vm, val_obj(m));
    PbStr *ks = pb_str_c(vm, k);
    vm_push_root(vm, val_obj(ks));
    pb_map_set(vm, m, ks, val);
    vm_pop_roots(vm, 3);
}

static bool pb_map_delete(PbMap *m, const PbStr *key, PbValue *removed_out) {
    if (!m || m->len == 0 || m->bcap == 0) return false;
    size_t mask = m->bcap - 1;
    size_t slot = (size_t)key->hash & mask;
    for (size_t probes = 0; probes < m->bcap; probes++) {
        int32_t idx = m->buckets[slot];
        if (idx == -1) return false;
        if (idx >= 0) {
            PbMapEntry *e = &m->entries[idx];
            if (e->active && e->key->hash == key->hash && e->key->len == key->len &&
                memcmp(e->key->chars, key->chars, key->len) == 0) {
                if (removed_out) *removed_out = e->val;
                e->active = false;
                e->val = val_nil();
                m->buckets[slot] = -2;
                m->len--;
                return true;
            }
        }
        slot = (slot + 1) & mask;
    }
    return false;
}

static int zr_cmp_pbstr_ptr(const void *a, const void *b) {
    const PbStr *sa = *(const PbStr *const *)a;
    const PbStr *sb = *(const PbStr *const *)b;
    size_t min_l = sa->len < sb->len ? sa->len : sb->len;
    int c = memcmp(sa->chars, sb->chars, min_l);
    if (c != 0) return c;
    return (sa->len < sb->len) ? -1 : ((sa->len > sb->len) ? 1 : 0);
}

static PbStr **pb_map_sorted_keys(const PbMap *m, size_t *out_count) {
    *out_count = 0;
    if (!m || m->len == 0) return NULL;
    PbStr **keys = (PbStr **)zr_xmalloc(m->len * sizeof(PbStr *));
    size_t k = 0;
    for (size_t i = 0; i < m->used; i++) {
        if (m->entries[i].active) keys[k++] = m->entries[i].key;
    }
    if (k > 1) qsort(keys, k, sizeof(PbStr *), zr_cmp_pbstr_ptr);
    *out_count = k;
    return keys;
}

/* ---- BigInt / Rational / Decimal / Func / Closure / Module Constructors ---- */

static PbValue pb_bigint_new_take(PbVM *vm, ZrBigInt *bi) {
    PbBigIntObj *o = (PbBigIntObj *)vm_alloc_obj(vm, sizeof(PbBigIntObj), OBJ_BIGINT);
    o->v = *bi;
    zr_bi_init(bi);
    return val_obj(o);
}

static PbValue pb_rational_new_take(PbVM *vm, ZrRational *r) {
    if (zr_rat_is_int(r)) {
        ZrBigInt n = r->num;
        zr_bi_init(&r->num);
        zr_bi_free(&r->den);
        return pb_bigint_new_take(vm, &n);
    }
    PbRationalObj *o = (PbRationalObj *)vm_alloc_obj(vm, sizeof(PbRationalObj), OBJ_RATIONAL);
    o->v = *r;
    zr_bi_init(&r->num);
    zr_bi_init(&r->den);
    return val_obj(o);
}

static PbValue pb_decimal_new_take(PbVM *vm, ZrDecimal *d) {
    PbDecimalObj *o = (PbDecimalObj *)vm_alloc_obj(vm, sizeof(PbDecimalObj), OBJ_DECIMAL);
    o->v = *d;
    zr_bi_init(&d->coeff);
    d->scale = 0;
    return val_obj(o);
}

static PbFunc *pb_func_new(PbVM *vm) {
    return (PbFunc *)vm_alloc_obj(vm, sizeof(PbFunc), OBJ_FUNC);
}

static PbClosure *pb_closure_new(PbVM *vm, PbFunc *fn, uint32_t n_upvals, PbModule *mod) {
    vm_push_root(vm, val_obj(fn));
    if (mod) vm_push_root(vm, val_obj(mod));
    PbClosure *cl = (PbClosure *)vm_alloc_obj(vm, sizeof(PbClosure), OBJ_CLOSURE);
    cl->fn = fn;
    cl->module = mod;
    cl->upvalue_count = n_upvals;
    if (n_upvals > 0) {
        cl->upvalues = (PbUpvalue **)zr_xcalloc(n_upvals, sizeof(PbUpvalue *));
    }
    vm_write_barrier_obj(vm, (PbObj *)cl, (PbObj *)fn);
    if (mod) vm_write_barrier_obj(vm, (PbObj *)cl, (PbObj *)mod);
    vm_pop_roots(vm, mod ? 2 : 1);
    return cl;
}

static PbModule *pb_module_new(PbVM *vm, const char *name, const char *path) {
    PbModule *mod = (PbModule *)vm_alloc_obj(vm, sizeof(PbModule), OBJ_MODULE);
    vm->module_allocs++;
    vm_push_root(vm, val_obj(mod));
    mod->name = pb_str_c(vm, name ? name : "<module>");
    mod->path = pb_str_c(vm, path ? path : "");
    mod->globals = pb_map_new(vm);
    mod->exports = pb_map_new(vm);
    mod->cap_mask = CAP_ALL;
    mod->loading = false;
    mod->loaded = false;
    vm_write_barrier_obj(vm, (PbObj *)mod, (PbObj *)mod->name);
    vm_write_barrier_obj(vm, (PbObj *)mod, (PbObj *)mod->path);
    vm_write_barrier_obj(vm, (PbObj *)mod, (PbObj *)mod->globals);
    vm_write_barrier_obj(vm, (PbObj *)mod, (PbObj *)mod->exports);
    vm_pop_roots(vm, 1);
    return mod;
}

/* ---- Value Formatting & Equality ---- */

static void zr_format_number(ZrBuf *b, double n) {
    if (isnan(n)) {
        zr_buf_append_cstr(b, "NaN");
        return;
    }
    if (isinf(n)) {
        zr_buf_append_cstr(b, signbit(n) ? "-inf" : "inf");
        return;
    }
    if (n == 0.0) {
        zr_buf_append_cstr(b, signbit(n) ? "-0" : "0");
        return;
    }
    if (floor(n) == n && fabs(n) <= 9007199254740992.0) {
        zr_buf_printf(b, "%" PRId64, (int64_t)n);
        return;
    }
    char tmp[64];
    snprintf(tmp, sizeof(tmp), "%.15g", n);
    char *endp = NULL;
    double rt = strtod(tmp, &endp);
    if (rt != n) {
        snprintf(tmp, sizeof(tmp), "%.17g", n);
    }
    zr_buf_append_cstr(b, tmp);
}

static void vm_format_value_rec(PbVM *vm, ZrBuf *b, PbValue v, int depth) {
    if (depth > 32) {
        zr_buf_append_cstr(b, "...");
        return;
    }
    switch (v.tag) {
        case VAL_NIL:
            zr_buf_append_cstr(b, "nil");
            return;
        case VAL_BOOL:
            zr_buf_append_cstr(b, v.as.b ? "true" : "false");
            return;
        case VAL_NUM:
            zr_format_number(b, v.as.num);
            return;
        case VAL_BUILTIN: {
            const char *nm = (v.as.builtin_id < vm->builtin_count) ? vm->builtin_defs[v.as.builtin_id].name : "?";
            zr_buf_printf(b, "<builtin %s>", nm);
            return;
        }
        case VAL_OBJ:
            switch (v.as.obj->type) {
                case OBJ_STR: {
                    PbStr *s = AS_STR(v);
                    zr_buf_append(b, s->chars, s->len);
                    return;
                }
                case OBJ_BIGINT: {
                    char *s = zr_bi_to_str(&AS_BIGINT(v)->v);
                    zr_buf_append_cstr(b, s);
                    free(s);
                    return;
                }
                case OBJ_RATIONAL: {
                    char *s = zr_rat_to_str(&AS_RATIONAL(v)->v);
                    zr_buf_append_cstr(b, s);
                    free(s);
                    return;
                }
                case OBJ_DECIMAL: {
                    char *s = zr_dec_to_str(&AS_DECIMAL(v)->v);
                    zr_buf_append_cstr(b, s);
                    free(s);
                    return;
                }
                case OBJ_FUNC:
                    zr_buf_append_cstr(b, "fn");
                    return;
                case OBJ_CLOSURE:
                    zr_buf_append_cstr(b, "closure");
                    return;
                case OBJ_MODULE:
                    zr_buf_append_cstr(b, "<module>");
                    return;
                case OBJ_BYTES:
                    zr_buf_printf(b, "<bytes len=%zu>", AS_BYTES(v)->len);
                    return;
                case OBJ_F64BUF:
                    zr_buf_printf(b, "<f64buf len=%zu>", AS_F64BUF(v)->len);
                    return;
                case OBJ_ARRAY: {
                    PbArray *arr = AS_ARRAY(v);
                    zr_buf_push(b, '[');
                    for (size_t i = 0; i < arr->len; i++) {
                        if (i > 0) zr_buf_append_cstr(b, ", ");
                        vm_format_value_rec(vm, b, arr->items[i], depth + 1);
                    }
                    zr_buf_push(b, ']');
                    return;
                }
                case OBJ_MAP: {
                    PbMap *m = AS_MAP(v);
                    zr_buf_append_cstr(b, "map{");
                    size_t kcount = 0;
                    PbStr **keys = pb_map_sorted_keys(m, &kcount);
                    for (size_t i = 0; i < kcount; i++) {
                        if (i > 0) zr_buf_append_cstr(b, ", ");
                        zr_buf_append(b, keys[i]->chars, keys[i]->len);
                        zr_buf_append_cstr(b, ": ");
                        PbValue mv = val_nil();
                        pb_map_get(m, keys[i], &mv);
                        vm_format_value_rec(vm, b, mv, depth + 1);
                    }
                    free(keys);
                    zr_buf_push(b, '}');
                    return;
                }
                default:
                    zr_buf_append_cstr(b, "<object>");
                    return;
            }
    }
}

static char *vm_value_to_cstr(PbVM *vm, PbValue v, size_t *out_len) {
    ZrBuf b;
    zr_buf_init(&b);
    vm_format_value_rec(vm, &b, v, 0);
    return zr_buf_take(&b, out_len);
}

static PbStr *vm_value_to_pbstr(PbVM *vm, PbValue v) {
    if (is_obj_type(v, OBJ_STR)) return AS_STR(v);
    size_t len = 0;
    char *s = vm_value_to_cstr(vm, v, &len);
    PbStr *res = pb_str_new(vm, s, len);
    free(s);
    return res;
}

static bool val_equal(PbValue a, PbValue b) {
    if (a.tag == VAL_NUM && b.tag == VAL_NUM) return a.as.num == b.as.num;
    if ((a.tag == VAL_NUM || is_obj_type(a, OBJ_BIGINT) || is_obj_type(a, OBJ_RATIONAL) || is_obj_type(a, OBJ_DECIMAL)) &&
        (b.tag == VAL_NUM || is_obj_type(b, OBJ_BIGINT) || is_obj_type(b, OBJ_RATIONAL) || is_obj_type(b, OBJ_DECIMAL))) {
        if (is_obj_type(a, OBJ_BIGINT) && is_obj_type(b, OBJ_BIGINT)) {
            return zr_bi_cmp(&AS_BIGINT(a)->v, &AS_BIGINT(b)->v) == 0;
        }
        if (is_obj_type(a, OBJ_BIGINT) && b.tag == VAL_NUM) {
            ZrBigInt tb;
            if (!zr_bi_from_f64(&tb, b.as.num)) return false;
            int c = zr_bi_cmp(&AS_BIGINT(a)->v, &tb);
            zr_bi_free(&tb);
            return c == 0;
        }
        if (a.tag == VAL_NUM && is_obj_type(b, OBJ_BIGINT)) {
            ZrBigInt ta;
            if (!zr_bi_from_f64(&ta, a.as.num)) return false;
            int c = zr_bi_cmp(&ta, &AS_BIGINT(b)->v);
            zr_bi_free(&ta);
            return c == 0;
        }
        if (is_obj_type(a, OBJ_RATIONAL) && is_obj_type(b, OBJ_RATIONAL)) {
            return zr_bi_cmp(&AS_RATIONAL(a)->v.num, &AS_RATIONAL(b)->v.num) == 0 &&
                   zr_bi_cmp(&AS_RATIONAL(a)->v.den, &AS_RATIONAL(b)->v.den) == 0;
        }
        if (is_obj_type(a, OBJ_DECIMAL) && is_obj_type(b, OBJ_DECIMAL)) {
            return AS_DECIMAL(a)->v.scale == AS_DECIMAL(b)->v.scale &&
                   zr_bi_cmp(&AS_DECIMAL(a)->v.coeff, &AS_DECIMAL(b)->v.coeff) == 0;
        }
        return false;
    }
    if (a.tag != b.tag) return false;
    switch (a.tag) {
        case VAL_NIL:     return true;
        case VAL_BOOL:    return a.as.b == b.as.b;
        case VAL_NUM:     return a.as.num == b.as.num;
        case VAL_BUILTIN: return a.as.builtin_id == b.as.builtin_id;
        case VAL_OBJ: {
            if (a.as.obj == b.as.obj) return true;
            if (a.as.obj->type != b.as.obj->type) return false;
            if (a.as.obj->type == OBJ_STR) {
                PbStr *sa = AS_STR(a), *sb = AS_STR(b);
                return sa->hash == sb->hash && sa->len == sb->len && memcmp(sa->chars, sb->chars, sa->len) == 0;
            }
            if (a.as.obj->type == OBJ_BYTES) {
                PbBytes *ba = AS_BYTES(a), *bb = AS_BYTES(b);
                if (ba->len != bb->len) return false;
                return ba->len == 0 || memcmp(ba->data, bb->data, ba->len) == 0;
            }
            if (a.as.obj->type == OBJ_F64BUF) {
                PbF64Buf *ba = AS_F64BUF(a), *bb = AS_F64BUF(b);
                if (ba->len != bb->len) return false;
                for (size_t i = 0; i < ba->len; i++) {
                    if (ba->data[i] != bb->data[i]) return false;
                }
                return true;
            }
            if (a.as.obj->type == OBJ_ARRAY) {
                PbArray *aa = AS_ARRAY(a), *ab = AS_ARRAY(b);
                if (aa->len != ab->len) return false;
                for (size_t i = 0; i < aa->len; i++) {
                    if (!val_equal(aa->items[i], ab->items[i])) return false;
                }
                return true;
            }
            if (a.as.obj->type == OBJ_MAP) {
                PbMap *ma = AS_MAP(a), *mb = AS_MAP(b);
                if (ma->len != mb->len) return false;
                for (size_t i = 0; i < ma->used; i++) {
                    if (!ma->entries[i].active) continue;
                    PbValue vb;
                    if (!pb_map_get(mb, ma->entries[i].key, &vb)) return false;
                    if (!val_equal(ma->entries[i].val, vb)) return false;
                }
                return true;
            }
            return false;
        }
    }
    return false;
}

/* ---- Exact BigNum Promotion & Dunder Overloads ---- */

static bool zr_extract_bigint(PbValue v, ZrBigInt *out) {
    if (is_obj_type(v, OBJ_BIGINT)) {
        zr_bi_copy(out, &AS_BIGINT(v)->v);
        return true;
    }
    if (v.tag == VAL_NUM) return zr_bi_from_f64(out, v.as.num);
    return false;
}

static bool zr_extract_rational(PbValue v, ZrRational *out) {
    if (is_obj_type(v, OBJ_RATIONAL)) {
        zr_bi_copy(&out->num, &AS_RATIONAL(v)->v.num);
        zr_bi_copy(&out->den, &AS_RATIONAL(v)->v.den);
        return true;
    }
    if (is_obj_type(v, OBJ_BIGINT)) {
        zr_bi_copy(&out->num, &AS_BIGINT(v)->v);
        zr_bi_from_i64(&out->den, 1);
        return true;
    }
    if (is_obj_type(v, OBJ_DECIMAL)) {
        char *s = zr_dec_to_str(&AS_DECIMAL(v)->v);
        bool ok = zr_rat_from_str(out, s);
        free(s);
        return ok;
    }
    if (v.tag == VAL_NUM && isfinite(v.as.num)) {
        char buf[64];
        snprintf(buf, sizeof(buf), "%.17g", v.as.num);
        return zr_rat_from_str(out, buf);
    }
    return false;
}

static bool zr_extract_decimal(PbValue v, ZrDecimal *out) {
    if (is_obj_type(v, OBJ_DECIMAL)) {
        zr_bi_copy(&out->coeff, &AS_DECIMAL(v)->v.coeff);
        out->scale = AS_DECIMAL(v)->v.scale;
        return true;
    }
    if (is_obj_type(v, OBJ_BIGINT)) {
        zr_bi_copy(&out->coeff, &AS_BIGINT(v)->v);
        out->scale = 0;
        return true;
    }
    if (v.tag == VAL_NUM && isfinite(v.as.num)) {
        char buf[64];
        snprintf(buf, sizeof(buf), "%.15g", v.as.num);
        return zr_dec_from_str(out, buf);
    }
    return false;
}

static bool vm_lookup_map_dunder(PbValue val, const char *dunder, PbValue *out_fn) {
    if (!is_obj_type(val, OBJ_MAP)) return false;
    PbMap *cur = AS_MAP(val);
    for (int depth = 0; depth < 32 && cur != NULL; depth++) {
        if (pb_map_get_cstr(cur, dunder, out_fn) && out_fn->tag != VAL_NIL) return true;
        PbValue parent;
        if (pb_map_get_cstr(cur, "__class", &parent) && is_obj_type(parent, OBJ_MAP)) {
            cur = AS_MAP(parent);
        } else {
            break;
        }
    }
    return false;
}

static bool vm_bignum_arith(PbVM *vm, uint8_t op, PbValue a, PbValue b, PbValue *out) {
    if (is_obj_type(a, OBJ_RATIONAL) || is_obj_type(b, OBJ_RATIONAL)) {
        if (op == OP_POW) {
            ZrRational ra;
            ZrBigInt exp_bi;
            if (!zr_extract_rational(a, &ra) || !zr_extract_bigint(b, &exp_bi)) {
                return vm_error(vm, "rational ** requires integer exponent");
            }
            int esign = exp_bi.sign;
            exp_bi.sign = exp_bi.len ? 1 : 0;
            uint64_t exp_u = (uint64_t)zr_bi_to_f64(&exp_bi);
            zr_bi_free(&exp_bi);
            ZrBigInt pn, pd;
            zr_bi_pow(&pn, &ra.num, exp_u);
            zr_bi_pow(&pd, &ra.den, exp_u);
            zr_rat_free(&ra);
            ZrRational rr;
            bool ok = (esign < 0) ? zr_rat_init_norm(&rr, &pd, &pn) : zr_rat_init_norm(&rr, &pn, &pd);
            zr_bi_free(&pn); zr_bi_free(&pd);
            if (!ok) return vm_error(vm, "division by zero in rational power");
            *out = pb_rational_new_take(vm, &rr);
            return true;
        }
        ZrRational ra, rb, rr;
        if (!zr_extract_rational(a, &ra)) return vm_error(vm, "invalid rational left operand");
        if (!zr_extract_rational(b, &rb)) { zr_rat_free(&ra); return vm_error(vm, "invalid rational right operand"); }
        bool ok = true;
        if (op == OP_ADD || op == OP_SUB) {
            ZrBigInt t1, t2, num, den;
            zr_bi_mul(&t1, &ra.num, &rb.den);
            zr_bi_mul(&t2, &rb.num, &ra.den);
            if (op == OP_ADD) zr_bi_add(&num, &t1, &t2);
            else zr_bi_sub(&num, &t1, &t2);
            zr_bi_mul(&den, &ra.den, &rb.den);
            ok = zr_rat_init_norm(&rr, &num, &den);
            zr_bi_free(&t1); zr_bi_free(&t2); zr_bi_free(&num); zr_bi_free(&den);
        } else if (op == OP_MUL) {
            ZrBigInt num, den;
            zr_bi_mul(&num, &ra.num, &rb.num);
            zr_bi_mul(&den, &ra.den, &rb.den);
            ok = zr_rat_init_norm(&rr, &num, &den);
            zr_bi_free(&num); zr_bi_free(&den);
        } else if (op == OP_DIV) {
            ZrBigInt num, den;
            zr_bi_mul(&num, &ra.num, &rb.den);
            zr_bi_mul(&den, &ra.den, &rb.num);
            ok = zr_rat_init_norm(&rr, &num, &den);
            zr_bi_free(&num); zr_bi_free(&den);
        } else {
            zr_rat_free(&ra); zr_rat_free(&rb);
            return vm_error(vm, "unsupported rational operator");
        }
        zr_rat_free(&ra); zr_rat_free(&rb);
        if (!ok) return vm_error(vm, "rational division by zero");
        *out = pb_rational_new_take(vm, &rr);
        return true;
    }

    if (is_obj_type(a, OBJ_DECIMAL) || is_obj_type(b, OBJ_DECIMAL)) {
        ZrDecimal da, db, dr;
        if (!zr_extract_decimal(a, &da)) return vm_error(vm, "invalid decimal left operand");
        if (!zr_extract_decimal(b, &db)) { zr_dec_free(&da); return vm_error(vm, "invalid decimal right operand"); }
        if (op == OP_ADD || op == OP_SUB) {
            int32_t sc = da.scale > db.scale ? da.scale : db.scale;
            while (da.scale < sc) { zr_bi_mul_u32_ip(&da.coeff, 10); da.scale++; }
            while (db.scale < sc) { zr_bi_mul_u32_ip(&db.coeff, 10); db.scale++; }
            if (op == OP_ADD) zr_bi_add(&dr.coeff, &da.coeff, &db.coeff);
            else zr_bi_sub(&dr.coeff, &da.coeff, &db.coeff);
            dr.scale = sc;
            zr_dec_normalize(&dr);
        } else if (op == OP_MUL) {
            zr_bi_mul(&dr.coeff, &da.coeff, &db.coeff);
            dr.scale = da.scale + db.scale;
            zr_dec_normalize(&dr);
        } else if (op == OP_DIV) {
            if (db.coeff.sign == 0) { zr_dec_free(&da); zr_dec_free(&db); return vm_error(vm, "decimal division by zero"); }
            int32_t extra = 28;
            ZrBigInt scaled, rem;
            zr_bi_copy(&scaled, &da.coeff);
            for (int32_t i = 0; i < extra; i++) zr_bi_mul_u32_ip(&scaled, 10);
            zr_bi_divmod(&dr.coeff, &rem, &scaled, &db.coeff);
            zr_bi_free(&scaled); zr_bi_free(&rem);
            dr.scale = da.scale + extra - db.scale;
            while (dr.scale < 0) { zr_bi_mul_u32_ip(&dr.coeff, 10); dr.scale++; }
            zr_dec_normalize(&dr);
        } else {
            zr_dec_free(&da); zr_dec_free(&db);
            return vm_error(vm, "unsupported decimal operator");
        }
        zr_dec_free(&da); zr_dec_free(&db);
        *out = pb_decimal_new_take(vm, &dr);
        return true;
    }

    ZrBigInt ba, bb, br, rem;
    if (!zr_extract_bigint(a, &ba)) return vm_error(vm, "bigint arithmetic requires integer operands");
    if (!zr_extract_bigint(b, &bb)) { zr_bi_free(&ba); return vm_error(vm, "bigint arithmetic requires integer operands"); }
    zr_bi_init(&br);
    zr_bi_init(&rem);
    if (op == OP_ADD) zr_bi_add(&br, &ba, &bb);
    else if (op == OP_SUB) zr_bi_sub(&br, &ba, &bb);
    else if (op == OP_MUL) zr_bi_mul(&br, &ba, &bb);
    else if (op == OP_DIV) {
        if (!zr_bi_divmod(&br, &rem, &ba, &bb)) { zr_bi_free(&ba); zr_bi_free(&bb); return vm_error(vm, "division by zero"); }
        zr_bi_free(&rem);
    } else if (op == OP_MOD) {
        if (!zr_bi_divmod(&br, &rem, &ba, &bb)) { zr_bi_free(&ba); zr_bi_free(&bb); return vm_error(vm, "modulo by zero"); }
        zr_bi_free(&br);
        br = rem;
    } else if (op == OP_POW) {
        if (bb.sign < 0) { zr_bi_free(&ba); zr_bi_free(&bb); return vm_error(vm, "bigint ** negative exponent requires rational()"); }
        uint64_t exp_u = (uint64_t)zr_bi_to_f64(&bb);
        if (!zr_bi_pow(&br, &ba, exp_u)) { zr_bi_free(&ba); zr_bi_free(&bb); return vm_error(vm, "bigint exponent too large"); }
    }
    zr_bi_free(&ba); zr_bi_free(&bb);
    *out = pb_bigint_new_take(vm, &br);
    return true;
}

static bool vm_bignum_cmp(PbVM *vm, PbValue a, PbValue b, int *out_cmp) {
    if (is_obj_type(a, OBJ_RATIONAL) || is_obj_type(b, OBJ_RATIONAL)) {
        ZrRational ra, rb;
        if (!zr_extract_rational(a, &ra) || !zr_extract_rational(b, &rb)) return vm_error(vm, "cannot compare values");
        ZrBigInt lhs, rhs;
        zr_bi_mul(&lhs, &ra.num, &rb.den);
        zr_bi_mul(&rhs, &rb.num, &ra.den);
        *out_cmp = zr_bi_cmp(&lhs, &rhs);
        zr_bi_free(&lhs); zr_bi_free(&rhs); zr_rat_free(&ra); zr_rat_free(&rb);
        return true;
    }
    if (is_obj_type(a, OBJ_DECIMAL) || is_obj_type(b, OBJ_DECIMAL)) {
        ZrDecimal da, db;
        if (!zr_extract_decimal(a, &da) || !zr_extract_decimal(b, &db)) return vm_error(vm, "cannot compare values");
        int32_t sc = da.scale > db.scale ? da.scale : db.scale;
        while (da.scale < sc) { zr_bi_mul_u32_ip(&da.coeff, 10); da.scale++; }
        while (db.scale < sc) { zr_bi_mul_u32_ip(&db.coeff, 10); db.scale++; }
        *out_cmp = zr_bi_cmp(&da.coeff, &db.coeff);
        zr_dec_free(&da); zr_dec_free(&db);
        return true;
    }
    if (is_obj_type(a, OBJ_BIGINT) || is_obj_type(b, OBJ_BIGINT)) {
        if (is_obj_type(a, OBJ_BIGINT) && is_obj_type(b, OBJ_BIGINT)) {
            *out_cmp = zr_bi_cmp(&AS_BIGINT(a)->v, &AS_BIGINT(b)->v);
            return true;
        }
        double fa = is_obj_type(a, OBJ_BIGINT) ? zr_bi_to_f64(&AS_BIGINT(a)->v) : a.as.num;
        double fb = is_obj_type(b, OBJ_BIGINT) ? zr_bi_to_f64(&AS_BIGINT(b)->v) : b.as.num;
        if (floor(fa) == fa && floor(fb) == fb) {
            ZrBigInt ba, bb;
            if (zr_extract_bigint(a, &ba) && zr_extract_bigint(b, &bb)) {
                *out_cmp = zr_bi_cmp(&ba, &bb);
                zr_bi_free(&ba); zr_bi_free(&bb);
                return true;
            }
            zr_bi_free(&ba); zr_bi_free(&bb);
        }
        *out_cmp = (fa < fb) ? -1 : ((fa > fb) ? 1 : 0);
        return true;
    }
    return vm_error(vm, "cannot compare non-numeric values");
}

/* ---- Upvalues (Stack-Index Based — Safe Across Stack Reallocations!) ---- */

static PbUpvalue *vm_capture_upvalue(PbVM *vm, size_t stack_idx) {
    PbUpvalue *prev = NULL;
    PbUpvalue *cur = vm->open_upvalues;
    while (cur != NULL && cur->stack_idx > stack_idx) {
        prev = cur;
        cur = cur->next_open;
    }
    if (cur != NULL && cur->stack_idx == stack_idx) return cur;

    PbUpvalue *created = (PbUpvalue *)vm_alloc_obj(vm, sizeof(PbUpvalue), OBJ_UPVALUE);
    created->is_open = true;
    created->stack_idx = stack_idx;
    created->closed = val_nil();
    created->next_open = cur;
    if (prev == NULL) vm->open_upvalues = created;
    else prev->next_open = created;
    return created;
}

static void vm_close_upvalues_from(PbVM *vm, size_t from_stack_idx) {
    while (vm->open_upvalues != NULL && vm->open_upvalues->stack_idx >= from_stack_idx) {
        PbUpvalue *uv = vm->open_upvalues;
        uv->closed = (uv->stack_idx < vm->sp) ? vm->stack[uv->stack_idx] : val_nil();
        vm_write_barrier(vm, (PbObj *)uv, uv->closed);
        uv->is_open = false;
        vm->open_upvalues = uv->next_open;
        uv->next_open = NULL;
    }
}

/* ---- Two-Generation Nursery + Full Mark-and-Sweep Garbage Collector ---- */

static void gc_mark_obj(PbObj *o);
static void gc_mark_nursery_obj(PbObj *o);

static inline void gc_mark_val(PbValue v) {
    if (v.tag == VAL_OBJ && v.as.obj != NULL) gc_mark_obj(v.as.obj);
}

static inline void gc_mark_nursery_val(PbValue v) {
    if (v.tag == VAL_OBJ && v.as.obj != NULL && v.as.obj->gc_gen == 0) {
        gc_mark_nursery_obj(v.as.obj);
    }
}

static void gc_mark_obj(PbObj *o) {
    if (!o || o->marked) return;
    o->marked = 1;
    switch (o->type) {
        case OBJ_STR:
        case OBJ_BIGINT:
        case OBJ_RATIONAL:
        case OBJ_DECIMAL:
        case OBJ_BYTES:
        case OBJ_F64BUF:
            break;
        case OBJ_ARRAY: {
            PbArray *a = (PbArray *)o;
            for (size_t i = 0; i < a->len; i++) gc_mark_val(a->items[i]);
            break;
        }
        case OBJ_MAP: {
            PbMap *m = (PbMap *)o;
            for (size_t i = 0; i < m->used; i++) {
                if (m->entries[i].active) {
                    gc_mark_obj((PbObj *)m->entries[i].key);
                    gc_mark_val(m->entries[i].val);
                }
            }
            break;
        }
        case OBJ_FUNC: {
            PbFunc *fn = (PbFunc *)o;
            if (fn->name) gc_mark_obj((PbObj *)fn->name);
            if (fn->source_name) gc_mark_obj((PbObj *)fn->source_name);
            for (uint32_t i = 0; i < fn->n_defaults; i++) gc_mark_val(fn->defaults[i]);
            for (uint32_t i = 0; i < fn->const_len; i++) gc_mark_val(fn->consts[i]);
            break;
        }
        case OBJ_UPVALUE: {
            PbUpvalue *uv = (PbUpvalue *)o;
            if (!uv->is_open) gc_mark_val(uv->closed);
            break;
        }
        case OBJ_CLOSURE: {
            PbClosure *cl = (PbClosure *)o;
            gc_mark_obj((PbObj *)cl->fn);
            if (cl->module) gc_mark_obj((PbObj *)cl->module);
            for (uint32_t i = 0; i < cl->upvalue_count; i++) {
                if (cl->upvalues[i]) gc_mark_obj((PbObj *)cl->upvalues[i]);
            }
            break;
        }
        case OBJ_MODULE: {
            PbModule *mod = (PbModule *)o;
            if (mod->name) gc_mark_obj((PbObj *)mod->name);
            if (mod->path) gc_mark_obj((PbObj *)mod->path);
            if (mod->globals) gc_mark_obj((PbObj *)mod->globals);
            if (mod->exports) gc_mark_obj((PbObj *)mod->exports);
            break;
        }
    }
}

static void gc_scan_children_nursery(PbObj *o) {
    switch (o->type) {
        case OBJ_STR:
        case OBJ_BIGINT:
        case OBJ_RATIONAL:
        case OBJ_DECIMAL:
        case OBJ_BYTES:
        case OBJ_F64BUF:
            break;
        case OBJ_ARRAY: {
            PbArray *a = (PbArray *)o;
            for (size_t i = 0; i < a->len; i++) gc_mark_nursery_val(a->items[i]);
            break;
        }
        case OBJ_MAP: {
            PbMap *m = (PbMap *)o;
            for (size_t i = 0; i < m->used; i++) {
                if (m->entries[i].active) {
                    if (m->entries[i].key && m->entries[i].key->obj.gc_gen == 0)
                        gc_mark_nursery_obj((PbObj *)m->entries[i].key);
                    gc_mark_nursery_val(m->entries[i].val);
                }
            }
            break;
        }
        case OBJ_FUNC: {
            PbFunc *fn = (PbFunc *)o;
            if (fn->name && fn->name->obj.gc_gen == 0) gc_mark_nursery_obj((PbObj *)fn->name);
            if (fn->source_name && fn->source_name->obj.gc_gen == 0) gc_mark_nursery_obj((PbObj *)fn->source_name);
            for (uint32_t i = 0; i < fn->n_defaults; i++) gc_mark_nursery_val(fn->defaults[i]);
            for (uint32_t i = 0; i < fn->const_len; i++) gc_mark_nursery_val(fn->consts[i]);
            break;
        }
        case OBJ_UPVALUE: {
            PbUpvalue *uv = (PbUpvalue *)o;
            if (!uv->is_open) gc_mark_nursery_val(uv->closed);
            break;
        }
        case OBJ_CLOSURE: {
            PbClosure *cl = (PbClosure *)o;
            if (cl->fn && cl->fn->obj.gc_gen == 0) gc_mark_nursery_obj((PbObj *)cl->fn);
            if (cl->module && cl->module->obj.gc_gen == 0) gc_mark_nursery_obj((PbObj *)cl->module);
            for (uint32_t i = 0; i < cl->upvalue_count; i++) {
                if (cl->upvalues[i] && cl->upvalues[i]->obj.gc_gen == 0)
                    gc_mark_nursery_obj((PbObj *)cl->upvalues[i]);
            }
            break;
        }
        case OBJ_MODULE: {
            PbModule *mod = (PbModule *)o;
            if (mod->name && mod->name->obj.gc_gen == 0) gc_mark_nursery_obj((PbObj *)mod->name);
            if (mod->path && mod->path->obj.gc_gen == 0) gc_mark_nursery_obj((PbObj *)mod->path);
            if (mod->globals && mod->globals->obj.gc_gen == 0) gc_mark_nursery_obj((PbObj *)mod->globals);
            if (mod->exports && mod->exports->obj.gc_gen == 0) gc_mark_nursery_obj((PbObj *)mod->exports);
            break;
        }
    }
}

static void gc_mark_nursery_obj(PbObj *o) {
    if (!o || o->gc_gen != 0 || o->marked) return;
    o->marked = 1;
    gc_scan_children_nursery(o);
}

static void gc_free_obj(PbVM *vm, PbObj *o) {
    if (vm->live_bytes >= o->tracked_bytes) vm->live_bytes -= o->tracked_bytes;
    else vm->live_bytes = 0;
    switch (o->type) {
        case OBJ_STR:
            break;
        case OBJ_BYTES: {
            PbBytes *b = (PbBytes *)o;
            if (b->backing) zr_shared_release(b->backing);
            else free(b->data);
            break;
        }
        case OBJ_F64BUF: {
            PbF64Buf *b = (PbF64Buf *)o;
            if (b->backing) zr_shared_release(b->backing);
            else free(b->data);
            break;
        }
        case OBJ_ARRAY:
            free(((PbArray *)o)->items);
            break;
        case OBJ_MAP:
            free(((PbMap *)o)->entries);
            free(((PbMap *)o)->buckets);
            break;
        case OBJ_FUNC: {
            PbFunc *fn = (PbFunc *)o;
            zr_jit_free_code(fn);
            for (uint32_t i = 0; i < fn->code_len; i++) free(fn->code[i].caps);
            free(fn->code);
            free(fn->defaults);
            free(fn->consts);
            for (size_t i = 0; i < fn->top_test_fn_count; i++) free(fn->top_test_fns[i]);
            free(fn->top_test_fns);
            for (size_t i = 0; i < fn->poly_count; i++) {
                free(fn->poly_blocks[i].lang);
                free(fn->poly_blocks[i].name);
                for (size_t a = 0; a < fn->poly_blocks[i].attr_count; a++) {
                    free(fn->poly_blocks[i].attr_keys[a]);
                    free(fn->poly_blocks[i].attr_vals[a]);
                }
                free(fn->poly_blocks[i].attr_keys);
                free(fn->poly_blocks[i].attr_vals);
                free(fn->poly_blocks[i].code);
            }
            free(fn->poly_blocks);
            break;
        }
        case OBJ_CLOSURE:
            free(((PbClosure *)o)->upvalues);
            break;
        case OBJ_UPVALUE:
        case OBJ_MODULE:
            break;
        case OBJ_BIGINT:
            zr_bi_free(&((PbBigIntObj *)o)->v);
            break;
        case OBJ_RATIONAL:
            zr_rat_free(&((PbRationalObj *)o)->v);
            break;
        case OBJ_DECIMAL:
            zr_dec_free(&((PbDecimalObj *)o)->v);
            break;
    }
    free(o);
}

static void vm_collect_nursery(PbVM *vm) {
    if (vm->gc_inhibit > 0) return;
    if (vm->minor_since_full >= 7 && vm->live_bytes > vm->next_gc_bytes) {
        vm_collect_garbage(vm, NULL, NULL, NULL);
        return;
    }
    vm->gc_inhibit++;

    for (size_t i = 0; i < vm->sp; i++) gc_mark_nursery_val(vm->stack[i]);
    for (size_t i = 0; i < vm->temp_root_count; i++) gc_mark_nursery_val(vm->temp_roots[i]);
    for (size_t i = 0; i < vm->frame_count; i++) {
        if (vm->frames[i].closure && vm->frames[i].closure->obj.gc_gen == 0)
            gc_mark_nursery_obj((PbObj *)vm->frames[i].closure);
        for (size_t d = 0; d < vm->frames[i].defer_len; d++) {
            gc_mark_nursery_val(vm->frames[i].defers[d]);
        }
    }
    for (PbUpvalue *uv = vm->open_upvalues; uv != NULL; uv = uv->next_open) {
        if (uv->obj.gc_gen == 0) gc_mark_nursery_obj((PbObj *)uv);
    }
    if (vm->globals && vm->globals->obj.gc_gen == 0) gc_mark_nursery_obj((PbObj *)vm->globals);
    if (vm->exports && vm->exports->obj.gc_gen == 0) gc_mark_nursery_obj((PbObj *)vm->exports);
    if (vm->builtins && vm->builtins->obj.gc_gen == 0) gc_mark_nursery_obj((PbObj *)vm->builtins);
    if (vm->modules && vm->modules->obj.gc_gen == 0) gc_mark_nursery_obj((PbObj *)vm->modules);
    if (vm->signal_handlers && vm->signal_handlers->obj.gc_gen == 0) gc_mark_nursery_obj((PbObj *)vm->signal_handlers);
    zr_ffi_gc_mark_nursery_roots(vm);

    for (size_t i = 0; i < vm->rem_len; i++) {
        PbObj *p = vm->rem_set[i];
        if (p) {
            gc_scan_children_nursery(p);
            p->gc_rem = 0;
        }
    }
    vm->rem_len = 0;

    if (g_zr_null_vm_barrier != 0) {
        for (PbObj *p = vm->objects; p != NULL; p = p->next) {
            if (p->gc_rem) {
                gc_scan_children_nursery(p);
                p->gc_rem = 0;
            }
        }
        g_zr_null_vm_barrier = 0;
    }

    PbObj *cur = vm->nursery_objects;
    vm->nursery_objects = NULL;
    while (cur != NULL) {
        PbObj *next = cur->next;
        if (cur->marked) {
            cur->marked = 0;
            cur->gc_gen = 1;
            cur->gc_rem = 0;
            cur->next = vm->objects;
            vm->objects = cur;
        } else {
            vm->obj_count--;
            gc_free_obj(vm, cur);
        }
        cur = next;
    }

    vm->nursery_bytes = 0;
    vm->nursery_collections++;
    vm->gc_collections++;
    vm->minor_since_full++;
    vm->gc_inhibit--;
}

static void vm_collect_garbage(PbVM *vm, size_t *out_arr, size_t *out_map, size_t *out_mod) {
    if (vm->gc_inhibit > 0) return;
    vm->gc_inhibit++;

    /* Splice nursery into main object list before full mark-and-sweep */
    PbObj *ncur = vm->nursery_objects;
    vm->nursery_objects = NULL;
    while (ncur != NULL) {
        PbObj *nnext = ncur->next;
        ncur->gc_gen = 1;
        ncur->gc_rem = 0;
        ncur->next = vm->objects;
        vm->objects = ncur;
        ncur = nnext;
    }
    vm->nursery_bytes = 0;
    for (size_t i = 0; i < vm->rem_len; i++) {
        if (vm->rem_set[i]) vm->rem_set[i]->gc_rem = 0;
    }
    vm->rem_len = 0;

    for (size_t i = 0; i < vm->sp; i++) gc_mark_val(vm->stack[i]);
    for (size_t i = 0; i < vm->temp_root_count; i++) gc_mark_val(vm->temp_roots[i]);
    for (size_t i = 0; i < vm->frame_count; i++) {
        if (vm->frames[i].closure) gc_mark_obj((PbObj *)vm->frames[i].closure);
        for (size_t d = 0; d < vm->frames[i].defer_len; d++) {
            gc_mark_val(vm->frames[i].defers[d]);
        }
    }
    for (PbUpvalue *uv = vm->open_upvalues; uv != NULL; uv = uv->next_open) {
        gc_mark_obj((PbObj *)uv);
    }
    if (vm->globals) gc_mark_obj((PbObj *)vm->globals);
    if (vm->exports) gc_mark_obj((PbObj *)vm->exports);
    if (vm->builtins) gc_mark_obj((PbObj *)vm->builtins);
    if (vm->modules) gc_mark_obj((PbObj *)vm->modules);
    if (vm->signal_handlers) gc_mark_obj((PbObj *)vm->signal_handlers);
    zr_ffi_gc_mark_roots(vm);

    size_t swept_arr = 0, swept_map = 0, swept_mod = 0;
    PbObj **link = &vm->objects;
    while (*link != NULL) {
        PbObj *cur = *link;
        if (cur->marked) {
            cur->marked = 0;
            cur->gc_gen = 1;
            cur->gc_rem = 0;
            link = &cur->next;
        } else {
            *link = cur->next;
            if (cur->type == OBJ_ARRAY) swept_arr++;
            else if (cur->type == OBJ_MAP) swept_map++;
            else if (cur->type == OBJ_MODULE) swept_mod++;
            vm->obj_count--;
            gc_free_obj(vm, cur);
        }
    }

    size_t min_thresh = (sizeof(void *) <= 4) ? (512u * 1024u) : (2u * 1024u * 1024u);
    size_t doubled = vm->live_bytes * 2;
    vm->next_gc_bytes = (doubled > min_thresh) ? doubled : min_thresh;
    vm->full_collections++;
    vm->gc_collections++;
    vm->minor_since_full = 0;
    vm->gc_inhibit--;

    if (out_arr) *out_arr = swept_arr;
    if (out_map) *out_map = swept_map;
    if (out_mod) *out_mod = swept_mod;
}

/* ---- Hotspot Profiler Frame Accounting ---- */

static inline void vm_prof_enter_frame(PbVM *vm, CallFrame *fr) {
    if (vm->prof_enabled) {
        fr->prof_enter_ns = zr_mono_ns();
        fr->prof_child_ns = 0;
    } else {
        fr->prof_enter_ns = 0;
        fr->prof_child_ns = 0;
    }
}

static inline void vm_prof_exit_frame(PbVM *vm, CallFrame *fr, size_t frame_idx) {
    if (!vm->prof_enabled || fr->prof_enter_ns == 0 || !fr->closure || !fr->closure->fn) return;
    uint64_t now = zr_mono_ns();
    uint64_t total = (now >= fr->prof_enter_ns) ? (now - fr->prof_enter_ns) : 0;
    uint64_t self_ns = (total >= fr->prof_child_ns) ? (total - fr->prof_child_ns) : 0;
    PbFunc *fn = fr->closure->fn;
    fn->prof_calls++;
    fn->prof_total_ns += total;
    fn->prof_self_ns += self_ns;
    if (frame_idx > 0) {
        vm->frames[frame_idx - 1].prof_child_ns += total;
    }
    fr->prof_enter_ns = 0;
}

/* ---- Call Argument Preparation (Matches vm.rs lines 2682-2729!) ---- */

static bool vm_prepare_closure_args(PbVM *vm, PbFunc *fn, uint32_t argc, size_t callee_slot) {
    if (fn->is_vararg) {
        /* Note: in .pbck v1 & compiler, fn->arity is ALREADY fixed param count before ...rest! */
        uint32_t fixed = fn->arity;
        if (argc < fn->min_arity) {
            return vm_error(vm, "arity mismatch: '%s' expected at least %u arguments, got %u",
                            fn->name ? fn->name->chars : "<fn>", fn->min_arity, argc);
        }
        vm_ensure_stack(vm, (size_t)fixed + 8);
        while (argc < fixed) {
            uint32_t def_i = argc >= fn->min_arity ? (argc - fn->min_arity) : 0;
            PbValue dv = (def_i < fn->n_defaults) ? fn->defaults[def_i] : val_nil();
            vm_push(vm, dv);
            argc++;
        }
        uint32_t extra = argc - fixed;
        PbArray *rest = pb_array_new(vm, extra);
        vm_push_root(vm, val_obj(rest));
        size_t rest_start = callee_slot + 1 + fixed;
        for (uint32_t i = 0; i < extra; i++) {
            pb_array_push(vm, rest, vm->stack[rest_start + i]);
        }
        vm->sp = rest_start;
        vm_pop_roots(vm, 1);
        vm_push(vm, val_obj(rest));
        return true;
    }

    if (argc < fn->min_arity || argc > fn->arity) {
        return vm_error(vm, "arity mismatch: '%s' expected %u..%u arguments, got %u",
                        fn->name ? fn->name->chars : "<fn>", fn->min_arity, fn->arity, argc);
    }
    vm_ensure_stack(vm, (size_t)(fn->arity - argc) + 8);
    while (argc < fn->arity) {
        uint32_t def_i = argc - fn->min_arity;
        PbValue dv = (def_i < fn->n_defaults) ? fn->defaults[def_i] : val_nil();
        vm_push(vm, dv);
        argc++;
    }
    return true;
}

#if defined(__GNUC__) || defined(__clang__)
__attribute__((noinline))
#endif
static void zr_maybe_tier2_jit(PbVM *vm, PbFunc *fn) {
    size_t err_sz = sizeof(vm->err_msg);
    char *saved_err = (char *)malloc(err_sz);
    if (saved_err) memcpy(saved_err, vm->err_msg, err_sz);
    (void)zr_jit_compile_func(vm, fn);
    if (saved_err) {
        memcpy(vm->err_msg, saved_err, err_sz);
        free(saved_err);
    }
}

static bool vm_call_top(PbVM *vm, uint32_t argc, bool is_tail, size_t min_tail_depth) {
    if (vm->sp < (size_t)argc + 1) return vm_error(vm, "stack underflow on call");
    size_t callee_slot = vm->sp - 1 - (size_t)argc;

retry_call:;
    PbValue callee = vm->stack[callee_slot];
    if (callee.tag == VAL_BUILTIN) {
        if (callee.as.builtin_id >= vm->builtin_count) return vm_error(vm, "invalid builtin id");
        PbValue ret = val_nil();
        PbValue sbuf[16];
        PbValue *arg_copy = (argc <= 16) ? sbuf : (PbValue *)zr_xmalloc((size_t)argc * sizeof(PbValue));
        for (uint32_t i = 0; i < argc; i++) arg_copy[i] = vm->stack[callee_slot + 1 + i];
        bool bok = vm->builtin_defs[callee.as.builtin_id].fn(vm, (int)argc, arg_copy, &ret);
        if (arg_copy != sbuf) free(arg_copy);
        if (!bok) return false;
        vm->sp = callee_slot;
        vm_push(vm, ret);
        if (is_tail && vm->frame_count > min_tail_depth && vm->frames[vm->frame_count - 1].defer_len == 0) {
            bool handler_in_frame = false;
            for (size_t h = 0; h < vm->handler_count; h++) {
                if (vm->handlers[h].frame_idx == vm->frame_count - 1) {
                    handler_in_frame = true;
                    break;
                }
            }
            if (!handler_in_frame) {
                size_t cur_idx = vm->frame_count - 1;
                CallFrame *cur = &vm->frames[cur_idx];
                vm_prof_exit_frame(vm, cur, cur_idx);
                vm_close_upvalues_from(vm, cur->base + 1);
                size_t base = cur->base;
                vm->frame_count--;
                vm->sp = base;
                vm_push(vm, ret);
            }
        }
        return true;
    }

    PbClosure *cl = NULL;
    if (is_obj_type(callee, OBJ_CLOSURE)) {
        cl = AS_CLOSURE(callee);
    } else if (is_obj_type(callee, OBJ_FUNC)) {
        PbModule *cur_mod = (vm->frame_count > 0) ? vm->frames[vm->frame_count - 1].closure->module : NULL;
        cl = pb_closure_new(vm, AS_FUNC(callee), 0, cur_mod);
        vm->stack[callee_slot] = val_obj(cl);
    } else if (is_obj_type(callee, OBJ_MAP)) {
        PbValue call_fn;
        if (pb_map_get_cstr(AS_MAP(callee), "__call", &call_fn) && call_fn.tag != VAL_NIL) {
            vm->stack[callee_slot] = call_fn;
            goto retry_call;
        }
        if (vm_lookup_map_dunder(callee, "__call", &call_fn)) {
            /* Instance method __call(self, ...args) */
            vm_ensure_stack(vm, 2);
            for (int64_t k = (int64_t)argc; k >= 0; k--) {
                vm->stack[callee_slot + 1 + (size_t)k] = vm->stack[callee_slot + (size_t)k];
            }
            vm->stack[callee_slot] = call_fn;
            vm->sp++;
            argc++;
            goto retry_call;
        }
        return vm_error(vm, "can only call function/closure/builtin (map has no __call)");
    } else {
        return vm_error(vm, "can only call function/closure/builtin, got %s", zr_val_type_name(callee));
    }

    if (!vm_prepare_closure_args(vm, cl->fn, argc, callee_slot)) return false;

    /* Tier-2 W^X Native JIT compilation trigger (at 32 calls) & fastpath execution */
    PbFunc *fn = cl->fn;
    fn->call_count++;
    if (fn->call_count == 32 && !fn->jit_attempted) {
        zr_maybe_tier2_jit(vm, fn);
    }
    if (fn->jit_fn != NULL && !fn->is_vararg && fn->arity <= 8) {
        bool jit_ok = true;
        if (fn->jit_self_kind == 1) {
            PbMap *gmap = cl->module ? cl->module->globals : vm->globals;
            PbValue gv;
            if (fn->jit_self_arg >= fn->const_len || !is_obj_type(fn->consts[fn->jit_self_arg], OBJ_STR) ||
                !pb_map_get(gmap, AS_STR(fn->consts[fn->jit_self_arg]), &gv) ||
                !is_obj_type(gv, OBJ_CLOSURE) || AS_CLOSURE(gv)->fn != fn) {
                jit_ok = false;
            }
        } else if (fn->jit_self_kind == 2) {
            if (cl->upvalue_count < 1 || !cl->upvalues[0]) {
                jit_ok = false;
            } else {
                PbUpvalue *uv0 = cl->upvalues[0];
                PbValue uvv = uv0->is_open ? vm->stack[uv0->stack_idx] : uv0->closed;
                if (!is_obj_type(uvv, OBJ_CLOSURE) || AS_CLOSURE(uvv)->fn != fn) jit_ok = false;
            }
        }
        double jargs[8];
        if (jit_ok) {
            for (uint32_t i = 0; i < fn->arity; i++) {
                PbValue av = vm->stack[callee_slot + 1 + i];
                if (av.tag != VAL_NUM || av.taint != 0) {
                    jit_ok = false;
                    break;
                }
                jargs[i] = av.as.num;
            }
        }
        if (jit_ok) {
            int bailout = 0;
            uint64_t t0 = vm->prof_enabled ? zr_mono_ns() : 0;
            double jres = fn->jit_fn(jargs, &bailout);
            if (bailout == 0) {
                vm->jit_invocations++;
                if (vm->prof_enabled) {
                    uint64_t dt = zr_mono_ns() - t0;
                    fn->prof_calls++;
                    fn->prof_total_ns += dt;
                    fn->prof_self_ns += dt;
                    if (vm->frame_count > 0) vm->frames[vm->frame_count - 1].prof_child_ns += dt;
                }
                PbValue ret = fn->jit_ret_bool ? val_bool(jres != 0.0) : val_num(jres);
                vm->sp = callee_slot;
                vm_push(vm, ret);
                if (is_tail && vm->frame_count > min_tail_depth && vm->frames[vm->frame_count - 1].defer_len == 0) {
                    bool handler_in_frame = false;
                    for (size_t h = 0; h < vm->handler_count; h++) {
                        if (vm->handlers[h].frame_idx == vm->frame_count - 1) {
                            handler_in_frame = true;
                            break;
                        }
                    }
                    if (!handler_in_frame) {
                        size_t cur_idx = vm->frame_count - 1;
                        CallFrame *cur = &vm->frames[cur_idx];
                        vm_prof_exit_frame(vm, cur, cur_idx);
                        vm_close_upvalues_from(vm, cur->base + 1);
                        size_t base = cur->base;
                        vm->frame_count--;
                        vm->sp = base;
                        vm_push(vm, ret);
                    }
                }
                return true;
            }
        }
    }

    uint32_t mod_mask = cl->module ? cl->module->cap_mask : CAP_ALL;

    if (is_tail && vm->frame_count > min_tail_depth && vm->frames[vm->frame_count - 1].defer_len == 0) {
        bool handler_in_frame = false;
        for (size_t h = 0; h < vm->handler_count; h++) {
            if (vm->handlers[h].frame_idx == vm->frame_count - 1) {
                handler_in_frame = true;
                break;
            }
        }
        if (!handler_in_frame) {
            size_t cur_idx = vm->frame_count - 1;
            CallFrame *cur = &vm->frames[cur_idx];
            vm_prof_exit_frame(vm, cur, cur_idx);
            vm_close_upvalues_from(vm, cur->base + 1);
            size_t new_count = vm->sp - callee_slot;
            memmove(&vm->stack[cur->base], &vm->stack[callee_slot], new_count * sizeof(PbValue));
            vm->sp = cur->base + new_count;
            cur->closure = cl;
            cur->ip = 0;
            cur->cap_mask &= mod_mask;
            vm_prof_enter_frame(vm, cur);
            return true;
        }
    }

    if (vm->frame_count >= ZR_MAX_FRAMES) {
        return vm_error(vm, "call stack overflow (max %d frames)", ZR_MAX_FRAMES);
    }
    uint32_t parent_mask = (vm->frame_count > 0) ? vm->frames[vm->frame_count - 1].cap_mask : CAP_ALL;
    CallFrame *fr = &vm->frames[vm->frame_count++];
    fr->closure = cl;
    fr->ip = 0;
    fr->base = callee_slot;
    fr->cap_mask = parent_mask & mod_mask;
    fr->defers = NULL;
    fr->defer_len = 0;
    fr->defer_cap = 0;
    vm_prof_enter_frame(vm, fr);
    return true;
}

static void vm_run_frame_defers(PbVM *vm, CallFrame *fr, size_t down_to) {
    while (fr->defer_len > down_to) {
        PbValue dcallee = fr->defers[--fr->defer_len];
        size_t saved_sp = vm->sp;
        char saved_err[2048];
        memcpy(saved_err, vm->err_msg, sizeof(saved_err));
        PbValue unused = val_nil();
        vm_push_root(vm, dcallee);
        (void)vm_call_value_sync(vm, dcallee, 0, NULL, &unused);
        vm_pop_roots(vm, 1);
        memcpy(vm->err_msg, saved_err, sizeof(saved_err));
        vm->sp = saved_sp;
    }
    if (down_to == 0 && fr->defers) {
        free(fr->defers);
        fr->defers = NULL;
        fr->defer_cap = 0;
    }
}

static inline PbMap *vm_frame_globals(PbVM *vm, const CallFrame *fr) {
    return (fr->closure && fr->closure->module) ? fr->closure->module->globals : vm->globals;
}

static inline PbMap *vm_frame_exports(PbVM *vm, const CallFrame *fr) {
    return (fr->closure && fr->closure->module) ? fr->closure->module->exports : vm->exports;
}

/* ---- Hot-Loop OSR IEEE-754 Register Fast-Path for Straight-Line Numeric Loops ---- */

static inline uint8_t zr_orig_op_at(const PbFunc *fn, uint32_t idx) {
    uint8_t o = fn->code[idx].orig_op;
    return (o == 0 && fn->code[idx].op <= OP_THROW) ? fn->code[idx].op : o;
}

static int zr_osr_bind_var(PbVM *vm, CallFrame *frame, PbFunc *fn, PbMap *gmap,
                           bool is_global, uint32_t arg,
                           PbValue **vptrs, uint8_t *vkinds, uint32_t *vargs, int *nvars) {
    uint8_t k = is_global ? 1u : 0u;
    for (int i = 0; i < *nvars; i++) {
        if (vkinds[i] == k && vargs[i] == arg) return i;
    }
    if (*nvars >= 8) return -1;
    PbValue *vp = NULL;
    if (!is_global) {
        size_t s = frame->base + 1u + (size_t)arg;
        if (s >= vm->sp) return -1;
        vp = &vm->stack[s];
    } else {
        if (arg >= fn->const_len || !is_obj_type(fn->consts[arg], OBJ_STR)) return -1;
        PbStr *nm = AS_STR(fn->consts[arg]);
        int32_t eidx = pb_map_find_entry_idx(gmap, nm->chars, nm->len, nm->hash);
        if (eidx < 0) return -1;
        vp = &gmap->entries[eidx].val;
    }
    if (vp->tag != VAL_NUM || vp->taint != 0) return -1;
    int idx = (*nvars)++;
    vptrs[idx] = vp;
    vkinds[idx] = k;
    vargs[idx] = arg;
    return idx;
}

static inline bool zr_osr_cmp(uint8_t cmp_op, double a, double b) {
    if (cmp_op == OP_LT) return a < b;
    if (cmp_op == OP_LE) return a <= b;
    if (cmp_op == OP_GT) return a > b;
    return a >= b;
}

/* Executes remaining iterations of a straight-line pure-numeric while loop in
 * IEEE-754 hardware double registers when back-edge count reaches 32. */
static bool zr_try_osr_numeric_loop(PbVM *vm, CallFrame *frame, PbFunc *fn,
                                    uint32_t head_ip, uint32_t jump_ip) {
    if (head_ip + 5u > jump_ip || jump_ip + 1u >= fn->code_len) return false;
    uint8_t h0 = zr_orig_op_at(fn, head_ip);
    uint8_t h1 = zr_orig_op_at(fn, head_ip + 1u);
    uint8_t h2 = zr_orig_op_at(fn, head_ip + 2u);
    uint8_t h3 = zr_orig_op_at(fn, head_ip + 3u);
    uint8_t h4 = zr_orig_op_at(fn, head_ip + 4u);
    if ((h0 != OP_GET_LOCAL && h0 != OP_GET_GLOBAL) ||
        (h1 != OP_CONSTANT && h1 != OP_GET_LOCAL && h1 != OP_GET_GLOBAL) ||
        (h2 != OP_LT && h2 != OP_LE && h2 != OP_GT && h2 != OP_GE) ||
        h3 != OP_JUMP_IF_FALSE || fn->code[head_ip + 3u].arg != jump_ip + 1u ||
        h4 != OP_POP) {
        return false;
    }

    PbMap *gmap = vm_frame_globals(vm, frame);
    PbValue *vptrs[8];
    uint8_t vkinds[8];
    uint32_t vargs[8];
    int nvars = 0;

    int lhs_var = zr_osr_bind_var(vm, frame, fn, gmap, h0 == OP_GET_GLOBAL,
                                  fn->code[head_ip].arg, vptrs, vkinds, vargs, &nvars);
    if (lhs_var < 0) return false;

    bool rhs_is_const = (h1 == OP_CONSTANT);
    double rhs_const = 0.0;
    int rhs_var = -1;
    if (rhs_is_const) {
        uint32_t ca = fn->code[head_ip + 1u].arg;
        if (ca >= fn->const_len || fn->consts[ca].tag != VAL_NUM || fn->consts[ca].taint != 0) return false;
        rhs_const = fn->consts[ca].as.num;
    } else {
        rhs_var = zr_osr_bind_var(vm, frame, fn, fn ? gmap : NULL, h1 == OP_GET_GLOBAL,
                                  fn->code[head_ip + 1u].arg, vptrs, vkinds, vargs, &nvars);
        if (rhs_var < 0) return false;
    }

    /* Decode body [head_ip + 5 .. jump_ip - 1] into register micro-ops */
    enum { MOP_CONST = 0, MOP_LOAD = 1, MOP_STORE = 2, MOP_ADD = 3, MOP_SUB = 4, MOP_MUL = 5, MOP_POP = 6 };
    uint8_t mop_kind[32];
    int mop_idx[32];
    double mop_imm[32];
    int nmops = 0;
    int sp_delta = 0;
    uint8_t written_mask = 0;

    for (uint32_t ip = head_ip + 5u; ip < jump_ip; ip++) {
        if (nmops >= 32) return false;
        uint8_t op = zr_orig_op_at(fn, ip);
        uint32_t arg = fn->code[ip].arg;
        /* Reject any internal jump target that lands inside the loop */
        if (op == OP_CONSTANT) {
            if (arg >= fn->const_len || fn->consts[arg].tag != VAL_NUM || fn->consts[arg].taint != 0) return false;
            if (sp_delta >= 8) return false;
            mop_kind[nmops] = MOP_CONST;
            mop_imm[nmops] = fn->consts[arg].as.num;
            mop_idx[nmops] = 0;
            nmops++;
            sp_delta++;
        } else if (op == OP_GET_LOCAL || op == OP_GET_GLOBAL) {
            if (sp_delta >= 8) return false;
            int vi = zr_osr_bind_var(vm, frame, fn, gmap, op == OP_GET_GLOBAL, arg, vptrs, vkinds, vargs, &nvars);
            if (vi < 0) return false;
            mop_kind[nmops] = MOP_LOAD;
            mop_idx[nmops] = vi;
            mop_imm[nmops] = 0.0;
            nmops++;
            sp_delta++;
        } else if (op == OP_SET_LOCAL || op == OP_SET_GLOBAL) {
            if (sp_delta < 1) return false;
            int vi = zr_osr_bind_var(vm, frame, fn, gmap, op == OP_SET_GLOBAL, arg, vptrs, vkinds, vargs, &nvars);
            if (vi < 0) return false;
            mop_kind[nmops] = MOP_STORE;
            mop_idx[nmops] = vi;
            mop_imm[nmops] = 0.0;
            nmops++;
            written_mask |= (uint8_t)(1u << vi);
            if (op == OP_SET_LOCAL) {
                sp_delta--;
            } else {
                /* Check whether OP_SET_GLOBAL pops in this VM: yes, zr_vm_core.h:2599 vm_pop(vm)! */
                sp_delta--;
            }
        } else if (op == OP_ADD || op == OP_SUB || op == OP_MUL) {
            if (sp_delta < 2) return false;
            mop_kind[nmops] = (op == OP_ADD) ? MOP_ADD : (op == OP_SUB) ? MOP_SUB : MOP_MUL;
            mop_idx[nmops] = 0;
            mop_imm[nmops] = 0.0;
            nmops++;
            sp_delta--;
        } else if (op == OP_POP) {
            if (sp_delta < 1) return false;
            mop_kind[nmops] = MOP_POP;
            mop_idx[nmops] = 0;
            mop_imm[nmops] = 0.0;
            nmops++;
            sp_delta--;
        } else {
            return false;
        }
    }
    if (sp_delta != 0) return false;

    double regs[8];
    for (int i = 0; i < nvars; i++) regs[i] = vptrs[i]->as.num;

    bool bound_invariant = rhs_is_const || ((written_mask & (uint8_t)(1u << rhs_var)) == 0);
    double bound_val = rhs_is_const ? rhs_const : regs[rhs_var];

    /* Specialized SSE2 scalar register path 1: 4 micro-ops `v_lhs = v_lhs +/- const` */
    if (bound_invariant && nmops == 4 &&
        mop_kind[0] == MOP_LOAD && mop_idx[0] == lhs_var &&
        mop_kind[1] == MOP_CONST &&
        (mop_kind[2] == MOP_ADD || mop_kind[2] == MOP_SUB) &&
        mop_kind[3] == MOP_STORE && mop_idx[3] == lhs_var) {
        double iv = regs[lhs_var];
        double step = (mop_kind[2] == MOP_SUB) ? -mop_imm[1] : mop_imm[1];
        while (!vm->exited && zr_osr_cmp(h2, iv, bound_val)) {
            uint32_t blk = 0;
            do {
                iv += step;
            } while (++blk < 262144u && zr_osr_cmp(h2, iv, bound_val));
        }
        regs[lhs_var] = iv;
    }
    /* Specialized SSE2 scalar register path 2: 8 micro-ops `acc = acc + (idx|const); idx = idx +/- step` */
    else if (bound_invariant && nmops == 8 &&
             mop_kind[0] == MOP_LOAD && mop_idx[0] != lhs_var &&
             (mop_kind[1] == MOP_LOAD || mop_kind[1] == MOP_CONST) &&
             (mop_kind[2] == MOP_ADD || mop_kind[2] == MOP_SUB) &&
             mop_kind[3] == MOP_STORE && mop_idx[3] == mop_idx[0] &&
             mop_kind[4] == MOP_LOAD && mop_idx[4] == lhs_var &&
             mop_kind[5] == MOP_CONST &&
             (mop_kind[6] == MOP_ADD || mop_kind[6] == MOP_SUB) &&
             mop_kind[7] == MOP_STORE && mop_idx[7] == lhs_var) {
        int acc_var = mop_idx[0];
        bool add_idx = (mop_kind[1] == MOP_LOAD && mop_idx[1] == lhs_var);
        bool add_cst = (mop_kind[1] == MOP_CONST);
        if (add_idx || add_cst) {
            double iv = regs[lhs_var];
            double sv = regs[acc_var];
            double step = (mop_kind[6] == MOP_SUB) ? -mop_imm[5] : mop_imm[5];
            double cadd = mop_imm[1];
            bool is_sub_acc = (mop_kind[2] == MOP_SUB);
            while (!vm->exited && zr_osr_cmp(h2, iv, bound_val)) {
                uint32_t blk = 0;
                if (add_idx && !is_sub_acc) {
                    do {
                        sv += iv;
                        iv += step;
                    } while (++blk < 262144u && zr_osr_cmp(h2, iv, bound_val));
                } else if (add_idx && is_sub_acc) {
                    do {
                        sv -= iv;
                        iv += step;
                    } while (++blk < 262144u && zr_osr_cmp(h2, iv, bound_val));
                } else {
                    double dacc = is_sub_acc ? -cadd : cadd;
                    do {
                        sv += dacc;
                        iv += step;
                    } while (++blk < 262144u && zr_osr_cmp(h2, iv, bound_val));
                }
            }
            regs[lhs_var] = iv;
            regs[acc_var] = sv;
        } else {
            goto generic_osr_eval;
        }
    } else {
    generic_osr_eval:;
        double estk[8];
        while (!vm->exited) {
            double lv = regs[lhs_var];
            double rv = rhs_is_const ? rhs_const : regs[rhs_var];
            if (!zr_osr_cmp(h2, lv, rv)) break;
            uint32_t blk = 0;
            do {
                int esp = 0;
                for (int m = 0; m < nmops; m++) {
                    switch (mop_kind[m]) {
                        case MOP_CONST: estk[esp++] = mop_imm[m]; break;
                        case MOP_LOAD:  estk[esp++] = regs[mop_idx[m]]; break;
                        case MOP_STORE: regs[mop_idx[m]] = estk[--esp]; break;
                        case MOP_ADD:   esp--; estk[esp - 1] += estk[esp]; break;
                        case MOP_SUB:   esp--; estk[esp - 1] -= estk[esp]; break;
                        case MOP_MUL:   esp--; estk[esp - 1] *= estk[esp]; break;
                        case MOP_POP:   esp--; break;
                    }
                }
                lv = regs[lhs_var];
                rv = rhs_is_const ? rhs_const : regs[rhs_var];
            } while (++blk < 65536u && zr_osr_cmp(h2, lv, rv));
        }
    }

    for (int i = 0; i < nvars; i++) vptrs[i]->as.num = regs[i];
    if (!vm->exited) {
        vm_push(vm, val_bool(false));
        frame->ip = jump_ip + 1u;
    }
    return true;
}

/* ---- Bytecode Dispatch Loop (Computed-Goto Threaded + Superinstructions + Monomorphic ICs) ---- */

static bool vm_execute_until(PbVM *vm, size_t stop_frame_depth, PbValue *out_ret) {
#define RUNTIME_ERR(...) do { vm_error(vm, __VA_ARGS__); goto handle_exception; } while (0)
#define ZR_RELOAD_FRAME() do { \
    frame = &vm->frames[vm->frame_count - 1]; \
    fn = frame->closure->fn; \
} while (0)

#if (defined(__GNUC__) || defined(__clang__)) && !defined(_WIN32)
    static const void *const disp_tab[256] = {
        /* 0..47: .pbck v1 wire opcodes */
        &&lbl_OP_CONSTANT,          /* 0 */
        &&lbl_OP_ADD,               /* 1 */
        &&lbl_OP_SUB,               /* 2 */
        &&lbl_OP_MUL,               /* 3 */
        &&lbl_OP_DIV,               /* 4 */
        &&lbl_OP_EQ,                /* 5 */
        &&lbl_OP_NOT_EQ,            /* 6 */
        &&lbl_OP_GT,                /* 7 */
        &&lbl_OP_GE,                /* 8 */
        &&lbl_OP_LT,                /* 9 */
        &&lbl_OP_LE,                /* 10 */
        &&lbl_OP_POP,               /* 11 */
        &&lbl_OP_PRINT,             /* 12 */
        &&lbl_OP_DEFINE_GLOBAL,     /* 13 */
        &&lbl_OP_GET_GLOBAL,        /* 14 */
        &&lbl_OP_SET_GLOBAL,        /* 15 */
        &&lbl_OP_GET_ATTR,          /* 16 */
        &&lbl_OP_SET_ATTR,          /* 17 */
        &&lbl_OP_EXPORT,            /* 18 */
        &&lbl_OP_MAKE_ARRAY,        /* 19 */
        &&lbl_OP_MAKE_MAP,          /* 20 */
        &&lbl_OP_GET_INDEX,         /* 21 */
        &&lbl_OP_SET_INDEX,         /* 22 */
        &&lbl_OP_TRY,               /* 23 */
        &&lbl_OP_END_TRY,           /* 24 */
        &&lbl_OP_GET_LOCAL,         /* 25 */
        &&lbl_OP_SET_LOCAL,         /* 26 */
        &&lbl_OP_CLOSURE,           /* 27 */
        &&lbl_OP_GET_UPVALUE,       /* 28 */
        &&lbl_OP_SET_UPVALUE,       /* 29 */
        &&lbl_OP_CLOSE_UPVALUE,     /* 30 */
        &&lbl_OP_CALL,              /* 31 */
        &&lbl_OP_TAIL_CALL,         /* 32 */
        &&lbl_OP_RETURN,            /* 33 */
        &&lbl_OP_DEFER,             /* 34 */
        &&lbl_OP_JUMP,              /* 35 */
        &&lbl_OP_JUMP_IF_FALSE,     /* 36 */
        &&lbl_OP_JUMP_IF_TRUE,      /* 37 */
        &&lbl_OP_NOT,               /* 38 */
        &&lbl_OP_NEGATE,            /* 39 */
        &&lbl_OP_MOD,               /* 40 */
        &&lbl_OP_POW,               /* 41 */
        &&lbl_OP_SAFE_GET_ATTR,     /* 42 */
        &&lbl_OP_SAFE_INDEX,        /* 43 */
        &&lbl_OP_RESULT_UNWRAP,     /* 44 */
        &&lbl_OP_IS_NIL,            /* 45 */
        &&lbl_OP_DUP,               /* 46 */
        &&lbl_OP_THROW,             /* 47 */
        /* 48..54: in-memory peephole superinstructions */
        &&lbl_OP_INC_LOCAL_1,           /* 48 */
        &&lbl_OP_ADD_LOCAL_LOCAL,       /* 49 */
        &&lbl_OP_SUB_LOCAL_CONST,       /* 50 */
        &&lbl_OP_LT_LOCAL_CONST_JMPF,   /* 51 */
        &&lbl_OP_INC_GLOBAL_1,          /* 52 */
        &&lbl_OP_LT_GLOBAL_CONST_JMPF,  /* 53 */
        &&lbl_OP_LT_LOCAL_LOCAL_JMPF,   /* 54 */
        /* 55..255: invalid */
        &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID,
        &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID,
        &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID,
        &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID,
        &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID,
        &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID,
        &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID,
        &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID,
        &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID,
        &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID,
        &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID,
        &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID,
        &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID,
        &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID,
        &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID,
        &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID,
        &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID,
        &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID,
        &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID,
        &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID,
        &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID,
        &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID,
        &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID,
        &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID,
        &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID,
        &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID,
        &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID,
        &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID,
        &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID,
        &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID,
        &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID,
        &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID,
        &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID,
        &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID,
        &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID,
        &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID,
        &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID,
        &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID,
        &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID,
        &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID, &&lbl_OP_INVALID,
        &&lbl_OP_INVALID
    };
#define ZR_CASE(name) lbl_##name:
#define ZR_DISPATCH() do { \
    if (__builtin_expect(frame->ip >= fn->code_len, 0)) goto frame_falloff; \
    ins = &fn->code[frame->ip++]; \
    op = ins->op; \
    goto *disp_tab[op]; \
} while (0)
#else
#define ZR_CASE(name) case name:
#define ZR_DISPATCH() goto next_iter
#endif

    if (vm->exited || vm->frame_count <= stop_frame_depth) {
        if (out_ret) *out_ret = val_nil();
        return true;
    }

    CallFrame *frame = &vm->frames[vm->frame_count - 1];
    PbFunc *fn = frame->closure->fn;
    PbInstr *ins = NULL;
    uint8_t op = 0;

#if (defined(__GNUC__) || defined(__clang__)) && !defined(_WIN32)
    ZR_DISPATCH();
#else
next_iter:;
    if (vm->exited || vm->frame_count <= stop_frame_depth) {
        if (out_ret) *out_ret = val_nil();
        return true;
    }
    if (frame->ip >= fn->code_len) goto frame_falloff;
    ins = &fn->code[frame->ip++];
    op = ins->op;
    switch (op)
#endif
    {
        ZR_CASE(OP_CONSTANT) {
            if (ins->arg >= fn->const_len) RUNTIME_ERR("constant index out of bounds");
            vm_push(vm, fn->consts[ins->arg]);
            ZR_DISPATCH();
        }
        ZR_CASE(OP_POP) {
            if (vm->sp == 0) RUNTIME_ERR("stack underflow on pop");
            vm->sp--;
            ZR_DISPATCH();
        }
        ZR_CASE(OP_DUP) {
            if (vm->sp == 0) RUNTIME_ERR("stack underflow on dup");
            vm_push(vm, vm->stack[vm->sp - 1]);
            ZR_DISPATCH();
        }
        ZR_CASE(OP_IS_NIL) {
            if (vm->sp == 0) RUNTIME_ERR("stack underflow on is_nil");
            PbValue v = vm_pop(vm);
            vm_push(vm, val_bool(v.tag == VAL_NIL));
            ZR_DISPATCH();
        }
        ZR_CASE(OP_PRINT) {
            PbValue v = vm_pop(vm);
            if (vm->taint_guard_enabled && !zr_check_taint_sink(vm, v, "print")) goto handle_exception;
            size_t slen = 0;
            char *s = vm_value_to_cstr(vm, v, &slen);
            if (slen > 0) fwrite(s, 1, slen, stdout);
            fputc('\n', stdout);
            fflush(stdout);
            free(s);
            ZR_DISPATCH();
        }

        /* ---- Peephole Superinstructions (48..=54) ---- */

        ZR_CASE(OP_INC_LOCAL_1) {
            size_t slot = frame->base + 1 + (size_t)ins->arg;
            if (slot < vm->sp && vm->stack[slot].tag == VAL_NUM) {
                double d = fn->consts[ins->arg2].as.num;
                vm->stack[slot].as.num += (ins->ic_slot ? -d : d);
                frame->ip += 3;
                ZR_DISPATCH();
            }
            goto fallback_get_local;
        }
        ZR_CASE(OP_ADD_LOCAL_LOCAL) {
            size_t s1 = frame->base + 1 + (size_t)ins->arg;
            size_t s2 = frame->base + 1 + (size_t)ins->arg2;
            if (s1 < vm->sp && s2 < vm->sp &&
                vm->stack[s1].tag == VAL_NUM && vm->stack[s2].tag == VAL_NUM) {
                double r = ins->ic_slot ? (vm->stack[s1].as.num - vm->stack[s2].as.num)
                                        : (vm->stack[s1].as.num + vm->stack[s2].as.num);
                PbValue out_v = val_num(r);
                out_v.taint = (uint8_t)(vm->stack[s1].taint | vm->stack[s2].taint);
                vm_push(vm, out_v);
                frame->ip += 2;
                ZR_DISPATCH();
            }
            goto fallback_get_local;
        }
        ZR_CASE(OP_SUB_LOCAL_CONST) {
            size_t s1 = frame->base + 1 + (size_t)ins->arg;
            if (s1 < vm->sp && vm->stack[s1].tag == VAL_NUM) {
                double c = fn->consts[ins->arg2].as.num;
                double r = ins->ic_slot ? (vm->stack[s1].as.num - c) : (vm->stack[s1].as.num + c);
                PbValue out_v = val_num(r);
                out_v.taint = vm->stack[s1].taint;
                vm_push(vm, out_v);
                frame->ip += 2;
                ZR_DISPATCH();
            }
            goto fallback_get_local;
        }
        ZR_CASE(OP_LT_LOCAL_CONST_JMPF) {
            size_t s1 = frame->base + 1 + (size_t)ins->arg;
            if (s1 < vm->sp && vm->stack[s1].tag == VAL_NUM) {
                double lv = vm->stack[s1].as.num;
                double rv = fn->consts[ins->arg2].as.num;
                bool cond = (ins->ic_slot == 0) ? (lv < rv) :
                            (ins->ic_slot == 1) ? (lv <= rv) :
                            (ins->ic_slot == 2) ? (lv > rv) : (lv >= rv);
                if (cond) {
                    frame->ip += 4;
                } else {
                    vm_push(vm, val_bool(false));
                    frame->ip = (uint32_t)ins->ic_bcap;
                }
                ZR_DISPATCH();
            }
            goto fallback_get_local;
        }
        ZR_CASE(OP_LT_LOCAL_LOCAL_JMPF) {
            size_t s1 = frame->base + 1 + (size_t)ins->arg;
            size_t s2 = frame->base + 1 + (size_t)ins->arg2;
            if (s1 < vm->sp && s2 < vm->sp &&
                vm->stack[s1].tag == VAL_NUM && vm->stack[s2].tag == VAL_NUM) {
                double lv = vm->stack[s1].as.num;
                double rv = vm->stack[s2].as.num;
                bool cond = (ins->ic_slot == 0) ? (lv < rv) :
                            (ins->ic_slot == 1) ? (lv <= rv) :
                            (ins->ic_slot == 2) ? (lv > rv) : (lv >= rv);
                if (cond) {
                    frame->ip += 4;
                } else {
                    vm_push(vm, val_bool(false));
                    frame->ip = (uint32_t)ins->ic_bcap;
                }
                ZR_DISPATCH();
            }
            goto fallback_get_local;
        }
        ZR_CASE(OP_INC_GLOBAL_1) {
            PbStr *name = AS_STR(fn->consts[ins->arg]);
            PbMap *gmap = vm_frame_globals(vm, frame);
            if (gmap->bcap > 0 && ins->ic_bcap == gmap->bcap && (size_t)ins->ic_slot < gmap->used) {
                PbMapEntry *e = &gmap->entries[ins->ic_slot];
                if (e->active && e->key == name && e->val.tag == VAL_NUM) {
                    double d = fn->consts[ins->arg2].as.num;
                    e->val.as.num += (ins->pad_u8 ? -d : d);
                    frame->ip += 3;
                    ZR_DISPATCH();
                }
            }
            int32_t idx = pb_map_find_entry_idx(gmap, name->chars, name->len, name->hash);
            if (idx >= 0 && gmap->entries[idx].val.tag == VAL_NUM) {
                ins->ic_bcap = gmap->bcap;
                ins->ic_slot = idx;
                double d = fn->consts[ins->arg2].as.num;
                gmap->entries[idx].val.as.num += (ins->pad_u8 ? -d : d);
                frame->ip += 3;
                ZR_DISPATCH();
            }
            goto fallback_get_global;
        }
        ZR_CASE(OP_LT_GLOBAL_CONST_JMPF) {
            PbStr *name = AS_STR(fn->consts[ins->arg]);
            PbMap *gmap = vm_frame_globals(vm, frame);
            PbMapEntry *hit = NULL;
            if (gmap->bcap > 0 && ins->ic_bcap == gmap->bcap && (size_t)ins->ic_slot < gmap->used) {
                PbMapEntry *e = &gmap->entries[ins->ic_slot];
                if (e->active && e->key == name && e->val.tag == VAL_NUM) hit = e;
            } else {
                int32_t idx = pb_map_find_entry_idx(gmap, name->chars, name->len, name->hash);
                if (idx >= 0 && gmap->entries[idx].val.tag == VAL_NUM) {
                    ins->ic_bcap = gmap->bcap;
                    ins->ic_slot = idx;
                    hit = &gmap->entries[idx];
                }
            }
            if (hit) {
                double lv = hit->val.as.num;
                double rv = fn->consts[ins->arg2].as.num;
                uint8_t ck = ins->pad_u8 & 3u;
                bool cond = (ck == 0) ? (lv < rv) :
                            (ck == 1) ? (lv <= rv) :
                            (ck == 2) ? (lv > rv) : (lv >= rv);
                if (cond) {
                    frame->ip += 4;
                } else {
                    vm_push(vm, val_bool(false));
                    frame->ip = fn->code[frame->ip + 2].arg;
                }
                ZR_DISPATCH();
            }
            goto fallback_get_global;
        }

        ZR_CASE(OP_GET_LOCAL) {
        fallback_get_local:;
            size_t slot = frame->base + 1 + (size_t)ins->arg;
            if (slot >= vm->sp) RUNTIME_ERR("local slot %u out of bounds", ins->arg);
            vm_push(vm, vm->stack[slot]);
            ZR_DISPATCH();
        }
        ZR_CASE(OP_SET_LOCAL) {
            PbValue v = vm_pop(vm);
            size_t slot = frame->base + 1 + (size_t)ins->arg;
            if (slot >= vm->sp + 1) RUNTIME_ERR("local slot %u out of bounds on set", ins->arg);
            vm->stack[slot] = v;
            ZR_DISPATCH();
        }

        ZR_CASE(OP_DEFINE_GLOBAL) {
            if (ins->arg >= fn->const_len || !is_obj_type(fn->consts[ins->arg], OBJ_STR))
                RUNTIME_ERR("invalid global name constant");
            PbStr *name = AS_STR(fn->consts[ins->arg]);
            PbValue val = vm_pop(vm);
            pb_map_set(vm, vm_frame_globals(vm, frame), name, val);
            ZR_DISPATCH();
        }
        ZR_CASE(OP_GET_GLOBAL) {
        fallback_get_global:;
            if (ins->arg >= fn->const_len || !is_obj_type(fn->consts[ins->arg], OBJ_STR))
                RUNTIME_ERR("invalid global name constant");
            PbStr *name = AS_STR(fn->consts[ins->arg]);
            PbMap *gmap = vm_frame_globals(vm, frame);
            if (gmap->bcap > 0 && ins->ic_bcap == gmap->bcap && (size_t)ins->ic_slot < gmap->used) {
                PbMapEntry *e = &gmap->entries[ins->ic_slot];
                if (e->active && (e->key == name ||
                    (e->key->hash == name->hash && e->key->len == name->len &&
                     memcmp(e->key->chars, name->chars, name->len) == 0))) {
                    vm_push(vm, e->val);
                    ZR_DISPATCH();
                }
            }
            int32_t eidx = pb_map_find_entry_idx(gmap, name->chars, name->len, name->hash);
            if (eidx >= 0) {
                ins->ic_bcap = gmap->bcap;
                ins->ic_slot = eidx;
                vm_push(vm, gmap->entries[eidx].val);
            } else {
                PbValue val;
                if (pb_map_get(vm->builtins, name, &val)) {
                    vm_push(vm, val);
                } else {
                    RUNTIME_ERR("undefined variable '%s'", name->chars);
                }
            }
            ZR_DISPATCH();
        }
        ZR_CASE(OP_SET_GLOBAL) {
            if (ins->arg >= fn->const_len || !is_obj_type(fn->consts[ins->arg], OBJ_STR))
                RUNTIME_ERR("invalid global name constant");
            PbStr *name = AS_STR(fn->consts[ins->arg]);
            PbValue val = vm_pop(vm);
            PbMap *gmap = vm_frame_globals(vm, frame);
            if (gmap->bcap > 0 && ins->ic_bcap == gmap->bcap && (size_t)ins->ic_slot < gmap->used) {
                PbMapEntry *e = &gmap->entries[ins->ic_slot];
                if (e->active && (e->key == name ||
                    (e->key->hash == name->hash && e->key->len == name->len &&
                     memcmp(e->key->chars, name->chars, name->len) == 0))) {
                    e->val = val;
                    vm_write_barrier(vm, (PbObj *)gmap, val);
                    ZR_DISPATCH();
                }
            }
            int32_t eidx = pb_map_find_entry_idx(gmap, name->chars, name->len, name->hash);
            if (eidx < 0) {
                RUNTIME_ERR("undefined variable '%s'", name->chars);
            }
            gmap->entries[eidx].val = val;
            vm_write_barrier(vm, (PbObj *)gmap, val);
            ins->ic_bcap = gmap->bcap;
            ins->ic_slot = eidx;
            ZR_DISPATCH();
        }

        ZR_CASE(OP_GET_UPVALUE) {
            if (ins->arg >= frame->closure->upvalue_count) RUNTIME_ERR("upvalue index out of bounds");
            PbUpvalue *uv = frame->closure->upvalues[ins->arg];
            vm_push(vm, uv->is_open ? vm->stack[uv->stack_idx] : uv->closed);
            ZR_DISPATCH();
        }
        ZR_CASE(OP_SET_UPVALUE) {
            PbValue v = vm_pop(vm);
            if (ins->arg >= frame->closure->upvalue_count) RUNTIME_ERR("upvalue index out of bounds");
            PbUpvalue *uv = frame->closure->upvalues[ins->arg];
            if (uv->is_open) {
                vm->stack[uv->stack_idx] = v;
            } else {
                uv->closed = v;
                vm_write_barrier(vm, (PbObj *)uv, v);
            }
            ZR_DISPATCH();
        }
        ZR_CASE(OP_CLOSE_UPVALUE) {
            if (vm->sp == 0) RUNTIME_ERR("stack underflow on close_upvalue");
            vm_close_upvalues_from(vm, vm->sp - 1);
            vm->sp--;
            ZR_DISPATCH();
        }

        ZR_CASE(OP_CLOSURE) {
            if (ins->arg >= fn->const_len || !is_obj_type(fn->consts[ins->arg], OBJ_FUNC))
                RUNTIME_ERR("OpClosure expects function constant");
            PbFunc *sub_fn = AS_FUNC(fn->consts[ins->arg]);
            PbClosure *cl = pb_closure_new(vm, sub_fn, ins->n_caps, frame->closure->module);
            vm_push_root(vm, val_obj(cl));
            for (uint32_t i = 0; i < ins->n_caps; i++) {
                if (ins->caps[i].is_local) {
                    cl->upvalues[i] = vm_capture_upvalue(vm, frame->base + 1 + (size_t)ins->caps[i].idx);
                } else {
                    if (ins->caps[i].idx >= frame->closure->upvalue_count) {
                        vm_pop_roots(vm, 1);
                        RUNTIME_ERR("outer upvalue index out of bounds");
                    }
                    cl->upvalues[i] = frame->closure->upvalues[ins->caps[i].idx];
                }
                vm_write_barrier_obj(vm, (PbObj *)cl, (PbObj *)cl->upvalues[i]);
            }
            vm_pop_roots(vm, 1);
            vm_push(vm, val_obj(cl));
            ZR_DISPATCH();
        }

        ZR_CASE(OP_ADD) {
            if (vm->sp >= 2 &&
                vm->stack[vm->sp - 2].tag == VAL_NUM && vm->stack[vm->sp - 1].tag == VAL_NUM) {
                vm->stack[vm->sp - 2].as.num += vm->stack[vm->sp - 1].as.num;
                vm->stack[vm->sp - 2].taint |= vm->stack[vm->sp - 1].taint;
                vm->sp--;
                ZR_DISPATCH();
            }
            goto generic_bin_arith;
        }
        ZR_CASE(OP_SUB) {
            if (vm->sp >= 2 &&
                vm->stack[vm->sp - 2].tag == VAL_NUM && vm->stack[vm->sp - 1].tag == VAL_NUM) {
                vm->stack[vm->sp - 2].as.num -= vm->stack[vm->sp - 1].as.num;
                vm->stack[vm->sp - 2].taint |= vm->stack[vm->sp - 1].taint;
                vm->sp--;
                ZR_DISPATCH();
            }
            goto generic_bin_arith;
        }
        ZR_CASE(OP_MUL) {
            if (vm->sp >= 2 &&
                vm->stack[vm->sp - 2].tag == VAL_NUM && vm->stack[vm->sp - 1].tag == VAL_NUM) {
                vm->stack[vm->sp - 2].as.num *= vm->stack[vm->sp - 1].as.num;
                vm->stack[vm->sp - 2].taint |= vm->stack[vm->sp - 1].taint;
                vm->sp--;
                ZR_DISPATCH();
            }
            goto generic_bin_arith;
        }
        ZR_CASE(OP_DIV)
        ZR_CASE(OP_MOD)
        ZR_CASE(OP_POW) {
        generic_bin_arith:;
            uint8_t aop = ins->orig_op ? ins->orig_op : op;
            PbValue b = vm_pop(vm);
            PbValue a = vm_pop(vm);
            if (a.tag == VAL_NUM && b.tag == VAL_NUM) {
                double r = 0.0;
                if (aop == OP_ADD) r = a.as.num + b.as.num;
                else if (aop == OP_SUB) r = a.as.num - b.as.num;
                else if (aop == OP_MUL) r = a.as.num * b.as.num;
                else if (aop == OP_DIV) {
                    if (b.as.num == 0.0) RUNTIME_ERR("division by zero");
                    r = a.as.num / b.as.num;
                } else if (aop == OP_MOD) {
                    if (b.as.num == 0.0) RUNTIME_ERR("modulo by zero");
                    r = fmod(a.as.num, b.as.num);
                } else {
                    r = pow(a.as.num, b.as.num);
                }
                vm_push(vm, val_with_taint(val_num(r), (uint8_t)(a.taint | b.taint)));
            } else if (aop == OP_ADD && is_obj_type(a, OBJ_STR) && is_obj_type(b, OBJ_STR)) {
                PbStr *sa = AS_STR(a), *sb = AS_STR(b);
                uint8_t merged_taint = (uint8_t)(a.taint | b.taint);
                vm_push_root(vm, a);
                vm_push_root(vm, b);
                size_t nlen = sa->len + sb->len;
                PbStr *res = (PbStr *)vm_alloc_obj(vm, sizeof(PbStr) + nlen + 1, OBJ_STR);
                if (sa->len > 0) memcpy(res->chars, sa->chars, sa->len);
                if (sb->len > 0) memcpy(res->chars + sa->len, sb->chars, sb->len);
                res->chars[nlen] = '\0';
                res->len = nlen;
                res->hash = zr_fnv1a(res->chars, nlen);
                vm->string_allocs++;
                vm_pop_roots(vm, 2);
                vm_push(vm, val_with_taint(val_obj(res), merged_taint));
            } else if (aop == OP_ADD && is_obj_type(a, OBJ_ARRAY) && is_obj_type(b, OBJ_ARRAY)) {
                PbArray *aa = AS_ARRAY(a), *ab = AS_ARRAY(b);
                vm_push_root(vm, a);
                vm_push_root(vm, b);
                PbArray *res = pb_array_new(vm, aa->len + ab->len);
                for (size_t i = 0; i < aa->len; i++) pb_array_push(vm, res, aa->items[i]);
                for (size_t i = 0; i < ab->len; i++) pb_array_push(vm, res, ab->items[i]);
                vm_pop_roots(vm, 2);
                vm_push(vm, val_with_taint(val_obj(res), (uint8_t)(a.taint | b.taint)));
            } else if (is_obj_type(a, OBJ_BIGINT) || is_obj_type(b, OBJ_BIGINT) ||
                       is_obj_type(a, OBJ_RATIONAL) || is_obj_type(b, OBJ_RATIONAL) ||
                       is_obj_type(a, OBJ_DECIMAL) || is_obj_type(b, OBJ_DECIMAL)) {
                vm_push_root(vm, a);
                vm_push_root(vm, b);
                PbValue res = val_nil();
                bool ok = vm_bignum_arith(vm, aop, a, b, &res);
                vm_pop_roots(vm, 2);
                if (!ok) goto handle_exception;
                res.taint = (uint8_t)(a.taint | b.taint);
                vm_push(vm, res);
            } else {
                const char *dunder = (aop == OP_ADD) ? "__add__" :
                                     (aop == OP_SUB) ? "__sub__" :
                                     (aop == OP_MUL) ? "__mul__" :
                                     (aop == OP_DIV) ? "__div__" :
                                     (aop == OP_MOD) ? "__mod__" : "__pow__";
                const char *rdunder = (aop == OP_ADD) ? "__radd__" :
                                      (aop == OP_SUB) ? "__rsub__" :
                                      (aop == OP_MUL) ? "__rmul__" :
                                      (aop == OP_DIV) ? "__rdiv__" :
                                      (aop == OP_MOD) ? "__rmod__" : "__rpow__";
                PbValue fn_val;
                if (vm_lookup_map_dunder(a, dunder, &fn_val)) {
                    PbValue args2[2] = {a, b};
                    PbValue res = val_nil();
                    vm_push_root(vm, a); vm_push_root(vm, b);
                    bool ok = vm_call_value_sync(vm, fn_val, 2, args2, &res);
                    vm_pop_roots(vm, 2);
                    if (!ok) goto handle_exception;
                    if (vm->exited) { if (out_ret) *out_ret = val_nil(); return true; }
                    ZR_RELOAD_FRAME();
                    vm_push(vm, res);
                } else if (vm_lookup_map_dunder(b, rdunder, &fn_val)) {
                    PbValue args2[2] = {b, a};
                    PbValue res = val_nil();
                    vm_push_root(vm, a); vm_push_root(vm, b);
                    bool ok = vm_call_value_sync(vm, fn_val, 2, args2, &res);
                    vm_pop_roots(vm, 2);
                    if (!ok) goto handle_exception;
                    if (vm->exited) { if (out_ret) *out_ret = val_nil(); return true; }
                    ZR_RELOAD_FRAME();
                    vm_push(vm, res);
                } else {
                    RUNTIME_ERR("type error in arithmetic: %s vs %s", zr_val_type_name(a), zr_val_type_name(b));
                }
            }
            ZR_DISPATCH();
        }

        ZR_CASE(OP_NEGATE) {
            PbValue a = vm_pop(vm);
            if (a.tag == VAL_NUM) {
                vm_push(vm, val_with_taint(val_num(-a.as.num), a.taint));
            } else if (is_obj_type(a, OBJ_BIGINT)) {
                vm_push_root(vm, a);
                ZrBigInt nb;
                zr_bi_copy(&nb, &AS_BIGINT(a)->v);
                nb.sign = -nb.sign;
                PbValue r = pb_bigint_new_take(vm, &nb);
                vm_pop_roots(vm, 1);
                vm_push(vm, r);
            } else if (is_obj_type(a, OBJ_RATIONAL)) {
                vm_push_root(vm, a);
                ZrRational nr;
                zr_bi_copy(&nr.num, &AS_RATIONAL(a)->v.num);
                zr_bi_copy(&nr.den, &AS_RATIONAL(a)->v.den);
                nr.num.sign = -nr.num.sign;
                PbValue r = pb_rational_new_take(vm, &nr);
                vm_pop_roots(vm, 1);
                vm_push(vm, r);
            } else if (is_obj_type(a, OBJ_DECIMAL)) {
                vm_push_root(vm, a);
                ZrDecimal nd;
                zr_bi_copy(&nd.coeff, &AS_DECIMAL(a)->v.coeff);
                nd.coeff.sign = -nd.coeff.sign;
                nd.scale = AS_DECIMAL(a)->v.scale;
                PbValue r = pb_decimal_new_take(vm, &nd);
                vm_pop_roots(vm, 1);
                vm_push(vm, r);
            } else {
                PbValue fn_val;
                if (!vm_lookup_map_dunder(a, "__neg__", &fn_val)) {
                    RUNTIME_ERR("unary '-' expects a number, got %s", zr_val_type_name(a));
                }
                PbValue res = val_nil();
                vm_push_root(vm, a);
                bool ok = vm_call_value_sync(vm, fn_val, 1, &a, &res);
                vm_pop_roots(vm, 1);
                if (!ok) goto handle_exception;
                if (vm->exited) { if (out_ret) *out_ret = val_nil(); return true; }
                ZR_RELOAD_FRAME();
                vm_push(vm, res);
            }
            ZR_DISPATCH();
        }

        ZR_CASE(OP_NOT) {
            PbValue a = vm_pop(vm);
            vm_push(vm, val_bool(!val_truthy(a)));
            ZR_DISPATCH();
        }

        ZR_CASE(OP_EQ)
        ZR_CASE(OP_NOT_EQ) {
            PbValue b = vm_pop(vm);
            PbValue a = vm_pop(vm);
            if (a.tag == VAL_NUM && b.tag == VAL_NUM) {
                bool eq = (a.as.num == b.as.num);
                vm_push(vm, val_bool(op == OP_EQ ? eq : !eq));
                ZR_DISPATCH();
            }
            PbValue fn_val;
            if (vm_lookup_map_dunder(a, (op == OP_EQ) ? "__eq__" : "__ne__", &fn_val)) {
                PbValue args2[2] = {a, b};
                PbValue res = val_nil();
                vm_push_root(vm, a); vm_push_root(vm, b);
                bool ok = vm_call_value_sync(vm, fn_val, 2, args2, &res);
                vm_pop_roots(vm, 2);
                if (!ok) goto handle_exception;
                if (vm->exited) { if (out_ret) *out_ret = val_nil(); return true; }
                ZR_RELOAD_FRAME();
                vm_push(vm, res);
                ZR_DISPATCH();
            }
            if (op == OP_NOT_EQ && vm_lookup_map_dunder(a, "__eq__", &fn_val)) {
                PbValue args2[2] = {a, b};
                PbValue res = val_nil();
                vm_push_root(vm, a); vm_push_root(vm, b);
                bool ok = vm_call_value_sync(vm, fn_val, 2, args2, &res);
                vm_pop_roots(vm, 2);
                if (!ok) goto handle_exception;
                if (vm->exited) { if (out_ret) *out_ret = val_nil(); return true; }
                ZR_RELOAD_FRAME();
                vm_push(vm, val_bool(!val_truthy(res)));
                ZR_DISPATCH();
            }
            bool eq = val_equal(a, b);
            vm_push(vm, val_bool(op == OP_EQ ? eq : !eq));
            ZR_DISPATCH();
        }

        ZR_CASE(OP_LT)
        ZR_CASE(OP_LE)
        ZR_CASE(OP_GT)
        ZR_CASE(OP_GE) {
            PbValue b = vm_pop(vm);
            PbValue a = vm_pop(vm);
            if (a.tag == VAL_NUM && b.tag == VAL_NUM) {
                bool r = (op == OP_LT) ? (a.as.num < b.as.num) :
                         (op == OP_LE) ? (a.as.num <= b.as.num) :
                         (op == OP_GT) ? (a.as.num > b.as.num) :
                                         (a.as.num >= b.as.num);
                vm_push(vm, val_bool(r));
            } else if (is_obj_type(a, OBJ_STR) && is_obj_type(b, OBJ_STR)) {
                PbStr *sa = AS_STR(a), *sb = AS_STR(b);
                size_t ml = sa->len < sb->len ? sa->len : sb->len;
                int c = memcmp(sa->chars, sb->chars, ml);
                if (c == 0) c = (sa->len < sb->len) ? -1 : ((sa->len > sb->len) ? 1 : 0);
                bool r = (op == OP_LT) ? (c < 0) : (op == OP_LE) ? (c <= 0) : (op == OP_GT) ? (c > 0) : (c >= 0);
                vm_push(vm, val_bool(r));
            } else if (is_obj_type(a, OBJ_BIGINT) || is_obj_type(b, OBJ_BIGINT) ||
                       is_obj_type(a, OBJ_RATIONAL) || is_obj_type(b, OBJ_RATIONAL) ||
                       is_obj_type(a, OBJ_DECIMAL) || is_obj_type(b, OBJ_DECIMAL)) {
                int c = 0;
                if (!vm_bignum_cmp(vm, a, b, &c)) goto handle_exception;
                bool r = (op == OP_LT) ? (c < 0) : (op == OP_LE) ? (c <= 0) : (op == OP_GT) ? (c > 0) : (c >= 0);
                vm_push(vm, val_bool(r));
            } else {
                const char *dunder = (op == OP_LT) ? "__lt__" :
                                     (op == OP_LE) ? "__le__" :
                                     (op == OP_GT) ? "__gt__" : "__ge__";
                PbValue fn_val;
                if (!vm_lookup_map_dunder(a, dunder, &fn_val)) {
                    RUNTIME_ERR("cannot order %s and %s", zr_val_type_name(a), zr_val_type_name(b));
                }
                PbValue args2[2] = {a, b};
                PbValue res = val_nil();
                vm_push_root(vm, a); vm_push_root(vm, b);
                bool ok = vm_call_value_sync(vm, fn_val, 2, args2, &res);
                vm_pop_roots(vm, 2);
                if (!ok) goto handle_exception;
                if (vm->exited) { if (out_ret) *out_ret = val_nil(); return true; }
                ZR_RELOAD_FRAME();
                vm_push(vm, res);
            }
            ZR_DISPATCH();
        }

        ZR_CASE(OP_JUMP) {
            uint32_t tgt = ins->arg;
            uint32_t jump_ip = frame->ip - 1u;
            if (tgt <= jump_ip) {
                if (vm->exited) { if (out_ret) *out_ret = val_nil(); return true; }
                if (vm->gc_inhibit == 0 && vm->nursery_bytes >= vm->nursery_threshold) {
                    vm_collect_nursery(vm);
                }
                if (ins->ic_slot >= 0 && ++ins->ic_slot >= 32) {
                    if (zr_try_osr_numeric_loop(vm, frame, fn, tgt, jump_ip)) {
                        ins->ic_slot = 0;
                        ZR_DISPATCH();
                    }
                    ins->ic_slot = -1; /* mark non-OSR loop so we never re-scan */
                }
            }
            frame->ip = tgt;
            ZR_DISPATCH();
        }
        ZR_CASE(OP_JUMP_IF_FALSE) {
            if (!val_truthy(vm->stack[vm->sp - 1])) frame->ip = ins->arg;
            ZR_DISPATCH();
        }
        ZR_CASE(OP_JUMP_IF_TRUE) {
            if (val_truthy(vm->stack[vm->sp - 1])) frame->ip = ins->arg;
            ZR_DISPATCH();
        }

        ZR_CASE(OP_CALL) {
            if (!vm_call_top(vm, ins->arg, false, stop_frame_depth)) goto handle_exception;
            if (vm->exited) { if (out_ret) *out_ret = val_nil(); return true; }
            if (vm->gc_inhibit == 0 && vm->nursery_bytes >= vm->nursery_threshold) {
                vm_collect_nursery(vm);
            }
            if (vm->frame_count <= stop_frame_depth) {
                if (out_ret) *out_ret = (vm->sp > 0) ? vm->stack[vm->sp - 1] : val_nil();
                return true;
            }
            ZR_RELOAD_FRAME();
            ZR_DISPATCH();
        }
        ZR_CASE(OP_TAIL_CALL) {
            if (!vm_call_top(vm, ins->arg, true, stop_frame_depth)) goto handle_exception;
            if (vm->exited) { if (out_ret) *out_ret = val_nil(); return true; }
            if (vm->gc_inhibit == 0 && vm->nursery_bytes >= vm->nursery_threshold) {
                vm_collect_nursery(vm);
            }
            if (vm->frame_count <= stop_frame_depth) {
                if (out_ret) *out_ret = (vm->sp > 0) ? vm->stack[vm->sp - 1] : val_nil();
                return true;
            }
            ZR_RELOAD_FRAME();
            ZR_DISPATCH();
        }

        ZR_CASE(OP_RETURN) {
            PbValue ret = vm_pop(vm);
            vm_push_root(vm, ret);
            vm_run_frame_defers(vm, frame, 0);
            vm_prof_exit_frame(vm, frame, vm->frame_count - 1);
            size_t base = frame->base;
            vm_close_upvalues_from(vm, base + 1);
            while (vm->handler_count > 0 && vm->handlers[vm->handler_count - 1].frame_idx == vm->frame_count - 1) {
                vm->handler_count--;
            }
            vm->frame_count--;
            vm->sp = base;
            vm_pop_roots(vm, 1);
            if (vm->exited || vm->frame_count <= stop_frame_depth) {
                if (out_ret) *out_ret = ret;
                return true;
            }
            ZR_RELOAD_FRAME();
            vm_push(vm, ret);
            ZR_DISPATCH();
        }

        ZR_CASE(OP_DEFER) {
            PbValue dcallee = vm_pop(vm);
            if (frame->defer_len >= frame->defer_cap) {
                size_t ncap = frame->defer_cap ? frame->defer_cap * 2 : 4;
                frame->defers = (PbValue *)zr_xrealloc(frame->defers, ncap * sizeof(PbValue));
                frame->defer_cap = ncap;
            }
            frame->defers[frame->defer_len++] = dcallee;
            ZR_DISPATCH();
        }

        ZR_CASE(OP_MAKE_ARRAY) {
            uint32_t count = ins->arg;
            if (vm->sp < count) RUNTIME_ERR("stack underflow in make_array");
            PbArray *arr = pb_array_new(vm, count);
            for (uint32_t i = 0; i < count; i++) {
                arr->items[i] = vm->stack[vm->sp - count + i];
                vm_write_barrier(vm, (PbObj *)arr, arr->items[i]);
            }
            arr->len = count;
            vm->sp -= count;
            vm_push(vm, val_obj(arr));
            ZR_DISPATCH();
        }

        ZR_CASE(OP_MAKE_MAP) {
            uint32_t count = ins->arg;
            size_t need = (size_t)count * 2;
            if (vm->sp < need) RUNTIME_ERR("stack underflow in make_map");
            PbMap *m = pb_map_new_raw(vm, count);
            vm_push_root(vm, val_obj(m));
            size_t start = vm->sp - need;
            for (uint32_t i = 0; i < count; i++) {
                PbValue kv = vm->stack[start + (size_t)i * 2];
                PbValue vv = vm->stack[start + (size_t)i * 2 + 1];
                PbStr *ks = is_obj_type(kv, OBJ_STR) ? AS_STR(kv) : vm_value_to_pbstr(vm, kv);
                vm_push_root(vm, val_obj(ks));
                pb_map_set(vm, m, ks, vv);
                vm_pop_roots(vm, 1);
            }
            vm_pop_roots(vm, 1);
            vm->sp = start;
            vm_push(vm, val_obj(m));
            ZR_DISPATCH();
        }

        ZR_CASE(OP_GET_INDEX)
        ZR_CASE(OP_SAFE_INDEX) {
            bool opt = (op == OP_SAFE_INDEX);
            PbValue idx_v = vm_pop(vm);
            PbValue obj_v = vm_pop(vm);
            if (opt && obj_v.tag == VAL_NIL) {
                vm_push(vm, val_nil());
                ZR_DISPATCH();
            }
            if (is_obj_type(obj_v, OBJ_ARRAY)) {
                if (idx_v.tag != VAL_NUM) RUNTIME_ERR("array index must be a number");
                PbArray *arr = AS_ARRAY(obj_v);
                int64_t idx = (int64_t)idx_v.as.num;
                if (idx < 0) idx += (int64_t)arr->len;
                if (idx < 0 || (size_t)idx >= arr->len) {
                    if (opt) { vm_push(vm, val_nil()); ZR_DISPATCH(); }
                    RUNTIME_ERR("array index %" PRId64 " out of bounds (len %zu)", idx, arr->len);
                }
                vm_push(vm, arr->items[idx]);
            } else if (is_obj_type(obj_v, OBJ_BYTES)) {
                if (idx_v.tag != VAL_NUM) RUNTIME_ERR("bytes index must be a number");
                PbBytes *bb = AS_BYTES(obj_v);
                int64_t idx = (int64_t)idx_v.as.num;
                if (idx < 0) idx += (int64_t)bb->len;
                if (idx < 0 || (size_t)idx >= bb->len) {
                    if (opt) { vm_push(vm, val_nil()); ZR_DISPATCH(); }
                    RUNTIME_ERR("bytes index %" PRId64 " out of bounds (len %zu)", idx, bb->len);
                }
                vm_push(vm, val_with_taint(val_num((double)bb->data[idx]), obj_v.taint));
            } else if (is_obj_type(obj_v, OBJ_F64BUF)) {
                if (idx_v.tag != VAL_NUM) RUNTIME_ERR("f64buf index must be a number");
                PbF64Buf *fb = AS_F64BUF(obj_v);
                int64_t idx = (int64_t)idx_v.as.num;
                if (idx < 0) idx += (int64_t)fb->len;
                if (idx < 0 || (size_t)idx >= fb->len) {
                    if (opt) { vm_push(vm, val_nil()); ZR_DISPATCH(); }
                    RUNTIME_ERR("f64buf index %" PRId64 " out of bounds (len %zu)", idx, fb->len);
                }
                vm_push(vm, val_with_taint(val_num(fb->data[idx]), obj_v.taint));
            } else if (is_obj_type(obj_v, OBJ_MAP)) {
                PbMap *m = AS_MAP(obj_v);
                PbStr *ks = is_obj_type(idx_v, OBJ_STR) ? AS_STR(idx_v) : vm_value_to_pbstr(vm, idx_v);
                PbValue out;
                if (!pb_map_get(m, ks, &out)) {
                    if (opt) { vm_push(vm, val_nil()); ZR_DISPATCH(); }
                    RUNTIME_ERR("undefined key '%s'", ks->chars);
                }
                vm_push(vm, out);
            } else if (is_obj_type(obj_v, OBJ_STR)) {
                if (idx_v.tag != VAL_NUM) RUNTIME_ERR("string index must be a number");
                PbStr *s = AS_STR(obj_v);
                size_t clen = zr_utf8_char_len(s->chars, s->len);
                int64_t idx = (int64_t)idx_v.as.num;
                if (idx < 0) idx += (int64_t)clen;
                if (idx < 0 || (size_t)idx >= clen) {
                    if (opt) { vm_push(vm, val_nil()); ZR_DISPATCH(); }
                    RUNTIME_ERR("string index %" PRId64 " out of bounds (len %zu)", idx, clen);
                }
                size_t adv = 0;
                size_t off = zr_utf8_nth_offset(s->chars, s->len, (size_t)idx, &adv);
                vm_push_root(vm, obj_v);
                PbStr *ch = pb_str_new(vm, s->chars + off, adv);
                vm_pop_roots(vm, 1);
                vm_push(vm, val_with_taint(val_obj(ch), obj_v.taint));
            } else if (is_obj_type(obj_v, OBJ_MODULE)) {
                PbModule *mod = AS_MODULE(obj_v);
                PbStr *ks = is_obj_type(idx_v, OBJ_STR) ? AS_STR(idx_v) : vm_value_to_pbstr(vm, idx_v);
                PbValue out;
                if (!pb_map_get(mod->exports, ks, &out)) {
                    if (opt) { vm_push(vm, val_nil()); ZR_DISPATCH(); }
                    RUNTIME_ERR("module export '%s' not found", ks->chars);
                }
                vm_push(vm, out);
            } else if (opt) {
                vm_push(vm, val_nil());
            } else {
                RUNTIME_ERR("cannot index value of type %s", zr_val_type_name(obj_v));
            }
            ZR_DISPATCH();
        }

        ZR_CASE(OP_SET_INDEX) {
            PbValue val = vm_pop(vm);
            PbValue idx_v = vm_pop(vm);
            PbValue obj_v = vm_pop(vm);
            if (obj_v.tag == VAL_OBJ && obj_v.as.obj->frozen) {
                RUNTIME_ERR("cannot mutate frozen %s", zr_val_type_name(obj_v));
            }
            if (is_obj_type(obj_v, OBJ_ARRAY)) {
                if (idx_v.tag != VAL_NUM) RUNTIME_ERR("array index must be a number");
                PbArray *arr = AS_ARRAY(obj_v);
                int64_t idx = (int64_t)idx_v.as.num;
                if (idx < 0) idx += (int64_t)arr->len;
                if (idx >= 0 && (size_t)idx == arr->len) {
                    vm_push_root(vm, val);
                    vm_push_root(vm, obj_v);
                    pb_array_push(vm, arr, val);
                    vm_pop_roots(vm, 2);
                } else if (idx >= 0 && (size_t)idx < arr->len) {
                    arr->items[idx] = val;
                    vm_write_barrier(vm, (PbObj *)arr, val);
                } else {
                    RUNTIME_ERR("array index %" PRId64 " out of bounds (len %zu)", idx, arr->len);
                }
                vm_push(vm, val);
            } else if (is_obj_type(obj_v, OBJ_BYTES)) {
                if (idx_v.tag != VAL_NUM || val.tag != VAL_NUM)
                    RUNTIME_ERR("bytes index assignment expects (number, number)");
                PbBytes *bb = AS_BYTES(obj_v);
                int64_t idx = (int64_t)idx_v.as.num;
                if (idx < 0) idx += (int64_t)bb->len;
                if (idx < 0 || (size_t)idx >= bb->len)
                    RUNTIME_ERR("bytes index %" PRId64 " out of bounds (len %zu)", idx, bb->len);
                bb->data[idx] = (uint8_t)((int64_t)val.as.num & 0xFF);
                vm_push(vm, val);
            } else if (is_obj_type(obj_v, OBJ_F64BUF)) {
                if (idx_v.tag != VAL_NUM || val.tag != VAL_NUM)
                    RUNTIME_ERR("f64buf index assignment expects (number, number)");
                PbF64Buf *fb = AS_F64BUF(obj_v);
                int64_t idx = (int64_t)idx_v.as.num;
                if (idx < 0) idx += (int64_t)fb->len;
                if (idx < 0 || (size_t)idx >= fb->len)
                    RUNTIME_ERR("f64buf index %" PRId64 " out of bounds (len %zu)", idx, fb->len);
                fb->data[idx] = val.as.num;
                vm_push(vm, val);
            } else if (is_obj_type(obj_v, OBJ_MAP)) {
                PbMap *m = AS_MAP(obj_v);
                vm_push_root(vm, val);
                vm_push_root(vm, obj_v);
                PbStr *ks = is_obj_type(idx_v, OBJ_STR) ? AS_STR(idx_v) : vm_value_to_pbstr(vm, idx_v);
                vm_push_root(vm, val_obj(ks));
                pb_map_set(vm, m, ks, val);
                vm_pop_roots(vm, 3);
                vm_push(vm, val);
            } else {
                RUNTIME_ERR("cannot index-assign value of type %s", zr_val_type_name(obj_v));
            }
            ZR_DISPATCH();
        }

        ZR_CASE(OP_GET_ATTR)
        ZR_CASE(OP_SAFE_GET_ATTR) {
            bool opt = (op == OP_SAFE_GET_ATTR);
            if (ins->arg >= fn->const_len || !is_obj_type(fn->consts[ins->arg], OBJ_STR))
                RUNTIME_ERR("invalid attribute constant");
            PbStr *attr = AS_STR(fn->consts[ins->arg]);
            PbValue obj_v = vm_pop(vm);
            if (opt && obj_v.tag == VAL_NIL) {
                vm_push(vm, val_nil());
                ZR_DISPATCH();
            }
            if (is_obj_type(obj_v, OBJ_MAP)) {
                PbMap *m = AS_MAP(obj_v);
                if (m->bcap > 0 && ins->ic_bcap == m->bcap && (size_t)ins->ic_slot < m->used) {
                    PbMapEntry *e = &m->entries[ins->ic_slot];
                    if (e->active && (e->key == attr ||
                        (e->key->hash == attr->hash && e->key->len == attr->len &&
                         memcmp(e->key->chars, attr->chars, attr->len) == 0))) {
                        vm_push(vm, e->val);
                        ZR_DISPATCH();
                    }
                }
                int32_t eidx = pb_map_find_entry_idx(m, attr->chars, attr->len, attr->hash);
                if (eidx >= 0) {
                    ins->ic_bcap = m->bcap;
                    ins->ic_slot = eidx;
                    vm_push(vm, m->entries[eidx].val);
                } else {
                    PbValue out;
                    if (vm_lookup_map_dunder(obj_v, attr->chars, &out)) {
                        vm_push(vm, out);
                    } else if (opt) {
                        vm_push(vm, val_nil());
                    } else {
                        RUNTIME_ERR("undefined key '%s'", attr->chars);
                    }
                }
            } else if (is_obj_type(obj_v, OBJ_MODULE)) {
                PbModule *mod = AS_MODULE(obj_v);
                PbMap *emap = mod->exports;
                if (emap->bcap > 0 && ins->ic_bcap == emap->bcap && (size_t)ins->ic_slot < emap->used) {
                    PbMapEntry *e = &emap->entries[ins->ic_slot];
                    if (e->active && (e->key == attr ||
                        (e->key->hash == attr->hash && e->key->len == attr->len &&
                         memcmp(e->key->chars, attr->chars, attr->len) == 0))) {
                        vm_push(vm, e->val);
                        ZR_DISPATCH();
                    }
                }
                int32_t eidx = pb_map_find_entry_idx(emap, attr->chars, attr->len, attr->hash);
                if (eidx >= 0) {
                    ins->ic_bcap = emap->bcap;
                    ins->ic_slot = eidx;
                    vm_push(vm, emap->entries[eidx].val);
                } else if (opt) {
                    vm_push(vm, val_nil());
                } else {
                    RUNTIME_ERR("export '%s' not found in module '%s'", attr->chars, mod->name->chars);
                }
            } else {
                RUNTIME_ERR("cannot read attribute '%s' on %s", attr->chars, zr_val_type_name(obj_v));
            }
            ZR_DISPATCH();
        }

        ZR_CASE(OP_SET_ATTR) {
            if (ins->arg >= fn->const_len || !is_obj_type(fn->consts[ins->arg], OBJ_STR))
                RUNTIME_ERR("invalid attribute constant");
            PbStr *attr = AS_STR(fn->consts[ins->arg]);
            PbValue val = vm_pop(vm);
            PbValue obj_v = vm_pop(vm);
            if (!is_obj_type(obj_v, OBJ_MAP)) {
                RUNTIME_ERR("cannot set attribute '%s' on %s", attr->chars, zr_val_type_name(obj_v));
            }
            if (obj_v.as.obj->frozen) {
                RUNTIME_ERR("cannot mutate frozen map");
            }
            PbMap *m = AS_MAP(obj_v);
            if (m->bcap > 0 && ins->ic_bcap == m->bcap && (size_t)ins->ic_slot < m->used) {
                PbMapEntry *e = &m->entries[ins->ic_slot];
                if (e->active && (e->key == attr ||
                    (e->key->hash == attr->hash && e->key->len == attr->len &&
                     memcmp(e->key->chars, attr->chars, attr->len) == 0))) {
                    e->val = val;
                    vm_write_barrier(vm, (PbObj *)m, val);
                    vm_push(vm, val);
                    ZR_DISPATCH();
                }
            }
            int32_t eidx = pb_map_set_idx(vm, m, attr, val);
            ins->ic_bcap = m->bcap;
            ins->ic_slot = eidx;
            vm_push(vm, val);
            ZR_DISPATCH();
        }

        ZR_CASE(OP_EXPORT) {
            if (ins->arg >= fn->const_len || !is_obj_type(fn->consts[ins->arg], OBJ_STR))
                RUNTIME_ERR("invalid export constant");
            PbStr *name = AS_STR(fn->consts[ins->arg]);
            PbMap *gmap = vm_frame_globals(vm, frame);
            PbMap *emap = vm_frame_exports(vm, frame);
            PbValue val;
            if (!pb_map_get(gmap, name, &val) && !pb_map_get(vm->builtins, name, &val)) {
                RUNTIME_ERR("cannot export undefined '%s'", name->chars);
            }
            pb_map_set(vm, emap, name, val);
            ZR_DISPATCH();
        }

        ZR_CASE(OP_TRY) {
            if (vm->handler_count >= ZR_MAX_HANDLERS) RUNTIME_ERR("try handler stack overflow");
            TryHandler *h = &vm->handlers[vm->handler_count++];
            h->frame_idx = vm->frame_count - 1;
            h->stack_depth = vm->sp;
            h->target_ip = ins->arg;
            h->defer_depth = frame->defer_len;
            ZR_DISPATCH();
        }

        ZR_CASE(OP_END_TRY) {
            if (vm->handler_count > 0) vm->handler_count--;
            ZR_DISPATCH();
        }

        ZR_CASE(OP_RESULT_UNWRAP) {
            PbValue rv = vm_pop(vm);
            if (rv.tag == VAL_NIL) {
                RUNTIME_ERR("? operator: cannot unwrap nil");
            } else if (is_obj_type(rv, OBJ_MAP)) {
                PbMap *m = AS_MAP(rv);
                PbValue ok_v;
                if (pb_map_get_cstr(m, "ok", &ok_v) && ok_v.tag == VAL_BOOL) {
                    if (!ok_v.as.b) {
                        PbValue err_v;
                        if (pb_map_get_cstr(m, "error", &err_v)) {
                            char *es = vm_value_to_cstr(vm, err_v, NULL);
                            vm_error(vm, "%s", es);
                            free(es);
                        } else {
                            vm_error(vm, "error (from ? operator)");
                        }
                        goto handle_exception;
                    } else {
                        PbValue res_v = val_nil();
                        pb_map_get_cstr(m, "value", &res_v);
                        vm_push(vm, res_v);
                    }
                } else {
                    vm_push(vm, rv);
                }
            } else {
                vm_push(vm, rv);
            }
            ZR_DISPATCH();
        }

        ZR_CASE(OP_THROW) {
            PbValue tv = vm_pop(vm);
            char *s = vm_value_to_cstr(vm, tv, NULL);
            vm_error(vm, "%s", s);
            free(s);
            goto handle_exception;
        }

#if (defined(__GNUC__) || defined(__clang__)) && !defined(_WIN32)
        ZR_CASE(OP_INVALID)
#else
        default:
#endif
        {
            RUNTIME_ERR("unknown opcode %u at ip %u", op, frame->ip - 1);
        }
    }

frame_falloff:;
    vm_run_frame_defers(vm, frame, 0);
    vm_prof_exit_frame(vm, frame, vm->frame_count - 1);
    {
        size_t base = frame->base;
        vm_close_upvalues_from(vm, base + 1);
        while (vm->handler_count > 0 && vm->handlers[vm->handler_count - 1].frame_idx == vm->frame_count - 1) {
            vm->handler_count--;
        }
        vm->frame_count--;
        vm->sp = base;
        if (vm->exited || vm->frame_count <= stop_frame_depth) {
            if (out_ret) *out_ret = val_nil();
            return true;
        }
        ZR_RELOAD_FRAME();
        vm_push(vm, val_nil());
        ZR_DISPATCH();
    }

handle_exception:;
    if (vm->handler_count > 0 && vm->handlers[vm->handler_count - 1].frame_idx >= stop_frame_depth) {
        TryHandler h = vm->handlers[--vm->handler_count];
        while (vm->frame_count - 1 > h.frame_idx) {
            size_t top_idx = vm->frame_count - 1;
            CallFrame *top_f = &vm->frames[top_idx];
            vm_run_frame_defers(vm, top_f, 0);
            vm_prof_exit_frame(vm, top_f, top_idx);
            vm_close_upvalues_from(vm, top_f->base + 1);
            vm->frame_count--;
        }
        CallFrame *target_f = &vm->frames[h.frame_idx];
        vm_run_frame_defers(vm, target_f, h.defer_depth);
        vm_close_upvalues_from(vm, h.stack_depth);
        vm->sp = h.stack_depth;
        target_f->ip = h.target_ip;
        PbStr *emsg = pb_str_c(vm, vm->err_msg);
        vm->err_msg[0] = '\0';
        ZR_RELOAD_FRAME();
        vm_push(vm, val_obj(emsg));
        ZR_DISPATCH();
    }

    if (vm->frame_count > stop_frame_depth && vm->err_msg[0] != '\0' && !strstr(vm->err_msg, " (at ")) {
        CallFrame *ef = &vm->frames[vm->frame_count - 1];
        PbFunc *efn = ef->closure ? ef->closure->fn : NULL;
        if (efn && efn->code && efn->code_len > 0) {
            uint32_t eip = ef->ip > 0 ? ef->ip - 1 : 0;
            if (eip >= efn->code_len) eip = efn->code_len - 1;
            uint32_t eln = efn->code[eip].line;
            const char *esrc = efn->source_name ? efn->source_name->chars : "<script>";
            const char *efname = efn->name ? efn->name->chars : "<main>";
            size_t cur_len = strlen(vm->err_msg);
            if (cur_len + 64 < sizeof(vm->err_msg)) {
                snprintf(vm->err_msg + cur_len, sizeof(vm->err_msg) - cur_len,
                         " (at %.160s:%u in %.60s)", esrc, (unsigned)eln, efname);
            }
        }
    }
    while (vm->frame_count > stop_frame_depth) {
        size_t top_idx = vm->frame_count - 1;
        CallFrame *top_f = &vm->frames[top_idx];
        vm_run_frame_defers(vm, top_f, 0);
        vm_prof_exit_frame(vm, top_f, top_idx);
        vm_close_upvalues_from(vm, top_f->base + 1);
        vm->sp = top_f->base;
        vm->frame_count--;
    }
    return false;

#undef ZR_DISPATCH
#undef ZR_CASE
#undef ZR_RELOAD_FRAME
#undef RUNTIME_ERR
}

static bool vm_call_value_sync(PbVM *vm, PbValue callee, int argc, const PbValue *argv, PbValue *out_ret) {
    if (callee.tag == VAL_BUILTIN) {
        PbValue ret = val_nil();
        if (!vm->builtin_defs[callee.as.builtin_id].fn(vm, argc, (PbValue *)argv, &ret)) return false;
        if (out_ret) *out_ret = ret;
        return true;
    }
    size_t saved_depth = vm->frame_count;
    size_t saved_sp = vm->sp;
    vm_push(vm, callee);
    for (int i = 0; i < argc; i++) vm_push(vm, argv[i]);
    if (!vm_call_top(vm, (uint32_t)argc, false, saved_depth)) {
        vm->sp = saved_sp;
        return false;
    }
    if (vm->frame_count <= saved_depth) {
        PbValue r = (vm->sp > saved_sp) ? vm->stack[vm->sp - 1] : val_nil();
        vm->sp = saved_sp;
        if (out_ret) *out_ret = r;
        return true;
    }
    return vm_execute_until(vm, saved_depth, out_ret);
}

static bool vm_run_root_func(PbVM *vm, PbFunc *root_fn, PbModule *mod, PbValue *out_ret) {
    if (!root_fn->peephole_done) zr_peephole_optimize_func(root_fn);
    PbMap *target_g = mod ? mod->globals : vm->globals;
    zr_populate_polyglot_globals(vm, target_g, root_fn);
    PbClosure *cl = pb_closure_new(vm, root_fn, 0, mod);
    return vm_call_value_sync(vm, val_obj(cl), 0, NULL, out_ret);
}

/* ---- VM Init & Destroy ---- */

static void vm_register_builtin(PbVM *vm, const char *name, PbBuiltinFn fn) {
    if (vm->builtin_count >= ZR_MAX_BUILTINS) return;
    uint16_t id = vm->builtin_count++;
    vm->builtin_defs[id].name = name;
    vm->builtin_defs[id].fn = fn;
    pb_map_set_cstr(vm, vm->builtins, name, val_builtin(id));
}

static void vm_init(PbVM *vm) {
    memset(vm, 0, sizeof(*vm));
    vm->gc_inhibit = 1;
    vm->nursery_enabled = false;
    vm->nursery_threshold = (sizeof(void *) <= 4) ? (256u * 1024u) : (512u * 1024u);
    vm->next_gc_bytes = (sizeof(void *) <= 4) ? (512u * 1024u) : (2u * 1024u * 1024u);
    vm->stack_cap = ZR_STACK_INIT;
    vm->stack = (PbValue *)zr_xcalloc(vm->stack_cap, sizeof(PbValue));
    zr_caps_init_default(&vm->caps);

    vm->globals = pb_map_new(vm);
    vm->exports = pb_map_new(vm);
    vm->builtins = pb_map_new(vm);
    vm->modules = pb_map_new(vm);
    vm->signal_handlers = pb_map_new(vm);

    zr_register_all_builtins(vm);
    vm->nursery_enabled = true;
    vm->gc_inhibit = 0;
}

static void vm_free(PbVM *vm) {
    vm->gc_inhibit = 1;
    zr_ffi_cleanup_vm(vm);
    zr_flight_cleanup_vm(vm);
    for (size_t i = 0; i < vm->frame_count; i++) {
        free(vm->frames[i].defers);
    }
    PbObj *ncur = vm->nursery_objects;
    while (ncur != NULL) {
        PbObj *nnext = ncur->next;
        gc_free_obj(vm, ncur);
        ncur = nnext;
    }
    PbObj *cur = vm->objects;
    while (cur != NULL) {
        PbObj *next = cur->next;
        gc_free_obj(vm, cur);
        cur = next;
    }
    free(vm->rem_set);
    free(vm->stack);
    free(vm->current_file);
    free(vm->stdlib_root);
    free(vm->exe_path);
    for (size_t i = 0; i < vm->caps.ffi_allowed_libs_len; i++) free(vm->caps.ffi_allowed_libs[i]);
    free(vm->caps.ffi_allowed_libs);
    for (size_t i = 0; i < vm->caps.ffi_allowed_syms_len; i++) free(vm->caps.ffi_allowed_syms[i]);
    free(vm->caps.ffi_allowed_syms);
    memset(vm, 0, sizeof(*vm));
}

/* ---- Cross-Header VM Helper Aliases ---- */

typedef PbBigIntObj   PbBigInt;
typedef PbRationalObj PbRational;
typedef PbDecimalObj  PbDecimal;

#define PBCK_MAGIC_BYTES "PBCK"
#define vm_raise(vm, ...) vm_error((vm), __VA_ARGS__)

static inline bool val_is_obj(PbValue v, PbObjType t) { return is_obj_type(v, t); }
static inline PbObj *val_as_obj(PbValue v) { return v.as.obj; }
static inline bool vm_is_truthy(PbValue v) { return val_truthy(v); }
static inline const char *vm_type_name(PbValue v) { return zr_val_type_name(v); }

static inline void vm_temp_push(PbVM *vm, PbValue v) { vm_push_root(vm, v); }
static inline void vm_temp_pop(PbVM *vm, size_t n) { vm_pop_roots(vm, n); }

static inline PbStr *vm_new_str(PbVM *vm, const char *s, size_t n) { return pb_str_new(vm, s, n); }
static inline PbStr *vm_new_cstr(PbVM *vm, const char *s) { return pb_str_c(vm, s); }
static inline PbStr *vm_const_str(PbVM *vm, const char *s) { return pb_str_c(vm, s); }
static inline PbArray *vm_new_array(PbVM *vm, size_t cap) { return pb_array_new(vm, cap); }
static inline PbMap *vm_new_map(PbVM *vm, size_t hint) { return pb_map_new_raw(vm, hint); }
static inline PbModule *vm_new_module(PbVM *vm, PbStr *path_str) {
    const char *p = path_str ? path_str->chars : "";
    return pb_module_new(vm, p, p);
}

static inline void arr_push(PbArray *a, PbValue v) { pb_array_push(NULL, a, v); }
static inline bool map_get(const PbMap *m, const PbStr *k, PbValue *out) { return pb_map_get(m, k, out); }
static inline bool map_get_cstr(const PbMap *m, const char *k, PbValue *out) { return pb_map_get_cstr(m, k, out); }
static inline void map_set(PbMap *m, PbStr *k, PbValue v) { pb_map_set(NULL, m, k, v); }
static inline bool map_delete(PbMap *m, const PbStr *k) { return pb_map_delete(m, k, NULL); }

static inline void vm_format_value(PbValue v, ZrBuf *b) {
    /* Format value into ZrBuf; safe with NULL vm unless printing unnamed builtin id */
    switch (v.tag) {
        case VAL_NIL:     zr_buf_append_cstr(b, "nil"); break;
        case VAL_BOOL:    zr_buf_append_cstr(b, v.as.b ? "true" : "false"); break;
        case VAL_NUM:     zr_format_number(b, v.as.num); break;
        case VAL_BUILTIN: zr_buf_printf(b, "<builtin#%u>", (unsigned)v.as.builtin_id); break;
        case VAL_OBJ:
            if (v.as.obj->type == OBJ_STR) {
                zr_buf_append(b, AS_STR(v)->chars, AS_STR(v)->len);
            } else if (v.as.obj->type == OBJ_BIGINT) {
                char *s = zr_bi_to_str(&AS_BIGINT(v)->v);
                zr_buf_append_cstr(b, s);
                free(s);
            } else if (v.as.obj->type == OBJ_RATIONAL) {
                char *s = zr_rat_to_str(&AS_RATIONAL(v)->v);
                zr_buf_append_cstr(b, s);
                free(s);
            } else if (v.as.obj->type == OBJ_DECIMAL) {
                char *s = zr_dec_to_str(&AS_DECIMAL(v)->v);
                zr_buf_append_cstr(b, s);
                free(s);
            } else if (v.as.obj->type == OBJ_BYTES) {
                zr_buf_printf(b, "<bytes len=%zu>", AS_BYTES(v)->len);
            } else if (v.as.obj->type == OBJ_F64BUF) {
                zr_buf_printf(b, "<f64buf len=%zu>", AS_F64BUF(v)->len);
            } else {
                zr_buf_printf(b, "<%s>", zr_val_type_name(v));
            }
            break;
    }
}

static int pb_map_entry_cmp(const void *a, const void *b) {
    const PbMapEntry *ea = *(const PbMapEntry *const *)a;
    const PbMapEntry *eb = *(const PbMapEntry *const *)b;
    return zr_cmp_pbstr_ptr(&ea->key, &eb->key);
}

#endif /* PBLANG_ZR_VM_CORE_H */
