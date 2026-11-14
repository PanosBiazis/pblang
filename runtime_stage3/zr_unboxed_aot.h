/* ============================================================================
 * pblang Sovereign Zero-Rust Native Runtime v4.0.0 — zr_unboxed_aot.h
 *
 * 1. Unboxed Contiguous Packed Struct Tables (`OBJ_STRUCT_BUF`):
 *      - Zero 16-byte `PbValue` boxing: `N` records stored in a single
 *        contiguous `stride * cap` byte slab (`i64`, `f64`, `i32`, `u8` fields)
 *      - Reentrant thread-safe in-place C99 Introsort (`struct_sort_by`)
 *      - Vectorized reductions (`struct_reduce_f64`: sum, min, max, mean, dot)
 * 2. Unboxed C-Speed CSR Graph Kernels:
 *      - `graph_bfs_shortest_path`: flat `uint32_t` ring-queue BFS
 *      - `graph_dijkstra_f64`: contiguous 4-ary min-heap Dijkstra
 * 3. Native Z-Buffered 3D Cook-Torrance PBR Triangle Rasterizer:
 *      - `gfx3d_raster_triangles`: perspective-correct depth Z-buffer +
 *        GGX/Schlick-Fresnel metallic-roughness shading into `OBJ_BYTES` RGBA
 * 4. Pure-PBLang Native Unboxed AOT Bridge (`aot_build_native` -> `.sapp`)
 *
 * Strictly portable C99 (POSIX Linux/macOS + Windows Win32/MinGW-w64).
 * Public-safe: zero personal paths, zero host-specific identifiers.
 * ============================================================================ */
#ifndef PBLANG_ZR_UNBOXED_AOT_H
#define PBLANG_ZR_UNBOXED_AOT_H

#if defined(__GNUC__) && !defined(__clang__) && !defined(_WIN32)
#pragma GCC push_options
#pragma GCC optimize("O3,unroll-loops,tree-vectorize")
#endif

/* ============================================================================
 * 1. Unboxed Contiguous Packed Struct Tables (`OBJ_STRUCT_BUF`)
 * ============================================================================ */

static inline PbStr *zr_ub_str_cstr(PbVM *vm, const char *s) {
    return pb_str_new(vm, s ? s : "", s ? strlen(s) : 0);
}

static inline bool zr_ub_req_num(PbVM *vm, const char *fn, PbValue v, double *out) {
    if (v.tag == VAL_NUM) {
        *out = v.as.num;
        return true;
    }
    return vm_error(vm, "%s(): expected number, got %s", fn, zr_val_type_name(v));
}

static bool zr_parse_sfield_type(const char *tstr, uint8_t *out_kind, uint8_t *out_size) {
    if (!tstr) return false;
    while (*tstr == ' ' || *tstr == '\t') tstr++;
    if (strcmp(tstr, "i64") == 0 || strcmp(tstr, "int") == 0 || strcmp(tstr, "int64") == 0 || strcmp(tstr, "u64") == 0) {
        *out_kind = ZR_SFIELD_I64; *out_size = 8; return true;
    }
    if (strcmp(tstr, "f64") == 0 || strcmp(tstr, "num") == 0 || strcmp(tstr, "number") == 0 ||
        strcmp(tstr, "float") == 0 || strcmp(tstr, "double") == 0) {
        *out_kind = ZR_SFIELD_F64; *out_size = 8; return true;
    }
    if (strcmp(tstr, "i32") == 0 || strcmp(tstr, "int32") == 0 || strcmp(tstr, "u32") == 0) {
        *out_kind = ZR_SFIELD_I32; *out_size = 4; return true;
    }
    if (strcmp(tstr, "u8") == 0 || strcmp(tstr, "byte") == 0 || strcmp(tstr, "bool") == 0 || strcmp(tstr, "i8") == 0) {
        *out_kind = ZR_SFIELD_U8; *out_size = 1; return true;
    }
    return false;
}

static bool zr_parse_one_field_spec(const char *spec, PbStructField *out_f) {
    while (*spec == ' ' || *spec == '\t') spec++;
    const char *colon = strchr(spec, ':');
    if (!colon || colon == spec) return false;
    size_t nlen = (size_t)(colon - spec);
    while (nlen > 0 && (spec[nlen - 1] == ' ' || spec[nlen - 1] == '\t')) nlen--;
    if (nlen == 0 || nlen >= sizeof(out_f->name)) return false;
    memcpy(out_f->name, spec, nlen);
    out_f->name[nlen] = '\0';

    char tbuf[32];
    const char *tp = colon + 1;
    while (*tp == ' ' || *tp == '\t') tp++;
    size_t tl = strlen(tp);
    while (tl > 0 && (tp[tl - 1] == ' ' || tp[tl - 1] == '\t')) tl--;
    if (tl == 0 || tl >= sizeof(tbuf)) return false;
    memcpy(tbuf, tp, tl);
    tbuf[tl] = '\0';
    return zr_parse_sfield_type(tbuf, &out_f->kind, &out_f->size);
}

static bool bi_struct_schema(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc < 1 || argc > 128) return vm_error(vm, "struct_schema() expects 1..128 arguments (field array or spec strings)");
    PbStructField tmp_fields[128];
    uint16_t fcount = 0;
    size_t offset = 0;

    if (argc == 1 && is_obj_type(argv[0], OBJ_ARRAY)) {
        PbArray *arr = AS_ARRAY(argv[0]);
        if (arr->len == 0 || arr->len > 128) {
            return vm_error(vm, "struct_schema(): field count must be in 1..128");
        }
        for (size_t i = 0; i < arr->len; i++) {
            PbValue item = arr->items[i];
            memset(&tmp_fields[fcount], 0, sizeof(PbStructField));
            if (is_obj_type(item, OBJ_STR)) {
                if (!zr_parse_one_field_spec(AS_STR(item)->chars, &tmp_fields[fcount])) {
                    return vm_error(vm, "struct_schema(): invalid field spec '%s' (expected 'name:i64|f64|i32|u8')",
                                    AS_STR(item)->chars);
                }
            } else if (is_obj_type(item, OBJ_MAP)) {
                PbValue nv = val_nil(), tv = val_nil();
                pb_map_get_cstr(AS_MAP(item), "name", &nv);
                pb_map_get_cstr(AS_MAP(item), "type", &tv);
                if (!is_obj_type(nv, OBJ_STR) || !is_obj_type(tv, OBJ_STR) ||
                    AS_STR(nv)->len == 0 || AS_STR(nv)->len >= sizeof(tmp_fields[0].name)) {
                    return vm_error(vm, "struct_schema(): map field spec requires string 'name' and 'type'");
                }
                memcpy(tmp_fields[fcount].name, AS_STR(nv)->chars, AS_STR(nv)->len + 1);
                if (!zr_parse_sfield_type(AS_STR(tv)->chars, &tmp_fields[fcount].kind, &tmp_fields[fcount].size)) {
                    return vm_error(vm, "struct_schema(): unsupported field type '%s'", AS_STR(tv)->chars);
                }
            } else {
                return vm_error(vm, "struct_schema(): field entry must be string or map");
            }
            if (offset + tmp_fields[fcount].size > 65535u) {
                return vm_error(vm, "struct_schema(): record stride exceeds 65535 bytes");
            }
            tmp_fields[fcount].offset = (uint16_t)offset;
            offset += tmp_fields[fcount].size;
            fcount++;
        }
    } else if (is_obj_type(argv[0], OBJ_STR)) {
        for (int ai = 0; ai < argc; ai++) {
            if (!is_obj_type(argv[ai], OBJ_STR)) {
                return vm_error(vm, "struct_schema(): variadic arguments must all be strings");
            }
            const char *s = AS_STR(argv[ai])->chars;
            const char *p = s;
            while (*p) {
                while (*p == ' ' || *p == '\t' || *p == ',' || *p == ';') p++;
                if (!*p) break;
                if (fcount >= 128) return vm_error(vm, "struct_schema(): too many fields (>128)");
                const char *end = p;
                while (*end && *end != ',' && *end != ';') end++;
                char seg[128];
                size_t sl = (size_t)(end - p);
                if (sl >= sizeof(seg)) return vm_error(vm, "struct_schema(): field token too long");
                memcpy(seg, p, sl);
                seg[sl] = '\0';
                memset(&tmp_fields[fcount], 0, sizeof(PbStructField));
                if (!zr_parse_one_field_spec(seg, &tmp_fields[fcount])) {
                    return vm_error(vm, "struct_schema(): invalid field spec '%s'", seg);
                }
                if (offset + tmp_fields[fcount].size > 65535u) {
                    return vm_error(vm, "struct_schema(): record stride exceeds 65535 bytes");
                }
                tmp_fields[fcount].offset = (uint16_t)offset;
                offset += tmp_fields[fcount].size;
                fcount++;
                p = end;
            }
        }
        if (fcount == 0) return vm_error(vm, "struct_schema(): empty schema specification");
    } else {
        return vm_error(vm, "struct_schema(): expected array or string specification");
    }

    PbStructBuf *sc = (PbStructBuf *)vm_alloc_obj(vm, sizeof(PbStructBuf), OBJ_STRUCT_BUF);
    sc->is_schema = true;
    sc->stride = offset;
    sc->field_count = fcount;
    sc->fields = (PbStructField *)zr_xcalloc(fcount, sizeof(PbStructField));
    memcpy(sc->fields, tmp_fields, fcount * sizeof(PbStructField));
    size_t fbytes = fcount * sizeof(PbStructField);
    vm->live_bytes += fbytes;
    if (sc->obj.gc_gen == 0) vm->nursery_bytes += fbytes;
    sc->obj.tracked_bytes += fbytes;
    *out = val_obj(sc);
    return true;
}

