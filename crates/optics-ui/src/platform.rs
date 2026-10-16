//! Platform abstraction trait and implementations for native, web, and tests.

use egui::ViewportCommand;
use std::sync::{Arc, Mutex};

/// Platform abstraction for file IO, clipboard, time, and viewport commands.
pub trait Platform: Send + Sync {
    /// Copies text to the system clipboard.
    fn set_clipboard(&mut self, text: String);

    /// Reads text from the system clipboard.
    fn get_clipboard(&self) -> Option<String>;

    /// Presents a file open dialog (or mock in tests) returning file bytes.
    fn pick_file(&mut self, filter_ext: &[&str]) -> Option<(String, Vec<u8>)>;

    /// Presents a file save dialog (or mock in tests) to write file bytes.
    fn save_file(&mut self, default_name: &str, content: &[u8]) -> Result<(), String>;

    /// Returns high-resolution monotonic time in seconds.
    fn now_seconds(&self) -> f64;

    /// Sends an egui viewport command.
    fn send_viewport_cmd(&mut self, cmd: ViewportCommand);
}

/// Name and binary content of a file payload.
pub type FilePayload = (String, Vec<u8>);

/// Thread-safe list of file payloads.
pub type SharedFileList = Arc<Mutex<Vec<FilePayload>>>;

/// Headless/mock platform implementation for unit and integration testing.
#[derive(Default, Clone, Debug)]
pub struct MockPlatform {
    /// Stored clipboard string.
    pub clipboard: Arc<Mutex<Option<String>>>,
    /// Files prepared for `pick_file`.
    pub files_to_pick: SharedFileList,
    /// Files written via `save_file`.
    pub saved_files: SharedFileList,
    /// Viewport commands captured.
    pub viewport_commands: Arc<Mutex<Vec<ViewportCommand>>>,
    /// Mock current time in seconds.
    pub current_time: Arc<Mutex<f64>>,
}

impl MockPlatform {
    /// Creates a new mock platform.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl Platform for MockPlatform {
    fn set_clipboard(&mut self, text: String) {
        if let Ok(mut lock) = self.clipboard.lock() {
            *lock = Some(text);
        }
    }

    fn get_clipboard(&self) -> Option<String> {
        self.clipboard.lock().ok().and_then(|lock| lock.clone())
    }

    fn pick_file(&mut self, _filter_ext: &[&str]) -> Option<(String, Vec<u8>)> {
        if let Ok(mut lock) = self.files_to_pick.lock() {
            if !lock.is_empty() {
                return Some(lock.remove(0));
            }
        }
        None
    }

    fn save_file(&mut self, default_name: &str, content: &[u8]) -> Result<(), String> {
        if let Ok(mut lock) = self.saved_files.lock() {
            lock.push((default_name.to_string(), content.to_vec()));
            Ok(())
        } else {
            Err("Lock poisoned".into())
        }
    }

    fn now_seconds(&self) -> f64 {
        self.current_time.lock().map_or(0.0, |t| *t)
    }

    fn send_viewport_cmd(&mut self, cmd: ViewportCommand) {
        if let Ok(mut lock) = self.viewport_commands.lock() {
            lock.push(cmd);
        }
    }
}

/// Real platform implementation wrapping `egui::Context`.
pub struct EguiPlatform<'a> {
    /// Reference to egui Context.
    pub ctx: &'a egui::Context,
    /// Pending viewport commands.
    pub viewport_commands: Vec<ViewportCommand>,
}

impl<'a> EguiPlatform<'a> {
    /// Creates a new platform wrapping `egui::Context`.
    #[must_use]
    pub fn new(ctx: &'a egui::Context) -> Self {
        Self {
            ctx,
            viewport_commands: Vec::new(),
        }
    }
}

impl Platform for EguiPlatform<'_> {
    fn set_clipboard(&mut self, text: String) {
        self.ctx.copy_text(text);
    }

    fn get_clipboard(&self) -> Option<String> {
        // egui Context doesn't expose get_clipboard directly synchronously
        None
    }

    fn pick_file(&mut self, _filter_ext: &[&str]) -> Option<(String, Vec<u8>)> {
        // Native file picker can be called via rfd if added, otherwise drag-and-drop
        None
    }

    fn save_file(&mut self, _default_name: &str, _content: &[u8]) -> Result<(), String> {
        // Handled via rfd or download on web
        Ok(())
    }

    fn now_seconds(&self) -> f64 {
        self.ctx.input(|i| i.time)
    }

    fn send_viewport_cmd(&mut self, cmd: ViewportCommand) {
        self.ctx.send_viewport_cmd(cmd);
    }
}
