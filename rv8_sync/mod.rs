//! Mono-protocol export/import adapters for RV8 storage.
//!
//! Payload shapes mirror `mono-protocol` objects (`CookieJarObject`, browser session
//! as `PrivateState`) without taking a hard dependency on the mono workspace yet.
//! A future `mono-browser` adapter can map these bytes through `BrowserEngine`.

use serde::{Deserialize, Serialize};

use crate::storage::{BrowserSessionSnapshot, CookieJarSnapshot, StorageManager};

pub const SYNC_FORMAT_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyncObjectClass {
    CookieJar,
    BrowserSession,
}

impl SyncObjectClass {
    pub fn mono_transfer_class_name(self) -> &'static str {
        match self {
            Self::CookieJar => "sensitive_session",
            Self::BrowserSession => "private_state",
        }
    }
}

#[derive(Debug)]
pub enum SyncError {
    UnsupportedClass(SyncObjectClass),
    Storage(crate::storage::StorageError),
    Serde(serde_json::Error),
}

impl std::fmt::Display for SyncError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedClass(class) => write!(f, "unsupported sync class: {class:?}"),
            Self::Storage(err) => write!(f, "{err}"),
            Self::Serde(err) => write!(f, "serialization error: {err}"),
        }
    }
}

impl std::error::Error for SyncError {}

impl From<crate::storage::StorageError> for SyncError {
    fn from(value: crate::storage::StorageError) -> Self {
        Self::Storage(value)
    }
}

impl From<serde_json::Error> for SyncError {
    fn from(value: serde_json::Error) -> Self {
        Self::Serde(value)
    }
}

/// `CookieJarObject`-compatible envelope (metadata + embedded jar snapshot).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct MonoCookieJarEnvelope {
    pub object_id: String,
    pub owner: String,
    pub jar_ref: String,
    pub jar: CookieJarSnapshot,
}

/// Browser session payload for `ObjectKind::BrowserSession` (`PrivateState`).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct MonoBrowserSessionEnvelope {
    pub object_id: String,
    pub owner: String,
    pub session: BrowserSessionSnapshot,
}

/// Cookie jar export/import aligned with `mono-protocol::CookieJarObject` handoff.
///
/// Full cookie bytes travel in the JSON envelope; `jar_ref` mirrors `CookieJarObject::jar_ref`.
pub trait MonoCookieJarSync {
    fn export_cookie_jar_json(&self) -> Result<String, SyncError>;
    fn import_cookie_jar(&self, json: &str) -> Result<(), SyncError>;
}

pub fn export_cookie_jar_json(
    storage: &StorageManager,
    owner: impl Into<String>,
) -> Result<String, SyncError> {
    Rv8SyncAdapter::new(storage, owner).export_cookie_jar_json()
}

pub fn import_cookie_jar(
    storage: &StorageManager,
    owner: impl Into<String>,
    json: &str,
) -> Result<(), SyncError> {
    Rv8SyncAdapter::new(storage, owner).import_cookie_jar(json)
}

pub struct Rv8SyncAdapter<'a> {
    storage: &'a StorageManager,
    owner: String,
    cookie_object_id: String,
    session_object_id: String,
}

impl<'a> Rv8SyncAdapter<'a> {
    pub fn new(storage: &'a StorageManager, owner: impl Into<String>) -> Self {
        let owner = owner.into();
        Self {
            storage,
            owner: owner.clone(),
            cookie_object_id: format!("rv8/cookie-jar/{owner}"),
            session_object_id: format!("rv8/browser-session/{owner}"),
        }
    }

    pub fn export_payload(&self, class: SyncObjectClass) -> Result<Vec<u8>, SyncError> {
        match class {
            SyncObjectClass::CookieJar => {
                let envelope = MonoCookieJarEnvelope {
                    object_id: self.cookie_object_id.clone(),
                    owner: self.owner.clone(),
                    jar_ref: self.storage.cookies.jar_ref(),
                    jar: self.storage.cookies.snapshot(),
                };
                Ok(serde_json::to_vec(&envelope)?)
            }
            SyncObjectClass::BrowserSession => {
                let envelope = MonoBrowserSessionEnvelope {
                    object_id: self.session_object_id.clone(),
                    owner: self.owner.clone(),
                    session: self.storage.session.snapshot(),
                };
                Ok(serde_json::to_vec(&envelope)?)
            }
        }
    }

    pub fn apply_payload(&self, payload: &[u8], class: SyncObjectClass) -> Result<(), SyncError> {
        match class {
            SyncObjectClass::CookieJar => {
                let envelope: MonoCookieJarEnvelope = serde_json::from_slice(payload)?;
                self.storage.cookies.replace_all(envelope.jar)?;
            }
            SyncObjectClass::BrowserSession => {
                let envelope: MonoBrowserSessionEnvelope = serde_json::from_slice(payload)?;
                self.storage.session.replace(envelope.session)?;
            }
        }
        Ok(())
    }

