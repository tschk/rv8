use std::fmt;

#[derive(Debug)]
pub enum StorageError {
    Io(std::io::Error),
    Db(sled::Error),
    Serde(serde_json::Error),
    NotFound(String),
    InvalidData(String),
}

impl fmt::Display for StorageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "io error: {e}"),
            Self::Db(e) => write!(f, "database error: {e}"),
            Self::Serde(e) => write!(f, "serialization error: {e}"),
            Self::NotFound(msg) => write!(f, "not found: {msg}"),
            Self::InvalidData(msg) => write!(f, "invalid data: {msg}"),
        }
    }
}

impl std::error::Error for StorageError {}

impl From<std::io::Error> for StorageError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<sled::Error> for StorageError {
    fn from(value: sled::Error) -> Self {
        Self::Db(value)
    }
}

impl From<serde_json::Error> for StorageError {
    fn from(value: serde_json::Error) -> Self {
        Self::Serde(value)
    }
}
