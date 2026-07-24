//! Network stack module

mod websocket;

pub use websocket::{
    WebSocketConnection, WebSocketFrame, WebSocketManager, WebSocketState,
};

use crate::optimizations::{
    DnsPrefetchCache, PrefetchManager, PrefetchPriority, PriorityQueue, ResourceRequest,
    ResourceType,
};
use crate::storage::{CookieJar, StorageManager};
use log::info;
use std::sync::Arc;

/// Default maximum concurrent network requests.
const DEFAULT_MAX_CONCURRENT: usize = 6;

/// Network manager for HTTP requests
pub struct NetworkManager {
    cookies: Arc<CookieJar>,
    /// DNS prefetch cache.
    dns_prefetch: DnsPrefetchCache,
    /// Resource prefetch manager.
    prefetch: PrefetchManager,
    /// Priority queue for in-flight resource requests.
    priority_queue: PriorityQueue,
}

impl NetworkManager {
    pub async fn new(storage: Arc<StorageManager>) -> Result<Self, String> {
        info!("Initializing network manager");
        Ok(NetworkManager {
            cookies: Arc::new(storage.cookies.clone()),
            dns_prefetch: DnsPrefetchCache::new(),
            prefetch: PrefetchManager::new(),
            priority_queue: PriorityQueue::new(DEFAULT_MAX_CONCURRENT),
        })
    }

    pub fn cookie_jar(&self) -> &CookieJar {
        &self.cookies
    }

    /// Access the DNS prefetch cache.
    pub fn dns_prefetch(&self) -> &DnsPrefetchCache {
        &self.dns_prefetch
    }

    /// Access the resource prefetch manager.
    pub fn prefetch(&self) -> &PrefetchManager {
        &self.prefetch
    }

    /// Access the resource priority queue.
    pub fn priority_queue(&self) -> &PriorityQueue {
        &self.priority_queue
    }

    /// Submit a resource request to the priority queue.
    pub fn queue_resource(&mut self, request: ResourceRequest) -> u64 {
        self.priority_queue.enqueue(request)
    }

    /// Request a resource prefetch.
    pub fn request_prefetch(&mut self, url: &str, priority: PrefetchPriority) {
        self.prefetch
            .request_prefetch(url.to_string(), ResourceType::Resource, priority);
    }

    /// Register a DNS prefetch candidate for a host.
    pub fn prefetch_host(&mut self, host: &str) {
        self.dns_prefetch.prefetch(host.to_string());
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