static bool bi_struct_schema_info(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 1 || !is_obj_type(argv[0], OBJ_STRUCT_BUF)) {
        return vm_error(vm, "struct_schema_info() expects a struct_schema or struct_buf");
    }
    PbStructBuf *sb = AS_STRUCT_BUF(argv[0]);
    vm_push_root(vm, argv[0]);
    PbMap *res = pb_map_new_raw(vm, 8);
    vm_push_root(vm, val_obj(res));
    PbArray *farr = pb_array_new(vm, sb->field_count);
    vm_push_root(vm, val_obj(farr));

    for (uint16_t i = 0; i < sb->field_count; i++) {
        PbMap *fm = pb_map_new_raw(vm, 8);
        vm_push_root(vm, val_obj(fm));
        const char *tn = "i64";
        if (sb->fields[i].kind == ZR_SFIELD_F64) tn = "f64";
        else if (sb->fields[i].kind == ZR_SFIELD_I32) tn = "i32";
        else if (sb->fields[i].kind == ZR_SFIELD_U8) tn = "u8";
        pb_map_set_cstr(vm, fm, "name", val_obj(zr_ub_str_cstr(vm, sb->fields[i].name)));
        pb_map_set_cstr(vm, fm, "type", val_obj(zr_ub_str_cstr(vm, tn)));
        pb_map_set_cstr(vm, fm, "size", val_num((double)sb->fields[i].size));
        pb_map_set_cstr(vm, fm, "offset", val_num((double)sb->fields[i].offset));
        pb_array_push(vm, farr, val_obj(fm));
        vm_pop_roots(vm, 1);
    }

    pb_map_set_cstr(vm, res, "stride", val_num((double)sb->stride));
    pb_map_set_cstr(vm, res, "field_count", val_num((double)sb->field_count));
    pb_map_set_cstr(vm, res, "fields", val_obj(farr));
    vm_pop_roots(vm, 3);
    *out = val_obj(res);
    return true;
}

static bool bi_struct_buf_new(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc < 2 || argc > 3) {
        return vm_error(vm, "struct_buf_new(schema, capacity[, shared]) expects 2 or 3 arguments");
    }
    if (!is_obj_type(argv[0], OBJ_STRUCT_BUF)) {
        return vm_error(vm, "struct_buf_new(): first argument must be a struct_schema");
    }
    PbStructBuf *sc = AS_STRUCT_BUF(argv[0]);
    if (sc->stride == 0 || sc->field_count == 0) {
        return vm_error(vm, "struct_buf_new(): invalid empty schema");
    }
    double cap_d = 0.0;
    if (!zr_ub_req_num(vm, "struct_buf_new", argv[1], &cap_d) || cap_d < 0.0 || cap_d > 268435456.0) {
        return vm_error(vm, "struct_buf_new(): capacity out of range");
    }
    size_t cap = (size_t)cap_d;
    bool shared = (argc == 3) ? val_truthy(argv[2]) : false;

    if (cap > 0 && sc->stride > (SIZE_MAX / 2) / cap) {
        return vm_error(vm, "struct_buf_new(): allocation byte size overflow");
    }
    size_t alloc_rows = cap ? cap : 1;
    size_t byte_len = alloc_rows * sc->stride;

    vm_push_root(vm, argv[0]);
    PbStructBuf *sb = (PbStructBuf *)vm_alloc_obj(vm, sizeof(PbStructBuf), OBJ_STRUCT_BUF);
    sc = AS_STRUCT_BUF(argv[0]); /* reload after potential nursery GC */
    sb->is_schema = false;
    sb->len = cap;
    sb->cap = alloc_rows;
    sb->stride = sc->stride;
    sb->field_count = sc->field_count;
    sb->shared = shared;
    sb->fields = (PbStructField *)zr_xcalloc(sc->field_count, sizeof(PbStructField));
    memcpy(sb->fields, sc->fields, sc->field_count * sizeof(PbStructField));

    if (shared) {
        sb->backing = zr_shared_backing_new(3, byte_len);
        sb->data = (uint8_t *)sb->backing->raw;
    } else {
        sb->backing = NULL;
        sb->data = (uint8_t *)zr_xcalloc(alloc_rows, sc->stride);
    }

    size_t tracked = byte_len + sc->field_count * sizeof(PbStructField);
    vm->live_bytes += tracked;
    if (sb->obj.gc_gen == 0) vm->nursery_bytes += tracked;
    sb->obj.tracked_bytes += tracked;

    vm_pop_roots(vm, 1);
    *out = val_obj(sb);
    return true;
}

static inline int zr_resolve_sfield_idx(const PbStructBuf *sb, PbValue fv) {
    if (fv.tag == VAL_NUM) {
        int idx = (int)fv.as.num;
        return (idx >= 0 && idx < (int)sb->field_count) ? idx : -1;
    }
    if (is_obj_type(fv, OBJ_STR)) {
        const char *nm = AS_STR(fv)->chars;
        for (uint16_t i = 0; i < sb->field_count; i++) {
            if (strcmp(sb->fields[i].name, nm) == 0) return (int)i;
        }
    }
    return -1;
}

static inline int64_t zr_sbuf_read_i64_raw(const uint8_t *rec, const PbStructField *f) {
    const uint8_t *p = rec + f->offset;
    switch (f->kind) {
        case ZR_SFIELD_I64: { int64_t v; memcpy(&v, p, 8); return v; }
        case ZR_SFIELD_F64: { double d;  memcpy(&d, p, 8); return (int64_t)d; }
        case ZR_SFIELD_I32: { int32_t v; memcpy(&v, p, 4); return (int64_t)v; }
        case ZR_SFIELD_U8:  return (int64_t)(*p);
        default:            return 0;
    }
}

static inline void zr_sbuf_write_i64_raw(uint8_t *rec, const PbStructField *f, int64_t val) {
    uint8_t *p = rec + f->offset;
    switch (f->kind) {
        case ZR_SFIELD_I64: memcpy(p, &val, 8); break;
        case ZR_SFIELD_F64: { double d = (double)val; memcpy(p, &d, 8); break; }
        case ZR_SFIELD_I32: { int32_t v = (int32_t)val; memcpy(p, &v, 4); break; }
        case ZR_SFIELD_U8:  *p = (uint8_t)(val & 0xFF); break;
    }
}

static inline double zr_sbuf_read_f64_raw(const uint8_t *rec, const PbStructField *f) {
    const uint8_t *p = rec + f->offset;
    switch (f->kind) {
        case ZR_SFIELD_F64: { double d;  memcpy(&d, p, 8); return d; }
        case ZR_SFIELD_I64: { int64_t v; memcpy(&v, p, 8); return (double)v; }
        case ZR_SFIELD_I32: { int32_t v; memcpy(&v, p, 4); return (double)v; }
        case ZR_SFIELD_U8:  return (double)(*p);
        default:            return 0.0;
    }
}

static inline void zr_sbuf_write_f64_raw(uint8_t *rec, const PbStructField *f, double val) {
    uint8_t *p = rec + f->offset;
    switch (f->kind) {
        case ZR_SFIELD_F64: memcpy(p, &val, 8); break;
        case ZR_SFIELD_I64: { int64_t v = (int64_t)val; memcpy(p, &v, 8); break; }
        case ZR_SFIELD_I32: { int32_t v = (int32_t)val; memcpy(p, &v, 4); break; }
        case ZR_SFIELD_U8:  *p = (uint8_t)((int)val & 0xFF); break;
    }
}

static bool bi_struct_buf_len(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 1 || !is_obj_type(argv[0], OBJ_STRUCT_BUF) || AS_STRUCT_BUF(argv[0])->is_schema) {
        return vm_error(vm, "struct_buf_len() expects a struct_buf");
    }
    *out = val_num((double)AS_STRUCT_BUF(argv[0])->len);
    return true;
}

static bool bi_struct_buf_stride(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 1 || !is_obj_type(argv[0], OBJ_STRUCT_BUF)) {
        return vm_error(vm, "struct_buf_stride() expects a struct_buf or struct_schema");
    }
    *out = val_num((double)AS_STRUCT_BUF(argv[0])->stride);
    return true;
}

static bool bi_struct_buf_ptr(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 1 || !is_obj_type(argv[0], OBJ_STRUCT_BUF) || AS_STRUCT_BUF(argv[0])->is_schema) {
        return vm_error(vm, "struct_buf_ptr() expects a struct_buf");
    }
    uintptr_t raw = (uintptr_t)AS_STRUCT_BUF(argv[0])->data;
    *out = val_num((double)raw);
    return true;
}

