//! `optics-web` — WebAssembly browser entry point.
//!
//! Provides a `WebHandle` struct exposed to JavaScript via `wasm-bindgen`
//! that loads and runs `OpticsLabApp` inside an HTML `<canvas>`.

#![warn(missing_docs)]

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

/// Web handle managing the lifecycle of the eframe web application.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub struct WebHandle {
    runner: eframe::WebRunner,
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
impl WebHandle {
    /// Creates a new `WebHandle` instance.
    #[wasm_bindgen(constructor)]
    #[must_use]
    pub fn new() -> Self {
        Self {
            runner: eframe::WebRunner::new(),
        }
    }

    /// Starts the application on the specified HTML canvas element.
    #[wasm_bindgen]
    pub async fn start(&self, canvas: web_sys::HtmlCanvasElement) -> Result<(), JsValue> {
        self.runner
            .start(
                canvas,
                eframe::WebOptions::default(),
                Box::new(|cc| Ok(Box::new(optics_ui::OpticsLabApp::new(cc)))),
            )
            .await
            .map_err(|err| JsValue::from_str(&format!("{err:?}")))
    }

    /// Returns `true` if the runner has panicked.
    #[wasm_bindgen]
    #[must_use]
    pub fn has_panicked(&self) -> bool {
        self.runner.has_panicked()
    }
}
