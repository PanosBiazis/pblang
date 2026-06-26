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
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};

thread_local! {
    static GFX_WINDOWS: RefCell<HashMap<usize, GfxWindow>> = RefCell::new(HashMap::new());
}
static GFX_NEXT: AtomicUsize = AtomicUsize::new(1);

struct GfxWindow {
    win: Window,
    w: usize,
    h: usize,
    buf: Vec<u32>, // ARGB
    last_mouse: Option<(f32, f32)>,
    last_left: bool,
    last_right: bool,
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

    #[cfg(feature = "sdl2")]
    if crate::sdl2_backend::has(id) {
        return crate::sdl2_backend::with_window_mut(host, id, "gfxdrawrect", |w| {
            w.draw_rect(x, y, rw, rh, c);
            Ok(Value::Nil)
        });
    }
    with_window_mut(host, id, "gfxdrawrect", |gw| {
        let x0 = x.max(0) as usize;
        let y0 = y.max(0) as usize;
        let x1 = (x + rw).min(gw.w as i64).max(0) as usize;
        let y1 = (y + rh).min(gw.h as i64).max(0) as usize;

        for yy in y0..y1 {
            let row = yy * gw.w;
            for xx in x0..x1 {
                gw.buf[row + xx] = c;
            }
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

// --- gfxpresent(handle)
pub fn gfxpresent(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 1 {
        return Err(host.err(format!("gfxpresent expects 1 argument, got {}", args.len())));
    }
    let id = handle_from_value(host, args[0].clone(), "gfxpresent")?;
    #[cfg(feature = "sdl2")]
    if crate::sdl2_backend::has(id) {
        return crate::sdl2_backend::with_window_mut(host, id, "gfxpresent", |w| {
            w.present()?;
            Ok(Value::Nil)
        });
    }
    with_window_mut(host, id, "gfxpresent", |gw| {
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

        // The framebuffer is a FIXED size (gw.w × gw.h) that minifb stretches to fill the
        // window; on HiDPI / when the WM resizes the window to fit the screen, the window
        // size diverges from the buffer, so the raw mouse position (which is in window
        // space) no longer matches what we drew. Map mouse → buffer space by the
        // buffer/window ratio. When window == buffer (the common case) this is a no-op.
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
            LineParams { x1: 0, y1: 0, x2: 1_000_000_000_000, y2: 0, color: 0xABCDEF },
        );
        // Row 0 should be fully colored; no write landed out of bounds (the test
        // completing without panic/hang is itself the assertion).
        for x in 0..w {
            assert_eq!(buf[x], 0xABCDEF, "pixel ({x},0) should be drawn");
        }
    }
}
