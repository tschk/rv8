//! RV8 browser chrome — pocb-style gpui shell with Servo rendering.
//! Build: cargo run --features chrome,servo-render --bin rv8-chrome
//! Shell: cargo run --features chrome --bin rv8-chrome

use crepuscularity_gpui::prelude::*;
use crepuscularity_gpui::Icon;

mod chrome_surface;
#[cfg(all(target_os = "macos", feature = "servo-render"))]
use chrome_surface::{NativeSurface, SurfaceConverter};

#[cfg(all(not(target_os = "macos"), feature = "servo-render"))]
use gpui::img;
use gpui::{
    actions, point, px, rgb, size, AnyElement, Bounds, KeyBinding, ObjectFit, Render, Window,
    WindowBounds, WindowOptions,
};
#[cfg(all(not(target_os = "macos"), feature = "servo-render"))]
use image::RgbaImage;
#[cfg(all(target_os = "macos", feature = "servo-render"))]
use std::cell::RefCell;
#[cfg(all(target_os = "macos", feature = "servo-render"))]
use std::rc::Rc;
#[cfg(all(not(target_os = "macos"), feature = "servo-render"))]
use std::sync::Arc;
#[cfg(all(target_os = "macos", feature = "servo-render"))]
use std::time::Duration;

#[cfg(feature = "servo-render")]
use rv8::servo_embed::ServoHost;

// ── Theme ──
const BG: u32 = 0x000000;
const TOOLBAR_BG: u32 = 0x1c1c1e;
const SIDEBAR_BG: u32 = 0x000000;
const HOVER: u32 = 0x222222;
const BORDER: u32 = 0x38383a;
const TEXT: u32 = 0xe4e4e7;
const TEXT_MUTED: u32 = 0x8e8e93;

const TOPBAR_H: f32 = 40.0;
const BTN_SIZE: f32 = 28.0;
const ADDR_H: f32 = 28.0;
const SIDEBAR_W: f32 = 240.0;
const TRAFFIC_H: f32 = 52.0;

actions!(
    rv8_chrome,
    [NewTab, CloseTab, GoBack, GoForward, Reload, FocusUrl]
);

struct Tab {
    #[allow(dead_code)]
    id: u64,
    url: String,
    title: String,
    history: Vec<String>,
    history_pos: usize,
    #[cfg(all(target_os = "macos", feature = "servo-render"))]
    frame_surface: Option<io_surface::IOSurface>,
    #[cfg(all(not(target_os = "macos"), feature = "servo-render"))]
    frame_img: Option<Arc<gpui::RenderImage>>,
}

impl Tab {
    fn new(id: u64, url: &str) -> Self {
        Self {
            id,
            url: url.to_string(),
            title: Self::title_from(url),
            history: vec![url.to_string()],
            history_pos: 0,
            #[cfg(all(target_os = "macos", feature = "servo-render"))]
            frame_surface: None,
            #[cfg(all(not(target_os = "macos"), feature = "servo-render"))]
            frame_img: None,
        }
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
    url_edit: Option<String>,
    #[cfg(feature = "servo-render")]
    servo_host: Option<ServoHost>,
    #[cfg(all(target_os = "macos", feature = "servo-render"))]
    surface_converter: Rc<RefCell<SurfaceConverter>>,
}

impl Chrome {
    fn new(_cx: &mut Context<Self>) -> Self {
        let url = "https://google.com".to_string();
        Self {
            tabs: vec![Tab::new(1, &url)],
            active: 0,
            next_id: 2,
            url_text: url,
            url_edit: None,
            #[cfg(feature = "servo-render")]
            servo_host: None,
            #[cfg(all(target_os = "macos", feature = "servo-render"))]
            surface_converter: Rc::new(RefCell::new(SurfaceConverter::new())),
        }
    }

    fn init(&mut self, cx: &mut Context<Self>) {
        let url = self.url_text.clone();
        self.navigate_to(&url);
        #[cfg(all(target_os = "macos", feature = "servo-render"))]
        self.start_render_loop(cx);
        cx.notify();
    }

