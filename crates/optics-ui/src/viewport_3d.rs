//! Real hardware-accelerated 3D viewport powered by WebGPU / wgpu path tracing.
//!
//! Provides progressive path tracing of physical glass dispersion, Sellmeier materials,
//! GGX glossy reflection floor, full camera orbit/pan/zoom controls, and vector overlays.

use egui::{Color32, Pos2, Rect, Stroke, Vec2};
use nalgebra::Matrix4;
use optics_bvh::{Aabb, BlasInstance, BvhScene, Triangle};
use optics_core::Vec3;
use optics_render::{GpuContext, PathTracingRenderer, RenderMode, ThinLensCamera};
use std::sync::{Arc, Mutex};

use crate::app::AppMode;
use crate::beam_ribbon::{
    BeamDisplayState, build_lens_light_propagation, build_light_propagation, render_beam_ribbons,
    render_spectral_caustic,
};
use crate::lens_sim::LensSimulation;
use crate::sim::PrismSimulation;
use crate::theme::OpticsTokens;

/// Orbital camera model for 3D viewport navigation.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct OrbitCamera {
    /// Azimuthal angle around target in radians.
    pub yaw: f32,
    /// Elevation angle in radians [-1.55, 1.55].
    pub pitch: f32,
    /// Distance from target in millimeters.
    pub distance: f32,
    /// Center point of focus [x, y, z] in mm.
    pub target: [f32; 3],
    /// Vertical field of view in degrees.
    pub fov_deg: f32,
}

impl Default for OrbitCamera {
    fn default() -> Self {
        Self {
            yaw: 0.52,
            pitch: 0.38,
            distance: 280.0,
            target: [-10.0, 35.0, 0.0],
            fov_deg: 45.0,
        }
    }
}

impl OrbitCamera {
    /// Computes the camera eye position in 3D world space.
    #[must_use]
    pub fn eye(&self) -> Vec3 {
        let x = self.target[0] as f64
            + (self.distance as f64) * (self.pitch as f64).cos() * (self.yaw as f64).sin();
        let y = self.target[1] as f64 + (self.distance as f64) * (self.pitch as f64).sin();
        let z = self.target[2] as f64
            + (self.distance as f64) * (self.pitch as f64).cos() * (self.yaw as f64).cos();
        Vec3::new(x, y, z)
    }

    /// Converts to a [`ThinLensCamera`] for GPU path tracing.
    #[must_use]
    pub fn to_thin_lens_camera(&self) -> ThinLensCamera {
        let eye = self.eye();
        let target = Vec3::new(
            self.target[0] as f64,
            self.target[1] as f64,
            self.target[2] as f64,
        );
        let dist = (eye - target).norm();
        ThinLensCamera::new(
            eye,
            target,
            Vec3::new(0.0, 1.0, 0.0),
            self.fov_deg as f64,
            0.0,
            dist,
        )
    }

    /// Projects a 3D point into 2D viewport coordinates.
    #[must_use]
    pub fn project(&self, pt: [f32; 3], screen_center: Pos2, rect: Rect) -> Option<Pos2> {
        let eye = self.eye();
        let p_rel = [
            pt[0] - eye.x as f32,
            pt[1] - eye.y as f32,
            pt[2] - eye.z as f32,
        ];

        let cos_y = self.yaw.cos();
        let sin_y = self.yaw.sin();
        let cos_p = self.pitch.cos();
        let sin_p = self.pitch.sin();

        let right = [cos_y, 0.0, -sin_y];
        let forward = [-cos_p * sin_y, -sin_p, -cos_p * cos_y];
        let up = [-sin_p * sin_y, cos_p, -sin_p * cos_y];

        let depth = p_rel[0] * forward[0] + p_rel[1] * forward[1] + p_rel[2] * forward[2];
        if depth <= 1.0 {
            return None;
        }

        let fov_rad = (self.fov_deg * std::f32::consts::PI / 180.0) * 0.5;
        let scale = (rect.height() * 0.5) / (fov_rad.tan() * depth);

        let x = (p_rel[0] * right[0] + p_rel[1] * right[1] + p_rel[2] * right[2]) * scale;
        let y = (p_rel[0] * up[0] + p_rel[1] * up[1] + p_rel[2] * up[2]) * scale;

        Some(Pos2::new(screen_center.x + x, screen_center.y - y))
    }
}

