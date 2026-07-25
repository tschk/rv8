//! Extension runtime: loading, lifecycle, tab-state mirrors, and API dispatch.
//!
//! `ExtensionRuntime` is intended to live in the browser process. It keeps a
//! snapshot of tab state so that extension API calls (tabs, windows, etc.) can
//! be answered synchronously by the adapter without holding `Browser` async
//! locks.

use log::{info, warn};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::atomic::{AtomicU64, Ordering};

use super::manifest::ExtensionManifest;
use super::permissions::{Permission, PermissionSet};
use super::storage::ExtensionStorage;
use super::api::{ApiRequest, ApiResponse};

/// Stable identifier for an installed extension.
/// For unpacked loads this is the directory name; store extensions would use
/// the `applications.gecko.id` or computed key.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ExtensionId(pub String);

/// A mirror of a browser tab used by the extension API adapter.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExtensionTab {
    pub id: u64,
    pub url: String,
    pub title: String,
    pub active: bool,
    pub pinned: bool,
    pub highlighted: bool,
    pub incognito: bool,
    pub window_id: u64,
    pub index: u32,
    pub status: Option<String>,
    pub favicon_url: Option<String>,
}

/// An installed, parsed extension.
#[derive(Debug)]
pub struct LoadedExtension {
    pub id: ExtensionId,
    pub manifest: ExtensionManifest,
    pub root: PathBuf,
    pub enabled: bool,
    pub permissions: PermissionSet,
}

impl LoadedExtension {
    fn new(id: ExtensionId, root: PathBuf, manifest: ExtensionManifest) -> Self {
        let mut permissions = PermissionSet::new();
        if let Some(perms) = &manifest.permissions {
            for p in perms {
                if let Ok(perm) = Permission::from_str(p) {
                    permissions.insert(perm);
                }
            }
        }
        if let Some(hosts) = &manifest.host_permissions {
            for h in hosts {
                permissions.insert(Permission::Host(h.clone()));
            }
        }
        Self {
            id,
            manifest,
            root,
            enabled: true,
            permissions,
        }
    }
}

/// In-memory alarm entry.
#[derive(Debug, Clone)]
pub struct Alarm {
    pub name: String,
    pub scheduled_at: std::time::Instant,
    pub period_in_minutes: Option<f64>,
    pub extension_id: ExtensionId,
}

/// Central extension manager.
pub struct ExtensionRuntime {
    extensions_dir: PathBuf,
    extensions: Mutex<HashMap<ExtensionId, LoadedExtension>>,
    tab_state: Mutex<Vec<ExtensionTab>>,
    storage: ExtensionStorage,
    alarms: Mutex<HashMap<String, Alarm>>,
    next_tab_id: AtomicU64,
    next_download_id: AtomicU64,
    next_menu_id: AtomicU64,
    downloads: Mutex<HashMap<u64, serde_json::Value>>,
    context_menu_items: Mutex<HashMap<u64, ContextMenuItem>>,
    notification_items: Mutex<HashMap<String, serde_json::Value>>,
    dnr_rulesets: Mutex<HashMap<String, bool>>,
    dnr_dynamic_rules: Mutex<HashMap<i64, serde_json::Value>>,
    dnr_session_rules: Mutex<HashMap<i64, serde_json::Value>>,
    action_state: Mutex<HashMap<ExtensionId, serde_json::Value>>,
}

#[derive(Debug, Clone)]
pub struct ContextMenuItem {
    pub id: u64,
    pub extension_id: ExtensionId,
    pub info: serde_json::Value,
}

impl ExtensionRuntime {
    pub fn new(extensions_dir: PathBuf) -> Self {
        std::fs::create_dir_all(&extensions_dir).ok();
        Self {
            extensions_dir,
            extensions: Mutex::new(HashMap::new()),
            tab_state: Mutex::new(Vec::new()),
            storage: ExtensionStorage::new(),
            alarms: Mutex::new(HashMap::new()),
            next_tab_id: AtomicU64::new(100_000),
            next_download_id: AtomicU64::new(1),
            next_menu_id: AtomicU64::new(1),
            downloads: Mutex::new(HashMap::new()),
            context_menu_items: Mutex::new(HashMap::new()),
            notification_items: Mutex::new(HashMap::new()),
            dnr_rulesets: Mutex::new(HashMap::new()),
            dnr_dynamic_rules: Mutex::new(HashMap::new()),
            dnr_session_rules: Mutex::new(HashMap::new()),
            action_state: Mutex::new(HashMap::new()),
        }
    }

