# Contract — Specs

EARS specs for holding `openapi.yaml`, `validate.rs`, the router, and the
cross-repo record shape to each other. Prefix `CONTRACT`; see
`contract-design.md`.

## Schema Conformance

- [x] **CONTRACT-001**: The test suite shall verify that every schema in `openapi.yaml` compiles as a JSON Schema validator.
- [x] **CONTRACT-002**: The test suite shall verify that each serialized response type validates against its OpenAPI schema, including the hike summary in both its scheduled and unscheduled shapes.
- [x] **CONTRACT-003**: The test suite shall assert the exact field key set of the stored hike record, since those are the bytes the public API deserializes and a rename would break it.

## Anti-Drift Assertions

- [x] **CONTRACT-004**: The test suite shall assert that the spec's slug length, trail count, name length, location count, and coordinate bounds are **equal** to the corresponding constants in the validator, so tightening either alone fails the build.
- [x] **CONTRACT-005**: The test suite shall assert that every request body the validator rejects also fails the spec's schema, including bodies carrying a `id` or `mapKey` the server derives.
- [x] **CONTRACT-006**: The test suite shall assert, for each malformed slug, that both the spec's `Slug` schema and the validator reject it, and for each well-formed slug that both accept it.
- [x] **CONTRACT-007**: The test suite shall compare the router's routes against the spec's paths in both directions, failing if the worker serves an undocumented route or the spec describes a route the worker does not serve.

## Mechanics

- [x] **CONTRACT-008**: When compiling an OpenAPI schema for validation, the test suite shall rewrite component references into a self-contained draft-07 document and desugar OpenAPI's `nullable` into an `anyOf` with a null branch.
- [x] **CONTRACT-009**: The contract test suite shall run in-process, requiring no network access and no deployed or dev worker, so it can run in the pre-push hook.
