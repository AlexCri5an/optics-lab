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

#[test]
fn test_command_scene_model_crud() {
    let mut app = OpticsLabApp::default();
    let mut platform = MockPlatform::new();

    let initial_count = app.scene_model.objects.len();

    // 1. Add Sphere
    let effects = dispatch(
        &mut app,
        Command::AddSceneObject("Sphere".into()),
        &mut platform,
    );
    assert_eq!(app.scene_model.objects.len(), initial_count + 1);
    assert_eq!(app.right_panel_tab, RightPanelTab::Properties);
    assert!(effects.contains(&Effect::SceneModified));
    let added_id = app.scene_model.selected_id.unwrap_or(0);
    assert_ne!(added_id, 0);

    // 2. Duplicate Sphere
    let effects = dispatch(
        &mut app,
        Command::DuplicateSceneObject(added_id),
        &mut platform,
    );
    assert_eq!(app.scene_model.objects.len(), initial_count + 2);
    assert!(effects.contains(&Effect::SceneModified));
    let dup_id = app.scene_model.selected_id.unwrap_or(0);
    assert_ne!(dup_id, 0);
    assert_ne!(added_id, dup_id);

    // 3. Toggle Visibility
    assert_eq!(
        app.scene_model.get_object(dup_id).map(|o| o.visible),
        Some(true)
    );
    dispatch(
        &mut app,
        Command::ToggleObjectVisibility(dup_id),
        &mut platform,
    );
    assert_eq!(
        app.scene_model.get_object(dup_id).map(|o| o.visible),
        Some(false)
    );

    // 4. Delete Objects
    dispatch(&mut app, Command::DeleteSceneObject(dup_id), &mut platform);
    dispatch(
        &mut app,
        Command::DeleteSceneObject(added_id),
        &mut platform,
    );
    assert_eq!(app.scene_model.objects.len(), initial_count);

    // 5. Select object by ID
    let first_id = app.scene_model.objects[0].id;
    dispatch(
        &mut app,
        Command::SelectSceneObjectId(Some(first_id)),
        &mut platform,
    );
    assert_eq!(app.scene_model.selected_id, Some(first_id));
    assert_eq!(app.right_panel_tab, RightPanelTab::Properties);
}

#[test]
fn test_command_dispatch_beam_display_and_caustic() {
    let mut app = OpticsLabApp::default();
    let mut platform = MockPlatform::new();

    assert!(app.gpu_state.beam_display.show_ribbons);
    dispatch(&mut app, Command::ToggleBeamRibbons, &mut platform);
    assert!(!app.gpu_state.beam_display.show_ribbons);

    assert!(app.gpu_state.beam_display.show_caustic);
    dispatch(&mut app, Command::ToggleFloorCaustic, &mut platform);
    assert!(!app.gpu_state.beam_display.show_caustic);

    assert!(!app.gpu_state.beam_display.show_tof_pulses);
    dispatch(&mut app, Command::ToggleTofPulses, &mut platform);
    assert!(app.gpu_state.beam_display.show_tof_pulses);

    dispatch(&mut app, Command::SetBeamWidth(4.5), &mut platform);
    assert!((app.gpu_state.beam_display.beam_width_mm - 4.5).abs() < 1e-4);

    dispatch(&mut app, Command::SetTofSpeed(2.5), &mut platform);
    assert!((app.gpu_state.beam_display.tof_speed - 2.5).abs() < 1e-4);
}

#[test]
fn test_registry_completeness() {
    let app = OpticsLabApp::default();
    let commands = all_test_commands();

    assert!(
        commands.len() >= 50,
        "Command registry must have at least 50 testable variants"
    );

    for cmd in &commands {
        let id = cmd.id();
        assert!(!id.is_empty(), "Command {cmd:?} has empty id");

        let label = cmd.label();
        assert!(!label.is_empty(), "Command {cmd:?} has empty label");

        let tooltip_win = cmd.tooltip(false);
        assert!(
            !tooltip_win.is_empty(),
            "Command {cmd:?} has empty tooltip (windows)"
        );

        let tooltip_mac = cmd.tooltip(true);
        assert!(
            !tooltip_mac.is_empty(),
            "Command {cmd:?} has empty tooltip (mac)"
        );

        let (_available, _reason) = cmd.is_available(&app);
    }
}

