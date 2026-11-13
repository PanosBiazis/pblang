/* ============================================================================
 * pblang_micro_vm.c — Sovereign Freestanding 8-bit / 16-bit / 32-bit Micro-VM
 * Version: 2.6.0
 *
 * Zero-malloc, zero-FPU, in-place `.pbck` v1 bytecode interpreter engineered
 * for ultra-constrained 8-bit, 16-bit, and 32-bit microcontrollers & legacy OSes:
 *   - 8-bit  Atmel AVR ATmega328P (Arduino Uno/Nano: 32 KB Flash, 2 KB SRAM)
 *   - 16-bit x86-16 / MSP430 / PIC24 (static arena < 2 KB RAM)
 *   - 32-bit ARM Cortex-M0/M3/M4, RISC-V RV32IM, ESP32, i386 Linux/Win32/DOS32
 *
 * Capabilities:
 *   - Parses genuine `PBCK\x01` bytecode emitted by `pblang build` / `pbc`
 *   - IEEE-754 binary64 -> signed 32-bit integer decoding in pure integer C
 *     (requires ZERO hardware floating-point unit; works when sizeof(double)==4)
 *   - Executes arithmetic (+, -, *, /, %, unary -), bitwise/comparisons,
 *     boolean logic, strings (zero-copy slices into .pbck image), globals,
 *     locals, jumps, functions, closures (stateless/global), recursion,
 *     `print`, and core builtins (`abs`, `min`, `max`, `len`, `str`).
 * ============================================================================ */

#include <stdint.h>
#include <stddef.h>
#include <stdbool.h>

#define PBMICRO_VERSION_STR "pblang-micro-vm 2.6.0 (8/16/32-bit freestanding PBCK v1 engine)"

#if defined(__AVR__)
#include <avr/io.h>
#define PBMICRO_BARE_AVR 1
#endif

#if !defined(PBMICRO_BARE_AVR) && !defined(PBMICRO_FREESTANDING)
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#endif

/* ---- Adaptive Static Memory Budgets (Zero Heap / Zero malloc) ---- */
#if defined(__AVR__) || defined(PBMICRO_TINY_RAM) || (UINTPTR_MAX <= 0xFFFFU)
#define PBMICRO_MAX_FUNCS     10
#define PBMICRO_MAX_CONSTS    48
#define PBMICRO_MAX_GLOBALS   20
#define PBMICRO_STACK_MAX     48
#define PBMICRO_FRAMES_MAX    14
#define PBMICRO_STRBUF_BYTES  96
#else
#define PBMICRO_MAX_FUNCS     128
#define PBMICRO_MAX_CONSTS    1024
#define PBMICRO_MAX_GLOBALS   256
#define PBMICRO_STACK_MAX     1024
#define PBMICRO_FRAMES_MAX    128
#define PBMICRO_STRBUF_BYTES  2048
#endif

/* ---- Exact .pbck v1 Wire Opcode Tags ---- */
enum {
    M_OP_CONSTANT       = 0,
    M_OP_ADD            = 1,
    M_OP_SUB            = 2,
    M_OP_MUL            = 3,
    M_OP_DIV            = 4,
    M_OP_EQ             = 5,
    M_OP_NOT_EQ         = 6,
    M_OP_GT             = 7,
    M_OP_GE             = 8,
    M_OP_LT             = 9,
    M_OP_LE             = 10,
    M_OP_POP            = 11,
    M_OP_PRINT          = 12,
    M_OP_DEFINE_GLOBAL  = 13,
    M_OP_GET_GLOBAL     = 14,
    M_OP_SET_GLOBAL     = 15,
    M_OP_GET_ATTR       = 16,
    M_OP_SET_ATTR       = 17,
    M_OP_EXPORT         = 18,
    M_OP_MAKE_ARRAY     = 19,
    M_OP_MAKE_MAP       = 20,
    M_OP_GET_INDEX      = 21,
    M_OP_SET_INDEX      = 22,
    M_OP_TRY            = 23,
    M_OP_END_TRY        = 24,
    M_OP_GET_LOCAL      = 25,
    M_OP_SET_LOCAL      = 26,
    M_OP_CLOSURE        = 27,
    M_OP_GET_UPVALUE    = 28,
    M_OP_SET_UPVALUE    = 29,
    M_OP_CLOSE_UPVALUE  = 30,
    M_OP_CALL           = 31,
    M_OP_TAIL_CALL      = 32,
    M_OP_RETURN         = 33,
    M_OP_DEFER          = 34,
    M_OP_JUMP           = 35,
    M_OP_JUMP_IF_FALSE  = 36,
    M_OP_JUMP_IF_TRUE   = 37,
    M_OP_NOT            = 38,
    M_OP_NEGATE         = 39,
    M_OP_MOD            = 40,
    M_OP_POW            = 41,
    M_OP_SAFE_GET_ATTR  = 42,
    M_OP_SAFE_INDEX     = 43,
    M_OP_RESULT_UNWRAP  = 44,
    M_OP_IS_NIL         = 45,
    M_OP_DUP            = 46,
    M_OP_THROW          = 47
};

/* ---- Compact Value Representation (5-8 bytes) ---- */
enum {
    MV_NIL     = 0,
    MV_BOOL    = 1,
    MV_INT     = 2,
    MV_STR_ROM = 3, /* Zero-copy slice inside .pbck image */
    MV_STR_BUF = 4, /* Formatted slice inside scratch ring */
    MV_FUNC    = 5,
    MV_BUILTIN = 6
};

