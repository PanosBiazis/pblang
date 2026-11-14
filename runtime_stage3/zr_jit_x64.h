/* ============================================================================
 * zr_jit_x64.h — Sovereign Zero-Rust Runtime v3.0.0
 * Peephole Superinstruction Optimizer + W^X x86_64 SSE2 Native JIT Compiler
 *
 * Strictly portable C99:
 *   - Peephole optimizer runs on ALL architectures (keeping .pbck v1 0..=47
 *     untouched in ins->orig_op and ins->arg).
 *   - Native x86_64 SSE2 JIT is guarded by:
 *       #if (defined(__x86_64__) || defined(_M_X64)) && !defined(__wasi__)
 *     and enforces strict W^X page permissions (PROT_READ|PROT_WRITE during
 *     emission -> mprotect(PROT_READ|PROT_EXEC) on POSIX; VirtualAlloc
 *     PAGE_READWRITE -> VirtualProtect PAGE_EXECUTE_READ on Win64; never RWX).
 * ============================================================================ */
#ifndef PBLANG_ZR_JIT_X64_H
#define PBLANG_ZR_JIT_X64_H

#include "zr_vm_core.h"

#if defined(__GNUC__) && !defined(__clang__)
#pragma GCC push_options
#pragma GCC optimize("O3,omit-frame-pointer")
#endif

/* ============================================================================
 * 1. In-Memory Peephole Superinstruction Pass (All Architectures)
 * ============================================================================ */

static int zr_cmp_op_to_kind(uint8_t op) {
    if (op == OP_LT) return 0;
    if (op == OP_LE) return 1;
    if (op == OP_GT) return 2;
    if (op == OP_GE) return 3;
    return -1;
}

static void zr_peephole_optimize_func(PbFunc *fn) {
    if (!fn || fn->peephole_done) return;
    fn->peephole_done = true;

    /* Recursively optimize nested function constants first */
    for (uint32_t c = 0; c < fn->const_len; c++) {
        if (is_obj_type(fn->consts[c], OBJ_FUNC)) {
            zr_peephole_optimize_func(AS_FUNC(fn->consts[c]));
        }
    }

    uint32_t n = fn->code_len;
    if (n < 3) {
        for (uint32_t i = 0; i < n; i++) {
            if (fn->code[i].orig_op == 0 && fn->code[i].op <= OP_THROW) {
                fn->code[i].orig_op = fn->code[i].op;
            }
        }
        return;
    }

    for (uint32_t i = 0; i < n; i++) {
        if (fn->code[i].op <= OP_THROW) {
            fn->code[i].orig_op = fn->code[i].op;
        }
        fn->code[i].op = fn->code[i].orig_op;
        fn->code[i].fused_skip = 0;
    }

    uint8_t *is_target = (uint8_t *)zr_xcalloc((size_t)n + 1, 1);
    for (uint32_t i = 0; i < n; i++) {
        uint8_t op = fn->code[i].orig_op;
        if (op == OP_JUMP || op == OP_JUMP_IF_FALSE || op == OP_JUMP_IF_TRUE || op == OP_TRY) {
            uint32_t tgt = fn->code[i].arg;
            if (tgt <= n) is_target[tgt] = 1;
        }
    }

    uint32_t i = 0;
    while (i < n) {
        PbInstr *ins = &fn->code[i];
        uint8_t op0 = ins->orig_op;

        if (op0 == OP_GET_LOCAL) {
            /* Pattern A (5 instrs): GET_LOCAL a; CONSTANT c(num); LT/LE/GT/GE; JUMP_IF_FALSE tgt; POP */
            if (i + 4 < n && !is_target[i + 1] && !is_target[i + 2] && !is_target[i + 3] && !is_target[i + 4] &&
                fn->code[i + 1].orig_op == OP_CONSTANT &&
                fn->code[i + 1].arg < fn->const_len &&
                fn->consts[fn->code[i + 1].arg].tag == VAL_NUM &&
                zr_cmp_op_to_kind(fn->code[i + 2].orig_op) >= 0 &&
                fn->code[i + 3].orig_op == OP_JUMP_IF_FALSE &&
                fn->code[i + 4].orig_op == OP_POP) {
                ins->op = OP_LT_LOCAL_CONST_JMPF;
                ins->arg2 = fn->code[i + 1].arg;
                ins->ic_slot = zr_cmp_op_to_kind(fn->code[i + 2].orig_op);
                ins->ic_bcap = (size_t)fn->code[i + 3].arg;
                ins->fused_skip = 4;
                i += 5;
                continue;
            }
            /* Pattern B (5 instrs): GET_LOCAL a; GET_LOCAL b; LT/LE/GT/GE; JUMP_IF_FALSE tgt; POP */
            if (i + 4 < n && !is_target[i + 1] && !is_target[i + 2] && !is_target[i + 3] && !is_target[i + 4] &&
                fn->code[i + 1].orig_op == OP_GET_LOCAL &&
                zr_cmp_op_to_kind(fn->code[i + 2].orig_op) >= 0 &&
                fn->code[i + 3].orig_op == OP_JUMP_IF_FALSE &&
                fn->code[i + 4].orig_op == OP_POP) {
                ins->op = OP_LT_LOCAL_LOCAL_JMPF;
                ins->arg2 = fn->code[i + 1].arg;
                ins->ic_slot = zr_cmp_op_to_kind(fn->code[i + 2].orig_op);
                ins->ic_bcap = (size_t)fn->code[i + 3].arg;
                ins->fused_skip = 4;
                i += 5;
                continue;
            }
            /* Pattern C (4 instrs): GET_LOCAL a; CONSTANT c(num); ADD|SUB; SET_LOCAL a */
            if (i + 3 < n && !is_target[i + 1] && !is_target[i + 2] && !is_target[i + 3] &&
                fn->code[i + 1].orig_op == OP_CONSTANT &&
                fn->code[i + 1].arg < fn->const_len &&
                fn->consts[fn->code[i + 1].arg].tag == VAL_NUM &&
                (fn->code[i + 2].orig_op == OP_ADD || fn->code[i + 2].orig_op == OP_SUB) &&
                fn->code[i + 3].orig_op == OP_SET_LOCAL &&
                fn->code[i + 3].arg == ins->arg) {
                ins->op = OP_INC_LOCAL_1;
                ins->arg2 = fn->code[i + 1].arg;
                ins->ic_slot = (fn->code[i + 2].orig_op == OP_SUB) ? 1 : 0;
                ins->fused_skip = 3;
                i += 4;
                continue;
            }
            /* Pattern D (3 instrs): GET_LOCAL a; GET_LOCAL b; ADD|SUB */
            if (i + 2 < n && !is_target[i + 1] && !is_target[i + 2] &&
                fn->code[i + 1].orig_op == OP_GET_LOCAL &&
                (fn->code[i + 2].orig_op == OP_ADD || fn->code[i + 2].orig_op == OP_SUB)) {
                ins->op = OP_ADD_LOCAL_LOCAL;
                ins->arg2 = fn->code[i + 1].arg;
                ins->ic_slot = (fn->code[i + 2].orig_op == OP_SUB) ? 1 : 0;
                ins->fused_skip = 2;
                i += 3;
                continue;
            }
            /* Pattern E (3 instrs): GET_LOCAL a; CONSTANT c(num); SUB|ADD */
            if (i + 2 < n && !is_target[i + 1] && !is_target[i + 2] &&
                fn->code[i + 1].orig_op == OP_CONSTANT &&
                fn->code[i + 1].arg < fn->const_len &&
                fn->consts[fn->code[i + 1].arg].tag == VAL_NUM &&
                (fn->code[i + 2].orig_op == OP_SUB || fn->code[i + 2].orig_op == OP_ADD)) {
                ins->op = OP_SUB_LOCAL_CONST;
                ins->arg2 = fn->code[i + 1].arg;
                ins->ic_slot = (fn->code[i + 2].orig_op == OP_SUB) ? 1 : 0;
                ins->fused_skip = 2;
                i += 3;
                continue;
            }
        } else if (op0 == OP_GET_GLOBAL && ins->arg < fn->const_len && is_obj_type(fn->consts[ins->arg], OBJ_STR)) {
            /* Pattern F (5 instrs): GET_GLOBAL g; CONSTANT c(num); LT/LE/GT/GE; JUMP_IF_FALSE tgt; POP */
            if (i + 4 < n && !is_target[i + 1] && !is_target[i + 2] && !is_target[i + 3] && !is_target[i + 4] &&
                fn->code[i + 1].orig_op == OP_CONSTANT &&
                fn->code[i + 1].arg < fn->const_len &&
                fn->consts[fn->code[i + 1].arg].tag == VAL_NUM &&
                zr_cmp_op_to_kind(fn->code[i + 2].orig_op) >= 0 &&
                fn->code[i + 3].orig_op == OP_JUMP_IF_FALSE &&
                fn->code[i + 4].orig_op == OP_POP) {
                ins->op = OP_LT_GLOBAL_CONST_JMPF;
                ins->arg2 = fn->code[i + 1].arg;
                ins->pad_u8 = (uint8_t)zr_cmp_op_to_kind(fn->code[i + 2].orig_op);
                ins->fused_skip = 4;
                i += 5;
                continue;
            }
            /* Pattern G (4 instrs): GET_GLOBAL g; CONSTANT c(num); ADD|SUB; SET_GLOBAL g */
            if (i + 3 < n && !is_target[i + 1] && !is_target[i + 2] && !is_target[i + 3] &&
                fn->code[i + 1].orig_op == OP_CONSTANT &&
                fn->code[i + 1].arg < fn->const_len &&
                fn->consts[fn->code[i + 1].arg].tag == VAL_NUM &&
                (fn->code[i + 2].orig_op == OP_ADD || fn->code[i + 2].orig_op == OP_SUB) &&
                fn->code[i + 3].orig_op == OP_SET_GLOBAL &&
                fn->code[i + 3].arg == ins->arg) {
                ins->op = OP_INC_GLOBAL_1;
                ins->arg2 = fn->code[i + 1].arg;
                ins->pad_u8 = (uint8_t)((fn->code[i + 2].orig_op == OP_SUB) ? 1 : 0);
                ins->fused_skip = 3;
                i += 4;
                continue;
            }
        }
        i++;
    }

    free(is_target);
}

