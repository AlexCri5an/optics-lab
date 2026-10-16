//! Single source of truth shortcut registry for OpticsLab.
//!
//! Provides platform-aware hotkey mappings, labels, and conflict detection.

use egui::{Key, Modifiers};

/// Typed identifier for application keyboard shortcuts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShortcutId {
    /// Select tool (S).
    SelectTool,
    /// Move / translation tool (M).
    MoveTool,
    /// Rotate tool (Alt+R).
    RotateTool,
    /// Ray inspector probe tool (I).
    RayProbe,
    /// Zoom to fit scene (Cmd/Ctrl + 0).
    FitView,
    /// Zoom in (Cmd/Ctrl + Plus/Equals).
    ZoomIn,
    /// Zoom out (Cmd/Ctrl + Minus).
    ZoomOut,
    /// Toggle Fresnel reflections.
    ToggleReflections,
    /// Toggle full screen mode (F11).
    ToggleFullscreen,
    /// Application preferences dialog (Cmd/Ctrl + Comma).
    Preferences,
    /// Keyboard shortcuts help cheat sheet (F1).
    Help,
    /// Escape action (closes dialogs, clears selection, exits fullscreen).
    Escape,
}

/// Shortcut specification defining trigger conditions and human-readable text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShortcutDef {
    /// Identifying ID.
    pub id: ShortcutId,
    /// Primary key.
    pub key: Key,
    /// Required modifiers.
    pub modifiers: Modifiers,
    /// Description of the action.
    pub description: &'static str,
    /// Plain text shortcut on macOS (e.g. "Cmd+0").
    pub macos_text: &'static str,
    /// Plain text shortcut on Linux / Windows (e.g. "Ctrl+0").
    pub standard_text: &'static str,
}

impl ShortcutDef {
    /// Returns the platform-appropriate display label (plain text, no symbols).
    #[must_use]
    pub const fn label(&self, is_macos: bool) -> &'static str {
        if is_macos {
            self.macos_text
        } else {
            self.standard_text
        }
    }
}

/// The complete, canonical list of shortcuts in OpticsLab.
pub const ALL_SHORTCUTS: &[ShortcutDef] = &[
    ShortcutDef {
        id: ShortcutId::SelectTool,
        key: Key::S,
        modifiers: Modifiers::NONE,
        description: "Select tool (ray & surface selection)",
        macos_text: "S",
        standard_text: "S",
    },
    ShortcutDef {
        id: ShortcutId::MoveTool,
        key: Key::M,
        modifiers: Modifiers::NONE,
        description: "Move / translate tool",
        macos_text: "M",
        standard_text: "M",
    },
    ShortcutDef {
        id: ShortcutId::RotateTool,
        key: Key::R,
        modifiers: Modifiers::ALT,
        description: "Rotate tool",
        macos_text: "Alt+R",
        standard_text: "Alt+R",
    },
    ShortcutDef {
        id: ShortcutId::RayProbe,
        key: Key::I,
        modifiers: Modifiers::NONE,
        description: "Ray probe inspector",
        macos_text: "I",
        standard_text: "I",
    },
    ShortcutDef {
        id: ShortcutId::FitView,
        key: Key::Num0,
        modifiers: Modifiers::COMMAND,
        description: "Reset view / fit scene",
        macos_text: "Cmd+0",
        standard_text: "Ctrl+0",
    },
    ShortcutDef {
        id: ShortcutId::ZoomIn,
        key: Key::Plus,
        modifiers: Modifiers::COMMAND,
        description: "Increase UI zoom scale",
        macos_text: "Cmd++",
        standard_text: "Ctrl++",
    },
    ShortcutDef {
        id: ShortcutId::ZoomOut,
        key: Key::Minus,
        modifiers: Modifiers::COMMAND,
        description: "Decrease UI zoom scale",
        macos_text: "Cmd+-",
        standard_text: "Ctrl+-",
    },
    ShortcutDef {
        id: ShortcutId::ToggleReflections,
        key: Key::B,
        modifiers: Modifiers::NONE,
        description: "Toggle Fresnel reflections",
        macos_text: "B",
        standard_text: "B",
    },
    ShortcutDef {
        id: ShortcutId::ToggleFullscreen,
        key: Key::F11,
        modifiers: Modifiers::NONE,
        description: "Toggle full screen",
        macos_text: "F11",
        standard_text: "F11",
    },
    ShortcutDef {
        id: ShortcutId::Preferences,
        key: Key::Comma,
        modifiers: Modifiers::COMMAND,
        description: "Open preferences dialog",
        macos_text: "Cmd+,",
        standard_text: "Ctrl+,",
    },
    ShortcutDef {
        id: ShortcutId::Help,
        key: Key::F1,
        modifiers: Modifiers::NONE,
        description: "Keyboard shortcuts reference",
        macos_text: "F1",
        standard_text: "F1",
    },
    ShortcutDef {
        id: ShortcutId::Escape,
        key: Key::Escape,
        modifiers: Modifiers::NONE,
        description: "Close dialog / clear selection",
        macos_text: "Esc",
        standard_text: "Esc",
    },
];

/// Looks up a shortcut definition by ID.
#[must_use]
pub fn get_shortcut(id: ShortcutId) -> &'static ShortcutDef {
    for s in ALL_SHORTCUTS {
        if s.id == id {
            return s;
        }
    }
    unreachable!("Every ShortcutId must exist in ALL_SHORTCUTS")
}

/// Generates a button tooltip containing the action name and shortcut.
///
/// Example: `format_tooltip("Ray probe", ShortcutId::RayProbe, true)` -> `"Ray probe (I)"`.
#[must_use]
pub fn format_tooltip(action: &str, id: ShortcutId, is_macos: bool) -> String {
    let s = get_shortcut(id);
    format!("{action} ({})", s.label(is_macos))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_shortcuts_have_unique_bindings() {
        let mut seen = std::collections::HashSet::new();
        for s in ALL_SHORTCUTS {
            let key = (
                s.key,
                s.modifiers.alt,
                s.modifiers.ctrl,
                s.modifiers.command,
            );
            assert!(
                seen.insert(key),
                "Duplicate keyboard shortcut detected for key {:?} with modifiers {:?}",
                s.key,
                s.modifiers
            );
        }
    }

    #[test]
    fn test_plain_text_labels() {
        for s in ALL_SHORTCUTS {
            assert!(
                !s.macos_text.contains('⌘'),
                "Use plain text 'Cmd', not glyphs"
            );
            assert!(
                !s.macos_text.contains('⌥'),
                "Use plain text 'Alt', not glyphs"
            );
            assert!(
                !s.macos_text.contains('⇧'),
                "Use plain text 'Shift', not glyphs"
            );
            assert!(
                !s.standard_text.contains('⌃'),
                "Use plain text 'Ctrl', not glyphs"
            );
        }
    }

    #[test]
    fn test_format_tooltip() {
        let tt = format_tooltip("Ray probe", ShortcutId::RayProbe, true);
        assert_eq!(tt, "Ray probe (I)");

        let fit = format_tooltip("Reset view", ShortcutId::FitView, true);
        assert_eq!(fit, "Reset view (Cmd+0)");

        let fit_win = format_tooltip("Reset view", ShortcutId::FitView, false);
        assert_eq!(fit_win, "Reset view (Ctrl+0)");
    }
}
