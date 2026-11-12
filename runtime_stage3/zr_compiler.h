/*
 * zr_compiler.h — Native C99 .sapp -> PbFunc* (.pbck v1) Lexer, Parser, Desugarer & Compiler
 *
 * Full Zero-Rust frontend supporting:
 *   - Preprocessing: shebangs, // & nested block comments, @lang polyglot blocks
 *   - Lexer: full Unicode (Greek/Latin/CJK) identifiers, hex/bin/oct/sci numbers,
 *     single/double/multiline strings, f"..." string interpolation, all operators
 *   - Parser & Desugarer: let/const, array & map destructuring, fn/async fn, defaults,
 *     ...rest varargs, closures & mutable upvalues, if/elif/else (stmt & expr),
 *     while, loop, break/continue, for i count, for x in col, for i in a to b,
 *     for k,v in map, switch/case/default, match (literals, _, bindings, arrays,
 *     maps, OR patterns, if guards, else), goto/label/end, try/catch, defer,
 *     throw, struct, class (extends, new0..new6, new, __call), interface, enum,
 *     colon method call obj:m(...), slice seq[a..b], ?., ?[, ??, ?, |>, ternary ?:,
 *     bitwise ~ & ^ | << >>, compound assignments (+= -= *= /= %= **=), print/printf,
 *     import/export, constant folding, jump threading & tail-call emission.
 */
#ifndef PBLANG_ZR_COMPILER_H
#define PBLANG_ZR_COMPILER_H

#include "zr_pbck_io.h"

/* ---- Fast Slab Arena for AST Nodes ---- */

typedef struct ZrArenaBlock {
    struct ZrArenaBlock *next;
    size_t               used;
    size_t               cap;
    uint8_t              data[];
} ZrArenaBlock;

typedef struct {
    ZrArenaBlock *head;
} ZrArena;

static inline void zr_arena_init(ZrArena *a) { a->head = NULL; }

static void *zr_arena_alloc(ZrArena *a, size_t sz) {
    sz = (sz + 7u) & ~7u;
    if (!a->head || a->head->used + sz > a->head->cap) {
        size_t bcap = (sz > 32768u) ? (sz + 4096u) : 65536u;
        ZrArenaBlock *blk = (ZrArenaBlock *)zr_xmalloc(sizeof(ZrArenaBlock) + bcap);
        blk->next = a->head;
        blk->used = 0;
        blk->cap = bcap;
        a->head = blk;
    }
    void *p = a->head->data + a->head->used;
    a->head->used += sz;
    memset(p, 0, sz);
    return p;
}

static char *zr_arena_strndup(ZrArena *a, const char *s, size_t n) {
    char *p = (char *)zr_arena_alloc(a, n + 1);
    if (n > 0 && s) memcpy(p, s, n);
    p[n] = '\0';
    return p;
}

static inline char *zr_arena_strdup(ZrArena *a, const char *s) {
    return zr_arena_strndup(a, s ? s : "", s ? strlen(s) : 0);
}

static void zr_arena_free(ZrArena *a) {
    ZrArenaBlock *cur = a->head;
    while (cur) {
        ZrArenaBlock *nx = cur->next;
        free(cur);
        cur = nx;
    }
    a->head = NULL;
}

/* ---- Preprocessor: Comments, Shebang & @lang Polyglot Extraction ---- */

static bool zr_preprocess_sapp(const char *raw,
                               size_t raw_len,
                               char **out_clean,
                               PbPolyBlock **out_polys,
                               size_t *out_poly_count,
                               char err_out[512]) {
    char *buf = (char *)zr_xmalloc(raw_len + 1);
    memcpy(buf, raw, raw_len);
    buf[raw_len] = '\0';

    PbPolyBlock *polys = NULL;
    size_t poly_len = 0, poly_cap = 0;

    size_t i = 0;
    uint32_t line = 1;

    /* Strip line-1 shebang */
    if (raw_len >= 2 && buf[0] == '#' && buf[1] == '!') {
        while (i < raw_len && buf[i] != '\n') {
            buf[i++] = ' ';
        }
    }

    while (i < raw_len) {
        char c = buf[i];
        if (c == '\n') {
            line++;
            i++;
            continue;
        }
        /* Skip string literals (including f"..." with nested {...}) */
        if ((c == 'f' && i + 1 < raw_len && (buf[i + 1] == '"' || buf[i + 1] == '\'')) ||
            c == '"' || c == '\'') {
            bool is_f = (c == 'f');
            if (is_f) i++;
            char q = buf[i++];
            while (i < raw_len) {
                char sc = buf[i];
                if (sc == '\n') line++;
                if (sc == '\\' && i + 1 < raw_len) {
                    if (buf[i + 1] == '\n') line++;
                    i += 2;
                    continue;
                }
                if (is_f && sc == '{') {
                    int bdepth = 1;
                    i++;
                    while (i < raw_len && bdepth > 0) {
                        char ic = buf[i];
                        if (ic == '\n') line++;
                        if (ic == '"' || ic == '\'') {
                            char iq = ic;
                            i++;
                            while (i < raw_len && buf[i] != iq) {
                                if (buf[i] == '\n') line++;
                                if (buf[i] == '\\' && i + 1 < raw_len) {
                                    if (buf[i + 1] == '\n') line++;
                                    i += 2;
                                    continue;
                                }
                                i++;
                            }
                            if (i < raw_len) i++;
                            continue;
                        }
                        if (ic == '{') bdepth++;
                        else if (ic == '}') bdepth--;
                        i++;
                    }
                    continue;
                }
                if (sc == q) {
                    i++;
                    break;
                }
                i++;
            }
            continue;
        }

        /* Line comment // */
        if (c == '/' && i + 1 < raw_len && buf[i + 1] == '/') {
            while (i < raw_len && buf[i] != '\n') {
                buf[i++] = ' ';
            }
            continue;
        }

        /* Nested block comment */
        if (c == '/' && i + 1 < raw_len && buf[i + 1] == '*') {
            uint32_t start_line = line;
            int depth = 1;
            buf[i++] = ' ';
            buf[i++] = ' ';
            while (i < raw_len && depth > 0) {
                if (buf[i] == '\n') {
                    line++;
                    i++;
                } else if (buf[i] == '/' && i + 1 < raw_len && buf[i + 1] == '*') {
                    depth++;
                    buf[i++] = ' ';
                    buf[i++] = ' ';
                } else if (buf[i] == '*' && i + 1 < raw_len && buf[i + 1] == '/') {
                    depth--;
                    buf[i++] = ' ';
                    buf[i++] = ' ';
                } else {
                    buf[i++] = ' ';
                }
            }
            if (depth > 0) {
                free(buf);
                for (size_t p = 0; p < poly_len; p++) {
                    free(polys[p].lang); free(polys[p].name); free(polys[p].code);
                    for (size_t a = 0; a < polys[p].attr_count; a++) {
                        free(polys[p].attr_keys[a]); free(polys[p].attr_vals[a]);
                    }
                    free(polys[p].attr_keys); free(polys[p].attr_vals);
                }
                free(polys);
                snprintf(err_out, 512, "syntax error at line %u: unterminated block comment", start_line);
                return false;
            }
            continue;
        }

        /* Polyglot block @lang [name] [(attrs) | [attrs]] { ... } */
        if (c == '@' && i + 1 < raw_len && (isalnum((unsigned char)buf[i + 1]) || buf[i + 1] == '_')) {
            size_t start_pos = i;
            uint32_t blk_line = line;
            size_t p = i + 1;
            while (p < raw_len && (isalnum((unsigned char)buf[p]) || buf[p] == '_' ||
                                   buf[p] == '+' || buf[p] == '-' || buf[p] == '#' || buf[p] == '.')) p++;
            char *lang = zr_xstrndup(buf + i + 1, p - (i + 1));

            while (p < raw_len && (buf[p] == ' ' || buf[p] == '\t')) p++;
            char *bname = NULL;
            if (p < raw_len && (isalpha((unsigned char)buf[p]) || buf[p] == '_')) {
                size_t ns = p;
                while (p < raw_len && (isalnum((unsigned char)buf[p]) || buf[p] == '_')) p++;
                bname = zr_xstrndup(buf + ns, p - ns);
            }
            while (p < raw_len && (buf[p] == ' ' || buf[p] == '\t')) p++;

            char **akeys = NULL, **avals = NULL;
            size_t acount = 0;
            if (p < raw_len && (buf[p] == '(' || buf[p] == '[')) {
                char close_ch = (buf[p] == '[') ? ']' : ')';
                p++;
                while (p < raw_len && buf[p] != close_ch) {
                    while (p < raw_len && (isspace((unsigned char)buf[p]) || buf[p] == ',')) p++;
                    if (p >= raw_len || buf[p] == close_ch) break;
                    size_t ks = p;
                    while (p < raw_len && (isalnum((unsigned char)buf[p]) || buf[p] == '_' || buf[p] == '-')) p++;
                    if (p == ks) { p++; continue; }
                    char *k = zr_xstrndup(buf + ks, p - ks);
                    while (p < raw_len && (buf[p] == ' ' || buf[p] == '\t')) p++;
                    char *v = zr_xstrdup("true");
                    if (p < raw_len && (buf[p] == '=' || buf[p] == ':')) {
                        p++;
                        while (p < raw_len && (buf[p] == ' ' || buf[p] == '\t')) p++;
                        free(v);
                        if (p < raw_len && (buf[p] == '"' || buf[p] == '\'')) {
                            char aq = buf[p++];
                            size_t vs = p;
                            while (p < raw_len && buf[p] != aq) p++;
                            v = zr_xstrndup(buf + vs, p - vs);
                            if (p < raw_len && buf[p] == aq) p++;
                        } else {
                            size_t vs = p;
                            while (p < raw_len && buf[p] != ',' && buf[p] != close_ch && !isspace((unsigned char)buf[p])) p++;
                            v = zr_xstrndup(buf + vs, p - vs);
                        }
                    }
                    if (!bname && strcmp(k, "name") == 0 && v[0] != '\0') {
                        bname = zr_xstrdup(v);
                    }
                    akeys = (char **)zr_xrealloc(akeys, (acount + 1) * sizeof(char *));
                    avals = (char **)zr_xrealloc(avals, (acount + 1) * sizeof(char *));
                    akeys[acount] = k;
                    avals[acount] = v;
                    acount++;
                }
                if (p < raw_len && buf[p] == close_ch) p++;
            }
            while (p < raw_len && isspace((unsigned char)buf[p])) p++;
            if (p < raw_len && buf[p] == '{') {
                size_t code_start = p + 1;
                int bdepth = 1;
                p++;
                while (p < raw_len && bdepth > 0) {
                    char pc = buf[p];
                    if (pc == '"') {
                        p++;
                        while (p < raw_len && buf[p] != '"') {
                            if (buf[p] == '\\' && p + 1 < raw_len) p += 2;
                            else p++;
                        }
                        if (p < raw_len) p++;
                        continue;
                    }
                    if (pc == '{') bdepth++;
                    else if (pc == '}') bdepth--;
                    if (bdepth > 0) p++;
                }
                if (bdepth == 0) {
                    char *pcode = zr_xstrndup(buf + code_start, p - code_start);
                    p++; /* consume '}' */
                    if (poly_len >= poly_cap) {
                        poly_cap = poly_cap ? poly_cap * 2 : 4;
                        polys = (PbPolyBlock *)zr_xrealloc(polys, poly_cap * sizeof(PbPolyBlock));
                    }
                    polys[poly_len].lang = lang;
                    polys[poly_len].name = bname;
                    polys[poly_len].attr_keys = akeys;
                    polys[poly_len].attr_vals = avals;
                    polys[poly_len].attr_count = acount;
                    polys[poly_len].code = pcode;
                    polys[poly_len].line = blk_line;
                    poly_len++;
                    for (size_t k = start_pos; k < p; k++) {
                        if (buf[k] == '\n') line++;
                        else buf[k] = ' ';
                    }
                    i = p;
                    continue;
                }
            }
            free(lang);
            free(bname);
            for (size_t a = 0; a < acount; a++) { free(akeys[a]); free(avals[a]); }
            free(akeys);
            free(avals);
        }
        i++;
    }

    *out_clean = buf;
    *out_polys = polys;
    *out_poly_count = poly_len;
    return true;
}

/* ---- Lexer ---- */

typedef enum {
    TOK_EOF = 0,
    TOK_NUM,
    TOK_STR,
    TOK_FSTR,       /* raw inner text of f"..." */
    TOK_IDENT,
    /* Keywords */
    TOK_LET, TOK_CONST, TOK_FN, TOK_ASYNC, TOK_AWAIT, TOK_RETURN,
    TOK_IF, TOK_ELIF, TOK_ELSE, TOK_WHILE, TOK_FOR, TOK_IN,
    TOK_LOOP, TOK_BREAK, TOK_CONTINUE,
    TOK_TRY, TOK_CATCH, TOK_THROW, TOK_DEFER,
    TOK_PRINT, TOK_PRINTF, TOK_EXPORT,
    TOK_STRUCT, TOK_CLASS, TOK_INTERFACE, TOK_ENUM,
    TOK_AND, TOK_OR, TOK_NOT, TOK_TRUE, TOK_FALSE, TOK_NIL,
    /* Punctuation & Operators */
    TOK_LPAREN, TOK_RPAREN, TOK_LBRACK, TOK_RBRACK, TOK_LBRACE, TOK_RBRACE,
    TOK_COMMA, TOK_SEMI, TOK_COLON, TOK_DOT, TOK_DOTDOT, TOK_ELLIPSIS,
    TOK_PLUS, TOK_MINUS, TOK_STAR, TOK_SLASH, TOK_PERCENT, TOK_STARSTAR,
    TOK_PLUSEQ, TOK_MINUSEQ, TOK_STAREQ, TOK_SLASHEQ, TOK_PERCENTEQ, TOK_STARSTAREQ,
    TOK_EQ, TOK_EQEQ, TOK_BANGEQ, TOK_LT, TOK_LTEQ, TOK_GT, TOK_GTEQ,
    TOK_AMP, TOK_PIPE, TOK_CARET, TOK_TILDE, TOK_SHL, TOK_SHR,
    TOK_BANG, TOK_QMARK, TOK_QDOT, TOK_QLBRACK, TOK_QQ,
    TOK_PIPEGT, TOK_ARROW, TOK_FATARROW
} ZrTokKind;

typedef struct {
    ZrTokKind   kind;
    const char *lexeme;   /* arena-allocated null-terminated string */
    size_t      lex_len;
    double      num_val;
    uint32_t    line;
} ZrToken;

typedef struct {
    const char *src;
    size_t      len;
    size_t      pos;
    uint32_t    line;
    ZrArena    *arena;
    ZrToken    *tokens;
    size_t      tok_len;
    size_t      tok_cap;
    char        err[512];
} ZrLexer;

static void zr_lex_push(ZrLexer *lx, ZrToken t) {
    if (lx->tok_len >= lx->tok_cap) {
        lx->tok_cap = lx->tok_cap ? lx->tok_cap * 2 : 256;
        lx->tokens = (ZrToken *)zr_xrealloc(lx->tokens, lx->tok_cap * sizeof(ZrToken));
    }
    lx->tokens[lx->tok_len++] = t;
}

static bool zr_unescape_string(ZrArena *arena, const char *raw, size_t rlen, const char **out_s, size_t *out_n) {
    ZrBuf b;
    zr_buf_init(&b);
    size_t i = 0;
    while (i < rlen) {
        if (raw[i] != '\\') {
            zr_buf_push(&b, raw[i++]);
            continue;
        }
        i++;
        if (i >= rlen) { zr_buf_push(&b, '\\'); break; }
        char e = raw[i++];
        switch (e) {
            case 'n':  zr_buf_push(&b, '\n'); break;
            case 'r':  zr_buf_push(&b, '\r'); break;
            case 't':  zr_buf_push(&b, '\t'); break;
            case '\\': zr_buf_push(&b, '\\'); break;
            case '"':  zr_buf_push(&b, '"');  break;
            case '\'': zr_buf_push(&b, '\''); break;
            case '0':  zr_buf_push(&b, '\0'); break;
            case 'b':  zr_buf_push(&b, '\b'); break;
            case 'f':  zr_buf_push(&b, '\f'); break;
            case '{':  zr_buf_push(&b, '{');  break;
            case '}':  zr_buf_push(&b, '}');  break;
            case 'x': {
                if (i + 2 <= rlen) {
                    int h1 = zr_hex_val(raw[i]), h2 = zr_hex_val(raw[i + 1]);
                    if (h1 >= 0 && h2 >= 0) {
                        zr_buf_push_u8(&b, (uint8_t)((h1 << 4) | h2));
                        i += 2;
                        break;
                    }
                }
                zr_buf_push(&b, 'x');
                break;
            }
            case 'u': {
                uint32_t cp = 0;
                bool ok = false;
                if (i < rlen && raw[i] == '{') {
                    i++;
                    size_t digits = 0;
                    while (i < rlen && raw[i] != '}' && digits < 6) {
                        int hv = zr_hex_val(raw[i]);
                        if (hv < 0) break;
                        cp = (cp << 4) | (uint32_t)hv;
                        i++;
                        digits++;
                    }
                    if (i < rlen && raw[i] == '}' && digits > 0) {
                        i++;
                        ok = true;
                    }
                } else if (i + 4 <= rlen) {
                    ok = true;
                    for (int k = 0; k < 4; k++) {
                        int hv = zr_hex_val(raw[i + k]);
                        if (hv < 0) { ok = false; break; }
                        cp = (cp << 4) | (uint32_t)hv;
                    }
                    if (ok) i += 4;
                }
                if (ok) {
                    char u8[4];
                    size_t un = zr_utf8_encode(cp, u8);
                    zr_buf_append(&b, u8, un);
                } else {
                    zr_buf_push(&b, 'u');
                }
                break;
            }
            default:
                /* Keep unknown escapes like \d, \w, \s? Wait: in PBLang strings "\\d+" has \\ -> \,
                   what if someone writes "\$" or "\/"? Emit the escaped char! */
                zr_buf_push(&b, e);
                break;
        }
    }
    *out_s = zr_arena_strndup(arena, b.data ? b.data : "", b.len);
    *out_n = b.len;
    zr_buf_free(&b);
    return true;
}

static ZrTokKind zr_classify_keyword(const char *s, size_t n) {
    switch (n) {
        case 2:
            if (memcmp(s, "fn", 2) == 0) return TOK_FN;
            if (memcmp(s, "if", 2) == 0) return TOK_IF;
            if (memcmp(s, "in", 2) == 0) return TOK_IN;
            if (memcmp(s, "or", 2) == 0) return TOK_OR;
            break;
        case 3:
            if (memcmp(s, "let", 3) == 0) return TOK_LET;
            if (memcmp(s, "for", 3) == 0) return TOK_FOR;
            if (memcmp(s, "try", 3) == 0) return TOK_TRY;
            if (memcmp(s, "and", 3) == 0) return TOK_AND;
            if (memcmp(s, "not", 3) == 0) return TOK_NOT;
            if (memcmp(s, "nil", 3) == 0) return TOK_NIL;
            break;
        case 4:
            if (memcmp(s, "elif", 4) == 0) return TOK_ELIF;
            if (memcmp(s, "else", 4) == 0) return TOK_ELSE;
            if (memcmp(s, "loop", 4) == 0) return TOK_LOOP;
            if (memcmp(s, "true", 4) == 0) return TOK_TRUE;
            if (memcmp(s, "enum", 4) == 0) return TOK_ENUM;
            break;
        case 5:
            if (memcmp(s, "const", 5) == 0) return TOK_CONST;
            if (memcmp(s, "async", 5) == 0) return TOK_ASYNC;
            if (memcmp(s, "await", 5) == 0) return TOK_AWAIT;
            if (memcmp(s, "while", 5) == 0) return TOK_WHILE;
            if (memcmp(s, "break", 5) == 0) return TOK_BREAK;
            if (memcmp(s, "catch", 5) == 0) return TOK_CATCH;
            if (memcmp(s, "throw", 5) == 0) return TOK_THROW;
            if (memcmp(s, "defer", 5) == 0) return TOK_DEFER;
            if (memcmp(s, "print", 5) == 0) return TOK_PRINT;
            if (memcmp(s, "class", 5) == 0) return TOK_CLASS;
            if (memcmp(s, "false", 5) == 0) return TOK_FALSE;
            break;
        case 6:
            if (memcmp(s, "return", 6) == 0) return TOK_RETURN;
            if (memcmp(s, "printf", 6) == 0) return TOK_PRINTF;
            if (memcmp(s, "export", 6) == 0) return TOK_EXPORT;
            if (memcmp(s, "struct", 6) == 0) return TOK_STRUCT;
            break;
        case 8:
            if (memcmp(s, "continue", 8) == 0) return TOK_CONTINUE;
            break;
        case 9:
            if (memcmp(s, "interface", 9) == 0) return TOK_INTERFACE;
            break;
        default:
            break;
    }
    return TOK_IDENT;
}

