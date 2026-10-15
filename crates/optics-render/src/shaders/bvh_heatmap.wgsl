// BVH Traversal Cost Heatmap Function (WGSL)

// Maps normalized traversal cost t in [0, 1] to temperature color ramp
fn debug_heat_color(t: f32) -> vec3<f32> {
    let r = clamp(1.5 - abs(4.0 * t - 3.0), 0.0, 1.0);
    let g = clamp(1.5 - abs(4.0 * t - 2.0), 0.0, 1.0);
    let b = clamp(1.5 - abs(4.0 * t - 1.0), 0.0, 1.0);
    return vec3<f32>(r, g, b);
}
