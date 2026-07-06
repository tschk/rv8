//! RV8 browser chrome — pocb-style gpui shell with Servo rendering.
//! Build: cargo run --features chrome,servo-render --bin rv8-chrome
//! Shell: cargo run --features chrome --bin rv8-chrome

use crepuscularity_gpui::prelude::*;
use crepuscularity_gpui::Icon;
use gpui::{
    actions, point, px, rgb, size, AnyElement, Bounds, KeyBinding, Render, Window,
    WindowBounds, WindowOptions,
};
use std::cell::RefCell;
use std::rc::Rc;

#[cfg(feature = "servo-render")]
use rv8::servo_embed::ServoHost;

// ── Theme (pocb dark) ──
const BG: u32 = 0x000000;
const TOOLBAR_BG: u32 = 0x1c1c1e;
const SIDEBAR_BG: u32 = 0x000000;
const HOVER: u32 = 0x222222;
const BORDER: u32 = 0x38383a;
const TEXT: u32 = 0xe4e4e7;
const TEXT_MUTED: u32 = 0x8e8e93;
const TEXT_URL: u32 = 0x34d399;
const ADDR_BG: u32 = 0x222224;

// pocb metrics
const TOPBAR_H: f32 = 40.0;
const BTN_SIZE: f32 = 28.0;
const ADDR_H: f32 = 28.0;
const SIDEBAR_W: f32 = 240.0;
const TRAFFIC_H: f32 = 52.0;

actions!(
    rv8_chrome,
    [NewTab, CloseTab, GoBack, GoForward, Reload, FocusUrl, ToggleSidebar]
);

struct Tab {
    id: u64,
    url: String,
    title: String,
}