enum {
    MBI_PRINT = 1,
    MBI_STR   = 2,
    MBI_LEN   = 3,
    MBI_ABS   = 4,
    MBI_MIN   = 5,
    MBI_MAX   = 6,
    MBI_BAND  = 7,
    MBI_BOR   = 8,
    MBI_BXOR  = 9,
    MBI_BSHL  = 10,
    MBI_BSHR  = 11
};

typedef struct {
    uint8_t  tag;
    uint16_t aux;  /* length for strings */
    int32_t  v;    /* int32 / offset / func_id / builtin_id */
} PbMicroVal;

typedef struct {
    uint32_t code_off;
    uint16_t n_ins;
    uint16_t const_base;
    uint16_t n_const;
    uint8_t  arity;
} PbMicroFunc;

typedef struct {
    uint32_t   name_off;
    uint16_t   name_len;
    PbMicroVal val;
} PbMicroGlobal;

typedef struct {
    uint16_t func_id;
    uint16_t base_slot;
    uint16_t ip_ins;
    uint32_t ip_byte;
} PbMicroFrame;

typedef struct {
    const uint8_t *rom;
    uint32_t       rom_len;
    PbMicroFunc    funcs[PBMICRO_MAX_FUNCS];
    uint16_t       n_funcs;
    PbMicroVal     consts[PBMICRO_MAX_CONSTS];
    uint16_t       n_consts;
    PbMicroGlobal  globals[PBMICRO_MAX_GLOBALS];
    uint16_t       n_globals;
    PbMicroVal     stack[PBMICRO_STACK_MAX];
    uint16_t       sp;
    PbMicroFrame   frames[PBMICRO_FRAMES_MAX];
    uint16_t       fp;
    char           strbuf[PBMICRO_STRBUF_BYTES];
    uint16_t       strbuf_pos;
    const char    *err;
} PbMicroVM;

/* ---- Platform Output Hook ---- */
static void pbm_putc(char c) {
#if defined(PBMICRO_BARE_AVR)
    while (!(UCSR0A & (1 << UDRE0))) {}
    UDR0 = (uint8_t)c;
#elif defined(PBMICRO_PUTC_MMIO)
    *(volatile uint8_t *)(uintptr_t)(PBMICRO_PUTC_MMIO) = (uint8_t)c;
#elif !defined(PBMICRO_FREESTANDING)
    putchar((unsigned char)c);
#else
    (void)c;
#endif
}

static void pbm_puts_n(const char *s, uint16_t n) {
    uint16_t i;
    for (i = 0; i < n; i++) pbm_putc(s[i]);
}

static void pbm_puts_c(const char *s) {
    while (*s) pbm_putc(*s++);
}

static uint16_t pbm_i32_to_dec(int32_t v, char out[16]) {
    char tmp[16];
    uint16_t n = 0, o = 0;
    uint32_t u;
    if (v < 0) {
        out[o++] = '-';
        u = (uint32_t)(-(v + 1)) + 1u;
    } else {
        u = (uint32_t)v;
    }
    if (u == 0) {
        out[o++] = '0';
        return o;
    }
    while (u > 0) {
        tmp[n++] = (char)('0' + (u % 10u));
        u /= 10u;
    }
    while (n > 0) out[o++] = tmp[--n];
    return o;
}

static void pbm_print_val(const PbMicroVM *vm, PbMicroVal v) {
    char num[16];
    uint16_t nl;
    switch (v.tag) {
        case MV_NIL:
            pbm_puts_c("nil");
            break;
        case MV_BOOL:
            pbm_puts_c(v.v ? "true" : "false");
            break;
        case MV_INT:
            nl = pbm_i32_to_dec(v.v, num);
            pbm_puts_n(num, nl);
            break;
        case MV_STR_ROM:
            pbm_puts_n((const char *)(vm->rom + (uint32_t)v.v), v.aux);
            break;
        case MV_STR_BUF:
            pbm_puts_n(vm->strbuf + (uint16_t)v.v, v.aux);
            break;
        default:
            pbm_puts_c("<fn>");
            break;
    }
}

/* ---- Pure-Integer Binary Readers & IEEE-754 f64 -> int32 Decoder ---- */
static bool pbm_read_u8(const uint8_t *rom, uint32_t len, uint32_t *pos, uint8_t *out) {
    if (*pos >= len) return false;
    *out = rom[(*pos)++];
    return true;
}

static bool pbm_read_u32(const uint8_t *rom, uint32_t len, uint32_t *pos, uint32_t *out) {
    if (*pos + 4u > len) return false;
    const uint8_t *p = rom + *pos;
    *out = (uint32_t)p[0] | ((uint32_t)p[1] << 8) | ((uint32_t)p[2] << 16) | ((uint32_t)p[3] << 24);
    *pos += 4u;
    return true;
}

