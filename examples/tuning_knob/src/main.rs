//! This example showcases a custom tuning knob widget that can be rotated
//! by dragging with the mouse. The widget is self-contained in an inline
//! module for easy copy-pasting into your own projects.

// ============================================================================
// KNOB WIDGET MODULE - Copy this entire module into your project!
// ============================================================================
//
// This module is self-contained and can be copied directly into your own
// iced application. It has no dependencies other than iced itself.
//
// Usage example:
//   use knob::knob;
//
//   knob(0.0..=100.0, current_value, Message::ValueChanged)
//       .size(120.0)
//       .step(0.1)
// ============================================================================

mod knob {
    use iced::advanced::layout::{self, Layout};
    use iced::advanced::mouse;
    use iced::advanced::renderer;
    use iced::advanced::widget::Widget;
    use iced::advanced::widget::tree::{self, Tree};
    use iced::advanced::{Clipboard, Renderer as _, Shell};
    use iced::border::Radius;
    use iced::event::Event;
    use iced::widget::canvas;
    use iced::{
        Border, Color, Element, Length, Point, Rectangle, Renderer, Shadow,
        Size,
    };

    /// A tuning knob widget that can be rotated by dragging vertically
    /// or adjusted with the mouse wheel.
    ///
    /// The knob displays a circular control with tick marks and a rotating
    /// indicator. It supports:
    /// - Vertical drag to adjust value
    /// - Mouse wheel scrolling
    /// - Touch input
    /// - Customizable size and step
    pub struct Knob<Message> {
        value: f32,
        range: std::ops::RangeInclusive<f32>,
        on_change: Box<dyn Fn(f32) -> Message>,
        size: f32,
        step: f32,
    }

    impl<Message> Knob<Message> {
        /// Creates a new [`Knob`].
        ///
        /// The knob will produce messages with the current value when dragged.
        pub fn new(
            range: std::ops::RangeInclusive<f32>,
            value: f32,
            on_change: impl Fn(f32) -> Message + 'static,
        ) -> Self {
            Self {
                value: value.clamp(*range.start(), *range.end()),
                range,
                on_change: Box::new(on_change),
                size: 80.0,
                step: 0.01,
            }
        }

        /// Sets the size of the knob.
        pub fn size(mut self, size: f32) -> Self {
            self.size = size;
            self
        }

        /// Sets the step size for value changes.
        pub fn step(mut self, step: f32) -> Self {
            self.step = step;
            self
        }

        /// Calculates the angle in radians based on the current value.
        /// Range: -2.5 radians to +2.5 radians (approximately 286 degrees total)
        fn angle(&self) -> f32 {
            let range_start = *self.range.start();
            let range_end = *self.range.end();
            let normalized =
                (self.value - range_start) / (range_end - range_start);

            // Map to angle range: -2.5 to +2.5 radians
            const MIN_ANGLE: f32 = -2.5;
            const MAX_ANGLE: f32 = 2.5;
            MIN_ANGLE + normalized * (MAX_ANGLE - MIN_ANGLE)
        }
    }

    /// Internal state for tracking drag interactions and caching geometry
    struct State {
        is_dragging: bool,
        drag_start_y: f32,
        drag_start_value: f32,
        cache: canvas::Cache,
    }

    impl Default for State {
        fn default() -> Self {
            Self {
                is_dragging: false,
                drag_start_y: 0.0,
                drag_start_value: 0.0,
                cache: canvas::Cache::default(),
            }
        }
    }

    impl<Message, Theme> Widget<Message, Theme, Renderer> for Knob<Message> {
        fn tag(&self) -> tree::Tag {
            tree::Tag::of::<State>()
        }

        fn state(&self) -> tree::State {
            tree::State::new(State::default())
        }

        fn size(&self) -> Size<Length> {
            Size {
                width: Length::Fixed(self.size),
                height: Length::Fixed(self.size),
            }
        }

