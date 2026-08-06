//! An interface rendered as pixel art.
//!
//! With a pixel scale, the whole interface is laid out and drawn into a low
//! resolution framebuffer and then upscaled with nearest-neighbor filtering—so
//! a virtual pixel always covers an exact square of physical pixels, like in
//! Aseprite.
//!
//! Every renderer supports it. Run this example with `ICED_BACKEND=tiny-skia`
//! to see the software renderer produce the very same image.
use iced::widget::{
    button, center_x, checkbox, column, container, image, pick_list, row,
    slider, text,
};
use iced::{Center, Element, Fill, PixelScaleMode, Theme};

use grid::grid;
use sprite::sprite;

pub fn main() -> iced::Result {
    iced::application(PixelArt::default, PixelArt::update, PixelArt::view)
        .title("Pixel Art - Iced")
        .theme(PixelArt::theme)
        .window_size((600.0, 560.0))
        // Each virtual pixel takes about 3 logical pixels, no matter the
        // density of the display. A window of a fixed logical size therefore
        // fits the same amount of virtual pixels everywhere.
        .pixel_scale(PixelScaleMode::Auto(3))
        .antialiasing(false)
        .run()
}

struct PixelArt {
    density: u8,
    is_inverted: bool,
    theme: Theme,
}

impl Default for PixelArt {
    fn default() -> Self {
        Self {
            density: 0,
            is_inverted: false,
            theme: Theme::Nightfly,
        }
    }
}

#[derive(Debug, Clone)]
enum Message {
    DensityChanged(u8),
    InvertToggled(bool),
    ThemeSelected(Theme),
    Reset,
}

impl PixelArt {
    fn update(&mut self, message: Message) {
        match message {
            Message::DensityChanged(density) => self.density = density,
            Message::InvertToggled(is_inverted) => {
                self.is_inverted = is_inverted;
            }
            Message::ThemeSelected(theme) => self.theme = theme,
            Message::Reset => *self = Self::default(),
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let controls = row![
            checkbox(self.is_inverted)
                .label("Invert")
                .on_toggle(Message::InvertToggled),
            slider(0..=8, self.density, Message::DensityChanged),
            button("Reset").on_press(Message::Reset),
        ]
        .spacing(8)
        .align_y(Center);

        let theme = row![
            text("Theme"),
            pick_list(Theme::ALL, Some(&self.theme), Message::ThemeSelected),
        ]
        .spacing(8)
        .align_y(Center);

        // An 8x8 sprite blown up 5x. It sits next to text of an odd width, so
        // it lands on a fractional position—exactly the case that used to
        // shift an image by a whole source pixel.
        let mascot = row![
            image(sprite())
                .width(40)
                .height(40)
                .filter_method(image::FilterMethod::Nearest),
            text("nearest-neighbor, on the grid").size(8),
        ]
        .spacing(5)
        .align_y(Center);

        center_x(
            column![
                text("Every pixel is a square").size(12),
                container(
                    grid(self.density, self.is_inverted).width(Fill).height(64)
                )
                .padding(1)
                .style(container::bordered_box),
                mascot,
                controls,
                theme,
            ]
            .spacing(8)
            .max_width(200),
        )
        .padding(8)
        .into()
    }

    fn theme(&self) -> Theme {
        self.theme.clone()
    }
}

/// A hand-drawn sprite, built straight from raw pixels.
mod sprite {
    use iced::widget::image;