    #[cfg(all(target_os = "macos", feature = "servo-render"))]
    fn start_render_loop(&self, cx: &mut Context<Self>) {
        cx.spawn(|this: gpui::WeakEntity<Chrome>, cx: &mut gpui::AsyncApp| {
            let mut cx = cx.clone();
            async move {
                loop {
                    cx.background_executor()
                        .timer(Duration::from_millis(16))
                        .await;
                    if let Some(entity) = this.upgrade() {
                        entity
                            .update(&mut cx, |chrome, cx| chrome.update_surface(cx))
                            .ok();
                    }
                }
            }
        })
        .detach();
    }

    #[cfg(all(target_os = "macos", feature = "servo-render"))]
    fn update_surface(&mut self, cx: &mut Context<Self>) {
        if let Some(ref host) = self.servo_host {
            match host.current_surface() {
                Ok(Some(surface)) => {
                    if let Some(tab) = self.tabs.get_mut(self.active) {
                        tab.frame_surface = Some(surface);
                    }
                }
                Ok(None) => {}
                Err(e) => log::error!("ServoHost current_surface: {e}"),
            }
        }
        cx.notify();
    }

    fn navigate_to(&mut self, raw: &str) {
        let url = normalize_url(raw);
        self.url_text.clone_from(&url);
        self.url_edit = None;
        #[cfg(feature = "servo-render")]
        self.servo_render(&url);
        if let Some(tab) = self.tabs.get_mut(self.active) {
            tab.url = url.clone();
            tab.title = Tab::title_from(&tab.url);
            if tab.history.get(tab.history_pos) != Some(&url) {
                tab.history.truncate(tab.history_pos + 1);
                tab.history.push(url);
                tab.history_pos = tab.history.len() - 1;
            }
        }
    }

    #[cfg(feature = "servo-render")]
    fn servo_render(&mut self, url: &str) {
        let host = self
            .servo_host
            .get_or_insert_with(|| ServoHost::launch(1280, 800).expect("ServoHost launch"));
        match host.navigate(url) {
            Ok(result) => {
                if let Some(ref mut tab) = self.tabs.get_mut(self.active) {
                    if !result.title.is_empty() {
                        tab.title = result.title;
                    }
                }
                #[cfg(all(target_os = "macos", feature = "servo-render"))]
                if let Ok(Some(surface)) = host.current_surface() {
                    if let Some(tab) = self.tabs.get_mut(self.active) {
                        tab.frame_surface = Some(surface);
                    }
                }
                #[cfg(all(not(target_os = "macos"), feature = "servo-render"))]
                if let Some(frame) = result.frame {
                    let w = frame.width;
                    let h = frame.height;
                    if let Some(rgba) = RgbaImage::from_raw(w, h, frame.pixels) {
                        let img_frame = image::Frame::new(rgba);
                        if let Some(tab) = self.tabs.get_mut(self.active) {
                            tab.frame_img = Some(Arc::new(RenderImage::new([img_frame])));
                        }
                    }
                }
            }
            Err(e) => log::error!("ServoHost: {e}"),
        }
    }

    fn do_new_tab(&mut self, _: &NewTab, _: &mut Window, cx: &mut Context<Self>) {
        let id = self.next_id;
        self.next_id += 1;
        let url = "https://google.com".to_string();
        self.tabs.push(Tab::new(id, &url));
        self.active = self.tabs.len() - 1;
        self.url_text = url.clone();
        self.navigate_to(&url);
        cx.notify();
    }

    fn do_close_tab(&mut self, _: &CloseTab, _: &mut Window, cx: &mut Context<Self>) {
        if self.tabs.len() <= 1 {
            return;
        }
        self.tabs.remove(self.active);
        self.active = self.active.min(self.tabs.len().saturating_sub(1));
        self.url_text.clone_from(&self.tabs[self.active].url);
        cx.notify();
    }

