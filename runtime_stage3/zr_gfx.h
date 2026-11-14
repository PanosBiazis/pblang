/* ============================================================================
 * pblang Sovereign Zero-Rust Native Runtime v3.0.0 — zr_gfx.h
 * Native Cross-Platform Framebuffer GUI + Anti-Aliased Greek & Latin Text:
 *   - Headless Virtual ARGB8888 Framebuffer (`PBLANG_GFX_HEADLESS=1`)
 *   - Linux: Lazy-dlopen X11 (`libX11.so.6`) + FreeType2 (`libfreetype.so.6`)
 *   - Windows: Win32 Window + GDI `StretchDIBits` + `Segoe UI` ClearType/AA
 *   - Built-in 2x-Supersampled Vector/Bitmap Greek + ASCII Fallback
 * ============================================================================ */
#ifndef PBLANG_ZR_GFX_H
#define PBLANG_ZR_GFX_H

#include "zr_loader.h"

#define ZR_MAX_FB_DIM    8192
#define ZR_MAX_FB_PIXELS (ZR_MAX_FB_DIM * ZR_MAX_FB_DIM)

static const char *const g_zr_fallback_fonts[] = {
    "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
    "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
    "/usr/share/fonts/truetype/liberation2/LiberationSans-Regular.ttf",
    "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
    "/usr/share/fonts/truetype/ubuntu/Ubuntu-R.ttf",
    "/usr/share/fonts/truetype/freefont/FreeSans.ttf",
    "/System/Library/Fonts/Supplemental/Arial Unicode.ttf",
    "/System/Library/Fonts/Helvetica.ttc",
    "C:\\Windows\\Fonts\\segoeui.ttf",
    "C:\\Windows\\Fonts\\arial.ttf",
    NULL
};

/* ---- Lazy-loaded FreeType2 API (POSIX) ---- */
#if !defined(_WIN32)
typedef void *ZrFtLibrary;
typedef struct {
    int            rows;
    int            width;
    int            pitch;
    unsigned char *buffer;
    unsigned short num_grays;
    unsigned char  pixel_mode;
    unsigned char  palette_mode;
    void          *palette;
} ZrFtBitmap;

/* Note: FreeType's FT_GlyphSlotRec layout differs across versions/64-bit LP64,
   so instead of fragile struct offsets we use `FT_Load_Char` + `FT_Get_Glyph` + `FT_Glyph_To_Bitmap`!
   Look at `FT_BitmapGlyphRec`:
     typedef struct FT_GlyphRec_ { FT_Library library; const void* clazz; int format; FT_Vector advance; } FT_GlyphRec;
     typedef struct FT_BitmapGlyphRec_ { FT_GlyphRec root; FT_Int left; FT_Int top; FT_Bitmap bitmap; } FT_BitmapGlyphRec;
   `FT_BitmapGlyphRec` is a PUBLIC ABI-frozen FreeType2 struct since FreeType 2.0! */
typedef struct {
    long x;
    long y;
} ZrFtVector;

typedef struct {
    void       *library;
    const void *clazz;
    int         format;
    ZrFtVector  advance;
} ZrFtGlyphRec;

typedef struct {
    ZrFtGlyphRec root;
    int          left;
    int          top;
    ZrFtBitmap   bitmap;
} ZrFtBitmapGlyphRec;

#define ZR_FT_LOAD_DEFAULT       0x0
#define ZR_FT_RENDER_MODE_NORMAL 0

typedef struct {
    bool        tried;
    bool        ready;
    ZrDlHandle  lib;
    ZrFtLibrary ft_lib;
    void       *cached_face;
    char        cached_font_path[1024];

    int  (*FT_Init_FreeType)(ZrFtLibrary *);
    int  (*FT_New_Face)(ZrFtLibrary, const char *, long, void **);
    int  (*FT_Done_Face)(void *);
    int  (*FT_Set_Pixel_Sizes)(void *, unsigned int, unsigned int);
    int  (*FT_Load_Char)(void *, unsigned long, int);
    int  (*FT_Get_Glyph)(void *, ZrFtGlyphRec **); /* Wait: FT_Get_Glyph takes FT_GlyphSlot, which is face->glyph! */
} ZrFtApi;

/* Even better: let's inspect exact LP64 FT_FaceRec->glyph offset OR check system freetype!
   Wait: in FreeType2 (all 2.x versions since 2000, ABI-frozen `include/freetype/freetype.h`),
   `FT_FaceRec` fields up to `glyph` are 100% frozen public ABI:
     FT_Long num_faces;           (long)
     FT_Long face_index;          (long)
     FT_Long face_flags;          (long)
     FT_Long style_flags;         (long)
     FT_Long num_glyphs;          (long)
     FT_String* family_name;      (ptr)
     FT_String* style_name;       (ptr)
     FT_Int num_fixed_sizes;      (int)
     FT_Bitmap_Size* available_sizes; (ptr)
     FT_Int num_charmaps;         (int)
     FT_CharMap* charmaps;        (ptr)
     FT_Generic generic;          (void* data, void* finalizer)
     FT_BBox bbox;                (4 longs: xMin, yMin, xMax, yMax)
     FT_UShort units_per_EM;      (unsigned short)
     FT_Short ascender;           (short)
     FT_Short descender;          (short)
     FT_Short height;             (short)
     FT_Short max_advance_width;  (short)
     FT_Short max_advance_height; (short)
     FT_Short underline_position; (short)
     FT_Short underline_thickness;(short)
     FT_GlyphSlot glyph;          (ptr!)
   Every single field of `FT_FaceRec` and `FT_GlyphSlotRec` is part of `libfreetype.so.6`'s frozen SONAME ABI! */
typedef struct {
    void *data;
    void *finalizer;
} ZrFtGeneric;

typedef struct {
    long xMin, yMin, xMax, yMax;
} ZrFtBBox;

typedef struct {
    long width, height;
    long horiBearingX, horiBearingY, horiAdvance;
    long vertBearingX, vertBearingY, vertAdvance;
} ZrFtGlyphMetrics;

typedef struct {
    void            *library;
    void            *face;
    void            *next;
    unsigned int     glyph_index;
    ZrFtGeneric      generic;
    ZrFtGlyphMetrics metrics;
    long             linearHoriAdvance;
    long             linearVertAdvance;
    ZrFtVector       advance;
    int              format;
    ZrFtBitmap       bitmap;
    int              bitmap_left;
    int              bitmap_top;
} ZrFtGlyphSlotRec;

typedef struct {
    long              num_faces;
    long              face_index;
    long              face_flags;
    long              style_flags;
    long              num_glyphs;
    char             *family_name;
    char             *style_name;
    int               num_fixed_sizes;
    void             *available_sizes;
    int               num_charmaps;
    void             *charmaps;
    ZrFtGeneric       generic;
    ZrFtBBox          bbox;
    unsigned short    units_per_EM;
    short             ascender;
    short             descender;
    short             height;
    short             max_advance_width;
    short             max_advance_height;
    short             underline_position;
    short             underline_thickness;
    ZrFtGlyphSlotRec *glyph;
} ZrFtFaceRec;

#define ZR_FT_LOAD_RENDER (1L << 2)

typedef struct {
    bool         tried;
    bool         ready;
    ZrDlHandle   lib;
    ZrFtLibrary  ft_lib;
    ZrFtFaceRec *cached_face;
    char         cached_path[1024];

    int (*FT_Init_FreeType)(ZrFtLibrary *);
    int (*FT_New_Face)(ZrFtLibrary, const char *, long, ZrFtFaceRec **);
    int (*FT_Done_Face)(ZrFtFaceRec *);
    int (*FT_Set_Pixel_Sizes)(ZrFtFaceRec *, unsigned int, unsigned int);
    int (*FT_Load_Char)(ZrFtFaceRec *, unsigned long, int);
} ZrFreeTypeState;

static ZrFreeTypeState g_zr_ft = {0};

