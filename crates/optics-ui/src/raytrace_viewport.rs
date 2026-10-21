//! 3D Photorealistic GPU Path Tracing Viewport.
//!
//! Integrates `optics-render` (WGSL compute path tracing) and `optics-bvh` (SAH BVH)
//! to render progressive physical path tracing in real-time within the OpticsLab GUI.

use eframe::egui::{
    self, Align, Align2, Color32, CornerRadius, FontId, Frame, Layout, Margin, Pos2, Rect, Response, RichText, Sense,
    Ui, Vec2,
};
use optics_core::{OpticalPrescription, Vec3};
use optics_render::{GpuContext, GpuMaterial, PathTracingRenderer, RenderMode, ThinLensCamera};

use crate::theme::OpticsTokens;
use crate::viewport::{Viewport3DCamera, ViewportState};

/// Engine state managing the GPU path tracer, accumulation, and presentation texture.
pub struct RaytraceEngine {
    /// Active GPU path tracing renderer.
    pub renderer: PathTracingRenderer,
    /// Hash or fingerprint of the optical prescription to detect edits.
    pub prescription_fingerprint: u64,
    /// Cached camera parameters: [azimuth, elevation, distance, pan_x, pan_y].
    pub camera_key: [i32; 5],
    /// Cached display texture uploaded to egui.
    pub texture_handle: Option<egui::TextureHandle>,
    /// Target width.
    pub width: u32,
    /// Target height.
    pub height: u32,
    /// Target max samples per pixel (progressive accumulation cap).
    pub max_spp: u64,
    /// Status message or error banner.
    pub status_message: Option<String>,
}

impl RaytraceEngine {
    /// Creates and initializes the GPU path tracing engine for the given prescription and dimensions.
    ///
    /// # Errors
    /// Returns an error string if adapter acquisition or pipeline compilation fails.
    pub fn new(
        prescription: &OpticalPrescription,
        camera: &ThinLensCamera,
        width: u32,
        height: u32,
    ) -> Result<Self, String> {
        let width = width.clamp(64, 4096);
        let height = height.clamp(64, 4096);

        let ctx = GpuContext::create_sync().map_err(|e| format!("Failed to acquire GPU compute device: {e}"))?;

        let (triangles, materials) = optics_bvh::prescription_to_trimesh_with_materials(prescription, 32, 16);

        let aabb = if triangles.is_empty() {
            optics_bvh::Aabb::from_points(&[Vec3::new(-10.0, -10.0, -10.0), Vec3::new(10.0, 10.0, 10.0)])
        } else {
            optics_bvh::Aabb::from_points(&triangles.iter().flat_map(|t| [t.v0, t.v1, t.v2]).collect::<Vec<_>>())
        };
        let inst = optics_bvh::BlasInstance::new(0, &aabb, nalgebra::Matrix4::identity());
        let scene = optics_bvh::BvhScene::build(vec![triangles], vec![inst]);

        let display_format = wgpu::TextureFormat::Rgba8Unorm;

        let mut renderer = PathTracingRenderer::new(ctx, &scene, camera, width, height, display_format)
            .map_err(|e| format!("Failed to initialize path tracing pipeline: {e}"))?;

        let gpu_materials: Vec<GpuMaterial> = materials.iter().map(|name| GpuMaterial::from_name(name)).collect();
        renderer.update_materials(&gpu_materials);

        Ok(Self {
            renderer,
            prescription_fingerprint: compute_prescription_fingerprint(prescription),
            camera_key: [0; 5],
            texture_handle: None,
            width,
            height,
            max_spp: 128,
            status_message: None,
        })
    }
}

impl std::fmt::Debug for RaytraceEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RaytraceEngine")
            .field("width", &self.width)
            .field("height", &self.height)
            .field("max_spp", &self.max_spp)
            .field("prescription_fingerprint", &self.prescription_fingerprint)
            .finish()
    }
}

