//! Schema-conformance contract test: proves the Rust types serialize into shapes
//! that validate against `openapi.yaml`, and — the part a write API needs that a
//! read-only one doesn't — that the spec's own constraints are at least as
//! strict as `validate.rs`. A limit tightened in one place and not the other
//! fails here. Runs in-process: no network, no deployed worker.
use hike_club_admin::models::{
    ErrorBody, HikeLocation, HikeRecord, HikeRequest, HikeSummary, MeetingCoords,
};
use hike_club_admin::validate::{
    self, MAX_LOCATIONS, MAX_NAME_LEN, MAX_SLUG_LEN, MAX_TRAILS, build_record,
};

const OPENAPI_YAML: &str = include_str!("../openapi.yaml");

/// OpenAPI 3.0's `nullable: true` isn't a JSON Schema keyword a generic
/// validator understands. Translate it the way OpenAPI tooling does: a
/// `type: X` becomes `anyOf: [{type: "null"}, {type: X, ...rest}]`, and a
/// `$ref`/`allOf` wrapper becomes `anyOf: [{type: "null"}, {allOf: [...]}]`.
// @spec CONTRACT-008
fn desugar_nullable(value: &mut serde_json::Value) {
    if let Some(obj) = value.as_object_mut() {
        for v in obj.values_mut() {
            desugar_nullable(v);
        }
        if obj.remove("nullable").is_some() {
            let rest = std::mem::take(obj);
            obj.insert(
                "anyOf".to_string(),
                serde_json::json!([{ "type": "null" }, serde_json::Value::Object(rest)]),
            );
        }
    } else if let Some(arr) = value.as_array_mut() {
        for v in arr.iter_mut() {
            desugar_nullable(v);
        }
    }
}

fn spec() -> serde_json::Value {
    serde_json::to_value(serde_yaml::from_str::<serde_yaml::Value>(OPENAPI_YAML).unwrap()).unwrap()
}

fn validator_for(root: &str) -> jsonschema::Validator {
    let schemas = spec()["components"]["schemas"].clone();

    // jsonschema needs a self-contained draft-07 document; rewrite OpenAPI's
    // "#/components/schemas/X" refs to plain "#/definitions/X" and nest the
    // component schemas there.
    let rewritten = schemas
        .to_string()
        .replace("#/components/schemas/", "#/definitions/");
    let mut definitions: serde_json::Value = serde_json::from_str(&rewritten).unwrap();
    desugar_nullable(&mut definitions);

    let schema = serde_json::json!({
        "$schema": "http://json-schema.org/draft-07/schema#",
        "$ref": format!("#/definitions/{root}"),
        "definitions": definitions,
    });

    jsonschema::validator_for(&schema).expect("openapi.yaml schema must compile")
}

fn locations() -> Vec<HikeLocation> {
    vec![HikeLocation {
        short_name: "cantigny-park".to_string(),
        full_name: "Cantigny".to_string(),
    }]
}

fn sample_request() -> HikeRequest {
    HikeRequest {
        meeting: MeetingCoords {
            lat: 41.855_026,
            lon: -88.152_169,
        },
        trails: vec!["Purple".to_string()],
    }
}

#[test]
// @spec CONTRACT-001
fn every_schema_in_the_spec_compiles() {
    let schemas = spec();
    let schemas = schemas["components"]["schemas"].as_object().unwrap();
    for name in schemas.keys() {
        let _ = validator_for(name);
    }
}

#[test]
// @spec CONTRACT-002
fn hike_request_matches_spec() {
    let validator = validator_for("HikeRequest");
    let body = serde_json::to_value(sample_request()).unwrap();
    assert!(validator.is_valid(&body), "{body}");
}

/// The cross-repo contract. These are the exact bytes `hike-club-api` reads out
/// of R2 and deserializes into its own `HikeRecord`; the spec's schema mirrors
/// that struct, so a rename here fails the build instead of the public API.
#[test]
// @spec CONTRACT-002, CONTRACT-003
fn stored_hike_record_matches_spec() {
    let record = build_record("cantigny-park", &sample_request(), &locations()).unwrap();
    let body = serde_json::to_value(&record).unwrap();
    assert!(validator_for("HikeRecord").is_valid(&body), "{body}");

    // Sorted: JSON object order is not meaningful, the exact key *set* is.
    let keys: Vec<_> = body.as_object().unwrap().keys().cloned().collect();
    assert_eq!(
        keys,
        vec!["id", "mapKey", "meeting", "trails"],
        "field names are read by hike-club-api/src/models.rs::HikeRecord"
    );
}

