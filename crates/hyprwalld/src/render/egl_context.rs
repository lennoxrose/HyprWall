//! Per-monitor EGL context bound to a layer-shell surface's `wl_egl_window`,
//! plus the `glow` GL function-pointer table loaded through it.
//!
//! Installed API notes (khronos-egl 6.0.0, wayland-egl 0.32.11, glow
//! 0.18.0), read directly from crate source rather than trusting the task
//! sketch (same caveat as Tasks 7-8):
//!
//! - **`DynamicInstance::<EGL1_4>::load()` does not exist.** `load()` is
//!   only implemented for `DynamicInstance<EGL1_0>` (an inherent impl keyed
//!   to that exact version). To require a minimum version (EGL 1.4 here,
//!   for `EGL_CONTEXT_CLIENT_VERSION`/`eglBindAPI`, both stable since 1.2,
//!   plus general 1.4-era platform robustness), the crate provides a
//!   separate `load_required()` associated function generated per-version,
//!   which loads as EGL 1.0 first and then upgrades, failing with
//!   `LoadError::InvalidVersion` if the library's actual version is lower.
//!   Used `EglInstance::load_required()` instead of `load()`.
//! - **No static `egl::API` instance is available.** The brief's `use
//!   egl::{self, API as EglApi}` assumes the `static`+`nightly` feature
//!   combo; this crate only enables `dynamic` (per Task 9's Step 1), which
//!   never defines `API`. Used `DynamicInstance<EGL1_4>` as the concrete
//!   type instead.
//! - **Getting the raw `wl_display` pointer**: neither `wayland_client`'s
//!   `Connection` nor its `backend::ObjectId` expose a `display_ptr()` by
//!   default -- `ObjectId::display_ptr()` exists but is gated behind
//!   `wayland-backend`'s `libwayland_client_1_23` feature, which nothing in
//!   this workspace enables. What *is* available unconditionally (given
//!   `wayland-egl` already pulls in `wayland-backend/client_system`, which
//!   Cargo's feature unification applies workspace-wide) is
//!   `wayland_client::Connection::backend() -> wayland_backend::client::Backend`,
//!   which has `Backend::display_ptr(&self) -> *mut wayland_sys::client::wl_display`.
//!   `connection.rs` passes that pointer (cast to `*mut c_void`) into
//!   `EglContext::new`.
//! - **`egl.get_proc_address` returns `Option<extern "system" fn()>`, not a
//!   raw pointer** -- `glow::Context::from_loader_function`'s closure must
//!   return `*const c_void`, so `None` (symbol not found) is mapped to
//!   `std::ptr::null()` rather than the brief's bare `as *const _` cast
//!   (which doesn't typecheck against an `Option`).
//! - **`glow::Context::from_loader_function` calls `glGetString(GL_VERSION)`
//!   immediately** to detect the GL version, which requires a context to
//!   already be current. So `make_current()` must run *before* constructing
//!   the `glow::Context`, not after (the brief's Step 4 sketch loads glow
//!   "once per EglContext", implicitly after the context exists, but did not
//!   spell out that ordering constraint) -- `EglContext::new` below calls
//!   `egl.make_current(...)` before building `gl`.
//!
//! ## Drop order
//!
//! `khronos-egl`'s `Display`/`Context`/`Surface` are plain `Copy` handles
//! with no `Drop` impl of their own -- destroying them is something *we*
//! must do explicitly by calling `eglDestroySurface`/`eglDestroyContext`.
//! `wayland_egl::WlEglSurface` *does* auto-destroy its native
//! `wl_egl_window` in its own `Drop` impl, and per its own docs must not be
//! destroyed before the EGL surface that wraps it. This `Drop` impl
//! therefore explicitly destroys the EGL surface and context first; the
//! `_wl_egl_surface` field is declared last so Rust's field-order drop runs
//! it only after that explicit cleanup returns. Deliberately *not* calling
//! `eglTerminate`: `eglGetDisplay` returns the same `EGLDisplay` handle for
//! the same native `wl_display` pointer, which every monitor's `EglContext`
//! shares (one Wayland connection for the whole process) -- terminating it
//! in one monitor's `Drop` would tear down EGL for every other monitor's
//! still-live context.

use khronos_egl as egl;
use wayland_client::protocol::wl_surface::WlSurface;
use wayland_client::Proxy;
use wayland_egl::WlEglSurface;

/// Concrete EGL instance type: dynamically loaded, requiring at least EGL
/// 1.4.
type EglInstance = egl::DynamicInstance<egl::EGL1_4>;