static bool zr_lex_all(ZrLexer *lx) {
    while (lx->pos < lx->len) {
        char c = lx->src[lx->pos];
        if (c == ' ' || c == '\t' || c == '\r') {
            lx->pos++;
            continue;
        }
        if (c == '\n') {
            lx->line++;
            lx->pos++;
            continue;
        }

        uint32_t tline = lx->line;

        /* f"..." / f'...' interpolated string */
        if (c == 'f' && lx->pos + 1 < lx->len && (lx->src[lx->pos + 1] == '"' || lx->src[lx->pos + 1] == '\'')) {
            char q = lx->src[lx->pos + 1];
            lx->pos += 2;
            size_t start = lx->pos;
            bool closed = false;
            while (lx->pos < lx->len) {
                char sc = lx->src[lx->pos];
                if (sc == '\n') lx->line++;
                if (sc == '\\' && lx->pos + 1 < lx->len) {
                    if (lx->src[lx->pos + 1] == '\n') lx->line++;
                    lx->pos += 2;
                    continue;
                }
                if (sc == '{') {
                    if (lx->pos + 1 < lx->len && lx->src[lx->pos + 1] == '{') {
                        lx->pos += 2;
                        continue;
                    }
                    int bdepth = 1;
                    lx->pos++;
                    while (lx->pos < lx->len && bdepth > 0) {
                        char ic = lx->src[lx->pos];
                        if (ic == '\n') lx->line++;
                        if (ic == '"' || ic == '\'') {
                            char iq = ic;
                            lx->pos++;
                            while (lx->pos < lx->len && lx->src[lx->pos] != iq) {
                                if (lx->src[lx->pos] == '\n') lx->line++;
                                if (lx->src[lx->pos] == '\\' && lx->pos + 1 < lx->len) {
                                    if (lx->src[lx->pos + 1] == '\n') lx->line++;
                                    lx->pos += 2;
                                    continue;
                                }
                                lx->pos++;
                            }
                            if (lx->pos < lx->len) lx->pos++;
                            continue;
                        }
                        if (ic == '{') bdepth++;
                        else if (ic == '}') bdepth--;
                        lx->pos++;
                    }
                    continue;
                }
                if (sc == q) {
                    closed = true;
                    break;
                }
                lx->pos++;
            }
            if (!closed) {
                snprintf(lx->err, sizeof(lx->err), "line %u: unterminated f-string literal", tline);
                return false;
            }
            size_t flen = lx->pos - start;
            const char *fraw = zr_arena_strndup(lx->arena, lx->src + start, flen);
            lx->pos++; /* consume closing quote */
            ZrToken tok = { TOK_FSTR, fraw, flen, 0.0, tline };
            zr_lex_push(lx, tok);
            continue;
        }

        /* String literal "..." or '...' */
        if (c == '"' || c == '\'') {
            char q = c;
            lx->pos++;
            size_t start = lx->pos;
            bool closed = false;
            while (lx->pos < lx->len) {
                char sc = lx->src[lx->pos];
                if (sc == '\n') lx->line++;
                if (sc == '\\' && lx->pos + 1 < lx->len) {
                    if (lx->src[lx->pos + 1] == '\n') lx->line++;
                    lx->pos += 2;
                    continue;
                }
                if (sc == q) {
                    closed = true;
                    break;
                }
                lx->pos++;
            }
            if (!closed) {
                snprintf(lx->err, sizeof(lx->err), "line %u: unterminated string literal", tline);
                return false;
            }
            size_t rlen = lx->pos - start;
            lx->pos++;
            const char *unesc = NULL;
            size_t ulen = 0;
            zr_unescape_string(lx->arena, lx->src + start, rlen, &unesc, &ulen);
            ZrToken tok = { TOK_STR, unesc, ulen, 0.0, tline };
            zr_lex_push(lx, tok);
            continue;
        }

        /* Number literal */
        if (c >= '0' && c <= '9') {
            size_t start = lx->pos;
            double val = 0.0;
            if (c == '0' && lx->pos + 1 < lx->len &&
                (lx->src[lx->pos + 1] == 'x' || lx->src[lx->pos + 1] == 'X' ||
                 lx->src[lx->pos + 1] == 'b' || lx->src[lx->pos + 1] == 'B' ||
                 lx->src[lx->pos + 1] == 'o' || lx->src[lx->pos + 1] == 'O')) {
                char mode = (char)tolower((unsigned char)lx->src[lx->pos + 1]);
                uint32_t base = (mode == 'x') ? 16 : (mode == 'b') ? 2 : 8;
                lx->pos += 2;
                uint64_t u = 0;
                size_t ndig = 0;
                while (lx->pos < lx->len) {
                    char dc = lx->src[lx->pos];
                    if (dc == '_') { lx->pos++; continue; }
                    int hv = zr_hex_val(dc);
                    if (hv < 0 || (uint32_t)hv >= base) break;
                    u = u * base + (uint64_t)hv;
                    lx->pos++;
                    ndig++;
                }
                if (ndig == 0) {
                    snprintf(lx->err, sizeof(lx->err), "line %u: invalid base-%u number literal", tline, base);
                    return false;
                }
                val = (double)u;
            } else {
                ZrBuf clean;
                zr_buf_init(&clean);
                while (lx->pos < lx->len && ((lx->src[lx->pos] >= '0' && lx->src[lx->pos] <= '9') || lx->src[lx->pos] == '_')) {
                    if (lx->src[lx->pos] != '_') zr_buf_push(&clean, lx->src[lx->pos]);
                    lx->pos++;
                }
                /* Fractional part — do NOT consume '..' range operator! */
                if (lx->pos + 1 < lx->len && lx->src[lx->pos] == '.' &&
                    lx->src[lx->pos + 1] >= '0' && lx->src[lx->pos + 1] <= '9') {
                    zr_buf_push(&clean, '.');
                    lx->pos++;
                    while (lx->pos < lx->len && ((lx->src[lx->pos] >= '0' && lx->src[lx->pos] <= '9') || lx->src[lx->pos] == '_')) {
                        if (lx->src[lx->pos] != '_') zr_buf_push(&clean, lx->src[lx->pos]);
                        lx->pos++;
                    }
                }
                if (lx->pos < lx->len && (lx->src[lx->pos] == 'e' || lx->src[lx->pos] == 'E')) {
                    zr_buf_push(&clean, 'e');
                    lx->pos++;
                    if (lx->pos < lx->len && (lx->src[lx->pos] == '+' || lx->src[lx->pos] == '-')) {
                        zr_buf_push(&clean, lx->src[lx->pos++]);
                    }
                    while (lx->pos < lx->len && ((lx->src[lx->pos] >= '0' && lx->src[lx->pos] <= '9') || lx->src[lx->pos] == '_')) {
                        if (lx->src[lx->pos] != '_') zr_buf_push(&clean, lx->src[lx->pos]);
                        lx->pos++;
                    }
                }
                val = strtod(clean.data ? clean.data : "0", NULL);
                zr_buf_free(&clean);
            }
            size_t nlen = lx->pos - start;
            const char *nlex = zr_arena_strndup(lx->arena, lx->src + start, nlen);
            ZrToken tok = { TOK_NUM, nlex, nlen, val, tline };
            zr_lex_push(lx, tok);
            continue;
        }

        /* Identifier or Keyword (ASCII + Full Unicode) */
        size_t adv = 0;
        uint32_t cp = zr_utf8_decode(lx->src + lx->pos, lx->len - lx->pos, &adv);
        if (adv > 0 && zr_is_ident_start_cp(cp)) {
            size_t start = lx->pos;
            lx->pos += adv;
            while (lx->pos < lx->len) {
                size_t cadv = 0;
                uint32_t ccp = zr_utf8_decode(lx->src + lx->pos, lx->len - lx->pos, &cadv);
                if (cadv == 0 || !zr_is_ident_continue_cp(ccp)) break;
                lx->pos += cadv;
            }
            size_t ilen = lx->pos - start;
            const char *ilex = zr_arena_strndup(lx->arena, lx->src + start, ilen);
            ZrTokKind kw = zr_classify_keyword(ilex, ilen);
            ZrToken tok = { kw, ilex, ilen, 0.0, tline };
            zr_lex_push(lx, tok);
            continue;
        }

        /* 3-char operators */
        if (lx->pos + 3 <= lx->len) {
            const char *p3 = lx->src + lx->pos;
            if (memcmp(p3, "...", 3) == 0) {
                lx->pos += 3;
                ZrToken tok = { TOK_ELLIPSIS, "...", 3, 0.0, tline };
                zr_lex_push(lx, tok);
                continue;
            }
            if (memcmp(p3, "**=", 3) == 0) {
                lx->pos += 3;
                ZrToken tok = { TOK_STARSTAREQ, "**=", 3, 0.0, tline };
                zr_lex_push(lx, tok);
                continue;
            }
        }

        /* 2-char operators */
        if (lx->pos + 2 <= lx->len) {
            char c0 = lx->src[lx->pos], c1 = lx->src[lx->pos + 1];
            ZrTokKind k2 = TOK_EOF;
            const char *s2 = NULL;
            if (c0 == '=' && c1 == '=')      { k2 = TOK_EQEQ;      s2 = "=="; }
            else if (c0 == '!' && c1 == '=') { k2 = TOK_BANGEQ;    s2 = "!="; }
            else if (c0 == '<' && c1 == '=') { k2 = TOK_LTEQ;      s2 = "<="; }
            else if (c0 == '>' && c1 == '=') { k2 = TOK_GTEQ;      s2 = ">="; }
            else if (c0 == '&' && c1 == '&') { k2 = TOK_AND;       s2 = "&&"; }
            else if (c0 == '|' && c1 == '|') { k2 = TOK_OR;        s2 = "||"; }
            else if (c0 == '+' && c1 == '=') { k2 = TOK_PLUSEQ;    s2 = "+="; }
            else if (c0 == '-' && c1 == '=') { k2 = TOK_MINUSEQ;   s2 = "-="; }
            else if (c0 == '*' && c1 == '=') { k2 = TOK_STAREQ;    s2 = "*="; }
            else if (c0 == '/' && c1 == '=') { k2 = TOK_SLASHEQ;   s2 = "/="; }
            else if (c0 == '%' && c1 == '=') { k2 = TOK_PERCENTEQ; s2 = "%="; }
            else if (c0 == '*' && c1 == '*') { k2 = TOK_STARSTAR;  s2 = "**"; }
            else if (c0 == '|' && c1 == '>') { k2 = TOK_PIPEGT;    s2 = "|>"; }
            else if (c0 == '?' && c1 == '.') { k2 = TOK_QDOT;      s2 = "?."; }
            else if (c0 == '?' && c1 == '[') { k2 = TOK_QLBRACK;   s2 = "?["; }
            else if (c0 == '?' && c1 == '?') { k2 = TOK_QQ;        s2 = "??"; }
            else if (c0 == '.' && c1 == '.') { k2 = TOK_DOTDOT;    s2 = ".."; }
            else if (c0 == '-' && c1 == '>') { k2 = TOK_ARROW;     s2 = "->"; }
            else if (c0 == '=' && c1 == '>') { k2 = TOK_FATARROW;  s2 = "=>"; }
            else if (c0 == '<' && c1 == '<') { k2 = TOK_SHL;       s2 = "<<"; }
            else if (c0 == '>' && c1 == '>') { k2 = TOK_SHR;       s2 = ">>"; }
            if (k2 != TOK_EOF) {
                lx->pos += 2;
                ZrToken tok = { k2, s2, 2, 0.0, tline };
                zr_lex_push(lx, tok);
                continue;
            }
        }

        /* 1-char punctuation */
        ZrTokKind k1 = TOK_EOF;
        const char *s1 = NULL;
        switch (c) {
            case '(': k1 = TOK_LPAREN;  s1 = "("; break;
            case ')': k1 = TOK_RPAREN;  s1 = ")"; break;
            case '[': k1 = TOK_LBRACK;  s1 = "["; break;
            case ']': k1 = TOK_RBRACK;  s1 = "]"; break;
            case '{': k1 = TOK_LBRACE;  s1 = "{"; break;
            case '}': k1 = TOK_RBRACE;  s1 = "}"; break;
            case ',': k1 = TOK_COMMA;   s1 = ","; break;
            case ';': k1 = TOK_SEMI;    s1 = ";"; break;
            case ':': k1 = TOK_COLON;   s1 = ":"; break;
            case '.': k1 = TOK_DOT;     s1 = "."; break;
            case '+': k1 = TOK_PLUS;    s1 = "+"; break;
            case '-': k1 = TOK_MINUS;   s1 = "-"; break;
            case '*': k1 = TOK_STAR;    s1 = "*"; break;
            case '/': k1 = TOK_SLASH;   s1 = "/"; break;
            case '%': k1 = TOK_PERCENT; s1 = "%"; break;
            case '=': k1 = TOK_EQ;      s1 = "="; break;
            case '<': k1 = TOK_LT;      s1 = "<"; break;
            case '>': k1 = TOK_GT;      s1 = ">"; break;
            case '&': k1 = TOK_AMP;     s1 = "&"; break;
            case '|': k1 = TOK_PIPE;    s1 = "|"; break;
            case '^': k1 = TOK_CARET;   s1 = "^"; break;
            case '~': k1 = TOK_TILDE;   s1 = "~"; break;
            case '!': k1 = TOK_BANG;    s1 = "!"; break;
            case '?': k1 = TOK_QMARK;   s1 = "?"; break;
            default:
                snprintf(lx->err, sizeof(lx->err), "line %u: unexpected character '%c' (0x%02x)", tline, c, (unsigned char)c);
                return false;
        }
        lx->pos++;
        ZrToken tok = { k1, s1, 1, 0.0, tline };
        zr_lex_push(lx, tok);
    }

    ZrToken eof_tok = { TOK_EOF, "", 0, 0.0, lx->line };
    zr_lex_push(lx, eof_tok);
    return true;
}

/* ---- AST Definitions ---- */

typedef struct ZrExpr  ZrExpr;
typedef struct ZrStmt  ZrStmt;
typedef struct ZrParam ZrParam;
typedef struct ZrPat   ZrPat;

struct ZrParam {
    const char *name;
    ZrExpr     *def_expr; /* nullable */
    bool        is_vararg;
};

typedef enum {
    PAT_WILD = 0,
    PAT_LIT,
    PAT_VAR,
    PAT_ARRAY,
    PAT_MAP,
    PAT_OR
} ZrPatKind;

struct ZrPat {
    ZrPatKind    kind;
    ZrExpr      *lit_expr;
    const char  *var_name;
    ZrPat      **sub_pats;
    const char **map_keys;
    size_t       sub_count;
    const char  *rest_name; /* for [a, b, ...rest] */
};

typedef struct {
    ZrPat  *pat;
    ZrExpr *guard;
    ZrExpr *body;
} ZrMatchArm;

typedef enum {
    EX_NIL = 0,
    EX_BOOL,
    EX_NUM,
    EX_STR,
    EX_VAR,
    EX_UNARY,
    EX_BINARY,
    EX_LOGICAL,   /* op: TOK_AND, TOK_OR, TOK_QQ */
    EX_TERNARY,
    EX_ARRAY,
    EX_MAP,
    EX_INDEX,
    EX_SAFE_INDEX,
    EX_ATTR,
    EX_SAFE_ATTR,
    EX_UNWRAP,    /* expr? */
    EX_CALL,
    EX_METHOD_CALL, /* obj:method(args) */
    EX_FN,
    EX_IF,
    EX_TRY,
    EX_BLOCK,
    EX_MATCH
} ZrExprKind;

struct ZrExpr {
    ZrExprKind   kind;
    uint32_t     line;
    union {
        bool     b;
        double   num;
        struct { const char *s; size_t len; } str;
        const char *var;
        struct { ZrTokKind op; ZrExpr *rhs; } unary;
        struct { ZrTokKind op; ZrExpr *lhs; ZrExpr *rhs; } bin;
        struct { ZrExpr *cond; ZrExpr *t_br; ZrExpr *f_br; } tern;
        struct { ZrExpr **items; size_t count; } arr;
        struct { ZrExpr **keys; ZrExpr **vals; size_t count; } map;
        struct { ZrExpr *obj; ZrExpr *idx; } idx;
        struct { ZrExpr *obj; const char *attr; } attr;
        struct { ZrExpr *callee; ZrExpr **args; size_t argc; } call;
        struct { ZrExpr *recv; const char *method; ZrExpr **args; size_t argc; } mcall;
        struct {
            const char *name; /* nullable */
            ZrParam    *params;
            size_t      param_count;
            bool        is_async;
            ZrStmt    **body;
            size_t      body_count;
        } fn;
        struct { ZrExpr *cond; ZrExpr *then_e; ZrExpr *else_e; } if_e;
        struct { ZrExpr *try_e; const char *err_var; ZrExpr *catch_e; } try_e;
        struct { ZrStmt **stmts; size_t stmt_count; ZrExpr *tail; } blk;
        struct { ZrExpr *scrut; ZrMatchArm *arms; size_t arm_count; } match_e;
    } u;
};

typedef enum {
    ST_EXPR = 0,
    ST_LET,
    ST_ASSIGN,     /* var = rhs */
    ST_SET_ATTR,   /* obj.attr = rhs */
    ST_SET_INDEX,  /* obj[idx] = rhs */
    ST_BLOCK,
    ST_SEQ,        /* sequence WITHOUT introducing lexical block scope */
    ST_IF,
    ST_WHILE,
    ST_LOOP,
    ST_FOR_COUNT,  /* for i count { body } */
    ST_FOR_IN,     /* for x in col { body } */
    ST_FOR_KV,     /* for k, v in map { body } */
    ST_BREAK,
    ST_CONTINUE,
    ST_RETURN,
    ST_PRINT,
    ST_PRINTF,
    ST_DEFER,
    ST_THROW,
    ST_EXPORT,
    ST_FN
} ZrStmtKind;

struct ZrStmt {
    ZrStmtKind kind;
    uint32_t   line;
    union {
        ZrExpr *expr;
        struct { const char *name; ZrExpr *init; bool is_const; } let_s;
        struct { const char *name; ZrExpr *rhs; } assign_s;
        struct { ZrExpr *obj; const char *attr; ZrExpr *rhs; } sattr;
        struct { ZrExpr *obj; ZrExpr *idx; ZrExpr *rhs; } sidx;
        struct { ZrStmt **stmts; size_t count; } blk;
        struct { ZrExpr *cond; ZrStmt **then_b; size_t then_n; ZrStmt **else_b; size_t else_n; } if_s;
        struct { ZrExpr *cond; ZrStmt **body; size_t body_n; } while_s;
        struct { ZrStmt **body; size_t body_n; } loop_s;
        struct { const char *var; ZrExpr *count_e; ZrStmt **body; size_t body_n; } for_c;
        struct { const char *var; ZrExpr *col_e; ZrStmt **body; size_t body_n; } for_in;
        struct { const char *kvar; const char *vvar; ZrExpr *map_e; ZrStmt **body; size_t body_n; } for_kv;
        struct { int prec; ZrExpr *expr; } printf_s;
        struct { const char **names; size_t count; } exp_s;
        struct {
            const char *name;
            ZrParam    *params;
            size_t      param_count;
            bool        is_async;
            ZrStmt    **body;
            size_t      body_count;
        } fn_s;
    } u;
};

/* ---- Recursive-Descent Parser ---- */

#define ZR_MAX_PARSE_DEPTH 512

typedef struct {
    ZrToken *toks;
    size_t   len;
    size_t   pos;
    int      depth;
    bool     in_goto_label;
    bool     no_struct_brace;
    uint32_t gensym;
    ZrArena *arena;
    char     err[512];
} ZrParser;

static inline ZrToken *zp_peek(ZrParser *p) {
    return &p->toks[p->pos < p->len ? p->pos : p->len - 1];
}

static inline ZrToken *zp_peek_n(ZrParser *p, size_t off) {
    size_t idx = p->pos + off;
    return &p->toks[idx < p->len ? idx : p->len - 1];
}

static inline ZrToken *zp_advance(ZrParser *p) {
    ZrToken *t = zp_peek(p);
    if (p->pos + 1 < p->len) p->pos++;
    return t;
}

static inline bool zp_check(ZrParser *p, ZrTokKind k) { return zp_peek(p)->kind == k; }

static inline bool zp_check_ident_str(ZrParser *p, const char *s) {
    ZrToken *t = zp_peek(p);
    return t->kind == TOK_IDENT && strcmp(t->lexeme, s) == 0;
}

static inline bool zp_match(ZrParser *p, ZrTokKind k) {
    if (zp_check(p, k)) { zp_advance(p); return true; }
    return false;
}

static bool zp_err(ZrParser *p, const char *fmt, ...) {
    if (p->err[0] != '\0') return false;
    uint32_t ln = zp_peek(p)->line;
    char msg[420];
    va_list ap;
    va_start(ap, fmt);
    vsnprintf(msg, sizeof(msg), fmt, ap);
    va_end(ap);
    snprintf(p->err, sizeof(p->err), "parse error at line %u: %s", ln, msg);
    return false;
}

static bool zp_expect(ZrParser *p, ZrTokKind k, const char *what) {
    if (zp_match(p, k)) return true;
    return zp_err(p, "expected %s, got '%s'", what, zp_peek(p)->lexeme);
}

static const char *zp_gensym(ZrParser *p, const char *prefix) {
    char buf[64];
    snprintf(buf, sizeof(buf), "__zr_%s_%u", prefix, ++p->gensym);
    return zr_arena_strdup(p->arena, buf);
}

/* Accept identifiers AND keywords when used as attribute names, map keys, or Contextual identifiers */
static const char *zp_expect_any_name(ZrParser *p, const char *what) {
    ZrToken *t = zp_peek(p);
    if (t->kind == TOK_IDENT || (t->kind >= TOK_LET && t->kind <= TOK_NIL)) {
        zp_advance(p);
        return t->lexeme;
    }
    zp_err(p, "expected %s, got '%s'", what, t->lexeme);
    return NULL;
}

static const char *zp_expect_decl_name(ZrParser *p, const char *what) {
    ZrToken *t = zp_peek(p);
    if (t->kind == TOK_IDENT || t->kind == TOK_IN || t->kind == TOK_LOOP ||
        t->kind == TOK_ASYNC || t->kind == TOK_AWAIT || t->kind == TOK_STRUCT ||
        t->kind == TOK_CLASS || t->kind == TOK_INTERFACE || t->kind == TOK_ENUM) {
        zp_advance(p);
        return t->lexeme;
    }
    zp_err(p, "expected %s, got '%s'", what, t->lexeme);
    return NULL;
}

static ZrExpr *zp_alloc_expr(ZrParser *p, ZrExprKind k, uint32_t line) {
    ZrExpr *e = (ZrExpr *)zr_arena_alloc(p->arena, sizeof(ZrExpr));
    e->kind = k;
    e->line = line;
    return e;
}

static ZrStmt *zp_alloc_stmt(ZrParser *p, ZrStmtKind k, uint32_t line) {
    ZrStmt *s = (ZrStmt *)zr_arena_alloc(p->arena, sizeof(ZrStmt));
    s->kind = k;
    s->line = line;
    return s;
}

static ZrExpr *zp_mk_var(ZrParser *p, const char *name, uint32_t line) {
    ZrExpr *e = zp_alloc_expr(p, EX_VAR, line);
    e->u.var = name;
    return e;
}

static ZrExpr *zp_mk_str(ZrParser *p, const char *s, size_t n, uint32_t line) {
    ZrExpr *e = zp_alloc_expr(p, EX_STR, line);
    e->u.str.s = zr_arena_strndup(p->arena, s, n);
    e->u.str.len = n;
    return e;
}

static ZrExpr *zp_mk_num(ZrParser *p, double v, uint32_t line) {
    ZrExpr *e = zp_alloc_expr(p, EX_NUM, line);
    e->u.num = v;
    return e;
}

static ZrExpr *zp_mk_call1(ZrParser *p, const char *fn_name, ZrExpr *arg0, uint32_t line) {
    ZrExpr *c = zp_alloc_expr(p, EX_CALL, line);
    c->u.call.callee = zp_mk_var(p, fn_name, line);
    c->u.call.args = (ZrExpr **)zr_arena_alloc(p->arena, sizeof(ZrExpr *));
    c->u.call.args[0] = arg0;
    c->u.call.argc = 1;
    return c;
}

static ZrExpr *zp_mk_call2(ZrParser *p, const char *fn_name, ZrExpr *a0, ZrExpr *a1, uint32_t line) {
    ZrExpr *c = zp_alloc_expr(p, EX_CALL, line);
    c->u.call.callee = zp_mk_var(p, fn_name, line);
    c->u.call.args = (ZrExpr **)zr_arena_alloc(p->arena, 2 * sizeof(ZrExpr *));
    c->u.call.args[0] = a0;
    c->u.call.args[1] = a1;
    c->u.call.argc = 2;
    return c;
}

static ZrExpr *zp_mk_call3(ZrParser *p, const char *fn_name, ZrExpr *a0, ZrExpr *a1, ZrExpr *a2, uint32_t line) {
    ZrExpr *c = zp_alloc_expr(p, EX_CALL, line);
    c->u.call.callee = zp_mk_var(p, fn_name, line);
    c->u.call.args = (ZrExpr **)zr_arena_alloc(p->arena, 3 * sizeof(ZrExpr *));
    c->u.call.args[0] = a0;
    c->u.call.args[1] = a1;
    c->u.call.args[2] = a2;
    c->u.call.argc = 3;
    return c;
}

static ZrExpr *zp_mk_bin(ZrParser *p, ZrTokKind op, ZrExpr *lhs, ZrExpr *rhs, uint32_t line) {
    /* Constant folding for pure numeric/string literals */
    if (lhs->kind == EX_NUM && rhs->kind == EX_NUM) {
        if (op == TOK_PLUS)  return zp_mk_num(p, lhs->u.num + rhs->u.num, line);
        if (op == TOK_MINUS) return zp_mk_num(p, lhs->u.num - rhs->u.num, line);
        if (op == TOK_STAR)  return zp_mk_num(p, lhs->u.num * rhs->u.num, line);
        if (op == TOK_SLASH && rhs->u.num != 0.0) return zp_mk_num(p, lhs->u.num / rhs->u.num, line);
    }
    if (op == TOK_PLUS && lhs->kind == EX_STR && rhs->kind == EX_STR) {
        size_t n = lhs->u.str.len + rhs->u.str.len;
        char *cat = (char *)zr_arena_alloc(p->arena, n + 1);
        memcpy(cat, lhs->u.str.s, lhs->u.str.len);
        memcpy(cat + lhs->u.str.len, rhs->u.str.s, rhs->u.str.len);
        cat[n] = '\0';
        ZrExpr *e = zp_alloc_expr(p, EX_STR, line);
        e->u.str.s = cat;
        e->u.str.len = n;
        return e;
    }
    ZrExpr *e = zp_alloc_expr(p, EX_BINARY, line);
    e->u.bin.op = op;
    e->u.bin.lhs = lhs;
    e->u.bin.rhs = rhs;
    return e;
}

/* Forward Parser Declarations */
static ZrExpr *zp_parse_expr(ZrParser *p);
static ZrStmt *zp_parse_stmt(ZrParser *p);
static bool    zp_parse_block_stmts(ZrParser *p, ZrStmt ***out_stmts, size_t *out_count);

/* Skip optional type expression (e.g., `number`, `array<string>`, `fn(number)->bool`, `?T`, `A | B`) */
static bool zp_skip_type_expr(ZrParser *p) {
    for (;;) {
        zp_match(p, TOK_QMARK);
        if (zp_match(p, TOK_FN)) {
            if (!zp_expect(p, TOK_LPAREN, "'(' in fn type")) return false;
            int d = 1;
            while (d > 0 && !zp_check(p, TOK_EOF)) {
                if (zp_match(p, TOK_LPAREN)) d++;
                else if (zp_match(p, TOK_RPAREN)) d--;
                else zp_advance(p);
            }
            if (zp_match(p, TOK_ARROW)) {
                if (!zp_skip_type_expr(p)) return false;
            }
        } else if (zp_match(p, TOK_LBRACK)) {
            int d = 1;
            while (d > 0 && !zp_check(p, TOK_EOF)) {
                if (zp_match(p, TOK_LBRACK)) d++;
                else if (zp_match(p, TOK_RBRACK)) d--;
                else zp_advance(p);
            }
        } else if (zp_match(p, TOK_LBRACE)) {
            int d = 1;
            while (d > 0 && !zp_check(p, TOK_EOF)) {
                if (zp_match(p, TOK_LBRACE)) d++;
                else if (zp_match(p, TOK_RBRACE)) d--;
                else zp_advance(p);
            }
        } else {
            if (!zp_expect_any_name(p, "type name")) return false;
            while (zp_match(p, TOK_DOT)) {
                if (!zp_expect_any_name(p, "qualified type name")) return false;
            }
            if (zp_match(p, TOK_LT)) {
                int d = 1;
                while (d > 0 && !zp_check(p, TOK_EOF)) {
                    if (zp_match(p, TOK_LT)) d++;
                    else if (zp_match(p, TOK_GT)) d--;
                    else if (zp_match(p, TOK_SHR)) { d -= 2; if (d < 0) d = 0; }
                    else zp_advance(p);
                }
            }
            while (zp_check(p, TOK_LBRACK) && zp_peek_n(p, 1)->kind == TOK_RBRACK) {
                zp_advance(p);
                zp_advance(p);
            }
        }
        if (!zp_match(p, TOK_PIPE)) break;
    }
    return true;
}

static bool zp_parse_param_list(ZrParser *p, ZrParam **out_params, size_t *out_count) {
    if (!zp_expect(p, TOK_LPAREN, "'('")) return false;
    ZrParam *arr = NULL;
    size_t n = 0, cap = 0;
    while (!zp_check(p, TOK_RPAREN) && !zp_check(p, TOK_EOF)) {
        bool is_va = zp_match(p, TOK_ELLIPSIS);
        const char *pname = zp_expect_decl_name(p, "parameter name");
        if (!pname) return false;
        if (zp_match(p, TOK_COLON)) {
            if (!zp_skip_type_expr(p)) return false;
        }
        ZrExpr *def_e = NULL;
        if (!is_va && zp_match(p, TOK_EQ)) {
            def_e = zp_parse_expr(p);
            if (!def_e) return false;
        }
        if (n >= cap) {
            cap = cap ? cap * 2 : 4;
            ZrParam *narr = (ZrParam *)zr_arena_alloc(p->arena, cap * sizeof(ZrParam));
            if (n > 0) memcpy(narr, arr, n * sizeof(ZrParam));
            arr = narr;
        }
        arr[n].name = pname;
        arr[n].def_expr = def_e;
        arr[n].is_vararg = is_va;
        n++;
        if (!zp_match(p, TOK_COMMA)) break;
    }
    if (!zp_expect(p, TOK_RPAREN, "')'")) return false;
    if (zp_match(p, TOK_ARROW)) {
        if (!zp_skip_type_expr(p)) return false;
    }
    *out_params = arr;
    *out_count = n;
    return true;
}

/* Parse f-string inner text into "" + ... + str(expr) + ... */
static ZrExpr *zp_parse_fstring(ZrParser *p, const char *raw, size_t rlen, uint32_t line) {
    ZrExpr *acc = NULL;
    size_t i = 0;
    ZrBuf lit;
    zr_buf_init(&lit);

    while (i < rlen) {
        if (raw[i] == '{') {
            if (i + 1 < rlen && raw[i + 1] == '{') {
                zr_buf_push(&lit, '{');
                i += 2;
                continue;
            }
            if (lit.len > 0) {
                const char *unesc = NULL; size_t ulen = 0;
                zr_unescape_string(p->arena, lit.data, lit.len, &unesc, &ulen);
                ZrExpr *le = zp_mk_str(p, unesc, ulen, line);
                acc = acc ? zp_mk_bin(p, TOK_PLUS, acc, le, line) : le;
                lit.len = 0;
            }
            i++;
            size_t expr_start = i;
            int bdepth = 1;
            while (i < rlen && bdepth > 0) {
                char ic = raw[i];
                if (ic == '"' || ic == '\'') {
                    char iq = ic;
                    i++;
                    while (i < rlen && raw[i] != iq) {
                        if (raw[i] == '\\' && i + 1 < rlen) { i += 2; continue; }
                        i++;
                    }
                    if (i < rlen) i++;
                    continue;
                }
                if (ic == '{') bdepth++;
                else if (ic == '}') bdepth--;
                if (bdepth > 0) i++;
            }
            size_t elen = i - expr_start;
            if (i < rlen && raw[i] == '}') i++;

            /* Sub-lex and sub-parse interpolated expression */
            ZrLexer sub_lx;
            memset(&sub_lx, 0, sizeof(sub_lx));
            sub_lx.src = raw + expr_start;
            sub_lx.len = elen;
            sub_lx.line = line;
            sub_lx.arena = p->arena;
            if (!zr_lex_all(&sub_lx)) {
                zr_buf_free(&lit);
                free(sub_lx.tokens);
                snprintf(p->err, sizeof(p->err), "%s", sub_lx.err);
                return NULL;
            }
            ZrParser sub_p;
            memset(&sub_p, 0, sizeof(sub_p));
            sub_p.toks = sub_lx.tokens;
            sub_p.len = sub_lx.tok_len;
            sub_p.arena = p->arena;
            sub_p.gensym = p->gensym;
            ZrExpr *inner_e = zp_parse_expr(&sub_p);
            p->gensym = sub_p.gensym;
            free(sub_lx.tokens);
            if (!inner_e) {
                zr_buf_free(&lit);
                snprintf(p->err, sizeof(p->err), "%s", sub_p.err);
                return NULL;
            }
            ZrExpr *str_e = zp_mk_call1(p, "str", inner_e, line);
            acc = acc ? zp_mk_bin(p, TOK_PLUS, acc, str_e, line) : str_e;
            continue;
        }
        if (raw[i] == '}' && i + 1 < rlen && raw[i + 1] == '}') {
            zr_buf_push(&lit, '}');
            i += 2;
            continue;
        }
        if (raw[i] == '\\' && i + 1 < rlen) {
            zr_buf_push(&lit, raw[i++]);
            zr_buf_push(&lit, raw[i++]);
            continue;
        }
        zr_buf_push(&lit, raw[i++]);
    }

    if (lit.len > 0 || acc == NULL) {
        const char *unesc = NULL; size_t ulen = 0;
        zr_unescape_string(p->arena, lit.data ? lit.data : "", lit.len, &unesc, &ulen);
        ZrExpr *le = zp_mk_str(p, unesc, ulen, line);
        acc = acc ? zp_mk_bin(p, TOK_PLUS, acc, le, line) : le;
    }
    zr_buf_free(&lit);
    return acc;
}

