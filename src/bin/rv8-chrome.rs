//! RV8 browser chrome: GPUI shell (crepuscularity).
//! Separate from engine — chrome-only process.
//! For full rendering, build with `--features chrome,servo-render` (requires Servo compile).

use crepuscularity_gpui::prelude::*;
use gpui::{
    actions, bounds, point, px, rgb, size, AnyElement, ClickEvent, Entity, Focusable, KeyBinding,
    Render, StatefulInteractiveElement, Window,
};

actions!(
    rv8_chrome,
    [NewTab, CloseTab, GoBack, GoForward, Reload, FocusOmnibox]
);

const CHROME_BG: u32 = 0x101010;
const CONTENT_BG: u32 = 0x1b1b1b;
const TEXT: u32 = 0xe4e4e7;
const MUTED: u32 = 0x71717a;

struct BrowserTab {
    id: u64,
    title: String,
    url: String,
    loading: bool,
}

impl BrowserTab {
    fn new(id: u64, url: impl Into<String>) -> Self {
        let url = url.into();
        let title = title_from_url(&url);
        Self {
            id,
            title,
            url,
            loading: true,
        }
    }
}

fn title_from_url(url: &str) -> String {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return "New Tab".into();
    }
    let host = trimmed
        .strip_prefix("https://")
        .or_else(|| trimmed.strip_prefix("http://"))
        .unwrap_or(trimmed)
        .split('/')
        .next()
        .unwrap_or(trimmed)
        .split('@')
        .next_back()
        .unwrap_or(trimmed);
    if host.len() > 28 {
        format!("{}…", &host[..25])
    } else {
        host.to_string()
    }
}

struct ChromeView {
    tabs: Vec<BrowserTab>,
    active_tab: usize,
    next_tab_id: u64,
    omnibox_text: String,
}

impl ChromeView {
    fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        let start_url = "https://example.com/";
        let tab = BrowserTab::new(1, start_url);
        Self {
            tabs: vec![tab],
            active_tab: 0,
            next_tab_id: 2,
            omnibox_text: start_url.to_string(),
        }
    }

    fn navigate_to(&mut self, raw: &str, cx: &mut Context<Self>) {
        let url = normalize_url(raw);
        if let Some(tab) = self.tabs.get_mut(self.active_tab) {
            tab.url = url.clone();
            tab.title = title_from_url(&url);
        }
        self.omnibox_text = url;
        cx.notify();
    }

    fn active_url(&self) -> String {
        self.tabs
            .get(self.active_tab)
            .map(|t| t.url.clone())
            .unwrap_or_default()
    }

    fn new_tab(&mut self, _: &NewTab, _w: &mut Window, cx: &mut Context<Self>) {
        let id = self.next_tab_id;
        self.next_tab_id += 1;
        self.tabs.push(BrowserTab::new(id, "https://example.com/"));
        self.active_tab = self.tabs.len() - 1;
        self.omnibox_text = "https://example.com/".into();
        cx.notify();
    }

    fn close_tab(&mut self, _: &CloseTab, _w: &mut Window, cx: &mut Context<Self>) {
        if self.tabs.len() <= 1 {
            return;
        }
        self.tabs.remove(self.active_tab);
        if self.active_tab >= self.tabs.len() {
            self.active_tab = self.tabs.len() - 1;
        }
        self.omnibox_text = self.active_url();
        cx.notify();
    }

    fn select_tab(&mut self, idx: usize, cx: &mut Context<Self>) {
        if idx >= self.tabs.len() || idx == self.active_tab {
            return;
        }
        self.active_tab = idx;
        self.omnibox_text = self.active_url();
        cx.notify();
    }

    fn go_back(&mut self, _: &GoBack, _w: &mut Window, cx: &mut Context<Self>) {
        cx.notify();
    }

    fn go_forward(&mut self, _: &GoForward, _w: &mut Window, cx: &mut Context<Self>) {
        cx.notify();
    }

    fn reload(&mut self, _: &Reload, _w: &mut Window, cx: &mut Context<Self>) {
        cx.notify();
    }

    fn focus_omnibox(&mut self, _: &FocusOmnibox, _w: &mut Window, cx: &mut Context<Self>) {
        cx.notify();
    }
}

fn normalize_url(input: &str) -> String {
    let t = input.trim();
    if t.is_empty() {
        return "about:blank".into();
    }
    if t.contains("://") || t.starts_with("about:") {
        return t.to_string();
    }
    if t.contains('.') && !t.contains(' ') {
        format!("https://{t}")
    } else {
        format!("https://duckduckgo.com/?q={}", t.replace(' ', "+"))
    }
}