impl Tab {
    fn new(id: u64, url: &str) -> Self {
        Self { id, url: url.to_string(), title: Self::title_from(url) }
    }
    fn title_from(url: &str) -> String {
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

struct Chrome {
    tabs: Vec<Tab>,
    active: usize,
    next_id: u64,
    url_text: String,
    #[cfg(feature = "servo-render")]
    servo_host: Option<ServoHost>,
    #[cfg(feature = "servo-render")]
    frame: Rc<RefCell<Option<RenderFrame>>>,
}

#[cfg(feature = "servo-render")]
struct RenderFrame {
    _width: u32,
    _height: u32,
}

impl Chrome {
    fn new() -> Self {
        let url = "https://google.com".to_string();
        Self {
            tabs: vec![Tab::new(1, &url)],
            active: 0,
            next_id: 2,
            url_text: url,
            #[cfg(feature = "servo-render")]
            servo_host: None,
            #[cfg(feature = "servo-render")]
            frame: Rc::new(RefCell::new(None)),
        }
    }

    fn navigate_to(&mut self, raw: &str) {
        let url = normalize_url(raw);
        self.url_text = url.clone();
        if let Some(tab) = self.tabs.get_mut(self.active) {
            tab.url = url.clone();
            tab.title = Tab::title_from(&url);
        }
        #[cfg(feature = "servo-render")]
        self.servo_render(&url);
    }

    #[cfg(feature = "servo-render")]
    fn servo_render(&mut self, url: &str) {
        let frame = self.frame.clone();
        if self.servo_host.is_none() {
            self.servo_host = ServoHost::launch(1280, 800).ok();
        }
        if let Some(ref host) = self.servo_host {
            match host.navigate(url) {
                Ok(result) => {
                    if let Some(ref mut tab) = self.tabs.get_mut(self.active) {
                        if !result.title.is_empty() {
                            tab.title = result.title;
                        }
                    }
                    // ponytail: frame RGBA stored but gpui RenderImage not wired yet
                    if result.frame.is_some() {
                        *frame.borrow_mut() = Some(RenderFrame { _width: 1280, _height: 800 });
                    }
                }
                Err(e) => log::error!("ServoHost: {e}"),
            }
        }
    }

    fn do_new_tab(&mut self, _: &NewTab, _: &mut Window, cx: &mut Context<Self>) {
        let id = self.next_id; self.next_id += 1;
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
        let url = self.url_text.clone();
        #[cfg(feature = "servo-render")]
        self.servo_render(&url);
        cx.notify();
    }

    fn do_reload(&mut self, _: &Reload, _: &mut Window, cx: &mut Context<Self>) {
        let url = self.url_text.clone();
        self.navigate_to(&url);
        cx.notify();
    }

    fn do_nav(&mut self, _: &GoBack, _: &mut Window, _: &mut Context<Self>) {}
    fn do_fwd(&mut self, _: &GoForward, _: &mut Window, _: &mut Context<Self>) {}
    fn do_focus(&mut self, _: &FocusUrl, _: &mut Window, _: &mut Context<Self>) {}
    fn toggle_sidebar(&mut self, _: &ToggleSidebar, _: &mut Window, _: &mut Context<Self>) {}
}

fn normalize_url(input: &str) -> String {
    let t = input.trim();
    if t.is_empty() { return "about:blank".into(); }
    if t.contains("://") || t.starts_with("about:") || t.starts_with("data:") { return t.to_string(); }
    if t.contains('.') && !t.contains(' ') { format!("https://{t}") }
    else { format!("https://duckduckgo.com/?q={}", t.replace(' ', "+")) }
}

fn icon_muted() -> Icon {
    Icon::new("globe").size(px(16.)).text_color(0x8e8e93)
}

fn make_icon(name: &'static str, color: u32) -> Icon {
    Icon::new(name).size(px(16.)).text_color(color)
}

// ── Render ──
impl Render for Chrome {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let secure = self.url_text.starts_with("https://");

        // ── Sidebar with vertical tabs ──
        let mut tab_rows: Vec<AnyElement> = Vec::new();
        for (i, tab) in self.tabs.iter().enumerate() {
            let sel = i == self.active;
            let idx = i;
            let row = div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.))
                .w_full()
                .px(px(12.))
                .py(px(6.))
                .rounded(px(6.))
                .bg(rgb(if sel { HOVER } else { SIDEBAR_BG }))
                .cursor_pointer()
                .hover(|s| s.bg(rgb(HOVER)))
                .on_mouse_down(gpui::MouseButton::Left, cx.listener(
                    move |this: &mut Chrome, _: &gpui::MouseDownEvent, _: &mut Window, cx: &mut Context<Chrome>| {
                        this.select_tab(idx, cx);
                    },
                ))
                .child(make_icon("globe", 0x8e8e93))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .overflow_hidden()
                        .text_sm()
                        .text_color(rgb(TEXT))
                        .child(tab.title.clone()),
                )
                .into_any_element();
            tab_rows.push(row);
        }

        let sidebar = div()
            .id("sidebar")
            .flex()
            .flex_col()
            .w(px(SIDEBAR_W))
            .min_w(px(SIDEBAR_W))
            .bg(rgb(SIDEBAR_BG))
            .border_r_1()
            .border_color(rgb(BORDER))
            .child(
                // Traffic light clearance + tabs
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .pt(px(TRAFFIC_H))
                    .gap(px(2.))
                    .px(px(4.))
                    .children(tab_rows),
            )
            .child(
                // New tab button at bottom
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.))
                    .px(px(16.))
                    .py(px(10.))
                    .border_t_1()
                    .border_color(rgb(BORDER))
                    .cursor_pointer()
                    .child(make_icon("plus.circle", 0x8e8e93))
                    .child(div().text_sm().text_color(rgb(TEXT_MUTED)).child("New Tab")),
            );

        // ── Topbar ──
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
            .child(div().size(px(BTN_SIZE)).flex().items_center().justify_center().rounded(px(6.)).hover(|s| s.bg(rgb(HOVER))).cursor_pointer().child(make_icon("chevron.backward", 0x8e8e93)))
            .child(div().size(px(BTN_SIZE)).flex().items_center().justify_center().rounded(px(6.)).hover(|s| s.bg(rgb(HOVER))).cursor_pointer().child(make_icon("chevron.forward", 0x8e8e93)))
            .child(div().size(px(BTN_SIZE)).flex().items_center().justify_center().rounded(px(6.)).hover(|s| s.bg(rgb(HOVER))).cursor_pointer().child(make_icon("arrow.clockwise", 0x8e8e93)))
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_row()
                    .items_center()
                    .h(px(ADDR_H))
                    .px(px(10.))
                    .gap(px(6.))
                    .rounded(px(7.))
                    .bg(rgb(ADDR_BG))
                    .border_1()
                    .border_color(rgb(BORDER))
                    .child(if secure { make_icon("lock.fill", 0x34d399) } else { make_icon("globe", 0x8e8e93) })
                    .child(
                        div()
                            .flex_1()
                            .text_sm()
                            .text_color(rgb(if secure { TEXT_URL } else { TEXT }))
                            .overflow_hidden()
                            .child(self.url_text.clone()),
                    ),
            )
            .child(div().size(px(BTN_SIZE)).flex().items_center().justify_center().rounded(px(6.)).hover(|s| s.bg(rgb(HOVER))).cursor_pointer().child(make_icon("plus", 0x8e8e93)));

        // ── Content area ──
        let content = div()
            .flex_1()
            .w_full()
            .flex()
            .items_center()
            .justify_center()
            .bg(rgb(BG))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(12.))
                    .child(make_icon("globe", 0x333333).size(px(48.)))
                    .child(div().text_lg().text_color(rgb(0x333333)).child("RV8"))
                    .child(div().text_sm().text_color(rgb(0x222222)).child(
                        if cfg!(feature = "servo-render") {
                            "Servo renderer ready"
                        } else {
                            "Build with --features chrome,servo-render"
                        })),
            );

        // ── Root ──
        div()
            .id("root")
            .size_full()
            .flex()
            .flex_row()
            .bg(rgb(BG))
            .on_action(cx.listener(Self::do_new_tab))
            .on_action(cx.listener(Self::do_close_tab))
            .on_action(cx.listener(Self::do_nav))
            .on_action(cx.listener(Self::do_fwd))
            .on_action(cx.listener(Self::do_reload))
            .on_action(cx.listener(Self::do_focus))
            .on_action(cx.listener(Self::toggle_sidebar))
            .child(sidebar)
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .child(topbar)
                    .child(div().h(px(1.)).bg(rgb(BORDER)))
                    .child(content),
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
                    { Some(point(px(18.), px(18.))) }
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