    /// The classic 8x8 smiley, as an [`image::Handle`].
    pub fn sprite() -> image::Handle {
        const ART: [&str; 8] = [
            "..YYYY..", ".YYYYYY.", "YY.YY.YY", "YYYYYYYY", "YY.YY.YY",
            "YYY..YYY", ".YYYYYY.", "..YYYY..",
        ];

        let pixels: Vec<u8> = ART
            .iter()
            .flat_map(|row| row.chars())
            .flat_map(|pixel| match pixel {
                'Y' => [0xFF, 0xCC, 0x33, 0xFF],
                _ => [0x00, 0x00, 0x00, 0x00],
            })
            .collect();

        image::Handle::from_rgba(8, 8, pixels)
    }
}

/// A grid of single pixel details.
///
/// Stripes one virtual pixel wide are the harshest test of an upscale: if a
/// single one of them ends up wider than its neighbors, the whole pattern
/// starts to beat and shimmer.
mod grid {
    use iced::advanced::layout::{self, Layout};
    use iced::advanced::renderer;
    use iced::advanced::widget::{self, Widget};
    use iced::advanced::{self, mouse};
    use iced::{Color, Element, Length, Rectangle, Size, Theme};

    pub fn grid(density: u8, is_inverted: bool) -> Grid {
        Grid {
            density,
            is_inverted,
            width: Length::Fill,
            height: Length::Fill,
        }
    }

    pub struct Grid {
        density: u8,
        is_inverted: bool,
        width: Length,
        height: Length,
    }

    impl Grid {
        pub fn width(mut self, width: impl Into<Length>) -> Self {
            self.width = width.into();
            self
        }

        pub fn height(mut self, height: impl Into<Length>) -> Self {
            self.height = height.into();
            self
        }
    }

    impl<Message, Renderer> Widget<Message, Theme, Renderer> for Grid
    where
        Renderer: advanced::Renderer,
    {
        fn size(&self) -> Size<Length> {
            Size::new(self.width, self.height)
        }

        fn layout(
            &mut self,
            _tree: &mut widget::Tree,
            _renderer: &Renderer,
            limits: &layout::Limits,
        ) -> layout::Node {
            layout::atomic(limits, self.width, self.height)
        }

        fn draw(
            &self,
            _tree: &widget::Tree,
            renderer: &mut Renderer,
            theme: &Theme,
            _style: &renderer::Style,
            layout: Layout<'_>,
            _cursor: mouse::Cursor,
            _viewport: &Rectangle,
        ) {
            let bounds = layout.bounds();
            let palette = theme.palette();

            let (background, foreground) = if self.is_inverted {
                (palette.text, palette.background)
            } else {
                (palette.background, palette.text)
            };

            renderer.fill_quad(
                renderer::Quad {
                    bounds,
                    ..renderer::Quad::default()
                },
                background,
            );

            // One pixel wide vertical stripes, spaced further and further apart
            let period = 2 + u32::from(self.density);

            for column in 0..(bounds.width as u32) {
                if column % period != 0 {
                    continue;
                }

                renderer.fill_quad(
                    renderer::Quad {
                        bounds: Rectangle {
                            x: bounds.x + column as f32,
                            y: bounds.y,
                            width: 1.0,
                            height: bounds.height / 2.0,
                        },
                        ..renderer::Quad::default()
                    },
                    foreground,
                );
            }

            // A checkerboard of single pixels, the classic dither pattern
            let checkerboard = Rectangle {
                y: bounds.y + bounds.height / 2.0,
                height: bounds.height / 2.0,
                ..bounds
            };

            for y in 0..(checkerboard.height as u32) {
                for x in 0..(checkerboard.width as u32) {
                    if (x + y) % 2 != 0 {
                        continue;
                    }

                    renderer.fill_quad(
                        renderer::Quad {
                            bounds: Rectangle {
                                x: checkerboard.x + x as f32,
                                y: checkerboard.y + y as f32,
                                width: 1.0,
                                height: 1.0,
                            },
                            ..renderer::Quad::default()
                        },
                        Color {
                            a: foreground.a * 0.9,
                            ..foreground
                        },
                    );
                }
            }
        }
    }

    impl<'a, Message, Renderer> From<Grid> for Element<'a, Message, Theme, Renderer>
    where
        Renderer: advanced::Renderer + 'a,
        Message: 'a,
    {
        fn from(grid: Grid) -> Self {
            Element::new(grid)
        }
    }
}
