//! OpticsLab Desktop — native academic optical engineering workbench.

use eframe::egui::Vec2;

fn main() -> Result<(), eframe::Error> {
    let native_options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("OpticsLab — Academic Optical Workbench")
            .with_inner_size(Vec2::new(1200.0, 780.0))
            .with_min_inner_size(Vec2::new(600.0, 420.0)),
        ..Default::default()
    };

    eframe::run_native(
        "OpticsLab",
        native_options,
        Box::new(|cc| Ok(Box::new(optics_ui::OpticsWorkbenchApp::new(cc)))),
    )
}
