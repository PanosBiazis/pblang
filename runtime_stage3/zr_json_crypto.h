/* ============================================================================
 * pblang Sovereign Zero-Rust Native Runtime v2.0.0 — zr_json_crypto.h
 * RFC 8259 JSON parser/serializer, SHA-1/256/512, HMAC-SHA256, CSPRNG,
 * Base64, Hex, and RFC 3986 URL percent-encoding builtins.
 * ============================================================================ */
#ifndef PBLANG_ZR_JSON_CRYPTO_H
#define PBLANG_ZR_JSON_CRYPTO_H

#include "zr_vm_core.h"

/* ---- RFC 8259 Recursive-Descent JSON Parser ---- */
typedef struct {
    PbVM       *vm;
    const char *s;
    size_t      len;
    size_t      pos;
    int         depth;
    char        err[256];
} ZrJsonParser;

static void zj_skip_ws(ZrJsonParser *p) {
    while (p->pos < p->len) {
        char c = p->s[p->pos];
        if (c == ' ' || c == '\t' || c == '\r' || c == '\n') p->pos++;
        else break;
    }
}

static int zj_hex4(const char *s) {
    int v = 0;
    for (int i = 0; i < 4; i++) {
        int d = zr_hex_nibble(s[i]);
        if (d < 0) return -1;
        v = (v << 4) | d;
    }
    return v;
}

static bool zj_parse_value(ZrJsonParser *p, PbValue *out);

static PbStr *zj_parse_raw_str(ZrJsonParser *p) {
    if (p->pos >= p->len || p->s[p->pos] != '"') {
        snprintf(p->err, sizeof(p->err), "expected '\"' at offset %zu", p->pos);
        return NULL;
    }
    p->pos++; /* skip '"' */
    ZrBuf b;
    zr_buf_init(&b);
    while (p->pos < p->len) {
        unsigned char c = (unsigned char)p->s[p->pos++];
        if (c == '"') {
            PbStr *res = vm_new_str(p->vm, (const char *)(b.data ? b.data : ""), b.len);
            zr_buf_free(&b);
            return res;
        }
        if (c < 0x20) {
            zr_buf_free(&b);
            snprintf(p->err, sizeof(p->err), "unescaped control character in JSON string");
            return NULL;
        }
        if (c == '\\') {
            if (p->pos >= p->len) break;
            char esc = p->s[p->pos++];
            switch (esc) {
                case '"':  zr_buf_push_u8(&b, '"'); break;
                case '\\': zr_buf_push_u8(&b, '\\'); break;
                case '/':  zr_buf_push_u8(&b, '/'); break;
                case 'b':  zr_buf_push_u8(&b, '\b'); break;
                case 'f':  zr_buf_push_u8(&b, '\f'); break;
                case 'n':  zr_buf_push_u8(&b, '\n'); break;
                case 'r':  zr_buf_push_u8(&b, '\r'); break;
                case 't':  zr_buf_push_u8(&b, '\t'); break;
                case 'u': {
                    if (p->pos + 4 > p->len) {
                        zr_buf_free(&b);
                        snprintf(p->err, sizeof(p->err), "truncated \\u escape in JSON");
                        return NULL;
                    }
                    int u1 = zj_hex4(p->s + p->pos);
                    if (u1 < 0) {
                        zr_buf_free(&b);
                        snprintf(p->err, sizeof(p->err), "invalid \\u hex escape in JSON");
                        return NULL;
                    }
                    p->pos += 4;
                    uint32_t cp = (uint32_t)u1;
                    if (cp >= 0xD800 && cp <= 0xDBFF) {
                        /* UTF-16 surrogate pair */
                        if (p->pos + 6 <= p->len && p->s[p->pos] == '\\' && p->s[p->pos + 1] == 'u') {
                            int u2 = zj_hex4(p->s + p->pos + 2);
                            if (u2 >= 0xDC00 && u2 <= 0xDFFF) {
                                p->pos += 6;
                                cp = 0x10000u + (((cp - 0xD800u) << 10) | ((uint32_t)u2 - 0xDC00u));
                            } else {
                                cp = 0xFFFDu;
                            }
                        } else {
                            cp = 0xFFFDu;
                        }
                    } else if (cp >= 0xDC00 && cp <= 0xDFFF) {
                        cp = 0xFFFDu;
                    }
                    uint8_t u8[4];
                    size_t ulen = zr_utf8_encode(cp, u8);
                    zr_buf_append(&b, u8, ulen);
                    break;
                }
                default:
                    zr_buf_free(&b);
                    snprintf(p->err, sizeof(p->err), "invalid escape '\\%c' in JSON", esc);
                    return NULL;
            }
        } else {
            zr_buf_push_u8(&b, c);
        }
    }
    zr_buf_free(&b);
    snprintf(p->err, sizeof(p->err), "unterminated string in JSON");
    return NULL;
}

