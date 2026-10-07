use crate::core::Rectangle;

#[derive(Debug, Clone, PartialEq)]
pub enum Primitive {
    /// A path filled with some paint.
    Fill {
        /// The path to fill.
        path: tiny_skia::Path,
        /// The paint to use.
        paint: tiny_skia::Paint<'static>,
        /// The fill rule to follow.
        rule: tiny_skia::FillRule,
    },
    /// A path stroked with some paint.
    Stroke {
        /// The path to stroke.
        path: tiny_skia::Path,
        /// The paint to use.
        paint: tiny_skia::Paint<'static>,
        /// The stroke settings.
        stroke: tiny_skia::Stroke,
    },
}

impl Primitive {
    /// Returns the visible bounds of the [`Primitive`].
    ///
    /// They contain every pixel it touches: a stroke reaches half its width
    /// past its path (further at a miter or a square cap's corner), and
    /// anti-aliasing a pixel more.
    pub fn visible_bounds(&self) -> Rectangle {
        let (bounds, reach) = match self {
            Primitive::Fill { path, .. } => (path.bounds(), 0.0),
            Primitive::Stroke { path, stroke, .. } => {
                let join = match stroke.line_join {
                    tiny_skia::LineJoin::Miter | tiny_skia::LineJoin::MiterClip => {
                        stroke.miter_limit
                    }
                    tiny_skia::LineJoin::Round | tiny_skia::LineJoin::Bevel => 1.0,
                };
                let cap = match stroke.line_cap {
                    tiny_skia::LineCap::Square => std::f32::consts::SQRT_2,
                    tiny_skia::LineCap::Butt | tiny_skia::LineCap::Round => 1.0,
                };

                (path.bounds(), stroke.width / 2.0 * join.max(cap))
            }
        };

        Rectangle {
            x: bounds.x(),
            y: bounds.y(),
            width: bounds.width(),
            height: bounds.height(),
        }
        .expand(reach + 1.0)
    }
}