static bool bi_struct_buf_push(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 2 || !is_obj_type(argv[0], OBJ_STRUCT_BUF) || AS_STRUCT_BUF(argv[0])->is_schema ||
        !is_obj_type(argv[1], OBJ_ARRAY)) {
        return vm_error(vm, "struct_buf_push(buf, row_values_array) expects (struct_buf, array)");
    }
    PbStructBuf *sb = AS_STRUCT_BUF(argv[0]);
    PbArray *vals = AS_ARRAY(argv[1]);
    if (sb->shared) return vm_error(vm, "struct_buf_push(): cannot resize shared fixed buffer");
    if (vals->len != sb->field_count) {
        return vm_error(vm, "struct_buf_push(): expected %u field values, got %zu",
                        (unsigned)sb->field_count, vals->len);
    }
    if (sb->len >= sb->cap) {
        size_t ncap = sb->cap < 16 ? 16 : sb->cap * 2;
        size_t old_bytes = sb->cap * sb->stride;
        size_t new_bytes = ncap * sb->stride;
        sb->data = (uint8_t *)zr_xrealloc(sb->data, new_bytes);
        memset(sb->data + old_bytes, 0, new_bytes - old_bytes);
        sb->cap = ncap;
        size_t diff = new_bytes - old_bytes;
        vm->live_bytes += diff;
        if (sb->obj.gc_gen == 0) vm->nursery_bytes += diff;
        sb->obj.tracked_bytes += diff;
    }
    uint8_t *rec = sb->data + sb->len * sb->stride;
    for (uint16_t i = 0; i < sb->field_count; i++) {
        PbValue v = vals->items[i];
        double d = (v.tag == VAL_NUM) ? v.as.num : (v.tag == VAL_BOOL ? (v.as.b ? 1.0 : 0.0) : 0.0);
        if (sb->fields[i].kind == ZR_SFIELD_F64) zr_sbuf_write_f64_raw(rec, &sb->fields[i], d);
        else zr_sbuf_write_i64_raw(rec, &sb->fields[i], (int64_t)d);
    }
    sb->len++;
    *out = val_num((double)sb->len);
    return true;
}

static bool bi_struct_get_i64(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 3 || !is_obj_type(argv[0], OBJ_STRUCT_BUF) || argv[1].tag != VAL_NUM) {
        return vm_error(vm, "struct_get_i64(buf, row, field) expects (struct_buf, number, field)");
    }
    PbStructBuf *sb = AS_STRUCT_BUF(argv[0]);
    if (sb->is_schema) return vm_error(vm, "struct_get_i64(): got schema instead of buffer");
    int64_t row = (int64_t)argv[1].as.num;
    if (row < 0 || (size_t)row >= sb->len) {
        return vm_error(vm, "struct_get_i64(): row %" PRId64 " out of bounds (len %zu)", row, sb->len);
    }
    int fidx = zr_resolve_sfield_idx(sb, argv[2]);
    if (fidx < 0) return vm_error(vm, "struct_get_i64(): invalid field index or name");
    int64_t v = zr_sbuf_read_i64_raw(sb->data + (size_t)row * sb->stride, &sb->fields[fidx]);
    *out = val_num((double)v);
    return true;
}

static bool bi_struct_set_i64(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 4 || !is_obj_type(argv[0], OBJ_STRUCT_BUF) || argv[1].tag != VAL_NUM) {
        return vm_error(vm, "struct_set_i64(buf, row, field, val) expects 4 arguments");
    }
    PbStructBuf *sb = AS_STRUCT_BUF(argv[0]);
    if (sb->is_schema) return vm_error(vm, "struct_set_i64(): got schema instead of buffer");
    int64_t row = (int64_t)argv[1].as.num;
    if (row < 0 || (size_t)row >= sb->len) {
        return vm_error(vm, "struct_set_i64(): row %" PRId64 " out of bounds (len %zu)", row, sb->len);
    }
    int fidx = zr_resolve_sfield_idx(sb, argv[2]);
    if (fidx < 0) return vm_error(vm, "struct_set_i64(): invalid field index or name");
    double dv = 0.0;
    if (argv[3].tag == VAL_BOOL) dv = argv[3].as.b ? 1.0 : 0.0;
    else if (!zr_ub_req_num(vm, "struct_set_i64", argv[3], &dv)) return false;
    zr_sbuf_write_i64_raw(sb->data + (size_t)row * sb->stride, &sb->fields[fidx], (int64_t)dv);
    *out = val_nil();
    return true;
}

static bool bi_struct_get_f64(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 3 || !is_obj_type(argv[0], OBJ_STRUCT_BUF) || argv[1].tag != VAL_NUM) {
        return vm_error(vm, "struct_get_f64(buf, row, field) expects (struct_buf, number, field)");
    }
    PbStructBuf *sb = AS_STRUCT_BUF(argv[0]);
    if (sb->is_schema) return vm_error(vm, "struct_get_f64(): got schema instead of buffer");
    int64_t row = (int64_t)argv[1].as.num;
    if (row < 0 || (size_t)row >= sb->len) {
        return vm_error(vm, "struct_get_f64(): row %" PRId64 " out of bounds (len %zu)", row, sb->len);
    }
    int fidx = zr_resolve_sfield_idx(sb, argv[2]);
    if (fidx < 0) return vm_error(vm, "struct_get_f64(): invalid field index or name");
    double v = zr_sbuf_read_f64_raw(sb->data + (size_t)row * sb->stride, &sb->fields[fidx]);
    *out = val_num(v);
    return true;
}

static bool bi_struct_set_f64(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 4 || !is_obj_type(argv[0], OBJ_STRUCT_BUF) || argv[1].tag != VAL_NUM) {
        return vm_error(vm, "struct_set_f64(buf, row, field, val) expects 4 arguments");
    }
    PbStructBuf *sb = AS_STRUCT_BUF(argv[0]);
    if (sb->is_schema) return vm_error(vm, "struct_set_f64(): got schema instead of buffer");
    int64_t row = (int64_t)argv[1].as.num;
    if (row < 0 || (size_t)row >= sb->len) {
        return vm_error(vm, "struct_set_f64(): row %" PRId64 " out of bounds (len %zu)", row, sb->len);
    }
    int fidx = zr_resolve_sfield_idx(sb, argv[2]);
    if (fidx < 0) return vm_error(vm, "struct_set_f64(): invalid field index or name");
    double dv = 0.0;
    if (!zr_ub_req_num(vm, "struct_set_f64", argv[3], &dv)) return false;
    zr_sbuf_write_f64_raw(sb->data + (size_t)row * sb->stride, &sb->fields[fidx], dv);
    *out = val_nil();
    return true;
}

/* Reentrant Thread-Safe In-Place Introsort over Packed `stride`-Byte Records */
static inline int zr_sbuf_cmp_rec(const uint8_t *a, const uint8_t *b, const PbStructField *f, bool desc) {
    int c = 0;
    if (f->kind == ZR_SFIELD_F64) {
        double da, db;
        memcpy(&da, a + f->offset, 8);
        memcpy(&db, b + f->offset, 8);
        c = (da < db) ? -1 : (da > db ? 1 : 0);
    } else if (f->kind == ZR_SFIELD_I64) {
        int64_t ia, ib;
        memcpy(&ia, a + f->offset, 8);
        memcpy(&ib, b + f->offset, 8);
        c = (ia < ib) ? -1 : (ia > ib ? 1 : 0);
    } else if (f->kind == ZR_SFIELD_I32) {
        int32_t ia, ib;
        memcpy(&ia, a + f->offset, 4);
        memcpy(&ib, b + f->offset, 4);
        c = (ia < ib) ? -1 : (ia > ib ? 1 : 0);
    } else {
        uint8_t ua = a[f->offset], ub = b[f->offset];
        c = (ua < ub) ? -1 : (ua > ub ? 1 : 0);
    }
    return desc ? -c : c;
}

static inline void zr_sbuf_swap_rec(uint8_t *a, uint8_t *b, size_t stride, uint8_t *tmp) {
    if (a == b) return;
    memcpy(tmp, a, stride);
    memcpy(a, b, stride);
    memcpy(b, tmp, stride);
}

static void zr_sbuf_heapsort_range(uint8_t *base, size_t n, size_t stride,
                                   const PbStructField *f, bool desc, uint8_t *tmp) {
    if (n < 2) return;
    for (size_t start = n / 2; start > 0; start--) {
        size_t root = start - 1;
        for (;;) {
            size_t child = root * 2 + 1;
            if (child >= n) break;
            if (child + 1 < n &&
                zr_sbuf_cmp_rec(base + child * stride, base + (child + 1) * stride, f, desc) < 0) {
                child++;
            }
            if (zr_sbuf_cmp_rec(base + root * stride, base + child * stride, f, desc) < 0) {
                zr_sbuf_swap_rec(base + root * stride, base + child * stride, stride, tmp);
                root = child;
            } else {
                break;
            }
        }
    }
    for (size_t end = n - 1; end > 0; end--) {
        zr_sbuf_swap_rec(base, base + end * stride, stride, tmp);
        size_t root = 0;
        for (;;) {
            size_t child = root * 2 + 1;
            if (child >= end) break;
            if (child + 1 < end &&
                zr_sbuf_cmp_rec(base + child * stride, base + (child + 1) * stride, f, desc) < 0) {
                child++;
            }
            if (zr_sbuf_cmp_rec(base + root * stride, base + child * stride, f, desc) < 0) {
                zr_sbuf_swap_rec(base + root * stride, base + child * stride, stride, tmp);
                root = child;
            } else {
                break;
            }
        }
    }
}

