# Reverse Engineering Document 03: BVH Acceleration & Traversal Cost Heatmap Diagnostics

## 1. Overview & Inspiration
Rendering complex optical setups (curved lenses, prisms, housing apertures, mechanical mounts) requires fast ray-geometry intersection testing via a Bounding Volume Hierarchy (BVH). 

Inspirations reverse-engineered:
- **three-gpu-pathtracer (Garrett Johnson):** 2-Level BVH (TLAS / BLAS) traversal cost shader, ray-box slab testing in WGSL, heatmap color ramps.
- **rayfinder & WebGPU diagnostic tools:** Per-pixel traversal complexity visualization.

---

## 2. 2-Level BVH Architecture (TLAS & BLAS)

To support dynamic repositioning of optical elements (moving lenses, adjusting spacing) without rebuilding the entire geometry hierarchy:
1. **Top-Level Acceleration Structure (TLAS):** Contains 1 node per scene object (e.g. Lens 1, Lens 2, Prism, Aperture Stop, Mirror). Fast to update each frame.
2. **Bottom-Level Acceleration Structure (BLAS):** Contains the triangle mesh hierarchy for each optical element. Built once and cached in GPU memory.

```
                  [ TLAS Root ]
                 /             \
        [ Lens 1 Instance ]   [ Prism Instance ]
               |                     |
         [ BLAS Root ]         [ BLAS Root ]
         /           \         /           \
     [Node]        [Node]   [Node]        [Node]
     /    \        /    \   /    \        /    \
   Tri1  Tri2    Tri3  Tri4 Tri5 Tri6   Tri7  Tri8
```

---

## 3. GPU Traversal & Heatmap Implementation (WGSL)

### 3.1 Ray-AABB Slab Test
The axis-aligned bounding box (AABB) intersection is evaluated using the slab method with ray direction reciprocals ($1/\vec{d}$):

```wgsl
struct Ray {
    origin: vec3f,
    direction: vec3f,
    inv_direction: vec3f,
};

struct AABB {
    min_pt: vec3f,
    max_pt: vec3f,
};

fn intersect_aabb(ray: Ray, box: AABB) -> vec2f {
    let t0 = (box.min_pt - ray.origin) * ray.inv_direction;
    let t1 = (box.max_pt - ray.origin) * ray.inv_direction;
    let t_min = min(t0, t1);
    let t_max = max(t0, t1);
    
    let enter = max(max(t_min.x, t_min.y), t_min.z);
    let exit  = min(min(t_max.x, t_max.y), t_max.z);
    
    return vec2f(enter, exit); // hit if enter <= exit && exit >= 0.0
}
```

### 3.2 Heat Color Map Function
A smooth, perceptually intuitive temperature ramp ($Blue \to Cyan \to Green \to Yellow \to Red$):

```wgsl
fn debug_heat_color(t: f32) -> vec3f {
    let r = clamp(1.5 - abs(4.0 * t - 3.0), 0.0, 1.0);
    let g = clamp(1.5 - abs(4.0 * t - 2.0), 0.0, 1.0);
    let b = clamp(1.5 - abs(4.0 * t - 1.0), 0.0, 1.0);
    return vec3f(r, g, b);
}
```

### 3.3 Traversal Cost Accumulation
During ray traversal:
```wgsl
var tlas_tests: u32 = 0u;
var blas_tests: u32 = 0u;
var tri_tests:  u32 = 0u;

// In traversal loop:
tlas_tests += 1u;
// When walking BLAS:
blas_tests += 1u;
// When intersecting leaf triangles:
tri_tests += 1u;

let total_cost = f32(tlas_tests + blas_tests + tri_tests);
let normalized_heat = clamp(total_cost / f32(saturation_threshold), 0.0, 1.0);
let pixel_color = debug_heat_color(normalized_heat);
```

---

## 4. BVH Diagnostics UI Panel

In the OpticsLab interface:
- **Heatmap Toggle:** Switch between `Standard Render`, `BVH Traversal Cost Heatmap`, and `Surface Normal Buffer`.
- **Saturation Threshold Slider:** Adjust maximum cost threshold (e.g., 32 to 256 tests).
- **AABB Wireframe Overlay:** Render 3D bounding boxes around active optical elements with hierarchy depth color-coding.
- **Traversal Statistics:** Live counter showing Average Traversal Steps / Ray, Peak Traversal Steps, and Triangles Tested / Frame.