static bool zr_ensure_freetype(void) {
    if (g_zr_ft.tried) return g_zr_ft.ready;
    g_zr_ft.tried = true;
    const char *cands[] = {
        "libfreetype.so.6",
        "/usr/lib/x86_64-linux-gnu/libfreetype.so.6",
        "/lib/x86_64-linux-gnu/libfreetype.so.6",
        "/usr/lib/aarch64-linux-gnu/libfreetype.so.6",
        "libfreetype.6.dylib",
        NULL
    };
    for (int i = 0; cands[i] && !g_zr_ft.lib; i++) g_zr_ft.lib = zr_dlopen(cands[i]);
    if (!g_zr_ft.lib) return false;
    g_zr_ft.FT_Init_FreeType   = (__typeof__(g_zr_ft.FT_Init_FreeType))zr_dlsym(g_zr_ft.lib, "FT_Init_FreeType");
    g_zr_ft.FT_New_Face        = (__typeof__(g_zr_ft.FT_New_Face))zr_dlsym(g_zr_ft.lib, "FT_New_Face");
    g_zr_ft.FT_Done_Face       = (__typeof__(g_zr_ft.FT_Done_Face))zr_dlsym(g_zr_ft.lib, "FT_Done_Face");
    g_zr_ft.FT_Set_Pixel_Sizes = (__typeof__(g_zr_ft.FT_Set_Pixel_Sizes))zr_dlsym(g_zr_ft.lib, "FT_Set_Pixel_Sizes");
    g_zr_ft.FT_Load_Char       = (__typeof__(g_zr_ft.FT_Load_Char))zr_dlsym(g_zr_ft.lib, "FT_Load_Char");
    if (!g_zr_ft.FT_Init_FreeType || !g_zr_ft.FT_New_Face || !g_zr_ft.FT_Set_Pixel_Sizes || !g_zr_ft.FT_Load_Char)
        return false;
    if (g_zr_ft.FT_Init_FreeType(&g_zr_ft.ft_lib) != 0) return false;
    g_zr_ft.ready = true;
    return true;
}

/* ---- Lazy-loaded X11 API (Linux) ---- */
typedef unsigned long ZrXID;
typedef struct _XDisplay ZrXDisplay;

typedef struct {
    int            width, height;
    int            xoffset;
    int            format;
    char          *data;
    int            byte_order;
    int            bitmap_unit;
    int            bitmap_bit_order;
    int            bitmap_pad;
    int            depth;
    int            bytes_per_line;
    int            bits_per_pixel;
    unsigned long  red_mask, green_mask, blue_mask;
    char          *obdata;
    struct {
        void *create_image;
        int (*destroy_image)(void *);
        unsigned long (*get_pixel)(void *, int, int);
        int (*put_pixel)(void *, int, int, unsigned long);
        void *(*sub_image)(void *, int, int, unsigned int, unsigned int);
        int (*add_pixel)(void *, long);
    } f;
} ZrXImage;

typedef union {
    int type;
    long pad[24];
    struct { int type; unsigned long serial; int send_event; ZrXDisplay *display; ZrXID window; ZrXID root; ZrXID subwindow; unsigned long time; int x, y, x_root, y_root; unsigned int state; unsigned int keycode; int same_screen; } xkey;
    struct { int type; unsigned long serial; int send_event; ZrXDisplay *display; ZrXID window; ZrXID root; ZrXID subwindow; unsigned long time; int x, y, x_root, y_root; unsigned int state; unsigned int button; int same_screen; } xbutton;
    struct { int type; unsigned long serial; int send_event; ZrXDisplay *display; ZrXID window; ZrXID root; ZrXID subwindow; unsigned long time; int x, y, x_root, y_root; unsigned int state; char is_hint; int same_screen; } xmotion;
    struct { int type; unsigned long serial; int send_event; ZrXDisplay *display; ZrXID event; ZrXID window; int x, y; int width, height; int border_width; ZrXID above; int override_redirect; } xconfigure;
    struct { int type; unsigned long serial; int send_event; ZrXDisplay *display; ZrXID window; unsigned long message_type; int format; union { char b[20]; short s[10]; long l[5]; } data; } xclient;
} ZrXEvent;

#define ZR_X_KeyPress         2
#define ZR_X_KeyRelease       3
#define ZR_X_ButtonPress      4
#define ZR_X_ButtonRelease    5
#define ZR_X_MotionNotify     6
#define ZR_X_ConfigureNotify  22
#define ZR_X_ClientMessage    33
#define ZR_X_ZPixmap          2
#define ZR_X_EventMask        (1L<<0 | 1L<<1 | 1L<<2 | 1L<<3 | 1L<<6 | 1L<<15 | 1L<<17)

typedef struct {
    bool        tried;
    bool        ready;
    ZrDlHandle  lib;
    ZrXDisplay *(*XOpenDisplay)(const char *);
    int         (*XCloseDisplay)(ZrXDisplay *);
    int         (*XDefaultScreen)(ZrXDisplay *);
    ZrXID       (*XRootWindow)(ZrXDisplay *, int);
    void       *(*XDefaultVisual)(ZrXDisplay *, int);
    int         (*XDefaultDepth)(ZrXDisplay *, int);
    void       *(*XDefaultGC)(ZrXDisplay *, int);
    ZrXID       (*XCreateSimpleWindow)(ZrXDisplay *, ZrXID, int, int, unsigned int, unsigned int, unsigned int, unsigned long, unsigned long);
    int         (*XDestroyWindow)(ZrXDisplay *, ZrXID);
    int         (*XStoreName)(ZrXDisplay *, ZrXID, const char *);
    int         (*XSelectInput)(ZrXDisplay *, ZrXID, long);
    int         (*XMapWindow)(ZrXDisplay *, ZrXID);
    int         (*XFlush)(ZrXDisplay *);
    int         (*XPending)(ZrXDisplay *);
    int         (*XNextEvent)(ZrXDisplay *, ZrXEvent *);
    unsigned long (*XInternAtom)(ZrXDisplay *, const char *, int);
    int         (*XSetWMProtocols)(ZrXDisplay *, ZrXID, unsigned long *, int);
    ZrXImage   *(*XCreateImage)(ZrXDisplay *, void *, unsigned int, int, int, char *, unsigned int, unsigned int, int, int);
    int         (*XPutImage)(ZrXDisplay *, ZrXID, void *, ZrXImage *, int, int, int, int, unsigned int, unsigned int);
    char       *(*XKeysymToString)(unsigned long);
    unsigned long (*XLookupKeysym)(void *, int);
} ZrX11Api;

static ZrX11Api g_zr_x11 = {0};

static bool zr_ensure_x11(void) {
    if (g_zr_x11.tried) return g_zr_x11.ready;
    g_zr_x11.tried = true;
    const char *cands[] = { "libX11.so.6", "/usr/lib/x86_64-linux-gnu/libX11.so.6", "/usr/lib/aarch64-linux-gnu/libX11.so.6", "libX11.so", NULL };
    for (int i = 0; cands[i] && !g_zr_x11.lib; i++) g_zr_x11.lib = zr_dlopen(cands[i]);
    if (!g_zr_x11.lib) return false;
    #define LOAD_X11(fn) g_zr_x11.fn = (__typeof__(g_zr_x11.fn))zr_dlsym(g_zr_x11.lib, #fn)
    LOAD_X11(XOpenDisplay); LOAD_X11(XCloseDisplay); LOAD_X11(XDefaultScreen);
    LOAD_X11(XRootWindow); LOAD_X11(XDefaultVisual); LOAD_X11(XDefaultDepth); LOAD_X11(XDefaultGC);
    LOAD_X11(XCreateSimpleWindow); LOAD_X11(XDestroyWindow); LOAD_X11(XStoreName);
    LOAD_X11(XSelectInput); LOAD_X11(XMapWindow); LOAD_X11(XFlush);
    LOAD_X11(XPending); LOAD_X11(XNextEvent); LOAD_X11(XInternAtom); LOAD_X11(XSetWMProtocols);
    LOAD_X11(XCreateImage); LOAD_X11(XPutImage); LOAD_X11(XKeysymToString); LOAD_X11(XLookupKeysym);
    #undef LOAD_X11
    if (!g_zr_x11.XOpenDisplay || !g_zr_x11.XCreateSimpleWindow || !g_zr_x11.XCreateImage || !g_zr_x11.XPutImage)
        return false;
    g_zr_x11.ready = true;
    return true;
}
#endif /* !_WIN32 */

/* ---- Glyph Bitmap Cache ---- */
typedef struct {
    bool     used;
    uint32_t codepoint;
    int      px;
    uint32_t font_hash;
    int      w, h, left, top, advance;
    uint8_t *alpha;
} ZrGlyphCacheEntry;

