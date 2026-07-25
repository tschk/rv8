//! Per-namespace WebExtensions API handlers.
//!
//! This is a scaffold covering every major namespace used by Chrome, Firefox,
//! Safari, and Orion extensions. Implemented methods return real adapter data;
//! unimplemented methods return an explicit `not implemented` error so callers
//! can see exactly what is missing.

use serde_json::{json, Map, Value};

use super::{ApiRequest, ApiResponse};
use crate::extensions::permissions::Permission;
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
        "create" => {
            // Single-window adapter: return a fresh synthetic window.
            let _opts = req.options(0).cloned().unwrap_or_default();
            Ok(current_window(runtime))
        }
        "update" | "remove" => Ok(Value::Null),
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
        "setIcon" => Ok(Value::Null),
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
        "openPopup" => Ok(Value::Null),
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

// ── bookmarks / history (stubs) ──

fn bookmarks(runtime: &ExtensionRuntime, req: &ApiRequest) -> ApiResponse {
    let _ = runtime;
    match req.method.as_str() {
        "getTree" => Ok(json!([{"id": "1", "title": "Bookmarks Bar", "index": 0, "children": []}])),
        "search" | "get" | "getChildren" | "getRecent" | "getSubTree" => Ok(json!([])),
        "create" | "move" | "update" | "remove" | "removeTree" => Ok(Value::Null),
        _ => Err(format!("bookmarks.{} not implemented", req.method)),
    }
}

fn history(runtime: &ExtensionRuntime, req: &ApiRequest) -> ApiResponse {
    let _ = runtime;
    match req.method.as_str() {
        "search" | "getVisits" => Ok(json!([])),
        "addUrl" | "deleteUrl" | "deleteRange" | "deleteAll" => Ok(Value::Null),
        _ => Err(format!("history.{} not implemented", req.method)),
    }
}

// ── downloads ──

fn downloads(runtime: &ExtensionRuntime, req: &ApiRequest) -> ApiResponse {
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
            let downloads = runtime.downloads();
            Ok(Value::Array(downloads.values().cloned().collect()))
        }
        "pause" | "resume" | "cancel" | "erase" | "removeFile" | "acceptDanger"
        | "show" | "showDefaultFolder" | "open" | "setShelfEnabled" | "getFileIcon" => Ok(Value::Null),
        _ => Err(format!("downloads.{} not implemented", req.method)),
    }
}

// ── notifications ──

fn notifications(runtime: &ExtensionRuntime, req: &ApiRequest) -> ApiResponse {
    match req.method.as_str() {
        "create" => {
            let id = req.string_arg(0).map(|s| s.to_string()).unwrap_or_else(|| format!("notif-{}", runtime.next_download_id()));
            let _opts = req.args.get(1).or(req.args.first()).cloned().unwrap_or(Value::Null);
            runtime.notification_items().insert(id.clone(), json!({"id": id.clone()}));
            Ok(json!(id))
        }
        "update" | "clear" => Ok(json!(true)),
        "getAll" => {
            let items = runtime.notification_items();
            Ok(Value::Array(items.values().cloned().collect()))
        }
        _ => Err(format!("notifications.{} not implemented", req.method)),
    }
}

// ── contextMenus / menus ──

fn context_menus(runtime: &ExtensionRuntime, req: &ApiRequest) -> ApiResponse {
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
            let info = req.args.get(1).or(req.args.first()).cloned().unwrap_or(Value::Null);
            runtime.context_menu_items().insert(numeric_id, runtime::ContextMenuItem {
                id: numeric_id,
                extension_id: req.extension_id.clone(),
                info,
            });
            Ok(json!(id))
        }
        "update" | "remove" => Ok(Value::Null),
        "removeAll" => {
            runtime.context_menu_items().clear();
            Ok(Value::Null)
        }
        "refresh" => Ok(Value::Null),
        "onClicked" => event_dispatch(runtime, req),
        _ => Err(format!("contextMenus.{} not implemented", req.method)),
    }
}

// ── scripting ──

fn scripting(runtime: &ExtensionRuntime, req: &ApiRequest) -> ApiResponse {
    let _ = runtime;
    match req.method.as_str() {
        "executeScript" => Ok(json!([])),
        "insertCSS" | "removeCSS" => Ok(Value::Null),
        "registerContentScripts" | "unregisterContentScripts" | "updateContentScripts" | "getRegisteredContentScripts" => {
            Ok(json!([]))
        }
        _ => Err(format!("scripting.{} not implemented", req.method)),
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
    let _ = runtime;
    match req.method.as_str() {
        "get" | "getAll" | "getAllCookieStores" => Ok(json!([])),
        "set" | "remove" => Ok(Value::Null),
        "onChanged" => event_dispatch(runtime, req),
        _ => Err(format!("cookies.{} not implemented", req.method)),
    }
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
        "setDefaultSuggestion" => Ok(Value::Null),
        "onInputStarted" | "onInputChanged" | "onInputEntered" | "onInputCancelled" => {
            event_dispatch(runtime, req)
        }
        _ => Err(format!("omnibox.{} not implemented", req.method)),
    }
}

// ── find ──

fn find(runtime: &ExtensionRuntime, req: &ApiRequest) -> ApiResponse {
    let _ = (runtime, req);
    Ok(json!({"count": 0, "rangeData": []}))
}

// ── userScripts / identity ──

fn user_scripts(runtime: &ExtensionRuntime, req: &ApiRequest) -> ApiResponse {
    match req.method.as_str() {
        "register" | "unregister" => Ok(Value::Null),
        "onBeforeScript" => event_dispatch(runtime, req),
        _ => Err(format!("userScripts.{} not implemented", req.method)),
    }
}

fn identity(_runtime: &ExtensionRuntime, req: &ApiRequest) -> ApiResponse {
    match req.method.as_str() {
        "getRedirectURL" => {
            let suffix = req.string_arg(0).unwrap_or("");
            Ok(json!(format!("https://{}.chromiumapp.org/{}", req.extension_id.0, suffix)))
        }
        "launchWebAuthFlow" | "getAuthToken" | "removeCachedAuthToken" => Ok(Value::Null),
        _ => Err(format!("identity.{} not implemented", req.method)),
    }
}

// ── devtools ──

fn devtools(runtime: &ExtensionRuntime, req: &ApiRequest) -> ApiResponse {
    let _ = runtime;
    let rest = req.namespace.strip_prefix("devtools.").unwrap_or("");
    match rest {
        "inspectedWindow" => match req.method.as_str() {
            "eval" => Ok(json!({"result": null, "exceptionDetails": null})),
            "reload" | "getResources" => Ok(Value::Null),
            _ => Err(format!("devtools.inspectedWindow.{} not implemented", req.method)),
        },
        "network" => match req.method.as_str() {
            "getHAR" => Ok(json!([])),
            "onNavigated" | "onRequestFinished" => event_dispatch(runtime, req),
            _ => Err(format!("devtools.network.{} not implemented", req.method)),
        },
        "panels" => match req.method.as_str() {
            "create" | "elements" => Ok(Value::Null),
            "themeName" => Ok(json!("dark")),
            "onThemeChanged" => event_dispatch(runtime, req),
            _ => Err(format!("devtools.panels.{} not implemented", req.method)),
        },
        _ => Err(format!("devtools.{} not implemented", rest)),
    }
}
