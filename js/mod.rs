//! JavaScript engine module - V8 based

#[cfg(feature = "rv8-v8")]
pub mod bindings;
#[cfg(feature = "rv8-v8")]
mod engine;
#[cfg(feature = "rv8-v8")]
pub mod inspector;
#[cfg(feature = "rv8-v8")]
pub mod worker;
#[cfg(feature = "servo-render")]
pub mod soliloquy;
mod value;

#[cfg(feature = "rv8-v8")]
pub use engine::JsEngine;
#[cfg(feature = "rv8-v8")]
pub use inspector::{CdpSession, CdpSessionState};
#[cfg(feature = "rv8-v8")]
pub use worker::{V8Worker, WorkerMessage};
pub use value::JsValue;
