---
parent: high-level-design
prefix: TRUST
---

# Trust Boundary

## Context and Design Philosophy

This worker is the only writer to a bucket that a public API reads. Everything
that reaches storage passes through `src/validate.rs`, and the single question
that module answers is: *may this request name this key?*

The guiding principle is that a key is never transcribed, only constructed. A
caller supplies a slug in the **path**; the worker proves it is well-formed,
proves it names a location that exists, and then builds the key itself from
constants and that slug. No request body field ever reaches a key, because the
request types have no field that could.

## Slug Rules

The accepted shape is `^[a-z0-9]+(-[a-z0-9]+)*$`, enforced by hand rather than
with the `regex` crate — one pattern does not justify the wasm binary growth.

| Rule | Rejects |
|---|---|
| Non-empty, at most 64 characters | `""`, 65+ characters |
| Lowercase ASCII letters, digits, hyphen only | `Danada`, `a b`, `café`, `map.png`, `hikes/x` |
| No leading, trailing, or doubled hyphen | `-lead`, `trail-`, `a--b` |

The cases that matter are the ones that would escape a key prefix: `..`,
`../secrets`, `hikes/x`. They are enumerated as a test
(`validate.rs:292-306`) and the same list is asserted against `openapi.yaml`'s
`Slug` schema (`contract.rs:271-299`), so the two definitions cannot drift.

## Two Independent Gates

Well-formedness is necessary but not sufficient. A write additionally requires
the slug to name an entry in the location mapping — see
`docs/intent/location-mapping/`. The two checks defend different things: the
pattern stops a key escaping its prefix, the allowlist stops a well-formed key
being created for a place that does not exist.

Read and delete paths check shape only, so an unknown but well-formed slug
returns 404 rather than 400. Write paths check both.

## Key Derivation

```
slug ──> HikeRecord::key(slug)      ──> hikes/{slug}.json
     └─> HikeRecord::map_key_for(slug) ──> hikes/{slug}/map.png
```

These two functions (`models.rs:29,36`) are the only places an R2 key is built.
`build_record` calls `map_key_for` to fill the record's `mapKey`
(`validate.rs:155`), so even the value stored *inside* the record is derived,
never echoed.

The longest key either function can produce is 78 bytes — `hikes/` plus a
64-character slug plus `/map.png` — against R2's 1024-byte key limit. The slug
cap bounds key length with 946 bytes to spare, so no separate key-length check
is reachable.

`HikeRequest` has no `id` and no `mapKey` field. That absence is the control,
not an oversight — deserialization has nowhere to put a smuggled key, and
`openapi.yaml` marks the schema `additionalProperties: false` so the contract
test can prove the spec rejects them too.

## Ordering

Validation completes before the store is touched. A rejected request writes
nothing — asserted for path-traversing slugs (`admin.rs:328-335`), for unknown
locations, and for bad map uploads (`admin.rs:466-474`), each checking that the
bucket still holds only what it held before.

## Decisions & Alternatives

| Decision | Chosen | Alternatives Considered | Rationale |
|---|---|---|---|
| Slug matching | Hand-rolled byte checks | The `regex` crate; a `once_cell` compiled pattern | One pattern does not justify the wasm size `regex` adds. The rules are six lines and fully tested. |
| Key source | Always the path slug | Accept `id`/`mapKey` in the body and validate them | A validated-but-supplied key is one refactor away from an unvalidated one. Deriving removes the class of bug. |
| Request type shape | Omit `id` and `mapKey` from `HikeRequest` | Include and ignore them; include and reject them | A field that cannot be expressed cannot be smuggled. Ignoring silently would also mislead API callers. |
| Allowlist | The location mapping | A pattern-only check; a separate allowlist | Pattern alone permits any well-formed key. The mapping already exists and already means "places we hike". |
| Read-path strictness | Shape only, not membership | Require membership everywhere | A read cannot create an object, so an unknown location is a 404, not a rejection. Keeps reads cheap — no mapping fetch. |
| Character set | Lowercase ASCII only | Allow Unicode with NFC normalisation | Slugs become R2 keys and URL path segments. Normalisation has edge cases; the club's names are ASCII. |

## Open Questions & Future Decisions

### Resolved
1. ✅ `id` and `mapKey` are derived, never read from a body
   (`validate.rs:330-335` and `contract.rs:242-261` assert both sides).
2. ✅ A rejected request writes nothing.

3. ✅ Read and delete paths check slug shape but not membership, and that is
   deliberate: they create nothing, membership would cost a mapping fetch per
   read, and 404 is the better answer for a location with no hike. Recorded on
   `validate_known_slug` so the asymmetry is not "fixed" later.
4. ✅ The 64-character slug cap bounds key length implicitly and sufficiently —
   78 bytes at most against R2's 1024-byte limit.

### Deferred

_None._

## References

- `docs/intent/location-mapping/` — the allowlist this module consults
- `docs/intent/contract/` — how these rules are held equal to `openapi.yaml`
- `docs/intent/hike-record/` — the record whose `mapKey` is derived here
