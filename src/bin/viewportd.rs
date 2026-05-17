//! RV8 viewport daemon: Servo render thread, length-prefixed RGBA frames on stdout.

use std::io::{self, BufRead, Write};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use rv8::servo_embed::viewport::ServoViewport;

const FRAME_MAGIC: &[u8; 4] = b"RV8F";
const META_MAGIC: &[u8; 4] = b"RV8M";

enum Cmd {
    Navigate(String),
    Resize { width: u32, height: u32 },
    Scroll { delta_x: f32, delta_y: f32 },
    Quit,
}

fn write_frame(
    out: &mut impl Write,
    generation: u64,
    width: u32,
    height: u32,
    pixels: &[u8],
) -> io::Result<()> {
    out.write_all(FRAME_MAGIC)?;
    out.write_all(&generation.to_le_bytes())?;
    out.write_all(&width.to_le_bytes())?;
    out.write_all(&height.to_le_bytes())?;
    let len = pixels.len() as u32;
    out.write_all(&len.to_le_bytes())?;
    out.write_all(pixels)?;
    out.flush()
}

fn write_meta(
    out: &mut impl Write,
    generation: u64,
    title: &str,
    url: &str,
) -> io::Result<()> {
    let title_bytes = title.as_bytes();
    let url_bytes = url.as_bytes();
    out.write_all(META_MAGIC)?;
    out.write_all(&generation.to_le_bytes())?;
    out.write_all(&(title_bytes.len() as u32).to_le_bytes())?;
    out.write_all(&(url_bytes.len() as u32).to_le_bytes())?;
    out.write_all(title_bytes)?;
    out.write_all(url_bytes)?;
    out.flush()
}

fn main() {
    rv8::js::soliloquy::ensure_soliloquy_v8_selected();

    let width = std::env::var("RV8_VIEWPORT_WIDTH")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1280);
    let height = std::env::var("RV8_VIEWPORT_HEIGHT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(800);

    let viewport = match ServoViewport::open(width, height) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("viewportd: init failed: {e}");
            std::process::exit(1);
        }
    };

    let (cmd_tx, cmd_rx) = mpsc::channel::<Cmd>();
    thread::spawn(move || {
        let stdin = io::stdin();
        for line in stdin.lock().lines().map_while(Result::ok) {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if line == "QUIT" {
                let _ = cmd_tx.send(Cmd::Quit);
                break;
            }
            if let Some(url) = line.strip_prefix("NAV ") {
                let _ = cmd_tx.send(Cmd::Navigate(url.to_string()));
                continue;
            }
            if let Some(rest) = line.strip_prefix("SIZE ") {
                let mut parts = rest.split_whitespace();
                if let (Some(w), Some(h)) = (parts.next(), parts.next()) {
                    if let (Ok(w), Ok(h)) = (w.parse::<u32>(), h.parse::<u32>()) {
                        let _ = cmd_tx.send(Cmd::Resize { width: w, height: h });
                    }
                }
                continue;
            }
            if let Some(rest) = line.strip_prefix("SCROLL ") {
                let mut parts = rest.split_whitespace();
                if let (Some(dx), Some(dy)) = (parts.next(), parts.next()) {
                    if let (Ok(dx), Ok(dy)) = (dx.parse::<f32>(), dy.parse::<f32>()) {
                        let _ = cmd_tx.send(Cmd::Scroll { delta_x: dx, delta_y: dy });
                    }
                }
            }
        }
    });

    let mut last_gen = 0u64;
    let mut last_title = String::new();
    let mut last_url = String::new();
    loop {
        while let Ok(cmd) = cmd_rx.try_recv() {
            match cmd {
                Cmd::Quit => return,
                Cmd::Navigate(url) => viewport.navigate(&url),
                Cmd::Resize { width, height } => viewport.resize(width, height),
                Cmd::Scroll { delta_x, delta_y } => viewport.scroll_by(delta_x, delta_y),
            }
        }

        let snap = viewport.snapshot();
        if snap.title != last_title || snap.url != last_url {
            last_title = snap.title.clone();
            last_url = snap.url.clone();
            let mut out = io::stdout().lock();
            let _ = write_meta(
                &mut out,
                snap.frame_generation,
                &snap.title,
                &snap.url,
            );
        }
        if snap.frame_generation != last_gen {
            if let Some(ref px) = snap.pixels {
                last_gen = snap.frame_generation;
                let mut out = io::stdout().lock();
                let _ = write_frame(&mut out, snap.frame_generation, snap.width, snap.height, px);
            }
        }

        thread::sleep(Duration::from_millis(16));
    }
}
