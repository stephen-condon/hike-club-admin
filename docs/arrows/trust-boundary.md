# Arrow: trust-boundary

What may be written, and where. Slug rules, the known-location check, and the
server-side derivation of every R2 key this worker touches.

## Status

**MAPPED** — last audited 2026-09-19 (git SHA `c911233`). Reverse-engineered
from existing code; behavior fully observed, no gaps found.

## References

### HLD
- `docs/high-level-design.md` — Approach, Key Design Decisions

### LLD
- `docs/intent/trust-boundary/trust-boundary-design.md`

### EARS
- `docs/intent/trust-boundary/trust-boundary-specs.md` (9 specs)

### Tests
- `src/validate.rs:267-335` — slug acceptance, rejection, key-escape cases
- `src/admin.rs:328-335,466-474` — a rejected slug writes nothing
- `tests/contract.rs:240-299` — smuggled `id`/`mapKey` rejected by the spec too

### Code
- `src/validate.rs:41-81` — `validate_slug`, `validate_known_slug`
- `src/models.rs:27-39` — `key`, `map_key_for`
- `src/models.rs:41-49` — `HikeRequest`, which has no `id` or `mapKey`

## Architecture

**Purpose:** Guarantee that nothing a caller sends can name an R2 key. Every
key is constructed from a slug that has been proven well-formed *and* known.

**Key Components:**
1. `validate_slug` (`validate.rs:45`) — hand-rolled pattern match, no `regex`.
2. `validate_known_slug` (`validate.rs:73`) — membership in the location mapping.
3. `HikeRecord::key` / `map_key_for` (`models.rs:29,36`) — the only two places
   an R2 key is built.
4. `HikeRequest` (`models.rs:44`) — the absence of `id` and `mapKey` fields is
   itself the control.

## Spec Coverage

| Category | Spec IDs | Implemented | Deferred | Gaps |
|----------|----------|-------------|----------|------|
| Slug shape | TRUST-001 to -004 | 4 | 0 | 0 |
| Key derivation | TRUST-005 to -007 | 3 | 0 | 0 |
| Ordering | TRUST-008 to -009 | 2 | 0 | 0 |

**Summary:** 9 of 9 active specs implemented; 0 deferred.

## Key Findings

1. **Type-level control, not just a check** — `HikeRequest` has no `id` and no
   `mapKey` field, so there is nowhere in a request body to put one. The
   contract test at `tests/contract.rs:242-261` proves the *spec* rejects them
   too, closing the gap between the two repos.
2. **Hand-rolled matcher is deliberate** — `validate.rs:41-44` records that
   pulling in `regex` for one pattern would bloat the wasm binary.
3. **The named adversarial cases are tested** — `..`, `../secrets`, `hikes/x`,
   `Danada`, `a b`, `map.png`, `café` (`validate.rs:294-306`), and the same list
   is asserted against the OpenAPI `Slug` schema (`contract.rs:274-294`).
4. **Two-stage check** — well-formedness alone is not enough; a write also needs
   the slug to name a known location, so the allowlist and the pattern are
   independent defences.
5. **Read paths check shape only** — `get_hike`, `get_map` and `delete_hike`
   call `validate_slug` but not `validate_known_slug`, so a well-formed unknown
   slug yields 404 rather than 400. Writes check both.
6. **TRUST-006 has no test citing it** — that the request type has no `id` or
   `mapKey` field is enforced by the type system; there is no runtime behavior
   to assert. `contract.rs:242-261` covers the spec side instead.

## Work Required

### Must Fix
_None._

### Should Fix
_None._

### Nice to Have
1. Finding 5 is a deliberate-looking asymmetry with no comment explaining it.
   Confirm the intent so it does not get "fixed" later.
