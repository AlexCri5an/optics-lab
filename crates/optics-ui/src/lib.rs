//! `optics-ui` — egui design tokens, dockable panels, interactive canvas,
//! and all diagnostic panel widgets.

#![deny(missing_docs)]
#![deny(unsafe_code)]

pub mod theme;

pub use theme::{apply_tokens, wcag_contrast_ratio, wcag_relative_luminance, OpticsTokens};