/* ============================================================================
 * 2. Real W^X x86_64 SSE2 Function JIT Compiler
 * ============================================================================ */

#if (defined(__x86_64__) || defined(_M_X64)) && !defined(__wasi__)
#define ZR_JIT_ARCH_AVAILABLE 1
#define ZR_JIT_ARCH_NAME      "x86_64"

enum {
    JTAG_NONE = 0,
    JTAG_NUM  = 1,
    JTAG_BOOL = 2,
    JTAG_SELF = 3
};

#define ZR_JIT_MAX_SLOTS 64

typedef struct {
    uint8_t *buf;
    size_t   len;
    size_t   cap;
} ZrJitEmit;

typedef struct {
    size_t   disp_pos; /* position of 4-byte rel32 operand */
    uint32_t target_ip;
    uint8_t  kind;     /* 0 = bytecode ip, 1 = jit_body, 2 = tail_loop, 3 = bailout */
} ZrJitReloc;

static void jemit_reserve(ZrJitEmit *e, size_t extra) {
    if (e->len + extra <= e->cap) return;
    size_t ncap = e->cap ? e->cap * 2 : 512;
    while (ncap < e->len + extra) ncap *= 2;
    e->buf = (uint8_t *)zr_xrealloc(e->buf, ncap);
    e->cap = ncap;
}

static inline void jemit_u8(ZrJitEmit *e, uint8_t b) {
    jemit_reserve(e, 1);
    e->buf[e->len++] = b;
}

static inline void jemit_bytes(ZrJitEmit *e, const void *src, size_t n) {
    jemit_reserve(e, n);
    memcpy(e->buf + e->len, src, n);
    e->len += n;
}

static inline void jemit_i32(ZrJitEmit *e, int32_t v) {
    uint32_t u = (uint32_t)v;
    jemit_reserve(e, 4);
    e->buf[e->len++] = (uint8_t)(u & 0xFFu);
    e->buf[e->len++] = (uint8_t)((u >> 8) & 0xFFu);
    e->buf[e->len++] = (uint8_t)((u >> 16) & 0xFFu);
    e->buf[e->len++] = (uint8_t)((u >> 24) & 0xFFu);
}

static inline void jemit_u64(ZrJitEmit *e, uint64_t u) {
    jemit_reserve(e, 8);
    for (int i = 0; i < 8; i++) {
        e->buf[e->len++] = (uint8_t)((u >> (i * 8)) & 0xFFu);
    }
}

static inline void jpatch_i32(uint8_t *buf, size_t pos, int32_t v) {
    uint32_t u = (uint32_t)v;
    buf[pos + 0] = (uint8_t)(u & 0xFFu);
    buf[pos + 1] = (uint8_t)((u >> 8) & 0xFFu);
    buf[pos + 2] = (uint8_t)((u >> 16) & 0xFFu);
    buf[pos + 3] = (uint8_t)((u >> 24) & 0xFFu);
}

static inline int32_t jslot_disp(uint32_t slot) {
    return -(int32_t)((slot + 1u) * 8u);
}

/* Load slot into xmm<xmm_reg> (xmm_reg in 0..5).
 * When use_reg_slots is true (leaf functions with no recursive calls),
 * slots 0..3 live directly in volatile SSE2 registers xmm2..xmm5! */
static void jemit_load_slot_xmm_ex(ZrJitEmit *e, uint8_t xmm_reg, uint32_t slot, bool use_reg_slots) {
    if (use_reg_slots && slot < 4u) {
        uint8_t sreg = (uint8_t)(2u + slot);
        if (xmm_reg == sreg) return;
        jemit_u8(e, 0xF2);
        jemit_u8(e, 0x0F);
        jemit_u8(e, 0x10);
        jemit_u8(e, (uint8_t)(0xC0u | ((xmm_reg & 7u) << 3) | (sreg & 7u)));
        return;
    }
    jemit_u8(e, 0xF2);
    jemit_u8(e, 0x0F);
    jemit_u8(e, 0x10);
    jemit_u8(e, (uint8_t)(0x85u | ((xmm_reg & 7u) << 3)));
    jemit_i32(e, jslot_disp(slot));
}

static void jemit_store_slot_xmm_ex(ZrJitEmit *e, uint32_t slot, uint8_t xmm_reg, bool use_reg_slots) {
    if (use_reg_slots && slot < 4u) {
        uint8_t sreg = (uint8_t)(2u + slot);
        if (sreg == xmm_reg) return;
        jemit_u8(e, 0xF2);
        jemit_u8(e, 0x0F);
        jemit_u8(e, 0x10);
        jemit_u8(e, (uint8_t)(0xC0u | ((sreg & 7u) << 3) | (xmm_reg & 7u)));
        return;
    }
    jemit_u8(e, 0xF2);
    jemit_u8(e, 0x0F);
    jemit_u8(e, 0x11);
    jemit_u8(e, (uint8_t)(0x85u | ((xmm_reg & 7u) << 3)));
    jemit_i32(e, jslot_disp(slot));
}

static void jemit_const_xmm(ZrJitEmit *e, uint8_t xmm_reg, double d) {
    uint64_t bits = 0;
    memcpy(&bits, &d, sizeof(bits));
    if (bits == 0ULL) {
        /* xorpd xmm<reg>, xmm<reg> */
        jemit_u8(e, 0x66);
        jemit_u8(e, 0x0F);
        jemit_u8(e, 0x57);
        jemit_u8(e, (uint8_t)(0xC0u | ((xmm_reg & 7u) << 3) | (xmm_reg & 7u)));
    } else {
        /* movabs rax, imm64 ; movq xmm<reg>, rax */
        jemit_u8(e, 0x48);
        jemit_u8(e, 0xB8);
        jemit_u64(e, bits);
        jemit_u8(e, 0x66);
        jemit_u8(e, 0x48);
        jemit_u8(e, 0x0F);
        jemit_u8(e, 0x6E);
        jemit_u8(e, (uint8_t)(0xC0u | ((xmm_reg & 7u) << 3)));
    }
}