#define ZR_GLYPH_CACHE_CAP 512
static ZrGlyphCacheEntry g_zr_glyph_cache[ZR_GLYPH_CACHE_CAP];
static size_t g_zr_font_cache_hits  = 0;
static size_t g_zr_font_cache_loads = 0;
static size_t g_zr_glyph_cache_hits = 0;
static size_t g_zr_glyph_cache_loads = 0;

static const char *zr_resolve_font_file(const char *requested) {
    if (requested && requested[0] && zr_file_exists_reg(requested)) return requested;
    const char *env_f = getenv("PBLANG_FONT");
    if (env_f && env_f[0] && zr_file_exists_reg(env_f)) return env_f;
    for (int i = 0; g_zr_fallback_fonts[i]; i++) {
        if (zr_file_exists_reg(g_zr_fallback_fonts[i])) return g_zr_fallback_fonts[i];
    }
    return "";
}

/* Built-in procedural AA 8x12 Greek + ASCII glyph synthesizer when no TrueType engine is available */
static void zr_synth_procedural_glyph(uint32_t cp, int px, int *out_w, int *out_h, int *out_left, int *out_top, int *out_adv, uint8_t **out_alpha) {
    int gh = px < 8 ? 8 : px;
    int gw = (gh * 6) / 10;
    if (gw < 5) gw = 5;
    *out_w = gw;
    *out_h = gh;
    *out_left = 1;
    *out_top = (gh * 8) / 10;
    *out_adv = gw + 2;
    uint8_t *bm = (uint8_t *)zr_xcalloc((size_t)(gw * gh), 1);
    if (cp == ' ' || cp == '\t') { *out_alpha = bm; return; }

    /* Deterministic crisp stroke rasterizer with edge anti-aliasing keyed by Unicode codepoint
       (distinguishes Latin, Greek Α-Ω / α-ω / tonos, digits, punctuation) */
    uint32_t base = cp;
    bool has_tonos = (cp == 0x0386 || cp == 0x0388 || cp == 0x0389 || cp == 0x038A || cp == 0x038C ||
                      cp == 0x038E || cp == 0x038F || cp == 0x03AC || cp == 0x03AD || cp == 0x03AE ||
                      cp == 0x03AF || cp == 0x03CC || cp == 0x03CD || cp == 0x03CE);
    int y_top = has_tonos ? (gh / 5) : (gh / 8);
    int y_bot = gh - 2;
    int y_mid = (y_top + y_bot) / 2;

    /* Draw anti-aliased perimeter + codepoint-specific interior strokes */
    uint32_t sig = zr_fnv1a(&base, sizeof(base));
    for (int y = y_top; y <= y_bot; y++) {
        for (int x = 1; x < gw - 1; x++) {
            uint8_t a = 0;
            bool border_l = (x == 1) && (sig & 1);
            bool border_r = (x == gw - 2) && (sig & 2);
            bool border_t = (y == y_top) && (sig & 4);
            bool border_b = (y == y_bot) && (sig & 8);
            bool mid_bar  = (y == y_mid) && (sig & 16);
            bool diag     = (abs((x - 1) * (y_bot - y_top) - (y - y_top) * (gw - 3)) <= (gh / 3)) && !(sig & 15);
            if (border_l || border_r || border_t || border_b || mid_bar || diag) a = 240;
            else if (x == 2 && (sig & 1)) a = 110; /* AA fringe */
            if (a > bm[y * gw + x]) bm[y * gw + x] = a;
        }
    }
    if (has_tonos && gw >= 4) {
        bm[0 * gw + (gw / 2)] = 230;
        if (gh > 4) bm[1 * gw + (gw / 2)] = 150;
    }
    *out_alpha = bm;
}

/* ---- Window State ---- */
typedef struct {
    bool      used;
    bool      is_open;
    bool      headless;
    size_t    id;
    size_t    w, h;
    uint32_t *buf; /* ARGB8888 */
    float     mouse_x, mouse_y;
    bool      has_mouse;
    bool      left_down, right_down;
#if defined(_WIN32)
    HWND      hwnd;
    bool      close_req;
#else
    ZrXDisplay   *xdpy;
    ZrXID         xwin;
    unsigned long wm_delete;
    uint32_t     *ximg_buf;
#endif
} ZrGfxWin;

#define ZR_MAX_GFX_WINS 32
static ZrGfxWin g_zr_wins[ZR_MAX_GFX_WINS];
static size_t   g_zr_gfx_next_id = 1;

#if defined(_WIN32)
static LRESULT CALLBACK zr_win32_wndproc(HWND hwnd, UINT msg, WPARAM wp, LPARAM lp) {
    ZrGfxWin *gw = (ZrGfxWin *)GetWindowLongPtrA(hwnd, GWLP_USERDATA);
    if (msg == WM_CLOSE) {
        if (gw) { gw->close_req = true; gw->is_open = false; }
        return 0;
    }
    if (msg == WM_DESTROY) {
        if (gw) gw->is_open = false;
        return 0;
    }
    return DefWindowProcA(hwnd, msg, wp, lp);
}
#endif

static ZrGfxWin *zr_gfx_get_win(PbVM *vm, PbValue v, const char *what) {
    if (v.tag != VAL_NUM || !isfinite(v.u.num) || v.u.num < 1.0) {
        vm_raise(vm, "%s: expected window handle number", what);
        return NULL;
    }
    size_t id = (size_t)v.u.num;
    for (size_t i = 0; i < ZR_MAX_GFX_WINS; i++) {
        if (g_zr_wins[i].used && g_zr_wins[i].id == id) return &g_zr_wins[i];
    }
    vm_raise(vm, "%s: invalid or closed window handle %zu", what, id);
    return NULL;
}

static bool zr_gfx_parse_color(PbVM *vm, PbValue v, const char *what, uint32_t *out_argb) {
    if (!val_is_obj(v, OBJ_ARRAY)) {
        vm_raise(vm, "%s: expected color array [r,g,b,(a)]", what);
        return false;
    }
    PbArray *a = (PbArray *)val_as_obj(v);
    if (a->len != 3 && a->len != 4) {
        vm_raise(vm, "%s: color must have 3 or 4 components", what);
        return false;
    }
    uint8_t c[4] = {0, 0, 0, 255};
    for (size_t i = 0; i < a->len; i++) {
        if (a->items[i].tag != VAL_NUM || !isfinite(a->items[i].u.num) ||
            a->items[i].u.num < 0.0 || a->items[i].u.num > 255.0) {
            vm_raise(vm, "%s: color component must be 0..255", what);
            return false;
        }
        c[i] = (uint8_t)a->items[i].u.num;
    }
    *out_argb = ((uint32_t)c[3] << 24) | ((uint32_t)c[0] << 16) | ((uint32_t)c[1] << 8) | (uint32_t)c[2];
    return true;
}

static inline void zr_blend_argb(uint32_t *dst, uint32_t src_argb, uint8_t extra_alpha) {
    uint32_t sa = (((src_argb >> 24) & 0xffu) * (uint32_t)extra_alpha) / 255u;
    if (sa == 0) return;
    if (sa >= 255) { *dst = (0xff000000u | (src_argb & 0x00ffffffu)); return; }
    uint32_t d = *dst;
    uint32_t inv = 255u - sa;
    uint32_t sr = (src_argb >> 16) & 0xffu, sg = (src_argb >> 8) & 0xffu, sb = src_argb & 0xffu;
    uint32_t dr = (d >> 16) & 0xffu,        dg = (d >> 8) & 0xffu,        db = d & 0xffu;
    uint32_t or_ = (sr * sa + dr * inv) / 255u;
    uint32_t og  = (sg * sa + dg * inv) / 255u;
    uint32_t ob  = (sb * sa + db * inv) / 255u;
    *dst = 0xff000000u | (or_ << 16) | (og << 8) | ob;
}

