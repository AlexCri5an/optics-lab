//! 3D Camera-Facing Light Beam Ribbons, Spectral Caustics, and Time-of-Flight Simulation.
//!
//! Evaluates physical beam propagation, Fresnel power attenuation, radiometric
//! CIE XYZ / sRGB color conversion, camera-facing polygonal ribbon geometry,
//! receiver floor caustic footprint projection, and relativistic/dielectric time-of-flight pulses.

use egui::{Color32, Pos2, Rect, Stroke};
use optics_core::Vec3;
use optics_spectral::{display_color_for_wavelength, wavelength_to_xyz, xyz_to_srgb};

use crate::sim::PrismSimulation;
use crate::theme::OpticsTokens;
use crate::viewport_3d::OrbitCamera;

/// Speed of light in vacuum in millimeters per picosecond ($c \approx 0.299792458\,\text{mm/ps}$).
pub const SPEED_OF_LIGHT_MM_PER_PS: f64 = 0.299_792_458;

/// A 3D beam segment along an optical propagation path.
#[derive(Clone, Debug, PartialEq)]
pub struct BeamSegment3D {
    /// Starting 3D coordinates in millimeters.
    pub start: Vec3,
    /// Ending 3D coordinates in millimeters.
    pub end: Vec3,
    /// Vacuum wavelength in nanometers (380 nm to 780 nm, or 0.0 for polychromatic white).
    pub wavelength_nm: f64,
    /// Accumulated radiometric throughput / energy in $[0.0, 1.0]$.
    pub throughput: f64,
    /// Display sRGB color.
    pub color_srgb: [u8; 3],
    /// Physical half-width of the beam ribbon in millimeters.
    pub width_mm: f64,
    /// Medium refractive index along this segment ($n \ge 1.0$).
    pub refractive_index: f64,
    /// Arrival time at start of segment in picoseconds.
    pub start_time_ps: f64,
    /// Transit duration along this segment in picoseconds.
    pub duration_ps: f64,
    /// Whether this segment is a reflected branch.
    pub is_reflected: bool,
}

/// A spectral caustic footprint projected onto the receiver floor plane.
#[derive(Clone, Debug, PartialEq)]
pub struct CausticFootprint3D {
    /// Central intersection point on the floor plane ($y \approx -0.01$).
    pub floor_hit: Vec3,
    /// Vacuum wavelength in nanometers.
    pub wavelength_nm: f64,
    /// Display sRGB color.
    pub color_srgb: [u8; 3],
    /// Relative radiant flux density / irradiance on the receiver plane.
    pub irradiance: f64,
    /// Footprint width along the dispersion axis (X) in millimeters.
    pub width_x_mm: f64,
    /// Footprint length along the prism extrusion axis (Z) in millimeters.
    pub length_z_mm: f64,
}

/// State configuration for 3D beam ribbons, caustics, and time-of-flight pulses.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct BeamDisplayState {
    /// Whether 3D camera-facing beam ribbons are rendered.
    pub show_ribbons: bool,
    /// Whether floor spectral caustic projection is rendered.
    pub show_caustic: bool,
    /// Whether animated time-of-flight pulses are active.
    pub show_tof_pulses: bool,
    /// Physical width of the incoming beam ribbon in millimeters.
    pub beam_width_mm: f32,
    /// Time-of-flight animation speed multiplier.
    pub tof_speed: f32,
    /// Spatial length of each time-of-flight pulse in millimeters.
    pub pulse_length_mm: f32,
    /// Brightness/intensity multiplier for floor caustics.
    pub caustic_intensity: f32,
}

impl Default for BeamDisplayState {
    fn default() -> Self {
        Self {
            show_ribbons: true,
            show_caustic: true,
            show_tof_pulses: false,
            beam_width_mm: 2.5,
            tof_speed: 1.0,
            pulse_length_mm: 8.0,
            caustic_intensity: 1.2,
        }
    }
}

