// ACES Filmic Tone Mapping & Piecewise IEC 61966-2-1 sRGB Transfer Function (WGSL)

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;
    // Generate fullscreen triangle: (0,0), (2,0), (0,2) in NDC
    let x = f32((vertex_index << 1u) & 2u);
    let y = f32(vertex_index & 2u);
    out.position = vec4<f32>(x * 2.0 - 1.0, 1.0 - y * 2.0, 0.0, 1.0);
    out.uv = vec2<f32>(x, y);
    return out;
}

@group(0) @binding(0) var hdr_texture: texture_2d<f32>;
@group(0) @binding(1) var hdr_sampler: sampler;

// ACES Filmic Tone Mapping approximation
fn aces_filmic(x: vec3<f32>) -> vec3<f32> {
    let a = 2.51;
    let b = 0.03;
    let c = 2.43;
    let d = 0.59;
    let e = 0.14;
    return clamp((x * (a * x + b)) / (x * (c * x + d) + e), vec3<f32>(0.0), vec3<f32>(1.0));
}

// IEC 61966-2-1 piecewise sRGB transfer function
fn linear_to_srgb(c: f32) -> f32 {
    if (c <= 0.0031308) {
        return 12.92 * c;
    } else {
        return 1.055 * pow(c, 1.0 / 2.4) - 0.055;
    }
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let hdr_sample = textureSample(hdr_texture, hdr_sampler, in.uv).rgb;
    let mapped = aces_filmic(hdr_sample);

    let srgb = vec3<f32>(
        linear_to_srgb(mapped.r),
        linear_to_srgb(mapped.g),
        linear_to_srgb(mapped.b)
    );

    return vec4<f32>(srgb, 1.0);
}