    fn select_tab(&mut self, idx: usize, cx: &mut Context<Self>) {
        if idx >= self.tabs.len() || idx == self.active {
            return;
        }
        self.active = idx;
        self.url_text.clone_from(&self.tabs[idx].url);
        #[cfg(feature = "servo-render")]
        {
            let url = self.url_text.clone();
            #[cfg(all(target_os = "macos", feature = "servo-render"))]
            if self.tabs[idx].frame_surface.is_none() {
                self.servo_render(&url);
            }
            #[cfg(all(not(target_os = "macos"), feature = "servo-render"))]
            if self.tabs[idx].frame_img.is_none() {
                self.servo_render(&url);
            }
        }
        cx.notify();
    }

    fn start_url_edit(&mut self, cx: &mut Context<Self>) {
        if self.url_edit.is_none() {
            self.url_edit = Some(self.url_text.clone());
        }
        cx.notify();
    }

    fn set_url_edit(&mut self, text: String, cx: &mut Context<Self>) {
        self.url_edit = Some(text);
        cx.notify();
    }

    fn commit_url_edit(&mut self, cx: &mut Context<Self>) {
        let url = self
            .url_edit
            .take()
            .unwrap_or_else(|| self.url_text.clone());
        self.navigate_to(&url);
        cx.notify();
    }

    fn cancel_url_edit(&mut self, cx: &mut Context<Self>) {
        self.url_edit = None;
        cx.notify();
    }

    fn do_reload(&mut self, _: &Reload, _: &mut Window, cx: &mut Context<Self>) {
        let url = self.url_text.clone();
        self.navigate_to(&url);
        cx.notify();
    }
    fn do_nav(&mut self, _: &GoBack, _: &mut Window, cx: &mut Context<Self>) {
        let url = self.tabs.get(self.active).and_then(|tab| {
            tab.history_pos
                .checked_sub(1)
                .and_then(|i| tab.history.get(i).cloned())
        });
        if let Some(url) = url {
            self.tabs[self.active].history_pos -= 1;
            self.url_text.clone_from(&url);
            #[cfg(feature = "servo-render")]
            self.servo_render(&url);
            if let Some(tab) = self.tabs.get_mut(self.active) {
                tab.url = url.clone();
                tab.title = Tab::title_from(&tab.url);
            }
            cx.notify();
        }
    }
    fn do_fwd(&mut self, _: &GoForward, _: &mut Window, cx: &mut Context<Self>) {
        let url = self
            .tabs
            .get(self.active)
            .and_then(|tab| tab.history.get(tab.history_pos + 1).cloned());
        if let Some(url) = url {
            self.tabs[self.active].history_pos += 1;
            self.url_text.clone_from(&url);
            #[cfg(feature = "servo-render")]
            self.servo_render(&url);
            if let Some(tab) = self.tabs.get_mut(self.active) {
                tab.url = url.clone();
                tab.title = Tab::title_from(&tab.url);
            }
            cx.notify();
        }
    }
    fn do_focus(&mut self, _: &FocusUrl, _: &mut Window, cx: &mut Context<Self>) {
        self.start_url_edit(cx);
    }

    #[cfg(feature = "servo-render")]
    fn content_to_viewport(&self, x: f32, y: f32, window: &Window) -> (f32, f32) {
        let size = window.bounds().size;
        let content_x = x - SIDEBAR_W;
        let content_y = y - TOPBAR_H - 1.0;
        let content_w = (f32::from(size.width) - SIDEBAR_W).max(1.0);
        let content_h = (f32::from(size.height) - TOPBAR_H - 1.0).max(1.0);
        let vp_x = (content_x / content_w) * 1280.0;
        let vp_y = (content_y / content_h) * 800.0;
        (vp_x.clamp(0.0, 1280.0), vp_y.clamp(0.0, 800.0))
    }

