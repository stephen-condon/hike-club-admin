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
(`index.html:199-205`) — and the only status it distinguishes is 204. So the
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

### Deferred
1. This node holds two purposes — the storage seam and HTTP translation. They
   were kept together during mapping because `lib.rs` is 91 lines of glue. If it
   grows, split `transport` into its own leaf.
2. The `wrangler dev --remote` verification that stands in for coverage on
   `lib.rs` and `r2_store.rs` is manual, unscripted, and leaves no record.
3. `R2Store::list` pages defensively but nothing tests the multi-page path,
   since the fake returns everything at once.

## References

- `docs/intent/contract/` — the router's routes are held equal to `openapi.yaml`
- `docs/intent/hike-record/` — the main consumer of `list`
- `docs/high-level-design.md` — the `AdminStore` test-seam decision
