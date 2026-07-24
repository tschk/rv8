use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sled::Tree;

use super::error::StorageError;

pub const INDEXEDDB_FILE: &str = "indexeddb.sled";
const META_TREE: &str = "meta";
const DATA_TREE: &str = "data";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexMetadata {
    pub name: String,
    pub key_path: String,
    #[serde(default)]
    pub unique: bool,
    #[serde(default)]
    pub multi_entry: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObjectStoreMetadata {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key_path: Option<String>,
    #[serde(default)]
    pub auto_increment: bool,
    #[serde(default)]
    pub current_auto_id: u64,
    #[serde(default)]
    pub indexes: Vec<IndexMetadata>,
}

impl ObjectStoreMetadata {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            key_path: None,
            auto_increment: false,
            current_auto_id: 0,
            indexes: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DatabaseMetadata {
    pub version: u32,
    pub name: String,
    #[serde(default)]
    pub object_stores: HashMap<String, ObjectStoreMetadata>,
}

impl DatabaseMetadata {
    pub fn new(name: impl Into<String>, version: u32) -> Self {
        Self {
            version,
            name: name.into(),
            object_stores: HashMap::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum KeyRange {
    Only(Value),
    LowerBound { key: Value, open: bool },
    UpperBound { key: Value, open: bool },
    Bound {
        lower: Value,
        upper: Value,
        lower_open: bool,
        upper_open: bool,
    },
}

impl KeyRange {
    pub fn only(key: Value) -> Self {
        Self::Only(key)
    }

    pub fn lower_bound(key: Value, open: bool) -> Self {
        Self::LowerBound { key, open }
    }

    pub fn upper_bound(key: Value, open: bool) -> Self {
        Self::UpperBound { key, open }
    }

    pub fn bound(lower: Value, upper: Value, lower_open: bool, upper_open: bool) -> Self {
        Self::Bound {
            lower,
            upper,
            lower_open,
            upper_open,
        }
    }

    pub fn contains(&self, key: &Value) -> bool {
        match self {
            Self::Only(k) => k == key,
            Self::LowerBound { key: lower, open } => match compare_keys(key, lower) {
                std::cmp::Ordering::Greater => true,
                std::cmp::Ordering::Equal => !open,
                std::cmp::Ordering::Less => false,
            },
            Self::UpperBound { key: upper, open } => match compare_keys(key, upper) {
                std::cmp::Ordering::Less => true,
                std::cmp::Ordering::Equal => !open,
                std::cmp::Ordering::Greater => false,
            },
            Self::Bound {
                lower,
                upper,
                lower_open,
                upper_open,
            } => {
                let lower_ok = match compare_keys(key, lower) {
                    std::cmp::Ordering::Greater => true,
                    std::cmp::Ordering::Equal => !lower_open,
                    std::cmp::Ordering::Less => false,
                };
                let upper_ok = match compare_keys(key, upper) {
                    std::cmp::Ordering::Less => true,
                    std::cmp::Ordering::Equal => !upper_open,
                    std::cmp::Ordering::Greater => false,
                };
                lower_ok && upper_ok
            }
        }
    }
}

#[derive(Clone)]
pub struct IndexedDb {
    inner: Arc<IndexedDbInner>,
}

enum IndexedDbInner {
    Persistent {
        meta_tree: Tree,
        data_tree: Tree,
    },
    Ephemeral {
        metadata: RwLock<HashMap<String, DatabaseMetadata>>,
        records: RwLock<HashMap<Vec<u8>, Vec<u8>>>,
    },
}

impl IndexedDb {
    pub fn open(profile_dir: &std::path::Path, ephemeral: bool) -> Result<Self, StorageError> {
        if ephemeral {
            return Ok(Self::ephemeral());
        }

        std::fs::create_dir_all(profile_dir)?;
        let db_path = profile_dir.join(INDEXEDDB_FILE);
        let db = sled::open(&db_path)?;
        let meta_tree = db.open_tree(META_TREE)?;
        let data_tree = db.open_tree(DATA_TREE)?;

        Ok(Self {
            inner: Arc::new(IndexedDbInner::Persistent {
                meta_tree,
                data_tree,
            }),
        })
    }

    pub fn ephemeral() -> Self {
        Self {
            inner: Arc::new(IndexedDbInner::Ephemeral {
                metadata: RwLock::new(HashMap::new()),
                records: RwLock::new(HashMap::new()),
            }),
        }
    }

    pub fn get_metadata(&self, db_name: &str) -> Result<DatabaseMetadata, StorageError> {
        match &*self.inner {
            IndexedDbInner::Persistent { meta_tree, .. } => meta_tree
                .get(meta_key(db_name))?
                .map(|bytes| serde_json::from_slice::<DatabaseMetadata>(&bytes))
                .transpose()?
                .ok_or_else(|| StorageError::NotFound(format!("database {db_name}"))),
            IndexedDbInner::Ephemeral { metadata, .. } => metadata
                .read()
                .get(db_name)
                .cloned()
                .ok_or_else(|| StorageError::NotFound(format!("database {db_name}"))),
        }
    }

    pub fn set_metadata(
        &self,
        db_name: &str,
        metadata: DatabaseMetadata,
    ) -> Result<(), StorageError> {
        if metadata.name != db_name {
            return Err(StorageError::InvalidData(format!(
                "metadata name {:?} does not match database name {:?}",
                metadata.name, db_name
            )));
        }

        match &*self.inner {
            IndexedDbInner::Persistent { meta_tree, .. } => {
                meta_tree.insert(meta_key(db_name), serde_json::to_vec(&metadata)?)?;
                meta_tree.flush()?;
            }
            IndexedDbInner::Ephemeral { metadata: cache, .. } => {
                cache.write().insert(db_name.to_string(), metadata);
            }
        }
        Ok(())
    }

    pub fn create_object_store(
        &self,
        db_name: &str,
        store: ObjectStoreMetadata,
    ) -> Result<(), StorageError> {
        let mut metadata = self
            .get_metadata(db_name)
            .unwrap_or_else(|_| DatabaseMetadata::new(db_name, 1));

        if metadata.object_stores.contains_key(&store.name) {
            return Err(StorageError::InvalidData(format!(
                "object store {} already exists in database {db_name}",
                store.name
            )));
        }

        metadata.object_stores.insert(store.name.clone(), store);
        self.set_metadata(db_name, metadata)
    }

    pub fn put(
        &self,
        db_name: &str,
        store_name: &str,
        key: Value,
        value: Value,
    ) -> Result<(), StorageError> {
        self.ensure_object_store(db_name, store_name)?;
        let record_key = record_key(db_name, store_name, &key)?;

        match &*self.inner {
            IndexedDbInner::Persistent { data_tree, .. } => {
                data_tree.insert(record_key, serde_json::to_vec(&value)?)?;
                data_tree.flush()?;
            }
            IndexedDbInner::Ephemeral { records, .. } => {
                records
                    .write()
                    .insert(record_key, serde_json::to_vec(&value)?);
            }
        }
        Ok(())
    }

    pub fn get(
        &self,
        db_name: &str,
        store_name: &str,
        key: &Value,
    ) -> Result<Option<Value>, StorageError> {
        self.ensure_object_store(db_name, store_name)?;
        let record_key = record_key(db_name, store_name, key)?;

        let bytes = match &*self.inner {
            IndexedDbInner::Persistent { data_tree, .. } => data_tree.get(record_key)?.map(|v| v.to_vec()),
            IndexedDbInner::Ephemeral { records, .. } => records.read().get(&record_key).cloned(),
        };

        bytes
            .map(|raw| serde_json::from_slice::<Value>(&raw))
            .transpose()
            .map_err(StorageError::from)
    }

    pub fn delete(
        &self,
        db_name: &str,
        store_name: &str,
        key: &Value,
    ) -> Result<bool, StorageError> {
        self.ensure_object_store(db_name, store_name)?;
        let record_key = record_key(db_name, store_name, key)?;

        match &*self.inner {
            IndexedDbInner::Persistent { data_tree, .. } => {
                let existed = data_tree.remove(record_key)?.is_some();
                data_tree.flush()?;
                Ok(existed)
            }
            IndexedDbInner::Ephemeral { records, .. } => {
                Ok(records.write().remove(&record_key).is_some())
            }
        }
    }

    pub fn query_range(
        &self,
        db_name: &str,
        store_name: &str,
        range: &KeyRange,
    ) -> Result<Vec<(Value, Value)>, StorageError> {
        self.ensure_object_store(db_name, store_name)?;

        if let KeyRange::Only(key) = range {
            return Ok(self
                .get(db_name, store_name, key)?
                .map(|value| vec![(key.clone(), value)])
                .unwrap_or_default());
        }

        let prefix = store_prefix(db_name, store_name);
        let mut results = Vec::new();

        match &*self.inner {
            IndexedDbInner::Persistent { data_tree, .. } => {
                for item in data_tree.scan_prefix(&prefix) {
                    let (raw_key, raw_value) = item?;
                    let key = decode_record_key(&raw_key, &prefix)?;
                    if range.contains(&key) {
                        let value = serde_json::from_slice::<Value>(&raw_value)?;
                        results.push((key, value));
                    }
                }
            }
            IndexedDbInner::Ephemeral { records, .. } => {
                for (raw_key, raw_value) in records.read().iter() {
                    if !raw_key.starts_with(&prefix) {
                        continue;
                    }
                    let key = decode_record_key(raw_key, &prefix)?;
                    if range.contains(&key) {
                        let value = serde_json::from_slice::<Value>(raw_value)?;
                        results.push((key, value));
                    }
                }
            }
        }

        results.sort_by(|(a, _), (b, _)| compare_keys(a, b));
        Ok(results)
    }

    pub fn next_auto_id(&self, db_name: &str, store_name: &str) -> Result<u64, StorageError> {
        let mut metadata = self.get_metadata(db_name)?;
        let store = metadata
            .object_stores
            .get_mut(store_name)
            .ok_or_else(|| StorageError::NotFound(format!("object store {store_name}")))?;

        if !store.auto_increment {
            return Err(StorageError::InvalidData(format!(
                "object store {store_name} is not auto-incrementing"
            )));
        }

        store.current_auto_id = store
            .current_auto_id
            .checked_add(1)
            .ok_or_else(|| StorageError::InvalidData("auto-increment id overflow".into()))?;
        let next_id = store.current_auto_id;
        self.set_metadata(db_name, metadata)?;
        Ok(next_id)
    }

    pub fn delete_database(&self, db_name: &str) -> Result<(), StorageError> {
        match &*self.inner {
            IndexedDbInner::Persistent { meta_tree, data_tree } => {
                meta_tree.remove(meta_key(db_name))?;
                let prefix = format!("{db_name}\0");
                let mut batch = sled::Batch::default();
                for item in data_tree.scan_prefix(prefix.as_bytes()) {
                    let (key, _) = item?;
                    batch.remove(key);
                }
                data_tree.apply_batch(batch)?;
                meta_tree.flush()?;
                data_tree.flush()?;
            }
            IndexedDbInner::Ephemeral { metadata, records } => {
                metadata.write().remove(db_name);
                let prefix = format!("{db_name}\0");
                records.write().retain(|key, _| !key.starts_with(prefix.as_bytes()));
            }
        }
        Ok(())
    }

    fn ensure_object_store(&self, db_name: &str, store_name: &str) -> Result<(), StorageError> {
        let metadata = self.get_metadata(db_name)?;
        if !metadata.object_stores.contains_key(store_name) {
            return Err(StorageError::NotFound(format!(
                "object store {store_name} in database {db_name}"
            )));
        }
        Ok(())
    }
}

fn meta_key(db_name: &str) -> Vec<u8> {
    format!("meta\0{db_name}").into_bytes()
}

fn store_prefix(db_name: &str, store_name: &str) -> Vec<u8> {
    format!("{db_name}\0{store_name}\0").into_bytes()
}

fn record_key(db_name: &str, store_name: &str, key: &Value) -> Result<Vec<u8>, StorageError> {
    let mut out = store_prefix(db_name, store_name);
    out.extend_from_slice(&encode_sortable_key(key)?);
    Ok(out)
}

fn decode_record_key(raw_key: &[u8], prefix: &[u8]) -> Result<Value, StorageError> {
    if raw_key.len() <= prefix.len() {
        return Err(StorageError::InvalidData("malformed record key".into()));
    }
    decode_sortable_key(&raw_key[prefix.len()..])
}

fn encode_sortable_key(key: &Value) -> Result<Vec<u8>, StorageError> {
    match key {
        Value::Null => Ok(vec![0x00]),
        Value::Bool(b) => Ok(vec![0x01, u8::from(*b)]),
        Value::Number(n) => {
            let f = n
                .as_f64()
                .ok_or_else(|| StorageError::InvalidData("invalid number key".into()))?;
            let bits = f.to_bits();
            let sortable = if bits & (1 << 63) != 0 {
                !bits
            } else {
                bits ^ (1 << 63)
            };
            let mut out = vec![0x02];
            out.extend_from_slice(&sortable.to_be_bytes());
            Ok(out)
        }
        Value::String(s) => {
            let mut out = vec![0x03];
            out.extend_from_slice(s.as_bytes());
            out.push(0x00);
            Ok(out)
        }
        Value::Array(items) => {
            let mut out = vec![0x04];
            for item in items {
                let encoded = encode_sortable_key(item)?;
                out.extend_from_slice(&(encoded.len() as u32).to_be_bytes());
                out.extend_from_slice(&encoded);
            }
            Ok(out)
        }
        Value::Object(_) => Err(StorageError::InvalidData(
            "object keys are not supported".into(),
        )),
    }
}

fn decode_sortable_key(bytes: &[u8]) -> Result<Value, StorageError> {
    if bytes.is_empty() {
        return Err(StorageError::InvalidData("empty encoded key".into()));
    }

    match bytes[0] {
        0x00 => Ok(Value::Null),
        0x01 => {
            let b = bytes
                .get(1)
                .copied()
                .ok_or_else(|| StorageError::InvalidData("truncated bool key".into()))?;
            Ok(Value::Bool(b != 0))
        }
        0x02 => {
            if bytes.len() != 9 {
                return Err(StorageError::InvalidData("truncated number key".into()));
            }
            let sortable = u64::from_be_bytes(bytes[1..9].try_into().unwrap());
            let bits = if sortable & (1 << 63) != 0 {
                sortable ^ (1 << 63)
            } else {
                !sortable
            };
            Ok(Value::Number(
                serde_json::Number::from_f64(f64::from_bits(bits))
                    .ok_or_else(|| StorageError::InvalidData("invalid number key".into()))?,
            ))
        }
        0x03 => {
            if bytes.len() < 2 {
                return Err(StorageError::InvalidData("truncated string key".into()));
            }
            let s = std::str::from_utf8(&bytes[1..bytes.len() - 1])
                .map_err(|e| StorageError::InvalidData(e.to_string()))?;
            Ok(Value::String(s.to_string()))
        }
        0x04 => {
            let mut offset = 1;
            let mut items = Vec::new();
            while offset < bytes.len() {
                if offset + 4 > bytes.len() {
                    return Err(StorageError::InvalidData("truncated array key".into()));
                }
                let len = u32::from_be_bytes(bytes[offset..offset + 4].try_into().unwrap()) as usize;
                offset += 4;
                if offset + len > bytes.len() {
                    return Err(StorageError::InvalidData("truncated array key".into()));
                }
                items.push(decode_sortable_key(&bytes[offset..offset + len])?);
                offset += len;
            }
            Ok(Value::Array(items))
        }
        tag => Err(StorageError::InvalidData(format!(
            "unknown encoded key tag {tag}"
        ))),
    }
}

fn compare_keys(a: &Value, b: &Value) -> std::cmp::Ordering {
    use std::cmp::Ordering;

    fn key_type_rank(value: &Value) -> u8 {
        match value {
            Value::Number(_) => 0,
            Value::String(_) => 1,
            Value::Bool(_) => 2,
            Value::Null => 3,
            Value::Array(_) => 4,
            Value::Object(_) => 5,
        }
    }

    match key_type_rank(a).cmp(&key_type_rank(b)) {
        Ordering::Equal => {}
        other => return other,
    }

    match (a, b) {
        (Value::Number(a), Value::Number(b)) => {
            let af = a.as_f64().unwrap_or(f64::NAN);
            let bf = b.as_f64().unwrap_or(f64::NAN);
            af.partial_cmp(&bf).unwrap_or(Ordering::Equal)
        }
        (Value::String(a), Value::String(b)) => a.cmp(b),
        (Value::Bool(a), Value::Bool(b)) => a.cmp(b),
        (Value::Null, Value::Null) => Ordering::Equal,
        (Value::Array(a), Value::Array(b)) => {
            for (left, right) in a.iter().zip(b.iter()) {
                let cmp = compare_keys(left, right);
                if cmp != Ordering::Equal {
                    return cmp;
                }
            }
            a.len().cmp(&b.len())
        }
        _ => Ordering::Equal,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use sled::Config;

    fn temp_indexeddb() -> IndexedDb {
        let dir = tempfile::tempdir().expect("tempdir");
        IndexedDb::open(dir.path(), false).expect("open")
    }

    fn setup_db(idb: &IndexedDb, db_name: &str) -> Result<(), StorageError> {
        idb.set_metadata(db_name, DatabaseMetadata::new(db_name, 1))?;
        idb.create_object_store(
            db_name,
            ObjectStoreMetadata {
                name: "items".into(),
                key_path: Some("id".into()),
                auto_increment: false,
                current_auto_id: 0,
                indexes: vec![IndexMetadata {
                    name: "by_name".into(),
                    key_path: "name".into(),
                    unique: false,
                    multi_entry: false,
                }],
            },
        )
    }

    #[test]
    fn metadata_round_trip() {
        let idb = temp_indexeddb();
        let meta = DatabaseMetadata::new("app", 3);
        idb.set_metadata("app", meta.clone()).expect("set");
        assert_eq!(idb.get_metadata("app").expect("get"), meta);
    }

    #[test]
    fn missing_database_returns_not_found() {
        let idb = temp_indexeddb();
        let err = idb.get_metadata("missing").unwrap_err();
        assert!(matches!(err, StorageError::NotFound(_)));
    }

    #[test]
    fn put_get_delete_round_trip() {
        let idb = temp_indexeddb();
        setup_db(&idb, "app").expect("setup");

        idb.put("app", "items", json!(1), json!({"name": "alpha"}))
            .expect("put");
        let got = idb
            .get("app", "items", &json!(1))
            .expect("get")
            .expect("value");
        assert_eq!(got, json!({"name": "alpha"}));

        assert!(idb.delete("app", "items", &json!(1)).expect("delete"));
        assert!(idb.get("app", "items", &json!(1)).expect("get").is_none());
    }

    #[test]
    fn create_object_store_rejects_duplicates() {
        let idb = temp_indexeddb();
        setup_db(&idb, "app").expect("setup");
        let err = idb
            .create_object_store("app", ObjectStoreMetadata::new("items"))
            .unwrap_err();
        assert!(matches!(err, StorageError::InvalidData(_)));
    }

    #[test]
    fn next_auto_id_increments_current_auto_id() {
        let idb = temp_indexeddb();
        idb.set_metadata("app", DatabaseMetadata::new("app", 1))
            .expect("set");
        idb.create_object_store(
            "app",
            ObjectStoreMetadata {
                name: "notes".into(),
                key_path: None,
                auto_increment: true,
                current_auto_id: 0,
                indexes: Vec::new(),
            },
        )
        .expect("store");

        assert_eq!(idb.next_auto_id("app", "notes").expect("id"), 1);
        assert_eq!(idb.next_auto_id("app", "notes").expect("id"), 2);

        let meta = idb.get_metadata("app").expect("meta");
        assert_eq!(meta.object_stores["notes"].current_auto_id, 2);
    }

    #[test]
    fn next_auto_id_requires_auto_increment() {
        let idb = temp_indexeddb();
        setup_db(&idb, "app").expect("setup");
        let err = idb.next_auto_id("app", "items").unwrap_err();
        assert!(matches!(err, StorageError::InvalidData(_)));
    }

    #[test]
    fn key_range_only() {
        let range = KeyRange::only(json!(5));
        assert!(range.contains(&json!(5)));
        assert!(!range.contains(&json!(6)));
    }

    #[test]
    fn key_range_lower_and_upper_bounds() {
        let lower = KeyRange::lower_bound(json!(10), false);
        assert!(lower.contains(&json!(10)));
        assert!(lower.contains(&json!(11)));
        assert!(!lower.contains(&json!(9)));

        let lower_open = KeyRange::lower_bound(json!(10), true);
        assert!(!lower_open.contains(&json!(10)));
        assert!(lower_open.contains(&json!(11)));

        let upper = KeyRange::upper_bound(json!(10), false);
        assert!(upper.contains(&json!(10)));
        assert!(upper.contains(&json!(9)));
        assert!(!upper.contains(&json!(11)));

        let upper_open = KeyRange::upper_bound(json!(10), true);
        assert!(!upper_open.contains(&json!(10)));
        assert!(upper_open.contains(&json!(9)));
    }

    #[test]
    fn key_range_bound() {
        let range = KeyRange::bound(json!(2), json!(5), false, false);
        assert!(range.contains(&json!(2)));
        assert!(range.contains(&json!(5)));
        assert!(range.contains(&json!(3)));
        assert!(!range.contains(&json!(1)));
        assert!(!range.contains(&json!(6)));

        let open = KeyRange::bound(json!(2), json!(5), true, true);
        assert!(!open.contains(&json!(2)));
        assert!(!open.contains(&json!(5)));
        assert!(open.contains(&json!(3)));
    }

    #[test]
    fn query_range_filters_records() {
        let idb = temp_indexeddb();
        setup_db(&idb, "app").expect("setup");

        for n in 1..=5 {
            idb.put("app", "items", json!(n), json!({"n": n}))
                .expect("put");
        }

        let only = idb
            .query_range("app", "items", &KeyRange::only(json!(3)))
            .expect("only");
        assert_eq!(only.len(), 1);
        assert_eq!(only[0].1, json!({"n": 3}));

        let lower = idb
            .query_range("app", "items", &KeyRange::lower_bound(json!(3), false))
            .expect("lower");
        assert_eq!(lower.len(), 3);
        assert_eq!(lower[0].1, json!({"n": 3}));

        let upper = idb
            .query_range("app", "items", &KeyRange::upper_bound(json!(3), false))
            .expect("upper");
        assert_eq!(upper.len(), 3);
        assert_eq!(upper.last().unwrap().1, json!({"n": 3}));

        let bound = idb
            .query_range(
                "app",
                "items",
                &KeyRange::bound(json!(2), json!(4), false, false),
            )
            .expect("bound");
        assert_eq!(bound.len(), 3);
        assert_eq!(bound[0].1, json!({"n": 2}));
        assert_eq!(bound[2].1, json!({"n": 4}));
    }

    #[test]
    fn query_range_string_keys() {
        let idb = temp_indexeddb();
        setup_db(&idb, "app").expect("setup");

        for key in ["a", "b", "c"] {
            idb.put("app", "items", json!(key), json!(key))
                .expect("put");
        }

        let results = idb
            .query_range(
                "app",
                "items",
                &KeyRange::bound(json!("a"), json!("b"), false, false),
            )
            .expect("range");
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].0, json!("a"));
        assert_eq!(results[1].0, json!("b"));
    }

    #[test]
    fn delete_database_removes_metadata_and_records() {
        let dir = tempfile::tempdir().expect("tempdir");
        {
            let idb = IndexedDb::open(dir.path(), false).expect("open");
            setup_db(&idb, "app").expect("setup");
            idb.put("app", "items", json!(1), json!("one"))
                .expect("put");
            idb.delete_database("app").expect("delete");
            assert!(matches!(
                idb.get_metadata("app"),
                Err(StorageError::NotFound(_))
            ));
            assert!(matches!(
                idb.get("app", "items", &json!(1)),
                Err(StorageError::NotFound(_))
            ));
        }

        let idb = IndexedDb::open(dir.path(), false).expect("reopen");
        assert!(matches!(
            idb.get_metadata("app"),
            Err(StorageError::NotFound(_))
        ));
    }

    #[test]
    fn ephemeral_mode_does_not_use_disk() {
        let dir = tempfile::tempdir().expect("tempdir");
        {
            let idb = IndexedDb::ephemeral();
            setup_db(&idb, "app").expect("setup");
            idb.put("app", "items", json!(1), json!("one"))
                .expect("put");
        }

        let idb = IndexedDb::open(dir.path(), false).expect("open");
        assert!(matches!(
            idb.get_metadata("app"),
            Err(StorageError::NotFound(_))
        ));
    }

    #[test]
    fn persistent_reopen_round_trip() {
        let dir = tempfile::tempdir().expect("tempdir");
        {
            let idb = IndexedDb::open(dir.path(), false).expect("open");
            setup_db(&idb, "app").expect("setup");
            idb.put("app", "items", json!(42), json!({"answer": 42}))
                .expect("put");
        }

        let idb = IndexedDb::open(dir.path(), false).expect("reopen");
        let value = idb
            .get("app", "items", &json!(42))
            .expect("get")
            .expect("value");
        assert_eq!(value, json!({"answer": 42}));
    }

    #[test]
    fn sortable_key_encoding_is_order_preserving_for_numbers() {
        let keys = [1.0, 2.0, 10.0, 20.0];
        let json_keys: Vec<Value> = keys.iter().map(|n| json!(*n)).collect();
        let mut encoded: Vec<_> = json_keys
            .iter()
            .map(|k| encode_sortable_key(k).expect("encode"))
            .collect();
        encoded.sort();
        let decoded: Vec<f64> = encoded
            .iter()
            .map(|bytes| {
                decode_sortable_key(bytes)
                    .expect("decode")
                    .as_f64()
                    .expect("number")
            })
            .collect();
        assert_eq!(decoded, keys);
    }

    #[test]
    fn ephemeral_delete_database_clears_state() {
        let idb = IndexedDb::ephemeral();
        setup_db(&idb, "app").expect("setup");
        idb.put("app", "items", json!(1), json!("one"))
            .expect("put");
        idb.delete_database("app").expect("delete");
        assert!(matches!(
            idb.get_metadata("app"),
            Err(StorageError::NotFound(_))
        ));
    }

    #[test]
    fn put_rejects_missing_object_store() {
        let idb = temp_indexeddb();
        idb.set_metadata("app", DatabaseMetadata::new("app", 1))
            .expect("set");
        let err = idb
            .put("app", "missing", json!(1), json!("x"))
            .unwrap_err();
        assert!(matches!(err, StorageError::NotFound(_)));
    }

    #[test]
    fn metadata_name_mismatch_is_invalid() {
        let idb = temp_indexeddb();
        let err = idb
            .set_metadata("app", DatabaseMetadata::new("other", 1))
            .unwrap_err();
        assert!(matches!(err, StorageError::InvalidData(_)));
    }

    #[test]
    fn in_memory_db_from_config() {
        let db = Config::new().temporary(true).open().expect("db");
        let _meta = db.open_tree(META_TREE).expect("meta");
        let _data = db.open_tree(DATA_TREE).expect("data");
    }
}