static void jpush_reloc(ZrJitReloc **arr, size_t *len, size_t *cap, size_t disp_pos, uint32_t target_ip, uint8_t kind) {
    if (*len >= *cap) {
        *cap = *cap ? (*cap * 2) : 32;
        *arr = (ZrJitReloc *)zr_xrealloc(*arr, (*cap) * sizeof(ZrJitReloc));
    }
    (*arr)[*len].disp_pos = disp_pos;
    (*arr)[*len].target_ip = target_ip;
    (*arr)[*len].kind = kind;
    (*len)++;
}

static void *zr_jit_alloc_wx(const uint8_t *code, size_t len, size_t *out_alloc_sz) {
#ifdef _WIN32
    SYSTEM_INFO si;
    GetSystemInfo(&si);
    size_t gran = si.dwPageSize ? (size_t)si.dwPageSize : 4096u;
    size_t sz = (len + gran - 1u) & ~(gran - 1u);
    if (sz == 0) sz = gran;
    /* Step 1: Allocate RW only (NEVER PAGE_EXECUTE_READWRITE!) */
    void *mem = VirtualAlloc(NULL, sz, MEM_COMMIT | MEM_RESERVE, PAGE_READWRITE);
    if (!mem) return NULL;
    memcpy(mem, code, len);
    /* Step 2: Transition RW -> RX (strict W^X) */
    DWORD old_prot = 0;
    if (!VirtualProtect(mem, sz, PAGE_EXECUTE_READ, &old_prot)) {
        VirtualFree(mem, 0, MEM_RELEASE);
        return NULL;
    }
    FlushInstructionCache(GetCurrentProcess(), mem, len);
    *out_alloc_sz = sz;
    return mem;
#else
    long ps = sysconf(_SC_PAGESIZE);
    size_t page_sz = (ps > 0) ? (size_t)ps : 4096u;
    size_t sz = (len + page_sz - 1u) & ~(page_sz - 1u);
    if (sz == 0) sz = page_sz;
#ifndef MAP_ANONYMOUS
#ifdef MAP_ANON
#define MAP_ANONYMOUS MAP_ANON
#else
#define MAP_ANONYMOUS 0x20
#endif
#endif
    /* Step 1: Allocate PROT_READ | PROT_WRITE only (NEVER PROT_WRITE | PROT_EXEC!) */
    void *mem = mmap(NULL, sz, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
    if (mem == MAP_FAILED || !mem) return NULL;
    memcpy(mem, code, len);
    /* Step 2: Transition RW -> RX (strict W^X) */
    if (mprotect(mem, sz, PROT_READ | PROT_EXEC) != 0) {
        munmap(mem, sz);
        return NULL;
    }
#if defined(__GNUC__) || defined(__clang__)
    __builtin___clear_cache((char *)mem, (char *)mem + len);
#endif
    *out_alloc_sz = sz;
    return mem;
#endif
}

static void zr_jit_free_code(PbFunc *fn) {
    if (!fn || !fn->jit_code) return;
#ifdef _WIN32
    VirtualFree(fn->jit_code, 0, MEM_RELEASE);
#else
    if (fn->jit_code_bytes > 0) {
        munmap(fn->jit_code, fn->jit_code_bytes);
    }
#endif
    fn->jit_code = NULL;
    fn->jit_code_bytes = 0;
    fn->jit_fn = NULL;
}

static bool zr_jit_compile_func(PbVM *vm, PbFunc *fn) {
    if (!fn) return false;
    if (fn->jit_fn != NULL) return true;
    fn->jit_attempted = true;

    if (!fn->peephole_done) zr_peephole_optimize_func(fn);
    if (fn->is_vararg || fn->is_async || fn->arity > 8 || fn->code_len == 0 || fn->code_len > 4096) {
        return vm_error(vm, "jit_compile: function '%s' exceeds arity/async/size limits",
                        fn->name ? fn->name->chars : "<fn>");
    }

    uint32_t n = fn->code_len;
    uint8_t *visited = (uint8_t *)zr_xcalloc(n, 1);
    uint8_t *in_work = (uint8_t *)zr_xcalloc(n, 1);
    uint32_t *sp_at = (uint32_t *)zr_xcalloc(n, sizeof(uint32_t));
    uint8_t (*tags_at)[ZR_JIT_MAX_SLOTS] =
        (uint8_t (*)[ZR_JIT_MAX_SLOTS])zr_xcalloc(n, ZR_JIT_MAX_SLOTS);
    uint32_t *work = (uint32_t *)zr_xmalloc((size_t)n * sizeof(uint32_t));
    size_t wlen = 0;

    visited[0] = 1;
    in_work[0] = 1;
    sp_at[0] = fn->arity;
    for (uint32_t p = 0; p < fn->arity; p++) tags_at[0][p] = JTAG_NUM;
    work[wlen++] = 0;

    uint32_t max_slots = fn->arity;
    int8_t self_kind = 0;
    uint32_t self_arg = 0;
    uint8_t ret_tag = JTAG_NONE;
    bool ok = true;

#define JIT_ENQUEUE(tgt_ip, next_sp, cur_tags) do { \
    uint32_t _t = (tgt_ip); \
    uint32_t _nsp = (next_sp); \
    if (_t >= n || _nsp > ZR_JIT_MAX_SLOTS) { ok = false; break; } \
    if (!visited[_t]) { \
        visited[_t] = 1; \
        sp_at[_t] = _nsp; \
        if (_nsp > 0) memcpy(tags_at[_t], (cur_tags), _nsp); \
        if (!in_work[_t]) { in_work[_t] = 1; work[wlen++] = _t; } \
    } else if (sp_at[_t] != _nsp) { \
        ok = false; break; \
    } else { \
        bool _changed = false; \
        for (uint32_t _k = 0; _k < _nsp; _k++) { \
            if (tags_at[_t][_k] != (cur_tags)[_k]) { \
                /* Coalesce JTAG_BOOL and JTAG_NUM as JTAG_NUM if mixed across branches */ \
                if ((tags_at[_t][_k] == JTAG_NUM || tags_at[_t][_k] == JTAG_BOOL) && \
                    ((cur_tags)[_k] == JTAG_NUM || (cur_tags)[_k] == JTAG_BOOL)) { \
                    if (tags_at[_t][_k] != JTAG_NUM) { tags_at[_t][_k] = JTAG_NUM; _changed = true; } \
                } else { ok = false; break; } \
            } \
        } \
        if (!ok) break; \
        if (_changed && !in_work[_t]) { in_work[_t] = 1; work[wlen++] = _t; } \
    } \
} while (0)

    while (wlen > 0 && ok) {
        uint32_t ip = work[--wlen];
        in_work[ip] = 0;
        uint32_t sp = sp_at[ip];
        uint8_t tcur[ZR_JIT_MAX_SLOTS];
        if (sp > 0) memcpy(tcur, tags_at[ip], sp);
        if (sp > max_slots) max_slots = sp;

        const PbInstr *ins = &fn->code[ip];
        uint8_t op = ins->orig_op ? ins->orig_op : ins->op;
        if (op == 0 && ins->op == OP_CONSTANT) op = OP_CONSTANT;

        switch (op) {
            case OP_CONSTANT: {
                if (ins->arg >= fn->const_len || sp >= ZR_JIT_MAX_SLOTS) { ok = false; break; }
                PbValue cv = fn->consts[ins->arg];
                if (cv.tag == VAL_NUM && cv.taint == 0) tcur[sp++] = JTAG_NUM;
                else if (cv.tag == VAL_BOOL && cv.taint == 0) tcur[sp++] = JTAG_BOOL;
                else { ok = false; break; }
                if (sp > max_slots) max_slots = sp;
                JIT_ENQUEUE(ip + 1, sp, tcur);
                break;
            }
            case OP_POP: {
                if (sp < 1) { ok = false; break; }
                sp--;
                JIT_ENQUEUE(ip + 1, sp, tcur);
                break;
            }
            case OP_DUP: {
                if (sp < 1 || sp >= ZR_JIT_MAX_SLOTS) { ok = false; break; }
                tcur[sp] = tcur[sp - 1];
                sp++;
                if (sp > max_slots) max_slots = sp;
                JIT_ENQUEUE(ip + 1, sp, tcur);
                break;
            }
            case OP_GET_LOCAL: {
                if (ins->arg >= sp || sp >= ZR_JIT_MAX_SLOTS) { ok = false; break; }
                uint8_t lt = tcur[ins->arg];
                if (lt != JTAG_NUM && lt != JTAG_BOOL) { ok = false; break; }
                tcur[sp++] = lt;
                if (sp > max_slots) max_slots = sp;
                JIT_ENQUEUE(ip + 1, sp, tcur);
                break;
            }
            case OP_SET_LOCAL: {
                if (sp < 1) { ok = false; break; }
                uint8_t vt = tcur[--sp];
                if (ins->arg >= sp || (vt != JTAG_NUM && vt != JTAG_BOOL)) { ok = false; break; }
                tcur[ins->arg] = vt;
                JIT_ENQUEUE(ip + 1, sp, tcur);
                break;
            }
            case OP_ADD:
            case OP_SUB:
            case OP_MUL:
            case OP_DIV: {
                if (sp < 2 || tcur[sp - 2] != JTAG_NUM || tcur[sp - 1] != JTAG_NUM) { ok = false; break; }
                sp--;
                tcur[sp - 1] = JTAG_NUM;
                JIT_ENQUEUE(ip + 1, sp, tcur);
                break;
            }
            case OP_NEGATE: {
                if (sp < 1 || tcur[sp - 1] != JTAG_NUM) { ok = false; break; }
                JIT_ENQUEUE(ip + 1, sp, tcur);
                break;
            }
            case OP_NOT: {
                if (sp < 1 || (tcur[sp - 1] != JTAG_NUM && tcur[sp - 1] != JTAG_BOOL)) { ok = false; break; }
                tcur[sp - 1] = JTAG_BOOL;
                JIT_ENQUEUE(ip + 1, sp, tcur);
                break;
            }
            case OP_EQ:
            case OP_NOT_EQ: {
                if (sp < 2) { ok = false; break; }
                uint8_t ta = tcur[sp - 2], tb = tcur[sp - 1];
                if ((ta != JTAG_NUM && ta != JTAG_BOOL) || ta != tb) { ok = false; break; }
                sp--;
                tcur[sp - 1] = JTAG_BOOL;
                JIT_ENQUEUE(ip + 1, sp, tcur);
                break;
            }
            case OP_LT:
            case OP_LE:
            case OP_GT:
            case OP_GE: {
                if (sp < 2 || tcur[sp - 2] != JTAG_NUM || tcur[sp - 1] != JTAG_NUM) { ok = false; break; }
                sp--;
                tcur[sp - 1] = JTAG_BOOL;
                JIT_ENQUEUE(ip + 1, sp, tcur);
                break;
            }
            case OP_JUMP: {
                JIT_ENQUEUE(ins->arg, sp, tcur);
                break;
            }
            case OP_JUMP_IF_FALSE:
            case OP_JUMP_IF_TRUE: {
                if (sp < 1 || (tcur[sp - 1] != JTAG_NUM && tcur[sp - 1] != JTAG_BOOL)) { ok = false; break; }
                JIT_ENQUEUE(ip + 1, sp, tcur);
                if (ok) JIT_ENQUEUE(ins->arg, sp, tcur);
                break;
            }
            case OP_GET_GLOBAL: {
                if (!fn->name || ins->arg >= fn->const_len || !is_obj_type(fn->consts[ins->arg], OBJ_STR) ||
                    strcmp(AS_STR(fn->consts[ins->arg])->chars, fn->name->chars) != 0 ||
                    sp >= ZR_JIT_MAX_SLOTS || (self_kind != 0 && (self_kind != 1 || self_arg != ins->arg))) {
                    ok = false;
                    break;
                }
                self_kind = 1;
                self_arg = ins->arg;
                tcur[sp++] = JTAG_SELF;
                if (sp > max_slots) max_slots = sp;
                JIT_ENQUEUE(ip + 1, sp, tcur);
                break;
            }
            case OP_GET_UPVALUE: {
                if (ins->arg != 0 || sp >= ZR_JIT_MAX_SLOTS || (self_kind != 0 && self_kind != 2)) {
                    ok = false;
                    break;
                }
                self_kind = 2;
                self_arg = 0;
                tcur[sp++] = JTAG_SELF;
                if (sp > max_slots) max_slots = sp;
                JIT_ENQUEUE(ip + 1, sp, tcur);
                break;
            }
            case OP_CALL:
            case OP_TAIL_CALL: {
                uint32_t cargc = ins->arg;
                if (cargc != fn->arity || sp < cargc + 1) { ok = false; break; }
                uint32_t cslot = sp - 1 - cargc;
                if (tcur[cslot] != JTAG_SELF) { ok = false; break; }
                for (uint32_t a = 0; a < cargc; a++) {
                    if (tcur[cslot + 1 + a] != JTAG_NUM) { ok = false; break; }
                }
                if (!ok) break;
                if (ret_tag == JTAG_BOOL) { ok = false; break; }
                ret_tag = JTAG_NUM;
                sp = cslot + 1;
                tcur[cslot] = JTAG_NUM;
                JIT_ENQUEUE(ip + 1, sp, tcur);
                break;
            }
            case OP_RETURN: {
                if (sp < 1) { ok = false; break; }
                uint8_t rt = tcur[sp - 1];
                if (rt != JTAG_NUM && rt != JTAG_BOOL) { ok = false; break; }
                if (ret_tag == JTAG_NONE) ret_tag = rt;
                else if (ret_tag != rt) { ok = false; break; }
                break;
            }
            default:
                ok = false;
                break;
        }
    }
#undef JIT_ENQUEUE

    if (!ok || ret_tag == JTAG_NONE) {
        free(visited);
        free(in_work);
        free(sp_at);
        free(tags_at);
        free(work);
        return vm_error(vm, "jit_compile: function '%s' uses non-numeric/unsupported opcodes",
                        fn->name ? fn->name->chars : "<fn>");
    }

    /* ---- Emit x86_64 + SSE2 Machine Code ---- */
    bool use_reg_slots = (self_kind == 0);
    bool has_div = false;
    uint8_t *is_jtarget = (uint8_t *)zr_xcalloc((size_t)n + 1, 1);
    for (uint32_t k = 0; k < n; k++) {
        if (visited[k]) {
            uint8_t kop = fn->code[k].orig_op ? fn->code[k].orig_op : fn->code[k].op;
            if (kop == OP_DIV) has_div = true;
            if ((kop == OP_JUMP || kop == OP_JUMP_IF_FALSE || kop == OP_JUMP_IF_TRUE) && fn->code[k].arg <= n) {
                is_jtarget[fn->code[k].arg] = 1;
            }
        }
    }

    uint64_t cpool[128];
    uint32_t cpool_len = 0;
#define JIT_CPOOL_IDX(dval, out_idx) do { \
    uint64_t _cbits = 0; \
    double _cd = (dval); \
    memcpy(&_cbits, &_cd, 8); \
    int _found = -1; \
    for (uint32_t _ci = 0; _ci < cpool_len; _ci++) { \
        if (cpool[_ci] == _cbits) { _found = (int)_ci; break; } \
    } \
    if (_found < 0 && cpool_len < 128u) { \
        _found = (int)cpool_len; \
        cpool[cpool_len++] = _cbits; \
    } \
    (out_idx) = _found; \
} while (0)

    ZrJitEmit em = { NULL, 0, 0 };
    ZrJitReloc *relocs = NULL;
    size_t reloc_len = 0, reloc_cap = 0;
    size_t *ip_off = (size_t *)zr_xcalloc((size_t)n + 1, sizeof(size_t));

    /* 1. OS ABI entry_stub at offset 0:
     *    Win64: rcx = args, rdx = bailout -> r8 = args, r10 = bailout
     *    SysV:  rdi = args, rsi = bailout -> r8 = args, r10 = bailout */
