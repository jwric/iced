struct Uniforms {
    coverage: vec2<f32>,
    scanline_intensity: f32,
    screen_curvature: f32,
    rgb_separation: f32,
    vignette_strength: f32,
    brightness: f32,
    contrast: f32,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@group(0) @binding(0) var source: texture_2d<f32>;
@group(0) @binding(1) var source_sampler: sampler;

@group(1) @binding(0) var<uniform> uniforms: Uniforms;

// A triangle strip covering the upscaled target, anchored to the top left
// corner of the surface.
//
// `coverage` is greater than 1 when the surface is not an exact multiple of
// the pixel scale; the overflow is simply clipped away, which keeps every
// pixel of the source exactly `pixel_scale` pixels wide.
@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> VertexOutput {
    let uv = vec2<f32>(f32(index & 1u), f32((index >> 1u) & 1u));

    var output: VertexOutput;
    output.position = vec4<f32>(
        -1.0 + 2.0 * uv.x * uniforms.coverage.x,
        1.0 - 2.0 * uv.y * uniforms.coverage.y,
        0.0,
        1.0,
    );
    output.uv = uv;

    return output;
}

// Barrel distortion, for the curvature of a CRT screen
fn curve(uv: vec2<f32>, amount: f32) -> vec2<f32> {
    if amount == 0.0 {
        return uv;
    }

    let centered = uv * 2.0 - 1.0;
    let distance = length(centered);

    return centered * (1.0 + amount * distance * distance) * 0.5 + 0.5;
}

// Darkening towards the edges of the screen
fn vignette(uv: vec2<f32>, strength: f32) -> f32 {
    if strength == 0.0 {
        return 1.0;
    }

    let centered = uv * 2.0 - 1.0;

    return 1.0 - smoothstep(0.5, 1.5, length(centered)) * strength;
}

// Alternating dark lines, one per row of source pixels
fn scanlines(uv: vec2<f32>, intensity: f32) -> f32 {
    if intensity == 0.0 {
        return 1.0;
    }

    let rows = f32(textureDimensions(source).y);
    let scanline = sin(uv.y * rows * 3.14159265359);

    return 1.0 - (scanline * 0.5 + 0.5) * intensity;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let uv = curve(input.uv, uniforms.screen_curvature);

    // Anything pushed outside of the source by the curvature is off-screen
    if uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0 {
        return vec4<f32>(0.0, 0.0, 0.0, 1.0);
    }

    var color: vec3<f32>;

    if uniforms.rgb_separation > 0.0 {
        // The separation is expressed in pixels of the source, so that it
        // stays consistent no matter the resolution
        let offset =
            uniforms.rgb_separation / f32(textureDimensions(source).x);

        color = vec3<f32>(
            textureSample(source, source_sampler, uv - vec2<f32>(offset, 0.0)).r,
            textureSample(source, source_sampler, uv).g,
            textureSample(source, source_sampler, uv + vec2<f32>(offset, 0.0)).b,
        );
    } else {
        color = textureSample(source, source_sampler, uv).rgb;
    }

    color *= scanlines(uv, uniforms.scanline_intensity);
    color *= vignette(uv, uniforms.vignette_strength);

    color = (color - 0.5) * uniforms.contrast + 0.5;
    color *= uniforms.brightness;

    return vec4<f32>(clamp(color, vec3<f32>(0.0), vec3<f32>(1.0)), 1.0);
}
