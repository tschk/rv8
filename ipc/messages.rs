//! IPC Message Types
//!
//! Defines all message types for inter-process communication.

use serde::{Deserialize, Serialize};

use crate::js::JsValue;
use crate::renderer::RenderFrame;

// ── Inlined surface types (formerly from rv8_browser_optimizations::runtime) ──

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlatformTier {
    Desktop,
    ArmLinux,
    Mobile,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SurfaceRotation {
    #[default]
    Deg0,
    Deg90,
    Deg180,
    Deg270,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SurfaceSize {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SurfaceId(pub u64);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SurfaceDescriptor {
    pub id: SurfaceId,
    pub size: SurfaceSize,
    pub scale_factor: f32,
    pub tier: PlatformTier,
    pub rotation: SurfaceRotation,
    pub safe_area: SafeAreaInsets,
    pub touch_enabled: bool,
    pub keyboard_enabled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SafeAreaInsets {
    pub top: u32,
    pub right: u32,
    pub bottom: u32,
    pub left: u32,
}

impl SurfaceDescriptor {
    pub fn new(id: u64, width: u32, height: u32, tier: PlatformTier) -> Self {
        Self {
            id: SurfaceId(id),
            size: SurfaceSize { width, height },
            scale_factor: 1.0,
            tier,
            rotation: SurfaceRotation::Deg0,
            safe_area: SafeAreaInsets::default(),
            touch_enabled: matches!(tier, PlatformTier::Mobile | PlatformTier::ArmLinux),
            keyboard_enabled: true,
        }
    }
}

impl Default for SurfaceDescriptor {
    fn default() -> Self {
        Self::new(0, 1920, 1080, PlatformTier::Desktop)
    }
}

// ── IPC message types ──

use ipc_channel::ipc::IpcSender;

/// Messages from renderer to browser process
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BrowserMessage {
    /// Request navigation to URL
    Navigate { tab_id: u64, url: String },
    /// Document title changed
    TitleChanged { tab_id: u64, title: String },
    /// Page finished loading
    LoadComplete { tab_id: u64 },
    /// Software-rendered frame ready for presentation
    FrameReady { tab_id: u64, frame: RenderFrame },
    /// Request reload
    Reload { tab_id: u64 },
    /// Stop loading
    Stop { tab_id: u64 },
    /// Load progress update (0-100)
    LoadProgress { tab_id: u64, progress: u8 },
    /// Result of a browser-requested script evaluation
    ScriptResult {
        tab_id: u64,
        callback_id: u64,
        result: Result<JsValue, String>,
    },
    /// Request to close tab
    CloseTab { tab_id: u64 },
    /// Renderer process exited or crashed
    RendererCrashed { tab_id: u64 },
}

/// Messages from browser to renderer process
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RendererMessage {
    /// Initialize IPC (handshake)
    Initialize {
        browser_tx: IpcSender<BrowserMessage>,
    },
    /// Navigate to URL
    Navigate { url: String },
    /// Reload page
    Reload,
    /// Stop loading
    Stop,
    /// Go back in history
    GoBack,
    /// Go forward in history
    GoForward,
    /// Execute JavaScript
    ExecuteScript { script: String, callback_id: u64 },
    /// Shutdown renderer
    Shutdown,
}

/// Messages to/from GPU process
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GpuMessage {
    /// Submit frame for compositing
    SubmitFrame {
        tab_id: u64,
        surface_id: SurfaceId,
        frame_id: u64,
    },
    /// Present composited frame
    Present { tab_id: u64, surface_id: SurfaceId },
}

/// Messages to/from network process
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NetworkMessage {
    /// Fetch resource
    Fetch {
        request_id: u64,
        url: String,
        method: String,
        headers: Vec<(String, String)>,
        body: Option<Vec<u8>>,
    },
    /// Response headers received
    ResponseHeaders {
        request_id: u64,
        status: u16,
        headers: Vec<(String, String)>,
    },
    /// Response body chunk
    ResponseBody {
        request_id: u64,
        data: Vec<u8>,
        done: bool,
    },
    /// Response error
    ResponseError { request_id: u64, error: String },
}