#[test]
// @spec CONTRACT-002
fn hike_summary_matches_spec_scheduled_and_not() {
    let validator = validator_for("HikeSummary");

    let scheduled = HikeSummary {
        short_name: "cantigny-park".to_string(),
        full_name: "Cantigny".to_string(),
        scheduled: true,
        has_map: true,
        trail: Some("Purple".to_string()),
    };
    let body = serde_json::to_value(&scheduled).unwrap();
    assert!(validator.is_valid(&body), "{body}");

    let unscheduled = HikeSummary {
        scheduled: false,
        has_map: false,
        trail: None,
        ..scheduled
    };
    let body = serde_json::to_value(&unscheduled).unwrap();
    assert!(validator.is_valid(&body), "{body}");
}

#[test]
// @spec CONTRACT-002
fn locations_and_error_bodies_match_spec() {
    let body = serde_json::to_value(locations()).unwrap();
    assert!(validator_for("HikeLocations").is_valid(&body), "{body}");

    let body = serde_json::to_value(ErrorBody::new("nope")).unwrap();
    assert!(validator_for("Error").is_valid(&body), "{body}");
}

/// The spec is documentation *and* an assertion about the implementation. If a
/// limit moves in `validate.rs` without moving in `openapi.yaml`, the spec is
/// lying to whoever reads it — fail here rather than let them drift.
#[test]
// @spec CONTRACT-004
fn spec_limits_match_validate_limits() {
    let spec = spec();
    let schemas = &spec["components"]["schemas"];

    assert_eq!(schemas["Slug"]["maxLength"], MAX_SLUG_LEN);
    assert_eq!(schemas["Slug"]["pattern"], "^[a-z0-9]+(-[a-z0-9]+)*$");
    assert_eq!(schemas["Trails"]["maxItems"], MAX_TRAILS);
    assert_eq!(schemas["Trails"]["minItems"], 1);
    assert_eq!(schemas["Trails"]["items"]["maxLength"], MAX_NAME_LEN);
    assert_eq!(
        schemas["HikeLocation"]["properties"]["full_name"]["maxLength"],
        MAX_NAME_LEN
    );
    assert_eq!(schemas["HikeLocations"]["maxItems"], MAX_LOCATIONS);

    let meeting = &schemas["MeetingCoords"]["properties"];
    assert_eq!(meeting["lat"]["minimum"], -90);
    assert_eq!(meeting["lat"]["maximum"], 90);
    assert_eq!(meeting["lon"]["minimum"], -180);
    assert_eq!(meeting["lon"]["maximum"], 180);
}

/// Anything `validate.rs` turns away on shape grounds must also fail the spec.
/// A rejection path with no matching spec constraint is drift, and the point of
/// keeping a spec for a write API is catching exactly that.
#[test]
// @spec CONTRACT-005
fn spec_rejects_every_body_validate_rejects() {
    let validator = validator_for("HikeRequest");

    let cases: Vec<(&str, serde_json::Value)> = vec![
        (
            "empty trails",
            serde_json::json!({
                "meeting": {"lat": 41.0, "lon": -88.0},
                "trails": [],
            }),
        ),
        (
            "too many trails",
            serde_json::json!({
                "meeting": {"lat": 41.0, "lon": -88.0},
                "trails": vec!["Loop"; MAX_TRAILS + 1],
            }),
        ),
        (
            "blank trail name",
            serde_json::json!({
                "meeting": {"lat": 41.0, "lon": -88.0},
                "trails": [""],
            }),
        ),
        (
            "latitude out of range",
            serde_json::json!({
                "meeting": {"lat": 91.0, "lon": -88.0},
                "trails": ["Purple"],
            }),
        ),
        (
            "longitude out of range",
            serde_json::json!({
                "meeting": {"lat": 41.0, "lon": 181.0},
                "trails": ["Purple"],
            }),
        ),
        // The server sets these itself; accepting them in a body would be the
        // bug this whole module exists to prevent.
        (
            "smuggled mapKey",
            serde_json::json!({
                "meeting": {"lat": 41.0, "lon": -88.0},
                "trails": ["Purple"],
                "mapKey": "hikes/../../secrets/map.png",
            }),
        ),
        (
            "smuggled id",
            serde_json::json!({
                "meeting": {"lat": 41.0, "lon": -88.0},
                "trails": ["Purple"],
                "id": "somewhere-else",
            }),
        ),
        // The record no longer carries a date; a body still sending one is
        // an unknown field like any other.
        (
            "smuggled start",
            serde_json::json!({
                "meeting": {"lat": 41.0, "lon": -88.0},
                "trails": ["Purple"],
                "start": "2026-09-26T09:00:00-05:00",
            }),
        ),
    ];

    for (label, body) in cases {
        assert!(!validator.is_valid(&body), "spec should reject: {label}");
    }
}