/// One monitor's EGL context + window surface + loaded GL function table.
pub struct EglContext {
    egl: EglInstance,
    display: egl::Display,
    context: egl::Context,
    surface: egl::Surface,
    /// Loaded GL ES function pointers, valid as long as this `EglContext`'s
    /// context is (or was most recently) current.
    pub gl: glow::Context,
    /// Must outlive `surface` (see module docs on drop order); declared
    /// last so it is dropped last.
    _wl_egl_surface: WlEglSurface,
}

impl EglContext {
    /// Creates an EGL 1.4+ context and window surface bound to `wl_surface`,
    /// makes it current, and loads a `glow::Context` through it.
    ///
    /// `wl_display_ptr` must be the raw `wl_display*` for the same Wayland
    /// connection that `wl_surface` belongs to (see module docs for how
    /// callers obtain it).
    pub fn new(
        wl_display_ptr: *mut std::ffi::c_void,
        wl_surface: &WlSurface,
        width: i32,
        height: i32,
    ) -> anyhow::Result<Self> {
        let egl = unsafe { EglInstance::load_required() }
            .map_err(|e| anyhow::anyhow!("failed to load libEGL.so.1 (EGL >= 1.4 required): {e}"))?;

        let display = unsafe { egl.get_display(wl_display_ptr) }
            .ok_or_else(|| anyhow::anyhow!("eglGetDisplay failed"))?;
        egl.initialize(display)?;

        let attribs = [
            egl::SURFACE_TYPE,
            egl::WINDOW_BIT,
            egl::RENDERABLE_TYPE,
            egl::OPENGL_ES2_BIT,
            egl::RED_SIZE,
            8,
            egl::GREEN_SIZE,
            8,
            egl::BLUE_SIZE,
            8,
            egl::NONE,
        ];
        let config = egl
            .choose_first_config(display, &attribs)?
            .ok_or_else(|| anyhow::anyhow!("no matching EGL config"))?;

        egl.bind_api(egl::OPENGL_ES_API)?;
        let context_attribs = [egl::CONTEXT_CLIENT_VERSION, 2, egl::NONE];
        let context = egl.create_context(display, config, None, &context_attribs)?;

        let wl_egl_surface = WlEglSurface::new(wl_surface.id(), width, height)?;
        let surface = unsafe {
            egl.create_window_surface(
                display,
                config,
                wl_egl_surface.ptr() as *mut std::ffi::c_void,
                None,
            )?
        };

        // Must be current before `glow::Context::from_loader_function` runs
        // (it immediately calls `glGetString(GL_VERSION)`).
        egl.make_current(display, Some(surface), Some(surface), Some(context))?;

        let gl = unsafe {
            glow::Context::from_loader_function(|s| {
                egl.get_proc_address(s).map_or(std::ptr::null(), |f| f as *const _)
            })
        };

        Ok(Self { egl, display, context, surface, gl, _wl_egl_surface: wl_egl_surface })
    }

    pub fn make_current(&self) -> anyhow::Result<()> {
        self.egl.make_current(self.display, Some(self.surface), Some(self.surface), Some(self.context))?;
        Ok(())
    }

    pub fn swap_buffers(&self) -> anyhow::Result<()> {
        self.egl.swap_buffers(self.display, self.surface)?;
        Ok(())
    }
}

impl Drop for EglContext {
    fn drop(&mut self) {
        // Best-effort: ignore errors (e.g. if the connection is already
        // gone at process teardown). See module docs for why `eglTerminate`
        // is deliberately not called here.
        let _ = self.egl.destroy_surface(self.display, self.surface);
        let _ = self.egl.destroy_context(self.display, self.context);
    }
}

// SAFETY: `khronos_egl::Display`/`Context`/`Surface` are opaque handle
// newtypes wrapping raw pointers, which lose the auto-derived `Send` impl
// raw pointers don't get by default -- but there is nothing inherently
// thread-affine about merely *holding* one of these handles. EGL's real
// constraint is that a context must not be current on more than one thread
// at the same time, which this daemon never does: `AppData` (and therefore
// every `EglContext` inside its `render_targets` map) is moved once, in its
// entirety, into the single background thread that owns the Wayland event
// queue (see `main.rs`), and every EGL call (`EglContext::new`,
// `make_current`, `swap_buffers`) happens on that one thread from then on.
// `Send` is only needed here to satisfy that one-time ownership transfer.
unsafe impl Send for EglContext {}