static bool zj_parse_value(ZrJsonParser *p, PbValue *out) {
    if (++p->depth > 128) {
        p->depth--;
        snprintf(p->err, sizeof(p->err), "JSON nesting depth exceeds limit (128)");
        return false;
    }
    zj_skip_ws(p);
    if (p->pos >= p->len) {
        p->depth--;
        snprintf(p->err, sizeof(p->err), "unexpected end of JSON input");
        return false;
    }
    char c = p->s[p->pos];
    if (c == 'n' && p->pos + 4 <= p->len && memcmp(p->s + p->pos, "null", 4) == 0) {
        p->pos += 4; *out = val_nil(); p->depth--; return true;
    }
    if (c == 't' && p->pos + 4 <= p->len && memcmp(p->s + p->pos, "true", 4) == 0) {
        p->pos += 4; *out = val_bool(true); p->depth--; return true;
    }
    if (c == 'f' && p->pos + 5 <= p->len && memcmp(p->s + p->pos, "false", 5) == 0) {
        p->pos += 5; *out = val_bool(false); p->depth--; return true;
    }
    if (c == '"') {
        PbStr *s = zj_parse_raw_str(p);
        p->depth--;
        if (!s) return false;
        *out = val_obj(s);
        return true;
    }
    if (c == '[') {
        p->pos++;
        PbArray *arr = vm_new_array(p->vm, 4);
        vm_temp_push(p->vm, val_obj(arr));
        zj_skip_ws(p);
        if (p->pos < p->len && p->s[p->pos] == ']') {
            p->pos++;
            vm_temp_pop(p->vm, 1);
            *out = val_obj(arr);
            p->depth--;
            return true;
        }
        for (;;) {
            PbValue item;
            if (!zj_parse_value(p, &item)) { vm_temp_pop(p->vm, 1); p->depth--; return false; }
            arr_push(arr, item);
            zj_skip_ws(p);
            if (p->pos < p->len && p->s[p->pos] == ',') { p->pos++; continue; }
            if (p->pos < p->len && p->s[p->pos] == ']') { p->pos++; break; }
            vm_temp_pop(p->vm, 1);
            p->depth--;
            snprintf(p->err, sizeof(p->err), "expected ',' or ']' in JSON array at offset %zu", p->pos);
            return false;
        }
        vm_temp_pop(p->vm, 1);
        *out = val_obj(arr);
        p->depth--;
        return true;
    }
    if (c == '{') {
        p->pos++;
        PbMap *m = vm_new_map(p->vm, 8);
        vm_temp_push(p->vm, val_obj(m));
        zj_skip_ws(p);
        if (p->pos < p->len && p->s[p->pos] == '}') {
            p->pos++;
            vm_temp_pop(p->vm, 1);
            *out = val_obj(m);
            p->depth--;
            return true;
        }
        for (;;) {
            zj_skip_ws(p);
            PbStr *k = zj_parse_raw_str(p);
            if (!k) { vm_temp_pop(p->vm, 1); p->depth--; return false; }
            vm_temp_push(p->vm, val_obj(k));
            zj_skip_ws(p);
            if (p->pos >= p->len || p->s[p->pos] != ':') {
                vm_temp_pop(p->vm, 2);
                p->depth--;
                snprintf(p->err, sizeof(p->err), "expected ':' in JSON object at offset %zu", p->pos);
                return false;
            }
            p->pos++;
            PbValue v;
            if (!zj_parse_value(p, &v)) { vm_temp_pop(p->vm, 2); p->depth--; return false; }
            map_set(m, k, v);
            vm_temp_pop(p->vm, 1); /* k */
            zj_skip_ws(p);
            if (p->pos < p->len && p->s[p->pos] == ',') { p->pos++; continue; }
            if (p->pos < p->len && p->s[p->pos] == '}') { p->pos++; break; }
            vm_temp_pop(p->vm, 1);
            p->depth--;
            snprintf(p->err, sizeof(p->err), "expected ',' or '}' in JSON object at offset %zu", p->pos);
            return false;
        }
        vm_temp_pop(p->vm, 1);
        *out = val_obj(m);
        p->depth--;
        return true;
    }
    if (c == '-' || (c >= '0' && c <= '9')) {
        size_t start = p->pos;
        if (p->s[p->pos] == '-') p->pos++;
        if (p->pos >= p->len || p->s[p->pos] < '0' || p->s[p->pos] > '9') {
            p->depth--;
            snprintf(p->err, sizeof(p->err), "invalid number in JSON at offset %zu", start);
            return false;
        }
        if (p->s[p->pos] == '0') {
            p->pos++;
        } else {
            while (p->pos < p->len && p->s[p->pos] >= '0' && p->s[p->pos] <= '9') p->pos++;
        }
        if (p->pos < p->len && p->s[p->pos] == '.') {
            p->pos++;
            if (p->pos >= p->len || p->s[p->pos] < '0' || p->s[p->pos] > '9') {
                p->depth--;
                snprintf(p->err, sizeof(p->err), "invalid fraction in JSON number");
                return false;
            }
            while (p->pos < p->len && p->s[p->pos] >= '0' && p->s[p->pos] <= '9') p->pos++;
        }
        if (p->pos < p->len && (p->s[p->pos] == 'e' || p->s[p->pos] == 'E')) {
            p->pos++;
            if (p->pos < p->len && (p->s[p->pos] == '+' || p->s[p->pos] == '-')) p->pos++;
            if (p->pos >= p->len || p->s[p->pos] < '0' || p->s[p->pos] > '9') {
                p->depth--;
                snprintf(p->err, sizeof(p->err), "invalid exponent in JSON number");
                return false;
            }
            while (p->pos < p->len && p->s[p->pos] >= '0' && p->s[p->pos] <= '9') p->pos++;
        }
        size_t nlen = p->pos - start;
        char nbuf[128];
        if (nlen >= sizeof(nbuf)) {
            p->depth--;
            snprintf(p->err, sizeof(p->err), "JSON number literal too long");
            return false;
        }
        memcpy(nbuf, p->s + start, nlen);
        nbuf[nlen] = '\0';
        char *endp = NULL;
        double d = strtod(nbuf, &endp);
        if (!isfinite(d)) {
            p->depth--;
            snprintf(p->err, sizeof(p->err), "JSON number out of finite f64 range");
            return false;
        }
        *out = val_num(d);
        p->depth--;
        return true;
    }

    p->depth--;
    snprintf(p->err, sizeof(p->err), "unexpected character '%c' in JSON at offset %zu", c, p->pos);
    return false;
}

