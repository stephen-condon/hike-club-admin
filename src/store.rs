//! Abstraction over R2 so the handlers can be unit-tested without the Workers
//! runtime or the network. Deliberately shaped like R2 itself — get/put/delete
//! over byte blobs — so the real implementation in `r2_store` is a thin
//! translation and all the interesting logic stays in `admin`, where it is
//! testable against [`InMemoryStore`].
// @spec STORE-001, STORE-002, STORE-004, STORE-009
pub trait AdminStore {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, String>;
    async fn put(&self, key: &str, body: Vec<u8>, content_type: &str) -> Result<(), String>;
    async fn delete(&self, key: &str) -> Result<(), String>;
    /// Keys under `prefix`, used to answer "which locations have a hike, and a
    /// map?" with one list instead of two GETs per location.
    async fn list(&self, prefix: &str) -> Result<Vec<String>, String>;
}

#[cfg(test)]
pub mod fake {
    use super::AdminStore;
    use std::cell::RefCell;
    use std::collections::BTreeMap;

    /// In-memory [`AdminStore`] for tests. `RefCell` rather than a lock because
    /// the worker runtime is single-threaded and so are the tests.
    #[derive(Default)]
    // @spec STORE-003
    pub struct InMemoryStore {
        objects: RefCell<BTreeMap<String, (Vec<u8>, String)>>,
        /// When set, every operation fails with this message — the seam for
        /// exercising the 502 paths.
        pub fail_with: Option<String>,
    }

    impl InMemoryStore {
        pub fn new() -> Self {
            Self::default()
        }

        /// A store where every operation reports an upstream failure.
        pub fn failing() -> Self {
            Self {
                fail_with: Some("r2 unavailable".to_string()),
                ..Self::default()
            }
        }

        pub fn with_object(self, key: &str, body: &[u8], content_type: &str) -> Self {
            self.objects
                .borrow_mut()
                .insert(key.to_string(), (body.to_vec(), content_type.to_string()));
            self
        }

        pub fn with_json(self, key: &str, json: &str) -> Self {
            self.with_object(key, json.as_bytes(), "application/json")
        }

        pub fn read(&self, key: &str) -> Option<String> {
            self.objects
                .borrow()
                .get(key)
                .map(|(body, _)| String::from_utf8_lossy(body).to_string())
        }

        pub fn content_type(&self, key: &str) -> Option<String> {
            self.objects.borrow().get(key).map(|(_, ct)| ct.clone())
        }

        pub fn keys(&self) -> Vec<String> {
            self.objects.borrow().keys().cloned().collect()
        }

        fn check(&self) -> Result<(), String> {
            match &self.fail_with {
                Some(message) => Err(message.clone()),
                None => Ok(()),
            }
        }
    }

    impl AdminStore for InMemoryStore {
        async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, String> {
            self.check()?;
            Ok(self.objects.borrow().get(key).map(|(body, _)| body.clone()))
        }

        async fn put(&self, key: &str, body: Vec<u8>, content_type: &str) -> Result<(), String> {
            self.check()?;
            self.objects
                .borrow_mut()
                .insert(key.to_string(), (body, content_type.to_string()));
            Ok(())
        }

        async fn delete(&self, key: &str) -> Result<(), String> {
            self.check()?;
            self.objects.borrow_mut().remove(key);
            Ok(())
        }

        async fn list(&self, prefix: &str) -> Result<Vec<String>, String> {
            self.check()?;
            Ok(self
                .objects
                .borrow()
                .keys()
                .filter(|k| k.starts_with(prefix))
                .cloned()
                .collect())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::AdminStore;
    use super::fake::InMemoryStore;

    #[tokio::test]
    // @spec STORE-001, STORE-004
    async fn round_trips_an_object() {
        let store = InMemoryStore::new();
        store
            .put("hikes/a.json", b"{}".to_vec(), "application/json")
            .await
            .unwrap();
        assert_eq!(
            store.get("hikes/a.json").await.unwrap(),
            Some(b"{}".to_vec())
        );
        assert_eq!(
            store.content_type("hikes/a.json").unwrap(),
            "application/json"
        );
    }

    #[tokio::test]
    // @spec STORE-001
    async fn get_is_none_for_a_missing_key() {
        let store = InMemoryStore::new();
        assert_eq!(store.get("nope").await.unwrap(), None);
    }

    #[tokio::test]
    // @spec STORE-009
    async fn delete_removes_and_is_idempotent() {
        let store = InMemoryStore::new().with_json("hikes/a.json", "{}");
        store.delete("hikes/a.json").await.unwrap();
        assert_eq!(store.get("hikes/a.json").await.unwrap(), None);
        store.delete("hikes/a.json").await.unwrap();
        assert!(store.keys().is_empty());
    }

    #[tokio::test]
    // @spec STORE-001
    async fn list_filters_by_prefix() {
        let store = InMemoryStore::new()
            .with_json("hikes/a.json", "{}")
            .with_object("hikes/a/map.png", b"png", "image/png")
            .with_json("resources/hike-locations.json", "[]");
        let mut keys = store.list("hikes/").await.unwrap();
        keys.sort();
        assert_eq!(keys, vec!["hikes/a.json", "hikes/a/map.png"]);
    }

    #[tokio::test]
    // @spec STORE-003
    async fn a_failing_store_fails_every_operation() {
        let store = InMemoryStore::failing();
        assert!(store.get("k").await.is_err());
        assert!(store.put("k", vec![], "text/plain").await.is_err());
        assert!(store.delete("k").await.is_err());
        assert!(store.list("k").await.is_err());
        assert_eq!(store.read("k"), None);
    }
}
