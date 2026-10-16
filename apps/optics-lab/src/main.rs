//! OpticsLab — native desktop entry point.
//!
//! Launches the egui application via `eframe::run_native`.

use eframe::egui::Vec2;

fn main() -> Result<(), eframe::Error> {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--self-test") {
        println!("============================================================");
        println!("OpticsLab Native GUI Headless Self-Test");
        println!("============================================================");
        let mut app = optics_ui::OpticsLabApp::default();
        let initial_hash = app.stable_hash();
        println!("Initial AppState stable hash: 0x{:016x}", initial_hash);
        assert!(initial_hash > 0);

        let mut platform = optics_ui::platform::MockPlatform::new();
        let effects = optics_ui::commands::dispatch(
            &mut app,
            optics_ui::commands::Command::ResetDefaults,
            &mut platform,
        );
        assert!(!effects.is_empty());
        let post_hash = app.stable_hash();
        println!("Post-dispatch stable hash:    0x{:016x}", post_hash);
        println!("[PASS] OpticsLab GUI state initialization, dispatch, and hashing verified.");
        println!("============================================================");
        return Ok(());
    }

    let icon_data =
        eframe::icon_data::from_png_bytes(include_bytes!("../../../assets/icon_256.png")).ok();
    let mut viewport = eframe::egui::ViewportBuilder::default()
        .with_title("OpticsLab")
        .with_inner_size(Vec2::new(1200.0, 800.0))
        .with_min_inner_size(Vec2::new(900.0, 600.0));

    if let Some(icon) = icon_data {
        viewport = viewport.with_icon(icon);
    }

    let native_options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        "OpticsLab",
        native_options,
        Box::new(|cc| Ok(Box::new(optics_ui::OpticsLabApp::new(cc)))),
    )
}
