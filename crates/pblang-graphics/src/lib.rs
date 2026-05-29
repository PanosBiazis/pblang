//! Native graphics builtins for pblang.
//!
//! Two interchangeable backends behind a single builtin surface:
//!
//! - **minifb** (default, always built): pure-Rust software framebuffer
//!   + keyboard/mouse event polling. No system libraries needed beyond
//!   the platform's window manager. Lives in [`graphics`].
//! - **SDL2** (opt-in via the `sdl2` cargo feature and the
//!   `PBLANG_GFX=sdl2` env var at runtime): GPU-accelerated path that
//!   adds texture rendering and richer event support. Lives in the
//!   `sdl2_backend` module (only present when the feature is on). Both
//!   expose the **same** builtin names so pblang code is portable
//!   across them — the dispatch decision is per-call inside each
//!   builtin in [`graphics`].
//!
//! All public surface flows through [`gfx_builtins`], which returns the
//! ready-to-register [`pblang_ir::BuiltinFn`] table; `pblang-vm` merges
//! it into the master builtin table at startup. The legacy [`register`]
//! helper is kept for one external consumer that still wants the
//! `HashMap` shape; new code should use [`gfx_builtins`] directly.

pub mod graphics;

#[cfg(feature = "sdl2")]
pub mod sdl2_backend;

pub use graphics::gfx_builtins;

/// Install gfx builtins into a pre-existing `name -> BuiltinFn` map.
///
/// Kept for one external consumer; new code should call
/// [`gfx_builtins`] and merge the returned `Vec` directly.
pub fn register(builtins: &mut std::collections::HashMap<String, pblang_ir::BuiltinFn>) {
    for b in graphics::gfx_builtins() {
        builtins.insert(b.name.to_string(), b);
    }
}
