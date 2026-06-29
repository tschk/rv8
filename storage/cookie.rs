use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use sled::Tree;

use super::error::StorageError;

pub const COOKIE_TREE: &str = "cookies";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SameSite {
    Strict,
    Lax,
    None,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cookie {
    pub name: String,
    pub value: String,
    pub domain: String,
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_age_secs: Option<i64>,
    #[serde(default)]
    pub secure: bool,
    #[serde(default)]
    pub http_only: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub same_site: Option<SameSite>,
}

impl Cookie {
    fn key(&self) -> String {
        format!("{}|{}|{}", self.domain, self.path, self.name)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CookieJarSnapshot {
    pub version: u32,
    pub cookies: Vec<Cookie>,
}

/// Sled-backed cookie jar. Ephemeral mode uses in-memory HashMap.
///
/// ponytail: single storage backend —
///   sled provides both persistence and in-memory caching.
///   No separate HashMap cache layer needed.
#[derive(Clone)]
pub struct CookieJar {
    inner: Arc<CookieJarInner>,
}

enum CookieJarInner {
    Persistent { tree: Tree },
    Ephemeral { cache: RwLock<HashMap<String, Cookie>> },
}

impl CookieJar {
    pub fn open(tree: Tree) -> Result<Self, StorageError> {
        Ok(Self {
            inner: Arc::new(CookieJarInner::Persistent { tree }),
        })
    }

    pub fn ephemeral() -> Self {
        Self {
            inner: Arc::new(CookieJarInner::Ephemeral {
                cache: RwLock::new(HashMap::new()),
            }),
        }
    }

    pub fn insert(&self, cookie: Cookie) -> Result<(), StorageError> {
        match &*self.inner {
            CookieJarInner::Persistent { tree } => {
                let key = cookie.key();
                tree.insert(key.as_bytes(), serde_json::to_vec(&cookie)?)?;
                tree.flush()?;
            }
            CookieJarInner::Ephemeral { cache } => {
                cache.write().insert(cookie.key(), cookie);
            }
        }
        Ok(())
    }

    pub fn remove(&self, domain: &str, path: &str, name: &str) -> Result<bool, StorageError> {
        let key = format!("{domain}|{path}|{name}");
        match &*self.inner {
            CookieJarInner::Persistent { tree } => {
                let existed = tree.remove(key.as_bytes())?.is_some();
                tree.flush()?;
                Ok(existed)
            }
            CookieJarInner::Ephemeral { cache } => Ok(cache.write().remove(&key).is_some()),
        }
    }

    pub fn get(&self, domain: &str, path: &str, name: &str) -> Option<Cookie> {
        let key = format!("{domain}|{path}|{name}");
        match &*self.inner {
            CookieJarInner::Persistent { tree } => tree
                .get(key.as_bytes())
                .ok()
                .flatten()
                .and_then(|v| serde_json::from_slice::<Cookie>(&v).ok()),
            CookieJarInner::Ephemeral { cache } => cache.read().get(&key).cloned(),
        }
    }

    pub fn cookies_for_domain(&self, domain: &str) -> Vec<Cookie> {
        match &*self.inner {
            CookieJarInner::Persistent { tree } => tree
                .iter()
                .filter_map(|item| {
                    let (_, value) = item.ok()?;
                    serde_json::from_slice::<Cookie>(&value).ok()
                })
                .filter(|c| domain_matches(&c.domain, domain))
                .collect(),
            CookieJarInner::Ephemeral { cache } => cache
                .read()
                .values()
                .filter(|c| domain_matches(&c.domain, domain))
                .cloned()
                .collect(),
        }
    }

    pub fn all(&self) -> Vec<Cookie> {
        match &*self.inner {
            CookieJarInner::Persistent { tree } => tree
                .iter()
                .filter_map(|item| {
                    let (_, value) = item.ok()?;
                    serde_json::from_slice::<Cookie>(&value).ok()
                })
                .collect(),
            CookieJarInner::Ephemeral { cache } => cache.read().values().cloned().collect(),
        }
    }

    pub fn snapshot(&self) -> CookieJarSnapshot {
        CookieJarSnapshot {
            version: 1,
            cookies: self.all(),
        }
    }

    pub fn replace_all(&self, snapshot: CookieJarSnapshot) -> Result<(), StorageError> {
        match &*self.inner {
            CookieJarInner::Persistent { tree } => {
                let mut batch = sled::Batch::default();
                for item in tree.iter() {
                    let (key, _) = item?;
                    batch.remove(key);
                }
                for cookie in &snapshot.cookies {
                    batch.insert(cookie.key().as_bytes(), serde_json::to_vec(cookie)?);
                }
                tree.apply_batch(batch)?;
                tree.flush()?;
            }
            CookieJarInner::Ephemeral { cache } => {
                *cache.write() = snapshot
                    .cookies
                    .into_iter()
                    .map(|c| (c.key(), c))
                    .collect();
            }
        }
        Ok(())
    }
}

fn domain_matches(cookie_domain: &str, host: &str) -> bool {
    if cookie_domain.starts_with('.') {
        host.ends_with(cookie_domain) || host == cookie_domain.trim_start_matches('.')
    } else {
        host == cookie_domain || host.ends_with(&format!(".{cookie_domain}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sled::Config;

    fn temp_tree() -> Tree {
        let dir = tempfile::tempdir().expect("tempdir");
        Config::new()
            .temporary(true)
            .path(dir.path())
            .open()
            .expect("db")
            .open_tree(COOKIE_TREE)
            .expect("tree")
    }

    #[test]
    fn insert_and_get_round_trip() {
        let jar = CookieJar::open(temp_tree()).expect("jar");
        let cookie = Cookie {
            name: "sid".into(),
            value: "abc".into(),
            domain: "example.com".into(),
            path: "/".into(),
            expires_at: None,
            max_age_secs: None,
            secure: true,
            http_only: true,
            same_site: Some(SameSite::Lax),
        };
        jar.insert(cookie.clone()).expect("insert");
        let got = jar.get("example.com", "/", "sid").expect("cookie present");
        assert_eq!(got, cookie);
    }

    #[test]
    fn replace_all_clears() {
        let jar = CookieJar::open(temp_tree()).expect("jar");
        jar.insert(Cookie {
            name: "a".into(),
            value: "1".into(),
            domain: "a.test".into(),
            path: "/".into(),
            expires_at: None,
            max_age_secs: None,
            secure: false,
            http_only: false,
            same_site: None,
        })
        .expect("insert");
        jar.replace_all(CookieJarSnapshot {
            version: 1,
            cookies: vec![],
        })
        .expect("clear");
        assert!(jar.all().is_empty());
    }
}