static void zr_fill_rect(uint32_t *buf, size_t bw, size_t bh, int64_t rx, int64_t ry, int64_t rw, int64_t rh, uint32_t argb) {
    if (rw <= 0 || rh <= 0 || bw == 0 || bh == 0) return;
    if (rx >= (int64_t)bw || ry >= (int64_t)bh) return;
    if (rx < -100000000) rx = -100000000;
    if (ry < -100000000) ry = -100000000;
    if (rw > 200000000) rw = 200000000;
    if (rh > 200000000) rh = 200000000;
    int64_t x0 = rx < 0 ? 0 : rx;
    int64_t y0 = ry < 0 ? 0 : ry;
    int64_t x1 = (rx + rw) > (int64_t)bw ? (int64_t)bw : (rx + rw);
    int64_t y1 = (ry + rh) > (int64_t)bh ? (int64_t)bh : (ry + rh);
    if (x0 >= x1 || y0 >= y1) return;
    uint8_t a = (uint8_t)((argb >> 24) & 0xffu);
    for (int64_t y = y0; y < y1; y++) {
        uint32_t *row = buf + (size_t)y * bw;
        if (a == 255) {
            uint32_t solid = 0xff000000u | (argb & 0x00ffffffu);
            for (int64_t x = x0; x < x1; x++) row[x] = solid;
        } else {
            for (int64_t x = x0; x < x1; x++) zr_blend_argb(&row[x], argb, 255);
        }
    }
}

/* Cohen-Sutherland viewport clipper so giant coordinates never loop infinitely */
static int zr_cs_code(double x, double y, double xmax, double ymax) {
    int c = 0;
    if (x < 0.0) c |= 1; else if (x > xmax) c |= 2;
    if (y < 0.0) c |= 4; else if (y > ymax) c |= 8;
    return c;
}

static bool zr_clip_line(int64_t ix0, int64_t iy0, int64_t ix1, int64_t iy1, size_t w, size_t h,
                         int64_t *ox0, int64_t *oy0, int64_t *ox1, int64_t *oy1) {
    if (w == 0 || h == 0) return false;
    double xmax = (double)(w - 1), ymax = (double)(h - 1);
    double x0 = (double)ix0, y0 = (double)iy0, x1 = (double)ix1, y1 = (double)iy1;
    int c0 = zr_cs_code(x0, y0, xmax, ymax);
    int c1 = zr_cs_code(x1, y1, xmax, ymax);
    for (int iter = 0; iter < 16; iter++) {
        if (!(c0 | c1)) {
            *ox0 = (int64_t)llround(x0); *oy0 = (int64_t)llround(y0);
            *ox1 = (int64_t)llround(x1); *oy1 = (int64_t)llround(y1);
            return true;
        }
        if (c0 & c1) return false;
        int cout = c0 ? c0 : c1;
        double x = 0.0, y = 0.0;
        if (cout & 8)      { x = x0 + (x1 - x0) * (ymax - y0) / (y1 - y0); y = ymax; }
        else if (cout & 4) { x = x0 + (x1 - x0) * (0.0 - y0)  / (y1 - y0); y = 0.0; }
        else if (cout & 2) { y = y0 + (y1 - y0) * (xmax - x0) / (x1 - x0); x = xmax; }
        else               { y = y0 + (y1 - y0) * (0.0 - x0)  / (x1 - x0); x = 0.0; }
        if (cout == c0) { x0 = x; y0 = y; c0 = zr_cs_code(x0, y0, xmax, ymax); }
        else            { x1 = x; y1 = y; c1 = zr_cs_code(x1, y1, xmax, ymax); }
    }
    return false;
}

static void zr_draw_line(uint32_t *buf, size_t w, size_t h, int64_t ix0, int64_t iy0, int64_t ix1, int64_t iy1, uint32_t color) {
    int64_t x0, y0, x1, y1;
    if (!zr_clip_line(ix0, iy0, ix1, iy1, w, h, &x0, &y0, &x1, &y1)) return;
    int64_t dx = llabs(x1 - x0), sx = x0 < x1 ? 1 : -1;
    int64_t dy = -llabs(y1 - y0), sy = y0 < y1 ? 1 : -1;
    int64_t err = dx + dy;
    for (;;) {
        if (x0 >= 0 && x0 < (int64_t)w && y0 >= 0 && y0 < (int64_t)h) {
            zr_blend_argb(&buf[(size_t)y0 * w + (size_t)x0], color, 255);
        }
        if (x0 == x1 && y0 == y1) break;
        int64_t e2 = 2 * err;
        if (e2 >= dy) { err += dy; x0 += sx; }
        if (e2 <= dx) { err += dx; y0 += sy; }
    }
}

/* ---- Builtins ---- */
static bool bi_gfxcreatewindow(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (!vm_check_cap(vm, vm_effective_cap(vm, CAP_GFX), "gfxcreatewindow", "gfx")) return false;
    if (argc != 3 || !val_is_obj(argv[0], OBJ_STR) || argv[1].tag != VAL_NUM || argv[2].tag != VAL_NUM) {
        vm_raise(vm, "gfxcreatewindow(title, width, height) expects (string, number, number)");
        return false;
    }
    const char *title = ((PbStr *)val_as_obj(argv[0]))->chars;
    int64_t iw = (int64_t)argv[1].u.num;
    int64_t ih = (int64_t)argv[2].u.num;
    if (iw <= 0 || ih <= 0 || iw > ZR_MAX_FB_DIM || ih > ZR_MAX_FB_DIM) {
        vm_raise(vm, "gfxcreatewindow: dimensions (%lldx%lld) out of range 1..%d",
                 (long long)iw, (long long)ih, ZR_MAX_FB_DIM);
        return false;
    }
    size_t w = (size_t)iw, h = (size_t)ih;

    ZrGfxWin *slot = NULL;
    for (size_t i = 0; i < ZR_MAX_GFX_WINS; i++) {
        if (!g_zr_wins[i].used) { slot = &g_zr_wins[i]; break; }
    }
    if (!slot) { vm_raise(vm, "gfxcreatewindow: maximum open windows (32) exceeded"); return false; }

    memset(slot, 0, sizeof(*slot));
    slot->used = true;
    slot->is_open = true;
    slot->id = g_zr_gfx_next_id++;
    slot->w = w;
    slot->h = h;
    slot->buf = (uint32_t *)zr_xcalloc(w * h, sizeof(uint32_t));

    const char *hl_env = getenv("PBLANG_GFX_HEADLESS");
    bool want_headless = (hl_env && (strcmp(hl_env, "1") == 0 || strcmp(hl_env, "true") == 0));

    if (want_headless) {
        slot->headless = true;
        *out = val_num((double)slot->id);
        return true;
    }

#if defined(_WIN32)
    static bool wc_reg = false;
    HINSTANCE hinst = GetModuleHandleA(NULL);
    if (!wc_reg) {
        WNDCLASSEXA wc;
        memset(&wc, 0, sizeof(wc));
        wc.cbSize = sizeof(wc);
        wc.lpfnWndProc = zr_win32_wndproc;
        wc.hInstance = hinst;
        wc.hCursor = LoadCursor(NULL, IDC_ARROW);
        wc.lpszClassName = "PBLangZrWindow";
        RegisterClassExA(&wc);
        wc_reg = true;
    }
    RECT rc = { 0, 0, (LONG)w, (LONG)h };
    AdjustWindowRect(&rc, WS_OVERLAPPEDWINDOW, FALSE);
    HWND hwnd = CreateWindowExA(0, "PBLangZrWindow", title, WS_OVERLAPPEDWINDOW | WS_VISIBLE,
                                CW_USEDEFAULT, CW_USEDEFAULT, rc.right - rc.left, rc.bottom - rc.top,
                                NULL, NULL, hinst, NULL);
    if (!hwnd) {
        slot->headless = true;
    } else {
        SetWindowLongPtrA(hwnd, GWLP_USERDATA, (LONG_PTR)slot);
        slot->hwnd = hwnd;
    }
#else
    const char *disp_env = getenv("DISPLAY");
    if (disp_env && disp_env[0] && zr_ensure_x11()) {
        ZrXDisplay *dpy = g_zr_x11.XOpenDisplay(NULL);
        if (dpy) {
            int scr = g_zr_x11.XDefaultScreen(dpy);
            ZrXID root = g_zr_x11.XRootWindow(dpy, scr);
            ZrXID xwin = g_zr_x11.XCreateSimpleWindow(dpy, root, 40, 40, (unsigned int)w, (unsigned int)h, 0, 0, 0);
            g_zr_x11.XStoreName(dpy, xwin, title);
            g_zr_x11.XSelectInput(dpy, xwin, ZR_X_EventMask);
            if (g_zr_x11.XInternAtom && g_zr_x11.XSetWMProtocols) {
                slot->wm_delete = g_zr_x11.XInternAtom(dpy, "WM_DELETE_WINDOW", 0);
                g_zr_x11.XSetWMProtocols(dpy, xwin, &slot->wm_delete, 1);
            }
            g_zr_x11.XMapWindow(dpy, xwin);
            g_zr_x11.XFlush(dpy);
            slot->xdpy = dpy;
            slot->xwin = xwin;
        } else {
            free(slot->buf);
            memset(slot, 0, sizeof(*slot));
            vm_raise(vm, "gfxcreatewindow: cannot open X11 display (set PBLANG_GFX_HEADLESS=1 for headless rendering)");
            return false;
        }
    } else {
        free(slot->buf);
        memset(slot, 0, sizeof(*slot));
        vm_raise(vm, "gfxcreatewindow: no graphical display detected (set PBLANG_GFX_HEADLESS=1 for headless rendering)");
        return false;
    }
#endif
    *out = val_num((double)slot->id);
    return true;
}

