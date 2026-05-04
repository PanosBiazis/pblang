//! Optional SDL2 backend: same gfx API, select with PBLANG_GFX=sdl2 and feature "sdl2".

use pblang_ir::{alloc_string, RuntimeError, Value, VmHost};
use sdl2::event::Event;
use sdl2::mouse::MouseButton as SdlMouseButton;
use sdl2::pixels::PixelFormatEnum;
use sdl2::rect::Rect;
use sdl2::render::{Canvas, TextureCreator};
use sdl2::video::{Window, WindowContext};
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
    sdl: sdl2::Sdl,
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
    let video = sdl.video().map_err(|e| host.err(format!("sdl2 video: {e}")))?;
    let window = video
        .window(title, width, height)
        .build()
        .map_err(|e| host.err(format!("sdl2 window: {e}")))?;
    let canvas = window
        .into_canvas()
        .present_vsync()
        .build()
        .map_err(|e| host.err(format!("sdl2 canvas: {e}")))?;
    let texture_creator = canvas.texture_creator();
    let buf = vec![0u32; (width * height) as usize];

    let win = Sdl2Window {
        canvas,
        texture_creator,
        sdl,
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

impl Sdl2Window {
    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn clear(&mut self, color: u32) {
        self.buf.fill(color);
    }

    pub fn draw_rect(&mut self, x: i64, y: i64, rw: i64, rh: i64, color: u32) {
        let x0 = x.max(0) as u32;
        let y0 = y.max(0) as u32;
        let x1 = (x + rw).min(self.w as i64).max(0) as u32;
        let y1 = (y + rh).min(self.h as i64).max(0) as u32;
        for yy in y0..y1 {
            let row = (yy * self.w) as usize;
            for xx in x0..x1 {
                self.buf[row + xx as usize] = color;
            }
        }
    }

    pub fn draw_line(&mut self, x1: i64, y1: i64, x2: i64, y2: i64, color: u32) {
        let w = self.w as i64;
        let h = self.h as i64;
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
                for (i, &pixel) in self.buf.iter().enumerate() {
                    let y = i / self.w as usize;
                    let x = i % self.w as usize;
                    let offset = y * pitch + x * 4;
                    if offset + 4 <= out.len() {
                        out[offset] = (pixel & 0xff) as u8;
                        out[offset + 1] = ((pixel >> 8) & 0xff) as u8;
                        out[offset + 2] = ((pixel >> 16) & 0xff) as u8;
                        out[offset + 3] = ((pixel >> 24) & 0xff) as u8;
                    }
                }
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
        let mut event_pump = self.sdl.event_pump().map_err(|e| RuntimeError {
            message: format!("sdl2 event pump: {e}"),
            span: None,
            source_name: None,
            source: None,
            stack: vec![],
        })?;

        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. } => {
                    self.open = false;
                    let mut m = HashMap::new();
                    m.insert("type".into(), alloc_string("close"));
                    out.push(alloc_map(m));
                }
                Event::KeyDown {
                    keycode: Some(k), ..
                } => {
                    let mut m = HashMap::new();
                    m.insert("type".into(), alloc_string("keydown"));
                    m.insert("key".into(), alloc_string(format!("{:?}", k).replace("Keycode::", "")));
                    out.push(alloc_map(m));
                }
                Event::KeyUp {
                    keycode: Some(k), ..
                } => {
                    let mut m = HashMap::new();
                    m.insert("type".into(), alloc_string("keyup"));
                    m.insert("key".into(), alloc_string(format!("{:?}", k).replace("Keycode::", "")));
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
                Event::MouseButtonDown { mouse_btn, .. } => {
                    let mut m = HashMap::new();
                    m.insert("type".into(), alloc_string("mousedown"));
                    m.insert(
                        "button".into(),
                        alloc_string(format!("{:?}", mouse_btn).replace("MouseButton::", "")),
                    );
                    out.push(alloc_map(m));
                    if mouse_btn == SdlMouseButton::Left {
                        self.last_left = true;
                    }
                    if mouse_btn == SdlMouseButton::Right {
                        self.last_right = true;
                    }
                }
                Event::MouseButtonUp { mouse_btn, .. } => {
                    let mut m = HashMap::new();
                    m.insert("type".into(), alloc_string("mouseup"));
                    m.insert(
                        "button".into(),
                        alloc_string(format!("{:?}", mouse_btn).replace("MouseButton::", "")),
                    );
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