/// Constructs a watertight 3D [`BvhScene`] representing the prism and glossy floor plane.
#[must_use]
pub fn build_prism_scene_bvh(sim: &PrismSimulation) -> BvhScene {
    let half_apex_rad = (sim.apex_deg * 0.5).to_radians();
    let base_width = 80.0;
    let height = (base_width * 0.5) / half_apex_rad.tan().max(0.1);
    let depth_half = 40.0;

    let v_apex_front = Vec3::new(0.0, height, depth_half);
    let v_left_front = Vec3::new(-base_width * 0.5, 0.0, depth_half);
    let v_right_front = Vec3::new(base_width * 0.5, 0.0, depth_half);

    let v_apex_back = Vec3::new(0.0, height, -depth_half);
    let v_left_back = Vec3::new(-base_width * 0.5, 0.0, -depth_half);
    let v_right_back = Vec3::new(base_width * 0.5, 0.0, -depth_half);

    let prism_triangles = vec![
        // Front face
        Triangle::new(v_left_front, v_right_front, v_apex_front, 0),
        // Back face
        Triangle::new(v_right_back, v_left_back, v_apex_back, 0),
        // Left entry face (2 triangles)
        Triangle::new(v_left_back, v_left_front, v_apex_front, 0),
        Triangle::new(v_left_back, v_apex_front, v_apex_back, 0),
        // Right exit face (2 triangles)
        Triangle::new(v_right_front, v_right_back, v_apex_front, 0),
        Triangle::new(v_right_back, v_apex_back, v_apex_front, 0),
        // Bottom face (2 triangles)
        Triangle::new(v_left_front, v_left_back, v_right_front, 0),
        Triangle::new(v_right_front, v_left_back, v_right_back, 0),
    ];

    // Floor plane (2 triangles, material 1)
    let floor_size = 500.0;
    let f0 = Vec3::new(-floor_size, -0.01, -floor_size);
    let f1 = Vec3::new(floor_size, -0.01, -floor_size);
    let f2 = Vec3::new(floor_size, -0.01, floor_size);
    let f3 = Vec3::new(-floor_size, -0.01, floor_size);

    let floor_triangles = vec![Triangle::new(f0, f1, f2, 1), Triangle::new(f0, f2, f3, 1)];

    let prism_aabb = prism_triangles
        .iter()
        .fold(Aabb::empty(), |a, t| a.union(&t.aabb()));
    let floor_aabb = floor_triangles
        .iter()
        .fold(Aabb::empty(), |a, t| a.union(&t.aabb()));

    let mesh_geometries = vec![prism_triangles, floor_triangles];
    let instances = vec![
        BlasInstance::new(0, &prism_aabb, Matrix4::identity()),
        BlasInstance::new(1, &floor_aabb, Matrix4::identity()),
    ];

    BvhScene::build(mesh_geometries, instances)
}

/// Dynamic GPU path tracer viewport state and real telemetry counters.
#[derive(Debug)]
pub struct GpuViewportState {
    /// Physical GPU adapter brand/model.
    pub adapter_name: String,
    /// Active GPU graphics API backend (e.g. Metal, Vulkan, DX12, WebGPU).
    pub backend_name: String,
    /// Selected GPU device type (Discrete, Integrated, CPU).
    pub device_type: String,
    /// Whether hardware acceleration is successfully active.
    pub is_available: bool,
    /// Hardware validation error string if captured.
    pub validation_error: Option<String>,
    /// Accumulated sample count for the current scene configuration.
    pub sample_count: u64,
    /// Target sample count before convergence pausing.
    pub target_spp: u32,
    /// Whether rendering loop is paused.
    pub is_paused: bool,
    /// Super-sampling or sub-sampling scale factor.
    pub resolution_scale: f32,
    /// Active rendering diagnostic mode.
    pub render_mode: RenderMode,
    /// Whether bloom post-processing is enabled.
    pub bloom_enabled: bool,
    /// Monotonic duration of last frame in milliseconds.
    pub last_frame_time_ms: f64,
    /// Exponentially smoothed real frame rate.
    pub measured_fps: f64,
    /// Interactive orbital camera.
    pub camera: OrbitCamera,
    /// Path tracer pipeline instance.
    pub renderer: Option<Arc<Mutex<PathTracingRenderer>>>,
    /// Registered egui texture identifier for viewport blit.
    pub egui_texture_id: Option<egui::TextureId>,
    /// Beam ribbon, spectral caustic, and time-of-flight configuration.
    pub beam_display: BeamDisplayState,
}

