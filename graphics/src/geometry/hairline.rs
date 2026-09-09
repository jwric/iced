//! Draw one-pixel lines that cover one pixel.

use crate::core::{Point, Size};
use crate::geometry::Path;
use crate::geometry::Stroke;
use crate::geometry::path::lyon_path::{self, iterator::PathIterator};

/// Curves are flattened to within this distance of the true curve. A tenth of
/// a pixel never moves a sample into a different pixel.
const TOLERANCE: f32 = 0.1;

/// Whether `stroke` is drawn as the pixel [`spans`] it covers rather than as a
/// tessellated ribbon.
///
/// Only a stroke a pixel wide or thinner has a rasterizer's answer to fall
/// back on. A dash pattern keeps the tessellator too, since the pattern is
/// measured along the ribbon.
pub fn applies(stroke: &Stroke<'_>) -> bool {
    stroke.snap && stroke.width <= 1.0 && stroke.line_dash.segments.is_empty()
}

/// The pixels a one-pixel-wide stroke of `path` covers, as a set of
/// axis-aligned rectangles on integer coordinates.
///
/// Filling the result lights the pixels a rasterizer walking `path` would:
/// one per step along each segment's major axis, none twice. Stroking builds a
/// ribbon of constant *perpendicular* width instead, which measures
/// `1 / cos θ` across an axis and so covers two pixels at some angles.
///
/// `path` must already be in device space, where a pixel is the unit square
/// and the centre of the pixel at `(i, j)` is `(i + 0.5, j + 0.5)`.
pub fn spans(path: &Path) -> Path {
    // Neighbours in a row become one rectangle, so a shallow line costs a
    // handful of quads instead of one per pixel.
    let mut runs: Vec<(i32, i32, i32)> = Vec::new();

    for (x, y) in pixels(path) {
        match runs.last_mut() {
            Some((row, _, end)) if *row == y && x == *end + 1 => *end = x,
            _ => runs.push((y, x, x)),
        }
    }

    Path::new(|builder| {
        for (y, start, end) in runs {
            builder.rectangle(
                Point::new(start as f32, y as f32),
                Size::new((end - start + 1) as f32, 1.0),
            );
        }
    })
}

/// The pixels themselves, ordered by row and then by column, each one once.
fn pixels(path: &Path) -> Vec<(i32, i32)> {
    let mut pixels = Vec::new();
    let mut previous = None;

    for event in path.raw().iter().flattened(TOLERANCE) {
        match event {
            lyon_path::Event::Begin { .. } => previous = None,
            lyon_path::Event::Line { from, to } => {
                trace(from, to, &mut previous, &mut pixels);
            }
            lyon_path::Event::End { last, first, close } => {
                if close {
                    trace(last, first, &mut previous, &mut pixels);
                }

                previous = None;
            }
            // Flattening leaves no curves behind.
            lyon_path::Event::Quadratic { .. }
            | lyon_path::Event::Cubic { .. } => {}
        }
    }

    pixels.sort_unstable_by_key(|(x, y)| (*y, *x));
    pixels.dedup();
    pixels
}

/// Walks one segment, sampling it once per pixel along its major axis.
///
/// The minor axis then moves by at most one pixel per step, which is what
/// keeps a segment's own run unbroken.
fn trace(
    from: lyon_path::math::Point,
    to: lyon_path::math::Point,
    previous: &mut Option<(i32, i32)>,
    pixels: &mut Vec<(i32, i32)>,
) {
    let dx = to.x - from.x;
    let dy = to.y - from.y;

    // A zero-length segment draws nothing, the same as a butt-capped stroke of
    // one.
    if dx == 0.0 && dy == 0.0 {
        return;
    }

    let horizontal = dx.abs() >= dy.abs();
    let (start, end, span) = if horizontal {
        (from.x, to.x, dx)
    } else {
        (from.y, to.y, dy)
    };

    // Pixel centres sit at `i + 0.5`, so these are the pixels the segment
    // crosses the middle of — the ones a butt cap includes.
    let low = (start.min(end) - 0.5).ceil() as i32;
    let high = (start.max(end) - 0.5).floor() as i32;

    let mut walk = |i: i32| {
        let t = (i as f32 + 0.5 - start) / span;

        push(
            if horizontal {
                (i, (from.y + t * dy).floor() as i32)
            } else {
                ((from.x + t * dx).floor() as i32, i)
            },
            previous,
            pixels,
        );
    };

    // In the segment's own direction, so that the pixel a corner is bridged
    // from is the one the previous segment ended on.
    if span >= 0.0 {
        (low..=high).for_each(&mut walk);
    } else {
        (low..=high).rev().for_each(&mut walk);
    }
}