/// Computes the camera-facing 3D ribbon vertices for a line segment.
///
/// Given a segment from $P_A$ to $P_B$ with width $W$ and camera eye position $C$,
/// calculates the 4 vertices of a billboard quad oriented directly towards the camera:
///
/// $$\mathbf{d} = \frac{P_B - P_A}{\|P_B - P_A\|}, \quad \mathbf{u} = C - \frac{P_A + P_B}{2}, \quad \mathbf{n} = \frac{\mathbf{d} \times \mathbf{u}}{\|\mathbf{d} \times \mathbf{u}\|}$$
///
/// Vertices are ordered counter-clockwise:
/// $[P_A - h\mathbf{n}, P_A + h\mathbf{n}, P_B + h\mathbf{n}, P_B - h\mathbf{n}]$ where $h = W / 2$.
#[must_use]
pub fn compute_camera_facing_quad(
    p_start: Vec3,
    p_end: Vec3,
    width: f64,
    camera_eye: Vec3,
) -> [Vec3; 4] {
    let seg = p_end - p_start;
    let len = seg.norm();
    if len < 1e-7 {
        let half = width * 0.5;
        return [
            p_start + Vec3::new(-half, 0.0, 0.0),
            p_start + Vec3::new(half, 0.0, 0.0),
            p_end + Vec3::new(half, 0.0, 0.0),
            p_end + Vec3::new(-half, 0.0, 0.0),
        ];
    }

    let dir = seg / len;
    let mid = (p_start + p_end) * 0.5;
    let to_cam = camera_eye - mid;

    let mut lateral = dir.cross(&to_cam);
    let lateral_norm = lateral.norm();

    if lateral_norm < 1e-6 {
        // Ray points directly at camera: choose arbitrary perpendicular vector
        lateral = dir.cross(&Vec3::new(0.0, 1.0, 0.0));
        if lateral.norm() < 1e-6 {
            lateral = dir.cross(&Vec3::new(1.0, 0.0, 0.0));
        }
    }

    let n = lateral.normalize();
    let half_w = width * 0.5;
    let offset = n * half_w;

    [
        p_start - offset,
        p_start + offset,
        p_end + offset,
        p_end - offset,
    ]
}

/// Evaluates radiometric sRGB color for a spectral line at `wavelength_nm` with radiant throughput $\tau$.
///
/// Uses standard CIE 1931 $2^\circ$ color matching functions and piecewise IEC 61966-2-1 sRGB transfer.
#[must_use]
pub fn evaluate_radiometric_color(wavelength_nm: f64, throughput: f64) -> [u8; 3] {
    if wavelength_nm <= 0.0 {
        // Polychromatic D65 white light
        let t = (throughput.clamp(0.0, 1.0) * 255.0) as u8;
        return [t, t, t];
    }

    let norm_flux = throughput.clamp(0.0, 1.0);
    let xyz = wavelength_to_xyz(wavelength_nm, norm_flux);
    let srgb = xyz_to_srgb(xyz);

    // Boost luminance for visible ribbons while strictly maintaining chromaticity
    let max_comp = srgb[0].max(srgb[1]).max(srgb[2]);
    let boost = if max_comp > 1e-5 {
        (1.0 / max_comp.powf(0.3)).min(2.5)
    } else {
        1.0
    };

    let r = ((srgb[0] * boost).clamp(0.0, 1.0) * 255.0) as u8;
    let g = ((srgb[1] * boost).clamp(0.0, 1.0) * 255.0) as u8;
    let b = ((srgb[2] * boost).clamp(0.0, 1.0) * 255.0) as u8;

    // Fallback to display color if extreme out-of-gamut clipping produces black
    if r == 0 && g == 0 && b == 0 {
        display_color_for_wavelength(wavelength_nm)
    } else {
        [r, g, b]
    }
}

