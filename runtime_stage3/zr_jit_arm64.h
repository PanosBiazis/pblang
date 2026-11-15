/* ============================================================================
 * zr_jit_arm64.h — Sovereign Zero-Rust Runtime v4.2.0
 * Native ARMv8-A (AArch64) W^X Tier-2 JIT Compiler & Cross-Arch Verifier
 *
 * Strictly portable C99:
 *   - Platform-independent ARMv8-A A64 machine-code emitter (`zr_jit_emit_arm64_words`)
 *     available on ALL hosts (x86_64, aarch64, Linux, macOS, Windows).
 *   - Native W^X page allocator + execution (`zr_jit_compile_func` / `zr_jit_free_code`)
 *     active on `#if (defined(__aarch64__) || defined(_M_ARM64)) && !defined(__wasi__)`
 *     with Apple Silicon `MAP_JIT` + `pthread_jit_write_protect_np`, Linux `mmap`/`mprotect`,
 *     and Windows ARM64 `VirtualAlloc`/`VirtualProtect` + `FlushInstructionCache`.
 *   - Deterministic bounded ARMv8-A A64 machine-code subset emulator (`zr_arm64_emulate_words`)
 *     enabling bit-exact IEEE-754 verification of emitted ARM64 machine words on x86_64 CI.
 * ============================================================================ */
#ifndef PBLANG_ZR_JIT_ARM64_H
#define PBLANG_ZR_JIT_ARM64_H

#include "zr_vm_core.h"

#ifdef __APPLE__
#if defined(__aarch64__) || defined(_M_ARM64)
#include <pthread.h>
#include <libkern/OSCacheControl.h>
#endif
#endif

enum {
    ZR_A64_TAG_NONE = 0,
    ZR_A64_TAG_NUM  = 1,
    ZR_A64_TAG_BOOL = 2,
    ZR_A64_TAG_SELF = 3
};

#define ZR_A64_MAX_SLOTS 64u

typedef struct {
    uint32_t *words;
    size_t    len;
    size_t    cap;
} ZrArm64Emit;

typedef struct {
    size_t   word_idx;
    uint32_t target_ip;
    uint8_t  kind;     /* 0 = bytecode ip, 1 = jit_body, 2 = tail_loop, 3 = bailout */
    uint8_t  enc_type; /* 0 = B/BL imm26, 1 = B.cond/CBNZ imm19 */
} ZrArm64Reloc;

static void zr_a64_emit_word(ZrArm64Emit *e, uint32_t w) {
    if (e->len >= e->cap) {
        e->cap = e->cap ? (e->cap * 2u) : 128u;
        e->words = (uint32_t *)zr_xrealloc(e->words, e->cap * sizeof(uint32_t));
    }
    e->words[e->len++] = w;
}

static void zr_a64_push_reloc(ZrArm64Reloc **arr, size_t *len, size_t *cap,
                              size_t word_idx, uint32_t target_ip, uint8_t kind, uint8_t enc_type) {
    if (*len >= *cap) {
        *cap = *cap ? (*cap * 2u) : 32u;
        *arr = (ZrArm64Reloc *)zr_xrealloc(*arr, (*cap) * sizeof(ZrArm64Reloc));
    }
    (*arr)[*len].word_idx = word_idx;
    (*arr)[*len].target_ip = target_ip;
    (*arr)[*len].kind = kind;
    (*arr)[*len].enc_type = enc_type;
    (*len)++;
}

/* LDR Dt, [SP, #(slot * 8)] */
static inline void zr_a64_emit_ldr_d_sp(ZrArm64Emit *e, uint8_t dreg, uint32_t slot) {
    zr_a64_emit_word(e, 0xFD4003E0u | ((slot & 0xFFFu) << 10) | ((uint32_t)dreg & 31u));
}

/* STR Dt, [SP, #(slot * 8)] */
static inline void zr_a64_emit_str_d_sp(ZrArm64Emit *e, uint8_t dreg, uint32_t slot) {
    zr_a64_emit_word(e, 0xFD0003E0u | ((slot & 0xFFFu) << 10) | ((uint32_t)dreg & 31u));
}

/* Load 64-bit IEEE-754 constant into D<dreg> via XZR or MOVZ/MOVK X9 + FMOV D<dreg>, X9 */
static void zr_a64_emit_const_f64(ZrArm64Emit *e, uint8_t dreg, double d) {
    uint64_t bits = 0;
    memcpy(&bits, &d, sizeof(bits));
    if (bits == 0ULL) {
        /* FMOV Dd, XZR */
        zr_a64_emit_word(e, 0x9E6703E0u | ((uint32_t)dreg & 31u));
        return;
    }
    bool first = true;
    for (uint32_t hw = 0; hw < 4u; hw++) {
        uint32_t imm16 = (uint32_t)((bits >> (hw * 16u)) & 0xFFFFu);
        if (imm16 != 0u || (hw == 3u && first)) {
            if (first) {
                /* MOVZ X9, #imm16, LSL #(hw*16) */
                zr_a64_emit_word(e, 0xD2800009u | (hw << 21) | (imm16 << 5));
                first = false;
            } else {
                /* MOVK X9, #imm16, LSL #(hw*16) */
                zr_a64_emit_word(e, 0xF2800009u | (hw << 21) | (imm16 << 5));
            }
        }
    }
    /* FMOV Dd, X9 */
    zr_a64_emit_word(e, 0x9E670120u | ((uint32_t)dreg & 31u));
}

typedef struct {
    uint32_t *words;
    size_t    word_count;
    bool      ret_bool;
    int8_t    self_kind;
    uint32_t  self_arg;
    bool      has_prologue_stp;
    bool      has_epilogue_ret;
} ZrArm64CompileResult;

