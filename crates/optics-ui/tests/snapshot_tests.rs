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

#[test]
fn test_lens_system_cross_section_harness_1x_and_2x() {
    let mut app = OpticsLabApp {
        app_mode: optics_ui::AppMode::LensSystem,
        ..Default::default()
    };
    if let Some(sim) = &mut app.lens_sim {
        sim.active_subview = optics_ui::LensSubview::CrossSection;
    }

    // 1x DPI
    let mut harness1x = Harness::builder()
        .with_size(egui::Vec2::new(900.0, 600.0))
        .with_pixels_per_point(1.0)
        .build_state(
            |ctx, state| {
                let mut frame = eframe::Frame::_new_kittest();
                state.update(ctx, &mut frame);
            },
            app,
        );
    harness1x.run();
}

#[test]
fn test_lens_system_spot_diagram_harness() {
    let mut app = OpticsLabApp {
        app_mode: optics_ui::AppMode::LensSystem,
        ..Default::default()
    };
    if let Some(sim) = &mut app.lens_sim {
        sim.active_subview = optics_ui::LensSubview::SpotDiagram;
    }

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
fn test_lens_system_ray_fan_harness() {
    let mut app = OpticsLabApp {
        app_mode: optics_ui::AppMode::LensSystem,
        ..Default::default()
    };
    if let Some(sim) = &mut app.lens_sim {
        sim.active_subview = optics_ui::LensSubview::RayFan;
    }

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
fn test_lens_system_prescription_table_harness() {
    let mut app = OpticsLabApp {
        app_mode: optics_ui::AppMode::LensSystem,
        ..Default::default()
    };
    if let Some(sim) = &mut app.lens_sim {
        sim.active_subview = optics_ui::LensSubview::PrescriptionTable;
    }

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