#[test]
fn test_click_everything_interaction_and_hash_change() {
    let mut app = OpticsLabApp::default();
    let mut platform = MockPlatform::new();

    let initial_hash = app.stable_hash();
    assert!(initial_hash > 0);
    let mut non_zero_effects_count = 0;

    for cmd in all_test_commands() {
        let effects = dispatch(&mut app, cmd.clone(), &mut platform);
        if !effects.is_empty() {
            non_zero_effects_count += 1;
        }

        let new_hash = app.stable_hash();
        assert!(new_hash > 0);
    }

    assert!(
        non_zero_effects_count >= 30,
        "Exhaustive dispatch must trigger state effects across majority of commands (got {non_zero_effects_count})"
    );
}

fn all_test_commands() -> Vec<Command> {
    vec![
        Command::SelectTool(ActiveTool::Select),
        Command::SelectTool(ActiveTool::Move),
        Command::SelectTool(ActiveTool::Rotate),
        Command::SelectTool(ActiveTool::Scale),
        Command::SelectTool(ActiveTool::RayInspector),
        Command::SelectTool(ActiveTool::Spectrum),
        Command::SelectTool(ActiveTool::Bvh),
        Command::SelectTool(ActiveTool::Analysis),
        Command::SelectTool(ActiveTool::Camera),
        Command::SelectTool(ActiveTool::Settings),
        Command::SetMode(AppMode::PrismDispersion),
        Command::SetMode(AppMode::LensSystem),
        Command::SetViewportMode(ViewportMode::View3D),
        Command::SetViewportMode(ViewportMode::View2D),
        Command::SetViewportMode(ViewportMode::Spectral),
        Command::SetViewportMode(ViewportMode::Bvh),
        Command::SetViewportMode(ViewportMode::Convergence),
        Command::SetRightPanelTab(RightPanelTab::Properties),
        Command::SetRightPanelTab(RightPanelTab::RayInspector),
        Command::SetRightPanelTab(RightPanelTab::Prescription),
        Command::SetRightPanelTab(RightPanelTab::GlassCatalog),
        Command::LoadLensPreset(optics_ui::lens_sim::LensPreset::CookeTriplet),
        Command::LoadLensPreset(optics_ui::lens_sim::LensPreset::DoubleGauss),
        Command::LoadLensPreset(optics_ui::lens_sim::LensPreset::PlanoConvex),
        Command::ResetDefaults,
        Command::ResetView,
        Command::ZoomIn,
        Command::ZoomOut,
        Command::ToggleReflections,
        Command::ToggleTiming,
        Command::ToggleSoftGlow,
        Command::ToggleLeftSidebar,
        Command::ToggleRightInspector,
        Command::ToggleBottomDock,
        Command::ResetLayout,
        Command::ToggleFullscreen,
        Command::CycleTheme,
        Command::SetTheme(ThemePreference::ClassicDark),
        Command::SetTheme(ThemePreference::ClassicLight),
        Command::SetTheme(ThemePreference::HighContrast),
        Command::OpenPreferences,
        Command::ClosePreferences,
        Command::OpenAbout,
        Command::CloseAbout,
        Command::OpenShortcutsHelp,
        Command::CloseShortcutsHelp,
        Command::SelectRay(Some(0)),
        Command::SelectRay(None),
        Command::SelectSceneObject(Some("Prism".into())),
        Command::SelectSceneObject(None),
        Command::SetPrismIncidence(35.0),
        Command::SetPrismApex(50.0),
        Command::SetPrismSampleCount(16),
        Command::SetInspectedWavelength(550.0),
        Command::SetGlass("SF11".into()),
        Command::CopyRayBouncesTsv,
        Command::ExportRaysCsv,
        Command::ExportImage,
        Command::SaveProject,
        Command::DismissModalOrSelection,
        Command::ToggleRenderPause,
        Command::ResetAccumulation,
        Command::AddSceneObject("Sphere".into()),
        Command::ToggleBeamRibbons,
        Command::ToggleFloorCaustic,
        Command::ToggleTofPulses,
        Command::SetBeamWidth(2.0),
        Command::SetTofSpeed(1.2),
        Command::Quit,
    ]
}
