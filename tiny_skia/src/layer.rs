use crate::Primitive;
use crate::core::renderer::Quad;
use crate::core::{self, Background, Color, Point, Rectangle, Svg, Transformation};
use crate::graphics::damage;
use crate::graphics::layer;
use crate::graphics::text::{Editor, Paragraph, Text};
use crate::graphics::{self, Image};

use std::sync::Arc;

pub type Stack = layer::Stack<Layer>;

#[derive(Debug, Clone)]
pub struct Layer {
    pub bounds: Rectangle,
    pub quads: Vec<(Quad, Background)>,
    pub primitives: Vec<Item<Primitive>>,
    pub images: Vec<Image>,
    pub text: Vec<Item<Text>>,
}

impl Layer {
    pub fn draw_quad(
        &mut self,
        mut quad: Quad,
        background: Background,
        transformation: Transformation,
    ) {
        quad.bounds = quad.bounds * transformation;
        self.quads.push((quad, background));
    }

    pub fn draw_paragraph(
        &mut self,
        paragraph: &Paragraph,
        position: Point,
        color: Color,
        clip_bounds: Rectangle,
        transformation: Transformation,
    ) {
        let paragraph = Text::Paragraph {
            paragraph: paragraph.downgrade(),
            position,
            color,
            clip_bounds,
            transformation,
        };

        self.text.push(Item::Live(paragraph));
    }

    pub fn draw_editor(
        &mut self,
        editor: &Editor,
        position: Point,
        color: Color,
        clip_bounds: Rectangle,
        transformation: Transformation,
    ) {
        let editor = Text::Editor {
            editor: editor.downgrade(),
            position,
            color,
            clip_bounds,
            transformation,
        };

        self.text.push(Item::Live(editor));
    }

    pub fn draw_text(
        &mut self,
        text: core::Text,
        position: Point,
        color: Color,
        clip_bounds: Rectangle,
        transformation: Transformation,
    ) {
        let text = Text::Cached {
            content: text.content,
            bounds: Rectangle::new(position, text.bounds) * transformation,
            color,
            size: text.size * transformation.scale_factor(),
            line_height: text.line_height.to_absolute(text.size) * transformation.scale_factor(),
            font: text.font,
            align_x: text.align_x,
            align_y: text.align_y,
            shaping: text.shaping,
            wrapping: text.wrapping,
            ellipsis: text.ellipsis,
            clip_bounds: clip_bounds * transformation,
        };

        self.text.push(Item::Live(text));
    }

    pub fn draw_text_raw(&mut self, raw: graphics::text::Raw, transformation: Transformation) {
        let raw = Text::Raw {
            raw,
            transformation,
        };

        self.text.push(Item::Live(raw));
    }

    pub fn draw_text_group(
        &mut self,
        text: Vec<Text>,
        clip_bounds: Rectangle,
        transformation: Transformation,
    ) {
        self.text
            .push(Item::Group(text, clip_bounds, transformation));
    }

    pub fn draw_text_cache(
        &mut self,
        text: Arc<[Text]>,
        clip_bounds: Rectangle,
        transformation: Transformation,
    ) {
        self.text
            .push(Item::Cached(text, clip_bounds, transformation));
    }

    pub fn draw_image(&mut self, image: Image, transformation: Transformation) {
        match image {
            Image::Raster {
                image,
                bounds,
                clip_bounds,
            } => {
                self.draw_raster(image, bounds, clip_bounds, transformation);
            }
            Image::Vector {
                svg,
                bounds,
                clip_bounds,
            } => {
                self.draw_svg(svg, bounds, clip_bounds, transformation);
            }
        }
    }

    pub fn draw_raster(
        &mut self,
        image: core::Image,
        bounds: Rectangle,
        clip_bounds: Rectangle,
        transformation: Transformation,
    ) {
        let image = Image::Raster {
            image: core::Image {
                border_radius: image.border_radius * transformation.scale_factor(),
                ..image
            },
            bounds: bounds * transformation,
            clip_bounds: clip_bounds * transformation,
        };

        self.images.push(image);
    }

    pub fn draw_svg(
        &mut self,
        svg: Svg,
        bounds: Rectangle,
        clip_bounds: Rectangle,
        transformation: Transformation,
    ) {
        let svg = Image::Vector {
            svg,
            bounds: bounds * transformation,
            clip_bounds: clip_bounds * transformation,
        };

        self.images.push(svg);
    }

    pub fn draw_primitive_group(
        &mut self,
        primitives: Vec<Primitive>,
        clip_bounds: Rectangle,
        transformation: Transformation,
    ) {
        self.primitives.push(Item::Group(
            primitives,
            clip_bounds * transformation,
            transformation,
        ));
    }