static bool zr_json_parse_str(PbVM *vm, const char *s, size_t len, PbValue *out, char err_out[256]) {
    ZrJsonParser p;
    p.vm = vm; p.s = s; p.len = len; p.pos = 0; p.depth = 0; p.err[0] = '\0';
    if (!zj_parse_value(&p, out)) {
        snprintf(err_out, 256, "%s", p.err);
        return false;
    }
    zj_skip_ws(&p);
    if (p.pos != p.len) {
        snprintf(err_out, 256, "trailing characters after JSON value at offset %zu", p.pos);
        return false;
    }
    return true;
}

/* ---- RFC 8259 JSON Serializer ---- */
static void zj_append_escaped_str(ZrBuf *b, const char *s, size_t len) {
    zr_buf_push_u8(b, '"');
    for (size_t i = 0; i < len; i++) {
        unsigned char c = (unsigned char)s[i];
        switch (c) {
            case '"':  zr_buf_append_cstr(b, "\\\""); break;
            case '\\': zr_buf_append_cstr(b, "\\\\"); break;
            case '\b': zr_buf_append_cstr(b, "\\b"); break;
            case '\f': zr_buf_append_cstr(b, "\\f"); break;
            case '\n': zr_buf_append_cstr(b, "\\n"); break;
            case '\r': zr_buf_append_cstr(b, "\\r"); break;
            case '\t': zr_buf_append_cstr(b, "\\t"); break;
            default:
                if (c < 0x20) {
                    char u[8];
                    snprintf(u, sizeof(u), "\\u%04x", (unsigned)c);
                    zr_buf_append_cstr(b, u);
                } else {
                    zr_buf_push_u8(b, c);
                }
                break;
        }
    }
    zr_buf_push_u8(b, '"');
}

