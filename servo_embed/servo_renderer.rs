//! In-process Servo WebView + software GL readback for host embedders.

use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

#[cfg(all(target_os = "macos", feature = "servo-render"))]
use std::cell::{Cell, RefCell};

#[cfg(all(target_os = "macos", feature = "servo-render"))]
use core_foundation::base::TCFType;
#[cfg(all(target_os = "macos", feature = "servo-render"))]
use euclid::Size2D;
#[cfg(all(target_os = "macos", feature = "servo-render"))]
use gleam::gl::{self as gl, Gl};
#[cfg(all(target_os = "macos", feature = "servo-render"))]
use glow::{Context as GlowContext, NativeFramebuffer};
#[cfg(all(target_os = "macos", feature = "servo-render"))]
#[allow(deprecated)]
use io_surface::IOSurface;

#[cfg(all(target_os = "macos", feature = "servo-render"))]
struct SendableIOSurface(IOSurface);

#[cfg(all(target_os = "macos", feature = "servo-render"))]
unsafe impl Send for SendableIOSurface {}
#[cfg(all(target_os = "macos", feature = "servo-render"))]
use surfman::{
    Connection, Context, ContextAttributeFlags, ContextAttributes, Device, GLApi, GLVersion,
    SurfaceAccess, SurfaceType,
};

use dpi::PhysicalSize;
use embedder_traits::{Scroll, WebViewPoint, WebViewVector};
use servo::{
    DeviceIntPoint, DeviceIntRect, DeviceIntSize, DevicePoint, DeviceVector2D, EventLoopWaker,
    LoadStatus, RenderingContext, Servo, ServoBuilder, SoftwareRenderingContext, WebView,
    WebViewBuilder, WebViewDelegate,
};
use url::Url;

use embedder_traits::JSValue;

use super::embedder_polyfills;
use crate::js::soliloquy::ensure_soliloquy_v8_selected;
use crate::js::JsValue;
use crate::renderer::RenderFrame;

struct EmbedderDelegate {
    frame_ready: Arc<AtomicBool>,
    load_complete: Arc<AtomicBool>,
    head_parsed: Arc<AtomicBool>,
}

impl WebViewDelegate for EmbedderDelegate {
    fn notify_new_frame_ready(&self, webview: WebView) {
        self.frame_ready.store(true, Ordering::Relaxed);
        webview.paint();
    }

    fn notify_load_status_changed(&self, _webview: WebView, status: LoadStatus) {
        if status == LoadStatus::HeadParsed {
            self.head_parsed.store(true, Ordering::Relaxed);
        }
        if status == LoadStatus::Complete {
            self.load_complete.store(true, Ordering::Relaxed);
        }
    }
}

struct EventLoopWakerImpl(Arc<AtomicBool>);

impl EventLoopWaker for EventLoopWakerImpl {
    fn clone_box(&self) -> Box<dyn EventLoopWaker> {
        Box::new(EventLoopWakerImpl(self.0.clone()))
    }

    fn wake(&self) {
        self.0.store(true, Ordering::Relaxed);
    }
}

#[cfg(all(target_os = "macos", feature = "servo-render"))]
struct NativeSurfaceRenderingContext {
    size: Cell<PhysicalSize<u32>>,
    device: RefCell<Device>,
    context: RefCell<Context>,
    gleam_gl: Rc<dyn Gl>,
    glow_gl: Arc<GlowContext>,
}

#[cfg(all(target_os = "macos", feature = "servo-render"))]
impl NativeSurfaceRenderingContext {
    fn new(size: PhysicalSize<u32>) -> Result<Self, surfman::Error> {
        if size.width == 0 || size.height == 0 {
            return Err(surfman::Error::Failed);
        }
        let connection = Connection::new()?;
        let adapter = connection.create_hardware_adapter()?;
        let device = connection.create_device(&adapter)?;
        let gl_api = device.gl_api();
        let version = match gl_api {
            GLApi::GLES => GLVersion { major: 3, minor: 0 },
            GLApi::GL => GLVersion { major: 3, minor: 2 },
        };
        let flags = ContextAttributeFlags::ALPHA
            | ContextAttributeFlags::DEPTH
            | ContextAttributeFlags::STENCIL;
        let context_descriptor =
            device.create_context_descriptor(&ContextAttributes { flags, version })?;
        let mut context = device.create_context(&context_descriptor, None)?;
        let gleam_gl = unsafe {
            match gl_api {
                GLApi::GL => gl::GlFns::load_with(|name| device.get_proc_address(&context, name)),
                GLApi::GLES => {
                    gl::GlesFns::load_with(|name| device.get_proc_address(&context, name))
                }
            }
        };
        let glow_gl = unsafe {
            Arc::new(GlowContext::from_loader_function(|name| {
                device.get_proc_address(&context, name)
            }))
        };
        let surfman_size = Size2D::new(size.width as i32, size.height as i32);
        let surface = device.create_surface(
            &context,
            SurfaceAccess::GPUOnly,
            SurfaceType::Generic { size: surfman_size },
        )?;
        device
            .bind_surface_to_context(&mut context, surface)
            .map_err(|(e, mut s)| {
                let _ = device.destroy_surface(&mut context, &mut s);
                e
            })?;
        device.make_context_current(&context)?;
        Ok(Self {
            size: Cell::new(size),
            device: RefCell::new(device),
            context: RefCell::new(context),
            gleam_gl,
            glow_gl,
        })
    }