        fn layout(
            &mut self,
            _tree: &mut Tree,
            _renderer: &Renderer,
            _limits: &layout::Limits,
        ) -> layout::Node {
            layout::Node::new(Size::new(self.size, self.size))
        }

        fn draw(
            &self,
            tree: &Tree,
            renderer: &mut Renderer,
            _theme: &Theme,
            _style: &renderer::Style,
            layout: Layout<'_>,
            cursor: mouse::Cursor,
            _viewport: &Rectangle,
        ) {
            let state = tree.state.downcast_ref::<State>();
            let bounds: Rectangle = layout
                .bounds()
                .snap()
                .map(Into::into)
                .unwrap_or(layout.bounds());
            let center = bounds.center().round();
            let radius = self.size / 2.0;

            // Determine if hovering
            let is_hovered = cursor.is_over(bounds);

            // Draw outer circle (knob body)
            let knob_color = if is_hovered {
                Color::from_rgb(0.3, 0.3, 0.35)
            } else {
                Color::from_rgb(0.25, 0.25, 0.3)
            };

            renderer.fill_quad(
                renderer::Quad {
                    bounds,
                    border: Border {
                        color: Color::from_rgb(0.15, 0.15, 0.2),
                        width: 2.0,
                        radius: Radius::from(radius),
                    },
                    shadow: Shadow {
                        color: Color::from_rgba(0.0, 0.0, 0.0, 0.3),
                        offset: iced::Vector::new(0.0, 2.0),
                        blur_radius: 4.0,
                    },
                    snap: true,
                },
                knob_color,
            );

            // Draw inner highlight circle for depth
            let highlight_radius = radius * 0.85;
            let highlight_bounds = Rectangle {
                x: center.x - highlight_radius,
                y: center.y - highlight_radius,
                width: highlight_radius * 2.0,
                height: highlight_radius * 2.0,
            };

            renderer.fill_quad(
                renderer::Quad {
                    bounds: highlight_bounds,
                    border: Border {
                        color: Color::TRANSPARENT,
                        width: 0.0,
                        radius: Radius::from(highlight_radius),
                    },
                    shadow: Shadow::default(),
                    snap: true,
                },
                Color::from_rgb(0.28, 0.28, 0.33),
            );

            // Draw tick marks and indicator using canvas geometry
            let geometry = state.cache.draw(renderer, bounds.size(), |frame| {
                let center = frame.center().round();

                // Draw tick marks around the knob
                let num_ticks = 11;
                let tick_width = 2.0;
                let tick_height = radius * 0.1;
                let tick_distance = radius * 0.75;
                let tick_color = Color::from_rgb(0.5, 0.5, 0.55);

                for i in 0..num_ticks {
                    let tick_angle =
                        -2.5 + (5.0 / (num_ticks - 1) as f32) * i as f32;

                    frame.with_save(|frame| {
                        frame.translate(iced::Vector::new(center.x, center.y));
                        frame.rotate(tick_angle);
                        frame.translate(iced::Vector::new(0.0, -tick_distance));

                        let tick_rect = canvas::Path::rectangle(
                            Point::new(-tick_width / 2.0, -tick_height / 2.0),
                            Size::new(tick_width, tick_height),
                        );

                        frame.fill(&tick_rect, tick_color);
                    });
                }

                // Draw the indicator line
                let angle = self.angle();
                let indicator_width = 1.0;
                let indicator_length = radius * 0.35;
                let indicator_distance = radius * 0.475;

                frame.with_save(|frame| {
                    frame.translate(iced::Vector::new(center.x, center.y));
                    frame.rotate(angle);
                    frame
                        .translate(iced::Vector::new(0.0, -indicator_distance));

                    let indicator_rect = canvas::Path::rectangle(
                        Point::new(
                            -indicator_width / 2.0,
                            -indicator_length / 2.0,
                        ),
                        Size::new(indicator_width, indicator_length),
                    );

                    frame.fill(&indicator_rect, Color::from_rgb(0.9, 0.5, 0.2));
                });
            });

            renderer.with_translation(
                iced::Vector::new(bounds.x, bounds.y),
                |renderer| {
                    use iced::advanced::graphics::geometry::Renderer as _;

                    renderer.draw_geometry(geometry);
                },
            );

            // Draw center cap
            let cap_radius = radius * 0.2;
            let cap_bounds = Rectangle {
                x: center.x - cap_radius,
                y: center.y - cap_radius,
                width: cap_radius * 2.0,
                height: cap_radius * 2.0,
            };

            renderer.fill_quad(
                renderer::Quad {
                    bounds: cap_bounds,
                    border: Border {
                        color: Color::from_rgb(0.15, 0.15, 0.2),
                        width: 0.5,
                        radius: Radius::from(cap_radius),
                    },
                    shadow: Shadow::default(),
                    snap: true,
                },
                Color::from_rgb(0.35, 0.35, 0.4),
            );
        }