static void zr_sort_map_entries(PbMapEntry **ents, size_t n) {
    if (n < 2) return;
    size_t gap = n >> 1;
    while (gap > 0) {
        for (size_t i = gap; i < n; i++) {
            PbMapEntry *tmp = ents[i];
            size_t j = i;
            while (j >= gap && zr_cmp_pbstr_ptr(&ents[j - gap]->key, &tmp->key) > 0) {
                ents[j] = ents[j - gap];
                j -= gap;
            }
            ents[j] = tmp;
        }
        gap >>= 1;
    }
}

static bool zr_json_stringify_into(PbVM *vm, PbValue v, ZrBuf *b, int depth) {
    if (depth > 256) {
        vm_raise(vm, "json_stringify(): maximum nesting depth (256) exceeded (cyclic value?)");
        return false;
    }
    switch (v.tag) {
        case VAL_NIL:
            zr_buf_append_cstr(b, "null");
            return true;
        case VAL_BOOL:
            zr_buf_append_cstr(b, v.u.b ? "true" : "false");
            return true;
        case VAL_NUM: {
            if (!isfinite(v.u.num)) {
                vm_raise(vm, "json_stringify(): cannot serialize NaN or Infinity to JSON");
                return false;
            }
            char nb[64];
            double n = (v.u.num == 0.0) ? 0.0 : v.u.num;
            if (floor(n) == n && n >= -9007199254740991.0 && n <= 9007199254740991.0) {
                snprintf(nb, sizeof(nb), "%lld", (long long)n);
            } else {
                snprintf(nb, sizeof(nb), "%.17g", n);
                double rt = strtod(nb, NULL);
                char nb15[64];
                snprintf(nb15, sizeof(nb15), "%.15g", n);
                if (strtod(nb15, NULL) == rt) snprintf(nb, sizeof(nb), "%s", nb15);
            }
            zr_buf_append_cstr(b, nb);
            return true;
        }
        case VAL_OBJ: {
            PbObj *o = v.u.obj;
            switch (o->type) {
                case OBJ_STR: {
                    PbStr *s = (PbStr *)o;
                    zj_append_escaped_str(b, s->chars, s->len);
                    return true;
                }
                case OBJ_BIGINT: {
                    char *s = zr_bi_to_str(&((PbBigInt *)o)->v);
                    zr_buf_append_cstr(b, s);
                    free(s);
                    return true;
                }
                case OBJ_DECIMAL: {
                    char *s = zr_dec_to_str(&((PbDecimal *)o)->v);
                    zr_buf_append_cstr(b, s);
                    free(s);
                    return true;
                }
                case OBJ_RATIONAL: {
                    char *s = zr_rat_to_str(&((PbRational *)o)->v);
                    zj_append_escaped_str(b, s, strlen(s));
                    free(s);
                    return true;
                }
                case OBJ_ARRAY: {
                    PbArray *a = (PbArray *)o;
                    zr_buf_push_u8(b, '[');
                    for (size_t i = 0; i < a->len; i++) {
                        if (i > 0) zr_buf_push_u8(b, ',');
                        if (!zr_json_stringify_into(vm, a->items[i], b, depth + 1)) return false;
                    }
                    zr_buf_push_u8(b, ']');
                    return true;
                }
                case OBJ_MAP: {
                    PbMap *m = (PbMap *)o;
                    zr_buf_push_u8(b, '{');
                    if (m->count > 0) {
                        PbMapEntry **ents = (PbMapEntry **)zr_xmalloc(m->count * sizeof(PbMapEntry *));
                        size_t k = 0;
                        for (size_t i = 0; i < m->used; i++) {
                            if (m->entries[i].active && m->entries[i].key) ents[k++] = &m->entries[i];
                        }
                        zr_sort_map_entries(ents, k);
                        for (size_t i = 0; i < k; i++) {
                            if (i > 0) zr_buf_push_u8(b, ',');
                            zj_append_escaped_str(b, ents[i]->key->chars, ents[i]->key->len);
                            zr_buf_push_u8(b, ':');
                            if (!zr_json_stringify_into(vm, ents[i]->val, b, depth + 1)) {
                                free(ents);
                                return false;
                            }
                        }
                        free(ents);
                    }
                    zr_buf_push_u8(b, '}');
                    return true;
                }
                default:
                    vm_raise(vm, "json_stringify(): unsupported value type '%s'", vm_type_name(v));
                    return false;
            }
        }
        default:
            vm_raise(vm, "json_stringify(): unsupported value type '%s'", vm_type_name(v));
            return false;
    }
}

