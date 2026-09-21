---
parent: high-level-design
prefix: LOC
---

# Location Mapping

## Context and Design Philosophy

`resources/hike-locations.json` lists the preserves the club hikes: a short name
used in URLs and R2 keys, and a display name shown in the app. The public API
reads this same object from this same key and serves it as `GET /hike-locations`;
it holds no copy of its own.

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
The bucket genuinely starts without it, and saving the first location is what
creates it. The consequence is that a fresh bucket rejects every hike write with
"unknown location" rather than 500 — which is correct, if initially confusing:
the admin must add a location first. Until then the public API answers
`GET /hike-locations` with `500 server misconfigured: location list not found`
(`api:API-LOC-005`), so the first save here is also what brings the app's
location picker online.

A mapping that *exists* but is not valid JSON is a different situation and
surfaces as 502.

## Replacement, Not Mutation

There is no endpoint to add or remove a single location. The editor loads the
list, the admin adds or removes rows, and `PUT /api/locations` replaces the
whole thing.

Removing a location does not delete its hike record or trail map. Those are
separate objects under a different prefix; orphaning them is recoverable and
deleting them is not. Re-adding the location brings the hike back intact.

## Stranded Objects

Because removal orphans rather than deletes, the bucket accumulates objects with
no location. `GET /api/orphans` reports them: every slug under `hikes/` that has
a record or a map but no entry in the mapping, sorted, with a flag for each
object present. The admin page lists them read-only under "Left behind".

There is deliberately no endpoint that deletes an orphan. Recovery is re-adding
the location, which restores the hike intact; a delete would remove the thing
that makes removal safe, and it would need a write path whose key is *not* in
the allowlist — the inverse of every other write this worker performs.

## Renaming

`short_name` is the URL slug and the R2 key component, so changing it on an
existing location would leave that location's record and map under the old key
and present the location as unscheduled. It would also break every
`/hike/{slug}` link and printed QR code, which makes a rename semantically a new
location rather than an edit.

The editor therefore locks `short_name` on existing rows and leaves it editable
on new ones. Renaming is remove-and-re-add, performed deliberately, with the old
objects visible under "Left behind" until the admin decides what to do.

The API stays permissive: `PUT /api/locations` still accepts any valid list,
including one that renames an entry. Enforcing immutability server-side would
mean diffing the submitted list against the stored one — real logic for an
invariant the editor already prevents.

## Decisions & Alternatives

| Decision | Chosen | Alternatives Considered | Rationale |
|---|---|---|---|
| Allowlist source | The location mapping itself | A separate allowlist object; a hardcoded list; no allowlist | One list cannot disagree with itself. A separate allowlist would be a second thing to keep in step, and the failure mode of drift is an unwritable or over-writable key. |
| Absent object | Reads as an empty list | 404; 500; create it lazily | The bucket's real starting state, and the admin's editor is how it stops being absent. An error here would block the one action that fixes it. |
| Update granularity | Replace the whole list | Per-entry POST/DELETE; JSON Patch | A handful of preserves edited by one person. Whole-list replacement needs no merge semantics. |
| Removing a location | Leaves its record and map in place | Cascade-delete both objects | Orphaning is recoverable; deleting is not. A mis-click should not destroy a hike. |
| Duplicate detection | Reject the whole list | Keep the first; keep the last | A duplicate short name means two display names competing for one R2 key. There is no safe automatic answer. |
| Entry cap | 200 | No cap; a smaller cap | Far beyond a club's real roster, while still bounding the object the public API must parse. |
| Orphan visibility | List them read-only | Leave them invisible; add a delete endpoint | The admin could not previously tell a hike was recoverable. A delete would need a write path outside the allowlist and would remove the undo that orphaning provides. |
| Renaming a location | Lock `short_name` on existing rows | Allow it; implement rename as a copy-and-delete move | A rename already breaks `/hike/{slug}` links and QR codes, so it is a new location. A move would add partially-failing multi-object logic and still break those links. |
| Rename enforcement | Editor only, API stays permissive | Reject renames server-side | Server-side would require diffing against stored state. The editor is the only writer, and the API contract stays a simple whole-list replacement. |

## Open Questions & Future Decisions

### Resolved
1. ✅ Adding a location is the act that makes it schedulable
   (`admin.rs:545-565` asserts it).
2. ✅ Removing a location orphans rather than deletes
   (`admin.rs:587-600` asserts it).

3. ✅ Orphaned objects are now listed by `GET /api/orphans` and shown read-only
   under "Left behind" (LOC-013, LOC-014). Nothing deletes them: re-adding the
   location is the recovery.
4. ✅ Renaming is prevented in the editor rather than handled, because a rename
   already breaks existing links (LOC-015).

### Deferred
1. Whole-list replacement is last-writer-wins. One admin behind Access, so two
   concurrent editors cannot currently exist. If a second admin ever appears,
   the answer is R2 conditional writes — carry the `ETag` from the GET and send
   `If-Match` on the PUT, surfacing 412 as a conflict.

## References

- `docs/intent/trust-boundary/` — the slug rules each `short_name` must satisfy
- `docs/intent/hike-record/` — what becomes writable once a location exists
- `docs/intent/trail-map/` — the other object gated by this allowlist
- `../hike-club-api` — reads this same key and serves it; `500` when absent, `502` when unreadable (`api:API-LOC-005`, `-006`)
