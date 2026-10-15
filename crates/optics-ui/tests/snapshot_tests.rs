//! Harness and interaction tests for OpticsLab UI using egui_kittest.

use eframe::App;
use egui_kittest::Harness;
use optics_ui::OpticsLabApp;

#[test]
fn test_default_state_harness_1x() {
    let app = OpticsLabApp::default();
    let mut harness = Harness::builder()
        .with_size(egui::Vec2::new(900.0, 600.0))
        .with_pixels_per_point(1.0)
        .build_state(
            |ctx, state| {
                let mut frame = eframe::Frame::_new_kittest();
                state.update(ctx, &mut frame);
            },
            app,
        );
    harness.run();
}

#[test]
fn test_default_state_harness_2x() {
    let app = OpticsLabApp::default();
    let mut harness = Harness::builder()
        .with_size(egui::Vec2::new(900.0, 600.0))
        .with_pixels_per_point(2.0)
        .build_state(
            |ctx, state| {
                let mut frame = eframe::Frame::_new_kittest();
                state.update(ctx, &mut frame);
            },
            app,
        );
    harness.run();
}

#[test]
fn test_selected_ray_state_harness() {
    let app = OpticsLabApp {
        selected_ray_index: Some(5),
        ..Default::default()
    };
    let mut harness = Harness::builder()
        .with_size(egui::Vec2::new(900.0, 600.0))
        .with_pixels_per_point(1.0)
        .build_state(
            |ctx, state| {
                let mut frame = eframe::Frame::_new_kittest();
                state.update(ctx, &mut frame);
            },
            app,
        );
    harness.run();
}

#[test]
fn test_tir_edge_state_harness() {
    let mut app = OpticsLabApp::default();
    app.sim.glass_name = "SF11".to_string();
    app.sim.apex_deg = 90.0;
    app.sim.incidence_deg = 80.0;
    app.sim.retrace();
    assert_eq!(
        app.sim.detect_edge_state(),
        optics_ui::EdgeState::AllTotalInternalReflection
    );

    let mut harness = Harness::builder()
        .with_size(egui::Vec2::new(900.0, 600.0))
        .with_pixels_per_point(1.0)
        .build_state(
            |ctx, state| {
                let mut frame = eframe::Frame::_new_kittest();
                state.update(ctx, &mut frame);
            },
            app,
        );
    harness.run();
}
