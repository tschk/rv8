//! RV8 browser chrome — pocb-style gpui shell with Servo rendering.
//! Build: cargo run --features chrome,servo-render --bin rv8-chrome
//! Shell: cargo run --features chrome --bin rv8-chrome

use crepuscularity_gpui::prelude::*;
use crepuscularity_gpui::Icon;
use gpui::{
    actions, point, px, rgb, size, AnyElement, Bounds, KeyBinding, Render, Window,
    WindowBounds, WindowOptions,
};

#[cfg(feature = "servo-render")]
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(feature = "servo-render")]
use std::sync::{Arc, Mutex};
#[cfg(feature = "servo-render")]
use std::thread;

#[cfg(feature = "servo-render")]
use rv8::servo_embed::ServoRenderer;

// ── Theme (pocb dark) ──
const BG: u32 = 0x000000;
const TOOLBAR_BG: u32 = 0x1c1c1e;
const SIDEBAR_BG: u32 = 0x000000;
const HOVER: u32 = 0x2a2a2a;
const BORDER: u32 = 0x38383a;
const TEXT: u32 = 0xe4e4e7;
const TEXT_MUTED: u32 = 0x8e8e93;
const TEXT_URL: u32 = 0x34d399;
const TAB_BG: u32 = 0x000000;
const ADDR_BG: u32 = 0x222224;
const ADDR_RADIUS: f32 = 7.0;

// pocb metrics
const TOPBAR_H: f32 = 40.0;
const BTN_SIZE: f32 = 28.0;
const BTN_RADIUS: f32 = 6.0;
const ADDR_H: f32 = 28.0;
const SIDEBAR_W: f32 = 240.0;
const TRAFFIC_W: f32 = 72.0;

actions!(
    rv8_chrome,
    [NewTab, CloseTab, GoBack, GoForward, Reload, FocusUrl, ToggleSidebar]
);

// ── Frame data bridge Servo thread → gpui ──
#[cfg(feature = "servo-render")]
struct FrameStream {
    latest: Arc<Mutex<Option<(u32, u32, Vec<u8>)>>>,
    pending: Arc<AtomicBool>,
    renderer: Arc<Mutex<Option<ServoRenderer>>>,
    loading: Arc<AtomicBool>,
    title: Arc<Mutex<String>>,
}

#[cfg(feature = "servo-render")]
impl FrameStream {
    fn new() -> Self {
        Self {
            latest: Arc::new(Mutex::new(None)),
            pending: Arc::new(AtomicBool::new(false)),
            renderer: Arc::new(Mutex::new(None)),
            loading: Arc::new(AtomicBool::new(false)),
            title: Arc::new(Mutex::new(String::new())),
        }
    }

    fn navigate(&self, url: &str) {
        let url = url.to_string();
        let latest = self.latest.clone();
        let pending = self.pending.clone();
        let renderer = self.renderer.clone();
        let loading = self.loading.clone();
        let title = self.title.clone();

        loading.store(true, Ordering::Relaxed);
        thread::spawn(move || {
            let sv_w = 1280u32;
            let sv_h = 800u32;
            let mut r = match ServoRenderer::new(sv_w, sv_h) {
                Ok(r) => r,
                Err(e) => { log::error!("ServoRenderer: {e}"); return; }
            };
            if let Err(e) = r.navigate(&url) {
                log::error!("navigate: {e}");
                return;
            }
            *title.lock().unwrap() = r.title();
            if let Some(frame) = r.capture_frame(1) {
                *latest.lock().unwrap() = Some((frame.width, frame.height, frame.pixels));
            }
            *renderer.lock().unwrap() = Some(r);
            loading.store(false, Ordering::Relaxed);
            pending.store(true, Ordering::Relaxed);
        });
    }
}

// ── Tab ──
struct Tab {
    #[allow(dead_code)]
    id: u64,
    url: String,
    title: String,
    loading: bool,
    #[cfg(feature = "servo-render")]
    stream: FrameStream,
}

impl Tab {
    fn new(id: u64, url: &str) -> Self {
        Self {
            id,
            url: url.to_string(),
            title: Self::disp_title(url),
            loading: false,
            #[cfg(feature = "servo-render")]
            stream: FrameStream::new(),
        }
    }

    fn disp_title(url: &str) -> String {
        url.trim()
            .strip_prefix("https://")
            .or_else(|| url.trim().strip_prefix("http://"))
            .unwrap_or(url.trim())
            .split('/')
            .next()
            .unwrap_or(url.trim())
            .to_string()
    }
}

// ── Chrome state ──
struct Chrome {
    tabs: Vec<Tab>,
    active: usize,
    next_id: u64,
    url_text: String,
    sidebar_visible: bool,
    #[cfg(feature = "servo-render")]
    pending_frames: Vec<(usize, u32, u32, Vec<u8>)>,
}

