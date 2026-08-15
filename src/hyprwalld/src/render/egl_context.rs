//! One process-wide EGL context (`EglCore`) plus one window surface per
//! monitor (`MonitorSurface`), each backed by a layer-shell surface's
//! `wl_egl_window`.
//!
//! ## Why one context for all monitors (changed in Task 10)
//!
//! Task 9 gave every monitor its own `EGLContext`. That was fine while each
//! monitor only cleared itself to a solid color, but Task 10 renders a zone's
//! video *once* into an offscreen texture sized to the zone's bounding box and
//! then blits a different crop of that one texture into each member monitor's
//! surface. GL object names (textures, framebuffer objects) are **per
//! context**: an FBO created while monitor A's context was current simply does
//! not exist in monitor B's context, and even with an EGL share group,
//! framebuffer objects are container objects that are explicitly *not* shared.
//! Rather than juggling share groups plus per-context FBO clones, this module
//! keeps a single `EGLContext` and calls `eglMakeCurrent` with a different
//! *draw surface* per monitor -- which EGL explicitly allows, since every
//! surface here is created from the same `EGLConfig`. One context means one GL
//! namespace: the zone texture/FBO and everything mpv allocates are visible no
//! matter which monitor is currently being drawn.
//!
//! Two knock-on changes from Task 9:
//! - **GLES 3.0 instead of 2.0** (`CONTEXT_CLIENT_VERSION 3`,
//!   `EGL_OPENGL_ES3_BIT`). `glBlitFramebuffer`, which the zone blit is built
//!   on, does not exist in GLES 2.0.
//! - **No `unsafe impl Send`.** Task 9 needed it because `AppData` was moved
//!   into a background Wayland dispatch thread; Task 10's `main.rs` runs
//!   Wayland dispatch, IPC and rendering on one calloop loop on the main
//!   thread, so nothing crosses a thread boundary and the `Rc` used to share
//!   the core between surfaces is sound.
//!
//! ## Installed API notes (khronos-egl 6.0.0, wayland-egl 0.32.11, glow 0.18.0)
//!
//! Carried over from Task 9, still true:
//! - `DynamicInstance::<EGL1_4>::load()` does not exist; only
//!   `DynamicInstance<EGL1_0>` has `load()`. Use the per-version
//!   `load_required()`, which loads as 1.0 and then upgrades.
//! - No static `egl::API` instance exists in a `dynamic`-feature build.
//! - The raw `wl_display*` comes from
//!   `wayland_client::Connection::backend().display_ptr()` (available because
//!   `wayland-egl` unconditionally enables `wayland-backend/client_system`).
//! - `egl.get_proc_address` returns `Option<extern "system" fn()>`, so a
//!   missing symbol has to be mapped to a null pointer by hand.
//! - `glow::Context::from_loader_function` calls `glGetString(GL_VERSION)`
//!   immediately, so a context must already be current when it runs.
//!
//! ## Drop order
//!
//! `khronos-egl`'s `Display`/`Context`/`Surface` are plain `Copy` handles with
//! no `Drop` of their own. `MonitorSurface::drop` destroys its EGL surface
//! before its `_wl_egl_surface` field (declared last) tears down the native
//! `wl_egl_window`; `EglCore::drop` destroys the context. Because every
//! `MonitorSurface` holds an `Rc<EglCore>`, the context is necessarily
//! destroyed after the last surface that used it. `eglTerminate` is
//! deliberately never called: `eglGetDisplay` hands back the same `EGLDisplay`
//! for the same native `wl_display`, shared process-wide.

use std::ffi::c_void;
use std::rc::Rc;

use khronos_egl as egl;
use wayland_client::Proxy;
use wayland_client::protocol::wl_surface::WlSurface;
use wayland_egl::WlEglSurface;

/// Concrete EGL instance type: dynamically loaded, requiring at least EGL 1.4.
type EglInstance = egl::DynamicInstance<egl::EGL1_4>;

/// The single EGL display + config + context shared by every monitor, and the
/// GL function table loaded through it.
pub struct EglCore {
    egl: EglInstance,
    display: egl::Display,
    config: egl::Config,
    context: egl::Context,
    /// Loaded GL ES 3.0 function pointers. Valid whenever any of this core's
    /// surfaces is current.
    pub gl: glow::Context,
}

/// One monitor's EGL window surface, drawn with the shared `EglCore` context.
pub struct MonitorSurface {
    core: Rc<EglCore>,
    surface: egl::Surface,
    /// Must outlive `surface` (see module docs); declared last so it drops last.
    _wl_egl_surface: WlEglSurface,
}

