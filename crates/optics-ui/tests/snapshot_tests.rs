//! Visual regression snapshot test matrix using egui_kittest with wgpu rendering.
//!
//! Evaluates the target "Classic desktop" look across:
//! - Themes: Classic Light (default), Classic Dark, High Contrast
//! - Viewport sizes: 640x480 (compact), 900x600 (regular), 1024x768 (standard), 1920x1080 (wide)
//! - DPI scaling: 1.0x (standard), 2.0x (HiDPI / Retina)
//! - Simulation states: Default Prism, Ray selected, TIR edge condition, Lens cross-section,
//!   Spot diagram, Ray fan, Prescription table, Glass catalog, Preferences dialog, About dialog.

use eframe::App;
use egui::Vec2;
use egui_kittest::Harness;
use optics_ui::{AppMode, LensPreset, LensSubview, OpticsLabApp, RightPanelTab, ThemePreference};

fn run_snapshot(name: &str, app: OpticsLabApp, size: Vec2, ppp: f32) {
    let mut harness = Harness::builder()
        .wgpu()
        .with_size(size)
        .with_pixels_per_point(ppp)
        .build_state(
            |ctx, state| {
                let mut frame = eframe::Frame::_new_kittest();
                state.update(ctx, &mut frame);
            },
            app,
        );
    harness.snapshot(name);
}

// ---------------------------------------------------------------------------
// 1. Theme Matrix (Classic Light, Classic Dark, High Contrast)
// ---------------------------------------------------------------------------

#[test]
fn test_snapshot_theme_classic_light() {
    let mut app = OpticsLabApp::default();
    app.prefs.theme = ThemePreference::ClassicLight;
    run_snapshot("theme_classic_light", app, Vec2::new(1024.0, 768.0), 1.0);
}

#[test]
fn test_snapshot_theme_classic_dark() {
    let mut app = OpticsLabApp::default();
    app.prefs.theme = ThemePreference::ClassicDark;
    run_snapshot("theme_classic_dark", app, Vec2::new(1024.0, 768.0), 1.0);
}

#[test]
fn test_snapshot_theme_high_contrast() {
    let mut app = OpticsLabApp::default();
    app.prefs.theme = ThemePreference::HighContrast;
    run_snapshot("theme_high_contrast", app, Vec2::new(1024.0, 768.0), 1.0);
}

// ---------------------------------------------------------------------------
// 2. Responsive Breakpoints and DPI Scales
// ---------------------------------------------------------------------------

#[test]
fn test_snapshot_size_compact_640x480() {
    let app = OpticsLabApp::default();
    run_snapshot("size_compact_640x480", app, Vec2::new(640.0, 480.0), 1.0);
}

#[test]
fn test_snapshot_size_regular_900x600_1x() {
    let app = OpticsLabApp::default();
    run_snapshot("size_regular_900x600_1x", app, Vec2::new(900.0, 600.0), 1.0);
}

#[test]
fn test_snapshot_size_regular_900x600_2x() {
    let app = OpticsLabApp::default();
    run_snapshot("size_regular_900x600_2x", app, Vec2::new(900.0, 600.0), 2.0);
}

#[test]
fn test_snapshot_size_wide_1920x1080() {
    let app = OpticsLabApp::default();
    run_snapshot("size_wide_1920x1080", app, Vec2::new(1920.0, 1080.0), 1.0);
}

// ---------------------------------------------------------------------------
// 3. Simulation States & Edge Cases
// ---------------------------------------------------------------------------

#[test]
fn test_snapshot_state_ray_selected() {
    let app = OpticsLabApp {
        selected_ray_index: Some(6),
        ..Default::default()
    };
    run_snapshot("state_ray_selected", app, Vec2::new(1024.0, 768.0), 1.0);
}

