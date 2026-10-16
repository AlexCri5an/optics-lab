// OpticsLab Physical Path Tracer Compute Shader (WGSL)
// Iterative 2-level BVH traversal, Sellmeier spectral dispersion, GGX glossy floor, and progressive accumulation.

struct CameraUniform {
    inv_view_proj: mat4x4<f32>,
    position: vec4<f32>,
    params: vec4<f32>, // [fov_rad, aperture_radius_mm, focus_distance_mm, aspect_ratio]
    right: vec4<f32>,
    up: vec4<f32>,
    forward: vec4<f32>,
};

struct GpuBvhNode {
    min: vec3<f32>,
    left_or_first: u32,
    max: vec3<f32>,
    count: u32,
};

struct GpuTriangle {
    v0: vec4<f32>,
    v1: vec4<f32>,
    v2: vec4<f32>,
    normal: vec4<f32>, // normal.w holds material_id
};

struct RenderParams {
    frame_index: u32,
    sample_count: u32,
    max_bounces: u32,
    render_mode: u32, // 0: Standard, 1: Traversal Heatmap, 2: Surface Normals
    b1: f32,
    c1: f32,
    b2: f32,
    c2: f32,
    b3: f32,
    c3: f32,
    _pad0: f32,
    _pad1: f32,
};

@group(0) @binding(0) var<uniform> camera: CameraUniform;
@group(0) @binding(1) var<uniform> params: RenderParams;
@group(0) @binding(2) var<storage, read> bvh_nodes: array<GpuBvhNode>;
@group(0) @binding(3) var<storage, read> triangles: array<GpuTriangle>;
@group(0) @binding(4) var accum_texture: texture_storage_2d<rgba16float, write>;
@group(0) @binding(5) var<storage, read_write> accum_buffer: array<vec4<f32>>;

// PCG32 pseudo-random number generator
fn pcg32(state: ptr<function, u32>) -> f32 {
    let old = *state;
    *state = old * 747796405u + 2891336453u;
    let word = ((old >> ((old >> 28u) + 4u)) ^ old) * 277803737u;
    let res = (word >> 22u) ^ word;
    return f32(res) / 4294967295.0;
}

// Ray-AABB slab intersection test
fn intersect_aabb(ray_orig: vec3<f32>, inv_dir: vec3<f32>, b_min: vec3<f32>, b_max: vec3<f32>) -> vec2<f32> {
    let t0 = (b_min - ray_orig) * inv_dir;
    let t1 = (b_max - ray_orig) * inv_dir;
    let t_min = min(t0, t1);
    let t_max = max(t0, t1);
    let enter = max(max(t_min.x, t_min.y), t_min.z);
    let exit  = min(min(t_max.x, t_max.y), t_max.z);
    return vec2<f32>(enter, exit);
}

// Möller–Trumbore ray-triangle intersection
fn intersect_triangle(orig: vec3<f32>, dir: vec3<f32>, v0: vec3<f32>, v1: vec3<f32>, v2: vec3<f32>) -> vec4<f32> {
    let edge1 = v1 - v0;
    let edge2 = v2 - v0;
    let pvec = cross(dir, edge2);
    let det = dot(edge1, pvec);

    if (abs(det) < 1e-8) {
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }

    let inv_det = 1.0 / det;
    let tvec = orig - v0;
    let u = dot(tvec, pvec) * inv_det;
    if (u < 0.0 || u > 1.0) {
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }

    let qvec = cross(tvec, edge1);
    let v = dot(dir, qvec) * inv_det;
    if (v < 0.0 || u + v > 1.0) {
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }

    let t = dot(edge2, qvec) * inv_det;
    if (t <= 1e-4) {
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }

    return vec4<f32>(t, u, v, 1.0);
}

// BVH Traversal result
struct HitResult {
    hit: bool,
    t: f32,
    point: vec3<f32>,
    normal: vec3<f32>,
    mat_id: u32,
    traversal_cost: u32,
};