static int32_t pbm_decode_f64_to_i32(const uint8_t b[8]) {
    uint32_t lo = (uint32_t)b[0] | ((uint32_t)b[1] << 8) | ((uint32_t)b[2] << 16) | ((uint32_t)b[3] << 24);
    uint32_t hi = (uint32_t)b[4] | ((uint32_t)b[5] << 8) | ((uint32_t)b[6] << 16) | ((uint32_t)b[7] << 24);
    int sign = (hi >> 31) & 1;
    int exp = (int)((hi >> 20) & 0x7FFu) - 1023;
    uint64_t mant = (((uint64_t)(hi & 0x000FFFFFu)) << 32) | (uint64_t)lo;
    uint64_t mag;
    if (exp < 0) return 0;
    if (exp >= 31) return sign ? (-2147483647 - 1) : 2147483647;
    mant |= (1ULL << 52);
    if (exp >= 52) mag = mant << (exp - 52);
    else mag = mant >> (52 - exp);
    return sign ? -(int32_t)mag : (int32_t)mag;
}

static uint8_t pbm_op_size(const uint8_t *rom, uint32_t len, uint32_t pos, uint32_t *out_arg) {
    if (pos >= len) return 0;
    uint8_t op = rom[pos];
    *out_arg = 0;
    switch (op) {
        case M_OP_CONSTANT:
        case M_OP_DEFINE_GLOBAL:
        case M_OP_GET_GLOBAL:
        case M_OP_SET_GLOBAL:
        case M_OP_GET_ATTR:
        case M_OP_SET_ATTR:
        case M_OP_EXPORT:
        case M_OP_MAKE_ARRAY:
        case M_OP_MAKE_MAP:
        case M_OP_TRY:
        case M_OP_GET_LOCAL:
        case M_OP_SET_LOCAL:
        case M_OP_GET_UPVALUE:
        case M_OP_SET_UPVALUE:
        case M_OP_CALL:
        case M_OP_TAIL_CALL:
        case M_OP_JUMP:
        case M_OP_JUMP_IF_FALSE:
        case M_OP_JUMP_IF_TRUE:
        case M_OP_SAFE_GET_ATTR: {
            uint32_t p = pos + 1u;
            if (!pbm_read_u32(rom, len, &p, out_arg)) return 0;
            return 5;
        }
        case M_OP_CLOSURE: {
            uint32_t p = pos + 1u, n_caps = 0;
            if (!pbm_read_u32(rom, len, &p, out_arg)) return 0;
            if (!pbm_read_u32(rom, len, &p, &n_caps)) return 0;
            if (p + n_caps * 5u > len) return 0;
            return (uint8_t)(9u + n_caps * 5u);
        }
        default:
            return 1;
    }
}

static uint32_t pbm_ins_index_to_byte(const PbMicroVM *vm, const PbMicroFunc *fn, uint16_t target_ins) {
    uint32_t p = fn->code_off;
    uint16_t i;
    for (i = 0; i < target_ins && i < fn->n_ins; i++) {
        uint32_t dummy = 0;
        uint8_t sz = pbm_op_size(vm->rom, vm->rom_len, p, &dummy);
        if (sz == 0) break;
        p += sz;
    }
    return p;
}

/* ---- In-Place .pbck v1 Loader ---- */
static bool pbm_parse_chunk(PbMicroVM *vm, uint32_t *pos, uint8_t arity, uint16_t *out_fid);

static bool pbm_parse_const(PbMicroVM *vm, uint32_t *pos, PbMicroVal *out_val) {
    uint8_t tag = 0;
    if (!pbm_read_u8(vm->rom, vm->rom_len, pos, &tag)) return false;
    out_val->tag = MV_NIL;
    out_val->aux = 0;
    out_val->v = 0;
    switch (tag) {
        case 0: {
            if (*pos + 8u > vm->rom_len) return false;
            out_val->tag = MV_INT;
            out_val->v = pbm_decode_f64_to_i32(vm->rom + *pos);
            *pos += 8u;
            return true;
        }
        case 1: {
            uint8_t b = 0;
            if (!pbm_read_u8(vm->rom, vm->rom_len, pos, &b)) return false;
            out_val->tag = MV_BOOL;
            out_val->v = b ? 1 : 0;
            return true;
        }
        case 2:
            return true;
        case 3:
        case 4:
        case 5:
        case 6: {
            uint32_t slen = 0;
            if (!pbm_read_u32(vm->rom, vm->rom_len, pos, &slen)) return false;
            if (*pos + slen > vm->rom_len) return false;
            if (tag == 4) {
                int32_t acc = 0, sgn = 1;
                uint32_t i = 0;
                if (slen > 0 && vm->rom[*pos] == '-') { sgn = -1; i = 1; }
                for (; i < slen; i++) {
                    uint8_t c = vm->rom[*pos + i];
                    if (c >= '0' && c <= '9') acc = acc * 10 + (int32_t)(c - '0');
                }
                out_val->tag = MV_INT;
                out_val->v = acc * sgn;
            } else {
                out_val->tag = MV_STR_ROM;
                out_val->v = (int32_t)(*pos);
                out_val->aux = (uint16_t)(slen > 0xFFFFu ? 0xFFFFu : slen);
            }
            *pos += slen;
            return true;
        }
        case 7: {
            uint8_t has_name = 0, is_vararg = 0, has_pt = 0;
            uint32_t nlen = 0, arity = 0, min_arity = 0, n_def = 0, i;
            PbMicroVal dummy;
            uint16_t child_fid = 0;
            if (!pbm_read_u8(vm->rom, vm->rom_len, pos, &has_name)) return false;
            if (has_name) {
                if (!pbm_read_u32(vm->rom, vm->rom_len, pos, &nlen)) return false;
                if (*pos + nlen > vm->rom_len) return false;
                *pos += nlen;
            }
            if (!pbm_read_u32(vm->rom, vm->rom_len, pos, &arity)) return false;
            if (!pbm_read_u32(vm->rom, vm->rom_len, pos, &min_arity)) return false;
            (void)min_arity;
            if (!pbm_read_u32(vm->rom, vm->rom_len, pos, &n_def)) return false;
            for (i = 0; i < n_def; i++) {
                if (!pbm_parse_const(vm, pos, &dummy)) return false;
            }
            if (!pbm_read_u8(vm->rom, vm->rom_len, pos, &is_vararg)) return false;
            (void)is_vararg;
            if (!pbm_read_u8(vm->rom, vm->rom_len, pos, &has_pt)) return false;
            if (has_pt) {
                uint32_t n_pt = 0;
                if (!pbm_read_u32(vm->rom, vm->rom_len, pos, &n_pt)) return false;
                if (*pos + n_pt > vm->rom_len) return false;
                *pos += n_pt;
            }
            if (!pbm_parse_chunk(vm, pos, (uint8_t)arity, &child_fid)) return false;
            out_val->tag = MV_FUNC;
            out_val->v = (int32_t)child_fid;
            return true;
        }
        default:
            return false;
    }
}