static void zr_sbuf_introsort(uint8_t *data, size_t n, size_t stride, const PbStructField *f, bool desc) {
    if (n < 2) return;
    uint8_t stack_rec[128], stack_piv[128];
    uint8_t *tmp = (stride <= sizeof(stack_rec)) ? stack_rec : (uint8_t *)zr_xmalloc(stride * 2);
    uint8_t *piv = (stride <= sizeof(stack_piv)) ? stack_piv : (tmp + stride);

    typedef struct { size_t lo; size_t hi; int depth; } SortTask;
    SortTask st[64];
    int sp = 0;
    int max_depth = 0;
    for (size_t m = n; m > 1; m >>= 1) max_depth += 2;

    st[sp].lo = 0;
    st[sp].hi = n - 1;
    st[sp].depth = max_depth;
    sp++;

    while (sp > 0) {
        sp--;
        size_t lo = st[sp].lo;
        size_t hi = st[sp].hi;
        int depth = st[sp].depth;

        while (hi > lo) {
            size_t cnt = hi - lo + 1;
            if (cnt <= 16) {
                for (size_t i = lo + 1; i <= hi; i++) {
                    memcpy(tmp, data + i * stride, stride);
                    size_t j = i;
                    while (j > lo && zr_sbuf_cmp_rec(data + (j - 1) * stride, tmp, f, desc) > 0) {
                        memcpy(data + j * stride, data + (j - 1) * stride, stride);
                        j--;
                    }
                    memcpy(data + j * stride, tmp, stride);
                }
                break;
            }
            if (depth <= 0) {
                zr_sbuf_heapsort_range(data + lo * stride, cnt, stride, f, desc, tmp);
                break;
            }
            depth--;

            size_t mid = lo + (cnt >> 1);
            if (zr_sbuf_cmp_rec(data + mid * stride, data + lo * stride, f, desc) < 0)
                zr_sbuf_swap_rec(data + mid * stride, data + lo * stride, stride, tmp);
            if (zr_sbuf_cmp_rec(data + hi * stride, data + lo * stride, f, desc) < 0)
                zr_sbuf_swap_rec(data + hi * stride, data + lo * stride, stride, tmp);
            if (zr_sbuf_cmp_rec(data + hi * stride, data + mid * stride, f, desc) < 0)
                zr_sbuf_swap_rec(data + hi * stride, data + mid * stride, stride, tmp);

            memcpy(piv, data + mid * stride, stride);
            size_t i = lo, j = hi;
            for (;;) {
                while (zr_sbuf_cmp_rec(data + i * stride, piv, f, desc) < 0) i++;
                while (zr_sbuf_cmp_rec(piv, data + j * stride, f, desc) < 0) j--;
                if (i >= j) break;
                zr_sbuf_swap_rec(data + i * stride, data + j * stride, stride, tmp);
                i++;
                if (j > 0) j--;
            }

            size_t left_len = (j > lo) ? (j - lo + 1) : 0;
            size_t right_len = (i < hi) ? (hi - i + 1) : 0;
            if (left_len < right_len) {
                if (i < hi && sp < 63) {
                    st[sp].lo = i; st[sp].hi = hi; st[sp].depth = depth; sp++;
                }
                if (j > lo) { hi = j; } else { break; }
            } else {
                if (j > lo && sp < 63) {
                    st[sp].lo = lo; st[sp].hi = j; st[sp].depth = depth; sp++;
                }
                if (i < hi) { lo = i; } else { break; }
            }
        }
    }

    if (stride > sizeof(stack_rec)) free(tmp);
}

static bool bi_struct_sort_by(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc < 2 || argc > 3 || !is_obj_type(argv[0], OBJ_STRUCT_BUF)) {
        return vm_error(vm, "struct_sort_by(buf, field[, descending]) expects 2 or 3 arguments");
    }
    PbStructBuf *sb = AS_STRUCT_BUF(argv[0]);
    if (sb->is_schema) return vm_error(vm, "struct_sort_by(): expected struct_buf, got schema");
    int fidx = zr_resolve_sfield_idx(sb, argv[1]);
    if (fidx < 0) return vm_error(vm, "struct_sort_by(): invalid field index or name");
    bool desc = (argc == 3) ? val_truthy(argv[2]) : false;

    double t0 = zr_mono_sec();
    zr_sbuf_introsort(sb->data, sb->len, sb->stride, &sb->fields[fidx], desc);
    double elapsed_ms = (zr_mono_sec() - t0) * 1000.0;
    *out = val_num(elapsed_ms);
    return true;
}

static bool bi_struct_reduce_f64(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc < 3 || argc > 4 || !is_obj_type(argv[0], OBJ_STRUCT_BUF) || !is_obj_type(argv[2], OBJ_STR)) {
        return vm_error(vm, "struct_reduce_f64(buf, field, op[, field2]) expects 3 or 4 arguments");
    }
    PbStructBuf *sb = AS_STRUCT_BUF(argv[0]);
    if (sb->is_schema) return vm_error(vm, "struct_reduce_f64(): expected struct_buf");
    int fidx = zr_resolve_sfield_idx(sb, argv[1]);
    if (fidx < 0) return vm_error(vm, "struct_reduce_f64(): invalid field index or name");
    const char *op = AS_STR(argv[2])->chars;
    const PbStructField *f1 = &sb->fields[fidx];

    if (sb->len == 0) {
        *out = val_num(0.0);
        return true;
    }

    if (strcmp(op, "sum") == 0 || strcmp(op, "mean") == 0) {
        double acc = 0.0;
        if (f1->kind == ZR_SFIELD_F64) {
            const uint8_t *p = sb->data + f1->offset;
            for (size_t i = 0; i < sb->len; i++, p += sb->stride) {
                double v; memcpy(&v, p, 8); acc += v;
            }
        } else {
            for (size_t i = 0; i < sb->len; i++) {
                acc += zr_sbuf_read_f64_raw(sb->data + i * sb->stride, f1);
            }
        }
        if (strcmp(op, "mean") == 0) acc /= (double)sb->len;
        *out = val_num(acc);
        return true;
    }
    if (strcmp(op, "min") == 0) {
        double mv = zr_sbuf_read_f64_raw(sb->data, f1);
        for (size_t i = 1; i < sb->len; i++) {
            double v = zr_sbuf_read_f64_raw(sb->data + i * sb->stride, f1);
            if (v < mv) mv = v;
        }
        *out = val_num(mv);
        return true;
    }
    if (strcmp(op, "max") == 0) {
        double mv = zr_sbuf_read_f64_raw(sb->data, f1);
        for (size_t i = 1; i < sb->len; i++) {
            double v = zr_sbuf_read_f64_raw(sb->data + i * sb->stride, f1);
            if (v > mv) mv = v;
        }
        *out = val_num(mv);
        return true;
    }
    if (strcmp(op, "dot") == 0) {
        int fidx2 = fidx;
        if (argc == 4) {
            fidx2 = zr_resolve_sfield_idx(sb, argv[3]);
            if (fidx2 < 0) return vm_error(vm, "struct_reduce_f64(dot): invalid second field");
        }
        const PbStructField *f2 = &sb->fields[fidx2];
        double dot = 0.0;
        for (size_t i = 0; i < sb->len; i++) {
            const uint8_t *rec = sb->data + i * sb->stride;
            dot += zr_sbuf_read_f64_raw(rec, f1) * zr_sbuf_read_f64_raw(rec, f2);
        }
        *out = val_num(dot);
        return true;
    }

    return vm_error(vm, "struct_reduce_f64(): unknown op '%s' (expected sum|min|max|mean|dot)", op);
}

/* ============================================================================
 * 2. Unboxed C-Speed CSR Graph Algorithms (`graph_bfs_shortest_path`, `graph_dijkstra_f64`)
 * ============================================================================ */

static bool zr_extract_u32_buf(PbVM *vm, const char *fn, PbValue v, uint32_t **out_buf, size_t *out_len) {
    if (is_obj_type(v, OBJ_ARRAY)) {
        PbArray *a = AS_ARRAY(v);
        uint32_t *p = (uint32_t *)zr_xmalloc((a->len ? a->len : 1) * sizeof(uint32_t));
        for (size_t i = 0; i < a->len; i++) {
            p[i] = (a->items[i].tag == VAL_NUM && a->items[i].as.num >= 0.0) ? (uint32_t)a->items[i].as.num : 0u;
        }
        *out_buf = p;
        *out_len = a->len;
        return true;
    }
    if (is_obj_type(v, OBJ_F64BUF)) {
        PbF64Buf *fb = AS_F64BUF(v);
        uint32_t *p = (uint32_t *)zr_xmalloc((fb->len ? fb->len : 1) * sizeof(uint32_t));
        for (size_t i = 0; i < fb->len; i++) {
            p[i] = fb->data[i] >= 0.0 ? (uint32_t)fb->data[i] : 0u;
        }
        *out_buf = p;
        *out_len = fb->len;
        return true;
    }
    if (is_obj_type(v, OBJ_BYTES)) {
        PbBytes *bb = AS_BYTES(v);
        size_t cnt = bb->len / 4;
        uint32_t *p = (uint32_t *)zr_xmalloc((cnt ? cnt : 1) * sizeof(uint32_t));
        if (cnt > 0) memcpy(p, bb->data, cnt * 4);
        *out_buf = p;
        *out_len = cnt;
        return true;
    }
    return vm_error(vm, "%s(): expected array, f64buf, or u32 bytes buffer", fn);
}

