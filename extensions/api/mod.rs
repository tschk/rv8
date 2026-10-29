//! WebExtensions API dispatcher.
//!
//! `ApiRequest` carries a `namespace` and `method` plus JSON arguments.
//! Namespaces may contain a dot for events, e.g. `runtime.onMessage` or
//! `tabs.onCreated`. The dispatcher routes to per-namespace handlers in
//! `namespaces.rs`.

use serde_json::Value;

use super::permissions::Permission;
use super::runtime::{ExtensionId, ExtensionRuntime};

/// A request from an extension JavaScript context to the adapter.
#[derive(Debug, Clone)]
pub struct ApiRequest {
    pub namespace: String,
    pub method: String,
    pub args: Vec<Value>,
    pub extension_id: ExtensionId,
}

impl ApiRequest {
    pub fn new(namespace: &str, method: &str, extension_id: ExtensionId) -> Self {
        Self {
            namespace: namespace.to_string(),
            method: method.to_string(),
            args: Vec::new(),
            extension_id,
        }
    }

    pub fn with_args(mut self, args: Vec<Value>) -> Self {
        self.args = args;
        self
    }

    /// Convenience: first positional argument as JSON object.
    pub fn options(&self, idx: usize) -> Option<&serde_json::Map<String, Value>> {
        self.args.get(idx).and_then(|v| v.as_object())
    }

    /// Convenience: first positional argument as string.
    pub fn string_arg(&self, idx: usize) -> Option<&str> {
        self.args.get(idx).and_then(|v| v.as_str())
    }

    /// Convenience: argument as u64.
    pub fn u64_arg(&self, idx: usize) -> Option<u64> {
        self.args.get(idx).and_then(|v| v.as_u64())
    }

    /// Convenience: argument as i64.
    pub fn i64_arg(&self, idx: usize) -> Option<i64> {
        self.args.get(idx).and_then(|v| v.as_i64())
    }
}

pub type ApiResponse = Result<Value, String>;

/// Public dispatch entry used by `ExtensionRuntime::call_api`. Every call is
/// checked against the extension's manifest permissions first.
pub fn dispatch(runtime: &ExtensionRuntime, req: ApiRequest) -> ApiResponse {
    check_permissions(runtime, &req)?;
    namespaces::handle(runtime, req)
}

/// Manifest permissions that unlock a namespace (any one suffices), following
/// Chrome and Firefox. Unlisted namespaces such as `runtime`, `tabs`, `i18n`
/// and `action` need no permission.
fn required_permissions(namespace: &str, method: &str) -> Option<&'static [&'static str]> {
    let ns = namespace.split_once('.').map_or(namespace, |(ns, _)| ns);
    Some(match ns {
        "storage" => &["storage"],
        "alarms" => &["alarms"],
        "bookmarks" => &["bookmarks"],
        "history" => &["history"],
        "downloads" => &["downloads"],
        "notifications" => &["notifications"],
        "contextMenus" | "menus" => &["contextMenus", "menus"],
        "cookies" => &["cookies"],
        "scripting" => &["scripting"],
        "declarativeNetRequest" => &[
            "declarativeNetRequest",
            "declarativeNetRequestWithHostAccess",
        ],
        "identity" => &["identity"],
        "userScripts" => &["userScripts"],
        "webRequest" => &["webRequest"],
        "webNavigation" => &["webNavigation"],
        "management" if !matches!(method, "getSelf" | "uninstallSelf") => &["management"],
        _ => return None,
    })
}

