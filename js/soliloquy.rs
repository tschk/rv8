//! Select Servo's Soliloquy V8 JavaScript backend (must run before Servo starts).

use std::sync::Once;

static V8_BACKEND: Once = Once::new();

pub const SOLILOQUY_JS_ENGINE_ENV: &str = "SOLILOQUY_JS_ENGINE";

/// Prefer V8 for Servo embedder script evaluation when `servo/soliloquy_v8` is enabled.
fn apply_default_v8_engine_env_if_unset() {
    if std::env::var(SOLILOQUY_JS_ENGINE_ENV).is_err() {
        // SAFETY: set once on the main thread before Servo worker threads start.
        unsafe {
            std::env::set_var(SOLILOQUY_JS_ENGINE_ENV, "v8");
        }
    }
}

pub fn ensure_soliloquy_v8_selected() {
    V8_BACKEND.call_once(|| {
        apply_default_v8_engine_env_if_unset();
        log::info!(
            "RV8 Servo JavaScript backend: {}",
            std::env::var(SOLILOQUY_JS_ENGINE_ENV).unwrap_or_else(|_| "v8".to_string())
        );
    });
}

#[cfg(test)]
mod tests {
    use super::{SOLILOQUY_JS_ENGINE_ENV, apply_default_v8_engine_env_if_unset};
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn set_env(value: &str) {
        // SAFETY: tests hold ENV_LOCK while mutating process env.
        unsafe {
            std::env::set_var(SOLILOQUY_JS_ENGINE_ENV, value);
        }
    }

    fn clear_env() {
        // SAFETY: tests hold ENV_LOCK while mutating process env.
        unsafe {
            std::env::remove_var(SOLILOQUY_JS_ENGINE_ENV);
        }
    }

    #[test]
    fn default_engine_env_selects_v8_when_unset() {
        let _guard = ENV_LOCK.lock().unwrap();
        clear_env();
        apply_default_v8_engine_env_if_unset();
        assert_eq!(
            std::env::var(SOLILOQUY_JS_ENGINE_ENV).expect("env after default"),
            "v8"
        );
    }

    #[test]
    fn default_engine_env_preserves_override() {
        let _guard = ENV_LOCK.lock().unwrap();
        set_env("v8-experimental");
        apply_default_v8_engine_env_if_unset();
        assert_eq!(
            std::env::var(SOLILOQUY_JS_ENGINE_ENV).expect("env after default"),
            "v8-experimental"
        );
    }
}