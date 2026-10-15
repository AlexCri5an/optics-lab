//! `optics-ui` — egui design tokens, dockable panels, interactive canvas,
//! and all diagnostic panel widgets.

#![deny(missing_docs)]
#![deny(unsafe_code)]

pub mod app;
pub mod formatting;
pub mod hit_test;
pub mod icons;
pub mod sim;
pub mod theme;
pub mod view_transform;

pub use app::{ActiveTool, OpticsLabApp};
pub use formatting::{
    format_angle_deg, format_bounce_event, format_percent, format_refractive_index,
    format_wavelength,
};
pub use hit_test::{find_hovered_ray_index, point_to_segment_distance};
pub use icons::{ToolIcon, draw_tool_icon};
pub use sim::{PrismSimulation, TracedRay2D};
pub use theme::{OpticsTokens, apply_tokens, wcag_contrast_ratio, wcag_relative_luminance};
pub use view_transform::ViewTransform;