static bool zr_extract_f64_view_or_copy(PbVM *vm, const char *fn, PbValue v,
                                        const double **out_ptr, double **out_owned, size_t *out_len) {
    *out_owned = NULL;
    if (is_obj_type(v, OBJ_F64BUF)) {
        *out_ptr = AS_F64BUF(v)->data;
        *out_len = AS_F64BUF(v)->len;
        return true;
    }
    if (is_obj_type(v, OBJ_STRUCT_BUF) && !AS_STRUCT_BUF(v)->is_schema && (AS_STRUCT_BUF(v)->stride % 8 == 0)) {
        PbStructBuf *sb = AS_STRUCT_BUF(v);
        *out_ptr = (const double *)(const void *)sb->data;
        *out_len = sb->len * (sb->stride / 8);
        return true;
    }
    if (is_obj_type(v, OBJ_ARRAY)) {
        PbArray *a = AS_ARRAY(v);
        double *cp = (double *)zr_xmalloc((a->len ? a->len : 1) * sizeof(double));
        for (size_t i = 0; i < a->len; i++) {
            cp[i] = (a->items[i].tag == VAL_NUM) ? a->items[i].as.num : 0.0;
        }
        *out_ptr = cp;
        *out_owned = cp;
        *out_len = a->len;
        return true;
    }
    return vm_error(vm, "%s(): expected array, f64buf, or f64 struct_buf", fn);
}

static PbArray *zr_reconstruct_path(PbVM *vm, const int32_t *parent, uint32_t src, uint32_t dst, size_t max_nodes) {
    uint32_t *rev = (uint32_t *)zr_xmalloc((max_nodes ? max_nodes : 1) * sizeof(uint32_t));
    size_t plen = 0;
    int32_t cur = (int32_t)dst;
    while (cur >= 0 && plen < max_nodes) {
        rev[plen++] = (uint32_t)cur;
        if ((uint32_t)cur == src) break;
        cur = parent[cur];
    }
    PbArray *pa = pb_array_new(vm, plen);
    vm_push_root(vm, val_obj(pa));
    for (size_t i = plen; i > 0; i--) {
        pb_array_push(vm, pa, val_num((double)rev[i - 1]));
    }
    vm_pop_roots(vm, 1);
    free(rev);
    return pa;
}

static bool bi_graph_bfs_shortest_path(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 5) {
        return vm_error(vm, "graph_bfs_shortest_path(row_ptr, col_idx, n_nodes, src, dst) expects 5 arguments");
    }
    double n_d = 0.0, src_d = 0.0, dst_d = 0.0;
    if (!zr_ub_req_num(vm, "graph_bfs_shortest_path", argv[2], &n_d) ||
        !zr_ub_req_num(vm, "graph_bfs_shortest_path", argv[3], &src_d) ||
        !zr_ub_req_num(vm, "graph_bfs_shortest_path", argv[4], &dst_d)) {
        return false;
    }
    if (n_d <= 0.0 || n_d > 50000000.0) return vm_error(vm, "graph_bfs_shortest_path(): invalid n_nodes");
    size_t n = (size_t)n_d;
    if (src_d < 0.0 || (size_t)src_d >= n || dst_d < 0.0 || (size_t)dst_d >= n) {
        return vm_error(vm, "graph_bfs_shortest_path(): src or dst out of range");
    }
    uint32_t src = (uint32_t)src_d, dst = (uint32_t)dst_d;

    uint32_t *row_ptr = NULL, *col_idx = NULL;
    size_t rp_len = 0, ci_len = 0;
    if (!zr_extract_u32_buf(vm, "graph_bfs_shortest_path", argv[0], &row_ptr, &rp_len)) return false;
    if (!zr_extract_u32_buf(vm, "graph_bfs_shortest_path", argv[1], &col_idx, &ci_len)) {
        free(row_ptr);
        return false;
    }
    if (rp_len < n + 1) {
        free(row_ptr); free(col_idx);
        return vm_error(vm, "graph_bfs_shortest_path(): row_ptr length (%zu) < n_nodes + 1 (%zu)", rp_len, n + 1);
    }

    double t0 = zr_mono_sec();
    int32_t *dist = (int32_t *)zr_xmalloc(n * sizeof(int32_t));
    int32_t *parent = (int32_t *)zr_xmalloc(n * sizeof(int32_t));
    uint32_t *queue = (uint32_t *)zr_xmalloc(n * sizeof(uint32_t));
    memset(dist, -1, n * sizeof(int32_t));
    memset(parent, -1, n * sizeof(int32_t));

    size_t qh = 0, qt = 0, visited = 0;
    dist[src] = 0;
    queue[qt++] = src;
    visited = 1;

    while (qh < qt) {
        uint32_t u = queue[qh++];
        if (u == dst) break;
        uint32_t e0 = row_ptr[u], e1 = row_ptr[u + 1];
        if (e1 > ci_len) e1 = (uint32_t)ci_len;
        int32_t nd = dist[u] + 1;
        for (uint32_t e = e0; e < e1; e++) {
            uint32_t v = col_idx[e];
            if (v < n && dist[v] < 0) {
                dist[v] = nd;
                parent[v] = (int32_t)u;
                queue[qt++] = v;
                visited++;
            }
        }
    }
    double elapsed_ms = (zr_mono_sec() - t0) * 1000.0;

    bool reachable = (dist[dst] >= 0);
    int32_t best_dist = dist[dst];

    PbMap *res = pb_map_new_raw(vm, 8);
    vm_push_root(vm, val_obj(res));
    PbArray *path = reachable ? zr_reconstruct_path(vm, parent, src, dst, n) : pb_array_new(vm, 0);
    vm_push_root(vm, val_obj(path));

    pb_map_set_cstr(vm, res, "ok", val_bool(true));
    pb_map_set_cstr(vm, res, "reachable", val_bool(reachable));
    pb_map_set_cstr(vm, res, "distance", val_num((double)best_dist));
    pb_map_set_cstr(vm, res, "path", val_obj(path));
    pb_map_set_cstr(vm, res, "visited_count", val_num((double)visited));
    pb_map_set_cstr(vm, res, "elapsed_ms", val_num(elapsed_ms));

    vm_pop_roots(vm, 2);
    free(queue);
    free(parent);
    free(dist);
    free(col_idx);
    free(row_ptr);
    *out = val_obj(res);
    return true;
}

typedef struct {
    double   dist;
    uint32_t node;
} ZrDijkstraNode;

static inline void zr_dheap_push(ZrDijkstraNode **hp, size_t *hlen, size_t *hcap, double dist, uint32_t node) {
    if (*hlen >= *hcap) {
        *hcap = (*hcap) ? (*hcap * 2) : 256;
        *hp = (ZrDijkstraNode *)zr_xrealloc(*hp, (*hcap) * sizeof(ZrDijkstraNode));
    }
    ZrDijkstraNode *h = *hp;
    size_t i = (*hlen)++;
    while (i > 0) {
        size_t p = (i - 1) >> 2; /* 4-ary heap for cache locality */
        if (h[p].dist <= dist) break;
        h[i] = h[p];
        i = p;
    }
    h[i].dist = dist;
    h[i].node = node;
}

static inline ZrDijkstraNode zr_dheap_pop(ZrDijkstraNode *h, size_t *hlen) {
    ZrDijkstraNode top = h[0];
    ZrDijkstraNode last = h[--(*hlen)];
    if (*hlen > 0) {
        size_t i = 0, n = *hlen;
        for (;;) {
            size_t c0 = (i << 2) + 1;
            if (c0 >= n) break;
            size_t best = c0;
            size_t cmax = c0 + 4 < n ? c0 + 4 : n;
            for (size_t c = c0 + 1; c < cmax; c++) {
                if (h[c].dist < h[best].dist) best = c;
            }
            if (last.dist <= h[best].dist) break;
            h[i] = h[best];
            i = best;
        }
        h[i] = last;
    }
    return top;
}

