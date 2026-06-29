//! Servo embedding for RV8
//!
//! This module provides an interface to integrate Servo's rendering
//! capabilities while using V8 for JavaScript execution.
//!
//! Architecture:
//! - Servo handles HTML/CSS parsing, layout, and painting
//! - V8 (from rv8) handles JavaScript execution
//! - This module bridges the two engines

use log::{debug, info};
use parking_lot::RwLock;
use std::sync::Arc;

use crate::js::JsValue;
use crate::renderer::RenderFrame;

#[cfg(feature = "rv8-v8")]
use crate::js::bindings::V8ContextData;
#[cfg(feature = "rv8-v8")]
use crate::js::JsEngine;

#[cfg(not(feature = "servo-render"))]
pub mod dom;
#[cfg(feature = "servo-render")]
mod embedder_polyfills;
#[cfg(not(feature = "servo-render"))]
mod paint;
#[cfg(not(feature = "servo-render"))]
pub mod parser;
#[cfg(feature = "servo-render")]
mod servo_renderer;
#[cfg(feature = "servo-render")]
pub mod viewport;
#[cfg(not(feature = "servo-render"))]
pub mod web_apis;

#[cfg(not(feature = "servo-render"))]
use self::dom::{DomEvent, DomTree};
#[cfg(not(feature = "servo-render"))]
use self::web_apis::{ConsoleApi, StorageApi, TimerManager};

/// Servo embedding configuration
#[derive(Debug, Clone)]
pub struct ServoConfig {
    /// Initial viewport width
    pub width: u32,
    /// Initial viewport height
    pub height: u32,
    /// Enable hardware acceleration
    pub hardware_acceleration: bool,
    /// Enable WebGL
    pub webgl: bool,
    /// Enable WebGPU
    pub webgpu: bool,
    /// User agent string
    pub user_agent: String,
    /// Resource directory path
    pub resources_path: Option<String>,
}

impl Default for ServoConfig {
    fn default() -> Self {
        ServoConfig {
            width: 1280,
            height: 800,
            hardware_acceleration: true,
            webgl: true,
            webgpu: false,
            user_agent: crate::user_agent(),
            resources_path: None,
        }
    }
}

/// Servo embedder for RV8
pub struct ServoEmbedder {
    /// Configuration
    config: ServoConfig,
    /// Standalone V8 (software-render builds only; Servo path uses soliloquy_v8)
    #[cfg(feature = "rv8-v8")]
    pub js_engine: Arc<Mutex<JsEngine>>,
    /// DOM Tree
    dom_tree: Arc<RwLock<DomTree>>,
    #[allow(dead_code)]
    console_api: Arc<RwLock<ConsoleApi>>,
    /// Timer Manager
    timer_manager: Arc<RwLock<TimerManager>>,
    #[allow(dead_code)]
    local_storage: Arc<RwLock<StorageApi>>,
    #[allow(dead_code)]
    session_storage: Arc<RwLock<StorageApi>>,
    /// Current document URL
    current_url: String,
    /// Document title
    title: String,
    /// Is currently loading
    loading: bool,
    /// Load progress (0-100)
    load_progress: u8,
    #[allow(dead_code)]
    frame_generation: u64,
    #[cfg(feature = "servo-render")]
    servo: Option<servo_renderer::ServoRenderer>,
}

impl ServoEmbedder {
    /// Create a new Servo embedder
    pub async fn new(config: ServoConfig) -> Result<Self, String> {
        info!("Initializing Servo embedder");

        let dom_tree = Arc::new(RwLock::new(DomTree::new()));
        let console_api = Arc::new(RwLock::new(ConsoleApi::new()));
        let timer_manager = Arc::new(RwLock::new(TimerManager::new()));
        let local_storage = Arc::new(RwLock::new(StorageApi::new(5 * 1024 * 1024)));
        let session_storage = Arc::new(RwLock::new(StorageApi::new(5 * 1024 * 1024)));

        #[cfg(feature = "rv8-v8")]
        let js_engine = {
            let mut js_engine =
                JsEngine::new().map_err(|e| format!("Failed to create V8 engine: {}", e))?;
            info!("V8 JavaScript engine version: {}", JsEngine::version());
            js_engine.initialize(V8ContextData::new(
                dom_tree.clone(),
                console_api.clone(),
                timer_manager.clone(),
                local_storage.clone(),
                session_storage.clone(),
            ));
            Arc::new(Mutex::new(js_engine))
        };

        #[cfg(feature = "servo-render")]
        let servo = Some(
            servo_renderer::ServoRenderer::new(config.width, config.height)
                .map_err(|e| format!("Servo renderer init failed: {e}"))?,
        );

        Ok(ServoEmbedder {
            config,
            #[cfg(feature = "rv8-v8")]
            js_engine,
            dom_tree,
            console_api,
            timer_manager,
            local_storage,
            session_storage,
            current_url: String::new(),
            title: String::new(),
            loading: false,
            load_progress: 0,
            frame_generation: 0,
            #[cfg(feature = "servo-render")]
            servo,
        })
    }