static bool pbm_parse_chunk(PbMicroVM *vm, uint32_t *pos, uint8_t arity, uint16_t *out_fid) {
    uint32_t n_ins = 0, n_const = 0, n_spans = 0, i;
    uint8_t has_src = 0;
    if (vm->n_funcs >= PBMICRO_MAX_FUNCS) { vm->err = "func table full"; return false; }
    uint16_t fid = vm->n_funcs++;
    PbMicroFunc *fn = &vm->funcs[fid];

    if (!pbm_read_u32(vm->rom, vm->rom_len, pos, &n_ins)) return false;
    fn->code_off = *pos;
    fn->n_ins = (uint16_t)n_ins;
    fn->arity = arity;
    for (i = 0; i < n_ins; i++) {
        uint32_t arg = 0;
        uint8_t sz = pbm_op_size(vm->rom, vm->rom_len, *pos, &arg);
        if (sz == 0) { vm->err = "bad opcode"; return false; }
        *pos += sz;
    }

    if (!pbm_read_u32(vm->rom, vm->rom_len, pos, &n_const)) return false;
    if (vm->n_consts + n_const > PBMICRO_MAX_CONSTS) { vm->err = "const table full"; return false; }
    /* Temp parse into local or direct slot — note nested funcs also append to consts,
     * so parse into temporary stack/buffer or reserve [const_base..const_base+n_const) FIRST! */
    uint16_t cbase = vm->n_consts;
    vm->n_consts = (uint16_t)(vm->n_consts + n_const);
    fn->const_base = cbase;
    fn->n_const = (uint16_t)n_const;
    for (i = 0; i < n_const; i++) {
        PbMicroVal cv;
        if (!pbm_parse_const(vm, pos, &cv)) return false;
        vm->consts[cbase + i] = cv;
    }

    if (!pbm_read_u32(vm->rom, vm->rom_len, pos, &n_spans)) return false;
    if (*pos + n_spans * 8u > vm->rom_len) return false;
    *pos += n_spans * 8u;

    if (!pbm_read_u8(vm->rom, vm->rom_len, pos, &has_src)) return false;
    if (has_src) {
        uint32_t slen = 0;
        if (!pbm_read_u32(vm->rom, vm->rom_len, pos, &slen)) return false;
        if (*pos + slen > vm->rom_len) return false;
        *pos += slen;
    }
    *out_fid = fid;
    return true;
}

/* ---- String & Global Helpers ---- */
static bool pbm_bytes_eq(const uint8_t *a, uint16_t al, const char *b, uint16_t bl) {
    uint16_t i;
    if (al != bl) return false;
    for (i = 0; i < al; i++) if (a[i] != (uint8_t)b[i]) return false;
    return true;
}

static const char *pbm_val_str_ptr(const PbMicroVM *vm, PbMicroVal v, uint16_t *out_len) {
    if (v.tag == MV_STR_ROM) { *out_len = v.aux; return (const char *)(vm->rom + (uint32_t)v.v); }
    if (v.tag == MV_STR_BUF) { *out_len = v.aux; return vm->strbuf + (uint16_t)v.v; }
    *out_len = 0;
    return "";
}

static bool pbm_to_str_val(PbMicroVM *vm, PbMicroVal v, PbMicroVal *out) {
    if (v.tag == MV_STR_ROM || v.tag == MV_STR_BUF) { *out = v; return true; }
    char tmp[16];
    const char *src = tmp;
    uint16_t sl = 0, i;
    if (v.tag == MV_INT) sl = pbm_i32_to_dec(v.v, tmp);
    else if (v.tag == MV_BOOL) { src = v.v ? "true" : "false"; sl = v.v ? 4 : 5; }
    else { src = "nil"; sl = 3; }
    if (vm->strbuf_pos + sl > PBMICRO_STRBUF_BYTES) vm->strbuf_pos = 0;
    uint16_t off = vm->strbuf_pos;
    for (i = 0; i < sl; i++) vm->strbuf[vm->strbuf_pos++] = src[i];
    out->tag = MV_STR_BUF;
    out->v = (int32_t)off;
    out->aux = sl;
    return true;
}

