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
    /// HTTP client for making network requests.
    client: reqwest::Client,
}

impl NetworkManager {
    pub async fn new(storage: Arc<StorageManager>) -> Result<Self, String> {
        info!("Initializing network manager");
        Ok(NetworkManager {
            cookies: Arc::new(storage.cookies.clone()),
            dns_prefetch: DnsPrefetchCache::new(),
            prefetch: PrefetchManager::new(),
            priority_queue: PriorityQueue::new(DEFAULT_MAX_CONCURRENT),
            client: reqwest::Client::new(),
        })
    }

    pub fn cookie_jar(&self) -> &CookieJar {
        &self.cookies
    }

    /// Clone a handle to the cookie jar for sharing with the extension runtime.
    pub fn cookie_jar_arc(&self) -> Arc<CookieJar> {
        self.cookies.clone()
    }

    /// Access the underlying HTTP client.
    pub fn client(&self) -> &reqwest::Client {
        &self.client
    }

    pub async fn fetch(&self, request: Request) -> Result<Response, String> {
        let method = reqwest::Method::from_bytes(request.method.to_uppercase().as_bytes())
            .map_err(|_| format!("Invalid HTTP method: {}", request.method))?;
        let response = self
            .client
            .request(method, &request.url)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        let status = response.status().as_u16();
        let body = response.bytes().await.map_err(|e| e.to_string())?.to_vec();
        Ok(Response { status, body })
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