        fn update(
            &mut self,
            tree: &mut Tree,
            event: &Event,
            layout: Layout<'_>,
            cursor: mouse::Cursor,
            _renderer: &Renderer,
            _clipboard: &mut dyn Clipboard,
            shell: &mut Shell<'_, Message>,
            _viewport: &Rectangle,
        ) {
            let state = tree.state.downcast_mut::<State>();
            let bounds = layout.bounds();

            match event {
                Event::Mouse(mouse::Event::ButtonPressed(
                    mouse::Button::Left,
                ))
                | Event::Touch(iced::touch::Event::FingerPressed { .. }) => {
                    if cursor.is_over(bounds) {
                        if let Some(position) = cursor.position() {
                            state.is_dragging = true;
                            state.drag_start_y = position.y;
                            state.drag_start_value = self.value;
                        }
                    }
                }
                Event::Mouse(mouse::Event::ButtonReleased(
                    mouse::Button::Left,
                ))
                | Event::Touch(iced::touch::Event::FingerLifted { .. })
                | Event::Touch(iced::touch::Event::FingerLost { .. }) => {
                    if state.is_dragging {
                        state.is_dragging = false;
                    }
                }
                Event::Mouse(mouse::Event::CursorMoved { .. })
                | Event::Touch(iced::touch::Event::FingerMoved { .. }) => {
                    if state.is_dragging {
                        if let Some(position) = cursor.position() {
                            // Calculate value change based on vertical drag
                            let delta_y = state.drag_start_y - position.y;
                            let sensitivity = 0.005; // Adjust for drag sensitivity

                            let range_start = *self.range.start();
                            let range_end = *self.range.end();
                            let range_size = range_end - range_start;

                            let mut new_value = state.drag_start_value
                                + delta_y * sensitivity * range_size;

                            // Apply step
                            if self.step > 0.0 {
                                new_value =
                                    (new_value / self.step).round() * self.step;
                            }

                            // Clamp to range
                            new_value = new_value.clamp(range_start, range_end);

                            if (new_value - self.value).abs() > f32::EPSILON {
                                state.cache.clear();
                                shell.publish((self.on_change)(new_value));
                            }
                        }
                    }
                }
                Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
                    if cursor.is_over(bounds) {
                        let delta_value = match delta {
                            mouse::ScrollDelta::Lines { y, .. } => {
                                y * self.step * 5.0
                            }
                            mouse::ScrollDelta::Pixels { y, .. } => {
                                y * self.step * 0.5
                            }
                        };

                        let range_start = *self.range.start();
                        let range_end = *self.range.end();
                        let mut new_value = self.value + delta_value;

                        // Apply step
                        if self.step > 0.0 {
                            new_value =
                                (new_value / self.step).round() * self.step;
                        }

                        // Clamp to range
                        new_value = new_value.clamp(range_start, range_end);

                        if (new_value - self.value).abs() > f32::EPSILON {
                            state.cache.clear();
                            shell.publish((self.on_change)(new_value));
                        }
                    }
                }
                _ => {}
            }
        }
    }

    impl<'a, Message, Theme> From<Knob<Message>>
        for Element<'a, Message, Theme, Renderer>
    where
        Message: 'a,
    {
        fn from(knob: Knob<Message>) -> Self {
            Self::new(knob)
        }
    }

    /// Creates a new [`Knob`] widget.
    ///
    /// # Arguments
    /// * `range` - The range of values the knob can represent
    /// * `value` - The current value (will be clamped to range)
    /// * `on_change` - Callback function that receives the new value
    ///
    /// # Example
    /// ```ignore
    /// knob(0.0..=100.0, volume, Message::VolumeChanged)
    ///     .size(80.0)
    ///     .step(0.5)
    /// ```
    pub fn knob<Message>(
        range: std::ops::RangeInclusive<f32>,
        value: f32,
        on_change: impl Fn(f32) -> Message + 'static,
    ) -> Knob<Message> {
        Knob::new(range, value, on_change)
    }
}

