//! WebSocket connection management with simulated transport.
//!
//! Provides a state machine and thread-safe frame queues. Real network I/O
//! can be wired in later without changing the public API surface.

use log::info;
use parking_lot::Mutex;
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

static NEXT_CONNECTION_ID: AtomicU64 = AtomicU64::new(1);

/// WebSocket connection lifecycle state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebSocketState {
    Connecting,
    Open,
    Closing,
    Closed,
}

/// A decoded WebSocket frame payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WebSocketFrame {
    Text(String),
    Binary(Vec<u8>),
    Ping(Vec<u8>),
    Pong(Vec<u8>),
    Close(Option<u16>, Option<String>),
}

struct ConnectionInner {
    id: u64,
    url: String,
    state: WebSocketState,
    outbound: VecDeque<WebSocketFrame>,
    inbound: VecDeque<WebSocketFrame>,
}

/// A single WebSocket connection with thread-safe send and receive queues.
#[derive(Clone)]
pub struct WebSocketConnection {
    inner: Arc<Mutex<ConnectionInner>>,
}

impl WebSocketConnection {
    /// Create a connection in the `Connecting` state for the given URL.
    pub fn new(url: impl Into<String>) -> Self {
        let id = NEXT_CONNECTION_ID.fetch_add(1, Ordering::Relaxed);
        let url = url.into();
        info!("WebSocket connection {} created for {}", id, url);
        Self {
            inner: Arc::new(Mutex::new(ConnectionInner {
                id,
                url,
                state: WebSocketState::Connecting,
                outbound: VecDeque::new(),
                inbound: VecDeque::new(),
            })),
        }
    }

    /// Simulate completing the WebSocket handshake and transition to `Open`.
    pub fn connect(&self) -> Result<(), String> {
        let mut inner = self.inner.lock();
        match inner.state {
            WebSocketState::Connecting => {
                inner.state = WebSocketState::Open;
                info!("WebSocket connection {} opened for {}", inner.id, inner.url);
                Ok(())
            }
            WebSocketState::Open => Ok(()),
            WebSocketState::Closing => Err(format!(
                "WebSocket connection {} is closing and cannot connect",
                inner.id
            )),
            WebSocketState::Closed => Err(format!(
                "WebSocket connection {} is closed and cannot connect",
                inner.id
            )),
        }
    }

    /// Queue an outbound frame. Ping frames are echoed with a simulated Pong.
    pub fn send_frame(&self, frame: WebSocketFrame) -> Result<(), String> {
        let mut inner = self.inner.lock();
        if inner.state != WebSocketState::Open {
            return Err(format!(
                "WebSocket connection {} is not open (state: {:?})",
                inner.id, inner.state
            ));
        }

        if let WebSocketFrame::Ping(payload) = &frame {
            inner.inbound.push_back(WebSocketFrame::Pong(payload.clone()));
        }

        inner.outbound.push_back(frame);
        Ok(())
    }

    /// Receive the next inbound frame, if any.
    pub fn recv_frame(&self) -> Option<WebSocketFrame> {
        self.inner.lock().inbound.pop_front()
    }

    /// Drain all currently queued inbound frames.
    pub fn poll_frames(&self) -> Vec<WebSocketFrame> {
        let mut inner = self.inner.lock();
        inner.inbound.drain(..).collect()
    }

    /// Begin closing the connection with an optional status code and reason.
    pub fn close(&self, code: Option<u16>, reason: Option<String>) -> Result<(), String> {
        let mut inner = self.inner.lock();
        match inner.state {
            WebSocketState::Open => {
                inner.state = WebSocketState::Closing;
                inner
                    .outbound
                    .push_back(WebSocketFrame::Close(code, reason.clone()));
                inner
                    .inbound
                    .push_back(WebSocketFrame::Close(code, reason));
                inner.state = WebSocketState::Closed;
                info!("WebSocket connection {} closed", inner.id);
                Ok(())
            }
            WebSocketState::Closing | WebSocketState::Closed => Ok(()),
            WebSocketState::Connecting => Err(format!(
                "WebSocket connection {} is not connected",
                inner.id
            )),
        }
    }

    /// Current connection state.
    pub fn state(&self) -> WebSocketState {
        self.inner.lock().state
    }

    /// Unique connection identifier.
    pub fn id(&self) -> u64 {
        self.inner.lock().id
    }

    /// Target URL for this connection.
    pub fn url(&self) -> String {
        self.inner.lock().url.clone()
    }

    /// Drain all queued outbound frames (for simulated transport layers).
    pub fn poll_outbound(&self) -> Vec<WebSocketFrame> {
        let mut inner = self.inner.lock();
        inner.outbound.drain(..).collect()
    }

    /// Push an inbound frame (for simulated transport layers).
    pub fn push_inbound(&self, frame: WebSocketFrame) -> Result<(), String> {
        let mut inner = self.inner.lock();
        if matches!(
            inner.state,
            WebSocketState::Open | WebSocketState::Closing
        ) {
            if matches!(frame, WebSocketFrame::Close(..)) {
                inner.state = WebSocketState::Closed;
            }
            inner.inbound.push_back(frame);
            Ok(())
        } else {
            Err(format!(
                "WebSocket connection {} cannot receive frames in state {:?}",
                inner.id, inner.state
            ))
        }
    }
}

