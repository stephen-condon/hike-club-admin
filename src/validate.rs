//! The trust boundary. This worker is the only writer to the R2 bucket the
//! public API reads, so every request body is validated here before anything is
//! stored — nothing about a request is taken on trust, least of all which key
//! gets written.
//!
//! The limits below are mirrored in `openapi.yaml`; `tests/contract.rs` asserts
//! the spec is at least as strict as this module so the two can't drift.
use crate::models::{HikeLocation, HikeRecord, HikeRequest, MeetingCoords};
use chrono::{DateTime, Utc};

/// Matches `Slug.maxLength` in openapi.yaml.
pub const MAX_SLUG_LEN: usize = 64;
/// Matches `Trails.maxItems`.
pub const MAX_TRAILS: usize = 20;
/// Matches the `maxLength` of a `Trails` item and of `HikeLocation.full_name`.
pub const MAX_NAME_LEN: usize = 100;
/// Matches `HikeLocations.maxItems`.
pub const MAX_LOCATIONS: usize = 200;
/// Existing trail maps are 280 KB – 1.1 MB, so 5 MB is generous headroom.
pub const MAX_MAP_BYTES: usize = 5 * 1024 * 1024;

/// A rejected request: the message to return and the status to return it with.
/// Everything is a 400 except map uploads, which distinguish 415 and 413.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invalid {
    pub status: u16,
    pub message: String,
}

impl Invalid {
    fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: 400,
            message: message.into(),
        }
    }
}

type Checked<T> = Result<T, Invalid>;

/// Enforces `^[a-z0-9]+(-[a-z0-9]+)*$` without pulling in `regex` — lowercase
/// alphanumerics and single interior hyphens. This is what keeps a slug from
/// escaping its prefix (`..`, `/`, uppercase, unicode) when it's interpolated
/// into an R2 key.
// @spec TRUST-001, TRUST-002, TRUST-003, TRUST-004
pub fn validate_slug(slug: &str) -> Checked<()> {
    if slug.is_empty() {
        return Err(Invalid::bad_request("slug must not be empty"));
    }
    if slug.len() > MAX_SLUG_LEN {
        return Err(Invalid::bad_request(format!(
            "slug must be at most {MAX_SLUG_LEN} characters"
        )));
    }
    if slug.starts_with('-') || slug.ends_with('-') || slug.contains("--") {
        return Err(Invalid::bad_request(
            "slug must not start, end, or double up on '-'",
        ));
    }
    if !slug
        .bytes()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    {
        return Err(Invalid::bad_request(
            "slug must contain only lowercase letters, digits and '-'",
        ));
    }
    Ok(())
}

/// A well-formed slug that is also a *known* location. Writes are confined to
/// locations that exist, so the mapping doubles as the allowlist of keys this
/// worker may create.
///
/// Read and delete paths deliberately call [`validate_slug`] alone rather than
/// this: they create nothing, so the pattern is the whole defence, checking
/// membership would cost a mapping fetch per read, and an unknown location is
/// better answered 404 ("no hike scheduled") than 400 ("unknown location").
// @spec TRUST-009, LOC-009
pub fn validate_known_slug(slug: &str, locations: &[HikeLocation]) -> Checked<()> {
    validate_slug(slug)?;
    if !locations.iter().any(|l| l.short_name == slug) {
        return Err(Invalid::bad_request(format!(
            "unknown location '{slug}' — add it under Locations first"
        )));
    }
    Ok(())
}

fn parse_rfc3339(label: &str, value: &str) -> Checked<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .map(|d| d.with_timezone(&Utc))
        .map_err(|_| Invalid::bad_request(format!("{label} must be an RFC 3339 timestamp")))
}

fn validate_meeting(meeting: &MeetingCoords) -> Checked<()> {
    if !meeting.lat.is_finite() || !(-90.0..=90.0).contains(&meeting.lat) {
        return Err(Invalid::bad_request(
            "meeting.lat must be between -90 and 90",
        ));
    }
    if !meeting.lon.is_finite() || !(-180.0..=180.0).contains(&meeting.lon) {
        return Err(Invalid::bad_request(
            "meeting.lon must be between -180 and 180",
        ));
    }
    Ok(())
}

fn validate_trails(trails: &[String]) -> Checked<()> {
    if trails.is_empty() {
        return Err(Invalid::bad_request("at least one trail is required"));
    }
    if trails.len() > MAX_TRAILS {
        return Err(Invalid::bad_request(format!(
            "at most {MAX_TRAILS} trails are allowed"
        )));
    }
    for trail in trails {
        if trail.trim().is_empty() {
            return Err(Invalid::bad_request("trail names must not be blank"));
        }
        if trail.chars().count() > MAX_NAME_LEN {
            return Err(Invalid::bad_request(format!(
                "trail names must be at most {MAX_NAME_LEN} characters"
            )));
        }
    }
    Ok(())
}

