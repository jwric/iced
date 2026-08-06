use iced::advanced::graphics::color;
use iced::advanced::graphics::mesh::{Mesh, SolidVertex2D};
use iced::advanced::layout::{self, Layout};
use iced::advanced::renderer;
use iced::advanced::widget::{self, Widget};
use iced::advanced::{Clipboard, Shell};
use iced::mouse;
use iced::widget::{center, column, text};
use iced::{
    Element, Length, PixelScaleMode, Rectangle, Renderer as IcedRenderer, Size,
    Theme, Transformation, Vector,
};

pub fn main() -> iced::Result {
    iced::application(State::new, State::update, State::view)
        .pixel_scale(PixelScaleMode::Auto(2))
        .antialiasing(false)
        .subscription(State::subscription)
        .run()
}

use iced::{Point, Subscription, window};

struct State {
    cursor: Option<Point>,
}

#[derive(Debug, Clone)]
enum Message {
    Tick,
    CursorMoved(Option<Point>),
}

impl State {
    fn new() -> Self {
        Self { cursor: None }
    }

    fn update(&mut self, msg: Message) {
        match msg {
            Message::Tick => { /* nothing to do, just triggers redraw */ }
            Message::CursorMoved(pos) => {
                self.cursor = pos;
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        center(
            column![
                PixelLineWidget { cursor: self.cursor },
                text("A pixel-perfect white line, one end follows your cursor every tick.\n\
Pixel scale is set to Auto(2), so each virtual pixel is shown as a 2x2 block.\
\nYou should see a single row of 'fat' pixels, with no double lines or anti-aliasing.")
                    .size(20)
                    .width(Length::Fill)
            ]
            .spacing(30)
            .width(Length::Fill)
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }

    fn subscription(&self) -> Subscription<Message> {
        use iced::event::Event;
        Subscription::batch([
            window::frames().map(|_| Message::Tick),
            iced::event::listen().filter_map(|event| match event {
                Event::Mouse(mouse_event) => match mouse_event {
                    iced::mouse::Event::CursorMoved { position } => {
                        Some(Message::CursorMoved(Some(position)))
                    }
                    iced::mouse::Event::CursorLeft => {
                        Some(Message::CursorMoved(None))
                    }
                    _ => None,
                },
                _ => None,
            }),
        ])
    }
}

#[derive(Debug, Clone, Copy)]
struct PixelLineWidget {
    cursor: Option<Point>,
}

impl<Message> Widget<Message, Theme, IcedRenderer> for PixelLineWidget {
    fn size(&self) -> Size<Length> {
        Size {
            width: Length::Fill,
            height: Length::Fixed(60.0),
        }
    }

    fn layout(
        &mut self,
        _tree: &mut widget::Tree,
        _renderer: &IcedRenderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let size = limits
            .width(Length::Fill)
            .height(Length::Fixed(60.0))
            .resolve(Length::Fill, Length::Fixed(60.0), iced::Size::ZERO);

        layout::Node::new(size)
    }

    fn update(
        &mut self,
        _state: &mut widget::Tree,
        _event: &iced::Event,
        _layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _renderer: &IcedRenderer,
        _clipboard: &mut dyn Clipboard,
        _shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        // No interaction
    }

    fn draw(
        &self,
        _tree: &widget::Tree,
        renderer: &mut IcedRenderer,
        _theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
        use iced::advanced::Renderer as _;
        use iced::advanced::graphics::mesh::Renderer as _;
        let bounds = layout.bounds();

        // Draw a pixel-perfect white line, one end follows the cursor
        // With PixelScaleMode::Auto(2), each virtual pixel is rendered as a 2x2 block

        let y = 20.0;
        let x0 = 10.0;
        let (x1, y1) = if let Some(cursor) = self.cursor.map(Point::round) {
            (
                cursor.x.max(0.0).min(bounds.width),
                cursor.y.max(0.0).min(bounds.height),
            )
        } else {
            (110.0, y)
        };

        // Calculate the direction and length for a 1-pixel-thick line
        let dx = x1 - x0;
        let dy = y1 - y;
        let len = (dx * dx + dy * dy).sqrt().max(1.0);
        let nx = -dy / len;
        let ny = dx / len;

        // Offset for 1-pixel thickness
        let px = nx * 0.5;
        let py = ny * 0.5;

        let mesh = Mesh::Solid {
            buffers: iced::advanced::graphics::mesh::Indexed {
                vertices: vec![
                    SolidVertex2D {
                        position: [x0 + px, y + py],
                        color: color::pack([1.0, 1.0, 1.0, 1.0]), // White
                    },
                    SolidVertex2D {
                        position: [x1 + px, y1 + py],
                        color: color::pack([1.0, 1.0, 1.0, 1.0]), // White
                    },
                    SolidVertex2D {
                        position: [x0 - px, y - py],
                        color: color::pack([1.0, 1.0, 1.0, 1.0]), // White
                    },
                    SolidVertex2D {
                        position: [x1 - px, y1 - py],
                        color: color::pack([1.0, 1.0, 1.0, 1.0]), // White
                    },
                ],
                // Two triangles forming a 1-pixel-thick line (rectangle)
                indices: vec![0, 1, 2, 1, 3, 2],
            },
            transformation: Transformation::IDENTITY,
            clip_bounds: Rectangle::INFINITE,
        };

        renderer.with_translation(
            Vector::new(bounds.x, bounds.y),
            |renderer| {
                renderer.draw_mesh(mesh);
            },
        );
    }
}

impl<'a, Message> From<PixelLineWidget> for Element<'a, Message>
where
    Message: 'a,
{
    fn from(widget: PixelLineWidget) -> Self {
        Self::new(widget)
    }
}
