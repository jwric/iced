use crate::core::{Size, Transformation};

/// A viewing region for displaying computer graphics.
#[derive(Debug, Clone)]
pub struct Viewport {
    physical_size: Size<u32>,
    target_size: Size<u32>,
    logical_size: Size<f32>,
    scale_factor: f32,
    pixel_scale: u32,
    projection: Transformation,
}

impl Viewport {
    /// Creates a new [`Viewport`] with the given physical dimensions and scale
    /// factor.
    pub fn with_physical_size(size: Size<u32>, scale_factor: f32) -> Viewport {
        Viewport {
            physical_size: size,
            target_size: size,
            logical_size: Size::new(
                size.width as f32 / scale_factor,
                size.height as f32 / scale_factor,
            ),
            scale_factor,
            pixel_scale: 1,
            projection: Transformation::orthographic(size.width, size.height),
        }
    }

    /// Creates a new [`Viewport`] with the given physical dimensions and an
    /// integer __pixel scale__.
    ///
    /// A pixel scale is a scale factor that is applied by upscaling the
    /// rendered image with nearest-neighbor filtering, instead of during
    /// rasterization. The interface is laid out in __virtual pixels__, each of
    /// them covering exactly `pixel_scale` physical pixels—producing the
    /// crisp, chunky look of pixel art.
    ///
    /// Since a pixel scale _is_ the scale factor of the resulting [`Viewport`],
    /// it replaces the scale factor of the display instead of compounding with
    /// it. Derive it with [`PixelScaleMode::Auto`] if you want to keep the
    /// apparent size of an interface consistent across displays.
    ///
    /// [`PixelScaleMode::Auto`]: crate::core::PixelScaleMode::Auto
    pub fn with_pixel_scale(size: Size<u32>, pixel_scale: u32) -> Viewport {
        let pixel_scale = pixel_scale.max(1);

        if pixel_scale == 1 {
            return Self::with_physical_size(size, 1.0);
        }

        // The target is rounded __up__ so that it always covers the whole
        // surface. Otherwise, a surface whose size is not a multiple of the
        // pixel scale would end up with an unpainted edge of up to
        // `pixel_scale - 1` pixels.
        //
        // The flip side is that up to one virtual pixel may fall outside of
        // the surface on the right and bottom edges.
        let target_size = Size::new(
            size.width.div_ceil(pixel_scale).max(1),
            size.height.div_ceil(pixel_scale).max(1),
        );

        Viewport {
            physical_size: size,
            target_size,
            logical_size: Size::new(
                target_size.width as f32,
                target_size.height as f32,
            ),
            scale_factor: pixel_scale as f32,
            pixel_scale,
            projection: Transformation::orthographic(size.width, size.height),
        }
    }

    /// Returns the physical size of the [`Viewport`].
    ///
    /// This is the size of the surface that will be presented; which may be
    /// bigger than [`Self::target_size`] when pixel scaling is enabled.
    pub fn physical_size(&self) -> Size<u32> {
        self.physical_size
    }

    /// Returns the physical width of the [`Viewport`].
    pub fn physical_width(&self) -> u32 {
        self.physical_size.width
    }

    /// Returns the physical height of the [`Viewport`].
    pub fn physical_height(&self) -> u32 {
        self.physical_size.height
    }

    /// Returns the logical size of the [`Viewport`].
    pub fn logical_size(&self) -> Size<f32> {
        self.logical_size
    }

    /// Returns the scale factor of the [`Viewport`].
    pub fn scale_factor(&self) -> f32 {
        self.scale_factor
    }

    /// Returns the pixel scale of the [`Viewport`].
    ///
    /// This is the amount of physical pixels covered by a single rendered
    /// pixel. It is `1` unless the [`Viewport`] was created with
    /// [`Self::with_pixel_scale`].
    pub fn pixel_scale(&self) -> u32 {
        self.pixel_scale
    }

    /// Returns the size of the framebuffer a renderer must draw to.
    ///
    /// It is equal to [`Self::physical_size`] divided by
    /// [`Self::pixel_scale`], rounded up.
    pub fn target_size(&self) -> Size<u32> {
        self.target_size
    }

    /// Returns the [`Viewport`] a renderer must draw to.
    ///
    /// When pixel scaling is enabled, this is a [`Viewport`] of
    /// [`Self::target_size`] with a scale factor of `1.0`; which a compositor
    /// is then responsible for upscaling to [`Self::physical_size`] with
    /// nearest-neighbor filtering.
    ///
    /// It is a clone of the [`Viewport`] itself when pixel scaling is
    /// disabled. It is always idempotent.
    pub fn target(&self) -> Viewport {
        if self.pixel_scale == 1 {
            return self.clone();
        }

        Viewport {
            physical_size: self.target_size,
            target_size: self.target_size,
            logical_size: self.logical_size,
            scale_factor: 1.0,
            // Kept around so that renderers can tell they are drawing pixel
            // art and snap their primitives to the pixel grid accordingly.
            pixel_scale: self.pixel_scale,
            projection: Transformation::orthographic(
                self.target_size.width,
                self.target_size.height,
            ),
        }
    }

