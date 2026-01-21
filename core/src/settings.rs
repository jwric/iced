//! Configure your application.
use crate::{Font, Pixels};

use std::borrow::Cow;

/// The pixel scale mode for retro pixel effects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelScaleMode {
    /// Automatically adjust pixel scale based on monitor DPI to maintain a target physical pixel size.
    ///
    /// The `u32` value represents the target size in logical pixels (at 96 DPI) that each
    /// virtual pixel should appear on screen. For example:
    /// - `Auto(2)` means each virtual pixel should be approximately 2 logical pixels
    /// - `Auto(3)` means each virtual pixel should be approximately 3 logical pixels
    ///
    /// The actual pixel scale will be automatically calculated as an integer value
    /// based on the monitor's DPI to maintain pixel-perfect rendering while
    /// achieving the desired visual size.
    Auto(u32),

    /// Use a fixed pixel scale factor regardless of monitor DPI.
    ///
    /// The `u32` value is the pixel scale factor directly (1 = no scaling).
    Fixed(u32),
}

impl PixelScaleMode {
    /// Calculate the actual pixel scale to use given the monitor's scale factor.
    ///
    /// For `Auto` mode, this computes an integer scale that makes virtual pixels
    /// appear close to the target physical size.
    /// For `Fixed` mode, this returns the fixed value.
    pub fn calculate_pixel_scale(&self, scale_factor: f64) -> u32 {
        match self {
            PixelScaleMode::Auto(target_size) => {
                // Calculate the pixel scale that achieves the target size
                // scale_factor represents DPI/96 (e.g., 2.0 for 192 DPI)
                // We want: virtual_pixel_size = target_size
                // Since virtual pixels are scaled by pixel_scale in logical space,
                // and logical pixels are scaled by scale_factor to physical pixels:
                // pixel_scale = target_size (at 1.0 scale_factor)
                // At higher DPI, we want to maintain the same physical size, so:
                // pixel_scale = (target_size * scale_factor).round()
                let calculated =
                    ((*target_size as f64) * scale_factor).round() as u32;
                calculated.max(1)
            }
            PixelScaleMode::Fixed(scale) => (*scale).max(1),
        }
    }
}

impl Default for PixelScaleMode {
    fn default() -> Self {
        PixelScaleMode::Fixed(1)
    }
}

/// Configuration for CRT post-processing effects.
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

    /// The pixel scale mode for retro pixel effects.
    ///
    /// This determines how pixel scaling is applied:
    /// - `Auto(n)`: Automatically adjusts pixel scale based on monitor DPI to maintain
    ///   a target visual size. The value `n` is the target size in logical pixels.
    /// - `Fixed(n)`: Uses a fixed pixel scale factor regardless of monitor DPI.
    ///
    /// When pixel scale is greater than 1, the scene will be rendered to a
    /// downscaled texture and then upscaled with nearest-neighbor filtering,
    /// creating a pixelated retro look.
    ///
    /// By default, it is `Fixed(1)` (no pixel scaling).
    pub pixel_scale: PixelScaleMode,

    /// CRT post-processing effect settings.
    ///
    /// These effects are applied when pixel scaling is enabled.
    /// Configure properties like scanlines, screen curvature, and color separation
    /// for an authentic retro CRT monitor look.
    ///
    /// Set to `Some(settings)` to enable, `None` to disable.
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
    fn test_pixel_scale_mode_fixed() {
        let mode = PixelScaleMode::Fixed(3);

        // Fixed mode should return the same value regardless of scale factor
        assert_eq!(mode.calculate_pixel_scale(1.0), 3);
        assert_eq!(mode.calculate_pixel_scale(1.5), 3);
        assert_eq!(mode.calculate_pixel_scale(2.0), 3);
    }

    #[test]
    fn test_pixel_scale_mode_auto() {
        let mode = PixelScaleMode::Auto(2);

        // Auto mode should scale with DPI
        assert_eq!(mode.calculate_pixel_scale(1.0), 2); // 2 * 1.0 = 2
        assert_eq!(mode.calculate_pixel_scale(1.5), 3); // 2 * 1.5 = 3
        assert_eq!(mode.calculate_pixel_scale(2.0), 4); // 2 * 2.0 = 4
    }

    #[test]
    fn test_pixel_scale_mode_auto_rounding() {
        let mode = PixelScaleMode::Auto(2);

        // Test rounding behavior
        assert_eq!(mode.calculate_pixel_scale(1.25), 3); // 2 * 1.25 = 2.5 -> 3
        assert_eq!(mode.calculate_pixel_scale(1.75), 4); // 2 * 1.75 = 3.5 -> 4
    }

    #[test]
    fn test_pixel_scale_mode_minimum() {
        // Should never return less than 1
        let mode_fixed = PixelScaleMode::Fixed(0);
        assert_eq!(mode_fixed.calculate_pixel_scale(1.0), 1);

        let mode_auto = PixelScaleMode::Auto(0);
        assert_eq!(mode_auto.calculate_pixel_scale(1.0), 1);
    }

    #[test]
    fn test_pixel_scale_mode_default() {
        let mode = PixelScaleMode::default();
        assert_eq!(mode, PixelScaleMode::Fixed(1));
    }
}