/// Validates a `PUT /api/hikes/{slug}` body and builds the record to store.
///
/// `id` and `map_key` come from the *path* slug, never from the body — that's
/// why [`HikeRequest`] has no such fields. A client cannot point a write at an
/// object other than its own location's.
// @spec HIKE-REC-003, HIKE-REC-004, HIKE-REC-005, HIKE-REC-006, HIKE-REC-007, TRUST-005, TRUST-007
pub fn build_record(
    slug: &str,
    request: &HikeRequest,
    locations: &[HikeLocation],
) -> Checked<HikeRecord> {
    validate_known_slug(slug, locations)?;

    let start = parse_rfc3339("start", &request.start)?;
    let end = parse_rfc3339("end", &request.end)?;
    if end <= start {
        return Err(Invalid::bad_request("end must be after start"));
    }
    validate_meeting(&request.meeting)?;
    validate_trails(&request.trails)?;

    Ok(HikeRecord {
        id: slug.to_string(),
        start: request.start.clone(),
        end: request.end.clone(),
        meeting: request.meeting.clone(),
        trails: request
            .trails
            .iter()
            .map(|t| t.trim().to_string())
            .collect(),
        map_key: HikeRecord::map_key_for(slug),
    })
}

/// Validates a replacement location mapping. An empty list is allowed — it just
/// means nothing is selectable yet.
// @spec LOC-004, LOC-005, LOC-006, LOC-007
pub fn validate_locations(locations: &[HikeLocation]) -> Checked<()> {
    if locations.len() > MAX_LOCATIONS {
        return Err(Invalid::bad_request(format!(
            "at most {MAX_LOCATIONS} locations are allowed"
        )));
    }
    for location in locations {
        validate_slug(&location.short_name)?;
        if location.full_name.trim().is_empty() {
            return Err(Invalid::bad_request(format!(
                "location '{}' needs a display name",
                location.short_name
            )));
        }
        if location.full_name.chars().count() > MAX_NAME_LEN {
            return Err(Invalid::bad_request(format!(
                "display names must be at most {MAX_NAME_LEN} characters"
            )));
        }
    }
    for (i, location) in locations.iter().enumerate() {
        if locations[..i]
            .iter()
            .any(|l| l.short_name == location.short_name)
        {
            return Err(Invalid::bad_request(format!(
                "duplicate location '{}'",
                location.short_name
            )));
        }
    }
    Ok(())
}

/// Validates a trail map upload. 415 for the wrong type, 413 for too large,
/// 400 for empty: distinct statuses so the response names the actual reason.
/// The admin page renders whichever message comes back and does not branch on
/// the status, so the reason reaches the admin as text rather than as handling.
// @spec MAP-002, MAP-003, MAP-004, MAP-005
pub fn validate_map_upload(content_type: Option<&str>, len: usize) -> Checked<()> {
    // Browsers may append parameters, e.g. "image/png; charset=binary".
    let is_png = content_type
        .map(|c| {
            c.split(';')
                .next()
                .unwrap_or("")
                .trim()
                .eq_ignore_ascii_case("image/png")
        })
        .unwrap_or(false);
    if !is_png {
        return Err(Invalid {
            status: 415,
            message: "trail maps must be image/png".to_string(),
        });
    }
    if len == 0 {
        return Err(Invalid::bad_request("trail map is empty"));
    }
    if len > MAX_MAP_BYTES {
        return Err(Invalid {
            status: 413,
            message: format!(
                "trail map must be at most {} MB",
                MAX_MAP_BYTES / (1024 * 1024)
            ),
        });
    }
    Ok(())
}

