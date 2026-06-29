//! Inter-Process Communication for RV8
//!
//! This module provides IPC mechanisms for communication between
//! browser, renderer, GPU, and network processes.

use ipc_channel::ipc;
pub use ipc_channel::ipc::{IpcOneShotServer, IpcReceiver, IpcSender};
use serde::{Deserialize, Serialize};

pub mod messages;

// Re-exports
pub use messages::*;

/// Create a new IPC channel pair
pub fn channel<T>() -> Result<(IpcSender<T>, IpcReceiver<T>), String>
where
    T: Serialize + for<'de> Deserialize<'de>,
{
    ipc::channel().map_err(|e| e.to_string())
}

pub fn bridge_ipc_receiver<T>(rx: IpcReceiver<T>, tx: tokio::sync::mpsc::UnboundedSender<T>)
where
    T: serde::Serialize + for<'de> serde::Deserialize<'de> + Send + 'static,
{
    std::thread::spawn(move || {
        while let Ok(msg) = rx.recv() {
            if tx.send(msg).is_err() {
                break;
            }
        }
    });
}

/// Communication channel from a renderer process back to the browser.
pub struct RendererChannel {
    pub tab_id: u64,
    pub to_browser: IpcSender<BrowserMessage>,
}

impl RendererChannel {
    pub fn new(tab_id: u64, to_browser: IpcSender<BrowserMessage>) -> Self {
        RendererChannel { tab_id, to_browser }
    }

    pub fn send_title_changed(&self, title: &str) -> Result<(), String> {
        self.to_browser
            .send(BrowserMessage::TitleChanged {
                tab_id: self.tab_id,
                title: title.to_string(),
            })
            .map_err(|e| e.to_string())
    }

    pub fn send_load_complete(&self) -> Result<(), String> {
        self.to_browser
            .send(BrowserMessage::LoadComplete {
                tab_id: self.tab_id,
            })
            .map_err(|e| e.to_string())
    }

    pub fn send_frame_ready(&self, frame: crate::renderer::RenderFrame) -> Result<(), String> {
        self.to_browser
            .send(BrowserMessage::FrameReady {
                tab_id: self.tab_id,
                frame,
            })
            .map_err(|e| e.to_string())
    }

    pub fn send_script_result(
        &self,
        callback_id: u64,
        result: Result<crate::js::JsValue, String>,
    ) -> Result<(), String> {
        self.to_browser
            .send(BrowserMessage::ScriptResult {
                tab_id: self.tab_id,
                callback_id,
                result,
            })
            .map_err(|e| e.to_string())
    }
}

/// Client for sending messages from the browser to a renderer process.
pub struct RendererClient {
    pub tab_id: u64,
    pub tx: IpcSender<RendererMessage>,
}

impl RendererClient {
    pub fn new(tab_id: u64, tx: IpcSender<RendererMessage>) -> Self {
        RendererClient { tab_id, tx }
    }

    pub fn send_navigate(&self, url: &str) -> Result<(), String> {
        self.tx
            .send(RendererMessage::Navigate {
                url: url.to_string(),
            })
            .map_err(|e| e.to_string())
    }

    pub fn send_reload(&self) -> Result<(), String> {
        self.tx
            .send(RendererMessage::Reload)
            .map_err(|e| e.to_string())
    }

    pub fn send_stop(&self) -> Result<(), String> {
        self.tx
            .send(RendererMessage::Stop)
            .map_err(|e| e.to_string())
    }

    pub fn send_close(&self) -> Result<(), String> {
        self.tx
            .send(RendererMessage::Shutdown)
            .map_err(|e| e.to_string())
    }
}

/// IPC Server for managing channels to child processes
pub struct IpcServer {
    channels: std::sync::Mutex<std::collections::HashMap<String, IpcSender<BrowserMessage>>>,
}

impl IpcServer {
    pub fn new() -> Self {
        IpcServer {
            channels: std::sync::Mutex::new(std::collections::HashMap::new()),
        }
    }

    /// Create a bootstrap server for a new process
    pub fn create_bootstrap_server(
    ) -> Result<(String, IpcOneShotServer<IpcSender<RendererMessage>>), String> {
        let (server, name) = IpcOneShotServer::new()
            .map_err(|e| format!("Failed to create bootstrap server: {}", e))?;
        Ok((name, server))
    }

    /// Create a channel for in-process or manual connection
    pub fn create_channel(
        &self,
        channel_id: &str,
    ) -> Result<(RendererChannel, IpcReceiver<BrowserMessage>), String> {
        let (tx, rx) = ipc::channel().map_err(|e| e.to_string())?;

        {
            let mut channels = self.channels.lock().unwrap_or_else(|e| e.into_inner());
            channels.insert(channel_id.to_string(), tx.clone());
        }

        // Extract tab_id from channel name
        let tab_id = channel_id
            .split('-')
            .next_back()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(0);

        Ok((RendererChannel::new(tab_id, tx), rx))
    }

    pub async fn close_channel(&self, channel_id: &str) {
        let mut channels = self.channels.lock().unwrap_or_else(|e| e.into_inner());
        channels.remove(channel_id);
    }
}

impl Default for IpcServer {
    fn default() -> Self {
        Self::new()
    }
}
