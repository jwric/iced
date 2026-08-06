use crate::conversion;
use crate::core::{Color, PixelScaleMode, Size};
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
    pixel_scale_mode: PixelScaleMode,
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
        pixel_scale_mode: PixelScaleMode,
    ) -> Self {
        let title = program.title(window_id);
        let scale_factor = program.scale_factor(window_id);
        let theme = program.theme(window_id);
        let theme_mode =
            theme.as_ref().map(theme::Base::mode).unwrap_or_default();
        let default_theme = <P::Theme as theme::Base>::default(system_theme);
        let style = program.style(theme.as_ref().unwrap_or(&default_theme));

        let viewport = {
            #[cfg(not(target_arch = "wasm32"))]
            let physical_size = window.inner_size();

            // Firefox for Android can expose
            // `devicePixelContentBoxSize` without ever delivering the
            // corresponding ResizeObserver callback. In that case, winit's
            // cached inner size remains 0x0 during startup even though the
            // canvas already fills the page. Seed the viewport directly from
            // the rendered canvas bounds so the compositor has a valid
            // surface before the first browser resize event.
            #[cfg(target_arch = "wasm32")]
            let physical_size = web_canvas_physical_size(window);

            viewport(
                Size::new(physical_size.width, physical_size.height),
                window.scale_factor() as f32 * scale_factor,
                pixel_scale_mode,
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
            pixel_scale_mode,
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

    pub fn scale_factor(&self) -> f32 {
        self.viewport.scale_factor()
    }

    pub fn cursor(&self) -> mouse::Cursor {
        self.cursor_position
            .map(|cursor_position| {
                conversion::cursor_position(
                    cursor_position,
                    self.viewport.scale_factor(),
                )
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
                let size = Size::new(new_size.width, new_size.height);

                self.viewport = viewport(
                    size,
                    window.scale_factor() as f32 * self.scale_factor,
                    self.pixel_scale_mode,
                );
                self.surface_version += 1;
            }
            WindowEvent::ScaleFactorChanged {
                scale_factor: new_scale_factor,
                ..
            } => {
                let size = self.viewport.physical_size();

                self.viewport = viewport(
                    size,
                    *new_scale_factor as f32 * self.scale_factor,
                    self.pixel_scale_mode,
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

    /// Synchronize a browser window with the canvas's rendered CSS bounds.
    ///
    /// Firefox for Android may not emit winit's ResizeObserver event when the
    /// visual viewport changes for the software keyboard. Polling at the next
    /// browser input event keeps the compositor surface and UI layout atomic.
    #[cfg(target_arch = "wasm32")]
    pub fn synchronize_web_viewport(&mut self, window: &Window) -> bool {
        let physical_size = web_canvas_physical_size(window);

        if self.viewport.physical_size()
            == Size::new(physical_size.width, physical_size.height)
        {
            return false;
        }

        self.viewport = viewport(
            Size::new(physical_size.width, physical_size.height),
            window.scale_factor() as f32 * self.scale_factor,
            self.pixel_scale_mode,
        );
        self.surface_version += 1;
        true
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
            self.viewport = viewport(
                self.viewport.physical_size(),
                window.scale_factor() as f32 * new_scale_factor,
                self.pixel_scale_mode,
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

/// Builds the [`Viewport`] of a window of the given physical `size`.
///
/// When `pixel_scale_mode` resolves to a pixel scale greater than 1, the
/// resulting [`Viewport`] renders to a low resolution framebuffer that a
/// compositor upscales with nearest-neighbor filtering. The pixel scale then
/// __replaces__ `scale_factor`, which is only used to derive it.
fn viewport(
    size: Size<u32>,
    scale_factor: f32,
    pixel_scale_mode: PixelScaleMode,
) -> Viewport {
    let pixel_scale = pixel_scale_mode.resolve(scale_factor);

    if pixel_scale > 1 {
        Viewport::with_pixel_scale(size, pixel_scale)
    } else {
        Viewport::with_physical_size(size, scale_factor)
    }
}

#[cfg(target_arch = "wasm32")]
fn web_canvas_physical_size(window: &Window) -> winit::dpi::PhysicalSize<u32> {
    use winit::platform::web::WindowExtWebSys;

    let bounds = window
        .canvas()
        .expect("Get window canvas")
        .get_bounding_client_rect();
    let scale = window.scale_factor();

    winit::dpi::PhysicalSize::new(
        (bounds.width() * scale).round().max(1.0) as u32,
        (bounds.height() * scale).round().max(1.0) as u32,
    )
}
