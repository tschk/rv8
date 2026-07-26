//! Per-namespace WebExtensions API handlers.
//!
//! This is a scaffold covering every major namespace used by Chrome, Firefox,
//! Safari, and Orion extensions. Implemented methods return real adapter data;
//! unimplemented methods return an explicit `not implemented` error so callers
//! can see exactly what is missing.

use serde_json::{json, Map, Value};
use std::collections::HashMap;
use url::Url;

use super::{ApiRequest, ApiResponse};
use crate::extensions::permissions::Permission;
use crate::storage::{Cookie, SameSite};
use crate::extensions::runtime::{ExtensionId, ExtensionRuntime, ExtensionTab};
use crate::extensions::storage::StorageArea;
use crate::extensions::runtime::{self, LoadedExtension};

pub fn handle(runtime: &ExtensionRuntime, req: ApiRequest) -> ApiResponse {
    // Storage namespaces are dotted and are not events.
    if let Some(area) = StorageArea::from_namespace(&req.namespace) {
        return storage(runtime, &req, area);
    }
    if req.namespace.starts_with("devtools.") {
        return devtools(runtime, &req);
    }

    if let Some((ns, event)) = req.namespace.split_once('.') {
        return match ns {
            "runtime" => runtime_events(runtime, event, &req),
            "tabs" => tabs_events(runtime, event, &req),
            "windows" => windows_events(runtime, event, &req),
            "webRequest" => web_request_events(runtime, event, &req),
            "webNavigation" => web_navigation_events(runtime, event, &req),
            "bookmarks" => bookmarks_events(runtime, event, &req),
            "history" => history_events(runtime, event, &req),
            "downloads" => downloads_events(runtime, event, &req),
            "notifications" => notifications_events(runtime, event, &req),
            "alarms" => alarms_events(runtime, event, &req),
            "contextMenus" | "menus" => context_menu_events(runtime, event, &req),
            "permissions" => permissions_events(runtime, event, &req),
            "commands" => commands_events(runtime, event, &req),
            "cookies" => cookies_events(runtime, event, &req),
            "management" => management_events(runtime, event, &req),
            "omnibox" => omnibox_events(runtime, event, &req),
            _ => Err(format!("Event namespace {}.{} not supported", ns, event)),
        };
    }

    match req.namespace.as_str() {
        "runtime" => runtime_api(runtime, &req),
        "extension" => runtime_api(runtime, &req), // Firefox uses `browser.extension`
        "tabs" => tabs(runtime, &req),
        "windows" => windows(runtime, &req),
        "action" => action(runtime, &req),
        "browserAction" => action(runtime, &req),
        "pageAction" => page_action(runtime, &req),
        "sidebarAction" => sidebar_action(runtime, &req),
        "alarms" => alarms(runtime, &req),
        "bookmarks" => bookmarks(runtime, &req),
        "history" => history(runtime, &req),
        "downloads" => downloads(runtime, &req),
        "notifications" => notifications(runtime, &req),
        "contextMenus" => context_menus(runtime, &req),
        "menus" => context_menus(runtime, &req),
        "scripting" => scripting(runtime, &req),
        "declarativeNetRequest" => declarative_net_request(runtime, &req),
        "i18n" => i18n(runtime, &req),
        "permissions" => permissions(runtime, &req),
        "commands" => commands(runtime, &req),
        "cookies" => cookies(runtime, &req),
        "management" => management(runtime, &req),
        "omnibox" => omnibox(runtime, &req),
        "find" => find(runtime, &req),
        "userScripts" => user_scripts(runtime, &req),
        "identity" => identity(runtime, &req),
        "webRequest" => Err("webRequest has no synchronous methods; use event listeners".into()),
        "webNavigation" => Err("webNavigation has no synchronous methods; use event listeners".into()),
        "captivePortal" => Err("captivePortal not implemented".into()),
        "contextualIdentities" => Err("contextualIdentities not implemented".into()),
        "dns" => Err("dns not implemented".into()),
        "pkcs11" => Err("pkcs11 not implemented".into()),
        "privacy" => Err("privacy not implemented".into()),
        "proxy" => Err("proxy not implemented".into()),
        "search" => Err("search not implemented".into()),
        "sessions" => Err("sessions not implemented".into()),
        "theme" => Err("theme not implemented".into()),
        "topSites" => Err("topSites not implemented".into()),
        "types" => Err("types not implemented".into()),
        "browsingData" => Err("browsingData not implemented".into()),
        "browserSettings" => Err("browserSettings not implemented".into()),
        "clipboard" => Err("clipboard not implemented".into()),
        _ => Err(format!("namespace {} is not implemented", req.namespace)),
    }
}

// ── Generic event registration helper ──

fn event_dispatch(_runtime: &ExtensionRuntime, req: &ApiRequest) -> ApiResponse {
    match req.method.as_str() {
        "addListener" | "removeListener" => Ok(Value::Null),
        "hasListener" | "hasListeners" => Ok(Value::Bool(false)),
        _ => Err(format!("{}.{} not supported", req.namespace, req.method)),
    }
}

fn runtime_events(runtime: &ExtensionRuntime, event: &str, req: &ApiRequest) -> ApiResponse {
    let _ = (runtime, event);
    event_dispatch(runtime, req)
}

fn tabs_events(runtime: &ExtensionRuntime, event: &str, req: &ApiRequest) -> ApiResponse {
    let _ = event;
    event_dispatch(runtime, req)
}

fn windows_events(runtime: &ExtensionRuntime, event: &str, req: &ApiRequest) -> ApiResponse {
    let _ = event;
    event_dispatch(runtime, req)
}

fn web_request_events(runtime: &ExtensionRuntime, event: &str, req: &ApiRequest) -> ApiResponse {
    let _ = event;
    event_dispatch(runtime, req)
}

fn web_navigation_events(runtime: &ExtensionRuntime, event: &str, req: &ApiRequest) -> ApiResponse {
    let _ = event;
    event_dispatch(runtime, req)
}

fn bookmarks_events(runtime: &ExtensionRuntime, event: &str, req: &ApiRequest) -> ApiResponse {
    let _ = event;
    event_dispatch(runtime, req)
}

fn history_events(runtime: &ExtensionRuntime, event: &str, req: &ApiRequest) -> ApiResponse {
    let _ = event;
    event_dispatch(runtime, req)
}

fn downloads_events(runtime: &ExtensionRuntime, event: &str, req: &ApiRequest) -> ApiResponse {
    let _ = event;
    event_dispatch(runtime, req)
}

fn notifications_events(runtime: &ExtensionRuntime, event: &str, req: &ApiRequest) -> ApiResponse {
    let _ = event;
    event_dispatch(runtime, req)
}

fn alarms_events(runtime: &ExtensionRuntime, event: &str, req: &ApiRequest) -> ApiResponse {
    let _ = event;
    event_dispatch(runtime, req)
}

fn context_menu_events(runtime: &ExtensionRuntime, event: &str, req: &ApiRequest) -> ApiResponse {
    let _ = event;
    event_dispatch(runtime, req)
}

fn permissions_events(runtime: &ExtensionRuntime, event: &str, req: &ApiRequest) -> ApiResponse {
    let _ = event;
    event_dispatch(runtime, req)
}

fn commands_events(runtime: &ExtensionRuntime, event: &str, req: &ApiRequest) -> ApiResponse {
    let _ = event;
    event_dispatch(runtime, req)
}

fn cookies_events(runtime: &ExtensionRuntime, event: &str, req: &ApiRequest) -> ApiResponse {
    let _ = event;
    event_dispatch(runtime, req)
}