    /// Navigate to a URL
    pub async fn navigate(&mut self, url: &str) -> Result<(), String> {
        info!("Navigating to: {}", url);

        self.current_url = url.to_string();
        self.loading = true;
        self.load_progress = 0;

        #[cfg(feature = "servo-render")]
        if let Some(ref mut servo) = self.servo {
            servo.navigate(url)?;
            self.title = servo.title();
            self.frame_generation = self.frame_generation.saturating_add(1);
            self.loading = false;
            self.load_progress = 100;
            return Ok(());
        }

        #[cfg(not(feature = "servo-render"))]
        {
            use log::error;
            info!("Fetching URL: {}", url);
            match reqwest::get(url).await {
                Ok(response) => {
                    if !response.status().is_success() {
                        error!("Failed to fetch URL {}: Status {}", url, response.status());
                        self.loading = false;
                        return Err(format!("HTTP error: {}", response.status()));
                    }

                    match response.text().await {
                        Ok(html) => {
                            info!("Parsing HTML...");
                            self.load_progress = 50;

                            {
                                let mut dom = self.dom_tree.write();
                                *dom = DomTree::new();
                                parser::parse_html(&html, &mut dom);
                            }
                            self.title = self.dom_tree.read().document_title().unwrap_or_default();
                            info!("HTML parsing complete");
                        }
                        Err(e) => {
                            error!("Failed to read response text: {}", e);
                            self.loading = false;
                            return Err(format!("Failed to read response: {e}"));
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to fetch URL {}: {}", url, e);
                    self.loading = false;
                    return Err(format!("Network error: {e}"));
                }
            }
        }

        self.loading = false;
        self.load_progress = 100;

        Ok(())
    }

    /// Execute JavaScript in the context of the current document
    pub async fn go_back(&mut self) {
        #[cfg(feature = "servo-render")]
        if let Some(ref mut servo) = self.servo {
            servo.go_back();
        }
    }

    pub async fn go_forward(&mut self) {
        #[cfg(feature = "servo-render")]
        if let Some(ref mut servo) = self.servo {
            servo.go_forward();
        }
    }

    pub async fn execute_script(&mut self, script: &str) -> Result<String, String> {
        #[cfg(feature = "servo-render")]
        if let Some(ref mut servo) = self.servo {
            return servo.evaluate_script_sync(script);
        }
        #[cfg(feature = "rv8-v8")]
        {
            let mut engine = self.js_engine.lock().await;
            return engine.execute_to_string(script);
        }
        #[cfg(not(any(feature = "servo-render", feature = "rv8-v8")))]
        let _ = script;
        Err("JavaScript backend not enabled".to_string())
    }

    /// Execute JavaScript and return a typed transport value.
    pub async fn execute_script_value(&mut self, script: &str) -> Result<JsValue, String> {
        #[cfg(feature = "servo-render")]
        if let Some(ref mut servo) = self.servo {
            return servo.evaluate_script_value_sync(script);
        }
        #[cfg(feature = "rv8-v8")]
        {
            let mut engine = self.js_engine.lock().await;
            return engine.execute(script);
        }
        #[cfg(not(any(feature = "servo-render", feature = "rv8-v8")))]
        let _ = script;
        Err("JavaScript backend not enabled".to_string())
    }

    /// Get the current render frame
    pub fn get_render_frame(&mut self) -> Option<RenderFrame> {
        #[cfg(feature = "servo-render")]
        if let Some(ref mut servo) = self.servo {
            let gen = self.frame_generation.saturating_add(1);
            if let Some(frame) = servo.capture_frame(gen) {
                self.frame_generation = frame.id;
                return Some(frame);
            }
        }

        #[cfg(not(feature = "servo-render"))]
        {
            let mut frame = RenderFrame::new(self.config.width, self.config.height);
            let dom = self.dom_tree.read();
            let ctx = paint::PaintContext {
                url: &self.current_url,
                title: &self.title,
                loading: self.loading,
            };
            paint::paint_document_frame(&mut frame, &dom, &ctx);
            Some(frame)
        }

        #[cfg(feature = "servo-render")]
        None
    }

    /// Resize the viewport
    pub fn resize(&mut self, width: u32, height: u32) {
        self.config.width = width;
        self.config.height = height;
        #[cfg(feature = "servo-render")]
        if let Some(ref mut servo) = self.servo {
            servo.resize(width, height);
        }
        debug!("Viewport resized to {}x{}", width, height);
    }

    /// Handle mouse event
    pub async fn handle_mouse_move(&mut self, x: f32, y: f32) {
        debug!("Mouse move: ({}, {})", x, y);
        let target_id = self.dom_tree.read().document_id();
        let event = DomEvent::mouse("mousemove", target_id, x, y, MouseButton::Left);
        self.dom_tree.write().record_event(event.clone());

        #[cfg(feature = "servo-render")]
        if let Some(ref mut servo) = self.servo {
            servo.handle_mouse_move(x, y);
            self.frame_generation = self.frame_generation.saturating_add(1);
            return;
        }

        #[cfg(feature = "rv8-v8")]
        {
            let mut engine = self.js_engine.lock().await;
            engine.dispatch_event(&event);
        }
    }

    /// Handle mouse click
    pub async fn handle_mouse_click(&mut self, x: f32, y: f32, button: MouseButton) {
        debug!("Mouse click: ({}, {}) button={:?}", x, y, button);
        let target_id = self.dom_tree.read().document_id();
        let event = DomEvent::mouse("click", target_id, x, y, button);
        self.dom_tree.write().record_event(event.clone());
        #[cfg(feature = "rv8-v8")]
        {
            let mut engine = self.js_engine.lock().await;
            engine.dispatch_event(&event);
        }
    }

    /// Handle key event
    pub async fn handle_key(&mut self, key: &str, pressed: bool) {
        debug!("Key event: {} pressed={}", key, pressed);
        let target_id = self.dom_tree.read().document_id();
        let event_type = if pressed { "keydown" } else { "keyup" };
        let event = DomEvent::key(event_type, target_id, key);
        self.dom_tree.write().record_event(event.clone());
        #[cfg(feature = "rv8-v8")]
        {
            let mut engine = self.js_engine.lock().await;
            engine.dispatch_event(&event);
        }
    }

    /// Handle focus event
    pub fn handle_focus(&mut self, focused: bool) {
        #[cfg(feature = "servo-render")]
        if let Some(ref mut servo) = self.servo {
            servo.handle_focus(focused);
            self.frame_generation = self.frame_generation.saturating_add(1);
            return;
        }
        debug!("Focus changed: {}", focused);
    }

    /// Handle scroll event
    pub fn handle_scroll(&mut self, delta_x: f32, delta_y: f32) {
        #[cfg(feature = "servo-render")]
        if let Some(ref mut servo) = self.servo {
            servo.scroll_by(delta_x, delta_y);
            self.frame_generation = self.frame_generation.saturating_add(1);
            return;
        }
        debug!("Scroll: ({}, {})", delta_x, delta_y);
    }

    /// Poll and execute ready timers
    pub async fn poll_timers(&self) {
        let ready_timers = {
            let mut manager = self.timer_manager.write();
            manager.poll_ready_timers()
        };

        if !ready_timers.is_empty() {
            #[cfg(feature = "rv8-v8")]
            {
                let mut engine = self.js_engine.lock().await;
                for timer in ready_timers {
                    engine.call_timer_callback(timer.id);
                }
            }
        }
    }

    // Getters
    pub fn current_url(&self) -> &str {
        &self.current_url
    }
    pub fn title(&self) -> &str {
        &self.title
    }
    pub fn is_loading(&self) -> bool {
        self.loading
    }
    pub fn load_progress(&self) -> u8 {
        self.load_progress
    }
    pub fn viewport_size(&self) -> (u32, u32) {
        (self.config.width, self.config.height)
    }
}

/// Mouse button type
#[derive(Debug, Clone, Copy)]
pub enum MouseButton {
    Left,
    Middle,
    Right,
}

/// DOM element representation
#[derive(Debug, Clone)]
pub struct DomElement {
    /// Tag name (e.g., "div", "p", "a")
    pub tag: String,
    /// Element ID
    pub id: Option<String>,
    /// CSS classes
    pub classes: Vec<String>,
    /// Bounding box (x, y, width, height)
    pub bounds: (f32, f32, f32, f32),
}

/// Document information
#[derive(Debug, Clone)]
pub struct DocumentInfo {
    /// Document URL
    pub url: String,
    /// Document title
    pub title: String,
    /// Content type
    pub content_type: String,
    /// Character encoding
    pub charset: String,
    /// Is secure (HTTPS)
    pub secure: bool,
}