static ZrExpr *zp_parse_map_body(ZrParser *p, uint32_t line) {
    ZrExpr **keys = NULL, **vals = NULL;
    size_t n = 0, cap = 0;
    while (!zp_check(p, TOK_RBRACE) && !zp_check(p, TOK_EOF)) {
        ZrExpr *ke = NULL;
        ZrExpr *ve = NULL;
        if (zp_match(p, TOK_LBRACK)) {
            ke = zp_parse_expr(p);
            if (!ke || !zp_expect(p, TOK_RBRACK, "']'")) return NULL;
            if (!zp_expect(p, TOK_COLON, "':'")) return NULL;
            ve = zp_parse_expr(p);
            if (!ve) return NULL;
        } else if (zp_check(p, TOK_STR)) {
            ZrToken *st = zp_advance(p);
            ke = zp_mk_str(p, st->lexeme, st->lex_len, st->line);
            if (!zp_expect(p, TOK_COLON, "':'")) return NULL;
            ve = zp_parse_expr(p);
            if (!ve) return NULL;
        } else if (zp_check(p, TOK_NUM)) {
            ZrToken *nt = zp_advance(p);
            ke = zp_mk_str(p, nt->lexeme, nt->lex_len, nt->line);
            if (!zp_expect(p, TOK_COLON, "':'")) return NULL;
            ve = zp_parse_expr(p);
            if (!ve) return NULL;
        } else {
            const char *kname = zp_expect_any_name(p, "map key");
            if (!kname) return NULL;
            ke = zp_mk_str(p, kname, strlen(kname), zp_peek(p)->line);
            if (zp_match(p, TOK_COLON)) {
                ve = zp_parse_expr(p);
                if (!ve) return NULL;
            } else {
                /* Shorthand map{ x, y } -> x: x */
                ve = zp_mk_var(p, kname, zp_peek(p)->line);
            }
        }
        if (n >= cap) {
            cap = cap ? cap * 2 : 4;
            ZrExpr **nk = (ZrExpr **)zr_arena_alloc(p->arena, cap * sizeof(ZrExpr *));
            ZrExpr **nv = (ZrExpr **)zr_arena_alloc(p->arena, cap * sizeof(ZrExpr *));
            if (n > 0) {
                memcpy(nk, keys, n * sizeof(ZrExpr *));
                memcpy(nv, vals, n * sizeof(ZrExpr *));
            }
            keys = nk;
            vals = nv;
        }
        keys[n] = ke;
        vals[n] = ve;
        n++;
        if (!zp_match(p, TOK_COMMA)) break;
    }
    if (!zp_expect(p, TOK_RBRACE, "'}'")) return NULL;
    ZrExpr *m = zp_alloc_expr(p, EX_MAP, line);
    m->u.map.keys = keys;
    m->u.map.vals = vals;
    m->u.map.count = n;
    return m;
}

static ZrExpr *zp_parse_if_expr(ZrParser *p, uint32_t line);

/* Parse `{ stmts... [tail_expr] }` as expression */
static ZrExpr *zp_parse_block_expr(ZrParser *p, uint32_t line) {
    ZrStmt **stmts = NULL;
    size_t n = 0, cap = 0;
    ZrExpr *tail = NULL;

    while (!zp_check(p, TOK_RBRACE) && !zp_check(p, TOK_EOF)) {
        if (zp_match(p, TOK_SEMI)) continue;
        if (zp_check(p, TOK_IF)) {
            size_t mark = p->pos;
            uint32_t if_ln = zp_advance(p)->line;
            ZrExpr *ie = zp_parse_if_expr(p, if_ln);
            if (!ie) return NULL;
            ZrTokKind nk = zp_peek(p)->kind;
            if (ie->u.if_e.else_e != NULL &&
                (nk == TOK_PLUS || nk == TOK_STAR || nk == TOK_SLASH || nk == TOK_PERCENT ||
                 nk == TOK_STARSTAR || nk == TOK_EQEQ || nk == TOK_BANGEQ ||
                 nk == TOK_LT || nk == TOK_LTEQ || nk == TOK_GT || nk == TOK_GTEQ ||
                 nk == TOK_AND || nk == TOK_OR || nk == TOK_QQ || nk == TOK_QMARK || nk == TOK_PIPEGT)) {
                p->pos = mark;
                ie = zp_parse_expr(p);
                if (!ie) return NULL;
            }
            bool had_semi = zp_match(p, TOK_SEMI);
            if (!had_semi && zp_check(p, TOK_RBRACE)) {
                tail = ie;
                break;
            }
            ZrStmt *st_if = zp_alloc_stmt(p, ST_EXPR, if_ln);
            st_if->u.expr = ie;
            if (n >= cap) {
                cap = cap ? cap * 2 : 4;
                ZrStmt **ns = (ZrStmt **)zr_arena_alloc(p->arena, cap * sizeof(ZrStmt *));
                if (n > 0) memcpy(ns, stmts, n * sizeof(ZrStmt *));
                stmts = ns;
            }
            stmts[n++] = st_if;
            continue;
        }
        ZrStmt *st = zp_parse_stmt(p);
        if (!st) return NULL;
        bool had_semi = zp_match(p, TOK_SEMI);
        if (!had_semi && zp_check(p, TOK_RBRACE) && st->kind == ST_EXPR) {
            tail = st->u.expr;
            break;
        }
        if (n >= cap) {
            cap = cap ? cap * 2 : 4;
            ZrStmt **ns = (ZrStmt **)zr_arena_alloc(p->arena, cap * sizeof(ZrStmt *));
            if (n > 0) memcpy(ns, stmts, n * sizeof(ZrStmt *));
            stmts = ns;
        }
        stmts[n++] = st;
    }
    if (!zp_expect(p, TOK_RBRACE, "'}'")) return NULL;
    ZrExpr *b = zp_alloc_expr(p, EX_BLOCK, line);
    b->u.blk.stmts = stmts;
    b->u.blk.stmt_count = n;
    b->u.blk.tail = tail;
    return b;
}

static ZrPat *zp_parse_single_pat(ZrParser *p) {
    ZrPat *pat = (ZrPat *)zr_arena_alloc(p->arena, sizeof(ZrPat));
    if (zp_check_ident_str(p, "_")) {
        zp_advance(p);
        pat->kind = PAT_WILD;
        return pat;
    }
    if (zp_check(p, TOK_NIL) || zp_check(p, TOK_TRUE) || zp_check(p, TOK_FALSE) ||
        zp_check(p, TOK_NUM) || zp_check(p, TOK_STR) || zp_check(p, TOK_MINUS)) {
        uint32_t ln = zp_peek(p)->line;
        if (zp_match(p, TOK_NIL)) {
            pat->kind = PAT_LIT;
            pat->lit_expr = zp_alloc_expr(p, EX_NIL, ln);
            return pat;
        }
        if (zp_match(p, TOK_TRUE)) {
            pat->kind = PAT_LIT;
            pat->lit_expr = zp_alloc_expr(p, EX_BOOL, ln);
            pat->lit_expr->u.b = true;
            return pat;
        }
        if (zp_match(p, TOK_FALSE)) {
            pat->kind = PAT_LIT;
            pat->lit_expr = zp_alloc_expr(p, EX_BOOL, ln);
            pat->lit_expr->u.b = false;
            return pat;
        }
        if (zp_match(p, TOK_MINUS)) {
            ZrToken *nt = zp_peek(p);
            if (nt->kind != TOK_NUM) { zp_err(p, "expected number after '-' in pattern"); return NULL; }
            zp_advance(p);
            pat->kind = PAT_LIT;
            pat->lit_expr = zp_mk_num(p, -nt->num_val, ln);
            return pat;
        }
        if (zp_check(p, TOK_NUM)) {
            ZrToken *nt = zp_advance(p);
            pat->kind = PAT_LIT;
            pat->lit_expr = zp_mk_num(p, nt->num_val, ln);
            return pat;
        }
        if (zp_check(p, TOK_STR)) {
            ZrToken *st = zp_advance(p);
            pat->kind = PAT_LIT;
            pat->lit_expr = zp_mk_str(p, st->lexeme, st->lex_len, ln);
            return pat;
        }
    }
    if (zp_match(p, TOK_LBRACK)) {
        pat->kind = PAT_ARRAY;
        ZrPat **subs = NULL;
        size_t n = 0, cap = 0;
        while (!zp_check(p, TOK_RBRACK) && !zp_check(p, TOK_EOF)) {
            if (zp_match(p, TOK_ELLIPSIS)) {
                pat->rest_name = zp_check(p, TOK_RBRACK) ? "_" : zp_expect_decl_name(p, "rest binding");
                if (!pat->rest_name) return NULL;
                zp_match(p, TOK_COMMA);
                break;
            }
            ZrPat *sp = zp_parse_single_pat(p);
            if (!sp) return NULL;
            if (n >= cap) {
                cap = cap ? cap * 2 : 4;
                ZrPat **ns = (ZrPat **)zr_arena_alloc(p->arena, cap * sizeof(ZrPat *));
                if (n > 0) memcpy(ns, subs, n * sizeof(ZrPat *));
                subs = ns;
            }
            subs[n++] = sp;
            if (!zp_match(p, TOK_COMMA)) break;
        }
        if (!zp_expect(p, TOK_RBRACK, "']'")) return NULL;
        pat->sub_pats = subs;
        pat->sub_count = n;
        return pat;
    }
    if ((zp_check_ident_str(p, "map") && zp_peek_n(p, 1)->kind == TOK_LBRACE) || zp_check(p, TOK_LBRACE)) {
        if (zp_check_ident_str(p, "map")) zp_advance(p);
        zp_advance(p);
        pat->kind = PAT_MAP;
        ZrPat **subs = NULL;
        const char **mkeys = NULL;
        size_t n = 0, cap = 0;
        while (!zp_check(p, TOK_RBRACE) && !zp_check(p, TOK_EOF)) {
            const char *k = NULL;
            if (zp_check(p, TOK_STR)) k = zp_advance(p)->lexeme;
            else k = zp_expect_any_name(p, "map pattern key");
            if (!k) return NULL;
            ZrPat *vp = NULL;
            if (zp_match(p, TOK_COLON)) {
                vp = zp_parse_single_pat(p);
                if (!vp) return NULL;
            } else {
                vp = (ZrPat *)zr_arena_alloc(p->arena, sizeof(ZrPat));
                vp->kind = PAT_VAR;
                vp->var_name = k;
            }
            if (n >= cap) {
                cap = cap ? cap * 2 : 4;
                ZrPat **ns = (ZrPat **)zr_arena_alloc(p->arena, cap * sizeof(ZrPat *));
                const char **nk = (const char **)zr_arena_alloc(p->arena, cap * sizeof(const char *));
                if (n > 0) { memcpy(ns, subs, n * sizeof(ZrPat *)); memcpy(nk, mkeys, n * sizeof(const char *)); }
                subs = ns; mkeys = nk;
            }
            mkeys[n] = k;
            subs[n] = vp;
            n++;
            if (!zp_match(p, TOK_COMMA)) break;
        }
        if (!zp_expect(p, TOK_RBRACE, "'}'")) return NULL;
        pat->map_keys = mkeys;
        pat->sub_pats = subs;
        pat->sub_count = n;
        return pat;
    }
    const char *vname = zp_expect_decl_name(p, "pattern");
    if (!vname) return NULL;
    pat->kind = PAT_VAR;
    pat->var_name = vname;
    return pat;
}

static ZrPat *zp_parse_pat(ZrParser *p) {
    ZrPat *first = zp_parse_single_pat(p);
    if (!first) return NULL;
    if (!zp_check(p, TOK_PIPE)) return first;
    ZrPat **alts = (ZrPat **)zr_arena_alloc(p->arena, 8 * sizeof(ZrPat *));
    size_t n = 0, cap = 8;
    alts[n++] = first;
    while (zp_match(p, TOK_PIPE)) {
        ZrPat *nx = zp_parse_single_pat(p);
        if (!nx) return NULL;
        if (n >= cap) {
            cap *= 2;
            ZrPat **na = (ZrPat **)zr_arena_alloc(p->arena, cap * sizeof(ZrPat *));
            memcpy(na, alts, n * sizeof(ZrPat *));
            alts = na;
        }
        alts[n++] = nx;
    }
    ZrPat *orp = (ZrPat *)zr_arena_alloc(p->arena, sizeof(ZrPat));
    orp->kind = PAT_OR;
    orp->sub_pats = alts;
    orp->sub_count = n;
    return orp;
}

static ZrExpr *zp_parse_if_expr(ZrParser *p, uint32_t line) {
    bool saved_nb = p->no_struct_brace;
    p->no_struct_brace = true;
    ZrExpr *cond = zp_parse_expr(p);
    p->no_struct_brace = saved_nb;
    if (!cond) return NULL;
    if (!zp_expect(p, TOK_LBRACE, "'{' after if condition")) return NULL;
    ZrExpr *then_e = zp_parse_block_expr(p, zp_peek(p)->line);
    if (!then_e) return NULL;

    ZrExpr *else_e = NULL;
    if (zp_match(p, TOK_ELIF)) {
        else_e = zp_parse_if_expr(p, zp_peek(p)->line);
        if (!else_e) return NULL;
    } else if (zp_match(p, TOK_ELSE)) {
        if (zp_match(p, TOK_IF)) {
            else_e = zp_parse_if_expr(p, zp_peek(p)->line);
            if (!else_e) return NULL;
        } else {
            if (!zp_expect(p, TOK_LBRACE, "'{' after else")) return NULL;
            else_e = zp_parse_block_expr(p, zp_peek(p)->line);
            if (!else_e) return NULL;
        }
    }
    ZrExpr *ie = zp_alloc_expr(p, EX_IF, line);
    ie->u.if_e.cond = cond;
    ie->u.if_e.then_e = then_e;
    ie->u.if_e.else_e = else_e;
    return ie;
}

static ZrExpr *zp_parse_match_expr(ZrParser *p, uint32_t line) {
    bool saved_nb = p->no_struct_brace;
    p->no_struct_brace = true;
    ZrExpr *scrut = zp_parse_expr(p);
    p->no_struct_brace = saved_nb;
    if (!scrut) return NULL;
    if (!zp_expect(p, TOK_LBRACE, "'{' after match scrutinee")) return NULL;
    ZrMatchArm *arms = NULL;
    size_t n = 0, cap = 0;
    while (!zp_check(p, TOK_RBRACE) && !zp_check(p, TOK_EOF)) {
        ZrPat *pat = NULL;
        ZrExpr *guard = NULL;
        if (zp_match(p, TOK_ELSE)) {
            pat = (ZrPat *)zr_arena_alloc(p->arena, sizeof(ZrPat));
            pat->kind = PAT_WILD;
        } else {
            if (zp_check_ident_str(p, "case")) zp_advance(p);
            pat = zp_parse_pat(p);
            if (!pat) return NULL;
            if (zp_match(p, TOK_IF)) {
                bool snb = p->no_struct_brace;
                p->no_struct_brace = true;
                guard = zp_parse_expr(p);
                p->no_struct_brace = snb;
                if (!guard) return NULL;
            }
        }
        /* Separator: =>, ->, :, or direct { */
        if (!zp_match(p, TOK_FATARROW) && !zp_match(p, TOK_ARROW) && !zp_match(p, TOK_COLON)) {
            if (!zp_check(p, TOK_LBRACE)) {
                zp_err(p, "expected '=>', '->', ':', or '{' in match arm");
                return NULL;
            }
        }
        ZrExpr *body = NULL;
        if (zp_match(p, TOK_LBRACE)) {
            body = zp_parse_block_expr(p, zp_peek(p)->line);
        } else {
            body = zp_parse_expr(p);
        }
        if (!body) return NULL;
        zp_match(p, TOK_COMMA);
        zp_match(p, TOK_SEMI);

        if (n >= cap) {
            cap = cap ? cap * 2 : 4;
            ZrMatchArm *na = (ZrMatchArm *)zr_arena_alloc(p->arena, cap * sizeof(ZrMatchArm));
            if (n > 0) memcpy(na, arms, n * sizeof(ZrMatchArm));
            arms = na;
        }
        arms[n].pat = pat;
        arms[n].guard = guard;
        arms[n].body = body;
        n++;
    }
    if (!zp_expect(p, TOK_RBRACE, "'}'")) return NULL;
    ZrExpr *me = zp_alloc_expr(p, EX_MATCH, line);
    me->u.match_e.scrut = scrut;
    me->u.match_e.arms = arms;
    me->u.match_e.arm_count = n;
    return me;
}

static ZrExpr *zp_parse_primary(ZrParser *p) {
    if (++p->depth > ZR_MAX_PARSE_DEPTH) {
        p->depth--;
        zp_err(p, "maximum nesting depth (%d) exceeded", ZR_MAX_PARSE_DEPTH);
        return NULL;
    }
    ZrExpr *res = NULL;
    ZrToken *t = zp_peek(p);
    uint32_t ln = t->line;

    switch (t->kind) {
        case TOK_NIL:
            zp_advance(p);
            res = zp_alloc_expr(p, EX_NIL, ln);
            break;
        case TOK_TRUE:
            zp_advance(p);
            res = zp_alloc_expr(p, EX_BOOL, ln);
            res->u.b = true;
            break;
        case TOK_FALSE:
            zp_advance(p);
            res = zp_alloc_expr(p, EX_BOOL, ln);
            res->u.b = false;
            break;
        case TOK_NUM:
            zp_advance(p);
            res = zp_mk_num(p, t->num_val, ln);
            break;
        case TOK_STR:
            zp_advance(p);
            res = zp_mk_str(p, t->lexeme, t->lex_len, ln);
            break;
        case TOK_FSTR:
            zp_advance(p);
            res = zp_parse_fstring(p, t->lexeme, t->lex_len, ln);
            break;
        case TOK_LPAREN: {
            zp_advance(p);
            bool saved_nb = p->no_struct_brace;
            p->no_struct_brace = false;
            res = zp_parse_expr(p);
            p->no_struct_brace = saved_nb;
            if (res && !zp_expect(p, TOK_RPAREN, "')'")) res = NULL;
            break;
        }
        case TOK_LBRACK: {
            zp_advance(p);
            bool saved_nb = p->no_struct_brace;
            p->no_struct_brace = false;
            ZrExpr **items = NULL;
            size_t n = 0, cap = 0;
            while (!zp_check(p, TOK_RBRACK) && !zp_check(p, TOK_EOF)) {
                ZrExpr *it = zp_parse_expr(p);
                if (!it) { p->no_struct_brace = saved_nb; p->depth--; return NULL; }
                if (n >= cap) {
                    cap = cap ? cap * 2 : 4;
                    ZrExpr **ni = (ZrExpr **)zr_arena_alloc(p->arena, cap * sizeof(ZrExpr *));
                    if (n > 0) memcpy(ni, items, n * sizeof(ZrExpr *));
                    items = ni;
                }
                items[n++] = it;
                if (!zp_match(p, TOK_COMMA)) break;
            }
            p->no_struct_brace = saved_nb;
            if (!zp_expect(p, TOK_RBRACK, "']'")) { p->depth--; return NULL; }
            res = zp_alloc_expr(p, EX_ARRAY, ln);
            res->u.arr.items = items;
            res->u.arr.count = n;
            break;
        }
        case TOK_LBRACE: {
            zp_advance(p);
            bool saved_nb = p->no_struct_brace;
            p->no_struct_brace = false;
            /* Distinguish map literal `{}` / `{ key: val }` from block expression `{ stmts }` */
            ZrToken *t0 = zp_peek(p);
            ZrToken *t1 = zp_peek_n(p, 1);
            bool is_map_lit = (t0->kind == TOK_RBRACE) ||
                              ((t0->kind == TOK_IDENT || t0->kind == TOK_STR || t0->kind == TOK_NUM ||
                                (t0->kind >= TOK_LET && t0->kind <= TOK_NIL)) &&
                               t1->kind == TOK_COLON);
            if (is_map_lit) res = zp_parse_map_body(p, ln);
            else res = zp_parse_block_expr(p, ln);
            p->no_struct_brace = saved_nb;
            break;
        }
        case TOK_ASYNC:
        case TOK_FN: {
            bool is_async = zp_match(p, TOK_ASYNC);
            if (!zp_expect(p, TOK_FN, "'fn'")) { p->depth--; return NULL; }
            const char *fname = NULL;
            if (!zp_check(p, TOK_LPAREN)) {
                fname = zp_expect_any_name(p, "function name");
                if (!fname) { p->depth--; return NULL; }
            }
            ZrParam *params = NULL; size_t pcount = 0;
            if (!zp_parse_param_list(p, &params, &pcount)) { p->depth--; return NULL; }
            ZrStmt **body = NULL; size_t bcount = 0;
            bool saved_nb = p->no_struct_brace;
            p->no_struct_brace = false;
            bool bok = zp_parse_block_stmts(p, &body, &bcount);
            p->no_struct_brace = saved_nb;
            if (!bok) { p->depth--; return NULL; }
            res = zp_alloc_expr(p, EX_FN, ln);
            res->u.fn.name = fname;
            res->u.fn.params = params;
            res->u.fn.param_count = pcount;
            res->u.fn.is_async = is_async;
            res->u.fn.body = body;
            res->u.fn.body_count = bcount;
            break;
        }
        case TOK_IF:
            zp_advance(p);
            res = zp_parse_if_expr(p, ln);
            break;
        case TOK_TRY: {
            zp_advance(p);
            bool try_was_block = false;
            ZrExpr *try_e = NULL;
            if (zp_match(p, TOK_LBRACE)) {
                try_was_block = true;
                bool snb = p->no_struct_brace;
                p->no_struct_brace = false;
                try_e = zp_parse_block_expr(p, zp_peek(p)->line);
                p->no_struct_brace = snb;
            } else {
                try_e = zp_parse_expr(p);
            }
            if (!try_e) { p->depth--; return NULL; }
            const char *err_var = NULL;
            ZrExpr *catch_e = NULL;
            if (zp_match(p, TOK_CATCH)) {
                if (zp_check(p, TOK_LPAREN) && zp_peek_n(p, 1)->kind == TOK_IDENT &&
                    zp_peek_n(p, 2)->kind == TOK_RPAREN && zp_peek_n(p, 3)->kind == TOK_LBRACE) {
                    zp_advance(p);
                    err_var = zp_advance(p)->lexeme;
                    zp_advance(p);
                    zp_advance(p);
                    bool snb = p->no_struct_brace;
                    p->no_struct_brace = false;
                    catch_e = zp_parse_block_expr(p, zp_peek(p)->line);
                    p->no_struct_brace = snb;
                } else if ((try_was_block || !p->no_struct_brace) &&
                           zp_check(p, TOK_IDENT) &&
                           strcmp(zp_peek(p)->lexeme, "map") != 0 &&
                           zp_peek_n(p, 1)->kind == TOK_LBRACE) {
                    err_var = zp_advance(p)->lexeme;
                    zp_advance(p);
                    bool snb = p->no_struct_brace;
                    p->no_struct_brace = false;
                    catch_e = zp_parse_block_expr(p, zp_peek(p)->line);
                    p->no_struct_brace = snb;
                } else {
                    catch_e = zp_parse_expr(p);
                }
                if (!catch_e) { p->depth--; return NULL; }
            } else {
                catch_e = zp_alloc_expr(p, EX_NIL, ln);
            }
            res = zp_alloc_expr(p, EX_TRY, ln);
            res->u.try_e.try_e = try_e;
            res->u.try_e.err_var = err_var;
            res->u.try_e.catch_e = catch_e;
            break;
        }
        default: {
            /* Contextual `map{ ... }` */
            if (t->kind == TOK_IDENT && strcmp(t->lexeme, "map") == 0 && zp_peek_n(p, 1)->kind == TOK_LBRACE) {
                zp_advance(p);
                zp_advance(p);
                res = zp_parse_map_body(p, ln);
                break;
            }
            /* Contextual `match <scrut> { ... }` / `switch <scrut> { ... }` (preserve variables & calls named `match`/`switch`!) */
            {
                ZrTokKind nk = zp_peek_n(p, 1)->kind;
                bool can_start_scrut = (nk == TOK_IDENT || nk == TOK_NUM || nk == TOK_STR || nk == TOK_FSTR ||
                                        nk == TOK_TRUE || nk == TOK_FALSE || nk == TOK_NIL ||
                                        nk == TOK_MINUS || nk == TOK_BANG || nk == TOK_NOT);
                if (t->kind == TOK_IDENT && strcmp(t->lexeme, "match") == 0 && can_start_scrut) {
                    zp_advance(p);
                    res = zp_parse_match_expr(p, ln);
                    break;
                }
                if (t->kind == TOK_IDENT && strcmp(t->lexeme, "switch") == 0 && can_start_scrut) {
                    zp_advance(p);
                    ZrExpr *scrut = zp_parse_expr(p);
                    if (!scrut || !zp_expect(p, TOK_LBRACE, "'{' after switch")) { p->depth--; return NULL; }
                ZrMatchArm *arms = NULL;
                size_t n = 0, cap = 0;
                while (!zp_check(p, TOK_RBRACE) && !zp_check(p, TOK_EOF)) {
                    ZrPat *pat = (ZrPat *)zr_arena_alloc(p->arena, sizeof(ZrPat));
                    if (zp_check_ident_str(p, "default") || zp_check(p, TOK_ELSE)) {
                        zp_advance(p);
                        pat->kind = PAT_WILD;
                    } else {
                        if (zp_check_ident_str(p, "case")) zp_advance(p);
                        ZrExpr *ce = zp_parse_primary(p);
                        if (!ce) { p->depth--; return NULL; }
                        pat->kind = PAT_LIT;
                        pat->lit_expr = ce;
                    }
                    zp_match(p, TOK_COLON);
                    zp_match(p, TOK_FATARROW);
                    if (!zp_expect(p, TOK_LBRACE, "'{' in switch case")) { p->depth--; return NULL; }
                    ZrExpr *be = zp_parse_block_expr(p, zp_peek(p)->line);
                    if (!be) { p->depth--; return NULL; }
                    if (n >= cap) {
                        cap = cap ? cap * 2 : 4;
                        ZrMatchArm *na = (ZrMatchArm *)zr_arena_alloc(p->arena, cap * sizeof(ZrMatchArm));
                        if (n > 0) memcpy(na, arms, n * sizeof(ZrMatchArm));
                        arms = na;
                    }
                    arms[n].pat = pat;
                    arms[n].guard = NULL;
                    arms[n].body = be;
                    n++;
                }
                if (!zp_expect(p, TOK_RBRACE, "'}'")) { p->depth--; return NULL; }
                res = zp_alloc_expr(p, EX_MATCH, ln);
                res->u.match_e.scrut = scrut;
                res->u.match_e.arms = arms;
                res->u.match_e.arm_count = n;
                break;
                }
            }
            if (t->kind == TOK_IDENT || t->kind == TOK_IN || t->kind == TOK_ASYNC ||
                t->kind == TOK_ENUM || t->kind == TOK_INTERFACE || t->kind == TOK_DEFER ||
                t->kind == TOK_LOOP || t->kind == TOK_STRUCT || t->kind == TOK_CLASS ||
                t->kind == TOK_PRINT) {
                zp_advance(p);
                res = zp_mk_var(p, t->lexeme, ln);
                break;
            }
            zp_err(p, "unexpected token '%s' in expression", t->lexeme);
            break;
        }
    }

    p->depth--;
    return res;
}