static bool bi_json_parse(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 1 || !val_is_obj(argv[0], OBJ_STR)) {
        vm_raise(vm, "json_parse() expects 1 string argument");
        return false;
    }
    PbStr *s = (PbStr *)val_as_obj(argv[0]);
    char err[256];
    if (!zr_json_parse_str(vm, s->chars, s->len, out, err)) {
        vm_raise(vm, "json_parse(): %s", err);
        return false;
    }
    return true;
}

static bool bi_json_stringify(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 1) {
        vm_raise(vm, "json_stringify() expects 1 argument, got %d", argc);
        return false;
    }
    ZrBuf b;
    zr_buf_init(&b);
    if (!zr_json_stringify_into(vm, argv[0], &b, 0)) {
        zr_buf_free(&b);
        return false;
    }
    PbStr *res = vm_new_str(vm, b.data ? b.data : "", b.len);
    zr_buf_free(&b);
    uint8_t taint = zr_collect_value_taint(argv[0], 0);
    *out = val_with_taint(val_obj(res), taint);
    return true;
}

/* ---- Crypto & Encoding Builtins ---- */
static bool bi_sha1(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 1 || !val_is_obj(argv[0], OBJ_STR)) { vm_raise(vm, "sha1() expects 1 string argument"); return false; }
    PbStr *s = (PbStr *)val_as_obj(argv[0]);
    uint8_t d[20];
    zr_sha1_oneshot(s->chars, s->len, d);
    char hex[41];
    zr_hex_encode_raw(d, 20, hex);
    *out = val_obj(vm_new_str(vm, hex, 40));
    return true;
}

static bool bi_sha256(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 1 || !val_is_obj(argv[0], OBJ_STR)) { vm_raise(vm, "sha256() expects 1 string argument"); return false; }
    PbStr *s = (PbStr *)val_as_obj(argv[0]);
    uint8_t d[32];
    zr_sha256_oneshot(s->chars, s->len, d);
    char hex[65];
    zr_hex_encode_raw(d, 32, hex);
    *out = val_obj(vm_new_str(vm, hex, 64));
    return true;
}

static bool bi_sha512(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 1 || !val_is_obj(argv[0], OBJ_STR)) { vm_raise(vm, "sha512() expects 1 string argument"); return false; }
    PbStr *s = (PbStr *)val_as_obj(argv[0]);
    uint8_t d[64];
    zr_sha512_oneshot(s->chars, s->len, d);
    char hex[129];
    zr_hex_encode_raw(d, 64, hex);
    *out = val_obj(vm_new_str(vm, hex, 128));
    return true;
}

static bool bi_hmac_sha256(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 2 || !val_is_obj(argv[0], OBJ_STR) || !val_is_obj(argv[1], OBJ_STR)) {
        vm_raise(vm, "hmac_sha256(data, key) expects 2 string arguments");
        return false;
    }
    PbStr *msg = (PbStr *)val_as_obj(argv[0]);
    PbStr *key = (PbStr *)val_as_obj(argv[1]);
    uint8_t mac[32];
    zr_hmac_sha256((const uint8_t *)key->chars, key->len, (const uint8_t *)msg->chars, msg->len, mac);
    char hex[65];
    zr_hex_encode_raw(mac, 32, hex);
    *out = val_obj(vm_new_str(vm, hex, 64));
    return true;
}

static bool bi_constant_time_eq(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 2 || !val_is_obj(argv[0], OBJ_STR) || !val_is_obj(argv[1], OBJ_STR)) {
        vm_raise(vm, "constant_time_eq(a, b) expects 2 string arguments");
        return false;
    }
    PbStr *a = (PbStr *)val_as_obj(argv[0]);
    PbStr *b = (PbStr *)val_as_obj(argv[1]);
    size_t max_len = a->len > b->len ? a->len : b->len;
    volatile uint8_t diff = (a->len == b->len) ? 0u : 1u;
    for (size_t i = 0; i < max_len; i++) {
        uint8_t ba = (i < a->len) ? (uint8_t)a->chars[i] : 0u;
        uint8_t bb = (i < b->len) ? (uint8_t)b->chars[i] : 0u;
        diff |= (ba ^ bb);
    }
    *out = val_bool(diff == 0u);
    return true;
}

