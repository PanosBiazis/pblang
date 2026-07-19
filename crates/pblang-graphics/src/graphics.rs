//! Native graphics builtins: minifb framebuffer backend + keyboard/mouse events.
//!
//! Every public `gfx*` function in this module is registered as a pblang
//! builtin by [`gfx_builtins`]. The `GFX_WINDOWS` thread-local owns the
//! native window handles; each pblang-visible *handle* is a small
//! integer (the `GFX_NEXT` counter) so pblang code never sees a raw
//! pointer. The `usize` is round-tripped through [`Value::Number`].
//!
//! Window state — back-buffer, last-known mouse position, button state
//! — lives in the private `GfxWindow` struct. `gfx_present` blits the
//! buffer to the window; `gfx_poll_events` snapshots the input state
//! into a pblang map. When the `sdl2` feature is on and
//! `PBLANG_GFX=sdl2` is set, each builtin delegates to the
//! `sdl2_backend` module instead.

use minifb::{Key, KeyRepeat, MouseButton, MouseMode, Window, WindowOptions};
use pblang_ir::{alloc_string, RuntimeError, Value, VmHost};
use std::cell::RefCell;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};

const FALLBACK_FONT_PATHS: &[&str] = &[
    "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
    "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
    "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
    "/usr/share/fonts/truetype/liberation2/LiberationSans-Regular.ttf",
    "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
    "/usr/share/fonts/truetype/freefont/FreeSans.ttf",
    "/usr/share/fonts/truetype/ubuntu/Ubuntu-R.ttf",
    "/usr/share/fonts/opentype/cantarell/Cantarell-Regular.otf",
    "/usr/share/fonts/truetype/inter/Inter-Regular.ttf",
    "/System/Library/Fonts/SFNS.ttf",
    "/System/Library/Fonts/Helvetica.ttc",
    "/System/Library/Fonts/Arial.ttf",
    "C:\\Windows\\Fonts\\segoeui.ttf",
    "C:\\Windows\\Fonts\\calibri.ttf",
    "C:\\Windows\\Fonts\\arial.ttf",
    "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
    "/usr/share/fonts/truetype/noto/NotoSansMono-Regular.ttf",
    "/usr/share/fonts/truetype/liberation/LiberationMono-Regular.ttf",
    "/usr/share/fonts/truetype/ubuntu/UbuntuMono[wght].ttf",
    "C:\\Windows\\Fonts\\consola.ttf",
    "/System/Library/Fonts/Menlo.ttc",
    "/System/Library/Fonts/SFNSMono.ttf",
];

thread_local! {
    static GFX_WINDOWS: RefCell<HashMap<usize, GfxWindow>> = RefCell::new(HashMap::new());
    static TEXT_FONT_PATH_CACHE: RefCell<HashMap<String, Option<String>>> = RefCell::new(HashMap::new());
    static TEXT_FONT_CACHE: RefCell<HashMap<String, Rc<fontdue::Font>>> = RefCell::new(HashMap::new());
    static TEXT_GLYPH_CACHE: RefCell<HashMap<String, CachedGlyph>> = RefCell::new(HashMap::new());
}
static GFX_NEXT: AtomicUsize = AtomicUsize::new(1);
static TEXT_FONT_CACHE_HITS: AtomicUsize = AtomicUsize::new(0);
static TEXT_FONT_CACHE_LOADS: AtomicUsize = AtomicUsize::new(0);
static TEXT_GLYPH_CACHE_HITS: AtomicUsize = AtomicUsize::new(0);
static TEXT_GLYPH_CACHE_LOADS: AtomicUsize = AtomicUsize::new(0);

struct GfxWindow {
    win: Window,
    w: usize,
    h: usize,
    buf: Vec<u32>, // ARGB
    last_mouse: Option<(f32, f32)>,
    last_left: bool,
    last_right: bool,
}

const MAX_FRAMEBUFFER_DIM: usize = 8192;
const MAX_FRAMEBUFFER_PIXELS: usize = MAX_FRAMEBUFFER_DIM * MAX_FRAMEBUFFER_DIM;

fn sync_window_buffer(gw: &mut GfxWindow) -> Option<(usize, usize)> {
    let (cw, ch) = gw.win.get_size();
    if cw == 0 || ch == 0 {
        return None;
    }
    if cw > MAX_FRAMEBUFFER_DIM || ch > MAX_FRAMEBUFFER_DIM {
        return None;
    }
    let pixels = cw.checked_mul(ch)?;
    if pixels > MAX_FRAMEBUFFER_PIXELS {
        return None;
    }
    if cw != gw.w || ch != gw.h {
        gw.w = cw;
        gw.h = ch;
        gw.buf.resize(pixels, 0);
        return Some((cw, ch));
    }
    None
}

#[derive(Debug, Clone)]
struct CachedGlyph {
    metrics: fontdue::Metrics,
    bitmap: Rc<Vec<u8>>,
}

fn handle_from_value(host: &mut dyn VmHost, v: Value, what: &str) -> Result<usize, RuntimeError> {
    match v {
        Value::Number(n) if n.is_finite() && n.fract() == 0.0 && n >= 1.0 => Ok(n as usize),
        _ => Err(host.err(format!("{what}: expected handle number"))),
    }
}

fn number(host: &mut dyn VmHost, v: &Value, what: &str) -> Result<f64, RuntimeError> {
    match v {
        Value::Number(n) => Ok(*n),
        _ => Err(host.err(format!("{what}: expected number"))),
    }
}

fn string_val(host: &mut dyn VmHost, v: &Value, what: &str) -> Result<String, RuntimeError> {
    v.as_str()
        .map_err(|e| host.err(format!("{what}: {e}")))
        .map(|s| s.to_string())
}

fn color_from_value(host: &mut dyn VmHost, v: &Value, what: &str) -> Result<u32, RuntimeError> {
    let arr = match v {
        Value::Array(a) => a.borrow(),
        _ => return Err(host.err(format!("{what}: expected color array [r,g,b,(a)]"))),
    };
    if arr.len() != 3 && arr.len() != 4 {
        return Err(host.err(format!("{what}: color must have 3 or 4 components")));
    }
    let comp = |i: usize, name: &str| -> Result<u8, RuntimeError> {
        match arr[i] {
            Value::Number(n) if n.is_finite() && (0.0..=255.0).contains(&n) => Ok(n as u8),
            _ => Err(host.err(format!("{what}: {name} must be 0..255"))),
        }
    };
    let r = comp(0, "r")?;
    let g = comp(1, "g")?;
    let b = comp(2, "b")?;
    let a = if arr.len() == 4 { comp(3, "a")? } else { 255 };
    Ok(((a as u32) << 24) | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32))
}

fn rect_from_value(
    host: &mut dyn VmHost,
    v: &Value,
    what: &str,
) -> Result<Option<(i64, i64, i64, i64)>, RuntimeError> {
    let m = match v {
        Value::Map(m) => m.borrow(),
        _ => return Err(host.err(format!("{what}: expected rect map {{x,y,w,h}}"))),
    };
    let mut get = |name: &str| -> Result<i64, RuntimeError> {
        let val = m
            .get(name)
            .ok_or_else(|| host.err(format!("{what}: rect missing {name}")))?;
        Ok(number(host, val, &format!("{what} {name}"))? as i64)
    };
    let x = get("x")?;
    let y = get("y")?;
    let w = get("w")?;
    let h = get("h")?;
    if w <= 0 || h <= 0 {
        return Ok(None);
    }
    Ok(Some((x, y, w, h)))
}

