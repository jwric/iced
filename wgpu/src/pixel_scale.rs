//! Present a low resolution frame, pixel art-style.
//!
//! When a [`Viewport`] has a pixel scale greater than 1, the scene is rendered
//! to an intermediate texture of [`Viewport::target_size`] and then upscaled
//! with nearest-neighbor filtering; so that every virtual pixel ends up
//! covering the exact same square of physical pixels.
//!
//! The same pass optionally applies [`CrtEffectSettings`], emulating the look
//! of a CRT monitor.
//!
//! [`Viewport`]: crate::graphics::Viewport
//! [`Viewport::target_size`]: crate::graphics::Viewport::target_size
use crate::core::Size;
use crate::graphics::Viewport;

use std::mem;

pub use crate::core::CrtEffectSettings;

/// The post-processing pass that upscales a low resolution frame.
#[derive(Debug)]
pub struct PixelScaler {
    crt_effects: Option<CrtEffectSettings>,
    target: Option<Target>,
    pipeline: Option<Pipeline>,
}

impl PixelScaler {
    /// Creates a new [`PixelScaler`] with the given CRT effects.
    pub fn new(crt_effects: Option<CrtEffectSettings>) -> Self {
        Self {
            crt_effects,
            target: None,
            pipeline: None,
        }
    }

    /// Returns whether a frame drawn to the given [`Viewport`] needs to go
    /// through the [`PixelScaler`].
    ///
    /// A frame that is neither upscaled nor CRT-filtered can be rendered
    /// straight to the surface.
    pub fn is_enabled(&self, viewport: &Viewport) -> bool {
        viewport.pixel_scale() > 1 || self.crt_effects.is_some()
    }

    /// Returns the texture view a frame must be rendered to.
    ///
    /// The texture is created—or recreated—as needed.
    pub fn target(
        &mut self,
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        size: Size<u32>,
    ) -> &wgpu::TextureView {
        let size = Size::new(size.width.max(1), size.height.max(1));

        if self
            .target
            .as_ref()
            .is_none_or(|target| target.size != size || target.format != format)
        {
            self.target = Some(Target::new(device, format, size));
        }

        &self
            .target
            .as_ref()
            .expect("Pixel scale target just created")
            .view
    }

    /// Upscales the last [`Self::target`] onto `view`, filling a surface of
    /// `size` physical pixels.
    ///
    /// The upscale is an exact integer ratio: a source pixel always covers
    /// `pixel_scale` physical pixels. Since the target is rounded up to cover
    /// the whole surface, up to one source pixel may fall outside of it on the
    /// right and bottom edges.
    pub fn present(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        view: &wgpu::TextureView,
        size: Size<u32>,
        pixel_scale: u32,
    ) {
        let Some(target) = &self.target else {
            return;
        };

        if self
            .pipeline
            .as_ref()
            .is_none_or(|pipeline| pipeline.format != format)
        {
            self.pipeline = Some(Pipeline::new(device, format));
        }

        let pipeline =
            self.pipeline.as_ref().expect("Pixel scale pipeline exists");

        let coverage = [
            (target.size.width * pixel_scale) as f32 / size.width.max(1) as f32,
            (target.size.height * pixel_scale) as f32
                / size.height.max(1) as f32,
        ];

        queue.write_buffer(
            &pipeline.uniforms,
            0,
            bytemuck::bytes_of(&Uniforms::new(coverage, self.crt_effects)),
        );

        let mut encoder =
            device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("iced_wgpu::pixel_scale encoder"),
            });

        {
            let mut pass =
                encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("iced_wgpu::pixel_scale pass"),
                    color_attachments: &[Some(
                        wgpu::RenderPassColorAttachment {
                            view,
                            resolve_target: None,
                            depth_slice: None,
                            ops: wgpu::Operations {
                                // Screen curvature can leave parts of the
                                // surface uncovered
                                load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                                store: wgpu::StoreOp::Store,
                            },
                        },
                    )],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });

            pass.set_pipeline(&pipeline.raw);
            pass.set_bind_group(0, target.bind_group(device, pipeline), &[]);
            pass.set_bind_group(1, &pipeline.uniform_bind_group, &[]);
            pass.draw(0..4, 0..1);
        }

        let _submission = queue.submit([encoder.finish()]);
    }
}

