//! WebExtensions API adapter for RV8.
//!
//! Provides Manifest V2/V3 parsing, permission modeling, extension lifecycle,
//! and a namespaced API dispatcher covering the major browser extension
//! namespaces (runtime, tabs, windows, storage, scripting, declarativeNetRequest,
//! bookmarks, history, downloads, notifications, etc.).

pub mod api;
pub mod manifest;
pub mod matchers;
pub mod permissions;
pub mod runtime;
pub mod storage;

pub use manifest::ExtensionManifest;
pub use permissions::{Permission, PermissionSet};
pub use runtime::{ExtensionId, ExtensionRuntime, ExtensionTab};
