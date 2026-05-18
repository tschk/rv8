//! Host viewport: dedicated OS thread running Servo (no Tokio on the GL thread).

use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use super::servo_renderer::ServoRenderer;

#[derive(Clone, Debug, Default)]
pub struct ViewportSnapshot {
    pub url: String,
    pub title: String,
    pub width: u32,
    pub height: u32,
    pub loading: bool,
    pub pixels: Option<Vec<u8>>,
    pub frame_generation: u64,
    pub favicon: Option<Vec<u8>>,
    pub favicon_mime: Option<String>,
    pub find_matches: u32,
    pub find_active: u32,
    pub link_hover_url: Option<String>,
}

enum ViewportCmd {
    Navigate(String),
    Resize { width: u32, height: u32 },
    Scroll { delta_x: f32, delta_y: f32 },
    FindInPage { query: String, forward: bool },
    FindStop,
    Click { x: f32, y: f32 },
}

pub struct ServoViewport {
    snap: Arc<Mutex<ViewportSnapshot>>,
    tx: Sender<ViewportCmd>,
}

impl ServoViewport {
    pub fn open(width: u32, height: u32) -> Result<Self, String> {
        let snap = Arc::new(Mutex::new(ViewportSnapshot {
            width,
            height,
            ..Default::default()
        }));
        let (tx, rx) = mpsc::channel();
        let snap_bg = snap.clone();
        thread::Builder::new()
            .name("rv8-servo-viewport".into())
            .spawn(move || viewport_thread(snap_bg, rx, width, height))
            .map_err(|e| format!("spawn viewport thread: {e}"))?;
        Ok(ServoViewport { snap, tx })
    }

    pub fn snapshot(&self) -> ViewportSnapshot {
        self.snap.lock().expect("viewport snap").clone()
    }

    pub fn navigate(&self, url: impl Into<String>) {
        let _ = self.tx.send(ViewportCmd::Navigate(url.into()));
    }

    pub fn resize(&self, width: u32, height: u32) {
        let _ = self.tx.send(ViewportCmd::Resize { width, height });
    }

    pub fn scroll_by(&self, delta_x: f32, delta_y: f32) {
        let _ = self.tx.send(ViewportCmd::Scroll { delta_x, delta_y });
    }

    pub fn find_in_page(&self, query: &str, forward: bool) {
        let _ = self
            .tx
            .send(ViewportCmd::FindInPage { query: query.to_string(), forward });
    }

    pub fn find_stop(&self) {
        let _ = self.tx.send(ViewportCmd::FindStop);
    }

    pub fn click_at(&self, x: f32, y: f32) {
        let _ = self.tx.send(ViewportCmd::Click { x, y });
    }
}

fn viewport_thread(
    snap: Arc<Mutex<ViewportSnapshot>>,
    rx: Receiver<ViewportCmd>,
    mut width: u32,
    mut height: u32,
) {
    let mut renderer = match ServoRenderer::new(width, height) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("[rv8] Servo viewport init failed: {e}");
            return;
        }
    };

    let mut generation: u64 = 0;
    let mut last_frame_push = Instant::now();
    let idle_tick = Duration::from_millis(32);

    loop {
        match rx.recv_timeout(idle_tick) {
            Ok(ViewportCmd::Navigate(url)) => {
                {
                    let mut s = snap.lock().expect("snap");
                    s.url = url.clone();
                    s.loading = true;
                }
                let nav_result = renderer.navigate(&url);
                generation = generation.saturating_add(1);
                {
                    let mut s = snap.lock().expect("snap");
                    s.title = renderer.title();
                    s.loading = false;
                    s.frame_generation = generation;
                    if let Ok(fav) = extract_favicon(&mut renderer) {
                        s.favicon = Some(fav.0);
                        s.favicon_mime = Some(fav.1);
                    }
                    s.find_matches = 0;
                    s.find_active = 0;
                }
                if let Err(ref e) = nav_result {
                    eprintln!("[rv8] navigate {url}: {e}");
                }
                if let Some(frame) = renderer.capture_frame(generation) {
                    let mut s = snap.lock().expect("snap");
                    s.width = frame.width;
                    s.height = frame.height;
                    s.pixels = Some(frame.pixels);
                    last_frame_push = Instant::now();
                }
            }
            Ok(ViewportCmd::Resize { width: w, height: h }) => {
                width = w.max(1);
                height = h.max(1);
                renderer.resize(width, height);
                generation = generation.saturating_add(1);
                let mut s = snap.lock().expect("snap");
                s.width = width;
                s.height = height;
                s.frame_generation = generation;
            }
            Ok(ViewportCmd::Scroll { delta_x, delta_y }) => {
                renderer.scroll_by(delta_x, delta_y);
                generation = generation.saturating_add(1);
                let mut s = snap.lock().expect("snap");
                s.frame_generation = generation;
                if let Some(frame) = renderer.capture_frame(generation) {
                    s.width = frame.width;
                    s.height = frame.height;
                    s.pixels = Some(frame.pixels);
                    last_frame_push = Instant::now();
                }
            }
            Ok(ViewportCmd::FindInPage { query, forward }) => {
                let result = perform_find_in_page(&mut renderer, &query, forward);
                let mut s = snap.lock().expect("snap");
                s.find_matches = result.matches;
                s.find_active = result.active;
                s.frame_generation = generation;
            }
            Ok(ViewportCmd::FindStop) => {
                let _ = renderer.evaluate_script_sync("window.getSelection().removeAllRanges();");
                let mut s = snap.lock().expect("snap");
                s.find_matches = 0;
                s.find_active = 0;
            }
            Ok(ViewportCmd::Click { x, y }) => {
                renderer.handle_mouse_click_at(x, y);
                generation = generation.saturating_add(1);
                let mut s = snap.lock().expect("snap");
                s.frame_generation = generation;
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                renderer.tick();
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }

        {
            let title = renderer.title();
            if !title.is_empty() {
                let mut s = snap.lock().expect("snap");
                if s.title != title {
                    s.title = title;
                }
            }
        }

        let should_capture = renderer.has_pending_frame()
            || last_frame_push.elapsed() > Duration::from_millis(250);
        if should_capture {
            if let Some(frame) = renderer.capture_frame(generation) {
                let mut s = snap.lock().expect("snap");
                if s.frame_generation == generation || s.pixels.is_none() {
                    s.width = frame.width;
                    s.height = frame.height;
                    s.pixels = Some(frame.pixels);
                    last_frame_push = Instant::now();
                }
            }
        }
    }
}