static bool zr_jit_emit_arm64_words(PbVM *vm, PbFunc *fn, ZrArm64CompileResult *out_res) {
    memset(out_res, 0, sizeof(*out_res));
    if (!fn) return vm_error(vm, "jit_compile_arm64: null function");
    if (!fn->peephole_done) zr_peephole_optimize_func(fn);
    if (fn->is_vararg || fn->is_async || fn->arity > 8 || fn->code_len == 0 || fn->code_len > 4096) {
        return vm_error(vm, "jit_compile_arm64: function '%s' exceeds arity/async/size limits",
                        fn->name ? fn->name->chars : "<fn>");
    }

    uint32_t n = fn->code_len;
    uint8_t *visited = (uint8_t *)zr_xcalloc(n, 1);
    uint8_t *in_work = (uint8_t *)zr_xcalloc(n, 1);
    uint32_t *sp_at = (uint32_t *)zr_xcalloc(n, sizeof(uint32_t));
    uint8_t (*tags_at)[ZR_A64_MAX_SLOTS] =
        (uint8_t (*)[ZR_A64_MAX_SLOTS])zr_xcalloc(n, ZR_A64_MAX_SLOTS);
    uint32_t *work = (uint32_t *)zr_xmalloc((size_t)n * sizeof(uint32_t));
    size_t wlen = 0;

    visited[0] = 1;
    in_work[0] = 1;
    sp_at[0] = fn->arity;
    for (uint32_t p = 0; p < fn->arity; p++) tags_at[0][p] = ZR_A64_TAG_NUM;
    work[wlen++] = 0;

    uint32_t max_slots = fn->arity;
    int8_t self_kind = 0;
    uint32_t self_arg = 0;
    uint8_t ret_tag = ZR_A64_TAG_NONE;
    bool ok = true;

#define A64_ENQUEUE(tgt_ip, next_sp, cur_tags) do { \
    uint32_t _t = (tgt_ip); \
    uint32_t _nsp = (next_sp); \
    if (_t >= n || _nsp > ZR_A64_MAX_SLOTS) { ok = false; break; } \
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
                if ((tags_at[_t][_k] == ZR_A64_TAG_NUM || tags_at[_t][_k] == ZR_A64_TAG_BOOL) && \
                    ((cur_tags)[_k] == ZR_A64_TAG_NUM || (cur_tags)[_k] == ZR_A64_TAG_BOOL)) { \
                    if (tags_at[_t][_k] != ZR_A64_TAG_NUM) { tags_at[_t][_k] = ZR_A64_TAG_NUM; _changed = true; } \
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
        uint8_t tcur[ZR_A64_MAX_SLOTS];
        if (sp > 0) memcpy(tcur, tags_at[ip], sp);
        if (sp > max_slots) max_slots = sp;

        const PbInstr *ins = &fn->code[ip];
        uint8_t op = ins->orig_op ? ins->orig_op : ins->op;
        if (op == 0 && ins->op == OP_CONSTANT) op = OP_CONSTANT;

        switch (op) {
            case OP_CONSTANT: {
                if (ins->arg >= fn->const_len || sp >= ZR_A64_MAX_SLOTS) { ok = false; break; }
                PbValue cv = fn->consts[ins->arg];
                if (cv.tag == VAL_NUM && cv.taint == 0) tcur[sp++] = ZR_A64_TAG_NUM;
                else if (cv.tag == VAL_BOOL && cv.taint == 0) tcur[sp++] = ZR_A64_TAG_BOOL;
                else { ok = false; break; }
                if (sp > max_slots) max_slots = sp;
                A64_ENQUEUE(ip + 1u, sp, tcur);
                break;
            }
            case OP_POP: {
                if (sp < 1u) { ok = false; break; }
                sp--;
                A64_ENQUEUE(ip + 1u, sp, tcur);
                break;
            }
            case OP_DUP: {
                if (sp < 1u || sp >= ZR_A64_MAX_SLOTS) { ok = false; break; }
                tcur[sp] = tcur[sp - 1u];
                sp++;
                if (sp > max_slots) max_slots = sp;
                A64_ENQUEUE(ip + 1u, sp, tcur);
                break;
            }
            case OP_GET_LOCAL: {
                if (ins->arg >= sp || sp >= ZR_A64_MAX_SLOTS) { ok = false; break; }
                uint8_t lt = tcur[ins->arg];
                if (lt != ZR_A64_TAG_NUM && lt != ZR_A64_TAG_BOOL) { ok = false; break; }
                tcur[sp++] = lt;
                if (sp > max_slots) max_slots = sp;
                A64_ENQUEUE(ip + 1u, sp, tcur);
                break;
            }
            case OP_SET_LOCAL: {
                if (sp < 1u) { ok = false; break; }
                uint8_t vt = tcur[--sp];
                if (ins->arg >= sp || (vt != ZR_A64_TAG_NUM && vt != ZR_A64_TAG_BOOL)) { ok = false; break; }
                tcur[ins->arg] = vt;
                A64_ENQUEUE(ip + 1u, sp, tcur);
                break;
            }
            case OP_ADD:
            case OP_SUB:
            case OP_MUL:
            case OP_DIV: {
                if (sp < 2u || tcur[sp - 2u] != ZR_A64_TAG_NUM || tcur[sp - 1u] != ZR_A64_TAG_NUM) { ok = false; break; }
                sp--;
                tcur[sp - 1u] = ZR_A64_TAG_NUM;
                A64_ENQUEUE(ip + 1u, sp, tcur);
                break;
            }
            case OP_NEGATE: {
                if (sp < 1u || tcur[sp - 1u] != ZR_A64_TAG_NUM) { ok = false; break; }
                A64_ENQUEUE(ip + 1u, sp, tcur);
                break;
            }
            case OP_NOT: {
                if (sp < 1u || (tcur[sp - 1u] != ZR_A64_TAG_NUM && tcur[sp - 1u] != ZR_A64_TAG_BOOL)) { ok = false; break; }
                tcur[sp - 1u] = ZR_A64_TAG_BOOL;
                A64_ENQUEUE(ip + 1u, sp, tcur);
                break;
            }
            case OP_EQ:
            case OP_NOT_EQ: {
                if (sp < 2u) { ok = false; break; }
                uint8_t ta = tcur[sp - 2u], tb = tcur[sp - 1u];
                if ((ta != ZR_A64_TAG_NUM && ta != ZR_A64_TAG_BOOL) || ta != tb) { ok = false; break; }
                sp--;
                tcur[sp - 1u] = ZR_A64_TAG_BOOL;
                A64_ENQUEUE(ip + 1u, sp, tcur);
                break;
            }
            case OP_LT:
            case OP_LE:
            case OP_GT:
            case OP_GE: {
                if (sp < 2u || tcur[sp - 2u] != ZR_A64_TAG_NUM || tcur[sp - 1u] != ZR_A64_TAG_NUM) { ok = false; break; }
                sp--;
                tcur[sp - 1u] = ZR_A64_TAG_BOOL;
                A64_ENQUEUE(ip + 1u, sp, tcur);
                break;
            }
            case OP_JUMP: {
                A64_ENQUEUE(ins->arg, sp, tcur);
                break;
            }
            case OP_JUMP_IF_FALSE:
            case OP_JUMP_IF_TRUE: {
                if (sp < 1u || (tcur[sp - 1u] != ZR_A64_TAG_NUM && tcur[sp - 1u] != ZR_A64_TAG_BOOL)) { ok = false; break; }
                A64_ENQUEUE(ip + 1u, sp, tcur);
                if (ok) A64_ENQUEUE(ins->arg, sp, tcur);
                break;
            }
            case OP_GET_GLOBAL: {
                if (!fn->name || ins->arg >= fn->const_len || !is_obj_type(fn->consts[ins->arg], OBJ_STR) ||
                    strcmp(AS_STR(fn->consts[ins->arg])->chars, fn->name->chars) != 0 ||
                    sp >= ZR_A64_MAX_SLOTS || (self_kind != 0 && (self_kind != 1 || self_arg != ins->arg))) {
                    ok = false;
                    break;
                }
                self_kind = 1;
                self_arg = ins->arg;
                tcur[sp++] = ZR_A64_TAG_SELF;
                if (sp > max_slots) max_slots = sp;
                A64_ENQUEUE(ip + 1u, sp, tcur);
                break;
            }
            case OP_GET_UPVALUE: {
                if (ins->arg != 0u || sp >= ZR_A64_MAX_SLOTS || (self_kind != 0 && self_kind != 2)) {
                    ok = false;
                    break;
                }
                self_kind = 2;
                self_arg = 0u;
                tcur[sp++] = ZR_A64_TAG_SELF;
                if (sp > max_slots) max_slots = sp;
                A64_ENQUEUE(ip + 1u, sp, tcur);
                break;
            }
            case OP_CALL:
            case OP_TAIL_CALL: {
                uint32_t cargc = ins->arg;
                if (cargc != fn->arity || sp < cargc + 1u) { ok = false; break; }
                uint32_t cslot = sp - 1u - cargc;
                if (tcur[cslot] != ZR_A64_TAG_SELF) { ok = false; break; }
                for (uint32_t a = 0; a < cargc; a++) {
                    if (tcur[cslot + 1u + a] != ZR_A64_TAG_NUM) { ok = false; break; }
                }
                if (!ok) break;
                if (ret_tag == ZR_A64_TAG_BOOL) { ok = false; break; }
                ret_tag = ZR_A64_TAG_NUM;
                sp = cslot + 1u;
                tcur[cslot] = ZR_A64_TAG_NUM;
                A64_ENQUEUE(ip + 1u, sp, tcur);
                break;
            }
            case OP_RETURN: {
                if (sp < 1u) { ok = false; break; }
                uint8_t rt = tcur[sp - 1u];
                if (rt != ZR_A64_TAG_NUM && rt != ZR_A64_TAG_BOOL) { ok = false; break; }
                if (ret_tag == ZR_A64_TAG_NONE) ret_tag = rt;
                else if (ret_tag != rt) { ok = false; break; }
                break;
            }
            default:
                ok = false;
                break;
        }
    }
#undef A64_ENQUEUE

    if (!ok || ret_tag == ZR_A64_TAG_NONE) {
        free(visited);
        free(in_work);
        free(sp_at);
        free(tags_at);
        free(work);
        return vm_error(vm, "jit_compile_arm64: function '%s' uses non-numeric/unsupported opcodes",
                        fn->name ? fn->name->chars : "<fn>");
    }

    bool has_div = false;
    for (uint32_t k = 0; k < n; k++) {
        if (visited[k]) {
            uint8_t kop = fn->code[k].orig_op ? fn->code[k].orig_op : fn->code[k].op;
            if (kop == OP_DIV) has_div = true;
        }
    }

    ZrArm64Emit em = { NULL, 0, 0 };
    ZrArm64Reloc *relocs = NULL;
    size_t reloc_len = 0, reloc_cap = 0;
    size_t *ip_word = (size_t *)zr_xcalloc((size_t)n + 1u, sizeof(size_t));

    /* 1. AAPCS64 entry stub: X0 = const double *args, X1 = int *bailout
     *    MOV X8, X0 ; MOV X11, X1 ; LDR D0..D(arity-1), [X8, #p*8] */
    zr_a64_emit_word(&em, 0xAA0003E8u); /* MOV X8, X0  */
    zr_a64_emit_word(&em, 0xAA0103EBu); /* MOV X11, X1 */
    for (uint32_t p = 0; p < fn->arity; p++) {
        /* LDR D<p>, [X8, #(p * 8)] */
        zr_a64_emit_word(&em, 0xFD400100u | (p << 10) | p);
    }

    /* 2. Internal recursive entry `jit_body` */
    size_t jit_body_word = em.len;
    uint32_t frame_slots = (max_slots + 1u) & ~1u;
    if (frame_slots == 0u) frame_slots = 2u;
    uint32_t frame_bytes = frame_slots * 8u;

    zr_a64_emit_word(&em, 0xA9BF7BFDu); /* STP X29, X30, [SP, #-16]! */
    zr_a64_emit_word(&em, 0x910003FDu); /* MOV X29, SP               */
    zr_a64_emit_word(&em, 0xD10003FFu | ((frame_bytes & 0xFFFu) << 10)); /* SUB SP, SP, #frame_bytes */
    out_res->has_prologue_stp = true;

    /* 3. Tail-call target: store incoming D0..D(arity-1) into stack slots */
    size_t tail_loop_word = em.len;
    for (uint32_t p = 0; p < fn->arity; p++) {
        zr_a64_emit_str_d_sp(&em, (uint8_t)p, p);
    }

    /* 4. Emit A64 instructions per reachable bytecode instruction */
    for (uint32_t ip = 0; ip < n; ip++) {
        ip_word[ip] = em.len;
        if (!visited[ip]) continue;

        uint32_t sp = sp_at[ip];
        const PbInstr *ins = &fn->code[ip];
        uint8_t op = ins->orig_op ? ins->orig_op : ins->op;
        if (op == 0 && ins->op == OP_CONSTANT) op = OP_CONSTANT;

        switch (op) {
            case OP_CONSTANT: {
                PbValue cv = fn->consts[ins->arg];
                double d = (cv.tag == VAL_BOOL) ? (cv.as.b ? 1.0 : 0.0) : cv.as.num;
                zr_a64_emit_const_f64(&em, 0, d);
                zr_a64_emit_str_d_sp(&em, 0, sp);
                break;
            }
            case OP_POP:
            case OP_GET_GLOBAL:
            case OP_GET_UPVALUE:
                break;
            case OP_DUP:
                zr_a64_emit_ldr_d_sp(&em, 0, sp - 1u);
                zr_a64_emit_str_d_sp(&em, 0, sp);
                break;
            case OP_GET_LOCAL:
                zr_a64_emit_ldr_d_sp(&em, 0, ins->arg);
                zr_a64_emit_str_d_sp(&em, 0, sp);
                break;
            case OP_SET_LOCAL:
                zr_a64_emit_ldr_d_sp(&em, 0, sp - 1u);
                zr_a64_emit_str_d_sp(&em, 0, ins->arg);
                break;
            case OP_ADD:
            case OP_SUB:
            case OP_MUL:
            case OP_DIV: {
                zr_a64_emit_ldr_d_sp(&em, 1, sp - 1u);
                if (op == OP_DIV) {
                    /* FCMP D1, #0.0 ; B.EQ bailout */
                    zr_a64_emit_word(&em, 0x1E602028u);
                    zr_a64_push_reloc(&relocs, &reloc_len, &reloc_cap, em.len, 0u, 3u, 1u);
                    zr_a64_emit_word(&em, 0x54000000u | 0x0u); /* B.EQ */
                }
                zr_a64_emit_ldr_d_sp(&em, 0, sp - 2u);
                if (op == OP_ADD)      zr_a64_emit_word(&em, 0x1E612800u); /* FADD D0, D0, D1 */
                else if (op == OP_SUB) zr_a64_emit_word(&em, 0x1E613800u); /* FSUB D0, D0, D1 */
                else if (op == OP_MUL) zr_a64_emit_word(&em, 0x1E610800u); /* FMUL D0, D0, D1 */
                else                   zr_a64_emit_word(&em, 0x1E611800u); /* FDIV D0, D0, D1 */
                zr_a64_emit_str_d_sp(&em, 0, sp - 2u);
                break;
            }
            case OP_NEGATE:
                zr_a64_emit_ldr_d_sp(&em, 0, sp - 1u);
                zr_a64_emit_word(&em, 0x1E614000u); /* FNEG D0, D0 */
                zr_a64_emit_str_d_sp(&em, 0, sp - 1u);
                break;
            case OP_NOT:
                zr_a64_emit_ldr_d_sp(&em, 0, sp - 1u);
                zr_a64_emit_word(&em, 0x1E602008u); /* FCMP D0, #0.0 */
                zr_a64_emit_word(&em, 0x9A9F07E9u | (0x1u << 12)); /* CSET X9, EQ (invert=NE=1) */
                zr_a64_emit_word(&em, 0x9E630120u); /* UCVTF D0, X9 */
                zr_a64_emit_str_d_sp(&em, 0, sp - 1u);
                break;
            case OP_EQ:
            case OP_NOT_EQ:
            case OP_LT:
            case OP_LE:
            case OP_GT:
            case OP_GE: {
                zr_a64_emit_ldr_d_sp(&em, 0, sp - 2u);
                zr_a64_emit_ldr_d_sp(&em, 1, sp - 1u);
                zr_a64_emit_word(&em, 0x1E612000u); /* FCMP D0, D1 */
                /* IEEE-754 ARM64 condition codes:
                 * EQ=0x0, NE=0x1, MI(<)=0x4, LS(<=)=0x9, GT(>)=0xC, GE(>=)=0xA */
                uint32_t cond = (op == OP_EQ)     ? 0x0u :
                                (op == OP_NOT_EQ) ? 0x1u :
                                (op == OP_LT)     ? 0x4u :
                                (op == OP_LE)     ? 0x9u :
                                (op == OP_GT)     ? 0xCu : 0xAu;
                zr_a64_emit_word(&em, 0x9A9F07E9u | ((cond ^ 1u) << 12)); /* CSET X9, cond */
                zr_a64_emit_word(&em, 0x9E630120u); /* UCVTF D0, X9 */
                zr_a64_emit_str_d_sp(&em, 0, sp - 2u);
                break;
            }
            case OP_JUMP:
                zr_a64_push_reloc(&relocs, &reloc_len, &reloc_cap, em.len, ins->arg, 0u, 0u);
                zr_a64_emit_word(&em, 0x14000000u); /* B imm26 */
                break;
            case OP_JUMP_IF_FALSE:
                zr_a64_emit_ldr_d_sp(&em, 0, sp - 1u);
                zr_a64_emit_word(&em, 0x1E602008u); /* FCMP D0, #0.0 */
                zr_a64_push_reloc(&relocs, &reloc_len, &reloc_cap, em.len, ins->arg, 0u, 1u);
                zr_a64_emit_word(&em, 0x54000000u | 0x0u); /* B.EQ target */
                break;
            case OP_JUMP_IF_TRUE:
                zr_a64_emit_ldr_d_sp(&em, 0, sp - 1u);
                zr_a64_emit_word(&em, 0x1E602008u); /* FCMP D0, #0.0 */
                zr_a64_push_reloc(&relocs, &reloc_len, &reloc_cap, em.len, ins->arg, 0u, 1u);
                zr_a64_emit_word(&em, 0x54000000u | 0x1u); /* B.NE target */
                break;
            case OP_CALL:
            case OP_TAIL_CALL: {
                uint32_t cargc = ins->arg;
                uint32_t cslot = sp - 1u - cargc;
                for (uint32_t p = 0; p < cargc; p++) {
                    zr_a64_emit_ldr_d_sp(&em, (uint8_t)p, cslot + 1u + p);
                }
                if (op == OP_TAIL_CALL) {
                    zr_a64_push_reloc(&relocs, &reloc_len, &reloc_cap, em.len, 0u, 2u, 0u);
                    zr_a64_emit_word(&em, 0x14000000u); /* B tail_loop */
                } else {
                    zr_a64_push_reloc(&relocs, &reloc_len, &reloc_cap, em.len, 0u, 1u, 0u);
                    zr_a64_emit_word(&em, 0x94000000u); /* BL jit_body */
                    if (has_div) {
                        zr_a64_emit_word(&em, 0xB9400169u); /* LDR W9, [X11] */
                        zr_a64_push_reloc(&relocs, &reloc_len, &reloc_cap, em.len, 0u, 3u, 1u);
                        zr_a64_emit_word(&em, 0x35000009u); /* CBNZ W9, bailout */
                    }
                    zr_a64_emit_str_d_sp(&em, 0, cslot);
                }
                break;
            }
            case OP_RETURN:
                zr_a64_emit_ldr_d_sp(&em, 0, sp - 1u);
                zr_a64_emit_word(&em, 0x910003FFu | ((frame_bytes & 0xFFFu) << 10)); /* ADD SP, SP, #frame_bytes */
                zr_a64_emit_word(&em, 0xA8C17BFDu); /* LDP X29, X30, [SP], #16 */
                zr_a64_emit_word(&em, 0xD65F03C0u); /* RET */
                out_res->has_epilogue_ret = true;
                break;
            default:
                break;
        }
    }
    ip_word[n] = em.len;

    /* 5. Bailout epilogue (division by zero -> *bailout = 1, return 0.0) */
    size_t bailout_word = em.len;
    zr_a64_emit_word(&em, 0x52800029u); /* MOV W9, #1 */
    zr_a64_emit_word(&em, 0xB9000169u); /* STR W9, [X11] */
    zr_a64_emit_word(&em, 0x9E6703E0u); /* FMOV D0, XZR */
    zr_a64_emit_word(&em, 0x910003FFu | ((frame_bytes & 0xFFFu) << 10)); /* ADD SP, SP, #frame_bytes */
    zr_a64_emit_word(&em, 0xA8C17BFDu); /* LDP X29, X30, [SP], #16 */
    zr_a64_emit_word(&em, 0xD65F03C0u); /* RET */

    /* 6. Patch PC-relative word offsets */
    bool patch_ok = true;
    for (size_t r = 0; r < reloc_len; r++) {
        size_t pos = relocs[r].word_idx;
        size_t tgt = 0;
        if (relocs[r].kind == 0u) tgt = ip_word[relocs[r].target_ip];
        else if (relocs[r].kind == 1u) tgt = jit_body_word;
        else if (relocs[r].kind == 2u) tgt = tail_loop_word;
        else tgt = bailout_word;

        int64_t diff = (int64_t)tgt - (int64_t)pos;
        if (relocs[r].enc_type == 0u) {
            if (diff < -(1 << 25) || diff >= (1 << 25)) { patch_ok = false; break; }
            em.words[pos] = (em.words[pos] & 0xFC000000u) | ((uint32_t)diff & 0x03FFFFFFu);
        } else {
            if (diff < -(1 << 18) || diff >= (1 << 18)) { patch_ok = false; break; }
            em.words[pos] = (em.words[pos] & 0xFF00001Fu) | (((uint32_t)diff & 0x7FFFFu) << 5);
        }
    }

    free(relocs);
    free(ip_word);
    free(visited);
    free(in_work);
    free(sp_at);
    free(tags_at);
    free(work);

    if (!patch_ok) {
        free(em.words);
        return vm_error(vm, "jit_compile_arm64: branch displacement out of A64 range");
    }

    out_res->words = em.words;
    out_res->word_count = em.len;
    out_res->ret_bool = (ret_tag == ZR_A64_TAG_BOOL);
    out_res->self_kind = self_kind;
    out_res->self_arg = self_arg;
    return true;
}

