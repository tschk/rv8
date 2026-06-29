//! Main Browser struct - the coordinator for all browser functionality

use log::{debug, info};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};

use super::{BrowserConfig, ProcessManager, Tab, TabId};
use crate::compositor::Compositor;
use crate::ipc::BrowserMessage;
use crate::networking::NetworkManager;
use crate::storage::StorageManager;

/// Main browser instance
pub struct Browser {
    /// Browser configuration
    config: BrowserConfig,

    /// Active tabs
    tabs: RwLock<HashMap<TabId, Arc<Mutex<Tab>>>>,

    /// Currently focused tab
    active_tab: RwLock<Option<TabId>>,

    /// Next tab ID counter
    next_tab_id: std::sync::atomic::AtomicU64,

    /// Process manager for child processes
    process_manager: Arc<ProcessManager>,

    /// GPU compositor
    compositor: Arc<Compositor>,

    /// Network manager
    network: Arc<NetworkManager>,

    /// Storage manager
    storage: Arc<StorageManager>,

    /// Shutdown signal
    shutdown: tokio::sync::broadcast::Sender<()>,

    /// Renderer → browser IPC events
    browser_events: tokio::sync::mpsc::UnboundedReceiver<BrowserMessage>,
}

impl Browser {
    /// Create a new browser instance
    pub async fn new(config: BrowserConfig) -> Result<Self, String> {
        info!("Initializing RV8 browser...");

        // Initialize storage first (needed for cookies, cache)
        let storage = StorageManager::open(&config.data_dirs.profile_dir, config.incognito)
            .map_err(|e| format!("Failed to init storage: {e}"))?;
        let storage = Arc::new(storage);
        info!("Storage manager initialized");

        // Initialize network manager
        let network = NetworkManager::new(storage.clone())
            .await
            .map_err(|e| format!("Failed to init network: {}", e))?;
        let network = Arc::new(network);
        info!("Network manager initialized");

        let (browser_event_tx, browser_event_rx) = tokio::sync::mpsc::unbounded_channel();

        // Initialize process manager
        let process_manager = if config.multi_process {
            ProcessManager::new_multi_process()
        } else {
            ProcessManager::new_single_process()
        };
        let process_manager = Arc::new(process_manager);
        process_manager
            .set_browser_event_forwarder(browser_event_tx)
            .await;
        info!(
            "Process manager initialized (multi_process={})",
            config.multi_process
        );

        // Initialize compositor
        let compositor = Compositor::new(&config)
            .await
            .map_err(|e| format!("Failed to init compositor: {}", e))?;
        let compositor = Arc::new(compositor);
        info!("Compositor initialized");

        let (shutdown_tx, _) = tokio::sync::broadcast::channel(1);

        Ok(Browser {
            config,
            tabs: RwLock::new(HashMap::new()),
            active_tab: RwLock::new(None),
            next_tab_id: std::sync::atomic::AtomicU64::new(1),
            process_manager,
            compositor,
            network,
            storage,
            shutdown: shutdown_tx,
            browser_events: browser_event_rx,
        })
    }

    async fn handle_renderer_message(&self, msg: BrowserMessage) {
        match msg {
            BrowserMessage::FrameReady { tab_id, frame } => {
                let tab_id = TabId(tab_id);
                let tabs = self.tabs.read().await;
                if let Some(tab) = tabs.get(&tab_id) {
                    tab.lock().await.set_render_frame(frame);
                }
            }
            BrowserMessage::TitleChanged { tab_id, title } => {
                let tab_id = TabId(tab_id);
                let tabs = self.tabs.read().await;
                if let Some(tab) = tabs.get(&tab_id) {
                    tab.lock().await.set_title(title);
                }
            }
            BrowserMessage::LoadComplete { tab_id } => {
                debug!("Tab {} load complete", tab_id);
            }
            other => {
                debug!("Browser message: {:?}", other);
            }
        }
        self.compositor.request_frame().await;
    }

    /// Create a new tab and navigate to the given URL
    pub async fn new_tab(&mut self, url: &str) -> Result<TabId, String> {
        let tab_id = TabId(
            self.next_tab_id
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst),
        );

        info!("Creating new tab {} with URL: {}", tab_id.0, url);