/// Computes a lightweight structural fingerprint for change detection.
fn compute_prescription_fingerprint(p: &OpticalPrescription) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    p.name.hash(&mut hasher);
    p.entrance_pupil_diameter_mm.to_bits().hash(&mut hasher);
    p.surfaces.len().hash(&mut hasher);
    for s in &p.surfaces {
        s.radius_mm.to_bits().hash(&mut hasher);
        s.thickness_mm.to_bits().hash(&mut hasher);
        s.semi_diameter_mm.to_bits().hash(&mut hasher);
        s.conic_constant.to_bits().hash(&mut hasher);
        s.material_name.hash(&mut hasher);
        if let Some(cb) = &s.coordinate_break {
            cb.decenter_x_mm.to_bits().hash(&mut hasher);
            cb.decenter_y_mm.to_bits().hash(&mut hasher);
            cb.tilt_x_deg.to_bits().hash(&mut hasher);
            cb.tilt_y_deg.to_bits().hash(&mut hasher);
        }
    }
    hasher.finish()
}

/// Converts a 3D camera state into a thin-lens camera model for the path tracer.
pub(crate) fn build_thin_lens_camera(cam: &Viewport3DCamera) -> ThinLensCamera {
    let target = Vec3::new(
        cam.target_mm[0] as f64,
        cam.target_mm[1] as f64,
        cam.target_mm[2] as f64,
    );
    let cos_el = (cam.elevation_rad as f64).cos();
    let sin_el = (cam.elevation_rad as f64).sin();
    let cos_az = (cam.azimuth_rad as f64).cos();
    let sin_az = (cam.azimuth_rad as f64).sin();

    let eye = target
        + Vec3::new(
            (cam.distance_mm as f64) * cos_el * sin_az,
            (cam.distance_mm as f64) * sin_el,
            (cam.distance_mm as f64) * cos_el * cos_az,
        );

    let up_ref = if cos_el.abs() > 0.001 {
        Vec3::new(0.0, 1.0, 0.0)
    } else {
        Vec3::new(0.0, 0.0, -1.0)
    };

    let fwd = (target - eye).normalize();
    let right = fwd.cross(&up_ref).normalize();
    let up = right.cross(&fwd).normalize();

    let focus_dist = (target - eye).norm().max(1.0);

    ThinLensCamera::new(eye, target, up, cam.fov_deg as f64, 0.0, focus_dist)
}

fn make_camera_key(cam: &Viewport3DCamera) -> [i32; 5] {
    [
        (cam.azimuth_rad * 1000.0) as i32,
        (cam.elevation_rad * 1000.0) as i32,
        (cam.distance_mm * 10.0) as i32,
        (cam.pan_screen_mm.x * 10.0) as i32,
        (cam.pan_screen_mm.y * 10.0) as i32,
    ]
}

