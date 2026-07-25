//! In-memory extension storage (`storage.local` / `storage.sync` / `storage.session`).
//!
//! A real implementation would persist `local` to the profile sled store, `sync`
//! to an account-backed sync server, and keep `session` ephemeral. This adapter
//! layer uses in-memory maps so the API surface compiles and tests independently.

use parking_lot::RwLock;
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StorageArea {
    Local,
    Sync,
    Session,
    Managed,
}

impl StorageArea {
    pub fn from_namespace(ns: &str) -> Option<Self> {
        match ns {
            "storage.local" => Some(StorageArea::Local),
            "storage.sync" => Some(StorageArea::Sync),
            "storage.session" => Some(StorageArea::Session),
            "storage.managed" => Some(StorageArea::Managed),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            StorageArea::Local => "local",
            StorageArea::Sync => "sync",
            StorageArea::Session => "session",
            StorageArea::Managed => "managed",
        }
    }
}

type ExtensionStore = HashMap<String, HashMap<String, Value>>;

#[derive(Debug, Default)]
pub struct ExtensionStorage {
    data: RwLock<HashMap<StorageArea, ExtensionStore>>,
}

impl ExtensionStorage {
    pub fn new() -> Self {
        Self::default()
    }

    #[allow(clippy::unnecessary_map_or)]
    pub fn get(&self, area: StorageArea, ext_id: &str, keys: &Value) -> Value {
        if area == StorageArea::Managed {
            // `managed` is read-only and controlled by enterprise policy.
            return Value::Object(Default::default());
        }

        let data = self.data.read();
        let store = data
            .get(&area)
            .and_then(|m| m.get(ext_id))
            .cloned()
            .unwrap_or_default();

        let empty_obj = keys.as_object().map_or(false, |o| o.is_empty());
        if keys.is_null() || empty_obj {
            return Value::Object(store.into_iter().collect());
        }

        if let Some(key) = keys.as_str() {
            return store
                .get(key)
                .cloned()
                .map(|v| {
                    let mut m = serde_json::Map::new();
                    m.insert(key.to_string(), v);
                    Value::Object(m)
                })
                .unwrap_or_else(|| Value::Object(Default::default()));
        }

        if let Some(arr) = keys.as_array() {
            let mut result = serde_json::Map::new();
            for k in arr.iter().filter_map(|v| v.as_str()) {
                if let Some(v) = store.get(k) {
                    result.insert(k.to_string(), v.clone());
                }
            }
            return Value::Object(result);
        }

        Value::Object(Default::default())
    }

    pub fn set(&self, area: StorageArea, ext_id: &str, items: &Value) -> Result<(), String> {
        if area == StorageArea::Managed {
            return Err("storage.managed is read-only".into());
        }
        let Some(items) = items.as_object() else {
            return Err("storage.set requires an object".into());
        };
        let mut data = self.data.write();
        let area_store = data.entry(area).or_default();
        let ext_store = area_store.entry(ext_id.to_string()).or_default();
        for (k, v) in items {
            ext_store.insert(k.clone(), v.clone());
        }
        Ok(())
    }

    pub fn remove(&self, area: StorageArea, ext_id: &str, keys: &Value) -> Result<(), String> {
        if area == StorageArea::Managed {
            return Err("storage.managed is read-only".into());
        }
        let mut data = self.data.write();
        let Some(ext_store) = data.get_mut(&area).and_then(|m| m.get_mut(ext_id)) else {
            return Ok(());
        };
        if let Some(key) = keys.as_str() {
            ext_store.remove(key);
        } else if let Some(arr) = keys.as_array() {
            for k in arr.iter().filter_map(|v| v.as_str()) {
                ext_store.remove(k);
            }
        }
        Ok(())
    }

    pub fn clear(&self, area: StorageArea, ext_id: &str) -> Result<(), String> {
        if area == StorageArea::Managed {
            return Err("storage.managed is read-only".into());
        }
        let mut data = self.data.write();
        if let Some(ext_store) = data.get_mut(&area).and_then(|m| m.get_mut(ext_id)) {
            ext_store.clear();
        }
        Ok(())
    }

    pub fn get_bytes_in_use(&self, _area: StorageArea, _ext_id: &str, _keys: &Value) -> usize {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn storage_round_trip() {
        let s = ExtensionStorage::new();
        let items = serde_json::json!({"foo": 1, "bar": "baz"});
        s.set(StorageArea::Local, "ext1", &items).unwrap();
        let got = s.get(StorageArea::Local, "ext1", &serde_json::json!("foo"));
        assert_eq!(got, serde_json::json!({"foo": 1}));
    }
}
