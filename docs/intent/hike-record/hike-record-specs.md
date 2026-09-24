# Hike Record — Specs

EARS specs for `hikes/{slug}.json`: its lifecycle, the derived summary list,
and the authoring surface. Prefix `HIKE`; see `hike-record-design.md`.

## Record Lifecycle

- [x] **HIKE-REC-001**: The system shall store at most one hike record per location, at the R2 key `hikes/{slug}.json`.
- [x] **HIKE-REC-002**: When a hike is saved for a location that already has a record, the system shall overwrite that record in place, keeping the same key and the same `id`.
- [x] **HIKE-REC-003**: When building a hike record, the system shall derive `id` and `mapKey` from the path slug and shall not read either from the request body.
- [x] **HIKE-REC-005**: When a hike is saved, the system shall require between 1 and 20 trail names, each non-blank and at most 100 characters.
- [x] **HIKE-REC-006**: When storing trail names, the system shall trim surrounding whitespace from each name.
- [x] **HIKE-REC-007**: When a hike is saved, the system shall require a finite meeting latitude within -90 to 90 and a finite meeting longitude within -180 to 180.
- [x] **HIKE-REC-008**: If a hike record is requested for a location that has none, then the system shall respond 404 rather than an empty record.
- [x] **HIKE-REC-009**: When a hike is unscheduled, the system shall delete only the hike record and shall leave that location's trail map in place.
- [x] **HIKE-REC-010**: When a hike is unscheduled for a location that has no record, the system shall respond 204, so repeating the request is safe.
- [x] **HIKE-REC-011**: The system shall neither require nor write `start` or `end` on a hike record; a record's date and time belong to hike-club-app, not to this repo.

## Summary Listing

- [x] **HIKE-LIST-001**: The system shall return one summary row per known location, whether or not a hike is scheduled there.
- [x] **HIKE-LIST-002**: When assembling the summary list, the system shall use a single prefix listing of `hikes/` to determine which locations have a record and which have a trail map, fetching a record only where its key is present.
- [x] **HIKE-LIST-003**: Where a location has a scheduled hike, the summary row shall carry that record's first trail name, so the list can render its blaze colour without a further request per row.
- [x] **HIKE-LIST-004**: If a stored hike record is not valid JSON, then the system shall fail the whole summary listing with 502 rather than report that location as unscheduled.
- [x] **HIKE-LIST-005**: Where a location has a trail map but no hike record, the system shall report it as unscheduled with a map, not as a scheduled hike.

## Authoring Surface

- [x] **HIKE-UI-006**: When a hike is saved successfully, the admin page shall briefly highlight that location's row, except where the viewer has requested reduced motion.
- [x] **HIKE-UI-008**: The admin page shall follow the viewer's light or dark colour-scheme preference.