    fn framebuffer(&self) -> Option<NativeFramebuffer> {
        self.device
            .borrow()
            .context_surface_info(&self.context.borrow())
            .unwrap_or(None)
            .and_then(|info| info.framebuffer_object)
    }

    fn current_surface(&self) -> Option<IOSurface> {
        use objc2_core_foundation::CFRetained;
        let mut context = self.context.borrow_mut();
        let surface = self
            .device
            .borrow()
            .unbind_surface_from_context(&mut context)
            .ok()??;
        let native = self.device.borrow().native_surface(&surface);
        // surfman 0.13 now returns an objc2-io-surface CFRetained; convert to the
        // io_surface crate type that core-video 0.4 uses without double-releasing.
        let cloned = native.0.clone();
        let ptr = CFRetained::as_ptr(&cloned).as_ptr() as io_surface::IOSurfaceRef;
        let io_surface = unsafe { io_surface::IOSurface::wrap_under_create_rule(ptr) };
        std::mem::forget(cloned);
        let _ = self
            .device
            .borrow()
            .bind_surface_to_context(&mut context, surface);
        Some(io_surface)
    }
}

#[cfg(all(target_os = "macos", feature = "servo-render"))]
impl Drop for NativeSurfaceRenderingContext {
    fn drop(&mut self) {
        let device = &mut self.device.borrow_mut();
        let context = &mut self.context.borrow_mut();
        let _ = device.destroy_context(context);
    }
}

#[cfg(all(target_os = "macos", feature = "servo-render"))]
impl RenderingContext for NativeSurfaceRenderingContext {
    fn prepare_for_rendering(&self) {
        let framebuffer_id = self
            .framebuffer()
            .map_or(0, |framebuffer| framebuffer.0.into());
        self.gleam_gl
            .bind_framebuffer(gleam::gl::FRAMEBUFFER, framebuffer_id);
    }

    fn read_to_image(&self, source_rectangle: DeviceIntRect) -> Option<image::RgbaImage> {
        let framebuffer_id = self
            .framebuffer()
            .map_or(0, |framebuffer| framebuffer.0.into());
        self.gleam_gl
            .bind_framebuffer(gleam::gl::FRAMEBUFFER, framebuffer_id);
        self.gleam_gl.bind_vertex_array(0);
        let mut pixels = self.gleam_gl.read_pixels(
            source_rectangle.min.x,
            source_rectangle.min.y,
            source_rectangle.width(),
            source_rectangle.height(),
            gleam::gl::BGRA,
            gleam::gl::UNSIGNED_BYTE,
        );
        let gl_error = self.gleam_gl.get_error();
        if gl_error != gleam::gl::NO_ERROR {
            log::warn!("GL error code 0x{gl_error:x} set after read_pixels");
        }
        let source_rectangle = source_rectangle.to_usize();
        let orig_pixels = pixels.clone();
        let stride = source_rectangle.width() * 4;
        for y in 0..source_rectangle.height() {
            let dst_start = y * stride;
            let src_start = (source_rectangle.height() - y - 1) * stride;
            let src_slice = &orig_pixels[src_start..src_start + stride];
            pixels[dst_start..dst_start + stride].copy_from_slice(src_slice);
        }
        image::RgbaImage::from_raw(
            source_rectangle.width() as u32,
            source_rectangle.height() as u32,
            pixels,
        )
    }

    fn size(&self) -> PhysicalSize<u32> {
        self.size.get()
    }

    fn resize(&self, size: PhysicalSize<u32>) {
        if size.width == 0 || size.height == 0 || self.size.get() == size {
            return;
        }
        self.size.set(size);
        let _ = self.device.borrow().resize_bound_surface(
            &mut self.context.borrow_mut(),
            Size2D::new(size.width as i32, size.height as i32),
        );
    }

    fn present(&self) {
        self.gleam_gl.flush();
    }

    fn make_current(&self) -> Result<(), surfman::Error> {
        self.device
            .borrow()
            .make_context_current(&self.context.borrow())
    }

