# Arrow: contract

`openapi.yaml` as an executable contract: held equal to `validate.rs`, to the
router, and to the bytes `hike-club-api` deserializes.

## Status

**MAPPED** — last audited 2026-09-19 (git SHA `c911233`). Reverse-engineered
from existing code. Owns no runtime behavior; see Key Findings.

## References

### HLD
- `docs/high-level-design.md` — Key Design Decisions

### LLD
- `docs/intent/contract/contract-design.md`

### EARS
- `docs/intent/contract/contract-specs.md` (9 specs)

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
| Anti-drift assertions | CONTRACT-004 to -007 | 4 | 0 | 0 |
| Mechanics | CONTRACT-008 to -009 | 2 | 0 | 0 |

**Summary:** 9 of 9 active specs implemented; 0 deferred.

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
4. **Smuggled `id`/`mapKey` are rejected by the spec** because both
   `HikeRequest` and `HikeRecord` set `additionalProperties: false`
   (`openapi.yaml:259,279`). Rust-side, serde simply ignores unknown fields, so
   the spec is the stricter of the two.
5. **Route comparison is textual** — it greps `src/lib.rs` for
   `.{method}_async("{route}"` and counts `_async("` occurrences
   (`contract.rs:321,332`). Crude by admission, and it needs no Workers runtime.
6. **`nullable` desugaring is hand-rolled** (`contract.rs:17-34`) because
   OpenAPI 3.0's `nullable` is not a JSON Schema keyword.

## Work Required

### Must Fix
_None._

### Should Fix
_None._

### Nice to Have
1. The textual route check would miss a route added with unusual formatting
   (line breaks between the method and the path string). Tolerable while
   `lib.rs` is 91 lines.
2. Finding 4 means body-level strictness lives only in the spec. If the Rust
   side should also reject unknown fields, that is `#[serde(deny_unknown_fields)]`
   and a new spec.
