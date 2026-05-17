//! Select Servo's Soliloquy V8 JavaScript backend (must run before Servo starts).

use std::sync::Once;

static V8_BACKEND: Once = Once::new();

pub const SOLILOQUY_JS_ENGINE_ENV: &str = "SOLILOQUY_JS_ENGINE";

/// Prefer V8 for Servo embedder script evaluation when `servo/soliloquy_v8` is enabled.
pub fn ensure_soliloquy_v8_selected() {
    V8_BACKEND.call_once(|| {
        if std::env::var(SOLILOQUY_JS_ENGINE_ENV).is_err() {
            // SAFETY: set once on the main thread before Servo worker threads start.
            unsafe {
                std::env::set_var(SOLILOQUY_JS_ENGINE_ENV, "v8");
            }
        }
        log::info!(
            "RV8 Servo JavaScript backend: {}",
            std::env::var(SOLILOQUY_JS_ENGINE_ENV).unwrap_or_else(|_| "v8".to_string())
        );
    });
}
