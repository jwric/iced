use crate::core::{Color, Rectangle, Size};
use crate::graphics::compositor::{self, Information};
use crate::graphics::damage;
use crate::graphics::error::{self, Error};
use crate::graphics::{self, Shell, Viewport};
use crate::{Layer, Renderer, Settings};

use std::collections::VecDeque;
use std::num::NonZeroU32;

pub struct Compositor {
    context: softbuffer::Context<Box<dyn compositor::Display>>,
    settings: Settings,
}

pub struct Surface {
    window: softbuffer::Surface<
        Box<dyn compositor::Display>,
        Box<dyn compositor::Window>,
    >,
    clip_mask: tiny_skia::Mask,
    layer_stack: VecDeque<Vec<Layer>>,
    background_color: Color,
    max_age: u8,
    /// The low resolution framebuffer drawn to when pixel scaling is enabled,
    /// which is then upscaled onto the window.
    framebuffer: Option<tiny_skia::Pixmap>,
}

impl crate::graphics::Compositor for Compositor {
    type Renderer = Renderer;
    type Surface = Surface;

    async fn with_backend(
        settings: graphics::Settings,
        display: impl compositor::Display,
        _compatible_window: impl compositor::Window,
        _shell: Shell,
        backend: Option<&str>,
    ) -> Result<Self, Error> {
        match backend {
            None | Some("tiny-skia") | Some("tiny_skia") => {
                Ok(new(settings.into(), display))
            }
            Some(backend) => Err(Error::GraphicsAdapterNotFound {
                backend: "tiny-skia",
                reason: error::Reason::DidNotMatch {
                    preferred_backend: backend.to_owned(),
                },
            }),
        }
    }

    fn create_renderer(&self) -> Self::Renderer {
        Renderer::new(
            self.settings.default_font,
            self.settings.default_text_size,
        )
    }

    fn create_surface<W: compositor::Window + Clone>(
        &mut self,
        window: W,
        width: u32,
        height: u32,
    ) -> Self::Surface {
        let window = softbuffer::Surface::new(
            &self.context,
            Box::new(window.clone()) as _,
        )
        .expect("Create softbuffer surface for window");

        let mut surface = Surface {
            window,
            clip_mask: tiny_skia::Mask::new(1, 1).expect("Create clip mask"),
            layer_stack: VecDeque::new(),
            background_color: Color::BLACK,
            max_age: 0,
            framebuffer: None,
        };

        if width > 0 && height > 0 {
            self.configure_surface(&mut surface, width, height);
        }

        surface
    }

    fn configure_surface(
        &mut self,
        surface: &mut Self::Surface,
        width: u32,
        height: u32,
    ) {
        surface
            .window
            .resize(
                NonZeroU32::new(width).expect("Non-zero width"),
                NonZeroU32::new(height).expect("Non-zero height"),
            )
            .expect("Resize surface");

        // The clip mask and the framebuffer are sized against the render
        // target, which `present` is the first to know about.
        surface.layer_stack.clear();
    }

    fn information(&self) -> Information {
        Information {
            adapter: String::from("CPU"),
            backend: String::from("tiny-skia"),
        }
    }

    fn present(
        &mut self,
        renderer: &mut Self::Renderer,
        surface: &mut Self::Surface,
        viewport: &Viewport,
        background_color: Color,
        on_pre_present: impl FnOnce(),
    ) -> Result<(), compositor::SurfaceError> {
        present(
            renderer,
            surface,
            viewport,
            background_color,
            on_pre_present,
        )
    }

    fn screenshot(
        &mut self,
        renderer: &mut Self::Renderer,
        viewport: &Viewport,
        background_color: Color,
    ) -> Vec<u8> {
        screenshot(renderer, viewport, background_color)
    }
}

pub fn new(
    settings: Settings,
    display: impl compositor::Display,
) -> Compositor {
    #[allow(unsafe_code)]
    let context = softbuffer::Context::new(Box::new(display) as _)
        .expect("Create softbuffer context");

    Compositor { context, settings }
}