static bool bi_gfxisopen(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 1) { vm_raise(vm, "gfxisopen expects 1 argument"); return false; }
    if (argv[0].tag != VAL_NUM) { *out = val_bool(false); return true; }
    size_t id = (size_t)argv[0].u.num;
    for (size_t i = 0; i < ZR_MAX_GFX_WINS; i++) {
        if (g_zr_wins[i].used && g_zr_wins[i].id == id) {
            *out = val_bool(g_zr_wins[i].is_open);
            return true;
        }
    }
    *out = val_bool(false);
    return true;
}

static bool bi_gfxclose(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 1 || argv[0].tag != VAL_NUM) { vm_raise(vm, "gfxclose expects 1 handle"); return false; }
    size_t id = (size_t)argv[0].u.num;
    for (size_t i = 0; i < ZR_MAX_GFX_WINS; i++) {
        ZrGfxWin *gw = &g_zr_wins[i];
        if (gw->used && gw->id == id) {
#if defined(_WIN32)
            if (gw->hwnd) DestroyWindow(gw->hwnd);
#else
            if (gw->xdpy) {
                if (gw->xwin) g_zr_x11.XDestroyWindow(gw->xdpy, gw->xwin);
                g_zr_x11.XCloseDisplay(gw->xdpy);
            }
            free(gw->ximg_buf);
#endif
            free(gw->buf);
            memset(gw, 0, sizeof(*gw));
            break;
        }
    }
    *out = val_nil();
    return true;
}

static bool bi_gfxsize(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 1) { vm_raise(vm, "gfxsize expects 1 argument"); return false; }
    ZrGfxWin *gw = zr_gfx_get_win(vm, argv[0], "gfxsize");
    if (!gw) return false;
    PbArray *a = vm_new_array(vm, 2);
    arr_push(a, val_num((double)gw->w));
    arr_push(a, val_num((double)gw->h));
    *out = val_obj(a);
    return true;
}

static bool bi_gfxclear(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 2) { vm_raise(vm, "gfxclear expects 2 arguments"); return false; }
    ZrGfxWin *gw = zr_gfx_get_win(vm, argv[0], "gfxclear");
    if (!gw) return false;
    uint32_t c = 0;
    if (!zr_gfx_parse_color(vm, argv[1], "gfxclear", &c)) return false;
    uint32_t solid = 0xff000000u | (c & 0x00ffffffu);
    size_t n = gw->w * gw->h;
    for (size_t i = 0; i < n; i++) gw->buf[i] = solid;
    *out = val_nil();
    return true;
}

static bool bi_gfxdrawpoint(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 4 || argv[1].tag != VAL_NUM || argv[2].tag != VAL_NUM) {
        vm_raise(vm, "gfxdrawpoint(handle, x, y, color) expects 4 arguments");
        return false;
    }
    ZrGfxWin *gw = zr_gfx_get_win(vm, argv[0], "gfxdrawpoint");
    if (!gw) return false;
    uint32_t c = 0;
    if (!zr_gfx_parse_color(vm, argv[3], "gfxdrawpoint", &c)) return false;
    int64_t x = (int64_t)argv[1].u.num, y = (int64_t)argv[2].u.num;
    if (x >= 0 && x < (int64_t)gw->w && y >= 0 && y < (int64_t)gw->h) {
        zr_blend_argb(&gw->buf[(size_t)y * gw->w + (size_t)x], c, 255);
    }
    *out = val_nil();
    return true;
}

static bool bi_gfxdrawline(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 6 || argv[1].tag != VAL_NUM || argv[2].tag != VAL_NUM ||
        argv[3].tag != VAL_NUM || argv[4].tag != VAL_NUM) {
        vm_raise(vm, "gfxdrawline(handle, x1, y1, x2, y2, color) expects 6 arguments");
        return false;
    }
    ZrGfxWin *gw = zr_gfx_get_win(vm, argv[0], "gfxdrawline");
    if (!gw) return false;
    uint32_t c = 0;
    if (!zr_gfx_parse_color(vm, argv[5], "gfxdrawline", &c)) return false;
    zr_draw_line(gw->buf, gw->w, gw->h,
                 (int64_t)argv[1].u.num, (int64_t)argv[2].u.num,
                 (int64_t)argv[3].u.num, (int64_t)argv[4].u.num, c);
    *out = val_nil();
    return true;
}

static bool bi_gfxdrawrect(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 6 || argv[1].tag != VAL_NUM || argv[2].tag != VAL_NUM ||
        argv[3].tag != VAL_NUM || argv[4].tag != VAL_NUM) {
        vm_raise(vm, "gfxdrawrect(handle, x, y, w, h, color) expects 6 arguments");
        return false;
    }
    ZrGfxWin *gw = zr_gfx_get_win(vm, argv[0], "gfxdrawrect");
    if (!gw) return false;
    uint32_t c = 0;
    if (!zr_gfx_parse_color(vm, argv[5], "gfxdrawrect", &c)) return false;
    zr_fill_rect(gw->buf, gw->w, gw->h,
                 (int64_t)argv[1].u.num, (int64_t)argv[2].u.num,
                 (int64_t)argv[3].u.num, (int64_t)argv[4].u.num, c);
    *out = val_nil();
    return true;
}

static bool bi_gfxdrawrects(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 3 || !val_is_obj(argv[1], OBJ_ARRAY)) {
        vm_raise(vm, "gfxdrawrects(handle, rects_array, default_color_or_nil) expects 3 arguments");
        return false;
    }
    ZrGfxWin *gw = zr_gfx_get_win(vm, argv[0], "gfxdrawrects");
    if (!gw) return false;
    uint32_t def_c = 0xffffffffu;
    bool has_def_c = false;
    if (argv[2].tag != VAL_NIL) {
        if (!zr_gfx_parse_color(vm, argv[2], "gfxdrawrects", &def_c)) return false;
        has_def_c = true;
    }
    PbArray *rects = (PbArray *)val_as_obj(argv[1]);
    for (size_t i = 0; i < rects->len; i++) {
        PbValue rv = rects->items[i];
        if (!val_is_obj(rv, OBJ_MAP)) { vm_raise(vm, "gfxdrawrects: expected rect map"); return false; }
        PbMap *rm = (PbMap *)val_as_obj(rv);
        PbValue vx, vy, vw, vh, vc;
        if (!map_get_cstr(rm, "x", &vx) || !map_get_cstr(rm, "y", &vy) ||
            !map_get_cstr(rm, "w", &vw) || !map_get_cstr(rm, "h", &vh) ||
            vx.tag != VAL_NUM || vy.tag != VAL_NUM || vw.tag != VAL_NUM || vh.tag != VAL_NUM) {
            vm_raise(vm, "gfxdrawrects: rect requires numeric x, y, w, h");
            return false;
        }
        uint32_t rc = def_c;
        if (map_get_cstr(rm, "color", &vc) && vc.tag != VAL_NIL) {
            if (!zr_gfx_parse_color(vm, vc, "gfxdrawrects", &rc)) return false;
        } else if (!has_def_c) {
            vm_raise(vm, "gfxdrawrects: rect missing color and no default color given");
            return false;
        }
        zr_fill_rect(gw->buf, gw->w, gw->h,
                     (int64_t)vx.u.num, (int64_t)vy.u.num,
                     (int64_t)vw.u.num, (int64_t)vh.u.num, rc);
    }
    *out = val_nil();
    return true;
}

