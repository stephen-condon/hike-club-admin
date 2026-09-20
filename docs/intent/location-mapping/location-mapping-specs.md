# Location Mapping — Specs

EARS specs for `resources/hike-locations.json`, which lists the club's preserves
and gates every write to the bucket. Prefix `LOC`; see
`location-mapping-design.md`.

## Storage and Reading

- [x] **LOC-001**: The system shall store the location mapping at the R2 key `resources/hike-locations.json`, the same key the public API reads.
- [x] **LOC-002**: If the location mapping object is absent, then the system shall read it as an empty list rather than reporting an error, since the bucket starts without it.
- [x] **LOC-003**: If the stored location mapping is present but not valid JSON, then the system shall respond 502.

## Validation

- [x] **LOC-004**: When a location mapping is saved, the system shall require every `short_name` to be a valid slug.
- [x] **LOC-005**: When a location mapping is saved, the system shall require every `full_name` to be non-blank after trimming and at most 100 characters.
- [x] **LOC-006**: When a location mapping is saved, the system shall reject the list if any `short_name` appears more than once.
- [x] **LOC-007**: When a location mapping is saved, the system shall allow at most 200 entries, and shall accept an empty list as meaning nothing is schedulable yet.
- [x] **LOC-008**: If any entry in a submitted location mapping is invalid, then the system shall reject the whole list and leave the stored mapping unchanged.

## Consequences and Editing

- [x] **LOC-009**: The system shall permit a hike record or trail map to be written only for a slug that appears in the stored location mapping, so the mapping is the allowlist of keys this worker may create.
- [x] **LOC-010**: When a location is added to the mapping, the system shall thereby make that location schedulable, with no further step required.
- [x] **LOC-011**: When a location is removed from the mapping, the system shall leave its hike record and trail map in the bucket, so re-adding the location restores them.
- [x] **LOC-012**: When the admin saves the locations editor, the admin page shall send the entire list, replacing the stored mapping rather than patching it.
- [x] **LOC-013**: The system shall report the slugs that have a stored hike record or trail map but no entry in the location mapping, stating for each whether a record, a map, or both are present.
- [x] **LOC-014**: While any orphaned slug exists, the admin page shall list them read-only and state that re-adding the location recovers them.
- [x] **LOC-015**: When the admin edits an existing location, the admin page shall prevent its `short_name` from being changed, since the name is also the object key and a rename would strand the location's record and map.
