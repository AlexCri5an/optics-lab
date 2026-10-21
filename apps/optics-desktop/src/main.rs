//! OpticsLab Desktop — native academic optical engineering workbench.

use eframe::egui::Vec2;

fn load_icon() -> Option<egui::IconData> {
    eframe::icon_data::from_png_bytes(include_bytes!("../../../assets/icon-256.png")).ok()
}

fn main() -> Result<(), eframe::Error> {
    let mut viewport = eframe::egui::ViewportBuilder::default()
        .with_title("OpticsLab")
        .with_app_id("com.alexandrucrisan.optics-lab")
        .with_inner_size(Vec2::new(1280.0, 800.0))
        .with_min_inner_size(Vec2::new(800.0, 500.0));

    if let Some(icon) = load_icon() {
        viewport = viewport.with_icon(icon);
    }

    let native_options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        "OpticsLab",
        native_options,
        Box::new(|cc| Ok(Box::new(optics_ui::OpticsWorkbenchApp::new(cc)))),
    )
}