/* ============================================================================
 * Deterministic ARMv8-A A64 FP64/Branch Machine-Word Emulator (All Hosts)
 * ============================================================================ */

static inline bool zr_a64_eval_cond(uint32_t cond, uint8_t nzcv) {
    bool n = ((nzcv >> 3) & 1u) != 0;
    bool z = ((nzcv >> 2) & 1u) != 0;
    bool c = ((nzcv >> 1) & 1u) != 0;
    bool v = (nzcv & 1u) != 0;
    bool base = false;
    switch ((cond >> 1) & 7u) {
        case 0: base = z; break;                 /* EQ / NE */
        case 1: base = c; break;                 /* CS / CC */
        case 2: base = n; break;                 /* MI / PL */
        case 3: base = v; break;                 /* VS / VC */
        case 4: base = (c && !z); break;         /* HI / LS */
        case 5: base = (n == v); break;          /* GE / LT */
        case 6: base = (!z && (n == v)); break;  /* GT / LE */
        default: base = true; break;             /* AL      */
    }
    if ((cond & 1u) && cond != 0xFu) base = !base;
    return base;
}

static inline uint8_t zr_a64_fcmp_flags(double a, double b) {
    if (isnan(a) || isnan(b)) return 0x3u; /* N=0, Z=0, C=1, V=1 (unordered) */
    if (a == b) return 0x6u;               /* N=0, Z=1, C=1, V=0 */
    if (a < b)  return 0x8u;               /* N=1, Z=0, C=0, V=0 */
    return 0x2u;                           /* N=0, Z=0, C=1, V=0 (a > b) */
}