    pub fn draw_primitive_cache(
        &mut self,
        primitives: Arc<[Primitive]>,
        clip_bounds: Rectangle,
        transformation: Transformation,
    ) {
        self.primitives.push(Item::Cached(
            primitives,
            clip_bounds * transformation,
            transformation,
        ));
    }

    /// Draws the primitives, images and text of a canvas [`Geometry`].
    ///
    /// [`Geometry`]: crate::Geometry
    #[cfg(feature = "geometry")]
    pub fn draw_geometry(&mut self, geometry: crate::Geometry, transformation: Transformation) {
        match geometry {
            crate::Geometry::Live {
                primitives,
                images,
                text,
                clip_bounds,
            } => {
                self.draw_primitive_group(primitives, clip_bounds, transformation);

                for image in images {
                    self.draw_image(image, transformation);
                }

                self.draw_text_group(text, clip_bounds, transformation);
            }
            crate::Geometry::Cache(cache) => {
                self.draw_primitive_cache(cache.primitives, cache.clip_bounds, transformation);

                for image in cache.images.iter() {
                    self.draw_image(image.clone(), transformation);
                }

                self.draw_text_cache(cache.text, cache.clip_bounds, transformation);
            }
        }
    }

    pub fn damage(previous: &Self, current: &Self) -> Vec<Rectangle> {
        if previous.bounds != current.bounds {
            return vec![previous.bounds, current.bounds];
        }

        let layer_bounds = current.bounds.expand(1.0);

        let mut damage = damage::list(
            &previous.quads,
            &current.quads,
            |(quad, _)| {
                let Some(bounds) = quad.bounds.expand(1.0).intersection(&layer_bounds) else {
                    return vec![];
                };

                vec![if quad.shadow.color.a > 0.0 {
                    bounds.expand(
                        quad.shadow.offset.x.abs().max(quad.shadow.offset.y.abs())
                            + quad.shadow.blur_radius,
                    )
                } else {
                    bounds
                }]
            },
            |(quad_a, background_a), (quad_b, background_b)| {
                quad_a == quad_b && background_a == background_b
            },
        );

        let text = damage::diff(
            &previous.text,
            &current.text,
            |item| {
                item.as_slice()
                    .iter()
                    .filter_map(Text::visible_bounds)
                    .map(|bounds| bounds * item.transformation())
                    .collect()
            },
            |text_a, text_b| {
                damage::list(
                    text_a.as_slice(),
                    text_b.as_slice(),
                    |text| {
                        text.visible_bounds()
                            .into_iter()
                            .map(|bounds| bounds * text_a.transformation())
                            .collect()
                    },
                    |text_a, text_b| text_a == text_b,
                )
            },
        );

        let primitives = damage::list(
            &previous.primitives,
            &current.primitives,
            |item| match item {
                Item::Live(primitive) => vec![primitive.visible_bounds()],
                Item::Group(primitives, group_bounds, transformation) => primitives
                    .as_slice()
                    .iter()
                    .map(Primitive::visible_bounds)
                    .map(|bounds| bounds * *transformation)
                    .filter_map(|bounds| bounds.intersection(group_bounds))
                    .collect(),
                Item::Cached(_primitives, bounds, _transformation) => {
                    vec![*bounds]
                }
            },
            |primitive_a, primitive_b| match (primitive_a, primitive_b) {
                (
                    Item::Cached(cache_a, bounds_a, transformation_a),
                    Item::Cached(cache_b, bounds_b, transformation_b),
                ) => {
                    Arc::ptr_eq(cache_a, cache_b)
                        && bounds_a == bounds_b
                        && transformation_a == transformation_b
                }
                _ => false,
            },
        );

        let images = damage::list(
            &previous.images,
            &current.images,
            |image| vec![image.bounds().expand(1.0)],
            Image::eq,
        );

        damage.extend(text);
        damage.extend(primitives);
        damage.extend(images);
        damage
    }
}

impl Default for Layer {
    fn default() -> Self {
        Self {
            bounds: Rectangle::INFINITE,
            quads: Vec::new(),
            primitives: Vec::new(),
            text: Vec::new(),
            images: Vec::new(),
        }
    }
}

impl graphics::Layer for Layer {
    fn with_bounds(bounds: Rectangle) -> Self {
        Self {
            bounds,
            ..Self::default()
        }
    }

    fn bounds(&self) -> Rectangle {
        self.bounds
    }

    fn flush(&mut self) {}

    fn resize(&mut self, bounds: Rectangle) {
        self.bounds = bounds;
    }

    fn reset(&mut self) {
        self.bounds = Rectangle::INFINITE;

        self.quads.clear();
        self.primitives.clear();
        self.text.clear();
        self.images.clear();
    }

    fn start(&self) -> usize {
        if !self.quads.is_empty() {
            return 1;
        }

        if !self.primitives.is_empty() {
            return 2;
        }

        if !self.images.is_empty() {
            return 3;
        }

        if !self.text.is_empty() {
            return 4;
        }

        usize::MAX
    }