impl Chrome {
    fn new() -> Self {
        let url = "https://google.com".to_string();
        Self {
            tabs: vec![Tab::new(1, &url)],
            active: 0,
            next_id: 2,
            url_text: url.clone(),
            sidebar_visible: false,
            #[cfg(feature = "servo-render")]
            pending_frames: Vec::new(),
        }
    }

    fn active_tab_mut(&mut self) -> Option<&mut Tab> { self.tabs.get_mut(self.active) }

    fn navigate_to(&mut self, raw: &str) {
        let url = normalize_url(raw);
        self.url_text = url.clone();
        if let Some(tab) = self.active_tab_mut() {
            tab.url = url.clone();
            tab.title = Tab::disp_title(&url);
            tab.loading = true;
            #[cfg(feature = "servo-render")]
            tab.stream.navigate(&url);
        }
    }

    fn do_nav(&mut self, _: &GoBack, _: &mut Window, _: &mut Context<Self>) {
        // ponytail: Servo history via WebView::go_back — needs renderer ref
    }
    fn do_fwd(&mut self, _: &GoForward, _: &mut Window, _: &mut Context<Self>) {
        // ponytail: WebView::go_forward
    }
    fn do_reload(&mut self, _: &Reload, _: &mut Window, cx: &mut Context<Self>) {
        let url = self.url_text.clone();
        self.navigate_to(&url);
        cx.notify();
    }
    fn do_focus(&mut self, _: &FocusUrl, _: &mut Window, _: &mut Context<Self>) {}

    fn do_new_tab(&mut self, _: &NewTab, _: &mut Window, cx: &mut Context<Self>) {
        let id = self.next_id;
        self.next_id += 1;
        let url = "https://google.com".to_string();
        self.tabs.push(Tab::new(id, &url));
        self.active = self.tabs.len() - 1;
        self.url_text = url;
        cx.notify();
    }

    fn do_close_tab(&mut self, _: &CloseTab, _: &mut Window, cx: &mut Context<Self>) {
        if self.tabs.len() <= 1 { return; }
        self.tabs.remove(self.active);
        self.active = self.active.min(self.tabs.len().saturating_sub(1));
        self.url_text = self.tabs[self.active].url.clone();
        cx.notify();
    }

    fn select_tab(&mut self, idx: usize, cx: &mut Context<Self>) {
        if idx >= self.tabs.len() || idx == self.active { return; }
        self.active = idx;
        self.url_text = self.tabs[idx].url.clone();
        cx.notify();
    }

    fn toggle_sidebar(&mut self, _: &ToggleSidebar, _: &mut Window, cx: &mut Context<Self>) {
        self.sidebar_visible = !self.sidebar_visible;
        cx.notify();
    }
}

fn normalize_url(input: &str) -> String {
    let t = input.trim();
    if t.is_empty() { return "about:blank".into(); }
    if t.contains("://") || t.starts_with("about:") || t.starts_with("data:") {
        return t.to_string();
    }
    if t.contains('.') && !t.contains(' ') {
        format!("https://{t}")
    } else {
        format!("https://duckduckgo.com/?q={}", t.replace(' ', "+"))
    }
}

// ── Nav button builder ──
fn mk_btn(name: &'static str) -> impl IntoElement {
    div()
        .size(px(BTN_SIZE))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(BTN_RADIUS))
        .text_color(rgb(TEXT_MUTED))
        .hover(|s| s.bg(rgb(HOVER)).text_color(rgb(TEXT)))
        .cursor_pointer()
        .child(Icon::new(name).size(px(16.)))
}

fn mk_url(dsp: &str) -> impl IntoElement {
    div()
        .flex_1()
        .px(px(8.))
        .text_sm()
        .text_color(rgb(if dsp.starts_with("https://") { TEXT_URL } else { TEXT }))
        .overflow_hidden()
        .child(dsp.to_string())
}

// ── Render ──
impl Render for Chrome {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let secure = self.url_text.starts_with("https://");

        // ── Tab strip (pocb sidebar tree — simplified to horizontal for now) ──
        let mut tab_els: Vec<AnyElement> = Vec::new();
        for (i, tab) in self.tabs.iter().enumerate() {
            let sel = i == self.active;
            let idx = i;

            let el = div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(4.))
                .max_w(px(160.))
                .flex_shrink_0()
                .px(px(8.))
                .py(px(6.))
                .rounded_t(px(7.))
                .bg(rgb(if sel { TAB_BG } else { BG }))
                .border_t_1()
                .border_x_1()
                .border_color(rgb(if sel { BORDER } else { BG }))
                .cursor_pointer()
                .hover(|s| s.bg(rgb(if sel { TAB_BG } else { HOVER })))
                .on_mouse_down(gpui::MouseButton::Left, cx.listener(
                    move |this: &mut Chrome, _: &gpui::MouseDownEvent, _: &mut Window, cx: &mut Context<Chrome>| {
                        this.select_tab(idx, cx);
                    },
                ))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .overflow_hidden()
                        .text_sm()
                        .text_color(rgb(TEXT))
                        .child(if tab.loading { format!("◌ {}", tab.title) } else { tab.title.clone() }),
                )
                .into_any_element();

            tab_els.push(el);
        }

        let tab_strip = div()
            .flex()
            .flex_row()
            .items_end()
            .gap(px(4.))
            .pl(px(TRAFFIC_W))
            .overflow_hidden()
            .children(tab_els);

        // ── Topbar (pocb ChromeBar, 40px) ──
        let topbar = div()
            .id("topbar")
            .flex()
            .flex_row()
            .items_center()
            .gap(px(2.))
            .h(px(TOPBAR_H))
            .px(px(8.))
            .py(px(4.))
            .bg(rgb(TOOLBAR_BG))
