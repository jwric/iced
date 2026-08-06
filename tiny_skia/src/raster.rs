use crate::core::image as raster;
use crate::core::{Rectangle, Size};
use crate::graphics;

use rustc_hash::{FxHashMap, FxHashSet};
use std::cell::RefCell;
use std::collections::hash_map;

#[derive(Debug)]
pub struct Pipeline {
    cache: RefCell<Cache>,
}

impl Pipeline {
    pub fn new() -> Self {
        Self {
            cache: RefCell::new(Cache::default()),
        }
    }

    pub fn load(
        &self,
        handle: &raster::Handle,
    ) -> Result<raster::Allocation, raster::Error> {
        let mut cache = self.cache.borrow_mut();
        let image = cache.allocate(handle)?;

        #[allow(unsafe_code)]
        Ok(unsafe {
            raster::allocate(handle, Size::new(image.width(), image.height()))
        })
    }

    pub fn dimensions(&self, handle: &raster::Handle) -> Option<Size<u32>> {
        let mut cache = self.cache.borrow_mut();
        let image = cache.allocate(handle).ok()?;

        Some(Size::new(image.width(), image.height()))
    }

    pub fn draw(
        &mut self,
        handle: &raster::Handle,
        filter_method: raster::FilterMethod,
        bounds: Rectangle,
        opacity: f32,
        pixels: &mut tiny_skia::PixmapMut<'_>,
        transform: tiny_skia::Transform,
        clip_mask: Option<&tiny_skia::Mask>,
    ) {
        let mut cache = self.cache.borrow_mut();

        let Ok(image) = cache.allocate(handle) else {
            return;
        };

        let width_scale = bounds.width / image.width() as f32;
        let height_scale = bounds.height / image.height() as f32;

        // `draw_pixmap` only takes an __integer__ offset, and applies it
        // _before_ the transform. Passing the position there would therefore
        // quantize it to multiples of the scale of the image—shifting, say, a
        // sprite drawn at 4x by up to 4 pixels, and by a different amount
        // depending on where it happens to land.
        //
        // Carrying the position in the transform instead keeps it exact.
        let transform = transform
            .pre_translate(bounds.x, bounds.y)
            .pre_scale(width_scale, height_scale);

        let quality = match filter_method {
            raster::FilterMethod::Linear => tiny_skia::FilterQuality::Bilinear,
            raster::FilterMethod::Nearest => tiny_skia::FilterQuality::Nearest,
        };

        pixels.draw_pixmap(
            0,
            0,
            image,
            &tiny_skia::PixmapPaint {
                quality,
                opacity,
                ..Default::default()
            },
            transform,
            clip_mask,
        );
    }

    pub fn trim_cache(&mut self) {
        self.cache.borrow_mut().trim();
    }
}

#[derive(Debug, Default)]
struct Cache {
    entries: FxHashMap<raster::Id, Option<Entry>>,
    hits: FxHashSet<raster::Id>,
}

impl Cache {
    pub fn allocate(
        &mut self,
        handle: &raster::Handle,
    ) -> Result<tiny_skia::PixmapRef<'_>, raster::Error> {
        let id = handle.id();

        if let hash_map::Entry::Vacant(entry) = self.entries.entry(id) {
            let image = match graphics::image::load(handle) {
                Ok(image) => image,
                Err(error) => {
                    let _ = entry.insert(None);

                    return Err(error);
                }
            };

            if image.width() == 0 || image.height() == 0 {
                return Err(raster::Error::Empty);
            }

            let mut buffer =
                vec![0u32; image.width() as usize * image.height() as usize];

            for (i, pixel) in image.pixels().enumerate() {
                let [r, g, b, a] = pixel.0;

                buffer[i] = bytemuck::cast(
                    tiny_skia::ColorU8::from_rgba(b, g, r, a).premultiply(),
                );
            }

            let _ = entry.insert(Some(Entry {
                width: image.width(),
                height: image.height(),
                pixels: buffer,
            }));
        }

        let _ = self.hits.insert(id);