    fn end(&self) -> usize {
        if !self.text.is_empty() {
            return 4;
        }

        if !self.images.is_empty() {
            return 3;
        }

        if !self.primitives.is_empty() {
            return 2;
        }

        if !self.quads.is_empty() {
            return 1;
        }

        0
    }

    fn merge(&mut self, layer: &mut Self) {
        self.quads.append(&mut layer.quads);
        self.primitives.append(&mut layer.primitives);
        self.text.append(&mut layer.text);
        self.images.append(&mut layer.images);
    }
}

#[derive(Debug, Clone)]
pub enum Item<T> {
    Live(T),
    Group(Vec<T>, Rectangle, Transformation),
    Cached(Arc<[T]>, Rectangle, Transformation),
}

impl<T> Item<T> {
    pub fn transformation(&self) -> Transformation {
        match self {
            Item::Live(_) => Transformation::IDENTITY,
            Item::Group(_, _, transformation) | Item::Cached(_, _, transformation) => {
                *transformation
            }
        }
    }

    pub fn clip_bounds(&self) -> Rectangle {
        match self {
            Item::Live(_) => Rectangle::INFINITE,
            Item::Group(_, clip_bounds, _) | Item::Cached(_, clip_bounds, _) => *clip_bounds,
        }
    }

    pub fn as_slice(&self) -> &[T] {
        match self {
            Item::Live(item) => std::slice::from_ref(item),
            Item::Group(group, _, _) => group.as_slice(),
            Item::Cached(cache, _, _) => cache,
        }
    }
}

#[cfg(all(test, feature = "geometry"))]
mod tests {
    use super::*;
    use crate::Geometry;
    use crate::core::Point;
    use crate::core::text::LineHeight;
    use crate::geometry::Frame;
    use crate::graphics::geometry::frame::Backend;
    use crate::graphics::geometry::{self, Path, Stroke};

    const VIEW: Rectangle = Rectangle {
        x: 0.0,
        y: 0.0,
        width: 400.0,
        height: 300.0,
    };

    fn frame(draw: impl FnOnce(&mut Frame)) -> Geometry {
        let mut frame = Frame::new(VIEW);
        draw(&mut frame);
        frame.into_geometry()
    }

    fn layer(geometries: Vec<Geometry>, transformation: Transformation) -> Layer {
        let mut layer = Layer {
            bounds: VIEW,
            ..Layer::default()
        };

        for geometry in geometries {
            layer.draw_geometry(geometry, transformation);
        }

        layer
    }

    fn text(frame: &mut Frame, content: &str, x: f32, y: f32) {
        frame.fill_text(geometry::Text {
            content: content.to_owned(),
            position: Point::new(x, y),
            max_width: f32::INFINITY,
            size: 11.0.into(),
            line_height: LineHeight::Absolute(12.0.into()),
            ..geometry::Text::default()
        });
    }

    /// The damage between two layers, grouped as the compositor groups it.
    fn damage(previous: &Layer, current: &Layer) -> Vec<Rectangle> {
        damage::group(Layer::damage(previous, current), VIEW)
    }

    fn covers(regions: &[Rectangle], point: Point) -> bool {
        regions.iter().any(|region| region.contains(point))
    }

    #[test]
    fn changed_canvas_text_damages_its_line_not_the_rest_of_the_canvas() {
        let before = layer(
            vec![frame(|f| text(f, "A", 10.0, 10.0))],
            Transformation::IDENTITY,
        );
        let after = layer(
            vec![frame(|f| text(f, "B", 10.0, 10.0))],
            Transformation::IDENTITY,
        );
        let regions = damage(&before, &after);

        assert!(
            Layer::damage(&before, &after)
                .iter()
                .all(|region| region.x.is_finite()
                    && region.width.is_finite()
                    && region.height.is_finite()),
            "{regions:?}"
        );
        assert!(covers(&regions, Point::new(13.0, 16.0)), "{regions:?}");
        assert!(!covers(&regions, Point::new(13.0, 100.0)), "{regions:?}");
    }

    #[test]
    fn a_thick_stroke_damages_its_whole_width() {
        let before = layer(Vec::new(), Transformation::IDENTITY);
        let after = layer(
            vec![frame(|f| {
                f.stroke(
                    &Path::line(Point::new(100.0, 100.0), Point::new(200.0, 100.0)),
                    Stroke::default().with_width(10.0),
                );
            })],
            Transformation::IDENTITY,
        );
        let regions = damage(&before, &after);

        assert!(covers(&regions, Point::new(150.0, 96.0)), "{regions:?}");
        assert!(covers(&regions, Point::new(150.0, 104.0)), "{regions:?}");
    }
}