impl Default for GpuViewportState {
    fn default() -> Self {
        Self {
            adapter_name: "Unavailable (Headless / WebGPU unsupported)".into(),
            backend_name: "None".into(),
            device_type: "Unknown".into(),
            is_available: false,
            validation_error: None,
            sample_count: 0,
            target_spp: 256,
            is_paused: false,
            resolution_scale: 1.0,
            render_mode: RenderMode::Standard,
            bloom_enabled: false,
            last_frame_time_ms: 0.0,
            measured_fps: 60.0,
            camera: OrbitCamera::default(),
            renderer: None,
            egui_texture_id: None,
            beam_display: BeamDisplayState::default(),
        }
    }
}

impl GpuViewportState {
    /// Resets progressive path tracing sample accumulation.
    pub fn reset_accumulation(&mut self) {
        self.sample_count = 0;
        if let Some(r) = &self.renderer {
            if let Ok(mut lock) = r.lock() {
                lock.reset_accumulation();
            }
        }
    }

    /// Re-uploads an updated BVH acceleration structure and resets sample accumulation.
    pub fn update_scene_bvh(&mut self, scene: &BvhScene) {
        if let Some(r) = &self.renderer {
            if let Ok(mut lock) = r.lock() {
                lock.update_scene(scene);
            }
        }
        self.sample_count = 0;
    }

    /// Initializes hardware acceleration using the given `eframe` wgpu render state and scene.
    pub fn init_from_wgpu_render_state(
        &mut self,
        wgpu_state: &egui_wgpu::RenderState,
        scene: &BvhScene,
    ) {
        let adapter_info = wgpu_state.adapter.get_info();
        self.adapter_name = adapter_info.name.clone();
        self.backend_name = format!("{:?}", adapter_info.backend);
        self.device_type = format!("{:?}", adapter_info.device_type);

        let instance = wgpu::Instance::default();
        let gpu_ctx = GpuContext::from_parts(
            instance,
            wgpu_state.adapter.clone(),
            wgpu_state.device.clone(),
            wgpu_state.queue.clone(),
        );

        let camera = self.camera.to_thin_lens_camera();
        let initial_w = 800;
        let initial_h = 600;

        match PathTracingRenderer::new(
            gpu_ctx,
            scene,
            &camera,
            initial_w,
            initial_h,
            wgpu_state.target_format,
        ) {
            Ok(renderer) => {
                let tex_id = wgpu_state.renderer.write().register_native_texture(
                    &wgpu_state.device,
                    &renderer.display_target.view,
                    wgpu::FilterMode::Linear,
                );
                self.renderer = Some(Arc::new(Mutex::new(renderer)));
                self.egui_texture_id = Some(tex_id);
                self.is_available = true;
            }
            Err(e) => {
                self.validation_error = Some(format!("GPU Renderer initialization failed: {e}"));
            }
        }
    }
}