#ifdef _WIN32
    {
        static const uint8_t w64_in[6] = { 0x49, 0x89, 0xC8, 0x49, 0x89, 0xD2 };
        jemit_bytes(&em, w64_in, 6);
    }
#else
    {
        static const uint8_t sysv_in[6] = { 0x49, 0x89, 0xF8, 0x49, 0x89, 0xF2 };
        jemit_bytes(&em, sysv_in, 6);
    }
#endif
    if (!use_reg_slots) {
        for (uint32_t p = 0; p < fn->arity; p++) {
            if (p < 6) {
                /* movsd xmm<p>, [r8 + p*8] */
                jemit_u8(&em, 0xF2);
                jemit_u8(&em, 0x41);
                jemit_u8(&em, 0x0F);
                jemit_u8(&em, 0x10);
                jemit_u8(&em, (uint8_t)(0x80u | (p << 3)));
                jemit_i32(&em, (int32_t)(p * 8u));
            } else if (p == 6) {
                static const uint8_t ld6[4] = { 0x49, 0x8B, 0x40, 0x30 };
                jemit_bytes(&em, ld6, 4);
            } else {
                static const uint8_t ld7[4] = { 0x49, 0x8B, 0x50, 0x38 };
                jemit_bytes(&em, ld7, 4);
            }
        }
    }

    /* 2. Internal recursive entry jit_body */
    size_t jit_body_off = em.len;
    uint32_t stack_needed = (use_reg_slots && max_slots <= 4u) ? 0u : max_slots;
    uint32_t frame_slots = (stack_needed + 1u) & ~1u; /* even count -> 16-byte stack alignment */
    int32_t frame_bytes = (int32_t)(frame_slots * 8u);
    /* push rbp ; mov rbp, rsp */
    {
        static const uint8_t prol[4] = { 0x55, 0x48, 0x89, 0xE5 };
        jemit_bytes(&em, prol, 4);
    }
    if (frame_bytes > 0) {
        /* sub rsp, imm32 */
        jemit_u8(&em, 0x48);
        jemit_u8(&em, 0x81);
        jemit_u8(&em, 0xEC);
        jemit_i32(&em, frame_bytes);
    }

    /* Store incoming args into register/stack slots (tail-call loop target!) */
    size_t tail_loop_off = em.len;
    if (use_reg_slots) {
        for (uint32_t p = 0; p < fn->arity; p++) {
            /* movsd xmm0, [r8 + p*8] -> store slot p */
            jemit_u8(&em, 0xF2);
            jemit_u8(&em, 0x41);
            jemit_u8(&em, 0x0F);
            jemit_u8(&em, 0x10);
            jemit_u8(&em, 0x80);
            jemit_i32(&em, (int32_t)(p * 8u));
            jemit_store_slot_xmm_ex(&em, p, 0, true);
        }
    } else {
        for (uint32_t p = 0; p < fn->arity; p++) {
            if (p < 6) {
                jemit_store_slot_xmm_ex(&em, p, (uint8_t)p, false);
            } else if (p == 6) {
                jemit_u8(&em, 0x48);
                jemit_u8(&em, 0x89);
                jemit_u8(&em, 0x85);
                jemit_i32(&em, jslot_disp(6));
            } else {
                jemit_u8(&em, 0x48);
                jemit_u8(&em, 0x89);
                jemit_u8(&em, 0x95);
                jemit_i32(&em, jslot_disp(7));
            }
        }
    }

    /* 3. Emit reachable instructions (including peephole-fused SSE2 superinstructions) */
    for (uint32_t ip = 0; ip < n; ip++) {
        ip_off[ip] = em.len;
        if (!visited[ip]) continue;

        uint32_t sp = sp_at[ip];
        const PbInstr *ins = &fn->code[ip];

        /* Native SSE2 superinstruction fast-paths */
        if ((ins->op == OP_LT_LOCAL_CONST_JMPF || ins->op == OP_LT_LOCAL_LOCAL_JMPF) &&
            ins->fused_skip == 4 && ip + 4u < n) {
            uint8_t lv_reg = 0;
            if (use_reg_slots && ins->arg < 4u) {
                lv_reg = (uint8_t)(2u + ins->arg);
            } else if (!(ip == 0u && !use_reg_slots && fn->arity >= 1u && ins->arg == 0u && !is_jtarget[0])) {
                jemit_load_slot_xmm_ex(&em, 0, ins->arg, use_reg_slots);
            }
            uint8_t rv_reg = 1;
            if (ins->op == OP_LT_LOCAL_CONST_JMPF) {
                double cd = fn->consts[ins->arg2].as.num;
                int cidx = -1;
                if (cd != 0.0) JIT_CPOOL_IDX(cd, cidx);
                if (cd == 0.0) {
                    static const uint8_t z1[4] = { 0x66, 0x0F, 0x57, 0xC9 };
                    jemit_bytes(&em, z1, 4);
                } else if (cidx >= 0) {
                    /* movsd xmm1, [rip + disp32] */
                    static const uint8_t m1[4] = { 0xF2, 0x0F, 0x10, 0x0D };
                    jemit_bytes(&em, m1, 4);
                    jpush_reloc(&relocs, &reloc_len, &reloc_cap, em.len, (uint32_t)cidx, 4);
                    jemit_i32(&em, 0);
                } else {
                    jemit_const_xmm(&em, 1, cd);
                }
            } else if (use_reg_slots && ins->arg2 < 4u) {
                rv_reg = (uint8_t)(2u + ins->arg2);
            } else {
                jemit_load_slot_xmm_ex(&em, 1, ins->arg2, use_reg_slots);
            }
            uint8_t ck = (uint8_t)(ins->ic_slot & 3);
            jemit_u8(&em, 0x66);
            jemit_u8(&em, 0x0F);
            jemit_u8(&em, 0x2E);
            if (ck == 0 || ck == 1) {
                /* LT / LE: ucomisd rv_reg, lv_reg */
                jemit_u8(&em, (uint8_t)(0xC0u | ((rv_reg & 7u) << 3) | (lv_reg & 7u)));
            } else {
                /* GT / GE: ucomisd lv_reg, rv_reg */
                jemit_u8(&em, (uint8_t)(0xC0u | ((lv_reg & 7u) << 3) | (rv_reg & 7u)));
            }
            /* ja (0x77) for LT/GT, jae (0x73) for LE/GE — false on NaN (CF=1, ZF=1)! */
            jemit_u8(&em, (ck == 0 || ck == 2) ? 0x77 : 0x73);
            size_t jtrue_disp_pos = em.len;
            jemit_u8(&em, 0);
            /* False stub: push false (0.0) into slot sp, then jmp target */
            static const uint8_t z0[4] = { 0x66, 0x0F, 0x57, 0xC0 };
            jemit_bytes(&em, z0, 4);
            jemit_store_slot_xmm_ex(&em, sp, 0, use_reg_slots);
            jemit_u8(&em, 0xE9);
            jpush_reloc(&relocs, &reloc_len, &reloc_cap, em.len, (uint32_t)ins->ic_bcap, 0);
            jemit_i32(&em, 0);
            em.buf[jtrue_disp_pos] = (uint8_t)(em.len - (jtrue_disp_pos + 1u));
            for (uint32_t sk = 1; sk <= 4u; sk++) ip_off[ip + sk] = em.len;
            ip += 4u;
            continue;
        }
        if (ins->op == OP_INC_LOCAL_1 && ins->fused_skip == 3 && ip + 3u < n) {
            double cd = fn->consts[ins->arg2].as.num;
            int cidx = -1;
            JIT_CPOOL_IDX(cd, cidx);
            if (use_reg_slots && ins->arg < 4u && cidx >= 0) {
                /* Direct single-instruction `addsd/subsd xmm<2+slot>, [rip + disp32]`! */
                uint8_t dreg = (uint8_t)(2u + ins->arg);
                jemit_u8(&em, 0xF2);
                jemit_u8(&em, 0x0F);
                jemit_u8(&em, ins->ic_slot ? 0x5C : 0x58);
                jemit_u8(&em, (uint8_t)(((dreg & 7u) << 3) | 0x05u));
                jpush_reloc(&relocs, &reloc_len, &reloc_cap, em.len, (uint32_t)cidx, 4);
                jemit_i32(&em, 0);
            } else if (cidx >= 0) {
                jemit_load_slot_xmm_ex(&em, 0, ins->arg, use_reg_slots);
                jemit_u8(&em, 0xF2);
                jemit_u8(&em, 0x0F);
                jemit_u8(&em, ins->ic_slot ? 0x5C : 0x58);
                jemit_u8(&em, 0x05);
                jpush_reloc(&relocs, &reloc_len, &reloc_cap, em.len, (uint32_t)cidx, 4);
                jemit_i32(&em, 0);
                jemit_store_slot_xmm_ex(&em, ins->arg, 0, use_reg_slots);
            } else {
                jemit_load_slot_xmm_ex(&em, 0, ins->arg, use_reg_slots);
                jemit_const_xmm(&em, 1, cd);
                jemit_u8(&em, 0xF2);
                jemit_u8(&em, 0x0F);
                jemit_u8(&em, ins->ic_slot ? 0x5C : 0x58);
                jemit_u8(&em, 0xC1);
                jemit_store_slot_xmm_ex(&em, ins->arg, 0, use_reg_slots);
            }
            for (uint32_t sk = 1; sk <= 3u; sk++) ip_off[ip + sk] = em.len;
            ip += 3u;
            continue;
        }
        if (ins->op == OP_ADD_LOCAL_LOCAL && ins->fused_skip == 2 && ip + 2u < n) {
            /* Check if immediately followed by OP_SET_LOCAL dst (`s = s + i`)! */
            if (ip + 3u < n && visited[ip + 3u] && !is_jtarget[ip + 3u]) {
                uint8_t nop = fn->code[ip + 3u].orig_op ? fn->code[ip + 3u].orig_op : fn->code[ip + 3u].op;
                uint32_t dst = fn->code[ip + 3u].arg;
                if (nop == OP_SET_LOCAL && use_reg_slots && dst == ins->arg && dst < 4u && ins->arg2 < 4u) {
                    /* Single 4-byte instruction: `addsd/subsd xmm<2+dst>, xmm<2+arg2>`! */
                    uint8_t dreg = (uint8_t)(2u + dst);
                    uint8_t sreg = (uint8_t)(2u + ins->arg2);
                    jemit_u8(&em, 0xF2);
                    jemit_u8(&em, 0x0F);
                    jemit_u8(&em, ins->ic_slot ? 0x5C : 0x58);
                    jemit_u8(&em, (uint8_t)(0xC0u | ((dreg & 7u) << 3) | (sreg & 7u)));
                    for (uint32_t sk = 1; sk <= 3u; sk++) ip_off[ip + sk] = em.len;
                    ip += 3u;
                    continue;
                }
            }
            jemit_load_slot_xmm_ex(&em, 0, ins->arg, use_reg_slots);
            jemit_load_slot_xmm_ex(&em, 1, ins->arg2, use_reg_slots);
            jemit_u8(&em, 0xF2);
            jemit_u8(&em, 0x0F);
            jemit_u8(&em, ins->ic_slot ? 0x5C : 0x58);
            jemit_u8(&em, 0xC1);
            jemit_store_slot_xmm_ex(&em, sp, 0, use_reg_slots);
            for (uint32_t sk = 1; sk <= 2u; sk++) ip_off[ip + sk] = em.len;
            ip += 2u;
            continue;
        }
        if (ins->op == OP_SUB_LOCAL_CONST && ins->fused_skip == 2 && ip + 2u < n) {
            jemit_load_slot_xmm_ex(&em, 0, ins->arg, use_reg_slots);
            double cd = fn->consts[ins->arg2].as.num;
            int cidx = -1;
            JIT_CPOOL_IDX(cd, cidx);
            if (cidx >= 0) {
                jemit_u8(&em, 0xF2);
                jemit_u8(&em, 0x0F);
                jemit_u8(&em, ins->ic_slot ? 0x5C : 0x58);
                jemit_u8(&em, 0x05);
                jpush_reloc(&relocs, &reloc_len, &reloc_cap, em.len, (uint32_t)cidx, 4);
                jemit_i32(&em, 0);
            } else {
                jemit_const_xmm(&em, 1, cd);
                jemit_u8(&em, 0xF2);
                jemit_u8(&em, 0x0F);
                jemit_u8(&em, ins->ic_slot ? 0x5C : 0x58);
                jemit_u8(&em, 0xC1);
            }
            /* Fast-path for 1-arg self-recursive call (`fib(n - 1)`): xmm0 already holds arg0! */
            if (!use_reg_slots && !has_div && fn->arity == 1u && sp >= 1u &&
                ip + 3u < n && visited[ip + 3u] && !is_jtarget[ip + 3u]) {
                uint8_t cop = fn->code[ip + 3u].orig_op ? fn->code[ip + 3u].orig_op : fn->code[ip + 3u].op;
                if (cop == OP_CALL && fn->code[ip + 3u].arg == 1u) {
                    jemit_u8(&em, 0xE8);
                    jpush_reloc(&relocs, &reloc_len, &reloc_cap, em.len, 0, 1);
                    jemit_i32(&em, 0);
                    jemit_store_slot_xmm_ex(&em, sp - 1u, 0, false);
                    for (uint32_t sk = 1; sk <= 3u; sk++) ip_off[ip + sk] = em.len;
                    ip += 3u;
                    continue;
                }
            }
            jemit_store_slot_xmm_ex(&em, sp, 0, use_reg_slots);
            for (uint32_t sk = 1; sk <= 2u; sk++) ip_off[ip + sk] = em.len;
            ip += 2u;
            continue;
        }

        uint8_t op = ins->orig_op ? ins->orig_op : ins->op;
        if (op == 0 && ins->op == OP_CONSTANT) op = OP_CONSTANT;

        switch (op) {
            case OP_CONSTANT: {
                PbValue cv = fn->consts[ins->arg];
                double d = (cv.tag == VAL_BOOL) ? (cv.as.b ? 1.0 : 0.0) : cv.as.num;
                int cidx = -1;
                if (d != 0.0) JIT_CPOOL_IDX(d, cidx);
                if (d == 0.0) {
                    jemit_const_xmm(&em, 0, 0.0);
                } else if (cidx >= 0) {
                    static const uint8_t m0[4] = { 0xF2, 0x0F, 0x10, 0x05 };
                    jemit_bytes(&em, m0, 4);
                    jpush_reloc(&relocs, &reloc_len, &reloc_cap, em.len, (uint32_t)cidx, 4);
                    jemit_i32(&em, 0);
                } else {
                    jemit_const_xmm(&em, 0, d);
                }
                jemit_store_slot_xmm_ex(&em, sp, 0, use_reg_slots);
                break;
            }
            case OP_POP:
            case OP_GET_GLOBAL:
            case OP_GET_UPVALUE:
                break;
            case OP_DUP: {
                jemit_load_slot_xmm_ex(&em, 0, sp - 1, use_reg_slots);
                jemit_store_slot_xmm_ex(&em, sp, 0, use_reg_slots);
                break;
            }
            case OP_GET_LOCAL: {
                jemit_load_slot_xmm_ex(&em, 0, ins->arg, use_reg_slots);
                jemit_store_slot_xmm_ex(&em, sp, 0, use_reg_slots);
                break;
            }
            case OP_SET_LOCAL: {
                jemit_load_slot_xmm_ex(&em, 0, sp - 1, use_reg_slots);
                jemit_store_slot_xmm_ex(&em, ins->arg, 0, use_reg_slots);
                break;
            }
            case OP_ADD:
            case OP_SUB:
            case OP_MUL:
            case OP_DIV: {
                jemit_load_slot_xmm_ex(&em, 1, sp - 1, use_reg_slots);
                if (op == OP_DIV) {
                    /* xorpd xmm0, xmm0 ; ucomisd xmm1, xmm0 ; jp +6 ; je bailout */
                    static const uint8_t zchk[10] = {
                        0x66, 0x0F, 0x57, 0xC0,
                        0x66, 0x0F, 0x2E, 0xC8,
                        0x7A, 0x06
                    };
                    jemit_bytes(&em, zchk, 10);
                    jemit_u8(&em, 0x0F);
                    jemit_u8(&em, 0x84);
                    jpush_reloc(&relocs, &reloc_len, &reloc_cap, em.len, 0, 3);
                    jemit_i32(&em, 0);
                }
                jemit_load_slot_xmm_ex(&em, 0, sp - 2, use_reg_slots);
                jemit_u8(&em, 0xF2);
                jemit_u8(&em, 0x0F);
                jemit_u8(&em, (op == OP_ADD) ? 0x58 :
                              (op == OP_SUB) ? 0x5C :
                              (op == OP_MUL) ? 0x59 : 0x5E);
                jemit_u8(&em, 0xC1);
                jemit_store_slot_xmm_ex(&em, sp - 2, 0, use_reg_slots);
                break;
            }
            case OP_NEGATE: {
                jemit_load_slot_xmm_ex(&em, 0, sp - 1, use_reg_slots);
                jemit_u8(&em, 0x48);
                jemit_u8(&em, 0xB8);
                jemit_u64(&em, 0x8000000000000000ULL);
                static const uint8_t neg_tail[9] = {
                    0x66, 0x48, 0x0F, 0x6E, 0xC8,
                    0x66, 0x0F, 0x57, 0xC1
                };
                jemit_bytes(&em, neg_tail, 9);
                jemit_store_slot_xmm_ex(&em, sp - 1, 0, use_reg_slots);
                break;
            }
            case OP_NOT: {
                jemit_load_slot_xmm_ex(&em, 0, sp - 1, use_reg_slots);
                static const uint8_t not_seq[26] = {
                    0x31, 0xC0,                   /* xor eax, eax */
                    0x66, 0x0F, 0x57, 0xC9,       /* xorpd xmm1, xmm1 */
                    0x66, 0x0F, 0x2E, 0xC1,       /* ucomisd xmm0, xmm1 */
                    0x0F, 0x94, 0xC0,             /* sete al */
                    0x0F, 0x9B, 0xC1,             /* setnp cl */
                    0x21, 0xC8,                   /* and eax, ecx */
                    0x66, 0x0F, 0x57, 0xC0,       /* xorpd xmm0, xmm0 */
                    0xF2, 0x0F, 0x2A, 0xC0        /* cvtsi2sd xmm0, eax */
                };
                jemit_bytes(&em, not_seq, sizeof(not_seq));
                jemit_store_slot_xmm_ex(&em, sp - 1, 0, use_reg_slots);
                break;
            }
            case OP_LT:
            case OP_LE:
            case OP_GT:
            case OP_GE: {
                jemit_load_slot_xmm_ex(&em, 0, sp - 2, use_reg_slots);
                jemit_load_slot_xmm_ex(&em, 1, sp - 1, use_reg_slots);
                jemit_u8(&em, 0x31); jemit_u8(&em, 0xC0); /* xor eax, eax */
                if (op == OP_LT || op == OP_LE) {
                    static const uint8_t uc10[4] = { 0x66, 0x0F, 0x2E, 0xC8 };
                    jemit_bytes(&em, uc10, 4);
                } else {
                    static const uint8_t uc01[4] = { 0x66, 0x0F, 0x2E, 0xC1 };
                    jemit_bytes(&em, uc01, 4);
                }
                jemit_u8(&em, 0x0F);
                jemit_u8(&em, (op == OP_LT || op == OP_GT) ? 0x97 : 0x93);
                jemit_u8(&em, 0xC0);
                static const uint8_t cvt[8] = {
                    0x66, 0x0F, 0x57, 0xC0,
                    0xF2, 0x0F, 0x2A, 0xC0
                };
                jemit_bytes(&em, cvt, 8);
                jemit_store_slot_xmm_ex(&em, sp - 2, 0, use_reg_slots);
                break;
            }
            case OP_EQ:
            case OP_NOT_EQ: {
                jemit_load_slot_xmm_ex(&em, 0, sp - 2, use_reg_slots);
                jemit_load_slot_xmm_ex(&em, 1, sp - 1, use_reg_slots);
                if (op == OP_EQ) {
                    static const uint8_t eq_seq[22] = {
                        0x31, 0xC0,
                        0x66, 0x0F, 0x2E, 0xC1,
                        0x0F, 0x94, 0xC0,
                        0x0F, 0x9B, 0xC1,
                        0x21, 0xC8,
                        0x66, 0x0F, 0x57, 0xC0,
                        0xF2, 0x0F, 0x2A, 0xC0
                    };
                    jemit_bytes(&em, eq_seq, sizeof(eq_seq));
                } else {
                    static const uint8_t ne_seq[22] = {
                        0x31, 0xC0,
                        0x66, 0x0F, 0x2E, 0xC1,
                        0x0F, 0x95, 0xC0,
                        0x0F, 0x9A, 0xC1,
                        0x09, 0xC8,
                        0x66, 0x0F, 0x57, 0xC0,
                        0xF2, 0x0F, 0x2A, 0xC0
                    };
                    jemit_bytes(&em, ne_seq, sizeof(ne_seq));
                }
                jemit_store_slot_xmm_ex(&em, sp - 2, 0, use_reg_slots);
                break;
            }
            case OP_JUMP: {
                jemit_u8(&em, 0xE9);
                jpush_reloc(&relocs, &reloc_len, &reloc_cap, em.len, ins->arg, 0);
                jemit_i32(&em, 0);
                break;
            }
            case OP_JUMP_IF_FALSE: {
                jemit_load_slot_xmm_ex(&em, 0, sp - 1, use_reg_slots);
                static const uint8_t jf_pre[10] = {
                    0x66, 0x0F, 0x57, 0xC9,
                    0x66, 0x0F, 0x2E, 0xC1,
                    0x7A, 0x06
                };
                jemit_bytes(&em, jf_pre, 10);
                jemit_u8(&em, 0x0F);
                jemit_u8(&em, 0x84);
                jpush_reloc(&relocs, &reloc_len, &reloc_cap, em.len, ins->arg, 0);
                jemit_i32(&em, 0);
                break;
            }
            case OP_JUMP_IF_TRUE: {
                jemit_load_slot_xmm_ex(&em, 0, sp - 1, use_reg_slots);
                static const uint8_t jt_pre[8] = {
                    0x66, 0x0F, 0x57, 0xC9,
                    0x66, 0x0F, 0x2E, 0xC1
                };
                jemit_bytes(&em, jt_pre, 8);
                jemit_u8(&em, 0x0F);
                jemit_u8(&em, 0x8A);
                jpush_reloc(&relocs, &reloc_len, &reloc_cap, em.len, ins->arg, 0);
                jemit_i32(&em, 0);
                jemit_u8(&em, 0x0F);
                jemit_u8(&em, 0x85);
                jpush_reloc(&relocs, &reloc_len, &reloc_cap, em.len, ins->arg, 0);
                jemit_i32(&em, 0);
                break;
            }
            case OP_CALL:
            case OP_TAIL_CALL: {
                uint32_t cargc = ins->arg;
                uint32_t cslot = sp - 1 - cargc;
                for (uint32_t p = 0; p < cargc; p++) {
                    if (p < 6) {
                        jemit_load_slot_xmm_ex(&em, (uint8_t)p, cslot + 1 + p, false);
                    } else if (p == 6) {
                        jemit_u8(&em, 0x48);
                        jemit_u8(&em, 0x8B);
                        jemit_u8(&em, 0x85);
                        jemit_i32(&em, jslot_disp(cslot + 1 + p));
                    } else {
                        jemit_u8(&em, 0x48);
                        jemit_u8(&em, 0x8B);
                        jemit_u8(&em, 0x95);
                        jemit_i32(&em, jslot_disp(cslot + 1 + p));
                    }
                }
                if (op == OP_TAIL_CALL) {
                    jemit_u8(&em, 0xE9);
                    jpush_reloc(&relocs, &reloc_len, &reloc_cap, em.len, 0, 2);
                    jemit_i32(&em, 0);
                } else {
                    jemit_u8(&em, 0xE8);
                    jpush_reloc(&relocs, &reloc_len, &reloc_cap, em.len, 0, 1);
                    jemit_i32(&em, 0);
                    if (has_div) {
                        static const uint8_t chk_bail[4] = { 0x41, 0x83, 0x3A, 0x00 };
                        jemit_bytes(&em, chk_bail, 4);
                        jemit_u8(&em, 0x0F);
                        jemit_u8(&em, 0x85);
                        jpush_reloc(&relocs, &reloc_len, &reloc_cap, em.len, 0, 3);
                        jemit_i32(&em, 0);
                    }
                    jemit_store_slot_xmm_ex(&em, cslot, 0, false);
                }
                break;
            }
            case OP_RETURN: {
                jemit_load_slot_xmm_ex(&em, 0, sp - 1, use_reg_slots);
                if (frame_bytes == 0) {
                    static const uint8_t epil0[2] = { 0x5D, 0xC3 };
                    jemit_bytes(&em, epil0, 2);
                } else {
                    static const uint8_t epil[5] = { 0x48, 0x89, 0xEC, 0x5D, 0xC3 };
                    jemit_bytes(&em, epil, 5);
                }
                break;
            }
            default:
                break;
        }
    }
    ip_off[n] = em.len;