        // Create renderer process for this tab
        let renderer_channel = self.process_manager.spawn_renderer(tab_id).await?;

        // Create tab
        let tab = Tab::new(
            tab_id,
            url.to_string(),
            renderer_channel,
            self.network.clone(),
        )
        .await?;

        // Store tab
        {
            let mut tabs = self.tabs.write().await;
            tabs.insert(tab_id, Arc::new(Mutex::new(tab)));
        }

        // Set as active if first tab
        {
            let mut active = self.active_tab.write().await;
            if active.is_none() {
                *active = Some(tab_id);
            }
        }

        // Navigate to URL
        self.navigate_tab(tab_id, url).await?;

        let _ = self.storage.session.upsert_tab(crate::storage::SessionTab {
            tab_id: tab_id.0,
            url: url.to_string(),
            title: String::new(),
        });
        let _ = self.storage.session.set_active_tab(Some(tab_id.0));

        Ok(tab_id)
    }

    /// Navigate a tab to a URL
    pub async fn navigate_tab(&self, tab_id: TabId, url: &str) -> Result<(), String> {
        let tabs = self.tabs.read().await;
        let tab = tabs
            .get(&tab_id)
            .ok_or_else(|| format!("Tab {} not found", tab_id.0))?;

        let mut tab = tab.lock().await;
        tab.navigate(url).await?;
        drop(tab);
        drop(tabs);

        self.compositor.request_frame().await;
        Ok(())
    }

    /// Navigate the active tab to a URL
    pub async fn navigate(&self, url: &str) -> Result<(), String> {
        let active = self.active_tab.read().await;
        let tab_id = active.ok_or("No active tab")?;
        drop(active);

        self.navigate_tab(tab_id, url).await
    }

    /// Close a tab
    pub async fn close_tab(&mut self, tab_id: TabId) -> Result<(), String> {
        info!("Closing tab {}", tab_id.0);

        let mut tabs = self.tabs.write().await;
        if let Some(tab) = tabs.remove(&tab_id) {
            let tab = tab.lock().await;
            tab.close().await;
        }

        // Update active tab
        let mut active = self.active_tab.write().await;
        let was_active = *active == Some(tab_id);
        if *active == Some(tab_id) {
            *active = tabs.keys().next().copied();
        }
        drop(active);
        drop(tabs);

        if was_active {
            self.compositor.request_frame().await;
        }

        // Terminate renderer process
        self.process_manager.terminate_renderer(tab_id).await;

        let _ = self.storage.session.remove_tab(tab_id.0);

        Ok(())
    }

    /// Get the active tab ID
    pub async fn active_tab(&self) -> Option<TabId> {
        *self.active_tab.read().await
    }

    /// Set the active tab
    pub async fn set_active_tab(&self, tab_id: TabId) -> Result<(), String> {
        let tabs = self.tabs.read().await;
        if !tabs.contains_key(&tab_id) {
            return Err(format!("Tab {} not found", tab_id.0));
        }
        drop(tabs);

        let mut active = self.active_tab.write().await;
        *active = Some(tab_id);
        drop(active);

        self.compositor.request_frame().await;

        Ok(())
    }

    /// Get tab count
    pub async fn tab_count(&self) -> usize {
        self.tabs.read().await.len()
    }

    /// Run the browser event loop
    pub async fn run(&mut self) {
        info!("Starting browser event loop");

        let mut shutdown_rx = self.shutdown.subscribe();

        // Main event loop
        loop {
            tokio::select! {
                _ = shutdown_rx.recv() => {
                    info!("Shutdown signal received");
                    break;
                }

                Some(msg) = self.browser_events.recv() => {
                    self.handle_renderer_message(msg).await;
                }

                // Compositor frame
                _ = self.compositor.wait_for_frame() => {
                    self.render_frame().await;
                }


            }
        }

        self.shutdown().await;
    }

    /// Render a frame
    async fn render_frame(&self) {
        // Get active tab
        let active_id = match *self.active_tab.read().await {
            Some(id) => id,
            None => return,
        };

        // Get tab's render frame
        let tabs = self.tabs.read().await;
        if let Some(tab) = tabs.get(&active_id) {
            let tab = tab.lock().await;
            if let Some(frame) = tab.get_render_frame().await {
                self.compositor.submit_frame(frame).await;
            }
        }
    }

    /// Shutdown the browser
    async fn shutdown(&mut self) {
        info!("Shutting down browser...");

        // Close all tabs
        let tab_ids: Vec<TabId> = self.tabs.read().await.keys().copied().collect();
        for tab_id in tab_ids {
            let _ = self.close_tab(tab_id).await;
        }

        // Shutdown process manager
        self.process_manager.shutdown().await;

        // Flush storage
        self.storage.flush().await;

        info!("Browser shutdown complete");
    }

    /// Request shutdown
    pub fn request_shutdown(&self) {
        let _ = self.shutdown.send(());
    }

    /// Persistent profile storage (cookies, session, metadata).
    pub fn config(&self) -> &BrowserConfig {
        &self.config
    }

    pub fn storage(&self) -> &StorageManager {
        &self.storage
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::BrowserDataDirs;
    use tempfile::tempdir;

    async fn create_test_browser() -> (Browser, tempfile::TempDir) {
        let dir = tempdir().unwrap();
        let mut config = BrowserConfig::default();
        config.user_data_dir = dir.path().to_path_buf();
        config.data_dirs.profile_dir = dir.path().to_path_buf();
        config.data_dirs.cache_dir = dir.path().join("cache");
        config.data_dirs.downloads_dir = dir.path().join("downloads");
        config.data_dirs.state_dir = dir.path().join("state");
        config.data_dirs.logs_dir = dir.path().join("logs");
        config.data_dirs.terminal_state_dir = dir.path().join("terminal");
        config.incognito = true;
        config.multi_process = false;

        let browser = Browser::new(config).await.unwrap();
        (browser, dir)
    }

    #[tokio::test]
    async fn test_browser_new_success() {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let mut config = BrowserConfig::default();

        let data_dirs = BrowserDataDirs {
            profile_dir: temp_dir.path().join("profile"),
            cache_dir: temp_dir.path().join("cache"),
            downloads_dir: temp_dir.path().join("downloads"),
            state_dir: temp_dir.path().join("state"),
            logs_dir: temp_dir.path().join("logs"),
            terminal_state_dir: temp_dir.path().join("terminal"),
        };
        config.data_dirs = data_dirs;
        config.user_data_dir = temp_dir.path().join("profile");

        let browser = Browser::new(config)
            .await
            .expect("Failed to create Browser");

        assert_eq!(browser.tab_count().await, 0);
        assert!(browser.active_tab().await.is_none());
        assert_eq!(
            browser
                .next_tab_id
                .load(std::sync::atomic::Ordering::SeqCst),
            1
        );
    }

    #[tokio::test]
    async fn test_browser_incognito() {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let mut config = BrowserConfig::incognito();

        let data_dirs = BrowserDataDirs {
            profile_dir: temp_dir.path().join("profile"),
            cache_dir: temp_dir.path().join("cache"),
            downloads_dir: temp_dir.path().join("downloads"),
            state_dir: temp_dir.path().join("state"),
            logs_dir: temp_dir.path().join("logs"),
            terminal_state_dir: temp_dir.path().join("terminal"),
        };
        config.data_dirs = data_dirs;
        config.user_data_dir = temp_dir.path().join("profile");

        let browser = Browser::new(config)
            .await
            .expect("Failed to create Browser in incognito mode");

        assert_eq!(browser.tab_count().await, 0);
        assert!(browser.active_tab().await.is_none());
        assert_eq!(
            browser
                .next_tab_id
                .load(std::sync::atomic::Ordering::SeqCst),
            1
        );
        assert!(browser.config.incognito);
    }

    #[tokio::test]
    async fn test_new_tab_success() {
        let (mut browser, _dir) = create_test_browser().await;

        let tab_id = browser
            .new_tab("https://example.com")
            .await
            .expect("Failed to create new tab");

        assert_eq!(tab_id.0, 1);
        assert_eq!(browser.tab_count().await, 1);
        assert_eq!(browser.active_tab().await, Some(tab_id));

        let tabs = browser.tabs.read().await;
        let tab = tabs.get(&tab_id).unwrap().lock().await;
        assert_eq!(tab.url(), "https://example.com/");
    }

    #[tokio::test]
    async fn test_new_tab_consecutive_ids() {
        let (mut browser, _dir) = create_test_browser().await;

        let tab_id1 = browser.new_tab("https://example.com").await.unwrap();
        let tab_id2 = browser.new_tab("https://example.org").await.unwrap();

        assert_eq!(tab_id1.0, 1);
        assert_eq!(tab_id2.0, 2);
        assert_eq!(browser.tab_count().await, 2);
    }

    #[tokio::test]
    async fn test_new_tab_empty_url() {
        let (mut browser, _dir) = create_test_browser().await;
        assert!(browser.new_tab("").await.is_err());
    }

    #[tokio::test]
    async fn test_set_active_tab() {
        let (mut browser, _dir) = create_test_browser().await;

        assert!(browser.active_tab().await.is_none());

        let tab_id1 = browser.new_tab("https://example.com").await.unwrap();
        assert_eq!(browser.active_tab().await, Some(tab_id1));

        let tab_id2 = browser.new_tab("https://example.org").await.unwrap();
        assert_eq!(browser.active_tab().await, Some(tab_id1));

        browser.set_active_tab(tab_id2).await.unwrap();
        assert_eq!(browser.active_tab().await, Some(tab_id2));

        let result = browser.set_active_tab(TabId(999)).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Tab 999 not found");
        assert_eq!(browser.active_tab().await, Some(tab_id2));
    }

    #[tokio::test]
    async fn test_navigate() {
        let (mut browser, _dir) = create_test_browser().await;

        let tab_id = browser.new_tab("https://example.com").await.unwrap();
        assert_eq!(browser.active_tab().await, Some(tab_id));

        browser.navigate("https://rust-lang.org").await.unwrap();

        let tabs = browser.tabs.read().await;
        let tab = tabs.get(&tab_id).unwrap().lock().await;
        assert_eq!(tab.url(), "https://rust-lang.org/");
    }

    #[tokio::test]
    async fn test_navigate_no_active_tab() {
        let (browser, _dir) = create_test_browser().await;

        assert_eq!(browser.active_tab().await, None);
        let result = browser.navigate("https://rust-lang.org").await;
        assert_eq!(result, Err("No active tab".to_string()));
    }

    #[tokio::test]
    async fn test_navigate_tab() {
        let (mut browser, _dir) = create_test_browser().await;

        let tab_id = browser.new_tab("https://example.com").await.unwrap();
        browser
            .navigate_tab(tab_id, "https://github.com")
            .await
            .unwrap();

        let tabs = browser.tabs.read().await;
        let tab = tabs.get(&tab_id).unwrap().lock().await;
        assert_eq!(tab.url(), "https://github.com/");
        drop(tab);
        drop(tabs);

        let result = browser
            .navigate_tab(TabId(999), "https://example.com")
            .await;
        assert_eq!(result, Err("Tab 999 not found".to_string()));
    }

    #[tokio::test]
    async fn test_close_tab() {
        let (mut browser, _dir) = create_test_browser().await;

        let tab1 = browser.new_tab("https://example.com").await.unwrap();
        let tab2 = browser.new_tab("https://example.org").await.unwrap();

        assert_eq!(browser.tab_count().await, 2);

        browser.set_active_tab(tab2).await.unwrap();
        browser.close_tab(tab1).await.unwrap();
        assert_eq!(browser.tab_count().await, 1);
        assert_eq!(browser.active_tab().await, Some(tab2));

        browser.close_tab(tab2).await.unwrap();
        assert_eq!(browser.tab_count().await, 0);
        assert_eq!(browser.active_tab().await, None);

        browser.close_tab(TabId(999)).await.unwrap();
        assert_eq!(browser.tab_count().await, 0);
    }

    #[tokio::test]
    async fn test_tab_count() {
        let (mut browser, _dir) = create_test_browser().await;
        assert_eq!(browser.tab_count().await, 0);

        let tab_id_1 = browser.new_tab("https://example.com").await.unwrap();
        assert_eq!(browser.tab_count().await, 1);

        let tab_id_2 = browser.new_tab("https://example.org").await.unwrap();
        assert_eq!(browser.tab_count().await, 2);

        browser.close_tab(tab_id_1).await.unwrap();
        assert_eq!(browser.tab_count().await, 1);

        browser.close_tab(tab_id_2).await.unwrap();
        assert_eq!(browser.tab_count().await, 0);
    }
}
