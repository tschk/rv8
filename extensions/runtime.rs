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
use std::sync::Arc;

use super::api::{ApiRequest, ApiResponse};
use super::manifest::{Background, ExtensionManifest};
use super::matchers::{glob_match, url_matches_pattern};
use super::permissions::{Permission, PermissionSet};
use super::storage::ExtensionStorage;
use crate::storage::CookieJar;

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

/// A content script that should be injected for a given URL.
#[derive(Debug, Clone)]
pub struct ContentScriptMatch {
    pub extension_id: ExtensionId,
    pub js: Vec<String>,
    pub css: Vec<String>,
    pub run_at: String,
    pub all_frames: bool,
    pub match_about_blank: bool,
}

/// Tab driver trait supplied by the browser process so extension API calls
/// that create, close, or mutate tabs can reach the real Browser state.
pub trait TabDriver: Send + Sync {
    /// Create a tab and return its new id.
    fn create_tab(&self, url: &str, active: bool) -> Result<u64, String>;
    /// Close the given tab ids.
    fn close_tabs(&self, ids: &[u64]) -> Result<(), String>;
    /// Update tab properties (url, pinned, active).
    fn update_tab(&self, id: u64, props: serde_json::Value) -> Result<(), String>;
    /// Reload a tab.
    fn reload_tab(&self, id: u64) -> Result<(), String>;
}

/// Background script / service worker descriptor for an extension.
#[derive(Debug, Clone)]
pub struct BackgroundScript {
    pub extension_id: ExtensionId,
    pub scripts: Vec<String>,
    pub service_worker: Option<String>,
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
    next_bookmark_id: AtomicU64,
    next_history_id: AtomicU64,
    downloads: Mutex<HashMap<u64, serde_json::Value>>,
    context_menu_items: Mutex<HashMap<u64, ContextMenuItem>>,
    notification_items: Mutex<HashMap<String, serde_json::Value>>,
    dnr_rulesets: Mutex<HashMap<String, bool>>,
    dnr_dynamic_rules: Mutex<HashMap<i64, serde_json::Value>>,
    dnr_session_rules: Mutex<HashMap<i64, serde_json::Value>>,
    action_state: Mutex<HashMap<ExtensionId, serde_json::Value>>,
    /// Event listeners registered by extensions: key "ext_id|namespace.event".
    event_listeners: Mutex<HashMap<String, Vec<serde_json::Value>>>,
    /// Messages queued by runtime.sendMessage until the JS bridge dispatches them.
    pending_messages: Mutex<Vec<PendingMessage>>,
    /// Optional driver that routes extension tab operations to the browser process.
    tab_driver: Mutex<Option<Arc<dyn TabDriver>>>,
    /// Optional handle to the browser cookie jar.
    cookie_jar: Mutex<Option<Arc<CookieJar>>>,
    /// In-memory bookmark tree.
    bookmarks: Mutex<HashMap<String, BookmarkNode>>,
    /// In-memory browser history.
    history: Mutex<Vec<HistoryItem>>,
    /// Omnibox default suggestion.
    omnibox_default_suggestion: Mutex<Option<serde_json::Value>>,
}

#[derive(Debug, Clone)]
pub struct BookmarkNode {
    pub id: String,
    pub parent_id: Option<String>,
    pub index: u32,
    pub title: String,
    pub url: Option<String>,
    pub children: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct HistoryItem {
    pub id: String,
    pub url: String,
    pub title: String,
    pub last_visit_time: u64,
    pub visit_count: u32,
}

#[derive(Debug, Clone)]
pub struct PendingMessage {
    pub extension_id: ExtensionId,
    pub sender: serde_json::Value,
    pub message: serde_json::Value,
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
            next_bookmark_id: AtomicU64::new(1),
            next_history_id: AtomicU64::new(1),
            downloads: Mutex::new(HashMap::new()),
            context_menu_items: Mutex::new(HashMap::new()),
            notification_items: Mutex::new(HashMap::new()),
            dnr_rulesets: Mutex::new(HashMap::new()),
            dnr_dynamic_rules: Mutex::new(HashMap::new()),
            dnr_session_rules: Mutex::new(HashMap::new()),
            action_state: Mutex::new(HashMap::new()),
            event_listeners: Mutex::new(HashMap::new()),
            pending_messages: Mutex::new(Vec::new()),
            tab_driver: Mutex::new(None),
            cookie_jar: Mutex::new(None),
            bookmarks: Mutex::new(HashMap::new()),
            history: Mutex::new(Vec::new()),
            omnibox_default_suggestion: Mutex::new(None),
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

