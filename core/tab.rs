//! Tab representation and management

use log::{debug, info};
use std::sync::Arc;

use super::NavigationController;
use crate::ipc::RendererClient;
use crate::networking::NetworkManager;
use crate::renderer::RenderFrame;

/// Unique tab identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TabId(pub u64);

/// Tab loading state
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TabState {
    /// Initial state
    New,
    /// Loading content
    Loading,
    /// Content loaded, rendering
    Loaded,
    /// Error occurred
    Error(String),
    /// Tab is crashed
    Crashed,
}

/// A browser tab containing a single web page
pub struct Tab {
    /// Tab ID
    id: TabId,

    /// Current URL
    url: String,

    /// Page title
    title: String,

    /// Favicon URL
    favicon_url: Option<String>,

    /// Loading state
    state: TabState,

    /// Navigation history
    navigation: NavigationController,

    /// IPC client for renderer process
    renderer_client: RendererClient,

    #[allow(dead_code)]
    network: Arc<NetworkManager>,

    /// Latest render frame
    render_frame: Option<RenderFrame>,

    /// Loading progress (0-100)
    loading_progress: u8,

    /// Is this tab audible (playing audio)?
    audible: bool,

    /// Is this tab muted?
    muted: bool,
}

impl Tab {
    /// Create a new tab
    pub async fn new(
        id: TabId,
        url: String,
        renderer_client: RendererClient,
        network: Arc<NetworkManager>,
    ) -> Result<Self, String> {
        info!("Creating tab {} with URL: {}", id.0, url);

        Ok(Tab {
            id,
            url: url.clone(),
            title: String::new(),
            favicon_url: None,
            state: TabState::New,
            navigation: NavigationController::new(url),
            renderer_client,
            network,
            render_frame: None,
            loading_progress: 0,
            audible: false,
            muted: false,
        })
    }

    /// Navigate to a URL
    pub async fn navigate(&mut self, url: &str) -> Result<(), String> {
        info!("Tab {} navigating to: {}", self.id.0, url);

        // Validate URL
        let parsed = url::Url::parse(url)
            .or_else(|_| url::Url::parse(&format!("https://{}", url)))
            .map_err(|e| format!("Invalid URL: {}", e))?;

        self.url = parsed.to_string();
        self.state = TabState::Loading;
        self.loading_progress = 0;

        // Add to navigation history
        self.navigation.push(parsed.to_string());

        // Send navigation request to renderer
        self.renderer_client.send_navigate(&self.url)?;

        Ok(())
    }

    /// Go back in history
    pub async fn go_back(&mut self) -> Result<(), String> {
        #[cfg(feature = "servo-render")]
        {
            self.renderer_client.send_go_back()?;
        }
        #[cfg(not(feature = "servo-render"))]
        if let Some(url) = self.navigation.go_back() {
            self.url = url.clone();
            self.state = TabState::Loading;
            self.renderer_client.send_navigate(&url)?;
        }
        Ok(())
    }

    /// Go forward in history
    pub async fn go_forward(&mut self) -> Result<(), String> {
        #[cfg(feature = "servo-render")]
        {
            self.renderer_client.send_go_forward()?;
        }
        #[cfg(not(feature = "servo-render"))]
        if let Some(url) = self.navigation.go_forward() {
            self.url = url.clone();
            self.state = TabState::Loading;
            self.renderer_client.send_navigate(&url)?;
        }
        Ok(())
    }

    /// Reload the page
    pub async fn reload(&mut self) -> Result<(), String> {
        self.state = TabState::Loading;
        self.loading_progress = 0;
        self.renderer_client.send_reload()
    }

    /// Stop loading
    pub async fn stop(&mut self) -> Result<(), String> {
        self.renderer_client.send_stop()?;
        self.state = TabState::Loaded;
        Ok(())
    }

    /// Close the tab
    pub async fn close(&self) {
        info!("Closing tab {}", self.id.0);
        let _ = self.renderer_client.send_close();
    }

    /// Get the latest render frame
    pub async fn get_render_frame(&self) -> Option<RenderFrame> {
        self.render_frame.clone()
    }

    /// Update title from renderer
    pub fn set_title(&mut self, title: String) {
        debug!("Tab {} title: {}", self.id.0, title);
        self.title = title;
    }

    /// Update favicon
    pub fn set_favicon(&mut self, url: String) {
        self.favicon_url = Some(url);
    }

    /// Update loading progress
    pub fn set_loading_progress(&mut self, progress: u8) {
        self.loading_progress = progress.min(100);
        if progress >= 100 {
            self.state = TabState::Loaded;
        }
    }

    /// Update render frame
    pub fn set_render_frame(&mut self, frame: RenderFrame) {
        self.render_frame = Some(frame);
    }

    /// Mark tab as crashed
    pub fn mark_crashed(&mut self) {
        self.state = TabState::Crashed;
    }

    // Getters
    pub fn id(&self) -> TabId {
        self.id
    }
    pub fn url(&self) -> &str {
        &self.url
    }
    pub fn title(&self) -> &str {
        &self.title
    }
    pub fn state(&self) -> &TabState {
        &self.state
    }
    pub fn loading_progress(&self) -> u8 {
        self.loading_progress
    }
    pub fn is_audible(&self) -> bool {
        self.audible
    }
    pub fn is_muted(&self) -> bool {
        self.muted
    }

    /// Toggle mute
    pub fn toggle_mute(&mut self) {
        self.muted = !self.muted;
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::ipc::{channel, RendererClient, RendererMessage};
    use crate::networking::NetworkManager;
    use crate::storage::StorageManager;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_tab_set_title() {
        let temp_dir = tempfile::tempdir().unwrap();
        let storage = Arc::new(StorageManager::open(temp_dir.path(), true).unwrap());
        let network = Arc::new(NetworkManager::new(storage).await.unwrap());

        let (tx, _rx) = channel::<RendererMessage>().unwrap();
        let renderer_client = RendererClient::new(1, tx);

        let id = TabId(1);
        let url = "https://example.com".to_string();

        let mut tab = Tab::new(id, url, renderer_client, network).await.unwrap();

        assert_eq!(tab.title(), "");

        let new_title = "Example Domain".to_string();
        tab.set_title(new_title.clone());

        assert_eq!(tab.title(), new_title);
    }
}