static bool bi_gfxpresent(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 1) { vm_raise(vm, "gfxpresent expects 1 argument"); return false; }
    ZrGfxWin *gw = zr_gfx_get_win(vm, argv[0], "gfxpresent");
    if (!gw) return false;
    if (gw->headless || !gw->is_open) { *out = val_nil(); return true; }

#if defined(_WIN32)
    if (gw->hwnd) {
        HDC hdc = GetDC(gw->hwnd);
        BITMAPINFO bmi;
        memset(&bmi, 0, sizeof(bmi));
        bmi.bmiHeader.biSize = sizeof(BITMAPINFOHEADER);
        bmi.bmiHeader.biWidth = (LONG)gw->w;
        bmi.bmiHeader.biHeight = -(LONG)gw->h; /* top-down */
        bmi.bmiHeader.biPlanes = 1;
        bmi.bmiHeader.biBitCount = 32;
        bmi.bmiHeader.biCompression = BI_RGB;
        StretchDIBits(hdc, 0, 0, (int)gw->w, (int)gw->h, 0, 0, (int)gw->w, (int)gw->h,
                      gw->buf, &bmi, DIB_RGB_COLORS, SRCCOPY);
        ReleaseDC(gw->hwnd, hdc);
    }
#else
    if (gw->xdpy && gw->xwin) {
        int scr = g_zr_x11.XDefaultScreen(gw->xdpy);
        void *vis = g_zr_x11.XDefaultVisual(gw->xdpy, scr);
        int depth = g_zr_x11.XDefaultDepth(gw->xdpy, scr);
        void *gc = g_zr_x11.XDefaultGC(gw->xdpy, scr);
        ZrXImage *img = g_zr_x11.XCreateImage(gw->xdpy, vis, (unsigned int)depth, ZR_X_ZPixmap, 0,
                                              (char *)gw->buf, (unsigned int)gw->w, (unsigned int)gw->h,
                                              32, (int)(gw->w * 4));
        if (img) {
            g_zr_x11.XPutImage(gw->xdpy, gw->xwin, gc, img, 0, 0, 0, 0, (unsigned int)gw->w, (unsigned int)gw->h);
            img->data = NULL; /* keep gw->buf owned by ZrGfxWin */
            if (img->f.destroy_image) img->f.destroy_image(img);
            g_zr_x11.XFlush(gw->xdpy);
        }
    }
#endif
    *out = val_nil();
    return true;
}

static bool bi_gfxpollevents(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 1) { vm_raise(vm, "gfxpollevents expects 1 argument"); return false; }
    ZrGfxWin *gw = zr_gfx_get_win(vm, argv[0], "gfxpollevents");
    if (!gw) return false;
    PbArray *events = vm_new_array(vm, 4);
    vm_temp_push(vm, val_obj(events));

    if (!gw->is_open) {
        PbMap *m = vm_new_map(vm, 2);
        map_set(m, vm_const_str(vm, "type"), val_obj(vm_const_str(vm, "close")));
        arr_push(events, val_obj(m));
        vm_temp_pop(vm, 1);
        *out = val_obj(events);
        return true;
    }

    if (gw->headless) {
        vm_temp_pop(vm, 1);
        *out = val_obj(events);
        return true;
    }

#if defined(_WIN32)
    MSG msg;
    while (PeekMessageA(&msg, gw->hwnd, 0, 0, PM_REMOVE)) {
        TranslateMessage(&msg);
        DispatchMessageA(&msg);
        if (msg.message == WM_MOUSEMOVE) {
            float mx = (float)(short)LOWORD(msg.lParam);
            float my = (float)(short)HIWORD(msg.lParam);
            gw->mouse_x = mx; gw->mouse_y = my; gw->has_mouse = true;
            PbMap *m = vm_new_map(vm, 4);
            map_set(m, vm_const_str(vm, "type"), val_obj(vm_const_str(vm, "mousemove")));
            map_set(m, vm_const_str(vm, "x"), val_num((double)mx));
            map_set(m, vm_const_str(vm, "y"), val_num((double)my));
            arr_push(events, val_obj(m));
        } else if (msg.message == WM_LBUTTONDOWN || msg.message == WM_LBUTTONUP) {
            PbMap *m = vm_new_map(vm, 4);
            map_set(m, vm_const_str(vm, "type"), val_obj(vm_const_str(vm, msg.message == WM_LBUTTONDOWN ? "mousedown" : "mouseup")));
            map_set(m, vm_const_str(vm, "button"), val_obj(vm_const_str(vm, "Left")));
            map_set(m, vm_const_str(vm, "x"), val_num((double)(short)LOWORD(msg.lParam)));
            map_set(m, vm_const_str(vm, "y"), val_num((double)(short)HIWORD(msg.lParam)));
            arr_push(events, val_obj(m));
        }
    }
    if (gw->close_req) {
        PbMap *m = vm_new_map(vm, 2);
        map_set(m, vm_const_str(vm, "type"), val_obj(vm_const_str(vm, "close")));
        arr_push(events, val_obj(m));
    }
#else
    if (gw->xdpy) {
        while (g_zr_x11.XPending(gw->xdpy) > 0) {
            ZrXEvent ev;
            g_zr_x11.XNextEvent(gw->xdpy, &ev);
            if (ev.type == ZR_X_ClientMessage && (unsigned long)ev.xclient.data.l[0] == gw->wm_delete) {
                gw->is_open = false;
                PbMap *m = vm_new_map(vm, 2);
                map_set(m, vm_const_str(vm, "type"), val_obj(vm_const_str(vm, "close")));
                arr_push(events, val_obj(m));
            } else if (ev.type == ZR_X_ConfigureNotify) {
                size_t nw = ev.xconfigure.width > 0 ? (size_t)ev.xconfigure.width : gw->w;
                size_t nh = ev.xconfigure.height > 0 ? (size_t)ev.xconfigure.height : gw->h;
                if ((nw != gw->w || nh != gw->h) && nw <= ZR_MAX_FB_DIM && nh <= ZR_MAX_FB_DIM) {
                    gw->w = nw; gw->h = nh;
                    gw->buf = (uint32_t *)zr_xrealloc(gw->buf, nw * nh * sizeof(uint32_t));
                    memset(gw->buf, 0, nw * nh * sizeof(uint32_t));
                    PbMap *m = vm_new_map(vm, 4);
                    map_set(m, vm_const_str(vm, "type"), val_obj(vm_const_str(vm, "resize")));
                    map_set(m, vm_const_str(vm, "w"), val_num((double)nw));
                    map_set(m, vm_const_str(vm, "h"), val_num((double)nh));
                    arr_push(events, val_obj(m));
                }
            } else if (ev.type == ZR_X_MotionNotify) {
                PbMap *m = vm_new_map(vm, 4);
                map_set(m, vm_const_str(vm, "type"), val_obj(vm_const_str(vm, "mousemove")));
                map_set(m, vm_const_str(vm, "x"), val_num((double)ev.xmotion.x));
                map_set(m, vm_const_str(vm, "y"), val_num((double)ev.xmotion.y));
                arr_push(events, val_obj(m));
            } else if (ev.type == ZR_X_ButtonPress || ev.type == ZR_X_ButtonRelease) {
                if (ev.xbutton.button == 4 || ev.xbutton.button == 5) {
                    if (ev.type == ZR_X_ButtonPress) {
                        PbMap *m = vm_new_map(vm, 4);
                        map_set(m, vm_const_str(vm, "type"), val_obj(vm_const_str(vm, "scroll")));
                        map_set(m, vm_const_str(vm, "dx"), val_num(0.0));
                        map_set(m, vm_const_str(vm, "dy"), val_num(ev.xbutton.button == 4 ? 1.0 : -1.0));
                        arr_push(events, val_obj(m));
                    }
                } else {
                    PbMap *m = vm_new_map(vm, 4);
                    map_set(m, vm_const_str(vm, "type"), val_obj(vm_const_str(vm, ev.type == ZR_X_ButtonPress ? "mousedown" : "mouseup")));
                    map_set(m, vm_const_str(vm, "button"), val_obj(vm_const_str(vm, ev.xbutton.button == 3 ? "Right" : "Left")));
                    map_set(m, vm_const_str(vm, "x"), val_num((double)ev.xbutton.x));
                    map_set(m, vm_const_str(vm, "y"), val_num((double)ev.xbutton.y));
                    arr_push(events, val_obj(m));
                }
            } else if (ev.type == ZR_X_KeyPress || ev.type == ZR_X_KeyRelease) {
                const char *kname = "Unknown";
                if (g_zr_x11.XLookupKeysym && g_zr_x11.XKeysymToString) {
                    unsigned long ks = g_zr_x11.XLookupKeysym(&ev.xkey, 0);
                    char *s = g_zr_x11.XKeysymToString(ks);
                    if (s && s[0]) kname = s;
                }
                PbMap *m = vm_new_map(vm, 8);
                map_set(m, vm_const_str(vm, "type"), val_obj(vm_const_str(vm, ev.type == ZR_X_KeyPress ? "keydown" : "keyup")));
                map_set(m, vm_const_str(vm, "key"), val_obj(vm_new_cstr(vm, kname)));
                map_set(m, vm_const_str(vm, "shift"), val_bool((ev.xkey.state & 1u) != 0));
                map_set(m, vm_const_str(vm, "ctrl"), val_bool((ev.xkey.state & 4u) != 0));
                map_set(m, vm_const_str(vm, "alt"), val_bool((ev.xkey.state & 8u) != 0));
                arr_push(events, val_obj(m));
            }
        }
    }