static bool zr_arm64_emulate_words(PbVM *vm, const uint32_t *words, size_t nwords,
                                   const double *args, size_t argc,
                                   double *out_val, int *out_bailout) {
    if (!words || nwords == 0) return vm_error(vm, "jit_emulate_arm64: empty instruction buffer");
    enum { A64_STACK_BYTES = 262144u, A64_ARGS_BASE = 262144u, A64_BAIL_OFF = 262272u, A64_MEM_TOTAL = 262400u };
    uint8_t *mem = (uint8_t *)zr_xcalloc(A64_MEM_TOTAL, 1);
    for (size_t i = 0; i < argc && i < 8u; i++) {
        memcpy(mem + A64_ARGS_BASE + i * 8u, &args[i], sizeof(double));
    }

    uint64_t x[31];
    double   d[32];
    memset(x, 0, sizeof(x));
    memset(d, 0, sizeof(d));
    uint64_t sp = A64_STACK_BYTES;
    x[0] = A64_ARGS_BASE;
    x[1] = A64_BAIL_OFF;
    x[30] = UINT64_MAX; /* sentinel return address */

    uint8_t nzcv = 0;
    size_t pc = 0;
    uint64_t steps = 0;
    const uint64_t max_steps = 50000000ULL;

    while (pc != (size_t)UINT64_MAX) {
        if (pc >= nwords) {
            free(mem);
            return vm_error(vm, "jit_emulate_arm64: PC %zu out of bounds (%zu words)", pc, nwords);
        }
        if (++steps > max_steps) {
            free(mem);
            return vm_error(vm, "jit_emulate_arm64: exceeded step budget");
        }
        uint32_t w = words[pc];

        if (w == 0xD65F03C0u) { /* RET */
            if (x[30] == UINT64_MAX) break;
            pc = (size_t)x[30];
            continue;
        } else if (w == 0xA9BF7BFDu) { /* STP X29, X30, [SP, #-16]! */
            if (sp < 16u) { free(mem); return vm_error(vm, "jit_emulate_arm64: stack overflow"); }
            sp -= 16u;
            memcpy(mem + sp, &x[29], 8u);
            memcpy(mem + sp + 8u, &x[30], 8u);
        } else if (w == 0xA8C17BFDu) { /* LDP X29, X30, [SP], #16 */
            if (sp + 16u > A64_STACK_BYTES) { free(mem); return vm_error(vm, "jit_emulate_arm64: stack underflow"); }
            memcpy(&x[29], mem + sp, 8u);
            memcpy(&x[30], mem + sp + 8u, 8u);
            sp += 16u;
        } else if (w == 0x910003FDu) { /* MOV X29, SP */
            x[29] = sp;
        } else if ((w & 0xFFE0FFE0u) == 0xAA0003E0u) { /* MOV Xd, Xm */
            uint32_t rd = w & 31u, rm = (w >> 16) & 31u;
            if (rd < 31u) x[rd] = (rm < 31u) ? x[rm] : 0ULL;
        } else if ((w & 0xFFC003FFu) == 0xD10003FFu) { /* SUB SP, SP, #imm12 */
            uint64_t imm = (w >> 10) & 0xFFFu;
            if (sp < imm) { free(mem); return vm_error(vm, "jit_emulate_arm64: stack frame overflow"); }
            sp -= imm;
        } else if ((w & 0xFFC003FFu) == 0x910003FFu) { /* ADD SP, SP, #imm12 */
            uint64_t imm = (w >> 10) & 0xFFFu;
            if (sp + imm > A64_STACK_BYTES) { free(mem); return vm_error(vm, "jit_emulate_arm64: stack frame underflow"); }
            sp += imm;
        } else if ((w & 0xFFC00000u) == 0xFD400000u) { /* LDR Dt, [Xn|SP, #(uimm12*8)] */
            uint32_t rt = w & 31u, rn = (w >> 5) & 31u;
            uint64_t base = (rn == 31u) ? sp : x[rn];
            uint64_t addr = base + (uint64_t)((w >> 10) & 0xFFFu) * 8ULL;
            if (addr + 8u > A64_MEM_TOTAL) { free(mem); return vm_error(vm, "jit_emulate_arm64: OOB LDR D"); }
            memcpy(&d[rt], mem + addr, 8u);
        } else if ((w & 0xFFC00000u) == 0xFD000000u) { /* STR Dt, [Xn|SP, #(uimm12*8)] */
            uint32_t rt = w & 31u, rn = (w >> 5) & 31u;
            uint64_t base = (rn == 31u) ? sp : x[rn];
            uint64_t addr = base + (uint64_t)((w >> 10) & 0xFFFu) * 8ULL;
            if (addr + 8u > A64_MEM_TOTAL) { free(mem); return vm_error(vm, "jit_emulate_arm64: OOB STR D"); }
            memcpy(mem + addr, &d[rt], 8u);
        } else if ((w & 0xFF800000u) == 0xD2800000u) { /* MOVZ Xd, #imm16, LSL #(hw*16) */
            uint32_t rd = w & 31u, hw = (w >> 21) & 3u, imm16 = (w >> 5) & 0xFFFFu;
            if (rd < 31u) x[rd] = ((uint64_t)imm16) << (hw * 16u);
        } else if ((w & 0xFF800000u) == 0xF2800000u) { /* MOVK Xd, #imm16, LSL #(hw*16) */
            uint32_t rd = w & 31u, hw = (w >> 21) & 3u, imm16 = (w >> 5) & 0xFFFFu;
            if (rd < 31u) {
                uint64_t mask = 0xFFFFULL << (hw * 16u);
                x[rd] = (x[rd] & ~mask) | (((uint64_t)imm16) << (hw * 16u));
            }
        } else if ((w & 0xFF800000u) == 0x52800000u) { /* MOVZ Wd, #imm16 */
            uint32_t rd = w & 31u, hw = (w >> 21) & 1u, imm16 = (w >> 5) & 0xFFFFu;
            if (rd < 31u) x[rd] = (uint64_t)(((uint32_t)imm16) << (hw * 16u));
        } else if ((w & 0xFFFFFC00u) == 0x9E670000u) { /* FMOV Dd, Xn */
            uint32_t rd = w & 31u, rn = (w >> 5) & 31u;
            uint64_t bits = (rn == 31u) ? 0ULL : x[rn];
            memcpy(&d[rd], &bits, 8u);
        } else if ((w & 0xFFFFFC00u) == 0x9E630000u) { /* UCVTF Dd, Xn */
            uint32_t rd = w & 31u, rn = (w >> 5) & 31u;
            uint64_t v = (rn == 31u) ? 0ULL : x[rn];
            d[rd] = (double)v;
        } else if ((w & 0xFFE0FC00u) == 0x1E602800u) { /* FADD Dd, Dn, Dm */
            d[w & 31u] = d[(w >> 5) & 31u] + d[(w >> 16) & 31u];
        } else if ((w & 0xFFE0FC00u) == 0x1E603800u) { /* FSUB Dd, Dn, Dm */
            d[w & 31u] = d[(w >> 5) & 31u] - d[(w >> 16) & 31u];
        } else if ((w & 0xFFE0FC00u) == 0x1E600800u) { /* FMUL Dd, Dn, Dm */
            d[w & 31u] = d[(w >> 5) & 31u] * d[(w >> 16) & 31u];
        } else if ((w & 0xFFE0FC00u) == 0x1E601800u) { /* FDIV Dd, Dn, Dm */
            d[w & 31u] = d[(w >> 5) & 31u] / d[(w >> 16) & 31u];
        } else if ((w & 0xFFFFFC00u) == 0x1E614000u) { /* FNEG Dd, Dn */
            d[w & 31u] = -d[(w >> 5) & 31u];
        } else if ((w & 0xFFE0FC1Fu) == 0x1E602000u) { /* FCMP Dn, Dm */
            nzcv = zr_a64_fcmp_flags(d[(w >> 5) & 31u], d[(w >> 16) & 31u]);
        } else if ((w & 0xFFFFFC1Fu) == 0x1E602008u) { /* FCMP Dn, #0.0 */
            nzcv = zr_a64_fcmp_flags(d[(w >> 5) & 31u], 0.0);
        } else if ((w & 0xFFFF0FE0u) == 0x9A9F07E0u) { /* CSET Xd, cond (CSINC Xd, XZR, XZR, inv_cond) */
            uint32_t rd = w & 31u;
            uint32_t inv_cond = (w >> 12) & 0xFu;
            if (rd < 31u) x[rd] = zr_a64_eval_cond(inv_cond ^ 1u, nzcv) ? 1ULL : 0ULL;
        } else if ((w & 0xFC000000u) == 0x14000000u) { /* B imm26 */
            int32_t imm26 = (int32_t)(w & 0x03FFFFFFu);
            if (imm26 & (1 << 25)) imm26 |= (int32_t)0xFC000000u;
            pc = (size_t)((int64_t)pc + (int64_t)imm26);
            continue;
        } else if ((w & 0xFC000000u) == 0x94000000u) { /* BL imm26 */
            int32_t imm26 = (int32_t)(w & 0x03FFFFFFu);
            if (imm26 & (1 << 25)) imm26 |= (int32_t)0xFC000000u;
            x[30] = (uint64_t)(pc + 1u);
            pc = (size_t)((int64_t)pc + (int64_t)imm26);
            continue;
        } else if ((w & 0xFF000010u) == 0x54000000u) { /* B.cond imm19 */
            uint32_t cond = w & 0xFu;
            if (zr_a64_eval_cond(cond, nzcv)) {
                int32_t imm19 = (int32_t)((w >> 5) & 0x7FFFFu);
                if (imm19 & (1 << 18)) imm19 |= (int32_t)0xFFF80000u;
                pc = (size_t)((int64_t)pc + (int64_t)imm19);
                continue;
            }
        } else if ((w & 0xFF000000u) == 0x35000000u) { /* CBNZ Wt, imm19 */
            uint32_t rt = w & 31u;
            uint32_t wv = (rt < 31u) ? (uint32_t)x[rt] : 0u;
            if (wv != 0u) {
                int32_t imm19 = (int32_t)((w >> 5) & 0x7FFFFu);
                if (imm19 & (1 << 18)) imm19 |= (int32_t)0xFFF80000u;
                pc = (size_t)((int64_t)pc + (int64_t)imm19);
                continue;
            }
        } else if ((w & 0xFFC00000u) == 0xB9000000u) { /* STR Wt, [Xn, #(uimm12*4)] */
            uint32_t rt = w & 31u, rn = (w >> 5) & 31u;
            uint64_t base = (rn == 31u) ? sp : x[rn];
            uint64_t addr = base + (uint64_t)((w >> 10) & 0xFFFu) * 4ULL;
            if (addr + 4u > A64_MEM_TOTAL) { free(mem); return vm_error(vm, "jit_emulate_arm64: OOB STR W"); }
            uint32_t wv = (rt < 31u) ? (uint32_t)x[rt] : 0u;
            memcpy(mem + addr, &wv, 4u);
        } else if ((w & 0xFFC00000u) == 0xB9400000u) { /* LDR Wt, [Xn, #(uimm12*4)] */
            uint32_t rt = w & 31u, rn = (w >> 5) & 31u;
            uint64_t base = (rn == 31u) ? sp : x[rn];
            uint64_t addr = base + (uint64_t)((w >> 10) & 0xFFFu) * 4ULL;
            if (addr + 4u > A64_MEM_TOTAL) { free(mem); return vm_error(vm, "jit_emulate_arm64: OOB LDR W"); }
            uint32_t wv = 0u;
            memcpy(&wv, mem + addr, 4u);
            if (rt < 31u) x[rt] = (uint64_t)wv;
        } else {
            free(mem);
            return vm_error(vm, "jit_emulate_arm64: unsupported A64 opcode 0x%08X at word %zu", w, pc);
        }
        pc++;
    }

    int32_t bail = 0;
    memcpy(&bail, mem + A64_BAIL_OFF, 4u);
    free(mem);
    if (out_bailout) *out_bailout = (int)bail;
    if (out_val) *out_val = d[0];
    return true;
}