    fn gleam_gl_api(&self) -> Rc<dyn gleam::gl::Gl> {
        self.gleam_gl.clone()
    }

    fn glow_gl_api(&self) -> Arc<GlowContext> {
        self.glow_gl.clone()
    }

    fn connection(&self) -> Option<Connection> {
        Some(self.device.borrow().connection())
    }
}

pub struct ServoRenderer {
    servo: Servo,
    rendering_context: Rc<dyn RenderingContext>,
    #[cfg(all(target_os = "macos", feature = "servo-render"))]
    native_surface: Option<Rc<NativeSurfaceRenderingContext>>,
    webview: Option<WebView>,
    delegate: Rc<EmbedderDelegate>,
    width: u32,
    height: u32,
    frame_ready: Arc<AtomicBool>,
    load_complete: Arc<AtomicBool>,
    head_parsed: Arc<AtomicBool>,
    last_capture: Instant,
}

impl ServoRenderer {
    pub fn new(width: u32, height: u32) -> Result<Self, String> {
        ensure_soliloquy_v8_selected();
        let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

        let width = width.max(1);
        let height = height.max(1);
        let physical = PhysicalSize::new(width, height);

        #[cfg(all(target_os = "macos", feature = "servo-render"))]
        let (rendering_context, native_surface) =
            if let Ok(ctx) = NativeSurfaceRenderingContext::new(physical) {
                let ctx = Rc::new(ctx);
                (ctx.clone() as Rc<dyn RenderingContext>, Some(ctx))
            } else {
                let ctx = Rc::new(
                    SoftwareRenderingContext::new(physical)
                        .map_err(|e| format!("SoftwareRenderingContext: {e:?}"))?,
                );
                (ctx as Rc<dyn RenderingContext>, None)
            };

        #[cfg(not(all(target_os = "macos", feature = "servo-render")))]
        let rendering_context = Rc::new(
            SoftwareRenderingContext::new(physical)
                .map_err(|e| format!("SoftwareRenderingContext: {e:?}"))?,
        );

        rendering_context
            .make_current()
            .map_err(|e| format!("make_current: {e:?}"))?;

        let woken = Arc::new(AtomicBool::new(false));
        let mut preferences = servo::Preferences::default();
        preferences.network_http_proxy_uri.clear();
        preferences.network_https_proxy_uri.clear();
        preferences.viewport_meta_enabled = true;

        let servo = ServoBuilder::default()
            .preferences(preferences)
            .event_loop_waker(Box::new(EventLoopWakerImpl(woken)))
            .build();

        let frame_ready = Arc::new(AtomicBool::new(false));
        let load_complete = Arc::new(AtomicBool::new(false));
        let head_parsed = Arc::new(AtomicBool::new(false));
        let delegate = Rc::new(EmbedderDelegate {
            frame_ready: frame_ready.clone(),
            load_complete: load_complete.clone(),
            head_parsed: head_parsed.clone(),
        });

        let renderer = ServoRenderer {
            servo,
            rendering_context,
            #[cfg(all(target_os = "macos", feature = "servo-render"))]
            native_surface,
            webview: None,
            delegate,
            width,
            height,
            frame_ready,
            load_complete,
            head_parsed,
            last_capture: Instant::now(),
        };
        Ok(renderer)
    }