#undef JIT_CPOOL_IDX

    /* 4. Bailout handler (division by zero -> fallback to interpreter) */
    size_t bailout_off = em.len;
    {
        static const uint8_t bail_seq[16] = {
            0x41, 0xC7, 0x02, 0x01, 0x00, 0x00, 0x00,
            0x66, 0x0F, 0x57, 0xC0,
            0x48, 0x89, 0xEC, 0x5D, 0xC3
        };
        jemit_bytes(&em, bail_seq, sizeof(bail_seq));
    }

    /* 5. 8-byte aligned RIP-relative IEEE-754 double constant pool */
    while ((em.len & 7u) != 0) jemit_u8(&em, 0xCC);
    size_t cpool_off = em.len;
    for (uint32_t ci = 0; ci < cpool_len; ci++) {
        jemit_u64(&em, cpool[ci]);
    }

    /* 6. Patch all relative 32-bit displacements */
    for (size_t r = 0; r < reloc_len; r++) {
        size_t pos = relocs[r].disp_pos;
        size_t tgt = 0;
        if (relocs[r].kind == 0) tgt = ip_off[relocs[r].target_ip];
        else if (relocs[r].kind == 1) tgt = jit_body_off;
        else if (relocs[r].kind == 2) tgt = tail_loop_off;
        else if (relocs[r].kind == 3) tgt = bailout_off;
        else tgt = cpool_off + (size_t)relocs[r].target_ip * 8u;
        int64_t rel = (int64_t)tgt - (int64_t)(pos + 4u);
        jpatch_i32(em.buf, pos, (int32_t)rel);
    }

    size_t alloc_sz = 0;
    void *exec_mem = zr_jit_alloc_wx(em.buf, em.len, &alloc_sz);
    size_t emitted_bytes = em.len;

    free(em.buf);
    free(relocs);
    free(ip_off);
    free(is_jtarget);
    free(visited);
    free(in_work);
    free(sp_at);
    free(tags_at);
    free(work);

    if (!exec_mem) {
        return vm_error(vm, "jit_compile: W^X page allocation failed");
    }

    fn->jit_code = exec_mem;
    fn->jit_code_bytes = alloc_sz;
    fn->jit_ret_bool = (ret_tag == JTAG_BOOL);
    fn->jit_self_kind = self_kind;
    fn->jit_self_arg = self_arg;
    /* ISO C99 safe object-to-function pointer cast via memcpy */
    memcpy(&fn->jit_fn, &exec_mem, sizeof(fn->jit_fn));

    vm->jit_compiled_functions++;
    vm->jit_total_code_bytes += emitted_bytes;
    return true;
}