fn traverse_bvh(ray_orig: vec3<f32>, ray_dir: vec3<f32>) -> HitResult {
    var res: HitResult;
    res.hit = false;
    res.t = 1e30;
    res.traversal_cost = 0u;

    let node_count = arrayLength(&bvh_nodes);
    let tri_count = arrayLength(&triangles);

    if (node_count == 0u || tri_count == 0u) {
        return res;
    }

    let inv_dir = 1.0 / (ray_dir + sign(ray_dir) * 1e-12);

    var stack: array<u32, 32>;
    var stack_ptr: u32 = 1u;
    stack[0] = 0u;

    while (stack_ptr > 0u) {
        stack_ptr -= 1u;
        let node_idx = stack[stack_ptr];
        if (node_idx >= node_count) {
            continue;
        }

        res.traversal_cost += 1u;
        let node = bvh_nodes[node_idx];
        let slab = intersect_aabb(ray_orig, inv_dir, node.min, node.max);

        if (slab.x > slab.y || slab.y < 0.0 || slab.x >= res.t) {
            continue;
        }

        if (node.count > 0u) {
            for (var i: u32 = 0u; i < node.count; i += 1u) {
                let tri_idx = node.left_or_first + i;
                if (tri_idx < tri_count) {
                    res.traversal_cost += 1u;
                    let tri = triangles[tri_idx];
                    let tri_hit = intersect_triangle(ray_orig, ray_dir, tri.v0.xyz, tri.v1.xyz, tri.v2.xyz);

                    if (tri_hit.w > 0.5 && tri_hit.x < res.t) {
                        res.hit = true;
                        res.t = tri_hit.x;
                        res.point = ray_orig + ray_dir * tri_hit.x;
                        res.normal = tri.normal.xyz;
                        res.mat_id = u32(tri.normal.w);
                    }
                }
            }
        } else {
            if (stack_ptr < 30u) {
                stack[stack_ptr] = node.left_or_first + 1u;
                stack_ptr += 1u;
                stack[stack_ptr] = node.left_or_first;
                stack_ptr += 1u;
            }
        }
    }

    return res;
}

// Physical Snell/Fresnel dielectric interaction
fn evaluate_dielectric(
    dir: vec3<f32>,
    normal: vec3<f32>,
    n1: f32,
    n2: f32,
    rng: ptr<function, u32>,
    out_dir: ptr<function, vec3<f32>>
) -> vec3<f32> {
    var n = normal;
    var cos_theta_i = -dot(dir, n);
    var eta = n1 / n2;

    if (cos_theta_i < 0.0) {
        cos_theta_i = -cos_theta_i;
        n = -n;
        eta = n2 / n1;
    }

    let sin2_theta_i = max(0.0, 1.0 - cos_theta_i * cos_theta_i);
    let sin2_theta_t = eta * eta * sin2_theta_i;

    if (sin2_theta_t >= 1.0) {
        *out_dir = reflect(dir, n);
        return vec3<f32>(1.0, 1.0, 1.0);
    }

    let cos_theta_t = sqrt(1.0 - sin2_theta_t);
    let r_perp = (eta * cos_theta_i - cos_theta_t) / (eta * cos_theta_i + cos_theta_t);
    let r_par  = (cos_theta_i - eta * cos_theta_t) / (cos_theta_i + eta * cos_theta_t);
    let r_total = 0.5 * (r_perp * r_perp + r_par * r_par);

    if (pcg32(rng) < r_total) {
        *out_dir = reflect(dir, n);
    } else {
        *out_dir = normalize(eta * dir + (eta * cos_theta_i - cos_theta_t) * n);
    }

    return vec3<f32>(1.0, 1.0, 1.0);
}

// Evaluates refractive index n using Sellmeier equation with dynamic coefficients (lambda in nm)
fn sellmeier_glass(lambda_nm: f32) -> f32 {
    let l_um = clamp(lambda_nm, 350.0, 850.0) * 0.001;
    let l2 = l_um * l_um;
    let n2 = 1.0 
        + (params.b1 * l2) / (l2 - params.c1)
        + (params.b2 * l2) / (l2 - params.c2)
        + (params.b3 * l2) / (l2 - params.c3);
    return sqrt(max(1.0, n2));
}