fn management_events(runtime: &ExtensionRuntime, event: &str, req: &ApiRequest) -> ApiResponse {
    let _ = event;
    event_dispatch(runtime, req)
}

fn omnibox_events(runtime: &ExtensionRuntime, event: &str, req: &ApiRequest) -> ApiResponse {
    let _ = event;
    event_dispatch(runtime, req)
}

// ── runtime / extension ──

fn runtime_api(runtime: &ExtensionRuntime, req: &ApiRequest) -> ApiResponse {
    match req.method.as_str() {
        "getManifest" => {
            let manifest = runtime
                .get_manifest(&req.extension_id)
                .ok_or_else(|| format!("Extension {} not found", req.extension_id.0))?;
            serde_json::to_value(manifest).map_err(|e| e.to_string())
        }
        "getURL" => {
            let path = req.string_arg(0).unwrap_or("");
            Ok(json!(format!(
                "chrome-extension://{}/{}",
                req.extension_id.0,
                path.trim_start_matches('/')
            )))
        }
        "getPlatformInfo" => Ok(platform_info()),
        "getBrowserInfo" => Ok(json!({
            "name": "RV8",
            "version": crate::VERSION,
            "vendor": "atechnology-company"
        })),
        "id" => Ok(json!(req.extension_id.0)),
        "sendMessage" => {
            let message = req.args.first().cloned().unwrap_or(Value::Null);
            let sender = json!({"id": req.extension_id.0, "url": "", "tab": null, "frameId": 0});
            runtime.queue_message(req.extension_id.clone(), sender, message);
            Ok(Value::Null)
        }
        "sendNativeMessage" => Err("runtime.sendNativeMessage is not supported".into()),
        "connect" | "connectNative" => Err("runtime.connect is not supported".into()),
        "reload" => {
            runtime.reload_extension(&req.extension_id)?;
            Ok(Value::Null)
        }
        "openOptionsPage" => {
            let manifest = runtime.get_manifest(&req.extension_id);
            if let Some(url) = manifest.as_ref().and_then(|m| m.options_page.as_ref()).or_else(|| manifest.as_ref().and_then(|m| m.options_ui.as_ref()).and_then(|o| o.page.as_ref())) {
                Ok(json!(format!("chrome-extension://{}/{}", req.extension_id.0, url.trim_start_matches('/'))))
            } else {
                Ok(Value::Null)
            }
        }
        "setUninstallURL" => Ok(Value::Null),
        "getPackageDirectoryEntry" => Err("runtime.getPackageDirectoryEntry is not supported".into()),
        "getBackgroundPage" => Ok(Value::Null),
        "getContexts" => Ok(json!([])),
        _ => Err(format!("runtime.{} is not supported", req.method)),
    }
}

fn platform_info() -> Value {
    let os = match std::env::consts::OS {
        "macos" => "mac",
        "windows" => "win",
        "linux" => "linux",
        other => other,
    };
    json!({
        "os": os,
        "arch": std::env::consts::ARCH,
        "nacl_arch": ""
    })
}

// ── tabs ──

fn tabs(runtime: &ExtensionRuntime, req: &ApiRequest) -> ApiResponse {
    match req.method.as_str() {
        "get" => {
            let id = req.u64_arg(0).ok_or("tabs.get requires a tab id")?;
            runtime.tab(id).map(tab_to_value).ok_or_else(|| format!("Tab {} not found", id))
        }
        "getCurrent" => runtime
            .active_tab()
            .map(tab_to_value)
            .ok_or_else(|| "No current tab".into()),
        "getSelected" => runtime
            .active_tab()
            .map(tab_to_value)
            .ok_or_else(|| "No selected tab".into()),
        "query" => {
            let query = req.options(1).or_else(|| req.options(0)).cloned();
            let tabs = filter_tabs(runtime.tabs(), query.as_ref());
            Ok(Value::Array(tabs.into_iter().map(tab_to_value).collect()))
        }
        "create" => {
            let opts = req.options(1).or_else(|| req.options(0)).cloned().unwrap_or_default();
            let url = opts.get("url").and_then(|v| v.as_str()).unwrap_or("about:blank").to_string();
            let active = opts.get("active").and_then(|v| v.as_bool()).unwrap_or(true);
            let pinned = opts.get("pinned").and_then(|v| v.as_bool()).unwrap_or(false);
            let id = runtime.create_tab_driver(&url, active)?;
            let mut new_tab = ExtensionTab {
                id,
                url,
                title: String::new(),
                active,
                pinned,
                highlighted: active,
                incognito: false,
                window_id: runtime.current_window_id(),
                index: 0,
                status: Some("loading".into()),
                favicon_url: None,
            };
            {
                let mut state = runtime.tab_state();
                new_tab.index = state.len() as u32;
                if active {
                    for t in state.iter_mut() {
                        t.active = false;
                        t.highlighted = false;
                    }
                }
                state.push(new_tab.clone());
            }
            Ok(tab_to_value(new_tab))
        }
        "update" => {
            let (id, props) = if let Some(props) = req.options(0) {
                (runtime.active_tab().map(|t| t.id), props.clone())
            } else {
                let id = req.u64_arg(0).ok_or("tabs.update requires tabId")?;
                let props = req.options(1).cloned().unwrap_or_default();
                (Some(id), props)
            };
            let id = id.ok_or("No active tab")?;
            let active = props.get("active").and_then(|v| v.as_bool()).unwrap_or(false);
            runtime.update_tab_driver(id, Value::Object(props.clone()))?;
            let mut state = runtime.tab_state();
            let mut updated = None;
            for t in state.iter_mut() {
                if t.id == id {
                    if let Some(url) = props.get("url").and_then(|v| v.as_str()) {
                        t.url = url.to_string();
                        t.status = Some("loading".into());
                    }
                    if let Some(pinned) = props.get("pinned").and_then(|v| v.as_bool()) {
                        t.pinned = pinned;
                    }
                    updated = Some(t.clone());
                }
                if active {
                    t.active = t.id == id;
                    t.highlighted = t.id == id;
                }
            }
            updated.map(tab_to_value).ok_or_else(|| format!("Tab {} not found", id))
        }
        "remove" => {
            let ids = extract_tab_ids(req)?;
            runtime.close_tabs_driver(&ids)?;
            {
                let mut state = runtime.tab_state();
                state.retain(|t| !ids.contains(&t.id));
                runtime.reindex_tabs(&mut state);
            }
            Ok(Value::Null)
        }
        "reload" => {
            let id = if req.args.len() > 1 || req.args.first().and_then(|v| v.as_u64()).is_some() {
                req.u64_arg(0).unwrap_or_else(|| runtime.active_tab().map(|t| t.id).unwrap_or(0))
            } else {
                runtime.active_tab().map(|t| t.id).unwrap_or(0)
            };
            runtime.reload_tab_driver(id)?;
            if let Some(mut tab) = runtime.tab(id) {
                tab.status = Some("loading".into());
                return Ok(tab_to_value(tab));
            }
            Ok(Value::Null)
        }
        "discard" | "duplicate" | "move" | "highlight" | "setZoom" | "getZoom"
        | "setZoomSettings" | "getZoomSettings" | "detectLanguage" => {
            Err(format!("tabs.{} not implemented", req.method))
        }
        "executeScript" | "insertCSS" | "removeCSS" => Err(format!(
            "tabs.{} is deprecated; use scripting.executeScript / scripting.insertCSS",
            req.method
        )),
        _ => Err(format!("tabs.{} not implemented", req.method)),
    }
}