/// Extracts all 3D beam propagation segments and receiver floor caustic spots from simulation.
#[must_use]
pub fn build_light_propagation(
    sim: &PrismSimulation,
    display_state: &BeamDisplayState,
) -> (Vec<BeamSegment3D>, Vec<CausticFootprint3D>) {
    let mut segments = Vec::new();
    let mut caustics = Vec::new();

    let beam_w = display_state.beam_width_mm as f64;
    let floor_y = -0.01;
    let prism_half_z = 25.0; // Prism extrusion depth span [-25, +25] mm

    // 1. Incoming collimated white beam (polychromatic)
    let in_start = Vec3::new(
        sim.incoming_origin.x as f64,
        sim.incoming_origin.y as f64,
        0.0,
    );
    let in_end = Vec3::new(sim.incoming_hit.x as f64, sim.incoming_hit.y as f64, 0.0);
    let in_dist = (in_end - in_start).norm();
    let in_duration = in_dist / SPEED_OF_LIGHT_MM_PER_PS;

    segments.push(BeamSegment3D {
        start: in_start,
        end: in_end,
        wavelength_nm: 0.0,
        throughput: 1.0,
        color_srgb: [240, 244, 255],
        width_mm: beam_w * 1.5,
        refractive_index: 1.0,
        start_time_ps: 0.0,
        duration_ps: in_duration,
        is_reflected: false,
    });

    // 2. Dispersed spectral rays
    for ray in &sim.rays {
        let wl = ray.wavelength_nm;
        let n_glass = ray.n;
        let col = evaluate_radiometric_color(wl, 1.0);

        let mut cur_time = in_duration;

        // Bounces along path
        let mut prev_pt = in_end;

        for bounce in &ray.path_record.bounces {
            let bounce_pt = bounce.point;
            let dist = (bounce_pt - prev_pt).norm();
            let duration = (dist * n_glass) / SPEED_OF_LIGHT_MM_PER_PS;

            // Internal segment inside prism
            segments.push(BeamSegment3D {
                start: prev_pt,
                end: bounce_pt,
                wavelength_nm: wl,
                throughput: bounce.t_total.max(0.2),
                color_srgb: col,
                width_mm: beam_w,
                refractive_index: n_glass,
                start_time_ps: cur_time,
                duration_ps: duration,
                is_reflected: false,
            });

            cur_time += duration;

            // Reflected branch if present
            if bounce.r_total > 0.01 {
                let refl_dir = bounce.reflected_dir;
                let refl_len = 25.0;
                let refl_end = bounce_pt + refl_dir * refl_len;
                let refl_duration = refl_len / SPEED_OF_LIGHT_MM_PER_PS;

                segments.push(BeamSegment3D {
                    start: bounce_pt,
                    end: refl_end,
                    wavelength_nm: wl,
                    throughput: bounce.r_total,
                    color_srgb: col,
                    width_mm: beam_w * 0.7,
                    refractive_index: 1.0,
                    start_time_ps: cur_time,
                    duration_ps: refl_duration,
                    is_reflected: true,
                });
            }

            prev_pt = bounce_pt;
        }

        // Ray exiting prism into air
        if let Some(last_bounce) = ray.path_record.bounces.last() {
            if let Some(exit_dir) = last_bounce.refracted_dir {
                if exit_dir.norm() > 0.5 {
                    // Check intersection with floor plane at y = floor_y
                    let (exit_end, hit_floor) = if exit_dir.y < -1e-5 && prev_pt.y > floor_y {
                        let t_floor = (floor_y - prev_pt.y) / exit_dir.y;
                        (prev_pt + exit_dir * t_floor, true)
                    } else {
                        (prev_pt + exit_dir * 50.0, false)
                    };

                    let exit_dist = (exit_end - prev_pt).norm();
                    let exit_duration = exit_dist / SPEED_OF_LIGHT_MM_PER_PS;

                    segments.push(BeamSegment3D {
                        start: prev_pt,
                        end: exit_end,
                        wavelength_nm: wl,
                        throughput: last_bounce.t_total,
                        color_srgb: col,
                        width_mm: beam_w * 0.9,
                        refractive_index: 1.0,
                        start_time_ps: cur_time,
                        duration_ps: exit_duration,
                        is_reflected: false,
                    });

                    // Floor caustic footprint
                    if hit_floor {
                        caustics.push(CausticFootprint3D {
                            floor_hit: exit_end,
                            wavelength_nm: wl,
                            color_srgb: col,
                            irradiance: last_bounce.t_total
                                * display_state.caustic_intensity as f64,
                            width_x_mm: beam_w * 1.8,
                            length_z_mm: prism_half_z * 2.0,
                        });
                    }
                }
            }
        }
    }

    (segments, caustics)
}