    /// Returns the projection transformation of the [`Viewport`].
    pub fn projection(&self) -> Transformation {
        self.projection
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_viewport_without_pixel_scaling_is_its_own_target() {
        let viewport = Viewport::with_physical_size(Size::new(800, 600), 2.0);

        assert_eq!(viewport.pixel_scale(), 1);
        assert_eq!(viewport.physical_size(), Size::new(800, 600));
        assert_eq!(viewport.target_size(), Size::new(800, 600));
        assert_eq!(viewport.logical_size(), Size::new(400.0, 300.0));

        let target = viewport.target();

        assert_eq!(target.physical_size(), viewport.physical_size());
        assert_eq!(target.scale_factor(), viewport.scale_factor());
        assert_eq!(target.logical_size(), viewport.logical_size());
    }

    #[test]
    fn a_pixel_scale_is_the_scale_factor() {
        let viewport = Viewport::with_pixel_scale(Size::new(800, 600), 4);

        assert_eq!(viewport.pixel_scale(), 4);
        assert_eq!(viewport.scale_factor(), 4.0);
        assert_eq!(viewport.physical_size(), Size::new(800, 600));
        assert_eq!(viewport.target_size(), Size::new(200, 150));
        assert_eq!(viewport.logical_size(), Size::new(200.0, 150.0));
    }

    #[test]
    fn a_target_covers_a_surface_that_does_not_divide_evenly() {
        let viewport = Viewport::with_pixel_scale(Size::new(1001, 599), 3);

        let target = viewport.target_size();

        assert_eq!(target, Size::new(334, 200));
        assert!(target.width * 3 >= 1001);
        assert!(target.height * 3 >= 599);

        // ...and never overshoots by a whole virtual pixel
        assert!((target.width - 1) * 3 < 1001);
        assert!((target.height - 1) * 3 < 599);
    }

    #[test]
    fn a_target_viewport_renders_at_one_to_one() {
        let viewport = Viewport::with_pixel_scale(Size::new(1001, 599), 3);
        let target = viewport.target();

        assert_eq!(target.scale_factor(), 1.0);
        assert_eq!(target.physical_size(), viewport.target_size());
        assert_eq!(target.logical_size(), Size::new(334.0, 200.0));

        // The renderer still knows it is drawing pixel art
        assert_eq!(target.pixel_scale(), 3);

        // Taking the target of a target changes nothing
        let target = target.target();

        assert_eq!(target.physical_size(), viewport.target_size());
        assert_eq!(target.target_size(), viewport.target_size());
        assert_eq!(target.scale_factor(), 1.0);
    }

    #[test]
    fn a_cursor_lands_in_the_layout_it_is_rendered_over() {
        // This is the whole point of folding the pixel scale into the scale
        // factor: a shell converts a physical cursor position with
        // `scale_factor`, and the result has to land inside `logical_size`—
        // the very size widgets were laid out against.
        let viewport = Viewport::with_pixel_scale(Size::new(1001, 599), 3);

        let logical = viewport.logical_size();
        let scale = viewport.scale_factor();

        for (x, y) in [(0.0, 0.0), (500.0, 300.0), (1000.0, 598.0)] {
            let (x, y) = (x / scale, y / scale);

            assert!(x <= logical.width, "{x} is outside {}", logical.width);
            assert!(y <= logical.height, "{y} is outside {}", logical.height);
        }

        // ...and the far corner of the surface still reaches the far corner
        // of the layout, give or take the virtual pixel it sits in
        assert!(1000.0 / scale > logical.width - 1.0);
        assert!(598.0 / scale > logical.height - 1.0);
    }

    #[test]
    fn a_pixel_scale_of_one_is_no_pixel_scaling() {
        let viewport = Viewport::with_pixel_scale(Size::new(800, 600), 1);

        assert_eq!(viewport.pixel_scale(), 1);
        assert_eq!(viewport.scale_factor(), 1.0);
        assert_eq!(viewport.logical_size(), Size::new(800.0, 600.0));
    }

    #[test]
    fn a_tiny_surface_still_has_a_target() {
        let viewport = Viewport::with_pixel_scale(Size::new(2, 1), 8);

        assert_eq!(viewport.target_size(), Size::new(1, 1));
    }
}
