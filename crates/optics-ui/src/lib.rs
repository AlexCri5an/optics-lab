//! `optics-ui` — egui design tokens, dockable panels, interactive canvas,
//! and all diagnostic panel widgets.

#![deny(missing_docs)]
#![deny(unsafe_code)]

pub mod app;
pub mod formatting;
pub mod hit_test;
pub mod icons;
pub mod legend;
pub mod lens_sim;
pub mod lens_view;
pub mod sim;
pub mod theme;
pub mod view_transform;

pub use app::{ActiveTool, AppMode, OpticsLabApp};
pub use formatting::{
    format_angle_deg, format_bounce_event, format_percent, format_refractive_index,
    format_wavelength,
};
pub use hit_test::{find_hovered_ray_index, point_to_segment_distance};
pub use icons::{ToolIcon, draw_tool_icon};
pub use legend::{draw_wavelength_legend, find_closest_ray_by_x, wavelength_to_x, x_to_wavelength};
pub use lens_sim::{
    LensElement2D, LensPreset, LensSimulation, LensSubview, TracedSequentialRay2D,
    build_lens_elements, trace_cross_section_rays,
};
pub use lens_view::render_lens_system_panel;
pub use sim::{
    EdgeState, PrismSimulation, ReflectedBranch2D, TracedRay2D, map_reflectance_to_alpha,
};
pub use theme::{OpticsTokens, apply_tokens, wcag_contrast_ratio, wcag_relative_luminance};
pub use view_transform::ViewTransform;