fn push(
    pixel: (i32, i32),
    previous: &mut Option<(i32, i32)>,
    pixels: &mut Vec<(i32, i32)>,
) {
    if let Some(last) = *previous {
        bridge(last, pixel, pixels);
    }

    pixels.push(pixel);
    *previous = Some(pixel);
}

/// Fills the gap between two segments meeting at a corner.
///
/// Each segment steps along its own major axis, so where they differ the last
/// pixel of one and the first of the next need not touch.
fn bridge(from: (i32, i32), to: (i32, i32), pixels: &mut Vec<(i32, i32)>) {
    let dx = to.0 - from.0;
    let dy = to.1 - from.1;
    let steps = dx.abs().max(dy.abs());

    if steps <= 1 {
        return;
    }

    for step in 1..steps {
        let t = step as f32 / steps as f32;

        pixels.push((
            from.0 + (dx as f32 * t).round() as i32,
            from.1 + (dy as f32 * t).round() as i32,
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(from: (f32, f32), to: (f32, f32)) -> Vec<(i32, i32)> {
        pixels(&Path::line(Point::new(from.0, from.1), Point::new(to.0, to.1)))
    }

    fn polyline(points: &[(f32, f32)]) -> Vec<(i32, i32)> {
        pixels(&Path::new(|builder| {
            builder.move_to(Point::new(points[0].0, points[0].1));

            for (x, y) in &points[1..] {
                builder.line_to(Point::new(*x, *y));
            }
        }))
    }

    /// Every pixel touches the next, so the line has no holes in it.
    fn is_unbroken(pixels: &[(i32, i32)]) -> bool {
        let mut sorted = pixels.to_vec();
        sorted.sort_unstable();

        sorted.iter().all(|(x, y)| {
            pixels.len() == 1
                || sorted.iter().any(|(other_x, other_y)| {
                    (other_x, other_y) != (x, y)
                        && (other_x - x).abs() <= 1
                        && (other_y - y).abs() <= 1
                })
        })
    }

    #[test]
    fn a_horizontal_line_covers_the_row_it_runs_along() {
        assert_eq!(
            line((2.0, 5.5), (6.0, 5.5)),
            [(2, 5), (3, 5), (4, 5), (5, 5)]
        );
    }

    /// A stroked ribbon centred on an integer straddles two rows and lands on
    /// whichever one the fill rule breaks the tie towards. A span picks one.
    #[test]
    fn a_row_is_the_one_the_line_runs_through() {
        assert_eq!(line((2.0, 5.0), (6.0, 5.0)), line((2.0, 5.4), (6.0, 5.4)));
        assert_eq!(line((2.0, 5.9), (6.0, 5.9)), line((2.0, 5.5), (6.0, 5.5)));
    }

    #[test]
    fn a_diagonal_covers_one_pixel_per_column() {
        let pixels = line((2.0, 2.0), (10.0, 10.0));

        assert_eq!(pixels.len(), 8);
        assert!(is_unbroken(&pixels));
    }

    /// The slope that made a stroked ribbon cover two pixels in eight of its
    /// columns: 88 columns, 88 pixels.
    #[test]
    fn a_shallow_diagonal_covers_one_pixel_per_column() {
        let pixels = line((2.0, 4.0), (90.0, 20.0));

        assert_eq!(pixels.len(), 88);
        assert!(is_unbroken(&pixels));
    }

    #[test]
    fn a_steep_diagonal_covers_one_pixel_per_row() {
        let pixels = line((4.0, 2.0), (20.0, 58.0));

        assert_eq!(pixels.len(), 56);
        assert!(is_unbroken(&pixels));
    }

    /// Two segments step along different axes, so the corner between them is
    /// the one place a run can come apart.
    #[test]
    fn a_corner_joins_the_segments_that_meet_at_it() {
        assert!(is_unbroken(&polyline(&[
            (2.5, 20.5),
            (6.5, 4.5),
            (10.5, 20.5),
            (22.5, 18.5),
        ])));
    }

    #[test]
    fn a_closed_path_covers_its_last_edge() {
        let square =
            polyline(&[(2.5, 2.5), (8.5, 2.5), (8.5, 8.5), (2.5, 8.5)]);
        let mut closed = pixels(&Path::rectangle(
            Point::new(2.5, 2.5),
            crate::core::Size::new(6.0, 6.0),
        ));

        closed.retain(|pixel| !square.contains(pixel));

        assert_eq!(closed, [(2, 3), (2, 4), (2, 5), (2, 6), (2, 7)]);
    }

    #[test]
    fn a_point_is_not_a_line() {
        assert!(line((4.0, 4.0), (4.0, 4.0)).is_empty());
    }
}