static bool bi_random_bytes(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 1 || argv[0].tag != VAL_NUM || !isfinite(argv[0].u.num) || argv[0].u.num < 0.0 || argv[0].u.num > 16777216.0) {
        vm_raise(vm, "random_bytes(n) expects byte count 0..16777216");
        return false;
    }
    size_t n = (size_t)argv[0].u.num;
    if (vm->flight_replaying) {
        const ZrFlightEvent *ev = NULL;
        if (!zr_flight_replay_next(vm, "random_bytes", &ev)) return false;
        const char *hx = ev->str_val ? ev->str_val : "";
        size_t nb = ev->str_len / 2;
        PbArray *arr = vm_new_array(vm, nb);
        for (size_t i = 0; i < nb; i++) {
            int hi = zr_hex_nibble(hx[i * 2]);
            int lo = zr_hex_nibble(hx[i * 2 + 1]);
            uint8_t b = (uint8_t)(((hi >= 0 ? hi : 0) << 4) | (lo >= 0 ? lo : 0));
            arr_push(arr, val_num((double)b));
        }
        *out = val_obj(arr);
        return true;
    }
    uint8_t *raw = (uint8_t *)zr_xmalloc(n > 0 ? n : 1);
    if (n > 0 && !zr_csprng_bytes(raw, n)) {
        free(raw);
        vm_raise(vm, "random_bytes(): OS CSPRNG failed");
        return false;
    }
    if (vm->flight_recording) {
        char *hex = (char *)zr_xmalloc(n * 2 + 1);
        zr_hex_encode_raw(raw, n, hex);
        zr_flight_record_str(vm, "random_bytes", "bytes", hex, n * 2);
        free(hex);
    }
    PbArray *arr = vm_new_array(vm, n);
    for (size_t i = 0; i < n; i++) arr_push(arr, val_num((double)raw[i]));
    free(raw);
    *out = val_obj(arr);
    return true;
}

static bool bi_random_float(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    (void)argv;
    if (argc != 0) { vm_raise(vm, "random_float() takes no arguments"); return false; }
    if (vm->flight_replaying) {
        const ZrFlightEvent *ev = NULL;
        if (!zr_flight_replay_next(vm, "random_float", &ev)) return false;
        *out = val_num(ev->num_val);
        return true;
    }
    uint8_t b[8];
    if (!zr_csprng_bytes(b, 8)) { vm_raise(vm, "random_float(): OS CSPRNG failed"); return false; }
    uint64_t u = 0;
    for (int i = 0; i < 8; i++) u = (u << 8) | b[i];
    double d = (double)(u >> 11) * (1.0 / 9007199254740992.0);
    if (vm->flight_recording) {
        zr_flight_record_num(vm, "random_float", d);
    }
    *out = val_num(d);
    return true;
}

static bool bi_csprng_hex(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    size_t n = 16;
    if (argc >= 1) {
        if (argc > 1 || argv[0].tag != VAL_NUM || !isfinite(argv[0].u.num) || argv[0].u.num < 0.0 || argv[0].u.num > 1048576.0) {
            vm_raise(vm, "csprng_hex([n_bytes]) expects byte count 0..1048576");
            return false;
        }
        n = (size_t)argv[0].u.num;
    }
    if (vm->flight_replaying) {
        const ZrFlightEvent *ev = NULL;
        if (!zr_flight_replay_next(vm, "csprng_hex", &ev)) return false;
        *out = val_obj(vm_new_str(vm, ev->str_val ? ev->str_val : "", ev->str_len));
        return true;
    }
    uint8_t *raw = (uint8_t *)zr_xmalloc(n > 0 ? n : 1);
    if (n > 0 && !zr_csprng_bytes(raw, n)) {
        free(raw);
        vm_raise(vm, "csprng_hex(): OS CSPRNG failed");
        return false;
    }
    char *hex = (char *)zr_xmalloc(n * 2 + 1);
    zr_hex_encode_raw(raw, n, hex);
    free(raw);
    if (vm->flight_recording) {
        zr_flight_record_str(vm, "csprng_hex", "str", hex, n * 2);
    }
    *out = val_obj(vm_new_str(vm, hex, n * 2));
    free(hex);
    return true;
}

static bool bi_base64_encode(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 1 || !val_is_obj(argv[0], OBJ_STR)) { vm_raise(vm, "base64_encode() expects 1 string argument"); return false; }
    PbStr *s = (PbStr *)val_as_obj(argv[0]);
    size_t elen = 0;
    char *enc = zr_base64_encode((const uint8_t *)s->chars, s->len, &elen);
    *out = val_with_taint(val_obj(vm_new_str(vm, enc, elen)), argv[0].taint);
    free(enc);
    return true;
}