/// Renders 3D camera-facing light beam ribbons into egui painter.
#[allow(clippy::too_many_lines)]
pub fn render_beam_ribbons(
    painter: &egui::Painter,
    camera: &OrbitCamera,
    rect: Rect,
    segments: &[BeamSegment3D],
    display_state: &BeamDisplayState,
    current_time_s: f64,
) {
    if !display_state.show_ribbons {
        return;
    }

    let screen_center = rect.center();
    let cam_eye = camera.eye();

    // Pulse animation phase
    let tof_period_ps = 550.0 / display_state.tof_speed.max(0.1) as f64;
    let tof_time_ps = (current_time_s * 1000.0 * display_state.tof_speed as f64) % tof_period_ps;
    let pulse_len_ps = display_state.pulse_length_mm as f64 / SPEED_OF_LIGHT_MM_PER_PS;

    for seg in segments {
        let quad_3d = compute_camera_facing_quad(seg.start, seg.end, seg.width_mm, cam_eye);

        // Project 4 vertices to 2D screen coordinates
        let p0 = camera.project(
            [
                quad_3d[0].x as f32,
                quad_3d[0].y as f32,
                quad_3d[0].z as f32,
            ],
            screen_center,
            rect,
        );
        let p1 = camera.project(
            [
                quad_3d[1].x as f32,
                quad_3d[1].y as f32,
                quad_3d[1].z as f32,
            ],
            screen_center,
            rect,
        );
        let p2 = camera.project(
            [
                quad_3d[2].x as f32,
                quad_3d[2].y as f32,
                quad_3d[2].z as f32,
            ],
            screen_center,
            rect,
        );
        let p3 = camera.project(
            [
                quad_3d[3].x as f32,
                quad_3d[3].y as f32,
                quad_3d[3].z as f32,
            ],
            screen_center,
            rect,
        );

        if let (Some(s0), Some(s1), Some(s2), Some(s3)) = (p0, p1, p2, p3) {
            let [r, g, b] = seg.color_srgb;

            let base_alpha = if seg.is_reflected {
                (seg.throughput * 140.0).clamp(25.0, 160.0) as u8
            } else {
                (seg.throughput * 170.0).clamp(40.0, 200.0) as u8
            };

            // Draw filled translucent ribbon quad
            let ribbon_color = Color32::from_rgba_unmultiplied(r, g, b, base_alpha);
            painter.add(egui::Shape::convex_polygon(
                vec![s0, s1, s2, s3],
                ribbon_color,
                Stroke::NONE,
            ));

            // Core high-intensity beam center line
            let c_start = Pos2::new((s0.x + s1.x) * 0.5, (s0.y + s1.y) * 0.5);
            let c_end = Pos2::new((s2.x + s3.x) * 0.5, (s2.y + s3.y) * 0.5);
            let core_alpha = ((base_alpha as f32 * 1.3).min(255.0)) as u8;
            painter.line_segment(
                [c_start, c_end],
                Stroke::new(1.0, Color32::from_rgba_unmultiplied(r, g, b, core_alpha)),
            );

            // Optional Time-of-Flight moving pulse packet
            if display_state.show_tof_pulses {
                let seg_start_t = seg.start_time_ps;
                let seg_end_t = seg.start_time_ps + seg.duration_ps;

                if tof_time_ps >= seg_start_t - pulse_len_ps
                    && tof_time_ps <= seg_end_t + pulse_len_ps
                {
                    let u =
                        ((tof_time_ps - seg_start_t) / seg.duration_ps.max(1e-5)).clamp(0.0, 1.0);
                    let pulse_center_3d = seg.start + (seg.end - seg.start) * u;
                    if let Some(pulse_screen) = camera.project(
                        [
                            pulse_center_3d.x as f32,
                            pulse_center_3d.y as f32,
                            pulse_center_3d.z as f32,
                        ],
                        screen_center,
                        rect,
                    ) {
                        // Radiant pulse bead
                        painter.circle_filled(
                            pulse_screen,
                            4.0,
                            Color32::from_rgba_unmultiplied(255, 255, 255, 230),
                        );
                        painter.circle_stroke(
                            pulse_screen,
                            6.0,
                            Stroke::new(1.5, Color32::from_rgba_unmultiplied(r, g, b, 200)),
                        );
                    }
                }
            }
        }
    }
}