fn check_permissions(runtime: &ExtensionRuntime, req: &ApiRequest) -> Result<(), String> {
    let granted = runtime.permissions(&req.extension_id).unwrap_or_default();
    if let Some(required) = required_permissions(&req.namespace, &req.method) {
        if !required.iter().any(|perm| granted.has_api(perm)) {
            return Err(format!(
                "{}.{} requires the \"{}\" permission",
                req.namespace, req.method, required[0]
            ));
        }
    }

    // Injecting into a page also needs host access to that page, or
    // activeTab for the active tab.
    let injects = matches!(
        req.method.as_str(),
        "executeScript" | "insertCSS" | "removeCSS"
    );
    if req.namespace == "scripting" && injects {
        let tab_id = req
            .options(0)
            .and_then(|injection| injection.get("target"))
            .and_then(|target| target.get("tabId"))
            .and_then(Value::as_u64);
        let tab = tab_id.and_then(|id| runtime.tab(id));
        let allowed = tab.is_some_and(|tab| {
            granted.has_host(&tab.url) || (tab.active && granted.contains(&Permission::ActiveTab))
        });
        if !allowed {
            return Err(format!(
                "scripting.{} requires host permission for the target tab",
                req.method
            ));
        }
    }

    // Cookie calls are scoped to hosts the extension may access. The
    // handlers key on `url` (and `domain` for get/set/remove), so check the
    // same field; `getAll` ignores `domain`, so it needs a `url`.
    let cookie_call = matches!(req.method.as_str(), "get" | "getAll" | "set" | "remove");
    if req.namespace == "cookies" && cookie_call {
        let details = req.options(0);
        let field = |key: &str| details.and_then(|d| d.get(key)).and_then(Value::as_str);
        let urls = match (field("url"), field("domain")) {
            (Some(url), _) => vec![url.to_string()],
            (None, Some(domain)) if req.method != "getAll" && !domain.is_empty() => {
                let host = domain.trim_start_matches('.');
                vec![format!("https://{host}/"), format!("http://{host}/")]
            }
            _ => Vec::new(),
        };
        if !urls.iter().any(|url| granted.has_host(url)) {
            return Err(format!(
                "cookies.{} requires host permission for the cookie's url",
                req.method
            ));
        }
    }
    Ok(())
}

mod namespaces;

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn runtime_with(permissions: &str) -> (ExtensionRuntime, ExtensionId, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let ext_dir = dir.path().join("perm-ext");
        std::fs::create_dir(&ext_dir).unwrap();
        std::fs::write(
            ext_dir.join("manifest.json"),
            format!(
                r#"{{"manifest_version":3,"name":"P","version":"1.0","permissions":{permissions}}}"#
            ),
        )
        .unwrap();
        let rt = ExtensionRuntime::new(dir.path().join("extensions"));
        let id = rt.load_from_dir(&ext_dir).unwrap();
        (rt, id, dir)
    }

    #[test]
    fn api_calls_require_the_declared_permission() {
        let (rt, id, _dir) = runtime_with(r#"["tabs"]"#);
        let denied = rt.call_api(ApiRequest::new("storage.local", "get", id.clone()));
        assert_eq!(
            denied,
            Err(r#"storage.local.get requires the "storage" permission"#.into())
        );
        let denied = rt.call_api(
            ApiRequest::new("history", "search", id.clone()).with_args(vec![json!({"text": ""})]),
        );
        assert!(denied.unwrap_err().contains(r#""history" permission"#));

        let (rt, id, _dir) = runtime_with(r#"["storage"]"#);
        assert!(rt
            .call_api(ApiRequest::new("storage.local", "get", id))
            .is_ok());
    }

    #[test]
    fn cookie_calls_require_host_access_to_the_cookie_domain() {
        let (rt, id, _dir) = runtime_with(r#"["cookies", "https://*.example.com/*"]"#);
        let call = |method: &str, details: Value| {
            rt.call_api(ApiRequest::new("cookies", method, id.clone()).with_args(vec![details]))
        };

        for (method, details) in [
            ("get", json!({"url": "https://bank.test/", "name": "sid"})),
            (
                "set",
                json!({"url": "https://bank.test/", "name": "sid", "value": "x"}),
            ),
            ("remove", json!({"domain": ".bank.test", "name": "sid"})),
            ("getAll", json!({})),
        ] {
            let denied = call(method, details).unwrap_err();
            assert!(denied.contains("host permission"), "{method}: {denied}");
        }
        // Permitted calls get past the check and fail later only because this
        // test has no cookie jar.
        let allowed = call(
            "get",
            json!({"url": "https://a.example.com/", "name": "sid"}),
        );
        assert!(allowed.unwrap_err().contains("cookie jar"));
    }

    #[test]
    fn script_injection_requires_host_access_to_the_tab() {
        let (rt, id, _dir) = runtime_with(r#"["scripting", "https://*.example.com/*"]"#);
        rt.new_tab(1, "https://foo.example.com/");
        rt.new_tab(2, "https://bank.test/");
        let inject = |tab: u64| {
            ApiRequest::new("scripting", "executeScript", id.clone())
                .with_args(vec![json!({"target": {"tabId": tab}, "code": "1"})])
        };

        let denied = rt.call_api(inject(2)).unwrap_err();
        assert!(denied.contains("host permission"), "{denied}");
        // Permitted calls get past the check and fail later only because this
        // test has no browser tab driver.
        let allowed = rt.call_api(inject(1)).unwrap_err();
        assert!(allowed.contains("browser driver"), "{allowed}");
    }
}