    #[cfg(feature = "servo-render")]
    fn handle_mouse_move(&mut self, x: f32, y: f32, cx: &mut Context<Self>) {
        if let Some(ref host) = self.servo_host {
            host.handle_mouse_move(x, y);
            cx.notify();
        }
    }

    #[cfg(feature = "servo-render")]
    fn handle_mouse_click(&mut self, x: f32, y: f32, cx: &mut Context<Self>) {
        if let Some(ref host) = self.servo_host {
            host.handle_mouse_click_at(x, y);
            cx.notify();
        }
    }

    #[cfg(feature = "servo-render")]
    fn handle_scroll(&mut self, delta_x: f32, delta_y: f32, cx: &mut Context<Self>) {
        if let Some(ref host) = self.servo_host {
            host.scroll_by(delta_x, delta_y);
            cx.notify();
        }
    }

    #[cfg(feature = "servo-render")]
    fn handle_key_event(&mut self, event: &gpui::KeyDownEvent, cx: &mut Context<Self>) {
        use keyboard_types::{Key, KeyState, Modifiers};
        use std::str::FromStr;
        if let Some(ref host) = self.servo_host {
            let key = Key::from_str(&event.keystroke.key)
                .unwrap_or_else(|_| Key::Character(event.keystroke.key.clone()));
            let mut modifiers = Modifiers::empty();
            if event.keystroke.modifiers.control {
                modifiers |= Modifiers::CONTROL;
            }
            if event.keystroke.modifiers.alt {
                modifiers |= Modifiers::ALT;
            }
            if event.keystroke.modifiers.shift {
                modifiers |= Modifiers::SHIFT;
            }
            if event.keystroke.modifiers.platform {
                modifiers |= Modifiers::META;
            }
            if event.keystroke.modifiers.function {
                modifiers |= Modifiers::FN;
            }
            let keyboard_event = keyboard_types::KeyboardEvent {
                state: KeyState::Down,
                key,
                modifiers,
                ..Default::default()
            };
            host.handle_key_event(keyboard_event);
            cx.notify();
        }
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

fn make_icon(name: &'static str, color: u32) -> Icon {
    Icon::new(name).size(px(16.)).text_color(color)
}

impl Render for Chrome {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let secure = self.url_text.starts_with("https://");

        // ── Sidebar tabs ──
        let mut tab_rows: Vec<AnyElement> = Vec::new();
        for (i, tab) in self.tabs.iter().enumerate() {
            let sel = i == self.active;
            let idx = i;
            tab_rows.push(
                div()
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
                    .on_mouse_down(
                        gpui::MouseButton::Left,
                        cx.listener(
                            move |this: &mut Chrome,
                                  _: &gpui::MouseDownEvent,
                                  _: &mut Window,
                                  cx: &mut Context<Chrome>| {
                                this.select_tab(idx, cx)
                            },
                        ),
                    )
                    .child(make_icon("globe", TEXT_MUTED))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .overflow_hidden()
                            .text_sm()
                            .text_color(rgb(TEXT))
                            .child(tab.title.clone()),
                    )
                    .into_any_element(),
            );
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
                    .on_mouse_down(
                        gpui::MouseButton::Left,
                        cx.listener(
                            |this: &mut Chrome,
                             _: &gpui::MouseDownEvent,
                             window: &mut Window,
                             cx: &mut Context<Chrome>| {
                                this.do_new_tab(&NewTab, window, cx)
                            },
                        ),
                    )
                    .child(make_icon("plus.circle", TEXT_MUTED))
                    .child(div().text_sm().text_color(rgb(TEXT_MUTED)).child("New Tab")),
            );

        // ── Content ──
        #[cfg(all(not(target_os = "macos"), feature = "servo-render"))]
        let frame_img = self.tabs.get(self.active).and_then(|t| t.frame_img.clone());
        #[cfg(all(target_os = "macos", feature = "servo-render"))]
        let frame_surface = self
            .tabs
            .get(self.active)
            .and_then(|t| t.frame_surface.clone());

        #[cfg(all(target_os = "macos", feature = "servo-render"))]
        let content: AnyElement = if let Some(ref surface) = frame_surface {
            let converter = self.surface_converter.clone();
            div()
                .flex_1()
                .w_full()
                .overflow_hidden()
                .bg(rgb(BG))
                .child(
                    NativeSurface::new(surface.clone(), converter)
                        .w_full()
                        .h_full()
                        .object_fit(ObjectFit::Contain),
                )
                .on_mouse_move(cx.listener(
                    |this: &mut Chrome,
                     event: &gpui::MouseMoveEvent,
                     window: &mut Window,
                     cx: &mut Context<Chrome>| {
                        let (x, y) = this.content_to_viewport(
                            event.position.x.into(),
                            event.position.y.into(),
                            window,
                        );
                        this.handle_mouse_move(x, y, cx);
                    },
                ))
                .on_mouse_down(
                    gpui::MouseButton::Left,
                    cx.listener(
                        |this: &mut Chrome,
                         event: &gpui::MouseDownEvent,
                         window: &mut Window,
                         cx: &mut Context<Chrome>| {
                            this.cancel_url_edit(cx);
                            let (x, y) = this.content_to_viewport(
                                event.position.x.into(),
                                event.position.y.into(),
                                window,
                            );
                            this.handle_mouse_click(x, y, cx);
                        },
                    ),
                )
                .on_scroll_wheel(cx.listener(
                    |this: &mut Chrome,
                     event: &gpui::ScrollWheelEvent,
                     _: &mut Window,
                     cx: &mut Context<Chrome>| {
                        let (dx, dy) = match event.delta {
                            gpui::ScrollDelta::Pixels(p) => (p.x.into(), p.y.into()),
                            gpui::ScrollDelta::Lines(p) => (p.x, p.y),
                        };
                        this.handle_scroll(dx, dy, cx);
                    },
                ))
                .into_any_element()
        } else {
            div().flex_1().w_full().bg(rgb(BG)).into_any_element()
        };

        #[cfg(all(not(target_os = "macos"), feature = "servo-render"))]
        let content: AnyElement = if let Some(ref render_image) = frame_img {
            div()
                .flex_1()
                .w_full()
                .overflow_hidden()
                .bg(rgb(BG))
                .child(
                    img(render_image.clone())
                        .w_full()
                        .h_full()
                        .object_fit(gpui::ObjectFit::Contain),
                )
                .on_mouse_move(cx.listener(
                    |this: &mut Chrome,
                     event: &gpui::MouseMoveEvent,
                     window: &mut Window,
                     cx: &mut Context<Chrome>| {
                        let (x, y) = this.content_to_viewport(
                            event.position.x.into(),
                            event.position.y.into(),
                            window,
                        );
                        this.handle_mouse_move(x, y, cx);
                    },
                ))
                .on_mouse_down(
                    gpui::MouseButton::Left,
                    cx.listener(
                        |this: &mut Chrome,
                         event: &gpui::MouseDownEvent,
                         window: &mut Window,
                         cx: &mut Context<Chrome>| {
                            this.cancel_url_edit(cx);
                            let (x, y) = this.content_to_viewport(
                                event.position.x.into(),
                                event.position.y.into(),
                                window,
                            );
                            this.handle_mouse_click(x, y, cx);
                        },
                    ),
                )
                .on_scroll_wheel(cx.listener(
                    |this: &mut Chrome,
                     event: &gpui::ScrollWheelEvent,
                     _: &mut Window,
                     cx: &mut Context<Chrome>| {
                        let (dx, dy) = match event.delta {
                            gpui::ScrollDelta::Pixels(p) => (p.x.into(), p.y.into()),
                            gpui::ScrollDelta::Lines(p) => (p.x, p.y),
                        };
                        this.handle_scroll(dx, dy, cx);
                    },
                ))
                .into_any_element()
        } else {
            div().flex_1().w_full().bg(rgb(BG)).into_any_element()
        };

        #[cfg(not(feature = "servo-render"))]
        let content: AnyElement = div().flex_1().w_full().bg(rgb(BG)).into_any_element();

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
            .child(btn_with_action(
                cx,
                "chevron.backward",
                |this, window, cx| this.do_nav(&GoBack, window, cx),
            ))
            .child(btn_with_action(
                cx,
                "chevron.forward",
                |this, window, cx| this.do_fwd(&GoForward, window, cx),
            ))
            .child(btn_with_action(
                cx,
                "arrow.clockwise",
                |this, window, cx| this.do_reload(&Reload, window, cx),
            ))
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
                    .bg(rgb(0x222224))
                    .border_1()
                    .border_color(rgb(if self.url_edit.is_some() {
                        0x0a84ff
                    } else {
                        BORDER
                    }))
                    .child(if secure {
                        make_icon("lock.fill", 0x34d399)
                    } else {
                        make_icon("globe", TEXT_MUTED)
                    })
                    .child(
                        div()
                            .id("url-bar")
                            .flex_1()
                            .flex()
                            .flex_row()
                            .items_center()
                            .text_sm()
                            .text_color(rgb(if secure { 0x34d399 } else { TEXT }))
                            .overflow_hidden()
                            .on_mouse_down(
                                gpui::MouseButton::Left,
                                cx.listener(
                                    |this: &mut Chrome,
                                     _: &gpui::MouseDownEvent,
                                     _: &mut Window,
                                     cx: &mut Context<Chrome>| {
                                        this.start_url_edit(cx);
                                    },
                                ),
                            )
                            .child(self.url_edit.as_ref().unwrap_or(&self.url_text).clone()),
                    ),
            )
            .child(btn_with_action(cx, "plus", |this, window, cx| {
                this.do_new_tab(&NewTab, window, cx)
            }));