static bool bi_graph_dijkstra_f64(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc < 5 || argc > 6) {
        return vm_error(vm, "graph_dijkstra_f64(row_ptr, col_idx, weights, n_nodes, src[, dst]) expects 5 or 6 arguments");
    }
    double n_d = 0.0, src_d = 0.0;
    if (!zr_ub_req_num(vm, "graph_dijkstra_f64", argv[3], &n_d) ||
        !zr_ub_req_num(vm, "graph_dijkstra_f64", argv[4], &src_d)) {
        return false;
    }
    if (n_d <= 0.0 || n_d > 50000000.0) return vm_error(vm, "graph_dijkstra_f64(): invalid n_nodes");
    size_t n = (size_t)n_d;
    if (src_d < 0.0 || (size_t)src_d >= n) return vm_error(vm, "graph_dijkstra_f64(): src out of range");
    uint32_t src = (uint32_t)src_d;
    bool has_dst = (argc == 6 && argv[5].tag != VAL_NIL);
    uint32_t dst = (uint32_t)(n - 1);
    if (has_dst) {
        double dst_d = 0.0;
        if (!zr_ub_req_num(vm, "graph_dijkstra_f64", argv[5], &dst_d) || dst_d < 0.0 || (size_t)dst_d >= n) {
            return vm_error(vm, "graph_dijkstra_f64(): dst out of range");
        }
        dst = (uint32_t)dst_d;
    }

    uint32_t *row_ptr = NULL, *col_idx = NULL;
    size_t rp_len = 0, ci_len = 0;
    if (!zr_extract_u32_buf(vm, "graph_dijkstra_f64", argv[0], &row_ptr, &rp_len)) return false;
    if (!zr_extract_u32_buf(vm, "graph_dijkstra_f64", argv[1], &col_idx, &ci_len)) {
        free(row_ptr);
        return false;
    }
    const double *weights = NULL;
    double *weights_owned = NULL;
    size_t w_len = 0;
    if (!zr_extract_f64_view_or_copy(vm, "graph_dijkstra_f64", argv[2], &weights, &weights_owned, &w_len)) {
        free(row_ptr); free(col_idx);
        return false;
    }
    if (rp_len < n + 1 || w_len < ci_len) {
        free(weights_owned); free(col_idx); free(row_ptr);
        return vm_error(vm, "graph_dijkstra_f64(): CSR buffer length mismatch");
    }

    double t0 = zr_mono_sec();
    double *dist = (double *)zr_xmalloc(n * sizeof(double));
    int32_t *parent = (int32_t *)zr_xmalloc(n * sizeof(int32_t));
    for (size_t i = 0; i < n; i++) {
        dist[i] = DBL_MAX;
        parent[i] = -1;
    }

    size_t hlen = 0, hcap = n < 1024 ? 1024 : n;
    ZrDijkstraNode *heap = (ZrDijkstraNode *)zr_xmalloc(hcap * sizeof(ZrDijkstraNode));
    dist[src] = 0.0;
    zr_dheap_push(&heap, &hlen, &hcap, 0.0, src);
    size_t settled = 0;

    while (hlen > 0) {
        ZrDijkstraNode cur = zr_dheap_pop(heap, &hlen);
        uint32_t u = cur.node;
        if (cur.dist > dist[u]) continue;
        settled++;
        if (has_dst && u == dst) break;

        uint32_t e0 = row_ptr[u], e1 = row_ptr[u + 1];
        if (e1 > ci_len) e1 = (uint32_t)ci_len;
        double du = cur.dist;
        for (uint32_t e = e0; e < e1; e++) {
            uint32_t v = col_idx[e];
            double w = weights[e];
            if (v < n && w >= 0.0) {
                double nd = du + w;
                if (nd < dist[v]) {
                    dist[v] = nd;
                    parent[v] = (int32_t)u;
                    zr_dheap_push(&heap, &hlen, &hcap, nd, v);
                }
            }
        }
    }
    double elapsed_ms = (zr_mono_sec() - t0) * 1000.0;

    bool reachable = (dist[dst] < DBL_MAX * 0.5);
    double best_d = reachable ? dist[dst] : -1.0;

    PbMap *res = pb_map_new_raw(vm, 8);
    vm_push_root(vm, val_obj(res));
    PbArray *path = reachable ? zr_reconstruct_path(vm, parent, src, dst, n) : pb_array_new(vm, 0);
    vm_push_root(vm, val_obj(path));
    size_t samp_n = n < 8 ? n : 8;
    PbArray *samp = pb_array_new(vm, samp_n);
    vm_push_root(vm, val_obj(samp));
    for (size_t i = 0; i < samp_n; i++) {
        pb_array_push(vm, samp, val_num(dist[i] < DBL_MAX * 0.5 ? dist[i] : -1.0));
    }

    pb_map_set_cstr(vm, res, "ok", val_bool(true));
    pb_map_set_cstr(vm, res, "reachable", val_bool(reachable));
    pb_map_set_cstr(vm, res, "distance", val_num(best_d));
    pb_map_set_cstr(vm, res, "path", val_obj(path));
    pb_map_set_cstr(vm, res, "visited_count", val_num((double)settled));
    pb_map_set_cstr(vm, res, "dist_sample", val_obj(samp));
    pb_map_set_cstr(vm, res, "elapsed_ms", val_num(elapsed_ms));

    vm_pop_roots(vm, 3);
    free(heap);
    free(parent);
    free(dist);
    free(weights_owned);
    free(col_idx);
    free(row_ptr);
    *out = val_obj(res);
    return true;
}

/* ============================================================================
 * 3. Native C99 Z-Buffered 3D Cook-Torrance PBR Triangle Rasterizer
 * ============================================================================ */

static inline float zr_clampf(float x, float lo, float hi) {
    return x < lo ? lo : (x > hi ? hi : x);
}