    pub fn navigate(&mut self, url: &str) -> Result<(), String> {
        let parsed = Url::parse(url).map_err(|e| format!("invalid URL: {e}"))?;
        self.load_complete.store(false, Ordering::Relaxed);
        self.head_parsed.store(false, Ordering::Relaxed);
        self.frame_ready.store(false, Ordering::Relaxed);
        if let Some(webview) = &self.webview {
            webview.load(parsed);
        } else {
            self.webview = Some(
                WebViewBuilder::new(&self.servo, self.rendering_context.clone())
                    .delegate(self.delegate.clone())
                    .url(parsed)
                    .build(),
            );
        }
        if let Some(webview) = &self.webview {
            webview.show();
        }

        let load_timeout_secs = std::env::var("RV8_LOAD_TIMEOUT_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(if cfg!(test) { 15 } else { 180 });
        let load_timeout = Duration::from_secs(load_timeout_secs);
        let load_ok = self
            .pump_until(|| self.load_complete.load(Ordering::Relaxed), load_timeout)
            .is_ok();

        if !load_ok {
            eprintln!(
                "[rv8] load did not reach Complete within {:?}; continuing with partial render",
                load_timeout
            );
        }
        let settle_ms = std::env::var("RV8_SCRIPT_SETTLE_MS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(if cfg!(test) { 150 } else { 3500 });
        self.pump_for(Duration::from_millis(settle_ms));
        self.install_polyfills();
        if let Some(webview) = &self.webview {
            webview.paint();
        }
        let frame_timeout_secs = if cfg!(test) { 10 } else { 45 };
        let _ = self.pump_until(
            || self.frame_ready.load(Ordering::Relaxed),
            Duration::from_secs(frame_timeout_secs),
        );
        self.pump_for(Duration::from_millis(400));
        Ok(())
    }

    pub fn tick(&self) {
        for _ in 0..8 {
            self.servo.spin_event_loop();
        }
    }

    pub fn has_pending_frame(&self) -> bool {
        self.frame_ready.load(Ordering::Relaxed)
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.width = width.max(1);
        self.height = height.max(1);
        if let Some(webview) = &self.webview {
            webview.resize(PhysicalSize::new(self.width, self.height));
        }
        self.frame_ready.store(false, Ordering::Relaxed);
        self.pump_for(Duration::from_millis(50));
    }

    /// Forward wheel/trackpad scroll into Servo (device pixels; positive `delta_y` scrolls down).
    pub fn scroll_by(&mut self, delta_x: f32, delta_y: f32) {
        let vector = WebViewVector::Device(DeviceVector2D::new(delta_x, delta_y));
        let center = DevicePoint::new(
            (self.width as f32 * 0.5).round(),
            (self.height as f32 * 0.5).round(),
        );
        if let Some(webview) = &self.webview {
            webview.notify_scroll_event(Scroll::Delta(vector), WebViewPoint::Device(center));
            self.frame_ready.store(true, Ordering::Relaxed);
            self.tick();
        }
    }

    pub fn handle_focus(&mut self, focused: bool) {
        if let Some(webview) = &self.webview {
            if focused {
                webview.focus();
            } else {
                webview.blur();
            }
            self.frame_ready.store(true, Ordering::Relaxed);
            self.tick();
        }
    }

    pub fn go_back(&mut self) {
        if let Some(webview) = &self.webview {
            webview.go_back(1);
            self.frame_ready.store(true, Ordering::Relaxed);
            self.tick();
        }
    }

    pub fn go_forward(&mut self) {
        if let Some(webview) = &self.webview {
            webview.go_forward(1);
            self.frame_ready.store(true, Ordering::Relaxed);
            self.tick();
        }
    }

    pub fn handle_mouse_move(&mut self, x: f32, y: f32) {
        use embedder_traits::{InputEvent, MouseMoveEvent, WebViewPoint};
        use servo::DevicePoint;

        if let Some(webview) = &self.webview {
            let point = WebViewPoint::Device(DevicePoint::new(x, y));
            webview.notify_input_event(InputEvent::MouseMove(MouseMoveEvent::new(point)));
            self.tick();
        }
    }

    pub fn handle_mouse_click_at(&mut self, x: f32, y: f32) {
        let script = build_click_script(x, y);
        let _ = self.evaluate_script_sync(&script);
        self.frame_ready.store(true, Ordering::Relaxed);
        self.tick();
    }

    pub fn handle_key_event(&mut self, event: keyboard_types::KeyboardEvent) {
        use embedder_traits::{InputEvent, KeyboardEvent};
        if let Some(webview) = &self.webview {
            webview.notify_input_event(InputEvent::Keyboard(KeyboardEvent::new(event)));
            self.frame_ready.store(true, Ordering::Relaxed);
            self.tick();
        }
    }

    #[cfg(all(target_os = "macos", feature = "servo-render"))]
    pub fn current_surface(&self) -> Option<IOSurface> {
        self.native_surface.as_ref()?.current_surface()
    }

    pub fn title(&self) -> String {
        self.webview
            .as_ref()
            .and_then(|webview| webview.page_title())
            .unwrap_or_default()
    }

    /// Evaluate JavaScript and return the result as a string.
    /// Scripts run in Servo's default evaluation scope (may lack window globals).
    pub fn evaluate_script_sync(&mut self, script: &str) -> Result<String, String> {
        self.evaluate_raw(script)
    }

    /// Evaluate JavaScript in the page's window scope by resolving globals
    /// from `globalThis`. This fixes `typeof document`, `window`, `navigator`, `fetch`
    /// returning `undefined` in Servo's default evaluation scope.
    pub fn evaluate_in_page_scope(&mut self, script: &str) -> Result<String, String> {
        // ponytail: assign globals from globalThis to local vars so bare `document`,
        // `window`, `navigator`, `fetch` resolve. Fallback to raw if wrapper fails.
        let wrapped = format!(
            "(function(){{var w=globalThis,d=w.document,l=w.location,n=w.navigator,f=w.fetch;return eval({});}}).call(globalThis)",
            serde_json::to_string(script).unwrap_or_else(|_| format!("{:?}", script))
        );
        match self.evaluate_raw(&wrapped) {
            Ok(v) => Ok(v),
            Err(e) => {
                log::warn!("page-scope eval fallback to raw: {e}");
                self.evaluate_raw(script)
            }
        }
    }

    fn evaluate_raw(&mut self, script: &str) -> Result<String, String> {
        use std::sync::mpsc;

        let (tx, rx) = mpsc::channel();
        let Some(webview) = &self.webview else {
            return Err("JavaScript evaluation requested before navigation".to_string());
        };
        webview.evaluate_javascript(script, move |result| {
            let _ = tx.send(result);
        });
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            self.servo.spin_event_loop();
            match rx.recv_timeout(Duration::from_millis(1)) {
                Ok(result) => {
                    return match result {
                        Ok(value) => Ok(js_value_to_string(&value)),
                        Err(err) => Err(format!("JavaScript evaluation failed: {err:?}")),
                    };
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    if Instant::now() >= deadline {
                        return Err("JavaScript evaluation timed out".to_string());
                    }
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    return Err("JavaScript evaluation failed: channel disconnected".to_string());
                }
            }
        }
    }

    pub fn evaluate_script_value_sync(&mut self, script: &str) -> Result<JsValue, String> {
        use std::sync::mpsc;

        let (tx, rx) = mpsc::channel();
        let Some(webview) = &self.webview else {
            return Err("JavaScript evaluation requested before navigation".to_string());
        };
        webview.evaluate_javascript(script, move |result| {
            let _ = tx.send(result);
        });
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            self.servo.spin_event_loop();
            match rx.recv_timeout(Duration::from_millis(1)) {
                Ok(result) => {
                    return match result {
                        Ok(value) => Ok(js_value_from_embedder(&value)),
                        Err(err) => Err(format!("JavaScript evaluation failed: {err:?}")),
                    };
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    if Instant::now() >= deadline {
                        return Err("JavaScript evaluation timed out".to_string());
                    }
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    return Err("JavaScript evaluation failed: channel disconnected".to_string());
                }
            }
        }
    }

