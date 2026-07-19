//! Optional SDL2 backend: same gfx API surface as the minifb backend.
//!
//! Selected at runtime when **both** of these are true:
//!  - the crate was built with the `sdl2` cargo feature on, and
//!  - `PBLANG_GFX=sdl2` is set in the environment.
//!
//! Each `gfx*` builtin in [`super::graphics`] checks [`use_sdl2`] and
//! routes to a `sdl2_*` counterpart here. The handle-vending scheme is
//! the same (an integer indexing the [`GFX_SDL2_WINDOWS`] thread-local);
//! `Sdl2Window` adds a `Canvas` + `TextureCreator` so we can blit the
//! `Vec<u32>` back-buffer through a streaming SDL texture every frame.

use pblang_ir::{alloc_string, RuntimeError, Value, VmHost};
use sdl2::event::Event;
use sdl2::keyboard::Mod;
use sdl2::mouse::MouseButton as SdlMouseButton;
use sdl2::pixels::PixelFormatEnum;
use sdl2::rect::Rect;
use sdl2::render::{Canvas, TextureCreator};
use sdl2::video::{Window, WindowContext};
use sdl2::{EventPump, Sdl};
use std::cell::RefCell;
use std::collections::HashMap;

thread_local! {
    static GFX_SDL2_WINDOWS: RefCell<HashMap<usize, Sdl2Window>> = RefCell::new(HashMap::new());
}

pub fn use_sdl2() -> bool {
    std::env::var("PBLANG_GFX").as_deref() == Ok("sdl2")
}

pub(crate) struct Sdl2Window {
    canvas: Canvas<Window>,
    texture_creator: TextureCreator<WindowContext>,
    _sdl: Sdl,
    event_pump: EventPump,
    w: u32,
    h: u32,
    buf: Vec<u32>,
    open: bool,
    last_mouse: Option<(f32, f32)>,
    last_left: bool,
    last_right: bool,
}

pub(crate) fn create(
    host: &mut dyn VmHost,
    width: u32,
    height: u32,
    title: &str,
) -> Result<Sdl2Window, RuntimeError> {
    let sdl = sdl2::init().map_err(|e| host.err(format!("sdl2 init: {e}")))?;
    let video = sdl
        .video()
        .map_err(|e| host.err(format!("sdl2 video: {e}")))?;
    let window = video
        .window(title, width, height)
        .resizable()
        .build()
        .map_err(|e| host.err(format!("sdl2 window: {e}")))?;
    // Explicitly request a GPU-ACCELERATED renderer (was relying on SDL's default driver
    // pick). This puts the per-frame texture upload + blit on the GPU. NOTE: the 2D
    // primitives (rect/line/text) are still rasterized on the CPU into `buf`; only the
    // present path is GPU-accelerated here. `PBLANG_GFX_SOFTWARE=1` forces the old software
    // renderer (useful for headless/CI or broken drivers).
    let mut cb = window.into_canvas().present_vsync();
    if std::env::var("PBLANG_GFX_SOFTWARE")
        .map(|v| v == "1")
        .unwrap_or(false)
    {
        cb = cb.software();
    } else {
        cb = cb.accelerated();
    }
    let canvas = cb
        .build()
        .map_err(|e| host.err(format!("sdl2 canvas: {e}")))?;
    let texture_creator = canvas.texture_creator();
    let event_pump = sdl
        .event_pump()
        .map_err(|e| host.err(format!("sdl2 event pump: {e}")))?;
    let buf = vec![0u32; (width * height) as usize];

    let win = Sdl2Window {
        canvas,
        texture_creator,
        _sdl: sdl,
        event_pump,
        w: width,
        h: height,
        buf,
        open: true,
        last_mouse: None,
        last_left: false,
        last_right: false,
    };
    Ok(win)
}

fn insert_key_modifiers(m: &mut HashMap<String, Value>, keymod: Mod) {
    let shift = keymod.intersects(Mod::LSHIFTMOD | Mod::RSHIFTMOD);
    let ctrl = keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD);
    let alt = keymod.intersects(Mod::LALTMOD | Mod::RALTMOD);
    let meta = keymod.intersects(Mod::LGUIMOD | Mod::RGUIMOD);
    m.insert("shift".into(), Value::Bool(shift));
    m.insert("ctrl".into(), Value::Bool(ctrl));
    m.insert("alt".into(), Value::Bool(alt));
    m.insert("meta".into(), Value::Bool(meta));
}