// Convert wavelength (nm) to normalized sRGB color
fn wavelength_to_rgb(wl: f32) -> vec3<f32> {
    var r = 0.0;
    var g = 0.0;
    var b = 0.0;
    if (wl >= 380.0 && wl < 440.0) {
        r = -(wl - 440.0) / (440.0 - 380.0);
        b = 1.0;
    } else if (wl >= 440.0 && wl < 490.0) {
        g = (wl - 440.0) / (490.0 - 440.0);
        b = 1.0;
    } else if (wl >= 490.0 && wl < 510.0) {
        g = 1.0;
        b = -(wl - 510.0) / (510.0 - 490.0);
    } else if (wl >= 510.0 && wl < 580.0) {
        r = (wl - 510.0) / (580.0 - 510.0);
        g = 1.0;
    } else if (wl >= 580.0 && wl < 645.0) {
        r = 1.0;
        g = -(wl - 645.0) / (645.0 - 580.0);
    } else if (wl >= 645.0 && wl <= 780.0) {
        r = 1.0;
    }
    var factor = 0.0;
    if (wl >= 380.0 && wl < 420.0) {
        factor = 0.3 + 0.7 * (wl - 380.0) / (420.0 - 380.0);
    } else if (wl >= 420.0 && wl <= 700.0) {
        factor = 1.0;
    } else if (wl > 700.0 && wl <= 780.0) {
        factor = 0.3 + 0.7 * (780.0 - wl) / (780.0 - 700.0);
    }
    return vec3<f32>(pow(r * factor, 0.8), pow(g * factor, 0.8), pow(b * factor, 0.8));
}