/// Renders the interactive 3D Photorealistic Path-Traced Viewport with progressive Monte Carlo accumulation.
pub fn render_raytrace_viewport(
    ui: &mut Ui,
    prescription: &OpticalPrescription,
    state: &mut ViewportState,
    tokens: &OpticsTokens,
) -> Response {
    let (rect, response) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());

    if !state.camera_3d.initialized && rect.width() > 50.0 && rect.height() > 50.0 {
        state.camera_3d.zoom_fit(prescription, rect.size());
    }

    // Top-right HUD bounds
    let is_compact_hud = rect.width() < 640.0;
    let overlay_w = if is_compact_hud { 260.0 } else { 380.0 };
    let overlay_h = 28.0;
    let overlay_rect = Rect::from_min_size(
        Pos2::new(rect.max.x - overlay_w - 10.0, rect.min.y + 10.0),
        Vec2::new(overlay_w, overlay_h),
    );

    // -----------------------------------------------------------------------
    // 1. Mouse & Gesture 3D Camera Controls (Orbit, Pan, Zoom)
    // -----------------------------------------------------------------------
    let is_over_controls = response
        .interact_pointer_pos()
        .map_or(false, |p| overlay_rect.contains(p));

    let mut camera_moved = false;

    if response.dragged() && !is_over_controls {
        let delta = response.drag_delta();
        let shift_down = ui.input(|i| i.modifiers.shift);
        let middle_drag = response.dragged_by(egui::PointerButton::Middle);

        if shift_down || middle_drag {
            let pan_scale = (state.camera_3d.distance_mm / rect.height().max(50.0)) * 0.8;
            state.camera_3d.pan_screen_mm.x += delta.x * pan_scale;
            state.camera_3d.pan_screen_mm.y -= delta.y * pan_scale;
            camera_moved = true;
        } else {
            state.camera_3d.azimuth_rad -= delta.x * 0.007;
            state.camera_3d.elevation_rad = (state.camera_3d.elevation_rad - delta.y * 0.007).clamp(-1.45, 1.45);
            camera_moved = true;
        }
    }

    if response.hovered() && !is_over_controls {
        let pinch = ui.input(|i| i.zoom_delta());
        let scroll_y = ui.input(|i| i.raw_scroll_delta.y);
        let mut zoom_factor = 1.0f32;
        if (pinch - 1.0).abs() > 0.001 {
            zoom_factor /= pinch;
        } else if scroll_y.abs() > 0.1 {
            zoom_factor *= (-scroll_y * 0.003).exp();
        }

        if (zoom_factor - 1.0).abs() > 0.001 {
            state.camera_3d.distance_mm = (state.camera_3d.distance_mm * zoom_factor).clamp(5.0, 20000.0);
            camera_moved = true;
        }
    }

    // -----------------------------------------------------------------------
    // 2. Hardware Path Tracing Progressive Execution & Presentation
    // -----------------------------------------------------------------------
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, CornerRadius::ZERO, Color32::from_rgb(12, 14, 18));

    let current_cam_key = make_camera_key(&state.camera_3d);
    let current_presc_fp = compute_prescription_fingerprint(prescription);
    let target_w = (rect.width() as u32).clamp(64, 4096);
    let target_h = (rect.height() as u32).clamp(64, 4096);

    let engine_arc = state.raytrace_engine.clone();
    let mut engine_guard = engine_arc.lock().unwrap_or_else(|e| e.into_inner());

    if engine_guard.is_none() {
        let thin_cam = build_thin_lens_camera(&state.camera_3d);
        match RaytraceEngine::new(prescription, &thin_cam, target_w, target_h) {
            Ok(eng) => *engine_guard = Some(eng),
            Err(e) => {
                painter.text(
                    rect.center(),
                    Align2::CENTER_CENTER,
                    format!("GPU Path Tracing Initialization Failed:\n{e}"),
                    FontId::monospace(12.0),
                    tokens.warning,
                );
                return response;
            }
        }
    }

    let mut current_spp = 0u64;
    let mut max_spp = 128u64;
    let mut current_mode = RenderMode::Standard;

    if let Some(engine) = engine_guard.as_mut() {
        max_spp = engine.max_spp;
        current_mode = engine.renderer.render_mode;

        // A. Handle size changes
        if engine.width != target_w || engine.height != target_h {
            engine.renderer.resize(target_w, target_h);
            engine.width = target_w;
            engine.height = target_h;
            engine.texture_handle = None;
        }

        // B. Handle prescription edits
        if engine.prescription_fingerprint != current_presc_fp {
            let (triangles, materials) = optics_bvh::prescription_to_trimesh_with_materials(prescription, 32, 16);
            let aabb = if triangles.is_empty() {
                optics_bvh::Aabb::from_points(&[Vec3::new(-10.0, -10.0, -10.0), Vec3::new(10.0, 10.0, 10.0)])
            } else {
                optics_bvh::Aabb::from_points(&triangles.iter().flat_map(|t| [t.v0, t.v1, t.v2]).collect::<Vec<_>>())
            };
            let inst = optics_bvh::BlasInstance::new(0, &aabb, nalgebra::Matrix4::identity());
            let scene = optics_bvh::BvhScene::build(vec![triangles], vec![inst]);
            engine.renderer.update_scene(&scene);

            let gpu_materials: Vec<GpuMaterial> = materials.iter().map(|name| GpuMaterial::from_name(name)).collect();
            engine.renderer.update_materials(&gpu_materials);

            engine.prescription_fingerprint = current_presc_fp;
        }

        // C. Handle camera movements
        if camera_moved || engine.camera_key != current_cam_key {
            let thin_cam = build_thin_lens_camera(&state.camera_3d);
            engine.renderer.update_camera(&thin_cam);
            engine.camera_key = current_cam_key;
        }

        // D. Progressive Sample Accumulation
        if engine.renderer.get_sample_count() < engine.max_spp {
            engine.renderer.render_frame();

            if let Ok(rgba) = engine.renderer.read_display_rgba() {
                if rgba.len() == (target_w * target_h * 4) as usize {
                    let image = egui::ColorImage::from_rgba_unmultiplied([target_w as usize, target_h as usize], &rgba);
                    if let Some(tex) = &mut engine.texture_handle {
                        tex.set(image, egui::TextureOptions::LINEAR);
                    } else {
                        engine.texture_handle = Some(ui.ctx().load_texture(
                            "gpu_path_trace_image",
                            image,
                            egui::TextureOptions::LINEAR,
                        ));
                    }
                }
            }

            ui.ctx().request_repaint();
        }

        current_spp = engine.renderer.get_sample_count();

        // E. Draw image on canvas
        if let Some(tex) = &engine.texture_handle {
            painter.image(
                tex.id(),
                rect,
                Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );
        }
    }
    drop(engine_guard);

    // -----------------------------------------------------------------------
    // 3. Top-Right Mode & Navigation HUD
    // -----------------------------------------------------------------------
    let overlay_frame = Frame::NONE
        .fill(tokens.bg_panel.gamma_multiply(0.95))
        .stroke(tokens.border)
        .corner_radius(CornerRadius::ZERO)
        .inner_margin(Margin::symmetric(4, 2));

    ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(overlay_rect)
            .layout(Layout::left_to_right(Align::Center)),
        |ui| {
            overlay_frame.show(ui, |ui| {
                ui.spacing_mut().item_spacing = Vec2::new(3.0, 0.0);

                // Telemetry Badge
                let is_converged = current_spp >= max_spp;
                let spp_badge_color = if is_converged { tokens.success } else { tokens.accent };
                ui.label(
                    RichText::new(format!("SPP: {}/{}", current_spp, max_spp))
                        .monospace()
                        .strong()
                        .size(9.5)
                        .color(spp_badge_color),
                );

                ui.separator();

                // Diagnostic render mode toggle button
                let mode_label = match current_mode {
                    RenderMode::Standard => "Realistic",
                    RenderMode::BvhHeatmap => "Heatmap",
                    RenderMode::SurfaceNormals => "Normals",
                };
                if ui
                    .button(RichText::new(mode_label).monospace().size(9.5))
                    .on_hover_text("Toggle path tracer diagnostic mode (Realistic / BVH Heatmap / Normals)")
                    .clicked()
                {
                    if let Ok(mut guard) = engine_arc.lock() {
                        if let Some(engine) = guard.as_mut() {
                            let next_mode = match engine.renderer.render_mode {
                                RenderMode::Standard => RenderMode::BvhHeatmap,
                                RenderMode::BvhHeatmap => RenderMode::SurfaceNormals,
                                RenderMode::SurfaceNormals => RenderMode::Standard,
                            };
                            engine.renderer.render_mode = next_mode;
                            engine.renderer.reset_accumulation();
                        }
                    }
                }

                // Camera Presets
                if ui.button(RichText::new("ISO").monospace().size(9.5)).clicked() {
                    state.camera_3d.azimuth_rad = 35.0f32.to_radians();
                    state.camera_3d.elevation_rad = 22.0f32.to_radians();
                    state.camera_3d.pan_screen_mm = Vec2::ZERO;
                }
                if ui.button(RichText::new("SIDE").monospace().size(9.5)).clicked() {
                    state.camera_3d.azimuth_rad = 90.0f32.to_radians();
                    state.camera_3d.elevation_rad = 0.0;
                    state.camera_3d.pan_screen_mm = Vec2::ZERO;
                }
                if ui.button(RichText::new("FIT").monospace().size(9.5)).clicked() {
                    state.camera_3d.zoom_fit(prescription, rect.size());
                }

                ui.separator();

                // Switch to 2D
                if ui
                    .button(RichText::new("2D").size(9.5).monospace().color(tokens.text_muted))
                    .on_hover_text("Switch to 2D Cross-Section (Ctrl+2)")
                    .clicked()
                {
                    state.set_mode_2d();
                }

                // Switch to 3D CAD
                if ui
                    .button(RichText::new("3D CAD").size(9.5).monospace().color(tokens.text_muted))
                    .on_hover_text("Switch to 3D Interactive CAD Wireframe/Solid (Ctrl+3)")
                    .clicked()
                {
                    state.set_mode_3d_cad();
                }

                // Active Mode indicator
                ui.label(
                    RichText::new("PHOTO")
                        .size(9.5)
                        .monospace()
                        .strong()
                        .color(tokens.accent),
                );
            });
        },
    );

    response
}