static bool zp_parse_call_args(ZrParser *p, ZrExpr ***out_args, size_t *out_argc) {
    ZrExpr **args = NULL;
    size_t n = 0, cap = 0;
    while (!zp_check(p, TOK_RPAREN) && !zp_check(p, TOK_EOF)) {
        ZrExpr *a = zp_parse_expr(p);
        if (!a) return false;
        if (n >= cap) {
            cap = cap ? cap * 2 : 4;
            ZrExpr **na = (ZrExpr **)zr_arena_alloc(p->arena, cap * sizeof(ZrExpr *));
            if (n > 0) memcpy(na, args, n * sizeof(ZrExpr *));
            args = na;
        }
        args[n++] = a;
        if (!zp_match(p, TOK_COMMA)) break;
    }
    if (!zp_expect(p, TOK_RPAREN, "')'")) return false;
    *out_args = args;
    *out_argc = n;
    return true;
}

static ZrExpr *zp_parse_postfix(ZrParser *p) {
    ZrExpr *cur = zp_parse_primary(p);
    if (!cur) return NULL;

    for (;;) {
        uint32_t ln = zp_peek(p)->line;
        if (zp_match(p, TOK_LPAREN)) {
            ZrExpr **args = NULL; size_t argc = 0;
            if (!zp_parse_call_args(p, &args, &argc)) return NULL;
            ZrExpr *ce = zp_alloc_expr(p, EX_CALL, ln);
            ce->u.call.callee = cur;
            ce->u.call.args = args;
            ce->u.call.argc = argc;
            cur = ce;
        } else if (zp_match(p, TOK_DOT)) {
            const char *attr = zp_expect_any_name(p, "attribute name");
            if (!attr) return NULL;
            ZrExpr *ae = zp_alloc_expr(p, EX_ATTR, ln);
            ae->u.attr.obj = cur;
            ae->u.attr.attr = attr;
            cur = ae;
        } else if (zp_match(p, TOK_QDOT)) {
            const char *attr = zp_expect_any_name(p, "optional attribute name");
            if (!attr) return NULL;
            ZrExpr *ae = zp_alloc_expr(p, EX_SAFE_ATTR, ln);
            ae->u.attr.obj = cur;
            ae->u.attr.attr = attr;
            cur = ae;
        } else if (zp_match(p, TOK_LBRACK)) {
            /* Check slice syntax: cur[a..b], cur[..b], cur[a..], cur[..] */
            if (zp_match(p, TOK_DOTDOT)) {
                ZrExpr *start_e = zp_mk_num(p, 0.0, ln);
                ZrExpr *end_e = zp_check(p, TOK_RBRACK) ? zp_alloc_expr(p, EX_NIL, ln) : zp_parse_expr(p);
                if (!end_e || !zp_expect(p, TOK_RBRACK, "']'")) return NULL;
                cur = zp_mk_call3(p, "slice", cur, start_e, end_e, ln);
            } else {
                ZrExpr *first_e = zp_parse_expr(p);
                if (!first_e) return NULL;
                if (zp_match(p, TOK_DOTDOT)) {
                    ZrExpr *end_e = zp_check(p, TOK_RBRACK) ? zp_alloc_expr(p, EX_NIL, ln) : zp_parse_expr(p);
                    if (!end_e || !zp_expect(p, TOK_RBRACK, "']'")) return NULL;
                    cur = zp_mk_call3(p, "slice", cur, first_e, end_e, ln);
                } else {
                    if (!zp_expect(p, TOK_RBRACK, "']'")) return NULL;
                    ZrExpr *ie = zp_alloc_expr(p, EX_INDEX, ln);
                    ie->u.idx.obj = cur;
                    ie->u.idx.idx = first_e;
                    cur = ie;
                }
            }
        } else if (zp_match(p, TOK_QLBRACK)) {
            ZrExpr *idx_e = zp_parse_expr(p);
            if (!idx_e || !zp_expect(p, TOK_RBRACK, "']'")) return NULL;
            ZrExpr *ie = zp_alloc_expr(p, EX_SAFE_INDEX, ln);
            ie->u.idx.obj = cur;
            ie->u.idx.idx = idx_e;
            cur = ie;
        } else if (zp_check(p, TOK_COLON) &&
                   (zp_peek_n(p, 1)->kind == TOK_IDENT ||
                    (zp_peek_n(p, 1)->kind >= TOK_LET && zp_peek_n(p, 1)->kind <= TOK_NIL)) &&
                   zp_peek_n(p, 2)->kind == TOK_LPAREN) {
            /* Colon method call: obj:method(args) */
            zp_advance(p);
            const char *mname = zp_expect_any_name(p, "method name");
            zp_advance(p); /* '(' */
            ZrExpr **args = NULL; size_t argc = 0;
            if (!zp_parse_call_args(p, &args, &argc)) return NULL;
            ZrExpr *me = zp_alloc_expr(p, EX_METHOD_CALL, ln);
            me->u.mcall.recv = cur;
            me->u.mcall.method = mname;
            me->u.mcall.args = args;
            me->u.mcall.argc = argc;
            cur = me;
        } else if (zp_check(p, TOK_QMARK)) {
            /* Distinguish postfix unwrap `expr?` from ternary `cond ? then : else`:
               Look ahead: if `?` is immediately at end of expr (followed by `;`, `,`, `)`, `]`, `}`,
               binary operator, etc.), it is postfix unwrap! Wait: what if `cond ? a : b`?
               Let's scan balanced tokens after `?` to see if a top-level `:` exists before `;`/`)`/`]`/`}`! */
            bool is_ternary = false;
            int d_par = 0, d_brk = 0, d_brc = 0;
            for (size_t k = p->pos + 1; k < p->len; k++) {
                ZrTokKind tk = p->toks[k].kind;
                if (tk == TOK_EOF) break;
                if (tk == TOK_LPAREN) d_par++;
                else if (tk == TOK_RPAREN) { if (d_par == 0) break; d_par--; }
                else if (tk == TOK_LBRACK || tk == TOK_QLBRACK) d_brk++;
                else if (tk == TOK_RBRACK) { if (d_brk == 0) break; d_brk--; }
                else if (tk == TOK_LBRACE) d_brc++;
                else if (tk == TOK_RBRACE) { if (d_brc == 0) break; d_brc--; }
                else if (d_par == 0 && d_brk == 0 && d_brc == 0) {
                    if (tk == TOK_SEMI || tk == TOK_COMMA) break;
                    if (tk == TOK_COLON && (k + 2 >= p->len || p->toks[k + 2].kind != TOK_LPAREN)) {
                        is_ternary = true;
                        break;
                    }
                }
            }
            if (is_ternary) break;
            zp_advance(p);
            ZrExpr *ue = zp_alloc_expr(p, EX_UNWRAP, ln);
            ue->u.unary.op = TOK_QMARK;
            ue->u.unary.rhs = cur;
            cur = ue;
        } else {
            break;
        }
    }
    return cur;
}

static ZrExpr *zp_parse_unary(ZrParser *p) {
    uint32_t ln = zp_peek(p)->line;
    if (zp_match(p, TOK_AWAIT)) {
        return zp_parse_unary(p);
    }
    if (zp_match(p, TOK_MINUS)) {
        ZrExpr *rhs = zp_parse_unary(p);
        if (!rhs) return NULL;
        if (rhs->kind == EX_NUM) return zp_mk_num(p, -rhs->u.num, ln);
        ZrExpr *e = zp_alloc_expr(p, EX_UNARY, ln);
        e->u.unary.op = TOK_MINUS;
        e->u.unary.rhs = rhs;
        return e;
    }
    if (zp_match(p, TOK_NOT) || zp_match(p, TOK_BANG)) {
        ZrExpr *rhs = zp_parse_unary(p);
        if (!rhs) return NULL;
        ZrExpr *e = zp_alloc_expr(p, EX_UNARY, ln);
        e->u.unary.op = TOK_NOT;
        e->u.unary.rhs = rhs;
        return e;
    }
    if (zp_match(p, TOK_TILDE)) {
        ZrExpr *rhs = zp_parse_unary(p);
        if (!rhs) return NULL;
        return zp_mk_call1(p, "bit_not", rhs, ln);
    }
    if (zp_match(p, TOK_THROW)) {
        ZrExpr *rhs = zp_parse_unary(p);
        if (!rhs) return NULL;
        ZrExpr *e = zp_alloc_expr(p, EX_UNARY, ln);
        e->u.unary.op = TOK_THROW;
        e->u.unary.rhs = rhs;
        return e;
    }
    if (zp_match(p, TOK_PLUS)) {
        return zp_parse_unary(p);
    }
    return zp_parse_postfix(p);
}

static ZrExpr *zp_parse_pow(ZrParser *p) {
    ZrExpr *lhs = zp_parse_unary(p);
    if (!lhs) return NULL;
    if (zp_match(p, TOK_STARSTAR)) {
        uint32_t ln = zp_peek(p)->line;
        ZrExpr *rhs = zp_parse_pow(p); /* right-associative */
        if (!rhs) return NULL;
        return zp_mk_bin(p, TOK_STARSTAR, lhs, rhs, ln);
    }
    return lhs;
}

static ZrExpr *zp_parse_mul(ZrParser *p) {
    ZrExpr *lhs = zp_parse_pow(p);
    if (!lhs) return NULL;
    while (zp_check(p, TOK_STAR) || zp_check(p, TOK_SLASH) || zp_check(p, TOK_PERCENT)) {
        ZrToken *op = zp_advance(p);
        ZrExpr *rhs = zp_parse_pow(p);
        if (!rhs) return NULL;
        lhs = zp_mk_bin(p, op->kind, lhs, rhs, op->line);
    }
    return lhs;
}

static ZrExpr *zp_parse_add(ZrParser *p) {
    ZrExpr *lhs = zp_parse_mul(p);
    if (!lhs) return NULL;
    while (zp_check(p, TOK_PLUS) || zp_check(p, TOK_MINUS)) {
        ZrToken *op = zp_advance(p);
        ZrExpr *rhs = zp_parse_mul(p);
        if (!rhs) return NULL;
        lhs = zp_mk_bin(p, op->kind, lhs, rhs, op->line);
    }
    return lhs;
}

static ZrExpr *zp_parse_shift(ZrParser *p) {
    ZrExpr *lhs = zp_parse_add(p);
    if (!lhs) return NULL;
    while (zp_check(p, TOK_SHL) || zp_check(p, TOK_SHR)) {
        ZrToken *op = zp_advance(p);
        ZrExpr *rhs = zp_parse_add(p);
        if (!rhs) return NULL;
        lhs = zp_mk_call2(p, op->kind == TOK_SHL ? "bit_shl" : "bit_shr", lhs, rhs, op->line);
    }
    return lhs;
}

static ZrExpr *zp_parse_bitand(ZrParser *p) {
    ZrExpr *lhs = zp_parse_shift(p);
    if (!lhs) return NULL;
    while (zp_check(p, TOK_AMP)) {
        ZrToken *op = zp_advance(p);
        ZrExpr *rhs = zp_parse_shift(p);
        if (!rhs) return NULL;
        lhs = zp_mk_call2(p, "bit_and", lhs, rhs, op->line);
    }
    return lhs;
}

static ZrExpr *zp_parse_bitxor(ZrParser *p) {
    ZrExpr *lhs = zp_parse_bitand(p);
    if (!lhs) return NULL;
    while (zp_check(p, TOK_CARET)) {
        ZrToken *op = zp_advance(p);
        ZrExpr *rhs = zp_parse_bitand(p);
        if (!rhs) return NULL;
        lhs = zp_mk_call2(p, "bit_xor", lhs, rhs, op->line);
    }
    return lhs;
}

static ZrExpr *zp_parse_bitor(ZrParser *p) {
    ZrExpr *lhs = zp_parse_bitxor(p);
    if (!lhs) return NULL;
    while (zp_check(p, TOK_PIPE)) {
        ZrToken *op = zp_advance(p);
        ZrExpr *rhs = zp_parse_bitxor(p);
        if (!rhs) return NULL;
        lhs = zp_mk_call2(p, "bit_or", lhs, rhs, op->line);
    }
    return lhs;
}

static ZrExpr *zp_parse_cmp(ZrParser *p) {
    ZrExpr *lhs = zp_parse_bitor(p);
    if (!lhs) return NULL;
    while (zp_check(p, TOK_LT) || zp_check(p, TOK_LTEQ) || zp_check(p, TOK_GT) || zp_check(p, TOK_GTEQ)) {
        ZrToken *op = zp_advance(p);
        ZrExpr *rhs = zp_parse_bitor(p);
        if (!rhs) return NULL;
        lhs = zp_mk_bin(p, op->kind, lhs, rhs, op->line);
    }
    return lhs;
}

static ZrExpr *zp_parse_eq(ZrParser *p) {
    ZrExpr *lhs = zp_parse_cmp(p);
    if (!lhs) return NULL;
    while (zp_check(p, TOK_EQEQ) || zp_check(p, TOK_BANGEQ)) {
        ZrToken *op = zp_advance(p);
        ZrExpr *rhs = zp_parse_cmp(p);
        if (!rhs) return NULL;
        lhs = zp_mk_bin(p, op->kind, lhs, rhs, op->line);
    }
    return lhs;
}

static ZrExpr *zp_parse_and(ZrParser *p) {
    ZrExpr *lhs = zp_parse_eq(p);
    if (!lhs) return NULL;
    while (zp_check(p, TOK_AND)) {
        ZrToken *op = zp_advance(p);
        ZrExpr *rhs = zp_parse_eq(p);
        if (!rhs) return NULL;
        ZrExpr *e = zp_alloc_expr(p, EX_LOGICAL, op->line);
        e->u.bin.op = TOK_AND;
        e->u.bin.lhs = lhs;
        e->u.bin.rhs = rhs;
        lhs = e;
    }
    return lhs;
}

static ZrExpr *zp_parse_or(ZrParser *p) {
    ZrExpr *lhs = zp_parse_and(p);
    if (!lhs) return NULL;
    while (zp_check(p, TOK_OR)) {
        ZrToken *op = zp_advance(p);
        ZrExpr *rhs = zp_parse_and(p);
        if (!rhs) return NULL;
        ZrExpr *e = zp_alloc_expr(p, EX_LOGICAL, op->line);
        e->u.bin.op = TOK_OR;
        e->u.bin.lhs = lhs;
        e->u.bin.rhs = rhs;
        lhs = e;
    }
    return lhs;
}

static ZrExpr *zp_parse_coalesce(ZrParser *p) {
    ZrExpr *lhs = zp_parse_or(p);
    if (!lhs) return NULL;
    while (zp_check(p, TOK_QQ)) {
        ZrToken *op = zp_advance(p);
        ZrExpr *rhs = zp_parse_or(p);
        if (!rhs) return NULL;
        ZrExpr *e = zp_alloc_expr(p, EX_LOGICAL, op->line);
        e->u.bin.op = TOK_QQ;
        e->u.bin.lhs = lhs;
        e->u.bin.rhs = rhs;
        lhs = e;
    }
    return lhs;
}

static ZrExpr *zp_parse_pipeline(ZrParser *p) {
    ZrExpr *lhs = zp_parse_coalesce(p);
    if (!lhs) return NULL;
    while (zp_check(p, TOK_PIPEGT)) {
        ZrToken *op = zp_advance(p);
        ZrExpr *rhs = zp_parse_coalesce(p);
        if (!rhs) return NULL;
        if (rhs->kind == EX_CALL) {
            /* Replace `_` placeholder if present, else prepend `lhs` as arg 0 */
            bool replaced = false;
            for (size_t i = 0; i < rhs->u.call.argc; i++) {
                if (rhs->u.call.args[i]->kind == EX_VAR && strcmp(rhs->u.call.args[i]->u.var, "_") == 0) {
                    rhs->u.call.args[i] = lhs;
                    replaced = true;
                }
            }
            if (!replaced) {
                ZrExpr **nargs = (ZrExpr **)zr_arena_alloc(p->arena, (rhs->u.call.argc + 1) * sizeof(ZrExpr *));
                nargs[0] = lhs;
                if (rhs->u.call.argc > 0) memcpy(nargs + 1, rhs->u.call.args, rhs->u.call.argc * sizeof(ZrExpr *));
                rhs->u.call.args = nargs;
                rhs->u.call.argc++;
            }
            lhs = rhs;
        } else {
            ZrExpr *ce = zp_alloc_expr(p, EX_CALL, op->line);
            ce->u.call.callee = rhs;
            ce->u.call.args = (ZrExpr **)zr_arena_alloc(p->arena, sizeof(ZrExpr *));
            ce->u.call.args[0] = lhs;
            ce->u.call.argc = 1;
            lhs = ce;
        }
    }
    return lhs;
}

static ZrExpr *zp_parse_expr(ZrParser *p) {
    ZrExpr *cond = zp_parse_pipeline(p);
    if (!cond) return NULL;
    if (zp_match(p, TOK_QMARK)) {
        uint32_t ln = zp_peek(p)->line;
        ZrExpr *t_br = zp_parse_expr(p);
        if (!t_br || !zp_expect(p, TOK_COLON, "':' in ternary expression")) return NULL;
        ZrExpr *f_br = zp_parse_expr(p);
        if (!f_br) return NULL;
        ZrExpr *te = zp_alloc_expr(p, EX_TERNARY, ln);
        te->u.tern.cond = cond;
        te->u.tern.t_br = t_br;
        te->u.tern.f_br = f_br;
        return te;
    }
    return cond;
}

/* ---- Statement Parsing & Desugaring ---- */

static bool zp_parse_block_stmts(ZrParser *p, ZrStmt ***out_stmts, size_t *out_count) {
    if (!zp_expect(p, TOK_LBRACE, "'{'")) return false;
    ZrStmt **stmts = NULL;
    size_t n = 0, cap = 0;
    while (!zp_check(p, TOK_RBRACE) && !zp_check(p, TOK_EOF)) {
        if (zp_match(p, TOK_SEMI)) continue;
        ZrStmt *s = zp_parse_stmt(p);
        if (!s) return false;
        zp_match(p, TOK_SEMI);
        if (n >= cap) {
            cap = cap ? cap * 2 : 8;
            ZrStmt **ns = (ZrStmt **)zr_arena_alloc(p->arena, cap * sizeof(ZrStmt *));
            if (n > 0) memcpy(ns, stmts, n * sizeof(ZrStmt *));
            stmts = ns;
        }
        stmts[n++] = s;
    }
    if (!zp_expect(p, TOK_RBRACE, "'}'")) return false;
    *out_stmts = stmts;
    *out_count = n;
    return true;
}

static ZrStmt *zp_parse_if_stmt(ZrParser *p, uint32_t line) {
    bool saved_nb = p->no_struct_brace;
    p->no_struct_brace = true;
    ZrExpr *cond = zp_parse_expr(p);
    p->no_struct_brace = saved_nb;
    if (!cond) return NULL;
    ZrStmt **then_b = NULL; size_t then_n = 0;
    if (!zp_parse_block_stmts(p, &then_b, &then_n)) return NULL;

    ZrStmt **else_b = NULL; size_t else_n = 0;
    if (zp_match(p, TOK_ELIF)) {
        ZrStmt *nested = zp_parse_if_stmt(p, zp_peek(p)->line);
        if (!nested) return NULL;
        else_b = (ZrStmt **)zr_arena_alloc(p->arena, sizeof(ZrStmt *));
        else_b[0] = nested;
        else_n = 1;
    } else if (zp_match(p, TOK_ELSE)) {
        if (zp_match(p, TOK_IF)) {
            ZrStmt *nested = zp_parse_if_stmt(p, zp_peek(p)->line);
            if (!nested) return NULL;
            else_b = (ZrStmt **)zr_arena_alloc(p->arena, sizeof(ZrStmt *));
            else_b[0] = nested;
            else_n = 1;
        } else {
            if (!zp_parse_block_stmts(p, &else_b, &else_n)) return NULL;
        }
    }
    ZrStmt *s = zp_alloc_stmt(p, ST_IF, line);
    s->u.if_s.cond = cond;
    s->u.if_s.then_b = then_b;
    s->u.if_s.then_n = then_n;
    s->u.if_s.else_b = else_b;
    s->u.if_s.else_n = else_n;
    return s;
}

static ZrStmt *zp_parse_let_or_const(ZrParser *p, bool is_const, uint32_t line) {
    /* Array destructuring: let [a, b, ...rest] = expr */
    if (!is_const && zp_match(p, TOK_LBRACK)) {
        const char **names = NULL;
        bool *is_rest = NULL;
        size_t n = 0, cap = 0;
        while (!zp_check(p, TOK_RBRACK) && !zp_check(p, TOK_EOF)) {
            bool r = zp_match(p, TOK_ELLIPSIS);
            const char *nm = zp_expect_decl_name(p, "destructuring binding");
            if (!nm) return NULL;
            if (n >= cap) {
                cap = cap ? cap * 2 : 4;
                const char **nn = (const char **)zr_arena_alloc(p->arena, cap * sizeof(const char *));
                bool *nr = (bool *)zr_arena_alloc(p->arena, cap * sizeof(bool));
                if (n > 0) { memcpy(nn, names, n * sizeof(const char *)); memcpy(nr, is_rest, n * sizeof(bool)); }
                names = nn; is_rest = nr;
            }
            names[n] = nm;
            is_rest[n] = r;
            n++;
            if (!zp_match(p, TOK_COMMA)) break;
        }
        if (!zp_expect(p, TOK_RBRACK, "']'") || !zp_expect(p, TOK_EQ, "'='")) return NULL;
        ZrExpr *rhs = zp_parse_expr(p);
        if (!rhs) return NULL;

        const char *tmp = zp_gensym(p, "adest");
        ZrStmt **seq = (ZrStmt **)zr_arena_alloc(p->arena, (n + 2) * sizeof(ZrStmt *));
        size_t sc = 0;
        ZrStmt *s0 = zp_alloc_stmt(p, ST_LET, line);
        s0->u.let_s.name = tmp;
        s0->u.let_s.init = rhs;
        seq[sc++] = s0;

        size_t reg_idx = 0;
        for (size_t i = 0; i < n; i++) {
            ZrStmt *si = zp_alloc_stmt(p, ST_LET, line);
            si->u.let_s.name = names[i];
            if (is_rest[i]) {
                si->u.let_s.init = zp_mk_call2(p, "array_drop", zp_mk_var(p, tmp, line), zp_mk_num(p, (double)reg_idx, line), line);
            } else {
                ZrExpr *ie = zp_alloc_expr(p, EX_INDEX, line);
                ie->u.idx.obj = zp_mk_var(p, tmp, line);
                ie->u.idx.idx = zp_mk_num(p, (double)reg_idx, line);
                si->u.let_s.init = ie;
                reg_idx++;
            }
            seq[sc++] = si;
        }
        ZrStmt *res = zp_alloc_stmt(p, ST_SEQ, line);
        res->u.blk.stmts = seq;
        res->u.blk.count = sc;
        return res;
    }

    /* Map destructuring: let {x, k: alias} = expr */
    if (!is_const && zp_match(p, TOK_LBRACE)) {
        const char **keys = NULL;
        const char **locals = NULL;
        size_t n = 0, cap = 0;
        while (!zp_check(p, TOK_RBRACE) && !zp_check(p, TOK_EOF)) {
            const char *k = zp_expect_any_name(p, "map destructuring key");
            if (!k) return NULL;
            const char *loc = k;
            if (zp_match(p, TOK_COLON)) {
                loc = zp_expect_decl_name(p, "local binding name");
                if (!loc) return NULL;
            }
            if (n >= cap) {
                cap = cap ? cap * 2 : 4;
                const char **nk = (const char **)zr_arena_alloc(p->arena, cap * sizeof(const char *));
                const char **nl = (const char **)zr_arena_alloc(p->arena, cap * sizeof(const char *));
                if (n > 0) { memcpy(nk, keys, n * sizeof(const char *)); memcpy(nl, locals, n * sizeof(const char *)); }
                keys = nk; locals = nl;
            }
            keys[n] = k;
            locals[n] = loc;
            n++;
            if (!zp_match(p, TOK_COMMA)) break;
        }
        if (!zp_expect(p, TOK_RBRACE, "'}'") || !zp_expect(p, TOK_EQ, "'='")) return NULL;
        ZrExpr *rhs = zp_parse_expr(p);
        if (!rhs) return NULL;

        const char *tmp = zp_gensym(p, "mdest");
        ZrStmt **seq = (ZrStmt **)zr_arena_alloc(p->arena, (n + 2) * sizeof(ZrStmt *));
        size_t sc = 0;
        ZrStmt *s0 = zp_alloc_stmt(p, ST_LET, line);
        s0->u.let_s.name = tmp;
        s0->u.let_s.init = rhs;
        seq[sc++] = s0;
        for (size_t i = 0; i < n; i++) {
            ZrStmt *si = zp_alloc_stmt(p, ST_LET, line);
            si->u.let_s.name = locals[i];
            ZrExpr *ae = zp_alloc_expr(p, EX_ATTR, line);
            ae->u.attr.obj = zp_mk_var(p, tmp, line);
            ae->u.attr.attr = keys[i];
            si->u.let_s.init = ae;
            seq[sc++] = si;
        }
        ZrStmt *res = zp_alloc_stmt(p, ST_SEQ, line);
        res->u.blk.stmts = seq;
        res->u.blk.count = sc;
        return res;
    }

    const char *vname = zp_expect_decl_name(p, "variable name");
    if (!vname) return NULL;
    if (zp_match(p, TOK_COLON)) {
        if (!zp_skip_type_expr(p)) return NULL;
    }
    ZrExpr *init = NULL;
    if (zp_match(p, TOK_EQ)) {
        init = zp_parse_expr(p);
        if (!init) return NULL;
    } else if (is_const) {
        zp_err(p, "const '%s' requires '= <expr>'", vname);
        return NULL;
    } else {
        init = zp_alloc_expr(p, EX_NIL, line);
    }
    ZrStmt *s = zp_alloc_stmt(p, ST_LET, line);
    s->u.let_s.name = vname;
    s->u.let_s.init = init;
    s->u.let_s.is_const = is_const;
    return s;
}

