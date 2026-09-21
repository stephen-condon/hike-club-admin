---
parent: high-level-design
prefix: CONTRACT
---

# Contract

## Context and Design Philosophy

A read-only API can treat its spec as documentation. A write API cannot: the
spec describes what will be *accepted*, and if it is more permissive than the
validator it is lying to whoever reads it, while if it is stricter it promises
rejections that never happen.

So `openapi.yaml` here is executable. `tests/contract.rs` holds the code to it
in three directions, and the interesting one is not "do responses match the
schema" — it is "are the spec's limits the same numbers as the validator's."

There is a fourth relationship the file quietly protects: `HikeRecord`'s schema
is the cross-repo contract. Those are the exact bytes
`hike-club-api/src/models.rs` deserializes, so a field rename here breaks a
different repository's deploy.

## The Three Directions

**1. Responses validate against their schemas.** Each type is serialized and
checked against its component schema — `HikeRequest`, `HikeRecord`,
`HikeSummary` in both its scheduled and unscheduled shapes, `HikeLocations`,
`Error`. `HikeRecord` additionally has its exact key *set* asserted
(`contract.rs:105-111`), with a comment naming the downstream reader.

**2. The spec's limits equal the validator's constants.** Not "are compatible
with" — `assert_eq!` against `MAX_SLUG_LEN`, `MAX_TRAILS`, `MAX_NAME_LEN`,
`MAX_LOCATIONS`, and the coordinate bounds. Separately, every body and every
slug that `validate.rs` rejects is asserted to fail the spec too, including
bodies carrying a smuggled `id` or `mapKey`.

**3. The router and the spec describe the same routes.** Both ways: a spec path
with no route is a promise the worker does not keep; a route with no spec entry
is undocumented. Implemented by matching text in `src/lib.rs` and comparing
counts — crude, but the router is a dozen lines and this needs no Workers
runtime to run.

## Mechanics

`jsonschema` wants a self-contained draft-07 document, so `validator_for`
rewrites `#/components/schemas/X` to `#/definitions/X`, nests the component
schemas under `definitions`, and compiles.

OpenAPI 3.0's `nullable: true` is not a JSON Schema keyword, so
`desugar_nullable` (`contract.rs:17`) rewrites it the way OpenAPI tooling does:
`type: X` with `nullable` becomes `anyOf: [{type: "null"}, {type: X, …}]`.

Everything runs in-process. No network, no deployed worker, no spun-up runtime
— which is why it can sit in the pre-push hook.

## Where Strictness Actually Lives

`HikeRequest` and `HikeRecord` both set `additionalProperties: false` in the
spec. The implementation mirrors that on **`HikeRequest` only**, through
`#[serde(deny_unknown_fields)]`: a body carrying a smuggled `id` or `mapKey` is
now refused with a 400 rather than deserialized with the field dropped, so both
sides reject for the same reason and the caller is told.

`HikeRecord` is deliberately left tolerant. It deserializes *stored* data, and
the two boundaries want opposite things: at the input boundary an unexpected
field is a caller error worth reporting, while at the storage boundary it is a
record written by hand or by an older version, and refusing to read it would
turn something recoverable into a 502 with no way to inspect it.

So the asymmetry between spec and implementation is now confined to
`HikeRecord`, and it is a choice rather than an oversight.

## Decisions & Alternatives

| Decision | Chosen | Alternatives Considered | Rationale |
|---|---|---|---|
| Spec's role | Executable contract, asserted against code | Documentation only; generate code from the spec; generate the spec from code | A write API's spec that drifts is worse than none. Generation in either direction would couple the repos' build systems. |
| Limit checking | `assert_eq!` spec numbers to Rust constants | Assert the spec is merely no stricter | Equality catches a change in either direction. "Compatible" would let the spec quietly go stale. |
| Rejection parity | Every body/slug `validate.rs` rejects must fail the spec | Spot-check a few; property-test | Enumerated cases are readable and name the attack they prevent. |
| Route comparison | Textual match against `src/lib.rs` | Parse the router; integration-test every route | Parsing needs a Workers runtime. The router is a dozen lines, so text matching is adequate and free. |
| `nullable` handling | Hand-rolled desugaring | An OpenAPI-aware validator crate; drop `nullable` from the spec | One small function versus a heavier dependency in dev-deps. |
| Cross-repo contract | Assert the exact field key set | Trust the schema; share a crate between repos | A shared crate would couple deploys. The key-set assertion fails here rather than in the public API's deserializer. |
| Unknown request fields | `deny_unknown_fields` on `HikeRequest`; `HikeRecord` left tolerant | Strict on both; strict on neither | An unexpected field is a caller error at the input boundary and a readability hazard at the storage boundary. Strictness on stored data would turn a recoverable record into a 502. |
| `mapKey` pattern drift | Assert a derived key against the spec's pattern | Trust the full-record schema test; generate one from the other | The full-record test exercises the pattern only incidentally, with one slug. Asserting the derivation directly is a few lines and fails on a change to either side. |
| Route-check technique | Match source text in `src/lib.rs` | Parse the router; tolerate multi-line formatting | `cargo fmt` runs in the pre-commit hook and CI re-checks with `--check`, so a route split across lines cannot survive a commit. A parser would need a Workers runtime. |

## Open Questions & Future Decisions

### Resolved
1. ✅ Spec limits are asserted equal to `validate.rs` constants
   (`contract.rs:157-177`).
2. ✅ Routes are compared with the spec in both directions
   (`contract.rs:306-338`).

3. ✅ `HikeRequest` now carries `deny_unknown_fields`, so a smuggled field is
   refused rather than dropped (CONTRACT-010). `HikeRecord` stays tolerant by
   choice, for the reasons above.
4. ✅ A derived map key is asserted against the spec's `mapKey` pattern
   (CONTRACT-011), so the format string and the pattern cannot drift.
5. ✅ The textual route check is adequate because `cargo fmt` in the pre-commit
   hook keeps each route on one line, and CI re-checks with `--check`.

### Deferred

_None._

## References

- `docs/intent/trust-boundary/` — the constants and rules this segment pins
- `docs/intent/hike-record/` — the record whose schema is the cross-repo contract
- `docs/intent/store-and-transport/` — the router compared against the spec
- `../hike-club-api/src/models.rs` — the downstream deserializer