fn clipped_rect_bounds(
    w: usize,
    h: usize,
    x: i64,
    y: i64,
    rw: i64,
    rh: i64,
) -> Option<(usize, usize, usize, usize)> {
    if w == 0 || h == 0 || rw <= 0 || rh <= 0 {
        return None;
    }
    let fw = w as i64;
    let fh = h as i64;
    let x0 = x.clamp(0, fw);
    let y0 = y.clamp(0, fh);
    let x1 = x.saturating_add(rw).clamp(0, fw);
    let y1 = y.saturating_add(rh).clamp(0, fh);
    if x1 <= x0 || y1 <= y0 {
        return None;
    }
    Some((x0 as usize, y0 as usize, x1 as usize, y1 as usize))
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn fill_rect_into(
    buf: &mut [u32],
    w: usize,
    h: usize,
    x: i64,
    y: i64,
    rw: i64,
    rh: i64,
    color: u32,
) {
    if let Some((x0, y0, x1, y1)) = clipped_rect_bounds(w, h, x, y, rw, rh) {
        for yy in y0..y1 {
            let row = yy * w;
            buf[row + x0..row + x1].fill(color);
        }
    }
}

fn text_font_path_uncached(requested: &str) -> Option<String> {
    let requested = requested.trim();
    if !requested.is_empty() && Path::new(requested).is_file() {
        return Some(requested.to_string());
    }
    for env_name in ["PBIDE_FONT", "PBLANG_FONT"] {
        if let Ok(v) = std::env::var(env_name) {
            let v = v.trim();
            if !v.is_empty() && Path::new(v).is_file() {
                return Some(v.to_string());
            }
        }
    }
    FALLBACK_FONT_PATHS
        .iter()
        .find(|p| Path::new(p).is_file())
        .map(|p| (*p).to_string())
}

fn text_font_path(requested: &str) -> Option<String> {
    let key = requested.trim().to_string();
    if let Some(cached) = TEXT_FONT_PATH_CACHE.with(|cache| cache.borrow().get(&key).cloned()) {
        return cached;
    }
    let resolved = text_font_path_uncached(&key);
    TEXT_FONT_PATH_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if cache.len() > 32 {
            cache.clear();
        }
        cache.insert(key, resolved.clone());
    });
    resolved
}

fn text_font_cache_size() -> usize {
    TEXT_FONT_CACHE.with(|cache| cache.borrow().len())
}

fn text_glyph_cache_size() -> usize {
    TEXT_GLYPH_CACHE.with(|cache| cache.borrow().len())
}

fn text_profile_map(font_path: Option<String>) -> Value {
    let mut m = HashMap::new();
    m.insert(
        "format".into(),
        alloc_string("pblang.graphics.text.profile.v1"),
    );
    m.insert("renderer".into(), alloc_string("native-aa-fontdue"));
    m.insert("fallback".into(), alloc_string("bitmap-font"));
    m.insert("antialias".into(), Value::Bool(true));
    m.insert(
        "font_cache_size".into(),
        Value::Number(text_font_cache_size() as f64),
    );
    m.insert(
        "font_cache_hits".into(),
        Value::Number(TEXT_FONT_CACHE_HITS.load(Ordering::Relaxed) as f64),
    );
    m.insert(
        "font_cache_loads".into(),
        Value::Number(TEXT_FONT_CACHE_LOADS.load(Ordering::Relaxed) as f64),
    );
    m.insert(
        "glyph_cache_size".into(),
        Value::Number(text_glyph_cache_size() as f64),
    );
    m.insert(
        "glyph_cache_hits".into(),
        Value::Number(TEXT_GLYPH_CACHE_HITS.load(Ordering::Relaxed) as f64),
    );
    m.insert(
        "glyph_cache_loads".into(),
        Value::Number(TEXT_GLYPH_CACHE_LOADS.load(Ordering::Relaxed) as f64),
    );
    match font_path {
        Some(p) => {
            m.insert("available".into(), Value::Bool(true));
            m.insert("font".into(), alloc_string(p));
        }
        None => {
            m.insert("available".into(), Value::Bool(false));
            m.insert("font".into(), alloc_string(""));
        }
    }
    alloc_map(m)
}

fn load_text_font(requested_font_path: &str) -> Result<(String, Rc<fontdue::Font>, bool), String> {
    let font_path = text_font_path(requested_font_path)
        .ok_or_else(|| "no usable TrueType/OpenType font found".to_string())?;
    if let Some(font) = TEXT_FONT_CACHE.with(|cache| cache.borrow().get(&font_path).cloned()) {
        TEXT_FONT_CACHE_HITS.fetch_add(1, Ordering::Relaxed);
        return Ok((font_path, font, true));
    }
    let font_bytes = fs::read(&font_path).map_err(|e| format!("read font {font_path}: {e}"))?;
    let font = Rc::new(
        fontdue::Font::from_bytes(font_bytes, fontdue::FontSettings::default())
            .map_err(|e| format!("load font {font_path}: {e}"))?,
    );
    TEXT_FONT_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if cache.len() >= 8 {
            cache.clear();
        }
        cache.insert(font_path.clone(), Rc::clone(&font));
    });
    TEXT_FONT_CACHE_LOADS.fetch_add(1, Ordering::Relaxed);
    Ok((font_path, font, false))
}

fn glyph_cache_key(font_path: &str, ch: char, px: f32) -> String {
    format!("{font_path}\u{1f}{:.2}\u{1f}{:x}", px, ch as u32)
}

fn rasterize_cached_glyph(font_path: &str, font: &fontdue::Font, ch: char, px: f32) -> CachedGlyph {
    let key = glyph_cache_key(font_path, ch, px);
    if let Some(cached) = TEXT_GLYPH_CACHE.with(|cache| cache.borrow().get(&key).cloned()) {
        TEXT_GLYPH_CACHE_HITS.fetch_add(1, Ordering::Relaxed);
        return cached;
    }
    let (metrics, bitmap) = font.rasterize(ch, px);
    let cached = CachedGlyph {
        metrics,
        bitmap: Rc::new(bitmap),
    };
    TEXT_GLYPH_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if cache.len() >= 4096 {
            cache.clear();
        }
        cache.insert(key, cached.clone());
    });
    TEXT_GLYPH_CACHE_LOADS.fetch_add(1, Ordering::Relaxed);
    cached
}

/// One rasterized glyph destined for a GPU atlas: the resolved font path plus the R8 coverage
/// bitmap and the metrics the GPU text layout needs. Reuses the same font + glyph caches as the
/// CPU `draw_text_aa_into` path, so GPU and CPU text rasterize identically. Lives here (not in
/// `wgpu_backend`) so it compiles with or without the `wgpu-render` feature.
#[allow(dead_code)]
pub(crate) struct GlyphRaster {
    pub font_path: String,
    pub width: usize,
    pub height: usize,
    pub xmin: i32,
    pub advance_width: f32,
    pub bitmap: Rc<Vec<u8>>,
}

/// Resolve `requested_font_path` and rasterize `ch` at `px`, returning the coverage bitmap +
/// metrics for atlas upload. `px` is clamped to the same 6..256 range as the CPU path.
#[allow(dead_code)]
pub(crate) fn rasterize_glyph_for_atlas(
    requested_font_path: &str,
    ch: char,
    px: f32,
) -> Result<GlyphRaster, String> {
    let px = px.clamp(6.0, 256.0);
    let (font_path, font, _hit) = load_text_font(requested_font_path)?;
    let cached = rasterize_cached_glyph(&font_path, font.as_ref(), ch, px);
    Ok(GlyphRaster {
        font_path,
        width: cached.metrics.width,
        height: cached.metrics.height,
        xmin: cached.metrics.xmin,
        advance_width: cached.metrics.advance_width,
        bitmap: cached.bitmap,
    })
}

