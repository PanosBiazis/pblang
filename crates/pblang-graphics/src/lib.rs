pub mod graphics;

#[cfg(feature = "sdl2")]
pub mod sdl2_backend;

pub use graphics::gfx_builtins;

/// Legacy: install gfx builtins into an existing HashMap.
pub fn register(builtins: &mut std::collections::HashMap<String, pblang_ir::BuiltinFn>) {
    for b in graphics::gfx_builtins() {
        builtins.insert(b.name.to_string(), b);
    }
}
