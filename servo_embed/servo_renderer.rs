//! In-process Servo WebView + software GL readback for host embedders.

use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

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

pub struct ServoRenderer {
    servo: Servo,
    rendering_context: Rc<dyn RenderingContext>,
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

        let load_timeout = Duration::from_secs(180);
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
            .unwrap_or(3500);
        self.pump_for(Duration::from_millis(settle_ms));
        self.install_polyfills();
        if let Some(webview) = &self.webview {
            webview.paint();
        }
        let _ = self.pump_until(
            || self.frame_ready.load(Ordering::Relaxed),
            Duration::from_secs(45),
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

    pub fn handle_mouse_click_at(&mut self, x: f32, y: f32) {
        let script = build_click_script(x, y);
        let _ = self.evaluate_script_sync(&script);
        self.frame_ready.store(true, Ordering::Relaxed);
        self.tick();
    }

    pub fn title(&self) -> String {
        self.webview
            .as_ref()
            .and_then(|webview| webview.page_title())
            .unwrap_or_else(|| String::new())
    }

    pub fn evaluate_script_sync(&mut self, script: &str) -> Result<String, String> {
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
            if let Ok(result) = rx.try_recv() {
                return match result {
                    Ok(value) => Ok(js_value_to_string(&value)),
                    Err(err) => Err(format!("JavaScript evaluation failed: {err:?}")),
                };
            }
            if Instant::now() >= deadline {
                return Err("JavaScript evaluation timed out".to_string());
            }
            self.servo.spin_event_loop();
            thread::sleep(Duration::from_millis(1));
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
            if let Ok(result) = rx.try_recv() {
                return match result {
                    Ok(value) => Ok(js_value_from_embedder(&value)),
                    Err(err) => Err(format!("JavaScript evaluation failed: {err:?}")),
                };
            }
            if Instant::now() >= deadline {
                return Err("JavaScript evaluation timed out".to_string());
            }
            self.servo.spin_event_loop();
            thread::sleep(Duration::from_millis(1));
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
        let _ = self.pump_until(|| done.load(Ordering::Relaxed), Duration::from_secs(8));
        self.pump_for(Duration::from_millis(200));
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
    }

    #[test]
    #[ignore = "slow network integration; run with --ignored"]
    fn undivisible_dev_renders_non_error_page() {
        let mut renderer = ServoRenderer::new(1280, 800).expect("servo renderer");
        renderer
            .navigate("https://undivisible.dev")
            .expect("navigate undivisible.dev");
        let frame = renderer
            .capture_frame(1)
            .expect("frame after undivisible.dev");
        let dark_pixels = frame
            .pixels
            .chunks_exact(4)
            .filter(|px| px[0] < 48 && px[1] < 48 && px[2] < 48)
            .count();
        let total = frame.pixels.len() / 4;
        assert!(
            dark_pixels < total * 9 / 10,
            "expected mostly non-black pixels (got {dark_pixels}/{total} dark); likely error page"
        );
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