/// `map_key_for` builds the key from a format string while the spec pins it with
/// a pattern, and the two are maintained by hand. Assert a real derived key
/// against the real pattern so a change to either side fails here.
#[test]
// @spec CONTRACT-011
fn a_derived_map_key_matches_the_spec_pattern() {
    let spec = spec();
    let pattern = spec["components"]["schemas"]["HikeRecord"]["properties"]["mapKey"]["pattern"]
        .as_str()
        .expect("HikeRecord.mapKey must carry a pattern");
    let validator = jsonschema::validator_for(&serde_json::json!({
        "type": "string",
        "pattern": pattern,
    }))
    .expect("the mapKey pattern must compile");

    for slug in ["cantigny-park", "a1", "pratts-wayne-woods-forest-preserve"] {
        let key = HikeRecord::map_key_for(slug);
        assert!(
            validator.is_valid(&serde_json::json!(key)),
            "derived key {key:?} does not match the spec pattern {pattern:?}"
        );
    }
}

/// The mirror of the above for slugs, which travel in the path rather than the
/// body and so are checked against the `Slug` schema.
#[test]
// @spec CONTRACT-006
fn spec_rejects_every_slug_validate_rejects() {
    let validator = validator_for("Slug");
    for bad in [
        "",
        "..",
        "../secrets",
        "hikes/x",
        "Danada",
        "a b",
        "-lead",
        "trail-",
        "a--b",
    ] {
        let body = serde_json::json!(bad);
        assert!(
            !validator.is_valid(&body),
            "spec should reject slug {bad:?}"
        );
        assert!(
            validate::validate_slug(bad).is_err(),
            "validate.rs should reject slug {bad:?}"
        );
    }
    for good in ["cantigny-park", "pratts-wayne-woods-forest-preserve", "a1"] {
        assert!(validator.is_valid(&serde_json::json!(good)), "{good}");
        assert!(validate::validate_slug(good).is_ok(), "{good}");
    }
}

/// The spec only verifies anything if it describes the routes that actually
/// exist. Compare it against the router both ways: a route with no spec entry
/// is undocumented, a spec entry with no route is a promise the worker doesn't
/// keep. Matching on source text is crude, but the router is a dozen lines and
/// this needs no Workers runtime to run.
#[test]
// @spec CONTRACT-007
fn the_router_and_the_spec_describe_the_same_routes() {
    const LIB_RS: &str = include_str!("../src/lib.rs");

    let spec = spec();
    let paths = spec["paths"].as_object().unwrap();

    let mut expected: Vec<String> = Vec::new();
    for (path, item) in paths {
        // OpenAPI's {slug} is the router's :slug.
        let route = path.replace("{slug}", ":slug");
        for method in item.as_object().unwrap().keys() {
            if method == "parameters" {
                continue;
            }
            expected.push(format!(".{method}_async(\"{route}\""));
        }
    }

    for route in &expected {
        assert!(
            LIB_RS.contains(route),
            "openapi.yaml describes a route the worker does not serve: {route}"
        );
    }

    let served = LIB_RS.matches("_async(\"").count();
    assert_eq!(
        served,
        expected.len(),
        "the worker serves {served} routes but openapi.yaml describes {}",
        expected.len()
    );
}

/// The admin page is served from a binary that sits behind Cloudflare Access.
/// A third-party asset — a font, an icon set, a script — would be fetched by
/// the viewer's browser from outside that perimeter, so the page carries none.
///
/// The check is deliberately blunt: no `://` anywhere in the file. Same-origin
/// references (`/api/map/{slug}`) are relative and pass; a URL written in a
/// comment would fail. If a comment ever genuinely needs one, narrow this to
/// scan `href`/`src` attribute values rather than loosening it.
// @spec STORE-016
#[test]
fn admin_page_references_no_third_party_origin() {
    const INDEX_HTML: &str = include_str!("../src/index.html");

    let offenders: Vec<&str> = INDEX_HTML
        .lines()
        .filter(|line| line.contains("://"))
        .collect();

    assert!(
        offenders.is_empty(),
        "the admin page must reference no third-party origin, but found:\n{}",
        offenders.join("\n")
    );
}
