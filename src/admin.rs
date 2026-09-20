//! Handler logic, generic over [`AdminStore`] so every path here is unit-tested
//! against the in-memory fake — no R2, no network, no Workers runtime.
//!
//! Handlers return [`Outcome`], a status plus an already-serialized body, so
//! `lib.rs` stays pure translation into `worker::Response`.
use crate::models::{ErrorBody, HikeLocation, HikeRecord, HikeRequest, LOCATIONS_KEY};
use crate::store::AdminStore;
use crate::validate::{self, Invalid};

/// What a handler decided: an HTTP status and the bytes to send with it.
#[derive(Debug, Clone, PartialEq)]
pub struct Outcome {
    pub status: u16,
    pub content_type: &'static str,
    pub body: Vec<u8>,
}

impl Outcome {
    pub fn json(status: u16, value: &impl serde::Serialize) -> Self {
        Self {
            status,
            content_type: "application/json",
            // Serializing our own owned types; a failure here is a bug, not
            // input, so it surfaces as a 500 body rather than a panic in the
            // worker.
            body: serde_json::to_vec(value)
                .unwrap_or_else(|_| br#"{"error":"serialization failed"}"#.to_vec()),
        }
    }

    pub fn error(status: u16, message: impl Into<String>) -> Self {
        Self::json(status, &ErrorBody::new(message))
    }

    pub fn no_content() -> Self {
        Self {
            status: 204,
            content_type: "application/json",
            body: Vec::new(),
        }
    }
}

impl From<Invalid> for Outcome {
    fn from(invalid: Invalid) -> Self {
        Outcome::error(invalid.status, invalid.message)
    }
}

/// R2 failures are the caller's 502 — the admin worker is a proxy for storage,
/// and there is nothing the browser can do but retry.
fn upstream(message: String) -> Outcome {
    Outcome::error(502, format!("storage error: {message}"))
}

async fn read_locations(store: &impl AdminStore) -> Result<Vec<HikeLocation>, Outcome> {
    let bytes = store.get(LOCATIONS_KEY).await.map_err(upstream)?;
    // A missing mapping is an empty one: the bucket starts out without the
    // object, and `hike-club-api` falls back to its embedded copy until this
    // worker writes it.
    let Some(bytes) = bytes else {
        return Ok(Vec::new());
    };
    serde_json::from_slice(&bytes)
        .map_err(|e| Outcome::error(502, format!("stored locations are not valid JSON: {e}")))
}

async fn load_record(store: &impl AdminStore, slug: &str) -> Result<Option<HikeRecord>, Outcome> {
    let bytes = store.get(&HikeRecord::key(slug)).await.map_err(upstream)?;
    let Some(bytes) = bytes else {
        return Ok(None);
    };
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|e| Outcome::error(502, format!("stored record is not valid JSON: {e}")))
}

pub async fn get_hike(store: &impl AdminStore, slug: &str) -> Outcome {
    if let Err(invalid) = validate::validate_slug(slug) {
        return invalid.into();
    }
    match load_record(store, slug).await {
        Ok(Some(record)) => Outcome::json(200, &record),
        Ok(None) => Outcome::error(404, format!("no hike scheduled for '{slug}'")),
        Err(outcome) => outcome,
    }
}

pub async fn put_hike(store: &impl AdminStore, slug: &str, body: &[u8]) -> Outcome {
    let locations = match read_locations(store).await {
        Ok(l) => l,
        Err(outcome) => return outcome,
    };
    let request: HikeRequest = match serde_json::from_slice(body) {
        Ok(r) => r,
        Err(e) => return Outcome::error(400, format!("invalid hike JSON: {e}")),
    };
    let record = match validate::build_record(slug, &request, &locations) {
        Ok(record) => record,
        Err(invalid) => return invalid.into(),
    };
    let encoded = match serde_json::to_vec(&record) {
        Ok(e) => e,
        Err(e) => return Outcome::error(500, format!("could not encode record: {e}")),
    };
    match store
        .put(&HikeRecord::key(slug), encoded, "application/json")
        .await
    {
        Ok(()) => Outcome::json(200, &record),
        Err(e) => upstream(e),
    }
}