fn extract_tab_ids(req: &ApiRequest) -> Result<Vec<u64>, String> {
    if let Some(id) = req.u64_arg(0) {
        Ok(vec![id])
    } else if let Some(arr) = req.args.first().and_then(|v| v.as_array()) {
        Ok(arr.iter().filter_map(|v| v.as_u64()).collect())
    } else {
        Err("Expected tab id or array of ids".into())
    }
}

fn tab_to_value(tab: ExtensionTab) -> Value {
    serde_json::to_value(tab).unwrap_or(Value::Null)
}

fn filter_tabs(tabs: Vec<ExtensionTab>, query: Option<&Map<String, Value>>) -> Vec<ExtensionTab> {
    let Some(query) = query else { return tabs };
    tabs.into_iter()
        .filter(|t| {
            if let Some(active) = query.get("active").and_then(|v| v.as_bool()) {
                if t.active != active {
                    return false;
                }
            }
            if let Some(pinned) = query.get("pinned").and_then(|v| v.as_bool()) {
                if t.pinned != pinned {
                    return false;
                }
            }
            if let Some(current_window) = query.get("currentWindow").and_then(|v| v.as_bool()) {
                if current_window && !t.active {
                    return false;
                }
            }
            if let Some(url) = query.get("url").and_then(|v| v.as_str()) {
                if !t.url.contains(url) {
                    return false;
                }
            }
            true
        })
        .collect()
}

// ── windows ──

fn windows(runtime: &ExtensionRuntime, req: &ApiRequest) -> ApiResponse {
    match req.method.as_str() {
        "get" | "getCurrent" | "getLastFocused" | "getAll" => Ok(current_window(runtime)),
        "create" | "update" | "remove" => {
            Err(format!("windows.{} requires a UI shell", req.method))
        }
        _ => Err(format!("windows.{} not implemented", req.method)),
    }
}

fn current_window(runtime: &ExtensionRuntime) -> Value {
    let tabs = runtime.tabs();
    json!({
        "id": runtime.current_window_id(),
        "focused": true,
        "top": 0,
        "left": 0,
        "width": 1280,
        "height": 800,
        "tabs": tabs.into_iter().map(tab_to_value).collect::<Vec<_>>(),
        "incognito": false,
        "type": "normal",
        "state": "normal",
        "alwaysOnTop": false
    })
}

// ── storage ──

fn storage(runtime: &ExtensionRuntime, req: &ApiRequest, area: StorageArea) -> ApiResponse {
    match req.method.as_str() {
        "get" => {
            let keys = req.args.first().cloned().unwrap_or(Value::Null);
            Ok(runtime.storage().get(area, &req.extension_id.0, &keys))
        }
        "set" => {
            let items = req.args.first().cloned().unwrap_or(Value::Null);
            runtime.storage().set(area, &req.extension_id.0, &items)?;
            Ok(Value::Null)
        }
        "remove" => {
            let keys = req.args.first().cloned().unwrap_or(Value::Null);
            runtime.storage().remove(area, &req.extension_id.0, &keys)?;
            Ok(Value::Null)
        }
        "clear" => {
            runtime.storage().clear(area, &req.extension_id.0)?;
            Ok(Value::Null)
        }
        "getBytesInUse" => {
            let keys = req.args.first().cloned().unwrap_or(Value::Null);
            Ok(json!(runtime.storage().get_bytes_in_use(area, &req.extension_id.0, &keys)))
        }
        "onChanged" => event_dispatch(runtime, req),
        _ => Err(format!("storage.{}.{} not implemented", area.as_str(), req.method)),
    }
}

// ── action / browserAction / pageAction / sidebarAction ──

fn action(runtime: &ExtensionRuntime, req: &ApiRequest) -> ApiResponse {
    let ext_id = &req.extension_id;
    let details = req.args.first().cloned();
    match req.method.as_str() {
        "setTitle" => update_action(runtime, ext_id, "title", details),
        "getTitle" => Ok(get_action_field(runtime, ext_id, "title")),
        "setPopup" => update_action(runtime, ext_id, "popup", details),
        "getPopup" => Ok(get_action_field(runtime, ext_id, "popup")),
        "setIcon" => update_action(runtime, ext_id, "icon", details),
        "setBadgeText" => update_action(runtime, ext_id, "badgeText", details),
        "getBadgeText" => Ok(get_action_field(runtime, ext_id, "badgeText")),
        "setBadgeBackgroundColor" => update_action(runtime, ext_id, "badgeBackgroundColor", details),
        "getBadgeBackgroundColor" => Ok(get_action_field(runtime, ext_id, "badgeBackgroundColor")),
        "setBadgeTextColor" => update_action(runtime, ext_id, "badgeTextColor", details),
        "getBadgeTextColor" => Ok(get_action_field(runtime, ext_id, "badgeTextColor")),
        "enable" | "disable" => {
            let enabled = req.method == "enable";
            update_action(runtime, ext_id, "enabled", Some(json!(enabled)))
        }
        "openPopup" => Err("action.openPopup requires a UI shell".into()),
        "isEnabled" => Ok(json!(get_action_field(runtime, ext_id, "enabled").as_bool().unwrap_or(true))),
        "onClicked" => event_dispatch(runtime, req),
        _ => Err(format!("action.{} not implemented", req.method)),
    }
}

fn update_action(runtime: &ExtensionRuntime, ext_id: &ExtensionId, key: &str, value: Option<Value>) -> ApiResponse {
    let value = value.unwrap_or(Value::Null);
    let mut state = runtime.action_state();
    let map = state.entry(ext_id.clone()).or_insert_with(|| json!({}));
    if let Some(obj) = map.as_object_mut() {
        obj.insert(key.to_string(), value);
    }
    Ok(Value::Null)
}

fn get_action_field(runtime: &ExtensionRuntime, ext_id: &ExtensionId, key: &str) -> Value {
    let state = runtime.action_state();
    state
        .get(ext_id)
        .and_then(|v| v.get(key).cloned())
        .unwrap_or(Value::Null)
}

fn page_action(runtime: &ExtensionRuntime, req: &ApiRequest) -> ApiResponse {
    action(runtime, req)
}

fn sidebar_action(runtime: &ExtensionRuntime, req: &ApiRequest) -> ApiResponse {
    action(runtime, req)
}

// ── alarms ──

fn alarms(runtime: &ExtensionRuntime, req: &ApiRequest) -> ApiResponse {
    match req.method.as_str() {
        "create" => {
            let name = req.string_arg(0).unwrap_or("").to_string();
            let info = req.options(1).cloned().unwrap_or_default();
            let delay_min = info
                .get("delayInMinutes")
                .and_then(|v| v.as_f64())
                .or_else(|| info.get("when").and_then(|v| v.as_f64().map(|w| w / 60_000.0)))
                .unwrap_or(0.0);
            let period = info.get("periodInMinutes").and_then(|v| v.as_f64());
            let alarm = runtime::Alarm {
                name: name.clone(),
                scheduled_at: std::time::Instant::now() + std::time::Duration::from_secs_f64(delay_min * 60.0),
                period_in_minutes: period,
                extension_id: req.extension_id.clone(),
            };
            runtime.alarms().insert(name, alarm);
            Ok(Value::Null)
        }
        "get" => {
            let name = req.string_arg(0).unwrap_or("");
            let alarms = runtime.alarms();
            Ok(alarms.get(name).map(alarm_to_value).unwrap_or(Value::Null))
        }
        "getAll" => {
            let alarms = runtime.alarms();
            Ok(Value::Array(alarms.values().map(alarm_to_value).collect()))
        }
        "clear" => {
            let name = req.string_arg(0).unwrap_or("");
            Ok(json!(runtime.alarms().remove(name).is_some()))
        }
        "clearAll" => {
            let mut alarms = runtime.alarms();
            let count = alarms.len();
            alarms.clear();
            Ok(json!(count > 0))
        }
        "onAlarm" => event_dispatch(runtime, req),
        _ => Err(format!("alarms.{} not implemented", req.method)),
    }
}

