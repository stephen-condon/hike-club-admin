# Arrow: location-mapping

Custody of `resources/hike-locations.json` — the list of preserves the club
hikes, which doubles as the allowlist of R2 keys this worker may write.

## Status

**AUDITED** — last audited 2026-09-20 (git SHA `2a345f6`). Specs verified
against code. Two of three deferred questions closed by building: stranded
objects are now listed, and renaming is prevented rather than silently
orphaning.

## References

### HLD
- `docs/high-level-design.md` — Approach, System Design

### LLD
- `docs/intent/location-mapping/location-mapping-design.md`

### EARS
- `docs/intent/location-mapping/location-mapping-specs.md` (15 specs)

### Tests
- `src/admin.rs:529-600` — read, replace, allowlist effect, orphaning
- `src/admin.rs:695-760` — orphan detection, both object kinds, sorting
- `src/admin.rs:674-687` — 502 paths
- `src/validate.rs:426-470` — list validation rules

### Code
- `src/admin.rs:65-75` — `read_locations`, absent-reads-as-empty
- `src/admin.rs:175-201` — `get_locations`, `put_locations`
- `src/validate.rs:159-193` — `validate_locations`
- `src/models.rs:51-61` — `HikeLocation`, `LOCATIONS_KEY`
- `src/admin.rs:247-292` — `list_orphans`
- `src/index.html` `locationRow`, `openLocations`, `saveLocations` — locations
  editor, `short_name` locked on existing rows
- `src/index.html` `renderOrphans` — the "Left behind" section

## Architecture

**Purpose:** Decide which preserves exist. Because a write is only permitted to
a slug in this list, the mapping is simultaneously a UI convenience and the
security allowlist.

**Key Components:**
1. `validate_locations` (`validate.rs:161`) — shape, size, and uniqueness.
2. `read_locations` (`admin.rs:65`) — the absent-means-empty rule every other
   handler depends on.
3. `put_locations` (`admin.rs:185`) — whole-list replacement.
4. Locations editor (`index.html` `locationRow`, `saveLocations`) — add/remove
   rows, save the list.

## Spec Coverage

| Category | Spec IDs | Implemented | Deferred | Gaps |
|----------|----------|-------------|----------|------|
| Storage and reading | LOC-001 to -003 | 3 | 0 | 0 |
| Validation | LOC-004 to -008 | 5 | 0 | 0 |
| Consequences and UI | LOC-009 to -012 | 4 | 0 | 0 |
| Stranded objects | LOC-013 to -015 | 3 | 0 | 0 |

**Summary:** 15 of 15 active specs implemented; 0 deferred.

## Key Findings

1. **This file is the allowlist** — `validate_known_slug` (`validate.rs:73`)
   consults it on every record and map write, so adding a location is the act
   that makes it writable. `admin.rs:545-565` asserts exactly that.
2. **Absent reads as empty, deliberately** — the bucket starts without the
   object until this worker writes one, and `hike-club-api` answers `500` for
   the list until then (`admin.rs:67-72`). An empty mapping therefore means "nothing is
   schedulable yet," not an error.
3. **Removal orphans rather than deletes** — dropping a location leaves its
   record and map in the bucket, so re-adding it restores the hike intact
   (`admin.rs:587-600`). Those objects are now visible via `GET /api/orphans`;
   nothing deletes them, because the orphan *is* the undo.
4. **Whole-list replacement** — there is no per-location add or remove endpoint;
   the editor sends the entire list every time.
5. **Three specs have no test citing them** — LOC-012, LOC-014 and LOC-015 are
   all admin-page behavior, and the project has no JavaScript test harness.
   LOC-013, the server half of orphan detection, has six.

## Work Required

### Must Fix
_None._

### Should Fix
_None._

### Nice to Have
1. Whole-list replacement has a last-writer-wins race. Single-user today, so it
   has never mattered; the recorded answer is R2 conditional writes if a second
   admin ever appears.