static bool bi_gfx3d_raster_triangles(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc < 5 || argc > 6) {
        return vm_error(vm, "gfx3d_raster_triangles(w, h, verts, indices, cam_mvp[, opts]) expects 5 or 6 arguments");
    }
    double wd = 0.0, hd = 0.0;
    if (!zr_ub_req_num(vm, "gfx3d_raster_triangles", argv[0], &wd) ||
        !zr_ub_req_num(vm, "gfx3d_raster_triangles", argv[1], &hd)) {
        return false;
    }
    if (wd < 1.0 || wd > 8192.0 || hd < 1.0 || hd > 8192.0) {
        return vm_error(vm, "gfx3d_raster_triangles(): invalid viewport dimensions");
    }
    int W = (int)wd, H = (int)hd;

    const double *verts = NULL;
    double *verts_owned = NULL;
    size_t vlen = 0;
    if (!zr_extract_f64_view_or_copy(vm, "gfx3d_raster_triangles", argv[2], &verts, &verts_owned, &vlen)) {
        return false;
    }

    /* Extract triangle indices (supports flat `[i0, i1, i2, ...]` OR array of `[i0, i1, i2]` triplets) */
    uint32_t *indices = NULL;
    size_t ilen = 0;
    uint32_t max_vidx = 0;
    if (is_obj_type(argv[3], OBJ_ARRAY) && AS_ARRAY(argv[3])->len > 0 &&
        is_obj_type(AS_ARRAY(argv[3])->items[0], OBJ_ARRAY)) {
        PbArray *outer = AS_ARRAY(argv[3]);
        ilen = outer->len * 3;
        indices = (uint32_t *)zr_xmalloc((ilen ? ilen : 1) * sizeof(uint32_t));
        size_t k = 0;
        for (size_t i = 0; i < outer->len; i++) {
            if (is_obj_type(outer->items[i], OBJ_ARRAY) && AS_ARRAY(outer->items[i])->len >= 3) {
                PbArray *tri = AS_ARRAY(outer->items[i]);
                for (int c = 0; c < 3; c++) {
                    uint32_t idx = (tri->items[c].tag == VAL_NUM && tri->items[c].as.num >= 0.0)
                                   ? (uint32_t)tri->items[c].as.num : 0u;
                    if (idx > max_vidx) max_vidx = idx;
                    indices[k++] = idx;
                }
            }
        }
        ilen = k;
    } else {
        if (!zr_extract_u32_buf(vm, "gfx3d_raster_triangles", argv[3], &indices, &ilen)) {
            free(verts_owned);
            return false;
        }
        for (size_t i = 0; i < ilen; i++) {
            if (indices[i] > max_vidx) max_vidx = indices[i];
        }
    }

    /* MVP matrix (4x4 row-major default; identity/perspective fallback if nil) */
    float mvp[16] = {
        1.0f, 0.0f, 0.0f, 0.0f,
        0.0f, 1.0f, 0.0f, 0.0f,
        0.0f, 0.0f, 1.0f, 0.0f,
        0.0f, 0.0f, 0.0f, 1.0f
    };
    if (argv[4].tag != VAL_NIL) {
        const double *mp = NULL;
        double *mp_owned = NULL;
        size_t mlen = 0;
        if (zr_extract_f64_view_or_copy(vm, "gfx3d_raster_triangles", argv[4], &mp, &mp_owned, &mlen) && mlen >= 16) {
            for (int i = 0; i < 16; i++) mvp[i] = (float)mp[i];
        }
        free(mp_owned);
    }

    /* Shading & layout options */
    int vstride = 0;
    float metallic = 0.35f, roughness = 0.38f, ambient = 0.18f;
    float alb_r = 0.82f, alb_g = 0.56f, alb_b = 0.28f;
    float lx = 0.48f, ly = 0.76f, lz = 0.44f;
    uint8_t bg_r = 16, bg_g = 20, bg_b = 30, bg_a = 255;

    if (argc == 6 && is_obj_type(argv[5], OBJ_MAP)) {
        PbMap *om = AS_MAP(argv[5]);
        PbValue v = val_nil();
        if (pb_map_get_cstr(om, "stride", &v) && v.tag == VAL_NUM) vstride = (int)v.as.num;
        else if (pb_map_get_cstr(om, "vertex_stride", &v) && v.tag == VAL_NUM) vstride = (int)v.as.num;
        if (pb_map_get_cstr(om, "metallic", &v) && v.tag == VAL_NUM) metallic = zr_clampf((float)v.as.num, 0.0f, 1.0f);
        if (pb_map_get_cstr(om, "roughness", &v) && v.tag == VAL_NUM) roughness = zr_clampf((float)v.as.num, 0.05f, 1.0f);
        if (pb_map_get_cstr(om, "ambient", &v) && v.tag == VAL_NUM) ambient = zr_clampf((float)v.as.num, 0.0f, 1.0f);
        if (pb_map_get_cstr(om, "albedo", &v) && is_obj_type(v, OBJ_ARRAY) && AS_ARRAY(v)->len >= 3) {
            PbArray *a = AS_ARRAY(v);
            if (a->items[0].tag == VAL_NUM) alb_r = (float)a->items[0].as.num;
            if (a->items[1].tag == VAL_NUM) alb_g = (float)a->items[1].as.num;
            if (a->items[2].tag == VAL_NUM) alb_b = (float)a->items[2].as.num;
            if (alb_r > 1.5f || alb_g > 1.5f || alb_b > 1.5f) {
                alb_r /= 255.0f; alb_g /= 255.0f; alb_b /= 255.0f;
            }
        }
        if (pb_map_get_cstr(om, "light_dir", &v) && is_obj_type(v, OBJ_ARRAY) && AS_ARRAY(v)->len >= 3) {
            PbArray *a = AS_ARRAY(v);
            if (a->items[0].tag == VAL_NUM) lx = (float)a->items[0].as.num;
            if (a->items[1].tag == VAL_NUM) ly = (float)a->items[1].as.num;
            if (a->items[2].tag == VAL_NUM) lz = (float)a->items[2].as.num;
        }
        if (pb_map_get_cstr(om, "bg_color", &v) && is_obj_type(v, OBJ_ARRAY) && AS_ARRAY(v)->len >= 3) {
            PbArray *a = AS_ARRAY(v);
            if (a->items[0].tag == VAL_NUM) bg_r = (uint8_t)((int)a->items[0].as.num & 0xFF);
            if (a->items[1].tag == VAL_NUM) bg_g = (uint8_t)((int)a->items[1].as.num & 0xFF);
            if (a->items[2].tag == VAL_NUM) bg_b = (uint8_t)((int)a->items[2].as.num & 0xFF);
            if (a->len >= 4 && a->items[3].tag == VAL_NUM) bg_a = (uint8_t)((int)a->items[3].as.num & 0xFF);
        }
    }

    if (vstride <= 0) {
        size_t req_verts = (size_t)max_vidx + 1;
        if (vlen >= req_verts * 8 && (vlen % 8) == 0) vstride = 8;
        else if (vlen >= req_verts * 6 && (vlen % 6) == 0) vstride = 6;
        else if (vlen >= req_verts * 9 && (vlen % 9) == 0) vstride = 9;
        else vstride = 3;
    }

    float llen = sqrtf(lx * lx + ly * ly + lz * lz);
    if (llen > 1e-6f) { lx /= llen; ly /= llen; lz /= llen; }
    /* Half-vector H = normalize(L + V) with V = (0, 0, 1) */
    float hx = lx, hy = ly, hz = lz + 1.0f;
    float hlen = sqrtf(hx * hx + hy * hy + hz * hz);
    if (hlen > 1e-6f) { hx /= hlen; hy /= hlen; hz /= hlen; }

    size_t npix = (size_t)W * (size_t)H;
    PbBytes *rgba_obj = pb_bytes_new(vm, npix * 4, false);
    vm_push_root(vm, val_obj(rgba_obj));

    double t0 = zr_mono_sec();
    uint8_t *fb = rgba_obj->data;
    uint32_t bg_pack;
    {
        uint8_t b4[4] = { bg_r, bg_g, bg_b, bg_a };
        memcpy(&bg_pack, b4, 4);
    }
    uint32_t *fb32 = (uint32_t *)fb;
    for (size_t i = 0; i < npix; i++) fb32[i] = bg_pack;

    float *zbuf = (float *)zr_xmalloc(npix * sizeof(float));
    for (size_t i = 0; i < npix; i++) zbuf[i] = 1e30f;

    size_t n_tris = ilen / 3;
    size_t tris_drawn = 0;
    float half_w = 0.5f * (float)W;
    float half_h = 0.5f * (float)H;
    float alpha = roughness * roughness;
    float alpha2 = alpha * alpha;
    float k_ibl = ((roughness + 1.0f) * (roughness + 1.0f)) * 0.125f;
    float f0_r = 0.04f * (1.0f - metallic) + alb_r * metallic;
    float f0_g = 0.04f * (1.0f - metallic) + alb_g * metallic;
    float f0_b = 0.04f * (1.0f - metallic) + alb_b * metallic;

    for (size_t t = 0; t < n_tris; t++) {
        uint32_t i0 = indices[t * 3 + 0];
        uint32_t i1 = indices[t * 3 + 1];
        uint32_t i2 = indices[t * 3 + 2];
        if (((size_t)i0 + 1) * (size_t)vstride > vlen ||
            ((size_t)i1 + 1) * (size_t)vstride > vlen ||
            ((size_t)i2 + 1) * (size_t)vstride > vlen) {
            continue;
        }
        const double *v0 = verts + (size_t)i0 * (size_t)vstride;
        const double *v1 = verts + (size_t)i1 * (size_t)vstride;
        const double *v2 = verts + (size_t)i2 * (size_t)vstride;

        float p0x = (float)v0[0], p0y = (float)v0[1], p0z = (float)v0[2];
        float p1x = (float)v1[0], p1y = (float)v1[1], p1z = (float)v1[2];
        float p2x = (float)v2[0], p2y = (float)v2[1], p2z = (float)v2[2];

        /* Transform by 4x4 MVP */
        float c0x = mvp[0]*p0x + mvp[1]*p0y + mvp[2]*p0z + mvp[3];
        float c0y = mvp[4]*p0x + mvp[5]*p0y + mvp[6]*p0z + mvp[7];
        float c0z = mvp[8]*p0x + mvp[9]*p0y + mvp[10]*p0z + mvp[11];
        float c0w = mvp[12]*p0x + mvp[13]*p0y + mvp[14]*p0z + mvp[15];

        float c1x = mvp[0]*p1x + mvp[1]*p1y + mvp[2]*p1z + mvp[3];
        float c1y = mvp[4]*p1x + mvp[5]*p1y + mvp[6]*p1z + mvp[7];
        float c1z = mvp[8]*p1x + mvp[9]*p1y + mvp[10]*p1z + mvp[11];
        float c1w = mvp[12]*p1x + mvp[13]*p1y + mvp[14]*p1z + mvp[15];

        float c2x = mvp[0]*p2x + mvp[1]*p2y + mvp[2]*p2z + mvp[3];
        float c2y = mvp[4]*p2x + mvp[5]*p2y + mvp[6]*p2z + mvp[7];
        float c2z = mvp[8]*p2x + mvp[9]*p2y + mvp[10]*p2z + mvp[11];
        float c2w = mvp[12]*p2x + mvp[13]*p2y + mvp[14]*p2z + mvp[15];

        if (c0w <= 0.001f || c1w <= 0.001f || c2w <= 0.001f) continue;

        float iw0 = 1.0f / c0w, iw1 = 1.0f / c1w, iw2 = 1.0f / c2w;
        float sx0 = (c0x * iw0 + 1.0f) * half_w, sy0 = (1.0f - c0y * iw0) * half_h, sz0 = c0z * iw0;
        float sx1 = (c1x * iw1 + 1.0f) * half_w, sy1 = (1.0f - c1y * iw1) * half_h, sz1 = c1z * iw1;
        float sx2 = (c2x * iw2 + 1.0f) * half_w, sy2 = (1.0f - c2y * iw2) * half_h, sz2 = c2z * iw2;

        float area = (sx1 - sx0) * (sy2 - sy0) - (sy1 - sy0) * (sx2 - sx0);
        if (fabsf(area) < 1e-4f) continue;
        if (area < 0.0f) {
            /* Normalize winding so two-sided meshes render cleanly */
            float tx = sx1, ty = sy1, tz = sz1;
            sx1 = sx2; sy1 = sy2; sz1 = sz2;
            sx2 = tx;  sy2 = ty;  sz2 = tz;
            area = -area;
        }

        int min_x = (int)floorf(sx0 < sx1 ? (sx0 < sx2 ? sx0 : sx2) : (sx1 < sx2 ? sx1 : sx2));
        int max_x = (int)ceilf(sx0 > sx1 ? (sx0 > sx2 ? sx0 : sx2) : (sx1 > sx2 ? sx1 : sx2));
        int min_y = (int)floorf(sy0 < sy1 ? (sy0 < sy2 ? sy0 : sy2) : (sy1 < sy2 ? sy1 : sy2));
        int max_y = (int)ceilf(sy0 > sy1 ? (sy0 > sy2 ? sy0 : sy2) : (sy1 > sy2 ? sy1 : sy2));
        if (min_x < 0) min_x = 0;
        if (min_y < 0) min_y = 0;
        if (max_x >= W) max_x = W - 1;
        if (max_y >= H) max_y = H - 1;
        if (min_x > max_x || min_y > max_y) continue;

        /* Normal for Cook-Torrance GGX PBR lighting */
        float nx, ny, nz;
        if (vstride >= 6) {
            nx = (float)(v0[3] + v1[3] + v2[3]);
            ny = (float)(v0[4] + v1[4] + v2[4]);
            nz = (float)(v0[5] + v1[5] + v2[5]);
        } else {
            float e1x = p1x - p0x, e1y = p1y - p0y, e1z = p1z - p0z;
            float e2x = p2x - p0x, e2y = p2y - p0y, e2z = p2z - p0z;
            nx = e1y * e2z - e1z * e2y;
            ny = e1z * e2x - e1x * e2z;
            nz = e1x * e2y - e1y * e2x;
        }
        float nlen = sqrtf(nx * nx + ny * ny + nz * nz);
        if (nlen > 1e-6f) { nx /= nlen; ny /= nlen; nz /= nlen; }
        else { nx = 0.0f; ny = 0.0f; nz = 1.0f; }
        if (nz < 0.0f) { nx = -nx; ny = -ny; nz = -nz; }

        float ndotl = zr_clampf(nx * lx + ny * ly + nz * lz, 0.0f, 1.0f);
        float ndotv = zr_clampf(nz, 0.02f, 1.0f);
        float ndoth = zr_clampf(nx * hx + ny * hy + nz * hz, 0.0f, 1.0f);
        float hdotv = zr_clampf(hz, 0.0f, 1.0f);

        /* GGX Normal Distribution D + Schlick-GGX Geometry G + Schlick Fresnel F */
        float denom_d = ndoth * ndoth * (alpha2 - 1.0f) + 1.0f;
        float D = alpha2 / (3.14159265f * denom_d * denom_d + 1e-6f);
        float G = (ndotv / (ndotv * (1.0f - k_ibl) + k_ibl)) * (ndotl / (ndotl * (1.0f - k_ibl) + k_ibl));
        float omh = 1.0f - hdotv;
        float omh5 = omh * omh * omh * omh * omh;
        float fr_r = f0_r + (1.0f - f0_r) * omh5;
        float fr_g = f0_g + (1.0f - f0_g) * omh5;
        float fr_b = f0_b + (1.0f - f0_b) * omh5;
        float spec_scale = (D * G) / (4.0f * ndotv * ndotl + 1e-4f);
        float kd = (1.0f - metallic) * (1.0f - (fr_r + fr_g + fr_b) * 0.3333f);

        float cr = ambient * alb_r + (kd * alb_r / 3.14159265f + fr_r * spec_scale) * ndotl * 2.4f;
        float cg = ambient * alb_g + (kd * alb_g / 3.14159265f + fr_g * spec_scale) * ndotl * 2.4f;
        float cb = ambient * alb_b + (kd * alb_b / 3.14159265f + fr_b * spec_scale) * ndotl * 2.4f;
        /* Reinhard tone map + sRGB gamma approx */
        cr = sqrtf(cr / (1.0f + cr));
        cg = sqrtf(cg / (1.0f + cg));
        cb = sqrtf(cb / (1.0f + cb));

        uint8_t pr = (uint8_t)(zr_clampf(cr, 0.0f, 1.0f) * 255.0f);
        uint8_t pg = (uint8_t)(zr_clampf(cg, 0.0f, 1.0f) * 255.0f);
        uint8_t pb = (uint8_t)(zr_clampf(cb, 0.0f, 1.0f) * 255.0f);
        uint8_t pa4[4] = { pr, pg, pb, 255 };
        uint32_t shaded_rgba;
        memcpy(&shaded_rgba, pa4, 4);

        float inv_area = 1.0f / area;
        float dx12 = sx2 - sx1, dy12 = sy2 - sy1;
        float dx20 = sx0 - sx2, dy20 = sy0 - sy2;
        float dx01 = sx1 - sx0, dy01 = sy1 - sy0;

        for (int py = min_y; py <= max_y; py++) {
            float fy = (float)py + 0.5f;
            float fx_start = (float)min_x + 0.5f;
            float w0 = (fx_start - sx1) * dy12 - (fy - sy1) * dx12;
            float w1 = (fx_start - sx2) * dy20 - (fy - sy2) * dx20;
            float w2 = (fx_start - sx0) * dy01 - (fy - sy0) * dx01;
            size_t row_off = (size_t)py * (size_t)W;
            for (int px = min_x; px <= max_x; px++) {
                if (w0 >= 0.0f && w1 >= 0.0f && w2 >= 0.0f) {
                    float b0 = w0 * inv_area;
                    float b1 = w1 * inv_area;
                    float b2 = 1.0f - b0 - b1;
                    float z = b0 * sz0 + b1 * sz1 + b2 * sz2;
                    size_t pidx = row_off + (size_t)px;
                    if (z < zbuf[pidx]) {
                        zbuf[pidx] = z;
                        fb32[pidx] = shaded_rgba;
                    }
                }
                w0 += dy12;
                w1 += dy20;
                w2 += dy01;
            }
        }
        tris_drawn++;
    }

    double elapsed_ms = (zr_mono_sec() - t0) * 1000.0;
    double fps_equiv = elapsed_ms > 1e-6 ? (1000.0 / elapsed_ms) : 999999.0;

    PbMap *res = pb_map_new_raw(vm, 8);
    vm_push_root(vm, val_obj(res));
    pb_map_set_cstr(vm, res, "ok", val_bool(true));
    pb_map_set_cstr(vm, res, "width", val_num((double)W));
    pb_map_set_cstr(vm, res, "height", val_num((double)H));
    pb_map_set_cstr(vm, res, "pixels_rgba", val_obj(rgba_obj));
    pb_map_set_cstr(vm, res, "triangles_drawn", val_num((double)tris_drawn));
    pb_map_set_cstr(vm, res, "elapsed_ms", val_num(elapsed_ms));
    pb_map_set_cstr(vm, res, "fps_equiv", val_num(fps_equiv));
    vm_pop_roots(vm, 2);

    free(zbuf);
    free(indices);
    free(verts_owned);
    *out = val_obj(res);
    return true;
}

