//! One libmpv player + its OpenGL render context, driving a single zone.
//!
//! ## Installed API notes (libmpv2 6.0.0 against libmpv 2.5 / mpv 0.41)
//!
//! The task sketch was written against a different shape of the crate; what is
//! actually installed differs in four load-bearing ways:
//!
//! 1. **There is no `RenderContext::new(...)`.** A render context is created
//!    from the player itself: `Mpv::create_render_context(params)`.
//! 2. **`RenderContext<'a>` borrows the `Mpv`** (`PhantomData<&'a Mpv>`), so
//!    `struct MpvInstance { mpv: Mpv, render_context: RenderContext }` is
//!    self-referential and does not compile as written. See "Lifetime" below.
//! 3. **`RenderParam::InitParams` takes an `OpenGLInitParams<C>`, not a
//!    closure**: a plain `fn(&C, &str) -> *mut c_void` function pointer plus an
//!    owned context value `C` that mpv passes back. A capturing closure cannot
//!    be used; the capture has to live in `C`. This module takes
//!    `Rc<dyn Fn(&str) -> *mut c_void>` as `C` and uses a non-capturing
//!    trampoline, which keeps EGL specifics out of this file.
//! 4. **`render()` is generic over the (here unused) GL-context type** and must
//!    be turbofished: `render::<()>(fbo, w, h, flip)`.
//!
//! ## Lifetime
//!
//! `create_render_context(&self) -> RenderContext<'_>` ties the context to a
//! borrow of the `Mpv`. The borrow is purely a `PhantomData` marker -- the
//! struct itself only holds a `*mut mpv_render_context` and never dereferences
//! the `Mpv` -- so the invariant that actually matters is *the `Mpv` must
//! outlive the `RenderContext`* (`mpv_render_context_free` must run before
//! `mpv_destroy`). This module transmutes the lifetime to `'static` and then
//! enforces that invariant structurally: `render_context` is declared before
//! `mpv`, and Rust drops fields in declaration order, so the render context is
//! always freed first. The `Mpv` is boxed so its address is stable even though
//! nothing in `RenderContext` currently depends on that.
//!
//! ## Threading
//!
//! `set_update_callback` fires from an mpv-internal thread. Its callback must
//! not touch GL or call back into mpv -- it only pings the calloop event loop
//! (see `render/frame_scheduler.rs`). Every other method here must be called
//! from the render thread (the daemon's main thread).
//!
//! Note that libmpv2 frees the `OpenGLInitParams` (and therefore the `Rc`
//! loader) immediately after `mpv_render_context_create` returns, on the
//! calling thread -- so the non-`Send` `Rc` never escapes this thread.

use std::ffi::c_void;
use std::rc::Rc;

use libmpv2::Mpv;
use libmpv2::render::{
    OpenGLInitParams, RenderContext, RenderParam, RenderParamApiType, mpv_render_update,
};

/// Resolves GL entry points for mpv. Owned by mpv only for the duration of
/// `create_render_context`.
type ProcAddressLoader = Rc<dyn Fn(&str) -> *mut c_void>;

pub struct MpvInstance {
    /// Declared first: must be dropped before `mpv` (see module docs).
    render_context: RenderContext<'static>,
    mpv: Box<Mpv>,
}

impl MpvInstance {
    /// Creates a muted, infinitely looping player wired to the GL context
    /// `get_proc_address` resolves symbols for.
    ///
    /// The caller must have made that GL context current before calling this:
    /// `mpv_render_context_create` inspects the live GL context.
    pub fn new(get_proc_address: ProcAddressLoader) -> anyhow::Result<Self> {
        let mpv = Box::new(Mpv::with_initializer(|init| {
            // The render API requires the libmpv video output.
            init.set_option("vo", "libmpv")?;
            // Wallpapers loop forever.
            init.set_option("loop-file", "inf")?;
            // Spec: wallpapers are silent, with no per-wallpaper audio policy
            // in v1. `mute` alone would still open an audio device (and could
            // be undone by a stray property write), so the audio output is
            // also forced to the null driver.
            init.set_option("mute", "yes")?;
            init.set_option("ao", "null")?;
            // Without this, `mpv_render_context_render` blocks until the
            // frame's scheduled display time, which would stall every *other*
            // zone rendered from the same thread.
            init.set_option("video-timing-offset", "0")?;
            Ok(())
        })?);

        let params = vec![
            RenderParam::ApiType(RenderParamApiType::OpenGl),
            RenderParam::InitParams(OpenGLInitParams {
                // Non-capturing, so it coerces to the `fn` pointer mpv wants.
                get_proc_address: |ctx: &ProcAddressLoader, name: &str| ctx(name),
                ctx: get_proc_address,
            }),
        ];
        let render_context = mpv.create_render_context(params)?;

        // SAFETY: `RenderContext<'a>`'s lifetime is a `PhantomData<&'a Mpv>`
        // marker only; the struct holds a raw `*mut mpv_render_context` and
        // never reads through the `Mpv`. The real requirement is that the
        // `Mpv` outlive the render context, which the field declaration order
        // below guarantees (fields drop in declaration order, so
        // `mpv_render_context_free` always runs before `mpv_destroy`).
        // Neither field is ever moved out or exposed by reference.
        let render_context: RenderContext<'static> =
            unsafe { std::mem::transmute::<RenderContext<'_>, RenderContext<'static>>(render_context) };

        Ok(Self { render_context, mpv })
    }

    /// Starts (or replaces) playback of `path`.
    pub fn load_file(&mut self, path: &str) -> anyhow::Result<()> {
        self.mpv.command("loadfile", &[path, "replace"])?;
        Ok(())
    }

    /// Renders the current frame into `fbo` at `width` x `height`.
    ///
    /// `flip` is true: mpv's unflipped output puts the video's first row at
    /// GL y=0, which is the *bottom* of a GL framebuffer. Flipping puts the
    /// top of the picture at high y, matching the orientation an EGL window
    /// surface is presented in, so the offscreen texture and the monitor's
    /// backbuffer share one convention and the per-monitor blit is a straight
    /// copy.
    pub fn render_to_fbo(&mut self, fbo: i32, width: i32, height: i32) -> anyhow::Result<()> {
        self.render_context.render::<()>(fbo, width, height, true)?;
        Ok(())
    }

    /// Registers a callback fired from an mpv thread when a new frame is
    /// available. It must not call into mpv or GL.
    pub fn set_wakeup_callback(&mut self, cb: impl Fn() + Send + 'static) {
        self.render_context.set_update_callback(cb);
    }

    /// Whether mpv has a new frame that wants drawing. Must be called on the
    /// render thread, never from the wakeup callback.
    pub fn wants_redraw(&self) -> bool {
        match self.render_context.update() {
            Ok(flags) => flags & mpv_render_update::Frame != 0,
            Err(_) => false,
        }
    }

    pub fn set_paused(&self, paused: bool) -> anyhow::Result<()> {
        self.mpv.set_property("pause", paused)?;
        Ok(())
    }
}
