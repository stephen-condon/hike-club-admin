---
parent: high-level-design
prefix: MAP
---

# Trail Map

## Context and Design Philosophy

A trail map belongs to a *place*, not to an occasion. The same preserve has the
same trails whether the hike is in June or December, so the image lives at
`hikes/{slug}/map.png` — one per location — and every record for that location
points at it through the `mapKey` the server derives.

That separation is the whole design. Uploading a map does not touch the hike
record; scheduling or unscheduling a hike does not touch the map. Two objects,
two independent writes, and rescheduling never asks the admin to find the PNG
again.

## Upload Rules

| Condition | Status | Message |
|---|---|---|
| Content type is not `image/png` (or absent) | 415 | trail maps must be image/png |
| Body is empty | 400 | trail map is empty |
| Body exceeds 5 MB | 413 | trail map must be at most 5 MB |
| Slug is not a known location | 400 | unknown location '…' |

The type check compares only the part before any `;`, case-insensitively,
because browsers append parameters such as `charset=binary`.

The 5 MB ceiling is headroom, not a guess: the club's existing maps run
280 KB to 1.1 MB (`validate.rs:19`).

Validation runs before the store is touched, so a rejected upload writes
nothing — asserted at `admin.rs:468-474,482-505`.

## Serving

`GET /api/map/{slug}` returns the stored bytes unaltered with
`content-type: image/png`, or 404 when the location has no map. The admin page
shows the preview only when the summary row says `hasMap`, and appends a
timestamp query parameter (`index.html:342`) so a replaced map is displayed
rather than the browser's cached copy.

## Decisions & Alternatives

| Decision | Chosen | Alternatives Considered | Rationale |
|---|---|---|---|
| Map ownership | One map per location, shared by every record | One map per hike record; a map per record with fallback | The trails don't change between hikes. Rescheduling would otherwise need a re-upload every time. |
| Upload coupling | Separate endpoint from scheduling | Multipart form carrying both; embed the image in the record | Keeps the record small and lets a map be replaced without rewriting the hike. The UI still presents both in one sheet. |
| Rejected type | 415, distinct from 413 and 400 | A single 400 for every bad upload | Correct HTTP, and the sizes and types fail for genuinely different reasons. See Open Questions 1. |
| Size ceiling | 5 MB | 2 MB; no limit | Roughly 5x the largest existing map, so it will not bite in practice, while still bounding a worker request. |
| Content-type parsing | Split on `;`, compare case-insensitively | Exact string match | Browsers append parameters; an exact match would reject legitimate uploads. |
| Delete behavior | No endpoint deletes a map | A DELETE route; delete alongside the record | Orphaning is recoverable, deleting is not; a map is replaced by uploading another. |

## Open Questions & Future Decisions

### Resolved
1. ✅ Replacing a map leaves the hike record untouched and vice versa
   (`admin.rs:448-460` asserts both directions).

### Deferred
1. `validate.rs:195-196` says 415 and 413 are "the two cases the UI reports
   differently," but the page renders every error's `error` text through one
   path (`index.html:203`). Either the UI should branch, or the comment should
   stop claiming it does.
2. No route deletes a trail map. A location removed from the mapping leaves its
   map in the bucket indefinitely.

## References

- `docs/intent/hike-record/` — the record whose `mapKey` points here
- `docs/intent/trust-boundary/` — why the key is server-derived
- `docs/intent/location-mapping/` — the allowlist an upload is checked against
