/*
 * zr_pbck_io.h — .pbck v1 Binary Serializer / Deserializer & PBLZREXE Standalone Bundler
 */
#ifndef PBLANG_ZR_PBCK_IO_H
#define PBLANG_ZR_PBCK_IO_H

#include "zr_vm_core.h"

#define ZR_PBCK_MAX_BYTES (64u * 1024u * 1024u)
#define ZR_PBCK_VERSION   1u
#define ZR_PBCK_MAX_DEPTH 256u
#define ZR_BUNDLE_MAGIC   "PBLZREXE"

typedef struct {
    const uint8_t *buf;
    size_t         len;
    size_t         pos;
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
    uint8_t tag = 0;
    if (!zr_pr_u8(r, &tag)) return false;
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
    }
    return true;
}

static PbFunc *pbck_deserialize_script(PbVM *vm, const uint8_t *buf, size_t len, char err_out[256]) {
    if (len > ZR_PBCK_MAX_BYTES) {
        snprintf(err_out, 256, "pbck: artifact byte limit exceeded");
        return NULL;
    }
    if (len < 5 || memcmp(buf, "PBCK", 4) != 0) {
        snprintf(err_out, 256, "pbck: bad magic (not a PBCK file)");
        return NULL;
    }
    if (buf[4] != ZR_PBCK_VERSION) {
        snprintf(err_out, 256, "pbck: unsupported version %u (expected v1)", (unsigned)buf[4]);
        return NULL;
    }
    ZrPbckReader r;
    r.buf = buf;
    r.len = len;
    r.pos = 5;
    r.err[0] = '\0';

    vm->gc_inhibit++;
    PbFunc *script_fn = pb_func_new(vm);
    script_fn->name = pb_str_c(vm, "<script>");
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

/* ---- .pbck v1 Binary Serializer ---- */

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

static void zr_pbck_write_chunk(ZrBuf *w, const PbFunc *fn);
static void zr_pbck_write_func(ZrBuf *w, const PbFunc *fn);

static void zr_pbck_write_value(ZrBuf *w, PbValue v) {
    switch (v.tag) {
        case VAL_NUM:
            zr_pw_u8(w, 0);
            zr_pw_f64(w, v.as.num);
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
                    zr_pbck_write_func(w, AS_FUNC(v));
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

static void zr_pbck_write_opcode(ZrBuf *w, const PbInstr *ins) {
    zr_pw_u8(w, ins->op);
    switch (ins->op) {
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

static void zr_pbck_write_func(ZrBuf *w, const PbFunc *fn) {
    if (fn->name && fn->name->len > 0) {
        zr_pw_u8(w, 1);
        zr_pw_bytes(w, fn->name->chars, fn->name->len);
    } else {
        zr_pw_u8(w, 0);
    }
    zr_pw_u32(w, fn->arity);
    zr_pw_u32(w, fn->min_arity);
    zr_pw_u32(w, fn->n_defaults);
    for (uint32_t i = 0; i < fn->n_defaults; i++) {
        zr_pbck_write_value(w, fn->defaults[i]);
    }
    zr_pw_u8(w, fn->is_vararg ? 1 : 0);
    zr_pw_u8(w, 0); /* has_param_types = 0 */
    zr_pbck_write_chunk(w, fn);
}

static void zr_pbck_write_chunk(ZrBuf *w, const PbFunc *fn) {
    zr_pw_u32(w, fn->code_len);
    for (uint32_t i = 0; i < fn->code_len; i++) {
        zr_pbck_write_opcode(w, &fn->code[i]);
    }
    zr_pw_u32(w, fn->const_len);
    for (uint32_t i = 0; i < fn->const_len; i++) {
        zr_pbck_write_value(w, fn->consts[i]);
    }
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

static uint8_t *pbck_serialize_script(const PbFunc *script_fn, size_t *out_len) {
    ZrBuf w;
    zr_buf_init(&w);
    zr_buf_append(&w, "PBCK", 4);
    zr_pw_u8(&w, ZR_PBCK_VERSION);
    zr_pbck_write_chunk(&w, script_fn);
    return (uint8_t *)zr_buf_take(&w, out_len);
}

/* ---- Executable Self-Path & PBLZREXE Trailer Bundler ---- */

static char *zr_resolve_self_exe(const char *argv0) {
#ifdef _WIN32
    char buf[MAX_PATH * 4];
    DWORD n = GetModuleFileNameA(NULL, buf, sizeof(buf));
    if (n > 0 && n < sizeof(buf)) return zr_xstrndup(buf, (size_t)n);
#else
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
    if (fseek(fp, total_sz - 16, SEEK_SET) != 0) { fclose(fp); return false; }
    uint8_t footer[16];
    if (fread(footer, 1, 16, fp) != 16) { fclose(fp); return false; }
    if (memcmp(footer + 8, ZR_BUNDLE_MAGIC, 8) != 0) { fclose(fp); return false; }

    uint64_t pbck_len = 0;
    for (int i = 0; i < 8; i++) pbck_len |= ((uint64_t)footer[i]) << (i * 8);
    if (pbck_len < 5 || pbck_len > ZR_PBCK_MAX_BYTES || pbck_len > (uint64_t)(total_sz - 16)) {
        fclose(fp);
        return false;
    }
    long start_off = total_sz - 16 - (long)pbck_len;
    if (fseek(fp, start_off, SEEK_SET) != 0) { fclose(fp); return false; }
    uint8_t *buf = (uint8_t *)zr_xmalloc((size_t)pbck_len);
    if (fread(buf, 1, (size_t)pbck_len, fp) != (size_t)pbck_len) {
        free(buf);
        fclose(fp);
        return false;
    }
    fclose(fp);
    if (memcmp(buf, "PBCK", 4) != 0) {
        free(buf);
        return false;
    }
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

    /* Strip any pre-existing PBLZREXE trailer from host binary */
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
#ifndef _WIN32
    chmod(out_exe_path, 0755);
#endif
    return true;
}

#endif /* PBLANG_ZR_PBCK_IO_H */