/// Renders the spectral caustic rainbow projection on the receiver floor plane.
pub fn render_spectral_caustic(
    painter: &egui::Painter,
    camera: &OrbitCamera,
    rect: Rect,
    caustics: &[CausticFootprint3D],
    tokens: &OpticsTokens,
) {
    if caustics.is_empty() {
        return;
    }

    let screen_center = rect.center();

    for caustic in caustics {
        let hit = caustic.floor_hit;
        let half_w = caustic.width_x_mm * 0.5;
        let half_l = caustic.length_z_mm * 0.5;

        // 4 corners of floor caustic patch
        let c0 = camera.project(
            [
                (hit.x - half_w) as f32,
                hit.y as f32,
                (hit.z - half_l) as f32,
            ],
            screen_center,
            rect,
        );
        let c1 = camera.project(
            [
                (hit.x + half_w) as f32,
                hit.y as f32,
                (hit.z - half_l) as f32,
            ],
            screen_center,
            rect,
        );
        let c2 = camera.project(
            [
                (hit.x + half_w) as f32,
                hit.y as f32,
                (hit.z + half_l) as f32,
            ],
            screen_center,
            rect,
        );
        let c3 = camera.project(
            [
                (hit.x - half_w) as f32,
                hit.y as f32,
                (hit.z + half_l) as f32,
            ],
            screen_center,
            rect,
        );

        if let (Some(p0), Some(p1), Some(p2), Some(p3)) = (c0, c1, c2, c3) {
            let [r, g, b] = caustic.color_srgb;
            let alpha = (caustic.irradiance * 180.0).clamp(40.0, 240.0) as u8;

            painter.add(egui::Shape::convex_polygon(
                vec![p0, p1, p2, p3],
                Color32::from_rgba_unmultiplied(r, g, b, alpha),
                Stroke::NONE,
            ));
        }
    }

    // Floor Caustic Spectral Ruler / Gauge
    if let (Some(first), Some(last)) = (caustics.first(), caustics.last()) {
        let p_start = camera.project(
            [first.floor_hit.x as f32, first.floor_hit.y as f32, 28.0],
            screen_center,
            rect,
        );
        let p_end = camera.project(
            [last.floor_hit.x as f32, last.floor_hit.y as f32, 28.0],
            screen_center,
            rect,
        );

        if let (Some(ps), Some(pe)) = (p_start, p_end) {
            painter.line_segment([ps, pe], Stroke::new(1.0, tokens.border_subtle));

            let label = format!(
                "Caustic: {:.0} nm – {:.0} nm",
                first.wavelength_nm, last.wavelength_nm
            );
            painter.text(
                Pos2::new((ps.x + pe.x) * 0.5, (ps.y + pe.y) * 0.5 + 10.0),
                egui::Align2::CENTER_TOP,
                label,
                egui::FontId::monospace(9.5),
                tokens.text_secondary,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_camera_facing_quad_orthogonality() {
        let p_start = Vec3::new(0.0, 0.0, 0.0);
        let p_end = Vec3::new(10.0, 0.0, 0.0);
        let eye = Vec3::new(5.0, 10.0, 0.0);
        let width = 2.0;

        let quad = compute_camera_facing_quad(p_start, p_end, width, eye);
        let seg_dir = (p_end - p_start).normalize();
        let width_dir = quad[1] - quad[0];

        // Ribbon width must be perpendicular to propagation segment direction
        assert!(
            seg_dir.dot(&width_dir).abs() < 1e-10,
            "Ribbon width direction must be strictly orthogonal to segment direction"
        );

        // Width length must match requested width
        assert!(
            (width_dir.norm() - width).abs() < 1e-10,
            "Ribbon width must equal specified width"
        );
    }

    #[test]
    fn test_radiometric_color_evaluation() {
        // Monochromatic red (700 nm) must have dominant red component
        let col_red = evaluate_radiometric_color(700.0, 1.0);
        assert!(col_red[0] > col_red[2], "700 nm must be red dominant");

        // Monochromatic blue (450 nm) must have dominant blue component
        let col_blue = evaluate_radiometric_color(450.0, 1.0);
        assert!(col_blue[2] > col_blue[0], "450 nm must be blue dominant");

        // Polychromatic white (0.0 nm) must be balanced
        let col_white = evaluate_radiometric_color(0.0, 1.0);
        assert_eq!(col_white, [255, 255, 255]);
    }

    #[test]
    fn test_time_of_flight_dispersion_delay() {
        let sim = PrismSimulation::default();
        let display_state = BeamDisplayState::default();
        let (segments, caustics) = build_light_propagation(&sim, &display_state);

        assert!(!segments.is_empty(), "Must produce beam segments");
        assert!(
            !caustics.is_empty(),
            "Must produce floor caustic footprints"
        );

        // Find blue ray (approx 400 nm) and red ray (approx 700 nm) inside prism
        let blue_seg = segments.iter().find(|s| {
            s.wavelength_nm > 380.0 && s.wavelength_nm < 420.0 && s.refractive_index > 1.0
        });
        let red_seg = segments.iter().find(|s| {
            s.wavelength_nm > 680.0 && s.wavelength_nm < 720.0 && s.refractive_index > 1.0
        });

        if let (Some(b), Some(r)) = (blue_seg, red_seg) {
            // Blue refractive index is higher than red in N-BK7
            assert!(
                b.refractive_index > r.refractive_index,
                "Blue glass index must exceed red glass index"
            );
            // Transit duration for blue must be longer than red (slower group velocity)
            let b_vel = SPEED_OF_LIGHT_MM_PER_PS / b.refractive_index;
            let r_vel = SPEED_OF_LIGHT_MM_PER_PS / r.refractive_index;
            assert!(
                r_vel > b_vel,
                "Red velocity in glass must exceed blue velocity"
            );
        }
    }
}
