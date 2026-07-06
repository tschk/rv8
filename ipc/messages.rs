//! IPC Message Types
//!
//! Defines all message types for inter-process communication.

use serde::{Deserialize, Serialize};

use rv8_browser_optimizations::runtime::SurfaceId;

use crate::js::JsValue;
use crate::renderer::RenderFrame;

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
