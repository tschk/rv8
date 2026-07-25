//! Permission model for WebExtensions APIs.
//!
//! Maps string permission tokens to a typed set and provides host matching.

use super::matchers::url_matches_pattern;
use std::collections::HashSet;
use std::str::FromStr;

/// A typed extension permission.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Permission {
    Tabs,
    ActiveTab,
    Storage,
    Bookmarks,
    History,
    Downloads,
    DownloadsOpen,
    Cookies,
    WebRequest,
    WebRequestBlocking,
    DeclarativeNetRequest,
    DeclarativeNetRequestWithHostAccess,
    DeclarativeNetRequestFeedback,
    Management,
    ContextMenus,
    Menus,
    Notifications,
    Alarms,
    Identity,
    IdentityEmail,
    UnlimitedStorage,
    ClipboardRead,
    ClipboardWrite,
    ContentScripts,
    Scripting,
    Host(String),
    Other(String),
}

impl FromStr for Permission {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "tabs" => Permission::Tabs,
            "activeTab" => Permission::ActiveTab,
            "storage" => Permission::Storage,
            "bookmarks" => Permission::Bookmarks,
            "history" => Permission::History,
            "downloads" => Permission::Downloads,
            "downloads.open" => Permission::DownloadsOpen,
            "cookies" => Permission::Cookies,
            "webRequest" => Permission::WebRequest,
            "webRequestBlocking" => Permission::WebRequestBlocking,
            "declarativeNetRequest" => Permission::DeclarativeNetRequest,
            "declarativeNetRequestWithHostAccess" => Permission::DeclarativeNetRequestWithHostAccess,
            "declarativeNetRequestFeedback" => Permission::DeclarativeNetRequestFeedback,
            "management" => Permission::Management,
            "contextMenus" => Permission::ContextMenus,
            "menus" => Permission::Menus,
            "notifications" => Permission::Notifications,
            "alarms" => Permission::Alarms,
            "identity" => Permission::Identity,
            "identity.email" => Permission::IdentityEmail,
            "unlimitedStorage" => Permission::UnlimitedStorage,
            "clipboardRead" => Permission::ClipboardRead,
            "clipboardWrite" => Permission::ClipboardWrite,
            "contentScripts" => Permission::ContentScripts,
            "scripting" => Permission::Scripting,
            other if other.contains("://") || other.contains('*') || other.starts_with('<') => Permission::Host(other.to_string()),
            other => Permission::Other(other.to_string()),
        })
    }
}

impl Permission {
    pub fn as_str(&self) -> String {
        match self {
            Permission::Tabs => "tabs".into(),
            Permission::ActiveTab => "activeTab".into(),
            Permission::Storage => "storage".into(),
            Permission::Bookmarks => "bookmarks".into(),
            Permission::History => "history".into(),
            Permission::Downloads => "downloads".into(),
            Permission::DownloadsOpen => "downloads.open".into(),
            Permission::Cookies => "cookies".into(),
            Permission::WebRequest => "webRequest".into(),
            Permission::WebRequestBlocking => "webRequestBlocking".into(),
            Permission::DeclarativeNetRequest => "declarativeNetRequest".into(),
            Permission::DeclarativeNetRequestWithHostAccess => "declarativeNetRequestWithHostAccess".into(),
            Permission::DeclarativeNetRequestFeedback => "declarativeNetRequestFeedback".into(),
            Permission::Management => "management".into(),
            Permission::ContextMenus => "contextMenus".into(),
            Permission::Menus => "menus".into(),
            Permission::Notifications => "notifications".into(),
            Permission::Alarms => "alarms".into(),
            Permission::Identity => "identity".into(),
            Permission::IdentityEmail => "identity.email".into(),
            Permission::UnlimitedStorage => "unlimitedStorage".into(),
            Permission::ClipboardRead => "clipboardRead".into(),
            Permission::ClipboardWrite => "clipboardWrite".into(),
            Permission::ContentScripts => "contentScripts".into(),
            Permission::Scripting => "scripting".into(),
            Permission::Host(s) | Permission::Other(s) => s.clone(),
        }
    }
}

/// A set of granted permissions for one extension.
#[derive(Debug, Clone, Default)]
pub struct PermissionSet {
    permissions: HashSet<Permission>,
}

impl PermissionSet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn parse<I, S>(items: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut set = Self::new();
        for item in items {
            if let Ok(p) = Permission::from_str(item.as_ref()) {
                set.permissions.insert(p);
            }
        }
        set
    }

    pub fn contains(&self, perm: &Permission) -> bool {
        self.permissions.contains(perm)
    }

    pub fn has_api(&self, name: &str) -> bool {
        Permission::from_str(name)
            .map(|p| self.permissions.contains(&p))
            .unwrap_or(false)
    }

    pub fn has_host(&self, url: &str) -> bool {
        for perm in &self.permissions {
            if let Permission::Host(pattern) = perm {
                if url_matches_pattern(pattern, url) {
                    return true;
                }
            }
        }
        false
    }

    pub fn iter(&self) -> impl Iterator<Item = &Permission> {
        self.permissions.iter()
    }

    pub fn insert(&mut self, perm: Permission) {
        self.permissions.insert(perm);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_pattern_matching() {
        let set = PermissionSet::parse(["https://*.example.com/*"]);
        assert!(set.has_host("https://foo.example.com/bar"));
        assert!(!set.has_host("https://other.org/"));
        let all = PermissionSet::parse(["<all_urls>"]);
        assert!(all.has_host("https://anywhere.test/"));
    }

    #[test]
    fn api_permission_check() {
        let set = PermissionSet::parse(["tabs", "storage"]);
        assert!(set.has_api("tabs"));
        assert!(!set.has_api("bookmarks"));
    }
}