/// Renders the real 3D hardware-accelerated viewport with camera orbit/pan/zoom and overlays.
#[allow(clippy::too_many_lines)]
pub fn render_viewport_3d(
    ui: &mut egui::Ui,
    rect: Rect,
    sim: &PrismSimulation,
    lens_sim: Option<&LensSimulation>,
    app_mode: AppMode,
    tokens: &OpticsTokens,
    gpu_state: &mut GpuViewportState,
) {
    let ppp = ui.ctx().pixels_per_point();
    let target_pixel_w = ((rect.width() * ppp * gpu_state.resolution_scale).round() as u32).max(64);
    let target_pixel_h =
        ((rect.height() * ppp * gpu_state.resolution_scale).round() as u32).max(64);

    let (response, painter) = ui.allocate_painter(rect.size(), egui::Sense::drag());

    // 1. Interactive orbital camera handling
    let mut camera_moved = false;

    if response.dragged_by(egui::PointerButton::Secondary)
        || (response.dragged() && ui.input(|i| i.modifiers.alt))
    {
        let delta = response.drag_delta();
        gpu_state.camera.yaw += delta.x * 0.008;
        gpu_state.camera.pitch = (gpu_state.camera.pitch + delta.y * 0.008).clamp(-1.5, 1.5);
        camera_moved = true;
    }

    if response.dragged_by(egui::PointerButton::Middle)
        || (response.dragged() && ui.input(|i| i.modifiers.shift))
    {
        let delta = response.drag_delta();
        let scale = gpu_state.camera.distance * 0.002;
        gpu_state.camera.target[0] -= delta.x * scale;
        gpu_state.camera.target[1] += delta.y * scale;
        camera_moved = true;
    }

    if response.hovered() {
        let scroll = ui.input(|i| i.smooth_scroll_delta.y);
        if scroll.abs() > 0.1 {
            gpu_state.camera.distance =
                (gpu_state.camera.distance - scroll * 0.6).clamp(30.0, 900.0);
            camera_moved = true;
        }
    }

    if camera_moved {
        gpu_state.reset_accumulation();
        if let Some(r) = &gpu_state.renderer {
            if let Ok(mut lock) = r.lock() {
                lock.update_camera(&gpu_state.camera.to_thin_lens_camera());
            }
        }
        ui.ctx().request_repaint();
    }

    // 2. Hardware Path Tracing presentation
    if gpu_state.is_available && gpu_state.renderer.is_some() {
        let renderer_arc = if let Some(r) = &gpu_state.renderer {
            r.clone()
        } else {
            return;
        };

        // Execute frame pass if active and not paused
        if !gpu_state.is_paused && gpu_state.sample_count < gpu_state.target_spp as u64 {
            let start = std::time::Instant::now();
            if let Ok(mut r) = renderer_arc.lock() {
                let material = match sim.glass_name.as_str() {
                    "SF11" => optics_glass::GlassMaterial::sf11(),
                    "F2" => optics_glass::GlassMaterial::f2(),
                    "Fused Silica" => optics_glass::GlassMaterial::fused_silica(),
                    _ => optics_glass::GlassMaterial::n_bk7(),
                };
                if let optics_glass::material::DispersionModel::Sellmeier1 { b, c } =
                    material.dispersion
                {
                    r.set_material_sellmeier(
                        [b[0] as f32, b[1] as f32, b[2] as f32],
                        [c[0] as f32, c[1] as f32, c[2] as f32],
                    );
                }
                r.render_mode = gpu_state.render_mode;
                r.resize(target_pixel_w, target_pixel_h);
                r.render_frame();
                gpu_state.sample_count = r.get_sample_count().min(gpu_state.target_spp as u64);
            }
            let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
            gpu_state.last_frame_time_ms = elapsed_ms;
            let dt = ui.input(|i| i.stable_dt).max(0.001) as f64;
            let current_fps = 1.0 / dt;
            gpu_state.measured_fps = gpu_state.measured_fps * 0.9 + current_fps * 0.1;
            ui.ctx().request_repaint();
        }

        // Draw texture if registered
        if let Some(tex_id) = gpu_state.egui_texture_id {
            painter.image(
                tex_id,
                rect,
                Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );
        }
    } else {
        // Fallback for headless environments or unsupported WebGPU:
        // Render background canvas and 3D geometric wireframe
        painter.rect_filled(rect, egui::CornerRadius::ZERO, tokens.canvas);

        let screen_center = rect.center();
        let wire_stroke = Stroke::new(1.0, tokens.border_subtle);

        if let (AppMode::LensSystem, Some(l_sim)) = (app_mode, lens_sim) {
            for elem in &l_sim.elements {
                for window in elem.boundary_points.windows(2) {
                    let p_a = [0.0_f32, window[0][1] as f32, window[0][0] as f32];
                    let p_b = [0.0_f32, window[1][1] as f32, window[1][0] as f32];
                    if let (Some(s_a), Some(s_b)) = (
                        gpu_state.camera.project(p_a, screen_center, rect),
                        gpu_state.camera.project(p_b, screen_center, rect),
                    ) {
                        painter.line_segment([s_a, s_b], wire_stroke);
                    }
                }
            }
        } else {
            // Project prism 3D wireframe edges
            let half_apex_rad = (sim.apex_deg * 0.5).to_radians();
            let base_width = 80.0_f32;
            let height = (base_width * 0.5) / (half_apex_rad.tan() as f32).max(0.1);
            let depth_half = 40.0_f32;

            let v_af = [0.0, height, depth_half];
            let v_lf = [-base_width * 0.5, 0.0, depth_half];
            let v_rf = [base_width * 0.5, 0.0, depth_half];
            let v_ab = [0.0, height, -depth_half];
            let v_lb = [-base_width * 0.5, 0.0, -depth_half];
            let v_rb = [base_width * 0.5, 0.0, -depth_half];

            let edges = [
                (v_lf, v_rf),
                (v_rf, v_af),
                (v_af, v_lf),
                (v_lb, v_rb),
                (v_rb, v_ab),
                (v_ab, v_lb),
                (v_lf, v_lb),
                (v_rf, v_rb),
                (v_af, v_ab),
            ];

            for (p_a, p_b) in edges {
                if let (Some(s_a), Some(s_b)) = (
                    gpu_state.camera.project(p_a, screen_center, rect),
                    gpu_state.camera.project(p_b, screen_center, rect),
                ) {
                    painter.line_segment([s_a, s_b], wire_stroke);
                }
            }
        }

        let msg = if let Some(err) = &gpu_state.validation_error {
            err.as_str()
        } else {
            "3D Viewport: Vector beam visualization"
        };

        painter.text(
            Pos2::new(rect.center().x, rect.bottom() - 25.0),
            egui::Align2::CENTER_BOTTOM,
            msg,
            egui::FontId::proportional(11.0),
            tokens.text_dim,
        );
    }

    // 3. Beam Ribbons and Spectral Caustics
    let (segments, caustics) = if app_mode == AppMode::LensSystem {
        if let Some(l_sim) = lens_sim {
            (
                build_lens_light_propagation(l_sim, &gpu_state.beam_display),
                Vec::new(),
            )
        } else {
            (Vec::new(), Vec::new())
        }
    } else {
        build_light_propagation(sim, &gpu_state.beam_display)
    };

    if gpu_state.beam_display.show_caustic {
        render_spectral_caustic(&painter, &gpu_state.camera, rect, &caustics, tokens);
    }

    let current_time_s = ui.input(|i| i.time);
    render_beam_ribbons(
        &painter,
        &gpu_state.camera,
        rect,
        &segments,
        &gpu_state.beam_display,
        current_time_s,
    );

    if gpu_state.beam_display.show_tof_pulses {
        ui.ctx().request_repaint();
    }

    // 4. Viewport vector overlays
    render_viewport_overlays(&painter, rect, tokens, gpu_state, sim);
}