static int16_t pbm_find_builtin(const uint8_t *nm, uint16_t nl) {
    if (pbm_bytes_eq(nm, nl, "print", 5) || pbm_bytes_eq(nm, nl, "println", 7)) return MBI_PRINT;
    if (pbm_bytes_eq(nm, nl, "str", 3) || pbm_bytes_eq(nm, nl, "to_string", 9)) return MBI_STR;
    if (pbm_bytes_eq(nm, nl, "len", 3)) return MBI_LEN;
    if (pbm_bytes_eq(nm, nl, "num_abs", 7) || pbm_bytes_eq(nm, nl, "abs", 3)) return MBI_ABS;
    if (pbm_bytes_eq(nm, nl, "min", 3)) return MBI_MIN;
    if (pbm_bytes_eq(nm, nl, "max", 3)) return MBI_MAX;
    if (pbm_bytes_eq(nm, nl, "bit_and", 7)) return MBI_BAND;
    if (pbm_bytes_eq(nm, nl, "bit_or", 6)) return MBI_BOR;
    if (pbm_bytes_eq(nm, nl, "bit_xor", 7)) return MBI_BXOR;
    if (pbm_bytes_eq(nm, nl, "bit_shl", 7)) return MBI_BSHL;
    if (pbm_bytes_eq(nm, nl, "bit_shr", 7)) return MBI_BSHR;
    return 0;
}

static bool pbm_truthy(PbMicroVal v) {
    if (v.tag == MV_NIL) return false;
    if (v.tag == MV_BOOL || v.tag == MV_INT) return v.v != 0;
    if (v.tag == MV_STR_ROM || v.tag == MV_STR_BUF) return v.aux > 0;
    return true;
}

