//! Pixel scaling support for retro-style pixel art effects.
//!
//! This module provides utilities for rendering to a downscaled intermediate
//! texture and then upscaling it with nearest-neighbor filtering for a
//! pixelated retro look.

use crate::graphics::Viewport;
use wgpu::util::DeviceExt;

// Re-export CrtEffectSettings from core for convenience
pub use crate::core::CrtEffectSettings;

/// Uniform buffer data for CRT effects shader.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct CrtUniforms {
    scanline_intensity: f32,
    screen_curvature: f32,
    rgb_separation: f32,
    vignette_strength: f32,
    brightness: f32,
    contrast: f32,
    _padding: [f32; 2], // Align to 16 bytes
}

impl From<crate::core::CrtEffectSettings> for CrtUniforms {
    fn from(settings: crate::core::CrtEffectSettings) -> Self {
        Self {
            scanline_intensity: settings.scanline_intensity,
            screen_curvature: settings.screen_curvature,
            rgb_separation: settings.rgb_separation,
            vignette_strength: settings.vignette_strength,
            brightness: settings.brightness,
            contrast: settings.contrast,
            _padding: [0.0; 2],
        }
    }
}

/// Manages pixel scaling state for a compositor.
pub struct PixelScaleState {
    /// The pixel scale factor (1 = no scaling).
    pixel_scale: u32,
    /// The intermediate texture for downscaled rendering.
    intermediate_texture: Option<IntermediateTexture>,
    /// The blit pipeline for upscaling.
    blit_pipeline: Option<BlitPipeline>,
    /// CRT effect settings. None means disabled.
    pub crt_settings: Option<crate::core::CrtEffectSettings>,
}

struct IntermediateTexture {
    #[allow(dead_code)]
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    width: u32,
    height: u32,
}

struct BlitPipeline {
    pipeline: wgpu::RenderPipeline,
    texture_bind_group_layout: wgpu::BindGroupLayout,
    uniform_bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    uniform_buffer: wgpu::Buffer,
}

impl PixelScaleState {
    /// Creates a new pixel scale state.
    pub fn new(
        pixel_scale: u32,
        crt_settings: Option<crate::core::CrtEffectSettings>,
    ) -> Self {
        Self {
            pixel_scale: pixel_scale.max(1),
            intermediate_texture: None,
            blit_pipeline: None,
            crt_settings,
        }
    }

    /// Returns whether pixel scaling is enabled.
    pub fn is_enabled(&self) -> bool {
        self.pixel_scale > 1
    }

    /// Gets the pixel scale factor.
    pub fn pixel_scale(&self) -> u32 {
        self.pixel_scale
    }

    /// Updates the CRT effect settings.
    pub fn update_crt_settings(
        &mut self,
        settings: Option<crate::core::CrtEffectSettings>,
    ) {
        self.crt_settings = settings;
    }

    /// Gets the current CRT effect settings.
    pub fn crt_settings(&self) -> Option<crate::core::CrtEffectSettings> {
        self.crt_settings
    }

    /// Presents with pixel scaling if enabled.
    ///
    /// Returns `Some(view, viewport)` if pixel scaling is enabled and rendering
    /// should go to the intermediate texture. Returns `None` if rendering should
    /// go directly to the surface.
    pub fn prepare_render_target(
        &mut self,
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
        scale_factor: f64,
    ) -> Option<(&wgpu::TextureView, Viewport)> {
        if !self.is_enabled() {
            return None;
        }

        // Calculate logical size from physical size and DPI scale factor
        let logical_width =
            (width as f64 / scale_factor).round().max(1.0) as u32;
        let logical_height =
            (height as f64 / scale_factor).round().max(1.0) as u32;

        // Snap logical size to be divisible by pixel_scale to prevent gaps
        // This ensures intermediate texture upscales perfectly to fill the logical size
        let _snapped_logical_width =
            (logical_width / self.pixel_scale) * self.pixel_scale;
        let _snapped_logical_height =
            (logical_height / self.pixel_scale) * self.pixel_scale;

        // Calculate intermediate texture size from snapped logical size
        let intermediate_width = (width / self.pixel_scale).max(1);
        let intermediate_height = (height / self.pixel_scale).max(1);

        self.ensure_intermediate_texture(
            device,
            format,
            intermediate_width,
            intermediate_height,
        );

        let intermediate = self.intermediate_texture.as_ref()?;

        // Create viewport with the snapped logical size divided by pixel_scale
        // Use scale_factor=1.0 for pixel-perfect 1:1 rendering with no sub-pixel positioning
        // This ensures text and UI elements are positioned at whole pixel coordinates
        let scaled_viewport = Viewport::with_physical_size(
            crate::core::Size::new(intermediate.width, intermediate.height),
            1.0,
        );

        Some((&intermediate.view, scaled_viewport))
    }