/* Desugar `struct Name { f1: T1, f2: T2 }` -> `fn Name(f1, f2) { return map{ __struct: "Name", f1: f1, f2: f2 }; }` */
static ZrStmt *zp_parse_struct_stmt(ZrParser *p, uint32_t line) {
    const char *sname = zp_expect_decl_name(p, "struct name");
    if (!sname || !zp_expect(p, TOK_LBRACE, "'{'")) return NULL;
    const char **fields = NULL;
    size_t n = 0, cap = 0;
    while (!zp_check(p, TOK_RBRACE) && !zp_check(p, TOK_EOF)) {
        const char *fnm = zp_expect_any_name(p, "struct field name");
        if (!fnm) return NULL;
        if (zp_match(p, TOK_COLON)) {
            if (!zp_skip_type_expr(p)) return NULL;
        }
        if (n >= cap) {
            cap = cap ? cap * 2 : 4;
            const char **nf = (const char **)zr_arena_alloc(p->arena, cap * sizeof(const char *));
            if (n > 0) memcpy(nf, fields, n * sizeof(const char *));
            fields = nf;
        }
        fields[n++] = fnm;
        if (!zp_match(p, TOK_COMMA) && !zp_match(p, TOK_SEMI)) break;
    }
    if (!zp_expect(p, TOK_RBRACE, "'}'")) return NULL;

    ZrParam *params = (ZrParam *)zr_arena_alloc(p->arena, (n ? n : 1) * sizeof(ZrParam));
    ZrExpr **mkeys = (ZrExpr **)zr_arena_alloc(p->arena, (n + 1) * sizeof(ZrExpr *));
    ZrExpr **mvals = (ZrExpr **)zr_arena_alloc(p->arena, (n + 1) * sizeof(ZrExpr *));
    mkeys[0] = zp_mk_str(p, "__struct", 8, line);
    mvals[0] = zp_mk_str(p, sname, strlen(sname), line);
    for (size_t i = 0; i < n; i++) {
        params[i].name = fields[i];
        params[i].def_expr = NULL;
        params[i].is_vararg = false;
        mkeys[i + 1] = zp_mk_str(p, fields[i], strlen(fields[i]), line);
        mvals[i + 1] = zp_mk_var(p, fields[i], line);
    }
    ZrExpr *me = zp_alloc_expr(p, EX_MAP, line);
    me->u.map.keys = mkeys;
    me->u.map.vals = mvals;
    me->u.map.count = n + 1;

    ZrStmt *ret = zp_alloc_stmt(p, ST_RETURN, line);
    ret->u.expr = me;
    ZrStmt **body = (ZrStmt **)zr_arena_alloc(p->arena, sizeof(ZrStmt *));
    body[0] = ret;

    ZrStmt *fn_s = zp_alloc_stmt(p, ST_FN, line);
    fn_s->u.fn_s.name = sname;
    fn_s->u.fn_s.params = params;
    fn_s->u.fn_s.param_count = n;
    fn_s->u.fn_s.body = body;
    fn_s->u.fn_s.body_count = 1;
    return fn_s;
}

/* Desugar `interface Name { m1; m2(a); }` -> `fn Name(x) { return is_map(x) and has_member(x, "m1") ...; }` */
static ZrStmt *zp_parse_interface_stmt(ZrParser *p, uint32_t line) {
    const char *iname = zp_expect_decl_name(p, "interface name");
    if (!iname || !zp_expect(p, TOK_LBRACE, "'{'")) return NULL;
    ZrExpr *acc = zp_mk_call1(p, "is_map", zp_mk_var(p, "x", line), line);
    while (!zp_check(p, TOK_RBRACE) && !zp_check(p, TOK_EOF)) {
        const char *mname = zp_expect_any_name(p, "interface member");
        if (!mname) return NULL;
        if (zp_match(p, TOK_LPAREN)) {
            int d = 1;
            while (d > 0 && !zp_check(p, TOK_EOF)) {
                if (zp_match(p, TOK_LPAREN)) d++;
                else if (zp_match(p, TOK_RPAREN)) d--;
                else zp_advance(p);
            }
        }
        if (zp_match(p, TOK_COLON) || zp_match(p, TOK_ARROW)) {
            if (!zp_skip_type_expr(p)) return NULL;
        }
        zp_match(p, TOK_SEMI);
        zp_match(p, TOK_COMMA);
        ZrExpr *hm = zp_mk_call2(p, "has_member", zp_mk_var(p, "x", line), zp_mk_str(p, mname, strlen(mname), line), line);
        ZrExpr *and_e = zp_alloc_expr(p, EX_LOGICAL, line);
        and_e->u.bin.op = TOK_AND;
        and_e->u.bin.lhs = acc;
        and_e->u.bin.rhs = hm;
        acc = and_e;
    }
    if (!zp_expect(p, TOK_RBRACE, "'}'")) return NULL;
    ZrParam *params = (ZrParam *)zr_arena_alloc(p->arena, sizeof(ZrParam));
    params[0].name = "x";
    ZrStmt *ret = zp_alloc_stmt(p, ST_RETURN, line);
    ret->u.expr = acc;
    ZrStmt **body = (ZrStmt **)zr_arena_alloc(p->arena, sizeof(ZrStmt *));
    body[0] = ret;
    ZrStmt *fn_s = zp_alloc_stmt(p, ST_FN, line);
    fn_s->u.fn_s.name = iname;
    fn_s->u.fn_s.params = params;
    fn_s->u.fn_s.param_count = 1;
    fn_s->u.fn_s.body = body;
    fn_s->u.fn_s.body_count = 1;
    return fn_s;
}

/* Desugar `enum Name { Red, Circle(number) }` */
static ZrStmt *zp_parse_enum_stmt(ZrParser *p, uint32_t line) {
    const char *ename = zp_expect_decl_name(p, "enum name");
    if (!ename || !zp_expect(p, TOK_LBRACE, "'{'")) return NULL;
    ZrExpr **ekeys = NULL, **evals = NULL;
    size_t n = 0, cap = 0;
    while (!zp_check(p, TOK_RBRACE) && !zp_check(p, TOK_EOF)) {
        const char *vname = zp_expect_any_name(p, "enum variant");
        if (!vname) return NULL;
        ZrExpr *ventry = NULL;
        if (zp_match(p, TOK_LPAREN)) {
            size_t arity = 0;
            while (!zp_check(p, TOK_RPAREN) && !zp_check(p, TOK_EOF)) {
                if (!zp_skip_type_expr(p)) return NULL;
                arity++;
                if (!zp_match(p, TOK_COMMA)) break;
            }
            if (!zp_expect(p, TOK_RPAREN, "')'")) return NULL;
            ZrParam *params = (ZrParam *)zr_arena_alloc(p->arena, (arity ? arity : 1) * sizeof(ZrParam));
            ZrExpr **varr_items = (ZrExpr **)zr_arena_alloc(p->arena, (arity ? arity : 1) * sizeof(ZrExpr *));
            for (size_t i = 0; i < arity; i++) {
                char pnbuf[24];
                snprintf(pnbuf, sizeof(pnbuf), "_%zu", i);
                params[i].name = zr_arena_strdup(p->arena, pnbuf);
                varr_items[i] = zp_mk_var(p, params[i].name, line);
            }
            ZrExpr *val_arr = zp_alloc_expr(p, EX_ARRAY, line);
            val_arr->u.arr.items = varr_items;
            val_arr->u.arr.count = arity;

            ZrExpr **vk = (ZrExpr **)zr_arena_alloc(p->arena, 3 * sizeof(ZrExpr *));
            ZrExpr **vv = (ZrExpr **)zr_arena_alloc(p->arena, 3 * sizeof(ZrExpr *));
            vk[0] = zp_mk_str(p, "__enum", 6, line);    vv[0] = zp_mk_str(p, ename, strlen(ename), line);
            vk[1] = zp_mk_str(p, "__variant", 9, line); vv[1] = zp_mk_str(p, vname, strlen(vname), line);
            vk[2] = zp_mk_str(p, "values", 6, line);    vv[2] = val_arr;
            ZrExpr *rmap = zp_alloc_expr(p, EX_MAP, line);
            rmap->u.map.keys = vk; rmap->u.map.vals = vv; rmap->u.map.count = 3;

            ZrStmt *rs = zp_alloc_stmt(p, ST_RETURN, line);
            rs->u.expr = rmap;
            ZrStmt **fbody = (ZrStmt **)zr_arena_alloc(p->arena, sizeof(ZrStmt *));
            fbody[0] = rs;
            ventry = zp_alloc_expr(p, EX_FN, line);
            ventry->u.fn.params = params;
            ventry->u.fn.param_count = arity;
            ventry->u.fn.body = fbody;
            ventry->u.fn.body_count = 1;
        } else {
            ZrExpr **vk = (ZrExpr **)zr_arena_alloc(p->arena, 2 * sizeof(ZrExpr *));
            ZrExpr **vv = (ZrExpr **)zr_arena_alloc(p->arena, 2 * sizeof(ZrExpr *));
            vk[0] = zp_mk_str(p, "__enum", 6, line);    vv[0] = zp_mk_str(p, ename, strlen(ename), line);
            vk[1] = zp_mk_str(p, "__variant", 9, line); vv[1] = zp_mk_str(p, vname, strlen(vname), line);
            ventry = zp_alloc_expr(p, EX_MAP, line);
            ventry->u.map.keys = vk; ventry->u.map.vals = vv; ventry->u.map.count = 2;
        }
        if (n >= cap) {
            cap = cap ? cap * 2 : 4;
            ZrExpr **nk = (ZrExpr **)zr_arena_alloc(p->arena, cap * sizeof(ZrExpr *));
            ZrExpr **nv = (ZrExpr **)zr_arena_alloc(p->arena, cap * sizeof(ZrExpr *));
            if (n > 0) { memcpy(nk, ekeys, n * sizeof(ZrExpr *)); memcpy(nv, evals, n * sizeof(ZrExpr *)); }
            ekeys = nk; evals = nv;
        }
        ekeys[n] = zp_mk_str(p, vname, strlen(vname), line);
        evals[n] = ventry;
        n++;
        if (!zp_match(p, TOK_COMMA) && !zp_match(p, TOK_SEMI)) break;
    }
    if (!zp_expect(p, TOK_RBRACE, "'}'")) return NULL;
    ZrExpr *emap = zp_alloc_expr(p, EX_MAP, line);
    emap->u.map.keys = ekeys; emap->u.map.vals = evals; emap->u.map.count = n;
    ZrStmt *ls = zp_alloc_stmt(p, ST_LET, line);
    ls->u.let_s.name = ename;
    ls->u.let_s.init = emap;
    return ls;
}

/* Desugar `class Name [extends Base] [implements ...] { fn m(self, ...) { ... } }` */
static ZrStmt *zp_parse_class_stmt(ZrParser *p, uint32_t line) {
    const char *cname = zp_expect_decl_name(p, "class name");
    if (!cname) return NULL;
    const char *parent_name = NULL;
    if (zp_check_ident_str(p, "extends")) {
        zp_advance(p);
        parent_name = zp_expect_decl_name(p, "parent class name");
        if (!parent_name) return NULL;
    }
    if (zp_check_ident_str(p, "implements")) {
        zp_advance(p);
        do {
            if (!zp_expect_decl_name(p, "interface name")) return NULL;
        } while (zp_match(p, TOK_COMMA));
    }
    if (!zp_expect(p, TOK_LBRACE, "'{'")) return NULL;

    ZrExpr **mkeys = NULL, **mvals = NULL;
    size_t mn = 0, mcap = 0;
    int init_arity = -1;

    if (parent_name) {
        mcap = 8;
        mkeys = (ZrExpr **)zr_arena_alloc(p->arena, mcap * sizeof(ZrExpr *));
        mvals = (ZrExpr **)zr_arena_alloc(p->arena, mcap * sizeof(ZrExpr *));
        mkeys[mn] = zp_mk_str(p, "__class", 7, line);
        mvals[mn] = zp_mk_var(p, parent_name, line);
        mn++;
    }

    while (!zp_check(p, TOK_RBRACE) && !zp_check(p, TOK_EOF)) {
        zp_match(p, TOK_FN); /* optional `fn` keyword inside class body */
        const char *mname = zp_expect_any_name(p, "method name");
        if (!mname) return NULL;
        ZrParam *params = NULL; size_t pcount = 0;
        if (!zp_parse_param_list(p, &params, &pcount)) return NULL;
        ZrStmt **body = NULL; size_t bcount = 0;
        if (!zp_parse_block_stmts(p, &body, &bcount)) return NULL;
        zp_match(p, TOK_SEMI);

        if (strcmp(mname, "init") == 0) {
            init_arity = (pcount > 0) ? (int)(pcount - 1) : 0;
        }
        ZrExpr *fe = zp_alloc_expr(p, EX_FN, line);
        fe->u.fn.name = mname;
        fe->u.fn.params = params;
        fe->u.fn.param_count = pcount;
        fe->u.fn.body = body;
        fe->u.fn.body_count = bcount;

        if (mn >= mcap) {
            mcap = mcap ? mcap * 2 : 8;
            ZrExpr **nk = (ZrExpr **)zr_arena_alloc(p->arena, mcap * sizeof(ZrExpr *));
            ZrExpr **nv = (ZrExpr **)zr_arena_alloc(p->arena, mcap * sizeof(ZrExpr *));
            if (mn > 0) { memcpy(nk, mkeys, mn * sizeof(ZrExpr *)); memcpy(nv, mvals, mn * sizeof(ZrExpr *)); }
            mkeys = nk; mvals = nv;
        }
        mkeys[mn] = zp_mk_str(p, mname, strlen(mname), line);
        mvals[mn] = fe;
        mn++;
    }
    if (!zp_expect(p, TOK_RBRACE, "'}'")) return NULL;

    ZrExpr *cmap = zp_alloc_expr(p, EX_MAP, line);
    cmap->u.map.keys = mkeys; cmap->u.map.vals = mvals; cmap->u.map.count = mn;

    /* Build `let ClassName = map{...};` + `new0..new4` + `new` + `__call` */
    ZrStmt **seq = (ZrStmt **)zr_arena_alloc(p->arena, 10 * sizeof(ZrStmt *));
    size_t sc = 0;
    ZrStmt *s_let = zp_alloc_stmt(p, ST_LET, line);
    s_let->u.let_s.name = cname;
    s_let->u.let_s.init = cmap;
    seq[sc++] = s_let;

    int max_new = (init_arity > 3) ? init_arity : 3;
    if (max_new > 6) max_new = 6;
    for (int k = 0; k <= max_new; k++) {
        ZrParam *np = (ZrParam *)zr_arena_alloc(p->arena, (k ? k : 1) * sizeof(ZrParam));
        ZrExpr **iargs = (ZrExpr **)zr_arena_alloc(p->arena, (k + 1) * sizeof(ZrExpr *));
        iargs[0] = zp_mk_var(p, "__obj", line);
        for (int a = 0; a < k; a++) {
            char abuf[16];
            snprintf(abuf, sizeof(abuf), "a%d", a);
            np[a].name = zr_arena_strdup(p->arena, abuf);
            iargs[a + 1] = zp_mk_var(p, np[a].name, line);
        }
        ZrExpr **ok = (ZrExpr **)zr_arena_alloc(p->arena, sizeof(ZrExpr *));
        ZrExpr **ov = (ZrExpr **)zr_arena_alloc(p->arena, sizeof(ZrExpr *));
        ok[0] = zp_mk_str(p, "__class", 7, line);
        ov[0] = zp_mk_var(p, cname, line);
        ZrExpr *omap = zp_alloc_expr(p, EX_MAP, line);
        omap->u.map.keys = ok; omap->u.map.vals = ov; omap->u.map.count = 1;

        ZrStmt *lo = zp_alloc_stmt(p, ST_LET, line);
        lo->u.let_s.name = "__obj";
        lo->u.let_s.init = omap;

        ZrExpr *init_attr = zp_alloc_expr(p, EX_ATTR, line);
        init_attr->u.attr.obj = zp_mk_var(p, cname, line);
        init_attr->u.attr.attr = "init";
        ZrExpr *init_call = zp_alloc_expr(p, EX_CALL, line);
        init_call->u.call.callee = init_attr;
        init_call->u.call.args = iargs;
        init_call->u.call.argc = (size_t)(k + 1);

        ZrExpr *try_init = zp_alloc_expr(p, EX_TRY, line);
        try_init->u.try_e.try_e = init_call;
        try_init->u.try_e.catch_e = zp_alloc_expr(p, EX_NIL, line);
        ZrStmt *st_try = zp_alloc_stmt(p, ST_EXPR, line);
        st_try->u.expr = try_init;

        ZrStmt *st_ret = zp_alloc_stmt(p, ST_RETURN, line);
        st_ret->u.expr = zp_mk_var(p, "__obj", line);

        ZrStmt **nbody = (ZrStmt **)zr_arena_alloc(p->arena, 3 * sizeof(ZrStmt *));
        nbody[0] = lo; nbody[1] = st_try; nbody[2] = st_ret;

        char new_name[16];
        snprintf(new_name, sizeof(new_name), "new%d", k);
        const char *nn = zr_arena_strdup(p->arena, new_name);
        ZrExpr *nfn = zp_alloc_expr(p, EX_FN, line);
        nfn->u.fn.name = nn;
        nfn->u.fn.params = np;
        nfn->u.fn.param_count = (size_t)k;
        nfn->u.fn.body = nbody;
        nfn->u.fn.body_count = 3;

        ZrStmt *sa = zp_alloc_stmt(p, ST_SET_ATTR, line);
        sa->u.sattr.obj = zp_mk_var(p, cname, line);
        sa->u.sattr.attr = nn;
        sa->u.sattr.rhs = nfn;
        seq[sc++] = sa;
    }

    int target_k = (init_arity >= 0 && init_arity <= max_new) ? init_arity : 0;
    char target_new[16];
    snprintf(target_new, sizeof(target_new), "new%d", target_k);
    const char *tn = zr_arena_strdup(p->arena, target_new);

    ZrExpr *get_tn = zp_alloc_expr(p, EX_ATTR, line);
    get_tn->u.attr.obj = zp_mk_var(p, cname, line);
    get_tn->u.attr.attr = tn;

    ZrStmt *s_new = zp_alloc_stmt(p, ST_SET_ATTR, line);
    s_new->u.sattr.obj = zp_mk_var(p, cname, line);
    s_new->u.sattr.attr = "new";
    s_new->u.sattr.rhs = get_tn;
    seq[sc++] = s_new;

    ZrStmt *s_call = zp_alloc_stmt(p, ST_SET_ATTR, line);
    s_call->u.sattr.obj = zp_mk_var(p, cname, line);
    s_call->u.sattr.attr = "__call";
    s_call->u.sattr.rhs = get_tn;
    seq[sc++] = s_call;

    ZrStmt *res = zp_alloc_stmt(p, ST_SEQ, line);
    res->u.blk.stmts = seq;
    res->u.blk.count = sc;
    return res;
}

/* Desugar `goto { label L1 { ... } label L2 { ... } }` matching parser.rs */
static ZrStmt *zp_parse_goto_block(ZrParser *p, uint32_t line) {
    if (!zp_expect(p, TOK_LBRACE, "'{' after goto")) return NULL;
    const char **lnames = NULL;
    ZrStmt ***lbodies = NULL;
    size_t *lcounts = NULL;
    size_t ln = 0, lcap = 0;

    bool saved_goto = p->in_goto_label;
    p->in_goto_label = true;
    while (!zp_check(p, TOK_RBRACE) && !zp_check(p, TOK_EOF)) {
        if (!zp_check_ident_str(p, "label")) {
            p->in_goto_label = saved_goto;
            zp_err(p, "expected 'label' inside goto block");
            return NULL;
        }
        zp_advance(p);
        const char *lbl = zp_expect_decl_name(p, "label name");
        if (!lbl) { p->in_goto_label = saved_goto; return NULL; }
        ZrStmt **b = NULL; size_t bc = 0;
        if (!zp_parse_block_stmts(p, &b, &bc)) { p->in_goto_label = saved_goto; return NULL; }
        if (ln >= lcap) {
            lcap = lcap ? lcap * 2 : 4;
            const char **nn = (const char **)zr_arena_alloc(p->arena, lcap * sizeof(const char *));
            ZrStmt ****nb = (ZrStmt ****)zr_arena_alloc(p->arena, lcap * sizeof(ZrStmt **));
            size_t *nc = (size_t *)zr_arena_alloc(p->arena, lcap * sizeof(size_t));
            if (ln > 0) {
                memcpy(nn, lnames, ln * sizeof(const char *));
                memcpy(nb, lbodies, ln * sizeof(ZrStmt **));
                memcpy(nc, lcounts, ln * sizeof(size_t));
            }
            lnames = nn; lbodies = (ZrStmt ***)nb; lcounts = nc;
        }
        lnames[ln] = lbl;
        lbodies[ln] = b;
        lcounts[ln] = bc;
        ln++;
    }
    p->in_goto_label = saved_goto;
    if (!zp_expect(p, TOK_RBRACE, "'}'")) return NULL;
    if (ln == 0) return zp_alloc_stmt(p, ST_BLOCK, line);

    const char *pc_var = zp_gensym(p, "pc");
    ZrStmt **blk_stmts = (ZrStmt **)zr_arena_alloc(p->arena, (ln + 2) * sizeof(ZrStmt *));
    size_t bs = 0;

    ZrStmt *let_pc = zp_alloc_stmt(p, ST_LET, line);
    let_pc->u.let_s.name = pc_var;
    let_pc->u.let_s.init = zp_mk_str(p, lnames[0], strlen(lnames[0]), line);
    blk_stmts[bs++] = let_pc;

    const char **fnames = (const char **)zr_arena_alloc(p->arena, ln * sizeof(const char *));
    for (size_t i = 0; i < ln; i++) {
        fnames[i] = zp_gensym(p, "lbl");
        const char *fall = (i + 1 < ln) ? lnames[i + 1] : "__done";
        ZrStmt **fb = (ZrStmt **)zr_arena_alloc(p->arena, (lcounts[i] + 1) * sizeof(ZrStmt *));
        if (lcounts[i] > 0) memcpy(fb, lbodies[i], lcounts[i] * sizeof(ZrStmt *));
        ZrStmt *ret_f = zp_alloc_stmt(p, ST_RETURN, line);
        ret_f->u.expr = zp_mk_str(p, fall, strlen(fall), line);
        fb[lcounts[i]] = ret_f;

        ZrStmt *fns = zp_alloc_stmt(p, ST_FN, line);
        fns->u.fn_s.name = fnames[i];
        fns->u.fn_s.body = fb;
        fns->u.fn_s.body_count = lcounts[i] + 1;
        blk_stmts[bs++] = fns;
    }

    /* while __pc != "__done" { if __pc == L0 { __pc = f0(); } ... else { break; } } */
    ZrStmt *brk = zp_alloc_stmt(p, ST_BREAK, line);
    ZrStmt **else_chain = (ZrStmt **)zr_arena_alloc(p->arena, sizeof(ZrStmt *));
    else_chain[0] = brk;
    size_t else_n = 1;

    for (size_t i = ln; i > 0; i--) {
        size_t idx = i - 1;
        ZrExpr *cond = zp_mk_bin(p, TOK_EQEQ, zp_mk_var(p, pc_var, line), zp_mk_str(p, lnames[idx], strlen(lnames[idx]), line), line);
        ZrExpr *call = zp_alloc_expr(p, EX_CALL, line);
        call->u.call.callee = zp_mk_var(p, fnames[idx], line);
        ZrStmt *asgn = zp_alloc_stmt(p, ST_ASSIGN, line);
        asgn->u.assign_s.name = pc_var;
        asgn->u.assign_s.rhs = call;
        ZrStmt **tb = (ZrStmt **)zr_arena_alloc(p->arena, sizeof(ZrStmt *));
        tb[0] = asgn;
        ZrStmt *ifs = zp_alloc_stmt(p, ST_IF, line);
        ifs->u.if_s.cond = cond;
        ifs->u.if_s.then_b = tb;
        ifs->u.if_s.then_n = 1;
        ifs->u.if_s.else_b = else_chain;
        ifs->u.if_s.else_n = else_n;
        else_chain = (ZrStmt **)zr_arena_alloc(p->arena, sizeof(ZrStmt *));
        else_chain[0] = ifs;
        else_n = 1;
    }

    ZrStmt *wh = zp_alloc_stmt(p, ST_WHILE, line);
    wh->u.while_s.cond = zp_mk_bin(p, TOK_BANGEQ, zp_mk_var(p, pc_var, line), zp_mk_str(p, "__done", 6, line), line);
    wh->u.while_s.body = else_chain;
    wh->u.while_s.body_n = else_n;
    blk_stmts[bs++] = wh;

    ZrStmt *blk = zp_alloc_stmt(p, ST_BLOCK, line);
    blk->u.blk.stmts = blk_stmts;
    blk->u.blk.count = bs;
    return blk;
}

static ZrTokKind zp_aug_to_bin(ZrTokKind aug) {
    switch (aug) {
        case TOK_PLUSEQ:     return TOK_PLUS;
        case TOK_MINUSEQ:    return TOK_MINUS;
        case TOK_STAREQ:     return TOK_STAR;
        case TOK_SLASHEQ:    return TOK_SLASH;
        case TOK_PERCENTEQ:  return TOK_PERCENT;
        case TOK_STARSTAREQ: return TOK_STARSTAR;
        default:             return TOK_EOF;
    }
}

