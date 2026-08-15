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

use hyprwall_config::model::{FitMode, WallpaperSettings};
use libmpv2::Mpv;
use libmpv2::render::{OpenGLInitParams, RenderContext, RenderParam, RenderParamApiType, mpv_render_update};

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
            // Silent until `apply_wallpaper_settings` (below) decides
            // otherwise -- never a frame where a freshly-loaded video could
            // be audible before that first settings-apply call. Not paired
            // with a hard `ao=null` (as this used to be, back when
            // wallpapers had no audio policy at all): a real audio device is
            // what makes per-wallpaper volume possible.
            init.set_option("mute", "yes")?;
            // Without this, `mpv_render_context_render` blocks until the
            // frame's scheduled display time, which would stall every *other*
            // zone rendered from the same thread.
            init.set_option("video-timing-offset", "0")?;
            // A still image (one video frame, no audio) hits EOF after mpv's
            // default display duration; `loop-file=inf` above would then
            // restart it forever, producing a perpetual wakeup-ping/redraw
            // cycle for a picture that never changes. This option only
            // applies to files mpv classifies as a still image -- video
            // files and animated gif/webp are unaffected.
            init.set_option("image-display-duration", "inf")?;
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

    /// Applies every per-picture display setting to this zone's mpv
    /// instance in one call. `zoom` is converted from `WallpaperSettings`'
    /// linear factor (1.0 = no zoom) into mpv's own log2 `video-zoom`
    /// space here, at the boundary -- nowhere else needs to know mpv's
    /// convention. `fit` maps onto mpv's `keepaspect`/`panscan` pair:
    /// `Contain` keeps aspect and letterboxes (panscan 0), `Cover` keeps
    /// aspect and crops to fill (panscan 1), `Stretch` drops aspect
    /// entirely. `mute` is derived from `volume` rather than tracked
    /// separately -- 0 volume and "muted" are the same state here.
    pub fn apply_wallpaper_settings(&self, settings: &WallpaperSettings) -> anyhow::Result<()> {
        self.mpv.set_property("video-zoom", settings.zoom.max(0.001).log2())?;
        self.mpv.set_property("video-pan-x", settings.pan_x)?;
        self.mpv.set_property("video-pan-y", settings.pan_y)?;
        let (keepaspect, panscan) = match settings.fit {
            FitMode::Contain => (true, 0.0),
            FitMode::Cover => (true, 1.0),
            FitMode::Stretch => (false, 0.0),
        };
        self.mpv.set_property("keepaspect", keepaspect)?;
        self.mpv.set_property("panscan", panscan)?;
        self.mpv.set_property("volume", settings.volume.clamp(0.0, 100.0))?;
        self.mpv.set_property("mute", settings.volume <= 0.0)?;
        self.mpv
            .set_property("brightness", settings.brightness.clamp(-100.0, 100.0))?;
        self.mpv
            .set_property("contrast", settings.contrast.clamp(-100.0, 100.0))?;
        self.mpv.set_property("hue", settings.hue.clamp(-100.0, 100.0))?;
        self.mpv
            .set_property("saturation", settings.saturation.clamp(-100.0, 100.0))?;
        Ok(())
    }
}
