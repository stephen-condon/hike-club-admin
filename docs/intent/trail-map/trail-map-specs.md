# Trail Map — Specs

EARS specs for `hikes/{slug}/map.png`. Prefix `MAP`; see `trail-map-design.md`.

## Upload Rules

- [x] **MAP-001**: The system shall store at most one trail map per location, at the R2 key `hikes/{slug}/map.png`, shared by every hike record for that location.
- [x] **MAP-002**: When a trail map is uploaded, the system shall require a content type of `image/png`, ignoring any parameters after a semicolon and ignoring case.
- [x] **MAP-003**: If a trail map upload declares a content type other than `image/png`, or declares none, then the system shall reject it with 415.
- [x] **MAP-004**: If a trail map upload has an empty body, then the system shall reject it with 400.
- [x] **MAP-005**: If a trail map upload exceeds 5 MB, then the system shall reject it with 413.
- [x] **MAP-006**: When a trail map is uploaded for a slug that does not name a known location, the system shall reject it with 400 and write nothing.
- [x] **MAP-007**: When a trail map is stored, the system shall record its content type as `image/png` and respond 204.

## Serving and Presentation

- [x] **MAP-008**: When a trail map is requested for a location that has one, the system shall return the stored bytes unaltered with content type `image/png`.
- [x] **MAP-009**: If a trail map is requested for a location that has none, then the system shall respond 404.
- [x] **MAP-010**: When a trail map is replaced, the system shall leave that location's hike record unchanged; and when a hike record is written or deleted, the system shall leave that location's trail map unchanged.
- [x] **MAP-011**: When the admin page displays a trail map preview, it shall append a cache-busting query parameter so a replaced map is shown rather than a cached copy.

## Removal

- [x] **MAP-012**: When a trail map is deleted, the system shall remove only that location's map and shall leave its hike record in place.
- [x] **MAP-013**: When a trail map is deleted for a location that has none, the system shall respond 204, so a repeated delete is safe.
- [x] **MAP-014**: When deleting a trail map, the system shall require the slug to be well-formed but shall not require it to name a known location, so a map stranded by a removed location can still be cleared.
- [x] **MAP-015**: While a location has a trail map, the scheduling sheet shall offer to remove it, and shall confirm before doing so.
