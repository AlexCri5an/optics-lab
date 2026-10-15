//! Wavelength spectral legend bar and pure coordinate mapping helpers.
//!
//! Provides interactive wavelength visualization (380 nm to 780 nm) with
//! sample tick markers, selection indicators, and click selection.

use crate::sim::TracedRay2D;
use crate::theme::OpticsTokens;
use egui::{Align2, Color32, FontId, Pos2, Rect, Stroke, Vec2};
use optics_spectral::display_color_for_wavelength;

/// Maps a wavelength in nanometers (within [380.0, 780.0]) to an x-coordinate inside [min_x, max_x].
#[must_use]
pub fn wavelength_to_x(wavelength_nm: f64, min_x: f32, max_x: f32) -> f32 {
    let t = ((wavelength_nm - 380.0) / (780.0 - 380.0)).clamp(0.0, 1.0) as f32;
    min_x + t * (max_x - min_x)
}

/// Maps an x-coordinate inside [min_x, max_x] back to a wavelength in nanometers (within [380.0, 780.0]).
#[must_use]
pub fn x_to_wavelength(x: f32, min_x: f32, max_x: f32) -> f64 {
    let span = max_x - min_x;
    if span.abs() < 1e-5 {
        return 580.0;
    }
    let t = ((x - min_x) / span).clamp(0.0, 1.0) as f64;
    380.0 + t * (780.0 - 380.0)
}

/// Finds the index of the closest sampled ray to a clicked or hovered x-coordinate.
#[must_use]
pub fn find_closest_ray_by_x(
    x: f32,
    min_x: f32,
    max_x: f32,
    rays: &[TracedRay2D],
) -> Option<usize> {
    if rays.is_empty() {
        return None;
    }
    let target_wl = x_to_wavelength(x, min_x, max_x);
    rays.iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| {
            let diff_a = (a.wavelength_nm - target_wl).abs();
            let diff_b = (b.wavelength_nm - target_wl).abs();
            diff_a.total_cmp(&diff_b)
        })
        .map(|(idx, _)| idx)
}

/// Renders the horizontal wavelength legend bar across the given rectangle.
///
/// Returns `Some(index)` if a sample marker was clicked.
pub fn draw_wavelength_legend(
    painter: &egui::Painter,
    rect: Rect,
    rays: &[TracedRay2D],
    selected_idx: Option<usize>,
    tokens: &OpticsTokens,
) {
    let bar_height = 8.0_f32;
    let bar_rect = Rect::from_min_max(rect.min, Pos2::new(rect.max.x, rect.min.y + bar_height));

    // Draw background subtle frame
    painter.rect_filled(bar_rect, 3.0, tokens.panel_bg);

    // Draw spectral color gradient segments across 380-780 nm
    let n_steps = 60;
    let step_w = bar_rect.width() / n_steps as f32;
    for i in 0..n_steps {
        let wl0 = 380.0 + (i as f64 / n_steps as f64) * 400.0;
        let rgb = display_color_for_wavelength(wl0);
        let x0 = bar_rect.min.x + i as f32 * step_w;
        let x1 = x0 + step_w + 0.5;
        let seg_rect =
            Rect::from_min_max(Pos2::new(x0, bar_rect.min.y), Pos2::new(x1, bar_rect.max.y));
        painter.rect_filled(seg_rect, 0.0, Color32::from_rgb(rgb[0], rgb[1], rgb[2]));
    }

    // Border around bar
    painter.rect_stroke(
        bar_rect,
        3.0,
        Stroke::new(1.0, tokens.border),
        egui::StrokeKind::Inside,
    );

    // 100 nm tick marks: 400, 500, 600, 700 nm
    for wl in [400.0, 500.0, 600.0, 700.0] {
        let x = wavelength_to_x(wl, bar_rect.min.x, bar_rect.max.x);
        let tick_top = bar_rect.max.y + 1.0;
        let tick_bottom = tick_top + 4.0;
        painter.line_segment(
            [Pos2::new(x, tick_top), Pos2::new(x, tick_bottom)],
            Stroke::new(1.0, tokens.text_dim),
        );

        painter.text(
            Pos2::new(x, tick_bottom + 6.0),
            Align2::CENTER_CENTER,
            format!("{wl:.0}"),
            FontId::proportional(9.0),
            tokens.text_dim,
        );
    }

    // Sample markers (dots along the bar)
    for (idx, ray) in rays.iter().enumerate() {
        let x = wavelength_to_x(ray.wavelength_nm, bar_rect.min.x, bar_rect.max.x);
        let y = bar_rect.center().y;
        let is_selected = selected_idx == Some(idx);

        if is_selected {
            // Selected ray gets a prominent white indicator triangle above bar and ring
            painter.circle(
                Pos2::new(x, y),
                5.0,
                Color32::from_rgb(ray.color_rgb[0], ray.color_rgb[1], ray.color_rgb[2]),
                Stroke::new(2.0, Color32::WHITE),
            );

            // Pointer triangle pointing down into the bar
            let tri_tip = Pos2::new(x, bar_rect.min.y - 2.0);
            let tri_l = tri_tip + Vec2::new(-4.0, -6.0);
            let tri_r = tri_tip + Vec2::new(4.0, -6.0);
            painter.add(egui::Shape::convex_polygon(
                vec![tri_tip, tri_r, tri_l],
                Color32::WHITE,
                Stroke::NONE,
            ));
        } else {
            // Normal sample marker: small dot
            painter.circle_filled(
                Pos2::new(x, y),
                1.8,
                Color32::from_rgba_unmultiplied(255, 255, 255, 180),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wavelength_mapping_round_trip() {
        let min_x = 100.0_f32;
        let max_x = 500.0_f32;

        for wl in [380.0, 400.0, 486.13, 550.0, 587.56, 656.27, 700.0, 780.0] {
            let x = wavelength_to_x(wl, min_x, max_x);
            assert!(x >= min_x && x <= max_x);

            let round_trip_wl = x_to_wavelength(x, min_x, max_x);
            assert!(
                (round_trip_wl - wl).abs() < 1e-4,
                "Mismatch for {wl}: got {round_trip_wl}"
            );
        }
    }

    #[test]
    fn test_wavelength_mapping_endpoints() {
        let min_x = 50.0_f32;
        let max_x = 250.0_f32;

        assert!((wavelength_to_x(380.0, min_x, max_x) - min_x).abs() < 1e-5);
        assert!((wavelength_to_x(780.0, min_x, max_x) - max_x).abs() < 1e-5);
        assert!((x_to_wavelength(min_x, min_x, max_x) - 380.0).abs() < 1e-5);
        assert!((x_to_wavelength(max_x, min_x, max_x) - 780.0).abs() < 1e-5);
    }
}