// Sample GGX visible normal distribution
fn sample_ggx(normal: vec3<f32>, roughness: f32, rng: ptr<function, u32>) -> vec3<f32> {
    let u1 = pcg32(rng);
    let u2 = pcg32(rng);
    let a = roughness * roughness;
    let phi = 2.0 * 3.14159265 * u1;
    let cos_theta = sqrt((1.0 - u2) / (1.0 + (a * a - 1.0) * u2));
    let sin_theta = sqrt(max(0.0, 1.0 - cos_theta * cos_theta));

    let h = vec3<f32>(sin_theta * cos(phi), cos_theta, sin_theta * sin(phi));

    var up = vec3<f32>(0.0, 1.0, 0.0);
    if (abs(normal.y) > 0.999) {
        up = vec3<f32>(1.0, 0.0, 0.0);
    }
    let tangent = normalize(cross(up, normal));
    let bitangent = cross(normal, tangent);
    return normalize(tangent * h.x + normal * h.y + bitangent * h.z);
}

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let dims = textureDimensions(accum_texture);
    if (global_id.x >= dims.x || global_id.y >= dims.y) {
        return;
    }

    var rng = global_id.x + global_id.y * dims.x + params.frame_index * 1999999973u;

    let sub_pixel = vec2<f32>(pcg32(&rng), pcg32(&rng));
    let uv = (vec2<f32>(global_id.xy) + sub_pixel) / vec2<f32>(dims);

    // Primary ray from thin-lens camera
    let theta_rad = camera.params.x;
    let focus_dist = camera.params.z;
    let aspect = camera.params.w;
    let half_h = tan(theta_rad * 0.5) * focus_dist;
    let half_w = half_h * aspect;

    let p_focus = camera.position.xyz
        + camera.forward.xyz * focus_dist
        + camera.right.xyz * ((2.0 * uv.x - 1.0) * half_w)
        + camera.up.xyz * ((1.0 - 2.0 * uv.y) * half_h);

    var ray_orig = camera.position.xyz;
    var ray_dir = normalize(p_focus - ray_orig);

    // Sample hero wavelength for dispersion
    let hero_wl = 380.0 + pcg32(&rng) * 400.0;
    let glass_n = sellmeier_glass(hero_wl);
    let spectral_weight = wavelength_to_rgb(hero_wl);

    var radiance = vec3<f32>(0.0, 0.0, 0.0);
    var throughput = vec3<f32>(1.0, 1.0, 1.0);
    var total_cost: u32 = 0u;

    for (var bounce: u32 = 0u; bounce < params.max_bounces; bounce += 1u) {
        let hit = traverse_bvh(ray_orig, ray_dir);
        total_cost += hit.traversal_cost;

        if (!hit.hit) {
            let t = 0.5 * (ray_dir.y + 1.0);
            let sky_color = mix(vec3<f32>(0.02, 0.025, 0.035), vec3<f32>(0.06, 0.07, 0.10), t);
            radiance += throughput * sky_color;
            break;
        }

        if (params.render_mode == 2u) {
            radiance = hit.normal * 0.5 + vec3<f32>(0.5, 0.5, 0.5);
            break;
        }

        if (hit.mat_id == 0u) {
            // Material 0: Dielectric Prism with Sellmeier spectral dispersion
            var next_dir: vec3<f32>;
            let attenuation = evaluate_dielectric(ray_dir, hit.normal, 1.0, glass_n, &rng, &next_dir);
            throughput *= attenuation;
            ray_dir = next_dir;
            let nudge = sign(dot(ray_dir, hit.normal)) * hit.normal * 1e-4;
            ray_orig = hit.point + nudge;
        } else if (hit.mat_id == 1u) {
            // Material 1: Glossy Dark Floor Plane (GGX reflection + subtle floor grid)
            let floor_roughness = 0.05;
            let h = sample_ggx(hit.normal, floor_roughness, &rng);
            let spec_dir = reflect(ray_dir, h);

            let f0 = 0.04;
            let cos_theta = max(dot(-ray_dir, h), 0.0);
            let fresnel = f0 + (1.0 - f0) * pow(1.0 - cos_theta, 5.0);

            if (pcg32(&rng) < fresnel) {
                ray_dir = spec_dir;
                throughput *= vec3<f32>(0.92, 0.92, 0.95);
            } else {
                let grid_pos = fract(abs(hit.point.xz) * 0.04);
                var floor_albedo = vec3<f32>(0.02, 0.022, 0.028);
                if (grid_pos.x < 0.03 || grid_pos.y < 0.03) {
                    floor_albedo = vec3<f32>(0.05, 0.055, 0.07);
                }
                throughput *= floor_albedo;
                let u1 = pcg32(&rng);
                let u2 = pcg32(&rng);
                let r = sqrt(u1);
                let theta = 2.0 * 3.14159265 * u2;
                let diff_dir = normalize(vec3<f32>(r * cos(theta), sqrt(max(0.0, 1.0 - u1)), r * sin(theta)));
                ray_dir = diff_dir;
            }
            ray_orig = hit.point + hit.normal * 1e-4;
        } else {
            // Diffuse receiver
            throughput *= vec3<f32>(0.8, 0.8, 0.8);
            ray_orig = hit.point + hit.normal * 1e-4;
            break;
        }

        if (bounce >= 3u) {
            let p_continue = max(max(throughput.x, throughput.y), throughput.z);
            if (pcg32(&rng) > p_continue) {
                break;
            }
            throughput /= p_continue;
        }
    }

    if (params.render_mode == 1u) {
        let norm_cost = clamp(f32(total_cost) / 128.0, 0.0, 1.0);
        let r = clamp(1.5 - abs(4.0 * norm_cost - 3.0), 0.0, 1.0);
        let g = clamp(1.5 - abs(4.0 * norm_cost - 2.0), 0.0, 1.0);
        let b = clamp(1.5 - abs(4.0 * norm_cost - 1.0), 0.0, 1.0);
        radiance = vec3<f32>(r, g, b);
    }

    let color_sample = radiance * spectral_weight * 2.5;

    let pixel_idx = global_id.x + global_id.y * dims.x;
    var accumulated: vec4<f32>;
    if (params.sample_count == 0u) {
        accumulated = vec4<f32>(color_sample, 1.0);
    } else {
        let prev = accum_buffer[pixel_idx];
        accumulated = prev + vec4<f32>(color_sample, 1.0);
    }
    accum_buffer[pixel_idx] = accumulated;

    let avg = accumulated.xyz / f32(params.sample_count + 1u);
    textureStore(accum_texture, vec2<i32>(global_id.xy), vec4<f32>(avg, 1.0));
}