static bool bi_base64_encode_bytes(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc == 1 && val_is_obj(argv[0], OBJ_BYTES)) {
        PbBytes *b = (PbBytes *)val_as_obj(argv[0]);
        size_t elen = 0;
        char *enc = zr_base64_encode(b->data, b->len, &elen);
        *out = val_with_taint(val_obj(vm_new_str(vm, enc, elen)), argv[0].taint);
        free(enc);
        return true;
    }
    if (argc != 1 || !val_is_obj(argv[0], OBJ_ARRAY)) {
        vm_raise(vm, "base64_encode_bytes() expects 1 byte-array or bytes argument");
        return false;
    }
    PbArray *a = (PbArray *)val_as_obj(argv[0]);
    uint8_t *bytes = (uint8_t *)zr_xmalloc(a->len > 0 ? a->len : 1);
    uint8_t taint = argv[0].taint;
    for (size_t i = 0; i < a->len; i++) {
        PbValue v = a->items[i];
        if (v.tag != VAL_NUM || !isfinite(v.u.num) || floor(v.u.num) != v.u.num || v.u.num < 0.0 || v.u.num > 255.0) {
            free(bytes);
            vm_raise(vm, "base64_encode_bytes(): expected byte numbers 0..255");
            return false;
        }
        taint |= v.taint;
        bytes[i] = (uint8_t)v.u.num;
    }
    size_t elen = 0;
    char *enc = zr_base64_encode(bytes, a->len, &elen);
    free(bytes);
    *out = val_with_taint(val_obj(vm_new_str(vm, enc, elen)), taint);
    free(enc);
    return true;
}

static bool bi_base64_decode(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 1 || !val_is_obj(argv[0], OBJ_STR)) { vm_raise(vm, "base64_decode() expects 1 string argument"); return false; }
    PbStr *s = (PbStr *)val_as_obj(argv[0]);
    uint8_t *dec = NULL; size_t dlen = 0;
    if (!zr_base64_decode(s->chars, s->len, &dec, &dlen)) {
        vm_raise(vm, "base64_decode(): invalid base64 payload");
        return false;
    }
    *out = val_with_taint(val_obj(vm_new_str(vm, (const char *)dec, dlen)), argv[0].taint);
    free(dec);
    return true;
}

static bool bi_base64_decode_bytes(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 1 || !val_is_obj(argv[0], OBJ_STR)) { vm_raise(vm, "base64_decode_bytes() expects 1 string argument"); return false; }
    PbStr *s = (PbStr *)val_as_obj(argv[0]);
    uint8_t *dec = NULL; size_t dlen = 0;
    if (!zr_base64_decode(s->chars, s->len, &dec, &dlen)) {
        vm_raise(vm, "base64_decode_bytes(): invalid base64 payload");
        return false;
    }
    PbArray *arr = vm_new_array(vm, dlen);
    for (size_t i = 0; i < dlen; i++) arr_push(arr, val_num((double)dec[i]));
    free(dec);
    *out = val_with_taint(val_obj(arr), argv[0].taint);
    return true;
}

static bool bi_hex_encode(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 1) { vm_raise(vm, "hex_encode() expects 1 argument"); return false; }
    if (val_is_obj(argv[0], OBJ_STR)) {
        PbStr *s = (PbStr *)val_as_obj(argv[0]);
        char *hex = (char *)zr_xmalloc(s->len * 2 + 1);
        zr_hex_encode_raw((const uint8_t *)s->chars, s->len, hex);
        *out = val_with_taint(val_obj(vm_new_str(vm, hex, s->len * 2)), argv[0].taint);
        free(hex);
        return true;
    }
    if (val_is_obj(argv[0], OBJ_BYTES)) {
        PbBytes *b = (PbBytes *)val_as_obj(argv[0]);
        char *hex = (char *)zr_xmalloc(b->len * 2 + 1);
        zr_hex_encode_raw(b->data, b->len, hex);
        *out = val_with_taint(val_obj(vm_new_str(vm, hex, b->len * 2)), argv[0].taint);
        free(hex);
        return true;
    }
    if (val_is_obj(argv[0], OBJ_ARRAY)) {
        PbArray *a = (PbArray *)val_as_obj(argv[0]);
        uint8_t *raw = (uint8_t *)zr_xmalloc(a->len > 0 ? a->len : 1);
        uint8_t taint = argv[0].taint;
        for (size_t i = 0; i < a->len; i++) {
            PbValue v = a->items[i];
            if (v.tag != VAL_NUM || v.u.num < 0.0 || v.u.num > 255.0) {
                free(raw);
                vm_raise(vm, "hex_encode(): byte array elements must be 0..255");
                return false;
            }
            taint |= v.taint;
            raw[i] = (uint8_t)v.u.num;
        }
        char *hex = (char *)zr_xmalloc(a->len * 2 + 1);
        zr_hex_encode_raw(raw, a->len, hex);
        free(raw);
        *out = val_with_taint(val_obj(vm_new_str(vm, hex, a->len * 2)), taint);
        free(hex);
        return true;
    }
    vm_raise(vm, "hex_encode() expects string, bytes, or byte array");
    return false;
}