    pub fn capture_frame(&mut self, generation: u64) -> Option<RenderFrame> {
        let min_interval = Duration::from_millis(33);
        let pending = self.frame_ready.load(Ordering::Relaxed);
        if !pending && self.last_capture.elapsed() < min_interval {
            return None;
        }

        if pending {
            self.tick();
        } else {
            self.pump_for(Duration::from_millis(8));
        }
        let webview = self.webview.as_ref()?;
        if !self.frame_ready.load(Ordering::Relaxed) {
            webview.paint();
        }
        let rect = DeviceIntRect::from_origin_and_size(
            DeviceIntPoint::zero(),
            DeviceIntSize::new(self.width as i32, self.height as i32),
        );
        self.frame_ready.store(false, Ordering::Relaxed);
        self.last_capture = Instant::now();
        let image = self.rendering_context.read_to_image(rect)?;

        let mut frame = RenderFrame::new(self.width, self.height);
        frame.id = generation;
        let img_w = image.width();
        let img_h = image.height();
        let rgba = image.into_raw();
        if img_w == self.width && img_h == self.height && rgba.len() == frame.pixels.len() {
            frame.pixels = rgba;
        } else {
            let copy_len = frame.pixels.len().min(rgba.len());
            frame.pixels[..copy_len].copy_from_slice(&rgba[..copy_len]);
        }
        Some(frame)
    }

    fn install_polyfills(&mut self) {
        let done = Arc::new(AtomicBool::new(false));
        let done_cb = done.clone();
        if let Some(webview) = &self.webview {
            webview.evaluate_javascript(embedder_polyfills::SCRIPT, move |result| {
                if let Err(err) = result {
                    eprintln!("[rv8] embedder polyfill injection failed: {err:?}");
                }
                done_cb.store(true, Ordering::Relaxed);
            });
        }
        let polyfill_timeout_secs = if cfg!(test) { 3 } else { 8 };
        let _ = self.pump_until(
            || done.load(Ordering::Relaxed),
            Duration::from_secs(polyfill_timeout_secs),
        );
        self.pump_for(Duration::from_millis(if cfg!(test) { 50 } else { 200 }));
    }

    fn pump_until(&self, done: impl Fn() -> bool, timeout: Duration) -> Result<(), String> {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            if done() {
                return Ok(());
            }
            self.servo.spin_event_loop();
            thread::sleep(Duration::from_millis(1));
        }
        Err("Servo navigation timed out".to_string())
    }

    fn pump_for(&self, duration: Duration) {
        let deadline = Instant::now() + duration;
        while Instant::now() < deadline {
            self.servo.spin_event_loop();
            thread::sleep(Duration::from_millis(1));
        }
    }
}

