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
}

enum ViewportCmd {
    Navigate(String),
    Resize { width: u32, height: u32 },
    Scroll { delta_x: f32, delta_y: f32 },
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
                let mut s = snap.lock().expect("snap");
                s.title = renderer.title();
                s.loading = false;
                s.frame_generation = generation;
                if let Err(ref e) = nav_result {
                    eprintln!("[rv8] navigate {url}: {e}");
                }
                if let Some(frame) = renderer.capture_frame(generation) {
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
