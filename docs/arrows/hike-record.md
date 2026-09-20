# Arrow: hike-record

Custody of `hikes/{slug}.json` — the record that says when and where the next
hike is, and everything that reads or writes it.

## Status

**MAPPED** — last audited 2026-09-19 (git SHA `c911233`). Reverse-engineered
from existing code; specs reflect observed behavior, design rationale is
partly `[inferred]` and awaits confirmation.

## References

### HLD
- `docs/high-level-design.md` — Problem, System Design

### LLD
- `docs/intent/hike-record/hike-record-design.md`

### EARS
- `docs/intent/hike-record/hike-record-specs.md` (25 specs)

### Tests
- `src/admin.rs:272-414` — record round-trip, overwrite, delete, error paths
- `src/admin.rs:602-672` — summary list, staleness, blaze trail
- `src/validate.rs:320-424` — record construction and field rules
- `src/validate.rs:506-519` — staleness turnover
- `tests/contract.rs:99-142` — stored record and summary schema conformance

### Code
- `src/admin.rs:87-134` — get/put/delete handlers
- `src/admin.rs:203-245` — `list_hikes`, summary assembly
- `src/validate.rs:125-157` — `build_record`
- `src/validate.rs:229-236` — `is_stale`
- `src/models.rs:11-49` — `HikeRecord`, `HikeRequest`
- `src/models.rs:63-86` — `HikeSummary`
- `src/index.html:143-167` — scheduling sheet markup
- `src/index.html:240-314` — hero and list rendering
- `src/index.html:327-408` — open, save, unschedule

## Architecture

**Purpose:** Own the lifecycle of the one JSON object per location that records
when a hike is, and surface the one failure mode that object can silently cause.

**Key Components:**
1. `build_record` (`validate.rs:130`) — turns a validated request plus a path
   slug into the exact bytes `hike-club-api` deserializes.
2. `put_hike` / `get_hike` / `delete_hike` (`admin.rs:87-134`) — single-object
   custody; delete is record-only by design.
3. `list_hikes` (`admin.rs:206`) — the derived read: one prefix listing plus
   record fetches only where a key exists.
4. `is_stale` (`validate.rs:231`) — the comparison the whole tool exists for.
5. Scheduling sheet and hero (`index.html:143-167,240-314`) — authoring and
   triage surface.

## Spec Coverage

| Category | Spec IDs | Implemented | Deferred | Gaps |
|----------|----------|-------------|----------|------|
| Record lifecycle | HIKE-REC-001 to -010 | 10 | 0 | 0 |
| Summary listing | HIKE-LIST-001 to -005 | 5 | 0 | 0 |
| Staleness | HIKE-STALE-001 to -004 | 4 | 0 | 0 |
| Authoring UI | HIKE-UI-001 to -006 | 6 | 0 | 0 |

**Summary:** 25 of 25 active specs implemented; 0 deferred.

## Key Findings

1. **Ids carry no date, by design** — `HikeRecord::key` (`models.rs:29`) is
   `hikes/{slug}.json`, so rescheduling overwrites in place and existing
   `/hike/{slug}` links keep working. The cost is that `start`/`end` are the
   only record of when a hike is, which is what makes staleness possible at all.
2. **Staleness is the reason this segment exists** — a record left with a past
   `end` makes the public API serve the previous hike's observed weather as a
   forecast, silently (`admin.rs:630-643` names this in a test docstring).
3. **`[inferred]` — the next-Saturday default** (`index.html:334-336`) and the
   09:00–11:00 default (`:337-338`) encode a club cadence nowhere stated in the
   repo. Confirm or refute.
4. **`[inferred]` — blaze colour from the first trail** (`index.html:186-191`,
   `models.rs:77-81`). The comment explains the convention; whether *first*
   trail (rather than, say, longest or a named primary) is intentional is not
   stated.
5. **View state on the model** — `hike.element` is stashed on the summary object
   (`index.html:312`) so the save path can flash the row (`:393`). Works; couples
   row DOM to fetched data.
6. **`const status` shadows `window.status`** (`index.html:194`). Harmless today.
7. **Seven specs have no test citing them** — HIKE-UI-001 through -006 and
   HIKE-STALE-004 all describe admin-page behavior, and the project has no
   JavaScript test harness. They are annotated in `src/index.html` and marked
   `[x]` on observed behavior, not on test coverage.

## Work Required

### Must Fix
_None._

### Should Fix
1. Confirm or refute findings 3 and 4 so the `[inferred]` markers can come off
   the Decisions table (HIKE-UI-003, HIKE-LIST-003).

### Nice to Have
2. Decide whether the hero should ever surface a stale hike rather than falling
   through to the empty state (HIKE-UI-001, HIKE-UI-002).
3. Rename `status` in `index.html` to avoid shadowing `window.status`.