/// Renders orientation axis gizmo and live telemetry tags over the viewport.
fn render_viewport_overlays(
    painter: &egui::Painter,
    rect: Rect,
    tokens: &OpticsTokens,
    gpu_state: &GpuViewportState,
    sim: &PrismSimulation,
) {
    // A. 3D Coordinate Axis Orientation Gizmo in bottom-left
    let gizmo_center = Pos2::new(rect.left() + 45.0, rect.bottom() - 45.0);
    let cos_y = gpu_state.camera.yaw.cos();
    let sin_y = gpu_state.camera.yaw.sin();
    let cos_p = gpu_state.camera.pitch.cos();
    let sin_p = gpu_state.camera.pitch.sin();

    let right = [cos_y, 0.0, -sin_y];
    let up = [-sin_p * sin_y, cos_p, -sin_p * cos_y];

    let axis_len = 28.0;

    // X axis (Red)
    let p_x = Pos2::new(
        gizmo_center.x + right[0] * axis_len,
        gizmo_center.y - up[0] * axis_len,
    );
    painter.line_segment(
        [gizmo_center, p_x],
        Stroke::new(2.0, Color32::from_rgb(220, 60, 60)),
    );
    painter.text(
        p_x,
        egui::Align2::CENTER_CENTER,
        "X",
        egui::FontId::monospace(10.0),
        Color32::from_rgb(220, 60, 60),
    );

    // Y axis (Green)
    let p_y = Pos2::new(
        gizmo_center.x + right[1] * axis_len,
        gizmo_center.y - up[1] * axis_len,
    );
    painter.line_segment(
        [gizmo_center, p_y],
        Stroke::new(2.0, Color32::from_rgb(60, 200, 60)),
    );
    painter.text(
        p_y,
        egui::Align2::CENTER_CENTER,
        "Y",
        egui::FontId::monospace(10.0),
        Color32::from_rgb(60, 200, 60),
    );

    // Z axis (Blue)
    let p_z = Pos2::new(
        gizmo_center.x + right[2] * axis_len,
        gizmo_center.y - up[2] * axis_len,
    );
    painter.line_segment(
        [gizmo_center, p_z],
        Stroke::new(2.0, Color32::from_rgb(60, 120, 240)),
    );
    painter.text(
        p_z,
        egui::Align2::CENTER_CENTER,
        "Z",
        egui::FontId::monospace(10.0),
        Color32::from_rgb(60, 120, 240),
    );

    // B. Live Telemetry Tag in top-left
    let status_str = if gpu_state.is_paused {
        "PAUSED"
    } else if gpu_state.sample_count >= gpu_state.target_spp as u64 {
        "COMPLETE"
    } else {
        "TRACING"
    };

    let tag_text = format!(
        "{} | SPP: {}/{} | {:.1} ms ({:.0} FPS)",
        status_str,
        gpu_state.sample_count,
        gpu_state.target_spp,
        gpu_state.last_frame_time_ms,
        gpu_state.measured_fps
    );

    let tag_rect = Rect::from_min_size(
        Pos2::new(rect.left() + 10.0, rect.top() + 10.0),
        Vec2::new(280.0, 22.0),
    );

    painter.rect_filled(
        tag_rect,
        egui::CornerRadius::same(2),
        Color32::from_rgba_unmultiplied(10, 12, 16, 200),
    );
    painter.rect_stroke(
        tag_rect,
        egui::CornerRadius::same(2),
        Stroke::new(1.0, tokens.border_subtle),
        egui::StrokeKind::Inside,
    );
    painter.text(
        Pos2::new(tag_rect.left() + 8.0, tag_rect.center().y),
        egui::Align2::LEFT_CENTER,
        tag_text,
        egui::FontId::monospace(10.5),
        tokens.text_primary,
    );

    // C. Physical Edge Condition Banner (TIR warning)
    let edge_state = sim.detect_edge_state();
    if edge_state != crate::sim::EdgeState::Normal {
        let banner_text = match edge_state {
            crate::sim::EdgeState::AllTotalInternalReflection => {
                "Total internal reflection: rays reflect internally and cannot escape exit face"
            }
            crate::sim::EdgeState::NoRaysTransmitted => {
                "No rays transmitted through prism exit face"
            }
            crate::sim::EdgeState::RayMissedPrism => {
                "Incoming ray does not intersect prism surface"
            }
            crate::sim::EdgeState::Normal => "",
        };
        let banner_rect = Rect::from_center_size(
            Pos2::new(rect.center().x, rect.top() + 24.0),
            Vec2::new((rect.width() - 80.0).clamp(200.0, 560.0), 24.0),
        );
        painter.rect_filled(
            banner_rect,
            egui::CornerRadius::same(2),
            Color32::from_rgba_unmultiplied(220, 150, 40, 25),
        );
        painter.rect_stroke(
            banner_rect,
            egui::CornerRadius::same(2),
            Stroke::new(1.0, tokens.status_warning),
            egui::StrokeKind::Inside,
        );
        painter.text(
            banner_rect.center(),
            egui::Align2::CENTER_CENTER,
            banner_text,
            egui::FontId::proportional(11.0),
            tokens.text_primary,
        );
    }
}