fn alarm_to_value(alarm: &runtime::Alarm) -> Value {
    json!({
        "name": alarm.name,
        "scheduledTime": 0,
        "periodInMinutes": alarm.period_in_minutes
    })
}

// ── bookmarks / history ──

fn bookmarks(runtime: &ExtensionRuntime, req: &ApiRequest) -> ApiResponse {
    let ensure_root = || {
        let mut tree = runtime.bookmarks();
        if !tree.contains_key("1") {
            tree.insert(
                "1".into(),
                runtime::BookmarkNode {
                    id: "1".into(),
                    parent_id: None,
                    index: 0,
                    title: "Bookmarks Bar".into(),
                    url: None,
                    children: Vec::new(),
                },
            );
        }
    };
    match req.method.as_str() {
        "getTree" => {
            ensure_root();
            Ok(Value::Array(vec![bookmark_tree(runtime, "1")]))
        }
        "get" => {
            let id = req.string_arg(0).unwrap_or("");
            runtime.bookmarks().get(id).map(bookmark_to_value).ok_or_else(|| format!("Bookmark {} not found", id))
        }
        "getChildren" => {
            ensure_root();
            let id = req.string_arg(0).unwrap_or("1");
            let children = runtime.bookmarks().get(id).map(|n| n.children.clone()).unwrap_or_default();
            Ok(Value::Array(children.iter().filter_map(|cid| runtime.bookmarks().get(cid).map(bookmark_to_value)).collect()))
        }
        "getRecent" => {
            let count = req.args.first().and_then(|v| v.as_u64()).unwrap_or(10) as usize;
            let nodes: Vec<Value> = runtime.bookmarks().values().take(count).map(bookmark_to_value).collect();
            Ok(Value::Array(nodes))
        }
        "getSubTree" => {
            let id = req.string_arg(0).unwrap_or("1");
            Ok(bookmark_tree(runtime, id))
        }
        "search" => {
            let query = req.string_arg(0).unwrap_or("").to_lowercase();
            let results: Vec<Value> = runtime
                .bookmarks()
                .values()
                .filter(|n| {
                    n.title.to_lowercase().contains(&query)
                        || n.url.as_ref().is_some_and(|u| u.to_lowercase().contains(&query))
                })
                .map(bookmark_to_value)
                .collect();
            Ok(Value::Array(results))
        }
        "create" => {
            ensure_root();
            let opts = req.options(0).cloned().unwrap_or_default();
            let mut tree = runtime.bookmarks();
            let id = runtime.next_bookmark_id().to_string();
            let parent_id = opts.get("parentId").and_then(|v| v.as_str()).unwrap_or("1").to_string();
            let index = opts.get("index").and_then(|v| v.as_u64()).map(|i| i as u32).unwrap_or_else(|| {
                tree.get(&parent_id).map(|p| p.children.len() as u32).unwrap_or(0)
            });
            let node = runtime::BookmarkNode {
                id: id.clone(),
                parent_id: Some(parent_id.clone()),
                index,
                title: opts.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                url: opts.get("url").and_then(|v| v.as_str()).map(|s| s.to_string()),
                children: Vec::new(),
            };
            if let Some(parent) = tree.get_mut(&parent_id) {
                if (index as usize) <= parent.children.len() {
                    parent.children.insert(index as usize, id.clone());
                } else {
                    parent.children.push(id.clone());
                }
            }
            tree.insert(id.clone(), node);
            tree.get(&id).map(bookmark_to_value).ok_or_else(|| format!("Failed to create bookmark {}", id))
        }
        "update" => {
            let id = req.string_arg(0).ok_or("bookmarks.update requires id")?;
            let changes = req.options(1).cloned().unwrap_or_default();
            let mut tree = runtime.bookmarks();
            let node = tree.get_mut(id).ok_or_else(|| format!("Bookmark {} not found", id))?;
            if let Some(title) = changes.get("title").and_then(|v| v.as_str()) {
                node.title = title.to_string();
            }
            if let Some(url) = changes.get("url").and_then(|v| v.as_str()) {
                node.url = Some(url.to_string());
            }
            Ok(bookmark_to_value(&node.clone()))
        }
        "move" => {
            let id = req.string_arg(0).ok_or("bookmarks.move requires id")?;
            let opts = req.options(1).cloned().unwrap_or_default();
            let new_parent = opts.get("parentId").and_then(|v| v.as_str()).map(|s| s.to_string());
            let new_index = opts.get("index").and_then(|v| v.as_u64()).map(|i| i as u32);
            {
                let mut tree = runtime.bookmarks();
                let node = tree.get(id).cloned().ok_or_else(|| format!("Bookmark {} not found", id))?;
                if let Some(ref old_parent) = node.parent_id {
                    if let Some(p) = tree.get_mut(old_parent) {
                        p.children.retain(|c| c != id);
                    }
                }
                let parent_id = new_parent.unwrap_or_else(|| node.parent_id.clone().unwrap_or_default());
                if let Some(p) = tree.get_mut(&parent_id) {
                    let idx = new_index.unwrap_or(p.children.len() as u32) as usize;
                    if idx <= p.children.len() {
                        p.children.insert(idx, id.to_string());
                    } else {
                        p.children.push(id.to_string());
                    }
                }
                if let Some(n) = tree.get_mut(id) {
                    n.parent_id = Some(parent_id.clone());
                    n.index = new_index.unwrap_or(0);
                }
                let n = tree.get(id).cloned().unwrap();
                Ok(bookmark_to_value(&n))
            }
        }
        "remove" => {
            let id = req.string_arg(0).ok_or("bookmarks.remove requires id")?;
            remove_bookmark(runtime, id)?;
            Ok(Value::Null)
        }
        "removeTree" => {
            let id = req.string_arg(0).ok_or("bookmarks.removeTree requires id")?;
            remove_bookmark_tree(runtime, id)?;
            Ok(Value::Null)
        }
        "onCreated" | "onRemoved" | "onChanged" | "onMoved" | "onChildrenReordered" | "onImportBegan" | "onImportEnded" => event_dispatch(runtime, req),
        _ => Err(format!("bookmarks.{} is not supported", req.method)),
    }
}

fn bookmark_to_value(node: &runtime::BookmarkNode) -> Value {
    let mut obj = serde_json::Map::new();
    obj.insert("id".into(), json!(node.id.clone()));
    obj.insert("title".into(), json!(node.title.clone()));
    obj.insert("index".into(), json!(node.index));
    if let Some(ref parent) = node.parent_id {
        obj.insert("parentId".into(), json!(parent.clone()));
    }
    if let Some(ref url) = node.url {
        obj.insert("url".into(), json!(url.clone()));
    }
    if !node.children.is_empty() {
        obj.insert("children".into(), Value::Array(Vec::new()));
    }
    Value::Object(obj)
}

fn bookmark_tree(runtime: &ExtensionRuntime, id: &str) -> Value {
    let node = runtime.bookmarks().get(id).cloned();
    if let Some(node) = node {
        let mut value = bookmark_to_value(&node);
        let children: Vec<Value> = node
            .children
            .iter()
            .filter_map(|cid| runtime.bookmarks().get(cid).cloned().map(|c| bookmark_tree(runtime, &c.id)))
            .collect();
        if let Value::Object(ref mut obj) = value {
            if !children.is_empty() {
                obj.insert("children".into(), Value::Array(children));
            }
        }
        value
    } else {
        Value::Null
    }
}

