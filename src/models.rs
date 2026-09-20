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
    pub start: String,
    pub end: String,
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
// @spec TRUST-006
pub struct HikeRequest {
    pub start: String,
    pub end: String,
    pub meeting: MeetingCoords,
    pub trails: Vec<String>,
}

/// One entry of the location mapping served verbatim by the public API's
/// `GET /hike-locations`, stored at [`LOCATIONS_KEY`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HikeLocation {
    pub short_name: String,
    pub full_name: String,
}

/// R2 key of the location mapping. `hike-club-api` reads this same key, falling
/// back to its embedded copy when the object is absent.
pub const LOCATIONS_KEY: &str = "resources/hike-locations.json";

/// One row of the admin list: a location, plus its hike if one is scheduled.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HikeSummary {
    #[serde(rename = "shortName")]
    pub short_name: String,
    #[serde(rename = "fullName")]
    pub full_name: String,
    pub scheduled: bool,
    /// The record's `end` is in the past. Such a record makes the public API
    /// serve the *last* hike's observed weather as though it were current, so
    /// the UI flags it rather than letting it fail silently.
    pub stale: bool,
    #[serde(rename = "hasMap")]
    pub has_map: bool,
    /// The record's first trail. Trail blazes are painted in the trail's own
    /// colour and the club names its trails for those colours, so the UI uses
    /// this to colour the row — which saves a GET per location too.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end: Option<String>,
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
