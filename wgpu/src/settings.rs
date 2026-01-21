//! Configure a renderer.
use crate::core::{self, Font, PixelScaleMode, Pixels};
use crate::graphics::{self, Antialiasing};

/// The settings of a [`Renderer`].
///
/// [`Renderer`]: crate::Renderer
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Settings {
    /// The present mode of the [`Renderer`].
    ///
    /// [`Renderer`]: crate::Renderer
    pub present_mode: wgpu::PresentMode,

    /// The graphics backends to use.
    pub backends: wgpu::Backends,

    /// The default [`Font`] to use.
    pub default_font: Font,

    /// The default size of text.
    ///
    /// By default, it will be set to `16.0`.
    pub default_text_size: Pixels,

    /// The antialiasing strategy that will be used for triangle primitives.
    ///
    /// By default, it is `None`.
    pub antialiasing: Option<Antialiasing>,

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
    pub crt_effects: Option<core::CrtEffectSettings>,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            present_mode: wgpu::PresentMode::AutoVsync,
            backends: wgpu::Backends::all(),
            default_font: Font::default(),
            default_text_size: Pixels(16.0),
            antialiasing: None,
            pixel_scale: PixelScaleMode::default(),
            crt_effects: None,
        }
    }
}

impl From<graphics::Settings> for Settings {
    fn from(settings: graphics::Settings) -> Self {
        Self {
            present_mode: if settings.vsync {
                wgpu::PresentMode::AutoVsync
            } else {
                wgpu::PresentMode::AutoNoVsync
            },
            default_font: settings.default_font,
            default_text_size: settings.default_text_size,
            antialiasing: settings.antialiasing,
            pixel_scale: settings.pixel_scale,
            crt_effects: settings.crt_effects,
            ..Settings::default()
        }
    }
}

/// Obtains a [`wgpu::PresentMode`] from the current environment
/// configuration, if set.
///
/// The value returned by this function can be changed by setting
/// the `ICED_PRESENT_MODE` env variable. The possible values are:
///
/// - `vsync` → [`wgpu::PresentMode::AutoVsync`]
/// - `no_vsync` → [`wgpu::PresentMode::AutoNoVsync`]
/// - `immediate` → [`wgpu::PresentMode::Immediate`]
/// - `fifo` → [`wgpu::PresentMode::Fifo`]
/// - `fifo_relaxed` → [`wgpu::PresentMode::FifoRelaxed`]
/// - `mailbox` → [`wgpu::PresentMode::Mailbox`]
impl Settings {
    /// Sets the CRT effect settings.
    ///
    /// Enables and configures retro CRT monitor effects like scanlines,
    /// screen curvature, and color separation.
    pub fn crt_effects(mut self, crt_effects: core::CrtEffectSettings) -> Self {
        self.crt_effects = Some(crt_effects);
        self
    }

    /// Enables CRT effects with default settings.
    pub fn with_crt_effects(mut self) -> Self {
        self.crt_effects = Some(core::CrtEffectSettings::default());
        self
    }
}

pub fn present_mode_from_env() -> Option<wgpu::PresentMode> {
    let present_mode = std::env::var("ICED_PRESENT_MODE").ok()?;

    match present_mode.to_lowercase().as_str() {
        "vsync" => Some(wgpu::PresentMode::AutoVsync),
        "no_vsync" => Some(wgpu::PresentMode::AutoNoVsync),
        "immediate" => Some(wgpu::PresentMode::Immediate),
        "fifo" => Some(wgpu::PresentMode::Fifo),
        "fifo_relaxed" => Some(wgpu::PresentMode::FifoRelaxed),
        "mailbox" => Some(wgpu::PresentMode::Mailbox),
        _ => None,
    }
}