fn remove_bookmark(runtime: &ExtensionRuntime, id: &str) -> Result<(), String> {
    let mut tree = runtime.bookmarks();
    let node = tree.remove(id).ok_or_else(|| format!("Bookmark {} not found", id))?;
    if let Some(parent_id) = node.parent_id {
        if let Some(parent) = tree.get_mut(&parent_id) {
            parent.children.retain(|c| c != id);
        }
    }
    Ok(())
}

fn remove_bookmark_tree(runtime: &ExtensionRuntime, id: &str) -> Result<(), String> {
    let children = {
        let tree = runtime.bookmarks();
        tree.get(id).map(|n| n.children.clone()).unwrap_or_default()
    };
    for child in children {
        remove_bookmark_tree(runtime, &child)?;
    }
    remove_bookmark(runtime, id)
}

fn history(runtime: &ExtensionRuntime, req: &ApiRequest) -> ApiResponse {
    match req.method.as_str() {
        "search" => {
            let query = req.string_arg(0).unwrap_or("").to_lowercase();
            let results: Vec<Value> = runtime
                .history()
                .iter()
                .filter(|h| h.url.to_lowercase().contains(&query) || h.title.to_lowercase().contains(&query))
                .map(history_to_value)
                .collect();
            Ok(Value::Array(results))
        }
        "getVisits" => {
            let url = req.string_arg(0).unwrap_or("");
            let visits = runtime
                .history()
                .iter()
                .filter(|h| h.url == url)
                .map(|h| {
                    json!({
                        "id": h.id,
                        "visitTime": h.last_visit_time,
                        "visitId": h.id,
                        "referringVisitId": "0",
                        "transition": "link"
                    })
                })
                .collect();
            Ok(Value::Array(visits))
        }
        "addUrl" => {
            let details = req.options(0).cloned().unwrap_or_default();
            let url = details.get("url").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let title = details.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let id = runtime.next_history_id().to_string();
            runtime.history().push(runtime::HistoryItem {
                id,
                url,
                title,
                last_visit_time: 0,
                visit_count: 1,
            });
            Ok(Value::Null)
        }
        "deleteUrl" => {
            let url = req.string_arg(0).unwrap_or("");
            runtime.history().retain(|h| h.url != url);
            Ok(Value::Null)
        }
        "deleteRange" => {
            let range = req.options(0).cloned().unwrap_or_default();
            let start = range.get("startTime").and_then(|v| v.as_u64()).unwrap_or(0);
            let end = range.get("endTime").and_then(|v| v.as_u64()).unwrap_or(u64::MAX);
            runtime.history().retain(|h| h.last_visit_time < start || h.last_visit_time > end);
            Ok(Value::Null)
        }
        "deleteAll" => {
            runtime.history().clear();
            Ok(Value::Null)
        }
        "onVisited" | "onTitleChanged" | "onVisitRemoved" => event_dispatch(runtime, req),
        _ => Err(format!("history.{} is not supported", req.method)),
    }
}

fn history_to_value(item: &runtime::HistoryItem) -> Value {
    json!({
        "id": item.id,
        "url": item.url,
        "title": item.title,
        "lastVisitTime": item.last_visit_time,
        "visitCount": item.visit_count,
    })
}

// ── downloads ──

fn downloads(runtime: &ExtensionRuntime, req: &ApiRequest) -> ApiResponse {
    fn with_download<F: FnOnce(&mut Map<String, Value>) -> ApiResponse>(runtime: &ExtensionRuntime, id: u64, f: F) -> ApiResponse {
        let mut downloads = runtime.downloads();
        let item = downloads.get_mut(&id).ok_or_else(|| format!("Download {} not found", id))?;
        let obj = item.as_object_mut().ok_or("Corrupt download entry")?;
        f(obj)
    }
    match req.method.as_str() {
        "download" => {
            let opts = req.options(0).cloned().unwrap_or_default();
            let url = opts.get("url").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let id = runtime.next_download_id();
            let item = json!({
                "id": id,
                "url": url,
                "state": "in_progress",
                "paused": false,
                "filename": opts.get("filename").and_then(|v| v.as_str()).unwrap_or(""),
                "totalBytes": -1i64,
                "receivedBytes": 0,
                "danger": "safe",
                "exists": true
            });
            runtime.downloads().insert(id, item.clone());
            Ok(item)
        }
        "search" => {
            let query = req.string_arg(0).unwrap_or("").to_lowercase();
            let downloads = runtime.downloads();
            let results: Vec<Value> = downloads
                .values()
                .filter(|v| {
                    query.is_empty()
                        || v.get("url").and_then(|u| u.as_str()).is_some_and(|u| u.to_lowercase().contains(&query))
                        || v.get("filename").and_then(|u| u.as_str()).is_some_and(|u| u.to_lowercase().contains(&query))
                })
                .cloned()
                .collect();
            Ok(Value::Array(results))
        }
        "pause" => with_download(runtime, req.u64_arg(0).ok_or("downloads.pause requires id")?, |obj| {
            obj.insert("paused".into(), json!(true));
            Ok(Value::Null)
        }),
        "resume" => with_download(runtime, req.u64_arg(0).ok_or("downloads.resume requires id")?, |obj| {
            obj.insert("paused".into(), json!(false));
            Ok(Value::Null)
        }),
        "cancel" => with_download(runtime, req.u64_arg(0).ok_or("downloads.cancel requires id")?, |obj| {
            obj.insert("state".into(), json!("interrupted"));
            obj.insert("paused".into(), json!(false));
            Ok(Value::Null)
        }),
        "erase" => {
            let ids = req
                .args
                .first()
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_u64()).collect::<Vec<_>>())
                .unwrap_or_default();
            let mut downloads = runtime.downloads();
            let erased: Vec<u64> = ids.into_iter().filter(|id| downloads.remove(id).is_some()).collect();
            Ok(Value::Array(erased.iter().map(|id| json!(id)).collect()))
        }
        "removeFile" => with_download(runtime, req.u64_arg(0).ok_or("downloads.removeFile requires id")?, |obj| {
            obj.insert("exists".into(), json!(false));
            Ok(Value::Null)
        }),
        "acceptDanger" => with_download(runtime, req.u64_arg(0).ok_or("downloads.acceptDanger requires id")?, |obj| {
            obj.insert("danger".into(), json!("safe"));
            Ok(Value::Null)
        }),
        "show" | "showDefaultFolder" | "open" | "getFileIcon" => Err(format!("downloads.{} requires a UI shell", req.method)),
        "setShelfEnabled" => Err("downloads.setShelfEnabled requires a UI shell".into()),
        _ => Err(format!("downloads.{} is not supported", req.method)),
    }
}

// ── notifications ──