pub fn present(
    renderer: &mut Renderer,
    surface: &mut Surface,
    viewport: &Viewport,
    background_color: Color,
    on_pre_present: impl FnOnce(),
) -> Result<(), compositor::SurfaceError> {
    let physical_size = viewport.physical_size();
    let pixel_scale = viewport.pixel_scale();

    // When pixel scaling is enabled, the scene is drawn to a low resolution
    // framebuffer that is then upscaled onto the window.
    let target = viewport.target();
    let target_size = target.physical_size();

    // A freshly allocated framebuffer has nothing to reuse, no matter how
    // young the contents of the window are.
    let is_stale = prepare(surface, target_size, pixel_scale);

    let mut buffer = surface
        .window
        .buffer_mut()
        .map_err(|_| compositor::SurfaceError::Lost)?;

    let last_layers = {
        let age = buffer.age();

        surface.max_age = surface.max_age.max(age);
        surface.layer_stack.truncate(surface.max_age as usize);

        if age > 0 && !is_stale {
            surface.layer_stack.get(age as usize - 1)
        } else {
            None
        }
    };

    let damage = last_layers
        .and_then(|last_layers| {
            (surface.background_color == background_color).then(|| {
                damage::diff(
                    last_layers,
                    renderer.layers(),
                    |layer| vec![layer.bounds],
                    Layer::damage,
                )
            })
        })
        .unwrap_or_else(|| vec![Rectangle::with_size(target.logical_size())]);

    if damage.is_empty() {
        if let Some(last_layers) = last_layers {
            surface.layer_stack.push_front(last_layers.clone());
        }
    } else {
        surface.layer_stack.push_front(renderer.layers().to_vec());
        surface.background_color = background_color;

        let damage =
            damage::group(damage, Rectangle::with_size(target.logical_size()));

        match &mut surface.framebuffer {
            Some(framebuffer) => {
                renderer.draw(
                    &mut framebuffer.as_mut(),
                    &mut surface.clip_mask,
                    &target,
                    &damage,
                    background_color,
                );

                let source: &[u32] = bytemuck::cast_slice(framebuffer.data());

                for region in &damage {
                    graphics::pixel_scale::upscale(
                        source,
                        target_size,
                        &mut buffer,
                        physical_size,
                        pixel_scale,
                        upscaled(*region, pixel_scale),
                    );
                }
            }
            None => {
                let mut pixels = tiny_skia::PixmapMut::from_bytes(
                    bytemuck::cast_slice_mut(&mut buffer),
                    physical_size.width,
                    physical_size.height,
                )
                .expect("Create pixel map");

                renderer.draw(
                    &mut pixels,
                    &mut surface.clip_mask,
                    &target,
                    &damage,
                    background_color,
                );
            }
        }
    }

    on_pre_present();
    buffer.present().map_err(|_| compositor::SurfaceError::Lost)
}

/// Resizes the clip mask and the framebuffer of a [`Surface`] to fit a render
/// target of `size`.
///
/// Returns whether the contents of the window must be redrawn in full.
fn prepare(surface: &mut Surface, size: Size<u32>, pixel_scale: u32) -> bool {
    let mut is_stale = false;

    if surface.clip_mask.width() != size.width
        || surface.clip_mask.height() != size.height
    {
        surface.clip_mask = tiny_skia::Mask::new(size.width, size.height)
            .expect("Create clip mask");
    }

    if pixel_scale > 1 {
        if surface.framebuffer.as_ref().is_none_or(|framebuffer| {
            framebuffer.width() != size.width
                || framebuffer.height() != size.height
        }) {
            surface.framebuffer = Some(
                tiny_skia::Pixmap::new(size.width, size.height)
                    .expect("Create framebuffer"),
            );

            is_stale = true;
        }
    } else if surface.framebuffer.take().is_some() {
        is_stale = true;
    }

    is_stale
}

/// Converts a damaged `region` of a render target into the region of the
/// window it ends up covering once upscaled.
fn upscaled(region: Rectangle, pixel_scale: u32) -> Rectangle<u32> {
    let x = region.x.floor().max(0.0) as u32;
    let y = region.y.floor().max(0.0) as u32;

    Rectangle {
        x: x * pixel_scale,
        y: y * pixel_scale,
        width: ((region.x + region.width).ceil().max(0.0) as u32)
            .saturating_sub(x)
            * pixel_scale,
        height: ((region.y + region.height).ceil().max(0.0) as u32)
            .saturating_sub(y)
            * pixel_scale,
    }
}

pub fn screenshot(
    renderer: &mut Renderer,
    viewport: &Viewport,
    background_color: Color,
) -> Vec<u8> {
    let physical_size = viewport.physical_size();
    let pixel_scale = viewport.pixel_scale();

    let target = viewport.target();
    let target_size = target.physical_size();

    let mut framebuffer: Vec<u32> =
        vec![0; target_size.width as usize * target_size.height as usize];

    let mut clip_mask =
        tiny_skia::Mask::new(target_size.width, target_size.height)
            .expect("Create clip mask");

    renderer.draw(
        &mut tiny_skia::PixmapMut::from_bytes(
            bytemuck::cast_slice_mut(&mut framebuffer),
            target_size.width,
            target_size.height,
        )
        .expect("Create offscreen pixel map"),
        &mut clip_mask,
        &target,
        &[Rectangle::with_size(target.logical_size())],
        background_color,
    );

    let offscreen_buffer = if pixel_scale == 1 {
        framebuffer
    } else {
        let mut screenshot = vec![
            0;
            physical_size.width as usize
                * physical_size.height as usize
        ];

        graphics::pixel_scale::upscale(
            &framebuffer,
            target_size,
            &mut screenshot,
            physical_size,
            pixel_scale,
            Rectangle {
                x: 0,
                y: 0,
                width: physical_size.width,
                height: physical_size.height,
            },
        );

        screenshot
    };

    offscreen_buffer.iter().fold(
        Vec::with_capacity(offscreen_buffer.len() * 4),
        |mut acc, pixel| {
            const A_MASK: u32 = 0xFF_00_00_00;
            const R_MASK: u32 = 0x00_FF_00_00;
            const G_MASK: u32 = 0x00_00_FF_00;
            const B_MASK: u32 = 0x00_00_00_FF;

            let a = ((A_MASK & pixel) >> 24) as u8;
            let r = ((R_MASK & pixel) >> 16) as u8;
            let g = ((G_MASK & pixel) >> 8) as u8;
            let b = (B_MASK & pixel) as u8;

            acc.extend([r, g, b, a]);
            acc
        },
    )
}