// ============================================================================
// END OF KNOB WIDGET MODULE
// ============================================================================

use iced::widget::{center, column, container, row, text};
use iced::{Center, Element, PixelScaleMode};
use knob::knob;

pub fn main() -> iced::Result {
    iced::application(TuningKnob::new, TuningKnob::update, TuningKnob::view)
        .window_size((600.0, 450.0))
        .pixel_scale(PixelScaleMode::Auto(2))
        .antialiasing(false)
        .run()
}

struct TuningKnob {
    frequency: f32,
    volume: f32,
    pan: f32,
}

#[derive(Debug, Clone, Copy)]
enum Message {
    FrequencyChanged(f32),
    VolumeChanged(f32),
    PanChanged(f32),
}

impl TuningKnob {
    fn new() -> Self {
        TuningKnob {
            frequency: 440.0,
            volume: 0.7,
            pan: 0.0,
        }
    }

    fn update(&mut self, message: Message) {
        match message {
            Message::FrequencyChanged(value) => {
                self.frequency = value;
            }
            Message::VolumeChanged(value) => {
                self.volume = value;
            }
            Message::PanChanged(value) => {
                self.pan = value;
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let frequency_knob = column![
            knob(20.0..=20000.0, self.frequency, Message::FrequencyChanged)
                .size(31.0)
                .step(1.0),
            text!("Frequency").size(14),
            text!("{:.0} Hz", self.frequency).size(16),
        ]
        .spacing(10)
        .align_x(Center);

        let volume_knob = column![
            knob(0.0..=1.0, self.volume, Message::VolumeChanged)
                .size(31.0)
                .step(0.01),
            text!("Volume").size(14),
            text!("{:.0}%", self.volume * 100.0).size(16),
        ]
        .spacing(10)
        .align_x(Center);

        let pan_knob = column![
            knob(-1.0..=1.0, self.pan, Message::PanChanged)
                .size(31.0)
                .step(0.01),
            text!("Pan").size(14),
            text!(
                "{}",
                if self.pan < -0.05 {
                    format!("L {:.0}%", -self.pan * 100.0)
                } else if self.pan > 0.05 {
                    format!("R {:.0}%", self.pan * 100.0)
                } else {
                    "Center".to_string()
                }
            )
            .size(16),
        ]
        .spacing(10)
        .align_x(Center);

        let controls = row![frequency_knob, volume_knob, pan_knob]
            .spacing(40)
            .padding(20);

        let content = column![
            text!("Tuning Knob Widget Example").size(28),
            text!("Drag knobs vertically or use mouse wheel to adjust values")
                .size(14),
            container(controls).padding(20),
        ]
        .spacing(20)
        .align_x(Center);

        center(content).into()
    }
}

impl Default for TuningKnob {
    fn default() -> Self {
        Self::new()
    }
}
