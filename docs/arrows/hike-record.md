# Arrow: hike-record

Custody of `hikes/{slug}.json` — the record that says where and on which
trails the next hike at a location is, and everything that reads or writes it.

## Status

**AUDITED** — last audited 2026-09-22 (feat/undated-records). `start`/`end`
were dropped from the record: hike-club-app now holds each hike's date and
sends it as query parameters when it fetches trail info, so nothing in this
repo reads a date and neither the record, the summary list, nor the admin
page's hero (removed) needs one.

## References

### HLD
- `docs/high-level-design.md` — Problem, System Design, Non-Goals

### LLD
- `docs/intent/hike-record/hike-record-design.md`

### EARS
- `docs/intent/hike-record/hike-record-specs.md` (16 specs)

### Tests
- `src/admin.rs:341-397` — record round-trip, legacy-record load, overwrite
- `src/admin.rs:712-791` — summary list, blaze trail, map presence
- `src/validate.rs:244-343` — record construction and field rules
- `tests/contract.rs:104-146` — stored record and summary schema conformance

### Code
- `src/admin.rs:94-144` — get/put/delete handlers
- `src/admin.rs:234-267` — `list_hikes`, summary assembly
- `src/validate.rs:131-150` — `build_record`
- `src/models.rs:11-42` — `HikeRecord`, `HikeRequest`
- `src/models.rs:71-79` — `HikeSummary`
- `src/index.html:141-158` — scheduling sheet markup
- `src/index.html:209-241` — list rendering
- `src/index.html:285-370` — open, save, unschedule

## Architecture

**Purpose:** Own the lifecycle of the one JSON object per location that
records where a hike meets and which trails it walks.

**Key Components:**
1. `build_record` (`validate.rs:131`) — turns a validated request plus a path
   slug into the exact bytes `hike-club-api` deserializes.
2. `put_hike` / `get_hike` / `delete_hike` (`admin.rs:94-144`) — single-object
   custody; delete is record-only by design.
3. `list_hikes` (`admin.rs:234`) — the derived read: one prefix listing plus
   record fetches only where a key exists.
4. Scheduling sheet and list (`index.html:141-158,209-241`) — authoring
   surface.

## Spec Coverage

| Category | Spec IDs | Implemented | Deferred | Gaps |
|----------|----------|-------------|----------|------|
| Record lifecycle | HIKE-REC-001 to -011 | 10 | 0 | 0 |
| Summary listing | HIKE-LIST-001 to -005 | 5 | 0 | 0 |
| Authoring UI | HIKE-UI-006 | 1 | 0 | 0 |

**Summary:** 16 of 16 active specs implemented; 0 deferred.

## Key Findings

1. **Ids carry no date, by design** — `HikeRecord::key` (`models.rs:29`) is
   `hikes/{slug}.json`, so rescheduling overwrites in place and existing
   `/hike/{slug}` links keep working.
2. **The record now carries no date at all** — hike-club-app holds each
   hike's date and sends it as query parameters when it fetches trail info
   from the public API (api:API-WIN-*), so a date on the admin's record would
   have been write-only. `HikeRecord::deny_unknown_fields` is off, so a
   record written before this change still loads; the next save drops the
   leftover fields (`admin.rs:355-378`).
3. **The hero and staleness UI are gone, not adapted** — both existed only to
   surface a stale `end`. With no stored date, there is nothing to compare
   against `now`, so the segment lost them rather than finding a replacement.
4. **View state is off the model** — row elements live in a slug-to-element
   `Map` (`index.html`), not on the fetched summary objects.
5. **One spec has no test citing it** — HIKE-UI-006 describes admin-page
   behavior, and the project has no JavaScript test harness. It is annotated
   in `src/index.html` and marked `[x]` on observed behavior, not on test
   coverage.

## Work Required

### Must Fix
_None._

### Should Fix
_None._

### Nice to Have
1. HIKE-UI-006 stays untested until the project grows a JavaScript test
   harness — a real trade against a codebase with no bundler.