/// Tracks active WebSocket connections by id.
pub struct WebSocketManager {
    connections: Mutex<HashMap<u64, WebSocketConnection>>,
}

impl WebSocketManager {
    pub fn new() -> Self {
        Self {
            connections: Mutex::new(HashMap::new()),
        }
    }

    /// Create and register a new connection.
    pub fn create(&self, url: &str) -> WebSocketConnection {
        let connection = WebSocketConnection::new(url);
        let id = connection.id();
        self.connections.lock().insert(id, connection.clone());
        connection
    }

    /// Look up a connection by id.
    pub fn get(&self, id: u64) -> Option<WebSocketConnection> {
        self.connections.lock().get(&id).cloned()
    }

    /// Remove a connection from the manager.
    pub fn remove(&self, id: u64) -> Option<WebSocketConnection> {
        self.connections.lock().remove(&id)
    }

    /// List ids for all tracked connections.
    pub fn ids(&self) -> Vec<u64> {
        self.connections.lock().keys().copied().collect()
    }

    /// Number of tracked connections.
    pub fn len(&self) -> usize {
        self.connections.lock().len()
    }

    /// Returns true when no connections are tracked.
    pub fn is_empty(&self) -> bool {
        self.connections.lock().is_empty()
    }
}

impl Default for WebSocketManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_connection_starts_connecting() {
        let conn = WebSocketConnection::new("wss://example.com/ws");
        assert_eq!(conn.state(), WebSocketState::Connecting);
        assert_eq!(conn.url(), "wss://example.com/ws");
        assert!(conn.id() > 0);
    }

    #[test]
    fn connect_transitions_to_open() {
        let conn = WebSocketConnection::new("wss://example.com/ws");
        conn.connect().expect("connect");
        assert_eq!(conn.state(), WebSocketState::Open);
        conn.connect().expect("idempotent connect");
        assert_eq!(conn.state(), WebSocketState::Open);
    }

    #[test]
    fn send_and_recv_text_frame() {
        let conn = WebSocketConnection::new("wss://example.com/ws");
        conn.connect().expect("connect");

        conn.send_frame(WebSocketFrame::Text("hello".into()))
            .expect("send");
        let outbound = conn.poll_outbound();
        assert_eq!(outbound, vec![WebSocketFrame::Text("hello".into())]);

        conn.push_inbound(WebSocketFrame::Text("world".into()))
            .expect("push");
        assert_eq!(
            conn.recv_frame(),
            Some(WebSocketFrame::Text("world".into()))
        );
    }

    #[test]
    fn ping_generates_simulated_pong() {
        let conn = WebSocketConnection::new("wss://example.com/ws");
        conn.connect().expect("connect");

        conn.send_frame(WebSocketFrame::Ping(vec![1, 2, 3]))
            .expect("send ping");
        assert_eq!(
            conn.recv_frame(),
            Some(WebSocketFrame::Pong(vec![1, 2, 3]))
        );
    }

    #[test]
    fn send_fails_when_not_open() {
        let conn = WebSocketConnection::new("wss://example.com/ws");
        let err = conn
            .send_frame(WebSocketFrame::Text("nope".into()))
            .expect_err("must fail");
        assert!(err.contains("not open"));
    }

    #[test]
    fn close_transitions_to_closed() {
        let conn = WebSocketConnection::new("wss://example.com/ws");
        conn.connect().expect("connect");

        conn.close(Some(1000), Some("bye".into()))
            .expect("close");
        assert_eq!(conn.state(), WebSocketState::Closed);

        let frames = conn.poll_frames();
        assert_eq!(
            frames,
            vec![WebSocketFrame::Close(
                Some(1000),
                Some("bye".into())
            )]
        );
    }

    #[test]
    fn poll_frames_drains_inbound_queue() {
        let conn = WebSocketConnection::new("wss://example.com/ws");
        conn.connect().expect("connect");

        conn.push_inbound(WebSocketFrame::Binary(vec![1]))
            .expect("push");
        conn.push_inbound(WebSocketFrame::Binary(vec![2]))
            .expect("push");

        let frames = conn.poll_frames();
        assert_eq!(
            frames,
            vec![
                WebSocketFrame::Binary(vec![1]),
                WebSocketFrame::Binary(vec![2]),
            ]
        );
        assert!(conn.poll_frames().is_empty());
    }

    #[test]
    fn manager_tracks_connections() {
        let manager = WebSocketManager::new();
        let a = manager.create("wss://a.example/ws");
        let b = manager.create("wss://b.example/ws");

        assert_eq!(manager.len(), 2);
        assert_ne!(a.id(), b.id());
        assert_eq!(manager.get(a.id()).unwrap().url(), "wss://a.example/ws");

        manager.remove(a.id());
        assert_eq!(manager.len(), 1);
        assert!(manager.get(a.id()).is_none());
        assert!(manager.get(b.id()).is_some());
    }

    #[test]
    fn inbound_close_frame_closes_connection() {
        let conn = WebSocketConnection::new("wss://example.com/ws");
        conn.connect().expect("connect");

        conn.push_inbound(WebSocketFrame::Close(None, None))
            .expect("push close");
        assert_eq!(conn.state(), WebSocketState::Closed);
    }
}
