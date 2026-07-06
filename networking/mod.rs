//! Network stack module

use crate::storage::{CookieJar, StorageManager};
use log::info;
use std::sync::Arc;

/// Network manager for HTTP requests
pub struct NetworkManager {
    cookies: Arc<CookieJar>,
}

impl NetworkManager {
    pub async fn new(storage: Arc<StorageManager>) -> Result<Self, String> {
        info!("Initializing network manager");
        Ok(NetworkManager {
            cookies: Arc::new(storage.cookies.clone()),
        })
    }

    pub fn cookie_jar(&self) -> &CookieJar {
        &self.cookies
    }
}

/// HTTP request
pub struct Request {
    pub url: String,
    pub method: String,
}

/// HTTP response
pub struct Response {
    pub status: u16,
    pub body: Vec<u8>,
}

/// Network process (runs in child process)
pub struct NetworkProcess {
    _channel_id: String,
}

impl NetworkProcess {
    pub async fn new(channel_id: &str) -> Self {
        info!("Network process initializing with channel: {}", channel_id);
        NetworkProcess {
            _channel_id: channel_id.to_string(),
        }
    }

    pub async fn run(&self) {
        info!("Network process running on channel {}", self._channel_id);
        // ponytail: subprocess IPC bootstrap not wired yet.
        // See spawn_renderer_process for IpcOneShotServer pattern.
        std::future::pending::<()>().await;
    }
}
