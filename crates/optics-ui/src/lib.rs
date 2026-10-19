//! OpticsLab UI — academic minimalist design tokens, responsive layout, and Lucide icons.

pub mod analysis;
pub mod app;
pub mod glass_browser;
pub mod icons;
pub mod layout;
pub mod lens_editor;
pub mod optimization;
pub mod theme;
pub mod topbar;
pub mod viewport;

pub use analysis::{AnalysisPanelState, AnalysisTab, render_analysis_panel};
pub use app::OpticsWorkbenchApp;
pub use glass_browser::{
    MaterialVendorFilter, render_materials_panel,
};
pub use icons::{
    LucideIcon, lucide_small_icon_button, lucide_text_button, lucide_toolbar_button,
    render_lucide_icon,
};
pub use layout::{Breakpoint, LayoutMetrics};
pub use lens_editor::{LeftPanelTab, LensEditorState, render_lens_editor};
pub use optimization::{DlsRunSummary, OptimizationModalState, render_optimization_panel};
pub use theme::{OpticsTokens, ThemeMode};
pub use topbar::{TopBarAction, TopBarState, render_topbar};
pub use viewport::{ViewportState, render_optical_viewport};
