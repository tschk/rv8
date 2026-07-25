//! WebExtensions API dispatcher.
//!
//! `ApiRequest` carries a `namespace` and `method` plus JSON arguments.
//! Namespaces may contain a dot for events, e.g. `runtime.onMessage` or
//! `tabs.onCreated`. The dispatcher routes to per-namespace handlers in
//! `namespaces.rs`.

use serde_json::Value;

use super::runtime::{ExtensionId, ExtensionRuntime};

/// A request from an extension JavaScript context to the adapter.
#[derive(Debug, Clone)]
pub struct ApiRequest {
    pub namespace: String,
    pub method: String,
    pub args: Vec<Value>,
    pub extension_id: ExtensionId,
}

impl ApiRequest {
    pub fn new(namespace: &str, method: &str, extension_id: ExtensionId) -> Self {
        Self {
            namespace: namespace.to_string(),
            method: method.to_string(),
            args: Vec::new(),
            extension_id,
        }
    }

    pub fn with_args(mut self, args: Vec<Value>) -> Self {
        self.args = args;
        self
    }

    /// Convenience: first positional argument as JSON object.
    pub fn options(&self, idx: usize) -> Option<&serde_json::Map<String, Value>> {
        self.args.get(idx).and_then(|v| v.as_object())
    }

    /// Convenience: first positional argument as string.
    pub fn string_arg(&self, idx: usize) -> Option<&str> {
        self.args.get(idx).and_then(|v| v.as_str())
    }

    /// Convenience: argument as u64.
    pub fn u64_arg(&self, idx: usize) -> Option<u64> {
        self.args.get(idx).and_then(|v| v.as_u64())
    }

    /// Convenience: argument as i64.
    pub fn i64_arg(&self, idx: usize) -> Option<i64> {
        self.args.get(idx).and_then(|v| v.as_i64())
    }
}

pub type ApiResponse = Result<Value, String>;

/// Public dispatch entry used by `ExtensionRuntime::call_api`.
pub fn dispatch(runtime: &ExtensionRuntime, req: ApiRequest) -> ApiResponse {
    namespaces::handle(runtime, req)
}

mod namespaces;
