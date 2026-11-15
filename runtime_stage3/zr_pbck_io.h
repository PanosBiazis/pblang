/*
 * zr_pbck_io.h — .pbck v1 & PBSH v1 Polymorphic Shield Serializer / Deserializer & PBLZREXE Bundler
 */
#ifndef PBLANG_ZR_PBCK_IO_H
#define PBLANG_ZR_PBCK_IO_H

#include "zr_vm_core.h"

#define ZR_PBCK_MAX_BYTES        (64u * 1024u * 1024u)
#define ZR_PBCK_VERSION          1u
#define ZR_PBSH_VERSION          1u
#define ZR_PBSH_HEADER_BYTES     84u
#define ZR_PBSH_MAC_BYTES        16u
#define ZR_PBSH_PREFIX_BYTES     100u
#define ZR_PBSH_FLAG_STRIPPED    0x01u
#define ZR_PBSH_FLAG_OPCODE_PERM 0x02u
#define ZR_PBSH_FLAG_CHACHA20    0x04u
#define ZR_PBSH_FLAG_ANTI_DEBUG  0x08u
#define ZR_PBCK_MAX_DEPTH        256u
#define ZR_BUNDLE_MAGIC          "PBLZREXE"

typedef struct {
    bool           strip_debug;
    const uint8_t *op_perm; /* 256-byte forward map or NULL */
} ZrPbckWriteOpts;

typedef struct {
    const uint8_t *buf;
    size_t         len;
    size_t         pos;
    const uint8_t *op_inv_perm; /* 256-byte inverse opcode map or NULL */
    char           err[256];
} ZrPbckReader;

static inline bool zr_pr_need(ZrPbckReader *r, size_t n) {
    if (n > r->len - r->pos) {
        snprintf(r->err, sizeof(r->err), "pbck: unexpected EOF at offset %zu (need %zu)", r->pos, n);
        return false;
    }
    return true;
}

static inline bool zr_pr_u8(ZrPbckReader *r, uint8_t *out) {
    if (!zr_pr_need(r, 1)) return false;
    *out = r->buf[r->pos++];
    return true;
}

static inline bool zr_pr_flag(ZrPbckReader *r, const char *what, bool *out) {
    uint8_t v = 0;
    if (!zr_pr_u8(r, &v)) return false;
    if (v > 1) {
        snprintf(r->err, sizeof(r->err), "pbck: invalid %s flag %u", what, (unsigned)v);
        return false;
    }
    *out = (v == 1);
    return true;
}

static inline bool zr_pr_u32(ZrPbckReader *r, uint32_t *out) {
    if (!zr_pr_need(r, 4)) return false;
    const uint8_t *p = r->buf + r->pos;
    *out = (uint32_t)p[0] | ((uint32_t)p[1] << 8) | ((uint32_t)p[2] << 16) | ((uint32_t)p[3] << 24);
    r->pos += 4;
    return true;
}

static inline bool zr_pr_f64(ZrPbckReader *r, double *out) {
    if (!zr_pr_need(r, 8)) return false;
    uint64_t bits = 0;
    for (int i = 0; i < 8; i++) bits |= ((uint64_t)r->buf[r->pos + i]) << (i * 8);
    r->pos += 8;
    memcpy(out, &bits, sizeof(double));
    return true;
}

static inline bool zr_pr_str(ZrPbckReader *r, PbVM *vm, PbStr **out) {
    uint32_t slen = 0;
    if (!zr_pr_u32(r, &slen)) return false;
    if (!zr_pr_need(r, slen)) return false;
    *out = pb_str_new(vm, (const char *)(r->buf + r->pos), slen);
    r->pos += slen;
    return true;
}

static bool zr_pbck_read_chunk_into(ZrPbckReader *r, PbVM *vm, PbFunc *fn, uint32_t depth);
static bool zr_pbck_read_func(ZrPbckReader *r, PbVM *vm, PbFunc **out_fn, uint32_t depth);