struct FindResult {
    matches: u32,
    active: u32,
}

fn perform_find_in_page(renderer: &mut ServoRenderer, query: &str, forward: bool) -> FindResult {
    let mut script = String::with_capacity(512);
    script.push_str("(()=>{const q='");
    script.push_str(&query.replace('\\', "\\\\").replace('\'', "\\'"));
    script.push_str("';const d=!!window.__rv8_find_dir;window.__rv8_find_dir=");
    script.push_str(if forward { "true" } else { "false" });
    script.push_str(";if(window.__rv8_find_query!==q){window.__rv8_find_query=q;window.__rv8_find_idx=0;}const r=window.find(q,false,false,true,true,false);if(!r){window.__rv8_find_idx=0;return JSON.stringify({m:0,a:0});}window.__rv8_find_idx+=1;let c=0;const b=document.body;if(b){const t=b.innerText||b.textContent||'';let i=-1;while((i=t.indexOf(q,i+1))!==-1)c++;}return JSON.stringify({m:c,a:window.__rv8_find_idx});})()");
    match renderer.evaluate_script_sync(&script) {
        Ok(val) => {
            serde_json::from_str::<serde_json::Value>(&val)
                .ok()
                .and_then(|v| {
                    Some(FindResult {
                        matches: v.get("m")?.as_u64()? as u32,
                        active: v.get("a")?.as_u64()? as u32,
                    })
                })
                .unwrap_or(FindResult { matches: 0, active: 0 })
        }
        Err(_) => FindResult { matches: 0, active: 0 },
    }
}

fn extract_favicon(renderer: &mut ServoRenderer) -> Result<(Vec<u8>, String), ()> {
    let script = r#"
    (()=>{const l=document.querySelector('link[rel*="icon"]');if(!l)return'';const h=l.getAttribute('href')||'';if(!h)return'';try{return new URL(h,document.baseURI).toString();}catch{return h;}})()
    "#;
    let url = match renderer.evaluate_script_sync(script) {
        Ok(val) => val.trim_matches(|c| c == '"' || c == '\'').to_string(),
        Err(_) => return Err(()),
    };
    if url.is_empty() {
        return Err(());
    }
    if url.starts_with("data:") {
        if let Some(comma) = url.find(',') {
            let data = &url[comma + 1..];
            let bytes = base64_decode(data).unwrap_or_default();
            if !bytes.is_empty() {
                return Ok((bytes, "image/x-icon".into()));
            }
        }
    }
    Err(())
}

fn base64_decode(input: &str) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let mut buf = 0u32;
    let mut bits = 0;
    for b in input.bytes() {
        let val = match b {
            b'A'..=b'Z' => b - b'A',
            b'a'..=b'z' => b - b'a' + 26,
            b'0'..=b'9' => b - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' => {
                bits = 0;
                continue;
            }
            _ => continue,
        } as u32;
        buf = (buf << 6) | val;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
        }
    }
    if out.is_empty() { None } else { Some(out) }
}