/* ============================================================================
 * Native W^X Execution on AArch64 Hosts
 * ============================================================================ */
#if (defined(__aarch64__) || defined(_M_ARM64)) && !defined(__wasi__)

static void *zr_jit_alloc_wx_arm64(const uint32_t *words, size_t word_count, size_t *out_alloc_sz) {
    size_t len = word_count * sizeof(uint32_t);
#ifdef _WIN32
    SYSTEM_INFO si;
    GetSystemInfo(&si);
    size_t gran = si.dwPageSize ? (size_t)si.dwPageSize : 4096u;
    size_t sz = (len + gran - 1u) & ~(gran - 1u);
    if (sz == 0u) sz = gran;
    void *mem = VirtualAlloc(NULL, sz, MEM_COMMIT | MEM_RESERVE, PAGE_READWRITE);
    if (!mem) return NULL;
    memcpy(mem, words, len);
    DWORD old_prot = 0;
    if (!VirtualProtect(mem, sz, PAGE_EXECUTE_READ, &old_prot)) {
        VirtualFree(mem, 0, MEM_RELEASE);
        return NULL;
    }
    FlushInstructionCache(GetCurrentProcess(), mem, len);
    *out_alloc_sz = sz;
    return mem;
#elif defined(__APPLE__)
    long ps = sysconf(_SC_PAGESIZE);
    size_t page_sz = (ps > 0) ? (size_t)ps : 16384u;
    size_t sz = (len + page_sz - 1u) & ~(page_sz - 1u);
    if (sz == 0u) sz = page_sz;
#ifndef MAP_JIT
#define MAP_JIT 0x0800
#endif
    void *mem = mmap(NULL, sz, PROT_READ | PROT_WRITE | PROT_EXEC,
                     MAP_PRIVATE | MAP_ANON | MAP_JIT, -1, 0);
    if (mem == MAP_FAILED || !mem) return NULL;
    pthread_jit_write_protect_np(0);
    memcpy(mem, words, len);
    pthread_jit_write_protect_np(1);
    sys_icache_invalidate(mem, len);
    *out_alloc_sz = sz;
    return mem;
#else
    long ps = sysconf(_SC_PAGESIZE);
    size_t page_sz = (ps > 0) ? (size_t)ps : 4096u;
    size_t sz = (len + page_sz - 1u) & ~(page_sz - 1u);
    if (sz == 0u) sz = page_sz;
#ifndef MAP_ANONYMOUS
#ifdef MAP_ANON
#define MAP_ANONYMOUS MAP_ANON
#else
#define MAP_ANONYMOUS 0x20
#endif
#endif
    void *mem = mmap(NULL, sz, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
    if (mem == MAP_FAILED || !mem) return NULL;
    memcpy(mem, words, len);
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

    ZrArm64CompileResult cres;
    if (!zr_jit_emit_arm64_words(vm, fn, &cres)) {
        return false;
    }
    size_t alloc_sz = 0;
    size_t emitted_bytes = cres.word_count * sizeof(uint32_t);
    void *exec_mem = zr_jit_alloc_wx_arm64(cres.words, cres.word_count, &alloc_sz);
    free(cres.words);
    if (!exec_mem) {
        return vm_error(vm, "jit_compile: ARM64 W^X page allocation failed");
    }

    fn->jit_code = exec_mem;
    fn->jit_code_bytes = alloc_sz;
    fn->jit_ret_bool = cres.ret_bool;
    fn->jit_self_kind = cres.self_kind;
    fn->jit_self_arg = cres.self_arg;
    memcpy(&fn->jit_fn, &exec_mem, sizeof(fn->jit_fn));

    vm->jit_compiled_functions++;
    vm->jit_total_code_bytes += emitted_bytes;
    return true;
}

#endif /* AArch64 Native W^X Execution */

/* ============================================================================
 * Cross-Architecture ARM64 JIT Inspection & Emulation Builtins (All Hosts)
 * ============================================================================ */

static bool bi_jit_compile_arm64_bytes(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 1) return vm_error(vm, "jit_compile_arm64_bytes(fn) expects 1 function argument");
    PbFunc *fn = NULL;
    if (is_obj_type(argv[0], OBJ_CLOSURE)) fn = AS_CLOSURE(argv[0])->fn;
    else if (is_obj_type(argv[0], OBJ_FUNC)) fn = AS_FUNC(argv[0]);
    else return vm_error(vm, "jit_compile_arm64_bytes(fn) expects function or closure, got %s", zr_val_type_name(argv[0]));

    char saved_err[2048];
    memcpy(saved_err, vm->err_msg, sizeof(saved_err));

    ZrArm64CompileResult cres;
    bool ok = zr_jit_emit_arm64_words(vm, fn, &cres);
    char reason[256];
    reason[0] = '\0';
    if (!ok) {
        snprintf(reason, sizeof(reason), "%.250s", vm->err_msg);
        memcpy(vm->err_msg, saved_err, sizeof(saved_err));
    }

    PbMap *res = pb_map_new_raw(vm, 12);
    vm_push_root(vm, val_obj(res));
    pb_map_set_cstr(vm, res, "ok", val_bool(ok));
    pb_map_set_cstr(vm, res, "arch", val_obj(pb_str_c(vm, "arm64")));

    if (ok) {
        size_t nbytes = cres.word_count * 4u;
        uint8_t *le_bytes = (uint8_t *)zr_xmalloc(nbytes ? nbytes : 1u);
        for (size_t i = 0; i < cres.word_count; i++) {
            uint32_t w = cres.words[i];
            le_bytes[i * 4u + 0u] = (uint8_t)(w & 0xFFu);
            le_bytes[i * 4u + 1u] = (uint8_t)((w >> 8) & 0xFFu);
            le_bytes[i * 4u + 2u] = (uint8_t)((w >> 16) & 0xFFu);
            le_bytes[i * 4u + 3u] = (uint8_t)((w >> 24) & 0xFFu);
        }
        PbBytes *bobj = pb_bytes_new(vm, nbytes, false);
        if (nbytes > 0) memcpy(bobj->data, le_bytes, nbytes);
        free(le_bytes);
        vm_push_root(vm, val_obj(bobj));

        PbArray *warr = pb_array_new(vm, cres.word_count);
        vm_push_root(vm, val_obj(warr));
        for (size_t i = 0; i < cres.word_count; i++) {
            pb_array_push(vm, warr, val_num((double)cres.words[i]));
        }

        pb_map_set_cstr(vm, res, "byte_len", val_num((double)nbytes));
        pb_map_set_cstr(vm, res, "insn_count", val_num((double)cres.word_count));
        pb_map_set_cstr(vm, res, "bytes", val_obj(bobj));
        pb_map_set_cstr(vm, res, "words", val_obj(warr));
        pb_map_set_cstr(vm, res, "ret_bool", val_bool(cres.ret_bool));
        pb_map_set_cstr(vm, res, "prologue_stp_x29_x30", val_bool(cres.has_prologue_stp));
        pb_map_set_cstr(vm, res, "epilogue_ret", val_bool(cres.has_epilogue_ret));
        pb_map_set_cstr(vm, res, "reason", val_obj(pb_str_c(vm, "")));

        vm_pop_roots(vm, 2);
        free(cres.words);
    } else {
        pb_map_set_cstr(vm, res, "byte_len", val_num(0.0));
        pb_map_set_cstr(vm, res, "insn_count", val_num(0.0));
        pb_map_set_cstr(vm, res, "prologue_stp_x29_x30", val_bool(false));
        pb_map_set_cstr(vm, res, "epilogue_ret", val_bool(false));
        pb_map_set_cstr(vm, res, "reason", val_obj(pb_str_c(vm, reason[0] != '\0' ? reason : "unsupported bytecode")));
    }

    vm_pop_roots(vm, 1);
    *out = val_obj(res);
    return true;
}

