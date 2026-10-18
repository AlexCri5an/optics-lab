//! Responsive layout engine for OpticsLab desktop workbench.
//!
//! Provides breakpoints and layout metrics to keep the interface fluid and
//! uncluttered whether the window is small, resized, or displayed on a 4K monitor.

/// Responsive layout breakpoints based on window width in logical points.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Breakpoint {
    /// Window width < 780 px: compact mode (icon-only toolbar, tight spacing).
    Compact,
    /// Window width 780–1300 px: standard desktop mode.
    Regular,
    /// Window width > 1300 px: wide multi-panel workstation mode.
    Wide,
}

impl Breakpoint {
    /// Classifies an available width into a responsive breakpoint.
    #[must_use]
    pub const fn from_width(width: f32) -> Self {
        if width < 780.0 {
            Self::Compact
        } else if width < 1300.0 {
            Self::Regular
        } else {
            Self::Wide
        }
    }

    /// Returns whether text labels in the toolbar should be shown or hidden (icon-only).
    #[must_use]
    pub const fn show_toolbar_labels(self) -> bool {
        match self {
            Self::Compact => false,
            Self::Regular | Self::Wide => true,
        }
    }
}

/// Geometric constraints and dimensions for UI chrome.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayoutMetrics {
    /// Current window width.
    pub window_width: f32,
    /// Current window height.
    pub window_height: f32,
    /// Current responsive breakpoint.
    pub breakpoint: Breakpoint,
    /// Height of the top bar in logical points.
    pub topbar_height: f32,
    /// Height of the status bar in logical points.
    pub statusbar_height: f32,
}

impl LayoutMetrics {
    /// Calculates layout metrics for the given window size.
    #[must_use]
    pub const fn new(width: f32, height: f32) -> Self {
        let breakpoint = Breakpoint::from_width(width);
        Self {
            window_width: width,
            window_height: height,
            breakpoint,
            topbar_height: 54.0, // 26px menu bar + 28px toolbar
            statusbar_height: 22.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_responsive_breakpoints() {
        assert_eq!(Breakpoint::from_width(600.0), Breakpoint::Compact);
        assert_eq!(Breakpoint::from_width(779.0), Breakpoint::Compact);
        assert_eq!(Breakpoint::from_width(780.0), Breakpoint::Regular);
        assert_eq!(Breakpoint::from_width(1200.0), Breakpoint::Regular);
        assert_eq!(Breakpoint::from_width(1299.0), Breakpoint::Regular);
        assert_eq!(Breakpoint::from_width(1300.0), Breakpoint::Wide);
        assert_eq!(Breakpoint::from_width(2560.0), Breakpoint::Wide);
    }

    #[test]
    fn test_layout_metrics() {
        let metrics = LayoutMetrics::new(1200.0, 800.0);
        assert_eq!(metrics.breakpoint, Breakpoint::Regular);
        assert_eq!(metrics.topbar_height, 54.0);
        assert_eq!(metrics.statusbar_height, 22.0);
    }
}
