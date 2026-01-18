//! Configure your application.
use crate::{Font, Pixels};

use std::borrow::Cow;

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

    /// The pixel scale factor for retro pixel effects.
    ///
    /// When set to a value greater than 1, the scene will be rendered to a
    /// downscaled texture and then upscaled with nearest-neighbor filtering,
    /// creating a pixelated retro look.
    ///
    /// By default, it is `1` (no pixel scaling).
    pub pixel_scale: u32,

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
            pixel_scale: 1,
            crt_effects: None,
        }
    }
}