#else /* Non-x86_64 or WASI portable interpreter fallback */

#define ZR_JIT_ARCH_AVAILABLE 0
#if defined(__aarch64__) || defined(_M_ARM64)
#define ZR_JIT_ARCH_NAME "aarch64"
#elif defined(__wasi__) || defined(__wasm__)
#define ZR_JIT_ARCH_NAME "wasm32"
#elif defined(__i386__) || defined(_M_IX86)
#define ZR_JIT_ARCH_NAME "x86"
#else
#define ZR_JIT_ARCH_NAME "portable-c99"
#endif

static void zr_jit_free_code(PbFunc *fn) {
    if (!fn) return;
    fn->jit_code = NULL;
    fn->jit_code_bytes = 0;
    fn->jit_fn = NULL;
}

static bool zr_jit_compile_func(PbVM *vm, PbFunc *fn) {
    if (fn) {
        fn->jit_attempted = true;
        if (!fn->peephole_done) zr_peephole_optimize_func(fn);
    }
    return vm_error(vm, "jit_compile: native JIT not available on target %s", ZR_JIT_ARCH_NAME);
}

#endif /* x86_64 W^X JIT */

/* ============================================================================
 * 3. JIT Introspection & Explicit Compile Builtins
 * ============================================================================ */

static bool bi_jit_compile(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 1) return vm_error(vm, "jit_compile(fn) expects 1 function argument");
    PbFunc *fn = NULL;
    if (is_obj_type(argv[0], OBJ_CLOSURE)) fn = AS_CLOSURE(argv[0])->fn;
    else if (is_obj_type(argv[0], OBJ_FUNC)) fn = AS_FUNC(argv[0]);
    else return vm_error(vm, "jit_compile(fn) expects function or closure, got %s", zr_val_type_name(argv[0]));

    char saved_err[2048];
    memcpy(saved_err, vm->err_msg, sizeof(saved_err));
    size_t before_bytes = vm->jit_total_code_bytes;
    bool already = (fn->jit_fn != NULL);
    bool compiled = already ? true : zr_jit_compile_func(vm, fn);
    char reason[256];
    reason[0] = '\0';
    if (!compiled) {
        snprintf(reason, sizeof(reason), "%.250s", vm->err_msg);
        memcpy(vm->err_msg, saved_err, sizeof(saved_err));
    }

    PbMap *res = pb_map_new_raw(vm, 6);
    vm_push_root(vm, val_obj(res));
    pb_map_set_cstr(vm, res, "ok", val_bool(compiled));
    pb_map_set_cstr(vm, res, "jit_compiled", val_bool(fn->jit_fn != NULL));
    size_t cbytes = already ? fn->jit_code_bytes : (compiled ? (vm->jit_total_code_bytes - before_bytes) : 0);
    pb_map_set_cstr(vm, res, "code_bytes", val_num((double)cbytes));
    pb_map_set_cstr(vm, res, "arch", val_obj(pb_str_c(vm, ZR_JIT_ARCH_NAME)));
    if (!compiled && reason[0] != '\0') {
        pb_map_set_cstr(vm, res, "reason", val_obj(pb_str_c(vm, reason)));
    }
    vm_pop_roots(vm, 1);
    *out = val_obj(res);
    return true;
}

static bool bi_jit_stats(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    (void)argc;
    (void)argv;
    PbMap *res = pb_map_new_raw(vm, 6);
    vm_push_root(vm, val_obj(res));
    pb_map_set_cstr(vm, res, "jit_available", val_bool(ZR_JIT_ARCH_AVAILABLE != 0));
    pb_map_set_cstr(vm, res, "arch", val_obj(pb_str_c(vm, ZR_JIT_ARCH_NAME)));
    pb_map_set_cstr(vm, res, "compiled_functions", val_num((double)vm->jit_compiled_functions));
    pb_map_set_cstr(vm, res, "total_code_bytes", val_num((double)vm->jit_total_code_bytes));
    pb_map_set_cstr(vm, res, "jit_invocations", val_num((double)vm->jit_invocations));
    vm_pop_roots(vm, 1);
    *out = val_obj(res);
    return true;
}

#if defined(__GNUC__) && !defined(__clang__)
#pragma GCC pop_options
#endif

#endif /* PBLANG_ZR_JIT_X64_H */
