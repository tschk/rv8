use serde::{Deserialize, Serialize};
use sled::Tree;

use super::error::StorageError;

pub const SESSION_TREE: &str = "session";
const DEFAULT_SESSION_KEY: &[u8] = b"default";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionTab {
    pub tab_id: u64,
    pub url: String,
    pub title: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrowserSessionSnapshot {
    pub version: u32,
    pub profile_id: String,
    pub tabs: Vec<SessionTab>,
    pub active_tab_id: Option<u64>,
    pub updated_at: i64,
    #[serde(skip)]
    pub tab_indices: std::collections::HashMap<u64, usize>,
}

impl Default for BrowserSessionSnapshot {
    fn default() -> Self {
        Self {
            version: 1,
            profile_id: "default".into(),
            tabs: Vec::new(),
            active_tab_id: None,
            updated_at: 0,
            tab_indices: std::collections::HashMap::new(),
        }
    }
}

pub struct SessionStore {
    tree: Option<Tree>,
    snapshot: parking_lot::RwLock<BrowserSessionSnapshot>,
}

impl SessionStore {
    pub fn open(tree: Tree, profile_id: impl Into<String>) -> Result<Self, StorageError> {
        let profile_id = profile_id.into();
        let mut snapshot = tree
            .get(DEFAULT_SESSION_KEY)?
            .map(|bytes| serde_json::from_slice::<BrowserSessionSnapshot>(&bytes))
            .transpose()?
            .unwrap_or_else(|| BrowserSessionSnapshot {
                profile_id,
                ..Default::default()
            });

        // Rebuild tab_indices map for O(1) lookups
        for (idx, tab) in snapshot.tabs.iter().enumerate() {
            snapshot.tab_indices.insert(tab.tab_id, idx);
        }

        Ok(Self {
            tree: Some(tree),
            snapshot: parking_lot::RwLock::new(snapshot),
        })
    }

    pub fn ephemeral(profile_id: impl Into<String>) -> Self {
        Self {
            tree: None,
            snapshot: parking_lot::RwLock::new(BrowserSessionSnapshot {
                profile_id: profile_id.into(),
                ..Default::default()
            }),
        }
    }

    pub fn snapshot(&self) -> BrowserSessionSnapshot {
        self.snapshot.read().clone()
    }

    pub fn replace(&self, mut snapshot: BrowserSessionSnapshot) -> Result<(), StorageError> {
        // Ensure indices are correct in the provided snapshot
        snapshot.tab_indices.clear();
        for (idx, tab) in snapshot.tabs.iter().enumerate() {
            snapshot.tab_indices.insert(tab.tab_id, idx);
        }

        if let Some(tree) = &self.tree {
            tree.insert(DEFAULT_SESSION_KEY, serde_json::to_vec(&snapshot)?)?;
            tree.flush()?;
        }
        *self.snapshot.write() = snapshot;
        Ok(())
    }

    pub fn upsert_tab(&self, tab: SessionTab) -> Result<(), StorageError> {
        let mut snap = self.snapshot.write();
        if let Some(&idx) = snap.tab_indices.get(&tab.tab_id) {
            snap.tabs[idx] = tab;
        } else {
            let idx = snap.tabs.len();
            snap.tab_indices.insert(tab.tab_id, idx);
            snap.tabs.push(tab);
        }
        snap.updated_at = now_unix();
        self.persist(&snap)
    }

    pub fn remove_tab(&self, tab_id: u64) -> Result<(), StorageError> {
        let mut snap = self.snapshot.write();
        if let Some(idx) = snap.tab_indices.remove(&tab_id) {
            snap.tabs.remove(idx);
            // Rebuild indices for elements that shifted
            for i in idx..snap.tabs.len() {
                let shifted_tab_id = snap.tabs[i].tab_id;
                snap.tab_indices.insert(shifted_tab_id, i);
            }
        }
        if snap.active_tab_id == Some(tab_id) {
            snap.active_tab_id = snap.tabs.first().map(|t| t.tab_id);
        }
        snap.updated_at = now_unix();
        self.persist(&snap)
    }

    pub fn set_active_tab(&self, tab_id: Option<u64>) -> Result<(), StorageError> {
        let mut snap = self.snapshot.write();
        snap.active_tab_id = tab_id;
        snap.updated_at = now_unix();
        self.persist(&snap)
    }

    fn persist(&self, snap: &BrowserSessionSnapshot) -> Result<(), StorageError> {
        if let Some(tree) = &self.tree {
            tree.insert(DEFAULT_SESSION_KEY, serde_json::to_vec(snap)?)?;
            tree.flush()?;
        }
        Ok(())
    }
}

fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sled::Config;

    fn temp_tree() -> Tree {
        Config::new()
            .temporary(true)
            .open()
            .expect("db")
            .open_tree(SESSION_TREE)
            .expect("tree")
    }

    #[test]
    fn session_tab_round_trip() {
        let store = SessionStore::open(temp_tree(), "test").expect("store");
        store
            .upsert_tab(SessionTab {
                tab_id: 1,
                url: "https://example.com".into(),
                title: "Example".into(),
            })
            .expect("upsert");
        store.set_active_tab(Some(1)).expect("active");
        let snap = store.snapshot();
        assert_eq!(snap.tabs.len(), 1);
        assert_eq!(snap.active_tab_id, Some(1));
    }
}