static ZrStmt *zp_parse_stmt(ZrParser *p) {
    if (++p->depth > ZR_MAX_PARSE_DEPTH) {
        p->depth--;
        zp_err(p, "maximum statement nesting depth exceeded");
        return NULL;
    }
    ZrToken *t = zp_peek(p);
    uint32_t ln = t->line;
    ZrStmt *res = NULL;

    /* Inside goto label: `goto Target` and `end` */
    if (p->in_goto_label) {
        if (zp_check_ident_str(p, "goto") && zp_peek_n(p, 1)->kind != TOK_LBRACE) {
            zp_advance(p);
            const char *lbl = zp_expect_decl_name(p, "goto label");
            if (lbl) {
                res = zp_alloc_stmt(p, ST_RETURN, ln);
                res->u.expr = zp_mk_str(p, lbl, strlen(lbl), ln);
            }
            p->depth--;
            return res;
        }
        if (zp_check_ident_str(p, "end") &&
            (zp_peek_n(p, 1)->kind == TOK_SEMI || zp_peek_n(p, 1)->kind == TOK_RBRACE)) {
            zp_advance(p);
            res = zp_alloc_stmt(p, ST_RETURN, ln);
            res->u.expr = zp_mk_str(p, "__done", 6, ln);
            p->depth--;
            return res;
        }
    }

    switch (t->kind) {
        case TOK_LET:
            zp_advance(p);
            res = zp_parse_let_or_const(p, false, ln);
            break;
        case TOK_CONST:
            zp_advance(p);
            res = zp_parse_let_or_const(p, true, ln);
            break;
        case TOK_ASYNC:
        case TOK_FN: {
            /* Named function declaration if `fn <ident>(...)`; else expression statement */
            size_t fn_off = (t->kind == TOK_ASYNC) ? 1 : 0;
            if (zp_peek_n(p, fn_off)->kind == TOK_FN && zp_peek_n(p, fn_off + 1)->kind != TOK_LPAREN) {
                bool is_async = zp_match(p, TOK_ASYNC);
                zp_advance(p); /* fn */
                const char *fname = zp_expect_any_name(p, "function name");
                if (!fname) break;
                ZrParam *params = NULL; size_t pcount = 0;
                if (!zp_parse_param_list(p, &params, &pcount)) break;
                ZrStmt **body = NULL; size_t bcount = 0;
                if (!zp_parse_block_stmts(p, &body, &bcount)) break;
                res = zp_alloc_stmt(p, ST_FN, ln);
                res->u.fn_s.name = fname;
                res->u.fn_s.params = params;
                res->u.fn_s.param_count = pcount;
                res->u.fn_s.is_async = is_async;
                res->u.fn_s.body = body;
                res->u.fn_s.body_count = bcount;
                break;
            }
            ZrExpr *e = zp_parse_expr(p);
            if (e) { res = zp_alloc_stmt(p, ST_EXPR, ln); res->u.expr = e; }
            break;
        }
        case TOK_IF:
            zp_advance(p);
            res = zp_parse_if_stmt(p, ln);
            break;
        case TOK_WHILE: {
            zp_advance(p);
            bool snb = p->no_struct_brace;
            p->no_struct_brace = true;
            ZrExpr *cond = zp_parse_expr(p);
            p->no_struct_brace = snb;
            if (!cond) break;
            ZrStmt **body = NULL; size_t bn = 0;
            if (!zp_parse_block_stmts(p, &body, &bn)) break;
            res = zp_alloc_stmt(p, ST_WHILE, ln);
            res->u.while_s.cond = cond;
            res->u.while_s.body = body;
            res->u.while_s.body_n = bn;
            break;
        }
        case TOK_LOOP: {
            if (zp_peek_n(p, 1)->kind != TOK_LBRACE) goto expr_or_assign;
            zp_advance(p);
            ZrStmt **body = NULL; size_t bn = 0;
            if (!zp_parse_block_stmts(p, &body, &bn)) break;
            res = zp_alloc_stmt(p, ST_LOOP, ln);
            res->u.loop_s.body = body;
            res->u.loop_s.body_n = bn;
            break;
        }
        case TOK_FOR: {
            zp_advance(p);
            const char *v1 = zp_expect_decl_name(p, "loop variable");
            if (!v1) break;
            if (zp_match(p, TOK_COMMA)) {
                /* for k, v in map_expr { body } */
                const char *v2 = zp_expect_decl_name(p, "second loop variable");
                if (!v2 || !zp_expect(p, TOK_IN, "'in'")) break;
                bool snb = p->no_struct_brace;
                p->no_struct_brace = true;
                ZrExpr *me = zp_parse_expr(p);
                p->no_struct_brace = snb;
                if (!me) break;
                ZrStmt **body = NULL; size_t bn = 0;
                if (!zp_parse_block_stmts(p, &body, &bn)) break;
                res = zp_alloc_stmt(p, ST_FOR_KV, ln);
                res->u.for_kv.kvar = v1;
                res->u.for_kv.vvar = v2;
                res->u.for_kv.map_e = me;
                res->u.for_kv.body = body;
                res->u.for_kv.body_n = bn;
            } else if (zp_match(p, TOK_IN)) {
                /* for x in col { body } OR for i in start to end { body } */
                bool snb = p->no_struct_brace;
                p->no_struct_brace = true;
                ZrExpr *start_e = zp_parse_expr(p);
                if (!start_e) { p->no_struct_brace = snb; break; }
                if (zp_check_ident_str(p, "to")) {
                    zp_advance(p);
                    ZrExpr *end_e = zp_parse_expr(p);
                    if (!end_e) { p->no_struct_brace = snb; break; }
                    ZrExpr *end_inc = zp_mk_bin(p, TOK_PLUS, end_e, zp_mk_num(p, 1.0, ln), ln);
                    start_e = zp_mk_call2(p, "range", start_e, end_inc, ln);
                }
                p->no_struct_brace = snb;
                ZrStmt **body = NULL; size_t bn = 0;
                if (!zp_parse_block_stmts(p, &body, &bn)) break;
                res = zp_alloc_stmt(p, ST_FOR_IN, ln);
                res->u.for_in.var = v1;
                res->u.for_in.col_e = start_e;
                res->u.for_in.body = body;
                res->u.for_in.body_n = bn;
            } else {
                /* for i count_expr { body } */
                bool snb = p->no_struct_brace;
                p->no_struct_brace = true;
                ZrExpr *cnt = zp_parse_expr(p);
                p->no_struct_brace = snb;
                if (!cnt) break;
                ZrStmt **body = NULL; size_t bn = 0;
                if (!zp_parse_block_stmts(p, &body, &bn)) break;
                res = zp_alloc_stmt(p, ST_FOR_COUNT, ln);
                res->u.for_c.var = v1;
                res->u.for_c.count_e = cnt;
                res->u.for_c.body = body;
                res->u.for_c.body_n = bn;
            }
            break;
        }
        case TOK_BREAK:
            zp_advance(p);
            res = zp_alloc_stmt(p, ST_BREAK, ln);
            break;
        case TOK_CONTINUE:
            zp_advance(p);
            res = zp_alloc_stmt(p, ST_CONTINUE, ln);
            break;
        case TOK_RETURN: {
            zp_advance(p);
            res = zp_alloc_stmt(p, ST_RETURN, ln);
            if (!zp_check(p, TOK_SEMI) && !zp_check(p, TOK_RBRACE) && !zp_check(p, TOK_EOF)) {
                res->u.expr = zp_parse_expr(p);
                if (!res->u.expr) res = NULL;
            } else {
                res->u.expr = zp_alloc_expr(p, EX_NIL, ln);
            }
            break;
        }
        case TOK_PRINT: {
            ZrTokKind nk = zp_peek_n(p, 1)->kind;
            if (nk == TOK_EQ || nk == TOK_DOT || nk == TOK_LBRACK || zp_aug_to_bin(nk) != TOK_EOF) goto expr_or_assign;
            zp_advance(p);
            ZrExpr *e = zp_parse_expr(p);
            if (e) { res = zp_alloc_stmt(p, ST_PRINT, ln); res->u.expr = e; }
            break;
        }
        case TOK_PRINTF: {
            zp_advance(p);
            if (!zp_check(p, TOK_NUM)) { zp_err(p, "printf expects integer precision"); break; }
            int prec = (int)zp_advance(p)->num_val;
            ZrExpr *e = zp_parse_expr(p);
            if (e) { res = zp_alloc_stmt(p, ST_PRINTF, ln); res->u.printf_s.prec = prec; res->u.printf_s.expr = e; }
            break;
        }
        case TOK_THROW: {
            zp_advance(p);
            ZrExpr *e = zp_parse_expr(p);
            if (e) { res = zp_alloc_stmt(p, ST_THROW, ln); res->u.expr = e; }
            break;
        }
        case TOK_DEFER: {
            ZrTokKind nk = zp_peek_n(p, 1)->kind;
            if (nk == TOK_EQ || nk == TOK_DOT || nk == TOK_LBRACK || zp_aug_to_bin(nk) != TOK_EOF) goto expr_or_assign;
            zp_advance(p);
            ZrExpr *e = NULL;
            if (zp_match(p, TOK_LBRACE)) e = zp_parse_block_expr(p, zp_peek(p)->line);
            else e = zp_parse_expr(p);
            if (e) { res = zp_alloc_stmt(p, ST_DEFER, ln); res->u.expr = e; }
            break;
        }
        case TOK_EXPORT: {
            zp_advance(p);
            /* Support `export fn ...`, `export let ...`, `export const ...`, `export struct/class/enum ...`, AND `export a, b;` */
            if (zp_check(p, TOK_FN) || zp_check(p, TOK_ASYNC) || zp_check(p, TOK_LET) ||
                zp_check(p, TOK_CONST) || zp_check(p, TOK_STRUCT) || zp_check(p, TOK_CLASS) ||
                zp_check(p, TOK_ENUM) || zp_check(p, TOK_INTERFACE)) {
                ZrStmt *decl = zp_parse_stmt(p);
                if (!decl) break;
                const char *ename = NULL;
                if (decl->kind == ST_FN) ename = decl->u.fn_s.name;
                else if (decl->kind == ST_LET) ename = decl->u.let_s.name;
                else if (decl->kind == ST_SEQ && decl->u.blk.count > 0 && decl->u.blk.stmts[0]->kind == ST_LET)
                    ename = decl->u.blk.stmts[0]->u.let_s.name;
                if (ename) {
                    ZrStmt *ex = zp_alloc_stmt(p, ST_EXPORT, ln);
                    ex->u.exp_s.names = (const char **)zr_arena_alloc(p->arena, sizeof(const char *));
                    ex->u.exp_s.names[0] = ename;
                    ex->u.exp_s.count = 1;
                    ZrStmt **sq = (ZrStmt **)zr_arena_alloc(p->arena, 2 * sizeof(ZrStmt *));
                    sq[0] = decl; sq[1] = ex;
                    res = zp_alloc_stmt(p, ST_SEQ, ln);
                    res->u.blk.stmts = sq; res->u.blk.count = 2;
                } else {
                    res = decl;
                }
                break;
            }
            const char **names = NULL;
            size_t n = 0, cap = 0;
            do {
                if (zp_check(p, TOK_SEMI) || zp_check(p, TOK_RBRACE) || zp_check(p, TOK_EOF)) break;
                const char *nm = zp_expect_any_name(p, "export symbol");
                if (!nm) { p->depth--; return NULL; }
                if (n >= cap) {
                    cap = cap ? cap * 2 : 4;
                    const char **nn = (const char **)zr_arena_alloc(p->arena, cap * sizeof(const char *));
                    if (n > 0) memcpy(nn, names, n * sizeof(const char *));
                    names = nn;
                }
                names[n++] = nm;
            } while (zp_match(p, TOK_COMMA));
            res = zp_alloc_stmt(p, ST_EXPORT, ln);
            res->u.exp_s.names = names;
            res->u.exp_s.count = n;
            break;
        }
        case TOK_STRUCT:
            if (zp_peek_n(p, 1)->kind == TOK_IDENT || (zp_peek_n(p, 1)->kind >= TOK_LET && zp_peek_n(p, 1)->kind <= TOK_NIL)) {
                zp_advance(p);
                res = zp_parse_struct_stmt(p, ln);
                break;
            }
            goto expr_or_assign;
        case TOK_INTERFACE:
            if (zp_peek_n(p, 1)->kind == TOK_IDENT || (zp_peek_n(p, 1)->kind >= TOK_LET && zp_peek_n(p, 1)->kind <= TOK_NIL)) {
                zp_advance(p);
                res = zp_parse_interface_stmt(p, ln);
                break;
            }
            goto expr_or_assign;
        case TOK_ENUM:
            if (zp_peek_n(p, 1)->kind == TOK_IDENT || (zp_peek_n(p, 1)->kind >= TOK_LET && zp_peek_n(p, 1)->kind <= TOK_NIL)) {
                zp_advance(p);
                res = zp_parse_enum_stmt(p, ln);
                break;
            }
            goto expr_or_assign;
        case TOK_CLASS:
            if (zp_peek_n(p, 1)->kind == TOK_IDENT || (zp_peek_n(p, 1)->kind >= TOK_LET && zp_peek_n(p, 1)->kind <= TOK_NIL)) {
                zp_advance(p);
                res = zp_parse_class_stmt(p, ln);
                break;
            }
            goto expr_or_assign;
        case TOK_LBRACE: {
            /* Standalone block `{ ... }` unless empty `{}` map */
            if (zp_peek_n(p, 1)->kind != TOK_RBRACE && zp_peek_n(p, 2)->kind != TOK_COLON) {
                ZrStmt **bs = NULL; size_t bn = 0;
                if (!zp_parse_block_stmts(p, &bs, &bn)) break;
                res = zp_alloc_stmt(p, ST_BLOCK, ln);
                res->u.blk.stmts = bs;
                res->u.blk.count = bn;
                break;
            }
            goto expr_or_assign;
        }
        default:
        expr_or_assign:;
            /* Contextual `goto { ... }` */
            if (zp_check_ident_str(p, "goto") && zp_peek_n(p, 1)->kind == TOK_LBRACE) {
                zp_advance(p);
                res = zp_parse_goto_block(p, ln);
                break;
            }
            /* Contextual `import "path"` statement form */
            if (zp_check_ident_str(p, "import") && zp_peek_n(p, 1)->kind == TOK_STR) {
                zp_advance(p);
                ZrToken *st = zp_advance(p);
                ZrExpr *call = zp_mk_call1(p, "import", zp_mk_str(p, st->lexeme, st->lex_len, ln), ln);
                res = zp_alloc_stmt(p, ST_EXPR, ln);
                res->u.expr = call;
                break;
            }

            ZrExpr *lhs = zp_parse_expr(p);
            if (!lhs) break;

            /* Assignment or compound assignment: `=`, `+=`, `-=`, `*=`, `/=`, `%=`, `**=` */
            ZrTokKind ak = zp_peek(p)->kind;
            if (ak == TOK_EQ || zp_aug_to_bin(ak) != TOK_EOF) {
                zp_advance(p);
                ZrExpr *rhs = zp_parse_expr(p);
                if (!rhs) break;
                ZrTokKind bop = zp_aug_to_bin(ak);
                if (lhs->kind == EX_VAR) {
                    res = zp_alloc_stmt(p, ST_ASSIGN, ln);
                    res->u.assign_s.name = lhs->u.var;
                    res->u.assign_s.rhs = (bop == TOK_EOF) ? rhs : zp_mk_bin(p, bop, lhs, rhs, ln);
                } else if (lhs->kind == EX_ATTR) {
                    if (bop == TOK_EOF) {
                        res = zp_alloc_stmt(p, ST_SET_ATTR, ln);
                        res->u.sattr.obj = lhs->u.attr.obj;
                        res->u.sattr.attr = lhs->u.attr.attr;
                        res->u.sattr.rhs = rhs;
                    } else {
                        const char *ot = zp_gensym(p, "obj");
                        ZrStmt *s0 = zp_alloc_stmt(p, ST_LET, ln);
                        s0->u.let_s.name = ot;
                        s0->u.let_s.init = lhs->u.attr.obj;
                        ZrExpr *ga = zp_alloc_expr(p, EX_ATTR, ln);
                        ga->u.attr.obj = zp_mk_var(p, ot, ln);
                        ga->u.attr.attr = lhs->u.attr.attr;
                        ZrStmt *s1 = zp_alloc_stmt(p, ST_SET_ATTR, ln);
                        s1->u.sattr.obj = zp_mk_var(p, ot, ln);
                        s1->u.sattr.attr = lhs->u.attr.attr;
                        s1->u.sattr.rhs = zp_mk_bin(p, bop, ga, rhs, ln);
                        ZrStmt **sq = (ZrStmt **)zr_arena_alloc(p->arena, 2 * sizeof(ZrStmt *));
                        sq[0] = s0; sq[1] = s1;
                        res = zp_alloc_stmt(p, ST_BLOCK, ln);
                        res->u.blk.stmts = sq; res->u.blk.count = 2;
                    }
                } else if (lhs->kind == EX_INDEX) {
                    if (bop == TOK_EOF) {
                        res = zp_alloc_stmt(p, ST_SET_INDEX, ln);
                        res->u.sidx.obj = lhs->u.idx.obj;
                        res->u.sidx.idx = lhs->u.idx.idx;
                        res->u.sidx.rhs = rhs;
                    } else {
                        const char *ot = zp_gensym(p, "obj");
                        const char *it = zp_gensym(p, "idx");
                        ZrStmt *s0 = zp_alloc_stmt(p, ST_LET, ln);
                        s0->u.let_s.name = ot; s0->u.let_s.init = lhs->u.idx.obj;
                        ZrStmt *s1 = zp_alloc_stmt(p, ST_LET, ln);
                        s1->u.let_s.name = it; s1->u.let_s.init = lhs->u.idx.idx;
                        ZrExpr *gi = zp_alloc_expr(p, EX_INDEX, ln);
                        gi->u.idx.obj = zp_mk_var(p, ot, ln);
                        gi->u.idx.idx = zp_mk_var(p, it, ln);
                        ZrStmt *s2 = zp_alloc_stmt(p, ST_SET_INDEX, ln);
                        s2->u.sidx.obj = zp_mk_var(p, ot, ln);
                        s2->u.sidx.idx = zp_mk_var(p, it, ln);
                        s2->u.sidx.rhs = zp_mk_bin(p, bop, gi, rhs, ln);
                        ZrStmt **sq = (ZrStmt **)zr_arena_alloc(p->arena, 3 * sizeof(ZrStmt *));
                        sq[0] = s0; sq[1] = s1; sq[2] = s2;
                        res = zp_alloc_stmt(p, ST_BLOCK, ln);
                        res->u.blk.stmts = sq; res->u.blk.count = 3;
                    }
                } else {
                    zp_err(p, "invalid assignment target");
                }
            } else {
                res = zp_alloc_stmt(p, ST_EXPR, ln);
                res->u.expr = lhs;
            }
            break;
    }

    p->depth--;
    return res;
}

/* ---- Bytecode Compiler (AST -> PbFunc*) ---- */

typedef struct {
    const char *name;
    int         depth;
    bool        is_captured;
    bool        is_const;
} ZrLocal;

typedef struct ZrLoopCtx {
    struct ZrLoopCtx *prev;
    int               scope_depth;
    uint32_t         *break_jumps;
    size_t            break_len;
    size_t            break_cap;
    uint32_t         *cont_jumps;
    size_t            cont_len;
    size_t            cont_cap;
    int               cont_target_ip; /* >= 0 if known, else patched at continue site */
} ZrLoopCtx;

typedef struct ZrCompiler {
    struct ZrCompiler *enclosing;
    PbVM              *vm;
    PbFunc            *fn;
    bool               is_script;
    int                scope_depth;
    ZrLocal           *locals;
    uint32_t           local_count;
    uint32_t           local_cap;
    PbCaptureSpec     *upvalues;
    uint32_t           upvalue_count;
    uint32_t           upvalue_cap;
    ZrLoopCtx         *loop;
    char              *err_buf;
} ZrCompiler;

static bool zc_err(ZrCompiler *c, uint32_t line, const char *fmt, ...) {
    if (c->err_buf[0] != '\0') return false;
    char msg[420];
    va_list ap;
    va_start(ap, fmt);
    vsnprintf(msg, sizeof(msg), fmt, ap);
    va_end(ap);
    snprintf(c->err_buf, 512, "compile error at line %u: %s", line, msg);
    return false;
}

static uint32_t zc_emit(ZrCompiler *c, uint8_t op, uint32_t arg, uint32_t line) {
    PbFunc *fn = c->fn;
    if (fn->code_len >= fn->code_cap) {
        fn->code_cap = fn->code_cap ? fn->code_cap * 2 : 64;
        fn->code = (PbInstr *)zr_xrealloc(fn->code, fn->code_cap * sizeof(PbInstr));
    }
    uint32_t idx = fn->code_len++;
    memset(&fn->code[idx], 0, sizeof(PbInstr));
    fn->code[idx].op = op;
    fn->code[idx].arg = arg;
    fn->code[idx].line = line;
    return idx;
}

static inline void zc_patch_jump(ZrCompiler *c, uint32_t ins_idx, uint32_t target_ip) {
    c->fn->code[ins_idx].arg = target_ip;
}

static uint32_t zc_add_const(ZrCompiler *c, PbValue v) {
    PbFunc *fn = c->fn;
    /* Deduplicate string, number, bool, and nil constants */
    for (uint32_t i = 0; i < fn->const_len; i++) {
        PbValue cur = fn->consts[i];
        if (cur.tag == v.tag) {
            if (v.tag == VAL_NIL) return i;
            if (v.tag == VAL_BOOL && cur.as.b == v.as.b) return i;
            if (v.tag == VAL_NUM && cur.as.num == v.as.num && signbit(cur.as.num) == signbit(v.as.num)) return i;
            if (is_obj_type(v, OBJ_STR) && is_obj_type(cur, OBJ_STR)) {
                PbStr *sa = AS_STR(cur), *sb = AS_STR(v);
                if (sa->hash == sb->hash && sa->len == sb->len && memcmp(sa->chars, sb->chars, sa->len) == 0)
                    return i;
            }
        }
    }
    if (fn->const_len >= fn->const_cap) {
        fn->const_cap = fn->const_cap ? fn->const_cap * 2 : 16;
        fn->consts = (PbValue *)zr_xrealloc(fn->consts, fn->const_cap * sizeof(PbValue));
    }
    uint32_t idx = fn->const_len++;
    fn->consts[idx] = v;
    return idx;
}

static uint32_t zc_str_const(ZrCompiler *c, const char *s, size_t len) {
    PbStr *ps = pb_str_new(c->vm, s, len);
    return zc_add_const(c, val_obj(ps));
}

static inline uint32_t zc_cstr_const(ZrCompiler *c, const char *s) {
    return zc_str_const(c, s, strlen(s));
}

static uint32_t zc_add_local(ZrCompiler *c, const char *name, bool is_const) {
    if (c->local_count >= c->local_cap) {
        c->local_cap = c->local_cap ? c->local_cap * 2 : 16;
        c->locals = (ZrLocal *)zr_xrealloc(c->locals, c->local_cap * sizeof(ZrLocal));
    }
    uint32_t slot = c->local_count++;
    c->locals[slot].name = name;
    c->locals[slot].depth = c->scope_depth;
    c->locals[slot].is_captured = false;
    c->locals[slot].is_const = is_const;
    return slot;
}

static inline void zc_push_temp(ZrCompiler *c) {
    (void)zc_add_local(c, "", false);
}

static inline void zc_pop_temps(ZrCompiler *c, uint32_t n) {
    c->local_count -= n;
}

static int zc_resolve_local(ZrCompiler *c, const char *name) {
    for (int i = (int)c->local_count - 1; i >= 0; i--) {
        if (c->locals[i].name[0] != '\0' && strcmp(c->locals[i].name, name) == 0) {
            return i;
        }
    }
    return -1;
}

static uint32_t zc_add_upvalue(ZrCompiler *c, uint8_t is_local, uint32_t idx) {
    for (uint32_t i = 0; i < c->upvalue_count; i++) {
        if (c->upvalues[i].is_local == is_local && c->upvalues[i].idx == idx) return i;
    }
    if (c->upvalue_count >= c->upvalue_cap) {
        c->upvalue_cap = c->upvalue_cap ? c->upvalue_cap * 2 : 8;
        c->upvalues = (PbCaptureSpec *)zr_xrealloc(c->upvalues, c->upvalue_cap * sizeof(PbCaptureSpec));
    }
    uint32_t slot = c->upvalue_count++;
    c->upvalues[slot].is_local = is_local;
    c->upvalues[slot].idx = idx;
    return slot;
}

static int zc_resolve_upvalue(ZrCompiler *c, const char *name) {
    if (!c->enclosing) return -1;
    int loc = zc_resolve_local(c->enclosing, name);
    if (loc >= 0) {
        c->enclosing->locals[loc].is_captured = true;
        return (int)zc_add_upvalue(c, 1, (uint32_t)loc);
    }
    int up = zc_resolve_upvalue(c->enclosing, name);
    if (up >= 0) {
        return (int)zc_add_upvalue(c, 0, (uint32_t)up);
    }
    return -1;
}

static void zc_begin_scope(ZrCompiler *c) { c->scope_depth++; }

static void zc_end_scope(ZrCompiler *c, uint32_t line) {
    c->scope_depth--;
    while (c->local_count > 0 && c->locals[c->local_count - 1].depth > c->scope_depth) {
        if (c->locals[c->local_count - 1].is_captured) zc_emit(c, OP_CLOSE_UPVALUE, 0, line);
        else zc_emit(c, OP_POP, 0, line);
        c->local_count--;
    }
}

static void zc_emit_pop_to_depth(ZrCompiler *c, int target_depth, uint32_t line) {
    for (int i = (int)c->local_count - 1; i >= 0 && c->locals[i].depth > target_depth; i--) {
        if (c->locals[i].is_captured) zc_emit(c, OP_CLOSE_UPVALUE, 0, line);
        else zc_emit(c, OP_POP, 0, line);
    }
}

static bool zc_compile_expr(ZrCompiler *c, const ZrExpr *e);
static bool zc_compile_stmt(ZrCompiler *c, const ZrStmt *s);
static PbFunc *zc_compile_fn_node(ZrCompiler *enclosing,
                                  const char *name,
                                  const ZrParam *params,
                                  size_t param_count,
                                  bool is_async,
                                  ZrStmt *const *body,
                                  size_t body_count,
                                  uint32_t line,
                                  PbCaptureSpec **out_caps,
                                  uint32_t *out_ncaps);

static bool zc_eval_default_const(ZrCompiler *c, const ZrExpr *e, PbValue *out) {
    if (!e) { *out = val_nil(); return true; }
    switch (e->kind) {
        case EX_NIL:  *out = val_nil(); return true;
        case EX_BOOL: *out = val_bool(e->u.b); return true;
        case EX_NUM:  *out = val_num(e->u.num); return true;
        case EX_STR:  *out = val_obj(pb_str_new(c->vm, e->u.str.s, e->u.str.len)); return true;
        case EX_UNARY:
            if (e->u.unary.op == TOK_MINUS && e->u.unary.rhs->kind == EX_NUM) {
                *out = val_num(-e->u.unary.rhs->u.num);
                return true;
            }
            break;
        case EX_ARRAY:
            if (e->u.arr.count == 0) { *out = val_obj(pb_array_new(c->vm, 0)); return true; }
            break;
        case EX_MAP:
            if (e->u.map.count == 0) { *out = val_obj(pb_map_new(c->vm)); return true; }
            break;
        default:
            break;
    }
    *out = val_nil();
    return true;
}

static void zc_emit_get_var(ZrCompiler *c, const char *name, uint32_t line) {
    int loc = zc_resolve_local(c, name);
    if (loc >= 0) {
        zc_emit(c, OP_GET_LOCAL, (uint32_t)loc, line);
        return;
    }
    int up = zc_resolve_upvalue(c, name);
    if (up >= 0) {
        zc_emit(c, OP_GET_UPVALUE, (uint32_t)up, line);
        return;
    }
    zc_emit(c, OP_GET_GLOBAL, zc_cstr_const(c, name), line);
}

static bool zc_emit_set_var(ZrCompiler *c, const char *name, uint32_t line) {
    int loc = zc_resolve_local(c, name);
    if (loc >= 0) {
        if (c->locals[loc].is_const) return zc_err(c, line, "cannot reassign const '%s'", name);
        zc_emit(c, OP_SET_LOCAL, (uint32_t)loc, line);
        return true;
    }
    int up = zc_resolve_upvalue(c, name);
    if (up >= 0) {
        zc_emit(c, OP_SET_UPVALUE, (uint32_t)up, line);
        return true;
    }
    zc_emit(c, OP_SET_GLOBAL, zc_cstr_const(c, name), line);
    return true;
}

