//! RV8 browser chrome — gpui shell with Servo rendering.
//! Build: cargo run --features chrome,servo-render --bin rv8-chrome

use crepuscularity_gpui::prelude::*;
use gpui::{
    actions, point, px, rgb, size, Bounds, KeyBinding, Render, Window,
    WindowBounds, WindowOptions,
};

actions!(
    rv8_chrome,
    [NewTab, CloseTab, GoBack, GoForward, Reload, FocusUrl]
);

const BG: u32 = 0x1a1a1a;
const BG_HOVER: u32 = 0x2a2a2a;
const SURFACE: u32 = 0x2c2c2e;
const BORDER: u32 = 0x3a3a3c;
const TEXT: u32 = 0xe4e4e7;
const TEXT_MUTED: u32 = 0x8e8e93;
const TEXT_URL: u32 = 0x34d399;
const TAB_BG: u32 = 0x000000;
const TAB_INACTIVE: u32 = 0x1a1a1a;
const TOOLBAR: u32 = 0x171717;

struct Tab {
    id: u64,
    url: String,
    title: String,
    loading: bool,
}

impl Tab {
    fn new(id: u64, url: &str) -> Self {
        Self {
            id,
            url: url.to_string(),
            title: Self::display_title(url),
            loading: false,
        }
    }

    fn display_title(url: &str) -> String {
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
    loading: bool,
}

impl Chrome {
    fn new() -> Self {
        let url = "https://google.com".to_string();
        Self {
            tabs: vec![Tab::new(1, &url)],
            active: 0,
            next_id: 2,
            url_text: url,
            loading: true,
        }
    }

    fn navigate(&mut self, raw: &str) {
        let url = normalize_url(raw);
        self.url_text = url.clone();
        self.loading = true;
        if let Some(tab) = self.tabs.get_mut(self.active) {
            tab.url = url;
            tab.loading = true;
        }
    }

    fn nav(&mut self, _: &GoBack, _: &mut Window, _: &mut Context<Self>) {}
    fn nav_fwd(&mut self, _: &GoForward, _: &mut Window, _: &mut Context<Self>) {}
    fn do_reload(&mut self, _: &Reload, _: &mut Window, cx: &mut Context<Self>) {
        let url = self.url_text.clone();
        self.navigate(&url);
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
        if self.tabs.len() <= 1 {
            return;
        }
        self.tabs.remove(self.active);
        self.active = self.active.min(self.tabs.len().saturating_sub(1));
        self.url_text = self.tabs[self.active].url.clone();
        cx.notify();
    }

    fn select_tab(&mut self, idx: usize, cx: &mut Context<Self>) {
        if idx >= self.tabs.len() || idx == self.active {
            return;
        }
        self.active = idx;
        self.url_text = self.tabs[idx].url.clone();
        cx.notify();
    }
}

fn normalize_url(input: &str) -> String {
    let t = input.trim();
    if t.is_empty() {
        return "about:blank".into();
    }
    if t.contains("://") || t.starts_with("about:") || t.starts_with("data:") {
        return t.to_string();
    }
    if t.contains('.') && !t.contains(' ') {
        format!("https://{t}")
    } else {
        format!("https://duckduckgo.com/?q={}", t.replace(' ', "+"))
    }
}

impl Render for Chrome {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let secure = self.url_text.starts_with("https://");

        // Tab bar
        let mut tab_els = Vec::new();
        for (i, tab) in self.tabs.iter().enumerate() {
            let sel = i == self.active;
            let bg = if sel { TAB_BG } else { TAB_INACTIVE };
            let close = if self.tabs.len() > 1 {
                div()
                    .size(px(20.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(4.))
                    .text_sm()
                    .text_color(rgb(TEXT_MUTED))
                    .hover(|s| s.bg(rgb(SURFACE)).text_color(rgb(TEXT)))
                    .child("×")
                    .into_any_element()
            } else {
                div().into_any_element()
            };

            let t = div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(4.))
                .max_w(px(180.))
                .flex_shrink_0()
                .px(px(8.))
                .py(px(6.))
                .rounded_t(px(7.))
                .bg(rgb(bg))
                .border_t_1()
                .border_x_1()
                .border_color(rgb(if sel { BORDER } else { BG }))
                .cursor_pointer()
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .overflow_hidden()
                        .text_sm()
                        .text_color(rgb(TEXT))
                        .child(if tab.loading {
                            format!("◌ {}", tab.title)
                        } else {
                            tab.title.clone()
                        }),
                )
                .child(close);

            // Clone what we need for the closure
            let idx = i;
            let t = t.on_mouse_down(
                gpui::MouseButton::Left,
                cx.listener(move |this: &mut Chrome, _e: &gpui::MouseDownEvent, _w: &mut Window, cx: &mut Context<Chrome>| {
                    this.select_tab(idx, cx);
                }),
            );
            tab_els.push(t);
        }

        let tabs = div()
            .flex()
            .flex_row()
            .items_end()
            .gap(px(4.))
            .pl(px(80.))
            .overflow_hidden()
            .children(tab_els);

        // Navigation
        let nav_btn = |label: &'static str| {
            div()
                .size(px(32.))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(8.))
                .text_color(rgb(TEXT_MUTED))
                .hover(|s| s.bg(rgb(BG_HOVER)).text_color(rgb(TEXT)))
                .cursor_pointer()
                .child(label)
        };

        let url_bar = div()
            .flex_1()
            .flex()
            .flex_row()
            .items_center()
            .h(px(36.))
            .px(px(12.))
            .rounded(px(18.))
            .bg(rgb(BG))
            .border_1()
            .border_color(rgb(BORDER))
            .child(
                div()
                    .text_xs()
                    .px(px(4.))
                    .child(if secure { "🔒" } else { "⊞" }),
            )
            .child(
                div()
                    .flex_1()
                    .px(px(8.))
                    .text_sm()
                    .text_color(rgb(if secure { TEXT_URL } else { TEXT }))
                    .overflow_hidden()
                    .child(self.url_text.clone()),
            );

        let toolbar = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.))
            .px(px(12.))
            .py(px(8.))
            .min_h(px(48.))
            .bg(rgb(TOOLBAR))
            .border_b_1()
            .border_color(rgb(BORDER))
            .child(nav_btn("◀"))
            .child(nav_btn("▶"))
            .child(nav_btn("⟳"))
            .child(url_bar)
            .child(
                div()
                    .size(px(28.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(6.))
                    .text_color(rgb(TEXT_MUTED))
                    .hover(|s| s.bg(rgb(BG_HOVER)).text_color(rgb(TEXT)))
                    .cursor_pointer()
                    .child("+"),
            );

        let content = div().flex_1().w_full().bg(rgb(TAB_BG));

        div()
            .id("root")
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(BG))
            .on_action(cx.listener(Self::do_new_tab))
            .on_action(cx.listener(Self::do_close_tab))
            .on_action(cx.listener(Self::nav))
            .on_action(cx.listener(Self::nav_fwd))
            .on_action(cx.listener(Self::do_reload))
            .on_action(cx.listener(Self::do_focus))
            .child(tabs)
            .child(toolbar)
            .child(content)
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
                    {
                        Some(point(px(14.), px(16.)))
                    }
                    #[cfg(not(target_os = "macos"))]
                    {
                        None
                    }
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
