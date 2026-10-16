//! Exhaustive tests for command architecture, dispatcher, effect contracts, and state hashing (P1).

use optics_ui::commands::{Command, Effect, dispatch};
use optics_ui::platform::{MockPlatform, Platform};
use optics_ui::{ActiveTool, AppMode, OpticsLabApp, RightPanelTab, ThemePreference, ViewportMode};

#[test]
fn test_state_snapshot_hash_changes_on_state_modifications() {
    let mut app = OpticsLabApp::default();
    let initial_hash = app.stable_hash();

    // Changing tool changes hash
    app.active_tool = ActiveTool::Move;
    let tool_hash = app.stable_hash();
    assert_ne!(initial_hash, tool_hash, "Changing tool must change hash");

    // Changing mode changes hash
    app.app_mode = AppMode::LensSystem;
    let mode_hash = app.stable_hash();
    assert_ne!(tool_hash, mode_hash, "Changing mode must change hash");

    // Changing viewport mode changes hash
    app.viewport_mode = ViewportMode::Spectral;
    let vp_hash = app.stable_hash();
    assert_ne!(
        mode_hash, vp_hash,
        "Changing viewport mode must change hash"
    );

    // Changing right panel tab changes hash
    app.right_panel_tab = RightPanelTab::GlassCatalog;
    let tab_hash = app.stable_hash();
    assert_ne!(vp_hash, tab_hash, "Changing tab must change hash");

    // Changing theme changes hash
    app.prefs.theme = ThemePreference::HighContrast;
    let theme_hash = app.stable_hash();
    assert_ne!(tab_hash, theme_hash, "Changing theme must change hash");

    // Changing prism simulation parameters changes hash
    app.sim.incidence_deg = 35.0;
    let inc_hash = app.stable_hash();
    assert_ne!(
        theme_hash, inc_hash,
        "Changing prism incidence must change hash"
    );

    app.sim.apex_deg = 45.0;
    let apex_hash = app.stable_hash();
    assert_ne!(inc_hash, apex_hash, "Changing prism apex must change hash");
}

#[test]
fn test_command_dispatch_tools_and_modes() {
    let mut app = OpticsLabApp::default();
    let mut platform = MockPlatform::new();

    let effects = dispatch(
        &mut app,
        Command::SelectTool(ActiveTool::RayInspector),
        &mut platform,
    );
    assert_eq!(app.active_tool, ActiveTool::RayInspector);
    assert_eq!(effects, vec![Effect::StateChanged("active_tool")]);

    let effects = dispatch(
        &mut app,
        Command::SetMode(AppMode::LensSystem),
        &mut platform,
    );
    assert_eq!(app.app_mode, AppMode::LensSystem);
    assert_eq!(app.viewport_mode, ViewportMode::View2D);
    assert_eq!(app.right_panel_tab, RightPanelTab::Prescription);
    assert_eq!(effects, vec![Effect::StateChanged("app_mode")]);

    let effects = dispatch(
        &mut app,
        Command::SetViewportMode(ViewportMode::Convergence),
        &mut platform,
    );
    assert_eq!(app.viewport_mode, ViewportMode::Convergence);
    assert_eq!(effects, vec![Effect::StateChanged("viewport_mode")]);
}

#[test]
fn test_command_dispatch_clipboard_and_project_io() {
    let mut app = OpticsLabApp::default();
    let mut platform = MockPlatform::new();

    // Select a ray and test TSV copy
    app.selected_ray_index = Some(0);
    let effects = dispatch(&mut app, Command::CopyRayBouncesTsv, &mut platform);
    assert!(!effects.is_empty(), "Copy ray bounces must emit an effect");
    let clip = platform.get_clipboard().unwrap_or_default();
    assert!(clip.contains("Surface\tEvent\tWavelength_nm"));

    // Rays CSV export
    let effects = dispatch(&mut app, Command::ExportRaysCsv, &mut platform);
    assert!(!effects.is_empty(), "Export rays CSV must emit an effect");
    let clip_csv = platform.get_clipboard().unwrap_or_default();
    assert!(clip_csv.contains("Ray_Index,Wavelength_nm"));

    // Save project
    let effects = dispatch(&mut app, Command::SaveProject, &mut platform);
    assert_eq!(effects, vec![Effect::FileSaved("scene.json")]);
    let bytes_clone = {
        let saved_lock = platform.saved_files.lock();
        assert!(saved_lock.is_ok());
        let Ok(saved) = saved_lock else {
            return;
        };
        assert_eq!(saved.len(), 1);
        let (filename, bytes) = &saved[0];
        assert_eq!(filename, "scene.json");
        bytes.clone()
    };

    // Modify app state then load the project bytes back
    app.sim.incidence_deg = 80.0;
    assert_eq!(app.sim.incidence_deg, 80.0);
    let load_effects = dispatch(&mut app, Command::LoadProject(bytes_clone), &mut platform);
    assert_eq!(
        load_effects,
        vec![Effect::FileLoaded("project.json"), Effect::SceneModified]
    );
    assert_eq!(
        app.sim.incidence_deg, 45.0,
        "Loading project must restore incidence angle"
    );
}

#[test]
fn test_command_modal_dialogs() {
    let mut app = OpticsLabApp::default();
    let mut platform = MockPlatform::new();

    // Preferences modal
    assert!(!app.show_preferences);
    let effects = dispatch(&mut app, Command::OpenPreferences, &mut platform);
    assert!(app.show_preferences);
    assert_eq!(effects, vec![Effect::ModalOpened("preferences")]);

    let effects = dispatch(&mut app, Command::ClosePreferences, &mut platform);
    assert!(!app.show_preferences);
    assert_eq!(effects, vec![Effect::ModalClosed("preferences")]);

    // Shortcuts modal
    assert!(!app.show_shortcuts_help);
    let effects = dispatch(&mut app, Command::OpenShortcutsHelp, &mut platform);
    assert!(app.show_shortcuts_help);
    assert_eq!(effects, vec![Effect::ModalOpened("shortcuts")]);

    let effects = dispatch(&mut app, Command::CloseShortcutsHelp, &mut platform);
    assert!(!app.show_shortcuts_help);
    assert_eq!(effects, vec![Effect::ModalClosed("shortcuts")]);
}
