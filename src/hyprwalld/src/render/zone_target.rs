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

            Ok(Self {
                fbo,
                texture,
                bounding_box,
            })
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
        let (src_x0, src_y0) = src_rect_for_monitor(self.bounding_box, monitor_logical);
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

/// Computes the `(src_x0, src_y0)` lower-left corner of the sub-rect within
/// this zone's GL framebuffer that `monitor_logical` should be blitted from.
/// Pulled out of [`ZoneTarget::blit_region`] so the coordinate math is
/// testable without a real GL context.
///
/// `bounding_box`/`monitor_logical` are both in Wayland `xdg-output`
/// `logical_position` space: top-left origin, Y increases **downward**.
/// `glBlitFramebuffer`'s source rectangle is in GL framebuffer pixel space:
/// bottom-left origin, Y increases **upward**. `MpvInstance::render_to_fbo`
/// renders with `flip=true` specifically so the video's (and therefore the
/// whole zone's) top row lands at *high* Y in the FBO -- so the zone's
/// Wayland-space top edge (`bounding_box.y`) corresponds to GL Y =
/// `bounding_box.h`, and the zone's Wayland-space bottom edge corresponds to
/// GL Y = `0`. Combining the two coordinate systems without accounting for
/// this inversion silently picks the wrong vertical half of the shared
/// texture for any monitor pair that isn't at the same logical Y (invisible
/// for side-by-side layouts, where every monitor's height equals the
/// bounding box's height and `src_y0` comes out to `0` either way).
///
/// `src_x0` needs no such conversion -- X increases rightward in both
/// coordinate systems.
fn src_rect_for_monitor(bounding_box: Rect, monitor_logical: Rect) -> (i32, i32) {
    let src_x0 = monitor_logical.x - bounding_box.x;
    // Distance from the zone's Wayland-space top edge down to this
    // monitor's Wayland-space top edge.
    let dist_from_top = monitor_logical.y - bounding_box.y;
    // The monitor's *bottom* edge (`dist_from_top + monitor_logical.h` down
    // from the zone's top) maps to the *lower* GL Y bound of the source
    // rect, since GL Y decreases going down the picture while Wayland's Y
    // increases going down the picture.
    let src_y0 = bounding_box.h - dist_from_top - monitor_logical.h;
    (src_x0, src_y0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn side_by_side_same_y_monitors_have_zero_src_y() {
        // Two 1920x1080 monitors side by side at the same logical Y -- the
        // only layout tested live on this machine. Each monitor's height
        // equals the bounding box's height, so both should sample from GL Y
        // = 0 (matches the behavior this codebase has actually verified
        // on real hardware).
        let bounding_box = Rect {
            x: 0,
            y: 0,
            w: 3840,
            h: 1080,
        };
        let left = Rect {
            x: 0,
            y: 0,
            w: 1920,
            h: 1080,
        };
        let right = Rect {
            x: 1920,
            y: 0,
            w: 1920,
            h: 1080,
        };

        assert_eq!(src_rect_for_monitor(bounding_box, left), (0, 0));
        assert_eq!(src_rect_for_monitor(bounding_box, right), (1920, 0));
    }

    #[test]
    fn vertically_stacked_monitors_sample_opposite_halves() {
        // Top monitor (Wayland y = 0..1000) must sample the *top* of the GL
        // texture (high Y, since flip=true puts the video's top row at high
        // Y); the bottom monitor (y = 1000..1800) must sample the bottom
        // (low Y). Before the fix, both got src_y0 = 0 unconditionally --
        // correct for the top monitor only by coincidence here, and wrong
        // for the bottom monitor (which would have shown the top half of
        // the video instead of the bottom half).
        let bounding_box = Rect {
            x: 0,
            y: 0,
            w: 1920,
            h: 1800,
        };
        let top = Rect {
            x: 0,
            y: 0,
            w: 1920,
            h: 1000,
        };
        let bottom = Rect {
            x: 0,
            y: 1000,
            w: 1920,
            h: 800,
        };

        assert_eq!(src_rect_for_monitor(bounding_box, top), (0, 800));
        assert_eq!(src_rect_for_monitor(bounding_box, bottom), (0, 0));
    }

    #[test]
    fn single_monitor_zone_samples_from_origin() {
        // monitor_logical == bounding_box (the un-spanned case): always
        // (0, 0) regardless of logical position, and regardless of the
        // Y-axis fix.
        let r = Rect {
            x: 100,
            y: 200,
            w: 1920,
            h: 1080,
        };
        assert_eq!(src_rect_for_monitor(r, r), (0, 0));
    }

    #[test]
    fn vertically_offset_bounding_box_accounts_for_box_origin() {
        // bounding_box.y is non-zero (the topmost member monitor isn't at
        // Wayland y = 0), so the formula must subtract bounding_box.y, not
        // just use monitor_logical.y directly.
        let bounding_box = Rect {
            x: 0,
            y: 500,
            w: 1920,
            h: 1800,
        };
        let top = Rect {
            x: 0,
            y: 500,
            w: 1920,
            h: 1000,
        };
        let bottom = Rect {
            x: 0,
            y: 1500,
            w: 1920,
            h: 800,
        };

        assert_eq!(src_rect_for_monitor(bounding_box, top), (0, 800));
        assert_eq!(src_rect_for_monitor(bounding_box, bottom), (0, 0));
    }
}