    /// Load every unpacked extension in `extensions_dir`.
    pub fn load_all(&self) -> Result<usize, String> {
        let mut count = 0;
        let entries = std::fs::read_dir(&self.extensions_dir)
            .map_err(|e| format!("Cannot read extensions dir: {}", e))?;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if let Err(e) = self.load_from_dir(&path) {
                    warn!("Failed to load extension at {:?}: {}", path, e);
                } else {
                    count += 1;
                }
            }
        }
        info!("Loaded {} extension(s) from {:?}", count, self.extensions_dir);
        Ok(count)
    }

    /// Load a single unpacked extension directory.
    pub fn load_from_dir(&self, dir: &Path) -> Result<ExtensionId, String> {
        let manifest = ExtensionManifest::from_dir(dir)?;
        let id = dir
            .file_name()
            .and_then(|s| s.to_str())
            .map(|s| ExtensionId(s.to_string()))
            .ok_or("Extension directory has no name")?;
        info!("Loading extension {} ({}) from {:?}", id.0, manifest.name, dir);
        let ext = LoadedExtension::new(id.clone(), dir.to_path_buf(), manifest);
        self.extensions.lock().insert(id.clone(), ext);
        Ok(id)
    }

    pub fn unload(&self, id: &ExtensionId) -> bool {
        self.extensions.lock().remove(id).is_some()
    }

    pub fn list(&self) -> Vec<LoadedExtension> {
        self.extensions.lock().values().cloned().collect()
    }

    pub fn get(&self, id: &ExtensionId) -> Option<LoadedExtension> {
        self.extensions.lock().get(id).cloned()
    }

    pub fn set_enabled(&self, id: &ExtensionId, enabled: bool) -> Result<(), String> {
        let mut exts = self.extensions.lock();
        let ext = exts
            .get_mut(id)
            .ok_or_else(|| format!("Extension {} not found", id.0))?;
        ext.enabled = enabled;
        Ok(())
    }

    pub fn get_manifest(&self, id: &ExtensionId) -> Option<ExtensionManifest> {
        self.extensions.lock().get(id).map(|e| e.manifest.clone())
    }

    pub fn permissions(&self, id: &ExtensionId) -> Option<PermissionSet> {
        self.extensions.lock().get(id).map(|e| e.permissions.clone())
    }

    pub fn has_permission(&self, id: &ExtensionId, perm: &Permission) -> bool {
        self.extensions
            .lock()
            .get(id)
            .map(|e| e.permissions.contains(perm))
            .unwrap_or(false)
    }

    pub fn extensions_dir(&self) -> &Path {
        &self.extensions_dir
    }

    pub fn storage(&self) -> &ExtensionStorage {
        &self.storage
    }

    // ── Tab state mirror (updated by Browser) ──

    pub fn new_tab(&self, tab_id: u64, url: &str) {
        let mut state = self.tab_state.lock();
        let index = state.len() as u32;
        let active = state.is_empty();
        state.push(ExtensionTab {
            id: tab_id,
            url: url.to_string(),
            title: String::new(),
            active,
            pinned: false,
            highlighted: active,
            incognito: false,
            window_id: 1,
            index,
            status: Some("loading".into()),
            favicon_url: None,
        });
        self.reindex_tabs(&mut state);
    }

    pub fn close_tab(&self, tab_id: u64) {
        let mut state = self.tab_state.lock();
        state.retain(|t| t.id != tab_id);
        self.reindex_tabs(&mut state);
    }

    pub fn set_active_tab(&self, tab_id: u64) {
        let mut state = self.tab_state.lock();
        for t in state.iter_mut() {
            t.active = t.id == tab_id;
            t.highlighted = t.active;
        }
    }

    pub fn update_tab_url(&self, tab_id: u64, url: &str) {
        let mut state = self.tab_state.lock();
        if let Some(t) = state.iter_mut().find(|t| t.id == tab_id) {
            t.url = url.to_string();
        }
    }

    pub fn update_tab_title(&self, tab_id: u64, title: &str) {
        let mut state = self.tab_state.lock();
        if let Some(t) = state.iter_mut().find(|t| t.id == tab_id) {
            t.title = title.to_string();
        }
    }

    pub fn update_tab_status(&self, tab_id: u64, status: Option<String>) {
        let mut state = self.tab_state.lock();
        if let Some(t) = state.iter_mut().find(|t| t.id == tab_id) {
            t.status = status;
        }
    }

    pub fn tabs(&self) -> Vec<ExtensionTab> {
        self.tab_state.lock().clone()
    }

    pub fn tab(&self, tab_id: u64) -> Option<ExtensionTab> {
        self.tab_state.lock().iter().find(|t| t.id == tab_id).cloned()
    }

    pub fn active_tab(&self) -> Option<ExtensionTab> {
        self.tab_state.lock().iter().find(|t| t.active).cloned()
    }

    pub fn current_window_id(&self) -> u64 {
        1
    }

    pub(crate) fn reindex_tabs(&self, tabs: &mut [ExtensionTab]) {
        for (i, t) in tabs.iter_mut().enumerate() {
            t.index = i as u32;
        }
    }

    // ── Helper accessors used by API handlers ──

    pub fn tab_state(&self) -> parking_lot::MutexGuard<'_, Vec<ExtensionTab>> {
        self.tab_state.lock()
    }

    pub fn next_tab_id(&self) -> u64 {
        self.next_tab_id.fetch_add(1, Ordering::SeqCst)
    }

    pub fn next_download_id(&self) -> u64 {
        self.next_download_id.fetch_add(1, Ordering::SeqCst)
    }

    pub fn next_menu_id(&self) -> u64 {
        self.next_menu_id.fetch_add(1, Ordering::SeqCst)
    }

    pub fn downloads(&self) -> parking_lot::MutexGuard<'_, HashMap<u64, serde_json::Value>> {
        self.downloads.lock()
    }

    pub fn context_menu_items(&self) -> parking_lot::MutexGuard<'_, HashMap<u64, ContextMenuItem>> {
        self.context_menu_items.lock()
    }

    pub fn notification_items(&self) -> parking_lot::MutexGuard<'_, HashMap<String, serde_json::Value>> {
        self.notification_items.lock()
    }

    pub fn alarms(&self) -> parking_lot::MutexGuard<'_, HashMap<String, Alarm>> {
        self.alarms.lock()
    }

    pub fn dnr_rulesets(&self) -> parking_lot::MutexGuard<'_, HashMap<String, bool>> {
        self.dnr_rulesets.lock()
    }

    pub fn dnr_dynamic_rules(&self) -> parking_lot::MutexGuard<'_, HashMap<i64, serde_json::Value>> {
        self.dnr_dynamic_rules.lock()
    }

    pub fn dnr_session_rules(&self) -> parking_lot::MutexGuard<'_, HashMap<i64, serde_json::Value>> {
        self.dnr_session_rules.lock()
    }

    pub fn action_state(&self) -> parking_lot::MutexGuard<'_, HashMap<ExtensionId, serde_json::Value>> {
        self.action_state.lock()
    }

    /// Dispatch an API request into the adapter.
    pub fn call_api(&self, req: ApiRequest) -> ApiResponse {
        crate::extensions::api::dispatch(self, req)
    }
}

// Manual Clone for LoadedExtension (manifest/root cloneable).
impl Clone for LoadedExtension {
    fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            manifest: self.manifest.clone(),
            root: self.root.clone(),
            enabled: self.enabled,
            permissions: self.permissions.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_loads_and_tracks_tabs() {
        let dir = tempfile::tempdir().unwrap();
        let ext_dir = dir.path().join("test-ext");
        std::fs::create_dir(&ext_dir).unwrap();
        std::fs::write(
            ext_dir.join("manifest.json"),
            r#"{"manifest_version":2,"name":"T","version":"1.0","permissions":["tabs"]}"#,
        )
        .unwrap();

        let rt = ExtensionRuntime::new(dir.path().join("extensions"));
        let id = rt.load_from_dir(&ext_dir).unwrap();
        assert_eq!(id.0, "test-ext");

        rt.new_tab(1, "https://example.com");
        rt.set_active_tab(1);
        rt.update_tab_title(1, "Example");

        let tab = rt.active_tab().unwrap();
        assert_eq!(tab.title, "Example");
    }
}
