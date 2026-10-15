//! `optics-web` — WebAssembly browser entry point.
//!
//! `eframe` exposes a `start` function automatically when compiled to
//! `wasm32-unknown-unknown`. This module will call
//! `eframe::WebRunner::new().start(...)` in Phase 12.

/// WASM entry point — placeholder until Phase 12.
#[cfg(target_arch = "wasm32")]
#[allow(dead_code)]
pub fn start() {
    // Phase 12: eframe WebRunner initialisation goes here.
}