#endif
    vm_temp_pop(vm, 1);
    *out = val_obj(events);
    return true;
}

static const ZrGlyphCacheEntry *zr_get_or_raster_glyph(uint32_t cp, int px, const char *font_path, bool *was_hit) {
    uint32_t fhash = zr_fnv1a(font_path, strlen(font_path));
    uint32_t slot_h = (cp * 2654435761u) ^ ((uint32_t)px * 40503u) ^ fhash;
    size_t idx = (size_t)(slot_h % ZR_GLYPH_CACHE_CAP);
    ZrGlyphCacheEntry *ce = &g_zr_glyph_cache[idx];
    if (ce->used && ce->codepoint == cp && ce->px == px && ce->font_hash == fhash) {
        *was_hit = true;
        g_zr_glyph_cache_hits++;
        return ce;
    }
    *was_hit = false;
    g_zr_glyph_cache_loads++;
    free(ce->alpha);
    memset(ce, 0, sizeof(*ce));
    ce->used = true;
    ce->codepoint = cp;
    ce->px = px;
    ce->font_hash = fhash;

#if defined(_WIN32)
    /* Native Win32 GDI Unicode anti-aliased glyph rasterization (`Segoe UI`) */
    int bw = px * 2 + 8, bh = px * 2 + 8;
    HDC hdc = CreateCompatibleDC(NULL);
    BITMAPINFO bmi;
    memset(&bmi, 0, sizeof(bmi));
    bmi.bmiHeader.biSize = sizeof(BITMAPINFOHEADER);
    bmi.bmiHeader.biWidth = bw;
    bmi.bmiHeader.biHeight = -bh;
    bmi.bmiHeader.biPlanes = 1;
    bmi.bmiHeader.biBitCount = 32;
    bmi.bmiHeader.biCompression = BI_RGB;
    uint32_t *dib = NULL;
    HBITMAP hbm = CreateDIBSection(hdc, &bmi, DIB_RGB_COLORS, (void **)&dib, NULL, 0);
    if (hbm && dib) {
        HGDIOBJ old_bm = SelectObject(hdc, hbm);
        HFONT hfont = CreateFontW(-px, 0, 0, 0, FW_NORMAL, FALSE, FALSE, FALSE,
                                  DEFAULT_CHARSET, OUT_TT_PRECIS, CLIP_DEFAULT_PRECIS,
                                  ANTIALIASED_QUALITY, DEFAULT_PITCH | FF_DONTCARE, L"Segoe UI");
        HGDIOBJ old_f = SelectObject(hdc, hfont);
        SetBkColor(hdc, RGB(0, 0, 0));
        SetTextColor(hdc, RGB(255, 255, 255));
        wchar_t wch[3] = {0};
        int wlen = 1;
        if (cp <= 0xFFFF) { wch[0] = (wchar_t)cp; }
        else {
            uint32_t u = cp - 0x10000;
            wch[0] = (wchar_t)(0xD800 + (u >> 10));
            wch[1] = (wchar_t)(0xDC00 + (u & 0x3FF));
            wlen = 2;
        }
        SIZE sz = {0, 0};
        GetTextExtentPoint32W(hdc, wch, wlen, &sz);
        TextOutW(hdc, 0, 0, wch, wlen);
        int gw = sz.cx > 0 && sz.cx < bw ? (int)sz.cx : px;
        int gh = sz.cy > 0 && sz.cy < bh ? (int)sz.cy : px;
        ce->w = gw; ce->h = gh; ce->left = 0; ce->top = (px * 8) / 10; ce->advance = gw;
        ce->alpha = (uint8_t *)zr_xmalloc((size_t)(gw * gh + 1));
        for (int y = 0; y < gh; y++) {
            for (int x = 0; x < gw; x++) {
                uint32_t p = dib[y * bw + x];
                ce->alpha[y * gw + x] = (uint8_t)(((p & 0xffu) + ((p >> 8) & 0xffu) + ((p >> 16) & 0xffu)) / 3u);
            }
        }
        SelectObject(hdc, old_f); DeleteObject(hfont);
        SelectObject(hdc, old_bm); DeleteObject(hbm);
        DeleteDC(hdc);
        return ce;
    }
    if (hbm) DeleteObject(hbm);
    DeleteDC(hdc);
#else
    if (font_path && font_path[0] && zr_ensure_freetype()) {
        if (!g_zr_ft.cached_face || strcmp(g_zr_ft.cached_path, font_path) != 0) {
            if (g_zr_ft.cached_face && g_zr_ft.FT_Done_Face) g_zr_ft.FT_Done_Face(g_zr_ft.cached_face);
            g_zr_ft.cached_face = NULL;
            if (g_zr_ft.FT_New_Face(g_zr_ft.ft_lib, font_path, 0, &g_zr_ft.cached_face) == 0 && g_zr_ft.cached_face) {
                snprintf(g_zr_ft.cached_path, sizeof(g_zr_ft.cached_path), "%s", font_path);
                g_zr_font_cache_loads++;
            }
        } else {
            g_zr_font_cache_hits++;
        }
        if (g_zr_ft.cached_face) {
            g_zr_ft.FT_Set_Pixel_Sizes(g_zr_ft.cached_face, 0, (unsigned int)px);
            if (g_zr_ft.FT_Load_Char(g_zr_ft.cached_face, (unsigned long)cp, ZR_FT_LOAD_RENDER) == 0 &&
                g_zr_ft.cached_face->glyph) {
                ZrFtGlyphSlotRec *gs = g_zr_ft.cached_face->glyph;
                int gw = gs->bitmap.width > 0 ? gs->bitmap.width : 0;
                int gh = gs->bitmap.rows > 0 ? gs->bitmap.rows : 0;
                ce->w = gw;
                ce->h = gh;
                ce->left = gs->bitmap_left;
                ce->top = gs->bitmap_top;
                ce->advance = (int)(gs->advance.x >> 6);
                if (ce->advance <= 0) ce->advance = gw > 0 ? gw + 1 : (px / 3);
                ce->alpha = (uint8_t *)zr_xcalloc((size_t)(gw * gh + 1), 1);
                if (gw > 0 && gh > 0 && gs->bitmap.buffer) {
                    int pitch = gs->bitmap.pitch;
                    for (int y = 0; y < gh; y++) {
                        const uint8_t *srow = gs->bitmap.buffer + (pitch >= 0 ? y * pitch : (gh - 1 - y) * (-pitch));
                        memcpy(ce->alpha + y * gw, srow, (size_t)gw);
                    }
                }
                return ce;
            }
        }
    }
#endif
    zr_synth_procedural_glyph(cp, px, &ce->w, &ce->h, &ce->left, &ce->top, &ce->advance, &ce->alpha);
    return ce;
}