impl EglCore {
    /// Bootstraps EGL for the whole process and creates the first monitor's
    /// surface.
    ///
    /// The first surface has to be created here rather than through
    /// [`EglCore::create_surface`] because loading `glow` requires a current
    /// context, and making a context current requires a surface -- so the
    /// bootstrap surface and the core are necessarily built together.
    ///
    /// `wl_display_ptr` must be the raw `wl_display*` of the same Wayland
    /// connection `wl_surface` belongs to.
    pub fn new(
        wl_display_ptr: *mut c_void,
        wl_surface: &WlSurface,
        width: i32,
        height: i32,
    ) -> anyhow::Result<(Rc<EglCore>, MonitorSurface)> {
        let egl = unsafe { EglInstance::load_required() }
            .map_err(|e| anyhow::anyhow!("failed to load libEGL.so.1 (EGL >= 1.4 required): {e}"))?;

        let display =
            unsafe { egl.get_display(wl_display_ptr) }.ok_or_else(|| anyhow::anyhow!("eglGetDisplay failed"))?;
        egl.initialize(display)?;

        let attribs = [
            egl::SURFACE_TYPE,
            egl::WINDOW_BIT,
            egl::RENDERABLE_TYPE,
            egl::OPENGL_ES3_BIT,
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
            .ok_or_else(|| anyhow::anyhow!("no matching EGL config (GLES 3.0 capable)"))?;

        egl.bind_api(egl::OPENGL_ES_API)?;
        let context_attribs = [egl::CONTEXT_CLIENT_VERSION, 3, egl::NONE];
        let context = egl.create_context(display, config, None, &context_attribs)?;

        let (wl_egl_surface, surface) = create_window_surface(&egl, display, config, wl_surface, width, height)?;

        // Must be current before `glow::Context::from_loader_function` runs
        // (it immediately calls `glGetString(GL_VERSION)`).
        egl.make_current(display, Some(surface), Some(surface), Some(context))?;

        let gl = unsafe {
            glow::Context::from_loader_function(|s| egl.get_proc_address(s).map_or(std::ptr::null(), |f| f as *const _))
        };

        let core = Rc::new(EglCore {
            egl,
            display,
            config,
            context,
            gl,
        });
        let monitor_surface = MonitorSurface {
            core: Rc::clone(&core),
            surface,
            _wl_egl_surface: wl_egl_surface,
        };
        Ok((core, monitor_surface))
    }

    /// Creates an additional monitor's window surface against this same
    /// context.
    pub fn create_surface(
        self: &Rc<Self>,
        wl_surface: &WlSurface,
        width: i32,
        height: i32,
    ) -> anyhow::Result<MonitorSurface> {
        let (wl_egl_surface, surface) =
            create_window_surface(&self.egl, self.display, self.config, wl_surface, width, height)?;
        Ok(MonitorSurface {
            core: Rc::clone(self),
            surface,
            _wl_egl_surface: wl_egl_surface,
        })
    }

    /// Resolves a GL (or EGL) entry point by name, for handing to libmpv's
    /// render API. Returns null if the symbol is unavailable, which is what
    /// mpv's `get_proc_address` contract expects.
    pub fn get_proc_address(&self, name: &str) -> *mut c_void {
        self.egl
            .get_proc_address(name)
            .map_or(std::ptr::null_mut(), |f| f as *mut c_void)
    }
}

fn create_window_surface(
    egl: &EglInstance,
    display: egl::Display,
    config: egl::Config,
    wl_surface: &WlSurface,
    width: i32,
    height: i32,
) -> anyhow::Result<(WlEglSurface, egl::Surface)> {
    let wl_egl_surface = WlEglSurface::new(wl_surface.id(), width, height)?;
    let surface = unsafe { egl.create_window_surface(display, config, wl_egl_surface.ptr() as *mut c_void, None)? };
    Ok((wl_egl_surface, surface))
}

impl MonitorSurface {
    /// Binds the shared context to this monitor's surface. Every GL call that
    /// follows targets this monitor's default framebuffer.
    pub fn make_current(&self) -> anyhow::Result<()> {
        self.core.egl.make_current(
            self.core.display,
            Some(self.surface),
            Some(self.surface),
            Some(self.core.context),
        )?;
        Ok(())
    }

    pub fn swap_buffers(&self) -> anyhow::Result<()> {
        self.core.egl.swap_buffers(self.core.display, self.surface)?;
        Ok(())
    }

    pub fn gl(&self) -> &glow::Context {
        &self.core.gl
    }
}

impl Drop for MonitorSurface {
    fn drop(&mut self) {
        // Best-effort: ignore errors (e.g. the connection is already gone at
        // process teardown).
        let _ = self.core.egl.destroy_surface(self.core.display, self.surface);
    }
}

impl Drop for EglCore {
    fn drop(&mut self) {
        // Runs only after the last `MonitorSurface` holding an `Rc` to this
        // core has been dropped, so no surface is still referencing the
        // context. See module docs for why `eglTerminate` is not called.
        let _ = self.egl.destroy_context(self.display, self.context);
    }
}