fn blend_argb(dst: u32, src: u32, coverage: u8) -> u32 {
    if coverage == 0 {
        return dst;
    }
    let src_a = (((src >> 24) & 0xff) * coverage as u32) / 255;
    if src_a == 0 {
        return dst;
    }
    let inv = 255 - src_a;
    let sr = (src >> 16) & 0xff;
    let sg = (src >> 8) & 0xff;
    let sb = src & 0xff;
    let da = (dst >> 24) & 0xff;
    let dr = (dst >> 16) & 0xff;
    let dg = (dst >> 8) & 0xff;
    let db = dst & 0xff;
    let out_a = (src_a + (da * inv) / 255).min(255);
    let out_r = ((sr * src_a + dr * inv) / 255).min(255);
    let out_g = ((sg * src_a + dg * inv) / 255).min(255);
    let out_b = ((sb * src_a + db * inv) / 255).min(255);
    (out_a << 24) | (out_r << 16) | (out_g << 8) | out_b
}

#[derive(Debug, Clone)]
pub(crate) struct TextDrawStats {
    pub width: i64,
    pub height: i64,
    pub glyphs: usize,
    pub font_path: String,
    pub font_cache_hit: bool,
    pub glyph_cache_hits: usize,
    pub glyph_cache_loads: usize,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_text_aa_into(
    buf: &mut [u32],
    w: usize,
    h: usize,
    x: i64,
    y: i64,
    text: &str,
    color: u32,
    px: f32,
    requested_font_path: &str,
    fixed_advance: f32,
) -> Result<TextDrawStats, String> {
    if w == 0 || h == 0 || text.is_empty() {
        return Ok(TextDrawStats {
            width: 0,
            height: 0,
            glyphs: 0,
            font_path: String::new(),
            font_cache_hit: false,
            glyph_cache_hits: 0,
            glyph_cache_loads: 0,
        });
    }
    let px = px.clamp(6.0, 256.0);
    let (font_path, font, font_cache_hit) = load_text_font(requested_font_path)?;
    let glyph_hits_before = TEXT_GLYPH_CACHE_HITS.load(Ordering::Relaxed);
    let glyph_loads_before = TEXT_GLYPH_CACHE_LOADS.load(Ordering::Relaxed);
    let mut cursor = x as f32;
    let mut glyphs = 0usize;
    let mut max_right = x;
    let mut max_bottom = y;
    for ch in text.chars() {
        if ch == '\n' || ch == '\r' {
            continue;
        }
        let cached = rasterize_cached_glyph(&font_path, font.as_ref(), ch, px);
        let metrics = cached.metrics;
        let bitmap = cached.bitmap;
        if metrics.width > 0 && metrics.height > 0 {
            let gx = cursor.round() as i64 + metrics.xmin as i64;
            // PBIDE uses top-left cell coordinates. Keep glyphs vertically centered in
            // the existing cell instead of changing line layout or cursor hit-testing.
            let gy = y + ((px as i64 - metrics.height as i64) / 2).max(0);
            for row in 0..metrics.height {
                let py = gy + row as i64;
                if py < 0 || py >= h as i64 {
                    continue;
                }
                let src_row = row * metrics.width;
                let dst_row = py as usize * w;
                for col in 0..metrics.width {
                    let px0 = gx + col as i64;
                    if px0 < 0 || px0 >= w as i64 {
                        continue;
                    }
                    let coverage = bitmap[src_row + col];
                    let idx = dst_row + px0 as usize;
                    buf[idx] = blend_argb(buf[idx], color, coverage);
                }
            }
            max_right = max_right.max(gx + metrics.width as i64);
            max_bottom = max_bottom.max(gy + metrics.height as i64);
            glyphs += 1;
        }
        let adv = if fixed_advance > 0.0 {
            fixed_advance
        } else {
            metrics.advance_width.max(px * 0.5)
        };
        cursor += adv;
    }
    Ok(TextDrawStats {
        width: (cursor.round() as i64 - x).max(max_right - x),
        height: (max_bottom - y).max(px as i64),
        glyphs,
        font_path,
        font_cache_hit,
        glyph_cache_hits: TEXT_GLYPH_CACHE_HITS
            .load(Ordering::Relaxed)
            .saturating_sub(glyph_hits_before),
        glyph_cache_loads: TEXT_GLYPH_CACHE_LOADS
            .load(Ordering::Relaxed)
            .saturating_sub(glyph_loads_before),
    })
}

fn with_window_mut<F, R>(
    host: &mut dyn VmHost,
    id: usize,
    what: &str,
    f: F,
) -> Result<R, RuntimeError>
where
    F: FnOnce(&mut GfxWindow) -> Result<R, RuntimeError>,
{
    GFX_WINDOWS.with(|tbl| {
        let mut t = tbl.borrow_mut();
        let w = t
            .get_mut(&id)
            .ok_or_else(|| host.err(format!("{what}: invalid handle")))?;
        f(w)
    })
}

fn insert_key_modifiers(m: &mut HashMap<String, Value>, win: &Window) {
    let shift = win.is_key_down(Key::LeftShift) || win.is_key_down(Key::RightShift);
    let ctrl = win.is_key_down(Key::LeftCtrl) || win.is_key_down(Key::RightCtrl);
    let alt = win.is_key_down(Key::LeftAlt) || win.is_key_down(Key::RightAlt);
    let meta = win.is_key_down(Key::LeftSuper) || win.is_key_down(Key::RightSuper);
    m.insert("shift".into(), Value::Bool(shift));
    m.insert("ctrl".into(), Value::Bool(ctrl));
    m.insert("alt".into(), Value::Bool(alt));
    m.insert("meta".into(), Value::Bool(meta));
}

fn alloc_array(items: Vec<Value>) -> Value {
    Value::Array(Rc::new(RefCell::new(items)))
}

fn alloc_map(m: HashMap<String, Value>) -> Value {
    Value::Map(Rc::new(RefCell::new(m)))
}

// --- gfxcreatewindow(width, height, title) -> handle
pub fn gfxcreatewindow(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 3 {
        return Err(host.err(format!(
            "gfxcreatewindow expects 3 arguments, got {}",
            args.len()
        )));
    }
    let w = number(host, &args[0], "gfxcreatewindow width")? as i64;
    let h = number(host, &args[1], "gfxcreatewindow height")? as i64;
    if w <= 0 || h <= 0 || w > 8192 || h > 8192 {
        return Err(host.err("gfxcreatewindow: invalid size".into()));
    }
    let title = string_val(host, &args[2], "gfxcreatewindow title")?;

    #[cfg(feature = "wgpu-render")]
    if crate::wgpu_backend::use_wgpu() {
        let id = GFX_NEXT.fetch_add(1, Ordering::Relaxed);
        let win = crate::wgpu_backend::create(host, w as u32, h as u32, &title)?;
        crate::wgpu_backend::insert(id, win);
        return Ok(Value::Number(id as f64));
    }

    #[cfg(feature = "sdl2")]
    if crate::sdl2_backend::use_sdl2() {
        let id = GFX_NEXT.fetch_add(1, Ordering::Relaxed);
        let win = crate::sdl2_backend::create(host, w as u32, h as u32, &title)?;
        crate::sdl2_backend::insert(id, win);
        return Ok(Value::Number(id as f64));
    }

    let opts = WindowOptions {
        borderless: false,
        title: true,
        resize: true,
        ..WindowOptions::default()
    };
    let mut win = Window::new(&title, w as usize, h as usize, opts)
        .map_err(|e| host.err(format!("gfxcreatewindow: {e}")))?;
    win.set_target_fps(60);
    win.set_background_color(0, 0, 0);

    let id = GFX_NEXT.fetch_add(1, Ordering::Relaxed);
    let gw = GfxWindow {
        win,
        w: w as usize,
        h: h as usize,
        buf: vec![0u32; (w as usize) * (h as usize)],
        last_mouse: None,
        last_left: false,
        last_right: false,
    };

    GFX_WINDOWS.with(|tbl| tbl.borrow_mut().insert(id, gw));
    Ok(Value::Number(id as f64))
}

// --- gfxisopen(handle) -> bool
pub fn gfxisopen(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 1 {
        return Err(host.err(format!("gfxisopen expects 1 argument, got {}", args.len())));
    }
    let id = handle_from_value(host, args[0].clone(), "gfxisopen")?;
    #[cfg(feature = "wgpu-render")]
    if crate::wgpu_backend::has(id) {
        return crate::wgpu_backend::with_window_mut(host, id, "gfxisopen", |w| {
            Ok(Value::Bool(w.is_open()))
        });
    }
    #[cfg(feature = "sdl2")]
    if crate::sdl2_backend::has(id) {
        return crate::sdl2_backend::with_window_mut(host, id, "gfxisopen", |w| {
            Ok(Value::Bool(w.is_open()))
        });
    }
    let open = with_window_mut(host, id, "gfxisopen", |gw| Ok(gw.win.is_open()))?;
    Ok(Value::Bool(open))
}

// --- gfxclose(handle)
pub fn gfxclose(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 1 {
        return Err(host.err(format!("gfxclose expects 1 argument, got {}", args.len())));
    }
    let id = handle_from_value(host, args[0].clone(), "gfxclose")?;
    #[cfg(feature = "wgpu-render")]
    if crate::wgpu_backend::has(id) {
        crate::wgpu_backend::remove(id);
        return Ok(Value::Nil);
    }
    #[cfg(feature = "sdl2")]
    if crate::sdl2_backend::has(id) {
        crate::sdl2_backend::remove(id);
        return Ok(Value::Nil);
    }
    GFX_WINDOWS.with(|tbl| {
        tbl.borrow_mut().remove(&id);
    });
    Ok(Value::Nil)
}

// --- gfxsize(handle) -> map { "w": number, "h": number }
pub fn gfxsize(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 1 {
        return Err(host.err(format!("gfxsize expects 1 argument, got {}", args.len())));
    }
    let id = handle_from_value(host, args[0].clone(), "gfxsize")?;
    #[cfg(feature = "wgpu-render")]
    if crate::wgpu_backend::has(id) {
        return crate::wgpu_backend::with_window_mut(host, id, "gfxsize", |w| {
            let (ww, hh) = w.size();
            let mut m = HashMap::new();
            m.insert("w".into(), Value::Number(ww as f64));
            m.insert("h".into(), Value::Number(hh as f64));
            Ok(alloc_map(m))
        });
    }
    #[cfg(feature = "sdl2")]
    if crate::sdl2_backend::has(id) {
        return crate::sdl2_backend::with_window_mut(host, id, "gfxsize", |w| {
            let (ww, hh) = w.size();
            let mut m = HashMap::new();
            m.insert("w".into(), Value::Number(ww as f64));
            m.insert("h".into(), Value::Number(hh as f64));
            Ok(alloc_map(m))
        });
    }
    with_window_mut(host, id, "gfxsize", |gw| {
        // Report the LIVE window size (and keep the framebuffer matched), so callers see the
        // current size after a resize rather than the size the window was created at.
        let _ = sync_window_buffer(gw);
        let mut m = HashMap::new();
        m.insert("w".into(), Value::Number(gw.w as f64));
        m.insert("h".into(), Value::Number(gw.h as f64));
        Ok(alloc_map(m))
    })
}

// --- gfxclear(handle, color)
pub fn gfxclear(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 2 {
        return Err(host.err(format!("gfxclear expects 2 arguments, got {}", args.len())));
    }
    let id = handle_from_value(host, args[0].clone(), "gfxclear")?;
    let c = color_from_value(host, &args[1], "gfxclear")?;
    #[cfg(feature = "wgpu-render")]
    if crate::wgpu_backend::has(id) {
        return crate::wgpu_backend::with_window_mut(host, id, "gfxclear", |w| {
            w.clear(c);
            Ok(Value::Nil)
        });
    }
    #[cfg(feature = "sdl2")]
    if crate::sdl2_backend::has(id) {
        return crate::sdl2_backend::with_window_mut(host, id, "gfxclear", |w| {
            w.clear(c);
            Ok(Value::Nil)
        });
    }
    with_window_mut(host, id, "gfxclear", |gw| {
        gw.buf.fill(c);
        Ok(Value::Nil)
    })
}

// --- gfxdrawrect(handle, x, y, w, h, color)
pub fn gfxdrawrect(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 6 {
        return Err(host.err(format!(
            "gfxdrawrect expects 6 arguments, got {}",
            args.len()
        )));
    }
    let id = handle_from_value(host, args[0].clone(), "gfxdrawrect")?;
    let x = number(host, &args[1], "gfxdrawrect x")? as i64;
    let y = number(host, &args[2], "gfxdrawrect y")? as i64;
    let rw = number(host, &args[3], "gfxdrawrect w")? as i64;
    let rh = number(host, &args[4], "gfxdrawrect h")? as i64;
    let c = color_from_value(host, &args[5], "gfxdrawrect")?;

    if rw <= 0 || rh <= 0 {
        return Ok(Value::Nil);
    }

    #[cfg(feature = "wgpu-render")]
    if crate::wgpu_backend::has(id) {
        return crate::wgpu_backend::with_window_mut(host, id, "gfxdrawrect", |w| {
            w.draw_rect(x, y, rw, rh, c);
            Ok(Value::Nil)
        });
    }
    #[cfg(feature = "sdl2")]
    if crate::sdl2_backend::has(id) {
        return crate::sdl2_backend::with_window_mut(host, id, "gfxdrawrect", |w| {
            w.draw_rect(x, y, rw, rh, c);
            Ok(Value::Nil)
        });
    }
    with_window_mut(host, id, "gfxdrawrect", |gw| {
        fill_rect_into(&mut gw.buf, gw.w, gw.h, x, y, rw, rh, c);
        Ok(Value::Nil)
    })
}

// --- gfxdrawrects(handle, rects, color)
pub fn gfxdrawrects(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 3 {
        return Err(host.err(format!(
            "gfxdrawrects expects 3 arguments, got {}",
            args.len()
        )));
    }
    let id = handle_from_value(host, args[0].clone(), "gfxdrawrects")?;
    let rects_value = match &args[1] {
        Value::Array(a) => a.borrow(),
        _ => return Err(host.err("gfxdrawrects rects: expected array of rect maps".to_string())),
    };
    if rects_value.len() > 100_000 {
        return Err(host.err("gfxdrawrects rects: too many rects (max 100000)".to_string()));
    }
    let c = color_from_value(host, &args[2], "gfxdrawrects")?;
    let mut rects = Vec::with_capacity(rects_value.len().min(4096));
    for v in rects_value.iter() {
        if let Some(rect) = rect_from_value(host, v, "gfxdrawrects rect")? {
            rects.push(rect);
        }
    }
    if rects.is_empty() {
        return Ok(Value::Nil);
    }

    #[cfg(feature = "wgpu-render")]
    if crate::wgpu_backend::has(id) {
        return crate::wgpu_backend::with_window_mut(host, id, "gfxdrawrects", |w| {
            w.draw_rects(&rects, c);
            Ok(Value::Nil)
        });
    }
    #[cfg(feature = "sdl2")]
    if crate::sdl2_backend::has(id) {
        return crate::sdl2_backend::with_window_mut(host, id, "gfxdrawrects", |w| {
            w.draw_rects(&rects, c);
            Ok(Value::Nil)
        });
    }
    with_window_mut(host, id, "gfxdrawrects", |gw| {
        for (x, y, rw, rh) in rects {
            fill_rect_into(&mut gw.buf, gw.w, gw.h, x, y, rw, rh, c);
        }
        Ok(Value::Nil)
    })
}

// --- gfxdrawpoint(handle, x, y, color)
pub fn gfxdrawpoint(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 4 {
        return Err(host.err(format!(
            "gfxdrawpoint expects 4 arguments, got {}",
            args.len()
        )));
    }
    let id = handle_from_value(host, args[0].clone(), "gfxdrawpoint")?;
    let x = number(host, &args[1], "gfxdrawpoint x")? as i64;
    let y = number(host, &args[2], "gfxdrawpoint y")? as i64;
    let c = color_from_value(host, &args[3], "gfxdrawpoint")?;

    #[cfg(feature = "wgpu-render")]
    if crate::wgpu_backend::has(id) {
        return crate::wgpu_backend::with_window_mut(host, id, "gfxdrawpoint", |w| {
            w.draw_rect(x, y, 1, 1, c);
            Ok(Value::Nil)
        });
    }
    #[cfg(feature = "sdl2")]
    if crate::sdl2_backend::has(id) {
        return crate::sdl2_backend::with_window_mut(host, id, "gfxdrawpoint", |w| {
            w.draw_rect(x, y, 1, 1, c);
            Ok(Value::Nil)
        });
    }
    with_window_mut(host, id, "gfxdrawpoint", |gw| {
        // An off-screen point is a no-op. The previous code CLAMPED to the nearest
        // edge, which silently painted a phantom pixel on the border.
        if x >= 0 && y >= 0 && x < gw.w as i64 && y < gw.h as i64 {
            gw.buf[y as usize * gw.w + x as usize] = c;
        }
        Ok(Value::Nil)
    })
}

// --- gfxdrawline(handle, x1, y1, x2, y2, color) — Bresenham line
pub fn gfxdrawline(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 6 {
        return Err(host.err(format!(
            "gfxdrawline expects 6 arguments, got {}",
            args.len()
        )));
    }
    let id = handle_from_value(host, args[0].clone(), "gfxdrawline")?;
    let x1 = number(host, &args[1], "gfxdrawline x1")? as i64;
    let y1 = number(host, &args[2], "gfxdrawline y1")? as i64;
    let x2 = number(host, &args[3], "gfxdrawline x2")? as i64;
    let y2 = number(host, &args[4], "gfxdrawline y2")? as i64;
    let c = color_from_value(host, &args[5], "gfxdrawline")?;

    #[cfg(feature = "wgpu-render")]
    if crate::wgpu_backend::has(id) {
        return crate::wgpu_backend::with_window_mut(host, id, "gfxdrawline", |w| {
            w.draw_line(x1, y1, x2, y2, c);
            Ok(Value::Nil)
        });
    }
    #[cfg(feature = "sdl2")]
    if crate::sdl2_backend::has(id) {
        return crate::sdl2_backend::with_window_mut(host, id, "gfxdrawline", |w| {
            w.draw_line(x1, y1, x2, y2, c);
            Ok(Value::Nil)
        });
    }
    with_window_mut(host, id, "gfxdrawline", |gw| {
        draw_line_bresenham(
            &mut gw.buf,
            gw.w,
            gw.h,
            LineParams {
                x1,
                y1,
                x2,
                y2,
                color: c,
            },
        );
        Ok(Value::Nil)
    })
}

/// Parameters for Bresenham line drawing — groups coordinates to stay under the argument limit.
struct LineParams {
    x1: i64,
    y1: i64,
    x2: i64,
    y2: i64,
    color: u32,
}

/// Cohen–Sutherland: clip the segment to the inclusive pixel rect
/// `[0, w-1] × [0, h-1]`. Returns the clipped integer endpoints, or `None` if
/// the segment is entirely outside the viewport.
///
/// This is what makes line drawing safe against huge user coordinates: without
/// it, `draw_line_bresenham` iterates `max(|dx|,|dy|)` times, so a line to a
/// 10^12 endpoint hangs the program. Clipping first bounds the loop to the
/// framebuffer while preserving the line's slope (so partially-visible lines
/// still render correctly).
pub(crate) fn clip_line(
    x1: i64,
    y1: i64,
    x2: i64,
    y2: i64,
    w: usize,
    h: usize,
) -> Option<(i64, i64, i64, i64)> {
    if w == 0 || h == 0 {
        return None;
    }
    const INSIDE: u8 = 0;
    const LEFT: u8 = 1;
    const RIGHT: u8 = 2;
    const BOTTOM: u8 = 4;
    const TOP: u8 = 8;
    let (xmax, ymax) = ((w - 1) as f64, (h - 1) as f64);
    let code = |x: f64, y: f64| -> u8 {
        let mut c = INSIDE;
        if x < 0.0 {
            c |= LEFT;
        } else if x > xmax {
            c |= RIGHT;
        }
        if y < 0.0 {
            c |= BOTTOM;
        } else if y > ymax {
            c |= TOP;
        }
        c
    };
    let (mut ax, mut ay, mut bx, mut by) = (x1 as f64, y1 as f64, x2 as f64, y2 as f64);
    let mut ca = code(ax, ay);
    let mut cb = code(bx, by);
    loop {
        if ca | cb == 0 {
            return Some((
                ax.round() as i64,
                ay.round() as i64,
                bx.round() as i64,
                by.round() as i64,
            ));
        }
        if ca & cb != 0 {
            return None; // both share an outside half-plane → fully outside
        }
        // Clip the endpoint that is outside. The chosen edge guarantees a
        // non-degenerate denominator (a horizontal line never selects TOP/BOTTOM,
        // a vertical line never selects LEFT/RIGHT — such lines share the bit on
        // both ends and are rejected by the `ca & cb` test above).
        let out = if ca != 0 { ca } else { cb };
        let (nx, ny) = if out & TOP != 0 {
            (ax + (bx - ax) * (ymax - ay) / (by - ay), ymax)
        } else if out & BOTTOM != 0 {
            (ax + (bx - ax) * (0.0 - ay) / (by - ay), 0.0)
        } else if out & RIGHT != 0 {
            (xmax, ay + (by - ay) * (xmax - ax) / (bx - ax))
        } else {
            (0.0, ay + (by - ay) * (0.0 - ax) / (bx - ax))
        };
        if out == ca {
            ax = nx;
            ay = ny;
            ca = code(ax, ay);
        } else {
            bx = nx;
            by = ny;
            cb = code(bx, by);
        }
    }
}

fn draw_line_bresenham(buf: &mut [u32], w: usize, h: usize, params: LineParams) {
    let color = params.color;
    // Clip to the viewport first so the loop is bounded by the framebuffer, not
    // by the (possibly enormous) user-supplied coordinates.
    let (x1, y1, x2, y2) = match clip_line(params.x1, params.y1, params.x2, params.y2, w, h) {
        Some(c) => c,
        None => return,
    };
    let mut x = x1;
    let mut y = y1;
    let dx = (x2 - x1).abs();
    let dy = (y2 - y1).abs();
    let sx = if x1 < x2 { 1 } else { -1 };
    let sy = if y1 < y2 { 1 } else { -1 };
    let mut err = dx - dy;
    loop {
        if x >= 0 && x < w as i64 && y >= 0 && y < h as i64 {
            buf[y as usize * w + x as usize] = color;
        }
        if x == x2 && y == y2 {
            break;
        }
        let e2 = 2 * err;
        if e2 > -dy {
            err -= dy;
            x += sx;
        }
        if e2 < dx {
            err += dx;
            y += sy;
        }
    }
}

// --- gfxdrawtextaa(handle, x, y, text, color, px, font_path, advance)
pub fn gfxdrawtextaa(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 8 {
        return Err(host.err(format!(
            "gfxdrawtextaa expects 8 arguments, got {}",
            args.len()
        )));
    }
    let id = handle_from_value(host, args[0].clone(), "gfxdrawtextaa")?;
    let x = number(host, &args[1], "gfxdrawtextaa x")? as i64;
    let y = number(host, &args[2], "gfxdrawtextaa y")? as i64;
    let text = string_val(host, &args[3], "gfxdrawtextaa text")?;
    let c = color_from_value(host, &args[4], "gfxdrawtextaa")?;
    let px = number(host, &args[5], "gfxdrawtextaa px")? as f32;
    let font_path = string_val(host, &args[6], "gfxdrawtextaa font_path")?;
    let advance = number(host, &args[7], "gfxdrawtextaa advance")? as f32;

    #[cfg(feature = "wgpu-render")]
    if crate::wgpu_backend::has(id) {
        // M2: rasterize glyphs into the wgpu R8 atlas and queue one instance per glyph. Cache
        // deltas are read off the shared glyph-cache counters (the atlas reuses the same
        // fontdue rasterization as the CPU path, so these are accurate).
        let hits_before = TEXT_GLYPH_CACHE_HITS.load(Ordering::Relaxed);
        let loads_before = TEXT_GLYPH_CACHE_LOADS.load(Ordering::Relaxed);
        return crate::wgpu_backend::with_window_mut(host, id, "gfxdrawtextaa", |w| {
            let (glyphs, width, height, resolved_font) =
                w.draw_text(x, y, &text, c, px, &font_path, advance);
            let stats = TextDrawStats {
                width,
                height,
                glyphs,
                font_path: resolved_font,
                font_cache_hit: false,
                glyph_cache_hits: TEXT_GLYPH_CACHE_HITS
                    .load(Ordering::Relaxed)
                    .saturating_sub(hits_before),
                glyph_cache_loads: TEXT_GLYPH_CACHE_LOADS
                    .load(Ordering::Relaxed)
                    .saturating_sub(loads_before),
            };
            Ok(text_draw_result(Ok(stats)))
        });
    }
    #[cfg(feature = "sdl2")]
    if crate::sdl2_backend::has(id) {
        return crate::sdl2_backend::with_window_mut(host, id, "gfxdrawtextaa", |w| {
            let stats = w.draw_text_aa(x, y, &text, c, px, &font_path, advance);
            Ok(text_draw_result(stats))
        });
    }
    with_window_mut(host, id, "gfxdrawtextaa", |gw| {
        let stats = draw_text_aa_into(
            &mut gw.buf,
            gw.w,
            gw.h,
            x,
            y,
            &text,
            c,
            px,
            &font_path,
            advance,
        );
        Ok(text_draw_result(stats))
    })
}

fn text_draw_result(stats: Result<TextDrawStats, String>) -> Value {
    let mut m = HashMap::new();
    m.insert(
        "format".into(),
        alloc_string("pblang.graphics.text.draw.v1"),
    );
    match stats {
        Ok(s) => {
            m.insert("ok".into(), Value::Bool(true));
            m.insert("renderer".into(), alloc_string("native-aa-fontdue"));
            m.insert("font".into(), alloc_string(s.font_path));
            m.insert("width".into(), Value::Number(s.width as f64));
            m.insert("height".into(), Value::Number(s.height as f64));
            m.insert("glyphs".into(), Value::Number(s.glyphs as f64));
            m.insert("font_cache_hit".into(), Value::Bool(s.font_cache_hit));
            m.insert(
                "glyph_cache_hits".into(),
                Value::Number(s.glyph_cache_hits as f64),
            );
            m.insert(
                "glyph_cache_loads".into(),
                Value::Number(s.glyph_cache_loads as f64),
            );
            m.insert("error".into(), alloc_string(""));
        }
        Err(e) => {
            m.insert("ok".into(), Value::Bool(false));
            m.insert("renderer".into(), alloc_string("bitmap-font-fallback"));
            m.insert("font".into(), alloc_string(""));
            m.insert("width".into(), Value::Number(0.0));
            m.insert("height".into(), Value::Number(0.0));
            m.insert("glyphs".into(), Value::Number(0.0));
            m.insert("font_cache_hit".into(), Value::Bool(false));
            m.insert("glyph_cache_hits".into(), Value::Number(0.0));
            m.insert("glyph_cache_loads".into(), Value::Number(0.0));
            m.insert("error".into(), alloc_string(e));
        }
    }
    alloc_map(m)
}

// --- gfxtextprofile() -> map
pub fn gfxtextprofile(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if !args.is_empty() {
        return Err(host.err(format!(
            "gfxtextprofile expects 0 arguments, got {}",
            args.len()
        )));
    }
    Ok(text_profile_map(text_font_path("")))
}

// --- gfxbackendprofile() -> map
pub fn gfxbackendprofile(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if !args.is_empty() {
        return Err(host.err(format!(
            "gfxbackendprofile expects 0 arguments, got {}",
            args.len()
        )));
    }

    let requested = std::env::var("PBLANG_GFX").unwrap_or_default();
    let software_forced = std::env::var("PBLANG_GFX_SOFTWARE")
        .map(|v| v == "1")
        .unwrap_or(false);
    let compiled_sdl2 = cfg!(feature = "sdl2");
    let selected_sdl2 = compiled_sdl2 && requested == "sdl2";

    // Experimental wgpu render backend introspection (only meaningful when the feature is
    // compiled in; the `use_wgpu()` env gate accepts PBIDE_GRAPHICS=wgpu or PBLANG_GFX=wgpu).
    #[cfg(feature = "wgpu-render")]
    if crate::wgpu_backend::use_wgpu() {
        let mut m = HashMap::new();
        m.insert(
            "format".into(),
            alloc_string("pblang.graphics.backend.profile.v1"),
        );
        m.insert("requested".into(), alloc_string("wgpu"));
        m.insert("backend".into(), alloc_string("wgpu"));
        m.insert("present".into(), alloc_string("gpu-surface-present"));
        m.insert("present_copy".into(), alloc_string("swapchain"));
        m.insert("event_pump".into(), alloc_string("native-window-poll"));
        m.insert("rect_fill".into(), alloc_string("instanced-quad"));
        m.insert("gpu_present".into(), Value::Bool(true));
        m.insert("compiled_sdl2".into(), Value::Bool(compiled_sdl2));
        m.insert("compiled_wgpu".into(), Value::Bool(true));
        m.insert("software_forced".into(), Value::Bool(false));
        m.insert("cpu_raster_ui".into(), Value::Bool(false));
        m.insert("text_supported".into(), Value::Bool(true));
        m.insert(
            "note".into(),
            alloc_string(
                "EXPERIMENTAL wgpu render (M1: surface+clear+rects+lines; M2: glyph-atlas text). GPU-untested.",
            ),
        );
        return Ok(alloc_map(m));
    }

    let (backend, present, gpu_present, note) = if selected_sdl2 && software_forced {
        (
            "sdl2",
            "sdl2-software-present",
            false,
            "SDL2 compiled and requested, but PBLANG_GFX_SOFTWARE=1 forces software canvas",
        )
    } else if selected_sdl2 {
        (
            "sdl2",
            "gpu-assisted-texture-present",
            true,
            "SDL2 compiled and requested; pblang still CPU-rasterizes UI into a texture",
        )
    } else if requested == "sdl2" {
        (
            "minifb",
            "cpu-framebuffer",
            false,
            "PBLANG_GFX=sdl2 requested, but this binary was not built with pblang-graphics/sdl2",
        )
    } else {
        (
            "minifb",
            "cpu-framebuffer",
            false,
            "default native framebuffer; pure CPU present path",
        )
    };

    let mut m = HashMap::new();
    m.insert(
        "format".into(),
        alloc_string("pblang.graphics.backend.profile.v1"),
    );
    m.insert("requested".into(), alloc_string(requested));
    m.insert("backend".into(), alloc_string(backend));
    m.insert("present".into(), alloc_string(present));
    m.insert(
        "present_copy".into(),
        alloc_string(if backend == "sdl2" {
            "row-copy-argb8888"
        } else {
            "native-framebuffer"
        }),
    );
    m.insert(
        "event_pump".into(),
        alloc_string(if backend == "sdl2" {
            "cached-window-event-pump"
        } else {
            "native-window-poll"
        }),
    );
    m.insert("rect_fill".into(), alloc_string("row-slice-fill"));
    m.insert("gpu_present".into(), Value::Bool(gpu_present));
    m.insert("compiled_sdl2".into(), Value::Bool(compiled_sdl2));
    m.insert("software_forced".into(), Value::Bool(software_forced));
    m.insert("cpu_raster_ui".into(), Value::Bool(true));
    m.insert("idle_present_skip".into(), Value::Bool(true));
    m.insert("present_resize_sync".into(), Value::Bool(true));
    m.insert(
        "max_framebuffer_dim".into(),
        Value::Number(MAX_FRAMEBUFFER_DIM as f64),
    );
    m.insert(
        "max_framebuffer_pixels".into(),
        Value::Number(MAX_FRAMEBUFFER_PIXELS as f64),
    );
    m.insert(
        "event_contract".into(),
        alloc_string("pblang.graphics.events.v1"),
    );
    m.insert("resize_events".into(), Value::Bool(true));
    m.insert("mouse_xy_events".into(), Value::Bool(true));
    m.insert("scroll_events".into(), Value::Bool(true));
    m.insert("scroll_event".into(), alloc_string("scroll"));
    m.insert("note".into(), alloc_string(note));
    Ok(alloc_map(m))
}

// --- gfxpresent(handle)
pub fn gfxpresent(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 1 {
        return Err(host.err(format!("gfxpresent expects 1 argument, got {}", args.len())));
    }
    let id = handle_from_value(host, args[0].clone(), "gfxpresent")?;
    #[cfg(feature = "wgpu-render")]
    if crate::wgpu_backend::has(id) {
        return crate::wgpu_backend::with_window_mut(host, id, "gfxpresent", |w| {
            w.present()?;
            Ok(Value::Nil)
        });
    }
    #[cfg(feature = "sdl2")]
    if crate::sdl2_backend::has(id) {
        return crate::sdl2_backend::with_window_mut(host, id, "gfxpresent", |w| {
            w.present()?;
            Ok(Value::Nil)
        });
    }
    with_window_mut(host, id, "gfxpresent", |gw| {
        let _ = sync_window_buffer(gw);
        gw.win
            .update_with_buffer(&gw.buf, gw.w, gw.h)
            .map_err(|e| RuntimeError {
                message: format!("gfxpresent: {e}"),
                span: None,
                source_name: None,
                source: None,
                stack: vec![],
            })?;
        Ok(Value::Nil)
    })
}

// --- gfxpollevents(handle) -> array of event maps
pub fn gfxpollevents(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 1 {
        return Err(host.err(format!(
            "gfxpollevents expects 1 argument, got {}",
            args.len()
        )));
    }
    let id = handle_from_value(host, args[0].clone(), "gfxpollevents")?;

    #[cfg(feature = "wgpu-render")]
    if crate::wgpu_backend::has(id) {
        return crate::wgpu_backend::with_window_mut(host, id, "gfxpollevents", |w| {
            w.poll_events(alloc_array, alloc_map)
        });
    }
    #[cfg(feature = "sdl2")]
    if crate::sdl2_backend::has(id) {
        return crate::sdl2_backend::with_window_mut(host, id, "gfxpollevents", |w| {
            w.poll_events(alloc_array, alloc_map)
        });
    }
    with_window_mut(host, id, "gfxpollevents", |gw| {
        let mut out: Vec<Value> = Vec::new();

        if !gw.win.is_open() {
            let mut m = HashMap::new();
            m.insert("type".into(), alloc_string("close"));
            out.push(alloc_map(m));
            return Ok(alloc_array(out));
        }

        // RESIZABLE: grow/shrink the framebuffer to match the live window so the app draws at
        // the real resolution and REFLOWS, instead of minifb stretching a fixed buffer (blurry,
        // never relayouts). Emit a "resize" event so the app can recompute its layout. After
        // this, buffer == window so the mouse maps 1:1.
        if let Some((cur_w, cur_h)) = sync_window_buffer(gw) {
            let mut m = HashMap::new();
            m.insert("type".into(), alloc_string("resize"));
            m.insert("w".into(), Value::Number(cur_w as f64));
            m.insert("h".into(), Value::Number(cur_h as f64));
            out.push(alloc_map(m));
        }

        // Mouse is in window space; with the buffer kept equal to the window above these are
        // 1.0 (retained for the rare frame where the sizes momentarily diverge).
        let (win_w, win_h) = gw.win.get_size();
        let msx = if win_w > 0 {
            gw.w as f32 / win_w as f32
        } else {
            1.0
        };
        let msy = if win_h > 0 {
            gw.h as f32 / win_h as f32
        } else {
            1.0
        };

        // Key down. KeyRepeat::Yes so a HELD key (arrows, backspace, a letter) auto-repeats
        // at the OS rate — continuous cursor movement / typing while held.
        let keys = gw.win.get_keys_pressed(KeyRepeat::Yes);
        for k in keys {
            let mut m = HashMap::new();
            m.insert("type".into(), alloc_string("keydown"));
            m.insert("key".into(), alloc_string(format!("{k:?}")));
            insert_key_modifiers(&mut m, &gw.win);
            out.push(alloc_map(m));
        }

        // Key up
        let keys_up = gw.win.get_keys_released();
        for k in keys_up {
            let mut m = HashMap::new();
            m.insert("type".into(), alloc_string("keyup"));
            m.insert("key".into(), alloc_string(format!("{k:?}")));
            insert_key_modifiers(&mut m, &gw.win);
            out.push(alloc_map(m));
        }

        // Mouse move
        if let Some((rmx, rmy)) = gw.win.get_mouse_pos(MouseMode::Discard) {
            let mx = rmx * msx;
            let my = rmy * msy;
            let changed = match gw.last_mouse {
                Some((px, py)) => (px != mx) || (py != my),
                None => true,
            };
            if changed {
                gw.last_mouse = Some((mx, my));
                let mut m = HashMap::new();
                m.insert("type".into(), alloc_string("mousemove"));
                m.insert("x".into(), Value::Number(mx as f64));
                m.insert("y".into(), Value::Number(my as f64));
                out.push(alloc_map(m));
            }
        }

        // Mouse buttons (edge detection). Attach the cursor position to every
        // button event: a click must know WHERE it happened, and the separate
        // `mousemove` only fires when the position changes (and is suppressed when
        // the cursor strays outside the client area), so consumers can't reliably
        // reconstruct the click point from a tracked mousemove. `Clamp` keeps a
        // top/edge click inside the window instead of dropping it to None.
        let left = gw.win.get_mouse_down(MouseButton::Left);
        let right = gw.win.get_mouse_down(MouseButton::Right);
        let (rbx, rby) = gw.win.get_mouse_pos(MouseMode::Clamp).unwrap_or((0.0, 0.0));
        let (bx, by) = (rbx * msx, rby * msy);

        if left && !gw.last_left {
            let mut m = HashMap::new();
            m.insert("type".into(), alloc_string("mousedown"));
            m.insert("button".into(), alloc_string("Left"));
            m.insert("x".into(), Value::Number(bx as f64));
            m.insert("y".into(), Value::Number(by as f64));
            out.push(alloc_map(m));
        }
        if !left && gw.last_left {
            let mut m = HashMap::new();
            m.insert("type".into(), alloc_string("mouseup"));
            m.insert("button".into(), alloc_string("Left"));
            m.insert("x".into(), Value::Number(bx as f64));
            m.insert("y".into(), Value::Number(by as f64));
            out.push(alloc_map(m));
        }
        if right && !gw.last_right {
            let mut m = HashMap::new();
            m.insert("type".into(), alloc_string("mousedown"));
            m.insert("button".into(), alloc_string("Right"));
            m.insert("x".into(), Value::Number(bx as f64));
            m.insert("y".into(), Value::Number(by as f64));
            out.push(alloc_map(m));
        }
        if !right && gw.last_right {
            let mut m = HashMap::new();
            m.insert("type".into(), alloc_string("mouseup"));
            m.insert("button".into(), alloc_string("Right"));
            m.insert("x".into(), Value::Number(bx as f64));
            m.insert("y".into(), Value::Number(by as f64));
            out.push(alloc_map(m));
        }

        gw.last_left = left;
        gw.last_right = right;

        // Scroll wheel (dx, dy; e.g. trackpad or mouse wheel)
        if let Some((sx, sy)) = gw.win.get_scroll_wheel() {
            if sx != 0.0 || sy != 0.0 {
                let mut m = HashMap::new();
                m.insert("type".into(), alloc_string("scroll"));
                m.insert("dx".into(), Value::Number(sx as f64));
                m.insert("dy".into(), Value::Number(sy as f64));
                out.push(alloc_map(m));
            }
        }

        Ok(alloc_array(out))
    })
}

// Backward-compat aliases (old stdlib used these names)
pub fn gfxwindowopen(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    gfxisopen(host, args)
}
pub fn gfxclosewindow(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    gfxclose(host, args)
}

/// Returns all gfx builtins for VM registration.
pub fn gfx_builtins() -> Vec<pblang_ir::BuiltinFn> {
    vec![
        pblang_ir::BuiltinFn {
            name: "gfxcreatewindow",
            arity: Some(3),
            fun: gfxcreatewindow,
        },
        pblang_ir::BuiltinFn {
            name: "gfxisopen",
            arity: Some(1),
            fun: gfxisopen,
        },
        pblang_ir::BuiltinFn {
            name: "gfxclose",
            arity: Some(1),
            fun: gfxclose,
        },
        pblang_ir::BuiltinFn {
            name: "gfxsize",
            arity: Some(1),
            fun: gfxsize,
        },
        pblang_ir::BuiltinFn {
            name: "gfxclear",
            arity: Some(2),
            fun: gfxclear,
        },
        pblang_ir::BuiltinFn {
            name: "gfxdrawrect",
            arity: Some(6),
            fun: gfxdrawrect,
        },
        pblang_ir::BuiltinFn {
            name: "gfxdrawrects",
            arity: Some(3),
            fun: gfxdrawrects,
        },
        pblang_ir::BuiltinFn {
            name: "gfxdrawpoint",
            arity: Some(4),
            fun: gfxdrawpoint,
        },
        pblang_ir::BuiltinFn {
            name: "gfxdrawline",
            arity: Some(6),
            fun: gfxdrawline,
        },
        pblang_ir::BuiltinFn {
            name: "gfxdrawtextaa",
            arity: Some(8),
            fun: gfxdrawtextaa,
        },
        pblang_ir::BuiltinFn {
            name: "gfxtextprofile",
            arity: Some(0),
            fun: gfxtextprofile,
        },
        pblang_ir::BuiltinFn {
            name: "gfxbackendprofile",
            arity: Some(0),
            fun: gfxbackendprofile,
        },
        pblang_ir::BuiltinFn {
            name: "gfxpresent",
            arity: Some(1),
            fun: gfxpresent,
        },
        pblang_ir::BuiltinFn {
            name: "gfxpollevents",
            arity: Some(1),
            fun: gfxpollevents,
        },
        pblang_ir::BuiltinFn {
            name: "gfxwindowopen",
            arity: Some(1),
            fun: gfxwindowopen,
        },
        pblang_ir::BuiltinFn {
            name: "gfxclosewindow",
            arity: Some(1),
            fun: gfxclosewindow,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clip_line_keeps_fully_inside_segment() {
        assert_eq!(clip_line(2, 3, 5, 6, 10, 10), Some((2, 3, 5, 6)));
    }

    #[test]
    fn clip_line_rejects_fully_outside_segment() {
        assert_eq!(clip_line(-5, -5, -3, -3, 10, 10), None);
        assert_eq!(clip_line(20, 20, 30, 30, 10, 10), None);
    }

    #[test]
    fn clip_line_clips_horizontal_crossing() {
        // y=5 line spanning far left..far right clips to the pixel row [0..9].
        assert_eq!(clip_line(-1000, 5, 1000, 5, 10, 10), Some((0, 5, 9, 5)));
    }

    #[test]
    fn clip_line_bounds_huge_endpoint() {
        // The DoS case: a line to a 10^12 endpoint must clip to the viewport so
        // the Bresenham loop is bounded by the framebuffer, not the coordinate.
        let (cx1, cy1, cx2, cy2) =
            clip_line(0, 0, 1_000_000_000_000, 0, 10, 10).expect("crosses viewport");
        assert!((0..10).contains(&cx1) && (0..10).contains(&cx2));
        assert!((0..10).contains(&cy1) && (0..10).contains(&cy2));
    }

    #[test]
    fn draw_line_with_huge_endpoint_terminates_and_draws_visible_pixels() {
        // Without viewport clipping this loops ~10^12 times (an effective hang).
        // It must complete and color the on-screen run.
        let (w, h) = (16usize, 16usize);
        let mut buf = vec![0u32; w * h];
        draw_line_bresenham(
            &mut buf,
            w,
            h,
            LineParams {
                x1: 0,
                y1: 0,
                x2: 1_000_000_000_000,
                y2: 0,
                color: 0xABCDEF,
            },
        );
        // Row 0 should be fully colored; no write landed out of bounds (the test
        // completing without panic/hang is itself the assertion).
        for x in 0..w {
            assert_eq!(buf[x], 0xABCDEF, "pixel ({x},0) should be drawn");
        }
    }

    #[test]
    fn fill_rect_into_clips_and_fills_by_rows() {
        let (w, h) = (4usize, 4usize);
        let mut buf = vec![0u32; w * h];
        fill_rect_into(&mut buf, w, h, -1, -1, 3, 3, 0xCAFE_BABE);
        assert_eq!(buf[0], 0xCAFE_BABE);
        assert_eq!(buf[1], 0xCAFE_BABE);
        assert_eq!(buf[4], 0xCAFE_BABE);
        assert_eq!(buf[5], 0xCAFE_BABE);
        assert_eq!(buf[2], 0);
        assert_eq!(buf[8], 0);
    }

    #[test]
    fn fill_rect_into_ignores_offscreen_and_overflowing_rects() {
        let (w, h) = (4usize, 4usize);
        let mut buf = vec![0x11u32; w * h];
        fill_rect_into(&mut buf, w, h, i64::MAX - 2, 0, 16, 2, 0x22);
        fill_rect_into(&mut buf, w, h, 0, i64::MAX - 2, 2, 16, 0x22);
        fill_rect_into(&mut buf, w, h, 1, 1, 0, 8, 0x22);
        assert!(buf.iter().all(|&p| p == 0x11));
    }
}
