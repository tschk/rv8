//! Storage subsystems: profile metadata, cookies, and browser session state.
//!
//! Persistent state lives under [`crate::core::BrowserDataDirs::profile_dir`] in
//! `storage.sled` unless incognito mode is enabled (in-memory only).

mod cookie;
mod error;
mod profile;
mod session;

pub use cookie::{Cookie, CookieJar, CookieJarSnapshot, SameSite};
pub use error::StorageError;
pub use profile::{ProfileMeta, ProfileStore};
pub use session::{BrowserSessionSnapshot, SessionStore, SessionTab};

use log::info;
use std::path::{Path, PathBuf};

const DB_FILE: &str = "storage.sled";

/// Storage manager for profiles, cookies, and session snapshots.
pub struct StorageManager {
    db_path: PathBuf,
    ephemeral: bool,
    pub profile: ProfileStore,
    pub cookies: CookieJar,
    pub session: SessionStore,
}

impl StorageManager {
    pub async fn new(profile_dir: &Path) -> Result<Self, String> {
        Self::open(profile_dir, false).map_err(|e| e.to_string())
    }

    pub fn open(profile_dir: &Path, incognito: bool) -> Result<Self, StorageError> {
        std::fs::create_dir_all(profile_dir)?;

        if incognito {
            info!("Initializing ephemeral storage (incognito)");
            return Ok(Self {
                db_path: profile_dir.join(DB_FILE),
                ephemeral: true,
                profile: ProfileStore::ephemeral("incognito"),
                cookies: CookieJar::ephemeral(),
                session: SessionStore::ephemeral("incognito"),
            });
        }

        let db_path = profile_dir.join(DB_FILE);
        info!("Initializing storage at {:?}", db_path);
        let db = sled::open(&db_path)?;

        let cookie_tree = db.open_tree(cookie::COOKIE_TREE)?;
        let session_tree = db.open_tree(session::SESSION_TREE)?;
        let meta_tree = db.open_tree(profile::META_TREE)?;

        let profile_id = profile_dir
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("default")
            .to_string();

        Ok(Self {
            db_path,
            ephemeral: false,
            profile: ProfileStore::open(meta_tree, profile_id.clone())?,
            cookies: CookieJar::open(cookie_tree)?,
            session: SessionStore::open(session_tree, profile_id)?,
        })
    }

    pub fn db_path(&self) -> &Path {
        &self.db_path
    }

    pub fn is_ephemeral(&self) -> bool {
        self.ephemeral
    }

    pub async fn flush(&self) {
        info!("Flushing storage at {:?}", self.db_path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn persistent_storage_round_trip() {
        let dir = tempfile::tempdir().expect("tempdir");
        {
            let storage = StorageManager::open(dir.path(), false).expect("open");
            storage
                .cookies
                .insert(Cookie {
                    name: "token".into(),
                    value: "xyz".into(),
                    domain: "rv8.local".into(),
                    path: "/".into(),
                    expires_at: None,
                    max_age_secs: None,
                    secure: false,
                    http_only: true,
                    same_site: None,
                })
                .expect("cookie");
            storage
                .session
                .upsert_tab(SessionTab {
                    tab_id: 7,
                    url: "https://rv8.local".into(),
                    title: "RV8".into(),
                })
                .expect("tab");
        }

        let storage = StorageManager::open(dir.path(), false).expect("reopen");
        assert_eq!(storage.cookies.all().len(), 1);
        assert_eq!(storage.session.snapshot().tabs.len(), 1);
    }

    #[tokio::test]
    async fn incognito_does_not_persist() {
        let dir = tempfile::tempdir().expect("tempdir");
        let storage = StorageManager::open(dir.path(), true).expect("open");
        storage
            .cookies
            .insert(Cookie {
                name: "ephemeral".into(),
                value: "1".into(),
                domain: "rv8.local".into(),
                path: "/".into(),
                expires_at: None,
                max_age_secs: None,
                secure: false,
                http_only: false,
                same_site: None,
            })
            .expect("cookie");
        assert!(storage.is_ephemeral());
        assert_eq!(storage.cookies.all().len(), 1);

        let persisted = StorageManager::open(dir.path(), false).expect("reopen");
        assert!(persisted.cookies.all().is_empty());
    }
}
