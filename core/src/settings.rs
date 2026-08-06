//! Configure your application.
use crate::{Font, Pixels};

use std::borrow::Cow;

/// The strategy used to pick the __pixel scale__ of an interface.
///
/// The pixel scale is the amount of physical pixels that a single __virtual
/// pixel__ occupies on screen. When it is greater than 1, the interface is
/// laid out and rendered in virtual pixels—into a low resolution
/// framebuffer—and then upscaled with nearest-neighbor filtering; producing
/// the crisp, chunky look of pixel art applications like Aseprite.
///
/// A pixel scale is always an integer. This is what guarantees that every
/// virtual pixel ends up covering the exact same amount of physical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelScaleMode {
    /// Use a fixed pixel scale, no matter the display.
    ///
    /// `Fixed(1)` disables pixel scaling entirely.
    ///
    /// Note that a fixed pixel scale ignores the scale factor of the display;
    /// which means an interface will look smaller on a high density display.
    /// Use [`Auto`](Self::Auto) if you want to keep its apparent size
    /// consistent instead.
    Fixed(u32),

    /// Derive the pixel scale from the scale factor of the display, targeting
    /// the given amount of __logical__ pixels per virtual pixel.
    ///
    /// For instance, `Auto(2)` makes a virtual pixel take approximately the
    /// space of 2 logical pixels; which means a pixel scale of `2` on a
    /// regular display and `4` on a 2x high density display.
    ///
    /// This is normally what you want, since it keeps the apparent size of an
    /// interface consistent across displays while staying pixel-perfect.
    Auto(u32),
}

impl PixelScaleMode {
    /// A [`PixelScaleMode`] that disables pixel scaling.
    pub const NONE: Self = Self::Fixed(1);

    /// Resolves the [`PixelScaleMode`] into a pixel scale for a display with
    /// the given scale factor.
    ///
    /// The result is always greater than or equal to 1.
    pub fn resolve(self, scale_factor: f32) -> u32 {
        match self {
            Self::Fixed(pixel_scale) => pixel_scale.max(1),
            Self::Auto(target) => {
                if !scale_factor.is_finite() || scale_factor <= 0.0 {
                    return target.max(1);
                }

                let pixel_scale = (target as f32 * scale_factor).round();

                if pixel_scale >= 1.0 {
                    pixel_scale as u32
                } else {
                    1
                }
            }
        }
    }

    /// Returns true if the [`PixelScaleMode`] can never produce a pixel scale
    /// greater than 1.
    pub fn is_disabled(self) -> bool {
        matches!(self, Self::Fixed(pixel_scale) if pixel_scale <= 1)
    }
}

impl Default for PixelScaleMode {
    fn default() -> Self {
        Self::NONE
    }
}

impl From<u32> for PixelScaleMode {
    fn from(pixel_scale: u32) -> Self {
        Self::Fixed(pixel_scale)
    }
}

/// Configuration for CRT post-processing effects.
///
/// CRT effects are currently only supported by the `wgpu` renderer. They are
/// silently ignored by any other renderer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CrtEffectSettings {
    /// Scanline intensity (0.0 = none, 1.0 = maximum).
    pub scanline_intensity: f32,
    /// Screen curvature amount (0.0 = flat, higher = more curved).
    pub screen_curvature: f32,
    /// RGB color separation/chromatic aberration in pixels (0.0 = none).
    /// Specifies the horizontal offset in pixels at the source texture resolution.
    pub rgb_separation: f32,
    /// Vignette strength (0.0 = none, 1.0 = strong darkening at edges).
    pub vignette_strength: f32,
    /// Brightness adjustment (1.0 = normal).
    pub brightness: f32,
    /// Contrast adjustment (1.0 = normal).
    pub contrast: f32,
}

impl Default for CrtEffectSettings {
    fn default() -> Self {
        Self {
            scanline_intensity: 0.15,
            screen_curvature: 0.15,
            rgb_separation: 1.5, // 1.5 pixels of separation
            vignette_strength: 0.3,
            brightness: 1.0,
            contrast: 1.0,
        }
    }
}

/// The settings of an iced program.
#[derive(Debug, Clone)]
pub struct Settings {
    /// The identifier of the application.
    ///
    /// If provided, this identifier may be used to identify the application or
    /// communicate with it through the windowing system.
    pub id: Option<String>,