fn js_value_to_string(value: &JSValue) -> String {
    match value {
        JSValue::Undefined => "undefined".to_string(),
        JSValue::Null => "null".to_string(),
        JSValue::Boolean(v) => v.to_string(),
        JSValue::Number(n) => n.to_string(),
        JSValue::String(s) => s.clone(),
        JSValue::Element(_)
        | JSValue::ShadowRoot(_)
        | JSValue::Frame(_)
        | JSValue::Window(_)
        | JSValue::Object(_)
        | JSValue::Array(_) => "[object]".to_string(),
    }
}

fn js_value_from_embedder(value: &JSValue) -> JsValue {
    match value {
        JSValue::Undefined => JsValue::Undefined,
        JSValue::Null => JsValue::Null,
        JSValue::Boolean(v) => JsValue::Boolean(*v),
        JSValue::Number(n) => JsValue::Number(*n),
        JSValue::String(s) => JsValue::String(s.clone()),
        JSValue::Element(_)
        | JSValue::ShadowRoot(_)
        | JSValue::Frame(_)
        | JSValue::Window(_)
        | JSValue::Object(_) => JsValue::Object,
        JSValue::Array(_) => JsValue::Array,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_page_renders_with_layout() {
        let mut renderer = ServoRenderer::new(800, 600).expect("servo renderer");
        renderer
            .navigate("data:text/html,<html><body><h1>Hello%20RV8</h1><p>Visible%20content</p></body></html>")
            .expect("navigate data page");
        let frame = renderer.capture_frame(1).expect("frame");
        assert_eq!(frame.width, 800);
        assert_eq!(frame.height, 600);
        let non_white = frame
            .pixels
            .chunks_exact(4)
            .filter(|px| px[0] <= 245 || px[1] <= 245 || px[2] <= 245)
            .count();
        assert!(
            non_white > frame.pixels.len() / 4000,
            "expected visible non-white page pixels"
        );
        assert_eq!(
            renderer
                .evaluate_script_sync("document.readyState")
                .expect("evaluate realm smoke"),
            "complete"
        );
    }

    /// Probe a page's HTML depth and JS capability.
    /// Not a pass/fail test — it inspects what Servo receives and dumps diagnostics.
    pub fn diagnose_page(renderer: &mut ServoRenderer, label: &str) {
        // Phase 1: ES syntax baseline — no optional chaining, no destructuring
        // to rule out parser issues
        let raw_checks = [
            ("typeof document !== 'undefined'", "has_document"),
            ("typeof document.body", "typeof_body"),
            ("typeof document.documentElement", "typeof_htmlel"),
            ("typeof document.getElementById", "typeof_getId"),
            ("typeof document.querySelector", "typeof_qs"),
            ("typeof document.querySelectorAll", "typeof_qsa"),
            ("typeof navigator", "typeof_nav"),
            ("typeof window", "typeof_win"),
            ("typeof window.innerWidth", "typeof_iw"),
            ("typeof window.innerHeight", "typeof_ih"),
            ("typeof document.cookie", "typeof_cookie"),
            ("typeof fetch", "typeof_fetch"),
            ("typeof Promise === 'function'", "has_Promise"),
            ("typeof Symbol === 'function'", "has_Symbol"),
            ("'serviceWorker' in window", "has_sw"),
            ("typeof Proxy === 'function'", "has_Proxy"),
        ];
        for (script, key) in &raw_checks {
            match renderer.evaluate_script_sync(script) {
                Ok(v) => println!("  [{label}] {key} = {v}"),
                Err(e) => println!("  [{label}] {key} = ERR: {e}"),
            }
        }
        // Phase 2: test new evaluate_in_page_scope fix
        println!("  [{label}] --- evaluate_in_page_scope ---");
        let fixed = [
            ("document.title", "doc_title_fixed"),
            ("typeof document", "typeof_doc_fixed"),
            ("typeof window", "typeof_win_fixed"),
            ("typeof navigator", "typeof_nav_fixed"),
            ("typeof location", "typeof_loc_fixed"),
            ("typeof fetch", "typeof_fetch_fixed"),
            ("typeof globalThis", "typeof_global_fixed"),
            ("document.URL.substring(0, 50)", "doc_url_fixed"),
        ];
        for (script, key) in &fixed {
            match renderer.evaluate_in_page_scope(script) {
                Ok(v) => println!("  [{label}] {key} = {v}"),
                Err(e) => println!("  [{label}] {key} = ERR: {e}"),
            }
        }

        // Phase 3: raw value tests — what does Servo actually return?
        let dom_checks = [
            ("document.title", "doc_title_raw"),
            ("typeof document", "typeof_doc"),
            ("typeof window", "typeof_win2"),
            ("typeof location", "typeof_loc"),
            ("typeof navigator", "typeof_nav2"),
            ("typeof this", "typeof_this"),
            ("this === undefined", "this_is_undef"),
            ("typeof globalThis", "typeof_global"),
            ("String(1+1)", "str_2"),
            ("typeof (1+1)", "typeof_2"),
        ];
        for (script, key) in &dom_checks {
            match renderer.evaluate_script_sync(script) {
                Ok(v) => println!("  [{label}] {key} = {v}"),
                Err(e) => println!("  [{label}] {key} = ERR: {e}"),
            }
        }
    }

    #[test]
    #[ignore = "diagnose google.com rendering"]
    fn diagnose_google() {
        let mut renderer = ServoRenderer::new(1280, 800).expect("servo renderer");
        renderer
            .navigate("https://google.com")
            .expect("navigate google.com");
        diagnose_page(&mut renderer, "google");
        let frame = renderer.capture_frame(1).expect("frame");
        let total = frame.pixels.len() / 4;
        let rgba = &frame.pixels;
        let mut color_buckets: std::collections::HashMap<u32, u32> =
            std::collections::HashMap::new();
        for px in rgba.chunks_exact(4) {
            let key = ((px[0] as u32) << 16) | ((px[1] as u32) << 8) | (px[2] as u32);
            *color_buckets.entry(key >> 4).or_insert(0) += 1; // bucket by upper 4 bits per channel
        }
        let total_mapped: u32 = color_buckets.values().sum();
        println!(
            "  [google] frame {}x{} = {} pixels, {} unique color buckets",
            frame.width,
            frame.height,
            total,
            color_buckets.len()
        );
        let mut sorted: Vec<_> = color_buckets.into_iter().collect();
        sorted.sort_by_key(|&(_, count)| std::cmp::Reverse(count));
        for (color_bucket, count) in sorted.iter().take(12) {
            let r = (color_bucket >> 16) << 4;
            let g = ((color_bucket >> 8) & 0xFF) << 4;
            let b = (color_bucket & 0xFF) << 4;
            let pct = (*count as f64) / (total_mapped as f64) * 100.0;
            println!("  [google]   {:.1}%  rgb({r},{g},{b})", pct,);
        }
    }

    #[test]
    #[ignore = "google.com — run with --ignored to check google rendering"]
    fn google_com_renders_homepage() {
        let mut renderer = ServoRenderer::new(1280, 800).expect("servo renderer");
        renderer
            .navigate("https://google.com")
            .expect("navigate google.com");
        assert_eq!(
            renderer
                .evaluate_script_sync("document.readyState")
                .expect("ready state"),
            "complete"
        );
        let title = renderer
            .evaluate_script_sync("document.title")
            .expect("title");
        assert!(
            title.contains("Google"),
            "expected 'Google' in title, got: {}",
            title
        );
        let frame = renderer.capture_frame(1).expect("frame after google.com");
        let total = frame.pixels.len() / 4;
        let non_white = frame
            .pixels
            .chunks_exact(4)
            .filter(|px| px[0] > 240 && px[1] > 240 && px[2] > 240)
            .count();
        let colored = total - non_white;
        assert!(
            colored > 50,
            "expected visible colored pixels (Google logo/buttons), got {colored}/{total}"
        );
    }
}

// ── Thread-safe ServoHost ──
// ServoRenderer uses Rc internally (!Send). ServoHost spawns it on a
// dedicated OS thread so the handle is Send + Sync for gpui/async use.

use std::sync::mpsc;

/// Result from a Servo navigation command.
pub struct ServoHostResult {
    pub title: String,
    pub frame: Option<RenderFrame>,
}

enum ServoCmd {
    Navigate {
        url: String,
        tx: mpsc::Sender<Result<ServoHostResult, String>>,
    },
    MouseMove {
        x: f32,
        y: f32,
    },
    MouseClick {
        x: f32,
        y: f32,
    },
    Scroll {
        delta_x: f32,
        delta_y: f32,
    },
    KeyEvent {
        event: keyboard_types::KeyboardEvent,
    },
    CaptureFrame {
        tx: mpsc::Sender<Option<RenderFrame>>,
    },
    #[cfg(target_os = "macos")]
    CurrentSurface {
        tx: mpsc::Sender<Option<SendableIOSurface>>,
    },
    Shutdown,
}

/// Thread-safe handle to a Servo renderer on a dedicated thread.
/// Clone to upgrade to multiple command producers.
#[derive(Clone)]
pub struct ServoHost {
    tx: mpsc::Sender<ServoCmd>,
}

impl ServoHost {
    /// Launch a Servo renderer thread and return a handle.
    pub fn launch(width: u32, height: u32) -> Result<Self, String> {
        let (tx, rx) = mpsc::channel::<ServoCmd>();
        let handle = Self { tx };
        let rx_handle = rx;
        // ponytail: spawn a dedicated thread; Servo's Rc types live here.
        thread::Builder::new()
            .name("servo-renderer".into())
            .spawn(move || {
                let mut renderer = match ServoRenderer::new(width, height) {
                    Ok(r) => r,
                    Err(e) => {
                        log::error!("ServoHost: init failed: {e}");
                        return;
                    }
                };
                while let Ok(cmd) = rx_handle.recv() {
                    match cmd {
                        ServoCmd::Shutdown => break,
                        ServoCmd::Navigate { url, tx } => {
                            let result = (|| -> Result<ServoHostResult, String> {
                                renderer.navigate(&url)?;
                                let title = renderer.title();
                                let frame = renderer.capture_frame(1);
                                Ok(ServoHostResult { title, frame })
                            })();
                            let _ = tx.send(result);
                        }
                        ServoCmd::MouseMove { x, y } => {
                            renderer.handle_mouse_move(x, y);
                        }
                        ServoCmd::MouseClick { x, y } => {
                            renderer.handle_mouse_click_at(x, y);
                        }
                        ServoCmd::Scroll { delta_x, delta_y } => {
                            renderer.scroll_by(delta_x, delta_y);
                        }
                        ServoCmd::KeyEvent { event } => {
                            renderer.handle_key_event(event);
                        }
                        ServoCmd::CaptureFrame { tx } => {
                            let _ = tx.send(renderer.capture_frame(0));
                        }
                        #[cfg(target_os = "macos")]
                        ServoCmd::CurrentSurface { tx } => {
                            let _ = tx.send(renderer.current_surface().map(SendableIOSurface));
                        }
                    }
                }
                log::info!("ServoHost: thread exiting");
            })
            .map_err(|e| format!("ServoHost thread spawn: {e}"))?;
        Ok(handle)
    }

    /// Navigate to a URL and wait for the result.
    pub fn navigate(&self, url: &str) -> Result<ServoHostResult, String> {
        let (tx, rx) = mpsc::channel();
        self.tx
            .send(ServoCmd::Navigate {
                url: url.to_string(),
                tx,
            })
            .map_err(|e| format!("ServoHost send: {e}"))?;
        rx.recv().map_err(|e| format!("ServoHost recv: {e}"))?
    }

    pub fn handle_mouse_move(&self, x: f32, y: f32) {
        let _ = self.tx.send(ServoCmd::MouseMove { x, y });
    }

    pub fn handle_mouse_click_at(&self, x: f32, y: f32) {
        let _ = self.tx.send(ServoCmd::MouseClick { x, y });
    }

    pub fn scroll_by(&self, delta_x: f32, delta_y: f32) {
        let _ = self.tx.send(ServoCmd::Scroll { delta_x, delta_y });
    }

    pub fn handle_key_event(&self, event: keyboard_types::KeyboardEvent) {
        let _ = self.tx.send(ServoCmd::KeyEvent { event });
    }

    pub fn capture_frame(&self) -> Result<Option<RenderFrame>, String> {
        let (tx, rx) = mpsc::channel();
        self.tx
            .send(ServoCmd::CaptureFrame { tx })
            .map_err(|e| format!("ServoHost send: {e}"))?;
        rx.recv().map_err(|e| format!("ServoHost recv: {e}"))
    }

    #[cfg(target_os = "macos")]
    pub fn current_surface(&self) -> Result<Option<IOSurface>, String> {
        let (tx, rx) = mpsc::channel();
        self.tx
            .send(ServoCmd::CurrentSurface { tx })
            .map_err(|e| format!("ServoHost send: {e}"))?;
        rx.recv()
            .map_err(|e| format!("ServoHost recv: {e}"))?
            .map(|s| Ok(Some(s.0)))
            .unwrap_or(Ok(None))
    }

    /// Shut down the renderer thread.
    pub fn shutdown(&self) {
        let _ = self.tx.send(ServoCmd::Shutdown);
    }
}

fn build_click_script(x: f32, y: f32) -> String {
    let mut s = String::with_capacity(256);
    s.push_str("var e=document.elementFromPoint(");
    s.push_str(&x.to_string());
    s.push(',');
    s.push_str(&y.to_string());
    s.push_str(");if(e){e.click();e.dispatchEvent(new MouseEvent('click',{bubbles:true,cancelable:true,clientX:");
    s.push_str(&x.to_string());
    s.push_str(",clientY:");
    s.push_str(&y.to_string());
    s.push_str("}));}");
    s
}