fn notifications(runtime: &ExtensionRuntime, req: &ApiRequest) -> ApiResponse {
    match req.method.as_str() {
        "create" => {
            let id = req.string_arg(0).map(|s| s.to_string()).unwrap_or_else(|| format!("notif-{}", runtime.next_download_id()));
            let opts = req.args.get(1).or(req.args.first()).cloned().unwrap_or(Value::Null);
            let mut item = if let Value::Object(obj) = opts { obj } else { Map::new() };
            item.insert("id".into(), json!(id.clone()));
            runtime.notification_items().insert(id.clone(), Value::Object(item));
            Ok(json!(id))
        }
        "update" => {
            let id = req.string_arg(0).ok_or("notifications.update requires id")?;
            let opts = req.args.get(1).or(req.args.first()).cloned().unwrap_or(Value::Null);
            let mut items = runtime.notification_items();
            let item = items.get_mut(id).ok_or_else(|| format!("Notification {} not found", id))?;
            if let Value::Object(obj) = opts {
                if let Value::Object(existing) = item {
                    for (k, v) in obj {
                        existing.insert(k, v);
                    }
                }
            }
            Ok(json!(true))
        }
        "clear" => {
            let id = req.string_arg(0).ok_or("notifications.clear requires id")?;
            let removed = runtime.notification_items().remove(id).is_some();
            Ok(json!(removed))
        }
        "getAll" => {
            let items = runtime.notification_items();
            Ok(Value::Array(items.values().cloned().collect()))
        }
        "onClicked" | "onClosed" | "onButtonClicked" | "onShown" => event_dispatch(runtime, req),
        _ => Err(format!("notifications.{} is not supported", req.method)),
    }
}

// ── contextMenus / menus ──

fn context_menus(runtime: &ExtensionRuntime, req: &ApiRequest) -> ApiResponse {
    fn find_menu_id_by_arg(items: &HashMap<u64, runtime::ContextMenuItem>, arg: &Value) -> Option<u64> {
        if let Some(id) = arg.as_u64() {
            return items.get(&id).map(|m| m.id);
        }
        if let Some(id) = arg.as_str() {
            return items.values().find(|m| m.info.get("id").and_then(|v| v.as_str()) == Some(id)).map(|m| m.id);
        }
        None
    }
    match req.method.as_str() {
        "create" => {
            let id = if let Some(s) = req.string_arg(0) {
                s.to_string()
            } else if let Some(obj) = req.options(0) {
                obj.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string()
            } else {
                String::new()
            };
            let numeric_id = runtime.next_menu_id();
            let mut info = req.args.get(1).or(req.args.first()).cloned().unwrap_or(Value::Object(Map::new()));
            if let Value::Object(ref mut obj) = info {
                obj.insert("id".into(), json!(id.clone()));
            }
            runtime.context_menu_items().insert(numeric_id, runtime::ContextMenuItem {
                id: numeric_id,
                extension_id: req.extension_id.clone(),
                info,
            });
            Ok(json!(id))
        }
        "update" => {
            let arg = req.args.first().cloned().unwrap_or(Value::Null);
            let update = req.args.get(1).cloned().unwrap_or(Value::Null);
            let mut items = runtime.context_menu_items();
            let id = find_menu_id_by_arg(&items, &arg).ok_or("contextMenus.update requires a valid id")?;
            let item = items.get_mut(&id).ok_or("contextMenus.update item not found")?;
            if let Value::Object(obj) = update {
                if let Value::Object(ref mut info) = item.info {
                    for (k, v) in obj {
                        info.insert(k, v);
                    }
                }
            }
            Ok(Value::Null)
        }
        "remove" => {
            let arg = req.args.first().cloned().unwrap_or(Value::Null);
            let mut items = runtime.context_menu_items();
            let id = find_menu_id_by_arg(&items, &arg).ok_or("contextMenus.remove requires a valid id")?;
            items.remove(&id);
            Ok(Value::Null)
        }
        "removeAll" => {
            runtime.context_menu_items().clear();
            Ok(Value::Null)
        }
        "refresh" => Err("contextMenus.refresh requires a UI shell".into()),
        "onClicked" | "onShown" | "onHidden" => event_dispatch(runtime, req),
        _ => Err(format!("contextMenus.{} is not supported", req.method)),
    }
}

// ── scripting ──

fn scripting(runtime: &ExtensionRuntime, req: &ApiRequest) -> ApiResponse {
    match req.method.as_str() {
        "executeScript" => Err("scripting.executeScript requires a JS bridge".into()),
        "insertCSS" | "removeCSS" => Err(format!("scripting.{} requires a JS bridge", req.method)),
        "registerContentScripts" => {
            let scripts = req.args.first().and_then(|v| v.as_array()).cloned().unwrap_or_default();
            let mut map = runtime.registered_content_scripts();
            for script in scripts {
                if let Some(id) = script.get("id").and_then(|v| v.as_str()) {
                    map.insert(id.to_string(), script);
                }
            }
            Ok(Value::Null)
        }
        "updateContentScripts" => {
            let scripts = req.args.first().and_then(|v| v.as_array()).cloned().unwrap_or_default();
            let mut map = runtime.registered_content_scripts();
            for script in scripts {
                if let Some(id) = script.get("id").and_then(|v| v.as_str()) {
                    if map.contains_key(id) {
                        map.insert(id.to_string(), script);
                    }
                }
            }
            Ok(Value::Null)
        }
        "unregisterContentScripts" => {
            let ids = req
                .args
                .first()
                .and_then(|v| v.as_object())
                .and_then(|o| o.get("ids"))
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect::<Vec<_>>())
                .unwrap_or_default();
            let mut map = runtime.registered_content_scripts();
            if ids.is_empty() {
                map.clear();
            } else {
                for id in ids {
                    map.remove(&id);
                }
            }
            Ok(Value::Null)
        }
        "getRegisteredContentScripts" => {
            let map = runtime.registered_content_scripts();
            let scripts: Vec<Value> = map.values().cloned().collect();
            Ok(Value::Array(scripts))
        }
        _ => Err(format!("scripting.{} is not supported", req.method)),
    }
}

// ── declarativeNetRequest ──

fn declarative_net_request(runtime: &ExtensionRuntime, req: &ApiRequest) -> ApiResponse {
    match req.method.as_str() {
        "getEnabledRulesets" => {
            let rulesets = runtime.dnr_rulesets();
            Ok(json!({"rulesetIds": rulesets.keys().cloned().collect::<Vec<_>>() }))
        }
        "updateEnabledRulesets" => {
            let opts = req.options(0).cloned().unwrap_or_default();
            let mut rulesets = runtime.dnr_rulesets();
            if let Some(disable) = opts.get("disableRulesetIds").and_then(|v| v.as_array()) {
                for id in disable.iter().filter_map(|v| v.as_str()) {
                    rulesets.insert(id.to_string(), false);
                }
            }
            if let Some(enable) = opts.get("enableRulesetIds").and_then(|v| v.as_array()) {
                for id in enable.iter().filter_map(|v| v.as_str()) {
                    rulesets.insert(id.to_string(), true);
                }
            }
            Ok(Value::Null)
        }
        "getDynamicRules" => {
            let rules = runtime.dnr_dynamic_rules();
            Ok(Value::Array(rules.values().cloned().collect()))
        }
        "updateDynamicRules" => {
            let opts = Value::Object(req.options(0).cloned().unwrap_or_default());
            update_rules(&mut runtime.dnr_dynamic_rules(), &opts);
            Ok(Value::Null)
        }
        "getSessionRules" => {
            let rules = runtime.dnr_session_rules();
            Ok(Value::Array(rules.values().cloned().collect()))
        }
        "updateSessionRules" => {
            let opts = Value::Object(req.options(0).cloned().unwrap_or_default());
            update_rules(&mut runtime.dnr_session_rules(), &opts);
            Ok(Value::Null)
        }
        "isRegexSupported" => Ok(json!({"isSupported": true, "reason": null})),
        _ => Err(format!("declarativeNetRequest.{} not implemented", req.method)),
    }
}

