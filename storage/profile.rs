use serde::{Deserialize, Serialize};
use sled::Tree;

use super::error::StorageError;

pub const META_TREE: &str = "meta";
const PROFILE_META_KEY: &[u8] = b"profile";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileMeta {
    pub version: u32,
    pub profile_id: String,
    pub display_name: String,
    pub created_at: i64,
    pub updated_at: i64,
}

impl ProfileMeta {
    pub fn new(profile_id: impl Into<String>, display_name: impl Into<String>) -> Self {
        let now = now_unix();
        Self {
            version: 1,
            profile_id: profile_id.into(),
            display_name: display_name.into(),
            created_at: now,
            updated_at: now,
        }
    }
}

pub struct ProfileStore {
    tree: Option<Tree>,
    meta: parking_lot::RwLock<ProfileMeta>,
}

impl ProfileStore {
    pub fn open(tree: Tree, profile_id: impl Into<String>) -> Result<Self, StorageError> {
        let profile_id = profile_id.into();
        let meta = tree
            .get(PROFILE_META_KEY)?
            .map(|bytes| serde_json::from_slice::<ProfileMeta>(&bytes))
            .transpose()?
            .unwrap_or_else(|| ProfileMeta::new(profile_id.clone(), profile_id));
        Ok(Self {
            tree: Some(tree),
            meta: parking_lot::RwLock::new(meta),
        })
    }

    pub fn ephemeral(profile_id: impl Into<String>) -> Self {
        let profile_id = profile_id.into();
        Self {
            tree: None,
            meta: parking_lot::RwLock::new(ProfileMeta::new(profile_id.clone(), profile_id)),
        }
    }

    pub fn meta(&self) -> ProfileMeta {
        self.meta.read().clone()
    }

    pub fn touch(&self) -> Result<(), StorageError> {
        let mut meta = self.meta.write();
        meta.updated_at = now_unix();
        self.persist(&meta)
    }

    fn persist(&self, meta: &ProfileMeta) -> Result<(), StorageError> {
        if let Some(tree) = &self.tree {
            tree.insert(PROFILE_META_KEY, serde_json::to_vec(meta)?)?;
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