/* ============================================================================
 * 4. Pure-PBLang Native Unboxed AOT Bridge (`aot_build_native`)
 *    Delegates 100% to `stdlib:native/aot_unboxed_native.sapp` (ZERO gcc / C!)
 * ============================================================================ */

static bool bi_aot_build_native(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc < 2 || argc > 3 || !is_obj_type(argv[0], OBJ_STR) || !is_obj_type(argv[1], OBJ_STR)) {
        return vm_error(vm, "aot_build_native(sapp_path, out_bin_path[, opts_map]) expects (string, string[, map])");
    }
    if (!vm_check_cap(vm, vm_effective_cap(vm, CAP_FS), "aot_build_native", "fs")) return false;

    const char *eval_src =
        "let AOT = import(\"stdlib:native/aot_unboxed_native.sapp\"); "
        "return AOT.build_aot_native(__aot_src, __aot_out, __aot_opts);";

    PbVM sub;
    vm_init(&sub);
    sub.caps = vm->caps;
    if (vm->exe_path) sub.exe_path = zr_xstrdup(vm->exe_path);
    if (vm->stdlib_root) sub.stdlib_root = zr_xstrdup(vm->stdlib_root);
    if (vm->current_file) sub.current_file = zr_xstrdup(vm->current_file);

    pb_map_set_cstr(&sub, sub.globals, "__aot_src",
                    val_obj(pb_str_new(&sub, AS_STR(argv[0])->chars, AS_STR(argv[0])->len)));
    pb_map_set_cstr(&sub, sub.globals, "__aot_out",
                    val_obj(pb_str_new(&sub, AS_STR(argv[1])->chars, AS_STR(argv[1])->len)));
    PbMap *sub_opts = pb_map_new_raw(&sub, 4);
    vm_push_root(&sub, val_obj(sub_opts));
    if (argc == 3 && is_obj_type(argv[2], OBJ_MAP)) {
        PbMap *in_m = AS_MAP(argv[2]);
        for (size_t i = 0; i < in_m->used; i++) {
            if (!in_m->entries[i].active || !in_m->entries[i].key) continue;
            PbValue v = in_m->entries[i].val;
            PbValue cv = val_nil();
            if (v.tag == VAL_BOOL || v.tag == VAL_NUM) cv = v;
            else if (is_obj_type(v, OBJ_STR)) cv = val_obj(pb_str_new(&sub, AS_STR(v)->chars, AS_STR(v)->len));
            pb_map_set_cstr(&sub, sub_opts, in_m->entries[i].key->chars, cv);
        }
    }
    pb_map_set_cstr(&sub, sub.globals, "__aot_opts", val_obj(sub_opts));
    vm_pop_roots(&sub, 1);

    char cerr[512];
    cerr[0] = '\0';
    PbFunc *fn = zr_compile_sapp(&sub, eval_src, strlen(eval_src), "<aot_bridge>", cerr);
    if (!fn) {
        vm_free(&sub);
        return vm_error(vm, "aot_build_native(): %s", cerr[0] ? cerr : "compile error");
    }
    vm_push_root(&sub, val_obj(fn));
    PbValue sret = val_nil();
    bool ok = vm_run_root_func(&sub, fn, NULL, &sret);
    vm_pop_roots(&sub, 1);
    if (!ok) {
        char em[512];
        snprintf(em, sizeof(em), "%.500s", sub.err_msg[0] ? sub.err_msg : "AOT compilation failed");
        vm_free(&sub);
        return vm_error(vm, "aot_build_native(): %s", em);
    }

#ifndef _WIN32
    chmod(AS_STR(argv[1])->chars, 0755);
#endif

    PbMap *res = pb_map_new_raw(vm, 8);
    vm_push_root(vm, val_obj(res));
    if (is_obj_type(sret, OBJ_MAP)) {
        PbMap *sm = AS_MAP(sret);
        for (size_t i = 0; i < sm->used; i++) {
            if (!sm->entries[i].active || !sm->entries[i].key) continue;
            PbValue sv = sm->entries[i].val;
            PbValue dv = val_nil();
            if (sv.tag == VAL_BOOL || sv.tag == VAL_NUM) dv = sv;
            else if (is_obj_type(sv, OBJ_STR)) dv = val_obj(pb_str_new(vm, AS_STR(sv)->chars, AS_STR(sv)->len));
            pb_map_set_cstr(vm, res, sm->entries[i].key->chars, dv);
        }
    } else {
        pb_map_set_cstr(vm, res, "ok", val_bool(true));
    }
    vm_pop_roots(vm, 1);
    vm_free(&sub);
    *out = val_obj(res);
    return true;
}

#if defined(__GNUC__) && !defined(__clang__) && !defined(_WIN32)
#pragma GCC pop_options
#endif

#endif /* PBLANG_ZR_UNBOXED_AOT_H */
