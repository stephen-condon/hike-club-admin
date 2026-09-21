# Arrow: contract

`openapi.yaml` as an executable contract: held equal to `validate.rs`, to the
router, and to the bytes `hike-club-api` deserializes.

## Status

**AUDITED** — last audited 2026-09-20 (git SHA `c02926e`). Specs verified
against code; all three deferred questions resolved, two of them by closing
real drift risks.

## References

### HLD
- `docs/high-level-design.md` — Key Design Decisions

### LLD
- `docs/intent/contract/contract-design.md`

### EARS
- `docs/intent/contract/contract-specs.md` (11 specs)

### Tests
- `tests/contract.rs` — the whole file is this segment's test surface

### Code
- `openapi.yaml` — the contract itself
- `src/validate.rs:11-20` — the constants the spec is asserted against
- `src/models.rs:11-25` — the cross-repo record shape
- `src/lib.rs:30-69` — the routes the spec is compared with

## Architecture

**Purpose:** Stop three pairs of things drifting apart — spec and validator,
spec and router, this repo and `hike-club-api`.

**Key Components:**
1. `validator_for` (`contract.rs:40`) — compiles an OpenAPI component schema
   into a draft-07 validator, rewriting `$ref`s and desugaring `nullable`.
2. `spec_limits_match_validate_limits` (`contract.rs:157`) — asserts the spec's
   numbers **equal** the Rust constants.
3. `spec_rejects_every_body_validate_rejects` (`contract.rs:183`) — every
   rejection path must have a matching spec constraint.
4. `the_router_and_the_spec_describe_the_same_routes` (`contract.rs:307`) —
   compared in both directions.

## Spec Coverage

| Category | Spec IDs | Implemented | Deferred | Gaps |
|----------|----------|-------------|----------|------|
| Schema conformance | CONTRACT-001 to -003 | 3 | 0 | 0 |
| Anti-drift assertions | CONTRACT-004 to -007, -010, -011 | 6 | 0 | 0 |
| Mechanics | CONTRACT-008 to -009 | 2 | 0 | 0 |

**Summary:** 11 of 11 active specs implemented; 0 deferred.

## Key Findings

1. **This segment owns no runtime behavior.** It was kept as its own node during
   mapping because `openapi.yaml` has cross-repo consequence — a field rename
   here breaks the public API — not because it is a subsystem.
2. **The spec is asserted equal, not merely compatible** — `contract.rs:161-176`
   compares the spec's `maxLength`, `maxItems` and coordinate bounds to
   `validate.rs`'s constants directly, so tightening one without the other
   fails the build.
3. **Rejection parity runs both ways for slugs** — `contract.rs:271-299` asserts
   each bad slug fails *both* the `Slug` schema and `validate_slug`.
4. **Smuggled `id`/`mapKey` are now rejected by both sides** — the spec through
   `additionalProperties: false`, the implementation through
   `#[serde(deny_unknown_fields)]` on `HikeRequest`. `HikeRecord` stays tolerant
   on purpose: it reads stored data, where refusing an unexpected field would
   make a recoverable record unreadable.
5. **Route comparison is textual** — it greps `src/lib.rs` for
   `.{method}_async("{route}"` and counts `_async("` occurrences
   (`contract.rs:321,332`). Crude by admission, and it needs no Workers runtime.
6. **`nullable` desugaring is hand-rolled** (`contract.rs:17-34`) because
   OpenAPI 3.0's `nullable` is not a JSON Schema keyword.
7. **CONTRACT-009 has no citation at all** — that the suite runs in-process is
   a property of how it is written, not something any test asserts.

## Work Required

### Must Fix
_None._

### Should Fix
_None._

### Nice to Have
1. The textual route check would miss a route split across lines, which
   `cargo fmt` in the pre-commit hook prevents from ever being committed.