/* Compile pure stack-neutral pattern check against `val_slot` -> pushes 1 bool, adds 0 net locals */
static bool zc_compile_pat_check(ZrCompiler *c, const ZrPat *pat, uint32_t val_slot, uint32_t line) {
    switch (pat->kind) {
        case PAT_WILD:
        case PAT_VAR:
            zc_emit(c, OP_CONSTANT, zc_add_const(c, val_bool(true)), line);
            return true;
        case PAT_LIT:
            zc_emit(c, OP_GET_LOCAL, val_slot, line);
            zc_push_temp(c);
            if (!zc_compile_expr(c, pat->lit_expr)) return false;
            zc_pop_temps(c, 1);
            zc_emit(c, OP_EQ, 0, line);
            return true;
        case PAT_OR: {
            uint32_t *true_jumps = (uint32_t *)zr_xmalloc(pat->sub_count * sizeof(uint32_t));
            for (size_t i = 0; i < pat->sub_count; i++) {
                if (!zc_compile_pat_check(c, pat->sub_pats[i], val_slot, line)) { free(true_jumps); return false; }
                if (i + 1 < pat->sub_count) {
                    true_jumps[i] = zc_emit(c, OP_JUMP_IF_TRUE, 0, line);
                    zc_emit(c, OP_POP, 0, line);
                }
            }
            uint32_t end_ip = c->fn->code_len;
            for (size_t i = 0; i + 1 < pat->sub_count; i++) zc_patch_jump(c, true_jumps[i], end_ip);
            free(true_jumps);
            return true;
        }
        case PAT_ARRAY: {
            /* Reserve 1 reusable temp slot so every fail jump has identical stack height */
            zc_emit(c, OP_CONSTANT, zc_add_const(c, val_nil()), line);
            uint32_t tmp_slot = zc_add_local(c, "", false);
            zc_emit(c, OP_CONSTANT, zc_add_const(c, val_bool(false)), line);
            uint32_t res_slot = zc_add_local(c, "", false);

            uint32_t fail_jumps[64];
            size_t nf = 0;

            zc_emit_get_var(c, "is_array", line);
            zc_emit(c, OP_GET_LOCAL, val_slot, line);
            zc_emit(c, OP_CALL, 1, line);
            fail_jumps[nf++] = zc_emit(c, OP_JUMP_IF_FALSE, 0, line);
            zc_emit(c, OP_POP, 0, line);

            zc_emit_get_var(c, "len", line);
            zc_emit(c, OP_GET_LOCAL, val_slot, line);
            zc_emit(c, OP_CALL, 1, line);
            zc_emit(c, OP_CONSTANT, zc_add_const(c, val_num((double)pat->sub_count)), line);
            zc_emit(c, pat->rest_name ? OP_GE : OP_EQ, 0, line);
            fail_jumps[nf++] = zc_emit(c, OP_JUMP_IF_FALSE, 0, line);
            zc_emit(c, OP_POP, 0, line);

            for (size_t i = 0; i < pat->sub_count; i++) {
                zc_emit(c, OP_GET_LOCAL, val_slot, line);
                zc_emit(c, OP_CONSTANT, zc_add_const(c, val_num((double)i)), line);
                zc_emit(c, OP_GET_INDEX, 0, line);
                zc_emit(c, OP_SET_LOCAL, tmp_slot, line);
                if (!zc_compile_pat_check(c, pat->sub_pats[i], tmp_slot, line)) return false;
                if (nf < 63) fail_jumps[nf++] = zc_emit(c, OP_JUMP_IF_FALSE, 0, line);
                zc_emit(c, OP_POP, 0, line);
            }
            zc_emit(c, OP_CONSTANT, zc_add_const(c, val_bool(true)), line);
            zc_emit(c, OP_SET_LOCAL, res_slot, line);
            uint32_t done_jmp = zc_emit(c, OP_JUMP, 0, line);

            uint32_t fail_ip = c->fn->code_len;
            for (size_t i = 0; i < nf; i++) zc_patch_jump(c, fail_jumps[i], fail_ip);
            zc_emit(c, OP_POP, 0, line); /* pop false cond */

            zc_patch_jump(c, done_jmp, c->fn->code_len);
            /* Move res_slot into tmp_slot, pop res_slot, drop tmp_slot from compiler locals -> leaves bool on stack! */
            zc_emit(c, OP_GET_LOCAL, res_slot, line);
            zc_emit(c, OP_SET_LOCAL, tmp_slot, line);
            zc_emit(c, OP_POP, 0, line); /* pops res_slot */
            c->local_count -= 2;
            return true;
        }
        case PAT_MAP: {
            zc_emit(c, OP_CONSTANT, zc_add_const(c, val_nil()), line);
            uint32_t tmp_slot = zc_add_local(c, "", false);
            zc_emit(c, OP_CONSTANT, zc_add_const(c, val_bool(false)), line);
            uint32_t res_slot = zc_add_local(c, "", false);

            uint32_t fail_jumps[64];
            size_t nf = 0;

            zc_emit_get_var(c, "is_map", line);
            zc_emit(c, OP_GET_LOCAL, val_slot, line);
            zc_emit(c, OP_CALL, 1, line);
            fail_jumps[nf++] = zc_emit(c, OP_JUMP_IF_FALSE, 0, line);
            zc_emit(c, OP_POP, 0, line);

            for (size_t i = 0; i < pat->sub_count; i++) {
                zc_emit_get_var(c, "map_has", line);
                zc_emit(c, OP_GET_LOCAL, val_slot, line);
                zc_emit(c, OP_CONSTANT, zc_cstr_const(c, pat->map_keys[i]), line);
                zc_emit(c, OP_CALL, 2, line);
                if (nf < 63) fail_jumps[nf++] = zc_emit(c, OP_JUMP_IF_FALSE, 0, line);
                zc_emit(c, OP_POP, 0, line);

                zc_emit(c, OP_GET_LOCAL, val_slot, line);
                zc_emit(c, OP_GET_ATTR, zc_cstr_const(c, pat->map_keys[i]), line);
                zc_emit(c, OP_SET_LOCAL, tmp_slot, line);
                if (!zc_compile_pat_check(c, pat->sub_pats[i], tmp_slot, line)) return false;
                if (nf < 63) fail_jumps[nf++] = zc_emit(c, OP_JUMP_IF_FALSE, 0, line);
                zc_emit(c, OP_POP, 0, line);
            }
            zc_emit(c, OP_CONSTANT, zc_add_const(c, val_bool(true)), line);
            zc_emit(c, OP_SET_LOCAL, res_slot, line);
            uint32_t done_jmp = zc_emit(c, OP_JUMP, 0, line);

            uint32_t fail_ip = c->fn->code_len;
            for (size_t i = 0; i < nf; i++) zc_patch_jump(c, fail_jumps[i], fail_ip);
            zc_emit(c, OP_POP, 0, line); /* pop false cond */

            zc_patch_jump(c, done_jmp, c->fn->code_len);
            zc_emit(c, OP_GET_LOCAL, res_slot, line);
            zc_emit(c, OP_SET_LOCAL, tmp_slot, line);
            zc_emit(c, OP_POP, 0, line);
            c->local_count -= 2;
            return true;
        }
    }
    return true;
}

/* Bind pattern variables into current scope AFTER pattern check succeeded */
static bool zc_compile_pat_bind(ZrCompiler *c, const ZrPat *pat, uint32_t val_slot, uint32_t line) {
    switch (pat->kind) {
        case PAT_WILD:
        case PAT_LIT:
        case PAT_OR:
            return true;
        case PAT_VAR:
            zc_emit(c, OP_GET_LOCAL, val_slot, line);
            zc_add_local(c, pat->var_name, false);
            return true;
        case PAT_ARRAY:
            for (size_t i = 0; i < pat->sub_count; i++) {
                zc_emit(c, OP_GET_LOCAL, val_slot, line);
                zc_emit(c, OP_CONSTANT, zc_add_const(c, val_num((double)i)), line);
                zc_emit(c, OP_GET_INDEX, 0, line);
                uint32_t eslot = zc_add_local(c, "", false);
                if (!zc_compile_pat_bind(c, pat->sub_pats[i], eslot, line)) return false;
            }
            if (pat->rest_name && strcmp(pat->rest_name, "_") != 0) {
                zc_emit_get_var(c, "array_drop", line);
                zc_emit(c, OP_GET_LOCAL, val_slot, line);
                zc_emit(c, OP_CONSTANT, zc_add_const(c, val_num((double)pat->sub_count)), line);
                zc_emit(c, OP_CALL, 2, line);
                zc_add_local(c, pat->rest_name, false);
            }
            return true;
        case PAT_MAP:
            for (size_t i = 0; i < pat->sub_count; i++) {
                zc_emit(c, OP_GET_LOCAL, val_slot, line);
                zc_emit(c, OP_GET_ATTR, zc_cstr_const(c, pat->map_keys[i]), line);
                uint32_t vslot = zc_add_local(c, "", false);
                if (!zc_compile_pat_bind(c, pat->sub_pats[i], vslot, line)) return false;
            }
            return true;
    }
    return true;
}

static bool zc_compile_expr(ZrCompiler *c, const ZrExpr *e) {
    uint32_t ln = e->line;
    switch (e->kind) {
        case EX_NIL:
            zc_emit(c, OP_CONSTANT, zc_add_const(c, val_nil()), ln);
            return true;
        case EX_BOOL:
            zc_emit(c, OP_CONSTANT, zc_add_const(c, val_bool(e->u.b)), ln);
            return true;
        case EX_NUM:
            zc_emit(c, OP_CONSTANT, zc_add_const(c, val_num(e->u.num)), ln);
            return true;
        case EX_STR:
            zc_emit(c, OP_CONSTANT, zc_str_const(c, e->u.str.s, e->u.str.len), ln);
            return true;
        case EX_VAR:
            zc_emit_get_var(c, e->u.var, ln);
            return true;

        case EX_UNARY:
            if (!zc_compile_expr(c, e->u.unary.rhs)) return false;
            zc_emit(c, e->u.unary.op == TOK_MINUS ? OP_NEGATE :
                       (e->u.unary.op == TOK_THROW ? OP_THROW : OP_NOT), 0, ln);
            return true;

        case EX_UNWRAP:
            if (!zc_compile_expr(c, e->u.unary.rhs)) return false;
            zc_emit(c, OP_RESULT_UNWRAP, 0, ln);
            return true;

        case EX_BINARY: {
            if (!zc_compile_expr(c, e->u.bin.lhs)) return false;
            zc_push_temp(c);
            if (!zc_compile_expr(c, e->u.bin.rhs)) return false;
            zc_pop_temps(c, 1);
            uint8_t op = OP_ADD;
            switch (e->u.bin.op) {
                case TOK_PLUS:     op = OP_ADD;    break;
                case TOK_MINUS:    op = OP_SUB;    break;
                case TOK_STAR:     op = OP_MUL;    break;
                case TOK_SLASH:    op = OP_DIV;    break;
                case TOK_PERCENT:  op = OP_MOD;    break;
                case TOK_STARSTAR: op = OP_POW;    break;
                case TOK_EQEQ:     op = OP_EQ;     break;
                case TOK_BANGEQ:   op = OP_NOT_EQ; break;
                case TOK_LT:       op = OP_LT;     break;
                case TOK_LTEQ:     op = OP_LE;     break;
                case TOK_GT:       op = OP_GT;     break;
                case TOK_GTEQ:     op = OP_GE;     break;
                default: return zc_err(c, ln, "unsupported binary operator");
            }
            zc_emit(c, op, 0, ln);
            return true;
        }

        case EX_LOGICAL: {
            if (e->u.bin.op == TOK_AND) {
                if (!zc_compile_expr(c, e->u.bin.lhs)) return false;
                uint32_t j = zc_emit(c, OP_JUMP_IF_FALSE, 0, ln);
                zc_emit(c, OP_POP, 0, ln);
                if (!zc_compile_expr(c, e->u.bin.rhs)) return false;
                zc_patch_jump(c, j, c->fn->code_len);
                return true;
            }
            if (e->u.bin.op == TOK_OR) {
                if (!zc_compile_expr(c, e->u.bin.lhs)) return false;
                uint32_t j = zc_emit(c, OP_JUMP_IF_TRUE, 0, ln);
                zc_emit(c, OP_POP, 0, ln);
                if (!zc_compile_expr(c, e->u.bin.rhs)) return false;
                zc_patch_jump(c, j, c->fn->code_len);
                return true;
            }
            /* TOK_QQ: lhs ?? rhs */
            if (!zc_compile_expr(c, e->u.bin.lhs)) return false;
            zc_emit(c, OP_DUP, 0, ln);
            zc_emit(c, OP_IS_NIL, 0, ln);
            uint32_t j_not_nil = zc_emit(c, OP_JUMP_IF_FALSE, 0, ln);
            zc_emit(c, OP_POP, 0, ln); /* pop true */
            zc_emit(c, OP_POP, 0, ln); /* pop nil lhs */
            if (!zc_compile_expr(c, e->u.bin.rhs)) return false;
            uint32_t j_done = zc_emit(c, OP_JUMP, 0, ln);
            zc_patch_jump(c, j_not_nil, c->fn->code_len);
            zc_emit(c, OP_POP, 0, ln); /* pop false, leaving non-nil lhs */
            zc_patch_jump(c, j_done, c->fn->code_len);
            return true;
        }

        case EX_TERNARY:
        case EX_IF: {
            const ZrExpr *cond = (e->kind == EX_TERNARY) ? e->u.tern.cond : e->u.if_e.cond;
            const ZrExpr *tb   = (e->kind == EX_TERNARY) ? e->u.tern.t_br : e->u.if_e.then_e;
            const ZrExpr *fb   = (e->kind == EX_TERNARY) ? e->u.tern.f_br : e->u.if_e.else_e;
            if (!zc_compile_expr(c, cond)) return false;
            uint32_t jf = zc_emit(c, OP_JUMP_IF_FALSE, 0, ln);
            zc_emit(c, OP_POP, 0, ln);
            if (!zc_compile_expr(c, tb)) return false;
            uint32_t je = zc_emit(c, OP_JUMP, 0, ln);
            zc_patch_jump(c, jf, c->fn->code_len);
            zc_emit(c, OP_POP, 0, ln);
            if (fb) {
                if (!zc_compile_expr(c, fb)) return false;
            } else {
                zc_emit(c, OP_CONSTANT, zc_add_const(c, val_nil()), ln);
            }
            zc_patch_jump(c, je, c->fn->code_len);
            return true;
        }

        case EX_ARRAY: {
            for (size_t i = 0; i < e->u.arr.count; i++) {
                if (!zc_compile_expr(c, e->u.arr.items[i])) return false;
                zc_push_temp(c);
            }
            zc_pop_temps(c, (uint32_t)e->u.arr.count);
            zc_emit(c, OP_MAKE_ARRAY, (uint32_t)e->u.arr.count, ln);
            return true;
        }

        case EX_MAP: {
            for (size_t i = 0; i < e->u.map.count; i++) {
                if (!zc_compile_expr(c, e->u.map.keys[i])) return false;
                zc_push_temp(c);
                if (!zc_compile_expr(c, e->u.map.vals[i])) return false;
                zc_push_temp(c);
            }
            zc_pop_temps(c, (uint32_t)(e->u.map.count * 2));
            zc_emit(c, OP_MAKE_MAP, (uint32_t)e->u.map.count, ln);
            return true;
        }

        case EX_INDEX:
        case EX_SAFE_INDEX:
            if (!zc_compile_expr(c, e->u.idx.obj)) return false;
            zc_push_temp(c);
            if (!zc_compile_expr(c, e->u.idx.idx)) return false;
            zc_pop_temps(c, 1);
            zc_emit(c, e->kind == EX_SAFE_INDEX ? OP_SAFE_INDEX : OP_GET_INDEX, 0, ln);
            return true;

        case EX_ATTR:
        case EX_SAFE_ATTR:
            if (!zc_compile_expr(c, e->u.attr.obj)) return false;
            zc_emit(c, e->kind == EX_SAFE_ATTR ? OP_SAFE_GET_ATTR : OP_GET_ATTR,
                    zc_cstr_const(c, e->u.attr.attr), ln);
            return true;

        case EX_CALL: {
            if (!zc_compile_expr(c, e->u.call.callee)) return false;
            zc_push_temp(c);
            for (size_t i = 0; i < e->u.call.argc; i++) {
                if (!zc_compile_expr(c, e->u.call.args[i])) return false;
                zc_push_temp(c);
            }
            zc_pop_temps(c, (uint32_t)(e->u.call.argc + 1));
            zc_emit(c, OP_CALL, (uint32_t)e->u.call.argc, ln);
            return true;
        }

        case EX_METHOD_CALL: {
            /* Evaluate receiver once into hidden local, push receiver.method, push receiver, push args, OP_CALL(argc+1),
               save result into receiver slot, remove hidden local from compiler table (leaving result on stack!) */
            if (!zc_compile_expr(c, e->u.mcall.recv)) return false;
            uint32_t rslot = zc_add_local(c, "", false);
            zc_emit(c, OP_GET_LOCAL, rslot, ln);
            zc_emit(c, OP_GET_ATTR, zc_cstr_const(c, e->u.mcall.method), ln);
            zc_push_temp(c);
            zc_emit(c, OP_GET_LOCAL, rslot, ln);
            zc_push_temp(c);
            for (size_t i = 0; i < e->u.mcall.argc; i++) {
                if (!zc_compile_expr(c, e->u.mcall.args[i])) return false;
                zc_push_temp(c);
            }
            zc_pop_temps(c, (uint32_t)(e->u.mcall.argc + 2));
            zc_emit(c, OP_CALL, (uint32_t)(e->u.mcall.argc + 1), ln);
            zc_emit(c, OP_SET_LOCAL, rslot, ln);
            c->local_count--; /* rslot on stack now holds return value! */
            return true;
        }

        case EX_FN: {
            PbCaptureSpec *caps = NULL;
            uint32_t ncaps = 0;
            PbFunc *sub = zc_compile_fn_node(c, e->u.fn.name, e->u.fn.params, e->u.fn.param_count,
                                             e->u.fn.is_async, e->u.fn.body, e->u.fn.body_count,
                                             ln, &caps, &ncaps);
            if (!sub) return false;
            uint32_t fidx = zc_add_const(c, val_obj(sub));
            uint32_t ins_i = zc_emit(c, OP_CLOSURE, fidx, ln);
            c->fn->code[ins_i].n_caps = ncaps;
            c->fn->code[ins_i].caps = caps;
            return true;
        }

        case EX_TRY: {
            /* Reserve hidden result slot so catch error binding + upvalues unwind cleanly */
            zc_emit(c, OP_CONSTANT, zc_add_const(c, val_nil()), ln);
            uint32_t res_slot = zc_add_local(c, "", false);

            uint32_t try_ins = zc_emit(c, OP_TRY, 0, ln);
            if (!zc_compile_expr(c, e->u.try_e.try_e)) return false;
            zc_emit(c, OP_SET_LOCAL, res_slot, ln);
            zc_emit(c, OP_END_TRY, 0, ln);
            uint32_t ok_jmp = zc_emit(c, OP_JUMP, 0, ln);

            /* Catch target: VM unwinds sp to right above res_slot and pushes err string */
            zc_patch_jump(c, try_ins, c->fn->code_len);
            if (e->u.try_e.err_var) {
                zc_begin_scope(c);
                zc_add_local(c, e->u.try_e.err_var, false); /* occupies pushed err string slot! */
                if (!zc_compile_expr(c, e->u.try_e.catch_e)) return false;
                zc_emit(c, OP_SET_LOCAL, res_slot, ln);
                zc_end_scope(c, ln);
            } else {
                zc_emit(c, OP_POP, 0, ln); /* discard pushed err string */
                if (!zc_compile_expr(c, e->u.try_e.catch_e)) return false;
                zc_emit(c, OP_SET_LOCAL, res_slot, ln);
            }

            zc_patch_jump(c, ok_jmp, c->fn->code_len);
            c->local_count--; /* res_slot stays on top of stack as expression value! */
            return true;
        }

        case EX_BLOCK: {
            zc_emit(c, OP_CONSTANT, zc_add_const(c, val_nil()), ln);
            uint32_t res_slot = zc_add_local(c, "", false);
            zc_begin_scope(c);
            for (size_t i = 0; i < e->u.blk.stmt_count; i++) {
                if (!zc_compile_stmt(c, e->u.blk.stmts[i])) return false;
            }
            if (e->u.blk.tail) {
                if (!zc_compile_expr(c, e->u.blk.tail)) return false;
                zc_emit(c, OP_SET_LOCAL, res_slot, ln);
            }
            zc_end_scope(c, ln);
            c->local_count--; /* res_slot remains on stack as block value */
            return true;
        }

        case EX_MATCH: {
            /* Reserve res_slot, evaluate scrutinee into scrut_slot */
            zc_emit(c, OP_CONSTANT, zc_add_const(c, val_nil()), ln);
            uint32_t res_slot = zc_add_local(c, "", false);

            zc_begin_scope(c);
            if (!zc_compile_expr(c, e->u.match_e.scrut)) return false;
            uint32_t scrut_slot = zc_add_local(c, "", false);

            uint32_t *done_jumps = (uint32_t *)zr_xmalloc((e->u.match_e.arm_count + 1) * sizeof(uint32_t));
            size_t ndone = 0;

            for (size_t i = 0; i < e->u.match_e.arm_count; i++) {
                const ZrMatchArm *arm = &e->u.match_e.arms[i];
                if (!zc_compile_pat_check(c, arm->pat, scrut_slot, ln)) { free(done_jumps); return false; }
                uint32_t j_pat_fail = zc_emit(c, OP_JUMP_IF_FALSE, 0, ln);
                zc_emit(c, OP_POP, 0, ln);

                zc_begin_scope(c);
                uint32_t arm_base_locals = c->local_count;
                if (!zc_compile_pat_bind(c, arm->pat, scrut_slot, ln)) { free(done_jumps); return false; }
                uint32_t bound_count = c->local_count - arm_base_locals;

                uint32_t j_guard_fail = UINT32_MAX;
                if (arm->guard) {
                    if (!zc_compile_expr(c, arm->guard)) { free(done_jumps); return false; }
                    j_guard_fail = zc_emit(c, OP_JUMP_IF_FALSE, 0, ln);
                    zc_emit(c, OP_POP, 0, ln);
                }

                if (!zc_compile_expr(c, arm->body)) { free(done_jumps); return false; }
                zc_emit(c, OP_SET_LOCAL, res_slot, ln);
                zc_end_scope(c, ln);
                done_jumps[ndone++] = zc_emit(c, OP_JUMP, 0, ln);

                if (j_guard_fail != UINT32_MAX) {
                    zc_patch_jump(c, j_guard_fail, c->fn->code_len);
                    zc_emit(c, OP_POP, 0, ln); /* pop false guard */
                    for (uint32_t b = 0; b < bound_count; b++) {
                        zc_emit(c, OP_POP, 0, ln);
                    }
                    uint32_t skip_pat_pop = zc_emit(c, OP_JUMP, 0, ln);
                    zc_patch_jump(c, j_pat_fail, c->fn->code_len);
                    zc_emit(c, OP_POP, 0, ln); /* pop false pat */
                    zc_patch_jump(c, skip_pat_pop, c->fn->code_len);
                } else {
                    zc_patch_jump(c, j_pat_fail, c->fn->code_len);
                    zc_emit(c, OP_POP, 0, ln); /* pop false pat */
                }
            }

            uint32_t match_end = c->fn->code_len;
            for (size_t i = 0; i < ndone; i++) zc_patch_jump(c, done_jumps[i], match_end);
            free(done_jumps);

            zc_end_scope(c, ln); /* pops scrut_slot */
            c->local_count--;    /* leaves res_slot on stack */
            return true;
        }
    }
    return true;
}

static bool zc_compile_stmt_list(ZrCompiler *c, ZrStmt *const *stmts, size_t n) {
    for (size_t i = 0; i < n; i++) {
        if (!zc_compile_stmt(c, stmts[i])) return false;
    }
    return true;
}

static void zc_loop_push_jump(uint32_t **arr, size_t *len, size_t *cap, uint32_t jmp) {
    if (*len >= *cap) {
        *cap = *cap ? *cap * 2 : 8;
        *arr = (uint32_t *)zr_xrealloc(*arr, *cap * sizeof(uint32_t));
    }
    (*arr)[(*len)++] = jmp;
}

