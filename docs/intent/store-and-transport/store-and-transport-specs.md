# Store and Transport — Specs

EARS specs for the storage seam, its R2 implementation, the failure vocabulary,
and HTTP translation. Prefix `STORE`; see `store-and-transport-design.md`.

## Storage Seam

- [x] **STORE-001**: The system shall access storage through a byte-blob trait offering get, put, delete, and prefix-list, shaped like R2 so its real implementation is pure translation.
- [x] **STORE-002**: The system shall implement all handler logic generically over that trait, so every handler path is exercisable against an in-memory fake with no Workers runtime and no network.
- [x] **STORE-003**: The system shall provide an in-memory implementation that can be constructed in a mode where every operation fails, so upstream-failure paths are testable.
- [x] **STORE-004**: When storing an object, the system shall record its content type alongside its bytes.
- [x] **STORE-005**: When listing a prefix whose results span multiple R2 pages, the system shall follow the cursor until exhausted.

## Failure Vocabulary

- [x] **STORE-006**: If a storage operation fails, then the system shall respond 502, identifying the failure as a storage error.
- [x] **STORE-007**: If a stored object is not valid JSON, then the system shall respond 502 rather than 500, since the stored object is at fault rather than the request.
- [x] **STORE-008**: If serializing the system's own response types fails, then the system shall emit a fixed error body rather than panicking.
- [x] **STORE-009**: When deleting a key that does not exist, the system shall succeed, so a repeated delete is safe.
- [x] **STORE-014**: When responding with any status outside 2xx, the system shall carry a JSON body with an `error` field naming the reason, so the reason travels as text rather than being inferable only from the status code.
- [x] **STORE-015**: When the admin page receives a response outside 2xx, it shall display that response's `error` text to the admin, and shall not branch on the status code; the only status it distinguishes is 204, which carries no body.

## Transport

- [x] **STORE-010**: The system shall return handler results as a status plus already-serialized bytes, so no handler constructs a runtime response type.
- [x] **STORE-011**: When translating a handler result into an HTTP response, the system shall set the content type the handler chose.
- [x] **STORE-012**: The system shall serve the admin page from a single HTML file compiled into the binary, without a bundler or a static-assets binding.
- [x] **STORE-013**: The system shall expose a health endpoint that responds without touching storage.
