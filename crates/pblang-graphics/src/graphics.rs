//! Native graphics builtins: minifb backend with framebuffer, events (keyboard/mouse).

use pblang_ir::{alloc_string, RuntimeError, Value, VmHost};
use minifb::{KeyRepeat, MouseButton, MouseMode, Window, WindowOptions};
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
    win.limit_update_rate(Some(std::time::Duration::from_micros(16_000))); // ~60 fps
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
        return Err(host.err(format!(
            "gfxisopen expects 1 argument, got {}",
            args.len()
        )));
    }
    let id = handle_from_value(host, args[0].clone(), "gfxisopen")?;
    #[cfg(feature = "sdl2")]
    if crate::sdl2_backend::has(id) {
        return crate::sdl2_backend::with_window_mut(host, id, "gfxisopen", |w| Ok(Value::Bool(w.is_open())));
    }
    let open = with_window_mut(host, id, "gfxisopen", |gw| Ok(gw.win.is_open()))?;
    Ok(Value::Bool(open))
}

// --- gfxclose(handle)
pub fn gfxclose(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 1 {
        return Err(host.err(format!(
            "gfxclose expects 1 argument, got {}",
            args.len()
        )));
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
        return Err(host.err(format!(
            "gfxsize expects 1 argument, got {}",
            args.len()
        )));
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
        return Err(host.err(format!(
            "gfxclear expects 2 arguments, got {}",
            args.len()
        )));
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
        let px = x.max(0).min(gw.w as i64 - 1) as usize;
        let py = y.max(0).min(gw.h as i64 - 1) as usize;
        gw.buf[py * gw.w + px] = c;
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
        draw_line_bresenham(&mut gw.buf, gw.w, gw.h, LineParams { x1, y1, x2, y2, color: c });
        Ok(Value::Nil)
    })
}

/// Parameters for Bresenham line drawing — groups coordinates to stay under the argument limit.
struct LineParams {
    x1: i64, y1: i64,
    x2: i64, y2: i64,
    color: u32,
}

fn draw_line_bresenham(
    buf: &mut [u32],
    w: usize,
    h: usize,
    params: LineParams,
) {
    let LineParams { x1, y1, x2, y2, color } = params;
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
        return Err(host.err(format!(
            "gfxpresent expects 1 argument, got {}",
            args.len()
        )));
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

        // Key down
        let keys = gw.win.get_keys_pressed(KeyRepeat::No);
        for k in keys {
                let mut m = HashMap::new();
                m.insert("type".into(), alloc_string("keydown"));
                m.insert("key".into(), alloc_string(format!("{k:?}")));
                out.push(alloc_map(m));
        }

        // Key up
        let keys_up = gw.win.get_keys_released();
        for k in keys_up {
                let mut m = HashMap::new();
                m.insert("type".into(), alloc_string("keyup"));
                m.insert("key".into(), alloc_string(format!("{k:?}")));
                out.push(alloc_map(m));
        }

        // Mouse move
        if let Some((mx, my)) = gw.win.get_mouse_pos(MouseMode::Discard) {
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

        // Mouse buttons (edge detection)
        let left = gw.win.get_mouse_down(MouseButton::Left);
        let right = gw.win.get_mouse_down(MouseButton::Right);

        if left && !gw.last_left {
            let mut m = HashMap::new();
            m.insert("type".into(), alloc_string("mousedown"));
            m.insert("button".into(), alloc_string("Left"));
            out.push(alloc_map(m));
        }
        if !left && gw.last_left {
            let mut m = HashMap::new();
            m.insert("type".into(), alloc_string("mouseup"));
            m.insert("button".into(), alloc_string("Left"));
            out.push(alloc_map(m));
        }
        if right && !gw.last_right {
            let mut m = HashMap::new();
            m.insert("type".into(), alloc_string("mousedown"));
            m.insert("button".into(), alloc_string("Right"));
            out.push(alloc_map(m));
        }
        if !right && gw.last_right {
            let mut m = HashMap::new();
            m.insert("type".into(), alloc_string("mouseup"));
            m.insert("button".into(), alloc_string("Right"));
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
