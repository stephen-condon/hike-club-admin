//! Handler logic, generic over [`AdminStore`] so every path here is unit-tested
//! against the in-memory fake — no R2, no network, no Workers runtime.
//!
//! Handlers return [`Outcome`], a status plus an already-serialized body, so
//! `lib.rs` stays pure translation into `worker::Response`.
use crate::models::{ErrorBody, HikeLocation, HikeRecord, HikeRequest, HikeSummary, LOCATIONS_KEY};
use crate::store::AdminStore;
use crate::validate::{self, Invalid};
use chrono::{DateTime, Utc};

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

    fn png(body: Vec<u8>) -> Self {
        Self {
            status: 200,
            content_type: "image/png",
            body,
        }
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

pub async fn get_map(store: &impl AdminStore, slug: &str) -> Outcome {
    if let Err(invalid) = validate::validate_slug(slug) {
        return invalid.into();
    }
    match store.get(&HikeRecord::map_key_for(slug)).await {
        Ok(Some(bytes)) => Outcome::png(bytes),
        Ok(None) => Outcome::error(404, format!("no trail map for '{slug}'")),
        Err(e) => upstream(e),
    }
}

/// Maps are uploaded once per location and shared by every record for it, so
/// this is separate from scheduling: replacing a map doesn't touch the hike,
/// and rescheduling doesn't need a re-upload.
pub async fn put_map(
    store: &impl AdminStore,
    slug: &str,
    content_type: Option<&str>,
    body: Vec<u8>,
) -> Outcome {
    let locations = match read_locations(store).await {
        Ok(l) => l,
        Err(outcome) => return outcome,
    };
    if let Err(invalid) = validate::validate_known_slug(slug, &locations) {
        return invalid.into();
    }
    if let Err(invalid) = validate::validate_map_upload(content_type, body.len()) {
        return invalid.into();
    }
    match store
        .put(&HikeRecord::map_key_for(slug), body, "image/png")
        .await
    {
        Ok(()) => Outcome::no_content(),
        Err(e) => upstream(e),
    }
}

pub async fn get_locations(store: &impl AdminStore) -> Outcome {
    match read_locations(store).await {
        Ok(locations) => Outcome::json(200, &locations),
        Err(outcome) => outcome,
    }
}

/// Replaces the whole mapping. Removing a location leaves its record and map
/// behind — they're separate objects, and orphaning them is recoverable while
/// deleting them is not.
pub async fn put_locations(store: &impl AdminStore, body: &[u8]) -> Outcome {
    let locations: Vec<HikeLocation> = match serde_json::from_slice(body) {
        Ok(l) => l,
        Err(e) => return Outcome::error(400, format!("invalid locations JSON: {e}")),
    };
    if let Err(invalid) = validate::validate_locations(&locations) {
        return invalid.into();
    }
    let encoded = match serde_json::to_vec(&locations) {
        Ok(e) => e,
        Err(e) => return Outcome::error(500, format!("could not encode locations: {e}")),
    };
    match store.put(LOCATIONS_KEY, encoded, "application/json").await {
        Ok(()) => Outcome::json(200, &locations),
        Err(e) => upstream(e),
    }
}

/// One row per known location, with its hike if one is scheduled. A single
/// prefix list answers "scheduled?" and "has a map?" for every location, so
/// only the records that actually exist get fetched.
pub async fn list_hikes(store: &impl AdminStore, now: DateTime<Utc>) -> Outcome {
    let locations = match read_locations(store).await {
        Ok(l) => l,
        Err(outcome) => return outcome,
    };
    let keys = match store.list("hikes/").await {
        Ok(k) => k,
        Err(e) => return upstream(e),
    };

    let mut summaries = Vec::with_capacity(locations.len());
    for location in &locations {
        let slug = &location.short_name;
        let has_map = keys.contains(&HikeRecord::map_key_for(slug));
        let record = if keys.contains(&HikeRecord::key(slug)) {
            match load_record(store, slug).await {
                Ok(record) => record,
                Err(outcome) => return outcome,
            }
        } else {
            None
        };

        summaries.push(HikeSummary {
            short_name: slug.clone(),
            full_name: location.full_name.clone(),
            scheduled: record.is_some(),
            // Only a scheduled hike can be stale; an empty slot is just empty.
            stale: record
                .as_ref()
                .is_some_and(|r| validate::is_stale(&r.end, now)),
            has_map,
            trail: record.as_ref().and_then(|r| r.trails.first().cloned()),
            start: record.as_ref().map(|r| r.start.clone()),
            end: record.map(|r| r.end),
        });
    }

    Outcome::json(200, &summaries)
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

    /// A 1x1 PNG — enough to prove bytes round-trip unaltered.
    const PNG: &[u8] = b"\x89PNG\r\n\x1a\nfake-but-png-enough";

    #[tokio::test]
    async fn put_then_get_round_trips_a_map() {
        let store = seeded();
        let put = put_map(&store, "cantigny-park", Some("image/png"), PNG.to_vec()).await;
        assert_eq!(put.status, 204);

        let got = get_map(&store, "cantigny-park").await;
        assert_eq!(got.status, 200);
        assert_eq!(got.content_type, "image/png");
        assert_eq!(got.body, PNG);
    }

    /// The key must match what upload-hike.sh writes and what every record's
    /// mapKey points at, or the public API presigns a URL to nothing.
    #[tokio::test]
    async fn put_writes_the_shared_map_key() {
        let store = seeded();
        put_map(&store, "cantigny-park", Some("image/png"), PNG.to_vec()).await;
        assert!(
            store
                .keys()
                .contains(&"hikes/cantigny-park/map.png".to_string())
        );
        assert_eq!(
            store.content_type("hikes/cantigny-park/map.png").unwrap(),
            "image/png"
        );
    }

    /// Replacing a map leaves the hike record alone, and vice versa.
    #[tokio::test]
    async fn map_and_record_are_independent() {
        let store = seeded();
        put_hike(&store, "cantigny-park", REQUEST.as_bytes()).await;
        put_map(&store, "cantigny-park", Some("image/png"), PNG.to_vec()).await;

        delete_hike(&store, "cantigny-park").await;
        assert_eq!(get_map(&store, "cantigny-park").await.status, 200);

        put_map(&store, "cantigny-park", Some("image/png"), b"new".to_vec()).await;
        assert_eq!(get_map(&store, "cantigny-park").await.body, b"new");
    }

    #[tokio::test]
    async fn get_map_is_404_when_none_is_uploaded() {
        assert_eq!(get_map(&seeded(), "cantigny-park").await.status, 404);
    }

    #[tokio::test]
    async fn map_endpoints_reject_invalid_slugs() {
        assert_eq!(get_map(&seeded(), "../secrets").await.status, 400);
        let store = seeded();
        let outcome = put_map(&store, "../secrets", Some("image/png"), PNG.to_vec()).await;
        assert_eq!(outcome.status, 400);
        assert_eq!(store.keys(), vec![LOCATIONS_KEY]);
    }

    #[tokio::test]
    async fn put_map_rejects_an_unknown_location() {
        let outcome = put_map(&seeded(), "somewhere-else", Some("image/png"), PNG.to_vec()).await;
        assert_eq!(outcome.status, 400);
    }

    #[tokio::test]
    async fn put_map_rejects_the_wrong_type_and_oversized_bodies() {
        let store = seeded();
        assert_eq!(
            put_map(&store, "cantigny-park", Some("image/jpeg"), PNG.to_vec())
                .await
                .status,
            415
        );
        assert_eq!(
            put_map(&store, "cantigny-park", None, PNG.to_vec())
                .await
                .status,
            415
        );
        let huge = vec![0u8; crate::validate::MAX_MAP_BYTES + 1];
        assert_eq!(
            put_map(&store, "cantigny-park", Some("image/png"), huge)
                .await
                .status,
            413
        );
        assert_eq!(store.keys(), vec![LOCATIONS_KEY]);
    }

    #[tokio::test]
    async fn map_storage_failures_surface_as_502() {
        let store = InMemoryStore::failing();
        assert_eq!(get_map(&store, "cantigny-park").await.status, 502);
        assert_eq!(
            put_map(&store, "cantigny-park", Some("image/png"), PNG.to_vec())
                .await
                .status,
            502
        );
    }

    fn at(when: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(when)
            .unwrap()
            .with_timezone(&Utc)
    }

    fn summaries(outcome: &Outcome) -> Vec<serde_json::Value> {
        serde_json::from_slice(&outcome.body).unwrap()
    }

    #[tokio::test]
    async fn get_locations_returns_the_stored_mapping() {
        let outcome = get_locations(&seeded()).await;
        assert_eq!(outcome.status, 200);
        let locations = summaries(&outcome);
        assert_eq!(locations.len(), 2);
        assert_eq!(locations[0]["short_name"], "cantigny-park");
    }

    #[tokio::test]
    async fn get_locations_is_empty_before_anything_is_written() {
        let outcome = get_locations(&InMemoryStore::new()).await;
        assert_eq!(outcome.status, 200);
        assert!(summaries(&outcome).is_empty());
    }

    /// Adding a location is what makes it writable — the mapping is the
    /// allowlist, so this is the step that unblocks PUT /api/hikes/{slug}.
    #[tokio::test]
    async fn adding_a_location_makes_it_schedulable() {
        let store = seeded();
        assert_eq!(
            put_hike(&store, "oakhurst", REQUEST.as_bytes())
                .await
                .status,
            400
        );

        let added = r#"[{"short_name":"oakhurst","full_name":"Oakhurst"}]"#;
        assert_eq!(put_locations(&store, added.as_bytes()).await.status, 200);
        assert_eq!(
            put_hike(&store, "oakhurst", REQUEST.as_bytes())
                .await
                .status,
            200
        );
    }

    #[tokio::test]
    async fn put_locations_rejects_invalid_lists() {
        let store = seeded();
        assert_eq!(put_locations(&store, b"not json").await.status, 400);

        let dupes = r#"[{"short_name":"a","full_name":"A"},{"short_name":"a","full_name":"B"}]"#;
        assert_eq!(put_locations(&store, dupes.as_bytes()).await.status, 400);

        let bad_slug = r#"[{"short_name":"Not A Slug","full_name":"X"}]"#;
        assert_eq!(put_locations(&store, bad_slug.as_bytes()).await.status, 400);

        // Nothing was written by any of the above.
        assert_eq!(get_locations(&store).await.body, seeded_locations_body());
    }

    fn seeded_locations_body() -> Vec<u8> {
        let locations: Vec<HikeLocation> = serde_json::from_str(LOCATIONS).unwrap();
        serde_json::to_vec(&locations).unwrap()
    }

    /// Removing a location orphans its record rather than deleting it, so the
    /// hike comes back intact if the location is re-added.
    #[tokio::test]
    async fn removing_a_location_leaves_its_objects_alone() {
        let store = seeded();
        put_hike(&store, "cantigny-park", REQUEST.as_bytes()).await;
        put_locations(&store, b"[]").await;

        assert!(summaries(&list_hikes(&store, at("2026-09-20T00:00:00Z")).await).is_empty());
        assert!(store.read("hikes/cantigny-park.json").is_some());

        put_locations(&store, LOCATIONS.as_bytes()).await;
        assert_eq!(get_hike(&store, "cantigny-park").await.status, 200);
    }

    #[tokio::test]
    async fn list_reports_a_row_per_location_scheduled_or_not() {
        let store = seeded();
        put_hike(&store, "cantigny-park", REQUEST.as_bytes()).await;

        let rows = summaries(&list_hikes(&store, at("2026-09-20T00:00:00Z")).await);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0]["shortName"], "cantigny-park");
        assert_eq!(rows[0]["scheduled"], true);
        assert_eq!(rows[0]["start"], "2026-09-26T09:00:00-05:00");
        assert_eq!(rows[1]["shortName"], "danada-equestrian-center");
        assert_eq!(rows[1]["scheduled"], false);
        assert_eq!(rows[1]["start"], serde_json::Value::Null);
    }

    /// The row's blaze colour comes from the trail name, so the summary carries
    /// it and the list doesn't need a GET per location to draw itself.
    #[tokio::test]
    async fn list_carries_the_first_trail_for_the_blaze() {
        let store = seeded();
        let two_trails = REQUEST.replace(r#"["Purple"]"#, r#"["Purple","Green"]"#);
        put_hike(&store, "cantigny-park", two_trails.as_bytes()).await;

        let rows = summaries(&list_hikes(&store, at("2026-09-20T00:00:00Z")).await);
        assert_eq!(rows[0]["trail"], "Purple");
        assert_eq!(rows[1]["trail"], serde_json::Value::Null);
    }

    /// The footgun this whole list exists to surface: a record whose end has
    /// passed makes the public API serve the *last* hike's observed weather as
    /// though it were current, silently.
    #[tokio::test]
    async fn list_flags_a_record_whose_end_has_passed() {
        let store = seeded();
        put_hike(&store, "cantigny-park", REQUEST.as_bytes()).await;

        let before = summaries(&list_hikes(&store, at("2026-09-26T15:59:59Z")).await);
        assert_eq!(before[0]["stale"], false);

        let after = summaries(&list_hikes(&store, at("2026-09-26T16:00:01Z")).await);
        assert_eq!(after[0]["stale"], true);
    }

    #[tokio::test]
    async fn an_unscheduled_location_is_never_stale() {
        let rows = summaries(&list_hikes(&seeded(), at("2099-01-01T00:00:00Z")).await);
        assert_eq!(rows[0]["scheduled"], false);
        assert_eq!(rows[0]["stale"], false);
    }

    #[tokio::test]
    async fn list_reports_whether_a_map_is_uploaded() {
        let store = seeded();
        let rows = summaries(&list_hikes(&store, at("2026-09-20T00:00:00Z")).await);
        assert_eq!(rows[0]["hasMap"], false);

        put_map(&store, "cantigny-park", Some("image/png"), PNG.to_vec()).await;
        let rows = summaries(&list_hikes(&store, at("2026-09-20T00:00:00Z")).await);
        assert_eq!(rows[0]["hasMap"], true);
    }

    /// The map key is hikes/{slug}/map.png and the record is hikes/{slug}.json;
    /// one prefix list returns both, and neither may be mistaken for the other.
    #[tokio::test]
    async fn a_map_alone_does_not_count_as_a_scheduled_hike() {
        let store = seeded();
        put_map(&store, "cantigny-park", Some("image/png"), PNG.to_vec()).await;
        let rows = summaries(&list_hikes(&store, at("2026-09-20T00:00:00Z")).await);
        assert_eq!(rows[0]["scheduled"], false);
        assert_eq!(rows[0]["hasMap"], true);
    }

    #[tokio::test]
    async fn location_and_list_storage_failures_surface_as_502() {
        let store = InMemoryStore::failing();
        assert_eq!(get_locations(&store).await.status, 502);
        assert_eq!(put_locations(&store, b"[]").await.status, 502);
        assert_eq!(list_hikes(&store, Utc::now()).await.status, 502);
    }

    #[tokio::test]
    async fn corrupt_stored_locations_are_502() {
        let store = InMemoryStore::new().with_json(LOCATIONS_KEY, "{oops");
        assert_eq!(get_locations(&store).await.status, 502);
        assert_eq!(list_hikes(&store, Utc::now()).await.status, 502);
    }

    #[tokio::test]
    async fn a_corrupt_record_fails_the_list_rather_than_lying_about_it() {
        let store = seeded().with_json("hikes/cantigny-park.json", "{oops");
        assert_eq!(list_hikes(&store, Utc::now()).await.status, 502);
    }
}