/// Deletes the record only. The trail map is left in place so rescheduling the
/// same location doesn't need a re-upload.
pub async fn delete_hike(store: &impl AdminStore, slug: &str) -> Outcome {
    if let Err(invalid) = validate::validate_slug(slug) {
        return invalid.into();
    }
    match store.delete(&HikeRecord::key(slug)).await {
        Ok(()) => Outcome::no_content(),
        Err(e) => upstream(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::fake::InMemoryStore;

    const LOCATIONS: &str = r#"[
        {"short_name":"cantigny-park","full_name":"Cantigny"},
        {"short_name":"danada-equestrian-center","full_name":"Danada"}
    ]"#;

    const REQUEST: &str = r#"{
        "start":"2026-09-26T09:00:00-05:00",
        "end":"2026-09-26T11:00:00-05:00",
        "meeting":{"lat":41.855026,"lon":-88.152169},
        "trails":["Purple"]
    }"#;

    fn seeded() -> InMemoryStore {
        InMemoryStore::new().with_json(LOCATIONS_KEY, LOCATIONS)
    }

    fn body_json(outcome: &Outcome) -> serde_json::Value {
        serde_json::from_slice(&outcome.body).expect("body should be JSON")
    }

    #[tokio::test]
    async fn put_then_get_round_trips_a_hike() {
        let store = seeded();
        let put = put_hike(&store, "cantigny-park", REQUEST.as_bytes()).await;
        assert_eq!(put.status, 200);

        let got = get_hike(&store, "cantigny-park").await;
        assert_eq!(got.status, 200);
        assert_eq!(body_json(&got)["start"], "2026-09-26T09:00:00-05:00");
        assert_eq!(body_json(&got)["id"], "cantigny-park");
    }

    /// The stored bytes are what `hike-club-api` reads, so assert on them
    /// directly rather than on the response.
    #[tokio::test]
    async fn put_writes_the_record_to_the_key_the_public_api_reads() {
        let store = seeded();
        put_hike(&store, "cantigny-park", REQUEST.as_bytes()).await;
        let stored = store.read("hikes/cantigny-park.json").unwrap();
        let stored: serde_json::Value = serde_json::from_str(&stored).unwrap();
        assert_eq!(stored["mapKey"], "hikes/cantigny-park/map.png");
        assert_eq!(
            store.content_type("hikes/cantigny-park.json").unwrap(),
            "application/json"
        );
    }

    /// Rescheduling overwrites in place — same key, same id — so existing
    /// /hike/{slug} links and QR codes keep working.
    #[tokio::test]
    async fn rescheduling_overwrites_in_place() {
        let store = seeded();
        put_hike(&store, "cantigny-park", REQUEST.as_bytes()).await;
        let later = REQUEST.replace("2026-09-26", "2026-10-03");
        put_hike(&store, "cantigny-park", later.as_bytes()).await;

        assert_eq!(
            store.keys(),
            vec!["hikes/cantigny-park.json", LOCATIONS_KEY]
        );
        let got = get_hike(&store, "cantigny-park").await;
        assert_eq!(body_json(&got)["start"], "2026-10-03T09:00:00-05:00");
    }

    #[tokio::test]
    async fn put_rejects_an_unknown_location() {
        let outcome = put_hike(&seeded(), "somewhere-else", REQUEST.as_bytes()).await;
        assert_eq!(outcome.status, 400);
        assert!(
            body_json(&outcome)["error"]
                .as_str()
                .unwrap()
                .contains("unknown location")
        );
    }

    /// A slug that could escape its key prefix must never reach the store.
    #[tokio::test]
    async fn put_rejects_a_path_traversing_slug_without_writing() {
        let store = seeded();
        let outcome = put_hike(&store, "../secrets", REQUEST.as_bytes()).await;
        assert_eq!(outcome.status, 400);
        assert_eq!(store.keys(), vec![LOCATIONS_KEY]);
    }

    #[tokio::test]
    async fn put_rejects_malformed_json() {
        let outcome = put_hike(&seeded(), "cantigny-park", b"not json").await;
        assert_eq!(outcome.status, 400);
    }

    #[tokio::test]
    async fn put_rejects_an_invalid_body() {
        let bad = REQUEST.replace("\"Purple\"", "");
        let outcome = put_hike(&seeded(), "cantigny-park", bad.as_bytes()).await;
        assert_eq!(outcome.status, 400);
    }

    #[tokio::test]
    async fn get_is_404_for_an_unscheduled_location() {
        let outcome = get_hike(&seeded(), "danada-equestrian-center").await;
        assert_eq!(outcome.status, 404);
    }

    #[tokio::test]
    async fn get_rejects_an_invalid_slug() {
        assert_eq!(get_hike(&seeded(), "Not A Slug").await.status, 400);
    }

    #[tokio::test]
    async fn delete_removes_the_record_and_is_idempotent() {
        let store = seeded();
        put_hike(&store, "cantigny-park", REQUEST.as_bytes()).await;

        let outcome = delete_hike(&store, "cantigny-park").await;
        assert_eq!(outcome.status, 204);
        assert!(outcome.body.is_empty());
        assert_eq!(get_hike(&store, "cantigny-park").await.status, 404);

        assert_eq!(delete_hike(&store, "cantigny-park").await.status, 204);
    }

    #[tokio::test]
    async fn delete_rejects_an_invalid_slug() {
        assert_eq!(delete_hike(&seeded(), "../secrets").await.status, 400);
    }

    #[tokio::test]
    async fn storage_failures_surface_as_502() {
        let store = InMemoryStore::failing();
        assert_eq!(get_hike(&store, "cantigny-park").await.status, 502);
        assert_eq!(
            put_hike(&store, "cantigny-park", REQUEST.as_bytes())
                .await
                .status,
            502
        );
        assert_eq!(delete_hike(&store, "cantigny-park").await.status, 502);
    }

    #[tokio::test]
    async fn corrupt_stored_json_is_502_not_a_panic() {
        let store = seeded().with_json("hikes/cantigny-park.json", "{oops");
        assert_eq!(get_hike(&store, "cantigny-park").await.status, 502);

        let store = seeded().with_json(LOCATIONS_KEY, "{oops");
        assert_eq!(
            put_hike(&store, "cantigny-park", REQUEST.as_bytes())
                .await
                .status,
            502
        );
    }

    /// The bucket starts out without the mapping object; `hike-club-api` falls
    /// back to its embedded copy until this worker writes one. An absent
    /// mapping must not 500 — it just means nothing is writable yet.
    #[tokio::test]
    async fn a_missing_locations_object_reads_as_empty() {
        let store = InMemoryStore::new();
        let outcome = put_hike(&store, "cantigny-park", REQUEST.as_bytes()).await;
        assert_eq!(outcome.status, 400);
    }
}