    /// Blits the intermediate texture to the final surface view.
    pub fn blit_to_surface(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        target_view: &wgpu::TextureView,
        target_width: u32,
        target_height: u32,
    ) {
        self.ensure_blit_pipeline(device, format);

        let Some(intermediate) = &self.intermediate_texture else {
            return;
        };

        let blit_pipeline = self
            .blit_pipeline
            .as_ref()
            .expect("Blit pipeline should exist");

        // Update CRT uniforms
        let uniforms: CrtUniforms = if let Some(settings) = self.crt_settings {
            settings.into()
        } else {
            // When disabled (None), set all effects to neutral/zero values
            crate::core::CrtEffectSettings {
                scanline_intensity: 0.0,
                screen_curvature: 0.0,
                rgb_separation: 0.0,
                vignette_strength: 0.0,
                brightness: 1.0,
                contrast: 1.0,
            }
            .into()
        };
        queue.write_buffer(
            &blit_pipeline.uniform_buffer,
            0,
            bytemuck::cast_slice(&[uniforms]),
        );

        let mut encoder =
            device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("iced_wgpu::pixel_scale blit encoder"),
            });

        {
            let mut render_pass =
                encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("iced_wgpu::pixel_scale blit pass"),
                    color_attachments: &[Some(
                        wgpu::RenderPassColorAttachment {
                            view: target_view,
                            resolve_target: None,
                            depth_slice: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                                store: wgpu::StoreOp::Store,
                            },
                        },
                    )],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });

            // let src_w = intermediate.width;
            // let src_h = intermediate.height;

            // // Integer scale so every source pixel becomes exactly scale×scale physical pixels
            // let scale_x = target_width / src_w;
            // let scale_y = target_height / src_h;
            // let scale = scale_x.min(scale_y).max(1);

            // // Scaled blit area in physical pixels (exact integers)
            // let blit_w = src_w * scale;
            // let blit_h = src_h * scale;

            // // Center (letterbox)
            // let offset_x = ((target_width - blit_w) / 2) as f32;
            // let offset_y = ((target_height - blit_h) / 2) as f32;

            // render_pass.set_viewport(
            //     offset_x,
            //     offset_y,
            //     blit_w as f32,
            //     blit_h as f32,
            //     0.0,
            //     1.0,
            // );
            //
            render_pass.set_viewport(
                0.0,
                0.0,
                target_width as f32,
                target_height as f32,
                0.0,
                1.0,
            );

            blit_pipeline.render(
                device,
                &intermediate.view,
                target_width,
                target_height,
                &mut render_pass,
            );
        }

        let _submission = queue.submit([encoder.finish()]);
    }

    fn ensure_intermediate_texture(
        &mut self,
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) {
        let needs_recreate = self
            .intermediate_texture
            .as_ref()
            .map(|tex| tex.width != width || tex.height != height)
            .unwrap_or(true);

        if needs_recreate {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("iced_wgpu::pixel_scale intermediate texture"),
                size: wgpu::Extent3d {
                    width,
                    height,
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

            let view =
                texture.create_view(&wgpu::TextureViewDescriptor::default());

            self.intermediate_texture = Some(IntermediateTexture {
                texture,
                view,
                width,
                height,
            });
        }
    }

    fn ensure_blit_pipeline(
        &mut self,
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
    ) {
        if self.blit_pipeline.is_some() {
            return;
        }

        self.blit_pipeline = Some(BlitPipeline::new(device, format));
    }
}

impl BlitPipeline {
    fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        // Bind group 0: Texture and sampler
        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("iced_wgpu::pixel_scale texture bind group layout"),
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

        // Bind group 1: CRT uniforms
        let uniform_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("iced_wgpu::pixel_scale uniform bind group layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("iced_wgpu::pixel_scale pipeline layout"),
                bind_group_layouts: &[
                    &texture_bind_group_layout,
                    &uniform_bind_group_layout,
                ],
                push_constant_ranges: &[],
            });

        let shader =
            device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("iced_wgpu::pixel_scale shader"),
                source: wgpu::ShaderSource::Wgsl(BLIT_SHADER.into()),
            });

        let pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("iced_wgpu::pixel_scale pipeline"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    compilation_options:
                        wgpu::PipelineCompilationOptions::default(),
                    buffers: &[wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<Vertex>() as u64,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &wgpu::vertex_attr_array![
                            0 => Float32x2,
                            1 => Float32x2,
                        ],
                    }],
                },
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    front_face: wgpu::FrontFace::Cw,
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

        let vertex_buffer =
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("iced_wgpu::pixel_scale vertex buffer"),
                contents: bytemuck::cast_slice(VERTICES),
                usage: wgpu::BufferUsages::VERTEX,
            });

        let index_buffer =
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("iced_wgpu::pixel_scale index buffer"),
                contents: bytemuck::cast_slice(INDICES),
                usage: wgpu::BufferUsages::INDEX,
            });

        let uniform_buffer =
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("iced_wgpu::pixel_scale uniform buffer"),
                contents: bytemuck::cast_slice(&[CrtUniforms {
                    scanline_intensity: 0.0,
                    screen_curvature: 0.0,
                    rgb_separation: 0.0,
                    vignette_strength: 0.0,
                    brightness: 1.0,
                    contrast: 1.0,
                    _padding: [0.0; 2],
                }]),
                usage: wgpu::BufferUsages::UNIFORM
                    | wgpu::BufferUsages::COPY_DST,
            });

        Self {
            pipeline,
            texture_bind_group_layout,
            uniform_bind_group_layout,
            sampler,
            vertex_buffer,
            index_buffer,
            uniform_buffer,
        }
    }

    fn render(
        &self,
        device: &wgpu::Device,
        texture_view: &wgpu::TextureView,
        _target_width: u32,
        _target_height: u32,
        render_pass: &mut wgpu::RenderPass<'_>,
    ) {
        let texture_bind_group =
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("iced_wgpu::pixel_scale texture bind group"),
                layout: &self.texture_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(
                            texture_view,
                        ),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&self.sampler),
                    },
                ],
            });

        let uniform_bind_group =
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("iced_wgpu::pixel_scale uniform bind group"),
                layout: &self.uniform_bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.uniform_buffer.as_entire_binding(),
                }],
            });

        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &texture_bind_group, &[]);
        render_pass.set_bind_group(1, &uniform_bind_group, &[]);
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.set_index_buffer(
            self.index_buffer.slice(..),
            wgpu::IndexFormat::Uint16,
        );
        render_pass.draw_indexed(0..6, 0, 0..1);
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    position: [f32; 2],
    tex_coords: [f32; 2],
}