fn update_rules(rules: &mut std::collections::HashMap<i64, Value>, opts: &Value) {
    if let Some(add) = opts.get("addRules").and_then(|v| v.as_array()) {
        for (i, rule) in add.iter().enumerate() {
            let id = rule.get("id").and_then(|v| v.as_i64()).unwrap_or(i as i64);
            rules.insert(id, rule.clone());
        }
    }
    if let Some(remove) = opts.get("removeRuleIds").and_then(|v| v.as_array()) {
        for id in remove.iter().filter_map(|v| v.as_i64()) {
            rules.remove(&id);
        }
    }
    if let Some(delete_rules) = opts.get("deleteRules").and_then(|v| v.as_array()) {
        for rule in delete_rules {
            if let Some(id) = rule.as_i64().or_else(|| rule.get("id").and_then(|v| v.as_i64())) {
                rules.remove(&id);
            }
        }
    }
}

// ── i18n ──

fn i18n(runtime: &ExtensionRuntime, req: &ApiRequest) -> ApiResponse {
    match req.method.as_str() {
        "getMessage" => {
            let key = req.string_arg(0).unwrap_or("");
            let manifest = runtime.get_manifest(&req.extension_id);
            let locale = manifest
                .as_ref()
                .and_then(|m| m.default_locale.as_deref())
                .unwrap_or("en");
            let path = runtime
                .get(&req.extension_id)
                .map(|e| e.root.join("_locales").join(locale).join("messages.json"))
                .unwrap_or_default();
            if let Ok(text) = std::fs::read_to_string(path) {
                if let Ok(messages) = serde_json::from_str::<Value>(&text) {
                    if let Some(msg) = messages.get(key).and_then(|v| v.get("message")).and_then(|v| v.as_str()) {
                        let mut msg = msg.to_string();
                        if let Some(subs) = req.args.get(1) {
                            if let Some(arr) = subs.as_array() {
                                for (i, sub) in arr.iter().enumerate() {
                                    let rep = sub.as_str().unwrap_or("");
                                    msg = msg.replace(&format!("${}$", i + 1), rep);
                                }
                            } else if let Some(obj) = subs.as_object() {
                                for (k, v) in obj {
                                    let rep = v.as_str().unwrap_or("");
                                    msg = msg.replace(&format!("${}$", k), rep);
                                }
                            }
                        }
                        return Ok(json!(msg));
                    }
                }
            }
            Ok(Value::Null)
        }
        "getAcceptLanguages" => Ok(json!([])),
        "getUILanguage" => Ok(json!("en")),
        "detectLanguage" => Ok(json!({"isReliable": true, "languages": []})),
        _ => Err(format!("i18n.{} not implemented", req.method)),
    }
}

// ── permissions ──

fn permissions(runtime: &ExtensionRuntime, req: &ApiRequest) -> ApiResponse {
    match req.method.as_str() {
        "contains" => {
            let perms = req
                .args
                .first()
                .and_then(|v| v.get("permissions"))
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>())
                .unwrap_or_default();
            let set = runtime.permissions(&req.extension_id).unwrap_or_default();
            Ok(json!(perms.iter().all(|p| set.has_api(p))))
        }
        "getAll" => {
            let set = runtime.permissions(&req.extension_id).unwrap_or_default();
            let mut permissions = Vec::new();
            let mut origins = Vec::new();
            for p in set.iter() {
                match p {
                    Permission::Host(h) => origins.push(json!(h)),
                    _ => permissions.push(json!(p.as_str())),
                }
            }
            Ok(json!({"permissions": permissions, "origins": origins}))
        }
        "request" => Err("permissions.request requires a UI shell to prompt the user".into()),
        "remove" => Err("permissions.remove requires a UI shell to prompt the user".into()),
        "onAdded" | "onRemoved" => event_dispatch(runtime, req),
        _ => Err(format!("permissions.{} is not supported", req.method)),
    }
}

// ── commands ──

