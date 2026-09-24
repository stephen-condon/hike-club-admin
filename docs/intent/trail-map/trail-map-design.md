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
timestamp query parameter (`index.html` `openHike`) so a replaced map is displayed
rather than the browser's cached copy.

## Removal

`DELETE /api/map/{slug}` removes a location's map and leaves its hike record
untouched, mirroring how `delete_hike` removes the record and leaves the map.
It is idempotent: deleting a map that is not there is a 204.

Like `delete_hike`, it checks slug **shape only** and not membership. Deleting
creates nothing, so the pattern is the whole defence, and the case worth serving
is precisely the one membership would block — clearing a map stranded by a
location that has already been removed from the mapping. This is what
TRUST-009 already allows for read and delete paths.

The scheduling sheet offers *Remove map* whenever the location has one, and
confirms first. Unscheduling is recoverable — the record can be re-entered from
what the admin remembers — but a deleted map means finding the image file again,
so it is the one destructive action in the tool that asks.

## Decisions & Alternatives

| Decision | Chosen | Alternatives Considered | Rationale |
|---|---|---|---|
| Map ownership | One map per location, shared by every record | One map per hike record; a map per record with fallback | The trails don't change between hikes. Rescheduling would otherwise need a re-upload every time. |
| Upload coupling | Separate endpoint from scheduling | Multipart form carrying both; embed the image in the record | Keeps the record small and lets a map be replaced without rewriting the hike. The UI still presents both in one sheet. |
| Rejected type | 415, distinct from 413 and 400 | A single 400 for every bad upload | Correct HTTP, and the sizes and types fail for genuinely different reasons. The distinction is carried to the admin by the message text, not by any branch in the page. |
| Size ceiling | 5 MB | 2 MB; no limit | Roughly 5x the largest existing map, so it will not bite in practice, while still bounding a worker request. |
| Content-type parsing | Split on `;`, compare case-insensitively | Exact string match | Browsers append parameters; an exact match would reject legitimate uploads. |
| Delete behavior | A DELETE route, separate from the record's | Delete alongside the record; no delete at all | Symmetric with `delete_hike`, which already deletes a record without touching the map. Without it, a map stranded by a removed location could never be cleared. |
| Delete gating | Slug shape only, not membership | Require a known location; require an *unknown* one | Deleting creates nothing, so shape is the whole defence — the same rule `delete_hike` follows. Requiring membership would block the stranded-map case the route exists for. |
| Confirmation | Confirm before removing a map | Remove immediately, like unscheduling | Unscheduling is re-enterable from memory; a deleted map means finding the file again. The asymmetry is deliberate. |

## Open Questions & Future Decisions

### Resolved
1. ✅ Replacing a map leaves the hike record untouched and vice versa
   (`admin.rs:448-460` asserts both directions).
2. ✅ The upload statuses stay distinct, and the admin page does not branch on
   them. 415, 413 and 400 exist so the response names the actual reason; the
   page surfaces whichever message comes back (`index.html` `api`). The only
   status the page distinguishes is 204.

3. ✅ A trail map can now be deleted (MAP-012 to MAP-015), including one
   stranded by a removed location. The record is untouched, and the sheet
   confirms first.

### Deferred

_None._

## References

- `docs/intent/hike-record/` — the record whose `mapKey` points here
- `docs/intent/trust-boundary/` — why the key is server-derived
- `docs/intent/location-mapping/` — the allowlist an upload is checked against