static bool zr_pbck_read_opcode(ZrPbckReader *r, PbInstr *ins) {
    memset(ins, 0, sizeof(*ins));
    uint8_t raw_tag = 0;
    if (!zr_pr_u8(r, &raw_tag)) return false;
    uint8_t tag = r->op_inv_perm ? r->op_inv_perm[raw_tag] : raw_tag;
    ins->op = tag;
    ins->orig_op = tag;
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
            return zr_pr_u32(r, &ins->arg);
        case OP_CLOSURE: {
            if (!zr_pr_u32(r, &ins->arg)) return false;
            uint32_t n_caps = 0;
            if (!zr_pr_u32(r, &n_caps)) return false;
            if ((size_t)n_caps * 5u > r->len - r->pos) {
                snprintf(r->err, sizeof(r->err), "pbck: truncated closure captures");
                return false;
            }
            ins->n_caps = n_caps;
            ins->caps = n_caps > 0 ? (PbCaptureSpec *)zr_xcalloc(n_caps, sizeof(PbCaptureSpec)) : NULL;
            for (uint32_t i = 0; i < n_caps; i++) {
                bool is_loc = false;
                uint32_t idx = 0;
                if (!zr_pr_flag(r, "closure capture", &is_loc) || !zr_pr_u32(r, &idx)) {
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

static bool zr_pbck_read_value(ZrPbckReader *r, PbVM *vm, PbValue *out, uint32_t depth) {
    uint8_t tag = 0;
    if (!zr_pr_u8(r, &tag)) return false;
    switch (tag) {
        case 0: {
            double d = 0.0;
            if (!zr_pr_f64(r, &d)) return false;
            *out = val_num(d);
            return true;
        }
        case 1: {
            bool b = false;
            if (!zr_pr_flag(r, "boolean", &b)) return false;
            *out = val_bool(b);
            return true;
        }
        case 2:
            *out = val_nil();
            return true;
        case 3: {
            PbStr *s = NULL;
            if (!zr_pr_str(r, vm, &s)) return false;
            *out = val_obj(s);
            return true;
        }
        case 4: {
            PbStr *s = NULL;
            if (!zr_pr_str(r, vm, &s)) return false;
            ZrBigInt bi;
            if (!zr_bi_from_str(&bi, s->chars)) {
                snprintf(r->err, sizeof(r->err), "pbck: invalid bigint literal");
                return false;
            }
            *out = pb_bigint_new_take(vm, &bi);
            return true;
        }
        case 5: {
            PbStr *s = NULL;
            if (!zr_pr_str(r, vm, &s)) return false;
            ZrRational rat;
            if (!zr_rat_from_str(&rat, s->chars)) {
                snprintf(r->err, sizeof(r->err), "pbck: invalid rational literal");
                return false;
            }
            *out = pb_rational_new_take(vm, &rat);
            return true;
        }
        case 6: {
            PbStr *s = NULL;
            if (!zr_pr_str(r, vm, &s)) return false;
            ZrDecimal dec;
            if (!zr_dec_from_str(&dec, s->chars)) {
                snprintf(r->err, sizeof(r->err), "pbck: invalid decimal literal");
                return false;
            }
            *out = pb_decimal_new_take(vm, &dec);
            return true;
        }
        case 7: {
            PbFunc *fn = NULL;
            if (!zr_pbck_read_func(r, vm, &fn, depth)) return false;
            *out = val_obj(fn);
            return true;
        }
        case 8: {
            if (!zr_pr_need(r, 8)) return false;
            uint64_t bits = 0;
            for (int i = 0; i < 8; i++) bits |= ((uint64_t)r->buf[r->pos + i]) << (i * 8);
            r->pos += 8;
            *out = val_i64((int64_t)bits);
            return true;
        }
        default:
            snprintf(r->err, sizeof(r->err), "pbck: unsupported constant value tag %u", (unsigned)tag);
            return false;
    }
}

static bool zr_pbck_read_func(ZrPbckReader *r, PbVM *vm, PbFunc **out_fn, uint32_t depth) {
    if (depth >= ZR_PBCK_MAX_DEPTH) {
        snprintf(r->err, sizeof(r->err), "pbck: function nesting exceeds %u", ZR_PBCK_MAX_DEPTH);
        return false;
    }
    PbFunc *fn = pb_func_new(vm);
    bool has_name = false;
    if (!zr_pr_flag(r, "function-name", &has_name)) return false;
    if (has_name) {
        if (!zr_pr_str(r, vm, &fn->name)) return false;
        vm_write_barrier_obj(vm, (PbObj *)fn, (PbObj *)fn->name);
    }
    if (!zr_pr_u32(r, &fn->arity) || !zr_pr_u32(r, &fn->min_arity)) return false;
    uint32_t n_def = 0;
    if (!zr_pr_u32(r, &n_def)) return false;
    if ((size_t)n_def > r->len - r->pos) {
        snprintf(r->err, sizeof(r->err), "pbck: truncated function defaults");
        return false;
    }
    fn->n_defaults = n_def;
    fn->defaults = n_def > 0 ? (PbValue *)zr_xcalloc(n_def, sizeof(PbValue)) : NULL;
    for (uint32_t i = 0; i < n_def; i++) {
        if (!zr_pbck_read_value(r, vm, &fn->defaults[i], depth + 1)) return false;
        vm_write_barrier(vm, (PbObj *)fn, fn->defaults[i]);
    }
    if (!zr_pr_flag(r, "vararg", &fn->is_vararg)) return false;
    bool has_pt = false;
    if (!zr_pr_flag(r, "parameter-types", &has_pt)) return false;
    if (has_pt) {
        uint32_t n_pt = 0;
        if (!zr_pr_u32(r, &n_pt)) return false;
        if (!zr_pr_need(r, n_pt)) return false;
        for (uint32_t i = 0; i < n_pt; i++) {
            uint8_t pt = 0;
            if (!zr_pr_u8(r, &pt) || pt > 2) {
                snprintf(r->err, sizeof(r->err), "pbck: invalid param type %u", (unsigned)pt);
                return false;
            }
        }
    }
    if (!zr_pbck_read_chunk_into(r, vm, fn, depth + 1)) return false;
    *out_fn = fn;
    return true;
}

static bool zr_pbck_read_chunk_into(ZrPbckReader *r, PbVM *vm, PbFunc *fn, uint32_t depth) {
    uint32_t n_ins = 0;
    if (!zr_pr_u32(r, &n_ins)) return false;
    if ((size_t)n_ins > r->len - r->pos) {
        snprintf(r->err, sizeof(r->err), "pbck: truncated instructions");
        return false;
    }
    fn->code_len = n_ins;
    fn->code_cap = n_ins;
    fn->code = n_ins > 0 ? (PbInstr *)zr_xcalloc(n_ins, sizeof(PbInstr)) : NULL;
    for (uint32_t i = 0; i < n_ins; i++) {
        if (!zr_pbck_read_opcode(r, &fn->code[i])) return false;
    }

    uint32_t n_const = 0;
    if (!zr_pr_u32(r, &n_const)) return false;
    if ((size_t)n_const > r->len - r->pos) {
        snprintf(r->err, sizeof(r->err), "pbck: truncated constants");
        return false;
    }
    fn->const_len = n_const;
    fn->const_cap = n_const;
    fn->consts = n_const > 0 ? (PbValue *)zr_xcalloc(n_const, sizeof(PbValue)) : NULL;
    for (uint32_t i = 0; i < n_const; i++) {
        if (!zr_pbck_read_value(r, vm, &fn->consts[i], depth)) return false;
        vm_write_barrier(vm, (PbObj *)fn, fn->consts[i]);
    }

    uint32_t n_spans = 0;
    if (!zr_pr_u32(r, &n_spans)) return false;
    if ((size_t)n_spans * 8u > r->len - r->pos) {
        snprintf(r->err, sizeof(r->err), "pbck: truncated spans");
        return false;
    }
    for (uint32_t i = 0; i < n_spans; i++) {
        uint32_t s_lo = 0, s_hi = 0;
        zr_pr_u32(r, &s_lo);
        zr_pr_u32(r, &s_hi);
        if (i < fn->code_len) fn->code[i].line = s_lo;
        (void)s_hi;
    }

    bool has_src = false;
    if (!zr_pr_flag(r, "source-name", &has_src)) return false;
    if (has_src) {
        if (!zr_pr_str(r, vm, &fn->source_name)) return false;
        vm_write_barrier_obj(vm, (PbObj *)fn, (PbObj *)fn->source_name);
    }
    zr_peephole_optimize_func(fn);
    return true;
}

/* ---- PBSH v1 Cryptographic Primitives (RFC 8439 ChaCha20 + MBA White-Box + Keyed MAC) ---- */

static const uint8_t ZR_WB_CONST[32] = {
    0x63u, 0x7Cu, 0x77u, 0x7Bu, 0xF2u, 0x6Bu, 0x6Fu, 0xC5u,
    0x30u, 0x01u, 0x67u, 0x2Bu, 0xFEu, 0xD7u, 0xABu, 0x76u,
    0xCAu, 0x82u, 0xC9u, 0x7Du, 0xFAu, 0x59u, 0x47u, 0xF0u,
    0xADu, 0xD4u, 0xA2u, 0xAFu, 0x9Cu, 0xA4u, 0x72u, 0xC0u
};

static inline uint32_t zr_pbsh_rotl32(uint32_t x, uint32_t n) {
    return (x << n) | (x >> (32u - n));
}

static inline uint32_t zr_pbsh_load_u32_le(const uint8_t *p) {
    return (uint32_t)p[0] | ((uint32_t)p[1] << 8) | ((uint32_t)p[2] << 16) | ((uint32_t)p[3] << 24);
}

#define ZR_PBSH_QR(a, b, c, d) do { \
    (a) += (b); (d) ^= (a); (d) = zr_pbsh_rotl32((d), 16u); \
    (c) += (d); (b) ^= (c); (b) = zr_pbsh_rotl32((b), 12u); \
    (a) += (b); (d) ^= (a); (d) = zr_pbsh_rotl32((d),  8u); \
    (c) += (d); (b) ^= (c); (b) = zr_pbsh_rotl32((b),  7u); \
} while (0)

static void zr_pbsh_chacha20_block(const uint8_t key[32], uint32_t counter, const uint8_t nonce[12], uint8_t out[64]) {
    uint32_t st[16], wk[16];
    st[0]  = 0x61707865u;
    st[1]  = 0x3320646Eu;
    st[2]  = 0x79622D32u;
    st[3]  = 0x6B206574u;
    for (int i = 0; i < 8; i++) st[4 + i] = zr_pbsh_load_u32_le(key + i * 4);
    st[12] = counter;
    st[13] = zr_pbsh_load_u32_le(nonce + 0);
    st[14] = zr_pbsh_load_u32_le(nonce + 4);
    st[15] = zr_pbsh_load_u32_le(nonce + 8);

    memcpy(wk, st, sizeof(st));
    for (int r = 0; r < 10; r++) {
        ZR_PBSH_QR(wk[0], wk[4], wk[8],  wk[12]);
        ZR_PBSH_QR(wk[1], wk[5], wk[9],  wk[13]);
        ZR_PBSH_QR(wk[2], wk[6], wk[10], wk[14]);
        ZR_PBSH_QR(wk[3], wk[7], wk[11], wk[15]);
        ZR_PBSH_QR(wk[0], wk[5], wk[10], wk[15]);
        ZR_PBSH_QR(wk[1], wk[6], wk[11], wk[12]);
        ZR_PBSH_QR(wk[2], wk[7], wk[8],  wk[13]);
        ZR_PBSH_QR(wk[3], wk[4], wk[9],  wk[14]);
    }
    for (int i = 0; i < 16; i++) {
        uint32_t v = wk[i] + st[i];
        out[i * 4 + 0] = (uint8_t)(v & 0xFFu);
        out[i * 4 + 1] = (uint8_t)((v >> 8) & 0xFFu);
        out[i * 4 + 2] = (uint8_t)((v >> 16) & 0xFFu);
        out[i * 4 + 3] = (uint8_t)((v >> 24) & 0xFFu);
    }
    zr_secure_zero(st, sizeof(st));
    zr_secure_zero(wk, sizeof(wk));
}
#undef ZR_PBSH_QR

static void zr_pbsh_chacha20_xor(const uint8_t key[32], uint32_t init_counter, const uint8_t nonce[12], uint8_t *buf, size_t len) {
    uint8_t ks[64];
    uint32_t ctr = init_counter;
    size_t off = 0;
    while (off < len) {
        zr_pbsh_chacha20_block(key, ctr++, nonce, ks);
        size_t chunk = (len - off < 64u) ? (len - off) : 64u;
        for (size_t i = 0; i < chunk; i++) buf[off + i] ^= ks[i];
        off += chunk;
    }
    zr_secure_zero(ks, sizeof(ks));
}

static void zr_pbsh_unseal_key(const uint8_t nonce[12], const uint8_t share_a[32], const uint8_t share_b[32], uint8_t out_k[32]) {
    for (size_t i = 0; i < 32; i++) {
        /* Mixed Boolean-Arithmetic (MBA) White-Box Unsealing: (a ^ b) + 2*(a & b) == a + b */
        uint32_t mba = (uint32_t)(share_a[i] ^ share_b[i]) + 2u * (uint32_t)(share_a[i] & share_b[i]);
        out_k[i] = (uint8_t)((mba & 0xFFu) ^ ZR_WB_CONST[i] ^ nonce[i % 12u]);
    }
}

static void zr_pbsh_derive_schedule(const uint8_t key[32],
                                    const uint8_t nonce[12],
                                    uint8_t mac_subkey[32],
                                    uint8_t *op_perm,
                                    uint8_t *op_inv_perm) {
    uint8_t ks0[64];
    zr_pbsh_chacha20_block(key, 0u, nonce, ks0);
    memcpy(mac_subkey, ks0, 32);

    if (op_perm || op_inv_perm) {
        uint8_t pstream[512];
        for (uint32_t b = 0; b < 8u; b++) {
            zr_pbsh_chacha20_block(ks0 + 32, b, nonce, pstream + b * 64u);
        }
        uint8_t perm[256];
        for (int i = 0; i < 256; i++) perm[i] = (uint8_t)i;
        for (int i = 255; i > 0; i--) {
            uint32_t rnd = (uint32_t)pstream[i * 2] | ((uint32_t)pstream[i * 2 + 1] << 8);
            uint32_t j = rnd % (uint32_t)(i + 1);
            uint8_t tmp = perm[i];
            perm[i] = perm[j];
            perm[j] = tmp;
        }
        if (op_perm) memcpy(op_perm, perm, 256);
        if (op_inv_perm) {
            for (int i = 0; i < 256; i++) op_inv_perm[perm[i]] = (uint8_t)i;
        }
        zr_secure_zero(pstream, sizeof(pstream));
        zr_secure_zero(perm, sizeof(perm));
    }
    zr_secure_zero(ks0, sizeof(ks0));
}

static void zr_pbsh_compute_mac16(const uint8_t mac_subkey[32],
                                  const uint8_t hdr84[84],
                                  const uint8_t *ct,
                                  size_t ct_len,
                                  uint8_t out_mac[16]) {
    uint8_t ipad[64], opad[64];
    for (size_t i = 0; i < 64; i++) {
        uint8_t kb = (i < 32u) ? mac_subkey[i] : 0u;
        ipad[i] = kb ^ 0x36u;
        opad[i] = kb ^ 0x5Cu;
    }
    size_t in_len = 64u + 84u + ct_len;
    uint8_t *in_buf = (uint8_t *)zr_xmalloc(in_len);
    memcpy(in_buf, ipad, 64);
    memcpy(in_buf + 64, hdr84, 84);
    if (ct_len > 0) memcpy(in_buf + 64 + 84, ct, ct_len);

    uint8_t inner_digest[32], outer_buf[96], full_mac[32];
    zr_sha256(in_buf, in_len, inner_digest);
    zr_secure_zero(in_buf, 64);
    free(in_buf);

    memcpy(outer_buf, opad, 64);
    memcpy(outer_buf + 64, inner_digest, 32);
    zr_sha256(outer_buf, 96, full_mac);
    memcpy(out_mac, full_mac, 16);

    zr_secure_zero(ipad, sizeof(ipad));
    zr_secure_zero(opad, sizeof(opad));
    zr_secure_zero(inner_digest, sizeof(inner_digest));
    zr_secure_zero(outer_buf, sizeof(outer_buf));
    zr_secure_zero(full_mac, sizeof(full_mac));
}

/* Check for optional trailing PBIPSEAL provenance block ending at `end_pos` */
static size_t zr_pbck_strip_pbipseal_at(const uint8_t *buf, size_t end_pos, bool *out_sealed, char out_mode[32]) {
    if (out_sealed) *out_sealed = false;
    if (out_mode) out_mode[0] = '\0';
    if (!buf || end_pos < 96u) return end_pos;
    if (memcmp(buf + end_pos - 8u, "PBIPEOF!", 8) != 0) return end_pos;
    uint32_t env_total = zr_pbsh_load_u32_le(buf + end_pos - 12u);
    uint32_t seal_rel  = zr_pbsh_load_u32_le(buf + end_pos - 16u);
    if (env_total < 96u || seal_rel < 92u || seal_rel > env_total || (size_t)env_total >= end_pos) {
        return end_pos;
    }
    size_t seal_pos = end_pos - (size_t)seal_rel;
    if (memcmp(buf + seal_pos, "PBIPSEAL", 8) != 0) return end_pos;
    uint32_t json_len = zr_pbsh_load_u32_le(buf + seal_pos + 8u);
    if (seal_pos + 12u + (size_t)json_len + 64u + 16u != end_pos) return end_pos;
    if (out_sealed) *out_sealed = true;
    if (out_mode && json_len > 10u) {
        const char *jstart = (const char *)(buf + seal_pos + 12u);
        const char *jend = jstart + json_len;
        const char *needle = "\"mode\":\"";
        size_t nlen = 8u;
        for (const char *p = jstart; p + nlen < jend; p++) {
            if (memcmp(p, needle, nlen) == 0) {
                const char *v = p + nlen;
                size_t k = 0;
                while (v + k < jend && v[k] != '"' && k < 31u) {
                    out_mode[k] = v[k];
                    k++;
                }
                out_mode[k] = '\0';
                break;
            }
        }
    }
    return end_pos - (size_t)env_total;
}

static PbFunc *pbck_deserialize_script(PbVM *vm, const uint8_t *buf, size_t len, char err_out[256]) {
    if (len > ZR_PBCK_MAX_BYTES) {
        snprintf(err_out, 256, "pbck: artifact byte limit exceeded");
        return NULL;
    }
    size_t eff_len = zr_pbck_strip_pbipseal_at(buf, len, NULL, NULL);

    /* 1. PBSH v1 Polymorphic Encrypted + Stripped + Opcode-Permuted Container */
    if (eff_len >= 4u && memcmp(buf, "PBSH", 4) == 0) {
        if (eff_len < ZR_PBSH_PREFIX_BYTES + 1u) {
            snprintf(err_out, 256, "shield: truncated PBSH container");
            return NULL;
        }
        if (buf[4] != ZR_PBSH_VERSION) {
            snprintf(err_out, 256, "shield: unsupported PBSH version %u (expected v1)", (unsigned)buf[4]);
            return NULL;
        }
        uint8_t flags = buf[5];
        if ((flags & ZR_PBSH_FLAG_ANTI_DEBUG) != 0 && zr_is_debugger_attached()) {
            snprintf(err_out, 256, "shield: execution denied under active debugger/tracer");
            return NULL;
        }

        const uint8_t *nonce   = buf + 8u;
        const uint8_t *share_a = buf + 20u;
        const uint8_t *share_b = buf + 52u;
        const uint8_t *tag_in  = buf + 84u;
        const uint8_t *ct      = buf + ZR_PBSH_PREFIX_BYTES;
        size_t ct_len          = eff_len - ZR_PBSH_PREFIX_BYTES;

        uint8_t key[32], mac_subkey[32], op_inv_perm[256], expected_mac[16];
        zr_pbsh_unseal_key(nonce, share_a, share_b, key);
        zr_pbsh_derive_schedule(key, nonce, mac_subkey, NULL,
                                (flags & ZR_PBSH_FLAG_OPCODE_PERM) ? op_inv_perm : NULL);
        zr_pbsh_compute_mac16(mac_subkey, buf, ct, ct_len, expected_mac);

        volatile uint8_t diff = 0;
        for (size_t i = 0; i < 16u; i++) {
            diff |= (uint8_t)(expected_mac[i] ^ tag_in[i]);
        }
        zr_secure_zero(mac_subkey, sizeof(mac_subkey));
        zr_secure_zero(expected_mac, sizeof(expected_mac));
        if (diff != 0) {
            zr_secure_zero(key, sizeof(key));
            zr_secure_zero(op_inv_perm, sizeof(op_inv_perm));
            snprintf(err_out, 256, "shield: binary integrity verification failed (tampered payload)");
            return NULL;
        }

        uint8_t *plain = (uint8_t *)zr_xmalloc(ct_len);
        memcpy(plain, ct, ct_len);
        if ((flags & ZR_PBSH_FLAG_CHACHA20) != 0) {
            zr_pbsh_chacha20_xor(key, 1u, nonce, plain, ct_len);
        }
        zr_secure_zero(key, sizeof(key));

        ZrPbckReader r;
        r.buf = plain;
        r.len = ct_len;
        r.pos = 0;
        r.op_inv_perm = (flags & ZR_PBSH_FLAG_OPCODE_PERM) ? op_inv_perm : NULL;
        r.err[0] = '\0';

        vm->gc_inhibit++;
        PbFunc *script_fn = pb_func_new(vm);
        script_fn->name = pb_str_c(vm, "<script>");
        vm_write_barrier_obj(vm, (PbObj *)script_fn, (PbObj *)script_fn->name);
        bool ok = zr_pbck_read_chunk_into(&r, vm, script_fn, 0);
        vm->gc_inhibit--;

        size_t final_pos = r.pos;
        zr_secure_zero(plain, ct_len);
        free(plain);
        zr_secure_zero(op_inv_perm, sizeof(op_inv_perm));

        if (!ok) {
            snprintf(err_out, 256, "%s", r.err[0] ? r.err : "shield: malformed inner chunk");
            return NULL;
        }
        if (final_pos != ct_len) {
            snprintf(err_out, 256, "shield: trailing inner bytes");
            return NULL;
        }
        return script_fn;
    }

    /* 2. Standard PBCK v1 Unencrypted Container (100% Backward Compatible) */
    if (eff_len < 5u || memcmp(buf, "PBCK", 4) != 0) {
        snprintf(err_out, 256, "pbck: bad magic (not a PBCK file)");
        return NULL;
    }
    if (buf[4] != ZR_PBCK_VERSION) {
        snprintf(err_out, 256, "pbck: unsupported version %u (expected v1)", (unsigned)buf[4]);
        return NULL;
    }
    ZrPbckReader r;
    r.buf = buf;
    r.len = eff_len;
    r.pos = 5;
    r.op_inv_perm = NULL;
    r.err[0] = '\0';

    vm->gc_inhibit++;
    PbFunc *script_fn = pb_func_new(vm);
    script_fn->name = pb_str_c(vm, "<script>");
    vm_write_barrier_obj(vm, (PbObj *)script_fn, (PbObj *)script_fn->name);
    bool ok = zr_pbck_read_chunk_into(&r, vm, script_fn, 0);
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

/* ---- .pbck v1 & PBSH v1 Binary Serializer ---- */

static inline void zr_pw_u8(ZrBuf *w, uint8_t v) { zr_buf_push_u8(w, v); }

static inline void zr_pw_u32(ZrBuf *w, uint32_t v) {
    uint8_t b[4] = { (uint8_t)(v & 0xFFu), (uint8_t)((v >> 8) & 0xFFu),
                     (uint8_t)((v >> 16) & 0xFFu), (uint8_t)((v >> 24) & 0xFFu) };
    zr_buf_append(w, b, 4);
}

static inline void zr_pw_u64(ZrBuf *w, uint64_t v) {
    uint8_t b[8];
    for (int i = 0; i < 8; i++) b[i] = (uint8_t)((v >> (i * 8)) & 0xFFu);
    zr_buf_append(w, b, 8);
}

static inline void zr_pw_f64(ZrBuf *w, double d) {
    uint64_t bits = 0;
    memcpy(&bits, &d, sizeof(double));
    zr_pw_u64(w, bits);
}

static inline void zr_pw_bytes(ZrBuf *w, const char *s, size_t n) {
    zr_pw_u32(w, (uint32_t)n);
    if (n > 0) zr_buf_append(w, s, n);
}

static void zr_pbck_write_chunk_ex(ZrBuf *w, const PbFunc *fn, const ZrPbckWriteOpts *opts);
static void zr_pbck_write_func_ex(ZrBuf *w, const PbFunc *fn, const ZrPbckWriteOpts *opts);

static void zr_pbck_write_value_ex(ZrBuf *w, PbValue v, const ZrPbckWriteOpts *opts) {
    switch (v.tag) {
        case VAL_NUM:
            zr_pw_u8(w, 0);
            zr_pw_f64(w, v.as.num);
            return;
        case VAL_I64:
            zr_pw_u8(w, 8);
            zr_pw_u64(w, (uint64_t)v.as.i64);
            return;
        case VAL_BOOL:
            zr_pw_u8(w, 1);
            zr_pw_u8(w, v.as.b ? 1 : 0);
            return;
        case VAL_NIL:
            zr_pw_u8(w, 2);
            return;
        case VAL_OBJ:
            switch (v.as.obj->type) {
                case OBJ_STR:
                    zr_pw_u8(w, 3);
                    zr_pw_bytes(w, AS_STR(v)->chars, AS_STR(v)->len);
                    return;
                case OBJ_BIGINT: {
                    char *s = zr_bi_to_str(&AS_BIGINT(v)->v);
                    zr_pw_u8(w, 4);
                    zr_pw_bytes(w, s, strlen(s));
                    free(s);
                    return;
                }
                case OBJ_RATIONAL: {
                    char *s = zr_rat_to_str(&AS_RATIONAL(v)->v);
                    zr_pw_u8(w, 5);
                    zr_pw_bytes(w, s, strlen(s));
                    free(s);
                    return;
                }
                case OBJ_DECIMAL: {
                    char *s = zr_dec_to_str(&AS_DECIMAL(v)->v);
                    zr_pw_u8(w, 6);
                    zr_pw_bytes(w, s, strlen(s));
                    free(s);
                    return;
                }
                case OBJ_FUNC:
                    zr_pw_u8(w, 7);
                    zr_pbck_write_func_ex(w, AS_FUNC(v), opts);
                    return;
                default:
                    zr_pw_u8(w, 2);
                    return;
            }
        default:
            zr_pw_u8(w, 2);
            return;
    }
}

static void zr_pbck_write_opcode_ex(ZrBuf *w, const PbInstr *ins, const ZrPbckWriteOpts *opts) {
    uint8_t op = (ins->op <= OP_THROW) ? ins->op : ins->orig_op;
    uint8_t wire_op = (opts && opts->op_perm) ? opts->op_perm[op] : op;
    zr_pw_u8(w, wire_op);
    switch (op) {
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
            zr_pw_u32(w, ins->arg);
            break;
        case OP_CLOSURE:
            zr_pw_u32(w, ins->arg);
            zr_pw_u32(w, ins->n_caps);
            for (uint32_t i = 0; i < ins->n_caps; i++) {
                zr_pw_u8(w, ins->caps[i].is_local ? 1 : 0);
                zr_pw_u32(w, ins->caps[i].idx);
            }
            break;
        default:
            break;
    }
}

static void zr_pbck_write_func_ex(ZrBuf *w, const PbFunc *fn, const ZrPbckWriteOpts *opts) {
    bool strip = (opts && opts->strip_debug);
    if (!strip && fn->name && fn->name->len > 0) {
        zr_pw_u8(w, 1);
        zr_pw_bytes(w, fn->name->chars, fn->name->len);
    } else {
        zr_pw_u8(w, 0);
    }
    zr_pw_u32(w, fn->arity);
    zr_pw_u32(w, fn->min_arity);
    zr_pw_u32(w, fn->n_defaults);
    for (uint32_t i = 0; i < fn->n_defaults; i++) {
        zr_pbck_write_value_ex(w, fn->defaults[i], opts);
    }
    zr_pw_u8(w, fn->is_vararg ? 1 : 0);
    zr_pw_u8(w, 0); /* has_param_types = 0 */
    zr_pbck_write_chunk_ex(w, fn, opts);
}

static void zr_pbck_write_chunk_ex(ZrBuf *w, const PbFunc *fn, const ZrPbckWriteOpts *opts) {
    bool strip = (opts && opts->strip_debug);
    zr_pw_u32(w, fn->code_len);
    for (uint32_t i = 0; i < fn->code_len; i++) {
        zr_pbck_write_opcode_ex(w, &fn->code[i], opts);
    }
    zr_pw_u32(w, fn->const_len);
    for (uint32_t i = 0; i < fn->const_len; i++) {
        zr_pbck_write_value_ex(w, fn->consts[i], opts);
    }
    if (strip) {
        zr_pw_u32(w, 0); /* 0 debug line spans */
        zr_pw_u8(w, 0);  /* no source_name */
    } else {
        zr_pw_u32(w, fn->code_len);
        for (uint32_t i = 0; i < fn->code_len; i++) {
            zr_pw_u32(w, fn->code[i].line);
            zr_pw_u32(w, fn->code[i].line);
        }
        if (fn->source_name && fn->source_name->len > 0) {
            zr_pw_u8(w, 1);
            zr_pw_bytes(w, fn->source_name->chars, fn->source_name->len);
        } else {
            zr_pw_u8(w, 0);
        }
    }
}

static inline void zr_pbck_write_func(ZrBuf *w, const PbFunc *fn) {
    zr_pbck_write_func_ex(w, fn, NULL);
}

static inline void zr_pbck_write_chunk(ZrBuf *w, const PbFunc *fn) {
    zr_pbck_write_chunk_ex(w, fn, NULL);
}

static uint8_t *pbck_serialize_script(const PbFunc *script_fn, size_t *out_len) {
    ZrBuf w;
    zr_buf_init(&w);
    zr_buf_append(&w, "PBCK", 4);
    zr_pw_u8(&w, ZR_PBCK_VERSION);
    zr_pbck_write_chunk_ex(&w, script_fn, NULL);
    return (uint8_t *)zr_buf_take(&w, out_len);
}

static uint8_t *pbck_serialize_shielded(const PbFunc *script_fn, bool anti_debug, size_t *out_len) {
    uint8_t nonce[12], key[32], share_a[32], share_b[32], mac_subkey[32], op_perm[256], mac[16];
    if (!zr_csprng_bytes(nonce, sizeof(nonce)) ||
        !zr_csprng_bytes(key, sizeof(key)) ||
        !zr_csprng_bytes(share_a, sizeof(share_a))) {
        fputs("fatal: CSPRNG failure in pbck_serialize_shielded\n", stderr);
        abort();
    }
    for (size_t i = 0; i < 32u; i++) {
        uint32_t target = (uint32_t)(key[i] ^ ZR_WB_CONST[i] ^ nonce[i % 12u]);
        share_b[i] = (uint8_t)((target - (uint32_t)share_a[i]) & 0xFFu);
    }

    zr_pbsh_derive_schedule(key, nonce, mac_subkey, op_perm, NULL);

    ZrPbckWriteOpts opts;
    opts.strip_debug = true;
    opts.op_perm = op_perm;

    ZrBuf plain;
    zr_buf_init(&plain);
    zr_pbck_write_chunk_ex(&plain, script_fn, &opts);
    if (!plain.data) zr_buf_reserve(&plain, 1);

    zr_pbsh_chacha20_xor(key, 1u, nonce, (uint8_t *)plain.data, plain.len);

    uint8_t hdr[ZR_PBSH_HEADER_BYTES];
    memcpy(hdr, "PBSH", 4);
    hdr[4] = ZR_PBSH_VERSION;
    hdr[5] = (uint8_t)(ZR_PBSH_FLAG_STRIPPED | ZR_PBSH_FLAG_OPCODE_PERM | ZR_PBSH_FLAG_CHACHA20 |
                       (anti_debug ? ZR_PBSH_FLAG_ANTI_DEBUG : 0u));
    hdr[6] = 0u;
    hdr[7] = 0u;
    memcpy(hdr + 8u,  nonce,   12u);
    memcpy(hdr + 20u, share_a, 32u);
    memcpy(hdr + 52u, share_b, 32u);

    zr_pbsh_compute_mac16(mac_subkey, hdr, (const uint8_t *)plain.data, plain.len, mac);

    size_t total = ZR_PBSH_PREFIX_BYTES + plain.len;
    uint8_t *out = (uint8_t *)zr_xmalloc(total);
    memcpy(out, hdr, ZR_PBSH_HEADER_BYTES);
    memcpy(out + ZR_PBSH_HEADER_BYTES, mac, ZR_PBSH_MAC_BYTES);
    if (plain.len > 0) memcpy(out + ZR_PBSH_PREFIX_BYTES, plain.data, plain.len);

    zr_secure_zero(key, sizeof(key));
    zr_secure_zero(mac_subkey, sizeof(mac_subkey));
    zr_secure_zero(op_perm, sizeof(op_perm));
    if (plain.data && plain.cap > 0) zr_secure_zero(plain.data, plain.cap);
    zr_buf_free(&plain);

    if (out_len) *out_len = total;
    return out;
}

/* ---- Executable Self-Path & PBLZREXE Trailer Bundler ---- */

static char *zr_resolve_self_exe(const char *argv0) {
#ifdef _WIN32
    char buf[MAX_PATH * 4];
    DWORD n = GetModuleFileNameA(NULL, buf, sizeof(buf));
    if (n > 0 && n < sizeof(buf)) return zr_xstrndup(buf, (size_t)n);
#elif !defined(__wasi__)
    char buf[4096];
    ssize_t n = readlink("/proc/self/exe", buf, sizeof(buf) - 1);
    if (n > 0) {
        buf[n] = '\0';
        return zr_xstrdup(buf);
    }
#endif
    return zr_xstrdup(argv0 ? argv0 : "pblang");
}

static bool zr_check_embedded_pbck(const char *exe_path, uint8_t **out_pbck, size_t *out_len) {
    if (!exe_path || !*exe_path) return false;
    FILE *fp = fopen(exe_path, "rb");
    if (!fp) return false;
    if (fseek(fp, 0, SEEK_END) != 0) { fclose(fp); return false; }
    long total_sz = ftell(fp);
    if (total_sz < 21) { fclose(fp); return false; }

    long eff_end = total_sz;

    /* Support PBIPSEAL envelope appended after PBLZREXE (`[host][PBSH|PBCK][len]["PBLZREXE"][PBIPSEAL..."PBIPEOF!"]`) */
    if (total_sz > 120 && fseek(fp, total_sz - 16, SEEK_SET) == 0) {
        uint8_t ip_tail[16];
        if (fread(ip_tail, 1, 16, fp) == 16 && memcmp(ip_tail + 8, "PBIPEOF!", 8) == 0) {
            uint32_t seal_rel  = zr_pbsh_load_u32_le(ip_tail);
            uint32_t env_total = zr_pbsh_load_u32_le(ip_tail + 4);
            if (env_total >= 96u && seal_rel >= 92u && seal_rel <= env_total &&
                (long)env_total < total_sz - 24) {
                long cand_end = total_sz - (long)env_total;
                uint8_t cand_footer[16], seal_hdr[8];
                if (fseek(fp, cand_end - 16, SEEK_SET) == 0 &&
                    fread(cand_footer, 1, 16, fp) == 16 &&
                    memcmp(cand_footer + 8, ZR_BUNDLE_MAGIC, 8) == 0) {
                    if (fseek(fp, total_sz - (long)seal_rel, SEEK_SET) == 0 &&
                        fread(seal_hdr, 1, 8, fp) == 8 &&
                        memcmp(seal_hdr, "PBIPSEAL", 8) == 0) {
                        eff_end = cand_end;
                    } else {
                        fclose(fp);
                        uint8_t *bad = (uint8_t *)zr_xmalloc(4);
                        memcpy(bad, "PBSH", 4);
                        *out_pbck = bad;
                        *out_len = 4;
                        return true;
                    }
                }
            }
        }
    }

    if (fseek(fp, eff_end - 16, SEEK_SET) != 0) { fclose(fp); return false; }
    uint8_t footer[16];
    if (fread(footer, 1, 16, fp) != 16) { fclose(fp); return false; }
    if (memcmp(footer + 8, ZR_BUNDLE_MAGIC, 8) != 0) { fclose(fp); return false; }

    uint64_t pbck_len = 0;
    for (int i = 0; i < 8; i++) pbck_len |= ((uint64_t)footer[i]) << (i * 8);
    if (pbck_len < 5 || pbck_len > ZR_PBCK_MAX_BYTES || pbck_len > (uint64_t)(eff_end - 16)) {
        fclose(fp);
        /* Fail closed on tampered length field in PBLZREXE bundle */
        uint8_t *bad = (uint8_t *)zr_xmalloc(4);
        memcpy(bad, "PBSH", 4);
        *out_pbck = bad;
        *out_len = 4;
        return true;
    }
    long start_off = eff_end - 16 - (long)pbck_len;
    if (fseek(fp, start_off, SEEK_SET) != 0) { fclose(fp); return false; }
    uint8_t *buf = (uint8_t *)zr_xmalloc((size_t)pbck_len);
    if (fread(buf, 1, (size_t)pbck_len, fp) != (size_t)pbck_len) {
        free(buf);
        fclose(fp);
        return false;
    }
    fclose(fp);
    *out_pbck = buf;
    *out_len = (size_t)pbck_len;
    return true;
}

static bool zr_write_bundled_exe(const char *host_exe_path,
                                 const uint8_t *pbck_bytes,
                                 size_t pbck_len,
                                 const char *out_exe_path,
                                 char err_out[256]) {
    FILE *in = fopen(host_exe_path, "rb");
    if (!in) {
        snprintf(err_out, 256, "bundle-exe: cannot open host binary '%s'", host_exe_path);
        return false;
    }
    if (fseek(in, 0, SEEK_END) != 0) { fclose(in); snprintf(err_out, 256, "bundle-exe: seek failed"); return false; }
    long host_len = ftell(in);
    if (host_len < 64) { fclose(in); snprintf(err_out, 256, "bundle-exe: invalid host binary"); return false; }

    /* Strip any pre-existing PBIPSEAL and/or PBLZREXE trailer from host binary */
    for (int pass = 0; pass < 2; pass++) {
        if (host_len > 120 && fseek(in, host_len - 16, SEEK_SET) == 0) {
            uint8_t ip_tail[16];
            if (fread(ip_tail, 1, 16, in) == 16 && memcmp(ip_tail + 8, "PBIPEOF!", 8) == 0) {
                uint32_t env_total = zr_pbsh_load_u32_le(ip_tail + 4);
                if (env_total >= 96u && (long)env_total < host_len - 64) {
                    host_len -= (long)env_total;
                }
            }
        }
        if (host_len > 24 && fseek(in, host_len - 16, SEEK_SET) == 0) {
            uint8_t footer[16];
            if (fread(footer, 1, 16, in) == 16 && memcmp(footer + 8, ZR_BUNDLE_MAGIC, 8) == 0) {
                uint64_t old_len = 0;
                for (int i = 0; i < 8; i++) old_len |= ((uint64_t)footer[i]) << (i * 8);
                if (old_len < (uint64_t)(host_len - 16)) {
                    host_len = host_len - 16 - (long)old_len;
                }
            }
        }
    }
    if (fseek(in, 0, SEEK_SET) != 0) { fclose(in); snprintf(err_out, 256, "bundle-exe: rewind failed"); return false; }
    uint8_t *host_buf = (uint8_t *)zr_xmalloc((size_t)host_len);
    if (fread(host_buf, 1, (size_t)host_len, in) != (size_t)host_len) {
        free(host_buf);
        fclose(in);
        snprintf(err_out, 256, "bundle-exe: read host binary failed");
        return false;
    }
    fclose(in);

    FILE *out = fopen(out_exe_path, "wb");
    if (!out) {
        free(host_buf);
        snprintf(err_out, 256, "bundle-exe: cannot create '%.180s'", out_exe_path);
        return false;
    }
    uint8_t footer[16];
    uint64_t plen64 = (uint64_t)pbck_len;
    for (int i = 0; i < 8; i++) footer[i] = (uint8_t)((plen64 >> (i * 8)) & 0xFFu);
    memcpy(footer + 8, ZR_BUNDLE_MAGIC, 8);

    bool ok = (fwrite(host_buf, 1, (size_t)host_len, out) == (size_t)host_len) &&
              (fwrite(pbck_bytes, 1, pbck_len, out) == pbck_len) &&
              (fwrite(footer, 1, 16, out) == 16);
    free(host_buf);
    fclose(out);
    if (!ok) {
        snprintf(err_out, 256, "bundle-exe: write failed for '%.180s'", out_exe_path);
        return false;
    }
#if !defined(_WIN32) && !defined(__wasi__)
    chmod(out_exe_path, 0755);
#endif
    return true;
}

/* ---- Shield & Entropy Inspector (`pbck_shield_inspect` / `pblang shield-inspect`) ---- */

typedef struct {
    bool   ok;
    bool   shielded;
    char   format[16];
    int    version;
    bool   bundled_exe;
    bool   ip_sealed;
    char   ip_mode[32];
    bool   stripped;
    bool   opcode_permuted;
    bool   encrypted;
    bool   anti_debug;
    bool   integrity_ok;
    size_t artifact_bytes;
    size_t payload_bytes;
    double shannon_entropy;
} ZrShieldInspectInfo;

static double zr_calc_shannon_entropy(const uint8_t *data, size_t n) {
    if (!data || n == 0) return 0.0;
    size_t counts[256];
    size_t nib[16];
    memset(counts, 0, sizeof(counts));
    memset(nib, 0, sizeof(nib));
    for (size_t i = 0; i < n; i++) {
        uint8_t b = data[i];
        counts[b]++;
        nib[b >> 4]++;
        nib[b & 0x0Fu]++;
    }
    double inv_n = 1.0 / (double)n;
    double h8 = 0.0;
    size_t distinct = 0;
    for (int i = 0; i < 256; i++) {
        if (counts[i] > 0) {
            distinct++;
            double p = (double)counts[i] * inv_n;
            h8 -= p * (log(p) / 0.69314718055994530942);
        }
    }
    if (n < 1024u && n >= 16u) {
        double expected_distinct = 256.0 * (1.0 - pow(255.0 / 256.0, (double)n));
        if ((double)distinct >= 0.75 * expected_distinct) {
            double inv_2n = 1.0 / (double)(n * 2u);
            double h4 = 0.0;
            for (int i = 0; i < 16; i++) {
                if (nib[i] > 0) {
                    double p = (double)nib[i] * inv_2n;
                    h4 -= p * (log(p) / 0.69314718055994530942);
                }
            }
            double h4_scaled = 2.0 * h4;
            if (h4_scaled > h8) h8 = h4_scaled > 8.0 ? 8.0 : h4_scaled;
        }
    }
    return h8;
}

static void zr_inspect_shield_bytes(const uint8_t *raw, size_t raw_len, ZrShieldInspectInfo *out) {
    memset(out, 0, sizeof(*out));
    snprintf(out->format, sizeof(out->format), "unknown");
    out->artifact_bytes = raw_len;
    if (!raw || raw_len < 5u) return;

    bool ip_eof = false;
    char mode_buf[32];
    mode_buf[0] = '\0';
    size_t eff_end = zr_pbck_strip_pbipseal_at(raw, raw_len, &ip_eof, mode_buf);
    if (ip_eof) {
        out->ip_sealed = true;
        snprintf(out->ip_mode, sizeof(out->ip_mode), "%s", mode_buf);
    }

    const uint8_t *payload = raw;
    size_t payload_len = eff_end;

    /* Detect PBLZREXE standalone bundled executable */
    if (eff_end >= 24u && memcmp(raw + eff_end - 8u, ZR_BUNDLE_MAGIC, 8) == 0) {
        uint64_t plen = 0;
        for (int i = 0; i < 8; i++) plen |= ((uint64_t)raw[eff_end - 16u + (size_t)i]) << (i * 8);
        if (plen >= 5u && plen <= ZR_PBCK_MAX_BYTES && plen <= (uint64_t)(eff_end - 16u)) {
            size_t split_off = eff_end - 16u - (size_t)plen;
            out->bundled_exe = true;
            payload = raw + split_off;
            payload_len = (size_t)plen;
            if (!out->ip_sealed) {
                bool ip_inner = false;
                (void)zr_pbck_strip_pbipseal_at(raw, split_off, &ip_inner, mode_buf);
                if (ip_inner) {
                    out->ip_sealed = true;
                    snprintf(out->ip_mode, sizeof(out->ip_mode), "%s", mode_buf);
                }
            }
        }
    }

    out->payload_bytes = payload_len;

    if (payload_len >= ZR_PBSH_PREFIX_BYTES + 1u && memcmp(payload, "PBSH", 4) == 0 && payload[4] == ZR_PBSH_VERSION) {
        uint8_t flags = payload[5];
        out->shielded        = true;
        snprintf(out->format, sizeof(out->format), "PBSH-v1");
        out->version         = 1;
        out->stripped        = (flags & ZR_PBSH_FLAG_STRIPPED) != 0;
        out->opcode_permuted = (flags & ZR_PBSH_FLAG_OPCODE_PERM) != 0;
        out->encrypted       = (flags & ZR_PBSH_FLAG_CHACHA20) != 0;
        out->anti_debug      = (flags & ZR_PBSH_FLAG_ANTI_DEBUG) != 0;

        uint8_t key[32], mac_subkey[32], expected_mac[16];
        zr_pbsh_unseal_key(payload + 8u, payload + 20u, payload + 52u, key);
        zr_pbsh_derive_schedule(key, payload + 8u, mac_subkey, NULL, NULL);
        zr_pbsh_compute_mac16(mac_subkey, payload, payload + ZR_PBSH_PREFIX_BYTES,
                              payload_len - ZR_PBSH_PREFIX_BYTES, expected_mac);
        volatile uint8_t diff = 0;
        for (size_t i = 0; i < 16u; i++) diff |= (uint8_t)(expected_mac[i] ^ payload[84u + i]);
        zr_secure_zero(key, sizeof(key));
        zr_secure_zero(mac_subkey, sizeof(mac_subkey));
        zr_secure_zero(expected_mac, sizeof(expected_mac));

        out->integrity_ok    = (diff == 0);
        out->ok              = out->integrity_ok;
        out->shannon_entropy = zr_calc_shannon_entropy(payload + 8u, payload_len - 8u);
        return;
    }

    if (payload_len >= 5u && memcmp(payload, "PBCK", 4) == 0 && payload[4] == ZR_PBCK_VERSION) {
        out->ok              = true;
        out->shielded        = false;
        snprintf(out->format, sizeof(out->format), "PBCK-v1");
        out->version         = 1;
        out->stripped        = false;
        out->opcode_permuted = false;
        out->encrypted       = false;
        out->anti_debug      = false;
        out->integrity_ok    = true;
        out->shannon_entropy = zr_calc_shannon_entropy(payload, payload_len);
        return;
    }

    out->ok              = false;
    out->shielded        = false;
    out->integrity_ok    = false;
    out->shannon_entropy = zr_calc_shannon_entropy(payload, payload_len);
}

#endif /* PBLANG_ZR_PBCK_IO_H */