/* ---- Micro-VM Execution Loop ---- */
bool pbmicro_load_and_run(PbMicroVM *vm, const uint8_t *pbck, uint32_t pbck_len) {
    uint16_t i;
    vm->rom = pbck;
    vm->rom_len = pbck_len;
    vm->n_funcs = 0;
    vm->n_consts = 0;
    vm->n_globals = 0;
    vm->sp = 0;
    vm->fp = 0;
    vm->strbuf_pos = 0;
    vm->err = NULL;

    if (pbck_len < 6u || pbck[0] != 'P' || pbck[1] != 'B' || pbck[2] != 'C' || pbck[3] != 'K') {
        vm->err = "invalid PBCK magic";
        return false;
    }
    if (pbck[4] != 1u) {
        vm->err = "unsupported PBCK version";
        return false;
    }
    uint32_t pos = 5u;
    uint16_t root_fid = 0;
    if (!pbm_parse_chunk(vm, &pos, 0, &root_fid)) {
        if (!vm->err) vm->err = "corrupt PBCK chunk";
        return false;
    }

    /* Push root frame (slot 0 reserved for script callee) */
    PbMicroVal nilv = { MV_NIL, 0, 0 };
    vm->stack[vm->sp++] = nilv;
    vm->frames[0].func_id = root_fid;
    vm->frames[0].base_slot = 0;
    vm->frames[0].ip_ins = 0;
    vm->frames[0].ip_byte = vm->funcs[root_fid].code_off;
    vm->fp = 1;

    while (vm->fp > 0) {
        PbMicroFrame *fr = &vm->frames[vm->fp - 1];
        const PbMicroFunc *fn = &vm->funcs[fr->func_id];
        if (fr->ip_ins >= fn->n_ins) {
            vm->fp--;
            if (vm->fp == 0) return true;
            vm->sp = fr->base_slot;
            vm->stack[vm->sp++] = nilv;
            continue;
        }
        uint8_t op = vm->rom[fr->ip_byte];
        uint32_t arg = 0;
        uint8_t sz = pbm_op_size(vm->rom, vm->rom_len, fr->ip_byte, &arg);
        fr->ip_byte += sz;
        fr->ip_ins++;

        switch (op) {
            case M_OP_CONSTANT:
                if (vm->sp >= PBMICRO_STACK_MAX) { vm->err = "stack overflow"; return false; }
                vm->stack[vm->sp++] = vm->consts[fn->const_base + (uint16_t)arg];
                break;
            case M_OP_POP:
                if (vm->sp > fr->base_slot) vm->sp--;
                break;
            case M_OP_DUP:
                if (vm->sp == 0 || vm->sp >= PBMICRO_STACK_MAX) { vm->err = "dup overflow"; return false; }
                vm->stack[vm->sp] = vm->stack[vm->sp - 1];
                vm->sp++;
                break;
            case M_OP_PRINT: {
                PbMicroVal v = vm->stack[--vm->sp];
                pbm_print_val(vm, v);
                pbm_putc('\n');
                break;
            }
            case M_OP_GET_LOCAL: {
                uint16_t slot = (uint16_t)(fr->base_slot + 1u + (uint16_t)arg);
                if (vm->sp >= PBMICRO_STACK_MAX) { vm->err = "stack overflow"; return false; }
                PbMicroVal lv = (slot < vm->sp) ? vm->stack[slot] : nilv;
                vm->stack[vm->sp++] = lv;
                break;
            }
            case M_OP_SET_LOCAL: {
                PbMicroVal lv = vm->stack[--vm->sp];
                uint16_t slot = (uint16_t)(fr->base_slot + 1u + (uint16_t)arg);
                while (vm->sp <= slot && vm->sp < PBMICRO_STACK_MAX) vm->stack[vm->sp++] = nilv;
                vm->stack[slot] = lv;
                break;
            }
            case M_OP_DEFINE_GLOBAL:
            case M_OP_SET_GLOBAL: {
                PbMicroVal name_c = vm->consts[fn->const_base + (uint16_t)arg];
                PbMicroVal val = vm->stack[--vm->sp];
                const uint8_t *nm = vm->rom + (uint32_t)name_c.v;
                uint16_t nl = name_c.aux;
                bool found = false;
                for (i = 0; i < vm->n_globals; i++) {
                    if (vm->globals[i].name_len == nl &&
                        pbm_bytes_eq(vm->rom + vm->globals[i].name_off, nl, (const char *)nm, nl)) {
                        vm->globals[i].val = val;
                        found = true;
                        break;
                    }
                }
                if (!found) {
                    if (vm->n_globals >= PBMICRO_MAX_GLOBALS) { vm->err = "globals full"; return false; }
                    vm->globals[vm->n_globals].name_off = (uint32_t)name_c.v;
                    vm->globals[vm->n_globals].name_len = nl;
                    vm->globals[vm->n_globals].val = val;
                    vm->n_globals++;
                }
                break;
            }
            case M_OP_GET_GLOBAL: {
                PbMicroVal name_c = vm->consts[fn->const_base + (uint16_t)arg];
                const uint8_t *nm = vm->rom + (uint32_t)name_c.v;
                uint16_t nl = name_c.aux;
                PbMicroVal out = nilv;
                bool found = false;
                for (i = 0; i < vm->n_globals; i++) {
                    if (vm->globals[i].name_len == nl &&
                        pbm_bytes_eq(vm->rom + vm->globals[i].name_off, nl, (const char *)nm, nl)) {
                        out = vm->globals[i].val;
                        found = true;
                        break;
                    }
                }
                if (!found) {
                    int16_t bi = pbm_find_builtin(nm, nl);
                    if (bi > 0) { out.tag = MV_BUILTIN; out.v = bi; found = true; }
                }
                if (!found) { vm->err = "undefined global"; return false; }
                if (vm->sp >= PBMICRO_STACK_MAX) { vm->err = "stack overflow"; return false; }
                vm->stack[vm->sp++] = out;
                break;
            }
            case M_OP_CLOSURE: {
                PbMicroVal fn_c = vm->consts[fn->const_base + (uint16_t)arg];
                if (vm->sp >= PBMICRO_STACK_MAX) { vm->err = "stack overflow"; return false; }
                vm->stack[vm->sp++] = fn_c;
                break;
            }
            case M_OP_ADD: {
                PbMicroVal b = vm->stack[--vm->sp];
                PbMicroVal a = vm->stack[--vm->sp];
                if (a.tag == MV_INT && b.tag == MV_INT) {
                    PbMicroVal r = { MV_INT, 0, (int32_t)((uint32_t)a.v + (uint32_t)b.v) };
                    vm->stack[vm->sp++] = r;
                } else {
                    PbMicroVal sa, sb;
                    uint16_t la = 0, lb = 0, k;
                    pbm_to_str_val(vm, a, &sa);
                    pbm_to_str_val(vm, b, &sb);
                    const char *pa = pbm_val_str_ptr(vm, sa, &la);
                    const char *pb = pbm_val_str_ptr(vm, sb, &lb);
                    if ((uint32_t)la + (uint32_t)lb > PBMICRO_STRBUF_BYTES) { vm->err = "strbuf full"; return false; }
                    if (vm->strbuf_pos + la + lb > PBMICRO_STRBUF_BYTES) vm->strbuf_pos = 0;
                    uint16_t dst = vm->strbuf_pos;
                    for (k = 0; k < la; k++) vm->strbuf[vm->strbuf_pos++] = pa[k];
                    for (k = 0; k < lb; k++) vm->strbuf[vm->strbuf_pos++] = pb[k];
                    PbMicroVal r = { MV_STR_BUF, (uint16_t)(la + lb), (int32_t)dst };
                    vm->stack[vm->sp++] = r;
                }
                break;
            }
            case M_OP_SUB:
            case M_OP_MUL:
            case M_OP_DIV:
            case M_OP_MOD: {
                PbMicroVal b = vm->stack[--vm->sp];
                PbMicroVal a = vm->stack[--vm->sp];
                int32_t rv = 0;
                if (op == M_OP_SUB) rv = (int32_t)((uint32_t)a.v - (uint32_t)b.v);
                else if (op == M_OP_MUL) rv = (int32_t)((uint32_t)a.v * (uint32_t)b.v);
                else if (b.v == 0) { vm->err = "division by zero"; return false; }
                else if (op == M_OP_DIV) rv = a.v / b.v;
                else rv = a.v % b.v;
                PbMicroVal r = { MV_INT, 0, rv };
                vm->stack[vm->sp++] = r;
                break;
            }
            case M_OP_NEGATE:
                vm->stack[vm->sp - 1].v = -vm->stack[vm->sp - 1].v;
                break;
            case M_OP_NOT: {
                PbMicroVal v = vm->stack[vm->sp - 1];
                PbMicroVal r = { MV_BOOL, 0, pbm_truthy(v) ? 0 : 1 };
                vm->stack[vm->sp - 1] = r;
                break;
            }
            case M_OP_IS_NIL: {
                PbMicroVal v = vm->stack[vm->sp - 1];
                PbMicroVal r = { MV_BOOL, 0, v.tag == MV_NIL ? 1 : 0 };
                vm->stack[vm->sp - 1] = r;
                break;
            }
            case M_OP_EQ:
            case M_OP_NOT_EQ: {
                PbMicroVal b = vm->stack[--vm->sp];
                PbMicroVal a = vm->stack[--vm->sp];
                bool eq = false;
                if (a.tag == b.tag && (a.tag == MV_NIL || a.tag == MV_BOOL || a.tag == MV_INT)) {
                    eq = (a.tag == MV_NIL) ? true : (a.v == b.v);
                } else if ((a.tag == MV_STR_ROM || a.tag == MV_STR_BUF) &&
                           (b.tag == MV_STR_ROM || b.tag == MV_STR_BUF)) {
                    uint16_t la = 0, lb = 0;
                    const char *pa = pbm_val_str_ptr(vm, a, &la);
                    const char *pb = pbm_val_str_ptr(vm, b, &lb);
                    eq = pbm_bytes_eq((const uint8_t *)pa, la, pb, lb);
                }
                PbMicroVal r = { MV_BOOL, 0, (op == M_OP_EQ ? eq : !eq) ? 1 : 0 };
                vm->stack[vm->sp++] = r;
                break;
            }
            case M_OP_LT:
            case M_OP_LE:
            case M_OP_GT:
            case M_OP_GE: {
                PbMicroVal b = vm->stack[--vm->sp];
                PbMicroVal a = vm->stack[--vm->sp];
                bool cmp = false;
                if (op == M_OP_LT) cmp = a.v < b.v;
                else if (op == M_OP_LE) cmp = a.v <= b.v;
                else if (op == M_OP_GT) cmp = a.v > b.v;
                else cmp = a.v >= b.v;
                PbMicroVal r = { MV_BOOL, 0, cmp ? 1 : 0 };
                vm->stack[vm->sp++] = r;
                break;
            }
            case M_OP_JUMP:
                fr->ip_ins = (uint16_t)arg;
                fr->ip_byte = pbm_ins_index_to_byte(vm, fn, fr->ip_ins);
                break;
            case M_OP_JUMP_IF_FALSE:
                if (!pbm_truthy(vm->stack[vm->sp - 1])) {
                    fr->ip_ins = (uint16_t)arg;
                    fr->ip_byte = pbm_ins_index_to_byte(vm, fn, fr->ip_ins);
                }
                break;
            case M_OP_JUMP_IF_TRUE:
                if (pbm_truthy(vm->stack[vm->sp - 1])) {
                    fr->ip_ins = (uint16_t)arg;
                    fr->ip_byte = pbm_ins_index_to_byte(vm, fn, fr->ip_ins);
                }
                break;
            case M_OP_CALL:
            case M_OP_TAIL_CALL: {
                uint16_t argc = (uint16_t)arg;
                uint16_t callee_slot = (uint16_t)(vm->sp - argc - 1u);
                PbMicroVal callee = vm->stack[callee_slot];
                if (callee.tag == MV_BUILTIN) {
                    PbMicroVal res = nilv;
                    PbMicroVal *av = &vm->stack[callee_slot + 1u];
                    switch (callee.v) {
                        case MBI_PRINT:
                            for (i = 0; i < argc; i++) {
                                if (i > 0) pbm_putc(' ');
                                pbm_print_val(vm, av[i]);
                            }
                            pbm_putc('\n');
                            break;
                        case MBI_STR:
                            if (argc >= 1) pbm_to_str_val(vm, av[0], &res);
                            break;
                        case MBI_LEN:
                            res.tag = MV_INT;
                            res.v = (argc >= 1 && (av[0].tag == MV_STR_ROM || av[0].tag == MV_STR_BUF)) ? av[0].aux : 0;
                            break;
                        case MBI_ABS:
                            res.tag = MV_INT;
                            res.v = (argc >= 1 && av[0].v < 0) ? -av[0].v : (argc >= 1 ? av[0].v : 0);
                            break;
                        case MBI_MIN:
                            res.tag = MV_INT;
                            res.v = (argc >= 2) ? (av[0].v < av[1].v ? av[0].v : av[1].v) : 0;
                            break;
                        case MBI_MAX:
                            res.tag = MV_INT;
                            res.v = (argc >= 2) ? (av[0].v > av[1].v ? av[0].v : av[1].v) : 0;
                            break;
                        case MBI_BAND:
                            res.tag = MV_INT; res.v = (argc >= 2) ? (av[0].v & av[1].v) : 0; break;
                        case MBI_BOR:
                            res.tag = MV_INT; res.v = (argc >= 2) ? (av[0].v | av[1].v) : 0; break;
                        case MBI_BXOR:
                            res.tag = MV_INT; res.v = (argc >= 2) ? (av[0].v ^ av[1].v) : 0; break;
                        case MBI_BSHL:
                            res.tag = MV_INT; res.v = (argc >= 2) ? (int32_t)((uint32_t)av[0].v << (av[1].v & 31)) : 0; break;
                        case MBI_BSHR:
                            res.tag = MV_INT; res.v = (argc >= 2) ? (int32_t)((uint32_t)av[0].v >> (av[1].v & 31)) : 0; break;
                    }
                    vm->sp = callee_slot;
                    vm->stack[vm->sp++] = res;
                } else if (callee.tag == MV_FUNC) {
                    uint16_t fid = (uint16_t)callee.v;
                    if (vm->fp >= PBMICRO_FRAMES_MAX) { vm->err = "call depth overflow"; return false; }
                    PbMicroFrame *nf = &vm->frames[vm->fp++];
                    nf->func_id = fid;
                    nf->base_slot = callee_slot;
                    nf->ip_ins = 0;
                    nf->ip_byte = vm->funcs[fid].code_off;
                } else {
                    vm->err = "call target not callable";
                    return false;
                }
                break;
            }
            case M_OP_RETURN: {
                PbMicroVal ret = (vm->sp > fr->base_slot) ? vm->stack[--vm->sp] : nilv;
                uint16_t base = fr->base_slot;
                vm->fp--;
                if (vm->fp == 0) return true;
                vm->sp = base;
                vm->stack[vm->sp++] = ret;
                break;
            }
            case M_OP_CLOSE_UPVALUE:
            case M_OP_EXPORT:
            case M_OP_END_TRY:
                break;
            default:
                vm->err = "opcode unsupported in micro subset";
                return false;
        }
    }
    return true;
}