.child(mk_btn("sidebar.left"))
            .child(mk_btn("chevron.backward"))
            .child(mk_btn("chevron.forward"))
            .child(mk_btn("arrow.clockwise"))
            .child(
                // AddrPill
                div()
                    .flex_1()
                    .flex()
                    .flex_row()
                    .items_center()
                    .h(px(ADDR_H))
                    .px(px(8.))
                    .gap(px(6.))
                    .rounded(px(ADDR_RADIUS))
                    .bg(rgb(ADDR_BG))
                    .border_1()
                    .border_color(rgb(BORDER))
                    .child(Icon::new(if secure { "lock.fill" } else { "globe" }).size(px(12.)))
                    .child(mk_url(&self.url_text)),
            )
            .child(Icon::new("plus").size(px(14.)));

        // ── Web content area ──
        let content = div()
            .flex_1()
            .w_full()
            .bg(rgb(TAB_BG));

        // ── Sidebar (pocb style, hidden by default) ──
        let sidebar = if self.sidebar_visible {
            div()
                .id("sidebar")
                .flex()
                .flex_col()
                .w(px(SIDEBAR_W))
                .bg(rgb(SIDEBAR_BG))
                .border_r_1()
                .border_color(rgb(BORDER))
                .child(
                    div()
                        .flex_1()
                        .px(px(12.))
                        .pt(px(52.)) // traffic light clearance
                        .child(
                            div()
                                .text_sm()
                                .text_color(rgb(TEXT_MUTED))
                                .child("Tabs"),
                        ),
                )
                .into_any_element()
        } else {
            div().into_any_element()
        };

        // ── Root layout ──
        div()
            .id("root")
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(BG))
            .on_action(cx.listener(Self::do_new_tab))
            .on_action(cx.listener(Self::do_close_tab))
            .on_action(cx.listener(Self::do_nav))
            .on_action(cx.listener(Self::do_fwd))
            .on_action(cx.listener(Self::do_reload))
            .on_action(cx.listener(Self::do_focus))
            .on_action(cx.listener(Self::toggle_sidebar))
            .child(tab_strip)
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_row()
                    .overflow_hidden()
                    .child(sidebar)
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .child(topbar)
                            .child(
                                // 1px separator (pocb TopSeparator)
                                div()
                                    .h(px(1.))
                                    .bg(rgb(BORDER)),
                            )
                            .child(content),
                    ),
            )
    }
}

fn bind_keys(cx: &mut gpui::App) {
    cx.bind_keys([
        KeyBinding::new("cmd-t", NewTab, None),
        KeyBinding::new("cmd-w", CloseTab, None),
        KeyBinding::new("cmd-l", FocusUrl, None),
        KeyBinding::new("cmd-r", Reload, None),
        KeyBinding::new("cmd-[", GoBack, None),
        KeyBinding::new("cmd-]", GoForward, None),
        KeyBinding::new("cmd-\\", ToggleSidebar, None),
    ]);
}

fn main() {
    gpui::Application::new().run(move |cx: &mut gpui::App| {
        bind_keys(cx);

        let opts = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds {
                origin: point(px(64.), px(48.)),
                size: size(px(1280.), px(840.)),
            })),
            titlebar: Some(gpui::TitlebarOptions {
                title: None,
                appears_transparent: true,
                traffic_light_position: {
                    #[cfg(target_os = "macos")]
                    { Some(point(px(14.), px(16.))) }
                    #[cfg(not(target_os = "macos"))]
                    { None }
                },
            }),
            window_min_size: Some(size(px(640.), px(400.))),
            app_id: Some("rv8.chrome".into()),
            focus: true,
            show: true,
            kind: gpui::WindowKind::Normal,
            is_movable: true,
            is_resizable: true,
            is_minimizable: true,
            display_id: None,
            window_background: gpui::WindowBackgroundAppearance::Opaque,
            window_decorations: None,
            tabbing_identifier: None,
        };

        cx.open_window(opts, |_w, cx| cx.new(|_cx| Chrome::new()))
            .unwrap();
    });
}
