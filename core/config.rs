//! Browser configuration

use std::env;
use std::path::PathBuf;

/// Writable browser data directories for an immutable appliance layout.
///
/// The host stays read-only while browser state is confined to a narrow set
/// of explicit directories.
#[derive(Debug, Clone)]
pub struct BrowserDataDirs {
    pub profile_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub downloads_dir: PathBuf,
    pub state_dir: PathBuf,
    pub logs_dir: PathBuf,
    pub terminal_state_dir: PathBuf,
}

impl BrowserDataDirs {
    fn env_dir(key: &str, fallback: &str) -> PathBuf {
        env::var_os(key)
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(fallback))
    }

    /// Default writable locations for the browser appliance.
    pub fn appliance() -> Self {
        Self {
            profile_dir: Self::env_dir(
                "SOLILOQUY_PROFILE_DIR",
                "/var/lib/soliloquy/browser/profiles/default",
            ),
            cache_dir: Self::env_dir("SOLILOQUY_CACHE_DIR", "/var/lib/soliloquy/browser/cache"),
            downloads_dir: Self::env_dir(
                "SOLILOQUY_DOWNLOADS_DIR",
                "/var/lib/soliloquy/browser/downloads",
            ),
            state_dir: Self::env_dir("SOLILOQUY_STATE_DIR", "/var/lib/soliloquy/browser/state"),
            logs_dir: Self::env_dir("SOLILOQUY_LOG_DIR", "/var/lib/soliloquy/browser/logs"),
            terminal_state_dir: Self::env_dir(
                "SOLILOQUY_TERMINAL_STATE_DIR",
                "/var/lib/soliloquy/browser/terminal",
            ),
        }
    }
}

impl Default for BrowserDataDirs {
    fn default() -> Self {
        Self::appliance()
    }
}

/// Browser configuration options
#[derive(Debug, Clone)]
pub struct BrowserConfig {
    /// Enable multi-process mode (Chrome-like)
    pub multi_process: bool,

    /// User data directory for profiles, cache, etc.
    pub user_data_dir: PathBuf,

    /// Explicit writable browser data directories for immutable deployments.
    pub data_dirs: BrowserDataDirs,

    /// Enable sandboxing for renderer processes
    pub sandbox: bool,

    /// User agent override
    pub user_agent_override: Option<String>,

    /// Incognito mode (no persistent storage)
    pub incognito: bool,
}

impl Default for BrowserConfig {
    fn default() -> Self {
        let data_dirs = BrowserDataDirs::default();
        BrowserConfig {
            multi_process: true,
            user_data_dir: data_dirs.profile_dir.clone(),
            data_dirs,
            sandbox: true,
            user_agent_override: None,
            incognito: false,
        }
    }
}

impl BrowserConfig {
    /// Create settings optimized for the browser-only appliance.
    pub fn appliance() -> Self {
        let data_dirs = BrowserDataDirs::appliance();
        BrowserConfig {
            multi_process: true,
            user_data_dir: data_dirs.profile_dir.clone(),
            data_dirs,
            sandbox: true,
            user_agent_override: None,
            incognito: false,
        }
    }

    /// Create an incognito configuration
    pub fn incognito() -> Self {
        let data_dirs = BrowserDataDirs::default();
        BrowserConfig {
            incognito: true,
            user_data_dir: data_dirs.profile_dir.clone(),
            data_dirs,
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_env_dir_fallback() {
        env::remove_var("SOLILOQUY_TEST_NON_EXISTENT_VAR");
        let fallback = "/var/lib/soliloquy/browser/test_fallback";
        let path = BrowserDataDirs::env_dir("SOLILOQUY_TEST_NON_EXISTENT_VAR", fallback);
        assert_eq!(path.to_str().unwrap(), fallback);
    }

    #[test]
    fn test_env_dir_override() {
        let var_name = "SOLILOQUY_TEST_EXISTENT_VAR";
        let fallback = "/var/lib/soliloquy/browser/test_fallback";
        let override_val = "/tmp/soliloquy/test_override";

        env::set_var(var_name, override_val);
        let path = BrowserDataDirs::env_dir(var_name, fallback);
        assert_eq!(path.to_str().unwrap(), override_val);

        env::remove_var(var_name);
    }

    #[test]
    fn test_appliance_data_dirs() {
        env::remove_var("SOLILOQUY_PROFILE_DIR");
        let dirs = BrowserDataDirs::appliance();
        assert_eq!(
            dirs.profile_dir.to_str().unwrap(),
            "/var/lib/soliloquy/browser/profiles/default"
        );
    }

    #[test]
    fn test_config_default() {
        let config = BrowserConfig::default();
        assert!(config.multi_process);
        assert!(config.sandbox);
        assert!(!config.incognito);
        assert!(config.user_agent_override.is_none());
        assert_eq!(
            config.user_data_dir.to_str().unwrap(),
            "/var/lib/soliloquy/browser/profiles/default"
        );
    }

    #[test]
    fn test_config_appliance() {
        let config = BrowserConfig::appliance();
        assert!(config.multi_process);
        assert!(config.sandbox);
        assert!(!config.incognito);
        assert_eq!(config.user_data_dir, config.data_dirs.profile_dir);
    }

    #[test]
    fn test_config_incognito() {
        let config = BrowserConfig::incognito();
        assert!(config.incognito);
        assert!(config.sandbox);
        assert_eq!(config.user_data_dir, config.data_dirs.profile_dir);
    }
}