        // ── Root layout ──
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
            .on_key_down(cx.listener(
                |this: &mut Chrome,
                 event: &gpui::KeyDownEvent,
                 _: &mut Window,
                 cx: &mut Context<Chrome>| {
                    if this.url_edit.is_some() {
                        let key = event.keystroke.key.as_str();
                        if key == "backspace" || key == "delete" {
                            let mut text = this.url_edit.clone().unwrap_or_default();
                            if !text.is_empty() {
                                text.pop();
                            }
                            this.set_url_edit(text, cx);
                        } else if key == "enter" || key == "return" {
                            this.commit_url_edit(cx);
                        } else if key == "escape" {
                            this.cancel_url_edit(cx);
                        } else if let Some(ch) = event.keystroke.key_char.as_ref() {
                            if ch.len() == 1
                                && !event.keystroke.modifiers.control
                                && !event.keystroke.modifiers.platform
                            {
                                let mut text = this.url_edit.clone().unwrap_or_default();
                                text.push_str(ch);
                                this.set_url_edit(text, cx);
                            }
                        }
                    } else {
                        #[cfg(feature = "servo-render")]
                        this.handle_key_event(event, cx);
                    }
                },
            ))
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

fn btn_with_action(
    cx: &mut Context<Chrome>,
    name: &'static str,
    on_click: impl Fn(&mut Chrome, &mut Window, &mut Context<Chrome>) + 'static,
) -> impl IntoElement {
    div()
        .size(px(BTN_SIZE))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(6.))
        .hover(|s| s.bg(rgb(HOVER)))
        .cursor_pointer()
        .on_mouse_down(
            gpui::MouseButton::Left,
            cx.listener(
                move |this: &mut Chrome,
                      _: &gpui::MouseDownEvent,
                      window: &mut Window,
                      cx: &mut Context<Chrome>| on_click(this, window, cx),
            ),
        )
        .child(make_icon(name, TEXT_MUTED))
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
                        Some(point(px(18.), px(18.)))
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
        cx.open_window(opts, |_w, cx| {
            cx.new(|cx| {
                let mut chrome = Chrome::new(cx);
                chrome.init(cx);
                chrome
            })
        })
        .unwrap();
    });
}
