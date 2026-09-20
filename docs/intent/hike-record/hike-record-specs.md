# Hike Record — Specs

EARS specs for `hikes/{slug}.json`: its lifecycle, the derived summary list,
staleness, and the authoring surface. Prefix `HIKE`; see
`hike-record-design.md`.

## Record Lifecycle

- [x] **HIKE-REC-001**: The system shall store at most one hike record per location, at the R2 key `hikes/{slug}.json`.
- [x] **HIKE-REC-002**: When a hike is saved for a location that already has a record, the system shall overwrite that record in place, keeping the same key and the same `id`.
- [x] **HIKE-REC-003**: When building a hike record, the system shall derive `id` and `mapKey` from the path slug and shall not read either from the request body.
- [x] **HIKE-REC-004**: When a hike is saved, the system shall require `start` and `end` to be RFC 3339 timestamps and `end` to be later than `start` compared as instants rather than as strings.
- [x] **HIKE-REC-005**: When a hike is saved, the system shall require between 1 and 20 trail names, each non-blank and at most 100 characters.
- [x] **HIKE-REC-006**: When storing trail names, the system shall trim surrounding whitespace from each name.
- [x] **HIKE-REC-007**: When a hike is saved, the system shall require a finite meeting latitude within -90 to 90 and a finite meeting longitude within -180 to 180.
- [x] **HIKE-REC-008**: If a hike record is requested for a location that has none, then the system shall respond 404 rather than an empty record.
- [x] **HIKE-REC-009**: When a hike is unscheduled, the system shall delete only the hike record and shall leave that location's trail map in place.
- [x] **HIKE-REC-010**: When a hike is unscheduled for a location that has no record, the system shall respond 204, so repeating the request is safe.

## Summary Listing

- [x] **HIKE-LIST-001**: The system shall return one summary row per known location, whether or not a hike is scheduled there.
- [x] **HIKE-LIST-002**: When assembling the summary list, the system shall use a single prefix listing of `hikes/` to determine which locations have a record and which have a trail map, fetching a record only where its key is present.
- [x] **HIKE-LIST-003**: Where a location has a scheduled hike, the summary row shall carry that record's first trail name, so the list can render its blaze colour without a further request per row.
- [x] **HIKE-LIST-004**: If a stored hike record is not valid JSON, then the system shall fail the whole summary listing with 502 rather than report that location as unscheduled.
- [x] **HIKE-LIST-005**: Where a location has a trail map but no hike record, the system shall report it as unscheduled with a map, not as a scheduled hike.

## Staleness

- [x] **HIKE-STALE-001**: While a location has a scheduled hike whose `end` is earlier than the current instant, the system shall mark that summary row stale.
- [x] **HIKE-STALE-002**: If a hike record's `end` cannot be parsed as RFC 3339, then the system shall treat that record as stale.
- [x] **HIKE-STALE-003**: While a location has no scheduled hike, the system shall not mark it stale.
- [x] **HIKE-STALE-004**: While a summary row is stale, the admin page shall show it in the alert colour and state that the date has already passed and needs replacing.

## Authoring Surface

- [x] **HIKE-UI-001**: The admin page shall show the earliest scheduled, non-stale hike as the page hero, with a countdown reading "today", "tomorrow", or a number of days.
- [x] **HIKE-UI-002**: While no scheduled, non-stale hike exists, the admin page shall show an empty hero directing the admin to add a location or set a date, according to whether any location exists.
- [x] **HIKE-UI-003**: When the admin opens a location that has no scheduled hike, the admin page shall default the form to the next Saturday, 09:00 to 11:00.
- [x] **HIKE-UI-004**: When the admin saves a hike, the admin page shall build `start` and `end` as RFC 3339 using the browser's UTC offset **for the chosen date**, so a summer and a winter hike carry different offsets without a server-side timezone table.
- [x] **HIKE-UI-005**: When displaying a stored timestamp, the admin page shall read its wall-clock date and time from the string literally rather than converting the instant into the viewer's timezone.
- [x] **HIKE-UI-006**: When a hike is saved successfully, the admin page shall briefly highlight that location's row, except where the viewer has requested reduced motion.