        Ok(self
            .entries
            .get(&id)
            .unwrap()
            .as_ref()
            .map(|entry| {
                tiny_skia::PixmapRef::from_bytes(
                    bytemuck::cast_slice(&entry.pixels),
                    entry.width,
                    entry.height,
                )
                .expect("Build pixmap from image bytes")
            })
            .expect("Image should be allocated"))
    }

    fn trim(&mut self) {
        self.entries.retain(|key, _| self.hits.contains(key));
        self.hits.clear();
    }
}

#[derive(Debug)]
struct Entry {
    width: u32,
    height: u32,
    pixels: Vec<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::core::Point;

    /// Draws a 2x2 image into a 16x16 pixmap and returns, for every pixel, the
    /// index of the source texel that ended up there—or `None` where nothing
    /// was drawn.
    fn draw(bounds: Rectangle) -> Vec<Option<usize>> {
        const COLORS: [[u8; 4]; 4] = [
            [255, 0, 0, 255],
            [0, 255, 0, 255],
            [0, 0, 255, 255],
            [255, 255, 0, 255],
        ];

        let handle = raster::Handle::from_rgba(
            2,
            2,
            COLORS.into_iter().flatten().collect::<Vec<u8>>(),
        );

        let mut pipeline = Pipeline::new();
        let mut pixmap =
            tiny_skia::Pixmap::new(16, 16).expect("Create test pixmap");

        pipeline.draw(
            &handle,
            raster::FilterMethod::Nearest,
            bounds,
            1.0,
            &mut pixmap.as_mut(),
            tiny_skia::Transform::identity(),
            None,
        );

        // The cache stores pixels as BGRA premultiplied, so match on that
        let expected: Vec<u32> = COLORS
            .into_iter()
            .map(|[r, g, b, a]| {
                bytemuck::cast(
                    tiny_skia::ColorU8::from_rgba(b, g, r, a).premultiply(),
                )
            })
            .collect();

        bytemuck::cast_slice::<u8, u32>(pixmap.data())
            .iter()
            .map(|pixel| expected.iter().position(|color| color == pixel))
            .collect()
    }

    #[test]
    fn an_image_lands_exactly_on_its_bounds() {
        // A 2x2 image blown up 4x, at a position that is not a multiple of
        // that scale. `draw_pixmap` only takes an integer offset applied
        // before the scale, so naively passing `bounds.x / width_scale` used
        // to snap this to (4, 0) instead of (5, 3).
        let drawn = draw(Rectangle {
            x: 5.0,
            y: 3.0,
            width: 8.0,
            height: 8.0,
        });

        for y in 0..16 {
            for x in 0..16 {
                let inside = (5..13).contains(&x) && (3..11).contains(&y);

                let texel = inside
                    .then(|| usize::from(y >= 7) * 2 + usize::from(x >= 9));

                assert_eq!(
                    drawn[y * 16 + x],
                    texel,
                    "at {x}, {y}: expected {texel:?}"
                );
            }
        }
    }

    #[test]
    fn an_image_at_the_origin_is_unaffected() {
        // The old arithmetic happened to be correct here; make sure the fix
        // did not move the easy case.
        let drawn = draw(Rectangle {
            x: 0.0,
            y: 0.0,
            width: 8.0,
            height: 8.0,
        });

        assert_eq!(drawn[0], Some(0));
        assert_eq!(drawn[7], Some(1));
        assert_eq!(drawn[4 * 16], Some(2));
        assert_eq!(drawn[4 * 16 + 7], Some(3));

        // ...and nothing spills past the far edge
        assert_eq!(drawn[8], None);
        assert_eq!(drawn[8 * 16], None);
    }

    #[test]
    fn an_image_drawn_at_its_native_size_is_pixel_aligned() {
        let drawn =
            draw(Rectangle::new(Point::new(3.0, 9.0), Size::new(2.0, 2.0)));

        assert_eq!(drawn[9 * 16 + 3], Some(0));
        assert_eq!(drawn[9 * 16 + 4], Some(1));
        assert_eq!(drawn[10 * 16 + 3], Some(2));
        assert_eq!(drawn[10 * 16 + 4], Some(3));
    }
}
