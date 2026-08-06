//! Upscale a low resolution image into a surface, pixel art-style.
//!
//! Renderers that draw to a [`Viewport`] with a pixel scale greater than 1
//! produce an image of [`Viewport::target_size`], which must then be upscaled
//! to [`Viewport::physical_size`] with nearest-neighbor filtering.
//!
//! This module implements that upscaling on the CPU. GPU-accelerated
//! renderers generally use a blit pass instead—but they still need it to
//! produce screenshots.
//!
//! [`Viewport`]: crate::Viewport
//! [`Viewport::target_size`]: crate::Viewport::target_size
//! [`Viewport::physical_size`]: crate::Viewport::physical_size
use crate::core::{Rectangle, Size};

/// Upscales a `source` image into a `target` image by an integer `scale`,
/// with nearest-neighbor filtering.
///
/// Only the given `region` of the `target` is written to; which lets a
/// renderer upscale nothing but the parts of a frame that actually changed.
///
/// The `region` is clipped to the bounds of the `target`. Any pixel of the
/// `region` that is not covered by the upscaled `source` is left untouched,
/// which can happen when `source` is smaller than `target / scale`.
pub fn upscale<T: Copy>(
    source: &[T],
    source_size: Size<u32>,
    target: &mut [T],
    target_size: Size<u32>,
    scale: u32,
    region: Rectangle<u32>,
) {
    debug_assert!(
        source.len() >= (source_size.width * source_size.height) as usize
    );
    debug_assert!(
        target.len() >= (target_size.width * target_size.height) as usize
    );

    let scale = scale.max(1);

    // Clip the region to the target and to whatever the source can cover
    let x = region.x.min(target_size.width);
    let y = region.y.min(target_size.height);

    let end_x = region
        .x
        .saturating_add(region.width)
        .min(target_size.width)
        .min(source_size.width.saturating_mul(scale));

    let end_y = region
        .y
        .saturating_add(region.height)
        .min(target_size.height)
        .min(source_size.height.saturating_mul(scale));

    if x >= end_x || y >= end_y {
        return;
    }

    let target_width = target_size.width as usize;
    let source_width = source_size.width as usize;

    // Every `scale` consecutive rows of the target sample the same row of the
    // source, so only the first one of each band is expanded pixel by pixel;
    // the rest is copied verbatim.
    let mut band: Option<(u32, usize)> = None;

    for y in y..end_y {
        let target_row = y as usize * target_width;
        let source_row = (y / scale) as usize * source_width;

        match band {
            Some((source_y, first_row)) if source_y == y / scale => {
                target.copy_within(
                    first_row + x as usize..first_row + end_x as usize,
                    target_row + x as usize,
                );
            }
            _ => {
                let mut x = x;

                while x < end_x {
                    let source_x = (x / scale) as usize;

                    // The run ends where the next source pixel begins
                    let run_end = ((x / scale + 1) * scale).min(end_x);

                    target[target_row + x as usize
                        ..target_row + run_end as usize]
                        .fill(source[source_row + source_x]);

                    x = run_end;
                }

                band = Some((y / scale, target_row));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn checkerboard(width: u32, height: u32) -> Vec<u32> {
        (0..width * height)
            .map(|i| {
                let (x, y) = (i % width, i / width);
                if (x + y) % 2 == 0 { 0xAA } else { 0xBB }
            })
            .collect()
    }

    fn upscale_all(
        source: &[u32],
        source_size: Size<u32>,
        target_size: Size<u32>,
        scale: u32,
    ) -> Vec<u32> {
        let mut target =
            vec![0; (target_size.width * target_size.height) as usize];

        upscale(
            source,
            source_size,
            &mut target,
            target_size,
            scale,
            Rectangle {
                x: 0,
                y: 0,
                width: target_size.width,
                height: target_size.height,
            },
        );

        target
    }

    #[test]
    fn every_source_pixel_becomes_a_square_block() {
        let source_size = Size::new(3, 2);
        let source = checkerboard(3, 2);
        let target_size = Size::new(9, 6);

        let target = upscale_all(&source, source_size, target_size, 3);

        for y in 0..6u32 {
            for x in 0..9u32 {
                assert_eq!(
                    target[(y * 9 + x) as usize],
                    source[((y / 3) * 3 + x / 3) as usize],
                    "at {x}, {y}"
                );
            }
        }
    }

    #[test]
    fn a_target_that_does_not_divide_evenly_is_fully_covered() {
        // 8x5 surface at a pixel scale of 3 -> a 3x2 target that covers it
        let source_size = Size::new(3, 2);
        let source = checkerboard(3, 2);
        let target_size = Size::new(8, 5);

        let target = upscale_all(&source, source_size, target_size, 3);

        for y in 0..5u32 {
            for x in 0..8u32 {
                assert_eq!(
                    target[(y * 8 + x) as usize],
                    source[((y / 3) * 3 + x / 3) as usize],
                    "at {x}, {y}"
                );
            }
        }
    }

    #[test]
    fn only_the_region_is_written() {
        let source_size = Size::new(4, 4);
        let source = vec![0xFFu32; 16];
        let target_size = Size::new(8, 8);

        let mut target = vec![0u32; 64];

        upscale(
            &source,
            source_size,
            &mut target,
            target_size,
            2,
            Rectangle {
                x: 2,
                y: 2,
                width: 4,
                height: 4,
            },
        );

        for y in 0..8u32 {
            for x in 0..8u32 {
                let inside = (2..6).contains(&x) && (2..6).contains(&y);

                assert_eq!(
                    target[(y * 8 + x) as usize],
                    if inside { 0xFF } else { 0 },
                    "at {x}, {y}"
                );
            }
        }
    }

    #[test]
    fn a_region_out_of_bounds_is_clipped() {
        let source = vec![0xFFu32; 4];
        let mut target = vec![0u32; 16];

        upscale(
            &source,
            Size::new(2, 2),
            &mut target,
            Size::new(4, 4),
            2,
            Rectangle {
                x: 3,
                y: 3,
                width: 1_000,
                height: 1_000,
            },
        );

        assert_eq!(target[15], 0xFF);
        assert_eq!(target[14], 0);
    }

    #[test]
    fn a_scale_of_one_is_a_copy() {
        let source = checkerboard(4, 4);
        let target = upscale_all(&source, Size::new(4, 4), Size::new(4, 4), 1);

        assert_eq!(target, source);
    }
}
