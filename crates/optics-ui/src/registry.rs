//! Per-frame UI Registry tracking registered controls and routing events to Command dispatch.

use crate::app::OpticsLabApp;
use crate::commands::Command;
use crate::icons::{draw_icon, toolbar_button};
use crate::theme::OpticsTokens;
use egui::{Response, Sense, Ui, Vec2};

/// A registered interactive control in the current frame.
#[derive(Debug, Clone, PartialEq)]
pub struct RegisteredWidget {
    /// Associated executable command.
    pub command: Command,
    /// Location identifier (e.g. "top_bar.file_menu", "toolbar.select").
    pub location: &'static str,
    /// Whether the widget was enabled during this frame.
    pub enabled: bool,
    /// Non-empty reason string if the widget was disabled.
    pub disabled_reason: Option<&'static str>,
    /// Screen-space bounding rectangle.
    pub rect: egui::Rect,
}

/// Tracks all interactive widgets rendered during each UI frame and collects triggered commands.
#[derive(Default, Debug)]
pub struct UiRegistry {
    /// All widgets registered during the current frame.
    pub widgets: Vec<RegisteredWidget>,
    /// Commands queued for execution at the end of the frame.
    pub queued_commands: Vec<Command>,
}

impl UiRegistry {
    /// Creates a new per-frame UI registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Clears frame tracking state.
    pub fn clear(&mut self) {
        self.widgets.clear();
        self.queued_commands.clear();
    }

    /// Renders a standard menu or dialog button bound to a command.
    pub fn command_button(
        &mut self,
        ui: &mut Ui,
        cmd: Command,
        app: &OpticsLabApp,
        location: &'static str,
    ) -> Response {
        let is_macos = cfg!(target_os = "macos");
        let (available, reason) = cmd.is_available(app);
        let tooltip = cmd.tooltip(is_macos);

        let resp = ui
            .add_enabled_ui(available, |ui| {
                let label = cmd.label();
                let mut btn = ui.button(label);
                if available {
                    btn = btn.on_hover_text(&tooltip);
                } else if !reason.is_empty() {
                    btn = btn.on_hover_text(reason);
                }
                btn
            })
            .inner;

        if resp.clicked() && available {
            self.queued_commands.push(cmd.clone());
        }

        self.widgets.push(RegisteredWidget {
            command: cmd,
            location,
            enabled: available,
            disabled_reason: if available { None } else { Some(reason) },
            rect: resp.rect,
        });

        resp
    }

    /// Renders a toolbar button with an icon bound to a command.
    pub fn toolbar_command_button(
        &mut self,
        ui: &mut Ui,
        cmd: Command,
        app: &OpticsLabApp,
        is_active: bool,
        location: &'static str,
        tokens: &OpticsTokens,
    ) -> Response {
        let is_macos = cfg!(target_os = "macos");
        let (available, reason) = cmd.is_available(app);
        let tooltip = cmd.tooltip(is_macos);
        let icon = cmd.icon().unwrap_or(crate::icons::IconId::Select);

        let resp = if available {
            toolbar_button(ui, icon, tokens, is_active, &tooltip)
        } else {
            // Disabled icon button
            let (rect, resp) = ui.allocate_exact_size(Vec2::new(26.0, 24.0), Sense::click());
            draw_icon(ui.painter(), rect, icon, tokens.text_disabled, 14.0);
            if !reason.is_empty() {
                resp.on_hover_text(reason)
            } else {
                resp
            }
        };

        if resp.clicked() && available {
            self.queued_commands.push(cmd.clone());
        }

        self.widgets.push(RegisteredWidget {
            command: cmd,
            location,
            enabled: available,
            disabled_reason: if available { None } else { Some(reason) },
            rect: resp.rect,
        });

        resp
    }

    /// Renders a selectable label button bound to a command.
    pub fn selectable_command_label(
        &mut self,
        ui: &mut Ui,
        selected: bool,
        cmd: Command,
        app: &OpticsLabApp,
        location: &'static str,
    ) -> Response {
        let (available, reason) = cmd.is_available(app);
        let resp = ui
            .add_enabled_ui(available, |ui| {
                let label = cmd.label();
                let mut lbl = ui.selectable_label(selected, label);
                if !available && !reason.is_empty() {
                    lbl = lbl.on_hover_text(reason);
                }
                lbl
            })
            .inner;

        if resp.clicked() && available {
            self.queued_commands.push(cmd.clone());
        }

        self.widgets.push(RegisteredWidget {
            command: cmd,
            location,
            enabled: available,
            disabled_reason: if available { None } else { Some(reason) },
            rect: resp.rect,
        });

        resp
    }
}