/* ---- Built-in Self-Test PBCK v1 Image (`print 6 * 7;`) ---- */
static const uint8_t g_pbmicro_selftest_pbck[] = {
    'P', 'B', 'C', 'K', 1,
    /* n_ins = 5 */
    5, 0, 0, 0,
    M_OP_CONSTANT, 0, 0, 0, 0,
    M_OP_CONSTANT, 1, 0, 0, 0,
    M_OP_MUL,
    M_OP_PRINT,
    M_OP_RETURN,
    /* n_const = 2: 6.0 (0x4018000000000000), 7.0 (0x401C000000000000) */
    2, 0, 0, 0,
    0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x18, 0x40,
    0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x1C, 0x40,
    /* n_spans = 0, has_src = 0 */
    0, 0, 0, 0,
    0
};

static PbMicroVM g_micro_vm;

#if defined(PBMICRO_BARE_AVR)
int main(void) {
    /* Initialize ATmega328P UART0: 9600 8N1 @ 16 MHz (UBRR0 = 103) */
    UBRR0H = 0;
    UBRR0L = 103;
    UCSR0B = (1 << TXEN0);
    UCSR0C = (1 << UCSZ01) | (1 << UCSZ00);
    DDRB |= (1 << 5); /* Arduino D13 status LED */

    pbm_puts_c(PBMICRO_VERSION_STR "\r\n");
    if (pbmicro_load_and_run(&g_micro_vm, g_pbmicro_selftest_pbck, sizeof(g_pbmicro_selftest_pbck))) {
        PORTB |= (1 << 5);
        pbm_puts_c("AVR8 OK\r\n");
    } else {
        pbm_puts_c("AVR8 ERR\r\n");
    }
    for (;;) {}
    return 0;
}
#elif !defined(PBMICRO_FREESTANDING) && !defined(PBMICRO_LIBRARY_ONLY)
int main(int argc, char **argv) {
    if (argc < 2 || strcmp(argv[1], "--help") == 0 || strcmp(argv[1], "-h") == 0) {
        printf("%s\nUsage:\n  %s <program.pbck>\n  %s --selftest\n  %s --version\n",
               PBMICRO_VERSION_STR, argv[0], argv[0], argv[0]);
        return 0;
    }
    if (strcmp(argv[1], "--version") == 0 || strcmp(argv[1], "-V") == 0) {
        printf("%s (word_bits=%u, static_vm_bytes=%u)\n",
               PBMICRO_VERSION_STR,
               (unsigned)(sizeof(void *) * 8u),
               (unsigned)sizeof(PbMicroVM));
        return 0;
    }
    if (strcmp(argv[1], "--selftest") == 0) {
        if (!pbmicro_load_and_run(&g_micro_vm, g_pbmicro_selftest_pbck, sizeof(g_pbmicro_selftest_pbck))) {
            fprintf(stderr, "selftest failed: %s\n", g_micro_vm.err ? g_micro_vm.err : "error");
            return 1;
        }
        return 0;
    }
    FILE *fp = fopen(argv[1], "rb");
    if (!fp) {
        fprintf(stderr, "pblang-micro-vm: cannot open '%s'\n", argv[1]);
        return 1;
    }
    if (fseek(fp, 0, SEEK_END) != 0) { fclose(fp); return 1; }
    long flen = ftell(fp);
    if (flen <= 0 || flen > 4 * 1024 * 1024) { fclose(fp); return 1; }
    rewind(fp);
    uint8_t *buf = (uint8_t *)malloc((size_t)flen);
    if (!buf) { fclose(fp); return 1; }
    if (fread(buf, 1, (size_t)flen, fp) != (size_t)flen) { free(buf); fclose(fp); return 1; }
    fclose(fp);
    bool ok = pbmicro_load_and_run(&g_micro_vm, buf, (uint32_t)flen);
    free(buf);
    if (!ok) {
        fprintf(stderr, "pblang-micro-vm error: %s\n", g_micro_vm.err ? g_micro_vm.err : "execution failed");
        return 1;
    }
    return 0;
}
#endif
