---
parent: high-level-design
prefix: STORE
---

# Store and Transport

## Context and Design Philosophy

Two thin layers wrap every decision this worker makes, and both exist so the
decisions themselves can be tested without Cloudflare.

Below the handlers, `AdminStore` is a byte-blob trait shaped deliberately like
R2 — get, put, delete, list over `Vec<u8>` — so the real implementation has no
logic to get wrong and the in-memory fake is a faithful stand-in. Above them,
`Outcome` is a status plus already-serialized bytes, so a handler never
constructs a `worker::Response` and never needs a runtime to produce one.

The result is that `src/lib.rs` and `src/r2_store.rs` are the only files that
cannot run in `cargo test`, and they contain nothing worth testing.

## The Store Seam

```rust
trait AdminStore {
    async fn get(&self, key) -> Result<Option<Vec<u8>>, String>;
    async fn put(&self, key, body, content_type) -> Result<(), String>;
    async fn delete(&self, key) -> Result<(), String>;
    async fn list(&self, prefix) -> Result<Vec<String>, String>;
}
```

`list` exists for one reason: answering "which locations have a hike, and a
map?" with a single prefix listing instead of two GETs per location.

`InMemoryStore` backs it with a `BTreeMap` in a `RefCell` — not a lock, because
the worker runtime is single-threaded and so are the tests. Its `failing()`
constructor makes every operation return an error, which is how every 502 path
in the codebase is exercised.

`R2Store` translates. Its only non-trivial code is the cursor loop in `list`:
R2 pages at 1000 keys, the bucket holds two objects per location, so one page
covers the club today and the loop is insurance rather than a live requirement.

## Failure Vocabulary

| Situation | Status | Reasoning |
|---|---|---|
| R2 operation failed | 502 | This worker proxies storage; the browser can only retry. |
| Stored JSON will not parse | 502 | The upstream object is wrong, not the request. |
| Request body will not parse | 400 | The caller's problem. |
| Own types fail to serialize | 500 | A bug here, surfaced rather than panicked. |

Nothing panics. `Outcome::json` falls back to a fixed error body if
serialization fails, because a panic in a worker loses the request entirely.

Every one of these carries the same body shape: `{"error": "<reason>"}`. That
matters because the status code alone is not what the admin sees. The page has
one error path — any non-2xx becomes `body.error` text in a note line
(`index.html` `api`) — and the only status it distinguishes is 204. So the
reason a request failed reaches the admin as the message this layer wrote, and
a status split that is not matched by a distinct message is invisible to them.

## Transport

`lib.rs` is glue: set the panic hook, build a router of eight routes, look up
the `HIKES` R2 binding per request, call a handler, translate the `Outcome`.

The `:slug` helper returns an empty string when the parameter is absent. That
cannot happen for a matched route, and an empty slug fails validation in the
handler anyway, so the path needs no error branch.

The admin page ships inside the binary through `include_str!` — no bundler, no
static-assets binding, mirroring how `hike-club-api` ships its location mapping.

## Coverage Boundary

`src/lib.rs` and `src/r2_store.rs` are excluded from the 85% line-coverage gate
(`.githooks/pre-push:16-18`, `.github/workflows/ci.yml:65-69`) because they can
only execute inside a deployed or dev worker. They are verified by running
`npx wrangler dev --remote` against the real bucket.

This is the trade the seam buys: everything with a decision in it is held to
85%, and the two files that are excluded contain no decisions.

### Runtime verification procedure

What the excluded files get instead of coverage. Run against the real bucket:

```bash
npx wrangler dev --remote --port 8788
```

Then exercise each path that only exists in the excluded files:

| Check | Exercises | Expect |
|---|---|---|
| `GET /health` | routing, no storage | `ok` |
| `GET /` | `include_str!` delivery | the admin page renders |
| `GET /api/hikes` | `R2Store::list`, cursor loop, `respond` | a row per location |
| `PUT /api/map/{slug}` with a PNG | `R2Store::put`, HTTP metadata | 204, then the preview loads |
| `GET /api/map/{slug}` | `R2Store::get`, byte fidelity | the same PNG |

This cannot run in CI: Cloudflare Access rejects unauthenticated requests
before the Worker runs, so CI would need a service token — the same reason
`release.yml` carries no post-deploy smoke test. STORE-005's cursor loop is
only reachable here, since the in-memory fake is not R2 and returns every key
at once.

### When to split this node

This node holds two purposes, kept together because `src/lib.rs` is glue: a
router, a binding lookup, a slug accessor and a response translation, with no
conditionals and nothing a test would assert. Split `transport` into its own
leaf when that stops being true — when `lib.rs` acquires middleware, request
rewriting, identity handling, or any branch whose outcome a test would check.

## Decisions & Alternatives

| Decision | Chosen | Alternatives Considered | Rationale |
|---|---|---|---|
| Storage abstraction | Byte-blob trait shaped like R2 | A domain-shaped repository trait (`save_hike`, `load_map`); no abstraction | A domain trait would move logic into the untestable implementation. Shaping it like R2 keeps `R2Store` pure translation. |
| Handler return type | `Outcome` (status + bytes) | Return `worker::Response`; return a domain type and serialize in `lib.rs` | Keeps handlers runtime-free and unit-testable, and keeps serialization failures inside the tested layer. |
| Fake's interior mutability | `RefCell` | `Mutex`/`RwLock`; `&mut self` on the trait | The worker runtime is single-threaded and so are the tests. A lock would be ceremony. |
| R2 failure status | 502 | 500; 503 | The worker is a proxy for storage; the failure is upstream of it. |
| Corrupt stored JSON | 502 | 500; skip the object; return empty | The stored object is wrong, not the request. Returning empty would misreport the bucket's state. |
| Serialization failure | Fixed fallback body, status 500 | `unwrap()`; propagate a `Result` everywhere | A panic in a worker loses the request. This cannot happen for owned types anyway. |
| Coverage gate scope | Exclude `lib.rs` and `r2_store.rs` | Gate everything; gate nothing; integration-test the worker | Both files need a live worker. Excluding them keeps the gate honest rather than padded with untestable lines. |
| Listing strategy | One prefix list, then targeted gets | A get per location; maintain an index object | Two objects per location, so one page. An index would be another thing to keep coherent. |
| UI delivery | `include_str!` into the binary | A static-assets binding; a bundler | One file, no build step, and it matches `hike-club-api`'s approach. |

## Open Questions & Future Decisions

### Resolved
1. ✅ Storage failures and corrupt stored JSON both surface as 502 and never as
   a panic (`admin.rs:379-404` asserts both).
2. ✅ `delete` is idempotent (`store.rs:139-146`).

3. ✅ The node keeps both purposes while `lib.rs` stays glue; the condition that
   should trigger a split is recorded above rather than re-argued each time.
4. ✅ The `wrangler dev --remote` check is written down as a procedure. It cannot
   be automated: Access rejects unauthenticated requests before the Worker runs.
5. ✅ `R2Store::list`'s cursor loop is reachable only under a real Bucket, so
   STORE-005 is verified by the runtime procedure rather than by a unit test.

### Deferred

_None._

## References

- `docs/intent/contract/` — the router's routes are held equal to `openapi.yaml`
- `docs/intent/hike-record/` — the main consumer of `list`
- `docs/high-level-design.md` — the `AdminStore` test-seam decision
