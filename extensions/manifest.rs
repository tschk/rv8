//! Extension manifest parsing (MV2 / MV3 / WebKit-compatible).
//!
//! Supports the common subset used by Chrome, Firefox, and Safari web
//! extensions. Safari-specific `Info.plist` wrappers are not parsed here;
//! the adapter expects a `manifest.json` produced by `xcrun` or by hand.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// A WebExtensions manifest as consumed by the adapter.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ExtensionManifest {
    pub manifest_version: u32,
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub permissions: Option<Vec<String>>,
    #[serde(default, rename = "optional_permissions")]
    pub optional_permissions: Option<Vec<String>>,
    #[serde(default, rename = "host_permissions")]
    pub host_permissions: Option<Vec<String>>,
    #[serde(default)]
    pub background: Option<Background>,
    #[serde(default, rename = "content_scripts")]
    pub content_scripts: Option<Vec<ContentScript>>,
    #[serde(default)]
    pub action: Option<Action>,
    #[serde(default, rename = "browser_action")]
    pub browser_action: Option<Action>,
    #[serde(default, rename = "page_action")]
    pub page_action: Option<Action>,
    #[serde(default, rename = "sidebar_action")]
    pub sidebar_action: Option<Action>,
    #[serde(default)]
    pub icons: Option<HashMap<String, String>>,
    #[serde(default, rename = "options_page")]
    pub options_page: Option<String>,
    #[serde(default, rename = "options_ui")]
    pub options_ui: Option<OptionsUi>,
    #[serde(default, rename = "content_security_policy")]
    pub content_security_policy: Option<serde_json::Value>,
    #[serde(default, rename = "web_accessible_resources")]
    pub web_accessible_resources: Option<Vec<WebAccessibleResource>>,
    #[serde(default, rename = "default_locale")]
    pub default_locale: Option<String>,
    #[serde(default)]
    pub commands: Option<HashMap<String, Command>>,
    #[serde(default, rename = "externally_connectable")]
    pub externally_connectable: Option<ExternallyConnectable>,
}

impl ExtensionManifest {
    /// Parse `manifest.json` from an unpacked extension directory.
    pub fn from_dir(dir: &Path) -> Result<Self, String> {
        let path = dir.join("manifest.json");
        let text = std::fs::read_to_string(&path)
            .map_err(|e| format!("Failed to read manifest at {:?}: {}", path, e))?;
        Self::from_json(&text)
    }

    pub fn from_json(text: &str) -> Result<Self, String> {
        serde_json::from_str(text).map_err(|e| format!("Failed to parse manifest: {}", e))
    }

    /// True if this is a Manifest V3 extension.
    pub fn is_mv3(&self) -> bool {
        self.manifest_version >= 3
    }

    /// Effective browser action / action descriptor.
    /// MV3 uses `action`, MV2 uses `browser_action`.
    pub fn action(&self) -> Option<&Action> {
        self.action.as_ref().or(self.browser_action.as_ref())
    }

    /// All host-like permissions declared by the extension.
    pub fn declared_hosts(&self) -> impl Iterator<Item = &String> {
        let perms = self.permissions.iter().flat_map(|v| v.iter());
        let hosts = self.host_permissions.iter().flat_map(|v| v.iter());
        perms.chain(hosts)
    }

    /// All API permissions declared by the extension (non-host strings).
    pub fn declared_permissions(&self) -> impl Iterator<Item = &String> + '_ {
        self.permissions.iter().flat_map(|v| v.iter()).filter(|p| {
            // Host permissions begin with a scheme or contain a wildcard.
            !p.contains("://") && !p.contains("*")
        })
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Background {
    Mv2 {
        scripts: Vec<String>,
        #[serde(default)]
        persistent: Option<bool>,
    },
    Mv3 {
        #[serde(rename = "service_worker")]
        service_worker: String,
        #[serde(rename = "type")]
        type_: Option<String>,
    },
}

impl Background {
    pub fn scripts(&self) -> Vec<String> {
        match self {
            Background::Mv2 { scripts, .. } => scripts.clone(),
            Background::Mv3 { service_worker, .. } => vec![service_worker.clone()],
        }
    }

    pub fn is_persistent(&self) -> bool {
        match self {
            Background::Mv2 { persistent, .. } => persistent.unwrap_or(true),
            Background::Mv3 { .. } => false,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ContentScript {
    pub matches: Vec<String>,
    #[serde(default)]
    pub js: Option<Vec<String>>,
    #[serde(default)]
    pub css: Option<Vec<String>>,
    #[serde(default, rename = "run_at")]
    pub run_at: Option<String>,
    #[serde(default, rename = "all_frames")]
    pub all_frames: Option<bool>,
    #[serde(default, rename = "match_about_blank")]
    pub match_about_blank: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Action {
    #[serde(default, rename = "default_icon")]
    pub default_icon: Option<serde_json::Value>,
    #[serde(default, rename = "default_title")]
    pub default_title: Option<String>,
    #[serde(default, rename = "default_popup")]
    pub default_popup: Option<String>,
    #[serde(default)]
    pub enabled: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OptionsUi {
    pub page: Option<String>,
    #[serde(default, rename = "browser_style")]
    pub browser_style: Option<bool>,
    #[serde(default, rename = "open_in_tab")]
    pub open_in_tab: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WebAccessibleResource {
    pub resources: Vec<String>,
    #[serde(default)]
    pub matches: Option<Vec<String>>,
    #[serde(default, rename = "extension_ids")]
    pub extension_ids: Option<Vec<String>>,
    #[serde(default, rename = "use_dynamic_url")]
    pub use_dynamic_url: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Command {
    #[serde(default, rename = "suggested_key")]
    pub suggested_key: Option<HashMap<String, String>>,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ExternallyConnectable {
    #[serde(default)]
    pub ids: Option<Vec<String>>,
    #[serde(default)]
    pub matches: Option<Vec<String>>,
    #[serde(default, rename = "accepts_tls_channel_id")]
    pub accepts_tls_channel_id: Option<bool>,
}

#[cfg(test)]
mod tests {
    use super::*;

    const MV2: &str = r#"{
        "manifest_version": 2,
        "name": "Test",
        "version": "1.0",
        "permissions": ["tabs", "storage", "https://*.example.com/*"],
        "background": {"scripts": ["bg.js"], "persistent": false},
        "browser_action": {"default_title": "Click"}
    }"#;

    const MV3: &str = r#"{
        "manifest_version": 3,
        "name": "Test",
        "version": "1.0",
        "permissions": ["storage"],
        "host_permissions": ["https://*.example.com/*"],
        "background": {"service_worker": "sw.js"},
        "action": {"default_popup": "popup.html"}
    }"#;

    #[test]
    fn parse_mv2_manifest() {
        let m = ExtensionManifest::from_json(MV2).unwrap();
        assert!(!m.is_mv3());
        assert_eq!(m.action().unwrap().default_title.as_deref(), Some("Click"));
        let bg = m.background.unwrap();
        assert_eq!(bg.scripts(), vec!["bg.js"]);
        assert!(!bg.is_persistent());
    }

    #[test]
    fn parse_mv3_manifest() {
        let m = ExtensionManifest::from_json(MV3).unwrap();
        assert!(m.is_mv3());
        assert_eq!(m.action().unwrap().default_popup.as_deref(), Some("popup.html"));
    }
}