/// Whether a record's `end` has already passed. An unparseable `end` counts as
/// stale: something is wrong with it either way, and the UI should say so.
// @spec HIKE-STALE-001, HIKE-STALE-002
pub fn is_stale(end: &str, now: DateTime<Utc>) -> bool {
    match DateTime::parse_from_rfc3339(end) {
        Ok(end) => end.with_timezone(&Utc) < now,
        Err(_) => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn locations() -> Vec<HikeLocation> {
        vec![
            HikeLocation {
                short_name: "danada-equestrian-center".to_string(),
                full_name: "Danada".to_string(),
            },
            HikeLocation {
                short_name: "cantigny-park".to_string(),
                full_name: "Cantigny".to_string(),
            },
        ]
    }

    fn request() -> HikeRequest {
        HikeRequest {
            start: "2026-09-26T09:00:00-05:00".to_string(),
            end: "2026-09-26T11:00:00-05:00".to_string(),
            meeting: MeetingCoords {
                lat: 41.855_026,
                lon: -88.152_169,
            },
            trails: vec!["Purple".to_string()],
        }
    }

    #[test]
    // @spec TRUST-001
    fn accepts_a_conventional_slug() {
        assert!(validate_slug("pratts-wayne-woods-forest-preserve").is_ok());
        assert!(validate_slug("cantigny-park").is_ok());
        assert!(validate_slug("a1").is_ok());
    }

    #[test]
    // @spec TRUST-002
    fn rejects_empty_slug() {
        assert_eq!(validate_slug("").unwrap_err().status, 400);
    }

    #[test]
    // @spec TRUST-002
    fn rejects_overlong_slug() {
        assert!(validate_slug(&"a".repeat(MAX_SLUG_LEN + 1)).is_err());
        assert!(validate_slug(&"a".repeat(MAX_SLUG_LEN)).is_ok());
    }

    #[test]
    // @spec TRUST-003
    fn rejects_slug_hyphen_edges_and_doubles() {
        assert!(validate_slug("-leading").is_err());
        assert!(validate_slug("trailing-").is_err());
        assert!(validate_slug("double--hyphen").is_err());
    }

    /// The reason the slug rules exist: none of these may reach an R2 key.
    #[test]
    // @spec TRUST-004
    fn rejects_slug_that_would_escape_its_key_prefix() {
        for bad in [
            "..",
            "../secrets",
            "hikes/x",
            "Danada",
            "a b",
            "map.png",
            "café",
        ] {
            assert!(validate_slug(bad).is_err(), "should reject {bad:?}");
        }
    }

    #[test]
    // @spec LOC-009
    fn rejects_unknown_location() {
        let err = validate_known_slug("not-a-place", &locations()).unwrap_err();
        assert_eq!(err.status, 400);
        assert!(err.message.contains("unknown location"));
    }

    #[test]
    // @spec LOC-009
    fn accepts_known_location() {
        assert!(validate_known_slug("cantigny-park", &locations()).is_ok());
    }

    #[test]
    // @spec HIKE-REC-003
    fn builds_a_record_from_a_valid_request() {
        let record = build_record("cantigny-park", &request(), &locations()).unwrap();
        assert_eq!(record.id, "cantigny-park");
        assert_eq!(record.start, "2026-09-26T09:00:00-05:00");
        assert_eq!(record.trails, vec!["Purple".to_string()]);
    }

    /// The write target is derived from the path, so a hostile body can't aim
    /// it somewhere else — there is nowhere in `HikeRequest` to put one.
    #[test]
    // @spec TRUST-005, TRUST-007, HIKE-REC-001, MAP-001
    fn map_key_is_server_derived_not_client_supplied() {
        let record = build_record("cantigny-park", &request(), &locations()).unwrap();
        assert_eq!(record.map_key, "hikes/cantigny-park/map.png");
        assert_eq!(HikeRecord::key("cantigny-park"), "hikes/cantigny-park.json");
    }

    #[test]
    // @spec HIKE-REC-006
    fn trims_whitespace_around_trail_names() {
        let mut req = request();
        req.trails = vec!["  Purple  ".to_string()];
        let record = build_record("cantigny-park", &req, &locations()).unwrap();
        assert_eq!(record.trails, vec!["Purple".to_string()]);
    }

    #[test]
    // @spec HIKE-REC-004
    fn rejects_unparseable_timestamps() {
        let mut req = request();
        req.start = "2026-09-26 09:00".to_string();
        let err = build_record("cantigny-park", &req, &locations()).unwrap_err();
        assert!(err.message.contains("start"));

        let mut req = request();
        req.end = "next saturday".to_string();
        let err = build_record("cantigny-park", &req, &locations()).unwrap_err();
        assert!(err.message.contains("end"));
    }

    #[test]
    // @spec HIKE-REC-004
    fn rejects_end_before_or_equal_to_start() {
        let mut req = request();
        req.end = req.start.clone();
        assert!(build_record("cantigny-park", &req, &locations()).is_err());

        let mut req = request();
        req.end = "2026-09-26T08:00:00-05:00".to_string();
        assert!(build_record("cantigny-park", &req, &locations()).is_err());
    }

    /// Offsets differ but the instants are two hours apart, which is what counts.
    #[test]
    // @spec HIKE-REC-004
    fn compares_timestamps_as_instants_not_strings() {
        let mut req = request();
        req.start = "2026-09-26T14:00:00Z".to_string();
        req.end = "2026-09-26T11:00:00-05:00".to_string();
        assert!(build_record("cantigny-park", &req, &locations()).is_ok());
    }

    #[test]
    // @spec HIKE-REC-007
    fn rejects_out_of_range_coordinates() {
        for (lat, lon) in [(91.0, 0.0), (-91.0, 0.0), (0.0, 181.0), (0.0, -181.0)] {
            let mut req = request();
            req.meeting = MeetingCoords { lat, lon };
            assert!(
                build_record("cantigny-park", &req, &locations()).is_err(),
                "should reject {lat},{lon}"
            );
        }
    }

    #[test]
    // @spec HIKE-REC-007
    fn rejects_non_finite_coordinates() {
        let mut req = request();
        req.meeting = MeetingCoords {
            lat: f64::NAN,
            lon: 0.0,
        };
        assert!(build_record("cantigny-park", &req, &locations()).is_err());

        let mut req = request();
        req.meeting = MeetingCoords {
            lat: 0.0,
            lon: f64::INFINITY,
        };
        assert!(build_record("cantigny-park", &req, &locations()).is_err());
    }

    #[test]
    // @spec HIKE-REC-005
    fn rejects_empty_blank_or_overlong_trail_lists() {
        let mut req = request();
        req.trails = vec![];
        assert!(build_record("cantigny-park", &req, &locations()).is_err());

        let mut req = request();
        req.trails = vec!["   ".to_string()];
        assert!(build_record("cantigny-park", &req, &locations()).is_err());

        let mut req = request();
        req.trails = vec!["Loop".to_string(); MAX_TRAILS + 1];
        assert!(build_record("cantigny-park", &req, &locations()).is_err());

        let mut req = request();
        req.trails = vec!["x".repeat(MAX_NAME_LEN + 1)];
        assert!(build_record("cantigny-park", &req, &locations()).is_err());
    }

    #[test]
    // @spec LOC-007
    fn accepts_a_valid_location_list() {
        assert!(validate_locations(&locations()).is_ok());
        assert!(validate_locations(&[]).is_ok());
    }

    #[test]
    // @spec LOC-006
    fn rejects_duplicate_location_slugs() {
        let mut list = locations();
        list.push(list[0].clone());
        let err = validate_locations(&list).unwrap_err();
        assert!(err.message.contains("duplicate"));
    }

    #[test]
    // @spec LOC-004, LOC-005
    fn rejects_locations_with_bad_slug_or_blank_name() {
        let bad_slug = vec![HikeLocation {
            short_name: "Not A Slug".to_string(),
            full_name: "Somewhere".to_string(),
        }];
        assert!(validate_locations(&bad_slug).is_err());

        let blank_name = vec![HikeLocation {
            short_name: "somewhere".to_string(),
            full_name: "  ".to_string(),
        }];
        assert!(validate_locations(&blank_name).is_err());

        let long_name = vec![HikeLocation {
            short_name: "somewhere".to_string(),
            full_name: "x".repeat(MAX_NAME_LEN + 1),
        }];
        assert!(validate_locations(&long_name).is_err());
    }

    #[test]
    // @spec LOC-007
    fn rejects_too_many_locations() {
        let many: Vec<_> = (0..=MAX_LOCATIONS)
            .map(|i| HikeLocation {
                short_name: format!("place-{i}"),
                full_name: format!("Place {i}"),
            })
            .collect();
        assert!(validate_locations(&many).is_err());
    }

    #[test]
    // @spec MAP-002
    fn accepts_a_png_upload_with_parameters() {
        assert!(validate_map_upload(Some("image/png"), 1024).is_ok());
        assert!(validate_map_upload(Some("image/PNG; charset=binary"), 1024).is_ok());
    }

    #[test]
    // @spec MAP-003
    fn rejects_non_png_uploads_with_415() {
        assert_eq!(
            validate_map_upload(Some("image/jpeg"), 1024)
                .unwrap_err()
                .status,
            415
        );
        assert_eq!(validate_map_upload(None, 1024).unwrap_err().status, 415);
    }

    #[test]
    // @spec MAP-004, MAP-005
    fn rejects_empty_and_oversized_uploads() {
        assert_eq!(
            validate_map_upload(Some("image/png"), 0)
                .unwrap_err()
                .status,
            400
        );
        assert_eq!(
            validate_map_upload(Some("image/png"), MAX_MAP_BYTES + 1)
                .unwrap_err()
                .status,
            413
        );
        assert!(validate_map_upload(Some("image/png"), MAX_MAP_BYTES).is_ok());
    }

    #[test]
    // @spec HIKE-STALE-001
    fn staleness_turns_over_exactly_at_end() {
        let now = DateTime::parse_from_rfc3339("2026-09-26T11:00:00-05:00")
            .unwrap()
            .with_timezone(&Utc);
        assert!(!is_stale("2026-09-26T11:00:00-05:00", now), "end == now");
        assert!(!is_stale("2026-09-26T11:00:01-05:00", now));
        assert!(is_stale("2026-09-26T10:59:59-05:00", now));
    }

    #[test]
    // @spec HIKE-STALE-002
    fn an_unparseable_end_counts_as_stale() {
        assert!(is_stale("whenever", Utc::now()));
    }
}