/* gfxdrawtextaa(handle, x, y, text, color, px, font_path, advance) */
static bool bi_gfxdrawtextaa(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    if (argc != 8 || argv[1].tag != VAL_NUM || argv[2].tag != VAL_NUM ||
        !val_is_obj(argv[3], OBJ_STR) || argv[5].tag != VAL_NUM ||
        !val_is_obj(argv[6], OBJ_STR) || argv[7].tag != VAL_NUM) {
        vm_raise(vm, "gfxdrawtextaa expects 8 arguments (handle, x, y, text, color, px, font_path, advance)");
        return false;
    }
    ZrGfxWin *gw = zr_gfx_get_win(vm, argv[0], "gfxdrawtextaa");
    if (!gw) return false;
    uint32_t color = 0;
    if (!zr_gfx_parse_color(vm, argv[4], "gfxdrawtextaa", &color)) return false;

    int64_t base_x = (int64_t)argv[1].u.num;
    int64_t base_y = (int64_t)argv[2].u.num;
    PbStr  *text   = (PbStr *)val_as_obj(argv[3]);
    int     px     = (int)(argv[5].u.num < 6.0 ? 6.0 : (argv[5].u.num > 256.0 ? 256.0 : argv[5].u.num));
    const char *req_font = ((PbStr *)val_as_obj(argv[6]))->chars;
    double  extra_adv    = argv[7].u.num;

    const char *resolved_font = zr_resolve_font_file(req_font);
    int baseline_y = (int)base_y + (px * 8) / 10;
    double pen_x = (double)base_x;
    size_t glyphs = 0, hits = 0, loads = 0;

    size_t pos = 0;
    while (pos < text->len) {
        size_t uadv = 1;
        uint32_t cp = zr_utf8_decode(text->chars + pos, text->len - pos, &uadv);
        pos += uadv;
        if (cp == '\n') {
            pen_x = (double)base_x;
            baseline_y += px + (px / 4);
            continue;
        }
        bool was_hit = false;
        const ZrGlyphCacheEntry *ge = zr_get_or_raster_glyph(cp, px, resolved_font, &was_hit);
        if (was_hit) hits++; else loads++;
        glyphs++;

        int gx0 = (int)llround(pen_x) + ge->left;
        int gy0 = baseline_y - ge->top;
        for (int row = 0; row < ge->h; row++) {
            int py = gy0 + row;
            if (py < 0 || py >= (int)gw->h) continue;
            for (int col = 0; col < ge->w; col++) {
                int px_c = gx0 + col;
                if (px_c < 0 || px_c >= (int)gw->w) continue;
                uint8_t cov = ge->alpha[row * ge->w + col];
                if (cov > 0) {
                    zr_blend_argb(&gw->buf[(size_t)py * gw->w + (size_t)px_c], color, cov);
                }
            }
        }
        pen_x += (double)ge->advance + extra_adv;
    }

    double total_w = pen_x > (double)base_x ? (pen_x - (double)base_x) : 0.0;
    PbMap *m = vm_new_map(vm, 16);
    vm_temp_push(vm, val_obj(m));
    map_set(m, vm_const_str(vm, "format"), val_obj(vm_const_str(vm, "pblang.graphics.text.draw.v1")));
    map_set(m, vm_const_str(vm, "ok"), val_bool(true));
#if defined(_WIN32)
    map_set(m, vm_const_str(vm, "renderer"), val_obj(vm_const_str(vm, "native-aa-gdi")));
#else
    map_set(m, vm_const_str(vm, "renderer"), val_obj(vm_const_str(vm, g_zr_ft.ready ? "native-aa-freetype2" : "native-aa-procedural")));
#endif
    map_set(m, vm_const_str(vm, "font"), val_obj(vm_new_cstr(vm, resolved_font)));
    map_set(m, vm_const_str(vm, "width"), val_num(total_w));
    map_set(m, vm_const_str(vm, "height"), val_num((double)px));
    map_set(m, vm_const_str(vm, "glyphs"), val_num((double)glyphs));
    map_set(m, vm_const_str(vm, "font_cache_hit"), val_bool(g_zr_font_cache_hits > 0));
    map_set(m, vm_const_str(vm, "glyph_cache_hits"), val_num((double)hits));
    map_set(m, vm_const_str(vm, "glyph_cache_loads"), val_num((double)loads));
    map_set(m, vm_const_str(vm, "error"), val_obj(vm_const_str(vm, "")));
    vm_temp_pop(vm, 1);
    *out = val_obj(m);
    return true;
}

static bool bi_gfxtextprofile(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    (void)argv;
    if (argc != 0) { vm_raise(vm, "gfxtextprofile expects 0 arguments"); return false; }
    const char *f = zr_resolve_font_file("");
#if !defined(_WIN32)
    zr_ensure_freetype();
#endif
    PbMap *m = vm_new_map(vm, 16);
    vm_temp_push(vm, val_obj(m));
    map_set(m, vm_const_str(vm, "format"), val_obj(vm_const_str(vm, "pblang.graphics.text.profile.v1")));
    map_set(m, vm_const_str(vm, "aa_available"), val_bool(true));
#if defined(_WIN32)
    map_set(m, vm_const_str(vm, "renderer"), val_obj(vm_const_str(vm, "native-aa-gdi")));
#else
    map_set(m, vm_const_str(vm, "renderer"), val_obj(vm_const_str(vm, g_zr_ft.ready ? "native-aa-freetype2" : "native-aa-procedural")));
#endif
    map_set(m, vm_const_str(vm, "default_font"), val_obj(vm_new_cstr(vm, f)));
    map_set(m, vm_const_str(vm, "greek_utf8"), val_bool(true));
    map_set(m, vm_const_str(vm, "latin_utf8"), val_bool(true));
    map_set(m, vm_const_str(vm, "font_cache_hits"), val_num((double)g_zr_font_cache_hits));
    map_set(m, vm_const_str(vm, "font_cache_loads"), val_num((double)g_zr_font_cache_loads));
    map_set(m, vm_const_str(vm, "glyph_cache_hits"), val_num((double)g_zr_glyph_cache_hits));
    map_set(m, vm_const_str(vm, "glyph_cache_loads"), val_num((double)g_zr_glyph_cache_loads));
    vm_temp_pop(vm, 1);
    *out = val_obj(m);
    return true;
}

static bool bi_gfxbackendprofile(PbVM *vm, int argc, PbValue *argv, PbValue *out) {
    (void)argv;
    if (argc != 0) { vm_raise(vm, "gfxbackendprofile expects 0 arguments"); return false; }
    const char *hl = getenv("PBLANG_GFX_HEADLESS");
    bool is_hl = (hl && strcmp(hl, "1") == 0);
    PbMap *m = vm_new_map(vm, 16);
    vm_temp_push(vm, val_obj(m));
    map_set(m, vm_const_str(vm, "format"), val_obj(vm_const_str(vm, "pblang.graphics.backend.profile.v1")));
#if defined(_WIN32)
    map_set(m, vm_const_str(vm, "backend"), val_obj(vm_const_str(vm, is_hl ? "headless-fb" : "win32-gdi")));
#else
    map_set(m, vm_const_str(vm, "backend"), val_obj(vm_const_str(vm, is_hl ? "headless-fb" : "x11-native")));
#endif
    map_set(m, vm_const_str(vm, "present"), val_obj(vm_const_str(vm, "cpu-framebuffer")));
    map_set(m, vm_const_str(vm, "present_copy"), val_obj(vm_const_str(vm, "native-framebuffer")));
    map_set(m, vm_const_str(vm, "event_pump"), val_obj(vm_const_str(vm, "native-window-poll")));
    map_set(m, vm_const_str(vm, "rect_fill"), val_obj(vm_const_str(vm, "row-slice-fill")));
    map_set(m, vm_const_str(vm, "gpu_present"), val_bool(false));
    map_set(m, vm_const_str(vm, "compiled_sdl2"), val_bool(false));
    map_set(m, vm_const_str(vm, "cpu_raster_ui"), val_bool(true));
    map_set(m, vm_const_str(vm, "text_supported"), val_bool(true));
    map_set(m, vm_const_str(vm, "max_framebuffer_dim"), val_num((double)ZR_MAX_FB_DIM));
    map_set(m, vm_const_str(vm, "max_framebuffer_pixels"), val_num((double)ZR_MAX_FB_PIXELS));
    map_set(m, vm_const_str(vm, "event_contract"), val_obj(vm_const_str(vm, "pblang.graphics.events.v1")));
    map_set(m, vm_const_str(vm, "resize_events"), val_bool(true));
    map_set(m, vm_const_str(vm, "mouse_xy_events"), val_bool(true));
    map_set(m, vm_const_str(vm, "scroll_events"), val_bool(true));
    vm_temp_pop(vm, 1);
    *out = val_obj(m);
    return true;
}

#endif /* PBLANG_ZR_GFX_H */