pub(crate) fn insert(id: usize, win: Sdl2Window) {
    GFX_SDL2_WINDOWS.with(|tbl| tbl.borrow_mut().insert(id, win));
}

pub fn has(id: usize) -> bool {
    GFX_SDL2_WINDOWS.with(|tbl| tbl.borrow().contains_key(&id))
}

pub(crate) fn with_window_mut<F, R>(
    host: &mut dyn VmHost,
    id: usize,
    what: &str,
    f: F,
) -> Result<R, RuntimeError>
where
    F: FnOnce(&mut Sdl2Window) -> Result<R, RuntimeError>,
{
    GFX_SDL2_WINDOWS.with(|tbl| {
        let mut t = tbl.borrow_mut();
        let w = t
            .get_mut(&id)
            .ok_or_else(|| host.err(format!("{what}: invalid handle")))?;
        f(w)
    })
}

pub fn remove(id: usize) {
    GFX_SDL2_WINDOWS.with(|tbl| {
        tbl.borrow_mut().remove(&id);
    });
}

fn write_argb8888_rows(out: &mut [u8], pitch: usize, buf: &[u32], width: usize, height: usize) {
    let min_pitch = width.saturating_mul(4);
    if width == 0 || height == 0 || pitch < min_pitch {
        return;
    }
    let rows = height.min(buf.len() / width).min(out.len() / pitch);
    for row in 0..rows {
        let src_row = &buf[row * width..(row + 1) * width];
        let dst_row = &mut out[row * pitch..row * pitch + min_pitch];
        for (dst, &pixel) in dst_row.chunks_exact_mut(4).zip(src_row.iter()) {
            dst[0] = (pixel & 0xff) as u8;
            dst[1] = ((pixel >> 8) & 0xff) as u8;
            dst[2] = ((pixel >> 16) & 0xff) as u8;
            dst[3] = ((pixel >> 24) & 0xff) as u8;
        }
    }
}