const VERTICES: &[Vertex] = &[
    Vertex {
        position: [-1.0, -1.0],
        tex_coords: [0.0, 1.0],
    },
    Vertex {
        position: [1.0, -1.0],
        tex_coords: [1.0, 1.0],
    },
    Vertex {
        position: [1.0, 1.0],
        tex_coords: [1.0, 0.0],
    },
    Vertex {
        position: [-1.0, 1.0],
        tex_coords: [0.0, 0.0],
    },
];

const INDICES: &[u16] = &[0, 1, 2, 0, 2, 3];

const BLIT_SHADER: &str = r#"
struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) tex_coords: vec2<f32>,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
}

struct CrtUniforms {
    scanline_intensity: f32,
    screen_curvature: f32,
    rgb_separation: f32,
    vignette_strength: f32,
    brightness: f32,
    contrast: f32,
}

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    output.position = vec4<f32>(input.position, 0.0, 1.0);
    output.tex_coords = input.tex_coords;
    return output;
}

@group(0) @binding(0)
var t_texture: texture_2d<f32>;
@group(0) @binding(1)
var t_sampler: sampler;

@group(1) @binding(0)
var<uniform> crt: CrtUniforms;

// Apply barrel distortion for CRT screen curvature
fn apply_curvature(uv: vec2<f32>, amount: f32) -> vec2<f32> {
    if (amount == 0.0) {
        return uv;
    }

    let centered = uv * 2.0 - 1.0;
    let dist = length(centered);
    let distortion = 1.0 + amount * dist * dist;
    let curved = centered * distortion;
    return curved * 0.5 + 0.5;
}

// Vignette effect (darkening at screen edges)
fn apply_vignette(uv: vec2<f32>, strength: f32) -> f32 {
    if (strength == 0.0) {
        return 1.0;
    }

    let centered = uv * 2.0 - 1.0;
    let dist = length(centered);
    return 1.0 - smoothstep(0.5, 1.5, dist) * strength;
}

// Scanline effect
fn apply_scanlines(uv: vec2<f32>, intensity: f32) -> f32 {
    if (intensity == 0.0) {
        return 1.0;
    }

    let dims = vec2<f32>(textureDimensions(t_texture));
    let scaled_y = uv.y * dims.y;
    let scanline = sin(scaled_y * 3.14159265359);
    return 1.0 - (scanline * 0.5 + 0.5) * intensity;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    var uv = input.tex_coords;

    // Apply screen curvature
    uv = apply_curvature(uv, crt.screen_curvature);

    // Check if we're outside the screen after curvature
    if (uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0) {
        return vec4<f32>(0.0, 0.0, 0.0, 1.0);
    }

    // Sample with RGB separation (chromatic aberration)
    // rgb_separation is specified in pixels at the source texture resolution
    // Convert to UV space for resolution-independent effect
    var color: vec3<f32>;
    if (crt.rgb_separation > 0.0) {
        let dims = vec2<f32>(textureDimensions(t_texture));
        let uv_offset = crt.rgb_separation / dims.x;
        let r = textureSample(t_texture, t_sampler, uv + vec2<f32>(-uv_offset, 0.0)).r;
        let g = textureSample(t_texture, t_sampler, uv).g;
        let b = textureSample(t_texture, t_sampler, uv + vec2<f32>(uv_offset, 0.0)).b;
        color = vec3<f32>(r, g, b);
    } else {
        color = textureSample(t_texture, t_sampler, uv).rgb;
    }

    // Apply scanlines
    color *= apply_scanlines(uv, crt.scanline_intensity);

    // Apply vignette
    color *= apply_vignette(uv, crt.vignette_strength);

    // Apply brightness and contrast
    color = (color - 0.5) * crt.contrast + 0.5;
    color *= crt.brightness;

    // Clamp to valid range
    color = clamp(color, vec3<f32>(0.0), vec3<f32>(1.0));

    return vec4<f32>(color, 1.0);
}
"#;
