use crate::Antialiasing;
use crate::core::{self, Font, Pixels};

/// The settings of a renderer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Settings {
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

    /// Whether or not to synchronize frames.
    ///
    /// By default, it is `true`.
    pub vsync: bool,

    /// The pixel scale factor for retro pixel effects.
    ///
    /// When set to a value greater than 1, the scene will be rendered to a
    /// downscaled texture and then upscaled with nearest-neighbor filtering,
    /// creating a pixelated retro look.
    ///
    /// By default, it is `1` (no pixel scaling).
    pub pixel_scale: u32,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            default_font: Font::default(),
            default_text_size: Pixels(16.0),
            antialiasing: None,
            vsync: true,
            pixel_scale: 1,
        }
    }
}

impl From<core::Settings> for Settings {
    fn from(settings: core::Settings) -> Self {
        Self {
            default_font: if cfg!(all(
                target_arch = "wasm32",
                feature = "fira-sans"
            )) && settings.default_font == Font::default()
            {
                Font::with_name("Fira Sans")
            } else {
                settings.default_font
            },
            default_text_size: settings.default_text_size,
            antialiasing: settings.antialiasing.then_some(Antialiasing::MSAAx4),
            vsync: settings.vsync,
            pixel_scale: settings.pixel_scale,
        }
    }
}
