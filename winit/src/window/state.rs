use crate::conversion;
use crate::core::{Color, Size};
use crate::core::{mouse, theme, window};
use crate::graphics::Viewport;
use crate::program::{self, Program};

use winit::event::{Touch, WindowEvent};
use winit::window::Window;

use std::fmt::{Debug, Formatter};

/// The state of the window of a [`Program`].
pub struct State<P: Program>
where
    P::Theme: theme::Base,
{
    title: String,
    scale_factor: f32,
    viewport: Viewport,
    surface_version: u64,
    cursor_position: Option<winit::dpi::PhysicalPosition<f64>>,
    modifiers: winit::keyboard::ModifiersState,
    theme: Option<P::Theme>,
    theme_mode: theme::Mode,
    default_theme: P::Theme,
    style: theme::Style,
    pixel_scale: u32,
}

impl<P: Program> Debug for State<P>
where
    P::Theme: theme::Base,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("window::State")
            .field("title", &self.title)
            .field("scale_factor", &self.scale_factor)
            .field("viewport", &self.viewport)
            .field("cursor_position", &self.cursor_position)
            .field("style", &self.style)
            .finish()
    }
}

impl<P: Program> State<P>
where
    P::Theme: theme::Base,
{
    /// Creates a new [`State`] for the provided [`Program`]'s `window`.
    pub fn new(
        program: &program::Instance<P>,
        window_id: window::Id,
        window: &Window,
        system_theme: theme::Mode,
        pixel_scale: u32,
    ) -> Self {
        let title = program.title(window_id);
        let scale_factor = program.scale_factor(window_id);
        let theme = program.theme(window_id);
        let theme_mode =
            theme.as_ref().map(theme::Base::mode).unwrap_or_default();
        let default_theme = <P::Theme as theme::Base>::default(system_theme);
        let style = program.style(theme.as_ref().unwrap_or(&default_theme));

        let viewport = {
            let physical_size = window.inner_size();

            Viewport::with_physical_size(
                Size::new(physical_size.width, physical_size.height),
                window.scale_factor() as f32 * scale_factor,
            )
        };

        Self {
            title,
            scale_factor,
            viewport,
            surface_version: 0,
            cursor_position: None,
            modifiers: winit::keyboard::ModifiersState::default(),
            theme,
            theme_mode,
            default_theme,
            style,
            pixel_scale: pixel_scale.max(1),
        }
    }

    pub fn viewport(&self) -> &Viewport {
        &self.viewport
    }

    pub fn surface_version(&self) -> u64 {
        self.surface_version
    }

    pub fn physical_size(&self) -> Size<u32> {
        self.viewport.physical_size()
    }

    pub fn logical_size(&self) -> Size<f32> {
        self.viewport.logical_size()
    }

    /// Returns the scaled physical size of the window scaled by the pixel scale factor.
    /// This is the size that should be used for UI layout when pixel scaling is enabled.
    pub fn scaled_physical_size(&self) -> Size<f32> {
        let logical = self.viewport.physical_size();
        let scale = self.pixel_scale as u32;

        Size::new(
            (logical.width / scale) as f32,
            (logical.height / scale) as f32,
        )
    }

    pub fn scale_factor(&self) -> f32 {
        self.viewport.scale_factor()
    }

    pub fn pixel_scale(&self) -> u32 {
        self.pixel_scale
    }

    pub fn cursor(&self) -> mouse::Cursor {
        self.cursor_position
            .map(|cursor_position| {
                let mut point =
                    conversion::cursor_position(cursor_position, 1.0);

                // Adjust for pixel scaling - the cursor is in window space
                // but rendering happens at 1/pixel_scale resolution
                if self.pixel_scale > 1 {
                    point.x /= self.pixel_scale as f32;
                    point.y /= self.pixel_scale as f32;
                }

                point
            })
            .map(mouse::Cursor::Available)
            .unwrap_or(mouse::Cursor::Unavailable)
    }

    pub fn modifiers(&self) -> winit::keyboard::ModifiersState {
        self.modifiers
    }

    pub fn theme(&self) -> &P::Theme {
        self.theme.as_ref().unwrap_or(&self.default_theme)
    }

    pub fn theme_mode(&self) -> theme::Mode {
        self.theme_mode
    }

    pub fn background_color(&self) -> Color {
        self.style.background_color
    }

    pub fn text_color(&self) -> Color {
        self.style.text_color
    }

    pub fn update(
        &mut self,
        program: &program::Instance<P>,
        window: &Window,
        event: &WindowEvent,
    ) {
        match event {
            WindowEvent::Resized(new_size) => {
                let k = self.pixel_scale;

                // Use the actual physical size from winit
                let size = Size::new(new_size.width, new_size.height);

                if k > 1
                    && !window.is_maximized()
                    && window.fullscreen().is_none()
                {
                    let snapped_w = (size.width / k).max(1) * k;
                    let snapped_h = (size.height / k).max(1) * k;

                    if snapped_w != size.width || snapped_h != size.height {
                        let snapped =
                            winit::dpi::PhysicalSize::new(snapped_w, snapped_h);

                        // Request the snapped size...
                        let _ = window.request_inner_size(snapped);

                        // ...but DO NOT change viewport yet, because the window is still `new_size`.
                        // Wait for the next Resized event that matches `snapped`.
                        return;
                    }
                }

                // At this point, either snapping is disabled OR size is already snapped.
                self.viewport = Viewport::with_physical_size(
                    size,
                    window.scale_factor() as f32 * self.scale_factor,
                );
                self.surface_version += 1;
            }
            WindowEvent::ScaleFactorChanged {
                scale_factor: sf, ..
            } => {
                let new_inner_size = window.inner_size();
                let mut size =
                    Size::new(new_inner_size.width, new_inner_size.height);
                let k = self.pixel_scale;

                if k > 1
                    && !window.is_maximized()
                    && window.fullscreen().is_none()
                {
                    let snapped_w = (size.width / k).max(1) * k;
                    let snapped_h = (size.height / k).max(1) * k;

                    if snapped_w != size.width || snapped_h != size.height {
                        let snapped =
                            winit::dpi::PhysicalSize::new(snapped_w, snapped_h);
                        let _ = window.request_inner_size(snapped);
                        return;
                    }

                    size = Size::new(snapped_w, snapped_h);
                }

                self.viewport = Viewport::with_physical_size(
                    size,
                    *sf as f32 * self.scale_factor,
                );
                self.surface_version += 1;
            }
            WindowEvent::CursorMoved { position, .. }
            | WindowEvent::Touch(Touch {
                location: position, ..
            }) => {
                self.cursor_position = Some(*position);
            }
            WindowEvent::CursorLeft { .. } => {
                self.cursor_position = None;
            }
            WindowEvent::ModifiersChanged(new_modifiers) => {
                self.modifiers = new_modifiers.state();
            }
            WindowEvent::ThemeChanged(theme) => {
                self.default_theme = <P::Theme as theme::Base>::default(
                    conversion::theme_mode(*theme),
                );

                if self.theme.is_none() {
                    self.style = program.style(&self.default_theme);
                    window.request_redraw();
                }
            }
            _ => {}
        }
    }

    pub fn synchronize(
        &mut self,
        program: &program::Instance<P>,
        window_id: window::Id,
        window: &Window,
    ) {
        // Update window title
        let new_title = program.title(window_id);

        if self.title != new_title {
            window.set_title(&new_title);
            self.title = new_title;
        }

        // Update scale factor
        let new_scale_factor = program.scale_factor(window_id);

        if self.scale_factor != new_scale_factor {
            self.viewport = Viewport::with_physical_size(
                self.viewport.physical_size(),
                window.scale_factor() as f32 * new_scale_factor,
            );

            self.scale_factor = new_scale_factor;
        }

        // Update theme and appearance
        self.theme = program.theme(window_id);
        self.style = program.style(self.theme());

        let new_mode = self
            .theme
            .as_ref()
            .map(theme::Base::mode)
            .unwrap_or_default();

        if self.theme_mode != new_mode {
            #[cfg(not(target_os = "linux"))]
            {
                window.set_theme(conversion::window_theme(new_mode));

                // Assume the old mode matches the system one
                // We will be notified otherwise
                if new_mode == theme::Mode::None {
                    self.default_theme =
                        <P::Theme as theme::Base>::default(self.theme_mode);

                    if self.theme.is_none() {
                        self.style = program.style(&self.default_theme);
                    }
                }
            }

            #[cfg(target_os = "linux")]
            {
                // mundy always notifies system theme changes, so we
                // just restore the default theme mode.
                let new_mode = if new_mode == theme::Mode::None {
                    theme::Base::mode(&self.default_theme)
                } else {
                    new_mode
                };

                window.set_theme(conversion::window_theme(new_mode));
            }

            self.theme_mode = new_mode;
        }
    }
}