impl Sdl2Window {
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Pixel size of the display the window currently sits on (for live UI
    /// density when the window is dragged between monitors). Best-effort.
    fn current_display_size(&self) -> Option<(u32, u32)> {
        let window = self.canvas.window();
        let idx = window.display_index().ok()?;
        let bounds = window.subsystem().display_bounds(idx).ok()?;
        Some((bounds.width(), bounds.height()))
    }

    pub fn clear(&mut self, color: u32) {
        self.buf.fill(color);
    }

    pub fn draw_rect(&mut self, x: i64, y: i64, rw: i64, rh: i64, color: u32) {
        crate::graphics::fill_rect_into(
            &mut self.buf,
            self.w as usize,
            self.h as usize,
            x,
            y,
            rw,
            rh,
            color,
        );
    }

    pub fn draw_rects(&mut self, rects: &[(i64, i64, i64, i64)], color: u32) {
        for &(x, y, rw, rh) in rects {
            self.draw_rect(x, y, rw, rh, color);
        }
    }

    pub fn draw_line(&mut self, x1: i64, y1: i64, x2: i64, y2: i64, color: u32) {
        let w = self.w as i64;
        let h = self.h as i64;
        // Clip to the viewport first so a huge user coordinate can't make this
        // Bresenham loop run ~max(|dx|,|dy|) times (an effective hang). Shares
        // the unit-tested clipper with the minifb path.
        let (x1, y1, x2, y2) =
            match crate::graphics::clip_line(x1, y1, x2, y2, self.w as usize, self.h as usize) {
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
            if x >= 0 && x < w && y >= 0 && y < h {
                self.buf[y as usize * self.w as usize + x as usize] = color;
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

    #[allow(clippy::too_many_arguments)]
    pub fn draw_text_aa(
        &mut self,
        x: i64,
        y: i64,
        text: &str,
        color: u32,
        px: f32,
        font_path: &str,
        advance: f32,
    ) -> Result<crate::graphics::TextDrawStats, String> {
        crate::graphics::draw_text_aa_into(
            &mut self.buf,
            self.w as usize,
            self.h as usize,
            x,
            y,
            text,
            color,
            px,
            font_path,
            advance,
        )
    }

    pub fn present(&mut self) -> Result<(), RuntimeError> {
        let mut texture = self
            .texture_creator
            .create_texture_streaming(PixelFormatEnum::ARGB8888, self.w, self.h)
            .map_err(|e| RuntimeError {
                message: format!("sdl2 texture: {e}"),
                span: None,
                source_name: None,
                source: None,
                stack: vec![],
            })?;
        texture
            .with_lock(None, |out: &mut [u8], pitch: usize| {
                write_argb8888_rows(out, pitch, &self.buf, self.w as usize, self.h as usize);
            })
            .map_err(|e| RuntimeError {
                message: format!("sdl2 lock: {e}"),
                span: None,
                source_name: None,
                source: None,
                stack: vec![],
            })?;
        self.canvas.clear();
        self.canvas
            .copy(&texture, None, Some(Rect::new(0, 0, self.w, self.h)))
            .map_err(|e| RuntimeError {
                message: format!("sdl2 copy: {e}"),
                span: None,
                source_name: None,
                source: None,
                stack: vec![],
            })?;
        self.canvas.present();
        Ok(())
    }

    pub fn poll_events(
        &mut self,
        alloc_array: impl Fn(Vec<Value>) -> Value,
        alloc_map: impl Fn(HashMap<String, Value>) -> Value,
    ) -> Result<Value, RuntimeError> {
        let mut out: Vec<Value> = Vec::new();

        while let Some(event) = self.event_pump.poll_event() {
            match event {
                Event::Quit { .. } => {
                    self.open = false;
                    let mut m = HashMap::new();
                    m.insert("type".into(), alloc_string("close"));
                    out.push(alloc_map(m));
                }
                // RESIZABLE: track the new window size, regrow the back-buffer, and tell the app.
                Event::Window {
                    win_event:
                        sdl2::event::WindowEvent::Resized(nw, nh)
                        | sdl2::event::WindowEvent::SizeChanged(nw, nh),
                    ..
                } => {
                    if nw > 0 && nh > 0 {
                        self.w = nw as u32;
                        self.h = nh as u32;
                        self.buf.resize((self.w * self.h) as usize, 0);
                        let mut m = HashMap::new();
                        m.insert("type".into(), alloc_string("resize"));
                        m.insert("w".into(), Value::Number(self.w as f64));
                        m.insert("h".into(), Value::Number(self.h as f64));
                        if let Some((dw, dh)) = self.current_display_size() {
                            m.insert("display_w".into(), Value::Number(dw as f64));
                            m.insert("display_h".into(), Value::Number(dh as f64));
                        }
                        out.push(alloc_map(m));
                    }
                }
                // LIVE MULTI-MONITOR: when the window is dragged, report where it went
                // and how big the display under it is, so the app can rescale its UI
                // the moment the window crosses onto another screen (no shell probes).
                Event::Window {
                    win_event: sdl2::event::WindowEvent::Moved(nx, ny),
                    ..
                } => {
                    let mut m = HashMap::new();
                    m.insert("type".into(), alloc_string("moved"));
                    m.insert("x".into(), Value::Number(nx as f64));
                    m.insert("y".into(), Value::Number(ny as f64));
                    if let Some((dw, dh)) = self.current_display_size() {
                        m.insert("display_w".into(), Value::Number(dw as f64));
                        m.insert("display_h".into(), Value::Number(dh as f64));
                    }
                    out.push(alloc_map(m));
                }
                Event::KeyDown {
                    keycode: Some(k),
                    keymod,
                    ..
                } => {
                    let mut m = HashMap::new();
                    m.insert("type".into(), alloc_string("keydown"));
                    m.insert(
                        "key".into(),
                        alloc_string(format!("{:?}", k).replace("Keycode::", "")),
                    );
                    insert_key_modifiers(&mut m, keymod);
                    out.push(alloc_map(m));
                }
                Event::KeyUp {
                    keycode: Some(k),
                    keymod,
                    ..
                } => {
                    let mut m = HashMap::new();
                    m.insert("type".into(), alloc_string("keyup"));
                    m.insert(
                        "key".into(),
                        alloc_string(format!("{:?}", k).replace("Keycode::", "")),
                    );
                    insert_key_modifiers(&mut m, keymod);
                    out.push(alloc_map(m));
                }
                Event::MouseMotion { x, y, .. } => {
                    let mut m = HashMap::new();
                    m.insert("type".into(), alloc_string("mousemove"));
                    m.insert("x".into(), Value::Number(x as f64));
                    m.insert("y".into(), Value::Number(y as f64));
                    out.push(alloc_map(m));
                    self.last_mouse = Some((x as f32, y as f32));
                }
                Event::MouseButtonDown {
                    mouse_btn, x, y, ..
                } => {
                    let mut m = HashMap::new();
                    m.insert("type".into(), alloc_string("mousedown"));
                    m.insert(
                        "button".into(),
                        alloc_string(format!("{:?}", mouse_btn).replace("MouseButton::", "")),
                    );
                    m.insert("x".into(), Value::Number(x as f64));
                    m.insert("y".into(), Value::Number(y as f64));
                    out.push(alloc_map(m));
                    if mouse_btn == SdlMouseButton::Left {
                        self.last_left = true;
                    }
                    if mouse_btn == SdlMouseButton::Right {
                        self.last_right = true;
                    }
                }
                Event::MouseButtonUp {
                    mouse_btn, x, y, ..
                } => {
                    let mut m = HashMap::new();
                    m.insert("type".into(), alloc_string("mouseup"));
                    m.insert(
                        "button".into(),
                        alloc_string(format!("{:?}", mouse_btn).replace("MouseButton::", "")),
                    );
                    m.insert("x".into(), Value::Number(x as f64));
                    m.insert("y".into(), Value::Number(y as f64));
                    out.push(alloc_map(m));
                    if mouse_btn == SdlMouseButton::Left {
                        self.last_left = false;
                    }
                    if mouse_btn == SdlMouseButton::Right {
                        self.last_right = false;
                    }
                }
                Event::MouseWheel { y, x, .. } => {
                    let mut m = HashMap::new();
                    m.insert("type".into(), alloc_string("scroll"));
                    m.insert("dx".into(), Value::Number(x as f64));
                    m.insert("dy".into(), Value::Number(y as f64));
                    out.push(alloc_map(m));
                }
                _ => {}
            }
        }
        Ok(alloc_array(out))
    }

    pub fn size(&self) -> (usize, usize) {
        (self.w as usize, self.h as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::write_argb8888_rows;

    #[test]
    fn write_argb8888_rows_preserves_byte_order() {
        let pixels = [0x11223344, 0xAABBCCDD];
        let mut out = [0u8; 8];
        write_argb8888_rows(&mut out, 8, &pixels, 2, 1);
        assert_eq!(out, [0x44, 0x33, 0x22, 0x11, 0xDD, 0xCC, 0xBB, 0xAA]);
    }

    #[test]
    fn write_argb8888_rows_respects_pitch_padding() {
        let pixels = [0x01020304, 0x05060708, 0x11121314, 0x15161718];
        let mut out = [0xEEu8; 24];
        write_argb8888_rows(&mut out, 12, &pixels, 2, 2);
        assert_eq!(
            out,
            [
                0x04, 0x03, 0x02, 0x01, 0x08, 0x07, 0x06, 0x05, 0xEE, 0xEE, 0xEE, 0xEE, 0x14, 0x13,
                0x12, 0x11, 0x18, 0x17, 0x16, 0x15, 0xEE, 0xEE, 0xEE, 0xEE
            ]
        );
    }

    #[test]
    fn write_argb8888_rows_ignores_invalid_pitch() {
        let pixels = [0x01020304, 0x05060708];
        let mut out = [0xEEu8; 6];
        write_argb8888_rows(&mut out, 6, &pixels, 2, 1);
        assert_eq!(out, [0xEE; 6]);
    }
}
