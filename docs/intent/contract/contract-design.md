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

`HikeRequest` and `HikeRecord` both set `additionalProperties: false`, which is
what makes the spec reject a smuggled `id`. The Rust side does not mirror this:
serde ignores unknown fields by default, so a body carrying `id` deserializes
fine and the value is dropped.

That is safe — `build_record` derives `id` from the path regardless — but it
means the two sides reject for different reasons. The spec rejects the request;
the implementation ignores the field. Both prevent the attack; only the spec
reports it.

## Decisions & Alternatives

| Decision | Chosen | Alternatives Considered | Rationale |
|---|---|---|---|
| Spec's role | Executable contract, asserted against code | Documentation only; generate code from the spec; generate the spec from code | A write API's spec that drifts is worse than none. Generation in either direction would couple the repos' build systems. |
| Limit checking | `assert_eq!` spec numbers to Rust constants | Assert the spec is merely no stricter | Equality catches a change in either direction. "Compatible" would let the spec quietly go stale. |
| Rejection parity | Every body/slug `validate.rs` rejects must fail the spec | Spot-check a few; property-test | Enumerated cases are readable and name the attack they prevent. |
| Route comparison | Textual match against `src/lib.rs` | Parse the router; integration-test every route | Parsing needs a Workers runtime. The router is a dozen lines, so text matching is adequate and free. |
| `nullable` handling | Hand-rolled desugaring | An OpenAPI-aware validator crate; drop `nullable` from the spec | One small function versus a heavier dependency in dev-deps. |
| Cross-repo contract | Assert the exact field key set | Trust the schema; share a crate between repos | A shared crate would couple deploys. The key-set assertion fails here rather than in the public API's deserializer. |
| Unknown request fields | Rejected by the spec, ignored by serde | `#[serde(deny_unknown_fields)]` on both | Not a decision anyone recorded — see Open Questions 1. |

## Open Questions & Future Decisions

### Resolved
1. ✅ Spec limits are asserted equal to `validate.rs` constants
   (`contract.rs:157-177`).
2. ✅ Routes are compared with the spec in both directions
   (`contract.rs:306-338`).

### Deferred
1. The spec forbids unknown request fields; serde silently ignores them. Should
   the Rust types carry `#[serde(deny_unknown_fields)]` so both sides reject for
   the same reason? Behavior is safe either way.
2. The route check matches source text, so a route written across multiple lines
   between the method and its path string would be missed.
3. Nothing asserts that `openapi.yaml`'s `mapKey` pattern
   (`^hikes/[a-z0-9]+(-[a-z0-9]+)*/map\.png$`) stays in step with
   `map_key_for`'s format string; they are independently maintained.

## References

- `docs/intent/trust-boundary/` — the constants and rules this segment pins
- `docs/intent/hike-record/` — the record whose schema is the cross-repo contract
- `docs/intent/store-and-transport/` — the router compared against the spec
- `../hike-club-api/src/models.rs` — the downstream deserializer