    pub fn cookie_jar_envelope(&self) -> MonoCookieJarEnvelope {
        MonoCookieJarEnvelope {
            object_id: self.cookie_object_id.clone(),
            owner: self.owner.clone(),
            jar_ref: self.storage.cookies.jar_ref(),
            jar: self.storage.cookies.snapshot(),
        }
    }
}

impl MonoCookieJarSync for Rv8SyncAdapter<'_> {
    fn export_cookie_jar_json(&self) -> Result<String, SyncError> {
        Ok(serde_json::to_string(&self.cookie_jar_envelope())?)
    }

    fn import_cookie_jar(&self, json: &str) -> Result<(), SyncError> {
        let envelope: MonoCookieJarEnvelope = serde_json::from_str(json)?;
        self.storage.cookies.replace_all(envelope.jar)?;
        Ok(())
    }
}

/// Engine-style hooks aligned with `mono-browser::BrowserEngine` transfer-class strings.
pub struct Rv8EngineSync<'a> {
    adapter: Rv8SyncAdapter<'a>,
}

impl<'a> Rv8EngineSync<'a> {
    pub fn new(storage: &'a StorageManager, owner: impl Into<String>) -> Self {
        Self {
            adapter: Rv8SyncAdapter::new(storage, owner),
        }
    }

    pub fn export_object_payload(&self, transfer_class: &str) -> Result<Vec<u8>, SyncError> {
        match transfer_class {
            "sensitive_session" => self.adapter.export_payload(SyncObjectClass::CookieJar),
            "private_state" => self.adapter.export_payload(SyncObjectClass::BrowserSession),
            _ => Err(SyncError::UnsupportedClass(SyncObjectClass::BrowserSession)),
        }
    }

    pub fn apply_object_payload(&self, payload: &[u8], transfer_class: &str) -> Result<(), SyncError> {
        match transfer_class {
            "sensitive_session" => self
                .adapter
                .apply_payload(payload, SyncObjectClass::CookieJar),
            "private_state" => self
                .adapter
                .apply_payload(payload, SyncObjectClass::BrowserSession),
            _ => Err(SyncError::UnsupportedClass(SyncObjectClass::BrowserSession)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{Cookie, SessionTab, StorageManager};

    #[test]
    fn export_cookie_jar_json_round_trip() {
        let dir = tempfile::tempdir().expect("tempdir");
        let storage = StorageManager::open(dir.path(), false).expect("storage");
        storage
            .cookies
            .insert(Cookie {
                name: "json".into(),
                value: "v".into(),
                domain: "mono.local".into(),
                path: "/".into(),
                expires_at: None,
                max_age_secs: None,
                secure: false,
                http_only: false,
                same_site: None,
            })
            .expect("insert");

        let json = export_cookie_jar_json(&storage, "identity-json").expect("export");
        storage
            .cookies
            .replace_all(CookieJarSnapshot {
                version: 1,
                cookies: vec![],
            })
            .expect("clear");
        import_cookie_jar(&storage, "identity-json", &json).expect("import");
        assert_eq!(storage.cookies.all().len(), 1);
    }

    #[test]
    fn cookie_jar_export_import_round_trip() {
        let dir = tempfile::tempdir().expect("tempdir");
        let storage = StorageManager::open(dir.path(), false).expect("storage");
        storage
            .cookies
            .insert(Cookie {
                name: "sync".into(),
                value: "1".into(),
                domain: "mono.local".into(),
                path: "/".into(),
                expires_at: None,
                max_age_secs: None,
                secure: false,
                http_only: false,
                same_site: None,
            })
            .expect("insert");

        let adapter = Rv8SyncAdapter::new(&storage, "identity-test");
        let exported = adapter
            .export_payload(SyncObjectClass::CookieJar)
            .expect("export");
        storage.cookies.replace_all(CookieJarSnapshot {
            version: 1,
            cookies: vec![],
        })
        .expect("clear");
        adapter
            .apply_payload(&exported, SyncObjectClass::CookieJar)
            .expect("import");
        assert_eq!(storage.cookies.all().len(), 1);
    }

    #[test]
    fn browser_session_export_import_round_trip() {
        let dir = tempfile::tempdir().expect("tempdir");
        let storage = StorageManager::open(dir.path(), false).expect("storage");
        storage
            .session
            .upsert_tab(SessionTab {
                tab_id: 3,
                url: "https://mono.local".into(),
                title: "Mono".into(),
            })
            .expect("tab");

        let adapter = Rv8SyncAdapter::new(&storage, "identity-test");
        let exported = adapter
            .export_payload(SyncObjectClass::BrowserSession)
            .expect("export");
        adapter
            .apply_payload(&exported, SyncObjectClass::BrowserSession)
            .expect("import");
        assert_eq!(storage.session.snapshot().tabs.len(), 1);
    }
}
