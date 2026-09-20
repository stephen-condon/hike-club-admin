# Arrow: trail-map

Custody of `hikes/{slug}/map.png` — one trail map per location, shared by every
hike record for that location.

## Status

**MAPPED** — last audited 2026-09-20 (git SHA `f1d3319`). Reverse-engineered
from existing code; behavior fully observed, no gaps found. One inferred
behavior confirmed since mapping: the upload statuses are not branched on.

## References

### HLD
- `docs/high-level-design.md` — System Design

### LLD
- `docs/intent/trail-map/trail-map-design.md`

### EARS
- `docs/intent/trail-map/trail-map-specs.md` (11 specs)

### Tests
- `src/admin.rs:416-517` — round-trip, key, independence from the record, 415/413/502
- `src/validate.rs:472-504` — upload type and size rules

### Code
- `src/admin.rs:136-173` — `get_map`, `put_map`
- `src/validate.rs:195-227` — `validate_map_upload`
- `src/models.rs:33-38` — `map_key_for`
- `src/index.html:158` — file input and preview element
- `src/index.html:340-342,381-388` — preview load, upload on save

## Architecture

**Purpose:** Hold the one image a location's hikers navigate by, decoupled from
when the hike is.

**Key Components:**
1. `validate_map_upload` (`validate.rs:197`) — type and size gate; the only
   place in the codebase that returns 415 or 413.
2. `put_map` / `get_map` (`admin.rs:136-173`) — custody of the image object.
3. Preview and file input (`index.html:158,340-342`) — cache-busted so a
   replacement is visible immediately.

## Spec Coverage

| Category | Spec IDs | Implemented | Deferred | Gaps |
|----------|----------|-------------|----------|------|
| Upload rules | MAP-001 to -007 | 7 | 0 | 0 |
| Serving and UI | MAP-008 to -011 | 4 | 0 | 0 |

**Summary:** 11 of 11 active specs implemented; 0 deferred.

## Key Findings

1. **The map outlives the record** — `delete_hike` removes only the JSON
   (`admin.rs:124-134`), so rescheduling the same preserve needs no re-upload.
   Asserted at `admin.rs:448-460`.
2. **Distinct status codes are deliberate, and carried by message text** —
   415 for the wrong type, 413 for too large, 400 for empty, so the response
   names the actual reason. The admin page does not branch on them: every
   error reaches the admin as `body.error` through one path
   (`index.html:199-205`). The only status the page distinguishes is 204.
3. **5 MB ceiling is sized from real data** — `validate.rs:19` records that
   existing maps run 280 KB to 1.1 MB.
4. **Content-type parameters tolerated** — `image/PNG; charset=binary` passes
   (`validate.rs:198-207`), because browsers append them.

## Work Required

### Must Fix
_None._

### Should Fix
_None._

### Nice to Have
1. No route deletes a trail map; a location removed from the mapping leaves its
   map in the bucket indefinitely.
