---
parent: high-level-design
prefix: HIKE
---

# Hike Record

## Context and Design Philosophy

One location, one record. The object at `hikes/{slug}.json` is the whole answer
to "when is the next hike at this preserve," and it is the object the public API
reads. Rescheduling replaces it rather than appending to it, so there is no
history and no hike id that outlives a date change.

That choice buys stable links — `/hike/{slug}` and any printed QR code keep
working across a reschedule — and it costs the ability to tell a current record
from an abandoned one by inspection. `start` and `end` are the only record of
when a hike is. A record left with a past `end` does not fail; the public API
keeps serving it, and the weather it serves is the *last* hike's observed
weather presented as a forecast. Surfacing that is this segment's main job, not
an incidental feature.

## Record Shape

The stored object is the cross-repo contract — `hike-club-api/src/models.rs`
deserializes exactly these bytes. Field names and `serde` renames are fixed by
`openapi.yaml`'s `HikeRecord` schema and asserted in `tests/contract.rs:105-111`.

| Field | Type | Source |
|---|---|---|
| `id` | string | the path slug, server-derived |
| `start` | string | request body, RFC 3339 with the hike's local offset |
| `end` | string | request body, RFC 3339 with the hike's local offset |
| `meeting` | `{lat, lon}` | request body, range-checked |
| `trails` | string[] | request body, trimmed |
| `mapKey` | string | `hikes/{slug}/map.png`, server-derived |

`HikeRequest` deliberately has no `id` and no `mapKey` field. There is nowhere
in the request type to put one, which is what stops a write being aimed at an
arbitrary object. See `docs/intent/trust-boundary/`.

## Timestamps and the Local Offset

Stored timestamps carry the hike's own UTC offset for the date it falls on. The
browser builds them from its own offset for the chosen date
(`index.html:217-223`), so a hike in June gets `-05:00` and one in December gets
`-06:00` without any timezone table on the server. Reading them back takes the
wall-clock substring literally (`index.html:212-215`) rather than parsing into
a `Date` and reformatting, so the displayed time is the time the admin typed.

This holds as long as the admin schedules from the club's own timezone. An admin
travelling abroad would author offsets for wherever they are sitting.

Comparisons, by contrast, are instant-based: `end` must be after `start` as a
parsed instant, not as a string (`validate.rs:137-141`), and staleness compares
instants against `now`.

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

## Staleness

`is_stale` is a single comparison: the record's `end` against now. An `end` that
cannot be parsed counts as stale — something is wrong with it either way. Only a
scheduled hike can be stale; an empty slot is just empty.

The flag travels on `HikeSummary.stale` and the list renders it as *"already
past, needs a new date"* in the alert colour.

The hero escalates it. It shows the earliest scheduled non-stale hike when one
exists; when none does but a stale record remains, it shows the most recently
ended stale hike in the alert treatment — *"ended 7 days ago — still being
served as the next hike"* — rather than falling through to the empty state. A
stale record is not an absence of a hike; it is the public API actively serving
last hike's observed weather as a forecast, and the most prominent element on
the page is where that belongs. The empty state is reserved for a club with no
scheduled hike at all.

## Authoring Surface

```
┌────────────────────────────────────────────┐
│ Cantigny                                   │  ← location full_name
├────────────────────────────────────────────┤
│ Date          │ Start      │ End           │
│ [2026-09-26]  │ [09:00]    │ [11:00]       │
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

Opening an unscheduled location pre-fills the next Saturday, 09:00–11:00. Saving
issues `PUT /api/hikes/{slug}` and then, only if a file was chosen,
`PUT /api/map/{slug}` — two independent objects, two independent writes.

## Decisions & Alternatives

| Decision | Chosen | Alternatives Considered | Rationale |
|---|---|---|---|
| Record identity | One record per location, overwritten on reschedule | One record per hike occurrence, dated ids | Stable `/hike/{slug}` links and printed QR codes survive a reschedule; the club runs one hike per preserve at a time. Cost: no history, and staleness becomes possible. |
| Staleness detection | Compare `end` to now at read time | Store an explicit `active` flag; expire records on a schedule | No writer runs between hikes, so a flag would go stale itself. A read-time comparison cannot drift. |
| Unparseable `end` | Treat as stale | Treat as an error; treat as current | Something is wrong with the record either way, and the admin should be told. Failing the row would hide the rest of the list. |
| Timestamp storage | RFC 3339 carrying the hike's local offset | Store UTC and convert for display; store a timezone name | The browser already knows the offset for the chosen date, so CDT/CST needs no server-side table. Assumes the admin browses from the club's timezone. |
| Summary assembly | One prefix listing, then fetch only existing records | A GET per location; store a precomputed index | Two objects per location means one page covers the bucket today. A precomputed index would be a second thing to keep coherent. |
| Corrupt record in a listing | Fail the whole listing with 502 | Skip the row; report it unscheduled | Reporting unscheduled is a lie precisely when the admin needs the truth. |
| Delete semantics | Record only; the trail map stays | Delete both objects | Rescheduling the same location shouldn't need a map re-upload. Orphaning is recoverable, deleting is not. |
| Blaze colour source | First trail name, carried on the summary | Fetch each record to render the list; a per-location colour field | Trails are entered in the order they are walked and the first is the one the hike is named for, so its blaze is the right one. Carrying it on the summary also saves a GET per row. |
| Form defaults | Next Saturday, 09:00–11:00 | No defaults; last-used values; a configured cadence | The club hikes Saturday mornings, so the default is right almost every time. A configurable cadence would add a stored value and a settings surface for something one person changes rarely. |
| Hero when only stale records remain | Show the stale hike in the alert treatment | Fall through to the empty state; show nothing | A stale record means the public API is serving stale weather as a forecast *now*. "No hike scheduled" is true but underplays an active fault; the hero is the most prominent place to state it. |
| Row element tracking | A slug-to-element `Map` | Stash the DOM node on the fetched summary object | Keeps view state off fetched data for the same line count. |

## Open Questions & Future Decisions

### Resolved
1. ✅ Rescheduling overwrites in place rather than creating a new record — stable
   links win over history (`admin.rs:299-314` asserts it).
2. ✅ Unscheduling leaves the trail map behind (`admin.rs:448-460` asserts it).

3. ✅ The hero surfaces a stale hike rather than falling through to the empty
   state (HIKE-UI-007). The empty state now means no scheduled hike at all.
4. ✅ The next-Saturday, 09:00–11:00 default matches the club's cadence and is
   recorded as intent rather than inference.
5. ✅ The first trail is the one the hike is named for, so its blaze is the
   right one; recorded as intent rather than inference.
6. ✅ Row elements are tracked in a slug-to-element `Map`, and the page-level
   note is `statusNote` rather than shadowing `window.status`.

### Deferred

_None._

## References

- `docs/intent/trust-boundary/` — slug rules and server-derived keys
- `docs/intent/trail-map/` — the other object a location owns
- `docs/intent/location-mapping/` — what makes a location schedulable
- `docs/intent/contract/` — `HikeRecord` as the cross-repo byte contract
- `../hike-club-api/src/models.rs` — the reader of these exact bytes
