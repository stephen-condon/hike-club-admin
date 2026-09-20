# Trust Boundary — Specs

EARS specs for what may be written and where: slug shape, the known-location
check, and server-side key derivation. Prefix `TRUST`; see
`trust-boundary-design.md`.

## Slug Shape

- [x] **TRUST-001**: The system shall accept a slug only if it matches `^[a-z0-9]+(-[a-z0-9]+)*$`, enforced without a regular-expression dependency.
- [x] **TRUST-002**: The system shall reject a slug that is empty or longer than 64 characters.
- [x] **TRUST-003**: The system shall reject a slug that starts with, ends with, or contains a doubled hyphen.
- [x] **TRUST-004**: The system shall reject any slug that could escape its R2 key prefix, including path separators, parent-directory references, uppercase letters, spaces, dots, and non-ASCII characters.

## Key Derivation

- [x] **TRUST-005**: The system shall construct every R2 key it writes from a validated path slug and a fixed prefix, and shall never take a key or key fragment from a request body.
- [x] **TRUST-006**: The system shall define the hike request type without `id` or `mapKey` fields, so neither can be supplied by a caller.
- [x] **TRUST-007**: When building a hike record, the system shall set `mapKey` by derivation from the path slug, so the value stored inside the record is also server-derived.

## Ordering and Scope

- [x] **TRUST-008**: When a request is rejected by validation, the system shall leave the bucket unchanged, performing no partial write.
- [x] **TRUST-009**: When a request would write a hike record or a trail map, the system shall require the slug both to be well-formed and to name a location in the stored mapping; when a request only reads or deletes, the system shall require well-formedness alone.