    pub fn uninstall(&self, id: &ExtensionId) -> Result<(), String> {
        let dir = self.extensions_dir.join(&id.0);
        self.extensions.lock().remove(id);
        if dir.exists() {
            std::fs::remove_dir_all(&dir).map_err(|e| format!("Failed to remove extension dir: {}", e))?;
        }
        Ok(())
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

    // ── Content / background script enumeration ──

    /// Return all content scripts whose `matches` patterns cover `url`.
    pub fn content_scripts_for_url(&self, url: &str) -> Vec<ContentScriptMatch> {
        let exts = self.extensions.lock();
        let mut matches = Vec::new();
        for ext in exts.values().filter(|e| e.enabled) {
            if let Some(scripts) = &ext.manifest.content_scripts {
                for cs in scripts {
                    if cs.matches.iter().any(|pat| url_matches_pattern(pat, url)) {
                        matches.push(ContentScriptMatch {
                            extension_id: ext.id.clone(),
                            js: cs.js.clone().unwrap_or_default(),
                            css: cs.css.clone().unwrap_or_default(),
                            run_at: cs.run_at.clone().unwrap_or_else(|| "document_idle".into()),
                            all_frames: cs.all_frames.unwrap_or(false),
                            match_about_blank: cs.match_about_blank.unwrap_or(false),
                        });
                    }
                }
            }
        }
        matches
    }

    /// Return background scripts / service workers for enabled extensions.
    pub fn background_scripts(&self) -> Vec<BackgroundScript> {
        let exts = self.extensions.lock();
        exts.values()
            .filter(|e| e.enabled)
            .filter_map(|ext| {
                ext.manifest.background.as_ref().map(|bg| BackgroundScript {
                    extension_id: ext.id.clone(),
                    scripts: bg.scripts(),
                    service_worker: match bg {
                        Background::Mv3 { service_worker, .. } => Some(service_worker.clone()),
                        _ => None,
                    },
                })
            })
            .collect()
    }

    /// Read an extension file as a UTF-8 string.
    pub fn read_extension_file(&self, id: &ExtensionId, path: &str) -> Option<String> {
        let exts = self.extensions.lock();
        let ext = exts.get(id)?;
        let rel = path.trim_start_matches('/');
        let full = ext.root.join(rel);
        std::fs::read_to_string(full).ok()
    }

    /// Check whether `resource_path` is web-accessible from `url` for `ext_id`.
    pub fn is_web_accessible_resource(&self, id: &ExtensionId, resource_path: &str, url: &str) -> bool {
        let exts = self.extensions.lock();
        let Some(ext) = exts.get(id) else { return false };
        let Some(wars) = &ext.manifest.web_accessible_resources else { return false };
        for war in wars {
            if !war.resources.iter().any(|r| glob_match(r, resource_path)) {
                continue;
            }
            if let Some(ids) = &war.extension_ids {
                if !ids.iter().any(|i| i == &id.0) {
                    continue;
                }
            }
            if let Some(m) = &war.matches {
                if !m.iter().any(|p| url_matches_pattern(p, url)) {
                    continue;
                }
            }
            return true;
        }
        false
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

    pub fn next_bookmark_id(&self) -> u64 {
        self.next_bookmark_id.fetch_add(1, Ordering::SeqCst)
    }

    pub fn next_history_id(&self) -> u64 {
        self.next_history_id.fetch_add(1, Ordering::SeqCst)
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

    // ── Event listener registry ──

    fn event_key(ext_id: &ExtensionId, namespace: &str, event: &str) -> String {
        format!("{}|{}.{}" , ext_id.0, namespace, event)
    }

    pub fn add_event_listener(&self, ext_id: &ExtensionId, namespace: &str, event: &str, listener: serde_json::Value) {
        let key = Self::event_key(ext_id, namespace, event);
        self.event_listeners.lock().entry(key).or_default().push(listener);
    }

    pub fn remove_event_listener(&self, ext_id: &ExtensionId, namespace: &str, event: &str, listener: &serde_json::Value) {
        let key = Self::event_key(ext_id, namespace, event);
        let mut listeners = self.event_listeners.lock();
        if let Some(v) = listeners.get_mut(&key) {
            v.retain(|l| l != listener);
        }
    }

    pub fn has_event_listener(&self, ext_id: &ExtensionId, namespace: &str, event: &str) -> bool {
        let key = Self::event_key(ext_id, namespace, event);
        matches!(self.event_listeners.lock().get(&key), Some(v) if !v.is_empty())
    }

    pub fn queue_message(&self, extension_id: ExtensionId, sender: serde_json::Value, message: serde_json::Value) {
        self.pending_messages.lock().push(PendingMessage {
            extension_id,
            sender,
            message,
        });
    }

    pub fn drain_pending_messages(&self) -> Vec<PendingMessage> {
        std::mem::take(&mut *self.pending_messages.lock())
    }

    /// Reload an extension by unloading and re-reading its directory.
    pub fn reload_extension(&self, id: &ExtensionId) -> Result<(), String> {
        let dir = self.extensions_dir.join(&id.0);
        self.unload(id);
        self.load_from_dir(&dir).map(|_| ())
    }

    // ── Tab driver wiring ──

    pub fn set_tab_driver(&self, driver: Arc<dyn TabDriver>) {
        *self.tab_driver.lock() = Some(driver);
    }

    pub fn set_cookie_jar(&self, jar: Arc<CookieJar>) {
        *self.cookie_jar.lock() = Some(jar);
    }

    pub fn with_cookie_jar<R>(&self, f: impl FnOnce(&CookieJar) -> R) -> Option<R> {
        self.cookie_jar.lock().as_ref().map(|jar| f(jar))
    }

    pub fn cookie_jar(&self) -> Option<Arc<CookieJar>> {
        self.cookie_jar.lock().clone()
    }

    pub fn bookmarks(&self) -> parking_lot::MutexGuard<'_, HashMap<String, BookmarkNode>> {
        self.bookmarks.lock()
    }

    pub fn history(&self) -> parking_lot::MutexGuard<'_, Vec<HistoryItem>> {
        self.history.lock()
    }

    pub fn omnibox_default_suggestion(&self) -> parking_lot::MutexGuard<'_, Option<serde_json::Value>> {
        self.omnibox_default_suggestion.lock()
    }