static bool zc_compile_stmt(ZrCompiler *c, const ZrStmt *s) {
    uint32_t ln = s->line;
    switch (s->kind) {
        case ST_EXPR:
            if (!zc_compile_expr(c, s->u.expr)) return false;
            zc_emit(c, OP_POP, 0, ln);
            return true;

        case ST_LET: {
            if (!zc_compile_expr(c, s->u.let_s.init)) return false;
            if (c->scope_depth == 0 && c->is_script) {
                zc_emit(c, OP_DEFINE_GLOBAL, zc_cstr_const(c, s->u.let_s.name), ln);
            } else {
                zc_add_local(c, s->u.let_s.name, s->u.let_s.is_const);
            }
            return true;
        }

        case ST_ASSIGN:
            if (!zc_compile_expr(c, s->u.assign_s.rhs)) return false;
            return zc_emit_set_var(c, s->u.assign_s.name, ln);

        case ST_SET_ATTR:
            if (!zc_compile_expr(c, s->u.sattr.obj)) return false;
            zc_push_temp(c);
            if (!zc_compile_expr(c, s->u.sattr.rhs)) return false;
            zc_pop_temps(c, 1);
            zc_emit(c, OP_SET_ATTR, zc_cstr_const(c, s->u.sattr.attr), ln);
            zc_emit(c, OP_POP, 0, ln);
            return true;

        case ST_SET_INDEX:
            if (!zc_compile_expr(c, s->u.sidx.obj)) return false;
            zc_push_temp(c);
            if (!zc_compile_expr(c, s->u.sidx.idx)) return false;
            zc_push_temp(c);
            if (!zc_compile_expr(c, s->u.sidx.rhs)) return false;
            zc_pop_temps(c, 2);
            zc_emit(c, OP_SET_INDEX, 0, ln);
            zc_emit(c, OP_POP, 0, ln);
            return true;

        case ST_SEQ:
            return zc_compile_stmt_list(c, s->u.blk.stmts, s->u.blk.count);

        case ST_BLOCK:
            zc_begin_scope(c);
            if (!zc_compile_stmt_list(c, s->u.blk.stmts, s->u.blk.count)) return false;
            zc_end_scope(c, ln);
            return true;

        case ST_IF: {
            if (!zc_compile_expr(c, s->u.if_s.cond)) return false;
            uint32_t jf = zc_emit(c, OP_JUMP_IF_FALSE, 0, ln);
            zc_emit(c, OP_POP, 0, ln);
            zc_begin_scope(c);
            if (!zc_compile_stmt_list(c, s->u.if_s.then_b, s->u.if_s.then_n)) return false;
            zc_end_scope(c, ln);

            if (s->u.if_s.else_n > 0) {
                uint32_t je = zc_emit(c, OP_JUMP, 0, ln);
                zc_patch_jump(c, jf, c->fn->code_len);
                zc_emit(c, OP_POP, 0, ln);
                zc_begin_scope(c);
                if (!zc_compile_stmt_list(c, s->u.if_s.else_b, s->u.if_s.else_n)) return false;
                zc_end_scope(c, ln);
                zc_patch_jump(c, je, c->fn->code_len);
            } else {
                uint32_t je = zc_emit(c, OP_JUMP, 0, ln);
                zc_patch_jump(c, jf, c->fn->code_len);
                zc_emit(c, OP_POP, 0, ln);
                zc_patch_jump(c, je, c->fn->code_len);
            }
            return true;
        }

        case ST_WHILE: {
            ZrLoopCtx lctx;
            memset(&lctx, 0, sizeof(lctx));
            lctx.prev = c->loop;
            lctx.scope_depth = c->scope_depth;
            uint32_t loop_start = c->fn->code_len;
            lctx.cont_target_ip = (int)loop_start;
            c->loop = &lctx;

            if (!zc_compile_expr(c, s->u.while_s.cond)) { c->loop = lctx.prev; return false; }
            uint32_t exit_j = zc_emit(c, OP_JUMP_IF_FALSE, 0, ln);
            zc_emit(c, OP_POP, 0, ln);

            zc_begin_scope(c);
            if (!zc_compile_stmt_list(c, s->u.while_s.body, s->u.while_s.body_n)) {
                c->loop = lctx.prev; free(lctx.break_jumps); free(lctx.cont_jumps); return false;
            }
            zc_end_scope(c, ln);
            zc_emit(c, OP_JUMP, loop_start, ln);

            uint32_t after_cond_fail = c->fn->code_len;
            zc_patch_jump(c, exit_j, after_cond_fail);
            zc_emit(c, OP_POP, 0, ln);

            uint32_t loop_end = c->fn->code_len;
            for (size_t i = 0; i < lctx.break_len; i++) zc_patch_jump(c, lctx.break_jumps[i], loop_end);
            for (size_t i = 0; i < lctx.cont_len; i++) zc_patch_jump(c, lctx.cont_jumps[i], loop_start);
            free(lctx.break_jumps);
            free(lctx.cont_jumps);
            c->loop = lctx.prev;
            return true;
        }

        case ST_LOOP: {
            ZrLoopCtx lctx;
            memset(&lctx, 0, sizeof(lctx));
            lctx.prev = c->loop;
            lctx.scope_depth = c->scope_depth;
            uint32_t loop_start = c->fn->code_len;
            lctx.cont_target_ip = (int)loop_start;
            c->loop = &lctx;

            zc_begin_scope(c);
            if (!zc_compile_stmt_list(c, s->u.loop_s.body, s->u.loop_s.body_n)) {
                c->loop = lctx.prev; free(lctx.break_jumps); free(lctx.cont_jumps); return false;
            }
            zc_end_scope(c, ln);
            zc_emit(c, OP_JUMP, loop_start, ln);

            uint32_t loop_end = c->fn->code_len;
            for (size_t i = 0; i < lctx.break_len; i++) zc_patch_jump(c, lctx.break_jumps[i], loop_end);
            for (size_t i = 0; i < lctx.cont_len; i++) zc_patch_jump(c, lctx.cont_jumps[i], loop_start);
            free(lctx.break_jumps);
            free(lctx.cont_jumps);
            c->loop = lctx.prev;
            return true;
        }

        case ST_FOR_COUNT: {
            zc_begin_scope(c);
            if (!zc_compile_expr(c, s->u.for_c.count_e)) return false;
            uint32_t lim_slot = zc_add_local(c, "", true);
            zc_emit(c, OP_CONSTANT, zc_add_const(c, val_num(0.0)), ln);
            uint32_t idx_slot = zc_add_local(c, "", false);

            ZrLoopCtx lctx;
            memset(&lctx, 0, sizeof(lctx));
            lctx.prev = c->loop;
            lctx.scope_depth = c->scope_depth;
            lctx.cont_target_ip = -1;
            c->loop = &lctx;

            uint32_t loop_start = c->fn->code_len;
            zc_emit(c, OP_GET_LOCAL, idx_slot, ln);
            zc_emit(c, OP_GET_LOCAL, lim_slot, ln);
            zc_emit(c, OP_LT, 0, ln);
            uint32_t exit_j = zc_emit(c, OP_JUMP_IF_FALSE, 0, ln);
            zc_emit(c, OP_POP, 0, ln);

            zc_begin_scope(c);
            zc_emit(c, OP_GET_LOCAL, idx_slot, ln);
            zc_add_local(c, s->u.for_c.var, false);
            if (!zc_compile_stmt_list(c, s->u.for_c.body, s->u.for_c.body_n)) {
                c->loop = lctx.prev; free(lctx.break_jumps); free(lctx.cont_jumps); return false;
            }
            zc_end_scope(c, ln);

            uint32_t step_ip = c->fn->code_len;
            zc_emit(c, OP_GET_LOCAL, idx_slot, ln);
            zc_emit(c, OP_CONSTANT, zc_add_const(c, val_num(1.0)), ln);
            zc_emit(c, OP_ADD, 0, ln);
            zc_emit(c, OP_SET_LOCAL, idx_slot, ln);
            zc_emit(c, OP_JUMP, loop_start, ln);

            zc_patch_jump(c, exit_j, c->fn->code_len);
            zc_emit(c, OP_POP, 0, ln);

            uint32_t loop_end = c->fn->code_len;
            for (size_t i = 0; i < lctx.break_len; i++) zc_patch_jump(c, lctx.break_jumps[i], loop_end);
            for (size_t i = 0; i < lctx.cont_len; i++) zc_patch_jump(c, lctx.cont_jumps[i], step_ip);
            free(lctx.break_jumps);
            free(lctx.cont_jumps);
            c->loop = lctx.prev;

            zc_end_scope(c, ln);
            return true;
        }

        case ST_FOR_IN: {
            zc_begin_scope(c);
            if (!zc_compile_expr(c, s->u.for_in.col_e)) return false;
            uint32_t col_slot = zc_add_local(c, "", true);
            zc_emit_get_var(c, "len", ln);
            zc_emit(c, OP_GET_LOCAL, col_slot, ln);
            zc_emit(c, OP_CALL, 1, ln);
            uint32_t len_slot = zc_add_local(c, "", true);
            zc_emit(c, OP_CONSTANT, zc_add_const(c, val_num(0.0)), ln);
            uint32_t idx_slot = zc_add_local(c, "", false);

            ZrLoopCtx lctx;
            memset(&lctx, 0, sizeof(lctx));
            lctx.prev = c->loop;
            lctx.scope_depth = c->scope_depth;
            lctx.cont_target_ip = -1;
            c->loop = &lctx;

            uint32_t loop_start = c->fn->code_len;
            zc_emit(c, OP_GET_LOCAL, idx_slot, ln);
            zc_emit(c, OP_GET_LOCAL, len_slot, ln);
            zc_emit(c, OP_LT, 0, ln);
            uint32_t exit_j = zc_emit(c, OP_JUMP_IF_FALSE, 0, ln);
            zc_emit(c, OP_POP, 0, ln);

            zc_begin_scope(c);
            zc_emit(c, OP_GET_LOCAL, col_slot, ln);
            zc_emit(c, OP_GET_LOCAL, idx_slot, ln);
            zc_emit(c, OP_GET_INDEX, 0, ln);
            zc_add_local(c, s->u.for_in.var, false);
            if (!zc_compile_stmt_list(c, s->u.for_in.body, s->u.for_in.body_n)) {
                c->loop = lctx.prev; free(lctx.break_jumps); free(lctx.cont_jumps); return false;
            }
            zc_end_scope(c, ln);

            uint32_t step_ip = c->fn->code_len;
            zc_emit(c, OP_GET_LOCAL, idx_slot, ln);
            zc_emit(c, OP_CONSTANT, zc_add_const(c, val_num(1.0)), ln);
            zc_emit(c, OP_ADD, 0, ln);
            zc_emit(c, OP_SET_LOCAL, idx_slot, ln);
            zc_emit(c, OP_JUMP, loop_start, ln);

            zc_patch_jump(c, exit_j, c->fn->code_len);
            zc_emit(c, OP_POP, 0, ln);

            uint32_t loop_end = c->fn->code_len;
            for (size_t i = 0; i < lctx.break_len; i++) zc_patch_jump(c, lctx.break_jumps[i], loop_end);
            for (size_t i = 0; i < lctx.cont_len; i++) zc_patch_jump(c, lctx.cont_jumps[i], step_ip);
            free(lctx.break_jumps);
            free(lctx.cont_jumps);
            c->loop = lctx.prev;

            zc_end_scope(c, ln);
            return true;
        }

        case ST_FOR_KV: {
            zc_begin_scope(c);
            if (!zc_compile_expr(c, s->u.for_kv.map_e)) return false;
            uint32_t map_slot = zc_add_local(c, "", true);
            zc_emit_get_var(c, "map_keys", ln);
            zc_emit(c, OP_GET_LOCAL, map_slot, ln);
            zc_emit(c, OP_CALL, 1, ln);
            uint32_t keys_slot = zc_add_local(c, "", true);
            zc_emit_get_var(c, "len", ln);
            zc_emit(c, OP_GET_LOCAL, keys_slot, ln);
            zc_emit(c, OP_CALL, 1, ln);
            uint32_t len_slot = zc_add_local(c, "", true);
            zc_emit(c, OP_CONSTANT, zc_add_const(c, val_num(0.0)), ln);
            uint32_t idx_slot = zc_add_local(c, "", false);

            ZrLoopCtx lctx;
            memset(&lctx, 0, sizeof(lctx));
            lctx.prev = c->loop;
            lctx.scope_depth = c->scope_depth;
            lctx.cont_target_ip = -1;
            c->loop = &lctx;

            uint32_t loop_start = c->fn->code_len;
            zc_emit(c, OP_GET_LOCAL, idx_slot, ln);
            zc_emit(c, OP_GET_LOCAL, len_slot, ln);
            zc_emit(c, OP_LT, 0, ln);
            uint32_t exit_j = zc_emit(c, OP_JUMP_IF_FALSE, 0, ln);
            zc_emit(c, OP_POP, 0, ln);

            zc_begin_scope(c);
            zc_emit(c, OP_GET_LOCAL, keys_slot, ln);
            zc_emit(c, OP_GET_LOCAL, idx_slot, ln);
            zc_emit(c, OP_GET_INDEX, 0, ln);
            uint32_t k_slot = zc_add_local(c, s->u.for_kv.kvar, false);
            zc_emit(c, OP_GET_LOCAL, map_slot, ln);
            zc_emit(c, OP_GET_LOCAL, k_slot, ln);
            zc_emit(c, OP_GET_INDEX, 0, ln);
            zc_add_local(c, s->u.for_kv.vvar, false);

            if (!zc_compile_stmt_list(c, s->u.for_kv.body, s->u.for_kv.body_n)) {
                c->loop = lctx.prev; free(lctx.break_jumps); free(lctx.cont_jumps); return false;
            }
            zc_end_scope(c, ln);

            uint32_t step_ip = c->fn->code_len;
            zc_emit(c, OP_GET_LOCAL, idx_slot, ln);
            zc_emit(c, OP_CONSTANT, zc_add_const(c, val_num(1.0)), ln);
            zc_emit(c, OP_ADD, 0, ln);
            zc_emit(c, OP_SET_LOCAL, idx_slot, ln);
            zc_emit(c, OP_JUMP, loop_start, ln);

            zc_patch_jump(c, exit_j, c->fn->code_len);
            zc_emit(c, OP_POP, 0, ln);

            uint32_t loop_end = c->fn->code_len;
            for (size_t i = 0; i < lctx.break_len; i++) zc_patch_jump(c, lctx.break_jumps[i], loop_end);
            for (size_t i = 0; i < lctx.cont_len; i++) zc_patch_jump(c, lctx.cont_jumps[i], step_ip);
            free(lctx.break_jumps);
            free(lctx.cont_jumps);
            c->loop = lctx.prev;

            zc_end_scope(c, ln);
            return true;
        }

        case ST_BREAK: {
            if (!c->loop) return zc_err(c, ln, "'break' outside of loop");
            zc_emit_pop_to_depth(c, c->loop->scope_depth, ln);
            uint32_t j = zc_emit(c, OP_JUMP, 0, ln);
            zc_loop_push_jump(&c->loop->break_jumps, &c->loop->break_len, &c->loop->break_cap, j);
            return true;
        }

        case ST_CONTINUE: {
            if (!c->loop) return zc_err(c, ln, "'continue' outside of loop");
            zc_emit_pop_to_depth(c, c->loop->scope_depth, ln);
            uint32_t j = zc_emit(c, OP_JUMP, 0, ln);
            zc_loop_push_jump(&c->loop->cont_jumps, &c->loop->cont_len, &c->loop->cont_cap, j);
            return true;
        }

        case ST_RETURN: {
            /* Tail-call optimization if returning a direct call `return f(args);` */
            if (s->u.expr && s->u.expr->kind == EX_CALL) {
                const ZrExpr *ce = s->u.expr;
                if (!zc_compile_expr(c, ce->u.call.callee)) return false;
                zc_push_temp(c);
                for (size_t i = 0; i < ce->u.call.argc; i++) {
                    if (!zc_compile_expr(c, ce->u.call.args[i])) return false;
                    zc_push_temp(c);
                }
                zc_pop_temps(c, (uint32_t)(ce->u.call.argc + 1));
                zc_emit(c, OP_TAIL_CALL, (uint32_t)ce->u.call.argc, ln);
                zc_emit(c, OP_RETURN, 0, ln);
                return true;
            }
            if (!zc_compile_expr(c, s->u.expr)) return false;
            zc_emit(c, OP_RETURN, 0, ln);
            return true;
        }

        case ST_PRINT:
            if (!zc_compile_expr(c, s->u.expr)) return false;
            zc_emit(c, OP_PRINT, 0, ln);
            return true;

        case ST_PRINTF: {
            char fmt[32];
            snprintf(fmt, sizeof(fmt), "{:.%d}", s->u.printf_s.prec >= 0 ? s->u.printf_s.prec : 0);
            zc_emit_get_var(c, "str_format", ln);
            zc_push_temp(c);
            zc_emit(c, OP_CONSTANT, zc_cstr_const(c, fmt), ln);
            zc_push_temp(c);
            if (!zc_compile_expr(c, s->u.printf_s.expr)) return false;
            zc_pop_temps(c, 2);
            zc_emit(c, OP_CALL, 2, ln);
            zc_emit(c, OP_PRINT, 0, ln);
            return true;
        }

        case ST_THROW:
            if (!zc_compile_expr(c, s->u.expr)) return false;
            zc_emit(c, OP_THROW, 0, ln);
            return true;

        case ST_DEFER: {
            /* Wrap deferred expression into a 0-arg thunk `fn() { expr; return nil; }` */
            ZrStmt st_e;
            memset(&st_e, 0, sizeof(st_e));
            st_e.kind = ST_EXPR;
            st_e.line = ln;
            st_e.u.expr = s->u.expr;
            ZrStmt *body_ptr = &st_e;

            PbCaptureSpec *caps = NULL;
            uint32_t ncaps = 0;
            PbFunc *thunk = zc_compile_fn_node(c, "<defer>", NULL, 0, false, &body_ptr, 1, ln, &caps, &ncaps);
            if (!thunk) return false;
            uint32_t fidx = zc_add_const(c, val_obj(thunk));
            uint32_t ins_i = zc_emit(c, OP_CLOSURE, fidx, ln);
            c->fn->code[ins_i].n_caps = ncaps;
            c->fn->code[ins_i].caps = caps;
            zc_emit(c, OP_DEFER, 0, ln);
            return true;
        }

        case ST_EXPORT: {
            for (size_t i = 0; i < s->u.exp_s.count; i++) {
                zc_emit(c, OP_EXPORT, zc_cstr_const(c, s->u.exp_s.names[i]), ln);
            }
            return true;
        }

        case ST_FN: {
            /* For local recursive functions, declare the local slot BEFORE compiling the body! */
            bool is_top = (c->scope_depth == 0 && c->is_script);
            if (!is_top) {
                zc_emit(c, OP_CONSTANT, zc_add_const(c, val_nil()), ln);
                uint32_t fslot = zc_add_local(c, s->u.fn_s.name, false);
                PbCaptureSpec *caps = NULL; uint32_t ncaps = 0;
                PbFunc *sub = zc_compile_fn_node(c, s->u.fn_s.name, s->u.fn_s.params, s->u.fn_s.param_count,
                                                 s->u.fn_s.is_async, s->u.fn_s.body, s->u.fn_s.body_count,
                                                 ln, &caps, &ncaps);
                if (!sub) return false;
                uint32_t fidx = zc_add_const(c, val_obj(sub));
                uint32_t ins_i = zc_emit(c, OP_CLOSURE, fidx, ln);
                c->fn->code[ins_i].n_caps = ncaps;
                c->fn->code[ins_i].caps = caps;
                zc_emit(c, OP_SET_LOCAL, fslot, ln);
            } else {
                PbCaptureSpec *caps = NULL; uint32_t ncaps = 0;
                PbFunc *sub = zc_compile_fn_node(c, s->u.fn_s.name, s->u.fn_s.params, s->u.fn_s.param_count,
                                                 s->u.fn_s.is_async, s->u.fn_s.body, s->u.fn_s.body_count,
                                                 ln, &caps, &ncaps);
                if (!sub) return false;
                uint32_t fidx = zc_add_const(c, val_obj(sub));
                uint32_t ins_i = zc_emit(c, OP_CLOSURE, fidx, ln);
                c->fn->code[ins_i].n_caps = ncaps;
                c->fn->code[ins_i].caps = caps;
                zc_emit(c, OP_DEFINE_GLOBAL, zc_cstr_const(c, s->u.fn_s.name), ln);
            }
            return true;
        }
    }
    return true;
}

static PbFunc *zc_compile_fn_node(ZrCompiler *enclosing,
                                  const char *name,
                                  const ZrParam *params,
                                  size_t param_count,
                                  bool is_async,
                                  ZrStmt *const *body,
                                  size_t body_count,
                                  uint32_t line,
                                  PbCaptureSpec **out_caps,
                                  uint32_t *out_ncaps) {
    PbVM *vm = enclosing->vm;
    PbFunc *fn = pb_func_new(vm);
    if (name) fn->name = pb_str_c(vm, name);
    fn->source_name = enclosing->fn->source_name;
    fn->is_async = is_async;

    ZrCompiler sub;
    memset(&sub, 0, sizeof(sub));
    sub.enclosing = enclosing;
    sub.vm = vm;
    sub.fn = fn;
    sub.is_script = false;
    sub.scope_depth = 1;
    sub.err_buf = enclosing->err_buf;

    bool has_vararg = (param_count > 0 && params[param_count - 1].is_vararg);
    size_t fixed_count = has_vararg ? (param_count - 1) : param_count;
    fn->is_vararg = has_vararg;
    fn->arity = (uint32_t)fixed_count;

    uint32_t min_ar = 0;
    bool seen_def = false;
    for (size_t i = 0; i < fixed_count; i++) {
        if (params[i].def_expr != NULL) seen_def = true;
        else if (!seen_def) min_ar++;
    }
    fn->min_arity = min_ar;
    fn->n_defaults = (uint32_t)(fixed_count - min_ar);
    if (fn->n_defaults > 0) {
        fn->defaults = (PbValue *)zr_xcalloc(fn->n_defaults, sizeof(PbValue));
        for (uint32_t i = 0; i < fn->n_defaults; i++) {
            zc_eval_default_const(&sub, params[min_ar + i].def_expr, &fn->defaults[i]);
        }
    }

    for (size_t i = 0; i < param_count; i++) {
        zc_add_local(&sub, params[i].name, false);
    }

    /* Emit runtime initialization for non-literal default parameter expressions! */
    for (size_t i = min_ar; i < fixed_count; i++) {
        const ZrExpr *de = params[i].def_expr;
        if (de && de->kind != EX_NIL && de->kind != EX_BOOL && de->kind != EX_NUM && de->kind != EX_STR) {
            zc_emit(&sub, OP_GET_LOCAL, (uint32_t)i, line);
            zc_emit(&sub, OP_IS_NIL, 0, line);
            uint32_t j_skip = zc_emit(&sub, OP_JUMP_IF_FALSE, 0, line);
            zc_emit(&sub, OP_POP, 0, line);
            if (!zc_compile_expr(&sub, de)) {
                free(sub.locals); free(sub.upvalues);
                return NULL;
            }
            zc_emit(&sub, OP_SET_LOCAL, (uint32_t)i, line);
            uint32_t j_done = zc_emit(&sub, OP_JUMP, 0, line);
            zc_patch_jump(&sub, j_skip, fn->code_len);
            zc_emit(&sub, OP_POP, 0, line);
            zc_patch_jump(&sub, j_done, fn->code_len);
        }
    }

    if (!zc_compile_stmt_list(&sub, body, body_count)) {
        free(sub.locals);
        free(sub.upvalues);
        return NULL;
    }

    zc_emit(&sub, OP_CONSTANT, zc_add_const(&sub, val_nil()), line);
    zc_emit(&sub, OP_RETURN, 0, line);

    free(sub.locals);
    *out_caps = sub.upvalues;
    *out_ncaps = sub.upvalue_count;
    return fn;
}

/* Jump Threading Peephole Pass: collapse jump -> OP_JUMP chains */
static void zr_peephole_jump_thread(PbFunc *fn) {
    for (uint32_t i = 0; i < fn->code_len; i++) {
        uint8_t op = fn->code[i].op;
        if (op == OP_JUMP || op == OP_JUMP_IF_FALSE || op == OP_JUMP_IF_TRUE) {
            uint32_t tgt = fn->code[i].arg;
            int hops = 0;
            while (tgt < fn->code_len && fn->code[tgt].op == OP_JUMP && hops < 16) {
                uint32_t next_tgt = fn->code[tgt].arg;
                if (next_tgt == tgt) break;
                tgt = next_tgt;
                hops++;
            }
            fn->code[i].arg = tgt;
        }
        if (op == OP_CONSTANT && fn->code[i].arg < fn->const_len &&
            is_obj_type(fn->consts[fn->code[i].arg], OBJ_FUNC)) {
            zr_peephole_jump_thread(AS_FUNC(fn->consts[fn->code[i].arg]));
        }
    }
}

/* Discover top-level `fn test_*()` declarations & check if top-level already invokes `test_*()` */
static bool zr_expr_calls_test_fn(const ZrExpr *e) {
    if (!e) return false;
    if (e->kind == EX_CALL && e->u.call.callee->kind == EX_VAR &&
        strncmp(e->u.call.callee->u.var, "test_", 5) == 0) {
        return true;
    }
    return false;
}

static void zr_scan_top_tests(PbFunc *root_fn, ZrStmt *const *stmts, size_t count) {
    for (size_t i = 0; i < count; i++) {
        const ZrStmt *s = stmts[i];
        if (s->kind == ST_FN && strncmp(s->u.fn_s.name, "test_", 5) == 0 && s->u.fn_s.param_count == 0) {
            root_fn->top_test_fns = (char **)zr_xrealloc(root_fn->top_test_fns,
                                                         (root_fn->top_test_fn_count + 1) * sizeof(char *));
            root_fn->top_test_fns[root_fn->top_test_fn_count++] = zr_xstrdup(s->u.fn_s.name);
        } else if (s->kind == ST_SEQ) {
            zr_scan_top_tests(root_fn, s->u.blk.stmts, s->u.blk.count);
        } else if (s->kind == ST_EXPR && zr_expr_calls_test_fn(s->u.expr)) {
            root_fn->has_top_test_calls = true;
        } else if (s->kind == ST_LET && zr_expr_calls_test_fn(s->u.let_s.init)) {
            root_fn->has_top_test_calls = true;
        }
    }
}

/* Main Entry: Compile .sapp source text -> PbFunc* */
static PbFunc *zr_compile_sapp(PbVM *vm,
                               const char *source,
                               size_t source_len,
                               const char *source_name,
                               char err_out[512]) {
    err_out[0] = '\0';

    char *clean = NULL;
    PbPolyBlock *polys = NULL;
    size_t poly_count = 0;
    if (!zr_preprocess_sapp(source, source_len, &clean, &polys, &poly_count, err_out)) {
        return NULL;
    }

    ZrArena arena;
    zr_arena_init(&arena);

    ZrLexer lx;
    memset(&lx, 0, sizeof(lx));
    lx.src = clean;
    lx.len = source_len;
    lx.line = 1;
    lx.arena = &arena;

    if (!zr_lex_all(&lx)) {
        snprintf(err_out, 512, "%s", lx.err);
        free(lx.tokens);
        zr_arena_free(&arena);
        free(clean);
        for (size_t i = 0; i < poly_count; i++) {
            free(polys[i].lang); free(polys[i].name); free(polys[i].code);
            for (size_t a = 0; a < polys[i].attr_count; a++) {
                free(polys[i].attr_keys[a]); free(polys[i].attr_vals[a]);
            }
            free(polys[i].attr_keys); free(polys[i].attr_vals);
        }
        free(polys);
        return NULL;
    }

    ZrParser parser;
    memset(&parser, 0, sizeof(parser));
    parser.toks = lx.tokens;
    parser.len = lx.tok_len;
    parser.arena = &arena;

    ZrStmt **top_stmts = NULL;
    size_t top_len = 0, top_cap = 0;
    bool parse_ok = true;
    while (!zp_check(&parser, TOK_EOF)) {
        if (zp_match(&parser, TOK_SEMI)) continue;
        ZrStmt *s = zp_parse_stmt(&parser);
        if (!s) { parse_ok = false; break; }
        zp_match(&parser, TOK_SEMI);
        if (top_len >= top_cap) {
            top_cap = top_cap ? top_cap * 2 : 32;
            ZrStmt **ns = (ZrStmt **)zr_arena_alloc(&arena, top_cap * sizeof(ZrStmt *));
            if (top_len > 0) memcpy(ns, top_stmts, top_len * sizeof(ZrStmt *));
            top_stmts = ns;
        }
        top_stmts[top_len++] = s;
    }

    free(lx.tokens);
    free(clean);

    if (!parse_ok) {
        snprintf(err_out, 512, "%s", parser.err[0] ? parser.err : "syntax error");
        zr_arena_free(&arena);
        for (size_t i = 0; i < poly_count; i++) {
            free(polys[i].lang); free(polys[i].name); free(polys[i].code);
            for (size_t a = 0; a < polys[i].attr_count; a++) {
                free(polys[i].attr_keys[a]); free(polys[i].attr_vals[a]);
            }
            free(polys[i].attr_keys); free(polys[i].attr_vals);
        }
        free(polys);
        return NULL;
    }

    vm->gc_inhibit++;
    PbFunc *root_fn = pb_func_new(vm);
    root_fn->name = pb_str_c(vm, "<script>");
    root_fn->source_name = pb_str_c(vm, source_name ? source_name : "<script>");
    root_fn->poly_blocks = polys;
    root_fn->poly_count = poly_count;

    zr_scan_top_tests(root_fn, top_stmts, top_len);

    ZrCompiler comp;
    memset(&comp, 0, sizeof(comp));
    comp.vm = vm;
    comp.fn = root_fn;
    comp.is_script = true;
    comp.scope_depth = 0;
    comp.err_buf = err_out;

    /* Hoist top-level `fn` definitions first so mutual/forward top-level calls always resolve! */
    bool comp_ok = true;
    for (size_t i = 0; i < top_len && comp_ok; i++) {
        if (top_stmts[i]->kind == ST_FN) {
            comp_ok = zc_compile_stmt(&comp, top_stmts[i]);
        } else if (top_stmts[i]->kind == ST_SEQ && top_stmts[i]->u.blk.count == 2 &&
                   top_stmts[i]->u.blk.stmts[0]->kind == ST_FN &&
                   top_stmts[i]->u.blk.stmts[1]->kind == ST_EXPORT) {
            comp_ok = zc_compile_stmt(&comp, top_stmts[i]);
        }
    }
    for (size_t i = 0; i < top_len && comp_ok; i++) {
        if (top_stmts[i]->kind == ST_FN) continue;
        if (top_stmts[i]->kind == ST_SEQ && top_stmts[i]->u.blk.count == 2 &&
            top_stmts[i]->u.blk.stmts[0]->kind == ST_FN &&
            top_stmts[i]->u.blk.stmts[1]->kind == ST_EXPORT) continue;
        comp_ok = zc_compile_stmt(&comp, top_stmts[i]);
    }

    if (comp_ok) {
        zc_emit(&comp, OP_CONSTANT, zc_add_const(&comp, val_nil()), 0);
        zc_emit(&comp, OP_RETURN, 0, 0);
        zr_peephole_jump_thread(root_fn);
    }

    free(comp.locals);
    free(comp.upvalues);
    zr_arena_free(&arena);
    vm->gc_inhibit--;

    if (!comp_ok) return NULL;
    return root_fn;
}

#endif /* PBLANG_ZR_COMPILER_H */
