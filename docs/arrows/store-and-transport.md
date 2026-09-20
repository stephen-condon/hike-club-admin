# Arrow: store-and-transport

How a request becomes a stored byte: the `AdminStore` seam, its R2
implementation, the `Outcome` type, routing, and the failure vocabulary.

## Status

**AUDITED** — last audited 2026-09-20 (git SHA `2fdc7ef`). Specs verified
against code; all three deferred questions resolved. Two files remain excluded
from the coverage gate by design, now with a written runtime procedure in
place of it.

## References

### HLD
- `docs/high-level-design.md` — System Design, Key Design Decisions

### LLD
- `docs/intent/store-and-transport/store-and-transport-design.md`

### EARS
- `docs/intent/store-and-transport/store-and-transport-specs.md` (15 specs)

### Tests
- `src/store.rs:111-167` — round-trip, prefix listing, idempotent delete, failing store
- `src/admin.rs:379-404` — 502 on storage failure and on corrupt JSON
- `src/admin.rs:674-693` — 502 across locations and listing paths

### Code
- `src/store.rs:1-13` — the `AdminStore` trait
- `src/store.rs:16-108` — `InMemoryStore`, the test fake
- `src/r2_store.rs` — the real implementation, including cursor paging
- `src/admin.rs:11-63` — `Outcome`, `upstream`
- `src/lib.rs:26-91` — router, binding lookup, `Outcome` → `Response`

## Architecture

**Purpose:** Keep every decision testable without a Workers runtime, and give
storage failure one honest vocabulary.

**Key Components:**
1. `AdminStore` (`store.rs:6`) — a byte-blob trait shaped like R2 itself, so
   the real implementation is pure translation.
2. `InMemoryStore` (`store.rs:24`) — the fake every handler test runs against,
   with a `failing()` constructor as the seam for 502 paths.
3. `R2Store` (`r2_store.rs:12`) — translation only, plus a cursor loop.
4. `Outcome` (`admin.rs:13`) — status plus already-serialized bytes, so handlers
   never touch `worker::Response`.
5. Router (`lib.rs:30-69`) — eight routes, each a binding lookup and a call.

## Spec Coverage

| Category | Spec IDs | Implemented | Deferred | Gaps |
|----------|----------|-------------|----------|------|
| Storage seam | STORE-001 to -005 | 5 | 0 | 0 |
| Failure vocabulary | STORE-006 to -009, -014, -015 | 6 | 0 | 0 |
| Transport | STORE-010 to -013 | 4 | 0 | 0 |

**Summary:** 15 of 15 active specs implemented; 0 deferred.

## Key Findings

1. **The seam is the reason coverage is meaningful** — because handlers are
   generic over `AdminStore`, `src/lib.rs` and `src/r2_store.rs` are the only
   files that need a worker to run, and both are excluded from the 85% gate
   (`.githooks/pre-push:16-18`, `ci.yml:65-69`). They are verified by
   `wrangler dev --remote` instead.
2. **R2 errors are 502, not 500** — `upstream` (`admin.rs:61`) treats this
   worker as a proxy for storage: the browser can only retry.
3. **Corrupt stored JSON is also 502** — deliberately not a panic and not a 500
   (`admin.rs:73-74,82-84`).
4. **Cursor paging is pre-emptive and untestable in-process** — `r2_store.rs:50-52`
   notes the bucket holds two objects per location, so one page covers it today.
   The loop needs a real `Bucket` to exercise, so STORE-005 is verified by the
   runtime procedure in the LLD rather than by a unit test.
5. **Serialization failure degrades rather than panics** — `Outcome::json` falls
   back to a fixed error body (`admin.rs:27-28`).
6. **An absent `:slug` is impossible for a matched route**, so `lib.rs:81-83`
   falls back to an empty string, which then fails validation downstream.
7. **Eight specs have no test citing them** — STORE-011, -012 and -013 live in
   `src/lib.rs`, excluded from the coverage gate by design; STORE-005's cursor
   loop is unreachable through the fake; STORE-002 and -010 are structural,
   proven by the suite's existence rather than by an assertion; STORE-008's
   fallback cannot be reached, since serializing owned types does not fail;
   STORE-015 is admin-page behavior with no JavaScript harness.

## Work Required

### Must Fix
_None._

### Should Fix
_None._

### Nice to Have
1. The `wrangler dev --remote` procedure is written down but still manual.
   Automating it needs a Cloudflare Access service token; deliberately not
   taken, for a single-user admin tool writing to one real bucket.