    pub fn create_tab_driver(&self, url: &str, active: bool) -> Result<u64, String> {
        self.tab_driver
            .lock()
            .as_ref()
            .ok_or("Tab operations require a browser driver".to_string())?
            .create_tab(url, active)
    }

    pub fn close_tabs_driver(&self, ids: &[u64]) -> Result<(), String> {
        self.tab_driver
            .lock()
            .as_ref()
            .ok_or("Tab operations require a browser driver".to_string())?
            .close_tabs(ids)
    }

    pub fn update_tab_driver(&self, id: u64, props: serde_json::Value) -> Result<(), String> {
        self.tab_driver
            .lock()
            .as_ref()
            .ok_or("Tab operations require a browser driver".to_string())?
            .update_tab(id, props)
    }

    pub fn reload_tab_driver(&self, id: u64) -> Result<(), String> {
        self.tab_driver
            .lock()
            .as_ref()
            .ok_or("Tab operations require a browser driver".to_string())?
            .reload_tab(id)
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

    #[test]
    fn content_scripts_and_resources_match() {
        let dir = tempfile::tempdir().unwrap();
        let ext_dir = dir.path().join("cs-ext");
        std::fs::create_dir(&ext_dir).unwrap();
        std::fs::create_dir(ext_dir.join("js")).unwrap();
        std::fs::write(
            ext_dir.join("manifest.json"),
            r#"{
                "manifest_version": 3,
                "name": "CS",
                "version": "1.0",
                "content_scripts": [{
                    "matches": ["*://*.example.com/*"],
                    "js": ["js/inject.js"],
                    "css": ["js/style.css"],
                    "run_at": "document_start"
                }],
                "web_accessible_resources": [{
                    "resources": ["js/inject.js"],
                    "matches": ["*://*.example.com/*"]
                }]
            }"#,
        )
        .unwrap();
        std::fs::write(ext_dir.join("js/inject.js"), "console.log('injected');").unwrap();

        let rt = ExtensionRuntime::new(dir.path().join("extensions"));
        let id = rt.load_from_dir(&ext_dir).unwrap();

        let matches = rt.content_scripts_for_url("https://foo.example.com/page");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].js, vec!["js/inject.js"]);
        assert_eq!(matches[0].run_at, "document_start");

        assert!(rt.content_scripts_for_url("https://other.org").is_empty());

        assert_eq!(rt.read_extension_file(&id, "js/inject.js"), Some("console.log('injected');".into()));
        assert!(rt.is_web_accessible_resource(&id, "js/inject.js", "https://foo.example.com/page"));
        assert!(!rt.is_web_accessible_resource(&id, "js/missing.js", "https://foo.example.com/page"));
    }
}