static bool bi_jit_emulate_arm64(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc < 1) {
        return vm_error(vm, "jit_emulate_arm64(fn_or_compiled_or_bytes[, args...]) expects at least 1 argument");
    }

    double args[8] = {0};
    size_t n_args = 0;
    if (argc == 2 && is_obj_type(argv[1], OBJ_ARRAY)) {
        PbArray *arr = AS_ARRAY(argv[1]);
        if (arr->len > 8u) {
            return vm_error(vm, "jit_emulate_arm64: maximum 8 arguments supported");
        }
        n_args = arr->len;
        for (size_t i = 0; i < n_args; i++) {
            PbValue av = arr->items[i];
            if (av.tag == VAL_NUM) args[i] = av.as.num;
            else if (av.tag == VAL_BOOL) args[i] = av.as.b ? 1.0 : 0.0;
            else return vm_error(vm, "jit_emulate_arm64: argument %zu must be number or bool", i);
        }
    } else if (argc == 2 && argv[1].tag == VAL_NIL) {
        n_args = 0;
    } else if (argc > 1) {
        if ((size_t)(argc - 1) > 8u) {
            return vm_error(vm, "jit_emulate_arm64: maximum 8 arguments supported");
        }
        n_args = (size_t)(argc - 1);
        for (size_t i = 0; i < n_args; i++) {
            PbValue av = argv[i + 1];
            if (av.tag == VAL_NUM) args[i] = av.as.num;
            else if (av.tag == VAL_BOOL) args[i] = av.as.b ? 1.0 : 0.0;
            else return vm_error(vm, "jit_emulate_arm64: argument %zu must be number or bool", i);
        }
    }

    PbValue bval = argv[0];
    bool ret_bool = false;
    uint32_t *words = NULL;
    size_t nwords = 0;

    if (is_obj_type(bval, OBJ_CLOSURE) || is_obj_type(bval, OBJ_FUNC)) {
        PbFunc *fn = is_obj_type(bval, OBJ_CLOSURE) ? AS_CLOSURE(bval)->fn : AS_FUNC(bval);
        ZrArm64CompileResult cres;
        if (!zr_jit_emit_arm64_words(vm, fn, &cres)) return false;
        words = cres.words;
        nwords = cres.word_count;
        ret_bool = cres.ret_bool;
    } else {
        if (is_obj_type(bval, OBJ_MAP)) {
            PbValue rb;
            if (pb_map_get_cstr(AS_MAP(bval), "ret_bool", &rb) && rb.tag == VAL_BOOL) ret_bool = rb.as.b;
            if (!pb_map_get_cstr(AS_MAP(bval), "bytes", &bval)) {
                return vm_error(vm, "jit_emulate_arm64: map argument has no 'bytes' field");
            }
        }
        if (!is_obj_type(bval, OBJ_BYTES)) {
            return vm_error(vm, "jit_emulate_arm64 expects fn, bytes, or compiled map as arg 1, got %s", zr_val_type_name(argv[0]));
        }
        PbBytes *bobj = AS_BYTES(bval);
        if (bobj->len == 0 || (bobj->len & 3u) != 0u) {
            return vm_error(vm, "jit_emulate_arm64: machine code byte length (%zu) must be a non-zero multiple of 4", bobj->len);
        }
        nwords = bobj->len / 4u;
        words = (uint32_t *)zr_xmalloc(nwords * sizeof(uint32_t));
        for (size_t i = 0; i < nwords; i++) {
            words[i] = (uint32_t)bobj->data[i * 4u + 0u] |
                       ((uint32_t)bobj->data[i * 4u + 1u] << 8) |
                       ((uint32_t)bobj->data[i * 4u + 2u] << 16) |
                       ((uint32_t)bobj->data[i * 4u + 3u] << 24);
        }
    }

    double res_num = 0.0;
    int bailout = 0;
    bool ok = zr_arm64_emulate_words(vm, words, nwords, args, n_args, &res_num, &bailout);
    free(words);
    if (!ok) return false;
    if (bailout != 0) {
        return vm_error(vm, "division by zero in ARM64 JIT code");
    }
    *out = ret_bool ? val_bool(res_num != 0.0) : val_num(res_num);
    return true;
}

#endif /* PBLANG_ZR_JIT_ARM64_H */
