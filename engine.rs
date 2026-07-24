use crate::renderer::RenderFrame;
use crate::servo_embed::{ServoConfig, ServoEmbedder};

pub struct BrowserEngine {
    embedder: ServoEmbedder,
}

impl BrowserEngine {
    pub async fn new(config: ServoConfig) -> Result<Self, String> {
        Ok(Self {
            embedder: ServoEmbedder::new(config).await?,
        })
    }

    pub async fn navigate(&mut self, url: &str) -> Result<(), String> {
        self.embedder.navigate(url).await
    }

    pub async fn evaluate_script(&mut self, script: &str) -> Result<String, String> {
        self.embedder.execute_script(script).await
    }

    pub fn capture_frame(&mut self) -> Option<RenderFrame> {
        self.embedder.get_render_frame()
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.embedder.resize(width, height);
    }

    pub fn title(&self) -> &str {
        self.embedder.title()
    }

    pub fn current_url(&self) -> &str {
        self.embedder.current_url()
    }

    /// Send a Chrome DevTools Protocol JSON-RPC message.
    pub async fn cdp_send(&mut self, json: &str) -> Result<String, String> {
        self.embedder.cdp_send(json).await
    }

    pub fn viewport_size(&self) -> (u32, u32) {
        self.embedder.viewport_size()
    }

    pub fn is_loading(&self) -> bool {
        self.embedder.is_loading()
    }

    pub fn load_progress(&self) -> u8 {
        self.embedder.load_progress()
    }
}

#[cfg(test)]
mod tests {
    #[cfg(not(feature = "servo-render"))]
    use super::*;

    #[cfg(not(feature = "servo-render"))]
    #[tokio::test]
    async fn engine_uses_configured_viewport() {
        let engine = BrowserEngine::new(ServoConfig {
            width: 320,
            height: 240,
            ..ServoConfig::default()
        })
        .await
        .unwrap();

        assert_eq!(engine.viewport_size(), (320, 240));
        assert!(!engine.is_loading());
        assert_eq!(engine.load_progress(), 0);
    }
}
