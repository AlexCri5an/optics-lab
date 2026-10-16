//! `optics-ui` — egui design tokens, dockable panels, interactive canvas,
//! and all diagnostic panel widgets.

#![deny(missing_docs)]
#![deny(unsafe_code)]

pub mod analytics;
pub mod app;
pub mod formatting;
pub mod glass_browser;
pub mod hit_test;
pub mod icons;
pub mod inspector;
pub mod layout;
pub mod legend;
pub mod lens_sim;
pub mod lens_view;
pub mod shortcuts;
pub mod sim;
pub mod theme;
pub mod view_transform;
pub mod viewport_3d;
pub mod widgets;

pub use app::{ActiveTool, AppMode, OpticsLabApp, RightPanelTab, ViewportMode};
pub use formatting::{
    format_angle_deg, format_bounce_event, format_percent, format_refractive_index,
    format_wavelength,
};
pub use glass_browser::{GlassBrowserState, render_glass_catalog_panel};
pub use hit_test::{find_hovered_ray_index, point_to_segment_distance};
pub use icons::{IconId, draw_icon, icon_button, init_icon_fonts, toolbar_button};
pub use layout::{Breakpoint, LayoutPlan, MIN_CANVAS_HEIGHT, MIN_CANVAS_WIDTH, compute_layout};
pub use legend::{draw_wavelength_legend, find_closest_ray_by_x, wavelength_to_x, x_to_wavelength};
pub use lens_sim::{
    LensElement2D, LensPreset, LensSimulation, LensSubview, TracedSequentialRay2D,
    build_lens_elements, trace_cross_section_rays,
};
pub use lens_view::{render_lens_system_panel, render_prescription_table_view};
pub use shortcuts::{ALL_SHORTCUTS, ShortcutDef, ShortcutId, format_tooltip, get_shortcut};
pub use sim::{
    EdgeState, PrismSimulation, ReflectedBranch2D, TracedRay2D, map_reflectance_to_alpha,
};
pub use theme::{
    AppPreferences, CanvasBackgroundPreference, OpticsTokens, ThemePreference, apply_tokens,
    ray_color_for_background, resolve_theme, wcag_contrast_ratio, wcag_relative_luminance,
};
pub use view_transform::ViewTransform;
pub use widgets::{abbe_diagram, dispersion_curve_plot, scrub_field};