#[derive(Debug)]
struct Target {
    size: Size<u32>,
    format: wgpu::TextureFormat,
    view: wgpu::TextureView,
    bind_group: std::sync::OnceLock<wgpu::BindGroup>,
}

impl Target {
    fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        size: Size<u32>,
    ) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("iced_wgpu::pixel_scale texture"),
            size: wgpu::Extent3d {
                width: size.width,
                height: size.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        Self {
            size,
            format,
            view: texture.create_view(&wgpu::TextureViewDescriptor::default()),
            bind_group: std::sync::OnceLock::new(),
        }
    }

    /// The bind group only ever refers to the texture of this [`Target`], so
    /// it is created once and then reused for every frame.
    fn bind_group(
        &self,
        device: &wgpu::Device,
        pipeline: &Pipeline,
    ) -> &wgpu::BindGroup {
        self.bind_group.get_or_init(|| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("iced_wgpu::pixel_scale texture bind group"),
                layout: &pipeline.texture_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(
                            &self.view,
                        ),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(
                            &pipeline.sampler,
                        ),
                    },
                ],
            })
        })
    }
}

#[derive(Debug)]
struct Pipeline {
    raw: wgpu::RenderPipeline,
    format: wgpu::TextureFormat,
    texture_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    uniforms: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,
}

impl Pipeline {
    fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let texture_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("iced_wgpu::pixel_scale texture layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float {
                                filterable: true,
                            },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(
                            wgpu::SamplerBindingType::Filtering,
                        ),
                        count: None,
                    },
                ],
            });

        let uniform_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("iced_wgpu::pixel_scale uniform layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(
                            mem::size_of::<Uniforms>() as u64,
                        ),
                    },
                    count: None,
                }],
            });

        let layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("iced_wgpu::pixel_scale pipeline layout"),
                bind_group_layouts: &[&texture_layout, &uniform_layout],
                push_constant_ranges: &[],
            });

        let shader =
            device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("iced_wgpu::pixel_scale shader"),
                source: wgpu::ShaderSource::Wgsl(
                    include_str!("shader/pixel_scale.wgsl").into(),
                ),
            });

        let raw =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("iced_wgpu::pixel_scale pipeline"),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    compilation_options:
                        wgpu::PipelineCompilationOptions::default(),
                    buffers: &[],
                },
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleStrip,
                    ..Default::default()
                },
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_main"),
                    compilation_options:
                        wgpu::PipelineCompilationOptions::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview: None,
                cache: None,
            });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("iced_wgpu::pixel_scale sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let uniforms = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("iced_wgpu::pixel_scale uniforms"),
            size: mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let uniform_bind_group =
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("iced_wgpu::pixel_scale uniform bind group"),
                layout: &uniform_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniforms.as_entire_binding(),
                }],
            });

        Self {
            raw,
            format,
            texture_layout,
            sampler,
            uniforms,
            uniform_bind_group,
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniforms {
    /// The fraction of the surface covered by the upscaled target, which is
    /// slightly greater than 1 when the surface is not a multiple of the pixel
    /// scale.
    coverage: [f32; 2],
    scanline_intensity: f32,
    screen_curvature: f32,
    rgb_separation: f32,
    vignette_strength: f32,
    brightness: f32,
    contrast: f32,
}

impl Uniforms {
    fn new(coverage: [f32; 2], crt_effects: Option<CrtEffectSettings>) -> Self {
        // Neutral values, so that the pass is a plain upscale
        let crt_effects = crt_effects.unwrap_or(CrtEffectSettings {
            scanline_intensity: 0.0,
            screen_curvature: 0.0,
            rgb_separation: 0.0,
            vignette_strength: 0.0,
            brightness: 1.0,
            contrast: 1.0,
        });

        Self {
            coverage,
            scanline_intensity: crt_effects.scanline_intensity,
            screen_curvature: crt_effects.screen_curvature,
            rgb_separation: crt_effects.rgb_separation,
            vignette_strength: crt_effects.vignette_strength,
            brightness: crt_effects.brightness,
            contrast: crt_effects.contrast,
        }
    }
}

// A uniform binding must be a multiple of 16 bytes on downlevel and Web
// backends.
const _: () = assert!(mem::size_of::<Uniforms>().is_multiple_of(16));
