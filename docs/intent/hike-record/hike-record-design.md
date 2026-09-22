---
parent: high-level-design
prefix: HIKE
---

# Hike Record

## Context and Design Philosophy

One location, one record. The object at `hikes/{slug}.json` is the whole answer
to "where does the next hike at this preserve meet, and on which trails," and
it is the object the public API reads. Rescheduling replaces it rather than
appending to it, so there is no history and no hike id that outlives a
reschedule.

That choice buys stable links — `/hike/{slug}` and any printed QR code keep
working across a reschedule. The record carries no date: the hike-club-app
holds each hike's date and sends it as query parameters when it fetches trail
info, so this repo has no view of when a hike is and needs none. See Non-Goals
in `docs/high-level-design.md`.

## Record Shape

The stored object is the cross-repo contract — `hike-club-api/src/models.rs`
deserializes exactly these bytes. Field names and `serde` renames are fixed by
`openapi.yaml`'s `HikeRecord` schema and asserted in `tests/contract.rs:105-111`.

| Field | Type | Source |
|---|---|---|
| `id` | string | the path slug, server-derived |
| `meeting` | `{lat, lon}` | request body, range-checked |
| `trails` | string[] | request body, trimmed |
| `mapKey` | string | `hikes/{slug}/map.png`, server-derived |

`HikeRequest` deliberately has no `id` and no `mapKey` field. There is nowhere
in the request type to put one, which is what stops a write being aimed at an
arbitrary object. See `docs/intent/trust-boundary/`.

A record written before this segment dropped `start`/`end` may still carry
them in R2. `HikeRecord` deserializes such a record — it is not
`deny_unknown_fields` — and the next save overwrites it without them.

## Deriving the Summary List

`list_hikes` returns one row per *known location*, not one row per stored record
— an empty slot is a thing the admin needs to see. It issues one prefix listing
of `hikes/` and uses key presence to decide both "is there a record" and "is
there a map," fetching a record only where one exists. A location with a map and
no record is unscheduled-with-map, never a scheduled hike; the two keys
(`hikes/{slug}.json` and `hikes/{slug}/map.png`) share a prefix and must not be
mistaken for each other.

A record that fails to parse fails the whole listing with 502. Reporting the
location as unscheduled would be a lie in exactly the situation the admin most
needs the truth.

## Authoring Surface

```
┌────────────────────────────────────────────┐
│ Cantigny                                   │  ← location full_name
├────────────────────────────────────────────┤
│ Trails                                     │
│ [Purple, Green                           ] │
│ Meeting latitude  │ Meeting longitude      │
│ [41.855026]       │ [-88.152169]           │
│ Trail map                                  │
│ [ preview image, if uploaded             ] │
│ [ Choose file ]                            │
├────────────────────────────────────────────┤
│ [Save hike]  [Cancel]          [Unschedule]│
└────────────────────────────────────────────┘
```

Saving issues `PUT /api/hikes/{slug}` and then, only if a file was chosen,
`PUT /api/map/{slug}` — two independent objects, two independent writes.

## Decisions & Alternatives

| Decision | Chosen | Alternatives Considered | Rationale |
|---|---|---|---|
| Record identity | One record per location, overwritten on reschedule | One record per hike occurrence, dated ids | Stable `/hike/{slug}` links and printed QR codes survive a reschedule; the club runs one hike per preserve at a time. Cost: no history. |
| Hike date/time ownership | The app holds it, sent as query params on each fetch | Keep `start`/`end` on the stored record; store a separate admin-only date | The record's dates were write-only once the app sent its own window — nothing read them. A separate admin-only date would need a UI and mean two independent, driftable statements of "when" for the same hike, for a value nothing here consumes. |
| Summary assembly | One prefix listing, then fetch only existing records | A GET per location; store a precomputed index | Two objects per location means one page covers the bucket today. A precomputed index would be a second thing to keep coherent. |
| Corrupt record in a listing | Fail the whole listing with 502 | Skip the row; report it unscheduled | Reporting unscheduled is a lie precisely when the admin needs the truth. |
| Delete semantics | Record only; the trail map stays | Delete both objects | Rescheduling the same location shouldn't need a map re-upload. Orphaning is recoverable, deleting is not. |
| Blaze colour source | First trail name, carried on the summary | Fetch each record to render the list; a per-location colour field | Trails are entered in the order they are walked and the first is the one the hike is named for, so its blaze is the right one. Carrying it on the summary also saves a GET per row. |
| Row element tracking | A slug-to-element `Map` | Stash the DOM node on the fetched summary object | Keeps view state off fetched data for the same line count. |

## Open Questions & Future Decisions

### Resolved
1. ✅ Rescheduling overwrites in place rather than creating a new record — stable
   links win over history (`admin.rs:299-314` asserts it).
2. ✅ Unscheduling leaves the trail map behind (`admin.rs:448-460` asserts it).
3. ✅ The first trail is the one the hike is named for, so its blaze is the
   right one; recorded as intent rather than inference.
4. ✅ Row elements are tracked in a slug-to-element `Map`, and the page-level
   note is `statusNote` rather than shadowing `window.status`.
5. ✅ The record's `start`/`end` are dropped rather than kept write-only: once
   the app sends its own window with each fetch, nothing in either repo reads
   them, and an unread field is a liability, not a courtesy.

### Deferred

_None._

## References

- `docs/intent/trust-boundary/` — slug rules and server-derived keys
- `docs/intent/trail-map/` — the other object a location owns
- `docs/intent/location-mapping/` — what makes a location schedulable
- `docs/intent/contract/` — `HikeRecord` as the cross-repo byte contract
- `../hike-club-api/src/models.rs` — the reader of these exact bytes
