//! Renderer module - Servo-based rendering
//!
//! Handles HTML/CSS parsing, layout, and painting.

mod process;
mod render_frame;

pub use process::RendererProcess;
pub use render_frame::RenderFrame;

/// Web view for rendering content
#[deprecated(note = "use the Servo embedder viewport path")]
pub struct WebView {
    // TODO: Implement WebView
}