impl Render for ChromeView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let active_url = self.active_url();
        let secure = active_url.starts_with("https://");

        let tab_count = self.tabs.len();
        let mut tab_els: Vec<AnyElement> = Vec::new();
        for (idx, tab) in self.tabs.iter().enumerate() {
            let sel = idx == self.active_tab;
            let bg = if sel { CONTENT_BG } else { CHROME_BG };
            let mut el = div()
                .flex()
                .flex_row()
                .items_center()
                .gap_0p5()
                .max_w(px(200.))
                .flex_shrink_0()
                .px_2()
                .py_1p5()
                .rounded_t(px(7.))
                .bg(rgb(bg))
                .border_t_1()
                .border_x_1()
                .border_color(rgb(if sel { 0x3f3f46 } else { CHROME_BG }))
                .hover(|s| s.bg(rgb(if sel { CONTENT_BG } else { 0x242424 })))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .overflow_hidden()
                        .text_sm()
                        .text_color(rgb(TEXT))
                        .child(if tab.loading {
                            format!("○ {}", tab.title)
                        } else {
                            tab.title.clone()
                        }),
                );

            if tab_count > 1 {
                let close = div()
                    .size(px(20.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(4.))
                    .text_sm()
                    .text_color(rgb(MUTED))
                    .hover(|s| s.bg(rgb(0x3f3f46)).text_color(rgb(TEXT)))
                    .child("×");
                el = el.child(close);
            }
            tab_els.push(el.into_any_element());
        }

        let tab_strip = div()
            .flex()
            .flex_row()
            .items_end()
            .gap_0p5()
            .pl(px(72.))
            .overflow_hidden()
            .children(tab_els);

        let omnibox = div()
            .flex_1()
            .flex()
            .flex_row()
            .items_center()
            .h(px(36.))
            .mx_1()
            .px_3()
            .rounded(px(18.))
            .bg(rgb(0x181818))
            .border_1()
            .border_color(rgb(0x3f3f46))
            .child(if secure {
                div().text_xs().text_color(rgb(0x34d399)).px_1().child("🔒")
            } else {
                div()
            })
            .child(
                div()
                    .flex_1()
                    .text_sm()
                    .text_color(rgb(TEXT))
                    .overflow_hidden()
                    .child(self.omnibox_text.clone()),
            );

        let content = div().flex_1().w_full().bg(rgb(CONTENT_BG)).child(
            div()
                .flex_1()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap_3()
                .child(div().text_lg().text_color(rgb(TEXT)).child("RV8"))
                .child(
                    div()
                        .text_sm()
                        .text_color(rgb(MUTED))
                        .child("Build with --features chrome,servo-render for embedded viewport"),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(rgb(0x52525b))
                        .child("chrome-shell mode — tabs & navigation only"),
                ),
        );

        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(CHROME_BG))
            .on_action(cx.listener(Self::new_tab))
            .on_action(cx.listener(Self::close_tab))
            .on_action(cx.listener(Self::go_back))
            .on_action(cx.listener(Self::go_forward))
            .on_action(cx.listener(Self::reload))
            .on_action(cx.listener(Self::focus_omnibox))
            .child(tab_strip)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_1p5()
                    .px_3()
                    .py_2()
                    .min_h(px(44.))
                    .bg(rgb(0x171717))
                    .border_b_1()
                    .border_color(rgb(0x27272a))
                    .child(nav_icon("‹"))
                    .child(nav_icon("›"))
                    .child(nav_icon("↻"))
                    .child(omnibox),
            )
            .child(content)
    }
}

fn nav_icon(label: &'static str) -> impl IntoElement {
    div()
        .size(px(32.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(8.))
        .text_color(rgb(TEXT))
        .hover(|s| s.bg(rgb(0x242424)))
        .child(label)
}

fn bind_keys(cx: &mut gpui::App) {
    cx.bind_keys([
        KeyBinding::new("cmd-t", NewTab, None),
        KeyBinding::new("cmd-w", CloseTab, None),
        KeyBinding::new("cmd-l", FocusOmnibox, None),
        KeyBinding::new("cmd-r", Reload, None),
        KeyBinding::new("cmd-[", GoBack, None),
        KeyBinding::new("cmd-]", GoForward, None),
    ]);
}

fn main() {
    gpui::Application::new().run(move |cx: &mut gpui::App| {
        bind_keys(cx);

        let opts = gpui::WindowOptions {
            window_bounds: Some(gpui::WindowBounds::Windowed(bounds(
                point(px(64.), px(48.)),
                size(px(1280.), px(840.)),
            ))),
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

        cx.open_window(opts, |window, cx| cx.new(|cx| ChromeView::new(window, cx)))
            .unwrap();
    });
}