static bool bi_hex_decode(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 1 || !val_is_obj(argv[0], OBJ_STR)) { vm_raise(vm, "hex_decode() expects 1 string argument"); return false; }
    PbStr *s = (PbStr *)val_as_obj(argv[0]);
    const char *p = s->chars;
    size_t len = s->len;
    while (len > 0 && (*p == ' ' || *p == '\t' || *p == '\r' || *p == '\n')) { p++; len--; }
    while (len > 0 && (p[len - 1] == ' ' || p[len - 1] == '\t' || p[len - 1] == '\r' || p[len - 1] == '\n')) len--;
    if (len % 2 != 0) { vm_raise(vm, "hex_decode(): odd-length hex string"); return false; }
    size_t blen = len / 2;
    char *raw = (char *)zr_xmalloc(blen + 1);
    for (size_t i = 0; i < blen; i++) {
        int hi = zr_hex_nibble(p[i * 2]);
        int lo = zr_hex_nibble(p[i * 2 + 1]);
        if (hi < 0 || lo < 0) {
            free(raw);
            vm_raise(vm, "hex_decode(): invalid hex digit");
            return false;
        }
        raw[i] = (char)((hi << 4) | lo);
    }
    raw[blen] = '\0';
    *out = val_obj(vm_new_str(vm, raw, blen));
    free(raw);
    return true;
}

static bool bi_url_encode(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 1 || !val_is_obj(argv[0], OBJ_STR)) { vm_raise(vm, "url_encode() expects 1 string argument"); return false; }
    PbStr *s = (PbStr *)val_as_obj(argv[0]);
    ZrBuf b;
    zr_buf_init(&b);
    for (size_t i = 0; i < s->len; i++) {
        unsigned char c = (unsigned char)s->chars[i];
        if ((c >= 'A' && c <= 'Z') || (c >= 'a' && c <= 'z') || (c >= '0' && c <= '9') ||
            c == '-' || c == '_' || c == '.' || c == '~') {
            zr_buf_push_u8(&b, c);
        } else {
            char tmp[4];
            snprintf(tmp, sizeof(tmp), "%%%02X", (unsigned)c);
            zr_buf_append(&b, tmp, 3);
        }
    }
    *out = val_obj(vm_new_str(vm, (const char *)(b.data ? b.data : ""), b.len));
    zr_buf_free(&b);
    return true;
}

static bool bi_url_decode(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 1 || !val_is_obj(argv[0], OBJ_STR)) { vm_raise(vm, "url_decode() expects 1 string argument"); return false; }
    PbStr *s = (PbStr *)val_as_obj(argv[0]);
    ZrBuf b;
    zr_buf_init(&b);
    for (size_t i = 0; i < s->len; i++) {
        char c = s->chars[i];
        if (c == '+') {
            zr_buf_push_u8(&b, ' ');
        } else if (c == '%') {
            if (i + 2 >= s->len) { zr_buf_free(&b); vm_raise(vm, "url_decode(): truncated escape sequence"); return false; }
            int hi = zr_hex_nibble(s->chars[i + 1]);
            int lo = zr_hex_nibble(s->chars[i + 2]);
            if (hi < 0 || lo < 0) { zr_buf_free(&b); vm_raise(vm, "url_decode(): invalid hex escape"); return false; }
            zr_buf_push_u8(&b, (uint8_t)((hi << 4) | lo));
            i += 2;
        } else {
            zr_buf_push_u8(&b, (uint8_t)c);
        }
    }
    *out = val_obj(vm_new_str(vm, (const char *)(b.data ? b.data : ""), b.len));
    zr_buf_free(&b);
    return true;
}

#endif /* PBLANG_ZR_JSON_CRYPTO_H */
