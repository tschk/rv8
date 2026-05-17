//! JavaScript engine module - V8 based

#[cfg(feature = "rv8-v8")]
pub mod bindings;
#[cfg(feature = "rv8-v8")]
mod engine;
#[cfg(feature = "servo-render")]
pub mod soliloquy;
mod value;

#[cfg(feature = "rv8-v8")]
pub use engine::JsEngine;
pub use value::JsValue;
