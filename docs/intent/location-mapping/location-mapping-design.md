---
parent: high-level-design
prefix: LOC
---

# Location Mapping

## Context and Design Philosophy

`resources/hike-locations.json` lists the preserves the club hikes: a short name
used in URLs and R2 keys, and a display name shown in the app. The public API
serves this same object from this same key.

It carries a second job that is easy to miss. Every write to the bucket is
checked against it — a hike record or a trail map may only be written for a slug
that appears here. The convenience list *is* the security allowlist, which means
editing it is the most consequential action in the tool, and why its own
validation is the strictest.

## Shape and Rules

Each entry is `{ "short_name": …, "full_name": … }`.

| Rule | Limit |
|---|---|
| Entries | at most 200 |
| `short_name` | a valid slug (see `docs/intent/trust-boundary/`) |
| `short_name` uniqueness | no duplicates within the list |
| `full_name` | non-blank after trimming, at most 100 characters |

An empty list is valid — it means nothing is selectable yet, which is the
bucket's starting state.

Validation runs over the whole list before anything is written, so a list with
one bad entry leaves the stored mapping untouched
(`admin.rs:567-580` asserts this).

## Absent Means Empty

`read_locations` treats a missing object as an empty list rather than an error.
The bucket genuinely starts without it, and `hike-club-api` falls back to its
own embedded copy until this worker writes one. The consequence is that a fresh
bucket rejects every hike write with "unknown location" rather than 500 — which
is correct, if initially confusing: the admin must add a location first.

A mapping that *exists* but is not valid JSON is a different situation and
surfaces as 502.

## Replacement, Not Mutation

There is no endpoint to add or remove a single location. The editor loads the
list, the admin adds or removes rows, and `PUT /api/locations` replaces the
whole thing.

Removing a location does not delete its hike record or trail map. Those are
separate objects under a different prefix; orphaning them is recoverable and
deleting them is not. Re-adding the location brings the hike back intact.

## Decisions & Alternatives

| Decision | Chosen | Alternatives Considered | Rationale |
|---|---|---|---|
| Allowlist source | The location mapping itself | A separate allowlist object; a hardcoded list; no allowlist | One list cannot disagree with itself. A separate allowlist would be a second thing to keep in step, and the failure mode of drift is an unwritable or over-writable key. |
| Absent object | Reads as an empty list | 404; 500; create it lazily | The bucket's real starting state, and the public API already falls back to its embedded copy. An error would make a fresh deployment look broken. |
| Update granularity | Replace the whole list | Per-entry POST/DELETE; JSON Patch | A handful of preserves edited by one person. Whole-list replacement needs no merge semantics. |
| Removing a location | Leaves its record and map in place | Cascade-delete both objects | Orphaning is recoverable; deleting is not. A mis-click should not destroy a hike. |
| Duplicate detection | Reject the whole list | Keep the first; keep the last | A duplicate short name means two display names competing for one R2 key. There is no safe automatic answer. |
| Entry cap | 200 | No cap; a smaller cap | Far beyond a club's real roster, while still bounding the object the public API must parse. |

## Open Questions & Future Decisions

### Resolved
1. ✅ Adding a location is the act that makes it schedulable
   (`admin.rs:545-565` asserts it).
2. ✅ Removing a location orphans rather than deletes
   (`admin.rs:587-600` asserts it).

### Deferred
1. Orphaned records and maps are invisible: nothing lists keys that no longer
   correspond to a location, and nothing reclaims them.
2. Whole-list replacement is last-writer-wins. With one admin this has never
   mattered; a second concurrent editor would silently lose changes.
3. `short_name` is used both as a URL slug and as an R2 key component. Renaming
   a location therefore orphans its objects rather than moving them.

## References

- `docs/intent/trust-boundary/` — the slug rules each `short_name` must satisfy
- `docs/intent/hike-record/` — what becomes writable once a location exists
- `docs/intent/trail-map/` — the other object gated by this allowlist
- `../hike-club-api` — reads this same key, with an embedded fallback