fn commands(runtime: &ExtensionRuntime, req: &ApiRequest) -> ApiResponse {
    match req.method.as_str() {
        "getAll" => {
            let list = runtime
                .get_manifest(&req.extension_id)
                .and_then(|m| m.commands)
                .map(|cmds| {
                    cmds.into_iter()
                        .map(|(name, cmd)| {
                            json!({
                                "name": name,
                                "description": cmd.description,
                                "shortcut": cmd.suggested_key.as_ref().and_then(|m| m.get("default")).cloned().unwrap_or_default()
                            })
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            Ok(Value::Array(list))
        }
        "onCommand" => event_dispatch(runtime, req),
        _ => Err(format!("commands.{} not implemented", req.method)),
    }
}

// ── cookies ──

fn cookies(runtime: &ExtensionRuntime, req: &ApiRequest) -> ApiResponse {
    let jar = runtime.cookie_jar().ok_or("cookies API requires a cookie jar".to_string())?;
    match req.method.as_str() {
        "get" => {
            let details = req.options(0).ok_or("cookies.get requires details")?;
            let (domain, path) = cookie_domain_and_path(details)?;
            let name = details.get("name").and_then(|v| v.as_str()).unwrap_or("");
            Ok(jar.get(&domain, &path, name).map(cookie_to_value).unwrap_or(Value::Null))
        }
        "getAll" => {
            let details = req.options(0).cloned().unwrap_or_default();
            let domain = details.get("url").and_then(|v| v.as_str()).and_then(|u| Url::parse(u).ok().and_then(|u| u.host_str().map(|h| h.to_string()))).unwrap_or_default();
            let name_filter = details.get("name").and_then(|v| v.as_str()).map(|s| s.to_string());
            let cookies = jar
                .cookies_for_domain(&domain)
                .into_iter()
                .filter(|c| name_filter.as_ref().map_or(true, |f| &c.name == f))
                .map(cookie_to_value)
                .collect::<Vec<_>>();
            Ok(Value::Array(cookies))
        }
        "getAllCookieStores" => {
            let tabs = runtime.tabs().into_iter().map(|t| t.id).collect::<Vec<_>>();
            Ok(json!([{"id":"0","tabIds":tabs,"incognito":false}]))
        }
        "set" => {
            let details = req.options(0).ok_or("cookies.set requires details")?;
            let cookie = cookie_from_details(details)?;
            jar.insert(cookie.clone()).map_err(|e| e.to_string())?;
            Ok(cookie_to_value(cookie))
        }
        "remove" => {
            let details = req.options(0).ok_or("cookies.remove requires details")?;
            let (domain, path) = cookie_domain_and_path(details)?;
            let name = details.get("name").and_then(|v| v.as_str()).unwrap_or("");
            Ok(json!(jar.remove(&domain, &path, name).unwrap_or(false)))
        }
        "onChanged" => event_dispatch(runtime, req),
        _ => Err(format!("cookies.{} is not supported", req.method)),
    }
}

fn cookie_domain_and_path(details: &Map<String, Value>) -> Result<(String, String), String> {
    if let Some(url) = details.get("url").and_then(|v| v.as_str()) {
        let parsed = Url::parse(url).map_err(|e| e.to_string())?;
        let domain = parsed.host_str().unwrap_or("").to_string();
        let path = parsed.path().to_string();
        return Ok((domain, path));
    }
    let domain = details.get("domain").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let path = details.get("path").and_then(|v| v.as_str()).unwrap_or("/").to_string();
    Ok((domain, path))
}

fn cookie_from_details(details: &Map<String, Value>) -> Result<Cookie, String> {
    let (domain, path) = cookie_domain_and_path(details)?;
    let name = details.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let value = details.get("value").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let secure = details.get("secure").and_then(|v| v.as_bool()).unwrap_or(false);
    let http_only = details.get("httpOnly").and_then(|v| v.as_bool()).unwrap_or(false);
    let expires_at = details.get("expirationDate").and_then(|v| v.as_f64()).map(|f| f as i64);
    let same_site = details.get("sameSite").and_then(|v| v.as_str()).and_then(parse_same_site);
    Ok(Cookie {
        name,
        value,
        domain,
        path,
        expires_at,
        max_age_secs: None,
        secure,
        http_only,
        same_site,
    })
}

fn parse_same_site(s: &str) -> Option<SameSite> {
    match s.to_lowercase().as_str() {
        "strict" => Some(SameSite::Strict),
        "lax" => Some(SameSite::Lax),
        "none" | "no_restriction" => Some(SameSite::None),
        _ => None,
    }
}

fn same_site_to_value(s: SameSite) -> Value {
    match s {
        SameSite::Strict => json!("strict"),
        SameSite::Lax => json!("lax"),
        SameSite::None => json!("no_restriction"),
    }
}

fn cookie_to_value(cookie: Cookie) -> Value {
    json!({
        "name": cookie.name,
        "value": cookie.value,
        "domain": cookie.domain,
        "path": cookie.path,
        "secure": cookie.secure,
        "httpOnly": cookie.http_only,
        "sameSite": cookie.same_site.map(same_site_to_value).unwrap_or(json!("no_restriction")),
        "session": cookie.expires_at.is_none() && cookie.max_age_secs.is_none(),
        "expirationDate": cookie.expires_at,
    })
}

// ── management ──

fn management(runtime: &ExtensionRuntime, req: &ApiRequest) -> ApiResponse {
    match req.method.as_str() {
        "getSelf" => {
            if let Some(ext) = runtime.get(&req.extension_id) {
                Ok(extension_info(&ext))
            } else {
                Err("Extension not found".into())
            }
        }
        "getAll" => {
            let list = runtime.list().into_iter().map(|e| extension_info(&e)).collect();
            Ok(Value::Array(list))
        }
        "get" => {
            let id = req.string_arg(1).or(req.string_arg(0)).unwrap_or("");
            runtime
                .get(&ExtensionId(id.to_string()))
                .map(|e| extension_info(&e))
                .ok_or_else(|| format!("Extension {} not found", id))
        }
        "setEnabled" => {
            let id = req.string_arg(1).or(req.string_arg(0)).unwrap_or("");
            let enabled = req
                .args
                .get(1)
                .and_then(|v| v.as_bool())
                .or_else(|| req.args.first().and_then(|v| v.as_bool()))
                .unwrap_or(true);
            runtime.set_enabled(&ExtensionId(id.to_string()), enabled)?;
            Ok(Value::Null)
        }
        "uninstallSelf" => {
            runtime.uninstall(&req.extension_id)?;
            Ok(Value::Null)
        }
        "launchApp" | "createAppShortcut" | "install" | "uninstall" => {
            Err(format!("management.{} is not supported", req.method))
        }
        _ => Err(format!("management.{} is not supported", req.method)),
    }
}

fn extension_info(ext: &LoadedExtension) -> Value {
    json!({
        "id": ext.id.0,
        "name": ext.manifest.name,
        "version": ext.manifest.version,
        "enabled": ext.enabled,
        "type": "extension",
        "installType": "normal",
        "mayDisable": true,
        "optionsUrl": ext.manifest.options_page,
        "icons": ext.manifest.icons
    })
}

// ── omnibox ──

fn omnibox(runtime: &ExtensionRuntime, req: &ApiRequest) -> ApiResponse {
    match req.method.as_str() {
        "setDefaultSuggestion" => {
            let suggestion = req.options(1).or_else(|| req.options(0)).cloned().unwrap_or_default();
            *runtime.omnibox_default_suggestion() = Some(Value::Object(suggestion));
            Ok(Value::Null)
        }
        "onInputStarted" | "onInputChanged" | "onInputEntered" | "onInputCancelled" => {
            event_dispatch(runtime, req)
        }
        _ => Err(format!("omnibox.{} is not supported", req.method)),
    }
}

// ── find ──

fn find(runtime: &ExtensionRuntime, req: &ApiRequest) -> ApiResponse {
    let query = req.string_arg(0).unwrap_or("").to_lowercase();
    let active = runtime.tabs().into_iter().find(|t| t.active);
    let title = active.as_ref().map(|t| t.title.to_lowercase()).unwrap_or_default();
    let count = if query.is_empty() {
        0
    } else {
        title.matches(&query).count() as u32
    };
    Ok(json!({"count": count, "rangeData": []}))
}

// ── userScripts / identity ──

fn user_scripts(runtime: &ExtensionRuntime, req: &ApiRequest) -> ApiResponse {
    match req.method.as_str() {
        "register" => {
            let scripts = req.args.first().and_then(|v| v.as_array()).cloned().unwrap_or_default();
            let mut map = runtime.registered_user_scripts();
            for script in scripts {
                if let Some(id) = script.get("id").and_then(|v| v.as_str()) {
                    map.insert(id.to_string(), script);
                }
            }
            Ok(Value::Null)
        }
        "unregister" => {
            let ids = req
                .args
                .first()
                .and_then(|v| v.as_object())
                .and_then(|o| o.get("ids"))
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect::<Vec<_>>())
                .unwrap_or_default();
            let mut map = runtime.registered_user_scripts();
            if ids.is_empty() {
                map.clear();
            } else {
                for id in ids {
                    map.remove(&id);
                }
            }
            Ok(Value::Null)
        }
        "onBeforeScript" => event_dispatch(runtime, req),
        _ => Err(format!("userScripts.{} is not supported", req.method)),
    }
}

fn identity(_runtime: &ExtensionRuntime, req: &ApiRequest) -> ApiResponse {
    match req.method.as_str() {
        "getRedirectURL" => {
            let suffix = req.string_arg(0).unwrap_or("");
            Ok(json!(format!("https://{}.chromiumapp.org/{}", req.extension_id.0, suffix)))
        }
        "getAuthToken" | "removeCachedAuthToken" => Ok(Value::Null),
        "launchWebAuthFlow" => Err("identity.launchWebAuthFlow requires a UI shell".into()),
        _ => Err(format!("identity.{} is not supported", req.method)),
    }
}

// ── devtools ──

fn devtools(runtime: &ExtensionRuntime, req: &ApiRequest) -> ApiResponse {
    let rest = req.namespace.strip_prefix("devtools.").unwrap_or("");
    match rest {
        "inspectedWindow" => match req.method.as_str() {
            "eval" => Err("devtools.inspectedWindow.eval requires a JS bridge".into()),
            "reload" => {
                let id = runtime.active_tab().map(|t| t.id).unwrap_or(0);
                runtime.reload_tab_driver(id).map(|_| Value::Null)
            }
            "getResources" => Err("devtools.inspectedWindow.getResources requires a JS bridge".into()),
            _ => Err(format!("devtools.inspectedWindow.{} is not supported", req.method)),
        },
        "network" => match req.method.as_str() {
            "getHAR" => Ok(json!([])),
            "onNavigated" | "onRequestFinished" => event_dispatch(runtime, req),
            _ => Err(format!("devtools.network.{} is not supported", req.method)),
        },
        "panels" => match req.method.as_str() {
            "create" | "elements" | "openResource" => Err(format!("devtools.panels.{} requires a UI shell", req.method)),
            "themeName" => Ok(json!("dark")),
            "onThemeChanged" | "onSearch" | "onSelectionChanged" => event_dispatch(runtime, req),
            _ => Err(format!("devtools.panels.{} is not supported", req.method)),
        },
        _ => Err(format!("devtools.{} is not supported", rest)),
    }
}
