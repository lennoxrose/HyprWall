//! A zone's offscreen render target: one RGBA texture (backed by an FBO)
//! sized to the zone's bounding box, plus a per-monitor blit that copies the
//! sub-rect of that texture corresponding to one member monitor's logical
//! position into that monitor's currently-bound (via `MonitorSurface::
//! make_current`) default framebuffer.
//!
//! This is what makes a multi-monitor zone show one continuous image instead
//! of the same frame repeated on every monitor: mpv renders into this single
//! shared texture once per frame, and each monitor just gets a different crop
//! of it.

use glow::HasContext;

use crate::monitor::Rect;

pub struct ZoneTarget {
    pub fbo: glow::Framebuffer,
    pub texture: glow::Texture,
    pub bounding_box: Rect,
}

impl ZoneTarget {
    /// Allocates a new RGBA8 texture + FBO sized to `bounding_box`. The
    /// caller must have already made a context sharing `gl`'s namespace
    /// current (see `RenderResources::apply_set_outcome`).
    pub fn new(gl: &glow::Context, bounding_box: Rect) -> anyhow::Result<Self> {
        unsafe {
            let texture = gl.create_texture().map_err(|e| anyhow::anyhow!(e))?;
            gl.bind_texture(glow::TEXTURE_2D, Some(texture));
            gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                glow::RGBA8 as i32,
                bounding_box.w,
                bounding_box.h,
                0,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                glow::PixelUnpackData::Slice(None),
            );
            gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MIN_FILTER, glow::LINEAR as i32);
            gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MAG_FILTER, glow::LINEAR as i32);
            gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_S, glow::CLAMP_TO_EDGE as i32);
            gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_T, glow::CLAMP_TO_EDGE as i32);

            let fbo = gl.create_framebuffer().map_err(|e| anyhow::anyhow!(e))?;
            gl.bind_framebuffer(glow::FRAMEBUFFER, Some(fbo));
            gl.framebuffer_texture_2d(
                glow::FRAMEBUFFER,
                glow::COLOR_ATTACHMENT0,
                glow::TEXTURE_2D,
                Some(texture),
                0,
            );

            Ok(Self { fbo, texture, bounding_box })
        }
    }

    /// The FBO's raw GL object name, in the form `libmpv2::render::
    /// RenderContext::render`'s `fbo: i32` parameter wants it (0 means the
    /// default framebuffer to mpv; this is never 0 since `glow::Framebuffer`
    /// wraps a `NonZeroU32`).
    pub fn fbo_raw(&self) -> i32 {
        self.fbo.0.get() as i32
    }

    /// Blits the sub-rect of this zone's texture that corresponds to one
    /// member monitor's logical position into that monitor's
    /// currently-bound default framebuffer.
    ///
    /// A single-monitor zone has `monitor_logical == bounding_box`, so
    /// `src_x0`/`src_y0` are always `0, 0` there -- no special case, the same
    /// code path handles the single- and multi-monitor case.
    pub fn blit_region(&self, gl: &glow::Context, monitor_logical: Rect) {
        let src_x0 = monitor_logical.x - self.bounding_box.x;
        let src_y0 = monitor_logical.y - self.bounding_box.y;
        unsafe {
            gl.bind_framebuffer(glow::READ_FRAMEBUFFER, Some(self.fbo));
            gl.bind_framebuffer(glow::DRAW_FRAMEBUFFER, None); // caller's current default framebuffer
            gl.blit_framebuffer(
                src_x0,
                src_y0,
                src_x0 + monitor_logical.w,
                src_y0 + monitor_logical.h,
                0,
                0,
                monitor_logical.w,
                monitor_logical.h,
                glow::COLOR_BUFFER_BIT,
                glow::LINEAR,
            );
        }
    }

    /// Deletes the underlying GL objects. Not a `Drop` impl: glow's object
    /// handles carry no reference to a GL context, so deleting them safely
    /// requires the caller to first make a context that shares this
    /// texture/FBO's namespace current -- something only the caller (which
    /// holds the `MonitorSurface`s) can guarantee. Called from
    /// `RenderResources` right before a `ZoneTarget` is dropped, while a
    /// member monitor's surface is current.
    pub fn destroy(&self, gl: &glow::Context) {
        unsafe {
            gl.delete_framebuffer(self.fbo);
            gl.delete_texture(self.texture);
        }
    }
}