#[test]
fn test_snapshot_state_tir_edge() {
    let mut app = OpticsLabApp::default();
    app.sim.glass_name = "SF11".to_string();
    app.sim.apex_deg = 80.0;
    app.sim.incidence_deg = 75.0;
    app.sim.retrace();
    assert_eq!(
        app.sim.detect_edge_state(),
        optics_ui::EdgeState::AllTotalInternalReflection
    );
    run_snapshot("state_tir_edge", app, Vec2::new(1024.0, 768.0), 1.0);
}

#[test]
fn test_snapshot_state_lens_cross_section() {
    let mut app = OpticsLabApp {
        app_mode: AppMode::LensSystem,
        viewport_mode: optics_ui::ViewportMode::View2D,
        right_panel_tab: RightPanelTab::Prescription,
        ..Default::default()
    };
    if let Some(sim) = &mut app.lens_sim {
        sim.select_preset(LensPreset::CookeTriplet);
        sim.active_subview = LensSubview::CrossSection;
    }
    run_snapshot(
        "state_lens_cross_section",
        app,
        Vec2::new(1024.0, 768.0),
        1.0,
    );
}

#[test]
fn test_snapshot_state_lens_spot_diagram() {
    let mut app = OpticsLabApp {
        app_mode: AppMode::LensSystem,
        viewport_mode: optics_ui::ViewportMode::View2D,
        right_panel_tab: RightPanelTab::Prescription,
        ..Default::default()
    };
    if let Some(sim) = &mut app.lens_sim {
        sim.select_preset(LensPreset::CookeTriplet);
        sim.active_subview = LensSubview::SpotDiagram;
    }
    run_snapshot(
        "state_lens_spot_diagram",
        app,
        Vec2::new(1024.0, 768.0),
        1.0,
    );
}

#[test]
fn test_snapshot_state_lens_ray_fan() {
    let mut app = OpticsLabApp {
        app_mode: AppMode::LensSystem,
        viewport_mode: optics_ui::ViewportMode::View2D,
        right_panel_tab: RightPanelTab::Prescription,
        ..Default::default()
    };
    if let Some(sim) = &mut app.lens_sim {
        sim.select_preset(LensPreset::CookeTriplet);
        sim.active_subview = LensSubview::RayFan;
    }
    run_snapshot("state_lens_ray_fan", app, Vec2::new(1024.0, 768.0), 1.0);
}

#[test]
fn test_snapshot_state_lens_prescription_table() {
    let mut app = OpticsLabApp {
        app_mode: AppMode::LensSystem,
        viewport_mode: optics_ui::ViewportMode::View2D,
        right_panel_tab: RightPanelTab::Prescription,
        ..Default::default()
    };
    if let Some(sim) = &mut app.lens_sim {
        sim.select_preset(LensPreset::CookeTriplet);
        sim.active_subview = LensSubview::PrescriptionTable;
    }
    run_snapshot(
        "state_lens_prescription_table",
        app,
        Vec2::new(1024.0, 768.0),
        1.0,
    );
}

#[test]
fn test_snapshot_state_glass_catalog() {
    let mut app = OpticsLabApp {
        right_panel_tab: RightPanelTab::GlassCatalog,
        ..Default::default()
    };
    app.glass_browser_state.selected_glass = "N-BK7".to_string();
    run_snapshot("state_glass_catalog", app, Vec2::new(1024.0, 768.0), 1.0);
}

// ---------------------------------------------------------------------------
// 4. Modal Dialog States
// ---------------------------------------------------------------------------

#[test]
fn test_snapshot_dialog_preferences() {
    let app = OpticsLabApp {
        show_preferences: true,
        ..Default::default()
    };
    run_snapshot("dialog_preferences", app, Vec2::new(1024.0, 768.0), 1.0);
}

#[test]
fn test_snapshot_dialog_about() {
    let app = OpticsLabApp {
        show_about: true,
        ..Default::default()
    };
    run_snapshot("dialog_about", app, Vec2::new(1024.0, 768.0), 1.0);
}