    /// The fonts to load on boot.
    pub fonts: Vec<Cow<'static, [u8]>>,

    /// The default [`Font`] to be used.
    ///
    /// By default, it uses [`Family::SansSerif`](crate::font::Family::SansSerif).
    pub default_font: Font,

    /// The text size that will be used by default.
    ///
    /// The default value is `16.0`.
    pub default_text_size: Pixels,

    /// If set to true, the renderer will try to perform antialiasing for some
    /// primitives.
    ///
    /// Enabling it can produce a smoother result in some widgets, like the
    /// `canvas` widget, at a performance cost.
    ///
    /// By default, it is enabled.
    pub antialiasing: bool,

    /// Whether or not to attempt to synchronize rendering when possible.
    ///
    /// Disabling it can improve rendering performance on some platforms.
    ///
    /// By default, it is enabled.
    pub vsync: bool,

    /// The [`PixelScaleMode`] of the application.
    ///
    /// When the resolved pixel scale is greater than 1, the interface is laid
    /// out and rendered into a low resolution framebuffer and then upscaled
    /// with nearest-neighbor filtering; producing crisp, pixel art-style
    /// graphics.
    ///
    /// A pixel scale replaces the scale factor of the display, instead of
    /// compounding with it. Use [`PixelScaleMode::Auto`] to derive it from the
    /// display and keep the apparent size of the interface consistent.
    ///
    /// By default, it is [`PixelScaleMode::NONE`] (no pixel scaling).
    pub pixel_scale: PixelScaleMode,

    /// The CRT post-processing effects of the application.
    ///
    /// Configure properties like scanlines, screen curvature, and color
    /// separation for an authentic retro CRT monitor look.
    ///
    /// Only supported by the `wgpu` renderer; ignored by any other renderer.
    ///
    /// By default, CRT effects are disabled (`None`).
    pub crt_effects: Option<CrtEffectSettings>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            id: None,
            fonts: Vec::new(),
            default_font: Font::default(),
            default_text_size: Pixels(16.0),
            antialiasing: true,
            vsync: true,
            pixel_scale: PixelScaleMode::default(),
            crt_effects: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_pixel_scale_ignores_the_scale_factor() {
        let mode = PixelScaleMode::Fixed(3);

        assert_eq!(mode.resolve(1.0), 3);
        assert_eq!(mode.resolve(1.5), 3);
        assert_eq!(mode.resolve(2.0), 3);
    }

    #[test]
    fn automatic_pixel_scale_follows_the_scale_factor() {
        let mode = PixelScaleMode::Auto(2);

        assert_eq!(mode.resolve(1.0), 2);
        assert_eq!(mode.resolve(1.5), 3);
        assert_eq!(mode.resolve(2.0), 4);

        // Halfway cases round away from zero
        assert_eq!(mode.resolve(1.25), 3);
        assert_eq!(mode.resolve(1.75), 4);
    }

    #[test]
    fn a_pixel_scale_is_never_smaller_than_one() {
        assert_eq!(PixelScaleMode::Fixed(0).resolve(1.0), 1);
        assert_eq!(PixelScaleMode::Auto(0).resolve(1.0), 1);

        // A tiny scale factor must not disable rendering altogether
        assert_eq!(PixelScaleMode::Auto(1).resolve(0.1), 1);
    }

    #[test]
    fn a_nonsensical_scale_factor_falls_back_to_the_target() {
        let mode = PixelScaleMode::Auto(3);

        assert_eq!(mode.resolve(f32::NAN), 3);
        assert_eq!(mode.resolve(0.0), 3);
        assert_eq!(mode.resolve(-1.0), 3);
    }

    #[test]
    fn pixel_scaling_is_disabled_by_default() {
        assert_eq!(PixelScaleMode::default(), PixelScaleMode::NONE);
        assert!(PixelScaleMode::default().is_disabled());
        assert_eq!(PixelScaleMode::default().resolve(2.0), 1);

        assert!(!PixelScaleMode::Auto(1).is_disabled());
        assert!(!PixelScaleMode::Fixed(2).is_disabled());
    }

    #[test]
    fn a_bare_integer_is_a_fixed_pixel_scale() {
        assert_eq!(PixelScaleMode::from(4), PixelScaleMode::Fixed(4));
    }
}
