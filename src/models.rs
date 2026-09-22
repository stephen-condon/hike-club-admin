use serde::{Deserialize, Serialize};

/// Meeting-point coordinates, as stored. Mirrors `MeetingCoords` in
/// `hike-club-api/src/models.rs`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MeetingCoords {
    pub lat: f64,
    pub lon: f64,
}

/// The object stored at `hikes/{slug}.json`.
///
/// This is the cross-repo contract: `hike-club-api` deserializes exactly these
/// bytes into its own `HikeRecord`. Field names and `serde` renames here must
/// match `openapi.yaml`'s `HikeRecord`, which `tests/contract.rs` enforces.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HikeRecord {
    pub id: String,
    pub meeting: MeetingCoords,
    pub trails: Vec<String>,
    #[serde(rename = "mapKey")]
    pub map_key: String,
}

impl HikeRecord {
    /// The R2 key a record for `slug` is stored under.
    // @spec HIKE-REC-001
    pub fn key(slug: &str) -> String {
        format!("hikes/{slug}.json")
    }

    /// The R2 key of `slug`'s trail map. The server always derives this — it is
    /// never taken from a request body, so a client cannot aim a write at an
    /// arbitrary object.
    // @spec MAP-001, TRUST-005
    pub fn map_key_for(slug: &str) -> String {
        format!("hikes/{slug}/map.png")
    }
}

/// The body of `PUT /api/hikes/{slug}`. Has no `id` or `mapKey`: both are
/// derived from the path slug when the record is built.
///
/// `deny_unknown_fields` matches `additionalProperties: false` on the spec's
/// `HikeRequest`, so a smuggled field is refused here for the same reason the
/// spec refuses it rather than being silently dropped. [`HikeRecord`] is
/// deliberately *not* strict: it deserializes stored data, where tolerating an
/// unexpected field keeps a readable record readable.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
// @spec TRUST-006, CONTRACT-010, HIKE-REC-011
pub struct HikeRequest {
    pub meeting: MeetingCoords,
    pub trails: Vec<String>,
}

/// One entry of the location mapping the public API serves as
/// `GET /hike-locations`, stored at [`LOCATIONS_KEY`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HikeLocation {
    pub short_name: String,
    pub full_name: String,
}

/// R2 key of the location mapping. `hike-club-api` reads this same key and has
/// no copy of its own, so it answers `500` while the object is absent.
pub const LOCATIONS_KEY: &str = "resources/hike-locations.json";

/// One row of the admin list: a location, plus its hike if one is scheduled.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HikeSummary {
    #[serde(rename = "shortName")]
    pub short_name: String,
    #[serde(rename = "fullName")]
    pub full_name: String,
    pub scheduled: bool,
    #[serde(rename = "hasMap")]
    pub has_map: bool,
    /// The record's first trail. Trail blazes are painted in the trail's own
    /// colour and the club names its trails for those colours, so the UI uses
    /// this to colour the row — which saves a GET per location too.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trail: Option<String>,
}

/// One slug with stored objects but no entry in the location mapping. Its
/// objects are stranded: removing a location leaves them behind rather than
/// deleting them, so re-adding the location is what recovers them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrphanedObject {
    pub slug: String,
    #[serde(rename = "hasRecord")]
    pub has_record: bool,
    #[serde(rename = "hasMap")]
    pub has_map: bool,
}

/// The body of every non-2xx response.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ErrorBody {
    pub error: String,
}

impl ErrorBody {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            error: message.into(),
        }
    }
}
